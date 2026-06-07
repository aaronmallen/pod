use std::{collections::HashMap, time::Duration};

use tokio::time::Instant;

use super::{
  job::{JobKey, JobKind},
  subject::Subject,
};
use crate::config::FeatureFlags;

const BACKOFF_BASE: Duration = Duration::from_secs(2);
const BACKOFF_CAP: Duration = Duration::from_secs(300);
const PARK_DELAY: Duration = Duration::from_secs(365 * 24 * 60 * 60);

#[derive(Default)]
pub struct Schedule {
  entries: Vec<Entry>,
  features: FeatureFlags,
}

impl Schedule {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with_features(features: FeatureFlags) -> Self {
    Self {
      features,
      ..Self::default()
    }
  }

  pub fn due(&self, now: Instant) -> Vec<JobKey> {
    self
      .entries
      .iter()
      .filter(|entry| !entry.in_flight && entry.next_run_at <= now)
      .map(|entry| entry.key)
      .collect()
  }

  pub fn enroll(&mut self, subject: Subject, now: Instant) {
    self.enroll_kinds(subject, JobKind::for_subject(subject), now);
  }

  pub fn enroll_kinds(&mut self, subject: Subject, kinds: impl IntoIterator<Item = JobKind>, now: Instant) {
    self.enroll_kinds_seeded(subject, kinds, now, &HashMap::new());
  }

  pub fn enroll_kinds_seeded(
    &mut self,
    subject: Subject,
    kinds: impl IntoIterator<Item = JobKind>,
    now: Instant,
    seeds: &HashMap<JobKind, Instant>,
  ) {
    for kind in kinds {
      if !kind.is_feature_enabled(&self.features) {
        continue;
      }
      let key = JobKey::new(kind, subject);
      if !self.entries.iter().any(|entry| entry.key == key) {
        let next_run_at = seeds.get(&kind).copied().unwrap_or(now);
        self.entries.push(Entry::new(key, next_run_at));
      }
    }
  }

  pub fn make_due_now(&mut self, kind: JobKind, now: Instant) {
    for entry in self.entries.iter_mut().filter(|entry| entry.key.kind == kind) {
      entry.consecutive_failures = 0;
      entry.next_run_at = now;
    }
  }

  pub fn make_due_now_for_subject(&mut self, kind: JobKind, subject: Subject, now: Instant) {
    for entry in self
      .entries
      .iter_mut()
      .filter(|entry| entry.key.kind == kind && entry.key.subject == subject)
    {
      entry.consecutive_failures = 0;
      entry.next_run_at = now;
    }
  }

  pub fn mark_in_flight(&mut self, key: JobKey) {
    if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
      entry.in_flight = true;
    }
  }

  pub fn next_deadline(&self) -> Option<Instant> {
    self
      .entries
      .iter()
      .filter(|entry| !entry.in_flight)
      .map(|entry| entry.next_run_at)
      .min()
  }

  pub fn next_in(&self, key: JobKey, now: Instant) -> Option<Duration> {
    self
      .entries
      .iter()
      .find(|entry| entry.key == key)
      .map(|entry| entry.next_run_at.saturating_duration_since(now))
  }

  pub fn reschedule_failure(&mut self, key: JobKey, now: Instant) {
    if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
      entry.in_flight = false;
      entry.consecutive_failures += 1;
      entry.next_run_at = now + backoff_delay(entry.consecutive_failures);
    }
  }

  pub fn reschedule_permanent(&mut self, key: JobKey, now: Instant) {
    if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
      entry.in_flight = false;
      entry.consecutive_failures = 0;
      entry.next_run_at = now + PARK_DELAY;
    }
  }

  pub fn reschedule_success(&mut self, key: JobKey, now: Instant) {
    if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
      entry.in_flight = false;
      entry.consecutive_failures = 0;
      entry.next_run_at = now + key.kind.interval();
    }
  }

  pub fn reschedule_throttle(&mut self, key: JobKey, now: Instant, delay: Duration) {
    if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
      entry.in_flight = false;
      entry.next_run_at = now + delay;
    }
  }

  pub fn run_now(&mut self, subject: Subject, now: Instant) {
    for entry in self.entries.iter_mut().filter(|entry| entry.key.subject == subject) {
      entry.consecutive_failures = 0;
      entry.next_run_at = now;
    }
  }

  pub fn withdraw(&mut self, subject: Subject) {
    self.entries.retain(|entry| entry.key.subject != subject);
  }
}

struct Entry {
  consecutive_failures: u32,
  in_flight: bool,
  key: JobKey,
  next_run_at: Instant,
}

impl Entry {
  fn new(key: JobKey, next_run_at: Instant) -> Self {
    Self {
      consecutive_failures: 0,
      in_flight: false,
      key,
      next_run_at,
    }
  }
}

fn backoff_delay(consecutive_failures: u32) -> Duration {
  let exponent = consecutive_failures.saturating_sub(1).min(8);
  BACKOFF_BASE.saturating_mul(1u32 << exponent).min(BACKOFF_CAP)
}

#[cfg(test)]
mod tests {
  use super::*;

  const CHARACTER: Subject = Subject::Character(42);
  const KEY: JobKey = JobKey {
    kind: JobKind::CharacterProfile,
    subject: CHARACTER,
  };

  mod backoff_delay {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_doubles_from_the_base() {
      assert_eq!(backoff_delay(1), Duration::from_secs(2));
      assert_eq!(backoff_delay(2), Duration::from_secs(4));
      assert_eq!(backoff_delay(3), Duration::from_secs(8));
    }

    #[test]
    fn it_clamps_at_the_cap() {
      assert_eq!(backoff_delay(100), Duration::from_secs(300));
    }
  }

  mod schedule {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_enrolls_a_subject_due_immediately() {
      let now = Instant::now();
      let mut schedule = Schedule::new();

      schedule.enroll(CHARACTER, now);

      assert!(schedule.due(now).contains(&KEY));
    }

    #[tokio::test]
    async fn it_does_not_duplicate_or_reset_on_re_enroll() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      schedule.enroll(CHARACTER, now);
      schedule.reschedule_success(KEY, now);

      schedule.enroll(CHARACTER, now + Duration::from_secs(10));

      assert!(!schedule.due(now + Duration::from_secs(20)).contains(&KEY));
    }

    #[tokio::test]
    async fn it_excludes_in_flight_and_future_jobs_from_due() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      schedule.enroll(CHARACTER, now + Duration::from_secs(60));

      assert_eq!(schedule.due(now), vec![]);

      let mut schedule = Schedule::new();
      schedule.enroll(CHARACTER, now);
      schedule.mark_in_flight(KEY);

      assert!(!schedule.due(now).contains(&KEY));
    }

    #[tokio::test]
    async fn it_reschedules_success_one_interval_out() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      schedule.enroll(CHARACTER, now);
      schedule.mark_in_flight(KEY);

      schedule.reschedule_success(KEY, now);

      assert!(!schedule.due(now).contains(&KEY));
      assert!(schedule.due(now + JobKind::CharacterProfile.interval()).contains(&KEY));
    }

    #[tokio::test]
    async fn it_backs_off_exponentially_on_repeated_failures() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      schedule.enroll(CHARACTER, now);

      schedule.reschedule_failure(KEY, now);
      assert!(!schedule.due(now + Duration::from_secs(1)).contains(&KEY));
      assert!(schedule.due(now + Duration::from_secs(2)).contains(&KEY));

      schedule.reschedule_failure(KEY, now);
      assert!(!schedule.due(now + Duration::from_secs(2)).contains(&KEY));
      assert!(schedule.due(now + Duration::from_secs(4)).contains(&KEY));
    }

    #[tokio::test]
    async fn it_parks_a_permanently_failed_job_far_into_the_future() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      schedule.enroll(CHARACTER, now);
      schedule.mark_in_flight(KEY);

      schedule.reschedule_permanent(KEY, now);

      assert!(
        !schedule.due(now + Duration::from_secs(24 * 60 * 60)).contains(&KEY),
        "a permanently parked job must not become due again on the normal cadence"
      );
    }

    #[tokio::test]
    async fn it_revives_a_parked_job_on_run_now() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      schedule.enroll(CHARACTER, now);
      schedule.reschedule_permanent(KEY, now);

      schedule.run_now(CHARACTER, now);

      assert!(
        schedule.due(now).contains(&KEY),
        "re-authentication via run_now must revive a parked job"
      );
    }

    #[tokio::test]
    async fn it_reschedules_a_throttle_without_escalating() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      schedule.enroll(CHARACTER, now);

      schedule.reschedule_throttle(KEY, now, Duration::from_secs(90));

      assert!(!schedule.due(now + Duration::from_secs(89)).contains(&KEY));
      assert!(schedule.due(now + Duration::from_secs(90)).contains(&KEY));
    }

    #[tokio::test]
    async fn it_forces_due_on_run_now_and_clears_backoff() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      schedule.enroll(CHARACTER, now);
      schedule.reschedule_throttle(KEY, now, Duration::from_secs(300));

      schedule.run_now(CHARACTER, now);

      assert!(schedule.due(now).contains(&KEY));
    }

    #[tokio::test]
    async fn it_makes_a_kind_due_now_clearing_its_backoff() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      schedule.enroll(CHARACTER, now);
      schedule.reschedule_throttle(KEY, now, Duration::from_secs(300));

      schedule.make_due_now(JobKind::CharacterProfile, now);

      assert!(schedule.due(now).contains(&KEY));
    }

    #[tokio::test]
    async fn it_makes_every_entry_of_a_kind_due_now_across_subjects() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      let other = Subject::Character(43);
      schedule.enroll(CHARACTER, now);
      schedule.enroll(other, now);
      schedule.reschedule_success(KEY, now);
      schedule.reschedule_success(JobKey::new(JobKind::CharacterProfile, other), now);

      schedule.make_due_now(JobKind::CharacterProfile, now);

      let due = schedule.due(now);
      assert!(due.contains(&KEY));
      assert!(due.contains(&JobKey::new(JobKind::CharacterProfile, other)));
    }

    #[tokio::test]
    async fn it_withdraws_a_subject() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      schedule.enroll(CHARACTER, now);

      schedule.withdraw(CHARACTER);

      assert_eq!(schedule.due(now), vec![]);
      assert_eq!(schedule.next_deadline(), None);
    }

    #[tokio::test]
    async fn it_reports_the_earliest_deadline() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      schedule.enroll(CHARACTER, now + Duration::from_secs(30));

      assert_eq!(schedule.next_deadline(), Some(now + Duration::from_secs(30)));
    }

    #[tokio::test]
    async fn it_reports_the_remaining_time_until_a_keys_next_run() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      schedule.enroll(CHARACTER, now);
      schedule.reschedule_success(KEY, now);

      assert_eq!(schedule.next_in(KEY, now), Some(JobKind::CharacterProfile.interval()));
      assert_eq!(
        schedule.next_in(JobKey::new(JobKind::CharacterProfile, Subject::Character(999)), now),
        None,
        "an unknown key has no scheduled next run"
      );
    }

    #[tokio::test]
    async fn it_makes_a_kind_due_now_for_a_single_subject_only() {
      let now = Instant::now();
      let mut schedule = Schedule::new();
      let other = Subject::Character(43);
      schedule.enroll(CHARACTER, now);
      schedule.enroll(other, now);
      schedule.reschedule_success(JobKey::new(JobKind::AssetSync, CHARACTER), now);
      schedule.reschedule_success(JobKey::new(JobKind::AssetSync, other), now);

      schedule.make_due_now_for_subject(JobKind::AssetSync, CHARACTER, now);

      let due = schedule.due(now);
      assert!(due.contains(&JobKey::new(JobKind::AssetSync, CHARACTER)));
      assert!(
        !due.contains(&JobKey::new(JobKind::AssetSync, other)),
        "a profile landing for one subject must not re-fire another subject's gather"
      );
    }

    #[tokio::test]
    async fn it_does_not_schedule_a_job_whose_feature_is_disabled() {
      use crate::config::FeatureFlags;

      let now = Instant::now();
      let flags: FeatureFlags = toml::from_str("wallet = false").unwrap();
      let mut schedule = Schedule::with_features(flags);

      schedule.enroll(CHARACTER, now);
      let due = schedule.due(now);

      let wallet = JobKey::new(JobKind::CharacterWallet, CHARACTER);
      assert!(
        !due.contains(&wallet),
        "a disabled feature's job must not be scheduled, got {due:?}"
      );
      assert!(JobKind::for_subject(CHARACTER).contains(&JobKind::CharacterWallet));
      assert!(due.contains(&KEY));
    }

    #[tokio::test]
    async fn it_schedules_character_skills_not_wallet_when_wallet_off_and_skill_monitoring_on() {
      let now = Instant::now();
      let flags: FeatureFlags = toml::from_str("wallet = false\nskill_monitoring = true").unwrap();
      let mut schedule = Schedule::with_features(flags);

      schedule.enroll(CHARACTER, now);
      let due = schedule.due(now);

      let skills = JobKey::new(JobKind::CharacterSkills, CHARACTER);
      let wallet = JobKey::new(JobKind::CharacterWallet, CHARACTER);
      assert!(
        due.contains(&skills),
        "SkillMonitoring is on, so CharacterSkills must be scheduled, got {due:?}"
      );
      assert!(
        !due.contains(&wallet),
        "Wallet is off, so CharacterWallet must not be scheduled, got {due:?}"
      );
    }
  }
}
