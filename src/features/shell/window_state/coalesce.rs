use std::time::{Duration, Instant};

use super::UiState;

pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(500);

#[derive(Clone, Debug)]
pub struct WriteCoalescer {
  debounce: Duration,
  pending: Option<Pending>,
}

impl WriteCoalescer {
  pub fn new() -> Self {
    Self::with_debounce(DEFAULT_DEBOUNCE)
  }

  pub fn with_debounce(debounce: Duration) -> Self {
    Self {
      debounce,
      pending: None,
    }
  }

  #[cfg(test)]
  pub fn has_pending(&self) -> bool {
    self.pending.is_some()
  }

  pub fn is_due(&self, now: Instant) -> bool {
    self
      .pending
      .as_ref()
      .is_some_and(|pending| now.saturating_duration_since(pending.requested_at) >= self.debounce)
  }

  pub fn request(&mut self, state: UiState, now: Instant) {
    self.pending = Some(Pending {
      requested_at: now,
      state,
    });
  }

  pub fn take(&mut self) -> Option<UiState> {
    self.pending.take().map(|pending| pending.state)
  }

  pub fn take_due(&mut self, now: Instant) -> Option<UiState> {
    if self.is_due(now) { self.take() } else { None }
  }
}

impl Default for WriteCoalescer {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Clone, Debug)]
struct Pending {
  requested_at: Instant,
  state: UiState,
}

#[cfg(test)]
mod tests {
  use std::collections::BTreeMap;

  use super::*;
  use crate::features::shell::window_state::WindowGeometry;

  fn geometry(width: f32) -> UiState {
    UiState {
      windows: BTreeMap::from([(
        "main".to_owned(),
        WindowGeometry {
          height: 800.0,
          width,
          x: 0.0,
          y: 0.0,
        },
      )]),
      ..UiState::default()
    }
  }

  mod is_due {
    use super::*;

    #[test]
    fn it_is_a_pure_check_that_does_not_consume_the_pending_state() {
      let mut coalescer = WriteCoalescer::with_debounce(Duration::from_millis(500));
      let start = Instant::now();

      coalescer.request(geometry(1000.0), start);
      let settled = start + Duration::from_millis(500);

      assert!(coalescer.is_due(settled));
      assert!(coalescer.is_due(settled));
      assert!(coalescer.has_pending());
    }
  }

  mod take {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_drains_the_pending_write_so_a_later_poll_does_not_repeat_it() {
      let mut coalescer = WriteCoalescer::with_debounce(Duration::from_millis(500));
      let start = Instant::now();

      coalescer.request(geometry(1000.0), start);
      assert_eq!(coalescer.take(), Some(geometry(1000.0)));

      assert_eq!(coalescer.take_due(start + Duration::from_millis(500)), None);
    }

    #[test]
    fn it_flushes_the_pending_state_immediately_for_a_settling_edge() {
      let mut coalescer = WriteCoalescer::with_debounce(Duration::from_millis(500));
      let start = Instant::now();

      coalescer.request(geometry(1000.0), start);

      assert_eq!(coalescer.take(), Some(geometry(1000.0)));
      assert!(!coalescer.has_pending());
    }

    #[test]
    fn it_returns_nothing_when_there_is_no_pending_state() {
      let mut coalescer = WriteCoalescer::with_debounce(Duration::from_millis(500));

      assert_eq!(coalescer.take(), None);
    }
  }

  mod take_due {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collapses_many_rapid_requests_within_the_window_into_a_single_write() {
      let mut coalescer = WriteCoalescer::with_debounce(Duration::from_millis(500));
      let start = Instant::now();

      for step in 0..20 {
        let now = start + Duration::from_millis(step * 50);
        coalescer.request(geometry(1000.0 + step as f32), now);
        assert_eq!(coalescer.take_due(now), None);
      }

      let last_request_at = start + Duration::from_millis(19 * 50);

      let first = coalescer.take_due(last_request_at + Duration::from_millis(500));
      assert_eq!(first, Some(geometry(1019.0)));

      let second = coalescer.take_due(last_request_at + Duration::from_millis(1000));
      assert_eq!(second, None);
    }

    #[test]
    fn it_commits_one_write_once_the_gesture_has_settled() {
      let mut coalescer = WriteCoalescer::with_debounce(Duration::from_millis(500));
      let start = Instant::now();

      coalescer.request(geometry(1000.0), start);

      assert_eq!(
        coalescer.take_due(start + Duration::from_millis(500)),
        Some(geometry(1000.0))
      );
    }

    #[test]
    fn it_measures_quiescence_from_the_latest_request_not_the_first() {
      let mut coalescer = WriteCoalescer::with_debounce(Duration::from_millis(500));
      let start = Instant::now();

      coalescer.request(geometry(1000.0), start);
      coalescer.request(geometry(1100.0), start + Duration::from_millis(400));

      assert_eq!(coalescer.take_due(start + Duration::from_millis(500)), None);
      assert_eq!(
        coalescer.take_due(start + Duration::from_millis(900)),
        Some(geometry(1100.0))
      );
    }

    #[test]
    fn it_returns_nothing_when_no_save_was_requested() {
      let mut coalescer = WriteCoalescer::with_debounce(Duration::from_millis(500));

      assert_eq!(coalescer.take_due(Instant::now()), None);
    }

    #[test]
    fn it_withholds_the_write_until_the_debounce_window_elapses() {
      let mut coalescer = WriteCoalescer::with_debounce(Duration::from_millis(500));
      let start = Instant::now();

      coalescer.request(geometry(1000.0), start);

      assert_eq!(coalescer.take_due(start + Duration::from_millis(499)), None);
      assert!(coalescer.has_pending());
    }
  }
}
