use std::time::Instant;

/// Tracks the state of an ESI data synchronisation cycle.
#[derive(Clone, Debug, Default)]
pub struct SyncState {
  /// Timestamp of the most recent completed sync.
  last_synced: Option<Instant>,
  /// Number of operations completed in the current cycle.
  ops_done: u8,
  /// Total operations expected in the current cycle.
  ops_total: u8,
}

impl SyncState {
  /// Records one completed ESI operation and updates the sync timestamp.
  pub fn complete_op(&mut self) {
    self.ops_done = self.ops_done.saturating_add(1);
    self.last_synced = Some(Instant::now());
  }

  /// Returns true while a sync cycle is in progress.
  pub fn is_syncing(&self) -> bool {
    self.ops_total > 0 && self.ops_done < self.ops_total
  }

  /// Returns the fraction of operations completed (0.0–1.0).
  ///
  /// Returns 0.0 before the first `start` call.
  pub fn progress(&self) -> f32 {
    if self.ops_total == 0 {
      return 0.0;
    }
    (self.ops_done as f32 / self.ops_total as f32).min(1.0)
  }

  /// Updates the sync timestamp without incrementing the operation count.
  ///
  /// Call this when a background auto-refresh completes outside a
  /// `RefreshAll` cycle.
  pub fn record_background_sync(&mut self) {
    self.last_synced = Some(Instant::now());
  }

  /// Returns seconds elapsed since the last completed sync, or 0 if no
  /// sync has occurred.
  pub fn secs_since_sync(&self) -> u64 {
    self.last_synced.map_or(0, |t| t.elapsed().as_secs())
  }

  /// Begins a new sync cycle expecting `n` ESI operations.
  pub fn start(&mut self, n: u8) {
    self.ops_done = 0;
    self.ops_total = n;
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod complete_op {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_increments_ops_done() {
      let mut s = SyncState::default();
      s.start(3);
      s.complete_op();

      assert_eq!(s.progress(), 1.0 / 3.0);
    }

    #[test]
    fn it_updates_the_sync_timestamp() {
      let mut s = SyncState::default();
      s.start(1);
      s.complete_op();

      assert_eq!(s.secs_since_sync(), 0);
    }
  }

  mod is_syncing {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_false_before_start() {
      let s = SyncState::default();

      assert_eq!(s.is_syncing(), false);
    }

    #[test]
    fn it_returns_true_after_start() {
      let mut s = SyncState::default();
      s.start(2);

      assert_eq!(s.is_syncing(), true);
    }

    #[test]
    fn it_returns_false_after_all_ops_complete() {
      let mut s = SyncState::default();
      s.start(2);
      s.complete_op();
      s.complete_op();

      assert_eq!(s.is_syncing(), false);
    }
  }

  mod progress {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_zero_before_start() {
      let s = SyncState::default();

      assert_eq!(s.progress(), 0.0);
    }

    #[test]
    fn it_steps_by_the_inverse_of_total_per_op() {
      let mut s = SyncState::default();
      s.start(4);
      s.complete_op();

      assert_eq!(s.progress(), 0.25);
    }

    #[test]
    fn it_reaches_one_when_all_ops_are_done() {
      let mut s = SyncState::default();
      s.start(3);
      s.complete_op();
      s.complete_op();
      s.complete_op();

      assert_eq!(s.progress(), 1.0);
    }
  }

  mod record_background_sync {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_updates_the_sync_timestamp() {
      let mut s = SyncState::default();
      s.record_background_sync();

      assert_eq!(s.secs_since_sync(), 0);
    }
  }
}
