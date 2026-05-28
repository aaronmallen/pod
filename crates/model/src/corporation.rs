//! Domain model for an EVE Online corporation account.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use getset::{Getters, MutGetters};
use validator::Validate;

/// A corporation authenticated via EVE SSO with corp-level API scopes.
///
/// Tracks OAuth tokens and public identity data fetched from ESI.
/// DB-mapped fields use `set_*` setters that mark the model dirty when
/// already persisted. Transient fields (`icon_data`, `scopes`) are
/// accessed via mutable getters without dirty tracking.
#[derive(Clone, Debug, Getters, MutGetters, Validate)]
pub struct Model {
  /// OAuth access token for ESI corp API calls.
  #[get = "pub"]
  access_token: String,
  /// Alliance ID, if the corporation is a member of an alliance.
  #[get = "pub"]
  alliance_id: Option<i64>,
  /// Alliance name, if the corporation is a member of an alliance.
  #[get = "pub"]
  alliance_name: Option<String>,
  /// EVE character ID of the authenticating director/accountant.
  #[get = "pub"]
  auth_character_id: i64,
  /// EVE character ID of the corporation CEO.
  #[get = "pub"]
  ceo_character_id: i64,
  /// ISO date string for when the corporation was founded.
  #[get = "pub"]
  date_founded: Option<String>,
  /// Corporation description from ESI.
  #[get = "pub"]
  description: Option<String>,
  dirty: bool,
  /// Faction ID, if enrolled in factional warfare.
  #[get = "pub"]
  faction_id: Option<i64>,
  /// EVE station or structure ID of the corporation headquarters.
  #[get = "pub"]
  home_station_id: Option<i64>,
  /// Resolved display name of the corporation headquarters station.
  #[getset(get = "pub", get_mut = "pub")]
  hq_name: Option<String>,
  /// Raw corporation icon PNG bytes (128×128), fetched from ESI.
  #[getset(get = "pub", get_mut = "pub")]
  icon_data: Option<Vec<u8>>,
  /// Unique EVE corporation ID.
  #[get = "pub"]
  id: i64,
  /// Number of corporation members.
  #[get = "pub"]
  member_count: i32,
  /// EVE corporation name.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  persisted: bool,
  /// OAuth refresh token.
  #[get = "pub"]
  refresh_token: String,
  /// OAuth scopes granted for this corporation token.
  #[getset(get = "pub", get_mut = "pub")]
  scopes: Vec<String>,
  /// Tags assigned to this corporation.
  #[getset(get = "pub", get_mut = "pub")]
  tags: Vec<(i32, String)>,
  /// Number of shares the corporation has issued.
  #[get = "pub"]
  shares: Option<i64>,
  /// Corporation tax rate as a fraction (0.0–1.0).
  #[get = "pub"]
  tax_rate: f64,
  /// EVE corporation ticker symbol.
  #[get = "pub"]
  #[validate(length(min = 1))]
  ticker: String,
  /// Unix timestamp at which the access token expires.
  #[get = "pub"]
  token_expires_at: i64,
  /// Corporation website URL.
  #[get = "pub"]
  url: Option<String>,
  /// Whether the corporation is eligible for war declarations.
  #[get = "pub"]
  war_eligible: Option<bool>,
}

impl Model {
  /// Creates a new unpersisted corporation with the given id and name.
  pub fn new(id: i64, name: impl Into<String>) -> Self {
    Self {
      access_token: String::new(),
      alliance_id: None,
      alliance_name: None,
      auth_character_id: 0,
      ceo_character_id: 0,
      date_founded: None,
      description: None,
      dirty: false,
      faction_id: None,
      home_station_id: None,
      hq_name: None,
      icon_data: None,
      id,
      member_count: 0,
      name: name.into(),
      persisted: false,
      refresh_token: String::new(),
      scopes: Vec::new(),
      shares: None,
      tags: Vec::new(),
      tax_rate: 0.0,
      ticker: String::new(),
      token_expires_at: 0,
      url: None,
      war_eligible: None,
    }
  }

  /// Returns `true` if the access token has expired or is within
  /// [`crate::ACCESS_TOKEN_EXPIRY_LEEWAY`] of expiring.
  pub fn access_token_expired(&self) -> bool {
    let expires_at = UNIX_EPOCH + Duration::from_secs(self.token_expires_at as u64);
    SystemTime::now() + crate::ACCESS_TOKEN_EXPIRY_LEEWAY >= expires_at
  }

  /// Returns `true` if any DB field has been modified since the last persist.
  pub fn has_changes(&self) -> bool {
    self.dirty
  }

  /// Returns `true` if this model was loaded from the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Marks this model as loaded from the database without affecting the dirty flag.
  pub fn mark_persisted(&mut self) -> &mut Self {
    self.persisted = true;
    self
  }

  /// Sets the OAuth access token, marking the model dirty if already persisted.
  pub fn set_access_token(&mut self, access_token: impl Into<String>) -> &mut Self {
    self.access_token = access_token.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the alliance ID, marking the model dirty if already persisted.
  pub fn set_alliance_id(&mut self, alliance_id: Option<i64>) -> &mut Self {
    self.alliance_id = alliance_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the alliance name, marking the model dirty if already persisted.
  pub fn set_alliance_name(&mut self, alliance_name: Option<String>) -> &mut Self {
    self.alliance_name = alliance_name;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the authenticating character ID, marking the model dirty if already persisted.
  pub fn set_auth_character_id(&mut self, auth_character_id: i64) -> &mut Self {
    self.auth_character_id = auth_character_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the CEO character ID, marking the model dirty if already persisted.
  pub fn set_ceo_character_id(&mut self, ceo_character_id: i64) -> &mut Self {
    self.ceo_character_id = ceo_character_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the founding date, marking the model dirty if already persisted.
  pub fn set_date_founded(&mut self, date_founded: Option<String>) -> &mut Self {
    self.date_founded = date_founded;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the corporation description, marking the model dirty if already persisted.
  pub fn set_description(&mut self, description: Option<String>) -> &mut Self {
    self.description = description;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the faction ID, marking the model dirty if already persisted.
  pub fn set_faction_id(&mut self, faction_id: Option<i64>) -> &mut Self {
    self.faction_id = faction_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the headquarters station/structure ID, marking the model dirty if already persisted.
  pub fn set_home_station_id(&mut self, home_station_id: Option<i64>) -> &mut Self {
    self.home_station_id = home_station_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the member count, marking the model dirty if already persisted.
  pub fn set_member_count(&mut self, member_count: i32) -> &mut Self {
    self.member_count = member_count;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the corporation name, marking the model dirty if already persisted.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the OAuth refresh token, marking the model dirty if already persisted.
  pub fn set_refresh_token(&mut self, refresh_token: impl Into<String>) -> &mut Self {
    self.refresh_token = refresh_token.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the number of shares, marking the model dirty if already persisted.
  pub fn set_shares(&mut self, shares: Option<i64>) -> &mut Self {
    self.shares = shares;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the tax rate, marking the model dirty if already persisted.
  pub fn set_tax_rate(&mut self, tax_rate: f64) -> &mut Self {
    self.tax_rate = tax_rate;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the ticker symbol, marking the model dirty if already persisted.
  pub fn set_ticker(&mut self, ticker: impl Into<String>) -> &mut Self {
    self.ticker = ticker.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the token expiry timestamp, marking the model dirty if already persisted.
  pub fn set_token_expires_at(&mut self, token_expires_at: i64) -> &mut Self {
    self.token_expires_at = token_expires_at;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the corporation website URL, marking the model dirty if already persisted.
  pub fn set_url(&mut self, url: Option<String>) -> &mut Self {
    self.url = url;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the war eligibility flag, marking the model dirty if already persisted.
  pub fn set_war_eligible(&mut self, war_eligible: Option<bool>) -> &mut Self {
    self.war_eligible = war_eligible;
    if self.persisted {
      self.dirty = true;
    }
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_corp() -> Model {
    let mut c = Model::new(98000001_i64, "Test Corp");
    c.set_ticker("TEST")
      .set_auth_character_id(90000001_i64)
      .set_ceo_character_id(90000001_i64)
      .set_access_token("token")
      .set_refresh_token("refresh")
      .set_token_expires_at(0);
    c
  }

  mod access_token_expired {
    use super::*;

    #[test]
    fn it_returns_true_for_epoch_zero() {
      let c = make_corp();
      assert!(c.access_token_expired());
    }

    #[test]
    fn it_returns_false_for_far_future() {
      let mut c = make_corp();
      c.set_token_expires_at((SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 3600) as i64);
      assert!(!c.access_token_expired());
    }
  }

  mod has_changes {
    use super::*;

    #[test]
    fn it_returns_false_before_persist() {
      let mut c = make_corp();
      c.set_name("New Name");
      assert!(!c.has_changes());
    }

    #[test]
    fn it_returns_true_after_persist_and_mutation() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_name("New Name");
      assert!(c.has_changes());
    }
  }

  mod mark_persisted {
    use super::*;

    #[test]
    fn it_marks_model_as_persisted_without_dirtying() {
      let mut c = make_corp();
      c.mark_persisted();
      assert!(c.is_persisted());
      assert!(!c.has_changes());
    }
  }

  mod set_alliance_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_alliance_id(Some(99_000_001));
      assert!(c.has_changes());
    }
  }

  mod set_member_count {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_member_count(500);
      assert!(c.has_changes());
    }
  }

  mod set_alliance_name {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_alliance_name(Some("Test Alliance".into()));
      assert!(c.has_changes());
    }
  }

  mod set_auth_character_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_auth_character_id(90_000_002);
      assert!(c.has_changes());
    }
  }

  mod set_ceo_character_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_ceo_character_id(90_000_002);
      assert!(c.has_changes());
    }
  }

  mod set_date_founded {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_date_founded(Some("2003-05-06".into()));
      assert!(c.has_changes());
    }
  }

  mod set_description {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_description(Some("A test corporation.".into()));
      assert!(c.has_changes());
    }
  }

  mod set_faction_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_faction_id(Some(500_001));
      assert!(c.has_changes());
    }
  }

  mod set_home_station_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_home_station_id(Some(60_003_760));
      assert!(c.has_changes());
    }
  }

  mod set_shares {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_shares(Some(1_000_000));
      assert!(c.has_changes());
    }
  }

  mod set_tax_rate {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_tax_rate(0.1);
      assert!(c.has_changes());
    }
  }

  mod set_ticker {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_ticker("TST2");
      assert!(c.has_changes());
    }
  }

  mod set_url {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_url(Some("https://example.com".into()));
      assert!(c.has_changes());
    }
  }

  mod set_war_eligible {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_corp();
      c.mark_persisted();
      c.set_war_eligible(Some(true));
      assert!(c.has_changes());
    }
  }
}
