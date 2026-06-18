use super::subject::Subject;
use crate::config::FeatureFlags;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
  Discover,
  // Exercised only by unit tests / forward-looking sync surface; no production reader yet.
  #[allow(dead_code)]
  Drain,
  Enroll(Subject),
  RunNow(Subject),
  SetFeatures(FeatureFlags),
  Shutdown,
  Withdraw(Subject),
}
