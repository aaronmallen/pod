//! The embedded MCP server: a localhost, bearer-authenticated automation surface an external agent
//! connects to over HTTP/JSON-RPC to drive Pod.
//!
//! Off by default ([`crate::config::McpConfig`]). When enabled, [`Server`] runs a `127.0.0.1`
//! listener ([`server`], [`transport`]) speaking a small JSON-RPC slice ([`protocol`]). A `tools/call`
//! is routed into the iced update loop through the [`bridge`] — mirroring the deep-link bridge — so
//! tools run on the thread that owns the live database, gated by the typed catalog and permission
//! gate in [`tool`]. A write tool raises [`reload::signal`] so open views refresh.
//!
//! This module is the epic linchpin: the read/write/mail specs register their tools on the
//! [`Registry`] built by [`registry`] and consume the [`bridge::McpRequest`] / [`tool`] API here.

pub mod args;
pub mod bridge;
pub mod names;
pub mod protocol;
pub mod reload;
pub mod server;
pub mod tool;
pub mod tools_captains_log;
pub mod tools_dossier;
pub mod tools_mail;
pub mod tools_read;
pub mod tools_write;
pub mod transport;

use serde_json::{Value, json};

pub use self::{bridge::McpRequest, server::Server, tool::Registry};
use crate::{
  config::McpConfig,
  services::mcp::tool::{McpTool, Permission, ToolError},
  store::Database,
};

/// Builds the tool catalog the server exposes: the `ping` smoke tool plus the read, local-write, and
/// mail tool families ([`tools_read`], [`tools_write`], [`tools_mail`]). Each tool carries the
/// [`Permission`] the gate enforces before its handler runs.
pub fn registry() -> Registry {
  let mut registry = Registry::default().with(ping_tool());
  for tool in tools_read::tools() {
    registry.register(tool);
  }
  for tool in tools_captains_log::tools() {
    registry.register(tool);
  }
  for tool in tools_dossier::tools() {
    registry.register(tool);
  }
  for tool in tools_write::tools() {
    registry.register(tool);
  }
  for tool in tools_mail::tools() {
    registry.register(tool);
  }
  registry
}

/// Builds the app-held [`Server`] over the tool catalog. The caller drives its lifecycle with
/// [`Server::apply`] against the live [`McpConfig`]; tools run in the update loop over the app's
/// live database via the bridge, so the server itself holds none.
pub fn server() -> Server {
  Server::new(registry())
}

/// Runs a bridged tool call to completion against the live config and database, replying to the
/// waiting agent through the request's one-shot. This is the update-loop side of the bridge: it
/// re-checks the request against the *real* [`McpConfig::perms`] (the authoritative gate) before
/// running the tool, then fires [`reload::signal`] when a write tool reports it changed data.
///
/// Returns the work as a future the caller spawns; it produces no UI message of its own.
pub async fn fulfill(request: McpRequest, registry: Registry, config: McpConfig, db: Database) {
  let outcome = registry
    .dispatch(request.tool(), config.perms(), db, request.args().clone())
    .await;
  if outcome.is_ok() && writes(&registry, request.tool()) {
    reload::signal();
  }
  request.reply(outcome);
}

/// Whether the named tool requires a mutating permission, so a successful call should raise the
/// reload signal. Read tools leave the GUI untouched and skip it.
fn writes(registry: &Registry, tool: &str) -> bool {
  registry
    .get(tool)
    .is_some_and(|tool| !matches!(tool.permission(), Permission::Read))
}

fn ping_tool() -> McpTool {
  McpTool::new(
    "ping",
    t!("mcp.tools.ping").into_owned(),
    Permission::Read,
    |_db, args: Value| async move {
      let message = args.get("message").and_then(Value::as_str).unwrap_or("pong");
      if message.len() > 4096 {
        return Err(ToolError::InvalidArguments("message is too long".to_owned()));
      }
      Ok(json!({ "pong": true, "message": message }))
    },
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  async fn database() -> Database {
    let dir = tempfile::tempdir().expect("temp dir for an isolated database");
    crate::store::open(&dir.path().join("pod.db"))
      .await
      .expect("open an empty migrated database")
  }

  mod registry {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_ships_the_ping_smoke_tool_alongside_the_tool_families() {
      let names: Vec<&str> = super::registry().tools().map(McpTool::name).collect();

      assert!(names.contains(&"ping"));
      assert!(names.contains(&"list_characters"));
      assert!(names.contains(&"budget_assign_category"));
      assert!(names.contains(&"send_mail"));
    }

    #[test]
    fn its_tool_names_are_unique() {
      let names: Vec<&str> = super::registry().tools().map(McpTool::name).collect();
      let mut unique = names.clone();
      unique.sort_unstable();
      unique.dedup();

      assert_eq!(names.len(), unique.len());
    }

    #[test]
    fn ping_is_a_read_tool() {
      assert!(matches!(
        super::registry().get("ping").map(McpTool::permission),
        Some(Permission::Read)
      ));
    }
  }

  mod fulfill {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_runs_the_smoke_tool_and_replies_through_the_bridge() {
      let (request, rx) = McpRequest::new("ping".to_owned(), json!({ "message": "hi" }));

      fulfill(request, super::registry(), McpConfig::default(), database().await).await;

      let value = rx.await.unwrap().unwrap();
      assert_eq!(value, json!({ "pong": true, "message": "hi" }));
    }

    #[tokio::test]
    async fn it_replies_with_permission_denied_when_the_gate_is_off() {
      let mut config = McpConfig::default();
      let mut perms = *config.perms();
      perms.set_read(false);
      config.set_perms(perms);
      let (request, rx) = McpRequest::new("ping".to_owned(), Value::Null);

      fulfill(request, super::registry(), config, database().await).await;

      assert!(
        matches!(rx.await.unwrap(), Err(ToolError::PermissionDenied("read"))),
        "the gate refuses the call against the live config"
      );
    }

    #[tokio::test]
    async fn it_reports_an_unknown_tool() {
      let (request, rx) = McpRequest::new("nope".to_owned(), Value::Null);

      fulfill(request, super::registry(), McpConfig::default(), database().await).await;

      assert!(matches!(rx.await.unwrap(), Err(ToolError::UnknownTool(name)) if name == "nope"));
    }

    #[tokio::test]
    async fn it_persists_a_captains_log_narrative_through_the_bridge() {
      let db = database().await;
      let (request, rx) = McpRequest::new(
        "captains_log_set_narrative".to_owned(),
        json!({ "date": "2026-07-05", "text": "Filed by the agent." }),
      );

      fulfill(request, super::registry(), McpConfig::default(), db.clone()).await;

      let value = rx.await.unwrap().unwrap();
      assert_eq!(value["narrative"], "Filed by the agent.");
      let row = crate::store::repo::captains_log::get(&db, "2026-07-05")
        .await
        .unwrap()
        .unwrap();
      assert_eq!(row.narrative().as_deref(), Some("Filed by the agent."));
    }
  }

  mod writes {
    use super::*;

    #[test]
    fn it_flags_captains_log_write_tools_for_the_reload_signal() {
      let registry = super::registry();

      assert!(writes(&registry, "captains_log_set_narrative"));
      assert!(writes(&registry, "captains_log_set_answer"));
      assert!(writes(&registry, "captains_log_set_kill_report"));
      assert!(!writes(&registry, "captains_log_list_days"));
      assert!(!writes(&registry, "captains_log_get_day"));
    }

    #[test]
    fn it_flags_dossier_write_tools_for_the_reload_signal() {
      let registry = super::registry();

      assert!(writes(&registry, "dossier_set_brief"));
      assert!(writes(&registry, "dossier_add_order"));
      assert!(writes(&registry, "dossier_edit_order"));
      assert!(writes(&registry, "dossier_complete_order"));
      assert!(writes(&registry, "dossier_cancel_order"));
      assert!(writes(&registry, "dossier_reopen_order"));
      assert!(writes(&registry, "dossier_remove_order"));
      assert!(writes(&registry, "dossier_link_objective"));
      assert!(writes(&registry, "dossier_unlink_objective"));
      assert!(!writes(&registry, "dossier_get"));
      assert!(!writes(&registry, "dossier_list_orders"));
    }
  }
}
