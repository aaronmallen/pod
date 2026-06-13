use reqwest::StatusCode;

use crate::{
  clients::{Error, esi::models::industry::IndustryJob, eve_sso::Grant},
  store::{
    model::{CharacterIndustryJob, CorporationIndustryJob, OwnerType},
    repo::{character, industry, infra, org},
  },
  sync::{job::JobCtx, outcome::Outcome, subject::Subject},
};

const FACTORY_MANAGER_ROLES: &[&str] = &["Director", "Factory_Manager"];

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  match ctx.key.subject {
    Subject::Character(character_id) => run_character(ctx, character_id).await,
    Subject::Corporation(corporation_id) => run_corporation(ctx, corporation_id).await,
  }
}

async fn run_character(ctx: &JobCtx<'_>, character_id: i64) -> Result<Outcome, Error> {
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character industry jobs job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authenticated = ctx.esi.character_authenticated(grant);

  let esi_jobs = authenticated.industry_jobs().await?;
  let rows: Vec<CharacterIndustryJob> = esi_jobs.iter().map(|job| to_character_job(character_id, job)).collect();

  industry::replace_for_character(ctx.db, character_id, &rows).await?;
  Ok(Outcome::from_rows(rows.len()))
}

async fn run_corporation(ctx: &JobCtx<'_>, corporation_id: i64) -> Result<Outcome, Error> {
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "corporation industry jobs job for {corporation_id} requires a grant"
    )));
  };
  if org::get_corporation(ctx.db, corporation_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authorized_by = authorizing_character(ctx, corporation_id).await?;
  if !holds_factory_manager_role(ctx, grant, corporation_id, authorized_by).await? {
    return Ok(Outcome::Skipped {
      reason: format!(
        "authorizing character {authorized_by} lacks the Factory_Manager role in corporation {corporation_id}"
      ),
    });
  }

  let authenticated = ctx.esi.corporation_authenticated(grant);
  let esi_jobs = match authenticated.industry_jobs(corporation_id).await {
    Ok(jobs) => jobs,
    Err(error) if is_forbidden(&error) => {
      tracing::warn!(
        corporation_id,
        "skipping corporation industry jobs: forbidden (Factory_Manager role required)"
      );
      return Ok(Outcome::Skipped {
        reason: format!("corporation {corporation_id} industry jobs are forbidden (missing Factory_Manager role)"),
      });
    }
    Err(error) => return Err(reauth_error(error, corporation_id)),
  };

  let rows: Vec<CorporationIndustryJob> = esi_jobs
    .iter()
    .map(|job| to_corporation_job(corporation_id, job))
    .collect();

  industry::replace_for_corporation(ctx.db, corporation_id, &rows).await?;
  Ok(Outcome::from_rows(rows.len()))
}

async fn authorizing_character(ctx: &JobCtx<'_>, corporation_id: i64) -> Result<i64, Error> {
  let credential = infra::get(ctx.db, corporation_id, OwnerType::Corporation)
    .await?
    .ok_or_else(|| Error::Internal(format!("no corporation credential for {corporation_id}")))?;
  credential.authorized_by().ok_or_else(|| {
    Error::Internal(format!(
      "corporation credential for {corporation_id} has no authorizing character"
    ))
  })
}

async fn holds_factory_manager_role(
  ctx: &JobCtx<'_>,
  grant: &Grant,
  corporation_id: i64,
  authorized_by: i64,
) -> Result<bool, Error> {
  let roles = ctx
    .esi
    .corporation_authenticated(grant)
    .member_roles(corporation_id)
    .await
    .map_err(|error| reauth_error(error, corporation_id))?;
  Ok(
    roles
      .iter()
      .find(|member| member.character_id == authorized_by)
      .is_some_and(|member| {
        member
          .roles
          .iter()
          .any(|role| FACTORY_MANAGER_ROLES.contains(&role.as_str()))
      }),
  )
}

fn is_forbidden(error: &Error) -> bool {
  matches!(error, Error::Http(http) if http.status() == Some(StatusCode::FORBIDDEN))
}

fn is_unauthorized(error: &Error) -> bool {
  matches!(error, Error::Http(http) if http.status() == Some(StatusCode::UNAUTHORIZED))
}

fn reauth_error(error: Error, corporation_id: i64) -> Error {
  if is_unauthorized(&error) {
    Error::Internal(format!(
      "corporation {corporation_id} credential was rejected (401); needs re-authentication"
    ))
  } else {
    error
  }
}

fn to_character_job(character_id: i64, job: &IndustryJob) -> CharacterIndustryJob {
  CharacterIndustryJob {
    activity_id: i64::from(job.activity_id),
    blueprint_id: job.blueprint_id,
    blueprint_location_id: job.blueprint_location_id,
    blueprint_type_id: i64::from(job.blueprint_type_id),
    character_id,
    completed_character_id: job.completed_character_id,
    completed_date: job.completed_date.clone(),
    cost: job.cost,
    duration: i64::from(job.duration),
    end_date: job.end_date.clone(),
    facility_id: job.facility_id,
    installer_id: job.installer_id,
    job_id: job.job_id,
    licensed_runs: job.licensed_runs.map(i64::from),
    output_location_id: job.output_location_id,
    pause_date: job.pause_date.clone(),
    probability: job.probability,
    product_type_id: job.product_type_id.map(i64::from),
    runs: i64::from(job.runs),
    start_date: job.start_date.clone(),
    station_id: job.station_id,
    status: job.status.clone(),
    successful_runs: job.successful_runs.map(i64::from),
  }
}

fn to_corporation_job(corporation_id: i64, job: &IndustryJob) -> CorporationIndustryJob {
  CorporationIndustryJob {
    activity_id: i64::from(job.activity_id),
    blueprint_id: job.blueprint_id,
    blueprint_location_id: job.blueprint_location_id,
    blueprint_type_id: i64::from(job.blueprint_type_id),
    completed_character_id: job.completed_character_id,
    completed_date: job.completed_date.clone(),
    corporation_id,
    cost: job.cost,
    duration: i64::from(job.duration),
    end_date: job.end_date.clone(),
    facility_id: job.facility_id,
    installer_id: job.installer_id,
    job_id: job.job_id,
    licensed_runs: job.licensed_runs.map(i64::from),
    output_location_id: job.output_location_id,
    pause_date: job.pause_date.clone(),
    probability: job.probability,
    product_type_id: job.product_type_id.map(i64::from),
    runs: i64::from(job.runs),
    start_date: job.start_date.clone(),
    station_id: job.station_id,
    status: job.status.clone(),
    successful_runs: job.successful_runs.map(i64::from),
  }
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, esi::scopes::CORPORATION_ROLES, eve_image, eve_sso::Grant, http},
    store::{
      self, images,
      model::{Alliance, Bloodline, Character, Corporation, CorporationMemberRole, Gender, Race},
      repo::character,
    },
    sync::job::{JobKey, JobKind},
  };

  const CHARACTER_ID: i64 = 42;
  const CORP: i64 = 90_000_001;
  const DIRECTOR: i64 = 100;

  async fn seed_character(db: &store::Database, id: i64) {
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, CORP, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(CORP, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, CORP, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, CORP, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  async fn seed_corporation(db: &store::Database) {
    let mut corporation = Corporation::new(CORP, "Test Corp", "TST");
    corporation.set_ceo_id(DIRECTOR);
    corporation.set_creator_id(DIRECTOR);
    corporation.set_member_count(42);
    corporation.set_tax_rate(0.1);
    org::upsert_corporation(db, &corporation).await.unwrap();
  }

  async fn seed_corporation_credential(db: &store::Database) {
    infra::upsert(
      db,
      CORP,
      OwnerType::Corporation,
      "tok",
      "rt",
      4_102_444_800,
      Some(DIRECTOR),
      Some(CORPORATION_ROLES),
    )
    .await
    .unwrap();
  }

  async fn mount_character_jobs(server: &MockServer, character_id: i64, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/characters/{character_id}/industry/jobs/")))
      .respond_with(
        ResponseTemplate::new(200)
          .insert_header("X-Pages", "1")
          .set_body_json(body),
      )
      .mount(server)
      .await;
  }

  async fn mount_corporation_jobs(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/corporations/{CORP}/industry/jobs/")))
      .respond_with(
        ResponseTemplate::new(200)
          .insert_header("X-Pages", "1")
          .set_body_json(body),
      )
      .mount(server)
      .await;
  }

  async fn mount_roles(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/corporations/{CORP}/roles/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: &'a Grant,
    subject: Subject,
  ) -> JobCtx<'a> {
    let kind = match subject {
      Subject::Character(_) => JobKind::CharacterIndustryJobs,
      Subject::Corporation(_) => JobKind::CorporationIndustryJobs,
    };
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(kind, subject),
      grant: Some(grant),
    }
  }

  fn character_job_json(job_id: i64) -> serde_json::Value {
    serde_json::json!({
      "activity_id": 1,
      "blueprint_id": 1_000_000_000_001_i64,
      "blueprint_location_id": 60_003_760,
      "blueprint_type_id": 12_345,
      "cost": 123.45,
      "duration": 3600,
      "end_date": "2026-06-14T00:00:00Z",
      "facility_id": 60_003_760,
      "installer_id": CHARACTER_ID,
      "job_id": job_id,
      "licensed_runs": 10,
      "output_location_id": 60_003_760,
      "product_type_id": 54_321,
      "runs": 5,
      "start_date": "2026-06-13T00:00:00Z",
      "station_id": 60_003_760,
      "status": "active"
    })
  }

  fn corporation_job_json(job_id: i64) -> serde_json::Value {
    serde_json::json!({
      "activity_id": 8,
      "blueprint_id": 1_000_000_000_002_i64,
      "blueprint_location_id": 60_003_760,
      "blueprint_type_id": 22_345,
      "duration": 7200,
      "end_date": "2026-06-14T00:00:00Z",
      "facility_id": 60_003_760,
      "installer_id": DIRECTOR,
      "job_id": job_id,
      "output_location_id": 60_003_760,
      "probability": 0.5,
      "product_type_id": 64_321,
      "runs": 1,
      "start_date": "2026-06-13T00:00:00Z",
      "status": "active"
    })
  }

  async fn build_clients(
    db: &store::Database,
    server: &MockServer,
  ) -> (esi::Client, eve_image::Client, images::Store, tempfile::TempDir) {
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = esi::Client::with_base_url(http.clone(), server.uri());
    let image = eve_image::Client::with_base_url(http, server.uri());
    let images_dir = tempfile::tempdir().unwrap();
    let image_store = images::Store::new(images_dir.path().to_path_buf());
    (esi, image, image_store, images_dir)
  }

  mod run_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_full_replaces_the_character_jobs_table() {
      let server = MockServer::start().await;
      mount_character_jobs(
        &server,
        CHARACTER_ID,
        serde_json::json!([character_job_json(9001), character_job_json(9002)]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("token", CHARACTER_ID);
      let ctx = ctx_with_grant(
        &db,
        &esi,
        &image,
        &image_store,
        &grant,
        Subject::Character(CHARACTER_ID),
      );

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 2
        }
      );
      let rows = industry::list_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(rows.len(), 2);
      assert!(rows.iter().all(|row| row.activity_id() == 1));
    }

    #[tokio::test]
    async fn it_reports_empty_when_the_character_has_no_jobs() {
      let server = MockServer::start().await;
      mount_character_jobs(&server, CHARACTER_ID, serde_json::json!([])).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("token", CHARACTER_ID);
      let ctx = ctx_with_grant(
        &db,
        &esi,
        &image,
        &image_store,
        &grant,
        Subject::Character(CHARACTER_ID),
      );

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(outcome, Outcome::Empty);
    }

    #[tokio::test]
    async fn it_short_retries_without_an_esi_call_when_the_character_row_is_absent() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/characters/{CHARACTER_ID}/industry/jobs/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("token", CHARACTER_ID);
      let ctx = ctx_with_grant(
        &db,
        &esi,
        &image,
        &image_store,
        &grant,
        Subject::Character(CHARACTER_ID),
      );

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
    }
  }

  mod run_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn authorize(db: &store::Database, role: &str) {
      seed_character(db, DIRECTOR).await;
      seed_corporation(db).await;
      seed_corporation_credential(db).await;
      org::replace_for_corporation(
        db,
        CORP,
        &[CorporationMemberRole::from((CORP, DIRECTOR, role.to_owned()))],
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_full_replaces_the_corporation_jobs_table_when_the_role_is_held() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Factory_Manager"] }]),
      )
      .await;
      mount_corporation_jobs(&server, serde_json::json!([corporation_job_json(7001)])).await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Factory_Manager").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Corporation(CORP));

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 1
        }
      );
      let rows = industry::list_all(&db).await.unwrap().corporation_jobs;
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].activity_id(), 8);
    }

    #[tokio::test]
    async fn it_skips_honestly_when_the_authorizing_character_lacks_the_role() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Hangar_Take_1"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/industry/jobs/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Hangar_Take_1").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Corporation(CORP));

      let outcome = run(&ctx).await.unwrap();

      assert!(
        matches!(outcome, Outcome::Skipped { .. }),
        "a missing Factory_Manager role is an honest skip, not a failure, got {outcome:?}"
      );
      assert!(industry::list_all(&db).await.unwrap().corporation_jobs.is_empty());
    }

    #[tokio::test]
    async fn it_skips_honestly_when_the_jobs_endpoint_is_forbidden() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Factory_Manager"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/industry/jobs/")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Factory_Manager").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Corporation(CORP));

      let outcome = run(&ctx).await.unwrap();

      assert!(
        matches!(outcome, Outcome::Skipped { .. }),
        "a 403 from the jobs endpoint is an honest skip, got {outcome:?}"
      );
    }

    #[tokio::test]
    async fn it_surfaces_a_401_as_needs_reauthentication() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Factory_Manager"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/industry/jobs/")))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Factory_Manager").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Corporation(CORP));

      let result = run(&ctx).await;

      assert!(
        matches!(&result, Err(Error::Internal(message)) if message.contains("needs re-authentication")),
        "expected a re-authentication error, got {result:?}"
      );
    }

    #[tokio::test]
    async fn it_short_retries_when_the_corporation_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/roles/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Corporation(CORP));

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
    }
  }
}
