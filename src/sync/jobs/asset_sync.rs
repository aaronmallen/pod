use std::collections::{HashMap, HashSet};

use crate::{
  clients::{
    Error,
    esi::models::{character::Asset as EsiCharacterAsset, corporation::CorporationAsset as EsiCorporationAsset},
  },
  store::{
    model::{CharacterAsset, CorporationAsset},
    repo::{assets, character},
  },
  sync::{job::JobCtx, outcome::Outcome, structure_resolution, subject::Subject},
};

const LOCATION_TYPE_ITEM: &str = "item";
const LOCATION_TYPE_STATION: &str = "station";
const LOCATION_TYPE_STRUCTURE: &str = "structure";

struct BoardedShip {
  item_id: i64,
  name: String,
  type_id: i64,
}

struct AssetNode {
  is_active_ship: bool,
  is_blueprint_copy: Option<bool>,
  is_singleton: bool,
  item_id: i64,
  location_flag: String,
  location_id: i64,
  location_type: String,
  quantity: i64,
  ship_name: Option<String>,
  type_id: i64,
}

impl AssetNode {
  fn active_ship(ship: &BoardedShip) -> Self {
    Self {
      is_active_ship: true,
      is_blueprint_copy: None,
      is_singleton: true,
      item_id: ship.item_id,
      location_flag: "ShipHangar".to_owned(),
      location_id: 0,
      location_type: "solar_system".to_owned(),
      quantity: 1,
      ship_name: Some(ship.name.clone()),
      type_id: ship.type_id,
    }
  }

  fn mark_active_ship(&mut self, ship: &BoardedShip) {
    self.is_active_ship = true;
    self.ship_name = Some(ship.name.clone());
  }

  fn parent(&self, present: &HashSet<i64>) -> Option<i64> {
    present.contains(&self.location_id).then_some(self.location_id)
  }

  fn to_character_asset(&self, character_id: i64, resolved: &Resolved) -> CharacterAsset {
    CharacterAsset {
      character_id,
      container_id: resolved.container_id,
      depth: resolved.depth,
      is_active_ship: self.is_active_ship,
      is_blueprint_copy: self.is_blueprint_copy,
      is_container: resolved.is_container,
      is_singleton: self.is_singleton,
      item_id: self.item_id,
      location_flag: self.location_flag.clone(),
      location_id: self.location_id,
      location_type: self.location_type.clone(),
      quantity: self.quantity,
      ship_name: self.ship_name.clone(),
      type_id: self.type_id,
    }
  }

  fn to_corporation_asset(&self, corporation_id: i64, resolved: &Resolved) -> CorporationAsset {
    CorporationAsset {
      container_id: resolved.container_id,
      corporation_id,
      depth: resolved.depth,
      is_blueprint_copy: self.is_blueprint_copy,
      is_container: resolved.is_container,
      is_singleton: self.is_singleton,
      item_id: self.item_id,
      location_flag: self.location_flag.clone(),
      location_id: self.location_id,
      location_type: self.location_type.clone(),
      quantity: self.quantity,
      type_id: self.type_id,
    }
  }
}

impl From<&EsiCharacterAsset> for AssetNode {
  fn from(asset: &EsiCharacterAsset) -> Self {
    Self {
      is_active_ship: false,
      is_blueprint_copy: asset.is_blueprint_copy,
      is_singleton: asset.is_singleton,
      item_id: asset.item_id,
      location_flag: asset.location_flag.clone(),
      location_id: asset.location_id,
      location_type: asset.location_type.clone(),
      quantity: i64::from(asset.quantity),
      ship_name: None,
      type_id: i64::from(asset.type_id),
    }
  }
}

impl From<&EsiCorporationAsset> for AssetNode {
  fn from(asset: &EsiCorporationAsset) -> Self {
    Self {
      is_active_ship: false,
      is_blueprint_copy: asset.is_blueprint_copy,
      is_singleton: asset.is_singleton,
      item_id: asset.item_id,
      location_flag: asset.location_flag.clone(),
      location_id: asset.location_id,
      location_type: asset.location_type.clone(),
      quantity: i64::from(asset.quantity),
      ship_name: None,
      type_id: i64::from(asset.type_id),
    }
  }
}

struct Resolved {
  container_id: Option<i64>,
  depth: i64,
  is_container: bool,
}

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  match ctx.key.subject {
    Subject::Character(character_id) => run_character(ctx, character_id).await,
    Subject::Corporation(corporation_id) => run_corporation(ctx, corporation_id).await,
  }
}

async fn run_character(ctx: &JobCtx<'_>, character_id: i64) -> Result<Outcome, Error> {
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character asset job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authenticated = ctx.esi.character_authenticated(grant);

  let esi_assets = authenticated.assets().await?;
  let mut nodes: Vec<AssetNode> = esi_assets.iter().map(AssetNode::from).collect();

  let ship = authenticated.ship().await?;
  let boarded_ship = BoardedShip {
    item_id: ship.ship_item_id,
    type_id: i64::from(ship.ship_type_id),
    name: ship.ship_name,
  };
  match nodes.iter_mut().find(|node| node.item_id == boarded_ship.item_id) {
    Some(existing) => existing.mark_active_ship(&boarded_ship),
    None => nodes.push(AssetNode::active_ship(&boarded_ship)),
  }

  reclassify_structure_roots(&mut nodes);
  resolve_references(ctx, &nodes).await?;

  let hierarchy = build_hierarchy(&nodes);
  let rows: Vec<CharacterAsset> = nodes
    .iter()
    .zip(&hierarchy)
    .filter_map(|(node, resolved)| {
      let resolved = resolved.as_ref()?;
      Some(node.to_character_asset(character_id, resolved))
    })
    .collect();

  assets::replace_for_character(ctx.db, character_id, &rows).await?;
  Ok(Outcome::from_rows(rows.len()))
}

async fn run_corporation(ctx: &JobCtx<'_>, corporation_id: i64) -> Result<Outcome, Error> {
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "corporation asset job for {corporation_id} requires a grant"
    )));
  };
  let esi_assets = ctx.esi.corporation_authenticated(grant).assets(corporation_id).await?;
  let mut nodes: Vec<AssetNode> = esi_assets.iter().map(AssetNode::from).collect();

  reclassify_structure_roots(&mut nodes);
  resolve_references(ctx, &nodes).await?;

  let hierarchy = build_hierarchy(&nodes);
  let rows: Vec<CorporationAsset> = nodes
    .iter()
    .zip(&hierarchy)
    .filter_map(|(node, resolved)| {
      let resolved = resolved.as_ref()?;
      Some(node.to_corporation_asset(corporation_id, resolved))
    })
    .collect();

  assets::replace_for_corporation(ctx.db, corporation_id, &rows).await?;
  Ok(Outcome::from_rows(rows.len()))
}

fn build_hierarchy(nodes: &[AssetNode]) -> Vec<Option<Resolved>> {
  let present: HashSet<i64> = nodes.iter().map(|node| node.item_id).collect();
  let by_id: HashMap<i64, &AssetNode> = nodes.iter().map(|node| (node.item_id, node)).collect();

  let mut depth_cache: HashMap<i64, Option<i64>> = HashMap::new();
  for node in nodes {
    resolve_depth(node.item_id, &by_id, &present, &mut depth_cache);
  }

  let mut container_ids: HashSet<i64> = HashSet::new();
  for node in nodes {
    if depth_cache.get(&node.item_id).copied().flatten().is_none() {
      continue;
    }
    if let Some(parent) = node.parent(&present) {
      container_ids.insert(parent);
    }
  }

  nodes
    .iter()
    .map(|node| {
      let depth = depth_cache.get(&node.item_id).copied().flatten()?;
      Some(Resolved {
        container_id: node.parent(&present),
        depth,
        is_container: container_ids.contains(&node.item_id),
      })
    })
    .collect()
}

fn dedup(ids: &mut Vec<i64>) {
  let unique: HashSet<i64> = ids.iter().copied().collect();
  *ids = unique.into_iter().collect();
  ids.sort_unstable();
}

fn reclassify_structure_roots(nodes: &mut [AssetNode]) {
  let present: HashSet<i64> = nodes.iter().map(|node| node.item_id).collect();
  for node in nodes.iter_mut() {
    if node.location_type == LOCATION_TYPE_ITEM && !present.contains(&node.location_id) {
      node.location_type = LOCATION_TYPE_STRUCTURE.to_owned();
    }
  }
}

fn resolve_depth(
  item_id: i64,
  by_id: &HashMap<i64, &AssetNode>,
  present: &HashSet<i64>,
  cache: &mut HashMap<i64, Option<i64>>,
) -> Option<i64> {
  if let Some(cached) = cache.get(&item_id) {
    return *cached;
  }
  cache.insert(item_id, None);

  let node = by_id.get(&item_id)?;
  let depth = match node.parent(present) {
    None => Some(0),
    Some(parent_id) => resolve_depth(parent_id, by_id, present, cache).map(|parent_depth| parent_depth + 1),
  };
  cache.insert(item_id, depth);
  depth
}

async fn resolve_references(ctx: &JobCtx<'_>, nodes: &[AssetNode]) -> Result<(), Error> {
  let mut type_ids: Vec<i64> = nodes.iter().map(|node| node.type_id).collect();
  let mut station_ids: Vec<i64> = Vec::new();
  let mut structure_ids: Vec<i64> = Vec::new();
  for node in nodes {
    match node.location_type.as_str() {
      LOCATION_TYPE_STATION => station_ids.push(node.location_id),
      LOCATION_TYPE_STRUCTURE => structure_ids.push(node.location_id),
      _ => {}
    }
  }
  dedup(&mut type_ids);
  dedup(&mut station_ids);
  dedup(&mut structure_ids);

  structure_resolution::resolve_asset_references(ctx, &type_ids, &station_ids, &structure_ids).await
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, esi::scopes, eve_image, eve_sso::Grant, http},
    store::{
      self, images,
      model::{ItemCategory, ItemGroup, ItemType},
      repo::sde,
    },
    sync::job::{JobKey, JobKind},
  };

  const STRUCTURE_ID: i64 = 1_021_000_000_000;

  async fn seed_item_types(db: &store::Database, type_ids: &[i64]) {
    sde::upsert_item_category(
      db,
      &ItemCategory {
        id: 1,
        icon_id: None,
        name: "Ship".to_owned(),
        published: true,
      },
    )
    .await
    .unwrap();
    sde::upsert_item_group(
      db,
      &ItemGroup {
        category_id: 1,
        icon_id: None,
        id: 1,
        name: "Frigate".to_owned(),
        published: true,
      },
    )
    .await
    .unwrap();
    for &type_id in type_ids {
      sde::upsert_item_type(
        db,
        &ItemType {
          capacity: None,
          description: Some("Test Item".to_owned()),
          dogma_attributes: "[]".to_owned(),
          group_id: 1,
          icon_id: None,
          id: type_id,
          market_group_id: None,
          name: "Test Item".to_owned(),
          packaged_volume: None,
          portion_size: None,
          published: true,
          radius: None,
          volume: None,
        },
      )
      .await
      .unwrap();
    }
  }

  fn node(item_id: i64, location_id: i64, location_type: &str) -> AssetNode {
    AssetNode {
      is_active_ship: false,
      is_blueprint_copy: None,
      is_singleton: false,
      item_id,
      location_flag: "Hangar".to_owned(),
      location_id,
      location_type: location_type.to_owned(),
      quantity: 1,
      ship_name: None,
      type_id: 587,
    }
  }

  mod build_hierarchy {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_a_nested_chain_with_container_id_depth_and_is_container() {
      let nodes = vec![
        node(100, 60_003_760, "station"),
        node(101, 100, "item"),
        node(102, 101, "item"),
      ];

      let resolved = build_hierarchy(&nodes);

      let root = resolved[0].as_ref().expect("root resolves");
      assert_eq!(root.container_id, None);
      assert_eq!(root.depth, 0);
      assert!(root.is_container, "100 holds 101");

      let mid = resolved[1].as_ref().expect("mid resolves");
      assert_eq!(mid.container_id, Some(100));
      assert_eq!(mid.depth, 1);
      assert!(mid.is_container, "101 holds 102");

      let leaf = resolved[2].as_ref().expect("leaf resolves");
      assert_eq!(leaf.container_id, Some(101));
      assert_eq!(leaf.depth, 2);
      assert!(!leaf.is_container, "102 holds nothing");
    }

    #[test]
    fn it_never_emits_container_id_zero_for_a_station_root() {
      let nodes = vec![node(100, 0, "solar_system")];

      let resolved = build_hierarchy(&nodes);

      assert_eq!(resolved[0].as_ref().unwrap().container_id, None);
    }

    #[test]
    fn it_lands_a_node_as_a_root_when_its_location_is_absent_from_the_snapshot() {
      let nodes = vec![node(100, 60_003_760, "station"), node(200, 999, "item")];

      let resolved = build_hierarchy(&nodes);

      assert!(resolved[0].is_some(), "the present root still resolves");

      let orphan = resolved[1]
        .as_ref()
        .expect("an absent location means root, not dropped");
      assert_eq!(orphan.container_id, None);
      assert_eq!(orphan.depth, 0);
    }

    #[test]
    fn it_lands_a_node_as_a_root_when_its_middle_parent_is_absent() {
      let nodes = vec![node(100, 60_003_760, "station"), node(102, 101, "item")];

      let resolved = build_hierarchy(&nodes);

      assert!(resolved[0].is_some());

      let grandchild = resolved[1].as_ref().expect("an absent parent means root, not dropped");
      assert_eq!(grandchild.container_id, None);
      assert_eq!(grandchild.depth, 0);
    }
  }

  async fn mount_assets(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
      .respond_with(
        ResponseTemplate::new(200)
          .insert_header("X-Pages", "1")
          .set_body_json(body),
      )
      .mount(server)
      .await;
  }

  async fn mount_ship(server: &MockServer, character_id: i64, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/characters/{character_id}/ship/")))
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

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: &'a Grant,
    subject: Subject,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::AssetSync, subject),
      grant: Some(grant),
    }
  }

  mod run_character {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{Constellation, Region, SolarSystem, Structure};

    const CITADEL_ID: i64 = 1_021_000_000_500;

    #[tokio::test]
    async fn it_syncs_with_the_persisted_row_count() {
      let server = MockServer::start().await;
      mount_assets(
        &server,
        "/characters/47/assets/",
        serde_json::json!([
          { "is_singleton": false, "item_id": 101, "location_flag": "Hangar", "location_id": 60003760,
            "location_type": "station", "quantity": 1, "type_id": 34 },
        ]),
      )
      .await;
      mount_ship(
        &server,
        47,
        serde_json::json!({ "ship_item_id": 9002, "ship_name": "Pod", "ship_type_id": 670 }),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 47).await;
      seed_item_types(&db, &[34, 670]).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 47);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Character(47));

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 2
        },
        "the held item plus the boarded ship are both persisted"
      );
    }

    async fn seed_citadel(db: &store::Database, id: i64, name: &str) {
      sde::upsert_region(
        db,
        &Region {
          description: None,
          id: 10_000_002,
          name: "The Forge".to_owned(),
        },
      )
      .await
      .unwrap();
      sde::upsert_constellation(
        db,
        &Constellation {
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
        &SolarSystem {
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
        &Structure {
          id,
          name: name.to_owned(),
          owner_id: 90_000_001,
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
    async fn it_persists_assets_with_hierarchy_and_the_synthetic_boarded_ship() {
      let server = MockServer::start().await;
      mount_assets(
        &server,
        "/characters/42/assets/",
        serde_json::json!([
          { "is_singleton": true, "item_id": 100, "location_flag": "Hangar", "location_id": 60003760,
            "location_type": "station", "quantity": 1, "type_id": 587 },
          { "is_singleton": false, "item_id": 101, "location_flag": "Cargo", "location_id": 100,
            "location_type": "item", "quantity": 5, "type_id": 34 },
        ]),
      )
      .await;
      mount_ship(
        &server,
        42,
        serde_json::json!({ "ship_item_id": 9001, "ship_name": "My Rifter", "ship_type_id": 587 }),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_types(&db, &[34, 587]).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Character(42));

      run(&ctx).await.unwrap();

      let rows = assets::for_character(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 3, "two assets plus the synthetic ship");

      let root = rows.iter().find(|row| row.item_id() == 100).unwrap();
      assert_eq!(root.container_id(), None);
      assert_eq!(root.depth(), 0);
      assert!(root.is_container(), "100 holds 101");

      let child = rows.iter().find(|row| row.item_id() == 101).unwrap();
      assert_eq!(child.container_id(), Some(100));
      assert_eq!(child.depth(), 1);

      let ship = rows.iter().find(|row| row.item_id() == 9001).unwrap();
      assert!(ship.is_active_ship());
      assert_eq!(ship.ship_name().as_deref(), Some("My Rifter"));
      assert_eq!(ship.container_id(), None, "the boarded ship is a space root");
    }

    #[tokio::test]
    async fn it_persists_cleanly_when_the_boarded_ship_is_also_in_the_assets_list() {
      let server = MockServer::start().await;
      mount_assets(
        &server,
        "/characters/45/assets/",
        serde_json::json!([
          { "is_singleton": true, "item_id": 9001, "location_flag": "Hangar", "location_id": 60003760,
            "location_type": "station", "quantity": 1, "type_id": 587 },
          { "is_singleton": false, "item_id": 9002, "location_flag": "Cargo", "location_id": 9001,
            "location_type": "item", "quantity": 5, "type_id": 34 },
        ]),
      )
      .await;
      mount_ship(
        &server,
        45,
        serde_json::json!({ "ship_item_id": 9001, "ship_name": "My Rifter", "ship_type_id": 587 }),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 45).await;
      seed_item_types(&db, &[34, 587]).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 45);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Character(45));

      run(&ctx).await.unwrap();

      let rows = assets::for_character(&db, 45).await.unwrap();
      let ship_rows: Vec<_> = rows.iter().filter(|row| row.item_id() == 9001).collect();
      assert_eq!(
        ship_rows.len(),
        1,
        "the boarded ship must be represented exactly once, never duplicated into a UNIQUE violation"
      );
      assert!(
        ship_rows[0].is_active_ship(),
        "the real assets row carrying the ship's item_id is flagged as the active ship"
      );
      assert_eq!(ship_rows[0].ship_name().as_deref(), Some("My Rifter"));
      assert!(
        rows.iter().any(|row| row.item_id() == 9002),
        "an item nested inside the boarded ship still lands once the ship is a single node"
      );
    }

    #[tokio::test]
    async fn it_short_retries_without_an_esi_call_when_the_character_row_is_absent() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/46/assets/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([])),
        )
        .expect(0)
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/characters/46/ship/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 46);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Character(46));

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
    }

    #[tokio::test]
    async fn it_lands_an_asset_whose_location_is_absent_as_a_structure_root() {
      let server = MockServer::start().await;
      mount_assets(
        &server,
        "/characters/43/assets/",
        serde_json::json!([
          { "is_singleton": false, "item_id": 101, "location_flag": "Hangar", "location_id": 100,
            "location_type": "item", "quantity": 1, "type_id": 34 },
        ]),
      )
      .await;
      mount_ship(
        &server,
        43,
        serde_json::json!({ "ship_item_id": 9002, "ship_name": "Pod", "ship_type_id": 670 }),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 43).await;
      seed_item_types(&db, &[34, 670]).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 43);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Character(43));

      run(&ctx).await.unwrap();

      let rows = assets::for_character(&db, 43).await.unwrap();

      let item = rows.iter().find(|row| row.item_id() == 101).unwrap();
      assert_eq!(
        item.location_type(),
        "structure",
        "an 'item' root is reclassified to a structure"
      );
      assert_eq!(item.depth(), 0);
      assert_eq!(item.container_id(), None);

      assert!(rows.iter().any(|row| row.item_id() == 9002), "the ship still lands");
    }

    #[tokio::test]
    async fn it_persists_an_asset_in_an_inaccessible_structure_and_marks_it() {
      let server = MockServer::start().await;
      mount_assets(
        &server,
        "/characters/44/assets/",
        serde_json::json!([
          { "is_singleton": true, "item_id": 500, "location_flag": "Hangar", "location_id": STRUCTURE_ID,
            "location_type": "structure", "quantity": 1, "type_id": 587 },
        ]),
      )
      .await;
      mount_ship(
        &server,
        44,
        serde_json::json!({ "ship_item_id": 9003, "ship_name": "Capsule", "ship_type_id": 587 }),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 44).await;
      seed_item_types(&db, &[587]).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes("token", 44, vec![scopes::UNIVERSE_STRUCTURES.to_owned()]);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Character(44));

      run(&ctx).await.unwrap();

      let rows = assets::for_character(&db, 44).await.unwrap();
      assert!(
        rows.iter().any(|row| row.item_id() == 500),
        "the asset in the inaccessible structure is still persisted, never dropped"
      );
      assert!(
        sde::is_structure_inaccessible(&db, 44, store::model::OwnerType::Character, STRUCTURE_ID)
          .await
          .unwrap(),
        "the 403 structure is durably marked inaccessible for the owner"
      );
    }

    #[tokio::test]
    async fn it_lands_a_top_level_citadel_asset_as_a_resolved_structure_root() {
      let server = MockServer::start().await;
      mount_assets(
        &server,
        "/characters/47/assets/",
        serde_json::json!([
          { "is_singleton": true, "item_id": 600, "location_flag": "Hangar", "location_id": CITADEL_ID,
            "location_type": "item", "quantity": 1, "type_id": 587 },
          { "is_singleton": false, "item_id": 601, "location_flag": "Cargo", "location_id": 600,
            "location_type": "item", "quantity": 3, "type_id": 34 },
        ]),
      )
      .await;
      mount_ship(
        &server,
        47,
        serde_json::json!({ "ship_item_id": 9004, "ship_name": "Capsule", "ship_type_id": 587 }),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 47).await;
      seed_item_types(&db, &[34, 587]).await;
      seed_citadel(&db, CITADEL_ID, "Test Citadel").await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes("token", 47, vec![scopes::UNIVERSE_STRUCTURES.to_owned()]);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Character(47));

      run(&ctx).await.unwrap();

      let rows = assets::for_character(&db, 47).await.unwrap();

      let root = rows.iter().find(|row| row.item_id() == 600).unwrap();
      assert_eq!(
        root.location_type(),
        "structure",
        "the 'item' citadel root is stored as a structure"
      );
      assert_eq!(root.depth(), 0);
      assert_eq!(root.container_id(), None);
      assert!(root.is_container(), "the citadel root holds 601");

      let child = rows.iter().find(|row| row.item_id() == 601).unwrap();
      assert_eq!(child.depth(), 1);
      assert_eq!(child.container_id(), Some(600));

      let rendered = assets::render_for_character(&db, 47).await.unwrap();
      let rendered_root = rendered.iter().find(|row| row.item_id == 600).unwrap();
      assert_eq!(
        rendered_root.location_label.as_deref(),
        Some("Test Citadel"),
        "the resolved citadel renders its name"
      );
    }

    #[tokio::test]
    async fn it_persists_a_top_level_citadel_asset_in_an_inaccessible_structure_and_marks_it() {
      let server = MockServer::start().await;
      mount_assets(
        &server,
        "/characters/48/assets/",
        serde_json::json!([
          { "is_singleton": true, "item_id": 700, "location_flag": "Hangar", "location_id": STRUCTURE_ID,
            "location_type": "item", "quantity": 1, "type_id": 587 },
          { "is_singleton": false, "item_id": 701, "location_flag": "Cargo", "location_id": 700,
            "location_type": "item", "quantity": 2, "type_id": 34 },
        ]),
      )
      .await;
      mount_ship(
        &server,
        48,
        serde_json::json!({ "ship_item_id": 9005, "ship_name": "Capsule", "ship_type_id": 587 }),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 48).await;
      seed_item_types(&db, &[34, 587]).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes("token", 48, vec![scopes::UNIVERSE_STRUCTURES.to_owned()]);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Character(48));

      run(&ctx).await.unwrap();

      let rows = assets::for_character(&db, 48).await.unwrap();

      let root = rows.iter().find(|row| row.item_id() == 700).unwrap();
      assert_eq!(
        root.location_type(),
        "structure",
        "the inaccessible 'item' root is stored as a structure"
      );
      assert_eq!(root.depth(), 0);
      assert_eq!(root.container_id(), None);

      let child = rows.iter().find(|row| row.item_id() == 701).unwrap();
      assert_eq!(
        child.depth(),
        1,
        "nested contents still land even when the structure is inaccessible"
      );
      assert_eq!(child.container_id(), Some(700));

      assert!(
        sde::is_structure_inaccessible(&db, 48, store::model::OwnerType::Character, STRUCTURE_ID)
          .await
          .unwrap(),
        "the 403 citadel is durably marked inaccessible for the owner"
      );
    }
  }

  mod run_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_corp_assets_with_hierarchy() {
      let server = MockServer::start().await;
      mount_assets(
        &server,
        "/corporations/90000001/assets/",
        serde_json::json!([
          { "is_singleton": true, "item_id": 300, "location_flag": "CorpDeliveries", "location_id": 60003760,
            "location_type": "station", "quantity": 1, "type_id": 587 },
          { "is_singleton": false, "item_id": 301, "location_flag": "Cargo", "location_id": 300,
            "location_type": "item", "quantity": 9, "type_id": 34 },
        ]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_types(&db, &[34, 587]).await;
      crate::store::repo::infra::upsert(
        &db,
        90_000_001,
        crate::store::model::OwnerType::Corporation,
        "tok",
        "rt",
        4_102_444_800,
        Some(42),
        None,
      )
      .await
      .unwrap();
      crate::store::repo::org::replace_for_corporation(
        &db,
        90_000_001,
        &[crate::store::model::CorporationMemberRole::from((
          90_000_001_i64,
          42_i64,
          "Director".to_string(),
        ))],
      )
      .await
      .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", 90_000_001);
      let ctx = ctx_with_grant(
        &db,
        &esi,
        &image,
        &image_store,
        &grant,
        Subject::Corporation(90_000_001),
      );

      run(&ctx).await.unwrap();

      let rows = assets::for_corporation(&db, 90_000_001).await.unwrap();
      assert_eq!(rows.len(), 2);
      let root = rows.iter().find(|row| row.item_id() == 300).unwrap();
      assert_eq!(root.container_id(), None);
      assert!(root.is_container());
      let child = rows.iter().find(|row| row.item_id() == 301).unwrap();
      assert_eq!(child.container_id(), Some(300));
      assert_eq!(child.depth(), 1);
    }

    #[tokio::test]
    async fn it_reports_empty_when_the_corporation_owns_no_assets() {
      let server = MockServer::start().await;
      mount_assets(&server, "/corporations/90000001/assets/", serde_json::json!([])).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", 90_000_001);
      let ctx = ctx_with_grant(
        &db,
        &esi,
        &image,
        &image_store,
        &grant,
        Subject::Corporation(90_000_001),
      );

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(outcome, Outcome::Empty);
    }
  }

  mod readiness_invariant {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      store::model::{Constellation, OwnerType, Region, SolarSystem, Structure},
      sync::{event::Event, job::JobKey, status::SyncStatus},
    };

    const INACCESSIBLE_ID: i64 = 1_021_000_000_001;
    const RESOLVED_ID: i64 = 1_021_000_000_002;

    async fn seed_named_structure(db: &store::Database, id: i64, name: &str) {
      sde::upsert_region(
        db,
        &Region {
          description: None,
          id: 10_000_002,
          name: "The Forge".to_owned(),
        },
      )
      .await
      .unwrap();
      sde::upsert_constellation(
        db,
        &Constellation {
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
        &SolarSystem {
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
        &Structure {
          id,
          name: name.to_owned(),
          owner_id: 90_000_001,
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
    async fn it_keeps_the_chip_in_progress_until_assets_are_displayable_then_done() {
      let server = MockServer::start().await;
      mount_assets(
        &server,
        "/characters/44/assets/",
        serde_json::json!([
          { "is_singleton": true, "item_id": 500, "location_flag": "Hangar", "location_id": INACCESSIBLE_ID,
            "location_type": "structure", "quantity": 1, "type_id": 587 },
          { "is_singleton": true, "item_id": 501, "location_flag": "Hangar", "location_id": RESOLVED_ID,
            "location_type": "structure", "quantity": 1, "type_id": 587 },
        ]),
      )
      .await;
      mount_ship(
        &server,
        44,
        serde_json::json!({ "ship_item_id": 9003, "ship_name": "Capsule", "ship_type_id": 587 }),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{INACCESSIBLE_ID}/")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 44).await;
      seed_item_types(&db, &[587]).await;
      seed_named_structure(&db, RESOLVED_ID, "Some Citadel").await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes("token", 44, vec![scopes::UNIVERSE_STRUCTURES.to_owned()]);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, Subject::Character(44));
      let key = JobKey::new(JobKind::AssetSync, Subject::Character(44));

      let mut status = SyncStatus::new();
      status.apply(&Event::Started {
        key,
      });

      assert!(status.is_syncing(), "the chip reads In Progress while AssetSync runs");
      assert!(
        assets::render_for_character(&db, 44).await.unwrap().is_empty(),
        "the screen is blank until AssetSync finishes"
      );

      run(&ctx).await.unwrap();
      status.apply(&Event::Finished {
        key,
        outcome: crate::sync::Outcome::synced(),
      });

      assert!(!status.is_syncing(), "Finished clears the In Progress chip");

      let rows = assets::render_for_character(&db, 44).await.unwrap();
      assert_eq!(rows.len(), 3, "two assets plus the synthetic ship are displayable");
      assert!(
        rows.iter().all(|row| !row.type_name.is_empty()),
        "every displayable row is named"
      );

      let inaccessible = rows.iter().find(|row| row.item_id == 500).unwrap();
      assert_eq!(
        inaccessible.location_label.as_deref(),
        Some("Inaccessible Structure"),
        "a 403 structure renders Inaccessible Structure rather than hanging the chip In Progress"
      );
      let resolved = rows.iter().find(|row| row.item_id == 501).unwrap();
      assert_eq!(resolved.location_label.as_deref(), Some("Some Citadel"));

      assert!(
        sde::is_structure_inaccessible(&db, 44, OwnerType::Character, INACCESSIBLE_ID)
          .await
          .unwrap(),
        "the inaccessible structure is durably marked for the owner"
      );
    }
  }
}
