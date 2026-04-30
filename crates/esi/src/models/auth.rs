//! OAuth2 grant model returned after successful EVE SSO authentication.

use std::time::SystemTime;

use getset::Getters;

/// An active OAuth2 grant holding tokens and character metadata returned by EVE SSO.
#[derive(Clone, Debug, Getters)]
pub struct Grant {
  /// The bearer access token for ESI requests.
  #[getset(get = "pub")]
  access_token: String,
  /// The authenticated EVE character's ID.
  #[getset(get = "pub")]
  character_id: i64,
  /// The authenticated EVE character's name.
  #[getset(get = "pub")]
  character_name: String,
  /// The instant at which the access token expires.
  #[getset(get = "pub")]
  expires_at: SystemTime,
  /// The refresh token used to obtain a new access token.
  #[getset(get = "pub")]
  refresh_token: String,
  /// The OAuth2 scopes granted by this token.
  #[getset(get = "pub")]
  scopes: Vec<String>,
}

impl Grant {
  /// Creates a new [`Grant`].
  pub fn new(
    access_token: impl Into<String>,
    character_id: i64,
    character_name: impl Into<String>,
    expires_at: SystemTime,
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

  /// Returns `true` if the access token has expired.
  pub fn is_expired(&self) -> bool {
    SystemTime::now() >= self.expires_at
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod grant {
    use super::*;

    mod is_expired {
      use std::time::Duration;

      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_false_when_not_expired() {
        let grant = Grant::new(
          "token",
          12345,
          "Test Character",
          SystemTime::now() + Duration::from_secs(3600),
          "refresh",
          vec![],
        );

        assert_eq!(grant.is_expired(), false);
      }

      #[test]
      fn it_returns_true_when_expired() {
        let grant = Grant::new(
          "token",
          12345,
          "Test Character",
          SystemTime::UNIX_EPOCH,
          "refresh",
          vec![],
        );

        assert_eq!(grant.is_expired(), true);
      }
    }
  }
}
