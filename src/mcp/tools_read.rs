use serde_json::{Value, json};

use crate::{
  features::wallet::budget,
  mcp::{
    args::{ArgSpec, DEFAULT_LIMIT, paginate_vec, pagination, require_i64},
    tool::{McpTool, Permission, ToolError},
  },
  store::{
    model::{BudgetScope, CharacterMail},
    repo::{assets, character, finance, industry, mail, org},
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
    list_mail_tool(),
    get_mail_body_tool(),
    get_market_prices_tool(),
  ]
}

fn list_characters_tool() -> McpTool {
  McpTool::new(
    "list_characters",
    "Lists every owned character: id, name, corporation, total skill points, and wallet balance.",
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
    "Returns per-character liquid/asset/net-worth figures and every owned corporation's wallet divisions.",
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
    "Pages a character's wallet journal (most recent first).",
    Permission::Read,
    |db, args: Value| async move {
      let character_id = require_i64(&args, "character_id")?;
      let (page, limit) = pagination(&args);
      let mut rows = finance::wallet_journal(&db, character_id).await.map_err(internal)?;
      let (slice, has_more) = paginate_vec(&mut rows, page, limit);
      let entries: Vec<Value> = slice
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
        .collect();
      Ok(json!({ "entries": entries, "has_more": has_more, "page": page }))
    },
  )
  .with_args(paginated_character_args())
}

fn list_market_transactions_tool() -> McpTool {
  McpTool::new(
    "list_market_transactions",
    "Pages a character's market transactions (most recent first).",
    Permission::Read,
    |db, args: Value| async move {
      let character_id = require_i64(&args, "character_id")?;
      let (page, limit) = pagination(&args);
      let mut rows = finance::wallet_transactions(&db, character_id)
        .await
        .map_err(internal)?;
      let (slice, has_more) = paginate_vec(&mut rows, page, limit);
      let entries: Vec<Value> = slice
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
        .collect();
      Ok(json!({ "has_more": has_more, "page": page, "transactions": entries }))
    },
  )
  .with_args(paginated_character_args())
}

fn list_contracts_tool() -> McpTool {
  McpTool::new(
    "list_contracts",
    "Pages a character's contracts (most recent first).",
    Permission::Read,
    |db, args: Value| async move {
      let character_id = require_i64(&args, "character_id")?;
      let (page, limit) = pagination(&args);
      let mut rows = finance::contracts(&db, character_id).await.map_err(internal)?;
      let (slice, has_more) = paginate_vec(&mut rows, page, limit);
      let entries: Vec<Value> = slice
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
        .collect();
      Ok(json!({ "contracts": entries, "has_more": has_more, "page": page }))
    },
  )
  .with_args(paginated_character_args())
}

fn get_budget_view_tool() -> McpTool {
  McpTool::new(
    "get_budget_view",
    "Returns the global budget envelopes, assignments, and Ready-to-Assign for a month. Args: month (YYYY-MM, \
      defaults to current).",
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
}

fn get_skills_tool() -> McpTool {
  McpTool::new(
    "get_skills",
    "Returns a character's trained skills and total skill points.",
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
    "Returns a character's skill training queue in position order.",
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
    "Returns every character and authorized-corporation industry job.",
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
    "Lists saved industry plans, or returns one plan's full type tree and segments.",
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
    "An industry plan id; omit to list every saved plan.",
  )])
}

fn list_assets_tool() -> McpTool {
  McpTool::new(
    "list_assets",
    "Pages a character's asset holdings.",
    Permission::Read,
    |db, args: Value| async move {
      let character_id = require_i64(&args, "character_id")?;
      let (page, limit) = pagination(&args);
      let mut rows = assets::for_character(&db, character_id).await.map_err(internal)?;
      let (slice, has_more) = paginate_vec(&mut rows, page, limit);
      let entries: Vec<Value> = slice
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
        .collect();
      Ok(json!({ "assets": entries, "has_more": has_more, "page": page }))
    },
  )
  .with_args(paginated_character_args())
}

fn list_mail_tool() -> McpTool {
  McpTool::new(
    "list_mail",
    "Pages a character's mail headers (most recent first).",
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
    "Returns a single mail's header, recipients, labels, and full body.",
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
    ArgSpec::integer("mail_id", "The mail id whose body to fetch."),
  ])
}

fn get_market_prices_tool() -> McpTool {
  McpTool::new(
    "get_market_prices",
    "Returns the canonical per-type market prices (adjusted/average). Optional args: type_ids (array).",
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
}

fn internal(error: impl std::fmt::Display) -> ToolError {
  ToolError::Internal(error.to_string())
}

fn character_id_arg() -> ArgSpec {
  ArgSpec::integer("character_id", "The character whose data to read.")
}

fn paginated_character_args() -> [ArgSpec; 3] {
  [
    character_id_arg(),
    ArgSpec::optional_integer("page", 0, "Zero-based page index (defaults to 0)."),
    ArgSpec::optional_integer(
      "limit",
      DEFAULT_LIMIT,
      "Maximum rows per page (1..=500, defaults to 50).",
    ),
  ]
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    config::McpPerms,
    store::{
      Database,
      repo::industry::{PlanTree, PlanType},
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
    fn list_journal_advertises_character_id_and_pagination() {
      let schema = schema("list_journal");

      assert_eq!(schema["properties"]["character_id"]["type"], "integer");
      assert_eq!(schema["properties"]["page"]["type"], "integer");
      assert_eq!(schema["properties"]["limit"]["type"], "integer");

      let required = schema["required"].as_array().unwrap();
      assert!(required.contains(&json!("character_id")));
      assert!(!required.contains(&json!("page")));
      assert!(!required.contains(&json!("limit")));
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
}
