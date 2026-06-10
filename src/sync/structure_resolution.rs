use crate::{
  clients::{self, Error, esi, esi::scopes, eve_image, eve_sso::Grant},
  store::{
    Database, images,
    model::{Alliance, Bloodline, Character, Corporation, Faction, OwnerType, Race},
    repo::{character, org, sde},
  },
  sync::{
    job::{JobCtx, JobKey, JobKind},
    jobs::resolve::resolve_item_type,
    subject::Subject,
  },
};

const CONSTELLATION_ID_FLOOR: i64 = 20_000_000;
const REGION_ID_FLOOR: i64 = 10_000_000;
const SOLAR_SYSTEM_ID_CEIL: i64 = 32_000_000;
const SOLAR_SYSTEM_ID_FLOOR: i64 = 30_000_000;
const STRUCTURE_ID_FLOOR: i64 = 1_000_000_000_000;

enum StructureOutcome {
  Inaccessible,
  Resolved,
}

pub async fn resolve_stockpile_location(
  db: &Database,
  esi: &esi::Client,
  image: &eve_image::Client,
  image_store: &images::Store,
  grant: &Grant,
  location_id: i64,
) -> Result<(), Error> {
  if (SOLAR_SYSTEM_ID_FLOOR..SOLAR_SYSTEM_ID_CEIL).contains(&location_id) {
    return Ok(());
  }
  let ctx = JobCtx {
    db,
    esi,
    image,
    image_store,
    key: JobKey::new(JobKind::AssetSync, Subject::Character(*grant.character_id())),
    grant: Some(grant),
  };
  if (REGION_ID_FLOOR..CONSTELLATION_ID_FLOOR).contains(&location_id) {
    return resolve_region(&ctx, location_id).await;
  }
  if (CONSTELLATION_ID_FLOOR..SOLAR_SYSTEM_ID_FLOOR).contains(&location_id) {
    return resolve_constellation(&ctx, location_id).await;
  }
  if location_id >= STRUCTURE_ID_FLOOR {
    resolve_asset_references(&ctx, &[], &[], &[location_id]).await
  } else {
    resolve_asset_references(&ctx, &[], &[location_id], &[]).await
  }
}

pub async fn resolve_asset_references(
  ctx: &JobCtx<'_>,
  type_ids: &[i64],
  station_ids: &[i64],
  structure_ids: &[i64],
) -> Result<(), Error> {
  for &type_id in type_ids {
    resolve_item_type(ctx, type_id).await?;
  }

  for &station_id in station_ids {
    match resolve_station(ctx, station_id).await {
      Ok(()) => {}
      Err(Error::Http(error)) if is_access_miss(&error) => {
        tracing::warn!(station_id, "station not resolvable (403/404); leaving name unresolved");
      }
      Err(error) => return Err(error),
    }
  }

  if structure_ids.is_empty() {
    return Ok(());
  }
  let Some(grant) = ctx.grant else {
    tracing::debug!("asset subject has no grant; leaving structure names unresolved");
    return Ok(());
  };
  let (owner_id, owner_type) = match ctx.key.subject {
    Subject::Character(id) => (id, OwnerType::Character),
    Subject::Corporation(id) => (id, OwnerType::Corporation),
  };
  for &structure_id in structure_ids {
    match resolve_structure(ctx, grant, structure_id).await? {
      StructureOutcome::Resolved => {}
      StructureOutcome::Inaccessible => {
        sde::mark_inaccessible_structure(ctx.db, owner_id, owner_type, structure_id).await?;
      }
    }
  }
  Ok(())
}

async fn resolve_structure(ctx: &JobCtx<'_>, grant: &Grant, structure_id: i64) -> Result<StructureOutcome, Error> {
  if sde::get_structure(ctx.db, structure_id).await?.is_some() {
    return Ok(StructureOutcome::Resolved);
  }
  if !grant.has_scope(scopes::UNIVERSE_STRUCTURES) {
    return Ok(StructureOutcome::Resolved);
  }
  match ctx.esi.universe().structure(structure_id, grant).await {
    Ok(structure) => {
      resolve_owner_corporation(ctx, structure.owner_id).await?;
      resolve_solar_system(ctx, structure.solar_system_id).await?;
      if let Some(type_id) = structure.type_id {
        resolve_item_type(ctx, i64::from(type_id)).await?;
      }
      sde::upsert_structure(ctx.db, &(structure_id, structure).into()).await?;
      Ok(StructureOutcome::Resolved)
    }
    Err(clients::Error::Http(error)) if is_access_miss(&error) => {
      tracing::warn!(
        structure_id,
        "structure not accessible (403/404); recording as unresolvable"
      );
      Ok(StructureOutcome::Inaccessible)
    }
    Err(error) => Err(error),
  }
}

fn is_access_miss(error: &reqwest::Error) -> bool {
  matches!(
    error.status(),
    Some(reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::NOT_FOUND)
  )
}

fn is_unfetchable_character(error: &reqwest::Error) -> bool {
  matches!(
    error.status(),
    Some(reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::UNPROCESSABLE_ENTITY)
  )
}

async fn resolve_station(ctx: &JobCtx<'_>, station_id: i64) -> Result<(), Error> {
  if sde::get_station(ctx.db, station_id).await?.is_some() {
    return Ok(());
  }
  let station = ctx.esi.universe().station(station_id).await?;
  resolve_item_type(ctx, i64::from(station.type_id)).await?;
  if let Some(owner_id) = station.owner {
    resolve_owner_corporation(ctx, owner_id).await?;
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

pub async fn resolve_constellation(ctx: &JobCtx<'_>, constellation_id: i64) -> Result<(), Error> {
  if sde::get_constellation(ctx.db, constellation_id).await?.is_some() {
    return Ok(());
  }
  let constellation = ctx.esi.universe().constellation(constellation_id).await?;
  resolve_region(ctx, constellation.region_id).await?;
  sde::upsert_constellation(ctx.db, &constellation.into()).await?;
  Ok(())
}

pub async fn resolve_region(ctx: &JobCtx<'_>, region_id: i64) -> Result<(), Error> {
  if sde::get_region(ctx.db, region_id).await?.is_some() {
    return Ok(());
  }
  let region = ctx.esi.universe().region(region_id).await?;
  sde::upsert_region(ctx.db, &region.into()).await?;
  Ok(())
}

pub async fn resolve_solar_system(ctx: &JobCtx<'_>, system_id: i64) -> Result<(), Error> {
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

pub(crate) async fn resolve_owner_corporation(ctx: &JobCtx<'_>, owner_id: i64) -> Result<(), Error> {
  if org::get_corporation(ctx.db, owner_id).await?.is_some() {
    return Ok(());
  }
  let info = ctx.esi.corporation().info(owner_id).await?;
  let alliance_id = info.alliance_id;
  let faction_id = info.faction_id;
  let ceo_id = info.ceo_id;
  let corporation = Corporation::from((owner_id, info));

  // NPC corporations report ceo_id = 1 (EVE's "nobody"); GET /characters/1/ answers 422. A real
  // CEO who has been biomassed answers 404. In either case persist the corporation without a CEO
  // so station/structure owner names still resolve, instead of aborting the whole sync.
  let ceo_info = match ctx.esi.character().public_info(ceo_id).await {
    Ok(ceo_info) => ceo_info,
    Err(Error::Http(error)) if is_unfetchable_character(&error) => {
      if let Some(id) = corporation.alliance_id() {
        ensure_alliance(ctx, id).await?;
      }
      org::upsert_corporation(ctx.db, &corporation).await?;
      return Ok(());
    }
    Err(error) => return Err(error),
  };
  let race_id = ceo_info.race_id;
  let bloodline_id = ceo_info.bloodline_id;
  let ceo = Character::from((ceo_id, ceo_info));

  let alliance = match alliance_id {
    Some(id) => Some(Alliance::from((id, ctx.esi.alliance().info(id).await?))),
    None => None,
  };
  let faction = match faction_id {
    Some(id) => Some(resolve_faction(ctx, id).await?),
    None => None,
  };
  // The CEO carries its own alliance/faction, which can differ from the corporation's (or exist when
  // the corp has none). upsert_with_org only persists the corp-side rows, so resolve the CEO's ends
  // too or the non-deferrable characters.alliance_id/faction_id FKs fail at commit.
  if let Some(id) = ceo.alliance_id() {
    ensure_alliance(ctx, id).await?;
  }
  if let Some(id) = ceo.faction_id() {
    ensure_faction(ctx, id).await?;
  }
  let race = resolve_race_model(ctx, i64::from(race_id)).await?;
  let bloodline = resolve_bloodline(ctx, i64::from(bloodline_id)).await?;

  character::upsert_with_org(
    ctx.db,
    &ceo,
    &bloodline,
    &race,
    &corporation,
    alliance.as_ref(),
    faction.as_ref(),
  )
  .await?;
  Ok(())
}

async fn ensure_alliance(ctx: &JobCtx<'_>, alliance_id: i64) -> Result<(), Error> {
  if org::get_alliance(ctx.db, alliance_id).await?.is_some() {
    return Ok(());
  }
  let alliance = Alliance::from((alliance_id, ctx.esi.alliance().info(alliance_id).await?));
  org::upsert_alliance(ctx.db, &alliance).await?;
  Ok(())
}

async fn ensure_faction(ctx: &JobCtx<'_>, faction_id: i64) -> Result<(), Error> {
  if sde::get_faction(ctx.db, faction_id).await?.is_some() {
    return Ok(());
  }
  let faction = resolve_faction(ctx, faction_id).await?;
  sde::upsert_faction(ctx.db, &faction).await?;
  Ok(())
}

async fn resolve_race(ctx: &JobCtx<'_>, race_id: i64) -> Result<(), Error> {
  if sde::get_race(ctx.db, race_id).await?.is_some() {
    return Ok(());
  }
  let race = resolve_race_model(ctx, race_id).await?;
  sde::upsert_race(ctx.db, &race).await?;
  Ok(())
}

async fn resolve_race_model(ctx: &JobCtx<'_>, race_id: i64) -> Result<Race, Error> {
  if let Some(race) = sde::get_race(ctx.db, race_id).await? {
    return Ok(race);
  }
  let lookup_id =
    i32::try_from(race_id).map_err(|_| Error::Internal(format!("race id {race_id} out of range for ESI lookup")))?;
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

async fn resolve_faction(ctx: &JobCtx<'_>, faction_id: i64) -> Result<Faction, Error> {
  if let Some(faction) = sde::get_faction(ctx.db, faction_id).await? {
    return Ok(faction);
  }
  ctx
    .esi
    .faction()
    .list()
    .await?
    .into_iter()
    .find(|faction| faction.faction_id == faction_id)
    .map(Faction::from)
    .ok_or_else(|| Error::Internal(format!("faction {faction_id} not in /universe/factions")))
}

async fn resolve_bloodline(ctx: &JobCtx<'_>, bloodline_id: i64) -> Result<Bloodline, Error> {
  if let Some(bloodline) = sde::get_bloodline(ctx.db, bloodline_id).await? {
    return Ok(bloodline);
  }
  let lookup_id = i32::try_from(bloodline_id)
    .map_err(|_| Error::Internal(format!("bloodline id {bloodline_id} out of range for ESI lookup")))?;
  ctx
    .esi
    .bloodlines()
    .list()
    .await?
    .into_iter()
    .find(|bloodline| bloodline.bloodline_id == lookup_id)
    .map(Bloodline::from)
    .ok_or_else(|| Error::Internal(format!("bloodline {bloodline_id} not in /universe/bloodlines")))
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, eve_image, http},
    store::{
      self, images,
      model::{Alliance, Constellation, Gender, Region, SolarSystem},
      repo::sde,
    },
    sync::job::{JobKey, JobKind},
  };

  const STRUCTURE_ID: i64 = 1_021_000_000_000;
  const STATION_ID: i64 = 60_003_760;
  const SYSTEM_ID: i64 = 30_000_142;
  const CONSTELLATION_ID: i64 = 20_000_020;
  const REGION_ID: i64 = 10_000_002;
  const OWNER_CORP_ID: i64 = 1_000_035;

  struct Harness {
    db: store::Database,
    esi: Arc<esi::Client>,
    image: eve_image::Client,
    image_store: images::Store,
    _images_dir: tempfile::TempDir,
  }

  impl Harness {
    async fn new(base_url: &str) -> Self {
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), base_url.to_owned()));
      let image = eve_image::Client::with_base_url(http.clone(), base_url.to_owned());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      Self {
        db,
        esi,
        image,
        image_store,
        _images_dir: images_dir,
      }
    }
  }

  fn ctx_with<'a>(harness: &'a Harness, grant: Option<&'a Grant>, subject: Subject) -> JobCtx<'a> {
    JobCtx {
      db: &harness.db,
      esi: &harness.esi,
      image: &harness.image,
      image_store: &harness.image_store,
      key: JobKey::new(JobKind::AssetSync, subject),
      grant,
    }
  }

  async fn seed_character(db: &store::Database, id: i64) {
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, OWNER_CORP_ID, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(OWNER_CORP_ID, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, OWNER_CORP_ID, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, OWNER_CORP_ID, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
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

  async fn mount_structure_ok(server: &MockServer, expected: u64) {
    Mock::given(method("GET"))
      .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "name": "A Player Structure",
        "owner_id": OWNER_CORP_ID,
        "solar_system_id": SYSTEM_ID,
      })))
      .expect(expected)
      .mount(server)
      .await;
  }

  fn scoped_grant() -> Grant {
    Grant::new_test_with_scopes("structure-token", 100, vec![scopes::UNIVERSE_STRUCTURES.to_owned()])
  }

  async fn mount_npc_station(server: &MockServer) {
    Mock::given(method("GET"))
      .and(path(format!("/universe/stations/{STATION_ID}/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "max_dockable_ship_volume": 50_000_000.0,
        "name": "Jita IV - Moon 4 - Caldari Navy Assembly Plant",
        "office_rental_cost": 10_000.0,
        "position": { "x": 1.0, "y": 2.0, "z": 3.0 },
        "reprocessing_efficiency": 0.5,
        "reprocessing_stations_take": 0.05,
        "services": [],
        "station_id": STATION_ID,
        "system_id": SYSTEM_ID,
        "type_id": 1529,
      })))
      .mount(server)
      .await;
    mount_item_type(server).await;
    Mock::given(method("GET"))
      .and(path(format!("/universe/systems/{SYSTEM_ID}/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "constellation_id": CONSTELLATION_ID, "name": "Jita", "position": { "x": 1.0, "y": 2.0, "z": 3.0 },
        "security_status": 0.946, "system_id": SYSTEM_ID,
      })))
      .mount(server)
      .await;
    Mock::given(method("GET"))
      .and(path(format!("/universe/constellations/{CONSTELLATION_ID}/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "constellation_id": CONSTELLATION_ID, "name": "Kimotoro", "position": { "x": 1.0, "y": 2.0, "z": 3.0 },
        "region_id": REGION_ID, "systems": [SYSTEM_ID],
      })))
      .mount(server)
      .await;
    Mock::given(method("GET"))
      .and(path(format!("/universe/regions/{REGION_ID}/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "constellations": [CONSTELLATION_ID], "description": "The Forge.", "name": "The Forge", "region_id": REGION_ID,
      })))
      .mount(server)
      .await;
  }

  async fn mount_item_type(server: &MockServer) {
    Mock::given(method("GET"))
      .and(path("/universe/types/1529/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "description": "A station.", "group_id": 15, "name": "Caldari Station", "published": true, "type_id": 1529,
      })))
      .mount(server)
      .await;
    Mock::given(method("GET"))
      .and(path("/universe/groups/15/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "category_id": 3, "group_id": 15, "name": "Station", "published": true, "types": [1529],
      })))
      .mount(server)
      .await;
    Mock::given(method("GET"))
      .and(path("/universe/categories/3/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "category_id": 3, "groups": [15], "name": "Station", "published": true,
      })))
      .mount(server)
      .await;
  }

  mod resolve_asset_references {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_resolves_and_caches_a_structure() {
      let server = MockServer::start().await;
      mount_structure_ok(&server, 1).await;
      let harness = Harness::new(&server.uri()).await;
      seed_character(&harness.db, 100).await;
      seed_geography(&harness.db).await;
      let grant = scoped_grant();
      let ctx = ctx_with(&harness, Some(&grant), Subject::Character(100));

      resolve_asset_references(&ctx, &[], &[], &[STRUCTURE_ID]).await.unwrap();

      let structure = sde::get_structure(&harness.db, STRUCTURE_ID)
        .await
        .unwrap()
        .expect("the structure is cached");
      assert_eq!(structure.name(), "A Player Structure");
    }

    #[tokio::test]
    async fn it_marks_an_inaccessible_structure_for_the_owning_character() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let grant = scoped_grant();
      let ctx = ctx_with(&harness, Some(&grant), Subject::Character(100));

      resolve_asset_references(&ctx, &[], &[], &[STRUCTURE_ID]).await.unwrap();

      assert!(
        sde::is_structure_inaccessible(&harness.db, 100, OwnerType::Character, STRUCTURE_ID)
          .await
          .unwrap(),
        "a 403 structure is marked inaccessible for the owner"
      );
      assert!(
        sde::get_structure(&harness.db, STRUCTURE_ID).await.unwrap().is_none(),
        "an inaccessible structure leaves no cache row"
      );
    }

    #[tokio::test]
    async fn it_marks_an_inaccessible_structure_per_subject_for_a_corporation() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let grant = scoped_grant();
      let ctx = ctx_with(&harness, Some(&grant), Subject::Corporation(OWNER_CORP_ID));

      resolve_asset_references(&ctx, &[], &[], &[STRUCTURE_ID]).await.unwrap();

      assert!(
        sde::is_structure_inaccessible(&harness.db, OWNER_CORP_ID, OwnerType::Corporation, STRUCTURE_ID)
          .await
          .unwrap(),
        "the mark is keyed to the corporation subject"
      );
      assert!(
        !sde::is_structure_inaccessible(&harness.db, OWNER_CORP_ID, OwnerType::Character, STRUCTURE_ID)
          .await
          .unwrap(),
        "the character owner of the same id is unaffected"
      );
    }

    #[tokio::test]
    async fn it_leaves_a_structure_unmarked_without_the_structures_scope() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let grant = Grant::new_test("no-scope-token", 100);
      let ctx = ctx_with(&harness, Some(&grant), Subject::Character(100));

      resolve_asset_references(&ctx, &[], &[], &[STRUCTURE_ID]).await.unwrap();

      assert!(sde::get_structure(&harness.db, STRUCTURE_ID).await.unwrap().is_none());
      assert!(
        !sde::is_structure_inaccessible(&harness.db, 100, OwnerType::Character, STRUCTURE_ID)
          .await
          .unwrap(),
        "without the scope the id stays unmarked, so a later re-auth can still resolve it"
      );
    }

    #[tokio::test]
    async fn it_resolves_a_referenced_npc_station() {
      let server = MockServer::start().await;
      mount_npc_station(&server).await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_with(&harness, None, Subject::Character(100));

      resolve_asset_references(&ctx, &[], &[STATION_ID], &[]).await.unwrap();

      let station = sde::get_station(&harness.db, STATION_ID)
        .await
        .unwrap()
        .expect("the NPC station is cached");
      assert_eq!(station.name(), "Jita IV - Moon 4 - Caldari Navy Assembly Plant");
    }

    #[tokio::test]
    async fn it_tolerates_an_inaccessible_station_without_erroring() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/stations/{STATION_ID}/")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_with(&harness, None, Subject::Character(100));

      resolve_asset_references(&ctx, &[], &[STATION_ID], &[]).await.unwrap();

      assert!(
        sde::get_station(&harness.db, STATION_ID).await.unwrap().is_none(),
        "a 403/404 station is left unresolved, not fatal"
      );
    }

    #[tokio::test]
    async fn it_resolves_a_referenced_item_type() {
      let server = MockServer::start().await;
      mount_item_type(&server).await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_with(&harness, None, Subject::Character(100));

      resolve_asset_references(&ctx, &[1529], &[], &[]).await.unwrap();

      assert!(
        sde::get_item_type(&harness.db, 1529).await.unwrap().is_some(),
        "the referenced item type is resolved before persist"
      );
    }
  }

  async fn mount_owner_corporation_stack(server: &MockServer) {
    Mock::given(method("GET"))
      .and(path(format!("/corporations/{OWNER_CORP_ID}/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "ceo_id": 3_004_029, "creator_id": 3_004_029, "member_count": 10_000, "name": "Caldari Navy",
        "tax_rate": 0.0, "ticker": "CN",
      })))
      .mount(server)
      .await;
    Mock::given(method("GET"))
      .and(path("/characters/3004029/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "birthday": "2003-01-01T00:00:00Z", "bloodline_id": 5, "corporation_id": OWNER_CORP_ID,
        "gender": "male", "name": "Caldari Navy CEO", "race_id": 1,
      })))
      .mount(server)
      .await;
    Mock::given(method("GET"))
      .and(path("/universe/races/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
        { "alliance_id": 500_001, "description": "The Caldari.", "name": "Caldari", "race_id": 1 },
      ])))
      .mount(server)
      .await;
    Mock::given(method("GET"))
      .and(path("/universe/bloodlines/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
        { "bloodline_id": 5, "charisma": 6, "corporation_id": OWNER_CORP_ID, "description": "The Civire.",
          "intelligence": 7, "memory": 5, "name": "Civire", "perception": 5, "race_id": 1,
          "ship_type_id": 601, "willpower": 5 },
      ])))
      .mount(server)
      .await;
  }

  fn ctx_no_grant<'a>(harness: &'a Harness) -> JobCtx<'a> {
    ctx_with(harness, None, Subject::Character(0))
  }

  mod resolve_stockpile_location {
    use super::*;

    #[tokio::test]
    async fn it_resolves_a_station_picked_for_a_stockpile() {
      let server = MockServer::start().await;
      mount_npc_station(&server).await;
      let harness = Harness::new(&server.uri()).await;
      let grant = Grant::new_test("token", 100);

      resolve_stockpile_location(
        &harness.db,
        &harness.esi,
        &harness.image,
        &harness.image_store,
        &grant,
        STATION_ID,
      )
      .await
      .unwrap();

      assert!(
        sde::get_station(&harness.db, STATION_ID).await.unwrap().is_some(),
        "an NPC station chosen for a stockpile is resolved into the universe cache"
      );
    }

    #[tokio::test]
    async fn it_short_circuits_a_solar_system_without_touching_esi() {
      let server = MockServer::start().await;
      let harness = Harness::new(&server.uri()).await;
      let grant = Grant::new_test("token", 100);

      resolve_stockpile_location(
        &harness.db,
        &harness.esi,
        &harness.image,
        &harness.image_store,
        &grant,
        SYSTEM_ID,
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_resolves_a_region_picked_for_a_stockpile() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/regions/{REGION_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "constellations": [CONSTELLATION_ID], "description": "The Forge.", "name": "The Forge", "region_id": REGION_ID,
        })))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let grant = Grant::new_test("token", 100);

      resolve_stockpile_location(
        &harness.db,
        &harness.esi,
        &harness.image,
        &harness.image_store,
        &grant,
        REGION_ID,
      )
      .await
      .unwrap();

      assert!(
        sde::get_region(&harness.db, REGION_ID).await.unwrap().is_some(),
        "a region chosen for a stockpile is resolved into the universe cache"
      );
    }

    #[tokio::test]
    async fn it_resolves_a_constellation_and_its_region_for_a_stockpile() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/constellations/{CONSTELLATION_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "constellation_id": CONSTELLATION_ID, "name": "Kimotoro", "position": { "x": 1.0, "y": 2.0, "z": 3.0 },
          "region_id": REGION_ID, "systems": [SYSTEM_ID],
        })))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/regions/{REGION_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "constellations": [CONSTELLATION_ID], "description": "The Forge.", "name": "The Forge", "region_id": REGION_ID,
        })))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let grant = Grant::new_test("token", 100);

      resolve_stockpile_location(
        &harness.db,
        &harness.esi,
        &harness.image,
        &harness.image_store,
        &grant,
        CONSTELLATION_ID,
      )
      .await
      .unwrap();

      assert!(
        sde::get_constellation(&harness.db, CONSTELLATION_ID)
          .await
          .unwrap()
          .is_some(),
        "the picked constellation is cached"
      );
      assert!(
        sde::get_region(&harness.db, REGION_ID).await.unwrap().is_some(),
        "its parent region is resolved too, satisfying the constellation's region FK"
      );
    }
  }

  mod resolve_owner_corporation {
    use super::*;

    #[tokio::test]
    async fn it_short_circuits_when_the_corporation_is_already_cached() {
      let server = MockServer::start().await;
      let harness = Harness::new(&server.uri()).await;
      seed_character(&harness.db, 100).await;
      let ctx = ctx_no_grant(&harness);

      resolve_owner_corporation(&ctx, OWNER_CORP_ID).await.unwrap();

      assert!(
        org::get_corporation(&harness.db, OWNER_CORP_ID)
          .await
          .unwrap()
          .is_some()
      );
    }

    #[tokio::test]
    async fn it_resolves_the_full_owner_org_stack_when_uncached() {
      let server = MockServer::start().await;
      mount_owner_corporation_stack(&server).await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      resolve_owner_corporation(&ctx, OWNER_CORP_ID).await.unwrap();

      let corporation = org::get_corporation(&harness.db, OWNER_CORP_ID)
        .await
        .unwrap()
        .expect("the owner corporation is cached");
      assert_eq!(corporation.name(), "Caldari Navy");
      assert!(character::get(&harness.db, 3_004_029).await.unwrap().is_some());
      assert!(sde::get_race(&harness.db, 1).await.unwrap().is_some());
      assert!(sde::get_bloodline(&harness.db, 5).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_resolves_the_alliance_and_faction_when_the_corporation_carries_them() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{OWNER_CORP_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "alliance_id": 99_000_001, "ceo_id": 3_004_029, "creator_id": 3_004_029, "faction_id": 500_001,
          "member_count": 10_000, "name": "Caldari Navy", "tax_rate": 0.0, "ticker": "CN",
        })))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/characters/3004029/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "birthday": "2003-01-01T00:00:00Z", "bloodline_id": 5, "corporation_id": OWNER_CORP_ID,
          "gender": "male", "name": "Caldari Navy CEO", "race_id": 1,
        })))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/alliances/99000001/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "creator_corporation_id": OWNER_CORP_ID, "creator_id": 3_004_029, "date_founded": "2003-01-01T00:00:00Z",
          "executor_corporation_id": OWNER_CORP_ID, "name": "Test Alliance", "ticker": "TST",
        })))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/factions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          { "corporation_id": OWNER_CORP_ID, "description": "The State.", "faction_id": 500_001,
            "is_unique": true, "name": "Caldari State", "size_factor": 5.0, "station_count": 100,
            "station_system_count": 50 },
        ])))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/races/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          { "alliance_id": 500_001, "description": "The Caldari.", "name": "Caldari", "race_id": 1 },
        ])))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/bloodlines/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          { "bloodline_id": 5, "charisma": 6, "corporation_id": OWNER_CORP_ID, "description": "The Civire.",
            "intelligence": 7, "memory": 5, "name": "Civire", "perception": 5, "race_id": 1,
            "ship_type_id": 601, "willpower": 5 },
        ])))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      resolve_owner_corporation(&ctx, OWNER_CORP_ID).await.unwrap();

      assert!(
        org::get_corporation(&harness.db, OWNER_CORP_ID)
          .await
          .unwrap()
          .is_some()
      );
      assert!(org::get_alliance(&harness.db, 99_000_001).await.unwrap().is_some());
      assert!(sde::get_faction(&harness.db, 500_001).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_persists_an_npc_corp_without_a_ceo_when_the_ceo_is_unprocessable() {
      let server = MockServer::start().await;
      // NPC corporations report ceo_id = 1, and GET /characters/1/ answers 422.
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{OWNER_CORP_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "ceo_id": 1, "creator_id": 1, "member_count": 10_000, "name": "Caldari Navy",
          "tax_rate": 0.0, "ticker": "CN",
        })))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/characters/1/"))
        .respond_with(ResponseTemplate::new(422))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      resolve_owner_corporation(&ctx, OWNER_CORP_ID)
        .await
        .expect("a 422 CEO is tolerated, not fatal");

      let corporation = org::get_corporation(&harness.db, OWNER_CORP_ID)
        .await
        .unwrap()
        .expect("the NPC owner corporation is still persisted so station names resolve");
      assert_eq!(corporation.name(), "Caldari Navy");
      assert!(
        character::get(&harness.db, 1).await.unwrap().is_none(),
        "no CEO character row is created for an unfetchable NPC-corp CEO"
      );
    }

    #[tokio::test]
    async fn it_persists_a_corp_without_a_ceo_when_the_ceo_is_not_found() {
      let server = MockServer::start().await;
      // A biomassed player CEO answers 404; the corp must still persist.
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{OWNER_CORP_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "alliance_id": 99_000_001, "ceo_id": 3_004_029, "creator_id": 3_004_029, "member_count": 10_000,
          "name": "Caldari Navy", "tax_rate": 0.0, "ticker": "CN",
        })))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/characters/3004029/"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/alliances/99000001/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "creator_corporation_id": OWNER_CORP_ID, "creator_id": 3_004_029, "date_founded": "2003-01-01T00:00:00Z",
          "executor_corporation_id": OWNER_CORP_ID, "name": "Test Alliance", "ticker": "TST",
        })))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      resolve_owner_corporation(&ctx, OWNER_CORP_ID)
        .await
        .expect("a 404 CEO is tolerated, not fatal");

      assert!(
        org::get_corporation(&harness.db, OWNER_CORP_ID)
          .await
          .unwrap()
          .is_some()
      );
      assert!(
        org::get_alliance(&harness.db, 99_000_001).await.unwrap().is_some(),
        "the corp's alliance row is ensured first so the deferred alliance_id FK holds at commit"
      );
      assert!(
        character::get(&harness.db, 3_004_029).await.unwrap().is_none(),
        "no CEO character row is created when the CEO 404s"
      );
    }

    #[tokio::test]
    async fn it_propagates_a_non_miss_error_from_the_ceo_fetch() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{OWNER_CORP_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "ceo_id": 3_004_029, "creator_id": 3_004_029, "member_count": 10_000, "name": "Caldari Navy",
          "tax_rate": 0.0, "ticker": "CN",
        })))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/characters/3004029/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      let result = resolve_owner_corporation(&ctx, OWNER_CORP_ID).await;

      assert!(
        result.is_err(),
        "a 500 CEO fetch still aborts; only 404/422 are tolerated"
      );
      assert!(
        org::get_corporation(&harness.db, OWNER_CORP_ID)
          .await
          .unwrap()
          .is_none(),
        "nothing is persisted when the CEO fetch fails with an untolerated status"
      );
    }

    #[tokio::test]
    async fn it_resolves_the_ceos_own_alliance_and_faction_when_the_corp_carries_neither() {
      let server = MockServer::start().await;
      // The corporation itself belongs to no alliance and no faction...
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{OWNER_CORP_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "ceo_id": 3_004_029, "creator_id": 3_004_029, "member_count": 10_000, "name": "Caldari Navy",
          "tax_rate": 0.0, "ticker": "CN",
        })))
        .mount(&server)
        .await;
      // ...but its CEO personally holds an alliance and is enlisted in factional warfare.
      Mock::given(method("GET"))
        .and(path("/characters/3004029/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "alliance_id": 99_000_002, "birthday": "2003-01-01T00:00:00Z", "bloodline_id": 5,
          "corporation_id": OWNER_CORP_ID, "faction_id": 500_001, "gender": "male",
          "name": "Caldari Navy CEO", "race_id": 1,
        })))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/alliances/99000002/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "creator_corporation_id": OWNER_CORP_ID, "creator_id": 3_004_029, "date_founded": "2003-01-01T00:00:00Z",
          "executor_corporation_id": OWNER_CORP_ID, "name": "CEO Personal Alliance", "ticker": "CPA",
        })))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/factions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          { "corporation_id": OWNER_CORP_ID, "description": "The State.", "faction_id": 500_001,
            "is_unique": true, "name": "Caldari State", "size_factor": 5.0, "station_count": 100,
            "station_system_count": 50 },
        ])))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/races/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          { "alliance_id": 500_001, "description": "The Caldari.", "name": "Caldari", "race_id": 1 },
        ])))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/bloodlines/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          { "bloodline_id": 5, "charisma": 6, "corporation_id": OWNER_CORP_ID, "description": "The Civire.",
            "intelligence": 7, "memory": 5, "name": "Civire", "perception": 5, "race_id": 1,
            "ship_type_id": 601, "willpower": 5 },
        ])))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      resolve_owner_corporation(&ctx, OWNER_CORP_ID)
        .await
        .expect("the CEO's own alliance and faction are resolved, so no FK violation at commit");

      assert!(
        org::get_alliance(&harness.db, 99_000_002).await.unwrap().is_some(),
        "the CEO's personal alliance is persisted even though the corp has none"
      );
      assert!(
        sde::get_faction(&harness.db, 500_001).await.unwrap().is_some(),
        "the CEO's faction is persisted even though the corp has none"
      );
      assert!(character::get(&harness.db, 3_004_029).await.unwrap().is_some());
    }
  }

  mod resolve_solar_system {
    use super::*;

    #[tokio::test]
    async fn it_short_circuits_when_the_system_is_already_cached() {
      let server = MockServer::start().await;
      let harness = Harness::new(&server.uri()).await;
      seed_geography(&harness.db).await;
      let ctx = ctx_no_grant(&harness);

      resolve_solar_system(&ctx, SYSTEM_ID).await.unwrap();

      assert!(sde::get_solar_system(&harness.db, SYSTEM_ID).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_resolves_the_system_with_its_constellation_and_region_when_uncached() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/systems/{SYSTEM_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "constellation_id": CONSTELLATION_ID, "name": "Jita", "position": { "x": 1.0, "y": 2.0, "z": 3.0 },
          "security_status": 0.946, "system_id": SYSTEM_ID,
        })))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/constellations/{CONSTELLATION_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "constellation_id": CONSTELLATION_ID, "name": "Kimotoro", "position": { "x": 1.0, "y": 2.0, "z": 3.0 },
          "region_id": REGION_ID, "systems": [SYSTEM_ID],
        })))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/regions/{REGION_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "constellations": [CONSTELLATION_ID], "description": "The Forge.", "name": "The Forge", "region_id": REGION_ID,
        })))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      resolve_solar_system(&ctx, SYSTEM_ID).await.unwrap();

      let system = sde::get_solar_system(&harness.db, SYSTEM_ID)
        .await
        .unwrap()
        .expect("the system is cached");
      assert_eq!(system.name(), "Jita");
      assert!(
        sde::get_constellation(&harness.db, CONSTELLATION_ID)
          .await
          .unwrap()
          .is_some()
      );
      assert!(sde::get_region(&harness.db, REGION_ID).await.unwrap().is_some());
    }
  }

  mod resolve_race {
    use super::*;

    #[tokio::test]
    async fn it_short_circuits_when_the_race_is_already_cached() {
      let server = MockServer::start().await;
      let harness = Harness::new(&server.uri()).await;
      sde::upsert_race(&harness.db, &Race::new(1, 500_001, "The Caldari.", "Caldari"))
        .await
        .unwrap();
      let ctx = ctx_no_grant(&harness);

      resolve_race(&ctx, 1).await.unwrap();

      assert!(sde::get_race(&harness.db, 1).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_fetches_and_persists_a_race_from_the_esi_list() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/universe/races/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          { "alliance_id": 500_001, "description": "The Caldari.", "name": "Caldari", "race_id": 1 },
        ])))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      resolve_race(&ctx, 1).await.unwrap();

      let race = sde::get_race(&harness.db, 1)
        .await
        .unwrap()
        .expect("the race is cached");
      assert_eq!(race.name(), "Caldari");
    }
  }

  mod resolve_race_model {
    use super::*;

    #[tokio::test]
    async fn it_errors_when_the_race_is_absent_from_the_esi_list() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/universe/races/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      let result = resolve_race_model(&ctx, 7).await;

      assert!(matches!(result, Err(Error::Internal(_))));
    }

    #[tokio::test]
    async fn it_errors_when_the_race_id_overflows_an_i32() {
      let server = MockServer::start().await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      let result = resolve_race_model(&ctx, i64::from(i32::MAX) + 1).await;

      assert!(matches!(result, Err(Error::Internal(_))));
    }
  }

  mod resolve_faction {
    use super::*;

    #[tokio::test]
    async fn it_short_circuits_when_the_faction_is_already_cached() {
      let server = MockServer::start().await;
      let harness = Harness::new(&server.uri()).await;
      sde::upsert_faction(&harness.db, &Faction::new(500_001, "Caldari State", true, 5.0, 100, 50))
        .await
        .unwrap();
      let ctx = ctx_no_grant(&harness);

      let faction = resolve_faction(&ctx, 500_001).await.unwrap();

      assert_eq!(faction.name(), "Caldari State");
    }

    #[tokio::test]
    async fn it_fetches_a_faction_from_the_esi_list() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/universe/factions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          { "corporation_id": OWNER_CORP_ID, "description": "The State.", "faction_id": 500_001,
            "is_unique": true, "name": "Caldari State", "size_factor": 5.0, "station_count": 100,
            "station_system_count": 50 },
        ])))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      let faction = resolve_faction(&ctx, 500_001).await.unwrap();

      assert_eq!(faction.name(), "Caldari State");
    }

    #[tokio::test]
    async fn it_errors_when_the_faction_is_absent_from_the_esi_list() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/universe/factions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      let result = resolve_faction(&ctx, 500_001).await;

      assert!(matches!(result, Err(Error::Internal(_))));
    }
  }

  mod resolve_bloodline {
    use super::*;

    #[tokio::test]
    async fn it_short_circuits_when_the_bloodline_is_already_cached() {
      let server = MockServer::start().await;
      let harness = Harness::new(&server.uri()).await;
      seed_character(&harness.db, 100).await;
      sde::upsert_bloodline(
        &harness.db,
        &Bloodline::new(5, OWNER_CORP_ID, 2, 3, "The Civire.", 4, 5, "Civire", 4, 4),
      )
      .await
      .unwrap();
      let ctx = ctx_no_grant(&harness);

      let bloodline = resolve_bloodline(&ctx, 5).await.unwrap();

      assert_eq!(bloodline.name(), "Civire");
    }

    #[tokio::test]
    async fn it_fetches_a_bloodline_from_the_esi_list() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/universe/bloodlines/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          { "bloodline_id": 5, "charisma": 6, "corporation_id": OWNER_CORP_ID, "description": "The Civire.",
            "intelligence": 7, "memory": 5, "name": "Civire", "perception": 5, "race_id": 1,
            "ship_type_id": 601, "willpower": 5 },
        ])))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      let bloodline = resolve_bloodline(&ctx, 5).await.unwrap();

      assert_eq!(bloodline.name(), "Civire");
    }

    #[tokio::test]
    async fn it_errors_when_the_bloodline_id_overflows_an_i32() {
      let server = MockServer::start().await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      let result = resolve_bloodline(&ctx, i64::from(i32::MAX) + 1).await;

      assert!(matches!(result, Err(Error::Internal(_))));
    }

    #[tokio::test]
    async fn it_errors_when_the_bloodline_is_absent_from_the_esi_list() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/universe/bloodlines/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;
      let harness = Harness::new(&server.uri()).await;
      let ctx = ctx_no_grant(&harness);

      let result = resolve_bloodline(&ctx, 5).await;

      assert!(matches!(result, Err(Error::Internal(_))));
    }
  }
}
