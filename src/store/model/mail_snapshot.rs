#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct MailSnapshot {
  pub body: Option<String>,
  pub character_id: i64,
  pub folder: Option<SnapshotFolder>,
  pub from_corp: bool,
  pub from_id: i64,
  pub from_name: String,
  pub from_system: bool,
  pub has_attachment: bool,
  pub important: bool,
  pub is_read: bool,
  pub label_ids: Vec<i64>,
  pub mail_id: i64,
  pub recipients: Vec<SnapshotRecipient>,
  pub snooze_until: Option<String>,
  pub subject: Option<String>,
  pub timestamp: String,
  pub triage: Option<SnapshotTriage>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SnapshotFolder {
  pub assigned_at: Option<String>,
  pub folder: String,
  pub remap_label_id: Option<i64>,
  pub soft_delete_intent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SnapshotRecipient {
  pub recipient_id: i64,
  pub recipient_name: String,
  pub recipient_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SnapshotTriage {
  pub star: bool,
}
