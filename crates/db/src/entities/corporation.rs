//! Database entity for EVE Online corporation accounts.

use pod_model::Corporation;
use sea_orm::{Set, prelude::*};

/// A corporation stored in the `corporations` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "corporations")]
pub struct Model {
  /// OAuth access token for ESI corp API calls.
  pub access_token: String,
  /// Alliance ID, if the corporation is in an alliance.
  pub alliance_id: Option<i64>,
  /// Alliance name, if the corporation is in an alliance.
  pub alliance_name: Option<String>,
  /// EVE character ID of the authenticating director/accountant.
  pub auth_character_id: i64,
  /// EVE character ID of the corporation CEO.
  pub ceo_character_id: i64,
  /// ISO date string for when the corporation was founded.
  pub date_founded: Option<String>,
  /// Corporation description.
  pub description: Option<String>,
  /// Faction ID, if the corporation is enrolled in factional warfare.
  pub faction_id: Option<i64>,
  /// EVE station or structure ID of the corporation headquarters.
  pub home_station_id: Option<i64>,
  /// Raw corporation icon PNG bytes (128×128).
  pub icon_data: Option<Vec<u8>>,
  /// Unique EVE corporation ID.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i64,
  /// Number of corporation members.
  pub member_count: i32,
  /// EVE corporation name.
  pub name: String,
  /// OAuth refresh token.
  pub refresh_token: String,
  /// JSON-encoded list of granted OAuth scopes.
  pub scopes: String,
  /// Number of shares the corporation has issued.
  pub shares: Option<i64>,
  /// Corporation tax rate as a fraction (0.0–1.0).
  pub tax_rate: f64,
  /// EVE corporation ticker symbol.
  pub ticker: String,
  /// Unix timestamp at which the access token expires.
  pub token_expires_at: i64,
  /// Corporation website URL.
  pub url: Option<String>,
  /// Whether the corporation is eligible for war declarations.
  pub war_eligible: Option<bool>,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for Corporation {
  fn from(entity: Model) -> Self {
    let scopes: Vec<String> = serde_json::from_str(&entity.scopes).unwrap_or_default();
    let mut model = Corporation::new(entity.id, entity.name);
    model
      .set_access_token(entity.access_token)
      .set_alliance_id(entity.alliance_id)
      .set_alliance_name(entity.alliance_name)
      .set_auth_character_id(entity.auth_character_id)
      .set_ceo_character_id(entity.ceo_character_id)
      .set_date_founded(entity.date_founded)
      .set_description(entity.description)
      .set_faction_id(entity.faction_id)
      .set_home_station_id(entity.home_station_id)
      .set_member_count(entity.member_count)
      .set_refresh_token(entity.refresh_token)
      .set_shares(entity.shares)
      .set_tax_rate(entity.tax_rate)
      .set_ticker(entity.ticker)
      .set_token_expires_at(entity.token_expires_at)
      .set_url(entity.url)
      .set_war_eligible(entity.war_eligible)
      .mark_persisted();
    *model.icon_data_mut() = entity.icon_data;
    *model.scopes_mut() = scopes;
    model
  }
}

impl From<Corporation> for ActiveModel {
  fn from(model: Corporation) -> Self {
    let scopes = serde_json::to_string(model.scopes()).unwrap_or_default();
    Self {
      access_token: Set(model.access_token().clone()),
      alliance_id: Set(*model.alliance_id()),
      alliance_name: Set(model.alliance_name().clone()),
      auth_character_id: Set(*model.auth_character_id()),
      ceo_character_id: Set(*model.ceo_character_id()),
      date_founded: Set(model.date_founded().clone()),
      description: Set(model.description().clone()),
      faction_id: Set(*model.faction_id()),
      home_station_id: Set(*model.home_station_id()),
      icon_data: Set(model.icon_data().clone()),
      id: Set(*model.id()),
      member_count: Set(*model.member_count()),
      name: Set(model.name().clone()),
      refresh_token: Set(model.refresh_token().clone()),
      scopes: Set(scopes),
      shares: Set(*model.shares()),
      tax_rate: Set(*model.tax_rate()),
      ticker: Set(model.ticker().clone()),
      token_expires_at: Set(*model.token_expires_at()),
      url: Set(model.url().clone()),
      war_eligible: Set(*model.war_eligible()),
    }
  }
}
