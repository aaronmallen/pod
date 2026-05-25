//! Repository for character clone and implant persistence.

use pod_model::NeuralAttributes;
use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};

use crate::{
  Error,
  entities::{
    character_clone::{
      self, ActiveModel as CloneActive, Column as CloneColumn, Entity as CloneEntity, Model as CloneModel,
    },
    character_clone_implant::{
      self, ActiveModel as ImplantActive, Column as ImplantColumn, Entity as ImplantEntity, Model as ImplantModel,
    },
  },
};

/// Minimal clone data written during the startup ESI sync.
pub struct StartupClone {
  /// ID of the owning character.
  pub character_id: i64,
  /// EVE clone identifier (0 for the active home clone).
  pub id: i64,
  /// ISO-8601 timestamp when the clone was installed, if known.
  pub installed_at: Option<String>,
  /// Whether this is the character's currently active clone.
  pub is_active: bool,
  /// Location structure or station ID.
  pub location_id: i64,
  /// Optional user-assigned name (jump clones only).
  pub name: Option<String>,
  /// ISO-8601 timestamp when this record was synced.
  pub synced_at: String,
}

/// Minimal implant data written during the startup ESI sync.
pub struct StartupImplant {
  /// FK to the owning clone.
  pub clone_id: i64,
  /// EVE type ID of the implant.
  pub type_id: i32,
  /// Implant slot number (1–10).
  pub slot: i32,
  /// Resolved display name.
  pub name: String,
  /// JSON object mapping neural attribute names to bonus amounts.
  pub attribute_bonus: String,
}

/// Repository for character clone and implant CRUD operations.
pub struct Repo<'a> {
  db: &'a DatabaseConnection,
}

impl<'a> Repo<'a> {
  /// Creates a new `Repo` bound to the given database connection.
  pub fn new(db: &'a DatabaseConnection) -> Self {
    Self {
      db,
    }
  }

  /// Returns the summed implant attribute bonuses from the active clone for
  /// the given character.
  ///
  /// Finds the clone with `is_active = true`, collects all of its implants,
  /// parses each `attribute_bonus` JSON string (mapping attribute name →
  /// bonus amount), and sums the values into a single `NeuralAttributes`.
  ///
  /// Returns `Ok(None)` when no active clone record exists (data not yet
  /// synced from ESI). Returns `Ok(Some(attrs))` when the clone record is
  /// present — even if all attribute bonuses are zero (the character simply
  /// has no neural attribute implants).
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn active_clone_implant_bonus(&self, character_id: i64) -> Result<Option<NeuralAttributes>, Error> {
    let active_clone = CloneEntity::find()
      .filter(CloneColumn::CharacterId.eq(character_id))
      .filter(CloneColumn::IsActive.eq(true))
      .one(self.db)
      .await?;

    let Some(clone) = active_clone else {
      return Ok(None);
    };

    let implants = ImplantEntity::find()
      .filter(ImplantColumn::CloneId.eq(clone.id))
      .all(self.db)
      .await?;

    let bonus = implants.into_iter().fold(NeuralAttributes::default(), |acc, implant| {
      apply_implant_bonus(acc, &implant.attribute_bonus)
    });

    Ok(Some(bonus))
  }

  /// Returns all clone rows for the given character, each paired with its implants.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn find_for_character(&self, character_id: i64) -> Result<Vec<(CloneModel, Vec<ImplantModel>)>, Error> {
    let rows = character_clone::Entity::load()
      .with(character_clone_implant::Entity)
      .filter(CloneColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    let result = rows
      .into_iter()
      .map(|mut row| {
        let raw_implants = std::mem::take(&mut row.implants);
        let implants: Vec<ImplantModel> = raw_implants.into_iter().map(Into::into).collect();
        let clone: CloneModel = row.into();
        (clone, implants)
      })
      .collect();
    Ok(result)
  }

  /// Upserts clones and their implants for the given character transactionally.
  ///
  /// Each `(clone_model, implants)` pair is written using ON CONFLICT DO UPDATE so
  /// stale data is always replaced with the latest ESI response.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn upsert_for_character(&self, clones: &[(CloneModel, Vec<ImplantModel>)]) -> Result<(), Error> {
    for (clone, implants) in clones {
      let active = CloneActive {
        character_id: ActiveValue::Set(clone.character_id),
        id: ActiveValue::Set(clone.id),
        installed_at: ActiveValue::Set(clone.installed_at.clone()),
        is_active: ActiveValue::Set(clone.is_active),
        location_id: ActiveValue::Set(clone.location_id),
        name: ActiveValue::Set(clone.name.clone()),
        region_name: ActiveValue::Set(clone.region_name.clone()),
        station_name: ActiveValue::Set(clone.station_name.clone()),
        synced_at: ActiveValue::Set(clone.synced_at.clone()),
        system_id: ActiveValue::Set(clone.system_id),
      };
      CloneEntity::insert(active)
        .on_conflict(
          OnConflict::column(CloneColumn::Id)
            .update_columns([
              CloneColumn::CharacterId,
              CloneColumn::InstalledAt,
              CloneColumn::IsActive,
              CloneColumn::LocationId,
              CloneColumn::Name,
              CloneColumn::RegionName,
              CloneColumn::StationName,
              CloneColumn::SyncedAt,
              CloneColumn::SystemId,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;

      for implant in implants {
        let active = ImplantActive {
          attribute_bonus: ActiveValue::Set(implant.attribute_bonus.clone()),
          clone_id: ActiveValue::Set(implant.clone_id),
          id: ActiveValue::NotSet,
          name: ActiveValue::Set(implant.name.clone()),
          slot: ActiveValue::Set(implant.slot),
          type_id: ActiveValue::Set(implant.type_id),
        };
        ImplantEntity::insert(active)
          .on_conflict(
            OnConflict::columns([ImplantColumn::CloneId, ImplantColumn::Slot])
              .update_columns([
                ImplantColumn::AttributeBonus,
                ImplantColumn::Name,
                ImplantColumn::TypeId,
              ])
              .to_owned(),
          )
          .exec(self.db)
          .await?;
      }
    }
    Ok(())
  }

  /// Upserts clones and their implants from the startup ESI sync.
  ///
  /// Accepts the minimal data produced during bootstrap (no location names or system IDs),
  /// so that `active_clone_implant_bonus` works correctly as soon as startup completes.
  #[tracing::instrument(level = "trace", skip(self, clones))]
  pub async fn upsert_startup_clones(&self, clones: &[(StartupClone, Vec<StartupImplant>)]) -> Result<(), Error> {
    for (clone, implants) in clones {
      let active = CloneActive {
        character_id: ActiveValue::Set(clone.character_id),
        id: ActiveValue::Set(clone.id),
        installed_at: ActiveValue::Set(clone.installed_at.clone()),
        is_active: ActiveValue::Set(clone.is_active),
        location_id: ActiveValue::Set(clone.location_id),
        name: ActiveValue::Set(clone.name.clone()),
        region_name: ActiveValue::Set(String::new()),
        station_name: ActiveValue::Set(String::new()),
        synced_at: ActiveValue::Set(clone.synced_at.clone()),
        system_id: ActiveValue::Set(0),
      };
      CloneEntity::insert(active)
        .on_conflict(
          OnConflict::column(CloneColumn::Id)
            .update_columns([
              CloneColumn::CharacterId,
              CloneColumn::InstalledAt,
              CloneColumn::IsActive,
              CloneColumn::LocationId,
              CloneColumn::Name,
              CloneColumn::SyncedAt,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;

      ImplantEntity::delete_many()
        .filter(ImplantColumn::CloneId.eq(clone.id))
        .exec(self.db)
        .await?;

      for implant in implants {
        let active = ImplantActive {
          attribute_bonus: ActiveValue::Set(implant.attribute_bonus.clone()),
          clone_id: ActiveValue::Set(implant.clone_id),
          id: ActiveValue::NotSet,
          name: ActiveValue::Set(implant.name.clone()),
          slot: ActiveValue::Set(implant.slot),
          type_id: ActiveValue::Set(implant.type_id),
        };
        ImplantEntity::insert(active).exec(self.db).await?;
      }
    }
    Ok(())
  }
}

fn apply_attr_amount(bonus: &mut NeuralAttributes, attr: &str, amount: i32) {
  match attr {
    "charisma" => bonus.charisma += amount,
    "intelligence" => bonus.intelligence += amount,
    "memory" => bonus.memory += amount,
    "perception" => bonus.perception += amount,
    "willpower" => bonus.willpower += amount,
    _ => {}
  }
}

fn apply_implant_bonus(mut bonus: NeuralAttributes, attribute_bonus: &str) -> NeuralAttributes {
  let Ok(map) = serde_json::from_str::<serde_json::Value>(attribute_bonus) else {
    return bonus;
  };
  let Some(obj) = map.as_object() else {
    return bonus;
  };
  for (attr, val) in obj {
    if let Some(amount) = val.as_i64().map(|v| v as i32) {
      apply_attr_amount(&mut bonus, attr.as_str(), amount);
    }
  }
  bonus
}

#[cfg(test)]
mod tests {
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  async fn insert_character(db: &DatabaseConnection, character_id: i64) {
    use sea_orm::ActiveValue::Set;
    crate::entities::character::Entity::insert(crate::entities::character::ActiveModel {
      access_token: Set(String::new()),
      charisma: Set(None),
      corp_id: Set(0),
      corp_name: Set(String::new()),
      granted_scopes: Set(None),
      id: Set(character_id),
      intelligence: Set(None),
      isk_balance: Set(None),
      location_docked: Set(None),
      location_name: Set(None),
      memory: Set(None),
      name: Set("Test Character".to_string()),
      perception: Set(None),
      portrait_tone: Set(0),
      refresh_token: Set(String::new()),
      sort_order: Set(0),
      token_expires_at: Set(0),
      willpower: Set(None),
    })
    .exec(db)
    .await
    .unwrap();
  }

  mod active_clone_implant_bonus {
    use super::*;

    #[tokio::test]
    async fn returns_none_when_no_active_clone_exists() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.active_clone_implant_bonus(1).await.unwrap();
      assert!(result.is_none());
    }

    #[tokio::test]
    async fn returns_zeroed_attrs_when_active_clone_has_no_implants() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let clone = CloneModel {
        character_id: 1,
        id: 1,
        installed_at: None,
        is_active: true,
        location_id: 0,
        name: None,
        region_name: String::new(),
        station_name: String::new(),
        synced_at: "2025-01-01".to_string(),
        system_id: 0,
      };
      repo.upsert_for_character(&[(clone, vec![])]).await.unwrap();

      let result = repo.active_clone_implant_bonus(1).await.unwrap();
      assert!(result.is_some());
      let attrs = result.unwrap();
      assert_eq!(attrs, NeuralAttributes::default());
    }

    #[tokio::test]
    async fn returns_none_when_only_inactive_clone_exists() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let clone = CloneModel {
        character_id: 1,
        id: 2,
        installed_at: None,
        is_active: false,
        location_id: 0,
        name: None,
        region_name: String::new(),
        station_name: String::new(),
        synced_at: "2025-01-01".to_string(),
        system_id: 0,
      };
      repo.upsert_for_character(&[(clone, vec![])]).await.unwrap();

      let result = repo.active_clone_implant_bonus(1).await.unwrap();
      assert!(result.is_none());
    }

    #[tokio::test]
    async fn sums_perception_and_willpower_from_implants() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let clone = CloneModel {
        character_id: 1,
        id: 3,
        installed_at: None,
        is_active: true,
        location_id: 0,
        name: None,
        region_name: String::new(),
        station_name: String::new(),
        synced_at: "2025-01-01".to_string(),
        system_id: 0,
      };
      let implant = ImplantModel {
        attribute_bonus: r#"{"perception":4,"willpower":4}"#.to_string(),
        clone_id: 3,
        id: 0,
        name: "Ocular Filter - Improved".to_string(),
        slot: 1,
        type_id: 10209,
      };
      repo.upsert_for_character(&[(clone, vec![implant])]).await.unwrap();

      let result = repo.active_clone_implant_bonus(1).await.unwrap().unwrap();
      assert_eq!(result.perception, 4);
      assert_eq!(result.willpower, 4);
      assert_eq!(result.charisma, 0);
      assert_eq!(result.intelligence, 0);
      assert_eq!(result.memory, 0);
    }

    #[tokio::test]
    async fn sums_all_five_attributes_across_multiple_implants() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let clone = CloneModel {
        character_id: 1,
        id: 4,
        installed_at: None,
        is_active: true,
        location_id: 0,
        name: None,
        region_name: String::new(),
        station_name: String::new(),
        synced_at: "2025-01-01".to_string(),
        system_id: 0,
      };
      let implants = vec![
        ImplantModel {
          attribute_bonus: r#"{"charisma":3}"#.to_string(),
          clone_id: 4,
          id: 0,
          name: "Implant A".to_string(),
          slot: 1,
          type_id: 1,
        },
        ImplantModel {
          attribute_bonus: r#"{"intelligence":4,"memory":4}"#.to_string(),
          clone_id: 4,
          id: 0,
          name: "Implant B".to_string(),
          slot: 2,
          type_id: 2,
        },
        ImplantModel {
          attribute_bonus: r#"{"perception":4,"willpower":4}"#.to_string(),
          clone_id: 4,
          id: 0,
          name: "Implant C".to_string(),
          slot: 3,
          type_id: 3,
        },
      ];
      repo.upsert_for_character(&[(clone, implants)]).await.unwrap();

      let result = repo.active_clone_implant_bonus(1).await.unwrap().unwrap();
      assert_eq!(result.charisma, 3);
      assert_eq!(result.intelligence, 4);
      assert_eq!(result.memory, 4);
      assert_eq!(result.perception, 4);
      assert_eq!(result.willpower, 4);
    }

    #[tokio::test]
    async fn skips_implant_with_malformed_json() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let clone = CloneModel {
        character_id: 1,
        id: 5,
        installed_at: None,
        is_active: true,
        location_id: 0,
        name: None,
        region_name: String::new(),
        station_name: String::new(),
        synced_at: "2025-01-01".to_string(),
        system_id: 0,
      };
      let implants = vec![
        ImplantModel {
          attribute_bonus: "not-valid-json".to_string(),
          clone_id: 5,
          id: 0,
          name: "Bad Implant".to_string(),
          slot: 1,
          type_id: 99,
        },
        ImplantModel {
          attribute_bonus: r#"{"intelligence":5}"#.to_string(),
          clone_id: 5,
          id: 0,
          name: "Good Implant".to_string(),
          slot: 2,
          type_id: 100,
        },
      ];
      repo.upsert_for_character(&[(clone, implants)]).await.unwrap();

      let result = repo.active_clone_implant_bonus(1).await.unwrap().unwrap();
      assert_eq!(result.intelligence, 5);
    }

    #[tokio::test]
    async fn skips_implant_where_bonus_is_not_object() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let clone = CloneModel {
        character_id: 1,
        id: 6,
        installed_at: None,
        is_active: true,
        location_id: 0,
        name: None,
        region_name: String::new(),
        station_name: String::new(),
        synced_at: "2025-01-01".to_string(),
        system_id: 0,
      };
      let implant = ImplantModel {
        attribute_bonus: "[1, 2, 3]".to_string(),
        clone_id: 6,
        id: 0,
        name: "Array Implant".to_string(),
        slot: 1,
        type_id: 99,
      };
      repo.upsert_for_character(&[(clone, vec![implant])]).await.unwrap();

      let result = repo.active_clone_implant_bonus(1).await.unwrap().unwrap();
      assert_eq!(result, NeuralAttributes::default());
    }

    #[tokio::test]
    async fn skips_unknown_attribute_keys() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let clone = CloneModel {
        character_id: 1,
        id: 7,
        installed_at: None,
        is_active: true,
        location_id: 0,
        name: None,
        region_name: String::new(),
        station_name: String::new(),
        synced_at: "2025-01-01".to_string(),
        system_id: 0,
      };
      let implant = ImplantModel {
        attribute_bonus: r#"{"strength":5,"agility":3}"#.to_string(),
        clone_id: 7,
        id: 0,
        name: "Unknown Attr Implant".to_string(),
        slot: 1,
        type_id: 99,
      };
      repo.upsert_for_character(&[(clone, vec![implant])]).await.unwrap();

      let result = repo.active_clone_implant_bonus(1).await.unwrap().unwrap();
      assert_eq!(result, NeuralAttributes::default());
    }
  }

  mod find_for_character {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_clones_exist() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.find_for_character(99).await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_clone_with_its_implants() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let clone = CloneModel {
        character_id: 1,
        id: 10,
        installed_at: None,
        is_active: true,
        location_id: 0,
        name: None,
        region_name: String::new(),
        station_name: String::new(),
        synced_at: "2025-01-01".to_string(),
        system_id: 0,
      };
      let implant = ImplantModel {
        attribute_bonus: r#"{"perception":4}"#.to_string(),
        clone_id: 10,
        id: 0,
        name: "Ocular Filter".to_string(),
        slot: 1,
        type_id: 10209,
      };
      repo.upsert_for_character(&[(clone, vec![implant])]).await.unwrap();

      let result = repo.find_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].0.id, 10);
      assert_eq!(result[0].1.len(), 1);
      assert_eq!(result[0].1[0].slot, 1);
    }
  }
}
