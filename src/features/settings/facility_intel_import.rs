use super::facility_intel_share::PortableFacility;
use crate::store::{Database, repo::industry};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportSummary {
  pub imported: usize,
  pub skipped: Vec<String>,
}

/// Imports every entry in a valid pack as a straight DB upsert, using each entry's own name/system/type snapshot —
/// so intel for a facility the importer cannot access still imports. Overwrites rather than merges: a rig missing
/// from the incoming snapshot clears any rig previously recorded for that facility. Only a DB write failure skips
/// an entry.
pub async fn import_facilities(db: Database, facilities: Vec<PortableFacility>) -> ImportSummary {
  let mut summary = ImportSummary::default();
  for facility in facilities {
    let intel = facility.to_intel();
    let written = industry::upsert_facility_intel(
      &db,
      intel.facility_id,
      intel.name,
      intel.rig_1_type_id,
      intel.rig_2_type_id,
      intel.rig_3_type_id,
      intel.solar_system_id,
      intel.type_id,
    )
    .await
    .is_ok();
    if written {
      summary.imported += 1;
    } else {
      summary.skipped.push(skip_label(&facility));
    }
  }
  summary
}

pub fn skipped_summary(facilities: &[PortableFacility]) -> ImportSummary {
  ImportSummary {
    imported: 0,
    skipped: facilities.iter().map(skip_label).collect(),
  }
}

fn skip_label(facility: &PortableFacility) -> String {
  facility
    .name
    .clone()
    .unwrap_or_else(|| format!("#{}", facility.facility_id))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{self, model::FacilityIntel, repo::sde};

  const STATION_ID: i64 = 60_003_760;
  const STRUCTURE_ID: i64 = 1_021_000_000_001;

  async fn seed_rig_types(db: &Database) {
    sde::upsert_item_category(
      db,
      &store::model::ItemCategory {
        id: 66,
        icon_id: None,
        name: "Structure Modifier".to_owned(),
        published: true,
      },
    )
    .await
    .unwrap();
    sde::upsert_item_group(
      db,
      &store::model::ItemGroup {
        category_id: 66,
        icon_id: None,
        id: 1,
        name: "Rig".to_owned(),
        published: true,
      },
    )
    .await
    .unwrap();
    for (id, name) in [(37_180, "Standup M-Set ME I"), (43_704, "Standup XL-Set TE I")] {
      sde::upsert_item_type(
        db,
        &store::model::ItemType {
          capacity: None,
          description: Some("A rig.".to_owned()),
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
        },
      )
      .await
      .unwrap();
    }
  }

  fn portable(facility_id: i64, name: Option<&str>, solar_system_id: Option<i64>) -> PortableFacility {
    PortableFacility {
      facility_id,
      name: name.map(str::to_owned),
      rigs: [Some(37_180), None, Some(43_704)],
      solar_system_id,
      type_id: Some(35_834),
    }
  }

  async fn intel_rows(db: &Database) -> Vec<FacilityIntel> {
    industry::list_facility_intel(db).await.unwrap()
  }

  mod import_facilities {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_imports_every_entry_offline_with_no_esi_or_token() {
      let db = store::open_test().await.unwrap();
      seed_rig_types(&db).await;

      let summary = import_facilities(
        db.clone(),
        vec![
          portable(STATION_ID, Some("Jita IV - Moon 4"), Some(30_000_142)),
          portable(STRUCTURE_ID, Some("Allied Fortizar"), Some(30_002_187)),
        ],
      )
      .await;

      assert_eq!(summary.imported, 2);
      assert!(summary.skipped.is_empty());

      let rows = intel_rows(&db).await;
      assert_eq!(
        rows.iter().map(|row| row.facility_id).collect::<Vec<_>>(),
        [STATION_ID, STRUCTURE_ID]
      );
    }

    #[tokio::test]
    async fn it_persists_the_pack_snapshot_for_an_inaccessible_structure() {
      let db = store::open_test().await.unwrap();
      seed_rig_types(&db).await;

      import_facilities(
        db.clone(),
        vec![portable(STRUCTURE_ID, Some("Allied Fortizar"), Some(30_002_187))],
      )
      .await;

      let rows = intel_rows(&db).await;
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].name.as_deref(), Some("Allied Fortizar"));
      assert_eq!(rows[0].solar_system_id, Some(30_002_187));
      assert_eq!(rows[0].type_id, Some(35_834));
      assert_eq!(rows[0].rig_1_type_id, Some(37_180));
      assert_eq!(rows[0].rig_3_type_id, Some(43_704));
    }

    #[tokio::test]
    async fn it_overwrites_the_rigs_of_a_matching_facility() {
      let db = store::open_test().await.unwrap();
      seed_rig_types(&db).await;
      industry::upsert_facility_intel(&db, STATION_ID, None, Some(43_704), Some(37_180), None, None, None)
        .await
        .unwrap();

      let summary = import_facilities(db.clone(), vec![portable(STATION_ID, None, None)]).await;

      assert_eq!(summary.imported, 1);

      let rows = intel_rows(&db).await;
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].rig_1_type_id, Some(37_180));
      assert_eq!(rows[0].rig_2_type_id, None);
      assert_eq!(rows[0].rig_3_type_id, Some(43_704));
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
    fn it_uses_the_snapshot_name() {
      assert_eq!(
        skip_label(&portable(STRUCTURE_ID, Some("Allied Fortizar"), Some(30_000_142))),
        "Allied Fortizar"
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
