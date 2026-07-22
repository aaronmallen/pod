/// Staleness token for dropping out-of-date async results: bump via `next` on every (re)issue or
/// scope change, capture `current` when a request is dispatched, and discard the result when
/// `matches` later returns false.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LoadEpoch(u64);

impl LoadEpoch {
  pub fn current(self) -> u64 {
    self.0
  }

  pub fn matches(self, captured: u64) -> bool {
    self.0 == captured
  }

  pub fn next(&mut self) -> u64 {
    self.0 = self.0.wrapping_add(1);
    self.0
  }
}
