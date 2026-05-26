//! Assets window view — multi-character asset inventory.

pub mod abyssals_tab;
pub mod drag_overlay;
pub mod header;
pub mod inventory_tab;
pub mod main_panel;
pub mod picker_overlay;
pub mod sidebar;
pub mod stockpiles_tab;
pub mod tracker_tab;
pub mod values_tab;

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
pub use drag_overlay::DragOverlay;
pub use header::Component as Header;
use iced::{
  Background, Element, Event, Length, Subscription, keyboard, mouse,
  widget::{column, container, image, stack},
};
pub use main_panel::Component as MainPanel;
pub use picker_overlay::PickerOverlay;
use pod_model::{Character, Corporation, missing_scopes};
pub use sidebar::Component as Sidebar;

use crate::{
  asset_filter_query::AssetFilterQuery,
  components::{
    CharacterPicker, ScopeMissing,
    character_picker::{self, CharacterEntry, CorporationEntry},
    scope_missing,
  },
  style::color,
};

/// A single resolved asset record loaded from the database.
#[derive(Clone, Debug)]
pub struct AssetRecord {
  pub category_key: String,
  pub character_id: i64,
  /// item_id of the direct parent container, or 0 when not in a container.
  pub container_id: i64,
  /// Non-empty when this item is inside a container. Formatted as
  /// `"<station> · <hangar_flag> · <container type name>"`.
  pub container_path: String,
  pub constellation_id: i32,
  pub constellation_name: String,
  /// Nesting depth: 0 = top-level, 1 = inside one container, etc.
  pub depth: usize,
  pub group_name: String,
  /// `"icon"`, `"bpc"`, or `"bpo"` — determines which cached image to display.
  pub icon_variant: String,
  /// True if at least one other asset is located inside this item.
  pub is_container: bool,
  pub is_singleton: bool,
  pub item_id: i64,
  pub location_id: i64,
  pub location_name: String,
  pub quantity: i64,
  pub region_id: i32,
  pub region_name: String,
  pub system_name: String,
  pub type_id: i32,
  pub type_name: String,
  pub unit_price: f64,
  pub volume: f64,
}

/// Item category display filter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Category {
  #[default]
  All,
  Ship,
  Module,
  Drone,
  Charge,
  Implant,
  Blueprint,
  Material,
  Book,
  Commodity,
}

const CATEGORY_TABLE: &[(&str, &str)] = &[
  ("all", "All"),
  ("ship", "Ships"),
  ("module", "Modules"),
  ("drone", "Drones"),
  ("charge", "Charges"),
  ("implant", "Implants"),
  ("blueprint", "Blueprints"),
  ("material", "Materials"),
  ("book", "Skill Books"),
  ("commodity", "Commodities"),
];

impl Category {
  fn index(&self) -> usize {
    [
      Self::All,
      Self::Ship,
      Self::Module,
      Self::Drone,
      Self::Charge,
      Self::Implant,
      Self::Blueprint,
      Self::Material,
      Self::Book,
      Self::Commodity,
    ]
    .iter()
    .position(|c| c == self)
    .unwrap_or(0)
  }

  pub fn key(&self) -> &'static str {
    CATEGORY_TABLE[self.index()].0
  }

  pub fn label(&self) -> &'static str {
    CATEGORY_TABLE[self.index()].1
  }

  pub fn all() -> &'static [Category] {
    &[
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
    ]
  }
}

/// Active assets tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tab {
  Abyssals,
  Inventory,
  Stockpiles,
  Tracker,
  Values,
}

/// Sort column for the inventory table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SortCol {
  #[default]
  Name,
  Category,
  Qty,
  UnitValue,
  TotalValue,
  Volume,
  Location,
  Owner,
}

/// Range selector for the NAV tracker chart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrackerRange {
  D7,
  D30,
  D90,
  Ytd,
}

impl TrackerRange {
  pub fn label(&self) -> &'static str {
    match self {
      Self::D7 => "7d",
      Self::D30 => "30d",
      Self::D90 => "90d",
      Self::Ytd => "YTD",
    }
  }

  pub fn days(&self) -> usize {
    match self {
      Self::D7 => 7,
      Self::D30 => 30,
      Self::D90 => 90,
      Self::Ytd => 90,
    }
  }

  pub fn all() -> &'static [TrackerRange] {
    &[
      TrackerRange::D7,
      TrackerRange::D30,
      TrackerRange::D90,
      TrackerRange::Ytd,
    ]
  }
}

/// Per-item fill status enriched with percentage.
#[derive(Clone, Debug)]
pub struct StockpileItemStatus {
  /// EVE type ID.
  pub type_id: i32,
  /// Desired quantity.
  pub target_quantity: i32,
  /// Sum of matching character_assets quantities.
  pub have_quantity: i64,
  /// Human-readable item name.
  pub type_name: String,
  /// Fraction fulfilled (clamped 0.0–1.0).
  pub pct: f32,
}

/// A stockpile combined with fill status for each item.
#[derive(Clone, Debug)]
pub struct StockpileWithStatus {
  /// Optional character scope.
  pub character_id: Option<i64>,
  /// Database ID.
  pub id: i64,
  /// Per-item fill status.
  pub items: Vec<StockpileItemStatus>,
  /// Optional location scope.
  pub location_id: Option<i64>,
  /// Resolved display name for the location, or None if the ID is unscoped.
  pub location_name: Option<String>,
  /// Display name.
  pub name: String,
  /// Overall fill fraction (0.0–1.0).
  pub overall_pct: f32,
  /// True when every item meets its target.
  pub ready: bool,
}

/// An item row being edited in the stockpile form.
#[derive(Clone, Debug, Default)]
pub struct StockpileFormItem {
  /// Raw text from the type_id field.
  pub type_id_text: String,
  /// Raw text from the target_quantity field.
  pub qty_text: String,
}

/// The stockpile form / modal state.
#[derive(Clone, Debug, Default)]
pub struct StockpileForm {
  /// Stockpile ID being edited; `None` when creating a new one.
  pub editing_id: Option<i64>,
  /// Stockpile name field value.
  pub name: String,
  /// Location ID field value (raw text).
  pub location_id_text: String,
  /// Item rows in the form.
  pub items: Vec<StockpileFormItem>,
  /// Non-empty when a validation error should be shown.
  pub error: String,
}

/// One cell in the character × location value matrix.
#[derive(Clone, Debug)]
pub struct CharacterStructureCell {
  pub character_id: i64,
  pub character_name: String,
  pub structure_id: String,
  pub structure_name: String,
  pub value: f64,
}

/// A category's share of total asset value.
#[derive(Clone, Debug)]
pub struct CategoryValue {
  pub category_name: String,
  pub value: f64,
  pub pct: f64,
}

/// One row in the top-items-by-value list.
#[derive(Clone, Debug)]
pub struct TopItem {
  pub type_id: i32,
  pub type_name: String,
  pub category_name: String,
  pub total_quantity: i64,
  pub value: f64,
}

/// Computed asset value breakdown for the Values tab.
#[derive(Clone, Debug)]
pub struct AssetValuesData {
  pub character_structure_cells: Vec<CharacterStructureCell>,
  pub category_breakdown: Vec<CategoryValue>,
  pub top_items: Vec<TopItem>,
  pub total_value: f64,
}

/// Messages produced by the assets controller.
#[derive(Clone, Debug)]
pub enum Message {
  AbyssalsLoaded(pod_model::AbyssalsData),
  AbyssalsTab(abyssals_tab::Message),
  AssetsLoaded(Result<Vec<AssetRecord>, String>),
  InventoryTab(inventory_tab::Message),
  ItemIconsLoaded(Vec<(i32, String, Vec<u8>)>),
  LoadMoreAssets,
  LocationSelected(Option<String>),
  NavHistoryLoaded(Vec<(NaiveDate, f64)>),
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart,
  Picker(character_picker::Message),
  ReauthorizeCharacter(i64),
  RefreshNavHistory,
  StockpilesLoaded(Vec<StockpileWithStatus>),
  StockpilesTab(stockpiles_tab::Message),
  TabSelected(Tab),
  ToggleSidebarGroup(String),
  TrackerTab(tracker_tab::Message),
  ValuesLoaded(AssetValuesData),
  ValuesTab(values_tab::Message),
}

/// Runtime state for the assets controller.
pub struct State {
  pub abyssals: abyssals_tab::AbyssalsState,
  pub active_tab: Tab,
  pub asset_values_data: Option<AssetValuesData>,
  pub assets: Vec<AssetRecord>,
  pub category: Category,
  pub characters: Vec<Character>,
  pub collapsed_sidebar_groups: HashSet<String>,
  pub corporations: Vec<Corporation>,
  pub dragging_pane: bool,
  pub expanded_containers: HashSet<i64>,
  pub help_pop_over: inventory_tab::help_pop_over::State,
  pub item_icons: HashMap<(i32, String), image::Handle>,
  pub last_drag_x: f32,
  pub loading: bool,
  pub nav_history: Vec<(NaiveDate, f64)>,
  pub nav_series: Vec<f64>,
  pub picker: CharacterPicker,
  pub search_query: String,
  pub selected_loc: Option<String>,
  pub sidebar_width: f32,
  pub sort_asc: bool,
  pub sort_col: SortCol,
  pub stockpile_form: Option<StockpileForm>,
  pub stockpiles: Vec<StockpileWithStatus>,
  pub tracker_range: TrackerRange,
  pub values_loading: bool,
  pub visible_count: usize,
}

impl State {
  pub fn all_assets(&self) -> &[AssetRecord] {
    &self.assets
  }

  pub fn selected_character(&self) -> Option<i64> {
    self.picker.selected_character_id()
  }

  pub fn selected_corporation(&self) -> Option<i64> {
    self.picker.selected_corporation_id()
  }

  pub fn total_value(&self) -> f64 {
    self.visible_assets().map(asset_value).sum()
  }

  pub fn total_volume(&self) -> f64 {
    self.visible_assets().map(asset_volume).sum()
  }

  pub fn total_count(&self) -> u64 {
    self.visible_assets().map(|a| a.quantity as u64).sum()
  }

  pub fn total_locations(&self) -> usize {
    self
      .visible_assets()
      .map(|a| a.location_id)
      .collect::<HashSet<_>>()
      .len()
  }

  pub fn visible_assets(&self) -> impl Iterator<Item = &AssetRecord> {
    let corp_id = self.selected_corporation();
    let char_id = self.selected_character();
    let cat_key = self.category.key();
    let query = AssetFilterQuery::parse(&self.search_query).with_me(char_id);
    let loc = self.selected_loc.clone();

    self.assets.iter().filter(move |a| {
      let owner_id = corp_id.or(char_id);
      asset_filter_predicate(a, owner_id, cat_key, loc.as_deref(), &query)
    })
  }

  pub fn sorted_assets(&self) -> Vec<&AssetRecord> {
    let mut v: Vec<&AssetRecord> = self.visible_assets().collect();
    let col = self.sort_col.clone();
    let asc = self.sort_asc;
    v.sort_by(|a, b| {
      let cmp = sort_cmp(&col, a, b);
      if asc { cmp } else { cmp.reverse() }
    });
    v
  }

  pub fn visible_nav_history(&self) -> &[(NaiveDate, f64)] {
    let days = self.tracker_range.days();
    let total = self.nav_history.len();
    if total <= days {
      &self.nav_history
    } else {
      &self.nav_history[total - days..]
    }
  }

  pub fn visible_nav_series(&self) -> Vec<f64> {
    self.visible_nav_history().iter().map(|(_, v)| *v).collect()
  }
}

/// View title.
pub fn title() -> &'static str {
  "Assets"
}

/// Creates a new assets state.
pub fn new(
  characters: Vec<Character>,
  corporations: Vec<Corporation>,
  sidebar_width: f32,
  abyssals_filter_pane_width: f32,
) -> State {
  let picker_entries = build_picker_entries(&characters);
  let corp_entries = build_corp_picker_entries(&corporations);
  let picker = CharacterPicker::new()
    .all_label("All Characters")
    .entries(picker_entries)
    .corp_entries(corp_entries)
    .show_all(true);
  let abyssals = abyssals_tab::AbyssalsState {
    filter_pane_width: abyssals_filter_pane_width,
    ..Default::default()
  };
  State {
    abyssals,
    active_tab: Tab::Inventory,
    asset_values_data: None,
    assets: Vec::new(),
    category: Category::All,
    characters,
    collapsed_sidebar_groups: HashSet::new(),
    corporations,
    dragging_pane: false,
    expanded_containers: HashSet::new(),
    help_pop_over: inventory_tab::help_pop_over::State::default(),
    item_icons: HashMap::new(),
    last_drag_x: 0.0,
    loading: true,
    nav_history: Vec::new(),
    nav_series: Vec::new(),
    picker,
    search_query: String::new(),
    selected_loc: None,
    sidebar_width,
    sort_asc: true,
    sort_col: SortCol::Name,
    stockpile_form: None,
    stockpiles: Vec::new(),
    tracker_range: TrackerRange::D90,
    values_loading: false,
    visible_count: 100,
  }
}

fn build_picker_entries(characters: &[Character]) -> Vec<CharacterEntry> {
  let mut entries = vec![CharacterEntry {
    id: None,
    name: "All Assets".to_string(),
    corp_name: format!("{} characters", characters.len()),
    tone: 200,
    portrait_handle: None,
  }];
  for c in characters {
    let portrait_handle = c.portrait_data().as_ref().map(|b| image::Handle::from_bytes(b.clone()));
    entries.push(CharacterEntry {
      id: Some(*c.id()),
      name: c.name().clone(),
      corp_name: c.corp_name().clone(),
      tone: *c.portrait_tone() as u16,
      portrait_handle,
    });
  }
  entries
}

fn build_corp_picker_entries(corporations: &[Corporation]) -> Vec<CorporationEntry> {
  corporations
    .iter()
    .map(|c| {
      let icon_handle = c.icon_data().as_ref().map(|b| image::Handle::from_bytes(b.clone()));
      CorporationEntry {
        icon_handle,
        id: *c.id(),
        name: c.name().clone(),
        ticker: c.ticker().clone(),
      }
    })
    .collect()
}

fn asset_owner_matches(a: &AssetRecord, owner_id: Option<i64>) -> bool {
  owner_id.is_none_or(|id| a.character_id == id)
}

fn asset_category_matches(a: &AssetRecord, cat_key: &str) -> bool {
  cat_key == "all" || a.category_key == cat_key
}

fn asset_loc_matches(a: &AssetRecord, loc: Option<&str>) -> bool {
  match loc {
    None => true,
    Some(filter) => asset_matches_loc_filter(a, filter),
  }
}

fn asset_filter_predicate(
  a: &AssetRecord,
  owner_id: Option<i64>,
  cat_key: &str,
  loc: Option<&str>,
  query: &AssetFilterQuery,
) -> bool {
  if !asset_owner_matches(a, owner_id) {
    return false;
  }
  if !asset_category_matches(a, cat_key) {
    return false;
  }
  if !asset_loc_matches(a, loc) {
    return false;
  }
  query.matches(a)
}

fn asset_matches_loc_filter(a: &AssetRecord, filter: &str) -> bool {
  if let Some(result) = asset_matches_loc_primary(a, filter) {
    return result;
  }
  asset_matches_loc_secondary(a, filter)
}

fn asset_matches_loc_primary(a: &AssetRecord, filter: &str) -> Option<bool> {
  if let Some(sys) = filter.strip_prefix("system:") {
    Some(a.system_name == sys)
  } else if let Some(loc_name) = filter.strip_prefix("location:") {
    Some(a.location_name == loc_name)
  } else if let Some(cid_str) = filter.strip_prefix("container:") {
    let cid = cid_str.parse::<i64>().ok();
    Some(cid.is_none_or(|id| a.container_id == id))
  } else {
    None
  }
}

fn asset_matches_loc_secondary(a: &AssetRecord, filter: &str) -> bool {
  if let Some(region) = filter.strip_prefix("region:") {
    a.region_name == region
  } else if let Some(constellation) = filter.strip_prefix("constellation:") {
    a.constellation_name == constellation
  } else {
    true
  }
}

fn sort_cmp(col: &SortCol, a: &AssetRecord, b: &AssetRecord) -> std::cmp::Ordering {
  match col {
    SortCol::Name | SortCol::Category | SortCol::Qty | SortCol::Owner => sort_cmp_identity(col, a, b),
    _ => sort_cmp_numeric(col, a, b),
  }
}

fn sort_cmp_identity(col: &SortCol, a: &AssetRecord, b: &AssetRecord) -> std::cmp::Ordering {
  match col {
    SortCol::Name => a.type_name.cmp(&b.type_name),
    SortCol::Category => a.category_key.cmp(&b.category_key),
    SortCol::Qty => a.quantity.cmp(&b.quantity),
    _ => a.character_id.cmp(&b.character_id),
  }
}

fn sort_cmp_numeric(col: &SortCol, a: &AssetRecord, b: &AssetRecord) -> std::cmp::Ordering {
  match col {
    SortCol::UnitValue => std::cmp::Ordering::Equal,
    SortCol::TotalValue => asset_value(a)
      .partial_cmp(&asset_value(b))
      .unwrap_or(std::cmp::Ordering::Equal),
    SortCol::Volume => asset_volume(a)
      .partial_cmp(&asset_volume(b))
      .unwrap_or(std::cmp::Ordering::Equal),
    _ => sort_cmp_location(a, b),
  }
}

fn sort_cmp_location(a: &AssetRecord, b: &AssetRecord) -> std::cmp::Ordering {
  let a_loc = if a.container_path.is_empty() {
    &a.location_name
  } else {
    &a.container_path
  };
  let b_loc = if b.container_path.is_empty() {
    &b.location_name
  } else {
    &b.container_path
  };
  a_loc.cmp(b_loc)
}

fn update_category_changed(state: &mut State, cat: Category) {
  state.category = cat;
  state.visible_count = 100;
}

fn update_help_pop_over_inner(state: &mut State, inner: inventory_tab::help_pop_over::Message) {
  if let inventory_tab::help_pop_over::Message::QueryInserted(ref q) = inner {
    let sep = if state.search_query.is_empty() { "" } else { " " };
    state.search_query = format!("{}{sep}{q}", state.search_query);
    state.visible_count = 100;
  }
  let _ = state.help_pop_over.update(inner);
}

fn update_help_toggle(state: &mut State) {
  let inner = if state.help_pop_over.visible {
    inventory_tab::help_pop_over::Message::Close
  } else {
    inventory_tab::help_pop_over::Message::Open
  };
  let _ = state.help_pop_over.update(inner);
}

fn update_scroll_update(state: &mut State, y: f32) {
  if y > 0.85 {
    let total = state.visible_assets().count();
    if state.visible_count < total {
      state.visible_count += 50;
    }
  }
}

fn update_search_changed(state: &mut State, q: String) {
  state.search_query = q;
  state.visible_count = 100;
}

fn update_sort_changed(state: &mut State, col: SortCol) {
  if state.sort_col == col {
    state.sort_asc = !state.sort_asc;
  } else {
    state.sort_col = col;
    state.sort_asc = matches!(state.sort_col, SortCol::Name | SortCol::Category | SortCol::Location);
  }
}

fn update_inventory_tab(state: &mut State, msg: inventory_tab::Message) {
  match msg {
    inventory_tab::Message::CategoryChanged(cat) => update_category_changed(state, cat),
    inventory_tab::Message::HelpPopOver(inner) => update_help_pop_over_inner(state, inner),
    inventory_tab::Message::HelpToggle => update_help_toggle(state),
    msg => update_inventory_tab_secondary(state, msg),
  }
}

fn update_inventory_tab_secondary(state: &mut State, msg: inventory_tab::Message) {
  match msg {
    inventory_tab::Message::ScrollUpdate(y) => update_scroll_update(state, y),
    inventory_tab::Message::SearchChanged(q) => update_search_changed(state, q),
    msg => update_inventory_tab_tertiary(state, msg),
  }
}

fn update_inventory_tab_tertiary(state: &mut State, msg: inventory_tab::Message) {
  match msg {
    inventory_tab::Message::SortChanged(col) => update_sort_changed(state, col),
    inventory_tab::Message::ToggleContainer(id) => update_toggle_container(state, id),
    _ => {}
  }
}

fn update_toggle_container(state: &mut State, id: i64) {
  if !state.expanded_containers.remove(&id) {
    state.expanded_containers.insert(id);
  }
}

fn form_set_name(state: &mut State, name: String) {
  if let Some(form) = state.stockpile_form.as_mut() {
    form.name = name;
  }
}

fn form_set_location(state: &mut State, loc: String) {
  if let Some(form) = state.stockpile_form.as_mut() {
    form.location_id_text = loc;
  }
}

fn form_set_item_type(state: &mut State, idx: usize, val: String) {
  if let Some(form) = state.stockpile_form.as_mut()
    && let Some(item) = form.items.get_mut(idx)
  {
    item.type_id_text = val;
  }
}

fn form_set_item_qty(state: &mut State, idx: usize, val: String) {
  if let Some(form) = state.stockpile_form.as_mut()
    && let Some(item) = form.items.get_mut(idx)
  {
    item.qty_text = val;
  }
}

fn form_add_item(state: &mut State) {
  if let Some(form) = state.stockpile_form.as_mut() {
    form.items.push(StockpileFormItem::default());
  }
}

fn form_remove_item(state: &mut State, idx: usize) {
  if let Some(form) = state.stockpile_form.as_mut()
    && idx < form.items.len()
  {
    form.items.remove(idx);
  }
}

fn update_stockpile_form(state: &mut State, msg: stockpiles_tab::Message) {
  match msg {
    stockpiles_tab::Message::FormNameChanged(name) => form_set_name(state, name),
    stockpiles_tab::Message::FormLocationChanged(loc) => form_set_location(state, loc),
    stockpiles_tab::Message::FormCancel => state.stockpile_form = None,
    msg => update_stockpile_form_items(state, msg),
  }
}

fn update_stockpile_form_items(state: &mut State, msg: stockpiles_tab::Message) {
  match msg {
    stockpiles_tab::Message::FormItemTypeChanged(idx, val) => form_set_item_type(state, idx, val),
    stockpiles_tab::Message::FormItemQtyChanged(idx, val) => form_set_item_qty(state, idx, val),
    msg => update_stockpile_form_items_secondary(state, msg),
  }
}

fn update_stockpile_form_items_secondary(state: &mut State, msg: stockpiles_tab::Message) {
  match msg {
    stockpiles_tab::Message::FormAddItem => form_add_item(state),
    stockpiles_tab::Message::FormRemoveItem(idx) => form_remove_item(state, idx),
    _ => {}
  }
}

fn update_stockpiles_tab(state: &mut State, msg: stockpiles_tab::Message) {
  match msg {
    stockpiles_tab::Message::NewStockpile => {
      state.stockpile_form = Some(StockpileForm {
        items: vec![StockpileFormItem::default()],
        ..StockpileForm::default()
      });
    }
    stockpiles_tab::Message::EditStockpile(id) => update_edit_stockpile(state, id),
    msg => update_stockpiles_tab_secondary(state, msg),
  }
}

fn update_stockpiles_tab_secondary(state: &mut State, msg: stockpiles_tab::Message) {
  match msg {
    stockpiles_tab::Message::DeleteStockpile(_id) => {}
    stockpiles_tab::Message::ConfirmDelete(_) => {}
    stockpiles_tab::Message::FormSave => {}
    msg => update_stockpile_form(state, msg),
  }
}

fn update_edit_stockpile(state: &mut State, id: i64) {
  if let Some(pile) = state.stockpiles.iter().find(|p| p.id == id) {
    let items = pile
      .items
      .iter()
      .map(|it| StockpileFormItem {
        type_id_text: it.type_id.to_string(),
        qty_text: it.target_quantity.to_string(),
      })
      .collect();
    state.stockpile_form = Some(StockpileForm {
      editing_id: Some(id),
      name: pile.name.clone(),
      location_id_text: pile.location_id.map(|l| l.to_string()).unwrap_or_default(),
      items,
      error: String::new(),
    });
  }
}

fn apply_assets_loaded(state: &mut State, assets: Vec<AssetRecord>) {
  let known_keys = collect_known_loc_keys(&state.assets);
  for a in &assets {
    maybe_collapse_region(state, a, &known_keys);
    maybe_collapse_constellation(state, a, &known_keys);
  }
  state.assets = assets;
  state.loading = false;
}

fn build_abyssal_portrait_handles(characters: &[Character]) -> HashMap<i64, image::Handle> {
  characters
    .iter()
    .filter_map(|c| {
      c.portrait_data()
        .as_ref()
        .map(|bytes| (*c.id(), image::Handle::from_bytes(bytes.clone())))
    })
    .collect()
}

fn apply_data_loaded(state: &mut State, message: Message) {
  match message {
    Message::AbyssalsLoaded(data) => {
      state.abyssals.abyssals = data.items;
      state.abyssals.categories = data.categories;
      state.abyssals.type_icons = data
        .type_icons
        .into_iter()
        .map(|(id, bytes)| (id, image::Handle::from_bytes(bytes)))
        .collect();
      state.abyssals.portrait_handles = build_abyssal_portrait_handles(&state.characters);
    }
    Message::AssetsLoaded(Ok(assets)) => apply_assets_loaded(state, assets),
    Message::AssetsLoaded(Err(e)) => apply_assets_load_error(state, e),
    msg => apply_data_loaded_secondary(state, msg),
  }
}

fn apply_assets_load_error(state: &mut State, e: String) {
  eprintln!("assets: failed to load: {e}");
  state.loading = false;
}

fn apply_data_loaded_secondary(state: &mut State, message: Message) {
  match message {
    Message::ItemIconsLoaded(icons) => load_item_icons(state, icons),
    Message::NavHistoryLoaded(history) => update_nav_history(state, history),
    msg => apply_data_loaded_tertiary(state, msg),
  }
}

fn apply_data_loaded_tertiary(state: &mut State, message: Message) {
  match message {
    Message::StockpilesLoaded(piles) => state.stockpiles = piles,
    Message::ValuesLoaded(data) => apply_values_loaded(state, data),
    _ => {}
  }
}

fn apply_values_loaded(state: &mut State, data: AssetValuesData) {
  state.asset_values_data = Some(data);
  state.values_loading = false;
}

fn collect_known_loc_keys(assets: &[AssetRecord]) -> HashSet<String> {
  assets
    .iter()
    .flat_map(|a| {
      [
        if a.region_name.is_empty() {
          None
        } else {
          Some(format!("region:{}", a.region_name))
        },
        if a.constellation_name.is_empty() {
          None
        } else {
          Some(format!("constellation:{}", a.constellation_name))
        },
      ]
      .into_iter()
      .flatten()
    })
    .collect()
}

fn load_item_icons(state: &mut State, icons: Vec<(i32, String, Vec<u8>)>) {
  for (type_id, variant, bytes) in icons {
    state
      .item_icons
      .insert((type_id, variant), image::Handle::from_bytes(bytes));
  }
}

fn maybe_collapse_constellation(state: &mut State, a: &AssetRecord, known_keys: &HashSet<String>) {
  if !a.constellation_name.is_empty() {
    let key = format!("constellation:{}", a.constellation_name);
    if !known_keys.contains(&key) {
      state.collapsed_sidebar_groups.insert(key);
    }
  }
}

fn maybe_collapse_region(state: &mut State, a: &AssetRecord, known_keys: &HashSet<String>) {
  if !a.region_name.is_empty() {
    let key = format!("region:{}", a.region_name);
    if !known_keys.contains(&key) {
      state.collapsed_sidebar_groups.insert(key);
    }
  }
}

fn update_abyssals_tab(state: &mut State, msg: abyssals_tab::Message) {
  match msg {
    abyssals_tab::Message::CloseTypeModal => {
      state.abyssals.modal_open = false;
    }
    abyssals_tab::Message::FilterReset => {
      state.abyssals.modal_open = false;
      state.abyssals.selected_source_type_id = None;
      state.abyssals.stat_range_filters.clear();
      state.abyssals.visible_count = 50;
    }
    abyssals_tab::Message::OpenTypeModal => {
      state.abyssals.modal_open = true;
    }
    abyssals_tab::Message::PaneDragStart => {
      state.abyssals.filter_pane_dragging = true;
      state.abyssals.filter_pane_last_drag_x = 0.0;
    }
    abyssals_tab::Message::PaneDrag(x) => {
      if state.abyssals.filter_pane_last_drag_x > 0.0 {
        let delta = x - state.abyssals.filter_pane_last_drag_x;
        state.abyssals.filter_pane_width = (state.abyssals.filter_pane_width + delta).max(160.0);
      }
      state.abyssals.filter_pane_last_drag_x = x;
    }
    abyssals_tab::Message::PaneDragEnd => {
      state.abyssals.filter_pane_dragging = false;
      state.abyssals.filter_pane_last_drag_x = 0.0;
    }
    abyssals_tab::Message::ScrollUpdate(y) => {
      let selected = state.abyssals.selected_source_type_id;
      let filters = &state.abyssals.stat_range_filters;
      let total = state
        .abyssals
        .abyssals
        .iter()
        .filter(|item| {
          if selected.is_some_and(|id| item.type_id != id) {
            return false;
          }
          for (attr_id, (min_val, max_val)) in filters {
            if let Some(stat) = item.stats.iter().find(|s| s.attribute_id == *attr_id)
              && (stat.rolled_value < *min_val || stat.rolled_value > *max_val)
            {
              return false;
            }
          }
          true
        })
        .count();
      if y > 0.85 && state.abyssals.visible_count < total {
        state.abyssals.visible_count += 25;
      }
    }
    abyssals_tab::Message::StatMaxFilterChanged(attr_id, val) => {
      let entry = state.abyssals.stat_range_filters.entry(attr_id).or_insert((val, val));
      entry.1 = val;
      state.abyssals.visible_count = 50;
    }
    abyssals_tab::Message::StatMinFilterChanged(attr_id, val) => {
      let entry = state.abyssals.stat_range_filters.entry(attr_id).or_insert((val, val));
      entry.0 = val;
      state.abyssals.visible_count = 50;
    }
    abyssals_tab::Message::TypeSelected(id) => {
      state.abyssals.modal_open = false;
      state.abyssals.selected_source_type_id = id;
      state.abyssals.stat_range_filters.clear();
      state.abyssals.slider_editing = None;
      state.abyssals.slider_edit_text.clear();
      state.abyssals.visible_count = 50;
    }
    abyssals_tab::Message::SliderEditStart(attr_id, endpoint, current_value) => {
      let unit = state
        .abyssals
        .categories
        .iter()
        .flat_map(|c| c.source_types.iter())
        .find(|t| state.abyssals.selected_source_type_id == Some(t.type_id))
        .and_then(|t| t.stat_templates.iter().find(|s| s.attribute_id == attr_id))
        .map(|s| s.unit_suffix.clone())
        .unwrap_or_default();
      state.abyssals.slider_editing = Some((attr_id, endpoint));
      state.abyssals.slider_edit_text = format_stat_value_for_edit(current_value, &unit);
    }
    abyssals_tab::Message::SliderEditInput(text) => {
      state.abyssals.slider_edit_text = text;
    }
    abyssals_tab::Message::SliderEditCommit(attr_id, endpoint) => {
      commit_slider_edit(state, attr_id, endpoint);
    }
  }
}

fn format_stat_value_for_edit(value: f64, unit: &str) -> String {
  let formatted = format!("{:.2}", value);
  let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
  let _ = unit;
  trimmed.to_string()
}

fn commit_slider_edit(state: &mut State, attr_id: i32, endpoint: abyssals_tab::SliderEndpoint) {
  let text = state.abyssals.slider_edit_text.trim().to_string();
  let Some(src_id) = state.abyssals.selected_source_type_id else {
    state.abyssals.slider_editing = None;
    return;
  };
  let Some(stat) = state
    .abyssals
    .categories
    .iter()
    .flat_map(|c| c.source_types.iter())
    .find(|t| t.type_id == src_id)
    .and_then(|t| t.stat_templates.iter().find(|s| s.attribute_id == attr_id))
    .cloned()
  else {
    state.abyssals.slider_editing = None;
    return;
  };
  let lo = stat.base_value * stat.min_mult.min(stat.max_mult);
  let hi = stat.base_value * stat.min_mult.max(stat.max_mult);
  let current = state
    .abyssals
    .stat_range_filters
    .get(&attr_id)
    .copied()
    .unwrap_or((lo, hi));
  if let Ok(parsed) = text.parse::<f64>() {
    let clamped = parsed.clamp(lo, hi);
    match endpoint {
      abyssals_tab::SliderEndpoint::Min => {
        let new_min = clamped.min(current.1);
        state.abyssals.stat_range_filters.insert(attr_id, (new_min, current.1));
      }
      abyssals_tab::SliderEndpoint::Max => {
        let new_max = clamped.max(current.0);
        state.abyssals.stat_range_filters.insert(attr_id, (current.0, new_max));
      }
    }
  }
  state.abyssals.slider_editing = None;
  state.abyssals.slider_edit_text.clear();
}

fn update_assets_state_messages(state: &mut State, message: Message) {
  match message {
    Message::LoadMoreAssets => update_load_more_assets(state),
    Message::LocationSelected(loc) => update_location_selected(state, loc),
    msg => update_assets_state_messages_secondary(state, msg),
  }
}

fn update_assets_state_messages_secondary(state: &mut State, message: Message) {
  match message {
    Message::TabSelected(tab) => update_tab_selected(state, tab),
    Message::ToggleSidebarGroup(key) => update_toggle_sidebar_group(state, key),
    msg => apply_data_loaded(state, msg),
  }
}

fn update_assets_secondary(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::AbyssalsTab(msg) => update_abyssals_tab(state, msg),
    Message::StockpilesTab(msg) => update_stockpiles_tab(state, msg),
    Message::TrackerTab(msg) => update_tracker_tab(state, msg),
    msg => update_assets_state_messages(state, msg),
  }
  iced::Task::none()
}

fn update_load_more_assets(state: &mut State) {
  state.visible_count += 50;
}

fn update_location_selected(state: &mut State, loc: Option<String>) {
  state.selected_loc = loc;
}

fn update_nav_history(state: &mut State, history: Vec<(chrono::NaiveDate, f64)>) {
  state.nav_series = history.iter().map(|(_, v)| *v).collect();
  state.nav_history = history;
}

fn update_pane_drag(state: &mut State, x: f32) {
  if state.last_drag_x > 0.0 {
    let delta = x - state.last_drag_x;
    state.sidebar_width = (state.sidebar_width + delta).max(160.0);
  }
  state.last_drag_x = x;
}

fn update_pane_drag_end(state: &mut State) {
  state.dragging_pane = false;
  state.last_drag_x = 0.0;
}

fn update_pane_drag_start(state: &mut State) {
  state.dragging_pane = true;
  state.last_drag_x = 0.0;
}

fn update_picker(state: &mut State, msg: character_picker::Message) -> iced::Task<Message> {
  if let character_picker::Message::Select(_) = &msg {
    state.visible_count = 100;
    state.selected_loc = None;
  }
  state.picker.update(msg);
  iced::Task::none()
}

fn update_tab_selected(state: &mut State, tab: Tab) {
  state.active_tab = tab;
}

fn update_toggle_sidebar_group(state: &mut State, key: String) {
  if !state.collapsed_sidebar_groups.remove(&key) {
    state.collapsed_sidebar_groups.insert(key);
  }
}

fn update_tracker_tab(state: &mut State, msg: tracker_tab::Message) {
  match msg {
    tracker_tab::Message::TrackerRangeChanged(r) => {
      state.tracker_range = r;
    }
  }
}

/// Processes an assets message and returns a task.
pub fn update(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::InventoryTab(msg) => {
      update_inventory_tab(state, msg);
      iced::Task::none()
    }
    Message::Picker(msg) => update_picker(state, msg),
    msg => update_pane_or_secondary(state, msg),
  }
}

fn update_pane_or_secondary(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::PaneDrag(x) => {
      update_pane_drag(state, x);
      iced::Task::none()
    }
    Message::PaneDragEnd => {
      update_pane_drag_end(state);
      iced::Task::none()
    }
    Message::PaneDragStart => {
      update_pane_drag_start(state);
      iced::Task::none()
    }
    msg => update_assets_secondary(state, msg),
  }
}

pub fn asset_value(a: &AssetRecord) -> f64 {
  a.unit_price * a.quantity as f64
}

fn nav_refresh_subscription() -> Subscription<Message> {
  iced::time::every(std::time::Duration::from_secs(1800)).map(|_| Message::RefreshNavHistory)
}

fn drag_event_to_message(event: Event) -> Option<Message> {
  match event {
    Event::Mouse(mouse::Event::CursorMoved {
      position,
    }) => Some(Message::PaneDrag(position.x)),
    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::PaneDragEnd),
    _ => None,
  }
}

fn abyssals_drag_event_to_message(event: Event) -> Option<Message> {
  match event {
    Event::Mouse(mouse::Event::CursorMoved {
      position,
    }) => Some(Message::AbyssalsTab(abyssals_tab::Message::PaneDrag(position.x))),
    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
      Some(Message::AbyssalsTab(abyssals_tab::Message::PaneDragEnd))
    }
    _ => None,
  }
}

fn modal_key_event_to_message(event: Event) -> Option<Message> {
  match event {
    Event::Keyboard(keyboard::Event::KeyPressed {
      key: keyboard::Key::Named(keyboard::key::Named::Escape),
      ..
    }) => Some(Message::AbyssalsTab(abyssals_tab::Message::CloseTypeModal)),
    _ => None,
  }
}

/// Returns subscriptions for nav history refresh and pane drag tracking.
pub fn subscription(state: &State) -> Subscription<Message> {
  let nav_refresh = nav_refresh_subscription();
  let mut subs: Vec<Subscription<Message>> = vec![nav_refresh];
  if state.dragging_pane {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      drag_event_to_message(event)
    }));
  }
  if state.abyssals.filter_pane_dragging {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      abyssals_drag_event_to_message(event)
    }));
  }
  if state.abyssals.modal_open {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      modal_key_event_to_message(event)
    }));
  }
  Subscription::batch(subs)
}

pub fn asset_volume(a: &AssetRecord) -> f64 {
  a.volume * a.quantity as f64
}

pub fn fmt_qty(n: u64) -> String {
  if n >= 1_000_000 {
    format!("{:.1}M", n as f64 / 1_000_000.0)
  } else if n >= 1_000 {
    format!("{:.1}K", n as f64 / 1_000.0)
  } else {
    n.to_string()
  }
}

pub fn fmt_vol(m3: f64) -> String {
  if m3 >= 1_000_000.0 {
    format!("{:.2} Mm³", m3 / 1_000_000.0)
  } else if m3 >= 10_000.0 {
    format!("{:.1} km³", m3 / 1_000.0)
  } else if m3 >= 1.0 {
    format!("{:.1} m³", m3)
  } else {
    format!("{:.2} m³", m3)
  }
}

const CAT_COLORS: &[(&str, (f32, f32, f32))] = &[
  ("ship", (0.247, 0.722, 0.859)),
  ("module", (0.851, 0.698, 0.322)),
  ("drone", (0.357, 0.725, 0.494)),
  ("charge", (0.878, 0.459, 0.349)),
  ("implant", (0.612, 0.408, 0.839)),
  ("blueprint", (0.247, 0.722, 0.780)),
  ("material", (0.600, 0.510, 0.360)),
  ("book", (0.420, 0.420, 0.780)),
  ("commodity", (0.247, 0.780, 0.800)),
];

const CAT_GLYPHS: &[(&str, &str)] = &[
  ("ship", "◆"),
  ("module", "▣"),
  ("drone", "◇"),
  ("charge", "▴"),
  ("implant", "✦"),
  ("blueprint", "▤"),
  ("material", "◯"),
  ("book", "◐"),
  ("commodity", "⬟"),
];

pub fn cat_color_rgb(cat: &str) -> (f32, f32, f32) {
  CAT_COLORS
    .iter()
    .find(|&&(k, _)| k == cat)
    .map(|&(_, v)| v)
    .unwrap_or((0.600, 0.600, 0.600))
}

pub fn cat_glyph(cat: &str) -> &'static str {
  CAT_GLYPHS
    .iter()
    .find(|&&(k, _)| k == cat)
    .map(|&(_, v)| v)
    .unwrap_or("·")
}

const STRUCT_GLYPHS: &[(&str, &str)] = &[("keepstar", "✦"), ("astrahus", "◇"), ("station", "⊟"), ("space", "∞")];

pub fn struct_glyph(kind: &str) -> &'static str {
  STRUCT_GLYPHS
    .iter()
    .find(|&&(k, _)| k == kind)
    .map(|&(_, v)| v)
    .unwrap_or("⊟")
}

fn render_base<'a>(state: &'a State, window_width: f32) -> Element<'a, Message> {
  let header_el = Header::new(state).render();
  let main_el = MainPanel::new(state).window_width(window_width).render();
  container(column([header_el, main_el]))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn render_drag_overlay(state: &State) -> Option<Element<'static, Message>> {
  DragOverlay::new(state.dragging_pane).render()
}

fn render_picker_overlay<'a>(state: &'a State) -> Option<Element<'a, Message>> {
  PickerOverlay::new(&state.picker).render()
}

fn scope_missing_element(char_id: i64) -> Element<'static, Message> {
  ScopeMissing::new(char_id, "asset tracking")
    .render()
    .map(scope_missing_to_message)
}

fn scope_missing_to_message(m: scope_missing::Message) -> Message {
  match m {
    scope_missing::Message::ReauthorizePressed(id) => Message::ReauthorizeCharacter(id),
  }
}

fn character_missing_asset_scope(state: &State, char_id: i64) -> bool {
  let Some(character) = state.characters.iter().find(|c| *c.id() == char_id) else {
    return false;
  };
  let granted = character.granted_scopes_list();
  !missing_scopes(&granted, &["esi-assets.read_assets.v1"]).is_empty()
}

fn render_scope_missing<'a>(state: &'a State) -> Option<Element<'a, Message>> {
  let char_id = state.selected_character()?;
  if character_missing_asset_scope(state, char_id) {
    Some(scope_missing_element(char_id))
  } else {
    None
  }
}

/// Builder for the assets view.
pub struct Component<'a> {
  state: &'a State,
  window_width: f32,
}

impl<'a> Component<'a> {
  /// Creates a new view builder for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
      window_width: 1200.0,
    }
  }

  /// Sets the available window width used by responsive sub-panels.
  pub fn window_width(mut self, width: f32) -> Self {
    self.window_width = width;
    self
  }

  /// Renders the full assets window into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    if let Some(el) = render_scope_missing(state) {
      return el;
    }
    let base = render_base(state, self.window_width);
    let mut layers: Vec<Element<'_, Message>> = vec![base];
    if let Some(d) = render_drag_overlay(state) {
      layers.push(d);
    }
    if let Some(p) = render_picker_overlay(state) {
      layers.push(p);
    }
    if layers.len() == 1 {
      layers.into_iter().next().unwrap()
    } else {
      stack(layers).into()
    }
  }
}
