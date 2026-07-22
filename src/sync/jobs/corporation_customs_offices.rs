use chrono::Utc;
use reqwest::StatusCode;

use crate::{
  clients::{Error, esi::models::corporation::CorporationCustomsOffice, eve_sso::Grant},
  store::{
    model::{CustomsOffice, OwnerType},
    repo::{character, customs_office, infra, org},
  },
  sync::{job::JobCtx, outcome::Outcome, structure_resolution::resolve_solar_system, subject::Subject},
};

const CUSTOMS_OFFICE_ROLES: &[&str] = &["Director"];

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  match ctx.key.subject {
    Subject::Character(character_id) => run_character(ctx, character_id).await,
    Subject::Corporation(corporation_id) => run_corporation(ctx, corporation_id).await,
  }
}

async fn run_character(ctx: &JobCtx<'_>, character_id: i64) -> Result<Outcome, Error> {
  let corporation_id = character_corporation(ctx, character_id).await?;
  let grant = ready_grant(ctx, corporation_id).await?;
  sync_offices(ctx, grant, corporation_id, character_id).await
}

async fn run_corporation(ctx: &JobCtx<'_>, corporation_id: i64) -> Result<Outcome, Error> {
  let grant = ready_grant(ctx, corporation_id).await?;
  let authorized_by = authorizing_character(ctx, corporation_id).await?;
  sync_offices(ctx, grant, corporation_id, authorized_by).await
}

async fn ready_grant<'a>(ctx: &JobCtx<'a>, corporation_id: i64) -> Result<&'a Grant, Error> {
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "corporation customs offices job for {corporation_id} requires a grant"
    )));
  };
  if org::get_corporation(ctx.db, corporation_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  Ok(grant)
}

async fn sync_offices(
  ctx: &JobCtx<'_>,
  grant: &Grant,
  corporation_id: i64,
  character_id: i64,
) -> Result<Outcome, Error> {
  if !holds_director_role(ctx, grant, corporation_id, character_id).await? {
    return Ok(Outcome::Skipped {
      reason: format!("character {character_id} lacks the Director role in corporation {corporation_id}"),
    });
  }

  let authenticated = ctx.esi.corporation_authenticated(grant);
  let offices = match authenticated.customs_offices(corporation_id).await {
    Ok(offices) => offices,
    Err(error) if is_forbidden(&error) => {
      tracing::warn!(
        corporation_id,
        "skipping corporation customs offices: forbidden (Director role required)"
      );
      return Ok(Outcome::Skipped {
        reason: format!("corporation {corporation_id} customs offices are forbidden (missing Director role)"),
      });
    }
    Err(error) => return Err(reauth_error(error, corporation_id)),
  };

  let synced_at = Utc::now().to_rfc3339();
  let mut persisted = 0;
  for office in &offices {
    if persist_office(ctx, corporation_id, office, &synced_at).await? {
      persisted += 1;
    }
  }
  Ok(Outcome::from_rows(persisted))
}

async fn persist_office(
  ctx: &JobCtx<'_>,
  corporation_id: i64,
  office: &CorporationCustomsOffice,
  synced_at: &str,
) -> Result<bool, Error> {
  match resolve_solar_system(ctx, office.system_id).await {
    Ok(()) => {
      customs_office::upsert(ctx.db, &to_customs_office(corporation_id, office, synced_at)).await?;
      Ok(true)
    }
    Err(Error::Http(error)) if is_access_miss(&error) => {
      tracing::warn!(
        office_id = office.office_id,
        "customs office references are inaccessible (403/404); skipping"
      );
      Ok(false)
    }
    Err(error) => Err(error),
  }
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

async fn character_corporation(ctx: &JobCtx<'_>, character_id: i64) -> Result<i64, Error> {
  let Some(character) = character::get(ctx.db, character_id).await? else {
    return Err(Error::NotReady);
  };
  Ok(character.corporation_id())
}

async fn holds_director_role(
  ctx: &JobCtx<'_>,
  grant: &Grant,
  corporation_id: i64,
  character_id: i64,
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
      .find(|member| member.character_id == character_id)
      .is_some_and(|member| {
        member
          .roles
          .iter()
          .any(|role| CUSTOMS_OFFICE_ROLES.contains(&role.as_str()))
      }),
  )
}

fn is_access_miss(error: &reqwest::Error) -> bool {
  matches!(error.status(), Some(StatusCode::FORBIDDEN | StatusCode::NOT_FOUND))
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

fn to_customs_office(corporation_id: i64, office: &CorporationCustomsOffice, synced_at: &str) -> CustomsOffice {
  CustomsOffice {
    alliance_tax_rate: office.alliance_tax_rate,
    allow_access_with_standings: office.allow_access_with_standings,
    allow_alliance_access: office.allow_alliance_access,
    bad_standing_tax_rate: office.bad_standing_tax_rate,
    corporation_id,
    corporation_tax_rate: office.corporation_tax_rate,
    excellent_standing_tax_rate: office.excellent_standing_tax_rate,
    good_standing_tax_rate: office.good_standing_tax_rate,
    neutral_standing_tax_rate: office.neutral_standing_tax_rate,
    office_id: office.office_id,
    planet_id: None,
    reinforce_exit_end: i64::from(office.reinforce_exit_end),
    reinforce_exit_start: i64::from(office.reinforce_exit_start),
    standing_level: office.standing_level.clone(),
    synced_at: synced_at.to_owned(),
    system_id: office.system_id,
    terrible_standing_tax_rate: office.terrible_standing_tax_rate,
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
      model::{
        Alliance, Bloodline, Character, Constellation, Corporation, CorporationMemberRole, Gender, Race, Region,
        SolarSystem,
      },
      repo::{character, sde},
    },
    sync::job::{JobKey, JobKind},
  };

  const CORP: i64 = 90_000_001;

  const DIRECTOR: i64 = 100;

  const SYSTEM_ID: i64 = 30_000_142;

  const CONSTELLATION_ID: i64 = 20_000_020;

  const REGION_ID: i64 = 10_000_002;

  const OFFICE_ID: i64 = 1_026_000_000_001;

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

  async fn seed_geography(db: &store::Database) {
    sde::upsert_region(
      db,
      &Region {
        description: None,
        id: REGION_ID,
        name: "The Forge".to_owned(),
      },
    )
    .await
    .unwrap();
    sde::upsert_constellation(
      db,
      &Constellation {
        id: CONSTELLATION_ID,
        name: "Kimotoro".to_owned(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        region_id: REGION_ID,
      },
    )
    .await
    .unwrap();
    sde::upsert_solar_system(
      db,
      &SolarSystem {
        constellation_id: CONSTELLATION_ID,
        id: SYSTEM_ID,
        name: "Jita".to_owned(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        security_class: None,
        security_status: 0.9,
        star_id: None,
      },
    )
    .await
    .unwrap();
  }

  async fn mount_roles(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/corporations/{CORP}/roles/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn mount_customs_offices(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/corporations/{CORP}/customs_offices/")))
      .respond_with(
        ResponseTemplate::new(200)
          .insert_header("X-Pages", "1")
          .set_body_json(body),
      )
      .mount(server)
      .await;
  }

  fn office_json(office_id: i64) -> serde_json::Value {
    serde_json::json!({
      "office_id": office_id,
      "system_id": SYSTEM_ID,
      "reinforce_exit_start": 18,
      "reinforce_exit_end": 22,
      "standing_level": "neutral",
      "allow_alliance_access": true,
      "allow_access_with_standings": false,
      "alliance_tax_rate": 0.02,
      "corporation_tax_rate": 0.05,
      "excellent_standing_tax_rate": 0.01,
      "good_standing_tax_rate": 0.02,
      "neutral_standing_tax_rate": 0.05,
      "bad_standing_tax_rate": 0.2,
      "terrible_standing_tax_rate": 0.3,
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

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: &'a Grant,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::CorporationCustomsOffices, Subject::Corporation(CORP)),
      grant: Some(grant),
      sso: None,
    }
  }

  fn ctx_with_character_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: &'a Grant,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::CorporationCustomsOffices, Subject::Character(DIRECTOR)),
      grant: Some(grant),
      sso: None,
    }
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

  mod run_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_discovered_offices_when_the_director_role_is_held() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Director"] }]),
      )
      .await;
      mount_customs_offices(&server, serde_json::json!([office_json(OFFICE_ID)])).await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Director").await;
      seed_geography(&db).await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant);

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 1
        }
      );

      let office = customs_office::read(&db, OFFICE_ID)
        .await
        .unwrap()
        .expect("the discovered office is persisted");
      assert_eq!(office.corporation_id, CORP);
      assert_eq!(office.system_id, SYSTEM_ID);
      assert_eq!(office.standing_level, "neutral");
      assert_eq!(office.reinforce_exit_start, 18);
      assert_eq!(office.reinforce_exit_end, 22);
      assert!(office.allow_alliance_access);
      assert!(!office.allow_access_with_standings);
      assert_eq!(office.corporation_tax_rate, Some(0.05));
      assert_eq!(office.planet_id, None);
      assert!(!office.synced_at.is_empty());
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
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant);

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
    }

    #[tokio::test]
    async fn it_skips_honestly_when_the_authorizing_character_lacks_the_director_role() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Station_Manager"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/customs_offices/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Station_Manager").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant);

      let outcome = run(&ctx).await.unwrap();

      assert!(
        matches!(outcome, Outcome::Skipped { .. }),
        "a missing Director role is an honest skip, not a failure, got {outcome:?}"
      );
      assert!(customs_office::read(&db, OFFICE_ID).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_skips_honestly_when_the_customs_offices_endpoint_is_forbidden() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Director"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/customs_offices/")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Director").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant);

      let outcome = run(&ctx).await.unwrap();

      assert!(
        matches!(outcome, Outcome::Skipped { .. }),
        "a 403 from the customs offices endpoint is an honest skip, got {outcome:?}"
      );
    }

    #[tokio::test]
    async fn it_surfaces_a_401_as_needs_reauthentication() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Director"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/customs_offices/")))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Director").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant);

      let result = run(&ctx).await;

      assert!(
        matches!(&result, Err(Error::Internal(message)) if message.contains("needs re-authentication")),
        "expected a re-authentication error, got {result:?}"
      );
    }
  }

  mod run_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_offices_for_a_managing_character_without_a_corp_credential() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Director"] }]),
      )
      .await;
      mount_customs_offices(&server, serde_json::json!([office_json(OFFICE_ID)])).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR).await;
      seed_geography(&db).await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("char-token", DIRECTOR);
      let ctx = ctx_with_character_grant(&db, &esi, &image, &image_store, &grant);

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 1
        }
      );
      let office = customs_office::read(&db, OFFICE_ID)
        .await
        .unwrap()
        .expect("a managing character drives the sync without any corporation credential");
      assert_eq!(office.corporation_id, CORP);
    }

    #[tokio::test]
    async fn it_skips_honestly_when_the_character_lacks_the_director_role() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Station_Manager"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/customs_offices/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR).await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("char-token", DIRECTOR);
      let ctx = ctx_with_character_grant(&db, &esi, &image, &image_store, &grant);

      let outcome = run(&ctx).await.unwrap();

      assert!(
        matches!(outcome, Outcome::Skipped { .. }),
        "a character without the Director role is an honest skip, got {outcome:?}"
      );
    }
  }

  mod to_customs_office {
    use pretty_assertions::assert_eq;

    use super::*;

    fn sample() -> CorporationCustomsOffice {
      serde_json::from_value(office_json(OFFICE_ID)).unwrap()
    }

    #[test]
    fn it_maps_every_field_from_the_esi_payload() {
      let office = sample();

      let mapped = super::super::to_customs_office(CORP, &office, "2026-07-15T00:00:00Z");

      assert_eq!(mapped.office_id, OFFICE_ID);
      assert_eq!(mapped.corporation_id, CORP);
      assert_eq!(mapped.system_id, SYSTEM_ID);
      assert_eq!(mapped.planet_id, None);
      assert_eq!(mapped.standing_level, "neutral");
      assert_eq!(mapped.reinforce_exit_start, 18);
      assert_eq!(mapped.reinforce_exit_end, 22);
      assert!(mapped.allow_alliance_access);
      assert!(!mapped.allow_access_with_standings);
      assert_eq!(mapped.alliance_tax_rate, Some(0.02));
      assert_eq!(mapped.corporation_tax_rate, Some(0.05));
      assert_eq!(mapped.excellent_standing_tax_rate, Some(0.01));
      assert_eq!(mapped.terrible_standing_tax_rate, Some(0.3));
      assert_eq!(mapped.synced_at, "2026-07-15T00:00:00Z");
    }
  }
}
