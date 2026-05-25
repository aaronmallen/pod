//! Domain model for an EVE Online character account.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use getset::{Getters, MutGetters};
use validator::Validate;

use crate::{
  character_asset::Model as CharacterAsset, character_attributes::Model as NeuralAttributes,
  character_skill::Model as CharacterSkill, clone::Clone as CharacterClone,
};

/// One entry in the EVE training queue (transient, not persisted).
#[derive(Clone, Debug)]
pub struct TrainingQueueEntry {
  pub skill_id: i32,
  pub skill_name: Option<String>,
  pub from_level: i32,
  pub to_level: i32,
  pub start_date: Option<i64>,
  pub finish_date: Option<i64>,
  pub training_start_sp: Option<i64>,
  pub level_start_sp: Option<i64>,
  pub level_end_sp: Option<i64>,
}

/// A character account with change-tracking for database persistence.
///
/// DB-mapped fields use `set_*` setters that mark the model dirty when
/// already persisted. Transient runtime fields (`skills`, `tags`,
/// `portrait_data`, `attributes`) are accessed via mutable getters without
/// dirty tracking.
#[derive(Clone, Debug, Getters, MutGetters, Validate)]
pub struct Model {
  /// OAuth access token for ESI API calls.
  #[get = "pub"]
  access_token: String,
  /// ID of the corporation the character belongs to.
  #[get = "pub"]
  corp_id: i64,
  /// Name of the corporation the character belongs to.
  #[get = "pub"]
  corp_name: String,
  dirty: bool,
  /// Space-separated ESI scopes granted to this character's OAuth token.
  #[get = "pub"]
  granted_scopes: Option<String>,
  /// Unique EVE character ID.
  #[get = "pub"]
  id: i64,
  /// Wallet ISK balance, if fetched.
  #[get = "pub"]
  isk_balance: Option<f64>,
  /// Whether the character is currently docked.
  #[get = "pub"]
  location_docked: Option<bool>,
  /// Human-readable current location name.
  #[get = "pub"]
  location_name: Option<String>,
  /// EVE character name.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  persisted: bool,
  /// Neural attribute allocation fetched from ESI; not persisted to DB.
  #[get = "pub"]
  attributes: Option<NeuralAttributes>,
  /// Assets belonging to this character.
  #[getset(get = "pub", get_mut = "pub")]
  assets: Vec<CharacterAsset>,
  /// Clones belonging to this character.
  #[getset(get = "pub", get_mut = "pub")]
  clones: Vec<CharacterClone>,
  /// Raw portrait PNG bytes fetched from the EVE image server.
  #[getset(get = "pub", get_mut = "pub")]
  portrait_data: Option<Vec<u8>>,
  /// Hue value (0–360) used to generate the portrait background.
  #[get = "pub"]
  #[validate(range(min = 0, max = 360))]
  portrait_tone: i32,
  /// OAuth refresh token.
  #[get = "pub"]
  refresh_token: String,
  /// Display order in the character grid (lower = earlier).
  #[get = "pub"]
  sort_order: i32,
  /// Skills loaded from the database or ESI.
  #[getset(get = "pub", get_mut = "pub")]
  skills: Vec<CharacterSkill>,
  /// Tags assigned to this character.
  #[getset(get = "pub", get_mut = "pub")]
  tags: Vec<(i32, String, Option<String>)>,
  /// Full training queue fetched from ESI (transient, not persisted).
  #[getset(get = "pub", get_mut = "pub")]
  training_queue: Vec<TrainingQueueEntry>,
  /// Unix timestamp at which the access token expires.
  #[get = "pub"]
  token_expires_at: i64,
}

impl Model {
  /// Creates a new unpersisted character with the given id and name.
  pub fn new(id: i64, name: impl Into<String>) -> Self {
    Self {
      access_token: String::new(),
      assets: Vec::new(),
      attributes: None,
      clones: Vec::new(),
      corp_id: 0,
      corp_name: String::new(),
      dirty: false,
      granted_scopes: None,
      id,
      isk_balance: None,
      location_docked: None,
      location_name: None,
      name: name.into(),
      persisted: false,
      portrait_data: None,
      portrait_tone: 0,
      refresh_token: String::new(),
      sort_order: 0,
      skills: Vec::new(),
      tags: Vec::new(),
      token_expires_at: 0,
      training_queue: Vec::new(),
    }
  }

  /// Returns the granted scopes as a list, treating `None` as empty.
  pub fn granted_scopes_list(&self) -> Vec<&str> {
    match &self.granted_scopes {
      Some(s) if !s.is_empty() => s.split(' ').collect(),
      _ => Vec::new(),
    }
  }

  /// Returns `true` if the access token has expired.
  pub fn access_token_expired(&self) -> bool {
    let expires_at = UNIX_EPOCH + Duration::from_secs(self.token_expires_at as u64);
    SystemTime::now() >= expires_at
  }

  /// Returns the skill currently being trained, if any.
  pub fn active_training(&self) -> Option<&CharacterSkill> {
    self.skills.iter().find(|s| s.is_active_training)
  }

  /// Returns `true` if any DB field has been modified since the last persist.
  pub fn has_changes(&self) -> bool {
    self.dirty
  }

  /// Returns `true` if this model was loaded from the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Returns the ISK balance formatted as a compact string (e.g. `"4.82B"`, `"312.0M"`).
  pub fn isk_formatted(&self) -> String {
    let Some(isk) = self.isk_balance else {
      return "\u{2014}".to_string();
    };

    let abs = isk.abs();
    let sign = if isk < 0.0 { "-" } else { "" };

    if abs >= 1_000_000_000.0 {
      format!("{sign}{:.2}B", abs / 1_000_000_000.0)
    } else if abs >= 1_000_000.0 {
      format!("{sign}{:.1}M", abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
      format!("{sign}{:.1}K", abs / 1_000.0)
    } else {
      format!("{sign}{:.0}", abs)
    }
  }

  /// Sets the neural attribute allocation (transient; not persisted to DB).
  pub fn set_attributes(&mut self, attrs: NeuralAttributes) -> &mut Self {
    self.attributes = Some(attrs);
    self
  }

  /// Sets the access token, marking the model dirty if already persisted.
  pub fn set_access_token(&mut self, access_token: impl Into<String>) -> &mut Self {
    self.access_token = access_token.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the corporation ID, marking the model dirty if already persisted.
  pub fn set_corp_id(&mut self, corp_id: i64) -> &mut Self {
    self.corp_id = corp_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the corporation name, marking the model dirty if already persisted.
  pub fn set_corp_name(&mut self, corp_name: impl Into<String>) -> &mut Self {
    self.corp_name = corp_name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the granted ESI scopes, marking the model dirty if already persisted.
  pub fn set_granted_scopes(&mut self, granted_scopes: Option<String>) -> &mut Self {
    self.granted_scopes = granted_scopes;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the ISK balance, marking the model dirty if already persisted.
  pub fn set_isk_balance(&mut self, isk_balance: Option<f64>) -> &mut Self {
    self.isk_balance = isk_balance;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the docked status, marking the model dirty if already persisted.
  pub fn set_location_docked(&mut self, location_docked: Option<bool>) -> &mut Self {
    self.location_docked = location_docked;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the location name, marking the model dirty if already persisted.
  pub fn set_location_name(&mut self, location_name: Option<String>) -> &mut Self {
    self.location_name = location_name;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the character name, marking the model dirty if already persisted.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the portrait hue, marking the model dirty if already persisted.
  pub fn set_portrait_tone(&mut self, portrait_tone: i32) -> &mut Self {
    self.portrait_tone = portrait_tone;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the refresh token, marking the model dirty if already persisted.
  pub fn set_refresh_token(&mut self, refresh_token: impl Into<String>) -> &mut Self {
    self.refresh_token = refresh_token.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the display sort order, marking the model dirty if already persisted.
  pub fn set_sort_order(&mut self, sort_order: i32) -> &mut Self {
    self.sort_order = sort_order;
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

  /// Marks this model as loaded from the database without affecting the dirty flag.
  pub fn mark_persisted(&mut self) -> &mut Self {
    self.persisted = true;
    self
  }

  /// Returns the training completion percentage (0.0–1.0) for the active skill.
  pub fn training_percent(&self) -> Option<f64> {
    let skill = self.active_training()?;
    let end_time = skill.training_end_time?;
    let start_time = skill.training_start_time?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;

    if let (Some(level_start), Some(level_end), Some(run_start_sp)) = (
      skill.training_level_start_sp,
      skill.training_level_end_sp,
      skill.training_start_sp,
    ) {
      return sp_based_percent(level_start, level_end, run_start_sp, start_time, end_time, now);
    }

    let total_duration = (end_time - start_time) as f64;
    if total_duration <= 0.0 {
      return Some(1.0);
    }
    Some(((now - start_time) as f64 / total_duration).clamp(0.0, 1.0))
  }
}

fn sp_based_percent(
  level_start: i64,
  level_end: i64,
  run_start_sp: i64,
  start_time: i64,
  end_time: i64,
  now: i64,
) -> Option<f64> {
  let level_range = (level_end - level_start) as f64;
  if level_range <= 0.0 {
    return Some(1.0);
  }
  let run_duration = (end_time - start_time) as f64;
  if run_duration <= 0.0 {
    return Some(1.0);
  }
  let sp_rate = (level_end - run_start_sp) as f64 / run_duration;
  let current_sp = run_start_sp as f64 + (now - start_time).max(0) as f64 * sp_rate;
  Some(((current_sp - level_start as f64) / level_range).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_character(isk: Option<f64>) -> Model {
    let mut c = Model::new(12345678_i64, "Test Character");
    c.set_corp_id(1000001_i64)
      .set_corp_name("Test Corp")
      .set_portrait_tone(180)
      .set_access_token("token")
      .set_refresh_token("refresh")
      .set_token_expires_at(0)
      .set_isk_balance(isk);
    c
  }

  fn make_active_skill(start_time: i64, end_time: i64) -> CharacterSkill {
    CharacterSkill {
      active_level: 4,
      character_id: 1,
      is_active_training: true,
      skill_id: 3300,
      skill_name: Some("Caldari Frigate".into()),
      skillpoints: 135_765,
      trained_level: 4,
      training_end_time: Some(end_time),
      training_level_end_sp: None,
      training_level_start_sp: None,
      training_start_sp: None,
      training_start_time: Some(start_time),
    }
  }

  mod access_token_expired {
    use super::*;

    #[test]
    fn it_returns_false_for_far_future() {
      let mut c = make_character(None);
      c.set_token_expires_at((SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 3600) as i64);
      assert!(!c.access_token_expired());
    }

    #[test]
    fn it_returns_true_for_epoch_zero() {
      let c = make_character(None);
      assert!(c.access_token_expired());
    }
  }

  mod granted_scopes_list {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_empty_for_none() {
      let c = make_character(None);

      assert_eq!(c.granted_scopes_list(), Vec::<&str>::new());
    }

    #[test]
    fn it_returns_empty_for_empty_string() {
      let mut c = make_character(None);
      c.set_granted_scopes(Some(String::new()));

      assert_eq!(c.granted_scopes_list(), Vec::<&str>::new());
    }

    #[test]
    fn it_splits_space_separated_scopes() {
      let mut c = make_character(None);
      c.set_granted_scopes(Some("esi-mail.read esi-skills.read".into()));

      assert_eq!(c.granted_scopes_list(), vec!["esi-mail.read", "esi-skills.read"]);
    }
  }

  mod has_changes {
    use super::*;

    #[test]
    fn it_returns_false_before_persist() {
      let mut c = make_character(None);
      c.set_name("New Name");
      assert!(!c.has_changes());
    }

    #[test]
    fn it_returns_true_after_persist_and_mutation() {
      let mut c = make_character(None);
      c.mark_persisted();
      c.set_name("New Name");
      assert!(c.has_changes());
    }
  }

  mod isk_formatted {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_billions() {
      let c = make_character(Some(4_820_000_000.0));
      assert_eq!(c.isk_formatted(), "4.82B");
    }

    #[test]
    fn it_formats_millions() {
      let c = make_character(Some(312_000_000.0));
      assert_eq!(c.isk_formatted(), "312.0M");
    }

    #[test]
    fn it_formats_negative() {
      let c = make_character(Some(-500_000.0));
      assert_eq!(c.isk_formatted(), "-500.0K");
    }

    #[test]
    fn it_formats_small_amount() {
      let c = make_character(Some(42.0));
      assert_eq!(c.isk_formatted(), "42");
    }

    #[test]
    fn it_formats_thousands() {
      let c = make_character(Some(78_400.0));
      assert_eq!(c.isk_formatted(), "78.4K");
    }

    #[test]
    fn it_returns_dash_when_no_balance() {
      let c = make_character(None);
      assert_eq!(c.isk_formatted(), "\u{2014}");
    }
  }

  mod set_corp_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_character(None);
      c.mark_persisted();
      c.set_corp_id(1_000_002);
      assert!(c.has_changes());
    }
  }

  mod set_corp_name {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_character(None);
      c.mark_persisted();
      c.set_corp_name("New Corp");
      assert!(c.has_changes());
    }
  }

  mod set_granted_scopes {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_character(None);
      c.mark_persisted();
      c.set_granted_scopes(Some("esi-mail.read".into()));
      assert!(c.has_changes());
    }
  }

  mod set_isk_balance {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_character(None);
      c.mark_persisted();
      c.set_isk_balance(Some(1_000_000.0));
      assert!(c.has_changes());
    }
  }

  mod set_location_docked {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_character(None);
      c.mark_persisted();
      c.set_location_docked(Some(true));
      assert!(c.has_changes());
    }
  }

  mod set_location_name {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_character(None);
      c.mark_persisted();
      c.set_location_name(Some("Jita IV - Moon 4".into()));
      assert!(c.has_changes());
    }
  }

  mod set_portrait_tone {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_character(None);
      c.mark_persisted();
      c.set_portrait_tone(200);
      assert!(c.has_changes());
    }
  }

  mod set_sort_order {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut c = make_character(None);
      c.mark_persisted();
      c.set_sort_order(5);
      assert!(c.has_changes());
    }
  }

  mod training_percent {
    use super::*;

    #[test]
    fn it_returns_none_when_no_active_training() {
      let c = make_character(None);
      assert!(c.training_percent().is_none());
    }

    #[test]
    fn it_returns_none_when_skill_has_no_times() {
      let mut c = make_character(None);
      let skill = CharacterSkill {
        active_level: 4,
        character_id: 1,
        is_active_training: true,
        skill_id: 3300,
        skill_name: None,
        skillpoints: 0,
        trained_level: 4,
        training_end_time: None,
        training_level_end_sp: None,
        training_level_start_sp: None,
        training_start_sp: None,
        training_start_time: None,
      };
      *c.skills_mut() = vec![skill];

      assert!(c.training_percent().is_none());
    }

    #[test]
    fn it_returns_one_when_zero_duration() {
      let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
      let mut c = make_character(None);
      let mut skill = make_active_skill(now, now);
      skill.training_level_start_sp = Some(0);
      skill.training_level_end_sp = Some(0);
      skill.training_start_sp = Some(0);
      *c.skills_mut() = vec![skill];

      assert_eq!(c.training_percent(), Some(1.0));
    }

    #[test]
    fn it_clamps_to_one_when_past_end() {
      let past = 1_000_000_i64;
      let mut c = make_character(None);
      let skill = make_active_skill(past, past + 10);
      *c.skills_mut() = vec![skill];

      let pct = c.training_percent().unwrap();
      assert_eq!(pct, 1.0);
    }

    #[test]
    fn it_returns_zero_when_not_yet_started() {
      let future = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64 + 10_000;
      let mut c = make_character(None);
      let skill = make_active_skill(future, future + 10_000);
      *c.skills_mut() = vec![skill];

      let pct = c.training_percent().unwrap();
      assert_eq!(pct, 0.0);
    }

    #[test]
    fn it_returns_one_for_sp_path_with_zero_level_range() {
      let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
      let mut c = make_character(None);
      let mut skill = make_active_skill(now - 100, now + 100);
      skill.training_level_start_sp = Some(1000);
      skill.training_level_end_sp = Some(1000);
      skill.training_start_sp = Some(1000);
      *c.skills_mut() = vec![skill];

      assert_eq!(c.training_percent(), Some(1.0));
    }

    #[test]
    fn it_returns_one_for_sp_path_with_zero_run_duration() {
      let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
      let mut c = make_character(None);
      let mut skill = make_active_skill(now, now);
      skill.training_level_start_sp = Some(0);
      skill.training_level_end_sp = Some(1000);
      skill.training_start_sp = Some(0);
      *c.skills_mut() = vec![skill];

      assert_eq!(c.training_percent(), Some(1.0));
    }
  }
}
