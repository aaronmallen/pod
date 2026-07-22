use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::character::Attributes;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  pub accrued_remap_cooldown_date: Option<String>,
  #[getset(get_copy = "pub")]
  pub bonus_remaps: i64,
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub charisma: i64,
  #[getset(get_copy = "pub")]
  pub intelligence: i64,
  #[getset(get = "pub")]
  pub last_remap_date: Option<String>,
  #[getset(get_copy = "pub")]
  pub memory: i64,
  #[getset(get_copy = "pub")]
  pub perception: i64,
  #[getset(get_copy = "pub")]
  pub unallocated_sp: i64,
  #[getset(get_copy = "pub")]
  pub willpower: i64,
}

impl From<(i64, Attributes, i64)> for Model {
  fn from((character_id, attributes, unallocated_sp): (i64, Attributes, i64)) -> Self {
    Self {
      accrued_remap_cooldown_date: attributes.accrued_remap_cooldown_date,
      bonus_remaps: i64::from(attributes.bonus_remaps),
      character_id,
      charisma: i64::from(attributes.charisma),
      intelligence: i64::from(attributes.intelligence),
      last_remap_date: attributes.last_remap_date,
      memory: i64::from(attributes.memory),
      perception: i64::from(attributes.perception),
      unallocated_sp,
      willpower: i64::from(attributes.willpower),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_attributes() -> Attributes {
      Attributes {
        accrued_remap_cooldown_date: Some("2026-01-01T00:00:00Z".to_owned()),
        bonus_remaps: 2,
        charisma: 19,
        intelligence: 20,
        last_remap_date: Some("2025-06-01T00:00:00Z".to_owned()),
        memory: 21,
        perception: 22,
        willpower: 23,
      }
    }

    #[test]
    fn it_attaches_character_id_and_unallocated_sp_and_widens_attributes() {
      let model = Model::from((42, make_attributes(), 15_000));

      assert_eq!(model.character_id(), 42);
      assert_eq!(model.unallocated_sp(), 15_000);

      assert_eq!(model.charisma(), 19);
      assert_eq!(model.intelligence(), 20);
      assert_eq!(model.memory(), 21);
      assert_eq!(model.perception(), 22);
      assert_eq!(model.willpower(), 23);
      assert_eq!(model.bonus_remaps(), 2);

      assert_eq!(model.last_remap_date().as_deref(), Some("2025-06-01T00:00:00Z"));
      assert_eq!(
        model.accrued_remap_cooldown_date().as_deref(),
        Some("2026-01-01T00:00:00Z")
      );
    }

    #[test]
    fn it_carries_none_remap_dates_for_a_never_remapped_pilot() {
      let mut attributes = make_attributes();
      attributes.last_remap_date = None;
      attributes.accrued_remap_cooldown_date = None;

      let model = Model::from((7, attributes, 0));

      assert_eq!(model.last_remap_date(), &None);
      assert_eq!(model.accrued_remap_cooldown_date(), &None);
    }
  }
}
