mod auth;
mod card;
mod corp_card;
mod roster;
mod roster_tabs;
mod search_help;
mod squad_ui;
mod tag_ui;

use std::{
  collections::{HashMap, HashSet},
  ops::ControlFlow,
  time::Duration,
};

use card::{CardModel, TagChip, Training};
use chrono::{DateTime, Utc};
use corp_card::CorpCardModel;
use iced::{
  Color, Element, Length, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, operation, svg, text},
};

use crate::{
  config::FeatureFlags,
  features::{auth as auth_feature, registry},
  store::{
    Database, images,
    model::{
      Character, CharacterSkillqueue, CharacterSquad, CharacterState, Corporation, ENTITY_TYPE_CHARACTER,
      ENTITY_TYPE_CORPORATION, EntityTag, OwnerType, Squad, Tag, character_card, corporation_card,
    },
    repo::{character, infra, org, sde},
    search::parse,
  },
  sync::{JobKey, JobKind, Phase, Subject, SyncStatus},
  ui::{
    components::{
      color_picker, confirm_modal,
      context_menu::{self, Item},
      header,
      modal_overlay::modal_overlay,
    },
    format::{corp_ticker_label, skill_label},
    style::{color, control, spacing, typography},
  },
};

const DEFAULT_SQUAD_ACCENT: Color = color::accent::PLASMA;

const NO_MATCH_ICON: f32 = 28.0;

static SEARCH_ICON: &[u8] = include_bytes!("../../assets/images/icons/search.svg");

const SEARCH_DEBOUNCE_MS: u64 = 200;

const SEARCH_INPUT_ID: &str = "roster-search-input";

/// Stable scroll identity for the corporations grid, so its offset survives the same
/// tree-shape changes that flipping into a drag triggers. Mirrors the roster grids'
/// [`roster::ROSTER_SCROLL_ID`] / [`roster::FILTERED_SCROLL_ID`].
const CORP_SCROLL_ID: &str = "corporations-grid-scroll";

/// How often the drag auto-scroll re-evaluates the cursor's edge proximity and nudges the
/// grid. ~60 fps keeps the pull smooth without flooding the update loop.
const AUTO_SCROLL_INTERVAL: Duration = Duration::from_millis(16);

/// Thickness (pixels) of the hot zone at each viewport edge. The cursor must be within this
/// band of the top or bottom edge for the grid to auto-scroll.
const AUTO_SCROLL_EDGE_ZONE: f32 = 72.0;

/// Pixels-per-tick at the very edge of the viewport (proximity 1.0). Scroll speed ramps
/// linearly from [`AUTO_SCROLL_MIN_SPEED`] at the inner edge of the zone up to this.
const AUTO_SCROLL_MAX_SPEED: f32 = 18.0;

/// Pixels-per-tick at the inner edge of the hot zone (proximity ~0), so crossing the
/// threshold starts a gentle creep rather than a dead band that suddenly jumps.
const AUTO_SCROLL_MIN_SPEED: f32 = 2.0;

#[derive(Clone, Debug)]
pub struct AddTagModal {
  pub entity_id: i64,
  pub entity_type: &'static str,
  pub input: String,
}

#[derive(Clone, Debug)]
pub struct ContextMenu {
  pub anchor: iced::Point,
  pub character_id: i64,
  pub name: String,
  pub needs_fix: bool,
}

#[derive(Clone, Debug)]
pub struct CorpContextMenu {
  pub anchor: iced::Point,
  pub corporation_id: i64,
  pub name: String,
  pub needs_reauth: bool,
}

#[derive(Clone, Debug)]
pub enum CorpFiltered {
  Error(String),
  Loaded(Vec<CorpCardModel>),
  Loading,
}

#[derive(Clone, Debug)]
pub struct CorpRemoveConfirm {
  pub corporation_id: i64,
  pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Drag {
  Card(i64),
  Squad(i64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DropTarget {
  pub position: i64,
  pub squad_id: i64,
}

/// Snapshot of a roster grid's scroll geometry, captured from its scrollable viewport. Holds
/// everything the drag auto-scroll needs to detect edge proximity (the visible band `top..top +
/// height`) and clamp its nudges (`offset` in `0.0..=max_offset`), in absolute window pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GridViewport {
  pub height: f32,
  pub max_offset: f32,
  pub offset: f32,
  pub top: f32,
}

impl GridViewport {
  fn from_viewport(viewport: &iced::widget::scrollable::Viewport) -> Self {
    let bounds = viewport.bounds();
    let content = viewport.content_bounds();
    Self {
      height: bounds.height,
      max_offset: (content.height - bounds.height).max(0.0),
      offset: viewport.absolute_offset().y,
      top: bounds.y,
    }
  }

  #[cfg(test)]
  fn at_offset(offset: f32) -> Self {
    Self {
      offset,
      ..Self::default()
    }
  }
}

#[derive(Clone, Debug)]
pub enum Filtered {
  Error(String),
  Loaded(Vec<CardModel>),
  Loading,
}

#[derive(Clone, Debug)]
pub enum Message {
  AddCharacterRequested,
  AddCorporationRequested,
  /// Fires on a fixed interval while a card or squad is being dragged. Pulls off-screen rows
  /// into view by nudging the active grid's scroll offset toward whichever viewport edge the
  /// cursor is hovering near, so trackpad press-drags (which emit no further `on_move`) can
  /// still reach drop targets below or above the fold.
  AutoScrollTick,
  AddTagInputChanged(String),
  AssignTag {
    entity_id: i64,
    entity_type: &'static str,
    tag_id: i64,
  },
  AssignToSquad {
    character_id: i64,
    position: i64,
    squad_id: i64,
  },
  CancelDrag,
  CardRightPressed(i64),
  CharacterRemoved(Result<(), String>),
  CharacterSelected(i64),
  CharactersLoaded(Result<Roster, String>),
  ClearSearch,
  CloseAddTagModal,
  CloseContextMenu,
  CloseCorpContextMenu,
  CloseCorpRemoveConfirm,
  CloseRemoveConfirm,
  CloseSquadCreator,
  CloseSquadMenu,
  CopyCharacterName(String),
  CopyCorporationName(String),
  CorporationRemoved(Result<(), String>),
  CorporationSelected(i64),
  CorpRightPressed(i64),
  /// Persists the corporations grid's scroll geometry so its offset survives a drag's
  /// tree-shape change and the auto-scroll-during-drag tick can clamp to its bounds.
  CorpScrolled(GridViewport),
  CorpSearchResults {
    generation: u64,
    result: Result<Vec<CorpCardModel>, String>,
  },
  CreateAndAssignTag,
  CreateSquad,
  DeleteSquad(i64),
  DragMoved(iced::Point),
  DropDragged,
  /// Persists the filtered/search roster grid's scroll geometry so grabbing a card there
  /// does not snap the grid to the top and the auto-scroll tick can clamp to its bounds.
  FilteredScrolled(GridViewport),
  HoverSquadSlot(usize),
  HoverTarget(DropTarget),
  InsertQuery(String),
  LeaveSquadSlot(usize),
  LeaveTarget(DropTarget),
  OpenAddTagModal {
    entity_id: i64,
    entity_type: &'static str,
  },
  OpenCorpRemoveConfirm(i64),
  OpenRemoveConfirm(i64),
  OpenSquadCreator,
  OpenSquadEditor(i64),
  OpenSquadMenu(i64),
  PickUpCard(i64),
  PickUpSquad(i64),
  ReauthCharacterRequested(i64),
  ReauthCorporationRequested(i64),
  RemoveCharacterConfirmed(i64),
  RemoveCorporationConfirmed(i64),
  /// Persists the main (grouped) roster grid's scroll geometry so grabbing a card there
  /// does not snap the grid to the top and the auto-scroll tick can clamp to its bounds.
  RosterScrolled(GridViewport),
  SearchChanged(String),
  SearchResults {
    generation: u64,
    result: Result<Vec<CardModel>, String>,
  },
  SignedIn {
    character_id: i64,
    name: String,
  },
  SquadColorHexChanged(String),
  SquadColorHexSubmitted,
  SquadColorPickerToggled,
  SquadColorSelected(String),
  SquadCreatorDescriptionChanged(String),
  SquadCreatorNameChanged(String),
  SquadsChanged(Result<(), String>),
  TabSelected(Pane),
  TagsChanged(Result<(), String>),
  ToggleSearchHelp,
  ToggleSquadCollapse(i64),
  TrainingSkillClicked(i64),
  UnassignTag {
    entity_id: i64,
    entity_type: &'static str,
    tag_id: i64,
  },
  UngroupSquad(i64),
}

impl Message {
  /// Whether handling this message can surface new image-bearing cards (portraits/logos), so the shell should
  /// recheck for stale images. Interaction-only messages (drag, search edits, menu toggles) return `false` to keep
  /// the staleness scan off the per-frame path.
  pub fn loads_data(&self) -> bool {
    matches!(
      self,
      Message::CharactersLoaded(_)
        | Message::CorpSearchResults { .. }
        | Message::SearchResults { .. }
        | Message::SignedIn { .. }
    )
  }
}

#[derive(Clone, Debug)]
pub struct OwnedPilot {
  pub color: Color,
  pub granted_scopes: Option<String>,
  pub id: i64,
  pub name: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Pane {
  #[default]
  Characters,
  Corporations,
}

impl Pane {
  pub fn from_id(id: &str) -> Option<Pane> {
    match id {
      "characters" => Some(Pane::Characters),
      "corporations" => Some(Pane::Corporations),
      _ => None,
    }
  }

  pub fn id(self) -> &'static str {
    match self {
      Pane::Characters => "characters",
      Pane::Corporations => "corporations",
    }
  }
}

#[derive(Clone, Debug)]
pub struct RemoveConfirm {
  pub character_id: i64,
  pub name: String,
}

pub type Roster = (
  Vec<SquadGroup>,
  Vec<CardModel>,
  i64,
  Vec<Tag>,
  Vec<CorpCardModel>,
  FeatureFlags,
  HashMap<i64, Option<String>>,
);

#[derive(Clone, Debug)]
pub struct SquadCreator {
  pub color: String,
  pub color_popover_open: bool,
  pub description: String,
  pub editing: Option<i64>,
  pub hex_draft: String,
  pub hex_invalid: bool,
  pub name: String,
}

impl SquadCreator {
  fn editing(squad_id: i64, name: String, description: Option<String>, color: Option<String>) -> Self {
    let color = color.unwrap_or_else(default_squad_hex);
    Self {
      color: color.clone(),
      color_popover_open: false,
      description: description.unwrap_or_default(),
      editing: Some(squad_id),
      hex_draft: color,
      hex_invalid: false,
      name,
    }
  }
}

impl Default for SquadCreator {
  fn default() -> Self {
    Self {
      color: default_squad_hex(),
      color_popover_open: false,
      description: String::new(),
      editing: None,
      hex_draft: default_squad_hex(),
      hex_invalid: false,
      name: String::new(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct SquadGroup {
  pub accent: Color,
  pub cards: Vec<CardModel>,
  pub color_hex: Option<String>,
  pub description: Option<String>,
  pub name: String,
  pub squad_id: i64,
}

#[derive(Clone, Debug)]
pub struct SquadMenu {
  pub anchor: iced::Point,
  pub collapsed: bool,
  pub is_empty: bool,
  pub name: String,
  pub squad_id: i64,
}

#[derive(Debug, Default)]
pub struct State {
  active_pane: Pane,
  add_tag_modal: Option<AddTagModal>,
  all_tags: Vec<Tag>,
  collapsed_squads: HashSet<i64>,
  context_menu: Option<ContextMenu>,
  corp_context_menu: Option<CorpContextMenu>,
  corp_filtered: Option<CorpFiltered>,
  corp_remove_confirm: Option<CorpRemoveConfirm>,
  corp_scroll_offset: f32,
  corps: Vec<CorpCardModel>,
  cursor: Option<iced::Point>,
  dragging: Option<Drag>,
  drop_target: Option<DropTarget>,
  features: FeatureFlags,
  filtered: Option<Filtered>,
  filtered_scroll_offset: f32,
  granted_scopes_by_id: HashMap<i64, Option<String>>,
  groups: Vec<SquadGroup>,
  load_error: Option<String>,
  pending: HashMap<i64, CardModel>,
  reauth_by_id: HashMap<i64, bool>,
  remove_confirm: Option<RemoveConfirm>,
  // Geometry of whichever grid last reported a scroll, used by the drag auto-scroll to detect
  // edge proximity and clamp. Drags only happen in the Characters pane, so this tracks the main
  // or filtered grid; the corporations grid updates it too but never enters a drag.
  roster_viewport: GridViewport,
  roster_scroll_offset: f32,
  search_generation: u64,
  search_help_open: bool,
  search_query: String,
  squad_creator: Option<SquadCreator>,
  squad_drop_target: Option<usize>,
  squad_menu: Option<SquadMenu>,
  unassigned: Vec<CardModel>,
  unassigned_squad_id: i64,
}

impl State {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn active_pane(&self) -> Pane {
    self.active_pane
  }

  pub fn select_pane_by_id(&mut self, id: &str) -> bool {
    match Pane::from_id(id) {
      Some(pane) => {
        self.active_pane = pane;
        true
      }
      None => false,
    }
  }

  pub fn corp_filtered(&self) -> Option<&CorpFiltered> {
    self.corp_filtered.as_ref()
  }

  // The character-detail view is only worth opening when at least one of its tab-backed features is
  // enabled; with all of them off it would render nothing, so the card name is left non-clickable.
  pub(super) fn detail_navigable(&self) -> bool {
    self
      .features
      .enabled()
      .iter()
      .any(|&feature| registry::descriptor(feature).tab.is_some())
  }

  pub fn filtered(&self) -> Option<&Filtered> {
    self.filtered.as_ref()
  }

  pub(super) fn location_card_enabled(&self) -> bool {
    self
      .features
      .is_sub_enabled(crate::config::SubFeature::LocationTracking)
  }

  pub(super) fn training_card_enabled(&self) -> bool {
    self.features.is_sub_enabled(crate::config::SubFeature::SkillQueue)
  }

  pub fn is_corp_filtered(&self) -> bool {
    self.corp_filtered.is_some()
  }

  pub fn is_filtered(&self) -> bool {
    self.filtered.is_some()
  }

  pub fn search_help_open(&self) -> bool {
    self.search_help_open
  }

  pub fn search_query(&self) -> &str {
    &self.search_query
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    let group_cards = self.groups.iter().flat_map(|group| group.cards.iter());
    let filtered_cards = match &self.filtered {
      Some(Filtered::Loaded(cards)) => cards.as_slice(),
      _ => &[],
    };
    let card_keys = group_cards
      .chain(self.unassigned.iter())
      .chain(filtered_cards.iter())
      .filter_map(|card| card.portrait.stale_key());

    let filtered_corps = match &self.corp_filtered {
      Some(CorpFiltered::Loaded(corps)) => corps.as_slice(),
      _ => &[],
    };
    let corp_keys = self
      .corps
      .iter()
      .chain(filtered_corps.iter())
      .filter_map(|corp| corp.logo.stale_key());

    card_keys.chain(corp_keys).collect()
  }
}

struct CardInputs<'a> {
  corp: Option<&'a Corporation>,
  granted_scopes: Option<&'a str>,
  persisted_reauth: bool,
  position: i64,
  required_scopes: &'a [&'a str],
  squad_accent: Option<Color>,
  state: Option<&'a CharacterState>,
  store: &'a images::Store,
  tags: Vec<TagChip>,
}

enum SquadWrite {
  Assign {
    character_id: i64,
    position: i64,
    squad_id: i64,
  },
  Create {
    color: Option<String>,
    description: Option<String>,
    name: String,
  },
  Delete {
    squad_id: i64,
  },
  Reorder {
    ordered: Vec<i64>,
  },
  Ungroup {
    squad_id: i64,
  },
  Update {
    color: Option<String>,
    description: Option<String>,
    name: String,
    squad_id: i64,
  },
}

enum TagWrite {
  Assign {
    entity_id: i64,
    entity_type: &'static str,
    tag_id: i64,
  },
  CreateAndAssign {
    entity_id: i64,
    entity_type: &'static str,
    name: String,
  },
  Unassign {
    entity_id: i64,
    entity_type: &'static str,
    tag_id: i64,
  },
}

fn default_squad_hex() -> String {
  color_to_hex(DEFAULT_SQUAD_ACCENT)
}

fn color_to_hex(color: Color) -> String {
  let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
  format!(
    "#{:02X}{:02X}{:02X}",
    channel(color.r),
    channel(color.g),
    channel(color.b)
  )
}

pub fn load(db: &Database, features: FeatureFlags) -> Task<Message> {
  Task::perform(load_roster(db.clone(), features), Message::CharactersLoaded)
}

/// Shows a placeholder card built from just the character id and JWT name, with no `characters` row required yet.
fn insert_signed_in_card(state: &mut State, character_id: i64, name: String) {
  let card = synthesize_pending_card(character_id, name, &images::default_store());
  state.pending.insert(character_id, card.clone());
  if !roster_contains(state, character_id) {
    append_unassigned(state, card);
  }
}

fn append_unassigned(state: &mut State, mut card: CardModel) {
  card.position = next_append_slot(&state.unassigned);
  state.unassigned.push(card);
}

/// Re-appends placeholder cards after each load, retiring one only once its real row appears, so a freshly added card
/// does not vanish while its first sync is still in flight.
fn merge_pending(state: &mut State) {
  let loaded: HashSet<i64> = state
    .groups
    .iter()
    .flat_map(|group| group.cards.iter())
    .chain(state.unassigned.iter())
    .map(|card| card.character_id)
    .collect();
  state.pending.retain(|id, _| !loaded.contains(id));
  for card in state.pending.values().cloned().collect::<Vec<_>>() {
    append_unassigned(state, card);
  }
}

fn next_append_slot(cards: &[CardModel]) -> i64 {
  cards
    .iter()
    .map(|card| card.position)
    .max()
    .map_or(0, |max| max.saturating_add(1))
}

fn roster_contains(state: &State, character_id: i64) -> bool {
  state
    .groups
    .iter()
    .flat_map(|group| group.cards.iter())
    .chain(state.unassigned.iter())
    .any(|card| card.character_id == character_id)
}

fn synthesize_pending_card(character_id: i64, name: String, store: &images::Store) -> CardModel {
  CardModel {
    accent: None,
    character_id,
    corp_ticker: "\u{2014}".to_owned(),
    docked: None,
    location: None,
    name,
    needs_reauth: false,
    portrait: images::resolve(store, images::ImageKind::CharacterPortrait, character_id),
    position: 0,
    tags: Vec::new(),
    total_sp: None,
    training: None,
    wallet_balance: None,
  }
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  let message = match update_drag(state, message, db) {
    ControlFlow::Break(task) => return task,
    ControlFlow::Continue(message) => message,
  };
  let message = match update_squad(state, message, db) {
    ControlFlow::Break(task) => return task,
    ControlFlow::Continue(message) => message,
  };
  let message = match update_squad_creator(state, message) {
    ControlFlow::Break(task) => return task,
    ControlFlow::Continue(message) => message,
  };
  let message = match update_tags(state, message, db) {
    ControlFlow::Break(task) => return task,
    ControlFlow::Continue(message) => message,
  };
  let message = match update_menus(state, message) {
    ControlFlow::Break(task) => return task,
    ControlFlow::Continue(message) => message,
  };
  let message = match update_search(state, message, db) {
    ControlFlow::Break(task) => return task,
    ControlFlow::Continue(message) => message,
  };
  update_lifecycle(state, message, db)
}

fn update_drag(state: &mut State, message: Message, db: &Database) -> ControlFlow<Task<Message>, Message> {
  let task = match message {
    Message::AssignToSquad {
      character_id,
      position,
      squad_id,
    } => {
      end_drag(state);
      write_squad(
        db.clone(),
        SquadWrite::Assign {
          character_id,
          position,
          squad_id,
        },
      )
    }
    Message::CancelDrag => {
      end_drag(state);
      Task::none()
    }
    Message::DragMoved(point) => {
      state.cursor = Some(point);
      Task::none()
    }
    Message::DropDragged => {
      let drop = match (state.dragging, state.drop_target) {
        (
          Some(Drag::Card(character_id)),
          Some(DropTarget {
            position,
            squad_id,
          }),
        ) => Message::AssignToSquad {
          character_id,
          position,
          squad_id,
        },
        (Some(Drag::Squad(squad_id)), _) => {
          if let Some(index) = state.squad_drop_target {
            return ControlFlow::Break(reorder_squad(state, squad_id, index, db));
          }
          Message::CancelDrag
        }
        _ => Message::CancelDrag,
      };
      update(state, drop, db)
    }
    Message::HoverSquadSlot(index) => {
      if matches!(state.dragging, Some(Drag::Squad(_))) {
        state.squad_drop_target = Some(index);
      }
      Task::none()
    }
    Message::HoverTarget(target) => {
      if matches!(state.dragging, Some(Drag::Card(_))) {
        state.drop_target = Some(target);
      }
      Task::none()
    }
    Message::LeaveSquadSlot(index) => {
      if state.squad_drop_target == Some(index) {
        state.squad_drop_target = None;
      }
      Task::none()
    }
    Message::LeaveTarget(target) => {
      if state.drop_target == Some(target) {
        state.drop_target = None;
      }
      Task::none()
    }
    Message::AutoScrollTick => auto_scroll_active_grid(state),
    Message::CorpScrolled(viewport) => {
      state.corp_scroll_offset = viewport.offset;
      state.roster_viewport = viewport;
      Task::none()
    }
    Message::FilteredScrolled(viewport) => {
      state.filtered_scroll_offset = viewport.offset;
      state.roster_viewport = viewport;
      Task::none()
    }
    Message::RosterScrolled(viewport) => {
      state.roster_scroll_offset = viewport.offset;
      state.roster_viewport = viewport;
      Task::none()
    }
    Message::PickUpCard(character_id) => {
      state.dragging = Some(Drag::Card(character_id));
      state.drop_target = None;
      state.squad_drop_target = None;
      // Grabbing re-wraps the scrollable in a Stack for the ghost overlay; re-apply the
      // persisted offset so the grid holds its place instead of snapping to the top.
      restore_active_grid_scroll(state)
    }
    Message::PickUpSquad(squad_id) => {
      if state.groups.iter().any(|group| group.squad_id == squad_id) {
        state.dragging = Some(Drag::Squad(squad_id));
        state.drop_target = None;
        state.squad_drop_target = None;
        // A squad drag reshapes the same grid; hold its scroll position too.
        return ControlFlow::Break(restore_active_grid_scroll(state));
      }
      Task::none()
    }
    other => return ControlFlow::Continue(other),
  };
  ControlFlow::Break(task)
}

fn update_squad(state: &mut State, message: Message, db: &Database) -> ControlFlow<Task<Message>, Message> {
  let task = match message {
    Message::CreateSquad => {
      let Some((editing, name, description, color)) = state.squad_creator.as_ref().and_then(|creator| {
        let name = creator.name.trim();
        (!name.is_empty()).then(|| {
          (
            creator.editing,
            name.to_owned(),
            non_blank(&creator.description),
            non_blank(&creator.color),
          )
        })
      }) else {
        return ControlFlow::Break(Task::none());
      };
      state.squad_creator = None;
      let write = match editing {
        Some(squad_id) => SquadWrite::Update {
          color,
          description,
          name,
          squad_id,
        },
        None => SquadWrite::Create {
          color,
          description,
          name,
        },
      };
      write_squad(db.clone(), write)
    }
    Message::DeleteSquad(squad_id) => {
      state.squad_menu = None;
      write_squad(
        db.clone(),
        SquadWrite::Delete {
          squad_id,
        },
      )
    }
    Message::OpenSquadEditor(squad_id) => {
      state.squad_menu = None;
      let seed = state
        .groups
        .iter()
        .find(|group| group.squad_id == squad_id)
        .map(|group| (group.name.clone(), group.description.clone(), group.color_hex.clone()));
      if let Some((name, description, color)) = seed {
        state.squad_creator = Some(SquadCreator::editing(squad_id, name, description, color));
      }
      Task::none()
    }
    Message::OpenSquadMenu(squad_id) => {
      let menu = state.cursor.and_then(|anchor| {
        state
          .groups
          .iter()
          .find(|group| group.squad_id == squad_id)
          .map(|group| SquadMenu {
            anchor,
            collapsed: state.collapsed_squads.contains(&squad_id),
            is_empty: group.cards.is_empty(),
            name: group.name.clone(),
            squad_id,
          })
      });
      if menu.is_some() {
        state.squad_menu = menu;
      }
      Task::none()
    }
    Message::ToggleSquadCollapse(squad_id) => {
      state.squad_menu = None;
      if !state.collapsed_squads.remove(&squad_id) {
        state.collapsed_squads.insert(squad_id);
      }
      Task::none()
    }
    Message::UngroupSquad(squad_id) => {
      state.squad_menu = None;
      write_squad(
        db.clone(),
        SquadWrite::Ungroup {
          squad_id,
        },
      )
    }
    other => return ControlFlow::Continue(other),
  };
  ControlFlow::Break(task)
}

fn update_squad_creator(state: &mut State, message: Message) -> ControlFlow<Task<Message>, Message> {
  match message {
    Message::CloseSquadCreator => state.squad_creator = None,
    Message::OpenSquadCreator => state.squad_creator = Some(SquadCreator::default()),
    Message::SquadColorHexChanged(draft) => {
      if let Some(creator) = state.squad_creator.as_mut() {
        creator.hex_draft = draft;
        creator.hex_invalid = false;
      }
    }
    Message::SquadColorHexSubmitted => {
      if let Some(creator) = state.squad_creator.as_mut() {
        match color_picker::normalize_hex(&creator.hex_draft) {
          Some(hex) => {
            creator.color = hex.clone();
            creator.hex_draft = hex;
            creator.hex_invalid = false;
          }
          None => creator.hex_invalid = !creator.hex_draft.trim().is_empty(),
        }
      }
    }
    Message::SquadColorPickerToggled => {
      if let Some(creator) = state.squad_creator.as_mut() {
        creator.color_popover_open = !creator.color_popover_open;
        if creator.color_popover_open {
          creator.hex_draft = creator.color.clone();
          creator.hex_invalid = false;
        }
      }
    }
    Message::SquadColorSelected(hex) => {
      if let Some(creator) = state.squad_creator.as_mut() {
        creator.color = hex.clone();
        creator.hex_draft = hex;
        creator.hex_invalid = false;
        creator.color_popover_open = false;
      }
    }
    Message::SquadCreatorDescriptionChanged(description) => {
      if let Some(creator) = state.squad_creator.as_mut() {
        creator.description = description;
      }
    }
    Message::SquadCreatorNameChanged(name) => {
      if let Some(creator) = state.squad_creator.as_mut() {
        creator.name = name;
      }
    }
    other => return ControlFlow::Continue(other),
  }
  ControlFlow::Break(Task::none())
}

fn update_tags(state: &mut State, message: Message, db: &Database) -> ControlFlow<Task<Message>, Message> {
  let task = match message {
    Message::AddTagInputChanged(input) => {
      if let Some(modal) = state.add_tag_modal.as_mut() {
        modal.input = input;
      }
      Task::none()
    }
    Message::AssignTag {
      entity_id,
      entity_type,
      tag_id,
    } => {
      state.add_tag_modal = None;
      write_tag(
        db.clone(),
        TagWrite::Assign {
          entity_id,
          entity_type,
          tag_id,
        },
      )
    }
    Message::CloseAddTagModal => {
      state.add_tag_modal = None;
      Task::none()
    }
    Message::CreateAndAssignTag => {
      let Some((entity_type, entity_id, name)) = state.add_tag_modal.as_ref().and_then(|modal| {
        let name = modal.input.trim();
        (!name.is_empty()).then(|| (modal.entity_type, modal.entity_id, name.to_owned()))
      }) else {
        return ControlFlow::Break(Task::none());
      };
      state.add_tag_modal = None;
      write_tag(
        db.clone(),
        TagWrite::CreateAndAssign {
          entity_id,
          entity_type,
          name,
        },
      )
    }
    Message::OpenAddTagModal {
      entity_id,
      entity_type,
    } => {
      state.context_menu = None;
      state.corp_context_menu = None;
      state.add_tag_modal = Some(AddTagModal {
        entity_id,
        entity_type,
        input: String::new(),
      });
      Task::none()
    }
    Message::UnassignTag {
      entity_id,
      entity_type,
      tag_id,
    } => write_tag(
      db.clone(),
      TagWrite::Unassign {
        entity_id,
        entity_type,
        tag_id,
      },
    ),
    other => return ControlFlow::Continue(other),
  };
  ControlFlow::Break(task)
}

fn update_menus(state: &mut State, message: Message) -> ControlFlow<Task<Message>, Message> {
  let task = match message {
    Message::CardRightPressed(character_id) => {
      if let (Some(anchor), Some(card)) = (state.cursor, card_for(state, character_id)) {
        let name = card.name.clone();
        state.context_menu = Some(ContextMenu {
          anchor,
          character_id,
          name,
          needs_fix: state.reauth_by_id.get(&character_id).copied().unwrap_or(false),
        });
      }
      Task::none()
    }
    Message::CloseContextMenu => {
      state.context_menu = None;
      Task::none()
    }
    Message::CloseCorpContextMenu => {
      state.corp_context_menu = None;
      Task::none()
    }
    Message::CloseCorpRemoveConfirm => {
      state.corp_remove_confirm = None;
      Task::none()
    }
    Message::CloseRemoveConfirm => {
      state.remove_confirm = None;
      Task::none()
    }
    Message::CloseSquadMenu => {
      state.squad_menu = None;
      Task::none()
    }
    Message::CopyCharacterName(name) => {
      state.context_menu = None;
      iced::clipboard::write(name)
    }
    Message::CopyCorporationName(name) => {
      state.corp_context_menu = None;
      iced::clipboard::write(name)
    }
    Message::CorpRightPressed(corporation_id) => {
      if let (Some(anchor), Some(corp)) = (state.cursor, corp_for(state, corporation_id)) {
        state.corp_context_menu = Some(CorpContextMenu {
          anchor,
          corporation_id,
          name: corp.name.clone(),
          needs_reauth: corp.needs_reauth,
        });
      }
      Task::none()
    }
    Message::OpenCorpRemoveConfirm(corporation_id) => {
      state.corp_context_menu = None;
      if let Some(corp) = corp_for(state, corporation_id) {
        state.corp_remove_confirm = Some(CorpRemoveConfirm {
          corporation_id,
          name: corp.name.clone(),
        });
      }
      Task::none()
    }
    Message::OpenRemoveConfirm(character_id) => {
      state.context_menu = None;
      if let Some(card) = card_for(state, character_id) {
        state.remove_confirm = Some(RemoveConfirm {
          character_id,
          name: card.name.clone(),
        });
      }
      Task::none()
    }
    other => return ControlFlow::Continue(other),
  };
  ControlFlow::Break(task)
}

fn update_search(state: &mut State, message: Message, db: &Database) -> ControlFlow<Task<Message>, Message> {
  let task = match message {
    Message::ClearSearch => {
      state.search_query.clear();
      state.search_generation = state.search_generation.wrapping_add(1);
      state.filtered = None;
      state.corp_filtered = None;
      operation::focus(SEARCH_INPUT_ID)
    }
    Message::CorpSearchResults {
      generation,
      result,
    } => {
      if generation == state.search_generation {
        state.corp_filtered = Some(match result {
          Ok(cards) => CorpFiltered::Loaded(cards),
          Err(error) => CorpFiltered::Error(error),
        });
      }
      Task::none()
    }
    Message::InsertQuery(fragment) => {
      append_query(state, &fragment);
      Task::batch([trigger_search(state, db), operation::focus(SEARCH_INPUT_ID)])
    }
    Message::SearchChanged(query) => {
      state.search_query = query;
      trigger_search(state, db)
    }
    Message::SearchResults {
      generation,
      result,
    } => {
      if generation == state.search_generation {
        state.filtered = Some(match result {
          Ok(mut cards) => {
            for card in &mut cards {
              card.needs_reauth = state.reauth_by_id.get(&card.character_id).copied().unwrap_or(false);
            }
            Filtered::Loaded(cards)
          }
          Err(error) => Filtered::Error(error),
        });
      }
      Task::none()
    }
    Message::TabSelected(pane) => {
      state.active_pane = pane;
      trigger_search(state, db)
    }
    Message::ToggleSearchHelp => {
      state.search_help_open = !state.search_help_open;
      Task::none()
    }
    other => return ControlFlow::Continue(other),
  };
  ControlFlow::Break(task)
}

fn update_lifecycle(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::AddCharacterRequested
    | Message::AddCorporationRequested
    | Message::ReauthCharacterRequested(_)
    | Message::ReauthCorporationRequested(_) => Task::none(),
    Message::CharacterRemoved(Ok(()))
    | Message::CorporationRemoved(Ok(()))
    | Message::SquadsChanged(Ok(()))
    | Message::TagsChanged(Ok(())) => load(db, state.features),
    Message::CharacterRemoved(Err(error))
    | Message::CorporationRemoved(Err(error))
    | Message::SquadsChanged(Err(error))
    | Message::TagsChanged(Err(error)) => {
      state.load_error = Some(error);
      Task::none()
    }
    Message::CharacterSelected(id) => {
      tracing::info!(character_id = id, "character selected; detail view not yet implemented");
      Task::none()
    }
    Message::CorporationSelected(id) => {
      tracing::info!(corporation_id = id, "corporation selected; handled by the app router");
      Task::none()
    }
    Message::CharactersLoaded(Ok((
      groups,
      unassigned,
      unassigned_squad_id,
      all_tags,
      corps,
      features,
      granted_scopes_by_id,
    ))) => {
      state.reauth_by_id = groups
        .iter()
        .flat_map(|group| group.cards.iter())
        .chain(unassigned.iter())
        .map(|card| (card.character_id, card.needs_reauth))
        .collect();
      state.all_tags = all_tags;
      state.corps = corps;
      state.features = features;
      state.granted_scopes_by_id = granted_scopes_by_id;
      state.groups = groups;
      state.unassigned = unassigned;
      state.unassigned_squad_id = unassigned_squad_id;
      state.load_error = None;
      merge_pending(state);
      Task::none()
    }
    Message::CharactersLoaded(Err(error)) => {
      // A failed refresh must never blank a roster the user is already looking at: retries with
      // bounded backoff (see `load_roster`) absorb transient pool timeouts, and if one slips
      // through here we keep the last-good groups/unassigned/corps on screen rather than replacing
      // them with "Couldn't load characters." The error surfaces only on a cold load, when there is
      // no prior data to retain.
      if has_roster_data(state) {
        tracing::warn!(error, "roster refresh failed; retaining last-good roster");
      } else {
        state.load_error = Some(error);
      }
      Task::none()
    }
    Message::RemoveCharacterConfirmed(character_id) => {
      state.remove_confirm = None;
      Task::perform(remove_character(db.clone(), character_id), Message::CharacterRemoved)
    }
    Message::RemoveCorporationConfirmed(corporation_id) => {
      state.corp_remove_confirm = None;
      Task::perform(
        remove_corporation(db.clone(), corporation_id),
        Message::CorporationRemoved,
      )
    }
    Message::SignedIn {
      character_id,
      name,
    } => {
      insert_signed_in_card(state, character_id, name);
      Task::none()
    }
    _ => Task::none(),
  }
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  if state.dragging.is_none() {
    return iced::Subscription::none();
  }
  let drop = iced::event::listen_with(|event, _status, _id| {
    matches!(
      event,
      iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
    )
    .then_some(Message::DropDragged)
  });
  // A fixed tick (not on_move) drives the edge auto-scroll so a trackpad press-drag held
  // stationary in an edge zone keeps scrolling without further cursor events.
  let auto_scroll = iced::time::every(AUTO_SCROLL_INTERVAL).map(|_| Message::AutoScrollTick);
  iced::Subscription::batch([drop, auto_scroll])
}

pub fn view<'a>(state: &'a State, sync: &SyncStatus) -> Element<'a, Message> {
  let pane = state.active_pane;

  let toolbar = header::header(
    vec![roster_tabs::roster_tabs(vec![
      roster_tabs::Tab {
        count: roster_count(state),
        label: "Characters",
        on_press: (pane != Pane::Characters).then_some(Message::TabSelected(Pane::Characters)),
        selected: pane == Pane::Characters,
      },
      roster_tabs::Tab {
        count: corp_count(state),
        label: "Corporations",
        on_press: (pane != Pane::Corporations).then_some(Message::TabSelected(Pane::Corporations)),
        selected: pane == Pane::Corporations,
      },
    ])],
    pane_actions(state, pane),
  );

  let mut sections: Vec<Element<'a, Message>> = vec![toolbar, search_help::search_bar(state)];
  match pane {
    Pane::Characters => sections.push(roster::body(state, sync)),
    Pane::Corporations => sections.push(corporations_body(state, sync)),
  }

  let base: Element<'a, Message> = Column::with_children(sections)
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

  match active_overlay(state) {
    Some((backdrop_msg, content)) => modal_overlay(base, Some(backdrop_msg), content),
    None => base,
  }
}

fn active_overlay(state: &State) -> Option<(Message, Element<'_, Message>)> {
  if let Some(modal) = state.add_tag_modal.as_ref() {
    let (name, assigned, assignable) = resolve_add_tag_modal(state, modal.entity_type, modal.entity_id);
    return Some((
      Message::CloseAddTagModal,
      tag_ui::modal_view(modal, name, assigned, assignable),
    ));
  }

  if let Some(confirm) = state.remove_confirm.as_ref() {
    return Some((
      Message::CloseRemoveConfirm,
      confirm_modal::confirm_modal(
        "Remove character",
        format!("Remove {} from Pod?", confirm.name),
        "This unlinks the character from this app only. Their skills, assets and ISK on the EVE servers \
are unaffected. You can re-add them later via Add character.",
        "Remove",
        Message::RemoveCharacterConfirmed(confirm.character_id),
        Message::CloseRemoveConfirm,
      ),
    ));
  }

  if let Some(confirm) = state.corp_remove_confirm.as_ref() {
    return Some((
      Message::CloseCorpRemoveConfirm,
      confirm_modal::confirm_modal(
        "Remove corporation",
        format!("Remove {} from Pod?", confirm.name),
        "This unlinks the corporation from this app only. Its members, assets and structures on the EVE \
servers are unaffected. You can re-add it later via Add corporation.",
        "Remove",
        Message::RemoveCorporationConfirmed(confirm.corporation_id),
        Message::CloseCorpRemoveConfirm,
      ),
    ));
  }

  if let Some(menu) = state.corp_context_menu.as_ref() {
    return Some((Message::CloseCorpContextMenu, corp_context_menu_view(menu)));
  }

  if let Some(menu) = state.context_menu.as_ref() {
    return Some((Message::CloseContextMenu, context_menu_view(menu)));
  }

  if let Some(menu) = state.squad_menu.as_ref() {
    return Some((Message::CloseSquadMenu, squad_menu_view(menu)));
  }

  if state.search_help_open() {
    return Some((Message::ToggleSearchHelp, search_help::popover(all_tags(state))));
  }

  state
    .squad_creator
    .as_ref()
    .map(|creator| (Message::CloseSquadCreator, squad_ui::modal_view(creator)))
}

fn context_menu_view(menu: &ContextMenu) -> Element<'_, Message> {
  let mut items = Vec::new();
  if menu.needs_fix {
    items.push(Item::danger(
      "Fix Permissions",
      Message::ReauthCharacterRequested(menu.character_id),
    ));
    items.push(Item::separator());
  }
  items.push(Item::action("Copy name", Message::CopyCharacterName(menu.name.clone())));
  items.push(Item::action(
    "Edit tags",
    Message::OpenAddTagModal {
      entity_id: menu.character_id,
      entity_type: ENTITY_TYPE_CHARACTER,
    },
  ));
  items.push(Item::separator());
  items.push(Item::danger(
    "Remove from app",
    Message::OpenRemoveConfirm(menu.character_id),
  ));
  context_menu::context_menu(&menu.name, items, menu.anchor)
}

fn corp_context_menu_view(menu: &CorpContextMenu) -> Element<'_, Message> {
  let mut items = Vec::new();
  if menu.needs_reauth {
    items.push(Item::danger(
      "Re-authorize",
      Message::ReauthCorporationRequested(menu.corporation_id),
    ));
    items.push(Item::separator());
  }
  items.push(Item::action(
    "Copy name",
    Message::CopyCorporationName(menu.name.clone()),
  ));
  items.push(Item::action(
    "Edit tags",
    Message::OpenAddTagModal {
      entity_id: menu.corporation_id,
      entity_type: ENTITY_TYPE_CORPORATION,
    },
  ));
  items.push(Item::separator());
  items.push(Item::danger(
    "Remove from app",
    Message::OpenCorpRemoveConfirm(menu.corporation_id),
  ));
  context_menu::context_menu(&menu.name, items, menu.anchor)
}

fn squad_menu_view(menu: &SquadMenu) -> Element<'_, Message> {
  let collapse = if menu.collapsed { "Expand" } else { "Collapse" };
  let move_pilots = if menu.is_empty {
    Item::disabled("Move pilots to Unassigned")
  } else {
    Item::action("Move pilots to Unassigned", Message::UngroupSquad(menu.squad_id))
  };
  let items = vec![
    Item::action("Edit squad", Message::OpenSquadEditor(menu.squad_id)),
    Item::action(collapse, Message::ToggleSquadCollapse(menu.squad_id)),
    Item::separator(),
    move_pilots,
    Item::separator(),
    Item::danger("Delete squad", Message::DeleteSquad(menu.squad_id)),
  ];
  let anchor = iced::Point::new((menu.anchor.x - context_menu::MENU_WIDTH).max(0.0), menu.anchor.y);
  context_menu::context_menu(&menu.name, items, anchor)
}

fn resolve_add_tag_modal<'a>(
  state: &'a State,
  entity_type: &str,
  entity_id: i64,
) -> (&'a str, Vec<&'a Tag>, Vec<&'a Tag>) {
  let (name, assigned_ids): (&str, HashSet<i64>) = if entity_type == ENTITY_TYPE_CORPORATION {
    let corp = corp_for(state, entity_id);
    let name = corp.map_or("", |corp| corp.name.as_str());
    let assigned = corp
      .map(|corp| corp.tags.iter().map(|chip| chip.id).collect())
      .unwrap_or_default();
    (name, assigned)
  } else {
    let card = card_for(state, entity_id);
    let name = card.map_or("", |card| card.name.as_str());
    let assigned = card
      .map(|card| card.tags.iter().map(|chip| chip.id).collect())
      .unwrap_or_default();
    (name, assigned)
  };
  let assigned = state
    .all_tags
    .iter()
    .filter(|tag| assigned_ids.contains(&tag.id()))
    .collect();
  let assignable = state
    .all_tags
    .iter()
    .filter(|tag| !assigned_ids.contains(&tag.id()))
    .collect();
  (name, assigned, assignable)
}

fn card_for(state: &State, character_id: i64) -> Option<&CardModel> {
  state
    .groups
    .iter()
    .flat_map(|group| group.cards.iter())
    .chain(state.unassigned.iter())
    .find(|card| card.character_id == character_id)
}

fn corp_for(state: &State, corporation_id: i64) -> Option<&CorpCardModel> {
  state.corps.iter().find(|corp| corp.corporation_id == corporation_id)
}

fn pane_actions<'a>(_state: &'a State, pane: Pane) -> Vec<Element<'a, Message>> {
  match pane {
    Pane::Characters => vec![squad_ui::new_squad_button(), auth::add_character_button()],
    Pane::Corporations => vec![add_corporation_button()],
  }
}

fn corporations_body<'a>(state: &'a State, sync: &SyncStatus) -> Element<'a, Message> {
  if state.is_corp_filtered() {
    return corp_filtered_body(state, sync);
  }

  if state.corps.is_empty() {
    return corporations_empty_state();
  }

  corp_grid_scroll(&state.corps, sync)
}

fn corp_filtered_body<'a>(state: &'a State, sync: &SyncStatus) -> Element<'a, Message> {
  match state.corp_filtered() {
    Some(CorpFiltered::Loaded(corps)) if corps.is_empty() => corp_no_matches(),
    Some(CorpFiltered::Loaded(corps)) => corp_grid_scroll(corps, sync),
    Some(CorpFiltered::Error(error)) => corp_message(format!("Search failed: {error}"), color::status::DANGER),
    Some(CorpFiltered::Loading) | None => corp_message("Searching…".to_owned(), color::text::secondary()),
  }
}

fn corp_grid_scroll<'a>(corps: &'a [CorpCardModel], sync: &SyncStatus) -> Element<'a, Message> {
  let capped = container(corp_grid(corps, sync))
    .width(Length::Fill)
    .max_width(spacing::layout::GRID_MAX_WIDTH)
    .padding(spacing::SPACE_6);
  let centered = container(capped).width(Length::Fill).align_x(Horizontal::Center);
  let scroll = iced::widget::scrollable(centered)
    .id(CORP_SCROLL_ID)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .on_scroll(|viewport| Message::CorpScrolled(GridViewport::from_viewport(&viewport)));

  iced::widget::mouse_area(scroll).on_move(Message::DragMoved).into()
}

fn corp_grid<'a>(corps: &'a [CorpCardModel], sync: &SyncStatus) -> Element<'a, Message> {
  const COLUMNS: usize = 3;

  let mut rows: Vec<Element<'a, Message>> = Vec::with_capacity(corps.len() / COLUMNS + 1);
  for chunk in corps.chunks(COLUMNS) {
    let mut cells: Vec<Element<'a, Message>> = Vec::with_capacity(COLUMNS);
    for corp in chunk {
      cells.push(corp_card::corp_card(corp, corp_failure(sync, corp.corporation_id)));
    }
    while cells.len() < COLUMNS {
      cells.push(Space::new().width(Length::Fill).into());
    }
    rows.push(Row::with_children(cells).spacing(spacing::SPACE_3_5).into());
  }

  Column::with_children(rows)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn corporations_empty_state<'a>() -> Element<'a, Message> {
  let content = Column::with_children(vec![
    text("No corporations yet")
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text("Add a corporation to start tracking it.")
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_x(Horizontal::Center);

  container(content)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn corp_message<'a>(message: String, color: Color) -> Element<'a, Message> {
  let line = text(message)
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color));

  container(line)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn corp_no_matches<'a>() -> Element<'a, Message> {
  let content = Column::with_children(vec![
    svg(svg::Handle::from_memory(SEARCH_ICON))
      .width(Length::Fixed(NO_MATCH_ICON))
      .height(Length::Fixed(NO_MATCH_ICON))
      .style(|_, _| svg::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
    text("No corporations match")
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text("Try a different search or clear filters")
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
    button(
      text("Clear filters")
        .font(typography::body::REGULAR)
        .size(typography::size::SM),
    )
    .padding(control::padding())
    .on_press(Message::ClearSearch)
    .style(control::primary_button)
    .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_x(Horizontal::Center);

  container(content)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn add_corporation_button<'a>() -> Element<'a, Message> {
  button(
    Row::with_children(vec![
      text("+").size(typography::size::MD).into(),
      text("Add corporation")
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(control::padding())
  .on_press(Message::AddCorporationRequested)
  .style(control::ghost_button)
  .into()
}

fn roster_count(state: &State) -> String {
  let total = state.groups.iter().map(|group| group.cards.len()).sum::<usize>() + state.unassigned.len();
  match &state.filtered {
    Some(Filtered::Loaded(cards)) => format!("{} of {total}", cards.len()),
    _ => total.to_string(),
  }
}

fn corp_count(state: &State) -> String {
  let total = state.corps.len();
  match &state.corp_filtered {
    Some(CorpFiltered::Loaded(corps)) => format!("{} of {total}", corps.len()),
    _ => total.to_string(),
  }
}

pub fn all_tags(state: &State) -> &[Tag] {
  &state.all_tags
}

pub fn groups(state: &State) -> &[SquadGroup] {
  &state.groups
}

pub fn owned_roster(state: &State) -> Vec<OwnedPilot> {
  let cards = state
    .groups
    .iter()
    .flat_map(|group| group.cards.iter())
    .chain(state.unassigned.iter());

  cards
    .map(|card| OwnedPilot {
      color: card.accent.unwrap_or(DEFAULT_SQUAD_ACCENT),
      granted_scopes: state.granted_scopes_by_id.get(&card.character_id).cloned().flatten(),
      id: card.character_id,
      name: card.name.clone(),
    })
    .collect()
}

pub fn owned_corporations(state: &State) -> Vec<(i64, String)> {
  state
    .corps
    .iter()
    .map(|corp| (corp.corporation_id, corp.name.clone()))
    .collect()
}

pub fn is_squad_collapsed(state: &State, squad_id: i64) -> bool {
  state.collapsed_squads.contains(&squad_id)
}

pub fn unassigned(state: &State) -> &[CardModel] {
  &state.unassigned
}

pub fn unassigned_squad_id(state: &State) -> i64 {
  state.unassigned_squad_id
}

pub fn load_error(state: &State) -> Option<&str> {
  state.load_error.as_deref()
}

/// Whether the roster currently holds displayable data — squads, unassigned pilots, or corporations.
/// A failed refresh keeps this data on screen instead of blanking to an error (see the
/// `CharactersLoaded(Err(_))` handler); the error only surfaces on a cold load when this is false.
fn has_roster_data(state: &State) -> bool {
  !state.groups.is_empty() || !state.unassigned.is_empty() || !state.corps.is_empty()
}

pub fn dragging_card(state: &State) -> Option<i64> {
  match state.dragging {
    Some(Drag::Card(character_id)) => Some(character_id),
    _ => None,
  }
}

pub fn dragging_squad(state: &State) -> Option<i64> {
  match state.dragging {
    Some(Drag::Squad(squad_id)) => Some(squad_id),
    _ => None,
  }
}

pub fn drop_target(state: &State) -> Option<DropTarget> {
  state.drop_target
}

pub fn squad_drop_target(state: &State) -> Option<usize> {
  state.squad_drop_target
}

pub fn cursor(state: &State) -> Option<iced::Point> {
  state.cursor
}

/// Re-applies the persisted offset to whichever roster grid is currently visible, so a
/// drag's tree-shape change cannot snap it to the top. The corporations grid never enters
/// a card drag, so only the two character grids are re-applied here.
fn restore_active_grid_scroll(state: &State) -> Task<Message> {
  let (id, offset) = if state.is_filtered() {
    (roster::FILTERED_SCROLL_ID, state.filtered_scroll_offset)
  } else {
    (roster::ROSTER_SCROLL_ID, state.roster_scroll_offset)
  };
  operation::scroll_to(
    id,
    operation::AbsoluteOffset {
      x: 0.0,
      y: offset,
    },
  )
}

/// Signed pixels to move the grid this tick given the cursor's vertical position, or `None`
/// when the cursor is away from both edges or the grid cannot scroll further that way.
///
/// Proximity ramps from 0 at the inner edge of the hot zone to 1 at the viewport edge; speed
/// ramps linearly between [`AUTO_SCROLL_MIN_SPEED`] and [`AUTO_SCROLL_MAX_SPEED`]. The result is
/// clamped so the grid never scrolls past `0.0..=max_offset` (no overscroll/snap-back). A
/// degenerate viewport (zero height, no scrollable content) yields `None`.
fn auto_scroll_delta(cursor_y: f32, view: GridViewport) -> Option<f32> {
  if view.height <= 0.0 || view.max_offset <= 0.0 {
    return None;
  }
  let bottom = view.top + view.height;
  let speed = |proximity: f32| AUTO_SCROLL_MIN_SPEED + (AUTO_SCROLL_MAX_SPEED - AUTO_SCROLL_MIN_SPEED) * proximity;

  // Distance past the top edge of the hot zone (cursor near the top scrolls the content up,
  // i.e. toward a smaller offset); mirror for the bottom edge.
  let from_top = view.top + AUTO_SCROLL_EDGE_ZONE - cursor_y;
  let from_bottom = cursor_y - (bottom - AUTO_SCROLL_EDGE_ZONE);

  let delta = if from_top > 0.0 && from_top >= from_bottom {
    -speed((from_top / AUTO_SCROLL_EDGE_ZONE).clamp(0.0, 1.0))
  } else if from_bottom > 0.0 {
    speed((from_bottom / AUTO_SCROLL_EDGE_ZONE).clamp(0.0, 1.0))
  } else {
    return None;
  };

  let target = (view.offset + delta).clamp(0.0, view.max_offset);
  let applied = target - view.offset;
  (applied != 0.0).then_some(applied)
}

/// Advances the active grid's scroll offset one auto-scroll tick and re-applies it to the
/// scrollable, so a stationary cursor held in an edge zone keeps pulling rows into view.
fn auto_scroll_active_grid(state: &mut State) -> Task<Message> {
  let Some(cursor) = state.cursor else {
    return Task::none();
  };
  let Some(delta) = auto_scroll_delta(cursor.y, state.roster_viewport) else {
    return Task::none();
  };

  let offset = (state.roster_viewport.offset + delta).clamp(0.0, state.roster_viewport.max_offset);
  state.roster_viewport.offset = offset;
  if state.is_filtered() {
    state.filtered_scroll_offset = offset;
  } else {
    state.roster_scroll_offset = offset;
  }
  restore_active_grid_scroll(state)
}

fn end_drag(state: &mut State) {
  state.dragging = None;
  state.drop_target = None;
  state.squad_drop_target = None;
}

fn reorder_squad(state: &mut State, squad_id: i64, index: usize, db: &Database) -> Task<Message> {
  let mut ordered: Vec<i64> = state.groups.iter().map(|group| group.squad_id).collect();
  end_drag(state);
  let Some(from) = ordered.iter().position(|&id| id == squad_id) else {
    return Task::none();
  };
  let dragged = ordered.remove(from);
  ordered.insert(index.min(ordered.len()), dragged);
  write_squad(
    db.clone(),
    SquadWrite::Reorder {
      ordered,
    },
  )
}

pub fn card_failure(sync: &SyncStatus, character_id: i64) -> Option<Phase> {
  let subject = Subject::Character(character_id);
  let telemetry = sync.phase(&JobKey::new(JobKind::CharacterTelemetry, subject));
  let wallet = sync.phase(&JobKey::new(JobKind::CharacterWallet, subject));

  [telemetry, wallet]
    .into_iter()
    .flatten()
    .filter(|phase| matches!(phase, Phase::Failed | Phase::BackingOff))
    .max_by_key(failure_rank)
}

fn failure_rank(phase: &Phase) -> u8 {
  match phase {
    Phase::Failed => 2,
    Phase::BackingOff => 1,
    Phase::Blocked | Phase::Done | Phase::Empty | Phase::NotReady | Phase::Syncing => 0,
  }
}

pub fn corp_failure(sync: &SyncStatus, corporation_id: i64) -> Option<Phase> {
  [JobKind::CorporationProfile, JobKind::CorporationWallet]
    .into_iter()
    .filter_map(|kind| sync.phase(&JobKey::new(kind, Subject::Corporation(corporation_id))))
    .find(|phase| matches!(phase, Phase::Failed | Phase::BackingOff))
}

/// Bounded backoff schedule for retrying a roster load that hit a transient pool/acquire timeout.
/// The length of the array is the retry budget (the initial attempt plus one retry per entry); the
/// values grow so a brief write-storm has time to drain before each successive attempt. Genuine
/// errors are never retried — only transient timeouts (see [`is_transient_timeout`]).
const ROSTER_RETRY_BACKOFFS: [Duration; 3] = [
  Duration::from_millis(50),
  Duration::from_millis(150),
  Duration::from_millis(400),
];

/// A transient pool/acquire timeout is the single failure mode this layer recovers from: under the
/// one-writer/many-readers model a brief write-storm can starve the reader pool just long enough for
/// one of the roster load's ~13 queries to exceed `acquire_timeout`, even though the data is fine and
/// the very next attempt would succeed. Everything else (a real SQL error, a constraint violation, a
/// migration failure) is a genuine error that must surface unchanged.
fn is_transient_timeout(error: &crate::store::Error) -> bool {
  matches!(error, crate::store::Error::Sqlx(sqlx::Error::PoolTimedOut))
}

async fn load_roster(db: Database, features: FeatureFlags) -> Result<Roster, String> {
  retry_transient(ROSTER_RETRY_BACKOFFS, is_transient_timeout, || {
    load_roster_at(&db, Utc::now(), features)
  })
  .await
  .map_err(|err| err.to_string())
}

/// Runs `attempt`, retrying with the given bounded backoff only while the error is classified
/// transient by `is_transient`. The first non-transient error (or the last transient error once the
/// backoff budget is spent) is returned as-is, so genuine errors still surface. `backoffs` is the
/// retry budget: `attempt` runs `1 + backoffs.len()` times in the worst transient case, sleeping the
/// matching backoff between tries.
async fn retry_transient<T, E, F, Fut>(
  backoffs: impl IntoIterator<Item = Duration>,
  is_transient: impl Fn(&E) -> bool,
  mut attempt: F,
) -> Result<T, E>
where
  F: FnMut() -> Fut,
  Fut: std::future::Future<Output = Result<T, E>>,
{
  let mut backoffs = backoffs.into_iter();
  loop {
    match attempt().await {
      Ok(value) => return Ok(value),
      Err(error) if is_transient(&error) => match backoffs.next() {
        Some(delay) => tokio::time::sleep(delay).await,
        None => return Err(error),
      },
      Err(error) => return Err(error),
    }
  }
}

async fn load_roster_at(
  db: &Database,
  now: DateTime<Utc>,
  features: FeatureFlags,
) -> Result<Roster, crate::store::Error> {
  let characters = character::all_owned(db).await?;
  let corporations = org::all_corporations(db).await?;
  let states = character::all_states(db).await?;
  let squads = character::all_user_squads(db).await?;
  // The reserved "Unassigned" squad is never rendered as a group; its members ARE the
  // unassigned bucket, so it is normalized and split out separately from real squads.
  let reserved_unassigned_id = character::get_or_create_unassigned(db).await?.id();
  for squad in &squads {
    character::normalize(db, squad.id()).await?;
  }
  character::normalize(db, reserved_unassigned_id).await?;
  let squad_memberships = character::memberships(db).await?;
  let tags = infra::tag_all(db).await?;
  let tag_memberships = infra::memberships(db, ENTITY_TYPE_CHARACTER).await?;

  let required_scopes = auth_feature::scopes_for(&features);
  let credentials = infra::all(db).await?;
  let granted_by_id: HashMap<i64, Option<String>> = credentials
    .iter()
    .filter(|cred| cred.owner_type() == OwnerType::Character)
    .map(|cred| (cred.owner_id(), cred.scopes().clone()))
    .collect();
  let reauth_flag_by_id: HashMap<i64, bool> = credentials
    .iter()
    .filter(|cred| cred.owner_type() == OwnerType::Character)
    .map(|cred| (cred.owner_id(), cred.needs_reauth()))
    .collect();

  let store = images::default_store();
  let corp_by_id: HashMap<i64, _> = corporations.iter().map(|corp| (corp.id(), corp)).collect();
  let state_by_id: HashMap<i64, &CharacterState> = states.iter().map(|s| (s.character_id, s)).collect();
  let mut tags_by_char = tag_chips_by_entity(&tags, &tag_memberships);

  let squad_of: HashMap<i64, i64> = squad_memberships
    .iter()
    .map(|m| (m.character_id(), m.squad_id()))
    .collect();
  let position_of: HashMap<i64, i64> = squad_memberships
    .iter()
    .map(|m| (m.character_id(), m.position()))
    .collect();
  let squad_accent: HashMap<i64, Option<Color>> = squads
    .iter()
    .map(|s| (s.id(), parse_hex(s.color().as_deref())))
    .collect();

  let mut card_by_id: HashMap<i64, CardModel> = HashMap::new();
  for character in &characters {
    let id = character.id();
    let card = build_card(
      db,
      now,
      character,
      CardInputs {
        corp: corp_by_id.get(&character.corporation_id()).copied(),
        granted_scopes: granted_by_id.get(&id).and_then(Option::as_deref),
        persisted_reauth: reauth_flag_by_id.get(&id).copied().unwrap_or(false),
        position: position_of.get(&id).copied().unwrap_or(0),
        required_scopes: &required_scopes,
        squad_accent: squad_of
          .get(&id)
          .and_then(|sid| squad_accent.get(sid).copied().flatten()),
        state: state_by_id.get(&id).copied(),
        store: &store,
        tags: tags_by_char.remove(&id).unwrap_or_default(),
      },
    )
    .await?;
    card_by_id.insert(id, card);
  }

  let groups = assemble_groups(&squads, &squad_memberships, &mut card_by_id);
  let unassigned = assemble_unassigned(&characters, &squad_memberships, reserved_unassigned_id, &mut card_by_id);

  let corps = load_corps(db, features).await?;

  Ok((
    groups,
    unassigned,
    reserved_unassigned_id,
    tags,
    corps,
    features,
    granted_by_id,
  ))
}

async fn build_card(
  db: &Database,
  now: DateTime<Utc>,
  character: &Character,
  inputs: CardInputs<'_>,
) -> Result<CardModel, crate::store::Error> {
  let id = character.id();
  let state = inputs.state;
  let docked = state.map(|s| s.station_id.is_some() || s.structure_id.is_some());
  let training = match character::current_skillqueue(db, id, now).await? {
    Some(entry) => Some(resolve_training(db, &entry, now).await?),
    None => None,
  };
  let location = resolve_location(db, state).await?;
  Ok(CardModel {
    accent: inputs.squad_accent,
    character_id: id,
    corp_ticker: corp_ticker_label(inputs.corp.map(|c| c.ticker().as_str()), character.corporation_id()),
    docked,
    location,
    name: character.name().to_owned(),
    needs_reauth: inputs.persisted_reauth || needs_reauthorization(inputs.granted_scopes, inputs.required_scopes),
    portrait: images::resolve(inputs.store, images::ImageKind::CharacterPortrait, id),
    position: inputs.position,
    tags: inputs.tags,
    total_sp: state.and_then(|s| s.total_sp),
    training,
    wallet_balance: state.and_then(|s| s.wallet_balance),
  })
}

fn assemble_groups(
  squads: &[Squad],
  squad_memberships: &[CharacterSquad],
  card_by_id: &mut HashMap<i64, CardModel>,
) -> Vec<SquadGroup> {
  let mut members_by_squad: HashMap<i64, Vec<i64>> = HashMap::new();
  for membership in squad_memberships {
    members_by_squad
      .entry(membership.squad_id())
      .or_default()
      .push(membership.character_id());
  }

  let mut groups = Vec::new();
  for squad in squads {
    let cards = members_by_squad
      .get(&squad.id())
      .into_iter()
      .flatten()
      .filter_map(|cid| card_by_id.remove(cid))
      .collect::<Vec<_>>();
    groups.push(SquadGroup {
      accent: parse_hex(squad.color().as_deref()).unwrap_or(DEFAULT_SQUAD_ACCENT),
      cards,
      color_hex: squad.color().clone(),
      description: squad.description().clone(),
      name: squad.name().to_owned(),
      squad_id: squad.id(),
    });
  }
  groups
}

fn assemble_unassigned(
  characters: &[Character],
  squad_memberships: &[CharacterSquad],
  reserved_unassigned_id: i64,
  card_by_id: &mut HashMap<i64, CardModel>,
) -> Vec<CardModel> {
  let unassigned_position: HashMap<i64, i64> = squad_memberships
    .iter()
    .filter(|m| m.squad_id() == reserved_unassigned_id)
    .map(|m| (m.character_id(), m.position()))
    .collect();
  let mut unassigned: Vec<CardModel> = characters
    .iter()
    .filter_map(|character| card_by_id.remove(&character.id()))
    .collect();
  unassigned.sort_by_key(|card| {
    (
      unassigned_position.get(&card.character_id).copied().unwrap_or(i64::MAX),
      card.character_id,
    )
  });
  let mut next_straggler_slot = unassigned_position
    .values()
    .copied()
    .max()
    .map_or(0, |max| max.saturating_add(1));
  for card in &mut unassigned {
    if unassigned_position.contains_key(&card.character_id) {
      card.position = unassigned_position[&card.character_id];
    } else {
      card.position = next_straggler_slot;
      next_straggler_slot = next_straggler_slot.saturating_add(1);
    }
  }
  unassigned
}

fn tag_chips_by_entity(tags: &[Tag], memberships: &[EntityTag]) -> HashMap<i64, Vec<TagChip>> {
  let tag_color: HashMap<i64, Option<Color>> = tags.iter().map(|t| (t.id(), parse_hex(t.color().as_deref()))).collect();
  let tag_name: HashMap<i64, &str> = tags.iter().map(|t| (t.id(), t.name().as_str())).collect();
  let tag_rank: HashMap<i64, usize> = tags.iter().enumerate().map(|(rank, t)| (t.id(), rank)).collect();

  let mut by_entity: HashMap<i64, Vec<TagChip>> = HashMap::new();
  for membership in memberships {
    if let Some(name) = tag_name.get(&membership.tag_id()) {
      by_entity.entry(membership.entity_id()).or_default().push(TagChip {
        color: tag_color.get(&membership.tag_id()).copied().flatten(),
        id: membership.tag_id(),
        name: (*name).to_owned(),
      });
    }
  }
  for chips in by_entity.values_mut() {
    chips.sort_by_key(|chip| tag_rank.get(&chip.id).copied().unwrap_or(usize::MAX));
  }
  by_entity
}

async fn load_corps(db: &Database, features: FeatureFlags) -> Result<Vec<CorpCardModel>, crate::store::Error> {
  let owned = org::all_owned_corporations(db).await?;
  let store = images::default_store();

  let tags = infra::tag_all(db).await?;
  let tag_memberships = infra::memberships(db, ENTITY_TYPE_CORPORATION).await?;
  let mut tags_by_corp = tag_chips_by_entity(&tags, &tag_memberships);

  let required_scopes = auth_feature::corp_scopes_for(&features);
  let credentials = infra::all(db).await?;
  let granted_by_id: HashMap<i64, Option<String>> = credentials
    .iter()
    .filter(|cred| cred.owner_type() == OwnerType::Corporation)
    .map(|cred| (cred.owner_id(), cred.scopes().clone()))
    .collect();
  let reauth_flag_by_id: HashMap<i64, bool> = credentials
    .iter()
    .filter(|cred| cred.owner_type() == OwnerType::Corporation)
    .map(|cred| (cred.owner_id(), cred.needs_reauth()))
    .collect();

  let mut corps = Vec::with_capacity(owned.len());
  for corp in &owned {
    let id = corp.id();
    let granted = granted_by_id.get(&id).and_then(Option::clone);

    let alliance = match corp.alliance_id() {
      Some(alliance_id) => org::get_alliance(db, alliance_id).await?,
      None => None,
    };
    let ceo = character::get(db, corp.ceo_id()).await?.map(|c| c.name().to_owned());
    let hq = match corp.home_station_id() {
      Some(station_id) => sde::get_station(db, station_id)
        .await?
        .map(|station| station.name().to_owned()),
      None => None,
    };
    corps.push(CorpCardModel {
      alliance: alliance.as_ref().map(|a| a.name().to_owned()),
      alliance_ticker: alliance.as_ref().map(|a| a.ticker().to_owned()),
      ceo,
      corporation_id: id,
      needs_reauth: reauth_flag_by_id.get(&id).copied().unwrap_or(false)
        || needs_reauthorization(granted.as_deref(), &required_scopes),
      granted_scopes: granted,
      hq,
      logo: images::resolve(&store, images::ImageKind::CorporationLogo, id),
      members: Some(i64::from(corp.member_count())),
      name: corp.name().to_owned(),
      tags: tags_by_corp.remove(&id).unwrap_or_default(),
      tax_rate: Some(corp.tax_rate()),
      ticker: corp.ticker().to_owned(),
    });
  }

  Ok(corps)
}

async fn resolve_training(
  db: &Database,
  entry: &CharacterSkillqueue,
  now: DateTime<Utc>,
) -> Result<Training, crate::store::Error> {
  let item_type = sde::get_item_type(db, entry.skill_id()).await?;
  let skill = skill_label(item_type.as_ref().map(|t| t.name().as_str()), entry.skill_id());

  let finish = entry.finish_date().as_deref().and_then(parse_timestamp);
  let start = entry.start_date().as_deref().and_then(parse_timestamp);

  let remaining = finish.map_or_else(|| "—".to_owned(), |finish| format_remaining(finish - now));
  let progress = match (start, finish) {
    (Some(start), Some(finish)) if finish > start => {
      let span = (finish - start).num_seconds() as f32;
      let elapsed = (now - start).num_seconds() as f32;
      (elapsed / span).clamp(0.0, 1.0)
    }
    _ => 0.0,
  };

  Ok(Training {
    skill,
    level: entry.finished_level(),
    remaining,
    progress,
  })
}

async fn resolve_location(
  db: &Database,
  state: Option<&CharacterState>,
) -> Result<Option<String>, crate::store::Error> {
  let Some(state) = state else {
    return Ok(None);
  };

  if let Some(station_id) = state.station_id
    && let Some(station) = sde::get_station(db, station_id).await?
  {
    return Ok(Some(station.name().to_owned()));
  }
  if let Some(structure_id) = state.structure_id
    && let Some(structure) = sde::get_structure(db, structure_id).await?
  {
    return Ok(Some(structure.name().to_owned()));
  }
  if let Some(system_id) = state.solar_system_id
    && let Some(system) = sde::get_solar_system(db, system_id).await?
  {
    return Ok(Some(system.name().to_owned()));
  }
  Ok(None)
}

fn format_remaining(duration: chrono::Duration) -> String {
  let total_minutes = duration.num_minutes();
  if total_minutes <= 0 {
    return "Done".to_owned();
  }
  let days = total_minutes / (24 * 60);
  let hours = (total_minutes % (24 * 60)) / 60;
  let minutes = total_minutes % 60;
  if days > 0 {
    format!("{days}d {hours}h")
  } else if hours > 0 {
    format!("{hours}h {minutes}m")
  } else {
    format!("{minutes}m")
  }
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
  DateTime::parse_from_rfc3339(value)
    .ok()
    .map(|dt| dt.with_timezone(&Utc))
}

fn non_blank(value: &str) -> Option<String> {
  let trimmed = value.trim();
  (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn parse_hex(value: Option<&str>) -> Option<Color> {
  let hex = value?.strip_prefix('#')?;
  if hex.len() != 6 {
    return None;
  }
  let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
  let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
  let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
  Some(Color::from_rgb8(r, g, b))
}

pub fn needs_reauthorization(granted: Option<&str>, required: &[&str]) -> bool {
  let granted: HashSet<&str> = granted.unwrap_or_default().split_whitespace().collect();
  required.iter().any(|scope| !granted.contains(scope))
}

/// Persisted needs-reauth flag per owner id for the given owner type, sourced from the
/// credentials store. Used to overlay the flag onto cards built from the search/finder
/// projections, which carry no credential columns.
async fn reauth_flags(db: &Database, owner_type: OwnerType) -> Result<HashMap<i64, bool>, crate::store::Error> {
  Ok(
    infra::all(db)
      .await?
      .into_iter()
      .filter(|cred| cred.owner_type() == owner_type)
      .map(|cred| (cred.owner_id(), cred.needs_reauth()))
      .collect(),
  )
}

fn append_query(state: &mut State, fragment: &str) {
  let fragment = fragment.trim();
  if fragment.is_empty() {
    return;
  }
  if state.search_query.trim_end().is_empty() {
    state.search_query = fragment.to_owned();
  } else {
    state.search_query = format!("{} {fragment}", state.search_query.trim_end());
  }
}

#[cfg(test)]
fn card_from_row(row: character_card::CardRow, now: DateTime<Utc>) -> CardModel {
  card_from_row_with_reauth(row, now, false)
}

fn card_from_row_with_reauth(row: character_card::CardRow, now: DateTime<Utc>, needs_reauth: bool) -> CardModel {
  let store = images::default_store();
  CardModel {
    accent: parse_hex(row.squad_accent_hex.as_deref()),
    character_id: row.character_id,
    corp_ticker: corp_ticker_label(row.corp_ticker.as_deref(), row.corporation_id),
    docked: row.docked,
    location: row.location,
    name: row.name,
    needs_reauth,
    portrait: images::resolve(&store, images::ImageKind::CharacterPortrait, row.character_id),
    position: row.position.unwrap_or(0),
    tags: row
      .tags
      .into_iter()
      .map(|tag| TagChip {
        color: parse_hex(tag.color_hex.as_deref()),
        id: tag.id,
        name: tag.name,
      })
      .collect(),
    total_sp: row.total_sp,
    training: row.training.map(|training| training_from_row(training, now)),
    wallet_balance: row.wallet_balance,
  }
}

#[cfg(test)]
fn corp_card_from_row(row: corporation_card::CardRow) -> CorpCardModel {
  corp_card_from_row_with_reauth(row, false)
}

fn corp_card_from_row_with_reauth(row: corporation_card::CardRow, needs_reauth: bool) -> CorpCardModel {
  let store = images::default_store();
  CorpCardModel {
    alliance: row.alliance_name,
    alliance_ticker: row.alliance_ticker,
    ceo: row.ceo_name,
    corporation_id: row.corporation_id,
    // The corp finder projection carries no credential scopes; proactive scope drift is not
    // recomputed here, but the persisted needs-reauth flag is overlaid from the credentials store.
    granted_scopes: None,
    hq: row.hq_name,
    logo: images::resolve(&store, images::ImageKind::CorporationLogo, row.corporation_id),
    members: Some(row.member_count),
    name: row.name,
    needs_reauth,
    tags: row
      .tags
      .into_iter()
      .map(|tag| TagChip {
        color: parse_hex(tag.color_hex.as_deref()),
        id: tag.id,
        name: tag.name,
      })
      .collect(),
    tax_rate: Some(row.tax_rate),
    ticker: row.ticker,
  }
}

// Run the corp finder query and overlay each result's persisted needs-reauth flag. Split out from the
// `Task` wrapper so it can be exercised directly in tests (the debounce sleep stays in the wrapper).
async fn search_corp_cards(db: &Database, query: &str) -> Result<Vec<CorpCardModel>, String> {
  let rows = org::search_corporations(db, &parse(query))
    .await
    .map_err(|err| err.to_string())?;
  let reauth_by_id = reauth_flags(db, OwnerType::Corporation)
    .await
    .map_err(|err| err.to_string())?;
  Ok(
    rows
      .into_iter()
      .map(|row| {
        let needs_reauth = reauth_by_id.get(&row.corporation_id).copied().unwrap_or(false);
        corp_card_from_row_with_reauth(row, needs_reauth)
      })
      .collect(),
  )
}

fn run_corp_search(db: Database, query: String, generation: u64) -> Task<Message> {
  Task::perform(
    async move {
      tokio::time::sleep(Duration::from_millis(SEARCH_DEBOUNCE_MS)).await;
      search_corp_cards(&db, &query).await
    },
    move |result| Message::CorpSearchResults {
      generation,
      result,
    },
  )
}

// Run the character finder query and overlay each result's persisted needs-reauth flag. Split out from
// the `Task` wrapper so it can be exercised directly in tests (the debounce sleep stays in the wrapper).
async fn search_character_cards(db: &Database, query: &str) -> Result<Vec<CardModel>, String> {
  let now = Utc::now();
  let rows = character::search(db, &parse(query), &now.to_rfc3339())
    .await
    .map_err(|err| err.to_string())?;
  let reauth_by_id = reauth_flags(db, OwnerType::Character)
    .await
    .map_err(|err| err.to_string())?;
  Ok(
    rows
      .into_iter()
      .map(|row| {
        let needs_reauth = reauth_by_id.get(&row.character_id).copied().unwrap_or(false);
        card_from_row_with_reauth(row, now, needs_reauth)
      })
      .collect(),
  )
}

fn run_search(db: Database, query: String, generation: u64) -> Task<Message> {
  Task::perform(
    async move {
      tokio::time::sleep(Duration::from_millis(SEARCH_DEBOUNCE_MS)).await;
      search_character_cards(&db, &query).await
    },
    move |result| Message::SearchResults {
      generation,
      result,
    },
  )
}

fn trigger_search(state: &mut State, db: &Database) -> Task<Message> {
  state.search_generation = state.search_generation.wrapping_add(1);
  if state.search_query.trim().is_empty() {
    state.filtered = None;
    state.corp_filtered = None;
    return Task::none();
  }
  match state.active_pane {
    Pane::Characters => {
      state.corp_filtered = None;
      state.filtered = Some(Filtered::Loading);
      run_search(db.clone(), state.search_query.clone(), state.search_generation)
    }
    Pane::Corporations => {
      state.filtered = None;
      state.corp_filtered = Some(CorpFiltered::Loading);
      run_corp_search(db.clone(), state.search_query.clone(), state.search_generation)
    }
  }
}

fn training_from_row(training: character_card::CardTraining, now: DateTime<Utc>) -> Training {
  let finish = training.finish_date.as_deref().and_then(parse_timestamp);
  let start = training.start_date.as_deref().and_then(parse_timestamp);

  let remaining = finish.map_or_else(|| "—".to_owned(), |finish| format_remaining(finish - now));
  let progress = match (start, finish) {
    (Some(start), Some(finish)) if finish > start => {
      let span = (finish - start).num_seconds() as f32;
      let elapsed = (now - start).num_seconds() as f32;
      (elapsed / span).clamp(0.0, 1.0)
    }
    _ => 0.0,
  };

  Training {
    skill: skill_label(training.skill_name.as_deref(), training.skill_id),
    level: training.finished_level,
    remaining,
    progress,
  }
}

fn write_tag(db: Database, write: TagWrite) -> Task<Message> {
  Task::perform(async move { apply_tag_write(&db, write).await }, Message::TagsChanged)
}

async fn apply_tag_write(db: &Database, write: TagWrite) -> Result<(), String> {
  match write {
    TagWrite::Assign {
      entity_id,
      entity_type,
      tag_id,
    } => infra::assign(db, entity_type, entity_id, tag_id).await,
    TagWrite::CreateAndAssign {
      entity_id,
      entity_type,
      name,
    } => {
      let created = infra::create(db, &name, None, None)
        .await
        .map_err(|err| err.to_string())?;
      infra::assign(db, entity_type, entity_id, created.id()).await
    }
    TagWrite::Unassign {
      entity_id,
      entity_type,
      tag_id,
    } => infra::unassign(db, entity_type, entity_id, tag_id).await,
  }
  .map_err(|err| err.to_string())
}

fn write_squad(db: Database, write: SquadWrite) -> Task<Message> {
  Task::perform(
    async move { apply_squad_write(&db, write).await },
    Message::SquadsChanged,
  )
}

async fn apply_squad_write(db: &Database, write: SquadWrite) -> Result<(), String> {
  match write {
    SquadWrite::Assign {
      character_id,
      position,
      squad_id,
    } => character::assign(db, character_id, squad_id, position).await,
    SquadWrite::Create {
      color,
      description,
      name,
    } => character::create(db, &name, description.as_deref(), color.as_deref())
      .await
      .map(|_| ()),
    SquadWrite::Delete {
      squad_id,
    } => character::delete_squad(db, squad_id).await,
    SquadWrite::Reorder {
      ordered,
    } => character::reorder(db, &ordered).await,
    SquadWrite::Ungroup {
      squad_id,
    } => ungroup_squad(db, squad_id).await,
    SquadWrite::Update {
      color,
      description,
      name,
      squad_id,
    } => character::update(db, squad_id, &name, description.as_deref(), color.as_deref()).await,
  }
  .map_err(|err| err.to_string())
}

async fn ungroup_squad(db: &Database, squad_id: i64) -> Result<(), crate::store::Error> {
  for character_id in character::members(db, squad_id).await? {
    character::unassign(db, character_id).await?;
  }
  Ok(())
}

async fn remove_character(db: Database, character_id: i64) -> Result<(), String> {
  character::delete(&db, character_id)
    .await
    .map_err(|err| err.to_string())
}

async fn remove_corporation(db: Database, corporation_id: i64) -> Result<(), String> {
  infra::delete(&db, corporation_id, OwnerType::Corporation)
    .await
    .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::Feature;

  /// Feature flags with exactly the named groups enabled (every child on) and all others off.
  fn flags_with(features: &[Feature]) -> FeatureFlags {
    let mut flags = FeatureFlags::default();
    for feature in Feature::ALL {
      flags.set_enabled(feature, features.contains(&feature));
    }
    flags
  }

  mod card_failure {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::sync::Event;

    fn key(kind: JobKind, id: i64) -> JobKey {
      JobKey::new(kind, Subject::Character(id))
    }

    #[test]
    fn it_is_none_for_healthy_or_unreported_jobs() {
      let mut sync = SyncStatus::new();
      sync.apply(&Event::Finished {
        key: key(JobKind::CharacterWallet, 1),
        outcome: crate::sync::Outcome::synced(),
      });

      assert_eq!(card_failure(&sync, 1), None);
      assert_eq!(card_failure(&sync, 999), None);
    }

    #[test]
    fn it_prefers_failed_over_backing_off() {
      let mut sync = SyncStatus::new();
      sync.apply(&Event::BackingOff {
        key: key(JobKind::CharacterTelemetry, 1),
        retry_secs: 30,
      });
      sync.apply(&Event::Failed {
        key: key(JobKind::CharacterWallet, 1),
        reason: "boom".to_owned(),
      });

      assert_eq!(card_failure(&sync, 1), Some(Phase::Failed));
    }

    #[test]
    fn it_surfaces_a_failing_wallet_job() {
      let mut sync = SyncStatus::new();
      sync.apply(&Event::Finished {
        key: key(JobKind::CharacterTelemetry, 1),
        outcome: crate::sync::Outcome::synced(),
      });
      sync.apply(&Event::Failed {
        key: key(JobKind::CharacterWallet, 1),
        reason: "boom".to_owned(),
      });

      assert_eq!(card_failure(&sync, 1), Some(Phase::Failed));
    }
  }

  mod corporations {
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      store::{
        self,
        model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
        repo::infra,
      },
      sync::Event,
    };

    fn now() -> DateTime<Utc> {
      Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap()
    }

    async fn seed_owned_corporation(db: &Database, corp_id: i64, ceo_id: i64) {
      let alliance = Alliance::new(
        corp_id,
        corp_id,
        ceo_id,
        "2010-01-01T00:00:00Z",
        "Iron Helix Pact",
        "IHP",
      );
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 6, 7);
      let race = Race::new(2, 500_001, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Cobalt Syndicate", "COBSY");
      corp.set_alliance_id(corp_id);
      corp.set_ceo_id(ceo_id);
      corp.set_creation_date("2019-03-14T00:00:00Z");
      corp.set_creator_id(ceo_id);
      corp.set_member_count(1247);
      corp.set_tax_rate(0.10);
      let ceo = Character::new(ceo_id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Vex Voronova");
      character::insert_with_org(db, &ceo, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
      infra::upsert(db, ceo_id, OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();
      infra::upsert(
        db,
        corp_id,
        OwnerType::Corporation,
        "tok",
        "rt",
        9999,
        Some(ceo_id),
        None,
      )
      .await
      .unwrap();
    }

    async fn reload(state: &mut State, db: &Database) {
      let roster = load_roster_at(db, now(), FeatureFlags::default()).await.unwrap();
      let _ = update(state, Message::CharactersLoaded(Ok(roster)), db);
    }

    async fn reload_corp_tag_names(state: &mut State, db: &Database, corporation_id: i64) -> Vec<String> {
      reload(state, db).await;
      corp_for(state, corporation_id)
        .map(|corp| corp.tags.iter().map(|chip| chip.name.clone()).collect())
        .unwrap_or_default()
    }

    #[tokio::test]
    async fn a_flagged_corp_context_menu_offers_a_reauthorize_item() {
      use iced::advanced::widget::Tree;

      let db = store::open_test().await.unwrap();
      seed_owned_corporation(&db, 2_000_001, 8001).await;
      let mut state = State::new();
      reload(&mut state, &db).await;

      // The seeded corp credential carries no scopes, so against Feature::ALL it is a strict
      // subset of the required corp set and must be flagged for re-authorization.
      assert!(state.corps[0].needs_reauth);

      state.cursor = Some(iced::Point::new(10.0, 10.0));
      let _ = update(&mut state, Message::CorpRightPressed(2_000_001), &db);
      let menu = state.corp_context_menu.as_ref().unwrap();
      assert!(menu.needs_reauth);

      let flagged = corp_context_menu_view(menu);
      let flagged_rows = Tree::new(flagged.as_widget()).children.len();

      let mut clear = menu.clone();
      clear.needs_reauth = false;
      let clear_rows = Tree::new(corp_context_menu_view(&clear).as_widget()).children.len();

      assert_eq!(
        flagged_rows,
        clear_rows + 2,
        "a flagged corp menu adds the re-authorize row and its separator"
      );
    }

    #[tokio::test]
    async fn corp_failure_surfaces_a_lost_role_as_needs_reauthentication() {
      let mut sync = SyncStatus::new();
      let key = JobKey::new(JobKind::CorporationProfile, Subject::Corporation(2_000_001));

      assert_eq!(corp_failure(&sync, 2_000_001), None);

      sync.apply(&Event::Failed {
        key,
        reason: "needs re-authentication".to_owned(),
      });
      assert_eq!(corp_failure(&sync, 2_000_001), Some(Phase::Failed));
    }

    #[tokio::test]
    async fn corp_failure_surfaces_a_wallet_job_401_as_needs_reauthentication() {
      let mut sync = SyncStatus::new();
      let key = JobKey::new(JobKind::CorporationWallet, Subject::Corporation(2_000_002));

      assert_eq!(corp_failure(&sync, 2_000_002), None);

      sync.apply(&Event::Failed {
        key,
        reason: "needs re-authentication".to_owned(),
      });
      assert_eq!(corp_failure(&sync, 2_000_002), Some(Phase::Failed));
    }

    #[tokio::test]
    async fn it_assigns_a_tag_to_a_corporation_through_the_add_tag_flow() {
      let db = store::open_test().await.unwrap();
      seed_owned_corporation(&db, 2_000_001, 8001).await;
      let industry = infra::create(&db, "Industry", None, None).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;

      state.cursor = Some(iced::Point::new(10.0, 10.0));
      let _ = update(&mut state, Message::CorpRightPressed(2_000_001), &db);
      let _ = update(
        &mut state,
        Message::OpenAddTagModal {
          entity_id: 2_000_001,
          entity_type: ENTITY_TYPE_CORPORATION,
        },
        &db,
      );
      assert!(state.corp_context_menu.is_none());
      assert_eq!(
        state.add_tag_modal.as_ref().map(|modal| modal.entity_type),
        Some(ENTITY_TYPE_CORPORATION)
      );
      let _ = update(
        &mut state,
        Message::AssignTag {
          entity_id: 2_000_001,
          entity_type: ENTITY_TYPE_CORPORATION,
          tag_id: industry.id(),
        },
        &db,
      );
      assert!(state.add_tag_modal.is_none());

      apply_tag_write(
        &db,
        TagWrite::Assign {
          entity_id: 2_000_001,
          entity_type: ENTITY_TYPE_CORPORATION,
          tag_id: industry.id(),
        },
      )
      .await
      .unwrap();
      let names = reload_corp_tag_names(&mut state, &db, 2_000_001).await;

      assert_eq!(names, vec!["Industry".to_owned()]);
      assert!(
        infra::members(&db, industry.id(), ENTITY_TYPE_CHARACTER)
          .await
          .unwrap()
          .is_empty()
      );
    }

    #[tokio::test]
    async fn it_derives_needs_reauth_from_the_corp_grant_versus_enabled_features_and_clears_on_a_wider_grant() {
      use crate::clients::esi::scopes;

      let db = store::open_test().await.unwrap();
      seed_owned_corporation(&db, 2_000_001, 8001).await;

      let enabled = flags_with(&[Feature::Industry, Feature::Wallet]);
      let required = auth_feature::corp_scopes_for(&enabled);

      async fn grant(db: &Database, scopes: &str) {
        infra::upsert(
          db,
          2_000_001,
          OwnerType::Corporation,
          "tok",
          "rt",
          9999,
          Some(8001),
          Some(scopes),
        )
        .await
        .unwrap();
      }

      // A grant dropping one required corp scope is a strict subset of the required set.
      let strict_subset = required[1..].join(" ");
      grant(&db, &strict_subset).await;
      let corps = load_corps(&db, enabled).await.unwrap();
      assert_eq!(corps.len(), 1);
      assert!(
        corps[0].needs_reauth,
        "a corp grant missing a required corp scope must flag needs-reauth"
      );

      // A grant covering every required corp scope must clear the flag.
      grant(&db, &required.join(" ")).await;
      let corps = load_corps(&db, enabled).await.unwrap();
      assert!(
        !corps[0].needs_reauth,
        "a corp grant covering every required corp scope must clear needs-reauth"
      );

      // A superset grant (every required scope plus an extra) must also stay clear.
      grant(
        &db,
        &format!("{} {}", required.join(" "), scopes::CORPORATION_KILLMAILS),
      )
      .await;
      let corps = load_corps(&db, enabled).await.unwrap();
      assert!(
        !corps[0].needs_reauth,
        "a superset corp grant must not flag needs-reauth"
      );
    }

    #[tokio::test]
    async fn it_excludes_reference_corps_without_a_corporation_credential() {
      let db = store::open_test().await.unwrap();
      super::load_roster::seed_character(&db, 1, "Solo Pilot").await;

      let (.., corps, _features, _granted) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();

      assert!(corps.is_empty());
    }

    #[tokio::test]
    async fn it_renders_the_corp_remove_confirm_modal_over_the_backdrop_when_open() {
      let db = store::open_test().await.unwrap();
      seed_owned_corporation(&db, 2_000_001, 8001).await;
      let sync = SyncStatus::new();
      let mut state = State::new();
      state.active_pane = Pane::Corporations;
      reload(&mut state, &db).await;
      state.cursor = Some(iced::Point::new(40.0, 60.0));
      let _ = update(&mut state, Message::CorpRightPressed(2_000_001), &db);

      let _ = update(&mut state, Message::OpenCorpRemoveConfirm(2_000_001), &db);
      assert!(state.corp_remove_confirm.is_some());
      let _open: Element<'_, Message> = view(&state, &sync);
    }

    #[tokio::test]
    async fn it_resolves_a_director_added_corp_into_a_card_model() {
      let db = store::open_test().await.unwrap();
      seed_owned_corporation(&db, 2_000_001, 8001).await;

      let (.., corps, _features, _granted) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();

      assert_eq!(corps.len(), 1);
      let corp = &corps[0];
      assert_eq!(corp.name, "Cobalt Syndicate");
      assert_eq!(corp.ticker, "COBSY");
      assert_eq!(corp.alliance.as_deref(), Some("Iron Helix Pact"));
      assert_eq!(corp.alliance_ticker.as_deref(), Some("IHP"));
      assert_eq!(corp.ceo.as_deref(), Some("Vex Voronova"));
      assert_eq!(corp.members, Some(1247));
    }

    #[tokio::test]
    async fn it_unassigns_a_tag_from_a_corporation_leaving_the_rest() {
      let db = store::open_test().await.unwrap();
      seed_owned_corporation(&db, 2_000_001, 8001).await;
      let kept = infra::create(&db, "Kept", None, None).await.unwrap();
      let dropped = infra::create(&db, "Dropped", None, None).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_CORPORATION, 2_000_001, kept.id())
        .await
        .unwrap();
      infra::assign(&db, ENTITY_TYPE_CORPORATION, 2_000_001, dropped.id())
        .await
        .unwrap();
      let mut state = State::new();

      apply_tag_write(
        &db,
        TagWrite::Unassign {
          entity_id: 2_000_001,
          entity_type: ENTITY_TYPE_CORPORATION,
          tag_id: dropped.id(),
        },
      )
      .await
      .unwrap();
      let names = reload_corp_tag_names(&mut state, &db, 2_000_001).await;

      assert_eq!(names, vec!["Kept".to_owned()]);
    }

    #[tokio::test]
    async fn load_corps_populates_the_card_model_tags_from_the_polymorphic_join() {
      let db = store::open_test().await.unwrap();
      seed_owned_corporation(&db, 2_000_001, 8001).await;
      let industry = infra::create(&db, "Industry", None, None).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_CORPORATION, 2_000_001, industry.id())
        .await
        .unwrap();

      let (.., corps, _features, _granted) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();

      let corp = &corps[0];
      assert_eq!(
        corp.tags.iter().map(|chip| chip.name.as_str()).collect::<Vec<_>>(),
        ["Industry"]
      );
    }

    #[tokio::test]
    async fn the_corp_add_tag_modal_offers_only_tags_not_already_on_the_corp() {
      let db = store::open_test().await.unwrap();
      seed_owned_corporation(&db, 2_000_001, 8001).await;
      let on = infra::create(&db, "On", None, None).await.unwrap();
      let off = infra::create(&db, "Off", None, None).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_CORPORATION, 2_000_001, on.id())
        .await
        .unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;

      let (name, assigned, assignable) = resolve_add_tag_modal(&state, ENTITY_TYPE_CORPORATION, 2_000_001);
      assert_eq!(name, "Cobalt Syndicate");
      assert_eq!(assigned.iter().map(|tag| tag.id()).collect::<Vec<_>>(), vec![on.id()]);
      assert_eq!(
        assignable.iter().map(|tag| tag.id()).collect::<Vec<_>>(),
        vec![off.id()]
      );
    }

    #[test]
    fn the_corporations_pane_renders_the_grid_and_the_empty_state() {
      let sync = SyncStatus::new();
      let mut state = State::new();
      state.active_pane = Pane::Corporations;

      {
        let _empty: Element<'_, Message> = view(&state, &sync);
      }

      state.corps = vec![CorpCardModel {
        alliance: Some("Iron Helix Pact".to_owned()),
        alliance_ticker: Some("IHP".to_owned()),
        ceo: Some("Vex Voronova".to_owned()),
        corporation_id: 2_000_001,
        granted_scopes: None,
        hq: Some("Jita IV — Moon 4".to_owned()),
        logo: images::ImageState::Stale {
          id: 2_000_001,
          kind: images::ImageKind::CorporationLogo,
        },
        members: Some(1247),
        name: "Cobalt Syndicate".to_owned(),
        needs_reauth: false,
        tags: Vec::new(),
        tax_rate: Some(0.10),
        ticker: "COBSY".to_owned(),
      }];
      {
        let _populated: Element<'_, Message> = view(&state, &sync);
      }
    }

    #[tokio::test]
    async fn the_remove_flow_drops_the_corporation_credential() {
      let db = store::open_test().await.unwrap();
      seed_owned_corporation(&db, 2_000_001, 8001).await;
      let mut state = State::new();
      reload(&mut state, &db).await;

      state.cursor = Some(iced::Point::new(10.0, 10.0));
      let _ = update(&mut state, Message::CorpRightPressed(2_000_001), &db);
      assert!(state.corp_context_menu.is_some());
      let _ = update(&mut state, Message::OpenCorpRemoveConfirm(2_000_001), &db);
      assert!(state.corp_context_menu.is_none());
      assert!(state.corp_remove_confirm.is_some());

      let _ = update(&mut state, Message::RemoveCorporationConfirmed(2_000_001), &db);
      assert!(state.corp_remove_confirm.is_none());
      remove_corporation(db.clone(), 2_000_001).await.unwrap();

      reload(&mut state, &db).await;
      assert!(state.corps.is_empty());
      assert!(
        infra::get(&db, 2_000_001, OwnerType::Corporation)
          .await
          .unwrap()
          .is_none()
      );
      assert!(org::get_corporation(&db, 2_000_001).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn the_tab_count_reflects_the_number_of_director_added_corps() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      reload(&mut state, &db).await;
      assert_eq!(corp_count(&state), "0");

      seed_owned_corporation(&db, 2_000_001, 8001).await;
      reload(&mut state, &db).await;
      assert_eq!(corp_count(&state), "1");
    }
  }

  mod format_remaining {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_coarse_durations() {
      assert_eq!(
        format_remaining(chrono::Duration::minutes(2 * 24 * 60 + 14 * 60 + 22)),
        "2d 14h"
      );
      assert_eq!(format_remaining(chrono::Duration::minutes(8 * 60 + 12)), "8h 12m");
      assert_eq!(format_remaining(chrono::Duration::minutes(45)), "45m");
      assert_eq!(format_remaining(chrono::Duration::minutes(-5)), "Done");
    }
  }

  mod insert_signed_in_card {
    use pretty_assertions::{assert_eq, assert_ne};

    use super::*;

    fn real_card(character_id: i64) -> CardModel {
      CardModel {
        accent: None,
        character_id,
        corp_ticker: "TST".to_owned(),
        docked: Some(false),
        location: None,
        name: format!("Pilot {character_id}"),
        needs_reauth: false,
        portrait: images::ImageState::Stale {
          id: character_id,
          kind: images::ImageKind::CharacterPortrait,
        },
        position: 0,
        tags: Vec::new(),
        total_sp: Some(5_000_000),
        training: None,
        wallet_balance: Some(1.0),
      }
    }

    #[test]
    fn it_drops_the_placeholder_once_the_real_row_loads() {
      let mut state = State::new();
      insert_signed_in_card(&mut state, 42, "New Pilot".to_owned());

      // The sync backfill created the characters row; the next load carries the real card.
      state.groups = Vec::new();
      state.unassigned = vec![real_card(42)];
      merge_pending(&mut state);

      assert!(
        !state.pending.contains_key(&42),
        "the placeholder is retired once the real card loads"
      );
      assert_eq!(
        state.unassigned.iter().filter(|card| card.character_id == 42).count(),
        1,
        "the real card supersedes the placeholder with no duplicate"
      );
    }

    #[test]
    fn it_gives_the_pending_card_a_finite_append_slot_after_the_real_cards() {
      let mut state = State::new();
      let mut anchor = real_card(7);
      anchor.position = 4;
      state.unassigned = vec![anchor];

      insert_signed_in_card(&mut state, 42, "New Pilot".to_owned());

      let card = state
        .unassigned
        .iter()
        .find(|card| card.character_id == 42)
        .expect("the synthesized card is visible immediately");
      assert_eq!(card.position, 5);
      assert_ne!(state.pending[&42].position, i64::MAX);
    }

    #[test]
    fn it_keeps_the_card_after_a_load_that_still_lacks_its_row() {
      let mut state = State::new();
      insert_signed_in_card(&mut state, 42, "New Pilot".to_owned());

      // A full load completes but the new character's sync has not created its row yet.
      state.groups = Vec::new();
      state.unassigned = vec![real_card(7)];
      merge_pending(&mut state);

      assert!(
        state.unassigned.iter().any(|card| card.character_id == 42),
        "the onboarding card survives a load without its row, instead of vanishing"
      );
      assert!(state.pending.contains_key(&42));
    }

    #[test]
    fn it_renders_a_card_from_the_jwt_name_with_no_characters_row() {
      let mut state = State::new();

      insert_signed_in_card(&mut state, 42, "New Pilot".to_owned());

      let card = state
        .unassigned
        .iter()
        .find(|card| card.character_id == 42)
        .expect("the synthesized card is visible immediately");
      assert_eq!(card.name, "New Pilot");
      assert!(
        state.pending.contains_key(&42),
        "it is tracked as pending until its real row loads"
      );
    }

    #[test]
    fn it_sorts_the_pending_card_last_among_the_unassigned_cards() {
      let mut state = State::new();
      let mut anchor = real_card(7);
      anchor.position = 2;
      state.unassigned = vec![anchor];

      insert_signed_in_card(&mut state, 42, "New Pilot".to_owned());

      let last = state
        .unassigned
        .iter()
        .max_by_key(|card| card.position)
        .expect("the bucket is non-empty");
      assert_eq!(last.character_id, 42);
    }
  }

  mod load_roster {
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{Bloodline, Character, Corporation, Gender, OwnerType, Race},
      repo::infra,
    };

    fn now() -> DateTime<Utc> {
      Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap()
    }

    pub(super) async fn seed_character(db: &Database, id: i64, name: &str) {
      let bloodline = Bloodline::new(1, 90_000_001, 2, 3, "A bloodline.", 4, 5, "Civire", 6, 7);
      let race = Race::new(2, 500_001, "A race.", "Caldari");
      let mut corp = Corporation::new(90_000_001, "Corp One", "CORP1");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let pilot = Character::new(id, 1, 90_000_001, 2, "2003-05-12", Gender::Male, name);
      character::insert_with_org(db, &pilot, &bloodline, &race, &corp, None, None)
        .await
        .unwrap();
      infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn disabling_a_feature_clears_needs_reauth_without_touching_the_grant() {
      use crate::clients::esi::scopes;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Toggle Pilot").await;
      infra::upsert(
        &db,
        1,
        OwnerType::Character,
        "tok",
        "rt",
        9999,
        None,
        Some(&format!("{} {}", scopes::CHARACTER_WALLET, scopes::CHARACTER_CONTRACTS,)),
      )
      .await
      .unwrap();

      let (_groups, unassigned, ..) = load_roster_at(&db, now(), flags_with(&[Feature::Wallet, Feature::Mail]))
        .await
        .unwrap();
      assert!(unassigned[0].needs_reauth);

      let (_groups, unassigned, ..) = load_roster_at(&db, now(), flags_with(&[Feature::Wallet]))
        .await
        .unwrap();
      assert!(!unassigned[0].needs_reauth);
    }

    #[tokio::test]
    async fn it_carries_total_sp_from_character_state_onto_the_card() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Skilled Pilot").await;
      for (skill_id, sp) in [(100, 5_000_000_i64), (101, 1_250_000)] {
        sqlx::query("INSERT INTO character_skills (character_id, skill_id, active_skill_level, skillpoints_in_skill, trained_skill_level) VALUES (?, ?, ?, ?, ?)")
          .bind(1_i64)
          .bind(skill_id)
          .bind(0_i64)
          .bind(sp)
          .bind(0_i64)
          .execute(db.writer())
          .await
          .unwrap();
      }

      let (_, unassigned, ..) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();

      assert_eq!(unassigned[0].total_sp, Some(6_250_000));
    }

    #[tokio::test]
    async fn it_derives_needs_reauth_from_the_grant_versus_enabled_features_and_clears_on_a_wider_grant() {
      use crate::clients::esi::scopes;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Reauth Pilot").await;

      let enabled = flags_with(&[Feature::Wallet, Feature::Mail]);
      infra::upsert(
        &db,
        1,
        OwnerType::Character,
        "tok",
        "rt",
        9999,
        None,
        Some(scopes::CHARACTER_WALLET),
      )
      .await
      .unwrap();

      let (_groups, unassigned, ..) = load_roster_at(&db, now(), enabled).await.unwrap();
      assert!(
        unassigned[0].needs_reauth,
        "a grant missing a required scope must flag needs-reauth"
      );

      // The full required set for Wallet + Mail; granting all of it must clear the flag.
      let wider = auth_feature::scopes_for(&enabled).join(" ");
      infra::upsert(&db, 1, OwnerType::Character, "tok", "rt", 9999, None, Some(&wider))
        .await
        .unwrap();

      let (_groups, unassigned, ..) = load_roster_at(&db, now(), enabled).await.unwrap();
      assert!(
        !unassigned[0].needs_reauth,
        "a grant covering every required scope must clear needs-reauth"
      );
    }

    #[tokio::test]
    async fn it_groups_a_member_under_its_squad() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Squad Pilot").await;
      let group = character::create(&db, "Supers", None, Some("#3FB8DB")).await.unwrap();
      character::assign(&db, 1, group.id(), 0).await.unwrap();

      let (groups, unassigned, ..) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();

      assert_eq!(groups.len(), 1);
      assert_eq!(groups[0].name, "Supers");
      assert_eq!(groups[0].cards.len(), 1);
      assert!(groups[0].cards[0].accent.is_some());
      assert!(unassigned.is_empty());
    }

    #[tokio::test]
    async fn it_leaves_total_sp_none_when_no_skills_are_synced() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Unskilled Pilot").await;

      let (_, unassigned, ..) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();

      assert!(unassigned[0].total_sp.is_none());
    }

    #[tokio::test]
    async fn it_leaves_training_none_for_an_empty_queue() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Idle Pilot").await;

      let (_, unassigned, ..) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();

      assert!(unassigned[0].training.is_none());
    }

    #[tokio::test]
    async fn it_orders_card_chips_by_the_settings_sort_order() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Sorted Pilot").await;
      let first = infra::create(&db, "First", None, None).await.unwrap();
      let second = infra::create(&db, "Second", None, None).await.unwrap();
      let third = infra::create(&db, "Third", None, None).await.unwrap();
      for tag in [&third, &first, &second] {
        infra::assign(&db, ENTITY_TYPE_CHARACTER, 1, tag.id()).await.unwrap();
      }
      infra::reorder(&db, &[third.id(), first.id(), second.id()])
        .await
        .unwrap();

      let (_, unassigned, ..) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();

      assert_eq!(
        unassigned[0]
          .tags
          .iter()
          .map(|chip| chip.name.as_str())
          .collect::<Vec<_>>(),
        vec!["Third", "First", "Second"]
      );
    }

    #[tokio::test]
    async fn it_places_an_unsquadded_character_in_the_unassigned_bucket() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Solo Pilot").await;

      let (groups, unassigned, ..) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();

      assert!(groups.is_empty());
      assert_eq!(unassigned.len(), 1);
      assert_eq!(unassigned[0].corp_ticker, "CORP1");
    }

    #[tokio::test]
    async fn it_resolves_a_training_skill_with_remaining_and_progress() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Trainer").await;
      let entry = CharacterSkillqueue {
        character_id: 1,
        finish_date: Some("2026-05-25T00:00:00Z".to_owned()),
        finished_level: 5,
        level_end_sp: Some(256_000),
        level_start_sp: Some(45_255),
        queue_position: 0,
        skill_id: 3300,
        start_date: Some("2026-05-05T00:00:00Z".to_owned()),
        training_start_sp: Some(45_255),
      };
      character::replace_skillqueue(&db, 1, &[entry]).await.unwrap();

      let (_, unassigned, ..) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();
      let training = unassigned[0].training.as_ref().unwrap();

      assert_eq!(training.level, 5);
      assert!((training.progress - 0.5).abs() < 0.01);
      assert_eq!(training.remaining, "10d 0h");
    }

    #[tokio::test]
    async fn it_resolves_tag_chips_for_a_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Tagged Pilot").await;
      let main = infra::create(&db, "Main", None, Some("#5BB97E")).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_CHARACTER, 1, main.id()).await.unwrap();

      let (_, unassigned, ..) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();

      assert_eq!(unassigned[0].tags.len(), 1);
      assert_eq!(unassigned[0].tags[0].name, "Main");
      assert!(unassigned[0].tags[0].color.is_some());
    }

    #[tokio::test]
    async fn no_row_stragglers_sort_after_positioned_members_by_id() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 5, "Positioned").await;
      seed_character(&db, 9, "Straggler Nine").await;
      seed_character(&db, 1, "Straggler One").await;
      character::unassign(&db, 5).await.unwrap();

      let (_groups, unassigned, ..) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();

      assert_eq!(
        unassigned.iter().map(|card| card.character_id).collect::<Vec<_>>(),
        vec![5, 1, 9]
      );
    }

    #[tokio::test]
    async fn the_reserved_unassigned_squad_is_never_a_group_and_its_members_are_the_bucket() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Solo Pilot").await;
      character::unassign(&db, 1).await.unwrap();

      let (groups, unassigned, ..) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();

      assert!(groups.is_empty());
      assert_eq!(unassigned.len(), 1);
      assert_eq!(unassigned[0].character_id, 1);
    }

    #[tokio::test]
    async fn the_unassigned_bucket_is_ordered_by_reserved_squad_position() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 10, "Ten").await;
      seed_character(&db, 20, "Twenty").await;
      seed_character(&db, 30, "Thirty").await;
      character::unassign(&db, 20).await.unwrap();
      character::unassign(&db, 30).await.unwrap();
      character::unassign(&db, 10).await.unwrap();

      let (_groups, unassigned, ..) = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();

      assert_eq!(
        unassigned.iter().map(|card| card.character_id).collect::<Vec<_>>(),
        vec![20, 30, 10]
      );
    }
  }

  mod resilience {
    use std::cell::Cell;

    use chrono::TimeZone;
    use pretty_assertions::assert_eq;

    use super::{load_roster::seed_character, *};
    use crate::store;

    fn now() -> DateTime<Utc> {
      Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap()
    }

    /// A single transient pool timeout mid-load is the failure the retry layer must absorb so the UI
    /// never shows "Couldn't load characters." This simulates a load that times out twice before
    /// succeeding and asserts the wrapper retries through it to the success.
    #[tokio::test]
    async fn it_retries_through_a_transient_pool_timeout_to_a_successful_load() {
      let attempts = Cell::new(0_u32);

      let result: Result<&str, crate::store::Error> =
        retry_transient(ROSTER_RETRY_BACKOFFS, is_transient_timeout, || {
          let attempt = attempts.get() + 1;
          attempts.set(attempt);
          async move {
            if attempt < 3 {
              Err(crate::store::Error::Sqlx(sqlx::Error::PoolTimedOut))
            } else {
              Ok("loaded")
            }
          }
        })
        .await;

      assert_eq!(result.unwrap(), "loaded");
      assert_eq!(attempts.get(), 3, "should have retried the two transient timeouts");
    }

    /// A genuine (non-transient) error must surface immediately, never retried — only pool timeouts
    /// are recoverable. Here a row-not-found error is returned on the first try and is not retried.
    #[tokio::test]
    async fn it_surfaces_a_genuine_error_without_retrying() {
      let attempts = Cell::new(0_u32);

      let result: Result<&str, crate::store::Error> =
        retry_transient(ROSTER_RETRY_BACKOFFS, is_transient_timeout, || {
          attempts.set(attempts.get() + 1);
          async move { Err(crate::store::Error::Sqlx(sqlx::Error::RowNotFound)) }
        })
        .await;

      assert!(matches!(
        result,
        Err(crate::store::Error::Sqlx(sqlx::Error::RowNotFound))
      ));
      assert_eq!(attempts.get(), 1, "a genuine error must not be retried");
    }

    /// A pool timeout that never clears within the retry budget surfaces as an error after the
    /// budget is spent (initial attempt plus one try per backoff entry).
    #[tokio::test]
    async fn it_gives_up_after_the_backoff_budget_on_a_persistent_timeout() {
      let attempts = Cell::new(0_u32);

      let result: Result<&str, crate::store::Error> =
        retry_transient(ROSTER_RETRY_BACKOFFS, is_transient_timeout, || {
          attempts.set(attempts.get() + 1);
          async move { Err(crate::store::Error::Sqlx(sqlx::Error::PoolTimedOut)) }
        })
        .await;

      assert!(matches!(
        result,
        Err(crate::store::Error::Sqlx(sqlx::Error::PoolTimedOut))
      ));
      assert_eq!(
        attempts.get(),
        1 + ROSTER_RETRY_BACKOFFS.len() as u32,
        "should attempt once plus one retry per backoff entry"
      );
    }

    #[test]
    fn it_classifies_only_a_pool_timeout_as_transient() {
      assert!(is_transient_timeout(&crate::store::Error::Sqlx(
        sqlx::Error::PoolTimedOut
      )));
      assert!(!is_transient_timeout(&crate::store::Error::Sqlx(
        sqlx::Error::RowNotFound
      )));
      assert!(!is_transient_timeout(&crate::store::Error::ReservedSquad));
    }

    /// A failed refresh while a roster is already on screen must keep that roster rather than blank
    /// it to "Couldn't load characters." This loads a real roster, then feeds a failed
    /// `CharactersLoaded` and asserts the cards survive and no load error is set.
    #[tokio::test]
    async fn a_failed_refresh_retains_the_already_displayed_roster() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Resilient Pilot").await;
      let mut state = State::new();
      let roster = load_roster_at(&db, now(), FeatureFlags::default()).await.unwrap();
      let _ = update(&mut state, Message::CharactersLoaded(Ok(roster)), &db);
      assert!(has_roster_data(&state), "precondition: roster is populated");

      let _ = update(
        &mut state,
        Message::CharactersLoaded(Err("pool timed out".to_owned())),
        &db,
      );

      assert_eq!(load_error(&state), None, "a failed refresh must not blank the roster");
      assert_eq!(state.unassigned.len(), 1, "last-good roster must be retained");
    }

    /// On a cold load (nothing displayed yet) a genuine load failure still surfaces, so the user is
    /// told something is wrong instead of staring at a silent empty view.
    #[tokio::test]
    async fn a_failed_cold_load_still_surfaces_the_error() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      assert!(!has_roster_data(&state), "precondition: nothing displayed yet");

      let _ = update(&mut state, Message::CharactersLoaded(Err("boom".to_owned())), &db);

      assert_eq!(load_error(&state), Some("boom"));
    }
  }

  mod needs_reauthorization {
    use super::*;
    use crate::clients::esi::scopes;

    #[test]
    fn a_missing_required_scope_needs_reauth() {
      let granted = scopes::CHARACTER_SKILLS;
      let required = [scopes::CHARACTER_SKILLS, scopes::CHARACTER_WALLET];

      assert!(needs_reauthorization(Some(granted), &required));
    }

    #[test]
    fn a_skill_monitoring_grant_lacking_implants_needs_reauth() {
      let granted = format!("{} {}", scopes::CHARACTER_SKILLS, scopes::CHARACTER_SKILLQUEUE);
      let required = auth_feature::scopes_for(&flags_with(&[Feature::SkillMonitoring]));

      assert!(needs_reauthorization(Some(&granted), &required));
    }

    #[test]
    fn a_superset_grant_clears_reauth() {
      let granted = format!(
        "{} {} {}",
        scopes::CHARACTER_WALLET,
        scopes::CHARACTER_SKILLS,
        scopes::CHARACTER_MAIL,
      );
      let required = [scopes::CHARACTER_WALLET, scopes::CHARACTER_SKILLS];

      assert!(!needs_reauthorization(Some(&granted), &required));
    }

    #[test]
    fn an_empty_grant_needs_reauth_when_anything_is_required() {
      assert!(needs_reauthorization(None, &[scopes::CHARACTER_WALLET]));
      assert!(needs_reauthorization(Some(""), &[scopes::CHARACTER_WALLET]));
      assert!(needs_reauthorization(Some("   "), &[scopes::CHARACTER_WALLET]));
    }

    #[test]
    fn an_exact_match_clears_reauth() {
      let granted = format!("{} {}", scopes::CHARACTER_WALLET, scopes::CHARACTER_SKILLS);
      let required = [scopes::CHARACTER_WALLET, scopes::CHARACTER_SKILLS];

      assert!(!needs_reauthorization(Some(&granted), &required));
    }

    #[test]
    fn extra_whitespace_in_the_grant_is_ignored() {
      let granted = format!("  {}   {}  ", scopes::CHARACTER_WALLET, scopes::CHARACTER_SKILLS);
      let required = [scopes::CHARACTER_WALLET, scopes::CHARACTER_SKILLS];

      assert!(!needs_reauthorization(Some(&granted), &required));
    }

    #[test]
    fn no_required_scopes_never_needs_reauth() {
      assert!(!needs_reauthorization(None, &[]));
      assert!(!needs_reauthorization(Some(scopes::CHARACTER_WALLET), &[]));
    }
  }

  mod owned_roster {
    use chrono::TimeZone;
    use pretty_assertions::{assert_eq, assert_ne};

    use super::{load_roster::seed_character, *};
    use crate::store;

    async fn reload(state: &mut State, db: &Database) {
      let roster = load_roster_at(
        db,
        Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap(),
        FeatureFlags::default(),
      )
      .await
      .unwrap();
      let _ = update(state, Message::CharactersLoaded(Ok(roster)), db);
    }

    #[tokio::test]
    async fn it_flattens_squad_members_then_the_unassigned_bucket_with_per_pilot_colors() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Squad Pilot").await;
      seed_character(&db, 2, "Solo Pilot").await;
      let group = character::create(&db, "Supers", None, Some("#3FB8DB")).await.unwrap();
      character::assign(&db, 1, group.id(), 0).await.unwrap();

      let mut state = State::new();
      reload(&mut state, &db).await;
      let pilots = owned_roster(&state);

      assert_eq!(pilots.len(), 2);
      assert_eq!(pilots[0].id, 1);
      assert_eq!(pilots[0].name, "Squad Pilot");
      assert_eq!(pilots[1].id, 2);

      assert_ne!(pilots[0].color, DEFAULT_SQUAD_ACCENT);
      assert_eq!(pilots[1].color, DEFAULT_SQUAD_ACCENT);
    }

    #[tokio::test]
    async fn it_is_empty_for_an_empty_roster() {
      let db = store::open_test().await.unwrap();

      let mut state = State::new();
      reload(&mut state, &db).await;

      assert!(owned_roster(&state).is_empty());
    }
  }

  mod parse_hex {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_a_six_digit_hex() {
      assert_eq!(parse_hex(Some("#3FB8DB")), Some(Color::from_rgb8(0x3F, 0xB8, 0xDB)));
    }

    #[test]
    fn it_rejects_malformed_or_absent_colors() {
      assert_eq!(parse_hex(None), None);
      assert_eq!(parse_hex(Some("3FB8DB")), None);
      assert_eq!(parse_hex(Some("#FFF")), None);
    }
  }

  mod search {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    const NOW: &str = "2026-06-01T00:00:00Z";

    fn now() -> DateTime<Utc> {
      parse_timestamp(NOW).unwrap()
    }

    fn sample_row(character_id: i64, name: &str) -> character_card::CardRow {
      character_card::CardRow {
        character_id,
        corp_ticker: Some("CBLT".to_owned()),
        corporation_id: 98_000_001,
        docked: Some(true),
        location: Some("Jita IV".to_owned()),
        name: name.to_owned(),
        position: Some(2),
        squad_accent_hex: Some("#3FB8DB".to_owned()),
        tags: vec![character_card::CardTag {
          color_hex: Some("#FF0000".to_owned()),
          id: 7,
          name: "PvP".to_owned(),
        }],
        total_sp: Some(1_000_000),
        training: Some(character_card::CardTraining {
          finish_date: Some("2026-06-01T01:00:00Z".to_owned()),
          finished_level: 4,
          skill_id: 3300,
          skill_name: Some("Gunnery".to_owned()),
          start_date: Some("2026-05-31T23:00:00Z".to_owned()),
        }),
        wallet_balance: Some(50.0),
      }
    }

    fn sample_corp_row(corporation_id: i64, name: &str) -> corporation_card::CardRow {
      corporation_card::CardRow {
        alliance_name: Some("Brave Collective".to_owned()),
        alliance_ticker: Some("BRAVE".to_owned()),
        ceo_name: Some("Cobalt Director".to_owned()),
        corporation_id,
        hq_name: Some("Jita Trade Hub".to_owned()),
        member_count: 100,
        name: name.to_owned(),
        tags: vec![corporation_card::CardTag {
          color_hex: Some("#00CCFF".to_owned()),
          id: 3,
          name: "Mining".to_owned(),
        }],
        tax_rate: 0.1,
        ticker: "CBLT".to_owned(),
      }
    }

    #[tokio::test]
    async fn it_appends_an_inserted_fragment_space_separated() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::InsertQuery("corp:cobalt".to_owned()), &db);
      assert_eq!(state.search_query, "corp:cobalt");

      let _ = update(&mut state, Message::InsertQuery("tag:pvp".to_owned()), &db);

      assert_eq!(state.search_query, "corp:cobalt tag:pvp");
      assert!(matches!(state.filtered, Some(Filtered::Loading)));
    }

    #[tokio::test]
    async fn it_applies_loaded_corp_results_for_the_current_generation() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      state.active_pane = Pane::Corporations;
      let _ = update(&mut state, Message::SearchChanged("mining".to_owned()), &db);

      let _ = update(
        &mut state,
        Message::CorpSearchResults {
          generation: 1,
          result: Ok(vec![corp_card_from_row(sample_corp_row(2001, "Cobalt Industries"))]),
        },
        &db,
      );

      match &state.corp_filtered {
        Some(CorpFiltered::Loaded(corps)) => {
          assert_eq!(corps.len(), 1);
          assert_eq!(corps[0].name, "Cobalt Industries");
        }
        other => panic!("expected loaded corp results, got {other:?}"),
      }
    }

    #[tokio::test]
    async fn it_applies_only_the_current_generation_of_results() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::SearchChanged("corp:cobalt".to_owned()), &db);

      let _ = update(
        &mut state,
        Message::SearchResults {
          generation: 0,
          result: Ok(vec![card_from_row(sample_row(1, "Stale"), now())]),
        },
        &db,
      );
      assert!(matches!(state.filtered, Some(Filtered::Loading)));

      let _ = update(
        &mut state,
        Message::SearchResults {
          generation: 1,
          result: Ok(vec![card_from_row(sample_row(2, "Fresh"), now())]),
        },
        &db,
      );

      match &state.filtered {
        Some(Filtered::Loaded(cards)) => {
          assert_eq!(cards.len(), 1);
          assert_eq!(cards[0].name, "Fresh");
        }
        other => panic!("expected loaded results, got {other:?}"),
      }
    }

    #[tokio::test]
    async fn it_clears_both_filters_on_a_tab_switch_with_an_empty_query() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::TabSelected(Pane::Corporations), &db);

      assert!(!state.is_filtered());
      assert!(!state.is_corp_filtered());
    }

    #[tokio::test]
    async fn it_clears_the_corp_filter_on_clear_search() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      state.active_pane = Pane::Corporations;
      let _ = update(&mut state, Message::SearchChanged("mining".to_owned()), &db);
      assert!(state.is_corp_filtered());

      let _ = update(&mut state, Message::ClearSearch, &db);

      assert_eq!(state.search_query, "");
      assert!(!state.is_corp_filtered());
    }

    #[tokio::test]
    async fn it_clears_the_filter_on_an_empty_change() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::SearchChanged("corp:cobalt".to_owned()), &db);

      let _ = update(&mut state, Message::SearchChanged("   ".to_owned()), &db);

      assert!(!state.is_filtered());
    }

    #[tokio::test]
    async fn it_clears_the_query_and_filter_on_clear_search() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::SearchChanged("corp:cobalt".to_owned()), &db);

      let _ = update(&mut state, Message::ClearSearch, &db);

      assert_eq!(state.search_query, "");
      assert!(!state.is_filtered());
    }

    #[test]
    fn it_defaults_position_and_clears_training_when_idle() {
      let mut row = sample_row(1, "Pilot");
      row.position = None;
      row.training = None;

      let card = card_from_row(row, now());

      assert_eq!(card.position, 0);
      assert!(card.training.is_none());
    }

    #[tokio::test]
    async fn it_discards_stale_corp_results() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      state.active_pane = Pane::Corporations;
      let _ = update(&mut state, Message::SearchChanged("mining".to_owned()), &db);

      let _ = update(
        &mut state,
        Message::CorpSearchResults {
          generation: 0,
          result: Ok(vec![corp_card_from_row(sample_corp_row(9, "Stale Corp"))]),
        },
        &db,
      );

      assert!(matches!(state.corp_filtered, Some(CorpFiltered::Loading)));
    }

    #[test]
    fn it_maps_a_corp_projection_row_to_a_card_model() {
      let card = corp_card_from_row(sample_corp_row(2001, "Cobalt Industries"));

      assert_eq!(card.corporation_id, 2001);
      assert_eq!(card.name, "Cobalt Industries");
      assert_eq!(card.ticker, "CBLT");
      assert_eq!(card.alliance.as_deref(), Some("Brave Collective"));
      assert_eq!(card.alliance_ticker.as_deref(), Some("BRAVE"));
      assert_eq!(card.ceo.as_deref(), Some("Cobalt Director"));
      assert_eq!(card.hq.as_deref(), Some("Jita Trade Hub"));
      assert_eq!(card.members, Some(100));
      assert_eq!(card.tax_rate, Some(0.1));

      assert_eq!(card.tags.len(), 1);
      assert_eq!(card.tags[0].name, "Mining");
      assert_eq!(card.tags[0].color, parse_hex(Some("#00CCFF")));

      assert_eq!(card.logo.stale_key(), Some((images::ImageKind::CorporationLogo, 2001)));
    }

    #[test]
    fn it_maps_a_projection_row_to_a_card_model() {
      let card = card_from_row(sample_row(42, "Cobalt Scout"), now());

      assert_eq!(card.character_id, 42);
      assert_eq!(card.name, "Cobalt Scout");
      assert_eq!(card.corp_ticker, "CBLT");
      assert_eq!(card.docked, Some(true));
      assert_eq!(card.location.as_deref(), Some("Jita IV"));
      assert_eq!(card.position, 2);
      assert_eq!(card.total_sp, Some(1_000_000));
      assert_eq!(card.wallet_balance, Some(50.0));

      assert_eq!(card.accent, parse_hex(Some("#3FB8DB")));
      assert_eq!(card.tags.len(), 1);
      assert_eq!(card.tags[0].name, "PvP");
      assert_eq!(card.tags[0].color, parse_hex(Some("#FF0000")));

      assert_eq!(
        card.portrait.stale_key(),
        Some((images::ImageKind::CharacterPortrait, 42))
      );
    }

    #[tokio::test]
    async fn it_re_triggers_the_now_active_tab_on_a_tab_switch() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::SearchChanged("cobalt".to_owned()), &db);
      assert!(matches!(state.filtered, Some(Filtered::Loading)));
      assert!(state.corp_filtered.is_none());
      let after_type = state.search_generation;

      let _ = update(&mut state, Message::TabSelected(Pane::Corporations), &db);
      assert_eq!(state.active_pane, Pane::Corporations);
      assert!(matches!(state.corp_filtered, Some(CorpFiltered::Loading)));
      assert!(state.filtered.is_none());
      assert_eq!(state.search_query, "cobalt");
      assert_eq!(state.search_generation, after_type + 1);
    }

    #[tokio::test]
    async fn it_renders_an_empty_corp_result_as_the_no_matches_state() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      state.active_pane = Pane::Corporations;
      let _ = update(&mut state, Message::SearchChanged("nomatch".to_owned()), &db);

      let _ = update(
        &mut state,
        Message::CorpSearchResults {
          generation: 1,
          result: Ok(vec![]),
        },
        &db,
      );

      assert!(matches!(state.corp_filtered, Some(CorpFiltered::Loaded(ref corps)) if corps.is_empty()));
    }

    #[test]
    fn it_resolves_training_progress_and_remaining_against_now() {
      let card = card_from_row(sample_row(1, "Pilot"), now());

      let training = card.training.expect("training present");

      assert_eq!(training.skill, "Gunnery");
      assert_eq!(training.level, 4);
      assert_eq!(training.progress, 0.5);
      assert_eq!(training.remaining, "1h 0m");
    }

    #[test]
    fn it_shows_the_corp_tab_badge_as_n_of_m_when_filtered() {
      let mut state = State::new();
      state.corps = vec![
        corp_card_from_row(sample_corp_row(1, "A")),
        corp_card_from_row(sample_corp_row(2, "B")),
        corp_card_from_row(sample_corp_row(3, "C")),
      ];

      assert_eq!(corp_count(&state), "3");

      state.corp_filtered = Some(CorpFiltered::Loaded(vec![corp_card_from_row(sample_corp_row(1, "A"))]));
      assert_eq!(corp_count(&state), "1 of 3");

      state.corp_filtered = Some(CorpFiltered::Loading);
      assert_eq!(corp_count(&state), "3");
    }

    #[test]
    fn it_shows_the_tab_badge_as_n_of_m_when_filtered() {
      let mut state = State::new();
      state.unassigned = vec![
        card_from_row(sample_row(1, "A"), now()),
        card_from_row(sample_row(2, "B"), now()),
        card_from_row(sample_row(3, "C"), now()),
      ];

      assert_eq!(roster_count(&state), "3");

      state.filtered = Some(Filtered::Loaded(vec![card_from_row(sample_row(1, "A"), now())]));
      assert_eq!(roster_count(&state), "1 of 3");

      state.filtered = Some(Filtered::Loading);
      assert_eq!(roster_count(&state), "3");
    }

    #[tokio::test]
    async fn it_starts_loading_the_corp_filter_on_a_non_empty_change_on_the_corps_tab() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      state.active_pane = Pane::Corporations;

      let _ = update(&mut state, Message::SearchChanged("mining".to_owned()), &db);

      assert_eq!(state.search_query, "mining");
      assert!(state.is_corp_filtered());
      assert!(matches!(state.corp_filtered, Some(CorpFiltered::Loading)));
      assert!(state.filtered.is_none());
      assert_eq!(state.search_generation, 1);
    }

    #[tokio::test]
    async fn it_stores_the_query_and_starts_loading_on_a_non_empty_change() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::SearchChanged("corp:cobalt".to_owned()), &db);

      assert_eq!(state.search_query, "corp:cobalt");
      assert!(state.is_filtered());
      assert!(matches!(state.filtered, Some(Filtered::Loading)));
      assert_eq!(state.search_generation, 1);
    }

    #[tokio::test]
    async fn it_toggles_the_help_popover() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::ToggleSearchHelp, &db);
      assert!(state.search_help_open());

      let _ = update(&mut state, Message::ToggleSearchHelp, &db);
      assert!(!state.search_help_open());
    }

    #[test]
    fn the_corp_filtered_body_renders_each_state_without_panicking() {
      let sync = SyncStatus::new();
      let mut state = State::new();
      state.active_pane = Pane::Corporations;

      state.corp_filtered = Some(CorpFiltered::Loading);
      {
        let _loading: Element<'_, Message> = view(&state, &sync);
      }

      state.corp_filtered = Some(CorpFiltered::Loaded(vec![corp_card_from_row(sample_corp_row(
        1, "Cobalt",
      ))]));
      {
        let _loaded: Element<'_, Message> = view(&state, &sync);
      }

      state.corp_filtered = Some(CorpFiltered::Loaded(vec![]));
      {
        let _empty: Element<'_, Message> = view(&state, &sync);
      }

      state.corp_filtered = Some(CorpFiltered::Error("boom".to_owned()));
      {
        let _error: Element<'_, Message> = view(&state, &sync);
      }
    }
  }

  mod search_cards {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
      repo::infra,
    };

    async fn seed_character(db: &Database, id: i64, name: &str) {
      let bloodline = Bloodline::new(1, 90_000_001, 2, 3, "A bloodline.", 4, 5, "Civire", 6, 7);
      let race = Race::new(2, 500_001, "A race.", "Caldari");
      let mut corp = Corporation::new(90_000_001, "Corp One", "CORP1");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let pilot = Character::new(id, 1, 90_000_001, 2, "2003-05-12", Gender::Male, name);
      character::insert_with_org(db, &pilot, &bloodline, &race, &corp, None, None)
        .await
        .unwrap();
      infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();
    }

    async fn seed_corporation(db: &Database, corp_id: i64, ceo_id: i64, name: &str) {
      let alliance = Alliance::new(
        corp_id,
        corp_id,
        ceo_id,
        "2010-01-01T00:00:00Z",
        "Iron Helix Pact",
        "IHP",
      );
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 6, 7);
      let race = Race::new(2, 500_001, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, name, "COBSY");
      corp.set_alliance_id(corp_id);
      corp.set_ceo_id(ceo_id);
      corp.set_creation_date("2019-03-14T00:00:00Z");
      corp.set_creator_id(ceo_id);
      corp.set_member_count(1247);
      corp.set_tax_rate(0.10);
      let ceo = Character::new(ceo_id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Vex Voronova");
      character::insert_with_org(db, &ceo, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
      infra::upsert(db, ceo_id, OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();
      infra::upsert(
        db,
        corp_id,
        OwnerType::Corporation,
        "tok",
        "rt",
        9999,
        Some(ceo_id),
        None,
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_leaves_character_cards_unflagged_when_no_credential_needs_reauth() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Quiet Pilot").await;

      let cards = search_character_cards(&db, "pilot").await.unwrap();

      assert_eq!(cards.len(), 1);
      assert!(!cards[0].needs_reauth);
    }

    #[tokio::test]
    async fn it_leaves_corp_cards_unflagged_when_no_credential_needs_reauth() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 90_100_003, 3, "Cobalt Quiet").await;

      let cards = search_corp_cards(&db, "cobalt").await.unwrap();

      assert_eq!(cards.len(), 1);
      assert!(!cards[0].needs_reauth);
    }

    #[tokio::test]
    async fn it_overlays_the_persisted_reauth_flag_onto_character_results() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Flagged Pilot").await;
      seed_character(&db, 2, "Clean Pilot").await;
      infra::mark_needs_reauth(&db, 1, OwnerType::Character).await.unwrap();

      let cards = search_character_cards(&db, "pilot").await.unwrap();

      let flagged = cards
        .iter()
        .find(|card| card.character_id == 1)
        .expect("the flagged pilot is in the results");
      let clean = cards
        .iter()
        .find(|card| card.character_id == 2)
        .expect("the clean pilot is in the results");
      assert!(flagged.needs_reauth, "the marked credential surfaces a reauth card");
      assert!(!clean.needs_reauth, "the unmarked credential stays clean");
    }

    #[tokio::test]
    async fn it_overlays_the_persisted_reauth_flag_onto_corp_results() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 90_100_001, 1, "Cobalt Flagged").await;
      seed_corporation(&db, 90_100_002, 2, "Cobalt Clean").await;
      infra::mark_needs_reauth(&db, 90_100_001, OwnerType::Corporation)
        .await
        .unwrap();

      let cards = search_corp_cards(&db, "cobalt").await.unwrap();

      let flagged = cards
        .iter()
        .find(|card| card.corporation_id == 90_100_001)
        .expect("the flagged corp is in the results");
      let clean = cards
        .iter()
        .find(|card| card.corporation_id == 90_100_002)
        .expect("the clean corp is in the results");
      assert!(flagged.needs_reauth, "the marked credential surfaces a reauth card");
      assert!(!clean.needs_reauth, "the unmarked credential stays clean");
    }
  }

  mod stale_images {
    use pretty_assertions::assert_eq;

    use super::*;

    fn card(character_id: i64, portrait: images::ImageState) -> CardModel {
      CardModel {
        accent: None,
        character_id,
        corp_ticker: "CORP".to_owned(),
        docked: None,
        location: None,
        name: "Pilot".to_owned(),
        needs_reauth: false,
        portrait,
        position: 0,
        tags: Vec::new(),
        total_sp: None,
        training: None,
        wallet_balance: None,
      }
    }

    fn corp(corporation_id: i64, logo: images::ImageState) -> CorpCardModel {
      CorpCardModel {
        alliance: None,
        alliance_ticker: None,
        ceo: None,
        corporation_id,
        granted_scopes: None,
        hq: None,
        logo,
        members: None,
        name: "Corp".to_owned(),
        needs_reauth: false,
        tags: Vec::new(),
        tax_rate: None,
        ticker: "CORP".to_owned(),
      }
    }

    fn fresh() -> images::ImageState {
      images::ImageState::Fresh(std::path::PathBuf::from("/cache/portrait.jpg"))
    }

    fn stale_logo(id: i64) -> images::ImageState {
      images::ImageState::Stale {
        id,
        kind: images::ImageKind::CorporationLogo,
      }
    }

    fn stale_portrait(id: i64) -> images::ImageState {
      images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      }
    }

    #[test]
    fn it_collects_stale_keys_from_the_filtered_card_and_corp_results() {
      let mut state = State::new();
      state.filtered = Some(Filtered::Loaded(vec![card(1, stale_portrait(1))]));
      state.corp_filtered = Some(CorpFiltered::Loaded(vec![corp(2, stale_logo(2))]));

      let stale = state.stale_images();

      assert_eq!(
        stale,
        vec![
          (images::ImageKind::CharacterPortrait, 1),
          (images::ImageKind::CorporationLogo, 2),
        ]
      );
    }

    #[test]
    fn it_collects_stale_portraits_and_logos_across_groups_unassigned_and_corps() {
      let mut state = State::new();
      state.groups = vec![SquadGroup {
        accent: color::accent::PLASMA,
        cards: vec![card(1, stale_portrait(1))],
        color_hex: None,
        description: None,
        name: "Wing".to_owned(),
        squad_id: 10,
      }];
      state.unassigned = vec![card(2, stale_portrait(2)), card(3, fresh())];
      state.corps = vec![corp(4, stale_logo(4))];

      let stale = state.stale_images();

      assert_eq!(
        stale,
        vec![
          (images::ImageKind::CharacterPortrait, 1),
          (images::ImageKind::CharacterPortrait, 2),
          (images::ImageKind::CorporationLogo, 4),
        ]
      );
    }

    #[test]
    fn it_is_empty_when_every_image_is_fresh() {
      let mut state = State::new();
      state.unassigned = vec![card(1, fresh())];
      state.corps = vec![corp(2, fresh())];

      assert!(state.stale_images().is_empty());
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{Bloodline, Character, Corporation, Gender, OwnerType, Race},
      repo::infra,
    };

    async fn seed_character(db: &Database, id: i64, name: &str) {
      let bloodline = Bloodline::new(1, 90_000_001, 2, 3, "A bloodline.", 4, 5, "Civire", 6, 7);
      let race = Race::new(2, 500_001, "A race.", "Caldari");
      let mut corp = Corporation::new(90_000_001, "Corp One", "CORP1");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let pilot = Character::new(id, 1, 90_000_001, 2, "2003-05-12", Gender::Male, name);
      character::insert_with_org(db, &pilot, &bloodline, &race, &corp, None, None)
        .await
        .unwrap();
      infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();
    }

    async fn reload_tag_names(state: &mut State, db: &Database, character_id: i64) -> Vec<String> {
      let roster = load_roster_at(db, Utc::now(), FeatureFlags::default()).await.unwrap();
      let _ = update(state, Message::CharactersLoaded(Ok(roster)), db);
      unassigned(state)
        .iter()
        .find(|card| card.character_id == character_id)
        .map(|card| card.tags.iter().map(|chip| chip.name.clone()).collect())
        .unwrap_or_default()
    }

    async fn reload_squad_of(state: &mut State, db: &Database, character_id: i64) -> Option<i64> {
      let roster = load_roster_at(db, Utc::now(), FeatureFlags::default()).await.unwrap();
      let _ = update(state, Message::CharactersLoaded(Ok(roster)), db);
      groups(state)
        .iter()
        .find(|group| group.cards.iter().any(|card| card.character_id == character_id))
        .map(|group| group.squad_id)
    }

    async fn stored_position(db: &Database, character_id: i64) -> Option<i64> {
      character::memberships(db)
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.character_id() == character_id)
        .map(|m| m.position())
    }

    async fn reload(state: &mut State, db: &Database) {
      let roster = load_roster_at(db, Utc::now(), FeatureFlags::default()).await.unwrap();
      let _ = update(state, Message::CharactersLoaded(Ok(roster)), db);
    }

    async fn reload_group_names(state: &mut State, db: &Database) -> Vec<String> {
      reload(state, db).await;
      groups(state).iter().map(|group| group.name.clone()).collect()
    }

    async fn reordered_ids(db: &Database, squad_id: i64, index: usize) -> Vec<i64> {
      let mut ids: Vec<i64> = character::all_user_squads(db)
        .await
        .unwrap()
        .iter()
        .map(|s| s.id())
        .collect();
      let from = ids.iter().position(|&id| id == squad_id).unwrap();
      let moved = ids.remove(from);
      ids.insert(index.min(ids.len()), moved);
      ids
    }

    #[tokio::test]
    async fn a_card_hover_is_ignored_during_a_squad_drag_and_vice_versa() {
      let db = store::open_test().await.unwrap();
      let a = character::create(&db, "A", None, None).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;

      let _ = update(&mut state, Message::PickUpSquad(a.id()), &db);
      let _ = update(
        &mut state,
        Message::HoverTarget(DropTarget {
          position: 0,
          squad_id: a.id(),
        }),
        &db,
      );
      assert_eq!(drop_target(&state), None);

      let _ = update(&mut state, Message::CancelDrag, &db);
      let _ = update(&mut state, Message::PickUpCard(1), &db);
      let _ = update(&mut state, Message::HoverSquadSlot(0), &db);
      assert_eq!(squad_drop_target(&state), None);
    }

    #[tokio::test]
    async fn a_failed_squad_write_surfaces_as_a_load_error() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::SquadsChanged(Err("boom".to_owned())), &db);

      assert_eq!(load_error(&state), Some("boom"));
    }

    #[tokio::test]
    async fn a_failed_tag_write_surfaces_as_a_load_error() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::TagsChanged(Err("boom".to_owned())), &db);

      assert_eq!(load_error(&state), Some("boom"));
    }

    #[tokio::test]
    async fn a_right_press_opens_the_context_menu_at_the_cursor_for_the_card() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Right Click Pilot").await;
      let mut state = State::new();
      reload(&mut state, &db).await;

      let _ = update(&mut state, Message::DragMoved(iced::Point::new(120.0, 200.0)), &db);
      let _ = update(&mut state, Message::CardRightPressed(1), &db);

      let menu = state.context_menu.as_ref().expect("menu open");
      assert_eq!(menu.character_id, 1);
      assert_eq!(menu.name, "Right Click Pilot");
      assert_eq!(menu.anchor, iced::Point::new(120.0, 200.0));
    }

    #[tokio::test]
    async fn a_right_press_without_a_tracked_cursor_opens_nothing() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let mut state = State::new();
      reload(&mut state, &db).await;

      let _ = update(&mut state, Message::CardRightPressed(1), &db);

      assert!(state.context_menu.is_none());
    }

    #[tokio::test]
    async fn a_valid_custom_hex_sets_the_color_on_commit() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::OpenSquadCreator, &db);
      let _ = update(&mut state, Message::SquadColorPickerToggled, &db);

      let _ = update(&mut state, Message::SquadColorHexChanged("a1b2c3".to_owned()), &db);
      let creator = state.squad_creator.as_ref().unwrap();
      assert_eq!(creator.hex_draft, "a1b2c3");
      assert_eq!(creator.color, "#3FB8DB");
      assert!(!creator.hex_invalid);

      let _ = update(&mut state, Message::SquadColorHexSubmitted, &db);
      let creator = state.squad_creator.as_ref().unwrap();
      assert_eq!(creator.color, "#A1B2C3");
      assert!(!creator.hex_invalid);
      assert!(creator.color_popover_open);
    }

    #[tokio::test]
    async fn an_invalid_custom_hex_is_rejected_and_flagged() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::OpenSquadCreator, &db);
      let _ = update(&mut state, Message::SquadColorPickerToggled, &db);

      let _ = update(&mut state, Message::SquadColorHexChanged("nothex".to_owned()), &db);
      let _ = update(&mut state, Message::SquadColorHexSubmitted, &db);

      let creator = state.squad_creator.as_ref().unwrap();
      assert_eq!(creator.color, "#3FB8DB");
      assert!(creator.hex_invalid);
    }

    #[tokio::test]
    async fn assigning_to_a_new_squad_moves_the_character_out_of_the_old_one() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let from = character::create(&db, "From", None, None).await.unwrap();
      let to = character::create(&db, "To", None, None).await.unwrap();
      character::assign(&db, 1, from.id(), 0).await.unwrap();
      let mut state = State::new();

      apply_squad_write(
        &db,
        SquadWrite::Assign {
          character_id: 1,
          position: 0,
          squad_id: to.id(),
        },
      )
      .await
      .unwrap();

      assert_eq!(reload_squad_of(&mut state, &db, 1).await, Some(to.id()));
      assert!(character::members(&db, from.id()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn cancelling_the_confirm_modal_clears_it_without_removing() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(10.0, 10.0)), &db);
      let _ = update(&mut state, Message::CardRightPressed(1), &db);
      let _ = update(&mut state, Message::OpenRemoveConfirm(1), &db);

      let _ = update(&mut state, Message::CloseRemoveConfirm, &db);

      assert!(state.remove_confirm.is_none());
      assert!(character::get(&db, 1).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn choosing_remove_transitions_the_menu_to_the_confirm_modal() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Doomed Pilot").await;
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(10.0, 10.0)), &db);
      let _ = update(&mut state, Message::CardRightPressed(1), &db);

      let _ = update(&mut state, Message::OpenRemoveConfirm(1), &db);

      assert!(state.context_menu.is_none());
      let confirm = state.remove_confirm.as_ref().expect("confirm open");
      assert_eq!(confirm.character_id, 1);
      assert_eq!(confirm.name, "Doomed Pilot");
    }

    #[tokio::test]
    async fn closing_the_context_menu_clears_it() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(10.0, 10.0)), &db);
      let _ = update(&mut state, Message::CardRightPressed(1), &db);
      assert!(state.context_menu.is_some());

      let _ = update(&mut state, Message::CloseContextMenu, &db);

      assert!(state.context_menu.is_none());
    }

    #[tokio::test]
    async fn closing_the_squad_menu_clears_it() {
      let db = store::open_test().await.unwrap();
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(10.0, 10.0)), &db);
      let _ = update(&mut state, Message::OpenSquadMenu(squad.id()), &db);
      assert!(state.squad_menu.is_some());

      let _ = update(&mut state, Message::CloseSquadMenu, &db);

      assert!(state.squad_menu.is_none());
    }

    #[tokio::test]
    async fn confirming_remove_closes_the_modal_and_deletes_the_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Doomed").await;
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(10.0, 10.0)), &db);
      let _ = update(&mut state, Message::CardRightPressed(1), &db);
      let _ = update(&mut state, Message::OpenRemoveConfirm(1), &db);

      let _ = update(&mut state, Message::RemoveCharacterConfirmed(1), &db);
      assert!(state.remove_confirm.is_none());
      character::delete(&db, 1).await.unwrap();
      reload(&mut state, &db).await;

      assert!(character::get(&db, 1).await.unwrap().is_none());
      assert!(card_for(&state, 1).is_none());
    }

    #[tokio::test]
    async fn copy_name_closes_the_menu() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(10.0, 10.0)), &db);
      let _ = update(&mut state, Message::CardRightPressed(1), &db);

      let _ = update(&mut state, Message::CopyCharacterName("Pilot".to_owned()), &db);

      assert!(state.context_menu.is_none());
    }

    #[tokio::test]
    async fn create_and_assign_with_blank_input_is_a_noop() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(
        &mut state,
        Message::OpenAddTagModal {
          entity_id: 1,
          entity_type: ENTITY_TYPE_CHARACTER,
        },
        &db,
      );
      let _ = update(&mut state, Message::AddTagInputChanged("   ".to_owned()), &db);

      let _ = update(&mut state, Message::CreateAndAssignTag, &db);

      assert!(infra::tag_all(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn deleting_a_squad_from_the_menu_closes_it_and_drops_pilots_to_unassigned() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let squad = character::create(&db, "Doomed", None, None).await.unwrap();
      character::assign(&db, 1, squad.id(), 0).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(10.0, 10.0)), &db);
      let _ = update(&mut state, Message::OpenSquadMenu(squad.id()), &db);

      let _ = update(&mut state, Message::DeleteSquad(squad.id()), &db);
      assert!(state.squad_menu.is_none());
      apply_squad_write(
        &db,
        SquadWrite::Delete {
          squad_id: squad.id(),
        },
      )
      .await
      .unwrap();

      assert!(character::get_squad(&db, squad.id()).await.unwrap().is_none());
      assert_eq!(reload_squad_of(&mut state, &db, 1).await, None);
      assert!(card_for(&state, 1).is_some());
    }

    #[tokio::test]
    async fn drag_moved_tracks_the_cursor_drag_or_not() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::DragMoved(iced::Point::new(10.0, 20.0)), &db);
      assert_eq!(cursor(&state), Some(iced::Point::new(10.0, 20.0)));

      let _ = update(&mut state, Message::PickUpCard(1), &db);
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(30.0, 40.0)), &db);
      assert_eq!(cursor(&state), Some(iced::Point::new(30.0, 40.0)));
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(55.0, 66.0)), &db);
      assert_eq!(cursor(&state), Some(iced::Point::new(55.0, 66.0)));
    }

    #[tokio::test]
    async fn dropping_a_dragged_squad_reorders_it_to_the_hovered_index() {
      let db = store::open_test().await.unwrap();
      let a = character::create(&db, "A", None, None).await.unwrap();
      character::create(&db, "B", None, None).await.unwrap();
      character::create(&db, "C", None, None).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;

      let _ = update(&mut state, Message::PickUpSquad(a.id()), &db);
      let _ = update(&mut state, Message::HoverSquadSlot(2), &db);
      let _ = update(&mut state, Message::DropDragged, &db);

      assert_eq!(dragging_squad(&state), None);
      assert_eq!(squad_drop_target(&state), None);
      apply_squad_write(
        &db,
        SquadWrite::Reorder {
          ordered: reordered_ids(&db, a.id(), 2).await,
        },
      )
      .await
      .unwrap();

      assert_eq!(reload_group_names(&mut state, &db).await, ["B", "C", "A"]);
    }

    #[tokio::test]
    async fn dropping_a_squad_over_no_target_cancels_without_reordering() {
      let db = store::open_test().await.unwrap();
      let a = character::create(&db, "A", None, None).await.unwrap();
      character::create(&db, "B", None, None).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;

      let _ = update(&mut state, Message::PickUpSquad(a.id()), &db);
      assert_eq!(dragging_squad(&state), Some(a.id()));
      let _ = update(&mut state, Message::DropDragged, &db);

      assert_eq!(dragging_squad(&state), None);
      assert_eq!(squad_drop_target(&state), None);
      assert_eq!(reload_group_names(&mut state, &db).await, ["A", "B"]);
    }

    #[tokio::test]
    async fn grabbing_a_card_preserves_the_main_roster_scroll_offset() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(
        &mut state,
        Message::RosterScrolled(GridViewport::at_offset(4_200.0)),
        &db,
      );

      let _ = update(&mut state, Message::PickUpCard(1), &db);

      assert_eq!(
        state.roster_scroll_offset, 4_200.0,
        "grabbing a card must hold the roster scroll position, not snap it to the top"
      );
    }

    #[tokio::test]
    async fn grabbing_a_card_preserves_the_filtered_roster_scroll_offset() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let mut state = State::new();
      reload(&mut state, &db).await;
      state.filtered = Some(Filtered::Loaded(Vec::new()));
      let _ = update(
        &mut state,
        Message::FilteredScrolled(GridViewport::at_offset(3_100.0)),
        &db,
      );

      let _ = update(&mut state, Message::PickUpCard(1), &db);

      assert_eq!(
        state.filtered_scroll_offset, 3_100.0,
        "grabbing a card in the filtered grid must hold its scroll position"
      );
    }

    #[tokio::test]
    async fn grabbing_a_squad_preserves_the_main_roster_scroll_offset() {
      let db = store::open_test().await.unwrap();
      let a = character::create(&db, "A", None, None).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(
        &mut state,
        Message::RosterScrolled(GridViewport::at_offset(2_500.0)),
        &db,
      );

      let _ = update(&mut state, Message::PickUpSquad(a.id()), &db);

      assert_eq!(dragging_squad(&state), Some(a.id()));
      assert_eq!(
        state.roster_scroll_offset, 2_500.0,
        "grabbing a squad must hold the roster scroll position, not snap it to the top"
      );
    }

    #[tokio::test]
    async fn the_corporations_grid_persists_its_scroll_offset() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::CorpScrolled(GridViewport::at_offset(1_750.0)), &db);

      assert_eq!(state.corp_scroll_offset, 1_750.0);
    }

    #[tokio::test]
    async fn dropping_at_a_higher_slot_leaves_a_sparse_gap() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::PickUpCard(1), &db);
      let _ = update(
        &mut state,
        Message::HoverTarget(DropTarget {
          position: 4,
          squad_id: squad.id(),
        }),
        &db,
      );
      let _ = update(&mut state, Message::DropDragged, &db);
      apply_squad_write(
        &db,
        SquadWrite::Assign {
          character_id: 1,
          position: 4,
          squad_id: squad.id(),
        },
      )
      .await
      .unwrap();

      assert_eq!(stored_position(&db, 1).await, Some(4));
      assert_eq!(reload_squad_of(&mut state, &db, 1).await, Some(squad.id()));
      let card = groups(&state)
        .iter()
        .flat_map(|group| group.cards.iter())
        .find(|card| card.character_id == 1)
        .unwrap();
      assert_eq!(card.position, 4);
    }

    #[tokio::test]
    async fn dropping_over_a_hovered_squad_lands_the_card_in_it() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::PickUpCard(1), &db);
      let _ = update(
        &mut state,
        Message::HoverTarget(DropTarget {
          position: 0,
          squad_id: squad.id(),
        }),
        &db,
      );
      let _ = update(&mut state, Message::DropDragged, &db);
      assert_eq!(dragging_card(&state), None);
      assert_eq!(drop_target(&state), None);
      apply_squad_write(
        &db,
        SquadWrite::Assign {
          character_id: 1,
          position: 0,
          squad_id: squad.id(),
        },
      )
      .await
      .unwrap();

      assert_eq!(reload_squad_of(&mut state, &db, 1).await, Some(squad.id()));
    }

    #[tokio::test]
    async fn dropping_over_no_target_cancels_with_no_change() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      character::assign(&db, 1, squad.id(), 0).await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::PickUpCard(1), &db);
      assert_eq!(dragging_card(&state), Some(1));
      let _ = update(&mut state, Message::DropDragged, &db);

      assert_eq!(dragging_card(&state), None);
      assert_eq!(drop_target(&state), None);
      assert_eq!(reload_squad_of(&mut state, &db, 1).await, Some(squad.id()));
    }

    #[tokio::test]
    async fn dropping_over_the_unassigned_bucket_unassigns_the_card() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      character::assign(&db, 1, squad.id(), 0).await.unwrap();
      let reserved = character::get_or_create_unassigned(&db).await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::PickUpCard(1), &db);
      let _ = update(
        &mut state,
        Message::HoverTarget(DropTarget {
          position: 2,
          squad_id: reserved.id(),
        }),
        &db,
      );
      let _ = update(&mut state, Message::DropDragged, &db);
      assert_eq!(dragging_card(&state), None);
      apply_squad_write(
        &db,
        SquadWrite::Assign {
          character_id: 1,
          position: 2,
          squad_id: reserved.id(),
        },
      )
      .await
      .unwrap();

      assert_eq!(reload_squad_of(&mut state, &db, 1).await, None);
      assert!(character::members(&db, squad.id()).await.unwrap().is_empty());
      assert_eq!(stored_position(&db, 1).await, Some(2));
    }

    #[tokio::test]
    async fn dropping_with_no_drag_in_progress_is_a_noop() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      character::assign(&db, 1, squad.id(), 0).await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::DropDragged, &db);

      assert_eq!(dragging_card(&state), None);
      assert_eq!(reload_squad_of(&mut state, &db, 1).await, Some(squad.id()));
    }

    #[tokio::test]
    async fn edit_tags_opens_the_add_tag_modal_and_closes_the_menu() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(10.0, 10.0)), &db);
      let _ = update(&mut state, Message::CardRightPressed(1), &db);

      let _ = update(
        &mut state,
        Message::OpenAddTagModal {
          entity_id: 1,
          entity_type: ENTITY_TYPE_CHARACTER,
        },
        &db,
      );

      assert!(state.context_menu.is_none());
      assert_eq!(state.add_tag_modal.as_ref().map(|modal| modal.entity_id), Some(1));
      assert_eq!(
        state.add_tag_modal.as_ref().map(|modal| modal.entity_type),
        Some(ENTITY_TYPE_CHARACTER)
      );
    }

    #[tokio::test]
    async fn editing_a_squad_from_the_menu_seeds_the_creator_and_closes_the_menu() {
      let db = store::open_test().await.unwrap();
      let squad = character::create(&db, "Supers", Some("Cap fleet"), Some("#5BB97E"))
        .await
        .unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(10.0, 10.0)), &db);
      let _ = update(&mut state, Message::OpenSquadMenu(squad.id()), &db);

      let _ = update(&mut state, Message::OpenSquadEditor(squad.id()), &db);

      assert!(state.squad_menu.is_none());
      let creator = state.squad_creator.as_ref().expect("creator open in edit mode");
      assert_eq!(creator.editing, Some(squad.id()));
      assert_eq!(creator.name, "Supers");
      assert_eq!(creator.description, "Cap fleet");
      assert_eq!(creator.color, "#5BB97E");
      assert_eq!(creator.hex_draft, "#5BB97E");
    }

    #[tokio::test]
    async fn ending_a_drag_leaves_the_tracked_cursor_for_the_next_right_click() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::PickUpCard(1), &db);
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(30.0, 40.0)), &db);
      assert_eq!(cursor(&state), Some(iced::Point::new(30.0, 40.0)));
      let _ = update(&mut state, Message::CancelDrag, &db);
      assert_eq!(dragging_card(&state), None);
      assert_eq!(cursor(&state), Some(iced::Point::new(30.0, 40.0)));

      let _ = update(&mut state, Message::PickUpCard(2), &db);
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(12.0, 34.0)), &db);
      let _ = update(
        &mut state,
        Message::AssignToSquad {
          character_id: 2,
          position: 0,
          squad_id: 7,
        },
        &db,
      );
      assert_eq!(cursor(&state), Some(iced::Point::new(12.0, 34.0)));
    }

    #[tokio::test]
    async fn hover_and_cancel_track_the_drag_gesture() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let slot = |squad_id, position| DropTarget {
        position,
        squad_id,
      };

      let _ = update(&mut state, Message::HoverTarget(slot(5, 0)), &db);
      assert_eq!(drop_target(&state), None);

      let _ = update(&mut state, Message::PickUpCard(1), &db);
      let _ = update(&mut state, Message::HoverTarget(slot(5, 0)), &db);
      assert_eq!(drop_target(&state), Some(slot(5, 0)));

      let _ = update(&mut state, Message::LeaveTarget(slot(5, 1)), &db);
      assert_eq!(drop_target(&state), Some(slot(5, 0)));

      let _ = update(&mut state, Message::LeaveTarget(slot(5, 0)), &db);
      assert_eq!(drop_target(&state), None);
      let _ = update(&mut state, Message::CancelDrag, &db);
      assert_eq!(dragging_card(&state), None);
    }

    #[tokio::test]
    async fn hovering_a_squad_index_during_a_squad_drag_records_the_target() {
      let db = store::open_test().await.unwrap();
      let a = character::create(&db, "A", None, None).await.unwrap();
      character::create(&db, "B", None, None).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;

      let _ = update(&mut state, Message::HoverSquadSlot(1), &db);
      assert_eq!(squad_drop_target(&state), None);

      let _ = update(&mut state, Message::PickUpSquad(a.id()), &db);
      let _ = update(&mut state, Message::HoverSquadSlot(1), &db);
      assert_eq!(squad_drop_target(&state), Some(1));

      let _ = update(&mut state, Message::LeaveSquadSlot(0), &db);
      assert_eq!(squad_drop_target(&state), Some(1));
      let _ = update(&mut state, Message::LeaveSquadSlot(1), &db);
      assert_eq!(squad_drop_target(&state), None);
    }

    #[tokio::test]
    async fn it_assigns_a_dragged_card_to_a_squad() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::PickUpCard(1), &db);
      assert_eq!(dragging_card(&state), Some(1));
      let _ = update(
        &mut state,
        Message::AssignToSquad {
          character_id: 1,
          position: 0,
          squad_id: squad.id(),
        },
        &db,
      );
      assert_eq!(dragging_card(&state), None);
      apply_squad_write(
        &db,
        SquadWrite::Assign {
          character_id: 1,
          position: 0,
          squad_id: squad.id(),
        },
      )
      .await
      .unwrap();

      assert_eq!(reload_squad_of(&mut state, &db, 1).await, Some(squad.id()));
    }

    #[tokio::test]
    async fn it_assigns_an_existing_tag_without_replacing_others() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let main = infra::create(&db, "Main", None, None).await.unwrap();
      let alt = infra::create(&db, "Alt", None, None).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_CHARACTER, 1, main.id()).await.unwrap();
      let mut state = State::new();

      apply_tag_write(
        &db,
        TagWrite::Assign {
          entity_id: 1,
          entity_type: ENTITY_TYPE_CHARACTER,
          tag_id: alt.id(),
        },
      )
      .await
      .unwrap();
      let names = reload_tag_names(&mut state, &db, 1).await;

      assert_eq!(names, vec!["Main".to_owned(), "Alt".to_owned()]);
    }

    #[tokio::test]
    async fn it_assigns_then_unassigns_a_tag_from_a_corporation_card() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let tag = infra::create(&db, "Industry", None, None).await.unwrap();
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::OpenAddTagModal {
          entity_id: 90_000_001,
          entity_type: ENTITY_TYPE_CORPORATION,
        },
        &db,
      );
      let modal = state.add_tag_modal.as_ref().expect("the corp add-tag modal is open");
      assert_eq!(modal.entity_id, 90_000_001);
      assert_eq!(modal.entity_type, ENTITY_TYPE_CORPORATION);

      let _ = update(
        &mut state,
        Message::AssignTag {
          entity_id: 90_000_001,
          entity_type: ENTITY_TYPE_CORPORATION,
          tag_id: tag.id(),
        },
        &db,
      );
      apply_tag_write(
        &db,
        TagWrite::Assign {
          entity_id: 90_000_001,
          entity_type: ENTITY_TYPE_CORPORATION,
          tag_id: tag.id(),
        },
      )
      .await
      .unwrap();
      let assigned = infra::memberships(&db, ENTITY_TYPE_CORPORATION).await.unwrap();
      assert!(
        assigned
          .iter()
          .any(|membership| membership.entity_id() == 90_000_001 && membership.tag_id() == tag.id())
      );

      let _ = update(
        &mut state,
        Message::UnassignTag {
          entity_id: 90_000_001,
          entity_type: ENTITY_TYPE_CORPORATION,
          tag_id: tag.id(),
        },
        &db,
      );
      apply_tag_write(
        &db,
        TagWrite::Unassign {
          entity_id: 90_000_001,
          entity_type: ENTITY_TYPE_CORPORATION,
          tag_id: tag.id(),
        },
      )
      .await
      .unwrap();
      let remaining = infra::memberships(&db, ENTITY_TYPE_CORPORATION).await.unwrap();

      assert!(
        !remaining
          .iter()
          .any(|membership| membership.entity_id() == 90_000_001 && membership.tag_id() == tag.id())
      );
    }

    #[tokio::test]
    async fn it_clears_the_creator_on_close() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::OpenSquadCreator, &db);
      let _ = update(&mut state, Message::SquadCreatorNameChanged("Draft".to_owned()), &db);

      let _ = update(&mut state, Message::CloseSquadCreator, &db);

      assert!(state.squad_creator.is_none());
    }

    #[tokio::test]
    async fn it_closes_the_add_tag_modal_when_an_existing_tag_is_assigned() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let main = infra::create(&db, "Main", None, None).await.unwrap();
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::OpenAddTagModal {
          entity_id: 1,
          entity_type: ENTITY_TYPE_CHARACTER,
        },
        &db,
      );
      let _ = update(
        &mut state,
        Message::AssignTag {
          entity_id: 1,
          entity_type: ENTITY_TYPE_CHARACTER,
          tag_id: main.id(),
        },
        &db,
      );

      assert!(state.add_tag_modal.is_none());
    }

    #[tokio::test]
    async fn it_creates_a_squad_from_the_creator_with_description_and_color() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::OpenSquadCreator, &db);
      let _ = update(
        &mut state,
        Message::SquadCreatorNameChanged("  Supers  ".to_owned()),
        &db,
      );
      let _ = update(
        &mut state,
        Message::SquadCreatorDescriptionChanged("  Cap fleet  ".to_owned()),
        &db,
      );
      let _ = update(&mut state, Message::SquadColorSelected("#3FB8DB".to_owned()), &db);
      let _ = update(&mut state, Message::CreateSquad, &db);
      assert!(state.squad_creator.is_none());

      apply_squad_write(
        &db,
        SquadWrite::Create {
          color: Some("#3FB8DB".to_owned()),
          description: Some("Cap fleet".to_owned()),
          name: "Supers".to_owned(),
        },
      )
      .await
      .unwrap();

      let squads = character::all_squads(&db).await.unwrap();
      assert_eq!(squads.len(), 1);
      assert_eq!(squads[0].name(), "Supers");
      assert_eq!(squads[0].description().as_deref(), Some("Cap fleet"));
      assert_eq!(squads[0].color().as_deref(), Some("#3FB8DB"));
    }

    #[tokio::test]
    async fn it_creates_then_assigns_a_new_tag() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::OpenAddTagModal {
          entity_id: 1,
          entity_type: ENTITY_TYPE_CHARACTER,
        },
        &db,
      );
      let _ = update(&mut state, Message::AddTagInputChanged("Hauler".to_owned()), &db);
      let name = state.add_tag_modal.as_ref().unwrap().input.clone();
      let _ = update(&mut state, Message::CreateAndAssignTag, &db);
      assert!(state.add_tag_modal.is_none());
      apply_tag_write(
        &db,
        TagWrite::CreateAndAssign {
          entity_id: 1,
          entity_type: ENTITY_TYPE_CHARACTER,
          name,
        },
      )
      .await
      .unwrap();
      let names = reload_tag_names(&mut state, &db, 1).await;

      assert_eq!(names, vec!["Hauler".to_owned()]);
      assert!(all_tags(&state).iter().any(|tag| tag.name() == "Hauler"));
    }

    #[tokio::test]
    async fn it_edits_the_open_creator_name_and_description() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::OpenSquadCreator, &db);

      let _ = update(&mut state, Message::SquadCreatorNameChanged("Supers".to_owned()), &db);
      let _ = update(
        &mut state,
        Message::SquadCreatorDescriptionChanged("Cap fleet".to_owned()),
        &db,
      );

      let creator = state.squad_creator.as_ref().expect("creator open");
      assert_eq!(creator.name, "Supers");
      assert_eq!(creator.description, "Cap fleet");
    }

    #[tokio::test]
    async fn it_is_a_noop_when_creating_a_squad_with_a_blank_name() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::OpenSquadCreator, &db);
      let _ = update(&mut state, Message::SquadCreatorNameChanged("   ".to_owned()), &db);

      let _ = update(&mut state, Message::CreateSquad, &db);

      assert!(character::all_squads(&db).await.unwrap().is_empty());
      assert!(state.squad_creator.is_some());
    }

    #[tokio::test]
    async fn it_offers_only_tags_not_already_on_the_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let main = infra::create(&db, "Main", None, None).await.unwrap();
      let alt = infra::create(&db, "Alt", None, None).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_CHARACTER, 1, main.id()).await.unwrap();
      let mut state = State::new();
      let roster = load_roster_at(&db, Utc::now(), FeatureFlags::default()).await.unwrap();
      let _ = update(&mut state, Message::CharactersLoaded(Ok(roster)), &db);

      let (name, assigned, assignable) = resolve_add_tag_modal(&state, ENTITY_TYPE_CHARACTER, 1);
      assert_eq!(name, "Pilot");
      assert_eq!(assigned.iter().map(|tag| tag.id()).collect::<Vec<_>>(), vec![main.id()]);
      assert_eq!(
        assignable.iter().map(|tag| tag.id()).collect::<Vec<_>>(),
        vec![alt.id()]
      );
    }

    #[tokio::test]
    async fn it_opens_a_squad_creator_seeded_to_plasma() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::OpenSquadCreator, &db);

      let creator = state.squad_creator.as_ref().expect("creator open");
      assert!(creator.name.is_empty());
      assert!(creator.description.is_empty());
      assert_eq!(creator.color, "#3FB8DB");
      assert_eq!(creator.hex_draft, "#3FB8DB");
      assert!(!creator.color_popover_open);
      assert!(!creator.hex_invalid);
    }

    #[tokio::test]
    async fn it_persists_a_blank_description_as_none_and_the_seeded_color() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::OpenSquadCreator, &db);
      let _ = update(&mut state, Message::SquadCreatorNameChanged("Solo".to_owned()), &db);
      let _ = update(
        &mut state,
        Message::SquadCreatorDescriptionChanged("  ".to_owned()),
        &db,
      );

      let _ = update(&mut state, Message::CreateSquad, &db);
      apply_squad_write(
        &db,
        SquadWrite::Create {
          color: non_blank("#3FB8DB"),
          description: non_blank("  "),
          name: "Solo".to_owned(),
        },
      )
      .await
      .unwrap();

      let squads = character::all_squads(&db).await.unwrap();
      assert_eq!(squads[0].description(), &None);
      assert_eq!(squads[0].color().as_deref(), Some("#3FB8DB"));
    }

    #[tokio::test]
    async fn it_persists_the_collapsed_set_across_a_reload() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      character::assign(&db, 1, squad.id(), 0).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;

      let _ = update(&mut state, Message::ToggleSquadCollapse(squad.id()), &db);
      assert!(is_squad_collapsed(&state, squad.id()));
      reload(&mut state, &db).await;
      assert!(is_squad_collapsed(&state, squad.id()));
    }

    #[test]
    fn it_renders_an_element_in_both_panes() {
      let sync = SyncStatus::new();
      let mut state = State::new();

      {
        let _characters: Element<'_, Message> = view(&state, &sync);
      }
      state.active_pane = Pane::Corporations;
      let _corporations: Element<'_, Message> = view(&state, &sync);
    }

    #[tokio::test]
    async fn it_renders_the_add_tag_modal_over_the_backdrop_when_open() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      infra::create(&db, "Hauler", None, None).await.unwrap();
      let sync = SyncStatus::new();
      let mut state = State::new();
      let roster = load_roster_at(&db, Utc::now(), FeatureFlags::default()).await.unwrap();
      let _ = update(&mut state, Message::CharactersLoaded(Ok(roster)), &db);

      {
        let _closed: Element<'_, Message> = view(&state, &sync);
      }

      let _ = update(
        &mut state,
        Message::OpenAddTagModal {
          entity_id: 1,
          entity_type: ENTITY_TYPE_CHARACTER,
        },
        &db,
      );
      {
        let _open: Element<'_, Message> = view(&state, &sync);
      }
      let _ = update(&mut state, Message::AddTagInputChanged("Logi".to_owned()), &db);
      let _typed: Element<'_, Message> = view(&state, &sync);
    }

    #[tokio::test]
    async fn it_renders_the_confirm_modal_over_the_backdrop_when_open() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let sync = SyncStatus::new();
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(40.0, 60.0)), &db);
      let _ = update(&mut state, Message::CardRightPressed(1), &db);

      let _ = update(&mut state, Message::OpenRemoveConfirm(1), &db);
      let _open: Element<'_, Message> = view(&state, &sync);
    }

    #[tokio::test]
    async fn it_renders_the_context_menu_over_the_backdrop_when_open() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let sync = SyncStatus::new();
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(40.0, 60.0)), &db);

      {
        let _closed: Element<'_, Message> = view(&state, &sync);
      }

      let _ = update(&mut state, Message::CardRightPressed(1), &db);
      let _open: Element<'_, Message> = view(&state, &sync);
    }

    #[test]
    fn it_renders_the_modal_with_the_color_swatch_and_open_popover() {
      let sync = SyncStatus::new();
      let mut state = State::new();
      {
        let _closed: Element<'_, Message> = view(&state, &sync);
      }

      state.squad_creator = Some(SquadCreator {
        name: "Supers".to_owned(),
        ..SquadCreator::default()
      });
      {
        let _swatch_only: Element<'_, Message> = view(&state, &sync);
      }

      state.squad_creator = Some(SquadCreator {
        name: "Supers".to_owned(),
        color_popover_open: true,
        ..SquadCreator::default()
      });
      let _open: Element<'_, Message> = view(&state, &sync);
    }

    #[tokio::test]
    async fn it_renders_the_squad_menu_over_the_backdrop_when_open() {
      let db = store::open_test().await.unwrap();
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      let sync = SyncStatus::new();
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(40.0, 60.0)), &db);

      let _ = update(&mut state, Message::OpenSquadMenu(squad.id()), &db);
      let _open: Element<'_, Message> = view(&state, &sync);
    }

    #[tokio::test]
    async fn it_switches_the_active_pane_on_tab_selected() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      assert_eq!(state.active_pane, Pane::Characters);

      let _ = update(&mut state, Message::TabSelected(Pane::Corporations), &db);
      assert_eq!(state.active_pane, Pane::Corporations);

      let _ = update(&mut state, Message::TabSelected(Pane::Characters), &db);
      assert_eq!(state.active_pane, Pane::Characters);
    }

    #[tokio::test]
    async fn it_toggles_a_squad_collapse_adding_then_removing_its_id() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      assert!(!is_squad_collapsed(&state, 5));
      let _ = update(&mut state, Message::ToggleSquadCollapse(5), &db);
      assert!(is_squad_collapsed(&state, 5));
      let _ = update(&mut state, Message::ToggleSquadCollapse(5), &db);
      assert!(!is_squad_collapsed(&state, 5));
    }

    #[tokio::test]
    async fn it_unassigns_a_dragged_card_dropped_on_the_unassigned_bucket() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      character::assign(&db, 1, squad.id(), 0).await.unwrap();
      let reserved = character::get_or_create_unassigned(&db).await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::PickUpCard(1), &db);
      let _ = update(
        &mut state,
        Message::AssignToSquad {
          character_id: 1,
          position: 0,
          squad_id: reserved.id(),
        },
        &db,
      );
      assert_eq!(dragging_card(&state), None);
      apply_squad_write(
        &db,
        SquadWrite::Assign {
          character_id: 1,
          position: 0,
          squad_id: reserved.id(),
        },
      )
      .await
      .unwrap();

      assert_eq!(reload_squad_of(&mut state, &db, 1).await, None);
      assert!(character::members(&db, squad.id()).await.unwrap().is_empty());
      assert_eq!(character::members(&db, reserved.id()).await.unwrap(), vec![1]);
      assert_eq!(stored_position(&db, 1).await, Some(0));
    }

    #[tokio::test]
    async fn it_unassigns_a_tag_leaving_the_rest() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let main = infra::create(&db, "Main", None, None).await.unwrap();
      let dropped = infra::create(&db, "Dropped", None, None).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_CHARACTER, 1, main.id()).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_CHARACTER, 1, dropped.id())
        .await
        .unwrap();
      let mut state = State::new();

      apply_tag_write(
        &db,
        TagWrite::Unassign {
          entity_id: 1,
          entity_type: ENTITY_TYPE_CHARACTER,
          tag_id: dropped.id(),
        },
      )
      .await
      .unwrap();
      let names = reload_tag_names(&mut state, &db, 1).await;

      assert_eq!(names, vec!["Main".to_owned()]);
    }

    #[tokio::test]
    async fn open_then_input_then_close_tracks_the_add_tag_modal() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::OpenAddTagModal {
          entity_id: 7,
          entity_type: ENTITY_TYPE_CHARACTER,
        },
        &db,
      );
      let _ = update(&mut state, Message::AddTagInputChanged("Scout".to_owned()), &db);
      let modal = state.add_tag_modal.as_ref().expect("modal open");
      assert_eq!(modal.entity_id, 7);
      assert_eq!(modal.entity_type, ENTITY_TYPE_CHARACTER);
      assert_eq!(modal.input, "Scout");

      let _ = update(&mut state, Message::CloseAddTagModal, &db);
      assert!(state.add_tag_modal.is_none());
    }

    #[tokio::test]
    async fn opening_the_squad_kebab_menu_anchors_at_the_cursor_with_the_squad_state() {
      let db = store::open_test().await.unwrap();
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(80.0, 120.0)), &db);

      let _ = update(&mut state, Message::OpenSquadMenu(squad.id()), &db);

      let menu = state.squad_menu.as_ref().expect("squad menu open");
      assert_eq!(menu.squad_id, squad.id());
      assert_eq!(menu.name, "Supers");
      assert_eq!(menu.anchor, iced::Point::new(80.0, 120.0));
      assert!(menu.is_empty);
      assert!(!menu.collapsed);
    }

    #[tokio::test]
    async fn opening_the_squad_menu_without_a_tracked_cursor_opens_nothing() {
      let db = store::open_test().await.unwrap();
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;

      let _ = update(&mut state, Message::OpenSquadMenu(squad.id()), &db);

      assert!(state.squad_menu.is_none());
    }

    #[tokio::test]
    async fn picking_up_a_user_squad_starts_a_squad_drag() {
      let db = store::open_test().await.unwrap();
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;

      let _ = update(&mut state, Message::PickUpSquad(squad.id()), &db);

      assert_eq!(dragging_squad(&state), Some(squad.id()));
      assert_eq!(dragging_card(&state), None);
    }

    #[tokio::test]
    async fn picking_up_the_reserved_squad_is_refused() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      character::unassign(&db, 1).await.unwrap();
      let reserved = character::get_or_create_unassigned(&db).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;

      let _ = update(&mut state, Message::PickUpSquad(reserved.id()), &db);

      assert_eq!(dragging_squad(&state), None);
    }

    #[tokio::test]
    async fn remove_character_drops_the_db_row_but_leaves_the_portrait_on_disk() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 424_242, "Removed Pilot").await;
      let dir = tempfile::tempdir().unwrap();
      let images = images::Store::new(dir.path().to_path_buf());
      let portrait = images.character_portrait_path(424_242);
      images.write(&portrait, &[1]).unwrap();

      remove_character(db.clone(), 424_242).await.unwrap();

      assert!(
        character::get(&db, 424_242).await.unwrap().is_none(),
        "the db row is removed"
      );
      assert!(
        portrait.exists(),
        "user-data images are never deleted from disk, only overwritten"
      );
    }

    #[tokio::test]
    async fn remove_corporation_drops_the_db_row_but_leaves_the_logo_on_disk() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 8242, "Director").await;
      infra::upsert(
        &db,
        525_252,
        OwnerType::Corporation,
        "tok",
        "rt",
        9999,
        Some(8242),
        None,
      )
      .await
      .unwrap();
      let dir = tempfile::tempdir().unwrap();
      let images = images::Store::new(dir.path().to_path_buf());
      let logo = images.corporation_logo_path(525_252);
      images.write(&logo, &[1]).unwrap();

      remove_corporation(db.clone(), 525_252).await.unwrap();

      assert!(
        infra::get(&db, 525_252, OwnerType::Corporation)
          .await
          .unwrap()
          .is_none(),
        "the db row is removed"
      );
      assert!(
        logo.exists(),
        "user-data images are never deleted from disk, only overwritten"
      );
    }

    #[tokio::test]
    async fn selecting_a_preset_sets_the_color_and_closes_the_popover() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::OpenSquadCreator, &db);
      let _ = update(&mut state, Message::SquadColorPickerToggled, &db);

      let _ = update(&mut state, Message::SquadColorSelected("#5BB97E".to_owned()), &db);

      let creator = state.squad_creator.as_ref().expect("creator open");
      assert_eq!(creator.color, "#5BB97E");
      assert_eq!(creator.hex_draft, "#5BB97E");
      assert!(!creator.color_popover_open);
    }

    #[tokio::test]
    async fn submitting_an_edited_squad_updates_it_in_place() {
      let db = store::open_test().await.unwrap();
      let squad = character::create(&db, "Old", None, Some("#3FB8DB")).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(10.0, 10.0)), &db);
      let _ = update(&mut state, Message::OpenSquadMenu(squad.id()), &db);
      let _ = update(&mut state, Message::OpenSquadEditor(squad.id()), &db);
      let _ = update(&mut state, Message::SquadCreatorNameChanged("New".to_owned()), &db);

      let _ = update(&mut state, Message::CreateSquad, &db);
      assert!(state.squad_creator.is_none());
      apply_squad_write(
        &db,
        SquadWrite::Update {
          color: Some("#3FB8DB".to_owned()),
          description: None,
          name: "New".to_owned(),
          squad_id: squad.id(),
        },
      )
      .await
      .unwrap();

      assert_eq!(character::all_user_squads(&db).await.unwrap().len(), 1);
      assert_eq!(
        character::get_squad(&db, squad.id()).await.unwrap().unwrap().name(),
        "New"
      );
    }

    #[tokio::test]
    async fn the_color_picker_toggles_the_popover_and_seeds_its_draft() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::OpenSquadCreator, &db);

      let _ = update(&mut state, Message::SquadColorPickerToggled, &db);
      let creator = state.squad_creator.as_ref().expect("creator open");
      assert!(creator.color_popover_open);
      assert_eq!(creator.hex_draft, "#3FB8DB");

      let _ = update(&mut state, Message::SquadColorPickerToggled, &db);
      assert!(!state.squad_creator.as_ref().unwrap().color_popover_open);
    }

    #[tokio::test]
    async fn the_menu_collapse_row_toggles_collapse_and_closes_the_squad_menu() {
      let db = store::open_test().await.unwrap();
      let squad = character::create(&db, "Supers", None, None).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(10.0, 10.0)), &db);
      let _ = update(&mut state, Message::OpenSquadMenu(squad.id()), &db);

      let _ = update(&mut state, Message::ToggleSquadCollapse(squad.id()), &db);

      assert!(is_squad_collapsed(&state, squad.id()));
      assert!(state.squad_menu.is_none());
    }

    #[tokio::test]
    async fn ungrouping_a_squad_from_the_menu_moves_its_pilots_to_unassigned() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1, "Pilot").await;
      let squad = character::create(&db, "Crew", None, None).await.unwrap();
      character::assign(&db, 1, squad.id(), 0).await.unwrap();
      let mut state = State::new();
      reload(&mut state, &db).await;
      let _ = update(&mut state, Message::DragMoved(iced::Point::new(10.0, 10.0)), &db);
      let _ = update(&mut state, Message::OpenSquadMenu(squad.id()), &db);

      let _ = update(&mut state, Message::UngroupSquad(squad.id()), &db);
      assert!(state.squad_menu.is_none());
      apply_squad_write(
        &db,
        SquadWrite::Ungroup {
          squad_id: squad.id(),
        },
      )
      .await
      .unwrap();

      assert!(character::get_squad(&db, squad.id()).await.unwrap().is_some());
      assert!(character::members(&db, squad.id()).await.unwrap().is_empty());
      assert_eq!(reload_squad_of(&mut state, &db, 1).await, None);
    }
  }

  mod auto_scroll {
    use pretty_assertions::assert_eq;

    use super::*;

    // A viewport whose top edge sits at `top`, 600px tall, with 1000px of headroom to scroll.
    fn view(top: f32, offset: f32) -> GridViewport {
      GridViewport {
        height: 600.0,
        max_offset: 1_000.0,
        offset,
        top,
      }
    }

    #[test]
    fn it_does_not_scroll_when_the_cursor_is_away_from_both_edges() {
      // Cursor parked in the middle of a 600px viewport, well outside either 72px edge zone.
      assert_eq!(auto_scroll_delta(300.0, view(0.0, 400.0)), None);
    }

    #[test]
    fn it_scrolls_up_near_the_top_edge_and_down_near_the_bottom_edge() {
      let up = auto_scroll_delta(10.0, view(0.0, 400.0)).unwrap();
      assert!(up < 0.0, "cursor near the top edge scrolls toward a smaller offset");

      let down = auto_scroll_delta(590.0, view(0.0, 400.0)).unwrap();
      assert!(down > 0.0, "cursor near the bottom edge scrolls toward a larger offset");
    }

    #[test]
    fn it_ramps_speed_with_edge_proximity() {
      // Deeper into the bottom zone (cursor at the very edge) must move faster than just inside it.
      let edge = auto_scroll_delta(600.0, view(0.0, 400.0)).unwrap();
      let shallow = auto_scroll_delta(535.0, view(0.0, 400.0)).unwrap();

      assert!(
        edge > shallow,
        "speed must ramp up as the cursor approaches the edge: {edge} !> {shallow}"
      );
      assert!(edge <= AUTO_SCROLL_MAX_SPEED);
    }

    #[test]
    fn it_clamps_to_max_offset_at_the_bottom() {
      // Already 5px from the bottom of the content: the nudge cannot exceed the remaining travel.
      let delta = auto_scroll_delta(600.0, view(0.0, 995.0)).unwrap();
      assert_eq!(delta, 5.0, "must not scroll past max_offset");
    }

    #[test]
    fn it_stops_at_the_bottom_bound() {
      // Pinned at max_offset: no further down-scroll is possible, so no nudge is emitted.
      assert_eq!(auto_scroll_delta(600.0, view(0.0, 1_000.0)), None);
    }

    #[test]
    fn it_stops_at_the_top_bound() {
      // Pinned at offset 0: no further up-scroll is possible near the top edge.
      assert_eq!(auto_scroll_delta(0.0, view(0.0, 0.0)), None);
    }

    #[test]
    fn it_does_not_scroll_a_grid_that_fits_its_viewport() {
      let fits = GridViewport {
        height: 600.0,
        max_offset: 0.0,
        offset: 0.0,
        top: 0.0,
      };
      assert_eq!(auto_scroll_delta(600.0, fits), None);
    }

    #[test]
    fn it_honors_the_viewport_top_offset() {
      // Viewport pushed 200px down the window: the top edge zone is 200..272, not 0..72.
      assert!(auto_scroll_delta(210.0, view(200.0, 400.0)).unwrap() < 0.0);
      // The same window-y, with the viewport at the origin, is mid-grid and must not scroll.
      assert_eq!(auto_scroll_delta(210.0, view(0.0, 400.0)), None);
    }
  }
}
