use crate::{
  clients::{self, Error, esi::scopes, eve_sso::Grant},
  store::{
    model::{CharacterTelemetry, Race},
    repo::{character, sde},
  },
  sync::{job::JobCtx, jobs::resolve::resolve_item_type, outcome::Outcome, structure_resolution, subject::Subject},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character telemetry job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authenticated = ctx.esi.character_authenticated(grant);

  let location = authenticated.location().await?;
  let online = authenticated.online().await?;
  let ship = authenticated.ship().await?;

  resolve_solar_system(ctx, location.solar_system_id).await?;
  if let Some(station_id) = location.station_id {
    resolve_station(ctx, station_id).await?;
  }
  if let Some(structure_id) = location.structure_id {
    resolve_structure(ctx, grant, structure_id).await?;
  }

  let synced_at = now_secs();
  let telemetry = CharacterTelemetry::from((character_id, online, location, Some(ship), synced_at));
  character::upsert_telemetry(ctx.db, &telemetry).await?;
  Ok(Outcome::Synced {
    rows_touched: 1,
  })
}

async fn resolve_solar_system(ctx: &JobCtx<'_>, system_id: i64) -> Result<(), Error> {
  if sde::get_solar_system(ctx.db, system_id).await?.is_some() {
    return Ok(());
  }
  let system = ctx.esi.universe().solar_system(system_id).await?;
  let constellation = ctx.esi.universe().constellation(system.constellation_id).await?;
  let region = ctx.esi.universe().region(constellation.region_id).await?;
  sde::upsert_region(ctx.db, &region.into()).await?;
  sde::upsert_constellation(ctx.db, &constellation.into()).await?;
  sde::upsert_solar_system(ctx.db, &system.into()).await?;
  Ok(())
}

async fn resolve_station(ctx: &JobCtx<'_>, station_id: i64) -> Result<(), Error> {
  if sde::get_station(ctx.db, station_id).await?.is_some() {
    tracing::debug!(station_id, "resolved station from db");
    return Ok(());
  }
  tracing::debug!(station_id, "fetching station from esi");
  let station = ctx.esi.universe().station(station_id).await?;
  resolve_item_type(ctx, i64::from(station.type_id)).await?;
  if let Some(owner_id) = station.owner {
    structure_resolution::resolve_owner_corporation(ctx, owner_id).await?;
  }
  if let Some(race_id) = station.race_id {
    resolve_race(ctx, i64::from(race_id)).await?;
  }
  let system = ctx.esi.universe().solar_system(station.system_id).await?;
  let constellation = ctx.esi.universe().constellation(system.constellation_id).await?;
  let region = ctx.esi.universe().region(constellation.region_id).await?;
  sde::insert_station_with_geography(
    ctx.db,
    &station.into(),
    &system.into(),
    &constellation.into(),
    &region.into(),
  )
  .await?;
  Ok(())
}

async fn resolve_race(ctx: &JobCtx<'_>, race_id: i64) -> Result<(), Error> {
  if sde::get_race(ctx.db, race_id).await?.is_some() {
    tracing::debug!(race_id, "resolved race from db");
    return Ok(());
  }
  let race = resolve_race_model(ctx, race_id).await?;
  sde::upsert_race(ctx.db, &race).await?;
  Ok(())
}

async fn resolve_race_model(ctx: &JobCtx<'_>, race_id: i64) -> Result<Race, Error> {
  if let Some(race) = sde::get_race(ctx.db, race_id).await? {
    tracing::debug!(race_id, "resolved race from db");
    return Ok(race);
  }
  let lookup_id =
    i32::try_from(race_id).map_err(|_| Error::Internal(format!("race id {race_id} out of range for ESI lookup")))?;
  tracing::debug!(race_id, "fetching race from esi");
  ctx
    .esi
    .races()
    .list()
    .await?
    .into_iter()
    .find(|race| race.race_id == lookup_id)
    .map(Race::from)
    .ok_or_else(|| Error::Internal(format!("race {race_id} not in /universe/races")))
}

async fn resolve_structure(ctx: &JobCtx<'_>, grant: &Grant, structure_id: i64) -> Result<(), Error> {
  if sde::get_structure(ctx.db, structure_id).await?.is_some() {
    tracing::debug!(structure_id, "resolved structure from db");
    return Ok(());
  }
  if !grant.has_scope(scopes::UNIVERSE_STRUCTURES) {
    tracing::debug!(
      structure_id,
      "skipping structure resolution: grant lacks read_structures scope"
    );
    return Ok(());
  }
  tracing::debug!(structure_id, "fetching structure from esi");
  match ctx.esi.universe().structure(structure_id, grant).await {
    Ok(structure) => {
      structure_resolution::resolve_owner_corporation(ctx, structure.owner_id).await?;
      if let Some(type_id) = structure.type_id {
        resolve_item_type(ctx, i64::from(type_id)).await?;
      }
      sde::upsert_structure(ctx.db, &(structure_id, structure).into()).await?;
      Ok(())
    }
    Err(clients::Error::Http(error)) if error.status() == Some(reqwest::StatusCode::FORBIDDEN) => {
      tracing::warn!(structure_id, "structure not visible (403); leaving name unresolved");
      Ok(())
    }
    Err(error) => Err(error),
  }
}

fn now_secs() -> i64 {
  use std::time::{SystemTime, UNIX_EPOCH};
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, eve_image, http},
    store::{
      self, images,
      repo::{org, sde},
    },
    sync::job::{JobKey, JobKind},
  };

  async fn mount_json(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
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

  async fn mount_location(server: &MockServer, character_id: i64, body: serde_json::Value) {
    mount_json(server, &format!("/characters/{character_id}/location/"), body).await;
  }

  async fn mount_online(server: &MockServer, character_id: i64, online: bool) {
    mount_json(
      server,
      &format!("/characters/{character_id}/online/"),
      serde_json::json!({ "online": online }),
    )
    .await;
  }

  async fn mount_ship(server: &MockServer, character_id: i64) {
    mount_json(
      server,
      &format!("/characters/{character_id}/ship/"),
      serde_json::json!({ "ship_item_id": 1_000_000_016_991_i64, "ship_name": "My Rifter", "ship_type_id": 587 }),
    )
    .await;
  }

  async fn mount_system_geography(server: &MockServer) {
    mount_json(
      server,
      "/universe/systems/30000142/",
      serde_json::json!({
        "constellation_id": 20000020, "name": "Jita", "position": { "x": 1.0, "y": 2.0, "z": 3.0 },
        "security_status": 0.946, "system_id": 30000142,
      }),
    )
    .await;
    mount_json(
      server,
      "/universe/constellations/20000020/",
      serde_json::json!({
        "constellation_id": 20000020, "name": "Kimotoro", "position": { "x": 1.0, "y": 2.0, "z": 3.0 },
        "region_id": 10000002, "systems": [30000142],
      }),
    )
    .await;
    mount_json(
      server,
      "/universe/regions/10000002/",
      serde_json::json!({
        "constellations": [20000020], "description": "The Forge.", "name": "The Forge", "region_id": 10000002,
      }),
    )
    .await;
  }

  async fn mount_station_hull_type(server: &MockServer) {
    mount_json(
      server,
      "/universe/types/1529/",
      serde_json::json!({
        "description": "A station.", "group_id": 15, "market_group_id": 1500, "name": "Caldari Station",
        "published": true, "type_id": 1529,
      }),
    )
    .await;
    mount_json(
      server,
      "/universe/groups/15/",
      serde_json::json!({ "category_id": 3, "group_id": 15, "name": "Station", "published": true, "types": [1529] }),
    )
    .await;
    mount_json(
      server,
      "/universe/categories/3/",
      serde_json::json!({ "category_id": 3, "groups": [15], "name": "Station", "published": true }),
    )
    .await;
  }

  async fn mount_market_groups(server: &MockServer) {
    mount_json(
      server,
      "/markets/groups/1500/",
      serde_json::json!({
        "description": "Stations.", "market_group_id": 1500, "name": "Stations",
        "parent_group_id": 1499, "types": [1529],
      }),
    )
    .await;
    mount_json(
      server,
      "/markets/groups/1499/",
      serde_json::json!({
        "description": "Structures.", "market_group_id": 1499, "name": "Structures", "types": [],
      }),
    )
    .await;
  }

  async fn mount_owner_corporation(server: &MockServer) {
    mount_json(
      server,
      "/corporations/1000035/",
      serde_json::json!({
        "ceo_id": 3004029, "creator_id": 3004029, "member_count": 10000, "name": "Caldari Navy",
        "tax_rate": 0.0, "ticker": "CN",
      }),
    )
    .await;
    mount_json(
      server,
      "/characters/3004029/",
      serde_json::json!({
        "birthday": "2003-01-01T00:00:00Z", "bloodline_id": 5, "corporation_id": 1000035,
        "gender": "male", "name": "Caldari Navy CEO", "race_id": 1,
      }),
    )
    .await;
  }

  async fn mount_races(server: &MockServer) {
    mount_json(
      server,
      "/universe/races/",
      serde_json::json!([
        { "alliance_id": 500001, "description": "The Caldari.", "name": "Caldari", "race_id": 1 },
      ]),
    )
    .await;
  }

  async fn mount_bloodlines(server: &MockServer) {
    mount_json(
      server,
      "/universe/bloodlines/",
      serde_json::json!([
        { "bloodline_id": 5, "charisma": 6, "corporation_id": 1000035, "description": "The Civire.",
          "intelligence": 7, "memory": 5, "name": "Civire", "perception": 5, "race_id": 1,
          "ship_type_id": 601, "willpower": 5 },
      ]),
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
      key: JobKey::new(JobKind::CharacterTelemetry, Subject::Character(character_id)),
      grant: Some(grant),
      sso: None,
    }
  }

  mod resolve_race_model {
    use super::*;

    #[tokio::test]
    async fn it_fetches_a_race_from_esi_when_absent_from_the_db() {
      let server = MockServer::start().await;
      mount_races(&server).await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      let race = super::super::resolve_race_model(&ctx, 1).await.unwrap();

      assert_eq!(race.name(), "Caldari");
    }

    #[tokio::test]
    async fn it_returns_a_db_cached_race_without_fetching() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();
      sde::upsert_race(&db, &Race::new(1, 500_001, "The Caldari.", "Caldari"))
        .await
        .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 1);

      let race = super::super::resolve_race_model(&ctx, 1).await.unwrap();

      assert_eq!(race.name(), "Caldari");
    }
  }

  mod run {
    use super::*;

    #[tokio::test]
    async fn it_errors_and_persists_nothing_when_the_location_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/location/"))
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
      assert!(character::telemetry(&db, 42).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_persists_a_telemetry_snapshot_resolving_location_names() {
      let server = MockServer::start().await;
      mount_location(
        &server,
        42,
        serde_json::json!({ "solar_system_id": 30000142, "station_id": 60003760 }),
      )
      .await;
      mount_online(&server, 42, true).await;
      mount_ship(&server, 42).await;
      mount_system_geography(&server).await;
      mount_json(
        &server,
        "/universe/stations/60003760/",
        serde_json::json!({
          "max_dockable_ship_volume": 50000000.0, "name": "Jita IV - Moon 4 - Caldari Navy Assembly Plant",
          "office_rental_cost": 10000.0, "owner": 1000035, "position": { "x": 1.0, "y": 2.0, "z": 3.0 },
          "race_id": 1, "reprocessing_efficiency": 0.5, "reprocessing_stations_take": 0.05, "services": [],
          "station_id": 60003760, "system_id": 30000142, "type_id": 1529,
        }),
      )
      .await;
      mount_station_hull_type(&server).await;
      mount_market_groups(&server).await;
      mount_owner_corporation(&server).await;
      mount_races(&server).await;
      mount_bloodlines(&server).await;
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

      let telemetry = character::telemetry(&db, 42)
        .await
        .unwrap()
        .expect("telemetry persisted");
      assert_eq!(telemetry.solar_system_id(), 30000142);
      assert_eq!(telemetry.station_id(), Some(60003760));
      assert!(telemetry.online());
      assert!(sde::get_solar_system(&db, 30000142).await.unwrap().is_some());
      assert!(sde::get_station(&db, 60003760).await.unwrap().is_some());
      assert!(org::get_corporation(&db, 1000035).await.unwrap().is_some());
      assert!(sde::get_race(&db, 1).await.unwrap().is_some());
      assert!(sde::get_market_group(&db, 1500).await.unwrap().is_some());
      assert!(sde::get_market_group(&db, 1499).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_resolves_a_visible_structures_type_before_persisting_it() {
      let server = MockServer::start().await;
      mount_location(
        &server,
        42,
        serde_json::json!({ "solar_system_id": 30000142, "structure_id": 1021000000000_i64 }),
      )
      .await;
      mount_online(&server, 42, true).await;
      mount_ship(&server, 42).await;
      mount_system_geography(&server).await;
      mount_json(
        &server,
        "/universe/structures/1021000000000/",
        serde_json::json!({
          "name": "A Player Structure", "owner_id": 1000035, "position": { "x": 1.0, "y": 2.0, "z": 3.0 },
          "solar_system_id": 30000142, "type_id": 35833,
        }),
      )
      .await;
      mount_json(
        &server,
        "/universe/types/35833/",
        serde_json::json!({
          "description": "An Astrahus.", "group_id": 1657, "market_group_id": 1500, "name": "Astrahus",
          "published": true, "type_id": 35833,
        }),
      )
      .await;
      mount_json(
        &server,
        "/universe/groups/1657/",
        serde_json::json!({ "category_id": 65, "group_id": 1657, "name": "Citadel", "published": true, "types": [35833] }),
      )
      .await;
      mount_json(
        &server,
        "/universe/categories/65/",
        serde_json::json!({ "category_id": 65, "groups": [1657], "name": "Structure", "published": true }),
      )
      .await;
      mount_market_groups(&server).await;
      mount_owner_corporation(&server).await;
      mount_races(&server).await;
      mount_bloodlines(&server).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes("token", 42, vec![scopes::UNIVERSE_STRUCTURES.to_owned()]);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      assert!(sde::get_structure(&db, 1021000000000).await.unwrap().is_some());
      assert!(sde::get_item_type(&db, 35833).await.unwrap().is_some());
      assert!(sde::get_market_group(&db, 1500).await.unwrap().is_some());
      assert!(org::get_corporation(&db, 1000035).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_resolves_the_solar_system_from_the_db_without_refetching() {
      let server = MockServer::start().await;
      mount_location(&server, 42, serde_json::json!({ "solar_system_id": 30000142 })).await;
      mount_online(&server, 42, true).await;
      mount_ship(&server, 42).await;
      Mock::given(method("GET"))
        .and(path("/universe/systems/30000142/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "constellation_id": 20000020, "name": "Jita", "position": { "x": 1.0, "y": 2.0, "z": 3.0 },
          "security_status": 0.946, "system_id": 30000142,
        })))
        .expect(1)
        .mount(&server)
        .await;
      mount_json(
        &server,
        "/universe/constellations/20000020/",
        serde_json::json!({
          "constellation_id": 20000020, "name": "Kimotoro", "position": { "x": 1.0, "y": 2.0, "z": 3.0 },
          "region_id": 10000002, "systems": [30000142],
        }),
      )
      .await;
      mount_json(
        &server,
        "/universe/regions/10000002/",
        serde_json::json!({
          "constellations": [20000020], "description": "The Forge.", "name": "The Forge", "region_id": 10000002,
        }),
      )
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
      run(&ctx).await.unwrap();

      assert!(character::telemetry(&db, 42).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_skips_structure_resolution_when_the_grant_lacks_the_scope() {
      let server = MockServer::start().await;
      mount_location(
        &server,
        42,
        serde_json::json!({ "solar_system_id": 30000142, "structure_id": 1045971617379_i64 }),
      )
      .await;
      mount_online(&server, 42, true).await;
      mount_ship(&server, 42).await;
      mount_system_geography(&server).await;
      Mock::given(method("GET"))
        .and(path("/universe/structures/1045971617379/"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
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

      let telemetry = character::telemetry(&db, 42)
        .await
        .unwrap()
        .expect("telemetry persisted");
      assert_eq!(telemetry.structure_id(), Some(1045971617379));
      assert!(sde::get_structure(&db, 1045971617379).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/location/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "solar_system_id": 30000142 })))
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
      assert!(character::telemetry(&db, 42).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_tolerates_an_unresolvable_structure_without_failing() {
      let server = MockServer::start().await;
      mount_location(
        &server,
        42,
        serde_json::json!({ "solar_system_id": 30000142, "structure_id": 1021000000000_i64 }),
      )
      .await;
      mount_online(&server, 42, true).await;
      mount_ship(&server, 42).await;
      mount_system_geography(&server).await;
      Mock::given(method("GET"))
        .and(path("/universe/structures/1021000000000/"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes("token", 42, vec![scopes::UNIVERSE_STRUCTURES.to_owned()]);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      let telemetry = character::telemetry(&db, 42)
        .await
        .unwrap()
        .expect("telemetry persisted");
      assert_eq!(telemetry.structure_id(), Some(1021000000000));
      assert!(sde::get_structure(&db, 1021000000000).await.unwrap().is_none());
    }
  }
}
