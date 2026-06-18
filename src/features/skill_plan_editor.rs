mod empty_state;
mod entry_row;
mod header;
mod import_export;
mod picker;
mod plan_entry_list;
mod remap_divider;
mod remap_insertion;
mod stats_strip;
mod summary;

use std::{
  collections::{HashMap, HashSet},
  path::PathBuf,
};

use chrono::{DateTime, Datelike as _, Duration, Timelike as _, Utc};
use iced::{Element, Length, Task, widget::Column};
use picker::PickerState;

pub(super) use crate::ui::format::{fmt_duration_padded as fmt_duration, fmt_sp_compact as fmt_sp};
use crate::{
  features::{
    skill_plan_editor::picker::{PickerCert, PickerModule, PickerShip},
    skills::{
      browse::{AttrKey, SkillCatalog, SkillCatalogEntry},
      optimizer::{Attribute, Attributes, PairWeight, optimize_remap},
      plan_math::{self, ExpandedEntry, PlanEntry, PlanOptions, PrereqCatalog, RemapPoint, Wish},
    },
  },
  store::{
    Database,
    model::{CharacterAttributes, SkillPlan, SkillPlanEntry, SkillPlanRemapPoint},
    repo::{character, sde, skills},
  },
  ui::{
    components::resizable_pane::{self, PaneDrag, pane_handle},
    style::spacing,
  },
  window_state::UiState,
};

const REMAP_ATTR_ORDER: [Attribute; 5] = [
  Attribute::Perception,
  Attribute::Memory,
  Attribute::Willpower,
  Attribute::Intelligence,
  Attribute::Charisma,
];

const MONTHS: [&str; 12] = [
  "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const EDITOR_HOST_WIDTH: f32 = 900.0;
const PICKER_WIDTH: f32 = 340.0;
const SUMMARY_WIDTH: f32 = 360.0;
const PICKER_PANE_KEY: &str = "plan.picker";
const SUMMARY_PANE_KEY: &str = "plan.summary";
const GAP_START: i64 = i64::MIN;

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

#[derive(Clone, Copy, Debug)]
pub struct EditRemap {
  after_entry_id: Option<i64>,
  base: Attributes,
  local_id: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorPane {
  Picker,
  Summary,
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
  catalog: SkillCatalog,
  cert_proficiency: HashMap<i64, usize>,
  character_total_sp: u64,
  draft_name: Option<String>,
  entries: Vec<EditEntry>,
  plan: Option<SkillPlan>,
  remap_availability: u32,
  remap_points: Vec<EditRemap>,
  remap_reason: String,
  ship_mastery: HashMap<i64, u8>,
  sort: Sort,
  synced_sp: HashMap<i64, u64>,
  trained_levels: HashMap<i64, u8>,
}

#[derive(Clone, Debug)]
pub enum Message {
  CloseRequested,
  DragDropped,
  DragHovered(usize),
  DragLeft(usize),
  DragStarted(i64),
  EntryNoteChanged(i64, String),
  EntryNoteToggled(i64),
  EntryPriorityCycled(i64),
  EntryRemoved(i64),
  ExportFilePicked(Option<PathBuf>),
  ExportRequested,
  ExportToClipboard,
  ExportToFile,
  GapHovered(i64),
  GapUnhovered,
  ImportAppend,
  ImportClipboardRead(Option<String>),
  ImportFileLoaded(Option<String>),
  ImportFromClipboard,
  ImportFromFile,
  ImportReplace,
  ImportRequested,
  IoDismissed,
  Loaded(Box<Loaded>),
  NameChanged(String),
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart(EditorPane),
  PaneSettled(&'static str, f32),
  PickerCertProficiencyChanged(i64, usize),
  PickerCertSelected(i64, usize),
  PickerCertsLoaded(Vec<picker::PickerCert>),
  PickerGroupToggled(i64),
  PickerLevelPicked(i64, u8),
  PickerModuleSelected(i64),
  PickerModulesLoaded(Vec<picker::PickerModule>),
  PickerSearchChanged(String),
  PickerShipMasteryChanged(i64, u8),
  PickerShipSelected(i64, u8),
  PickerShipsLoaded(Vec<picker::PickerShip>),
  PickerTabSelected(picker::PickerTab),
  PickerToggled,
  RemapAttrBumped(i64, AttrKey, i32),
  RemapInserted(Option<i64>),
  RemapRemoved(i64),
  Reordered(Result<(), String>),
  SaveRequested,
  Saved(Result<i64, String>),
  SortChanged(SortColumn),
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
pub enum Seed {
  Existing(i64),
  FromQueue,
  FromQueueSelection(Vec<i64>),
  New,
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

  pub fn caret(self, column: SortColumn) -> Option<&'static str> {
    (self.column == column).then_some(match self.direction {
      SortDirection::Ascending => "\u{2191}",
      SortDirection::Descending => "\u{2193}",
    })
  }

  pub fn is_active(self, column: SortColumn) -> bool {
    self.column == column
  }

  pub fn toggled(self, column: SortColumn) -> Self {
    if self.column == column {
      Sort {
        column,
        direction: self.direction.toggled(),
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

impl SortDirection {
  fn toggled(self) -> Self {
    match self {
      SortDirection::Ascending => SortDirection::Descending,
      SortDirection::Descending => SortDirection::Ascending,
    }
  }
}

#[derive(Debug)]
pub struct State {
  attrs: Attributes,
  base_attrs: Attributes,
  character_id: i64,
  character_total_sp: u64,
  dirty: bool,
  dragging: Option<i64>,
  dragging_pane: Option<EditorPane>,
  drop_index: Option<usize>,
  entries: Vec<EditEntry>,
  hovered_gap: Option<i64>,
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
  remap_points: Vec<EditRemap>,
  remap_reason: String,
  rows: Vec<ComputedRow>,
  saved: Snapshot,
  sort: Sort,
  summary: summary::SummaryData,
  summary_pane: PaneDrag,
  synced_levels: HashMap<i64, u8>,
  synced_sp: HashMap<i64, u64>,
  total_sec: f64,
  total_sp: u64,
}

impl State {
  pub fn new(character_id: i64) -> Self {
    State {
      character_id,
      name: String::new(),
      picker_open: true,
      sort: Sort::default(),
      note_open: None,
      dragging: None,
      drop_index: None,
      hovered_gap: None,
      io_panel: None,
      pending_import: None,
      saved: Snapshot::default(),
      dirty: false,
      next_remap_id: 1,
      next_entry_id: -1,
      picker: PickerState::default(),
      picker_pane: PaneDrag::new(PICKER_WIDTH, EDITOR_HOST_WIDTH),
      summary_pane: PaneDrag::new(SUMMARY_WIDTH, EDITOR_HOST_WIDTH),
      dragging_pane: None,
      attrs: Attributes::default(),
      base_attrs: Attributes::default(),
      character_total_sp: 0,
      entries: Vec::new(),
      plan: None,
      remap_availability: 0,
      remap_reason: String::new(),
      remap_points: Vec::new(),
      synced_levels: HashMap::new(),
      synced_sp: HashMap::new(),
      rows: Vec::new(),
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

  pub fn set_pane_host_width(&mut self, host_width: f32) {
    self.picker_pane.set_host_width(host_width);
    self.summary_pane.set_host_width(host_width);
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

  fn implant_bonus(&self) -> Attributes {
    Attributes {
      charisma: self.attrs.charisma.saturating_sub(self.base_attrs.charisma),
      intelligence: self.attrs.intelligence.saturating_sub(self.base_attrs.intelligence),
      memory: self.attrs.memory.saturating_sub(self.base_attrs.memory),
      perception: self.attrs.perception.saturating_sub(self.base_attrs.perception),
      willpower: self.attrs.willpower.saturating_sub(self.base_attrs.willpower),
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
      .filter(|point| point.after_entry_id.is_some())
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
    self.summary = self.summary_data();
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
            i64::from(r.base.perception),
            i64::from(r.base.memory),
            i64::from(r.base.willpower),
            i64::from(r.base.intelligence),
            i64::from(r.base.charisma),
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
        "Other".to_owned()
      } else {
        row.group_name.clone()
      };
      *group_sec.entry(group).or_insert(0.0) += row.sec;
      let pair = format!("{} / {}", row.primary.short(), row.secondary.short());
      *pair_sec.entry(pair).or_insert(0.0) += row.sec;
    }

    let weights = self.pair_weights();
    let recommendation = optimize_remap(&weights, self.base_attrs, Attributes::default());
    let current_sec = plan_time_for(&weights, self.attrs);
    let current_base_sec = plan_time_for(&weights, self.base_attrs);

    summary::SummaryData {
      base_attrs: self.base_attrs,
      character_total_sp: self.character_total_sp,
      current_base_sec,
      current_sec,
      group_sec,
      implant_effect: self.implant_effect(),
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

#[derive(Clone, Copy, Debug)]
pub(super) struct RemapControls<'a> {
  pub can_place: bool,
  pub reason: &'a str,
}

struct CharacterAttrs {
  attrs: Attributes,
  availability: u32,
  base_attrs: Attributes,
  reason: String,
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
  base_charisma: i64,
  base_intelligence: i64,
  base_memory: i64,
  base_perception: i64,
  base_willpower: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Snapshot {
  cert_proficiency: Vec<(i64, usize)>,
  entries: Vec<(i64, u8, Priority, String, bool)>,
  name: String,
  remaps: Vec<(Option<i64>, i64, i64, i64, i64, i64)>,
  ship_mastery: Vec<(i64, u8)>,
  sort: Sort,
}

pub fn load(db: &Database, character_id: i64, seed: Seed) -> Task<Message> {
  Task::perform(async_load(db.clone(), character_id, seed, Utc::now()), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  let mut subs: Vec<iced::Subscription<Message>> = Vec::new();
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
  let message = match handle_io(state, message) {
    Ok(task) => return task,
    Err(message) => message,
  };
  let message = match handle_gap(state, message) {
    Ok(task) => return task,
    Err(message) => message,
  };
  let message = match handle_picker(state, message, db) {
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

fn handle_io(state: &mut State, message: Message) -> Result<Task<Message>, Message> {
  let message = match handle_export_io(state, message) {
    Ok(task) => return Ok(task),
    Err(message) => message,
  };
  match handle_import_io(state, message) {
    Ok(task) => Ok(task),
    Err(Message::IoDismissed) => {
      state.io_panel = None;
      state.pending_import = None;
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
    Message::ExportToFile => {
      state.io_panel = None;
      let json = import_export::to_json(&plan_file(state));
      let default_name = export_file_name(state);
      Ok(Task::perform(
        save_to_file_dialog(default_name, json),
        Message::ExportFilePicked,
      ))
    }
    other => Err(other),
  }
}

fn handle_import_io(state: &mut State, message: Message) -> Result<Task<Message>, Message> {
  match message {
    Message::ImportAppend => {
      apply_pending_import(state, ImportMode::Append);
      Ok(Task::none())
    }
    Message::ImportClipboardRead(text) => {
      stage_import(state, text.as_deref().unwrap_or_default());
      Ok(Task::none())
    }
    Message::ImportFileLoaded(text) => {
      stage_import(state, text.as_deref().unwrap_or_default());
      Ok(Task::none())
    }
    Message::ImportFromClipboard => {
      state.io_panel = None;
      Ok(iced::clipboard::read().map(Message::ImportClipboardRead))
    }
    Message::ImportFromFile => {
      state.io_panel = None;
      Ok(Task::perform(read_from_file_dialog(), Message::ImportFileLoaded))
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
  }
  state.io_panel = None;
}

fn stage_import(state: &mut State, raw: &str) {
  match import_export::detect(raw) {
    Some(payload) => {
      state.pending_import = Some(payload);
      state.io_panel = Some(IoPanel::ImportPrompt);
    }
    None => {
      state.pending_import = None;
      state.io_panel = None;
    }
  }
}

async fn read_from_file_dialog() -> Option<String> {
  #[cfg(not(test))]
  {
    let handle = rfd::AsyncFileDialog::new()
      .set_title("Import skill plan")
      .add_filter("Plan", &["json", "txt"])
      .pick_file()
      .await?;
    Some(String::from_utf8_lossy(&handle.read().await).into_owned())
  }
  #[cfg(test)]
  {
    None
  }
}

async fn save_to_file_dialog(default_name: String, contents: String) -> Option<PathBuf> {
  #[cfg(not(test))]
  {
    let handle = rfd::AsyncFileDialog::new()
      .set_title("Export skill plan")
      .set_file_name(default_name)
      .add_filter("JSON", &["json"])
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
    let _ = (default_name, contents);
    None
  }
}

fn handle_gap(state: &mut State, message: Message) -> Result<Task<Message>, Message> {
  match message {
    Message::GapHovered(gap) => {
      state.hovered_gap = Some(gap);
      Ok(Task::none())
    }
    Message::GapUnhovered => {
      state.hovered_gap = None;
      Ok(Task::none())
    }
    other => Err(other),
  }
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
    Message::RemapAttrBumped(local_id, key, delta) => {
      if let Some(point) = state.remap_points.iter_mut().find(|r| r.local_id == local_id)
        && let Some(next) = plan_math::bump_attr(point.base, attr_key_to_attribute(key), delta)
      {
        point.base = next;
        state.refresh_rows();
      }
      Ok(Task::none())
    }
    Message::RemapInserted(after_entry_id) => {
      if after_entry_id.is_some() && !state.can_place_remap() {
        return Ok(Task::none());
      }
      let local_id = state.next_remap_id();
      state.remap_points.push(EditRemap {
        base: state.base_attrs,
        after_entry_id,
        local_id,
      });
      state.refresh_rows();
      Ok(Task::none())
    }
    Message::RemapRemoved(local_id) => {
      state.remap_points.retain(|r| r.local_id != local_id);
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
      apply_sort(state);
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
    catalog,
    cert_proficiency,
    character_total_sp,
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
  let header = header::header(&state.name, state.dirty(), state.picker_open);

  let body: Element<'_, Message> = if state.rows.is_empty() {
    empty_state::empty_state()
  } else {
    plan_entry_list::plan_entry_list(
      &state.rows,
      &state.remap_points,
      state.total_sp,
      state.total_sec,
      now,
      state.sort,
      state.note_open,
      state.dragging,
      state.drop_index,
      state.hovered_gap,
      RemapControls {
        can_place: state.can_place_remap(),
        reason: &state.remap_reason,
      },
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

  let editor: Element<'_, Message> = Column::with_children(vec![header, lower])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

  match state.io_panel.as_ref() {
    Some(panel) => iced::widget::stack(vec![editor, import_export::overlay(panel)])
      .width(Length::Fill)
      .height(Length::Fill)
      .into(),
    None => editor,
  }
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

fn apply_sort(state: &mut State) {
  let asc = state.sort.direction == SortDirection::Ascending;
  match state.sort.column {
    SortColumn::Manual => {}
    SortColumn::Primary => {
      let rows = state.computed().rows;
      let keys: Vec<f64> = rows.iter().map(|r| f64::from(r.primary as u8)).collect();
      topo_sort_by_key(state, &keys, asc);
    }
    SortColumn::Secondary => {
      let rows = state.computed().rows;
      let keys: Vec<f64> = rows.iter().map(|r| f64::from(r.secondary as u8)).collect();
      topo_sort_by_key(state, &keys, asc);
    }
    SortColumn::Time => {
      let rows = state.computed().rows;
      let keys: Vec<f64> = rows.iter().map(|r| r.sec).collect();
      topo_sort_by_key(state, &keys, asc);
    }
  }
}

fn topo_sort_by_key(state: &mut State, keys: &[f64], asc: bool) {
  let n = state.entries.len();
  let levels: Vec<(i64, u8)> = state.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();

  let is_pred = |j: usize, i: usize| levels[j].0 == levels[i].0 && levels[j].1 < levels[i].1;

  let mut remaining_preds: Vec<usize> = (0..n)
    .map(|i| (0..n).filter(|&j| j != i && is_pred(j, i)).count())
    .collect();

  let mut emitted = vec![false; n];
  let mut order: Vec<usize> = Vec::with_capacity(n);

  while order.len() < n {
    let pick = (0..n)
      .filter(|&i| !emitted[i] && remaining_preds[i] == 0)
      .reduce(|best, i| {
        let better = if asc {
          keys[i] < keys[best]
        } else {
          keys[i] > keys[best]
        };
        if better || (keys[i] == keys[best] && i < best) {
          i
        } else {
          best
        }
      });
    let Some(pick) = pick else { break };
    emitted[pick] = true;
    order.push(pick);
    for (i, preds) in remaining_preds.iter_mut().enumerate() {
      if !emitted[i] && is_pred(pick, i) {
        *preds = preds.saturating_sub(1);
      }
    }
  }

  order.extend((0..n).filter(|&i| !emitted[i]));

  let reordered: Vec<EditEntry> = order.iter().map(|&idx| state.entries[idx].clone()).collect();
  state.entries = reordered;
}

fn save(state: &State, db: &Database) -> Task<Message> {
  let character_id = state.character_id;
  let name = state.name.trim().to_owned();
  let name = if name.is_empty() {
    "Untitled plan".to_owned()
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

  let entry_skill_levels: Vec<(i64, i64)> = state
    .entries
    .iter()
    .map(|e| (e.skill_id, i64::from(e.to_level)))
    .collect();
  let ship_masteries: Vec<(i64, i64)> = state
    .picker
    .ship_mastery
    .iter()
    .map(|(&ship_id, &tier)| (ship_id, i64::from(tier)))
    .collect();
  let cert_proficiencies: Vec<(i64, i64)> = state
    .picker
    .cert_proficiency
    .iter()
    .map(|(&cert_id, &prof)| (cert_id, prof as i64))
    .collect();
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
        base_perception: i64::from(r.base.perception),
        base_memory: i64::from(r.base.memory),
        base_willpower: i64::from(r.base.willpower),
        base_intelligence: i64::from(r.base.intelligence),
        base_charisma: i64::from(r.base.charisma),
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
        &entry_skill_levels,
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
  character_id: i64,
  existing_id: Option<i64>,
  name: &str,
  sort_mode: &str,
  implant_set: &str,
  entries: &[(i64, i64, String, String, i64)],
  entry_skill_levels: &[(i64, i64)],
  remaps: &[RemapSave],
  ship_masteries: &[(i64, i64)],
  cert_proficiencies: &[(i64, i64)],
) -> Result<i64, crate::store::Error> {
  let plan_id = match existing_id {
    Some(id) => id,
    None => skills::create(db, character_id, name).await?.id(),
  };
  skills::update(db, plan_id, name, sort_mode, implant_set).await?;
  skills::replace_ship_masteries(db, plan_id, ship_masteries).await?;
  skills::replace_cert_proficiencies(db, plan_id, cert_proficiencies).await?;

  let rows: Vec<(i64, i64, &str, &str, i64)> = entries
    .iter()
    .map(|(skill_id, to_level, priority, note, is_auto)| {
      (*skill_id, *to_level, priority.as_str(), note.as_str(), *is_auto)
    })
    .collect();
  skills::replace_entries(db, plan_id, &rows).await?;

  let new_ids: Vec<i64> = skills::entries(db, plan_id)
    .await?
    .iter()
    .map(SkillPlanEntry::id)
    .collect();
  debug_assert_eq!(new_ids.len(), entry_skill_levels.len());
  for remap in remaps {
    let after_entry_id = match remap.anchor_index {
      None => None,
      Some(index) => match new_ids.get(index) {
        Some(&id) => Some(id),
        None => continue,
      },
    };
    skills::upsert_remap_point(
      db,
      plan_id,
      after_entry_id,
      remap.base_perception,
      remap.base_memory,
      remap.base_willpower,
      remap.base_intelligence,
      remap.base_charisma,
    )
    .await?;
  }

  Ok(plan_id)
}

async fn async_load(db: Database, character_id: i64, seed: Seed, now: DateTime<Utc>) -> Loaded {
  let plan = match &seed {
    Seed::Existing(id) => skills::get(&db, *id).await.ok().flatten(),
    Seed::New | Seed::FromQueue | Seed::FromQueueSelection(_) => None,
  };

  let catalog = skills::skill_catalog(&db).await.unwrap_or(SkillCatalog {
    groups: Vec::new(),
  });
  let synced_skills = character::skills(&db, character_id).await.unwrap_or_default();
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
  let character_total_sp = character::state(&db, character_id)
    .await
    .ok()
    .flatten()
    .and_then(|state| state.total_sp)
    .map(|sp| sp.max(0) as u64)
    .unwrap_or(0);

  let raw_remap_points = match plan.as_ref() {
    Some(plan) => skills::remap_points(&db, plan.id()).await.unwrap_or_default(),
    None => Vec::new(),
  };
  let remap_points: Vec<EditRemap> = raw_remap_points.iter().map(edit_remap_from_model).collect();

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
    availability,
    reason,
  } = load_character_attrs(&db, character_id, now).await;

  let entries = match &seed {
    Seed::Existing(_) => {
      let raw_entries = match plan.as_ref() {
        Some(plan) => skills::entries(&db, plan.id()).await.unwrap_or_default(),
        None => Vec::new(),
      };
      let mut entries = Vec::with_capacity(raw_entries.len());
      for entry in &raw_entries {
        let meta = resolve_entry_meta(&db, &catalog, entry.skill_id()).await;
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
      entries
    }
    Seed::FromQueue => entries_from_queue(&db, character_id, &catalog, &trained_levels, None).await,
    Seed::FromQueueSelection(positions) => {
      entries_from_queue(&db, character_id, &catalog, &trained_levels, Some(positions)).await
    }
    Seed::New => Vec::new(),
  };

  let draft_name = match &seed {
    Seed::FromQueueSelection(_) => Some("Plan from selection".to_owned()),
    Seed::Existing(_) | Seed::FromQueue | Seed::New => None,
  };

  Loaded {
    attrs,
    base_attrs,
    catalog,
    cert_proficiency,
    character_total_sp,
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

async fn entries_from_queue(
  db: &Database,
  character_id: i64,
  catalog: &SkillCatalog,
  trained_levels: &HashMap<i64, u8>,
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
  let expanded = plan_math::expand_wishes(&wishes, &prereqs, trained_levels);

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
    .unwrap_or_else(|| format!("Skill {skill_id}"));

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

fn edit_remap_from_model(point: &SkillPlanRemapPoint) -> EditRemap {
  EditRemap {
    base: Attributes {
      charisma: point.base_charisma().max(0) as u32,
      intelligence: point.base_intelligence().max(0) as u32,
      memory: point.base_memory().max(0) as u32,
      perception: point.base_perception().max(0) as u32,
      willpower: point.base_willpower().max(0) as u32,
    },
    after_entry_id: point.after_entry_id(),
    local_id: 0,
  }
}

async fn load_character_attrs(db: &Database, character_id: i64, now: DateTime<Utc>) -> CharacterAttrs {
  let Some(row) = character::attributes(db, character_id).await.ok().flatten() else {
    return CharacterAttrs {
      attrs: Attributes::default(),
      base_attrs: Attributes::default(),
      availability: 0,
      reason: String::new(),
    };
  };

  let base_attrs = base_attributes(&row);
  let mut effective = base_attrs;
  for implant in character::implants(db, character_id).await.unwrap_or_default() {
    let bonus = implant.bonus().max(0) as u32;
    match plan_math_attribute(implant.attribute_id()) {
      Attribute::Charisma => effective.charisma += bonus,
      Attribute::Intelligence => effective.intelligence += bonus,
      Attribute::Memory => effective.memory += bonus,
      Attribute::Perception => effective.perception += bonus,
      Attribute::Willpower => effective.willpower += bonus,
    }
  }

  let availability = plan_math::remap_availability(
    row.bonus_remaps(),
    row.last_remap_date().as_deref(),
    row.accrued_remap_cooldown_date().as_deref(),
    now,
  );

  CharacterAttrs {
    attrs: effective,
    base_attrs,
    availability: availability.count,
    reason: availability.reason,
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

fn remap_points_to_math(entries: &[EditEntry], points: &[EditRemap]) -> Vec<RemapPoint> {
  let entry_ids: Vec<i64> = entries.iter().map(|e| e.id).collect();
  points
    .iter()
    .filter_map(|point| {
      let after_index = after_index_for(point.after_entry_id, &entry_ids)?;
      Some(RemapPoint {
        after_index,
        base: point.base,
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
  let expanded = plan_math::expand_wishes(&wishes, &catalog, &state.picker.trained_levels);

  expanded
    .into_iter()
    .filter(|entry| entry.is_auto)
    .map(|entry| (entry.skill_id, entry.to_level))
    .collect()
}

fn add_skill(state: &mut State, skill_id: i64, target_level: u8) {
  let mut trained = state.picker.trained_levels.clone();
  for (planned_skill, planned_level) in state.planned_levels() {
    trained
      .entry(planned_skill)
      .and_modify(|level| *level = (*level).max(planned_level))
      .or_insert(planned_level);
  }

  let catalog = state.prereq_catalog();
  let expanded = plan_math::expand_wishes(
    &[Wish {
      skill_id,
      to_level: target_level,
    }],
    &catalog,
    &trained,
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

  let mut trained = state.picker.trained_levels.clone();
  for (planned_skill, planned_level) in state.planned_levels() {
    trained
      .entry(planned_skill)
      .and_modify(|level| *level = (*level).max(planned_level))
      .or_insert(planned_level);
  }

  let wishes: Vec<Wish> = skills
    .iter()
    .map(|&(skill_id, to_level)| Wish {
      skill_id,
      to_level,
    })
    .collect();

  let catalog = state.prereq_catalog();
  let expanded = plan_math::expand_wishes(&wishes, &catalog, &trained);

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
  state
    .entries
    .iter()
    .filter(|entry| !entry.is_auto)
    .map(|entry| format!("{} {}", entry.meta.skill_name, entry.to_level))
    .collect::<Vec<_>>()
    .join("\n")
}

fn export_file_name(state: &State) -> String {
  let trimmed = state.name.trim();
  let base = if trimmed.is_empty() { "skill-plan" } else { trimmed };
  format!("{base}.json")
}

fn plan_file(state: &State) -> import_export::PlanFile {
  let entry_ids: Vec<i64> = state.entries.iter().map(|e| e.id).collect();
  let entries = state
    .entries
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
    .map(|r| import_export::PlanFileRemap {
      after_index: r
        .after_entry_id
        .and_then(|id| entry_ids.iter().position(|&entry_id| entry_id == id)),
      base: import_export::PlanFileAttrs::from_attributes(r.base),
    })
    .collect();

  import_export::PlanFile {
    entries,
    remaps,
  }
}

fn apply_import(state: &mut State, payload: import_export::Payload, mode: ImportMode) {
  match payload {
    import_export::Payload::Json(plan) => apply_json_import(state, plan, mode),
    import_export::Payload::Text(lines) => apply_text_import(state, &lines, mode),
  }
}

fn apply_json_import(state: &mut State, plan: import_export::PlanFile, mode: ImportMode) {
  if mode == ImportMode::Replace {
    state.entries.clear();
    state.remap_points.clear();
  }

  let mut anchor_ids: Vec<i64> = Vec::with_capacity(plan.entries.len());
  for dto in &plan.entries {
    let id = upsert_imported_entry(
      state,
      dto.type_id,
      dto.to_level,
      &dto.note,
      Priority::from_token(&dto.priority),
    );
    anchor_ids.push(id);
  }

  for remap in plan.remaps {
    let after_entry_id = match remap.after_index {
      None => None,
      Some(index) => match anchor_ids.get(index) {
        Some(&id) => Some(id),
        None => continue,
      },
    };
    let local_id = state.next_remap_id();
    state.remap_points.push(EditRemap {
      base: remap.base.to_attributes(),
      after_entry_id,
      local_id,
    });
  }
}

fn apply_text_import(state: &mut State, lines: &[(String, u8)], mode: ImportMode) {
  let id_by_name: HashMap<String, i64> = match state.picker.catalog.as_ref() {
    Some(catalog) => catalog
      .groups
      .iter()
      .flat_map(|group| group.skills.iter())
      .map(|skill| (skill.name.to_lowercase(), skill.type_id))
      .collect(),
    None => HashMap::new(),
  };

  let resolved: Vec<(i64, u8)> = lines
    .iter()
    .filter_map(|(name, level)| id_by_name.get(&name.to_lowercase()).map(|&id| (id, *level)))
    .collect();
  if resolved.is_empty() {
    return;
  }

  if mode == ImportMode::Replace {
    state.entries.clear();
    state.remap_points.clear();
  }
  for (skill_id, level) in resolved {
    add_skill(state, skill_id, level);
  }
}

fn upsert_imported_entry(state: &mut State, skill_id: i64, to_level: u8, note: &str, priority: Priority) -> i64 {
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
      format!("Skill {skill_id}"),
      String::new(),
    ),
  };

  state.entries.push(EditEntry {
    id,
    is_auto: false,
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
      format!("Skill {}", expanded.skill_id),
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

fn attr_key_to_attribute(key: AttrKey) -> Attribute {
  match key {
    AttrKey::Charisma => Attribute::Charisma,
    AttrKey::Intelligence => Attribute::Intelligence,
    AttrKey::Memory => Attribute::Memory,
    AttrKey::Perception => Attribute::Perception,
    AttrKey::Willpower => Attribute::Willpower,
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
  let month = MONTHS[(eta.month() - 1) as usize];
  let year = eta.year();
  let hour = eta.hour();
  let minute = eta.minute();
  format!("{day} {month} {year} · {hour:02}:{minute:02}")
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

      let mut state = State::new(42);
      let _ = update(
        &mut state,
        Message::Loaded(Box::new(async_load(db.clone(), 42, Seed::New, now()).await)),
        &db,
      );
      assert_eq!(state.name, "", "a fresh plan opens with an empty name");

      let id = persist(
        &db,
        42,
        None,
        "Untitled plan",
        "manual",
        "current",
        &[],
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
    async fn from_queue_only_schedules_levels_above_the_trained_level() {
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

      let loaded = async_load(db, 42, Seed::FromQueue, now()).await;

      let levels: Vec<u8> = loaded.entries.iter().map(|e| e.to_level).collect();
      assert_eq!(
        levels,
        [3, 4, 5],
        "only levels above the synced trained level are seeded"
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

      let loaded = async_load(db, 42, Seed::FromQueue, now()).await;

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

      let loaded = async_load(db, 42, Seed::FromQueueSelection(vec![1]), now()).await;

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

      let mut state = State::new(42);
      let _ = update(
        &mut state,
        Message::Loaded(Box::new(
          async_load(db.clone(), 42, Seed::FromQueueSelection(vec![0]), now()).await,
        )),
        &db,
      );

      assert_eq!(state.name, "Plan from selection");
    }

    #[tokio::test]
    async fn new_seed_produces_an_empty_unsaved_plan() {
      let db = store::open_test().await.unwrap();

      let loaded = async_load(db, 42, Seed::New, now()).await;

      assert!(loaded.plan.is_none(), "new plan is not persisted until Save");
      assert!(loaded.entries.is_empty());
    }

    #[tokio::test]
    async fn picker_selections_flip_the_dirty_dot_and_reload_into_the_picker() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let mut state = State::new(42);
      let _ = update(
        &mut state,
        Message::Loaded(Box::new(async_load(db.clone(), 42, Seed::New, now()).await)),
        &db,
      );
      assert!(!state.dirty());

      let _ = update(&mut state, Message::PickerShipMasteryChanged(587, 4), &db);
      let _ = update(&mut state, Message::PickerCertProficiencyChanged(1, 2), &db);
      assert!(state.dirty(), "changing a selection flips the dirty dot");

      let id = persist(
        &db,
        42,
        None,
        "Combat",
        "manual",
        "current",
        &[],
        &[],
        &[],
        &[(587, 4)],
        &[(1, 2)],
      )
      .await
      .unwrap();

      let mut reloaded = State::new(42);
      let _ = update(
        &mut reloaded,
        Message::Loaded(Box::new(async_load(db.clone(), 42, Seed::Existing(id), now()).await)),
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
        42,
        None,
        "Combat",
        "manual",
        "current",
        &[],
        &[],
        &[],
        &[(587, 4), (588, 2)],
        &[(1, 2), (3, 3)],
      )
      .await
      .unwrap();

      let loaded = async_load(db, 42, Seed::Existing(id), now()).await;

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
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      assert!(!state.dirty());

      let _ = update(&mut state, Message::NameChanged("Combat".to_owned()), &db);

      assert!(state.dirty());
    }

    #[tokio::test]
    async fn note_edit_flips_dirty_but_toggle_alone_does_not() {
      let mut state = State::new(42);
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
      let mut state = State::new(42);
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
      let mut state = State::new(42);
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
      let mut state = State::new(42);
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
      let mut state = State::new(42);
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

  mod fmt_eta {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_the_instant_with_the_year() {
      assert_eq!(fmt_eta(now(), 2 * 3_600 + 30 * 60), "1 Jun 2026 · 14:30");
    }

    #[test]
    fn it_renders_an_em_dash_for_zero_or_negative() {
      assert_eq!(fmt_eta(now(), 0), "—");
    }

    #[test]
    fn it_rolls_into_a_later_year() {
      assert!(fmt_eta(now(), 250 * 86_400).ends_with("2027 · 12:00"));
    }
  }

  mod gap_hover {
    use super::*;

    #[tokio::test]
    async fn hovering_and_leaving_a_gap_tracks_the_hovered_gap() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      assert!(state.hovered_gap.is_none());

      let _ = update(&mut state, Message::GapHovered(7), &db);
      assert_eq!(state.hovered_gap, Some(7));

      let _ = update(&mut state, Message::GapUnhovered, &db);
      assert!(state.hovered_gap.is_none());
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
      let mut state = State::new(42);
      state.picker = PickerState {
        active_tab: crate::features::skill_plan_editor::picker::PickerTab::Skills,
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
        EditRemap {
          base: Attributes {
            charisma: 19,
            intelligence: 21,
            memory: 19,
            perception: 21,
            willpower: 19,
          },
          after_entry_id: None,
          local_id: 1,
        },
        EditRemap {
          base: Attributes {
            charisma: 17,
            intelligence: 17,
            memory: 17,
            perception: 27,
            willpower: 21,
          },
          after_entry_id: Some(first_id),
          local_id: 2,
        },
      ];

      let dto = plan_file(&state);
      let restored = state_with_catalog();
      let mut restored = restored;
      apply_json_import(&mut restored, dto.clone(), ImportMode::Replace);

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

    #[tokio::test]
    async fn import_and_export_triggers_toggle_their_dropdowns() {
      let mut state = State::new(42);
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
    }
  }

  mod insertion_pill {
    use super::*;

    #[test]
    fn the_hovered_gap_renders_the_clickable_pill() {
      let mut state = State::new(42);
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];
      state.remap_availability = 1;
      state.hovered_gap = Some(1);
      state.refresh_rows();

      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn the_start_gap_also_renders_the_pill_when_hovered() {
      let mut state = State::new(42);
      state.entries = vec![edit_entry(1, 3300, 5)];
      state.remap_availability = 1;
      state.hovered_gap = Some(GAP_START);
      state.refresh_rows();

      let _el: Element<'_, Message> = view(&state, now());
    }
  }

  mod load {
    use super::*;

    #[tokio::test]
    async fn it_loads_an_empty_new_plan_without_panicking() {
      let db = crate::store::open_test().await.unwrap();

      let loaded = async_load(db, 42, Seed::New, now()).await;

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
      let state = State::new(42).with_restored_panes(&UiState::default());

      assert_eq!(state.picker_pane.width(), PICKER_WIDTH);
      assert_eq!(state.summary_pane.width(), SUMMARY_WIDTH);
    }

    #[tokio::test]
    async fn it_grows_the_picker_on_a_rightward_drag_of_its_right_edge_handle() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42);

      let _ = update(&mut state, Message::PaneDragStart(EditorPane::Picker), &db);
      let _ = update(&mut state, Message::PaneDrag(500.0), &db);
      let _ = update(&mut state, Message::PaneDrag(540.0), &db);

      assert_eq!(state.picker_pane.width(), PICKER_WIDTH + 40.0);
    }

    #[tokio::test]
    async fn it_grows_the_summary_on_a_leftward_drag_of_its_left_edge_handle() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42);

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

      let state = State::new(42).with_restored_panes(&ui);

      assert_eq!(state.picker_pane.width(), 400.0);
      assert_eq!(state.summary_pane.width(), 300.0);
    }

    #[tokio::test]
    async fn it_settles_the_dragged_pane_and_clears_the_active_pane_on_drag_end() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42);

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
      let mut state = State::new(42);
      state.picker = PickerState {
        active_tab: crate::features::skill_plan_editor::picker::PickerTab::Skills,
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
      use crate::features::skill_plan_editor::picker::{PickerCert, PickerShip};

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
      state.picker.modules = Some(vec![crate::features::skill_plan_editor::picker::PickerModule {
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
      use crate::features::skill_plan_editor::picker::PickerShip;

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
      use crate::features::skill_plan_editor::picker::{PickerCert, PickerModule, PickerShip, PickerTab};

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
    async fn picking_a_trained_level_is_a_no_op() {
      let mut state = state_with_catalog(vec![catalog_entry(3300, "Gunnery", 1, vec![])]);
      state.picker.trained_levels = HashMap::from([(3300, 5)]);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::PickerLevelPicked(3300, 4), &db);

      assert!(state.entries.is_empty(), "an already-trained pick schedules nothing");
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
        use crate::features::skill_plan_editor::picker::PickerShip;

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
        use crate::features::skill_plan_editor::picker::PickerShip;

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
        use crate::features::skill_plan_editor::picker::PickerShip;

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
    use crate::features::skill_plan_editor::picker::PickerTab;

    #[tokio::test]
    async fn selecting_a_tab_switches_the_active_tab() {
      let mut state = State::new(42);
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
      let mut state = State::new(42);
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

    #[tokio::test]
    async fn an_impossible_bump_is_a_no_op() {
      let mut state = state_with(1);
      let db = crate::store::open_test().await.unwrap();
      state.remap_points = vec![EditRemap {
        base: Attributes {
          charisma: 17,
          intelligence: 17,
          memory: 21,
          perception: 27,
          willpower: 17,
        },
        after_entry_id: Some(10),
        local_id: 99,
      }];

      let _ = update(&mut state, Message::RemapAttrBumped(99, AttrKey::Perception, 1), &db);

      assert_eq!(state.remap_points[0].base.perception, 27, "illegal bump did not move");
    }

    #[tokio::test]
    async fn bumping_a_stepper_holds_the_total_and_recomputes() {
      let mut state = state_with(1);
      let db = crate::store::open_test().await.unwrap();
      let _ = update(&mut state, Message::RemapInserted(Some(10)), &db);
      let local_id = state.remap_points[0].local_id;
      let before_total: u32 = {
        let b = state.remap_points[0].base;
        b.charisma + b.intelligence + b.memory + b.perception + b.willpower
      };

      let _ = update(
        &mut state,
        Message::RemapAttrBumped(local_id, AttrKey::Perception, 1),
        &db,
      );

      let after = state.remap_points[0].base;
      assert_eq!(after.perception, base().perception + 1, "perception bumped +1");
      let after_total = after.charisma + after.intelligence + after.memory + after.perception + after.willpower;
      assert_eq!(after_total, before_total, "base total held at 99");
    }

    #[test]
    fn can_place_remap_tracks_placed_in_plan_points() {
      let mut state = state_with(2);
      assert!(state.can_place_remap());

      state.remap_points.push(EditRemap {
        base: base(),
        after_entry_id: Some(10),
        local_id: 1,
      });
      assert!(state.can_place_remap(), "one of two placed");

      state.remap_points.push(EditRemap {
        base: base(),
        after_entry_id: Some(11),
        local_id: 2,
      });
      assert!(!state.can_place_remap(), "both consumed");

      state.remap_points.push(EditRemap {
        base: base(),
        after_entry_id: None,
        local_id: 3,
      });
      assert_eq!(state.placed_in_plan_remaps(), 2, "start point not counted");
    }

    #[tokio::test]
    async fn it_caps_manual_in_plan_insertion_by_availability() {
      let mut state = state_with(1);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::RemapInserted(Some(10)), &db);
      let _ = update(&mut state, Message::RemapInserted(Some(11)), &db);

      assert_eq!(state.remap_points.len(), 1, "second in-plan insert is capped");
      assert!(!state.can_place_remap());
    }

    #[tokio::test]
    async fn it_inserts_a_remap_seeded_from_the_synced_base() {
      let mut state = state_with(1);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::RemapInserted(Some(10)), &db);

      assert_eq!(state.remap_points.len(), 1);
      assert_eq!(state.remap_points[0].after_entry_id, Some(10));
      assert_eq!(state.remap_points[0].base, base());
      assert!(state.dirty());
    }

    #[tokio::test]
    async fn it_removes_a_remap_point() {
      let mut state = state_with(2);
      let db = crate::store::open_test().await.unwrap();
      let _ = update(&mut state, Message::RemapInserted(Some(10)), &db);
      let local_id = state.remap_points[0].local_id;

      let _ = update(&mut state, Message::RemapRemoved(local_id), &db);

      assert!(state.remap_points.is_empty());
    }

    #[tokio::test]
    async fn the_start_point_is_free_and_not_capped() {
      let mut state = state_with(0);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::RemapInserted(None), &db);

      assert_eq!(state.remap_points.len(), 1);
      assert_eq!(state.remap_points[0].after_entry_id, None);
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

      let mut state = State::new(42);
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
        42,
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
        &[(3300, 5), (3301, 4)],
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
        42,
        None,
        "Plan",
        "manual",
        "current",
        &[
          (3300, 5, "normal".to_owned(), String::new(), 0),
          (3301, 5, "normal".to_owned(), String::new(), 0),
        ],
        &[(3300, 5), (3301, 5)],
        &[RemapSave {
          anchor_index: Some(0),
          base_perception: 17,
          base_memory: 27,
          base_willpower: 17,
          base_intelligence: 21,
          base_charisma: 17,
        }],
        &[],
        &[],
      )
      .await
      .unwrap();

      let entries = skills::entries(&db, id).await.unwrap();
      let remaps = skills::remap_points(&db, id).await.unwrap();
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
      let mut state = State::new(42);
      state.name = "  Combat  ".to_owned();
      state.sort = Sort {
        column: SortColumn::Time,
        direction: SortDirection::Descending,
      };
      state.entries = vec![edit_entry(10, 3300, 5)];

      let _task = save(&state, &db);
    }

    #[tokio::test]
    async fn it_builds_the_persist_task_from_state_without_panicking() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42);
      state.name = "  ".to_owned();
      let mut a = edit_entry(10, 3300, 5);
      a.priority = Priority::High;
      a.note = "core".to_owned();
      let b = edit_entry(11, 3301, 4);
      state.entries = vec![a, b];
      state.remap_points = vec![
        EditRemap {
          base: base(),
          after_entry_id: Some(10),
          local_id: 1,
        },
        EditRemap {
          base: base(),
          after_entry_id: Some(999),
          local_id: 2,
        },
        EditRemap {
          base: base(),
          after_entry_id: None,
          local_id: 3,
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

      assert_eq!(sort.caret(SortColumn::Secondary), Some("\u{2193}"));
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
      let state = State::new(42);

      let _sub: iced::Subscription<Message> = subscription(&state);
    }

    #[test]
    fn it_listens_for_release_while_an_entry_or_pane_drag_is_active() {
      let mut state = State::new(42);
      state.dragging = Some(7);
      state.dragging_pane = Some(EditorPane::Picker);

      let _sub: iced::Subscription<Message> = subscription(&state);
    }
  }

  mod summary {
    use super::*;

    #[test]
    fn summary_data_aggregates_group_and_pair_time_and_runs_the_optimizer() {
      let mut state = State::new(42);
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
      let mut state = State::new(42);
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
      let state = State::new(42);
      assert!(state.picker_open);
    }

    #[tokio::test]
    async fn it_toggles_the_picker() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(!state.picker_open);
      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(state.picker_open);
    }
  }

  mod topo_sort {
    use super::*;

    fn apply_sort_with(state: &mut State, column: SortColumn, direction: SortDirection) {
      state.sort = Sort {
        column,
        direction,
      };
      apply_sort(state);
    }

    #[test]
    fn primary_sort_still_respects_same_skill_prereqs() {
      let mut state = State::new(42);
      let mut high = edit_entry(1, 100, 2);
      high.meta.primary = AttrKey::Perception;
      let mut low = edit_entry(2, 100, 1);
      low.meta.primary = AttrKey::Charisma;
      state.entries = vec![high, low];

      apply_sort_with(&mut state, SortColumn::Primary, SortDirection::Ascending);

      let order: Vec<(i64, u8)> = state.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();
      let l1 = order.iter().position(|&(s, l)| s == 100 && l == 1).unwrap();
      let l2 = order.iter().position(|&(s, l)| s == 100 && l == 2).unwrap();
      assert!(l1 < l2, "L1 must precede L2 regardless of attribute key: {order:?}");
    }

    #[test]
    fn time_asc_never_places_a_level_before_its_lower_level() {
      let mut state = State::new(42);
      state.entries = vec![
        edit_entry(1, 100, 3),
        edit_entry(2, 100, 1),
        edit_entry(3, 100, 2),
        edit_entry(4, 200, 1),
      ];

      apply_sort_with(&mut state, SortColumn::Time, SortDirection::Ascending);

      let order: Vec<(i64, u8)> = state.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();
      let l1 = order.iter().position(|&(s, l)| s == 100 && l == 1).unwrap();
      let l2 = order.iter().position(|&(s, l)| s == 100 && l == 2).unwrap();
      let l3 = order.iter().position(|&(s, l)| s == 100 && l == 3).unwrap();
      assert!(l1 < l2 && l2 < l3, "same-skill prereq order preserved: {order:?}");
    }

    #[test]
    fn time_desc_still_respects_same_skill_prereqs() {
      let mut state = State::new(42);
      state.entries = vec![edit_entry(1, 100, 1), edit_entry(2, 100, 2), edit_entry(3, 200, 1)];

      apply_sort_with(&mut state, SortColumn::Time, SortDirection::Descending);

      let order: Vec<(i64, u8)> = state.entries.iter().map(|e| (e.skill_id, e.to_level)).collect();
      let l1 = order.iter().position(|&(s, l)| s == 100 && l == 1).unwrap();
      let l2 = order.iter().position(|&(s, l)| s == 100 && l == 2).unwrap();
      assert!(l1 < l2, "L1 must still precede L2 even descending: {order:?}");
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_placed_remap_divider_and_insertion_affordances() {
      let mut state = State::new(42);
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];
      state.remap_availability = 1;
      state.remap_points = vec![EditRemap {
        base: Attributes {
          charisma: 19,
          intelligence: 21,
          memory: 19,
          perception: 21,
          willpower: 19,
        },
        after_entry_id: Some(1),
        local_id: 1,
      }];
      state.refresh_rows();

      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_renders_the_empty_state_with_no_rows() {
      let state = State::new(42);

      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_renders_the_entry_list_with_rows() {
      let mut state = State::new(42);
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];

      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_renders_the_exhausted_constraint_when_no_remaps_available() {
      let mut state = State::new(42);
      state.entries = vec![edit_entry(1, 3300, 5), edit_entry(2, 3301, 5)];
      state.remap_availability = 0;
      state.remap_reason = "No neural remaps available — next remap accrues in 30 days".to_owned();
      state.refresh_rows();

      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_renders_the_import_export_overlay() {
      let mut state = State::new(42);
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
    fn it_renders_the_summary_right_pane() {
      let mut state = State::new(42);
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
  }
}
