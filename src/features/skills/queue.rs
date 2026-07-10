use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::{
  format::sp_per_sec,
  queue_timing::{active_queue, parse_timestamp, queue_entry_progress, sp_for_range},
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ClickKind {
  #[default]
  Plain,
  Range,
  RangeMerge,
  Toggle,
}

impl ClickKind {
  pub fn from_modifiers(command: bool, shift: bool) -> Self {
    match (command, shift) {
      (true, true) => ClickKind::RangeMerge,
      (false, true) => ClickKind::Range,
      (true, false) => ClickKind::Toggle,
      (false, false) => ClickKind::Plain,
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
  pub queue_position: i64,
  pub rank: u8,
  pub secondary: Attr,
  pub skill_name: String,
  pub sp_needed: u64,
  pub sp_now: u64,
  pub sp_to: u64,
  pub to_level: u8,
}

/// Ephemeral multi-selection over queue rows, keyed on the stable
/// [`CharacterSkillqueue::queue_position`]. The set is kept in queue order
/// whenever it is read back out so a derived plan preserves training order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueueSelection {
  anchor: Option<i64>,
  selected: Vec<i64>,
}

impl QueueSelection {
  pub fn len(&self) -> usize {
    self.selected.len()
  }

  pub fn is_empty(&self) -> bool {
    self.selected.is_empty()
  }

  pub fn contains(&self, position: i64) -> bool {
    self.selected.contains(&position)
  }

  pub fn clear(&mut self) {
    self.selected.clear();
    self.anchor = None;
  }

  pub fn ordered(&self, order: &[i64]) -> Vec<i64> {
    order.iter().copied().filter(|p| self.selected.contains(p)).collect()
  }

  pub fn prune(&mut self, order: &[i64]) {
    self.selected.retain(|p| order.contains(p));
    if self.anchor.is_some_and(|a| !order.contains(&a)) {
      self.anchor = None;
    }
  }

  pub fn apply(&mut self, position: i64, kind: ClickKind, order: &[i64]) {
    match kind {
      ClickKind::Plain => {
        if self.selected.len() == 1 && self.selected[0] == position {
          self.clear();
        } else {
          self.selected = vec![position];
          self.anchor = Some(position);
        }
      }
      ClickKind::Toggle => {
        if let Some(idx) = self.selected.iter().position(|p| *p == position) {
          self.selected.remove(idx);
        } else {
          self.selected.push(position);
        }
        self.anchor = Some(position);
      }
      ClickKind::Range => {
        self.selected = range_positions(self.anchor, position, order);
      }
      ClickKind::RangeMerge => {
        for pos in range_positions(self.anchor, position, order) {
          if !self.selected.contains(&pos) {
            self.selected.push(pos);
          }
        }
      }
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueWarning {
  Empty,
  LowQueue,
  Paused { queued: usize },
}

impl QueueWarning {
  pub fn message(self) -> String {
    match self {
      QueueWarning::Empty => t!("skills.warning.empty").into_owned(),
      QueueWarning::LowQueue => t!("skills.warning.low_queue").into_owned(),
      QueueWarning::Paused {
        queued,
      } => t!("skills.warning.paused", count => queued, noun => skill_word(queued)).into_owned(),
    }
  }
}

pub fn skill_word(count: usize) -> &'static str {
  if count == 1 { "skill" } else { "skills" }
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

fn range_positions(anchor: Option<i64>, position: i64, order: &[i64]) -> Vec<i64> {
  let target = match order.iter().position(|p| *p == position) {
    Some(idx) => idx,
    None => return vec![position],
  };
  let start = anchor
    .and_then(|a| order.iter().position(|p| *p == a))
    .unwrap_or(target);
  let (lo, hi) = if start <= target {
    (start, target)
  } else {
    (target, start)
  };
  order[lo..=hi].to_vec()
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
  now: DateTime<Utc>,
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
    let duration_secs = entry_duration_secs(entry, index == 0, sp.sp_needed, sp_rate, now);
    let cum_start_secs = cursor;
    cursor += duration_secs;

    result.push(ComputedQueueItem {
      cum_start_secs,
      duration_secs,
      from_level,
      group_name,
      primary,
      progress: sp.progress,
      queue_position: entry.queue_position(),
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

/// Head entry uses finish - now; later entries use finish - start; falls back to SP/rate when ESI dates are absent.
fn entry_duration_secs(
  entry: &CharacterSkillqueue,
  is_head: bool,
  sp_needed: u64,
  sp_rate: f64,
  now: DateTime<Utc>,
) -> f64 {
  let start = entry.start_date().as_deref().and_then(parse_timestamp);
  let finish = entry.finish_date().as_deref().and_then(parse_timestamp);

  let dated = if is_head {
    finish.map(|finish| (finish - now).num_seconds() as f64)
  } else {
    match (start, finish) {
      (Some(start), Some(finish)) => Some((finish - start).num_seconds() as f64),
      _ => None,
    }
  };

  dated
    .map(|secs| secs.max(0.0))
    .unwrap_or_else(|| attribute_rate_duration(sp_needed, sp_rate))
}

fn attribute_rate_duration(sp_needed: u64, sp_rate: f64) -> f64 {
  if sp_rate > 0.0 { sp_needed as f64 / sp_rate } else { 0.0 }
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

  let items = compute_queue(&with_progress, sp_rate, &skill_meta, now);
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
  let sheet = character::skills(db, character_id, Utc::now())
    .await
    .unwrap_or_default();
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueStatus {
  Training,
  Paused { queued: usize },
  Empty,
}

impl QueueStatus {
  #[cfg(test)]
  pub fn is_training(self) -> bool {
    matches!(self, QueueStatus::Training)
  }

  #[cfg(test)]
  pub fn is_paused(self) -> bool {
    matches!(self, QueueStatus::Paused { .. })
  }

  #[cfg(test)]
  pub fn is_empty(self) -> bool {
    matches!(self, QueueStatus::Empty)
  }
}

/// A head whose `finish_date` has already passed is training that ESI still
/// lists but that has, in fact, finished; the queue is effectively idle, so
/// `queue_status` reports it as `Empty` rather than `Training`.
fn head_is_completed(head: Option<&CharacterSkillqueue>, now: DateTime<Utc>) -> bool {
  head.is_some_and(|entry| {
    entry
      .finish_date()
      .as_deref()
      .and_then(parse_timestamp)
      .is_some_and(|finish| finish <= now)
  })
}

fn head_is_training(head: Option<&CharacterSkillqueue>, now: DateTime<Utc>) -> bool {
  head.is_some_and(|entry| {
    entry.start_date().is_some()
      && entry
        .finish_date()
        .as_deref()
        .and_then(parse_timestamp)
        .is_some_and(|finish| finish > now)
  })
}

pub fn queue_status(head: Option<&CharacterSkillqueue>, queued_count: usize, now: DateTime<Utc>) -> QueueStatus {
  if head_is_training(head, now) {
    QueueStatus::Training
  } else if head_is_completed(head, now) {
    QueueStatus::Empty
  } else if queued_count > 0 {
    QueueStatus::Paused {
      queued: queued_count,
    }
  } else {
    QueueStatus::Empty
  }
}

pub fn queue_warnings(computed: &ComputedQueue, status: QueueStatus) -> Vec<QueueWarning> {
  match status {
    QueueStatus::Paused {
      queued,
    } => vec![QueueWarning::Paused {
      queued,
    }],
    QueueStatus::Empty => vec![QueueWarning::Empty],
    QueueStatus::Training => {
      if computed.total_secs > 0.0 && computed.total_secs < LOW_QUEUE_THRESHOLD_SECS {
        vec![QueueWarning::LowQueue]
      } else {
        Vec::new()
      }
    }
  }
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

  mod active_attr_pair {
    use pretty_assertions::assert_eq;

    use super::*;

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
    fn it_prefers_the_head_skill_pair() {
      let mut skill_meta = HashMap::new();
      skill_meta.insert(100, meta(1, Attr::Intelligence, Attr::Memory, 0));
      skill_meta.insert(200, meta(1, Attr::Charisma, Attr::Willpower, 0));

      assert_eq!(
        active_attr_pair(Some(100), Some(200), &skill_meta),
        (Attr::Intelligence, Attr::Memory)
      );
    }
  }

  mod click_kind {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_modifier_flags_to_intents() {
      assert_eq!(ClickKind::from_modifiers(false, false), ClickKind::Plain);
      assert_eq!(ClickKind::from_modifiers(true, false), ClickKind::Toggle);
      assert_eq!(ClickKind::from_modifiers(false, true), ClickKind::Range);
      assert_eq!(ClickKind::from_modifiers(true, true), ClickKind::RangeMerge);
    }
  }

  mod compute_queue {
    use pretty_assertions::assert_eq;

    use super::*;

    fn dated(queue_position: i64, skill_id: i64, start: &str, finish: &str) -> (CharacterSkillqueue, f32) {
      (
        CharacterSkillqueue {
          finish_date: Some(finish.to_owned()),
          start_date: Some(start.to_owned()),
          ..entry(queue_position, skill_id, 5)
        },
        0.0,
      )
    }

    #[test]
    fn it_sources_the_head_eta_from_finish_date_minus_now() {
      let mut skill_meta = HashMap::new();
      skill_meta.insert(100, meta(1, Attr::Perception, Attr::Willpower, 0));
      let queue = vec![dated(0, 100, "2026-06-01T12:00:00Z", "2026-06-02T12:00:00Z")];

      let computed = compute_queue(&queue, 1.0, &skill_meta, now());

      assert_eq!(computed[0].duration_secs, 86_400.0);
    }

    #[test]
    fn it_sources_later_entry_durations_from_finish_minus_start() {
      let mut skill_meta = HashMap::new();
      skill_meta.insert(100, meta(1, Attr::Perception, Attr::Willpower, 0));
      skill_meta.insert(101, meta(1, Attr::Perception, Attr::Willpower, 0));
      let queue = vec![
        dated(0, 100, "2026-06-01T12:00:00Z", "2026-06-02T12:00:00Z"),
        dated(1, 101, "2026-06-02T12:00:00Z", "2026-06-04T12:00:00Z"),
      ];

      let computed = compute_queue(&queue, 1.0, &skill_meta, now());

      assert_eq!(computed[1].duration_secs, 172_800.0);
    }

    #[test]
    fn it_falls_back_to_the_attribute_rate_when_an_entry_has_no_dates() {
      let mut skill_meta = HashMap::new();
      skill_meta.insert(100, meta(1, Attr::Perception, Attr::Willpower, 0));
      let queue = vec![(entry(0, 100, 1), 0.0)];

      let computed = compute_queue(&queue, 2.0, &skill_meta, now());

      assert_eq!(computed[0].sp_needed, 250);
      assert_eq!(computed[0].duration_secs, 125.0);
    }

    #[test]
    fn it_yields_a_queue_total_equal_to_the_last_finish_minus_now_when_dated() {
      let mut skill_meta = HashMap::new();
      skill_meta.insert(100, meta(1, Attr::Perception, Attr::Willpower, 0));
      skill_meta.insert(101, meta(1, Attr::Perception, Attr::Willpower, 0));
      skill_meta.insert(102, meta(1, Attr::Perception, Attr::Willpower, 0));
      let queue = vec![
        dated(0, 100, "2026-06-01T12:00:00Z", "2026-06-02T12:00:00Z"),
        dated(1, 101, "2026-06-02T12:00:00Z", "2026-06-05T12:00:00Z"),
        dated(2, 102, "2026-06-05T12:00:00Z", "2026-06-06T12:00:00Z"),
      ];

      let computed = compute_queue(&queue, 1.0, &skill_meta, now());
      let total = computed.last().map(|i| i.cum_start_secs + i.duration_secs).unwrap();

      assert_eq!(total, 432_000.0);
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

      let computed = compute_queue(&queue, 1.0, &skill_meta, now());

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
    fn it_falls_back_to_rank_one_perception_willpower_when_metadata_is_absent() {
      let queue = vec![(entry(0, 100, 1), 0.0), (entry(1, 101, 5), 0.0)];

      let computed = compute_queue(&queue, 1.0, &HashMap::new(), now());

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
    fn it_treats_the_head_entry_progress_and_sp_differently_from_later_entries() {
      let queue = vec![(entry(0, 100, 5), 0.5), (entry(1, 100, 5), 0.0)];
      let mut skill_meta = HashMap::new();
      skill_meta.insert(100, meta(1, Attr::Perception, Attr::Willpower, 1_000_000));

      let computed = compute_queue(&queue, 1.0, &skill_meta, now());

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
    fn it_yields_an_empty_vec_for_an_empty_queue() {
      let computed = compute_queue(&[], 1.0, &HashMap::new(), now());

      assert!(computed.is_empty());
    }

    #[test]
    fn it_yields_zero_remaining_and_zero_duration_for_a_complete_or_over_progressed_head() {
      let mut skill_meta = HashMap::new();
      skill_meta.insert(100, meta(1, Attr::Perception, Attr::Willpower, 1_000_000));

      for progress in [1.0_f32, 2.5, f32::INFINITY] {
        let computed = compute_queue(&[(entry(0, 100, 5), progress)], 1.0, &skill_meta, now());

        assert_eq!(computed[0].sp_needed, 0, "progress {progress} leaves zero remaining SP");
        assert_eq!(
          computed[0].duration_secs, 0.0,
          "progress {progress} leaves zero duration"
        );
        assert!(computed[0].sp_now <= computed[0].sp_to);
      }
    }

    #[test]
    fn it_zeroes_durations_when_the_sp_rate_is_unknown() {
      let queue = vec![(entry(0, 100, 1), 0.0)];

      let computed = compute_queue(&queue, 0.0, &HashMap::new(), now());

      assert_eq!(computed[0].duration_secs, 0.0);
      assert_eq!(computed[0].sp_needed, 250);
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

  mod from_neural_id {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_degrades_unknown_ids_to_perception() {
      assert_eq!(Attr::from_neural_id(0), Attr::Perception);
      assert_eq!(Attr::from_neural_id(999), Attr::Perception);
    }

    #[test]
    fn it_maps_the_five_neural_ids() {
      assert_eq!(Attr::from_neural_id(164), Attr::Charisma);
      assert_eq!(Attr::from_neural_id(165), Attr::Intelligence);
      assert_eq!(Attr::from_neural_id(166), Attr::Memory);
      assert_eq!(Attr::from_neural_id(167), Attr::Perception);
      assert_eq!(Attr::from_neural_id(168), Attr::Willpower);
    }
  }

  mod queue_status {
    use super::*;

    fn dated() -> CharacterSkillqueue {
      CharacterSkillqueue {
        finish_date: Some("2026-06-11T00:00:00Z".to_owned()),
        start_date: Some("2026-06-01T00:00:00Z".to_owned()),
        ..entry(0, 100, 5)
      }
    }

    fn completed() -> CharacterSkillqueue {
      CharacterSkillqueue {
        finish_date: Some("2026-05-11T00:00:00Z".to_owned()),
        start_date: Some("2026-05-01T00:00:00Z".to_owned()),
        ..entry(0, 100, 5)
      }
    }

    #[test]
    fn it_classifies_a_fully_dated_head_as_training() {
      assert_eq!(queue_status(Some(&dated()), 3, now()), QueueStatus::Training);
    }

    #[test]
    fn it_classifies_an_undated_head_with_queued_skills_as_paused() {
      assert_eq!(
        queue_status(Some(&entry(0, 100, 5)), 4, now()),
        QueueStatus::Paused {
          queued: 4
        }
      );
    }

    #[test]
    fn it_classifies_a_head_missing_either_date_with_queued_skills_as_paused() {
      let only_start = CharacterSkillqueue {
        start_date: Some("2026-06-01T00:00:00Z".to_owned()),
        ..entry(0, 100, 5)
      };
      assert_eq!(
        queue_status(Some(&only_start), 2, now()),
        QueueStatus::Paused {
          queued: 2
        }
      );

      let only_finish = CharacterSkillqueue {
        finish_date: Some("2026-06-11T00:00:00Z".to_owned()),
        ..entry(0, 100, 5)
      };
      assert_eq!(
        queue_status(Some(&only_finish), 1, now()),
        QueueStatus::Paused {
          queued: 1
        }
      );
    }

    #[test]
    fn it_classifies_a_head_whose_finish_has_passed_as_empty() {
      assert_eq!(queue_status(Some(&completed()), 1, now()), QueueStatus::Empty);
    }

    #[test]
    fn it_classifies_no_head_as_empty() {
      assert_eq!(queue_status(None, 0, now()), QueueStatus::Empty);
    }

    #[test]
    fn it_classifies_a_zero_count_as_empty_even_with_an_undated_head() {
      assert_eq!(queue_status(Some(&entry(0, 100, 5)), 0, now()), QueueStatus::Empty);
    }

    #[test]
    fn it_prioritizes_training_over_count_when_the_head_is_dated() {
      assert_eq!(queue_status(Some(&dated()), 0, now()), QueueStatus::Training);
    }

    #[test]
    fn it_exposes_predicate_helpers() {
      assert!(QueueStatus::Training.is_training());
      assert!(
        QueueStatus::Paused {
          queued: 1
        }
        .is_paused()
      );
      assert!(QueueStatus::Empty.is_empty());
      assert!(!QueueStatus::Training.is_paused());
    }
  }

  mod load_computed_queue {
    use super::*;

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

    #[tokio::test]
    async fn it_yields_an_empty_model_for_a_character_with_no_queue() {
      let db = crate::store::open_test().await.unwrap();

      let computed = super::load_computed_queue(&db, 42, now()).await;

      assert!(computed.items.is_empty());
      assert_eq!(computed.total_secs, 0.0);
    }
  }

  mod no_queue_mutation_messages {
    #[test]
    fn the_skills_feature_declares_no_reorder_remove_or_add_message() {
      const SOURCES: [(&str, &str); 34] = [
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
        (
          "skills/right_panel/queue_tab.rs",
          include_str!("../skills/right_panel/queue_tab.rs"),
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

  mod queue_selection {
    use pretty_assertions::assert_eq;

    use super::*;

    fn order() -> Vec<i64> {
      vec![0, 1, 2, 3, 4]
    }

    #[test]
    fn a_plain_click_elsewhere_replaces_the_selection() {
      let mut sel = QueueSelection::default();
      sel.apply(2, ClickKind::Plain, &order());
      sel.apply(4, ClickKind::Plain, &order());
      assert_eq!(sel.ordered(&order()), vec![4]);
    }

    #[test]
    fn a_plain_click_selects_only_that_row() {
      let mut sel = QueueSelection::default();
      sel.apply(2, ClickKind::Plain, &order());
      assert_eq!(sel.ordered(&order()), vec![2]);
    }

    #[test]
    fn a_plain_re_click_on_the_lone_selection_clears_it() {
      let mut sel = QueueSelection::default();
      sel.apply(2, ClickKind::Plain, &order());
      sel.apply(2, ClickKind::Plain, &order());
      assert!(sel.is_empty());
    }

    #[test]
    fn a_range_click_replaces_the_prior_selection() {
      let mut sel = QueueSelection::default();
      sel.apply(0, ClickKind::Toggle, &order());
      sel.apply(2, ClickKind::Plain, &order());
      sel.apply(4, ClickKind::Range, &order());
      assert_eq!(sel.ordered(&order()), vec![2, 3, 4]);
    }

    #[test]
    fn a_range_click_selects_a_contiguous_run_from_the_anchor() {
      let mut sel = QueueSelection::default();
      sel.apply(1, ClickKind::Plain, &order());
      sel.apply(3, ClickKind::Range, &order());
      assert_eq!(sel.ordered(&order()), vec![1, 2, 3]);
    }

    #[test]
    fn a_range_handles_a_reversed_anchor() {
      let mut sel = QueueSelection::default();
      sel.apply(3, ClickKind::Plain, &order());
      sel.apply(1, ClickKind::Range, &order());
      assert_eq!(sel.ordered(&order()), vec![1, 2, 3]);
    }

    #[test]
    fn a_range_merge_unions_the_run_into_the_existing_selection() {
      let mut sel = QueueSelection::default();
      sel.apply(0, ClickKind::Toggle, &order());
      sel.apply(2, ClickKind::Toggle, &order());
      sel.apply(4, ClickKind::RangeMerge, &order());
      assert_eq!(sel.ordered(&order()), vec![0, 2, 3, 4]);
    }

    #[test]
    fn a_toggle_click_adds_and_removes_keeping_the_rest() {
      let mut sel = QueueSelection::default();
      sel.apply(1, ClickKind::Toggle, &order());
      sel.apply(3, ClickKind::Toggle, &order());
      assert_eq!(sel.ordered(&order()), vec![1, 3]);
      sel.apply(1, ClickKind::Toggle, &order());
      assert_eq!(sel.ordered(&order()), vec![3]);
    }

    #[test]
    fn clear_empties_the_selection_and_anchor() {
      let mut sel = QueueSelection::default();
      sel.apply(2, ClickKind::Plain, &order());
      sel.clear();
      assert!(sel.is_empty());
      assert_eq!(sel.len(), 0);
    }

    #[test]
    fn ordered_returns_queue_order_regardless_of_click_order() {
      let mut sel = QueueSelection::default();
      sel.apply(4, ClickKind::Toggle, &order());
      sel.apply(1, ClickKind::Toggle, &order());
      sel.apply(3, ClickKind::Toggle, &order());
      assert_eq!(sel.ordered(&order()), vec![1, 3, 4]);
    }

    #[test]
    fn prune_drops_positions_that_left_the_queue() {
      let mut sel = QueueSelection::default();
      sel.apply(1, ClickKind::Toggle, &order());
      sel.apply(4, ClickKind::Toggle, &order());
      sel.prune(&[0, 1, 2]);
      assert_eq!(sel.ordered(&[0, 1, 2]), vec![1]);
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
          queue_position: i as i64,
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
    fn it_does_not_warn_for_a_healthy_queue() {
      let warnings = queue_warnings(&computed(48.0 * 3_600.0, 2), QueueStatus::Training);

      assert!(warnings.is_empty());
    }

    #[test]
    fn it_never_warns_for_a_zero_duration_active_queue() {
      let warnings = queue_warnings(&computed(0.0, 0), QueueStatus::Training);

      assert!(warnings.is_empty());
    }

    #[test]
    fn it_surfaces_the_paused_warning_with_a_count_and_suppresses_low_queue() {
      let warnings = queue_warnings(
        &computed(1.0 * 3_600.0, 1),
        QueueStatus::Paused {
          queued: 7,
        },
      );

      assert_eq!(
        warnings,
        vec![QueueWarning::Paused {
          queued: 7
        }]
      );
    }

    #[test]
    fn it_surfaces_the_empty_warning_for_an_empty_queue() {
      let warnings = queue_warnings(&computed(0.0, 0), QueueStatus::Empty);

      assert_eq!(warnings, vec![QueueWarning::Empty]);
    }

    #[test]
    fn it_surfaces_the_low_queue_warning_under_24h() {
      let warnings = queue_warnings(&computed(23.0 * 3_600.0, 1), QueueStatus::Training);

      assert_eq!(warnings, vec![QueueWarning::LowQueue]);
    }
  }

  mod queue_warning_message {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_pluralizes_the_paused_skill_count() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);
      assert_eq!(
        QueueWarning::Paused {
          queued: 1
        }
        .message(),
        "Training paused \u{b7} 1 skill queued"
      );
      assert_eq!(
        QueueWarning::Paused {
          queued: 12
        }
        .message(),
        "Training paused \u{b7} 12 skills queued"
      );
    }

    #[test]
    fn it_describes_an_empty_queue_distinctly_from_paused() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);
      assert_eq!(
        QueueWarning::Empty.message(),
        "Training inactive \u{b7} skill queue is empty"
      );
    }
  }

  mod skill_word {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_the_singular_only_for_one() {
      assert_eq!(skill_word(0), "skills");
      assert_eq!(skill_word(1), "skill");
      assert_eq!(skill_word(2), "skills");
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
  }
}
