#![cfg_attr(not(test), expect(dead_code))]

use std::collections::HashMap;

// SDE category 66 "Structure Modifier" (confirmed against the seeded SDE). Engineering rigs (manufacturing +
// science) carry attr 2593 (attributeEngRigTimeBonus, TE), 2594 (attributeEngRigMatBonus, ME), 2595
// (attributeEngRigCostBonus, install fee/role). Reactor (reaction) rigs carry 2713 (RefRigTimeBonus, TE) and
// 2714 (RefRigMatBonus, ME) and carry no cost attribute. The fee bonus is a per-rig dogma attribute (2595),
// not a flat per-structure role constant.
pub const FEE_ATTRIBUTE_IDS: [i64; 1] = [2595];
pub const ME_ATTRIBUTE_IDS: [i64; 2] = [2594, 2714];
pub const TE_ATTRIBUTE_IDS: [i64; 2] = [2593, 2713];

const HIGH_SEC_MULTIPLIER: f64 = 1.0;
const LOW_SEC_MULTIPLIER: f64 = 1.9;
const NULL_SEC_MULTIPLIER: f64 = 2.1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Activity {
  Manufacturing,
  Reaction,
  Science,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DerivedRigBonuses {
  pub fee: f64,
  pub me: f64,
  pub te: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
  Fee,
  Me,
  Te,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RigBonus {
  pub fee: f64,
  pub me: f64,
  pub te: f64,
}

impl Activity {
  pub fn classify(name: &str) -> Activity {
    if name.contains("Reactor") {
      Activity::Reaction
    } else if name.contains("Research")
      || name.contains("Invention")
      || name.contains("Copy")
      || name.contains("Laboratory")
    {
      Activity::Science
    } else {
      Activity::Manufacturing
    }
  }
}

impl Kind {
  pub fn from_attribute_id(attribute_id: i64) -> Option<Kind> {
    if ME_ATTRIBUTE_IDS.contains(&attribute_id) {
      Some(Kind::Me)
    } else if TE_ATTRIBUTE_IDS.contains(&attribute_id) {
      Some(Kind::Te)
    } else if FEE_ATTRIBUTE_IDS.contains(&attribute_id) {
      Some(Kind::Fee)
    } else {
      None
    }
  }
}

impl RigBonus {
  pub fn apply(&mut self, attribute_id: i64, value: f64) {
    match Kind::from_attribute_id(attribute_id) {
      Some(Kind::Fee) => self.fee += value,
      Some(Kind::Me) => self.me += value,
      Some(Kind::Te) => self.te += value,
      None => {}
    }
  }
}

pub fn derive_rig_bonuses(
  rig_type_ids: &[i64],
  catalog: &HashMap<i64, RigBonus>,
  security_status: f64,
) -> DerivedRigBonuses {
  let multiplier = security_band_multiplier(security_status);
  let mut derived = DerivedRigBonuses::default();

  for type_id in rig_type_ids {
    if let Some(rig) = catalog.get(type_id) {
      derived.fee += rig.fee;
      derived.me += rig.me;
      derived.te += rig.te;
    }
  }

  derived.fee *= multiplier;
  derived.me *= multiplier;
  derived.te *= multiplier;

  derived
}

fn security_band_multiplier(security_status: f64) -> f64 {
  if security_status >= 0.5 {
    HIGH_SEC_MULTIPLIER
  } else if security_status > 0.0 {
    LOW_SEC_MULTIPLIER
  } else {
    NULL_SEC_MULTIPLIER
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn catalog() -> HashMap<i64, RigBonus> {
    HashMap::from([
      (
        100,
        RigBonus {
          fee: 0.0,
          me: -2.0,
          te: 0.0,
        },
      ),
      (
        101,
        RigBonus {
          fee: 0.0,
          me: 0.0,
          te: -20.0,
        },
      ),
      (
        102,
        RigBonus {
          fee: -10.0,
          me: 0.0,
          te: 0.0,
        },
      ),
    ])
  }

  fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-9
  }

  mod classify {
    use super::*;

    #[test]
    fn it_reads_reactor_rigs_as_reactions() {
      assert_eq!(
        Activity::classify("Standup M-Set Composite Reactor Material Efficiency II"),
        Activity::Reaction
      );
    }

    #[test]
    fn it_reads_research_and_invention_rigs_as_science() {
      assert_eq!(
        Activity::classify("Standup L-Set Invention Optimization II"),
        Activity::Science
      );
      assert_eq!(
        Activity::classify("Standup M-Set ME Research Cost Optimization I"),
        Activity::Science
      );
      assert_eq!(
        Activity::classify("Standup M-Set Laboratory Optimization I"),
        Activity::Science
      );
    }

    #[test]
    fn it_defaults_to_manufacturing() {
      assert_eq!(
        Activity::classify("Standup M-Set Equipment Manufacturing Material Efficiency I"),
        Activity::Manufacturing
      );
    }
  }

  mod derive_rig_bonuses {
    use super::*;

    #[test]
    fn it_returns_zero_for_an_empty_rig_list() {
      let derived = derive_rig_bonuses(&[], &catalog(), -1.0);

      assert_eq!(derived, DerivedRigBonuses::default());
    }

    #[test]
    fn it_ignores_rig_type_ids_absent_from_the_catalog() {
      let derived = derive_rig_bonuses(&[999], &catalog(), 0.9);

      assert_eq!(derived, DerivedRigBonuses::default());
    }

    #[test]
    fn it_applies_the_hi_sec_multiplier_at_the_half_boundary() {
      let derived = derive_rig_bonuses(&[100], &catalog(), 0.5);

      assert!(close(derived.me, -2.0));
    }

    #[test]
    fn it_applies_the_hi_sec_multiplier_above_the_half_boundary() {
      let derived = derive_rig_bonuses(&[100], &catalog(), 0.95);

      assert!(close(derived.me, -2.0));
    }

    #[test]
    fn it_applies_the_low_sec_multiplier_just_below_the_half_boundary() {
      let derived = derive_rig_bonuses(&[100], &catalog(), 0.49);

      assert!(close(derived.me, -2.0 * 1.9));
    }

    #[test]
    fn it_applies_the_low_sec_multiplier_just_above_zero() {
      let derived = derive_rig_bonuses(&[101], &catalog(), 0.000_1);

      assert!(close(derived.te, -20.0 * 1.9));
    }

    #[test]
    fn it_applies_the_null_sec_multiplier_at_the_zero_boundary() {
      let derived = derive_rig_bonuses(&[102], &catalog(), 0.0);

      assert!(close(derived.fee, -10.0 * 2.1));
    }

    #[test]
    fn it_applies_the_null_sec_multiplier_for_negative_security_wormholes() {
      let derived = derive_rig_bonuses(&[102], &catalog(), -1.0);

      assert!(close(derived.fee, -10.0 * 2.1));
    }

    #[test]
    fn it_sums_every_fitted_rig_before_scaling() {
      let derived = derive_rig_bonuses(&[100, 101, 102], &catalog(), 0.49);

      assert!(close(derived.me, -2.0 * 1.9));
      assert!(close(derived.te, -20.0 * 1.9));
      assert!(close(derived.fee, -10.0 * 1.9));
    }
  }

  mod from_attribute_id {
    use super::*;

    #[test]
    fn it_maps_the_engineering_and_reactor_attribute_ids() {
      assert_eq!(Kind::from_attribute_id(2594), Some(Kind::Me));
      assert_eq!(Kind::from_attribute_id(2714), Some(Kind::Me));
      assert_eq!(Kind::from_attribute_id(2593), Some(Kind::Te));
      assert_eq!(Kind::from_attribute_id(2713), Some(Kind::Te));
      assert_eq!(Kind::from_attribute_id(2595), Some(Kind::Fee));
    }

    #[test]
    fn it_returns_none_for_an_unrelated_attribute() {
      assert_eq!(Kind::from_attribute_id(182), None);
    }
  }

  mod apply {
    use super::*;

    #[test]
    fn it_accumulates_each_bonus_into_its_kind() {
      let mut rig = RigBonus::default();

      rig.apply(2594, -2.0);
      rig.apply(2593, -20.0);
      rig.apply(2595, -10.0);

      assert_eq!(
        rig,
        RigBonus {
          fee: -10.0,
          me: -2.0,
          te: -20.0
        }
      );
    }

    #[test]
    fn it_skips_attributes_that_are_not_rig_bonuses() {
      let mut rig = RigBonus::default();

      rig.apply(182, 26252.0);

      assert_eq!(rig, RigBonus::default());
    }
  }
}
