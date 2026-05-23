//! Persistent OAuth 2.0 callback listener.
//!
//! Binds port 47823 once at application startup and keeps the listener running
//! for the app's lifetime. Each accepted connection is parsed for the EVE SSO
//! `?code=&state=` query parameters and yielded as an iced subscription event.

use iced::Subscription;

const CALLBACK_PORT: u16 = 47823;

/// Returns a subscription that yields `(code, state)` pairs delivered to the
/// EVE SSO callback URL.
pub fn subscription() -> Subscription<(String, String)> {
  Subscription::run(stream)
}

fn stream() -> impl iced::futures::Stream<Item = (String, String)> {
  iced::stream::channel(4, async |mut tx| {
    use iced::futures::SinkExt as _;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let std_listener = match std::net::TcpListener::bind(format!("127.0.0.1:{CALLBACK_PORT}")) {
      Ok(l) => l,
      Err(e) => {
        tracing::error!("OAuth callback port {CALLBACK_PORT} unavailable: {e}");
        return;
      }
    };
    if let Err(e) = std_listener.set_nonblocking(true) {
      tracing::error!("OAuth callback listener set_nonblocking failed: {e}");
      return;
    }
    let listener = match tokio::net::TcpListener::from_std(std_listener) {
      Ok(l) => l,
      Err(e) => {
        tracing::error!("OAuth callback listener failed: {e}");
        return;
      }
    };

    loop {
      let Ok((mut stream, _)) = listener.accept().await else {
        tracing::warn!("auth: failed to accept OAuth callback connection");
        continue;
      };

      let mut buf = vec![0u8; 32768];
      let Ok(n) = stream.read(&mut buf).await else {
        tracing::warn!("auth: failed to read OAuth callback request");
        continue;
      };

      let request = String::from_utf8_lossy(&buf[..n]);
      let Ok((code, state)) = parse_callback(&request) else {
        tracing::warn!("auth: received malformed OAuth callback request");
        continue;
      };

      tracing::info!("auth: OAuth callback received, code and state parsed");

      let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nAuthorized. You may close this tab.";
      let _ = stream.write_all(response.as_bytes()).await;
      let _ = tx.send((code, state)).await;
    }
  })
}

fn parse_callback(request: &str) -> Result<(String, String), ()> {
  let first_line = request.lines().next().ok_or(())?;
  let path = first_line.split_whitespace().nth(1).ok_or(())?;
  let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
  extract_query_params(query)
}

fn extract_query_params(query: &str) -> Result<(String, String), ()> {
  let mut code = None;
  let mut state = None;
  for pair in query.split('&') {
    if let Some((k, v)) = pair.split_once('=') {
      match k {
        "code" => code = Some(v.to_owned()),
        "state" => state = Some(v.to_owned()),
        _ => {}
      }
    }
  }
  Ok((code.ok_or(())?, state.ok_or(())?))
}

#[cfg(test)]
mod tests {
  use super::*;

  mod extract_query_params {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_extracts_code_and_state() {
      let (code, state) = extract_query_params("code=abc123&state=xyz").unwrap();

      assert_eq!(code, "abc123");
      assert_eq!(state, "xyz");
    }

    #[test]
    fn it_extracts_with_reversed_param_order() {
      let (code, state) = extract_query_params("state=xyz&code=abc123").unwrap();

      assert_eq!(code, "abc123");
      assert_eq!(state, "xyz");
    }

    #[test]
    fn it_returns_error_when_code_is_missing() {
      let result = extract_query_params("state=xyz");

      assert!(result.is_err());
    }

    #[test]
    fn it_returns_error_when_state_is_missing() {
      let result = extract_query_params("code=abc123");

      assert!(result.is_err());
    }

    #[test]
    fn it_returns_error_on_empty_query() {
      let result = extract_query_params("");

      assert!(result.is_err());
    }
  }

  mod parse_callback {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_extracts_code_and_state_from_get_request() {
      let request = "GET /?code=mycode&state=mystate HTTP/1.1\r\nHost: localhost\r\n\r\n";

      let (code, state) = parse_callback(request).unwrap();

      assert_eq!(code, "mycode");
      assert_eq!(state, "mystate");
    }

    #[test]
    fn it_returns_error_on_empty_request() {
      let result = parse_callback("");

      assert!(result.is_err());
    }

    #[test]
    fn it_returns_error_when_path_has_no_query_string() {
      let request = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";

      let result = parse_callback(request);

      assert!(result.is_err());
    }

    #[test]
    fn it_returns_error_when_code_is_missing_from_query() {
      let request = "GET /?state=only HTTP/1.1\r\nHost: localhost\r\n\r\n";

      let result = parse_callback(request);

      assert!(result.is_err());
    }
  }
}
