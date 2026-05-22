//! Plan computation engine: prereq expansion, SP/time calculation,
//! attribute optimizer, and implant savings analysis.
//!
//! All functions are pure — no DB, no Iced tasks, no global state.

use std::collections::HashMap;

use pod_model::{AttrKey, SkillDef, SkillGroupDef};

use crate::format::{sp_cost, sp_per_sec};

/// Collects skills required for mastery tiers 1 through `mastery_level` (cumulative).
/// Deduplicates by skill name, keeping the highest required level.
pub fn skills_for_mastery(
  cert_ids: &[Vec<i32>],
  mastery_level: u8,
  certificates: &HashMap<i32, pod_model::Certificate>,
  lookup: &dyn Fn(i32) -> Option<String>,
) -> Vec<(String, u8)> {
  let mut by_name: HashMap<String, u8> = HashMap::new();
  let tier_count = mastery_level.min(5) as usize;
  for (tier_idx, tier_certs) in cert_ids.iter().take(tier_count).enumerate() {
    let prof_idx = tier_idx.min(3); // 0=basic 1=improved 2=advanced 3+=elite
    for &cert_id in tier_certs {
      let Some(cert) = certificates.get(&cert_id) else {
        continue;
      };
      for &(type_id, levels) in &cert.skills {
        let level = levels[prof_idx];
        let Some(name) = lookup(type_id) else {
          continue;
        };
        let entry = by_name.entry(name).or_insert(0);
        if level > *entry {
          *entry = level;
        }
      }
    }
  }
  by_name.into_iter().collect()
}

/// Returns the skill requirements as-is — names are already resolved at the DB layer.
pub fn skills_for_module(requirements: &[(String, u8)]) -> Vec<(String, u8)> {
  requirements.to_vec()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
  Low,
  Normal,
  High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImplantSet {
  None,
  Plus3,
  Plus4,
  Plus5,
  Current,
}

#[derive(Debug, Clone, Default)]
pub struct BaseAttrs {
  pub perception: i32,
  pub memory: i32,
  pub willpower: i32,
  pub intelligence: i32,
  pub charisma: i32,
}

pub type ImplantBonus = BaseAttrs;
pub type EffectiveAttrs = BaseAttrs;

#[derive(Debug, Clone)]
pub struct PlanEntry {
  pub id: String,
  pub skill_name: String,
  pub to_level: u8,
  pub priority: Priority,
  pub note: Option<String>,
  pub auto: bool,
}

#[derive(Debug, Clone)]
pub struct ComputedEntry {
  pub id: String,
  pub skill_name: String,
  pub to_level: u8,
  pub from_level: u8,
  pub priority: Priority,
  pub note: Option<String>,
  pub auto: bool,
  pub skipped: bool,
  pub sp: u64,
  pub sec: f64,
  pub cum_sec: f64,
  pub eta_unix_ms: i64,
  pub rank: u8,
  pub primary: AttrKey,
  pub secondary: AttrKey,
}

#[derive(Debug, Clone, Default)]
pub struct ComputedPlan {
  pub items: Vec<ComputedEntry>,
  pub total_sec: f64,
  pub total_sp: u64,
  pub group_sec: HashMap<String, f64>,
  pub pair_sec: HashMap<String, f64>,
}

#[derive(Debug, Clone)]
pub struct PairWeight {
  pub primary: AttrKey,
  pub secondary: AttrKey,
  pub sp: u64,
}

#[derive(Debug, Clone)]
pub struct RemapResult {
  pub base: BaseAttrs,
  pub total_sec: f64,
  pub current_sec: f64,
  pub is_current: bool,
}

#[derive(Debug, Clone)]
pub struct ImplantSaving {
  pub attr: AttrKey,
  pub saved_sec: f64,
}

fn find_skill_in_groups<'a>(name: &str, skill_groups: &'a [SkillGroupDef]) -> Option<(&'a SkillDef, &'a str)> {
  for g in skill_groups {
    if let Some(s) = g.skills.iter().find(|s| s.name == name) {
      return Some((s, &g.name));
    }
  }
  None
}

fn schedule_skill(
  skill_name: &str,
  to_level: u8,
  trained: &mut HashMap<String, u8>,
  is_auto: bool,
  skill_groups: &[SkillGroupDef],
  out: &mut Vec<PlanEntry>,
) {
  let Some((skill, _group)) = find_skill_in_groups(skill_name, skill_groups) else {
    return;
  };

  let current = std::cmp::max(skill.level, trained.get(skill_name).copied().unwrap_or(0));
  if current >= to_level {
    return;
  }

  for (req_skill, req_level) in &skill.prereqs {
    schedule_skill(req_skill, *req_level, trained, true, skill_groups, out);
  }

  for lv in (current + 1)..=to_level {
    let id = format!("{}-{}", skill_name.to_lowercase().replace(' ', "-"), lv);
    out.push(PlanEntry {
      id,
      skill_name: skill_name.to_string(),
      to_level: lv,
      priority: Priority::Normal,
      note: None,
      auto: is_auto,
    });
  }
  trained.insert(skill_name.to_string(), to_level);
}

/// Expand a list of (skill_name, target_level) wishes into a flat list of
/// single-level entries with prereqs filled in. Entries whose (skill, level)
/// was in the original wishes list have `auto: false`; prereq-inserted entries
/// have `auto: true`.
pub fn expand_wishes(wishes: &[(&str, u8)], skill_groups: &[SkillGroupDef]) -> Vec<PlanEntry> {
  let mut trained: HashMap<String, u8> = HashMap::new();
  let mut out: Vec<PlanEntry> = Vec::new();

  for &(skill, level) in wishes {
    schedule_skill(skill, level, &mut trained, false, skill_groups, &mut out);
  }

  let wish_set: std::collections::HashSet<String> = wishes
    .iter()
    .map(|&(skill, level)| format!("{}|{}", skill, level))
    .collect();

  for entry in &mut out {
    let key = format!("{}|{}", entry.skill_name, entry.to_level);
    entry.auto = !wish_set.contains(&key);
  }

  out
}

fn attr_value(attrs: &EffectiveAttrs, key: AttrKey) -> u32 {
  let v = match key {
    AttrKey::Perception => attrs.perception,
    AttrKey::Memory => attrs.memory,
    AttrKey::Willpower => attrs.willpower,
    AttrKey::Intelligence => attrs.intelligence,
    AttrKey::Charisma => attrs.charisma,
  };
  v.max(0) as u32
}

/// Compute the full plan: per-entry SP, durations, cumulative time, ETAs, and
/// group/pair breakdowns.
pub fn compute_plan(entries: &[PlanEntry], attrs: &EffectiveAttrs, skill_groups: &[SkillGroupDef]) -> ComputedPlan {
  let now_ms = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis() as i64;

  let mut cum_sec = 0.0f64;
  let mut cum_sp = 0u64;
  let mut items = Vec::new();
  let mut group_sec: HashMap<String, f64> = HashMap::new();
  let mut pair_sec: HashMap<String, f64> = HashMap::new();
  let mut skill_progress: HashMap<&str, u8> = HashMap::new();

  for entry in entries {
    let Some((skill, group_name)) = find_skill_in_groups(&entry.skill_name, skill_groups) else {
      continue;
    };

    let starting_level = std::cmp::max(
      skill.level,
      skill_progress.get(entry.skill_name.as_str()).copied().unwrap_or(0),
    );

    if entry.to_level <= starting_level {
      items.push(ComputedEntry {
        id: entry.id.clone(),
        skill_name: entry.skill_name.clone(),
        to_level: entry.to_level,
        from_level: starting_level,
        priority: entry.priority,
        note: entry.note.clone(),
        auto: entry.auto,
        skipped: true,
        sp: 0,
        sec: 0.0,
        cum_sec,
        eta_unix_ms: now_ms + (cum_sec * 1000.0) as i64,
        rank: skill.rank,
        primary: skill.primary,
        secondary: skill.secondary,
      });
      continue;
    }

    let step_sp = if entry.to_level == starting_level + 1 {
      if !skill_progress.contains_key(entry.skill_name.as_str()) && skill.level == starting_level {
        let full_level_sp = sp_cost(skill.rank as f64, entry.to_level);
        (full_level_sp as i64 - skill.sp as i64).max(0) as u64
      } else {
        let cost_to = sp_cost(skill.rank as f64, entry.to_level);
        let cost_from = sp_cost(skill.rank as f64, starting_level);
        cost_to.saturating_sub(cost_from)
      }
    } else {
      sp_cost(skill.rank as f64, entry.to_level)
    };

    let rate = sp_per_sec(attr_value(attrs, skill.primary), attr_value(attrs, skill.secondary)) as f64;

    let sec = if rate > 0.0 { step_sp as f64 / rate } else { 0.0 };
    cum_sec += sec;
    cum_sp += step_sp;

    let pair_key = format!("{}/{}", skill.primary.label(), skill.secondary.label());
    *pair_sec.entry(pair_key).or_insert(0.0) += sec;
    *group_sec.entry(group_name.to_owned()).or_insert(0.0) += sec;

    items.push(ComputedEntry {
      id: entry.id.clone(),
      skill_name: entry.skill_name.clone(),
      to_level: entry.to_level,
      from_level: starting_level,
      priority: entry.priority,
      note: entry.note.clone(),
      auto: entry.auto,
      skipped: false,
      sp: step_sp,
      sec,
      cum_sec,
      eta_unix_ms: now_ms + (cum_sec * 1000.0) as i64,
      rank: skill.rank,
      primary: skill.primary,
      secondary: skill.secondary,
    });

    skill_progress.insert(entry.skill_name.as_str(), entry.to_level);
  }

  ComputedPlan {
    items,
    total_sec: cum_sec,
    total_sp: cum_sp,
    group_sec,
    pair_sec,
  }
}

/// Compute effective attributes by adding implant bonuses to base attributes.
pub fn effective_attrs(base: &BaseAttrs, implant: &ImplantBonus) -> EffectiveAttrs {
  EffectiveAttrs {
    perception: base.perception + implant.perception,
    memory: base.memory + implant.memory,
    willpower: base.willpower + implant.willpower,
    intelligence: base.intelligence + implant.intelligence,
    charisma: base.charisma + implant.charisma,
  }
}

/// Return the implant bonus for a given implant set. For `ImplantSet::Current`,
/// the caller should pass the character's actual implant values as
/// `current_attrs_implants`.
pub fn implant_bonus_for_set(set: ImplantSet, current_attrs_implants: &BaseAttrs) -> ImplantBonus {
  match set {
    ImplantSet::None => ImplantBonus::default(),
    ImplantSet::Plus3 => ImplantBonus {
      perception: 3,
      memory: 3,
      willpower: 3,
      intelligence: 3,
      charisma: 3,
    },
    ImplantSet::Plus4 => ImplantBonus {
      perception: 4,
      memory: 4,
      willpower: 4,
      intelligence: 4,
      charisma: 4,
    },
    ImplantSet::Plus5 => ImplantBonus {
      perception: 5,
      memory: 5,
      willpower: 5,
      intelligence: 5,
      charisma: 5,
    },
    ImplantSet::Current => current_attrs_implants.clone(),
  }
}

/// Sum the SP demand per (primary, secondary) attribute pair across all
/// entries. Used for fast optimizer passes that avoid re-walking entries.
pub fn pair_weights(entries: &[PlanEntry], attrs: &EffectiveAttrs, skill_groups: &[SkillGroupDef]) -> Vec<PairWeight> {
  let _ = attrs;
  let mut weights: HashMap<(AttrKey, AttrKey), u64> = HashMap::new();
  let mut skill_progress: HashMap<&str, u8> = HashMap::new();

  for entry in entries {
    let Some((skill, _)) = find_skill_in_groups(&entry.skill_name, skill_groups) else {
      continue;
    };

    let starting_level = std::cmp::max(
      skill.level,
      skill_progress.get(entry.skill_name.as_str()).copied().unwrap_or(0),
    );

    if entry.to_level <= starting_level {
      continue;
    }

    let step_sp = if entry.to_level == starting_level + 1 {
      if !skill_progress.contains_key(entry.skill_name.as_str()) && skill.level == starting_level {
        let full_level_sp = sp_cost(skill.rank as f64, entry.to_level);
        (full_level_sp as i64 - skill.sp as i64).max(0) as u64
      } else {
        let cost_to = sp_cost(skill.rank as f64, entry.to_level);
        let cost_from = sp_cost(skill.rank as f64, starting_level);
        cost_to.saturating_sub(cost_from)
      }
    } else {
      sp_cost(skill.rank as f64, entry.to_level)
    };

    *weights.entry((skill.primary, skill.secondary)).or_insert(0) += step_sp;
    skill_progress.insert(entry.skill_name.as_str(), entry.to_level);
  }

  weights
    .into_iter()
    .map(|((primary, secondary), sp)| PairWeight {
      primary,
      secondary,
      sp,
    })
    .collect()
}

/// Total plan time in seconds for a given attribute set and pair weights.
pub fn plan_time_with_attrs(weights: &[PairWeight], attrs: &EffectiveAttrs) -> f64 {
  let mut total = 0.0f64;
  for w in weights {
    let rate = sp_per_sec(attr_value(attrs, w.primary), attr_value(attrs, w.secondary)) as f64;
    if rate <= 0.0 {
      return f64::INFINITY;
    }
    total += w.sp as f64 / rate;
  }
  total
}

/// Brute-force the optimal base attribute distribution. Tests all valid
/// combinations in [17, 27] that sum to `base_total`. Always includes
/// `current_base` as a candidate so the result is never worse than the
/// character's existing allocation.
pub fn optimize_remap(
  entries: &[PlanEntry],
  current_base: &BaseAttrs,
  base_total: i32,
  implant: &ImplantBonus,
  skill_groups: &[SkillGroupDef],
) -> Option<RemapResult> {
  let dummy_attrs = effective_attrs(current_base, implant);
  let weights = pair_weights(entries, &dummy_attrs, skill_groups);
  if weights.is_empty() {
    return None;
  }

  let current_eff = effective_attrs(current_base, implant);
  let current_time = plan_time_with_attrs(&weights, &current_eff);
  let mut best = RemapResult {
    base: current_base.clone(),
    total_sec: current_time,
    current_sec: current_time,
    is_current: true,
  };

  const ATTR_MIN: i32 = 17;
  const ATTR_MAX: i32 = 27;

  for per in ATTR_MIN..=ATTR_MAX {
    for mem in ATTR_MIN..=ATTR_MAX {
      for wil in ATTR_MIN..=ATTR_MAX {
        for intl in ATTR_MIN..=ATTR_MAX {
          let cha = base_total - per - mem - wil - intl;
          if !(ATTR_MIN..=ATTR_MAX).contains(&cha) {
            continue;
          }
          let base = BaseAttrs {
            perception: per,
            memory: mem,
            willpower: wil,
            intelligence: intl,
            charisma: cha,
          };
          let eff = effective_attrs(&base, implant);
          let t = plan_time_with_attrs(&weights, &eff);
          if t < best.total_sec {
            best = RemapResult {
              base,
              total_sec: t,
              current_sec: current_time,
              is_current: false,
            };
          }
        }
      }
    }
  }

  Some(best)
}

/// For each attribute, compute how many seconds would be saved by adding +1
/// to that attribute's implant. Returns results sorted by savings descending.
pub fn compute_implant_savings(
  weights: &[PairWeight],
  base: &BaseAttrs,
  implant: &ImplantBonus,
  current_total_sec: f64,
) -> Vec<ImplantSaving> {
  let mut savings: Vec<ImplantSaving> = AttrKey::ALL
    .iter()
    .filter_map(|&attr| {
      let mut boosted = implant.clone();
      match attr {
        AttrKey::Perception => boosted.perception += 1,
        AttrKey::Memory => boosted.memory += 1,
        AttrKey::Willpower => boosted.willpower += 1,
        AttrKey::Intelligence => boosted.intelligence += 1,
        AttrKey::Charisma => boosted.charisma += 1,
      }
      let eff = effective_attrs(base, &boosted);
      let new_time = plan_time_with_attrs(weights, &eff);
      let saved = current_total_sec - new_time;
      if saved > 0.0 {
        Some(ImplantSaving {
          attr,
          saved_sec: saved,
        })
      } else {
        None
      }
    })
    .collect();

  savings.sort_by(|a, b| {
    b.saved_sec
      .partial_cmp(&a.saved_sec)
      .unwrap_or(std::cmp::Ordering::Equal)
  });
  savings
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_skill_groups() -> Vec<SkillGroupDef> {
    vec![
      SkillGroupDef {
        id: "spaceship".to_string(),
        name: "Spaceship Command".to_string(),
        skills: vec![
          SkillDef {
            type_id: 3327,
            name: "Spaceship Command".to_string(),
            rank: 1,
            level: 0,
            sp: 0,
            primary: AttrKey::Perception,
            secondary: AttrKey::Willpower,
            prereqs: vec![],
          },
          SkillDef {
            type_id: 3334,
            name: "Caldari Cruiser".to_string(),
            rank: 5,
            level: 0,
            sp: 0,
            primary: AttrKey::Perception,
            secondary: AttrKey::Willpower,
            prereqs: vec![("Spaceship Command".to_string(), 3)],
          },
        ],
      },
      SkillGroupDef {
        id: "navigation".to_string(),
        name: "Navigation".to_string(),
        skills: vec![SkillDef {
          type_id: 3449,
          name: "Navigation".to_string(),
          rank: 1,
          level: 0,
          sp: 0,
          primary: AttrKey::Intelligence,
          secondary: AttrKey::Perception,
          prereqs: vec![],
        }],
      },
      SkillGroupDef {
        id: "gunnery".to_string(),
        name: "Gunnery".to_string(),
        skills: vec![SkillDef {
          type_id: 3300,
          name: "Gunnery".to_string(),
          rank: 1,
          level: 0,
          sp: 0,
          primary: AttrKey::Perception,
          secondary: AttrKey::Willpower,
          prereqs: vec![],
        }],
      },
    ]
  }

  fn test_attrs() -> EffectiveAttrs {
    EffectiveAttrs {
      perception: 27,
      memory: 19,
      willpower: 24,
      intelligence: 21,
      charisma: 17,
    }
  }

  #[test]
  fn test_expand_wishes_no_dupes() {
    let groups = test_skill_groups();
    let wishes = &[("Caldari Cruiser", 5u8)];
    let entries = expand_wishes(wishes, &groups);
    let names: Vec<_> = entries.iter().map(|e| (&e.skill_name, e.to_level)).collect();
    for (name, level) in &names {
      let count = names.iter().filter(|(n, l)| n == name && l == level).count();
      assert_eq!(count, 1, "duplicate entry: {} {}", name, level);
    }
  }

  #[test]
  fn test_expand_wishes_auto_flag() {
    let groups = test_skill_groups();
    let wishes = &[("Caldari Cruiser", 5u8)];
    let entries = expand_wishes(wishes, &groups);
    for e in &entries {
      if e.skill_name == "Caldari Cruiser" && e.to_level == 5 {
        assert!(!e.auto, "user-requested entry should have auto=false");
      } else {
        assert!(
          e.auto,
          "prereq entry should have auto=true: {} {}",
          e.skill_name, e.to_level
        );
      }
    }
  }

  #[test]
  fn test_compute_plan_totals() {
    let groups = test_skill_groups();
    let wishes = &[("Gunnery", 1u8)];
    let entries = expand_wishes(wishes, &groups);
    let attrs = test_attrs();
    let plan = compute_plan(&entries, &attrs, &groups);
    assert!(plan.total_sp > 0);
    assert!(plan.total_sec > 0.0);
  }

  #[test]
  fn test_plan_time_with_attrs() {
    let groups = test_skill_groups();
    let wishes = &[("Navigation", 3u8)];
    let entries = expand_wishes(wishes, &groups);
    let attrs = test_attrs();
    let weights = pair_weights(&entries, &attrs, &groups);
    let t = plan_time_with_attrs(&weights, &attrs);
    assert!(t > 0.0);
  }

  #[test]
  fn test_optimize_remap_returns_result() {
    let groups = test_skill_groups();
    let wishes = &[("Navigation", 5u8)];
    let entries = expand_wishes(wishes, &groups);
    let base = BaseAttrs {
      perception: 20,
      memory: 20,
      willpower: 20,
      intelligence: 20,
      charisma: 20,
    };
    let implant = ImplantBonus::default();
    let result = optimize_remap(&entries, &base, 100, &implant, &groups);
    assert!(result.is_some());
  }

  #[test]
  fn test_effective_attrs() {
    let base = BaseAttrs {
      perception: 20,
      memory: 19,
      willpower: 20,
      intelligence: 20,
      charisma: 17,
    };
    let implant = ImplantBonus {
      perception: 5,
      memory: 4,
      willpower: 5,
      intelligence: 4,
      charisma: 3,
    };
    let eff = effective_attrs(&base, &implant);
    assert_eq!(eff.perception, 25);
    assert_eq!(eff.memory, 23);
  }
}
