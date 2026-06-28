//! The inbound bridge from the server thread into the iced update loop.
//!
//! Mirrors `features::roster::auth::deep_link`: an iced [`Subscription`] stashes a static `SENDER`, and the
//! server thread hands a fully-formed [`McpRequest`] to [`deliver`], which `try_send`s it so the
//! app's `update()` can dispatch the tool on the UI thread (where it owns the live state) and reply
//! through the oneshot the request carries.

use std::sync::{Arc, Mutex};

use iced::{Subscription, futures::Stream};
use serde_json::Value;
use tokio::sync::oneshot;

use crate::mcp::tool::ToolOutcome;

static SENDER: Mutex<Option<Sender>> = Mutex::new(None);

type Sender = iced::futures::channel::mpsc::Sender<McpRequest>;

/// One tool invocation routed from the server thread into `update()`.
///
/// `Message` is `Clone`, so the oneshot reply is held behind a shared cell the dispatcher takes once
/// via [`McpRequest::reply`]; the request itself stays cheaply cloneable. A request whose reply is
/// dropped without being answered resolves to a channel error on the server side. The call is gated
/// in the update loop against the live config — the authoritative permission set — so the request
/// carries no permission snapshot of its own.
#[derive(Clone)]
pub struct McpRequest {
  args: Value,
  reply: Arc<Mutex<Option<oneshot::Sender<ToolOutcome>>>>,
  tool: String,
}

impl McpRequest {
  pub fn new(tool: String, args: Value) -> (Self, oneshot::Receiver<ToolOutcome>) {
    let (tx, rx) = oneshot::channel();
    let request = Self {
      args,
      reply: Arc::new(Mutex::new(Some(tx))),
      tool,
    };
    (request, rx)
  }

  pub fn args(&self) -> &Value {
    &self.args
  }

  pub fn tool(&self) -> &str {
    &self.tool
  }

  pub fn reply(&self, outcome: ToolOutcome) {
    if let Ok(mut guard) = self.reply.lock()
      && let Some(tx) = guard.take()
    {
      let _ = tx.send(outcome);
    }
  }
}

impl std::fmt::Debug for McpRequest {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("McpRequest")
      .field("tool", &self.tool)
      .finish_non_exhaustive()
  }
}

pub fn subscription() -> Subscription<McpRequest> {
  Subscription::run(stream)
}

pub fn deliver(request: McpRequest) -> bool {
  if let Ok(mut guard) = SENDER.lock()
    && let Some(tx) = guard.as_mut()
  {
    return tx.try_send(request).is_ok();
  }
  false
}

fn stream() -> impl Stream<Item = McpRequest> {
  iced::stream::channel(16, |tx: Sender| async move {
    if let Ok(mut guard) = SENDER.lock() {
      *guard = Some(tx);
    }
    std::future::pending::<()>().await;
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  mod mcp_request {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::mcp::tool::ToolError;

    #[tokio::test]
    async fn reply_delivers_the_outcome_to_the_waiting_receiver() {
      let (request, rx) = McpRequest::new("ping".to_owned(), Value::Null);

      request.reply(Ok(serde_json::json!({"pong": true})));

      assert_eq!(rx.await.unwrap().unwrap(), serde_json::json!({"pong": true}));
    }

    #[tokio::test]
    async fn a_second_reply_is_a_no_op() {
      let (request, rx) = McpRequest::new("ping".to_owned(), Value::Null);

      request.reply(Err(ToolError::Internal("first".to_owned())));
      request.reply(Ok(Value::Null));

      assert!(matches!(rx.await.unwrap(), Err(ToolError::Internal(reason)) if reason == "first"));
    }

    #[test]
    fn it_carries_the_tool_and_args() {
      let (request, _rx) = McpRequest::new("echo".to_owned(), serde_json::json!({"x": 1}));

      assert_eq!(request.tool(), "echo");
      assert_eq!(request.args(), &serde_json::json!({"x": 1}));
    }
  }
}
