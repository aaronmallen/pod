use std::collections::{HashMap, HashSet};

use crate::{
  clients::{self, Error, esi::scopes, eve_image::Size, eve_sso::Grant},
  store::{
    model::{CharacterClone, CharacterCloneImplant, CharacterJumpClone},
    repo::character,
  },
  sync::{job::JobCtx, jobs::names::resolve_names, outcome::Outcome, subject::Subject},
};

const ICON_SIZE: Size = Size::S64;
const LOCATION_TYPE_STRUCTURE: &str = "structure";

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character clones job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authenticated = ctx.esi.character_authenticated(grant);

  let clones = authenticated.clones().await?;
  let active_implant_type_ids = authenticated.implants().await?;

  let home_location_id = clones.home_location.location_id.ok_or_else(|| {
    Error::Internal(format!(
      "character {character_id} clones response missing home location_id"
    ))
  })?;
  let home_location_type = clones.home_location.location_type.ok_or_else(|| {
    Error::Internal(format!(
      "character {character_id} clones response missing home location_type"
    ))
  })?;

  let mut station_ids: Vec<i64> = Vec::new();
  let mut structure_ids: HashSet<i64> = HashSet::new();
  collect_location(
    &home_location_type,
    home_location_id,
    &mut station_ids,
    &mut structure_ids,
  );
  for clone in &clones.jump_clones {
    collect_location(
      &clone.location_type,
      clone.location_id,
      &mut station_ids,
      &mut structure_ids,
    );
  }
  let station_names = resolve_names(ctx, &station_ids).await?;
  let mut structure_names: HashMap<i64, String> = HashMap::new();
  for structure_id in structure_ids {
    if let Some(name) = resolve_structure_name(ctx, grant, structure_id).await? {
      structure_names.insert(structure_id, name);
    }
  }

  let location_name = |location_type: &str, location_id: i64| -> Option<String> {
    if location_type == LOCATION_TYPE_STRUCTURE {
      structure_names.get(&location_id).cloned()
    } else {
      station_names.get(&location_id).map(|record| record.name.clone())
    }
  };

  let mut implant_type_ids: HashSet<i64> = active_implant_type_ids.iter().copied().collect();
  for clone in &clones.jump_clones {
    implant_type_ids.extend(clone.implants.iter().map(|&id| i64::from(id)));
  }
  let mut implant_meta: HashMap<i64, (String, Option<String>)> = HashMap::with_capacity(implant_type_ids.len());
  for type_id in implant_type_ids {
    implant_meta.insert(type_id, resolve_implant(ctx, type_id).await?);
  }
  let implant_row = |character_id: i64, clone_id: Option<i64>, type_id: i64| -> CharacterCloneImplant {
    let (name, icon) = implant_meta.get(&type_id).cloned().unwrap_or_default();
    CharacterCloneImplant {
      character_id,
      clone_id,
      icon,
      name,
      type_id,
    }
  };

  let active = CharacterClone {
    character_id,
    home_location_id,
    home_location_name: location_name(&home_location_type, home_location_id),
    home_location_type,
    last_clone_jump_date: clones.last_clone_jump_date,
    last_station_change_date: clones.last_station_change_date,
  };

  let mut jump_clones = Vec::with_capacity(clones.jump_clones.len());
  let mut implants: Vec<CharacterCloneImplant> = active_implant_type_ids
    .iter()
    .map(|&type_id| implant_row(character_id, None, type_id))
    .collect();
  for clone in clones.jump_clones {
    jump_clones.push(CharacterJumpClone {
      character_id,
      jump_clone_id: clone.jump_clone_id,
      location_id: clone.location_id,
      location_name: location_name(&clone.location_type, clone.location_id),
      location_type: clone.location_type,
      name: clone.name,
    });
    implants.extend(
      clone
        .implants
        .iter()
        .map(|&type_id| implant_row(character_id, Some(clone.jump_clone_id), i64::from(type_id))),
    );
  }

  character::replace_clones_for_character(ctx.db, character_id, &active, &jump_clones, &implants).await?;
  Ok(Outcome::from_rows(1 + jump_clones.len() + implants.len()))
}

async fn cache_implant_icon(ctx: &JobCtx<'_>, type_id: i64) -> Option<String> {
  let icon_path = ctx.image_store.type_icon_path(type_id, ICON_SIZE);
  if icon_path.exists() {
    return Some(icon_path.to_string_lossy().into_owned());
  }

  let icon_url = ctx.image.type_icon_url(type_id, ICON_SIZE);
  let icon_bytes = match ctx.image.fetch(&icon_url).await {
    Ok(bytes) => bytes,
    Err(error) => {
      tracing::warn!(
        type_id,
        "clone implant icon fetch failed; leaving icon unresolved: {error}"
      );
      return None;
    }
  };

  match ctx.image_store.write(&icon_path, &icon_bytes) {
    Ok(()) => Some(icon_path.to_string_lossy().into_owned()),
    Err(error) => {
      tracing::warn!(
        type_id,
        "clone implant icon write failed; leaving icon unresolved: {error}"
      );
      None
    }
  }
}

fn collect_location(location_type: &str, location_id: i64, stations: &mut Vec<i64>, structures: &mut HashSet<i64>) {
  if location_type == LOCATION_TYPE_STRUCTURE {
    structures.insert(location_id);
  } else {
    stations.push(location_id);
  }
}

async fn resolve_implant(ctx: &JobCtx<'_>, type_id: i64) -> Result<(String, Option<String>), Error> {
  let lookup_id =
    i32::try_from(type_id).map_err(|_| Error::Internal(format!("implant type id {type_id} out of range for ESI")))?;
  let item_type = ctx.esi.universe().item_type(lookup_id).await?;

  Ok((item_type.name, cache_implant_icon(ctx, type_id).await))
}

async fn resolve_structure_name(ctx: &JobCtx<'_>, grant: &Grant, structure_id: i64) -> Result<Option<String>, Error> {
  if let Some(structure) = crate::store::repo::sde::get_structure(ctx.db, structure_id).await? {
    return Ok(Some(structure.name().clone()));
  }
  if !grant.has_scope(scopes::UNIVERSE_STRUCTURES) {
    tracing::debug!(
      structure_id,
      "skipping clone structure name: grant lacks read_structures scope"
    );
    return Ok(None);
  }
  match ctx.esi.universe().structure(structure_id, grant).await {
    Ok(structure) => Ok(Some(structure.name)),
    Err(clients::Error::Http(error)) if error.status() == Some(reqwest::StatusCode::FORBIDDEN) => {
      tracing::warn!(
        structure_id,
        "clone structure not visible (403); leaving name unresolved"
      );
      Ok(None)
    }
    Err(error) => Err(error),
  }
}

#[cfg(test)]
#[allow(clippy::needless_raw_string_hashes)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, eve_image, http},
    store::{self, images},
    sync::job::{JobKey, JobKind},
  };

  async fn mount_json(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn mount_names(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("POST"))
      .and(path("/universe/names/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn mount_icon(server: &MockServer, type_id: i64) {
    Mock::given(method("GET"))
      .and(path(format!("/types/{type_id}/icon")))
      .respond_with(ResponseTemplate::new(200).set_body_raw(vec![1u8, 2, 3], "image/png"))
      .mount(server)
      .await;
  }

  async fn mount_item_type(server: &MockServer, type_id: i64, name: &str) {
    mount_json(
      server,
      &format!("/universe/types/{type_id}/"),
      serde_json::json!({
        "description": "An implant.", "group_id": 300, "name": name, "published": true, "type_id": type_id,
      }),
    )
    .await;
  }

  async fn seed_character(db: &store::Database, id: i64) {
    use store::{
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::character,
    };
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

  async fn mount_clones(server: &MockServer, character_id: i64) {
    mount_json(
      server,
      &format!("/characters/{character_id}/clones/"),
      serde_json::json!({
        "home_location": { "location_id": 60_003_760, "location_type": "station" },
        "jump_clones": [
          { "implants": [9941], "jump_clone_id": 1, "location_id": 60_008_494, "location_type": "station",
            "name": "Backup" },
        ],
        "last_clone_jump_date": "2026-01-01T00:00:00Z",
        "last_station_change_date": "2026-02-01T00:00:00Z",
      }),
    )
    .await;
    mount_json(
      server,
      &format!("/characters/{character_id}/implants/"),
      serde_json::json!([9899]),
    )
    .await;
  }

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: &'a Grant,
    character_id: i64,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::CharacterClones, Subject::Character(character_id)),
      grant: Some(grant),
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_the_full_clone_picture_with_names_and_icons() {
      let server = MockServer::start().await;
      mount_clones(&server, 42).await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "station", "id": 60_003_760, "name": "Jita IV - Moon 4" },
          { "category": "station", "id": 60_008_494, "name": "Amarr VIII" },
        ]),
      )
      .await;
      mount_item_type(&server, 9899, "Memory Augmentation").await;
      mount_item_type(&server, 9941, "Ocular Filter").await;
      mount_icon(&server, 9899).await;
      mount_icon(&server, 9941).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      let result = character::clones(&db, 42).await.unwrap().unwrap();
      assert_eq!(result.active.clone.home_location_id(), 60_003_760);
      assert_eq!(
        result.active.clone.home_location_name().as_deref(),
        Some("Jita IV - Moon 4")
      );
      assert_eq!(
        result.active.implants.iter().map(|i| i.type_id()).collect::<Vec<_>>(),
        [9899]
      );
      assert_eq!(result.active.implants[0].name(), "Memory Augmentation");
      assert!(std::path::Path::new(result.active.implants[0].icon().as_deref().unwrap()).ends_with("types/9899.png"));
      assert_eq!(result.jump_clones.len(), 1);
      assert_eq!(
        result.jump_clones[0].clone.location_name().as_deref(),
        Some("Amarr VIII")
      );
      assert_eq!(
        result.jump_clones[0]
          .implants
          .iter()
          .map(|i| i.type_id())
          .collect::<Vec<_>>(),
        [9941]
      );
      assert_eq!(
        std::fs::read(image_store.type_icon_path(9899, ICON_SIZE)).unwrap(),
        vec![1u8, 2, 3]
      );
    }

    #[tokio::test]
    async fn it_aborts_without_writing_when_the_clones_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/clones/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(character::clones(&db, 42).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_completes_and_resolves_the_implant_name_when_an_icon_fetch_fails() {
      let server = MockServer::start().await;
      mount_clones(&server, 42).await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "station", "id": 60_003_760, "name": "Jita IV - Moon 4" },
          { "category": "station", "id": 60_008_494, "name": "Amarr VIII" },
        ]),
      )
      .await;
      mount_item_type(&server, 9899, "Memory Augmentation").await;
      mount_item_type(&server, 9941, "Ocular Filter").await;
      Mock::given(method("GET"))
        .and(path("/types/9899/icon"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/types/9941/icon"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      let result = character::clones(&db, 42).await.unwrap().unwrap();
      assert_eq!(result.active.implants[0].name(), "Memory Augmentation");
      assert!(
        result.active.implants[0].icon().is_none(),
        "the failed icon degrades to None"
      );
      assert!(!image_store.type_icon_path(9899, ICON_SIZE).exists());
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/clones/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "home_location": { "location_id": 60_003_760, "location_type": "station" },
        })))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
      assert!(character::clones(&db, 42).await.unwrap().is_none());
    }
  }

  mod resolve_structure_name {
    use super::*;

    const STRUCTURE_ID: i64 = 1_021_000_000_000;

    struct Harness {
      db: store::Database,
      esi: esi::Client,
      image: eve_image::Client,
      image_store: images::Store,
      _images_dir: tempfile::TempDir,
    }

    async fn harness(server: &MockServer) -> Harness {
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      Harness {
        db,
        esi,
        image,
        image_store,
        _images_dir: images_dir,
      }
    }

    fn ctx<'a>(h: &'a Harness, grant: &'a Grant) -> JobCtx<'a> {
      ctx_with_grant(&h.db, &h.esi, &h.image, &h.image_store, grant, 42)
    }

    async fn seed_structure(db: &store::Database, name: &str) {
      use store::{model, repo::sde};
      let owner_id = 90_000_001;
      seed_character(db, 1).await;
      sde::upsert_region(
        db,
        &model::Region {
          description: None,
          id: 10_000_002,
          name: "The Forge".to_owned(),
        },
      )
      .await
      .unwrap();
      sde::upsert_constellation(
        db,
        &model::Constellation {
          id: 20_000_020,
          name: "Kimotoro".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          region_id: 10_000_002,
        },
      )
      .await
      .unwrap();
      sde::upsert_solar_system(
        db,
        &model::SolarSystem {
          constellation_id: 20_000_020,
          id: 30_000_142,
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
      sde::upsert_structure(
        db,
        &model::Structure {
          id: STRUCTURE_ID,
          name: name.to_owned(),
          owner_id,
          position_x: None,
          position_y: None,
          position_z: None,
          solar_system_id: 30_000_142,
          type_id: None,
        },
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_returns_the_cached_name_without_fetching() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
      let h = harness(&server).await;
      seed_structure(&h.db, "Cached Citadel").await;
      let grant = Grant::new_test_with_scopes("token", 42, vec![scopes::UNIVERSE_STRUCTURES.to_owned()]);

      let name = resolve_structure_name(&ctx(&h, &grant), &grant, STRUCTURE_ID)
        .await
        .unwrap();

      assert_eq!(name.as_deref(), Some("Cached Citadel"));
    }

    #[tokio::test]
    async fn it_skips_silently_when_the_grant_lacks_the_scope() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
      let h = harness(&server).await;
      let grant = Grant::new_test("token", 42);

      let name = resolve_structure_name(&ctx(&h, &grant), &grant, STRUCTURE_ID)
        .await
        .unwrap();

      assert!(name.is_none());
    }

    #[tokio::test]
    async fn it_fetches_a_visible_structure_name_from_esi() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "name": "A Player Structure", "owner_id": 1_000_035, "solar_system_id": 30_000_142,
        })))
        .mount(&server)
        .await;
      let h = harness(&server).await;
      let grant = Grant::new_test_with_scopes("token", 42, vec![scopes::UNIVERSE_STRUCTURES.to_owned()]);

      let name = resolve_structure_name(&ctx(&h, &grant), &grant, STRUCTURE_ID)
        .await
        .unwrap();

      assert_eq!(name.as_deref(), Some("A Player Structure"));
    }

    #[tokio::test]
    async fn it_treats_a_403_as_unresolved() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let h = harness(&server).await;
      let grant = Grant::new_test_with_scopes("token", 42, vec![scopes::UNIVERSE_STRUCTURES.to_owned()]);

      let name = resolve_structure_name(&ctx(&h, &grant), &grant, STRUCTURE_ID)
        .await
        .unwrap();

      assert!(name.is_none());
    }

    #[tokio::test]
    async fn it_propagates_a_non_403_error() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let h = harness(&server).await;
      let grant = Grant::new_test_with_scopes("token", 42, vec![scopes::UNIVERSE_STRUCTURES.to_owned()]);

      let result = resolve_structure_name(&ctx(&h, &grant), &grant, STRUCTURE_ID).await;

      assert!(result.is_err());
    }
  }
}
