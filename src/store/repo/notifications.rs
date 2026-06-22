use sqlx::FromRow;

use crate::store::{
  Database, Error,
  model::{
    NewNotification, Notification, NotificationDestination, NotificationKind, NotificationOwner, NotificationTarget,
  },
};

/// Surfaced-row cap. emit() prunes the oldest surfaced rows beyond this many opportunistically, so the
/// center never grows without bound. Suppressed watermark rows are the dedup ledger and are NEVER
/// pruned here — dropping one would let its event re-notify on the next detector pass.
const SURFACED_RETENTION: i64 = 200;

#[allow(dead_code)]
#[derive(Clone, Debug, FromRow)]
struct Row {
  body: String,
  created_at: String,
  dedup_key: String,
  id: i64,
  kind: String,
  owner_id: i64,
  owner_type: String,
  read_at: Option<String>,
  target_char: Option<i64>,
  target_dest: String,
  target_sub: Option<String>,
  title: String,
}

#[allow(dead_code)]
impl Row {
  fn into_notification(self) -> Option<Notification> {
    Some(Notification {
      body: self.body,
      created_at: self.created_at,
      dedup_key: self.dedup_key,
      id: self.id,
      kind: NotificationKind::from_key(&self.kind)?,
      owner: NotificationOwner::from_key(&self.owner_type, self.owner_id)?,
      read_at: self.read_at,
      target: NotificationTarget {
        character: self.target_char,
        destination: NotificationDestination::from_key(&self.target_dest),
        sub: self.target_sub,
      },
      title: self.title,
    })
  }
}

// Notification storage repo (epic zyrmyrlk, spec A). Called by the detectors (spec B) and the
// center/toast UI (specs C/D); exercised only by unit tests until those land.
#[allow(dead_code)]
pub async fn clear_all(db: &Database) -> Result<(), Error> {
  sqlx::query("DELETE FROM notifications WHERE suppressed = 0")
    .execute(db.writer())
    .await?;
  Ok(())
}

// INSERT OR IGNORE makes this insert-if-absent: a duplicate dedup_key (already-notified, surfaced or
// watermarked) yields zero rows, so `RETURNING ... fetch_optional` gives None and the emit is a no-op.
// Surfaced rows pin suppressed=0; prune runs only after a successful insert.
#[allow(dead_code)]
pub async fn emit(db: &Database, notification: &NewNotification) -> Result<Option<Notification>, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, Row>(
    "INSERT OR IGNORE INTO notifications \
      (kind, owner_type, owner_id, dedup_key, title, body, target_dest, target_char, target_sub, created_at, suppressed) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0) \
      RETURNING id, kind, owner_type, owner_id, dedup_key, title, body, target_dest, target_char, target_sub, \
      created_at, read_at",
  )
  .bind(notification.kind.as_str())
  .bind(notification.owner.owner_type())
  .bind(notification.owner.owner_id())
  .bind(&notification.dedup_key)
  .bind(&notification.title)
  .bind(&notification.body)
  .bind(notification.target.destination.as_str())
  .bind(notification.target.character)
  .bind(&notification.target.sub)
  .bind(&now)
  .fetch_optional(db.writer())
  .await?;

  if row.is_some() {
    prune(db).await?;
  }

  Ok(row.and_then(Row::into_notification))
}

#[allow(dead_code)]
pub async fn list(db: &Database, limit: i64) -> Result<Vec<Notification>, Error> {
  let rows = sqlx::query_as::<_, Row>(
    "SELECT id, kind, owner_type, owner_id, dedup_key, title, body, target_dest, target_char, target_sub, \
      created_at, read_at \
      FROM notifications WHERE suppressed = 0 ORDER BY created_at DESC, id DESC LIMIT ?",
  )
  .bind(limit)
  .fetch_all(db.reader())
  .await?;
  Ok(rows.into_iter().filter_map(Row::into_notification).collect())
}

#[allow(dead_code)]
pub async fn mark_all_read(db: &Database) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  sqlx::query("UPDATE notifications SET read_at = ? WHERE suppressed = 0 AND read_at IS NULL")
    .bind(&now)
    .execute(db.writer())
    .await?;
  Ok(())
}

#[allow(dead_code)]
pub async fn mark_read(db: &Database, id: i64) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  sqlx::query("UPDATE notifications SET read_at = ? WHERE id = ? AND read_at IS NULL")
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

#[allow(dead_code)]
pub async fn unread_count(db: &Database) -> Result<i64, Error> {
  let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE suppressed = 0 AND read_at IS NULL")
    .fetch_one(db.reader())
    .await?;
  Ok(count)
}

// First-run backfill: insert suppressed=1 rows that occupy each dedup_key (so a later emit() with the
// same key is a no-op) but never surface in list()/unread_count(). INSERT OR IGNORE makes re-running
// the first-run guard safe. The kind/owner/target columns are intentionally placeholder empties: a
// watermark row is a pure dedup ledger entry, never rendered.
#[allow(dead_code)]
pub async fn watermark(db: &Database, dedup_keys: &[String]) -> Result<(), Error> {
  if dedup_keys.is_empty() {
    return Ok(());
  }

  let now = chrono::Utc::now().to_rfc3339();
  let mut tx = db.writer().begin().await?;
  for dedup_key in dedup_keys {
    sqlx::query(
      "INSERT OR IGNORE INTO notifications \
        (kind, owner_type, owner_id, dedup_key, title, body, target_dest, created_at, suppressed) \
        VALUES ('', 'character', 0, ?, '', '', '', ?, 1)",
    )
    .bind(dedup_key)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
  }
  tx.commit().await?;
  Ok(())
}

#[allow(dead_code)]
async fn prune(db: &Database) -> Result<(), Error> {
  sqlx::query(
    "DELETE FROM notifications WHERE suppressed = 0 AND id NOT IN \
      (SELECT id FROM notifications WHERE suppressed = 0 ORDER BY created_at DESC, id DESC LIMIT ?)",
  )
  .bind(SURFACED_RETENTION)
  .execute(db.writer())
  .await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  fn sample(dedup_key: &str) -> NewNotification {
    NewNotification {
      body: format!("body {dedup_key}"),
      dedup_key: dedup_key.to_owned(),
      kind: NotificationKind::Skill,
      owner: NotificationOwner::Character(42),
      target: NotificationTarget {
        character: Some(42),
        destination: NotificationDestination::Skills,
        sub: None,
      },
      title: format!("title {dedup_key}"),
    }
  }

  mod clear_all {
    use super::*;

    #[tokio::test]
    async fn it_removes_surfaced_rows_but_keeps_watermarks() {
      let db = store::open_test().await.unwrap();
      emit(&db, &sample("skill:1")).await.unwrap();
      watermark(&db, &["skill:wm".to_owned()]).await.unwrap();

      clear_all(&db).await.unwrap();

      assert!(list(&db, 50).await.unwrap().is_empty());
      assert!(emit(&db, &sample("skill:wm")).await.unwrap().is_none());
    }
  }

  mod emit {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_a_surfaced_row_the_first_time() {
      let db = store::open_test().await.unwrap();

      let inserted = emit(&db, &sample("skill:1")).await.unwrap();

      let notification = inserted.expect("first emit surfaces a row");
      assert_eq!(notification.dedup_key(), "skill:1");
      assert_eq!(notification.kind(), NotificationKind::Skill);
      assert_eq!(notification.owner(), NotificationOwner::Character(42));
      assert_eq!(notification.read_at(), &None);
    }

    #[tokio::test]
    async fn it_is_a_no_op_on_a_duplicate_dedup_key() {
      let db = store::open_test().await.unwrap();
      emit(&db, &sample("skill:1")).await.unwrap();

      let again = emit(&db, &sample("skill:1")).await.unwrap();

      assert_eq!(again, None);
      assert_eq!(list(&db, 50).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_does_not_surface_a_watermarked_key() {
      let db = store::open_test().await.unwrap();
      watermark(&db, &["skill:1".to_owned()]).await.unwrap();

      let blocked = emit(&db, &sample("skill:1")).await.unwrap();

      assert_eq!(blocked, None);
      assert!(list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_prunes_surfaced_rows_beyond_the_cap() {
      let db = store::open_test().await.unwrap();
      for i in 0..(SURFACED_RETENTION + 5) {
        let mut notification = sample(&format!("skill:{i}"));
        notification.title = format!("{i:05}");
        emit(&db, &notification).await.unwrap();
      }

      let listed = list(&db, SURFACED_RETENTION + 100).await.unwrap();

      assert_eq!(listed.len() as i64, SURFACED_RETENTION);
    }
  }

  mod list {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_surfaced_rows_newest_first() {
      let db = store::open_test().await.unwrap();
      emit(&db, &sample("skill:1")).await.unwrap();
      emit(&db, &sample("skill:2")).await.unwrap();

      let listed = list(&db, 50).await.unwrap();

      let keys = listed.iter().map(Notification::dedup_key).collect::<Vec<_>>();
      assert_eq!(keys, ["skill:2", "skill:1"]);
    }

    #[tokio::test]
    async fn it_excludes_watermark_rows() {
      let db = store::open_test().await.unwrap();
      watermark(&db, &["skill:wm".to_owned()]).await.unwrap();
      emit(&db, &sample("skill:1")).await.unwrap();

      let listed = list(&db, 50).await.unwrap();

      assert_eq!(listed.len(), 1);
      assert_eq!(listed[0].dedup_key(), "skill:1");
    }

    #[tokio::test]
    async fn it_honours_the_limit() {
      let db = store::open_test().await.unwrap();
      emit(&db, &sample("skill:1")).await.unwrap();
      emit(&db, &sample("skill:2")).await.unwrap();
      emit(&db, &sample("skill:3")).await.unwrap();

      let listed = list(&db, 2).await.unwrap();

      assert_eq!(listed.len(), 2);
    }
  }

  mod mark_all_read {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_clears_every_unread_badge() {
      let db = store::open_test().await.unwrap();
      emit(&db, &sample("skill:1")).await.unwrap();
      emit(&db, &sample("skill:2")).await.unwrap();

      mark_all_read(&db).await.unwrap();

      assert_eq!(unread_count(&db).await.unwrap(), 0);
    }
  }

  mod mark_read {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_drops_the_row_from_the_unread_count() {
      let db = store::open_test().await.unwrap();
      let one = emit(&db, &sample("skill:1")).await.unwrap().unwrap();
      emit(&db, &sample("skill:2")).await.unwrap();

      mark_read(&db, one.id()).await.unwrap();

      assert_eq!(unread_count(&db).await.unwrap(), 1);
    }
  }

  mod unread_count {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_only_surfaced_unread_rows() {
      let db = store::open_test().await.unwrap();
      emit(&db, &sample("skill:1")).await.unwrap();
      watermark(&db, &["skill:wm".to_owned()]).await.unwrap();

      assert_eq!(unread_count(&db).await.unwrap(), 1);
    }
  }

  mod watermark {
    use super::*;

    #[tokio::test]
    async fn it_is_a_no_op_on_conflict() {
      let db = store::open_test().await.unwrap();

      watermark(&db, &["skill:wm".to_owned()]).await.unwrap();
      watermark(&db, &["skill:wm".to_owned()]).await.unwrap();

      assert!(emit(&db, &sample("skill:wm")).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_ignores_an_empty_key_set() {
      let db = store::open_test().await.unwrap();

      watermark(&db, &[]).await.unwrap();

      assert!(list(&db, 50).await.unwrap().is_empty());
    }
  }
}
