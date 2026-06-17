use reqwest::StatusCode;

use crate::{
  clients::{Error, esi::models::blueprint::Blueprint, eve_sso::Grant},
  store::{
    model::{CharacterBlueprint, CorporationBlueprint, OwnerType},
    repo::{blueprints, character, infra, org},
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
      "character blueprints job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authenticated = ctx.esi.character_authenticated(grant);

  let esi_blueprints = authenticated.blueprints().await?;
  let rows: Vec<CharacterBlueprint> = esi_blueprints
    .iter()
    .map(|blueprint| to_character_blueprint(character_id, blueprint))
    .collect();

  blueprints::replace_for_character(ctx.db, character_id, &rows).await?;
  Ok(Outcome::from_rows(rows.len()))
}

async fn run_corporation(ctx: &JobCtx<'_>, corporation_id: i64) -> Result<Outcome, Error> {
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "corporation blueprints job for {corporation_id} requires a grant"
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
  let esi_blueprints = match authenticated.blueprints(corporation_id).await {
    Ok(blueprints) => blueprints,
    Err(error) if is_forbidden(&error) => {
      tracing::warn!(
        corporation_id,
        "skipping corporation blueprints: forbidden (Factory_Manager role required)"
      );
      return Ok(Outcome::Skipped {
        reason: format!("corporation {corporation_id} blueprints are forbidden (missing Factory_Manager role)"),
      });
    }
    Err(error) => return Err(reauth_error(error, corporation_id)),
  };

  let rows: Vec<CorporationBlueprint> = esi_blueprints
    .iter()
    .map(|blueprint| to_corporation_blueprint(corporation_id, blueprint))
    .collect();

  blueprints::replace_for_corporation(ctx.db, corporation_id, &rows).await?;
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

fn to_character_blueprint(character_id: i64, blueprint: &Blueprint) -> CharacterBlueprint {
  CharacterBlueprint {
    character_id,
    item_id: blueprint.item_id,
    location_flag: blueprint.location_flag.clone(),
    location_id: blueprint.location_id,
    material_efficiency: i64::from(blueprint.material_efficiency),
    quantity: i64::from(blueprint.quantity),
    runs: i64::from(blueprint.runs),
    time_efficiency: i64::from(blueprint.time_efficiency),
    type_id: i64::from(blueprint.type_id),
  }
}

fn to_corporation_blueprint(corporation_id: i64, blueprint: &Blueprint) -> CorporationBlueprint {
  CorporationBlueprint {
    corporation_id,
    item_id: blueprint.item_id,
    location_flag: blueprint.location_flag.clone(),
    location_id: blueprint.location_id,
    material_efficiency: i64::from(blueprint.material_efficiency),
    quantity: i64::from(blueprint.quantity),
    runs: i64::from(blueprint.runs),
    time_efficiency: i64::from(blueprint.time_efficiency),
    type_id: i64::from(blueprint.type_id),
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

  async fn mount_character_blueprints(server: &MockServer, character_id: i64, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/characters/{character_id}/blueprints/")))
      .respond_with(
        ResponseTemplate::new(200)
          .insert_header("X-Pages", "1")
          .set_body_json(body),
      )
      .mount(server)
      .await;
  }

  async fn mount_corporation_blueprints(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/corporations/{CORP}/blueprints/")))
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
      Subject::Character(_) => JobKind::CharacterBlueprints,
      Subject::Corporation(_) => JobKind::CorporationBlueprints,
    };
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(kind, subject),
      grant: Some(grant),
      sso: None,
    }
  }

  fn bpo_json(item_id: i64, type_id: i32) -> serde_json::Value {
    serde_json::json!({
      "item_id": item_id,
      "type_id": type_id,
      "location_id": 60_003_760,
      "location_flag": "Hangar",
      "quantity": -1,
      "material_efficiency": 10,
      "time_efficiency": 20,
      "runs": -1
    })
  }

  fn bpc_json(item_id: i64, type_id: i32) -> serde_json::Value {
    serde_json::json!({
      "item_id": item_id,
      "type_id": type_id,
      "location_id": 60_003_760,
      "location_flag": "Hangar",
      "quantity": 1,
      "material_efficiency": 2,
      "time_efficiency": 4,
      "runs": 300
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
    async fn it_full_replaces_the_character_blueprints_table() {
      let server = MockServer::start().await;
      mount_character_blueprints(
        &server,
        CHARACTER_ID,
        serde_json::json!([bpo_json(1_000_000_000_001, 962), bpc_json(1_000_000_000_002, 963)]),
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
      let rows = blueprints::list_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(rows.len(), 2);
      let bpo = rows.iter().find(|row| row.item_id() == 1_000_000_000_001).unwrap();
      assert_eq!(bpo.material_efficiency(), 10);
      assert_eq!(bpo.time_efficiency(), 20);
      assert_eq!(bpo.runs(), -1);
    }

    #[tokio::test]
    async fn it_reports_empty_when_the_character_has_no_blueprints() {
      let server = MockServer::start().await;
      mount_character_blueprints(&server, CHARACTER_ID, serde_json::json!([])).await;
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
        .and(path(format!("/characters/{CHARACTER_ID}/blueprints/")))
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
    async fn it_full_replaces_the_corporation_blueprints_table_when_the_role_is_held() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Factory_Manager"] }]),
      )
      .await;
      mount_corporation_blueprints(&server, serde_json::json!([bpo_json(1_000_000_000_003, 964)])).await;
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
      let rows = blueprints::list_all(&db).await.unwrap().corporation_blueprints;
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].type_id(), 964);
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

    #[tokio::test]
    async fn it_skips_honestly_when_the_authorizing_character_lacks_the_role() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Hangar_Take_1"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/blueprints/")))
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
      assert!(
        blueprints::list_all(&db)
          .await
          .unwrap()
          .corporation_blueprints
          .is_empty()
      );
    }

    #[tokio::test]
    async fn it_skips_honestly_when_the_blueprints_endpoint_is_forbidden() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Factory_Manager"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/blueprints/")))
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
        "a 403 from the blueprints endpoint is an honest skip, got {outcome:?}"
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
        .and(path(format!("/corporations/{CORP}/blueprints/")))
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
  }
}
