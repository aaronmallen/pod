//! The JSON-RPC 2.0 surface of the MCP server, independent of the HTTP transport.
//!
//! Pod speaks the small slice of MCP an automation agent needs over plain JSON-RPC: `initialize`,
//! `ping`, `tools/list`, and `tools/call`. Methods that only read the catalog resolve synchronously
//! here; `tools/call` is routed out to the update loop (it needs the live database), so this layer
//! returns a [`Dispatch`] describing what to do rather than performing the call itself.

use serde_json::{Value, json};

use crate::mcp::{
  args::input_schema,
  tool::{Registry, ToolError},
};

/// The MCP protocol revision Pod advertises in `initialize` when the client requests none or one Pod
/// does not recognize. This is the Streamable-HTTP-era revision; advertising it signals the
/// single-endpoint transport rather than the legacy two-endpoint HTTP+SSE transport.
const PROTOCOL_VERSION: &str = "2025-06-18";

/// The protocol revisions Pod will echo back when a client requests one. Any other requested version
/// falls back to [`PROTOCOL_VERSION`].
const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];

const SERVER_NAME: &str = "pod";

/// JSON-RPC error code for an unsupported method.
const METHOD_NOT_FOUND: i64 = -32601;

/// JSON-RPC error code for malformed params.
const INVALID_PARAMS: i64 = -32602;

/// Application error code Pod uses for a permission-denied tool call.
const PERMISSION_DENIED: i64 = -32001;

/// Application error code Pod uses for any other tool failure.
const TOOL_ERROR: i64 = -32000;

/// What a parsed request resolves to: either a response ready to serialize, or a tool call that must
/// be forwarded to the update loop (carrying the id so the eventual reply can be correlated).
#[derive(Debug)]
pub enum Dispatch {
  /// A fully-resolved response (`initialize`, `ping`, `tools/list`, or an error).
  Respond(Value),
  /// A `tools/call` to route to the update loop: the request id, tool name, and arguments.
  ToolCall { id: Value, tool: String, args: Value },
}

/// Resolves a single JSON-RPC request against the catalog. Catalog-only methods produce a
/// [`Dispatch::Respond`] immediately; `tools/call` produces a [`Dispatch::ToolCall`] for the update
/// loop. A JSON-RPC notification (no `id`) for an unsupported method yields no response.
pub fn dispatch(request: &Value, registry: &Registry) -> Option<Dispatch> {
  let id = request.get("id").cloned();
  let method = request.get("method").and_then(Value::as_str).unwrap_or_default();

  match method {
    "initialize" => Some(Dispatch::Respond(success(id?, initialize_result(request)))),
    "notifications/initialized" => None,
    "ping" => Some(Dispatch::Respond(success(id?, json!({})))),
    "tools/list" => Some(Dispatch::Respond(success(id?, tools_result(registry)))),
    "tools/call" => Some(tool_call(id?, request)),
    _ => Some(Dispatch::Respond(error(
      id,
      METHOD_NOT_FOUND,
      &format!("unknown method: {method}"),
    ))),
  }
}

/// Builds the JSON-RPC response wrapping a tool outcome, correlated to the original call id.
pub fn tool_response(id: Value, outcome: Result<Value, ToolError>) -> Value {
  match outcome {
    Ok(value) => success(id, tool_content(&value)),
    Err(ToolError::PermissionDenied(perm)) => error(
      Some(id),
      PERMISSION_DENIED,
      &format!("permission denied: the `{perm}` permission is disabled"),
    ),
    Err(other) => error(Some(id), TOOL_ERROR, &other.to_string()),
  }
}

/// Builds a top-level JSON-RPC error for a request that could not even be parsed or authorized.
pub fn error(id: Option<Value>, code: i64, message: &str) -> Value {
  json!({
    "jsonrpc": "2.0",
    "id": id.unwrap_or(Value::Null),
    "error": { "code": code, "message": message },
  })
}

fn success(id: Value, result: Value) -> Value {
  json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn initialize_result(request: &Value) -> Value {
  json!({
    "protocolVersion": negotiated_version(request),
    "capabilities": { "tools": {} },
    "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
  })
}

/// Picks the protocol version to advertise: echo the client's requested `params.protocolVersion` when
/// Pod recognizes it, otherwise fall back to Pod's latest ([`PROTOCOL_VERSION`]). A request with no
/// requested version also falls back to the default.
fn negotiated_version(request: &Value) -> &str {
  request
    .get("params")
    .and_then(|params| params.get("protocolVersion"))
    .and_then(Value::as_str)
    .filter(|requested| SUPPORTED_PROTOCOL_VERSIONS.contains(requested))
    .unwrap_or(PROTOCOL_VERSION)
}

fn tools_result(registry: &Registry) -> Value {
  let tools: Vec<Value> = registry
    .tools()
    .map(|tool| {
      json!({
        "name": tool.name(),
        "description": tool.description(),
        "inputSchema": input_schema(tool.args()),
      })
    })
    .collect();
  json!({ "tools": tools })
}

/// Wraps a tool's JSON result in the MCP `content` envelope a client expects from `tools/call`.
fn tool_content(value: &Value) -> Value {
  let text = serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned());
  json!({ "content": [ { "type": "text", "text": text } ] })
}

fn tool_call(id: Value, request: &Value) -> Dispatch {
  let params = request.get("params");
  let Some(tool) = params.and_then(|p| p.get("name")).and_then(Value::as_str) else {
    return Dispatch::Respond(error(Some(id), INVALID_PARAMS, "tools/call requires a `name`"));
  };
  let args = params
    .and_then(|p| p.get("arguments"))
    .cloned()
    .unwrap_or_else(|| json!({}));
  Dispatch::ToolCall {
    id,
    tool: tool.to_owned(),
    args,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::mcp::{
    args::ArgSpec,
    tool::{McpTool, Permission},
  };

  fn registry() -> Registry {
    Registry::default().with(McpTool::new(
      "ping",
      "Liveness check",
      Permission::Read,
      |_db, _args| async move { Ok(json!({ "ok": true })) },
    ))
  }

  mod dispatch {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn initialize_advertises_the_protocol_and_tool_capability() {
      let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" });

      let Some(Dispatch::Respond(response)) = dispatch(&request, &registry()) else {
        panic!("initialize must resolve in-band");
      };

      assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
      assert_eq!(response["result"]["serverInfo"]["name"], "pod");
      assert!(response["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn initialize_defaults_to_the_latest_version_when_the_client_requests_none() {
      let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" });

      let Some(Dispatch::Respond(response)) = dispatch(&request, &registry()) else {
        panic!("initialize must resolve in-band");
      };

      assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
    }

    #[test]
    fn initialize_echoes_a_recognized_requested_version() {
      for requested in ["2024-11-05", "2025-03-26", "2025-06-18"] {
        let request = json!({
          "jsonrpc": "2.0", "id": 1, "method": "initialize",
          "params": { "protocolVersion": requested },
        });

        let Some(Dispatch::Respond(response)) = dispatch(&request, &registry()) else {
          panic!("initialize must resolve in-band");
        };

        assert_eq!(
          response["result"]["protocolVersion"], requested,
          "a recognized requested version is echoed back"
        );
      }
    }

    #[test]
    fn initialize_falls_back_to_the_latest_for_an_unrecognized_requested_version() {
      let request = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "1999-01-01" },
      });

      let Some(Dispatch::Respond(response)) = dispatch(&request, &registry()) else {
        panic!("initialize must resolve in-band");
      };

      assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
    }

    #[test]
    fn tools_list_returns_the_registered_tools() {
      let request = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });

      let Some(Dispatch::Respond(response)) = dispatch(&request, &registry()) else {
        panic!("tools/list resolves in-band");
      };

      let tools = response["result"]["tools"].as_array().unwrap();
      assert_eq!(tools.len(), 1);
      assert_eq!(tools[0]["name"], "ping");
    }

    #[test]
    fn tools_list_emits_a_real_input_schema_from_arg_specs() {
      let registry = Registry::default()
        .with(
          McpTool::new("with_args", "Has args", Permission::Read, |_db, _args| async move {
            Ok(json!({}))
          })
          .with_args([
            ArgSpec::integer("character_id", "The character id"),
            ArgSpec::optional_integer("page", 0, "Zero-based page"),
          ]),
        )
        .with(McpTool::new(
          "no_args",
          "No args",
          Permission::Read,
          |_db, _args| async move { Ok(json!({})) },
        ));
      let request = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });

      let Some(Dispatch::Respond(response)) = dispatch(&request, &registry) else {
        panic!("tools/list resolves in-band");
      };

      let tools = response["result"]["tools"].as_array().unwrap();
      let with_args = tools.iter().find(|t| t["name"] == "with_args").unwrap();
      let no_args = tools.iter().find(|t| t["name"] == "no_args").unwrap();

      let schema = &with_args["inputSchema"];
      assert_eq!(schema["type"], "object");
      assert_eq!(schema["properties"]["character_id"]["type"], "integer");
      assert_eq!(schema["properties"]["character_id"]["description"], "The character id");
      assert_eq!(schema["properties"]["page"]["type"], "integer");
      assert_eq!(schema["required"], json!(["character_id"]));

      assert_eq!(no_args["inputSchema"]["properties"], json!({}));
      assert_eq!(no_args["inputSchema"]["required"], json!([]));
    }

    #[test]
    fn ping_resolves_in_band() {
      let request = json!({ "jsonrpc": "2.0", "id": 3, "method": "ping" });

      assert!(matches!(dispatch(&request, &registry()), Some(Dispatch::Respond(_))));
    }

    #[test]
    fn tools_call_routes_out_with_the_tool_and_args() {
      let request = json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": { "name": "ping", "arguments": { "n": 1 } },
      });

      let Some(Dispatch::ToolCall {
        id,
        tool,
        args,
      }) = dispatch(&request, &registry())
      else {
        panic!("tools/call must route to the update loop");
      };

      assert_eq!(id, json!(4));
      assert_eq!(tool, "ping");
      assert_eq!(args, json!({ "n": 1 }));
    }

    #[test]
    fn tools_call_without_a_name_is_an_invalid_params_error() {
      let request = json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {} });

      let Some(Dispatch::Respond(response)) = dispatch(&request, &registry()) else {
        panic!("a malformed call resolves to an error response");
      };

      assert_eq!(response["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn an_unknown_method_is_method_not_found() {
      let request = json!({ "jsonrpc": "2.0", "id": 6, "method": "frobnicate" });

      let Some(Dispatch::Respond(response)) = dispatch(&request, &registry()) else {
        panic!("an unknown method resolves to an error response");
      };

      assert_eq!(response["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn an_initialized_notification_produces_no_response() {
      let request = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });

      assert!(dispatch(&request, &registry()).is_none());
    }
  }

  mod tool_response {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_wraps_an_ok_result_in_the_content_envelope() {
      let response = tool_response(json!(7), Ok(json!({ "answer": 42 })));

      let text = response["result"]["content"][0]["text"].as_str().unwrap();
      assert_eq!(response["result"]["content"][0]["type"], "text");
      assert_eq!(serde_json::from_str::<Value>(text).unwrap(), json!({ "answer": 42 }));
    }

    #[test]
    fn a_permission_denial_maps_to_the_permission_denied_code() {
      let response = tool_response(json!(8), Err(ToolError::PermissionDenied("send_mail")));

      assert_eq!(response["error"]["code"], PERMISSION_DENIED);
      assert!(response["error"]["message"].as_str().unwrap().contains("send_mail"));
    }

    #[test]
    fn any_other_tool_error_maps_to_the_tool_error_code() {
      let response = tool_response(json!(9), Err(ToolError::InvalidArguments("bad".to_owned())));

      assert_eq!(response["error"]["code"], TOOL_ERROR);
    }
  }
}
