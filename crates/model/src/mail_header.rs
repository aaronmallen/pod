//! Domain model for mail headers.

use validator::Validate;

#[derive(Clone, Debug, Validate)]
pub struct Model {
  /// Cached plain-text mail body (joined paragraphs), or `None` if not yet
  /// fetched.
  pub body: Option<String>,
  /// EVE character ID of the owner.
  pub character_id: i64,
  /// ESI ID of the sender.
  pub from_id: Option<i64>,
  /// Whether the character has read this mail.
  pub is_read: bool,
  /// ESI mail ID.
  pub mail_id: i64,
  /// Short preview derived from the body (first ~250 chars at a word
  /// boundary), or `None` if the body has not been fetched yet.
  pub preview: Option<String>,
  /// Pre-formatted comma-separated recipient display names.
  pub recipients_display: String,
  /// Mail subject line.
  pub subject: String,
  /// ISO 8601 send timestamp.
  #[validate(length(min = 1))]
  pub timestamp: String,
}

#[cfg(test)]
mod tests {
  use validator::Validate;

  use super::*;

  fn make_header() -> Model {
    Model {
      body: None,
      character_id: 90_000_001,
      from_id: Some(90_000_002),
      is_read: false,
      mail_id: 12345,
      preview: None,
      recipients_display: "Test Pilot".into(),
      subject: "Hello capsuleer".into(),
      timestamp: "2024-06-01T12:00:00Z".into(),
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
