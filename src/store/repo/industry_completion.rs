use chrono::Utc;

use crate::store::{Database, Error};

pub async fn insert_if_absent(
  db: &Database,
  character_id: i64,
  job_id: i64,
  activity_id: i64,
  product_type_id: Option<i64>,
  runs: i64,
  completed_at: &str,
) -> Result<bool, Error> {
  let now = Utc::now().to_rfc3339();
  let result = sqlx::query(
    "INSERT INTO industry_completion \
      (character_id, job_id, activity_id, product_type_id, runs, completed_at, created_at) \
    SELECT ?, ?, ?, ?, ?, ?, ? WHERE ? IN (SELECT id FROM owned_characters) \
    ON CONFLICT (character_id, job_id) DO NOTHING",
  )
  .bind(character_id)
  .bind(job_id)
  .bind(activity_id)
  .bind(product_type_id)
  .bind(runs)
  .bind(completed_at)
  .bind(&now)
  .bind(character_id)
  .execute(db.writer())
  .await?;
  Ok(result.rows_affected() > 0)
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn for_character(
  db: &Database,
  character_id: i64,
) -> Result<Vec<(i64, i64, Option<i64>, i64, String)>, Error> {
  let rows = sqlx::query_as::<_, (i64, i64, Option<i64>, i64, String)>(
    "SELECT job_id, activity_id, product_type_id, runs, completed_at \
    FROM industry_completion WHERE character_id = ? ORDER BY completed_at, job_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
    repo::{character, infra},
  };

  const CHARACTER: i64 = 95_465_499;

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

  async fn own(db: &Database, id: i64) {
    infra::upsert(db, id, OwnerType::Character, "tok", "rt", 4_102_444_800, None, None)
      .await
      .unwrap();
  }

  mod insert_if_absent {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_records_a_delivered_job_once() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      own(&db, CHARACTER).await;

      let first = insert_if_absent(&db, CHARACTER, 42, 1, Some(587), 3, "2026-07-09T00:00:00+00:00")
        .await
        .unwrap();
      let second = insert_if_absent(&db, CHARACTER, 42, 1, Some(587), 3, "2026-07-09T12:00:00+00:00")
        .await
        .unwrap();

      assert!(first, "the first observation appends a row");
      assert!(!second, "a re-observed job appends no duplicate");
      let rows = for_character(&db, CHARACTER).await.unwrap();
      assert_eq!(
        rows,
        vec![(42, 1, Some(587), 3, "2026-07-09T00:00:00+00:00".to_owned())]
      );
    }

    #[tokio::test]
    async fn it_keys_rows_independently_per_job() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      own(&db, CHARACTER).await;

      insert_if_absent(&db, CHARACTER, 42, 1, Some(587), 1, "2026-07-08T00:00:00+00:00")
        .await
        .unwrap();
      insert_if_absent(&db, CHARACTER, 43, 9, None, 5, "2026-07-09T00:00:00+00:00")
        .await
        .unwrap();

      assert_eq!(for_character(&db, CHARACTER).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn it_ignores_a_non_owned_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;

      let wrote = insert_if_absent(&db, CHARACTER, 42, 1, Some(587), 1, "2026-07-09T00:00:00+00:00")
        .await
        .unwrap();

      assert!(!wrote, "only owned characters accrue completion history");
      assert!(for_character(&db, CHARACTER).await.unwrap().is_empty());
    }
  }
}
