mod abyssals;
mod header;
mod inventory;
mod shell;
mod stockpile_multibuy;
mod stockpile_search;
mod stockpiles;
mod tracker;
mod tree;
mod values;

use std::{
  collections::{HashMap, HashSet},
  time::Duration,
};

use chrono::{DateTime, Utc};
use iced::{Element, Task, widget::text_editor};

pub use self::{
  stockpile_multibuy::parse as parse_multibuy,
  stockpile_search::{
    LocationRef, LocationTier, MultibuyMatch, MultibuyResolution, resolve_multibuy, search_item_types,
    search_locations_enriched,
  },
  stockpiles::{
    EDITOR_WINDOW_HEIGHT as STOCKPILE_EDITOR_WINDOW_HEIGHT, EDITOR_WINDOW_WIDTH as STOCKPILE_EDITOR_WINDOW_WIDTH,
    Editor, EditorEffect, EditorSeed, IMPORT_WINDOW_HEIGHT as STOCKPILE_IMPORT_WINDOW_HEIGHT,
    IMPORT_WINDOW_WIDTH as STOCKPILE_IMPORT_WINDOW_WIDTH, ImportEffect, ImportPanel,
    SEARCH_MIN_CHARS as STOCKPILE_SEARCH_MIN_CHARS, apply_editor, apply_import,
    import_window_title as stockpile_import_window_title, import_window_view as stockpile_import_view,
    resolve_scope_pilots, save_stockpile, view as stockpile_editor_view, window_title as stockpile_editor_window_title,
  },
};
pub(crate) use crate::ui::format::{fmt_count, fmt_isk, fmt_volume};
use crate::{
  features::{
    shell::window_state::UiState,
    wallet::selection::{ClickKind, RowSelection},
  },
  store::{
    Database, images,
    model::{
      ENTITY_TYPE_ASSET, OwnerType, SavedAssetFilter, StatTemplate, TAG_SCOPE_ASSET, Tag,
      asset_query::{
        ChildFilter, GeoSort, GeoTree, InventoryCursor, InventoryQuery, InventoryRow, InventoryTotals, SortColumn,
        SortDirection,
      },
      normalize_tag_name,
    },
    repo::{assets, character, infra, org},
  },
  sync::JobKind,
  ui::{
    components::{
      add_tag_modal::{AddTagMessage, AddTagModal},
      resizable_pane::{self, PaneDrag},
    },
    load_epoch::LoadEpoch,
  },
};

const INVENTORY_PAGE_SIZE: i64 = 200;

const INVENTORY_SCROLL_THRESHOLD: f32 = 0.85;

const SEARCH_DEBOUNCE_MS: u64 = 200;

const HEADER_SIDE_PADDING: f32 = 28.0;

const SIDEBAR_PANE_KEY: &str = "assets.sidebar";

const SIDEBAR_DEFAULT_WIDTH: f32 = 280.0;

const ABYSSALS_FILTER_PANE_KEY: &str = "assets.abyssals_filter";

const ABYSSALS_FILTER_DEFAULT_WIDTH: f32 = 240.0;

const ABYSSAL_SCROLL_THRESHOLD: f32 = 0.85;

const MUTAMARKET_MODULE_URL: &str = "https://mutamarket.com/modules";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Category {
  #[default]
  All,
  Blueprint,
  Book,
  Charge,
  Commodity,
  Drone,
  Implant,
  Material,
  Module,
  Ship,
}

impl Category {
  pub(super) const ALL: [Category; 10] = [
    Category::All,
    Category::Ship,
    Category::Module,
    Category::Drone,
    Category::Charge,
    Category::Implant,
    Category::Blueprint,
    Category::Material,
    Category::Book,
    Category::Commodity,
  ];

  pub(super) fn label(self) -> String {
    match self {
      Category::All => t!("assets.category.all").into_owned(),
      Category::Blueprint => t!("assets.category.blueprints").into_owned(),
      Category::Book => t!("assets.category.skill_books").into_owned(),
      Category::Charge => t!("assets.category.charges").into_owned(),
      Category::Commodity => t!("assets.category.commodities").into_owned(),
      Category::Drone => t!("assets.category.drones").into_owned(),
      Category::Implant => t!("assets.category.implants").into_owned(),
      Category::Material => t!("assets.category.materials").into_owned(),
      Category::Module => t!("assets.category.modules").into_owned(),
      Category::Ship => t!("assets.category.ships").into_owned(),
    }
  }

  fn key(self) -> Option<&'static str> {
    match self {
      Category::All => None,
      Category::Blueprint => Some("blueprint"),
      Category::Book => Some("book"),
      Category::Charge => Some("charge"),
      Category::Commodity => Some("commodity"),
      Category::Drone => Some("drone"),
      Category::Implant => Some("implant"),
      Category::Material => Some("material"),
      Category::Module => Some("module"),
      Category::Ship => Some("ship"),
    }
  }

  fn from_key(key: Option<&str>) -> Category {
    Category::ALL
      .into_iter()
      .find(|category| category.key() == key)
      .unwrap_or(Category::All)
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeoNodeKey {
  Constellation(i64),
  Region(i64),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GeoSelection {
  #[default]
  All,
  Constellation(i64),
  Location(i64),
  Region(i64),
  System(i64),
}

#[derive(Clone, Debug, Default)]
pub struct Loaded {
  abyssals: abyssals::AbyssalsData,
  asset_tags: Vec<Tag>,
  corporations: Vec<RosterCorp>,
  /// Container ids on the path to a filter match, to auto-expand so nested matches are revealed.
  /// Empty when no filter is active. Ordered shallow-to-deep so apply seeds parents before children.
  expand_containers: Vec<i64>,
  geo_tree: GeoTree,
  inventory: Vec<InventoryRow>,
  /// Pre-loaded children of each `expand_containers` entry, keyed by container `item_id`. Seeds the
  /// lazy children cache so the auto-expanded ancestor chain renders without a round of fetches.
  inventory_children: HashMap<i64, Vec<InventoryRow>>,
  nav: tracker::NavSeries,
  roster: Vec<RosterPilot>,
  saved_filters: Vec<SavedAssetFilter>,
  stockpiles: Vec<stockpiles::StockpileCard>,
  tag_memberships: HashMap<i64, Vec<i64>>,
  totals: InventoryTotals,
  values: values::ValueSummary,
}

#[derive(Clone, Debug)]
pub enum Message {
  AbyssalCardsReloaded(abyssals::FilteredCards),
  AbyssalFilterReset,
  /// `relative` is the 0.0–1.0 scroll fraction that drives the pagination threshold; `absolute` is
  /// the pixel offset stored to window the card grid.
  AbyssalGridScrolled {
    absolute: f32,
    relative: f32,
  },
  AbyssalMutaMarketOpened(i64),
  AbyssalPageLoaded {
    cards: Vec<abyssals::AbyssalCard>,
    epoch: u64,
  },
  AbyssalPickerToggled,
  AbyssalSliderEditCommitted(i64, SliderEndpoint),
  AbyssalSliderEditInput(String),
  AbyssalSliderEditStarted(i64, SliderEndpoint, f64),
  AbyssalSourceTypeSelected(Option<i64>),
  AbyssalStatMaxChanged(i64, f64),
  AbyssalStatMinChanged(i64, f64),
  // Constructed only by handler-routing tests; the real range-update arm is wired but not yet triggered from the UI.
  #[cfg_attr(
    not(test),
    expect(
      dead_code,
      reason = "Range-update dispatch arm is wired; awaiting the abyssal UI to emit it."
    )
  )]
  AbyssalStatRangeChanged(i64, SliderEndpoint, f64),
  AbyssalStatTemplatesLoaded(Vec<StatTemplate>),
  AbyssalTypeModalClosed,
  AbyssalTypeModalOpened,
  AssetChartHovered(Option<f32>),
  /// A message from the shared add-tag modal, routed to the asset-tag host. Phase 4 (multi-select Edit
  /// Tags) reuses this same arm to drive the modal over a selection.
  AssetTagModal(AddTagMessage),
  /// The asset-tag registry and per-item membership map, reloaded after a modal write so chips update
  /// without a full inventory refetch.
  AssetTagsReloaded {
    memberships: HashMap<i64, Vec<i64>>,
    tags: Vec<Tag>,
  },
  CategorySelected(Category),
  ContainerChildrenLoaded(i64, Vec<InventoryRow>),
  ContainerToggled(i64),
  FeaturesChanged(crate::config::FeatureFlags),
  FilterExamplePicked(&'static str),
  GeoNodeSelected(GeoSelection),
  GeoNodeToggled(GeoNodeKey),
  /// The location-tree sort toggle picked a mode. Re-sorts the cached tree in
  /// place and persists the choice; never reloads from the DB.
  LocSortSelected(GeoSort),
  InventoryCursorMoved(iced::Point),
  InventoryHelpToggled,
  /// Tracks the live keyboard modifiers so a left-click on a row can resolve its
  /// plain/range/toggle intent (mirrors the wallet ledger's modifier tracking).
  InventoryModifiersChanged(iced::keyboard::Modifiers),
  /// Dismisses the row context menu (Escape or backdrop click).
  InventoryMenuDismissed,
  InventoryPageLoaded {
    epoch: u64,
    rows: Vec<InventoryRow>,
  },
  /// A left-click on an inventory row, applying the active modifiers to the
  /// multi-row selection.
  InventoryRowClicked(i64),
  /// A right-click on an inventory row: selects the row if it is not already in
  /// the selection, then opens the Edit Tags context menu at the cursor.
  InventoryRowRightPressed(i64),
  /// The pointer entered a row (`Some(item_id)`) or left the hovered row (`None`). Drives the
  /// hover-only `+ Tag` affordance, which is hidden until the pointer is over the row.
  InventoryRowHovered(Option<i64>),
  /// `relative` is the 0.0–1.0 scroll fraction that drives the pagination threshold; `absolute` is
  /// the pixel offset stored to window the virtual list.
  InventoryScrolled {
    absolute: f32,
    relative: f32,
  },
  Loaded(Box<Loaded>),
  /// Opens the shared add-tag modal over the current multi-row selection, fanning every
  /// assign/unassign across all selected stacks. Triggered by the row context menu's "Edit Tags".
  OpenSelectionTagModal,
  /// Opens the shared add-tag modal scoped to asset tags for one inventory row, keyed on its ESI
  /// `item_id`. Phase 4 adds a sibling that opens the same modal over a multi-row selection.
  OpenAssetTagModal {
    item_id: i64,
  },
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart(Pane),
  PaneSettled(&'static str, f32),
  PickerToggled,
  ReauthRequested(i64),
  SaveFilterCancelled,
  SaveFilterConfirmed,
  SaveFilterNameChanged(String),
  SaveFilterOpened,
  SavedFilterContextMenuClosed,
  SavedFilterCreated(Option<i64>, Vec<SavedAssetFilter>),
  SavedFilterDeleted(i64),
  SavedFilterRightPressed(i64),
  SavedFilterSelected(i64),
  SavedFiltersReloaded(Vec<SavedAssetFilter>),
  ScopeSelected(Scope),
  SearchChanged(String),
  SearchReloaded {
    generation: u64,
    loaded: Box<Loaded>,
  },
  SearchSubmitted,
  SidebarCursorMoved(iced::Point),
  SortSelected(SortColumn),
  StockpileCardRightPressed(i64),
  StockpileContextMenuClosed,
  StockpileCursorMoved(iced::Point),
  StockpileDeleted(i64),
  StockpileEditStarted(i64),
  StockpileEditorClosed,
  StockpileEditorItemPicked(i64, String),
  StockpileEditorItemRemoved(usize),
  StockpileEditorItemResults(Vec<(i64, String)>),
  StockpileEditorItemSearchChanged(String),
  StockpileEditorItemTargetChanged(usize, String),
  StockpileEditorLocationCleared,
  StockpileEditorLocationPicked(LocationRef),
  StockpileEditorLocationResults(u64, Vec<LocationRef>),
  StockpileEditorLocationSearchChanged(String),
  StockpileEditorLocationToggled,
  StockpileEditorNameChanged(String),
  StockpileEditorPopoversClosed,
  StockpileEditorSaved,
  StockpileEditorScopeChanged(String),
  StockpileEditorScopeResolved(Vec<stockpiles::ScopePilot>),
  StockpileImportClosed,
  StockpileImportConfirmed,
  StockpileImportOpened,
  StockpileImportResolveRequested,
  StockpileImportResolved(MultibuyResolution),
  StockpileImportTextChanged(text_editor::Action),
  StockpileItemsToggled(i64),
  StockpileMultibuyExportClosed,
  StockpileMultibuyExportCopied(i64),
  StockpileMultibuyExportOpened(i64),
  StockpileMultibuyModeChanged(stockpiles::MultibuyMode),
  StockpileNew,
  StockpilesReloaded(Vec<stockpiles::StockpileCard>),
  SyncReloaded {
    generation: u64,
    loaded: Box<Loaded>,
  },
  TabSelected(Tab),
  /// Records a persisted boolean UI flag in local state, then asks the app layer
  /// to write it through to `UiState`. Mirrors the wallet feature's flow.
  UiFlagSet(String, bool),
  /// Emitted after `UiFlagSet` so the app layer can persist the flag. A no-op
  /// inside the feature; the real write lives in the app-level assets handler
  /// (mirrors `wallet::Message::UiFlagPersisted`, routed in `app.rs` to
  /// `record_ui_flag`). The fields are read there, not within this feature.
  UiFlagPersisted(String, bool),
}

impl Message {
  /// Whether handling this message can surface new image-bearing rows, so the shell should recheck for stale
  /// icons. Interaction-only messages (scroll, hover, filter edits) return `false` to keep the staleness scan off
  /// the per-frame path.
  pub fn loads_data(&self) -> bool {
    matches!(
      self,
      Message::AbyssalCardsReloaded(_)
        | Message::AbyssalPageLoaded { .. }
        | Message::ContainerChildrenLoaded(..)
        | Message::InventoryPageLoaded { .. }
        | Message::Loaded(_)
        | Message::SearchReloaded { .. }
        | Message::StockpileEditorScopeResolved(_)
        | Message::StockpilesReloaded(_)
        | Message::SyncReloaded { .. }
    )
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pane {
  AbyssalsFilter,
  Sidebar,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RosterCorp {
  pub id: i64,
  pub logo: images::ImageState,
  pub name: String,
  pub ticker: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RosterPilot {
  pub corp: String,
  pub granted_scopes: Option<String>,
  pub id: i64,
  pub name: String,
  pub portrait: images::ImageState,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Scope {
  #[default]
  All,
  Character(i64),
  Corporation(i64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SliderEndpoint {
  Max,
  Min,
}

#[derive(Debug)]
pub struct State {
  abyssal_filters: abyssals::Filters,
  abyssal_has_more: bool,
  abyssal_loading: bool,
  abyssal_page_epoch: LoadEpoch,
  abyssal_picker_open: bool,
  abyssal_scroll_offset: f32,
  abyssal_slider_edit: Option<(i64, SliderEndpoint)>,
  abyssal_slider_edit_text: String,
  abyssal_source_types: Vec<abyssals::SourceTypeFilter>,
  abyssal_stat_templates: Vec<StatTemplate>,
  abyssal_total: i64,
  abyssals: Vec<abyssals::AbyssalCard>,
  abyssals_filter: PaneDrag,
  active: Scope,
  add_tag_modal: Option<AddTagModal>,
  add_tag_modal_name: String,
  asset_tags: Vec<Tag>,
  category: Category,
  chart_hover: Option<f32>,
  corporations: Vec<RosterCorp>,
  dirty: bool,
  enabled_tabs: Vec<Tab>,
  expanded_containers: HashSet<i64>,
  features: crate::config::FeatureFlags,
  geo_expanded: HashSet<GeoNodeKey>,
  geo_selected: GeoSelection,
  geo_tree: GeoTree,
  inventory: Vec<InventoryRow>,
  inventory_children: HashMap<i64, Vec<InventoryRow>>,
  inventory_cursor: Option<iced::Point>,
  inventory_has_more: bool,
  /// The item_id of the row the pointer is currently over, or `None`. Reveals that row's `+ Tag` affordance.
  inventory_hovered_row: Option<i64>,
  inventory_help_open: bool,
  inventory_loading: bool,
  /// The cursor anchor of the open row context menu; `None` when closed.
  inventory_menu: Option<iced::Point>,
  inventory_modifiers: iced::keyboard::Modifiers,
  inventory_page_epoch: LoadEpoch,
  inventory_scroll_offset: f32,
  /// Multi-row selection over the inventory, keyed on the globally-unique ESI `item_id`.
  inventory_selection: RowSelection<i64>,
  /// The item_ids the open add-tag modal applies to. One entry for a per-row open; the whole
  /// selection for a bulk Edit Tags open. Assign/unassign fan across every id here.
  selection_tag_ids: Vec<i64>,
  nav: tracker::NavSeries,
  picker_open: bool,
  roster: Vec<RosterPilot>,
  saved_filter_active: Option<i64>,
  saved_filter_context_menu: Option<SavedFilterContextMenu>,
  saved_filter_draft_name: String,
  saved_filter_modal_open: bool,
  saved_filters: Vec<SavedAssetFilter>,
  search: String,
  search_generation: u64,
  sidebar: PaneDrag,
  sidebar_cursor: Option<iced::Point>,
  sort: SortColumn,
  sort_dir: SortDirection,
  stockpile_context_menu: Option<StockpileContextMenu>,
  stockpile_cursor: Option<iced::Point>,
  stockpile_expanded: HashSet<i64>,
  stockpile_multibuy_copied: bool,
  stockpile_multibuy_export: Option<i64>,
  stockpile_multibuy_mode: stockpiles::MultibuyMode,
  stockpiles: Vec<stockpiles::StockpileCard>,
  tab: Tab,
  tag_memberships: HashMap<i64, Vec<i64>>,
  totals: InventoryTotals,
  /// Persisted boolean UI preferences (e.g. the location-tree sort mode),
  /// restored from `UiState.flags` and mirrored back on toggle. Mirrors the
  /// wallet feature's `ui_flags` precedent.
  ui_flags: std::collections::BTreeMap<String, bool>,
  values: values::ValueSummary,
}

/// `UiState.flags` key for the location-tree sort mode. `true` selects A–Z,
/// `false` (the default) selects Value, preserving today's behaviour.
const LOC_SORT_ALPHA_FLAG: &str = "assets.loc_sort_alpha";

impl State {
  pub fn new(features: crate::config::FeatureFlags) -> Self {
    let enabled_tabs = enabled_tabs(&features);
    State {
      abyssals_filter: PaneDrag::new(
        ABYSSALS_FILTER_DEFAULT_WIDTH,
        crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH,
      ),
      active: Scope::default(),
      add_tag_modal: None,
      add_tag_modal_name: String::new(),
      asset_tags: Vec::new(),
      category: Category::default(),
      chart_hover: None,
      corporations: Vec::new(),
      dirty: false,
      enabled_tabs: enabled_tabs.clone(),
      features,
      expanded_containers: HashSet::new(),
      geo_expanded: HashSet::new(),
      geo_selected: GeoSelection::default(),
      geo_tree: GeoTree::default(),
      inventory: Vec::new(),
      inventory_children: HashMap::new(),
      inventory_cursor: None,
      inventory_has_more: false,
      inventory_hovered_row: None,
      inventory_help_open: false,
      inventory_loading: false,
      inventory_menu: None,
      inventory_modifiers: iced::keyboard::Modifiers::default(),
      inventory_page_epoch: LoadEpoch::default(),
      inventory_scroll_offset: 0.0,
      inventory_selection: RowSelection::default(),
      selection_tag_ids: Vec::new(),
      picker_open: false,
      roster: Vec::new(),
      saved_filter_active: None,
      saved_filter_context_menu: None,
      saved_filter_draft_name: String::new(),
      saved_filter_modal_open: false,
      saved_filters: Vec::new(),
      search: String::new(),
      search_generation: 0,
      sidebar_cursor: None,
      sidebar: PaneDrag::new(
        SIDEBAR_DEFAULT_WIDTH,
        crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH,
      ),
      sort: SortColumn::Value,
      sort_dir: SortDirection::Descending,
      tab: resolve_first_tab(&enabled_tabs),
      tag_memberships: HashMap::new(),
      totals: InventoryTotals::default(),
      ui_flags: std::collections::BTreeMap::new(),
      values: values::ValueSummary::default(),
      nav: tracker::NavSeries::default(),
      stockpiles: Vec::new(),
      stockpile_context_menu: None,
      stockpile_cursor: None,
      stockpile_expanded: HashSet::new(),
      stockpile_multibuy_export: None,
      stockpile_multibuy_copied: false,
      stockpile_multibuy_mode: stockpiles::MultibuyMode::default(),
      abyssals: Vec::new(),
      abyssal_source_types: Vec::new(),
      abyssal_filters: abyssals::Filters::default(),
      abyssal_picker_open: false,
      abyssal_slider_edit: None,
      abyssal_slider_edit_text: String::new(),
      abyssal_stat_templates: Vec::new(),
      abyssal_has_more: false,
      abyssal_loading: false,
      abyssal_page_epoch: LoadEpoch::default(),
      abyssal_scroll_offset: 0.0,
      abyssal_total: 0,
    }
  }

  pub fn with_restored_panes(mut self, ui: &UiState) -> Self {
    let host_width = ui.host_width("main", crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH);
    self.sidebar = PaneDrag::from_store(ui, SIDEBAR_PANE_KEY, SIDEBAR_DEFAULT_WIDTH, host_width);
    self.abyssals_filter =
      PaneDrag::from_store(ui, ABYSSALS_FILTER_PANE_KEY, ABYSSALS_FILTER_DEFAULT_WIDTH, host_width);
    self.ui_flags = ui.flags.clone();
    self
  }

  /// Generic persisted boolean accessor, mirroring the wallet feature's flow.
  pub fn ui_flag(&self, key: &str, default: bool) -> bool {
    self.ui_flags.get(key).copied().unwrap_or(default)
  }

  /// The active location-tree sort mode, derived from the persisted flag.
  /// Defaults to `Value` (descending) on a fresh profile.
  pub(super) fn geo_sort(&self) -> GeoSort {
    if self.ui_flag(LOC_SORT_ALPHA_FLAG, false) {
      GeoSort::Alpha
    } else {
      GeoSort::Value
    }
  }

  pub fn set_pane_host_width(&mut self, host_width: f32) {
    self.sidebar.set_host_width(host_width);
    self.abyssals_filter.set_host_width(host_width);
  }

  pub fn active(&self) -> Scope {
    self.active
  }

  pub fn active_tab(&self) -> Tab {
    self.tab
  }

  pub fn select_tab_by_id(&mut self, id: &str) -> bool {
    match Tab::from_id(id) {
      Some(tab) => {
        self.tab = tab;
        true
      }
      None => false,
    }
  }

  pub fn drain_dirty(&mut self, db: &Database) -> Option<Task<Message>> {
    if !self.dirty {
      return None;
    }
    self.dirty = false;
    Some(sync_reload(self, db))
  }

  #[cfg(test)]
  pub fn is_dirty(&self) -> bool {
    self.dirty
  }

  // Unconditional reload flag for the kind-agnostic MCP write signal, which knows an agent wrote
  // something but not which job kind, so it can't go through the whitelisted mark_dirty path.
  pub fn force_dirty(&mut self) {
    self.dirty = true;
  }

  pub fn mark_dirty(&mut self, kind: JobKind) {
    if kind == JobKind::AssetSync {
      self.dirty = true;
    }
  }

  pub(super) fn enabled_tabs(&self) -> &[Tab] {
    &self.enabled_tabs
  }

  pub(super) fn sync_features(&mut self, features: crate::config::FeatureFlags) {
    self.features = features;
    self.enabled_tabs = enabled_tabs(&features);
    if !self.enabled_tabs.contains(&self.tab) {
      self.tab = resolve_first_tab(&self.enabled_tabs);
    }
  }

  /// The per-tab `(id, name, missing-scopes)` forbidden gate for a per-character view whose pilot
  /// lacks the active tab's read scopes; `None` for the combined view or an authorized pilot.
  pub(super) fn tab_scope_gate(&self) -> Option<(i64, &str, Vec<&'static str>)> {
    let Scope::Character(id) = self.active else {
      return None;
    };
    let pilot = self.roster.iter().find(|pilot| pilot.id == id)?;
    let missing =
      crate::ui::components::forbidden::missing_scopes(pilot.granted_scopes.as_deref(), &self.tab.read_scopes());
    if missing.is_empty() {
      return None;
    }
    Some((id, pilot.name.as_str(), missing))
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    self
      .roster
      .iter()
      .filter_map(|pilot| pilot.portrait.stale_key())
      .chain(self.corporations.iter().filter_map(|corp| corp.logo.stale_key()))
      .chain(self.abyssals.iter().filter_map(|card| card.portrait.stale_key()))
      .collect()
  }

  pub(super) fn tab(&self) -> Tab {
    self.tab
  }

  pub(super) fn inventory(&self) -> &[InventoryRow] {
    &self.inventory
  }

  /// The asset-tag chips assigned to one inventory stack (keyed on its ESI `item_id`), resolved from the
  /// loaded membership map against the asset-tag registry, in tag-position order. Empty for an untagged
  /// or unknown item.
  pub(super) fn asset_tags_for(&self, item_id: i64) -> Vec<&Tag> {
    let Some(tag_ids) = self.tag_memberships.get(&item_id) else {
      return Vec::new();
    };
    tag_ids
      .iter()
      .filter_map(|tag_id| self.asset_tags.iter().find(|tag| tag.id() == *tag_id))
      .collect()
  }

  /// The shared add-tag modal when open over an asset row, plus the asset-name resolver and the
  /// assigned/assignable tag partition for that row. Phase 4 reuses this same accessor shape (modal +
  /// resolver) to render the modal over a multi-row selection.
  pub(super) fn asset_tag_modal(&self) -> Option<&AddTagModal> {
    self.add_tag_modal.as_ref()
  }

  /// The display name for the asset row the open modal targets, resolved and stored at open time so the
  /// modal view can borrow it for the render's lifetime.
  pub(super) fn asset_tag_modal_entity_name(&self) -> &str {
    &self.add_tag_modal_name
  }

  /// Resolves an inventory row's display name — its custom name when set, else its type name, else a
  /// bare `Item <id>` fallback when the row has scrolled out of the loaded page.
  fn resolve_item_name(&self, item_id: i64) -> String {
    self
      .find_inventory_row(item_id)
      .map(|row| {
        row
          .name
          .as_deref()
          .filter(|name| !name.is_empty())
          .unwrap_or(&row.type_name)
          .to_owned()
      })
      .unwrap_or_else(|| t!("assets.inventory.item_fallback", id => item_id).into_owned())
  }

  /// The assigned/assignable partition for the open modal's targets.
  ///
  /// For a single target (per-row open) this is simply that stack's tags vs every other asset-scoped
  /// tag. For a bulk selection (Edit Tags) a tag counts as "assigned" only when it is on *all* selected
  /// stacks (the intersection), so the Current-tags chips show what the whole selection shares; every
  /// other tag is assignable. Adding a tag tags all selected stacks; removing untags all of them.
  pub(super) fn asset_tag_modal_partition(&self) -> (Vec<&Tag>, Vec<&Tag>) {
    if self.add_tag_modal.is_none() || self.selection_tag_ids.is_empty() {
      return (Vec::new(), Vec::new());
    }
    let assigned_ids = self.common_tag_ids();
    let assigned = self
      .asset_tags
      .iter()
      .filter(|tag| assigned_ids.contains(&tag.id()))
      .collect();
    let assignable = self
      .asset_tags
      .iter()
      .filter(|tag| !assigned_ids.contains(&tag.id()))
      .collect();
    (assigned, assignable)
  }

  /// The tag ids shared by *every* selected stack (the intersection of their membership sets). For a
  /// single target this is just that stack's tags; empty when any selected stack is untagged.
  fn common_tag_ids(&self) -> Vec<i64> {
    let mut iter = self.selection_tag_ids.iter();
    let Some(first) = iter.next() else {
      return Vec::new();
    };
    let mut common: Vec<i64> = self.tag_memberships.get(first).cloned().unwrap_or_default();
    for item_id in iter {
      let theirs: &[i64] = self.tag_memberships.get(item_id).map_or(&[], Vec::as_slice);
      common.retain(|tag_id| theirs.contains(tag_id));
      if common.is_empty() {
        break;
      }
    }
    common
  }

  /// The currently-loaded inventory rows in display order (flattened with any expanded containers'
  /// children spliced inline), keyed on `item_id` — the order the selection's range/prune resolve
  /// against.
  pub(super) fn inventory_order(&self) -> Vec<i64> {
    let mut order = Vec::with_capacity(self.inventory.len());
    for row in &self.inventory {
      push_inventory_order(self, &mut order, row);
    }
    order
  }

  pub(super) fn inventory_row_selected(&self, item_id: i64) -> bool {
    self.inventory_selection.contains(item_id)
  }

  pub(super) fn inventory_row_hovered(&self, item_id: i64) -> bool {
    self.inventory_hovered_row == Some(item_id)
  }

  pub(super) fn inventory_selection_count(&self) -> usize {
    self.inventory_selection.len()
  }

  pub(super) fn inventory_menu(&self) -> Option<iced::Point> {
    self.inventory_menu
  }

  /// The summed value and volume of the selected rows, for the selection summary slot. Walks the loaded
  /// rows (parents + expanded children) and sums those in the selection.
  pub(super) fn inventory_selection_totals(&self) -> (f64, f64) {
    let mut value = 0.0;
    let mut volume = 0.0;
    for row in self.inventory.iter().chain(self.inventory_children.values().flatten()) {
      if self.inventory_selection.contains(row.item_id) {
        value += row.value;
        volume += row.row_volume;
      }
    }
    (value, volume)
  }

  fn find_inventory_row(&self, item_id: i64) -> Option<&InventoryRow> {
    self
      .inventory
      .iter()
      .chain(self.inventory_children.values().flatten())
      .find(|row| row.item_id == item_id)
  }

  pub(super) fn inventory_total(&self) -> i64 {
    self.totals.items
  }

  pub(super) fn inventory_scroll_offset(&self) -> f32 {
    self.inventory_scroll_offset
  }

  pub(super) fn inventory_sort(&self) -> (SortColumn, SortDirection) {
    (self.sort, self.sort_dir)
  }

  pub(super) fn inventory_help_open(&self) -> bool {
    self.inventory_help_open
  }

  pub(super) fn container_is_open(&self, item_id: i64) -> bool {
    self.expanded_containers.contains(&item_id)
  }

  pub(super) fn container_children_of(&self, item_id: i64) -> Option<&[InventoryRow]> {
    self.inventory_children.get(&item_id).map(Vec::as_slice)
  }

  pub(super) fn search(&self) -> &str {
    &self.search
  }

  pub(super) fn category(&self) -> Category {
    self.category
  }

  pub(super) fn saved_filters(&self) -> &[SavedAssetFilter] {
    &self.saved_filters
  }

  pub(super) fn saved_filter_active(&self) -> Option<i64> {
    self.saved_filter_active
  }

  pub(super) fn saved_filter_modal_open(&self) -> bool {
    self.saved_filter_modal_open
  }

  pub(super) fn saved_filter_draft_name(&self) -> &str {
    &self.saved_filter_draft_name
  }

  pub(super) fn saved_filter_context_menu(&self) -> Option<&SavedFilterContextMenu> {
    self.saved_filter_context_menu.as_ref()
  }

  // The string captured by a "save filter" action: the raw query when present,
  // else `category:<key>` when only a category pill is set, else empty.
  pub(super) fn save_filter_capture(&self) -> String {
    let search = self.search.trim();
    if !search.is_empty() {
      return search.to_owned();
    }
    match self.category.key() {
      Some(key) => format!("category:{key}"),
      None => String::new(),
    }
  }

  pub(super) fn can_save_filter(&self) -> bool {
    !self.search.trim().is_empty() || self.category != Category::All
  }

  pub(super) fn roster(&self) -> &[RosterPilot] {
    &self.roster
  }

  pub(super) fn corporations(&self) -> &[RosterCorp] {
    &self.corporations
  }

  pub(super) fn geo_tree(&self) -> &GeoTree {
    &self.geo_tree
  }

  pub(super) fn geo_selected(&self) -> GeoSelection {
    self.geo_selected
  }

  pub(super) fn values(&self) -> &values::ValueSummary {
    &self.values
  }

  pub(super) fn nav(&self) -> &tracker::NavSeries {
    &self.nav
  }

  pub(super) fn chart_hover(&self) -> Option<f32> {
    self.chart_hover
  }

  pub(super) fn stockpiles(&self) -> &[stockpiles::StockpileCard] {
    &self.stockpiles
  }

  /// The card matching `id`, so the app layer can clone it to seed an Edit window. Returns `None` for
  /// a stale id (the card list may have reloaded out from under a pending context-menu action).
  pub fn stockpile_card(&self, id: i64) -> Option<&stockpiles::StockpileCard> {
    self.stockpiles.iter().find(|card| card.id == id)
  }

  /// Closes the open stockpile context menu, used by the app layer when opening an editor window from
  /// the menu's Edit action so the menu doesn't linger over the main view.
  pub fn dismiss_stockpile_context_menu(&mut self) {
    self.stockpile_context_menu = None;
  }

  pub(super) fn stockpile_context_menu(&self) -> Option<&StockpileContextMenu> {
    self.stockpile_context_menu.as_ref()
  }

  pub(super) fn stockpile_expanded(&self) -> &HashSet<i64> {
    &self.stockpile_expanded
  }

  pub(super) fn stockpile_multibuy_export(&self) -> Option<i64> {
    self.stockpile_multibuy_export
  }

  pub(super) fn stockpile_multibuy_copied(&self) -> bool {
    self.stockpile_multibuy_copied
  }

  pub(super) fn stockpile_multibuy_mode(&self) -> stockpiles::MultibuyMode {
    self.stockpile_multibuy_mode
  }

  pub(super) fn abyssals(&self) -> &[abyssals::AbyssalCard] {
    &self.abyssals
  }

  pub(super) fn abyssal_total(&self) -> i64 {
    self.abyssal_total
  }

  pub(super) fn abyssal_source_types(&self) -> &[abyssals::SourceTypeFilter] {
    &self.abyssal_source_types
  }

  pub(super) fn abyssal_filters(&self) -> &abyssals::Filters {
    &self.abyssal_filters
  }

  pub(super) fn abyssal_picker_open(&self) -> bool {
    self.abyssal_picker_open
  }

  pub(super) fn abyssal_stat_templates(&self) -> &[StatTemplate] {
    &self.abyssal_stat_templates
  }

  pub(super) fn abyssal_scroll_offset(&self) -> f32 {
    self.abyssal_scroll_offset
  }

  pub(super) fn abyssal_slider_edit(&self) -> Option<(i64, SliderEndpoint)> {
    self.abyssal_slider_edit
  }

  pub(super) fn abyssal_slider_edit_text(&self) -> &str {
    &self.abyssal_slider_edit_text
  }

  pub(super) fn geo_is_collapsed(&self, key: GeoNodeKey) -> bool {
    !self.geo_expanded.contains(&key)
  }

  #[cfg(test)]
  pub(super) fn set_geo_tree_for_test(&mut self, tree: GeoTree) {
    self.geo_tree = tree;
  }

  #[cfg(test)]
  pub(super) fn set_picker_for_test(&mut self, active: Scope, roster: Vec<RosterPilot>, corporations: Vec<RosterCorp>) {
    self.active = active;
    self.roster = roster;
    self.corporations = corporations;
  }

  #[cfg(test)]
  pub(super) fn set_abyssals_for_test(
    &mut self,
    cards: Vec<abyssals::AbyssalCard>,
    source_types: Vec<abyssals::SourceTypeFilter>,
    filters: abyssals::Filters,
    picker_open: bool,
  ) {
    self.abyssals = cards;
    self.abyssal_source_types = source_types;
    self.abyssal_filters = filters;
    self.abyssal_picker_open = picker_open;
  }

  #[cfg(test)]
  pub(super) fn set_abyssal_pagination_for_test(&mut self, has_more: bool, loading: bool) {
    self.abyssal_has_more = has_more;
    self.abyssal_loading = loading;
  }

  #[cfg(test)]
  pub(super) fn abyssal_has_more(&self) -> bool {
    self.abyssal_has_more
  }

  #[cfg(test)]
  pub(super) fn abyssal_loading(&self) -> bool {
    self.abyssal_loading
  }

  #[cfg(test)]
  pub(super) fn set_abyssal_stat_templates_for_test(&mut self, templates: Vec<StatTemplate>) {
    self.abyssal_stat_templates = templates;
  }

  #[cfg(test)]
  pub(super) fn set_for_test(
    &mut self,
    scope: Scope,
    roster: Vec<RosterPilot>,
    inventory: Vec<InventoryRow>,
    search: String,
  ) {
    self.active = scope;
    self.roster = roster;
    self.inventory = inventory;
    self.search = search;
  }

  #[cfg(test)]
  pub(super) fn set_inventory_children_for_test(&mut self, container_id: i64, children: Vec<InventoryRow>) {
    self.expanded_containers.insert(container_id);
    self.inventory_children.insert(container_id, children);
  }

  pub(super) fn pane(&self, pane: Pane) -> &PaneDrag {
    match pane {
      Pane::Sidebar => &self.sidebar,
      Pane::AbyssalsFilter => &self.abyssals_filter,
    }
  }

  fn active_drag(&self) -> Option<Pane> {
    if self.sidebar.is_active() {
      Some(Pane::Sidebar)
    } else if self.abyssals_filter.is_active() {
      Some(Pane::AbyssalsFilter)
    } else {
      None
    }
  }

  fn pane_mut(&mut self, pane: Pane) -> (&mut PaneDrag, &'static str) {
    match pane {
      Pane::Sidebar => (&mut self.sidebar, SIDEBAR_PANE_KEY),
      Pane::AbyssalsFilter => (&mut self.abyssals_filter, ABYSSALS_FILTER_PANE_KEY),
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Tab {
  Abyssals,
  #[default]
  Inventory,
  Stockpiles,
  Tracker,
  Values,
}

impl Tab {
  const ORDER: [Tab; 5] = [
    Tab::Inventory,
    Tab::Abyssals,
    Tab::Stockpiles,
    Tab::Values,
    Tab::Tracker,
  ];

  pub fn from_id(id: &str) -> Option<Tab> {
    match id {
      "abyssals" => Some(Tab::Abyssals),
      "inventory" => Some(Tab::Inventory),
      "stockpiles" => Some(Tab::Stockpiles),
      "tracker" => Some(Tab::Tracker),
      "values" => Some(Tab::Values),
      _ => None,
    }
  }

  pub fn id(self) -> &'static str {
    match self {
      Tab::Abyssals => "abyssals",
      Tab::Inventory => "inventory",
      Tab::Stockpiles => "stockpiles",
      Tab::Tracker => "tracker",
      Tab::Values => "values",
    }
  }

  pub(super) fn read_scopes(self) -> Vec<&'static str> {
    crate::features::shell::registry::sub_descriptor(self.sub_feature())
      .scopes
      .iter()
      .copied()
      .filter(|scope| !crate::clients::esi::scopes::is_write_scope(scope))
      .collect()
  }

  pub(super) fn sub_feature(self) -> crate::config::SubFeature {
    match self {
      Tab::Abyssals => crate::config::SubFeature::Abyssals,
      Tab::Inventory => crate::config::SubFeature::Inventory,
      Tab::Stockpiles => crate::config::SubFeature::Stockpiles,
      Tab::Tracker => crate::config::SubFeature::Tracker,
      Tab::Values => crate::config::SubFeature::Values,
    }
  }
}

pub(super) fn enabled_tabs(flags: &crate::config::FeatureFlags) -> Vec<Tab> {
  Tab::ORDER
    .into_iter()
    .filter(|tab| flags.is_sub_enabled(tab.sub_feature()))
    .collect()
}

pub(super) fn resolve_first_tab(enabled: &[Tab]) -> Tab {
  enabled.first().copied().unwrap_or_default()
}

#[derive(Clone, Debug)]
pub(super) struct SavedFilterContextMenu {
  pub anchor: iced::Point,
  pub id: i64,
  pub name: String,
}

#[derive(Clone, Debug)]
pub(super) struct StockpileContextMenu {
  pub anchor: iced::Point,
  pub id: i64,
  pub name: String,
}

#[derive(Clone, Debug)]
struct InventoryView {
  filter: String,
  limit: i64,
  location_ids: Vec<i64>,
  sort: SortColumn,
  sort_dir: SortDirection,
}

impl Default for InventoryView {
  fn default() -> Self {
    Self {
      filter: String::new(),
      limit: INVENTORY_PAGE_SIZE,
      location_ids: Vec::new(),
      sort: SortColumn::Value,
      sort_dir: SortDirection::Descending,
    }
  }
}

impl InventoryView {
  fn from_state(state: &State) -> Self {
    Self {
      filter: effective_filter(state.category, &state.search),
      limit: INVENTORY_PAGE_SIZE,
      location_ids: location_ids_for_selection(&state.geo_tree, state.geo_selected),
      sort: state.sort,
      sort_dir: state.sort_dir,
    }
  }

  fn with_limit(mut self, limit: i64) -> Self {
    self.limit = limit;
    self
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Owner {
  Character(i64),
  Combined {
    character_ids: Vec<i64>,
    corporation_ids: Vec<i64>,
  },
  Corporation(i64),
}

fn resolve_scope_owner(scope: Scope, roster: &[RosterPilot], corporations: &[RosterCorp]) -> Option<Owner> {
  match scope {
    Scope::All => Some(Owner::Combined {
      character_ids: roster.iter().map(|pilot| pilot.id).collect(),
      corporation_ids: corporations.iter().map(|corp| corp.id).collect(),
    }),
    Scope::Character(id) => Some(Owner::Character(id)),
    Scope::Corporation(id) => corporations
      .iter()
      .any(|corp| corp.id == id)
      .then_some(Owner::Corporation(id)),
  }
}

pub fn load(db: &Database) -> Task<Message> {
  Task::perform(
    load_assets(db.clone(), Scope::All, InventoryView::default()),
    |loaded| Message::Loaded(Box::new(loaded)),
  )
}

pub fn sync_reload(state: &State, db: &Database) -> Task<Message> {
  let generation = state.search_generation;
  let loaded_rows = (state.inventory.len() as i64).max(INVENTORY_PAGE_SIZE);
  let view = InventoryView::from_state(state).with_limit(loaded_rows);
  Task::perform(load_assets(db.clone(), state.active, view), move |loaded| {
    Message::SyncReloaded {
      generation,
      loaded: Box::new(loaded),
    }
  })
}

fn reload(db: &Database, scope: Scope, inventory: InventoryView) -> Task<Message> {
  Task::perform(load_assets(db.clone(), scope, inventory), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
}

fn reload_filtered(state: &mut State, db: &Database) -> Task<Message> {
  state.search_generation = state.search_generation.wrapping_add(1);
  reload(db, state.active, InventoryView::from_state(state))
}

fn trigger_search(state: &mut State, db: &Database) -> Task<Message> {
  state.search_generation = state.search_generation.wrapping_add(1);
  run_search(
    db.clone(),
    state.active,
    InventoryView::from_state(state),
    state.search_generation,
  )
}

fn run_search(db: Database, scope: Scope, view: InventoryView, generation: u64) -> Task<Message> {
  Task::perform(
    async move {
      tokio::time::sleep(Duration::from_millis(SEARCH_DEBOUNCE_MS)).await;
      load_assets(db, scope, view).await
    },
    move |loaded| Message::SearchReloaded {
      generation,
      loaded: Box::new(loaded),
    },
  )
}

fn apply_loaded(state: &mut State, loaded: Loaded) {
  let Loaded {
    asset_tags,
    corporations,
    expand_containers,
    geo_tree,
    inventory,
    inventory_children,
    roster,
    saved_filters,
    tag_memberships,
    totals,
    values,
    nav,
    stockpiles,
    abyssals,
  } = loaded;
  state.asset_tags = asset_tags;
  state.tag_memberships = tag_memberships;
  state.corporations = corporations;
  state.saved_filters = saved_filters;
  state.inventory_page_epoch.next();
  state.abyssal_page_epoch.next();
  state.inventory_has_more = inventory.len() as i64 == INVENTORY_PAGE_SIZE;
  state.inventory = inventory;
  state.inventory_loading = false;
  // Seed the filter-driven auto-expansion. With no filter active both are empty, so this collapses
  // back to normal browse (clearing any prior filter expansions); with a filter active it reveals
  // the ancestor chain holding each nested match.
  state.expanded_containers = expand_containers.into_iter().collect();
  state.inventory_children = inventory_children;
  state.roster = roster;
  state.totals = totals;
  state.values = values;
  state.nav = nav;
  state.stockpiles = stockpiles;
  state.abyssal_has_more = abyssals.cards.len() as i64 == abyssals::PAGE_SIZE;
  state.abyssal_loading = false;
  state.abyssal_scroll_offset = 0.0;
  state.abyssal_total = abyssals.total;
  state.abyssals = abyssals.cards;
  state.abyssal_source_types = abyssals.source_types;
  state.abyssal_filters = abyssals::Filters::default();
  state.abyssal_picker_open = false;
  state.abyssal_slider_edit = None;
  state.abyssal_slider_edit_text = String::new();
  state.abyssal_stat_templates = Vec::new();
  state.geo_tree = geo_tree;
  state.geo_tree.sort_by(state.geo_sort());
  prune_inventory_selection(state);
}

/// Drops selected rows that a reload, filter change, or container collapse removed from the live
/// inventory so a stale selection never points at rows the user can no longer see, and clears the
/// open context menu when its target selection emptied out.
fn prune_inventory_selection(state: &mut State) {
  let order = state.inventory_order();
  state.inventory_selection.prune(&order);
  if state.inventory_selection.is_empty() {
    state.inventory_menu = None;
  }
}

/// Refreshes data from a sync-triggered reload while preserving transient interaction state
/// (expanded containers, search text, abyssal filters, slider edits, and picker state).
///
/// Contrast with `apply_loaded`, which resets all of that state on every reload.
fn merge_loaded(state: &mut State, loaded: Loaded) {
  let Loaded {
    asset_tags,
    corporations,
    expand_containers,
    geo_tree,
    inventory,
    inventory_children,
    roster,
    saved_filters,
    tag_memberships,
    totals,
    values,
    nav,
    stockpiles,
    abyssals,
  } = loaded;
  state.asset_tags = asset_tags;
  state.tag_memberships = tag_memberships;
  state.corporations = corporations;
  state.saved_filters = saved_filters;
  state.inventory_page_epoch.next();
  state.abyssal_page_epoch.next();
  // Compare fresh page length against the limit that was requested (old inventory len or minimum
  // page size), not the constant — sync reloads may fetch more than one page worth.
  state.inventory_has_more = inventory.len() as i64 == (state.inventory.len() as i64).max(INVENTORY_PAGE_SIZE);
  state.inventory = inventory;
  state.roster = roster;
  state.totals = totals;
  state.values = values;
  state.nav = nav;
  state.stockpiles = stockpiles;
  state.abyssal_has_more = abyssals.cards.len() as i64 == abyssals::PAGE_SIZE;
  state.abyssal_loading = false;
  state.abyssal_total = abyssals.total;
  state.abyssals = abyssals.cards;
  state.abyssal_source_types = abyssals.source_types;
  state.geo_tree = geo_tree;
  state.geo_tree.sort_by(state.geo_sort());
  // Fold the filter-driven auto-expansion into the preserved manual expansions so a sync reload
  // keeps nested matches revealed, then drop any expansion whose container row is no longer present.
  for container_id in expand_containers {
    state.expanded_containers.insert(container_id);
  }
  for (container_id, children) in inventory_children {
    state.inventory_children.insert(container_id, children);
  }
  // A container is "present" if its row appears anywhere in the live tree — top-level or as a loaded
  // child of an expanded ancestor — so nested ancestors in an auto-expanded chain are not pruned.
  let present: HashSet<i64> = state
    .inventory
    .iter()
    .chain(state.inventory_children.values().flatten())
    .map(|row| row.item_id)
    .collect();
  state.expanded_containers.retain(|id| present.contains(id));
  state.inventory_children.retain(|id, _| present.contains(id));
  prune_inventory_selection(state);
}

fn effective_filter(category: Category, search: &str) -> String {
  match category.key() {
    Some(key) => {
      let search = search.trim();
      if search.is_empty() {
        format!("category:{key}")
      } else {
        format!("category:{key} {search}")
      }
    }
    None => search.to_owned(),
  }
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::AssetChartHovered(_)
    | Message::CategorySelected(_)
    | Message::FilterExamplePicked(_)
    | Message::InventoryHelpToggled
    | Message::Loaded(_)
    | Message::PickerToggled
    | Message::ScopeSelected(_)
    | Message::SearchChanged(_)
    | Message::SearchReloaded {
      ..
    }
    | Message::SearchSubmitted
    | Message::SortSelected(_)
    | Message::SyncReloaded {
      ..
    }
    | Message::TabSelected(_) => update_inventory(state, message, db),

    Message::ContainerChildrenLoaded(..)
    | Message::ContainerToggled(_)
    | Message::InventoryPageLoaded {
      ..
    }
    | Message::InventoryScrolled {
      ..
    } => update_pagination(state, message, db),

    Message::GeoNodeSelected(_)
    | Message::GeoNodeToggled(_)
    | Message::LocSortSelected(_)
    | Message::UiFlagSet(..)
    | Message::UiFlagPersisted(..) => update_geo(state, message, db),

    Message::AssetTagModal(_)
    | Message::AssetTagsReloaded {
      ..
    }
    | Message::InventoryCursorMoved(_)
    | Message::InventoryModifiersChanged(_)
    | Message::InventoryMenuDismissed
    | Message::InventoryRowClicked(_)
    | Message::InventoryRowHovered(_)
    | Message::InventoryRowRightPressed(_)
    | Message::OpenSelectionTagModal
    | Message::OpenAssetTagModal {
      ..
    } => update_asset_tags(state, message, db),

    Message::SaveFilterCancelled
    | Message::SaveFilterConfirmed
    | Message::SaveFilterNameChanged(_)
    | Message::SaveFilterOpened
    | Message::SavedFilterContextMenuClosed
    | Message::SavedFilterCreated(..)
    | Message::SavedFilterDeleted(_)
    | Message::SavedFilterRightPressed(_)
    | Message::SavedFilterSelected(_)
    | Message::SavedFiltersReloaded(_)
    | Message::SidebarCursorMoved(_) => update_saved_filter(state, message, db),

    Message::StockpileCardRightPressed(_)
    | Message::StockpileContextMenuClosed
    | Message::StockpileCursorMoved(_)
    | Message::StockpileDeleted(_)
    | Message::StockpileEditStarted(_)
    | Message::StockpileEditorClosed
    | Message::StockpileEditorItemPicked(..)
    | Message::StockpileEditorItemRemoved(_)
    | Message::StockpileEditorItemResults(..)
    | Message::StockpileEditorItemSearchChanged(..)
    | Message::StockpileEditorItemTargetChanged(..)
    | Message::StockpileEditorLocationCleared
    | Message::StockpileEditorLocationPicked(_)
    | Message::StockpileEditorLocationResults(..)
    | Message::StockpileEditorLocationSearchChanged(_)
    | Message::StockpileEditorLocationToggled
    | Message::StockpileEditorNameChanged(_)
    | Message::StockpileEditorPopoversClosed
    | Message::StockpileEditorSaved
    | Message::StockpileEditorScopeChanged(_)
    | Message::StockpileEditorScopeResolved(_)
    | Message::StockpileImportClosed
    | Message::StockpileImportConfirmed
    | Message::StockpileImportOpened
    | Message::StockpileImportResolveRequested
    | Message::StockpileImportResolved(_)
    | Message::StockpileImportTextChanged(_)
    | Message::StockpileItemsToggled(_)
    | Message::StockpileMultibuyExportClosed
    | Message::StockpileMultibuyExportCopied(_)
    | Message::StockpileMultibuyExportOpened(_)
    | Message::StockpileMultibuyModeChanged(_)
    | Message::StockpileNew
    | Message::StockpilesReloaded(_) => update_stockpile(state, message, db),

    Message::AbyssalCardsReloaded(_)
    | Message::AbyssalFilterReset
    | Message::AbyssalGridScrolled {
      ..
    }
    | Message::AbyssalMutaMarketOpened(_)
    | Message::AbyssalPageLoaded {
      ..
    }
    | Message::AbyssalPickerToggled
    | Message::AbyssalSliderEditCommitted(..)
    | Message::AbyssalSliderEditInput(_)
    | Message::AbyssalSliderEditStarted(..)
    | Message::AbyssalSourceTypeSelected(_)
    | Message::AbyssalStatMaxChanged(..)
    | Message::AbyssalStatMinChanged(..)
    | Message::AbyssalStatRangeChanged(..)
    | Message::AbyssalStatTemplatesLoaded(_)
    | Message::AbyssalTypeModalClosed
    | Message::AbyssalTypeModalOpened => update_abyssal(state, message, db),

    Message::PaneDrag(_) | Message::PaneDragEnd | Message::PaneDragStart(_) | Message::PaneSettled(..) => {
      update_pane(state, message)
    }

    Message::FeaturesChanged(features) => {
      let prev = state.tab;
      state.sync_features(features);
      if state.tab != prev {
        state.chart_hover = None;
        return reload_filtered(state, db);
      }
      Task::none()
    }

    Message::ReauthRequested(_) => Task::none(),
  }
}

fn update_inventory(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::Loaded(loaded) => {
      apply_loaded(state, *loaded);
      Task::none()
    }
    Message::SearchReloaded {
      generation,
      loaded,
    } => {
      if generation == state.search_generation {
        apply_loaded(state, *loaded);
      }
      Task::none()
    }
    Message::SyncReloaded {
      generation,
      loaded,
    } => {
      if generation == state.search_generation {
        merge_loaded(state, *loaded);
      }
      Task::none()
    }
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Task::none()
    }
    Message::ScopeSelected(scope) => {
      state.picker_open = false;
      if scope == state.active {
        return Task::none();
      }
      state.active = scope;
      state.chart_hover = None;
      state.geo_selected = GeoSelection::All;
      state.geo_expanded.clear();
      state.saved_filter_active = None;
      reload_filtered(state, db)
    }
    Message::TabSelected(tab) => {
      state.chart_hover = None;
      state.tab = tab;
      Task::none()
    }
    Message::AssetChartHovered(fraction) => {
      state.chart_hover = fraction;
      Task::none()
    }
    Message::SearchChanged(query) => {
      state.search = query;
      trigger_search(state, db)
    }
    Message::SearchSubmitted => trigger_search(state, db),
    Message::CategorySelected(category) => {
      if state.category == category {
        return Task::none();
      }
      state.category = category;
      reload_filtered(state, db)
    }
    Message::SortSelected(column) => {
      if state.sort == column {
        state.sort_dir = match state.sort_dir {
          SortDirection::Ascending => SortDirection::Descending,
          SortDirection::Descending => SortDirection::Ascending,
        };
      } else {
        state.sort = column;
        state.sort_dir = SortDirection::Descending;
      }
      reload_filtered(state, db)
    }
    Message::InventoryHelpToggled => {
      state.inventory_help_open = !state.inventory_help_open;
      Task::none()
    }
    Message::FilterExamplePicked(query) => {
      state.search = query.to_owned();
      state.inventory_help_open = false;
      reload_filtered(state, db)
    }
    _ => Task::none(),
  }
}

fn update_geo(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::GeoNodeToggled(key) => {
      if !state.geo_expanded.remove(&key) {
        state.geo_expanded.insert(key);
      }
      Task::none()
    }
    Message::GeoNodeSelected(selection) => {
      if state.geo_selected == selection {
        return Task::none();
      }
      state.geo_selected = selection;
      // Selecting a location clears any active saved filter (mutually exclusive).
      if !matches!(selection, GeoSelection::All) {
        state.saved_filter_active = None;
      }
      reload_filtered(state, db)
    }
    Message::LocSortSelected(mode) => {
      if state.geo_sort() == mode {
        return Task::none();
      }
      // Record the mode locally and re-sort the already-loaded tree in place —
      // collapse and selection state live elsewhere and are untouched — then ask
      // the app layer to persist the flag.
      let alpha = matches!(mode, GeoSort::Alpha);
      state.ui_flags.insert(LOC_SORT_ALPHA_FLAG.to_owned(), alpha);
      state.geo_tree.sort_by(mode);
      Task::done(Message::UiFlagSet(LOC_SORT_ALPHA_FLAG.to_owned(), alpha))
    }
    Message::UiFlagSet(key, value) => {
      state.ui_flags.insert(key.clone(), value);
      Task::done(Message::UiFlagPersisted(key, value))
    }
    // The app-level assets handler records the flag into `UiState`; nothing to do
    // here once local state already carries it.
    Message::UiFlagPersisted(..) => Task::none(),
    _ => Task::none(),
  }
}

fn update_saved_filter(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::SidebarCursorMoved(point) => {
      state.sidebar_cursor = Some(point);
      Task::none()
    }
    Message::SaveFilterOpened => {
      state.saved_filter_modal_open = true;
      state.saved_filter_draft_name = String::new();
      Task::none()
    }
    Message::SaveFilterCancelled => {
      state.saved_filter_modal_open = false;
      state.saved_filter_draft_name = String::new();
      Task::none()
    }
    Message::SaveFilterNameChanged(name) => {
      state.saved_filter_draft_name = name;
      Task::none()
    }
    Message::SaveFilterConfirmed => {
      let name = state.saved_filter_draft_name.trim().to_owned();
      if name.is_empty() {
        return Task::none();
      }
      let query = state.search.trim().to_owned();
      let category = state.category.key().map(str::to_owned);
      state.saved_filter_modal_open = false;
      state.saved_filter_draft_name = String::new();
      let db = db.clone();
      Task::perform(
        async move {
          let created = assets::create_saved_filter(&db, &name, &query, category.as_deref()).await;
          let filters = assets::saved_filters(&db).await.unwrap_or_default();
          (created.ok().map(|filter| filter.id()), filters)
        },
        |(new_id, filters)| Message::SavedFilterCreated(new_id, filters),
      )
    }
    Message::SavedFilterCreated(new_id, filters) => {
      state.saved_filters = filters;
      let Some(id) = new_id else {
        return Task::none();
      };
      // Selecting the new filter clears any geo selection (mutually exclusive).
      state.saved_filter_active = Some(id);
      state.geo_selected = GeoSelection::All;
      Task::none()
    }
    Message::SavedFiltersReloaded(filters) => {
      state.saved_filters = filters;
      Task::none()
    }
    Message::SavedFilterSelected(id) => {
      // Re-clicking the active filter clears it.
      if state.saved_filter_active == Some(id) {
        state.saved_filter_active = None;
        state.search = String::new();
        state.category = Category::All;
        return reload_filtered(state, db);
      }
      let Some(filter) = state.saved_filters.iter().find(|filter| filter.id() == id) else {
        return Task::none();
      };
      state.saved_filter_active = Some(id);
      state.search = filter.query().to_owned();
      state.category = Category::from_key(filter.category().as_deref());
      // Mutually exclusive with geo selection.
      state.geo_selected = GeoSelection::All;
      reload_filtered(state, db)
    }
    Message::SavedFilterRightPressed(id) => {
      if let (Some(anchor), Some(filter)) = (
        state.sidebar_cursor,
        state.saved_filters.iter().find(|filter| filter.id() == id),
      ) {
        state.saved_filter_context_menu = Some(SavedFilterContextMenu {
          anchor,
          id,
          name: filter.name().to_owned(),
        });
      }
      Task::none()
    }
    Message::SavedFilterContextMenuClosed => {
      state.saved_filter_context_menu = None;
      Task::none()
    }
    Message::SavedFilterDeleted(id) => {
      state.saved_filter_context_menu = None;
      if state.saved_filter_active == Some(id) {
        state.saved_filter_active = None;
      }
      let db = db.clone();
      Task::perform(
        async move {
          assets::delete_saved_filter(&db, id).await.ok();
          assets::saved_filters(&db).await.unwrap_or_default()
        },
        Message::SavedFiltersReloaded,
      )
    }
    _ => Task::none(),
  }
}

/// Drives the per-row asset-tag chips and the shared add-tag modal: open/close, search input, and the
/// assign/create/unassign writes (each followed by a registry+membership reload so chips update).
///
/// The state/message surface here is the one phase 4 (multi-select Edit Tags) reuses: an `Option<AddTagModal>`
/// host, the `AssetTagModal(AddTagMessage)` mapper arm, and the `reload_asset_tags` write-then-reload task.
/// Phase 4 adds a sibling open path over a selection but routes its modal messages through this same arm.
fn update_asset_tags(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::OpenAssetTagModal {
      item_id,
    } => {
      state.selection_tag_ids = vec![item_id];
      state.add_tag_modal_name = state.resolve_item_name(item_id);
      state.add_tag_modal = Some(AddTagModal::new(item_id, ENTITY_TYPE_ASSET));
      Task::none()
    }
    Message::OpenSelectionTagModal => {
      let order = state.inventory_order();
      let selected = state.inventory_selection.ordered(&order);
      if selected.is_empty() {
        return Task::none();
      }
      state.inventory_menu = None;
      state.add_tag_modal_name = selection_modal_name(state, &selected);
      // The modal's single `entity_id` is the first selected stack (a stand-in); assign/unassign
      // fan across every id in `selection_tag_ids`, so bulk acts on the whole selection.
      state.add_tag_modal = Some(AddTagModal::new(selected[0], ENTITY_TYPE_ASSET));
      state.selection_tag_ids = selected;
      Task::none()
    }
    Message::InventoryModifiersChanged(modifiers) => {
      state.inventory_modifiers = modifiers;
      Task::none()
    }
    Message::InventoryCursorMoved(point) => {
      state.inventory_cursor = Some(point);
      Task::none()
    }
    Message::InventoryRowHovered(item_id) => {
      state.inventory_hovered_row = item_id;
      Task::none()
    }
    Message::InventoryRowClicked(item_id) => {
      let order = state.inventory_order();
      let kind = ClickKind::from_modifiers(state.inventory_modifiers.command(), state.inventory_modifiers.shift());
      state.inventory_selection.apply(item_id, kind, &order);
      Task::none()
    }
    Message::InventoryRowRightPressed(item_id) => {
      // Right-clicking a row outside the current selection makes it the selection so the menu always
      // acts on what the user pointed at.
      if !state.inventory_selection.contains(item_id) {
        let order = state.inventory_order();
        state.inventory_selection.apply(item_id, ClickKind::Plain, &order);
      }
      if let Some(anchor) = state.inventory_cursor {
        state.inventory_menu = Some(anchor);
      }
      Task::none()
    }
    Message::InventoryMenuDismissed => {
      state.inventory_menu = None;
      Task::none()
    }
    Message::AssetTagsReloaded {
      memberships,
      tags,
    } => {
      state.asset_tags = tags;
      state.tag_memberships = memberships;
      Task::none()
    }
    Message::AssetTagModal(modal_message) => apply_asset_tag_modal(state, modal_message, db),
    _ => Task::none(),
  }
}

/// The modal header for a bulk Edit Tags: the lone stack's name for a single selection, otherwise an
/// "N stacks" count so the modal reads as acting on the whole selection.
fn selection_modal_name(state: &State, selected: &[i64]) -> String {
  match selected {
    [only] => state.resolve_item_name(*only),
    many => t!("assets.inventory.selection_stacks", count => many.len()).into_owned(),
  }
}

/// Fans a closure across the target stacks, building one task per id and batching them, each
/// followed by the shared tag reload. When a bulk modal is open the targets are the whole selection
/// (`selection_tag_ids`); otherwise the write applies to just `entity_id` — the stack named by the
/// message itself, as when a per-row tag chip's `×` unassigns directly without opening the modal.
fn fan_tag_write<F>(state: &State, db: &Database, entity_id: i64, write: F) -> Task<Message>
where
  F: Fn(Database, i64) -> Task<Message>,
{
  let tasks: Vec<Task<Message>> = tag_write_targets(state, entity_id)
    .into_iter()
    .map(|item_id| write(db.clone(), item_id))
    .collect();
  Task::batch(tasks)
}

/// The stacks a tag assign/unassign should apply to: the whole bulk selection when a modal has
/// populated `selection_tag_ids`, otherwise just `entity_id` — the stack named by the message, as
/// when a per-row chip's `×` unassigns directly without any modal open.
fn tag_write_targets(state: &State, entity_id: i64) -> Vec<i64> {
  if state.selection_tag_ids.is_empty() {
    vec![entity_id]
  } else {
    state.selection_tag_ids.clone()
  }
}

fn apply_asset_tag_modal(state: &mut State, message: AddTagMessage, db: &Database) -> Task<Message> {
  match message {
    AddTagMessage::InputChanged(value) => {
      if let Some(modal) = &mut state.add_tag_modal {
        modal.input = value;
      }
      Task::none()
    }
    AddTagMessage::Close => {
      state.add_tag_modal = None;
      state.selection_tag_ids = Vec::new();
      Task::none()
    }
    AddTagMessage::Assign {
      entity_id,
      entity_type,
      tag_id,
    } => fan_tag_write(state, db, entity_id, move |db, item_id| {
      Task::perform(assign_then_reload(db, entity_type, item_id, tag_id), |message| message)
    }),
    AddTagMessage::Unassign {
      entity_id,
      entity_type,
      tag_id,
    } => fan_tag_write(state, db, entity_id, move |db, item_id| {
      Task::perform(unassign_then_reload(db, entity_type, item_id, tag_id), |message| {
        message
      })
    }),
    AddTagMessage::CreateAndAssign {
      entity_type, ..
    } => {
      let Some(name) = state
        .add_tag_modal
        .as_ref()
        .map(|modal| modal.input.trim().to_owned())
        .filter(|name| !name.is_empty())
      else {
        return Task::none();
      };
      if state.selection_tag_ids.is_empty() {
        return Task::none();
      }
      // Reuse an existing asset tag of the same name rather than creating a duplicate. This in-memory
      // fast path keeps the UX snappy, but it MUST normalize names exactly like the repo's find-or-create
      // (`normalize_tag_name`, which mirrors the store's `(scope, lower(name))` key) so it never diverges
      // from what the store would do. The bespoke `eq_ignore_ascii_case` it replaces also folded ASCII
      // case, but did not trim — so a padded re-entry of the same name could slip past it and duplicate.
      let needle = normalize_tag_name(&name);
      let existing = state
        .asset_tags
        .iter()
        .find(|tag| normalize_tag_name(tag.name()) == needle)
        .map(Tag::id);
      if let Some(modal) = &mut state.add_tag_modal {
        modal.input.clear();
      }
      // Resolve (or create) the tag once, then assign it to every selected stack so a bulk create tags
      // the whole selection in one action.
      let item_ids = state.selection_tag_ids.clone();
      let db = db.clone();
      Task::perform(
        create_or_reuse_then_assign(db, name, existing, entity_type, item_ids),
        |message| message,
      )
    }
  }
}

/// Assign `tag_id` to one stack, then reload the asset-tag registry. The body of the modal's `Assign`
/// arm, lifted into a named future so the write-then-reload path is directly testable.
async fn assign_then_reload(db: Database, entity_type: &'static str, item_id: i64, tag_id: i64) -> Message {
  infra::assign(&db, entity_type, item_id, tag_id).await.ok();
  reload_asset_tags(db).await
}

/// Unassign `tag_id` from one stack, then reload the asset-tag registry. The body of the modal's
/// `Unassign` arm, lifted into a named future so the write-then-reload path is directly testable.
async fn unassign_then_reload(db: Database, entity_type: &'static str, item_id: i64, tag_id: i64) -> Message {
  infra::unassign(&db, entity_type, item_id, tag_id).await.ok();
  reload_asset_tags(db).await
}

/// Resolve the tag — reusing `existing` when set, otherwise creating it — then assign it to every
/// stack in `item_ids`, and reload. The body of the modal's `CreateAndAssign` arm, lifted into a
/// named future so the create/reuse-then-bulk-assign path is directly testable.
async fn create_or_reuse_then_assign(
  db: Database,
  name: String,
  existing: Option<i64>,
  entity_type: &'static str,
  item_ids: Vec<i64>,
) -> Message {
  let tag_id = match existing {
    Some(id) => Some(id),
    None => infra::create_scoped(&db, &name, None, None, TAG_SCOPE_ASSET)
      .await
      .ok()
      .map(|tag| tag.id()),
  };
  if let Some(tag_id) = tag_id {
    for item_id in item_ids {
      infra::assign(&db, entity_type, item_id, tag_id).await.ok();
    }
  }
  reload_asset_tags(db).await
}

/// Depth-first walk of the inventory order: a row, then any loaded children of an expanded container.
/// Mirrors `inventory::flatten_rows` so the selection's range/highlight resolve against the same order
/// the table renders.
fn push_inventory_order(state: &State, out: &mut Vec<i64>, row: &InventoryRow) {
  out.push(row.item_id);
  if row.is_container
    && state.expanded_containers.contains(&row.item_id)
    && let Some(children) = state.inventory_children.get(&row.item_id)
  {
    for child in children {
      push_inventory_order(state, out, child);
    }
  }
}

fn location_ids_for_selection(tree: &GeoTree, selection: GeoSelection) -> Vec<i64> {
  match selection {
    GeoSelection::All => Vec::new(),
    GeoSelection::Location(location_id) => vec![location_id],
    GeoSelection::Region(region_id) => tree
      .regions
      .iter()
      .find(|region| region.region_id == region_id)
      .map(|region| {
        region
          .constellations
          .iter()
          .flat_map(|constellation| &constellation.systems)
          .flat_map(|system| &system.locations)
          .map(|location| location.location_id)
          .collect()
      })
      .unwrap_or_default(),
    GeoSelection::Constellation(constellation_id) => tree
      .regions
      .iter()
      .flat_map(|region| &region.constellations)
      .find(|constellation| constellation.constellation_id == constellation_id)
      .map(|constellation| {
        constellation
          .systems
          .iter()
          .flat_map(|system| &system.locations)
          .map(|location| location.location_id)
          .collect()
      })
      .unwrap_or_default(),
    GeoSelection::System(system_id) => tree
      .regions
      .iter()
      .flat_map(|region| &region.constellations)
      .flat_map(|constellation| &constellation.systems)
      .find(|system| system.system_id == system_id)
      .map(|system| system.locations.iter().map(|location| location.location_id).collect())
      .unwrap_or_default(),
  }
}

fn update_pagination(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::InventoryScrolled {
      absolute,
      relative,
    } => {
      state.inventory_scroll_offset = absolute;
      if relative < INVENTORY_SCROLL_THRESHOLD || !state.inventory_has_more || state.inventory_loading {
        return Task::none();
      }
      let Some(cursor) = state.inventory.last().map(|row| row.cursor(state.sort)) else {
        return Task::none();
      };
      state.inventory_loading = true;
      let view = InventoryView::from_state(state);
      let epoch = state.inventory_page_epoch.current();
      let (db, scope, roster, corporations) = (
        db.clone(),
        state.active,
        state.roster.clone(),
        state.corporations.clone(),
      );
      Task::perform(
        async move { load_inventory_page(&db, scope, &roster, &corporations, &view, cursor).await },
        move |rows| Message::InventoryPageLoaded {
          epoch,
          rows,
        },
      )
    }
    Message::InventoryPageLoaded {
      epoch,
      rows,
    } => {
      if !state.inventory_page_epoch.matches(epoch) {
        return Task::none();
      }
      state.inventory_loading = false;
      state.inventory_has_more = rows.len() as i64 == INVENTORY_PAGE_SIZE;
      state.inventory.extend(rows);
      Task::none()
    }
    Message::ContainerToggled(item_id) => {
      if state.expanded_containers.remove(&item_id) {
        state.inventory_children.remove(&item_id);
        // The collapsed container's children left the loaded rows, so drop them from the selection.
        prune_inventory_selection(state);
        return Task::none();
      }
      state.expanded_containers.insert(item_id);
      if state.inventory_children.contains_key(&item_id) {
        return Task::none();
      }
      let (db, scope, roster, corporations) = (
        db.clone(),
        state.active,
        state.roster.clone(),
        state.corporations.clone(),
      );
      let filter = effective_filter(state.category, &state.search);
      Task::perform(
        async move { load_container_children(&db, scope, &roster, &corporations, item_id, &filter).await },
        move |children| Message::ContainerChildrenLoaded(item_id, children),
      )
    }
    Message::ContainerChildrenLoaded(item_id, children) => {
      if state.expanded_containers.contains(&item_id) {
        state.inventory_children.insert(item_id, children);
      }
      Task::none()
    }
    _ => Task::none(),
  }
}

fn update_stockpile(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  update_stockpile_lifecycle(state, message, db)
}

fn update_stockpile_lifecycle(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::StockpileDeleted(id) => {
      state.stockpile_context_menu = None;
      let db = db.clone();
      Task::perform(
        async move {
          stockpiles::delete(&db, id).await;
          stockpiles::load_cards(&db).await
        },
        Message::StockpilesReloaded,
      )
    }
    Message::StockpilesReloaded(cards) => {
      state.stockpiles = cards;
      Task::none()
    }
    Message::StockpileCursorMoved(point) => {
      state.stockpile_cursor = Some(point);
      Task::none()
    }
    Message::StockpileCardRightPressed(id) => {
      if let (Some(anchor), Some(card)) = (state.stockpile_cursor, state.stockpiles.iter().find(|c| c.id == id)) {
        state.stockpile_context_menu = Some(StockpileContextMenu {
          anchor,
          id,
          name: card.name.clone(),
        });
      }
      Task::none()
    }
    Message::StockpileContextMenuClosed => {
      state.stockpile_context_menu = None;
      Task::none()
    }
    Message::StockpileItemsToggled(id) => {
      if !state.stockpile_expanded.remove(&id) {
        state.stockpile_expanded.insert(id);
      }
      Task::none()
    }
    Message::StockpileMultibuyExportOpened(id) => {
      state.stockpile_context_menu = None;
      state.stockpile_multibuy_export = Some(id);
      state.stockpile_multibuy_copied = false;
      state.stockpile_multibuy_mode = stockpiles::MultibuyMode::default();
      Task::none()
    }
    Message::StockpileMultibuyModeChanged(mode) => {
      state.stockpile_multibuy_mode = mode;
      state.stockpile_multibuy_copied = false;
      Task::none()
    }
    Message::StockpileMultibuyExportClosed => {
      state.stockpile_multibuy_export = None;
      state.stockpile_multibuy_copied = false;
      Task::none()
    }
    Message::StockpileMultibuyExportCopied(id) => {
      let Some(card) = state.stockpiles.iter().find(|card| card.id == id) else {
        return Task::none();
      };
      let text = stockpile_multibuy::serialize(&card.multibuy_lines(state.stockpile_multibuy_mode));
      state.stockpile_multibuy_copied = true;
      iced::clipboard::write(text)
    }
    _ => Task::none(),
  }
}

fn update_abyssal(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::AbyssalPickerToggled => {
      state.abyssal_picker_open = !state.abyssal_picker_open;
      Task::none()
    }
    Message::AbyssalTypeModalOpened => {
      state.abyssal_picker_open = true;
      Task::none()
    }
    Message::AbyssalTypeModalClosed => {
      state.abyssal_picker_open = false;
      Task::none()
    }
    Message::AbyssalSourceTypeSelected(type_id) => {
      if state.abyssal_filters.source_type_id != type_id {
        state.abyssal_filters.stat_ranges.clear();
      }
      state.abyssal_filters.source_type_id = type_id;
      state.abyssal_picker_open = false;
      state.abyssal_slider_edit = None;
      state.abyssal_stat_templates = Vec::new();
      Task::batch([
        load_abyssal_templates(state, db, type_id),
        reload_abyssal_cards(state, db),
      ])
    }
    Message::AbyssalStatRangeChanged(attribute_id, endpoint, value) => {
      let (min_value, max_value) = endpoint_values(endpoint, value);
      apply_stat_range_and_reload(state, db, attribute_id, min_value, max_value)
    }
    Message::AbyssalStatMinChanged(attribute_id, value) => {
      apply_stat_range_and_reload(state, db, attribute_id, Some(value), None)
    }
    Message::AbyssalStatMaxChanged(attribute_id, value) => {
      apply_stat_range_and_reload(state, db, attribute_id, None, Some(value))
    }
    Message::AbyssalMutaMarketOpened(item_id) => {
      let _ = open::that_detached(format!("{MUTAMARKET_MODULE_URL}/{item_id}"));
      Task::none()
    }
    Message::AbyssalFilterReset => {
      state.abyssal_filters = abyssals::Filters::default();
      state.abyssal_slider_edit = None;
      state.abyssal_slider_edit_text = String::new();
      state.abyssal_stat_templates = Vec::new();
      reload_abyssal_cards(state, db)
    }
    Message::AbyssalGridScrolled {
      absolute,
      relative,
    } => {
      state.abyssal_scroll_offset = absolute;
      if relative < ABYSSAL_SCROLL_THRESHOLD || !state.abyssal_has_more || state.abyssal_loading {
        return Task::none();
      }
      let Some(cursor) = state.abyssals.last().map(abyssals::AbyssalCard::cursor) else {
        return Task::none();
      };
      state.abyssal_loading = true;
      load_abyssal_page(state, db, cursor)
    }
    Message::AbyssalPageLoaded {
      cards,
      epoch,
    } => {
      if !state.abyssal_page_epoch.matches(epoch) {
        return Task::none();
      }
      state.abyssal_loading = false;
      state.abyssal_has_more = cards.len() as i64 == abyssals::PAGE_SIZE;
      state.abyssals.extend(cards);
      // Deliberately leave abyssal_total alone: it is the filter-aware DB count, not the loaded
      // length, so appending a scroll page must not overwrite it.
      Task::none()
    }
    Message::AbyssalStatTemplatesLoaded(templates) => {
      state.abyssal_stat_templates = templates;
      Task::none()
    }
    Message::AbyssalCardsReloaded(reload) => {
      let abyssals::FilteredCards {
        cards,
        total,
      } = reload;
      state.abyssal_page_epoch.next();
      state.abyssal_has_more = cards.len() as i64 == abyssals::PAGE_SIZE;
      state.abyssal_loading = false;
      state.abyssal_scroll_offset = 0.0;
      state.abyssals = cards;
      state.abyssal_total = total;
      Task::none()
    }
    other => update_abyssal_slider(state, other, db),
  }
}

fn update_abyssal_slider(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::AbyssalSliderEditStarted(attribute_id, endpoint, value) => {
      state.abyssal_slider_edit = Some((attribute_id, endpoint));
      state.abyssal_slider_edit_text = format!("{value:.2}");
      Task::none()
    }
    Message::AbyssalSliderEditInput(text) => {
      state.abyssal_slider_edit_text = text;
      Task::none()
    }
    Message::AbyssalSliderEditCommitted(attribute_id, endpoint) => {
      let parsed = state.abyssal_slider_edit_text.trim().parse::<f64>().ok();
      state.abyssal_slider_edit = None;
      state.abyssal_slider_edit_text = String::new();
      let Some(value) = parsed else {
        return Task::none();
      };
      let (min_value, max_value) = endpoint_values(endpoint, value);
      apply_stat_range_and_reload(state, db, attribute_id, min_value, max_value)
    }
    _ => Task::none(),
  }
}

fn endpoint_values(endpoint: SliderEndpoint, value: f64) -> (Option<f64>, Option<f64>) {
  match endpoint {
    SliderEndpoint::Min => (Some(value), None),
    SliderEndpoint::Max => (None, Some(value)),
  }
}

fn apply_stat_range_and_reload(
  state: &mut State,
  db: &Database,
  attribute_id: i64,
  min_value: Option<f64>,
  max_value: Option<f64>,
) -> Task<Message> {
  if apply_abyssal_stat_range(state, attribute_id, min_value, max_value) {
    reload_abyssal_cards(state, db)
  } else {
    Task::none()
  }
}

fn apply_abyssal_stat_range(
  state: &mut State,
  attribute_id: i64,
  min_value: Option<f64>,
  max_value: Option<f64>,
) -> bool {
  let Some((lo, hi)) = state
    .abyssal_stat_templates
    .iter()
    .find(|template| template.attribute_id == attribute_id)
    .map(|template| (template.bound_lo, template.bound_hi))
  else {
    return false;
  };
  let (mut min, mut max) = state
    .abyssal_filters
    .stat_ranges
    .get(&attribute_id)
    .copied()
    .unwrap_or((lo, hi));
  if let Some(value) = min_value {
    min = value.clamp(lo, max);
  }
  if let Some(value) = max_value {
    max = value.clamp(min, hi);
  }
  if min <= lo + 1e-6 && max >= hi - 1e-6 {
    state.abyssal_filters.stat_ranges.remove(&attribute_id);
  } else {
    state.abyssal_filters.stat_ranges.insert(attribute_id, (min, max));
  }
  true
}

fn load_abyssal_templates(state: &State, db: &Database, type_id: Option<i64>) -> Task<Message> {
  let Some(type_id) = type_id else {
    return Task::done(Message::AbyssalStatTemplatesLoaded(Vec::new()));
  };
  let (db, scope, roster) = (db.clone(), state.active, state.roster.clone());
  Task::perform(
    async move { abyssals::load_stat_templates(&db, scope, &roster, type_id).await },
    Message::AbyssalStatTemplatesLoaded,
  )
}

fn reload_abyssal_cards(state: &State, db: &Database) -> Task<Message> {
  let (db, scope, roster, filters) = (
    db.clone(),
    state.active,
    state.roster.clone(),
    state.abyssal_filters.clone(),
  );
  Task::perform(
    async move { abyssals::load_filtered_cards(&db, scope, &roster, &filters).await },
    Message::AbyssalCardsReloaded,
  )
}

/// Fetch the next cursor-delimited page of abyssal cards under the active filters.
fn load_abyssal_page(state: &State, db: &Database, cursor: abyssals::AbyssalCursor) -> Task<Message> {
  let epoch = state.abyssal_page_epoch.current();
  let (db, scope, roster, filters) = (
    db.clone(),
    state.active,
    state.roster.clone(),
    state.abyssal_filters.clone(),
  );
  Task::perform(
    async move { abyssals::load_filtered_page(&db, scope, &roster, &filters, Some(cursor)).await },
    move |cards| Message::AbyssalPageLoaded {
      cards,
      epoch,
    },
  )
}

fn update_pane(state: &mut State, message: Message) -> Task<Message> {
  match message {
    Message::PaneDragStart(pane) => {
      state.pane_mut(pane).0.start();
      Task::none()
    }
    Message::PaneDrag(x) => {
      if let Some(pane) = state.active_drag() {
        state.pane_mut(pane).0.drag_to(x);
      }
      Task::none()
    }
    Message::PaneDragEnd => {
      let Some(pane) = state.active_drag() else {
        return Task::none();
      };
      let (drag, key) = state.pane_mut(pane);
      drag.end();
      let ratio = drag.ratio();
      Task::done(Message::PaneSettled(key, ratio))
    }
    _ => Task::none(),
  }
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  let mut subs: Vec<iced::Subscription<Message>> = Vec::new();
  if state.active_drag().is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      resizable_pane::drag_event(event, Message::PaneDrag, Message::PaneDragEnd)
    }));
  }
  // Track keyboard modifiers on the Inventory tab so a row click resolves its plain/range/toggle
  // intent, mirroring the wallet ledger.
  if state.tab == Tab::Inventory {
    subs.push(iced::event::listen_with(|event, _status, _id| match event {
      iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(modifiers)) => {
        Some(Message::InventoryModifiersChanged(modifiers))
      }
      _ => None,
    }));
  }
  iced::Subscription::batch(subs)
}

pub fn view(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  shell::shell(state, now)
}

pub fn escape_dismiss(state: &State) -> Option<Message> {
  shell::escape_dismiss(state)
}

async fn load_assets(db: Database, scope: Scope, view: InventoryView) -> Loaded {
  let roster = load_roster(&db).await;
  let corporations = load_corporations(&db).await;

  let ScopeLoad {
    totals,
    inventory,
    expand_containers,
    inventory_children,
  } = match resolve_scope_owner(scope, &roster, &corporations) {
    Some(owner) => load_scope(&db, &owner, &view).await,
    None => ScopeLoad::default(),
  };
  let geo_tree = tree::load_geo_tree(&db, scope, &roster, &corporations).await;
  let values = values::summarize(&inventory, totals.value, &roster, &corporations);
  let nav = tracker::load_series(&db, scope).await;
  let stockpiles = stockpiles::load_cards(&db).await;
  let abyssals = abyssals::load_cards(&db, scope, &roster).await;
  let saved_filters = assets::saved_filters(&db).await.unwrap_or_default();
  let asset_tags = infra::tag_all_scoped(&db, TAG_SCOPE_ASSET).await.unwrap_or_default();
  let tag_memberships = infra::membership_map(&db, ENTITY_TYPE_ASSET).await.unwrap_or_default();

  Loaded {
    asset_tags,
    corporations,
    expand_containers,
    geo_tree,
    inventory,
    inventory_children,
    roster,
    saved_filters,
    tag_memberships,
    totals,
    values,
    nav,
    stockpiles,
    abyssals,
  }
}

/// Reloads only the asset-tag registry and membership map after a modal write, so chips refresh without
/// a full inventory refetch.
async fn reload_asset_tags(db: Database) -> Message {
  let tags = infra::tag_all_scoped(&db, TAG_SCOPE_ASSET).await.unwrap_or_default();
  let memberships = infra::membership_map(&db, ENTITY_TYPE_ASSET).await.unwrap_or_default();
  Message::AssetTagsReloaded {
    memberships,
    tags,
  }
}

/// The per-scope inventory load: the filtered page plus, when a filter is active, the ancestor
/// containers that hold nested matches (so they can be auto-expanded to reveal those matches).
#[derive(Default)]
struct ScopeLoad {
  /// Container ids on the path to a match, shallow-to-deep. Empty when no filter is active.
  expand_containers: Vec<i64>,
  inventory: Vec<InventoryRow>,
  /// Pre-loaded children of each `expand_containers` entry, keyed by container `item_id`.
  inventory_children: HashMap<i64, Vec<InventoryRow>>,
  totals: InventoryTotals,
}

async fn load_scope(db: &Database, owner: &Owner, view: &InventoryView) -> ScopeLoad {
  let me_id = match owner {
    Owner::Character(id) => Some(*id),
    Owner::Combined {
      ..
    }
    | Owner::Corporation(_) => None,
  };
  let query = InventoryQuery {
    cursor: None,
    direction: view.sort_dir,
    filter: &view.filter,
    limit: view.limit,
    location_ids: &view.location_ids,
    me_id,
    reproc_yield: crate::config::reprocessing_yield_or_default(),
    sort: view.sort,
  };

  let (totals, mut inventory) = match owner {
    Owner::Character(id) => {
      let totals = assets::inventory_totals_for_character(db, *id, &view.filter, &view.location_ids, me_id)
        .await
        .unwrap_or_default();
      let inventory = assets::inventory_page_for_character(db, *id, &query)
        .await
        .unwrap_or_default();
      (totals, inventory)
    }
    Owner::Combined {
      character_ids,
      corporation_ids,
    } => {
      let totals = assets::inventory_totals_for_combined(
        db,
        character_ids,
        corporation_ids,
        &view.filter,
        &view.location_ids,
        None,
      )
      .await
      .unwrap_or_default();
      let inventory = assets::inventory_page_for_combined(db, character_ids, corporation_ids, &query)
        .await
        .unwrap_or_default();
      (totals, inventory)
    }
    Owner::Corporation(id) => {
      let totals = assets::inventory_totals_for_corporation(db, *id, &view.filter, &view.location_ids, None)
        .await
        .unwrap_or_default();
      let inventory = assets::inventory_page_for_corporation(db, *id, &query)
        .await
        .unwrap_or_default();
      (totals, inventory)
    }
  };

  // No filter ⇒ normal browse; nothing nested to surface, so skip the ancestor work entirely.
  if view.filter.trim().is_empty() {
    return ScopeLoad {
      expand_containers: Vec::new(),
      inventory,
      inventory_children: HashMap::new(),
      totals,
    };
  }

  let (expand_containers, inventory_children, injected_roots) =
    load_match_ancestors(db, owner, &view.filter, me_id).await;

  // The filtered page query only returns top-level matching rows (container_id IS NULL), so a
  // top-level ancestor container that does not itself match was dropped. Inject those root rows back
  // in so the chain has a visible parent to hang under, deduping against rows already present.
  let present: HashSet<i64> = inventory.iter().map(|row| row.item_id).collect();
  for row in injected_roots {
    if !present.contains(&row.item_id) {
      inventory.push(row);
    }
  }

  ScopeLoad {
    expand_containers,
    inventory,
    inventory_children,
    totals,
  }
}

/// Resolves the set of container ids on the path from the top level down to any filter match,
/// deduped across a combined scope's characters and corporations.
async fn resolve_match_ancestors(db: &Database, owner: &Owner, filter: &str, me_id: Option<i64>) -> Vec<i64> {
  match owner {
    Owner::Character(id) => assets::ancestors_of_match_for_character(db, *id, filter, me_id)
      .await
      .unwrap_or_default(),
    Owner::Combined {
      character_ids,
      corporation_ids,
    } => {
      let mut ancestors = assets::ancestors_of_match_for_characters(db, character_ids, filter, me_id)
        .await
        .unwrap_or_default();
      for corporation_id in corporation_ids {
        ancestors.extend(
          assets::ancestors_of_match_for_corporation(db, *corporation_id, filter, me_id)
            .await
            .unwrap_or_default(),
        );
      }
      ancestors.sort_unstable();
      ancestors.dedup();
      ancestors
    }
    Owner::Corporation(id) => assets::ancestors_of_match_for_corporation(db, *id, filter, me_id)
      .await
      .unwrap_or_default(),
  }
}

/// Computes the ancestor-container expansion for an active filter: the ordered set of container ids
/// on the path to any match, their pre-loaded children, and the top-level ancestor rows to inject
/// back into the page (the rows the filtered query dropped because they do not match themselves).
async fn load_match_ancestors(
  db: &Database,
  owner: &Owner,
  filter: &str,
  me_id: Option<i64>,
) -> (Vec<i64>, HashMap<i64, Vec<InventoryRow>>, Vec<InventoryRow>) {
  let reproc_yield = crate::config::reprocessing_yield_or_default();

  let ancestors = resolve_match_ancestors(db, owner, filter, me_id).await;

  if ancestors.is_empty() {
    return (Vec::new(), HashMap::new(), Vec::new());
  }

  // Fetch the ancestor rows themselves (ignoring the filter) so we can both inject the top-level
  // ones into the page and order the expansion shallow-to-deep.
  let ancestor_rows = rows_by_item_id(db, owner, &ancestors, reproc_yield).await;
  let injected_roots: Vec<InventoryRow> = ancestor_rows
    .iter()
    .filter(|row| row.container_id.is_none())
    .cloned()
    .collect();

  // Load every ancestor's children pruned to the page filter, keeping path containers (the ancestor
  // set itself) so deeper matches stay reachable while non-matching siblings are dropped.
  let child_filter = ChildFilter {
    filter,
    me_id,
    path_container_ids: &ancestors,
    reproc_yield,
  };
  let mut inventory_children = HashMap::with_capacity(ancestors.len());
  for &container_id in &ancestors {
    let children = load_container_children_for_owner(db, owner, container_id, &child_filter).await;
    inventory_children.insert(container_id, children);
  }

  // Order expansion shallow-to-deep by container_id chain depth so a parent is always seeded before
  // its descendants; a row's depth field is authoritative for nesting order.
  let depth_of: HashMap<i64, i64> = ancestor_rows.iter().map(|row| (row.item_id, row.depth)).collect();
  let mut expand_containers = ancestors;
  expand_containers.sort_by_key(|id| depth_of.get(id).copied().unwrap_or(0));

  (expand_containers, inventory_children, injected_roots)
}

/// Fetches inventory rows for an explicit `item_id` set within a scope, ignoring any filter.
async fn rows_by_item_id(db: &Database, owner: &Owner, item_ids: &[i64], reproc_yield: f64) -> Vec<InventoryRow> {
  match owner {
    Owner::Character(id) => assets::rows_by_item_id_for_character(db, *id, item_ids, reproc_yield)
      .await
      .unwrap_or_default(),
    Owner::Combined {
      character_ids,
      corporation_ids,
    } => {
      let mut rows = assets::rows_by_item_id_for_characters(db, character_ids, item_ids, reproc_yield)
        .await
        .unwrap_or_default();
      for corporation_id in corporation_ids {
        rows.extend(
          assets::rows_by_item_id_for_corporation(db, *corporation_id, item_ids, reproc_yield)
            .await
            .unwrap_or_default(),
        );
      }
      rows
    }
    Owner::Corporation(id) => assets::rows_by_item_id_for_corporation(db, *id, item_ids, reproc_yield)
      .await
      .unwrap_or_default(),
  }
}

/// Fetches a container's children for a scope pruned to the active filter, keeping the path
/// containers carried in `child_filter` so deeper matches stay reachable (mirrors
/// `load_container_children` but keyed on a resolved `Owner` so the ancestor loader can reuse it).
async fn load_container_children_for_owner(
  db: &Database,
  owner: &Owner,
  container_id: i64,
  child_filter: &ChildFilter<'_>,
) -> Vec<InventoryRow> {
  match owner {
    Owner::Character(id) => assets::children_render_filtered_for_character(db, *id, container_id, child_filter)
      .await
      .unwrap_or_default(),
    Owner::Combined {
      character_ids,
      corporation_ids,
    } => {
      let mut children = assets::children_render_filtered_for_characters(db, character_ids, container_id, child_filter)
        .await
        .unwrap_or_default();
      for corporation_id in corporation_ids {
        children.extend(
          assets::children_render_filtered_for_corporation(db, *corporation_id, container_id, child_filter)
            .await
            .unwrap_or_default(),
        );
      }
      children
    }
    Owner::Corporation(id) => assets::children_render_filtered_for_corporation(db, *id, container_id, child_filter)
      .await
      .unwrap_or_default(),
  }
}

async fn load_inventory_page(
  db: &Database,
  scope: Scope,
  roster: &[RosterPilot],
  corporations: &[RosterCorp],
  view: &InventoryView,
  cursor: InventoryCursor,
) -> Vec<InventoryRow> {
  let Some(owner) = resolve_scope_owner(scope, roster, corporations) else {
    return Vec::new();
  };
  let me_id = match &owner {
    Owner::Character(id) => Some(*id),
    Owner::Combined {
      ..
    }
    | Owner::Corporation(_) => None,
  };
  let query = InventoryQuery {
    cursor: Some(cursor),
    direction: view.sort_dir,
    filter: &view.filter,
    limit: INVENTORY_PAGE_SIZE,
    location_ids: &view.location_ids,
    me_id,
    reproc_yield: crate::config::reprocessing_yield_or_default(),
    sort: view.sort,
  };
  match &owner {
    Owner::Character(id) => assets::inventory_page_for_character(db, *id, &query).await,
    Owner::Combined {
      character_ids,
      corporation_ids,
    } => assets::inventory_page_for_combined(db, character_ids, corporation_ids, &query).await,
    Owner::Corporation(id) => assets::inventory_page_for_corporation(db, *id, &query).await,
  }
  .unwrap_or_default()
}

async fn load_container_children(
  db: &Database,
  scope: Scope,
  roster: &[RosterPilot],
  corporations: &[RosterCorp],
  container_id: i64,
  filter: &str,
) -> Vec<InventoryRow> {
  let Some(owner) = resolve_scope_owner(scope, roster, corporations) else {
    return Vec::new();
  };
  let reproc_yield = crate::config::reprocessing_yield_or_default();
  // With no filter, manual expansion lists every child. While a filter is active it stays in view
  // mode: only matching children plus the path containers that lead to deeper matches.
  if filter.trim().is_empty() {
    return load_container_children_unfiltered(db, &owner, container_id, reproc_yield).await;
  }
  let me_id = match &owner {
    Owner::Character(id) => Some(*id),
    Owner::Combined {
      ..
    }
    | Owner::Corporation(_) => None,
  };
  let path_container_ids = resolve_match_ancestors(db, &owner, filter, me_id).await;
  let child_filter = ChildFilter {
    filter,
    me_id,
    path_container_ids: &path_container_ids,
    reproc_yield,
  };
  load_container_children_for_owner(db, &owner, container_id, &child_filter).await
}

/// Fetches a container's full children for a scope, ignoring any filter (the no-filter manual-expand
/// path; the filtered counterpart is `load_container_children_for_owner`).
async fn load_container_children_unfiltered(
  db: &Database,
  owner: &Owner,
  container_id: i64,
  reproc_yield: f64,
) -> Vec<InventoryRow> {
  match owner {
    Owner::Character(id) => assets::children_render_for_character(db, *id, container_id, reproc_yield)
      .await
      .unwrap_or_default(),
    Owner::Combined {
      character_ids,
      corporation_ids,
    } => {
      let mut children = assets::children_render_for_characters(db, character_ids, container_id, reproc_yield)
        .await
        .unwrap_or_default();
      for corporation_id in corporation_ids {
        children.extend(
          assets::children_render_for_corporation(db, *corporation_id, container_id, reproc_yield)
            .await
            .unwrap_or_default(),
        );
      }
      children
    }
    Owner::Corporation(id) => assets::children_render_for_corporation(db, *id, container_id, reproc_yield)
      .await
      .unwrap_or_default(),
  }
}

async fn load_roster(db: &Database) -> Vec<RosterPilot> {
  let characters = character::all_owned(db).await.unwrap_or_default();
  let credentials = infra::all(db).await.unwrap_or_default();
  let scopes_by_id: std::collections::HashMap<i64, Option<String>> = credentials
    .into_iter()
    .filter(|cred| cred.owner_type() == OwnerType::Character)
    .map(|cred| (cred.owner_id(), cred.scopes().clone()))
    .collect();

  let mut roster = Vec::with_capacity(characters.len());
  for character in &characters {
    let corp = org::get_corporation(db, character.corporation_id())
      .await
      .ok()
      .flatten()
      .map(|c| c.ticker().to_owned())
      .unwrap_or_default();
    let portrait = images::resolve(
      &images::default_store(),
      images::ImageKind::CharacterPortrait,
      character.id(),
    );
    roster.push(RosterPilot {
      corp,
      granted_scopes: scopes_by_id.get(&character.id()).cloned().flatten(),
      id: character.id(),
      name: character.name().to_owned(),
      portrait,
    });
  }
  roster
}

async fn load_corporations(db: &Database) -> Vec<RosterCorp> {
  org::all_owned_corporations(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|corp| RosterCorp {
      id: corp.id(),
      logo: images::resolve(&images::default_store(), images::ImageKind::CorporationLogo, corp.id()),
      name: corp.name().to_owned(),
      ticker: corp.ticker().to_owned(),
    })
    .collect()
}

pub(super) fn owner_label(owner_id: i64, roster: &[RosterPilot], corporations: &[RosterCorp]) -> String {
  roster
    .iter()
    .find(|pilot| pilot.id == owner_id)
    .map(|pilot| pilot.name.clone())
    .or_else(|| {
      corporations
        .iter()
        .find(|corp| corp.id == owner_id)
        .map(|corp| corp.name.clone())
    })
    .unwrap_or_else(|| t!("assets.inventory.owner_fallback", id => fmt_count(owner_id)).into_owned())
}

#[cfg(test)]
mod tests {
  use super::*;

  mod enabled_tabs {
    use pretty_assertions::assert_eq;

    use super::*;

    fn only(sub: crate::config::SubFeature) -> crate::config::FeatureFlags {
      let mut flags = crate::config::FeatureFlags::default();
      for candidate in crate::config::SubFeature::ALL {
        flags.set_sub_enabled(candidate, candidate == sub);
      }
      flags
    }

    #[test]
    fn it_keeps_strip_order_with_every_sub_feature_enabled() {
      let tabs = enabled_tabs(&crate::config::FeatureFlags::default());

      assert_eq!(
        tabs,
        vec![
          Tab::Inventory,
          Tab::Abyssals,
          Tab::Stockpiles,
          Tab::Values,
          Tab::Tracker
        ]
      );
    }

    #[test]
    fn it_drops_a_disabled_sub_feature_from_the_strip() {
      let tabs = enabled_tabs(&only(crate::config::SubFeature::Values));

      assert_eq!(tabs, vec![Tab::Values]);
    }
  }

  mod tab {
    use super::*;

    mod id {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_round_trips_every_tab_through_its_catalog_id() {
        assert_eq!(Tab::Abyssals.id(), "abyssals");
        assert_eq!(Tab::Inventory.id(), "inventory");
        assert_eq!(Tab::Stockpiles.id(), "stockpiles");
        assert_eq!(Tab::Tracker.id(), "tracker");
        assert_eq!(Tab::Values.id(), "values");

        for tab in Tab::ORDER {
          assert_eq!(Tab::from_id(tab.id()), Some(tab));
        }
      }
    }
  }

  mod sync_features {
    use pretty_assertions::assert_eq;

    use super::*;

    fn without(sub: crate::config::SubFeature) -> crate::config::FeatureFlags {
      let mut flags = crate::config::FeatureFlags::default();
      flags.set_sub_enabled(sub, false);
      flags
    }

    #[test]
    fn it_redirects_off_a_disabled_active_tab_to_the_first_enabled_tab() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.tab = Tab::Inventory;

      state.sync_features(without(crate::config::SubFeature::Inventory));

      assert_eq!(state.tab, Tab::Abyssals);
    }

    #[test]
    fn it_keeps_a_still_enabled_active_tab() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.tab = Tab::Tracker;

      state.sync_features(without(crate::config::SubFeature::Inventory));

      assert_eq!(state.tab, Tab::Tracker);
    }
  }

  fn pilot(id: i64) -> RosterPilot {
    RosterPilot {
      corp: "TST".to_owned(),
      granted_scopes: None,
      id,
      name: format!("Pilot {id}"),
      portrait: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      },
    }
  }

  fn corp(id: i64) -> RosterCorp {
    RosterCorp {
      id,
      logo: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CorporationLogo,
      },
      name: format!("Corp {id}"),
      ticker: "CRP".to_owned(),
    }
  }

  mod effective_filter {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_passes_the_search_through_untouched_for_the_all_category() {
      assert_eq!(effective_filter(Category::All, "name:Rifter"), "name:Rifter");
      assert_eq!(effective_filter(Category::All, ""), "");
    }

    #[test]
    fn it_prefixes_a_category_clause_when_a_category_is_active() {
      assert_eq!(effective_filter(Category::Ship, ""), "category:ship");
      assert_eq!(
        effective_filter(Category::Module, "name:Rifter"),
        "category:module name:Rifter"
      );
    }
  }

  mod geo {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::asset_query::{GeoConstellationNode, GeoLocationNode, GeoRegionNode, GeoSystemNode};

    fn location(location_id: i64, value: f64) -> GeoLocationNode {
      GeoLocationNode {
        item_count: 1,
        location_id,
        location_label: Some(format!("Station {location_id}")),
        location_type: "station".to_owned(),
        value,
      }
    }

    fn geo_tree() -> GeoTree {
      GeoTree {
        orphans: Vec::new(),
        regions: vec![GeoRegionNode {
          constellations: vec![GeoConstellationNode {
            constellation_id: 20,
            constellation_name: "Kimotoro".to_owned(),
            item_count: 6,
            systems: vec![
              GeoSystemNode {
                item_count: 4,
                locations: vec![location(60_000_001, 100.0), location(60_000_002, 50.0)],
                security_status: Some(0.9),
                system_id: 30,
                system_name: "Jita".to_owned(),
                value: 150.0,
              },
              GeoSystemNode {
                item_count: 2,
                locations: vec![location(60_000_003, 25.0)],
                security_status: Some(0.9),
                system_id: 31,
                system_name: "Perimeter".to_owned(),
                value: 25.0,
              },
            ],
            value: 175.0,
          }],
          item_count: 6,
          region_id: 10,
          region_name: "The Forge".to_owned(),
          value: 175.0,
        }],
      }
    }

    fn sorted(mut ids: Vec<i64>) -> Vec<i64> {
      ids.sort_unstable();
      ids
    }

    #[tokio::test]
    async fn it_records_a_geo_selection() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_geo_tree_for_test(geo_tree());

      let _ = update(&mut state, Message::GeoNodeSelected(GeoSelection::System(30)), &db);

      assert_eq!(state.geo_selected(), GeoSelection::System(30));
    }

    #[tokio::test]
    async fn it_resets_the_geo_selection_and_collapse_state_when_the_scope_changes() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.geo_selected = GeoSelection::System(30);
      state.geo_expanded.insert(GeoNodeKey::Region(10));

      let _ = update(&mut state, Message::ScopeSelected(Scope::Character(7)), &db);

      assert_eq!(state.geo_selected(), GeoSelection::All);
      assert!(
        state.geo_is_collapsed(GeoNodeKey::Region(10)),
        "the scope change re-collapses every group"
      );
    }

    #[test]
    fn it_resolves_an_unknown_selection_to_no_location_ids() {
      let tree = geo_tree();

      assert!(location_ids_for_selection(&tree, GeoSelection::Region(999)).is_empty());
      assert!(location_ids_for_selection(&tree, GeoSelection::System(999)).is_empty());
    }

    #[test]
    fn it_resolves_each_selection_level_to_its_location_ids() {
      let tree = geo_tree();

      assert!(location_ids_for_selection(&tree, GeoSelection::All).is_empty());
      assert_eq!(
        location_ids_for_selection(&tree, GeoSelection::Location(60_000_002)),
        [60_000_002]
      );
      assert_eq!(
        sorted(location_ids_for_selection(&tree, GeoSelection::System(30))),
        [60_000_001, 60_000_002]
      );
      assert_eq!(
        sorted(location_ids_for_selection(&tree, GeoSelection::Constellation(20))),
        [60_000_001, 60_000_002, 60_000_003]
      );
      assert_eq!(
        sorted(location_ids_for_selection(&tree, GeoSelection::Region(10))),
        [60_000_001, 60_000_002, 60_000_003]
      );
    }

    #[tokio::test]
    async fn it_toggles_a_group_expanded_then_collapsed_from_a_collapsed_default() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      let key = GeoNodeKey::Region(10);
      assert!(state.geo_is_collapsed(key), "groups render collapsed by default");

      let _ = update(&mut state, Message::GeoNodeToggled(key), &db);
      assert!(!state.geo_is_collapsed(key));

      let _ = update(&mut state, Message::GeoNodeToggled(key), &db);
      assert!(state.geo_is_collapsed(key));
    }
  }

  mod load {
    use super::*;

    #[tokio::test]
    async fn it_loads_an_empty_portfolio_for_a_fresh_db() {
      let db = crate::store::open_test().await.unwrap();

      let loaded = load_assets(db, Scope::All, InventoryView::default()).await;

      assert!(loaded.roster.is_empty());
      assert_eq!(loaded.totals, InventoryTotals::default());
    }
  }

  /// Production-path coverage for the filter auto-expand: a filtered load runs through the real
  /// `load_assets` -> `load_scope` -> `load_match_ancestors` chain, and the resulting `Loaded` drives
  /// `apply_loaded`/`merge_loaded` exactly as it would in the app.
  mod filter_expansion {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::{Alliance, Bloodline, Character, CharacterAsset, Corporation, Gender, ItemCategory, ItemGroup, Race},
      repo::{
        character::insert_with_org,
        sde::{upsert_item_category, upsert_item_group},
      },
    };

    const PILOT: i64 = 42;

    async fn seed_pilot(db: &Database) {
      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, PILOT, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(PILOT);
      corp.set_creator_id(PILOT);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(PILOT, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    async fn seed_item_type(db: &Database, type_id: i64, name: &str, group_id: i64, group_name: &str, cat: &str) {
      let category = ItemCategory {
        id: group_id * 10,
        icon_id: None,
        name: cat.to_owned(),
        published: true,
      };
      let group = ItemGroup {
        category_id: category.id(),
        icon_id: None,
        id: group_id,
        name: group_name.to_owned(),
        published: true,
      };
      upsert_item_category(db, &category).await.unwrap();
      upsert_item_group(db, &group).await.unwrap();
      sqlx::query(
        "INSERT INTO item_types (id, group_id, description, name, published, icon_id, packaged_volume, volume) \
        VALUES (?, ?, ?, ?, 1, ?, 2.5, 27289.0)",
      )
      .bind(type_id)
      .bind(group_id)
      .bind("Test item")
      .bind(name)
      .bind(type_id + 1000)
      .execute(db.writer())
      .await
      .unwrap();
    }

    fn asset(item_id: i64, container_id: Option<i64>, type_id: i64, is_container: bool, depth: i64) -> CharacterAsset {
      CharacterAsset {
        character_id: PILOT,
        container_id,
        depth,
        is_active_ship: false,
        is_blueprint_copy: None,
        is_container,
        is_singleton: false,
        item_id,
        location_flag: "Hangar".to_owned(),
        location_id: 60_003_760,
        location_type: "station".to_owned(),
        name: None,
        quantity: 1,
        type_id,
      }
    }

    // Seeds a character whose holdings are:
    //   - container 100 (root) -> container 101 (sub) -> Tritanium 102 (the nested match)
    //   - container 101 also holds Rifter 103 (a ship sibling of the match; never matches)
    //   - container 100 also holds Rifter 104 (a ship sibling of the path container; never matches)
    //   - container 200 (root) -> Rifter 201 (a ship; never matches `category:material`)
    //
    // Neither root container matches `category:material`, so the filtered page query drops both.
    async fn seed_nested_holdings(db: &Database) {
      seed_pilot(db).await;
      seed_item_type(db, 24, "Tritanium", 18, "Mineral", "Mineral").await;
      seed_item_type(db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let holdings = [
        asset(100, None, 587, true, 0),
        asset(101, Some(100), 587, true, 1),
        asset(102, Some(101), 24, false, 2),
        asset(103, Some(101), 587, false, 2),
        asset(104, Some(100), 587, false, 1),
        asset(200, None, 587, true, 0),
        asset(201, Some(200), 587, false, 1),
      ];
      assets::replace_for_character(db, PILOT, &holdings).await.unwrap();
    }

    fn filtered_view() -> InventoryView {
      InventoryView {
        filter: "category:material".to_owned(),
        ..InventoryView::default()
      }
    }

    #[tokio::test]
    async fn it_auto_expands_the_ancestor_chain_holding_a_nested_match() {
      let db = crate::store::open_test().await.unwrap();
      seed_nested_holdings(&db).await;

      let loaded = load_assets(db, Scope::Character(PILOT), filtered_view()).await;

      // The whole path to the deep match is expanded, shallow-to-deep.
      assert_eq!(loaded.expand_containers, [100, 101]);
      // The dropped top-level ancestor row was injected back into the page so the chain has a parent.
      assert!(
        loaded.inventory.iter().any(|row| row.item_id == 100),
        "the root ancestor row is re-injected into the page"
      );
      // The nested match is reachable: container 101 is a loaded child of 100, and the match 102 is a
      // loaded child of 101.
      let sub_in_root = loaded.inventory_children[&100].iter().any(|row| row.item_id == 101);
      let match_in_sub = loaded.inventory_children[&101].iter().any(|row| row.item_id == 102);
      assert!(sub_in_root, "the sub-container is a loaded child of the root");
      assert!(match_in_sub, "the match is a loaded child of the sub-container");
    }

    #[tokio::test]
    async fn it_prunes_non_matching_siblings_at_every_level_of_the_chain() {
      let db = crate::store::open_test().await.unwrap();
      seed_nested_holdings(&db).await;

      let loaded = load_assets(db, Scope::Character(PILOT), filtered_view()).await;

      // The root keeps only the path container 101; its non-matching ship sibling 104 is pruned.
      let root_children: Vec<i64> = loaded.inventory_children[&100].iter().map(|row| row.item_id).collect();
      assert_eq!(
        root_children,
        [101],
        "the root lists only the path container, not its ship sibling"
      );

      // The sub-container keeps only the match 102; its non-matching ship sibling 103 is pruned.
      let sub_children: Vec<i64> = loaded.inventory_children[&101].iter().map(|row| row.item_id).collect();
      assert_eq!(
        sub_children,
        [102],
        "the sub-container lists only the match, not its ship sibling"
      );
    }

    #[tokio::test]
    async fn it_leaves_a_container_without_a_match_collapsed() {
      let db = crate::store::open_test().await.unwrap();
      seed_nested_holdings(&db).await;

      let loaded = load_assets(db, Scope::Character(PILOT), filtered_view()).await;

      // Container 200 holds only a ship, so it must not be auto-expanded or its children pre-loaded.
      assert!(
        !loaded.expand_containers.contains(&200),
        "a container with no matching descendant stays collapsed"
      );
      assert!(!loaded.inventory_children.contains_key(&200));
    }

    #[tokio::test]
    async fn it_seeds_state_expansion_from_a_filtered_load() {
      let db = crate::store::open_test().await.unwrap();
      seed_nested_holdings(&db).await;
      let loaded = load_assets(db, Scope::Character(PILOT), filtered_view()).await;

      let mut state = State::new(crate::config::FeatureFlags::default());
      apply_loaded(&mut state, loaded);

      assert!(state.container_is_open(100), "the root ancestor is auto-expanded");
      assert!(state.container_is_open(101), "the sub-container is auto-expanded");
      assert!(
        !state.container_is_open(200),
        "the non-matching container stays collapsed"
      );
      assert_eq!(
        state
          .container_children_of(101)
          .map(|rows| rows.iter().any(|row| row.item_id == 102)),
        Some(true),
        "the nested match is reachable under its auto-expanded parent"
      );
    }

    #[tokio::test]
    async fn it_resets_expansion_when_the_filter_clears() {
      let db = crate::store::open_test().await.unwrap();
      seed_nested_holdings(&db).await;

      // First load with a filter active, seeding filter-driven expansions.
      let filtered = load_assets(db.clone(), Scope::Character(PILOT), filtered_view()).await;
      let mut state = State::new(crate::config::FeatureFlags::default());
      apply_loaded(&mut state, filtered);
      assert!(state.container_is_open(100), "the filter expanded the ancestor chain");

      // Clearing the filter reloads with no expansion set; apply must collapse back to normal browse.
      let unfiltered = load_assets(db, Scope::Character(PILOT), InventoryView::default()).await;
      assert!(unfiltered.expand_containers.is_empty());
      apply_loaded(&mut state, unfiltered);

      assert!(
        !state.container_is_open(100),
        "clearing the filter collapses the auto-expansion"
      );
      assert!(!state.container_is_open(101));
    }
  }

  mod mark_dirty {
    use super::*;

    #[test]
    fn it_ignores_an_unrelated_kind() {
      let mut state = State::new(crate::config::FeatureFlags::default());

      state.mark_dirty(JobKind::CharacterWallet);

      assert!(!state.is_dirty());
    }

    #[test]
    fn it_marks_the_assets_dirty_for_an_asset_sync() {
      let mut state = State::new(crate::config::FeatureFlags::default());

      state.mark_dirty(JobKind::AssetSync);

      assert!(state.is_dirty());
    }
  }

  mod pagination {
    use pretty_assertions::assert_eq;

    use super::*;

    fn inv_row(item_id: i64, is_container: bool) -> InventoryRow {
      InventoryRow {
        category: "ship".to_owned(),
        container_id: None,
        depth: 0,
        group_name: "Frigate".to_owned(),
        is_active_ship: false,
        is_blueprint_copy: None,
        is_container,
        item_id,
        location_id: 60_003_760,
        location_label: Some("Jita IV - Moon 4".to_owned()),
        name: None,
        owner_id: 7,
        quantity: 1,
        reproc_value: 0.0,
        row_volume: 10.0,
        type_icon: images::IconResolution::Missing,
        type_id: 587,
        type_name: "Rifter".to_owned(),
        unit_price: 100.0,
        value: 100.0,
      }
    }

    #[tokio::test]
    async fn it_appends_a_loaded_page_and_clears_has_more_for_a_short_page() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.inventory = vec![inv_row(100, false)];
      state.inventory_has_more = true;
      state.inventory_loading = true;

      let epoch = state.inventory_page_epoch.current();
      let _ = update(
        &mut state,
        Message::InventoryPageLoaded {
          epoch,
          rows: vec![inv_row(101, false)],
        },
        &db,
      );

      assert_eq!(
        state.inventory.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100, 101]
      );
      assert!(
        !state.inventory_has_more,
        "a page shorter than the page size is the last page"
      );
      assert!(!state.inventory_loading);
    }

    #[tokio::test]
    async fn it_drops_an_inventory_page_captured_before_the_list_was_replaced() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.inventory = vec![inv_row(100, false)];
      state.inventory_has_more = true;
      state.inventory_loading = true;

      // The user scrolled (capturing the current epoch) and then filtered/switched scope, which
      // replaces the list and bumps the epoch.
      let stale_epoch = state.inventory_page_epoch.current();
      state.inventory_page_epoch.next();
      state.inventory = vec![inv_row(200, false)];

      let _ = update(
        &mut state,
        Message::InventoryPageLoaded {
          epoch: stale_epoch,
          rows: vec![inv_row(101, false)],
        },
        &db,
      );

      assert_eq!(
        state.inventory.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [200],
        "a page from the superseded query must not append foreign rows"
      );
    }

    #[tokio::test]
    async fn it_drops_expansions_for_items_absent_from_fresh_sync_data() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_inventory_children_for_test(100, vec![inv_row(101, false)]);
      let generation = state.search_generation;

      let _ = update(
        &mut state,
        Message::SyncReloaded {
          generation,
          loaded: Box::new(Loaded {
            inventory: vec![inv_row(200, false)],
            ..Loaded::default()
          }),
        },
        &db,
      );

      assert!(
        !state.container_is_open(100),
        "an expanded container that vanished from fresh data is dropped"
      );
      assert!(state.container_children_of(100).is_none());
    }

    #[tokio::test]
    async fn it_expands_a_container_loads_children_then_collapse_drops_them() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.active = Scope::Character(7);

      let _ = update(&mut state, Message::ContainerToggled(100), &db);
      assert!(state.container_is_open(100));

      let _ = update(
        &mut state,
        Message::ContainerChildrenLoaded(100, vec![inv_row(101, false)]),
        &db,
      );
      assert_eq!(state.container_children_of(100).map(<[_]>::len), Some(1));

      let _ = update(&mut state, Message::ContainerToggled(100), &db);
      assert!(!state.container_is_open(100));
      assert!(
        state.container_children_of(100).is_none(),
        "collapse drops the loaded children"
      );
    }

    #[tokio::test]
    async fn it_ignores_a_stale_sync_reload_after_the_search_advanced() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      let stale_generation = state.search_generation;
      state.search_generation = state.search_generation.wrapping_add(1);
      state.totals.items = 3;

      let _ = update(
        &mut state,
        Message::SyncReloaded {
          generation: stale_generation,
          loaded: Box::new(Loaded {
            totals: InventoryTotals {
              items: 99,
              locations: 1,
              value: 0.0,
              volume: 0.0,
            },
            ..Loaded::default()
          }),
        },
        &db,
      );

      assert_eq!(state.totals.items, 3, "a stale sync reload is discarded");
    }

    #[tokio::test]
    async fn it_ignores_scroll_below_the_threshold() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.inventory = vec![inv_row(100, false)];
      state.inventory_has_more = true;

      let _ = update(
        &mut state,
        Message::InventoryScrolled {
          absolute: 0.0,
          relative: 0.5,
        },
        &db,
      );

      assert!(!state.inventory_loading);
    }

    #[tokio::test]
    async fn it_ignores_scroll_when_there_are_no_more_pages() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.inventory = vec![inv_row(100, false)];
      state.inventory_has_more = false;

      let _ = update(
        &mut state,
        Message::InventoryScrolled {
          absolute: 0.0,
          relative: 0.95,
        },
        &db,
      );

      assert!(!state.inventory_loading, "no load is started when the set is exhausted");
    }

    #[tokio::test]
    async fn it_loads_an_empty_inventory_page_for_a_character_scope() {
      let db = crate::store::open_test().await.unwrap();
      let roster = vec![pilot(7)];
      let view = InventoryView::default();
      let cursor = inv_row(100, false).cursor(view.sort);

      let rows = load_inventory_page(&db, Scope::Character(7), &roster, &[], &view, cursor).await;

      assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn it_preserves_interaction_state_on_a_sync_triggered_reload() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.search = "name:Rifter".to_owned();
      state.abyssal_picker_open = true;
      state.set_inventory_children_for_test(200, vec![inv_row(201, false)]);
      let generation = state.search_generation;

      let _ = update(
        &mut state,
        Message::SyncReloaded {
          generation,
          loaded: Box::new(Loaded {
            inventory: vec![inv_row(200, true), inv_row(300, false)],
            totals: InventoryTotals {
              items: 9,
              locations: 1,
              value: 0.0,
              volume: 0.0,
            },
            ..Loaded::default()
          }),
        },
        &db,
      );

      assert_eq!(state.totals.items, 9, "a sync reload refreshes the underlying totals");
      assert_eq!(state.search, "name:Rifter", "the in-progress filter text survives");
      assert!(state.abyssal_picker_open, "an open picker survives a sync reload");
      assert!(
        state.container_is_open(200),
        "an expanded container still present in fresh data stays open"
      );
      assert_eq!(state.container_children_of(200).map(<[_]>::len), Some(1));
    }

    #[tokio::test]
    async fn it_resets_pagination_and_expansion_on_reload() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_inventory_children_for_test(100, vec![inv_row(101, false)]);

      let _ = update(
        &mut state,
        Message::Loaded(Box::new(Loaded {
          inventory: vec![inv_row(200, false)],
          ..Loaded::default()
        })),
        &db,
      );

      assert_eq!(state.inventory.iter().map(|r| r.item_id).collect::<Vec<_>>(), [200]);
      assert!(
        state.expanded_containers.is_empty(),
        "a reload clears stale container expansions"
      );
      assert!(state.container_children_of(100).is_none());
      assert!(!state.inventory_has_more, "a short first page leaves no more to load");
    }

    #[tokio::test]
    async fn it_starts_a_load_when_scrolling_past_the_threshold_with_more_pages() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.active = Scope::Character(7);
      state.inventory = vec![inv_row(100, false)];
      state.inventory_has_more = true;

      let _ = update(
        &mut state,
        Message::InventoryScrolled {
          absolute: 0.0,
          relative: 0.9,
        },
        &db,
      );

      assert!(
        state.inventory_loading,
        "crossing the threshold with more pages starts a fetch"
      );
    }

    #[tokio::test]
    async fn it_tracks_the_absolute_scroll_offset_for_windowing() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.inventory = vec![inv_row(100, false)];

      let _ = update(
        &mut state,
        Message::InventoryScrolled {
          absolute: 1_234.0,
          relative: 0.5,
        },
        &db,
      );

      assert_eq!(
        state.inventory_scroll_offset(),
        1_234.0,
        "the pixel offset is stored so the virtual list can window the body"
      );
    }

    #[tokio::test]
    async fn it_yields_no_rows_when_the_scope_resolves_to_no_owner() {
      let db = crate::store::open_test().await.unwrap();
      let view = InventoryView::default();
      let cursor = inv_row(100, false).cursor(view.sort);

      let rows = load_inventory_page(&db, Scope::Corporation(7), &[], &[], &view, cursor).await;

      assert!(rows.is_empty());
    }
  }

  mod panes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_both_pane_widths_when_the_store_is_empty() {
      let state = State::new(crate::config::FeatureFlags::default()).with_restored_panes(&UiState::default());

      assert_eq!(state.pane(Pane::Sidebar).width(), SIDEBAR_DEFAULT_WIDTH);
      assert_eq!(state.pane(Pane::AbyssalsFilter).width(), ABYSSALS_FILTER_DEFAULT_WIDTH);
    }

    #[test]
    fn it_does_not_listen_for_drag_events_while_no_pane_is_active() {
      let state = State::new(crate::config::FeatureFlags::default());

      let _sub: iced::Subscription<Message> = subscription(&state);
    }

    #[test]
    fn it_listens_for_drag_events_while_the_abyssals_filter_pane_is_active() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.abyssals_filter.start();

      let _sub: iced::Subscription<Message> = subscription(&state);
    }

    #[test]
    fn it_persists_the_settled_width_under_the_matching_pane_key() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      assert_eq!(state.pane_mut(Pane::Sidebar).1, SIDEBAR_PANE_KEY);
      assert_eq!(state.pane_mut(Pane::AbyssalsFilter).1, ABYSSALS_FILTER_PANE_KEY);
    }

    #[test]
    fn it_reports_the_active_drag_pane_only_while_dragging() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      assert!(state.active_drag().is_none());

      state.sidebar.start();
      assert_eq!(state.active_drag(), Some(Pane::Sidebar));

      state.sidebar.end();
      state.abyssals_filter.start();
      assert_eq!(state.active_drag(), Some(Pane::AbyssalsFilter));
    }

    #[tokio::test]
    async fn it_resizes_the_sidebar_during_a_drag_and_settles_its_width() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(&mut state, Message::PaneDragStart(Pane::Sidebar), &db);
      let _ = update(&mut state, Message::PaneDrag(500.0), &db);
      let _ = update(&mut state, Message::PaneDrag(560.0), &db);
      assert_eq!(state.pane(Pane::Sidebar).width(), SIDEBAR_DEFAULT_WIDTH + 60.0);
      assert!(state.pane(Pane::Sidebar).is_active());

      let _task = update(&mut state, Message::PaneDragEnd, &db);
      assert!(!state.pane(Pane::Sidebar).is_active());
      assert_eq!(state.pane(Pane::Sidebar).width(), SIDEBAR_DEFAULT_WIDTH + 60.0);
    }

    #[test]
    fn it_restores_both_pane_widths_from_the_keyed_store() {
      let mut ui = UiState::default();
      ui.panes.insert(SIDEBAR_PANE_KEY.to_owned(), 360.0);
      ui.panes.insert(ABYSSALS_FILTER_PANE_KEY.to_owned(), 200.0);

      let state = State::new(crate::config::FeatureFlags::default()).with_restored_panes(&ui);

      assert_eq!(state.pane(Pane::Sidebar).width(), 360.0);
      assert_eq!(state.pane(Pane::AbyssalsFilter).width(), 200.0);
    }

    #[tokio::test]
    async fn it_routes_a_drag_solely_to_the_active_abyssals_filter_pane() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(&mut state, Message::PaneDragStart(Pane::AbyssalsFilter), &db);
      let _ = update(&mut state, Message::PaneDrag(400.0), &db);
      let _ = update(&mut state, Message::PaneDrag(370.0), &db);
      assert_eq!(
        state.pane(Pane::AbyssalsFilter).width(),
        ABYSSALS_FILTER_DEFAULT_WIDTH - 30.0
      );
      assert_eq!(state.pane(Pane::Sidebar).width(), SIDEBAR_DEFAULT_WIDTH);

      let _task = update(&mut state, Message::PaneDragEnd, &db);
      assert!(!state.pane(Pane::AbyssalsFilter).is_active());
    }
  }

  mod resolve_scope_owner {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_aggregates_the_all_scope_across_every_owned_character_and_corporation() {
      assert_eq!(
        resolve_scope_owner(Scope::All, &[pilot(7), pilot(9)], &[corp(98)]),
        Some(Owner::Combined {
          character_ids: vec![7, 9],
          corporation_ids: vec![98],
        })
      );
    }

    #[test]
    fn it_resolves_a_character_scope_to_that_character() {
      assert_eq!(
        resolve_scope_owner(Scope::Character(7), &[], &[]),
        Some(Owner::Character(7))
      );
    }

    #[test]
    fn it_resolves_a_known_corporation_scope_to_that_corporation() {
      assert_eq!(
        resolve_scope_owner(Scope::Corporation(98), &[], &[corp(98)]),
        Some(Owner::Corporation(98))
      );
    }

    #[test]
    fn it_resolves_an_unknown_corporation_scope_to_none() {
      assert_eq!(resolve_scope_owner(Scope::Corporation(404), &[], &[corp(98)]), None);
    }

    #[test]
    fn it_resolves_the_all_scope_with_no_owned_characters_to_the_corporations_alone() {
      assert_eq!(
        resolve_scope_owner(Scope::All, &[], &[corp(98)]),
        Some(Owner::Combined {
          character_ids: vec![],
          corporation_ids: vec![98],
        })
      );
    }
  }

  mod saved_filter {
    use pretty_assertions::assert_eq;

    use super::*;

    fn filter(id: i64, name: &str, query: &str, category: Option<&str>) -> SavedAssetFilter {
      SavedAssetFilter {
        category: category.map(str::to_owned),
        id,
        name: name.to_owned(),
        query: query.to_owned(),
      }
    }

    #[tokio::test]
    async fn a_created_filter_is_recorded_selected_and_clears_the_geo_selection() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.geo_selected = GeoSelection::System(30);

      let _ = update(
        &mut state,
        Message::SavedFilterCreated(Some(5), vec![filter(5, "Ships", "category:ship", Some("ship"))]),
        &db,
      );

      assert_eq!(state.saved_filters().len(), 1);
      assert_eq!(state.saved_filter_active(), Some(5));
      assert_eq!(state.geo_selected(), GeoSelection::All);
    }

    #[tokio::test]
    async fn a_reloaded_list_replaces_the_saved_filters() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.saved_filters = vec![filter(1, "Old", "", None)];

      let _ = update(
        &mut state,
        Message::SavedFiltersReloaded(vec![filter(2, "New", "name:Rifter", None)]),
        &db,
      );

      assert_eq!(state.saved_filters().len(), 1);
      assert_eq!(state.saved_filters()[0].id(), 2);
    }

    #[tokio::test]
    async fn can_save_is_gated_on_a_query_or_a_non_all_category() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      assert!(!state.can_save_filter());

      state.search = "tritanium".to_owned();
      assert!(state.can_save_filter());

      state.search = String::new();
      state.category = Category::Module;
      assert!(state.can_save_filter());
    }

    #[tokio::test]
    async fn confirming_closes_the_modal() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.saved_filter_modal_open = true;
      state.saved_filter_draft_name = "Ships".to_owned();
      state.category = Category::Ship;

      let _ = update(&mut state, Message::SaveFilterConfirmed, &db);

      assert!(!state.saved_filter_modal_open());
      assert_eq!(state.saved_filter_draft_name(), "");
    }

    #[tokio::test]
    async fn confirming_with_an_empty_name_keeps_the_modal_open() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.saved_filter_modal_open = true;
      state.saved_filter_draft_name = "   ".to_owned();
      state.search = "tritanium".to_owned();

      let _ = update(&mut state, Message::SaveFilterConfirmed, &db);

      assert!(state.saved_filter_modal_open());
    }

    #[tokio::test]
    async fn deleting_the_active_filter_clears_the_selection() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.saved_filters = vec![filter(5, "Ships", "category:ship", Some("ship"))];
      state.saved_filter_active = Some(5);

      let _ = update(&mut state, Message::SavedFilterDeleted(5), &db);

      assert_eq!(state.saved_filter_active(), None);
    }

    #[tokio::test]
    async fn it_cancels_the_modal() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.saved_filter_modal_open = true;
      state.saved_filter_draft_name = "Jita".to_owned();

      let _ = update(&mut state, Message::SaveFilterCancelled, &db);

      assert!(!state.saved_filter_modal_open());
      assert_eq!(state.saved_filter_draft_name(), "");
    }

    #[tokio::test]
    async fn it_opens_and_clears_the_modal() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.saved_filter_draft_name = "stale".to_owned();

      let _ = update(&mut state, Message::SaveFilterOpened, &db);

      assert!(state.saved_filter_modal_open());
      assert_eq!(state.saved_filter_draft_name(), "");
    }

    #[tokio::test]
    async fn it_records_the_draft_name() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(
        &mut state,
        Message::SaveFilterNameChanged("Jita modules".to_owned()),
        &db,
      );

      assert_eq!(state.saved_filter_draft_name(), "Jita modules");
    }

    #[tokio::test]
    async fn re_selecting_the_active_filter_clears_it() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.saved_filters = vec![filter(5, "Modules", "name:Rifter", Some("module"))];
      state.saved_filter_active = Some(5);
      state.search = "name:Rifter".to_owned();
      state.category = Category::Module;

      let _ = update(&mut state, Message::SavedFilterSelected(5), &db);

      assert_eq!(state.saved_filter_active(), None);
      assert_eq!(state.search(), "");
      assert_eq!(state.category(), Category::All);
    }

    #[tokio::test]
    async fn right_pressing_a_filter_opens_its_context_menu_at_the_cursor() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.saved_filters = vec![filter(5, "Ships", "category:ship", Some("ship"))];
      state.sidebar_cursor = Some(iced::Point::new(12.0, 34.0));

      let _ = update(&mut state, Message::SavedFilterRightPressed(5), &db);

      let menu = state.saved_filter_context_menu().expect("a context menu");
      assert_eq!(menu.id, 5);
      assert_eq!(menu.name, "Ships");
    }

    #[tokio::test]
    async fn selecting_a_filter_restores_its_query_and_category_and_clears_the_location() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.saved_filters = vec![filter(5, "Modules", "name:Rifter", Some("module"))];
      state.geo_selected = GeoSelection::System(30);

      let _ = update(&mut state, Message::SavedFilterSelected(5), &db);

      assert_eq!(state.saved_filter_active(), Some(5));
      assert_eq!(state.search(), "name:Rifter");
      assert_eq!(state.category(), Category::Module);
      assert_eq!(state.geo_selected(), GeoSelection::All);
    }

    #[tokio::test]
    async fn selecting_a_location_clears_the_active_saved_filter() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.saved_filter_active = Some(5);

      let _ = update(&mut state, Message::GeoNodeSelected(GeoSelection::System(30)), &db);

      assert_eq!(state.saved_filter_active(), None);
    }

    #[tokio::test]
    async fn the_capture_preview_prefers_the_query_then_falls_back_to_the_category() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.search = "  name:Rifter  ".to_owned();
      assert_eq!(state.save_filter_capture(), "name:Rifter");

      state.search = String::new();
      state.category = Category::Ship;
      assert_eq!(state.save_filter_capture(), "category:ship");

      state.category = Category::All;
      assert_eq!(state.save_filter_capture(), "");
    }
  }

  mod tab_scope_gate {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_does_not_gate_a_character_with_the_asset_scopes() {
      let granted = crate::features::shell::registry::descriptor(crate::config::Feature::AssetTracking)
        .scopes
        .join(" ");
      let mut granted_pilot = pilot(1);
      granted_pilot.granted_scopes = Some(granted);
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_picker_for_test(Scope::Character(1), vec![granted_pilot], Vec::new());

      assert!(state.tab_scope_gate().is_none());
    }

    #[test]
    fn it_does_not_gate_the_all_or_corporation_scopes() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_picker_for_test(Scope::All, vec![pilot(1)], Vec::new());

      assert!(state.tab_scope_gate().is_none());

      state.set_picker_for_test(Scope::Corporation(99), vec![pilot(1)], Vec::new());

      assert!(state.tab_scope_gate().is_none());
    }

    #[test]
    fn it_gates_a_character_scope_missing_the_asset_scopes() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_picker_for_test(Scope::Character(1), vec![pilot(1)], Vec::new());

      let gate = state.tab_scope_gate().expect("missing scope should gate");

      assert_eq!(gate.0, 1);
      assert!(!gate.2.is_empty());
    }
  }

  mod stale_images {
    use pretty_assertions::assert_eq;

    use super::*;

    fn fresh_pilot(id: i64) -> RosterPilot {
      RosterPilot {
        corp: "TST".to_owned(),
        granted_scopes: None,
        id,
        name: format!("Pilot {id}"),
        portrait: images::ImageState::Fresh(std::path::PathBuf::from(format!("/cache/{id}.jpg"))),
      }
    }

    #[test]
    fn it_collects_stale_portraits_logos_and_abyssal_cards() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_picker_for_test(Scope::All, vec![pilot(7), fresh_pilot(9)], vec![corp(98)]);
      state.set_abyssals_for_test(
        vec![abyssals::AbyssalCard {
          character_id: 11,
          estimate: None,
          group_type_id: 2410,
          item_id: 1,
          location: String::new(),
          module_name: "Module".to_owned(),
          owner_name: "Vex".to_owned(),
          portrait: images::ImageState::Stale {
            id: 11,
            kind: images::ImageKind::CharacterPortrait,
          },
          price_unavailable: false,
          stats: Vec::new(),
          tier_label: "Gravid".to_owned(),
          type_icon: images::IconResolution::Missing,
        }],
        Vec::new(),
        abyssals::Filters::default(),
        false,
      );

      assert_eq!(
        state.stale_images(),
        vec![
          (images::ImageKind::CharacterPortrait, 7),
          (images::ImageKind::CorporationLogo, 98),
          (images::ImageKind::CharacterPortrait, 11),
        ]
      );
    }

    #[test]
    fn it_is_empty_when_every_model_is_fresh() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_picker_for_test(Scope::All, vec![fresh_pilot(7)], Vec::new());

      assert_eq!(state.stale_images(), Vec::new());
    }
  }

  mod stockpile {
    use pretty_assertions::assert_eq;

    use super::*;

    fn card(id: i64, name: &str) -> stockpiles::StockpileCard {
      stockpiles::StockpileCard {
        character_scope: None,
        fill_isk: 0.0,
        id,
        items: vec![],
        location_id: None,
        location_name: None,
        name: name.to_owned(),
        overall_pct: 0.0,
        scope_pilots: 0,
        target_isk: 0.0,
      }
    }

    #[tokio::test]
    async fn deleting_spawns_a_reload_and_reloaded_cards_replace_state() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.stockpiles = vec![card(1, "Old")];

      let _task = update(&mut state, Message::StockpileDeleted(1), &db);

      let _ = update(&mut state, Message::StockpilesReloaded(vec![card(2, "New")]), &db);
      assert_eq!(state.stockpiles, vec![card(2, "New")]);
    }

    #[tokio::test]
    async fn it_exposes_a_card_by_id_for_seeding_an_edit_window() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.stockpiles = vec![card(7, "Ammo")];

      assert_eq!(state.stockpile_card(7).map(|card| card.name.as_str()), Some("Ammo"));
      assert!(state.stockpile_card(404).is_none());
    }

    #[tokio::test]
    async fn it_ignores_a_multibuy_copy_for_an_unknown_card() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(&mut state, Message::StockpileMultibuyExportCopied(404), &db);

      assert!(!state.stockpile_multibuy_copied);
    }

    #[tokio::test]
    async fn it_ignores_a_right_press_without_a_cursor_anchor() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.stockpiles = vec![card(7, "Ammo")];

      let _ = update(&mut state, Message::StockpileCardRightPressed(7), &db);

      assert!(state.stockpile_context_menu.is_none());
    }

    #[tokio::test]
    async fn it_opens_a_context_menu_at_the_cursor_then_closes_it() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.stockpiles = vec![card(7, "Ammo")];

      let _ = update(
        &mut state,
        Message::StockpileCursorMoved(iced::Point::new(20.0, 40.0)),
        &db,
      );
      let _ = update(&mut state, Message::StockpileCardRightPressed(7), &db);
      assert_eq!(state.stockpile_context_menu.as_ref().map(|menu| menu.id), Some(7));

      let _ = update(&mut state, Message::StockpileContextMenuClosed, &db);
      assert!(state.stockpile_context_menu.is_none());
    }

    #[tokio::test]
    async fn it_opens_changes_and_closes_the_multibuy_export() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.stockpiles = vec![card(7, "Ammo")];

      let _ = update(&mut state, Message::StockpileMultibuyExportOpened(7), &db);
      assert_eq!(state.stockpile_multibuy_export, Some(7));
      assert!(!state.stockpile_multibuy_copied);

      let _ = update(
        &mut state,
        Message::StockpileMultibuyModeChanged(stockpiles::MultibuyMode::Remaining),
        &db,
      );
      assert_eq!(state.stockpile_multibuy_mode, stockpiles::MultibuyMode::Remaining);

      let _ = update(&mut state, Message::StockpileMultibuyExportCopied(7), &db);
      assert!(state.stockpile_multibuy_copied);

      let _ = update(&mut state, Message::StockpileMultibuyExportClosed, &db);
      assert!(state.stockpile_multibuy_export.is_none());
    }

    #[tokio::test]
    async fn it_toggles_a_card_expanded_then_collapsed() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(&mut state, Message::StockpileItemsToggled(7), &db);
      assert!(state.stockpile_expanded.contains(&7));

      let _ = update(&mut state, Message::StockpileItemsToggled(7), &db);
      assert!(!state.stockpile_expanded.contains(&7));
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    fn loaded_with_items(items: i64) -> Box<Loaded> {
      Box::new(Loaded {
        corporations: vec![],
        geo_tree: GeoTree::default(),
        inventory: vec![],
        roster: vec![],
        saved_filters: vec![],
        totals: InventoryTotals {
          items,
          locations: 1,
          value: 0.0,
          volume: 0.0,
        },
        values: values::ValueSummary::default(),
        nav: tracker::NavSeries::default(),
        stockpiles: vec![],
        abyssals: abyssals::AbyssalsData::default(),
        ..Loaded::default()
      })
    }

    fn abyssal_cards(count: usize) -> Vec<abyssals::AbyssalCard> {
      (0..count)
        .map(|i| abyssals::AbyssalCard {
          character_id: 1,
          estimate: None,
          group_type_id: 2410,
          item_id: i as i64,
          location: String::new(),
          module_name: "Module".to_owned(),
          owner_name: "Vex".to_owned(),
          portrait: images::ImageState::Stale {
            id: 1,
            kind: images::ImageKind::CharacterPortrait,
          },
          price_unavailable: false,
          stats: Vec::new(),
          tier_label: "Gravid".to_owned(),
          type_icon: images::IconResolution::Missing,
        })
        .collect()
    }

    #[tokio::test]
    async fn a_clamped_abyssal_stat_range_is_recorded_against_its_attribute() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_abyssal_stat_templates_for_test(vec![StatTemplate {
        attribute_id: 50,
        base_value: 40.0,
        bound_hi: 56.0,
        bound_lo: 28.0,
        display_name: "CPU Output".to_owned(),
        high_is_good: true,
        unit_id: Some(115),
      }]);
      state.abyssal_filters.source_type_id = Some(2410);

      let _ = update(
        &mut state,
        Message::AbyssalStatRangeChanged(50, SliderEndpoint::Min, 35.0),
        &db,
      );
      assert_eq!(state.abyssal_filters.stat_ranges.get(&50), Some(&(35.0, 56.0)));
    }

    #[tokio::test]
    async fn a_loaded_page_is_appended_and_clears_has_more_for_a_short_page() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_abyssals_for_test(abyssal_cards(60), Vec::new(), abyssals::Filters::default(), false);
      state.set_abyssal_pagination_for_test(true, true);

      // A short page (fewer than PAGE_SIZE) means the set is exhausted.
      let epoch = state.abyssal_page_epoch.current();
      let _ = update(
        &mut state,
        Message::AbyssalPageLoaded {
          cards: abyssal_cards(10),
          epoch,
        },
        &db,
      );

      assert_eq!(state.abyssals().len(), 70, "the page is appended to the loaded set");
      assert!(!state.abyssal_has_more(), "a short page leaves no more to load");
      assert!(!state.abyssal_loading());
    }

    #[tokio::test]
    async fn a_scroll_append_does_not_clobber_the_abyssal_total() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_abyssals_for_test(abyssal_cards(60), Vec::new(), abyssals::Filters::default(), false);
      state.set_abyssal_pagination_for_test(true, true);
      let _ = update(
        &mut state,
        Message::AbyssalCardsReloaded(abyssals::FilteredCards {
          cards: abyssal_cards(abyssals::PAGE_SIZE as usize),
          total: 200,
        }),
        &db,
      );
      state.set_abyssal_pagination_for_test(true, true);

      let epoch = state.abyssal_page_epoch.current();
      let _ = update(
        &mut state,
        Message::AbyssalPageLoaded {
          cards: abyssal_cards(10),
          epoch,
        },
        &db,
      );

      assert_eq!(
        state.abyssal_total(),
        200,
        "appending a scroll page leaves the DB total untouched"
      );
    }

    #[tokio::test]
    async fn a_stale_abyssal_page_is_dropped_after_the_set_is_reloaded() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_abyssals_for_test(abyssal_cards(60), Vec::new(), abyssals::Filters::default(), false);
      state.set_abyssal_pagination_for_test(true, true);

      // The user scrolled (capturing the epoch) then changed the filter, which reloads the set and
      // bumps the epoch.
      let stale_epoch = state.abyssal_page_epoch.current();
      let _ = update(
        &mut state,
        Message::AbyssalCardsReloaded(abyssals::FilteredCards {
          cards: abyssal_cards(10),
          total: 10,
        }),
        &db,
      );

      let _ = update(
        &mut state,
        Message::AbyssalPageLoaded {
          cards: abyssal_cards(5),
          epoch: stale_epoch,
        },
        &db,
      );

      assert_eq!(
        state.abyssals().len(),
        10,
        "a page from the superseded filter must not append foreign cards"
      );
    }

    #[tokio::test]
    async fn an_abyssal_stat_range_change_without_loaded_templates_is_a_noop() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.abyssal_filters.source_type_id = Some(2410);

      let _ = update(
        &mut state,
        Message::AbyssalStatRangeChanged(50, SliderEndpoint::Min, 5.0),
        &db,
      );

      assert!(state.abyssal_filters.stat_ranges.is_empty());
    }

    #[tokio::test]
    async fn committing_a_slider_value_edit_applies_the_typed_bound() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_abyssal_stat_templates_for_test(vec![StatTemplate {
        attribute_id: 50,
        base_value: 40.0,
        bound_hi: 56.0,
        bound_lo: 28.0,
        display_name: "CPU Output".to_owned(),
        high_is_good: true,
        unit_id: Some(115),
      }]);
      state.abyssal_filters.source_type_id = Some(2410);

      let _ = update(
        &mut state,
        Message::AbyssalSliderEditStarted(50, SliderEndpoint::Min, 28.0),
        &db,
      );
      assert_eq!(state.abyssal_slider_edit, Some((50, SliderEndpoint::Min)));

      let _ = update(&mut state, Message::AbyssalSliderEditInput("33".to_owned()), &db);
      let _ = update(
        &mut state,
        Message::AbyssalSliderEditCommitted(50, SliderEndpoint::Min),
        &db,
      );

      assert_eq!(state.abyssal_slider_edit, None);
      assert_eq!(state.abyssal_filters.stat_ranges.get(&50), Some(&(33.0, 56.0)));
    }

    #[tokio::test]
    async fn it_applies_search_results_for_the_current_generation() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.search_generation = 7;
      state.totals.items = 99;

      let _ = update(
        &mut state,
        Message::SearchReloaded {
          generation: 7,
          loaded: loaded_with_items(5),
        },
        &db,
      );

      assert_eq!(state.totals.items, 5);
    }

    #[tokio::test]
    async fn it_bumps_the_search_generation_on_each_keystroke() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(&mut state, Message::SearchChanged("r".to_owned()), &db);
      let _ = update(&mut state, Message::SearchChanged("ri".to_owned()), &db);

      assert_eq!(state.search_generation, 2);
    }

    #[tokio::test]
    async fn it_covers_the_remaining_inventory_message_branches() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(&mut state, Message::AssetChartHovered(Some(0.5)), &db);
      assert_eq!(state.chart_hover, Some(0.5));

      let _ = update(&mut state, Message::InventoryHelpToggled, &db);
      assert!(state.inventory_help_open);

      let _ = update(&mut state, Message::CategorySelected(Category::Ship), &db);
      let _ = update(&mut state, Message::CategorySelected(Category::Ship), &db);
      assert_eq!(state.category, Category::Ship);

      let _ = update(&mut state, Message::SortSelected(SortColumn::Name), &db);
      let _ = update(&mut state, Message::SortSelected(SortColumn::Name), &db);
      assert_eq!(state.sort_dir, SortDirection::Ascending);

      let active = state.active;
      let _ = update(&mut state, Message::ScopeSelected(active), &db);
      assert_eq!(state.active, active);

      let _ = update(&mut state, Message::SearchSubmitted, &db);
      let _ = update(&mut state, Message::FilterExamplePicked("ship"), &db);
      assert!(!state.inventory_help_open);
    }

    #[tokio::test]
    async fn it_drops_search_results_from_a_superseded_generation() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.search_generation = 8;
      state.totals.items = 99;

      let _ = update(
        &mut state,
        Message::SearchReloaded {
          generation: 7,
          loaded: loaded_with_items(5),
        },
        &db,
      );

      assert_eq!(state.totals.items, 99, "a stale keystroke's result is ignored");
    }

    #[tokio::test]
    async fn it_invalidates_in_flight_searches_when_a_discrete_reload_runs() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(&mut state, Message::SearchChanged("rifter".to_owned()), &db);
      let stale_generation = state.search_generation;
      let _ = update(&mut state, Message::CategorySelected(Category::Ship), &db);

      assert_ne!(
        state.search_generation, stale_generation,
        "changing category supersedes the pending search"
      );
    }

    #[tokio::test]
    async fn it_records_the_loaded_roster_and_totals() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(
        &mut state,
        Message::Loaded(Box::new(Loaded {
          corporations: vec![corp(98)],
          geo_tree: GeoTree::default(),
          inventory: vec![],
          roster: vec![pilot(7)],
          saved_filters: vec![],
          totals: InventoryTotals {
            items: 5,
            locations: 2,
            value: 100.0,
            volume: 50.0,
          },
          values: values::ValueSummary::default(),
          nav: tracker::NavSeries::default(),
          stockpiles: vec![],
          abyssals: abyssals::AbyssalsData::default(),
          ..Loaded::default()
        })),
        &db,
      );

      assert_eq!(state.roster, vec![pilot(7)]);
      assert_eq!(state.corporations, vec![corp(98)]);
      assert_eq!(state.totals.items, 5);
    }

    #[tokio::test]
    async fn it_records_the_search_string() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(&mut state, Message::SearchChanged("tritanium".to_owned()), &db);
      assert_eq!(state.search, "tritanium");
    }

    #[tokio::test]
    async fn it_records_the_selected_inventory_category() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(&mut state, Message::CategorySelected(Category::Ship), &db);
      assert_eq!(state.category, Category::Ship);
    }

    #[tokio::test]
    async fn it_records_the_selected_scope_and_closes_the_picker() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.picker_open = true;

      let _ = update(&mut state, Message::ScopeSelected(Scope::Character(42)), &db);

      assert_eq!(state.active, Scope::Character(42));
      assert!(!state.picker_open);
    }

    #[tokio::test]
    async fn it_switches_the_active_tab() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(&mut state, Message::TabSelected(Tab::Values), &db);
      assert_eq!(state.tab, Tab::Values);
    }

    #[tokio::test]
    async fn it_toggles_the_abyssal_picker() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(&mut state, Message::AbyssalPickerToggled, &db);
      assert!(state.abyssal_picker_open);
      let _ = update(&mut state, Message::AbyssalPickerToggled, &db);
      assert!(!state.abyssal_picker_open);
    }

    #[tokio::test]
    async fn it_toggles_the_picker_open_and_closed() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(state.picker_open);

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(!state.picker_open);
    }

    #[tokio::test]
    async fn reloading_cards_replaces_the_set_and_resets_pagination() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_abyssals_for_test(abyssal_cards(60), Vec::new(), abyssals::Filters::default(), false);
      state.set_abyssal_pagination_for_test(true, true);
      let _ = update(
        &mut state,
        Message::AbyssalGridScrolled {
          absolute: 999.0,
          relative: 0.9,
        },
        &db,
      );

      // A reload that returns a full page keeps has_more true and clears loading/offset.
      let _ = update(
        &mut state,
        Message::AbyssalCardsReloaded(abyssals::FilteredCards {
          cards: abyssal_cards(abyssals::PAGE_SIZE as usize),
          total: 137,
        }),
        &db,
      );

      assert_eq!(state.abyssals().len(), abyssals::PAGE_SIZE as usize);
      assert_eq!(state.abyssal_total(), 137, "the reload feeds the filter-aware DB total");
      assert!(state.abyssal_has_more(), "a full reload page implies more to load");
      assert!(!state.abyssal_loading());
      assert_eq!(
        state.abyssal_scroll_offset(),
        0.0,
        "a reload returns the grid to the top"
      );
    }

    #[tokio::test]
    async fn resetting_the_abyssal_filters_clears_type_and_ranges() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.abyssal_filters.source_type_id = Some(2410);
      state.abyssal_filters.stat_ranges.insert(50, (1.0, 2.0));

      let _ = update(&mut state, Message::AbyssalFilterReset, &db);

      assert_eq!(state.abyssal_filters, abyssals::Filters::default());
    }

    #[tokio::test]
    async fn scrolling_below_the_threshold_only_tracks_the_offset() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_abyssals_for_test(abyssal_cards(60), Vec::new(), abyssals::Filters::default(), false);
      state.set_abyssal_pagination_for_test(true, false);

      let _ = update(
        &mut state,
        Message::AbyssalGridScrolled {
          absolute: 120.0,
          relative: 0.5,
        },
        &db,
      );

      assert!(!state.abyssal_loading(), "a shallow scroll does not page");
      assert_eq!(state.abyssal_scroll_offset(), 120.0);
    }

    #[tokio::test]
    async fn scrolling_does_not_load_when_already_loading_or_exhausted() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_abyssals_for_test(abyssal_cards(60), Vec::new(), abyssals::Filters::default(), false);

      // No more pages: a threshold scroll must not start a load.
      state.set_abyssal_pagination_for_test(false, false);
      let _ = update(
        &mut state,
        Message::AbyssalGridScrolled {
          absolute: 0.0,
          relative: 0.95,
        },
        &db,
      );
      assert!(!state.abyssal_loading(), "no load is started once the set is exhausted");

      // Already loading: a second threshold scroll must not start a duplicate load.
      state.set_abyssal_pagination_for_test(true, true);
      let _ = update(
        &mut state,
        Message::AbyssalGridScrolled {
          absolute: 0.0,
          relative: 0.95,
        },
        &db,
      );
      assert!(state.abyssal_loading());
    }

    #[tokio::test]
    async fn scrolling_past_the_threshold_starts_loading_the_next_page() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_abyssals_for_test(abyssal_cards(60), Vec::new(), abyssals::Filters::default(), false);
      state.set_abyssal_pagination_for_test(true, false);

      let _ = update(
        &mut state,
        Message::AbyssalGridScrolled {
          absolute: 2_000.0,
          relative: 0.9,
        },
        &db,
      );

      assert!(state.abyssal_loading(), "crossing the threshold begins a page load");
      assert_eq!(
        state.abyssal_scroll_offset(),
        2_000.0,
        "the pixel offset is tracked for windowing"
      );
    }

    #[tokio::test]
    async fn selecting_an_abyssal_source_type_records_it_and_closes_the_picker() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.abyssal_picker_open = true;
      state.abyssal_filters.stat_ranges.insert(50, (1.0, 2.0));

      let _ = update(&mut state, Message::AbyssalSourceTypeSelected(Some(2410)), &db);

      assert_eq!(state.abyssal_filters.source_type_id, Some(2410));
      assert!(state.abyssal_filters.stat_ranges.is_empty());
      assert!(!state.abyssal_picker_open);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_loaded_state() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.roster = vec![pilot(1)];
      state.active = Scope::Character(1);
      state.totals = InventoryTotals {
        items: 5,
        locations: 2,
        value: 1_000.0,
        volume: 200.0,
      };

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_the_abyssal_picker_modal_overlay() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.roster = vec![pilot(1)];
      state.tab = Tab::Abyssals;
      state.abyssal_picker_open = true;

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_the_abyssals_tab_with_its_resizable_filter_rail() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.roster = vec![pilot(1)];
      state.tab = Tab::Abyssals;

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_the_empty_state_before_any_load() {
      let state = State::new(crate::config::FeatureFlags::default());

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_the_inventory_help_overlay() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.roster = vec![pilot(1)];
      state.tab = Tab::Inventory;
      state.inventory_help_open = true;

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_the_scope_picker_overlay() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.roster = vec![pilot(1)];
      state.picker_open = true;

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_the_stockpile_context_menu_overlay() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.roster = vec![pilot(1)];
      state.tab = Tab::Stockpiles;
      state.stockpile_context_menu = Some(StockpileContextMenu {
        anchor: iced::Point::new(10.0, 10.0),
        id: 1,
        name: "Ammo".to_owned(),
      });

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }
  }

  mod asset_tags {
    use pretty_assertions::assert_eq;

    use super::*;

    fn flags() -> crate::config::FeatureFlags {
      crate::config::FeatureFlags::default()
    }

    fn test_row(item_id: i64, type_name: &str) -> InventoryRow {
      InventoryRow {
        category: "ship".to_owned(),
        container_id: None,
        depth: 0,
        group_name: "Frigate".to_owned(),
        is_active_ship: false,
        is_blueprint_copy: None,
        is_container: false,
        item_id,
        location_id: 60_003_760,
        location_label: Some("Jita IV - Moon 4".to_owned()),
        name: None,
        owner_id: 7,
        quantity: 1,
        reproc_value: 0.0,
        row_volume: 10.0,
        type_icon: images::IconResolution::Missing,
        type_id: 587,
        type_name: type_name.to_owned(),
        unit_price: 100.0,
        value: 100.0,
      }
    }

    async fn seeded_tag(db: &Database, name: &str) -> Tag {
      infra::create_scoped(db, name, None, None, TAG_SCOPE_ASSET)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn opening_the_modal_keys_it_on_the_item_id_and_resolves_the_row_name() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(flags());
      state.inventory = vec![InventoryRow {
        name: Some("Loot Run".to_owned()),
        ..test_row(5001, "Giant Secure Container")
      }];

      let _ = update(
        &mut state,
        Message::OpenAssetTagModal {
          item_id: 5001,
        },
        &db,
      );

      let modal = state.asset_tag_modal().expect("the modal is open");
      assert_eq!(modal.entity_id, 5001);
      assert_eq!(modal.entity_type, ENTITY_TYPE_ASSET);
      assert_eq!(state.asset_tag_modal_entity_name(), "Loot Run");
    }

    #[tokio::test]
    async fn the_modal_input_round_trips_and_close_dismisses_it() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(flags());
      let _ = update(
        &mut state,
        Message::OpenAssetTagModal {
          item_id: 5001,
        },
        &db,
      );

      let _ = update(
        &mut state,
        Message::AssetTagModal(AddTagMessage::InputChanged("Keep".to_owned())),
        &db,
      );
      assert_eq!(state.asset_tag_modal().map(|m| m.input.as_str()), Some("Keep"));

      let _ = update(&mut state, Message::AssetTagModal(AddTagMessage::Close), &db);
      assert!(state.asset_tag_modal().is_none());
    }

    #[tokio::test]
    async fn assign_then_unassign_round_trips_the_membership_for_an_item_id() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(flags());
      let keep = seeded_tag(&db, "Keep").await;

      // Assign: the same write path the modal's Assign arm performs, then reload into state.
      infra::assign(&db, ENTITY_TYPE_ASSET, 5001, keep.id()).await.unwrap();
      let _ = update(&mut state, reload_asset_tags(db.clone()).await, &db);

      assert_eq!(
        state.asset_tags_for(5001).iter().map(|t| t.id()).collect::<Vec<_>>(),
        vec![keep.id()]
      );

      // Unassign: the modal's Unassign arm write, reloaded — the chip disappears.
      infra::unassign(&db, ENTITY_TYPE_ASSET, 5001, keep.id()).await.unwrap();
      let _ = update(&mut state, reload_asset_tags(db.clone()).await, &db);

      assert!(state.asset_tags_for(5001).is_empty());
    }

    #[tokio::test]
    async fn the_modal_partition_splits_assigned_from_assignable_for_the_target_item() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(flags());
      let keep = seeded_tag(&db, "Keep").await;
      let _sell = seeded_tag(&db, "Sell").await;
      infra::assign(&db, ENTITY_TYPE_ASSET, 5001, keep.id()).await.unwrap();

      let _ = update(
        &mut state,
        Message::OpenAssetTagModal {
          item_id: 5001,
        },
        &db,
      );
      let _ = update(&mut state, reload_asset_tags(db.clone()).await, &db);

      let (assigned, assignable) = state.asset_tag_modal_partition();
      assert_eq!(assigned.iter().map(|t| t.name().as_str()).collect::<Vec<_>>(), ["Keep"]);
      assert_eq!(
        assignable.iter().map(|t| t.name().as_str()).collect::<Vec<_>>(),
        ["Sell"]
      );
    }

    #[tokio::test]
    async fn the_loaded_inventory_carries_the_asset_tag_registry_and_membership_map() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(flags());
      let keep = seeded_tag(&db, "Keep").await;
      infra::assign(&db, ENTITY_TYPE_ASSET, 5001, keep.id()).await.unwrap();

      let loaded = load_assets(db.clone(), Scope::All, InventoryView::default()).await;
      apply_loaded(&mut state, loaded);

      assert_eq!(
        state.asset_tags.iter().map(|t| t.name().as_str()).collect::<Vec<_>>(),
        ["Keep"]
      );
      assert_eq!(state.tag_memberships.get(&5001), Some(&vec![keep.id()]));
    }

    // Drives the modal through `update` so `apply_asset_tag_modal` executes each branch directly. The
    // returned `Task`s are not run, so the assertions target the synchronous state transitions and the
    // task shape (units) each arm produces.
    fn open_modal(state: &mut State, db: &Database, item_id: i64) {
      let _ = update(
        state,
        Message::OpenAssetTagModal {
          item_id,
        },
        db,
      );
    }

    #[tokio::test]
    async fn input_changed_with_no_modal_open_is_a_no_op() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(flags());

      // No modal is open, so the `Some(modal)` guard is skipped and nothing is stored.
      let task = update(
        &mut state,
        Message::AssetTagModal(AddTagMessage::InputChanged("orphaned".to_owned())),
        &db,
      );

      assert!(state.asset_tag_modal().is_none());
      assert_eq!(task.units(), 0, "the no-modal input change produces no task");
    }

    #[tokio::test]
    async fn assign_through_the_modal_fans_a_write_for_the_target() {
      let db = crate::store::open_test().await.unwrap();
      let keep = seeded_tag(&db, "Keep").await;
      let mut state = State::new(flags());
      open_modal(&mut state, &db, 5001);

      // The Assign arm fans `fan_tag_write` over the modal's single target, producing a write task.
      let task = update(
        &mut state,
        Message::AssetTagModal(AddTagMessage::Assign {
          entity_id: 5001,
          entity_type: ENTITY_TYPE_ASSET,
          tag_id: keep.id(),
        }),
        &db,
      );

      assert!(task.units() > 0, "the Assign arm must produce a write task");
    }

    #[tokio::test]
    async fn unassign_through_the_modal_fans_a_write_for_the_target() {
      let db = crate::store::open_test().await.unwrap();
      let keep = seeded_tag(&db, "Keep").await;
      let mut state = State::new(flags());
      open_modal(&mut state, &db, 5001);

      let task = update(
        &mut state,
        Message::AssetTagModal(AddTagMessage::Unassign {
          entity_id: 5001,
          entity_type: ENTITY_TYPE_ASSET,
          tag_id: keep.id(),
        }),
        &db,
      );

      assert!(task.units() > 0, "the Unassign arm must produce a write task");
    }

    #[tokio::test]
    async fn create_and_assign_with_a_blank_name_short_circuits() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(flags());
      open_modal(&mut state, &db, 5001);
      state.selection_tag_ids = vec![5001];
      // Whitespace-only input trims to empty, so the `filter(!is_empty)` arm bails before any write.
      if let Some(modal) = &mut state.add_tag_modal {
        modal.input = "   ".to_owned();
      }

      let task = update(
        &mut state,
        Message::AssetTagModal(AddTagMessage::CreateAndAssign {
          entity_id: 5001,
          entity_type: ENTITY_TYPE_ASSET,
        }),
        &db,
      );

      assert_eq!(task.units(), 0, "a blank name creates no tag and no task");
    }

    #[tokio::test]
    async fn create_and_assign_with_an_empty_selection_short_circuits() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(flags());
      open_modal(&mut state, &db, 5001);
      // A real name, but no stacks selected: the empty-selection guard returns before any write.
      if let Some(modal) = &mut state.add_tag_modal {
        modal.input = "Keep".to_owned();
      }
      state.selection_tag_ids = Vec::new();

      let task = update(
        &mut state,
        Message::AssetTagModal(AddTagMessage::CreateAndAssign {
          entity_id: 5001,
          entity_type: ENTITY_TYPE_ASSET,
        }),
        &db,
      );

      assert_eq!(task.units(), 0, "an empty selection creates no tag and no task");
      assert_eq!(
        state.asset_tag_modal().map(|m| m.input.as_str()),
        Some("Keep"),
        "the input is left untouched when the selection guard bails"
      );
    }

    #[tokio::test]
    async fn create_and_assign_a_new_name_clears_the_input_and_fans_a_write() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(flags());
      open_modal(&mut state, &db, 5001);
      state.selection_tag_ids = vec![5001, 5002];
      // A fresh name with no existing match takes the `None => create_scoped` branch and clears input.
      if let Some(modal) = &mut state.add_tag_modal {
        modal.input = "Brand New".to_owned();
      }

      let task = update(
        &mut state,
        Message::AssetTagModal(AddTagMessage::CreateAndAssign {
          entity_id: 5001,
          entity_type: ENTITY_TYPE_ASSET,
        }),
        &db,
      );

      assert!(task.units() > 0, "creating a new tag fans a write task");
      assert_eq!(
        state.asset_tag_modal().map(|m| m.input.as_str()),
        Some(""),
        "the input is cleared once the create is dispatched"
      );

      // Run the dispatched create+assign (mirrors the task body), then reload to observe the effect.
      let created = infra::create_scoped(&db, "Brand New", None, None, TAG_SCOPE_ASSET)
        .await
        .unwrap();
      for id in [5001, 5002] {
        infra::assign(&db, ENTITY_TYPE_ASSET, id, created.id()).await.unwrap();
      }
      let _ = update(&mut state, reload_asset_tags(db.clone()).await, &db);
      for id in [5001, 5002] {
        assert_eq!(
          state
            .asset_tags_for(id)
            .iter()
            .map(|t| t.name().as_str())
            .collect::<Vec<_>>(),
          ["Brand New"],
          "stack {id} carries the newly created tag"
        );
      }
    }

    #[tokio::test]
    async fn create_and_assign_reuses_an_existing_tag_case_insensitively() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(flags());
      let keep = seeded_tag(&db, "Keep").await;
      // Load the existing "Keep" tag into the registry so the reuse lookup can find it.
      let _ = update(&mut state, reload_asset_tags(db.clone()).await, &db);
      open_modal(&mut state, &db, 5001);
      state.selection_tag_ids = vec![5001];
      // Different casing must still match the existing tag via `normalize_tag_name`, taking the
      // `Some(id)` reuse branch rather than creating a duplicate.
      if let Some(modal) = &mut state.add_tag_modal {
        modal.input = "kEeP".to_owned();
      }

      let task = update(
        &mut state,
        Message::AssetTagModal(AddTagMessage::CreateAndAssign {
          entity_id: 5001,
          entity_type: ENTITY_TYPE_ASSET,
        }),
        &db,
      );

      assert!(task.units() > 0, "reusing an existing tag still fans a write task");
      assert_eq!(
        state.asset_tag_modal().map(|m| m.input.as_str()),
        Some(""),
        "the input is cleared on the reuse path too"
      );

      // Apply the reuse (assign the existing tag), reload, and confirm no duplicate tag was created.
      infra::assign(&db, ENTITY_TYPE_ASSET, 5001, keep.id()).await.unwrap();
      let _ = update(&mut state, reload_asset_tags(db.clone()).await, &db);
      assert_eq!(
        state
          .asset_tags
          .iter()
          .filter(|t| t.name().eq_ignore_ascii_case("keep"))
          .count(),
        1,
        "the case-insensitive match reuses the single existing tag"
      );
      assert_eq!(
        state.asset_tags_for(5001).iter().map(|t| t.id()).collect::<Vec<_>>(),
        vec![keep.id()]
      );
    }

    #[tokio::test]
    async fn create_and_assign_in_memory_guard_agrees_with_the_repo_on_unicode_case() {
      // The in-memory fast path must stay in lock-step with the store. The store keys tags on SQLite's
      // ASCII-only `lower()`, so "Étagère" and "étagère" are DISTINCT rows there. The guard (built on
      // `normalize_tag_name`, which ASCII-folds to match) agrees: it does NOT treat the existing
      // "Étagère" as a reuse target for "étagère", and the repo find-or-create likewise inserts a
      // second row rather than reusing — no false dedup, no divergence between guard and store.
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(flags());
      let existing = seeded_tag(&db, "Étagère").await;
      let _ = update(&mut state, reload_asset_tags(db.clone()).await, &db);

      let resolved = infra::create_scoped(&db, "étagère", None, None, TAG_SCOPE_ASSET)
        .await
        .unwrap();
      assert_ne!(
        resolved.id(),
        existing.id(),
        "the store keys on ASCII lower(), so the Unicode-case variant is a distinct row"
      );

      // The in-memory guard reaches the same verdict: no ASCII-equal match against the existing row.
      let _ = update(&mut state, reload_asset_tags(db.clone()).await, &db);
      let needle = normalize_tag_name("étagère");
      assert!(
        !state
          .asset_tags
          .iter()
          .any(|t| normalize_tag_name(t.name()) == needle && t.id() == existing.id()),
        "the guard does not falsely match the Unicode-case variant against the existing row"
      );
    }

    // The three named futures the modal arms dispatch. Awaiting them directly exercises the
    // write-then-reload bodies (the work the returned `Task`s perform when iced polls them).

    #[tokio::test]
    async fn assign_then_reload_writes_the_membership_and_returns_a_reload() {
      let db = crate::store::open_test().await.unwrap();
      let keep = seeded_tag(&db, "Keep").await;
      let mut state = State::new(flags());

      let message = assign_then_reload(db.clone(), ENTITY_TYPE_ASSET, 5001, keep.id()).await;
      let _ = update(&mut state, message, &db);

      assert_eq!(
        state.asset_tags_for(5001).iter().map(|t| t.id()).collect::<Vec<_>>(),
        vec![keep.id()],
        "the assign future persists the membership before reloading"
      );
    }

    #[tokio::test]
    async fn unassign_then_reload_removes_the_membership_and_returns_a_reload() {
      let db = crate::store::open_test().await.unwrap();
      let keep = seeded_tag(&db, "Keep").await;
      infra::assign(&db, ENTITY_TYPE_ASSET, 5001, keep.id()).await.unwrap();
      let mut state = State::new(flags());

      let message = unassign_then_reload(db.clone(), ENTITY_TYPE_ASSET, 5001, keep.id()).await;
      let _ = update(&mut state, message, &db);

      assert!(
        state.asset_tags_for(5001).is_empty(),
        "the unassign future removes the membership before reloading"
      );
    }

    #[tokio::test]
    async fn create_or_reuse_creates_a_new_tag_and_assigns_every_stack() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(flags());

      // existing is None, so the future takes the `create_scoped` branch, then bulk-assigns.
      let message = create_or_reuse_then_assign(
        db.clone(),
        "Brand New".to_owned(),
        None,
        ENTITY_TYPE_ASSET,
        vec![5001, 5002],
      )
      .await;
      let _ = update(&mut state, message, &db);

      assert_eq!(
        state.asset_tags.iter().filter(|t| t.name() == "Brand New").count(),
        1,
        "exactly one tag is created"
      );
      for id in [5001, 5002] {
        assert_eq!(
          state
            .asset_tags_for(id)
            .iter()
            .map(|t| t.name().as_str())
            .collect::<Vec<_>>(),
          ["Brand New"],
          "stack {id} carries the created tag"
        );
      }
    }

    #[tokio::test]
    async fn create_or_reuse_reuses_an_existing_tag_id_without_creating_a_duplicate() {
      let db = crate::store::open_test().await.unwrap();
      let keep = seeded_tag(&db, "Keep").await;
      let mut state = State::new(flags());

      // existing is Some(id), so the future skips creation and assigns the supplied tag id.
      let message = create_or_reuse_then_assign(
        db.clone(),
        "Keep".to_owned(),
        Some(keep.id()),
        ENTITY_TYPE_ASSET,
        vec![5001],
      )
      .await;
      let _ = update(&mut state, message, &db);

      assert_eq!(
        state.asset_tags.iter().filter(|t| t.name() == "Keep").count(),
        1,
        "no duplicate tag is created on the reuse path"
      );
      assert_eq!(
        state.asset_tags_for(5001).iter().map(|t| t.id()).collect::<Vec<_>>(),
        vec![keep.id()]
      );
    }

    #[tokio::test]
    async fn create_or_reuse_with_no_targets_still_reloads_cleanly() {
      let db = crate::store::open_test().await.unwrap();
      let keep = seeded_tag(&db, "Keep").await;
      let mut state = State::new(flags());

      // A resolvable tag but no item ids: the assign loop is skipped and the future just reloads.
      let message = create_or_reuse_then_assign(
        db.clone(),
        "Keep".to_owned(),
        Some(keep.id()),
        ENTITY_TYPE_ASSET,
        Vec::new(),
      )
      .await;
      let _ = update(&mut state, message, &db);

      assert!(
        state.tag_memberships.is_empty() || state.tag_memberships.values().all(|ids| ids.is_empty()),
        "no memberships are written when there are no target stacks"
      );
    }
  }

  mod inventory_selection {
    use pretty_assertions::assert_eq;

    use super::*;

    fn row(item_id: i64) -> InventoryRow {
      InventoryRow {
        category: "ship".to_owned(),
        container_id: None,
        depth: 0,
        group_name: "Frigate".to_owned(),
        is_active_ship: false,
        is_blueprint_copy: None,
        is_container: false,
        item_id,
        location_id: 60_003_760,
        location_label: Some("Jita".to_owned()),
        name: None,
        owner_id: 7,
        quantity: 1,
        reproc_value: 0.0,
        row_volume: 10.0,
        type_icon: images::IconResolution::Missing,
        type_id: 587,
        type_name: "Rifter".to_owned(),
        unit_price: 100.0,
        value: 100.0,
      }
    }

    fn seeded(rows: &[i64]) -> State {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.inventory = rows.iter().map(|&id| row(id)).collect();
      state
    }

    async fn db() -> Database {
      crate::store::open_test().await.unwrap()
    }

    async fn seeded_tag(db: &Database, name: &str) -> Tag {
      infra::create_scoped(db, name, None, None, TAG_SCOPE_ASSET)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn a_plain_click_selects_a_single_row_and_a_modifier_click_extends() {
      let db = db().await;
      let mut state = seeded(&[1, 2, 3, 4]);

      let _ = update(&mut state, Message::InventoryRowClicked(2), &db);
      assert!(state.inventory_row_selected(2));
      assert_eq!(state.inventory_selection_count(), 1);

      // Shift extends a contiguous range from the anchor.
      let _ = update(
        &mut state,
        Message::InventoryModifiersChanged(iced::keyboard::Modifiers::SHIFT),
        &db,
      );
      let _ = update(&mut state, Message::InventoryRowClicked(4), &db);
      assert_eq!(
        state.inventory_selection.ordered(&state.inventory_order()),
        vec![2, 3, 4]
      );

      // ⌘ toggles an individual row out.
      let _ = update(
        &mut state,
        Message::InventoryModifiersChanged(iced::keyboard::Modifiers::COMMAND),
        &db,
      );
      let _ = update(&mut state, Message::InventoryRowClicked(3), &db);
      assert_eq!(state.inventory_selection.ordered(&state.inventory_order()), vec![2, 4]);
    }

    #[tokio::test]
    async fn a_right_click_outside_the_selection_reselects_then_opens_the_menu() {
      let db = db().await;
      let mut state = seeded(&[1, 2, 3]);
      let _ = update(&mut state, Message::InventoryRowClicked(1), &db);
      let _ = update(
        &mut state,
        Message::InventoryCursorMoved(iced::Point::new(20.0, 40.0)),
        &db,
      );

      let _ = update(&mut state, Message::InventoryRowRightPressed(3), &db);

      // Row 3 was not selected, so it becomes the lone selection, and the menu opens at the cursor.
      assert_eq!(state.inventory_selection.ordered(&state.inventory_order()), vec![3]);
      assert_eq!(state.inventory_menu(), Some(iced::Point::new(20.0, 40.0)));

      let _ = update(&mut state, Message::InventoryMenuDismissed, &db);
      assert!(state.inventory_menu().is_none());
    }

    #[tokio::test]
    async fn hovering_a_row_tracks_it_and_leaving_clears_it() {
      let db = db().await;
      let mut state = seeded(&[1, 2, 3]);
      assert!(!state.inventory_row_hovered(2), "no row is hovered initially");

      let _ = update(&mut state, Message::InventoryRowHovered(Some(2)), &db);
      assert!(state.inventory_row_hovered(2), "the hovered row is tracked");
      assert!(!state.inventory_row_hovered(1));

      let _ = update(&mut state, Message::InventoryRowHovered(None), &db);
      assert!(!state.inventory_row_hovered(2), "leaving the row clears the hover");
    }

    #[tokio::test]
    async fn a_right_click_inside_the_selection_keeps_it() {
      let db = db().await;
      let mut state = seeded(&[1, 2, 3]);
      let _ = update(&mut state, Message::InventoryRowClicked(1), &db);
      let _ = update(
        &mut state,
        Message::InventoryModifiersChanged(iced::keyboard::Modifiers::SHIFT),
        &db,
      );
      let _ = update(&mut state, Message::InventoryRowClicked(3), &db);
      let _ = update(
        &mut state,
        Message::InventoryCursorMoved(iced::Point::new(5.0, 5.0)),
        &db,
      );

      let _ = update(&mut state, Message::InventoryRowRightPressed(2), &db);

      // The whole range stays selected — the menu acts on it, not just the pointed row.
      assert_eq!(
        state.inventory_selection.ordered(&state.inventory_order()),
        vec![1, 2, 3]
      );
    }

    #[tokio::test]
    async fn a_reload_prunes_rows_that_left_the_filter() {
      let db = db().await;
      let mut state = seeded(&[1, 2, 3]);
      let _ = update(&mut state, Message::InventoryRowClicked(2), &db);
      let _ = update(
        &mut state,
        Message::InventoryModifiersChanged(iced::keyboard::Modifiers::COMMAND),
        &db,
      );
      let _ = update(&mut state, Message::InventoryRowClicked(3), &db);
      assert_eq!(state.inventory_selection_count(), 2);

      // A reload narrows the inventory to row 2 only; row 3 drops out of the selection.
      let loaded = Loaded {
        inventory: vec![row(2)],
        ..Loaded::default()
      };
      apply_loaded(&mut state, loaded);

      assert_eq!(state.inventory_selection.ordered(&state.inventory_order()), vec![2]);
      assert!(!state.inventory_row_selected(3));
    }

    /// Opening Edit Tags over a selection captures every selected stack into `selection_tag_ids`, the
    /// set the modal's Assign/Unassign arms fan their write across. The async writes themselves are
    /// driven separately (the returned `Task` is not executed in a unit test) to assert the bulk effect.
    #[tokio::test]
    async fn opening_edit_tags_targets_every_selected_stack() {
      let db = db().await;
      let mut state = seeded(&[10, 20, 30]);

      let _ = update(&mut state, Message::InventoryRowClicked(10), &db);
      let _ = update(
        &mut state,
        Message::InventoryModifiersChanged(iced::keyboard::Modifiers::SHIFT),
        &db,
      );
      let _ = update(&mut state, Message::InventoryRowClicked(30), &db);
      let _ = update(&mut state, Message::OpenSelectionTagModal, &db);

      assert!(state.asset_tag_modal().is_some());
      assert!(state.inventory_menu().is_none());
      assert_eq!(state.selection_tag_ids, vec![10, 20, 30]);
    }

    #[tokio::test]
    async fn a_bulk_assign_applies_the_tag_to_every_targeted_stack() {
      let db = db().await;
      let keep = seeded_tag(&db, "Keep").await;
      let mut state = seeded(&[10, 20, 30]);
      state.selection_tag_ids = vec![10, 20, 30];

      // Mirror the write the modal's Assign arm fans across `selection_tag_ids`.
      for &id in &state.selection_tag_ids {
        infra::assign(&db, ENTITY_TYPE_ASSET, id, keep.id()).await.unwrap();
      }
      let _ = update(&mut state, reload_asset_tags(db.clone()).await, &db);

      for id in [10, 20, 30] {
        assert_eq!(
          state.asset_tags_for(id).iter().map(|t| t.id()).collect::<Vec<_>>(),
          vec![keep.id()],
          "stack {id} should carry the bulk-assigned tag"
        );
      }
    }

    #[tokio::test]
    async fn a_bulk_unassign_removes_the_tag_from_every_targeted_stack() {
      let db = db().await;
      let keep = seeded_tag(&db, "Keep").await;
      for id in [10, 20, 30] {
        infra::assign(&db, ENTITY_TYPE_ASSET, id, keep.id()).await.unwrap();
      }
      let mut state = seeded(&[10, 20, 30]);
      state.selection_tag_ids = vec![10, 20, 30];

      // Mirror the write the modal's Unassign arm fans across `selection_tag_ids`.
      for &id in &state.selection_tag_ids {
        infra::unassign(&db, ENTITY_TYPE_ASSET, id, keep.id()).await.unwrap();
      }
      let _ = update(&mut state, reload_asset_tags(db.clone()).await, &db);

      for id in [10, 20, 30] {
        assert!(
          state.asset_tags_for(id).is_empty(),
          "stack {id} should have the bulk-unassigned tag removed"
        );
      }
    }

    #[test]
    fn a_chip_unassign_with_no_modal_open_targets_the_messages_own_stack() {
      let state = seeded(&[10, 20, 30]);

      // No modal open, so `selection_tag_ids` is empty: the unassign must target the chip's own stack.
      assert_eq!(tag_write_targets(&state, 20), vec![20]);
    }

    #[test]
    fn a_tag_write_with_a_populated_selection_fans_across_the_whole_selection() {
      let mut state = seeded(&[10, 20, 30]);
      state.selection_tag_ids = vec![10, 20, 30];

      // With a bulk modal open the selection wins over the message's single stack.
      assert_eq!(tag_write_targets(&state, 10), vec![10, 20, 30]);
    }

    #[tokio::test]
    async fn a_chip_unassign_through_update_removes_the_tag_from_the_clicked_stack() {
      let db = db().await;
      let keep = seeded_tag(&db, "Keep").await;
      for id in [10, 20] {
        infra::assign(&db, ENTITY_TYPE_ASSET, id, keep.id()).await.unwrap();
      }
      let mut state = seeded(&[10, 20]);
      let _ = update(&mut state, reload_asset_tags(db.clone()).await, &db);

      // Driving the chip's `×` message through `update` with no modal open must not short-circuit:
      // the prior bug returned `Task::none()` when `selection_tag_ids` was empty, so nothing happened.
      let task = update(
        &mut state,
        Message::AssetTagModal(AddTagMessage::Unassign {
          entity_id: 20,
          entity_type: ENTITY_TYPE_ASSET,
          tag_id: keep.id(),
        }),
        &db,
      );
      assert!(
        task.units() > 0,
        "the chip unassign must produce a write task, not a no-op"
      );

      // Apply the unassign the task fans (mirrors the task's body) and reload to observe the effect.
      infra::unassign(&db, ENTITY_TYPE_ASSET, 20, keep.id()).await.unwrap();
      let _ = update(&mut state, reload_asset_tags(db.clone()).await, &db);

      assert!(
        state.asset_tags_for(20).is_empty(),
        "the clicked stack should have its tag removed"
      );
      assert_eq!(
        state.asset_tags_for(10).iter().map(|t| t.id()).collect::<Vec<_>>(),
        vec![keep.id()],
        "an untouched stack keeps its tag"
      );
    }

    #[tokio::test]
    async fn the_bulk_partition_uses_the_intersection_of_the_selection() {
      let db = db().await;
      let keep = seeded_tag(&db, "Keep").await;
      let sell = seeded_tag(&db, "Sell").await;
      // Keep is on both stacks; Sell only on one — so only Keep is "assigned" across the selection.
      infra::assign(&db, ENTITY_TYPE_ASSET, 10, keep.id()).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_ASSET, 20, keep.id()).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_ASSET, 10, sell.id()).await.unwrap();

      let mut state = seeded(&[10, 20]);
      let _ = update(&mut state, reload_asset_tags(db.clone()).await, &db);
      let _ = update(&mut state, Message::InventoryRowClicked(10), &db);
      let _ = update(
        &mut state,
        Message::InventoryModifiersChanged(iced::keyboard::Modifiers::SHIFT),
        &db,
      );
      let _ = update(&mut state, Message::InventoryRowClicked(20), &db);
      let _ = update(&mut state, Message::OpenSelectionTagModal, &db);

      let (assigned, assignable) = state.asset_tag_modal_partition();
      assert_eq!(assigned.iter().map(|t| t.name().as_str()).collect::<Vec<_>>(), ["Keep"]);
      assert_eq!(
        assignable.iter().map(|t| t.name().as_str()).collect::<Vec<_>>(),
        ["Sell"]
      );
    }

    #[tokio::test]
    async fn the_selection_modal_titles_a_multi_row_selection_by_count() {
      let db = db().await;
      let mut state = seeded(&[10, 20]);
      let _ = update(&mut state, Message::InventoryRowClicked(10), &db);
      let _ = update(
        &mut state,
        Message::InventoryModifiersChanged(iced::keyboard::Modifiers::SHIFT),
        &db,
      );
      let _ = update(&mut state, Message::InventoryRowClicked(20), &db);
      let _ = update(&mut state, Message::OpenSelectionTagModal, &db);

      assert_eq!(state.asset_tag_modal_entity_name(), "2 stacks");
    }

    #[test]
    fn it_renders_the_inventory_context_menu_overlay() {
      let mut state = seeded(&[1, 2]);
      state.tab = Tab::Inventory;
      state
        .inventory_selection
        .apply(1, ClickKind::Plain, &state.inventory_order());
      state.inventory_menu = Some(iced::Point::new(10.0, 10.0));

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn opening_the_inventory_menu_keeps_the_base_at_a_stable_tree_position() {
      use iced::advanced::widget::Tree;

      // Iced matches a scrollable's internal offset by tree position + tag; if the base moved between
      // the closed (root container) and open (Stack child[0]) shells, the list would snap to the top
      // on right-click. The base must stay at child[0] with the same tag whether or not a menu is open.
      let mut closed = seeded(&[1, 2]);
      closed.tab = Tab::Inventory;
      let closed_view = view(&closed, Utc::now());
      let closed_tree = Tree::new(closed_view.as_widget());

      let mut open = seeded(&[1, 2]);
      open.tab = Tab::Inventory;
      open
        .inventory_selection
        .apply(1, ClickKind::Plain, &open.inventory_order());
      open.inventory_menu = Some(iced::Point::new(10.0, 10.0));
      let open_view = view(&open, Utc::now());
      let open_tree = Tree::new(open_view.as_widget());

      assert_eq!(closed_tree.children[0].tag, open_tree.children[0].tag);
    }
  }
}
