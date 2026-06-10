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

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use iced::{Element, Task, widget::text_editor};

pub use self::{
  stockpile_multibuy::parse as parse_multibuy,
  stockpile_search::{MultibuyResolution, resolve_multibuy, search_item_types, search_locations},
  stockpiles::{Editor, SEARCH_MIN_CHARS as STOCKPILE_SEARCH_MIN_CHARS, save_stockpile},
};
use crate::{
  store::{
    Database, images,
    model::{
      StatTemplate,
      asset_query::{
        GeoTree, InventoryCursor, InventoryQuery, InventoryRow, InventoryTotals, SortColumn, SortDirection,
      },
    },
    repo::{assets, character, org},
  },
  ui::components::resizable_pane::{self, PaneDrag},
  window_state::UiState,
};

const INVENTORY_PAGE_SIZE: i64 = 200;
const INVENTORY_SCROLL_THRESHOLD: f32 = 0.85;
const HEADER_SIDE_PADDING: f32 = 28.0;

const SIDEBAR_PANE_KEY: &str = "assets.sidebar";
const SIDEBAR_DEFAULT_WIDTH: f32 = 280.0;
const ABYSSALS_FILTER_PANE_KEY: &str = "assets.abyssals_filter";
const ABYSSALS_FILTER_DEFAULT_WIDTH: f32 = 240.0;
const ABYSSAL_PAGE_SIZE: usize = 50;
const ABYSSAL_PAGE_STEP: usize = 25;
const ABYSSAL_SCROLL_THRESHOLD: f32 = 0.85;
const MUTAMARKET_MODULE_URL: &str = "https://mutamarket.com/modules";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Scope {
  #[default]
  All,
  Character(i64),
  Corporation(i64),
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

  pub(super) fn label(self) -> &'static str {
    match self {
      Category::All => "All",
      Category::Blueprint => "Blueprints",
      Category::Book => "Skill Books",
      Category::Charge => "Charges",
      Category::Commodity => "Commodities",
      Category::Drone => "Drones",
      Category::Implant => "Implants",
      Category::Material => "Materials",
      Category::Module => "Modules",
      Category::Ship => "Ships",
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SliderEndpoint {
  Max,
  Min,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RosterPilot {
  pub corp: String,
  pub id: i64,
  pub name: String,
  pub portrait: images::ImageState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RosterCorp {
  pub id: i64,
  pub logo: images::ImageState,
  pub name: String,
  pub ticker: String,
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
  corporations: Vec<RosterCorp>,
  geo_tree: GeoTree,
  inventory: Vec<InventoryRow>,
  roster: Vec<RosterPilot>,
  totals: InventoryTotals,
  values: values::ValueSummary,
  nav: tracker::NavSeries,
  stockpiles: Vec<stockpiles::StockpileCard>,
  abyssals: abyssals::AbyssalsData,
}

#[derive(Clone, Debug)]
pub enum Message {
  AbyssalCardsReloaded(Vec<abyssals::AbyssalCard>),
  AbyssalFilterReset,
  AbyssalGridScrolled(f32),
  AbyssalMutaMarketOpened(i64),
  AbyssalPickerToggled,
  AbyssalSliderEditCommitted(i64, SliderEndpoint),
  AbyssalSliderEditInput(String),
  AbyssalSliderEditStarted(i64, SliderEndpoint, f64),
  AbyssalSourceTypeSelected(Option<i64>),
  AbyssalStatMaxChanged(i64, f64),
  AbyssalStatMinChanged(i64, f64),
  #[allow(dead_code)]
  AbyssalStatRangeChanged(i64, SliderEndpoint, f64),
  AbyssalStatTemplatesLoaded(Vec<StatTemplate>),
  AbyssalTypeModalClosed,
  AbyssalTypeModalOpened,
  AssetChartHovered(Option<f32>),
  CategorySelected(Category),
  ContainerChildrenLoaded(i64, Vec<InventoryRow>),
  ContainerToggled(i64),
  FilterExamplePicked(&'static str),
  GeoNodeSelected(GeoSelection),
  GeoNodeToggled(GeoNodeKey),
  InventoryHelpToggled,
  InventoryPageLoaded(Vec<InventoryRow>),
  InventoryScrolled(f32),
  Loaded(Box<Loaded>),
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart(Pane),
  #[allow(dead_code)]
  PaneSettled(&'static str, f32),
  PickerToggled,
  ScopeSelected(Scope),
  SearchChanged(String),
  SearchSubmitted,
  SortSelected(SortColumn),
  StockpileCardRightPressed(i64),
  StockpileContextMenuClosed,
  StockpileCursorMoved(iced::Point),
  StockpileDeleted(i64),
  StockpileEditStarted(i64),
  StockpileEditorClosed,
  StockpileEditorItemAdded,
  StockpileEditorItemPicked(usize, i64, String),
  StockpileEditorItemRemoved(usize),
  StockpileEditorItemResults(usize, Vec<(i64, String)>),
  StockpileEditorItemSearchChanged(usize, String),
  StockpileEditorItemTargetChanged(usize, String),
  StockpileEditorLocationCleared,
  StockpileEditorLocationPicked(i64, String),
  StockpileEditorLocationResults(Vec<(i64, String)>),
  StockpileEditorLocationSearchChanged(String),
  StockpileEditorNameChanged(String),
  StockpileEditorSaved,
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
  TabSelected(Tab),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pane {
  AbyssalsFilter,
  Sidebar,
}

#[derive(Clone, Debug)]
pub(super) struct StockpileContextMenu {
  pub anchor: iced::Point,
  pub id: i64,
  pub name: String,
}

#[derive(Debug)]
pub struct State {
  abyssals_filter: PaneDrag,
  active: Scope,
  category: Category,
  chart_hover: Option<f32>,
  corporations: Vec<RosterCorp>,
  expanded_containers: HashSet<i64>,
  geo_expanded: HashSet<GeoNodeKey>,
  geo_selected: GeoSelection,
  geo_tree: GeoTree,
  inventory: Vec<InventoryRow>,
  inventory_children: HashMap<i64, Vec<InventoryRow>>,
  inventory_has_more: bool,
  inventory_help_open: bool,
  inventory_loading: bool,
  picker_open: bool,
  roster: Vec<RosterPilot>,
  search: String,
  sort: SortColumn,
  sort_dir: SortDirection,
  sidebar: PaneDrag,
  tab: Tab,
  totals: InventoryTotals,
  values: values::ValueSummary,
  nav: tracker::NavSeries,
  stockpiles: Vec<stockpiles::StockpileCard>,
  stockpile_editor: Option<stockpiles::Editor>,
  stockpile_import: Option<stockpiles::ImportPanel>,
  stockpile_context_menu: Option<StockpileContextMenu>,
  stockpile_cursor: Option<iced::Point>,
  stockpile_expanded: HashSet<i64>,
  stockpile_multibuy_export: Option<i64>,
  stockpile_multibuy_copied: bool,
  stockpile_multibuy_mode: stockpiles::MultibuyMode,
  abyssals: Vec<abyssals::AbyssalCard>,
  abyssal_source_types: Vec<abyssals::SourceTypeFilter>,
  abyssal_filters: abyssals::Filters,
  abyssal_picker_open: bool,
  abyssal_slider_edit: Option<(i64, SliderEndpoint)>,
  abyssal_slider_edit_text: String,
  abyssal_stat_templates: Vec<StatTemplate>,
  abyssal_visible_count: usize,
}

impl State {
  pub fn new() -> Self {
    State {
      abyssals_filter: PaneDrag::new(ABYSSALS_FILTER_DEFAULT_WIDTH),
      active: Scope::default(),
      category: Category::default(),
      chart_hover: None,
      corporations: Vec::new(),
      expanded_containers: HashSet::new(),
      geo_expanded: HashSet::new(),
      geo_selected: GeoSelection::default(),
      geo_tree: GeoTree::default(),
      inventory: Vec::new(),
      inventory_children: HashMap::new(),
      inventory_has_more: false,
      inventory_help_open: false,
      inventory_loading: false,
      picker_open: false,
      roster: Vec::new(),
      search: String::new(),
      sidebar: PaneDrag::new(SIDEBAR_DEFAULT_WIDTH),
      sort: SortColumn::Value,
      sort_dir: SortDirection::Descending,
      tab: Tab::default(),
      totals: InventoryTotals::default(),
      values: values::ValueSummary::default(),
      nav: tracker::NavSeries::default(),
      stockpiles: Vec::new(),
      stockpile_editor: None,
      stockpile_import: None,
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
      abyssal_visible_count: ABYSSAL_PAGE_SIZE,
    }
  }

  #[allow(dead_code)]
  pub fn with_restored_panes(mut self, ui: &UiState) -> Self {
    self.sidebar = PaneDrag::from_store(ui, SIDEBAR_PANE_KEY, SIDEBAR_DEFAULT_WIDTH);
    self.abyssals_filter = PaneDrag::from_store(ui, ABYSSALS_FILTER_PANE_KEY, ABYSSALS_FILTER_DEFAULT_WIDTH);
    self
  }

  pub fn active(&self) -> Scope {
    self.active
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

  pub(super) fn stockpile_editor(&self) -> Option<&stockpiles::Editor> {
    self.stockpile_editor.as_ref()
  }

  pub fn take_stockpile_editor(&mut self) -> Option<stockpiles::Editor> {
    self.stockpile_editor.take()
  }

  pub(super) fn stockpile_import(&self) -> Option<&stockpiles::ImportPanel> {
    self.stockpile_import.as_ref()
  }

  pub fn stockpile_import_text(&self) -> Option<String> {
    self.stockpile_import.as_ref().map(stockpiles::ImportPanel::text)
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

  pub(super) fn abyssal_visible_count(&self) -> usize {
    self.abyssal_visible_count
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

fn reload(db: &Database, scope: Scope, inventory: InventoryView) -> Task<Message> {
  Task::perform(load_assets(db.clone(), scope, inventory), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
}

#[derive(Clone, Debug)]
struct InventoryView {
  filter: String,
  location_ids: Vec<i64>,
  sort: SortColumn,
  sort_dir: SortDirection,
}

impl Default for InventoryView {
  fn default() -> Self {
    Self {
      filter: String::new(),
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
      location_ids: location_ids_for_selection(&state.geo_tree, state.geo_selected),
      sort: state.sort,
      sort_dir: state.sort_dir,
    }
  }
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
    | Message::SearchSubmitted
    | Message::SortSelected(_)
    | Message::TabSelected(_) => update_inventory(state, message, db),

    Message::ContainerChildrenLoaded(..)
    | Message::ContainerToggled(_)
    | Message::InventoryPageLoaded(_)
    | Message::InventoryScrolled(_) => update_pagination(state, message, db),

    Message::GeoNodeSelected(_) | Message::GeoNodeToggled(_) => update_geo(state, message, db),

    Message::StockpileCardRightPressed(_)
    | Message::StockpileContextMenuClosed
    | Message::StockpileCursorMoved(_)
    | Message::StockpileDeleted(_)
    | Message::StockpileEditStarted(_)
    | Message::StockpileEditorClosed
    | Message::StockpileEditorItemAdded
    | Message::StockpileEditorItemPicked(..)
    | Message::StockpileEditorItemRemoved(_)
    | Message::StockpileEditorItemResults(..)
    | Message::StockpileEditorItemSearchChanged(..)
    | Message::StockpileEditorItemTargetChanged(..)
    | Message::StockpileEditorLocationCleared
    | Message::StockpileEditorLocationPicked(..)
    | Message::StockpileEditorLocationResults(_)
    | Message::StockpileEditorLocationSearchChanged(_)
    | Message::StockpileEditorNameChanged(_)
    | Message::StockpileEditorSaved
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
    | Message::AbyssalGridScrolled(_)
    | Message::AbyssalMutaMarketOpened(_)
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
  }
}

fn update_inventory(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::Loaded(loaded) => {
      let Loaded {
        corporations,
        geo_tree,
        inventory,
        roster,
        totals,
        values,
        nav,
        stockpiles,
        abyssals,
      } = *loaded;
      state.corporations = corporations;
      state.inventory_has_more = inventory.len() as i64 == INVENTORY_PAGE_SIZE;
      state.inventory = inventory;
      state.inventory_loading = false;
      state.expanded_containers.clear();
      state.inventory_children.clear();
      state.roster = roster;
      state.totals = totals;
      state.values = values;
      state.nav = nav;
      state.stockpiles = stockpiles;
      state.abyssals = abyssals.cards;
      state.abyssal_source_types = abyssals.source_types;
      state.abyssal_filters = abyssals::Filters::default();
      state.abyssal_picker_open = false;
      state.abyssal_slider_edit = None;
      state.abyssal_slider_edit_text = String::new();
      state.abyssal_stat_templates = Vec::new();
      state.abyssal_visible_count = ABYSSAL_PAGE_SIZE;
      state.geo_tree = geo_tree;
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
      reload(db, scope, InventoryView::from_state(state))
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
      Task::none()
    }
    Message::SearchSubmitted => reload(db, state.active, InventoryView::from_state(state)),
    Message::CategorySelected(category) => {
      if state.category == category {
        return Task::none();
      }
      state.category = category;
      reload(db, state.active, InventoryView::from_state(state))
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
      reload(db, state.active, InventoryView::from_state(state))
    }
    Message::InventoryHelpToggled => {
      state.inventory_help_open = !state.inventory_help_open;
      Task::none()
    }
    Message::FilterExamplePicked(query) => {
      state.search = query.to_owned();
      state.inventory_help_open = false;
      reload(db, state.active, InventoryView::from_state(state))
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
      reload(db, state.active, InventoryView::from_state(state))
    }
    _ => Task::none(),
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
    Message::InventoryScrolled(offset) => {
      if offset < INVENTORY_SCROLL_THRESHOLD || !state.inventory_has_more || state.inventory_loading {
        return Task::none();
      }
      let Some(cursor) = state.inventory.last().map(|row| row.cursor(state.sort)) else {
        return Task::none();
      };
      state.inventory_loading = true;
      let view = InventoryView::from_state(state);
      let (db, scope, roster, corporations) = (
        db.clone(),
        state.active,
        state.roster.clone(),
        state.corporations.clone(),
      );
      Task::perform(
        async move { load_inventory_page(&db, scope, &roster, &corporations, &view, cursor).await },
        Message::InventoryPageLoaded,
      )
    }
    Message::InventoryPageLoaded(rows) => {
      state.inventory_loading = false;
      state.inventory_has_more = rows.len() as i64 == INVENTORY_PAGE_SIZE;
      state.inventory.extend(rows);
      Task::none()
    }
    Message::ContainerToggled(item_id) => {
      if state.expanded_containers.remove(&item_id) {
        state.inventory_children.remove(&item_id);
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
      Task::perform(
        async move { load_container_children(&db, scope, &roster, &corporations, item_id).await },
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
  let message = match apply_stockpile_editor(state, message) {
    Ok(task) => return task,
    Err(message) => message,
  };
  match apply_stockpile_import(state, message) {
    Ok(task) => task,
    Err(message) => update_stockpile_lifecycle(state, message, db),
  }
}

fn apply_stockpile_editor(state: &mut State, message: Message) -> Result<Task<Message>, Message> {
  let Some(editor) = state.stockpile_editor.as_mut() else {
    return match message {
      Message::StockpileNew => {
        state.stockpile_editor = Some(stockpiles::Editor::blank());
        Ok(Task::none())
      }
      other => Err(other),
    };
  };
  match message {
    Message::StockpileNew => state.stockpile_editor = Some(stockpiles::Editor::blank()),
    Message::StockpileEditorNameChanged(name) => editor.set_name(name),
    Message::StockpileEditorLocationSearchChanged(value) => editor.set_location_query(value),
    Message::StockpileEditorLocationResults(results) => editor.set_location_suggestions(results),
    Message::StockpileEditorLocationPicked(id, name) => editor.pick_location(id, name),
    Message::StockpileEditorLocationCleared => editor.clear_location(),
    Message::StockpileEditorItemSearchChanged(index, value) => editor.set_item_query(index, value),
    Message::StockpileEditorItemResults(index, results) => editor.set_item_suggestions(index, results),
    Message::StockpileEditorItemPicked(index, id, name) => editor.pick_item(index, id, name),
    Message::StockpileEditorItemTargetChanged(index, value) => editor.set_item_target(index, value),
    Message::StockpileEditorItemAdded => editor.add_item(),
    Message::StockpileEditorItemRemoved(index) => editor.remove_item(index),
    Message::StockpileEditorClosed => state.stockpile_editor = None,
    other => return Err(other),
  }
  Ok(Task::none())
}

fn apply_stockpile_import(state: &mut State, message: Message) -> Result<Task<Message>, Message> {
  match message {
    Message::StockpileImportOpened => state.stockpile_import = Some(stockpiles::ImportPanel::blank()),
    Message::StockpileImportClosed => state.stockpile_import = None,
    Message::StockpileImportTextChanged(action) => {
      if let Some(panel) = state.stockpile_import.as_mut() {
        panel.apply(action);
      }
    }
    Message::StockpileImportResolved(resolution) => {
      if let Some(panel) = state.stockpile_import.as_mut() {
        panel.set_resolution(resolution);
      }
    }
    Message::StockpileImportConfirmed => {
      let matched = state
        .stockpile_import
        .as_ref()
        .map(|panel| panel.matched().to_vec())
        .unwrap_or_default();
      state.stockpile_import = None;
      let editor = state.stockpile_editor.get_or_insert_with(stockpiles::Editor::blank);
      editor.prefill_items(&matched);
    }
    other => return Err(other),
  }
  Ok(Task::none())
}

fn update_stockpile_lifecycle(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::StockpileEditStarted(id) => {
      state.stockpile_context_menu = None;
      state.stockpile_editor = state
        .stockpiles
        .iter()
        .find(|card| card.id == id)
        .map(stockpiles::Editor::from_card);
      Task::none()
    }
    Message::StockpileEditorSaved => {
      let Some(editor) = state.stockpile_editor.take() else {
        return Task::none();
      };
      let db = db.clone();
      Task::perform(
        async move {
          stockpiles::save(&db, &editor).await;
          stockpiles::load_cards(&db).await
        },
        Message::StockpilesReloaded,
      )
    }
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
    Message::AbyssalGridScrolled(offset) => {
      if offset >= ABYSSAL_SCROLL_THRESHOLD && state.abyssal_visible_count < state.abyssals.len() {
        state.abyssal_visible_count = (state.abyssal_visible_count + ABYSSAL_PAGE_STEP).min(state.abyssals.len());
      }
      Task::none()
    }
    Message::AbyssalStatTemplatesLoaded(templates) => {
      state.abyssal_stat_templates = templates;
      Task::none()
    }
    Message::AbyssalCardsReloaded(cards) => {
      state.abyssals = cards;
      state.abyssal_visible_count = ABYSSAL_PAGE_SIZE;
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
      let width = drag.width();
      Task::done(Message::PaneSettled(key, width))
    }
    _ => Task::none(),
  }
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  if state.active_drag().is_none() {
    return iced::Subscription::none();
  }
  iced::event::listen_with(|event, _status, _id| {
    resizable_pane::drag_event(event, Message::PaneDrag, Message::PaneDragEnd)
  })
}

pub fn view(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  shell::shell(state, now)
}

async fn load_assets(db: Database, scope: Scope, view: InventoryView) -> Loaded {
  let roster = load_roster(&db).await;
  let corporations = load_corporations(&db).await;

  let (totals, inventory) = match resolve_scope_owner(scope, &roster, &corporations) {
    Some(owner) => load_scope(&db, &owner, &view).await,
    None => (InventoryTotals::default(), Vec::new()),
  };
  let geo_tree = tree::load_geo_tree(&db, scope, &roster, &corporations).await;
  let values = values::summarize(&inventory, &roster);
  let nav = tracker::load_series(&db, scope).await;
  let stockpiles = stockpiles::load_cards(&db).await;
  let abyssals = abyssals::load_cards(&db, scope, &roster).await;

  Loaded {
    corporations,
    geo_tree,
    inventory,
    roster,
    totals,
    values,
    nav,
    stockpiles,
    abyssals,
  }
}

async fn load_scope(db: &Database, owner: &Owner, view: &InventoryView) -> (InventoryTotals, Vec<InventoryRow>) {
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
    limit: INVENTORY_PAGE_SIZE,
    location_ids: &view.location_ids,
    me_id,
    sort: view.sort,
  };

  match owner {
    Owner::Character(id) => {
      let totals = assets::inventory_totals_for_character(db, *id, &view.filter, me_id)
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
      let totals = assets::inventory_totals_for_combined(db, character_ids, corporation_ids, &view.filter, None)
        .await
        .unwrap_or_default();
      let inventory = assets::inventory_page_for_combined(db, character_ids, corporation_ids, &query)
        .await
        .unwrap_or_default();
      (totals, inventory)
    }
    Owner::Corporation(id) => {
      let totals = assets::inventory_totals_for_corporation(db, *id, &view.filter, None)
        .await
        .unwrap_or_default();
      let inventory = assets::inventory_page_for_corporation(db, *id, &query)
        .await
        .unwrap_or_default();
      (totals, inventory)
    }
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
) -> Vec<InventoryRow> {
  let Some(owner) = resolve_scope_owner(scope, roster, corporations) else {
    return Vec::new();
  };
  match &owner {
    Owner::Character(id) => assets::children_render_for_character(db, *id, container_id)
      .await
      .unwrap_or_default(),
    Owner::Combined {
      character_ids,
      corporation_ids,
    } => {
      let mut children = assets::children_render_for_characters(db, character_ids, container_id)
        .await
        .unwrap_or_default();
      for corporation_id in corporation_ids {
        children.extend(
          assets::children_render_for_corporation(db, *corporation_id, container_id)
            .await
            .unwrap_or_default(),
        );
      }
      children
    }
    Owner::Corporation(id) => assets::children_render_for_corporation(db, *id, container_id)
      .await
      .unwrap_or_default(),
  }
}

async fn load_roster(db: &Database) -> Vec<RosterPilot> {
  let characters = character::all_owned(db).await.unwrap_or_default();
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

pub fn fmt_isk(value: f64) -> String {
  let magnitude = value.abs();
  if magnitude >= 1e9 {
    format!("{:.2}B", value / 1e9)
  } else if magnitude >= 1e6 {
    format!("{:.1}M", value / 1e6)
  } else if magnitude >= 1e3 {
    format!("{:.1}K", value / 1e3)
  } else {
    format!("{value:.0}")
  }
}

pub fn fmt_volume(volume: f64) -> String {
  let magnitude = volume.abs();
  // \u{b3} is the superscript-three (cubic-metre) glyph, not an ASCII '3'.
  if magnitude >= 1e6 {
    format!("{:.1}Mm\u{b3}", volume / 1e6)
  } else if magnitude >= 1e3 {
    format!("{:.1}km\u{b3}", volume / 1e3)
  } else {
    format!("{volume:.0}m\u{b3}")
  }
}

pub fn fmt_count(count: i64) -> String {
  let digits = count.abs().to_string();
  let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
  for (index, ch) in digits.chars().enumerate() {
    if index > 0 && (digits.len() - index).is_multiple_of(3) {
      grouped.push(',');
    }
    grouped.push(ch);
  }
  if count < 0 { format!("-{grouped}") } else { grouped }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn pilot(id: i64) -> RosterPilot {
    RosterPilot {
      corp: "TST".to_owned(),
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

  mod stale_images {
    use pretty_assertions::assert_eq;

    use super::*;

    fn fresh_pilot(id: i64) -> RosterPilot {
      RosterPilot {
        corp: "TST".to_owned(),
        id,
        name: format!("Pilot {id}"),
        portrait: images::ImageState::Fresh(std::path::PathBuf::from(format!("/cache/{id}.jpg"))),
      }
    }

    #[test]
    fn it_is_empty_when_every_model_is_fresh() {
      let mut state = State::new();
      state.set_picker_for_test(Scope::All, vec![fresh_pilot(7)], Vec::new());

      assert_eq!(state.stale_images(), Vec::new());
    }

    #[test]
    fn it_collects_stale_portraits_logos_and_abyssal_cards() {
      let mut state = State::new();
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
    fn it_resolves_the_all_scope_with_no_owned_characters_to_the_corporations_alone() {
      assert_eq!(
        resolve_scope_owner(Scope::All, &[], &[corp(98)]),
        Some(Owner::Combined {
          character_ids: vec![],
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
  }

  mod fmt {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_isk_by_magnitude() {
      assert_eq!(fmt_isk(2_500_000_000.0), "2.50B");
      assert_eq!(fmt_isk(3_400_000.0), "3.4M");
      assert_eq!(fmt_isk(1_200.0), "1.2K");
      assert_eq!(fmt_isk(42.0), "42");
    }

    #[test]
    fn it_formats_volume_by_magnitude() {
      assert_eq!(fmt_volume(2_500_000.0), "2.5Mm\u{b3}");
      assert_eq!(fmt_volume(3_400.0), "3.4km\u{b3}");
      assert_eq!(fmt_volume(42.0), "42m\u{b3}");
    }

    #[test]
    fn it_groups_counts_in_thousands() {
      assert_eq!(fmt_count(0), "0");
      assert_eq!(fmt_count(42), "42");
      assert_eq!(fmt_count(1_234), "1,234");
      assert_eq!(fmt_count(1_234_567), "1,234,567");
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

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_records_the_loaded_roster_and_totals() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::Loaded(Box::new(Loaded {
          corporations: vec![corp(98)],
          geo_tree: GeoTree::default(),
          inventory: vec![],
          roster: vec![pilot(7)],
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
        })),
        &db,
      );

      assert_eq!(state.roster, vec![pilot(7)]);
      assert_eq!(state.corporations, vec![corp(98)]);
      assert_eq!(state.totals.items, 5);
    }

    #[tokio::test]
    async fn it_toggles_the_picker_open_and_closed() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(state.picker_open);

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(!state.picker_open);
    }

    #[tokio::test]
    async fn it_records_the_selected_scope_and_closes_the_picker() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.picker_open = true;

      let _ = update(&mut state, Message::ScopeSelected(Scope::Character(42)), &db);

      assert_eq!(state.active, Scope::Character(42));
      assert!(!state.picker_open);
    }

    #[tokio::test]
    async fn it_switches_the_active_tab() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::TabSelected(Tab::Values), &db);
      assert_eq!(state.tab, Tab::Values);
    }

    #[tokio::test]
    async fn it_records_the_search_string() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::SearchChanged("tritanium".to_owned()), &db);
      assert_eq!(state.search, "tritanium");
    }

    #[tokio::test]
    async fn it_records_the_selected_inventory_category() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::CategorySelected(Category::Ship), &db);
      assert_eq!(state.category, Category::Ship);
    }

    #[tokio::test]
    async fn it_toggles_the_abyssal_picker() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::AbyssalPickerToggled, &db);
      assert!(state.abyssal_picker_open);
      let _ = update(&mut state, Message::AbyssalPickerToggled, &db);
      assert!(!state.abyssal_picker_open);
    }

    #[tokio::test]
    async fn selecting_an_abyssal_source_type_records_it_and_closes_the_picker() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.abyssal_picker_open = true;
      state.abyssal_filters.stat_ranges.insert(50, (1.0, 2.0));

      let _ = update(&mut state, Message::AbyssalSourceTypeSelected(Some(2410)), &db);

      assert_eq!(state.abyssal_filters.source_type_id, Some(2410));
      assert!(state.abyssal_filters.stat_ranges.is_empty());
      assert!(!state.abyssal_picker_open);
    }

    #[tokio::test]
    async fn resetting_the_abyssal_filters_clears_type_and_ranges() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.abyssal_filters.source_type_id = Some(2410);
      state.abyssal_filters.stat_ranges.insert(50, (1.0, 2.0));

      let _ = update(&mut state, Message::AbyssalFilterReset, &db);

      assert_eq!(state.abyssal_filters, abyssals::Filters::default());
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
        })
        .collect()
    }

    #[tokio::test]
    async fn scrolling_past_the_threshold_grows_the_visible_page_then_caps_at_the_total() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.set_abyssals_for_test(abyssal_cards(60), Vec::new(), abyssals::Filters::default(), false);
      assert_eq!(state.abyssal_visible_count(), ABYSSAL_PAGE_SIZE);

      let _ = update(&mut state, Message::AbyssalGridScrolled(0.9), &db);
      assert_eq!(state.abyssal_visible_count(), 60);

      let _ = update(&mut state, Message::AbyssalGridScrolled(0.9), &db);
      assert_eq!(state.abyssal_visible_count(), 60);
    }

    #[tokio::test]
    async fn scrolling_below_the_threshold_does_not_grow_the_page() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.set_abyssals_for_test(abyssal_cards(200), Vec::new(), abyssals::Filters::default(), false);

      let _ = update(&mut state, Message::AbyssalGridScrolled(0.5), &db);

      assert_eq!(state.abyssal_visible_count(), ABYSSAL_PAGE_SIZE);
    }

    #[tokio::test]
    async fn reloading_cards_resets_the_visible_page() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.set_abyssals_for_test(abyssal_cards(200), Vec::new(), abyssals::Filters::default(), false);
      let _ = update(&mut state, Message::AbyssalGridScrolled(0.9), &db);
      assert_eq!(state.abyssal_visible_count(), ABYSSAL_PAGE_SIZE + ABYSSAL_PAGE_STEP);

      let _ = update(&mut state, Message::AbyssalCardsReloaded(abyssal_cards(200)), &db);

      assert_eq!(state.abyssal_visible_count(), ABYSSAL_PAGE_SIZE);
    }

    #[tokio::test]
    async fn an_abyssal_stat_range_change_without_loaded_templates_is_a_noop() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.abyssal_filters.source_type_id = Some(2410);

      let _ = update(
        &mut state,
        Message::AbyssalStatRangeChanged(50, SliderEndpoint::Min, 5.0),
        &db,
      );

      assert!(state.abyssal_filters.stat_ranges.is_empty());
    }

    #[tokio::test]
    async fn a_clamped_abyssal_stat_range_is_recorded_against_its_attribute() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
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
    async fn committing_a_slider_value_edit_applies_the_typed_bound() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
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

    #[test]
    fn it_resolves_an_unknown_selection_to_no_location_ids() {
      let tree = geo_tree();

      assert!(location_ids_for_selection(&tree, GeoSelection::Region(999)).is_empty());
      assert!(location_ids_for_selection(&tree, GeoSelection::System(999)).is_empty());
    }

    #[tokio::test]
    async fn it_toggles_a_group_expanded_then_collapsed_from_a_collapsed_default() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      let key = GeoNodeKey::Region(10);
      assert!(state.geo_is_collapsed(key), "groups render collapsed by default");

      let _ = update(&mut state, Message::GeoNodeToggled(key), &db);
      assert!(!state.geo_is_collapsed(key));

      let _ = update(&mut state, Message::GeoNodeToggled(key), &db);
      assert!(state.geo_is_collapsed(key));
    }

    #[tokio::test]
    async fn it_records_a_geo_selection() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.set_geo_tree_for_test(geo_tree());

      let _ = update(&mut state, Message::GeoNodeSelected(GeoSelection::System(30)), &db);

      assert_eq!(state.geo_selected(), GeoSelection::System(30));
    }

    #[tokio::test]
    async fn it_resets_the_geo_selection_and_collapse_state_when_the_scope_changes() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.geo_selected = GeoSelection::System(30);
      state.geo_expanded.insert(GeoNodeKey::Region(10));

      let _ = update(&mut state, Message::ScopeSelected(Scope::Character(7)), &db);

      assert_eq!(state.geo_selected(), GeoSelection::All);
      assert!(
        state.geo_is_collapsed(GeoNodeKey::Region(10)),
        "the scope change re-collapses every group"
      );
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
        owner_id: 7,
        quantity: 1,
        row_volume: 10.0,
        type_id: 587,
        type_name: "Rifter".to_owned(),
        unit_price: 100.0,
        value: 100.0,
      }
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
    async fn it_yields_no_rows_when_the_scope_resolves_to_no_owner() {
      let db = crate::store::open_test().await.unwrap();
      let view = InventoryView::default();
      let cursor = inv_row(100, false).cursor(view.sort);

      let rows = load_inventory_page(&db, Scope::Corporation(7), &[], &[], &view, cursor).await;

      assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn it_appends_a_loaded_page_and_clears_has_more_for_a_short_page() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.inventory = vec![inv_row(100, false)];
      state.inventory_has_more = true;
      state.inventory_loading = true;

      let _ = update(&mut state, Message::InventoryPageLoaded(vec![inv_row(101, false)]), &db);

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
    async fn it_ignores_scroll_when_there_are_no_more_pages() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.inventory = vec![inv_row(100, false)];
      state.inventory_has_more = false;

      let _ = update(&mut state, Message::InventoryScrolled(0.95), &db);

      assert!(!state.inventory_loading, "no load is started when the set is exhausted");
    }

    #[tokio::test]
    async fn it_ignores_scroll_below_the_threshold() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.inventory = vec![inv_row(100, false)];
      state.inventory_has_more = true;

      let _ = update(&mut state, Message::InventoryScrolled(0.5), &db);

      assert!(!state.inventory_loading);
    }

    #[tokio::test]
    async fn it_starts_a_load_when_scrolling_past_the_threshold_with_more_pages() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.active = Scope::Character(7);
      state.inventory = vec![inv_row(100, false)];
      state.inventory_has_more = true;

      let _ = update(&mut state, Message::InventoryScrolled(0.9), &db);

      assert!(
        state.inventory_loading,
        "crossing the threshold with more pages starts a fetch"
      );
    }

    #[tokio::test]
    async fn it_expands_a_container_loads_children_then_collapse_drops_them() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
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
    async fn it_resets_pagination_and_expansion_on_reload() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
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
  }

  mod stockpile {
    use pretty_assertions::assert_eq;

    use super::*;

    fn card(id: i64, name: &str) -> stockpiles::StockpileCard {
      stockpiles::StockpileCard {
        character_id: None,
        fill_isk: 0.0,
        id,
        items: vec![],
        location_id: None,
        location_name: None,
        name: name.to_owned(),
        overall_pct: 0.0,
        target_isk: 0.0,
      }
    }

    #[tokio::test]
    async fn it_opens_a_blank_editor_then_closes_it() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::StockpileNew, &db);
      assert!(state.stockpile_editor.is_some());

      let _ = update(&mut state, Message::StockpileEditorClosed, &db);
      assert!(state.stockpile_editor.is_none());
    }

    #[tokio::test]
    async fn it_opens_an_editor_prefilled_from_an_existing_card() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.stockpiles = vec![card(7, "Ammo")];

      let _ = update(&mut state, Message::StockpileEditStarted(7), &db);
      assert_eq!(state.stockpile_editor.as_ref().map(|e| e.name()), Some("Ammo"));

      let _ = update(&mut state, Message::StockpileEditStarted(404), &db);
      assert!(state.stockpile_editor.is_none());
    }

    #[tokio::test]
    async fn it_opens_a_context_menu_at_the_cursor_then_closes_it() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
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
    async fn it_ignores_a_right_press_without_a_cursor_anchor() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.stockpiles = vec![card(7, "Ammo")];

      let _ = update(&mut state, Message::StockpileCardRightPressed(7), &db);

      assert!(state.stockpile_context_menu.is_none());
    }

    #[tokio::test]
    async fn it_toggles_a_card_expanded_then_collapsed() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::StockpileItemsToggled(7), &db);
      assert!(state.stockpile_expanded.contains(&7));

      let _ = update(&mut state, Message::StockpileItemsToggled(7), &db);
      assert!(!state.stockpile_expanded.contains(&7));
    }

    #[tokio::test]
    async fn it_opens_changes_and_closes_the_multibuy_export() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
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
    async fn it_ignores_a_multibuy_copy_for_an_unknown_card() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::StockpileMultibuyExportCopied(404), &db);

      assert!(!state.stockpile_multibuy_copied);
    }

    #[tokio::test]
    async fn it_edits_the_draft_name_and_item_rows() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::StockpileNew, &db);

      let _ = update(
        &mut state,
        Message::StockpileEditorNameChanged("Cap boosters".to_owned()),
        &db,
      );
      assert_eq!(state.stockpile_editor.as_ref().map(|e| e.name()), Some("Cap boosters"));

      let _ = update(&mut state, Message::StockpileEditorItemAdded, &db);
      let _ = update(
        &mut state,
        Message::StockpileEditorItemPicked(1, 34, "Tritanium".to_owned()),
        &db,
      );
      let _ = update(
        &mut state,
        Message::StockpileEditorItemTargetChanged(1, "100".to_owned()),
        &db,
      );
      let editor = state.stockpile_editor.as_ref().unwrap();
      assert_eq!(editor.items().len(), 2);
      assert_eq!(editor.items()[1].type_id, Some(34));
      assert_eq!(editor.items()[1].type_name.as_deref(), Some("Tritanium"));
      assert_eq!(editor.items()[1].target, "100");

      let _ = update(&mut state, Message::StockpileEditorItemRemoved(1), &db);
      assert_eq!(state.stockpile_editor.as_ref().unwrap().items().len(), 1);
    }

    #[tokio::test]
    async fn it_ignores_editor_edits_when_no_editor_is_open() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::StockpileEditorNameChanged("x".to_owned()), &db);
      let _ = update(
        &mut state,
        Message::StockpileEditorItemPicked(0, 1, "Something".to_owned()),
        &db,
      );
      let _ = update(
        &mut state,
        Message::StockpileEditorItemTargetChanged(0, "1".to_owned()),
        &db,
      );
      let _ = update(&mut state, Message::StockpileEditorItemAdded, &db);
      let _ = update(&mut state, Message::StockpileEditorItemRemoved(0), &db);
      assert!(state.stockpile_editor.is_none());
    }

    #[tokio::test]
    async fn saving_with_no_open_editor_is_a_noop() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::StockpileEditorSaved, &db);
      assert!(state.stockpile_editor.is_none());
    }

    #[tokio::test]
    async fn saving_an_open_editor_clears_it_and_spawns_a_reload() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::StockpileNew, &db);

      let _task = update(&mut state, Message::StockpileEditorSaved, &db);
      assert!(state.stockpile_editor.is_none());
    }

    #[tokio::test]
    async fn deleting_spawns_a_reload_and_reloaded_cards_replace_state() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.stockpiles = vec![card(1, "Old")];

      let _task = update(&mut state, Message::StockpileDeleted(1), &db);

      let _ = update(&mut state, Message::StockpilesReloaded(vec![card(2, "New")]), &db);
      assert_eq!(state.stockpiles, vec![card(2, "New")]);
    }

    #[tokio::test]
    async fn it_opens_the_import_panel_records_its_text_then_closes_it() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::StockpileImportOpened, &db);
      assert!(state.stockpile_import.is_some());

      let _ = update(
        &mut state,
        Message::StockpileImportTextChanged(text_editor::Action::Edit(text_editor::Edit::Paste(
          std::sync::Arc::new("Tritanium 100".to_owned()),
        ))),
        &db,
      );
      assert_eq!(state.stockpile_import_text(), Some("Tritanium 100".to_owned()));

      let _ = update(&mut state, Message::StockpileImportClosed, &db);
      assert!(state.stockpile_import.is_none());
    }

    #[tokio::test]
    async fn it_confirms_an_import_by_prefilling_the_editor_and_closing_the_panel() {
      use crate::features::assets::stockpile_search::{MultibuyMatch, MultibuyResolution};

      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      let _ = update(&mut state, Message::StockpileImportOpened, &db);
      let _ = update(
        &mut state,
        Message::StockpileImportResolved(MultibuyResolution {
          matched: vec![MultibuyMatch {
            name: "Tritanium".to_owned(),
            quantity: 100,
            type_id: 34,
          }],
          unmatched: Vec::new(),
        }),
        &db,
      );

      let _ = update(&mut state, Message::StockpileImportConfirmed, &db);

      assert!(state.stockpile_import.is_none());
      let editor = state.stockpile_editor.as_ref().expect("confirm seeds an editor");
      assert!(editor.items().iter().any(|item| item.type_id == Some(34)));
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

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_empty_state_before_any_load() {
      let state = State::new();

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_a_loaded_state() {
      let mut state = State::new();
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
    fn it_renders_the_abyssals_tab_with_its_resizable_filter_rail() {
      let mut state = State::new();
      state.roster = vec![pilot(1)];
      state.tab = Tab::Abyssals;

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_the_scope_picker_overlay() {
      let mut state = State::new();
      state.roster = vec![pilot(1)];
      state.picker_open = true;

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_the_inventory_help_overlay() {
      let mut state = State::new();
      state.roster = vec![pilot(1)];
      state.tab = Tab::Inventory;
      state.inventory_help_open = true;

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_the_abyssal_picker_modal_overlay() {
      let mut state = State::new();
      state.roster = vec![pilot(1)];
      state.tab = Tab::Abyssals;
      state.abyssal_picker_open = true;

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_the_stockpile_context_menu_overlay() {
      let mut state = State::new();
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

  mod panes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_both_pane_widths_when_the_store_is_empty() {
      let state = State::new().with_restored_panes(&UiState::default());

      assert_eq!(state.pane(Pane::Sidebar).width(), SIDEBAR_DEFAULT_WIDTH);
      assert_eq!(state.pane(Pane::AbyssalsFilter).width(), ABYSSALS_FILTER_DEFAULT_WIDTH);
    }

    #[test]
    fn it_does_not_listen_for_drag_events_while_no_pane_is_active() {
      let state = State::new();

      let _sub: iced::Subscription<Message> = subscription(&state);
    }

    #[test]
    fn it_listens_for_drag_events_while_the_abyssals_filter_pane_is_active() {
      let mut state = State::new();
      state.abyssals_filter.start();

      let _sub: iced::Subscription<Message> = subscription(&state);
    }

    #[test]
    fn it_restores_both_pane_widths_from_the_keyed_store() {
      let mut ui = UiState::default();
      ui.panes.insert(SIDEBAR_PANE_KEY.to_owned(), 360.0);
      ui.panes.insert(ABYSSALS_FILTER_PANE_KEY.to_owned(), 200.0);

      let state = State::new().with_restored_panes(&ui);

      assert_eq!(state.pane(Pane::Sidebar).width(), 360.0);
      assert_eq!(state.pane(Pane::AbyssalsFilter).width(), 200.0);
    }

    #[tokio::test]
    async fn it_resizes_the_sidebar_during_a_drag_and_settles_its_width() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::PaneDragStart(Pane::Sidebar), &db);
      let _ = update(&mut state, Message::PaneDrag(500.0), &db);
      let _ = update(&mut state, Message::PaneDrag(560.0), &db);
      assert_eq!(state.pane(Pane::Sidebar).width(), SIDEBAR_DEFAULT_WIDTH + 60.0);
      assert!(state.pane(Pane::Sidebar).is_active());

      let _task = update(&mut state, Message::PaneDragEnd, &db);
      assert!(!state.pane(Pane::Sidebar).is_active());
      assert_eq!(state.pane(Pane::Sidebar).width(), SIDEBAR_DEFAULT_WIDTH + 60.0);
    }

    #[tokio::test]
    async fn it_routes_a_drag_solely_to_the_active_abyssals_filter_pane() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

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

    #[test]
    fn it_persists_the_settled_width_under_the_matching_pane_key() {
      let mut state = State::new();
      assert_eq!(state.pane_mut(Pane::Sidebar).1, SIDEBAR_PANE_KEY);
      assert_eq!(state.pane_mut(Pane::AbyssalsFilter).1, ABYSSALS_FILTER_PANE_KEY);
    }

    #[test]
    fn it_reports_the_active_drag_pane_only_while_dragging() {
      let mut state = State::new();
      assert!(state.active_drag().is_none());

      state.sidebar.start();
      assert_eq!(state.active_drag(), Some(Pane::Sidebar));

      state.sidebar.end();
      state.abyssals_filter.start();
      assert_eq!(state.active_drag(), Some(Pane::AbyssalsFilter));
    }
  }
}
