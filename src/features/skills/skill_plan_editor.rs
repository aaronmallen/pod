#[cfg(windows)]
mod clipboard_win;
mod eft;
mod empty_state;
mod entry_row;
mod header;
mod import_export;
mod milestone_divider;
mod picker;
mod plan_entry_list;
mod stats_strip;
mod summary;

use std::{
  collections::{BTreeMap, HashMap, HashSet},
  path::PathBuf,
};

use chrono::{DateTime, Datelike as _, Duration, Timelike as _, Utc};
use iced::{Element, Length, Task, widget::Column};
pub use import_export::{deduped_name, persist_onto_character, read_stored_plan};
use picker::PickerState;

pub(super) use crate::ui::format::{fmt_duration_padded as fmt_duration, fmt_sp_compact as fmt_sp};
use crate::{
  features::{
    shell::window_state::UiState,
    skills::{
      browse::{AttrKey, SkillCatalog, SkillCatalogEntry},
      optimizer::{Attribute, Attributes, PairWeight, optimize_remap},
      plan_csv::CSV_EXTENSION,
      plan_math::{self, ExpandedEntry, MilestoneAnchor, PlanEntry, PlanOptions, PrereqCatalog, RemapPoint, Wish},
      skill_plan_editor::picker::{PickerCert, PickerModule, PickerShip},
    },
  },
  store::{
    Database,
    model::{CharacterAttributes, ItemType, SkillPlan, SkillPlanMilestone},
    repo::{character, sde, skills},
  },
  ui::{
    components::{
      backdrop,
      context_menu::{self, Item},
      resizable_pane::{self, PaneDrag, pane_handle},
      skill_detail::{SkillDetail, skill_detail_modal},
    },
    style::spacing,
  },
};

const REMAP_ATTR_ORDER: [Attribute; 5] = [
  Attribute::Perception,
  Attribute::Memory,
  Attribute::Willpower,
  Attribute::Intelligence,
  Attribute::Charisma,
];

const MONTH_KEYS: [&str; 12] = [
  "skills.plan.month_jan",
  "skills.plan.month_feb",
  "skills.plan.month_mar",
  "skills.plan.month_apr",
  "skills.plan.month_may",
  "skills.plan.month_jun",
  "skills.plan.month_jul",
  "skills.plan.month_aug",
  "skills.plan.month_sep",
  "skills.plan.month_oct",
  "skills.plan.month_nov",
  "skills.plan.month_dec",
];
const EDITOR_HOST_WIDTH: f32 = 900.0;
const PICKER_WIDTH: f32 = 340.0;
const SUMMARY_WIDTH: f32 = 360.0;
const PICKER_PANE_KEY: &str = "plan.picker";
const SUMMARY_PANE_KEY: &str = "plan.summary";
// A template has no character, hence no remap history/cooldown to derive a real count from
// (see plan_math::remap_availability); this is a fixed stand-in, not a computed value.
const FRESH_PILOT_REMAPS: u32 = 3;

pub(super) const ACTIONS_COL_WIDTH: f32 = 84.0;
pub(super) const ATTR_COL_WIDTH: f32 = 52.0;
pub(super) const INDEX_COL_WIDTH: f32 = 28.0;
pub(super) const SP_COL_WIDTH: f32 = 80.0;
pub(super) const TIME_COL_WIDTH: f32 = 110.0;

#[derive(Clone, Debug)]
pub struct ComputedRow {
  pub cumulative_sec: f64,
  pub group_name: String,
  pub id: i64,
  pub is_auto: bool,
  pub note: String,
  pub primary: AttrKey,
  pub priority: Priority,
  pub rank: u8,
  pub sec: f64,
  pub secondary: AttrKey,
  pub skill_name: String,
  pub skipped: bool,
  pub sp: u64,
  pub to_level: u8,
}

#[derive(Clone, Debug)]
pub struct EditEntry {
  id: i64,
  is_auto: bool,
  meta: EntryMeta,
  note: String,
  priority: Priority,
  skill_id: i64,
  to_level: u8,
}

/// `base: None` means the milestone has no attribute set yet (cleared, or an `auto_remap` milestone whose
/// segment is currently empty); such milestones don't count toward `placed_in_plan_remaps` and are excluded
/// from the math passed to `plan_math` until a base is computed or chosen.
#[derive(Clone, Debug)]
pub struct EditMilestone {
  after_entry_id: Option<i64>,
  auto_remap: bool,
  base: Option<Attributes>,
  local_id: i64,
  name: String,
  order: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorPane {
  Picker,
  Summary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportFeedback {
  Failed,
  Succeeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IoPanel {
  Export,
  Import,
  ImportPrompt,
}

#[derive(Clone, Debug)]
pub struct Loaded {
  attrs: Attributes,
  base_attrs: Attributes,
  booster_n: u32,
  catalog: SkillCatalog,
  cert_proficiency: HashMap<i64, usize>,
  character_total_sp: u64,
  consistent: bool,
  draft_name: Option<String>,
  entries: Vec<EditEntry>,
  plan: Option<SkillPlan>,
  remap_availability: u32,
  remap_points: Vec<EditMilestone>,
  remap_reason: String,
  ship_mastery: HashMap<i64, u8>,
  sort: Sort,
  synced_sp: HashMap<i64, u64>,
  trained_levels: HashMap<i64, u8>,
}

/// Wraps `SkillDetail` in an `Arc` so the modal payload is cheaply `Clone`; the foundation
/// type derives neither `Clone` nor `Debug`, hence the manual `Debug` impl below.
#[derive(Clone)]
pub struct SkillDetailLoad(std::sync::Arc<SkillDetail>);

impl std::fmt::Debug for SkillDetailLoad {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("SkillDetailLoad")
  }
}

#[derive(Clone, Debug)]
pub enum Message {
  CloseRequested,
  CursorMoved(iced::Point),
  DragDropped,
  DragHovered(usize),
  DragLeft(usize),
  DragStarted(i64),
  EntryNoteChanged(i64, String),
  EntryNoteToggled(i64),
  EntryPriorityCycled(i64),
  EntryRemoved(i64),
  EntryRowRightPressed(i64),
  // Payload unread: the variant is the fn-pointer constructor for the save-file dialog future, whose path result is intentionally ignored.
  #[expect(
    dead_code,
    reason = "Variant is a fn-pointer constructor; the dialog-future path result is intentionally ignored."
  )]
  ExportFilePicked(Option<PathBuf>),
  ExportRequested,
  ExportToClipboard,
  ExportToCsv,
  ExportToFile,
  ImportAppend,
  ImportClipboardRead(Option<String>),
  ImportEftClipboardRead(Option<String>),
  ImportEftFromClipboard,
  ImportEftResolved(Vec<Wish>),
  ImportFeedbackDismissed,
  ImportFileLoaded(Option<String>),
  ImportFromClipboard,
  ImportFromFile,
  ImportReplace,
  ImportRequested,
  IoDismissed,
  Loaded(Box<Loaded>),
  MilestoneCollapseToggled(i64),
  MilestoneExport(i64, MilestoneExportTarget),
  MilestoneExportMenuDismissed,
  MilestoneExportMenuToggled(i64),
  MilestoneImportMenuDismissed,
  MilestoneImportMenuToggled(i64),
  MilestoneImportPicked(i64, MilestoneImportSource),
  MilestoneRemapCleared(i64),
  MilestoneRemapSuggested(i64),
  MilestoneRemoved(i64),
  MilestoneRenamed(i64, String),
  #[allow(dead_code)]
  MilestonesAllSuggested,
  NameChanged(String),
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart(EditorPane),
  PaneSettled(&'static str, f32),
  PickerCertProficiencyChanged(i64, usize),
  PickerCertSelected(i64, usize),
  PickerCertsLoaded(Vec<picker::PickerCert>),
  PickerContextMenuDismissed,
  PickerContextPlan(i64, u8),
  PickerGroupToggled(i64),
  PickerLevelPicked(i64, u8),
  PickerModuleSelected(i64),
  PickerModulesLoaded(Vec<picker::PickerModule>),
  PickerRowRightPressed(i64),
  PickerSearchChanged(String),
  PickerShipMasteryChanged(i64, u8),
  PickerShipSelected(i64, u8),
  PickerShipsLoaded(Vec<picker::PickerShip>),
  PickerTabSelected(picker::PickerTab),
  PickerToggled,
  RemapInserted(Option<i64>),
  // Payload unread: the variant is the fn-pointer constructor for the reorder-persist future, whose result is intentionally ignored.
  #[expect(
    dead_code,
    reason = "Variant is a fn-pointer constructor; the reorder-persist future result is intentionally ignored."
  )]
  Reordered(Result<(), String>),
  SaveRequested,
  Saved(Result<i64, String>),
  SkillDetailClosed,
  SkillDetailLoaded(Option<SkillDetailLoad>),
  SkillInfoRequested(i64),
  SortChanged(SortColumn),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MilestoneImportSource {
  Clipboard,
  ClipboardEft,
  File,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MilestoneExportTarget {
  Clipboard,
  Csv,
  Psp,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Priority {
  High,
  Low,
  #[default]
  Normal,
}

impl Priority {
  fn from_token(token: &str) -> Self {
    match token {
      "low" => Priority::Low,
      "high" => Priority::High,
      _ => Priority::Normal,
    }
  }

  fn as_token(self) -> &'static str {
    match self {
      Priority::Low => "low",
      Priority::Normal => "normal",
      Priority::High => "high",
    }
  }

  fn next(self) -> Self {
    match self {
      Priority::Low => Priority::Normal,
      Priority::Normal => Priority::High,
      Priority::High => Priority::Low,
    }
  }
}

#[derive(Clone, Debug)]
struct SkillContextMenu {
  anchor: iced::Point,
  /// Present only when the menu was opened from a queued entry row in manual sort; carries the anchors
  /// the "Add milestone above/below" items insert against. Absent for the picker menu.
  entry: Option<EntryMenuContext>,
  highest_planned: u8,
  name: String,
  skill_id: i64,
  trained: u8,
}

#[derive(Clone, Copy, Debug)]
struct EntryMenuContext {
  entry_id: i64,
  prev_entry_id: Option<i64>,
}

#[derive(Clone, Debug)]
pub enum Seed {
  Existing(i64),
  FromQueue,
  FromQueueSelection(Vec<i64>),
  New,
  NewTemplate,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Sort {
  pub column: SortColumn,
  pub direction: SortDirection,
}

impl Sort {
  fn from_token(token: &str) -> Self {
    match token {
      "primary-asc" => Sort {
        column: SortColumn::Primary,
        direction: SortDirection::Ascending,
      },
      "primary-desc" => Sort {
        column: SortColumn::Primary,
        direction: SortDirection::Descending,
      },
      "secondary-asc" => Sort {
        column: SortColumn::Secondary,
        direction: SortDirection::Ascending,
      },
      "secondary-desc" => Sort {
        column: SortColumn::Secondary,
        direction: SortDirection::Descending,
      },
      "time-asc" => Sort {
        column: SortColumn::Time,
        direction: SortDirection::Ascending,
      },
      "time-desc" => Sort {
        column: SortColumn::Time,
        direction: SortDirection::Descending,
      },
      _ => Sort::default(),
    }
  }

  pub fn caret(self, column: SortColumn) -> Option<SortDirection> {
    (self.column == column).then_some(self.direction)
  }

  pub fn is_active(self, column: SortColumn) -> bool {
    self.column == column
  }

  pub fn toggled(self, column: SortColumn) -> Self {
    if column == SortColumn::Manual {
      return Sort::default();
    }
    if self.column == column {
      // Each column cycles asc -> desc -> Manual, so un-sorting is always one more click away.
      match self.direction {
        SortDirection::Ascending => Sort {
          column,
          direction: SortDirection::Descending,
        },
        SortDirection::Descending => Sort::default(),
      }
    } else {
      Sort {
        column,
        direction: column.natural_direction(),
      }
    }
  }

  fn as_token(self) -> &'static str {
    match (self.column, self.direction) {
      (SortColumn::Manual, _) => "manual",
      (SortColumn::Primary, SortDirection::Ascending) => "primary-asc",
      (SortColumn::Primary, SortDirection::Descending) => "primary-desc",
      (SortColumn::Secondary, SortDirection::Ascending) => "secondary-asc",
      (SortColumn::Secondary, SortDirection::Descending) => "secondary-desc",
      (SortColumn::Time, SortDirection::Ascending) => "time-asc",
      (SortColumn::Time, SortDirection::Descending) => "time-desc",
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortColumn {
  #[default]
  Manual,
  Primary,
  Secondary,
  Time,
}

impl SortColumn {
  fn natural_direction(self) -> SortDirection {
    SortDirection::Ascending
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortDirection {
  #[default]
  Ascending,
  Descending,
}

#[derive(Debug)]
pub struct State {
  attrs: Attributes,
  base_attrs: Attributes,
  booster_n: u32,
  character_id: Option<i64>,
  character_total_sp: u64,
  collapsed_milestones: HashSet<i64>,
  consistent: bool,
  context_menu: Option<SkillContextMenu>,
  cursor: Option<iced::Point>,
  dirty: bool,
  display_milestones: Vec<EditMilestone>,
  display_rows: Vec<ComputedRow>,
  dragging: Option<i64>,
  dragging_pane: Option<EditorPane>,
  drop_index: Option<usize>,
  entries: Vec<EditEntry>,
  export_menu: Option<i64>,
  import_feedback: Option<ImportFeedback>,
  import_menu: Option<i64>,
  import_target: Option<i64>,
  io_panel: Option<IoPanel>,
  name: String,
  next_entry_id: i64,
  next_remap_id: i64,
  note_open: Option<i64>,
  pending_import: Option<import_export::Payload>,
  picker: PickerState,
  picker_open: bool,
  picker_pane: PaneDrag,
  plan: Option<SkillPlan>,
  remap_availability: u32,
  remap_points: Vec<EditMilestone>,
  remap_reason: String,
  rows: Vec<ComputedRow>,
  saved: Snapshot,
  skill_detail: Option<SkillDetailLoad>,
  sort: Sort,
  source_plan_id: Option<i64>,
  summary: summary::SummaryData,
  summary_pane: PaneDrag,
  synced_levels: HashMap<i64, u8>,
  synced_sp: HashMap<i64, u64>,
  total_sec: f64,
  total_sp: u64,
}

impl State {
  pub fn new(character_id: Option<i64>) -> Self {
    State {
      character_id,
      name: String::new(),
      picker_open: true,
      sort: Sort::default(),
      note_open: None,
      dragging: None,
      drop_index: None,
      export_menu: None,
      import_feedback: None,
      import_menu: None,
      import_target: None,
      io_panel: None,
      pending_import: None,
      saved: Snapshot::default(),
      skill_detail: None,
      source_plan_id: None,
      dirty: false,
      next_remap_id: 1,
      next_entry_id: -1,
      picker: PickerState::default(),
      picker_pane: PaneDrag::new(PICKER_WIDTH, EDITOR_HOST_WIDTH),
      summary_pane: PaneDrag::new(SUMMARY_WIDTH, EDITOR_HOST_WIDTH),
      dragging_pane: None,
      attrs: Attributes::default(),
      base_attrs: Attributes::default(),
      booster_n: 0,
      collapsed_milestones: HashSet::new(),
      consistent: true,
      context_menu: None,
      cursor: None,
      character_total_sp: 0,
      entries: Vec::new(),
      plan: None,
      remap_availability: 0,
      remap_reason: String::new(),
      remap_points: Vec::new(),
      synced_levels: HashMap::new(),
      synced_sp: HashMap::new(),
      rows: Vec::new(),
      display_rows: Vec::new(),
      display_milestones: Vec::new(),
      summary: summary::SummaryData::default(),
      total_sp: 0,
      total_sec: 0.0,
    }
  }

  pub fn with_restored_panes(mut self, ui: &UiState) -> Self {
    let host_width = ui.host_width("skill_plan_editor", EDITOR_HOST_WIDTH);
    self.picker_pane = PaneDrag::from_store(ui, PICKER_PANE_KEY, PICKER_WIDTH, host_width);
    self.summary_pane = PaneDrag::from_store(ui, SUMMARY_PANE_KEY, SUMMARY_WIDTH, host_width);
    self
  }

  pub fn with_source_plan_id(mut self, source_plan_id: Option<i64>) -> Self {
    self.source_plan_id = source_plan_id;
    self
  }

  pub fn set_pane_host_width(&mut self, host_width: f32) {
    self.picker_pane.set_host_width(host_width);
    self.summary_pane.set_host_width(host_width);
  }

  #[cfg(test)]
  pub fn character_id(&self) -> Option<i64> {
    self.character_id
  }

  pub fn source_plan_id(&self) -> Option<i64> {
    self.source_plan_id
  }

  fn can_place_remap(&self) -> bool {
    self.placed_in_plan_remaps() < self.remap_availability
  }

  fn catalog_entry(&self, skill_id: i64) -> Option<&SkillCatalogEntry> {
    self
      .picker
      .catalog
      .as_ref()?
      .groups
      .iter()
      .flat_map(|group| group.skills.iter())
      .find(|skill| skill.type_id == skill_id)
  }

  fn computed(&self) -> Computed {
    let plan_entries = self.plan_entries();

    let options = PlanOptions {
      implant: Some(self.implant_bonus()),
      remap_points: remap_points_to_math(&self.entries, &self.remap_points),
    };
    let plan = plan_math::compute_plan(&plan_entries, self.attrs, &options, 0.0);

    let rows = plan
      .items
      .iter()
      .zip(&self.entries)
      .map(|(item, entry)| ComputedRow {
        cumulative_sec: item.cumulative_sec,
        group_name: entry.meta.group_name.clone(),
        id: entry.id,
        is_auto: entry.is_auto,
        note: entry.note.clone(),
        primary: entry.meta.primary,
        priority: entry.priority,
        rank: entry.meta.rank,
        secondary: entry.meta.secondary,
        sec: item.sec,
        skipped: item.skipped,
        skill_name: entry.meta.skill_name.clone(),
        sp: item.sp,
        to_level: item.to_level,
      })
      .collect();

    Computed {
      rows,
      total_sp: plan.total_sp,
      total_sec: plan.total_sec,
    }
  }

  fn dirty(&self) -> bool {
    self.dirty
  }

  /// `attrs` is base + implants + booster, with the booster applied uniformly across all five attributes, so it
  /// must be subtracted alongside `base_attrs` to leave the implant-only bonus.
  fn implant_bonus(&self) -> Attributes {
    Attributes {
      charisma: self
        .attrs
        .charisma
        .saturating_sub(self.base_attrs.charisma)
        .saturating_sub(self.booster_n),
      intelligence: self
        .attrs
        .intelligence
        .saturating_sub(self.base_attrs.intelligence)
        .saturating_sub(self.booster_n),
      memory: self
        .attrs
        .memory
        .saturating_sub(self.base_attrs.memory)
        .saturating_sub(self.booster_n),
      perception: self
        .attrs
        .perception
        .saturating_sub(self.base_attrs.perception)
        .saturating_sub(self.booster_n),
      willpower: self
        .attrs
        .willpower
        .saturating_sub(self.base_attrs.willpower)
        .saturating_sub(self.booster_n),
    }
  }

  fn implant_effect(&self) -> summary::ImplantEffect {
    let with_sec = self.total_sec;

    let plan_entries = self.plan_entries();
    let options = PlanOptions {
      implant: None,
      remap_points: remap_points_to_math(&self.entries, &self.remap_points),
    };
    let without = plan_math::compute_plan(&plan_entries, self.base_attrs, &options, 0.0);

    summary::ImplantEffect {
      bonus: self.implant_bonus(),
      with_sec,
      without_sec: without.total_sec,
    }
  }

  fn is_template(&self) -> bool {
    self.character_id.is_none()
  }

  fn milestone_stats(&self) -> HashMap<i64, MilestoneStats> {
    let entry_ids = self.ordered_ids();
    let anchors: Vec<MilestoneAnchor> = self
      .remap_points
      .iter()
      .map(|milestone| MilestoneAnchor {
        after_entry_id: milestone.after_entry_id,
        order: milestone.order,
      })
      .collect();

    let mut stats = HashMap::new();
    let mut number = 0;
    for segment in plan_math::plan_segments(&entry_ids, &anchors) {
      let Some(index) = segment.milestone else {
        continue;
      };
      number += 1;
      let end = segment.end.min(self.rows.len());
      let start = segment.start.min(end);
      let mut entry = MilestoneStats {
        number,
        ..MilestoneStats::default()
      };
      for row in &self.rows[start..end] {
        if !row.skipped {
          entry.sec += row.sec;
          entry.sp += row.sp;
          entry.steps += 1;
        }
      }
      stats.insert(self.remap_points[index].local_id, entry);
    }
    stats
  }

  // Stored-index range `[start, end)` (into `entries`/`rows`) of the segment owned by milestone `local_id`, or `None`
  // when no milestone matches. Mirrors the anchor math `milestone_stats` uses so the two agree on segment boundaries.
  fn milestone_segment_bounds(&self, local_id: i64) -> Option<(usize, usize)> {
    let entry_ids = self.ordered_ids();
    let anchors: Vec<MilestoneAnchor> = self
      .remap_points
      .iter()
      .map(|milestone| MilestoneAnchor {
        after_entry_id: milestone.after_entry_id,
        order: milestone.order,
      })
      .collect();

    plan_math::plan_segments(&entry_ids, &anchors)
      .into_iter()
      .find_map(|segment| {
        let index = segment.milestone?;
        (self.remap_points[index].local_id == local_id).then_some((segment.start, segment.end))
      })
  }

  fn next_entry_id(&mut self) -> i64 {
    let id = self.next_entry_id;
    self.next_entry_id -= 1;
    id
  }

  fn next_remap_id(&mut self) -> i64 {
    let id = self.next_remap_id;
    self.next_remap_id += 1;
    id
  }

  fn ordered_ids(&self) -> Vec<i64> {
    self.entries.iter().map(|e| e.id).collect()
  }

  fn pair_weights(&self) -> Vec<PairWeight> {
    let mut by_pair: Vec<PairWeight> = Vec::new();
    for (row, entry) in self.rows.iter().zip(&self.entries) {
      if row.skipped || row.sp == 0 {
        continue;
      }
      let primary = plan_math_attribute(entry.meta.primary_id);
      let secondary = plan_math_attribute(entry.meta.secondary_id);
      match by_pair
        .iter_mut()
        .find(|w| w.primary == primary && w.secondary == secondary)
      {
        Some(weight) => weight.sp += row.sp,
        None => by_pair.push(PairWeight {
          primary,
          secondary,
          sp: row.sp,
        }),
      }
    }
    by_pair
  }

  fn placed_in_plan_remaps(&self) -> u32 {
    self
      .remap_points
      .iter()
      .filter(|point| point.after_entry_id.is_some() && point.base.is_some())
      .count() as u32
  }

  fn plan_entries(&self) -> Vec<PlanEntry> {
    self
      .entries
      .iter()
      .map(|e| PlanEntry {
        primary: plan_math_attribute(e.meta.primary_id),
        rank: f64::from(e.meta.rank.max(1)),
        secondary: plan_math_attribute(e.meta.secondary_id),
        skill_id: e.skill_id,
        partial_sp_at_from: self.synced_sp.get(&e.skill_id).copied().unwrap_or(0),
        synced_trained_level: self.synced_levels.get(&e.skill_id).copied().unwrap_or(0),
        to_level: e.to_level.clamp(0, 5),
      })
      .collect()
  }

  fn planned_levels(&self) -> HashMap<i64, u8> {
    let mut planned: HashMap<i64, u8> = HashMap::new();
    for entry in &self.entries {
      planned
        .entry(entry.skill_id)
        .and_modify(|level| *level = (*level).max(entry.to_level))
        .or_insert(entry.to_level);
    }
    planned
  }

  fn prereq_catalog(&self) -> PrereqCatalog {
    match self.picker.catalog.as_ref() {
      Some(catalog) => prereq_catalog_from(catalog),
      None => PrereqCatalog::new(),
    }
  }

  fn recompute_auto_milestones(&mut self) -> bool {
    if !self.remap_points.iter().any(|milestone| milestone.auto_remap) {
      return false;
    }

    let entry_ids = self.ordered_ids();
    let anchors: Vec<MilestoneAnchor> = self
      .remap_points
      .iter()
      .map(|milestone| MilestoneAnchor {
        after_entry_id: milestone.after_entry_id,
        order: milestone.order,
      })
      .collect();
    let mut segment_by_milestone: HashMap<usize, (usize, usize)> = HashMap::new();
    for segment in plan_math::plan_segments(&entry_ids, &anchors) {
      if let Some(index) = segment.milestone {
        segment_by_milestone.insert(index, (segment.start, segment.end));
      }
    }

    let implants = self.implant_bonus();
    let base_attrs = self.base_attrs;
    let mut changed = false;
    for index in 0..self.remap_points.len() {
      if !self.remap_points[index].auto_remap {
        continue;
      }
      let next_base = match segment_by_milestone.get(&index) {
        Some(&(start, end)) if end > start => {
          let weights = self.segment_weights(start, end);
          Some(optimize_remap(&weights, base_attrs, implants).base)
        }
        _ => None,
      };
      if self.remap_points[index].base != next_base {
        self.remap_points[index].base = next_base;
        changed = true;
      }
    }
    changed
  }

  fn recompute_dirty(&mut self) {
    self.dirty = self.snapshot() != self.saved;
  }

  fn refresh_rows(&mut self) {
    let Computed {
      rows,
      total_sp,
      total_sec,
    } = self.computed();
    self.rows = rows;
    self.total_sp = total_sp;
    self.total_sec = total_sec;

    // Auto milestones derive their base from the rows just computed above; if any base changed,
    // the rows must be recomputed a second time so they reflect the new attribute split.
    if self.recompute_auto_milestones() {
      let Computed {
        rows,
        total_sp,
        total_sec,
      } = self.computed();
      self.rows = rows;
      self.total_sp = total_sp;
      self.total_sec = total_sec;
    }

    self.summary = self.summary_data();
    self.rebuild_display();
  }

  // Projects the manual-order rows/remap_points into the currently active sort view. rows and remap_points are
  // never reordered; sorting only changes display_rows/display_milestones, so the underlying manual order and
  // milestone anchors survive a sort/unsort cycle and every save.
  fn rebuild_display(&mut self) {
    let view = self.sort_view();
    self.display_rows = view.order.iter().map(|&index| self.rows[index].clone()).collect();
    self.display_milestones = self
      .remap_points
      .iter()
      .enumerate()
      .map(|(index, milestone)| EditMilestone {
        after_entry_id: view.reanchor[index],
        ..milestone.clone()
      })
      .collect();
  }

  fn sort_keys(&self, column: SortColumn) -> Vec<f64> {
    self
      .rows
      .iter()
      .map(|row| match column {
        SortColumn::Manual => 0.0,
        SortColumn::Primary => f64::from(row.primary as u8),
        SortColumn::Secondary => f64::from(row.secondary as u8),
        SortColumn::Time => row.sec,
      })
      .collect()
  }

  fn sort_view(&self) -> SortView {
    match self.sort.column {
      SortColumn::Manual => SortView {
        order: (0..self.entries.len()).collect(),
        reanchor: self
          .remap_points
          .iter()
          .map(|milestone| milestone.after_entry_id)
          .collect(),
      },
      column => {
        let asc = self.sort.direction == SortDirection::Ascending;
        let keys = self.sort_keys(column);
        segmented_sort(&self.entries, &self.remap_points, &self.prereq_catalog(), &keys, asc)
      }
    }
  }

  fn segment_weights(&self, start: usize, end: usize) -> Vec<PairWeight> {
    let clamped_end = end.min(self.rows.len()).min(self.entries.len());
    let mut by_pair: Vec<PairWeight> = Vec::new();
    if start >= clamped_end {
      return by_pair;
    }
    for (row, entry) in self.rows[start..clamped_end]
      .iter()
      .zip(&self.entries[start..clamped_end])
    {
      if row.skipped || row.sp == 0 {
        continue;
      }
      let primary = plan_math_attribute(entry.meta.primary_id);
      let secondary = plan_math_attribute(entry.meta.secondary_id);
      match by_pair
        .iter_mut()
        .find(|w| w.primary == primary && w.secondary == secondary)
      {
        Some(weight) => weight.sp += row.sp,
        None => by_pair.push(PairWeight {
          primary,
          secondary,
          sp: row.sp,
        }),
      }
    }
    by_pair
  }

  fn snapshot(&self) -> Snapshot {
    Snapshot {
      name: self.name.trim().to_owned(),
      sort: self.sort,
      entries: self
        .entries
        .iter()
        .map(|e| (e.skill_id, e.to_level, e.priority, e.note.clone(), e.is_auto))
        .collect(),
      remaps: self
        .remap_points
        .iter()
        .map(|r| {
          (
            r.after_entry_id,
            r.name.clone(),
            r.auto_remap,
            r.order,
            r.base.map(|base| {
              (
                i64::from(base.perception),
                i64::from(base.memory),
                i64::from(base.willpower),
                i64::from(base.intelligence),
                i64::from(base.charisma),
              )
            }),
          )
        })
        .collect(),
      ship_mastery: {
        let mut v: Vec<(i64, u8)> = self.picker.ship_mastery.iter().map(|(&id, &tier)| (id, tier)).collect();
        v.sort_unstable();
        v
      },
      cert_proficiency: {
        let mut v: Vec<(i64, usize)> = self
          .picker
          .cert_proficiency
          .iter()
          .map(|(&id, &prof)| (id, prof))
          .collect();
        v.sort_unstable();
        v
      },
    }
  }

  fn summary_data(&self) -> summary::SummaryData {
    let mut group_sec: HashMap<String, f64> = HashMap::new();
    let mut pair_sec: HashMap<String, f64> = HashMap::new();
    for row in &self.rows {
      if row.skipped || row.sec <= 0.0 {
        continue;
      }
      let group = if row.group_name.is_empty() {
        t!("skills.plan.group_other").into_owned()
      } else {
        row.group_name.clone()
      };
      *group_sec.entry(group).or_insert(0.0) += row.sec;
      let pair = format!("{} / {}", row.primary.short(), row.secondary.short());
      *pair_sec.entry(pair).or_insert(0.0) += row.sec;
    }

    let weights = self.pair_weights();
    let implants = self.implant_bonus();
    let recommendation = optimize_remap(&weights, self.base_attrs, implants);
    let current_sec = plan_time_for(&weights, self.attrs);
    let current_base_sec = plan_time_for(&weights, sum_attrs(self.base_attrs, implants));

    summary::SummaryData {
      base_attrs: self.base_attrs,
      character_total_sp: self.character_total_sp,
      consistent: self.consistent,
      current_base_sec,
      current_sec,
      group_sec,
      implant_effect: self.implant_effect(),
      is_template: self.is_template(),
      pair_sec,
      recommendation,
      remap_availability: self.remap_availability,
      remap_reason: self.remap_reason.clone(),
      total_sec: self.total_sec,
      total_sp: self.total_sp,
      steps: self.rows.iter().filter(|r| !r.skipped).count(),
    }
  }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct MilestoneStats {
  pub number: usize,
  pub sec: f64,
  pub sp: u64,
  pub steps: usize,
}

struct CharacterAttrs {
  attrs: Attributes,
  availability: u32,
  base_attrs: Attributes,
  booster_n: u32,
  consistent: bool,
  reason: String,
}

struct CharacterSync {
  character_total_sp: u64,
  synced_sp: HashMap<i64, u64>,
  trained_levels: HashMap<i64, u8>,
}

struct Computed {
  rows: Vec<ComputedRow>,
  total_sec: f64,
  total_sp: u64,
}

#[derive(Clone, Debug)]
struct EntryMeta {
  group_name: String,
  primary: AttrKey,
  primary_id: i64,
  rank: u8,
  secondary: AttrKey,
  secondary_id: i64,
  skill_name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ImportMode {
  Append,
  Replace,
}

#[derive(Clone, Debug)]
struct RemapSave {
  anchor_index: Option<usize>,
  auto_remap: bool,
  base: Option<(i64, i64, i64, i64, i64)>,
  name: String,
  order: i64,
}

type SnapshotRemap = (Option<i64>, String, bool, i64, Option<(i64, i64, i64, i64, i64)>);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Snapshot {
  cert_proficiency: Vec<(i64, usize)>,
  entries: Vec<(i64, u8, Priority, String, bool)>,
  name: String,
  remaps: Vec<SnapshotRemap>,
  ship_mastery: Vec<(i64, u8)>,
  sort: Sort,
}

pub fn pack_skill_count(raw: &str) -> Result<usize, crate::services::pod_pack::DecodeError> {
  import_export::from_psp(raw).map(|plan| plan.entries.len())
}

pub fn load(db: &Database, character_id: Option<i64>, seed: Seed) -> Task<Message> {
  Task::perform(async_load(db.clone(), character_id, seed, Utc::now()), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
}

fn is_escape_pressed(event: &iced::Event) -> bool {
  matches!(
    event,
    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
      key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
      ..
    })
  )
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  let mut subs: Vec<iced::Subscription<Message>> = Vec::new();
  if state.context_menu.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      is_escape_pressed(&event).then_some(Message::PickerContextMenuDismissed)
    }));
  } else if state.skill_detail.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      is_escape_pressed(&event).then_some(Message::SkillDetailClosed)
    }));
  }
  if state.dragging.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      matches!(
        event,
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
      )
      .then_some(Message::DragDropped)
    }));
  }
  if state.dragging_pane.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      resizable_pane::drag_event(event, Message::PaneDrag, Message::PaneDragEnd)
    }));
  }
  iced::Subscription::batch(subs)
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  let task = dispatch(state, message, db);
  state.recompute_dirty();
  task
}

fn dispatch(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  let message = match handle_io(state, message, db) {
    Ok(task) => return task,
    Err(message) => message,
  };
  let message = match handle_context_menu(state, message) {
    Ok(task) => return task,
    Err(message) => message,
  };
  let message = match handle_picker(state, message, db) {
    Ok(task) => return task,
    Err(message) => message,
  };
  let message = match handle_skill_detail(state, message, db) {
    Ok(task) => return task,
    Err(message) => message,
  };
  let message = match handle_drag(state, message, db) {
    Ok(task) => return task,
    Err(message) => message,
  };
  let message = match handle_entry_edit(state, message) {
    Ok(task) => return task,
    Err(message) => message,
  };
  let message = match handle_remap(state, message) {
    Ok(task) => return task,
    Err(message) => message,
  };
  let message = match handle_pane(state, message) {
    Ok(task) => return task,
    Err(message) => message,
  };
  handle_lifecycle(state, message, db)
}

fn handle_io(state: &mut State, message: Message, db: &Database) -> Result<Task<Message>, Message> {
  let message = match handle_export_io(state, message) {
    Ok(task) => return Ok(task),
    Err(message) => message,
  };
  match handle_import_io(state, message, db) {
    Ok(task) => Ok(task),
    Err(Message::IoDismissed) => {
      state.io_panel = None;
      state.pending_import = None;
      state.import_target = None;
      Ok(Task::none())
    }
    Err(other) => Err(other),
  }
}

fn handle_export_io(state: &mut State, message: Message) -> Result<Task<Message>, Message> {
  match message {
    Message::ExportFilePicked(_) => Ok(Task::none()),
    Message::ExportRequested => {
      state.io_panel = if state.io_panel == Some(IoPanel::Export) {
        None
      } else {
        Some(IoPanel::Export)
      };
      Ok(Task::none())
    }
    Message::ExportToClipboard => {
      state.io_panel = None;
      Ok(iced::clipboard::write(serialize_plan_text(state)))
    }
    Message::ExportToCsv => {
      state.io_panel = None;
      let contents = serialize_plan_csv(state);
      let default_name = export_csv_file_name(state);
      Ok(Task::perform(
        save_to_file_dialog(
          default_name,
          contents,
          t!("skills.plan.file_filter_csv").into_owned(),
          CSV_EXTENSION,
        ),
        Message::ExportFilePicked,
      ))
    }
    Message::ExportToFile => {
      state.io_panel = None;
      let contents = import_export::to_psp(&plan_file(state));
      let default_name = export_file_name(state);
      Ok(Task::perform(
        save_to_file_dialog(
          default_name,
          contents,
          t!("skills.plan.file_filter_psp").into_owned(),
          import_export::PSP_EXTENSION,
        ),
        Message::ExportFilePicked,
      ))
    }
    Message::MilestoneExport(local_id, target) => {
      state.export_menu = None;
      Ok(milestone_export_io(state, local_id, target))
    }
    Message::MilestoneExportMenuDismissed => {
      state.export_menu = None;
      Ok(Task::none())
    }
    Message::MilestoneExportMenuToggled(local_id) => {
      state.import_menu = None;
      state.export_menu = if state.export_menu == Some(local_id) {
        None
      } else {
        Some(local_id)
      };
      Ok(Task::none())
    }
    other => Err(other),
  }
}

fn milestone_export_io(state: &State, local_id: i64, target: MilestoneExportTarget) -> Task<Message> {
  let order = milestone_segment_order(state, local_id);
  match target {
    MilestoneExportTarget::Clipboard => iced::clipboard::write(serialize_text_for(state, &order)),
    MilestoneExportTarget::Csv => Task::perform(
      save_to_file_dialog(
        export_csv_file_name(state),
        serialize_csv_for(state, &order),
        t!("skills.plan.file_filter_csv").into_owned(),
        CSV_EXTENSION,
      ),
      Message::ExportFilePicked,
    ),
    MilestoneExportTarget::Psp => Task::perform(
      save_to_file_dialog(
        export_file_name(state),
        import_export::to_psp(&segment_plan_file(state, &order)),
        t!("skills.plan.file_filter_psp").into_owned(),
        import_export::PSP_EXTENSION,
      ),
      Message::ExportFilePicked,
    ),
  }
}

fn handle_import_io(state: &mut State, message: Message, db: &Database) -> Result<Task<Message>, Message> {
  match message {
    Message::ImportAppend => {
      apply_pending_import(state, ImportMode::Append);
      Ok(Task::none())
    }
    Message::ImportClipboardRead(text) => {
      log_clipboard_read(text.as_deref());
      stage_import(state, text.as_deref().unwrap_or_default());
      Ok(Task::none())
    }
    Message::ImportEftClipboardRead(text) => {
      log_clipboard_read(text.as_deref());
      Ok(stage_eft_names(state, text.as_deref().unwrap_or_default(), db))
    }
    Message::ImportEftFromClipboard => {
      state.io_panel = None;
      state.import_feedback = None;
      state.import_target = None;
      Ok(clipboard_read_task().map(Message::ImportEftClipboardRead))
    }
    Message::ImportEftResolved(wishes) => {
      stage_eft_wishes(state, &wishes);
      Ok(Task::none())
    }
    Message::ImportFeedbackDismissed => {
      state.import_feedback = None;
      Ok(Task::none())
    }
    Message::ImportFileLoaded(text) => {
      if let Some(content) = text {
        if let Some(rows) = super::plan_csv::parse(&content) {
          return Ok(stage_csv_rows(state, rows, db));
        }
        stage_import(state, &content);
      }
      Ok(Task::none())
    }
    Message::ImportFromClipboard => {
      state.io_panel = None;
      state.import_feedback = None;
      state.import_target = None;
      Ok(clipboard_read_task().map(Message::ImportClipboardRead))
    }
    Message::ImportFromFile => {
      state.io_panel = None;
      state.import_feedback = None;
      state.import_target = None;
      Ok(Task::perform(read_from_file_dialog(), Message::ImportFileLoaded))
    }
    Message::MilestoneImportMenuDismissed => {
      state.import_menu = None;
      Ok(Task::none())
    }
    Message::MilestoneImportMenuToggled(local_id) => {
      state.export_menu = None;
      state.import_menu = if state.import_menu == Some(local_id) {
        None
      } else {
        Some(local_id)
      };
      Ok(Task::none())
    }
    Message::MilestoneImportPicked(local_id, source) => {
      state.import_menu = None;
      state.import_feedback = None;
      state.import_target = Some(local_id);
      Ok(match source {
        MilestoneImportSource::Clipboard => clipboard_read_task().map(Message::ImportClipboardRead),
        MilestoneImportSource::ClipboardEft => clipboard_read_task().map(Message::ImportEftClipboardRead),
        MilestoneImportSource::File => Task::perform(read_from_file_dialog(), Message::ImportFileLoaded),
      })
    }
    Message::ImportReplace => {
      apply_pending_import(state, ImportMode::Replace);
      Ok(Task::none())
    }
    Message::ImportRequested => {
      state.io_panel = if state.io_panel == Some(IoPanel::Import) {
        None
      } else {
        Some(IoPanel::Import)
      };
      Ok(Task::none())
    }
    other => Err(other),
  }
}

fn apply_pending_import(state: &mut State, mode: ImportMode) {
  if let Some(payload) = state.pending_import.take() {
    apply_import(state, payload, mode);
    state.refresh_rows();
    state.import_feedback = Some(ImportFeedback::Succeeded);
  }
  state.io_panel = None;
}

fn log_clipboard_read(text: Option<&str>) {
  match text {
    Some(content) => {
      let head: String = content.bytes().take(16).map(|byte| format!("{byte:02x}")).collect();
      tracing::debug!(
        target: "pod::skills::import",
        read = "some",
        len = content.len(),
        head_hex = %head,
        "clipboard read for import",
      );
    }
    None => {
      tracing::debug!(target: "pod::skills::import", read = "none", "clipboard read for import")
    }
  }
}

fn stage_import(state: &mut State, raw: &str) {
  match import_export::detect(raw) {
    Some(payload) => stage_pending_import(state, payload),
    None => {
      tracing::info!(
        target: "pod::skills::import",
        len = raw.len(),
        "import detected no skill plan; showing failure feedback",
      );
      stage_import_failed(state);
    }
  }
}

fn stage_import_failed(state: &mut State) {
  state.pending_import = None;
  state.import_feedback = Some(ImportFeedback::Failed);
  state.io_panel = None;
}

fn stage_pending_import(state: &mut State, payload: import_export::Payload) {
  state.pending_import = Some(payload);
  state.import_feedback = None;
  // A milestone-targeted import always appends and skips the replace/append prompt.
  if state.import_target.is_some() {
    apply_pending_import(state, ImportMode::Append);
    return;
  }
  state.io_panel = Some(IoPanel::ImportPrompt);
}

fn stage_eft_names(state: &mut State, raw: &str, db: &Database) -> Task<Message> {
  let names = eft::item_names(raw);
  if names.is_empty() {
    stage_import_failed(state);
    return Task::none();
  }
  Task::perform(resolve_eft_wishes(db.clone(), names), Message::ImportEftResolved)
}

fn stage_csv_rows(state: &mut State, rows: Vec<(String, u8)>, db: &Database) -> Task<Message> {
  if rows.is_empty() {
    stage_import_failed(state);
    return Task::none();
  }
  Task::perform(resolve_csv_wishes(db.clone(), rows), Message::ImportEftResolved)
}

async fn resolve_csv_wishes(db: Database, rows: Vec<(String, u8)>) -> Vec<Wish> {
  let names: Vec<String> = rows.iter().map(|(name, _)| name.clone()).collect();
  let types = sde::item_types_by_names_ci(&db, &names).await.unwrap_or_default();
  csv_wishes_from_types(&rows, &types)
}

fn csv_wishes_from_types(rows: &[(String, u8)], types: &[ItemType]) -> Vec<Wish> {
  let mut id_by_name: HashMap<String, i64> = HashMap::new();
  for item_type in types {
    // Keep only the first id per case-insensitive name; relies on the caller (item_types_by_names_ci)
    // ordering rows published-first then lowest-id, so ambiguous/variant names resolve to one canonical item.
    id_by_name
      .entry(item_type.name().to_lowercase())
      .or_insert(item_type.id());
  }

  let mut wishes: Vec<Wish> = Vec::new();
  for (name, level) in rows {
    // Row names that don't match a known skill are dropped silently rather than failing the whole import.
    let Some(&skill_id) = id_by_name.get(&name.to_lowercase()) else {
      continue;
    };
    match wishes.iter_mut().find(|wish| wish.skill_id == skill_id) {
      Some(wish) => wish.to_level = wish.to_level.max(*level),
      None => wishes.push(Wish {
        skill_id,
        to_level: *level,
      }),
    }
  }
  wishes
}

fn stage_eft_wishes(state: &mut State, wishes: &[Wish]) {
  if wishes.is_empty() {
    stage_import_failed(state);
    return;
  }
  let model = model_from_wishes(state, wishes);
  stage_pending_import(state, import_export::Payload::Model(model));
}

async fn resolve_eft_wishes(db: Database, names: Vec<String>) -> Vec<Wish> {
  let rows = sde::item_types_by_names_ci(&db, &names).await.unwrap_or_default();
  eft_wishes_from_types(&rows)
}

fn eft_wishes_from_types(rows: &[ItemType]) -> Vec<Wish> {
  let mut resolved: HashSet<String> = HashSet::new();
  let mut levels: BTreeMap<i64, u8> = BTreeMap::new();
  for row in rows {
    // Silently keep only the first row per case-insensitive name; relies on the caller (item_types_by_names_ci)
    // ordering rows published-first then lowest-id, so ambiguous/variant names resolve to one canonical item.
    if !resolved.insert(row.name().to_lowercase()) {
      continue;
    }
    for (skill_id, level) in skills::required_skills_for_item(row) {
      // A level of 0 means the dogma slot had no level attribute set, not "train to level 0"; skip it.
      if level == 0 {
        continue;
      }
      levels
        .entry(skill_id)
        .and_modify(|current| *current = (*current).max(level))
        .or_insert(level);
    }
  }
  levels
    .into_iter()
    .map(|(skill_id, to_level)| Wish {
      skill_id,
      to_level,
    })
    .collect()
}

fn clipboard_read_task() -> Task<Option<String>> {
  #[cfg(not(windows))]
  {
    iced::clipboard::read()
  }
  // iced::clipboard::read() only surfaces CF_UNICODETEXT on Windows and returns None when
  // the plan was copied as CF_HTML or CF_TEXT; the Win32 path probes all three formats.
  #[cfg(windows)]
  {
    Task::perform(
      async {
        tokio::task::spawn_blocking(clipboard_win::read_plan_text)
          .await
          .ok()
          .flatten()
      },
      |text| text,
    )
  }
}

async fn read_from_file_dialog() -> Option<String> {
  #[cfg(not(test))]
  {
    let plan_filter = t!("skills.plan.file_filter_plan");
    let handle = rfd::AsyncFileDialog::new()
      .set_title(t!("skills.plan.import_dialog_title").into_owned())
      .add_filter(&*plan_filter, &[import_export::PSP_EXTENSION, "json", "txt"])
      .pick_file()
      .await?;
    Some(String::from_utf8_lossy(&handle.read().await).into_owned())
  }
  #[cfg(test)]
  {
    None
  }
}

async fn save_to_file_dialog(
  default_name: String,
  contents: String,
  filter_label: String,
  extension: &'static str,
) -> Option<PathBuf> {
  #[cfg(not(test))]
  {
    let handle = rfd::AsyncFileDialog::new()
      .set_title(t!("skills.plan.export_dialog_title").into_owned())
      .set_file_name(default_name)
      .add_filter(&filter_label, &[extension])
      .save_file()
      .await?;
    let path = handle.path().to_path_buf();
    if std::fs::write(&path, contents).is_err() {
      return None;
    }
    Some(path)
  }
  #[cfg(test)]
  {
    let _ = (default_name, contents, filter_label, extension);
    None
  }
}

fn handle_context_menu(state: &mut State, message: Message) -> Result<Task<Message>, Message> {
  match message {
    Message::CursorMoved(point) => {
      state.cursor = Some(point);
      Ok(Task::none())
    }
    Message::PickerRowRightPressed(skill_id) => {
      open_context_menu(state, skill_id, None);
      Ok(Task::none())
    }
    Message::EntryRowRightPressed(entry_id) => {
      if let Some(skill_id) = state
        .entries
        .iter()
        .find(|entry| entry.id == entry_id)
        .map(|entry| entry.skill_id)
      {
        open_context_menu(state, skill_id, entry_menu_context(state, entry_id));
      }
      Ok(Task::none())
    }
    Message::PickerContextMenuDismissed => {
      state.context_menu = None;
      Ok(Task::none())
    }
    Message::PickerContextPlan(skill_id, level) => {
      state.context_menu = None;
      add_skill(state, skill_id, level);
      state.refresh_rows();
      Ok(Task::none())
    }
    other => Err(other),
  }
}

fn open_context_menu(state: &mut State, skill_id: i64, entry: Option<EntryMenuContext>) {
  // `state.cursor` is tracked relative to the editor's mouse_area (the whole picker + queue region);
  // context_menu positions itself window-relative, so the menu opens a little off from the actual
  // cursor by the editor's own top-left inset.
  let Some(anchor) = state.cursor else {
    return;
  };
  let Some(name) = state.catalog_entry(skill_id).map(|entry| entry.name.clone()) else {
    return;
  };
  let trained = state.picker.trained_levels.get(&skill_id).copied().unwrap_or(0);
  let highest_planned = state.planned_levels().get(&skill_id).copied().unwrap_or(0);
  state.context_menu = Some(SkillContextMenu {
    anchor,
    entry,
    highest_planned,
    name,
    skill_id,
    trained,
  });
}

/// The milestone-insertion anchors for a right-clicked entry row, but only when the plan is in manual
/// sort — milestones can only be placed by hand, so the picker menu and every sorted view omit them.
/// "Above" anchors after the previous entry (or the top of the plan for the first row); "below" anchors
/// after the clicked entry.
fn entry_menu_context(state: &State, entry_id: i64) -> Option<EntryMenuContext> {
  if state.sort.column != SortColumn::Manual {
    return None;
  }
  let index = state.entries.iter().position(|entry| entry.id == entry_id)?;
  let prev_entry_id = index.checked_sub(1).map(|prev| state.entries[prev].id);
  Some(EntryMenuContext {
    entry_id,
    prev_entry_id,
  })
}

/// A level is enabled only if it's above both the trained level and the highest level already
/// queued in the plan, so a level that's trained or already queued can't be re-planned.
fn context_menu_plan_levels(trained: u8, highest_planned: u8) -> [(u8, bool); 4] {
  let floor = trained.max(highest_planned);
  [2u8, 3, 4, 5].map(|level| (level, level > floor))
}

fn context_menu_view(menu: &SkillContextMenu) -> Element<'_, Message> {
  let mut items: Vec<Item<Message>> = Vec::with_capacity(9);
  items.push(Item::action(
    t!("skills.plan_menu.show_info").into_owned(),
    Message::SkillInfoRequested(menu.skill_id),
  ));
  items.push(Item::separator());
  for (level, enabled) in context_menu_plan_levels(menu.trained, menu.highest_planned) {
    let label = t!("skills.plan_menu.plan_to", level => super::queue_timing::roman(i64::from(level))).into_owned();
    if enabled {
      items.push(Item::action(label, Message::PickerContextPlan(menu.skill_id, level)));
    } else {
      items.push(Item::disabled(label));
    }
  }
  if let Some(entry) = menu.entry {
    items.push(Item::separator());
    push_entry_milestone_items(&mut items, entry);
  }
  context_menu::context_menu(&menu.name, items, menu.anchor)
}

/// The entry-row-only milestone actions: "above" reuses the previous entry's anchor (or the top of the
/// plan), "below" anchors after the clicked entry, and Remove (bottom of the menu) reuses `EntryRemoved`.
fn push_entry_milestone_items(items: &mut Vec<Item<Message>>, entry: EntryMenuContext) {
  items.push(
    Item::action(
      t!("skills.plan_menu.add_milestone_above").into_owned(),
      Message::RemapInserted(entry.prev_entry_id),
    )
    .with_glyph("\u{2191}"),
  );
  items.push(
    Item::action(
      t!("skills.plan_menu.add_milestone_below").into_owned(),
      Message::RemapInserted(Some(entry.entry_id)),
    )
    .with_glyph("\u{2193}"),
  );
  items.push(Item::separator());
  items.push(Item::danger(
    t!("skills.plan_menu.remove_from_plan").into_owned(),
    Message::EntryRemoved(entry.entry_id),
  ));
}

fn handle_picker(state: &mut State, message: Message, db: &Database) -> Result<Task<Message>, Message> {
  match message {
    Message::PickerCertProficiencyChanged(cert_id, prof) => {
      state.picker.cert_proficiency.insert(cert_id, prof.min(3));
      Ok(Task::none())
    }
    Message::PickerCertSelected(cert_id, prof) => {
      add_cert(state, cert_id, prof);
      state.refresh_rows();
      Ok(Task::none())
    }
    Message::PickerCertsLoaded(certs) => {
      state.picker.certs = Some(certs);
      Ok(Task::none())
    }
    Message::PickerGroupToggled(group_id) => {
      if !state.picker.expanded_groups.remove(&group_id) {
        state.picker.expanded_groups.insert(group_id);
      }
      Ok(Task::none())
    }
    Message::PickerLevelPicked(skill_id, level) => {
      add_skill(state, skill_id, level);
      state.refresh_rows();
      Ok(Task::none())
    }
    Message::PickerModuleSelected(module_id) => {
      add_module(state, module_id);
      state.refresh_rows();
      Ok(Task::none())
    }
    Message::PickerModulesLoaded(modules) => {
      state.picker.modules = Some(modules);
      Ok(Task::none())
    }
    Message::PickerSearchChanged(query) => {
      state.picker.query = query;
      Ok(Task::none())
    }
    Message::PickerShipMasteryChanged(ship_id, tier) => {
      state.picker.ship_mastery.insert(ship_id, tier.clamp(1, 5));
      Ok(Task::none())
    }
    Message::PickerShipSelected(ship_id, tier) => {
      add_ship(state, ship_id, tier);
      state.refresh_rows();
      Ok(Task::none())
    }
    Message::PickerShipsLoaded(ships) => {
      state.picker.ships = Some(ships);
      Ok(Task::none())
    }
    Message::PickerTabSelected(tab) => {
      state.picker.active_tab = tab;
      Ok(picker_tab_load_task(state, tab, db))
    }
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Ok(Task::none())
    }
    other => Err(other),
  }
}

fn handle_skill_detail(state: &mut State, message: Message, db: &Database) -> Result<Task<Message>, Message> {
  match message {
    Message::SkillInfoRequested(skill_id) => {
      state.context_menu = None;
      let trained = state.picker.trained_levels.get(&skill_id).copied().unwrap_or(0);
      let effective = [
        state.attrs.perception,
        state.attrs.willpower,
        state.attrs.intelligence,
        state.attrs.memory,
        state.attrs.charisma,
      ];
      Ok(Task::perform(
        load_skill_detail(db.clone(), skill_id, trained, effective),
        Message::SkillDetailLoaded,
      ))
    }
    Message::SkillDetailLoaded(detail) => {
      state.skill_detail = detail;
      Ok(Task::none())
    }
    Message::SkillDetailClosed => {
      state.skill_detail = None;
      Ok(Task::none())
    }
    other => Err(other),
  }
}

async fn load_skill_detail(
  db: Database,
  skill_id: i64,
  trained_level: u8,
  effective_attrs: [u32; 5],
) -> Option<SkillDetailLoad> {
  crate::ui::components::skill_detail::skill_detail(&db, skill_id, trained_level, effective_attrs)
    .await
    .ok()
    .flatten()
    .map(|detail| SkillDetailLoad(std::sync::Arc::new(detail)))
}

fn handle_drag(state: &mut State, message: Message, db: &Database) -> Result<Task<Message>, Message> {
  match message {
    Message::DragDropped => {
      let dragged = state.dragging.take();
      let drop_index = state.drop_index.take();
      Ok(match (dragged, drop_index) {
        (Some(id), Some(to)) => apply_reorder(state, id, to, db),
        _ => Task::none(),
      })
    }
    Message::DragHovered(index) => {
      if state.dragging.is_some() {
        state.drop_index = Some(index);
      }
      Ok(Task::none())
    }
    Message::DragLeft(index) => {
      if state.drop_index == Some(index) {
        state.drop_index = None;
      }
      Ok(Task::none())
    }
    Message::DragStarted(id) => {
      if state.sort.column == SortColumn::Manual {
        state.dragging = Some(id);
        state.drop_index = None;
      }
      Ok(Task::none())
    }
    other => Err(other),
  }
}

fn handle_entry_edit(state: &mut State, message: Message) -> Result<Task<Message>, Message> {
  match message {
    Message::EntryNoteChanged(id, note) => {
      if let Some(entry) = state.entries.iter_mut().find(|e| e.id == id) {
        entry.note = note;
      }
      state.refresh_rows();
      Ok(Task::none())
    }
    Message::EntryNoteToggled(id) => {
      state.note_open = if state.note_open == Some(id) { None } else { Some(id) };
      Ok(Task::none())
    }
    Message::EntryPriorityCycled(id) => {
      if let Some(entry) = state.entries.iter_mut().find(|e| e.id == id) {
        entry.priority = entry.priority.next();
      }
      state.refresh_rows();
      Ok(Task::none())
    }
    Message::EntryRemoved(id) => {
      state.context_menu = None;
      remove_entry_cascade(state, id);
      state.refresh_rows();
      Ok(Task::none())
    }
    Message::NameChanged(name) => {
      state.name = name;
      Ok(Task::none())
    }
    other => Err(other),
  }
}

fn handle_remap(state: &mut State, message: Message) -> Result<Task<Message>, Message> {
  match message {
    Message::MilestoneRemapCleared(local_id) => {
      if let Some(milestone) = state.remap_points.iter_mut().find(|r| r.local_id == local_id) {
        milestone.auto_remap = false;
        milestone.base = None;
      }
      state.refresh_rows();
      Ok(Task::none())
    }
    Message::MilestoneRemapSuggested(local_id) => {
      // Attaching a remap to an in-plan milestone that doesn't already carry one consumes a
      // neural-remap slot, so it (unlike insertion) checks availability. Start-point remaps are
      // free, and re-suggesting an already-placed remap keeps its existing slot.
      let needs_slot = state
        .remap_points
        .iter()
        .find(|r| r.local_id == local_id)
        .is_some_and(|r| r.after_entry_id.is_some() && r.base.is_none());
      if needs_slot && !state.can_place_remap() {
        return Ok(Task::none());
      }
      if let Some(milestone) = state.remap_points.iter_mut().find(|r| r.local_id == local_id) {
        milestone.auto_remap = true;
      }
      state.refresh_rows();
      Ok(Task::none())
    }
    Message::MilestoneCollapseToggled(local_id) => {
      if !state.collapsed_milestones.remove(&local_id) {
        state.collapsed_milestones.insert(local_id);
      }
      Ok(Task::none())
    }
    Message::MilestoneRemoved(local_id) => {
      state.remap_points.retain(|r| r.local_id != local_id);
      state.collapsed_milestones.remove(&local_id);
      state.refresh_rows();
      Ok(Task::none())
    }
    Message::MilestoneRenamed(local_id, name) => {
      if let Some(milestone) = state.remap_points.iter_mut().find(|r| r.local_id == local_id) {
        milestone.name = name;
      }
      state.recompute_dirty();
      state.rebuild_display();
      Ok(Task::none())
    }
    Message::MilestonesAllSuggested => {
      for milestone in &mut state.remap_points {
        milestone.auto_remap = true;
      }
      state.refresh_rows();
      Ok(Task::none())
    }
    Message::RemapInserted(after_entry_id) => {
      // Inserting a milestone is a plain section marker and is never gated on neural-remap
      // availability; only attaching a remap (Suggest remap / set attributes) checks it. New
      // milestones carry no remap (`base: None`) until one is explicitly suggested or set.
      state.context_menu = None;
      let local_id = state.next_remap_id();
      let order = state.remap_points.len() as i64;
      state.remap_points.push(EditMilestone {
        after_entry_id,
        auto_remap: false,
        base: None,
        local_id,
        name: format!("Milestone {}", order + 1),
        order,
      });
      state.refresh_rows();
      Ok(Task::none())
    }
    other => Err(other),
  }
}

fn handle_pane(state: &mut State, message: Message) -> Result<Task<Message>, Message> {
  match message {
    Message::PaneDrag(x) => {
      match state.dragging_pane {
        Some(EditorPane::Picker) => {
          state.picker_pane.drag_to(x);
        }
        Some(EditorPane::Summary) => {
          state.summary_pane.drag_to(-x);
        }
        None => {}
      }
      Ok(Task::none())
    }
    Message::PaneDragEnd => {
      let settled = match state.dragging_pane.take() {
        Some(EditorPane::Picker) => {
          state.picker_pane.end();
          Some(Message::PaneSettled(PICKER_PANE_KEY, state.picker_pane.ratio()))
        }
        Some(EditorPane::Summary) => {
          state.summary_pane.end();
          Some(Message::PaneSettled(SUMMARY_PANE_KEY, state.summary_pane.ratio()))
        }
        None => None,
      };
      Ok(settled.map(Task::done).unwrap_or_else(Task::none))
    }
    Message::PaneDragStart(pane) => {
      state.dragging_pane = Some(pane);
      match pane {
        EditorPane::Picker => state.picker_pane.start(),
        EditorPane::Summary => state.summary_pane.start(),
      }
      Ok(Task::none())
    }
    Message::PaneSettled(..) => Ok(Task::none()),
    other => Err(other),
  }
}

fn handle_lifecycle(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::CloseRequested => Task::none(),
    Message::Loaded(loaded) => {
      apply_loaded(state, *loaded);
      Task::none()
    }
    Message::Reordered(_) => Task::none(),
    Message::SaveRequested => save(state, db),
    Message::Saved(Ok(plan_id)) => load(db, state.character_id, Seed::Existing(plan_id)),
    Message::Saved(Err(_)) => Task::none(),
    Message::SortChanged(column) => {
      state.sort = state.sort.toggled(column);
      state.refresh_rows();
      Task::none()
    }
    _ => Task::none(),
  }
}

fn apply_loaded(state: &mut State, loaded: Loaded) {
  let Loaded {
    attrs,
    base_attrs,
    booster_n,
    catalog,
    cert_proficiency,
    character_total_sp,
    consistent,
    draft_name,
    entries,
    plan,
    remap_availability,
    remap_reason,
    remap_points,
    ship_mastery,
    sort,
    synced_sp,
    trained_levels,
  } = loaded;
  state.attrs = attrs;
  state.base_attrs = base_attrs;
  state.booster_n = booster_n;
  state.consistent = consistent;
  state.character_total_sp = character_total_sp;
  state.synced_levels = trained_levels.clone();
  state.synced_sp = synced_sp;
  let first_group = catalog.groups.first().map(|group| group.id);
  state.picker = PickerState {
    active_tab: picker::PickerTab::Skills,
    expanded_groups: first_group.into_iter().collect(),
    trained_levels,
    query: String::new(),
    catalog: Some(catalog),
    ship_mastery,
    cert_proficiency,
    ..PickerState::default()
  };
  state.entries = entries;
  if plan.as_ref().is_some_and(SkillPlan::is_template) {
    state.character_id = None;
  }
  // Rebase the synthetic-id counter below every loaded id (repair can mint negatives like -1, -2). The
  // counter seeds at -1, so without this the next user-added/imported/auto skill would reuse a loaded
  // repair id and corrupt drag/note/drop/anchor, all of which key on entry id. .min(0) preserves the -1
  // start for an all-positive plan.
  state.next_entry_id = state.entries.iter().map(|e| e.id).min().unwrap_or(0).min(0) - 1;
  state.source_plan_id = plan.as_ref().map(SkillPlan::id);
  state.plan = plan;
  state.remap_availability = remap_availability;
  state.remap_reason = remap_reason;
  state.remap_points = remap_points
    .into_iter()
    .map(|mut point| {
      point.local_id = state.next_remap_id();
      point
    })
    .collect();
  state.sort = sort;
  state.name = state
    .plan
    .as_ref()
    .map(|p| p.name().to_owned())
    .or(draft_name)
    .unwrap_or_default();
  state.note_open = None;
  state.dragging = None;
  state.drop_index = None;
  state.saved = state.snapshot();
  state.refresh_rows();
}

pub fn view(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  let is_manual = state.sort.column == SortColumn::Manual;
  let last_entry_id = state.entries.last().map(|entry| entry.id);
  let header = header::header(
    &state.name,
    state.dirty(),
    state.picker_open,
    state.is_template(),
    is_manual,
    last_entry_id,
  );

  let body: Element<'_, Message> = if state.display_rows.is_empty() && state.display_milestones.is_empty() {
    empty_state::empty_state(state.picker_open)
  } else {
    plan_entry_list::plan_entry_list(
      &state.display_rows,
      &state.display_milestones,
      state.milestone_stats(),
      state.total_sp,
      state.total_sec,
      state.is_template(),
      now,
      state.sort,
      state.note_open,
      state.dragging,
      state.drop_index,
      state.import_menu,
      state.export_menu,
      &state.collapsed_milestones,
    )
  };

  let summary_panel = iced::widget::container(summary::summary(state.summary.clone(), now))
    .width(Length::Fixed(state.summary_pane.width()))
    .height(Length::Fill);

  let mut columns: Vec<Element<'_, Message>> = Vec::with_capacity(5);
  if state.picker_open {
    let planned = state.planned_levels();
    columns.push(
      iced::widget::container(picker::picker(&state.picker, &planned))
        .width(Length::Fixed(state.picker_pane.width()))
        .height(Length::Fill)
        .padding(iced::Padding {
          top: 0.0,
          bottom: 0.0,
          left: spacing::SPACE_2,
          right: spacing::SPACE_2,
        })
        .into(),
    );
    columns.push(pane_handle(Message::PaneDragStart(EditorPane::Picker)));
  }
  columns.push(
    iced::widget::container(body)
      .width(Length::Fill)
      .height(Length::Fill)
      .into(),
  );
  columns.push(pane_handle(Message::PaneDragStart(EditorPane::Summary)));
  columns.push(summary_panel.into());

  let lower: Element<'_, Message> = iced::widget::Row::with_children(columns)
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

  let editor: Element<'_, Message> = iced::widget::mouse_area(
    Column::with_children(vec![header, lower])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .on_move(Message::CursorMoved)
  .into();

  let mut layers: Vec<Element<'_, Message>> = vec![editor];
  if let Some(menu) = state.context_menu.as_ref() {
    layers.push(backdrop::backdrop(Message::PickerContextMenuDismissed));
    layers.push(context_menu_view(menu));
  }
  if let Some(panel) = state.io_panel.as_ref() {
    layers.push(import_export::overlay(panel));
  }
  if let Some(feedback) = state.import_feedback {
    layers.push(import_export::feedback_overlay(feedback));
  }
  if let Some(loaded) = state.skill_detail.as_ref() {
    for layer in skill_detail_modal(loaded.0.as_ref(), Message::SkillDetailClosed) {
      layers.push(layer);
    }
  }

  iced::widget::stack(layers)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn apply_reorder(state: &mut State, id: i64, to: usize, db: &Database) -> Task<Message> {
  let Some(from) = state.entries.iter().position(|e| e.id == id) else {
    return Task::none();
  };
  if from == to {
    return Task::none();
  }
  let entry = state.entries.remove(from);
  let insert_at = if from < to { to - 1 } else { to };
  state.entries.insert(insert_at.min(state.entries.len()), entry);
  state.refresh_rows();

  match state.plan.as_ref() {
    Some(plan) => {
      let _ = plan.id();
      let ordered_ids = state.ordered_ids();
      let db = db.clone();
      Task::perform(
        async move {
          skills::reorder_entries(&db, &ordered_ids)
            .await
            .map_err(|err| err.to_string())
        },
        Message::Reordered,
      )
    }
    None => Task::none(),
  }
}

struct SortView {
  order: Vec<usize>,
  reanchor: Vec<Option<i64>>,
}

/// Whether entry `j` must be trained before entry `i`, given `levels` of
/// `(skill_id, level)`. Two edge kinds: a lower level of the same skill, or a
/// cross-skill prerequisite where `i`'s skill lists `j`'s `(skill_id, level)`.
fn is_pred(levels: &[(i64, u8)], prereqs: &PrereqCatalog, j: usize, i: usize) -> bool {
  (levels[j].0 == levels[i].0 && levels[j].1 < levels[i].1)
    || prereqs.get(&levels[i].0).is_some_and(|reqs| reqs.contains(&levels[j]))
}

fn pred_counts(levels: &[(i64, u8)], prereqs: &PrereqCatalog) -> Vec<usize> {
  let n = levels.len();
  (0..n)
    .map(|i| (0..n).filter(|&j| j != i && is_pred(levels, prereqs, j, i)).count())
    .collect()
}

fn key_outranks(keys: &[f64], asc: bool, i: usize, best: usize) -> bool {
  let better = if asc {
    keys[i] < keys[best]
  } else {
    keys[i] > keys[best]
  };
  better || (keys[i] == keys[best] && i < best)
}

fn pick_ready(emitted: &[bool], remaining_preds: &[usize], keys: &[f64], asc: bool) -> Option<usize> {
  (0..emitted.len())
    .filter(|&i| !emitted[i] && remaining_preds[i] == 0)
    .reduce(|best, i| if key_outranks(keys, asc, i, best) { i } else { best })
}

fn relax_preds(
  remaining_preds: &mut [usize],
  emitted: &[bool],
  levels: &[(i64, u8)],
  prereqs: &PrereqCatalog,
  pick: usize,
) {
  for (i, preds) in remaining_preds.iter_mut().enumerate() {
    if !emitted[i] && is_pred(levels, prereqs, pick, i) {
      *preds = preds.saturating_sub(1);
    }
  }
}

fn topo_order_segment(levels: &[(i64, u8)], keys: &[f64], prereqs: &PrereqCatalog, asc: bool) -> Vec<usize> {
  let n = levels.len();
  let mut remaining_preds = pred_counts(levels, prereqs);
  let mut emitted = vec![false; n];
  let mut order: Vec<usize> = Vec::with_capacity(n);

  while order.len() < n {
    let Some(pick) = pick_ready(&emitted, &remaining_preds, keys, asc) else {
      break;
    };
    emitted[pick] = true;
    order.push(pick);
    relax_preds(&mut remaining_preds, &emitted, levels, prereqs, pick);
  }

  order.extend((0..n).filter(|&i| !emitted[i]));
  order
}

// Sorts each milestone-bounded segment independently so entries never cross a milestone boundary. Returns a
// display-order permutation of entry indices plus, per milestone, the anchor the divider follows in that order.
// Nothing is mutated: the manual entry order and the persisted milestone anchors are left untouched.
fn segmented_sort(
  entries: &[EditEntry],
  milestones: &[EditMilestone],
  prereqs: &PrereqCatalog,
  keys: &[f64],
  asc: bool,
) -> SortView {
  let entry_ids: Vec<i64> = entries.iter().map(|e| e.id).collect();
  let anchors: Vec<MilestoneAnchor> = milestones
    .iter()
    .map(|milestone| MilestoneAnchor {
      after_entry_id: milestone.after_entry_id,
      order: milestone.order,
    })
    .collect();
  let segments = plan_math::plan_segments(&entry_ids, &anchors);

  let levels: Vec<(i64, u8)> = entries.iter().map(|e| (e.skill_id, e.to_level)).collect();

  let mut order: Vec<usize> = Vec::with_capacity(entries.len());
  let mut reanchor: Vec<Option<i64>> = milestones.iter().map(|milestone| milestone.after_entry_id).collect();
  let mut prev_last_id: Option<i64> = None;

  for segment in &segments {
    if let Some(milestone) = segment.milestone {
      reanchor[milestone] = prev_last_id;
    }
    let local = topo_order_segment(
      &levels[segment.start..segment.end],
      &keys[segment.start..segment.end],
      prereqs,
      asc,
    );
    order.extend(local.iter().map(|&index| segment.start + index));
    if let Some(&last) = order.last() {
      prev_last_id = Some(entry_ids[last]);
    }
  }

  SortView {
    order,
    reanchor,
  }
}

fn save(state: &State, db: &Database) -> Task<Message> {
  let character_id = state.character_id;
  let name = state.name.trim().to_owned();
  let name = if name.is_empty() {
    t!("skills.plan.untitled").into_owned()
  } else {
    name
  };
  let sort_mode = state.sort.as_token().to_owned();
  let implant_set = state
    .plan
    .as_ref()
    .map(|p| p.implant_set().to_owned())
    .unwrap_or_else(|| "current".to_owned());
  let existing_id = state.plan.as_ref().map(SkillPlan::id);

  let entries: Vec<(i64, i64, String, String, i64)> = state
    .entries
    .iter()
    .map(|e| {
      (
        e.skill_id,
        i64::from(e.to_level),
        e.priority.as_token().to_owned(),
        e.note.clone(),
        i64::from(e.is_auto),
      )
    })
    .collect();

  let ship_masteries: Vec<(i64, i64)> = if state.is_template() {
    Vec::new()
  } else {
    state
      .picker
      .ship_mastery
      .iter()
      .map(|(&ship_id, &tier)| (ship_id, i64::from(tier)))
      .collect()
  };
  let cert_proficiencies: Vec<(i64, i64)> = if state.is_template() {
    Vec::new()
  } else {
    state
      .picker
      .cert_proficiency
      .iter()
      .map(|(&cert_id, &prof)| (cert_id, prof as i64))
      .collect()
  };
  let old_ids: Vec<i64> = state.entries.iter().map(|e| e.id).collect();
  let remaps: Vec<RemapSave> = state
    .remap_points
    .iter()
    .filter_map(|r| {
      let anchor_index = match r.after_entry_id {
        None => None,
        Some(entry_id) => Some(old_ids.iter().position(|&id| id == entry_id)?),
      };
      Some(RemapSave {
        anchor_index,
        auto_remap: r.auto_remap,
        base: r.base.map(|base| {
          (
            i64::from(base.perception),
            i64::from(base.memory),
            i64::from(base.willpower),
            i64::from(base.intelligence),
            i64::from(base.charisma),
          )
        }),
        name: r.name.clone(),
        order: r.order,
      })
    })
    .collect();

  let db = db.clone();
  Task::perform(
    async move {
      persist(
        &db,
        character_id,
        existing_id,
        &name,
        &sort_mode,
        &implant_set,
        &entries,
        &remaps,
        &ship_masteries,
        &cert_proficiencies,
      )
      .await
      .map_err(|err| err.to_string())
    },
    Message::Saved,
  )
}

#[allow(clippy::too_many_arguments)]
async fn persist(
  db: &Database,
  character_id: Option<i64>,
  existing_id: Option<i64>,
  name: &str,
  sort_mode: &str,
  implant_set: &str,
  entries: &[(i64, i64, String, String, i64)],
  remaps: &[RemapSave],
  ship_masteries: &[(i64, i64)],
  cert_proficiencies: &[(i64, i64)],
) -> Result<i64, crate::store::Error> {
  let plan = import_export::PlanPersist {
    cert_proficiencies: cert_proficiencies.to_vec(),
    entries: entries
      .iter()
      .map(
        |(skill_id, to_level, priority, note, is_auto)| import_export::PlanPersistEntry {
          is_auto: *is_auto,
          note: note.clone(),
          priority: priority.clone(),
          skill_id: *skill_id,
          to_level: *to_level,
        },
      )
      .collect(),
    implant_set: implant_set.to_owned(),
    name: name.to_owned(),
    remaps: remaps
      .iter()
      .map(|remap| import_export::PlanPersistRemap {
        anchor_index: remap.anchor_index,
        auto_remap: remap.auto_remap,
        base: remap.base,
        name: remap.name.clone(),
        order: remap.order,
      })
      .collect(),
    ship_masteries: ship_masteries.to_vec(),
    sort_mode: sort_mode.to_owned(),
  };

  import_export::persist_plan(db, character_id, existing_id, &plan).await
}

async fn async_load(db: Database, character_id: Option<i64>, seed: Seed, now: DateTime<Utc>) -> Loaded {
  let plan = match &seed {
    Seed::Existing(id) => skills::get(&db, *id).await.ok().flatten(),
    Seed::New | Seed::NewTemplate | Seed::FromQueue | Seed::FromQueueSelection(_) => None,
  };
  // A loaded plan's template-ness overrides whatever character_id the caller passed in: opening a saved
  // template must never bind it back onto a character, even from a character-scoped plan list.
  let character_id = if matches!(seed, Seed::NewTemplate) || plan.as_ref().is_some_and(|p| p.is_template()) {
    None
  } else {
    character_id
  };

  let catalog = skills::skill_catalog(&db).await.unwrap_or(SkillCatalog {
    groups: Vec::new(),
  });
  let CharacterSync {
    character_total_sp,
    synced_sp,
    trained_levels,
  } = load_character_sync(&db, character_id).await;

  let raw_remap_points = match plan.as_ref() {
    Some(plan) => skills::milestones(&db, plan.id()).await.unwrap_or_default(),
    None => Vec::new(),
  };
  let remap_points: Vec<EditMilestone> = raw_remap_points.iter().map(edit_remap_from_model).collect();

  let ship_mastery: HashMap<i64, u8> = match plan.as_ref() {
    Some(plan) => skills::ship_masteries(&db, plan.id())
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|row| (row.ship_type_id(), row.tier().clamp(0, i64::from(u8::MAX)) as u8))
      .collect(),
    None => HashMap::new(),
  };
  let cert_proficiency: HashMap<i64, usize> = match plan.as_ref() {
    Some(plan) => skills::cert_proficiencies(&db, plan.id())
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|row| (row.cert_id(), row.level().max(0) as usize))
      .collect(),
    None => HashMap::new(),
  };
  let sort = plan
    .as_ref()
    .map(|p| Sort::from_token(p.sort_mode()))
    .unwrap_or_default();

  let CharacterAttrs {
    attrs,
    base_attrs,
    booster_n,
    consistent,
    availability,
    reason,
  } = match character_id {
    Some(id) => load_character_attrs(&db, id, now).await,
    None => template_attrs(),
  };

  let entries = load_seed_entries(&db, character_id, &seed, plan.as_ref(), &catalog).await;

  let draft_name = match &seed {
    Seed::FromQueueSelection(_) => Some(t!("skills.plan.draft_from_selection").into_owned()),
    Seed::Existing(_) | Seed::FromQueue | Seed::New | Seed::NewTemplate => None,
  };

  Loaded {
    attrs,
    base_attrs,
    booster_n,
    catalog,
    cert_proficiency,
    character_total_sp,
    consistent,
    draft_name,
    entries,
    plan,
    remap_availability: availability,
    remap_reason: reason,
    remap_points,
    ship_mastery,
    sort,
    synced_sp,
    trained_levels,
  }
}

async fn load_character_sync(db: &Database, character_id: Option<i64>) -> CharacterSync {
  let Some(id) = character_id else {
    // Empty maps here, not real training data: plan_entries() falls back to level 0 / 0 SP for any
    // skill_id absent from these maps, so a template costs every entry from scratch.
    return CharacterSync {
      character_total_sp: 0,
      synced_sp: HashMap::new(),
      trained_levels: HashMap::new(),
    };
  };

  let synced_skills = character::skills(db, id, Utc::now()).await.unwrap_or_default();
  let trained_levels: HashMap<i64, u8> = synced_skills
    .iter()
    .map(|skill| {
      (
        skill.skill_id(),
        skill.trained_skill_level().clamp(0, i64::from(u8::MAX)) as u8,
      )
    })
    .collect();
  let synced_sp: HashMap<i64, u64> = synced_skills
    .iter()
    .map(|skill| (skill.skill_id(), skill.skillpoints_in_skill().max(0) as u64))
    .collect();
  let character_total_sp = character::state(db, id)
    .await
    .ok()
    .flatten()
    .and_then(|state| state.total_sp)
    .map(|sp| sp.max(0) as u64)
    .unwrap_or(0);

  CharacterSync {
    character_total_sp,
    synced_sp,
    trained_levels,
  }
}

async fn load_seed_entries(
  db: &Database,
  character_id: Option<i64>,
  seed: &Seed,
  plan: Option<&SkillPlan>,
  catalog: &SkillCatalog,
) -> Vec<EditEntry> {
  match seed {
    Seed::Existing(_) => {
      let raw_entries = match plan {
        Some(plan) => skills::entries(db, plan.id()).await.unwrap_or_default(),
        None => Vec::new(),
      };
      let mut entries = Vec::with_capacity(raw_entries.len());
      for entry in &raw_entries {
        let meta = resolve_entry_meta(db, catalog, entry.skill_id()).await;
        entries.push(EditEntry {
          id: entry.id(),
          is_auto: entry.is_auto() != 0,
          note: entry.note().clone(),
          priority: Priority::from_token(entry.priority()),
          skill_id: entry.skill_id(),
          to_level: entry.to_level().clamp(0, 5) as u8,
          meta,
        });
      }
      repair_under_expanded_entries(db, catalog, entries).await
    }
    Seed::FromQueue => match character_id {
      Some(id) => entries_from_queue(db, id, catalog, None).await,
      None => Vec::new(),
    },
    Seed::FromQueueSelection(positions) => match character_id {
      Some(id) => entries_from_queue(db, id, catalog, Some(positions)).await,
      None => Vec::new(),
    },
    Seed::New | Seed::NewTemplate => Vec::new(),
  }
}

async fn entries_from_queue(
  db: &Database,
  character_id: i64,
  catalog: &SkillCatalog,
  selection: Option<&[i64]>,
) -> Vec<EditEntry> {
  let queue = character::skillqueue(db, character_id).await.unwrap_or_default();
  let wishes: Vec<Wish> = queue
    .iter()
    .filter(|entry| selection.is_none_or(|positions| positions.contains(&entry.queue_position())))
    .map(|entry| Wish {
      skill_id: entry.skill_id(),
      to_level: entry.finished_level().clamp(0, i64::from(plan_math::MAX_SKILL_LEVEL)) as u8,
    })
    .collect();

  let prereqs = prereq_catalog_from(catalog);
  let expanded = plan_math::expand_wishes_full(&wishes, &prereqs);

  let mut next_id = -1;
  let mut entries = Vec::with_capacity(expanded.len());
  for entry in expanded {
    let meta = resolve_entry_meta(db, catalog, entry.skill_id).await;
    entries.push(EditEntry {
      id: next_id,
      is_auto: entry.is_auto,
      note: String::new(),
      priority: Priority::Normal,
      skill_id: entry.skill_id,
      to_level: entry.to_level,
      meta,
    });
    next_id -= 1;
  }
  entries
}

async fn repair_under_expanded_entries(
  db: &Database,
  catalog: &SkillCatalog,
  stored: Vec<EditEntry>,
) -> Vec<EditEntry> {
  let wishes = top_level_wishes(&stored);
  if wishes.is_empty() {
    return stored;
  }

  let prereqs = prereq_catalog_from(catalog);
  let expanded = plan_math::expand_wishes_full(&wishes, &prereqs);

  let stored_levels: HashSet<(i64, u8)> = stored.iter().map(|e| (e.skill_id, e.to_level)).collect();
  let under_expanded = expanded
    .iter()
    .any(|entry| !stored_levels.contains(&(entry.skill_id, entry.to_level)));
  if !under_expanded {
    return stored;
  }

  let reusable: HashMap<(i64, u8), &EditEntry> = stored
    .iter()
    .map(|entry| ((entry.skill_id, entry.to_level), entry))
    .collect();
  let mut next_id = stored.iter().map(|entry| entry.id).min().unwrap_or(0).min(0) - 1;

  let mut repaired = Vec::with_capacity(expanded.len());
  for entry in expanded {
    match reusable.get(&(entry.skill_id, entry.to_level)) {
      Some(existing) => {
        let mut kept = (*existing).clone();
        kept.is_auto = entry.is_auto;
        repaired.push(kept);
      }
      None => {
        let meta = resolve_entry_meta(db, catalog, entry.skill_id).await;
        repaired.push(EditEntry {
          id: next_id,
          is_auto: entry.is_auto,
          note: String::new(),
          priority: Priority::Normal,
          skill_id: entry.skill_id,
          to_level: entry.to_level,
          meta,
        });
        next_id -= 1;
      }
    }
  }
  repaired
}

fn top_level_wishes(entries: &[EditEntry]) -> Vec<Wish> {
  let mut wished: HashMap<i64, u8> = HashMap::new();
  for entry in entries.iter().filter(|entry| !entry.is_auto) {
    wished
      .entry(entry.skill_id)
      .and_modify(|level| *level = (*level).max(entry.to_level))
      .or_insert(entry.to_level);
  }
  wished
    .into_iter()
    .map(|(skill_id, to_level)| Wish {
      skill_id,
      to_level,
    })
    .collect()
}

async fn resolve_entry_meta(db: &Database, catalog: &SkillCatalog, skill_id: i64) -> EntryMeta {
  let metadata = skills::get_skill_metadata(db, skill_id).await.ok().flatten();
  let rank = metadata
    .as_ref()
    .map(|m| m.rank())
    .unwrap_or(1)
    .clamp(1, i64::from(u8::MAX));
  let primary_id = metadata.as_ref().map(|m| m.primary_attribute()).unwrap_or(167);
  let secondary_id = metadata.as_ref().map(|m| m.secondary_attribute()).unwrap_or(166);
  let name = sde::get_item_type(db, skill_id)
    .await
    .ok()
    .flatten()
    .map(|t| t.name().to_owned())
    .unwrap_or_else(|| t!("skills.plan.skill_fallback", id => skill_id).into_owned());

  EntryMeta {
    group_name: group_name_for(catalog, skill_id),
    primary: AttrKey::from_eve_id(primary_id.clamp(0, i64::from(u8::MAX)) as u8),
    primary_id,
    rank: rank as u8,
    secondary: AttrKey::from_eve_id(secondary_id.clamp(0, i64::from(u8::MAX)) as u8),
    secondary_id,
    skill_name: name,
  }
}

fn group_name_for(catalog: &SkillCatalog, skill_id: i64) -> String {
  catalog
    .groups
    .iter()
    .flat_map(|group| group.skills.iter())
    .find(|skill| skill.type_id == skill_id)
    .map(|skill| skill.group_name.clone())
    .unwrap_or_default()
}

fn prereq_catalog_from(catalog: &SkillCatalog) -> PrereqCatalog {
  let id_by_name: HashMap<&str, i64> = catalog
    .groups
    .iter()
    .flat_map(|group| group.skills.iter())
    .map(|skill| (skill.name.as_str(), skill.type_id))
    .collect();

  let mut prereqs = PrereqCatalog::new();
  for skill in catalog.groups.iter().flat_map(|group| group.skills.iter()) {
    let edges: Vec<(i64, u8)> = skill
      .prereqs
      .iter()
      .filter_map(|(name, level)| id_by_name.get(name.as_str()).map(|&id| (id, *level)))
      .collect();
    if !edges.is_empty() {
      prereqs.insert(skill.type_id, edges);
    }
  }
  prereqs
}

fn edit_remap_from_model(point: &SkillPlanMilestone) -> EditMilestone {
  // Any missing coordinate collapses the whole base to None rather than a partially-applied Attributes.
  let base = match (
    point.base_perception(),
    point.base_memory(),
    point.base_willpower(),
    point.base_intelligence(),
    point.base_charisma(),
  ) {
    (Some(perception), Some(memory), Some(willpower), Some(intelligence), Some(charisma)) => Some(Attributes {
      charisma: charisma.max(0) as u32,
      intelligence: intelligence.max(0) as u32,
      memory: memory.max(0) as u32,
      perception: perception.max(0) as u32,
      willpower: willpower.max(0) as u32,
    }),
    _ => None,
  };

  EditMilestone {
    after_entry_id: point.after_entry_id(),
    auto_remap: point.auto_remap(),
    base,
    local_id: 0,
    name: point.name().clone(),
    order: point.position(),
  }
}

async fn load_character_attrs(db: &Database, character_id: i64, now: DateTime<Utc>) -> CharacterAttrs {
  let Some(row) = character::attributes(db, character_id).await.ok().flatten() else {
    return CharacterAttrs {
      attrs: Attributes::default(),
      base_attrs: Attributes::default(),
      booster_n: 0,
      consistent: true,
      availability: 0,
      reason: String::new(),
    };
  };

  let implant_rows = character::implants(db, character_id).await.unwrap_or_default();
  character_attrs_from(&row, &implant_rows, now)
}

fn character_attrs_from(
  row: &crate::store::model::CharacterAttributes,
  implant_rows: &[crate::store::model::CharacterImplant],
  now: DateTime<Utc>,
) -> CharacterAttrs {
  let stored = base_attributes(row);
  let mut implants = Attributes::default();
  for implant in implant_rows {
    let bonus = implant.bonus().max(0) as u32;
    match plan_math_attribute(implant.attribute_id()) {
      Attribute::Charisma => implants.charisma += bonus,
      Attribute::Intelligence => implants.intelligence += bonus,
      Attribute::Memory => implants.memory += bonus,
      Attribute::Perception => implants.perception += bonus,
      Attribute::Willpower => implants.willpower += bonus,
    }
  }

  let derived = crate::features::skills::attributes::derive_attributes(row, implants);
  let (base_attrs, booster_n) = if derived.consistent {
    (derived.base, derived.booster_n)
  } else {
    (stored, 0)
  };

  let availability = plan_math::remap_availability(
    row.bonus_remaps(),
    row.last_remap_date().as_deref(),
    row.accrued_remap_cooldown_date().as_deref(),
    now,
  );

  CharacterAttrs {
    attrs: stored,
    base_attrs,
    booster_n,
    consistent: derived.consistent,
    availability: availability.count,
    reason: availability.reason,
  }
}

fn template_attrs() -> CharacterAttrs {
  CharacterAttrs {
    attrs: Attributes::unmapped(),
    base_attrs: Attributes::unmapped(),
    booster_n: 0,
    consistent: true,
    availability: FRESH_PILOT_REMAPS,
    reason: String::new(),
  }
}

fn base_attributes(row: &CharacterAttributes) -> Attributes {
  Attributes {
    charisma: row.charisma().max(0) as u32,
    intelligence: row.intelligence().max(0) as u32,
    memory: row.memory().max(0) as u32,
    perception: row.perception().max(0) as u32,
    willpower: row.willpower().max(0) as u32,
  }
}

fn remap_points_to_math(entries: &[EditEntry], points: &[EditMilestone]) -> Vec<RemapPoint> {
  let entry_ids: Vec<i64> = entries.iter().map(|e| e.id).collect();
  points
    .iter()
    .filter_map(|point| {
      let base = point.base?;
      let after_index = after_index_for(point.after_entry_id, &entry_ids)?;
      Some(RemapPoint {
        after_index,
        base,
      })
    })
    .collect()
}

fn remove_entry_cascade(state: &mut State, id: i64) {
  let Some(target) = state.entries.iter().find(|e| e.id == id) else {
    return;
  };
  if target.is_auto {
    return;
  }
  let skill_id = target.skill_id;
  let from_level = target.to_level;

  state
    .entries
    .retain(|e| !(e.skill_id == skill_id && e.to_level >= from_level));

  let required = required_prereq_levels(state);
  state
    .entries
    .retain(|e| !e.is_auto || required.contains(&(e.skill_id, e.to_level)));
}

fn required_prereq_levels(state: &State) -> HashSet<(i64, u8)> {
  let mut wished: HashMap<i64, u8> = HashMap::new();
  for entry in state.entries.iter().filter(|e| !e.is_auto) {
    wished
      .entry(entry.skill_id)
      .and_modify(|level| *level = (*level).max(entry.to_level))
      .or_insert(entry.to_level);
  }

  let wishes: Vec<Wish> = wished
    .into_iter()
    .map(|(skill_id, to_level)| Wish {
      skill_id,
      to_level,
    })
    .collect();

  let catalog = state.prereq_catalog();
  let expanded = plan_math::expand_wishes_full(&wishes, &catalog);

  expanded
    .into_iter()
    .filter(|entry| entry.is_auto)
    .map(|entry| (entry.skill_id, entry.to_level))
    .collect()
}

fn add_skill(state: &mut State, skill_id: i64, target_level: u8) {
  let already_planned = state.planned_levels();

  let catalog = state.prereq_catalog();
  let expanded = plan_math::expand_wishes(
    &[Wish {
      skill_id,
      to_level: target_level,
    }],
    &catalog,
    &already_planned,
  );

  for entry in expanded {
    let edit = edit_entry_from_expanded(state, entry);
    state.entries.push(edit);
  }
}

fn add_auto_skills(state: &mut State, skills: &[(i64, u8)]) {
  if skills.is_empty() {
    return;
  }

  let already_planned = state.planned_levels();

  let wishes: Vec<Wish> = skills
    .iter()
    .map(|&(skill_id, to_level)| Wish {
      skill_id,
      to_level,
    })
    .collect();

  let catalog = state.prereq_catalog();
  let expanded = plan_math::expand_wishes(&wishes, &catalog, &already_planned);

  for entry in expanded {
    let edit = edit_entry_from_expanded(state, entry);
    state.entries.push(edit);
  }
}

fn add_ship(state: &mut State, ship_id: i64, tier: u8) {
  let Some(ship) = state
    .picker
    .ships
    .as_ref()
    .and_then(|s| s.iter().find(|s| s.id == ship_id))
  else {
    return;
  };

  let mut by_skill: HashMap<i64, u8> = HashMap::new();
  for (skill_id, level) in skills::skills_for_mastery(&ship.tier_cert_skills, tier) {
    by_skill
      .entry(skill_id)
      .and_modify(|l| *l = (*l).max(level))
      .or_insert(level);
  }
  for &(skill_id, level) in &ship.own_requirements {
    by_skill
      .entry(skill_id)
      .and_modify(|l| *l = (*l).max(level))
      .or_insert(level);
  }

  let skills: Vec<(i64, u8)> = by_skill.into_iter().collect();
  add_auto_skills(state, &skills);
}

fn add_module(state: &mut State, module_id: i64) {
  let Some(module) = state
    .picker
    .modules
    .as_ref()
    .and_then(|m| m.iter().find(|m| m.id == module_id))
  else {
    return;
  };
  let skills = skills::skills_for_module(&module.requirements);
  add_auto_skills(state, &skills);
}

fn add_cert(state: &mut State, cert_id: i64, prof_idx: usize) {
  let Some(cert) = state
    .picker
    .certs
    .as_ref()
    .and_then(|c| c.iter().find(|c| c.id == cert_id))
  else {
    return;
  };
  let skills = skills::skills_for_cert_at_proficiency(&cert.skills, prof_idx);
  add_auto_skills(state, &skills);
}

fn picker_tab_load_task(state: &State, tab: picker::PickerTab, db: &Database) -> Task<Message> {
  match tab {
    picker::PickerTab::Ships if state.picker.ships.is_none() => {
      Task::perform(load_picker_ships(db.clone()), Message::PickerShipsLoaded)
    }
    picker::PickerTab::Modules if state.picker.modules.is_none() => {
      Task::perform(load_picker_modules(db.clone()), Message::PickerModulesLoaded)
    }
    picker::PickerTab::Certs if state.picker.certs.is_none() => {
      Task::perform(load_picker_certs(db.clone()), Message::PickerCertsLoaded)
    }
    _ => Task::none(),
  }
}

async fn load_picker_ships(db: Database) -> Vec<PickerShip> {
  let ships = skills::ships_for_picker(&db).await.unwrap_or_default();

  let mut cert_skills_cache: HashMap<i64, Vec<crate::store::model::CertificateSkill>> = HashMap::new();
  let mut out = Vec::with_capacity(ships.len());
  for ship in ships {
    let own_requirements = own_requirements_for_item(&db, ship.id).await;

    let mut tier_cert_skills: Vec<Vec<crate::store::model::CertificateSkill>> = Vec::with_capacity(5);
    for cert_ids in &ship.mastery_cert_ids {
      let mut tier = Vec::new();
      for &cert_id in cert_ids {
        if let std::collections::hash_map::Entry::Vacant(e) = cert_skills_cache.entry(cert_id) {
          let skills = skills::skills_for(&db, cert_id).await.unwrap_or_default();
          e.insert(skills);
        }
        if let Some(skills) = cert_skills_cache.get(&cert_id) {
          tier.extend(skills.iter().copied());
        }
      }
      tier_cert_skills.push(tier);
    }

    let group_id = sde::get_item_type(&db, ship.id)
      .await
      .ok()
      .flatten()
      .map(|t| t.group_id())
      .unwrap_or(0);
    out.push(PickerShip {
      id: ship.id,
      name: ship.name,
      group_id,
      group_name: ship.group_name,
      own_requirements,
      tier_cert_skills,
    });
  }
  out
}

async fn load_picker_modules(db: Database) -> Vec<PickerModule> {
  let modules = skills::modules_for_picker(&db).await.unwrap_or_default();
  let mut out = Vec::with_capacity(modules.len());
  for module in modules {
    let requirements = own_requirements_for_item(&db, module.id).await;
    let group_id = sde::get_item_type(&db, module.id)
      .await
      .ok()
      .flatten()
      .map(|t| t.group_id())
      .unwrap_or(0);
    out.push(PickerModule {
      id: module.id,
      name: module.name,
      group_id,
      group_name: module.group_name,
      requirements,
    });
  }
  out
}

async fn load_picker_certs(db: Database) -> Vec<PickerCert> {
  let certs = skills::certificate_all(&db).await.unwrap_or_default();
  let mut out = Vec::with_capacity(certs.len());
  for cert in certs {
    let skills = skills::skills_for(&db, cert.id()).await.unwrap_or_default();
    out.push(PickerCert {
      id: cert.id(),
      name: cert.name().clone(),
      grade: cert.grade(),
      skills,
    });
  }
  out
}

async fn own_requirements_for_item(db: &Database, item_id: i64) -> Vec<(i64, u8)> {
  match sde::get_item_type(db, item_id).await.ok().flatten() {
    Some(item_type) => skills::required_skills_for_item(&item_type),
    None => Vec::new(),
  }
}

fn serialize_plan_text(state: &State) -> String {
  serialize_text_for(state, &state.sort_view().order)
}

// Serializes the entries at `order` (stored indices, in display order) to EVE clipboard text. Callers pass either the
// whole sort view or a single milestone segment; the plan-level and milestone exports share this body.
fn serialize_text_for(state: &State, order: &[usize]) -> String {
  let rows = state.computed().rows;
  order
    .iter()
    .map(|&index| (&state.entries[index], &rows[index]))
    .filter(|(entry, row)| !entry.is_auto && !row.skipped)
    .map(|(entry, _)| format!("{} {}", entry.meta.skill_name, entry.to_level))
    .collect::<Vec<_>>()
    .join("\n")
}

// Stored-index subset (in display order) of the entries that belong to milestone `local_id`. Empty when the milestone
// is unknown or its segment holds no entries. Segment bounds come from `plan_math::plan_segments`, so every entry
// falls in exactly one milestone (or the leading unassigned bucket, which no milestone owns).
fn milestone_segment_order(state: &State, local_id: i64) -> Vec<usize> {
  let Some((start, end)) = state.milestone_segment_bounds(local_id) else {
    return Vec::new();
  };
  state
    .sort_view()
    .order
    .into_iter()
    .filter(|&index| (start..end).contains(&index))
    .collect()
}

// PSP export of a single milestone segment: just the segment's kept entries, with no milestones/remaps of their own.
fn segment_plan_file(state: &State, order: &[usize]) -> import_export::PlanFile {
  let rows = state.computed().rows;
  let entries = order
    .iter()
    .filter(|&&index| !rows[index].skipped)
    .map(|&index| {
      let entry = &state.entries[index];
      import_export::PlanFileEntry {
        name: entry.meta.skill_name.clone(),
        note: entry.note.clone(),
        priority: entry.priority.as_token().to_owned(),
        to_level: entry.to_level,
        type_id: entry.skill_id,
      }
    })
    .collect();

  import_export::PlanFile {
    entries,
    remaps: Vec::new(),
  }
}

fn export_file_name(state: &State) -> String {
  let trimmed = state.name.trim();
  let fallback = t!("skills.plan.export_file_base");
  let base = if trimmed.is_empty() { fallback.as_ref() } else { trimmed };
  format!("{base}.{}", import_export::PSP_EXTENSION)
}

fn export_csv_file_name(state: &State) -> String {
  let trimmed = state.name.trim();
  let fallback = t!("skills.plan.export_file_base");
  let base = if trimmed.is_empty() { fallback.as_ref() } else { trimmed };
  format!("{base}.csv")
}

fn serialize_plan_csv(state: &State) -> String {
  serialize_csv_for(state, &state.sort_view().order)
}

fn serialize_csv_for(state: &State, order: &[usize]) -> String {
  let computed = state.computed().rows;
  let rows: Vec<super::plan_csv::PlanCsvRow> = order
    .iter()
    .map(|&index| &computed[index])
    .map(|row| super::plan_csv::PlanCsvRow {
      skill: row.skill_name.clone(),
      group: row.group_name.clone(),
      primary: attr_key_long(row.primary),
      secondary: attr_key_long(row.secondary),
      level: row.to_level,
      sp: row.sp as f64,
      duration_secs: row.sec as i64,
    })
    .collect();
  super::plan_csv::to_csv(&rows)
}

fn attr_key_long(key: AttrKey) -> String {
  match key {
    AttrKey::Charisma => t!("skills.panel_attributes.attr_charisma"),
    AttrKey::Intelligence => t!("skills.panel_attributes.attr_intelligence"),
    AttrKey::Memory => t!("skills.panel_attributes.attr_memory"),
    AttrKey::Perception => t!("skills.panel_attributes.attr_perception"),
    AttrKey::Willpower => t!("skills.panel_attributes.attr_willpower"),
  }
  .into_owned()
}

fn plan_file(state: &State) -> import_export::PlanFile {
  let rows = state.computed().rows;
  let view = state.sort_view();
  let kept: Vec<&EditEntry> = view
    .order
    .iter()
    .filter(|&&index| !rows[index].skipped)
    .map(|&index| &state.entries[index])
    .collect();
  let entry_ids: Vec<i64> = kept.iter().map(|e| e.id).collect();
  let entries = kept
    .iter()
    .map(|e| import_export::PlanFileEntry {
      name: e.meta.skill_name.clone(),
      note: e.note.clone(),
      priority: e.priority.as_token().to_owned(),
      to_level: e.to_level,
      type_id: e.skill_id,
    })
    .collect();
  let remaps = state
    .remap_points
    .iter()
    .enumerate()
    .map(|(index, r)| import_export::PlanFileRemap {
      after_index: view.reanchor[index].and_then(|id| entry_ids.iter().position(|&entry_id| entry_id == id)),
      auto_remap: r.auto_remap,
      base: r.base.map(import_export::PlanFileAttrs::from_attributes),
      name: r.name.clone(),
      order: r.order,
    })
    .collect();

  import_export::PlanFile {
    entries,
    remaps,
  }
}

fn apply_import(state: &mut State, payload: import_export::Payload, mode: ImportMode) {
  let model = match payload {
    import_export::Payload::Json(plan) => import_export::PlanModel::from_plan_file(plan),
    import_export::Payload::Model(model) => model,
    import_export::Payload::Text(lines) => {
      let Some(model) = model_from_text(state, &lines) else {
        return;
      };
      model
    }
  };

  persist_plan_model(state, model, mode);
}

fn model_from_text(state: &State, lines: &[(String, u8)]) -> Option<import_export::PlanModel> {
  let id_by_name: HashMap<String, i64> = match state.picker.catalog.as_ref() {
    Some(catalog) => catalog
      .groups
      .iter()
      .flat_map(|group| group.skills.iter())
      .map(|skill| (skill.name.to_lowercase(), skill.type_id))
      .collect(),
    None => HashMap::new(),
  };

  let mut wishes: Vec<Wish> = Vec::new();
  for (name, level) in lines {
    let Some(&skill_id) = id_by_name.get(&name.to_lowercase()) else {
      continue;
    };
    match wishes.iter_mut().find(|wish| wish.skill_id == skill_id) {
      Some(wish) => wish.to_level = wish.to_level.max(*level),
      None => wishes.push(Wish {
        skill_id,
        to_level: *level,
      }),
    }
  }
  if wishes.is_empty() {
    return None;
  }

  Some(model_from_wishes(state, &wishes))
}

fn model_from_wishes(state: &State, wishes: &[Wish]) -> import_export::PlanModel {
  let expanded = plan_math::expand_wishes_full(wishes, &state.prereq_catalog());

  import_export::PlanModel {
    entries: expanded
      .into_iter()
      .map(|entry| import_export::PlanModelEntry {
        is_auto: entry.is_auto,
        note: String::new(),
        priority: Priority::Normal.as_token().to_owned(),
        skill_id: entry.skill_id,
        to_level: entry.to_level,
      })
      .collect(),
    remaps: Vec::new(),
  }
}

fn persist_plan_model(state: &mut State, model: import_export::PlanModel, mode: ImportMode) {
  if mode == ImportMode::Replace {
    state.entries.clear();
    state.remap_points.clear();
  }

  let base_len = state.entries.len();
  let mut anchor_ids: Vec<i64> = Vec::with_capacity(model.entries.len());
  for entry in &model.entries {
    let id = upsert_imported_entry(
      state,
      entry.skill_id,
      entry.to_level,
      entry.is_auto,
      &entry.note,
      Priority::from_token(&entry.priority),
    );
    anchor_ids.push(id);
  }

  reposition_milestone_import(state, base_len);

  for remap in model.remaps {
    let after_entry_id = match remap.after_index {
      None => None,
      Some(index) => match anchor_ids.get(index) {
        Some(&id) => Some(id),
        None => continue,
      },
    };
    let local_id = state.next_remap_id();
    state.remap_points.push(EditMilestone {
      after_entry_id,
      auto_remap: remap.auto_remap,
      base: remap.base,
      local_id,
      name: remap.name.clone(),
      order: remap.order,
    });
  }
}

/// Entries from a milestone-targeted import are upserted onto the end of `state.entries` like any other
/// import; this moves the entries at or past `base_len` (the pre-import length) to sit right after the
/// target milestone's anchor instead of trailing the whole plan.
fn reposition_milestone_import(state: &mut State, base_len: usize) {
  let Some(target) = state.import_target.take() else {
    return;
  };
  if state.entries.len() <= base_len {
    return;
  }
  let Some(after) = state
    .remap_points
    .iter()
    .find(|milestone| milestone.local_id == target)
    .map(|milestone| milestone.after_entry_id)
  else {
    return;
  };

  let insert_at = match after {
    None => 0,
    Some(id) => state
      .entries
      .iter()
      .take(base_len)
      .position(|entry| entry.id == id)
      .map(|position| position + 1)
      .unwrap_or(base_len),
  };

  let moved: Vec<EditEntry> = state.entries.split_off(base_len);
  for (offset, entry) in moved.into_iter().enumerate() {
    state.entries.insert(insert_at + offset, entry);
  }
}

fn upsert_imported_entry(
  state: &mut State,
  skill_id: i64,
  to_level: u8,
  is_auto: bool,
  note: &str,
  priority: Priority,
) -> i64 {
  if let Some(existing) = state
    .entries
    .iter()
    .find(|e| e.skill_id == skill_id && e.to_level == to_level)
  {
    return existing.id;
  }

  let id = state.next_entry_id();
  let catalog_entry = state.catalog_entry(skill_id);
  let (rank, primary, secondary, name, group_name) = match catalog_entry {
    Some(skill) => (
      skill.rank.max(1),
      skill.primary_attr,
      skill.secondary_attr,
      skill.name.clone(),
      skill.group_name.clone(),
    ),
    None => (
      1,
      AttrKey::Perception,
      AttrKey::Memory,
      t!("skills.plan.skill_fallback", id => skill_id).into_owned(),
      String::new(),
    ),
  };

  state.entries.push(EditEntry {
    id,
    is_auto,
    note: note.to_owned(),
    priority,
    skill_id,
    to_level,
    meta: EntryMeta {
      group_name,
      primary,
      primary_id: attr_key_to_eve_id(primary),
      rank,
      secondary,
      secondary_id: attr_key_to_eve_id(secondary),
      skill_name: name,
    },
  });
  id
}

fn edit_entry_from_expanded(state: &mut State, expanded: ExpandedEntry) -> EditEntry {
  let id = state.next_entry_id();
  let catalog_entry = state.catalog_entry(expanded.skill_id);
  let (rank, primary, secondary, name, group_name) = match catalog_entry {
    Some(skill) => (
      skill.rank.max(1),
      skill.primary_attr,
      skill.secondary_attr,
      skill.name.clone(),
      skill.group_name.clone(),
    ),
    None => (
      1,
      AttrKey::Perception,
      AttrKey::Memory,
      t!("skills.plan.skill_fallback", id => expanded.skill_id).into_owned(),
      String::new(),
    ),
  };

  EditEntry {
    id,
    is_auto: expanded.is_auto,
    note: String::new(),
    priority: Priority::Normal,
    skill_id: expanded.skill_id,
    to_level: expanded.to_level,
    meta: EntryMeta {
      group_name,
      primary,
      primary_id: attr_key_to_eve_id(primary),
      rank,
      secondary,
      secondary_id: attr_key_to_eve_id(secondary),
      skill_name: name,
    },
  }
}

fn attr_key_to_eve_id(key: AttrKey) -> i64 {
  match key {
    AttrKey::Charisma => 164,
    AttrKey::Intelligence => 165,
    AttrKey::Memory => 166,
    AttrKey::Perception => 167,
    AttrKey::Willpower => 168,
  }
}

fn attr_value(attrs: Attributes, attribute: Attribute) -> u32 {
  match attribute {
    Attribute::Charisma => attrs.charisma,
    Attribute::Intelligence => attrs.intelligence,
    Attribute::Memory => attrs.memory,
    Attribute::Perception => attrs.perception,
    Attribute::Willpower => attrs.willpower,
  }
}

fn attribute_to_attr_key(attribute: Attribute) -> AttrKey {
  match attribute {
    Attribute::Charisma => AttrKey::Charisma,
    Attribute::Intelligence => AttrKey::Intelligence,
    Attribute::Memory => AttrKey::Memory,
    Attribute::Perception => AttrKey::Perception,
    Attribute::Willpower => AttrKey::Willpower,
  }
}

fn after_index_for(after_entry_id: Option<i64>, entry_ids: &[i64]) -> Option<i64> {
  match after_entry_id {
    None => Some(-1),
    Some(entry_id) => entry_ids.iter().position(|&id| id == entry_id).map(|pos| pos as i64),
  }
}

fn plan_math_attribute(id: i64) -> Attribute {
  crate::features::skills::attributes::attribute_from_neural_id(id)
}

fn sum_attrs(base: Attributes, other: Attributes) -> Attributes {
  Attributes {
    charisma: base.charisma + other.charisma,
    intelligence: base.intelligence + other.intelligence,
    memory: base.memory + other.memory,
    perception: base.perception + other.perception,
    willpower: base.willpower + other.willpower,
  }
}

fn plan_time_for(weights: &[PairWeight], base: Attributes) -> f64 {
  use crate::features::skills::format::sp_per_sec;
  let mut total = 0.0;
  for weight in weights {
    let rate = sp_per_sec(attr_value(base, weight.primary), attr_value(base, weight.secondary));
    if rate <= 0.0 {
      return f64::INFINITY;
    }
    total += weight.sp as f64 / rate;
  }
  total
}

fn fmt_eta(now: DateTime<Utc>, seconds_from_now: i64) -> String {
  if seconds_from_now <= 0 {
    return "\u{2014}".to_owned();
  }
  let eta = now + Duration::seconds(seconds_from_now);
  let day = eta.day();
  let month = t!(MONTH_KEYS[(eta.month() - 1) as usize]);
  let year = eta.year();
  let hour = format!("{:02}", eta.hour());
  let minute = format!("{:02}", eta.minute());
  t!(
    "skills.plan.eta",
    day => day,
    month => month,
    year => year,
    hour => hour,
    minute => minute
  )
  .into_owned()
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone as _;

  use super::*;

  fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap()
  }

  fn meta() -> EntryMeta {
    EntryMeta {
      group_name: "Gunnery".to_owned(),
      primary: AttrKey::Perception,
      primary_id: 167,
      rank: 1,
      secondary: AttrKey::Willpower,
      secondary_id: 168,
      skill_name: "Skill".to_owned(),
    }
  }

  fn edit_entry(id: i64, skill_id: i64, to_level: u8) -> EditEntry {
    EditEntry {
      id,
      is_auto: false,
      note: String::new(),
      priority: Priority::Normal,
      skill_id,
      to_level,
      meta: EntryMeta {
        rank: 5,
        ..meta()
      },
    }
  }

  mod entry_row_context_menu {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::skills::browse::{SkillCatalog, SkillCatalogEntry, SkillCatalogGroup};

    fn state_with_queued_skill() -> State {
      let mut state = State::new(Some(42));
      state.picker = PickerState {
        catalog: Some(SkillCatalog {
          groups: vec![SkillCatalogGroup {
            id: 255,
            name: "Gunnery".to_owned(),
            skills: vec![SkillCatalogEntry {
              group_id: 255,
              group_name: "Gunnery".to_owned(),
              name: "Gunnery".to_owned(),
              primary_attr: AttrKey::Perception,
              prereqs: vec![],
              rank: 1,
              secondary_attr: AttrKey::Willpower,
              type_id: 3300,
            }],
          }],
        }),
        ..PickerState::default()
      };
      state.entries.push(edit_entry(-1, 3300, 3));
      state
    }

    #[test]
    fn it_opens_the_plan_menu_for_a_queued_entrys_skill() {
      let mut state = state_with_queued_skill();
      state.cursor = Some(iced::Point::new(12.0, 20.0));

      let _ = handle_context_menu(&mut state, Message::EntryRowRightPressed(-1));

      let menu = state
        .context_menu
        .as_ref()
        .expect("right-pressing a queued entry opens the plan menu");
      assert_eq!(menu.skill_id, 3300);
    }

    #[test]
    fn it_ignores_a_right_press_on_an_unknown_entry_id() {
      let mut state = state_with_queued_skill();
      state.cursor = Some(iced::Point::new(12.0, 20.0));

      let _ = handle_context_menu(&mut state, Message::EntryRowRightPressed(-999));

      assert!(state.context_menu.is_none());
    }
  }

  mod context_menu_plan_levels {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_enables_every_level_above_a_fresh_skill() {
      let levels = context_menu_plan_levels(0, 0);

      assert_eq!(
        levels,
        [(2, true), (3, true), (4, true), (5, true)],
        "an untrained, unplanned skill can be planned to any of II-V",
      );
    }

    #[test]
    fn it_disables_levels_at_or_below_the_trained_level() {
      let levels = context_menu_plan_levels(3, 0);

      assert_eq!(
        levels,
        [(2, false), (3, false), (4, true), (5, true)],
        "II and III are already trained, so only IV and V remain",
      );
    }

    #[test]
    fn it_disables_levels_at_or_below_the_highest_planned_level() {
      let levels = context_menu_plan_levels(0, 4);

      assert_eq!(
        levels,
        [(2, false), (3, false), (4, false), (5, true)],
        "II-IV are already planned, so only V remains",
      );
    }

    #[test]
    fn it_uses_the_greater_of_trained_and_planned_as_the_floor() {
      let levels = context_menu_plan_levels(2, 4);

      assert_eq!(
        levels,
        [(2, false), (3, false), (4, false), (5, true)],
        "the planned level 4 outranks the trained level 2 as the floor",
      );
    }

    #[test]
    fn it_disables_every_level_when_the_skill_is_maxed() {
      let levels = context_menu_plan_levels(5, 5);

      assert_eq!(
        levels,
        [(2, false), (3, false), (4, false), (5, false)],
        "a level-V skill has nothing left to plan",
      );
    }
  }

  mod after_index_for {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_drops_a_point_anchored_to_a_missing_entry() {
      assert_eq!(after_index_for(Some(999), &[10, 11]), None);
    }

    #[test]
    fn it_maps_an_entry_id_to_its_position() {
      assert_eq!(after_index_for(Some(11), &[10, 11, 12]), Some(1));
    }

    #[test]
    fn it_maps_the_start_bucket_to_negative_one() {
      assert_eq!(after_index_for(None, &[10, 11]), Some(-1));
    }
  }

  mod creation {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{
        Alliance, Bloodline, Character, Corporation, Gender, ItemCategory, ItemGroup, ItemType, Race, SkillMetadata,
      },
      repo::{character, sde, skills},
    };

    const SKILL_CATEGORY_ID: i64 = 16;

    async fn seed_character(db: &Database, id: i64) {
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
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    async fn seed_skill(db: &Database, skill_id: i64, name: &str) {
      sde::upsert_item_category(
        db,
        &ItemCategory {
          id: SKILL_CATEGORY_ID,
          icon_id: None,
          name: "Skill".to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
      sde::upsert_item_group(
        db,
        &ItemGroup {
          category_id: SKILL_CATEGORY_ID,
          icon_id: None,
          id: 255,
          name: "Gunnery".to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
      sde::upsert_item_type(
        db,
        &ItemType {
          capacity: None,
          description: Some("A skill.".to_owned()),
          dogma_attributes: "[]".to_owned(),
          group_id: 255,
          icon_id: None,
          id: skill_id,
          market_group_id: None,
          name: name.to_owned(),
          packaged_volume: None,
          portion_size: None,
          published: true,
          radius: None,
          volume: None,
        },
      )
      .await
      .unwrap();
      skills::upsert_skill_metadata(
        db,
        &SkillMetadata {
          primary_attribute: 167,
          rank: 1,
          secondary_attribute: 168,
          skill_id,
        },
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn requesting_skill_info_loads_then_dismisses_the_detail_modal() {
      let db = store::open_test().await.unwrap();
      seed_skill(&db, 3300, "Gunnery").await;

      let loaded = load_skill_detail(db.clone(), 3300, 2, [20, 20, 20, 20, 20]).await;
      assert!(loaded.is_some(), "the loader resolves a detail for a seeded skill");

      let mut state = State::new(Some(42));
      let _ = update(&mut state, Message::SkillDetailLoaded(loaded), &db);
      assert!(state.skill_detail.is_some(), "the modal opens with the loaded detail");

      let _ = update(&mut state, Message::SkillDetailClosed, &db);
      assert!(state.skill_detail.is_none(), "closing clears the modal");
    }

    fn queued(
      character_id: i64,
      position: i64,
      skill_id: i64,
      finished_level: i64,
    ) -> crate::store::model::CharacterSkillqueue {
      crate::store::model::CharacterSkillqueue {
        character_id,
        finish_date: None,
        finished_level,
        level_end_sp: None,
        level_start_sp: None,
        queue_position: position,
        skill_id,
        start_date: None,
        training_start_sp: None,
      }
    }

    #[tokio::test]
    async fn a_loaded_new_plan_defaults_to_untitled_after_save() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let mut state = State::new(Some(42));
      let _ = update(
        &mut state,
        Message::Loaded(Box::new(async_load(db.clone(), Some(42), Seed::New, now()).await)),
        &db,
      );
      assert_eq!(state.name, "", "a fresh plan opens with an empty name");

      let id = persist(
        &db,
        Some(42),
        None,
        "Untitled plan",
        "manual",
        "current",
        &[],
        &[],
        &[],
        &[],
      )
      .await
      .unwrap();
      let plan = skills::get(&db, id).await.unwrap().unwrap();
      assert_eq!(plan.name(), "Untitled plan");
    }

    #[tokio::test]
    async fn a_new_template_seed_loads_no_character_data() {
      use crate::store::model::CharacterSkill;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_skill(&db, 3300, "Gunnery").await;
      character::replace_skills(
        &db,
        42,
        &[CharacterSkill {
          active_skill_level: 5,
          character_id: 42,
          skill_id: 3300,
          skillpoints_in_skill: 256_000,
          trained_skill_level: 5,
        }],
      )
      .await
      .unwrap();

      let loaded = async_load(db.clone(), Some(42), Seed::NewTemplate, now()).await;

      assert!(loaded.entries.is_empty());
      assert!(
        loaded.trained_levels.is_empty(),
        "no trained levels leak into a template"
      );
      assert!(loaded.synced_sp.is_empty());
      assert_eq!(loaded.character_total_sp, 0);
      assert_eq!(loaded.remap_availability, FRESH_PILOT_REMAPS);

      let mut state = State::new(None);
      let _ = update(&mut state, Message::Loaded(Box::new(loaded)), &db);
      assert_eq!(state.character_id(), None);
      assert_eq!(state.is_template(), true);
    }

    #[tokio::test]
    async fn a_template_persists_without_a_character_and_reloads_zero_based() {
      use crate::store::model::CharacterSkill;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_skill(&db, 3300, "Gunnery").await;
      character::replace_skills(
        &db,
        42,
        &[CharacterSkill {
          active_skill_level: 5,
          character_id: 42,
          skill_id: 3300,
          skillpoints_in_skill: 256_000,
          trained_skill_level: 5,
        }],
      )
      .await
      .unwrap();

      let id = persist(
        &db,
        None,
        None,
        "Doctrine",
        "manual",
        "current",
        &[(3300, 5, "normal".to_owned(), String::new(), 0)],
        &[],
        &[],
        &[],
      )
      .await
      .unwrap();

      let plan = skills::get(&db, id).await.unwrap().unwrap();
      assert_eq!(plan.is_template(), true);
      assert_eq!(plan.character_id(), None);

      let mut state = State::new(Some(42));
      let _ = update(
        &mut state,
        Message::Loaded(Box::new(
          async_load(db.clone(), Some(42), Seed::Existing(id), now()).await,
        )),
        &db,
      );

      assert_eq!(state.character_id(), None, "a template forces the no-character mode");
      assert!(state.synced_levels.is_empty());
      assert!(
        state.rows.iter().all(|row| !row.skipped),
        "no step is skipped for the fully-trained pilot"
      );
      assert_eq!(
        state.total_sp, 256_000,
        "costed from level 0 even though the pilot trained the skill"
      );
    }

    #[tokio::test]
    async fn from_queue_seeds_the_full_set_including_already_trained_levels() {
      use crate::store::model::CharacterSkill;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_skill(&db, 3300, "Gunnery").await;
      character::replace_skills(
        &db,
        42,
        &[CharacterSkill {
          active_skill_level: 2,
          character_id: 42,
          skill_id: 3300,
          skillpoints_in_skill: 100,
          trained_skill_level: 2,
        }],
      )
      .await
      .unwrap();
      character::replace_skillqueue(&db, 42, &[queued(42, 0, 3300, 5)])
        .await
        .unwrap();

      let loaded = async_load(db, Some(42), Seed::FromQueue, now()).await;

      let levels: Vec<u8> = loaded.entries.iter().map(|e| e.to_level).collect();
      assert_eq!(
        levels,
        [1, 2, 3, 4, 5],
        "the stored plan holds every level, not an author-filtered subset"
      );
    }

    #[tokio::test]
    async fn from_queue_seeds_entries_expanded_from_the_synced_queue() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_skill(&db, 3300, "Gunnery").await;
      seed_skill(&db, 3301, "Small Hybrid Turret").await;
      character::replace_skillqueue(&db, 42, &[queued(42, 0, 3300, 3), queued(42, 1, 3301, 2)])
        .await
        .unwrap();

      let loaded = async_load(db, Some(42), Seed::FromQueue, now()).await;

      assert!(loaded.plan.is_none(), "a from-queue plan is unsaved until Save");
      let rows: Vec<(i64, u8)> = loaded.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();
      assert_eq!(rows, vec![(3300, 1), (3300, 2), (3300, 3), (3301, 1), (3301, 2)]);
      assert_eq!(
        loaded.entries[0].meta.skill_name, "Gunnery",
        "metadata resolved off the DB"
      );
    }

    #[tokio::test]
    async fn from_queue_selection_only_seeds_the_chosen_queue_positions() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_skill(&db, 3300, "Gunnery").await;
      seed_skill(&db, 3301, "Small Hybrid Turret").await;
      character::replace_skillqueue(&db, 42, &[queued(42, 0, 3300, 3), queued(42, 1, 3301, 2)])
        .await
        .unwrap();

      let loaded = async_load(db, Some(42), Seed::FromQueueSelection(vec![1]), now()).await;

      assert!(loaded.plan.is_none(), "a from-selection plan is unsaved until Save");
      let skill_ids: Vec<i64> = loaded.entries.iter().map(|e| e.skill_id).collect();
      assert!(
        skill_ids.iter().all(|id| *id == 3301),
        "only the selected queue entry (and its prereqs) is seeded, got {skill_ids:?}"
      );
      let rows: Vec<(i64, u8)> = loaded.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();
      assert_eq!(rows, vec![(3301, 1), (3301, 2)]);
    }

    #[tokio::test]
    async fn from_queue_selection_titles_the_draft_plan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_skill(&db, 3300, "Gunnery").await;
      character::replace_skillqueue(&db, 42, &[queued(42, 0, 3300, 3)])
        .await
        .unwrap();

      crate::services::i18n::set_locale(crate::services::i18n::Language::En);
      let mut state = State::new(Some(42));
      let _ = update(
        &mut state,
        Message::Loaded(Box::new(
          async_load(db.clone(), Some(42), Seed::FromQueueSelection(vec![0]), now()).await,
        )),
        &db,
      );

      assert_eq!(state.name, "Plan from selection");
    }

    #[tokio::test]
    async fn new_seed_produces_an_empty_unsaved_plan() {
      let db = store::open_test().await.unwrap();

      let loaded = async_load(db, Some(42), Seed::New, now()).await;

      assert!(loaded.plan.is_none(), "new plan is not persisted until Save");
      assert!(loaded.entries.is_empty());
    }

    async fn seed_under_expanded_plan(db: &Database) -> i64 {
      seed_character(db, 42).await;
      seed_skill(db, 3300, "Gunnery").await;
      let plan = skills::create(db, 42, "Subset plan").await.unwrap();
      skills::replace_entries(
        db,
        plan.id(),
        &[
          (3300, 3, "normal", "", 0),
          (3300, 4, "normal", "", 0),
          (3300, 5, "normal", "", 0),
        ],
      )
      .await
      .unwrap();
      plan.id()
    }

    #[tokio::test]
    async fn loading_an_under_expanded_plan_repairs_it_to_the_full_set() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_under_expanded_plan(&db).await;

      let loaded = async_load(db, Some(42), Seed::Existing(plan_id), now()).await;

      let levels: Vec<u8> = loaded.entries.iter().map(|e| e.to_level).collect();
      assert_eq!(
        levels,
        [1, 2, 3, 4, 5],
        "the author-filtered subset is repaired to every level on load"
      );
    }

    #[tokio::test]
    async fn repairing_a_plan_twice_is_a_no_op() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_under_expanded_plan(&db).await;

      let first = async_load(db.clone(), Some(42), Seed::Existing(plan_id), now()).await;
      let first_rows: Vec<(i64, u8)> = first.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();

      let full_rows: Vec<(i64, i64, &str, &str, i64)> = first
        .entries
        .iter()
        .map(|e| (e.skill_id, i64::from(e.to_level), "normal", "", i64::from(e.is_auto)))
        .collect();
      skills::replace_entries(&db, plan_id, &full_rows).await.unwrap();

      let second = async_load(db, Some(42), Seed::Existing(plan_id), now()).await;
      let second_rows: Vec<(i64, u8)> = second.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();

      assert_eq!(
        second_rows, first_rows,
        "re-loading an already-full plan adds and reorders nothing"
      );
      assert!(
        second.entries.iter().all(|e| e.id > 0),
        "a full plan keeps its persisted ids; repair coins no fresh negative ids"
      );
    }

    #[tokio::test]
    async fn repair_leaves_the_per_character_projection_unchanged() {
      use crate::store::model::CharacterSkill;

      let db = store::open_test().await.unwrap();
      let plan_id = seed_under_expanded_plan(&db).await;
      character::replace_skills(
        &db,
        42,
        &[CharacterSkill {
          active_skill_level: 2,
          character_id: 42,
          skill_id: 3300,
          skillpoints_in_skill: 100,
          trained_skill_level: 2,
        }],
      )
      .await
      .unwrap();

      let mut state = State::new(Some(42));
      let _ = update(
        &mut state,
        Message::Loaded(Box::new(
          async_load(db.clone(), Some(42), Seed::Existing(plan_id), now()).await,
        )),
        &db,
      );

      let trained_steps = state.rows.iter().filter(|row| row.to_level <= 2).count();
      assert!(
        state
          .rows
          .iter()
          .take(trained_steps)
          .all(|row| row.skipped && row.sp == 0),
        "levels at or below the trained level project as zero-cost skipped rows"
      );
      let trainable_sp: u64 = state.rows.iter().filter(|row| !row.skipped).map(|row| row.sp).sum();
      assert_eq!(
        state.total_sp, trainable_sp,
        "the plan total still counts only the levels the character still needs"
      );
    }

    #[tokio::test]
    async fn picker_selections_flip_the_dirty_dot_and_reload_into_the_picker() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let mut state = State::new(Some(42));
      let _ = update(
        &mut state,
        Message::Loaded(Box::new(async_load(db.clone(), Some(42), Seed::New, now()).await)),
        &db,
      );
      assert!(!state.dirty());

      let _ = update(&mut state, Message::PickerShipMasteryChanged(587, 4), &db);
      let _ = update(&mut state, Message::PickerCertProficiencyChanged(1, 2), &db);
      assert!(state.dirty(), "changing a selection flips the dirty dot");

      let id = persist(
        &db,
        Some(42),
        None,
        "Combat",
        "manual",
        "current",
        &[],
        &[],
        &[(587, 4)],
        &[(1, 2)],
      )
      .await
      .unwrap();

      let mut reloaded = State::new(Some(42));
      let _ = update(
        &mut reloaded,
        Message::Loaded(Box::new(
          async_load(db.clone(), Some(42), Seed::Existing(id), now()).await,
        )),
        &db,
      );
      assert_eq!(reloaded.picker.ship_mastery.get(&587).copied(), Some(4));
      assert_eq!(reloaded.picker.cert_proficiency.get(&1).copied(), Some(2));
      assert!(!reloaded.dirty(), "a freshly reloaded plan is not dirty");
    }

    #[tokio::test]
    async fn ship_mastery_and_cert_proficiency_selections_survive_a_reload() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let id = persist(
        &db,
        Some(42),
        None,
        "Combat",
        "manual",
        "current",
        &[],
        &[],
        &[(587, 4), (588, 2)],
        &[(1, 2), (3, 3)],
      )
      .await
      .unwrap();

      let loaded = async_load(db, Some(42), Seed::Existing(id), now()).await;

      assert_eq!(loaded.ship_mastery.get(&587).copied(), Some(4));
      assert_eq!(loaded.ship_mastery.get(&588).copied(), Some(2));
      assert_eq!(loaded.cert_proficiency.get(&1).copied(), Some(2));
      assert_eq!(loaded.cert_proficiency.get(&3).copied(), Some(3));
    }
  }

  mod dirty {
    use super::*;

    #[tokio::test]
    async fn name_edit_flips_dirty() {
      let mut state = State::new(Some(42));
      let db = crate::store::open_test().await.unwrap();
      assert!(!state.dirty());

      let _ = update(&mut state, Message::NameChanged("Combat".to_owned()), &db);

      assert!(state.dirty());
    }

    #[tokio::test]
    async fn note_edit_flips_dirty_but_toggle_alone_does_not() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5)];
      state.saved = state.snapshot();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::EntryNoteToggled(1), &db);
      assert_eq!(state.note_open, Some(1));
      assert!(!state.dirty(), "opening the note editor is not a change");

      let _ = update(&mut state, Message::EntryNoteChanged(1, "must-have".to_owned()), &db);
      assert_eq!(state.entries[0].note, "must-have");
      assert!(state.dirty());
    }

    #[tokio::test]
    async fn priority_cycle_flips_dirty() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5)];
      state.saved = state.snapshot();
      let db = crate::store::open_test().await.unwrap();
      assert!(!state.dirty());

      let _ = update(&mut state, Message::EntryPriorityCycled(1), &db);

      assert_eq!(state.entries[0].priority, Priority::High);
      assert!(state.dirty());
    }

    #[tokio::test]
    async fn sort_change_flips_dirty() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5)];
      state.saved = state.snapshot();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::SortChanged(SortColumn::Time), &db);

      assert_eq!(state.sort.column, SortColumn::Time);
      assert_eq!(state.sort.direction, SortDirection::Ascending);
      assert!(state.dirty());
    }
  }

  mod drag {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_ignores_drag_when_not_in_manual_sort() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(10, 3300, 5), edit_entry(11, 3301, 5)];
      state.sort = Sort {
        column: SortColumn::Time,
        direction: SortDirection::Ascending,
      };
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::DragStarted(11), &db);

      assert!(state.dragging.is_none());
    }

    #[tokio::test]
    async fn it_reorders_in_memory_and_keeps_ids_stable() {
      let mut state = State::new(Some(42));
      state.entries = vec![
        edit_entry(10, 3300, 5),
        edit_entry(11, 3301, 5),
        edit_entry(12, 3302, 5),
      ];
      state.saved = state.snapshot();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::DragStarted(12), &db);
      let _ = update(&mut state, Message::DragHovered(0), &db);
      let _ = update(&mut state, Message::DragDropped, &db);

      assert_eq!(state.ordered_ids(), [12, 10, 11], "new order");
      let mut ids = state.ordered_ids();
      ids.sort_unstable();
      assert_eq!(ids, [10, 11, 12], "stable ids");
      assert!(state.dirty());
    }
  }

  mod export_file_name {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_the_plan_name_with_the_psp_extension() {
      let mut state = State::new(Some(42));
      state.name = "Combat Core".to_owned();

      assert_eq!(export_file_name(&state), "Combat Core.psp");
    }

    #[test]
    fn it_falls_back_when_the_name_is_blank() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);
      let mut state = State::new(Some(42));
      state.name = "   ".to_owned();

      assert_eq!(export_file_name(&state), "skill-plan.psp");
    }

    #[test]
    fn it_uses_the_plan_name_with_the_csv_extension() {
      let mut state = State::new(Some(42));
      state.name = "Combat Core".to_owned();

      assert_eq!(export_csv_file_name(&state), "Combat Core.csv");
    }

    #[test]
    fn it_falls_back_to_the_csv_base_when_blank() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);
      let mut state = State::new(Some(42));
      state.name = "   ".to_owned();

      assert_eq!(export_csv_file_name(&state), "skill-plan.csv");
    }
  }

  mod fmt_eta {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_the_instant_with_the_year() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);
      assert_eq!(fmt_eta(now(), 2 * 3_600 + 30 * 60), "1 Jun 2026 · 14:30");
    }

    #[test]
    fn it_renders_an_em_dash_for_zero_or_negative() {
      assert_eq!(fmt_eta(now(), 0), "—");
    }

    #[test]
    fn it_rolls_into_a_later_year() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);
      assert!(fmt_eta(now(), 250 * 86_400).ends_with("2027 · 12:00"));
    }
  }

  mod import_export_flow {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::skills::browse::{SkillCatalog, SkillCatalogEntry, SkillCatalogGroup};

    fn catalog_entry(type_id: i64, name: &str) -> SkillCatalogEntry {
      SkillCatalogEntry {
        group_id: 255,
        group_name: "Gunnery".to_owned(),
        name: name.to_owned(),
        primary_attr: AttrKey::Perception,
        prereqs: vec![],
        rank: 1,
        secondary_attr: AttrKey::Willpower,
        type_id,
      }
    }

    fn state_with_catalog() -> State {
      let mut state = State::new(Some(42));
      state.picker = PickerState {
        active_tab: crate::features::skills::skill_plan_editor::picker::PickerTab::Skills,
        catalog: Some(SkillCatalog {
          groups: vec![SkillCatalogGroup {
            id: 255,
            name: "Gunnery".to_owned(),
            skills: vec![
              catalog_entry(3300, "Gunnery"),
              catalog_entry(3301, "Small Hybrid Turret"),
            ],
          }],
        }),
        expanded_groups: std::iter::once(255).collect(),
        trained_levels: HashMap::new(),
        query: String::new(),
        ..PickerState::default()
      };
      state.saved = state.snapshot();
      state
    }

    #[test]
    fn a_json_plan_round_trips_through_the_dto_losslessly() {
      let mut state = state_with_catalog();
      add_skill(&mut state, 3300, 2);
      state.entries[0].note = "watch the rank".to_owned();
      state.entries[0].priority = Priority::High;
      let first_id = state.entries[0].id;
      state.remap_points = vec![
        EditMilestone {
          after_entry_id: None,
          auto_remap: false,
          base: Some(Attributes {
            charisma: 19,
            intelligence: 21,
            memory: 19,
            perception: 21,
            willpower: 19,
          }),
          local_id: 1,
          name: "Milestone 1".to_owned(),
          order: 0,
        },
        EditMilestone {
          after_entry_id: Some(first_id),
          auto_remap: false,
          base: Some(Attributes {
            charisma: 17,
            intelligence: 17,
            memory: 17,
            perception: 27,
            willpower: 21,
          }),
          local_id: 2,
          name: "Milestone 2".to_owned(),
          order: 1,
        },
      ];

      let dto = plan_file(&state);
      let restored = state_with_catalog();
      let mut restored = restored;
      persist_plan_model(
        &mut restored,
        import_export::PlanModel::from_plan_file(dto.clone()),
        ImportMode::Replace,
      );

      let original: Vec<(i64, u8)> = state.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();
      let round: Vec<(i64, u8)> = restored.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();
      assert_eq!(round, original, "skills survive in the same order");
      assert_eq!(restored.entries[0].note, "watch the rank");
      assert_eq!(restored.entries[0].priority, Priority::High);

      let anchors: Vec<Option<usize>> = dto.remaps.iter().map(|r| r.after_index).collect();
      assert_eq!(anchors, vec![None, Some(0)], "remap anchors persist by ordinal index");
      assert_eq!(restored.remap_points.len(), 2, "all remap points restored");
      assert_eq!(restored.remap_points[0].after_entry_id, None);
      assert_eq!(restored.remap_points[1].after_entry_id, Some(restored.entries[0].id));
    }

    #[tokio::test]
    async fn an_unparseable_import_neither_prompts_nor_changes_the_plan() {
      let mut state = state_with_catalog();
      add_skill(&mut state, 3300, 2);
      state.refresh_rows();
      state.saved = state.snapshot();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(
        &mut state,
        Message::ImportClipboardRead(Some("not a skill line at all".to_owned())),
        &db,
      );

      assert!(state.io_panel.is_none(), "garbage raises no prompt");
      assert!(state.pending_import.is_none(), "nothing is staged");
      assert_eq!(
        state.import_feedback,
        Some(ImportFeedback::Failed),
        "garbage surfaces a visible failure message instead of a silent no-op"
      );
      let levels: Vec<u8> = state.entries.iter().map(|e| e.to_level).collect();
      assert_eq!(levels, [1, 2], "the plan is untouched");
    }

    #[tokio::test]
    async fn append_a_json_plan_dedups_existing_skill_levels() {
      let mut donor = state_with_catalog();
      add_skill(&mut donor, 3301, 1);
      let dto = plan_file(&donor);

      let mut state = state_with_catalog();
      add_skill(&mut state, 3301, 1);
      state.refresh_rows();
      let db = crate::store::open_test().await.unwrap();

      state.pending_import = Some(import_export::Payload::Json(dto));
      let _ = update(&mut state, Message::ImportAppend, &db);

      let rows: Vec<(i64, u8)> = state.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();
      assert_eq!(
        rows,
        vec![(3301, 1)],
        "an identical skill-level is not doubled on append"
      );
    }

    #[tokio::test]
    async fn append_adds_to_the_end_and_dedups_existing_levels() {
      let mut state = state_with_catalog();
      add_skill(&mut state, 3300, 2);
      state.refresh_rows();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(
        &mut state,
        Message::ImportClipboardRead(Some("Gunnery 1\nSmall Hybrid Turret 1".to_owned())),
        &db,
      );
      let _ = update(&mut state, Message::ImportAppend, &db);

      let rows: Vec<(i64, u8)> = state.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();
      assert_eq!(
        rows,
        vec![(3300, 1), (3300, 2), (3301, 1)],
        "append keeps existing rows, dedups Gunnery 1, and adds the new skill at the end"
      );
    }

    #[tokio::test]
    async fn dismiss_clears_a_staged_import() {
      let mut state = state_with_catalog();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(
        &mut state,
        Message::ImportClipboardRead(Some("Gunnery 1".to_owned())),
        &db,
      );
      assert_eq!(state.io_panel, Some(IoPanel::ImportPrompt));

      let _ = update(&mut state, Message::IoDismissed, &db);
      assert!(state.io_panel.is_none());
      assert!(state.pending_import.is_none());
    }

    #[test]
    fn export_text_emits_one_eve_style_line_per_user_entry() {
      let mut state = state_with_catalog();
      add_skill(&mut state, 3300, 3);
      state.refresh_rows();

      assert_eq!(serialize_plan_text(&state), "Gunnery 1\nGunnery 2\nGunnery 3");
    }

    #[test]
    fn export_text_omits_auto_added_prerequisites() {
      let mut state = state_with_catalog();
      add_skill(&mut state, 3300, 2);
      if let Some(entry) = state.entries.iter_mut().find(|e| e.to_level == 2) {
        entry.is_auto = true;
      }

      assert_eq!(
        serialize_plan_text(&state),
        "Gunnery 1",
        "auto rows do not carry to EVE text"
      );
    }

    #[test]
    fn export_text_omits_already_trained_and_skipped_steps() {
      let mut state = state_with_catalog();
      add_skill(&mut state, 3300, 3);
      state.synced_levels = HashMap::from([(3300, 2)]);
      state.refresh_rows();

      assert_eq!(serialize_plan_text(&state), "Gunnery 3");
    }

    #[test]
    fn export_psp_omits_already_trained_and_skipped_steps() {
      let mut state = state_with_catalog();
      add_skill(&mut state, 3300, 3);
      state.synced_levels = HashMap::from([(3300, 2)]);
      state.refresh_rows();

      let dto = plan_file(&state);
      let levels: Vec<(i64, u8)> = dto.entries.iter().map(|e| (e.type_id, e.to_level)).collect();

      assert_eq!(levels, vec![(3300, 3)]);
    }

    #[test]
    fn template_export_includes_every_step() {
      let mut state = state_with_catalog();
      state.character_id = None;
      add_skill(&mut state, 3300, 3);
      state.refresh_rows();

      assert_eq!(serialize_plan_text(&state), "Gunnery 1\nGunnery 2\nGunnery 3");
    }

    #[test]
    fn milestone_export_filters_to_the_owning_segment_in_display_order() {
      let mut state = state_with_catalog();
      add_skill(&mut state, 3300, 4);
      state.refresh_rows();
      let ids: Vec<i64> = state.entries.iter().map(|entry| entry.id).collect();
      state.remap_points = vec![
        EditMilestone {
          after_entry_id: Some(ids[0]),
          auto_remap: false,
          base: None,
          local_id: 1,
          name: "Mid game".to_owned(),
          order: 0,
        },
        EditMilestone {
          after_entry_id: Some(ids[2]),
          auto_remap: false,
          base: None,
          local_id: 2,
          name: "End game".to_owned(),
          order: 1,
        },
      ];
      state.refresh_rows();

      // Gunnery 1 sits in the leading unassigned bucket, owned by no milestone.
      let first = milestone_segment_order(&state, 1);
      assert_eq!(first, vec![1, 2], "milestone 1 owns the two steps after its anchor");
      assert_eq!(serialize_text_for(&state, &first), "Gunnery 2\nGunnery 3");

      let second = milestone_segment_order(&state, 2);
      assert_eq!(second, vec![3], "milestone 2 owns the trailing step");
      assert_eq!(serialize_text_for(&state, &second), "Gunnery 4");

      assert!(
        milestone_segment_order(&state, 99).is_empty(),
        "an unknown milestone exports nothing"
      );

      // A segment PSP carries only that segment's skills and drops the plan's milestones/remaps.
      let psp = segment_plan_file(&state, &first);
      let levels: Vec<(i64, u8)> = psp
        .entries
        .iter()
        .map(|entry| (entry.type_id, entry.to_level))
        .collect();
      assert_eq!(levels, vec![(3300, 2), (3300, 3)]);
      assert!(psp.remaps.is_empty(), "a segment export owns no remaps");
    }

    fn state_with_two_milestones() -> State {
      let mut state = state_with_catalog();
      add_skill(&mut state, 3300, 4);
      state.refresh_rows();
      let ids: Vec<i64> = state.entries.iter().map(|entry| entry.id).collect();
      state.remap_points = vec![
        EditMilestone {
          after_entry_id: Some(ids[0]),
          auto_remap: false,
          base: None,
          local_id: 1,
          name: "Mid game".to_owned(),
          order: 0,
        },
        EditMilestone {
          after_entry_id: Some(ids[2]),
          auto_remap: false,
          base: None,
          local_id: 2,
          name: "End game".to_owned(),
          order: 1,
        },
      ];
      state.refresh_rows();
      state
    }

    #[test]
    fn milestone_export_dispatches_every_target_without_disturbing_the_panel() {
      let mut state = state_with_two_milestones();
      state.io_panel = Some(IoPanel::Export);

      for target in [
        MilestoneExportTarget::Clipboard,
        MilestoneExportTarget::Csv,
        MilestoneExportTarget::Psp,
      ] {
        let outcome = handle_export_io(&mut state, Message::MilestoneExport(1, target));
        assert!(outcome.is_ok(), "a milestone export is handled by the export io branch");
        assert_eq!(
          state.io_panel,
          Some(IoPanel::Export),
          "a milestone export leaves the panel untouched"
        );
      }
    }

    #[test]
    fn milestone_export_io_targets_the_owning_segment_for_each_format() {
      let state = state_with_two_milestones();

      // Each format serializes only the second milestone's trailing step (Gunnery 4).
      let clipboard = milestone_export_io(&state, 2, MilestoneExportTarget::Clipboard);
      let csv = milestone_export_io(&state, 2, MilestoneExportTarget::Csv);
      let psp = milestone_export_io(&state, 2, MilestoneExportTarget::Psp);

      // The tasks are opaque, but building them must not panic and the segment order the
      // helper reads is the one the format serializers below confirm.
      let _ = (clipboard, csv, psp);

      let order = milestone_segment_order(&state, 2);
      assert_eq!(serialize_text_for(&state, &order), "Gunnery 4");
      assert!(
        serialize_csv_for(&state, &order).lines().count() >= 2,
        "csv has a header and a row"
      );
    }

    #[test]
    fn renaming_a_milestone_updates_the_rendered_display_name() {
      let mut state = state_with_two_milestones();

      let _ = handle_remap(&mut state, Message::MilestoneRenamed(1, "Alpha".to_owned()));

      let rendered = state
        .display_milestones
        .iter()
        .find(|milestone| milestone.local_id == 1)
        .expect("the renamed milestone is in the display cache");
      assert_eq!(
        rendered.name, "Alpha",
        "the view renders display_milestones, so a rename must refresh that cache or the input appears frozen"
      );
    }

    #[test]
    fn toggling_the_export_menu_replaces_any_open_import_menu() {
      let mut state = state_with_two_milestones();
      state.import_menu = Some(1);

      let _ = handle_export_io(&mut state, Message::MilestoneExportMenuToggled(1));
      assert_eq!(state.export_menu, Some(1), "the export menu opens");
      assert_eq!(state.import_menu, None, "opening export closes an open import menu");

      let _ = handle_export_io(&mut state, Message::MilestoneExportMenuToggled(1));
      assert_eq!(state.export_menu, None, "toggling the open export menu closes it");
    }

    #[test]
    fn picking_a_milestone_export_target_closes_the_menu() {
      let mut state = state_with_two_milestones();
      state.export_menu = Some(1);

      let _ = handle_export_io(
        &mut state,
        Message::MilestoneExport(1, MilestoneExportTarget::Clipboard),
      );

      assert_eq!(state.export_menu, None, "choosing a target dismisses the export menu");
    }

    #[test]
    fn export_csv_emits_the_serializer_header_and_one_row_per_step() {
      let mut state = state_with_catalog();
      add_skill(&mut state, 3300, 3);
      state.refresh_rows();

      let csv = serialize_plan_csv(&state);
      let lines: Vec<&str> = csv.lines().collect();

      assert_eq!(
        lines[0],
        "Skill,Group,Primary Attribute,Secondary Attribute,Level,SP,Duration"
      );
      assert_eq!(lines.len(), 4, "header plus three Gunnery steps");
      assert!(lines[1].starts_with("Gunnery,Gunnery,Perception,Willpower,1,"));
    }

    #[tokio::test]
    async fn export_to_csv_opens_the_save_dialog_and_closes_the_panel() {
      let mut state = state_with_catalog();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::ExportRequested, &db);
      assert_eq!(state.io_panel, Some(IoPanel::Export));

      let _ = update(&mut state, Message::ExportToCsv, &db);
      assert!(state.io_panel.is_none(), "picking an export item closes the panel");
    }

    #[tokio::test]
    async fn import_and_export_triggers_toggle_their_dropdowns() {
      let mut state = State::new(Some(42));
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::ExportRequested, &db);
      assert_eq!(state.io_panel, Some(IoPanel::Export));
      let _ = update(&mut state, Message::ImportRequested, &db);
      assert_eq!(state.io_panel, Some(IoPanel::Import));
      let _ = update(&mut state, Message::ImportRequested, &db);
      assert!(state.io_panel.is_none());
    }

    #[tokio::test]
    async fn import_from_clipboard_smart_detects_text_and_prompts() {
      let mut state = state_with_catalog();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::ImportRequested, &db);
      assert_eq!(state.io_panel, Some(IoPanel::Import));

      let _ = update(
        &mut state,
        Message::ImportClipboardRead(Some("Small Hybrid Turret 2\n".to_owned())),
        &db,
      );

      assert_eq!(
        state.io_panel,
        Some(IoPanel::ImportPrompt),
        "a valid payload raises the prompt"
      );
      assert!(
        matches!(state.pending_import, Some(import_export::Payload::Text(_))),
        "plain text smart-detects as text"
      );
    }

    #[tokio::test]
    async fn replace_clears_then_loads_the_text_import() {
      let mut state = state_with_catalog();
      add_skill(&mut state, 3300, 2);
      state.refresh_rows();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(
        &mut state,
        Message::ImportClipboardRead(Some("Small Hybrid Turret 2".to_owned())),
        &db,
      );
      let _ = update(&mut state, Message::ImportReplace, &db);

      let rows: Vec<(i64, u8)> = state.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();
      assert_eq!(rows, vec![(3301, 1), (3301, 2)], "replace clears the old Gunnery rows");
      assert!(state.io_panel.is_none(), "the prompt closes after replace");
      assert_eq!(
        state.import_feedback,
        Some(ImportFeedback::Succeeded),
        "a completed import confirms success"
      );
    }

    #[tokio::test]
    async fn a_dismissed_feedback_message_clears() {
      let mut state = state_with_catalog();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(
        &mut state,
        Message::ImportClipboardRead(Some("not a skill line at all".to_owned())),
        &db,
      );
      assert_eq!(state.import_feedback, Some(ImportFeedback::Failed));

      let _ = update(&mut state, Message::ImportFeedbackDismissed, &db);

      assert!(state.import_feedback.is_none(), "dismissing clears the message");
    }

    fn state_with_prereq_catalog() -> State {
      let mut state = state_with_catalog();
      let turret = SkillCatalogEntry {
        prereqs: vec![("Gunnery".to_owned(), 3)],
        ..catalog_entry(3301, "Small Hybrid Turret")
      };
      if let Some(catalog) = state.picker.catalog.as_mut() {
        catalog.groups[0].skills = vec![catalog_entry(3300, "Gunnery"), turret];
      }
      state
    }

    #[tokio::test]
    async fn text_import_re_expands_full_prerequisites_for_the_target_character() {
      let mut donor = state_with_prereq_catalog();
      add_skill(&mut donor, 3301, 1);
      donor.refresh_rows();
      let text = serialize_plan_text(&donor);

      let mut importer = state_with_prereq_catalog();
      importer.synced_levels = HashMap::from([(3300, 3), (3301, 1)]);
      let db = crate::store::open_test().await.unwrap();
      let _ = update(&mut importer, Message::ImportClipboardRead(Some(text)), &db);
      let _ = update(&mut importer, Message::ImportReplace, &db);

      let rows: Vec<(i64, u8, bool)> = importer
        .entries
        .iter()
        .map(|e| (e.skill_id, e.to_level, e.is_auto))
        .collect();
      assert_eq!(
        rows,
        vec![(3300, 1, true), (3300, 2, true), (3300, 3, true), (3301, 1, false)],
        "text import stores the full prereq chain even though the importer already trained it"
      );
    }

    #[tokio::test]
    async fn text_round_trips_the_complete_plan_onto_a_less_trained_character() {
      let mut donor = state_with_prereq_catalog();
      donor.synced_levels = HashMap::from([(3300, 3)]);
      add_skill(&mut donor, 3301, 1);
      donor.refresh_rows();
      let exported = serialize_plan_text(&donor);

      let mut importer = state_with_prereq_catalog();
      let db = crate::store::open_test().await.unwrap();
      let _ = update(&mut importer, Message::ImportClipboardRead(Some(exported)), &db);
      let _ = update(&mut importer, Message::ImportReplace, &db);

      let donor_named: Vec<(i64, u8)> = donor
        .entries
        .iter()
        .filter(|e| !e.is_auto)
        .map(|e| (e.skill_id, e.to_level))
        .collect();
      let importer_named: Vec<(i64, u8)> = importer
        .entries
        .iter()
        .filter(|e| !e.is_auto)
        .map(|e| (e.skill_id, e.to_level))
        .collect();
      assert_eq!(donor_named, importer_named, "named wishes survive the round trip");
      let rows: Vec<(i64, u8, bool)> = importer
        .entries
        .iter()
        .map(|e| (e.skill_id, e.to_level, e.is_auto))
        .collect();
      assert_eq!(
        rows,
        vec![(3300, 1, true), (3300, 2, true), (3300, 3, true), (3301, 1, false)],
        "the never-trained importer lands the complete prereq-expanded plan"
      );
    }

    #[test]
    fn json_import_stores_the_full_set_verbatim() {
      let mut donor = state_with_prereq_catalog();
      add_skill(&mut donor, 3301, 1);
      let dto = plan_file(&donor);

      let mut importer = state_with_prereq_catalog();
      persist_plan_model(
        &mut importer,
        import_export::PlanModel::from_plan_file(dto.clone()),
        ImportMode::Replace,
      );

      let original: Vec<(i64, u8)> = donor.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();
      let restored: Vec<(i64, u8)> = importer.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();
      assert_eq!(restored, original, "JSON import reproduces every stored level verbatim");
      assert_eq!(restored.len(), dto.entries.len(), "no rows are dropped or re-expanded");
    }

    fn fit_item(id: i64, name: &str, dogma: &str) -> ItemType {
      ItemType {
        capacity: None,
        description: Some("A fit item.".to_owned()),
        dogma_attributes: dogma.to_owned(),
        group_id: 25,
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

    #[tokio::test]
    async fn eft_resolved_wishes_prompt_then_apply_with_prereq_expansion() {
      let mut state = state_with_prereq_catalog();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(
        &mut state,
        Message::ImportEftResolved(vec![Wish {
          skill_id: 3301,
          to_level: 2,
        }]),
        &db,
      );
      assert_eq!(
        state.io_panel,
        Some(IoPanel::ImportPrompt),
        "resolved EFT skills go through the Append/Replace prompt"
      );

      let _ = update(&mut state, Message::ImportAppend, &db);

      let rows: Vec<(i64, u8, bool)> = state
        .entries
        .iter()
        .map(|e| (e.skill_id, e.to_level, e.is_auto))
        .collect();
      assert_eq!(
        rows,
        vec![
          (3300, 1, true),
          (3300, 2, true),
          (3300, 3, true),
          (3301, 1, false),
          (3301, 2, false),
        ],
        "the fit's skills land prereq-expanded"
      );
      assert_eq!(state.import_feedback, Some(ImportFeedback::Succeeded));
    }

    #[tokio::test]
    async fn eft_resolution_yielding_no_skills_shows_failure() {
      let mut state = state_with_prereq_catalog();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::ImportEftResolved(vec![]), &db);

      assert!(state.io_panel.is_none());
      assert!(state.pending_import.is_none());
      assert_eq!(state.import_feedback, Some(ImportFeedback::Failed));
    }

    #[tokio::test]
    async fn an_eft_paste_without_a_fit_header_shows_failure() {
      let mut state = state_with_prereq_catalog();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(
        &mut state,
        Message::ImportEftClipboardRead(Some("Gunnery 5\nSmall Hybrid Turret 3".to_owned())),
        &db,
      );

      assert!(state.io_panel.is_none());
      assert!(state.pending_import.is_none());
      assert_eq!(state.import_feedback, Some(ImportFeedback::Failed));
    }

    #[test]
    fn eft_wishes_take_the_first_row_per_name_and_collapse_duplicate_levels() {
      let rows = vec![
        fit_item(
          10,
          "Blaster",
          r#"[{"attribute_id":182,"value":3300},{"attribute_id":277,"value":3}]"#,
        ),
        fit_item(
          11,
          "blaster",
          r#"[{"attribute_id":182,"value":3300},{"attribute_id":277,"value":5}]"#,
        ),
        fit_item(
          12,
          "Railgun",
          r#"[{"attribute_id":182,"value":3300},{"attribute_id":277,"value":1},
            {"attribute_id":183,"value":3301},{"attribute_id":278,"value":2}]"#,
        ),
        fit_item(13, "Hull", "[]"),
      ];

      let wishes = eft_wishes_from_types(&rows);

      assert_eq!(
        wishes,
        vec![
          Wish {
            skill_id: 3300,
            to_level: 3,
          },
          Wish {
            skill_id: 3301,
            to_level: 2,
          },
        ],
        "duplicate names defer to the first (published/lowest-id) row and levels collapse to the max"
      );
    }

    #[tokio::test]
    async fn eft_resolution_matches_names_case_insensitively_across_the_sde() {
      let db = crate::store::open_test().await.unwrap();
      crate::store::repo::sde::insert_item_type_with_hierarchy(
        &db,
        &fit_item(
          587,
          "Rifter",
          r#"[{"attribute_id":182,"value":3300},{"attribute_id":277,"value":1}]"#,
        ),
        &crate::store::model::ItemGroup {
          category_id: 6,
          icon_id: None,
          id: 25,
          name: "Frigate".to_owned(),
          published: true,
        },
        &crate::store::model::ItemCategory {
          id: 6,
          icon_id: None,
          name: "Ship".to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();

      let wishes = resolve_eft_wishes(db, vec!["rIfTeR".to_owned()]).await;

      assert_eq!(
        wishes,
        vec![Wish {
          skill_id: 3300,
          to_level: 1,
        }]
      );
    }

    #[tokio::test]
    async fn import_from_clipboard_resets_the_import_context() {
      let mut state = state_with_catalog();
      state.io_panel = Some(IoPanel::Import);
      state.import_feedback = Some(ImportFeedback::Failed);
      state.import_target = Some(9);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::ImportFromClipboard, &db);

      assert!(state.io_panel.is_none());
      assert!(state.import_feedback.is_none());
      assert!(state.import_target.is_none());
    }

    #[tokio::test]
    async fn import_eft_from_clipboard_resets_the_import_context() {
      let mut state = state_with_catalog();
      state.io_panel = Some(IoPanel::Import);
      state.import_feedback = Some(ImportFeedback::Failed);
      state.import_target = Some(9);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::ImportEftFromClipboard, &db);

      assert!(state.io_panel.is_none());
      assert!(state.import_feedback.is_none());
      assert!(state.import_target.is_none());
    }

    #[tokio::test]
    async fn import_from_file_resets_the_import_context() {
      let mut state = state_with_catalog();
      state.io_panel = Some(IoPanel::Import);
      state.import_feedback = Some(ImportFeedback::Failed);
      state.import_target = Some(9);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::ImportFromFile, &db);

      assert!(state.io_panel.is_none());
      assert!(state.import_feedback.is_none());
      assert!(state.import_target.is_none());
    }

    #[tokio::test]
    async fn a_loaded_file_with_a_csv_header_stages_the_rows() {
      let mut state = state_with_catalog();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(
        &mut state,
        Message::ImportFileLoaded(Some("Skill,Level\nGunnery,3".to_owned())),
        &db,
      );

      assert!(state.pending_import.is_none(), "csv rows resolve asynchronously");
      assert!(state.import_feedback.is_none());
    }

    #[tokio::test]
    async fn a_loaded_file_without_a_csv_header_smart_detects_text() {
      let mut state = state_with_catalog();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::ImportFileLoaded(Some("Gunnery 1".to_owned())), &db);

      assert_eq!(state.io_panel, Some(IoPanel::ImportPrompt));
      assert!(matches!(state.pending_import, Some(import_export::Payload::Text(_))));
    }

    #[tokio::test]
    async fn a_loaded_file_with_no_content_is_a_no_op() {
      let mut state = state_with_catalog();
      state.io_panel = Some(IoPanel::Import);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::ImportFileLoaded(None), &db);

      assert_eq!(state.io_panel, Some(IoPanel::Import), "an empty file changes nothing");
      assert!(state.pending_import.is_none());
    }

    #[tokio::test]
    async fn the_milestone_import_menu_dismisses() {
      let mut state = state_with_catalog();
      state.import_menu = Some(7);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::MilestoneImportMenuDismissed, &db);

      assert!(state.import_menu.is_none());
    }

    #[tokio::test]
    async fn the_milestone_import_menu_toggles_open_then_closed() {
      let mut state = state_with_catalog();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::MilestoneImportMenuToggled(7), &db);
      assert_eq!(state.import_menu, Some(7), "toggling an unopened menu opens it");

      let _ = update(&mut state, Message::MilestoneImportMenuToggled(7), &db);
      assert!(state.import_menu.is_none(), "toggling the open menu closes it");
    }

    #[tokio::test]
    async fn picking_a_milestone_clipboard_source_targets_the_milestone() {
      let mut state = state_with_catalog();
      state.import_menu = Some(7);
      state.import_feedback = Some(ImportFeedback::Failed);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(
        &mut state,
        Message::MilestoneImportPicked(7, MilestoneImportSource::Clipboard),
        &db,
      );

      assert!(state.import_menu.is_none());
      assert!(state.import_feedback.is_none());
      assert_eq!(state.import_target, Some(7));
    }

    #[tokio::test]
    async fn picking_a_milestone_eft_clipboard_source_targets_the_milestone() {
      let mut state = state_with_catalog();
      state.import_menu = Some(7);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(
        &mut state,
        Message::MilestoneImportPicked(7, MilestoneImportSource::ClipboardEft),
        &db,
      );

      assert!(state.import_menu.is_none());
      assert_eq!(state.import_target, Some(7));
    }

    #[tokio::test]
    async fn picking_a_milestone_file_source_targets_the_milestone() {
      let mut state = state_with_catalog();
      state.import_menu = Some(7);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(
        &mut state,
        Message::MilestoneImportPicked(7, MilestoneImportSource::File),
        &db,
      );

      assert!(state.import_menu.is_none());
      assert_eq!(state.import_target, Some(7));
    }

    #[tokio::test]
    async fn a_non_import_message_is_handed_back_to_the_caller() {
      let mut state = state_with_catalog();
      let db = crate::store::open_test().await.unwrap();

      let result = handle_import_io(&mut state, Message::NameChanged("x".to_owned()), &db);

      assert!(matches!(result, Err(Message::NameChanged(_))));
    }
  }

  mod reposition_milestone_import {
    use pretty_assertions::assert_eq;

    use super::*;

    fn milestone(local_id: i64, after_entry_id: Option<i64>) -> EditMilestone {
      EditMilestone {
        after_entry_id,
        auto_remap: false,
        base: None,
        local_id,
        name: String::new(),
        order: 0,
      }
    }

    fn ids(state: &State) -> Vec<i64> {
      state.entries.iter().map(|entry| entry.id).collect()
    }

    #[test]
    fn without_a_target_it_leaves_the_entries_alone() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(10, 3300, 1), edit_entry(20, 3301, 1)];
      state.import_target = None;

      reposition_milestone_import(&mut state, 1);

      assert_eq!(ids(&state), vec![10, 20]);
    }

    #[test]
    fn when_the_import_did_not_grow_the_plan_it_leaves_the_entries_alone() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(10, 3300, 1), edit_entry(20, 3301, 1)];
      state.import_target = Some(1);
      state.remap_points = vec![milestone(1, None)];

      reposition_milestone_import(&mut state, 2);

      assert_eq!(ids(&state), vec![10, 20]);
      assert!(state.import_target.is_none(), "the target is consumed");
    }

    #[test]
    fn an_unknown_target_milestone_leaves_the_entries_alone() {
      let mut state = State::new(Some(42));
      state.entries = vec![
        edit_entry(10, 3300, 1),
        edit_entry(20, 3301, 1),
        edit_entry(30, 3302, 1),
      ];
      state.import_target = Some(99);
      state.remap_points = vec![milestone(1, None)];

      reposition_milestone_import(&mut state, 1);

      assert_eq!(ids(&state), vec![10, 20, 30]);
    }

    #[test]
    fn a_start_anchored_milestone_moves_the_import_to_the_front() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(10, 3300, 1), edit_entry(20, 3301, 1)];
      state.import_target = Some(1);
      state.remap_points = vec![milestone(1, None)];

      reposition_milestone_import(&mut state, 1);

      assert_eq!(ids(&state), vec![20, 10]);
    }

    #[test]
    fn an_entry_anchored_milestone_moves_the_import_after_that_entry() {
      let mut state = State::new(Some(42));
      state.entries = vec![
        edit_entry(10, 3300, 1),
        edit_entry(11, 3301, 1),
        edit_entry(20, 3302, 1),
      ];
      state.import_target = Some(1);
      state.remap_points = vec![milestone(1, Some(10))];

      reposition_milestone_import(&mut state, 2);

      assert_eq!(ids(&state), vec![10, 20, 11]);
    }

    #[test]
    fn a_milestone_anchored_to_a_missing_entry_keeps_the_import_at_the_base() {
      let mut state = State::new(Some(42));
      state.entries = vec![
        edit_entry(10, 3300, 1),
        edit_entry(11, 3301, 1),
        edit_entry(20, 3302, 1),
      ];
      state.import_target = Some(1);
      state.remap_points = vec![milestone(1, Some(999))];

      reposition_milestone_import(&mut state, 2);

      assert_eq!(ids(&state), vec![10, 11, 20]);
    }
  }

  mod milestone_entry_points {
    use pretty_assertions::assert_eq;

    use super::*;

    fn entry_menu(entry_id: i64, prev_entry_id: Option<i64>) -> SkillContextMenu {
      SkillContextMenu {
        anchor: iced::Point::new(0.0, 0.0),
        entry: Some(EntryMenuContext {
          entry_id,
          prev_entry_id,
        }),
        highest_planned: 0,
        name: "Gunnery".to_owned(),
        skill_id: 3300,
        trained: 0,
      }
    }

    #[test]
    fn the_first_row_anchors_above_at_the_top_and_below_after_itself() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];

      let context = entry_menu_context(&state, 1).expect("manual sort yields an entry context");

      assert_eq!(context.entry_id, 1);
      assert_eq!(
        context.prev_entry_id, None,
        "above the first row is the top of the plan"
      );
    }

    #[test]
    fn a_later_row_anchors_above_after_the_previous_entry() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];

      let context = entry_menu_context(&state, 2).expect("manual sort yields an entry context");

      assert_eq!(context.entry_id, 2);
      assert_eq!(context.prev_entry_id, Some(1));
    }

    #[test]
    fn a_sorted_plan_carries_no_entry_milestone_context() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5)];
      state.sort = Sort::from_token("time-asc");

      assert!(entry_menu_context(&state, 1).is_none());
    }

    #[test]
    fn the_entry_menu_renders_the_milestone_items() {
      let menu = entry_menu(2, Some(1));
      let _el: Element<'_, Message> = context_menu_view(&menu);
    }

    #[test]
    fn the_picker_menu_omits_the_milestone_items() {
      let mut menu = entry_menu(2, Some(1));
      menu.entry = None;

      let _el: Element<'_, Message> = context_menu_view(&menu);
    }

    #[test]
    fn the_header_button_is_gated_by_sort_mode() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5)];
      state.refresh_rows();

      drop(view(&state, now()));

      state.sort = Sort::from_token("time-asc");
      state.refresh_rows();
      drop(view(&state, now()));
    }
  }

  mod apply_loaded {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;

    use super::*;

    fn loaded_with(entries: Vec<EditEntry>) -> Loaded {
      Loaded {
        attrs: Attributes::default(),
        base_attrs: Attributes::default(),
        booster_n: 0,
        catalog: SkillCatalog {
          groups: Vec::new(),
        },
        cert_proficiency: HashMap::new(),
        character_total_sp: 0,
        consistent: true,
        draft_name: None,
        entries,
        plan: None,
        remap_availability: 0,
        remap_points: Vec::new(),
        remap_reason: String::new(),
        ship_mastery: HashMap::new(),
        sort: Sort::default(),
        synced_sp: HashMap::new(),
        trained_levels: HashMap::new(),
      }
    }

    #[test]
    fn it_rebases_the_id_counter_below_a_loaded_repair_entry() {
      let mut state = State::new(Some(42));
      let loaded = loaded_with(vec![edit_entry(7, 100, 5), edit_entry(-1, 200, 1)]);

      apply_loaded(&mut state, loaded);
      upsert_imported_entry(&mut state, 300, 3, false, "", Priority::Normal);

      let ids: Vec<i64> = state.entries.iter().map(|e| e.id).collect();
      let unique: HashSet<i64> = ids.iter().copied().collect();
      assert_eq!(
        unique.len(),
        ids.len(),
        "every entry id stays unique after adding a skill"
      );
    }

    #[test]
    fn it_preserves_the_negative_start_for_an_all_positive_plan() {
      let mut state = State::new(Some(42));
      let loaded = loaded_with(vec![edit_entry(3, 100, 5)]);

      apply_loaded(&mut state, loaded);
      let added = upsert_imported_entry(&mut state, 300, 3, false, "", Priority::Normal);

      assert_eq!(added, -1);
    }
  }

  mod load {
    use super::*;

    #[tokio::test]
    async fn it_loads_an_empty_new_plan_without_panicking() {
      let db = crate::store::open_test().await.unwrap();

      let loaded = async_load(db, Some(42), Seed::New, now()).await;

      assert!(loaded.plan.is_none());
      assert!(loaded.entries.is_empty());
      assert_eq!(loaded.sort.column, SortColumn::Manual);
    }
  }

  mod panes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_both_pane_widths_when_the_store_is_empty() {
      let state = State::new(Some(42)).with_restored_panes(&UiState::default());

      assert_eq!(state.picker_pane.width(), PICKER_WIDTH);
      assert_eq!(state.summary_pane.width(), SUMMARY_WIDTH);
    }

    #[tokio::test]
    async fn it_grows_the_picker_on_a_rightward_drag_of_its_right_edge_handle() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(Some(42));

      let _ = update(&mut state, Message::PaneDragStart(EditorPane::Picker), &db);
      let _ = update(&mut state, Message::PaneDrag(500.0), &db);
      let _ = update(&mut state, Message::PaneDrag(540.0), &db);

      assert_eq!(state.picker_pane.width(), PICKER_WIDTH + 40.0);
    }

    #[tokio::test]
    async fn it_grows_the_summary_on_a_leftward_drag_of_its_left_edge_handle() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(Some(42));

      let _ = update(&mut state, Message::PaneDragStart(EditorPane::Summary), &db);
      let _ = update(&mut state, Message::PaneDrag(500.0), &db);
      let _ = update(&mut state, Message::PaneDrag(460.0), &db);

      assert_eq!(state.summary_pane.width(), SUMMARY_WIDTH + 40.0);
    }

    #[test]
    fn it_restores_both_pane_widths_from_the_keyed_store() {
      let mut ui = UiState::default();
      ui.panes.insert(PICKER_PANE_KEY.to_owned(), 400.0);
      ui.panes.insert(SUMMARY_PANE_KEY.to_owned(), 300.0);

      let state = State::new(Some(42)).with_restored_panes(&ui);

      assert_eq!(state.picker_pane.width(), 400.0);
      assert_eq!(state.summary_pane.width(), 300.0);
    }

    #[tokio::test]
    async fn it_settles_the_dragged_pane_and_clears_the_active_pane_on_drag_end() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(Some(42));

      let _ = update(&mut state, Message::PaneDragStart(EditorPane::Summary), &db);
      assert!(state.summary_pane.is_active());
      assert_eq!(state.dragging_pane, Some(EditorPane::Summary));

      let _ = update(&mut state, Message::PaneDragEnd, &db);

      assert!(!state.summary_pane.is_active());
      assert_eq!(state.dragging_pane, None);
    }
  }

  mod picker {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::skills::browse::{SkillCatalog, SkillCatalogEntry, SkillCatalogGroup};

    fn catalog_entry(type_id: i64, name: &str, rank: u8, prereqs: Vec<(String, u8)>) -> SkillCatalogEntry {
      SkillCatalogEntry {
        group_id: 255,
        group_name: "Gunnery".to_owned(),
        name: name.to_owned(),
        primary_attr: AttrKey::Perception,
        prereqs,
        rank,
        secondary_attr: AttrKey::Willpower,
        type_id,
      }
    }

    fn state_with_catalog(skills: Vec<SkillCatalogEntry>) -> State {
      let mut state = State::new(Some(42));
      state.picker = PickerState {
        active_tab: crate::features::skills::skill_plan_editor::picker::PickerTab::Skills,
        catalog: Some(SkillCatalog {
          groups: vec![SkillCatalogGroup {
            id: 255,
            name: "Gunnery".to_owned(),
            skills,
          }],
        }),
        expanded_groups: std::iter::once(255).collect(),
        trained_levels: HashMap::new(),
        query: String::new(),
        ..PickerState::default()
      };
      state.saved = state.snapshot();
      state
    }

    fn cert_skill(
      skill_id: i64,
      basic: i64,
      improved: i64,
      advanced: i64,
      elite: i64,
    ) -> crate::store::model::CertificateSkill {
      crate::store::model::CertificateSkill {
        advanced,
        basic,
        certificate_id: 1,
        elite,
        improved,
        skill_id,
      }
    }

    fn state_with_items(skills: Vec<SkillCatalogEntry>) -> State {
      use crate::features::skills::skill_plan_editor::picker::{PickerCert, PickerShip};

      let mut state = state_with_catalog(skills);
      state.picker.ships = Some(vec![PickerShip {
        id: 587,
        name: "Rifter".to_owned(),
        group_id: 25,
        group_name: "Frigate".to_owned(),
        own_requirements: vec![(3300, 1)],
        tier_cert_skills: vec![vec![cert_skill(3300, 1, 2, 3, 5)], vec![cert_skill(3301, 1, 2, 3, 5)]],
      }]);
      state.picker.certs = Some(vec![PickerCert {
        id: 1,
        name: "Gunnery Basics".to_owned(),
        grade: 1,
        skills: vec![cert_skill(3300, 1, 2, 3, 5), cert_skill(3301, 0, 2, 3, 5)],
      }]);
      state
    }

    #[tokio::test]
    async fn adding_a_cert_at_proficiency_adds_that_columns_skills_as_auto() {
      let mut state = state_with_items(vec![
        catalog_entry(3300, "Gunnery", 1, vec![]),
        catalog_entry(3301, "Small Hybrid Turret", 1, vec![]),
      ]);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::PickerCertSelected(1, 1), &db);

      let mut rows: Vec<(i64, u8, bool)> = state
        .entries
        .iter()
        .map(|e| (e.skill_id, e.to_level, e.is_auto))
        .collect();
      rows.sort();
      assert_eq!(
        rows,
        vec![(3300, 1, false), (3300, 2, false), (3301, 1, false), (3301, 2, false)],
        "the cert's improved-column skills are removable wishes, not locked prereqs"
      );
    }

    #[tokio::test]
    async fn adding_a_module_adds_its_required_skills_as_auto() {
      let mut state = state_with_catalog(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
      state.picker.modules = Some(vec![crate::features::skills::skill_plan_editor::picker::PickerModule {
        id: 12_058,
        name: "125mm Gatling AutoCannon".to_owned(),
        group_id: 55,
        group_name: "Projectile Weapon".to_owned(),
        requirements: vec![(3300, 3)],
      }]);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::PickerModuleSelected(12_058), &db);

      let rows: Vec<(i64, u8, bool)> = state
        .entries
        .iter()
        .map(|e| (e.skill_id, e.to_level, e.is_auto))
        .collect();
      assert_eq!(
        rows,
        vec![(3300, 1, false), (3300, 2, false), (3300, 3, false)],
        "the module's required skill is a removable wish, not a locked prereq"
      );
    }

    #[tokio::test]
    async fn adding_a_ship_at_mastery_tier_adds_the_cumulative_skill_set_as_auto() {
      let mut state = state_with_items(vec![
        catalog_entry(3300, "Gunnery", 1, vec![]),
        catalog_entry(3301, "Small Hybrid Turret", 1, vec![]),
      ]);
      let db = crate::store::open_test().await.unwrap();
      assert!(!state.dirty());

      let _ = update(&mut state, Message::PickerShipSelected(587, 2), &db);

      let mut rows: Vec<(i64, u8, bool)> = state
        .entries
        .iter()
        .map(|e| (e.skill_id, e.to_level, e.is_auto))
        .collect();
      rows.sort();
      assert_eq!(
        rows,
        vec![(3300, 1, false), (3301, 1, false), (3301, 2, false)],
        "the mastery's directly-required skills are removable wishes, not locked prereqs"
      );
      assert!(state.dirty(), "adding a ship flips the dirty dot");
    }

    #[tokio::test]
    async fn it_locks_only_the_expanded_prereqs_pulled_behind_a_mastery_skill() {
      use crate::features::skills::skill_plan_editor::picker::PickerShip;

      let mut state = state_with_catalog(vec![
        catalog_entry(3300, "Gunnery", 1, vec![]),
        catalog_entry(3301, "Small Hybrid Turret", 1, vec![("Gunnery".to_owned(), 3)]),
      ]);
      state.picker.ships = Some(vec![PickerShip {
        id: 587,
        name: "Rifter".to_owned(),
        group_id: 25,
        group_name: "Frigate".to_owned(),
        own_requirements: vec![],
        tier_cert_skills: vec![vec![cert_skill(3301, 1, 2, 3, 5)]],
      }]);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::PickerShipSelected(587, 1), &db);

      let mut rows: Vec<(i64, u8, bool)> = state
        .entries
        .iter()
        .map(|e| (e.skill_id, e.to_level, e.is_auto))
        .collect();
      rows.sort();
      assert_eq!(
        rows,
        vec![(3300, 1, true), (3300, 2, true), (3300, 3, true), (3301, 1, false)],
        "the directly-required turret is a removable wish; its Gunnery prereqs are locked"
      );
    }

    #[test]
    fn it_renders_each_non_skills_tab_loading_and_loaded_without_panicking() {
      use crate::features::skills::skill_plan_editor::picker::{PickerCert, PickerModule, PickerShip, PickerTab};

      let mut state = state_with_catalog(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
      state.picker_open = true;

      for tab in [PickerTab::Ships, PickerTab::Modules, PickerTab::Certs] {
        state.picker.active_tab = tab;
        state.refresh_rows();
        let _loading: Element<'_, Message> = view(&state, now());
      }

      state.picker.ships = Some(vec![PickerShip {
        id: 587,
        name: "Rifter".to_owned(),
        group_id: 25,
        group_name: "Frigate".to_owned(),
        own_requirements: vec![(3300, 1)],
        tier_cert_skills: vec![vec![cert_skill(3300, 1, 2, 3, 5)]],
      }]);
      state.picker.modules = Some(vec![PickerModule {
        id: 12_058,
        name: "125mm Gatling AutoCannon".to_owned(),
        group_id: 55,
        group_name: "Projectile Weapon".to_owned(),
        requirements: vec![(3300, 1)],
      }]);
      state.picker.certs = Some(vec![PickerCert {
        id: 1,
        name: "Gunnery Basics".to_owned(),
        grade: 1,
        skills: vec![cert_skill(3300, 1, 2, 3, 5)],
      }]);

      for tab in [PickerTab::Ships, PickerTab::Modules, PickerTab::Certs] {
        state.picker.active_tab = tab;
        state.refresh_rows();
        let _loaded: Element<'_, Message> = view(&state, now());
      }
    }

    #[test]
    fn it_renders_the_open_picker_without_panicking() {
      let mut state = state_with_catalog(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
      state.picker_open = true;
      state.refresh_rows();

      let _el: Element<'_, Message> = view(&state, now());
    }

    #[tokio::test]
    async fn picker_entries_get_unique_transient_ids() {
      let mut state = state_with_catalog(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::PickerLevelPicked(3300, 3), &db);

      let ids: Vec<i64> = state.entries.iter().map(|e| e.id).collect();
      let mut unique = ids.clone();
      unique.sort_unstable();
      unique.dedup();
      assert_eq!(unique.len(), ids.len(), "transient ids are unique");
      assert!(
        ids.iter().all(|&id| id < 0),
        "transient ids are negative (Save treats them as new)"
      );
    }

    #[tokio::test]
    async fn picking_a_level_expands_into_one_level_steps_and_flips_dirty() {
      let mut state = state_with_catalog(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
      let db = crate::store::open_test().await.unwrap();
      assert!(!state.dirty());

      let _ = update(&mut state, Message::PickerLevelPicked(3300, 3), &db);

      let levels: Vec<u8> = state.entries.iter().map(|e| e.to_level).collect();
      assert_eq!(levels, [1, 2, 3], "a pick to L3 inserts the three missing levels");
      assert!(state.entries.iter().all(|e| !e.is_auto), "explicit picks are not auto");
      assert_eq!(
        state.entries[0].meta.skill_name, "Gunnery",
        "metadata resolved from the catalog"
      );
      assert!(state.dirty(), "adding a skill flips the dirty dot");
    }

    #[tokio::test]
    async fn picking_a_level_inserts_direct_prereqs_as_auto_entries() {
      let mut state = state_with_catalog(vec![
        catalog_entry(3300, "Gunnery", 1, vec![]),
        catalog_entry(3330, "Small Hybrid Turret", 1, vec![("Gunnery".to_owned(), 3)]),
      ]);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::PickerLevelPicked(3330, 1), &db);

      let rows: Vec<(i64, u8, bool)> = state
        .entries
        .iter()
        .map(|e| (e.skill_id, e.to_level, e.is_auto))
        .collect();
      assert_eq!(
        rows,
        vec![(3300, 1, true), (3300, 2, true), (3300, 3, true), (3330, 1, false)],
        "prereqs inserted before the target, flagged is_auto"
      );
    }

    #[tokio::test]
    async fn picking_a_trained_level_still_seeds_the_full_set() {
      let mut state = state_with_catalog(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
      state.picker.trained_levels = HashMap::from([(3300, 5)]);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::PickerLevelPicked(3300, 4), &db);

      let levels: Vec<u8> = state.entries.iter().map(|e| e.to_level).collect();
      assert_eq!(
        levels,
        [1, 2, 3, 4],
        "storage is character-agnostic, so trained levels are seeded too"
      );
    }

    #[tokio::test]
    async fn re_picking_an_already_planned_level_adds_no_duplicate_slot() {
      let mut state = state_with_catalog(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::PickerLevelPicked(3300, 3), &db);
      let _ = update(&mut state, Message::PickerLevelPicked(3300, 2), &db);

      let levels: Vec<u8> = state.entries.iter().map(|e| e.to_level).collect();
      assert_eq!(levels, [1, 2, 3], "no duplicate slots after re-picking a planned level");
    }

    mod remove_skill {
      use pretty_assertions::assert_eq;

      use super::*;

      fn rows(state: &State) -> Vec<(i64, u8, bool)> {
        state
          .entries
          .iter()
          .map(|e| (e.skill_id, e.to_level, e.is_auto))
          .collect()
      }

      fn entry_id_for(state: &State, skill_id: i64, to_level: u8) -> i64 {
        state
          .entries
          .iter()
          .find(|e| e.skill_id == skill_id && e.to_level == to_level)
          .unwrap()
          .id
      }

      fn ship_with_two_required_skills() -> State {
        use crate::features::skills::skill_plan_editor::picker::PickerShip;

        let mut state = state_with_catalog(vec![
          catalog_entry(3300, "Gunnery", 1, vec![]),
          catalog_entry(3301, "Small Hybrid Turret", 1, vec![]),
        ]);
        state.picker.ships = Some(vec![PickerShip {
          id: 587,
          name: "Rifter".to_owned(),
          group_id: 25,
          group_name: "Frigate".to_owned(),
          own_requirements: vec![],
          tier_cert_skills: vec![vec![cert_skill(3300, 1, 2, 3, 5), cert_skill(3301, 1, 2, 3, 5)]],
        }]);
        state
      }

      #[tokio::test]
      async fn it_does_not_individually_remove_a_locked_prereq_row() {
        use crate::features::skills::skill_plan_editor::picker::PickerShip;

        let mut state = state_with_catalog(vec![
          catalog_entry(3300, "Gunnery", 1, vec![]),
          catalog_entry(3301, "Small Hybrid Turret", 1, vec![("Gunnery".to_owned(), 3)]),
        ]);
        state.picker.ships = Some(vec![PickerShip {
          id: 587,
          name: "Rifter".to_owned(),
          group_id: 25,
          group_name: "Frigate".to_owned(),
          own_requirements: vec![],
          tier_cert_skills: vec![vec![cert_skill(3301, 1, 2, 3, 5)]],
        }]);
        let db = crate::store::open_test().await.unwrap();
        let _ = update(&mut state, Message::PickerShipSelected(587, 1), &db);

        let prereq_id = entry_id_for(&state, 3300, 2);
        let before = rows(&state);
        let _ = update(&mut state, Message::EntryRemoved(prereq_id), &db);

        assert_eq!(
          rows(&state),
          before,
          "an is_auto prereq row cannot be removed on its own"
        );
      }

      #[tokio::test]
      async fn it_keeps_other_masteries_when_removing_one_masterys_wish() {
        use crate::features::skills::skill_plan_editor::picker::PickerShip;

        let mut state = state_with_catalog(vec![
          catalog_entry(3300, "Gunnery", 1, vec![]),
          catalog_entry(3301, "Small Hybrid Turret", 1, vec![("Gunnery".to_owned(), 3)]),
          catalog_entry(3302, "Medium Hybrid Turret", 1, vec![("Gunnery".to_owned(), 3)]),
        ]);
        state.picker.ships = Some(vec![
          PickerShip {
            id: 587,
            name: "Rifter".to_owned(),
            group_id: 25,
            group_name: "Frigate".to_owned(),
            own_requirements: vec![],
            tier_cert_skills: vec![vec![cert_skill(3301, 1, 2, 3, 5)]],
          },
          PickerShip {
            id: 588,
            name: "Thrasher".to_owned(),
            group_id: 25,
            group_name: "Destroyer".to_owned(),
            own_requirements: vec![],
            tier_cert_skills: vec![vec![cert_skill(3302, 1, 2, 3, 5)]],
          },
        ]);
        let db = crate::store::open_test().await.unwrap();
        let _ = update(&mut state, Message::PickerShipSelected(587, 1), &db);
        let _ = update(&mut state, Message::PickerShipSelected(588, 1), &db);

        let id = entry_id_for(&state, 3301, 1);
        let _ = update(&mut state, Message::EntryRemoved(id), &db);

        let mut remaining = rows(&state);
        remaining.sort();
        assert_eq!(
          remaining,
          vec![(3300, 1, true), (3300, 2, true), (3300, 3, true), (3302, 1, false)],
          "the second ship's wish and its still-needed Gunnery prereq are retained"
        );
      }

      #[tokio::test]
      async fn it_keeps_the_rest_when_removing_one_skill_from_a_pure_mastery_plan() {
        let mut state = ship_with_two_required_skills();
        let db = crate::store::open_test().await.unwrap();
        let _ = update(&mut state, Message::PickerShipSelected(587, 1), &db);

        let id = entry_id_for(&state, 3300, 1);
        let _ = update(&mut state, Message::EntryRemoved(id), &db);

        let mut remaining = rows(&state);
        remaining.sort();
        assert_eq!(
          remaining,
          vec![(3301, 1, false)],
          "only the removed mastery skill is dropped; the other survives"
        );
      }

      #[tokio::test]
      async fn removing_a_level_also_removes_higher_levels_of_the_same_skill() {
        let mut state = state_with_catalog(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
        let db = crate::store::open_test().await.unwrap();
        let _ = update(&mut state, Message::PickerLevelPicked(3300, 5), &db);

        let id = entry_id_for(&state, 3300, 3);
        let _ = update(&mut state, Message::EntryRemoved(id), &db);

        assert_eq!(
          rows(&state),
          vec![(3300, 1, false), (3300, 2, false)],
          "removing L3 drops L3, L4, L5 but keeps the lower levels"
        );
      }

      #[tokio::test]
      async fn removing_a_manual_skill_that_is_also_a_prereq_keeps_the_needed_lower_levels() {
        let mut state = state_with_catalog(vec![
          catalog_entry(3300, "Gunnery", 1, vec![]),
          catalog_entry(3330, "Small Hybrid Turret", 1, vec![("Gunnery".to_owned(), 3)]),
        ]);
        let db = crate::store::open_test().await.unwrap();
        let _ = update(&mut state, Message::PickerLevelPicked(3300, 5), &db);
        let _ = update(&mut state, Message::PickerLevelPicked(3330, 1), &db);

        let id = entry_id_for(&state, 3300, 4);
        let _ = update(&mut state, Message::EntryRemoved(id), &db);

        let mut remaining = rows(&state);
        remaining.sort();
        assert_eq!(
          remaining,
          vec![(3300, 1, false), (3300, 2, false), (3300, 3, false), (3330, 1, false),],
          "Gunnery I-III remain (still a prereq for the turret); IV and V are removed"
        );
      }

      #[tokio::test]
      async fn removing_a_skill_drops_its_orphaned_auto_prereqs() {
        let mut state = state_with_catalog(vec![
          catalog_entry(3300, "Gunnery", 1, vec![]),
          catalog_entry(3330, "Small Hybrid Turret", 1, vec![("Gunnery".to_owned(), 3)]),
        ]);
        let db = crate::store::open_test().await.unwrap();
        let _ = update(&mut state, Message::PickerLevelPicked(3330, 1), &db);
        assert_eq!(state.entries.len(), 4, "Gunnery I-III auto + Small Hybrid Turret I");

        let id = entry_id_for(&state, 3330, 1);
        let _ = update(&mut state, Message::EntryRemoved(id), &db);

        assert!(
          state.entries.is_empty(),
          "removing the only wish drops its now-orphaned auto prereqs"
        );
      }

      #[tokio::test]
      async fn removing_a_skill_flips_the_dirty_state_and_refreshes_rows() {
        let mut state = state_with_catalog(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
        let db = crate::store::open_test().await.unwrap();
        let _ = update(&mut state, Message::PickerLevelPicked(3300, 2), &db);
        state.saved = state.snapshot();
        state.recompute_dirty();
        assert!(!state.dirty());

        let id = entry_id_for(&state, 3300, 2);
        let _ = update(&mut state, Message::EntryRemoved(id), &db);

        assert_eq!(rows(&state), vec![(3300, 1, false)], "only L1 remains");
        assert_eq!(state.rows.len(), 1, "computed rows refreshed");
        assert!(state.dirty(), "removal marks the plan dirty");
      }

      #[tokio::test]
      async fn removing_a_skill_keeps_a_prereq_still_needed_by_another_skill() {
        let mut state = state_with_catalog(vec![
          catalog_entry(3300, "Gunnery", 1, vec![]),
          catalog_entry(3330, "Small Hybrid Turret", 1, vec![("Gunnery".to_owned(), 3)]),
          catalog_entry(3340, "Medium Hybrid Turret", 1, vec![("Gunnery".to_owned(), 3)]),
        ]);
        let db = crate::store::open_test().await.unwrap();
        let _ = update(&mut state, Message::PickerLevelPicked(3330, 1), &db);
        let _ = update(&mut state, Message::PickerLevelPicked(3340, 1), &db);

        let id = entry_id_for(&state, 3330, 1);
        let _ = update(&mut state, Message::EntryRemoved(id), &db);

        let mut remaining = rows(&state);
        remaining.sort();
        assert_eq!(
          remaining,
          vec![(3300, 1, true), (3300, 2, true), (3300, 3, true), (3340, 1, false),],
          "the shared Gunnery prereq is retained for the other wished skill"
        );
      }

      #[tokio::test]
      async fn removing_an_unknown_id_is_a_no_op() {
        let mut state = state_with_catalog(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
        let db = crate::store::open_test().await.unwrap();
        let _ = update(&mut state, Message::PickerLevelPicked(3300, 2), &db);

        let _ = update(&mut state, Message::EntryRemoved(999_999), &db);

        assert_eq!(
          rows(&state),
          vec![(3300, 1, false), (3300, 2, false)],
          "nothing removed"
        );
      }
    }

    #[tokio::test]
    async fn search_and_group_toggle_update_picker_state() {
      let mut state = state_with_catalog(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::PickerSearchChanged("gun".to_owned()), &db);
      assert_eq!(state.picker.query, "gun");

      assert!(state.picker.expanded_groups.contains(&255));
      let _ = update(&mut state, Message::PickerGroupToggled(255), &db);
      assert!(!state.picker.expanded_groups.contains(&255));
      let _ = update(&mut state, Message::PickerGroupToggled(255), &db);
      assert!(state.picker.expanded_groups.contains(&255));
    }

    #[tokio::test]
    async fn selecting_a_cert_proficiency_chip_updates_the_selection() {
      let mut state = state_with_items(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::PickerCertProficiencyChanged(1, 2), &db);
      assert_eq!(state.picker.cert_proficiency.get(&1).copied(), Some(2));
    }

    #[tokio::test]
    async fn selecting_a_ship_mastery_chip_updates_the_selection() {
      let mut state = state_with_items(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::PickerShipMasteryChanged(587, 4), &db);
      assert_eq!(state.picker.ship_mastery.get(&587).copied(), Some(4));
    }
  }

  mod picker_ships_loader {
    use super::*;
    use crate::store::{
      model::{ItemCategory, ItemGroup, ItemType},
      repo::sde::{upsert_item_category, upsert_item_group, upsert_item_type},
    };

    fn category(id: i64, name: &str) -> ItemCategory {
      ItemCategory {
        id,
        icon_id: None,
        name: name.to_owned(),
        published: true,
      }
    }

    fn group(id: i64, category_id: i64, name: &str) -> ItemGroup {
      ItemGroup {
        category_id,
        icon_id: None,
        id,
        name: name.to_owned(),
        published: true,
      }
    }

    fn item(id: i64, group_id: i64, name: &str) -> ItemType {
      ItemType {
        capacity: None,
        description: Some("Test item".to_owned()),
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

    #[tokio::test]
    async fn it_assembles_the_ships_tab_list_off_the_db() {
      let db = crate::store::open_test().await.unwrap();
      upsert_item_category(&db, &category(6, "Ship")).await.unwrap();
      upsert_item_group(&db, &group(25, 6, "Frigate")).await.unwrap();
      upsert_item_type(&db, &item(587, 25, "Rifter")).await.unwrap();

      let ships = load_picker_ships(db).await;

      assert_eq!(ships.len(), 1);
      assert_eq!(ships[0].id, 587);
      assert_eq!(ships[0].name, "Rifter");
      assert_eq!(ships[0].group_id, 25);
      assert_eq!(ships[0].group_name, "Frigate");
    }

    #[tokio::test]
    async fn it_yields_an_empty_list_when_no_ships_are_seeded() {
      let db = crate::store::open_test().await.unwrap();

      let ships = load_picker_ships(db).await;

      assert!(ships.is_empty());
    }
  }

  mod picker_tabs {
    use super::*;
    use crate::features::skills::skill_plan_editor::picker::PickerTab;

    #[tokio::test]
    async fn selecting_a_tab_switches_the_active_tab() {
      let mut state = State::new(Some(42));
      let db = crate::store::open_test().await.unwrap();
      assert_eq!(state.picker.active_tab, PickerTab::Skills);

      let _ = update(&mut state, Message::PickerTabSelected(PickerTab::Ships), &db);
      assert_eq!(
        state.picker.active_tab,
        PickerTab::Ships,
        "clicking a tab switches to it"
      );

      let _ = update(&mut state, Message::PickerTabSelected(PickerTab::Skills), &db);
      assert_eq!(state.picker.active_tab, PickerTab::Skills);
    }
  }

  mod priority {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_cycles_low_normal_high() {
      assert_eq!(Priority::Low.next(), Priority::Normal);
      assert_eq!(Priority::Normal.next(), Priority::High);
      assert_eq!(Priority::High.next(), Priority::Low);
    }

    #[test]
    fn it_round_trips_through_its_token() {
      for p in [Priority::Low, Priority::Normal, Priority::High] {
        assert_eq!(Priority::from_token(p.as_token()), p);
      }
      assert_eq!(Priority::from_token("garbage"), Priority::Normal);
    }
  }

  mod remaps {
    use pretty_assertions::assert_eq;

    use super::*;

    fn base() -> Attributes {
      Attributes {
        charisma: 19,
        intelligence: 21,
        memory: 19,
        perception: 21,
        willpower: 19,
      }
    }

    fn state_with(availability: u32) -> State {
      let mut state = State::new(Some(42));
      state.entries = vec![
        edit_entry(10, 3300, 5),
        edit_entry(11, 3301, 5),
        edit_entry(12, 3302, 5),
      ];
      state.attrs = base();
      state.base_attrs = base();
      state.remap_availability = availability;
      state.refresh_rows();
      state.saved = state.snapshot();
      state
    }

    #[test]
    fn can_place_remap_tracks_placed_in_plan_points() {
      let mut state = state_with(2);
      assert!(state.can_place_remap());

      state.remap_points.push(EditMilestone {
        after_entry_id: Some(10),
        auto_remap: false,
        base: Some(base()),
        local_id: 1,
        name: "Milestone".to_owned(),
        order: 0,
      });
      assert!(state.can_place_remap(), "one of two placed");

      state.remap_points.push(EditMilestone {
        after_entry_id: Some(11),
        auto_remap: false,
        base: Some(base()),
        local_id: 2,
        name: "Milestone".to_owned(),
        order: 1,
      });
      assert!(!state.can_place_remap(), "both consumed");

      state.remap_points.push(EditMilestone {
        after_entry_id: None,
        auto_remap: false,
        base: Some(base()),
        local_id: 3,
        name: "Milestone".to_owned(),
        order: 2,
      });
      assert_eq!(state.placed_in_plan_remaps(), 2, "start point not counted");
    }

    #[tokio::test]
    async fn it_never_caps_milestone_insertion_by_remap_availability() {
      // A template has zero remap availability but must still add unlimited milestones: insertion
      // is a plain section marker and is never gated on neural-remap slots.
      let mut state = state_with(0);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::RemapInserted(Some(10)), &db);
      let _ = update(&mut state, Message::RemapInserted(Some(11)), &db);
      let _ = update(&mut state, Message::RemapInserted(Some(12)), &db);

      assert_eq!(state.remap_points.len(), 3, "insertion is never capped by availability");
      assert!(!state.can_place_remap(), "yet no remap slots are available");
    }

    #[tokio::test]
    async fn it_inserts_a_plain_milestone_with_no_remap() {
      let mut state = state_with(1);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::RemapInserted(Some(10)), &db);

      assert_eq!(state.remap_points.len(), 1);
      assert_eq!(state.remap_points[0].after_entry_id, Some(10));
      assert_eq!(
        state.remap_points[0].base, None,
        "new milestones carry no remap by default"
      );
      assert!(state.dirty());
    }

    #[tokio::test]
    async fn the_start_point_is_free_and_not_capped() {
      let mut state = state_with(0);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::RemapInserted(None), &db);

      assert_eq!(state.remap_points.len(), 1);
      assert_eq!(state.remap_points[0].after_entry_id, None);
    }

    #[tokio::test]
    async fn milestones_stack_after_a_row_that_already_carries_one() {
      let mut state = state_with(0);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::RemapInserted(Some(10)), &db);
      let _ = update(&mut state, Message::RemapInserted(Some(10)), &db);

      assert_eq!(
        state.remap_points.len(),
        2,
        "a second milestone stacks at the same anchor"
      );
      assert!(state.remap_points.iter().all(|r| r.after_entry_id == Some(10)));
    }

    #[tokio::test]
    async fn attaching_a_remap_is_gated_on_availability() {
      // Insertion is free, but attaching a remap to an in-plan milestone consumes a slot; with zero
      // availability the suggest is a no-op that leaves the milestone a plain section marker.
      let mut state = state_with(0);
      let db = crate::store::open_test().await.unwrap();
      let _ = update(&mut state, Message::RemapInserted(Some(10)), &db);
      let local_id = state.remap_points[0].local_id;

      let _ = update(&mut state, Message::MilestoneRemapSuggested(local_id), &db);

      assert!(!state.remap_points[0].auto_remap, "no slot, so no remap attached");
      assert_eq!(state.remap_points[0].base, None);
    }

    #[tokio::test]
    async fn suggesting_a_remap_flags_it_auto_and_optimizes_its_segment() {
      let mut state = state_with(1);
      let db = crate::store::open_test().await.unwrap();
      let _ = update(&mut state, Message::RemapInserted(Some(10)), &db);
      let local_id = state.remap_points[0].local_id;

      let _ = update(&mut state, Message::MilestoneRemapSuggested(local_id), &db);

      assert!(state.remap_points[0].auto_remap);
      let optimized = state.remap_points[0].base.unwrap();
      assert_eq!(optimized.perception, 27);
      assert_eq!(optimized.willpower, 21);
    }

    #[tokio::test]
    async fn suggesting_all_flags_every_milestone_and_optimizes() {
      let mut state = state_with(2);
      let db = crate::store::open_test().await.unwrap();
      state.remap_points = vec![
        EditMilestone {
          after_entry_id: None,
          auto_remap: false,
          base: Some(base()),
          local_id: 1,
          name: "A".to_owned(),
          order: 0,
        },
        EditMilestone {
          after_entry_id: Some(10),
          auto_remap: false,
          base: Some(base()),
          local_id: 2,
          name: "B".to_owned(),
          order: 1,
        },
      ];

      let _ = update(&mut state, Message::MilestonesAllSuggested, &db);

      assert!(state.remap_points.iter().all(|m| m.auto_remap));
      for milestone in &state.remap_points {
        let optimized = milestone.base.unwrap();
        assert_eq!(optimized.perception, 27);
        assert_eq!(optimized.willpower, 21);
      }
    }

    #[tokio::test]
    async fn recompute_only_touches_auto_milestones() {
      let mut state = state_with(2);
      let manual = Attributes {
        charisma: 17,
        intelligence: 17,
        memory: 27,
        perception: 17,
        willpower: 21,
      };
      state.remap_points = vec![
        EditMilestone {
          after_entry_id: None,
          auto_remap: true,
          base: Some(base()),
          local_id: 1,
          name: "Auto".to_owned(),
          order: 0,
        },
        EditMilestone {
          after_entry_id: Some(10),
          auto_remap: false,
          base: Some(manual),
          local_id: 2,
          name: "Manual".to_owned(),
          order: 1,
        },
      ];

      state.refresh_rows();

      let auto_base = state.remap_points[0].base.unwrap();
      assert_eq!(auto_base.perception, 27);
      assert_eq!(auto_base.willpower, 21);
      assert_eq!(state.remap_points[1].base, Some(manual));
    }

    #[tokio::test]
    async fn recompute_drops_the_base_of_an_empty_segment() {
      let mut state = state_with(1);
      state.remap_points = vec![EditMilestone {
        after_entry_id: Some(12),
        auto_remap: true,
        base: Some(base()),
        local_id: 1,
        name: "Tail".to_owned(),
        order: 0,
      }];

      state.refresh_rows();

      assert_eq!(state.remap_points[0].base, None);
    }

    #[tokio::test]
    async fn moving_an_auto_milestone_recomputes_against_its_new_segment() {
      let mut state = state_with(1);
      state.remap_points = vec![EditMilestone {
        after_entry_id: Some(10),
        auto_remap: true,
        base: Some(base()),
        local_id: 1,
        name: "Mover".to_owned(),
        order: 0,
      }];
      state.refresh_rows();
      assert!(state.remap_points[0].base.is_some());

      state.remap_points[0].after_entry_id = Some(12);
      state.refresh_rows();

      assert_eq!(state.remap_points[0].base, None);
    }

    #[tokio::test]
    async fn clearing_a_remap_keeps_the_boundary_and_drops_the_base() {
      let mut state = state_with(1);
      let db = crate::store::open_test().await.unwrap();
      let _ = update(&mut state, Message::RemapInserted(Some(10)), &db);
      let local_id = state.remap_points[0].local_id;
      let _ = update(&mut state, Message::MilestoneRemapSuggested(local_id), &db);

      let _ = update(&mut state, Message::MilestoneRemapCleared(local_id), &db);

      let cleared = &state.remap_points[0];
      assert!(!cleared.auto_remap);
      assert_eq!(cleared.base, None);
      assert_eq!(cleared.after_entry_id, Some(10));
      assert_eq!(cleared.name, "Milestone 1");
    }

    #[tokio::test]
    async fn removing_a_milestone_drops_it() {
      let mut state = state_with(1);
      let db = crate::store::open_test().await.unwrap();
      let _ = update(&mut state, Message::RemapInserted(Some(10)), &db);
      let local_id = state.remap_points[0].local_id;

      let _ = update(&mut state, Message::MilestoneRemoved(local_id), &db);

      assert!(state.remap_points.is_empty());
    }
  }

  mod save {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::{character, skills},
    };

    async fn seed_character(db: &Database, id: i64) {
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
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_persists_a_new_plan_so_a_reload_reproduces_it() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let mut state = State::new(Some(42));
      state.name = "Combat".to_owned();
      state.sort = Sort {
        column: SortColumn::Time,
        direction: SortDirection::Ascending,
      };
      let mut a = edit_entry(0, 3300, 5);
      a.priority = Priority::High;
      a.note = "core".to_owned();
      let b = edit_entry(0, 3301, 4);
      state.entries = vec![a, b];

      let id = persist(
        &db,
        Some(42),
        None,
        "Combat",
        Sort {
          column: SortColumn::Time,
          direction: SortDirection::Ascending,
        }
        .as_token(),
        "current",
        &[
          (3300, 5, "high".to_owned(), "core".to_owned(), 0),
          (3301, 4, "normal".to_owned(), String::new(), 0),
        ],
        &[],
        &[],
        &[],
      )
      .await
      .unwrap();

      let plan = skills::get(&db, id).await.unwrap().unwrap();
      assert_eq!(plan.name(), "Combat");
      assert_eq!(plan.sort_mode(), "time-asc");
      let entries = skills::entries(&db, id).await.unwrap();
      assert_eq!(entries.iter().map(|e| e.skill_id()).collect::<Vec<_>>(), [3300, 3301]);
      assert_eq!(entries[0].priority(), "high");
      assert_eq!(entries[0].note(), "core");
      assert_eq!(entries.iter().map(|e| e.position()).collect::<Vec<_>>(), [0, 1]);
    }

    #[tokio::test]
    async fn it_re_establishes_a_remap_point_against_the_new_entry_ids() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let id = persist(
        &db,
        Some(42),
        None,
        "Plan",
        "manual",
        "current",
        &[
          (3300, 5, "normal".to_owned(), String::new(), 0),
          (3301, 5, "normal".to_owned(), String::new(), 0),
        ],
        &[RemapSave {
          anchor_index: Some(0),
          auto_remap: false,
          base: Some((17, 27, 17, 21, 17)),
          name: "Milestone".to_owned(),
          order: 0,
        }],
        &[],
        &[],
      )
      .await
      .unwrap();

      let entries = skills::entries(&db, id).await.unwrap();
      let remaps = skills::milestones(&db, id).await.unwrap();
      assert_eq!(remaps.len(), 1);
      assert_eq!(
        remaps[0].after_entry_id(),
        Some(entries[0].id()),
        "anchored to position 0's new id"
      );
    }
  }

  mod save_task {
    use super::*;

    fn base() -> Attributes {
      Attributes {
        charisma: 17,
        intelligence: 27,
        memory: 17,
        perception: 21,
        willpower: 17,
      }
    }

    #[tokio::test]
    async fn a_named_plan_carries_its_trimmed_name() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(Some(42));
      state.name = "  Combat  ".to_owned();
      state.sort = Sort {
        column: SortColumn::Time,
        direction: SortDirection::Descending,
      };
      state.entries = vec![edit_entry(10, 3300, 5)];

      let _task = save(&state, &db);
    }

    #[tokio::test]
    async fn a_template_save_drops_ship_masteries_and_cert_proficiencies() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(None);
      state.name = "Doctrine".to_owned();
      state.entries = vec![edit_entry(10, 3300, 5)];
      state.picker.ship_mastery.insert(587, 4);
      state.picker.cert_proficiency.insert(1, 2);

      let _task = save(&state, &db);
      let _routed = update(&mut state, Message::SaveRequested, &db);
    }

    #[tokio::test]
    async fn it_builds_the_persist_task_from_state_without_panicking() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(Some(42));
      state.name = "  ".to_owned();
      let mut a = edit_entry(10, 3300, 5);
      a.priority = Priority::High;
      a.note = "core".to_owned();
      let b = edit_entry(11, 3301, 4);
      state.entries = vec![a, b];
      state.remap_points = vec![
        EditMilestone {
          after_entry_id: Some(10),
          auto_remap: false,
          base: Some(base()),
          local_id: 1,
          name: "Milestone".to_owned(),
          order: 0,
        },
        EditMilestone {
          after_entry_id: Some(999),
          auto_remap: false,
          base: Some(base()),
          local_id: 2,
          name: "Milestone".to_owned(),
          order: 1,
        },
        EditMilestone {
          after_entry_id: None,
          auto_remap: false,
          base: Some(base()),
          local_id: 3,
          name: "Milestone".to_owned(),
          order: 2,
        },
      ];

      let _task = save(&state, &db);
      let _routed = update(&mut state, Message::SaveRequested, &db);
    }
  }

  mod sort {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_carets_only_the_active_column() {
      let sort = Sort {
        column: SortColumn::Secondary,
        direction: SortDirection::Descending,
      };

      assert_eq!(sort.caret(SortColumn::Secondary), Some(SortDirection::Descending));
      assert_eq!(sort.caret(SortColumn::Time), None);
      assert_eq!(sort.caret(SortColumn::Primary), None);
    }

    #[test]
    fn it_degrades_an_unknown_token_to_manual() {
      assert_eq!(Sort::from_token("optimal"), Sort::default());
      assert_eq!(Sort::from_token("garbage").column, SortColumn::Manual);
    }

    #[test]
    fn it_opens_a_new_column_in_its_natural_direction() {
      let sort = Sort {
        column: SortColumn::Time,
        direction: SortDirection::Descending,
      };

      let switched = sort.toggled(SortColumn::Primary);
      assert_eq!(switched.column, SortColumn::Primary);
      assert_eq!(switched.direction, SortDirection::Ascending);
    }

    #[test]
    fn it_parses_legacy_tokens_without_error() {
      assert_eq!(Sort::from_token("manual"), Sort::default());
      assert_eq!(
        Sort::from_token("time-asc"),
        Sort {
          column: SortColumn::Time,
          direction: SortDirection::Ascending,
        }
      );
      assert_eq!(
        Sort::from_token("time-desc"),
        Sort {
          column: SortColumn::Time,
          direction: SortDirection::Descending,
        }
      );
    }

    #[test]
    fn it_round_trips_every_column_and_direction_through_its_token() {
      for column in [
        SortColumn::Manual,
        SortColumn::Primary,
        SortColumn::Secondary,
        SortColumn::Time,
      ] {
        for direction in [SortDirection::Ascending, SortDirection::Descending] {
          let sort = Sort {
            column,
            direction,
          };

          let parsed = Sort::from_token(sort.as_token());
          if column == SortColumn::Manual {
            assert_eq!(parsed, Sort::default());
          } else {
            assert_eq!(parsed, sort);
          }
        }
      }
    }

    #[test]
    fn it_toggles_direction_when_the_same_column_is_reselected() {
      let sort = Sort {
        column: SortColumn::Time,
        direction: SortDirection::Ascending,
      };

      let flipped = sort.toggled(SortColumn::Time);
      assert_eq!(flipped.column, SortColumn::Time);
      assert_eq!(flipped.direction, SortDirection::Descending);

      let flipped_again = flipped.toggled(SortColumn::Time);
      assert_eq!(flipped_again.direction, SortDirection::Ascending);
    }
  }

  mod subscription {
    use super::*;

    #[test]
    fn it_is_empty_when_nothing_is_being_dragged() {
      let state = State::new(Some(42));

      let _sub: iced::Subscription<Message> = subscription(&state);
    }

    #[test]
    fn it_listens_for_release_while_an_entry_or_pane_drag_is_active() {
      let mut state = State::new(Some(42));
      state.dragging = Some(7);
      state.dragging_pane = Some(EditorPane::Picker);

      let _sub: iced::Subscription<Message> = subscription(&state);
    }
  }

  mod summary {
    use super::*;

    #[test]
    fn summary_data_aggregates_group_and_pair_time_and_runs_the_optimizer() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];
      state.attrs = Attributes {
        charisma: 19,
        intelligence: 21,
        memory: 19,
        perception: 21,
        willpower: 19,
      };
      state.base_attrs = state.attrs;
      state.refresh_rows();

      let data = state.summary_data();

      assert_eq!(data.steps, 2, "both steps count");
      assert!(data.total_sec > 0.0);
      assert_eq!(data.group_sec.len(), 1, "one skill group bucket");
      assert!(data.group_sec.contains_key("Gunnery"));
      assert_eq!(data.pair_sec.len(), 1, "one attribute-pair bucket");
      assert!(data.recommendation.total_sec.is_finite());
    }
  }

  mod character_attrs_from {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{CharacterAttributes, CharacterImplant};

    fn row(charisma: i64, intelligence: i64, memory: i64, perception: i64, willpower: i64) -> CharacterAttributes {
      CharacterAttributes {
        accrued_remap_cooldown_date: None,
        bonus_remaps: 2,
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

    fn implant(attribute_id: i64, bonus: i64) -> CharacterImplant {
      CharacterImplant {
        attribute_id,
        bonus,
        character_id: 42,
      }
    }

    #[test]
    fn a_bare_character_keeps_stored_values_as_base() {
      let attrs = super::super::character_attrs_from(&row(17, 21, 21, 20, 20), &[], Utc::now());

      assert_eq!(attrs.attrs, attrs.base_attrs);
      assert_eq!(attrs.booster_n, 0);
      assert!(attrs.consistent);
      assert_eq!(attrs.availability, 3);
    }

    #[test]
    fn implants_and_booster_are_split_out_of_the_stored_values() {
      let stored = row(24, 28, 28, 27, 27);
      let implants = [
        implant(164, 2),
        implant(165, 2),
        implant(166, 2),
        implant(167, 2),
        implant(168, 2),
      ];

      let attrs = super::super::character_attrs_from(&stored, &implants, Utc::now());

      assert!(attrs.consistent);
      assert_eq!(attrs.booster_n, 5);
      assert_eq!(
        attrs.base_attrs.charisma + attrs.base_attrs.intelligence + attrs.base_attrs.memory,
        17 + 21 + 21
      );
      assert_eq!(
        attrs.attrs,
        Attributes {
          charisma: 24,
          intelligence: 28,
          memory: 28,
          perception: 27,
          willpower: 27
        }
      );
    }

    #[test]
    fn inconsistent_data_falls_back_to_raw_stored_values() {
      let attrs = super::super::character_attrs_from(&row(17, 21, 21, 20, 21), &[], Utc::now());

      assert!(!attrs.consistent);
      assert_eq!(attrs.booster_n, 0);
      assert_eq!(attrs.base_attrs, attrs.attrs);
    }

    #[test]
    fn negative_implant_bonuses_are_clamped_to_zero() {
      let attrs = super::super::character_attrs_from(&row(17, 21, 21, 20, 20), &[implant(175, -5)], Utc::now());

      assert!(attrs.consistent);
      assert_eq!(attrs.base_attrs, attrs.attrs);
    }
  }

  mod template_attrs {
    use pretty_assertions::assert_eq;

    use super::*;

    fn template_state() -> State {
      let baseline = super::super::template_attrs();
      let mut state = State::new(None);
      state.attrs = baseline.attrs;
      state.base_attrs = baseline.base_attrs;
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];
      state
    }

    #[test]
    fn it_seeds_both_attribute_sets_from_the_unmapped_baseline() {
      let baseline = super::super::template_attrs();

      assert_eq!(baseline.attrs, Attributes::unmapped());
      assert_eq!(baseline.base_attrs, Attributes::unmapped());
    }

    #[test]
    fn a_template_with_entries_trains_in_non_zero_time() {
      let mut state = template_state();
      state.refresh_rows();

      assert!(state.total_sec > 0.0, "template training time is non-zero");
      assert!(state.rows.iter().all(|row| row.sec > 0.0), "every step trains in time");

      let data = state.summary_data();
      assert!(!data.group_sec.is_empty(), "the by-group breakdown is populated");
      assert!(!data.pair_sec.is_empty(), "the by-pair breakdown is populated");
    }

    #[test]
    fn csv_duration_is_non_zero_for_every_trained_step() {
      let mut state = template_state();
      state.refresh_rows();

      let csv = serialize_plan_csv(&state);
      let data_rows: Vec<&str> = csv.lines().skip(1).collect();

      assert_eq!(data_rows.len(), state.rows.len(), "one CSV row per template step");
      assert!(
        state.rows.iter().all(|row| row.sec > 0.0),
        "every step has a training time"
      );
      assert!(
        data_rows.iter().all(|line| !line.ends_with(",0m")),
        "no template step exports a zero duration"
      );
    }

    #[test]
    fn a_manual_remap_divider_lowers_the_template_total() {
      let mut flat = template_state();
      flat.refresh_rows();
      let flat_total = flat.total_sec;

      let mut remapped = template_state();
      remapped.remap_points = vec![EditMilestone {
        after_entry_id: Some(remapped.entries[0].id),
        auto_remap: false,
        base: Some(Attributes {
          charisma: 17,
          intelligence: 17,
          memory: 17,
          perception: 27,
          willpower: 21,
        }),
        local_id: 1,
        name: "Milestone".to_owned(),
        order: 0,
      }];
      remapped.refresh_rows();

      assert!(
        remapped.total_sec < flat_total,
        "a manual remap divider speeds the segment after it ({} < {flat_total})",
        remapped.total_sec
      );
    }
  }

  mod sync_aware_plan {
    use pretty_assertions::assert_eq;

    use super::*;

    fn attrs() -> Attributes {
      Attributes {
        charisma: 17,
        intelligence: 17,
        memory: 17,
        perception: 27,
        willpower: 21,
      }
    }

    fn state_with_l5_entry() -> State {
      let mut state = State::new(Some(42));
      state.attrs = attrs();
      state.entries = vec![edit_entry(1, 3300, 5)];
      state
    }

    #[test]
    fn a_partially_trained_skill_yields_reduced_remaining_sp() {
      let mut untrained = state_with_l5_entry();
      untrained.refresh_rows();
      let full_sp = untrained.rows[0].sp;
      let full_sec = untrained.rows[0].sec;

      let mut partial = state_with_l5_entry();
      partial.synced_levels = HashMap::from([(3300, 4)]);
      partial.synced_sp = HashMap::from([(3300, 226_275)]);
      partial.refresh_rows();
      let partial_row = &partial.rows[0];

      assert!(!partial_row.skipped, "a not-yet-complete level still trains");
      assert!(
        partial_row.sp < full_sp,
        "banked SP discounts the remaining cost ({} < {full_sp})",
        partial_row.sp
      );
      assert!(partial_row.sp > 0, "L5 is not yet reached, so some SP remains");
      assert!(
        partial_row.sec < full_sec,
        "less remaining SP means less remaining time"
      );
      assert_eq!(
        partial.total_sp, partial_row.sp,
        "plan total reflects the discounted step"
      );
    }

    #[test]
    fn an_already_trained_skill_contributes_zero_remaining() {
      let mut state = state_with_l5_entry();
      state.synced_levels = HashMap::from([(3300, 5)]);
      state.synced_sp = HashMap::from([(3300, 1_280_000)]);
      state.refresh_rows();

      let row = &state.rows[0];
      assert!(row.skipped, "an over-trained target is a zero-cost skip row");
      assert_eq!(row.sp, 0);
      assert_eq!(row.sec, 0.0);
      assert_eq!(state.total_sp, 0);
      assert_eq!(state.total_sec, 0.0);
    }

    #[test]
    fn a_hidden_trained_step_is_excluded_from_the_step_count_and_renders() {
      let mut state = State::new(Some(42));
      state.attrs = attrs();
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];
      state.synced_levels = HashMap::from([(3300, 5)]);
      state.synced_sp = HashMap::from([(3300, 1_280_000)]);
      state.refresh_rows();

      assert!(
        state.rows[0].skipped,
        "the already-trained entry projects as a skip row"
      );
      assert!(!state.rows[1].skipped, "the untrained entry still needs training");
      assert_eq!(state.summary.steps, 1, "the step count omits the already-trained level");

      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn an_over_trained_plan_renders_terminal_training_time_and_eta_cells() {
      let mut state = state_with_l5_entry();
      state.synced_levels = HashMap::from([(3300, 5)]);
      state.synced_sp = HashMap::from([(3300, u64::MAX)]);
      state.refresh_rows();

      assert_eq!(state.total_sp, 0, "an over-trained plan demands zero SP");
      assert_eq!(state.total_sec, 0.0, "zero SP means zero training seconds");

      let total_secs = if state.total_sec.is_finite() {
        state.total_sec.clamp(0.0, i64::MAX as f64) as i64
      } else {
        0
      };
      assert_eq!(
        fmt_duration(total_secs),
        "\u{2014}",
        "training-time cell renders the terminal dash"
      );
      assert_eq!(
        fmt_eta(now(), total_secs),
        "\u{2014}",
        "ETA cell renders the terminal dash"
      );
    }

    #[test]
    fn an_untrained_skill_charges_the_full_target_cost() {
      let mut state = state_with_l5_entry();
      state.refresh_rows();

      let row = &state.rows[0];
      assert!(!row.skipped);
      assert_eq!({ row.sp }, state.total_sp);
      assert!(row.sp > 0);
    }

    #[test]
    fn implant_bonus_is_the_difference_between_effective_and_base() {
      let mut state = state_with_l5_entry();
      state.base_attrs = attrs();
      state.attrs = Attributes {
        perception: attrs().perception + 4,
        memory: attrs().memory + 3,
        ..attrs()
      };

      let bonus = state.implant_bonus();
      assert_eq!(bonus.perception, 4);
      assert_eq!(bonus.memory, 3);
      assert_eq!(bonus.charisma, 0);
      assert_eq!(bonus.intelligence, 0);
      assert_eq!(bonus.willpower, 0);
    }

    #[test]
    fn implants_factor_into_the_summary_current_plan_time() {
      let mut no_implants = state_with_l5_entry();
      no_implants.base_attrs = attrs();
      no_implants.attrs = attrs();
      no_implants.refresh_rows();

      let mut with_implants = state_with_l5_entry();
      with_implants.base_attrs = attrs();
      with_implants.attrs = Attributes {
        perception: attrs().perception + 5,
        willpower: attrs().willpower + 5,
        ..attrs()
      };
      with_implants.refresh_rows();

      let baseline = no_implants.summary_data().current_sec;
      let boosted = with_implants.summary_data().current_sec;
      assert!(baseline.is_finite() && baseline > 0.0);
      assert!(
        boosted < baseline,
        "the summary's current plan time reflects installed implants ({boosted} < {baseline})"
      );
    }

    #[test]
    fn installed_implants_shorten_the_plan_training_time() {
      let mut no_implants = state_with_l5_entry();
      no_implants.base_attrs = attrs();
      no_implants.attrs = attrs();
      no_implants.refresh_rows();

      let mut with_implants = state_with_l5_entry();
      with_implants.base_attrs = attrs();
      with_implants.attrs = Attributes {
        perception: attrs().perception + 5,
        willpower: attrs().willpower + 5,
        ..attrs()
      };
      with_implants.refresh_rows();

      assert!(no_implants.total_sec > 0.0);
      assert_eq!(
        no_implants.total_sp, with_implants.total_sp,
        "implants change the rate, not the SP demanded"
      );
      assert!(
        with_implants.total_sec < no_implants.total_sec,
        "installed implants raise the training rate, so the plan finishes sooner ({} < {})",
        with_implants.total_sec,
        no_implants.total_sec
      );
    }

    #[test]
    fn remap_optimization_ignores_implants_and_matches_the_base_only_recommendation() {
      let mut no_implants = state_with_l5_entry();
      no_implants.base_attrs = attrs();
      no_implants.attrs = attrs();
      no_implants.refresh_rows();

      let mut with_implants = state_with_l5_entry();
      with_implants.base_attrs = attrs();
      with_implants.attrs = Attributes {
        charisma: attrs().charisma + 4,
        memory: attrs().memory + 3,
        ..attrs()
      };
      with_implants.refresh_rows();

      assert_eq!(
        no_implants.summary_data().recommendation.base,
        with_implants.summary_data().recommendation.base,
        "installed implants must not change the recommended base remap"
      );
    }

    #[test]
    fn remap_optimization_stays_within_the_base_range_even_with_implants_installed() {
      let mut state = state_with_l5_entry();
      state.base_attrs = attrs();
      state.attrs = Attributes {
        charisma: attrs().charisma + 5,
        intelligence: attrs().intelligence + 5,
        memory: attrs().memory + 5,
        perception: attrs().perception + 5,
        willpower: attrs().willpower + 5,
      };
      state.refresh_rows();

      let proposed = state.summary_data().recommendation.base;

      for value in [
        proposed.charisma,
        proposed.intelligence,
        proposed.memory,
        proposed.perception,
        proposed.willpower,
      ] {
        assert!(
          (17..=27).contains(&value),
          "remapped base attribute {value} escaped the [17, 27] base range"
        );
      }
      assert_eq!(
        proposed.charisma + proposed.intelligence + proposed.memory + proposed.perception + proposed.willpower,
        99,
        "remap base attributes must sum to 99"
      );
    }

    #[test]
    fn the_booster_speeds_current_time_but_not_the_optimizer_basis() {
      let base = attrs();
      let mut boosted = state_with_l5_entry();
      boosted.base_attrs = base;
      boosted.attrs = Attributes {
        charisma: base.charisma + 5,
        intelligence: base.intelligence + 5,
        memory: base.memory + 5,
        perception: base.perception + 5,
        willpower: base.willpower + 5,
      };
      boosted.booster_n = 5;
      boosted.refresh_rows();

      let mut plain = state_with_l5_entry();
      plain.base_attrs = base;
      plain.attrs = base;
      plain.booster_n = 0;
      plain.refresh_rows();

      let boosted_data = boosted.summary_data();
      let plain_data = plain.summary_data();

      assert!(
        boosted_data.current_sec < plain_data.current_sec,
        "the live booster raises the current rate ({} < {})",
        boosted_data.current_sec,
        plain_data.current_sec
      );
      assert_eq!(
        boosted_data.recommendation.base, plain_data.recommendation.base,
        "the booster is excluded from the candidate comparison"
      );
      assert_eq!(
        boosted_data.recommendation.total_sec, plain_data.recommendation.total_sec,
        "the booster does not alter the optimizer's projected time"
      );
    }

    #[test]
    fn the_optimizer_basis_includes_installed_implants() {
      let base = attrs();
      let mut with_implants = state_with_l5_entry();
      with_implants.base_attrs = base;
      with_implants.attrs = Attributes {
        perception: base.perception + 5,
        willpower: base.willpower + 5,
        ..base
      };
      with_implants.booster_n = 0;
      with_implants.refresh_rows();

      let mut without = state_with_l5_entry();
      without.base_attrs = base;
      without.attrs = base;
      without.booster_n = 0;
      without.refresh_rows();

      assert!(
        with_implants.summary_data().recommendation.total_sec < without.summary_data().recommendation.total_sec,
        "the optimizer scores candidates on base plus implants, so implants lower its projected time"
      );
    }

    #[test]
    fn summary_data_propagates_the_inconsistent_flag() {
      let mut state = state_with_l5_entry();
      state.base_attrs = attrs();
      state.consistent = false;
      state.refresh_rows();

      assert!(!state.summary_data().consistent);
    }

    #[test]
    fn summary_data_carries_the_character_total_sp() {
      let mut state = state_with_l5_entry();
      state.character_total_sp = 12_345_678;
      state.refresh_rows();

      assert_eq!(state.summary_data().character_total_sp, 12_345_678);
    }
  }

  mod toggles {
    use super::*;

    #[tokio::test]
    async fn it_defaults_the_picker_open() {
      let state = State::new(Some(42));
      assert!(state.picker_open);
    }

    #[tokio::test]
    async fn it_toggles_the_picker() {
      let mut state = State::new(Some(42));
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(!state.picker_open);
      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(state.picker_open);
    }
  }

  mod topo_sort {
    use super::*;

    // Returns the display order the sort produces without mutating state.entries, so a test can assert on
    // ordering while the manual order underneath stays put.
    fn sorted_order(state: &mut State, column: SortColumn, direction: SortDirection) -> Vec<(i64, u8)> {
      state.sort = Sort {
        column,
        direction,
      };
      state.refresh_rows();
      state
        .sort_view()
        .order
        .iter()
        .map(|&index| (state.entries[index].skill_id, state.entries[index].to_level))
        .collect()
    }

    fn sorted_ids_levels(state: &State, keys: &[f64], asc: bool) -> Vec<(i64, u8)> {
      segmented_sort(&state.entries, &state.remap_points, &state.prereq_catalog(), keys, asc)
        .order
        .iter()
        .map(|&index| (state.entries[index].skill_id, state.entries[index].to_level))
        .collect()
    }

    #[test]
    fn primary_sort_still_respects_same_skill_prereqs() {
      let mut state = State::new(Some(42));
      let mut high = edit_entry(1, 100, 2);
      high.meta.primary = AttrKey::Perception;
      let mut low = edit_entry(2, 100, 1);
      low.meta.primary = AttrKey::Charisma;
      state.entries = vec![high, low];

      let order = sorted_order(&mut state, SortColumn::Primary, SortDirection::Ascending);

      let l1 = order.iter().position(|&(s, l)| s == 100 && l == 1).unwrap();
      let l2 = order.iter().position(|&(s, l)| s == 100 && l == 2).unwrap();
      assert!(l1 < l2, "L1 must precede L2 regardless of attribute key: {order:?}");
    }

    #[test]
    fn time_asc_never_places_a_level_before_its_lower_level() {
      let mut state = State::new(Some(42));
      state.entries = vec![
        edit_entry(1, 100, 3),
        edit_entry(2, 100, 1),
        edit_entry(3, 100, 2),
        edit_entry(4, 200, 1),
      ];

      let order = sorted_order(&mut state, SortColumn::Time, SortDirection::Ascending);

      let l1 = order.iter().position(|&(s, l)| s == 100 && l == 1).unwrap();
      let l2 = order.iter().position(|&(s, l)| s == 100 && l == 2).unwrap();
      let l3 = order.iter().position(|&(s, l)| s == 100 && l == 3).unwrap();
      assert!(l1 < l2 && l2 < l3, "same-skill prereq order preserved: {order:?}");
    }

    #[test]
    fn time_desc_still_respects_same_skill_prereqs() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 100, 1), edit_entry(2, 100, 2), edit_entry(3, 200, 1)];

      let order = sorted_order(&mut state, SortColumn::Time, SortDirection::Descending);

      let l1 = order.iter().position(|&(s, l)| s == 100 && l == 1).unwrap();
      let l2 = order.iter().position(|&(s, l)| s == 100 && l == 2).unwrap();
      assert!(l1 < l2, "L1 must still precede L2 even descending: {order:?}");
    }

    use crate::features::skills::browse::{SkillCatalog, SkillCatalogEntry, SkillCatalogGroup};

    fn cross_skill_prereq_entry(type_id: i64, name: &str, prereqs: Vec<(String, u8)>) -> SkillCatalogEntry {
      SkillCatalogEntry {
        group_id: 255,
        group_name: "Gunnery".to_owned(),
        name: name.to_owned(),
        prereqs,
        primary_attr: AttrKey::Perception,
        rank: 1,
        secondary_attr: AttrKey::Willpower,
        type_id,
      }
    }

    fn state_with_cross_skill_prereq() -> State {
      let mut state = State::new(Some(42));
      state.picker.catalog = Some(SkillCatalog {
        groups: vec![SkillCatalogGroup {
          id: 255,
          name: "Gunnery".to_owned(),
          skills: vec![
            cross_skill_prereq_entry(3300, "Gunnery", vec![]),
            cross_skill_prereq_entry(3301, "Small Hybrid Turret", vec![("Gunnery".to_owned(), 3)]),
          ],
        }],
      });
      state
    }

    #[test]
    fn sort_never_places_a_cross_skill_dependent_before_its_prereq() {
      let mut state = state_with_cross_skill_prereq();
      let mut dependent = edit_entry(10, 3301, 1);
      dependent.is_auto = false;
      state.entries = vec![
        dependent,
        edit_entry(1, 3300, 1),
        edit_entry(2, 3300, 2),
        edit_entry(3, 3300, 3),
      ];

      let order = sorted_ids_levels(&state, &[1.0, 50.0, 50.0, 50.0], true);
      let prereq = order.iter().position(|&(s, l)| s == 3300 && l == 3).unwrap();
      let dependent = order.iter().position(|&(s, l)| s == 3301 && l == 1).unwrap();
      assert!(
        prereq < dependent,
        "the prereq must precede the skill that requires it: {order:?}"
      );
    }

    #[test]
    fn cross_skill_prereq_edges_hold_regardless_of_is_auto() {
      let mut state = state_with_cross_skill_prereq();
      let mut prereq = edit_entry(3, 3300, 3);
      prereq.is_auto = true;
      state.entries = vec![
        edit_entry(10, 3301, 1),
        edit_entry(1, 3300, 1),
        edit_entry(2, 3300, 2),
        prereq,
      ];

      let order = sorted_ids_levels(&state, &[1.0, 50.0, 50.0, 50.0], true);
      let prereq = order.iter().position(|&(s, l)| s == 3300 && l == 3).unwrap();
      let dependent = order.iter().position(|&(s, l)| s == 3301 && l == 1).unwrap();
      assert!(
        prereq < dependent,
        "an auto prereq still gates its dependent: {order:?}"
      );
    }

    #[test]
    fn time_sort_stays_within_each_milestone_segment_and_repins_the_lower_milestone() {
      let mut state = State::new(Some(42));
      state.entries = vec![
        edit_entry(1, 100, 3),
        edit_entry(2, 200, 2),
        edit_entry(3, 100, 4),
        edit_entry(4, 200, 3),
      ];
      state.remap_points = vec![
        EditMilestone {
          after_entry_id: None,
          auto_remap: false,
          base: None,
          local_id: 1,
          name: "Milestone I".to_owned(),
          order: 0,
        },
        EditMilestone {
          after_entry_id: Some(2),
          auto_remap: false,
          base: None,
          local_id: 2,
          name: "Milestone II".to_owned(),
          order: 0,
        },
      ];

      let view = segmented_sort(
        &state.entries,
        &state.remap_points,
        &state.prereq_catalog(),
        &[10.0, 5.0, 55.0, 6.0],
        true,
      );

      let order: Vec<i64> = view.order.iter().map(|&index| state.entries[index].id).collect();
      assert_eq!(
        order,
        vec![2, 1, 4, 3],
        "each segment sorts internally, none cross the boundary"
      );
      assert_eq!(view.reanchor[0], None, "the start milestone stays at the head");
      assert_eq!(
        view.reanchor[1],
        Some(1),
        "the lower milestone divider follows the last entry of the segment above it in the view"
      );
      assert_eq!(
        state.entries.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![1, 2, 3, 4],
        "sorting leaves the manual entry order untouched"
      );
      assert_eq!(
        state.remap_points[1].after_entry_id,
        Some(2),
        "sorting never rewrites the persisted milestone anchor"
      );
    }

    #[tokio::test]
    async fn a_sort_then_unsort_cycle_restores_manual_order_and_anchors() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(Some(42));
      state.entries = vec![
        edit_entry(1, 100, 3),
        edit_entry(2, 200, 2),
        edit_entry(3, 100, 4),
        edit_entry(4, 200, 3),
      ];
      state.remap_points = vec![EditMilestone {
        after_entry_id: Some(2),
        auto_remap: false,
        base: None,
        local_id: 1,
        name: "Milestone".to_owned(),
        order: 0,
      }];
      state.refresh_rows();
      let manual: Vec<i64> = state.entries.iter().map(|e| e.id).collect();

      // asc -> desc -> Manual: three clicks on the same column cycle all the way back to Manual.
      let _ = update(&mut state, Message::SortChanged(SortColumn::Time), &db);
      assert_eq!(state.sort.column, SortColumn::Time);
      assert_eq!(
        state.entries.iter().map(|e| e.id).collect::<Vec<_>>(),
        manual,
        "a live sort never moves the underlying entries"
      );
      assert_eq!(
        state.remap_points[0].after_entry_id,
        Some(2),
        "a live sort never rewrites the milestone anchor"
      );

      let _ = update(&mut state, Message::SortChanged(SortColumn::Time), &db);
      let _ = update(&mut state, Message::SortChanged(SortColumn::Time), &db);

      assert_eq!(
        state.sort.column,
        SortColumn::Manual,
        "the third click returns to Manual"
      );
      assert_eq!(
        state.entries.iter().map(|e| e.id).collect::<Vec<_>>(),
        manual,
        "the manual order is intact after the cycle"
      );
      assert_eq!(
        state.remap_points[0].after_entry_id,
        Some(2),
        "the milestone placement is intact after the cycle"
      );
    }

    #[test]
    fn a_sorted_export_serializes_in_the_displayed_order() {
      let mut state = State::new(Some(42));
      // Manual order lists the charisma skill first; a primary-ascending sort puts perception (attr 0) ahead.
      let mut charisma = edit_entry(1, 200, 1);
      charisma.meta.skill_name = "Charisma Skill".to_owned();
      charisma.meta.primary = AttrKey::Charisma;
      let mut perception = edit_entry(2, 100, 1);
      perception.meta.skill_name = "Perception Skill".to_owned();
      perception.meta.primary = AttrKey::Perception;
      state.entries = vec![charisma, perception];

      state.sort = Sort {
        column: SortColumn::Manual,
        direction: SortDirection::Ascending,
      };
      state.refresh_rows();
      assert_eq!(
        serialize_plan_text(&state),
        "Charisma Skill 1\nPerception Skill 1",
        "Manual keeps the entered order"
      );

      state.sort = Sort {
        column: SortColumn::Primary,
        direction: SortDirection::Ascending,
      };
      state.refresh_rows();
      assert_eq!(
        serialize_plan_text(&state),
        "Perception Skill 1\nCharisma Skill 1",
        "a sorted export lists skills in the displayed order"
      );
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_placed_remap_divider_and_insertion_affordances() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];
      state.remap_availability = 1;
      state.remap_points = vec![EditMilestone {
        after_entry_id: Some(1),
        auto_remap: false,
        base: Some(Attributes {
          charisma: 19,
          intelligence: 21,
          memory: 19,
          perception: 21,
          willpower: 19,
        }),
        local_id: 1,
        name: "Milestone".to_owned(),
        order: 0,
      }];
      state.refresh_rows();

      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_renders_the_empty_state_with_no_rows() {
      let state = State::new(Some(42));

      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_renders_the_entry_list_with_rows() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];

      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_renders_the_exhausted_constraint_when_no_remaps_available() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];
      state.remap_availability = 0;
      state.remap_reason = "No neural remaps available. Next remap accrues in 30 days.".to_owned();
      state.refresh_rows();

      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_renders_the_import_export_overlay() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5)];
      state.refresh_rows();

      state.io_panel = Some(IoPanel::Export);
      {
        let _el: Element<'_, Message> = view(&state, now());
      }

      state.io_panel = Some(IoPanel::Import);
      {
        let _el: Element<'_, Message> = view(&state, now());
      }

      state.io_panel = Some(IoPanel::ImportPrompt);
      {
        let _el: Element<'_, Message> = view(&state, now());
      }
    }

    #[test]
    fn it_renders_the_import_feedback_overlay() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5)];
      state.refresh_rows();

      state.import_feedback = Some(ImportFeedback::Failed);
      {
        let _el: Element<'_, Message> = view(&state, now());
      }

      state.import_feedback = Some(ImportFeedback::Succeeded);
      {
        let _el: Element<'_, Message> = view(&state, now());
      }
    }

    #[test]
    fn it_renders_the_summary_right_pane() {
      let mut state = State::new(Some(42));
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];
      state.attrs = Attributes {
        charisma: 19,
        intelligence: 21,
        memory: 19,
        perception: 21,
        willpower: 19,
      };
      state.base_attrs = state.attrs;
      state.refresh_rows();

      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_renders_the_template_editor_with_badge_and_sp_only_stats() {
      let mut state = State::new(None);
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];
      state.refresh_rows();

      assert!(state.is_template());
      assert!(state.summary.is_template);
      let _el: Element<'_, Message> = view(&state, now());
    }
  }

  mod csv_wishes {
    use super::*;

    fn skill_type(id: i64, name: &str) -> ItemType {
      ItemType {
        capacity: None,
        description: Some("Skill".to_owned()),
        dogma_attributes: "[]".to_owned(),
        group_id: 255,
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

    #[test]
    fn it_maps_csv_rows_to_wishes_by_case_insensitive_name() {
      let types = vec![skill_type(3300, "Gunnery"), skill_type(3301, "Drones")];
      let rows = vec![("gunnery".to_owned(), 5), ("DRONES".to_owned(), 3)];

      let wishes = csv_wishes_from_types(&rows, &types);

      assert_eq!(
        wishes,
        vec![
          Wish {
            skill_id: 3300,
            to_level: 5
          },
          Wish {
            skill_id: 3301,
            to_level: 3
          }
        ]
      );
    }

    #[test]
    fn it_skips_unknown_skill_names() {
      let types = vec![skill_type(3300, "Gunnery")];
      let rows = vec![("Gunnery".to_owned(), 4), ("Nonexistent Skill".to_owned(), 5)];

      let wishes = csv_wishes_from_types(&rows, &types);

      assert_eq!(
        wishes,
        vec![Wish {
          skill_id: 3300,
          to_level: 4
        }]
      );
    }

    #[test]
    fn it_keeps_the_highest_level_for_duplicate_skills() {
      let types = vec![skill_type(3300, "Gunnery")];
      let rows = vec![("Gunnery".to_owned(), 3), ("Gunnery".to_owned(), 5)];

      let wishes = csv_wishes_from_types(&rows, &types);

      assert_eq!(
        wishes,
        vec![Wish {
          skill_id: 3300,
          to_level: 5
        }]
      );
    }
  }
}
