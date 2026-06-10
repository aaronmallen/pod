use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::{
  format::sp_per_sec,
  queue_timing::{active_queue, queue_entry_progress, sp_for_range},
};
use crate::store::{
  Database,
  model::CharacterSkillqueue,
  repo::{character, sde, skills},
};

const LOW_QUEUE_THRESHOLD_SECS: f64 = 86_400.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Attr {
  Charisma,
  Intelligence,
  Memory,
  Perception,
  Willpower,
}

impl Attr {
  pub fn from_neural_id(id: i64) -> Self {
    match id {
      164 => Attr::Charisma,
      165 => Attr::Intelligence,
      166 => Attr::Memory,
      168 => Attr::Willpower,
      _ => Attr::Perception,
    }
  }

  pub fn short(self) -> &'static str {
    match self {
      Attr::Charisma => "Cha",
      Attr::Intelligence => "Int",
      Attr::Memory => "Mem",
      Attr::Perception => "Per",
      Attr::Willpower => "Wil",
    }
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AttrValues {
  charisma: u32,
  intelligence: u32,
  memory: u32,
  perception: u32,
  willpower: u32,
}

impl AttrValues {
  pub fn get(&self, attr: Attr) -> u32 {
    match attr {
      Attr::Charisma => self.charisma,
      Attr::Intelligence => self.intelligence,
      Attr::Memory => self.memory,
      Attr::Perception => self.perception,
      Attr::Willpower => self.willpower,
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComputedQueue {
  pub items: Vec<ComputedQueueItem>,
  pub sp_rate: f64,
  pub total_secs: f64,
  pub total_sp: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputedQueueItem {
  pub cum_start_secs: f64,
  pub duration_secs: f64,
  pub from_level: u8,
  pub group_name: String,
  pub primary: Attr,
  pub progress: f32,
  pub rank: u8,
  pub secondary: Attr,
  pub skill_name: String,
  pub sp_needed: u64,
  pub sp_now: u64,
  pub sp_to: u64,
  pub to_level: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueWarning {
  Idle,
  LowQueue,
}

impl QueueWarning {
  pub fn message(self) -> &'static str {
    match self {
      QueueWarning::Idle => "Training inactive \u{b7} no skill is currently training",
      QueueWarning::LowQueue => "Low queue \u{b7} less than 24h of training remains",
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkillMeta {
  pub group_name: String,
  pub primary: Attr,
  pub rank: u8,
  pub secondary: Attr,
  pub skill_name: String,
  pub sp_base: u64,
}

struct QueueItemSp {
  progress: f32,
  sp_needed: u64,
  sp_now: u64,
  sp_to: u64,
}

pub fn active_attr_pair(
  head_skill_id: Option<i64>,
  active_skill_id: Option<i64>,
  skill_meta: &HashMap<i64, SkillMeta>,
) -> (Attr, Attr) {
  head_skill_id
    .and_then(|id| skill_meta.get(&id))
    .or_else(|| active_skill_id.and_then(|id| skill_meta.get(&id)))
    .map(|m| (m.primary, m.secondary))
    .unwrap_or((Attr::Perception, Attr::Willpower))
}

pub fn compute_queue(
  queue: &[(CharacterSkillqueue, f32)],
  sp_rate: f64,
  skill_meta: &HashMap<i64, SkillMeta>,
) -> Vec<ComputedQueueItem> {
  let mut cursor = 0.0;
  let mut result = Vec::with_capacity(queue.len());

  for (index, (entry, progress)) in queue.iter().enumerate() {
    let to_level = entry.finished_level().clamp(0, 5) as u8;
    let from_level = entry.finished_level().saturating_sub(1).clamp(0, 5) as u8;

    let meta = skill_meta.get(&entry.skill_id());
    let rank = meta.map_or(1, |m| m.rank.max(1));
    let primary = meta.map_or(Attr::Perception, |m| m.primary);
    let secondary = meta.map_or(Attr::Willpower, |m| m.secondary);
    let group_name = meta.map(|m| m.group_name.clone()).unwrap_or_default();
    let skill_name = meta.map(|m| m.skill_name.clone()).unwrap_or_default();
    let sp_base = meta.map_or(0, |m| m.sp_base);

    let sp = compute_queue_item_sp(*progress, rank, from_level, to_level, sp_base, index == 0);
    let duration_secs = if sp_rate > 0.0 {
      sp.sp_needed as f64 / sp_rate
    } else {
      0.0
    };
    let cum_start_secs = cursor;
    cursor += duration_secs;

    result.push(ComputedQueueItem {
      cum_start_secs,
      duration_secs,
      from_level,
      group_name,
      primary,
      progress: sp.progress,
      rank,
      secondary,
      skill_name,
      sp_needed: sp.sp_needed,
      sp_now: sp.sp_now,
      sp_to: sp.sp_to,
      to_level,
    });
  }

  result
}

pub fn compute_sp_rate(pair: (Attr, Attr), attrs: &AttrValues) -> f64 {
  sp_per_sec(attrs.get(pair.0), attrs.get(pair.1))
}

pub async fn load_computed_queue(db: &Database, character_id: i64, now: DateTime<Utc>) -> ComputedQueue {
  let queue = active_queue(character::skillqueue(db, character_id).await.unwrap_or_default(), now);

  let attrs = effective_attr_values(db, character_id).await;
  let skill_meta = resolve_skill_meta(db, character_id, &queue).await;

  let head_skill_id = queue.first().map(CharacterSkillqueue::skill_id);
  let active_skill_id = character::current_skillqueue(db, character_id, now)
    .await
    .ok()
    .flatten()
    .map(|entry| entry.skill_id());
  let pair = active_attr_pair(head_skill_id, active_skill_id, &skill_meta);
  let sp_rate = compute_sp_rate(pair, &attrs);

  let with_progress: Vec<(CharacterSkillqueue, f32)> = queue
    .into_iter()
    .map(|entry| {
      let progress = queue_entry_progress(&entry, now);
      (entry, progress)
    })
    .collect();

  let items = compute_queue(&with_progress, sp_rate, &skill_meta);
  let total_secs = items
    .last()
    .map_or(0.0, |item| item.cum_start_secs + item.duration_secs);

  let total_sp = character::state(db, character_id)
    .await
    .ok()
    .flatten()
    .and_then(|state| state.total_sp)
    .unwrap_or(0);

  ComputedQueue {
    items,
    sp_rate,
    total_secs,
    total_sp,
  }
}

fn compute_queue_item_sp(
  progress: f32,
  rank: u8,
  from_level: u8,
  to_level: u8,
  sp_base: u64,
  is_first: bool,
) -> QueueItemSp {
  if is_first {
    let total = sp_for_range(rank, from_level, to_level);
    let done = (total as f32 * progress.clamp(0.0, 1.0)) as u64;
    QueueItemSp {
      progress,
      sp_needed: total.saturating_sub(done),
      sp_now: sp_base.saturating_add(done),
      sp_to: sp_base.saturating_add(total),
    }
  } else {
    let needed = sp_for_range(rank, from_level, to_level);
    QueueItemSp {
      progress: 0.0,
      sp_needed: needed,
      sp_now: sp_base,
      sp_to: sp_base.saturating_add(needed),
    }
  }
}

async fn effective_attr_values(db: &Database, character_id: i64) -> AttrValues {
  let base = character::attributes(db, character_id).await.ok().flatten();
  let mut values = base.as_ref().map_or_else(AttrValues::default, |row| AttrValues {
    charisma: row.charisma().max(0) as u32,
    intelligence: row.intelligence().max(0) as u32,
    memory: row.memory().max(0) as u32,
    perception: row.perception().max(0) as u32,
    willpower: row.willpower().max(0) as u32,
  });

  let implants = character::implants(db, character_id).await.unwrap_or_default();
  for implant in implants {
    let bonus = implant.bonus().max(0) as u32;
    match Attr::from_neural_id(implant.attribute_id()) {
      Attr::Charisma => values.charisma += bonus,
      Attr::Intelligence => values.intelligence += bonus,
      Attr::Memory => values.memory += bonus,
      Attr::Perception => values.perception += bonus,
      Attr::Willpower => values.willpower += bonus,
    }
  }

  values
}

async fn resolve_skill_meta(
  db: &Database,
  character_id: i64,
  queue: &[CharacterSkillqueue],
) -> HashMap<i64, SkillMeta> {
  let sheet = character::skills(db, character_id).await.unwrap_or_default();
  let sp_by_skill: HashMap<i64, u64> = sheet
    .into_iter()
    .map(|skill| (skill.skill_id(), skill.skillpoints_in_skill().max(0) as u64))
    .collect();

  let mut meta = HashMap::new();
  for skill_id in queue.iter().map(CharacterSkillqueue::skill_id) {
    if meta.contains_key(&skill_id) {
      continue;
    }
    let Some(item_type) = sde::get_item_type(db, skill_id).await.ok().flatten() else {
      continue;
    };

    let skill_name = item_type.name().to_owned();
    let group_name = sde::get_item_group(db, item_type.group_id())
      .await
      .ok()
      .flatten()
      .map(|g| g.name().to_owned())
      .unwrap_or_default();

    let row = skills::get_skill_metadata(db, skill_id).await.ok().flatten();
    let rank = row.as_ref().map_or(1, |r| r.rank().clamp(1, i64::from(u8::MAX)) as u8);
    let primary = row
      .as_ref()
      .map_or(Attr::Perception, |r| Attr::from_neural_id(r.primary_attribute()));
    let secondary = row
      .as_ref()
      .map_or(Attr::Willpower, |r| Attr::from_neural_id(r.secondary_attribute()));

    meta.insert(
      skill_id,
      SkillMeta {
        group_name,
        primary,
        rank,
        secondary,
        skill_name,
        sp_base: sp_by_skill.get(&skill_id).copied().unwrap_or(0),
      },
    );
  }

  meta
}

pub fn is_idle(head: Option<&CharacterSkillqueue>) -> bool {
  head.is_none_or(|entry| entry.start_date().is_none() || entry.finish_date().is_none())
}

pub fn queue_warnings(computed: &ComputedQueue, idle: bool) -> Vec<QueueWarning> {
  if idle {
    return vec![QueueWarning::Idle];
  }
  if computed.total_secs > 0.0 && computed.total_secs < LOW_QUEUE_THRESHOLD_SECS {
    return vec![QueueWarning::LowQueue];
  }
  Vec::new()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn entry(queue_position: i64, skill_id: i64, finished_level: i64) -> CharacterSkillqueue {
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

  fn meta(rank: u8, primary: Attr, secondary: Attr, sp_base: u64) -> SkillMeta {
    SkillMeta {
      group_name: "Gunnery".to_owned(),
      primary,
      rank,
      secondary,
      skill_name: "Test Skill".to_owned(),
      sp_base,
    }
  }

  fn now() -> DateTime<Utc> {
    use chrono::TimeZone as _;

    Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap()
  }

  async fn seed_skill_ref(db: &Database, skill_id: i64, group_id: i64, group_name: &str, skill_name: &str) {
    use crate::store::model::{ItemCategory, ItemGroup, ItemType};

    sde::upsert_item_category(
      db,
      &ItemCategory {
        icon_id: None,
        id: 16,
        name: "Skill".to_owned(),
        published: true,
      },
    )
    .await
    .unwrap();
    sde::upsert_item_group(
      db,
      &ItemGroup {
        category_id: 16,
        icon_id: None,
        id: group_id,
        name: group_name.to_owned(),
        published: true,
      },
    )
    .await
    .unwrap();
    sde::upsert_item_type(
      db,
      &ItemType {
        capacity: None,
        description: Some("Test skill".to_owned()),
        dogma_attributes: "[]".to_owned(),
        group_id,
        icon_id: None,
        id: skill_id,
        market_group_id: None,
        name: skill_name.to_owned(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        volume: None,
      },
    )
    .await
    .unwrap();
  }

  mod active_attr_pair {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_prefers_the_head_skill_pair() {
      let mut skill_meta = HashMap::new();
      skill_meta.insert(100, meta(1, Attr::Intelligence, Attr::Memory, 0));
      skill_meta.insert(200, meta(1, Attr::Charisma, Attr::Willpower, 0));

      assert_eq!(
        active_attr_pair(Some(100), Some(200), &skill_meta),
        (Attr::Intelligence, Attr::Memory)
      );
    }

    #[test]
    fn it_falls_back_to_the_active_training_skill_when_the_head_is_unknown() {
      let mut skill_meta = HashMap::new();
      skill_meta.insert(200, meta(1, Attr::Charisma, Attr::Willpower, 0));

      assert_eq!(
        active_attr_pair(Some(999), Some(200), &skill_meta),
        (Attr::Charisma, Attr::Willpower)
      );
    }

    #[test]
    fn it_falls_back_to_perception_willpower_when_nothing_resolves() {
      assert_eq!(
        active_attr_pair(None, None, &HashMap::new()),
        (Attr::Perception, Attr::Willpower)
      );
      assert_eq!(
        active_attr_pair(Some(1), Some(2), &HashMap::new()),
        (Attr::Perception, Attr::Willpower)
      );
    }
  }

  mod compute_queue {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_yields_an_empty_vec_for_an_empty_queue() {
      let computed = compute_queue(&[], 1.0, &HashMap::new());

      assert!(computed.is_empty());
    }

    #[test]
    fn it_accumulates_cumulative_offsets_and_a_running_total() {
      let queue = vec![
        (entry(0, 100, 5), 1.0),
        (entry(1, 101, 1), 0.0),
        (entry(2, 102, 2), 0.0),
      ];
      let mut skill_meta = HashMap::new();
      skill_meta.insert(100, meta(1, Attr::Perception, Attr::Willpower, 0));
      skill_meta.insert(101, meta(1, Attr::Perception, Attr::Willpower, 0));
      skill_meta.insert(102, meta(1, Attr::Perception, Attr::Willpower, 0));

      let computed = compute_queue(&queue, 1.0, &skill_meta);

      assert_eq!(computed.len(), 3);
      assert_eq!(computed[0].cum_start_secs, 0.0);
      assert_eq!(computed[0].duration_secs, 0.0);
      assert_eq!(computed[1].cum_start_secs, 0.0);
      assert_eq!(computed[1].duration_secs, 250.0);
      assert_eq!(computed[2].cum_start_secs, 250.0);
      assert_eq!(computed[2].duration_secs, 1_414.0);

      let total = computed.last().map(|i| i.cum_start_secs + i.duration_secs).unwrap();
      assert_eq!(total, 250.0 + 1_414.0);
    }

    #[test]
    fn it_treats_the_head_entry_progress_and_sp_differently_from_later_entries() {
      let queue = vec![(entry(0, 100, 5), 0.5), (entry(1, 100, 5), 0.0)];
      let mut skill_meta = HashMap::new();
      skill_meta.insert(100, meta(1, Attr::Perception, Attr::Willpower, 1_000_000));

      let computed = compute_queue(&queue, 1.0, &skill_meta);

      assert_eq!(computed[0].progress, 0.5);
      assert_eq!(computed[0].sp_needed, 128_000);
      assert_eq!(computed[0].sp_now, 1_000_000 + 128_000);
      assert_eq!(computed[0].sp_to, 1_000_000 + 256_000);
      assert_eq!(computed[1].progress, 0.0);
      assert_eq!(computed[1].sp_needed, 256_000);
      assert_eq!(computed[1].sp_now, 1_000_000);
      assert_eq!(computed[1].sp_to, 1_000_000 + 256_000);
    }

    #[test]
    fn it_falls_back_to_rank_one_perception_willpower_when_metadata_is_absent() {
      let queue = vec![(entry(0, 100, 1), 0.0), (entry(1, 101, 5), 0.0)];

      let computed = compute_queue(&queue, 1.0, &HashMap::new());

      assert_eq!(computed.len(), 2);
      for item in &computed {
        assert_eq!(item.rank, 1);
        assert_eq!(item.primary, Attr::Perception);
        assert_eq!(item.secondary, Attr::Willpower);
        assert_eq!(item.group_name, "");
        assert_eq!(item.skill_name, "");
      }
      assert_eq!(computed[0].sp_needed, 250);
      assert_eq!(computed[1].sp_needed, 256_000);
      assert_eq!(computed[1].cum_start_secs, 250.0);
    }

    #[test]
    fn it_zeroes_durations_when_the_sp_rate_is_unknown() {
      let queue = vec![(entry(0, 100, 1), 0.0)];

      let computed = compute_queue(&queue, 0.0, &HashMap::new());

      assert_eq!(computed[0].duration_secs, 0.0);
      assert_eq!(computed[0].sp_needed, 250);
    }

    #[test]
    fn it_yields_zero_remaining_and_zero_duration_for_a_complete_or_over_progressed_head() {
      let mut skill_meta = HashMap::new();
      skill_meta.insert(100, meta(1, Attr::Perception, Attr::Willpower, 1_000_000));

      for progress in [1.0_f32, 2.5, f32::INFINITY] {
        let computed = compute_queue(&[(entry(0, 100, 5), progress)], 1.0, &skill_meta);

        assert_eq!(computed[0].sp_needed, 0, "progress {progress} leaves zero remaining SP");
        assert_eq!(
          computed[0].duration_secs, 0.0,
          "progress {progress} leaves zero duration"
        );
        assert!(computed[0].sp_now <= computed[0].sp_to);
      }
    }
  }

  mod resolve_skill_meta {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{ItemCategory, ItemGroup, ItemType, SkillMetadata},
      repo::{sde, skills},
    };

    fn skill_item_type(id: i64, group_id: i64, name: &str) -> ItemType {
      ItemType {
        capacity: None,
        description: Some("Test skill".to_owned()),
        dogma_attributes: "[]".to_owned(),
        group_id,
        icon_id: None,
        id,
        market_group_id: None,
        name: name.to_owned(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        volume: None,
      }
    }

    async fn seed_skill(db: &Database, skill_id: i64, group_id: i64, group_name: &str, skill_name: &str) {
      sde::upsert_item_category(
        db,
        &ItemCategory {
          icon_id: None,
          id: 16,
          name: "Skill".to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
      sde::upsert_item_group(
        db,
        &ItemGroup {
          category_id: 16,
          icon_id: None,
          id: group_id,
          name: group_name.to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
      sde::upsert_item_type(db, &skill_item_type(skill_id, group_id, skill_name))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_resolves_the_name_and_applies_defaults_when_metadata_is_absent() {
      let db = store::open_test().await.unwrap();
      seed_skill(&db, 3300, 255, "Gunnery", "Small Hybrid Turret").await;

      let queue = vec![entry(0, 3300, 1)];
      let meta = super::resolve_skill_meta(&db, 42, &queue).await;

      let resolved = meta.get(&3300).expect("skill must be present even without metadata");
      assert_eq!(resolved.skill_name, "Small Hybrid Turret");
      assert_eq!(resolved.group_name, "Gunnery");
      assert_eq!(resolved.rank, 1);
      assert_eq!(resolved.primary, Attr::Perception);
      assert_eq!(resolved.secondary, Attr::Willpower);
      assert_eq!(resolved.sp_base, 0);
    }

    #[tokio::test]
    async fn it_reads_rank_and_attributes_from_metadata_when_present() {
      let db = store::open_test().await.unwrap();
      seed_skill(&db, 3301, 255, "Gunnery", "Gunnery").await;
      skills::upsert_skill_metadata(
        &db,
        &SkillMetadata {
          primary_attribute: 165,
          rank: 3,
          secondary_attribute: 166,
          skill_id: 3301,
        },
      )
      .await
      .unwrap();

      let queue = vec![entry(0, 3301, 1)];
      let meta = super::resolve_skill_meta(&db, 42, &queue).await;

      let resolved = meta.get(&3301).expect("skill must be present");
      assert_eq!(resolved.skill_name, "Gunnery");
      assert_eq!(resolved.rank, 3);
      assert_eq!(resolved.primary, Attr::Intelligence);
      assert_eq!(resolved.secondary, Attr::Memory);
    }
  }

  mod compute_sp_rate {
    use super::*;

    #[test]
    fn it_uses_effective_values_and_does_not_clamp() {
      let attrs = AttrValues {
        charisma: 17,
        intelligence: 21,
        memory: 19,
        perception: 32,
        willpower: 29,
      };

      let rate = compute_sp_rate((Attr::Perception, Attr::Willpower), &attrs);

      assert!((rate - sp_per_sec(32, 29)).abs() < 1e-9, "got {rate}");
      let base = AttrValues {
        perception: 27,
        willpower: 24,
        ..attrs
      };
      assert!(rate > compute_sp_rate((Attr::Perception, Attr::Willpower), &base));
    }
  }

  mod from_neural_id {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_the_five_neural_ids() {
      assert_eq!(Attr::from_neural_id(164), Attr::Charisma);
      assert_eq!(Attr::from_neural_id(165), Attr::Intelligence);
      assert_eq!(Attr::from_neural_id(166), Attr::Memory);
      assert_eq!(Attr::from_neural_id(167), Attr::Perception);
      assert_eq!(Attr::from_neural_id(168), Attr::Willpower);
    }

    #[test]
    fn it_degrades_unknown_ids_to_perception() {
      assert_eq!(Attr::from_neural_id(0), Attr::Perception);
      assert_eq!(Attr::from_neural_id(999), Attr::Perception);
    }
  }

  mod is_idle {
    use super::*;

    fn dated() -> CharacterSkillqueue {
      CharacterSkillqueue {
        finish_date: Some("2026-06-11T00:00:00Z".to_owned()),
        start_date: Some("2026-06-01T00:00:00Z".to_owned()),
        ..entry(0, 100, 5)
      }
    }

    #[test]
    fn it_reports_active_for_a_fully_dated_head() {
      assert!(!is_idle(Some(&dated())));
    }

    #[test]
    fn it_reports_idle_for_a_head_missing_either_date() {
      assert!(is_idle(Some(&entry(0, 100, 5))));
      let only_start = CharacterSkillqueue {
        start_date: Some("2026-06-01T00:00:00Z".to_owned()),
        ..entry(0, 100, 5)
      };
      assert!(is_idle(Some(&only_start)));
    }

    #[test]
    fn it_reports_idle_with_no_head() {
      assert!(is_idle(None));
    }
  }

  mod no_queue_mutation_messages {
    #[test]
    fn the_skills_feature_declares_no_reorder_remove_or_add_message() {
      const SOURCES: [(&str, &str); 33] = [
        ("skills.rs", include_str!("../skills.rs")),
        ("skills/queue.rs", include_str!("../skills/queue.rs")),
        ("skills/queue_timing.rs", include_str!("../skills/queue_timing.rs")),
        ("skills/format.rs", include_str!("../skills/format.rs")),
        ("skills/browse.rs", include_str!("../skills/browse.rs")),
        ("skills/attributes.rs", include_str!("../skills/attributes.rs")),
        ("skills/right_panel.rs", include_str!("../skills/right_panel.rs")),
        (
          "skills/right_panel/browser_tab.rs",
          include_str!("../skills/right_panel/browser_tab.rs"),
        ),
        (
          "skills/right_panel/browser_tab/group_header.rs",
          include_str!("../skills/right_panel/browser_tab/group_header.rs"),
        ),
        (
          "skills/right_panel/browser_tab/skill_row.rs",
          include_str!("../skills/right_panel/browser_tab/skill_row.rs"),
        ),
        (
          "skills/right_panel/browser_tab/search_bar.rs",
          include_str!("../skills/right_panel/browser_tab/search_bar.rs"),
        ),
        (
          "skills/right_panel/attributes_tab.rs",
          include_str!("../skills/right_panel/attributes_tab.rs"),
        ),
        (
          "skills/right_panel/attributes_tab/attr_row.rs",
          include_str!("../skills/right_panel/attributes_tab/attr_row.rs"),
        ),
        (
          "skills/right_panel/attributes_tab/rate_grid.rs",
          include_str!("../skills/right_panel/attributes_tab/rate_grid.rs"),
        ),
        (
          "skills/right_panel/attributes_tab/remap_card.rs",
          include_str!("../skills/right_panel/attributes_tab/remap_card.rs"),
        ),
        (
          "skills/right_panel/attributes_tab/section_header.rs",
          include_str!("../skills/right_panel/attributes_tab/section_header.rs"),
        ),
        (
          "skills/right_panel/plans_tab.rs",
          include_str!("../skills/right_panel/plans_tab.rs"),
        ),
        (
          "skills/right_panel/plans_tab/plan_card.rs",
          include_str!("../skills/right_panel/plans_tab/plan_card.rs"),
        ),
        (
          "skills/right_panel/plans_tab/new_plan_button.rs",
          include_str!("../skills/right_panel/plans_tab/new_plan_button.rs"),
        ),
        (
          "skills/right_panel/plans_tab/from_queue_button.rs",
          include_str!("../skills/right_panel/plans_tab/from_queue_button.rs"),
        ),
        (
          "skills/right_panel/plans_tab/empty_state.rs",
          include_str!("../skills/right_panel/plans_tab/empty_state.rs"),
        ),
        ("skills/training_hero.rs", include_str!("../skills/training_hero.rs")),
        (
          "skills/training_hero/active.rs",
          include_str!("../skills/training_hero/active.rs"),
        ),
        (
          "skills/training_hero/idle.rs",
          include_str!("../skills/training_hero/idle.rs"),
        ),
        (
          "skills/training_hero/queue_item.rs",
          include_str!("../skills/training_hero/queue_item.rs"),
        ),
        (
          "skills/training_hero/pip_row.rs",
          include_str!("../skills/training_hero/pip_row.rs"),
        ),
        (
          "skills/training_hero/right_col.rs",
          include_str!("../skills/training_hero/right_col.rs"),
        ),
        ("skills/queue_section.rs", include_str!("../skills/queue_section.rs")),
        (
          "skills/queue_section/col_header.rs",
          include_str!("../skills/queue_section/col_header.rs"),
        ),
        (
          "skills/queue_section/row.rs",
          include_str!("../skills/queue_section/row.rs"),
        ),
        (
          "skills/queue_section/footer.rs",
          include_str!("../skills/queue_section/footer.rs"),
        ),
        (
          "skills/queue_section/empty_state.rs",
          include_str!("../skills/queue_section/empty_state.rs"),
        ),
        ("skills/warning_strip.rs", include_str!("../skills/warning_strip.rs")),
      ];

      let forbidden: Vec<String> = vec![
        format!("{}{}", "Re", "order"),
        format!("{}{}", "Re", "ordered"),
        format!("{}{}", "Re", "move"),
        format!("{}{}", "Re", "moved"),
        format!("{}{}{}", "Add", "Sk", "ill"),
        format!("{}{}{}", "Queue", "A", "dd"),
        format!("{}{}{}", "Queue", "Re", "move"),
        format!("{}{}{}", "Queue", "Re", "order"),
      ];

      let test_marker = format!("#[cfg({})]", "test");
      for (name, source) in SOURCES {
        let production = source.split(&test_marker).next().unwrap_or(source);
        for needle in &forbidden {
          assert!(
            !production.contains(needle.as_str()),
            "found a forbidden queue-mutation token `{needle}` in {name}; the live queue is read-only \
            (no ESI write endpoint) and must carry no reorder/remove/add message"
          );
        }
      }
    }
  }

  mod queue_warnings {
    use pretty_assertions::assert_eq;

    use super::*;

    fn computed(total_secs: f64, item_count: usize) -> ComputedQueue {
      let items = (0..item_count)
        .map(|i| ComputedQueueItem {
          cum_start_secs: 0.0,
          duration_secs: total_secs,
          from_level: 0,
          group_name: String::new(),
          primary: Attr::Perception,
          progress: 0.0,
          rank: 1,
          secondary: Attr::Willpower,
          skill_name: format!("Skill {i}"),
          sp_needed: 0,
          sp_now: 0,
          sp_to: 0,
          to_level: 1,
        })
        .collect();
      ComputedQueue {
        items,
        sp_rate: 1.0,
        total_secs,
        total_sp: 0,
      }
    }

    #[test]
    fn it_surfaces_the_low_queue_warning_under_24h() {
      let warnings = queue_warnings(&computed(23.0 * 3_600.0, 1), false);

      assert_eq!(warnings, vec![QueueWarning::LowQueue]);
    }

    #[test]
    fn it_does_not_warn_for_a_healthy_queue() {
      let warnings = queue_warnings(&computed(48.0 * 3_600.0, 2), false);

      assert!(warnings.is_empty());
    }

    #[test]
    fn it_surfaces_the_idle_warning_and_suppresses_low_queue() {
      let warnings = queue_warnings(&computed(1.0 * 3_600.0, 1), true);

      assert_eq!(warnings, vec![QueueWarning::Idle]);
    }

    #[test]
    fn it_never_warns_for_a_zero_duration_active_queue() {
      let warnings = queue_warnings(&computed(0.0, 0), false);

      assert!(warnings.is_empty());
    }
  }

  async fn seed_character(db: &Database, id: i64) {
    use crate::store::model::{Alliance, Bloodline, Character, Corporation, Gender, Race};

    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Test Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  mod effective_attr_values {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{CharacterAttributes, CharacterImplant};

    fn attributes(character_id: i64) -> CharacterAttributes {
      CharacterAttributes {
        accrued_remap_cooldown_date: None,
        bonus_remaps: 0,
        character_id,
        charisma: 19,
        intelligence: 20,
        last_remap_date: None,
        memory: 21,
        perception: 27,
        unallocated_sp: 0,
        willpower: 24,
      }
    }

    #[tokio::test]
    async fn it_defaults_to_zero_without_an_attributes_row() {
      let db = crate::store::open_test().await.unwrap();

      let values = super::effective_attr_values(&db, 42).await;

      assert_eq!(values, AttrValues::default());
    }

    #[tokio::test]
    async fn it_sums_base_and_implant_bonuses_per_attribute() {
      let db = crate::store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      character::upsert_attributes(&db, &attributes(42)).await.unwrap();
      character::replace_implants(
        &db,
        42,
        &[CharacterImplant {
          attribute_id: 167,
          bonus: 5,
          character_id: 42,
        }],
      )
      .await
      .unwrap();

      let values = super::effective_attr_values(&db, 42).await;

      assert_eq!(values.perception, 32);
      assert_eq!(values.willpower, 24);
      assert_eq!(values.intelligence, 20);
    }
  }

  mod load_computed_queue {
    use super::*;

    #[tokio::test]
    async fn it_yields_an_empty_model_for_a_character_with_no_queue() {
      let db = crate::store::open_test().await.unwrap();

      let computed = super::load_computed_queue(&db, 42, now()).await;

      assert!(computed.items.is_empty());
      assert_eq!(computed.total_secs, 0.0);
    }

    #[tokio::test]
    async fn it_assembles_a_row_per_queue_entry() {
      let db = crate::store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_skill_ref(&db, 3300, 255, "Gunnery", "Small Hybrid Turret").await;
      character::replace_skillqueue(
        &db,
        42,
        &[CharacterSkillqueue {
          character_id: 42,
          finish_date: Some("2026-06-03T12:00:00Z".to_owned()),
          finished_level: 5,
          level_end_sp: None,
          level_start_sp: None,
          queue_position: 0,
          skill_id: 3300,
          start_date: Some("2026-05-30T12:00:00Z".to_owned()),
          training_start_sp: None,
        }],
      )
      .await
      .unwrap();

      let computed = super::load_computed_queue(&db, 42, now()).await;

      assert_eq!(computed.items.len(), 1);
      assert_eq!(computed.items[0].skill_name, "Small Hybrid Turret");
    }
  }
}
