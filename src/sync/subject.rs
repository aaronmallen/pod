#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Subject {
  Character(i64),
  Corporation(i64),
}

impl Subject {
  pub fn id(self) -> i64 {
    match self {
      Self::Character(id) | Self::Corporation(id) => id,
    }
  }
}
