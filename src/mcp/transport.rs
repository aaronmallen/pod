//! A minimal localhost HTTP/1.1 transport carrying JSON-RPC over `POST /mcp`.
//!
//! Pod hand-rolls this rather than pulling in an HTTP-server stack: the surface is one route, one
//! verb, JSON in and JSON out, behind a bearer token. The server binds `127.0.0.1` only and rejects
//! any request whose `Authorization`, `Origin`, or `Host` fails the checks in this module — those
//! checks are the security boundary and are kept as pure functions so they are exhaustively tested.

use std::net::{IpAddr, Ipv4Addr};

/// The single route the server answers. A request to any other path is a 404.
pub const ENDPOINT: &str = "/mcp";

/// Loopback the server binds to. Never `0.0.0.0`: the automation surface must not be reachable off
/// the machine, so it is pinned to localhost.
pub const BIND_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

/// Cap on the request body so a malformed or hostile client cannot make the server buffer without
/// bound. MCP requests are small JSON documents.
pub const MAX_BODY_BYTES: usize = 1 << 20;

/// A parsed HTTP request: just the parts the JSON-RPC layer and the auth checks need.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
  pub authorization: Option<String>,
  pub body: String,
  pub host: Option<String>,
  pub method: String,
  pub origin: Option<String>,
  pub path: String,
}

/// Why a request was refused before it reached the JSON-RPC layer. Each maps to an HTTP status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reject {
  BadOrigin,
  NotFound,
  Unauthorized,
}

impl Reject {
  pub fn status_line(self) -> &'static str {
    match self {
      Reject::BadOrigin => "HTTP/1.1 403 Forbidden",
      Reject::NotFound => "HTTP/1.1 404 Not Found",
      Reject::Unauthorized => "HTTP/1.1 401 Unauthorized",
    }
  }
}

/// Validates a parsed request against the route, the bearer token, and the Origin/Host allowlist.
///
/// The checks, in order: the path must be [`ENDPOINT`] (else 404); the `Authorization` header must
/// be `Bearer <token>` matching `token` exactly (else 401); and any `Origin`/`Host` present must be
/// loopback (else 403). A request that passes all three is `Ok`.
pub fn authorize(request: &Request, token: &str) -> Result<(), Reject> {
  if request.path != ENDPOINT {
    return Err(Reject::NotFound);
  }
  if !bearer_matches(request.authorization.as_deref(), token) {
    return Err(Reject::Unauthorized);
  }
  if !origin_is_local(request.origin.as_deref()) || !host_is_local(request.host.as_deref()) {
    return Err(Reject::BadOrigin);
  }
  Ok(())
}

/// Whether the `Authorization` header is exactly `Bearer <token>` for a non-empty token.
fn bearer_matches(header: Option<&str>, token: &str) -> bool {
  if token.is_empty() {
    return false;
  }
  header
    .and_then(|value| value.strip_prefix("Bearer "))
    .is_some_and(|presented| presented == token)
}

/// Whether an `Origin` header, if present, points at loopback. A missing Origin is allowed: a
/// native MCP client (not a browser) sends none, and the bearer token already gates access; the
/// Origin check exists to blunt a browser-driven DNS-rebinding/CSRF attempt, which always sends one.
fn origin_is_local(origin: Option<&str>) -> bool {
  match origin {
    None => true,
    Some(value) => host_is_local(host_part_of(value)),
  }
}

/// Whether a `Host` header, if present, is loopback — the DNS-rebinding guard: a rebound name
/// resolves to a private address but still carries the attacker's `Host`, so only `localhost` /
/// `127.0.0.1` / `[::1]` (with any port) are accepted.
fn host_is_local(host: Option<&str>) -> bool {
  let Some(host) = host else {
    return true;
  };
  let name = host.rsplit_once(':').map_or(host, |(head, _port)| head);
  let name = name.trim_start_matches('[').trim_end_matches(']');
  matches!(name, "localhost" | "127.0.0.1" | "::1")
}

/// Extracts the `host[:port]` authority from a URL-shaped `Origin` value (`scheme://host:port`).
fn host_part_of(origin: &str) -> Option<&str> {
  origin
    .split_once("://")
    .map(|(_scheme, rest)| rest.split('/').next().unwrap_or(rest))
}

/// Parses the head and body of an HTTP/1.1 request from its raw bytes. Returns `None` for a request
/// that is not well-formed enough to route (no request line). Header names are matched
/// case-insensitively; only the handful this transport needs are retained.
pub fn parse_request(raw: &str) -> Option<Request> {
  let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw, ""));
  let mut lines = head.lines();
  let request_line = lines.next()?;
  let mut parts = request_line.split_whitespace();
  let method = parts.next()?.to_owned();
  let path = parts.next()?.to_owned();

  let mut authorization = None;
  let mut host = None;
  let mut origin = None;
  for line in lines {
    let Some((name, value)) = line.split_once(':') else {
      continue;
    };
    let value = value.trim().to_owned();
    match name.trim().to_ascii_lowercase().as_str() {
      "authorization" => authorization = Some(value),
      "host" => host = Some(value),
      "origin" => origin = Some(value),
      _ => {}
    }
  }

  Some(Request {
    authorization,
    body: body.to_owned(),
    host,
    method,
    origin,
    path: path.split('?').next().unwrap_or(&path).to_owned(),
  })
}

/// Serializes a JSON body into a complete HTTP/1.1 response with the given status line.
pub fn http_response(status_line: &str, json: &str) -> String {
  format!(
    "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
    json.len()
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn request() -> Request {
    Request {
      authorization: Some("Bearer pod_mcp_secret".to_owned()),
      body: String::new(),
      host: Some("127.0.0.1:7373".to_owned()),
      method: "POST".to_owned(),
      origin: None,
      path: ENDPOINT.to_owned(),
    }
  }

  mod authorize {
    use super::*;

    #[test]
    fn it_accepts_a_loopback_request_with_the_right_token() {
      assert_eq!(authorize(&request(), "pod_mcp_secret"), Ok(()));
    }

    #[test]
    fn it_rejects_a_request_to_another_path_as_not_found() {
      let req = Request {
        path: "/admin".to_owned(),
        ..request()
      };

      assert_eq!(authorize(&req, "pod_mcp_secret"), Err(Reject::NotFound));
    }

    #[test]
    fn it_rejects_a_wrong_token_as_unauthorized() {
      let req = Request {
        authorization: Some("Bearer wrong".to_owned()),
        ..request()
      };

      assert_eq!(authorize(&req, "pod_mcp_secret"), Err(Reject::Unauthorized));
    }

    #[test]
    fn it_rejects_a_missing_token_as_unauthorized() {
      let req = Request {
        authorization: None,
        ..request()
      };

      assert_eq!(authorize(&req, "pod_mcp_secret"), Err(Reject::Unauthorized));
    }

    #[test]
    fn an_empty_configured_token_never_authorizes() {
      let req = Request {
        authorization: Some("Bearer ".to_owned()),
        ..request()
      };

      assert_eq!(authorize(&req, ""), Err(Reject::Unauthorized));
    }

    #[test]
    fn it_rejects_a_non_loopback_host_as_a_dns_rebinding_attempt() {
      let req = Request {
        host: Some("evil.example.com".to_owned()),
        ..request()
      };

      assert_eq!(authorize(&req, "pod_mcp_secret"), Err(Reject::BadOrigin));
    }

    #[test]
    fn it_rejects_a_cross_site_browser_origin() {
      let req = Request {
        origin: Some("https://evil.example".to_owned()),
        ..request()
      };

      assert_eq!(authorize(&req, "pod_mcp_secret"), Err(Reject::BadOrigin));
    }

    #[test]
    fn it_accepts_a_loopback_browser_origin() {
      let req = Request {
        origin: Some("http://localhost:7373".to_owned()),
        ..request()
      };

      assert_eq!(authorize(&req, "pod_mcp_secret"), Ok(()));
    }

    #[test]
    fn the_path_check_precedes_the_token_check() {
      let req = Request {
        authorization: None,
        path: "/admin".to_owned(),
        ..request()
      };

      assert_eq!(
        authorize(&req, "pod_mcp_secret"),
        Err(Reject::NotFound),
        "an unknown path is a 404 regardless of the token"
      );
    }
  }

  mod parse_request {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_a_post_with_headers_and_a_body() {
      let raw = "POST /mcp?x=1 HTTP/1.1\r\nHost: 127.0.0.1:7373\r\nAuthorization: Bearer abc\r\nOrigin: http://localhost\r\n\r\n{\"jsonrpc\":\"2.0\"}";

      let parsed = parse_request(raw).unwrap();

      assert_eq!(parsed.method, "POST");
      assert_eq!(parsed.path, "/mcp", "the query string is stripped from the path");
      assert_eq!(parsed.authorization.as_deref(), Some("Bearer abc"));
      assert_eq!(parsed.host.as_deref(), Some("127.0.0.1:7373"));
      assert_eq!(parsed.origin.as_deref(), Some("http://localhost"));
      assert_eq!(parsed.body, "{\"jsonrpc\":\"2.0\"}");
    }

    #[test]
    fn it_matches_header_names_case_insensitively() {
      let raw = "POST /mcp HTTP/1.1\r\nhost: localhost\r\nAUTHORIZATION: Bearer abc\r\n\r\n";

      let parsed = parse_request(raw).unwrap();

      assert_eq!(parsed.authorization.as_deref(), Some("Bearer abc"));
      assert_eq!(parsed.host.as_deref(), Some("localhost"));
    }

    #[test]
    fn it_returns_none_for_an_empty_request() {
      assert_eq!(parse_request(""), None);
    }
  }

  mod http_response {
    use super::*;

    #[test]
    fn it_sets_a_matching_content_length() {
      let body = "{\"ok\":true}";

      let response = http_response("HTTP/1.1 200 OK", body);

      assert!(response.contains(&format!("Content-Length: {}", body.len())));
      assert!(response.ends_with(body));
    }
  }
}
