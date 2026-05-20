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

#[cfg(test)]
mod tests {
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  async fn insert_character(db: &DatabaseConnection, id: i64) {
    use sea_orm::ActiveValue::Set;
    crate::entities::character::Entity::insert(crate::entities::character::ActiveModel {
      access_token: Set(String::new()),
      charisma: Set(None),
      corp_id: Set(0),
      corp_name: Set(String::new()),
      id: Set(id),
      intelligence: Set(None),
      isk_balance: Set(None),
      location_docked: Set(None),
      location_name: Set(None),
      memory: Set(None),
      name: Set(format!("Character {id}")),
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

  fn make_plan(id: &str, character_id: i64, entries: Vec<SkillPlanEntry>) -> SkillPlan {
    SkillPlan {
      character_id,
      created_at: 0,
      entries,
      id: id.to_string(),
      implant_set: "standard".to_string(),
      name: "Test Plan".to_string(),
      remap_json: None,
      updated_at: 0,
    }
  }

  fn make_entry(plan_id: &str, id: &str, position: i32, skill_name: &str) -> SkillPlanEntry {
    SkillPlanEntry {
      auto: false,
      id: id.to_string(),
      note: None,
      plan_id: plan_id.to_string(),
      position,
      priority: "normal".to_string(),
      skill_name: skill_name.to_string(),
      to_level: 1,
    }
  }

  mod all_for_character {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_plans_exist() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);
      let result = repo.all_for_character(1).await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_plans_for_the_given_character_only() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      insert_character(&db, 2).await;
      let repo = Repo::new(&db);

      let plan1 = make_plan("plan-1", 1, vec![]);
      let plan2 = make_plan("plan-2", 2, vec![]);
      repo.create(&plan1).await.unwrap();
      repo.create(&plan2).await.unwrap();

      let result = repo.all_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].id, "plan-1");
    }
  }

  mod find {
    use super::*;

    #[tokio::test]
    async fn returns_none_for_missing_plan() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.find("nonexistent").await.unwrap();
      assert!(result.is_none());
    }

    #[tokio::test]
    async fn returns_plan_with_entries_loaded() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let entry = make_entry("plan-1", "entry-1", 1, "Spaceship Command");
      let plan = make_plan("plan-1", 1, vec![entry]);
      repo.create(&plan).await.unwrap();

      let found = repo.find("plan-1").await.unwrap().unwrap();
      assert_eq!(found.id, "plan-1");
      assert_eq!(found.entries.len(), 1);
      assert_eq!(found.entries[0].skill_name, "Spaceship Command");
    }
  }

  mod upsert_entries {
    use super::*;

    #[tokio::test]
    async fn stores_entries_in_position_order() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let plan = make_plan("plan-1", 1, vec![]);
      repo.create(&plan).await.unwrap();

      let entries = vec![
        make_entry("plan-1", "e-3", 3, "Gunnery"),
        make_entry("plan-1", "e-1", 1, "Spaceship Command"),
        make_entry("plan-1", "e-2", 2, "Navigation"),
      ];
      repo.upsert_entries("plan-1", &entries).await.unwrap();

      let found = repo.find("plan-1").await.unwrap().unwrap();
      assert_eq!(found.entries.len(), 3);
      assert_eq!(found.entries[0].skill_name, "Spaceship Command");
      assert_eq!(found.entries[1].skill_name, "Navigation");
      assert_eq!(found.entries[2].skill_name, "Gunnery");
    }

    #[tokio::test]
    async fn replaces_all_previous_entries() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let old_entry = make_entry("plan-1", "old-entry", 1, "Old Skill");
      let plan = make_plan("plan-1", 1, vec![old_entry]);
      repo.create(&plan).await.unwrap();

      let new_entry = make_entry("plan-1", "new-entry", 1, "New Skill");
      repo.upsert_entries("plan-1", &[new_entry]).await.unwrap();

      let found = repo.find("plan-1").await.unwrap().unwrap();
      assert_eq!(found.entries.len(), 1);
      assert_eq!(found.entries[0].skill_name, "New Skill");
    }

    #[tokio::test]
    async fn clears_all_entries_when_given_empty_slice() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let entry = make_entry("plan-1", "e-1", 1, "Spaceship Command");
      let plan = make_plan("plan-1", 1, vec![entry]);
      repo.create(&plan).await.unwrap();

      repo.upsert_entries("plan-1", &[]).await.unwrap();

      let found = repo.find("plan-1").await.unwrap().unwrap();
      assert!(found.entries.is_empty());
    }
  }

  mod update {
    use super::*;

    #[tokio::test]
    async fn update_changes_plan_name() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let plan = make_plan("plan-1", 1, vec![]);
      repo.create(&plan).await.unwrap();

      let mut updated = plan.clone();
      updated.name = "Renamed Plan".to_string();
      repo.update(&updated).await.unwrap();

      let found = repo.find("plan-1").await.unwrap().unwrap();
      assert_eq!(found.name, "Renamed Plan");
    }
  }

  mod delete {
    use super::*;

    #[tokio::test]
    async fn delete_removes_plan() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let plan = make_plan("plan-1", 1, vec![]);
      repo.create(&plan).await.unwrap();

      repo.delete("plan-1").await.unwrap();

      let found = repo.find("plan-1").await.unwrap();
      assert!(found.is_none());
    }
  }
}
