//! Auto-purges mail that has sat in Trash for 30+ days by reusing the manual
//! permanent-delete path (`mail.delete` outbox + optimistic local purge) — no
//! separate ESI call is made.

use chrono::{DateTime, Duration, Utc};

use crate::store::{
  Database,
  model::OwnerType,
  repo::{infra, mail},
};

const TRASH_RETENTION_DAYS: i64 = 30;

pub fn retention_cutoff(now: DateTime<Utc>) -> String {
  (now - Duration::days(TRASH_RETENTION_DAYS))
    // `Z` is a literal suffix, not a chrono format specifier; the result is
    // always a UTC timestamp string suitable for direct SQL comparison.
    .format("%Y-%m-%dT%H:%M:%SZ")
    .to_string()
}

pub async fn purge_expired_trash(db: Database, now: DateTime<Utc>) -> Vec<(i64, i64)> {
  let cutoff = retention_cutoff(now);
  let expired = match mail::expired_trashed_mails(&db, &cutoff).await {
    Ok(rows) => rows,
    Err(error) => {
      tracing::warn!(%error, "trash purge-scheduler: querying expired trash failed");
      return Vec::new();
    }
  };

  let mut purged = Vec::with_capacity(expired.len());
  for assignment in &expired {
    let character_id = assignment.character_id();
    let mail_id = assignment.mail_id();
    if enqueue_permanent_delete(&db, character_id, mail_id).await {
      purged.push((character_id, mail_id));
    }
  }

  if !purged.is_empty() {
    tracing::debug!(count = purged.len(), "trash purge-scheduler: purged expired trash");
  }
  purged
}

async fn enqueue_permanent_delete(db: &Database, character_id: i64, mail_id: i64) -> bool {
  // Snapshot before purge: the local rows must exist when captured for the
  // outbox payload; purging first would leave nothing to snapshot.
  let Ok(Some(snapshot)) = mail::snapshot_mail(db, character_id, mail_id).await else {
    return false;
  };
  if mail::purge_mail(db, character_id, mail_id).await.is_err() {
    return false;
  }

  let Ok(payload) = serde_json::to_string(&snapshot) else {
    return false;
  };
  // Dedupe key format must match the outbox consumer's expectation.
  let dedupe = format!("delete_mail:{mail_id}");
  infra::append(
    db,
    OwnerType::Character,
    character_id,
    "mail.delete",
    &payload,
    Some(&dedupe),
  )
  .await
  .is_ok()
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone as _;

  use super::*;
  use crate::store::{
    self, Database,
    model::{Alliance, Bloodline, Character, CharacterMail, CharacterMailBody, Corporation, Gender, Race},
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

  async fn store_mail(db: &Database, character_id: i64, mail_id: i64) {
    let header = CharacterMail {
      character_id,
      from_id: 95_000_001,
      from_name: "Sender".to_owned(),
      mail_id,
      subject: Some("Subject".to_owned()),
      timestamp: "2026-01-01T10:00:00Z".to_owned(),
      ..Default::default()
    };
    let body = CharacterMailBody {
      body: "<p>hi</p>".to_owned(),
      character_id,
      mail_id,
    };
    mail::upsert_complete(db, &header, &body, &[]).await.unwrap();
  }

  async fn trash_at(db: &Database, character_id: i64, mail_id: i64, assigned_at: &str) {
    store_mail(db, character_id, mail_id).await;
    mail::assign_folder(db, character_id, mail_id, "trash", None, false, assigned_at)
      .await
      .unwrap();
  }

  fn at(year: i32, month: u32, day: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap()
  }

  mod purge_expired_trash {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn outbox_delete_count(db: &Database) -> i64 {
      sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox WHERE kind = 'mail.delete'")
        .fetch_one(&db.0)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn it_purges_trash_older_than_thirty_days() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      trash_at(&db, 42, 7, "2026-05-01T00:00:00Z").await;

      let purged = purge_expired_trash(db.clone(), at(2026, 6, 15)).await;

      assert_eq!(purged, [(42, 7)]);
      assert!(mail::snapshot_mail(&db, 42, 7).await.unwrap().is_none());
      assert_eq!(outbox_delete_count(&db).await, 1);
    }

    #[tokio::test]
    async fn it_leaves_recently_trashed_mail_alone() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      trash_at(&db, 42, 7, "2026-06-10T00:00:00Z").await;

      let purged = purge_expired_trash(db.clone(), at(2026, 6, 15)).await;

      assert!(purged.is_empty());
      assert!(mail::snapshot_mail(&db, 42, 7).await.unwrap().is_some());
      assert_eq!(outbox_delete_count(&db).await, 0);
    }

    #[tokio::test]
    async fn it_purges_only_the_aged_rows_in_a_mixed_box() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      trash_at(&db, 42, 7, "2026-05-01T00:00:00Z").await;
      trash_at(&db, 42, 8, "2026-06-14T00:00:00Z").await;

      let purged = purge_expired_trash(db.clone(), at(2026, 6, 15)).await;

      assert_eq!(purged, [(42, 7)]);
      assert!(mail::snapshot_mail(&db, 42, 7).await.unwrap().is_none());
      assert!(mail::snapshot_mail(&db, 42, 8).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_backdates_a_long_dormant_trash_on_the_next_launch() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      trash_at(&db, 42, 7, "2025-01-01T00:00:00Z").await;

      let purged = purge_expired_trash(db.clone(), at(2026, 6, 15)).await;

      assert_eq!(purged, [(42, 7)]);
      assert_eq!(outbox_delete_count(&db).await, 1);
    }
  }

  mod retention_cutoff {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_subtracts_thirty_days_from_now() {
      let now = Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap();

      assert_eq!(retention_cutoff(now), "2026-05-16T12:00:00Z");
    }
  }
}
