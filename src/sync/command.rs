use super::subject::Subject;
use crate::config::FeatureFlags;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
  Discover,
  Drain,
  Enroll(Subject),
  RunNow(Subject),
  SetFeatures(FeatureFlags),
  Shutdown,
  Withdraw(Subject),
}
