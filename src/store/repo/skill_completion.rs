use chrono::Utc;

use crate::store::{Database, Error, model::SkillCompletion};

pub async fn delete(db: &Database, id: i64) -> Result<u64, Error> {
  let result = sqlx::query("DELETE FROM skill_completion WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn for_day(db: &Database, date: &str, character_ids: &[i64]) -> Result<Vec<SkillCompletion>, Error> {
  if character_ids.is_empty() {
    return Ok(Vec::new());
  }

  let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
    "SELECT character_id, completed_at, created_at, id, level, skill_id, updated_at, verified FROM skill_completion",
  );
  builder.push(" WHERE substr(completed_at, 1, 10) = ");
  builder.push_bind(date.to_owned());
  builder.push(" AND character_id IN (");
  let mut separated = builder.separated(", ");
  for character_id in character_ids {
    separated.push_bind(*character_id);
  }
  builder.push(") ORDER BY character_id, completed_at, skill_id, level");

  let rows = builder.build_query_as::<SkillCompletion>().fetch_all(&db.0).await?;
  Ok(rows)
}

pub async fn insert_if_absent(
  db: &Database,
  character_id: i64,
  skill_id: i64,
  level: i64,
  completed_at: &str,
) -> Result<bool, Error> {
  let now = Utc::now().to_rfc3339();
  let result = sqlx::query(
    "INSERT INTO skill_completion \
      (character_id, skill_id, level, completed_at, verified, created_at, updated_at) \
    VALUES (?, ?, ?, ?, 0, ?, ?) \
    ON CONFLICT (character_id, skill_id, level) DO NOTHING",
  )
  .bind(character_id)
  .bind(skill_id)
  .bind(level)
  .bind(completed_at)
  .bind(&now)
  .bind(&now)
  .execute(db.writer())
  .await?;
  Ok(result.rows_affected() > 0)
}

pub async fn mark_verified(db: &Database, id: i64) -> Result<(), Error> {
  let now = Utc::now().to_rfc3339();
  sqlx::query("UPDATE skill_completion SET verified = 1, updated_at = ? WHERE id = ?")
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn unverified(db: &Database, character_id: i64) -> Result<Vec<SkillCompletion>, Error> {
  let rows = sqlx::query_as::<_, SkillCompletion>(
    "SELECT character_id, completed_at, created_at, id, level, skill_id, updated_at, verified \
    FROM skill_completion WHERE character_id = ? AND verified = 0 ORDER BY completed_at, skill_id, level",
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
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  const CHARACTER: i64 = 95_465_499;
  const OTHER_CHARACTER: i64 = 90_000_002;

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

  mod insert_if_absent {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_idempotent_for_a_repeated_completion() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;

      let first = insert_if_absent(&db, CHARACTER, 3300, 5, "2026-07-06T12:00:00+00:00")
        .await
        .unwrap();
      let second = insert_if_absent(&db, CHARACTER, 3300, 5, "2026-07-06T18:00:00+00:00")
        .await
        .unwrap();

      assert!(first, "the first detection inserts a row");
      assert!(!second, "a re-detected completion does not insert a duplicate");
      assert_eq!(unverified(&db, CHARACTER).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_keys_rows_independently_per_level() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;

      insert_if_absent(&db, CHARACTER, 3300, 4, "2026-07-05T09:00:00+00:00")
        .await
        .unwrap();
      insert_if_absent(&db, CHARACTER, 3300, 5, "2026-07-06T09:00:00+00:00")
        .await
        .unwrap();

      assert_eq!(unverified(&db, CHARACTER).await.unwrap().len(), 2);
    }
  }

  mod for_day {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_lists_completions_across_multiple_characters_for_the_day() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_character(&db, OTHER_CHARACTER).await;
      insert_if_absent(&db, CHARACTER, 3300, 5, "2026-07-06T08:00:00+00:00")
        .await
        .unwrap();
      insert_if_absent(&db, OTHER_CHARACTER, 3301, 4, "2026-07-06T20:00:00+00:00")
        .await
        .unwrap();
      insert_if_absent(&db, CHARACTER, 3302, 3, "2026-07-05T23:59:00+00:00")
        .await
        .unwrap();

      let rows = for_day(&db, "2026-07-06", &[CHARACTER, OTHER_CHARACTER]).await.unwrap();

      assert_eq!(rows.len(), 2);
      assert_eq!(rows[0].character_id, OTHER_CHARACTER);
      assert_eq!(rows[1].character_id, CHARACTER);
    }

    #[tokio::test]
    async fn it_is_empty_for_an_empty_character_set() {
      let db = store::open_test().await.unwrap();

      let rows = for_day(&db, "2026-07-06", &[]).await.unwrap();

      assert!(rows.is_empty());
    }
  }

  mod reconcile {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_round_trips_verify_and_delete() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      insert_if_absent(&db, CHARACTER, 3300, 5, "2026-07-06T08:00:00+00:00")
        .await
        .unwrap();
      insert_if_absent(&db, CHARACTER, 3301, 4, "2026-07-06T09:00:00+00:00")
        .await
        .unwrap();

      let pending = unverified(&db, CHARACTER).await.unwrap();
      assert_eq!(pending.len(), 2);

      mark_verified(&db, pending[0].id).await.unwrap();
      let after_verify = unverified(&db, CHARACTER).await.unwrap();
      assert_eq!(after_verify.len(), 1, "a verified row drops out of the unverified set");
      assert_eq!(after_verify[0].id, pending[1].id);

      let deleted = delete(&db, pending[1].id).await.unwrap();
      assert_eq!(deleted, 1);
      assert!(
        unverified(&db, CHARACTER).await.unwrap().is_empty(),
        "the reconcile correction removes the contradicted completion"
      );
    }
  }
}
