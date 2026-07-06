use chrono::Utc;
use sqlx::{QueryBuilder, Sqlite};

use crate::store::{Database, Error, model::KillmailReport};

const SELECT_COLUMNS: &str =
  "SELECT character_id, created_at, different, happened, killmail_id, outcome, takeaway, updated_at ";

#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReportInput {
  pub different: Option<String>,
  pub happened: String,
  pub outcome: String,
  pub takeaway: Option<String>,
}

#[allow(dead_code)]
pub async fn get(db: &Database, character_id: i64, killmail_id: i64) -> Result<Option<KillmailReport>, Error> {
  let row = sqlx::query_as::<_, KillmailReport>(
    "SELECT character_id, created_at, different, happened, killmail_id, outcome, takeaway, updated_at \
    FROM killmail_report WHERE character_id = ? AND killmail_id = ?",
  )
  .bind(character_id)
  .bind(killmail_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn list_for_day(db: &Database, character_ids: &[i64], day: &str) -> Result<Vec<KillmailReport>, Error> {
  if character_ids.is_empty() {
    return Ok(Vec::new());
  }
  let mut builder = QueryBuilder::<Sqlite>::new(
    "SELECT r.character_id, r.created_at, r.different, r.happened, r.killmail_id, r.outcome, r.takeaway, r.updated_at \
    FROM killmail_report r \
    JOIN character_killmails k ON k.character_id = r.character_id AND k.killmail_id = r.killmail_id \
    WHERE k.character_id IN (",
  );
  let mut separated = builder.separated(", ");
  for character_id in character_ids {
    separated.push_bind(*character_id);
  }
  separated.push_unseparated(") ");
  builder
    .push("AND substr(k.kill_time, 1, 10) = ")
    .push_bind(day.to_owned());
  builder.push(" ORDER BY k.kill_time, r.killmail_id");
  let rows = builder.build_query_as::<KillmailReport>().fetch_all(&db.0).await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn list_for_killmails(db: &Database, killmail_ids: &[i64]) -> Result<Vec<KillmailReport>, Error> {
  if killmail_ids.is_empty() {
    return Ok(Vec::new());
  }
  let mut builder = QueryBuilder::<Sqlite>::new(SELECT_COLUMNS);
  builder.push("FROM killmail_report WHERE killmail_id IN (");
  let mut separated = builder.separated(", ");
  for killmail_id in killmail_ids {
    separated.push_bind(*killmail_id);
  }
  separated.push_unseparated(") ");
  builder.push("ORDER BY character_id, killmail_id");
  let rows = builder.build_query_as::<KillmailReport>().fetch_all(&db.0).await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn upsert(db: &Database, character_id: i64, killmail_id: i64, input: &ReportInput) -> Result<(), Error> {
  let now = Utc::now().to_rfc3339();
  sqlx::query(
    "INSERT INTO killmail_report \
      (character_id, killmail_id, outcome, happened, different, takeaway, created_at, updated_at) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT (character_id, killmail_id) DO UPDATE SET \
      outcome = excluded.outcome, happened = excluded.happened, different = excluded.different, \
      takeaway = excluded.takeaway, updated_at = excluded.updated_at",
  )
  .bind(character_id)
  .bind(killmail_id)
  .bind(&input.outcome)
  .bind(&input.happened)
  .bind(&input.different)
  .bind(&input.takeaway)
  .bind(&now)
  .bind(&now)
  .execute(db.writer())
  .await?;
  Ok(())
}

#[cfg(test)]
mod tests {
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

  async fn seed_killmail(db: &Database, character_id: i64, killmail_id: i64, kill_time: &str) {
    sqlx::query(
      "INSERT INTO character_killmails \
        (character_id, killmail_id, kill_hash, is_kill, ship_type_id, system_id, kill_time, synced_at) \
        VALUES (?, ?, ?, 0, 587, 30000142, ?, ?)",
    )
    .bind(character_id)
    .bind(killmail_id)
    .bind(format!("hash-{killmail_id}"))
    .bind(kill_time)
    .bind("2026-07-06T00:00:00Z")
    .execute(db.writer())
    .await
    .unwrap();
  }

  fn input(outcome: &str, happened: &str, takeaway: Option<&str>) -> ReportInput {
    ReportInput {
      different: None,
      happened: happened.to_owned(),
      outcome: outcome.to_owned(),
      takeaway: takeaway.map(str::to_owned),
    }
  }

  mod upsert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_replaces_the_row_in_place_on_a_second_write() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::upsert(&db, 42, 1, &input("costly", "Lost the tackle.", Some("Warp sooner.")))
        .await
        .unwrap();
      super::upsert(
        &db,
        42,
        1,
        &input("learning", "Reshipped and won.", Some("Bring a scout.")),
      )
      .await
      .unwrap();

      let reports = super::list_for_killmails(&db, &[1]).await.unwrap();
      assert_eq!(reports.len(), 1);
      assert_eq!(reports[0].outcome(), "learning");
      assert_eq!(reports[0].happened(), "Reshipped and won.");
      assert_eq!(reports[0].takeaway().as_deref(), Some("Bring a scout."));
    }
  }

  mod list_for_day {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_only_reports_for_kills_on_that_day() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      seed_killmail(&db, 42, 1, "2026-07-06T11:00:00Z").await;
      seed_killmail(&db, 42, 2, "2026-07-05T23:00:00Z").await;
      seed_killmail(&db, 43, 3, "2026-07-06T18:00:00Z").await;

      super::upsert(&db, 42, 1, &input("clean", "On day.", None))
        .await
        .unwrap();
      super::upsert(&db, 42, 2, &input("costly", "Day before.", None))
        .await
        .unwrap();
      super::upsert(&db, 43, 3, &input("clean", "Other pilot, same day.", None))
        .await
        .unwrap();

      let reports = super::list_for_day(&db, &[42, 43], "2026-07-06").await.unwrap();

      assert_eq!(reports.iter().map(|r| r.killmail_id()).collect::<Vec<_>>(), [1, 3]);
    }
  }
}
