use chrono::{DateTime, Utc};

use super::{
  format::{sp_cost, sp_per_sec},
  optimizer::{Attribute, Attributes},
};

pub const MAX_SKILL_LEVEL: u8 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpandedEntry {
  pub is_auto: bool,
  pub skill_id: i64,
  pub to_level: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InjectorEstimate {
  pub large: u64,
  pub small: u64,
  pub yield_per: InjectorYield,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InjectorYield {
  pub large: u64,
  pub small: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MilestoneAnchor {
  pub after_entry_id: Option<i64>,
  pub order: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
  pub items: Vec<PlanItem>,
  pub total_sec: f64,
  pub total_sp: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanEntry {
  pub partial_sp_at_from: u64,
  pub primary: Attribute,
  pub rank: f64,
  pub secondary: Attribute,
  pub skill_id: i64,
  pub synced_trained_level: u8,
  pub to_level: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanItem {
  pub cumulative_sec: f64,
  pub cumulative_sp: u64,
  pub eta_secs: f64,
  pub from_level: u8,
  pub sec: f64,
  pub skill_id: i64,
  pub skipped: bool,
  pub sp: u64,
  pub to_level: u8,
}

#[derive(Clone, Debug, Default)]
pub struct PlanOptions {
  pub implant: Option<Attributes>,
  pub remap_points: Vec<RemapPoint>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanSegment {
  pub end: usize,
  pub milestone: Option<usize>,
  pub start: usize,
}

pub type PrereqCatalog = std::collections::HashMap<i64, Vec<(i64, u8)>>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RemapAvailability {
  pub count: u32,
  pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RemapPoint {
  pub after_index: i64,
  pub base: Attributes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wish {
  pub skill_id: i64,
  pub to_level: u8,
}

pub fn remap_availability(
  bonus_remaps: i64,
  last_remap_date: Option<&str>,
  accrued_remap_cooldown_date: Option<&str>,
  now: DateTime<Utc>,
) -> RemapAvailability {
  let _ = last_remap_date;
  let bonus = bonus_remaps.max(0) as u32;

  let cooldown_days_remaining = accrued_remap_cooldown_date
    .and_then(parse_timestamp)
    .map(|date| (date - now).num_days().max(0))
    .unwrap_or(0);
  let annual_available = cooldown_days_remaining == 0;

  let count = u32::from(annual_available) + bonus;
  let reason = if count == 0 {
    t!("skills.plan_math.no_remaps_available", count => cooldown_days_remaining).into_owned()
  } else {
    String::new()
  };

  RemapAvailability {
    count,
    reason,
  }
}

pub fn step_sp(rank: f64, from_level: u8, to_level: u8, partial_sp_at_from: u64) -> i64 {
  let _ = from_level;
  sp_cost(rank, to_level).saturating_sub(partial_sp_at_from) as i64
}

pub fn schedule_skill(
  skill_id: i64,
  to_level: u8,
  catalog: &PrereqCatalog,
  current: &mut std::collections::HashMap<i64, u8>,
  is_prereq: bool,
  out: &mut Vec<ExpandedEntry>,
) {
  let to_level = to_level.min(MAX_SKILL_LEVEL);
  let already = current.get(&skill_id).copied().unwrap_or(0);
  if already >= to_level {
    return;
  }

  current.insert(skill_id, to_level);

  if let Some(prereqs) = catalog.get(&skill_id) {
    for &(prereq_id, prereq_level) in prereqs {
      schedule_skill(prereq_id, prereq_level, catalog, current, true, out);
    }
  }

  for level in (already + 1)..=to_level {
    out.push(ExpandedEntry {
      is_auto: is_prereq,
      skill_id,
      to_level: level,
    });
  }
}

pub fn expand_wishes(
  wishes: &[Wish],
  catalog: &PrereqCatalog,
  trained: &std::collections::HashMap<i64, u8>,
) -> Vec<ExpandedEntry> {
  expand_wishes_from(wishes, catalog, trained.clone())
}

pub fn expand_wishes_full(wishes: &[Wish], catalog: &PrereqCatalog) -> Vec<ExpandedEntry> {
  expand_wishes_from(wishes, catalog, std::collections::HashMap::new())
}

fn expand_wishes_from(
  wishes: &[Wish],
  catalog: &PrereqCatalog,
  mut current: std::collections::HashMap<i64, u8>,
) -> Vec<ExpandedEntry> {
  let mut out: Vec<ExpandedEntry> = Vec::new();

  for wish in wishes {
    schedule_skill(wish.skill_id, wish.to_level, catalog, &mut current, false, &mut out);
  }

  let mut wished_to: std::collections::HashMap<i64, u8> = std::collections::HashMap::new();
  for wish in wishes {
    let level = wish.to_level.min(MAX_SKILL_LEVEL);
    wished_to
      .entry(wish.skill_id)
      .and_modify(|current| *current = (*current).max(level))
      .or_insert(level);
  }
  for entry in &mut out {
    entry.is_auto = wished_to
      .get(&entry.skill_id)
      .map(|&target| entry.to_level > target)
      .unwrap_or(true);
  }

  out
}

fn effective(base: Attributes, implant: Attributes) -> Attributes {
  Attributes {
    charisma: base.charisma + implant.charisma,
    intelligence: base.intelligence + implant.intelligence,
    memory: base.memory + implant.memory,
    perception: base.perception + implant.perception,
    willpower: base.willpower + implant.willpower,
  }
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

fn segment_attrs(remaps: &[RemapPoint], base: Attributes, implant: Option<Attributes>, index: i64) -> Attributes {
  let mut current = base;
  for point in remaps {
    if point.after_index < index {
      current = match implant {
        Some(implant) => effective(point.base, implant),
        None => point.base,
      };
    }
  }
  current
}

fn entry_step_sp(entry: &PlanEntry, prior_scheduled: Option<u8>, starting_level: u8) -> u64 {
  let partial = if prior_scheduled.is_none() && entry.synced_trained_level == starting_level {
    entry.partial_sp_at_from
  } else {
    sp_cost(entry.rank, starting_level)
  };
  step_sp(entry.rank, starting_level, entry.to_level, partial) as u64
}

pub fn compute_plan(entries: &[PlanEntry], current_attrs: Attributes, options: &PlanOptions, now_secs: f64) -> Plan {
  let mut sorted_remaps = options.remap_points.clone();
  sorted_remaps.sort_by_key(|point| point.after_index);

  let mut cumulative_sec = 0.0;
  let mut cumulative_sp: u64 = 0;
  let mut items = Vec::with_capacity(entries.len());
  let mut scheduled: std::collections::HashMap<i64, u8> = std::collections::HashMap::new();

  for (index, entry) in entries.iter().enumerate() {
    let prior_scheduled = scheduled.get(&entry.skill_id).copied();
    let starting_level = entry.synced_trained_level.max(prior_scheduled.unwrap_or(0));
    let skipped = entry.to_level <= starting_level;

    let (sp, sec) = if skipped {
      (0, 0.0)
    } else {
      let sp = entry_step_sp(entry, prior_scheduled, starting_level);
      let segment = segment_attrs(&sorted_remaps, current_attrs, options.implant, index as i64);
      let rate = sp_per_sec(value_of(segment, entry.primary), value_of(segment, entry.secondary));
      (sp, if rate > 0.0 { sp as f64 / rate } else { 0.0 })
    };

    cumulative_sec += sec;
    cumulative_sp = cumulative_sp.saturating_add(sp);
    items.push(PlanItem {
      cumulative_sec,
      cumulative_sp,
      eta_secs: now_secs + cumulative_sec,
      from_level: starting_level,
      sec,
      skill_id: entry.skill_id,
      skipped,
      sp,
      to_level: entry.to_level,
    });
    let next_level = if skipped {
      starting_level.max(entry.to_level)
    } else {
      entry.to_level
    };
    scheduled.insert(entry.skill_id, next_level);
  }

  Plan {
    items,
    total_sec: cumulative_sec,
    total_sp: cumulative_sp,
  }
}

/// Costs a plan for a template, which has no owning character to supply attributes or an implant, so it uses
/// [`Attributes::unmapped`] and no implant as a neutral baseline.
///
/// Must produce the same totals as the editor's own `compute_plan` call for the same entries and remap points.
pub fn template_plan(entries: &[PlanEntry], remap_points: Vec<RemapPoint>) -> Plan {
  let options = PlanOptions {
    implant: None,
    remap_points,
  };
  compute_plan(entries, Attributes::unmapped(), &options, 0.0)
}

pub fn plan_segments(entry_ids: &[i64], milestones: &[MilestoneAnchor]) -> Vec<PlanSegment> {
  let mut placed: Vec<(usize, i64, i64)> = milestones
    .iter()
    .enumerate()
    .filter_map(|(index, milestone)| {
      let anchor = match milestone.after_entry_id {
        None => -1,
        Some(id) => entry_ids.iter().position(|&candidate| candidate == id)? as i64,
      };
      Some((index, anchor, milestone.order))
    })
    .collect();
  placed.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)));

  let total = entry_ids.len();
  let mut segments = Vec::with_capacity(placed.len() + 1);

  let first_start = placed.first().map(|first| (first.1 + 1) as usize).unwrap_or(total);
  if first_start > 0 {
    segments.push(PlanSegment {
      end: first_start.min(total),
      milestone: None,
      start: 0,
    });
  }

  for (position, &(index, anchor, _)) in placed.iter().enumerate() {
    let end = placed
      .get(position + 1)
      .map(|next| (next.1 + 1) as usize)
      .unwrap_or(total);
    segments.push(PlanSegment {
      end: end.min(total),
      milestone: Some(index),
      start: ((anchor + 1) as usize).min(total),
    });
  }

  segments
}

/// A single stored skill-plan step: train `skill_id` up to `to_level`.
///
/// Plans store one row per level (a skill trained to 3 is three rows), so a
/// slice of these mirrors `skill_plan_entries` row-for-row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanStep {
  pub skill_id: i64,
  pub to_level: u8,
}

/// Count the plan steps that still need training for a character.
///
/// This reuses [`compute_plan`]'s skip rule exactly: a step is "remaining" only
/// when its `to_level` exceeds the running level for that skill, where the
/// running level starts at the character's trained level and climbs as earlier
/// steps in the plan schedule it higher. A step at or below that level is a
/// skipped (already-trained) row and is not counted.
///
/// `trained` maps `skill_id` to the character's current trained level; skills
/// absent from the map are treated as untrained (level 0). The result equals
/// the editor's visible "Steps" stat for the same plan and character.
pub fn remaining_steps(steps: &[PlanStep], trained: &std::collections::HashMap<i64, u8>) -> usize {
  let mut scheduled: std::collections::HashMap<i64, u8> = std::collections::HashMap::new();
  let mut remaining = 0;

  for step in steps {
    let trained_level = trained.get(&step.skill_id).copied().unwrap_or(0);
    let prior_scheduled = scheduled.get(&step.skill_id).copied().unwrap_or(0);
    let starting_level = trained_level.max(prior_scheduled);

    if step.to_level > starting_level {
      remaining += 1;
    }
    scheduled.insert(step.skill_id, starting_level.max(step.to_level));
  }

  remaining
}

pub fn distinct_skills(steps: &[PlanStep]) -> usize {
  steps
    .iter()
    .map(|step| step.skill_id)
    .collect::<std::collections::HashSet<_>>()
    .len()
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
  DateTime::parse_from_rfc3339(value)
    .ok()
    .map(|date| date.with_timezone(&Utc))
}

pub fn injector_yield(current_total_sp: u64) -> InjectorYield {
  if current_total_sp < 5_000_000 {
    InjectorYield {
      large: 500_000,
      small: 100_000,
    }
  } else if current_total_sp < 50_000_000 {
    InjectorYield {
      large: 400_000,
      small: 80_000,
    }
  } else if current_total_sp < 80_000_000 {
    InjectorYield {
      large: 300_000,
      small: 60_000,
    }
  } else {
    InjectorYield {
      large: 150_000,
      small: 30_000,
    }
  }
}

pub fn injectors_for_plan(remaining_plan_sp: u64, current_total_sp: u64) -> InjectorEstimate {
  let yield_per = injector_yield(current_total_sp);
  let smalls_per_large = yield_per.large / yield_per.small;
  let small_equivalents = ceil_div(remaining_plan_sp, yield_per.small);

  InjectorEstimate {
    large: small_equivalents / smalls_per_large,
    small: small_equivalents % smalls_per_large,
    yield_per,
  }
}

fn ceil_div(numerator: u64, divisor: u64) -> u64 {
  if divisor == 0 {
    return 0;
  }
  numerator / divisor + u64::from(!numerator.is_multiple_of(divisor))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn attrs(perception: u32, willpower: u32, intelligence: u32, memory: u32, charisma: u32) -> Attributes {
    Attributes {
      charisma,
      intelligence,
      memory,
      perception,
      willpower,
    }
  }

  fn entry(skill_id: i64, to_level: u8) -> PlanEntry {
    PlanEntry {
      primary: Attribute::Perception,
      rank: 1.0,
      secondary: Attribute::Willpower,
      skill_id,
      partial_sp_at_from: 0,
      synced_trained_level: 0,
      to_level,
    }
  }

  mod compute_plan {
    use pretty_assertions::assert_eq;

    use super::*;

    fn opts() -> PlanOptions {
      PlanOptions::default()
    }

    fn fast_attrs() -> Attributes {
      attrs(27, 21, 17, 17, 17)
    }

    #[test]
    fn it_adds_the_implant_to_each_remap_points_base() {
      let implant = attrs(5, 5, 5, 5, 5);
      let entries = [entry(3300, 5), entry(3301, 5)];
      let options = PlanOptions {
        implant: Some(implant),
        remap_points: vec![RemapPoint {
          after_index: 0,
          base: attrs(22, 16, 17, 17, 17),
        }],
      };

      let plan = compute_plan(&entries, attrs(20, 20, 17, 17, 25), &options, 0.0);

      let sec1 = 256_000.0 / sp_per_sec(27, 21);
      assert!((plan.items[1].sec - sec1).abs() < 1e-6);
    }

    #[test]
    fn it_applies_an_initial_segment_remap_before_the_first_entry() {
      let entries = [entry(3300, 5)];
      let options = PlanOptions {
        implant: None,
        remap_points: vec![RemapPoint {
          after_index: -1,
          base: fast_attrs(),
        }],
      };

      let plan = compute_plan(&entries, attrs(17, 17, 17, 17, 31), &options, 0.0);

      assert!((plan.items[0].sec - 256_000.0 / sp_per_sec(27, 21)).abs() < 1e-6);
    }

    #[test]
    fn it_computes_a_single_level_step() {
      let entries = [entry(3300, 5)];
      let plan = compute_plan(&entries, fast_attrs(), &opts(), 1_000.0);

      let rate = sp_per_sec(27, 21);
      let expected_sec = 256_000.0 / rate;

      assert_eq!(plan.items.len(), 1);
      assert_eq!(plan.items[0].sp, 256_000);
      assert!((plan.items[0].sec - expected_sec).abs() < 1e-6);
      assert_eq!(plan.total_sp, 256_000);
      assert!((plan.total_sec - expected_sec).abs() < 1e-6);
      assert!((plan.items[0].eta_secs - (1_000.0 + expected_sec)).abs() < 1e-6);
    }

    #[test]
    fn it_discounts_a_partially_trained_head_step() {
      let mut head = entry(3300, 5);
      head.synced_trained_level = 4;
      head.partial_sp_at_from = 100_000;

      let plan = compute_plan(&[head], fast_attrs(), &opts(), 0.0);

      assert_eq!(plan.items[0].from_level, 4);
      assert_eq!(plan.items[0].sp, 156_000);
    }

    #[test]
    fn it_emits_zero_cost_skipped_rows_for_already_trained_entries() {
      let mut entries = [
        entry(3300, 1),
        entry(3300, 2),
        entry(3300, 3),
        entry(3300, 4),
        entry(3300, 5),
      ];
      for entry in &mut entries {
        entry.synced_trained_level = 3;
      }

      let plan = compute_plan(&entries, fast_attrs(), &opts(), 500.0);

      for skipped in &plan.items[0..3] {
        assert!(skipped.skipped);
        assert_eq!(skipped.sp, 0);
        assert_eq!(skipped.sec, 0.0);
        assert_eq!(skipped.eta_secs, 500.0);
      }
      assert!(!plan.items[3].skipped);
      assert_eq!(plan.items[3].sp, { sp_cost(1.0, 4) - sp_cost(1.0, 3) });
      assert_eq!(plan.items[4].sp, { sp_cost(1.0, 5) - sp_cost(1.0, 4) });
      assert_eq!(plan.total_sp, { sp_cost(1.0, 5) - sp_cost(1.0, 3) });
    }

    #[test]
    fn it_handles_a_prereq_expanded_plan_as_running_level_deltas() {
      let entries = [
        entry(3300, 1),
        entry(3300, 2),
        entry(3300, 3),
        entry(3300, 4),
        entry(3300, 5),
      ];
      let plan = compute_plan(&entries, fast_attrs(), &opts(), 0.0);

      assert_eq!(plan.items[0].sp, { sp_cost(1.0, 1) });
      assert_eq!(plan.items[1].sp, { sp_cost(1.0, 2) - sp_cost(1.0, 1) });
      assert_eq!(plan.items[4].sp, { sp_cost(1.0, 5) - sp_cost(1.0, 4) });
      assert_eq!(plan.total_sp, { sp_cost(1.0, 5) });

      let rate = sp_per_sec(27, 21);
      assert!((plan.total_sec - sp_cost(1.0, 5) as f64 / rate).abs() < 1e-6);
    }

    #[test]
    fn it_handles_multiple_remap_points_picking_the_most_recent() {
      let initial = attrs(18, 18, 17, 17, 29);
      let mid = attrs(22, 20, 17, 17, 23);
      let late = attrs(27, 21, 17, 17, 17);
      let entries = [entry(3300, 5), entry(3301, 5), entry(3302, 5)];
      let options = PlanOptions {
        implant: None,
        remap_points: vec![
          RemapPoint {
            after_index: 1,
            base: late,
          },
          RemapPoint {
            after_index: 0,
            base: mid,
          },
        ],
      };

      let plan = compute_plan(&entries, initial, &options, 0.0);

      let sp = 256_000.0;
      assert!((plan.items[0].sec - sp / sp_per_sec(18, 18)).abs() < 1e-6);
      assert!((plan.items[1].sec - sp / sp_per_sec(22, 20)).abs() < 1e-6);
      assert!((plan.items[2].sec - sp / sp_per_sec(27, 21)).abs() < 1e-6);
    }

    #[test]
    fn it_swaps_to_per_segment_attrs_across_a_remap_point() {
      let slow = attrs(20, 20, 17, 17, 25);
      let fast = fast_attrs();
      let entries = [entry(3300, 5), entry(3301, 5)];
      let options = PlanOptions {
        implant: None,
        remap_points: vec![RemapPoint {
          after_index: 0,
          base: fast,
        }],
      };

      let plan = compute_plan(&entries, slow, &options, 0.0);

      let sp = 256_000.0;
      let sec0 = sp / sp_per_sec(20, 20);
      let sec1 = sp / sp_per_sec(27, 21);
      assert!((plan.items[0].sec - sec0).abs() < 1e-6);
      assert!((plan.items[1].sec - sec1).abs() < 1e-6);
      assert!((plan.total_sec - (sec0 + sec1)).abs() < 1e-6);
    }

    #[test]
    fn it_yields_zero_remaining_for_a_head_banked_above_the_target() {
      let mut head = entry(3300, 5);
      head.synced_trained_level = 4;
      head.partial_sp_at_from = u64::MAX;

      let plan = compute_plan(&[head], fast_attrs(), &opts(), 0.0);

      assert_eq!(plan.items[0].sp, 0, "over-banked head charges zero remaining SP");
      assert_eq!(plan.items[0].sec, 0.0, "zero remaining SP means zero remaining time");
      assert_eq!(plan.total_sp, 0);
      assert_eq!(plan.total_sec, 0.0);
    }

    #[test]
    fn it_yields_zero_seconds_for_an_untrainable_zero_rate_segment() {
      let entries = [entry(3300, 5)];
      let plan = compute_plan(&entries, attrs(0, 0, 0, 0, 0), &opts(), 0.0);

      assert_eq!(plan.items[0].sec, 0.0);
      assert_eq!(plan.items[0].sp, 256_000);
    }
  }

  mod template_plan {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_costs_the_plan_against_the_unmapped_baseline() {
      let entries = [entry(3300, 5), entry(3301, 5)];

      let plan = template_plan(&entries, Vec::new());

      let rate = sp_per_sec(17, 17);
      let expected = 2.0 * 256_000.0 / rate;
      assert!((plan.total_sec - expected).abs() < 1e-6);
    }

    #[test]
    fn it_matches_the_editor_template_code_path() {
      let entries = [entry(3300, 5), entry(3301, 5)];
      let remaps = vec![RemapPoint {
        after_index: 0,
        base: attrs(27, 21, 17, 17, 17),
      }];

      let list = template_plan(&entries, remaps.clone());

      let editor_options = PlanOptions {
        implant: Some(Attributes::default()),
        remap_points: remaps,
      };
      let editor = compute_plan(&entries, Attributes::unmapped(), &editor_options, 0.0);

      assert_eq!(list.total_sec, editor.total_sec);
      assert_eq!(list.total_sp, editor.total_sp);
    }
  }

  mod plan_segments {
    use pretty_assertions::assert_eq;

    use super::*;

    fn anchor(after_entry_id: Option<i64>, order: i64) -> MilestoneAnchor {
      MilestoneAnchor {
        after_entry_id,
        order,
      }
    }

    fn shape(segments: &[PlanSegment]) -> Vec<(Option<usize>, usize, usize)> {
      segments.iter().map(|s| (s.milestone, s.start, s.end)).collect()
    }

    #[test]
    fn it_returns_one_leading_segment_when_there_are_no_milestones() {
      let segments = plan_segments(&[10, 11, 12], &[]);

      assert_eq!(shape(&segments), vec![(None, 0, 3)]);
    }

    #[test]
    fn it_partitions_entries_between_a_start_milestone_and_a_mid_milestone() {
      let entries = [10, 11, 12, 13];
      let milestones = [anchor(None, 0), anchor(Some(11), 0)];

      let segments = plan_segments(&entries, &milestones);

      assert_eq!(shape(&segments), vec![(Some(0), 0, 2), (Some(1), 2, 4)]);
    }

    #[test]
    fn it_keeps_a_leading_null_segment_when_the_first_milestone_is_mid_plan() {
      let entries = [10, 11, 12];
      let milestones = [anchor(Some(10), 0)];

      let segments = plan_segments(&entries, &milestones);

      assert_eq!(shape(&segments), vec![(None, 0, 1), (Some(0), 1, 3)]);
    }

    #[test]
    fn it_drops_a_milestone_anchored_to_a_missing_entry() {
      let entries = [10, 11];
      let milestones = [anchor(Some(999), 0)];

      let segments = plan_segments(&entries, &milestones);

      assert_eq!(shape(&segments), vec![(None, 0, 2)]);
    }

    #[test]
    fn it_orders_milestones_sharing_an_anchor_by_their_order_field() {
      let entries = [10, 11];
      let milestones = [anchor(None, 5), anchor(None, 1)];

      let segments = plan_segments(&entries, &milestones);

      assert_eq!(
        segments.iter().map(|s| s.milestone).collect::<Vec<_>>(),
        vec![Some(1), Some(0)]
      );
    }
  }

  mod expand_wishes {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    fn wish(skill_id: i64, to_level: u8) -> Wish {
      Wish {
        skill_id,
        to_level,
      }
    }

    #[test]
    fn it_expands_a_fresh_wish_into_one_level_steps() {
      let out = expand_wishes(&[wish(3300, 3)], &PrereqCatalog::new(), &HashMap::new());

      assert_eq!(out.len(), 3);
      assert_eq!(out.iter().map(|e| e.to_level).collect::<Vec<_>>(), [1, 2, 3]);
      assert!(out.iter().all(|e| e.skill_id == 3300));
      assert!(out.iter().all(|e| !e.is_auto), "explicit wish levels are not auto");
    }

    #[test]
    fn it_inserts_direct_prerequisites_as_auto_entries_before_the_target() {
      let catalog = PrereqCatalog::from([(3330, vec![(3300, 3)])]);
      let out = expand_wishes(&[wish(3330, 1)], &catalog, &HashMap::new());

      let by_skill: Vec<(i64, u8, bool)> = out.iter().map(|e| (e.skill_id, e.to_level, e.is_auto)).collect();
      assert_eq!(
        by_skill,
        vec![(3300, 1, true), (3300, 2, true), (3300, 3, true), (3330, 1, false)]
      );
    }

    #[test]
    fn it_never_schedules_a_skill_level_twice_across_wishes() {
      let out = expand_wishes(&[wish(3300, 3), wish(3300, 5)], &PrereqCatalog::new(), &HashMap::new());

      assert_eq!(out.iter().map(|e| e.to_level).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn it_only_schedules_levels_above_the_trained_level() {
      let trained = HashMap::from([(3300, 3)]);
      let out = expand_wishes(&[wish(3300, 5)], &PrereqCatalog::new(), &trained);

      assert_eq!(out.iter().map(|e| e.to_level).collect::<Vec<_>>(), [4, 5]);
    }

    #[test]
    fn it_produces_no_entries_when_already_at_or_above_the_target() {
      let trained = HashMap::from([(3300, 5)]);
      let out = expand_wishes(&[wish(3300, 4)], &PrereqCatalog::new(), &trained);

      assert!(out.is_empty(), "an already-trained target schedules nothing");
    }

    #[test]
    fn it_terminates_on_a_malformed_prerequisite_cycle() {
      let catalog = PrereqCatalog::from([(1, vec![(2, 1)]), (2, vec![(1, 1)])]);
      let out = expand_wishes(&[wish(1, 1)], &catalog, &HashMap::new());

      assert_eq!(out.len(), 2);
      assert!(out.iter().any(|e| e.skill_id == 1 && e.to_level == 1));
      assert!(out.iter().any(|e| e.skill_id == 2 && e.to_level == 1));
    }

    #[test]
    fn it_treats_a_prereq_that_is_also_an_explicit_wish_as_not_auto() {
      let catalog = PrereqCatalog::from([(3330, vec![(3300, 3)])]);
      let out = expand_wishes(&[wish(3300, 3), wish(3330, 1)], &catalog, &HashMap::new());

      let three_hundred: Vec<bool> = out.iter().filter(|e| e.skill_id == 3300).map(|e| e.is_auto).collect();
      assert_eq!(
        three_hundred,
        vec![false, false, false],
        "explicit wish wins over the prereq path"
      );
    }
  }

  mod expand_wishes_full {
    use pretty_assertions::assert_eq;

    use super::*;

    fn wish(skill_id: i64, to_level: u8) -> Wish {
      Wish {
        skill_id,
        to_level,
      }
    }

    #[test]
    fn it_expands_every_level_regardless_of_what_is_trained() {
      let out = expand_wishes_full(&[wish(3300, 5)], &PrereqCatalog::new());

      assert_eq!(out.iter().map(|e| e.to_level).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn it_includes_every_prerequisite_level_without_a_trained_filter() {
      let catalog = PrereqCatalog::from([(3330, vec![(3300, 3)])]);
      let out = expand_wishes_full(&[wish(3330, 1)], &catalog);

      let by_skill: Vec<(i64, u8, bool)> = out.iter().map(|e| (e.skill_id, e.to_level, e.is_auto)).collect();
      assert_eq!(
        by_skill,
        vec![(3300, 1, true), (3300, 2, true), (3300, 3, true), (3330, 1, false)]
      );
    }
  }

  mod injector_yield {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_holds_the_second_band_just_below_50m() {
      assert_eq!(
        injector_yield(49_999_999),
        InjectorYield {
          large: 400_000,
          small: 80_000,
        }
      );
    }

    #[test]
    fn it_holds_the_third_band_just_below_80m() {
      assert_eq!(
        injector_yield(79_999_999),
        InjectorYield {
          large: 300_000,
          small: 60_000,
        }
      );
    }

    #[test]
    fn it_steps_to_the_second_band_at_exactly_5m() {
      assert_eq!(
        injector_yield(5_000_000),
        InjectorYield {
          large: 400_000,
          small: 80_000,
        }
      );
    }

    #[test]
    fn it_steps_to_the_third_band_at_exactly_50m() {
      assert_eq!(
        injector_yield(50_000_000),
        InjectorYield {
          large: 300_000,
          small: 60_000,
        }
      );
    }

    #[test]
    fn it_steps_to_the_top_band_at_exactly_80m() {
      assert_eq!(
        injector_yield(80_000_000),
        InjectorYield {
          large: 150_000,
          small: 30_000,
        }
      );
    }

    #[test]
    fn it_uses_the_lowest_band_just_below_5m() {
      assert_eq!(
        injector_yield(4_999_999),
        InjectorYield {
          large: 500_000,
          small: 100_000,
        }
      );
    }
  }

  mod injectors_for_plan {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_bands_on_the_characters_current_total_sp() {
      let estimate = injectors_for_plan(600_000, 100_000_000);

      assert_eq!(
        estimate.large, 4,
        "ceil(600k / 30k) == 20 small-equivalents == 4 larges"
      );
      assert_eq!(estimate.small, 0);
      assert_eq!(
        estimate.yield_per,
        InjectorYield {
          large: 150_000,
          small: 30_000,
        }
      );
    }

    #[test]
    fn it_does_not_overflow_for_a_near_max_remaining_sp() {
      let estimate = injectors_for_plan(u64::MAX, 1_000_000);

      let small_equivalents = u64::MAX / 100_000 + 1;
      assert_eq!(estimate.large, small_equivalents / 5);
      assert_eq!(estimate.small, small_equivalents % 5);
      assert!(estimate.small < 5, "smalls are always capped below one large");
    }

    #[test]
    fn it_does_not_round_up_an_exact_multiple() {
      let estimate = injectors_for_plan(500_000, 1_000_000);

      assert_eq!(estimate.large, 1);
      assert_eq!(estimate.small, 0);
    }

    #[test]
    fn it_fills_the_bulk_with_larges_and_the_remainder_with_smalls() {
      let estimate = injectors_for_plan(600_000, 1_000_000);

      assert_eq!(estimate.large, 1, "one large covers 500k");
      assert_eq!(estimate.small, 1, "one small covers the remaining 100k");
    }

    #[test]
    fn it_never_recommends_five_or_more_smalls() {
      let estimate = injectors_for_plan(450_000, 1_000_000);

      assert_eq!(estimate.large, 1, "five smalls collapse into one large");
      assert_eq!(estimate.small, 0);
    }

    #[test]
    fn it_uses_only_smalls_when_less_than_a_large_remains() {
      let estimate = injectors_for_plan(350_000, 1_000_000);

      assert_eq!(estimate.large, 0);
      assert_eq!(estimate.small, 4, "ceil(350k / 100k) == 4 smalls");
    }

    #[test]
    fn it_yields_zero_of_both_when_nothing_remains_to_train() {
      let estimate = injectors_for_plan(0, 2_000_000);

      assert_eq!(estimate.large, 0);
      assert_eq!(estimate.small, 0);
    }
  }

  mod needed_only_math {
    use pretty_assertions::assert_eq;

    use super::*;

    const CHARACTER_TOTAL_SP: u64 = 10_000_000;

    fn fast_attrs() -> Attributes {
      attrs(27, 21, 17, 17, 17)
    }

    fn trained(skill_id: i64, to_level: u8, synced_trained_level: u8) -> PlanEntry {
      PlanEntry {
        synced_trained_level,
        ..entry(skill_id, to_level)
      }
    }

    #[test]
    fn it_charges_zero_injectors_when_every_step_is_already_trained() {
      let full = [
        trained(3300, 1, 5),
        trained(3300, 2, 5),
        trained(3300, 3, 5),
        trained(3300, 4, 5),
        trained(3300, 5, 5),
      ];

      let plan = compute_plan(&full, fast_attrs(), &PlanOptions::default(), 0.0);
      let estimate = injectors_for_plan(plan.total_sp, CHARACTER_TOTAL_SP);

      assert_eq!(plan.total_sp, 0);
      assert_eq!(estimate.large, 0);
      assert_eq!(estimate.small, 0);
    }

    #[test]
    fn it_matches_the_trimmed_plan_for_injector_time_and_sp() {
      let banked_at_two = sp_cost(1.0, 2);
      let mut head = trained(3300, 3, 2);
      head.partial_sp_at_from = banked_at_two;
      let full = [
        trained(3300, 1, 2),
        trained(3300, 2, 2),
        head,
        trained(3300, 4, 2),
        trained(3300, 5, 2),
      ];
      let mut needed_head = entry(3300, 3);
      needed_head.synced_trained_level = 2;
      needed_head.partial_sp_at_from = banked_at_two;
      let needed_only = [needed_head, entry(3300, 4), entry(3300, 5)];

      let full_plan = compute_plan(&full, fast_attrs(), &PlanOptions::default(), 0.0);
      let needed_plan = compute_plan(&needed_only, fast_attrs(), &PlanOptions::default(), 0.0);

      assert_eq!(full_plan.total_sp, needed_plan.total_sp);
      assert_eq!(full_plan.total_sp, sp_cost(1.0, 5) - sp_cost(1.0, 2));
      assert!((full_plan.total_sec - needed_plan.total_sec).abs() < 1e-9);
      assert_eq!(
        injectors_for_plan(full_plan.total_sp, CHARACTER_TOTAL_SP),
        injectors_for_plan(needed_plan.total_sp, CHARACTER_TOTAL_SP),
      );
    }

    #[test]
    fn it_excludes_trained_levels_from_the_injector_estimate() {
      let full = [
        trained(3300, 1, 4),
        trained(3300, 2, 4),
        trained(3300, 3, 4),
        trained(3300, 4, 4),
        trained(3300, 5, 4),
      ];

      let plan = compute_plan(&full, fast_attrs(), &PlanOptions::default(), 0.0);
      let with_trained = injectors_for_plan(plan.total_sp, CHARACTER_TOTAL_SP);
      let needed_only = injectors_for_plan(sp_cost(1.0, 5) - sp_cost(1.0, 4), CHARACTER_TOTAL_SP);

      assert_eq!(plan.total_sp, sp_cost(1.0, 5) - sp_cost(1.0, 4));
      assert_eq!(with_trained, needed_only);
    }
  }

  mod remap_availability {
    use chrono::TimeZone as _;
    use pretty_assertions::assert_eq;

    use super::*;

    fn now() -> DateTime<Utc> {
      Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap()
    }

    #[test]
    fn it_adds_bonus_remaps_to_the_annual_remap() {
      let availability = remap_availability(2, Some("2025-01-01T00:00:00Z"), Some("2026-01-01T00:00:00Z"), now());

      assert_eq!(availability.count, 3);
      assert!(availability.reason.is_empty());
    }

    #[test]
    fn it_counts_only_bonus_remaps_while_the_annual_is_on_cooldown() {
      let availability = remap_availability(1, Some("2026-04-01T12:00:00Z"), Some("2026-09-01T12:00:00Z"), now());

      assert_eq!(availability.count, 1);
      assert!(availability.reason.is_empty());
    }

    #[test]
    fn it_counts_the_annual_remap_once_the_cooldown_has_passed() {
      let availability = remap_availability(0, Some("2025-01-01T00:00:00Z"), Some("2026-01-01T00:00:00Z"), now());

      assert_eq!(availability.count, 1);
      assert!(availability.reason.is_empty());
    }

    #[test]
    fn it_floors_a_negative_bonus_remap_count_at_zero() {
      let availability = remap_availability(-3, None, Some("2026-09-01T12:00:00Z"), now());

      assert_eq!(availability.count, 0);
      assert!(!availability.reason.is_empty());
    }

    #[test]
    fn it_treats_an_unparsable_cooldown_date_as_available() {
      let availability = remap_availability(0, Some("not-a-date"), Some("garbage"), now());

      assert_eq!(availability.count, 1);
    }

    #[test]
    fn it_treats_none_dates_as_available_without_panicking() {
      let availability = remap_availability(0, None, None, now());

      assert_eq!(availability.count, 1);
      assert!(availability.reason.is_empty());
    }

    #[test]
    fn it_yields_zero_with_a_non_empty_reason_when_on_cooldown_and_no_bonus() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);
      let availability = remap_availability(0, Some("2026-04-01T12:00:00Z"), Some("2026-08-30T12:00:00Z"), now());

      assert_eq!(availability.count, 0);
      assert!(!availability.reason.is_empty());
      assert!(
        availability.reason.contains("90"),
        "reason should name the remaining days: {}",
        availability.reason
      );
    }
  }

  mod single_source_of_truth {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::{
      super::super::attributes::{WeightSkill, queue_pair_weights},
      *,
    };
    use crate::store::model::CharacterSkillqueue;

    const RANK: f64 = 1.0;

    const TO_LEVEL: u8 = 5;

    const BANKED_SP: u64 = 100_000;

    fn queue_entry() -> CharacterSkillqueue {
      CharacterSkillqueue {
        character_id: 42,
        finish_date: None,
        finished_level: TO_LEVEL as i64,
        level_end_sp: None,
        level_start_sp: None,
        queue_position: 0,
        skill_id: 3300,
        start_date: None,
        training_start_sp: None,
      }
    }

    #[test]
    fn compute_plan_and_the_queue_weight_path_charge_equal_per_step_sp() {
      let plan_entry = PlanEntry {
        primary: Attribute::Perception,
        rank: RANK,
        secondary: Attribute::Willpower,
        skill_id: 3300,
        partial_sp_at_from: BANKED_SP,
        synced_trained_level: TO_LEVEL - 1,
        to_level: TO_LEVEL,
      };
      let plan = compute_plan(&[plan_entry], attrs(27, 21, 17, 17, 17), &PlanOptions::default(), 0.0);
      let plan_step_sp = plan.items[0].sp;

      let queue = [queue_entry()];
      let meta = HashMap::from([(
        3300,
        WeightSkill {
          primary: Attribute::Perception,
          rank: RANK,
          secondary: Attribute::Willpower,
          skillpoints_in_skill: BANKED_SP,
        },
      )]);
      let weights = queue_pair_weights(&queue, &meta);

      assert_eq!(weights.len(), 1);
      assert_eq!(plan_step_sp, weights[0].sp);
      assert_eq!(plan_step_sp, 156_000);
    }
  }

  mod remaining_steps {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    fn step(skill_id: i64, to_level: u8) -> PlanStep {
      PlanStep {
        skill_id,
        to_level,
      }
    }

    fn five_levels(skill_id: i64) -> Vec<PlanStep> {
      (1..=5).map(|level| step(skill_id, level)).collect()
    }

    #[test]
    fn it_counts_every_step_for_a_brand_new_character() {
      let steps = five_levels(3300);

      assert_eq!(remaining_steps(&steps, &HashMap::new()), 5);
    }

    #[test]
    fn it_excludes_already_trained_levels() {
      let steps = five_levels(3300);
      let trained = HashMap::from([(3300, 3)]);

      assert_eq!(
        remaining_steps(&steps, &trained),
        2,
        "levels 4 and 5 remain once the skill is trained to 3"
      );
    }

    #[test]
    fn it_yields_zero_for_a_fully_trained_plan() {
      let steps = five_levels(3300);
      let trained = HashMap::from([(3300, 5)]);

      assert_eq!(remaining_steps(&steps, &trained), 0);
    }

    #[test]
    fn it_climbs_the_running_level_across_in_plan_steps() {
      let steps = vec![step(3300, 1), step(3300, 2), step(3300, 2), step(3300, 3)];

      assert_eq!(
        remaining_steps(&steps, &HashMap::new()),
        3,
        "the duplicate level-2 step is already scheduled and does not recount"
      );
    }

    #[test]
    fn it_reflects_per_character_progress_for_the_same_plan() {
      let steps = five_levels(3300);
      let novice = HashMap::from([(3300, 1)]);
      let veteran = HashMap::from([(3300, 4)]);

      assert_ne!(
        remaining_steps(&steps, &novice),
        remaining_steps(&steps, &veteran),
        "two characters with different trained levels see different remaining counts"
      );
      assert_eq!(remaining_steps(&steps, &novice), 4);
      assert_eq!(remaining_steps(&steps, &veteran), 1);
    }

    #[test]
    fn it_matches_the_editor_steps_count_for_identical_inputs() {
      let plan_steps = vec![
        step(3300, 1),
        step(3300, 2),
        step(3300, 3),
        step(3300, 4),
        step(3300, 5),
        step(3301, 1),
        step(3301, 2),
      ];
      let trained = HashMap::from([(3300, 2), (3301, 1)]);

      let plan_entries: Vec<PlanEntry> = plan_steps
        .iter()
        .map(|s| PlanEntry {
          primary: Attribute::Perception,
          rank: 1.0,
          secondary: Attribute::Willpower,
          skill_id: s.skill_id,
          partial_sp_at_from: 0,
          synced_trained_level: trained.get(&s.skill_id).copied().unwrap_or(0),
          to_level: s.to_level,
        })
        .collect();
      let editor_steps = compute_plan(&plan_entries, attrs(27, 21, 17, 17, 17), &PlanOptions::default(), 0.0)
        .items
        .iter()
        .filter(|item| !item.skipped)
        .count();

      assert_eq!(remaining_steps(&plan_steps, &trained), editor_steps);
      assert_eq!(editor_steps, 4, "levels 3,4,5 of 3300 plus level 2 of 3301");
    }
  }

  mod distinct_skills {
    use pretty_assertions::assert_eq;

    use super::*;

    fn step(skill_id: i64, to_level: u8) -> PlanStep {
      PlanStep {
        skill_id,
        to_level,
      }
    }

    #[test]
    fn it_dedupes_levels_of_the_same_skill() {
      let steps = vec![step(3300, 1), step(3300, 2), step(3300, 3)];

      assert_eq!(distinct_skills(&steps), 1, "three levels are one distinct skill");
    }

    #[test]
    fn it_counts_each_distinct_skill_once() {
      let steps = vec![
        step(3300, 1),
        step(3300, 2),
        step(3301, 1),
        step(3302, 1),
        step(3302, 2),
        step(3302, 3),
      ];

      assert_eq!(distinct_skills(&steps), 3);
    }

    #[test]
    fn it_is_zero_for_an_empty_plan() {
      assert_eq!(distinct_skills(&[]), 0);
    }

    #[test]
    fn the_distinct_count_is_below_the_stored_slot_count_for_an_expanded_plan() {
      let steps = vec![
        step(3300, 1),
        step(3300, 2),
        step(3300, 3),
        step(3301, 1),
        step(3301, 2),
        step(3301, 3),
      ];

      assert!(distinct_skills(&steps) < steps.len());
      assert_eq!(distinct_skills(&steps), 2);
    }
  }

  mod step_sp {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_discounts_a_partially_trained_head_by_its_banked_sp() {
      assert_eq!(step_sp(1.0, 4, 5, 100_000), 156_000);
    }

    #[test]
    fn it_floors_an_over_banked_head_at_zero() {
      assert_eq!(step_sp(1.0, 4, 5, 300_000), 0);
    }

    #[test]
    fn it_saturates_against_a_huge_banked_value_without_wrapping() {
      assert_eq!(step_sp(1.0, 4, 5, u64::MAX), 0);
      assert_eq!(step_sp(1.0, 0, 5, i64::MAX as u64 + 1), 0);
    }

    #[test]
    fn it_scales_with_rank() {
      assert_eq!(step_sp(2.0, 4, 5, sp_cost(2.0, 4)), 2 * (256_000 - 45_255));
    }

    #[test]
    fn it_yields_the_full_target_cost_with_no_banked_sp() {
      assert_eq!(step_sp(1.0, 0, 5, 0), 256_000);
    }

    #[test]
    fn it_yields_the_level_delta_when_passed_the_from_level_cost() {
      assert_eq!(step_sp(1.0, 4, 5, sp_cost(1.0, 4)), 256_000 - 45_255);
    }
  }
}
