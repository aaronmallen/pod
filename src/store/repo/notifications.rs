use crate::store::{
  Database, Error,
  model::{HistoryCursor, NewNotification, Notification, NotificationKind, NotificationOwner, NotificationRow},
};

/// Time-based retention window for surfaced rows. emit() prunes surfaced rows whose created_at is
/// older than this many days opportunistically, so the center is bounded by age (not count) and the
/// full recent history stays available for keyset paging in the History view. Pruning tombstones
/// (suppressed = 1) rather than deletes, so an aged row leaves the center but keeps its dedup_key
/// occupied — its event can never re-notify on a later detector pass. Tunable; ~90 days keeps a
/// meaningful history without unbounded growth.
const NOTIFICATION_RETENTION_DAYS: i64 = 90;

/// Default keyset page size for the History view. A caller may request a different limit.
pub const HISTORY_PAGE_SIZE: i64 = 50;

// Notification storage repo (epic zyrmyrlk, spec A). Called by the detectors (spec B) and the
// center/toast UI (specs C/D); exercised only by unit tests until those land.
// Clearing the center tombstones the surfaced rows (suppressed = 1) rather than deleting them: the
// dedup_key stays occupied, so emit()'s INSERT OR IGNORE remains a permanent no-op and a cleared
// event can never re-surface or re-toast on a later detector pass. The rows drop out of
// list()/unread_count() (both filter suppressed = 0) but persist as ledger tombstones across restart.
#[cfg_attr(not(test), expect(dead_code))]
pub async fn clear_all(db: &Database) -> Result<(), Error> {
  sqlx::query("UPDATE notifications SET suppressed = 1 WHERE suppressed = 0")
    .execute(db.writer())
    .await?;
  Ok(())
}

// INSERT OR IGNORE makes this insert-if-absent: a duplicate dedup_key (already-notified, surfaced or
// watermarked) yields zero rows, so `RETURNING ... fetch_optional` gives None and the emit is a no-op.
// Surfaced rows pin suppressed=0; prune runs only after a successful insert.
pub async fn emit(db: &Database, notification: &NewNotification) -> Result<Option<Notification>, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, NotificationRow>(
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

  Ok(row.and_then(NotificationRow::into_notification))
}

pub async fn list(db: &Database, limit: i64) -> Result<Vec<Notification>, Error> {
  let rows = sqlx::query_as::<_, NotificationRow>(
    "SELECT id, kind, owner_type, owner_id, dedup_key, title, body, target_dest, target_char, target_sub, \
      created_at, read_at \
      FROM notifications WHERE suppressed = 0 ORDER BY created_at DESC, id DESC LIMIT ?",
  )
  .bind(limit)
  .fetch_all(db.reader())
  .await?;
  Ok(
    rows
      .into_iter()
      .filter_map(NotificationRow::into_notification)
      .collect(),
  )
}

// Keyset page over surfaced rows for the History view. `cursor` is the (created_at, id) of the last
// row the previous page returned; None requests the newest page. The row-value predicate
// (created_at, id) < (?, ?) plus the matching ORDER BY rides the idx_notifications_surfaced_keyset
// partial index, so deep history pages stay cheap. Walking with each page's last cursor visits every
// surfaced row exactly once, even when newer rows are inserted mid-paging — they sort ahead of the
// cursor and never reappear in a later page.
pub async fn list_page(db: &Database, cursor: Option<&HistoryCursor>, limit: i64) -> Result<Vec<Notification>, Error> {
  let rows = match cursor {
    Some(cursor) => {
      sqlx::query_as::<_, NotificationRow>(
        "SELECT id, kind, owner_type, owner_id, dedup_key, title, body, target_dest, target_char, target_sub, \
          created_at, read_at \
          FROM notifications \
          WHERE suppressed = 0 AND (created_at < ? OR (created_at = ? AND id < ?)) \
          ORDER BY created_at DESC, id DESC LIMIT ?",
      )
      .bind(&cursor.created_at)
      .bind(&cursor.created_at)
      .bind(cursor.id)
      .bind(limit)
      .fetch_all(db.reader())
      .await?
    }
    None => {
      sqlx::query_as::<_, NotificationRow>(
        "SELECT id, kind, owner_type, owner_id, dedup_key, title, body, target_dest, target_char, target_sub, \
          created_at, read_at \
          FROM notifications WHERE suppressed = 0 ORDER BY created_at DESC, id DESC LIMIT ?",
      )
      .bind(limit)
      .fetch_all(db.reader())
      .await?
    }
  };
  Ok(
    rows
      .into_iter()
      .filter_map(NotificationRow::into_notification)
      .collect(),
  )
}

// Surfaced unread rows (read_at IS NULL AND suppressed = 0), newest-first, for the New tab — correct
// independent of how far History has paged.
#[cfg_attr(not(test), expect(dead_code))]
pub async fn list_unread(db: &Database, limit: i64) -> Result<Vec<Notification>, Error> {
  let rows = sqlx::query_as::<_, NotificationRow>(
    "SELECT id, kind, owner_type, owner_id, dedup_key, title, body, target_dest, target_char, target_sub, \
      created_at, read_at \
      FROM notifications WHERE suppressed = 0 AND read_at IS NULL ORDER BY created_at DESC, id DESC LIMIT ?",
  )
  .bind(limit)
  .fetch_all(db.reader())
  .await?;
  Ok(
    rows
      .into_iter()
      .filter_map(NotificationRow::into_notification)
      .collect(),
  )
}

pub async fn mark_all_read(db: &Database) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  sqlx::query("UPDATE notifications SET read_at = ? WHERE suppressed = 0 AND read_at IS NULL")
    .bind(&now)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn mark_read(db: &Database, id: i64) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  sqlx::query("UPDATE notifications SET read_at = ? WHERE id = ? AND read_at IS NULL")
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn unread_count(db: &Database) -> Result<i64, Error> {
  let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE suppressed = 0 AND read_at IS NULL")
    .fetch_one(db.reader())
    .await?;
  Ok(count)
}

// Whether the table already holds any row (surfaced OR suppressed watermark) for this owner+kind. The
// detectors use this — not the sync ledger — to decide a subject's first scan: the sync engine records
// the ledger's last_success_at BEFORE the detector pulse, so a ledger-based first-run check would read
// false on the very first sync and flood the whole history. A row only exists here once a prior scan
// either surfaced or watermarked this owner+kind, so its presence is the true first-scan signal.
pub async fn has_any(db: &Database, owner: &NotificationOwner, kind: NotificationKind) -> Result<bool, Error> {
  let count: i64 =
    sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE owner_type = ? AND owner_id = ? AND kind = ? LIMIT 1")
      .bind(owner.owner_type())
      .bind(owner.owner_id())
      .bind(kind.as_str())
      .fetch_one(db.reader())
      .await?;
  Ok(count > 0)
}

/// dedup_key of the per-(owner, kind) sentinel a first scan always writes, so has_any() flips true even
/// when the source had no history — otherwise a later first item would read as another first scan and be
/// silently watermarked instead of surfaced. The "first_scan:" prefix can never collide with a real
/// event key (none start with it).
fn first_scan_sentinel(owner: &NotificationOwner, kind: NotificationKind) -> String {
  format!(
    "first_scan:{}:{}:{}",
    kind.as_str(),
    owner.owner_type(),
    owner.owner_id()
  )
}

// First-run backfill: insert suppressed=1 rows that occupy each dedup_key (so a later emit() with the
// same key is a no-op) but never surface in list()/unread_count(). A per-(owner, kind) sentinel row is
// always written so has_any() reports the subject as seen even when there was no history, keeping a
// later first item surfacing instead of being silently watermarked. INSERT OR IGNORE makes re-running
// the first-run guard safe. The rows carry the real owner+kind (not placeholders) so has_any() can
// recognise the subject's first scan; the title/body/target columns stay empty since a watermark row is
// a pure dedup ledger entry, never rendered.
pub async fn watermark(
  db: &Database,
  owner: &NotificationOwner,
  kind: NotificationKind,
  dedup_keys: &[String],
) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let sentinel = first_scan_sentinel(owner, kind);
  let mut tx = db.writer().begin().await?;
  for dedup_key in std::iter::once(&sentinel).chain(dedup_keys) {
    sqlx::query(
      "INSERT OR IGNORE INTO notifications \
        (kind, owner_type, owner_id, dedup_key, title, body, target_dest, created_at, suppressed) \
        VALUES (?, ?, ?, ?, '', '', '', ?, 1)",
    )
    .bind(kind.as_str())
    .bind(owner.owner_type())
    .bind(owner.owner_id())
    .bind(dedup_key)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
  }
  tx.commit().await?;
  Ok(())
}

// Time-based retention: tombstone surfaced rows whose created_at is older than the retention window,
// computed as now - NOTIFICATION_RETENTION_DAYS and bound as an RFC3339 string. String comparison is
// exact over the normalized RFC3339 timestamps emit() writes. Aged rows are flipped to suppressed = 1
// rather than deleted, so they leave the center (list()/unread_count() filter suppressed = 0) while
// keeping their dedup_key occupied — emit() stays a permanent no-op and the event never re-notifies.
// Suppressed watermark rows are already excluded by the suppressed = 0 guard.
async fn prune(db: &Database) -> Result<(), Error> {
  let cutoff = (chrono::Utc::now() - chrono::Duration::days(NOTIFICATION_RETENTION_DAYS)).to_rfc3339();
  sqlx::query("UPDATE notifications SET suppressed = 1 WHERE suppressed = 0 AND created_at < ?")
    .bind(&cutoff)
    .execute(db.writer())
    .await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{NotificationDestination, NotificationTarget},
  };

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

  // Insert a surfaced row with an exact created_at so paging/prune boundaries are testable (emit()
  // would stamp every row with now()). Returns the new row id.
  async fn seed_surfaced(db: &Database, dedup_key: &str, created_at: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
      "INSERT INTO notifications \
        (kind, owner_type, owner_id, dedup_key, title, body, target_dest, created_at, suppressed) \
        VALUES ('skill', 'character', 42, ?, ?, '', 'skills', ?, 0) RETURNING id",
    )
    .bind(dedup_key)
    .bind(format!("title {dedup_key}"))
    .bind(created_at)
    .fetch_one(db.writer())
    .await
    .unwrap()
  }

  mod clear_all {
    use super::*;

    #[tokio::test]
    async fn it_removes_surfaced_rows_but_keeps_watermarks() {
      let db = store::open_test().await.unwrap();
      emit(&db, &sample("skill:1")).await.unwrap();
      watermark(
        &db,
        &NotificationOwner::Character(42),
        NotificationKind::Skill,
        &["skill:wm".to_owned()],
      )
      .await
      .unwrap();

      clear_all(&db).await.unwrap();

      assert!(list(&db, 50).await.unwrap().is_empty());
      assert!(emit(&db, &sample("skill:wm")).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_tombstones_so_a_cleared_key_never_re_surfaces() {
      let db = store::open_test().await.unwrap();
      emit(&db, &sample("skill:1")).await.unwrap();

      clear_all(&db).await.unwrap();

      // Re-emitting the cleared key is a permanent no-op — the tombstone keeps the dedup_key occupied.
      assert_eq!(emit(&db, &sample("skill:1")).await.unwrap(), None);
      assert!(list(&db, 50).await.unwrap().is_empty());
      assert_eq!(unread_count(&db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn it_still_surfaces_a_genuinely_new_event_after_a_clear() {
      let db = store::open_test().await.unwrap();
      emit(&db, &sample("skill:1")).await.unwrap();
      clear_all(&db).await.unwrap();

      // A distinct dedup_key was never tombstoned, so it surfaces exactly once.
      let fresh = emit(&db, &sample("skill:2")).await.unwrap();

      assert!(fresh.is_some(), "a new event still surfaces after a clear");
      let keys = list(&db, 50)
        .await
        .unwrap()
        .iter()
        .map(|n| n.dedup_key().clone())
        .collect::<Vec<_>>();
      assert_eq!(keys, ["skill:2"]);
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
      watermark(
        &db,
        &NotificationOwner::Character(42),
        NotificationKind::Skill,
        &["skill:1".to_owned()],
      )
      .await
      .unwrap();

      let blocked = emit(&db, &sample("skill:1")).await.unwrap();

      assert_eq!(blocked, None);
      assert!(list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_prunes_surfaced_rows_older_than_the_retention_window() {
      let db = store::open_test().await.unwrap();
      // A row aged just past the window, plus a recent row and a watermark, all seeded directly so
      // their created_at is controlled. A fresh emit() then triggers prune().
      let old = (chrono::Utc::now() - chrono::Duration::days(NOTIFICATION_RETENTION_DAYS + 1)).to_rfc3339();
      let recent = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
      seed_surfaced(&db, "skill:old", &old).await;
      seed_surfaced(&db, "skill:recent", &recent).await;
      watermark(
        &db,
        &NotificationOwner::Character(42),
        NotificationKind::Skill,
        &["skill:wm".to_owned()],
      )
      .await
      .unwrap();

      emit(&db, &sample("skill:trigger")).await.unwrap();

      let keys = list(&db, 50)
        .await
        .unwrap()
        .iter()
        .map(|n| n.dedup_key().clone())
        .collect::<Vec<_>>();
      assert!(
        !keys.contains(&"skill:old".to_owned()),
        "the aged row leaves the center"
      );
      assert!(keys.contains(&"skill:recent".to_owned()), "a recent row survives");
      // The watermark is exempt from prune: re-emitting its key is still a no-op.
      assert!(emit(&db, &sample("skill:wm")).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_tombstones_aged_rows_so_they_never_re_surface() {
      let db = store::open_test().await.unwrap();
      let old = (chrono::Utc::now() - chrono::Duration::days(NOTIFICATION_RETENTION_DAYS + 1)).to_rfc3339();
      seed_surfaced(&db, "skill:old", &old).await;

      // A fresh emit triggers prune(), which tombstones (not deletes) the aged row.
      emit(&db, &sample("skill:trigger")).await.unwrap();

      // Re-emitting the aged key is a permanent no-op: its tombstone still occupies the dedup_key, so
      // the event can never re-surface or re-toast.
      assert_eq!(emit(&db, &sample("skill:old")).await.unwrap(), None);
      let keys = list(&db, 50)
        .await
        .unwrap()
        .iter()
        .map(|n| n.dedup_key().clone())
        .collect::<Vec<_>>();
      assert!(
        !keys.contains(&"skill:old".to_owned()),
        "the tombstone stays out of the center"
      );
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
      watermark(
        &db,
        &NotificationOwner::Character(42),
        NotificationKind::Skill,
        &["skill:wm".to_owned()],
      )
      .await
      .unwrap();
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
      watermark(
        &db,
        &NotificationOwner::Character(42),
        NotificationKind::Skill,
        &["skill:wm".to_owned()],
      )
      .await
      .unwrap();

      assert_eq!(unread_count(&db).await.unwrap(), 1);
    }
  }

  mod has_any {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_false_before_any_scan() {
      let db = store::open_test().await.unwrap();

      assert_eq!(
        has_any(&db, &NotificationOwner::Character(42), NotificationKind::Skill)
          .await
          .unwrap(),
        false
      );
    }

    #[tokio::test]
    async fn it_is_true_after_a_surfaced_row() {
      let db = store::open_test().await.unwrap();
      emit(&db, &sample("skill:1")).await.unwrap();

      assert!(
        has_any(&db, &NotificationOwner::Character(42), NotificationKind::Skill)
          .await
          .unwrap()
      );
    }

    #[tokio::test]
    async fn it_is_true_after_a_watermark_even_with_no_history() {
      let db = store::open_test().await.unwrap();
      watermark(&db, &NotificationOwner::Character(42), NotificationKind::Skill, &[])
        .await
        .unwrap();

      assert!(
        has_any(&db, &NotificationOwner::Character(42), NotificationKind::Skill)
          .await
          .unwrap()
      );
    }

    #[tokio::test]
    async fn it_is_scoped_to_owner_and_kind() {
      let db = store::open_test().await.unwrap();
      watermark(
        &db,
        &NotificationOwner::Character(42),
        NotificationKind::Skill,
        &["skill:wm".to_owned()],
      )
      .await
      .unwrap();

      assert!(
        !has_any(&db, &NotificationOwner::Character(42), NotificationKind::Mail)
          .await
          .unwrap()
      );
      assert!(
        !has_any(&db, &NotificationOwner::Corporation(42), NotificationKind::Skill)
          .await
          .unwrap()
      );
    }
  }

  mod watermark {
    use super::*;

    #[tokio::test]
    async fn it_is_a_no_op_on_conflict() {
      let db = store::open_test().await.unwrap();

      watermark(
        &db,
        &NotificationOwner::Character(42),
        NotificationKind::Skill,
        &["skill:wm".to_owned()],
      )
      .await
      .unwrap();
      watermark(
        &db,
        &NotificationOwner::Character(42),
        NotificationKind::Skill,
        &["skill:wm".to_owned()],
      )
      .await
      .unwrap();

      assert!(emit(&db, &sample("skill:wm")).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_surfaces_nothing_for_an_empty_key_set() {
      let db = store::open_test().await.unwrap();

      watermark(&db, &NotificationOwner::Character(42), NotificationKind::Skill, &[])
        .await
        .unwrap();

      assert!(list(&db, 50).await.unwrap().is_empty());
    }
  }

  mod list_page {
    use pretty_assertions::assert_eq;

    use super::*;

    // Distinct, ordered created_at values so (created_at, id) boundaries are unambiguous.
    async fn seed_history(db: &Database) {
      for i in 0..5 {
        let created = format!("2026-06-{:02}T00:00:00+00:00", 20 - i);
        seed_surfaced(db, &format!("skill:{i}"), &created).await;
      }
    }

    #[tokio::test]
    async fn it_returns_the_newest_page_for_a_none_cursor() {
      let db = store::open_test().await.unwrap();
      seed_history(&db).await;

      let page = list_page(&db, None, 2).await.unwrap();

      let keys = page.iter().map(Notification::dedup_key).collect::<Vec<_>>();
      assert_eq!(keys, ["skill:0", "skill:1"]);
    }

    #[tokio::test]
    async fn it_excludes_watermark_rows() {
      let db = store::open_test().await.unwrap();
      watermark(
        &db,
        &NotificationOwner::Character(42),
        NotificationKind::Skill,
        &["skill:wm".to_owned()],
      )
      .await
      .unwrap();
      seed_surfaced(&db, "skill:1", "2026-06-20T00:00:00+00:00").await;

      let page = list_page(&db, None, 50).await.unwrap();

      assert_eq!(page.len(), 1);
      assert_eq!(page[0].dedup_key(), "skill:1");
    }

    #[tokio::test]
    async fn it_walks_the_full_history_with_no_duplicates_or_gaps() {
      let db = store::open_test().await.unwrap();
      seed_history(&db).await;

      let mut seen = Vec::new();
      let mut cursor = None;
      loop {
        let page = list_page(&db, cursor.as_ref(), 2).await.unwrap();
        if page.is_empty() {
          break;
        }
        cursor = HistoryCursor::from_page(&page);
        seen.extend(page.into_iter().map(|n| n.dedup_key().clone()));
      }

      assert_eq!(seen, ["skill:0", "skill:1", "skill:2", "skill:3", "skill:4"]);
    }

    #[tokio::test]
    async fn it_keeps_stable_ordering_when_a_row_is_inserted_mid_paging() {
      let db = store::open_test().await.unwrap();
      seed_history(&db).await;

      let first = list_page(&db, None, 2).await.unwrap();
      let cursor = HistoryCursor::from_page(&first).unwrap();
      // A row newer than the cursor arrives between fetches; it sorts ahead of the cursor and must
      // not reappear in the next (older) page.
      seed_surfaced(&db, "skill:new", "2026-06-21T00:00:00+00:00").await;

      let second = list_page(&db, Some(&cursor), 2).await.unwrap();

      let keys = second.iter().map(|n| n.dedup_key().clone()).collect::<Vec<_>>();
      assert_eq!(keys, ["skill:2", "skill:3"]);
      assert!(
        !keys.contains(&"skill:new".to_owned()),
        "a row inserted after the cursor never appears in an older page"
      );
    }

    // Two rows sharing a created_at must still page cleanly via the id tiebreak.
    #[tokio::test]
    async fn it_breaks_created_at_ties_by_id() {
      let db = store::open_test().await.unwrap();
      let ts = "2026-06-20T00:00:00+00:00";
      let first = seed_surfaced(&db, "skill:a", ts).await;
      let second = seed_surfaced(&db, "skill:b", ts).await;
      assert!(second > first, "the second insert takes a higher id");

      let page1 = list_page(&db, None, 1).await.unwrap();
      assert_eq!(page1[0].dedup_key(), "skill:b");
      let cursor = HistoryCursor::from_page(&page1).unwrap();
      let page2 = list_page(&db, Some(&cursor), 1).await.unwrap();

      assert_eq!(page2[0].dedup_key(), "skill:a");
    }
  }

  mod list_unread {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_only_surfaced_unread_rows_newest_first() {
      let db = store::open_test().await.unwrap();
      let read = seed_surfaced(&db, "skill:read", "2026-06-18T00:00:00+00:00").await;
      seed_surfaced(&db, "skill:unread-old", "2026-06-19T00:00:00+00:00").await;
      seed_surfaced(&db, "skill:unread-new", "2026-06-20T00:00:00+00:00").await;
      watermark(
        &db,
        &NotificationOwner::Character(42),
        NotificationKind::Skill,
        &["skill:wm".to_owned()],
      )
      .await
      .unwrap();
      mark_read(&db, read).await.unwrap();

      let unread = list_unread(&db, 50).await.unwrap();

      let keys = unread.iter().map(Notification::dedup_key).collect::<Vec<_>>();
      assert_eq!(keys, ["skill:unread-new", "skill:unread-old"]);
    }
  }

  mod migration {
    use pretty_assertions::assert_eq;

    use super::*;

    // open_test() runs every embedded migration, so the keyset index exists only if 0105 applied.
    #[tokio::test]
    async fn it_creates_the_keyset_index() {
      let db = store::open_test().await.unwrap();

      let name: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'idx_notifications_surfaced_keyset'",
      )
      .fetch_optional(db.reader())
      .await
      .unwrap();

      assert_eq!(name.as_deref(), Some("idx_notifications_surfaced_keyset"));
    }
  }
}
