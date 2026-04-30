//! Repository for skill plan persistence.

use pod_model::{SkillPlan, SkillPlanEntry};
use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryFilter, QueryOrder};

use crate::{
  Error,
  entities::{
    skill_plan::{ActiveModel as PlanActive, Column as PlanColumn, Entity as PlanEntity},
    skill_plan_entry::{ActiveModel as EntryActive, Column as EntryColumn, Entity as EntryEntity},
  },
};

/// Repository for skill plan CRUD operations.
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

  /// Returns all skill plans for the given character, each with their entries loaded.
  pub async fn all_for_character(&self, character_id: i64) -> Result<Vec<SkillPlan>, Error> {
    let rows = PlanEntity::find()
      .filter(PlanColumn::CharacterId.eq(character_id))
      .order_by(PlanColumn::CreatedAt, Order::Asc)
      .all(self.db)
      .await?;
    let mut plans = Vec::with_capacity(rows.len());
    for row in rows {
      let entries = self.entries_for(&row.id).await?;
      plans.push(plan_from_row(row, entries));
    }
    Ok(plans)
  }

  /// Finds a skill plan by ID, loading its entries as well.
  pub async fn find(&self, id: &str) -> Result<Option<SkillPlan>, Error> {
    let Some(row) = PlanEntity::find_by_id(id.to_string()).one(self.db).await? else {
      return Ok(None);
    };
    let entries = self.entries_for(&row.id).await?;
    Ok(Some(plan_from_row(row, entries)))
  }

  /// Inserts a skill plan and all of its entries.
  pub async fn create(&self, plan: &SkillPlan) -> Result<(), Error> {
    PlanEntity::insert(plan_to_active(plan)).exec(self.db).await?;
    self.upsert_entries(&plan.id, &plan.entries).await
  }

  /// Updates the skill plan row and re-syncs its entries.
  pub async fn update(&self, plan: &SkillPlan) -> Result<(), Error> {
    PlanEntity::update(plan_to_active(plan)).exec(self.db).await?;
    self.upsert_entries(&plan.id, &plan.entries).await
  }

  /// Deletes a skill plan by ID; entries cascade via the FK constraint.
  pub async fn delete(&self, id: &str) -> Result<(), Error> {
    PlanEntity::delete_by_id(id.to_string()).exec(self.db).await?;
    Ok(())
  }

  /// Replaces all entries for the given plan with the provided slice, ordered by position.
  pub async fn upsert_entries(&self, plan_id: &str, entries: &[SkillPlanEntry]) -> Result<(), Error> {
    EntryEntity::delete_many()
      .filter(EntryColumn::PlanId.eq(plan_id))
      .exec(self.db)
      .await?;
    let mut sorted: Vec<&SkillPlanEntry> = entries.iter().collect();
    sorted.sort_by_key(|e| e.position);
    for entry in sorted {
      EntryEntity::insert(entry_to_active(entry)).exec(self.db).await?;
    }
    Ok(())
  }

  /// Loads all entry rows for the given plan ID, ordered by position.
  async fn entries_for(&self, plan_id: &str) -> Result<Vec<SkillPlanEntry>, Error> {
    let rows = EntryEntity::find()
      .filter(EntryColumn::PlanId.eq(plan_id))
      .order_by(EntryColumn::Position, Order::Asc)
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(entry_from_row).collect())
  }
}

fn plan_from_row(row: crate::entities::skill_plan::Model, entries: Vec<SkillPlanEntry>) -> SkillPlan {
  SkillPlan {
    character_id: row.character_id,
    created_at: row.created_at,
    entries,
    id: row.id,
    implant_set: row.implant_set,
    name: row.name,
    remap_json: row.remap_json,
    updated_at: row.updated_at,
  }
}

fn plan_to_active(plan: &SkillPlan) -> PlanActive {
  PlanActive {
    character_id: ActiveValue::Set(plan.character_id),
    created_at: ActiveValue::Set(plan.created_at),
    id: ActiveValue::Set(plan.id.clone()),
    implant_set: ActiveValue::Set(plan.implant_set.clone()),
    name: ActiveValue::Set(plan.name.clone()),
    remap_json: ActiveValue::Set(plan.remap_json.clone()),
    updated_at: ActiveValue::Set(plan.updated_at),
  }
}

fn entry_from_row(row: crate::entities::skill_plan_entry::Model) -> SkillPlanEntry {
  SkillPlanEntry {
    auto: row.auto != 0,
    id: row.id,
    note: row.note,
    plan_id: row.plan_id,
    position: row.position,
    priority: row.priority,
    skill_name: row.skill_name,
    to_level: row.to_level,
  }
}

fn entry_to_active(entry: &SkillPlanEntry) -> EntryActive {
  EntryActive {
    auto: ActiveValue::Set(i32::from(entry.auto)),
    id: ActiveValue::Set(entry.id.clone()),
    note: ActiveValue::Set(entry.note.clone()),
    plan_id: ActiveValue::Set(entry.plan_id.clone()),
    position: ActiveValue::Set(entry.position),
    priority: ActiveValue::Set(entry.priority.clone()),
    skill_name: ActiveValue::Set(entry.skill_name.clone()),
    to_level: ActiveValue::Set(entry.to_level),
  }
}
