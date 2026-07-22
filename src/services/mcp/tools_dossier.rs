use std::collections::HashMap;

use serde_json::{Value, json};

use crate::{
  clients::{Error as ClientError, esi::models::universe::NameRecord},
  services::mcp::{
    args::{ArgSpec, require_i64, require_str},
    names::{self, ResolvedName},
    tool::{McpTool, Permission, ToolError},
  },
  store::{
    Database,
    model::{Dossier, DossierOrder},
    repo::{dossier, objective},
  },
};

pub fn tools() -> Vec<McpTool> {
  vec![
    get_tool(),
    list_orders_tool(),
    set_brief_tool(),
    add_order_tool(),
    edit_order_tool(),
    complete_order_tool(),
    cancel_order_tool(),
    reopen_order_tool(),
    remove_order_tool(),
    link_objective_tool(),
    unlink_objective_tool(),
  ]
}

fn internal(error: impl std::fmt::Display) -> ToolError {
  ToolError::Internal(error.to_string())
}

fn require_affected(affected: u64, id: i64) -> Result<(), ToolError> {
  if affected == 0 {
    return Err(ToolError::InvalidArguments(format!("no dossier order with id {id}")));
  }
  Ok(())
}

async fn no_esi(_ids: Vec<i64>) -> Result<HashMap<i64, NameRecord>, ClientError> {
  Ok(HashMap::new())
}

async fn resolve_names_map(db: &Database, ids: &[i64]) -> Result<HashMap<i64, ResolvedName>, ToolError> {
  names::resolve(db, ids, no_esi).await.map_err(internal)
}

fn name_of(names: &HashMap<i64, ResolvedName>, id: i64) -> Option<&str> {
  names.get(&id).map(|resolved| resolved.name.as_str())
}

async fn resolve_titles(db: &Database, orders: &[DossierOrder]) -> Result<HashMap<i64, String>, ToolError> {
  let mut titles = HashMap::new();
  for objective_id in orders.iter().filter_map(|order| order.objective_id) {
    if titles.contains_key(&objective_id) {
      continue;
    }
    if let Some(objective) = objective::get(db, objective_id).await.map_err(internal)? {
      titles.insert(objective_id, objective.title);
    }
  }
  Ok(titles)
}

fn brief_value(brief: &Dossier) -> Value {
  json!({
    "character_id": brief.character_id,
    "created_at": brief.created_at,
    "near_term": brief.near_term,
    "purpose": brief.purpose,
    "updated_at": brief.updated_at,
  })
}

fn order_value(order: &DossierOrder, titles: &HashMap<i64, String>) -> Value {
  json!({
    "character_id": order.character_id,
    "created_at": order.created_at,
    "id": order.id,
    "objective_id": order.objective_id,
    "objective_title": order.objective_id.and_then(|id| titles.get(&id).cloned()),
    "position": order.position,
    "status": order.status,
    "text": order.text,
    "updated_at": order.updated_at,
  })
}

async fn get(db: Database, args: Value) -> Result<Value, ToolError> {
  let character_id = require_i64(&args, "character_id")?;
  let brief = dossier::get_brief(&db, character_id).await.map_err(internal)?;
  let orders = dossier::list_orders(&db, character_id).await.map_err(internal)?;
  let titles = resolve_titles(&db, &orders).await?;
  let names = resolve_names_map(&db, &[character_id]).await?;
  let orders: Vec<Value> = orders.iter().map(|order| order_value(order, &titles)).collect();
  Ok(json!({
    "brief": brief.as_ref().map(brief_value),
    "character_id": character_id,
    "character_name": name_of(&names, character_id),
    "orders": orders,
  }))
}

fn get_tool() -> McpTool {
  McpTool::new(
    "dossier_get",
    t!("mcp.tools.dossier_get").into_owned(),
    Permission::Read,
    |db, args: Value| async move { get(db, args).await },
  )
  .with_args([ArgSpec::integer(
    "character_id",
    t!("mcp.tools.dossier_get_character_id").into_owned(),
  )])
}

async fn list_orders(db: Database, args: Value) -> Result<Value, ToolError> {
  let character_id = require_i64(&args, "character_id")?;
  let orders = dossier::list_orders(&db, character_id).await.map_err(internal)?;
  let titles = resolve_titles(&db, &orders).await?;
  let orders: Vec<Value> = orders.iter().map(|order| order_value(order, &titles)).collect();
  Ok(json!({ "character_id": character_id, "orders": orders }))
}

fn list_orders_tool() -> McpTool {
  McpTool::new(
    "dossier_list_orders",
    t!("mcp.tools.dossier_list_orders").into_owned(),
    Permission::Read,
    |db, args: Value| async move { list_orders(db, args).await },
  )
  .with_args([ArgSpec::integer(
    "character_id",
    t!("mcp.tools.dossier_list_orders_character_id").into_owned(),
  )])
}

async fn set_brief(db: Database, args: Value) -> Result<Value, ToolError> {
  let character_id = require_i64(&args, "character_id")?;
  let purpose = args.get("purpose").and_then(Value::as_str);
  let near_term = args.get("near_term").and_then(Value::as_str);
  let brief = dossier::upsert_brief(&db, character_id, purpose, near_term)
    .await
    .map_err(internal)?;
  Ok(brief_value(&brief))
}

fn set_brief_tool() -> McpTool {
  McpTool::new(
    "dossier_set_brief",
    t!("mcp.tools.dossier_set_brief").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { set_brief(db, args).await },
  )
  .with_args([
    ArgSpec::integer(
      "character_id",
      t!("mcp.tools.dossier_set_brief_character_id").into_owned(),
    ),
    ArgSpec::optional_string("purpose", t!("mcp.tools.dossier_set_brief_purpose").into_owned()),
    ArgSpec::optional_string("near_term", t!("mcp.tools.dossier_set_brief_near_term").into_owned()),
  ])
}

async fn add_order(db: Database, args: Value) -> Result<Value, ToolError> {
  let character_id = require_i64(&args, "character_id")?;
  let text = require_str(&args, "text")?.to_owned();
  let order = dossier::add_order(&db, character_id, &text).await.map_err(internal)?;
  Ok(order_value(&order, &HashMap::new()))
}

fn add_order_tool() -> McpTool {
  McpTool::new(
    "dossier_add_order",
    t!("mcp.tools.dossier_add_order").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { add_order(db, args).await },
  )
  .with_args([
    ArgSpec::integer(
      "character_id",
      t!("mcp.tools.dossier_add_order_character_id").into_owned(),
    ),
    ArgSpec::string("text", t!("mcp.tools.dossier_add_order_text").into_owned()),
  ])
}

async fn edit_order(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let text = require_str(&args, "text")?.to_owned();
  let affected = dossier::edit_order(&db, id, &text).await.map_err(internal)?;
  require_affected(affected, id)?;
  Ok(json!({ "id": id, "text": text }))
}

fn edit_order_tool() -> McpTool {
  McpTool::new(
    "dossier_edit_order",
    t!("mcp.tools.dossier_edit_order").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { edit_order(db, args).await },
  )
  .with_args([
    ArgSpec::integer("id", t!("mcp.tools.dossier_edit_order_id").into_owned()),
    ArgSpec::string("text", t!("mcp.tools.dossier_edit_order_text").into_owned()),
  ])
}

async fn complete_order(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let affected = dossier::complete_order(&db, id).await.map_err(internal)?;
  require_affected(affected, id)?;
  Ok(json!({ "id": id, "status": "complete" }))
}

fn complete_order_tool() -> McpTool {
  McpTool::new(
    "dossier_complete_order",
    t!("mcp.tools.dossier_complete_order").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { complete_order(db, args).await },
  )
  .with_args([ArgSpec::integer(
    "id",
    t!("mcp.tools.dossier_complete_order_id").into_owned(),
  )])
}

async fn cancel_order(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let affected = dossier::cancel_order(&db, id).await.map_err(internal)?;
  require_affected(affected, id)?;
  Ok(json!({ "id": id, "status": "cancelled" }))
}

fn cancel_order_tool() -> McpTool {
  McpTool::new(
    "dossier_cancel_order",
    t!("mcp.tools.dossier_cancel_order").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { cancel_order(db, args).await },
  )
  .with_args([ArgSpec::integer(
    "id",
    t!("mcp.tools.dossier_cancel_order_id").into_owned(),
  )])
}

async fn reopen_order(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let affected = dossier::reopen_order(&db, id).await.map_err(internal)?;
  require_affected(affected, id)?;
  Ok(json!({ "id": id, "status": "active" }))
}

fn reopen_order_tool() -> McpTool {
  McpTool::new(
    "dossier_reopen_order",
    t!("mcp.tools.dossier_reopen_order").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { reopen_order(db, args).await },
  )
  .with_args([ArgSpec::integer(
    "id",
    t!("mcp.tools.dossier_reopen_order_id").into_owned(),
  )])
}

async fn remove_order(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let affected = dossier::remove_order(&db, id).await.map_err(internal)?;
  require_affected(affected, id)?;
  Ok(json!({ "id": id, "removed": true }))
}

fn remove_order_tool() -> McpTool {
  McpTool::new(
    "dossier_remove_order",
    t!("mcp.tools.dossier_remove_order").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { remove_order(db, args).await },
  )
  .with_args([ArgSpec::integer(
    "id",
    t!("mcp.tools.dossier_remove_order_id").into_owned(),
  )])
}

async fn link_objective(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let objective_id = require_i64(&args, "objective_id")?;
  let affected = dossier::set_objective(&db, id, objective_id).await.map_err(internal)?;
  require_affected(affected, id)?;
  Ok(json!({ "id": id, "objective_id": objective_id }))
}

fn link_objective_tool() -> McpTool {
  McpTool::new(
    "dossier_link_objective",
    t!("mcp.tools.dossier_link_objective").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { link_objective(db, args).await },
  )
  .with_args([
    ArgSpec::integer("id", t!("mcp.tools.dossier_link_objective_id").into_owned()),
    ArgSpec::integer(
      "objective_id",
      t!("mcp.tools.dossier_link_objective_objective_id").into_owned(),
    ),
  ])
}

async fn unlink_objective(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let affected = dossier::clear_objective(&db, id).await.map_err(internal)?;
  require_affected(affected, id)?;
  Ok(json!({ "id": id, "objective_id": Value::Null }))
}

fn unlink_objective_tool() -> McpTool {
  McpTool::new(
    "dossier_unlink_objective",
    t!("mcp.tools.dossier_unlink_objective").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { unlink_objective(db, args).await },
  )
  .with_args([ArgSpec::integer(
    "id",
    t!("mcp.tools.dossier_unlink_objective_id").into_owned(),
  )])
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, NewObjective, Race},
    repo::character,
  };

  const PILOT: i64 = 90_000_001;

  async fn seed_character(db: &Database, id: i64, name: &str) {
    let corp_id = 98_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, name);
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  async fn seed_objective(db: &Database, title: &str) -> i64 {
    objective::create(
      db,
      &NewObjective {
        accent: "#FF8800".to_owned(),
        horizon: None,
        target: None,
        title: title.to_owned(),
        why: None,
      },
    )
    .await
    .unwrap()
    .id
  }

  mod get {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_assembles_brief_orders_and_resolved_objective_titles() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot One").await;
      let objective_id = seed_objective(&db, "Fund a Nyx").await;
      set_brief(
        db.clone(),
        json!({ "character_id": PILOT, "purpose": "Hunt", "near_term": "Ratting" }),
      )
      .await
      .unwrap();
      let first = add_order(db.clone(), json!({ "character_id": PILOT, "text": "Fit a Loki" }))
        .await
        .unwrap();
      add_order(db.clone(), json!({ "character_id": PILOT, "text": "Save ISK" }))
        .await
        .unwrap();
      link_objective(db.clone(), json!({ "id": first["id"], "objective_id": objective_id }))
        .await
        .unwrap();

      let value = get(db, json!({ "character_id": PILOT })).await.unwrap();

      assert_eq!(value["character_id"], PILOT);
      assert_eq!(value["character_name"], "Pilot One");
      assert_eq!(value["brief"]["purpose"], "Hunt");
      assert_eq!(value["brief"]["near_term"], "Ratting");
      let orders = value["orders"].as_array().unwrap();
      assert_eq!(orders.len(), 2);
      assert_eq!(orders[0]["text"], "Fit a Loki");
      assert_eq!(orders[0]["objective_id"], objective_id);
      assert_eq!(orders[0]["objective_title"], "Fund a Nyx");
      assert_eq!(orders[1]["objective_title"], Value::Null);
    }

    #[tokio::test]
    async fn it_returns_a_null_brief_and_no_orders_before_anything_is_written() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot One").await;

      let value = get(db, json!({ "character_id": PILOT })).await.unwrap();

      assert_eq!(value["brief"], Value::Null);
      assert_eq!(value["orders"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn it_requires_the_character_id() {
      let db = store::open_test().await.unwrap();

      let outcome = get(db, json!({})).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod set_brief {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_upserts_the_brief_in_place() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot One").await;

      let created = set_brief(
        db.clone(),
        json!({ "character_id": PILOT, "purpose": "Hunt", "near_term": "Ratting" }),
      )
      .await
      .unwrap();
      assert_eq!(created["purpose"], "Hunt");

      let updated = set_brief(db.clone(), json!({ "character_id": PILOT, "purpose": "Mine" }))
        .await
        .unwrap();
      assert_eq!(updated["purpose"], "Mine");
      assert_eq!(updated["near_term"], Value::Null);
      assert_eq!(updated["created_at"], created["created_at"]);

      let row = dossier::get_brief(&db, PILOT).await.unwrap().unwrap();
      assert_eq!(row.purpose.as_deref(), Some("Mine"));
      assert_eq!(row.near_term, None);
    }
  }

  mod orders {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_adds_edits_and_transitions_status() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot One").await;

      let order = add_order(db.clone(), json!({ "character_id": PILOT, "text": "Roam" }))
        .await
        .unwrap();
      let id = order["id"].as_i64().unwrap();
      assert_eq!(order["status"], "active");
      assert_eq!(order["position"], 0);

      edit_order(db.clone(), json!({ "id": id, "text": "Roam harder" }))
        .await
        .unwrap();
      assert_eq!(dossier::list_orders(&db, PILOT).await.unwrap()[0].text, "Roam harder");

      complete_order(db.clone(), json!({ "id": id })).await.unwrap();
      assert_eq!(dossier::list_orders(&db, PILOT).await.unwrap()[0].status, "complete");

      cancel_order(db.clone(), json!({ "id": id })).await.unwrap();
      assert_eq!(dossier::list_orders(&db, PILOT).await.unwrap()[0].status, "cancelled");

      reopen_order(db.clone(), json!({ "id": id })).await.unwrap();
      assert_eq!(dossier::list_orders(&db, PILOT).await.unwrap()[0].status, "active");
    }

    #[tokio::test]
    async fn it_removes_an_order() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot One").await;
      let order = add_order(db.clone(), json!({ "character_id": PILOT, "text": "Roam" }))
        .await
        .unwrap();

      let value = remove_order(db.clone(), json!({ "id": order["id"] })).await.unwrap();

      assert_eq!(value["removed"], true);
      assert!(dossier::list_orders(&db, PILOT).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_errors_when_removing_a_missing_order() {
      let db = store::open_test().await.unwrap();

      let outcome = remove_order(db, json!({ "id": 999 })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_errors_when_editing_a_missing_order() {
      let db = store::open_test().await.unwrap();

      let outcome = edit_order(db, json!({ "id": 999, "text": "x" })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_lists_orders_for_a_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot One").await;
      add_order(db.clone(), json!({ "character_id": PILOT, "text": "One" }))
        .await
        .unwrap();
      add_order(db.clone(), json!({ "character_id": PILOT, "text": "Two" }))
        .await
        .unwrap();

      let value = list_orders(db, json!({ "character_id": PILOT })).await.unwrap();

      let orders = value["orders"].as_array().unwrap();
      assert_eq!(orders.len(), 2);
      assert_eq!(orders[0]["text"], "One");
      assert_eq!(orders[1]["text"], "Two");
    }
  }

  mod link {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_links_and_unlinks_an_objective() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot One").await;
      let objective_id = seed_objective(&db, "Fund a Nyx").await;
      let order = add_order(db.clone(), json!({ "character_id": PILOT, "text": "Save ISK" }))
        .await
        .unwrap();
      let id = order["id"].as_i64().unwrap();

      link_objective(db.clone(), json!({ "id": id, "objective_id": objective_id }))
        .await
        .unwrap();
      assert_eq!(
        dossier::list_orders(&db, PILOT).await.unwrap()[0].objective_id,
        Some(objective_id)
      );

      unlink_objective(db.clone(), json!({ "id": id })).await.unwrap();
      assert_eq!(dossier::list_orders(&db, PILOT).await.unwrap()[0].objective_id, None);
    }

    #[tokio::test]
    async fn it_errors_when_linking_a_missing_order() {
      let db = store::open_test().await.unwrap();
      let objective_id = seed_objective(&db, "Fund a Nyx").await;

      let outcome = link_objective(db, json!({ "id": 999, "objective_id": objective_id })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }
}
