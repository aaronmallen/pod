use std::collections::HashMap;

use serde_json::{Value, json};

use crate::{
  clients::{
    self, Error as ClientError, esi,
    esi::models::{
      market::{MarketHistory, RegionOrder},
      universe::NameRecord,
    },
    http,
  },
  features::wallet::budget,
  services::{
    mcp::{
      args::{ArgSpec, DEFAULT_LIMIT, paginate_vec, pagination, require_i64, require_i64_array, require_str},
      names::{self, ResolvedName},
      tool::{McpTool, Permission, ToolError},
    },
    prices::{JITA_STATION_ID, THE_FORGE_REGION_ID},
  },
  store::{
    Database,
    model::{CharacterMail, MarketOrder},
    repo::{assets, blueprints, character, finance, industry, mail, org, sde},
  },
};

pub fn tools() -> Vec<McpTool> {
  vec![
    list_characters_tool(),
    get_wallet_balances_tool(),
    list_journal_tool(),
    list_market_transactions_tool(),
    list_contracts_tool(),
    get_budget_view_tool(),
    get_skills_tool(),
    get_skill_queue_tool(),
    list_industry_jobs_tool(),
    get_planner_tool(),
    list_assets_tool(),
    list_blueprints_tool(),
    list_corporations_tool(),
    list_market_orders_tool(),
    list_mail_tool(),
    get_mail_body_tool(),
    get_market_prices_tool(),
    get_live_market_tool(),
    resolve_names_tool(),
  ]
}

fn list_characters_tool() -> McpTool {
  McpTool::new(
    "list_characters",
    t!("mcp.tools.list_characters").into_owned(),
    Permission::Read,
    |db, _args| async move {
      let characters = character::all_owned(&db).await.map_err(internal)?;
      let mut rows = Vec::with_capacity(characters.len());
      for char in &characters {
        let state = character::state(&db, char.id()).await.map_err(internal)?;
        rows.push(json!({
          "corporation_id": char.corporation_id(),
          "id": char.id(),
          "name": char.name(),
          "total_sp": state.as_ref().and_then(|s| s.total_sp),
          "wallet_balance": state.as_ref().and_then(|s| s.wallet_balance),
        }));
      }
      Ok(json!({ "characters": rows }))
    },
  )
}

fn get_wallet_balances_tool() -> McpTool {
  McpTool::new(
    "get_wallet_balances",
    t!("mcp.tools.get_wallet_balances").into_owned(),
    Permission::Read,
    |db, _args| async move {
      let financials = finance::financials_all(&db).await.map_err(internal)?;
      let characters: Vec<Value> = financials
        .iter()
        .map(|f| {
          json!({
            "asset_value": f.asset_value,
            "character_id": f.character_id,
            "escrow": f.escrow,
            "liquid": f.liquid,
            "net_worth": f.net_worth,
          })
        })
        .collect();

      let owned = org::all_owned_corporations(&db).await.map_err(internal)?;
      let mut corporations = Vec::with_capacity(owned.len());
      for corp in &owned {
        let divisions = finance::divisions(&db, corp.id()).await.map_err(internal)?;
        let division_rows: Vec<Value> = divisions
          .iter()
          .map(|d| {
            json!({
              "balance": d.balance(),
              "division": d.division(),
              "name": d.name(),
            })
          })
          .collect();
        corporations.push(json!({
          "corporation_id": corp.id(),
          "divisions": division_rows,
          "name": corp.name(),
        }));
      }

      Ok(json!({ "characters": characters, "corporations": corporations }))
    },
  )
}

fn list_journal_tool() -> McpTool {
  McpTool::new(
    "list_journal",
    t!("mcp.tools.list_journal").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let (page, limit) = pagination(&args);
      let Some(owner) = require_owner(&db, &args).await? else {
        return Ok(json!({ "entries": [], "has_more": false, "page": page }));
      };
      let mut rows = journal_rows(&db, owner).await?;
      let (entries, has_more) = paginate_vec(&mut rows, page, limit);
      Ok(json!({ "entries": entries, "has_more": has_more, "page": page }))
    },
  )
  .with_args(paginated_owner_args())
}

fn list_market_transactions_tool() -> McpTool {
  McpTool::new(
    "list_market_transactions",
    t!("mcp.tools.list_market_transactions").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let (page, limit) = pagination(&args);
      let Some(owner) = require_owner(&db, &args).await? else {
        return Ok(json!({ "has_more": false, "page": page, "transactions": [] }));
      };
      let mut rows = transaction_rows(&db, owner).await?;
      let (entries, has_more) = paginate_vec(&mut rows, page, limit);
      Ok(json!({ "has_more": has_more, "page": page, "transactions": entries }))
    },
  )
  .with_args(paginated_owner_args())
}

fn list_contracts_tool() -> McpTool {
  McpTool::new(
    "list_contracts",
    t!("mcp.tools.list_contracts").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let (page, limit) = pagination(&args);
      let Some(owner) = require_owner(&db, &args).await? else {
        return Ok(json!({ "contracts": [], "has_more": false, "page": page }));
      };
      let mut rows = contract_rows(&db, owner).await?;
      let (entries, has_more) = paginate_vec(&mut rows, page, limit);
      Ok(json!({ "contracts": entries, "has_more": has_more, "page": page }))
    },
  )
  .with_args(paginated_owner_args())
}

fn get_budget_view_tool() -> McpTool {
  McpTool::new(
    "get_budget_view",
    t!("mcp.tools.get_budget_view").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let month = args
        .get("month")
        .and_then(Value::as_str)
        .map_or_else(budget::current_month, str::to_owned);
      let view = budget::load(&db, &month).await;
      let groups: Vec<Value> = view
        .groups
        .iter()
        .map(|group| {
          let categories: Vec<Value> = group
            .categories
            .iter()
            .map(|c| {
              json!({
                "activity": c.activity,
                "assigned": c.assigned,
                "available": c.available(),
                "carry": c.carry,
                "id": c.id,
                "name": c.name,
              })
            })
            .collect();
          json!({ "categories": categories, "id": group.id, "name": group.name })
        })
        .collect();
      Ok(json!({
        "groups": groups,
        "month": view.month,
        "overspent": view.overspent,
        "pool": view.pool,
        "ready_to_assign": view.ready_to_assign,
      }))
    },
  )
  .with_args([ArgSpec::optional_string(
    "month",
    t!("mcp.tools.get_budget_view_month").into_owned(),
  )])
}

fn get_skills_tool() -> McpTool {
  McpTool::new(
    "get_skills",
    t!("mcp.tools.get_skills").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let character_id = require_i64(&args, "character_id")?;
      let skills = character::skills(&db, character_id, chrono::Utc::now())
        .await
        .map_err(internal)?;
      let state = character::state(&db, character_id).await.map_err(internal)?;
      let ids: Vec<i64> = skills.iter().map(|s| s.skill_id()).collect();
      let names = resolve_names_map(&db, &ids).await?;
      let rows: Vec<Value> = skills.iter().map(|s| skill_value(s, &names)).collect();
      Ok(json!({ "skills": rows, "total_sp": state.as_ref().and_then(|s| s.total_sp) }))
    },
  )
  .with_args([character_id_arg()])
}

fn get_skill_queue_tool() -> McpTool {
  McpTool::new(
    "get_skill_queue",
    t!("mcp.tools.get_skill_queue").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let character_id = require_i64(&args, "character_id")?;
      let queue = character::skillqueue(&db, character_id).await.map_err(internal)?;
      let ids: Vec<i64> = queue.iter().map(|e| e.skill_id()).collect();
      let names = resolve_names_map(&db, &ids).await?;
      let rows: Vec<Value> = queue.iter().map(|e| skillqueue_value(e, &names)).collect();
      Ok(json!({ "queue": rows }))
    },
  )
  .with_args([character_id_arg()])
}

fn list_industry_jobs_tool() -> McpTool {
  McpTool::new(
    "list_industry_jobs",
    t!("mcp.tools.list_industry_jobs").into_owned(),
    Permission::Read,
    |db, _args| async move {
      let jobs = industry::list_all(&db).await.map_err(internal)?;
      let mut ids = Vec::new();
      for j in &jobs.character_jobs {
        ids.push(j.blueprint_type_id);
        ids.extend(j.product_type_id);
      }
      for j in &jobs.corporation_jobs {
        ids.push(j.blueprint_type_id);
        ids.extend(j.product_type_id);
      }
      let names = resolve_names_map(&db, &ids).await?;
      let character_jobs: Vec<Value> = jobs
        .character_jobs
        .iter()
        .map(|j| character_job_value(j, &names))
        .collect();
      let corporation_jobs: Vec<Value> = jobs
        .corporation_jobs
        .iter()
        .map(|j| corporation_job_value(j, &names))
        .collect();
      Ok(json!({ "character_jobs": character_jobs, "corporation_jobs": corporation_jobs }))
    },
  )
}

fn get_planner_tool() -> McpTool {
  McpTool::new(
    "get_planner",
    t!("mcp.tools.get_planner").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      if let Some(plan_id) = args.get("plan_id").and_then(Value::as_i64) {
        let Some(tree) = industry::load_plan(&db, plan_id).await.map_err(internal)? else {
          return Err(ToolError::InvalidArguments(format!("no plan with id {plan_id}")));
        };
        let segments = industry::segments_for_plan(&db, plan_id).await.map_err(internal)?;
        let types: Vec<Value> = tree
          .types
          .iter()
          .map(|t| {
            json!({
              "built": t.built,
              "facility_structure": t.facility_structure,
              "facility_system": t.facility_system,
              "me": t.me,
              "te": t.te,
              "type_id": t.type_id,
              "use_stock": t.use_stock,
            })
          })
          .collect();
        let segment_rows: Vec<Value> = segments
          .iter()
          .map(|s| {
            json!({
              "clone_id": s.clone_id,
              "pilot_id": s.pilot_id,
              "runs": s.runs,
              "segment_index": s.segment_index,
              "type_id": s.type_id,
            })
          })
          .collect();
        return Ok(json!({
          "plan_id": plan_id,
          "product_type_id": tree.product_type_id,
          "root_facility_system": tree.root_facility_system,
          "runs": tree.runs,
          "segments": segment_rows,
          "types": types,
        }));
      }

      let plans = industry::list_plans(&db).await.map_err(internal)?;
      let rows: Vec<Value> = plans
        .iter()
        .map(|p| {
          json!({
            "id": p.id(),
            "name": p.name(),
            "product_type_id": p.product_type_id(),
            "runs": p.runs(),
            "saved_at": p.saved_at(),
          })
        })
        .collect();
      Ok(json!({ "plans": rows }))
    },
  )
  .with_args([ArgSpec::optional_integer(
    "plan_id",
    0,
    t!("mcp.tools.get_planner_plan_id").into_owned(),
  )])
}

fn list_assets_tool() -> McpTool {
  McpTool::new(
    "list_assets",
    t!("mcp.tools.list_assets").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let (page, limit) = pagination(&args);
      let Some(owner) = require_owner(&db, &args).await? else {
        return Ok(json!({ "assets": [], "has_more": false, "page": page }));
      };
      let mut rows = asset_rows(&db, owner).await?;
      let (entries, has_more) = paginate_vec(&mut rows, page, limit);
      Ok(json!({ "assets": entries, "has_more": has_more, "page": page }))
    },
  )
  .with_args(paginated_owner_args())
}

fn list_blueprints_tool() -> McpTool {
  McpTool::new(
    "list_blueprints",
    t!("mcp.tools.list_blueprints").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let (page, limit) = pagination(&args);
      let Some(owner) = require_owner(&db, &args).await? else {
        return Ok(json!({ "blueprints": [], "has_more": false, "page": page }));
      };
      let mut rows = blueprint_rows(&db, owner).await?;
      let (entries, has_more) = paginate_vec(&mut rows, page, limit);
      Ok(json!({ "blueprints": entries, "has_more": has_more, "page": page }))
    },
  )
  .with_args(paginated_owner_args())
}

fn list_corporations_tool() -> McpTool {
  McpTool::new(
    "list_corporations",
    t!("mcp.tools.list_corporations").into_owned(),
    Permission::Read,
    |db, _args| async move {
      let owned = org::all_owned_corporations(&db).await.map_err(internal)?;
      let rows: Vec<Value> = owned
        .iter()
        .map(|c| json!({ "corporation_id": c.id(), "name": c.name() }))
        .collect();
      Ok(json!({ "corporations": rows }))
    },
  )
}

fn list_market_orders_tool() -> McpTool {
  McpTool::new(
    "list_market_orders",
    t!("mcp.tools.list_market_orders").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let character_id = require_i64(&args, "character_id")?;
      let (page, limit) = pagination(&args);
      let mut rows: Vec<MarketOrder> = finance::for_character(&db, character_id).await.map_err(internal)?;
      let (slice, has_more) = paginate_vec(&mut rows, page, limit);
      let orders: Vec<Value> = slice.iter().map(market_order_value).collect();
      Ok(json!({ "has_more": has_more, "orders": orders, "page": page }))
    },
  )
  .with_args(paginated_character_args())
}

fn list_mail_tool() -> McpTool {
  McpTool::new(
    "list_mail",
    t!("mcp.tools.list_mail").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let character_id = require_i64(&args, "character_id")?;
      let (page, limit) = pagination(&args);
      let mut rows: Vec<CharacterMail> = mail::headers(&db, character_id).await.map_err(internal)?;
      let (slice, has_more) = paginate_vec(&mut rows, page, limit);
      let entries: Vec<Value> = slice
        .iter()
        .map(|m| {
          json!({
            "from_id": m.from_id,
            "from_name": m.from_name,
            "is_read": m.is_read,
            "mail_id": m.mail_id,
            "subject": m.subject,
            "timestamp": m.timestamp,
          })
        })
        .collect();
      Ok(json!({ "has_more": has_more, "mail": entries, "page": page }))
    },
  )
  .with_args(paginated_character_args())
}

fn get_mail_body_tool() -> McpTool {
  McpTool::new(
    "get_mail_body",
    t!("mcp.tools.get_mail_body").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let character_id = require_i64(&args, "character_id")?;
      let mail_id = require_i64(&args, "mail_id")?;
      let Some(render) = mail::mail(&db, character_id, mail_id).await.map_err(internal)? else {
        return Err(ToolError::InvalidArguments(format!(
          "no mail {mail_id} for character {character_id}"
        )));
      };
      Ok(json!({
        "body": render.body.body,
        "from_id": render.header.from_id,
        "from_name": render.header.from_name,
        "label_ids": render.label_ids,
        "mail_id": render.header.mail_id,
        "recipients": render.recipients_display,
        "subject": render.header.subject,
        "timestamp": render.header.timestamp,
      }))
    },
  )
  .with_args([
    character_id_arg(),
    ArgSpec::integer("mail_id", t!("mcp.tools.get_mail_body_mail_id").into_owned()),
  ])
}

fn get_market_prices_tool() -> McpTool {
  McpTool::new(
    "get_market_prices",
    t!("mcp.tools.get_market_prices").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let filter: Option<Vec<i64>> = args
        .get("type_ids")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_i64).collect());
      let prices = finance::market_prices_all(&db).await.map_err(internal)?;
      let rows: Vec<Value> = prices
        .iter()
        .filter(|p| filter.as_ref().is_none_or(|ids| ids.contains(&p.type_id)))
        .map(|p| {
          json!({
            "adjusted_price": p.adjusted_price,
            "average_price": p.average_price,
            "source": p.source,
            "type_id": p.type_id,
          })
        })
        .collect();
      Ok(json!({ "prices": rows }))
    },
  )
  .with_args([ArgSpec::optional_integer_array(
    "type_ids",
    t!("mcp.tools.get_market_prices_type_ids").into_owned(),
  )])
}

fn get_live_market_tool() -> McpTool {
  McpTool::new(
    "get_live_market",
    t!("mcp.tools.get_live_market").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let type_ids = require_i64_array(&args, "type_ids")?;
      let region_id = args
        .get("region_id")
        .and_then(Value::as_i64)
        .unwrap_or(THE_FORGE_REGION_ID);
      let location_id = args
        .get("location_id")
        .and_then(Value::as_i64)
        .unwrap_or(JITA_STATION_ID);
      let esi = public_esi(&db).map_err(internal)?;
      let names = resolve_names_map(&db, &type_ids).await?;
      let mut rows = Vec::with_capacity(type_ids.len());
      for type_id in type_ids {
        rows.push(live_market_row(&esi, region_id, location_id, type_id, &names).await?);
      }
      Ok(json!({ "location_id": location_id, "region_id": region_id, "types": rows }))
    },
  )
  .with_args([
    ArgSpec::integer_array("type_ids", t!("mcp.tools.get_live_market_type_ids").into_owned()),
    ArgSpec::optional_integer(
      "region_id",
      THE_FORGE_REGION_ID,
      t!("mcp.tools.get_live_market_region_id").into_owned(),
    ),
    ArgSpec::optional_integer(
      "location_id",
      JITA_STATION_ID,
      t!("mcp.tools.get_live_market_location_id").into_owned(),
    ),
  ])
}

fn resolve_names_tool() -> McpTool {
  McpTool::new(
    "resolve_names",
    t!("mcp.tools.resolve_names").into_owned(),
    Permission::Read,
    |db, args: Value| async move {
      let ids = require_i64_array(&args, "ids")?;
      let esi = public_esi(&db).map_err(internal)?;
      let resolved = names::resolve(&db, &ids, |missing| resolve_parties_via_esi(&esi, missing))
        .await
        .map_err(internal)?;
      let mut names_map = serde_json::Map::with_capacity(resolved.len());
      for (id, entry) in resolved {
        names_map.insert(id.to_string(), json!({ "kind": entry.kind, "name": entry.name }));
      }
      Ok(json!({ "names": names_map }))
    },
  )
  .with_args([ArgSpec::integer_array(
    "ids",
    t!("mcp.tools.resolve_names_ids").into_owned(),
  )])
}

fn internal(error: impl std::fmt::Display) -> ToolError {
  ToolError::Internal(error.to_string())
}

/// Resolves names from the local database only; unknown IDs return no entry rather than falling back to ESI.
async fn resolve_names_map(db: &Database, ids: &[i64]) -> Result<HashMap<i64, ResolvedName>, ToolError> {
  names::resolve(db, ids, no_esi).await.map_err(internal)
}

async fn no_esi(_ids: Vec<i64>) -> Result<HashMap<i64, NameRecord>, ClientError> {
  Ok(HashMap::new())
}

const NAME_CHUNK: usize = 1000;

/// Builds a tokenless ESI client; market and `/universe/names` endpoints are public and require no character token.
fn public_esi(db: &Database) -> Result<esi::Client, ClientError> {
  let http = http::Client::builder(http::Cache::new(db.clone())).build();
  esi::Client::builder(http).user_agent(clients::user_agent()).build()
}

async fn resolve_parties_via_esi(esi: &esi::Client, ids: Vec<i64>) -> Result<HashMap<i64, NameRecord>, ClientError> {
  let mut resolved = HashMap::with_capacity(ids.len());
  for chunk in ids.chunks(NAME_CHUNK) {
    match esi.universe().names(chunk).await {
      Ok(records) => resolved.extend(records.into_iter().map(|record| (record.id, record))),
      // ESI returns 404 (not an empty array) when no ID in the batch is recognized.
      Err(ClientError::Http(error)) if error.status() == Some(reqwest::StatusCode::NOT_FOUND) => {}
      Err(error) => return Err(error),
    }
  }
  Ok(resolved)
}

async fn live_market_row(
  esi: &esi::Client,
  region_id: i64,
  location_id: i64,
  type_id: i64,
  names: &HashMap<i64, ResolvedName>,
) -> Result<Value, ToolError> {
  let market = esi.market();
  let buy_orders = market.buy_orders(region_id, type_id).await.map_err(internal)?;
  let sell_orders = market.sell_orders(region_id, type_id).await.map_err(internal)?;
  let history = market.history(region_id, type_id).await.map_err(internal)?;
  Ok(market_row_value(
    type_id,
    name_of(names, type_id),
    &buy_orders,
    &sell_orders,
    &history,
    location_id,
  ))
}

fn market_row_value(
  type_id: i64,
  type_name: Option<&str>,
  buy_orders: &[RegionOrder],
  sell_orders: &[RegionOrder],
  history: &[MarketHistory],
  location_id: i64,
) -> Value {
  let best_buy = best_order(buy_orders, location_id, true);
  let best_sell = best_order(sell_orders, location_id, false);
  let latest = latest_history(history);
  json!({
    "best_buy": best_buy.map(|order| order.price),
    "best_buy_volume": best_buy.map(|order| order.volume_remain),
    "best_sell": best_sell.map(|order| order.price),
    "best_sell_volume": best_sell.map(|order| order.volume_remain),
    "daily": latest.map(history_value),
    "daily_volume": latest.map(|day| day.volume),
    "type_id": type_id,
    "type_name": type_name,
  })
}

fn history_value(day: &MarketHistory) -> Value {
  json!({
    "average": day.average,
    "date": day.date,
    "highest": day.highest,
    "lowest": day.lowest,
    "order_count": day.order_count,
    "volume": day.volume,
  })
}

fn best_order(orders: &[RegionOrder], location_id: i64, want_buy: bool) -> Option<&RegionOrder> {
  orders
    .iter()
    .filter(|order| order.is_buy_order == want_buy && order.location_id == location_id)
    .max_by(|a, b| {
      if want_buy {
        a.price.total_cmp(&b.price)
      } else {
        b.price.total_cmp(&a.price)
      }
    })
}

fn latest_history(history: &[MarketHistory]) -> Option<&MarketHistory> {
  history.iter().max_by(|a, b| a.date.cmp(&b.date))
}

fn name_of(names: &HashMap<i64, ResolvedName>, id: i64) -> Option<&str> {
  names.get(&id).map(|resolved| resolved.name.as_str())
}

fn optional_name(names: &HashMap<i64, ResolvedName>, id: Option<i64>) -> Option<&str> {
  id.and_then(|value| name_of(names, value))
}

fn skill_value(skill: &crate::store::model::CharacterSkill, names: &HashMap<i64, ResolvedName>) -> Value {
  json!({
    "active_skill_level": skill.active_skill_level(),
    "skill_id": skill.skill_id(),
    "skill_name": name_of(names, skill.skill_id()),
    "skillpoints_in_skill": skill.skillpoints_in_skill(),
    "trained_skill_level": skill.trained_skill_level(),
  })
}

fn skillqueue_value(entry: &crate::store::model::CharacterSkillqueue, names: &HashMap<i64, ResolvedName>) -> Value {
  json!({
    "finish_date": entry.finish_date(),
    "finished_level": entry.finished_level(),
    "queue_position": entry.queue_position(),
    "skill_id": entry.skill_id(),
    "skill_name": name_of(names, entry.skill_id()),
    "start_date": entry.start_date(),
  })
}

fn character_job_value(job: &crate::store::model::CharacterIndustryJob, names: &HashMap<i64, ResolvedName>) -> Value {
  json!({
    "activity_id": job.activity_id,
    "blueprint_type_id": job.blueprint_type_id,
    "blueprint_type_name": name_of(names, job.blueprint_type_id),
    "character_id": job.character_id,
    "end_date": job.end_date,
    "job_id": job.job_id,
    "product_type_id": job.product_type_id,
    "product_type_name": optional_name(names, job.product_type_id),
    "runs": job.runs,
    "start_date": job.start_date,
    "status": job.status,
  })
}

fn corporation_job_value(
  job: &crate::store::model::CorporationIndustryJob,
  names: &HashMap<i64, ResolvedName>,
) -> Value {
  json!({
    "activity_id": job.activity_id,
    "blueprint_type_id": job.blueprint_type_id,
    "blueprint_type_name": name_of(names, job.blueprint_type_id),
    "corporation_id": job.corporation_id,
    "end_date": job.end_date,
    "job_id": job.job_id,
    "product_type_id": job.product_type_id,
    "product_type_name": optional_name(names, job.product_type_id),
    "runs": job.runs,
    "start_date": job.start_date,
    "status": job.status,
  })
}

fn character_id_arg() -> ArgSpec {
  ArgSpec::integer("character_id", t!("mcp.tools.shared_arg_character_id").into_owned())
}

fn paginated_character_args() -> [ArgSpec; 3] {
  [
    character_id_arg(),
    ArgSpec::optional_integer("page", 0, t!("mcp.tools.shared_arg_page").into_owned()),
    ArgSpec::optional_integer("limit", DEFAULT_LIMIT, t!("mcp.tools.shared_arg_limit").into_owned()),
  ]
}

const OWNER_TYPE_CHARACTER: &str = "character";

const OWNER_TYPE_CORPORATION: &str = "corporation";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadOwner {
  Character(i64),
  Corporation(i64),
}

fn paginated_owner_args() -> [ArgSpec; 4] {
  [
    ArgSpec::string("owner_type", t!("mcp.tools.shared_arg_owner_type").into_owned()),
    ArgSpec::integer("owner_id", t!("mcp.tools.shared_arg_owner_id").into_owned()),
    ArgSpec::optional_integer("page", 0, t!("mcp.tools.shared_arg_page").into_owned()),
    ArgSpec::optional_integer("limit", DEFAULT_LIMIT, t!("mcp.tools.shared_arg_limit").into_owned()),
  ]
}

/// Returns `Ok(None)` for an unauthorized corporation rather than an error; characters are always trusted. Callers return an empty result set on `None`.
async fn require_owner(db: &Database, args: &Value) -> Result<Option<ReadOwner>, ToolError> {
  let owner_type = require_str(args, "owner_type")?;
  let owner_id = require_i64(args, "owner_id")?;
  match owner_type {
    OWNER_TYPE_CHARACTER => Ok(Some(ReadOwner::Character(owner_id))),
    OWNER_TYPE_CORPORATION => {
      if org::corp_is_authorized(db, owner_id).await.map_err(internal)? {
        Ok(Some(ReadOwner::Corporation(owner_id)))
      } else {
        Ok(None)
      }
    }
    _ => Err(ToolError::InvalidArguments(
      "`owner_type` must be character or corporation".to_owned(),
    )),
  }
}

struct AssetFields {
  is_blueprint_copy: Option<bool>,
  is_singleton: bool,
  item_id: i64,
  location_flag: String,
  location_id: i64,
  name: Option<String>,
  quantity: i64,
  type_id: i64,
}

struct AssetBlueprint {
  material_efficiency: i64,
  runs: i64,
  time_efficiency: i64,
}

impl AssetBlueprint {
  /// EVE uses -1 as a sentinel for unlimited runs (original blueprints); any non-negative count is a blueprint copy.
  fn is_copy(&self) -> bool {
    self.runs != -1
  }
}

async fn asset_rows(db: &Database, owner: ReadOwner) -> Result<Vec<Value>, ToolError> {
  let fields = asset_fields(db, owner).await?;
  let blueprints = asset_blueprints(db, owner).await?;
  let mut ids = Vec::with_capacity(fields.len() * 2);
  for asset in &fields {
    ids.push(asset.type_id);
    ids.push(asset.location_id);
  }
  let names = resolve_names_map(db, &ids).await?;
  Ok(
    fields
      .iter()
      .map(|asset| asset_value(asset, &names, &blueprints))
      .collect(),
  )
}

async fn asset_fields(db: &Database, owner: ReadOwner) -> Result<Vec<AssetFields>, ToolError> {
  let fields = match owner {
    ReadOwner::Character(id) => assets::for_character(db, id)
      .await
      .map_err(internal)?
      .iter()
      .map(|a| AssetFields {
        is_blueprint_copy: a.is_blueprint_copy(),
        is_singleton: a.is_singleton(),
        item_id: a.item_id(),
        location_flag: a.location_flag().clone(),
        location_id: a.location_id(),
        name: a.name().clone(),
        quantity: a.quantity(),
        type_id: a.type_id(),
      })
      .collect(),
    ReadOwner::Corporation(id) => assets::for_corporation(db, id)
      .await
      .map_err(internal)?
      .iter()
      .map(|a| AssetFields {
        is_blueprint_copy: a.is_blueprint_copy(),
        is_singleton: a.is_singleton(),
        item_id: a.item_id(),
        location_flag: a.location_flag().clone(),
        location_id: a.location_id(),
        name: a.name().clone(),
        quantity: a.quantity(),
        type_id: a.type_id(),
      })
      .collect(),
  };
  Ok(fields)
}

async fn asset_blueprints(db: &Database, owner: ReadOwner) -> Result<HashMap<i64, AssetBlueprint>, ToolError> {
  let map = match owner {
    ReadOwner::Character(id) => blueprints::list_for_character(db, id)
      .await
      .map_err(internal)?
      .iter()
      .map(|b| {
        (
          b.item_id(),
          AssetBlueprint {
            material_efficiency: b.material_efficiency(),
            runs: b.runs(),
            time_efficiency: b.time_efficiency(),
          },
        )
      })
      .collect(),
    ReadOwner::Corporation(id) => blueprints::list_for_corporation(db, id)
      .await
      .map_err(internal)?
      .iter()
      .map(|b| {
        (
          b.item_id(),
          AssetBlueprint {
            material_efficiency: b.material_efficiency(),
            runs: b.runs(),
            time_efficiency: b.time_efficiency(),
          },
        )
      })
      .collect(),
  };
  Ok(map)
}

fn asset_value(
  asset: &AssetFields,
  names: &HashMap<i64, ResolvedName>,
  blueprints: &HashMap<i64, AssetBlueprint>,
) -> Value {
  let blueprint = blueprints.get(&asset.item_id);
  json!({
    "is_blueprint_copy": blueprint.map(AssetBlueprint::is_copy).or(asset.is_blueprint_copy),
    "is_singleton": asset.is_singleton,
    "item_id": asset.item_id,
    "location_flag": asset.location_flag,
    "location_id": asset.location_id,
    "location_name": name_of(names, asset.location_id),
    "material_efficiency": blueprint.map(|b| b.material_efficiency),
    "name": asset.name,
    "quantity": asset.quantity,
    "runs": blueprint.map(|b| b.runs),
    "time_efficiency": blueprint.map(|b| b.time_efficiency),
    "type_id": asset.type_id,
    "type_name": name_of(names, asset.type_id),
  })
}

async fn contract_rows(db: &Database, owner: ReadOwner) -> Result<Vec<Value>, ToolError> {
  match owner {
    ReadOwner::Character(id) => {
      let rows = finance::contracts(db, id).await.map_err(internal)?;
      Ok(
        rows
          .iter()
          .map(|c| {
            json!({
              "acceptor_name": c.acceptor_name,
              "collateral": c.collateral,
              "contract_id": c.contract_id,
              "date_issued": c.date_issued,
              "issuer_name": c.issuer_name,
              "price": c.price,
              "reward": c.reward,
              "status": c.status,
              "title": c.title,
              "type": c.r#type,
              "volume": c.volume,
            })
          })
          .collect(),
      )
    }
    ReadOwner::Corporation(id) => {
      let rows = finance::corporation_contracts(db, id).await.map_err(internal)?;
      Ok(
        rows
          .iter()
          .map(|c| {
            json!({
              "acceptor_name": c.acceptor_name(),
              "collateral": c.collateral(),
              "contract_id": c.contract_id(),
              "date_issued": c.date_issued(),
              "issuer_name": c.issuer_name(),
              "price": c.price(),
              "reward": c.reward(),
              "status": c.status(),
              "title": c.title(),
              "type": c.r#type(),
              "volume": c.volume(),
            })
          })
          .collect(),
      )
    }
  }
}

struct JournalFields {
  amount: Option<f64>,
  balance: Option<f64>,
  date: String,
  description: String,
  division: Option<i64>,
  first_party_id: Option<i64>,
  id: i64,
  reason: Option<String>,
  ref_type: String,
  second_party_id: Option<i64>,
}

async fn journal_rows(db: &Database, owner: ReadOwner) -> Result<Vec<Value>, ToolError> {
  let fields = journal_fields(db, owner).await?;
  let mut ids = Vec::new();
  for entry in &fields {
    ids.extend(entry.first_party_id);
    ids.extend(entry.second_party_id);
  }
  let names = resolve_names_map(db, &ids).await?;
  Ok(fields.iter().map(|entry| journal_value(entry, &names)).collect())
}

async fn journal_fields(db: &Database, owner: ReadOwner) -> Result<Vec<JournalFields>, ToolError> {
  let fields = match owner {
    ReadOwner::Character(id) => finance::wallet_journal(db, id)
      .await
      .map_err(internal)?
      .iter()
      .map(|e| JournalFields {
        amount: e.amount,
        balance: e.balance,
        date: e.date.clone(),
        description: e.description.clone(),
        division: None,
        first_party_id: e.first_party_id,
        id: e.id,
        reason: e.reason.clone(),
        ref_type: e.ref_type.clone(),
        second_party_id: e.second_party_id,
      })
      .collect(),
    ReadOwner::Corporation(id) => finance::corporation_wallet_journal_all_divisions(db, id)
      .await
      .map_err(internal)?
      .iter()
      .map(|e| JournalFields {
        amount: e.amount(),
        balance: e.balance(),
        date: e.date().clone(),
        description: e.description().clone(),
        division: Some(e.division()),
        first_party_id: e.first_party_id(),
        id: e.id(),
        reason: e.reason().clone(),
        ref_type: e.ref_type().clone(),
        second_party_id: e.second_party_id(),
      })
      .collect(),
  };
  Ok(fields)
}

fn journal_value(entry: &JournalFields, names: &HashMap<i64, ResolvedName>) -> Value {
  json!({
    "amount": entry.amount,
    "balance": entry.balance,
    "date": entry.date,
    "description": entry.description,
    "division": entry.division,
    "first_party_id": entry.first_party_id,
    "first_party_name": optional_name(names, entry.first_party_id),
    "id": entry.id,
    "reason": entry.reason,
    "ref_type": entry.ref_type,
    "second_party_id": entry.second_party_id,
    "second_party_name": optional_name(names, entry.second_party_id),
  })
}

struct TransactionFields {
  client_id: i64,
  date: String,
  division: Option<i64>,
  is_buy: bool,
  location_id: i64,
  quantity: i64,
  transaction_id: i64,
  type_id: i64,
  unit_price: f64,
}

async fn transaction_rows(db: &Database, owner: ReadOwner) -> Result<Vec<Value>, ToolError> {
  let fields = transaction_fields(db, owner).await?;
  let mut ids = Vec::with_capacity(fields.len() * 3);
  for entry in &fields {
    ids.push(entry.type_id);
    ids.push(entry.client_id);
    ids.push(entry.location_id);
  }
  let names = resolve_names_map(db, &ids).await?;
  Ok(fields.iter().map(|entry| transaction_value(entry, &names)).collect())
}

async fn transaction_fields(db: &Database, owner: ReadOwner) -> Result<Vec<TransactionFields>, ToolError> {
  let fields = match owner {
    ReadOwner::Character(id) => finance::wallet_transactions(db, id)
      .await
      .map_err(internal)?
      .iter()
      .map(|t| TransactionFields {
        client_id: t.client_id,
        date: t.date.clone(),
        division: None,
        is_buy: t.is_buy,
        location_id: t.location_id,
        quantity: t.quantity,
        transaction_id: t.transaction_id,
        type_id: t.type_id,
        unit_price: t.unit_price,
      })
      .collect(),
    ReadOwner::Corporation(id) => finance::corporation_wallet_transactions_all_divisions(db, id)
      .await
      .map_err(internal)?
      .iter()
      .map(|t| TransactionFields {
        client_id: t.client_id(),
        date: t.date().clone(),
        division: Some(t.division()),
        is_buy: t.is_buy(),
        location_id: t.location_id(),
        quantity: t.quantity(),
        transaction_id: t.transaction_id(),
        type_id: t.type_id(),
        unit_price: t.unit_price(),
      })
      .collect(),
  };
  Ok(fields)
}

fn transaction_value(entry: &TransactionFields, names: &HashMap<i64, ResolvedName>) -> Value {
  json!({
    "client_id": entry.client_id,
    "client_name": name_of(names, entry.client_id),
    "date": entry.date,
    "division": entry.division,
    "is_buy": entry.is_buy,
    "location_id": entry.location_id,
    "location_name": name_of(names, entry.location_id),
    "quantity": entry.quantity,
    "transaction_id": entry.transaction_id,
    "type_id": entry.type_id,
    "type_name": name_of(names, entry.type_id),
    "unit_price": entry.unit_price,
  })
}

struct BlueprintFields {
  location_flag: String,
  location_id: i64,
  material_efficiency: i64,
  quantity: i64,
  runs: i64,
  time_efficiency: i64,
  type_id: i64,
}

async fn blueprint_rows(db: &Database, owner: ReadOwner) -> Result<Vec<Value>, ToolError> {
  let fields = match owner {
    ReadOwner::Character(id) => blueprints::list_for_character(db, id)
      .await
      .map_err(internal)?
      .iter()
      .map(|b| BlueprintFields {
        location_flag: b.location_flag().clone(),
        location_id: b.location_id(),
        material_efficiency: b.material_efficiency(),
        quantity: b.quantity(),
        runs: b.runs(),
        time_efficiency: b.time_efficiency(),
        type_id: b.type_id(),
      })
      .collect::<Vec<_>>(),
    ReadOwner::Corporation(id) => blueprints::list_for_corporation(db, id)
      .await
      .map_err(internal)?
      .iter()
      .map(|b| BlueprintFields {
        location_flag: b.location_flag().clone(),
        location_id: b.location_id(),
        material_efficiency: b.material_efficiency(),
        quantity: b.quantity(),
        runs: b.runs(),
        time_efficiency: b.time_efficiency(),
        type_id: b.type_id(),
      })
      .collect::<Vec<_>>(),
  };
  let type_ids: Vec<i64> = fields.iter().map(|b| b.type_id).collect();
  let details = sde::type_details_for(db, &type_ids).await.map_err(internal)?;
  let names: HashMap<i64, String> = details.into_iter().map(|(id, name, _)| (id, name)).collect();
  Ok(fields.iter().map(|b| blueprint_value(b, &names)).collect())
}

fn blueprint_value(blueprint: &BlueprintFields, names: &HashMap<i64, String>) -> Value {
  json!({
    "is_blueprint_copy": blueprint.runs != -1,
    "location_flag": blueprint.location_flag,
    "location_id": blueprint.location_id,
    "material_efficiency": blueprint.material_efficiency,
    "quantity": blueprint.quantity,
    "runs": blueprint.runs,
    "time_efficiency": blueprint.time_efficiency,
    "type_id": blueprint.type_id,
    "type_name": names.get(&blueprint.type_id),
  })
}

fn market_order_value(order: &MarketOrder) -> Value {
  json!({
    "duration": order.duration(),
    "escrow": order.escrow(),
    "is_buy_order": order.is_buy_order(),
    "issued": order.issued(),
    "location_id": order.location_id(),
    "order_id": order.order_id(),
    "price": order.price(),
    "range": order.range(),
    "region_id": order.region_id(),
    "state": order.state(),
    "type_id": order.type_id(),
    "volume_remain": order.volume_remain(),
    "volume_total": order.volume_total(),
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    config::McpPerms,
    store::{
      Database,
      model::{PlanTree, PlanType},
    },
  };

  async fn database() -> Database {
    crate::store::open_test().await.expect("open a migrated test database")
  }

  fn deny_read() -> McpPerms {
    let mut perms = McpPerms::default();
    perms.set_read(false);
    perms
  }

  fn registry() -> crate::services::mcp::tool::Registry {
    let mut registry = crate::services::mcp::tool::Registry::default();
    for tool in tools() {
      registry.register(tool);
    }
    registry
  }

  async fn seed_plan(db: &Database, name: &str) -> i64 {
    let tree = PlanTree {
      product_type_id: 587,
      root_facility_system: Some(30_000_142),
      runs: 10,
      types: vec![PlanType {
        built: false,
        facility_structure: None,
        facility_system: Some(30_000_142),
        me: 10,
        te: 20,
        type_id: 587,
        use_stock: false,
      }],
    };
    industry::create_plan(db, name, &tree).await.unwrap().id()
  }

  mod input_schema {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::services::mcp::args::input_schema;

    fn tool(name: &str) -> McpTool {
      tools().into_iter().find(|t| t.name() == name).expect("tool exists")
    }

    fn schema(name: &str) -> Value {
      input_schema(tool(name).args())
    }

    #[test]
    fn list_journal_advertises_owner_and_pagination() {
      let schema = schema("list_journal");

      assert_eq!(schema["properties"]["owner_type"]["type"], "string");
      assert_eq!(schema["properties"]["owner_id"]["type"], "integer");
      assert_eq!(schema["properties"]["page"]["type"], "integer");
      assert_eq!(schema["properties"]["limit"]["type"], "integer");

      let required = schema["required"].as_array().unwrap();
      assert!(required.contains(&json!("owner_type")));
      assert!(required.contains(&json!("owner_id")));
      assert!(!required.contains(&json!("page")));
      assert!(!required.contains(&json!("limit")));
    }

    #[test]
    fn list_blueprints_advertises_owner_and_pagination() {
      let schema = schema("list_blueprints");

      assert_eq!(schema["properties"]["owner_type"]["type"], "string");
      assert_eq!(schema["properties"]["owner_id"]["type"], "integer");
      assert_eq!(schema["properties"]["page"]["type"], "integer");
      assert_eq!(schema["properties"]["limit"]["type"], "integer");

      let required = schema["required"].as_array().unwrap();
      assert!(required.contains(&json!("owner_type")));
      assert!(required.contains(&json!("owner_id")));
    }

    #[test]
    fn list_market_orders_advertises_a_character_and_pagination() {
      let schema = schema("list_market_orders");

      assert_eq!(schema["properties"]["character_id"]["type"], "integer");
      assert_eq!(schema["properties"]["page"]["type"], "integer");
      assert_eq!(schema["properties"]["limit"]["type"], "integer");

      let required = schema["required"].as_array().unwrap();
      assert!(required.contains(&json!("character_id")));
      assert!(!required.contains(&json!("page")));
    }

    #[test]
    fn list_corporations_advertises_no_properties() {
      let schema = schema("list_corporations");

      assert!(schema["properties"].as_object().unwrap().is_empty());
      assert!(schema["required"].as_array().unwrap().is_empty());
    }

    #[test]
    fn get_mail_body_requires_two_integer_ids() {
      let schema = schema("get_mail_body");

      assert_eq!(schema["properties"]["character_id"]["type"], "integer");
      assert_eq!(schema["properties"]["mail_id"]["type"], "integer");

      let required = schema["required"].as_array().unwrap();
      assert!(required.contains(&json!("character_id")));
      assert!(required.contains(&json!("mail_id")));
    }

    #[test]
    fn get_skills_advertises_a_single_required_integer() {
      let schema = schema("get_skills");

      assert_eq!(schema["properties"]["character_id"]["type"], "integer");
      assert!(schema["required"].as_array().unwrap().contains(&json!("character_id")));
    }

    #[test]
    fn get_budget_view_advertises_an_optional_string_month() {
      let schema = schema("get_budget_view");

      assert_eq!(schema["properties"]["month"]["type"], "string");
      assert!(!schema["properties"].as_object().unwrap().is_empty());
      assert!(!schema["required"].as_array().unwrap().contains(&json!("month")));
    }

    #[test]
    fn get_market_prices_advertises_an_optional_integer_array() {
      let schema = schema("get_market_prices");

      assert_eq!(schema["properties"]["type_ids"]["type"], "array");
      assert_eq!(schema["properties"]["type_ids"]["items"]["type"], "integer");
      assert!(!schema["required"].as_array().unwrap().contains(&json!("type_ids")));
    }

    #[test]
    fn get_live_market_advertises_type_ids_and_optional_scoping() {
      let schema = schema("get_live_market");

      assert_eq!(schema["properties"]["type_ids"]["type"], "array");
      assert_eq!(schema["properties"]["type_ids"]["items"]["type"], "integer");
      assert_eq!(schema["properties"]["region_id"]["type"], "integer");
      assert_eq!(schema["properties"]["location_id"]["type"], "integer");

      let required = schema["required"].as_array().unwrap();
      assert!(required.contains(&json!("type_ids")));
      assert!(!required.contains(&json!("region_id")));
      assert!(!required.contains(&json!("location_id")));
    }

    #[test]
    fn resolve_names_advertises_a_required_id_array() {
      let schema = schema("resolve_names");

      assert_eq!(schema["properties"]["ids"]["type"], "array");
      assert_eq!(schema["properties"]["ids"]["items"]["type"], "integer");
      assert!(schema["required"].as_array().unwrap().contains(&json!("ids")));
    }

    #[test]
    fn zero_arg_tools_advertise_no_properties() {
      let schema = schema("list_characters");

      assert!(schema["properties"].as_object().unwrap().is_empty());
      assert!(schema["required"].as_array().unwrap().is_empty());
    }
  }

  mod read_tools {
    use super::*;

    #[tokio::test]
    async fn each_tool_returns_structured_data_when_read_is_on() {
      let db = database().await;
      let perms = McpPerms::default();
      let registry = registry();

      for tool in registry.tools() {
        let outcome = registry
          .dispatch(
            tool.name(),
            &perms,
            db.clone(),
            json!({ "character_id": 1, "mail_id": 1 }),
          )
          .await;

        match outcome {
          Ok(value) => {
            assert!(value.is_object(), "{} returned a JSON object", tool.name())
          }
          Err(ToolError::InvalidArguments(_)) => {}
          other => panic!("{} should return data or an argument error: {other:?}", tool.name()),
        }
      }
    }

    #[tokio::test]
    async fn every_tool_is_denied_when_read_is_off() {
      let db = database().await;
      let registry = registry();

      for tool in registry.tools() {
        let outcome = registry
          .dispatch(tool.name(), &deny_read(), db.clone(), Value::Null)
          .await;

        assert!(
          matches!(outcome, Err(ToolError::PermissionDenied("read"))),
          "{} must be gated by read",
          tool.name()
        );
      }
    }

    #[tokio::test]
    async fn list_characters_returns_a_character_array() {
      let db = database().await;
      let registry = registry();

      let value = registry
        .dispatch("list_characters", &McpPerms::default(), db, Value::Null)
        .await
        .unwrap();

      assert!(value.get("characters").and_then(Value::as_array).is_some());
    }

    #[tokio::test]
    async fn list_corporations_returns_a_corporation_array() {
      let db = database().await;
      let registry = registry();

      let value = registry
        .dispatch("list_corporations", &McpPerms::default(), db, Value::Null)
        .await
        .unwrap();

      assert!(value.get("corporations").and_then(Value::as_array).is_some());
    }

    #[tokio::test]
    async fn list_blueprints_pages_an_owner() {
      let db = database().await;
      let registry = registry();

      let value = registry
        .dispatch(
          "list_blueprints",
          &McpPerms::default(),
          db,
          json!({ "owner_type": "character", "owner_id": 1 }),
        )
        .await
        .unwrap();

      assert!(value.get("blueprints").and_then(Value::as_array).is_some());
      assert_eq!(value.get("page").and_then(Value::as_i64), Some(0));
    }

    #[tokio::test]
    async fn list_market_orders_pages_a_character() {
      let db = database().await;
      let registry = registry();

      let value = registry
        .dispatch(
          "list_market_orders",
          &McpPerms::default(),
          db,
          json!({ "character_id": 1 }),
        )
        .await
        .unwrap();

      assert!(value.get("orders").and_then(Value::as_array).is_some());
    }
  }

  mod get_planner {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_lists_saved_plans_when_no_id_is_given() {
      let db = database().await;
      seed_plan(&db, "Rifter run").await;
      let registry = registry();

      let value = registry
        .dispatch("get_planner", &McpPerms::default(), db, Value::Null)
        .await
        .unwrap();

      let plans = value.get("plans").and_then(Value::as_array).expect("plans array");
      assert_eq!(plans.len(), 1);
      assert_eq!(plans[0].get("name").and_then(Value::as_str), Some("Rifter run"));
    }

    #[tokio::test]
    async fn it_returns_the_full_tree_for_one_plan() {
      let db = database().await;
      let plan_id = seed_plan(&db, "Rifter run").await;
      let registry = registry();

      let value = registry
        .dispatch("get_planner", &McpPerms::default(), db, json!({ "plan_id": plan_id }))
        .await
        .unwrap();

      assert_eq!(value.get("plan_id").and_then(Value::as_i64), Some(plan_id));
      assert_eq!(value.get("product_type_id").and_then(Value::as_i64), Some(587));
      assert_eq!(value.get("types").and_then(Value::as_array).map(Vec::len), Some(1));
      assert!(value.get("segments").and_then(Value::as_array).is_some());
    }

    #[tokio::test]
    async fn it_rejects_a_missing_plan_id() {
      let db = database().await;
      let registry = registry();

      let outcome = registry
        .dispatch("get_planner", &McpPerms::default(), db, json!({ "plan_id": 9999 }))
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod paginated_owner_args {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::services::mcp::args::input_schema;

    #[test]
    fn it_advertises_owner_type_owner_id_and_pagination() {
      let schema = input_schema(&super::super::paginated_owner_args());

      assert_eq!(schema["properties"]["owner_type"]["type"], "string");
      assert_eq!(schema["properties"]["owner_id"]["type"], "integer");
      assert_eq!(schema["properties"]["page"]["type"], "integer");
      assert_eq!(schema["properties"]["limit"]["type"], "integer");

      let required = schema["required"].as_array().unwrap();
      assert!(required.contains(&json!("owner_type")));
      assert!(required.contains(&json!("owner_id")));
      assert!(!required.contains(&json!("page")));
    }
  }

  mod require_owner {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::{Corporation, CorporationMemberRole, OwnerType},
      repo::{infra, org},
    };

    const CORP_ID: i64 = 90_000_777;

    const DIRECTOR_ID: i64 = 4242;

    async fn authorize_corp(db: &Database) {
      let mut corp = Corporation::new(CORP_ID, "Owner Corp", "OWN");
      corp.set_ceo_id(DIRECTOR_ID);
      corp.set_creator_id(DIRECTOR_ID);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      org::upsert_corporation(db, &corp).await.unwrap();
      infra::upsert(
        db,
        CORP_ID,
        OwnerType::Corporation,
        "tok",
        "rt",
        4_102_444_800,
        Some(DIRECTOR_ID),
        None,
      )
      .await
      .unwrap();
      org::replace_for_corporation(
        db,
        CORP_ID,
        &[CorporationMemberRole::from((
          CORP_ID,
          DIRECTOR_ID,
          "Director".to_owned(),
        ))],
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_resolves_a_character_owner() {
      let db = database().await;

      let owner = super::super::require_owner(&db, &json!({ "owner_type": "character", "owner_id": 7 }))
        .await
        .unwrap();

      assert_eq!(owner, Some(super::super::ReadOwner::Character(7)));
    }

    #[tokio::test]
    async fn it_resolves_an_authorized_corporation_owner() {
      let db = database().await;
      authorize_corp(&db).await;

      let owner = super::super::require_owner(&db, &json!({ "owner_type": "corporation", "owner_id": CORP_ID }))
        .await
        .unwrap();

      assert_eq!(owner, Some(super::super::ReadOwner::Corporation(CORP_ID)));
    }

    #[tokio::test]
    async fn it_yields_empty_for_an_unauthorized_corporation() {
      let db = database().await;

      let owner = super::super::require_owner(&db, &json!({ "owner_type": "corporation", "owner_id": CORP_ID }))
        .await
        .unwrap();

      assert_eq!(owner, None);
    }

    #[tokio::test]
    async fn it_rejects_an_unknown_owner_type() {
      let db = database().await;

      let outcome = super::super::require_owner(&db, &json!({ "owner_type": "alliance", "owner_id": 1 })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_rejects_the_smoke_test_fixture_without_panicking() {
      let db = database().await;

      let outcome = super::super::require_owner(&db, &json!({ "character_id": 1, "mail_id": 1 })).await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod row_builders {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::{
        Alliance, Bloodline, Character, CharacterAsset, CharacterBlueprint, CharacterContract, CharacterWalletJournal,
        CharacterWalletTransaction, Corporation, Gender, Race,
      },
      repo::{assets, blueprints, character::insert_with_org, finance},
    };

    const CID: i64 = 1;

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

    async fn seed_character(db: &Database) {
      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, CID, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(CID);
      corp.set_creator_id(CID);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(CID, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn asset_rows_maps_character_assets_and_empties_an_unowned_corp() {
      let db = database().await;
      seed_character(&db).await;
      seed_type(&db, 587, "Rifter").await;
      assets::replace_for_character(
        &db,
        CID,
        &[CharacterAsset {
          character_id: CID,
          container_id: None,
          depth: 0,
          is_active_ship: false,
          is_blueprint_copy: None,
          is_container: false,
          is_singleton: true,
          item_id: 1_000,
          location_flag: "Hangar".to_owned(),
          location_id: 60_003_760,
          location_type: "station".to_owned(),
          name: Some("Rifter".to_owned()),
          quantity: 3,
          type_id: 587,
        }],
      )
      .await
      .unwrap();
      blueprints::replace_for_character(
        &db,
        CID,
        &[CharacterBlueprint {
          character_id: CID,
          item_id: 1_000,
          location_flag: "Hangar".to_owned(),
          location_id: 60_003_760,
          material_efficiency: 10,
          quantity: -1,
          runs: -1,
          time_efficiency: 20,
          type_id: 587,
        }],
      )
      .await
      .unwrap();

      let rows = super::super::asset_rows(&db, super::super::ReadOwner::Character(CID))
        .await
        .unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0]["item_id"].as_i64(), Some(1_000));
      assert_eq!(rows[0]["quantity"].as_i64(), Some(3));
      assert_eq!(rows[0]["type_name"].as_str(), Some("Rifter"));
      assert_eq!(rows[0]["is_blueprint_copy"].as_bool(), Some(false));
      assert_eq!(rows[0]["runs"].as_i64(), Some(-1));
      assert_eq!(rows[0]["material_efficiency"].as_i64(), Some(10));
      assert_eq!(rows[0]["time_efficiency"].as_i64(), Some(20));

      let corp = super::super::asset_rows(&db, super::super::ReadOwner::Corporation(999))
        .await
        .unwrap();
      assert!(corp.is_empty());
    }

    #[tokio::test]
    async fn contract_rows_maps_character_contracts_and_empties_an_unowned_corp() {
      let db = database().await;
      seed_character(&db).await;
      finance::replace_for_character(
        &db,
        CID,
        &[CharacterContract {
          acceptor_id: None,
          acceptor_name: None,
          assignee_id: None,
          assignee_name: None,
          availability: Some("public".to_owned()),
          character_id: CID,
          collateral: Some(1.0),
          contract_id: 42,
          date_accepted: None,
          date_completed: None,
          date_expired: None,
          date_issued: "2026-01-01T00:00:00Z".to_owned(),
          days_to_complete: None,
          end_location_id: None,
          for_corporation: false,
          issuer_corporation_id: None,
          issuer_id: 7,
          issuer_name: Some("Pilot".to_owned()),
          price: Some(100.0),
          reward: Some(0.0),
          start_location_id: None,
          status: "outstanding".to_owned(),
          title: Some("Haul".to_owned()),
          r#type: "courier".to_owned(),
          volume: Some(5.0),
        }],
      )
      .await
      .unwrap();

      let rows = super::super::contract_rows(&db, super::super::ReadOwner::Character(CID))
        .await
        .unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0]["contract_id"].as_i64(), Some(42));

      let corp = super::super::contract_rows(&db, super::super::ReadOwner::Corporation(999))
        .await
        .unwrap();
      assert!(corp.is_empty());
    }

    #[tokio::test]
    async fn journal_rows_maps_character_journal_and_empties_an_unowned_corp() {
      let db = database().await;
      seed_character(&db).await;
      finance::append_wallet_journal(
        &db,
        &[CharacterWalletJournal {
          amount: Some(10.0),
          balance: Some(20.0),
          character_id: CID,
          context_id: None,
          context_id_type: None,
          date: "2026-01-01T00:00:00Z".to_owned(),
          description: "bounty".to_owned(),
          first_party_id: Some(CID),
          id: 99,
          reason: None,
          ref_type: "bounty_prizes".to_owned(),
          second_party_id: None,
          tax: None,
          tax_receiver_id: None,
        }],
      )
      .await
      .unwrap();

      let rows = super::super::journal_rows(&db, super::super::ReadOwner::Character(CID))
        .await
        .unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0]["id"].as_i64(), Some(99));
      assert_eq!(rows[0]["first_party_name"].as_str(), Some("Pilot"));

      let corp = super::super::journal_rows(&db, super::super::ReadOwner::Corporation(999))
        .await
        .unwrap();
      assert!(corp.is_empty());
    }

    #[tokio::test]
    async fn transaction_rows_maps_character_transactions_and_empties_an_unowned_corp() {
      let db = database().await;
      seed_character(&db).await;
      seed_type(&db, 587, "Rifter").await;
      finance::append_wallet_transaction(
        &db,
        &[CharacterWalletTransaction {
          character_id: CID,
          client_id: CID,
          date: "2026-01-01T00:00:00Z".to_owned(),
          is_buy: true,
          is_personal: true,
          journal_ref_id: 1,
          location_id: 60_003_760,
          quantity: 4,
          transaction_id: 555,
          type_id: 587,
          unit_price: 12.5,
        }],
      )
      .await
      .unwrap();

      let rows = super::super::transaction_rows(&db, super::super::ReadOwner::Character(CID))
        .await
        .unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0]["transaction_id"].as_i64(), Some(555));
      assert_eq!(rows[0]["type_name"].as_str(), Some("Rifter"));
      assert_eq!(rows[0]["client_name"].as_str(), Some("Pilot"));

      let corp = super::super::transaction_rows(&db, super::super::ReadOwner::Corporation(999))
        .await
        .unwrap();
      assert!(corp.is_empty());
    }

    #[tokio::test]
    async fn blueprint_rows_maps_character_blueprints_and_empties_an_unowned_corp() {
      let db = database().await;
      seed_character(&db).await;
      blueprints::replace_for_character(
        &db,
        CID,
        &[CharacterBlueprint {
          character_id: CID,
          item_id: 1_000,
          location_flag: "Hangar".to_owned(),
          location_id: 60_003_760,
          material_efficiency: 10,
          quantity: -1,
          runs: -1,
          time_efficiency: 20,
          type_id: 587,
        }],
      )
      .await
      .unwrap();

      let rows = super::super::blueprint_rows(&db, super::super::ReadOwner::Character(CID))
        .await
        .unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0]["type_id"].as_i64(), Some(587));
      assert_eq!(rows[0]["material_efficiency"].as_i64(), Some(10));
      assert_eq!(rows[0]["is_blueprint_copy"].as_bool(), Some(false));

      let corp = super::super::blueprint_rows(&db, super::super::ReadOwner::Corporation(999))
        .await
        .unwrap();
      assert!(corp.is_empty());
    }
  }

  mod name_enrichment {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      services::mcp::names::{NameKind, ResolvedName},
      store::model::{CharacterIndustryJob, CharacterSkill, CharacterSkillqueue},
    };

    fn typed(id: i64, name: &str) -> (i64, ResolvedName) {
      (
        id,
        ResolvedName {
          kind: NameKind::Type,
          name: name.to_owned(),
        },
      )
    }

    fn names() -> HashMap<i64, ResolvedName> {
      [
        typed(3300, "Gunnery"),
        typed(587, "Rifter"),
        typed(588, "Rifter Blueprint"),
      ]
      .into_iter()
      .collect()
    }

    #[test]
    fn skill_value_emits_the_skill_name() {
      let skill = CharacterSkill {
        active_skill_level: 5,
        character_id: 1,
        skill_id: 3300,
        skillpoints_in_skill: 256_000,
        trained_skill_level: 5,
      };

      let value = super::super::skill_value(&skill, &names());

      assert_eq!(value["skill_id"].as_i64(), Some(3300));
      assert_eq!(value["skill_name"].as_str(), Some("Gunnery"));
    }

    #[test]
    fn skillqueue_value_emits_the_skill_name() {
      let entry = CharacterSkillqueue {
        character_id: 1,
        finish_date: None,
        finished_level: 5,
        level_end_sp: None,
        level_start_sp: None,
        queue_position: 0,
        skill_id: 3300,
        start_date: None,
        training_start_sp: None,
      };

      let value = super::super::skillqueue_value(&entry, &names());

      assert_eq!(value["skill_id"].as_i64(), Some(3300));
      assert_eq!(value["skill_name"].as_str(), Some("Gunnery"));
    }

    #[test]
    fn character_job_value_emits_blueprint_and_product_names() {
      let job = CharacterIndustryJob {
        activity_id: 1,
        blueprint_id: 1,
        blueprint_location_id: 60_003_760,
        blueprint_type_id: 588,
        character_id: 1,
        completed_character_id: None,
        completed_date: None,
        cost: None,
        duration: 3_600,
        end_date: "2026-01-01T01:00:00Z".to_owned(),
        facility_id: 60_003_760,
        installer_id: 1,
        job_id: 42,
        licensed_runs: None,
        output_location_id: 60_003_760,
        pause_date: None,
        probability: None,
        product_type_id: Some(587),
        runs: 1,
        start_date: "2026-01-01T00:00:00Z".to_owned(),
        station_id: None,
        status: "active".to_owned(),
        successful_runs: None,
      };

      let value = super::super::character_job_value(&job, &names());

      assert_eq!(value["blueprint_type_name"].as_str(), Some("Rifter Blueprint"));
      assert_eq!(value["product_type_name"].as_str(), Some("Rifter"));
    }
  }

  mod owner_type_docs {
    #[test]
    fn it_documents_the_corporation_owner_and_empty_result() {
      let doc = t!("mcp.tools.shared_arg_owner_type");

      assert!(doc.contains("corporation"), "{doc}");
      assert!(doc.to_lowercase().contains("empty"), "{doc}");
    }
  }

  mod live_market {
    use pretty_assertions::assert_eq;

    use crate::clients::esi::models::market::{MarketHistory, RegionOrder};

    fn order(is_buy_order: bool, location_id: i64, price: f64, volume_remain: i64) -> RegionOrder {
      RegionOrder {
        is_buy_order,
        location_id,
        price,
        type_id: 34,
        volume_remain,
        ..Default::default()
      }
    }

    fn day(date: &str, volume: i64) -> MarketHistory {
      MarketHistory {
        average: 5.0,
        date: date.to_owned(),
        highest: 6.0,
        lowest: 4.0,
        order_count: 10,
        volume,
      }
    }

    #[test]
    fn best_order_picks_the_highest_buy_at_the_location() {
      let orders = [
        order(true, 60_003_760, 5.0, 100),
        order(true, 60_003_760, 7.5, 200),
        order(true, 999, 9.0, 300),
        order(false, 60_003_760, 8.0, 400),
      ];

      let best = super::super::best_order(&orders, 60_003_760, true).unwrap();

      assert_eq!(best.price, 7.5);
      assert_eq!(best.volume_remain, 200);
    }

    #[test]
    fn best_order_picks_the_lowest_sell_at_the_location() {
      let orders = [
        order(false, 60_003_760, 8.0, 100),
        order(false, 60_003_760, 6.5, 200),
        order(false, 999, 0.1, 300),
        order(true, 60_003_760, 5.0, 400),
      ];

      let best = super::super::best_order(&orders, 60_003_760, false).unwrap();

      assert_eq!(best.price, 6.5);
      assert_eq!(best.volume_remain, 200);
    }

    #[test]
    fn best_order_is_none_when_nothing_matches_the_location() {
      let orders = [order(false, 999, 5.0, 100)];

      assert!(super::super::best_order(&orders, 60_003_760, false).is_none());
    }

    #[test]
    fn latest_history_picks_the_newest_day() {
      let history = [day("2026-06-27", 100), day("2026-06-29", 300), day("2026-06-28", 200)];

      let latest = super::super::latest_history(&history).unwrap();

      assert_eq!(latest.date, "2026-06-29");
      assert_eq!(latest.volume, 300);
    }

    #[test]
    fn market_row_value_shapes_best_buy_sell_and_daily_volume() {
      let buy = [order(true, 60_003_760, 7.5, 200)];
      let sell = [order(false, 60_003_760, 9.0, 150)];
      let history = [day("2026-06-28", 18_000), day("2026-06-29", 22_000)];

      let value = super::super::market_row_value(34, Some("Tritanium"), &buy, &sell, &history, 60_003_760);

      assert_eq!(value["type_id"].as_i64(), Some(34));
      assert_eq!(value["type_name"].as_str(), Some("Tritanium"));
      assert_eq!(value["best_buy"].as_f64(), Some(7.5));
      assert_eq!(value["best_buy_volume"].as_i64(), Some(200));
      assert_eq!(value["best_sell"].as_f64(), Some(9.0));
      assert_eq!(value["best_sell_volume"].as_i64(), Some(150));
      assert_eq!(value["daily_volume"].as_i64(), Some(22_000));
      assert_eq!(value["daily"]["date"].as_str(), Some("2026-06-29"));
      assert_eq!(value["daily"]["order_count"].as_i64(), Some(10));
    }

    #[test]
    fn market_row_value_is_null_where_no_orders_or_history_exist() {
      let value = super::super::market_row_value(34, None, &[], &[], &[], 60_003_760);

      assert!(value["best_buy"].is_null());
      assert!(value["best_sell"].is_null());
      assert!(value["daily"].is_null());
      assert!(value["daily_volume"].is_null());
    }
  }

  mod resolve_names {
    use pretty_assertions::assert_eq;

    use super::*;

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

    async fn seed_region(db: &Database, id: i64, name: &str) {
      sqlx::query("INSERT INTO regions (id, description, name) VALUES (?, NULL, ?)")
        .bind(id)
        .bind(name)
        .execute(db.writer())
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_resolves_a_mixed_local_batch_to_a_flat_map() {
      let db = database().await;
      seed_type(&db, 587, "Rifter").await;
      seed_region(&db, 10_000_002, "The Forge").await;
      let registry = registry();

      let value = registry
        .dispatch(
          "resolve_names",
          &McpPerms::default(),
          db,
          json!({ "ids": [587, 10_000_002] }),
        )
        .await
        .unwrap();

      let names = value.get("names").expect("names map");
      assert_eq!(names["587"]["name"].as_str(), Some("Rifter"));
      assert_eq!(names["587"]["kind"].as_str(), Some("type"));
      assert_eq!(names["10000002"]["name"].as_str(), Some("The Forge"));
      assert_eq!(names["10000002"]["kind"].as_str(), Some("location"));
    }
  }

  mod esi_helpers {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;
    use crate::clients::{esi, http};

    async fn make_esi(base_url: &str) -> esi::Client {
      let http = http::Client::builder(http::Cache::new(crate::store::open_test().await.unwrap())).build();
      esi::Client::with_base_url(http, base_url)
    }

    #[tokio::test]
    async fn public_esi_builds_a_client() {
      let db = database().await;

      assert!(super::super::public_esi(&db).is_ok());
    }

    #[tokio::test]
    async fn resolve_parties_via_esi_maps_ids_through_universe_names() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
          { "category": "character", "id": 95_465_499, "name": "CCP Bartender" }
        ])))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let resolved = super::super::resolve_parties_via_esi(&esi, vec![95_465_499])
        .await
        .unwrap();

      assert_eq!(resolved[&95_465_499].name, "CCP Bartender");
      assert_eq!(resolved[&95_465_499].category, "character");
    }

    #[tokio::test]
    async fn resolve_parties_via_esi_tolerates_a_404_batch() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let resolved = super::super::resolve_parties_via_esi(&esi, vec![1, 2]).await.unwrap();

      assert!(resolved.is_empty());
    }
  }
}
