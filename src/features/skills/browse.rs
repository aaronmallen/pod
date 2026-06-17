use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::format::{fmt_dur_short, sp_cost, sp_per_sec};
use crate::store::model::{CharacterSkill, CharacterSkillqueue};

const ATTR_TABLE: [(u8, &str, &str); 5] = [
  (167, "Per", "Perception"),
  (168, "Wil", "Willpower"),
  (165, "Int", "Intelligence"),
  (166, "Mem", "Memory"),
  (164, "Cha", "Charisma"),
];

// Rule-4 exception: variants stay in EVE attribute order rather than alphabetically because each discriminant
// indexes `ATTR_TABLE` and the `attrs` array (used via `self as usize`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(usize)]
pub enum AttrKey {
  Perception = 0,
  Willpower = 1,
  Intelligence = 2,
  Memory = 3,
  Charisma = 4,
}

impl AttrKey {
  pub const ALL: [AttrKey; 5] = [
    AttrKey::Perception,
    AttrKey::Willpower,
    AttrKey::Intelligence,
    AttrKey::Memory,
    AttrKey::Charisma,
  ];

  pub fn from_eve_id(id: u8) -> Self {
    ATTR_TABLE
      .iter()
      .position(|row| row.0 == id)
      .and_then(|i| AttrKey::ALL.get(i).copied())
      .unwrap_or(AttrKey::Perception)
  }

  pub fn label(self) -> &'static str {
    ATTR_TABLE[self as usize].2
  }

  pub fn short(self) -> &'static str {
    ATTR_TABLE[self as usize].1
  }

  fn value(self, attrs: [u32; 5]) -> u32 {
    attrs[self as usize]
  }
}

#[derive(Clone, Debug)]
pub struct GroupRow {
  pub id: i64,
  pub leaves: Vec<SkillLeaf>,
  pub name: String,
  pub total_skills: usize,
  pub total_sp: i64,
  pub trained_count: usize,
}

#[derive(Clone, Debug)]
pub struct SkillCatalog {
  pub groups: Vec<SkillCatalogGroup>,
}

#[derive(Clone, Debug)]
pub struct SkillCatalogEntry {
  #[allow(dead_code)]
  pub group_id: i64,
  pub group_name: String,
  pub name: String,
  pub prereqs: Vec<(String, u8)>,
  pub primary_attr: AttrKey,
  pub rank: u8,
  pub secondary_attr: AttrKey,
  pub type_id: i64,
}

#[derive(Clone, Debug)]
pub struct SkillCatalogGroup {
  pub id: i64,
  pub name: String,
  pub skills: Vec<SkillCatalogEntry>,
}

#[derive(Clone, Debug)]
pub struct SkillLeaf {
  pub level: u8,
  pub name: String,
  pub next_eta: String,
  pub prereqs: Vec<(String, u8)>,
  pub queue_delta: u8,
  pub rank: u8,
  pub skill_id: i64,
}

pub fn build_browser_tree(
  catalog: &SkillCatalog,
  skills: &[CharacterSkill],
  queue: &[CharacterSkillqueue],
  effective_attrs: [u32; 5],
  now: DateTime<Utc>,
) -> Vec<GroupRow> {
  let _ = now;

  let trained_by_id: HashMap<i64, &CharacterSkill> = skills.iter().map(|skill| (skill.skill_id(), skill)).collect();

  let mut max_queued_by_id: HashMap<i64, u8> = HashMap::new();
  for entry in queue {
    let level = entry.finished_level().clamp(0, i64::from(u8::MAX)) as u8;
    max_queued_by_id
      .entry(entry.skill_id())
      .and_modify(|current| *current = (*current).max(level))
      .or_insert(level);
  }

  let mut groups: Vec<GroupRow> = catalog
    .groups
    .iter()
    .map(|group| {
      let leaves: Vec<SkillLeaf> = group
        .skills
        .iter()
        .map(|skill| leaf_for_skill(skill, &trained_by_id, &max_queued_by_id, effective_attrs))
        .collect();

      let trained_count = leaves.iter().filter(|leaf| leaf.level >= 5).count();
      let total_skills = leaves.len();
      let total_sp = group
        .skills
        .iter()
        .filter_map(|skill| trained_by_id.get(&skill.type_id))
        .map(|skill| skill.skillpoints_in_skill())
        .sum();

      GroupRow {
        id: group.id,
        leaves,
        name: group.name.clone(),
        total_skills,
        total_sp,
        trained_count,
      }
    })
    .collect();

  groups.sort_by(|a, b| a.name.cmp(&b.name));
  groups
}

fn leaf_for_skill(
  skill: &SkillCatalogEntry,
  trained_by_id: &HashMap<i64, &CharacterSkill>,
  max_queued_by_id: &HashMap<i64, u8>,
  effective_attrs: [u32; 5],
) -> SkillLeaf {
  let trained_level = trained_by_id
    .get(&skill.type_id)
    .map(|row| row.trained_skill_level().clamp(0, i64::from(u8::MAX)) as u8)
    .unwrap_or(0);
  let max_queued_level = max_queued_by_id.get(&skill.type_id).copied().unwrap_or(0);

  let queue_delta = max_queued_level.saturating_sub(trained_level);

  let prereqs = if trained_level == 0 && !skill.prereqs.is_empty() {
    skill.prereqs.clone()
  } else {
    Vec::new()
  };

  let next_eta = next_level_eta(skill, trained_level, max_queued_level, effective_attrs);

  SkillLeaf {
    level: trained_level,
    name: skill.name.clone(),
    next_eta,
    prereqs,
    queue_delta,
    rank: skill.rank,
    skill_id: skill.type_id,
  }
}

fn next_level_eta(
  skill: &SkillCatalogEntry,
  trained_level: u8,
  max_queued_level: u8,
  effective_attrs: [u32; 5],
) -> String {
  let dash = "\u{2014}".to_owned();

  let progress = trained_level.max(max_queued_level);
  if progress >= 5 {
    return dash;
  }
  let next_level = progress + 1;

  let sp_rate = sp_per_sec(
    skill.primary_attr.value(effective_attrs),
    skill.secondary_attr.value(effective_attrs),
  );
  if sp_rate <= 0.0 {
    return dash;
  }

  let seconds = (sp_cost(f64::from(skill.rank), next_level) as f64 / sp_rate).round() as i64;
  fmt_dur_short(seconds)
}

#[cfg(test)]
mod tests {
  use super::*;

  mod attr_key {
    use super::*;

    mod from_eve_id {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_falls_back_to_perception_for_an_unknown_id() {
        assert_eq!(AttrKey::from_eve_id(0), AttrKey::Perception);
        assert_eq!(AttrKey::from_eve_id(255), AttrKey::Perception);
      }

      #[test]
      fn it_maps_each_dogma_id_to_its_attr() {
        assert_eq!(AttrKey::from_eve_id(164), AttrKey::Charisma);
        assert_eq!(AttrKey::from_eve_id(165), AttrKey::Intelligence);
        assert_eq!(AttrKey::from_eve_id(166), AttrKey::Memory);
        assert_eq!(AttrKey::from_eve_id(167), AttrKey::Perception);
        assert_eq!(AttrKey::from_eve_id(168), AttrKey::Willpower);
      }
    }

    mod label {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_the_display_label_for_each_attr() {
        assert_eq!(AttrKey::Charisma.label(), "Charisma");
        assert_eq!(AttrKey::Intelligence.label(), "Intelligence");
        assert_eq!(AttrKey::Memory.label(), "Memory");
        assert_eq!(AttrKey::Perception.label(), "Perception");
        assert_eq!(AttrKey::Willpower.label(), "Willpower");
      }
    }

    mod short {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_the_short_label_for_each_attr() {
        assert_eq!(AttrKey::Charisma.short(), "Cha");
        assert_eq!(AttrKey::Intelligence.short(), "Int");
        assert_eq!(AttrKey::Memory.short(), "Mem");
        assert_eq!(AttrKey::Perception.short(), "Per");
        assert_eq!(AttrKey::Willpower.short(), "Wil");
      }
    }
  }

  mod build_browser_tree {
    use chrono::TimeZone as _;
    use pretty_assertions::assert_eq;

    use super::*;

    const ATTRS: [u32; 5] = [27, 21, 24, 20, 19];

    fn now() -> DateTime<Utc> {
      Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap()
    }

    fn entry(type_id: i64, name: &str, prereqs: Vec<(String, u8)>) -> SkillCatalogEntry {
      SkillCatalogEntry {
        group_id: 255,
        group_name: "Gunnery".to_owned(),
        name: name.to_owned(),
        primary_attr: AttrKey::Perception,
        prereqs,
        rank: 1,
        secondary_attr: AttrKey::Intelligence,
        type_id,
      }
    }

    fn catalog(groups: Vec<SkillCatalogGroup>) -> SkillCatalog {
      SkillCatalog {
        groups,
      }
    }

    fn skill(skill_id: i64, trained_skill_level: i64, skillpoints_in_skill: i64) -> CharacterSkill {
      CharacterSkill {
        active_skill_level: trained_skill_level,
        character_id: 42,
        skill_id,
        skillpoints_in_skill,
        trained_skill_level,
      }
    }

    fn queued(skill_id: i64, finished_level: i64) -> CharacterSkillqueue {
      CharacterSkillqueue {
        character_id: 42,
        finish_date: None,
        finished_level,
        level_end_sp: None,
        level_start_sp: None,
        queue_position: 0,
        skill_id,
        start_date: None,
        training_start_sp: None,
      }
    }

    #[test]
    fn it_advances_the_eta_target_past_the_queued_level() {
      let cat = catalog(vec![SkillCatalogGroup {
        id: 255,
        name: "Gunnery".to_owned(),
        skills: vec![entry(3300, "Gunnery", vec![])],
      }]);
      let skills = [skill(3300, 2, 100)];
      let queue = [queued(3300, 4)];

      let tree = build_browser_tree(&cat, &skills, &queue, ATTRS, now());

      let rate = sp_per_sec(27, 21);
      let expected = fmt_dur_short((sp_cost(1.0, 5) as f64 / rate).round() as i64);
      assert_eq!(tree[0].leaves[0].next_eta, expected);
    }

    #[test]
    fn it_computes_the_next_level_eta_from_cost_over_rate() {
      let cat = catalog(vec![SkillCatalogGroup {
        id: 255,
        name: "Gunnery".to_owned(),
        skills: vec![entry(3300, "Gunnery", vec![])],
      }]);
      let skills = [skill(3300, 2, 100)];

      let tree = build_browser_tree(&cat, &skills, &[], ATTRS, now());

      let rate = sp_per_sec(27, 21);
      let expected = fmt_dur_short((sp_cost(1.0, 3) as f64 / rate).round() as i64);
      assert_eq!(tree[0].leaves[0].next_eta, expected);
      assert_ne!(tree[0].leaves[0].next_eta, "\u{2014}");
    }

    #[test]
    fn it_derives_the_queue_delta_as_max_queued_minus_trained() {
      let cat = catalog(vec![SkillCatalogGroup {
        id: 255,
        name: "Gunnery".to_owned(),
        skills: vec![entry(3300, "Gunnery", vec![])],
      }]);
      let skills = [skill(3300, 2, 100)];
      let queue = [queued(3300, 4), queued(3300, 5)];

      let tree = build_browser_tree(&cat, &skills, &queue, ATTRS, now());

      assert_eq!(tree[0].leaves[0].queue_delta, 3, "5 queued − 2 trained = 3");
    }

    #[test]
    fn it_guards_against_a_non_positive_sp_rate() {
      let cat = catalog(vec![SkillCatalogGroup {
        id: 255,
        name: "Gunnery".to_owned(),
        skills: vec![entry(3300, "Gunnery", vec![])],
      }]);

      let tree = build_browser_tree(&cat, &[], &[], [0; 5], now());

      assert_eq!(tree[0].leaves[0].next_eta, "\u{2014}");
    }

    #[test]
    fn it_renders_a_dash_when_already_at_level_five() {
      let cat = catalog(vec![SkillCatalogGroup {
        id: 255,
        name: "Gunnery".to_owned(),
        skills: vec![entry(3300, "MaxedTrained", vec![]), entry(3301, "QueuedToFive", vec![])],
      }]);
      let skills = [skill(3300, 5, 256_000), skill(3301, 1, 100)];
      let queue = [queued(3301, 5)];

      let tree = build_browser_tree(&cat, &skills, &queue, ATTRS, now());

      let leaves = &tree[0].leaves;
      assert_eq!(leaves[0].next_eta, "\u{2014}", "trained to 5 → no next level");
      assert_eq!(leaves[1].next_eta, "\u{2014}", "queued to 5 → no next level");
    }

    #[test]
    fn it_reports_no_queue_delta_when_the_queue_does_not_raise_the_skill() {
      let cat = catalog(vec![SkillCatalogGroup {
        id: 255,
        name: "Gunnery".to_owned(),
        skills: vec![entry(3300, "Gunnery", vec![])],
      }]);
      let skills = [skill(3300, 4, 100)];
      let queue = [queued(3300, 4)];

      let tree = build_browser_tree(&cat, &skills, &queue, ATTRS, now());

      assert_eq!(tree[0].leaves[0].queue_delta, 0);
    }

    #[test]
    fn it_resolves_trained_level_from_the_skill_sheet_or_zero_without_a_row() {
      let cat = catalog(vec![SkillCatalogGroup {
        id: 255,
        name: "Gunnery".to_owned(),
        skills: vec![entry(3300, "Trained", vec![]), entry(3301, "Untrained", vec![])],
      }]);
      let skills = [skill(3300, 4, 90_510)];

      let tree = build_browser_tree(&cat, &skills, &[], ATTRS, now());

      let leaves = &tree[0].leaves;
      assert_eq!(leaves[0].level, 4, "skill with a row uses its trained_skill_level");
      assert_eq!(leaves[1].level, 0, "skill with no row resolves to level 0");
    }

    #[test]
    fn it_returns_one_group_per_catalog_group_sorted_by_name() {
      let cat = catalog(vec![
        SkillCatalogGroup {
          id: 257,
          name: "Spaceship Command".to_owned(),
          skills: vec![entry(3327, "Spaceship Command", vec![])],
        },
        SkillCatalogGroup {
          id: 255,
          name: "Gunnery".to_owned(),
          skills: vec![entry(3300, "Gunnery", vec![])],
        },
      ]);

      let tree = build_browser_tree(&cat, &[], &[], ATTRS, now());

      assert_eq!(tree.len(), 2);
      assert_eq!(tree[0].name, "Gunnery");
      assert_eq!(tree[1].name, "Spaceship Command");
    }

    #[test]
    fn it_shows_prereq_chips_only_for_an_untrained_skill_with_prereqs() {
      let prereqs = vec![("Spaceship Command".to_owned(), 3)];
      let cat = catalog(vec![SkillCatalogGroup {
        id: 255,
        name: "Gunnery".to_owned(),
        skills: vec![
          entry(3300, "Untrained", prereqs.clone()),
          entry(3301, "Trained", prereqs.clone()),
          entry(3302, "NoPrereqs", vec![]),
        ],
      }]);
      let skills = [skill(3301, 1, 100)];

      let tree = build_browser_tree(&cat, &skills, &[], ATTRS, now());

      let leaves = &tree[0].leaves;
      assert_eq!(leaves[0].prereqs, prereqs, "level 0 with prereqs shows chips");
      assert!(leaves[1].prereqs.is_empty(), "trained skill never shows chips");
      assert!(leaves[2].prereqs.is_empty(), "level 0 without prereqs shows none");
    }

    #[test]
    fn it_sums_invested_sp_across_a_group() {
      let cat = catalog(vec![SkillCatalogGroup {
        id: 255,
        name: "Gunnery".to_owned(),
        skills: vec![
          entry(3300, "A", vec![]),
          entry(3301, "B", vec![]),
          entry(3302, "Untracked", vec![]),
        ],
      }]);
      let skills = [skill(3300, 5, 256_000), skill(3301, 3, 9_414)];

      let tree = build_browser_tree(&cat, &skills, &[], ATTRS, now());

      assert_eq!(tree[0].total_sp, 256_000 + 9_414);
      assert_eq!(tree[0].total_skills, 3);
      assert_eq!(tree[0].trained_count, 1, "only the level-5 skill counts as trained");
    }
  }
}
