use crate::store::{
  Database, Error,
  model::{MarketAlertKind, MarketAlertState},
};

pub async fn count_alerted(db: &Database, kind: MarketAlertKind) -> Result<i64, Error> {
  let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM market_alert_state WHERE kind = ? AND alerted = 1")
    .bind(kind.as_str())
    .fetch_one(&db.0)
    .await?;
  Ok(count)
}

#[allow(dead_code)]
pub async fn read(
  db: &Database,
  kind: MarketAlertKind,
  character_id: i64,
  subject_id: i64,
) -> Result<Option<MarketAlertState>, Error> {
  let row = sqlx::query_as::<_, MarketAlertState>(
    "SELECT alerted, character_id, created_at, kind, marker, subject_id, updated_at \
    FROM market_alert_state WHERE kind = ? AND character_id = ? AND subject_id = ?",
  )
  .bind(kind.as_str())
  .bind(character_id)
  .bind(subject_id)
  .fetch_optional(db.reader())
  .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn mark(
  db: &Database,
  kind: MarketAlertKind,
  character_id: i64,
  subject_id: i64,
  marker: &str,
) -> Result<MarketAlertState, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, MarketAlertState>(
    "INSERT INTO market_alert_state (kind, character_id, subject_id, alerted, marker, created_at, updated_at) \
    VALUES (?, ?, ?, 1, ?, ?, ?) \
    ON CONFLICT (kind, character_id, subject_id) DO UPDATE SET \
      marker = CASE WHEN market_alert_state.alerted = 0 THEN excluded.marker ELSE market_alert_state.marker END, \
      updated_at = CASE WHEN market_alert_state.alerted = 0 THEN excluded.updated_at ELSE market_alert_state.updated_at END, \
      alerted = 1 \
    RETURNING alerted, character_id, created_at, kind, marker, subject_id, updated_at",
  )
  .bind(kind.as_str())
  .bind(character_id)
  .bind(subject_id)
  .bind(marker)
  .bind(&now)
  .bind(&now)
  .fetch_one(db.writer())
  .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn clear(db: &Database, kind: MarketAlertKind, character_id: i64, subject_id: i64) -> Result<u64, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let result = sqlx::query(
    "UPDATE market_alert_state SET alerted = 0, updated_at = ? WHERE kind = ? AND character_id = ? AND subject_id = ?",
  )
  .bind(&now)
  .bind(kind.as_str())
  .bind(character_id)
  .bind(subject_id)
  .execute(db.writer())
  .await?;
  Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  const PILOT: i64 = 90_000_001;
  const OTHER: i64 = 90_000_002;
  const ORDER: i64 = 6_001_002_003;

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 98_000_001;
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

  mod read {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_before_any_mark() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;

      assert_eq!(read(&db, MarketAlertKind::Outbid, PILOT, ORDER).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_returns_the_alerted_state_and_marker_after_a_mark() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;
      mark(&db, MarketAlertKind::Outbid, PILOT, ORDER, "4999.0")
        .await
        .unwrap();

      let state = read(&db, MarketAlertKind::Outbid, PILOT, ORDER).await.unwrap().unwrap();

      assert!(state.alerted);
      assert_eq!(state.character_id, PILOT);
      assert_eq!(state.kind, "outbid");
      assert_eq!(state.subject_id, ORDER);
      assert_eq!(state.marker, "4999.0");
    }
  }

  mod mark {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_freezes_the_marker_and_dedup_key_while_alerted() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;

      let first = mark(&db, MarketAlertKind::Outbid, PILOT, ORDER, "4999.0")
        .await
        .unwrap();
      let again = mark(&db, MarketAlertKind::Outbid, PILOT, ORDER, "4500.0")
        .await
        .unwrap();

      assert_eq!(again.marker, "4999.0");
      assert_eq!(again.dedup_key(), first.dedup_key());
      assert!(again.alerted);
    }

    #[tokio::test]
    async fn it_is_scoped_by_kind_character_and_subject() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;
      seed_character(&db, OTHER).await;

      mark(&db, MarketAlertKind::Outbid, PILOT, ORDER, "a").await.unwrap();
      mark(&db, MarketAlertKind::Target, PILOT, ORDER, "b").await.unwrap();
      mark(&db, MarketAlertKind::Outbid, OTHER, ORDER, "c").await.unwrap();

      assert_eq!(
        read(&db, MarketAlertKind::Outbid, PILOT, ORDER)
          .await
          .unwrap()
          .unwrap()
          .marker,
        "a"
      );
      assert_eq!(
        read(&db, MarketAlertKind::Target, PILOT, ORDER)
          .await
          .unwrap()
          .unwrap()
          .marker,
        "b"
      );
      assert_eq!(
        read(&db, MarketAlertKind::Outbid, OTHER, ORDER)
          .await
          .unwrap()
          .unwrap()
          .marker,
        "c"
      );
    }
  }

  mod clear {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_resets_alerted_but_keeps_the_last_marker() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;
      mark(&db, MarketAlertKind::Outbid, PILOT, ORDER, "4999.0")
        .await
        .unwrap();

      let affected = clear(&db, MarketAlertKind::Outbid, PILOT, ORDER).await.unwrap();

      assert_eq!(affected, 1);
      let state = read(&db, MarketAlertKind::Outbid, PILOT, ORDER).await.unwrap().unwrap();
      assert!(!state.alerted);
      assert_eq!(state.marker, "4999.0");
    }

    #[tokio::test]
    async fn it_lets_the_next_crossing_mark_with_a_new_marker_and_distinct_dedup_key() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;
      let first = mark(&db, MarketAlertKind::Outbid, PILOT, ORDER, "4999.0")
        .await
        .unwrap();
      clear(&db, MarketAlertKind::Outbid, PILOT, ORDER).await.unwrap();

      let second = mark(&db, MarketAlertKind::Outbid, PILOT, ORDER, "4200.0")
        .await
        .unwrap();

      assert!(second.alerted);
      assert_eq!(second.marker, "4200.0");
      assert_ne!(second.dedup_key(), first.dedup_key());
    }

    #[tokio::test]
    async fn it_is_a_no_op_when_there_is_no_row() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;

      let affected = clear(&db, MarketAlertKind::Outbid, PILOT, ORDER).await.unwrap();

      assert_eq!(affected, 0);
    }
  }

  mod cascade {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_drops_alert_state_when_the_character_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;
      mark(&db, MarketAlertKind::Outbid, PILOT, ORDER, "4999.0")
        .await
        .unwrap();

      character::delete(&db, PILOT).await.unwrap();

      assert_eq!(read(&db, MarketAlertKind::Outbid, PILOT, ORDER).await.unwrap(), None);
    }
  }
}
