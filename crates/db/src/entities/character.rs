//! Database entity for EVE Online character accounts.

use pod_model::Character;
use sea_orm::{ActiveValue, Set, prelude::*};

/// A character stored in the `characters` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "characters")]
pub struct Model {
  /// OAuth access token for ESI API calls.
  pub access_token: String,
  /// ESI effective charisma (base + current implants), if synced.
  pub charisma: Option<i32>,
  /// ID of the corporation the character belongs to.
  pub corp_id: i64,
  /// Name of the corporation the character belongs to.
  pub corp_name: String,
  /// Space-separated ESI scopes granted to this character's token, if known.
  pub granted_scopes: Option<String>,
  /// Unique EVE character ID.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i64,
  /// ESI effective intelligence (base + current implants), if synced.
  pub intelligence: Option<i32>,
  /// Wallet ISK balance, if fetched.
  pub isk_balance: Option<f64>,
  /// Whether the character is currently docked.
  pub location_docked: Option<bool>,
  /// Human-readable current location name.
  pub location_name: Option<String>,
  /// ESI effective memory (base + current implants), if synced.
  pub memory: Option<i32>,
  /// EVE character name.
  pub name: String,
  /// ESI effective perception (base + current implants), if synced.
  pub perception: Option<i32>,
  /// Hue value (0–360) used to generate the portrait background.
  pub portrait_tone: i32,
  /// OAuth refresh token.
  pub refresh_token: String,
  /// Display order for the character grid (lower = earlier).
  pub sort_order: i32,
  /// Unix timestamp at which the access token expires.
  pub token_expires_at: i64,
  /// ESI effective willpower (base + current implants), if synced.
  pub willpower: Option<i32>,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for Character {
  fn from(entity: Model) -> Self {
    let mut model = Character::new(entity.id, entity.name);
    model
      .set_access_token(entity.access_token)
      .set_corp_id(entity.corp_id)
      .set_corp_name(entity.corp_name)
      .set_granted_scopes(entity.granted_scopes)
      .set_isk_balance(entity.isk_balance)
      .set_location_docked(entity.location_docked)
      .set_location_name(entity.location_name)
      .set_portrait_tone(entity.portrait_tone)
      .set_refresh_token(entity.refresh_token)
      .set_sort_order(entity.sort_order)
      .set_token_expires_at(entity.token_expires_at)
      .mark_persisted();
    model
  }
}

impl From<ModelEx> for Character {
  fn from(entity: ModelEx) -> Self {
    let mut model = Character::new(entity.id, entity.name);
    model
      .set_access_token(entity.access_token)
      .set_corp_id(entity.corp_id)
      .set_corp_name(entity.corp_name)
      .set_granted_scopes(entity.granted_scopes)
      .set_isk_balance(entity.isk_balance)
      .set_location_docked(entity.location_docked)
      .set_location_name(entity.location_name)
      .set_portrait_tone(entity.portrait_tone)
      .set_refresh_token(entity.refresh_token)
      .set_sort_order(entity.sort_order)
      .set_token_expires_at(entity.token_expires_at)
      .mark_persisted();
    model
  }
}

impl From<Character> for ActiveModel {
  fn from(model: Character) -> Self {
    Self {
      access_token: Set(model.access_token().clone()),
      charisma: ActiveValue::NotSet,
      corp_id: Set(*model.corp_id()),
      corp_name: Set(model.corp_name().clone()),
      granted_scopes: Set(model.granted_scopes().clone()),
      id: Set(*model.id()),
      intelligence: ActiveValue::NotSet,
      isk_balance: Set(*model.isk_balance()),
      location_docked: Set(*model.location_docked()),
      location_name: Set(model.location_name().clone()),
      memory: ActiveValue::NotSet,
      name: Set(model.name().clone()),
      perception: ActiveValue::NotSet,
      portrait_tone: Set(*model.portrait_tone()),
      refresh_token: Set(model.refresh_token().clone()),
      sort_order: Set(*model.sort_order()),
      token_expires_at: Set(*model.token_expires_at()),
      willpower: ActiveValue::NotSet,
    }
  }
}

impl From<Character> for ActiveModelEx {
  fn from(model: Character) -> Self {
    Self {
      access_token: Set(model.access_token().clone()),
      charisma: ActiveValue::NotSet,
      corp_id: Set(*model.corp_id()),
      corp_name: Set(model.corp_name().clone()),
      granted_scopes: Set(model.granted_scopes().clone()),
      id: Set(*model.id()),
      intelligence: ActiveValue::NotSet,
      isk_balance: Set(*model.isk_balance()),
      location_docked: Set(*model.location_docked()),
      location_name: Set(model.location_name().clone()),
      memory: ActiveValue::NotSet,
      name: Set(model.name().clone()),
      perception: ActiveValue::NotSet,
      portrait_tone: Set(*model.portrait_tone()),
      refresh_token: Set(model.refresh_token().clone()),
      sort_order: Set(*model.sort_order()),
      token_expires_at: Set(*model.token_expires_at()),
      willpower: ActiveValue::NotSet,
    }
  }
}
