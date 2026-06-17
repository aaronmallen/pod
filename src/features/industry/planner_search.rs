use std::sync::Arc;

use super::{planner::PinnedStructure, planner_loaders::PlannerFacility, planner_model::REACTION_ACTIVITY_ID};
use crate::{
  clients::{esi, eve_sso, eve_sso::Grant},
  store::{
    Database,
    model::OwnerType,
    repo::{character, industry, sde},
  },
};

const FACILITY_SEARCH_CATEGORIES: &[&str] = &["station", "structure"];
const MANUFACTURING_ACTIVITY_ID: i64 = 1;
const MAX_FACILITY_RESULTS: usize = 30;

pub async fn pin_facility(db: Database, pin: PinnedStructure) {
  if let Err(error) = sde::pin_structure(&db, pin.id, &pin.name, pin.solar_system_id, pin.type_id).await {
    tracing::warn!(target: "pod::industry", %error, structure_id = pin.id, "pinning facility failed");
  }
}

pub async fn search_facilities(
  db: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  query: String,
) -> Vec<PlannerFacility> {
  let Some(grant) = first_owned_grant(&db, &sso).await else {
    return Vec::new();
  };

  let result = match esi
    .universe()
    .search_with_categories(&query, FACILITY_SEARCH_CATEGORIES, &grant)
    .await
  {
    Ok(result) => result,
    Err(error) => {
      tracing::warn!(target: "pod::industry", %error, query = %query, "facility search failed");
      return Vec::new();
    }
  };

  let mut facilities = Vec::new();

  // NPC stations are already seeded in the SDE; resolve them locally with no ESI round-trip.
  for station_id in result.station {
    if let Ok(Some(station)) = sde::get_station(&db, station_id).await {
      facilities.push(
        facility_from(
          &db,
          station.id(),
          station.name().clone(),
          station.system_id(),
          Some(station.type_id()),
        )
        .await,
      );
    }
  }

  // Player structures need the per-id authenticated endpoint; a structure the character cannot dock at
  // (403) or that otherwise fails to resolve is skipped rather than aborting the whole search.
  for structure_id in result.structure {
    match esi.universe().structure(structure_id, &grant).await {
      Ok(structure) => {
        facilities.push(
          facility_from(
            &db,
            structure_id,
            structure.name,
            structure.solar_system_id,
            structure.type_id.map(i64::from),
          )
          .await,
        );
      }
      Err(error) => {
        tracing::warn!(target: "pod::industry", %error, structure_id, "facility structure resolution failed")
      }
    }
  }

  facilities.truncate(MAX_FACILITY_RESULTS);
  facilities
}

async fn facility_from(
  db: &Database,
  id: i64,
  name: String,
  solar_system_id: i64,
  type_id: Option<i64>,
) -> PlannerFacility {
  let (security_status, region, solar_system) = industry::system_geo(db, solar_system_id)
    .await
    .unwrap_or((None, None, None));
  PlannerFacility {
    id,
    manufacturing_index: super::planner_loaders::cost_index(db, solar_system_id, MANUFACTURING_ACTIVITY_ID).await,
    name,
    reaction_index: super::planner_loaders::cost_index(db, solar_system_id, REACTION_ACTIVITY_ID).await,
    region,
    security_status,
    solar_system,
    solar_system_id,
    type_id,
  }
}

async fn first_owned_grant(db: &Database, sso: &eve_sso::Client) -> Option<Grant> {
  let owner = character::all_owned(db).await.unwrap_or_default().into_iter().next()?;
  match crate::sync::token::fresh_token(db, sso, owner.id(), OwnerType::Character).await {
    Ok(grant) => grant,
    Err(error) => {
      tracing::warn!(target: "pod::industry", %error, "facility search: no usable token");
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
      model::{
        Alliance, Bloodline, Character, Constellation, Corporation, Gender, ItemType, OwnerType, Race, Region,
        SolarSystem, Station,
      },
      repo::{character, infra, sde},
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

  fn make_region() -> Region {
    Region {
      description: None,
      id: 10_000_001,
      name: "Test Region".to_owned(),
    }
  }

  fn make_constellation() -> Constellation {
    Constellation {
      id: 20_000_001,
      name: "Test Constellation".to_owned(),
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      region_id: 10_000_001,
    }
  }

  fn make_solar_system(id: i64) -> SolarSystem {
    SolarSystem {
      constellation_id: 20_000_001,
      id,
      name: "Test System".to_owned(),
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      security_class: None,
      security_status: 0.9,
      star_id: None,
    }
  }

  async fn seed_item_taxonomy(db: &Database) {
    sde::upsert_item_category(
      db,
      &store::model::ItemCategory {
        id: 1,
        icon_id: None,
        name: "Structure".to_owned(),
        published: true,
      },
    )
    .await
    .unwrap();
    sde::upsert_item_group(
      db,
      &store::model::ItemGroup {
        category_id: 1,
        icon_id: None,
        id: 1,
        name: "Citadel".to_owned(),
        published: true,
      },
    )
    .await
    .unwrap();
  }

  fn make_item_type(id: i64, name: &str) -> ItemType {
    ItemType {
      capacity: None,
      description: Some("A facility.".to_owned()),
      dogma_attributes: "[]".to_owned(),
      group_id: 1,
      icon_id: None,
      id,
      market_group_id: None,
      name: name.to_owned(),
      packaged_volume: None,
      portion_size: None,
      published: true,
      radius: None,
      volume: None,
    }
  }

  fn make_station(id: i64, system_id: i64) -> Station {
    Station {
      id,
      max_dockable_ship_volume: 0.0,
      name: "Jita IV - Moon 4 - CNAP".to_owned(),
      office_rental_cost: 0.0,
      owner: None,
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      race_id: None,
      reprocessing_efficiency: 0.0,
      reprocessing_stations_take: 0.0,
      services: String::new(),
      system_id,
      type_id: 54_678,
    }
  }

  async fn seed_geography(db: &Database, system_id: i64) {
    sde::upsert_region(db, &make_region()).await.unwrap();
    sde::upsert_constellation(db, &make_constellation()).await.unwrap();
    sde::upsert_solar_system(db, &make_solar_system(system_id))
      .await
      .unwrap();
  }

  mod pin_facility {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_a_pinned_structure() {
      let db = store::open_test().await.unwrap();
      seed_geography(&db, 30_000_142).await;

      pin_facility(
        db.clone(),
        PinnedStructure {
          id: 1_021_000_000_001,
          name: "Allied Fortizar".to_owned(),
          solar_system_id: 30_000_142,
          type_id: None,
        },
      )
      .await;

      let pinned: Option<(String, i64)> =
        sqlx::query_as("SELECT name, solar_system_id FROM pinned_structures WHERE id = ?")
          .bind(1_021_000_000_001_i64)
          .fetch_optional(&db.0)
          .await
          .unwrap();
      assert_eq!(pinned, Some(("Allied Fortizar".to_owned(), 30_000_142)));
    }
  }

  mod search_facilities {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_resolves_stations_locally_and_structures_via_esi() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .and(query_param("categories", "station,structure"))
        .and(query_param("search", "Jita"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"station":[60003760],"structure":[1021000000001]}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/structures/1021000000001/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"name":"Jita Keepstar","owner_id":98000001,"solar_system_id":30000142,"type_id":35834}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;
      seed_geography(&db, 30_000_142).await;
      seed_item_taxonomy(&db).await;
      sde::upsert_item_type(&db, &make_item_type(35_834, "Keepstar"))
        .await
        .unwrap();
      sde::upsert_item_type(&db, &make_item_type(54_678, "Station"))
        .await
        .unwrap();
      sde::upsert_station(&db, &make_station(60_003_760, 30_000_142))
        .await
        .unwrap();

      let results = search_facilities(db, esi, sso, "Jita".to_owned()).await;

      assert_eq!(results.len(), 2);
      assert_eq!(results[0].id, 60_003_760);
      assert_eq!(results[0].name, "Jita IV - Moon 4 - CNAP");
      assert_eq!(results[0].solar_system_id, 30_000_142);

      assert_eq!(results[1].id, 1_021_000_000_001);
      assert_eq!(results[1].name, "Jita Keepstar");
      assert_eq!(results[1].type_id, Some(35_834));
    }

    #[tokio::test]
    async fn it_returns_empty_without_a_credentialed_character() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;

      let results = search_facilities(db, esi, sso, "Jita".to_owned()).await;

      assert!(results.is_empty());
    }

    #[tokio::test]
    async fn it_skips_a_structure_the_character_cannot_dock_at() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .respond_with(
          ResponseTemplate::new(200).set_body_raw(r#"{"structure":[1021000000001,1021000000002]}"#, "application/json"),
        )
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/structures/1021000000001/"))
        .respond_with(ResponseTemplate::new(403).set_body_raw(r#"{"error":"Forbidden"}"#, "application/json"))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/universe/structures/1021000000002/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"name":"Allied Fortizar","owner_id":98000002,"solar_system_id":30000142,"type_id":35833}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;
      seed_geography(&db, 30_000_142).await;

      let results = search_facilities(db, esi, sso, "Fort".to_owned()).await;

      assert_eq!(results.len(), 1);
      assert_eq!(results[0].id, 1_021_000_000_002);
      assert_eq!(results[0].name, "Allied Fortizar");
    }
  }
}
