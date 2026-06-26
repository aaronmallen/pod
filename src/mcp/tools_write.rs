use serde_json::{Value, json};

use crate::{
  features::{
    self,
    wallet::budget::{self, MoveDest},
  },
  mcp::{
    args::{ArgSpec, require_i64, require_i64_array, require_str},
    tool::{McpTool, Permission, ToolError},
  },
  store::{
    Database,
    model::{BudgetEntryKind, BudgetOwner, BudgetScope, MatchMode, Rule, RuleCondition, RuleField, RuleOp},
    repo::{
      budget as budget_repo, industry as industry_repo,
      industry::{PlanSegment, PlanTree, PlanType},
      skills as skills_repo,
    },
  },
};

pub fn tools() -> Vec<McpTool> {
  vec![
    budget_assign_category_tool(),
    budget_move_money_tool(),
    budget_assign_entry_tool(),
    budget_set_rule_tool(),
    skill_plan_create_tool(),
    skill_plan_add_entry_tool(),
    skill_plan_remove_entry_tool(),
    skill_plan_reorder_tool(),
    skill_plan_replace_tool(),
    skill_plan_delete_tool(),
    planner_create_tool(),
    planner_replace_segments_tool(),
    planner_delete_tool(),
  ]
}

fn budget_assign_category_tool() -> McpTool {
  McpTool::new(
    "budget_assign_category",
    "Sets a budget category's assignment for a month (the global All scope).",
    Permission::LocalWrite,
    |db, args: Value| async move {
      let category_id = require_i64(&args, "category_id")?;
      let month = require_month(&args)?;
      let value = require_f64(&args, "value")?;
      let view = budget::load(&db, BudgetScope::All, &month).await;
      if view.category(category_id).is_none() {
        return Err(ToolError::InvalidArguments(format!(
          "no category with id {category_id}"
        )));
      }
      budget::persist_assignment(&db, category_id, &month, value).await;
      Ok(json!({ "assigned": value, "category_id": category_id, "month": month }))
    },
  )
  .with_args([
    ArgSpec::integer("category_id", "The budget category to assign to."),
    ArgSpec::string("month", "The budget month in YYYY-MM format."),
    ArgSpec::integer("value", "The amount to assign to the category for the month."),
  ])
}

fn budget_move_money_tool() -> McpTool {
  McpTool::new(
    "budget_move_money",
    "Moves assigned money between budget categories, or to Ready-to-Assign.",
    Permission::LocalWrite,
    |db, args: Value| async move {
      let month = require_month(&args)?;
      let from_id = require_i64(&args, "from_category_id")?;
      let amount = require_f64(&args, "amount")?;
      let view = budget::load(&db, BudgetScope::All, &month).await;
      if view.category(from_id).is_none() {
        return Err(ToolError::InvalidArguments(format!("no source category {from_id}")));
      }
      let to = match args.get("to_category_id").and_then(Value::as_i64) {
        Some(to_id) => {
          if view.category(to_id).is_none() {
            return Err(ToolError::InvalidArguments(format!("no destination category {to_id}")));
          }
          MoveDest::Category(to_id)
        }
        None => MoveDest::ReadyToAssign,
      };
      budget::move_money(&db, &view, from_id, to, amount).await;
      Ok(json!({ "amount": amount, "from_category_id": from_id, "month": month }))
    },
  )
  .with_args([
    ArgSpec::string("month", "The budget month in YYYY-MM format."),
    ArgSpec::integer("from_category_id", "The category to move money out of."),
    ArgSpec::integer("amount", "The amount of money to move."),
    ArgSpec::optional_integer(
      "to_category_id",
      0,
      "The destination category; omit to move the money to Ready-to-Assign.",
    ),
  ])
}

fn budget_assign_entry_tool() -> McpTool {
  McpTool::new(
    "budget_assign_entry",
    "Pins a single wallet entry to a budget category.",
    Permission::LocalWrite,
    |db, args: Value| async move {
      let owner = require_owner(&args)?;
      let entry_kind = require_entry_kind(&args)?;
      let entry_id = require_i64(&args, "entry_id")?;
      let category_id = require_i64(&args, "category_id")?;
      let assignment =
        features::wallet::budget_engine::assign_entry(&db, BudgetScope::All, owner, entry_kind, entry_id, category_id)
          .await
          .map_err(internal)?;
      match assignment {
        Some(row) => Ok(json!({ "category_id": row.category_id, "entry_id": row.entry_id, "id": row.id })),
        None => Err(ToolError::InvalidArguments(
          "that owner does not hold the named entry".to_owned(),
        )),
      }
    },
  )
  .with_args([
    ArgSpec::string("owner_kind", "The owner type: character or corporation."),
    ArgSpec::integer("owner_id", "The id of the owning character or corporation."),
    ArgSpec::string("entry_kind", "The entry type: journal or market."),
    ArgSpec::integer("entry_id", "The id of the wallet entry to pin."),
    ArgSpec::integer("category_id", "The budget category to pin the entry to."),
  ])
}

fn budget_set_rule_tool() -> McpTool {
  McpTool::new(
    "budget_set_rule",
    "Creates or updates an automation rule that files matching entries into a category. Optional inputs: match_mode \
      (all|any, default all), conditions (array of {field, op, value, value2?}), enabled (boolean, default true).",
    Permission::LocalWrite,
    |db, args: Value| async move {
      let category_id = require_i64(&args, "category_id")?;
      let name = require_str(&args, "name")?.to_owned();
      let match_mode = match args.get("match_mode").and_then(Value::as_str) {
        Some("any") => MatchMode::Any,
        _ => MatchMode::All,
      };
      let enabled = args.get("enabled").and_then(Value::as_bool).unwrap_or(true);
      let conditions = parse_conditions(&args)?;

      let rule_id = match args.get("rule_id").and_then(Value::as_i64) {
        Some(id) => {
          let rule = Rule {
            category_id,
            conditions: Vec::new(),
            enabled,
            id,
            match_mode,
            name,
          };
          budget_repo::update_rule(&db, &rule).await.map_err(internal)?;
          id
        }
        None => {
          let position = budget_repo::list_rules(&db, BudgetScope::All)
            .await
            .map_err(internal)?
            .len() as i64;
          let created = budget_repo::create_rule(
            &db,
            &budget_repo::NewRule {
              category_id,
              enabled,
              match_mode,
              name,
              position,
              scope: BudgetScope::All,
            },
          )
          .await
          .map_err(internal)?;
          created.id
        }
      };

      budget_repo::replace_rule_conditions(&db, rule_id, &conditions)
        .await
        .map_err(internal)?;
      Ok(json!({ "condition_count": conditions.len(), "rule_id": rule_id }))
    },
  )
  .with_args([
    ArgSpec::integer("category_id", "The category matching entries should be filed into."),
    ArgSpec::string("name", "A human-readable name for the rule."),
    ArgSpec::optional_integer("rule_id", 0, "The rule to update; omit to create a new rule."),
  ])
}

fn skill_plan_create_tool() -> McpTool {
  McpTool::new(
    "skill_plan_create",
    "Creates an empty skill plan for a character.",
    Permission::LocalWrite,
    |db, args: Value| async move {
      let character_id = require_i64(&args, "character_id")?;
      let name = require_str(&args, "name")?;
      let plan = skills_repo::create(&db, character_id, name).await.map_err(internal)?;
      Ok(json!({ "character_id": plan.character_id(), "name": plan.name(), "plan_id": plan.id() }))
    },
  )
  .with_args([
    ArgSpec::integer("character_id", "The character that owns the plan."),
    ArgSpec::string("name", "A human-readable name for the plan."),
  ])
}

fn skill_plan_add_entry_tool() -> McpTool {
  McpTool::new(
    "skill_plan_add_entry",
    "Appends a skill-to-level to a plan.",
    Permission::LocalWrite,
    |db, args: Value| async move {
      let plan_id = require_i64(&args, "plan_id")?;
      let skill_id = require_i64(&args, "skill_id")?;
      let to_level = require_level(&args)?;
      require_plan(&db, plan_id).await?;
      let entry = skills_repo::insert_entry(&db, plan_id, skill_id, to_level)
        .await
        .map_err(internal)?;
      Ok(json!({ "entry_id": entry.id(), "skill_id": entry.skill_id(), "to_level": entry.to_level() }))
    },
  )
  .with_args([
    ArgSpec::integer("plan_id", "The plan to append the entry to."),
    ArgSpec::integer("skill_id", "The skill to train."),
    ArgSpec::integer("to_level", "The target skill level, 1 through 5."),
  ])
}

fn skill_plan_remove_entry_tool() -> McpTool {
  McpTool::new(
    "skill_plan_remove_entry",
    "Removes one entry from a plan and densifies the order.",
    Permission::LocalWrite,
    |db, args: Value| async move {
      let entry_id = require_i64(&args, "entry_id")?;
      skills_repo::remove_entry(&db, entry_id).await.map_err(internal)?;
      Ok(json!({ "entry_id": entry_id, "removed": true }))
    },
  )
  .with_args([ArgSpec::integer("entry_id", "The plan entry to remove.")])
}

fn skill_plan_reorder_tool() -> McpTool {
  McpTool::new(
    "skill_plan_reorder",
    "Sets a plan's entry order.",
    Permission::LocalWrite,
    |db, args: Value| async move {
      let ids = require_i64_array(&args, "ordered_entry_ids")?;
      skills_repo::reorder_entries(&db, &ids).await.map_err(internal)?;
      Ok(json!({ "entry_count": ids.len() }))
    },
  )
  .with_args([ArgSpec::integer_array(
    "ordered_entry_ids",
    "The plan entry ids in the desired order.",
  )])
}

fn skill_plan_replace_tool() -> McpTool {
  McpTool::new(
    "skill_plan_replace",
    "Replaces every entry of a plan in one shot. Required `entries` is an array of {skill_id, to_level, priority?, \
      note?, is_auto?}.",
    Permission::LocalWrite,
    |db, args: Value| async move {
      let plan_id = require_i64(&args, "plan_id")?;
      require_plan(&db, plan_id).await?;
      let entries = parse_plan_entries(&args)?;
      let rows: Vec<(i64, i64, &str, &str, i64)> = entries
        .iter()
        .map(|e| (e.skill_id, e.to_level, e.priority.as_str(), e.note.as_str(), e.is_auto))
        .collect();
      skills_repo::replace_entries(&db, plan_id, &rows)
        .await
        .map_err(internal)?;
      Ok(json!({ "entry_count": rows.len(), "plan_id": plan_id }))
    },
  )
  .with_args([ArgSpec::integer("plan_id", "The plan whose entries are replaced.")])
}

fn skill_plan_delete_tool() -> McpTool {
  McpTool::new(
    "skill_plan_delete",
    "Deletes a skill plan and all of its entries.",
    Permission::LocalWrite,
    |db, args: Value| async move {
      let plan_id = require_i64(&args, "plan_id")?;
      skills_repo::delete(&db, plan_id).await.map_err(internal)?;
      Ok(json!({ "deleted": true, "plan_id": plan_id }))
    },
  )
  .with_args([ArgSpec::integer("plan_id", "The plan to delete.")])
}

fn planner_create_tool() -> McpTool {
  McpTool::new(
    "planner_create",
    "Creates a saved industry plan. Required `types` is an array of {type_id, me?, te?, built?, use_stock?, \
      facility_system?, facility_structure?}.",
    Permission::LocalWrite,
    |db, args: Value| async move {
      let name = require_str(&args, "name")?;
      let tree = parse_plan_tree(&args)?;
      let plan = industry_repo::create_plan(&db, name, &tree).await.map_err(internal)?;
      Ok(json!({ "name": plan.name(), "plan_id": plan.id(), "type_count": tree.types.len() }))
    },
  )
  .with_args([
    ArgSpec::string("name", "A human-readable name for the plan."),
    ArgSpec::integer("product_type_id", "The type id of the plan's final product."),
    ArgSpec::integer("runs", "The number of production runs of the product."),
    ArgSpec::optional_integer(
      "root_facility_system",
      0,
      "The solar system id of the root facility; omit to leave unset.",
    ),
  ])
}

fn planner_replace_segments_tool() -> McpTool {
  McpTool::new(
    "planner_replace_segments",
    "Replaces a plan's per-type build segments. Required `segments` is an array of {type_id, runs, segment_index, \
      pilot_id?, clone_id?}.",
    Permission::LocalWrite,
    |db, args: Value| async move {
      let plan_id = require_i64(&args, "plan_id")?;
      if industry_repo::load_plan(&db, plan_id)
        .await
        .map_err(internal)?
        .is_none()
      {
        return Err(ToolError::InvalidArguments(format!("no plan with id {plan_id}")));
      }
      let segments = parse_segments(&args)?;
      industry_repo::replace_plan_segments(&db, plan_id, &segments)
        .await
        .map_err(internal)?;
      Ok(json!({ "plan_id": plan_id, "segment_count": segments.len() }))
    },
  )
  .with_args([ArgSpec::integer("plan_id", "The plan whose segments are replaced.")])
}

fn planner_delete_tool() -> McpTool {
  McpTool::new(
    "planner_delete",
    "Deletes a saved industry plan and its types and segments.",
    Permission::LocalWrite,
    |db, args: Value| async move {
      let plan_id = require_i64(&args, "plan_id")?;
      industry_repo::delete_plan(&db, plan_id).await.map_err(internal)?;
      Ok(json!({ "deleted": true, "plan_id": plan_id }))
    },
  )
  .with_args([ArgSpec::integer("plan_id", "The plan to delete.")])
}

struct PlanEntryInput {
  is_auto: i64,
  note: String,
  priority: String,
  skill_id: i64,
  to_level: i64,
}

fn internal(error: impl std::fmt::Display) -> ToolError {
  ToolError::Internal(error.to_string())
}

fn parse_conditions(args: &Value) -> Result<Vec<RuleCondition>, ToolError> {
  let Some(items) = args.get("conditions").and_then(Value::as_array) else {
    return Ok(Vec::new());
  };
  items
    .iter()
    .map(|item| {
      let field = item
        .get("field")
        .and_then(Value::as_str)
        .map(RuleField::from_key)
        .ok_or_else(|| ToolError::InvalidArguments("each condition needs a `field`".to_owned()))?;
      let op = item
        .get("op")
        .and_then(Value::as_str)
        .map(RuleOp::from_key)
        .ok_or_else(|| ToolError::InvalidArguments("each condition needs an `op`".to_owned()))?;
      let value = item
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArguments("each condition needs a `value`".to_owned()))?
        .to_owned();
      let value2 = item.get("value2").and_then(Value::as_str).map(str::to_owned);
      Ok(RuleCondition {
        field,
        op,
        value,
        value2,
      })
    })
    .collect()
}

fn parse_plan_entries(args: &Value) -> Result<Vec<PlanEntryInput>, ToolError> {
  let items = args
    .get("entries")
    .and_then(Value::as_array)
    .ok_or_else(|| ToolError::InvalidArguments("`entries` must be an array".to_owned()))?;
  items
    .iter()
    .map(|item| {
      let skill_id = item
        .get("skill_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| ToolError::InvalidArguments("each entry needs a `skill_id`".to_owned()))?;
      let to_level = item
        .get("to_level")
        .and_then(Value::as_i64)
        .filter(|level| (1..=5).contains(level))
        .ok_or_else(|| ToolError::InvalidArguments("each entry needs a `to_level` of 1-5".to_owned()))?;
      Ok(PlanEntryInput {
        is_auto: i64::from(item.get("is_auto").and_then(Value::as_bool).unwrap_or(false)),
        note: item.get("note").and_then(Value::as_str).unwrap_or("").to_owned(),
        priority: item
          .get("priority")
          .and_then(Value::as_str)
          .unwrap_or("normal")
          .to_owned(),
        skill_id,
        to_level,
      })
    })
    .collect()
}

fn parse_plan_tree(args: &Value) -> Result<PlanTree, ToolError> {
  let product_type_id = require_i64(args, "product_type_id")?;
  let runs = require_i64(args, "runs")?;
  let root_facility_system = args.get("root_facility_system").and_then(Value::as_i64);
  let items = args
    .get("types")
    .and_then(Value::as_array)
    .ok_or_else(|| ToolError::InvalidArguments("`types` must be an array".to_owned()))?;
  let types = items
    .iter()
    .map(|item| {
      let type_id = item
        .get("type_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| ToolError::InvalidArguments("each type needs a `type_id`".to_owned()))?;
      Ok(PlanType {
        built: item.get("built").and_then(Value::as_bool).unwrap_or(false),
        facility_structure: item.get("facility_structure").and_then(Value::as_i64),
        facility_system: item.get("facility_system").and_then(Value::as_i64),
        me: item.get("me").and_then(Value::as_i64).unwrap_or(0),
        te: item.get("te").and_then(Value::as_i64).unwrap_or(0),
        type_id,
        use_stock: item.get("use_stock").and_then(Value::as_bool).unwrap_or(false),
      })
    })
    .collect::<Result<Vec<_>, ToolError>>()?;
  Ok(PlanTree {
    product_type_id,
    root_facility_system,
    runs,
    types,
  })
}

fn parse_segments(args: &Value) -> Result<Vec<PlanSegment>, ToolError> {
  let items = args
    .get("segments")
    .and_then(Value::as_array)
    .ok_or_else(|| ToolError::InvalidArguments("`segments` must be an array".to_owned()))?;
  items
    .iter()
    .map(|item| {
      let type_id = item
        .get("type_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| ToolError::InvalidArguments("each segment needs a `type_id`".to_owned()))?;
      let runs = item
        .get("runs")
        .and_then(Value::as_i64)
        .ok_or_else(|| ToolError::InvalidArguments("each segment needs `runs`".to_owned()))?;
      let segment_index = item
        .get("segment_index")
        .and_then(Value::as_i64)
        .ok_or_else(|| ToolError::InvalidArguments("each segment needs a `segment_index`".to_owned()))?;
      Ok(PlanSegment {
        clone_id: item.get("clone_id").and_then(Value::as_i64),
        pilot_id: item.get("pilot_id").and_then(Value::as_i64),
        runs,
        segment_index,
        type_id,
      })
    })
    .collect()
}

fn require_entry_kind(args: &Value) -> Result<BudgetEntryKind, ToolError> {
  match require_str(args, "entry_kind")? {
    "journal" => Ok(BudgetEntryKind::Journal),
    "market" => Ok(BudgetEntryKind::Market),
    other => Err(ToolError::InvalidArguments(format!(
      "`entry_kind` must be journal or market, got `{other}`"
    ))),
  }
}

fn require_f64(args: &Value, key: &str) -> Result<f64, ToolError> {
  args
    .get(key)
    .and_then(Value::as_f64)
    .ok_or_else(|| ToolError::InvalidArguments(format!("`{key}` is required and must be a number")))
}

fn require_level(args: &Value) -> Result<i64, ToolError> {
  require_i64(args, "to_level").and_then(|level| {
    if (1..=5).contains(&level) {
      Ok(level)
    } else {
      Err(ToolError::InvalidArguments("`to_level` must be 1-5".to_owned()))
    }
  })
}

fn require_month(args: &Value) -> Result<String, ToolError> {
  let month = require_str(args, "month")?;
  let valid = month.len() == 7
    && month.as_bytes()[4] == b'-'
    && month[..4].bytes().all(|b| b.is_ascii_digit())
    && month[5..].bytes().all(|b| b.is_ascii_digit());
  if valid {
    Ok(month.to_owned())
  } else {
    Err(ToolError::InvalidArguments("`month` must be YYYY-MM".to_owned()))
  }
}

fn require_owner(args: &Value) -> Result<BudgetOwner, ToolError> {
  let owner_kind = require_str(args, "owner_kind")?;
  let owner_id = require_i64(args, "owner_id")?;
  BudgetOwner::from_key(owner_kind, owner_id)
    .ok_or_else(|| ToolError::InvalidArguments("`owner_kind` must be character or corporation".to_owned()))
}

async fn require_plan(db: &Database, plan_id: i64) -> Result<(), ToolError> {
  if skills_repo::get(db, plan_id).await.map_err(internal)?.is_none() {
    return Err(ToolError::InvalidArguments(format!("no skill plan with id {plan_id}")));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{config::McpPerms, mcp::tool::Registry, store::repo::character};

  async fn database() -> Database {
    crate::store::open_test().await.expect("open a migrated test database")
  }

  async fn seed_character(db: &Database, id: i64) {
    use crate::store::model::{Alliance, Bloodline, Character, Corporation, Gender, Race};
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .expect("seed character");
  }

  async fn seed_category(db: &Database, name: &str) -> i64 {
    let group = budget_repo::create_group(
      db,
      &budget_repo::NewGroup {
        name: "Ops".to_owned(),
        position: 0,
        scope: BudgetScope::All,
      },
    )
    .await
    .unwrap();
    budget_repo::create_category(
      db,
      &budget_repo::NewCategory {
        group_id: group.id(),
        name: name.to_owned(),
        note: None,
        position: 0,
        tone: None,
      },
    )
    .await
    .unwrap()
    .id()
  }

  async fn seed_journal_entry(db: &Database, id: i64, character_id: i64) {
    sqlx::query(
      "INSERT INTO character_wallet_journal (id, character_id, date, description, ref_type, amount, balance) \
        VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(character_id)
    .bind("2026-01-01")
    .bind("Test")
    .bind("test")
    .bind(100.0)
    .bind(100.0)
    .execute(db.writer())
    .await
    .expect("seed journal entry");
  }

  fn deny_local_write() -> McpPerms {
    let mut perms = McpPerms::default();
    perms.set_local_write(false);
    perms
  }

  fn registry() -> Registry {
    let mut registry = Registry::default();
    for tool in tools() {
      registry.register(tool);
    }
    registry
  }

  mod gate {
    use super::*;

    #[tokio::test]
    async fn every_write_tool_is_denied_when_local_write_is_off() {
      let db = database().await;
      let registry = registry();

      for tool in registry.tools() {
        let outcome = registry
          .dispatch(tool.name(), &deny_local_write(), db.clone(), Value::Null)
          .await;

        assert!(
          matches!(outcome, Err(ToolError::PermissionDenied("local_write"))),
          "{} must be gated by local_write",
          tool.name()
        );
      }
    }

    #[tokio::test]
    async fn every_write_tool_requires_local_write_permission() {
      for tool in tools() {
        assert!(
          matches!(tool.permission(), Permission::LocalWrite),
          "{} is a local-write tool",
          tool.name()
        );
      }
    }
  }

  mod skill_plan_create {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_a_plan_and_returns_its_id() {
      let db = database().await;
      seed_character(&db, 42).await;
      let registry = registry();

      let value = registry
        .dispatch(
          "skill_plan_create",
          &McpPerms::default(),
          db.clone(),
          json!({ "character_id": 42, "name": "Caps" }),
        )
        .await
        .unwrap();

      let plan_id = value.get("plan_id").and_then(Value::as_i64).expect("plan id");
      let plan = skills_repo::get(&db, plan_id).await.unwrap().expect("plan persisted");
      assert_eq!(plan.name(), "Caps");
    }
  }

  mod skill_plan_add_entry {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_rejects_a_level_outside_one_to_five() {
      let db = database().await;
      seed_character(&db, 42).await;
      let plan = skills_repo::create(&db, 42, "Plan").await.unwrap();
      let registry = registry();

      let outcome = registry
        .dispatch(
          "skill_plan_add_entry",
          &McpPerms::default(),
          db,
          json!({ "plan_id": plan.id(), "skill_id": 3300, "to_level": 6 }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_appends_an_entry_to_an_existing_plan() {
      let db = database().await;
      seed_character(&db, 42).await;
      let plan = skills_repo::create(&db, 42, "Plan").await.unwrap();
      let registry = registry();

      let value = registry
        .dispatch(
          "skill_plan_add_entry",
          &McpPerms::default(),
          db.clone(),
          json!({ "plan_id": plan.id(), "skill_id": 3300, "to_level": 5 }),
        )
        .await
        .unwrap();

      assert_eq!(value.get("to_level").and_then(Value::as_i64), Some(5));
      assert_eq!(skills_repo::entries(&db, plan.id()).await.unwrap().len(), 1);
    }
  }

  mod skill_plan_replace {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_replaces_every_entry_of_a_plan() {
      let db = database().await;
      seed_character(&db, 42).await;
      let plan = skills_repo::create(&db, 42, "Plan").await.unwrap();
      skills_repo::insert_entry(&db, plan.id(), 3300, 1).await.unwrap();
      let registry = registry();

      let value = registry
        .dispatch(
          "skill_plan_replace",
          &McpPerms::default(),
          db.clone(),
          json!({
            "plan_id": plan.id(),
            "entries": [
              { "skill_id": 3301, "to_level": 4, "priority": "high", "note": "core" },
              { "skill_id": 3302, "to_level": 2 },
            ],
          }),
        )
        .await
        .unwrap();

      assert_eq!(value.get("entry_count").and_then(Value::as_i64), Some(2));
      let entries = skills_repo::entries(&db, plan.id()).await.unwrap();
      assert_eq!(entries.len(), 2);
      assert_eq!(entries[0].skill_id(), 3301);
    }

    #[tokio::test]
    async fn it_rejects_a_missing_plan() {
      let db = database().await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "skill_plan_replace",
          &McpPerms::default(),
          db,
          json!({ "plan_id": 9999, "entries": [] }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod planner_create {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_a_plan_with_its_types() {
      let db = database().await;
      let registry = registry();

      let value = registry
        .dispatch(
          "planner_create",
          &McpPerms::default(),
          db.clone(),
          json!({
            "name": "Rifter run",
            "product_type_id": 587,
            "runs": 10,
            "types": [{ "type_id": 587, "me": 10, "te": 20 }],
          }),
        )
        .await
        .unwrap();

      let plan_id = value.get("plan_id").and_then(Value::as_i64).expect("plan id");
      let tree = industry_repo::load_plan(&db, plan_id).await.unwrap().expect("plan");
      assert_eq!(tree.types.len(), 1);
      assert_eq!(tree.product_type_id, 587);
    }
  }

  mod budget_set_rule {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_a_rule_with_conditions() {
      let db = database().await;
      let group = budget_repo::create_group(
        &db,
        &budget_repo::NewGroup {
          name: "Ops".to_owned(),
          position: 0,
          scope: BudgetScope::All,
        },
      )
      .await
      .unwrap();
      let category = budget_repo::create_category(
        &db,
        &budget_repo::NewCategory {
          group_id: group.id(),
          name: "Fuel".to_owned(),
          note: None,
          position: 0,
          tone: None,
        },
      )
      .await
      .unwrap();
      let registry = registry();

      let value = registry
        .dispatch(
          "budget_set_rule",
          &McpPerms::default(),
          db.clone(),
          json!({
            "category_id": category.id(),
            "name": "Fuel buys",
            "conditions": [{ "field": "text", "op": "contains", "value": "fuel" }],
          }),
        )
        .await
        .unwrap();

      let rule_id = value.get("rule_id").and_then(Value::as_i64).expect("rule id");
      let rules = budget_repo::list_rules(&db, BudgetScope::All).await.unwrap();
      assert_eq!(rules.iter().filter(|r| r.id() == rule_id).count(), 1);
    }
  }

  mod budget_move_money {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_moves_assigned_money_between_categories() {
      let db = database().await;
      let from_id = seed_category(&db, "Fuel").await;
      let to_id = seed_category(&db, "Ammo").await;
      budget::persist_assignment(&db, from_id, "2026-01", 1000.0).await;
      let registry = registry();

      let value = registry
        .dispatch(
          "budget_move_money",
          &McpPerms::default(),
          db.clone(),
          json!({ "month": "2026-01", "from_category_id": from_id, "to_category_id": to_id, "amount": 300.0 }),
        )
        .await
        .unwrap();

      assert_eq!(value.get("amount").and_then(Value::as_f64), Some(300.0));
      let view = budget::load(&db, BudgetScope::All, "2026-01").await;
      assert_eq!(view.category(from_id).map(|c| c.assigned), Some(700.0));
      assert_eq!(view.category(to_id).map(|c| c.assigned), Some(300.0));
    }

    #[tokio::test]
    async fn it_moves_money_to_ready_to_assign_when_no_destination_is_given() {
      let db = database().await;
      let from_id = seed_category(&db, "Fuel").await;
      budget::persist_assignment(&db, from_id, "2026-01", 1000.0).await;
      let registry = registry();

      registry
        .dispatch(
          "budget_move_money",
          &McpPerms::default(),
          db.clone(),
          json!({ "month": "2026-01", "from_category_id": from_id, "amount": 250.0 }),
        )
        .await
        .unwrap();

      let view = budget::load(&db, BudgetScope::All, "2026-01").await;
      assert_eq!(view.category(from_id).map(|c| c.assigned), Some(750.0));
    }

    #[tokio::test]
    async fn it_rejects_an_unknown_source_category() {
      let db = database().await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "budget_move_money",
          &McpPerms::default(),
          db,
          json!({ "month": "2026-01", "from_category_id": 9999, "amount": 100.0 }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_rejects_an_unknown_destination_category() {
      let db = database().await;
      let from_id = seed_category(&db, "Fuel").await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "budget_move_money",
          &McpPerms::default(),
          db,
          json!({ "month": "2026-01", "from_category_id": from_id, "to_category_id": 9999, "amount": 100.0 }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_rejects_a_malformed_month() {
      let db = database().await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "budget_move_money",
          &McpPerms::default(),
          db,
          json!({ "month": "2026", "from_category_id": 1, "amount": 100.0 }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod budget_assign_entry {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_pins_a_journal_entry_to_a_category() {
      let db = database().await;
      seed_character(&db, 42).await;
      seed_journal_entry(&db, 5001, 42).await;
      let category_id = seed_category(&db, "Fuel").await;
      let registry = registry();

      let value = registry
        .dispatch(
          "budget_assign_entry",
          &McpPerms::default(),
          db,
          json!({
            "owner_kind": "character",
            "owner_id": 42,
            "entry_kind": "journal",
            "entry_id": 5001,
            "category_id": category_id,
          }),
        )
        .await
        .unwrap();

      assert_eq!(value.get("category_id").and_then(Value::as_i64), Some(category_id));
      assert_eq!(value.get("entry_id").and_then(Value::as_i64), Some(5001));
    }

    #[tokio::test]
    async fn it_rejects_an_entry_the_owner_does_not_hold() {
      let db = database().await;
      seed_character(&db, 42).await;
      let category_id = seed_category(&db, "Fuel").await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "budget_assign_entry",
          &McpPerms::default(),
          db,
          json!({
            "owner_kind": "character",
            "owner_id": 42,
            "entry_kind": "journal",
            "entry_id": 9999,
            "category_id": category_id,
          }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_rejects_an_unknown_owner_kind() {
      let db = database().await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "budget_assign_entry",
          &McpPerms::default(),
          db,
          json!({
            "owner_kind": "alliance",
            "owner_id": 1,
            "entry_kind": "journal",
            "entry_id": 1,
            "category_id": 1,
          }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_rejects_an_unknown_entry_kind() {
      let db = database().await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "budget_assign_entry",
          &McpPerms::default(),
          db,
          json!({
            "owner_kind": "character",
            "owner_id": 42,
            "entry_kind": "dividend",
            "entry_id": 1,
            "category_id": 1,
          }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod planner_replace_segments {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_plan(db: &Database) -> i64 {
      let tree = PlanTree {
        product_type_id: 587,
        root_facility_system: None,
        runs: 10,
        types: vec![PlanType {
          built: false,
          facility_structure: None,
          facility_system: None,
          me: 0,
          te: 0,
          type_id: 587,
          use_stock: false,
        }],
      };
      industry_repo::create_plan(db, "Rifter run", &tree).await.unwrap().id()
    }

    #[tokio::test]
    async fn it_replaces_a_plans_segments() {
      let db = database().await;
      let plan_id = seed_plan(&db).await;
      let registry = registry();

      let value = registry
        .dispatch(
          "planner_replace_segments",
          &McpPerms::default(),
          db.clone(),
          json!({
            "plan_id": plan_id,
            "segments": [{ "type_id": 587, "runs": 5, "segment_index": 0 }],
          }),
        )
        .await
        .unwrap();

      assert_eq!(value.get("segment_count").and_then(Value::as_i64), Some(1));
      assert_eq!(industry_repo::segments_for_plan(&db, plan_id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_rejects_a_missing_plan() {
      let db = database().await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "planner_replace_segments",
          &McpPerms::default(),
          db,
          json!({ "plan_id": 9999, "segments": [] }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_rejects_segments_that_are_not_an_array() {
      let db = database().await;
      let plan_id = seed_plan(&db).await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "planner_replace_segments",
          &McpPerms::default(),
          db,
          json!({ "plan_id": plan_id }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod parse_segments {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_segments_with_optional_pilot_and_clone() {
      let segments = super::super::parse_segments(&json!({
        "segments": [{ "type_id": 587, "runs": 5, "segment_index": 0, "pilot_id": 42, "clone_id": 7 }],
      }))
      .unwrap();

      assert_eq!(segments.len(), 1);
      assert_eq!(segments[0].type_id, 587);
      assert_eq!(segments[0].pilot_id, Some(42));
      assert_eq!(segments[0].clone_id, Some(7));
    }

    #[test]
    fn it_defaults_pilot_and_clone_to_none() {
      let segments = super::super::parse_segments(&json!({
        "segments": [{ "type_id": 587, "runs": 5, "segment_index": 0 }],
      }))
      .unwrap();

      assert_eq!(segments[0].pilot_id, None);
      assert_eq!(segments[0].clone_id, None);
    }

    #[test]
    fn it_errors_when_segments_is_not_an_array() {
      assert!(matches!(
        super::super::parse_segments(&json!({})),
        Err(ToolError::InvalidArguments(_))
      ));
    }

    #[test]
    fn it_errors_on_a_segment_missing_a_field() {
      assert!(matches!(
        super::super::parse_segments(&json!({ "segments": [{ "runs": 5, "segment_index": 0 }] })),
        Err(ToolError::InvalidArguments(_))
      ));
      assert!(matches!(
        super::super::parse_segments(&json!({ "segments": [{ "type_id": 1, "segment_index": 0 }] })),
        Err(ToolError::InvalidArguments(_))
      ));
      assert!(matches!(
        super::super::parse_segments(&json!({ "segments": [{ "type_id": 1, "runs": 5 }] })),
        Err(ToolError::InvalidArguments(_))
      ));
    }
  }

  mod parse_plan_entries {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_entries_with_defaults() {
      let entries = super::super::parse_plan_entries(&json!({
        "entries": [{ "skill_id": 3300, "to_level": 5 }],
      }))
      .unwrap();

      assert_eq!(entries.len(), 1);
      assert_eq!(entries[0].skill_id, 3300);
      assert_eq!(entries[0].to_level, 5);
      assert_eq!(entries[0].priority, "normal");
      assert_eq!(entries[0].note, "");
      assert_eq!(entries[0].is_auto, 0);
    }

    #[test]
    fn it_carries_through_priority_note_and_auto() {
      let entries = super::super::parse_plan_entries(&json!({
        "entries": [{ "skill_id": 3300, "to_level": 3, "priority": "high", "note": "first", "is_auto": true }],
      }))
      .unwrap();

      assert_eq!(entries[0].priority, "high");
      assert_eq!(entries[0].note, "first");
      assert_eq!(entries[0].is_auto, 1);
    }

    #[test]
    fn it_errors_when_entries_is_not_an_array() {
      assert!(matches!(
        super::super::parse_plan_entries(&json!({})),
        Err(ToolError::InvalidArguments(_))
      ));
    }

    #[test]
    fn it_errors_on_a_missing_skill_id_or_out_of_range_level() {
      assert!(matches!(
        super::super::parse_plan_entries(&json!({ "entries": [{ "to_level": 5 }] })),
        Err(ToolError::InvalidArguments(_))
      ));
      assert!(matches!(
        super::super::parse_plan_entries(&json!({ "entries": [{ "skill_id": 3300, "to_level": 6 }] })),
        Err(ToolError::InvalidArguments(_))
      ));
    }
  }

  mod arg_specs {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::mcp::args::ArgType;

    fn tool(name: &str) -> McpTool {
      tools().into_iter().find(|t| t.name() == name).expect("tool registered")
    }

    fn arg_ty(tool: &McpTool, name: &str) -> ArgType {
      tool
        .args()
        .iter()
        .find(|spec| spec.name() == name)
        .unwrap_or_else(|| panic!("`{name}` advertised"))
        .ty()
    }

    fn required_names(tool: &McpTool) -> Vec<&'static str> {
      tool
        .args()
        .iter()
        .filter(|spec| !matches!(spec.ty(), ArgType::OptionalInteger { .. }))
        .map(ArgSpec::name)
        .collect()
    }

    #[test]
    fn integer_id_tools_advertise_integer_args() {
      let add_entry = tool("skill_plan_add_entry");
      assert!(!add_entry.args().is_empty());
      assert_eq!(arg_ty(&add_entry, "plan_id"), ArgType::Integer);
      assert_eq!(arg_ty(&add_entry, "skill_id"), ArgType::Integer);
      assert_eq!(arg_ty(&add_entry, "to_level"), ArgType::Integer);

      let required = required_names(&add_entry);
      assert!(required.contains(&"plan_id"));
      assert!(required.contains(&"skill_id"));
      assert!(required.contains(&"to_level"));
    }

    #[test]
    fn string_and_integer_args_carry_their_wire_types() {
      let create = tool("skill_plan_create");
      assert_eq!(arg_ty(&create, "character_id"), ArgType::Integer);
      assert_eq!(arg_ty(&create, "name"), ArgType::String);
    }

    #[test]
    fn reorder_advertises_an_integer_array() {
      let reorder = tool("skill_plan_reorder");
      assert_eq!(arg_ty(&reorder, "ordered_entry_ids"), ArgType::IntegerArray);
    }

    #[test]
    fn optional_ids_are_not_required() {
      let move_money = tool("budget_move_money");
      assert!(matches!(
        arg_ty(&move_money, "to_category_id"),
        ArgType::OptionalInteger { .. }
      ));
      assert!(!required_names(&move_money).contains(&"to_category_id"));
      assert!(required_names(&move_money).contains(&"from_category_id"));
    }

    #[tokio::test]
    async fn a_numeric_string_id_is_coerced() {
      let db = database().await;
      seed_character(&db, 42).await;
      let plan = skills_repo::create(&db, 42, "Plan").await.unwrap();
      let registry = registry();

      let value = registry
        .dispatch(
          "skill_plan_add_entry",
          &McpPerms::default(),
          db.clone(),
          json!({ "plan_id": plan.id().to_string(), "skill_id": "3300", "to_level": 5 }),
        )
        .await
        .unwrap();

      assert_eq!(value.get("skill_id").and_then(Value::as_i64), Some(3300));
      assert_eq!(skills_repo::entries(&db, plan.id()).await.unwrap().len(), 1);
    }
  }

  mod require_month {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_a_well_formed_month() {
      assert_eq!(
        super::super::require_month(&json!({ "month": "2026-06" })).unwrap(),
        "2026-06"
      );
    }

    #[test]
    fn it_rejects_a_malformed_or_missing_month() {
      for bad in [
        json!({}),
        json!({ "month": "2026" }),
        json!({ "month": "2026-6" }),
        json!({ "month": "20XX-06" }),
      ] {
        assert!(matches!(
          super::super::require_month(&bad),
          Err(ToolError::InvalidArguments(_))
        ));
      }
    }
  }
}
