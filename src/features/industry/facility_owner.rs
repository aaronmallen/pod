use crate::store::{
  Database,
  repo::{org, sde},
};

const NPC_STATION_ID_CEILING: i64 = 1_000_000_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FacilityOwner {
  pub alliance: Option<String>,
  pub corporation: String,
}

impl FacilityOwner {
  pub fn display(&self) -> String {
    match &self.alliance {
      Some(alliance) => format!("{} ({})", self.corporation, alliance),
      None => self.corporation.clone(),
    }
  }
}

pub async fn resolve_facility_owner(db: &Database, facility_id: i64) -> Option<FacilityOwner> {
  let corporation_id = if facility_id < NPC_STATION_ID_CEILING {
    sde::get_station(db, facility_id).await.ok().flatten()?.owner()?
  } else {
    sde::get_structure(db, facility_id).await.ok().flatten()?.owner_id()
  };

  let corporation = org::get_corporation(db, corporation_id).await.ok().flatten()?;
  let alliance = match corporation.alliance_id() {
    Some(alliance_id) => org::get_alliance(db, alliance_id)
      .await
      .ok()
      .flatten()
      .map(|alliance| alliance.name().clone()),
    None => None,
  };

  Some(FacilityOwner {
    alliance,
    corporation: corporation.name().to_owned(),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  async fn seed_geography(db: &Database) {
    sqlx::query("INSERT OR IGNORE INTO regions (id, name) VALUES (10000001, 'Region')")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query(
      "INSERT OR IGNORE INTO constellations (id, region_id, name, position_x, position_y, position_z) \
      VALUES (20000001, 10000001, 'Constellation', 0, 0, 0)",
    )
    .execute(db.writer())
    .await
    .unwrap();
    sqlx::query(
      "INSERT OR IGNORE INTO solar_systems \
        (id, constellation_id, name, position_x, position_y, position_z, security_status) \
      VALUES (30000001, 20000001, 'System', 0, 0, 0, 0.9)",
    )
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_item_type(db: &Database, type_id: i64) {
    sqlx::query("INSERT OR IGNORE INTO item_categories (id, name, published) VALUES (1, 'Category', 1)")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query("INSERT OR IGNORE INTO item_groups (id, category_id, name, published) VALUES (1, 1, 'Group', 1)")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query(
      "INSERT OR IGNORE INTO item_types (id, group_id, description, name, published) VALUES (?, 1, '', 'Type', 1)",
    )
    .bind(type_id)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_alliance(db: &Database, id: i64, name: &str) {
    sqlx::query(
      "INSERT OR IGNORE INTO alliances (id, creator_corporation_id, creator_id, date_founded, name, ticker) \
      VALUES (?, 1, 1, '2020-01-01T00:00:00Z', ?, 'ALY')",
    )
    .bind(id)
    .bind(name)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_corporation(db: &Database, id: i64, name: &str, alliance_id: Option<i64>) {
    sqlx::query(
      "INSERT OR IGNORE INTO corporations (id, alliance_id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
      VALUES (?, ?, 1, 1, 1, ?, 0.0, 'COR')",
    )
    .bind(id)
    .bind(alliance_id)
    .bind(name)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_station(db: &Database, id: i64, owner: Option<i64>) {
    seed_geography(db).await;
    seed_item_type(db, 54).await;
    sqlx::query(
      "INSERT OR IGNORE INTO stations \
        (id, system_id, type_id, name, max_dockable_ship_volume, office_rental_cost, \
        reprocessing_efficiency, reprocessing_stations_take, owner, position_x, position_y, position_z) \
      VALUES (?, 30000001, 54, 'Station', 0, 0, 0.5, 0.05, ?, 0, 0, 0)",
    )
    .bind(id)
    .bind(owner)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_structure(db: &Database, id: i64, owner_id: i64) {
    seed_geography(db).await;
    sqlx::query(
      "INSERT OR IGNORE INTO structures (id, solar_system_id, owner_id, name) VALUES (?, 30000001, ?, 'Citadel')",
    )
    .bind(id)
    .bind(owner_id)
    .execute(db.writer())
    .await
    .unwrap();
  }

  mod facility_owner {
    use super::*;

    mod display {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_shows_only_the_corporation_when_there_is_no_alliance() {
        let owner = FacilityOwner {
          alliance: None,
          corporation: "Owner Corp".to_owned(),
        };

        assert_eq!(owner.display(), "Owner Corp");
      }

      #[test]
      fn it_appends_the_alliance_in_parentheses() {
        let owner = FacilityOwner {
          alliance: Some("Big Alliance".to_owned()),
          corporation: "Owner Corp".to_owned(),
        };

        assert_eq!(owner.display(), "Owner Corp (Big Alliance)");
      }
    }
  }

  mod resolve_facility_owner {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_resolves_an_npc_station_to_its_owning_corporation() {
      let db = crate::store::open_test().await.unwrap();
      seed_corporation(&db, 1_000_035, "Caldari Navy", None).await;
      seed_station(&db, 60_003_760, Some(1_000_035)).await;

      let owner = resolve_facility_owner(&db, 60_003_760).await;

      assert_eq!(
        owner,
        Some(FacilityOwner {
          alliance: None,
          corporation: "Caldari Navy".to_owned(),
        })
      );
    }

    #[tokio::test]
    async fn it_resolves_a_player_structure_to_its_owning_corporation() {
      let db = crate::store::open_test().await.unwrap();
      seed_corporation(&db, 98_000_001, "Owner Corp", None).await;
      seed_structure(&db, 1_030_000_000_001, 98_000_001).await;

      let owner = resolve_facility_owner(&db, 1_030_000_000_001).await;

      assert_eq!(
        owner,
        Some(FacilityOwner {
          alliance: None,
          corporation: "Owner Corp".to_owned(),
        })
      );
    }

    #[tokio::test]
    async fn it_includes_the_alliance_for_a_player_structure_owner_in_an_alliance() {
      let db = crate::store::open_test().await.unwrap();
      seed_alliance(&db, 99_000_001, "Big Alliance").await;
      seed_corporation(&db, 98_000_002, "Aligned Corp", Some(99_000_001)).await;
      seed_structure(&db, 1_030_000_000_002, 98_000_002).await;

      let owner = resolve_facility_owner(&db, 1_030_000_000_002).await;

      assert_eq!(
        owner,
        Some(FacilityOwner {
          alliance: Some("Big Alliance".to_owned()),
          corporation: "Aligned Corp".to_owned(),
        })
      );
    }

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_facility() {
      let db = crate::store::open_test().await.unwrap();

      assert_eq!(resolve_facility_owner(&db, 60_003_760).await, None);
      assert_eq!(resolve_facility_owner(&db, 1_030_000_000_001).await, None);
    }

    #[tokio::test]
    async fn it_returns_none_for_a_station_without_a_recorded_owner() {
      let db = crate::store::open_test().await.unwrap();
      seed_station(&db, 60_003_761, None).await;

      assert_eq!(resolve_facility_owner(&db, 60_003_761).await, None);
    }
  }
}
