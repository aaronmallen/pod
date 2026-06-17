use reqwest::StatusCode;

use crate::{
  clients::{Error, esi::models::corporation::CorporationStructure, eve_sso::Grant},
  store::{
    model::{OwnerType, Structure},
    repo::{infra, org, sde},
  },
  sync::{
    job::JobCtx,
    jobs::resolve::resolve_item_type,
    outcome::Outcome,
    structure_resolution::{resolve_owner_corporation, resolve_solar_system},
    subject::Subject,
  },
};

/// Roles that grant access to `/corporations/{id}/structures`: Station_Manager is the specific gate, Director the
/// superset.
const STRUCTURE_MANAGER_ROLES: &[&str] = &["Director", "Station_Manager"];

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  match ctx.key.subject {
    Subject::Character(character_id) => Err(Error::Internal(format!(
      "corporation structures job received a character subject {character_id}"
    ))),
    Subject::Corporation(corporation_id) => run_corporation(ctx, corporation_id).await,
  }
}

async fn run_corporation(ctx: &JobCtx<'_>, corporation_id: i64) -> Result<Outcome, Error> {
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "corporation structures job for {corporation_id} requires a grant"
    )));
  };
  if org::get_corporation(ctx.db, corporation_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authorized_by = authorizing_character(ctx, corporation_id).await?;
  if !holds_structure_manager_role(ctx, grant, corporation_id, authorized_by).await? {
    return Ok(Outcome::Skipped {
      reason: format!(
        "authorizing character {authorized_by} lacks the Station_Manager role in corporation {corporation_id}"
      ),
    });
  }

  let authenticated = ctx.esi.corporation_authenticated(grant);
  let structures = match authenticated.structures(corporation_id).await {
    Ok(structures) => structures,
    Err(error) if is_forbidden(&error) => {
      tracing::warn!(
        corporation_id,
        "skipping corporation structures: forbidden (Station_Manager role required)"
      );
      return Ok(Outcome::Skipped {
        reason: format!("corporation {corporation_id} structures are forbidden (missing Station_Manager role)"),
      });
    }
    Err(error) => return Err(reauth_error(error, corporation_id)),
  };

  let mut persisted = 0;
  for structure in &structures {
    if persist_structure(ctx, corporation_id, structure).await? {
      persisted += 1;
    }
  }
  Ok(Outcome::from_rows(persisted))
}

/// Records a 403/404 on the structure's FK references (system/type/owner) as inaccessible so it is not re-hammered
/// every sync, rather than failing the whole job.
async fn persist_structure(
  ctx: &JobCtx<'_>,
  corporation_id: i64,
  structure: &CorporationStructure,
) -> Result<bool, Error> {
  match resolve_references(ctx, structure).await {
    Ok(()) => {
      sde::upsert_structure(ctx.db, &to_structure(corporation_id, structure)).await?;
      Ok(true)
    }
    Err(Error::Http(error)) if is_access_miss(&error) => {
      tracing::warn!(
        structure_id = structure.structure_id,
        "structure references are inaccessible (403/404); recording as unresolvable"
      );
      sde::mark_inaccessible_structure(ctx.db, corporation_id, OwnerType::Corporation, structure.structure_id).await?;
      Ok(false)
    }
    Err(error) => Err(error),
  }
}

async fn resolve_references(ctx: &JobCtx<'_>, structure: &CorporationStructure) -> Result<(), Error> {
  resolve_owner_corporation(ctx, structure.corporation_id).await?;
  resolve_solar_system(ctx, structure.system_id).await?;
  resolve_item_type(ctx, i64::from(structure.type_id)).await?;
  Ok(())
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

async fn holds_structure_manager_role(
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
          .any(|role| STRUCTURE_MANAGER_ROLES.contains(&role.as_str()))
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

fn to_structure(corporation_id: i64, structure: &CorporationStructure) -> Structure {
  Structure {
    id: structure.structure_id,
    name: structure
      .name
      .clone()
      // structures.name is NOT NULL, but the corp list may omit it; synthesize a placeholder.
      .unwrap_or_else(|| format!("Structure {}", structure.structure_id)),
    owner_id: corporation_id,
    position_x: None,
    position_y: None,
    position_z: None,
    solar_system_id: structure.system_id,
    type_id: Some(i64::from(structure.type_id)),
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

  const TYPE_ID: i64 = 35_833;

  const STRUCTURE_ID: i64 = 1_021_000_000_001;

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

  async fn mount_item_type(server: &MockServer) {
    Mock::given(method("GET"))
      .and(path(format!("/universe/types/{TYPE_ID}/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "description": "A citadel.", "group_id": 1657, "name": "Astrahus", "published": true, "type_id": TYPE_ID,
      })))
      .mount(server)
      .await;
    Mock::given(method("GET"))
      .and(path("/universe/groups/1657/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "category_id": 65, "group_id": 1657, "name": "Citadel", "published": true, "types": [TYPE_ID],
      })))
      .mount(server)
      .await;
    Mock::given(method("GET"))
      .and(path("/universe/categories/65/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "category_id": 65, "groups": [1657], "name": "Structure", "published": true,
      })))
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

  async fn mount_structures(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/corporations/{CORP}/structures/")))
      .respond_with(
        ResponseTemplate::new(200)
          .insert_header("X-Pages", "1")
          .set_body_json(body),
      )
      .mount(server)
      .await;
  }

  fn structure_json(structure_id: i64) -> serde_json::Value {
    serde_json::json!({
      "corporation_id": CORP,
      "structure_id": structure_id,
      "system_id": SYSTEM_ID,
      "type_id": TYPE_ID,
      "name": "Test Citadel",
      "state": "shield_vulnerable",
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
      key: JobKey::new(JobKind::CorporationStructures, Subject::Corporation(CORP)),
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
    async fn it_persists_discovered_structures_when_the_role_is_held() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Station_Manager"] }]),
      )
      .await;
      mount_structures(&server, serde_json::json!([structure_json(STRUCTURE_ID)])).await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Station_Manager").await;
      seed_geography(&db).await;
      mount_item_type(&server).await;
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

      let structure = sde::get_structure(&db, STRUCTURE_ID)
        .await
        .unwrap()
        .expect("the discovered structure is persisted");
      assert_eq!(structure.name(), "Test Citadel");
      assert_eq!(structure.owner_id(), CORP);
      assert_eq!(structure.solar_system_id(), SYSTEM_ID);
    }

    #[tokio::test]
    async fn it_records_a_structure_with_unresolvable_references_as_inaccessible() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Station_Manager"] }]),
      )
      .await;
      mount_structures(&server, serde_json::json!([structure_json(STRUCTURE_ID)])).await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/systems/{SYSTEM_ID}/")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Station_Manager").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant);

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(outcome, Outcome::Empty);
      assert!(
        sde::is_structure_inaccessible(&db, CORP, OwnerType::Corporation, STRUCTURE_ID)
          .await
          .unwrap(),
        "a structure whose references 404 is recorded inaccessible, not re-hammered"
      );
      assert!(sde::get_structure(&db, STRUCTURE_ID).await.unwrap().is_none());
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
    async fn it_skips_honestly_when_the_authorizing_character_lacks_the_role() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Hangar_Take_1"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/structures/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Hangar_Take_1").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant);

      let outcome = run(&ctx).await.unwrap();

      assert!(
        matches!(outcome, Outcome::Skipped { .. }),
        "a missing Station_Manager role is an honest skip, not a failure, got {outcome:?}"
      );
      assert!(sde::get_structure(&db, STRUCTURE_ID).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_skips_honestly_when_the_structures_endpoint_is_forbidden() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Station_Manager"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/structures/")))
        .respond_with(ResponseTemplate::new(403))
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
        "a 403 from the structures endpoint is an honest skip, got {outcome:?}"
      );
    }

    #[tokio::test]
    async fn it_surfaces_a_401_as_needs_reauthentication() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Station_Manager"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/structures/")))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Station_Manager").await;
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
}
