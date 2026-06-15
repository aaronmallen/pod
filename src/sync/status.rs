use std::{
  collections::HashMap,
  time::{Duration, Instant},
};

use super::{event::Event, job::JobKey, outcome::Outcome};

#[derive(Clone, Debug, Default)]
pub struct OutboxStatus {
  rows: HashMap<i64, OutboxRow>,
}

impl OutboxStatus {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn apply(&mut self, event: &Event) {
    match event {
      Event::OutboxInflight {
        id,
      } => self.set(*id, OutboxPhase::Inflight, None),
      Event::OutboxDone {
        id,
      } => self.set(*id, OutboxPhase::Done, None),
      Event::OutboxRetrying {
        id, ..
      } => self.set(*id, OutboxPhase::Retrying, None),
      Event::OutboxFailed {
        id,
        reason,
      } => self.set(*id, OutboxPhase::Failed, Some(reason.clone())),
      Event::BackingOff {
        ..
      }
      | Event::Failed {
        ..
      }
      | Event::Finished {
        ..
      }
      | Event::GaveUp {
        ..
      }
      | Event::Heartbeat
      | Event::Restarted {
        ..
      }
      | Event::Scheduled {
        ..
      }
      | Event::Started {
        ..
      } => {}
    }
  }

  pub fn pending(&self) -> usize {
    self
      .rows
      .values()
      .filter(|row| matches!(row.phase, OutboxPhase::Inflight | OutboxPhase::Retrying))
      .count()
  }

  pub fn failed(&self) -> usize {
    self.count(OutboxPhase::Failed)
  }

  pub fn last_error(&self, id: i64) -> Option<&str> {
    self
      .rows
      .get(&id)
      .filter(|row| row.phase == OutboxPhase::Failed)
      .and_then(|row| row.last_error.as_deref())
  }

  fn count(&self, phase: OutboxPhase) -> usize {
    self.rows.values().filter(|row| row.phase == phase).count()
  }

  fn set(&mut self, id: i64, phase: OutboxPhase, last_error: Option<String>) {
    self.rows.insert(
      id,
      OutboxRow {
        phase,
        last_error,
      },
    );
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
  BackingOff,
  Blocked,
  Done,
  Empty,
  Failed,
  NotReady,
  Syncing,
}

#[derive(Clone, Debug, Default)]
pub struct SyncStatus {
  next_runs: HashMap<JobKey, Instant>,
  tasks: HashMap<JobKey, TaskStatus>,
}

impl SyncStatus {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn active(&self) -> usize {
    self.count(Phase::Syncing)
  }

  pub fn apply(&mut self, event: &Event) {
    match event {
      Event::BackingOff {
        key,
        retry_secs,
      } => self.set(*key, Phase::BackingOff, None, Some(*retry_secs)),
      Event::Failed {
        key,
        reason,
      } => self.set(*key, Phase::Failed, Some(reason.clone()), None),
      Event::Finished {
        key,
        outcome,
      } => {
        let (phase, reason) = phase_for_outcome(outcome);
        self.set(*key, phase, reason, None);
      }
      Event::GaveUp {
        ..
      }
      | Event::Heartbeat
      | Event::Restarted {
        ..
      } => {}
      Event::OutboxDone {
        ..
      }
      | Event::OutboxFailed {
        ..
      }
      | Event::OutboxInflight {
        ..
      }
      | Event::OutboxRetrying {
        ..
      } => {}
      Event::Scheduled {
        key,
        next_in_secs,
      } => self.set_next_in(*key, *next_in_secs),
      Event::Started {
        key,
      } => self.set(*key, Phase::Syncing, None, None),
    }
  }

  pub fn attention(&self) -> usize {
    self.count(Phase::Blocked) + self.count(Phase::NotReady)
  }

  pub fn done(&self) -> usize {
    self.count(Phase::Done) + self.count(Phase::Empty)
  }

  pub fn errors(&self) -> usize {
    self.count(Phase::BackingOff) + self.count(Phase::Failed)
  }

  pub fn is_syncing(&self) -> bool {
    self.active() > 0
  }

  pub fn phase(&self, key: &JobKey) -> Option<Phase> {
    self.tasks.get(key).map(|task| task.phase)
  }

  pub fn percent(&self) -> u8 {
    let total = self.total();
    if total == 0 {
      return 100;
    }
    ((self.done() * 100) / total) as u8
  }

  pub fn next_in_secs(&self, key: &JobKey) -> Option<u64> {
    if !self.tasks.contains_key(key) {
      return None;
    }
    self
      .next_runs
      .get(key)
      .map(|deadline| deadline.saturating_duration_since(Instant::now()).as_secs_f64().round() as u64)
  }

  pub fn reason(&self, key: &JobKey) -> Option<&str> {
    self.tasks.get(key).and_then(|task| task.reason.as_deref())
  }

  pub fn retry_secs(&self, key: &JobKey) -> Option<u64> {
    self.tasks.get(key).and_then(|task| task.retry_secs)
  }

  pub fn tasks(&self) -> impl Iterator<Item = &TaskStatus> {
    self.tasks.values()
  }

  pub fn total(&self) -> usize {
    self.tasks.len()
  }

  fn count(&self, phase: Phase) -> usize {
    self.tasks.values().filter(|task| task.phase == phase).count()
  }

  fn set(&mut self, key: JobKey, phase: Phase, reason: Option<String>, retry_secs: Option<u64>) {
    self.tasks.insert(
      key,
      TaskStatus {
        key,
        phase,
        reason,
        retry_secs,
      },
    );
  }

  fn set_next_in(&mut self, key: JobKey, next_in_secs: u64) {
    self
      .next_runs
      .insert(key, Instant::now() + Duration::from_secs(next_in_secs));
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskStatus {
  pub key: JobKey,
  pub phase: Phase,
  pub reason: Option<String>,
  pub retry_secs: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutboxPhase {
  Done,
  Failed,
  Inflight,
  Retrying,
}

#[derive(Clone, Debug)]
struct OutboxRow {
  phase: OutboxPhase,
  last_error: Option<String>,
}

fn phase_for_outcome(outcome: &Outcome) -> (Phase, Option<String>) {
  match outcome {
    Outcome::Blocked {
      reason,
    }
    | Outcome::Skipped {
      reason,
    } => (Phase::Blocked, Some(reason.clone())),
    Outcome::Empty => (Phase::Empty, None),
    Outcome::Failed {
      reason,
    } => (Phase::Failed, Some(reason.clone())),
    Outcome::NotReady => (Phase::NotReady, None),
    Outcome::Synced {
      ..
    } => (Phase::Done, None),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::sync::{job::JobKind, subject::Subject};

  fn key(id: i64) -> JobKey {
    JobKey::new(JobKind::CharacterProfile, Subject::Character(id))
  }

  mod apply {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::sync::Outcome;

    fn finished(key: JobKey, outcome: Outcome) -> Event {
      Event::Finished {
        key,
        outcome,
      }
    }

    #[test]
    fn it_distinguishes_synced_empty_blocked_and_not_ready_outcomes() {
      let mut status = SyncStatus::new();

      status.apply(&finished(
        key(1),
        Outcome::Synced {
          rows_touched: 3,
        },
      ));
      status.apply(&finished(key(2), Outcome::Empty));
      status.apply(&finished(
        key(3),
        Outcome::Blocked {
          reason: "no scope".to_string(),
        },
      ));
      status.apply(&finished(key(4), Outcome::NotReady));

      assert_eq!(status.phase(&key(1)), Some(Phase::Done));
      assert_eq!(status.phase(&key(2)), Some(Phase::Empty));
      assert_eq!(status.phase(&key(3)), Some(Phase::Blocked));
      assert_eq!(status.phase(&key(4)), Some(Phase::NotReady));
      assert_eq!(status.reason(&key(3)), Some("no scope"));
    }

    #[test]
    fn it_counts_an_empty_outcome_as_a_benign_success_not_attention() {
      let mut status = SyncStatus::new();

      status.apply(&finished(
        key(1),
        Outcome::Synced {
          rows_touched: 1,
        },
      ));
      status.apply(&finished(key(2), Outcome::Empty));
      status.apply(&finished(
        key(3),
        Outcome::Blocked {
          reason: "no scope".to_string(),
        },
      ));
      status.apply(&finished(key(4), Outcome::NotReady));

      assert_eq!(status.done(), 2, "synced and empty are both up-to-date successes");
      assert_eq!(status.attention(), 2, "only blocked and not-ready need attention");
      assert_eq!(status.errors(), 0, "an empty/blocked job is not an error");
      assert_eq!(
        status.percent(),
        50,
        "percent reflects the synced and empty jobs out of four"
      );
    }

    #[test]
    fn it_maps_a_skipped_outcome_to_blocked_with_its_reason() {
      let mut status = SyncStatus::new();

      status.apply(&finished(
        key(1),
        Outcome::Skipped {
          reason: "feature off".to_string(),
        },
      ));

      assert_eq!(status.phase(&key(1)), Some(Phase::Blocked));
      assert_eq!(status.reason(&key(1)), Some("feature off"));
    }

    #[test]
    fn it_tracks_the_latest_phase_per_job() {
      let mut status = SyncStatus::new();

      status.apply(&Event::Started {
        key: key(1),
      });
      status.apply(&Event::Started {
        key: key(2),
      });
      status.apply(&Event::Finished {
        key: key(1),
        outcome: crate::sync::Outcome::synced(),
      });

      assert_eq!(status.active(), 1);
      assert_eq!(status.done(), 1);
      assert_eq!(status.total(), 2);
      assert_eq!(status.percent(), 50);
    }

    #[test]
    fn it_records_backoff_and_counts_it_as_an_error() {
      let mut status = SyncStatus::new();

      status.apply(&Event::Started {
        key: key(1),
      });
      status.apply(&Event::BackingOff {
        key: key(1),
        retry_secs: 30,
      });

      assert_eq!(status.errors(), 1);
      assert_eq!(status.is_syncing(), false);
      let task = status.tasks().next().unwrap();
      assert_eq!(task.phase, Phase::BackingOff);
      assert_eq!(task.retry_secs, Some(30));
    }

    #[test]
    fn it_ignores_heartbeats() {
      let mut status = SyncStatus::new();

      status.apply(&Event::Heartbeat);

      assert_eq!(status.total(), 0);
      assert_eq!(status.percent(), 100);
    }

    #[test]
    fn it_retains_a_failure_reason_and_clears_it_on_recovery() {
      let mut status = SyncStatus::new();

      status.apply(&Event::Failed {
        key: key(1),
        reason: "token expired".to_string(),
      });

      assert_eq!(status.reason(&key(1)), Some("token expired"));
      assert_eq!(status.tasks().next().unwrap().reason.as_deref(), Some("token expired"));

      status.apply(&Event::Started {
        key: key(1),
      });

      assert_eq!(status.reason(&key(1)), None);

      status.apply(&Event::Failed {
        key: key(1),
        reason: "rate limited".to_string(),
      });
      status.apply(&Event::Finished {
        key: key(1),
        outcome: crate::sync::Outcome::synced(),
      });

      assert_eq!(status.reason(&key(1)), None);
    }

    #[test]
    fn it_ignores_outbox_events() {
      let mut status = SyncStatus::new();

      status.apply(&Event::OutboxInflight {
        id: 1,
      });
      status.apply(&Event::OutboxFailed {
        id: 1,
        reason: "boom".to_string(),
      });

      assert_eq!(status.total(), 0, "outbox rows do not enter the job-keyed aggregate");
    }

    #[test]
    fn it_records_the_next_run_from_a_scheduled_event() {
      let mut status = SyncStatus::new();

      status.apply(&Event::Finished {
        key: key(1),
        outcome: crate::sync::Outcome::synced(),
      });
      status.apply(&Event::Scheduled {
        key: key(1),
        next_in_secs: 2_520,
      });

      assert_eq!(status.next_in_secs(&key(1)), Some(2_520));
      assert_eq!(
        status.phase(&key(1)),
        Some(Phase::Done),
        "the next-run update must not disturb the job's phase"
      );
    }

    #[test]
    fn it_does_not_fabricate_a_done_task_for_an_unknown_scheduled_key() {
      let mut status = SyncStatus::new();

      status.apply(&Event::Scheduled {
        key: key(1),
        next_in_secs: 600,
      });

      assert_eq!(
        status.total(),
        0,
        "a Scheduled event for a key with no started task must not invent a phantom Done task"
      );
      assert_eq!(status.done(), 0);
      assert_eq!(status.next_in_secs(&key(1)), None);
    }

    #[test]
    fn it_keeps_the_next_run_across_a_later_phase_change() {
      let mut status = SyncStatus::new();

      status.apply(&Event::Scheduled {
        key: key(1),
        next_in_secs: 600,
      });
      status.apply(&Event::Started {
        key: key(1),
      });

      assert_eq!(
        status.next_in_secs(&key(1)),
        Some(600),
        "a re-sync starting does not erase the engine-computed next-run"
      );
    }
  }

  mod outbox_status {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_counts_inflight_and_retrying_rows_as_pending() {
      let mut status = OutboxStatus::new();

      status.apply(&Event::OutboxInflight {
        id: 1,
      });
      status.apply(&Event::OutboxRetrying {
        id: 2,
        retry_secs: 30,
      });

      assert_eq!(status.pending(), 2);
      assert_eq!(status.failed(), 0);
    }

    #[test]
    fn it_drops_a_row_from_pending_once_it_is_done() {
      let mut status = OutboxStatus::new();

      status.apply(&Event::OutboxInflight {
        id: 1,
      });
      assert_eq!(status.pending(), 1);

      status.apply(&Event::OutboxDone {
        id: 1,
      });

      assert_eq!(status.pending(), 0, "a done row is no longer pending");
      assert_eq!(status.failed(), 0);
    }

    #[test]
    fn it_records_a_failed_rows_last_error_and_counts_it() {
      let mut status = OutboxStatus::new();

      status.apply(&Event::OutboxInflight {
        id: 7,
      });
      status.apply(&Event::OutboxFailed {
        id: 7,
        reason: "403 Forbidden".to_string(),
      });

      assert_eq!(status.pending(), 0, "a failed row is no longer pending");
      assert_eq!(status.failed(), 1);
      assert_eq!(status.last_error(7), Some("403 Forbidden"));
      assert_eq!(status.last_error(999), None, "an unknown row has no error");
    }

    #[test]
    fn it_clears_a_prior_error_when_a_row_recovers_to_done() {
      let mut status = OutboxStatus::new();

      status.apply(&Event::OutboxFailed {
        id: 1,
        reason: "transient".to_string(),
      });
      status.apply(&Event::OutboxDone {
        id: 1,
      });

      assert_eq!(status.failed(), 0);
      assert_eq!(status.last_error(1), None, "a recovered row reports no error");
    }

    #[test]
    fn it_ignores_read_side_and_heartbeat_events() {
      let mut status = OutboxStatus::new();

      status.apply(&Event::Heartbeat);
      status.apply(&Event::Started {
        key: key(1),
      });
      status.apply(&Event::Finished {
        key: key(1),
        outcome: crate::sync::Outcome::synced(),
      });

      assert_eq!(status.pending(), 0);
      assert_eq!(status.failed(), 0);
    }
  }
}
