use reqwest::StatusCode;

use crate::{
  clients::{Error, esi::models::industry::MiningExtraction, eve_sso::Grant},
  store::{
    model::{CorporationMiningExtraction, OwnerType},
    repo::{infra, org},
  },
  sync::{job::JobCtx, outcome::Outcome, subject::Subject},
};

const STATION_MANAGER_ROLES: &[&str] = &["Director", "Station_Manager"];

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Corporation(corporation_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "corporation mining extractions job for {corporation_id} requires a grant"
    )));
  };
  if org::get_corporation(ctx.db, corporation_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authorized_by = authorizing_character(ctx, corporation_id).await?;
  if !holds_station_manager_role(ctx, grant, corporation_id, authorized_by).await? {
    return Ok(Outcome::Skipped {
      reason: format!(
        "authorizing character {authorized_by} lacks the Station_Manager role in corporation {corporation_id}"
      ),
    });
  }

  let extractions = match ctx
    .esi
    .corporation_authenticated(grant)
    .mining_extractions(corporation_id)
    .await
  {
    Ok(extractions) => extractions,
    Err(error) if is_forbidden(&error) => {
      tracing::warn!(
        corporation_id,
        "skipping corporation mining extractions: forbidden (Station_Manager role required)"
      );
      return Ok(Outcome::Skipped {
        reason: format!("corporation {corporation_id} mining extractions are forbidden (missing Station_Manager role)"),
      });
    }
    Err(error) => return Err(reauth_error(error, corporation_id)),
  };

  let rows: Vec<CorporationMiningExtraction> = extractions
    .iter()
    .map(|extraction| to_row(corporation_id, extraction))
    .collect();

  org::replace_extractions_for_corporation(ctx.db, corporation_id, &rows).await?;
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

async fn holds_station_manager_role(
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
          .any(|role| STATION_MANAGER_ROLES.contains(&role.as_str()))
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

fn to_row(corporation_id: i64, extraction: &MiningExtraction) -> CorporationMiningExtraction {
  CorporationMiningExtraction {
    chunk_arrival_time: Some(extraction.chunk_arrival_time.clone()),
    corporation_id,
    extraction_start_time: Some(extraction.extraction_start_time.clone()),
    moon_id: extraction.moon_id,
    moon_name: None,
    natural_decay_time: Some(extraction.natural_decay_time.clone()),
    security_status: None,
    solar_system_id: None,
    structure_id: extraction.structure_id,
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

  async fn mount_extractions(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/corporation/{CORP}/mining/extractions/")))
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

  fn extraction_json(moon_id: i64, structure_id: i64) -> serde_json::Value {
    serde_json::json!({
      "chunk_arrival_time": "2026-06-20T00:00:00Z",
      "extraction_start_time": "2026-06-13T00:00:00Z",
      "moon_id": moon_id,
      "natural_decay_time": "2026-06-21T00:00:00Z",
      "structure_id": structure_id
    })
  }

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: Option<&'a Grant>,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::CorporationMiningExtractions, Subject::Corporation(CORP)),
      grant,
      sso: None,
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_errors_when_the_grant_is_missing() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/corporation/{CORP}/mining/extractions/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_corporation(&db).await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, None);

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(org::corporation_mining_extractions(&db, CORP).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_full_replaces_the_extractions_table_when_the_role_is_held() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Station_Manager"] }]),
      )
      .await;
      mount_extractions(
        &server,
        serde_json::json!([extraction_json(40_000_001, 1_021_000_000_001_i64)]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Station_Manager").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant));

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 1
        }
      );
      let rows = org::corporation_mining_extractions(&db, CORP).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].moon_id(), 40_000_001);
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
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant));

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
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
        .and(path(format!("/corporation/{CORP}/mining/extractions/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Hangar_Take_1").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant));

      let outcome = run(&ctx).await.unwrap();

      assert!(
        matches!(outcome, Outcome::Skipped { .. }),
        "a missing Station_Manager role is an honest skip, not a failure, got {outcome:?}"
      );
      assert!(org::corporation_mining_extractions(&db, CORP).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_skips_honestly_when_the_extractions_endpoint_is_forbidden() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Station_Manager"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporation/{CORP}/mining/extractions/")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Station_Manager").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant));

      let outcome = run(&ctx).await.unwrap();

      assert!(
        matches!(outcome, Outcome::Skipped { .. }),
        "a 403 from the extractions endpoint is an honest skip, got {outcome:?}"
      );
    }
  }
}
