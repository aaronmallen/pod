//! Repository for character clone and implant persistence.

use pod_model::NeuralAttributes;
use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};

use crate::{
  Error,
  entities::{
    character_clone::{ActiveModel as CloneActive, Column as CloneColumn, Entity as CloneEntity, Model as CloneModel},
    character_clone_implant::{
      ActiveModel as ImplantActive, Column as ImplantColumn, Entity as ImplantEntity, Model as ImplantModel,
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

    let mut bonus = NeuralAttributes::default();
    for implant in implants {
      let Ok(map) = serde_json::from_str::<serde_json::Value>(&implant.attribute_bonus) else {
        continue;
      };
      let Some(obj) = map.as_object() else {
        continue;
      };
      for (attr, val) in obj {
        let amount = match val.as_i64() {
          Some(v) => v as i32,
          None => continue,
        };
        match attr.as_str() {
          "charisma" => bonus.charisma += amount,
          "intelligence" => bonus.intelligence += amount,
          "memory" => bonus.memory += amount,
          "perception" => bonus.perception += amount,
          "willpower" => bonus.willpower += amount,
          _ => {}
        }
      }
    }

    Ok(Some(bonus))
  }

  /// Returns all clone rows for the given character, each paired with its implants.
  pub async fn find_for_character(&self, character_id: i64) -> Result<Vec<(CloneModel, Vec<ImplantModel>)>, Error> {
    let clones = CloneEntity::find()
      .filter(CloneColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;

    let mut result = Vec::with_capacity(clones.len());
    for clone in clones {
      let implants = ImplantEntity::find()
        .filter(ImplantColumn::CloneId.eq(clone.id))
        .all(self.db)
        .await?;
      result.push((clone, implants));
    }
    Ok(result)
  }

  /// Upserts clones and their implants for the given character transactionally.
  ///
  /// Each `(clone_model, implants)` pair is written using ON CONFLICT DO UPDATE so
  /// stale data is always replaced with the latest ESI response.
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
