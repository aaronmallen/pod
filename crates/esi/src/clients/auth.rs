//! EVE SSO OAuth2 PKCE authentication client.

use std::time::{Duration, SystemTime};

use base64::Engine as _;
use rand::Rng;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{Error, models::auth::Grant};

const EVE_AUTH_URL: &str = "https://login.eveonline.com/v2/oauth/authorize";
const EVE_TOKEN_URL: &str = "https://login.eveonline.com/v2/oauth/token";

/// EVE SSO PKCE authentication client bound to an [`crate::Client`].
pub struct Client<'a> {
  esi: &'a crate::Client,
}

impl<'a> Client<'a> {
  /// Creates a new `Client` bound to the given ESI client.
  pub(crate) fn new(esi: &'a crate::Client) -> Self {
    Self {
      esi,
    }
  }

  /// Listens on `localhost:<port>` for the OAuth2 redirect and returns the
  /// `(code, state)` query parameters once the callback arrives.
  pub async fn await_callback(&self, port: u16) -> Result<(String, String), Error> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
      .await
      .map_err(|e| Error::Internal(format!("bind failed: {e}")))?;

    let (mut stream, _) = listener
      .accept()
      .await
      .map_err(|e| Error::Internal(format!("accept failed: {e}")))?;

    let mut buf = vec![0u8; 4096];
    let n = stream
      .read(&mut buf)
      .await
      .map_err(|e| Error::Internal(format!("read failed: {e}")))?;

    let request = String::from_utf8_lossy(&buf[..n]);
    let first_line = request
      .lines()
      .next()
      .ok_or_else(|| Error::Internal("empty HTTP request".into()))?;

    // First line is "GET /?code=...&state=... HTTP/1.1"
    let path = first_line
      .split_whitespace()
      .nth(1)
      .ok_or_else(|| Error::Internal("malformed HTTP request".into()))?;

    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");

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

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nAuthorized. You may close this tab.";
    let _ = stream.write_all(response.as_bytes()).await;

    let code = code.ok_or_else(|| Error::Internal("missing code in callback".into()))?;
    let state = state.ok_or_else(|| Error::Internal("missing state in callback".into()))?;
    Ok((code, state))
  }

  /// Computes the PKCE S256 code challenge from a code verifier.
  pub fn derive_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash)
  }

  /// Exchanges an authorization code for a [`Grant`].
  pub async fn exchange_code(&self, code: &str, redirect_uri: &str, verifier: &str) -> Result<Grant, Error> {
    let resp: TokenResponse = self
      .esi
      .http()
      .post_form_anon(
        EVE_TOKEN_URL,
        &[
          ("grant_type", "authorization_code"),
          ("code", code),
          ("client_id", self.esi.id()),
          ("code_verifier", verifier),
          ("redirect_uri", redirect_uri),
        ],
      )
      .await?;
    self.build_grant(resp)
  }

  /// Generates a random PKCE state string for CSRF protection.
  pub fn generate_state() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
  }

  /// Generates a random PKCE code verifier.
  pub fn generate_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
  }

  /// Parses the EVE character ID from the `sub` claim of a JWT access token.
  pub fn parse_character_id(token: &str) -> Result<i64, Error> {
    let claims = Self::decode_jwt_claims(token)?;
    claims
      .sub
      .split(':')
      .next_back()
      .and_then(|s| s.parse::<i64>().ok())
      .ok_or_else(|| Error::Authentication(format!("malformed sub claim: {}", claims.sub)))
  }

  /// Uses the refresh token in `grant` to obtain a new [`Grant`].
  #[tracing::instrument(skip(self, grant), fields(character_id = grant.character_id()))]
  pub async fn refresh(&self, grant: &Grant) -> Result<Grant, Error> {
    let resp: TokenResponse = self
      .esi
      .http()
      .post_form_anon(
        EVE_TOKEN_URL,
        &[
          ("grant_type", "refresh_token"),
          ("refresh_token", grant.refresh_token()),
          ("client_id", self.esi.id()),
        ],
      )
      .await?;
    self.build_grant(resp)
  }

  /// Builds the EVE SSO authorization URL and returns it with the PKCE verifier and state.
  pub fn sign_in(&self, scopes: &[&str], redirect_uri: &str) -> (String, String, String) {
    let verifier = Self::generate_verifier();
    let state = Self::generate_state();
    let challenge = Self::derive_challenge(&verifier);
    let scopes_str = scopes.join(" ");
    let client_id = self.esi.id();
    let url = format!(
      "{EVE_AUTH_URL}?response_type=code&client_id={client_id}&redirect_uri={redirect_uri}&scope={scopes_str}&code_challenge={challenge}&code_challenge_method=S256&state={state}"
    );
    (url, verifier, state)
  }

  /// Returns `Ok(())` if `expected` and `returned` are equal, otherwise an auth error.
  pub fn validate_state(&self, expected: &str, returned: &str) -> Result<(), Error> {
    if expected == returned {
      Ok(())
    } else {
      Err(Error::Authentication("state mismatch".into()))
    }
  }

  /// Builds a [`Grant`] from a [`TokenResponse`] by decoding the JWT claims.
  fn build_grant(&self, resp: TokenResponse) -> Result<Grant, Error> {
    let character_id = Self::parse_character_id(&resp.access_token)?;
    let claims = Self::decode_jwt_claims(&resp.access_token)?;
    let scopes = match claims.scp {
      None => vec![],
      Some(serde_json::Value::String(s)) => vec![s],
      Some(serde_json::Value::Array(arr)) => arr.into_iter().filter_map(|v| v.as_str().map(str::to_owned)).collect(),
      Some(_) => vec![],
    };
    let expires_at = SystemTime::now() + Duration::from_secs(resp.expires_in);
    Ok(Grant::new(
      resp.access_token,
      character_id,
      claims.name,
      expires_at,
      resp.refresh_token,
      scopes,
    ))
  }

  /// Decodes the payload section of a JWT without signature verification.
  fn decode_jwt_claims(token: &str) -> Result<JwtClaims, Error> {
    let payload = token
      .split('.')
      .nth(1)
      .ok_or_else(|| Error::Authentication("token is not a JWT".into()))?;
    let decoded = decode_base64url(payload).map_err(|e| Error::Authentication(format!("base64 decode: {e}")))?;
    serde_json::from_slice(&decoded).map_err(Error::from)
  }
}

/// Token endpoint response body.
#[derive(Deserialize)]
struct TokenResponse {
  access_token: String,
  expires_in: u64,
  refresh_token: String,
}

/// Subset of JWT claims present in EVE SSO access tokens.
#[derive(Deserialize)]
struct JwtClaims {
  name: String,
  scp: Option<serde_json::Value>,
  sub: String,
}

/// Decodes a base64url-encoded string, accepting both padded and unpadded input.
fn decode_base64url(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
  // Strip any existing padding and add correct padding for standard decode
  let stripped = s.trim_end_matches('=');
  let padded = match stripped.len() % 4 {
    2 => format!("{stripped}=="),
    3 => format!("{stripped}="),
    _ => stripped.to_owned(),
  };
  base64::engine::general_purpose::URL_SAFE.decode(padded)
}

#[cfg(test)]
mod tests {
  use super::*;

  mod derive_challenge {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_produces_rfc7636_appendix_b_vector() {
      // RFC 7636 Appendix B test vector
      let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
      let expected = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

      assert_eq!(Client::derive_challenge(verifier), expected);
    }
  }

  mod parse_character_id {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_jwt(sub: &str, name: &str) -> String {
      let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
      let payload_json = format!(r#"{{"sub":"{sub}","name":"{name}","scp":"esi-skills.read_skills.v1"}}"#);
      let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json);
      format!("{header}.{payload}.fakesig")
    }

    #[test]
    fn it_parses_character_id_from_valid_jwt() {
      let token = make_jwt("CHARACTER:EVE:12345678", "Test Character");

      assert_eq!(Client::parse_character_id(&token).unwrap(), 12345678);
    }

    #[test]
    fn it_returns_error_for_non_jwt() {
      let result = Client::parse_character_id("not-a-jwt");

      assert!(result.is_err());
    }

    #[test]
    fn it_returns_error_for_malformed_sub() {
      let token = make_jwt("BADINPUT", "Test Character");
      let result = Client::parse_character_id(&token);

      assert!(result.is_err());
    }
  }
}
