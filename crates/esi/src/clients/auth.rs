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
    let scopes = extract_scopes(claims.scp);
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
  let padded = pad_base64url(s.trim_end_matches('='));
  base64::engine::general_purpose::URL_SAFE.decode(padded)
}

/// Normalizes the `scp` JWT claim into a `Vec<String>`.
///
/// - `None` or `null` → empty vec
/// - `String` → single-element vec
/// - `Array` → all string elements, non-strings filtered out
/// - Any other JSON value → empty vec
fn extract_scopes(scp: Option<serde_json::Value>) -> Vec<String> {
  match scp {
    None => vec![],
    Some(serde_json::Value::String(s)) => vec![s],
    Some(serde_json::Value::Array(arr)) => arr.into_iter().filter_map(|v| v.as_str().map(str::to_owned)).collect(),
    Some(_) => vec![],
  }
}

/// Adds the correct `=` padding to a stripped base64url string.
fn pad_base64url(stripped: &str) -> String {
  match stripped.len() % 4 {
    2 => format!("{stripped}=="),
    3 => format!("{stripped}="),
    _ => stripped.to_owned(),
  }
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

  fn make_esi_client() -> crate::Client {
    crate::ClientBuilder::new("test-client").build().unwrap()
  }

  mod build_grant {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_builds_grant_with_string_scope() {
      let esi = make_esi_client();
      let auth = Client::new(&esi);
      let token = make_jwt("CHARACTER:EVE:99887766", "Test Char");
      let resp = TokenResponse {
        access_token: token.clone(),
        expires_in: 1200,
        refresh_token: "rt".to_owned(),
      };

      let grant = auth.build_grant(resp).unwrap();

      assert_eq!(*grant.character_id(), 99887766i64);
      assert_eq!(grant.character_name(), "Test Char");
      assert_eq!(grant.scopes(), &vec!["esi-skills.read_skills.v1".to_owned()]);
      assert_eq!(grant.refresh_token(), "rt");
    }

    #[test]
    fn it_builds_grant_with_array_of_scopes() {
      let esi = make_esi_client();
      let auth = Client::new(&esi);
      let token = make_jwt_with_scp(
        "CHARACTER:EVE:11111111",
        "Array Char",
        r#"["esi-skills.read_skills.v1","esi-wallet.read_character_wallet.v1"]"#,
      );
      let resp = TokenResponse {
        access_token: token.clone(),
        expires_in: 1200,
        refresh_token: "rt2".to_owned(),
      };

      let grant = auth.build_grant(resp).unwrap();

      assert_eq!(grant.scopes().len(), 2);
      assert!(grant.scopes().contains(&"esi-skills.read_skills.v1".to_owned()));
    }

    #[test]
    fn it_builds_grant_with_no_scopes_when_scp_is_null() {
      let esi = make_esi_client();
      let auth = Client::new(&esi);
      let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
      let payload_json = r#"{"sub":"CHARACTER:EVE:22222222","name":"No Scopes","scp":null}"#;
      let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json);
      let token = format!("{header}.{payload}.fakesig");
      let resp = TokenResponse {
        access_token: token,
        expires_in: 1200,
        refresh_token: "rt3".to_owned(),
      };

      let grant = auth.build_grant(resp).unwrap();

      assert_eq!(grant.scopes().len(), 0);
    }

    #[test]
    fn it_returns_error_for_invalid_jwt() {
      let esi = make_esi_client();
      let auth = Client::new(&esi);
      let resp = TokenResponse {
        access_token: "not-a-jwt".to_owned(),
        expires_in: 1200,
        refresh_token: "rt".to_owned(),
      };

      let result = auth.build_grant(resp);

      assert!(result.is_err());
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

  mod extract_scopes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_empty_vec_for_none() {
      assert_eq!(extract_scopes(None), Vec::<String>::new());
    }

    #[test]
    fn it_returns_single_element_for_string_value() {
      let scp = Some(serde_json::Value::String("esi-skills.read_skills.v1".to_owned()));

      assert_eq!(extract_scopes(scp), vec!["esi-skills.read_skills.v1"]);
    }

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
      let scp = Some(serde_json::Value::Bool(true));

      assert_eq!(extract_scopes(scp), Vec::<String>::new());
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

  mod pad_base64url {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_adds_two_padding_chars_when_remainder_is_two() {
      // "ab" has length 2, 2 % 4 == 2
      assert_eq!(pad_base64url("ab"), "ab==");
    }

    #[test]
    fn it_adds_one_padding_char_when_remainder_is_three() {
      // "abc" has length 3, 3 % 4 == 3
      assert_eq!(pad_base64url("abc"), "abc=");
    }

    #[test]
    fn it_leaves_string_unchanged_when_already_aligned() {
      // "abcd" has length 4, 4 % 4 == 0
      assert_eq!(pad_base64url("abcd"), "abcd");
    }
  }

  mod parse_character_id {
    use pretty_assertions::assert_eq;

    use super::*;

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

  mod sign_in {
    use super::*;

    #[test]
    fn it_returns_url_verifier_and_state() {
      let esi = make_esi_client();
      let auth = Client::new(&esi);

      let (url, verifier, state) = auth.sign_in(&["esi-skills.read_skills.v1"], "http://localhost:8080/callback");

      assert!(url.contains("response_type=code"));
      assert!(url.contains("client_id=test-client"));
      assert!(url.contains("code_challenge_method=S256"));
      assert!(!verifier.is_empty());
      assert!(!state.is_empty());
    }
  }

  mod validate_state {
    use super::*;

    #[test]
    fn it_returns_ok_when_states_match() {
      let esi = make_esi_client();
      let auth = Client::new(&esi);

      assert!(auth.validate_state("abc", "abc").is_ok());
    }

    #[test]
    fn it_returns_error_when_states_differ() {
      let esi = make_esi_client();
      let auth = Client::new(&esi);

      assert!(auth.validate_state("abc", "xyz").is_err());
    }
  }
}
