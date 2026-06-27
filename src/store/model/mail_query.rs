use crate::store::model::CharacterMail;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailCursor {
  pub mail_id: i64,
  pub timestamp: String,
}

impl MailCursor {
  pub fn new(timestamp: String, mail_id: i64) -> Self {
    Self {
      mail_id,
      timestamp,
    }
  }

  // Public store API exercised by unit tests; not yet wired into a production call site.
  #[allow(dead_code)]
  pub fn after(header: &CharacterMail) -> Self {
    Self {
      mail_id: header.mail_id(),
      timestamp: header.timestamp().clone(),
    }
  }
}
