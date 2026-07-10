use std::collections::HashSet;

use sqlx::{QueryBuilder, Sqlite};

use crate::store::{
  Database, Error,
  model::{
    AllIndustryJobs, CharacterIndustryJob, CorporationIndustryJob, Facility, FacilityIntel, IndustryCostIndex,
    IndustryPlan, PlanSegment, PlanTree, PlanType,
  },
  repo::{industry_completion, org},
};

const INDUSTRY_WRITE_BATCH_SIZE: usize = 500;

const SQLITE_MAX_BIND_PARAMS: usize = 999;

const STRUCTURE_FACILITY_ROLES: &[&str] = &["Director", "Station_Manager"];

/// Returns all NPC stations plus corp structures whose owning corp is authorized and whose `authorized_by` character
/// holds Director or Station_Manager (the same roles that gate structure discovery); ordered by manufacturing cost
/// index ascending, facilities with no index last. Nothing user-curated persists here: a facility the user can no
/// longer access drops out entirely (keep-forever intel lives in `facility_intel`, not the picker).
pub async fn accessible_facilities(db: &Database) -> Result<Vec<Facility>, Error> {
  let rows = sqlx::query_as::<_, Facility>(
    "WITH accessible_structures AS ( \
      SELECT s.id, s.owner_id, s.name, s.solar_system_id, s.type_id FROM structures s \
      JOIN owned_corporations oc ON oc.id = s.owner_id \
      JOIN corporation_member_roles cmr \
        ON cmr.corporation_id = oc.id \
        AND cmr.character_id = oc.authorized_by \
        AND cmr.role IN (?, ?) \
      LEFT JOIN inaccessible_structures ina \
        ON ina.id = s.id AND ina.owner_id = s.owner_id AND ina.owner_type = 'corporation' \
      WHERE ina.id IS NULL \
    ) \
    SELECT f.id AS id, ci.manufacturing AS manufacturing_index, f.name AS name, \
      f.owner_id AS owner_id, reg.name AS region, ss.security_status AS security_status, \
      ss.name AS solar_system, f.solar_system_id AS solar_system_id, f.type_id AS type_id \
    FROM ( \
      SELECT id, NULL AS owner_id, name, system_id AS solar_system_id, type_id FROM stations \
      UNION ALL \
      SELECT id, owner_id, name, solar_system_id, type_id FROM accessible_structures \
    ) f \
    LEFT JOIN industry_cost_indices ci ON ci.solar_system_id = f.solar_system_id \
    LEFT JOIN solar_systems ss ON ss.id = f.solar_system_id \
    LEFT JOIN constellations con ON con.id = ss.constellation_id \
    LEFT JOIN regions reg ON reg.id = con.region_id \
    ORDER BY ci.manufacturing IS NULL, ci.manufacturing, f.name, f.id",
  )
  .bind(STRUCTURE_FACILITY_ROLES[0])
  .bind(STRUCTURE_FACILITY_ROLES[1])
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn cost_index_for(db: &Database, solar_system_id: i64, activity_id: i64) -> Result<Option<f64>, Error> {
  Ok(
    cost_indices_for_system(db, solar_system_id)
      .await?
      .and_then(|indices| indices.for_activity(activity_id)),
  )
}

pub async fn cost_indices_for_system(db: &Database, solar_system_id: i64) -> Result<Option<IndustryCostIndex>, Error> {
  let row = sqlx::query_as::<_, IndustryCostIndex>(
    "SELECT copying, invention, manufacturing, reaction, researching_material_efficiency, \
    researching_time_efficiency, solar_system_id \
    FROM industry_cost_indices WHERE solar_system_id = ?",
  )
  .bind(solar_system_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn system_geo(
  db: &Database,
  solar_system_id: i64,
) -> Result<(Option<f64>, Option<String>, Option<String>), Error> {
  let row: Option<(Option<f64>, Option<String>, Option<String>)> = sqlx::query_as(
    "SELECT ss.security_status, reg.name, ss.name FROM solar_systems ss \
    LEFT JOIN constellations con ON con.id = ss.constellation_id \
    LEFT JOIN regions reg ON reg.id = con.region_id \
    WHERE ss.id = ?",
  )
  .bind(solar_system_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row.unwrap_or((None, None, None)))
}

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

/// Wholesale replaces all cost index rows in a single transaction; systems absent from `indices` are dropped. An empty
/// `indices` is treated as a transient ESI degradation (a 200 with a `[]` body) rather than a real "no system has a
/// cost index" state — which never occurs on Tranquility — so it is a no-op that leaves the existing rows intact.
pub async fn replace_cost_indices(db: &Database, indices: &[IndustryCostIndex]) -> Result<(), Error> {
  if indices.is_empty() {
    return Ok(());
  }

  let mut tx = db.writer().begin().await?;
  sqlx::query("DELETE FROM industry_cost_indices")
    .execute(&mut *tx)
    .await?;

  for chunk in indices.chunks(SQLITE_MAX_BIND_PARAMS / 7) {
    let mut builder = QueryBuilder::<Sqlite>::new(
      "INSERT INTO industry_cost_indices \
        (copying, invention, manufacturing, reaction, researching_material_efficiency, \
        researching_time_efficiency, solar_system_id) ",
    );
    builder.push_values(chunk, |mut row, index| {
      row
        .push_bind(index.copying())
        .push_bind(index.invention())
        .push_bind(index.manufacturing())
        .push_bind(index.reaction())
        .push_bind(index.researching_material_efficiency())
        .push_bind(index.researching_time_efficiency())
        .push_bind(index.solar_system_id());
    });
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn create_plan(db: &Database, name: &str, tree: &PlanTree) -> Result<IndustryPlan, Error> {
  let saved_at = chrono::Utc::now().to_rfc3339();
  let mut tx = db.writer().begin().await?;

  let plan = sqlx::query_as::<_, IndustryPlan>(
    "INSERT INTO industry_plans (name, product_type_id, runs, root_facility_system, saved_at) \
    VALUES (?, ?, ?, ?, ?) \
    RETURNING id, name, product_type_id, root_facility_system, runs, saved_at",
  )
  .bind(name)
  .bind(tree.product_type_id)
  .bind(tree.runs)
  .bind(tree.root_facility_system)
  .bind(&saved_at)
  .fetch_one(&mut *tx)
  .await?;

  for chunk in tree.types.chunks(SQLITE_MAX_BIND_PARAMS / 8) {
    let mut builder = QueryBuilder::<Sqlite>::new(
      "INSERT INTO industry_plan_types \
        (plan_id, type_id, me, te, facility_system, facility_structure, built, use_stock) ",
    );
    builder.push_values(chunk, |mut row, kind| {
      row
        .push_bind(plan.id())
        .push_bind(kind.type_id)
        .push_bind(kind.me)
        .push_bind(kind.te)
        .push_bind(kind.facility_system)
        .push_bind(kind.facility_structure)
        .push_bind(i64::from(kind.built))
        .push_bind(i64::from(kind.use_stock));
    });
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(plan)
}

pub async fn replace_plan_segments(db: &Database, plan_id: i64, segments: &[PlanSegment]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;
  sqlx::query("DELETE FROM industry_plan_segments WHERE plan_id = ?")
    .bind(plan_id)
    .execute(&mut *tx)
    .await?;
  insert_segments(&mut tx, plan_id, segments).await?;

  tx.commit().await?;
  Ok(())
}

pub async fn segments_for_plan(db: &Database, plan_id: i64) -> Result<Vec<PlanSegment>, Error> {
  let rows = sqlx::query_as::<_, (Option<i64>, Option<i64>, i64, i64, i64)>(
    "SELECT clone_id, pilot_id, runs, segment_index, type_id \
    FROM industry_plan_segments WHERE plan_id = ? ORDER BY type_id, segment_index",
  )
  .bind(plan_id)
  .fetch_all(&db.0)
  .await?;
  Ok(
    rows
      .into_iter()
      .map(|(clone_id, pilot_id, runs, segment_index, type_id)| PlanSegment {
        clone_id,
        pilot_id,
        runs,
        segment_index,
        type_id,
      })
      .collect(),
  )
}

pub async fn plan_facility_structures(db: &Database) -> Result<Vec<i64>, Error> {
  Ok(
    sqlx::query_scalar::<_, i64>(
      "SELECT DISTINCT facility_structure FROM industry_plan_types WHERE facility_structure IS NOT NULL",
    )
    .fetch_all(db.reader())
    .await?,
  )
}

pub async fn delete_plan(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM industry_plans WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn list_plans(db: &Database) -> Result<Vec<IndustryPlan>, Error> {
  let rows = sqlx::query_as::<_, IndustryPlan>(
    "SELECT id, name, product_type_id, root_facility_system, runs, saved_at FROM industry_plans \
    ORDER BY saved_at DESC, id DESC",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn load_plan(db: &Database, id: i64) -> Result<Option<PlanTree>, Error> {
  let Some(plan) = sqlx::query_as::<_, IndustryPlan>(
    "SELECT id, name, product_type_id, root_facility_system, runs, saved_at FROM industry_plans WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?
  else {
    return Ok(None);
  };

  let rows = sqlx::query_as::<_, (i64, i64, i64, Option<i64>, Option<i64>, i64, i64)>(
    "SELECT type_id, me, te, facility_system, facility_structure, built, use_stock \
    FROM industry_plan_types WHERE plan_id = ? ORDER BY type_id",
  )
  .bind(id)
  .fetch_all(&db.0)
  .await?;

  let types = rows
    .into_iter()
    .map(
      |(type_id, me, te, facility_system, facility_structure, built, use_stock)| PlanType {
        built: built != 0,
        facility_structure,
        facility_system,
        me,
        te,
        type_id,
        use_stock: use_stock != 0,
      },
    )
    .collect();

  Ok(Some(PlanTree {
    product_type_id: plan.product_type_id(),
    root_facility_system: plan.root_facility_system(),
    runs: plan.runs(),
    types,
  }))
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
  builder.build().execute(db.writer()).await?;
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
  builder.build().execute(db.writer()).await?;
  Ok(())
}

async fn insert_segments(
  tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
  plan_id: i64,
  segments: &[PlanSegment],
) -> Result<(), Error> {
  for chunk in segments.chunks(SQLITE_MAX_BIND_PARAMS / 6) {
    let mut builder = QueryBuilder::<Sqlite>::new(
      "INSERT INTO industry_plan_segments \
        (plan_id, type_id, segment_index, runs, pilot_id, clone_id) ",
    );
    builder.push_values(chunk, |mut row, segment| {
      row
        .push_bind(plan_id)
        .push_bind(segment.type_id)
        .push_bind(segment.segment_index)
        .push_bind(segment.runs)
        .push_bind(segment.pilot_id)
        .push_bind(segment.clone_id);
    });
    builder.build().execute(&mut **tx).await?;
  }
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

  capture_character_completions(db, jobs).await?;

  let batch_size = batch_size.max(1);
  for chunk in jobs.chunks(batch_size) {
    let mut tx = db.writer().begin().await?;
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

async fn capture_character_completions(db: &Database, jobs: &[CharacterIndustryJob]) -> Result<(), Error> {
  for job in jobs.iter().filter(|job| job.status() == "delivered") {
    industry_completion::insert_if_absent(
      db,
      job.character_id(),
      job.job_id(),
      job.activity_id(),
      job.product_type_id(),
      job.runs(),
      &completion_timestamp(job),
    )
    .await?;
  }
  Ok(())
}

fn completion_timestamp(job: &CharacterIndustryJob) -> String {
  job.completed_date().clone().unwrap_or_else(|| job.end_date().clone())
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
    let mut tx = db.writer().begin().await?;
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

pub const MANUFACTURING_ACTIVITY_ID: i64 = 1;

pub const REACTION_ACTIVITY_ID: i64 = 9;

pub async fn default_facility(db: &Database, activity_id: i64) -> Result<Option<i64>, Error> {
  Ok(
    sqlx::query_scalar::<_, i64>("SELECT facility_id FROM industry_default_facility WHERE activity_id = ?")
      .bind(activity_id)
      .fetch_optional(db.reader())
      .await?,
  )
}

pub async fn set_default_facility(db: &Database, activity_id: i64, facility_id: i64) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO industry_default_facility (activity_id, facility_id) VALUES (?, ?) \
    ON CONFLICT(activity_id) DO UPDATE SET facility_id = excluded.facility_id",
  )
  .bind(activity_id)
  .bind(facility_id)
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn clear_default_facility(db: &Database, activity_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM industry_default_facility WHERE activity_id = ?")
    .bind(activity_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn import_default_facilities(
  db: &Database,
  manufacturing: Option<i64>,
  reactions: Option<i64>,
) -> Result<(), Error> {
  for (activity_id, facility_id) in [
    (MANUFACTURING_ACTIVITY_ID, manufacturing),
    (REACTION_ACTIVITY_ID, reactions),
  ] {
    if let Some(facility_id) = facility_id {
      sqlx::query("INSERT OR IGNORE INTO industry_default_facility (activity_id, facility_id) VALUES (?, ?)")
        .bind(activity_id)
        .bind(facility_id)
        .execute(db.writer())
        .await?;
    }
  }
  Ok(())
}

pub async fn list_facility_intel(db: &Database) -> Result<Vec<FacilityIntel>, Error> {
  Ok(
    sqlx::query_as::<_, FacilityIntel>(
      "SELECT facility_id, rig_1_type_id, rig_2_type_id, rig_3_type_id, name, solar_system_id, type_id \
      FROM facility_intel ORDER BY facility_id",
    )
    .fetch_all(db.reader())
    .await?,
  )
}

/// Writes both the rigs and the facility's display snapshot; every write refreshes the snapshot so a Settings edit
/// or a `.pfi` import keeps the row self-contained. A rig or snapshot field absent from the incoming values clears
/// the stored one (overwrite, not merge).
#[expect(
  clippy::too_many_arguments,
  reason = "One column per intel field; grouping them would add a type for no gain."
)]
pub async fn upsert_facility_intel(
  db: &Database,
  facility_id: i64,
  name: Option<String>,
  rig_1_type_id: Option<i64>,
  rig_2_type_id: Option<i64>,
  rig_3_type_id: Option<i64>,
  solar_system_id: Option<i64>,
  type_id: Option<i64>,
) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO facility_intel \
      (facility_id, rig_1_type_id, rig_2_type_id, rig_3_type_id, name, solar_system_id, type_id) \
    VALUES (?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(facility_id) DO UPDATE SET \
      rig_1_type_id = excluded.rig_1_type_id, \
      rig_2_type_id = excluded.rig_2_type_id, \
      rig_3_type_id = excluded.rig_3_type_id, \
      name = excluded.name, \
      solar_system_id = excluded.solar_system_id, \
      type_id = excluded.type_id",
  )
  .bind(facility_id)
  .bind(rig_1_type_id)
  .bind(rig_2_type_id)
  .bind(rig_3_type_id)
  .bind(name)
  .bind(solar_system_id)
  .bind(type_id)
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn delete_facility_intel(db: &Database, facility_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM facility_intel WHERE facility_id = ?")
    .bind(facility_id)
    .execute(db.writer())
    .await?;
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

  fn cost_index(solar_system_id: i64, manufacturing: f64, reaction: f64) -> IndustryCostIndex {
    IndustryCostIndex {
      copying: None,
      invention: None,
      manufacturing: Some(manufacturing),
      reaction: Some(reaction),
      researching_material_efficiency: None,
      researching_time_efficiency: None,
      solar_system_id,
    }
  }

  mod accessible_facilities {
    use pretty_assertions::assert_eq;

    use super::*;

    const REGION_ID: i64 = 10_000_001;

    const CONSTELLATION_ID: i64 = 20_000_001;

    const STATION_TYPE_ID: i64 = 54;

    async fn seed_solar_system(db: &Database, id: i64) {
      sqlx::query("INSERT OR IGNORE INTO regions (id, name) VALUES (?, 'Test Region')")
        .bind(REGION_ID)
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT OR IGNORE INTO constellations (id, name, position_x, position_y, position_z, region_id) \
        VALUES (?, 'Test Constellation', 0, 0, 0, ?)",
      )
      .bind(CONSTELLATION_ID)
      .bind(REGION_ID)
      .execute(db.writer())
      .await
      .unwrap();
      sqlx::query(
        "INSERT OR IGNORE INTO solar_systems \
          (id, constellation_id, name, position_x, position_y, position_z, security_status) \
        VALUES (?, ?, 'Test System', 0, 0, 0, 1.0)",
      )
      .bind(id)
      .bind(CONSTELLATION_ID)
      .execute(db.writer())
      .await
      .unwrap();
    }

    async fn seed_station(db: &Database, id: i64, solar_system_id: i64, name: &str) {
      seed_solar_system(db, solar_system_id).await;
      sqlx::query("INSERT OR IGNORE INTO item_categories (id, name, published) VALUES (3, 'Station', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("INSERT OR IGNORE INTO item_groups (id, category_id, name, published) VALUES (15, 3, 'Station', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT OR IGNORE INTO item_types (id, group_id, name, description, published) \
        VALUES (?, 15, 'Station', '', 1)",
      )
      .bind(STATION_TYPE_ID)
      .execute(db.writer())
      .await
      .unwrap();
      sqlx::query(
        "INSERT INTO stations \
          (id, system_id, type_id, name, max_dockable_ship_volume, office_rental_cost, \
          reprocessing_efficiency, reprocessing_stations_take, services, position_x, position_y, position_z) \
        VALUES (?, ?, ?, ?, 0, 0, 0.5, 0.05, '[]', 0, 0, 0)",
      )
      .bind(id)
      .bind(solar_system_id)
      .bind(STATION_TYPE_ID)
      .bind(name)
      .execute(db.writer())
      .await
      .unwrap();
    }

    async fn seed_structure(db: &Database, id: i64, owner_id: i64, solar_system_id: i64, name: &str) {
      seed_solar_system(db, solar_system_id).await;
      sqlx::query("INSERT INTO structures (id, name, owner_id, solar_system_id, type_id) VALUES (?, ?, ?, ?, NULL)")
        .bind(id)
        .bind(name)
        .bind(owner_id)
        .bind(solar_system_id)
        .execute(db.writer())
        .await
        .unwrap();
    }

    async fn mark_inaccessible(db: &Database, id: i64, owner_id: i64) {
      sqlx::query(
        "INSERT INTO inaccessible_structures (owner_id, owner_type, id, marked_at) \
        VALUES (?, 'corporation', ?, '2026-06-14T00:00:00Z')",
      )
      .bind(owner_id)
      .bind(id)
      .execute(db.writer())
      .await
      .unwrap();
    }

    async fn authorize_corporation_with_role(db: &Database, role: &str) {
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
          role.to_owned(),
        ))],
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_enriches_facilities_with_security_status_and_region() {
      let db = store::open_test().await.unwrap();
      seed_station(&db, 60_000_001, 30_000_142, "Jita IV - Moon 4").await;

      let facilities = super::accessible_facilities(&db).await.unwrap();

      let station = facilities.iter().find(|f| f.id() == 60_000_001).unwrap();
      assert_eq!(station.security_status(), Some(1.0));
      assert_eq!(station.region(), &Some("Test Region".to_owned()));
    }

    #[tokio::test]
    async fn it_excludes_inaccessible_structures() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corporation(&db).await;
      seed_structure(&db, 1_021_000_000_001, CORPORATION_ID, 30_002_187, "Reachable").await;
      seed_structure(&db, 1_021_000_000_002, CORPORATION_ID, 30_002_187, "Unreachable").await;
      mark_inaccessible(&db, 1_021_000_000_002, CORPORATION_ID).await;

      let facilities = super::accessible_facilities(&db).await.unwrap();

      assert_eq!(
        facilities.iter().map(Facility::id).collect::<Vec<_>>(),
        [1_021_000_000_001]
      );
    }

    #[tokio::test]
    async fn it_excludes_structures_owned_by_an_unauthorized_corp() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      seed_structure(&db, 1_021_000_000_001, CORPORATION_ID, 30_002_187, "Locked Out").await;

      let facilities = super::accessible_facilities(&db).await.unwrap();

      assert!(facilities.is_empty());
    }

    #[tokio::test]
    async fn it_excludes_structures_when_the_authorizer_holds_only_factory_manager() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corporation_with_role(&db, "Factory_Manager").await;
      seed_structure(&db, 1_021_000_000_001, CORPORATION_ID, 30_002_187, "Factory").await;

      let facilities = super::accessible_facilities(&db).await.unwrap();

      assert!(facilities.is_empty());
    }

    #[tokio::test]
    async fn it_includes_structures_when_the_authorizer_holds_station_manager() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corporation_with_role(&db, "Station_Manager").await;
      seed_structure(&db, 1_021_000_000_001, CORPORATION_ID, 30_002_187, "Citadel").await;

      let facilities = super::accessible_facilities(&db).await.unwrap();

      assert_eq!(
        facilities.iter().map(Facility::id).collect::<Vec<_>>(),
        [1_021_000_000_001]
      );
    }

    #[tokio::test]
    async fn it_orders_by_manufacturing_index_ascending() {
      let db = store::open_test().await.unwrap();
      seed_station(&db, 60_000_001, 30_000_142, "Pricey System").await;
      seed_station(&db, 60_000_002, 30_002_187, "Cheap System").await;
      super::replace_cost_indices(
        &db,
        &[cost_index(30_000_142, 0.09, 0.0), cost_index(30_002_187, 0.02, 0.0)],
      )
      .await
      .unwrap();

      let facilities = super::accessible_facilities(&db).await.unwrap();

      assert_eq!(
        facilities.iter().map(Facility::id).collect::<Vec<_>>(),
        [60_000_002, 60_000_001]
      );
      assert_eq!(facilities[0].manufacturing_index(), Some(0.02));
      assert_eq!(facilities[1].manufacturing_index(), Some(0.09));
    }

    #[tokio::test]
    async fn it_sorts_facilities_without_a_cost_index_last() {
      let db = store::open_test().await.unwrap();
      seed_station(&db, 60_000_001, 30_000_142, "Indexed System").await;
      seed_station(&db, 60_000_002, 30_002_187, "Unindexed System").await;
      super::replace_cost_indices(&db, &[cost_index(30_000_142, 0.05, 0.0)])
        .await
        .unwrap();

      let facilities = super::accessible_facilities(&db).await.unwrap();

      assert_eq!(
        facilities.iter().map(Facility::id).collect::<Vec<_>>(),
        [60_000_001, 60_000_002]
      );
      assert_eq!(facilities[1].manufacturing_index(), None);
    }

    #[tokio::test]
    async fn it_unions_stations_and_accessible_structures_with_their_systems() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corporation(&db).await;
      seed_station(&db, 60_000_001, 30_000_142, "Jita IV - Moon 4").await;
      seed_structure(&db, 1_021_000_000_001, CORPORATION_ID, 30_002_187, "Amarr Citadel").await;

      let facilities = super::accessible_facilities(&db).await.unwrap();

      let ids = facilities.iter().map(Facility::id).collect::<Vec<_>>();
      assert_eq!(ids.len(), 2);
      assert!(ids.contains(&60_000_001));
      assert!(ids.contains(&1_021_000_000_001));
      let structure = facilities.iter().find(|f| f.id() == 1_021_000_000_001).unwrap();
      assert_eq!(structure.solar_system_id(), 30_002_187);
      assert_eq!(structure.owner_id(), Some(CORPORATION_ID));
      let station = facilities.iter().find(|f| f.id() == 60_000_001).unwrap();
      assert_eq!(station.solar_system_id(), 30_000_142);
      assert_eq!(station.owner_id(), None);
    }
  }

  mod cost_index_for {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_system() {
      let db = store::open_test().await.unwrap();

      assert_eq!(super::cost_index_for(&db, 30_000_142, 1).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_returns_the_indexed_activity_for_a_known_system() {
      let db = store::open_test().await.unwrap();
      super::replace_cost_indices(&db, &[cost_index(30_000_142, 0.05, 0.01)])
        .await
        .unwrap();

      assert_eq!(super::cost_index_for(&db, 30_000_142, 1).await.unwrap(), Some(0.05));
      assert_eq!(super::cost_index_for(&db, 30_000_142, 9).await.unwrap(), Some(0.01));
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

  mod list_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

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
  }

  mod plans {
    use pretty_assertions::assert_eq;

    use super::*;

    fn sample_tree() -> PlanTree {
      PlanTree {
        product_type_id: 22_544,
        root_facility_system: Some(30_000_142),
        runs: 7,
        types: vec![
          PlanType {
            built: false,
            facility_structure: Some(60_003_760),
            facility_system: Some(30_000_142),
            me: 10,
            te: 20,
            type_id: 22_544,
            use_stock: false,
          },
          PlanType {
            built: true,
            facility_structure: None,
            facility_system: None,
            me: 8,
            te: 16,
            type_id: 17_478,
            use_stock: false,
          },
          PlanType {
            built: true,
            facility_structure: Some(1_021_000_000_001),
            facility_system: Some(30_002_187),
            me: 5,
            te: 0,
            type_id: 34,
            use_stock: true,
          },
          PlanType {
            built: false,
            facility_structure: None,
            facility_system: None,
            me: 9,
            te: 18,
            type_id: 11_399,
            use_stock: false,
          },
        ],
      }
    }

    fn sample_segments() -> Vec<PlanSegment> {
      vec![
        PlanSegment {
          clone_id: Some(1001),
          pilot_id: Some(95_465_499),
          runs: 4,
          segment_index: 0,
          type_id: 22_544,
        },
        PlanSegment {
          clone_id: None,
          pilot_id: Some(90_000_001),
          runs: 3,
          segment_index: 1,
          type_id: 22_544,
        },
      ]
    }

    fn sorted_by_type(mut tree: PlanTree) -> PlanTree {
      tree.types.sort_by_key(|kind| kind.type_id);
      tree
    }

    #[tokio::test]
    async fn it_deletes_a_plan_and_cascades_its_types() {
      let db = store::open_test().await.unwrap();
      let plan = create_plan(&db, "Hulk run", &sample_tree()).await.unwrap();

      delete_plan(&db, plan.id()).await.unwrap();

      assert!(load_plan(&db, plan.id()).await.unwrap().is_none());
      let type_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM industry_plan_types WHERE plan_id = ?")
        .bind(plan.id())
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(type_count, 0);
    }

    #[tokio::test]
    async fn it_lists_saved_plans() {
      let db = store::open_test().await.unwrap();
      create_plan(&db, "First", &sample_tree()).await.unwrap();
      create_plan(&db, "Second", &sample_tree()).await.unwrap();

      let plans = list_plans(&db).await.unwrap();

      assert_eq!(plans.len(), 2);
    }

    #[tokio::test]
    async fn it_lists_distinct_plan_facility_structures() {
      let db = store::open_test().await.unwrap();
      create_plan(&db, "First", &sample_tree()).await.unwrap();
      create_plan(&db, "Second", &sample_tree()).await.unwrap();

      let mut ids = plan_facility_structures(&db).await.unwrap();
      ids.sort_unstable();

      assert_eq!(ids, vec![60_003_760, 1_021_000_000_001]);
    }

    #[tokio::test]
    async fn it_records_the_parent_metadata() {
      let db = store::open_test().await.unwrap();

      let plan = create_plan(&db, "Hulk run", &sample_tree()).await.unwrap();

      assert_eq!(plan.name(), "Hulk run");
      assert_eq!(plan.product_type_id(), 22_544);
      assert_eq!(plan.runs(), 7);
      assert_eq!(plan.root_facility_system(), Some(30_000_142));
    }

    #[tokio::test]
    async fn it_returns_none_loading_a_missing_plan() {
      let db = store::open_test().await.unwrap();

      assert_eq!(load_plan(&db, 404).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_round_trips_a_saved_tree() {
      let db = store::open_test().await.unwrap();
      let tree = sample_tree();

      let plan = create_plan(&db, "Hulk run", &tree).await.unwrap();
      let loaded = load_plan(&db, plan.id()).await.unwrap().unwrap();

      assert_eq!(sorted_by_type(loaded), sorted_by_type(tree));
    }

    #[tokio::test]
    async fn it_round_trips_the_per_type_built_flag() {
      let db = store::open_test().await.unwrap();

      let plan = create_plan(&db, "Hulk run", &sample_tree()).await.unwrap();
      let loaded = load_plan(&db, plan.id()).await.unwrap().unwrap();

      let built: Vec<i64> = loaded
        .types
        .iter()
        .filter(|kind| kind.built)
        .map(|kind| kind.type_id)
        .collect();
      assert_eq!(built, vec![34, 17_478]);
    }

    #[tokio::test]
    async fn it_round_trips_the_per_type_facility_structure() {
      let db = store::open_test().await.unwrap();

      let plan = create_plan(&db, "Hulk run", &sample_tree()).await.unwrap();
      let loaded = load_plan(&db, plan.id()).await.unwrap().unwrap();

      let root = loaded.types.iter().find(|kind| kind.type_id == 22_544).unwrap();
      let component = loaded.types.iter().find(|kind| kind.type_id == 34).unwrap();
      let unset = loaded.types.iter().find(|kind| kind.type_id == 17_478).unwrap();
      assert_eq!(root.facility_structure, Some(60_003_760));
      assert_eq!(component.facility_structure, Some(1_021_000_000_001));
      assert_eq!(unset.facility_structure, None);
    }

    #[tokio::test]
    async fn it_cascades_segments_when_a_plan_is_deleted() {
      let db = store::open_test().await.unwrap();
      let plan = create_plan(&db, "Hulk run", &sample_tree()).await.unwrap();
      replace_plan_segments(&db, plan.id(), &sample_segments()).await.unwrap();

      delete_plan(&db, plan.id()).await.unwrap();

      let segment_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM industry_plan_segments WHERE plan_id = ?")
        .bind(plan.id())
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(segment_count, 0);
    }

    #[tokio::test]
    async fn it_loads_no_segments_for_a_plan_saved_without_any() {
      let db = store::open_test().await.unwrap();
      let plan = create_plan(&db, "Hulk run", &sample_tree()).await.unwrap();

      let segments = segments_for_plan(&db, plan.id()).await.unwrap();

      assert!(segments.is_empty());
    }

    #[tokio::test]
    async fn it_replaces_segments_wholesale() {
      let db = store::open_test().await.unwrap();
      let plan = create_plan(&db, "Hulk run", &sample_tree()).await.unwrap();
      replace_plan_segments(&db, plan.id(), &sample_segments()).await.unwrap();

      replace_plan_segments(&db, plan.id(), &[]).await.unwrap();

      assert!(segments_for_plan(&db, plan.id()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_round_trips_a_plans_segments() {
      let db = store::open_test().await.unwrap();
      let plan = create_plan(&db, "Hulk run", &sample_tree()).await.unwrap();

      replace_plan_segments(&db, plan.id(), &sample_segments()).await.unwrap();

      assert_eq!(segments_for_plan(&db, plan.id()).await.unwrap(), sample_segments());
    }

    #[tokio::test]
    async fn it_round_trips_the_per_type_use_stock_intent() {
      let db = store::open_test().await.unwrap();

      let plan = create_plan(&db, "Hulk run", &sample_tree()).await.unwrap();
      let loaded = load_plan(&db, plan.id()).await.unwrap().unwrap();

      let use_stock: Vec<i64> = loaded
        .types
        .iter()
        .filter(|kind| kind.use_stock)
        .map(|kind| kind.type_id)
        .collect();
      assert_eq!(use_stock, vec![34]);
    }
  }

  mod replace_cost_indices {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_leaves_existing_rows_intact_when_the_new_set_is_empty() {
      let db = store::open_test().await.unwrap();
      super::replace_cost_indices(
        &db,
        &[cost_index(30_000_142, 0.05, 0.01), cost_index(30_002_187, 0.06, 0.02)],
      )
      .await
      .unwrap();

      super::replace_cost_indices(&db, &[]).await.unwrap();

      assert_eq!(
        super::cost_indices_for_system(&db, 30_000_142).await.unwrap(),
        Some(cost_index(30_000_142, 0.05, 0.01))
      );
      assert_eq!(
        super::cost_indices_for_system(&db, 30_002_187).await.unwrap(),
        Some(cost_index(30_002_187, 0.06, 0.02))
      );
    }

    #[tokio::test]
    async fn it_round_trips_every_system() {
      let db = store::open_test().await.unwrap();

      super::replace_cost_indices(
        &db,
        &[cost_index(30_000_142, 0.05, 0.01), cost_index(30_002_187, 0.06, 0.02)],
      )
      .await
      .unwrap();

      assert_eq!(
        super::cost_indices_for_system(&db, 30_000_142).await.unwrap(),
        Some(cost_index(30_000_142, 0.05, 0.01))
      );
      assert_eq!(
        super::cost_indices_for_system(&db, 30_002_187).await.unwrap(),
        Some(cost_index(30_002_187, 0.06, 0.02))
      );
    }

    #[tokio::test]
    async fn it_wholesale_replaces_dropping_systems_absent_from_the_new_set() {
      let db = store::open_test().await.unwrap();
      super::replace_cost_indices(
        &db,
        &[cost_index(30_000_142, 0.05, 0.01), cost_index(30_002_187, 0.06, 0.02)],
      )
      .await
      .unwrap();

      super::replace_cost_indices(&db, &[cost_index(30_002_187, 0.09, 0.03)])
        .await
        .unwrap();

      assert_eq!(super::cost_indices_for_system(&db, 30_000_142).await.unwrap(), None);
      assert_eq!(
        super::cost_indices_for_system(&db, 30_002_187).await.unwrap(),
        Some(cost_index(30_002_187, 0.09, 0.03))
      );
    }
  }

  mod replace_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

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
        .execute(db.writer())
        .await
        .unwrap();

      assert!(super::list_for_character(&db, CHARACTER_ID).await.unwrap().is_empty());
    }

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

    fn delivered(character_id: i64, job_id: i64) -> CharacterIndustryJob {
      let mut job = character_job(character_id, job_id, "2026-06-14T00:00:00Z");
      job.status = "delivered".to_owned();
      job.completed_date = Some("2026-06-14T05:00:00Z".to_owned());
      job
    }

    async fn own(db: &Database, id: i64) {
      infra::upsert(db, id, OwnerType::Character, "tok", "rt", 4_102_444_800, None, None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_records_a_delivered_completion_that_survives_the_mirror_delete() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      own(&db, CHARACTER_ID).await;

      super::replace_for_character(&db, CHARACTER_ID, &[delivered(CHARACTER_ID, 7)])
        .await
        .unwrap();
      super::replace_for_character(&db, CHARACTER_ID, &[]).await.unwrap();

      assert!(
        super::list_for_character(&db, CHARACTER_ID).await.unwrap().is_empty(),
        "the source job is mirror-deleted once it ages off ESI"
      );
      let completions = industry_completion::for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(
        completions,
        vec![(7, 1, Some(54_321), 5, "2026-06-14T05:00:00Z".to_owned())]
      );
    }

    #[tokio::test]
    async fn it_records_each_delivered_job_once_across_syncs() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      own(&db, CHARACTER_ID).await;

      super::replace_for_character(&db, CHARACTER_ID, &[delivered(CHARACTER_ID, 7)])
        .await
        .unwrap();
      super::replace_for_character(&db, CHARACTER_ID, &[delivered(CHARACTER_ID, 7)])
        .await
        .unwrap();

      assert_eq!(
        industry_completion::for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .len(),
        1,
        "re-observing the same delivered job appends no duplicate"
      );
    }

    #[tokio::test]
    async fn it_ignores_jobs_that_are_not_yet_delivered() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      own(&db, CHARACTER_ID).await;

      super::replace_for_character(
        &db,
        CHARACTER_ID,
        &[character_job(CHARACTER_ID, 7, "2026-06-14T00:00:00Z")],
      )
      .await
      .unwrap();

      assert!(
        industry_completion::for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .is_empty(),
        "only delivered jobs accrue completion history"
      );
    }
  }

  mod completion_timestamp {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_prefers_completed_date_over_end_date() {
      let mut job = character_job(CHARACTER_ID, 1, "2026-06-14T00:00:00Z");
      job.completed_date = Some("2026-06-14T05:00:00Z".to_owned());
      assert_eq!(super::completion_timestamp(&job), "2026-06-14T05:00:00Z");
    }

    #[test]
    fn it_falls_back_to_end_date_without_a_completed_date() {
      let job = character_job(CHARACTER_ID, 1, "2026-06-14T00:00:00Z");
      assert_eq!(super::completion_timestamp(&job), "2026-06-14T00:00:00Z");
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

  mod system_geo {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_system(db: &Database) {
      sqlx::query("INSERT INTO regions (id, name) VALUES (10000002, 'The Forge')")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT INTO constellations (id, name, position_x, position_y, position_z, region_id) \
        VALUES (20000002, 'Kimotoro', 0, 0, 0, 10000002)",
      )
      .execute(db.writer())
      .await
      .unwrap();
      sqlx::query(
        "INSERT INTO solar_systems \
          (id, constellation_id, name, position_x, position_y, position_z, security_status) \
        VALUES (30000142, 20000002, 'Jita', 0, 0, 0, 0.9)",
      )
      .execute(db.writer())
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_resolves_security_status_and_region_for_a_known_system() {
      let db = store::open_test().await.unwrap();
      seed_system(&db).await;

      let (security, region, system) = super::system_geo(&db, 30_000_142).await.unwrap();

      assert_eq!(security, Some(0.9));
      assert_eq!(region, Some("The Forge".to_owned()));
      assert_eq!(system, Some("Jita".to_owned()));
    }

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_system() {
      let db = store::open_test().await.unwrap();

      let geo = super::system_geo(&db, 30_000_142).await.unwrap();

      assert_eq!(geo, (None, None, None));
    }
  }

  mod facility_storage {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_item_type(db: &Database, id: i64) {
      sqlx::query("INSERT OR IGNORE INTO item_categories (id, name, published) VALUES (66, 'Structure Modifier', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("INSERT OR IGNORE INTO item_groups (id, category_id, name, published) VALUES (15, 66, 'Rig', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT OR IGNORE INTO item_types (id, group_id, name, description, published) VALUES (?, 15, 'Rig', '', 1)",
      )
      .bind(id)
      .execute(db.writer())
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_returns_none_for_an_unset_default_facility() {
      let db = store::open_test().await.unwrap();

      let facility = super::default_facility(&db, MANUFACTURING_ACTIVITY_ID).await.unwrap();

      assert_eq!(facility, None);
    }

    #[tokio::test]
    async fn it_sets_and_reads_a_default_facility_per_activity() {
      let db = store::open_test().await.unwrap();

      super::set_default_facility(&db, MANUFACTURING_ACTIVITY_ID, 60_003_760)
        .await
        .unwrap();
      super::set_default_facility(&db, REACTION_ACTIVITY_ID, 1_021_000_000_009)
        .await
        .unwrap();

      assert_eq!(
        super::default_facility(&db, MANUFACTURING_ACTIVITY_ID).await.unwrap(),
        Some(60_003_760)
      );
      assert_eq!(
        super::default_facility(&db, REACTION_ACTIVITY_ID).await.unwrap(),
        Some(1_021_000_000_009)
      );
    }

    #[tokio::test]
    async fn it_overwrites_a_default_facility_on_set() {
      let db = store::open_test().await.unwrap();

      super::set_default_facility(&db, MANUFACTURING_ACTIVITY_ID, 60_003_760)
        .await
        .unwrap();
      super::set_default_facility(&db, MANUFACTURING_ACTIVITY_ID, 60_008_494)
        .await
        .unwrap();

      assert_eq!(
        super::default_facility(&db, MANUFACTURING_ACTIVITY_ID).await.unwrap(),
        Some(60_008_494)
      );
    }

    #[tokio::test]
    async fn it_clears_a_default_facility_back_to_unset() {
      let db = store::open_test().await.unwrap();
      super::set_default_facility(&db, MANUFACTURING_ACTIVITY_ID, 60_003_760)
        .await
        .unwrap();

      super::clear_default_facility(&db, MANUFACTURING_ACTIVITY_ID)
        .await
        .unwrap();

      assert_eq!(
        super::default_facility(&db, MANUFACTURING_ACTIVITY_ID).await.unwrap(),
        None
      );
    }

    #[tokio::test]
    async fn it_imports_present_config_defaults() {
      let db = store::open_test().await.unwrap();

      super::import_default_facilities(&db, Some(60_003_760), Some(1_021_000_000_009))
        .await
        .unwrap();

      assert_eq!(
        super::default_facility(&db, MANUFACTURING_ACTIVITY_ID).await.unwrap(),
        Some(60_003_760)
      );
      assert_eq!(
        super::default_facility(&db, REACTION_ACTIVITY_ID).await.unwrap(),
        Some(1_021_000_000_009)
      );
    }

    #[tokio::test]
    async fn it_skips_absent_config_defaults() {
      let db = store::open_test().await.unwrap();

      super::import_default_facilities(&db, Some(60_003_760), None)
        .await
        .unwrap();

      assert_eq!(
        super::default_facility(&db, MANUFACTURING_ACTIVITY_ID).await.unwrap(),
        Some(60_003_760)
      );
      assert_eq!(super::default_facility(&db, REACTION_ACTIVITY_ID).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_does_not_clobber_a_db_edit_on_a_repeated_import() {
      let db = store::open_test().await.unwrap();

      super::import_default_facilities(&db, Some(60_003_760), None)
        .await
        .unwrap();
      super::set_default_facility(&db, MANUFACTURING_ACTIVITY_ID, 60_008_494)
        .await
        .unwrap();
      super::import_default_facilities(&db, Some(60_003_760), None)
        .await
        .unwrap();

      assert_eq!(
        super::default_facility(&db, MANUFACTURING_ACTIVITY_ID).await.unwrap(),
        Some(60_008_494)
      );
    }

    #[tokio::test]
    async fn it_upserts_and_lists_facility_intel_with_rigs() {
      let db = store::open_test().await.unwrap();
      seed_item_type(&db, 37_180).await;
      seed_item_type(&db, 37_181).await;

      super::upsert_facility_intel(
        &db,
        1_021_000_000_009,
        None,
        Some(37_180),
        Some(37_181),
        None,
        None,
        None,
      )
      .await
      .unwrap();

      let intel = super::list_facility_intel(&db).await.unwrap();
      assert_eq!(
        intel,
        vec![FacilityIntel {
          facility_id: 1_021_000_000_009,
          name: None,
          rig_1_type_id: Some(37_180),
          rig_2_type_id: Some(37_181),
          rig_3_type_id: None,
          solar_system_id: None,
          type_id: None,
        }]
      );
    }

    #[tokio::test]
    async fn it_allows_facility_intel_with_zero_rigs() {
      let db = store::open_test().await.unwrap();

      super::upsert_facility_intel(&db, 1_021_000_000_009, None, None, None, None, None, None)
        .await
        .unwrap();

      let intel = super::list_facility_intel(&db).await.unwrap();
      assert_eq!(
        intel,
        vec![FacilityIntel {
          facility_id: 1_021_000_000_009,
          name: None,
          rig_1_type_id: None,
          rig_2_type_id: None,
          rig_3_type_id: None,
          solar_system_id: None,
          type_id: None,
        }]
      );
    }

    #[tokio::test]
    async fn it_persists_and_overwrites_the_facility_snapshot() {
      let db = store::open_test().await.unwrap();

      super::upsert_facility_intel(
        &db,
        1_021_000_000_009,
        Some("Allied Fortizar".to_owned()),
        None,
        None,
        None,
        Some(30_002_187),
        Some(35_833),
      )
      .await
      .unwrap();

      let intel = super::list_facility_intel(&db).await.unwrap();
      assert_eq!(intel[0].name.as_deref(), Some("Allied Fortizar"));
      assert_eq!(intel[0].solar_system_id, Some(30_002_187));
      assert_eq!(intel[0].type_id, Some(35_833));

      super::upsert_facility_intel(&db, 1_021_000_000_009, None, None, None, None, None, None)
        .await
        .unwrap();

      let cleared = super::list_facility_intel(&db).await.unwrap();
      assert_eq!(cleared[0].name, None);
      assert_eq!(cleared[0].solar_system_id, None);
      assert_eq!(cleared[0].type_id, None);
    }

    #[tokio::test]
    async fn it_overwrites_facility_intel_rigs_on_upsert() {
      let db = store::open_test().await.unwrap();
      seed_item_type(&db, 37_180).await;
      seed_item_type(&db, 37_182).await;

      super::upsert_facility_intel(&db, 1_021_000_000_009, None, Some(37_180), None, None, None, None)
        .await
        .unwrap();
      super::upsert_facility_intel(&db, 1_021_000_000_009, None, Some(37_182), None, None, None, None)
        .await
        .unwrap();

      let intel = super::list_facility_intel(&db).await.unwrap();
      assert_eq!(intel.len(), 1);
      assert_eq!(intel[0].rig_1_type_id, Some(37_182));
    }

    #[tokio::test]
    async fn it_deletes_a_facility_intel_row() {
      let db = store::open_test().await.unwrap();

      super::upsert_facility_intel(&db, 1_021_000_000_009, None, None, None, None, None, None)
        .await
        .unwrap();
      super::delete_facility_intel(&db, 1_021_000_000_009).await.unwrap();

      assert!(super::list_facility_intel(&db).await.unwrap().is_empty());
    }
  }
}
