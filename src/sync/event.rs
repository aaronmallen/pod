use super::{job::JobKey, outcome::Outcome};

#[derive(Clone, Debug)]
pub enum Event {
  BackingOff {
    key: JobKey,
    // Exercised only by unit tests / forward-looking sync surface; no production reader yet.
    #[allow(dead_code)]
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
    // Exercised only by unit tests / forward-looking sync surface; no production reader yet.
    #[allow(dead_code)]
    retry_secs: u64,
  },
  Restarted {
    // Exercised only by unit tests / forward-looking sync surface; no production reader yet.
    #[allow(dead_code)]
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
