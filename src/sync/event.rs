use super::{job::JobKey, outcome::Outcome};

#[derive(Clone, Debug)]
pub enum Event {
  Seeded {
    key: JobKey,
    outcome: Outcome,
    next_in_secs: Option<u64>,
  },
  BackingOff {
    key: JobKey,
    retry_secs: u64,
  },
  Failed {
    key: JobKey,
    reason: String,
  },
  Finished {
    key: JobKey,
    outcome: Outcome,
  },
  GaveUp {
    reason: String,
  },
  Heartbeat,
  OutboxDone {
    id: i64,
  },
  OutboxFailed {
    id: i64,
    reason: String,
  },
  OutboxInflight {
    id: i64,
  },
  OutboxRetrying {
    id: i64,
    #[cfg(test)]
    retry_secs: u64,
  },
  Restarted {
    #[cfg(test)]
    attempt: u32,
  },
  Scheduled {
    key: JobKey,
    next_in_secs: u64,
  },
  Started {
    key: JobKey,
  },
}
