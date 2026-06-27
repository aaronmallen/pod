//! The typed tool catalog and the permission gate.
//!
//! A tool is registered once into the [`Registry`] with the [`Permission`] it requires and an
//! async handler that receives a database clone plus the JSON arguments. The registry is the single
//! plug-in point the later read/write/mail specs extend: each registers an [`McpTool`] and lets the
//! gate refuse the call when the relevant config flag is off — handlers never re-check permissions
//! themselves.

use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

use serde_json::Value;

use crate::{config::McpPerms, mcp::args::ArgSpec, store::Database};

/// The five-flag trust surface a tool can require. Mirrors [`crate::config::McpPerms`] one-to-one so
/// the gate can map a tool's requirement straight onto the configured flag.
///
/// A1 ships only the `Read` smoke tool; the mutating variants are the requirement labels the
/// write/mail tool specs (kqyllswo, quqlxuvw) attach to their tools, so today only the gate's
/// `match` arms and this module's tests construct them.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Permission {
  DeleteMail,
  LocalWrite,
  ManageLabels,
  Read,
  SendMail,
}

impl Permission {
  pub fn label(self) -> &'static str {
    match self {
      Permission::DeleteMail => "delete_mail",
      Permission::LocalWrite => "local_write",
      Permission::ManageLabels => "manage_labels",
      Permission::Read => "read",
      Permission::SendMail => "send_mail",
    }
  }

  /// Whether `perms` grants this permission. The gate ([`Registry::dispatch`]) consults this; a
  /// tool whose permission is not granted is refused before its handler runs.
  pub fn granted_by(self, perms: &McpPerms) -> bool {
    match self {
      Permission::DeleteMail => perms.delete_mail(),
      Permission::LocalWrite => perms.local_write(),
      Permission::ManageLabels => perms.manage_labels(),
      Permission::Read => perms.read(),
      Permission::SendMail => perms.send_mail(),
    }
  }
}

/// The outcome of running a tool: either a JSON result or a structured error. The transport maps
/// this onto a JSON-RPC response; [`ToolError::PermissionDenied`] becomes a permission-denied error.
pub type ToolOutcome = Result<Value, ToolError>;

#[derive(Clone, Debug, thiserror::Error)]
pub enum ToolError {
  #[error("invalid arguments: {0}")]
  InvalidArguments(String),
  #[error("internal error: {0}")]
  Internal(String),
  #[error("permission denied: this tool requires the `{0}` permission, which is disabled")]
  PermissionDenied(&'static str),
  #[error("unknown tool: {0}")]
  UnknownTool(String),
}

type ToolFuture = Pin<Box<dyn Future<Output = ToolOutcome> + Send>>;

type ToolHandler = Arc<dyn Fn(Database, Value) -> ToolFuture + Send + Sync>;

/// One registered MCP tool: its name, a one-line description surfaced to the agent, the permission
/// it requires, and the async handler that produces its result.
#[derive(Clone)]
pub struct McpTool {
  args: Vec<ArgSpec>,
  description: &'static str,
  handler: ToolHandler,
  name: &'static str,
  permission: Permission,
}

impl McpTool {
  pub fn new<F, Fut>(name: &'static str, description: &'static str, permission: Permission, handler: F) -> Self
  where
    F: Fn(Database, Value) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ToolOutcome> + Send + 'static,
  {
    Self {
      args: Vec::new(),
      description,
      handler: Arc::new(move |db, args| Box::pin(handler(db, args))),
      name,
      permission,
    }
  }

  /// Attaches the declarative argument list that becomes this tool's advertised JSON Schema. Tools
  /// that omit this call advertise empty properties.
  pub fn with_args(mut self, args: impl IntoIterator<Item = ArgSpec>) -> Self {
    self.args = args.into_iter().collect();
    self
  }

  pub fn args(&self) -> &[ArgSpec] {
    &self.args
  }

  pub fn description(&self) -> &'static str {
    self.description
  }

  pub fn name(&self) -> &'static str {
    self.name
  }

  pub fn permission(&self) -> Permission {
    self.permission
  }

  fn run(&self, db: Database, args: Value) -> ToolFuture {
    (self.handler)(db, args)
  }
}

impl std::fmt::Debug for McpTool {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("McpTool")
      .field("name", &self.name)
      .field("permission", &self.permission)
      .finish_non_exhaustive()
  }
}

/// The catalog of tools an agent may list and call. Built once at startup; the read/write/mail specs
/// register their tools here. Lookups and listing are name-keyed and deterministic.
#[derive(Clone, Debug, Default)]
pub struct Registry {
  tools: BTreeMap<&'static str, McpTool>,
}

impl Registry {
  pub fn register(&mut self, tool: McpTool) {
    self.tools.insert(tool.name(), tool);
  }

  pub fn with(mut self, tool: McpTool) -> Self {
    self.register(tool);
    self
  }

  pub fn get(&self, name: &str) -> Option<&McpTool> {
    self.tools.get(name)
  }

  pub fn tools(&self) -> impl Iterator<Item = &McpTool> {
    self.tools.values()
  }

  /// Runs `name` against `db` with `args`, refusing the call when `perms` does not grant the tool's
  /// permission. This is the single gate: an unregistered tool is [`ToolError::UnknownTool`] and a
  /// gated-off tool is [`ToolError::PermissionDenied`], both reported before the handler ever runs.
  pub async fn dispatch(&self, name: &str, perms: &McpPerms, db: Database, args: Value) -> ToolOutcome {
    let Some(tool) = self.get(name) else {
      return Err(ToolError::UnknownTool(name.to_owned()));
    };
    let permission = tool.permission();
    if !permission.granted_by(perms) {
      return Err(ToolError::PermissionDenied(permission.label()));
    }
    tool.run(db, args).await
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn deny_all() -> McpPerms {
    let mut perms = McpPerms::default();
    perms.set_read(false);
    perms.set_local_write(false);
    perms
  }

  fn echo_registry() -> Registry {
    Registry::default().with(McpTool::new(
      "echo",
      "Echoes its arguments",
      Permission::Read,
      |_db, args| async move { Ok(args) },
    ))
  }

  mod permission {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_each_flag_onto_the_matching_perm() {
      let perms = McpPerms::default();

      assert!(Permission::Read.granted_by(&perms));
      assert!(Permission::LocalWrite.granted_by(&perms));
      assert!(!Permission::SendMail.granted_by(&perms));
      assert!(!Permission::DeleteMail.granted_by(&perms));
      assert!(!Permission::ManageLabels.granted_by(&perms));
    }

    #[test]
    fn its_label_is_the_config_key() {
      assert_eq!(Permission::Read.label(), "read");
      assert_eq!(Permission::LocalWrite.label(), "local_write");
      assert_eq!(Permission::SendMail.label(), "send_mail");
    }
  }

  mod registry {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_lists_registered_tools_by_name() {
      let registry = echo_registry();

      let names: Vec<&str> = registry.tools().map(McpTool::name).collect();

      assert_eq!(names, vec!["echo"]);
    }
  }

  mod dispatch {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn database() -> Database {
      let dir = tempfile::tempdir().expect("temp dir for an isolated database");
      crate::store::open(&dir.path().join("pod.db"))
        .await
        .expect("open an empty migrated database")
    }

    #[tokio::test]
    async fn it_runs_an_allowed_tool_and_returns_its_result() {
      let registry = echo_registry();
      let db = database().await;

      let outcome = registry
        .dispatch("echo", &McpPerms::default(), db, serde_json::json!({"ping": 1}))
        .await;

      assert_eq!(outcome.unwrap(), serde_json::json!({"ping": 1}));
    }

    #[tokio::test]
    async fn it_refuses_a_tool_whose_permission_is_off() {
      let registry = echo_registry();
      let db = database().await;

      let outcome = registry.dispatch("echo", &deny_all(), db, Value::Null).await;

      assert!(
        matches!(outcome, Err(ToolError::PermissionDenied("read"))),
        "a gated-off tool never runs: {outcome:?}"
      );
    }

    #[tokio::test]
    async fn it_reports_an_unknown_tool() {
      let registry = echo_registry();
      let db = database().await;

      let outcome = registry.dispatch("nope", &McpPerms::default(), db, Value::Null).await;

      assert!(matches!(outcome, Err(ToolError::UnknownTool(name)) if name == "nope"));
    }
  }
}
