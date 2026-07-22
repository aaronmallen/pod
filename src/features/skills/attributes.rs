use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::{
  format::{sp_cost, sp_per_sec},
  optimizer::{Attribute, Attributes, PairWeight, Recommendation, optimize_remap},
  plan_math::step_sp,
};
use crate::store::model::{CharacterAttributes, CharacterSkillqueue};

const ATTR_ORDER: [Attribute; 5] = [
  Attribute::Perception,
  Attribute::Willpower,
  Attribute::Intelligence,
  Attribute::Memory,
  Attribute::Charisma,
];
const BASE_ATTR_MAX: u32 = 27;
const BASE_ATTR_MIN: u32 = 17;
const BASE_ATTR_TOTAL: u32 = 99;
const BOOSTER_STEP: u32 = 5;
const MAX_ATTR: u32 = 44;
const PAIR_ORDER: [(Attribute, Attribute); 6] = [
  (Attribute::Perception, Attribute::Willpower),
  (Attribute::Intelligence, Attribute::Memory),
  (Attribute::Memory, Attribute::Perception),
  (Attribute::Intelligence, Attribute::Perception),
  (Attribute::Willpower, Attribute::Charisma),
  (Attribute::Charisma, Attribute::Willpower),
];
const SECONDS_PER_HOUR: f64 = 3_600.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttrRow {
  pub attribute: Attribute,
  pub base: u32,
  pub booster: u32,
  pub effective: u32,
  pub fill: f64,
  pub implant: u32,
  pub role: Role,
}

#[derive(Clone, Debug)]
pub struct AttrTabModel {
  pub accrued_remap_cooldown_date: Option<String>,
  pub active: Option<(Attribute, Attribute)>,
  pub base: Attributes,
  pub bonus_remaps: i64,
  pub booster_n: u32,
  pub consistent: bool,
  pub current_total_sec: f64,
  pub implants: Attributes,
  pub last_remap_date: Option<String>,
  pub recommendation: Recommendation,
}

impl AttrTabModel {
  pub fn new(
    attributes: &CharacterAttributes,
    implants: Attributes,
    active: Option<(Attribute, Attribute)>,
    weights: &[PairWeight],
  ) -> Self {
    let stored = Attributes {
      charisma: attributes.charisma().max(0) as u32,
      intelligence: attributes.intelligence().max(0) as u32,
      memory: attributes.memory().max(0) as u32,
      perception: attributes.perception().max(0) as u32,
      willpower: attributes.willpower().max(0) as u32,
    };
    let derived = derive_attributes(attributes, implants);

    // Derivation failed to explain the stored totals (e.g. mid-remap or a stale snapshot); fall back
    // to the raw totals as base with no implants/booster split rather than guess at one.
    let (base, view_implants, booster_n) = if derived.consistent {
      (derived.base, implants, derived.booster_n)
    } else {
      (stored, Attributes::default(), 0)
    };

    // The optimizer excludes the booster (it expires) so remap suggestions stay valid long-term; the
    // live rate below includes it so the displayed pace matches the pilot's current training speed.
    let recommendation = optimize_remap(weights, base, view_implants);
    let current_total_sec = current_plan_time(weights, effective(base, view_implants, booster_n));

    AttrTabModel {
      accrued_remap_cooldown_date: attributes.accrued_remap_cooldown_date().clone(),
      active,
      base,
      bonus_remaps: attributes.bonus_remaps(),
      booster_n,
      consistent: derived.consistent,
      current_total_sec,
      implants: view_implants,
      last_remap_date: attributes.last_remap_date().clone(),
      recommendation,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DerivedAttributes {
  pub base: Attributes,
  pub booster_n: u32,
  pub consistent: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairRate {
  pub active: bool,
  pub primary: Attribute,
  pub secondary: Attribute,
  pub sp_per_hr: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RemapDays {
  pub cooldown_days: Option<i64>,
  pub last_remap_days: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
  None,
  Primary,
  Secondary,
}

#[derive(Clone, Copy, Debug)]
pub struct WeightSkill {
  pub primary: Attribute,
  pub rank: f64,
  pub secondary: Attribute,
  pub skillpoints_in_skill: u64,
}

/// Reverses the ESI-reported totals (base + implants + booster merged into one number) back into a
/// base and a uniform booster bonus. Base points always sum to `BASE_ATTR_TOTAL` with each stat in
/// `BASE_ATTR_MIN..=BASE_ATTR_MAX`, and an active booster adds the same amount to every stat, so
/// `consistent` is false whenever the totals can't be explained by that shape.
pub fn derive_attributes(stored: &CharacterAttributes, implants: Attributes) -> DerivedAttributes {
  let stored = Attributes {
    charisma: stored.charisma().max(0) as u32,
    intelligence: stored.intelligence().max(0) as u32,
    memory: stored.memory().max(0) as u32,
    perception: stored.perception().max(0) as u32,
    willpower: stored.willpower().max(0) as u32,
  };

  let surplus = i64::from(sum_of(stored)) - i64::from(sum_of(implants)) - i64::from(BASE_ATTR_TOTAL);
  let divisible = surplus >= 0 && surplus % i64::from(BOOSTER_STEP) == 0;
  let booster_n = if divisible {
    (surplus / i64::from(BOOSTER_STEP)) as u32
  } else {
    0
  };

  let base = Attributes {
    charisma: stored
      .charisma
      .saturating_sub(implants.charisma)
      .saturating_sub(booster_n),
    intelligence: stored
      .intelligence
      .saturating_sub(implants.intelligence)
      .saturating_sub(booster_n),
    memory: stored.memory.saturating_sub(implants.memory).saturating_sub(booster_n),
    perception: stored
      .perception
      .saturating_sub(implants.perception)
      .saturating_sub(booster_n),
    willpower: stored
      .willpower
      .saturating_sub(implants.willpower)
      .saturating_sub(booster_n),
  };

  let in_range = [
    base.charisma,
    base.intelligence,
    base.memory,
    base.perception,
    base.willpower,
  ]
  .iter()
  .all(|value| (BASE_ATTR_MIN..=BASE_ATTR_MAX).contains(value));

  DerivedAttributes {
    base,
    booster_n,
    consistent: divisible && in_range && sum_of(base) == BASE_ATTR_TOTAL,
  }
}

pub fn project_rows(
  base: Attributes,
  implants: Attributes,
  booster_n: u32,
  active: Option<(Attribute, Attribute)>,
) -> [AttrRow; 5] {
  ATTR_ORDER.map(|attribute| {
    let base = value_of(base, attribute);
    let implant = value_of(implants, attribute);
    let effective = base + implant + booster_n;
    let fill = (f64::from(effective) / f64::from(MAX_ATTR)).clamp(0.0, 1.0);

    AttrRow {
      attribute,
      base,
      booster: booster_n,
      effective,
      fill,
      implant,
      role: role_of(attribute, active),
    }
  })
}

pub fn remap_days(
  now: DateTime<Utc>,
  last_remap_date: Option<&str>,
  accrued_remap_cooldown_date: Option<&str>,
) -> RemapDays {
  let cooldown_days = accrued_remap_cooldown_date
    .and_then(parse_timestamp)
    .map(|date| (date - now).num_days().max(0));
  let last_remap_days = last_remap_date
    .and_then(parse_timestamp)
    .map(|date| (now - date).num_days().max(0));

  RemapDays {
    cooldown_days,
    last_remap_days,
  }
}

pub fn sp_per_hr_matrix(effective: Attributes, active: Option<(Attribute, Attribute)>) -> [PairRate; 6] {
  PAIR_ORDER.map(|(primary, secondary)| {
    let rate = sp_per_sec(value_of(effective, primary), value_of(effective, secondary));

    PairRate {
      active: active == Some((primary, secondary)),
      primary,
      secondary,
      sp_per_hr: (rate * SECONDS_PER_HOUR).round() as u64,
    }
  })
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
  DateTime::parse_from_rfc3339(value)
    .ok()
    .map(|date| date.with_timezone(&Utc))
}

fn role_of(attribute: Attribute, active: Option<(Attribute, Attribute)>) -> Role {
  match active {
    Some((primary, _)) if primary == attribute => Role::Primary,
    Some((_, secondary)) if secondary == attribute => Role::Secondary,
    _ => Role::None,
  }
}

fn sum_of(attributes: Attributes) -> u32 {
  attributes.charisma + attributes.intelligence + attributes.memory + attributes.perception + attributes.willpower
}

fn value_of(attributes: Attributes, attribute: Attribute) -> u32 {
  match attribute {
    Attribute::Charisma => attributes.charisma,
    Attribute::Intelligence => attributes.intelligence,
    Attribute::Memory => attributes.memory,
    Attribute::Perception => attributes.perception,
    Attribute::Willpower => attributes.willpower,
  }
}

pub fn attribute_from_neural_id(id: i64) -> Attribute {
  match id {
    164 => Attribute::Charisma,
    165 => Attribute::Intelligence,
    166 => Attribute::Memory,
    168 => Attribute::Willpower,
    _ => Attribute::Perception,
  }
}

fn effective(base: Attributes, implants: Attributes, booster_n: u32) -> Attributes {
  Attributes {
    charisma: base.charisma + implants.charisma + booster_n,
    intelligence: base.intelligence + implants.intelligence + booster_n,
    memory: base.memory + implants.memory + booster_n,
    perception: base.perception + implants.perception + booster_n,
    willpower: base.willpower + implants.willpower + booster_n,
  }
}

fn current_plan_time(weights: &[PairWeight], effective: Attributes) -> f64 {
  let mut total = 0.0;
  for weight in weights {
    let rate = sp_per_sec(
      value_of(effective, weight.primary),
      value_of(effective, weight.secondary),
    );
    if rate <= 0.0 {
      return f64::INFINITY;
    }
    total += weight.sp as f64 / rate;
  }
  total
}

pub fn queue_pair_weights(queue: &[CharacterSkillqueue], meta: &HashMap<i64, WeightSkill>) -> Vec<PairWeight> {
  let mut by_pair: HashMap<(usize, usize), u64> = HashMap::new();
  let mut order: Vec<(Attribute, Attribute)> = Vec::new();

  for (index, entry) in queue.iter().enumerate() {
    let Some(skill) = meta.get(&entry.skill_id()) else {
      continue;
    };
    let to_level = entry.finished_level().clamp(0, 5) as u8;
    let from_level = entry.finished_level().saturating_sub(1).clamp(0, 5) as u8;

    let partial = if index == 0 {
      skill.skillpoints_in_skill
    } else {
      sp_cost(skill.rank, from_level)
    };
    let sp = step_sp(skill.rank, from_level, to_level, partial) as u64;
    if sp == 0 {
      continue;
    }

    let key = (skill.primary as usize, skill.secondary as usize);
    if !by_pair.contains_key(&key) {
      order.push((skill.primary, skill.secondary));
    }
    *by_pair.entry(key).or_insert(0) += sp;
  }

  order
    .into_iter()
    .map(|(primary, secondary)| PairWeight {
      primary,
      secondary,
      sp: by_pair[&(primary as usize, secondary as usize)],
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone as _;

  use super::*;

  fn base() -> Attributes {
    Attributes {
      charisma: 17,
      intelligence: 21,
      memory: 20,
      perception: 22,
      willpower: 19,
    }
  }

  fn implants() -> Attributes {
    Attributes {
      charisma: 3,
      intelligence: 4,
      memory: 4,
      perception: 5,
      willpower: 5,
    }
  }

  impl Attributes {
    fn plus_for_test(self, implants: Attributes) -> Attributes {
      Attributes {
        charisma: self.charisma + implants.charisma,
        intelligence: self.intelligence + implants.intelligence,
        memory: self.memory + implants.memory,
        perception: self.perception + implants.perception,
        willpower: self.willpower + implants.willpower,
      }
    }
  }

  mod derive_attributes {
    use pretty_assertions::assert_eq;

    use super::*;

    fn stored(charisma: i64, intelligence: i64, memory: i64, perception: i64, willpower: i64) -> CharacterAttributes {
      CharacterAttributes {
        accrued_remap_cooldown_date: None,
        bonus_remaps: 0,
        character_id: 42,
        charisma,
        intelligence,
        last_remap_date: None,
        memory,
        perception,
        unallocated_sp: 0,
        willpower,
      }
    }

    fn attrs(charisma: u32, intelligence: u32, memory: u32, perception: u32, willpower: u32) -> Attributes {
      Attributes {
        charisma,
        intelligence,
        memory,
        perception,
        willpower,
      }
    }

    #[test]
    fn it_returns_the_stored_row_as_base_for_a_no_implant_pilot() {
      let row = stored(17, 21, 20, 22, 19);

      let derived = derive_attributes(&row, Attributes::default());

      assert_eq!(derived.base, attrs(17, 21, 20, 22, 19));
      assert_eq!(derived.booster_n, 0);
      assert!(derived.consistent);
    }

    #[test]
    fn it_subtracts_uniform_implants_to_recover_the_base_with_no_booster() {
      let row = stored(24, 24, 25, 27, 24);

      let derived = derive_attributes(&row, attrs(5, 5, 5, 5, 5));

      assert_eq!(derived.base, attrs(19, 19, 20, 22, 19));
      assert_eq!(derived.booster_n, 0);
      assert_eq!(sum_of(derived.base), 99);
      assert!(derived.consistent);
    }

    #[test]
    fn it_subtracts_non_uniform_implants_per_attribute_to_recover_the_base() {
      let row = stored(19, 25, 23, 27, 23);

      let derived = derive_attributes(&row, attrs(2, 4, 3, 5, 4));

      assert_eq!(derived.base, attrs(17, 21, 20, 22, 19));
      assert_eq!(derived.booster_n, 0);
      assert_eq!(sum_of(derived.base), 99);
      assert!(derived.base.charisma >= 17 && derived.base.perception <= 27);
      assert!(derived.consistent);
    }

    #[test]
    fn it_peels_a_uniform_booster_off_a_boosted_pilot() {
      let row = stored(37, 37, 37, 36, 37);

      let derived = derive_attributes(&row, attrs(5, 5, 5, 5, 5));

      assert_eq!(derived.base, attrs(20, 20, 20, 19, 20));
      assert_eq!(derived.booster_n, 12);
      assert_eq!(sum_of(derived.base), 99);
      assert!(derived.consistent);
    }

    #[test]
    fn it_peels_a_booster_and_non_uniform_implants_together() {
      let row = stored(22, 28, 26, 30, 26);

      let derived = derive_attributes(&row, attrs(2, 4, 3, 5, 4));

      assert_eq!(derived.base, attrs(17, 21, 20, 22, 19));
      assert_eq!(derived.booster_n, 3);
      assert_eq!(sum_of(derived.base), 99);
      assert!(derived.consistent);
    }

    #[test]
    fn it_flags_inconsistent_when_the_surplus_is_not_divisible_by_the_booster_step() {
      let row = stored(20, 21, 20, 21, 21);

      let derived = derive_attributes(&row, Attributes::default());

      assert!(!derived.consistent);
      assert_eq!(derived.booster_n, 0);
      assert_eq!(derived.base, attrs(20, 21, 20, 21, 21));
    }

    #[test]
    fn it_flags_inconsistent_when_the_stored_total_falls_below_the_base_and_implants() {
      let row = stored(17, 17, 17, 17, 17);

      let derived = derive_attributes(&row, attrs(5, 5, 5, 5, 5));

      assert!(!derived.consistent);
      assert_eq!(derived.booster_n, 0);
    }

    #[test]
    fn it_flags_inconsistent_when_a_derived_base_lands_outside_the_legal_range() {
      let row = stored(17, 17, 17, 20, 28);

      let derived = derive_attributes(&row, Attributes::default());

      assert_eq!(sum_of(derived.base), 99);
      assert!(!derived.consistent);
    }
  }

  mod attr_tab_model {
    use pretty_assertions::assert_eq;

    use super::*;

    fn row(charisma: i64, intelligence: i64, memory: i64, perception: i64, willpower: i64) -> CharacterAttributes {
      CharacterAttributes {
        accrued_remap_cooldown_date: None,
        bonus_remaps: 0,
        character_id: 42,
        charisma,
        intelligence,
        last_remap_date: None,
        memory,
        perception,
        unallocated_sp: 0,
        willpower,
      }
    }

    fn uniform_implants() -> Attributes {
      Attributes {
        charisma: 5,
        intelligence: 5,
        memory: 5,
        perception: 5,
        willpower: 5,
      }
    }

    fn weights() -> Vec<PairWeight> {
      vec![PairWeight {
        primary: Attribute::Perception,
        secondary: Attribute::Willpower,
        sp: 1_000_000,
      }]
    }

    #[test]
    fn it_rates_the_boosted_pilot_faster_than_the_boosterless_recommendation_basis() {
      let boosted = AttrTabModel::new(&row(36, 37, 37, 37, 37), uniform_implants(), None, &weights());
      let boosterless_rate = current_plan_time(&weights(), effective(boosted.base, boosted.implants, 0));

      assert_eq!(boosted.booster_n, 12);
      assert!(
        boosted.current_total_sec < boosterless_rate,
        "the live rate must include the booster and beat the boosterless basis"
      );
    }

    #[test]
    fn it_recovers_the_base_for_the_optimizer_so_an_implanted_pilot_is_not_out_of_spec() {
      let model = AttrTabModel::new(&row(22, 26, 25, 27, 24), uniform_implants(), None, &weights());

      assert!(model.consistent);
      assert!(!model.recommendation.current_out_of_spec);
      assert_eq!(sum_of(model.base), BASE_ATTR_TOTAL);
    }

    #[test]
    fn it_renders_raw_stored_values_when_the_snapshot_is_inconsistent() {
      let model = AttrTabModel::new(&row(17, 21, 20, 22, 19), uniform_implants(), None, &weights());

      assert!(!model.consistent);
      assert_eq!(model.booster_n, 0);
      assert_eq!(model.implants, Attributes::default());
      assert_eq!(
        model.base,
        Attributes {
          charisma: 17,
          intelligence: 21,
          memory: 20,
          perception: 22,
          willpower: 19,
        }
      );
    }
  }

  mod project_rows {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_the_bar_fill_when_effective_exceeds_the_ceiling_without_touching_effective() {
      let base = Attributes {
        perception: 35,
        ..base()
      };

      let rows = project_rows(base, implants(), 5, None);
      let perception = rows[0];

      assert_eq!(perception.effective, 45);
      assert_eq!(perception.booster, 5);
      assert_eq!(perception.fill, 1.0);
    }

    #[test]
    fn it_does_not_clamp_effective_when_a_plus_five_implant_overflows_the_ceiling() {
      let base = Attributes {
        charisma: 17,
        intelligence: 17,
        memory: 17,
        perception: 27,
        willpower: 21,
      };

      let rows = project_rows(base, implants(), 0, None);
      let perception = rows[0];

      assert_eq!(perception.base, 27);
      assert_eq!(perception.implant, 5);
      assert_eq!(perception.booster, 0);
      assert_eq!(perception.effective, 32);
      assert!((perception.fill - 32.0 / 44.0).abs() < 1e-9);
      assert!(perception.fill < 1.0);
    }

    #[test]
    fn it_keeps_the_base_total_at_ninety_nine() {
      let rows = project_rows(base(), implants(), 0, None);

      let base_total: u32 = rows.iter().map(|row| row.base).sum();
      assert_eq!(base_total, 99);
    }

    #[test]
    fn it_leaves_every_role_none_when_no_pair_is_active() {
      let rows = project_rows(base(), implants(), 0, None);

      assert!(rows.iter().all(|row| row.role == Role::None));
    }

    #[test]
    fn it_orders_rows_per_the_wireframe_attr_order() {
      let rows = project_rows(base(), implants(), 0, None);

      let order: Vec<Attribute> = rows.iter().map(|row| row.attribute).collect();
      assert_eq!(
        order,
        vec![
          Attribute::Perception,
          Attribute::Willpower,
          Attribute::Intelligence,
          Attribute::Memory,
          Attribute::Charisma,
        ]
      );
    }

    #[test]
    fn it_projects_base_implant_and_effective_separately() {
      let rows = project_rows(base(), implants(), 0, None);
      let perception = rows[0];

      assert_eq!(perception.base, 22);
      assert_eq!(perception.implant, 5);
      assert_eq!(perception.booster, 0);
      assert_eq!(perception.effective, 27);
    }

    #[test]
    fn it_adds_a_uniform_booster_segment_to_every_row() {
      let rows = project_rows(base(), implants(), 12, None);

      assert!(rows.iter().all(|row| row.booster == 12));
      let perception = rows[0];
      assert_eq!(perception.effective, perception.base + perception.implant + 12);
    }

    #[test]
    fn it_tags_the_active_pair_with_primary_and_secondary_roles() {
      let active = Some((Attribute::Perception, Attribute::Willpower));

      let rows = project_rows(base(), implants(), 0, active);

      assert_eq!(rows[0].role, Role::Primary);
      assert_eq!(rows[1].role, Role::Secondary);
      assert_eq!(rows[2].role, Role::None);
    }
  }

  mod queue_pair_weights {
    use pretty_assertions::assert_eq;

    use super::*;

    fn entry(skill_id: i64, finished_level: i64, queue_position: i64) -> CharacterSkillqueue {
      CharacterSkillqueue {
        character_id: 42,
        finish_date: None,
        finished_level,
        level_end_sp: None,
        level_start_sp: None,
        queue_position,
        skill_id,
        start_date: None,
        training_start_sp: None,
      }
    }

    fn skill(primary: Attribute, secondary: Attribute, skillpoints_in_skill: u64) -> WeightSkill {
      WeightSkill {
        primary,
        rank: 1.0,
        secondary,
        skillpoints_in_skill,
      }
    }

    #[test]
    fn it_discounts_the_head_skill_by_its_invested_skillpoints() {
      let queue = vec![entry(3300, 5, 0)];
      let meta = HashMap::from([(3300, skill(Attribute::Perception, Attribute::Willpower, 100_000))]);

      let weights = queue_pair_weights(&queue, &meta);

      assert_eq!(weights.len(), 1);
      assert_eq!(weights[0].primary, Attribute::Perception);
      assert_eq!(weights[0].secondary, Attribute::Willpower);
      assert_eq!(weights[0].sp, 156_000);
    }

    #[test]
    fn it_drops_a_zero_demand_head_skill() {
      let queue = vec![entry(3300, 5, 0)];
      let meta = HashMap::from([(3300, skill(Attribute::Perception, Attribute::Willpower, 999_999))]);

      let weights = queue_pair_weights(&queue, &meta);

      assert!(weights.is_empty());
    }

    #[test]
    fn it_skips_entries_with_no_metadata() {
      let queue = vec![entry(3300, 5, 0), entry(9999, 5, 1)];
      let meta = HashMap::from([(3300, skill(Attribute::Memory, Attribute::Perception, 0))]);

      let weights = queue_pair_weights(&queue, &meta);

      assert_eq!(weights.len(), 1, "the metadata-less entry contributes no pair");
      assert_eq!(weights[0].primary, Attribute::Memory);
    }

    #[test]
    fn it_sums_demand_across_entries_sharing_a_pair() {
      let queue = vec![entry(3300, 5, 0), entry(3301, 5, 1), entry(3302, 5, 2)];
      let meta = HashMap::from([
        (3300, skill(Attribute::Intelligence, Attribute::Memory, 0)),
        (3301, skill(Attribute::Perception, Attribute::Willpower, 0)),
        (3302, skill(Attribute::Perception, Attribute::Willpower, 0)),
      ]);

      let weights = queue_pair_weights(&queue, &meta);

      let per_wil = weights
        .iter()
        .find(|w| w.primary == Attribute::Perception && w.secondary == Attribute::Willpower)
        .expect("per/wil pair present");
      assert_eq!(per_wil.sp, 2 * (256_000 - 45_255));
    }

    #[test]
    fn it_uses_the_full_level_delta_for_non_head_steps() {
      let queue = vec![entry(3300, 4, 0), entry(3301, 5, 1)];
      let meta = HashMap::from([
        (3300, skill(Attribute::Intelligence, Attribute::Memory, 0)),
        (3301, skill(Attribute::Perception, Attribute::Willpower, 999_999)),
      ]);

      let weights = queue_pair_weights(&queue, &meta);

      let per_wil = weights
        .iter()
        .find(|w| w.primary == Attribute::Perception && w.secondary == Attribute::Willpower)
        .expect("per/wil pair present");
      assert_eq!(per_wil.sp, 256_000 - 45_255);
    }

    #[test]
    fn it_omits_an_already_trained_step_so_it_contributes_zero_to_attribute_math() {
      let trained_only = vec![entry(3300, 5, 0)];
      let with_needed = vec![entry(3300, 5, 0), entry(3301, 5, 1)];
      let meta = HashMap::from([
        (3300, skill(Attribute::Intelligence, Attribute::Memory, 999_999)),
        (3301, skill(Attribute::Perception, Attribute::Willpower, 0)),
      ]);

      let trained_weights = queue_pair_weights(&trained_only, &meta);
      let needed_weights = queue_pair_weights(&with_needed, &meta);

      assert!(trained_weights.is_empty());
      assert_eq!(
        optimize_remap(&trained_weights, base(), implants()),
        optimize_remap(&needed_weights[..0], base(), implants()),
        "an already-trained step leaves the optimizer with the same empty weight set"
      );
      assert_eq!(needed_weights.len(), 1);
      assert_eq!(needed_weights[0].primary, Attribute::Perception);
    }
  }

  mod remap_days {
    use pretty_assertions::assert_eq;

    use super::*;

    fn now() -> DateTime<Utc> {
      Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap()
    }

    #[test]
    fn it_computes_whole_days_since_the_last_remap() {
      let days = remap_days(now(), Some("2026-04-01T12:00:00Z"), None);

      assert_eq!(days.last_remap_days, Some(61));
    }

    #[test]
    fn it_computes_whole_days_until_the_cooldown_expires() {
      let days = remap_days(now(), None, Some("2026-09-01T12:00:00Z"));

      assert_eq!(days.cooldown_days, Some(92));
    }

    #[test]
    fn it_floors_a_future_last_remap_at_zero() {
      let days = remap_days(now(), Some("2026-07-01T12:00:00Z"), None);

      assert_eq!(days.last_remap_days, Some(0));
    }

    #[test]
    fn it_floors_a_past_cooldown_at_zero_meaning_available() {
      let days = remap_days(now(), None, Some("2026-01-01T12:00:00Z"));

      assert_eq!(days.cooldown_days, Some(0));
    }

    #[test]
    fn it_handles_none_dates_without_panicking() {
      let days = remap_days(now(), None, None);

      assert_eq!(days, RemapDays::default());
      assert_eq!(days.last_remap_days, None);
      assert_eq!(days.cooldown_days, None);
    }

    #[test]
    fn it_yields_none_for_an_unparsable_date() {
      let days = remap_days(now(), Some("not-a-date"), Some(""));

      assert_eq!(days.last_remap_days, None);
      assert_eq!(days.cooldown_days, None);
    }
  }

  mod sp_per_hr_matrix {
    use pretty_assertions::assert_eq;

    use super::*;

    fn effective() -> Attributes {
      base().plus_for_test(implants())
    }

    #[test]
    fn it_covers_the_six_wireframe_pairs_in_order() {
      let matrix = sp_per_hr_matrix(effective(), None);

      let pairs: Vec<(Attribute, Attribute)> = matrix.iter().map(|cell| (cell.primary, cell.secondary)).collect();
      assert_eq!(
        pairs,
        vec![
          (Attribute::Perception, Attribute::Willpower),
          (Attribute::Intelligence, Attribute::Memory),
          (Attribute::Memory, Attribute::Perception),
          (Attribute::Intelligence, Attribute::Perception),
          (Attribute::Willpower, Attribute::Charisma),
          (Attribute::Charisma, Attribute::Willpower),
        ]
      );
    }

    #[test]
    fn it_marks_no_pair_active_when_none_is_supplied() {
      let matrix = sp_per_hr_matrix(effective(), None);

      assert!(matrix.iter().all(|cell| !cell.active));
    }

    #[test]
    fn it_marks_only_the_matching_active_pair() {
      let active = Some((Attribute::Intelligence, Attribute::Memory));

      let matrix = sp_per_hr_matrix(effective(), active);

      let active_cells: Vec<_> = matrix.iter().filter(|cell| cell.active).collect();
      assert_eq!(active_cells.len(), 1);
      assert_eq!(active_cells[0].primary, Attribute::Intelligence);
      assert_eq!(active_cells[0].secondary, Attribute::Memory);
    }

    #[test]
    fn it_matches_round_sp_per_sec_times_thirty_six_hundred_for_each_pair() {
      let effective = effective();

      let matrix = sp_per_hr_matrix(effective, None);

      for cell in matrix {
        let primary = value_of(effective, cell.primary);
        let secondary = value_of(effective, cell.secondary);
        let expected = (sp_per_sec(primary, secondary) * 3_600.0).round() as u64;

        assert_eq!(cell.sp_per_hr, expected, "pair {:?}/{:?}", cell.primary, cell.secondary);
      }
    }

    #[test]
    fn it_uses_effective_attributes_so_perception_willpower_is_the_known_rate() {
      let matrix = sp_per_hr_matrix(effective(), None);

      assert_eq!(matrix[0].primary, Attribute::Perception);
      assert_eq!(matrix[0].secondary, Attribute::Willpower);
      assert_eq!(matrix[0].sp_per_hr, 2_340);
    }
  }
}
