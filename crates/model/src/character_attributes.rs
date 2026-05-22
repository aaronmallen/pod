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

#[cfg(test)]
mod tests {
  mod default {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_zeroes_all_attributes() {
      let attrs = Model::default();

      assert_eq!(attrs.charisma, 0);
      assert_eq!(attrs.intelligence, 0);
      assert_eq!(attrs.memory, 0);
      assert_eq!(attrs.perception, 0);
      assert_eq!(attrs.willpower, 0);
      assert_eq!(attrs.bonus_remaps, 0);
      assert!(attrs.last_remap_date.is_none());
      assert!(attrs.accrued_remap_cooldown_date.is_none());
    }
  }
}
