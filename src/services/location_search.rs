use std::{collections::HashSet, sync::Arc};

use crate::{
  clients::{esi, esi::scopes, eve_sso, eve_sso::Grant},
  store::{
    Database,
    model::OwnerType,
    repo::{character, sde},
  },
};

const MAX_LOCATION_RESULTS: usize = 20;
const STRUCTURE_SEARCH_CATEGORIES: &[&str] = &["structure"];

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

  pub fn parse(value: &str) -> Option<Self> {
    match value {
      "constellation" => Some(Self::Constellation),
      "region" => Some(Self::Region),
      "station" => Some(Self::Station),
      "structure" => Some(Self::Structure),
      "system" => Some(Self::System),
      _ => None,
    }
  }

  pub fn as_str(self) -> &'static str {
    match self {
      Self::Constellation => "constellation",
      Self::Region => "region",
      Self::Station => "station",
      Self::Structure => "structure",
      Self::System => "system",
    }
  }

  pub fn has_security(self) -> bool {
    matches!(self, Self::Station | Self::Structure | Self::System)
  }

  pub fn label(self) -> String {
    match self {
      Self::Constellation => t!("assets.location_tier.constellation").into_owned(),
      Self::Region => t!("assets.location_tier.region").into_owned(),
      Self::Station => t!("assets.location_tier.station").into_owned(),
      Self::Structure => t!("assets.location_tier.structure").into_owned(),
      Self::System => t!("assets.location_tier.system").into_owned(),
    }
  }
}

pub async fn search_locations_enriched(
  db: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  query: String,
  min_chars: usize,
) -> Vec<LocationRef> {
  let trimmed = query.trim();
  if trimmed.chars().count() < min_chars {
    return Vec::new();
  }
  let needle = trimmed.to_lowercase();

  let mut results = matching_regions(&db, &needle).await;
  results.extend(matching_constellations(&db, &needle).await);
  results.extend(matching_systems(&db, &needle).await);
  results.extend(matching_stations(&db, &needle).await);
  results.extend(structures(&db, &esi, &sso, trimmed, &needle).await);

  results.sort_by(|left, right| left.name.cmp(&right.name));
  results.truncate(MAX_LOCATION_RESULTS);
  results
}

pub(crate) async fn first_owned_grant(db: &Database, sso: &eve_sso::Client) -> Option<Grant> {
  let owner = character::all_owned(db).await.unwrap_or_default().into_iter().next()?;
  match crate::sync::token::fresh_token(db, sso, owner.id(), OwnerType::Character).await {
    Ok(grant) => grant,
    Err(error) => {
      tracing::warn!(target: "pod::location_search", %error, "location search: no usable token");
      None
    }
  }
}

async fn cached_structures(db: &Database, needle: &str) -> Vec<LocationRef> {
  sde::all_structures(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter(|structure| structure.name().to_lowercase().contains(needle))
    .map(|structure| plain_ref(structure.id(), structure.name().to_owned(), LocationTier::Structure))
    .collect()
}

// Player structures live behind the per-id authenticated endpoint. A structure the character cannot
// dock at (403) or that otherwise fails to resolve is skipped rather than aborting the whole search;
// a missing grant or absent search scopes degrade to no live hits (cached structures still surface).
async fn live_structures(db: &Database, esi: &esi::Client, sso: &eve_sso::Client, query: &str) -> Vec<LocationRef> {
  let Some(grant) = first_owned_grant(db, sso).await else {
    return Vec::new();
  };
  if !grant.has_scope(scopes::CHARACTER_SEARCH) || !grant.has_scope(scopes::UNIVERSE_STRUCTURES) {
    return Vec::new();
  }

  let ids = match esi
    .universe()
    .search_with_categories(query, STRUCTURE_SEARCH_CATEGORIES, &grant)
    .await
  {
    Ok(result) => result.structure,
    Err(error) => {
      tracing::warn!(target: "pod::location_search", %error, query = %query, "structure search failed");
      return Vec::new();
    }
  };

  let mut refs = Vec::new();
  for structure_id in ids {
    match esi.universe().structure(structure_id, &grant).await {
      Ok(structure) => refs.push(plain_ref(structure_id, structure.name, LocationTier::Structure)),
      Err(error) => {
        tracing::warn!(target: "pod::location_search", %error, structure_id, "structure resolution failed")
      }
    }
  }
  refs
}

async fn matching_constellations(db: &Database, needle: &str) -> Vec<LocationRef> {
  sde::all_constellations(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter(|constellation| constellation.name().to_lowercase().contains(needle))
    .map(|constellation| {
      plain_ref(
        constellation.id(),
        constellation.name().to_owned(),
        LocationTier::Constellation,
      )
    })
    .collect()
}

async fn matching_regions(db: &Database, needle: &str) -> Vec<LocationRef> {
  sde::all_regions(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter(|region| region.name().to_lowercase().contains(needle))
    .map(|region| plain_ref(region.id(), region.name().to_owned(), LocationTier::Region))
    .collect()
}

async fn matching_stations(db: &Database, needle: &str) -> Vec<LocationRef> {
  sde::all_stations(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter(|station| station.name().to_lowercase().contains(needle))
    .map(|station| plain_ref(station.id(), station.name().to_owned(), LocationTier::Station))
    .collect()
}

async fn matching_systems(db: &Database, needle: &str) -> Vec<LocationRef> {
  sde::all_solar_systems(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter(|system| system.name().to_lowercase().contains(needle))
    .map(|system| plain_ref(system.id(), system.name().to_owned(), LocationTier::System))
    .collect()
}

fn plain_ref(id: i64, name: String, tier: LocationTier) -> LocationRef {
  LocationRef {
    context: None,
    id,
    name,
    security_status: None,
    tier: Some(tier),
  }
}

// Live discovery unioned with cached rows, deduped by id so an owned/cached structure that also comes
// back from the live search appears once. Cached rows keep the picker useful offline and unscoped.
async fn structures(
  db: &Database,
  esi: &esi::Client,
  sso: &eve_sso::Client,
  query: &str,
  needle: &str,
) -> Vec<LocationRef> {
  let mut refs = live_structures(db, esi, sso, query).await;
  let mut seen: HashSet<i64> = refs.iter().map(|location| location.id).collect();
  for cached in cached_structures(db, needle).await {
    if seen.insert(cached.id) {
      refs.push(cached);
    }
  }
  refs
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
      model::{
        Alliance, Bloodline, Character, Constellation, Corporation, Gender, OwnerType, Race, Region, SolarSystem,
        Station, Structure,
      },
      repo::{character, infra, sde},
    },
  };

  const CHAR: i64 = 42;
  const SEARCH_SCOPES: &str = "esi-search.search_structures.v1 esi-universe.read_structures.v1";

  async fn make_clients(base_url: &str) -> (Database, Arc<esi::Client>, Arc<eve_sso::Client>) {
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = Arc::new(esi::Client::with_base_url(http.clone(), base_url));
    let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
    (db, esi, sso)
  }

  fn make_constellation(id: i64, name: &str) -> Constellation {
    Constellation {
      id,
      name: name.to_owned(),
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      region_id: 10_000_002,
    }
  }

  fn make_region(id: i64, name: &str) -> Region {
    Region {
      description: None,
      id,
      name: name.to_owned(),
    }
  }

  fn make_solar_system(id: i64, name: &str) -> SolarSystem {
    SolarSystem {
      constellation_id: 20_000_020,
      id,
      name: name.to_owned(),
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      security_class: None,
      security_status: 0.9,
      star_id: None,
    }
  }

  fn make_station(id: i64, name: &str) -> Station {
    Station {
      id,
      max_dockable_ship_volume: 0.0,
      name: name.to_owned(),
      office_rental_cost: 0.0,
      owner: None,
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      race_id: None,
      reprocessing_efficiency: 0.0,
      reprocessing_stations_take: 0.0,
      services: String::new(),
      system_id: 30_000_142,
      type_id: 54_678,
    }
  }

  fn make_structure(id: i64, name: &str) -> Structure {
    Structure {
      id,
      name: name.to_owned(),
      owner_id: 98_000_001,
      position_x: None,
      position_y: None,
      position_z: None,
      solar_system_id: 30_000_142,
      type_id: None,
    }
  }

  async fn seed_owned_character(db: &Database, scopes: Option<&str>) {
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
    infra::upsert(db, CHAR, OwnerType::Character, "tok", "rt", far_future, None, scopes)
      .await
      .unwrap();
  }

  async fn seed_item_type(db: &Database, type_id: i64) {
    sqlx::query("INSERT INTO item_categories (id, name, published) VALUES (6, 'Ship', 1)")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query("INSERT INTO item_groups (id, category_id, name, published) VALUES (25, 6, 'Frigate', 1)")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query(
      "INSERT INTO item_types (id, group_id, description, name, published) VALUES (?, 25, '', 'Station Type', 1)",
    )
    .bind(type_id)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_structure_parents(db: &Database) {
    sde::upsert_region(db, &make_region(10_000_002, "Home Region"))
      .await
      .unwrap();
    sde::upsert_constellation(db, &make_constellation(20_000_020, "Home Constellation"))
      .await
      .unwrap();
    sde::upsert_solar_system(db, &make_solar_system(30_000_142, "Home System"))
      .await
      .unwrap();
    sqlx::query(
      "INSERT INTO corporations (id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
      VALUES (98000001, 1, 1, 1, 'Owner Corp', 0.0, 'OWN')",
    )
    .execute(db.writer())
    .await
    .unwrap();
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

    #[test]
    fn it_round_trips_every_tier_through_its_wire_string() {
      for tier in [
        LocationTier::Constellation,
        LocationTier::Region,
        LocationTier::Station,
        LocationTier::Structure,
        LocationTier::System,
      ] {
        assert_eq!(LocationTier::parse(tier.as_str()), Some(tier));
      }

      assert_eq!(LocationTier::parse("nowhere"), None);
    }
  }

  mod search_locations_enriched {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_geography(db: &Database) {
      sde::upsert_region(db, &make_region(10_000_002, "The Forge"))
        .await
        .unwrap();
      sde::upsert_constellation(db, &make_constellation(20_000_020, "Kimotoro"))
        .await
        .unwrap();
      sde::upsert_solar_system(db, &make_solar_system(30_000_142, "Jita"))
        .await
        .unwrap();
      seed_item_type(db, 54_678).await;
      sde::upsert_station(db, &make_station(60_003_760, "Jita IV - Moon 4 - CNAP"))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_returns_the_sde_tiers_without_a_grant() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_geography(&db).await;

      let results = search_locations_enriched(db, esi, sso, "Jita".to_owned(), 1).await;

      let ids: Vec<i64> = results.iter().map(|location| location.id).collect();
      assert_eq!(ids, vec![30_000_142, 60_003_760]);
    }

    #[tokio::test]
    async fn it_unions_cached_structures_without_a_grant() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_structure_parents(&db).await;
      sde::upsert_structure(&db, &make_structure(1_035_000_000_001, "Jita Trade Hub"))
        .await
        .unwrap();

      let results = search_locations_enriched(db, esi, sso, "Jita".to_owned(), 1).await;

      let ids: Vec<i64> = results.iter().map(|location| location.id).collect();
      assert_eq!(ids, vec![1_035_000_000_001]);
    }

    #[tokio::test]
    async fn it_returns_empty_below_the_min_char_threshold() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_geography(&db).await;

      let results = search_locations_enriched(db, esi, sso, "Ji".to_owned(), 3).await;

      assert!(results.is_empty());
    }

    #[tokio::test]
    async fn it_caps_the_result_count() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      for offset in 0..25 {
        sde::upsert_region(&db, &make_region(10_000_100 + offset, &format!("Region {offset:02}")))
          .await
          .unwrap();
      }

      let results = search_locations_enriched(db, esi, sso, "Region".to_owned(), 1).await;

      assert_eq!(results.len(), MAX_LOCATION_RESULTS);
    }

    #[tokio::test]
    async fn it_orders_results_alphabetically_by_name() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      sde::upsert_region(&db, &make_region(10_000_002, "Zebra Region"))
        .await
        .unwrap();
      sde::upsert_constellation(&db, &make_constellation(20_000_020, "Mango Region"))
        .await
        .unwrap();
      sde::upsert_solar_system(&db, &make_solar_system(30_000_142, "Alpha Region"))
        .await
        .unwrap();

      let results = search_locations_enriched(db, esi, sso, "Region".to_owned(), 1).await;

      let names: Vec<String> = results.iter().map(|location| location.name.clone()).collect();
      assert_eq!(names, vec!["Alpha Region", "Mango Region", "Zebra Region"]);
    }

    #[tokio::test]
    async fn it_degrades_to_cached_structures_when_the_search_scopes_are_absent() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db, None).await;
      seed_structure_parents(&db).await;
      sde::upsert_structure(&db, &make_structure(1_035_000_000_001, "Cached Fortizar"))
        .await
        .unwrap();

      let results = search_locations_enriched(db, esi, sso, "Fortizar".to_owned(), 1).await;

      let ids: Vec<i64> = results.iter().map(|location| location.id).collect();
      assert_eq!(ids, vec![1_035_000_000_001]);
    }

    #[tokio::test]
    async fn it_discovers_live_structures_and_excludes_forbidden_ones() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .and(query_param("categories", "structure"))
        .and(query_param("search", "Fortizar"))
        .respond_with(
          ResponseTemplate::new(200).set_body_raw(r#"{"structure":[1035000000001,1035000000002]}"#, "application/json"),
        )
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/structures/1035000000001/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"name":"Dockable Fortizar","owner_id":98000001,"solar_system_id":30000142,"type_id":35833}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/structures/1035000000002/"))
        .respond_with(ResponseTemplate::new(403).set_body_raw(r#"{"error":"Forbidden"}"#, "application/json"))
        .mount(&server)
        .await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db, Some(SEARCH_SCOPES)).await;

      let results = search_locations_enriched(db, esi, sso, "Fortizar".to_owned(), 1).await;

      let resolved: Vec<(i64, String, Option<LocationTier>)> = results
        .into_iter()
        .map(|location| (location.id, location.name, location.tier))
        .collect();
      assert_eq!(
        resolved,
        vec![(
          1_035_000_000_001,
          "Dockable Fortizar".to_owned(),
          Some(LocationTier::Structure)
        )]
      );
    }
  }
}
