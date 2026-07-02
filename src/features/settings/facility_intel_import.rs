use std::sync::Arc;

use super::facility_intel_share::PortableFacility;
use crate::{
  clients::{esi, eve_sso, eve_sso::Grant},
  features::industry::{PinnedStructure, first_owned_grant, pin_facility},
  store::{
    Database,
    repo::{industry, sde},
  },
  ui::components::facility_combobox::MIN_STRUCTURE_ID,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportSummary {
  pub imported: usize,
  pub skipped: Vec<String>,
}

/// Overwrites existing facility intel on import rather than merging it: a rig missing from the incoming snapshot
/// clears any rig previously recorded for that facility.
pub async fn import_facilities(
  db: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  facilities: Vec<PortableFacility>,
) -> ImportSummary {
  let grant = first_owned_grant(&db, &sso).await;
  let mut summary = ImportSummary::default();
  for facility in facilities {
    if resolve_facility(&db, &esi, grant.as_ref(), &facility).await {
      let intel = facility.to_intel();
      let written = industry::upsert_facility_intel(
        &db,
        intel.facility_id,
        intel.rig_1_type_id,
        intel.rig_2_type_id,
        intel.rig_3_type_id,
      )
      .await
      .is_ok();
      if written {
        summary.imported += 1;
        continue;
      }
    }
    summary.skipped.push(skip_label(&facility));
  }
  summary
}

pub fn skipped_summary(facilities: &[PortableFacility]) -> ImportSummary {
  ImportSummary {
    imported: 0,
    skipped: facilities.iter().map(skip_label).collect(),
  }
}

async fn resolve_facility(
  db: &Database,
  esi: &esi::Client,
  grant: Option<&Grant>,
  facility: &PortableFacility,
) -> bool {
  if facility.facility_id < MIN_STRUCTURE_ID {
    return matches!(sde::get_station(db, facility.facility_id).await, Ok(Some(_)));
  }
  let Some(grant) = grant else {
    return false;
  };
  let Ok(structure) = esi.universe().structure(facility.facility_id, grant).await else {
    return false;
  };
  pin_facility(
    db.clone(),
    PinnedStructure {
      id: facility.facility_id,
      name: structure.name,
      solar_system_id: structure.solar_system_id,
      type_id: structure.type_id.map(i64::from),
    },
  )
  .await;
  true
}

fn skip_label(facility: &PortableFacility) -> String {
  let name = facility
    .name
    .clone()
    .unwrap_or_else(|| format!("#{}", facility.facility_id));
  match &facility.system {
    Some(system) => format!("{name} \u{b7} {system}"),
    None => name,
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
    clients::http,
    store::{
      self,
      model::{
        Alliance, Bloodline, Character, Constellation, Corporation, FacilityIntel, Gender, OwnerType, Race, Region,
        SolarSystem, Station,
      },
      repo::{character, infra},
    },
  };

  const CHAR: i64 = 42;
  const STATION_ID: i64 = 60_003_760;
  const STRUCTURE_ID: i64 = 1_021_000_000_001;

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

  async fn seed_item_type(db: &Database, type_id: i64, name: &str) {
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
    sde::upsert_item_type(
      db,
      &store::model::ItemType {
        capacity: None,
        description: Some("A facility.".to_owned()),
        dogma_attributes: "[]".to_owned(),
        group_id: 1,
        icon_id: None,
        id: type_id,
        market_group_id: None,
        name: name.to_owned(),
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

  async fn seed_geography(db: &Database, system_id: i64) {
    sde::upsert_region(
      db,
      &Region {
        description: None,
        id: 10_000_001,
        name: "Test Region".to_owned(),
      },
    )
    .await
    .unwrap();
    sde::upsert_constellation(
      db,
      &Constellation {
        id: 20_000_001,
        name: "Test Constellation".to_owned(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        region_id: 10_000_001,
      },
    )
    .await
    .unwrap();
    sde::upsert_solar_system(
      db,
      &SolarSystem {
        constellation_id: 20_000_001,
        id: system_id,
        name: "Test System".to_owned(),
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

  async fn seed_rig_types(db: &Database) {
    seed_item_type(db, 37_180, "Standup M-Set ME I").await;
    seed_item_type(db, 43_704, "Standup XL-Set TE I").await;
  }

  async fn seed_station(db: &Database, id: i64, system_id: i64) {
    seed_geography(db, system_id).await;
    seed_item_type(db, 54_678, "Station").await;
    seed_rig_types(db).await;
    sde::upsert_station(
      db,
      &Station {
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
      },
    )
    .await
    .unwrap();
  }

  fn portable(facility_id: i64, name: Option<&str>, system: Option<&str>) -> PortableFacility {
    PortableFacility {
      facility_id,
      name: name.map(str::to_owned),
      rigs: [Some(37_180), None, Some(43_704)],
      system: system.map(str::to_owned),
    }
  }

  async fn intel_rows(db: &Database) -> Vec<FacilityIntel> {
    industry::list_facility_intel(db).await.unwrap()
  }

  mod import_facilities {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_imports_a_station_from_the_sde_without_esi() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_station(&db, STATION_ID, 30_000_142).await;

      let summary = import_facilities(db.clone(), esi, sso, vec![portable(STATION_ID, None, None)]).await;

      assert_eq!(summary.imported, 1);
      assert!(summary.skipped.is_empty());

      let rows = intel_rows(&db).await;
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].facility_id, STATION_ID);
      assert_eq!(rows[0].rig_1_type_id, Some(37_180));
      assert_eq!(rows[0].rig_3_type_id, Some(43_704));
    }

    #[tokio::test]
    async fn it_overwrites_the_rigs_of_a_matching_facility() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_station(&db, STATION_ID, 30_000_142).await;
      industry::upsert_facility_intel(&db, STATION_ID, Some(43_704), Some(37_180), None)
        .await
        .unwrap();

      let summary = import_facilities(db.clone(), esi, sso, vec![portable(STATION_ID, None, None)]).await;

      assert_eq!(summary.imported, 1);

      let rows = intel_rows(&db).await;
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].rig_1_type_id, Some(37_180));
      assert_eq!(rows[0].rig_2_type_id, None);
      assert_eq!(rows[0].rig_3_type_id, Some(43_704));
    }

    #[tokio::test]
    async fn it_skips_an_unknown_station_and_names_it_from_the_envelope() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;

      let summary = import_facilities(
        db.clone(),
        esi,
        sso,
        vec![portable(STATION_ID, Some("Old Hub"), Some("Jita"))],
      )
      .await;

      assert_eq!(summary.imported, 0);
      assert_eq!(summary.skipped, vec!["Old Hub \u{b7} Jita".to_owned()]);

      assert!(intel_rows(&db).await.is_empty());
    }

    #[tokio::test]
    async fn it_resolves_pins_and_imports_a_structure_via_esi() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"name":"Jita Keepstar","owner_id":98000001,"solar_system_id":30000142,"type_id":35834}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;
      seed_geography(&db, 30_000_142).await;
      seed_item_type(&db, 35_834, "Keepstar").await;
      seed_rig_types(&db).await;

      let summary = import_facilities(db.clone(), esi, sso, vec![portable(STRUCTURE_ID, None, None)]).await;

      assert_eq!(summary.imported, 1);
      assert!(summary.skipped.is_empty());

      let rows = intel_rows(&db).await;
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].facility_id, STRUCTURE_ID);

      let pinned: Option<(String, i64)> =
        sqlx::query_as("SELECT name, solar_system_id FROM pinned_structures WHERE id = ?")
          .bind(STRUCTURE_ID)
          .fetch_optional(&db.0)
          .await
          .unwrap();
      assert_eq!(pinned, Some(("Jita Keepstar".to_owned(), 30_000_142)));
    }

    #[tokio::test]
    async fn it_skips_structures_without_a_usable_token_but_keeps_stations() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_station(&db, STATION_ID, 30_000_142).await;

      let summary = import_facilities(
        db.clone(),
        esi,
        sso,
        vec![
          portable(STATION_ID, None, None),
          portable(STRUCTURE_ID, Some("Allied Fortizar"), Some("Test System")),
        ],
      )
      .await;

      assert_eq!(summary.imported, 1);
      assert_eq!(summary.skipped, vec!["Allied Fortizar \u{b7} Test System".to_owned()]);

      let rows = intel_rows(&db).await;
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].facility_id, STATION_ID);
    }

    #[tokio::test]
    async fn it_skips_a_structure_esi_cannot_resolve() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(403).set_body_raw(r#"{"error":"Forbidden"}"#, "application/json"))
        .mount(&server)
        .await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;

      let summary = import_facilities(db.clone(), esi, sso, vec![portable(STRUCTURE_ID, None, None)]).await;

      assert_eq!(summary.imported, 0);
      assert_eq!(summary.skipped, vec![format!("#{STRUCTURE_ID}")]);

      assert!(intel_rows(&db).await.is_empty());
    }
  }

  mod skipped_summary {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_skips_every_facility_without_importing() {
      let facilities = vec![
        portable(STATION_ID, Some("Old Hub"), None),
        portable(STRUCTURE_ID, None, None),
      ];

      let summary = skipped_summary(&facilities);

      assert_eq!(summary.imported, 0);
      assert_eq!(summary.skipped, vec!["Old Hub".to_owned(), format!("#{STRUCTURE_ID}")]);
    }
  }

  mod skip_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_joins_the_snapshot_name_and_system() {
      assert_eq!(
        skip_label(&portable(STRUCTURE_ID, Some("Allied Fortizar"), Some("Jita"))),
        "Allied Fortizar \u{b7} Jita"
      );
    }

    #[test]
    fn it_falls_back_to_the_facility_id_without_a_snapshot() {
      assert_eq!(
        skip_label(&portable(STRUCTURE_ID, None, None)),
        format!("#{STRUCTURE_ID}")
      );
    }
  }
}
