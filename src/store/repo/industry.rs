use std::collections::HashSet;

use sqlx::{QueryBuilder, Sqlite};

use crate::store::{
  Database, Error,
  model::{CharacterIndustryJob, CorporationIndustryJob},
  repo::org,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AllIndustryJobs {
  pub character_jobs: Vec<CharacterIndustryJob>,
  pub corporation_jobs: Vec<CorporationIndustryJob>,
}

const INDUSTRY_WRITE_BATCH_SIZE: usize = 500;

pub async fn list_all(db: &Database) -> Result<AllIndustryJobs, Error> {
  let character_jobs = list_all_character(db).await?;
  let corporation_jobs = list_all_corporation(db).await?;
  Ok(AllIndustryJobs {
    character_jobs,
    corporation_jobs,
  })
}

pub async fn list_for_character(db: &Database, character_id: i64) -> Result<Vec<CharacterIndustryJob>, Error> {
  let rows = sqlx::query_as::<_, CharacterIndustryJob>(
    "SELECT activity_id, blueprint_id, blueprint_location_id, blueprint_type_id, character_id, completed_character_id, \
    completed_date, cost, duration, end_date, facility_id, installer_id, job_id, licensed_runs, output_location_id, \
    pause_date, probability, product_type_id, runs, start_date, station_id, status, successful_runs \
    FROM character_industry_jobs WHERE character_id = ? ORDER BY end_date, job_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn list_for_corporation(db: &Database, corporation_id: i64) -> Result<Vec<CorporationIndustryJob>, Error> {
  if !org::corp_is_authorized(db, corporation_id).await? {
    return Ok(Vec::new());
  }
  let rows = sqlx::query_as::<_, CorporationIndustryJob>(
    "SELECT activity_id, blueprint_id, blueprint_location_id, blueprint_type_id, completed_character_id, \
    completed_date, corporation_id, cost, duration, end_date, facility_id, installer_id, job_id, licensed_runs, \
    output_location_id, pause_date, probability, product_type_id, runs, start_date, station_id, status, \
    successful_runs FROM corporation_industry_jobs WHERE corporation_id = ? ORDER BY end_date, job_id",
  )
  .bind(corporation_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn replace_for_character(
  db: &Database,
  character_id: i64,
  jobs: &[CharacterIndustryJob],
) -> Result<(), Error> {
  replace_for_character_batched(db, character_id, jobs, INDUSTRY_WRITE_BATCH_SIZE).await
}

pub async fn replace_for_corporation(
  db: &Database,
  corporation_id: i64,
  jobs: &[CorporationIndustryJob],
) -> Result<(), Error> {
  replace_for_corporation_batched(db, corporation_id, jobs, INDUSTRY_WRITE_BATCH_SIZE).await
}

async fn delete_character_jobs(db: &Database, character_id: i64, job_ids: &[i64]) -> Result<(), Error> {
  if job_ids.is_empty() {
    return Ok(());
  }
  let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM character_industry_jobs WHERE character_id = ");
  builder.push_bind(character_id);
  builder.push(" AND job_id IN (");
  let mut separated = builder.separated(", ");
  for id in job_ids {
    separated.push_bind(*id);
  }
  builder.push(")");
  builder.build().execute(&db.0).await?;
  Ok(())
}

async fn delete_corporation_jobs(db: &Database, corporation_id: i64, job_ids: &[i64]) -> Result<(), Error> {
  if job_ids.is_empty() {
    return Ok(());
  }
  let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM corporation_industry_jobs WHERE corporation_id = ");
  builder.push_bind(corporation_id);
  builder.push(" AND job_id IN (");
  let mut separated = builder.separated(", ");
  for id in job_ids {
    separated.push_bind(*id);
  }
  builder.push(")");
  builder.build().execute(&db.0).await?;
  Ok(())
}

async fn insert_character_job(
  tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
  job: &CharacterIndustryJob,
) -> Result<(), Error> {
  sqlx::query(
    "INSERT OR REPLACE INTO character_industry_jobs \
      (job_id, character_id, activity_id, blueprint_id, blueprint_location_id, blueprint_type_id, \
      completed_character_id, completed_date, cost, duration, end_date, facility_id, installer_id, licensed_runs, \
      output_location_id, pause_date, probability, product_type_id, runs, start_date, station_id, status, \
      successful_runs) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(job.job_id())
  .bind(job.character_id())
  .bind(job.activity_id())
  .bind(job.blueprint_id())
  .bind(job.blueprint_location_id())
  .bind(job.blueprint_type_id())
  .bind(job.completed_character_id())
  .bind(job.completed_date())
  .bind(job.cost())
  .bind(job.duration())
  .bind(job.end_date())
  .bind(job.facility_id())
  .bind(job.installer_id())
  .bind(job.licensed_runs())
  .bind(job.output_location_id())
  .bind(job.pause_date())
  .bind(job.probability())
  .bind(job.product_type_id())
  .bind(job.runs())
  .bind(job.start_date())
  .bind(job.station_id())
  .bind(job.status())
  .bind(job.successful_runs())
  .execute(&mut **tx)
  .await?;
  Ok(())
}

async fn insert_corporation_job(
  tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
  job: &CorporationIndustryJob,
) -> Result<(), Error> {
  sqlx::query(
    "INSERT OR REPLACE INTO corporation_industry_jobs \
      (job_id, corporation_id, activity_id, blueprint_id, blueprint_location_id, blueprint_type_id, \
      completed_character_id, completed_date, cost, duration, end_date, facility_id, installer_id, licensed_runs, \
      output_location_id, pause_date, probability, product_type_id, runs, start_date, station_id, status, \
      successful_runs) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(job.job_id())
  .bind(job.corporation_id())
  .bind(job.activity_id())
  .bind(job.blueprint_id())
  .bind(job.blueprint_location_id())
  .bind(job.blueprint_type_id())
  .bind(job.completed_character_id())
  .bind(job.completed_date())
  .bind(job.cost())
  .bind(job.duration())
  .bind(job.end_date())
  .bind(job.facility_id())
  .bind(job.installer_id())
  .bind(job.licensed_runs())
  .bind(job.output_location_id())
  .bind(job.pause_date())
  .bind(job.probability())
  .bind(job.product_type_id())
  .bind(job.runs())
  .bind(job.start_date())
  .bind(job.station_id())
  .bind(job.status())
  .bind(job.successful_runs())
  .execute(&mut **tx)
  .await?;
  Ok(())
}

async fn list_all_character(db: &Database) -> Result<Vec<CharacterIndustryJob>, Error> {
  let rows = sqlx::query_as::<_, CharacterIndustryJob>(
    "SELECT activity_id, blueprint_id, blueprint_location_id, blueprint_type_id, character_id, completed_character_id, \
    completed_date, cost, duration, end_date, facility_id, installer_id, job_id, licensed_runs, output_location_id, \
    pause_date, probability, product_type_id, runs, start_date, station_id, status, successful_runs \
    FROM character_industry_jobs ORDER BY end_date, job_id",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

async fn list_all_corporation(db: &Database) -> Result<Vec<CorporationIndustryJob>, Error> {
  let rows = sqlx::query_as::<_, CorporationIndustryJob>(
    "SELECT activity_id, blueprint_id, blueprint_location_id, blueprint_type_id, completed_character_id, \
    completed_date, corporation_id, cost, duration, end_date, facility_id, installer_id, job_id, licensed_runs, \
    output_location_id, pause_date, probability, product_type_id, runs, start_date, station_id, status, \
    successful_runs FROM corporation_industry_jobs ORDER BY end_date, job_id",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

/// Reconciles a character's industry jobs to `jobs`, committing in batches rather than one atomic transaction.
///
/// Upserting the new set before pruning stale ids (instead of deleting all first) and committing each batch releases
/// SQLite's single write lock between batches so interactive writes can interleave. A concurrent reader may transiently
/// observe a superset (a stale row not yet pruned) but never a missing current row; the final state is identical to a
/// delete-all-then-insert-all replace.
async fn replace_for_character_batched(
  db: &Database,
  character_id: i64,
  jobs: &[CharacterIndustryJob],
  batch_size: usize,
) -> Result<(), Error> {
  let new_ids: HashSet<i64> = jobs.iter().map(CharacterIndustryJob::job_id).collect();
  let existing: Vec<i64> = sqlx::query_scalar("SELECT job_id FROM character_industry_jobs WHERE character_id = ?")
    .bind(character_id)
    .fetch_all(&db.0)
    .await?;
  let stale: Vec<i64> = existing.into_iter().filter(|id| !new_ids.contains(id)).collect();

  let batch_size = batch_size.max(1);
  for chunk in jobs.chunks(batch_size) {
    let mut tx = db.0.begin().await?;
    for job in chunk {
      insert_character_job(&mut tx, job).await?;
    }
    tx.commit().await?;
    tokio::task::yield_now().await;
  }
  for chunk in stale.chunks(batch_size) {
    delete_character_jobs(db, character_id, chunk).await?;
    tokio::task::yield_now().await;
  }
  Ok(())
}

async fn replace_for_corporation_batched(
  db: &Database,
  corporation_id: i64,
  jobs: &[CorporationIndustryJob],
  batch_size: usize,
) -> Result<(), Error> {
  let new_ids: HashSet<i64> = jobs.iter().map(CorporationIndustryJob::job_id).collect();
  let existing: Vec<i64> = sqlx::query_scalar("SELECT job_id FROM corporation_industry_jobs WHERE corporation_id = ?")
    .bind(corporation_id)
    .fetch_all(&db.0)
    .await?;
  let stale: Vec<i64> = existing.into_iter().filter(|id| !new_ids.contains(id)).collect();

  let batch_size = batch_size.max(1);
  for chunk in jobs.chunks(batch_size) {
    let mut tx = db.0.begin().await?;
    for job in chunk {
      insert_corporation_job(&mut tx, job).await?;
    }
    tx.commit().await?;
    tokio::task::yield_now().await;
  }
  for chunk in stale.chunks(batch_size) {
    delete_corporation_jobs(db, corporation_id, chunk).await?;
    tokio::task::yield_now().await;
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{Alliance, Bloodline, Character, Corporation, CorporationMemberRole, Gender, OwnerType, Race},
    repo::{character, infra},
  };

  const CHARACTER_ID: i64 = 42;
  const CORPORATION_ID: i64 = 90_000_001;
  const DIRECTOR_ID: i64 = 100;

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = CORPORATION_ID;
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

  async fn authorize_corporation(db: &Database) {
    infra::upsert(
      db,
      CORPORATION_ID,
      OwnerType::Corporation,
      "tok",
      "rt",
      4_102_444_800,
      Some(DIRECTOR_ID),
      None,
    )
    .await
    .unwrap();
    org::replace_for_corporation(
      db,
      CORPORATION_ID,
      &[CorporationMemberRole::from((
        CORPORATION_ID,
        DIRECTOR_ID,
        "Director".to_owned(),
      ))],
    )
    .await
    .unwrap();
  }

  fn character_job(character_id: i64, job_id: i64, end_date: &str) -> CharacterIndustryJob {
    CharacterIndustryJob {
      activity_id: 1,
      blueprint_id: 1_000_000_000_001,
      blueprint_location_id: 60_003_760,
      blueprint_type_id: 12_345,
      character_id,
      completed_character_id: None,
      completed_date: None,
      cost: Some(123.45),
      duration: 3600,
      end_date: end_date.to_owned(),
      facility_id: 60_003_760,
      installer_id: character_id,
      job_id,
      licensed_runs: Some(10),
      output_location_id: 60_003_760,
      pause_date: None,
      probability: None,
      product_type_id: Some(54_321),
      runs: 5,
      start_date: "2026-06-13T00:00:00Z".to_owned(),
      station_id: Some(60_003_760),
      status: "active".to_owned(),
      successful_runs: None,
    }
  }

  fn corporation_job(corporation_id: i64, job_id: i64, end_date: &str) -> CorporationIndustryJob {
    CorporationIndustryJob {
      activity_id: 8,
      blueprint_id: 1_000_000_000_002,
      blueprint_location_id: 60_003_760,
      blueprint_type_id: 22_345,
      completed_character_id: None,
      completed_date: None,
      corporation_id,
      cost: None,
      duration: 7200,
      end_date: end_date.to_owned(),
      facility_id: 60_003_760,
      installer_id: DIRECTOR_ID,
      job_id,
      licensed_runs: None,
      output_location_id: 60_003_760,
      pause_date: None,
      probability: Some(0.5),
      product_type_id: Some(64_321),
      runs: 1,
      start_date: "2026-06-13T00:00:00Z".to_owned(),
      station_id: None,
      status: "active".to_owned(),
      successful_runs: None,
    }
  }

  mod replace_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_the_full_job_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;

      super::replace_for_character(
        &db,
        CHARACTER_ID,
        &[
          character_job(CHARACTER_ID, 1, "2026-06-14T00:00:00Z"),
          character_job(CHARACTER_ID, 2, "2026-06-15T00:00:00Z"),
        ],
      )
      .await
      .unwrap();

      let jobs = super::list_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(
        jobs.iter().map(CharacterIndustryJob::job_id).collect::<Vec<_>>(),
        [1, 2]
      );
    }

    #[tokio::test]
    async fn it_prunes_stale_jobs_on_re_sync() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      super::replace_for_character(
        &db,
        CHARACTER_ID,
        &[
          character_job(CHARACTER_ID, 1, "2026-06-14T00:00:00Z"),
          character_job(CHARACTER_ID, 2, "2026-06-15T00:00:00Z"),
        ],
      )
      .await
      .unwrap();

      super::replace_for_character(
        &db,
        CHARACTER_ID,
        &[character_job(CHARACTER_ID, 2, "2026-06-15T00:00:00Z")],
      )
      .await
      .unwrap();

      let jobs = super::list_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(jobs.iter().map(CharacterIndustryJob::job_id).collect::<Vec<_>>(), [2]);
    }

    #[tokio::test]
    async fn it_round_trips_every_field() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      let job = character_job(CHARACTER_ID, 7, "2026-06-14T00:00:00Z");

      super::replace_for_character(&db, CHARACTER_ID, std::slice::from_ref(&job))
        .await
        .unwrap();

      let jobs = super::list_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(jobs, vec![job]);
    }

    #[tokio::test]
    async fn it_cascades_when_the_character_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      super::replace_for_character(
        &db,
        CHARACTER_ID,
        &[character_job(CHARACTER_ID, 1, "2026-06-14T00:00:00Z")],
      )
      .await
      .unwrap();

      sqlx::query("DELETE FROM characters WHERE id = ?")
        .bind(CHARACTER_ID)
        .execute(&db.0)
        .await
        .unwrap();

      assert!(super::list_for_character(&db, CHARACTER_ID).await.unwrap().is_empty());
    }
  }

  mod replace_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_then_prunes_for_an_authorized_corp() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corporation(&db).await;

      super::replace_for_corporation(
        &db,
        CORPORATION_ID,
        &[
          corporation_job(CORPORATION_ID, 10, "2026-06-14T00:00:00Z"),
          corporation_job(CORPORATION_ID, 11, "2026-06-15T00:00:00Z"),
        ],
      )
      .await
      .unwrap();
      super::replace_for_corporation(
        &db,
        CORPORATION_ID,
        &[corporation_job(CORPORATION_ID, 11, "2026-06-15T00:00:00Z")],
      )
      .await
      .unwrap();

      let jobs = super::list_for_corporation(&db, CORPORATION_ID).await.unwrap();
      assert_eq!(
        jobs.iter().map(CorporationIndustryJob::job_id).collect::<Vec<_>>(),
        [11]
      );
    }
  }

  mod list_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_jobs_for_an_authorized_corp() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corporation(&db).await;
      super::replace_for_corporation(
        &db,
        CORPORATION_ID,
        &[corporation_job(CORPORATION_ID, 10, "2026-06-14T00:00:00Z")],
      )
      .await
      .unwrap();

      let jobs = super::list_for_corporation(&db, CORPORATION_ID).await.unwrap();

      assert_eq!(jobs.len(), 1);
      assert_eq!(jobs[0].job_id(), 10);
    }

    #[tokio::test]
    async fn it_hides_jobs_for_an_unauthorized_corp() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      super::replace_for_corporation(
        &db,
        CORPORATION_ID,
        &[corporation_job(CORPORATION_ID, 10, "2026-06-14T00:00:00Z")],
      )
      .await
      .unwrap();

      assert!(
        super::list_for_corporation(&db, CORPORATION_ID)
          .await
          .unwrap()
          .is_empty()
      );
    }
  }

  mod list_all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_both_character_and_corporation_jobs() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      seed_character(&db, DIRECTOR_ID).await;
      super::replace_for_character(
        &db,
        CHARACTER_ID,
        &[character_job(CHARACTER_ID, 1, "2026-06-14T00:00:00Z")],
      )
      .await
      .unwrap();
      super::replace_for_corporation(
        &db,
        CORPORATION_ID,
        &[corporation_job(CORPORATION_ID, 10, "2026-06-15T00:00:00Z")],
      )
      .await
      .unwrap();

      let all = super::list_all(&db).await.unwrap();

      assert_eq!(
        all
          .character_jobs
          .iter()
          .map(CharacterIndustryJob::job_id)
          .collect::<Vec<_>>(),
        [1]
      );
      assert_eq!(
        all
          .corporation_jobs
          .iter()
          .map(CorporationIndustryJob::job_id)
          .collect::<Vec<_>>(),
        [10]
      );
    }
  }
}
