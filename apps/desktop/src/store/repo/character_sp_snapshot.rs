use crate::store::{Database, Error};

pub async fn record_today(db: &Database, character_id: i64, date: &str) -> Result<bool, Error> {
  let total_sp: Option<i64> = sqlx::query_scalar("SELECT total_sp FROM character_state WHERE character_id = ?")
    .bind(character_id)
    .fetch_optional(&db.0)
    .await?
    .flatten();
  let Some(total_sp) = total_sp else {
    return Ok(false);
  };
  let unallocated_sp: i64 =
    sqlx::query_scalar("SELECT unallocated_sp FROM character_attributes WHERE character_id = ?")
      .bind(character_id)
      .fetch_optional(&db.0)
      .await?
      .unwrap_or(0);
  upsert(db, character_id, date, total_sp, unallocated_sp).await?;
  Ok(true)
}

pub async fn upsert(
  db: &Database,
  character_id: i64,
  date: &str,
  total_sp: i64,
  unallocated_sp: i64,
) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO character_sp_snapshot (character_id, date, total_sp, unallocated_sp) \
    SELECT ?, ?, ?, ? WHERE ? IN (SELECT id FROM owned_characters) \
    ON CONFLICT (character_id, date) DO UPDATE SET total_sp = excluded.total_sp, unallocated_sp = excluded.unallocated_sp",
  )
  .bind(character_id)
  .bind(date)
  .bind(total_sp)
  .bind(unallocated_sp)
  .bind(character_id)
  .execute(db.writer())
  .await?;
  Ok(())
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn for_character(db: &Database, character_id: i64) -> Result<Vec<(String, i64, i64)>, Error> {
  let rows = sqlx::query_as::<_, (String, i64, i64)>(
    "SELECT date, total_sp, unallocated_sp FROM character_sp_snapshot WHERE character_id = ? ORDER BY date",
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

  async fn insert_skill(db: &Database, character_id: i64, skill_id: i64, points: i64) {
    sqlx::query(
      "INSERT INTO character_skills \
        (character_id, skill_id, active_skill_level, skillpoints_in_skill, trained_skill_level) \
      VALUES (?, ?, 5, ?, 5)",
    )
    .bind(character_id)
    .bind(skill_id)
    .bind(points)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn set_unallocated(db: &Database, character_id: i64, unallocated: i64) {
    sqlx::query(
      "INSERT INTO character_attributes \
        (character_id, charisma, intelligence, memory, perception, willpower, bonus_remaps, unallocated_sp) \
      VALUES (?, 20, 20, 20, 20, 20, 2, ?)",
    )
    .bind(character_id)
    .bind(unallocated)
    .execute(db.writer())
    .await
    .unwrap();
  }

  mod record_today {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_snapshots_summed_skillpoints_and_unallocated_sp() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      own(&db, CHARACTER).await;
      insert_skill(&db, CHARACTER, 3300, 256_000).await;
      insert_skill(&db, CHARACTER, 3301, 44_000).await;
      set_unallocated(&db, CHARACTER, 5_000).await;

      let wrote = record_today(&db, CHARACTER, "2026-07-09").await.unwrap();

      assert!(wrote);
      let rows = for_character(&db, CHARACTER).await.unwrap();
      assert_eq!(rows, vec![("2026-07-09".to_owned(), 300_000, 5_000)]);
    }

    #[tokio::test]
    async fn it_defaults_unallocated_to_zero_without_attributes() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      own(&db, CHARACTER).await;
      insert_skill(&db, CHARACTER, 3300, 100_000).await;

      record_today(&db, CHARACTER, "2026-07-09").await.unwrap();

      let rows = for_character(&db, CHARACTER).await.unwrap();
      assert_eq!(rows, vec![("2026-07-09".to_owned(), 100_000, 0)]);
    }

    #[tokio::test]
    async fn it_is_idempotent_and_refreshes_the_days_row() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      own(&db, CHARACTER).await;
      insert_skill(&db, CHARACTER, 3300, 100_000).await;

      record_today(&db, CHARACTER, "2026-07-09").await.unwrap();
      insert_skill(&db, CHARACTER, 3301, 50_000).await;
      record_today(&db, CHARACTER, "2026-07-09").await.unwrap();

      let rows = for_character(&db, CHARACTER).await.unwrap();
      assert_eq!(rows.len(), 1, "re-running the same day overwrites rather than appends");
      assert_eq!(rows[0], ("2026-07-09".to_owned(), 150_000, 0));
    }

    #[tokio::test]
    async fn it_skips_a_character_with_no_synced_skills() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      own(&db, CHARACTER).await;

      let wrote = record_today(&db, CHARACTER, "2026-07-09").await.unwrap();

      assert!(!wrote, "a character with no skill points has nothing to snapshot");
      assert!(for_character(&db, CHARACTER).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_does_not_snapshot_a_non_owned_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      insert_skill(&db, CHARACTER, 3300, 100_000).await;

      record_today(&db, CHARACTER, "2026-07-09").await.unwrap();

      assert!(
        for_character(&db, CHARACTER).await.unwrap().is_empty(),
        "only owned characters are snapshotted"
      );
    }
  }
}
