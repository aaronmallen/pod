use chrono::Utc;
use sqlx::{QueryBuilder, Sqlite};

use crate::store::{
  Database, Error,
  model::{
    CharacterMail, CharacterMailBody, CharacterMailLabel, CharacterMailLabelMembership, CharacterMailRecipient,
    MailDraft, MailFolderAssignment, MailSnooze, MailTriage,
    character_mail_view::{MailRender, UnifiedMail},
    mail_overlay_state::MailOverlayState,
  },
};

/// The visible-header column list, aliased to `m` (the `character_mail` table).
const VISIBLE_HEADER_COLUMNS: &str = "SELECT m.character_id, m.from_id, m.from_name, m.is_read, m.has_attachment, \
  m.important, m.from_corp, m.from_system, m.mail_id, m.subject, m.timestamp FROM character_mail m ";

/// A keyset cursor into the visible-mail listing, ordered `(timestamp DESC, mail_id DESC)`.
///
/// Pagination seeks strictly past the last row of the previous page rather than
/// using `OFFSET`, so inserts/deletes between page loads cannot duplicate or skip
/// rows. Build one from the last row of a loaded page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailCursor {
  pub mail_id: i64,
  pub timestamp: String,
}

impl MailCursor {
  /// Cursor at the `(timestamp, mail_id)` position of an already-loaded row.
  ///
  /// The next page seeks strictly past this position, so pass the last row of the
  /// page just loaded.
  pub fn new(timestamp: String, mail_id: i64) -> Self {
    Self {
      mail_id,
      timestamp,
    }
  }

  /// Cursor pointing just before `header`'s position in the listing.
  // Public store API exercised by unit tests; not yet wired into a production call site.
  #[allow(dead_code)]
  pub fn after(header: &CharacterMail) -> Self {
    Self {
      mail_id: header.mail_id,
      timestamp: header.timestamp.clone(),
    }
  }
}

/// A complete capture of every local row a single mail owns, taken before a permanent delete so the
/// outbox SAGA can restore it byte-for-byte if the ESI delete permanently fails.
///
/// It rides inside the `mail.delete` outbox payload, so the fields are plain serializable values
/// rather than the `FromRow` model structs. `(character_id, mail_id)` identifies the mail; the
/// dependent rows reference it implicitly.
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
  /// `None` for rows written before this column existed; `restore_mail` re-inserts it so a
  /// restored mail retains its original trash age and remains eligible for auto-purge.
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

pub async fn upsert_complete(
  db: &Database,
  header: &CharacterMail,
  body: &CharacterMailBody,
  recipients: &[CharacterMailRecipient],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query(
    "INSERT INTO character_mail \
      (character_id, mail_id, from_id, from_name, subject, timestamp, is_read, \
      has_attachment, important, from_corp, from_system) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT (character_id, mail_id) DO UPDATE SET \
      from_id = excluded.from_id, from_name = excluded.from_name, subject = excluded.subject, \
      timestamp = excluded.timestamp, is_read = excluded.is_read, \
      has_attachment = excluded.has_attachment, important = excluded.important, \
      from_corp = excluded.from_corp, from_system = excluded.from_system",
  )
  .bind(header.character_id())
  .bind(header.mail_id())
  .bind(header.from_id())
  .bind(header.from_name())
  .bind(header.subject())
  .bind(header.timestamp())
  .bind(header.is_read())
  .bind(header.has_attachment())
  .bind(header.important())
  .bind(header.from_corp())
  .bind(header.from_system())
  .execute(&mut *tx)
  .await?;

  sqlx::query("INSERT INTO character_mail_body (character_id, mail_id, body) VALUES (?, ?, ?) ON CONFLICT DO NOTHING")
    .bind(body.character_id())
    .bind(body.mail_id())
    .bind(body.body())
    .execute(&mut *tx)
    .await?;

  sqlx::query("DELETE FROM character_mail_recipients WHERE character_id = ? AND mail_id = ?")
    .bind(header.character_id())
    .bind(header.mail_id())
    .execute(&mut *tx)
    .await?;

  for recipient in recipients {
    sqlx::query(
      "INSERT INTO character_mail_recipients \
        (character_id, mail_id, recipient_id, recipient_type, recipient_name) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(recipient.character_id())
    .bind(recipient.mail_id())
    .bind(recipient.recipient_id())
    .bind(recipient.recipient_type())
    .bind(recipient.recipient_name())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn set_read(db: &Database, character_id: i64, mail_id: i64, is_read: bool) -> Result<(), Error> {
  sqlx::query("UPDATE character_mail SET is_read = ? WHERE character_id = ? AND mail_id = ?")
    .bind(is_read)
    .bind(character_id)
    .bind(mail_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn headers(db: &Database, character_id: i64) -> Result<Vec<CharacterMail>, Error> {
  let rows = sqlx::query_as::<_, CharacterMail>(
    "SELECT character_id, from_id, from_name, is_read, has_attachment, important, from_corp, from_system, \
      mail_id, subject, timestamp FROM character_mail \
    WHERE character_id = ? ORDER BY timestamp DESC, mail_id DESC",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn headers_for_label(db: &Database, character_id: i64, label_id: i64) -> Result<Vec<CharacterMail>, Error> {
  let rows = sqlx::query_as::<_, CharacterMail>(
    "SELECT m.character_id, m.from_id, m.from_name, m.is_read, m.has_attachment, m.important, \
      m.from_corp, m.from_system, m.mail_id, m.subject, m.timestamp \
    FROM character_mail m \
    JOIN character_mail_label_membership mem ON mem.character_id = m.character_id AND mem.mail_id = m.mail_id \
    WHERE m.character_id = ? AND mem.label_id = ? ORDER BY m.timestamp DESC, m.mail_id DESC",
  )
  .bind(character_id)
  .bind(label_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn body(db: &Database, character_id: i64, mail_id: i64) -> Result<Option<CharacterMailBody>, Error> {
  let row = sqlx::query_as::<_, CharacterMailBody>(
    "SELECT body, character_id, mail_id FROM character_mail_body WHERE character_id = ? AND mail_id = ?",
  )
  .bind(character_id)
  .bind(mail_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn has_body(db: &Database, character_id: i64, mail_id: i64) -> Result<bool, Error> {
  let exists =
    sqlx::query_scalar::<_, i64>("SELECT 1 FROM character_mail_body WHERE character_id = ? AND mail_id = ? LIMIT 1")
      .bind(character_id)
      .bind(mail_id)
      .fetch_optional(&db.0)
      .await?
      .is_some();
  Ok(exists)
}

pub async fn recipients(db: &Database, character_id: i64, mail_id: i64) -> Result<Vec<CharacterMailRecipient>, Error> {
  let rows = sqlx::query_as::<_, CharacterMailRecipient>(
    "SELECT character_id, mail_id, recipient_id, recipient_name, recipient_type FROM character_mail_recipients \
    WHERE character_id = ? AND mail_id = ? ORDER BY recipient_type, recipient_id",
  )
  .bind(character_id)
  .bind(mail_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn recipients_display(db: &Database, character_id: i64, mail_id: i64) -> Result<String, Error> {
  let names = sqlx::query_scalar::<_, String>(
    "SELECT recipient_name FROM character_mail_recipients \
    WHERE character_id = ? AND mail_id = ? ORDER BY recipient_type, recipient_id",
  )
  .bind(character_id)
  .bind(mail_id)
  .fetch_all(&db.0)
  .await?;
  Ok(names.join(", "))
}

pub async fn replace_labels_for_character(
  db: &Database,
  character_id: i64,
  labels: &[CharacterMailLabel],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  // label_id < 0 are optimistic rows for pending create-label outbox ops; preserve them across server replaces.
  sqlx::query("DELETE FROM character_mail_labels WHERE character_id = ? AND label_id >= 0")
    .bind(character_id)
    .execute(&mut *tx)
    .await?;

  for label in labels {
    sqlx::query("INSERT INTO character_mail_labels (character_id, label_id, name, color) VALUES (?, ?, ?, ?)")
      .bind(label.character_id())
      .bind(label.label_id())
      .bind(label.name())
      .bind(label.color())
      .execute(&mut *tx)
      .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn labels(db: &Database, character_id: i64) -> Result<Vec<CharacterMailLabel>, Error> {
  let rows = sqlx::query_as::<_, CharacterMailLabel>(
    "SELECT character_id, color, label_id, name FROM character_mail_labels WHERE character_id = ? ORDER BY label_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn replace_membership_for_character(
  db: &Database,
  character_id: i64,
  membership: &[CharacterMailLabelMembership],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  // label_id < 0 are optimistic rows for pending create-label outbox ops; preserve them across server replaces.
  sqlx::query("DELETE FROM character_mail_label_membership WHERE character_id = ? AND label_id >= 0")
    .bind(character_id)
    .execute(&mut *tx)
    .await?;

  for entry in membership {
    sqlx::query("INSERT INTO character_mail_label_membership (character_id, mail_id, label_id) VALUES (?, ?, ?)")
      .bind(entry.character_id())
      .bind(entry.mail_id())
      .bind(entry.label_id())
      .execute(&mut *tx)
      .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn membership(db: &Database, character_id: i64, mail_id: i64) -> Result<Vec<i64>, Error> {
  let rows = sqlx::query_scalar::<_, i64>(
    "SELECT label_id FROM character_mail_label_membership WHERE character_id = ? AND mail_id = ? ORDER BY label_id",
  )
  .bind(character_id)
  .bind(mail_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn insert_label(db: &Database, label: &CharacterMailLabel) -> Result<(), Error> {
  sqlx::query("INSERT INTO character_mail_labels (character_id, label_id, name, color) VALUES (?, ?, ?, ?)")
    .bind(label.character_id())
    .bind(label.label_id())
    .bind(label.name())
    .bind(label.color())
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn remap_label_id(
  db: &Database,
  character_id: i64,
  from_label_id: i64,
  to_label_id: i64,
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  // Move the label PK before the membership FK so the membership rows can follow it without
  // tripping the (character_id, label_id) foreign key mid-transaction.
  sqlx::query("PRAGMA defer_foreign_keys = ON").execute(&mut *tx).await?;

  sqlx::query("UPDATE character_mail_labels SET label_id = ? WHERE character_id = ? AND label_id = ?")
    .bind(to_label_id)
    .bind(character_id)
    .bind(from_label_id)
    .execute(&mut *tx)
    .await?;

  sqlx::query("UPDATE character_mail_label_membership SET label_id = ? WHERE character_id = ? AND label_id = ?")
    .bind(to_label_id)
    .bind(character_id)
    .bind(from_label_id)
    .execute(&mut *tx)
    .await?;

  tx.commit().await?;
  Ok(())
}

pub async fn add_membership(db: &Database, character_id: i64, mail_id: i64, label_id: i64) -> Result<(), Error> {
  sqlx::query(
    "INSERT OR IGNORE INTO character_mail_label_membership (character_id, mail_id, label_id) VALUES (?, ?, ?)",
  )
  .bind(character_id)
  .bind(mail_id)
  .bind(label_id)
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn remove_membership(db: &Database, character_id: i64, mail_id: i64, label_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM character_mail_label_membership WHERE character_id = ? AND mail_id = ? AND label_id = ?")
    .bind(character_id)
    .bind(mail_id)
    .bind(label_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn delete_label(db: &Database, character_id: i64, label_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM character_mail_labels WHERE character_id = ? AND label_id = ?")
    .bind(character_id)
    .bind(label_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn unread_count_for_label(db: &Database, character_id: i64, label_id: i64) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM character_mail_label_membership mem \
      JOIN character_mail m ON m.character_id = mem.character_id AND m.mail_id = mem.mail_id \
    WHERE mem.character_id = ? AND mem.label_id = ? AND m.is_read = 0 AND m.from_id != m.character_id",
  )
  .bind(character_id)
  .bind(label_id)
  .fetch_one(&db.0)
  .await?;
  Ok(count)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn unread_counts_by_label(db: &Database, character_id: i64) -> Result<Vec<(i64, i64)>, Error> {
  let rows = sqlx::query_as::<_, (i64, i64)>(
    "SELECT l.label_id, \
      COUNT(m.mail_id) FILTER (WHERE m.is_read = 0 AND m.from_id != m.character_id) AS unread \
    FROM character_mail_labels l \
    LEFT JOIN character_mail_label_membership mem \
      ON mem.character_id = l.character_id AND mem.label_id = l.label_id \
    LEFT JOIN character_mail m ON m.character_id = mem.character_id AND m.mail_id = mem.mail_id \
    WHERE l.character_id = ? \
    GROUP BY l.label_id ORDER BY l.label_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn unread_count(db: &Database, character_id: i64) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM character_mail \
    WHERE character_id = ? AND is_read = 0 AND from_id != character_id",
  )
  .bind(character_id)
  .fetch_one(&db.0)
  .await?;
  Ok(count)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn unified_unread_count(db: &Database) -> Result<i64, Error> {
  let count =
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mail_unified WHERE is_read = 0 AND from_id != character_id")
      .fetch_one(&db.0)
      .await?;
  Ok(count)
}

pub async fn unified(db: &Database) -> Result<Vec<UnifiedMail>, Error> {
  let rows = sqlx::query_as::<_, UnifiedMail>(
    "SELECT character_id, mail_id, from_id, from_name, subject, timestamp, is_read, \
      has_attachment, important, from_corp, from_system, body FROM mail_unified \
    WHERE from_id != character_id \
    ORDER BY timestamp DESC, mail_id DESC",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn mail(db: &Database, character_id: i64, mail_id: i64) -> Result<Option<MailRender>, Error> {
  let Some(body) = body(db, character_id, mail_id).await? else {
    return Ok(None);
  };
  let header = sqlx::query_as::<_, CharacterMail>(
    "SELECT character_id, from_id, from_name, is_read, has_attachment, important, from_corp, from_system, \
      mail_id, subject, timestamp FROM character_mail \
    WHERE character_id = ? AND mail_id = ?",
  )
  .bind(character_id)
  .bind(mail_id)
  .fetch_optional(&db.0)
  .await?;
  let Some(header) = header else {
    return Ok(None);
  };

  let recipients = recipients(db, character_id, mail_id).await?;
  let recipients_display = recipients
    .iter()
    .map(|r| r.recipient_name().as_str())
    .collect::<Vec<_>>()
    .join(", ");
  let label_ids = membership(db, character_id, mail_id).await?;

  Ok(Some(MailRender {
    header,
    body,
    recipients,
    recipients_display,
    label_ids,
  }))
}

pub async fn set_triage(db: &Database, character_id: i64, mail_id: i64, star: bool) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO mail_triage (character_id, mail_id, star) VALUES (?, ?, ?) \
    ON CONFLICT(character_id, mail_id) DO UPDATE SET star = excluded.star",
  )
  .bind(character_id)
  .bind(mail_id)
  .bind(star)
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn clear_triage(db: &Database, character_id: i64, mail_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM mail_triage WHERE character_id = ? AND mail_id = ?")
    .bind(character_id)
    .bind(mail_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn triage(db: &Database, character_id: i64, mail_id: i64) -> Result<Option<MailTriage>, Error> {
  let row = sqlx::query_as::<_, MailTriage>(
    "SELECT character_id, id, mail_id, star FROM mail_triage WHERE character_id = ? AND mail_id = ?",
  )
  .bind(character_id)
  .bind(mail_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn all_triage(db: &Database, character_id: i64) -> Result<Vec<MailTriage>, Error> {
  let rows = sqlx::query_as::<_, MailTriage>(
    "SELECT character_id, id, mail_id, star FROM mail_triage WHERE character_id = ? ORDER BY mail_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn upsert_snoozed_mail(
  db: &Database,
  character_id: i64,
  mail_id: i64,
  snooze_until: &str,
) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO mail_snooze (character_id, mail_id, snooze_until) VALUES (?, ?, ?) \
    ON CONFLICT(character_id, mail_id) DO UPDATE SET snooze_until = excluded.snooze_until",
  )
  .bind(character_id)
  .bind(mail_id)
  .bind(snooze_until)
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn delete_snoozed_mail(db: &Database, character_id: i64, mail_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM mail_snooze WHERE character_id = ? AND mail_id = ?")
    .bind(character_id)
    .bind(mail_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn all_snoozed_mails(db: &Database, character_id: i64) -> Result<Vec<MailSnooze>, Error> {
  let rows = sqlx::query_as::<_, MailSnooze>(
    "SELECT character_id, id, mail_id, snooze_until FROM mail_snooze WHERE character_id = ? ORDER BY snooze_until",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn expired_snoozed_mails(db: &Database, now: &str) -> Result<Vec<MailSnooze>, Error> {
  let rows = sqlx::query_as::<_, MailSnooze>(
    "SELECT character_id, id, mail_id, snooze_until FROM mail_snooze WHERE snooze_until <= ? ORDER BY snooze_until",
  )
  .bind(now)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn assign_folder(
  db: &Database,
  character_id: i64,
  mail_id: i64,
  folder: &str,
  remap_label_id: Option<i64>,
  soft_delete_intent: bool,
  assigned_at: &str,
) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO mail_folder_assignment (character_id, mail_id, folder, remap_label_id, soft_delete_intent, assigned_at) \
      VALUES (?, ?, ?, ?, ?, ?) \
    ON CONFLICT(character_id, mail_id) DO UPDATE SET \
      folder = excluded.folder, remap_label_id = excluded.remap_label_id, \
      soft_delete_intent = excluded.soft_delete_intent, assigned_at = excluded.assigned_at",
  )
  .bind(character_id)
  .bind(mail_id)
  .bind(folder)
  .bind(remap_label_id)
  .bind(soft_delete_intent)
  .bind(assigned_at)
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn clear_folder(db: &Database, character_id: i64, mail_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM mail_folder_assignment WHERE character_id = ? AND mail_id = ?")
    .bind(character_id)
    .bind(mail_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn folder(db: &Database, character_id: i64, mail_id: i64) -> Result<Option<MailFolderAssignment>, Error> {
  let row = sqlx::query_as::<_, MailFolderAssignment>(
    "SELECT assigned_at, character_id, folder, id, mail_id, remap_label_id, soft_delete_intent FROM mail_folder_assignment \
    WHERE character_id = ? AND mail_id = ?",
  )
  .bind(character_id)
  .bind(mail_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn all_in_folder(db: &Database, character_id: i64, folder: &str) -> Result<Vec<MailFolderAssignment>, Error> {
  let rows = sqlx::query_as::<_, MailFolderAssignment>(
    "SELECT assigned_at, character_id, folder, id, mail_id, remap_label_id, soft_delete_intent FROM mail_folder_assignment \
    WHERE character_id = ? AND folder = ? ORDER BY mail_id",
  )
  .bind(character_id)
  .bind(folder)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn expired_trashed_mails(db: &Database, cutoff: &str) -> Result<Vec<MailFolderAssignment>, Error> {
  let rows = sqlx::query_as::<_, MailFolderAssignment>(
    // NULL stamp (row pre-dates this column) is intentionally skipped — not treated as infinitely old.
    "SELECT assigned_at, character_id, folder, id, mail_id, remap_label_id, soft_delete_intent FROM mail_folder_assignment \
    WHERE folder = 'trash' AND assigned_at IS NOT NULL AND assigned_at <= ? ORDER BY assigned_at",
  )
  .bind(cutoff)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DraftInput {
  pub body: String,
  pub character_id: i64,
  pub kind: String,
  pub quote: Option<String>,
  pub recipients_cc: String,
  pub recipients_to: String,
  pub subject: String,
}

/// Persists a draft and returns its row id. `None` inserts a fresh row (preserving `created_at`); `Some(id)` updates
/// that row in place so re-saving an open draft never duplicates it.
pub async fn upsert_draft(db: &Database, id: Option<i64>, input: &DraftInput) -> Result<i64, Error> {
  let now = Utc::now().to_rfc3339();
  let id = match id {
    Some(id) => {
      sqlx::query(
        "UPDATE mail_drafts SET subject = ?, body = ?, recipients_to = ?, recipients_cc = ?, kind = ?, quote = ?, \
        updated_at = ? WHERE id = ?",
      )
      .bind(&input.subject)
      .bind(&input.body)
      .bind(&input.recipients_to)
      .bind(&input.recipients_cc)
      .bind(&input.kind)
      .bind(&input.quote)
      .bind(&now)
      .bind(id)
      .execute(db.writer())
      .await?;
      id
    }
    None => {
      sqlx::query_scalar::<_, i64>(
        "INSERT INTO mail_drafts \
          (character_id, subject, body, recipients_to, recipients_cc, kind, quote, created_at, updated_at) \
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
      )
      .bind(input.character_id)
      .bind(&input.subject)
      .bind(&input.body)
      .bind(&input.recipients_to)
      .bind(&input.recipients_cc)
      .bind(&input.kind)
      .bind(&input.quote)
      .bind(&now)
      .bind(&now)
      .fetch_one(&db.0)
      .await?
    }
  };
  Ok(id)
}

pub async fn draft(db: &Database, id: i64) -> Result<Option<MailDraft>, Error> {
  let row = sqlx::query_as::<_, MailDraft>(
    "SELECT body, character_id, created_at, id, kind, quote, recipients_cc, recipients_to, subject, updated_at \
    FROM mail_drafts WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn list_drafts_for_character(db: &Database, character_id: i64) -> Result<Vec<MailDraft>, Error> {
  let rows = sqlx::query_as::<_, MailDraft>(
    "SELECT body, character_id, created_at, id, kind, quote, recipients_cc, recipients_to, subject, updated_at \
    FROM mail_drafts WHERE character_id = ? ORDER BY updated_at DESC, id DESC",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn count_drafts_for_character(db: &Database, character_id: i64) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mail_drafts WHERE character_id = ?")
    .bind(character_id)
    .fetch_one(&db.0)
    .await?;
  Ok(count)
}

pub async fn delete_draft(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM mail_drafts WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn overlay_state(db: &Database, character_id: i64, mail_id: i64) -> Result<MailOverlayState, Error> {
  let row = sqlx::query_as::<_, MailOverlayState>(
    "SELECT k.mail_id AS mail_id, COALESCE(t.star, 0) AS is_starred, \
      s.snooze_until AS snooze_until, f.folder AS folder \
    FROM (SELECT ? AS character_id, ? AS mail_id) k \
    LEFT JOIN mail_triage t ON t.character_id = k.character_id AND t.mail_id = k.mail_id \
    LEFT JOIN mail_snooze s ON s.character_id = k.character_id AND s.mail_id = k.mail_id \
    LEFT JOIN mail_folder_assignment f ON f.character_id = k.character_id AND f.mail_id = k.mail_id",
  )
  .bind(character_id)
  .bind(mail_id)
  .fetch_one(&db.0)
  .await?;
  Ok(row)
}

/// Captures every local row a mail owns so a permanent delete can be undone. Returns `None` when
/// the mail header is gone (nothing to delete).
pub async fn snapshot_mail(db: &Database, character_id: i64, mail_id: i64) -> Result<Option<MailSnapshot>, Error> {
  let Some(header) = sqlx::query_as::<_, CharacterMail>(
    "SELECT character_id, from_corp, from_id, from_name, from_system, has_attachment, important, is_read, mail_id, \
      subject, timestamp FROM character_mail WHERE character_id = ? AND mail_id = ?",
  )
  .bind(character_id)
  .bind(mail_id)
  .fetch_optional(&db.0)
  .await?
  else {
    return Ok(None);
  };

  let body = body(db, character_id, mail_id).await?.map(|row| row.body().clone());
  let recipients = recipients(db, character_id, mail_id)
    .await?
    .into_iter()
    .map(|row| SnapshotRecipient {
      recipient_id: row.recipient_id(),
      recipient_name: row.recipient_name().clone(),
      recipient_type: row.recipient_type().clone(),
    })
    .collect();
  let label_ids = membership(db, character_id, mail_id).await?;
  let triage = triage(db, character_id, mail_id).await?.map(|row| SnapshotTriage {
    star: row.star(),
  });
  let snooze_until =
    sqlx::query_scalar::<_, String>("SELECT snooze_until FROM mail_snooze WHERE character_id = ? AND mail_id = ?")
      .bind(character_id)
      .bind(mail_id)
      .fetch_optional(&db.0)
      .await?;
  let folder = folder(db, character_id, mail_id).await?.map(|row| SnapshotFolder {
    assigned_at: row.assigned_at().clone(),
    folder: row.folder().clone(),
    remap_label_id: row.remap_label_id(),
    soft_delete_intent: row.soft_delete_intent(),
  });

  Ok(Some(MailSnapshot {
    body,
    character_id: header.character_id(),
    folder,
    from_corp: header.from_corp(),
    from_id: header.from_id(),
    from_name: header.from_name().clone(),
    from_system: header.from_system(),
    has_attachment: header.has_attachment(),
    important: header.important(),
    is_read: header.is_read(),
    label_ids,
    mail_id: header.mail_id(),
    recipients,
    snooze_until,
    subject: header.subject().clone(),
    timestamp: header.timestamp().clone(),
    triage,
  }))
}

/// Permanently removes every local row a mail owns across all dependent tables in one transaction,
/// leaving no orphans.
pub async fn purge_mail(db: &Database, character_id: i64, mail_id: i64) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  for statement in [
    "DELETE FROM character_mail_label_membership WHERE character_id = ? AND mail_id = ?",
    "DELETE FROM mail_folder_assignment WHERE character_id = ? AND mail_id = ?",
    "DELETE FROM mail_snooze WHERE character_id = ? AND mail_id = ?",
    "DELETE FROM mail_triage WHERE character_id = ? AND mail_id = ?",
    "DELETE FROM character_mail_recipients WHERE character_id = ? AND mail_id = ?",
    "DELETE FROM character_mail_body WHERE character_id = ? AND mail_id = ?",
    "DELETE FROM character_mail WHERE character_id = ? AND mail_id = ?",
  ] {
    sqlx::query(statement)
      .bind(character_id)
      .bind(mail_id)
      .execute(&mut *tx)
      .await?;
  }

  tx.commit().await?;
  Ok(())
}

/// Purges the optimistic Sent-folder placeholders a character holds — self-sent rows whose
/// `mail_id` is negative. The composer writes these so a sent mail shows in Sent immediately; mail
/// sync upserts the real sent mail under ESI's positive id and never deletes stale rows, so this
/// sweep (run by the mail sync job) reconciles the placeholder away once the real mail has landed.
pub async fn purge_synthetic_sent(db: &Database, character_id: i64) -> Result<(), Error> {
  let ids = sqlx::query_scalar::<_, i64>(
    "SELECT mail_id FROM character_mail WHERE character_id = ? AND from_id = ? AND mail_id < 0",
  )
  .bind(character_id)
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;

  for mail_id in ids {
    purge_mail(db, character_id, mail_id).await?;
  }
  Ok(())
}

/// Re-inserts every row captured by [`snapshot_mail`], reversing a [`purge_mail`] in one
/// transaction. Used to compensate a permanently failed ESI delete.
pub async fn restore_mail(db: &Database, snapshot: &MailSnapshot) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query(
    "INSERT INTO character_mail \
      (character_id, mail_id, from_id, from_name, subject, timestamp, is_read, \
      has_attachment, important, from_corp, from_system) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT (character_id, mail_id) DO NOTHING",
  )
  .bind(snapshot.character_id)
  .bind(snapshot.mail_id)
  .bind(snapshot.from_id)
  .bind(&snapshot.from_name)
  .bind(&snapshot.subject)
  .bind(&snapshot.timestamp)
  .bind(snapshot.is_read)
  .bind(snapshot.has_attachment)
  .bind(snapshot.important)
  .bind(snapshot.from_corp)
  .bind(snapshot.from_system)
  .execute(&mut *tx)
  .await?;

  if let Some(body) = &snapshot.body {
    sqlx::query(
      "INSERT INTO character_mail_body (character_id, mail_id, body) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
    )
    .bind(snapshot.character_id)
    .bind(snapshot.mail_id)
    .bind(body)
    .execute(&mut *tx)
    .await?;
  }

  for recipient in &snapshot.recipients {
    sqlx::query(
      "INSERT INTO character_mail_recipients \
        (character_id, mail_id, recipient_id, recipient_type, recipient_name) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(snapshot.character_id)
    .bind(snapshot.mail_id)
    .bind(recipient.recipient_id)
    .bind(&recipient.recipient_type)
    .bind(&recipient.recipient_name)
    .execute(&mut *tx)
    .await?;
  }

  if let Some(triage) = &snapshot.triage {
    sqlx::query("INSERT INTO mail_triage (character_id, mail_id, star) VALUES (?, ?, ?)")
      .bind(snapshot.character_id)
      .bind(snapshot.mail_id)
      .bind(triage.star)
      .execute(&mut *tx)
      .await?;
  }

  if let Some(snooze_until) = &snapshot.snooze_until {
    sqlx::query("INSERT INTO mail_snooze (character_id, mail_id, snooze_until) VALUES (?, ?, ?)")
      .bind(snapshot.character_id)
      .bind(snapshot.mail_id)
      .bind(snooze_until)
      .execute(&mut *tx)
      .await?;
  }

  if let Some(folder) = &snapshot.folder {
    sqlx::query(
      "INSERT INTO mail_folder_assignment (character_id, mail_id, folder, remap_label_id, soft_delete_intent, assigned_at) \
        VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(snapshot.character_id)
    .bind(snapshot.mail_id)
    .bind(&folder.folder)
    .bind(folder.remap_label_id)
    .bind(folder.soft_delete_intent)
    .bind(&folder.assigned_at)
    .execute(&mut *tx)
    .await?;
  }

  for label_id in &snapshot.label_ids {
    sqlx::query(
      "INSERT OR IGNORE INTO character_mail_label_membership (character_id, mail_id, label_id) VALUES (?, ?, ?)",
    )
    .bind(snapshot.character_id)
    .bind(snapshot.mail_id)
    .bind(label_id)
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn all_overlay_states(db: &Database, character_id: i64) -> Result<Vec<MailOverlayState>, Error> {
  let rows = sqlx::query_as::<_, MailOverlayState>(
    "SELECT k.mail_id AS mail_id, COALESCE(t.star, 0) AS is_starred, \
      s.snooze_until AS snooze_until, f.folder AS folder \
    FROM ( \
      SELECT character_id, mail_id FROM mail_triage WHERE character_id = ? \
      UNION SELECT character_id, mail_id FROM mail_snooze WHERE character_id = ? \
      UNION SELECT character_id, mail_id FROM mail_folder_assignment WHERE character_id = ? \
    ) k \
    LEFT JOIN mail_triage t ON t.character_id = k.character_id AND t.mail_id = k.mail_id \
    LEFT JOIN mail_snooze s ON s.character_id = k.character_id AND s.mail_id = k.mail_id \
    LEFT JOIN mail_folder_assignment f ON f.character_id = k.character_id AND f.mail_id = k.mail_id \
    ORDER BY k.mail_id",
  )
  .bind(character_id)
  .bind(character_id)
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn starred_mail_ids(db: &Database, character_id: i64) -> Result<Vec<i64>, Error> {
  let rows = sqlx::query_scalar::<_, i64>(
    "SELECT mail_id FROM mail_triage WHERE character_id = ? AND star = 1 ORDER BY mail_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn folder_mail_ids(db: &Database, character_id: i64, folder: &str) -> Result<Vec<i64>, Error> {
  let rows = sqlx::query_scalar::<_, i64>(
    "SELECT mail_id FROM mail_folder_assignment WHERE character_id = ? AND folder = ? ORDER BY mail_id",
  )
  .bind(character_id)
  .bind(folder)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn visible_headers(db: &Database, character_id: i64, now: &str) -> Result<Vec<CharacterMail>, Error> {
  let rows = sqlx::query_as::<_, CharacterMail>(
    "SELECT m.character_id, m.from_id, m.from_name, m.is_read, m.has_attachment, m.important, \
      m.from_corp, m.from_system, m.mail_id, m.subject, m.timestamp \
    FROM character_mail m \
    WHERE m.character_id = ? \
    AND NOT EXISTS ( \
      SELECT 1 FROM mail_folder_assignment fa \
      WHERE fa.character_id = m.character_id AND fa.mail_id = m.mail_id \
    ) AND NOT EXISTS ( \
      SELECT 1 FROM mail_snooze sn \
      WHERE sn.character_id = m.character_id AND sn.mail_id = m.mail_id AND sn.snooze_until > ? \
    ) \
    ORDER BY m.timestamp DESC, m.mail_id DESC",
  )
  .bind(character_id)
  .bind(now)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn visible_headers_for_label(
  db: &Database,
  character_id: i64,
  label_id: i64,
  now: &str,
) -> Result<Vec<CharacterMail>, Error> {
  let rows = sqlx::query_as::<_, CharacterMail>(
    "SELECT m.character_id, m.from_id, m.from_name, m.is_read, m.has_attachment, m.important, \
      m.from_corp, m.from_system, m.mail_id, m.subject, m.timestamp \
    FROM character_mail m \
    JOIN character_mail_label_membership mem ON mem.character_id = m.character_id AND mem.mail_id = m.mail_id \
    WHERE m.character_id = ? AND mem.label_id = ? \
    AND NOT EXISTS ( \
      SELECT 1 FROM mail_folder_assignment fa \
      WHERE fa.character_id = m.character_id AND fa.mail_id = m.mail_id \
    ) AND NOT EXISTS ( \
      SELECT 1 FROM mail_snooze sn \
      WHERE sn.character_id = m.character_id AND sn.mail_id = m.mail_id AND sn.snooze_until > ? \
    ) \
    ORDER BY m.timestamp DESC, m.mail_id DESC",
  )
  .bind(character_id)
  .bind(label_id)
  .bind(now)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

/// One bounded page of the inbox listing (visible), newest first.
///
/// This is the keyset-paginated replacement for the unbounded [`visible_headers`]
/// fetch: it seeks past `cursor` when one is supplied.
pub async fn visible_headers_page(
  db: &Database,
  character_id: i64,
  now: &str,
  cursor: Option<&MailCursor>,
  limit: i64,
) -> Result<Vec<CharacterMail>, Error> {
  let mut builder = QueryBuilder::<Sqlite>::new(VISIBLE_HEADER_COLUMNS);
  builder.push("WHERE m.character_id = ").push_bind(character_id);
  push_visible_tail_predicate(&mut builder, now);
  push_keyset_seek(&mut builder, cursor);
  push_order_and_limit(&mut builder, limit);
  let rows = builder.build_query_as::<CharacterMail>().fetch_all(&db.0).await?;
  Ok(rows)
}

/// One bounded page of a label folder's listing (visible), newest first.
pub async fn visible_headers_for_label_page(
  db: &Database,
  character_id: i64,
  label_id: i64,
  now: &str,
  cursor: Option<&MailCursor>,
  limit: i64,
) -> Result<Vec<CharacterMail>, Error> {
  let mut builder = QueryBuilder::<Sqlite>::new(VISIBLE_HEADER_COLUMNS);
  push_label_join(&mut builder);
  builder.push("WHERE m.character_id = ").push_bind(character_id);
  builder.push(" AND mem.label_id = ").push_bind(label_id);
  push_visible_tail_predicate(&mut builder, now);
  push_keyset_seek(&mut builder, cursor);
  push_order_and_limit(&mut builder, limit);
  let rows = builder.build_query_as::<CharacterMail>().fetch_all(&db.0).await?;
  Ok(rows)
}

/// One bounded page of visible mail whose subject or sender matches `needle`.
///
/// The bounded replacement for scanning the whole mailbox in memory on every
/// keystroke. `needle` is matched case-insensitively as a substring of the subject
/// or sender; `label_id` scopes the search to a label folder.
pub async fn search_visible_headers_page(
  db: &Database,
  character_id: i64,
  now: &str,
  needle: &str,
  label_id: Option<i64>,
  cursor: Option<&MailCursor>,
  limit: i64,
) -> Result<Vec<CharacterMail>, Error> {
  let pattern = format!("%{}%", escape_like(needle));
  let mut builder = QueryBuilder::<Sqlite>::new(VISIBLE_HEADER_COLUMNS);
  if label_id.is_some() {
    push_label_join(&mut builder);
  }
  builder.push("WHERE m.character_id = ").push_bind(character_id);
  if let Some(label_id) = label_id {
    builder.push(" AND mem.label_id = ").push_bind(label_id);
  }
  push_visible_tail_predicate(&mut builder, now);
  builder.push(" AND (m.subject LIKE ");
  builder.push_bind(pattern.clone());
  builder.push(" ESCAPE '\\' OR m.from_name LIKE ");
  builder.push_bind(pattern);
  builder.push(" ESCAPE '\\')");
  push_keyset_seek(&mut builder, cursor);
  push_order_and_limit(&mut builder, limit);
  let rows = builder.build_query_as::<CharacterMail>().fetch_all(&db.0).await?;
  Ok(rows)
}

/// One bounded page of unified (roster-wide) mail matching `needle`, newest first.
///
/// The unified-folder counterpart of [`search_visible_headers_page`]: it searches
/// the cross-character `mail_unified` view so the default folder's search still
/// spans the whole roster, keyset-paginated and excluding filed/snoozed mail.
pub async fn search_visible_unified_page(
  db: &Database,
  now: &str,
  needle: &str,
  cursor: Option<&MailCursor>,
  limit: i64,
) -> Result<Vec<UnifiedMail>, Error> {
  let pattern = format!("%{}%", escape_like(needle));
  let mut builder = QueryBuilder::<Sqlite>::new(
    "SELECT m.character_id, m.mail_id, m.from_id, m.from_name, m.subject, m.timestamp, m.is_read, \
      m.has_attachment, m.important, m.from_corp, m.from_system, m.body FROM mail_unified m \
      WHERE m.from_id != m.character_id",
  );
  push_visible_tail_predicate(&mut builder, now);
  builder.push(" AND (m.subject LIKE ");
  builder.push_bind(pattern.clone());
  builder.push(" ESCAPE '\\' OR m.from_name LIKE ");
  builder.push_bind(pattern);
  builder.push(" ESCAPE '\\')");
  push_keyset_seek(&mut builder, cursor);
  push_order_and_limit(&mut builder, limit);
  let rows = builder.build_query_as::<UnifiedMail>().fetch_all(&db.0).await?;
  Ok(rows)
}

/// Join `character_mail` to its label membership for a label-scoped listing.
fn push_label_join(builder: &mut QueryBuilder<Sqlite>) {
  builder
    .push("JOIN character_mail_label_membership mem ON mem.character_id = m.character_id AND mem.mail_id = m.mail_id ");
}

/// Exclude mail that is filed or snoozed.
fn push_visible_tail_predicate(builder: &mut QueryBuilder<Sqlite>, now: &str) {
  builder.push(
    " AND NOT EXISTS ( \
      SELECT 1 FROM mail_folder_assignment fa \
      WHERE fa.character_id = m.character_id AND fa.mail_id = m.mail_id \
    ) AND NOT EXISTS ( \
      SELECT 1 FROM mail_snooze sn \
      WHERE sn.character_id = m.character_id AND sn.mail_id = m.mail_id AND sn.snooze_until > ",
  );
  builder.push_bind(now.to_owned());
  builder.push(" )");
}

/// Seek strictly past `cursor` in `(timestamp DESC, mail_id DESC)` order.
fn push_keyset_seek(builder: &mut QueryBuilder<Sqlite>, cursor: Option<&MailCursor>) {
  if let Some(cursor) = cursor {
    builder.push(" AND (m.timestamp < ");
    builder.push_bind(cursor.timestamp.clone());
    builder.push(" OR (m.timestamp = ");
    builder.push_bind(cursor.timestamp.clone());
    builder.push(" AND m.mail_id < ");
    builder.push_bind(cursor.mail_id);
    builder.push("))");
  }
}

fn push_order_and_limit(builder: &mut QueryBuilder<Sqlite>, limit: i64) {
  builder.push(" ORDER BY m.timestamp DESC, m.mail_id DESC LIMIT ");
  builder.push_bind(limit);
}

/// Escape the SQL `LIKE` metacharacters so a user's literal text never acts as a
/// wildcard. Pairs with an `ESCAPE '\'` clause in the query.
fn escape_like(needle: &str) -> String {
  needle.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

pub async fn visible_unread_count(db: &Database, character_id: i64, now: &str) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM character_mail m \
    WHERE m.character_id = ? AND m.is_read = 0 AND m.from_id != m.character_id \
    AND NOT EXISTS ( \
      SELECT 1 FROM mail_folder_assignment fa \
      WHERE fa.character_id = m.character_id AND fa.mail_id = m.mail_id \
    ) AND NOT EXISTS ( \
      SELECT 1 FROM mail_snooze sn \
      WHERE sn.character_id = m.character_id AND sn.mail_id = m.mail_id AND sn.snooze_until > ? \
    )",
  )
  .bind(character_id)
  .bind(now)
  .fetch_one(&db.0)
  .await?;
  Ok(count)
}

pub async fn visible_unread_counts_by_label(
  db: &Database,
  character_id: i64,
  now: &str,
) -> Result<Vec<(i64, i64)>, Error> {
  let rows = sqlx::query_as::<_, (i64, i64)>(
    "SELECT l.label_id, \
      COUNT(m.mail_id) FILTER ( \
        WHERE m.is_read = 0 AND m.from_id != m.character_id \
        AND NOT EXISTS ( \
          SELECT 1 FROM mail_folder_assignment fa \
          WHERE fa.character_id = m.character_id AND fa.mail_id = m.mail_id \
        ) AND NOT EXISTS ( \
          SELECT 1 FROM mail_snooze sn \
          WHERE sn.character_id = m.character_id AND sn.mail_id = m.mail_id AND sn.snooze_until > ? \
        ) \
      ) AS unread \
    FROM character_mail_labels l \
    LEFT JOIN character_mail_label_membership mem \
      ON mem.character_id = l.character_id AND mem.label_id = l.label_id \
    LEFT JOIN character_mail m ON m.character_id = mem.character_id AND m.mail_id = mem.mail_id \
    WHERE l.character_id = ? \
    GROUP BY l.label_id ORDER BY l.label_id",
  )
  .bind(now)
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn visible_unified(db: &Database, now: &str) -> Result<Vec<UnifiedMail>, Error> {
  let rows = sqlx::query_as::<_, UnifiedMail>(
    "SELECT m.character_id, m.mail_id, m.from_id, m.from_name, m.subject, m.timestamp, m.is_read, \
      m.has_attachment, m.important, m.from_corp, m.from_system, m.body \
    FROM mail_unified m \
    WHERE m.from_id != m.character_id \
    AND NOT EXISTS ( \
      SELECT 1 FROM mail_folder_assignment fa \
      WHERE fa.character_id = m.character_id AND fa.mail_id = m.mail_id \
    ) AND NOT EXISTS ( \
      SELECT 1 FROM mail_snooze sn \
      WHERE sn.character_id = m.character_id AND sn.mail_id = m.mail_id AND sn.snooze_until > ? \
    ) \
    ORDER BY m.timestamp DESC, m.mail_id DESC",
  )
  .bind(now)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn visible_unified_unread_count(db: &Database, now: &str) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM mail_unified m \
    WHERE m.is_read = 0 AND m.from_id != m.character_id \
    AND NOT EXISTS ( \
      SELECT 1 FROM mail_folder_assignment fa \
      WHERE fa.character_id = m.character_id AND fa.mail_id = m.mail_id \
    ) AND NOT EXISTS ( \
      SELECT 1 FROM mail_snooze sn \
      WHERE sn.character_id = m.character_id AND sn.mail_id = m.mail_id AND sn.snooze_until > ? \
    )",
  )
  .bind(now)
  .fetch_one(&db.0)
  .await?;
  Ok(count)
}

#[cfg(test)]
mod core_tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn header(character_id: i64, mail_id: i64, subject: &str, ts: &str, is_read: bool) -> CharacterMail {
    CharacterMail {
      character_id,
      from_id: 95_000_001,
      from_name: "Sender Pilot".to_owned(),
      is_read,
      mail_id,
      subject: Some(subject.to_owned()),
      timestamp: ts.to_owned(),
      ..Default::default()
    }
  }

  fn body_of(character_id: i64, mail_id: i64, html: &str) -> CharacterMailBody {
    CharacterMailBody {
      body: html.to_owned(),
      character_id,
      mail_id,
    }
  }

  fn recipient(character_id: i64, mail_id: i64, id: i64, kind: &str, name: &str) -> CharacterMailRecipient {
    CharacterMailRecipient {
      character_id,
      mail_id,
      recipient_id: id,
      recipient_name: name.to_owned(),
      recipient_type: kind.to_owned(),
    }
  }

  fn sent_header(character_id: i64, mail_id: i64, ts: &str, is_read: bool) -> CharacterMail {
    CharacterMail {
      character_id,
      from_id: character_id,
      from_name: "Me".to_owned(),
      is_read,
      mail_id,
      subject: Some("Sent".to_owned()),
      timestamp: ts.to_owned(),
      ..Default::default()
    }
  }

  mod has_body {
    use super::*;

    #[tokio::test]
    async fn it_reports_body_presence() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      assert!(!super::has_body(&db, 42, 1).await.unwrap());

      super::upsert_complete(
        &db,
        &header(42, 1, "Hello", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>x</p>"),
        &[],
      )
      .await
      .unwrap();

      assert!(super::has_body(&db, 42, 1).await.unwrap());
    }
  }

  mod headers_for_label {
    use pretty_assertions::assert_eq;

    use super::*;

    fn label(character_id: i64, label_id: i64, name: &str) -> CharacterMailLabel {
      CharacterMailLabel {
        character_id,
        color: None,
        label_id,
        name: name.to_owned(),
      }
    }

    #[tokio::test]
    async fn it_returns_only_the_labels_members_newest_first() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_labels_for_character(&db, 42, &[label(42, 1, "Inbox"), label(42, 2, "Corp")])
        .await
        .unwrap();

      for (id, ts) in [
        (10, "2026-06-01T10:00:00Z"),
        (11, "2026-06-02T10:00:00Z"),
        (12, "2026-06-03T10:00:00Z"),
      ] {
        super::upsert_complete(&db, &header(42, id, "x", ts, false), &body_of(42, id, "<p>x</p>"), &[])
          .await
          .unwrap();
      }
      super::replace_membership_for_character(
        &db,
        42,
        &[
          CharacterMailLabelMembership {
            character_id: 42,
            label_id: 1,
            mail_id: 10,
          },
          CharacterMailLabelMembership {
            character_id: 42,
            label_id: 1,
            mail_id: 12,
          },
          CharacterMailLabelMembership {
            character_id: 42,
            label_id: 2,
            mail_id: 11,
          },
        ],
      )
      .await
      .unwrap();

      let inbox = super::headers_for_label(&db, 42, 1).await.unwrap();
      assert_eq!(inbox.iter().map(|m| m.mail_id()).collect::<Vec<_>>(), [12, 10]);
    }
  }

  mod label_crud {
    use pretty_assertions::assert_eq;

    use super::*;

    fn label(character_id: i64, label_id: i64, name: &str, color: Option<&str>) -> CharacterMailLabel {
      CharacterMailLabel {
        character_id,
        color: color.map(str::to_owned),
        label_id,
        name: name.to_owned(),
      }
    }

    async fn seed_mail(db: &Database, character_id: i64, mail_id: i64) {
      super::upsert_complete(
        db,
        &header(character_id, mail_id, "x", "2026-06-01T10:00:00Z", false),
        &body_of(character_id, mail_id, "<p>x</p>"),
        &[],
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_adds_membership_idempotently() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::insert_label(&db, &label(42, 1, "Inbox", None)).await.unwrap();
      seed_mail(&db, 42, 10).await;

      super::add_membership(&db, 42, 10, 1).await.unwrap();
      super::add_membership(&db, 42, 10, 1).await.unwrap();

      assert_eq!(super::membership(&db, 42, 10).await.unwrap(), [1]);
    }

    #[tokio::test]
    async fn it_deletes_a_label_and_cascades_its_membership() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::insert_label(&db, &label(42, 1, "Inbox", None)).await.unwrap();
      super::insert_label(&db, &label(42, 2, "Corp", None)).await.unwrap();
      seed_mail(&db, 42, 10).await;
      super::add_membership(&db, 42, 10, 1).await.unwrap();
      super::add_membership(&db, 42, 10, 2).await.unwrap();

      super::delete_label(&db, 42, 1).await.unwrap();

      assert_eq!(
        super::labels(&db, 42)
          .await
          .unwrap()
          .iter()
          .map(|l| l.label_id())
          .collect::<Vec<_>>(),
        [2]
      );
      assert_eq!(super::membership(&db, 42, 10).await.unwrap(), [2]);
    }

    #[tokio::test]
    async fn it_inserts_a_single_label_with_a_negative_temp_id() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::insert_label(&db, &label(42, -1, "Pending", Some("#ff0000")))
        .await
        .unwrap();

      let labels = super::labels(&db, 42).await.unwrap();
      assert_eq!(labels.len(), 1);
      assert_eq!(labels[0].label_id(), -1);
      assert_eq!(labels[0].color().as_deref(), Some("#ff0000"));
    }

    #[tokio::test]
    async fn it_remaps_a_temp_id_to_the_real_id_carrying_membership_along() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::insert_label(&db, &label(42, -1, "Pending", None)).await.unwrap();
      seed_mail(&db, 42, 10).await;
      seed_mail(&db, 42, 11).await;
      super::add_membership(&db, 42, 10, -1).await.unwrap();
      super::add_membership(&db, 42, 11, -1).await.unwrap();

      super::remap_label_id(&db, 42, -1, 555).await.unwrap();

      assert_eq!(
        super::labels(&db, 42)
          .await
          .unwrap()
          .iter()
          .map(|l| l.label_id())
          .collect::<Vec<_>>(),
        [555]
      );
      assert_eq!(super::membership(&db, 42, 10).await.unwrap(), [555]);
      assert_eq!(super::membership(&db, 42, 11).await.unwrap(), [555]);
      assert_eq!(super::headers_for_label(&db, 42, 555).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn it_removes_a_single_membership_leaving_other_labels_intact() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::insert_label(&db, &label(42, 1, "Inbox", None)).await.unwrap();
      super::insert_label(&db, &label(42, 2, "Corp", None)).await.unwrap();
      seed_mail(&db, 42, 10).await;
      super::add_membership(&db, 42, 10, 1).await.unwrap();
      super::add_membership(&db, 42, 10, 2).await.unwrap();

      super::remove_membership(&db, 42, 10, 1).await.unwrap();

      assert_eq!(super::membership(&db, 42, 10).await.unwrap(), [2]);
    }
  }

  mod labels {
    use pretty_assertions::assert_eq;

    use super::*;

    fn label(character_id: i64, label_id: i64, name: &str, color: Option<&str>) -> CharacterMailLabel {
      CharacterMailLabel {
        character_id,
        color: color.map(str::to_owned),
        label_id,
        name: name.to_owned(),
      }
    }

    #[tokio::test]
    async fn it_cascades_membership_when_a_label_is_removed_from_the_catalog() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_labels_for_character(&db, 42, &[label(42, 5, "Temp", None)])
        .await
        .unwrap();
      super::upsert_complete(
        &db,
        &header(42, 1, "x", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>x</p>"),
        &[],
      )
      .await
      .unwrap();
      super::replace_membership_for_character(
        &db,
        42,
        &[CharacterMailLabelMembership {
          character_id: 42,
          label_id: 5,
          mail_id: 1,
        }],
      )
      .await
      .unwrap();

      super::replace_labels_for_character(&db, 42, &[]).await.unwrap();

      assert!(super::membership(&db, 42, 1).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_preserves_optimistic_negative_id_labels_and_membership_across_a_server_replace() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::insert_label(&db, &label(42, -1, "Pending", Some("#ff0000")))
        .await
        .unwrap();
      super::upsert_complete(
        &db,
        &header(42, 1, "x", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>x</p>"),
        &[],
      )
      .await
      .unwrap();
      super::add_membership(&db, 42, 1, -1).await.unwrap();

      super::replace_labels_for_character(&db, 42, &[label(42, 5, "Inbox", None)])
        .await
        .unwrap();
      super::replace_membership_for_character(&db, 42, &[]).await.unwrap();

      assert_eq!(
        super::labels(&db, 42)
          .await
          .unwrap()
          .iter()
          .map(|l| l.label_id())
          .collect::<Vec<_>>(),
        [-1, 5]
      );
      assert_eq!(super::membership(&db, 42, 1).await.unwrap(), [-1]);
    }

    #[tokio::test]
    async fn it_replaces_the_catalog_and_membership_and_computes_unread_locally() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_labels_for_character(
        &db,
        42,
        &[label(42, 1, "Inbox", None), label(42, 2, "Important", Some("#ff0000"))],
      )
      .await
      .unwrap();

      super::upsert_complete(
        &db,
        &header(42, 10, "Unread", "2026-06-01T10:00:00Z", false),
        &body_of(42, 10, "<p>a</p>"),
        &[],
      )
      .await
      .unwrap();
      super::upsert_complete(
        &db,
        &header(42, 11, "Read", "2026-06-01T11:00:00Z", true),
        &body_of(42, 11, "<p>b</p>"),
        &[],
      )
      .await
      .unwrap();

      super::replace_membership_for_character(
        &db,
        42,
        &[
          CharacterMailLabelMembership {
            character_id: 42,
            label_id: 2,
            mail_id: 10,
          },
          CharacterMailLabelMembership {
            character_id: 42,
            label_id: 2,
            mail_id: 11,
          },
        ],
      )
      .await
      .unwrap();

      assert_eq!(
        super::labels(&db, 42)
          .await
          .unwrap()
          .iter()
          .map(|l| l.label_id())
          .collect::<Vec<_>>(),
        [1, 2]
      );
      assert_eq!(super::membership(&db, 42, 10).await.unwrap(), [2]);
      assert_eq!(super::unread_count_for_label(&db, 42, 2).await.unwrap(), 1);
    }
  }

  mod mail {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_assembles_the_full_render_shape() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_labels_for_character(
        &db,
        42,
        &[CharacterMailLabel {
          character_id: 42,
          color: None,
          label_id: 7,
          name: "Corp".to_owned(),
        }],
      )
      .await
      .unwrap();
      super::upsert_complete(
        &db,
        &header(42, 1, "Subject", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>Body</p>"),
        &[
          recipient(42, 1, 1, "character", "Alpha"),
          recipient(42, 1, 2, "corporation", "Bravo Corp"),
        ],
      )
      .await
      .unwrap();
      super::replace_membership_for_character(
        &db,
        42,
        &[CharacterMailLabelMembership {
          character_id: 42,
          label_id: 7,
          mail_id: 1,
        }],
      )
      .await
      .unwrap();

      let render = super::mail(&db, 42, 1).await.unwrap().unwrap();
      assert_eq!(render.header.subject().as_deref(), Some("Subject"));
      assert_eq!(render.body.body(), "<p>Body</p>");
      assert_eq!(render.recipients.len(), 2);
      assert_eq!(render.recipients_display, "Alpha, Bravo Corp");
      assert_eq!(render.label_ids, [7]);
    }

    #[tokio::test]
    async fn it_returns_none_for_a_bodyless_or_unknown_mail() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      assert!(super::mail(&db, 42, 999).await.unwrap().is_none());

      sqlx::query(
        "INSERT INTO character_mail (character_id, mail_id, from_id, from_name, subject, timestamp, is_read) \
          VALUES (42, 5, 95000001, 'X', 'No body', '2026-06-01T10:00:00Z', 0)",
      )
      .execute(db.writer())
      .await
      .unwrap();
      assert!(super::mail(&db, 42, 5).await.unwrap().is_none());
    }
  }

  mod recipients_display {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_empty_for_a_mail_with_no_recipients() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert_complete(
        &db,
        &header(42, 1, "x", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>x</p>"),
        &[],
      )
      .await
      .unwrap();

      assert_eq!(super::recipients_display(&db, 42, 1).await.unwrap(), "");
    }

    #[tokio::test]
    async fn it_joins_resolved_names_with_commas() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert_complete(
        &db,
        &header(42, 1, "x", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>x</p>"),
        &[
          recipient(42, 1, 1, "character", "Alpha"),
          recipient(42, 1, 2, "character", "Bravo"),
        ],
      )
      .await
      .unwrap();

      assert_eq!(super::recipients_display(&db, 42, 1).await.unwrap(), "Alpha, Bravo");
    }
  }

  mod set_read {
    use super::*;

    #[tokio::test]
    async fn it_flips_the_mutable_is_read_flag() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert_complete(
        &db,
        &header(42, 1, "Hello", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>x</p>"),
        &[],
      )
      .await
      .unwrap();

      super::set_read(&db, 42, 1, true).await.unwrap();

      assert!(super::headers(&db, 42).await.unwrap()[0].is_read());
    }
  }

  mod unified {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_excludes_a_header_without_a_body() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      sqlx::query(
        "INSERT INTO character_mail (character_id, mail_id, from_id, from_name, subject, timestamp, is_read) \
          VALUES (42, 9, 95000001, 'X', 'No body', '2026-06-01T10:00:00Z', 0)",
      )
      .execute(db.writer())
      .await
      .unwrap();

      assert!(super::unified(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_merges_owned_characters_mail_newest_first() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;

      super::upsert_complete(
        &db,
        &header(42, 1, "Older", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>from 42</p>"),
        &[],
      )
      .await
      .unwrap();
      super::upsert_complete(
        &db,
        &header(43, 2, "Newer", "2026-06-02T10:00:00Z", false),
        &body_of(43, 2, "<p>from 43</p>"),
        &[],
      )
      .await
      .unwrap();

      let unified = super::unified(&db).await.unwrap();
      assert_eq!(
        unified.iter().map(|m| (m.character_id, m.mail_id)).collect::<Vec<_>>(),
        [(43, 2), (42, 1)]
      );
      assert_eq!(unified[0].body, "<p>from 43</p>");
    }
  }

  mod unread_counts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_combines_unread_across_owned_characters() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      super::upsert_complete(
        &db,
        &header(42, 1, "a", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>a</p>"),
        &[],
      )
      .await
      .unwrap();
      super::upsert_complete(
        &db,
        &header(43, 2, "b", "2026-06-01T11:00:00Z", false),
        &body_of(43, 2, "<p>b</p>"),
        &[],
      )
      .await
      .unwrap();
      super::upsert_complete(
        &db,
        &sent_header(43, 3, "2026-06-01T12:00:00Z", false),
        &body_of(43, 3, "<p>c</p>"),
        &[],
      )
      .await
      .unwrap();

      assert_eq!(super::unified_unread_count(&db).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn it_counts_unread_per_character_excluding_sent() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert_complete(
        &db,
        &header(42, 1, "a", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>a</p>"),
        &[],
      )
      .await
      .unwrap();
      super::upsert_complete(
        &db,
        &header(42, 2, "b", "2026-06-01T11:00:00Z", true),
        &body_of(42, 2, "<p>b</p>"),
        &[],
      )
      .await
      .unwrap();
      super::upsert_complete(
        &db,
        &sent_header(42, 3, "2026-06-01T12:00:00Z", false),
        &body_of(42, 3, "<p>c</p>"),
        &[],
      )
      .await
      .unwrap();

      assert_eq!(super::unread_count(&db, 42).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn it_reports_zero_unread_for_each_label_and_excludes_sent() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_labels_for_character(
        &db,
        42,
        &[
          CharacterMailLabel {
            character_id: 42,
            color: None,
            label_id: 1,
            name: "Inbox".to_owned(),
          },
          CharacterMailLabel {
            character_id: 42,
            color: None,
            label_id: 2,
            name: "Sent".to_owned(),
          },
        ],
      )
      .await
      .unwrap();
      super::upsert_complete(
        &db,
        &header(42, 10, "a", "2026-06-01T10:00:00Z", false),
        &body_of(42, 10, "<p>a</p>"),
        &[],
      )
      .await
      .unwrap();
      super::upsert_complete(
        &db,
        &sent_header(42, 11, "2026-06-01T11:00:00Z", false),
        &body_of(42, 11, "<p>b</p>"),
        &[],
      )
      .await
      .unwrap();
      super::replace_membership_for_character(
        &db,
        42,
        &[
          CharacterMailLabelMembership {
            character_id: 42,
            label_id: 1,
            mail_id: 10,
          },
          CharacterMailLabelMembership {
            character_id: 42,
            label_id: 2,
            mail_id: 11,
          },
        ],
      )
      .await
      .unwrap();

      assert_eq!(super::unread_counts_by_label(&db, 42).await.unwrap(), [(1, 1), (2, 0)]);
    }
  }

  mod upsert_complete {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_keeps_the_immutable_body_on_a_re_sync() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::upsert_complete(
        &db,
        &header(42, 1, "Hello", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>original</p>"),
        &[],
      )
      .await
      .unwrap();
      super::upsert_complete(
        &db,
        &header(42, 1, "Hello", "2026-06-01T10:00:00Z", true),
        &body_of(42, 1, "<p>SHOULD NOT REPLACE</p>"),
        &[],
      )
      .await
      .unwrap();

      assert_eq!(
        super::body(&db, 42, 1).await.unwrap().unwrap().body(),
        "<p>original</p>"
      );
      assert!(super::headers(&db, 42).await.unwrap()[0].is_read());
    }

    #[tokio::test]
    async fn it_persists_header_body_and_recipients_together() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::upsert_complete(
        &db,
        &header(42, 1, "Hello", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>Hi there</p>"),
        &[
          recipient(42, 1, 42, "character", "Me"),
          recipient(42, 1, 90_000_001, "corporation", "My Corp"),
        ],
      )
      .await
      .unwrap();

      let headers = super::headers(&db, 42).await.unwrap();
      assert_eq!(headers.len(), 1);
      assert_eq!(headers[0].subject().as_deref(), Some("Hello"));
      assert_eq!(
        super::body(&db, 42, 1).await.unwrap().unwrap().body(),
        "<p>Hi there</p>"
      );
      assert_eq!(
        super::recipients(&db, 42, 1)
          .await
          .unwrap()
          .iter()
          .map(|r| r.recipient_name().clone())
          .collect::<Vec<_>>(),
        ["Me", "My Corp"]
      );
    }

    #[tokio::test]
    async fn it_replaces_the_recipient_set_on_re_sync() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::upsert_complete(
        &db,
        &header(42, 1, "Hello", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>x</p>"),
        &[recipient(42, 1, 1, "character", "Old")],
      )
      .await
      .unwrap();
      super::upsert_complete(
        &db,
        &header(42, 1, "Hello", "2026-06-01T10:00:00Z", false),
        &body_of(42, 1, "<p>x</p>"),
        &[recipient(42, 1, 2, "character", "New")],
      )
      .await
      .unwrap();

      let recipients = super::recipients(&db, 42, 1).await.unwrap();
      assert_eq!(recipients.len(), 1);
      assert_eq!(recipients[0].recipient_name(), "New");
    }
  }
}

#[cfg(test)]
mod overlay_tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{
      Alliance, Bloodline, Character, CharacterMailBody, CharacterMailLabel, CharacterMailLabelMembership,
      CharacterMailRecipient, Corporation, Gender, Race,
    },
    repo::character,
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  const NOW: &str = "2026-06-15T00:00:00Z";

  fn received(character_id: i64, mail_id: i64, ts: &str, is_read: bool) -> CharacterMail {
    CharacterMail {
      character_id,
      from_id: 95_000_001,
      from_name: "Sender".to_owned(),
      is_read,
      mail_id,
      subject: Some("Subject".to_owned()),
      timestamp: ts.to_owned(),
      ..Default::default()
    }
  }

  fn sent(character_id: i64, mail_id: i64, ts: &str) -> CharacterMail {
    CharacterMail {
      character_id,
      from_id: character_id,
      from_name: "Me".to_owned(),
      is_read: false,
      mail_id,
      subject: Some("Sent".to_owned()),
      timestamp: ts.to_owned(),
      ..Default::default()
    }
  }

  /// A corp broadcast: `from_corp = 1` and a sender id distinct from the owner.
  fn corp_sender(character_id: i64, mail_id: i64, ts: &str) -> CharacterMail {
    CharacterMail {
      character_id,
      from_id: 98_000_001,
      from_name: "Test Corp".to_owned(),
      from_corp: true,
      is_read: false,
      mail_id,
      subject: Some("Corp".to_owned()),
      timestamp: ts.to_owned(),
      ..Default::default()
    }
  }

  /// System mail: `from_system = 1`.
  fn system_sender(character_id: i64, mail_id: i64, ts: &str) -> CharacterMail {
    CharacterMail {
      character_id,
      from_id: 1,
      from_name: "EVE System".to_owned(),
      from_system: true,
      is_read: false,
      mail_id,
      subject: Some("System".to_owned()),
      timestamp: ts.to_owned(),
      ..Default::default()
    }
  }

  /// A mail received by `character_id` whose sender is another owned character.
  fn cross_character(character_id: i64, mail_id: i64, from_id: i64, ts: &str) -> CharacterMail {
    CharacterMail {
      character_id,
      from_id,
      from_name: "Other Pilot".to_owned(),
      is_read: false,
      mail_id,
      subject: Some("Cross".to_owned()),
      timestamp: ts.to_owned(),
      ..Default::default()
    }
  }

  /// Override a header's subject so search tests can share one needle across senders.
  fn with_subject(mut header: CharacterMail, subject: &str) -> CharacterMail {
    header.subject = Some(subject.to_owned());
    header
  }

  async fn store_mail(db: &Database, header: &CharacterMail) {
    let body = CharacterMailBody {
      body: "<p>x</p>".to_owned(),
      character_id: header.character_id(),
      mail_id: header.mail_id(),
    };
    super::upsert_complete(db, header, &body, &[]).await.unwrap();
  }

  mod all_overlay_states {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_merges_multiple_overlays_on_the_same_mail_into_one_row() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::set_triage(&db, 42, 5, true).await.unwrap();
      super::upsert_snoozed_mail(&db, 42, 5, "2026-06-10T08:00:00Z")
        .await
        .unwrap();

      let states = super::all_overlay_states(&db, 42).await.unwrap();

      assert_eq!(states.len(), 1);
      assert_eq!(states[0].mail_id, 5);
      assert!(states[0].is_starred);
      assert!(states[0].is_snoozed());
    }

    #[tokio::test]
    async fn it_returns_one_row_per_overlaid_mail_across_tables() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::set_triage(&db, 42, 1, true).await.unwrap();
      super::upsert_snoozed_mail(&db, 42, 2, "2026-06-10T08:00:00Z")
        .await
        .unwrap();
      super::assign_folder(&db, 42, 3, "archive", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();

      let states = super::all_overlay_states(&db, 42).await.unwrap();

      assert_eq!(states.iter().map(|s| s.mail_id).collect::<Vec<_>>(), [1, 2, 3]);
      assert!(states[0].is_starred);
      assert!(states[1].is_snoozed());
      assert_eq!(states[2].folder.as_deref(), Some("archive"));
    }

    #[tokio::test]
    async fn it_scopes_to_the_given_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      super::set_triage(&db, 42, 1, true).await.unwrap();
      super::set_triage(&db, 43, 2, true).await.unwrap();

      let states = super::all_overlay_states(&db, 42).await.unwrap();

      assert_eq!(states.iter().map(|s| s.mail_id).collect::<Vec<_>>(), [1]);
    }
  }

  mod folder_assignment {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_lists_by_folder_and_clears() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::assign_folder(&db, 42, 1, "archive", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();
      super::assign_folder(&db, 42, 2, "trash", None, true, "2026-06-01T00:00:00Z")
        .await
        .unwrap();
      super::assign_folder(&db, 42, 3, "archive", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();

      assert_eq!(
        super::all_in_folder(&db, 42, "archive")
          .await
          .unwrap()
          .iter()
          .map(|f| f.mail_id())
          .collect::<Vec<_>>(),
        [1, 3]
      );

      super::clear_folder(&db, 42, 1).await.unwrap();
      assert_eq!(
        super::all_in_folder(&db, 42, "archive")
          .await
          .unwrap()
          .iter()
          .map(|f| f.mail_id())
          .collect::<Vec<_>>(),
        [3]
      );
    }

    #[tokio::test]
    async fn it_rejects_an_unknown_folder() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let result = super::assign_folder(&db, 42, 1, "inbox", None, false, "2026-06-01T00:00:00Z").await;

      assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_upserts_a_folder_assignment_with_reserved_columns() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::assign_folder(&db, 42, 1, "archive", Some(7), false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();
      let row = super::folder(&db, 42, 1).await.unwrap().unwrap();
      assert_eq!(row.folder(), "archive");
      assert_eq!(row.remap_label_id(), Some(7));
      assert!(!row.soft_delete_intent());

      super::assign_folder(&db, 42, 1, "trash", None, true, "2026-06-01T00:00:00Z")
        .await
        .unwrap();
      let row = super::folder(&db, 42, 1).await.unwrap().unwrap();
      assert_eq!(row.folder(), "trash");
      assert_eq!(row.remap_label_id(), None);
      assert!(row.soft_delete_intent());
    }
  }

  mod membership_reads {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_lists_mail_ids_in_a_folder() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::assign_folder(&db, 42, 1, "archive", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();
      super::assign_folder(&db, 42, 2, "trash", None, true, "2026-06-01T00:00:00Z")
        .await
        .unwrap();
      super::assign_folder(&db, 42, 3, "archive", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();

      assert_eq!(super::folder_mail_ids(&db, 42, "archive").await.unwrap(), [1, 3]);
      assert_eq!(super::folder_mail_ids(&db, 42, "trash").await.unwrap(), [2]);
    }

    #[tokio::test]
    async fn it_lists_starred_mail_ids() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::set_triage(&db, 42, 1, true).await.unwrap();
      super::set_triage(&db, 42, 2, false).await.unwrap();
      super::set_triage(&db, 42, 3, true).await.unwrap();

      assert_eq!(super::starred_mail_ids(&db, 42).await.unwrap(), [1, 3]);
    }
  }

  mod overlay_state {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_joins_all_three_overlays_into_one_state() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::set_triage(&db, 42, 1, true).await.unwrap();
      super::upsert_snoozed_mail(&db, 42, 1, "2026-06-10T08:00:00Z")
        .await
        .unwrap();
      super::assign_folder(&db, 42, 1, "archive", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();

      let state = super::overlay_state(&db, 42, 1).await.unwrap();

      assert!(state.is_starred);
      assert!(state.is_snoozed());
      assert_eq!(state.snooze_until.as_deref(), Some("2026-06-10T08:00:00Z"));
      assert_eq!(state.folder.as_deref(), Some("archive"));
    }

    #[tokio::test]
    async fn it_reflects_a_partial_overlay() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::set_triage(&db, 42, 1, true).await.unwrap();

      let state = super::overlay_state(&db, 42, 1).await.unwrap();

      assert!(state.is_starred);
      assert!(!state.is_snoozed());
      assert_eq!(state.folder, None);
    }

    #[tokio::test]
    async fn it_returns_an_all_default_state_for_a_mail_with_no_overlays() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let state = super::overlay_state(&db, 42, 7).await.unwrap();

      assert_eq!(state.mail_id, 7);
      assert!(!state.is_starred);
      assert!(!state.is_snoozed());
      assert_eq!(state.snooze_until, None);
      assert_eq!(state.folder, None);
    }
  }

  mod paged_visible_headers {
    use pretty_assertions::assert_eq;

    use super::*;

    fn ids(rows: &[CharacterMail]) -> Vec<i64> {
      rows.iter().map(|m| m.mail_id()).collect()
    }

    #[tokio::test]
    async fn it_excludes_snoozed_and_archived_mail_from_the_listing() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      for id in 1..=4 {
        store_mail(&db, &received(42, id, &format!("2026-06-0{id}T10:00:00Z"), false)).await;
      }
      super::assign_folder(&db, 42, 3, "archive", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();
      super::upsert_snoozed_mail(&db, 42, 2, "2026-06-20T00:00:00Z")
        .await
        .unwrap();

      let page = super::visible_headers_page(&db, 42, NOW, None, 50).await.unwrap();
      assert_eq!(ids(&page), [4, 1]);
    }

    #[tokio::test]
    async fn it_pages_a_label_folder() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_labels_for_character(
        &db,
        42,
        &[CharacterMailLabel {
          character_id: 42,
          color: None,
          label_id: 7,
          name: "Fleet".to_owned(),
        }],
      )
      .await
      .unwrap();
      for id in 1..=3 {
        store_mail(&db, &received(42, id, &format!("2026-06-0{id}T10:00:00Z"), false)).await;
      }
      super::replace_membership_for_character(
        &db,
        42,
        &(1..=3)
          .map(|mail_id| CharacterMailLabelMembership {
            character_id: 42,
            label_id: 7,
            mail_id,
          })
          .collect::<Vec<_>>(),
      )
      .await
      .unwrap();

      let page = super::visible_headers_for_label_page(&db, 42, 7, NOW, None, 50)
        .await
        .unwrap();
      assert_eq!(ids(&page), [3, 2, 1]);
    }

    #[tokio::test]
    async fn it_walks_the_inbox_in_bounded_keyset_pages_newest_first() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      for id in 1..=5 {
        store_mail(&db, &received(42, id, &format!("2026-06-0{id}T10:00:00Z"), false)).await;
      }

      let first = super::visible_headers_page(&db, 42, NOW, None, 2).await.unwrap();
      assert_eq!(ids(&first), [5, 4]);

      let cursor = super::MailCursor::after(first.last().unwrap());
      let second = super::visible_headers_page(&db, 42, NOW, Some(&cursor), 2)
        .await
        .unwrap();
      assert_eq!(ids(&second), [3, 2]);

      let cursor = super::MailCursor::after(second.last().unwrap());
      let third = super::visible_headers_page(&db, 42, NOW, Some(&cursor), 2)
        .await
        .unwrap();
      assert_eq!(ids(&third), [1], "the final short page signals exhaustion");
    }
  }

  mod purge_mail {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn store_full_mail(db: &Database, character_id: i64, mail_id: i64) {
      let header = received(character_id, mail_id, "2026-06-01T10:00:00Z", false);
      let body = CharacterMailBody {
        body: "<p>secret</p>".to_owned(),
        character_id,
        mail_id,
      };
      let recipient = CharacterMailRecipient {
        character_id,
        mail_id,
        recipient_id: character_id,
        recipient_name: "Me".to_owned(),
        recipient_type: "character".to_owned(),
      };
      super::upsert_complete(db, &header, &body, &[recipient]).await.unwrap();
      super::replace_labels_for_character(
        db,
        character_id,
        &[CharacterMailLabel {
          character_id,
          color: None,
          label_id: 1,
          name: "Inbox".to_owned(),
        }],
      )
      .await
      .unwrap();
      super::add_membership(db, character_id, mail_id, 1).await.unwrap();
      super::set_triage(db, character_id, mail_id, true).await.unwrap();
      super::upsert_snoozed_mail(db, character_id, mail_id, "2099-01-01T00:00:00Z")
        .await
        .unwrap();
      super::assign_folder(db, character_id, mail_id, "trash", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();
    }

    async fn row_count(db: &Database, table: &str, character_id: i64, mail_id: i64) -> i64 {
      let sql = match table {
        "character_mail" => "SELECT COUNT(*) FROM character_mail WHERE character_id = ? AND mail_id = ?",
        "character_mail_body" => "SELECT COUNT(*) FROM character_mail_body WHERE character_id = ? AND mail_id = ?",
        "character_mail_recipients" => {
          "SELECT COUNT(*) FROM character_mail_recipients WHERE character_id = ? AND mail_id = ?"
        }
        "mail_triage" => "SELECT COUNT(*) FROM mail_triage WHERE character_id = ? AND mail_id = ?",
        "mail_snooze" => "SELECT COUNT(*) FROM mail_snooze WHERE character_id = ? AND mail_id = ?",
        "mail_folder_assignment" => {
          "SELECT COUNT(*) FROM mail_folder_assignment WHERE character_id = ? AND mail_id = ?"
        }
        _ => "SELECT COUNT(*) FROM character_mail_label_membership WHERE character_id = ? AND mail_id = ?",
      };
      sqlx::query_scalar::<_, i64>(sql)
        .bind(character_id)
        .bind(mail_id)
        .fetch_one(&db.0)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn it_leaves_no_orphan_rows_across_every_dependent_table() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_full_mail(&db, 42, 7).await;

      super::purge_mail(&db, 42, 7).await.unwrap();

      for table in [
        "character_mail",
        "character_mail_body",
        "character_mail_recipients",
        "mail_triage",
        "mail_snooze",
        "mail_folder_assignment",
        "character_mail_label_membership",
      ] {
        assert_eq!(row_count(&db, table, 42, 7).await, 0, "{table} retained an orphan");
      }
    }

    #[tokio::test]
    async fn it_round_trips_every_row_through_snapshot_purge_and_restore() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_full_mail(&db, 42, 7).await;
      let snapshot = super::snapshot_mail(&db, 42, 7).await.unwrap().expect("snapshot");

      super::purge_mail(&db, 42, 7).await.unwrap();
      super::restore_mail(&db, &snapshot).await.unwrap();

      assert_eq!(super::body(&db, 42, 7).await.unwrap().unwrap().body(), "<p>secret</p>");
      assert_eq!(super::recipients(&db, 42, 7).await.unwrap().len(), 1);
      assert_eq!(super::membership(&db, 42, 7).await.unwrap(), [1]);
      let triage = super::triage(&db, 42, 7).await.unwrap().unwrap();
      assert!(triage.star());
      assert_eq!(super::folder(&db, 42, 7).await.unwrap().unwrap().folder(), "trash");
      assert_eq!(super::snapshot_mail(&db, 42, 7).await.unwrap(), Some(snapshot));
    }

    #[tokio::test]
    async fn it_snapshots_none_for_a_mail_with_no_header() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      assert_eq!(super::snapshot_mail(&db, 42, 7).await.unwrap(), None);
    }
  }

  mod purge_synthetic_sent {
    use pretty_assertions::assert_eq;

    use super::*;

    fn self_sent(mail_id: i64, ts: &str) -> CharacterMail {
      CharacterMail {
        character_id: 42,
        from_id: 42,
        from_name: "Me".to_owned(),
        is_read: true,
        mail_id,
        subject: Some("Sent".to_owned()),
        timestamp: ts.to_owned(),
        ..Default::default()
      }
    }

    fn received_mail(mail_id: i64, ts: &str) -> CharacterMail {
      CharacterMail {
        character_id: 42,
        from_id: 95_000_001,
        from_name: "Sender".to_owned(),
        is_read: false,
        mail_id,
        subject: Some("Received".to_owned()),
        timestamp: ts.to_owned(),
        ..Default::default()
      }
    }

    fn body(mail_id: i64) -> CharacterMailBody {
      CharacterMailBody {
        body: "x".to_owned(),
        character_id: 42,
        mail_id,
      }
    }

    #[tokio::test]
    async fn it_drops_negative_self_sent_rows_but_keeps_real_mail() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert_complete(&db, &self_sent(-99, "2026-06-01T10:00:00Z"), &body(-99), &[])
        .await
        .unwrap();
      super::upsert_complete(&db, &self_sent(7, "2026-06-02T10:00:00Z"), &body(7), &[])
        .await
        .unwrap();
      super::upsert_complete(&db, &received_mail(-1, "2026-06-01T09:00:00Z"), &body(-1), &[])
        .await
        .unwrap();

      super::purge_synthetic_sent(&db, 42).await.unwrap();

      let mail_ids: Vec<i64> = super::headers(&db, 42)
        .await
        .unwrap()
        .iter()
        .map(|h| h.mail_id())
        .collect();
      assert_eq!(mail_ids, [7, -1], "only the negative self-sent placeholder is purged");
    }
  }

  mod search_visible_headers {
    use pretty_assertions::assert_eq;

    use super::*;

    fn with_subject_and_sender(mail_id: i64, ts: &str, subject: &str, sender: &str) -> CharacterMail {
      CharacterMail {
        character_id: 42,
        from_id: 95_000_001,
        from_name: sender.to_owned(),
        is_read: false,
        mail_id,
        subject: Some(subject.to_owned()),
        timestamp: ts.to_owned(),
        ..Default::default()
      }
    }

    #[tokio::test]
    async fn it_excludes_archived_mail_and_pages_by_cursor() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      for id in 1..=3 {
        store_mail(
          &db,
          &with_subject_and_sender(id, &format!("2026-06-0{id}T10:00:00Z"), "fleet ping", "Org"),
        )
        .await;
      }
      super::assign_folder(&db, 42, 2, "archive", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();

      let first = super::search_visible_headers_page(&db, 42, NOW, "fleet", None, None, 1)
        .await
        .unwrap();
      assert_eq!(first.iter().map(|m| m.mail_id()).collect::<Vec<_>>(), [3]);

      let cursor = super::MailCursor::after(first.last().unwrap());
      let second = super::search_visible_headers_page(&db, 42, NOW, "fleet", None, Some(&cursor), 50)
        .await
        .unwrap();
      assert_eq!(
        second.iter().map(|m| m.mail_id()).collect::<Vec<_>>(),
        [1],
        "the archived mail 2 is skipped and the cursor advances past it"
      );
    }

    #[tokio::test]
    async fn it_matches_subject_or_sender_case_insensitively_in_bounded_pages() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_mail(
        &db,
        &with_subject_and_sender(1, "2026-06-01T10:00:00Z", "CTA tonight", "Vex"),
      )
      .await;
      store_mail(
        &db,
        &with_subject_and_sender(2, "2026-06-02T10:00:00Z", "Market update", "Cta Bot"),
      )
      .await;
      store_mail(
        &db,
        &with_subject_and_sender(3, "2026-06-03T10:00:00Z", "Standings", "Other"),
      )
      .await;

      let hits = super::search_visible_headers_page(&db, 42, NOW, "cta", None, None, 50)
        .await
        .unwrap();

      assert_eq!(
        hits.iter().map(|m| m.mail_id()).collect::<Vec<_>>(),
        [2, 1],
        "newest first; matches either subject or sender, ignoring case"
      );
    }

    #[tokio::test]
    async fn it_treats_like_metacharacters_as_literals() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_mail(
        &db,
        &with_subject_and_sender(1, "2026-06-01T10:00:00Z", "50% off", "Trader"),
      )
      .await;
      store_mail(
        &db,
        &with_subject_and_sender(2, "2026-06-02T10:00:00Z", "no discount", "Trader"),
      )
      .await;

      let literal = super::search_visible_headers_page(&db, 42, NOW, "50%", None, None, 50)
        .await
        .unwrap();
      assert_eq!(literal.iter().map(|m| m.mail_id()).collect::<Vec<_>>(), [1]);

      let bare_percent = super::search_visible_headers_page(&db, 42, NOW, "%", None, None, 50)
        .await
        .unwrap();
      assert_eq!(
        bare_percent.iter().map(|m| m.mail_id()).collect::<Vec<_>>(),
        [1],
        "a bare percent is a literal needle: it matches only the subject containing '%', not every row"
      );
    }
  }

  mod snooze {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_deletes_a_snooze() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert_snoozed_mail(&db, 42, 1, "2026-06-10T08:00:00Z")
        .await
        .unwrap();

      super::delete_snoozed_mail(&db, 42, 1).await.unwrap();

      assert!(super::all_snoozed_mails(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_returns_only_expired_snoozes() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert_snoozed_mail(&db, 42, 1, "2026-06-01T00:00:00Z")
        .await
        .unwrap();
      super::upsert_snoozed_mail(&db, 42, 2, "2026-12-31T00:00:00Z")
        .await
        .unwrap();

      let expired = super::expired_snoozed_mails(&db, "2026-06-03T00:00:00Z").await.unwrap();

      assert_eq!(expired.iter().map(|s| s.mail_id()).collect::<Vec<_>>(), [1]);
    }

    #[tokio::test]
    async fn it_upserts_and_lists_snoozed_mail() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::upsert_snoozed_mail(&db, 42, 1, "2026-06-10T08:00:00Z")
        .await
        .unwrap();
      super::upsert_snoozed_mail(&db, 42, 1, "2026-06-11T08:00:00Z")
        .await
        .unwrap();
      super::upsert_snoozed_mail(&db, 42, 2, "2026-06-09T08:00:00Z")
        .await
        .unwrap();

      let all = super::all_snoozed_mails(&db, 42).await.unwrap();
      assert_eq!(all.iter().map(|s| s.mail_id()).collect::<Vec<_>>(), [2, 1]);
      assert_eq!(
        all.iter().find(|s| s.mail_id() == 1).unwrap().snooze_until(),
        "2026-06-11T08:00:00Z"
      );
    }
  }

  mod triage {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_cascades_when_the_character_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::set_triage(&db, 42, 1, true).await.unwrap();

      sqlx::query("DELETE FROM characters WHERE id = 42")
        .execute(db.writer())
        .await
        .unwrap();

      assert!(super::all_triage(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_clears_a_triage_row() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::set_triage(&db, 42, 1, true).await.unwrap();

      super::clear_triage(&db, 42, 1).await.unwrap();

      assert!(super::triage(&db, 42, 1).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_upserts_star_in_place() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::set_triage(&db, 42, 1, false).await.unwrap();
      super::set_triage(&db, 42, 1, true).await.unwrap();

      let row = super::triage(&db, 42, 1).await.unwrap().unwrap();
      assert!(row.star());
      assert_eq!(super::all_triage(&db, 42).await.unwrap().len(), 1);
    }
  }

  mod visible_headers {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_hides_snoozed_and_archived_mail() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_mail(&db, &received(42, 1, "2026-06-01T10:00:00Z", false)).await;
      store_mail(&db, &received(42, 2, "2026-06-02T10:00:00Z", false)).await;
      store_mail(&db, &received(42, 3, "2026-06-03T10:00:00Z", false)).await;
      store_mail(&db, &received(42, 4, "2026-06-04T10:00:00Z", false)).await;
      super::assign_folder(&db, 42, 2, "archive", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();
      super::upsert_snoozed_mail(&db, 42, 3, "2026-06-20T00:00:00Z")
        .await
        .unwrap();
      super::upsert_snoozed_mail(&db, 42, 4, "2026-06-10T00:00:00Z")
        .await
        .unwrap();

      let visible = super::visible_headers(&db, 42, NOW).await.unwrap();

      assert_eq!(visible.iter().map(|m| m.mail_id()).collect::<Vec<_>>(), [4, 1]);
    }

    #[tokio::test]
    async fn it_shows_an_expired_snooze_before_the_scheduler_tick_deletes_the_row() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_mail(&db, &received(42, 1, "2026-06-01T10:00:00Z", false)).await;
      super::upsert_snoozed_mail(&db, 42, 1, "2026-06-10T00:00:00Z")
        .await
        .unwrap();

      assert_eq!(super::visible_headers(&db, 42, NOW).await.unwrap().len(), 1);
    }
  }

  mod visible_headers_for_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_applies_the_same_overlay_exclusions() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_labels_for_character(
        &db,
        42,
        &[CharacterMailLabel {
          character_id: 42,
          color: None,
          label_id: 1,
          name: "Inbox".to_owned(),
        }],
      )
      .await
      .unwrap();
      store_mail(&db, &received(42, 10, "2026-06-01T10:00:00Z", false)).await;
      store_mail(&db, &received(42, 11, "2026-06-02T10:00:00Z", false)).await;
      super::replace_membership_for_character(
        &db,
        42,
        &[
          CharacterMailLabelMembership {
            character_id: 42,
            label_id: 1,
            mail_id: 10,
          },
          CharacterMailLabelMembership {
            character_id: 42,
            label_id: 1,
            mail_id: 11,
          },
        ],
      )
      .await
      .unwrap();
      super::assign_folder(&db, 42, 11, "trash", None, true, "2026-06-01T00:00:00Z")
        .await
        .unwrap();

      let visible = super::visible_headers_for_label(&db, 42, 1, NOW).await.unwrap();

      assert_eq!(visible.iter().map(|m| m.mail_id()).collect::<Vec<_>>(), [10]);
    }
  }

  mod visible_unified {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_merges_owned_characters_and_hides_overlaid_mail() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      store_mail(&db, &received(42, 1, "2026-06-01T10:00:00Z", false)).await;
      store_mail(&db, &received(43, 2, "2026-06-02T10:00:00Z", false)).await;
      store_mail(&db, &received(43, 3, "2026-06-03T10:00:00Z", false)).await;
      super::assign_folder(&db, 43, 2, "archive", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();
      super::upsert_snoozed_mail(&db, 43, 3, "2026-06-20T00:00:00Z")
        .await
        .unwrap();

      let unified = super::visible_unified(&db, NOW).await.unwrap();

      assert_eq!(
        unified.iter().map(|m| (m.character_id, m.mail_id)).collect::<Vec<_>>(),
        [(42, 1)]
      );
    }

    #[tokio::test]
    async fn it_excludes_self_sent_mail_but_keeps_corp_system_and_cross_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      store_mail(&db, &received(42, 1, "2026-06-01T10:00:00Z", false)).await;
      store_mail(&db, &sent(42, 2, "2026-06-02T10:00:00Z")).await;
      store_mail(&db, &corp_sender(43, 3, "2026-06-03T10:00:00Z")).await;
      store_mail(&db, &system_sender(43, 4, "2026-06-04T10:00:00Z")).await;
      store_mail(&db, &cross_character(43, 5, 42, "2026-06-05T10:00:00Z")).await;

      let unified = super::visible_unified(&db, NOW).await.unwrap();

      let ids = unified.iter().map(|m| m.mail_id).collect::<Vec<_>>();
      assert_eq!(ids, [5, 4, 3, 1]);
      assert!(!ids.contains(&2), "self-sent mail 2 must be absent from All Inboxes");
    }
  }

  mod search_visible_unified_page {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_excludes_self_sent_mail_from_search_results() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      // All share the searchable subject "Pod" so only the self-sent predicate filters.
      store_mail(
        &db,
        &with_subject(received(42, 1, "2026-06-01T10:00:00Z", false), "Pod from stranger"),
      )
      .await;
      store_mail(&db, &with_subject(sent(42, 2, "2026-06-02T10:00:00Z"), "Pod self sent")).await;
      store_mail(
        &db,
        &with_subject(corp_sender(43, 3, "2026-06-03T10:00:00Z"), "Pod corp"),
      )
      .await;
      store_mail(
        &db,
        &with_subject(cross_character(43, 5, 42, "2026-06-05T10:00:00Z"), "Pod cross"),
      )
      .await;

      let page = super::search_visible_unified_page(&db, NOW, "Pod", None, 50)
        .await
        .unwrap();

      let ids = page.iter().map(|m| m.mail_id).collect::<Vec<_>>();
      assert_eq!(ids, [5, 3, 1]);
      assert!(!ids.contains(&2), "self-sent mail 2 must not appear in unified search");
    }
  }

  mod unified_helper {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_excludes_self_sent_mail_but_keeps_corp_system_and_cross_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      // Received from a stranger.
      store_mail(&db, &received(42, 1, "2026-06-01T10:00:00Z", false)).await;
      // Self-sent by 42 (from_id == character_id): must be hidden from All Inboxes.
      store_mail(&db, &sent(42, 2, "2026-06-02T10:00:00Z")).await;
      // Corp broadcast (from_corp = 1) with a sender id distinct from the owner.
      store_mail(&db, &corp_sender(43, 3, "2026-06-03T10:00:00Z")).await;
      // System mail (from_system = 1).
      store_mail(&db, &system_sender(43, 4, "2026-06-04T10:00:00Z")).await;
      // 42 sent a mail 43 received: from_id (42) != 43's character_id, so 43's copy stays.
      store_mail(&db, &cross_character(43, 5, 42, "2026-06-05T10:00:00Z")).await;

      let unified = super::unified(&db).await.unwrap();

      let ids = unified.iter().map(|m| m.mail_id).collect::<Vec<_>>();
      assert_eq!(ids, [5, 4, 3, 1]);
      assert!(!ids.contains(&2), "self-sent mail 2 must be absent from All Inboxes");
    }
  }

  mod visible_unified_unread_count {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_aggregates_across_characters_with_the_same_exclusions() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      store_mail(&db, &received(42, 1, "2026-06-01T10:00:00Z", false)).await;
      store_mail(&db, &received(43, 2, "2026-06-02T10:00:00Z", false)).await;
      store_mail(&db, &sent(43, 3, "2026-06-03T10:00:00Z")).await;
      store_mail(&db, &received(43, 4, "2026-06-04T10:00:00Z", false)).await;
      super::upsert_snoozed_mail(&db, 43, 4, "2026-06-20T00:00:00Z")
        .await
        .unwrap();

      assert_eq!(super::visible_unified_unread_count(&db, NOW).await.unwrap(), 2);
    }
  }

  mod visible_unread_count {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_a_woken_mail_before_the_scheduler_tick() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_mail(&db, &received(42, 1, "2026-06-01T10:00:00Z", false)).await;
      super::upsert_snoozed_mail(&db, 42, 1, "2026-06-10T00:00:00Z")
        .await
        .unwrap();

      assert_eq!(super::visible_unread_count(&db, 42, NOW).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn it_drops_an_archived_read_mail_from_the_badge() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_mail(&db, &received(42, 1, "2026-06-01T10:00:00Z", true)).await;
      super::assign_folder(&db, 42, 1, "archive", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();

      assert_eq!(super::visible_unread_count(&db, 42, NOW).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn it_excludes_sent_snoozed_and_archived_mail() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_mail(&db, &received(42, 1, "2026-06-01T10:00:00Z", false)).await;
      store_mail(&db, &received(42, 2, "2026-06-02T10:00:00Z", true)).await;
      store_mail(&db, &sent(42, 3, "2026-06-03T10:00:00Z")).await;
      store_mail(&db, &received(42, 4, "2026-06-04T10:00:00Z", false)).await;
      store_mail(&db, &received(42, 5, "2026-06-05T10:00:00Z", false)).await;
      super::assign_folder(&db, 42, 4, "archive", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();
      super::upsert_snoozed_mail(&db, 42, 5, "2026-06-20T00:00:00Z")
        .await
        .unwrap();

      assert_eq!(super::visible_unread_count(&db, 42, NOW).await.unwrap(), 1);
    }
  }

  mod visible_unread_counts_by_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_excludes_snoozed_and_archived_mail_per_label() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_labels_for_character(
        &db,
        42,
        &[
          CharacterMailLabel {
            character_id: 42,
            color: None,
            label_id: 1,
            name: "Inbox".to_owned(),
          },
          CharacterMailLabel {
            character_id: 42,
            color: None,
            label_id: 2,
            name: "Corp".to_owned(),
          },
        ],
      )
      .await
      .unwrap();
      store_mail(&db, &received(42, 10, "2026-06-01T10:00:00Z", false)).await;
      store_mail(&db, &received(42, 11, "2026-06-02T10:00:00Z", false)).await;
      store_mail(&db, &received(42, 20, "2026-06-03T10:00:00Z", false)).await;
      super::replace_membership_for_character(
        &db,
        42,
        &[
          CharacterMailLabelMembership {
            character_id: 42,
            label_id: 1,
            mail_id: 10,
          },
          CharacterMailLabelMembership {
            character_id: 42,
            label_id: 1,
            mail_id: 11,
          },
          CharacterMailLabelMembership {
            character_id: 42,
            label_id: 2,
            mail_id: 20,
          },
        ],
      )
      .await
      .unwrap();
      super::upsert_snoozed_mail(&db, 42, 11, "2026-06-20T00:00:00Z")
        .await
        .unwrap();
      super::assign_folder(&db, 42, 20, "archive", None, false, "2026-06-01T00:00:00Z")
        .await
        .unwrap();

      assert_eq!(
        super::visible_unread_counts_by_label(&db, 42, NOW).await.unwrap(),
        [(1, 1), (2, 0)]
      );
    }
  }

  mod drafts {
    use pretty_assertions::assert_eq;

    use super::*;

    fn input(character_id: i64, subject: &str) -> super::super::DraftInput {
      super::super::DraftInput {
        body: "Body text".to_owned(),
        character_id,
        kind: "New".to_owned(),
        quote: None,
        recipients_cc: "[]".to_owned(),
        recipients_to: "[]".to_owned(),
        subject: subject.to_owned(),
      }
    }

    #[tokio::test]
    async fn it_inserts_a_new_row_when_id_is_none() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let id = super::super::upsert_draft(&db, None, &input(42, "First"))
        .await
        .unwrap();

      let stored = super::super::draft(&db, id).await.unwrap().unwrap();
      assert_eq!(stored.character_id, 42);
      assert_eq!(stored.subject, "First");
      assert_eq!(super::super::count_drafts_for_character(&db, 42).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn it_updates_in_place_without_duplicating_when_id_is_supplied() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let id = super::super::upsert_draft(&db, None, &input(42, "First"))
        .await
        .unwrap();

      let same = super::super::upsert_draft(&db, Some(id), &input(42, "Edited"))
        .await
        .unwrap();

      assert_eq!(same, id);
      assert_eq!(super::super::count_drafts_for_character(&db, 42).await.unwrap(), 1);
      assert_eq!(super::super::draft(&db, id).await.unwrap().unwrap().subject, "Edited");
    }

    #[tokio::test]
    async fn it_lists_only_the_requested_characters_drafts() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      super::super::upsert_draft(&db, None, &input(42, "Mine")).await.unwrap();
      super::super::upsert_draft(&db, None, &input(43, "Theirs"))
        .await
        .unwrap();

      let mine = super::super::list_drafts_for_character(&db, 42).await.unwrap();

      assert_eq!(mine.len(), 1);
      assert_eq!(mine[0].subject, "Mine");
    }

    #[tokio::test]
    async fn it_gets_none_for_a_missing_id() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      assert!(super::super::draft(&db, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_deletes_the_row_by_id() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let id = super::super::upsert_draft(&db, None, &input(42, "Doomed"))
        .await
        .unwrap();

      super::super::delete_draft(&db, id).await.unwrap();

      assert!(super::super::draft(&db, id).await.unwrap().is_none());
      assert_eq!(super::super::count_drafts_for_character(&db, 42).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn it_counts_drafts_scoped_to_the_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      super::super::upsert_draft(&db, None, &input(42, "One")).await.unwrap();
      super::super::upsert_draft(&db, None, &input(42, "Two")).await.unwrap();
      super::super::upsert_draft(&db, None, &input(43, "Other"))
        .await
        .unwrap();

      assert_eq!(super::super::count_drafts_for_character(&db, 42).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn it_round_trips_kind_and_quote_context() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut payload = input(42, "Re: Hello");
      payload.kind = "Reply".to_owned();
      payload.quote = Some("> original".to_owned());

      let id = super::super::upsert_draft(&db, None, &payload).await.unwrap();

      let stored = super::super::draft(&db, id).await.unwrap().unwrap();
      assert_eq!(stored.kind, "Reply");
      assert_eq!(stored.quote.as_deref(), Some("> original"));
    }

    #[tokio::test]
    async fn it_cascades_deletes_when_the_character_is_removed() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::super::upsert_draft(&db, None, &input(42, "Orphan"))
        .await
        .unwrap();

      character::delete(&db, 42).await.unwrap();

      assert_eq!(super::super::count_drafts_for_character(&db, 42).await.unwrap(), 0);
    }
  }
}
