use std::sync::Arc;

use crate::{
  clients::{esi, eve_image, eve_sso, eve_sso::Grant},
  store::{
    Database, images,
    model::OwnerType,
    repo::{character, industry},
  },
};

const ITEM_SEARCH_CATEGORIES: &[&str] = &["inventory_type"];
const LOCATION_SEARCH_CATEGORIES: &[&str] = &["region", "constellation", "solar_system", "station", "structure"];
const MAX_ITEM_RESULTS: usize = 20;
const MAX_LOCATION_RESULTS: usize = 20;
const RESOLVE_NAMES_CHUNK: usize = 1000;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocationRef {
  pub context: Option<String>,
  pub id: i64,
  pub name: String,
  pub security_status: Option<f64>,
  pub tier: Option<LocationTier>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LocationTier {
  Constellation,
  Region,
  Station,
  Structure,
  System,
}

impl LocationTier {
  /// Infers the tier from an EVE entity id using the game's disjoint id ranges. Player structures
  /// occupy the 64-bit space at or above `1_000_000_000_000`; the NPC ranges below it are densely
  /// packed (regions `10M`, constellations `20M`, systems `30M`, stations `60M`).
  #[allow(dead_code)] // consumed by the stockpile editor wired in the editor task
  pub fn from_id(id: i64) -> Option<Self> {
    match id {
      1_000_000_000_000.. => Some(Self::Structure),
      60_000_000..=63_999_999 => Some(Self::Station),
      30_000_000..=39_999_999 => Some(Self::System),
      20_000_000..=29_999_999 => Some(Self::Constellation),
      10_000_000..=19_999_999 => Some(Self::Region),
      _ => None,
    }
  }

  /// Whether a security pill is meaningful for this tier. Regions and constellations span many
  /// systems with varied security, so they carry no single status.
  pub fn has_security(self) -> bool {
    matches!(self, Self::Station | Self::Structure | Self::System)
  }

  pub fn label(self) -> &'static str {
    match self {
      Self::Constellation => "CONST",
      Self::Region => "REGION",
      Self::Station => "STATION",
      Self::Structure => "STRUCTURE",
      Self::System => "SYSTEM",
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultibuyMatch {
  pub name: String,
  pub quantity: u64,
  pub type_id: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MultibuyResolution {
  pub matched: Vec<MultibuyMatch>,
  pub unmatched: Vec<String>,
}

pub async fn resolve_multibuy(
  db: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  entries: Vec<(String, u64)>,
) -> MultibuyResolution {
  if entries.is_empty() {
    return MultibuyResolution::default();
  }

  let mut resolved: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
  let names: Vec<String> = entries.iter().map(|(name, _)| name.clone()).collect();
  for chunk in names.chunks(RESOLVE_NAMES_CHUNK) {
    let ids = match esi.universe().ids(chunk).await {
      Ok(ids) => ids,
      Err(error) => {
        tracing::warn!(target: "pod::assets", %error, "multibuy resolve failed");
        continue;
      }
    };
    for record in ids.inventory_types {
      resolved.insert(record.name.to_lowercase(), record.id);
    }
  }

  let _ = first_owned_grant(&db, &sso).await;

  let mut resolution = MultibuyResolution::default();
  for (name, quantity) in entries {
    match resolved.get(&name.to_lowercase()) {
      Some(&type_id) => resolution.matched.push(MultibuyMatch {
        name,
        quantity,
        type_id,
      }),
      None => resolution.unmatched.push(name),
    }
  }
  resolution
}

pub async fn search_item_types(
  db: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  query: String,
) -> Vec<(i64, String)> {
  let Some(grant) = first_owned_grant(&db, &sso).await else {
    return Vec::new();
  };

  let result = match esi
    .universe()
    .search_with_categories(&query, ITEM_SEARCH_CATEGORIES, &grant)
    .await
  {
    Ok(result) => result,
    Err(error) => {
      tracing::warn!(target: "pod::assets", %error, query = %query, "item search failed");
      return Vec::new();
    }
  };

  let ids: Vec<i64> = result.inventory_type.into_iter().take(MAX_ITEM_RESULTS).collect();
  if ids.is_empty() {
    return Vec::new();
  }

  match esi.universe().names(&ids).await {
    Ok(names) => names
      .into_iter()
      .filter(|record| record.category == "inventory_type")
      .map(|record| (record.id, record.name))
      .collect(),
    Err(error) => {
      tracing::warn!(target: "pod::assets", %error, "item name resolution failed");
      Vec::new()
    }
  }
}

pub async fn search_locations(
  db: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  query: String,
) -> Vec<(i64, String)> {
  let Some(grant) = first_owned_grant(&db, &sso).await else {
    return Vec::new();
  };

  let result = match esi
    .universe()
    .search_with_categories(&query, LOCATION_SEARCH_CATEGORIES, &grant)
    .await
  {
    Ok(result) => result,
    Err(error) => {
      tracing::warn!(target: "pod::assets", %error, query = %query, "location search failed");
      return Vec::new();
    }
  };

  let mut named: Vec<(i64, String)> = Vec::new();

  // Regions, constellations, systems, and stations are public and resolve by name in one /universe/names
  // batch; only player structures need the per-id authenticated endpoint below.
  let mut public_ids: Vec<i64> = result.region;
  public_ids.extend(result.constellation);
  public_ids.extend(result.solar_system);
  public_ids.extend(result.station);
  if !public_ids.is_empty() {
    match esi.universe().names(&public_ids).await {
      Ok(names) => named.extend(names.into_iter().map(|record| (record.id, record.name))),
      Err(error) => {
        tracing::warn!(target: "pod::assets", %error, "location name resolution failed")
      }
    }
  }

  for structure_id in result.structure {
    match esi.universe().structure(structure_id, &grant).await {
      Ok(structure) => named.push((structure_id, structure.name)),
      Err(error) => {
        tracing::warn!(target: "pod::assets", %error, structure_id, "structure name resolution failed")
      }
    }
  }

  named.truncate(MAX_LOCATION_RESULTS);
  named
}

/// Enriched sibling of [`search_locations`]: returns [`LocationRef`]s carrying tier, a region/system
/// context chain, and a security status (for system/station/structure tiers) instead of bare
/// `(id, name)` tuples. Searches the same ESI categories, then backfills geography from the SDE via
/// [`industry::system_geo`].
#[allow(dead_code)] // wired into the stockpile editor by the editor task
pub async fn search_locations_enriched(
  db: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  query: String,
) -> Vec<LocationRef> {
  let Some(grant) = first_owned_grant(&db, &sso).await else {
    return Vec::new();
  };

  let result = match esi
    .universe()
    .search_with_categories(&query, LOCATION_SEARCH_CATEGORIES, &grant)
    .await
  {
    Ok(result) => result,
    Err(error) => {
      tracing::warn!(target: "pod::assets", %error, query = %query, "location search failed");
      return Vec::new();
    }
  };

  let mut refs: Vec<LocationRef> = Vec::new();

  // Regions, constellations, systems, and stations resolve by name in one batch; structures need the
  // per-id authenticated endpoint.
  let mut public_ids: Vec<i64> = result.region;
  public_ids.extend(result.constellation);
  public_ids.extend(result.solar_system);
  public_ids.extend(result.station);
  if !public_ids.is_empty() {
    match esi.universe().names(&public_ids).await {
      Ok(names) => {
        for record in names {
          let tier = LocationTier::from_id(record.id);
          let mut location = LocationRef {
            context: None,
            id: record.id,
            name: record.name,
            security_status: None,
            tier,
          };
          match tier {
            Some(LocationTier::System) => enrich_from_system(&db, &mut location, record.id).await,
            Some(LocationTier::Station) => {
              if let Ok(station) = esi.universe().station(record.id).await {
                enrich_from_system(&db, &mut location, station.system_id).await;
              }
            }
            _ => {}
          }
          refs.push(location);
        }
      }
      Err(error) => {
        tracing::warn!(target: "pod::assets", %error, "location name resolution failed")
      }
    }
  }

  for structure_id in result.structure {
    match esi.universe().structure(structure_id, &grant).await {
      Ok(structure) => {
        let mut location = LocationRef {
          context: None,
          id: structure_id,
          name: structure.name,
          security_status: None,
          tier: LocationTier::from_id(structure_id),
        };
        enrich_from_system(&db, &mut location, structure.solar_system_id).await;
        refs.push(location);
      }
      Err(error) => {
        tracing::warn!(target: "pod::assets", %error, structure_id, "structure name resolution failed")
      }
    }
  }

  refs.truncate(MAX_LOCATION_RESULTS);
  refs
}

/// Backfills `location`'s security and context chain from the SDE geography for `solar_system_id`.
/// The chain reads `region · system`, omitting blank segments; a system tier collapses to just its
/// region (the system name is already the row title).
#[allow(dead_code)] // reached only through search_locations_enriched, wired by the editor task
async fn enrich_from_system(db: &Database, location: &mut LocationRef, solar_system_id: i64) {
  let (security, region, system) = industry::system_geo(db, solar_system_id)
    .await
    .unwrap_or((None, None, None));
  location.security_status = security;

  let region = region
    .map(|name| name.trim().to_owned())
    .filter(|name| !name.is_empty());
  let system = system
    .map(|name| name.trim().to_owned())
    .filter(|name| !name.is_empty());
  let system = match location.tier {
    Some(LocationTier::System) => None,
    _ => system,
  };

  location.context = match (region, system) {
    (Some(region), Some(system)) => Some(format!("{region} \u{00B7} {system}")),
    (Some(region), None) => Some(region),
    (None, Some(system)) => Some(system),
    (None, None) => None,
  };
}

pub async fn resolve_location(
  db: Database,
  esi: Arc<esi::Client>,
  image: Arc<eve_image::Client>,
  sso: Arc<eve_sso::Client>,
  location_id: i64,
) {
  let Some(grant) = first_owned_grant(&db, &sso).await else {
    return;
  };
  if let Err(error) =
    crate::sync::resolve_stockpile_location(&db, &esi, &image, &images::default_store(), &grant, location_id).await
  {
    tracing::warn!(target: "pod::assets", %error, location_id, "stockpile location resolution failed");
  }
}

async fn first_owned_grant(db: &Database, sso: &eve_sso::Client) -> Option<Grant> {
  let owner = character::all_owned(db).await.unwrap_or_default().into_iter().next()?;
  match crate::sync::token::fresh_token(db, sso, owner.id(), OwnerType::Character).await {
    Ok(grant) => grant,
    Err(error) => {
      tracing::warn!(target: "pod::assets", %error, "stockpile search: no usable token");
      None
    }
  }
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
  };

  use super::*;
  use crate::{
    clients::{eve_sso, http},
    store::{
      self,
      model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
      repo::{character, infra},
    },
  };

  const CHAR: i64 = 42;

  async fn make_clients(base_url: &str) -> (Database, Arc<esi::Client>, Arc<eve_sso::Client>) {
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = Arc::new(esi::Client::with_base_url(http.clone(), base_url));
    let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
    (db, esi, sso)
  }

  async fn seed_owned_character(db: &Database) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, CHAR, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(CHAR);
    corp.set_creator_id(CHAR);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(CHAR, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
    let far_future = chrono::Utc::now().timestamp() + 86_400;
    infra::upsert(db, CHAR, OwnerType::Character, "tok", "rt", far_future, None, None)
      .await
      .unwrap();
  }

  mod resolve_multibuy {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_matches_known_names_and_reports_unmatched_ones() {
      let server = MockServer::start().await;
      let body = r#"{"inventory_types":[{"id":34,"name":"Tritanium"},{"id":35,"name":"Pyerite"}]}"#;
      Mock::given(method("POST"))
        .and(path("/universe/ids/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;

      let resolution = resolve_multibuy(
        db,
        esi,
        sso,
        vec![
          ("tritanium".to_owned(), 100),
          ("Pyerite".to_owned(), 50),
          ("Notathing".to_owned(), 5),
        ],
      )
      .await;

      assert_eq!(
        resolution.matched,
        vec![
          MultibuyMatch {
            name: "tritanium".to_owned(),
            quantity: 100,
            type_id: 34,
          },
          MultibuyMatch {
            name: "Pyerite".to_owned(),
            quantity: 50,
            type_id: 35,
          },
        ]
      );
      assert_eq!(resolution.unmatched, vec!["Notathing".to_owned()]);
    }

    #[tokio::test]
    async fn it_returns_an_empty_resolution_for_no_entries() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;

      let resolution = resolve_multibuy(db, esi, sso, Vec::new()).await;

      assert_eq!(resolution, MultibuyResolution::default());
    }
  }

  mod search_item_types {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_credentialed_character_exists() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;

      let results = search_item_types(db, esi, sso, "Trit".to_owned()).await;

      assert!(results.is_empty());
    }

    #[tokio::test]
    async fn it_returns_resolved_type_ids_and_names() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .and(query_param("categories", "inventory_type"))
        .and(query_param("search", "Trit"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"inventory_type":[34,35]}"#, "application/json"))
        .mount(&server)
        .await;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"[{"category":"inventory_type","id":34,"name":"Tritanium"},{"category":"inventory_type","id":35,"name":"Pyerite"}]"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;

      let results = search_item_types(db, esi, sso, "Trit".to_owned()).await;

      assert_eq!(results, vec![(34, "Tritanium".to_owned()), (35, "Pyerite".to_owned())]);
    }
  }

  mod search_locations {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_resolves_public_locations_via_names_and_structures_via_the_authenticated_endpoint() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .and(query_param("categories", "region,constellation,solar_system,station,structure"))
        .and(query_param("search", "Jita"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"constellation":[20000020],"region":[10000002],"solar_system":[30000142],"station":[60003760],"structure":[1234567890]}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"[{"category":"region","id":10000002,"name":"The Forge"},{"category":"constellation","id":20000020,"name":"Kimotoro"},{"category":"solar_system","id":30000142,"name":"Jita"},{"category":"station","id":60003760,"name":"Jita IV - Moon 4 - CNAP"}]"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/structures/1234567890/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"name":"Jita Trade Hub","owner_id":98000001,"solar_system_id":30000142,"type_id":35833}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;

      let results = search_locations(db, esi, sso, "Jita".to_owned()).await;

      assert_eq!(
        results,
        vec![
          (10000002, "The Forge".to_owned()),
          (20000020, "Kimotoro".to_owned()),
          (30000142, "Jita".to_owned()),
          (60003760, "Jita IV - Moon 4 - CNAP".to_owned()),
          (1234567890, "Jita Trade Hub".to_owned()),
        ]
      );
    }
  }

  mod location_tier {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_infers_a_tier_from_each_eve_id_range() {
      assert_eq!(LocationTier::from_id(10_000_002), Some(LocationTier::Region));
      assert_eq!(LocationTier::from_id(20_000_020), Some(LocationTier::Constellation));
      assert_eq!(LocationTier::from_id(30_000_142), Some(LocationTier::System));
      assert_eq!(LocationTier::from_id(60_003_760), Some(LocationTier::Station));
      assert_eq!(LocationTier::from_id(1_000_000_000_000), Some(LocationTier::Structure));
    }

    #[test]
    fn it_reports_security_only_for_dockable_tiers() {
      assert!(LocationTier::System.has_security());
      assert!(LocationTier::Station.has_security());
      assert!(LocationTier::Structure.has_security());

      assert!(!LocationTier::Region.has_security());
      assert!(!LocationTier::Constellation.has_security());
    }

    #[test]
    fn it_returns_no_tier_for_an_id_outside_the_known_ranges() {
      assert_eq!(LocationTier::from_id(42), None);
    }
  }

  mod search_locations_enriched {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_geography(db: &Database) {
      sqlx::query("INSERT INTO regions (id, name) VALUES (10000002, 'The Forge')")
        .execute(&db.0)
        .await
        .unwrap();
      sqlx::query(
        "INSERT INTO constellations (id, name, position_x, position_y, position_z, region_id) \
        VALUES (20000020, 'Kimotoro', 0, 0, 0, 10000002)",
      )
      .execute(&db.0)
      .await
      .unwrap();
      sqlx::query(
        "INSERT INTO solar_systems \
          (id, constellation_id, name, position_x, position_y, position_z, security_status) \
        VALUES (30000142, 20000020, 'Jita', 0, 0, 0, 0.9)",
      )
      .execute(&db.0)
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_backfills_a_context_chain_and_security_for_dockable_tiers() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .and(query_param(
          "categories",
          "region,constellation,solar_system,station,structure",
        ))
        .and(query_param("search", "Jita"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"region":[10000002],"solar_system":[30000142],"station":[60003760],"structure":[1035000000001]}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"[{"category":"region","id":10000002,"name":"The Forge"},{"category":"solar_system","id":30000142,"name":"Jita"},{"category":"station","id":60003760,"name":"Jita IV - Moon 4 - CNAP"}]"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/stations/60003760/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"max_dockable_ship_volume":0.0,"name":"Jita IV - Moon 4 - CNAP","office_rental_cost":0.0,"position":{"x":0.0,"y":0.0,"z":0.0},"reprocessing_efficiency":0.5,"reprocessing_stations_take":0.05,"services":[],"station_id":60003760,"system_id":30000142,"type_id":52678}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/structures/1035000000001/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"name":"Jita Trade Hub","owner_id":98000001,"solar_system_id":30000142,"type_id":35833}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;
      seed_geography(&db).await;

      let results = search_locations_enriched(db, esi, sso, "Jita".to_owned()).await;

      let by_id = |id: i64| results.iter().find(|r| r.id == id).cloned().unwrap();
      let region = by_id(10000002);
      let system = by_id(30000142);
      let station = by_id(60003760);
      let structure = by_id(1035000000001);

      assert_eq!(region.context, None);
      assert_eq!(region.security_status, None);

      assert_eq!(system.context, Some("The Forge".to_owned()));
      assert_eq!(system.security_status, Some(0.9));

      assert_eq!(station.context, Some("The Forge \u{00B7} Jita".to_owned()));
      assert_eq!(station.security_status, Some(0.9));

      assert_eq!(structure.context, Some("The Forge \u{00B7} Jita".to_owned()));
      assert_eq!(structure.security_status, Some(0.9));
    }

    #[tokio::test]
    async fn it_tags_each_result_with_its_tier() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .and(query_param("categories", "region,constellation,solar_system,station,structure"))
        .and(query_param("search", "Jita"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"constellation":[20000020],"region":[10000002],"solar_system":[30000142],"station":[60003760],"structure":[1035000000001]}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"[{"category":"region","id":10000002,"name":"The Forge"},{"category":"constellation","id":20000020,"name":"Kimotoro"},{"category":"solar_system","id":30000142,"name":"Jita"},{"category":"station","id":60003760,"name":"Jita IV - Moon 4 - CNAP"}]"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/stations/60003760/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"max_dockable_ship_volume":0.0,"name":"Jita IV - Moon 4 - CNAP","office_rental_cost":0.0,"position":{"x":0.0,"y":0.0,"z":0.0},"reprocessing_efficiency":0.5,"reprocessing_stations_take":0.05,"services":[],"station_id":60003760,"system_id":30000142,"type_id":52678}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/structures/1035000000001/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"name":"Jita Trade Hub","owner_id":98000001,"solar_system_id":30000142,"type_id":35833}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;

      let results = search_locations_enriched(db, esi, sso, "Jita".to_owned()).await;

      let tiers: Vec<(i64, Option<LocationTier>)> = results.iter().map(|r| (r.id, r.tier)).collect();
      assert_eq!(
        tiers,
        vec![
          (10000002, Some(LocationTier::Region)),
          (20000020, Some(LocationTier::Constellation)),
          (30000142, Some(LocationTier::System)),
          (60003760, Some(LocationTier::Station)),
          (1035000000001, Some(LocationTier::Structure)),
        ]
      );
    }
  }
}
