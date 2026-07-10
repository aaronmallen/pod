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
    model::{LinkSource, NewObjective, Objective, ObjectiveLink, ObjectiveStatus, ObjectiveThreadEntry},
    repo::objective,
  },
};

pub fn tools() -> Vec<McpTool> {
  vec![
    list_tool(),
    get_tool(),
    create_tool(),
    update_tool(),
    complete_tool(),
    cancel_tool(),
    reopen_tool(),
    delete_tool(),
    assign_pilot_tool(),
    unassign_pilot_tool(),
    link_tool(),
    unlink_tool(),
  ]
}

fn internal(error: impl std::fmt::Display) -> ToolError {
  ToolError::Internal(error.to_string())
}

fn require_affected(affected: u64, id: i64) -> Result<(), ToolError> {
  if affected == 0 {
    return Err(ToolError::InvalidArguments(format!("no standing order with id {id}")));
  }
  Ok(())
}

async fn no_esi(_ids: Vec<i64>) -> Result<HashMap<i64, NameRecord>, ClientError> {
  Ok(HashMap::new())
}

async fn resolve_names_map(db: &Database, ids: &[i64]) -> Result<HashMap<i64, ResolvedName>, ToolError> {
  names::resolve(db, ids, no_esi).await.map_err(internal)
}

fn optional_string(args: &Value, key: &str) -> Option<String> {
  args.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn new_objective(args: &Value) -> Result<NewObjective, ToolError> {
  let title = require_str(args, "title")?.to_owned();
  let accent = require_str(args, "accent")?.to_owned();
  Ok(NewObjective {
    accent,
    horizon: optional_string(args, "horizon"),
    target: optional_string(args, "target"),
    title,
    why: optional_string(args, "why"),
  })
}

fn link_source(source_kind: &str, args: &Value) -> Result<LinkSource, ToolError> {
  match source_kind {
    "log_answer" => Ok(LinkSource::LogAnswer {
      question_id: require_str(args, "question_id")?.to_owned(),
    }),
    "field_note" => Ok(LinkSource::FieldNote {
      note_id: require_i64(args, "note_id")?,
    }),
    "killmail" => Ok(LinkSource::Killmail {
      character_id: require_i64(args, "character_id")?,
      killmail_id: require_i64(args, "killmail_id")?,
    }),
    "industry" => Ok(LinkSource::Industry {
      character_id: require_i64(args, "character_id")?,
      product_type_id: require_i64(args, "product_type_id")?,
    }),
    "skill" => Ok(LinkSource::Skill {
      character_id: require_i64(args, "character_id")?,
      skill_id: require_i64(args, "skill_id")?,
    }),
    other => Err(ToolError::InvalidArguments(format!("unknown source_kind `{other}`"))),
  }
}

fn objective_value(objective: &Objective) -> Value {
  json!({
    "accent": objective.accent,
    "cancelled_at": objective.cancelled_at,
    "completed_at": objective.completed_at,
    "created_at": objective.created_at,
    "horizon": objective.horizon,
    "id": objective.id,
    "status": objective.status,
    "target": objective.target,
    "title": objective.title,
    "why": objective.why,
  })
}

fn link_value(link: &ObjectiveLink) -> Value {
  json!({
    "date": link.date,
    "objective_id": link.objective_id,
    "source_kind": link.source_kind,
    "source_ref": link.source_ref,
  })
}

fn thread_value(entry: &ObjectiveThreadEntry) -> Value {
  json!({
    "character": entry.character,
    "date": entry.date,
    "source_kind": entry.source_kind,
    "source_ref": entry.source_ref,
    "text": entry.text,
  })
}

fn pilot_value(names: &HashMap<i64, ResolvedName>, character_id: i64) -> Value {
  json!({
    "character_id": character_id,
    "character_name": names.get(&character_id).map(|resolved| resolved.name.as_str()),
  })
}

async fn list(db: Database, args: Value) -> Result<Value, ToolError> {
  let status = match args.get("status").and_then(Value::as_str) {
    None => None,
    Some(text) => {
      Some(ObjectiveStatus::parse(text).ok_or_else(|| ToolError::InvalidArguments(format!("unknown status `{text}`")))?)
    }
  };
  let objectives = objective::list(&db, status).await.map_err(internal)?;
  let objectives: Vec<Value> = objectives.iter().map(objective_value).collect();
  Ok(json!({ "objectives": objectives }))
}

fn list_tool() -> McpTool {
  McpTool::new(
    "standing_order_list",
    t!("mcp.tools.standing_order_list").into_owned(),
    Permission::Read,
    |db, args: Value| async move { list(db, args).await },
  )
  .with_args([ArgSpec::optional_string(
    "status",
    t!("mcp.tools.standing_order_list_status").into_owned(),
  )])
}

async fn get(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let Some(objective) = objective::get(&db, id).await.map_err(internal)? else {
    return Err(ToolError::InvalidArguments(format!("no standing order with id {id}")));
  };
  let pilot_ids = objective::pilots(&db, id).await.map_err(internal)?;
  let names = resolve_names_map(&db, &pilot_ids).await?;
  let pilots: Vec<Value> = pilot_ids
    .iter()
    .map(|pilot_id| pilot_value(&names, *pilot_id))
    .collect();
  let links = objective::links_for_objective(&db, id).await.map_err(internal)?;
  let links: Vec<Value> = links.iter().map(link_value).collect();
  let thread = objective::thread(&db, id).await.map_err(internal)?;
  let thread: Vec<Value> = thread.iter().map(thread_value).collect();
  Ok(json!({
    "links": links,
    "objective": objective_value(&objective),
    "pilots": pilots,
    "thread": thread,
  }))
}

fn get_tool() -> McpTool {
  McpTool::new(
    "standing_order_get",
    t!("mcp.tools.standing_order_get").into_owned(),
    Permission::Read,
    |db, args: Value| async move { get(db, args).await },
  )
  .with_args([ArgSpec::integer(
    "id",
    t!("mcp.tools.standing_order_get_id").into_owned(),
  )])
}

async fn create(db: Database, args: Value) -> Result<Value, ToolError> {
  let input = new_objective(&args)?;
  let objective = objective::create(&db, &input).await.map_err(internal)?;
  Ok(objective_value(&objective))
}

fn write_fields() -> [ArgSpec; 5] {
  [
    ArgSpec::string("title", t!("mcp.tools.standing_order_title").into_owned()),
    ArgSpec::string("accent", t!("mcp.tools.standing_order_accent").into_owned()),
    ArgSpec::optional_string("why", t!("mcp.tools.standing_order_why").into_owned()),
    ArgSpec::optional_string("target", t!("mcp.tools.standing_order_target").into_owned()),
    ArgSpec::optional_string("horizon", t!("mcp.tools.standing_order_horizon").into_owned()),
  ]
}

fn create_tool() -> McpTool {
  McpTool::new(
    "standing_order_create",
    t!("mcp.tools.standing_order_create").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { create(db, args).await },
  )
  .with_args(write_fields())
}

async fn update(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let input = new_objective(&args)?;
  let affected = objective::update(&db, id, &input).await.map_err(internal)?;
  require_affected(affected, id)?;
  let objective = objective::get(&db, id).await.map_err(internal)?;
  Ok(objective.as_ref().map_or(Value::Null, objective_value))
}

fn update_tool() -> McpTool {
  let mut args = vec![ArgSpec::integer(
    "id",
    t!("mcp.tools.standing_order_update_id").into_owned(),
  )];
  args.extend(write_fields());
  McpTool::new(
    "standing_order_update",
    t!("mcp.tools.standing_order_update").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { update(db, args).await },
  )
  .with_args(args)
}

async fn complete(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let affected = objective::complete(&db, id).await.map_err(internal)?;
  require_affected(affected, id)?;
  Ok(json!({ "id": id, "status": "complete" }))
}

fn complete_tool() -> McpTool {
  McpTool::new(
    "standing_order_complete",
    t!("mcp.tools.standing_order_complete").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { complete(db, args).await },
  )
  .with_args([ArgSpec::integer(
    "id",
    t!("mcp.tools.standing_order_complete_id").into_owned(),
  )])
}

async fn cancel(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let affected = objective::cancel(&db, id).await.map_err(internal)?;
  require_affected(affected, id)?;
  Ok(json!({ "id": id, "status": "cancelled" }))
}

fn cancel_tool() -> McpTool {
  McpTool::new(
    "standing_order_cancel",
    t!("mcp.tools.standing_order_cancel").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { cancel(db, args).await },
  )
  .with_args([ArgSpec::integer(
    "id",
    t!("mcp.tools.standing_order_cancel_id").into_owned(),
  )])
}

async fn reopen(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let affected = objective::reopen(&db, id).await.map_err(internal)?;
  require_affected(affected, id)?;
  Ok(json!({ "id": id, "status": "active" }))
}

fn reopen_tool() -> McpTool {
  McpTool::new(
    "standing_order_reopen",
    t!("mcp.tools.standing_order_reopen").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { reopen(db, args).await },
  )
  .with_args([ArgSpec::integer(
    "id",
    t!("mcp.tools.standing_order_reopen_id").into_owned(),
  )])
}

async fn delete(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let affected = objective::delete(&db, id).await.map_err(internal)?;
  require_affected(affected, id)?;
  Ok(json!({ "id": id, "deleted": true }))
}

fn delete_tool() -> McpTool {
  McpTool::new(
    "standing_order_delete",
    t!("mcp.tools.standing_order_delete").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { delete(db, args).await },
  )
  .with_args([ArgSpec::integer(
    "id",
    t!("mcp.tools.standing_order_delete_id").into_owned(),
  )])
}

async fn assign_pilot(db: Database, args: Value) -> Result<Value, ToolError> {
  let objective_id = require_i64(&args, "objective_id")?;
  let character_id = require_i64(&args, "character_id")?;
  objective::assign_pilot(&db, objective_id, character_id)
    .await
    .map_err(internal)?;
  Ok(json!({ "character_id": character_id, "objective_id": objective_id }))
}

fn assign_pilot_tool() -> McpTool {
  McpTool::new(
    "standing_order_assign_pilot",
    t!("mcp.tools.standing_order_assign_pilot").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { assign_pilot(db, args).await },
  )
  .with_args([
    ArgSpec::integer(
      "objective_id",
      t!("mcp.tools.standing_order_assign_pilot_objective_id").into_owned(),
    ),
    ArgSpec::integer(
      "character_id",
      t!("mcp.tools.standing_order_assign_pilot_character_id").into_owned(),
    ),
  ])
}

async fn unassign_pilot(db: Database, args: Value) -> Result<Value, ToolError> {
  let objective_id = require_i64(&args, "objective_id")?;
  let character_id = require_i64(&args, "character_id")?;
  let affected = objective::unassign_pilot(&db, objective_id, character_id)
    .await
    .map_err(internal)?;
  Ok(json!({
    "character_id": character_id,
    "objective_id": objective_id,
    "unassigned": affected > 0,
  }))
}

fn unassign_pilot_tool() -> McpTool {
  McpTool::new(
    "standing_order_unassign_pilot",
    t!("mcp.tools.standing_order_unassign_pilot").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { unassign_pilot(db, args).await },
  )
  .with_args([
    ArgSpec::integer(
      "objective_id",
      t!("mcp.tools.standing_order_unassign_pilot_objective_id").into_owned(),
    ),
    ArgSpec::integer(
      "character_id",
      t!("mcp.tools.standing_order_unassign_pilot_character_id").into_owned(),
    ),
  ])
}

fn link_args() -> [ArgSpec; 9] {
  [
    ArgSpec::integer(
      "objective_id",
      t!("mcp.tools.standing_order_link_objective_id").into_owned(),
    ),
    ArgSpec::string("date", t!("mcp.tools.standing_order_link_date").into_owned()),
    ArgSpec::string(
      "source_kind",
      t!("mcp.tools.standing_order_link_source_kind").into_owned(),
    ),
    ArgSpec::optional_string(
      "question_id",
      t!("mcp.tools.standing_order_link_question_id").into_owned(),
    ),
    ArgSpec::optional_integer("note_id", 0, t!("mcp.tools.standing_order_link_note_id").into_owned()),
    ArgSpec::optional_integer(
      "character_id",
      0,
      t!("mcp.tools.standing_order_link_character_id").into_owned(),
    ),
    ArgSpec::optional_integer(
      "killmail_id",
      0,
      t!("mcp.tools.standing_order_link_killmail_id").into_owned(),
    ),
    ArgSpec::optional_integer(
      "product_type_id",
      0,
      t!("mcp.tools.standing_order_link_product_type_id").into_owned(),
    ),
    ArgSpec::optional_integer("skill_id", 0, t!("mcp.tools.standing_order_link_skill_id").into_owned()),
  ]
}

async fn link(db: Database, args: Value) -> Result<Value, ToolError> {
  let objective_id = require_i64(&args, "objective_id")?;
  let date = require_str(&args, "date")?.to_owned();
  let source_kind = require_str(&args, "source_kind")?;
  let source = link_source(source_kind, &args)?;
  objective::set_link(&db, objective_id, &date, &source)
    .await
    .map_err(internal)?;
  Ok(json!({
    "date": date,
    "objective_id": objective_id,
    "source_kind": source.source_kind(),
    "source_ref": source.source_ref(),
  }))
}

fn link_tool() -> McpTool {
  McpTool::new(
    "standing_order_link",
    t!("mcp.tools.standing_order_link").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { link(db, args).await },
  )
  .with_args(link_args())
}

async fn unlink(db: Database, args: Value) -> Result<Value, ToolError> {
  let objective_id = require_i64(&args, "objective_id")?;
  let date = require_str(&args, "date")?.to_owned();
  let source_kind = require_str(&args, "source_kind")?;
  let source = link_source(source_kind, &args)?;
  let affected = objective::clear_link(&db, objective_id, &date, &source)
    .await
    .map_err(internal)?;
  Ok(json!({
    "date": date,
    "objective_id": objective_id,
    "source_kind": source.source_kind(),
    "source_ref": source.source_ref(),
    "unlinked": affected > 0,
  }))
}

fn unlink_tool() -> McpTool {
  McpTool::new(
    "standing_order_unlink",
    t!("mcp.tools.standing_order_unlink").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { unlink(db, args).await },
  )
  .with_args(link_args())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
    repo::{captains_log, character, infra},
  };

  const PILOT: i64 = 90_000_001;

  async fn seed_owned(db: &Database, id: i64, name: &str) {
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
    infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
      .await
      .unwrap();
  }

  fn new_order() -> Value {
    json!({ "title": "Fund a Nyx", "accent": "#FF8800", "why": "Stay sharp", "target": "40b ISK", "horizon": "Q3" })
  }

  mod list {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_lists_every_objective_and_filters_by_status() {
      let db = store::open_test().await.unwrap();
      let first = create(db.clone(), new_order()).await.unwrap();
      create(db.clone(), json!({ "title": "Roam", "accent": "#00C2FF" }))
        .await
        .unwrap();
      complete(db.clone(), json!({ "id": first["id"] })).await.unwrap();

      let all = list(db.clone(), json!({})).await.unwrap();
      assert_eq!(all["objectives"].as_array().unwrap().len(), 2);

      let active = list(db.clone(), json!({ "status": "active" })).await.unwrap();
      let active = active["objectives"].as_array().unwrap();
      assert_eq!(active.len(), 1);
      assert_eq!(active[0]["title"], "Roam");

      let complete = list(db.clone(), json!({ "status": "complete" })).await.unwrap();
      let complete = complete["objectives"].as_array().unwrap();
      assert_eq!(complete.len(), 1);
      assert_eq!(complete[0]["id"], first["id"]);
    }

    #[tokio::test]
    async fn it_rejects_an_unknown_status() {
      let db = store::open_test().await.unwrap();

      let outcome = list(db, json!({ "status": "archived" })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod get {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_embeds_pilots_by_name_links_and_the_thread() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      let created = create(db.clone(), new_order()).await.unwrap();
      let id = created["id"].as_i64().unwrap();
      assign_pilot(db.clone(), json!({ "objective_id": id, "character_id": PILOT }))
        .await
        .unwrap();
      captains_log::upsert_answer(&db, "2026-07-04", "goal", Some("Undock the barge."))
        .await
        .unwrap();
      link(
        db.clone(),
        json!({ "objective_id": id, "date": "2026-07-04", "source_kind": "log_answer", "question_id": "goal" }),
      )
      .await
      .unwrap();

      let value = get(db, json!({ "id": id })).await.unwrap();

      assert_eq!(value["objective"]["title"], "Fund a Nyx");
      let pilots = value["pilots"].as_array().unwrap();
      assert_eq!(pilots.len(), 1);
      assert_eq!(pilots[0]["character_id"], PILOT);
      assert_eq!(pilots[0]["character_name"], "Pilot One");
      let links = value["links"].as_array().unwrap();
      assert_eq!(links.len(), 1);
      assert_eq!(links[0]["source_kind"], "log_answer");
      let thread = value["thread"].as_array().unwrap();
      assert_eq!(thread.len(), 1);
      assert_eq!(thread[0]["text"], "Undock the barge.");
    }

    #[tokio::test]
    async fn it_errors_for_a_missing_objective() {
      let db = store::open_test().await.unwrap();

      let outcome = get(db, json!({ "id": 999 })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod crud {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_reads_back_and_updates() {
      let db = store::open_test().await.unwrap();

      let created = create(db.clone(), new_order()).await.unwrap();
      let id = created["id"].as_i64().unwrap();
      assert_eq!(created["status"], "active");
      assert_eq!(created["title"], "Fund a Nyx");
      assert_eq!(created["accent"], "#FF8800");

      let updated = update(
        db.clone(),
        json!({ "id": id, "title": "Fund two Nyxes", "accent": "#00C2FF" }),
      )
      .await
      .unwrap();
      assert_eq!(updated["title"], "Fund two Nyxes");
      assert_eq!(updated["accent"], "#00C2FF");
      assert_eq!(updated["why"], Value::Null);

      let row = objective::get(&db, id).await.unwrap().unwrap();
      assert_eq!(row.title, "Fund two Nyxes");
      assert_eq!(row.why, None);
    }

    #[tokio::test]
    async fn it_transitions_status_through_the_repo() {
      let db = store::open_test().await.unwrap();
      let created = create(db.clone(), new_order()).await.unwrap();
      let id = created["id"].as_i64().unwrap();

      complete(db.clone(), json!({ "id": id })).await.unwrap();
      let row = objective::get(&db, id).await.unwrap().unwrap();
      assert_eq!(row.status, "complete");
      assert!(row.completed_at.is_some());

      cancel(db.clone(), json!({ "id": id })).await.unwrap();
      let row = objective::get(&db, id).await.unwrap().unwrap();
      assert_eq!(row.status, "cancelled");
      assert!(row.cancelled_at.is_some());

      reopen(db.clone(), json!({ "id": id })).await.unwrap();
      let row = objective::get(&db, id).await.unwrap().unwrap();
      assert_eq!(row.status, "active");
      assert!(row.completed_at.is_none());
      assert!(row.cancelled_at.is_none());
    }

    #[tokio::test]
    async fn it_deletes_an_objective() {
      let db = store::open_test().await.unwrap();
      let created = create(db.clone(), new_order()).await.unwrap();
      let id = created["id"].as_i64().unwrap();

      let value = delete(db.clone(), json!({ "id": id })).await.unwrap();

      assert_eq!(value["deleted"], true);
      assert_eq!(objective::get(&db, id).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_errors_when_deleting_a_missing_objective() {
      let db = store::open_test().await.unwrap();

      let outcome = delete(db, json!({ "id": 999 })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_errors_when_updating_a_missing_objective() {
      let db = store::open_test().await.unwrap();

      let outcome = update(db, json!({ "id": 999, "title": "Gone", "accent": "#FF8800" })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod pilots {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_assigns_and_unassigns_a_pilot() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      let created = create(db.clone(), new_order()).await.unwrap();
      let id = created["id"].as_i64().unwrap();

      assign_pilot(db.clone(), json!({ "objective_id": id, "character_id": PILOT }))
        .await
        .unwrap();
      assert_eq!(objective::pilots(&db, id).await.unwrap(), vec![PILOT]);

      let value = unassign_pilot(db.clone(), json!({ "objective_id": id, "character_id": PILOT }))
        .await
        .unwrap();
      assert_eq!(value["unassigned"], true);
      assert!(objective::pilots(&db, id).await.unwrap().is_empty());
    }
  }

  mod link {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_links_and_unlinks_across_source_kinds() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      let created = create(db.clone(), new_order()).await.unwrap();
      let id = created["id"].as_i64().unwrap();

      let killmail = link(
        db.clone(),
        json!({
          "objective_id": id, "date": "2026-07-05", "source_kind": "killmail",
          "character_id": PILOT, "killmail_id": 501,
        }),
      )
      .await
      .unwrap();
      assert_eq!(killmail["source_kind"], "killmail");
      assert_eq!(killmail["source_ref"], "90000001:501");

      let note = link(
        db.clone(),
        json!({ "objective_id": id, "date": "2026-07-05", "source_kind": "field_note", "note_id": 42 }),
      )
      .await
      .unwrap();
      assert_eq!(note["source_ref"], "42");

      assert_eq!(objective::links_for_objective(&db, id).await.unwrap().len(), 2);

      let unlinked = unlink(
        db.clone(),
        json!({
          "objective_id": id, "date": "2026-07-05", "source_kind": "killmail",
          "character_id": PILOT, "killmail_id": 501,
        }),
      )
      .await
      .unwrap();
      assert_eq!(unlinked["unlinked"], true);
      assert_eq!(objective::links_for_objective(&db, id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_rejects_an_unknown_source_kind() {
      let db = store::open_test().await.unwrap();
      let created = create(db.clone(), new_order()).await.unwrap();

      let outcome = link(
        db,
        json!({ "objective_id": created["id"], "date": "2026-07-05", "source_kind": "mystery" }),
      )
      .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_rejects_a_missing_ref_field() {
      let db = store::open_test().await.unwrap();
      let created = create(db.clone(), new_order()).await.unwrap();

      let outcome = link(
        db,
        json!({ "objective_id": created["id"], "date": "2026-07-05", "source_kind": "skill", "character_id": PILOT }),
      )
      .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }
}
