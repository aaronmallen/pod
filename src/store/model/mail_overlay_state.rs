use sqlx::FromRow;

#[derive(Clone, Debug, Default, Eq, FromRow, PartialEq)]
pub struct MailOverlayState {
  pub folder: Option<String>,
  pub is_pinned: bool,
  pub is_starred: bool,
  pub mail_id: i64,
  pub snooze_until: Option<String>,
}

impl MailOverlayState {
  pub fn is_snoozed(&self) -> bool {
    self.snooze_until.is_some()
  }
}
