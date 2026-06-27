// Stub: the preflight-logic task owns this module's contents. `Outcome` is the
// resolution the splash preflight will report; it has no consumer until that
// task wires the check in.
#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
  Failed(String),
  NoUpdate,
  UpdateAvailable { version: String },
}
