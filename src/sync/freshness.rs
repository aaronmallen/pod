use super::{
  job::JobKey,
  status::{Phase, SyncStatus},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Freshness {
  Attention,
  CatchingUp,
  Fresh,
  Refreshing,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FreshnessSummary {
  pub attention: usize,
  pub catching_up: usize,
  pub fresh: usize,
  pub refreshing: usize,
  pub total: usize,
}

impl FreshnessSummary {
  pub fn from_keys<'a>(status: &SyncStatus, keys: impl IntoIterator<Item = &'a JobKey>) -> Self {
    let mut summary = Self::default();
    for key in keys {
      summary.total += 1;
      match freshness_of(status, key) {
        Freshness::Attention => summary.attention += 1,
        Freshness::CatchingUp => summary.catching_up += 1,
        Freshness::Fresh => summary.fresh += 1,
        Freshness::Refreshing => summary.refreshing += 1,
      }
    }
    summary
  }

  pub fn record(&mut self, freshness: Freshness) {
    self.total += 1;
    match freshness {
      Freshness::Attention => self.attention += 1,
      Freshness::CatchingUp => self.catching_up += 1,
      Freshness::Fresh => self.fresh += 1,
      Freshness::Refreshing => self.refreshing += 1,
    }
  }

  pub fn is_up_to_date(&self) -> bool {
    self.attention == 0 && self.catching_up == 0 && self.refreshing == 0
  }

  #[allow(dead_code)]
  pub fn settled(&self) -> usize {
    self.fresh + self.attention
  }
}

pub fn freshness_of(status: &SyncStatus, key: &JobKey) -> Freshness {
  match status.phase(key) {
    None => Freshness::CatchingUp,
    Some(Phase::Done | Phase::Empty) => Freshness::Fresh,
    Some(Phase::Syncing | Phase::BackingOff) => Freshness::Refreshing,
    Some(Phase::Blocked | Phase::Failed | Phase::NotReady) => Freshness::Attention,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::sync::{Event, JobKind, Outcome, Subject};

  fn key(id: i64) -> JobKey {
    JobKey::new(JobKind::CharacterProfile, Subject::Character(id))
  }

  mod freshness_of {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reads_an_empty_seeded_job_as_fresh() {
      let mut status = SyncStatus::new();
      status.apply(&Event::Seeded {
        key: key(1),
        outcome: Outcome::Empty,
        next_in_secs: Some(3_600),
      });

      assert_eq!(freshness_of(&status, &key(1)), Freshness::Fresh);
      assert_eq!(status.next_in_secs(&key(1)), Some(3_600));
    }

    #[test]
    fn it_reads_a_synced_seeded_job_as_fresh_with_a_countdown() {
      let mut status = SyncStatus::new();
      status.apply(&Event::Seeded {
        key: key(1),
        outcome: Outcome::Synced {
          rows_touched: 4,
        },
        next_in_secs: Some(2_520),
      });

      assert_eq!(freshness_of(&status, &key(1)), Freshness::Fresh);
      assert_eq!(status.next_in_secs(&key(1)), Some(2_520));
    }

    #[test]
    fn it_reads_a_needs_reauth_seeded_job_as_attention() {
      let mut status = SyncStatus::new();
      status.apply(&Event::Seeded {
        key: key(1),
        outcome: Outcome::Blocked {
          reason: "needs re-authentication".to_string(),
        },
        next_in_secs: None,
      });

      assert_eq!(freshness_of(&status, &key(1)), Freshness::Attention);
      assert_eq!(status.reason(&key(1)), Some("needs re-authentication"));
    }

    #[test]
    fn it_reads_a_transient_backoff_as_refreshing_not_attention() {
      let mut status = SyncStatus::new();
      status.apply(&Event::BackingOff {
        key: key(1),
        retry_secs: 30,
      });

      assert_eq!(
        freshness_of(&status, &key(1)),
        Freshness::Refreshing,
        "a self-healing backoff stays calm and never raises the attention headline"
      );
    }

    #[test]
    fn it_reads_an_unreported_enrolled_job_as_catching_up() {
      let status = SyncStatus::new();

      assert_eq!(freshness_of(&status, &key(1)), Freshness::CatchingUp);
    }

    #[test]
    fn it_reads_a_running_job_as_refreshing() {
      let mut status = SyncStatus::new();
      status.apply(&Event::Started {
        key: key(1),
      });

      assert_eq!(freshness_of(&status, &key(1)), Freshness::Refreshing);
    }
  }

  mod summary {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reaches_up_to_date_when_every_job_is_fresh() {
      let mut status = SyncStatus::new();
      status.apply(&Event::Seeded {
        key: key(1),
        outcome: Outcome::Synced {
          rows_touched: 1,
        },
        next_in_secs: Some(3_600),
      });
      status.apply(&Event::Seeded {
        key: key(2),
        outcome: Outcome::Empty,
        next_in_secs: Some(60),
      });

      let summary = FreshnessSummary::from_keys(&status, &[key(1), key(2)]);

      assert_eq!(summary.fresh, 2);
      assert_eq!(summary.total, 2);
      assert!(summary.is_up_to_date());
    }

    #[test]
    fn it_counts_empty_as_fresh_alongside_synced() {
      let mut status = SyncStatus::new();
      status.apply(&Event::Seeded {
        key: key(1),
        outcome: Outcome::Empty,
        next_in_secs: Some(60),
      });

      let summary = FreshnessSummary::from_keys(&status, &[key(1)]);

      assert_eq!(
        summary.fresh, 1,
        "an empty result is a fresh success, never undercounted"
      );
    }

    #[test]
    fn it_is_not_up_to_date_while_a_job_needs_attention() {
      let mut status = SyncStatus::new();
      status.apply(&Event::Seeded {
        key: key(1),
        outcome: Outcome::Blocked {
          reason: "needs re-authentication".to_string(),
        },
        next_in_secs: None,
      });

      let summary = FreshnessSummary::from_keys(&status, &[key(1)]);

      assert_eq!(summary.attention, 1);
      assert!(!summary.is_up_to_date());
    }

    #[test]
    fn it_stays_up_to_date_through_a_transient_refresh() {
      let mut status = SyncStatus::new();
      status.apply(&Event::BackingOff {
        key: key(1),
        retry_secs: 30,
      });

      let summary = FreshnessSummary::from_keys(&status, &[key(1)]);

      assert_eq!(summary.attention, 0, "a backoff is not attention");
      assert_eq!(summary.refreshing, 1);
    }
  }
}
