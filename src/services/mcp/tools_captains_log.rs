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
    args::{ArgSpec, require_i64, require_str},
    names::{self, ResolvedName},
    tool::{McpTool, Permission, ToolError},
  },
  store::{
    Database,
    model::{CaptainsLog, KillmailReport, SkillCompletion},
    repo::{
      calendar_event_note, captains_log,
      captains_log::AnswerKey,
      captains_log_rollup::{self, CalendarEntry, CombatKill, DayMoney, IndustryDelivery, NetWorthDelta},
      character, killmail_report,
    },
  },
};

const YC_EPOCH_OFFSET: i32 = 1898;

pub fn tools() -> Vec<McpTool> {
  vec![
    list_days_tool(),
    get_day_tool(),
    set_answer_tool(),
    set_kill_report_tool(),
    set_narrative_tool(),
  ]
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
  let mut map = serde_json::Map::with_capacity(AnswerKey::ALL.len());
  for key in AnswerKey::ALL {
    let text = log.and_then(|log| answer_text(log, key));
    map.insert(key.as_key().to_owned(), json!(text));
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
  }
}

fn day_value(
  date: &str,
  rollup: &DayRollup,
  victims: &HashMap<i64, i64>,
  names: &HashMap<i64, ResolvedName>,
  log: Option<&CaptainsLog>,
  reports: &[KillmailReport],
  event_notes: Vec<Value>,
) -> Value {
  json!({
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

  Ok(day_value(
    &date,
    &rollup,
    &victims,
    &names,
    log.as_ref(),
    &reports,
    notes,
  ))
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

fn in_range(date: &str, from: Option<&str>, to: Option<&str>) -> bool {
  from.is_none_or(|from| date >= from) && to.is_none_or(|to| date <= to)
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

async fn resolve_names_map(db: &Database, ids: &[i64]) -> Result<HashMap<i64, ResolvedName>, ToolError> {
  names::resolve(db, ids, no_esi).await.map_err(internal)
}

async fn set_answer(db: Database, args: Value) -> Result<Value, ToolError> {
  let date = require_date(&args)?;
  let prompt = validate_prompt(&args)?;
  let text = require_str(&args, "text")?.to_owned();
  if dry_run(&args) {
    return Ok(json!({ "date": date, "dry_run": true, "prompt": prompt.as_key(), "text": text }));
  }

  captains_log::upsert_answer(&db, &date, prompt, Some(&text))
    .await
    .map_err(internal)?;
  Ok(json!({ "date": date, "prompt": prompt.as_key(), "text": text }))
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

fn validate_prompt(args: &Value) -> Result<AnswerKey, ToolError> {
  let key = require_str(args, "prompt")?;
  AnswerKey::from_key(key).ok_or_else(|| {
    let keys: Vec<&str> = AnswerKey::ALL.iter().map(|key| key.as_key()).collect();
    ToolError::InvalidArguments(format!(
      "`prompt` must be one of: {}, but received `{key}`",
      keys.join(", ")
    ))
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
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
}
