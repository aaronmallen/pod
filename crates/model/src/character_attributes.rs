//! Neural attribute allocation for a character.

/// Neural attribute allocation fetched from ESI; not persisted to the database.
#[derive(Clone, Debug, Default)]
pub struct Model {
  pub charisma: i32,
  pub intelligence: i32,
  pub memory: i32,
  pub perception: i32,
  pub willpower: i32,
  pub bonus_remaps: i32,
  pub last_remap_date: Option<String>,
  pub accrued_remap_cooldown_date: Option<String>,
}
