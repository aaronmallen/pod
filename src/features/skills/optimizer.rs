use super::format::sp_per_sec;

const ATTR_MAX: u32 = 27;
const ATTR_MIN: u32 = 17;
const BASE_TOTAL: u32 = 99;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Attribute {
  Charisma,
  Intelligence,
  Memory,
  Perception,
  Willpower,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Attributes {
  pub charisma: u32,
  pub intelligence: u32,
  pub memory: u32,
  pub perception: u32,
  pub willpower: u32,
}

impl Attributes {
  fn get(self, attribute: Attribute) -> u32 {
    match attribute {
      Attribute::Charisma => self.charisma,
      Attribute::Intelligence => self.intelligence,
      Attribute::Memory => self.memory,
      Attribute::Perception => self.perception,
      Attribute::Willpower => self.willpower,
    }
  }

  fn is_in_spec(self) -> bool {
    let attrs = [
      self.charisma,
      self.intelligence,
      self.memory,
      self.perception,
      self.willpower,
    ];
    attrs.iter().all(|&value| (ATTR_MIN..=ATTR_MAX).contains(&value)) && attrs.iter().sum::<u32>() == BASE_TOTAL
  }

  fn plus(self, implants: Attributes) -> Attributes {
    Attributes {
      charisma: self.charisma + implants.charisma,
      intelligence: self.intelligence + implants.intelligence,
      memory: self.memory + implants.memory,
      perception: self.perception + implants.perception,
      willpower: self.willpower + implants.willpower,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairWeight {
  pub primary: Attribute,
  pub secondary: Attribute,
  pub sp: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recommendation {
  pub base: Attributes,
  pub current_out_of_spec: bool,
  pub is_current: bool,
  pub total_sec: f64,
}

pub fn optimize_remap(weights: &[PairWeight], current_base: Attributes, implants: Attributes) -> Recommendation {
  let current_out_of_spec = !current_base.is_in_spec();
  let current_time = plan_time(weights, current_base.plus(implants));

  let mut best_base = None::<Attributes>;
  let mut best_time = f64::INFINITY;

  for perception in ATTR_MIN..=ATTR_MAX {
    for memory in ATTR_MIN..=ATTR_MAX {
      for willpower in ATTR_MIN..=ATTR_MAX {
        for intelligence in ATTR_MIN..=ATTR_MAX {
          let fixed = perception + memory + willpower + intelligence;
          if fixed + ATTR_MIN > BASE_TOTAL {
            continue;
          }
          let charisma = BASE_TOTAL - fixed;
          if !(ATTR_MIN..=ATTR_MAX).contains(&charisma) {
            continue;
          }

          let base = Attributes {
            charisma,
            intelligence,
            memory,
            perception,
            willpower,
          };
          let time = plan_time(weights, base.plus(implants));
          if best_base.is_none() || time < best_time {
            best_base = Some(base);
            best_time = time;
          }
        }
      }
    }
  }

  let best_base = best_base.expect("the [17,27] cube always contains an in-spec allocation summing to 99");
  let current_wins = current_time <= best_time;

  if !current_out_of_spec && current_wins {
    Recommendation {
      base: current_base,
      total_sec: current_time,
      is_current: true,
      current_out_of_spec,
    }
  } else {
    Recommendation {
      base: best_base,
      total_sec: best_time,
      is_current: current_wins,
      current_out_of_spec,
    }
  }
}

fn plan_time(weights: &[PairWeight], effective: Attributes) -> f64 {
  let mut total = 0.0;
  for weight in weights {
    let rate = sp_per_sec(effective.get(weight.primary), effective.get(weight.secondary));
    if rate <= 0.0 {
      return f64::INFINITY;
    }
    total += weight.sp as f64 / rate;
  }
  total
}

#[cfg(test)]
mod tests {
  use super::*;

  fn no_implants() -> Attributes {
    Attributes::default()
  }

  fn per_wil_plan(sp: u64) -> Vec<PairWeight> {
    vec![PairWeight {
      primary: Attribute::Perception,
      secondary: Attribute::Willpower,
      sp,
    }]
  }

  mod fixture_gate {
    use pretty_assertions::assert_eq;

    use super::{
      super::super::attributes::{self, Role},
      *,
    };
    use crate::clients::esi::models::{character, universe};

    const ATTRIBUTES_FIXTURE: &str = include_str!("../../../tests/fixtures/esi/character_attributes.json");

    const IMPLANT_FIXTURE: &str = include_str!("../../../tests/fixtures/esi/universe_types_9899.json");

    const CHARISMA_BONUS_ID: i32 = 175;

    const INTELLIGENCE_BONUS_ID: i32 = 176;

    const MEMORY_BONUS_ID: i32 = 177;

    const PERCEPTION_BONUS_ID: i32 = 178;

    const WILLPOWER_BONUS_ID: i32 = 179;

    fn base_from_esi(attributes: &character::Attributes) -> Attributes {
      Attributes {
        charisma: attributes.charisma as u32,
        intelligence: attributes.intelligence as u32,
        memory: attributes.memory as u32,
        perception: attributes.perception as u32,
        willpower: attributes.willpower as u32,
      }
    }

    fn implants_from_esi(items: &[universe::ItemType]) -> Attributes {
      let mut implants = Attributes::default();
      for item in items {
        for dogma in &item.dogma_attributes {
          let bonus = dogma.value.round().max(0.0) as u32;
          match dogma.attribute_id {
            CHARISMA_BONUS_ID => implants.charisma += bonus,
            INTELLIGENCE_BONUS_ID => implants.intelligence += bonus,
            MEMORY_BONUS_ID => implants.memory += bonus,
            PERCEPTION_BONUS_ID => implants.perception += bonus,
            WILLPOWER_BONUS_ID => implants.willpower += bonus,
            _ => {}
          }
        }
      }
      implants
    }

    fn fixtures() -> (Attributes, Attributes) {
      let attributes: character::Attributes =
        serde_json::from_str(ATTRIBUTES_FIXTURE).expect("parse attributes fixture");
      let implant: universe::ItemType = serde_json::from_str(IMPLANT_FIXTURE).expect("parse implant fixture");
      (base_from_esi(&attributes), implants_from_esi(&[implant]))
    }

    const ACTIVE_PAIR: (Attribute, Attribute) = (Attribute::Intelligence, Attribute::Memory);

    fn plan() -> Vec<PairWeight> {
      vec![
        PairWeight {
          primary: ACTIVE_PAIR.0,
          secondary: ACTIVE_PAIR.1,
          sp: 4_000_000,
        },
        PairWeight {
          primary: Attribute::Perception,
          secondary: Attribute::Willpower,
          sp: 1_000_000,
        },
      ]
    }

    fn effective_of(attribute: Attribute, base: Attributes, implants: Attributes) -> u32 {
      base.get(attribute) + implants.get(attribute)
    }

    #[test]
    fn it_computes_effective_as_base_plus_implant_without_clamping_in_both_paths() {
      let (base, implants) = fixtures();

      let rec = optimize_remap(&plan(), base, implants);
      let effective = rec.base.plus(implants);
      assert_eq!(effective.memory, rec.base.memory + 3);
      let recomputed = plan_time(&plan(), effective);
      assert!(
        (rec.total_sec - recomputed).abs() < 1e-6,
        "optimizer time {} does not match unclamped effective recompute {}",
        rec.total_sec,
        recomputed
      );

      let rows = attributes::project_rows(base, implants, None);
      for row in rows {
        assert_eq!(
          row.effective,
          row.base + row.implant,
          "projected effective for {:?} is not base + implant",
          row.attribute
        );
        assert_eq!(
          row.effective,
          effective_of(row.attribute, base, implants),
          "projection and base+implant disagree for {:?}",
          row.attribute
        );
      }

      let memory_row = rows.iter().find(|row| row.attribute == Attribute::Memory).unwrap();
      assert_eq!(memory_row.base, 21);
      assert_eq!(memory_row.implant, 3);
      assert_eq!(memory_row.effective, 24);

      let matrix = attributes::sp_per_hr_matrix(effective, Some(ACTIVE_PAIR));
      let int_mem = matrix
        .iter()
        .find(|cell| (cell.primary, cell.secondary) == ACTIVE_PAIR)
        .unwrap();
      let expected = (sp_per_sec(effective.intelligence, effective.memory) * 3_600.0).round() as u64;
      assert_eq!(int_mem.sp_per_hr, expected);
      assert!(int_mem.active);
    }

    #[test]
    fn it_flags_the_out_of_spec_fixture_base_and_still_emits_an_in_spec_recommendation() {
      let (base, implants) = fixtures();

      assert_eq!(
        base.charisma + base.intelligence + base.memory + base.perception + base.willpower,
        103
      );
      assert!(
        !base.is_in_spec(),
        "fixture base {base:?} is expected to be out of spec"
      );

      let rec = optimize_remap(&plan(), base, implants);

      assert!(rec.current_out_of_spec);

      let emitted = [
        rec.base.charisma,
        rec.base.intelligence,
        rec.base.memory,
        rec.base.perception,
        rec.base.willpower,
      ];
      assert_eq!(emitted.iter().sum::<u32>(), 99);
      assert!(
        emitted.iter().all(|&value| (17..=27).contains(&value)),
        "emitted base {:?} has an attribute outside [17, 27]",
        rec.base
      );
      assert!(rec.base.is_in_spec());

      let baseline_time = plan_time(&plan(), base.plus(implants));
      assert!(
        rec.total_sec <= baseline_time,
        "recommendation {} must not be slower than the real baseline {}",
        rec.total_sec,
        baseline_time
      );
    }

    #[test]
    fn it_marks_the_active_pair_in_the_projection_for_the_emitted_recommendation() {
      let (base, implants) = fixtures();
      let rec = optimize_remap(&plan(), base, implants);

      let rows = attributes::project_rows(rec.base, implants, Some(ACTIVE_PAIR));
      let intelligence_row = rows
        .iter()
        .find(|row| row.attribute == Attribute::Intelligence)
        .unwrap();
      let memory_row = rows.iter().find(|row| row.attribute == Attribute::Memory).unwrap();

      assert_eq!(intelligence_row.role, Role::Primary);
      assert_eq!(memory_row.role, Role::Secondary);
      assert_eq!(memory_row.effective, rec.base.memory + 3);
    }

    #[test]
    fn it_resolves_the_real_implant_as_a_plus_three_memory_bonus() {
      let (_, implants) = fixtures();

      assert_eq!(implants.memory, 3);
      assert_eq!(implants.charisma, 0);
      assert_eq!(implants.intelligence, 0);
      assert_eq!(implants.perception, 0);
      assert_eq!(implants.willpower, 0);
    }
  }

  mod formulas {
    use pretty_assertions::assert_eq;

    use super::{super::super::format::sp_cost, *};

    #[test]
    fn sp_cost_is_256000_at_level_five_rank_one() {
      assert_eq!(sp_cost(1.0, 5), 256_000);
    }

    #[test]
    fn sp_per_sec_matches_the_pod_rate_formula() {
      assert_eq!(sp_per_sec(27, 21), (27.0 + 10.5) / 60.0);
    }
  }

  mod optimize_remap {
    use pretty_assertions::assert_eq;

    use super::*;

    fn base(perception: u32, memory: u32, willpower: u32, intelligence: u32, charisma: u32) -> Attributes {
      Attributes {
        charisma,
        intelligence,
        memory,
        perception,
        willpower,
      }
    }

    fn best_per_wil_base() -> Attributes {
      base(27, 17, 21, 17, 17)
    }

    #[test]
    fn it_does_not_flag_an_in_spec_current() {
      let rec = optimize_remap(&per_wil_plan(1_000_000), base(20, 20, 20, 20, 19), no_implants());

      assert!(!rec.current_out_of_spec);
    }

    #[test]
    fn it_finds_the_known_optimum_for_a_single_pair() {
      let rec = optimize_remap(&per_wil_plan(1_000_000), base(20, 20, 20, 20, 19), no_implants());

      assert_eq!(rec.base, best_per_wil_base());
      assert!(!rec.is_current);
    }

    #[test]
    fn it_flags_an_out_of_spec_current_and_still_emits_an_in_spec_base() {
      let current = base(17, 17, 17, 17, 31);
      let weights = vec![PairWeight {
        primary: Attribute::Charisma,
        secondary: Attribute::Willpower,
        sp: 1_000_000,
      }];
      let current_effective = current.plus(no_implants());
      let current_time = 1_000_000.0 / sp_per_sec(current_effective.charisma, current_effective.willpower);

      let rec = optimize_remap(&weights, current, no_implants());

      assert!(rec.current_out_of_spec);
      assert!(rec.base.is_in_spec());
      assert!(rec.is_current);
      assert!(rec.total_sec >= current_time);
    }

    #[test]
    fn it_never_clamps_effective_attributes_above_the_max() {
      let implants = Attributes {
        charisma: 5,
        intelligence: 5,
        memory: 5,
        perception: 5,
        willpower: 5,
      };
      let weights = per_wil_plan(1_000_000);

      let rec = optimize_remap(&weights, base(20, 20, 20, 20, 19), implants);

      assert_eq!(rec.base.perception, 27);
      let effective = rec.base.plus(implants);
      let clamped_rate = sp_per_sec(27, effective.willpower.min(27));
      let real_rate = sp_per_sec(effective.perception, effective.willpower);
      assert!(real_rate > clamped_rate, "effective {effective:?} appears clamped");
      let expected = 1_000_000.0 / real_rate;
      assert!((rec.total_sec - expected).abs() < 1e-6);
    }

    #[test]
    fn it_never_recommends_a_config_slower_than_current() {
      let weights = per_wil_plan(2_000_000);
      let current = base(20, 20, 20, 20, 19);
      let current_time = {
        let effective = current.plus(no_implants());
        2_000_000.0 / sp_per_sec(effective.perception, effective.willpower)
      };

      let rec = optimize_remap(&weights, current, no_implants());

      assert!(rec.total_sec <= current_time);
    }

    #[test]
    fn it_only_ever_emits_an_in_spec_base() {
      let plans = [
        per_wil_plan(1_000_000),
        vec![
          PairWeight {
            primary: Attribute::Intelligence,
            secondary: Attribute::Memory,
            sp: 5_000_000,
          },
          PairWeight {
            primary: Attribute::Charisma,
            secondary: Attribute::Willpower,
            sp: 200_000,
          },
        ],
      ];
      let currents = [
        base(20, 20, 20, 20, 19),
        base(99, 99, 99, 99, 99),
        base(17, 17, 17, 17, 31),
      ];

      for plan in &plans {
        for &current in &currents {
          let rec = optimize_remap(plan, current, base(5, 5, 5, 5, 5));

          assert!(rec.base.is_in_spec(), "emitted base {:?} is out of spec", rec.base);
        }
      }
    }

    #[test]
    fn it_reports_is_current_when_the_current_base_is_already_optimal() {
      let weights = per_wil_plan(1_000_000);
      let optimal = best_per_wil_base();

      let rec = optimize_remap(&weights, optimal, no_implants());

      assert!(rec.is_current);
      assert_eq!(rec.base, optimal);
    }

    #[test]
    fn it_returns_the_current_baseline_for_an_empty_plan() {
      let current = base(20, 20, 20, 20, 19);

      let rec = optimize_remap(&[], current, no_implants());

      assert!(rec.is_current);
      assert_eq!(rec.base, current);
      assert_eq!(rec.total_sec, 0.0);
    }
  }
}
