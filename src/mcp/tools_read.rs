use std::collections::HashMap;

use serde_json::{Value, json};

use crate::{
  features::wallet::budget,
  mcp::{
    args::{ArgSpec, DEFAULT_LIMIT, paginate_vec, pagination, require_i64, require_str},
    tool::{McpTool, Permission, ToolError},
  },
  store::{
    Database,
    model::{BudgetScope, CharacterMail, MarketOrder},
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
      let view = budget::load(&db, BudgetScope::All, &month).await;
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
      let skills = character::skills(&db, character_id).await.map_err(internal)?;
      let state = character::state(&db, character_id).await.map_err(internal)?;
      let rows: Vec<Value> = skills
        .iter()
        .map(|s| {
          json!({
            "active_skill_level": s.active_skill_level(),
            "skill_id": s.skill_id(),
            "skillpoints_in_skill": s.skillpoints_in_skill(),
            "trained_skill_level": s.trained_skill_level(),
          })
        })
        .collect();
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
      let rows: Vec<Value> = queue
        .iter()
        .map(|e| {
          json!({
            "finish_date": e.finish_date(),
            "finished_level": e.finished_level(),
            "queue_position": e.queue_position(),
            "skill_id": e.skill_id(),
            "start_date": e.start_date(),
          })
        })
        .collect();
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
      let character_jobs: Vec<Value> = jobs
        .character_jobs
        .iter()
        .map(|j| {
          json!({
            "activity_id": j.activity_id,
            "blueprint_type_id": j.blueprint_type_id,
            "character_id": j.character_id,
            "end_date": j.end_date,
            "job_id": j.job_id,
            "product_type_id": j.product_type_id,
            "runs": j.runs,
            "start_date": j.start_date,
            "status": j.status,
          })
        })
        .collect();
      let corporation_jobs: Vec<Value> = jobs
        .corporation_jobs
        .iter()
        .map(|j| {
          json!({
            "activity_id": j.activity_id,
            "blueprint_type_id": j.blueprint_type_id,
            "corporation_id": j.corporation_id,
            "end_date": j.end_date,
            "job_id": j.job_id,
            "product_type_id": j.product_type_id,
            "runs": j.runs,
            "start_date": j.start_date,
            "status": j.status,
          })
        })
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

fn internal(error: impl std::fmt::Display) -> ToolError {
  ToolError::Internal(error.to_string())
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

async fn asset_rows(db: &Database, owner: ReadOwner) -> Result<Vec<Value>, ToolError> {
  match owner {
    ReadOwner::Character(id) => {
      let rows = assets::for_character(db, id).await.map_err(internal)?;
      Ok(
        rows
          .iter()
          .map(|a| {
            json!({
              "is_singleton": a.is_singleton(),
              "item_id": a.item_id(),
              "location_flag": a.location_flag(),
              "location_id": a.location_id(),
              "name": a.name(),
              "quantity": a.quantity(),
              "type_id": a.type_id(),
            })
          })
          .collect(),
      )
    }
    ReadOwner::Corporation(id) => {
      let rows = assets::for_corporation(db, id).await.map_err(internal)?;
      Ok(
        rows
          .iter()
          .map(|a| {
            json!({
              "is_singleton": a.is_singleton(),
              "item_id": a.item_id(),
              "location_flag": a.location_flag(),
              "location_id": a.location_id(),
              "name": a.name(),
              "quantity": a.quantity(),
              "type_id": a.type_id(),
            })
          })
          .collect(),
      )
    }
  }
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

async fn journal_rows(db: &Database, owner: ReadOwner) -> Result<Vec<Value>, ToolError> {
  match owner {
    ReadOwner::Character(id) => {
      let rows = finance::wallet_journal(db, id).await.map_err(internal)?;
      Ok(
        rows
          .iter()
          .map(|e| {
            json!({
              "amount": e.amount,
              "balance": e.balance,
              "date": e.date,
              "description": e.description,
              "first_party_id": e.first_party_id,
              "id": e.id,
              "reason": e.reason,
              "ref_type": e.ref_type,
              "second_party_id": e.second_party_id,
            })
          })
          .collect(),
      )
    }
    ReadOwner::Corporation(id) => {
      let rows = finance::corporation_wallet_journal_all_divisions(db, id)
        .await
        .map_err(internal)?;
      Ok(
        rows
          .iter()
          .map(|e| {
            json!({
              "amount": e.amount(),
              "balance": e.balance(),
              "date": e.date(),
              "description": e.description(),
              "division": e.division(),
              "first_party_id": e.first_party_id(),
              "id": e.id(),
              "reason": e.reason(),
              "ref_type": e.ref_type(),
              "second_party_id": e.second_party_id(),
            })
          })
          .collect(),
      )
    }
  }
}

async fn transaction_rows(db: &Database, owner: ReadOwner) -> Result<Vec<Value>, ToolError> {
  match owner {
    ReadOwner::Character(id) => {
      let rows = finance::wallet_transactions(db, id).await.map_err(internal)?;
      Ok(
        rows
          .iter()
          .map(|t| {
            json!({
              "client_id": t.client_id,
              "date": t.date,
              "is_buy": t.is_buy,
              "location_id": t.location_id,
              "quantity": t.quantity,
              "transaction_id": t.transaction_id,
              "type_id": t.type_id,
              "unit_price": t.unit_price,
            })
          })
          .collect(),
      )
    }
    ReadOwner::Corporation(id) => {
      let rows = finance::corporation_wallet_transactions_all_divisions(db, id)
        .await
        .map_err(internal)?;
      Ok(
        rows
          .iter()
          .map(|t| {
            json!({
              "client_id": t.client_id(),
              "date": t.date(),
              "division": t.division(),
              "is_buy": t.is_buy(),
              "location_id": t.location_id(),
              "quantity": t.quantity(),
              "transaction_id": t.transaction_id(),
              "type_id": t.type_id(),
              "unit_price": t.unit_price(),
            })
          })
          .collect(),
      )
    }
  }
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

  fn registry() -> crate::mcp::tool::Registry {
    let mut registry = crate::mcp::tool::Registry::default();
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
    use crate::mcp::args::input_schema;

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
          Ok(value) => assert!(value.is_object(), "{} returned a JSON object", tool.name()),
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
    use crate::mcp::args::input_schema;

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
}
