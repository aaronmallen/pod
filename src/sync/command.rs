use super::subject::Subject;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
  Discover,
  Drain,
  Enroll(Subject),
  RunNow(Subject),
  Withdraw(Subject),
}
