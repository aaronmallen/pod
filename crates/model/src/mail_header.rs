//! Domain model for mail headers.

use validator::Validate;

#[derive(Clone, Debug, Validate)]
pub struct Model {
  pub character_id: i64,
  pub mail_id: i64,
  pub subject: String,
  pub from_id: Option<i64>,
  pub is_read: bool,
  #[validate(length(min = 1))]
  pub timestamp: String,
  pub recipients_display: String,
}

#[cfg(test)]
mod tests {
  use validator::Validate;

  use super::*;

  fn make_header() -> Model {
    Model {
      character_id: 90_000_001,
      mail_id: 12345,
      subject: "Hello capsuleer".into(),
      from_id: Some(90_000_002),
      is_read: false,
      timestamp: "2024-06-01T12:00:00Z".into(),
      recipients_display: "Test Pilot".into(),
    }
  }

  mod validate {
    use super::*;

    #[test]
    fn it_passes_for_valid_header() {
      let header = make_header();
      assert!(header.validate().is_ok());
    }

    #[test]
    fn it_fails_when_timestamp_is_empty() {
      let mut header = make_header();
      header.timestamp = String::new();
      assert!(header.validate().is_err());
    }
  }
}
