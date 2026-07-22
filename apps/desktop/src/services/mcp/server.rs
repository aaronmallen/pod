use std::{net::SocketAddr, sync::Arc, time::Duration};

use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::{TcpListener, TcpStream},
  sync::oneshot,
};

use crate::{
  config::McpConfig,
  services::mcp::{
    bridge::{self, McpRequest},
    protocol::{self, Dispatch},
    tool::Registry,
    transport::{self, Reject},
  },
};

const TOOL_REPLY_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Eq, PartialEq)]
struct Binding {
  port: u16,
  token: String,
}

impl Binding {
  fn of(config: &McpConfig) -> Self {
    Self {
      port: *config.port(),
      token: config.token().clone(),
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
  Restart,
  Start,
  Stop,
  None,
}

struct Running {
  binding: Binding,
  shutdown: oneshot::Sender<()>,
}

pub struct Server {
  registry: Arc<Registry>,
  running: Option<Running>,
}

impl Server {
  pub fn new(registry: Registry) -> Self {
    Self {
      registry: Arc::new(registry),
      running: None,
    }
  }

  pub fn apply(&mut self, config: &McpConfig) {
    let want = config.enabled().then(|| Binding::of(config));
    let have = self.running.as_ref().map(|running| running.binding.clone());

    match plan(have.as_ref(), want.as_ref()) {
      Action::None => {}
      Action::Stop => self.stop(),
      Action::Start => self.start(want.expect("a Start plan implies a desired binding")),
      Action::Restart => {
        self.stop();
        self.start(want.expect("a Restart plan implies a desired binding"));
      }
    }
  }

  // Consumed by the Settings MCP tab (tylunpsl) to surface whether the listener is live; today only
  // this module's tests rely on the underlying state.
  #[cfg_attr(not(test), expect(dead_code))]
  pub fn is_running(&self) -> bool {
    self.running.is_some()
  }

  fn start(&mut self, binding: Binding) {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let addr = SocketAddr::new(transport::BIND_ADDR, binding.port);
    let context = Context {
      registry: Arc::clone(&self.registry),
      token: binding.token.clone(),
    };
    tokio::spawn(serve(addr, context, shutdown_rx));
    self.running = Some(Running {
      binding,
      shutdown: shutdown_tx,
    });
  }

  fn stop(&mut self) {
    if let Some(running) = self.running.take() {
      let _ = running.shutdown.send(());
    }
  }
}

#[derive(Clone)]
struct Context {
  registry: Arc<Registry>,
  token: String,
}

fn plan(have: Option<&Binding>, want: Option<&Binding>) -> Action {
  match (have, want) {
    (None, None) => Action::None,
    (None, Some(_)) => Action::Start,
    (Some(_), None) => Action::Stop,
    (Some(current), Some(desired)) if current == desired => Action::None,
    (Some(_), Some(_)) => Action::Restart,
  }
}

async fn serve(addr: SocketAddr, context: Context, mut shutdown: oneshot::Receiver<()>) {
  let listener = match TcpListener::bind(addr).await {
    Ok(listener) => listener,
    Err(error) => {
      tracing::warn!(%error, %addr, "mcp server failed to bind");
      return;
    }
  };
  tracing::info!(%addr, "mcp server listening");

  loop {
    tokio::select! {
      _ = &mut shutdown => break,
      accepted = listener.accept() => match accepted {
        Ok((stream, _peer)) => {
          let context = context.clone();
          tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, &context).await {
              tracing::debug!(%error, "mcp connection ended with an error");
            }
          });
        }
        Err(error) => tracing::warn!(%error, "mcp accept failed"),
      },
    }
  }
  tracing::info!(%addr, "mcp server stopped");
}

async fn handle_connection(mut stream: TcpStream, context: &Context) -> std::io::Result<()> {
  let raw = read_request(&mut stream).await?;
  let response = route(&raw, context).await;
  stream.write_all(response.as_bytes()).await?;
  stream.flush().await
}

async fn read_request(stream: &mut TcpStream) -> std::io::Result<String> {
  let mut buffer = Vec::new();
  let mut chunk = [0u8; 4096];
  loop {
    let read = stream.read(&mut chunk).await?;
    if read == 0 {
      break;
    }
    buffer.extend_from_slice(&chunk[..read]);
    if buffer.len() > transport::MAX_BODY_BYTES {
      break;
    }
    if request_is_complete(&buffer) {
      break;
    }
  }
  Ok(String::from_utf8_lossy(&buffer).into_owned())
}

fn request_is_complete(buffer: &[u8]) -> bool {
  let text = String::from_utf8_lossy(buffer);
  let Some((head, body)) = text.split_once("\r\n\r\n") else {
    return false;
  };
  let declared = content_length(head);
  body.len() >= declared
}

fn content_length(head: &str) -> usize {
  head
    .lines()
    .find_map(|line| {
      line
        .split_once(':')
        .filter(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
    })
    .and_then(|(_, value)| value.trim().parse().ok())
    .unwrap_or(0)
}

async fn route(raw: &str, context: &Context) -> String {
  let Some(request) = transport::parse_request(raw) else {
    return transport::http_response("HTTP/1.1 400 Bad Request", "{}");
  };
  if let Err(reject) = transport::authorize(&request, &context.token) {
    return reject_response(reject);
  }
  let Ok(value) = serde_json::from_str::<serde_json::Value>(&request.body) else {
    let body = protocol::error(None, -32700, "parse error: body is not valid JSON").to_string();
    return transport::http_response("HTTP/1.1 200 OK", &body);
  };
  match protocol::dispatch(&value, &context.registry) {
    None => transport::empty_response("HTTP/1.1 202 Accepted", &[]),
    Some(Dispatch::Respond(response)) => transport::http_response("HTTP/1.1 200 OK", &response.to_string()),
    Some(Dispatch::ToolCall {
      id,
      tool,
      args,
    }) => {
      let body = call_tool(id, tool, args).await.to_string();
      transport::http_response("HTTP/1.1 200 OK", &body)
    }
  }
}

/// Routes a `tools/call` through the bridge into the update loop and awaits its reply, falling back
/// to a tool error when the bridge is not attached, the reply is dropped, or it times out. The call
/// is gated against the live config in the update loop, so the accept loop forwards it unconditionally.
async fn call_tool(id: serde_json::Value, tool: String, args: serde_json::Value) -> serde_json::Value {
  let (request, rx) = McpRequest::new(tool, args);
  if !bridge::deliver(request) {
    return protocol::error(Some(id), -32002, "the application is not ready to handle tool calls");
  }
  match tokio::time::timeout(TOOL_REPLY_TIMEOUT, rx).await {
    Ok(Ok(outcome)) => protocol::tool_response(id, outcome),
    Ok(Err(_)) => protocol::error(Some(id), -32003, "the tool call was dropped without a reply"),
    Err(_) => protocol::error(Some(id), -32004, "the tool call timed out"),
  }
}

fn reject_response(reject: Reject) -> String {
  // A 405 must advertise the one method the endpoint serves; it carries no JSON-RPC body (the request
  // was never parsed). Every other rejection keeps the small JSON-RPC error body for client clarity.
  if reject == Reject::MethodNotAllowed {
    return transport::empty_response(reject.status_line(), &[("Allow", transport::ALLOWED_METHOD)]);
  }
  let body = protocol::error(None, -32600, "request rejected").to_string();
  transport::http_response(reject.status_line(), &body)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn binding(port: u16) -> Binding {
    Binding {
      port,
      token: "pod_mcp_secret".to_owned(),
    }
  }

  async fn free_port() -> u16 {
    let listener = TcpListener::bind((transport::BIND_ADDR, 0)).await.unwrap();
    listener.local_addr().unwrap().port()
  }

  async fn enabled_config(port: u16) -> McpConfig {
    let mut config = McpConfig::default();
    config.set_enabled(true);
    config.set_port(port);
    config.set_token("pod_mcp_secret".to_owned());
    config
  }

  async fn round_trip(port: u16, request: &str) -> String {
    let mut stream = TcpStream::connect((transport::BIND_ADDR, port)).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
  }

  async fn wait_until_listening(port: u16) {
    for _ in 0..50 {
      if TcpStream::connect((transport::BIND_ADDR, port)).await.is_ok() {
        return;
      }
      tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the mcp server never started listening on port {port}");
  }

  mod lifecycle {
    use super::*;
    use crate::services::mcp::tool::{McpTool, Permission};

    fn server() -> Server {
      let registry = Registry::default().with(McpTool::new(
        "ping",
        "Liveness check",
        Permission::Read,
        |_db, _args| async move { Ok(serde_json::json!({ "ok": true })) },
      ));
      Server::new(registry)
    }

    #[tokio::test]
    async fn enabling_then_disabling_opens_and_closes_the_port() {
      let port = free_port().await;
      let mut server = server();

      server.apply(&enabled_config(port).await);
      assert!(server.is_running(), "an enabled config starts the listener");
      wait_until_listening(port).await;

      let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#;
      let request = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer pod_mcp_secret\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
      );
      let response = round_trip(port, &request).await;
      assert!(
        response.contains("protocolVersion"),
        "the agent can initialize: {response}"
      );

      server.apply(&McpConfig::default());
      assert!(!server.is_running(), "a disabled config stops the listener");
      for _ in 0..50 {
        if TcpStream::connect((transport::BIND_ADDR, port)).await.is_err() {
          return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
      }
      panic!("the port stayed open after the server was disabled");
    }

    #[tokio::test]
    async fn changing_the_port_moves_the_listener() {
      let first = free_port().await;
      let mut server = server();
      server.apply(&enabled_config(first).await);
      wait_until_listening(first).await;

      let second = free_port().await;
      server.apply(&enabled_config(second).await);
      wait_until_listening(second).await;

      assert!(
        TcpStream::connect((transport::BIND_ADDR, second)).await.is_ok(),
        "the listener moved to the new port"
      );
    }
  }

  mod plan {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_does_nothing_when_neither_running_nor_wanted() {
      assert_eq!(plan(None, None), Action::None);
    }

    #[test]
    fn it_starts_when_wanted_but_not_running() {
      assert_eq!(plan(None, Some(&binding(7373))), Action::Start);
    }

    #[test]
    fn it_stops_when_running_but_no_longer_wanted() {
      assert_eq!(plan(Some(&binding(7373)), None), Action::Stop);
    }

    #[test]
    fn it_leaves_an_unchanged_binding_alone() {
      assert_eq!(plan(Some(&binding(7373)), Some(&binding(7373))), Action::None);
    }

    #[test]
    fn it_restarts_when_the_port_changes() {
      assert_eq!(plan(Some(&binding(7373)), Some(&binding(8000))), Action::Restart);
    }

    #[test]
    fn it_restarts_when_the_token_changes() {
      let rotated = Binding {
        token: "pod_mcp_rotated".to_owned(),
        ..binding(7373)
      };

      assert_eq!(plan(Some(&binding(7373)), Some(&rotated)), Action::Restart);
    }
  }

  mod binding {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_snapshots_the_port_and_token() {
      let mut config = McpConfig::default();
      config.set_port(9001);
      let token = config.token_or_generate();

      let snapshot = Binding::of(&config);

      assert_eq!(snapshot.port, 9001);
      assert_eq!(snapshot.token, token);
    }
  }

  mod content_length {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reads_the_declared_length_case_insensitively() {
      assert_eq!(content_length("POST /mcp HTTP/1.1\r\ncontent-length: 42"), 42);
    }

    #[test]
    fn it_defaults_to_zero_when_absent() {
      assert_eq!(content_length("POST /mcp HTTP/1.1\r\nHost: localhost"), 0);
    }
  }

  mod request_is_complete {
    use super::*;

    #[test]
    fn it_is_incomplete_without_the_header_terminator() {
      assert!(!request_is_complete(b"POST /mcp HTTP/1.1\r\nHost: localhost"));
    }

    #[test]
    fn it_is_complete_once_the_body_meets_the_declared_length() {
      let raw = b"POST /mcp HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}";

      assert!(request_is_complete(raw));
    }

    #[test]
    fn it_is_incomplete_while_the_body_is_short() {
      let raw = b"POST /mcp HTTP/1.1\r\nContent-Length: 10\r\n\r\n{}";

      assert!(!request_is_complete(raw));
    }
  }

  mod route {
    use super::*;
    use crate::services::mcp::tool::{McpTool, Permission};

    fn context() -> Context {
      let registry = Registry::default().with(McpTool::new(
        "ping",
        "Liveness check",
        Permission::Read,
        |_db, _args| async move { Ok(serde_json::json!({ "ok": true })) },
      ));
      Context {
        registry: Arc::new(registry),
        token: "pod_mcp_secret".to_owned(),
      }
    }

    fn post(body: &str, token: &str) -> String {
      format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
      )
    }

    #[tokio::test]
    async fn it_lists_tools_for_an_authorized_request() {
      let context = context();
      let raw = post(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#, "pod_mcp_secret");

      let response = route(&raw, &context).await;

      assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
      assert!(response.contains("\"ping\""), "the smoke tool is listed: {response}");
    }

    #[tokio::test]
    async fn it_rejects_a_wrong_token_with_401() {
      let context = context();
      let raw = post(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#, "nope");

      let response = route(&raw, &context).await;

      assert!(response.starts_with("HTTP/1.1 401 Unauthorized"), "{response}");
    }

    #[tokio::test]
    async fn it_initializes_for_an_authorized_request() {
      let context = context();
      let raw = post(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#, "pod_mcp_secret");

      let response = route(&raw, &context).await;

      assert!(response.contains("protocolVersion"), "{response}");
    }

    #[tokio::test]
    async fn an_authorized_get_returns_405_with_an_allow_post_header() {
      let context = context();
      let raw = "GET /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer pod_mcp_secret\r\n\r\n".to_owned();

      let response = route(&raw, &context).await;

      assert!(response.starts_with("HTTP/1.1 405 Method Not Allowed"), "{response}");
      assert!(response.contains("Allow: POST\r\n"), "{response}");
      assert!(
        response.contains("Content-Length: 0\r\n"),
        "the 405 has no body: {response}"
      );
    }

    #[tokio::test]
    async fn an_unauthorized_get_is_still_401() {
      let context = context();
      let raw = "GET /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer nope\r\n\r\n".to_owned();

      let response = route(&raw, &context).await;

      assert!(response.starts_with("HTTP/1.1 401 Unauthorized"), "{response}");
    }

    #[tokio::test]
    async fn an_unknown_path_is_still_404() {
      let context = context();
      let raw = "POST /admin HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer pod_mcp_secret\r\n\r\n".to_owned();

      let response = route(&raw, &context).await;

      assert!(response.starts_with("HTTP/1.1 404 Not Found"), "{response}");
    }

    #[tokio::test]
    async fn a_notification_returns_202_with_an_empty_body() {
      let context = context();
      let raw = post(
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "pod_mcp_secret",
      );

      let response = route(&raw, &context).await;

      assert!(response.starts_with("HTTP/1.1 202 Accepted"), "{response}");
      assert!(response.contains("Content-Length: 0\r\n"), "{response}");
      assert!(response.ends_with("\r\n\r\n"), "the 202 carries no body: {response}");
    }

    #[tokio::test]
    async fn a_post_carrying_handshake_headers_still_succeeds() {
      let context = context();
      let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
      let raw = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer pod_mcp_secret\r\nMcp-Session-Id: deadbeef\r\nMCP-Protocol-Version: 2025-06-18\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
      );

      let response = route(&raw, &context).await;

      assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
      assert!(response.contains("\"ping\""), "{response}");
    }
  }
}
