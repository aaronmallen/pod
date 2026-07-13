use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use serde_json::{Value, json};

use crate::{
  clients::{Error as ClientError, esi::models::universe::NameRecord},
  features::roster::captains_log::{
    prompts::{self, Completeness, DayActivity, LossEngagement},
    rollup::{self, Combat, DayRollup},
  },
  services::mcp::{
    args::{ArgSpec, paginate_vec, pagination, require_i64, require_str},
    names::{self, ResolvedName},
    tool::{McpTool, Permission, ToolError},
  },
  store::{
    Database,
    model::{
      CaptainsLog, Dossier, DossierOrder, FieldNote, KillmailReport, Objective, PromptQuestion, PromptQuestionKind,
      PromptSection, PromptSectionKind, PromptTriggers, SkillCompletion,
    },
    repo::{
      calendar_event_note, captains_log,
      captains_log::AnswerKey,
      captains_log_rollup::{self, CalendarEntry, CombatKill, DayMoney, IndustryDelivery, NetWorthDelta},
      character, dossier, field_notes, killmail_report, objective,
    },
  },
};

const DAYS_PER_PAGE: i64 = 7;

const YC_EPOCH_OFFSET: i32 = 1898;

pub fn tools() -> Vec<McpTool> {
  vec![
    list_days_tool(),
    range_tool(),
    get_day_tool(),
    describe_structure_tool(),
    set_answer_tool(),
    set_kill_report_tool(),
    set_narrative_tool(),
    add_note_tool(),
    list_notes_tool(),
    delete_note_tool(),
  ]
}

async fn add_note(db: Database, args: Value) -> Result<Value, ToolError> {
  let date = require_date(&args)?;
  let text = require_str(&args, "text")?.to_owned();
  let note = field_notes::insert(&db, &date, &text).await.map_err(internal)?;
  Ok(field_note_value(&note))
}

fn add_note_tool() -> McpTool {
  McpTool::new(
    "captains_log_add_note",
    t!("mcp.tools.captains_log_add_note").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { add_note(db, args).await },
  )
  .with_args([
    ArgSpec::string("date", t!("mcp.tools.captains_log_add_note_date").into_owned()),
    ArgSpec::string("text", t!("mcp.tools.captains_log_add_note_text").into_owned()),
  ])
}

async fn delete_note(db: Database, args: Value) -> Result<Value, ToolError> {
  let id = require_i64(&args, "id")?;
  let deleted = field_notes::delete(&db, id).await.map_err(internal)?;
  if deleted == 0 {
    return Err(ToolError::InvalidArguments(format!("no field note with id {id}")));
  }
  Ok(json!({ "deleted": true, "id": id }))
}

fn delete_note_tool() -> McpTool {
  McpTool::new(
    "captains_log_delete_note",
    t!("mcp.tools.captains_log_delete_note").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { delete_note(db, args).await },
  )
  .with_args([ArgSpec::integer(
    "id",
    t!("mcp.tools.captains_log_delete_note_id").into_owned(),
  )])
}

fn field_note_value(note: &FieldNote) -> Value {
  json!({
    "created_at": note.created_at,
    "date": note.date,
    "id": note.id,
    "text": note.text,
    "updated_at": note.updated_at,
  })
}

async fn list_notes(db: Database, args: Value) -> Result<Value, ToolError> {
  let date = require_date(&args)?;
  let notes = field_notes::list_for_date(&db, &date).await.map_err(internal)?;
  let notes: Vec<Value> = notes.iter().map(field_note_value).collect();
  Ok(json!({ "date": date, "notes": notes }))
}

fn list_notes_tool() -> McpTool {
  McpTool::new(
    "captains_log_list_notes",
    t!("mcp.tools.captains_log_list_notes").into_owned(),
    Permission::Read,
    |db, args: Value| async move { list_notes(db, args).await },
  )
  .with_args([ArgSpec::string(
    "date",
    t!("mcp.tools.captains_log_list_notes_date").into_owned(),
  )])
}

fn answer_text(log: &CaptainsLog, key: AnswerKey) -> Option<&str> {
  match key {
    AnswerKey::Blocked => log.blocked().as_deref(),
    AnswerKey::Build => log.build().as_deref(),
    AnswerKey::Combat => log.combat().as_deref(),
    AnswerKey::Goal => log.goal().as_deref(),
    AnswerKey::Next => log.next().as_deref(),
    AnswerKey::Remember => log.remember().as_deref(),
    AnswerKey::Research => log.research().as_deref(),
    AnswerKey::Skill => log.skill().as_deref(),
  }
}

fn answers_value(log: Option<&CaptainsLog>) -> Value {
  let mut map = serde_json::Map::new();
  for key in AnswerKey::ALL {
    let text = log.and_then(|log| answer_text(log, key));
    map.insert(key.as_key().to_owned(), json!(text));
  }
  if let Some(log) = log {
    // `log.answers()` also holds every canonical AnswerKey id already inserted above (goal, blocked, ...), since
    // both are read from the same table; `or_insert_with` only pulls in ids from custom prompt-config questions
    // without clobbering the canonical values.
    for (question_id, value) in log.answers() {
      map.entry(question_id.clone()).or_insert_with(|| json!(value));
    }
  }
  Value::Object(map)
}

async fn combat_victims(db: &Database, date: &str) -> Result<HashMap<i64, i64>, ToolError> {
  let rows = sqlx::query_as::<_, (i64, Option<i64>)>(
    "SELECT killmail_id, victim_id FROM character_killmails \
    WHERE substr(kill_time, 1, 10) = ? AND character_id IN (SELECT id FROM owned_characters)",
  )
  .bind(date)
  .fetch_all(db.reader())
  .await
  .map_err(internal)?;
  Ok(
    rows
      .into_iter()
      .filter_map(|(killmail_id, victim_id)| victim_id.map(|victim| (killmail_id, victim)))
      .collect(),
  )
}

fn combat_value(combat: &Combat, victims: &HashMap<i64, i64>, names: &HashMap<i64, ResolvedName>) -> Value {
  let engagements: Vec<Value> = combat
    .engagements
    .iter()
    .map(|engagement| engagement_value(engagement, victims, names))
    .collect();
  json!({
    "engagements": engagements,
    "kill_count": combat.kill_count,
    "kill_value": combat.kill_value,
    "loss_count": combat.loss_count,
    "loss_value": combat.loss_value,
  })
}

fn completeness_value(completeness: &Completeness) -> Value {
  let debriefs: Vec<Value> = completeness.missing_debriefs.iter().map(loss_value).collect();
  let missing_prompts: Vec<&str> = completeness.missing_prompts.iter().map(|key| key.as_key()).collect();
  json!({
    "is_complete": completeness.is_complete(),
    "missing_debriefs": debriefs,
    "missing_prompts": missing_prompts,
  })
}

fn day_activity(rollup: &DayRollup) -> DayActivity {
  DayActivity {
    engagement_count: rollup.combat.engagements.len() as u32,
    industry_count: rollup.industry.len() as u32,
    losses: rollup
      .combat
      .engagements
      .iter()
      .filter(|engagement| !engagement.is_kill)
      .map(|engagement| LossEngagement {
        character_id: engagement.character_id,
        killmail_id: engagement.killmail_id,
      })
      .collect(),
    skill_count: rollup.skills.len() as u32,
    ..DayActivity::default()
  }
}

#[allow(clippy::too_many_arguments)]
fn day_value(
  date: &str,
  rollup: &DayRollup,
  victims: &HashMap<i64, i64>,
  names: &HashMap<i64, ResolvedName>,
  log: Option<&CaptainsLog>,
  reports: &[KillmailReport],
  event_notes: Vec<Value>,
  advancing: Vec<Value>,
) -> Value {
  json!({
    "advancing": advancing,
    "answers": answers_value(log),
    "combat": combat_value(&rollup.combat, victims, names),
    "date": date,
    "eve_date": eve_label(date),
    "event_notes": event_notes,
    "events": events_value(&rollup.events),
    "industry": industry_value(&rollup.industry, names),
    "kill_reports": reports_value(reports),
    "money": money_value(&rollup.money),
    "narrative": log.and_then(|log| log.narrative().as_deref()),
    "net_worth": rollup.net_worth.map(net_worth_value),
    "skills": skills_value(&rollup.skills, names),
  })
}

async fn describe_structure(db: Database, _args: Value) -> Result<Value, ToolError> {
  let config = captains_log::load_prompt_config(&db).await.map_err(internal)?;
  let sections: Vec<Value> = config.sections.iter().map(section_value).collect();
  Ok(json!({ "sections": sections }))
}

fn describe_structure_tool() -> McpTool {
  McpTool::new(
    "captains_log_describe_structure",
    t!("mcp.tools.captains_log_describe_structure").into_owned(),
    Permission::Read,
    |db, args: Value| async move { describe_structure(db, args).await },
  )
}

fn section_value(section: &PromptSection) -> Value {
  let questions: Vec<Value> = section.questions.iter().map(question_value).collect();
  let mut value = json!({
    "id": section.id,
    "kind": section_kind(section.kind),
    "label": resolve_label(&section.label, &section.i18n_key),
    "questions": questions,
  });
  if let Some(triggers) = section.triggers {
    value["triggers"] = triggers_value(&triggers);
  }
  value
}

fn question_value(question: &PromptQuestion) -> Value {
  json!({
    "id": question.id,
    "kind": question_kind(question.kind),
    "label": resolve_label(&question.label, &question.i18n_key),
    "links_to_objective": question.links_to_objective,
    "placeholder": question.placeholder,
    "required": question.required,
  })
}

fn section_kind(kind: PromptSectionKind) -> &'static str {
  match kind {
    PromptSectionKind::Conditional => "conditional",
    PromptSectionKind::Free => "free",
  }
}

fn question_kind(kind: PromptQuestionKind) -> &'static str {
  match kind {
    PromptQuestionKind::Text => "text",
  }
}

fn triggers_value(triggers: &PromptTriggers) -> Value {
  json!({ "build": triggers.build, "combat": triggers.combat, "skill": triggers.skill })
}

/// Empty `label` is the sentinel for "no override configured" and falls back to `i18n_key`; a deliberately
/// empty-string override is indistinguishable from unset.
fn resolve_label(label: &str, i18n_key: &str) -> String {
  if label.is_empty() {
    t!(i18n_key).into_owned()
  } else {
    label.to_owned()
  }
}

fn dry_run(args: &Value) -> bool {
  args
    .get("dry_run")
    .map(|value| {
      value
        .as_bool()
        .unwrap_or_else(|| value.as_i64().is_some_and(|flag| flag != 0))
    })
    .unwrap_or(false)
}

fn dry_run_arg() -> ArgSpec {
  ArgSpec::optional_integer("dry_run", 0, t!("mcp.tools.shared_arg_dry_run").into_owned())
}

fn engagement_value(engagement: &CombatKill, victims: &HashMap<i64, i64>, names: &HashMap<i64, ResolvedName>) -> Value {
  let victim_id = victims.get(&engagement.killmail_id).copied();
  json!({
    "character_id": engagement.character_id,
    "is_kill": engagement.is_kill,
    "kill_time": engagement.kill_time,
    "killmail_id": engagement.killmail_id,
    "ship_type_id": engagement.ship_type_id,
    "ship_type_name": name_of(names, engagement.ship_type_id),
    "system_id": engagement.system_id,
    "system_name": name_of(names, engagement.system_id),
    "value_isk": engagement.value_isk,
    "victim_id": victim_id,
    "victim_name": victim_id.and_then(|id| name_of(names, id)),
  })
}

async fn event_notes(db: &Database, character_ids: &[i64], events: &[CalendarEntry]) -> Result<Vec<Value>, ToolError> {
  let event_ids: Vec<i64> = events.iter().map(|event| event.event_id).collect();
  let mut notes = Vec::new();
  for &character_id in character_ids {
    for note in calendar_event_note::list_for_events(db, character_id, &event_ids)
      .await
      .map_err(internal)?
    {
      notes.push(json!({
        "character_id": note.character_id,
        "event_id": note.event_id,
        "note": note.note,
        "updated_at": note.updated_at,
      }));
    }
  }
  Ok(notes)
}

fn events_value(events: &[CalendarEntry]) -> Vec<Value> {
  events
    .iter()
    .map(|event| {
      json!({
        "event_id": event.event_id,
        "response": event.response,
        "timestamp": event.timestamp,
        "title": event.title,
      })
    })
    .collect()
}

fn eve_label(date: &str) -> Option<String> {
  NaiveDate::parse_from_str(date, "%Y-%m-%d")
    .ok()
    .map(|date| format!("YC {}", date.year() - YC_EPOCH_OFFSET))
}

async fn get_day(db: Database, args: Value) -> Result<Value, ToolError> {
  let date = require_date(&args)?;
  let has_activity = rollup::has_activity(&db, &date).await.map_err(internal)?;
  let log = captains_log::get(&db, &date).await.map_err(internal)?;
  if !has_activity && log.is_none() {
    return Err(ToolError::InvalidArguments(format!(
      "no captain's log or activity recorded for {date}"
    )));
  }

  let rollup = rollup::for_date(&db, &date).await.map_err(internal)?;
  let victims = combat_victims(&db, &date).await?;
  let names = resolve_names_map(&db, &name_ids(&rollup, &victims)).await?;
  let character_ids = owned_ids(&db).await?;
  let reports = killmail_report::list_for_day(&db, &character_ids, &date)
    .await
    .map_err(internal)?;
  let notes = event_notes(&db, &character_ids, &rollup.events).await?;
  let advancing = advancing_value(&db, &date).await?;

  Ok(day_value(
    &date,
    &rollup,
    &victims,
    &names,
    log.as_ref(),
    &reports,
    notes,
    advancing,
  ))
}

async fn advancing_value(db: &Database, date: &str) -> Result<Vec<Value>, ToolError> {
  let links = objective::links_for_day(db, date).await.map_err(internal)?;
  let mut seen = Vec::new();
  let mut advancing = Vec::new();
  for link in &links {
    if seen.contains(&link.objective_id) {
      continue;
    }
    seen.push(link.objective_id);
    if let Some(objective) = objective::get(db, link.objective_id).await.map_err(internal)? {
      advancing.push(json!({ "id": objective.id, "title": objective.title }));
    }
  }
  Ok(advancing)
}

fn get_day_tool() -> McpTool {
  McpTool::new(
    "captains_log_get_day",
    t!("mcp.tools.captains_log_get_day").into_owned(),
    Permission::Read,
    |db, args: Value| async move { get_day(db, args).await },
  )
  .with_args([ArgSpec::string(
    "date",
    t!("mcp.tools.captains_log_get_day_date").into_owned(),
  )])
}

/// Takes the first 10 characters, i.e. the `YYYY-MM-DD` prefix of an RFC 3339 timestamp.
fn day_of(stamp: &str) -> String {
  stamp.get(..10).unwrap_or(stamp).to_owned()
}

fn dossier_brief_value(brief: &Dossier) -> Value {
  json!({
    "character_id": brief.character_id,
    "created_at": brief.created_at,
    "near_term": brief.near_term,
    "purpose": brief.purpose,
    "updated_at": brief.updated_at,
  })
}

fn dossier_order_value(order: &DossierOrder, titles: &HashMap<i64, String>) -> Value {
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

async fn header_value(
  db: &Database,
  character_ids: &[i64],
  objectives: &[Objective],
  orders: &[DossierOrder],
  titles: &HashMap<i64, String>,
) -> Result<Value, ToolError> {
  let names = resolve_names_map(db, character_ids).await?;
  let active_objectives: Vec<Value> = objectives
    .iter()
    .filter(|objective| objective.status == "active")
    .map(objective_summary)
    .collect();
  let active_orders: Vec<Value> = orders
    .iter()
    .filter(|order| order.status == "active")
    .map(|order| dossier_order_value(order, titles))
    .collect();

  let mut dossiers = Vec::new();
  for &character_id in character_ids {
    let brief = dossier::get_brief(db, character_id).await.map_err(internal)?;
    let character_orders: Vec<Value> = orders
      .iter()
      .filter(|order| order.character_id == character_id)
      .map(|order| dossier_order_value(order, titles))
      .collect();
    dossiers.push(json!({
      "brief": brief.as_ref().map(dossier_brief_value),
      "character_id": character_id,
      "character_name": name_of(&names, character_id),
      "orders": character_orders,
    }));
  }

  Ok(json!({
    "dossiers": dossiers,
    "standing_orders": { "dossier_orders": active_orders, "objectives": active_objectives },
  }))
}

fn in_range(date: &str, from: Option<&str>, to: Option<&str>) -> bool {
  from.is_none_or(|from| date >= from) && to.is_none_or(|to| date <= to)
}

fn is_resolved(status: &str) -> bool {
  status == "cancelled" || status == "complete"
}

fn industry_value(rows: &[IndustryDelivery], names: &HashMap<i64, ResolvedName>) -> Vec<Value> {
  rows
    .iter()
    .map(|row| {
      json!({
        "character_id": row.character_id,
        "product_type_id": row.product_type_id,
        "product_type_name": row.product_type_id.and_then(|id| name_of(names, id)),
        "runs": row.runs,
      })
    })
    .collect()
}

fn internal(error: impl std::fmt::Display) -> ToolError {
  ToolError::Internal(error.to_string())
}

async fn killmail_pair_exists(db: &Database, character_id: i64, killmail_id: i64) -> Result<bool, ToolError> {
  let found = sqlx::query_scalar::<_, i64>(
    "SELECT 1 FROM character_killmails WHERE character_id = ? AND killmail_id = ? LIMIT 1",
  )
  .bind(character_id)
  .bind(killmail_id)
  .fetch_optional(db.reader())
  .await
  .map_err(internal)?;
  Ok(found.is_some())
}

async fn list_day_value(
  db: &Database,
  date: &str,
  character_ids: &[i64],
  has_activity: bool,
  has_entry: bool,
) -> Result<Value, ToolError> {
  let rollup = rollup::for_date(db, date).await.map_err(internal)?;
  let activity = day_activity(&rollup);
  let completeness = prompts::completeness_for_day(db, date, character_ids, &activity)
    .await
    .map_err(internal)?;
  Ok(json!({
    "completeness": completeness_value(&completeness),
    "date": date,
    "eve_date": eve_label(date),
    "has_activity": has_activity,
    "has_entry": has_entry,
  }))
}

async fn list_days(db: Database, args: Value) -> Result<Value, ToolError> {
  let (from, to) = parse_range(&args)?;
  let character_ids = owned_ids(&db).await?;
  let authored = captains_log::dates(&db).await.map_err(internal)?;
  let active = captains_log_rollup::active_dates(&db).await.map_err(internal)?;

  let mut days = Vec::new();
  for date in merged_dates(&authored, &active) {
    if !in_range(&date, from.as_deref(), to.as_deref()) {
      continue;
    }
    let has_activity = active.iter().any(|day| day == &date);
    let has_entry = authored.iter().any(|day| day == &date);
    days.push(list_day_value(&db, &date, &character_ids, has_activity, has_entry).await?);
  }
  Ok(json!({ "days": days }))
}

fn list_days_tool() -> McpTool {
  McpTool::new(
    "captains_log_list_days",
    t!("mcp.tools.captains_log_list_days").into_owned(),
    Permission::Read,
    |db, args: Value| async move { list_days(db, args).await },
  )
  .with_args([
    ArgSpec::optional_string("from", t!("mcp.tools.captains_log_list_days_from").into_owned()),
    ArgSpec::optional_string("to", t!("mcp.tools.captains_log_list_days_to").into_owned()),
  ])
}

fn loss_value(loss: &LossEngagement) -> Value {
  json!({ "character_id": loss.character_id, "killmail_id": loss.killmail_id })
}

async fn range(db: Database, args: Value) -> Result<Value, ToolError> {
  let (from, to) = parse_range(&args)?;
  let (page, limit) = range_pagination(&args);

  let character_ids = owned_ids(&db).await?;
  let objectives = objective::list(&db, None).await.map_err(internal)?;
  let titles: HashMap<i64, String> = objectives
    .iter()
    .map(|objective| (objective.id, objective.title.clone()))
    .collect();
  let mut orders = Vec::new();
  for &character_id in &character_ids {
    orders.extend(dossier::list_orders(&db, character_id).await.map_err(internal)?);
  }

  let mut dates = range_dates(&db, from.as_deref(), to.as_deref(), &objectives, &orders).await?;
  let (window, has_more) = paginate_vec(&mut dates, page, limit);

  let mut days = Vec::new();
  for date in &window {
    days.push(range_day_value(&db, date, &objectives, &orders, &titles).await?);
  }

  let mut result = json!({ "days": days, "has_more": has_more, "page": page });
  // The current-state snapshot (dossiers, active orders/objectives) doesn't change page to page; only send it once,
  // on the first page.
  if page == 0 {
    result["current_state"] = header_value(&db, &character_ids, &objectives, &orders, &titles).await?;
  }
  Ok(result)
}

async fn range_day_value(
  db: &Database,
  date: &str,
  objectives: &[Objective],
  orders: &[DossierOrder],
  titles: &HashMap<i64, String>,
) -> Result<Value, ToolError> {
  let log = captains_log::get(db, date).await.map_err(internal)?;
  let rollup = rollup::for_date(db, date).await.map_err(internal)?;
  let victims = combat_victims(db, date).await?;
  let names = resolve_names_map(db, &name_ids(&rollup, &victims)).await?;
  let notes: Vec<Value> = field_notes::list_for_date(db, date)
    .await
    .map_err(internal)?
    .iter()
    .map(field_note_value)
    .collect();

  Ok(json!({
    "answers": answers_value(log.as_ref()),
    "combat": combat_value(&rollup.combat, &victims, &names),
    "date": date,
    "eve_date": eve_label(date),
    "events": events_value(&rollup.events),
    "field_notes": notes,
    "industry": industry_value(&rollup.industry, &names),
    "money": money_value(&rollup.money),
    "narrative": log.as_ref().and_then(|log| log.narrative().as_deref()),
    "net_worth": rollup.net_worth.map(net_worth_value),
    "resolved_orders": resolved_orders_value(date, objectives, orders, titles),
    "skills": skills_value(&rollup.skills, &names),
  }))
}

async fn range_dates(
  db: &Database,
  from: Option<&str>,
  to: Option<&str>,
  objectives: &[Objective],
  orders: &[DossierOrder],
) -> Result<Vec<String>, ToolError> {
  let mut dates = captains_log::dates(db).await.map_err(internal)?;
  dates.extend(captains_log_rollup::active_dates(db).await.map_err(internal)?);
  dates.extend(field_notes::dates(db).await.map_err(internal)?);
  for objective in objectives {
    if let Some(stamp) = resolution_stamp(objective) {
      dates.push(day_of(stamp));
    }
  }
  for order in orders {
    if is_resolved(&order.status) {
      dates.push(day_of(&order.updated_at));
    }
  }
  dates.retain(|date| in_range(date, from, to));
  dates.sort_unstable();
  dates.dedup();
  dates.reverse();
  Ok(dates)
}

/// Defaults `limit` to `DAYS_PER_PAGE` instead of `pagination`'s usual default, since assembling a day requires an
/// uncached per-day rollup and must stay cheap.
fn range_pagination(args: &Value) -> (i64, i64) {
  let (page, limit) = pagination(args);
  let limit = if args.get("limit").is_some() {
    limit
  } else {
    DAYS_PER_PAGE
  };
  (page, limit)
}

fn range_tool() -> McpTool {
  McpTool::new(
    "captains_log_range",
    t!("mcp.tools.captains_log_range").into_owned(),
    Permission::Read,
    |db, args: Value| async move { range(db, args).await },
  )
  .with_args([
    ArgSpec::optional_string("from", t!("mcp.tools.captains_log_range_from").into_owned()),
    ArgSpec::optional_string("to", t!("mcp.tools.captains_log_range_to").into_owned()),
    ArgSpec::optional_integer("page", 0, t!("mcp.tools.captains_log_range_page").into_owned()),
    ArgSpec::optional_integer(
      "limit",
      DAYS_PER_PAGE,
      t!("mcp.tools.captains_log_range_limit").into_owned(),
    ),
  ])
}

/// Dossier orders have no dedicated resolution timestamp; a cancelled/completed order is bucketed by `updated_at`
/// (its last edit), unlike objectives, which record dedicated `cancelled_at`/`completed_at` stamps.
fn resolved_orders_value(
  date: &str,
  objectives: &[Objective],
  orders: &[DossierOrder],
  titles: &HashMap<i64, String>,
) -> Value {
  let objectives_cancelled: Vec<Value> = objectives
    .iter()
    .filter(|objective| objective.status == "cancelled" && stamped_on(objective.cancelled_at.as_deref(), date))
    .map(objective_summary)
    .collect();
  let objectives_completed: Vec<Value> = objectives
    .iter()
    .filter(|objective| objective.status == "complete" && stamped_on(objective.completed_at.as_deref(), date))
    .map(objective_summary)
    .collect();
  let dossier_orders_cancelled: Vec<Value> = orders
    .iter()
    .filter(|order| order.status == "cancelled" && day_of(&order.updated_at) == date)
    .map(|order| dossier_order_value(order, titles))
    .collect();
  let dossier_orders_completed: Vec<Value> = orders
    .iter()
    .filter(|order| order.status == "complete" && day_of(&order.updated_at) == date)
    .map(|order| dossier_order_value(order, titles))
    .collect();
  json!({
    "dossier_orders_cancelled": dossier_orders_cancelled,
    "dossier_orders_completed": dossier_orders_completed,
    "objectives_cancelled": objectives_cancelled,
    "objectives_completed": objectives_completed,
  })
}

fn merged_dates(authored: &[String], active: &[String]) -> Vec<String> {
  let mut all: Vec<String> = authored.iter().chain(active).cloned().collect();
  all.sort_unstable();
  all.dedup();
  all.reverse();
  all
}

fn money_value(money: &DayMoney) -> Value {
  json!({ "earned": money.earned, "net": money.net(), "spent": money.spent })
}

fn name_ids(rollup: &DayRollup, victims: &HashMap<i64, i64>) -> Vec<i64> {
  let mut ids = Vec::new();
  for engagement in &rollup.combat.engagements {
    ids.push(engagement.ship_type_id);
    ids.push(engagement.system_id);
  }
  ids.extend(victims.values().copied());
  for delivery in &rollup.industry {
    if let Some(product_type_id) = delivery.product_type_id {
      ids.push(product_type_id);
    }
  }
  ids.extend(rollup.skills.iter().map(|skill| skill.skill_id));
  ids
}

fn name_of(names: &HashMap<i64, ResolvedName>, id: i64) -> Option<&str> {
  names.get(&id).map(|resolved| resolved.name.as_str())
}

fn net_worth_value(delta: NetWorthDelta) -> Value {
  json!({ "isk": delta.isk, "percent": delta.percent })
}

/// MCP tool handlers get no ESI client, so this stands in for a live lookup: `names::resolve` only enriches from
/// locally cached/SDE data here, never from ESI.
async fn no_esi(_ids: Vec<i64>) -> Result<HashMap<i64, NameRecord>, ClientError> {
  Ok(HashMap::new())
}

fn objective_summary(objective: &Objective) -> Value {
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

async fn owned_ids(db: &Database) -> Result<Vec<i64>, ToolError> {
  let characters = character::all_owned(db).await.map_err(internal)?;
  Ok(characters.iter().map(|character| character.id()).collect())
}

fn optional_date(args: &Value, key: &str) -> Result<Option<String>, ToolError> {
  match args.get(key).and_then(Value::as_str) {
    None => Ok(None),
    Some(text) => {
      validate_date(key, text)?;
      Ok(Some(text.to_owned()))
    }
  }
}

fn parse_range(args: &Value) -> Result<(Option<String>, Option<String>), ToolError> {
  let from = optional_date(args, "from")?;
  let to = optional_date(args, "to")?;
  if let (Some(from), Some(to)) = (&from, &to)
    && from > to
  {
    return Err(ToolError::InvalidArguments(format!(
      "`from` ({from}) must not be after `to` ({to})"
    )));
  }
  Ok((from, to))
}

fn reports_value(reports: &[KillmailReport]) -> Vec<Value> {
  reports
    .iter()
    .map(|report| {
      json!({
        "character_id": report.character_id(),
        "created_at": report.created_at(),
        "different": report.different(),
        "happened": report.happened(),
        "killmail_id": report.killmail_id(),
        "outcome": report.outcome(),
        "takeaway": report.takeaway(),
        "updated_at": report.updated_at(),
      })
    })
    .collect()
}

fn require_date(args: &Value) -> Result<String, ToolError> {
  let text = require_str(args, "date")?;
  validate_date("date", text)?;
  Ok(text.to_owned())
}

fn resolution_stamp(objective: &Objective) -> Option<&str> {
  match objective.status.as_str() {
    "cancelled" => objective.cancelled_at.as_deref(),
    "complete" => objective.completed_at.as_deref(),
    _ => None,
  }
}

async fn resolve_names_map(db: &Database, ids: &[i64]) -> Result<HashMap<i64, ResolvedName>, ToolError> {
  names::resolve(db, ids, no_esi).await.map_err(internal)
}

async fn set_answer(db: Database, args: Value) -> Result<Value, ToolError> {
  let date = require_date(&args)?;
  let question_id = validate_question_id(&db, &args).await?;
  let text = require_str(&args, "text")?.to_owned();
  if dry_run(&args) {
    return Ok(json!({ "date": date, "dry_run": true, "prompt": question_id, "text": text }));
  }

  captains_log::upsert_answer(&db, &date, question_id.as_str(), Some(&text))
    .await
    .map_err(internal)?;
  Ok(json!({ "date": date, "prompt": question_id, "text": text }))
}

fn set_answer_tool() -> McpTool {
  McpTool::new(
    "captains_log_set_answer",
    t!("mcp.tools.captains_log_set_answer").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { set_answer(db, args).await },
  )
  .with_args([
    ArgSpec::string("date", t!("mcp.tools.captains_log_set_answer_date").into_owned()),
    ArgSpec::string("prompt", t!("mcp.tools.captains_log_set_answer_prompt").into_owned()),
    ArgSpec::string("text", t!("mcp.tools.captains_log_set_answer_text").into_owned()),
    dry_run_arg(),
  ])
}

async fn set_kill_report(db: Database, args: Value) -> Result<Value, ToolError> {
  let character_id = require_i64(&args, "character_id")?;
  let killmail_id = require_i64(&args, "killmail_id")?;
  let outcome = validate_outcome(&args)?;
  let happened = require_str(&args, "happened")?.to_owned();
  let different = args.get("different").and_then(Value::as_str).map(str::to_owned);
  let takeaway = args.get("takeaway").and_then(Value::as_str).map(str::to_owned);
  // Reject before the dry_run check or any write: a debrief must reference a killmail this character actually
  // recorded, not just a well-formed id pair.
  if !killmail_pair_exists(&db, character_id, killmail_id).await? {
    return Err(ToolError::InvalidArguments(format!(
      "no killmail {killmail_id} recorded for character {character_id}"
    )));
  }

  let body = json!({
    "character_id": character_id,
    "different": different,
    "happened": happened,
    "killmail_id": killmail_id,
    "outcome": outcome,
    "takeaway": takeaway,
  });
  if dry_run(&args) {
    let mut preview = body;
    preview["dry_run"] = json!(true);
    return Ok(preview);
  }

  let input = killmail_report::ReportInput {
    different,
    happened,
    outcome,
    takeaway,
  };
  killmail_report::upsert(&db, character_id, killmail_id, &input)
    .await
    .map_err(internal)?;
  Ok(body)
}

fn set_kill_report_tool() -> McpTool {
  McpTool::new(
    "captains_log_set_kill_report",
    t!("mcp.tools.captains_log_set_kill_report").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { set_kill_report(db, args).await },
  )
  .with_args([
    ArgSpec::integer(
      "character_id",
      t!("mcp.tools.captains_log_set_kill_report_character_id").into_owned(),
    ),
    ArgSpec::integer(
      "killmail_id",
      t!("mcp.tools.captains_log_set_kill_report_killmail_id").into_owned(),
    ),
    ArgSpec::string(
      "outcome",
      t!("mcp.tools.captains_log_set_kill_report_outcome").into_owned(),
    ),
    ArgSpec::string(
      "happened",
      t!("mcp.tools.captains_log_set_kill_report_happened").into_owned(),
    ),
    ArgSpec::optional_string(
      "different",
      t!("mcp.tools.captains_log_set_kill_report_different").into_owned(),
    ),
    ArgSpec::optional_string(
      "takeaway",
      t!("mcp.tools.captains_log_set_kill_report_takeaway").into_owned(),
    ),
    dry_run_arg(),
  ])
}

async fn set_narrative(db: Database, args: Value) -> Result<Value, ToolError> {
  let date = require_date(&args)?;
  let text = require_str(&args, "text")?.to_owned();
  if dry_run(&args) {
    return Ok(json!({ "date": date, "dry_run": true, "narrative": text }));
  }

  captains_log::upsert_narrative(&db, &date, Some(&text))
    .await
    .map_err(internal)?;
  Ok(json!({ "date": date, "narrative": text }))
}

fn set_narrative_tool() -> McpTool {
  McpTool::new(
    "captains_log_set_narrative",
    t!("mcp.tools.captains_log_set_narrative").into_owned(),
    Permission::LocalWrite,
    |db, args: Value| async move { set_narrative(db, args).await },
  )
  .with_args([
    ArgSpec::string("date", t!("mcp.tools.captains_log_set_narrative_date").into_owned()),
    ArgSpec::string("text", t!("mcp.tools.captains_log_set_narrative_text").into_owned()),
    dry_run_arg(),
  ])
}

fn skills_value(skills: &[SkillCompletion], names: &HashMap<i64, ResolvedName>) -> Vec<Value> {
  skills
    .iter()
    .map(|skill| {
      json!({
        "character_id": skill.character_id,
        "completed_at": skill.completed_at,
        "level": skill.level,
        "skill_id": skill.skill_id,
        "skill_name": name_of(names, skill.skill_id),
      })
    })
    .collect()
}

fn stamped_on(stamp: Option<&str>, date: &str) -> bool {
  stamp.is_some_and(|stamp| day_of(stamp) == date)
}

fn validate_date(key: &str, text: &str) -> Result<(), ToolError> {
  NaiveDate::parse_from_str(text, "%Y-%m-%d")
    .map_err(|_| ToolError::InvalidArguments(format!("`{key}` must be a YYYY-MM-DD date, but received `{text}`")))?;
  Ok(())
}

fn validate_outcome(args: &Value) -> Result<String, ToolError> {
  let outcome = require_str(args, "outcome")?;
  match outcome {
    "clean" | "costly" | "learning" => Ok(outcome.to_owned()),
    _ => Err(ToolError::InvalidArguments(format!(
      "`outcome` must be one of: clean, costly, learning, but received `{outcome}`"
    ))),
  }
}

/// The 8 default `AnswerKey` ids are always accepted, even if a custom `PromptConfig` no longer lists them as
/// questions; only unrecognized ids fall through to a config lookup.
async fn validate_question_id(db: &Database, args: &Value) -> Result<String, ToolError> {
  let key = require_str(args, "prompt")?.to_owned();
  if AnswerKey::from_key(&key).is_some() {
    return Ok(key);
  }
  let config = captains_log::load_prompt_config(db).await.map_err(internal)?;
  if config
    .sections
    .iter()
    .flat_map(|section| &section.questions)
    .any(|question| question.id == key)
  {
    return Ok(key);
  }
  let defaults: Vec<&str> = AnswerKey::ALL.iter().map(|key| key.as_key()).collect();
  Err(ToolError::InvalidArguments(format!(
    "`prompt` must be a default prompt ({}) or a configured question id, but received `{key}`",
    defaults.join(", ")
  )))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, PromptConfig, Race},
    repo::{character, infra, killmail_report::ReportInput, skill_completion},
  };

  const ENEMY: i64 = 90_000_071;
  const OTHER: i64 = 90_000_002;
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

  async fn seed_owned(db: &Database, id: i64, name: &str) {
    seed_character(db, id, name).await;
    infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
      .await
      .unwrap();
  }

  async fn seed_type(db: &Database, id: i64, name: &str) {
    sqlx::query("INSERT OR IGNORE INTO item_categories (id, name, published) VALUES (6, 'Ship', 1)")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query("INSERT OR IGNORE INTO item_groups (id, category_id, name, published) VALUES (25, 6, 'Frigate', 1)")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query("INSERT INTO item_types (id, group_id, description, name, published) VALUES (?, 25, '', ?, 1)")
      .bind(id)
      .bind(name)
      .execute(db.writer())
      .await
      .unwrap();
  }

  async fn seed_system(db: &Database, id: i64, name: &str) {
    sqlx::query("INSERT OR IGNORE INTO regions (id, description, name) VALUES (1, NULL, 'Region')")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query(
      "INSERT OR IGNORE INTO constellations (id, region_id, name, position_x, position_y, position_z) \
        VALUES (1, 1, 'Constellation', 0, 0, 0)",
    )
    .execute(db.writer())
    .await
    .unwrap();
    sqlx::query(
      "INSERT INTO solar_systems (constellation_id, id, name, position_x, position_y, position_z, security_class, \
        security_status, star_id) VALUES (1, ?, ?, 0, 0, 0, NULL, 0, NULL)",
    )
    .bind(id)
    .bind(name)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_journal(db: &Database, id: i64, character_id: i64, date: &str, amount: f64) {
    sqlx::query(
      "INSERT INTO character_wallet_journal (id, character_id, date, description, ref_type, amount) \
      VALUES (?, ?, ?, '', 'player_trading', ?)",
    )
    .bind(id)
    .bind(character_id)
    .bind(date)
    .bind(amount)
    .execute(db.writer())
    .await
    .unwrap();
  }

  #[allow(clippy::too_many_arguments)]
  async fn seed_kill(
    db: &Database,
    character_id: i64,
    killmail_id: i64,
    is_kill: bool,
    victim_id: i64,
    ship_type_id: i64,
    system_id: i64,
    kill_time: &str,
    value: f64,
  ) {
    sqlx::query(
      "INSERT INTO character_killmails \
        (character_id, killmail_id, kill_hash, is_kill, ship_type_id, victim_id, system_id, value_isk, kill_time, \
          synced_at) \
      VALUES (?, ?, 'hash', ?, ?, ?, ?, ?, ?, '2026-07-05T00:00:00Z')",
    )
    .bind(character_id)
    .bind(killmail_id)
    .bind(is_kill)
    .bind(ship_type_id)
    .bind(victim_id)
    .bind(system_id)
    .bind(value)
    .bind(kill_time)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_industry(db: &Database, job_id: i64, character_id: i64, product: i64, runs: i64, completed: &str) {
    sqlx::query(
      "INSERT INTO character_industry_jobs \
        (job_id, character_id, activity_id, blueprint_id, blueprint_location_id, blueprint_type_id, \
          duration, end_date, facility_id, installer_id, output_location_id, product_type_id, runs, \
          start_date, status, completed_date) \
      VALUES (?, ?, 1, 1, 1, 1, 0, ?, 1, ?, 1, ?, ?, '2026-07-05T00:00:00Z', 'delivered', ?)",
    )
    .bind(job_id)
    .bind(character_id)
    .bind(completed)
    .bind(character_id)
    .bind(product)
    .bind(runs)
    .bind(completed)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_calendar(db: &Database, character_id: i64, event_id: i64, timestamp: &str, title: &str) {
    sqlx::query(
      "INSERT INTO character_calendar \
        (character_id, event_id, timestamp, owner_id, owner_name, owner_type, response, title, fetched_at) \
      VALUES (?, ?, ?, 1, 'Corp', 'corporation', 'accepted', ?, '2026-07-05T00:00:00Z')",
    )
    .bind(character_id)
    .bind(event_id)
    .bind(timestamp)
    .bind(title)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_busy_day(db: &Database) {
    seed_owned(db, PILOT, "Pilot One").await;
    seed_owned(db, OTHER, "Pilot Two").await;
    seed_character(db, ENEMY, "Bad Guy").await;
    seed_type(db, 670, "Capsule").await;
    seed_type(db, 22_544, "Hulk").await;
    seed_type(db, 3_300, "Gunnery").await;
    seed_system(db, 30_000_142, "Jita").await;
    seed_journal(db, 1, PILOT, "2026-07-05T10:00:00Z", 1_000.0).await;
    seed_journal(db, 2, OTHER, "2026-07-05T11:00:00Z", -300.0).await;
    seed_kill(
      db,
      PILOT,
      100,
      true,
      ENEMY,
      670,
      30_000_142,
      "2026-07-05T21:00:00Z",
      612.0,
    )
    .await;
    seed_kill(
      db,
      OTHER,
      101,
      false,
      OTHER,
      670,
      30_000_142,
      "2026-07-05T20:00:00Z",
      132.0,
    )
    .await;
    seed_industry(db, 1, PILOT, 22_544, 3, "2026-07-05T18:00:00Z").await;
    skill_completion::insert_if_absent(db, PILOT, 3_300, 5, "2026-07-05T08:00:00Z")
      .await
      .unwrap();
    seed_calendar(db, PILOT, 5, "2026-07-05T20:00:00Z", "Tama roam").await;
    calendar_event_note::upsert(db, PILOT, 5, "Bring the logi")
      .await
      .unwrap();
    captains_log::upsert_narrative(db, "2026-07-05", Some("Clean roam, one kill, lost the Hulk hauler."))
      .await
      .unwrap();
    captains_log::upsert_answer(db, "2026-07-05", AnswerKey::Goal, Some("Spin up the barge line."))
      .await
      .unwrap();
    killmail_report::upsert(
      db,
      OTHER,
      101,
      &ReportInput {
        different: Some("Aligned out earlier.".to_owned()),
        happened: "Warped in too hot.".to_owned(),
        outcome: "learning".to_owned(),
        takeaway: Some("Fit a warp core stabilizer.".to_owned()),
      },
    )
    .await
    .unwrap();
  }

  async fn seed_custom_question(db: &Database, id: &str, label: &str, required: bool) {
    let mut config = PromptConfig::default();
    config.sections[0].questions.push(PromptQuestion {
      id: id.to_owned(),
      kind: PromptQuestionKind::Text,
      label: label.to_owned(),
      i18n_key: String::new(),
      placeholder: String::new(),
      required,
      links_to_objective: false,
    });
    captains_log::save_prompt_config(db, &config).await.unwrap();
  }

  mod list_days {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_lists_active_and_authored_days_newest_first() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      seed_journal(&db, 1, PILOT, "2026-07-05T10:00:00Z", 100.0).await;
      captains_log::upsert_narrative(&db, "2026-07-04", Some("Quiet day."))
        .await
        .unwrap();

      let value = list_days(db, json!({})).await.unwrap();
      let days = value["days"].as_array().unwrap();

      assert_eq!(days.len(), 2);
      assert_eq!(days[0]["date"], "2026-07-05");
      assert_eq!(days[0]["eve_date"], "YC 128");
      assert_eq!(days[0]["has_activity"], true);
      assert_eq!(days[0]["has_entry"], false);
      assert_eq!(days[1]["date"], "2026-07-04");
      assert_eq!(days[1]["has_activity"], false);
      assert_eq!(days[1]["has_entry"], true);
    }

    #[tokio::test]
    async fn it_flags_an_undebriefed_loss_and_missing_goal() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      seed_kill(
        &db,
        PILOT,
        101,
        false,
        PILOT,
        670,
        30_000_142,
        "2026-07-05T20:00:00Z",
        132.0,
      )
      .await;

      let value = list_days(db, json!({})).await.unwrap();
      let completeness = &value["days"][0]["completeness"];

      assert_eq!(completeness["is_complete"], false);
      let debriefs = completeness["missing_debriefs"].as_array().unwrap();
      assert_eq!(debriefs.len(), 1);
      assert_eq!(debriefs[0]["killmail_id"], 101);
      assert!(
        completeness["missing_prompts"]
          .as_array()
          .unwrap()
          .iter()
          .any(|key| key == "goal")
      );
    }

    #[tokio::test]
    async fn it_reports_a_fully_debriefed_day_as_complete() {
      let db = store::open_test().await.unwrap();
      seed_busy_day(&db).await;

      let value = list_days(db, json!({})).await.unwrap();

      assert_eq!(value["days"][0]["completeness"]["is_complete"], true);
    }

    #[tokio::test]
    async fn it_filters_to_the_requested_date_range() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      seed_journal(&db, 1, PILOT, "2026-07-03T10:00:00Z", 1.0).await;
      seed_journal(&db, 2, PILOT, "2026-07-05T10:00:00Z", 1.0).await;
      seed_journal(&db, 3, PILOT, "2026-07-07T10:00:00Z", 1.0).await;

      let value = list_days(db, json!({ "from": "2026-07-04", "to": "2026-07-06" }))
        .await
        .unwrap();
      let dates: Vec<&str> = value["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|day| day["date"].as_str().unwrap())
        .collect();

      assert_eq!(dates, vec!["2026-07-05"]);
    }

    #[tokio::test]
    async fn it_rejects_an_inverted_range() {
      let db = store::open_test().await.unwrap();

      let outcome = list_days(db, json!({ "from": "2026-07-06", "to": "2026-07-01" })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_rejects_a_malformed_range_bound() {
      let db = store::open_test().await.unwrap();

      let outcome = list_days(db, json!({ "from": "2026/07/06" })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod get_day {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_assembles_the_full_day_payload_with_enriched_names() {
      let db = store::open_test().await.unwrap();
      seed_busy_day(&db).await;

      let value = get_day(db, json!({ "date": "2026-07-05" })).await.unwrap();

      assert_eq!(value["date"], "2026-07-05");
      assert_eq!(value["eve_date"], "YC 128");
      assert_eq!(value["money"]["net"], 700.0);
      assert_eq!(value["narrative"], "Clean roam, one kill, lost the Hulk hauler.");
      assert_eq!(value["answers"]["goal"], "Spin up the barge line.");

      let combat = &value["combat"];
      assert_eq!(combat["kill_count"], 1);
      assert_eq!(combat["loss_count"], 1);
      let kill = combat["engagements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|engagement| engagement["is_kill"] == true)
        .unwrap();
      assert_eq!(kill["ship_type_name"], "Capsule");
      assert_eq!(kill["system_name"], "Jita");
      assert_eq!(kill["victim_id"], ENEMY);
      assert_eq!(kill["victim_name"], "Bad Guy");
    }

    #[tokio::test]
    async fn it_resolves_industry_skill_and_report_details() {
      let db = store::open_test().await.unwrap();
      seed_busy_day(&db).await;

      let value = get_day(db, json!({ "date": "2026-07-05" })).await.unwrap();

      assert_eq!(value["industry"][0]["product_type_name"], "Hulk");
      assert_eq!(value["skills"][0]["skill_name"], "Gunnery");
      assert_eq!(value["events"][0]["title"], "Tama roam");
      assert_eq!(value["event_notes"][0]["note"], "Bring the logi");

      let report = &value["kill_reports"][0];
      assert_eq!(report["killmail_id"], 101);
      assert_eq!(report["outcome"], "learning");
      assert_eq!(report["takeaway"], "Fit a warp core stabilizer.");
    }

    #[tokio::test]
    async fn it_serves_an_authored_day_without_activity() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      captains_log::upsert_answer(&db, "2026-07-05", AnswerKey::Goal, Some("Rest and reship."))
        .await
        .unwrap();

      let value = get_day(db, json!({ "date": "2026-07-05" })).await.unwrap();

      assert_eq!(value["answers"]["goal"], "Rest and reship.");
      assert_eq!(value["combat"]["kill_count"], 0);
    }

    #[tokio::test]
    async fn it_surfaces_a_custom_answer_alongside_the_default_slots() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      seed_custom_question(&db, "mood", "Mood", false).await;
      set_answer(
        db.clone(),
        json!({ "date": "2026-07-05", "prompt": "mood", "text": "Focused." }),
      )
      .await
      .unwrap();

      let value = get_day(db, json!({ "date": "2026-07-05" })).await.unwrap();

      assert_eq!(value["answers"]["mood"], "Focused.");
      assert!(value["answers"].as_object().unwrap().contains_key("goal"));
    }

    #[tokio::test]
    async fn it_surfaces_the_objectives_a_day_advances() {
      use crate::store::{
        model::{LinkSource, NewObjective},
        repo::objective,
      };

      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      captains_log::upsert_answer(&db, "2026-07-05", AnswerKey::Goal, Some("Spin up the barge line."))
        .await
        .unwrap();
      let created = objective::create(
        &db,
        &NewObjective {
          accent: "#FF8800".to_owned(),
          horizon: None,
          target: None,
          title: "Establish the barge line".to_owned(),
          why: None,
        },
      )
      .await
      .unwrap();
      objective::set_link(
        &db,
        created.id,
        "2026-07-05",
        &LinkSource::LogAnswer {
          question_id: "goal".to_owned(),
        },
      )
      .await
      .unwrap();

      let value = get_day(db, json!({ "date": "2026-07-05" })).await.unwrap();

      let advancing = value["advancing"].as_array().unwrap();
      assert_eq!(advancing.len(), 1);
      assert_eq!(advancing[0]["id"], created.id);
      assert_eq!(advancing[0]["title"], "Establish the barge line");
    }

    #[tokio::test]
    async fn it_errors_on_a_day_with_neither_log_nor_activity() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;

      let outcome = get_day(db, json!({ "date": "2026-07-05" })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_rejects_a_malformed_date() {
      let db = store::open_test().await.unwrap();

      let outcome = get_day(db, json!({ "date": "not-a-date" })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_requires_the_date_argument() {
      let db = store::open_test().await.unwrap();

      let outcome = get_day(db, json!({})).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod set_narrative {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_the_day_and_stores_the_narrative() {
      let db = store::open_test().await.unwrap();

      let value = set_narrative(db.clone(), json!({ "date": "2026-07-05", "text": "Clean roam." }))
        .await
        .unwrap();

      assert_eq!(value["narrative"], "Clean roam.");
      let row = captains_log::get(&db, "2026-07-05").await.unwrap().unwrap();
      assert_eq!(row.narrative().as_deref(), Some("Clean roam."));
    }

    #[tokio::test]
    async fn it_requires_the_text_argument() {
      let db = store::open_test().await.unwrap();

      let outcome = set_narrative(db, json!({ "date": "2026-07-05" })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_does_not_write_on_a_dry_run() {
      let db = store::open_test().await.unwrap();

      let value = set_narrative(
        db.clone(),
        json!({ "date": "2026-07-05", "text": "Clean roam.", "dry_run": true }),
      )
      .await
      .unwrap();

      assert_eq!(value["dry_run"], true);
      assert_eq!(captains_log::get(&db, "2026-07-05").await.unwrap(), None);
    }
  }

  mod set_answer {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_the_day_and_stores_the_answer() {
      let db = store::open_test().await.unwrap();

      let value = set_answer(
        db.clone(),
        json!({ "date": "2026-07-05", "prompt": "goal", "text": "Spin up the barge line." }),
      )
      .await
      .unwrap();

      assert_eq!(value["prompt"], "goal");
      let row = captains_log::get(&db, "2026-07-05").await.unwrap().unwrap();
      assert_eq!(row.goal().as_deref(), Some("Spin up the barge line."));
    }

    #[tokio::test]
    async fn it_rejects_an_unknown_prompt_key() {
      let db = store::open_test().await.unwrap();

      let outcome = set_answer(db, json!({ "date": "2026-07-05", "prompt": "narrative", "text": "x" })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_does_not_write_on_a_dry_run() {
      let db = store::open_test().await.unwrap();

      let value = set_answer(
        db.clone(),
        json!({ "date": "2026-07-05", "prompt": "goal", "text": "x", "dry_run": true }),
      )
      .await
      .unwrap();

      assert_eq!(value["dry_run"], true);
      assert_eq!(captains_log::get(&db, "2026-07-05").await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_writes_and_reads_a_configured_custom_question() {
      let db = store::open_test().await.unwrap();
      seed_custom_question(&db, "mood", "How did today feel?", false).await;

      let value = set_answer(
        db.clone(),
        json!({ "date": "2026-07-05", "prompt": "mood", "text": "Focused." }),
      )
      .await
      .unwrap();

      assert_eq!(value["prompt"], "mood");
      let row = captains_log::get(&db, "2026-07-05").await.unwrap().unwrap();
      assert_eq!(row.answers().get("mood").map(String::as_str), Some("Focused."));
    }

    #[tokio::test]
    async fn it_rejects_a_custom_id_absent_from_the_config() {
      let db = store::open_test().await.unwrap();

      let outcome = set_answer(db, json!({ "date": "2026-07-05", "prompt": "mood", "text": "x" })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod describe_structure {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_surfaces_the_default_sections_and_questions() {
      let db = store::open_test().await.unwrap();

      let value = describe_structure(db, json!({})).await.unwrap();

      let sections = value["sections"].as_array().unwrap();
      let ids: Vec<&str> = sections.iter().map(|section| section["id"].as_str().unwrap()).collect();
      assert_eq!(ids, vec!["core", "conditional", "forward"]);

      let goal = sections[0]["questions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|question| question["id"] == "goal")
        .unwrap();
      assert_eq!(goal["required"], true);
      assert_eq!(goal["kind"], "text");
      assert!(!goal["label"].as_str().unwrap().is_empty());

      let conditional = sections.iter().find(|section| section["id"] == "conditional").unwrap();
      assert_eq!(conditional["kind"], "conditional");
      assert_eq!(conditional["triggers"]["combat"], true);
    }

    #[tokio::test]
    async fn it_reflects_a_saved_custom_question_with_its_literal_label() {
      let db = store::open_test().await.unwrap();
      seed_custom_question(&db, "mood", "How did today feel?", true).await;

      let value = describe_structure(db, json!({})).await.unwrap();

      let core = value["sections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|section| section["id"] == "core")
        .unwrap()
        .clone();
      let mood = core["questions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|question| question["id"] == "mood")
        .unwrap();
      assert_eq!(mood["label"], "How did today feel?");
      assert_eq!(mood["required"], true);
    }

    #[tokio::test]
    async fn it_exposes_a_question_flagged_to_link_an_objective() {
      let db = store::open_test().await.unwrap();
      let mut config = PromptConfig::default();
      config.sections[0].questions.push(PromptQuestion {
        id: "mission".to_owned(),
        kind: PromptQuestionKind::Text,
        label: "Mission".to_owned(),
        i18n_key: String::new(),
        placeholder: String::new(),
        required: false,
        links_to_objective: true,
      });
      captains_log::save_prompt_config(&db, &config).await.unwrap();

      let value = describe_structure(db, json!({})).await.unwrap();

      let mission = value["sections"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|section| section["questions"].as_array().unwrap())
        .find(|question| question["id"] == "mission")
        .unwrap();
      assert_eq!(mission["links_to_objective"], true);
    }
  }

  mod set_kill_report {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_loss(db: &Database) {
      seed_owned(db, PILOT, "Pilot One").await;
      seed_kill(
        db,
        PILOT,
        101,
        false,
        PILOT,
        670,
        30_000_142,
        "2026-07-05T20:00:00Z",
        132.0,
      )
      .await;
    }

    #[tokio::test]
    async fn it_files_a_debrief_for_a_recorded_loss() {
      let db = store::open_test().await.unwrap();
      seed_loss(&db).await;

      let value = set_kill_report(
        db.clone(),
        json!({
          "character_id": PILOT,
          "killmail_id": 101,
          "outcome": "learning",
          "happened": "Warped in too hot.",
          "takeaway": "Fit a stab.",
        }),
      )
      .await
      .unwrap();

      assert_eq!(value["outcome"], "learning");
      let report = killmail_report::get(&db, PILOT, 101).await.unwrap().unwrap();
      assert_eq!(report.outcome(), "learning");
      assert_eq!(report.takeaway().as_deref(), Some("Fit a stab."));
    }

    #[tokio::test]
    async fn it_rejects_an_unknown_outcome() {
      let db = store::open_test().await.unwrap();
      seed_loss(&db).await;

      let outcome = set_kill_report(
        db,
        json!({ "character_id": PILOT, "killmail_id": 101, "outcome": "great", "happened": "x" }),
      )
      .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_requires_the_happened_narrative() {
      let db = store::open_test().await.unwrap();
      seed_loss(&db).await;

      let outcome = set_kill_report(
        db,
        json!({ "character_id": PILOT, "killmail_id": 101, "outcome": "clean" }),
      )
      .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_rejects_a_killmail_the_character_never_flew() {
      let db = store::open_test().await.unwrap();
      seed_loss(&db).await;

      let outcome = set_kill_report(
        db,
        json!({ "character_id": PILOT, "killmail_id": 999, "outcome": "clean", "happened": "x" }),
      )
      .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_does_not_write_on_a_dry_run() {
      let db = store::open_test().await.unwrap();
      seed_loss(&db).await;

      let value = set_kill_report(
        db.clone(),
        json!({
          "character_id": PILOT,
          "killmail_id": 101,
          "outcome": "clean",
          "happened": "x",
          "dry_run": true,
        }),
      )
      .await
      .unwrap();

      assert_eq!(value["dry_run"], true);
      assert_eq!(killmail_report::get(&db, PILOT, 101).await.unwrap(), None);
    }
  }

  mod field_notes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_adds_lists_and_deletes_notes() {
      let db = store::open_test().await.unwrap();

      let added = add_note(db.clone(), json!({ "date": "2026-07-05", "text": "Cyno in Tama" }))
        .await
        .unwrap();
      let id = added["id"].as_i64().unwrap();
      assert_eq!(added["date"], "2026-07-05");
      assert_eq!(added["text"], "Cyno in Tama");

      add_note(db.clone(), json!({ "date": "2026-07-05", "text": "Second note" }))
        .await
        .unwrap();

      let listed = list_notes(db.clone(), json!({ "date": "2026-07-05" })).await.unwrap();
      let notes = listed["notes"].as_array().unwrap();
      assert_eq!(notes.len(), 2);
      assert_eq!(notes[0]["text"], "Second note", "notes list newest first");

      delete_note(db.clone(), json!({ "id": id })).await.unwrap();
      let after = list_notes(db.clone(), json!({ "date": "2026-07-05" })).await.unwrap();
      assert_eq!(after["notes"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_rejects_deleting_a_missing_note() {
      let db = store::open_test().await.unwrap();

      let outcome = delete_note(db, json!({ "id": 999 })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_rejects_a_malformed_date() {
      let db = store::open_test().await.unwrap();

      let outcome = add_note(db, json!({ "date": "nope", "text": "x" })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod session {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{config::McpPerms, services::mcp::registry};

    #[tokio::test]
    async fn it_reads_and_writes_the_log_through_the_registry() {
      let db = store::open_test().await.unwrap();
      seed_busy_day(&db).await;
      let registry = registry();
      let perms = McpPerms::default();

      let listed = registry
        .dispatch("captains_log_list_days", &perms, db.clone(), json!({}))
        .await
        .unwrap();
      assert_eq!(listed["days"][0]["date"], "2026-07-05");

      let day = registry
        .dispatch(
          "captains_log_get_day",
          &perms,
          db.clone(),
          json!({ "date": "2026-07-05" }),
        )
        .await
        .unwrap();
      assert_eq!(day["combat"]["kill_count"], 1);

      registry
        .dispatch(
          "captains_log_set_narrative",
          &perms,
          db.clone(),
          json!({ "date": "2026-07-08", "text": "Authored by the agent." }),
        )
        .await
        .unwrap();
      registry
        .dispatch(
          "captains_log_set_answer",
          &perms,
          db.clone(),
          json!({ "date": "2026-07-08", "prompt": "goal", "text": "Undock more." }),
        )
        .await
        .unwrap();
      registry
        .dispatch(
          "captains_log_set_kill_report",
          &perms,
          db.clone(),
          json!({ "character_id": PILOT, "killmail_id": 100, "outcome": "clean", "happened": "Clean tackle." }),
        )
        .await
        .unwrap();

      let authored = registry
        .dispatch(
          "captains_log_get_day",
          &perms,
          db.clone(),
          json!({ "date": "2026-07-08" }),
        )
        .await
        .unwrap();
      assert_eq!(authored["narrative"], "Authored by the agent.");
      assert_eq!(authored["answers"]["goal"], "Undock more.");

      let reread = registry
        .dispatch(
          "captains_log_get_day",
          &perms,
          db.clone(),
          json!({ "date": "2026-07-05" }),
        )
        .await
        .unwrap();
      assert_eq!(reread["kill_reports"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn it_denies_writes_but_serves_reads_with_local_write_off() {
      let db = store::open_test().await.unwrap();
      seed_busy_day(&db).await;
      let registry = registry();
      let mut perms = McpPerms::default();
      perms.set_local_write(false);

      for tool in [
        "captains_log_set_narrative",
        "captains_log_set_answer",
        "captains_log_set_kill_report",
      ] {
        let outcome = registry.dispatch(tool, &perms, db.clone(), json!({})).await;

        assert!(
          matches!(outcome, Err(ToolError::PermissionDenied("local_write"))),
          "{tool} must be gated off: {outcome:?}"
        );
      }

      registry
        .dispatch("captains_log_list_days", &perms, db.clone(), json!({}))
        .await
        .unwrap();
      registry
        .dispatch(
          "captains_log_get_day",
          &perms,
          db.clone(),
          json!({ "date": "2026-07-05" }),
        )
        .await
        .unwrap();

      assert_eq!(captains_log::get(&db, "2026-07-08").await.unwrap(), None);
    }
  }

  mod matches_rollup {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_matches_the_in_app_rollup_for_the_same_day() {
      let db = store::open_test().await.unwrap();
      seed_busy_day(&db).await;

      let rollup = rollup::for_date(&db, "2026-07-05").await.unwrap();
      let value = get_day(db.clone(), json!({ "date": "2026-07-05" })).await.unwrap();

      assert_eq!(value["money"]["net"].as_f64().unwrap(), rollup.money.net());
      assert_eq!(
        value["combat"]["kill_count"].as_u64().unwrap(),
        rollup.combat.kill_count as u64
      );
      assert_eq!(
        value["combat"]["loss_count"].as_u64().unwrap(),
        rollup.combat.loss_count as u64
      );
      assert_eq!(
        value["combat"]["kill_value"].as_f64().unwrap(),
        rollup.combat.kill_value
      );

      let mcp_kill = value["combat"]["engagements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|engagement| engagement["is_kill"] == true)
        .unwrap();
      let rollup_kill = rollup
        .combat
        .engagements
        .iter()
        .find(|engagement| engagement.is_kill)
        .unwrap();

      assert_eq!(mcp_kill["killmail_id"].as_i64().unwrap(), rollup_kill.killmail_id);
      assert_eq!(mcp_kill["ship_type_id"].as_i64().unwrap(), rollup_kill.ship_type_id);
      assert_eq!(mcp_kill["ship_type_name"], "Capsule");
      assert_eq!(mcp_kill["system_name"], "Jita");
    }
  }

  mod discoverability {
    use super::*;

    fn description(name: &str) -> String {
      tools()
        .into_iter()
        .find(|tool| tool.name() == name)
        .map(|tool| tool.description().to_owned())
        .unwrap()
    }

    #[test]
    fn it_describes_the_read_tools_so_read_my_logs_selects_them() {
      let list = description("captains_log_list_days").to_lowercase();
      let get = description("captains_log_get_day").to_lowercase();

      assert!(list.contains("captain's log"), "list_days: {list}");
      assert!(
        list.starts_with("lists"),
        "list_days should read as a listing tool: {list}"
      );
      assert!(get.contains("captain's log"), "get_day: {get}");
      assert!(get.starts_with("reads"), "get_day should read as a reading tool: {get}");
    }

    #[test]
    fn it_gates_reads_and_writes_under_the_expected_permissions() {
      let permission = |name: &str| {
        tools()
          .into_iter()
          .find(|tool| tool.name() == name)
          .map(|tool| tool.permission())
          .unwrap()
      };

      assert!(matches!(permission("captains_log_list_days"), Permission::Read));
      assert!(matches!(permission("captains_log_get_day"), Permission::Read));
      assert!(matches!(
        permission("captains_log_describe_structure"),
        Permission::Read
      ));
      assert!(matches!(
        permission("captains_log_set_narrative"),
        Permission::LocalWrite
      ));
      assert!(matches!(permission("captains_log_set_answer"), Permission::LocalWrite));
      assert!(matches!(
        permission("captains_log_set_kill_report"),
        Permission::LocalWrite
      ));
    }
  }

  mod day_taxonomy {
    use pretty_assertions::assert_eq;

    use super::*;

    fn day(value: &Value, date: &str) -> Value {
      value["days"]
        .as_array()
        .unwrap()
        .iter()
        .find(|day| day["date"] == date)
        .unwrap()
        .clone()
    }

    #[tokio::test]
    async fn it_reports_each_seeded_day_with_the_right_completeness() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      seed_type(&db, 670, "Capsule").await;
      seed_type(&db, 22_544, "Hulk").await;
      seed_system(&db, 30_000_142, "Jita").await;

      seed_kill(
        &db,
        PILOT,
        200,
        true,
        ENEMY,
        670,
        30_000_142,
        "2026-07-15T20:00:00Z",
        500.0,
      )
      .await;
      captains_log::upsert_answer(&db, "2026-07-15", AnswerKey::Goal, Some("Undock and roam."))
        .await
        .unwrap();

      seed_kill(
        &db,
        PILOT,
        201,
        false,
        PILOT,
        670,
        30_000_142,
        "2026-07-14T20:00:00Z",
        130.0,
      )
      .await;
      captains_log::upsert_answer(&db, "2026-07-14", AnswerKey::Goal, Some("Scout the pipe."))
        .await
        .unwrap();

      seed_industry(&db, 10, PILOT, 22_544, 3, "2026-07-13T18:00:00Z").await;
      captains_log::upsert_answer(&db, "2026-07-13", AnswerKey::Goal, Some("Sell the barges."))
        .await
        .unwrap();

      seed_kill(
        &db,
        PILOT,
        202,
        true,
        ENEMY,
        670,
        30_000_142,
        "2026-07-12T20:00:00Z",
        400.0,
      )
      .await;

      seed_industry(&db, 11, PILOT, 22_544, 2, "2026-07-11T18:00:00Z").await;
      captains_log::upsert_narrative(&db, "2026-07-11", Some("Quiet builder night."))
        .await
        .unwrap();

      let value = list_days(db, json!({})).await.unwrap();

      let dates: Vec<&str> = value["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|day| day["date"].as_str().unwrap())
        .collect();
      assert_eq!(
        dates,
        vec!["2026-07-15", "2026-07-14", "2026-07-13", "2026-07-12", "2026-07-11"]
      );

      let complete = day(&value, "2026-07-15");
      assert_eq!(complete["has_entry"], true);
      assert_eq!(complete["has_activity"], true);
      assert_eq!(complete["completeness"]["is_complete"], true);

      let missing_debrief = day(&value, "2026-07-14");
      assert_eq!(missing_debrief["completeness"]["is_complete"], false);
      assert_eq!(
        missing_debrief["completeness"]["missing_debriefs"]
          .as_array()
          .unwrap()
          .len(),
        1
      );

      let trade = day(&value, "2026-07-13");
      assert_eq!(trade["completeness"]["is_complete"], true);

      let activity_only = day(&value, "2026-07-12");
      assert_eq!(activity_only["has_entry"], false);
      assert_eq!(activity_only["has_activity"], true);

      let builder = day(&value, "2026-07-11");
      assert_eq!(builder["has_entry"], true);
      assert!(
        builder["completeness"]["missing_prompts"]
          .as_array()
          .unwrap()
          .iter()
          .any(|key| key == "goal")
      );
    }
  }

  mod range {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_objective_row(
      db: &Database,
      title: &str,
      status: &str,
      completed_at: Option<&str>,
      cancelled_at: Option<&str>,
    ) -> i64 {
      sqlx::query_scalar::<_, i64>(
        "INSERT INTO objectives (accent, created_at, status, title, completed_at, cancelled_at) \
        VALUES ('#FF8800', '2026-07-01T00:00:00Z', ?, ?, ?, ?) RETURNING id",
      )
      .bind(status)
      .bind(title)
      .bind(completed_at)
      .bind(cancelled_at)
      .fetch_one(db.writer())
      .await
      .unwrap()
    }

    async fn seed_order_row(db: &Database, character_id: i64, text: &str, status: &str, updated_at: &str) -> i64 {
      sqlx::query_scalar::<_, i64>(
        "INSERT INTO dossier_orders (character_id, created_at, position, status, text, updated_at) \
        VALUES (?, '2026-07-01T00:00:00Z', 0, ?, ?, ?) RETURNING id",
      )
      .bind(character_id)
      .bind(status)
      .bind(text)
      .bind(updated_at)
      .fetch_one(db.writer())
      .await
      .unwrap()
    }

    #[tokio::test]
    async fn it_carries_the_current_state_header_on_page_one() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      seed_owned(&db, OTHER, "Pilot Two").await;
      let active = seed_objective_row(&db, "Fund a Nyx", "active", None, None).await;
      dossier::upsert_brief(&db, PILOT, Some("Hunt"), Some("Rat"))
        .await
        .unwrap();
      let order = dossier::add_order(&db, PILOT, "Fit a Loki").await.unwrap();

      let value = range(db, json!({})).await.unwrap();

      let header = &value["current_state"];
      let objectives = header["standing_orders"]["objectives"].as_array().unwrap();
      assert_eq!(objectives.len(), 1);
      assert_eq!(objectives[0]["id"], active);
      let orders = header["standing_orders"]["dossier_orders"].as_array().unwrap();
      assert_eq!(orders.len(), 1);
      assert_eq!(orders[0]["id"], order.id);

      let dossiers = header["dossiers"].as_array().unwrap();
      assert_eq!(dossiers.len(), 2);
      let pilot = dossiers.iter().find(|entry| entry["character_id"] == PILOT).unwrap();
      assert_eq!(pilot["character_name"], "Pilot One");
      assert_eq!(pilot["brief"]["purpose"], "Hunt");
      assert_eq!(pilot["orders"].as_array().unwrap().len(), 1);
      let other = dossiers.iter().find(|entry| entry["character_id"] == OTHER).unwrap();
      assert_eq!(other["brief"], Value::Null);
    }

    #[tokio::test]
    async fn it_omits_the_header_after_page_one() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      seed_journal(&db, 1, PILOT, "2026-07-05T10:00:00Z", 100.0).await;
      seed_journal(&db, 2, PILOT, "2026-07-06T10:00:00Z", 100.0).await;

      let value = range(db, json!({ "limit": 1, "page": 1 })).await.unwrap();

      assert!(value.get("current_state").is_none());
      assert_eq!(value["page"], 1);
      assert_eq!(value["days"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_assembles_a_day_bundle_with_activity_and_resolved_orders() {
      let db = store::open_test().await.unwrap();
      seed_busy_day(&db).await;
      let completed = seed_objective_row(&db, "Barge line", "complete", Some("2026-07-05T12:00:00Z"), None).await;
      let order = seed_order_row(&db, PILOT, "Old order", "cancelled", "2026-07-05T09:00:00Z").await;
      crate::store::repo::field_notes::insert(&db, "2026-07-05", "Cyno in Tama")
        .await
        .unwrap();

      let value = range(db, json!({ "from": "2026-07-05", "to": "2026-07-05" }))
        .await
        .unwrap();

      let days = value["days"].as_array().unwrap();
      assert_eq!(days.len(), 1);
      let day = &days[0];
      assert_eq!(day["date"], "2026-07-05");
      assert_eq!(day["narrative"], "Clean roam, one kill, lost the Hulk hauler.");
      assert_eq!(day["answers"]["goal"], "Spin up the barge line.");
      assert_eq!(day["combat"]["kill_count"], 1);
      assert_eq!(day["industry"][0]["product_type_name"], "Hulk");
      assert_eq!(day["field_notes"][0]["text"], "Cyno in Tama");

      let resolved = &day["resolved_orders"];
      let completed_objectives = resolved["objectives_completed"].as_array().unwrap();
      assert_eq!(completed_objectives.len(), 1);
      assert_eq!(completed_objectives[0]["id"], completed);
      let cancelled_orders = resolved["dossier_orders_cancelled"].as_array().unwrap();
      assert_eq!(cancelled_orders.len(), 1);
      assert_eq!(cancelled_orders[0]["id"], order);
    }

    #[tokio::test]
    async fn it_buckets_resolved_orders_by_their_resolution_day() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      seed_journal(&db, 1, PILOT, "2026-07-05T10:00:00Z", 100.0).await;
      seed_journal(&db, 2, PILOT, "2026-07-06T10:00:00Z", 100.0).await;
      seed_objective_row(&db, "Done", "complete", Some("2026-07-05T12:00:00Z"), None).await;
      seed_objective_row(&db, "Scrapped", "cancelled", None, Some("2026-07-06T12:00:00Z")).await;

      let value = range(db, json!({})).await.unwrap();
      let day = |date: &str| {
        value["days"]
          .as_array()
          .unwrap()
          .iter()
          .find(|entry| entry["date"] == date)
          .unwrap()
          .clone()
      };

      let fifth = day("2026-07-05");
      assert_eq!(
        fifth["resolved_orders"]["objectives_completed"]
          .as_array()
          .unwrap()
          .len(),
        1
      );
      assert_eq!(
        fifth["resolved_orders"]["objectives_cancelled"]
          .as_array()
          .unwrap()
          .len(),
        0
      );

      let sixth = day("2026-07-06");
      assert_eq!(
        sixth["resolved_orders"]["objectives_cancelled"]
          .as_array()
          .unwrap()
          .len(),
        1
      );
      assert_eq!(
        sixth["resolved_orders"]["objectives_completed"]
          .as_array()
          .unwrap()
          .len(),
        0
      );
    }

    #[tokio::test]
    async fn it_omits_days_with_no_activity() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      seed_journal(&db, 1, PILOT, "2026-07-05T10:00:00Z", 100.0).await;
      seed_journal(&db, 2, PILOT, "2026-07-07T10:00:00Z", 100.0).await;

      let value = range(db, json!({ "from": "2026-07-05", "to": "2026-07-07" }))
        .await
        .unwrap();

      let dates: Vec<&str> = value["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["date"].as_str().unwrap())
        .collect();
      assert_eq!(dates, vec!["2026-07-07", "2026-07-05"]);
    }

    #[tokio::test]
    async fn it_paginates_by_day_and_flags_has_more() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT, "Pilot One").await;
      seed_journal(&db, 1, PILOT, "2026-07-05T10:00:00Z", 1.0).await;
      seed_journal(&db, 2, PILOT, "2026-07-06T10:00:00Z", 1.0).await;
      seed_journal(&db, 3, PILOT, "2026-07-07T10:00:00Z", 1.0).await;

      let first = range(db.clone(), json!({ "limit": 2 })).await.unwrap();
      assert!(first.get("current_state").is_some());
      assert_eq!(first["has_more"], true);
      let first_dates: Vec<&str> = first["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["date"].as_str().unwrap())
        .collect();
      assert_eq!(first_dates, vec!["2026-07-07", "2026-07-06"]);

      let second = range(db, json!({ "limit": 2, "page": 1 })).await.unwrap();
      assert!(second.get("current_state").is_none());
      assert_eq!(second["has_more"], false);
      let second_dates: Vec<&str> = second["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["date"].as_str().unwrap())
        .collect();
      assert_eq!(second_dates, vec!["2026-07-05"]);
    }

    #[test]
    fn it_registers_as_a_read_tool_that_reads_the_captains_log() {
      let tool = tools()
        .into_iter()
        .find(|tool| tool.name() == "captains_log_range")
        .unwrap();

      assert!(matches!(tool.permission(), Permission::Read));
      let description = tool.description().to_lowercase();
      assert!(description.starts_with("reads"), "{description}");
      assert!(description.contains("captain's log"), "{description}");
    }
  }
}
