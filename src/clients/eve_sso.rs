use std::sync::Arc;

use base64::Engine as _;
use chrono::{DateTime, Utc};
use getset::Getters;
use rand::Rng;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::clients::{self, http};

const AUTH_URL: &str = "https://login.eveonline.com/v2/oauth/authorize";
const TOKEN_URL: &str = "https://login.eveonline.com/v2/oauth/token";

pub struct Client {
  client_id: String,
  http: Arc<http::Client>,
  token_url: String,
}

impl Client {
  pub fn derive_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash)
  }

  pub fn generate_state() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
  }

  pub fn generate_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
  }

  pub fn new(http: Arc<http::Client>, client_id: impl Into<String>) -> Self {
    Self {
      client_id: client_id.into(),
      http,
      token_url: TOKEN_URL.to_owned(),
    }
  }

  #[cfg(test)]
  pub fn with_token_url(mut self, url: impl Into<String>) -> Self {
    self.token_url = url.into();
    self
  }

  pub async fn exchange_code(&self, code: &str, redirect_uri: &str, verifier: &str) -> Result<Grant, clients::Error> {
    let resp: TokenResponse = self
      .http
      .post_form(
        &self.token_url,
        &[
          ("client_id", self.client_id.as_str()),
          ("code", code),
          ("code_verifier", verifier),
          ("grant_type", "authorization_code"),
          ("redirect_uri", redirect_uri),
        ],
      )
      .await?;
    self.build_grant(resp)
  }

  pub async fn refresh_with_token(&self, refresh_token: &str) -> Result<Grant, clients::Error> {
    let resp: TokenResponse = self
      .http
      .post_form(
        &self.token_url,
        &[
          ("client_id", self.client_id.as_str()),
          ("grant_type", "refresh_token"),
          ("refresh_token", refresh_token),
        ],
      )
      .await?;
    self.build_grant(resp)
  }

  pub fn sign_in(&self, scopes: &[&str], redirect_uri: &str) -> PendingAuth {
    let verifier = Self::generate_verifier();
    let state = Self::generate_state();
    let challenge = Self::derive_challenge(&verifier);
    let scopes_str = scopes.join(" ");
    let client_id = &self.client_id;
    let url = format!(
      "{AUTH_URL}?client_id={client_id}&code_challenge={challenge}&code_challenge_method=S256&redirect_uri={redirect_uri}&response_type=code&scope={scopes_str}&state={state}"
    );
    PendingAuth {
      state,
      url,
      verifier,
    }
  }

  /// Decodes the JWT payload without verifying its signature; trust comes from receiving the
  /// token over TLS directly from the EVE SSO token endpoint, not from the token itself.
  fn decode_jwt_claims(token: &str) -> Result<JwtClaims, clients::Error> {
    let payload = token
      .split('.')
      .nth(1)
      .ok_or_else(|| clients::Error::Auth("token is not a JWT".into()))?;
    let decoded = decode_base64url(payload).map_err(|e| clients::Error::Auth(format!("base64 decode: {e}")))?;
    serde_json::from_slice(&decoded).map_err(clients::Error::from)
  }

  fn build_grant(&self, resp: TokenResponse) -> Result<Grant, clients::Error> {
    let claims = Self::decode_jwt_claims(&resp.access_token)?;
    let character_id = parse_id_from_sub(&claims.sub)?;
    let scopes = extract_scopes(claims.scp);
    let expires_at = Utc::now() + chrono::Duration::seconds(resp.expires_in as i64);
    Ok(Grant::new(
      resp.access_token,
      character_id,
      claims.name,
      expires_at,
      resp.refresh_token,
      scopes,
    ))
  }
}

#[derive(Clone, Debug, Getters)]
pub struct Grant {
  #[getset(get = "pub")]
  access_token: String,
  #[getset(get = "pub")]
  character_id: i64,
  #[getset(get = "pub")]
  character_name: String,
  #[getset(get = "pub")]
  expires_at: DateTime<Utc>,
  #[getset(get = "pub")]
  refresh_token: String,
  #[getset(get = "pub")]
  scopes: Vec<String>,
}

impl Grant {
  pub fn from_stored(
    access_token: impl Into<String>,
    character_id: i64,
    expires_at: DateTime<Utc>,
    refresh_token: impl Into<String>,
    scopes: Vec<String>,
  ) -> Self {
    Self::new(
      access_token,
      character_id,
      String::new(),
      expires_at,
      refresh_token,
      scopes,
    )
  }

  #[cfg(test)]
  pub fn new_test(access_token: impl Into<String>, character_id: i64) -> Self {
    Self::new(
      access_token,
      character_id,
      "Test Character",
      Utc::now() + chrono::Duration::hours(1),
      "refresh-token",
      vec![],
    )
  }

  #[cfg(test)]
  pub fn new_test_with_scopes(access_token: impl Into<String>, character_id: i64, scopes: Vec<String>) -> Self {
    Self::new(
      access_token,
      character_id,
      "Test Character",
      Utc::now() + chrono::Duration::hours(1),
      "refresh-token",
      scopes,
    )
  }

  fn new(
    access_token: impl Into<String>,
    character_id: i64,
    character_name: impl Into<String>,
    expires_at: DateTime<Utc>,
    refresh_token: impl Into<String>,
    scopes: Vec<String>,
  ) -> Self {
    Self {
      access_token: access_token.into(),
      character_id,
      character_name: character_name.into(),
      expires_at,
      refresh_token: refresh_token.into(),
      scopes,
    }
  }

  pub fn has_scope(&self, scope: &str) -> bool {
    self.scopes.iter().any(|granted| granted == scope)
  }
}

#[derive(Clone, Debug)]
pub struct PendingAuth {
  pub state: String,
  pub url: String,
  pub verifier: String,
}

impl PendingAuth {
  pub fn validate_state(&self, returned: &str) -> Result<(), clients::Error> {
    if self.state == returned {
      Ok(())
    } else {
      Err(clients::Error::Auth("state mismatch".into()))
    }
  }
}

#[derive(Deserialize)]
struct JwtClaims {
  name: String,
  scp: Option<serde_json::Value>,
  sub: String,
}

#[derive(Deserialize)]
struct TokenResponse {
  access_token: String,
  expires_in: u64,
  refresh_token: String,
}

/// Decodes EVE SSO's unpadded base64url JWT segments by restoring the stripped '=' padding
/// before decoding.
fn decode_base64url(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
  let padded = pad_base64url(s.trim_end_matches('='));
  base64::engine::general_purpose::URL_SAFE.decode(padded)
}

fn extract_scopes(scp: Option<serde_json::Value>) -> Vec<String> {
  match scp {
    None => vec![],
    Some(serde_json::Value::Array(arr)) => arr.into_iter().filter_map(|v| v.as_str().map(str::to_owned)).collect(),
    Some(serde_json::Value::String(s)) => vec![s],
    Some(_) => vec![],
  }
}

fn pad_base64url(stripped: &str) -> String {
  match stripped.len() % 4 {
    2 => format!("{stripped}=="),
    3 => format!("{stripped}="),
    _ => stripped.to_owned(),
  }
}

/// Extracts the numeric character id from the colon-delimited 'sub' claim (CHARACTER:EVE:<id>),
/// taking the segment after the last ':'.
fn parse_id_from_sub(sub: &str) -> Result<i64, clients::Error> {
  sub
    .split(':')
    .next_back()
    .and_then(|s| s.parse::<i64>().ok())
    .ok_or_else(|| clients::Error::Auth(format!("malformed sub claim: {sub}")))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_jwt_with_scp(sub: &str, name: &str, scp: &str) -> String {
    let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
    let payload_json = format!(r#"{{"sub":"{sub}","name":"{name}","scp":{scp}}}"#);
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json);
    format!("{header}.{payload}.fakesig")
  }

  fn make_jwt(sub: &str, name: &str) -> String {
    make_jwt_with_scp(sub, name, r#""esi-skills.read_skills.v1""#)
  }

  async fn make_test_client() -> Client {
    let db = crate::store::open_test().await.unwrap();
    let cache = http::Cache::new(db);
    let http = http::Client::builder(cache).build();
    Client::new(http, "test-client")
  }

  mod client {
    use super::*;

    mod build_grant {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_returns_error_for_invalid_jwt() {
        let client = make_test_client().await;
        let resp = TokenResponse {
          access_token: "not-a-jwt".into(),
          expires_in: 1200,
          refresh_token: "rt".into(),
        };

        assert!(client.build_grant(resp).is_err());
      }

      #[tokio::test]
      async fn it_returns_grant_with_array_scopes() {
        let client = make_test_client().await;
        let token = make_jwt_with_scp(
          "CHARACTER:EVE:11111111",
          "Char",
          r#"["esi-skills.read_skills.v1","esi-wallet.read_character_wallet.v1"]"#,
        );
        let resp = TokenResponse {
          access_token: token,
          expires_in: 1200,
          refresh_token: "rt".into(),
        };

        let grant = client.build_grant(resp).unwrap();

        assert_eq!(grant.scopes().len(), 2);
      }

      #[tokio::test]
      async fn it_returns_grant_with_character_data() {
        let client = make_test_client().await;
        let token = make_jwt("CHARACTER:EVE:99887766", "Eve Char");
        let resp = TokenResponse {
          access_token: token,
          expires_in: 1200,
          refresh_token: "rt-abc".into(),
        };

        let grant = client.build_grant(resp).unwrap();

        assert_eq!(*grant.character_id(), 99887766i64);
        assert_eq!(grant.character_name(), "Eve Char");
        assert_eq!(grant.refresh_token(), "rt-abc");
      }
    }

    mod derive_challenge {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_produces_rfc7636_appendix_b_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

        assert_eq!(Client::derive_challenge(verifier), expected);
      }
    }

    mod generate_state {
      use super::*;

      #[test]
      fn it_produces_non_empty_url_safe_string() {
        let state = Client::generate_state();

        assert!(!state.is_empty());
        assert!(state.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
      }

      #[test]
      fn it_produces_unique_values() {
        let a = Client::generate_state();
        let b = Client::generate_state();

        assert_ne!(a, b);
      }
    }

    mod generate_verifier {
      use super::*;

      #[test]
      fn it_produces_non_empty_url_safe_string() {
        let verifier = Client::generate_verifier();

        assert!(!verifier.is_empty());
        assert!(verifier.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
      }

      #[test]
      fn it_produces_unique_values() {
        let a = Client::generate_verifier();
        let b = Client::generate_verifier();

        assert_ne!(a, b);
      }
    }

    mod refresh_with_token {
      use pretty_assertions::assert_eq;
      use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, method, path},
      };

      use super::*;

      #[tokio::test]
      async fn it_returns_error_when_token_endpoint_rejects() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/token"))
          .respond_with(ResponseTemplate::new(400))
          .mount(&server)
          .await;
        let db = crate::store::open_test().await.unwrap();
        let http = http::Client::builder(http::Cache::new(db)).build();
        let client = Client::new(http, "test-client").with_token_url(format!("{}/token", server.uri()));

        assert!(client.refresh_with_token("dead-rt").await.is_err());
      }

      #[tokio::test]
      async fn it_rotates_tokens_from_a_raw_refresh_token() {
        let server = MockServer::start().await;
        let access = make_jwt("CHARACTER:EVE:42", "Refreshed");
        let body = format!(r#"{{"access_token":"{access}","expires_in":1200,"refresh_token":"new-rt"}}"#);
        Mock::given(method("POST"))
          .and(path("/token"))
          .and(body_string_contains("refresh_token=old-rt"))
          .and(body_string_contains("grant_type=refresh_token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body.into_bytes(), "application/json"))
          .mount(&server)
          .await;
        let db = crate::store::open_test().await.unwrap();
        let http = http::Client::builder(http::Cache::new(db)).build();
        let client = Client::new(http, "test-client").with_token_url(format!("{}/token", server.uri()));

        let grant = client.refresh_with_token("old-rt").await.unwrap();

        assert_eq!(*grant.character_id(), 42);
        assert_eq!(grant.refresh_token(), "new-rt");
      }
    }

    mod sign_in {
      use super::*;

      #[tokio::test]
      async fn it_returns_pending_auth_with_required_url_params() {
        let client = make_test_client().await;

        let pending = client.sign_in(&["esi-skills.read_skills.v1"], "http://localhost:8080/callback");

        assert!(pending.url.contains("response_type=code"));
        assert!(pending.url.contains("client_id=test-client"));
        assert!(pending.url.contains("code_challenge_method=S256"));
        assert!(!pending.verifier.is_empty());
        assert!(!pending.state.is_empty());
      }
    }
  }

  mod extract_scopes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_all_strings_for_array_value() {
      let scp = Some(serde_json::json!([
        "esi-skills.read_skills.v1",
        "esi-wallet.read_character_wallet.v1"
      ]));

      let result = extract_scopes(scp);

      assert_eq!(result.len(), 2);
      assert!(result.contains(&"esi-skills.read_skills.v1".to_owned()));
      assert!(result.contains(&"esi-wallet.read_character_wallet.v1".to_owned()));
    }

    #[test]
    fn it_returns_empty_vec_for_non_string_non_array_value() {
      assert_eq!(
        extract_scopes(Some(serde_json::Value::Bool(true))),
        Vec::<String>::new()
      );
    }

    #[test]
    fn it_returns_empty_vec_for_none() {
      assert_eq!(extract_scopes(None), Vec::<String>::new());
    }

    #[test]
    fn it_returns_single_element_for_string_value() {
      let scp = Some(serde_json::Value::String("esi-skills.read_skills.v1".to_owned()));

      assert_eq!(extract_scopes(scp), vec!["esi-skills.read_skills.v1"]);
    }
  }

  mod grant {
    use super::*;

    mod has_scope {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_reports_whether_a_scope_was_granted() {
        let grant = Grant::new(
          "token",
          1,
          "Name",
          Utc::now() + chrono::Duration::hours(1),
          "rt",
          vec![
            "esi-location.read_location.v1".to_owned(),
            "esi-universe.read_structures.v1".to_owned(),
          ],
        );

        assert_eq!(grant.has_scope("esi-universe.read_structures.v1"), true);
        assert_eq!(grant.has_scope("esi-wallet.read_character_wallet.v1"), false);
      }
    }
  }

  mod pad_base64url {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_adds_one_padding_char_when_remainder_is_three() {
      assert_eq!(pad_base64url("abc"), "abc=");
    }

    #[test]
    fn it_adds_two_padding_chars_when_remainder_is_two() {
      assert_eq!(pad_base64url("ab"), "ab==");
    }

    #[test]
    fn it_leaves_string_unchanged_when_already_aligned() {
      assert_eq!(pad_base64url("abcd"), "abcd");
    }
  }

  mod pending_auth {
    use super::*;

    mod validate_state {
      use super::*;

      #[test]
      fn it_returns_error_when_states_differ() {
        let pending = PendingAuth {
          state: "abc".into(),
          url: String::new(),
          verifier: String::new(),
        };

        assert!(pending.validate_state("xyz").is_err());
      }

      #[test]
      fn it_returns_ok_when_states_match() {
        let pending = PendingAuth {
          state: "abc".into(),
          url: String::new(),
          verifier: String::new(),
        };

        assert!(pending.validate_state("abc").is_ok());
      }
    }
  }
}
