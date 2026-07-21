mod book;
mod book_view;
mod browse;
mod cart;
mod compare;
mod history;
mod history_chart;
mod history_view;
mod i18n;
mod my_orders;
mod order_history;
pub mod outbid;
mod shell;
mod tree;
pub mod watch_eval;
mod watchlist;

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use iced::{Element, Point, Task};

use crate::{
  clients::{self, esi, esi::models::market::RegionOrder, eve_image, eve_sso, http},
  features::shell::window_state::UiState,
  services::{
    inventory_lots,
    location_search::{LocationRef, LocationTier},
  },
  store::{
    Database, images,
    model::{CorporationMarketOrder, MarketOrder, MarketWatch, OwnerType, WatchDirection},
    repo::{character as character_repo, finance, market as market_repo, org, sde},
  },
  ui::{
    components::{
      location_combobox::LocationSearch,
      resizable_pane::{self, PaneDrag},
    },
    style::spacing,
  },
};

const THE_FORGE_REGION_ID: i64 = 10_000_002;
const LOCATION_SEARCH_MIN_CHARS: usize = 3;
const MARKET_TREE_PANE_DEFAULT: f32 = 286.0;
const MARKET_TREE_PANE_KEY: &str = "market.tree";
// Telemetry syncs every 300s under LocationTracking; past three missed cycles the online flag is
// treated as unknown rather than authoritative, so the pre-flight guardrail errs toward attempting.
const TELEMETRY_STALE_SECS: i64 = 900;

#[derive(Clone, Debug)]
pub enum Message {
  TabSelected(Tab),
  TreeLoaded(Box<tree::MarketTree>),
  BookLoaded(Box<book::OrderBook>),
  BookRelabeled(Box<book::OrderBook>),
  StructureBookLoaded(StructureBook),
  NodeToggled(i64),
  FilterChanged(String),
  ItemSelected(i64),
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart,
  PaneSettled(&'static str, f32),
  DefaultMarketResolved(LocationRef),
  RegionPickerToggled,
  RegionPickerClosed,
  RegionSearchChanged(String),
  RegionResultsLoaded(u64, Vec<LocationRef>),
  RegionPicked(LocationRef),
  RegionResolved(LocationRef),
  OrdersLoaded(Box<OrdersData>),
  OrdersScopeToggled,
  OrdersScopeDismissed,
  OrdersScopeSelected(OrdersScope),
  OrdersSubTabSelected(OrdersSubTab),
  LotsLoaded(Vec<LotGroupCard>),
  LotDismissPrompted(Box<LotDismissPrompt>),
  LotDismissCancelled,
  LotDismissConfirmed,
  OpenInGame { character_id: i64, type_id: i64 },
  MarketWindowOpened(Result<(), OpenWindowFailure>),
  WatchNew,
  WatchEdit(Box<MarketWatch>),
  WatchModalClosed,
  WatchItemPickerToggled,
  WatchItemSearchChanged(String),
  WatchItemPicked(i64, String),
  WatchDirectionSelected(WatchDirection),
  WatchTargetChanged(String),
  WatchRegionPickerToggled,
  WatchRegionSearchChanged(String),
  WatchRegionResultsLoaded(u64, Vec<LocationRef>),
  WatchRegionPicked(LocationRef),
  WatchSubmitted,
  BrowseWatchSubmitted,
  WatchPricesLoaded(watch_eval::PriceMap),
  WatchesLoaded(Vec<WatchCard>),
  WatchCursorMoved(Point),
  WatchMenuOpened(i64),
  WatchMenuDismissed,
  WatchRemoved(i64),
  WatchDragStarted(i64),
  WatchDropEntered(i64),
  WatchDropExited(i64),
  WatchDropReleased,
  WatchGripEntered(i64),
  WatchGripExited(i64),
  OwnOrdersLoaded(Vec<MarketOrder>),
  AlertOutbidLoaded(i64),
  DetailViewSelected(DetailView),
  HistoryLoaded(i64, i64, Result<Vec<history::HistoryPoint>, String>),
  HistoryRangeSelected(history::Range),
  CompareAddPickerDismissed,
  CompareAddPickerOpened(compare::BlockId),
  CompareAddResultsLoaded(u64, Vec<LocationRef>),
  CompareAddSearchChanged(String),
  CompareBookLoaded(i64, i64, Box<book::OrderBook>),
  CompareCursorMoved(Point),
  CompareDragStarted(i64),
  CompareDropEntered(i64),
  CompareDropExited(i64),
  CompareDropReleased,
  CompareGripEntered(i64),
  CompareGripExited(i64),
  CompareMarketPicked(LocationRef),
  CompareMarketRemoved(compare::BlockId, i64),
  CompareMenuDismissed,
  CompareMenuOpened(compare::BlockId, i64),
  ComparePinRequested,
  ComparePinsLoaded(Vec<compare::CompareBlock>),
  CompareStructureBookLoaded(i64, i64, StructureBook),
  CompareTransientLoaded(Box<compare::CompareBlock>),
  CompareUnpinRequested(i64),
  CompareWatchSubmitted(compare::BlockId),
  CartAddFlashEnded(u64),
  CartAddQtyChanged(i64),
  CartAddSubmitted(i64),
  CartCleared,
  CartClosed,
  CartEscapePressed,
  CartExportFlashEnded(u64),
  CartExported,
  CartLineRemoved(i64),
  CartLoaded(Box<cart::Snapshot>),
  CartMenuAdded(i64),
  CartOpened,
  CartPricesLoaded(i64, crate::services::market_prices::BestSellPrices),
  CartQtyChanged(i64, i64),
  CartSaveCancelled,
  CartSaveCommitted,
  CartSaveNameChanged(String),
  CartSaveStarted,
  CartSavedCartLoaded(i64),
  CartSavedCartMerged(i64),
  CartSavedDeleted(i64),
  CartSavedRenameChanged(String),
  CartSavedRenameCommitted,
  CartSavedRenameStarted(i64),
  CartTabSelected(cart::View),
  TreeCursorMoved(Point),
  TreeMenuDismissed,
  TreeMenuItemOpened(i64),
  TreeMenuNodeOpened(i64),
  FeaturesChanged(crate::config::FeatureFlags),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OrdersScope {
  #[default]
  All,
  Character(i64),
}

impl OrdersScope {
  pub fn character_id(self) -> Option<i64> {
    match self {
      OrdersScope::All => None,
      OrdersScope::Character(id) => Some(id),
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OrdersSubTab {
  #[default]
  Current,
  History,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LotDismissPrompt {
  pub is_corporation: bool,
  pub item_name: String,
  pub owner_id: i64,
  pub transaction_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LotGroupCard {
  pub group: inventory_lots::LotGroup,
  pub owner_name: String,
  pub region_label: String,
  pub system_label: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DetailView {
  #[default]
  Orders,
  History,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum HistoryFetch {
  Empty,
  Failed,
  Loaded(Vec<history::HistoryPoint>),
  #[default]
  Loading,
}

// The order-book access state for the right pane. Only a structure fetch can leave `Ok`: an access
// miss (403/404) is a permanent `NoAccess`, and any other failure is a transient `Error`. A region
// book always resolves to `Ok`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BookAccess {
  #[default]
  Ok,
  NoAccess,
  Error,
}

// The outcome of an authed structure order-book fetch, threaded back through the app layer.
#[derive(Clone, Debug)]
pub enum StructureBook {
  Loaded(Box<book::OrderBook>),
  NoAccess,
  Error,
}

// A failed open-in-game-market-window attempt, threaded back from the authed task with the owner it
// was tried against and a ready-to-render, character-named message.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenWindowFailure {
  pub character_id: i64,
  pub message: String,
}

// The transient inline notice shown in the My Orders view after a failed open-window attempt. `at`
// is an epoch-second stamp so the view can drop it once it goes stale (auto-clear on the next tick).
#[derive(Clone, Debug, PartialEq)]
pub struct OpenWindowNotice {
  pub at: i64,
  pub character_id: i64,
  pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OpenWindowFailureKind {
  Generic,
  Offline,
  Reauthorize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OrderPilot {
  pub id: i64,
  pub name: String,
  pub portrait: Option<std::path::PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrderRow {
  pub character_id: i64,
  pub character_name: String,
  pub owner_is_corp: bool,
  pub owner_offline: bool,
  pub type_id: i64,
  pub region_label: String,
  pub system_label: String,
  pub price: f64,
  pub is_buy: bool,
  pub volume_remain: i64,
  pub volume_total: i64,
  pub expires_days: i64,
  pub done: bool,
  pub outbid: bool,
  pub best: Option<f64>,
  pub gap_pct: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OrdersData {
  pub scope: OrdersScope,
  pub rows: Vec<OrderRow>,
  pub roster: Vec<OrderPilot>,
  pub active_count: usize,
  pub sell_count: usize,
  pub buy_count: usize,
  pub outbid_count: usize,
  pub sell_listed: f64,
  pub buy_escrow: f64,
}

// One tracked watch, enriched for the Watchlist grid. Region/system names are resolved at load; the
// item name and live current/met status are derived from state (`tree`, `watch_prices`) at view time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WatchCard {
  pub direction: WatchDirection,
  pub region_id: Option<i64>,
  pub region_label: String,
  pub system_label: String,
  pub target: Option<f64>,
  pub type_id: i64,
  pub watch: MarketWatch,
}

// The open card context menu: the pointer anchor it renders at plus the full source watch, so its
// Edit action can pre-fill the modal and its Remove action can delete the exact row.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct WatchMenu {
  pub(super) anchor: Point,
  pub(super) watch: MarketWatch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct State {
  tab: Tab,
  tree: tree::MarketTree,
  tree_pane: PaneDrag,
  book: Option<book::OrderBook>,
  book_access: BookAccess,
  cart: cart::Cart,
  compare_add_target: Option<compare::BlockId>,
  compare_enabled: bool,
  compare_pins: Vec<compare::CompareBlock>,
  compare_search: LocationSearch,
  compare_transient: Option<compare::CompareBlock>,
  expanded: HashSet<i64>,
  filter: String,
  filtered: Option<Vec<tree::FilteredGroup>>,
  selected: Option<i64>,
  active_region: Option<LocationRef>,
  region_search: LocationSearch,
  region_picker_open: bool,
  orders_scope: OrdersScope,
  orders_picker_open: bool,
  orders: OrdersData,
  orders_sub: OrdersSubTab,
  lot_groups: Vec<LotGroupCard>,
  lot_dismiss: Option<LotDismissPrompt>,
  open_window_notice: Option<OpenWindowNotice>,
  alert_outbid: i64,
  watch_modal: Option<watchlist::WatchForm>,
  watch_prices: watch_eval::PriceMap,
  own_orders: Vec<MarketOrder>,
  detail_view: DetailView,
  active_structure: Option<LocationRef>,
  active_place: Option<LocationRef>,
  history_key: Option<(i64, i64)>,
  history_state: HistoryFetch,
  history_range: history::Range,
  watches: Vec<WatchCard>,
  watch_menu: Option<WatchMenu>,
  watch_cursor: Option<Point>,
  dragging_watch: Option<i64>,
  watch_drop_target: Option<i64>,
  watch_grip_hover: Option<i64>,
  compare_menu: Option<compare::CompareMenu>,
  compare_cursor: Option<Point>,
  compare_dragging: Option<i64>,
  compare_drop_target: Option<i64>,
  compare_grip_hover: Option<i64>,
  tree_menu: Option<cart::TreeMenu>,
  tree_cursor: Option<Point>,
}

impl State {
  pub fn new() -> Self {
    State {
      tab: Tab::default(),
      tree: tree::MarketTree::default(),
      tree_pane: PaneDrag::new(MARKET_TREE_PANE_DEFAULT, spacing::layout::WINDOW_DEFAULT_WIDTH),
      book: None,
      book_access: BookAccess::default(),
      cart: cart::Cart::default(),
      compare_add_target: None,
      compare_enabled: false,
      compare_pins: Vec::new(),
      compare_search: LocationSearch::default(),
      compare_transient: None,
      expanded: HashSet::new(),
      filter: String::new(),
      filtered: None,
      selected: None,
      active_region: None,
      region_search: LocationSearch::default(),
      region_picker_open: false,
      orders_scope: OrdersScope::default(),
      orders_picker_open: false,
      orders: OrdersData::default(),
      orders_sub: OrdersSubTab::default(),
      lot_groups: Vec::new(),
      lot_dismiss: None,
      open_window_notice: None,
      alert_outbid: 0,
      watch_modal: None,
      watch_prices: watch_eval::PriceMap::new(),
      own_orders: Vec::new(),
      detail_view: DetailView::default(),
      active_structure: None,
      active_place: None,
      history_key: None,
      history_state: HistoryFetch::default(),
      history_range: history::Range::default(),
      watches: Vec::new(),
      watch_menu: None,
      watch_cursor: None,
      dragging_watch: None,
      watch_drop_target: None,
      watch_grip_hover: None,
      compare_menu: None,
      compare_cursor: None,
      compare_dragging: None,
      compare_drop_target: None,
      compare_grip_hover: None,
      tree_menu: None,
      tree_cursor: None,
    }
  }

  pub fn with_restored_panes(mut self, ui: &UiState) -> Self {
    let host_width = ui.host_width("main", spacing::layout::WINDOW_DEFAULT_WIDTH);
    self.tree_pane = PaneDrag::from_store(ui, MARKET_TREE_PANE_KEY, MARKET_TREE_PANE_DEFAULT, host_width);
    self
  }

  pub fn with_features(mut self, features: crate::config::FeatureFlags) -> Self {
    self.compare_enabled = features.is_sub_enabled(crate::config::SubFeature::MarketCompare);
    self
  }

  pub fn set_pane_host_width(&mut self, host_width: f32) {
    self.tree_pane.set_host_width(host_width);
  }

  pub fn active_tab(&self) -> Tab {
    self.tab
  }

  pub fn tree(&self) -> &tree::MarketTree {
    &self.tree
  }

  pub(super) fn tree_pane_width(&self) -> f32 {
    self.tree_pane.width()
  }

  pub fn filter(&self) -> &str {
    &self.filter
  }

  pub(super) fn filtered_catalog(&self) -> Option<&[tree::FilteredGroup]> {
    self.filtered.as_deref()
  }

  pub fn is_expanded(&self, id: i64) -> bool {
    self.expanded.contains(&id)
  }

  pub fn selected_type_id(&self) -> Option<i64> {
    self.selected
  }

  pub fn active_region(&self) -> Option<&LocationRef> {
    self.active_region.as_ref()
  }

  pub fn active_region_id(&self) -> Option<i64> {
    self.active_region.as_ref().map(|region| region.id)
  }

  pub fn active_location(&self) -> Option<&LocationRef> {
    self
      .active_place
      .as_ref()
      .or(self.active_structure.as_ref())
      .or(self.active_region.as_ref())
  }

  pub fn region_picker_open(&self) -> bool {
    self.region_picker_open
  }

  pub fn region_query(&self) -> &str {
    self.region_search.query()
  }

  pub fn region_results(&self) -> &[LocationRef] {
    self.region_search.results()
  }

  pub fn region_highlight(&self) -> Option<usize> {
    self.region_search.highlight()
  }

  pub fn region_searching(&self) -> bool {
    self.region_search.searching()
  }

  pub fn book(&self) -> Option<&book::OrderBook> {
    self.book.as_ref()
  }

  pub fn book_access(&self) -> BookAccess {
    self.book_access
  }

  pub fn compare_enabled(&self) -> bool {
    self.compare_enabled
  }

  pub fn compare_pins(&self) -> &[compare::CompareBlock] {
    &self.compare_pins
  }

  pub fn compare_transient(&self) -> Option<&compare::CompareBlock> {
    self.compare_transient.as_ref()
  }

  pub fn compare_add_target(&self) -> Option<compare::BlockId> {
    self.compare_add_target
  }

  pub fn compare_query(&self) -> &str {
    self.compare_search.query()
  }

  pub fn compare_results(&self) -> &[LocationRef] {
    self.compare_search.results()
  }

  pub fn compare_highlight(&self) -> Option<usize> {
    self.compare_search.highlight()
  }

  pub fn compare_searching(&self) -> bool {
    self.compare_search.searching()
  }

  pub fn orders(&self) -> &OrdersData {
    &self.orders
  }

  pub fn orders_scope(&self) -> OrdersScope {
    self.orders_scope
  }

  pub fn orders_picker_open(&self) -> bool {
    self.orders_picker_open
  }

  pub fn orders_show_character(&self) -> bool {
    matches!(self.orders_scope, OrdersScope::All)
  }

  pub fn orders_sub(&self) -> OrdersSubTab {
    self.orders_sub
  }

  pub fn lot_groups(&self) -> &[LotGroupCard] {
    &self.lot_groups
  }

  pub(super) fn lot_dismiss(&self) -> Option<&LotDismissPrompt> {
    self.lot_dismiss.as_ref()
  }

  pub fn alert_outbid(&self) -> i64 {
    self.alert_outbid
  }

  pub fn outbid_count(&self) -> usize {
    self.orders.outbid_count
  }

  pub(super) fn open_window_notice(&self) -> Option<&OpenWindowNotice> {
    self.open_window_notice.as_ref()
  }

  #[allow(dead_code)]
  pub fn watch_prices(&self) -> &watch_eval::PriceMap {
    &self.watch_prices
  }

  pub fn watches(&self) -> &[WatchCard] {
    &self.watches
  }

  pub(super) fn watch_menu(&self) -> Option<&WatchMenu> {
    self.watch_menu.as_ref()
  }

  pub fn own_orders(&self) -> &[MarketOrder] {
    &self.own_orders
  }

  pub fn detail_view(&self) -> DetailView {
    self.detail_view
  }

  pub fn history_state(&self) -> &HistoryFetch {
    &self.history_state
  }

  pub fn history_range(&self) -> history::Range {
    self.history_range
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Tab {
  #[default]
  Browse,
  Compare,
  Orders,
  Watchlist,
}

impl Tab {
  pub const ORDER: [Tab; 4] = [Tab::Browse, Tab::Orders, Tab::Compare, Tab::Watchlist];

  pub fn from_id(id: &str) -> Option<Tab> {
    match id {
      "browse" => Some(Tab::Browse),
      "compare" => Some(Tab::Compare),
      "orders" => Some(Tab::Orders),
      "watchlist" => Some(Tab::Watchlist),
      _ => None,
    }
  }

  pub fn id(self) -> &'static str {
    match self {
      Tab::Browse => "browse",
      Tab::Compare => "compare",
      Tab::Orders => "orders",
      Tab::Watchlist => "watchlist",
    }
  }
}

pub fn load(db: &Database, esi: Arc<esi::Client>, sso: Arc<eve_sso::Client>) -> Task<Message> {
  Task::batch([
    Task::perform(load_tree(db.clone()), |tree| Message::TreeLoaded(Box::new(tree))),
    Task::perform(
      resolve_default_market(db.clone(), esi, sso),
      Message::DefaultMarketResolved,
    ),
    Task::perform(load_own_orders(db.clone()), Message::OwnOrdersLoaded),
    Task::perform(fetch_alert_outbid(db.clone()), Message::AlertOutbidLoaded),
    Task::perform(fetch_watches(db.clone()), Message::WatchesLoaded),
    cart::load_snapshot_task(db),
  ])
}

async fn fetch_alert_outbid(db: Database) -> i64 {
  crate::store::repo::market_alert_state::count_alerted_outbid_open(&db)
    .await
    .unwrap_or(0)
}

async fn load_own_orders(db: Database) -> Vec<MarketOrder> {
  finance::open_all(&db).await.unwrap_or_default()
}

async fn load_tree(db: Database) -> tree::MarketTree {
  let groups = sde::all_market_groups(&db).await.unwrap_or_default();
  let items = sde::all_item_types(&db).await.unwrap_or_default();
  tree::build_market_tree(&groups, &items)
}

async fn resolve_default_market(db: Database, esi: Arc<esi::Client>, sso: Arc<eve_sso::Client>) -> LocationRef {
  match market_repo::default_market(&db).await {
    Ok(Some(place)) => resolve_default_place(&db, &esi, &sso, place).await,
    _ => region_ref(&db, THE_FORGE_REGION_ID).await,
  }
}

async fn resolve_default_place(db: &Database, esi: &esi::Client, sso: &eve_sso::Client, place: i64) -> LocationRef {
  match LocationTier::from_id(place) {
    Some(LocationTier::Region) => region_ref(db, place).await,
    Some(LocationTier::Structure) => resolve_default_structure(db, esi, sso, place).await,
    Some(tier) => resolve_named_place(db, place, tier).await,
    None => region_ref(db, THE_FORGE_REGION_ID).await,
  }
}

async fn resolve_named_place(db: &Database, place: i64, tier: LocationTier) -> LocationRef {
  let name = named_place_name(db, place, tier)
    .await
    .unwrap_or_else(|| format!("#{place}"));
  place_ref(place, name, tier)
}

async fn named_place_name(db: &Database, place: i64, tier: LocationTier) -> Option<String> {
  match tier {
    LocationTier::Constellation => sde::get_constellation(db, place)
      .await
      .ok()
      .flatten()
      .map(|constellation| constellation.name().to_owned()),
    LocationTier::Station => sde::get_station(db, place)
      .await
      .ok()
      .flatten()
      .map(|station| station.name().to_owned()),
    LocationTier::System => sde::get_solar_system(db, place)
      .await
      .ok()
      .flatten()
      .map(|system| system.name().to_owned()),
    _ => None,
  }
}

/// Always resolves to `LocationTier::Structure`; unlike its sibling resolvers it never falls back
/// to a region or The Forge, even when both the local SDE lookup and the authed ESI lookup miss
/// (the name then falls back to `#{place}`).
async fn resolve_default_structure(db: &Database, esi: &esi::Client, sso: &eve_sso::Client, place: i64) -> LocationRef {
  if let Ok(Some(structure)) = sde::get_structure(db, place).await {
    return place_ref(place, structure.name().to_owned(), LocationTier::Structure);
  }
  let name = crate::features::industry::resolve_structure(db, esi, sso, place)
    .await
    .map(|facility| facility.name)
    .unwrap_or_else(|| format!("#{place}"));
  place_ref(place, name, LocationTier::Structure)
}

async fn region_of(db: &Database, place: i64) -> Option<i64> {
  match LocationTier::from_id(place) {
    Some(LocationTier::Region) => Some(place),
    Some(LocationTier::Constellation) => sde::get_constellation(db, place)
      .await
      .ok()
      .flatten()
      .map(|constellation| constellation.region_id()),
    Some(LocationTier::System) => region_of_system(db, place).await,
    Some(LocationTier::Station) => {
      let station = sde::get_station(db, place).await.ok().flatten()?;
      region_of_system(db, station.system_id()).await
    }
    // Structures have no single region here; callers fall back to The Forge for the region-scoped
    // order book, while structure defaults themselves resolve fully via `resolve_default_structure`.
    _ => None,
  }
}

async fn region_of_system(db: &Database, system_id: i64) -> Option<i64> {
  let system = sde::get_solar_system(db, system_id).await.ok().flatten()?;
  let constellation = sde::get_constellation(db, system.constellation_id())
    .await
    .ok()
    .flatten()?;
  Some(constellation.region_id())
}

async fn region_ref(db: &Database, region_id: i64) -> LocationRef {
  let name = sde::get_region(db, region_id)
    .await
    .ok()
    .flatten()
    .map(|region| region.name().to_owned())
    .unwrap_or_else(|| t!("market.region_fallback_name").into_owned());
  region_location(region_id, name)
}

fn region_location(id: i64, name: String) -> LocationRef {
  place_ref(id, name, LocationTier::Region)
}

fn place_ref(id: i64, name: String, tier: LocationTier) -> LocationRef {
  LocationRef {
    context: None,
    id,
    name,
    security_status: None,
    tier: Some(tier),
  }
}

async fn resolve_place_region(db: Database, place_id: i64) -> LocationRef {
  let region_id = region_of(&db, place_id).await.unwrap_or(THE_FORGE_REGION_ID);
  region_ref(&db, region_id).await
}

// Browse and the watch modal both search locations through the shared, ESI-aware service so their
// pickers offer every tier (including any dockable structure). Live structure discovery needs an
// authed grant, which only the app layer can build, so it calls `location_search_task` after the
// reducer has bumped the search generation.
pub enum LocationSearchField {
  Browse(String),
  Compare(String),
  Watch(String),
}

pub fn location_search_field(message: &Message) -> Option<LocationSearchField> {
  match message {
    Message::RegionSearchChanged(query) => Some(LocationSearchField::Browse(query.clone())),
    Message::CompareAddSearchChanged(query) => Some(LocationSearchField::Compare(query.clone())),
    Message::WatchRegionSearchChanged(query) => Some(LocationSearchField::Watch(query.clone())),
    _ => None,
  }
}

pub fn location_search_task(
  state: &State,
  db: &Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  field: LocationSearchField,
) -> Task<Message> {
  match field {
    LocationSearchField::Browse(query) => browse_location_search(state, db, esi, sso, query),
    LocationSearchField::Compare(query) => compare::location_search(state, db, esi, sso, query),
    LocationSearchField::Watch(query) => watchlist::watch_location_search(state, db, esi, sso, query),
  }
}

fn browse_location_search(
  state: &State,
  db: &Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  query: String,
) -> Task<Message> {
  if !state.region_search.searchable() {
    return Task::none();
  }
  let generation = state.region_search.generation();
  Task::perform(
    crate::services::location_search::search_locations_enriched(db.clone(), esi, sso, query, LOCATION_SEARCH_MIN_CHARS),
    move |results| Message::RegionResultsLoaded(generation, results),
  )
}

fn fetch_book_task(state: &State, db: &Database) -> Task<Message> {
  // A structure book needs an authed grant, so it is fetched at the app layer via
  // `fetch_structure_book_task`; skip the tokenless region path while a structure is active.
  if state.active_structure.is_some() {
    return Task::none();
  }
  match (state.active_region_id(), state.selected_type_id()) {
    (Some(region_id), Some(type_id)) => load_book(db, region_id, type_id, place_filter(state)),
    _ => Task::none(),
  }
}

// When the picked location narrows below the region, the region book is filtered client-side to that
// station (exact location) or system, since ESI only serves whole-region order books.
#[derive(Clone, Copy)]
pub enum PlaceFilter {
  Station(i64),
  System(i64),
}

fn place_filter(state: &State) -> Option<PlaceFilter> {
  let place = state.active_place.as_ref()?;
  match place.tier {
    Some(LocationTier::Station) => Some(PlaceFilter::Station(place.id)),
    Some(LocationTier::System) => Some(PlaceFilter::System(place.id)),
    _ => None,
  }
}

fn apply_place_filter(orders: Vec<RegionOrder>, filter: Option<PlaceFilter>) -> Vec<RegionOrder> {
  match filter {
    Some(PlaceFilter::Station(id)) => orders.into_iter().filter(|order| order.location_id == id).collect(),
    Some(PlaceFilter::System(id)) => orders.into_iter().filter(|order| order.system_id == id).collect(),
    None => orders,
  }
}

pub fn load_book(db: &Database, region_id: i64, type_id: i64, filter: Option<PlaceFilter>) -> Task<Message> {
  Task::perform(fetch_book(db.clone(), region_id, type_id, filter), |book| {
    Message::BookLoaded(Box::new(book))
  })
}

async fn fetch_book(db: Database, region_id: i64, type_id: i64, filter: Option<PlaceFilter>) -> book::OrderBook {
  let Ok(esi) = public_esi(&db) else {
    return book::OrderBook::default();
  };
  let mut orders = esi.market().sell_orders(region_id, type_id).await.unwrap_or_default();
  orders.extend(esi.market().buy_orders(region_id, type_id).await.unwrap_or_default());
  let mut book = book::build_order_book(apply_place_filter(orders, filter));
  label_book_locations(&db, &mut book).await;
  book
}

fn compare_place_filter(place: &LocationRef) -> Option<PlaceFilter> {
  match place.tier {
    Some(LocationTier::Station) => Some(PlaceFilter::Station(place.id)),
    Some(LocationTier::System) => Some(PlaceFilter::System(place.id)),
    _ => None,
  }
}

fn load_compare_book(db: &Database, place: &LocationRef, type_id: i64) -> Task<Message> {
  let place_id = place.id;
  Task::perform(fetch_compare_book(db.clone(), place.clone(), type_id), move |book| {
    Message::CompareBookLoaded(type_id, place_id, Box::new(book))
  })
}

async fn fetch_compare_book(db: Database, place: LocationRef, type_id: i64) -> book::OrderBook {
  let region_id = region_of(&db, place.id).await.unwrap_or(THE_FORGE_REGION_ID);
  fetch_book(db, region_id, type_id, compare_place_filter(&place)).await
}

pub fn compare_structure_fetches(state: &State, message: &Message) -> Vec<(i64, i64)> {
  match message {
    Message::ComparePinsLoaded(blocks) => dedup_fetch_pairs(blocks.iter().flat_map(compare_block_structures).collect()),
    Message::CompareTransientLoaded(block) => compare_block_structures(block),
    Message::CompareMarketPicked(place) => compare_picked_structure(state, place),
    _ => Vec::new(),
  }
}

fn compare_block_structures(block: &compare::CompareBlock) -> Vec<(i64, i64)> {
  compare::structure_fetches(block.columns.iter().map(|column| &column.place), block.type_id)
}

fn compare_picked_structure(state: &State, place: &LocationRef) -> Vec<(i64, i64)> {
  if place.tier != Some(LocationTier::Structure) {
    return Vec::new();
  }
  state
    .compare_add_target
    .and_then(|target| compare::find_block(state, target))
    .map(|block| vec![(place.id, block.type_id)])
    .unwrap_or_default()
}

fn dedup_fetch_pairs(pairs: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
  let mut seen = HashSet::new();
  pairs.into_iter().filter(|pair| seen.insert(*pair)).collect()
}

pub fn fetch_compare_structure_book_task(
  db: &Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  place_id: i64,
  type_id: i64,
) -> Task<Message> {
  Task::perform(
    fetch_structure_book(db.clone(), esi, sso, place_id, type_id),
    move |result| Message::CompareStructureBookLoaded(place_id, type_id, result),
  )
}

async fn label_book_locations(db: &Database, book: &mut book::OrderBook) {
  let mut ids: Vec<i64> = book
    .sell
    .iter()
    .chain(book.buy.iter())
    .map(|row| row.location_id)
    .collect();
  ids.sort_unstable();
  ids.dedup();
  let mut names: HashMap<i64, String> = sde::stations_for(db, &ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|station| (station.id(), station.name().clone()))
    .collect();
  for id in &ids {
    if names.contains_key(id) {
      continue;
    }
    let label = match sde::get_structure(db, *id).await {
      Ok(Some(structure)) => structure.name().clone(),
      _ if LocationTier::from_id(*id) == Some(LocationTier::Structure) => {
        t!("market.book_structure_fallback", id => id).into_owned()
      }
      _ => system_label(db, *id).await,
    };
    names.insert(*id, label);
  }
  for row in book.sell.iter_mut().chain(book.buy.iter_mut()) {
    if let Some(name) = names.get(&row.location_id) {
      row.location_label = name.clone();
    }
  }
}

// Player structures aren't in the static SDE, so a region book that quotes them shows a fallback until
// an authed lookup resolves and caches their names. Only structures the owning character can dock at
// (or public ones) resolve; the rest keep the fallback.
fn structure_ids(book: &book::OrderBook) -> Vec<i64> {
  let mut ids: Vec<i64> = book
    .sell
    .iter()
    .chain(book.buy.iter())
    .map(|row| row.location_id)
    .filter(|id| LocationTier::from_id(*id) == Some(LocationTier::Structure))
    .collect();
  ids.sort_unstable();
  ids.dedup();
  ids
}

pub fn book_structure_ids(state: &State) -> Vec<i64> {
  state.book.as_ref().map(structure_ids).unwrap_or_default()
}

pub fn resolve_book_structures_task(
  db: &Database,
  esi: Arc<esi::Client>,
  image: Arc<eve_image::Client>,
  sso: Arc<eve_sso::Client>,
  book: book::OrderBook,
) -> Task<Message> {
  Task::perform(resolve_book_structures(db.clone(), esi, image, sso, book), |book| {
    Message::BookRelabeled(Box::new(book))
  })
}

async fn resolve_book_structures(
  db: Database,
  esi: Arc<esi::Client>,
  image: Arc<eve_image::Client>,
  sso: Arc<eve_sso::Client>,
  mut book: book::OrderBook,
) -> book::OrderBook {
  let Some(grant) = first_owned_grant(&db, &sso).await else {
    return book;
  };
  let store = images::default_store();
  for id in structure_ids(&book) {
    if matches!(sde::get_structure(&db, id).await, Ok(Some(_))) {
      continue;
    }
    let _ = crate::sync::resolve_stockpile_location(&db, &esi, &image, &store, &grant, id).await;
  }
  label_book_locations(&db, &mut book).await;
  book
}

pub fn load_history(db: &Database, region_id: i64, type_id: i64) -> Task<Message> {
  Task::perform(fetch_history(db.clone(), region_id, type_id), move |result| {
    Message::HistoryLoaded(region_id, type_id, result)
  })
}

async fn fetch_history(db: Database, region_id: i64, type_id: i64) -> Result<Vec<history::HistoryPoint>, String> {
  let esi = public_esi(&db).map_err(|error| error.to_string())?;
  let raw = esi
    .market()
    .history(region_id, type_id)
    .await
    .map_err(|error| error.to_string())?;
  Ok(history::series(&raw))
}

fn history_follow_task(state: &State, prev_key: Option<(i64, i64)>, db: &Database) -> Task<Message> {
  match state.history_key {
    Some((region_id, type_id)) if state.history_key != prev_key => load_history(db, region_id, type_id),
    _ => Task::none(),
  }
}

pub fn wants_watch_prices(message: &Message) -> bool {
  matches!(message, Message::TabSelected(Tab::Watchlist))
}

pub fn watch_prices_task(db: &Database, esi: Arc<esi::Client>, sso: Arc<eve_sso::Client>) -> Task<Message> {
  Task::perform(fetch_watch_prices(db.clone(), esi, sso), Message::WatchPricesLoaded)
}

fn load_watches(db: &Database) -> Task<Message> {
  Task::perform(fetch_watches(db.clone()), Message::WatchesLoaded)
}

fn remove_watch_task(db: &Database, id: i64) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      let _ = crate::store::repo::market_watchlist::delete(&db, id).await;
      fetch_watches(db).await
    },
    Message::WatchesLoaded,
  )
}

// Enrich each tracked watch with its region/system names so the grid renders deterministically from
// state. The live current/met status is layered on in the view from `watch_prices`.
pub(super) async fn fetch_watches(db: Database) -> Vec<WatchCard> {
  let watches = crate::store::repo::market_watchlist::list(&db)
    .await
    .unwrap_or_default();
  let mut cards = Vec::with_capacity(watches.len());
  for watch in watches {
    cards.push(build_watch_card(&db, watch).await);
  }
  cards
}

async fn build_watch_card(db: &Database, watch: MarketWatch) -> WatchCard {
  let region_label = match watch.region_id {
    Some(region_id) => watch_region_name(db, region_id).await,
    None => String::new(),
  };
  let system_label = scope_place_label(db, &watch).await;
  WatchCard {
    direction: WatchDirection::parse(&watch.direction).unwrap_or_default(),
    region_id: watch.region_id,
    region_label,
    system_label,
    target: watch.target_price,
    type_id: watch.type_id,
    watch,
  }
}

fn watch_tier(watch: &MarketWatch, scope_id: i64) -> LocationTier {
  watch
    .location_tier
    .as_deref()
    .and_then(LocationTier::parse)
    .or_else(|| LocationTier::from_id(scope_id))
    .unwrap_or(LocationTier::Region)
}

async fn scope_place_label(db: &Database, watch: &MarketWatch) -> String {
  let Some(scope_id) = watch.location_id else {
    return String::new();
  };
  match watch_tier(watch, scope_id) {
    LocationTier::Region => String::new(),
    LocationTier::Constellation => named_or_fallback(
      sde::get_constellation(db, scope_id)
        .await
        .ok()
        .flatten()
        .map(|constellation| constellation.name().clone()),
      scope_id,
    ),
    LocationTier::Station => system_label(db, scope_id).await,
    LocationTier::Structure => named_or_fallback(
      sde::get_structure(db, scope_id)
        .await
        .ok()
        .flatten()
        .map(|structure| structure.name().clone()),
      scope_id,
    ),
    LocationTier::System => named_or_fallback(
      sde::get_solar_system(db, scope_id)
        .await
        .ok()
        .flatten()
        .map(|system| system.name().clone()),
      scope_id,
    ),
  }
}

fn named_or_fallback(name: Option<String>, id: i64) -> String {
  name.unwrap_or_else(|| t!("market.orders_location_fallback", id => id).into_owned())
}

async fn watch_region_name(db: &Database, region_id: i64) -> String {
  sde::get_region(db, region_id)
    .await
    .ok()
    .flatten()
    .map(|region| region.name().clone())
    .unwrap_or_else(|| t!("market.region_fallback_name").into_owned())
}

struct WatchScope {
  region_id: Option<i64>,
  scope_id: i64,
  tier: LocationTier,
  type_id: i64,
}

fn watch_scope(watch: &MarketWatch) -> Option<WatchScope> {
  let scope_id = watch.location_id.or(watch.region_id)?;
  Some(WatchScope {
    region_id: watch.region_id,
    scope_id,
    tier: watch_tier(watch, scope_id),
    type_id: watch.type_id,
  })
}

async fn fetch_watch_prices(db: Database, esi: Arc<esi::Client>, sso: Arc<eve_sso::Client>) -> watch_eval::PriceMap {
  let watches = crate::store::repo::market_watchlist::list(&db)
    .await
    .unwrap_or_default();
  let mut scopes: HashMap<watch_eval::BookKey, WatchScope> = HashMap::new();
  for watch in &watches {
    if let Some(scope) = watch_scope(watch) {
      scopes.entry((scope.type_id, scope.scope_id)).or_insert(scope);
    }
  }
  let mut prices = watch_eval::PriceMap::new();
  for (key, scope) in scopes {
    prices.insert(key, scope_best_prices(&db, &esi, &sso, &scope).await);
  }
  prices
}

// A structure watch reads that structure's own authed order book; every other tier derives from the
// region's public orders, then narrows to the scope in `filter_orders_to_scope` (station by location,
// system by system, constellation by member systems, region unfiltered).
async fn scope_best_prices(
  db: &Database,
  esi: &esi::Client,
  sso: &eve_sso::Client,
  scope: &WatchScope,
) -> watch_eval::BestPrices {
  match scope.tier {
    LocationTier::Structure => structure_scope_prices(db, esi, sso, scope.scope_id, scope.type_id).await,
    _ => region_scope_prices(db, esi, scope).await,
  }
}

async fn structure_scope_prices(
  db: &Database,
  esi: &esi::Client,
  sso: &eve_sso::Client,
  structure_id: i64,
  type_id: i64,
) -> watch_eval::BestPrices {
  let Some(grant) = first_owned_grant(db, sso).await else {
    return watch_eval::BestPrices::inaccessible();
  };
  match esi.market().structure_orders(structure_id, &grant).await {
    Ok(orders) => {
      let filtered = orders.into_iter().filter(|order| order.type_id == type_id).collect();
      let book = book::build_order_book(filtered);
      watch_eval::BestPrices::available(book.best_buy, book.best_sell)
    }
    Err(error) => {
      tracing::warn!(target: "pod::market", %error, structure_id, "watch structure price fetch failed");
      watch_eval::BestPrices::inaccessible()
    }
  }
}

async fn region_scope_prices(db: &Database, esi: &esi::Client, scope: &WatchScope) -> watch_eval::BestPrices {
  let Some(region_id) = scope.region_id else {
    return watch_eval::BestPrices::default();
  };
  let mut orders = esi
    .market()
    .sell_orders(region_id, scope.type_id)
    .await
    .unwrap_or_default();
  orders.extend(
    esi
      .market()
      .buy_orders(region_id, scope.type_id)
      .await
      .unwrap_or_default(),
  );
  let book = book::build_order_book(filter_orders_to_scope(db, scope, orders).await);
  watch_eval::BestPrices::available(book.best_buy, book.best_sell)
}

async fn filter_orders_to_scope(db: &Database, scope: &WatchScope, orders: Vec<RegionOrder>) -> Vec<RegionOrder> {
  match scope.tier {
    LocationTier::Station => orders
      .into_iter()
      .filter(|order| order.location_id == scope.scope_id)
      .collect(),
    LocationTier::System => orders
      .into_iter()
      .filter(|order| order.system_id == scope.scope_id)
      .collect(),
    LocationTier::Constellation => {
      let systems = constellation_systems(db, scope.scope_id).await;
      orders
        .into_iter()
        .filter(|order| systems.contains(&order.system_id))
        .collect()
    }
    _ => orders,
  }
}

async fn constellation_systems(db: &Database, constellation_id: i64) -> HashSet<i64> {
  sde::all_solar_systems(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter(|system| system.constellation_id() == constellation_id)
    .map(|system| system.id())
    .collect()
}

fn public_esi(db: &Database) -> Result<esi::Client, clients::Error> {
  let http = http::Client::builder(http::Cache::new(db.clone())).build();
  esi::Client::builder(http).user_agent(clients::user_agent()).build()
}

fn load_orders_task(state: &State, db: &Database) -> Task<Message> {
  let scope = state.orders_scope;
  Task::perform(fetch_orders(db.clone(), scope), |data| {
    Message::OrdersLoaded(Box::new(data))
  })
}

fn load_lots_task(db: &Database) -> Task<Message> {
  Task::perform(fetch_lots(db.clone()), Message::LotsLoaded)
}

fn dismiss_lot_task(db: &Database, transaction_id: i64, owner_id: i64, is_corporation: bool) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      if let Err(error) = finance::dismiss_lot(&db, transaction_id, owner_id, is_corporation).await {
        tracing::warn!(target: "pod::market", %error, transaction_id, "lot dismissal failed");
      }
      fetch_lots(db).await
    },
    Message::LotsLoaded,
  )
}

async fn fetch_lots(db: Database) -> Vec<LotGroupCard> {
  let groups = inventory_lots::derive(&db).await.unwrap_or_default();
  let owners = lot_owner_names(&db).await;
  let mut labels: HashMap<i64, (String, String)> = HashMap::new();
  let mut cards = Vec::with_capacity(groups.len());
  for group in groups {
    let (region_label, system_label) = match labels.get(&group.location_id) {
      Some(resolved) => resolved.clone(),
      None => {
        let resolved = lot_location_labels(&db, group.location_id).await;
        labels.insert(group.location_id, resolved.clone());
        resolved
      }
    };
    let owner_name = owners
      .get(&(group.owner_id, group.is_corporation))
      .cloned()
      .unwrap_or_default();
    cards.push(LotGroupCard {
      group,
      owner_name,
      region_label,
      system_label,
    });
  }
  sort_lot_cards(&mut cards);
  cards
}

async fn lot_owner_names(db: &Database) -> HashMap<(i64, bool), String> {
  let mut names = HashMap::new();
  for character in character_repo::all_owned(db).await.unwrap_or_default() {
    names.insert((character.id(), false), character.name().clone());
  }
  for corporation in org::all_owned_corporations(db).await.unwrap_or_default() {
    names.insert((corporation.id(), true), corporation.name().clone());
  }
  names
}

async fn lot_location_labels(db: &Database, location_id: i64) -> (String, String) {
  let Some(system_id) = lot_system_id(db, location_id).await else {
    return (
      t!("market.orders_location_fallback", id => location_id).into_owned(),
      String::new(),
    );
  };
  let system_label = sde::get_solar_system(db, system_id)
    .await
    .ok()
    .flatten()
    .map(|system| system.name().clone())
    .unwrap_or_else(|| t!("market.orders_location_fallback", id => location_id).into_owned());
  let region_label = match region_of_system(db, system_id).await {
    Some(region_id) => watch_region_name(db, region_id).await,
    None => t!("market.region_fallback_name").into_owned(),
  };
  (region_label, system_label)
}

async fn lot_system_id(db: &Database, location_id: i64) -> Option<i64> {
  if let Ok(Some(station)) = sde::get_station(db, location_id).await {
    return Some(station.system_id());
  }
  if let Ok(Some(structure)) = sde::get_structure(db, location_id).await {
    return Some(structure.solar_system_id());
  }
  None
}

fn sort_lot_cards(cards: &mut [LotGroupCard]) {
  cards.sort_by(|left, right| newest_lot_date(right).cmp(newest_lot_date(left)));
}

fn newest_lot_date(card: &LotGroupCard) -> &str {
  card.group.lots.last().map(|lot| lot.date.as_str()).unwrap_or("")
}

async fn fetch_orders(db: Database, scope: OrdersScope) -> OrdersData {
  let char_orders = load_char_orders(&db, scope).await;
  let corp_orders = load_corp_orders(&db, scope).await;

  let raw = dedup_orders(char_orders, corp_orders);

  let quotes = fetch_quotes(&db, &raw).await;
  let annotations = outbid::annotate_all(&raw, &quotes);
  let roster = load_roster(&db).await;
  let mut names: HashMap<i64, String> = roster.iter().map(|pilot| (pilot.id, pilot.name.clone())).collect();
  names.extend(load_corp_names(&db).await);

  let mut rows = Vec::with_capacity(raw.len());
  for (order, annotation) in raw.iter().zip(annotations.iter()) {
    rows.push(build_order_row(&db, order, annotation, &names).await);
  }
  sort_order_rows(&mut rows);

  OrdersData {
    scope,
    active_count: raw.iter().filter(|order| order.volume_remain() > 0).count(),
    sell_count: order_side_count(&raw, false),
    buy_count: order_side_count(&raw, true),
    outbid_count: annotations.iter().filter(|annotation| annotation.outbid).count(),
    sell_listed: sell_listed(&raw),
    buy_escrow: buy_escrow(&raw),
    roster,
    rows,
  }
}

// Char-table and corp-table copies of the same in-game order share an order_id; the char row wins
// wholesale (pilot's name, its stored is_corporation badge), so only corp-only orders survive from
// the corp-table set. De-duping here keeps every downstream count (rows, header stats, outbid) on a
// single row per in-game order.
fn dedup_orders(char_orders: Vec<MarketOrder>, corp_orders: Vec<MarketOrder>) -> Vec<MarketOrder> {
  let char_order_ids: HashSet<i64> = char_orders.iter().map(MarketOrder::order_id).collect();
  let mut out = char_orders;
  out.extend(
    corp_orders
      .into_iter()
      .filter(|order| !char_order_ids.contains(&order.order_id())),
  );
  out
}

async fn load_char_orders(db: &Database, scope: OrdersScope) -> Vec<MarketOrder> {
  match scope.character_id() {
    Some(id) => finance::open_for_character(db, id).await,
    None => finance::open_all(db).await,
  }
  .unwrap_or_default()
}

// Corp orders aren't attributable to a single character, so they only surface in the unscoped
// All view; a Character(id) scope shows that pilot's personal orders only.
async fn load_corp_orders(db: &Database, scope: OrdersScope) -> Vec<MarketOrder> {
  if !matches!(scope, OrdersScope::All) {
    return Vec::new();
  }
  let mut out = Vec::new();
  for corporation in org::all_owned_corporations(db).await.unwrap_or_default() {
    let orders = finance::open_for_corporation(db, corporation.id())
      .await
      .unwrap_or_default();
    out.extend(orders.iter().map(corp_order_as_market));
  }
  out
}

async fn load_corp_names(db: &Database) -> HashMap<i64, String> {
  org::all_owned_corporations(db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|corporation| (corporation.id(), corporation.name().clone()))
    .collect()
}

// Corp orders ride the same character-keyed pipeline as personal orders (roster/name lookup, outbid
// annotation), so the corporation id is stored in `character_id` and is_corporation drives the badge.
fn corp_order_as_market(order: &CorporationMarketOrder) -> MarketOrder {
  MarketOrder {
    character_id: order.corporation_id(),
    duration: order.duration(),
    escrow: order.escrow(),
    is_buy_order: order.is_buy_order(),
    is_corporation: true,
    issued: order.issued().clone(),
    location_id: order.location_id(),
    order_id: order.order_id(),
    price: order.price(),
    range: order.range().clone(),
    region_id: order.region_id(),
    state: order.state().clone(),
    type_id: order.type_id(),
    volume_remain: order.volume_remain(),
    volume_total: order.volume_total(),
  }
}

fn sell_listed(orders: &[MarketOrder]) -> f64 {
  orders
    .iter()
    .filter(|order| !order.is_buy_order())
    .map(|order| order.price() * order.volume_remain() as f64)
    .sum()
}

fn buy_escrow(orders: &[MarketOrder]) -> f64 {
  orders
    .iter()
    .filter(|order| order.is_buy_order())
    .map(MarketOrder::escrow)
    .sum()
}

async fn load_roster(db: &Database) -> Vec<OrderPilot> {
  character_repo::all_owned(db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|character| OrderPilot {
      id: character.id(),
      name: character.name().clone(),
      portrait: portrait_path(character.id()),
    })
    .collect()
}

fn portrait_path(character_id: i64) -> Option<std::path::PathBuf> {
  let path = images::default_store().character_portrait_path(character_id);
  path.exists().then_some(path)
}

fn order_side_count(orders: &[MarketOrder], is_buy: bool) -> usize {
  orders
    .iter()
    .filter(|order| order.is_buy_order() == is_buy && order.volume_remain() > 0)
    .count()
}

async fn fetch_quotes(db: &Database, orders: &[MarketOrder]) -> Vec<outbid::Quote> {
  let Ok(esi) = public_esi(db) else {
    return Vec::new();
  };
  let mut seen: HashSet<(i64, i64)> = HashSet::new();
  let mut quotes = Vec::new();
  for order in orders {
    let key = (order.region_id(), order.type_id());
    if !seen.insert(key) {
      continue;
    }
    let sells = esi.market().sell_orders(key.0, key.1).await.unwrap_or_default();
    let buys = esi.market().buy_orders(key.0, key.1).await.unwrap_or_default();
    push_quotes(&mut quotes, key.1, false, sells);
    push_quotes(&mut quotes, key.1, true, buys);
  }
  quotes
}

fn push_quotes(quotes: &mut Vec<outbid::Quote>, type_id: i64, is_buy: bool, orders: Vec<RegionOrder>) {
  for order in orders {
    quotes.push(outbid::Quote {
      is_buy_order: is_buy,
      location_id: order.location_id,
      price: order.price,
      type_id,
    });
  }
}

async fn build_order_row(
  db: &Database,
  order: &MarketOrder,
  annotation: &outbid::Annotation,
  names: &HashMap<i64, String>,
) -> OrderRow {
  let (region_label, system_label) = location_labels(db, order.region_id(), order.location_id()).await;
  let owner_is_corp = order.is_corporation();
  OrderRow {
    character_id: order.character_id(),
    character_name: names.get(&order.character_id()).cloned().unwrap_or_default(),
    owner_is_corp,
    owner_offline: !owner_is_corp && owner_known_offline(db, order.character_id()).await,
    type_id: order.type_id(),
    region_label,
    system_label,
    price: order.price(),
    is_buy: order.is_buy_order(),
    volume_remain: order.volume_remain(),
    volume_total: order.volume_total(),
    expires_days: order_expires_days(order.issued(), order.duration()),
    done: order.volume_remain() == 0,
    outbid: annotation.outbid,
    best: annotation.best,
    gap_pct: annotation.gap_pct,
  }
}

// Best-effort pre-flight: the owner is "known offline" only when fresh telemetry says so. Absent or
// stale telemetry (LocationTracking off, or the character not synced recently) reads as unknown, so
// the button stays enabled and the attempt surfaces its own error instead of being blocked.
async fn owner_known_offline(db: &Database, character_id: i64) -> bool {
  match character_repo::telemetry(db, character_id).await {
    Ok(Some(telemetry)) => telemetry_offline(
      telemetry.online(),
      telemetry.synced_at(),
      chrono::Utc::now().timestamp(),
    ),
    _ => false,
  }
}

fn telemetry_offline(online: bool, synced_at: i64, now: i64) -> bool {
  !online && now.saturating_sub(synced_at) <= TELEMETRY_STALE_SECS
}

async fn location_labels(db: &Database, region_id: i64, location_id: i64) -> (String, String) {
  let region = sde::get_region(db, region_id)
    .await
    .ok()
    .flatten()
    .map(|region| region.name().clone())
    .unwrap_or_else(|| t!("market.region_fallback_name").into_owned());
  (region, system_label(db, location_id).await)
}

async fn system_label(db: &Database, location_id: i64) -> String {
  if let Ok(Some(station)) = sde::get_station(db, location_id).await
    && let Ok(Some(system)) = sde::get_solar_system(db, station.system_id()).await
  {
    return system.name().clone();
  }
  t!("market.orders_location_fallback", id => location_id).into_owned()
}

fn order_expires_days(issued: &str, duration: i64) -> i64 {
  match chrono::DateTime::parse_from_rfc3339(issued) {
    Ok(issued) => {
      let expiry = issued + chrono::Duration::days(duration);
      (expiry.with_timezone(&chrono::Utc) - chrono::Utc::now())
        .num_days()
        .max(0)
    }
    Err(_) => duration.max(0),
  }
}

fn order_rank(row: &OrderRow) -> u8 {
  if row.done {
    2
  } else if row.outbid {
    0
  } else {
    1
  }
}

fn sort_order_rows(rows: &mut [OrderRow]) {
  rows.sort_by(|left, right| {
    order_rank(left)
      .cmp(&order_rank(right))
      .then(left.expires_days.cmp(&right.expires_days))
  });
}

pub fn open_market_window_task(
  db: &Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  character_id: i64,
  type_id: i64,
) -> Task<Message> {
  Task::perform(
    open_market_window(db.clone(), esi, sso, character_id, type_id),
    Message::MarketWindowOpened,
  )
}

async fn open_market_window(
  db: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  character_id: i64,
  type_id: i64,
) -> Result<(), OpenWindowFailure> {
  let character = character_display_name(&db, character_id).await;
  let grant = match fresh_character_grant(&db, &sso, character_id).await {
    Ok(grant) => grant,
    Err(error) => {
      tracing::warn!(target: "pod::market", character_id, %error, "open market window: no usable grant");
      return Err(OpenWindowFailure {
        character_id,
        message: open_window_message(OpenWindowFailureKind::Reauthorize, &character, ""),
      });
    }
  };
  match esi.market().open_market_window(type_id, &grant).await {
    Ok(()) => Ok(()),
    Err(failure) => {
      tracing::warn!(
        target: "pod::market",
        character_id,
        status = ?failure.status,
        body = %failure.body,
        "open market window request failed"
      );
      let kind = classify_open_window_failure(failure.status, &failure.body);
      Err(OpenWindowFailure {
        character_id,
        message: open_window_message(kind, &character, &esi_error_message(&failure.body)),
      })
    }
  }
}

async fn character_display_name(db: &Database, character_id: i64) -> String {
  match character_repo::get(db, character_id).await {
    Ok(Some(character)) => character.name().clone(),
    _ => t!("market.orders_open_character_fallback").into_owned(),
  }
}

fn classify_open_window_failure(status: Option<u16>, body: &str) -> OpenWindowFailureKind {
  if status == Some(403) {
    OpenWindowFailureKind::Reauthorize
  } else if body.to_lowercase().contains("online") {
    OpenWindowFailureKind::Offline
  } else {
    OpenWindowFailureKind::Generic
  }
}

// ESI reports the human-readable reason as `{"error":"..."}`; unwrap it when present, fall back to the
// raw body, and to a localized placeholder when the body is empty (a transport error carries none).
fn esi_error_message(body: &str) -> String {
  if let Some(message) = serde_json::from_str::<serde_json::Value>(body)
    .ok()
    .and_then(|value| value.get("error").and_then(|error| error.as_str()).map(str::to_owned))
    .filter(|message| !message.trim().is_empty())
  {
    return message;
  }
  let trimmed = body.trim();
  if trimmed.is_empty() {
    t!("market.orders_open_reason_unknown").into_owned()
  } else {
    trimmed.to_owned()
  }
}

fn open_window_message(kind: OpenWindowFailureKind, character: &str, esi_message: &str) -> String {
  match kind {
    OpenWindowFailureKind::Generic => {
      t!("market.orders_open_failed", character => character, message => esi_message).into_owned()
    }
    OpenWindowFailureKind::Offline => t!("market.orders_open_offline", character => character).into_owned(),
    OpenWindowFailureKind::Reauthorize => t!("market.orders_open_reauthorize", character => character).into_owned(),
  }
}

async fn fresh_character_grant(
  db: &Database,
  sso: &eve_sso::Client,
  character_id: i64,
) -> Result<eve_sso::Grant, String> {
  crate::sync::token::fresh_token(db, sso, character_id, OwnerType::Character)
    .await
    .map_err(|error| error.to_string())?
    .ok_or_else(|| t!("market.orders_open_no_grant").into_owned())
}

// Decides whether a message should drive a structure order-book fetch, and with which
// (structure_id, type_id). The authed fetch itself is threaded at the app layer because the
// db-only reducer cannot build a grant. Returns None when the region path applies or an input is
// missing (e.g. a structure picked before any item is selected).
pub fn structure_book_fetch(state: &State, message: &Message) -> Option<(i64, i64)> {
  match message {
    Message::RegionPicked(location) if location.tier == Some(LocationTier::Structure) => {
      Some((location.id, state.selected_type_id()?))
    }
    Message::ItemSelected(type_id) => Some((state.active_structure.as_ref()?.id, *type_id)),
    _ => None,
  }
}

pub fn fetch_structure_book_task(
  db: &Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  structure_id: i64,
  type_id: i64,
) -> Task<Message> {
  Task::perform(
    fetch_structure_book(db.clone(), esi, sso, structure_id, type_id),
    Message::StructureBookLoaded,
  )
}

async fn fetch_structure_book(
  db: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  structure_id: i64,
  type_id: i64,
) -> StructureBook {
  let Some(grant) = first_owned_grant(&db, &sso).await else {
    return StructureBook::Error;
  };
  let result = esi.market().structure_orders(structure_id, &grant).await;
  let mut shaped = shape_structure_response(structure_id, type_id, result);
  if let StructureBook::Loaded(book) = &mut shaped {
    label_book_locations(&db, book).await;
  }
  shaped
}

fn shape_structure_response(
  structure_id: i64,
  type_id: i64,
  result: Result<Vec<RegionOrder>, clients::Error>,
) -> StructureBook {
  match result {
    Ok(orders) => {
      let filtered = orders.into_iter().filter(|order| order.type_id == type_id).collect();
      StructureBook::Loaded(Box::new(book::build_order_book(filtered)))
    }
    Err(error) => match classify_structure_error(&error) {
      BookAccess::NoAccess => StructureBook::NoAccess,
      _ => {
        tracing::warn!(target: "pod::market", %error, structure_id, "structure order book fetch failed");
        StructureBook::Error
      }
    },
  }
}

fn classify_structure_error(error: &clients::Error) -> BookAccess {
  match error {
    clients::Error::Http(http) => access_from_status(http.status()),
    _ => BookAccess::Error,
  }
}

// A structure order-book fetch that returns 403 or 404 is a permanent access miss; every other
// status (5xx, timeouts, a statusless transport error) is transient and keeps the Error treatment.
fn access_from_status(status: Option<reqwest::StatusCode>) -> BookAccess {
  match status {
    Some(reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::NOT_FOUND) => BookAccess::NoAccess,
    _ => BookAccess::Error,
  }
}

async fn first_owned_grant(db: &Database, sso: &eve_sso::Client) -> Option<eve_sso::Grant> {
  let owner = character_repo::all_owned(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .next()?;
  match crate::sync::token::fresh_token(db, sso, owner.id(), OwnerType::Character).await {
    Ok(grant) => grant,
    Err(error) => {
      tracing::warn!(target: "pod::market", %error, "structure book: no usable token");
      None
    }
  }
}

fn filtered_for(tree: &tree::MarketTree, query: &str) -> Option<Vec<tree::FilteredGroup>> {
  if query.trim().is_empty() {
    None
  } else {
    Some(tree::filter_catalog(tree, query))
  }
}

// State-only reducer, kept free of the store so it stays synchronously testable. Side effects that
// need the database (region search, order-book fetch) are layered on by `dispatch`.
pub fn update(state: &mut State, message: Message) {
  match message {
    Message::TabSelected(tab) => state.tab = tab,
    Message::TreeLoaded(tree) => {
      state.tree = *tree;
      state.filtered = filtered_for(&state.tree, &state.filter);
    }
    Message::BookLoaded(book) => {
      state.book = Some(*book);
      state.book_access = BookAccess::Ok;
    }
    Message::BookRelabeled(book) => state.book = Some(*book),
    Message::StructureBookLoaded(result) => apply_structure_book(state, result),
    Message::OwnOrdersLoaded(orders) => state.own_orders = orders,
    Message::AlertOutbidLoaded(count) => state.alert_outbid = count,
    Message::NodeToggled(id) => {
      if !state.expanded.remove(&id) {
        state.expanded.insert(id);
      }
    }
    Message::FilterChanged(query) => {
      state.filter = query;
      state.filtered = filtered_for(&state.tree, &state.filter);
    }
    Message::ItemSelected(type_id) => {
      state.selected = Some(type_id);
      state.detail_view = DetailView::default();
      state.book_access = BookAccess::Ok;
      state.cart.reset_add_control();
    }
    Message::DefaultMarketResolved(location) => {
      // A user pick made before this async default resolves wins; only adopt the default when
      // nothing (place, structure, or region) has been selected yet.
      if state.active_location().is_none() {
        adopt_place(state, location);
      }
    }
    Message::RegionPickerToggled
    | Message::RegionPickerClosed
    | Message::RegionSearchChanged(_)
    | Message::RegionResultsLoaded(..)
    | Message::RegionPicked(_)
    | Message::RegionResolved(_) => update_region(state, message),
    Message::OrdersLoaded(_)
    | Message::OrdersScopeToggled
    | Message::OrdersScopeDismissed
    | Message::OrdersScopeSelected(_)
    | Message::OrdersSubTabSelected(_)
    | Message::LotsLoaded(_)
    | Message::LotDismissPrompted(_)
    | Message::LotDismissCancelled
    | Message::LotDismissConfirmed => update_orders(state, message),
    Message::OpenInGame {
      ..
    } => {}
    Message::MarketWindowOpened(result) => match result {
      Ok(()) => state.open_window_notice = None,
      Err(failure) => {
        state.open_window_notice = Some(OpenWindowNotice {
          at: chrono::Utc::now().timestamp(),
          character_id: failure.character_id,
          message: failure.message,
        });
      }
    },
    Message::WatchPricesLoaded(prices) => state.watch_prices = prices,
    Message::WatchesLoaded(watches) => state.watches = watches,
    Message::DetailViewSelected(view) => state.detail_view = view,
    // A range change only re-slices the already-fetched 365-day series in the view; the fetch holds
    // every day, so it drives no follow-up task.
    Message::HistoryRangeSelected(range) => state.history_range = range,
    Message::HistoryLoaded(region_id, type_id, result) => {
      apply_history_loaded(state, region_id, type_id, result);
    }
    Message::FeaturesChanged(flags) => apply_features(state, flags),
    other => watchlist::reduce(state, other),
  }
  sync_history_target(state);
}

fn apply_features(state: &mut State, flags: crate::config::FeatureFlags) {
  state.compare_enabled = flags.is_sub_enabled(crate::config::SubFeature::MarketCompare);
  if !state.compare_enabled && state.tab == Tab::Compare {
    state.tab = Tab::Browse;
  }
}

fn apply_history_loaded(
  state: &mut State,
  region_id: i64,
  type_id: i64,
  result: Result<Vec<history::HistoryPoint>, String>,
) {
  if state.history_key != Some((region_id, type_id)) {
    return;
  }
  state.history_state = match result {
    Ok(points) if points.is_empty() => HistoryFetch::Empty,
    Ok(points) => HistoryFetch::Loaded(points),
    Err(_) => HistoryFetch::Failed,
  };
}

fn apply_structure_book(state: &mut State, result: StructureBook) {
  match result {
    StructureBook::Loaded(book) => {
      state.book = Some(*book);
      state.book_access = BookAccess::Ok;
    }
    StructureBook::NoAccess => state.book_access = BookAccess::NoAccess,
    StructureBook::Error => state.book_access = BookAccess::Error,
  }
}

fn sync_history_target(state: &mut State) {
  let target = match (state.detail_view, state.active_region_id(), state.selected_type_id()) {
    (DetailView::History, Some(region_id), Some(type_id)) => Some((region_id, type_id)),
    _ => None,
  };
  if target.is_some() && target != state.history_key {
    state.history_key = target;
    state.history_state = HistoryFetch::Loading;
  }
}

fn update_region(state: &mut State, message: Message) {
  match message {
    Message::RegionPickerToggled => {
      state.region_picker_open = !state.region_picker_open;
      if !state.region_picker_open {
        state.region_search.clear();
      }
    }
    Message::RegionPickerClosed => {
      state.region_picker_open = false;
      state.region_search.clear();
    }
    Message::RegionSearchChanged(query) => {
      state.region_search.set_query(query);
    }
    Message::RegionResultsLoaded(generation, results) => {
      state.region_search.accept_results(generation, results);
    }
    Message::RegionPicked(location) => {
      state.region_picker_open = false;
      state.region_search.clear();
      // A fresh pick clears any prior structure NoAccess/Error so the incoming book (or the region
      // path) renders cleanly; the structure fetch re-sets it if the new market is inaccessible.
      state.book_access = BookAccess::Ok;
      adopt_place(state, location);
    }
    Message::RegionResolved(region) => {
      state.active_region = Some(region);
      state.active_structure = None;
    }
    _ => {}
  }
}

fn adopt_place(state: &mut State, location: LocationRef) {
  state.active_place = Some(location.clone());
  match location.tier {
    Some(LocationTier::Region) => {
      state.active_region = Some(location);
      state.active_structure = None;
    }
    Some(LocationTier::Structure) => state.active_structure = Some(location),
    _ => state.active_structure = None,
  }
}

fn update_orders(state: &mut State, message: Message) {
  match message {
    Message::OrdersLoaded(data) if data.scope == state.orders_scope => {
      state.orders = *data;
      // A fresh orders load is the natural tick to retire a stale open-window notice.
      state.open_window_notice = None;
    }
    Message::OrdersScopeToggled => {
      state.orders_picker_open = !state.orders_picker_open;
    }
    Message::OrdersScopeDismissed => state.orders_picker_open = false,
    Message::OrdersScopeSelected(scope) => {
      state.orders_picker_open = false;
      state.orders_scope = scope;
    }
    Message::OrdersSubTabSelected(sub) => state.orders_sub = sub,
    Message::LotsLoaded(cards) => state.lot_groups = cards,
    Message::LotDismissPrompted(prompt) => state.lot_dismiss = Some(*prompt),
    Message::LotDismissCancelled | Message::LotDismissConfirmed => state.lot_dismiss = None,
    _ => {}
  }
}

fn try_pane(state: &mut State, message: &Message) -> Option<Task<Message>> {
  match message {
    Message::PaneDragStart => {
      state.tree_pane.start();
      Some(Task::none())
    }
    Message::PaneDrag(x) => {
      state.tree_pane.drag_to(*x);
      Some(Task::none())
    }
    Message::PaneDragEnd => {
      state.tree_pane.end();
      Some(Task::done(Message::PaneSettled(
        MARKET_TREE_PANE_KEY,
        state.tree_pane.ratio(),
      )))
    }
    Message::PaneSettled(..) => Some(Task::none()),
    _ => None,
  }
}

// The Watchlist tab carries its own scope, independent of My Orders, so switching one tab never
// silently re-filters the other. Selecting a scope re-fetches the grid (see `dispatch`).
// App-facing entry point: applies the state reducer, then drives the database-backed follow-ups —
// the region search for a typed query, and the order-book fetch whenever an active region and a
// selected type are both present.
enum Follow {
  Book,
  DismissLot(i64, i64, bool),
  None,
  Orders,
  RemoveWatch(i64),
  ResolvePlace(i64),
  WatchPrices,
}

// A region pick fetches its book directly; a structure pick is fetched at the app layer; any other
// tier (constellation/system/station) resolves to its region first, then fetches on RegionResolved.
fn place_follow(location: &LocationRef) -> Follow {
  match location.tier {
    Some(LocationTier::Region) => Follow::Book,
    Some(LocationTier::Structure) => Follow::None,
    _ => Follow::ResolvePlace(location.id),
  }
}

fn classify_follow(state: &State, message: &Message) -> Follow {
  match message {
    Message::ItemSelected(_) | Message::RegionResolved(_) => Follow::Book,
    // The resolved default mirrors a manual pick, but only when it is actually adopted: a user pick made
    // during the async-resolve window keeps its own book (matching the reducer's guard).
    Message::DefaultMarketResolved(location) if state.active_location().is_none() => place_follow(location),
    Message::DefaultMarketResolved(_) => Follow::None,
    Message::RegionPicked(location) => place_follow(location),
    Message::TabSelected(Tab::Orders) | Message::OrdersScopeSelected(_) => Follow::Orders,
    Message::TabSelected(Tab::Watchlist) => Follow::WatchPrices,
    Message::WatchRemoved(id) => Follow::RemoveWatch(*id),
    Message::LotDismissConfirmed => match state.lot_dismiss.as_ref() {
      Some(prompt) => Follow::DismissLot(prompt.transaction_id, prompt.owner_id, prompt.is_corporation),
      None => Follow::None,
    },
    _ => Follow::None,
  }
}

fn follow_task(state: &State, follow: Follow, db: &Database) -> Task<Message> {
  match follow {
    Follow::None => Task::none(),
    Follow::Book => fetch_book_task(state, db),
    Follow::Orders => Task::batch([load_orders_task(state, db), load_lots_task(db)]),
    Follow::DismissLot(transaction_id, owner_id, is_corporation) => {
      dismiss_lot_task(db, transaction_id, owner_id, is_corporation)
    }
    Follow::WatchPrices => load_watches(db),
    Follow::RemoveWatch(id) => remove_watch_task(db, id),
    Follow::ResolvePlace(place_id) => {
      Task::perform(resolve_place_region(db.clone(), place_id), Message::RegionResolved)
    }
  }
}

fn compare_transient_task(state: &State, db: &Database) -> Task<Message> {
  match state.selected {
    Some(type_id) => compare::load_transient_task(db, type_id),
    None => Task::none(),
  }
}

pub fn dispatch(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  // Watchlist-modal messages carry their own reducer and follow-ups; peel them off here so the
  // browse/orders reducer below stays focused on the tree-and-book flow.
  let message = match watchlist::try_dispatch(state, message, db) {
    Ok(task) => return task,
    Err(message) => message,
  };

  // Compare-tab messages carry their own reducer and follow-ups (fan-out, persistence); peel them off
  // like the watchlist modal so the browse/orders reducer below stays focused on the tree-and-book flow.
  let message = match compare::try_dispatch(state, message, db) {
    Ok(task) => return task,
    Err(message) => message,
  };

  let message = match cart::try_dispatch(state, message, db) {
    Ok(task) => return task,
    Err(message) => message,
  };

  // Tree-pane drag messages mutate only the pane geometry and, on release, bubble the settled ratio
  // for persistence; peel them off before the browse/orders reducer so `update` stays free of them.
  if let Some(task) = try_pane(state, &message) {
    return task;
  }

  let follow = classify_follow(state, &message);

  let loads_compare = matches!(&message, Message::TabSelected(Tab::Compare));
  let selects_compare = matches!(&message, Message::ItemSelected(_)) && state.tab == Tab::Compare;

  let prev_history_key = state.history_key;
  update(state, message);
  let history = history_follow_task(state, prev_history_key, db);

  let base = follow_task(state, follow, db);

  let compare = if loads_compare {
    Task::batch([compare::load_pins_task(db), compare_transient_task(state, db)])
  } else if selects_compare {
    compare_transient_task(state, db)
  } else {
    Task::none()
  };

  Task::batch([base, history, compare])
}

pub fn view(state: &State) -> Element<'_, Message> {
  cart::mount(
    compare::mount(
      watchlist::mount(order_history::mount(shell::shell(state), state), state),
      state,
    ),
    state,
  )
}

pub fn cart_wants_prices(message: &Message) -> bool {
  cart::wants_prices(message)
}

pub fn cart_prices_task(
  state: &State,
  db: &Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
) -> Task<Message> {
  cart::prices_task(state, db, esi, sso)
}

fn watch_menu_escape(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
  is_escape_pressed(&event).then_some(Message::WatchMenuDismissed)
}

fn compare_menu_escape(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
  is_escape_pressed(&event).then_some(Message::CompareMenuDismissed)
}

fn cart_escape(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
  is_escape_pressed(&event).then_some(Message::CartEscapePressed)
}

fn tree_menu_escape(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
  is_escape_pressed(&event).then_some(Message::TreeMenuDismissed)
}

fn pane_drag(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
  resizable_pane::drag_event(event, Message::PaneDrag, Message::PaneDragEnd)
}

fn watch_drop_release(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
  matches!(
    event,
    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
  )
  .then_some(Message::WatchDropReleased)
}

fn compare_drop_release(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
  matches!(
    event,
    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
  )
  .then_some(Message::CompareDropReleased)
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  let mut subs = menu_subscriptions(state);
  subs.extend(drag_subscriptions(state));
  iced::Subscription::batch(subs)
}

fn menu_subscriptions(state: &State) -> Vec<iced::Subscription<Message>> {
  let mut subs = Vec::new();
  if state.watch_menu.is_some() {
    subs.push(iced::event::listen_with(watch_menu_escape));
  }
  if state.compare_menu.is_some() {
    subs.push(iced::event::listen_with(compare_menu_escape));
  }
  if state.cart.is_open() {
    subs.push(iced::event::listen_with(cart_escape));
  }
  if state.tree_menu.is_some() {
    subs.push(iced::event::listen_with(tree_menu_escape));
  }
  subs
}

fn drag_subscriptions(state: &State) -> Vec<iced::Subscription<Message>> {
  let mut subs = Vec::new();
  if state.tree_pane.is_active() {
    subs.push(iced::event::listen_with(pane_drag));
  }
  if state.dragging_watch.is_some() {
    subs.push(iced::event::listen_with(watch_drop_release));
  }
  if state.compare_dragging.is_some() {
    subs.push(iced::event::listen_with(compare_drop_release));
  }
  subs
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

#[cfg(test)]
mod tests {
  use super::*;

  mod tab {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_every_tab_through_its_id() {
      for tab in Tab::ORDER {
        assert_eq!(Tab::from_id(tab.id()), Some(tab));
      }
    }

    #[test]
    fn it_rejects_an_unknown_id() {
      assert_eq!(Tab::from_id("nope"), None);
    }
  }

  mod escape {
    use super::*;

    #[test]
    fn it_recognizes_only_an_escape_key_press() {
      use iced::keyboard;

      let escape = iced::Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(keyboard::key::Named::Escape),
        modified_key: keyboard::Key::Named(keyboard::key::Named::Escape),
        physical_key: keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::empty(),
        text: None,
        repeat: false,
      });
      assert!(is_escape_pressed(&escape));

      let other = iced::Event::Keyboard(keyboard::Event::ModifiersChanged(keyboard::Modifiers::empty()));
      assert!(!is_escape_pressed(&other));
    }
  }

  fn market_order(order_id: i64, is_buy: bool, is_corporation: bool) -> MarketOrder {
    MarketOrder {
      character_id: 90,
      duration: 90,
      escrow: if is_buy { 500.0 } else { 0.0 },
      is_buy_order: is_buy,
      is_corporation,
      issued: "2026-07-01T12:00:00Z".to_owned(),
      location_id: 60_003_760,
      order_id,
      price: 10.0,
      range: "region".to_owned(),
      region_id: 10_000_002,
      state: "open".to_owned(),
      type_id: 34,
      volume_remain: 100,
      volume_total: 200,
    }
  }

  fn corp_order(order_id: i64) -> CorporationMarketOrder {
    CorporationMarketOrder {
      corporation_id: 98_000_001,
      duration: 90,
      escrow: 500.0,
      is_buy_order: true,
      issued: "2026-07-01T12:00:00Z".to_owned(),
      location_id: 60_003_760,
      order_id,
      price: 10.0,
      range: "region".to_owned(),
      region_id: 10_000_002,
      state: "open".to_owned(),
      type_id: 34,
      volume_remain: 100,
      volume_total: 200,
    }
  }

  mod dedup_orders {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keeps_the_char_row_when_both_tables_hold_the_order() {
      let char_orders = vec![market_order(1, true, true)];
      let corp_orders = vec![
        MarketOrder {
          character_id: 98_000_001,
          ..market_order(1, true, true)
        },
        MarketOrder {
          character_id: 98_000_001,
          ..market_order(2, true, true)
        },
      ];

      let deduped = super::super::dedup_orders(char_orders, corp_orders);

      let survivor = deduped.iter().find(|order| order.order_id() == 1).unwrap();

      assert_eq!(deduped.len(), 2);
      assert_eq!(survivor.character_id(), 90);
      assert!(survivor.is_corporation());
    }

    #[test]
    fn it_retains_corp_only_orders() {
      let corp_orders = vec![MarketOrder {
        character_id: 98_000_001,
        ..market_order(7, true, true)
      }];

      let deduped = super::super::dedup_orders(Vec::new(), corp_orders);

      assert_eq!(deduped.len(), 1);
      assert_eq!(deduped[0].order_id(), 7);
      assert!(deduped[0].is_corporation());
    }
  }

  mod corp_order_as_market {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_flags_the_mapped_order_as_corporation() {
      let mapped = super::super::corp_order_as_market(&corp_order(5));

      assert_eq!(mapped.character_id(), 98_000_001);
      assert_eq!(mapped.order_id(), 5);
      assert!(mapped.is_corporation());
    }
  }

  mod sell_listed {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_remaining_value_of_sell_orders_only() {
      let orders = vec![market_order(1, false, false), market_order(2, true, false)];

      assert_eq!(super::super::sell_listed(&orders), 1_000.0);
    }
  }

  mod buy_escrow {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_escrow_of_buy_orders_only() {
      let orders = vec![market_order(1, true, false), market_order(2, false, false)];

      assert_eq!(super::super::buy_escrow(&orders), 500.0);
    }
  }

  mod telemetry_offline {
    use super::*;

    #[test]
    fn it_reports_offline_when_a_fresh_sync_says_the_owner_is_offline() {
      assert!(telemetry_offline(false, 1_000, 1_500));
    }

    #[test]
    fn it_reports_online_owners_as_available() {
      assert!(!telemetry_offline(true, 1_000, 1_500));
    }

    #[test]
    fn it_treats_stale_telemetry_as_unknown() {
      assert!(!telemetry_offline(false, 1_000, 1_000 + TELEMETRY_STALE_SECS + 1));
    }
  }

  mod classify_open_window_failure {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_a_403_to_reauthorize() {
      assert_eq!(
        classify_open_window_failure(Some(403), "forbidden"),
        OpenWindowFailureKind::Reauthorize
      );
    }

    #[test]
    fn it_maps_an_online_hint_to_offline() {
      assert_eq!(
        classify_open_window_failure(Some(520), "The character needs to be online"),
        OpenWindowFailureKind::Offline
      );
    }

    #[test]
    fn it_falls_back_to_generic() {
      assert_eq!(
        classify_open_window_failure(Some(500), "boom"),
        OpenWindowFailureKind::Generic
      );
    }
  }

  mod esi_error_message {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_unwraps_the_esi_error_field() {
      assert_eq!(esi_error_message(r#"{"error":"bad request"}"#), "bad request");
    }

    #[test]
    fn it_keeps_a_non_json_body_verbatim() {
      assert_eq!(esi_error_message("plain failure"), "plain failure");
    }

    #[test]
    fn it_falls_back_to_a_placeholder_for_an_empty_body() {
      assert_eq!(
        esi_error_message(""),
        t!("market.orders_open_reason_unknown").into_owned()
      );
    }
  }

  mod market_window_opened {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_records_a_notice_on_failure_and_clears_it_on_success() {
      let mut state = State::new();

      update(
        &mut state,
        Message::MarketWindowOpened(Err(OpenWindowFailure {
          character_id: 90,
          message: "nope".to_owned(),
        })),
      );
      assert_eq!(state.open_window_notice().map(|notice| notice.character_id), Some(90));

      update(&mut state, Message::MarketWindowOpened(Ok(())));
      assert!(state.open_window_notice().is_none());
    }
  }

  mod state {
    use super::*;

    #[test]
    fn it_defaults_to_the_browse_tab() {
      assert_eq!(State::new().active_tab(), Tab::Browse);
    }

    #[test]
    fn it_selects_a_tab_by_id() {
      let mut state = State::new();

      assert!(state.select_tab_by_id("watchlist"));
      assert_eq!(state.active_tab(), Tab::Watchlist);
    }

    #[test]
    fn it_ignores_an_unknown_tab_id() {
      let mut state = State::new();

      assert!(!state.select_tab_by_id("nope"));
      assert_eq!(state.active_tab(), Tab::Browse);
    }
  }

  mod filter_cache {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{ItemType, MarketGroup};

    fn group(id: i64, name: &str, parent: Option<i64>) -> MarketGroup {
      MarketGroup {
        description: String::new(),
        has_types: false,
        icon_id: None,
        id,
        name: name.to_owned(),
        parent_id: parent,
      }
    }

    fn item(id: i64, name: &str, market_group_id: Option<i64>) -> ItemType {
      ItemType {
        capacity: None,
        description: None,
        dogma_attributes: "[]".to_owned(),
        group_id: 0,
        icon_id: None,
        id,
        market_group_id,
        name: name.to_owned(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        volume: None,
      }
    }

    fn sample_tree() -> tree::MarketTree {
      let groups = vec![group(1, "Ships", None), group(2, "Frigates", Some(1))];
      let items = vec![item(587, "Rifter", Some(2)), item(588, "Punisher", Some(2))];
      tree::build_market_tree(&groups, &items)
    }

    #[test]
    fn it_caches_the_filtered_groups_on_a_filter_change() {
      let mut state = State::new();
      update(&mut state, Message::TreeLoaded(Box::new(sample_tree())));

      update(&mut state, Message::FilterChanged("rifter".to_owned()));

      let groups = state.filtered_catalog().expect("an active filter caches its groups");
      assert_eq!(groups.len(), 1);
      assert_eq!(groups[0].name, "Frigates");
      assert_eq!(groups[0].leaves.len(), 1);
    }

    #[test]
    fn it_clears_the_cache_when_the_filter_empties() {
      let mut state = State::new();
      update(&mut state, Message::TreeLoaded(Box::new(sample_tree())));
      update(&mut state, Message::FilterChanged("rifter".to_owned()));

      update(&mut state, Message::FilterChanged(String::new()));

      assert!(state.filtered_catalog().is_none());
    }

    #[test]
    fn it_rebuilds_the_cache_when_the_tree_loads_under_an_active_filter() {
      let mut state = State::new();
      update(&mut state, Message::FilterChanged("rifter".to_owned()));
      assert!(state.filtered_catalog().is_some_and(<[_]>::is_empty));

      update(&mut state, Message::TreeLoaded(Box::new(sample_tree())));

      assert!(state.filtered_catalog().is_some_and(|groups| !groups.is_empty()));
    }
  }

  mod tree_pane {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::shell::window_state::UiState;

    #[test]
    fn it_defaults_the_tree_pane_width_when_the_store_is_empty() {
      let state = State::new().with_restored_panes(&UiState::default());

      assert_eq!(state.tree_pane_width(), MARKET_TREE_PANE_DEFAULT);
    }

    #[test]
    fn it_restores_the_tree_pane_width_from_the_keyed_store() {
      let mut ui = UiState::default();
      ui.panes.insert(MARKET_TREE_PANE_KEY.to_owned(), 540.0);

      let state = State::new().with_restored_panes(&ui);

      assert_eq!(state.tree_pane_width(), 540.0);
    }

    #[tokio::test]
    async fn it_resizes_the_tree_pane_during_a_drag_and_bubbles_the_settled_width() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = dispatch(&mut state, Message::PaneDragStart, &db);
      let _ = dispatch(&mut state, Message::PaneDrag(500.0), &db);
      let _ = dispatch(&mut state, Message::PaneDrag(560.0), &db);
      assert_eq!(state.tree_pane_width(), MARKET_TREE_PANE_DEFAULT + 60.0);
      assert!(state.tree_pane.is_active());

      let _task = dispatch(&mut state, Message::PaneDragEnd, &db);
      assert!(!state.tree_pane.is_active());
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    fn region(id: i64) -> LocationRef {
      region_location(id, "The Forge".to_owned())
    }

    fn structure(id: i64) -> LocationRef {
      LocationRef {
        context: None,
        id,
        name: "Jita Trade Hub".to_owned(),
        security_status: None,
        tier: Some(LocationTier::Structure),
      }
    }

    #[test]
    fn it_switches_the_active_tab() {
      let mut state = State::new();

      update(&mut state, Message::TabSelected(Tab::Orders));

      assert_eq!(state.active_tab(), Tab::Orders);
    }

    #[test]
    fn it_toggles_a_node_open_and_closed() {
      let mut state = State::new();

      update(&mut state, Message::NodeToggled(7));
      assert!(state.is_expanded(7));

      update(&mut state, Message::NodeToggled(7));
      assert!(!state.is_expanded(7));
    }

    #[test]
    fn it_stores_the_filter_query() {
      let mut state = State::new();

      update(&mut state, Message::FilterChanged("rifter".to_owned()));

      assert_eq!(state.filter(), "rifter");
    }

    #[test]
    fn it_selects_an_item_by_type_id() {
      let mut state = State::new();

      update(&mut state, Message::ItemSelected(587));

      assert_eq!(state.selected_type_id(), Some(587));
    }

    #[test]
    fn it_defaults_the_detail_view_to_orders() {
      assert_eq!(State::new().detail_view(), DetailView::Orders);
    }

    #[test]
    fn it_switches_the_detail_view() {
      let mut state = State::new();

      update(&mut state, Message::DetailViewSelected(DetailView::History));

      assert_eq!(state.detail_view(), DetailView::History);
    }

    #[test]
    fn it_resets_the_detail_view_to_orders_when_a_new_item_is_selected() {
      let mut state = State::new();
      update(&mut state, Message::DetailViewSelected(DetailView::History));

      update(&mut state, Message::ItemSelected(587));

      assert_eq!(state.detail_view(), DetailView::Orders);
    }

    #[test]
    fn it_adopts_the_resolved_default_region() {
      let mut state = State::new();

      update(&mut state, Message::DefaultMarketResolved(region(THE_FORGE_REGION_ID)));

      assert_eq!(state.active_region_id(), Some(THE_FORGE_REGION_ID));
      assert_eq!(
        state.active_location().map(|location| location.id),
        Some(THE_FORGE_REGION_ID)
      );
    }

    #[test]
    fn it_adopts_a_station_default_and_filters_to_the_station() {
      let mut state = State::new();
      let station = place_ref(60_003_760, "Jita IV-4".to_owned(), LocationTier::Station);

      update(&mut state, Message::DefaultMarketResolved(station));

      assert_eq!(state.active_location().map(|location| location.id), Some(60_003_760));
      assert!(matches!(place_filter(&state), Some(PlaceFilter::Station(60_003_760))));
      assert_eq!(state.active_region_id(), None);
      assert!(state.active_structure.is_none());
    }

    #[test]
    fn it_adopts_a_system_default_and_filters_to_the_system() {
      let mut state = State::new();
      let system = place_ref(30_000_142, "Jita".to_owned(), LocationTier::System);

      update(&mut state, Message::DefaultMarketResolved(system));

      assert_eq!(state.active_location().map(|location| location.id), Some(30_000_142));
      assert!(matches!(place_filter(&state), Some(PlaceFilter::System(30_000_142))));
      assert!(state.active_structure.is_none());
    }

    #[test]
    fn it_adopts_a_constellation_default_as_the_active_place() {
      let mut state = State::new();
      let constellation = place_ref(20_000_020, "Kimotoro".to_owned(), LocationTier::Constellation);

      update(&mut state, Message::DefaultMarketResolved(constellation));

      assert_eq!(state.active_location().map(|location| location.id), Some(20_000_020));
      assert!(place_filter(&state).is_none());
      assert!(state.active_structure.is_none());
    }

    #[test]
    fn it_adopts_a_structure_default_without_a_region_fallback() {
      let mut state = State::new();

      update(&mut state, Message::DefaultMarketResolved(structure(1_035_000_000_001)));

      assert_eq!(
        state.active_structure.as_ref().map(|location| location.id),
        Some(1_035_000_000_001)
      );
      assert_eq!(
        state.active_location().map(|location| location.id),
        Some(1_035_000_000_001)
      );
      assert_eq!(state.active_region_id(), None);
    }

    #[test]
    fn it_keeps_a_user_region_over_a_late_default() {
      let mut state = State::new();

      update(&mut state, Message::RegionPicked(region(10_000_043)));
      update(&mut state, Message::DefaultMarketResolved(region(THE_FORGE_REGION_ID)));

      assert_eq!(state.active_region_id(), Some(10_000_043));
    }

    #[test]
    fn it_keeps_a_user_station_pick_over_a_late_default() {
      let mut state = State::new();
      let station = place_ref(60_003_760, "Jita IV-4".to_owned(), LocationTier::Station);

      update(&mut state, Message::RegionPicked(station));
      update(&mut state, Message::DefaultMarketResolved(region(THE_FORGE_REGION_ID)));

      assert_eq!(state.active_location().map(|location| location.id), Some(60_003_760));
      assert_eq!(state.active_region_id(), None);
    }

    #[test]
    fn it_keeps_a_user_structure_pick_over_a_late_default() {
      let mut state = State::new();

      update(&mut state, Message::RegionPicked(structure(1_035_000_000_001)));
      update(&mut state, Message::DefaultMarketResolved(region(THE_FORGE_REGION_ID)));

      assert_eq!(
        state.active_structure.as_ref().map(|location| location.id),
        Some(1_035_000_000_001)
      );
      assert_eq!(state.active_region_id(), None);
    }

    #[test]
    fn it_sets_the_active_region_when_a_region_is_picked() {
      let mut state = State::new();
      update(&mut state, Message::RegionPickerToggled);

      update(&mut state, Message::RegionPicked(region(10_000_043)));

      assert_eq!(state.active_region_id(), Some(10_000_043));
      assert!(!state.region_picker_open());
    }

    #[test]
    fn it_selects_a_structure_market_on_a_structure_pick() {
      let mut state = State::new();

      update(&mut state, Message::RegionPicked(structure(1_035_000_000_001)));

      assert_eq!(
        state.active_structure.as_ref().map(|location| location.id),
        Some(1_035_000_000_001)
      );
      assert_eq!(
        state.active_location().map(|location| location.id),
        Some(1_035_000_000_001)
      );
      assert_eq!(state.active_region_id(), None);
      assert!(!state.region_picker_open());
    }

    #[test]
    fn it_clears_the_active_structure_when_a_region_is_picked() {
      let mut state = State::new();
      update(&mut state, Message::RegionPicked(structure(1_035_000_000_001)));

      update(&mut state, Message::RegionPicked(region(10_000_043)));

      assert!(state.active_structure.is_none());
      assert_eq!(state.active_location().map(|location| location.id), Some(10_000_043));
    }

    #[test]
    fn it_reports_a_structure_book_fetch_for_a_structure_pick_with_a_selected_item() {
      let mut state = State::new();
      update(&mut state, Message::ItemSelected(34));

      let message = Message::RegionPicked(structure(1_035_000_000_001));

      assert_eq!(structure_book_fetch(&state, &message), Some((1_035_000_000_001, 34)));
    }

    #[test]
    fn it_skips_a_structure_book_fetch_without_a_selected_item() {
      let state = State::new();

      let message = Message::RegionPicked(structure(1_035_000_000_001));

      assert_eq!(structure_book_fetch(&state, &message), None);
    }

    #[test]
    fn it_refetches_the_structure_book_when_the_item_changes() {
      let mut state = State::new();
      update(&mut state, Message::RegionPicked(structure(1_035_000_000_001)));

      let fetch = structure_book_fetch(&state, &Message::ItemSelected(587));

      assert_eq!(fetch, Some((1_035_000_000_001, 587)));
    }

    #[test]
    fn it_does_not_report_a_structure_fetch_for_an_item_change_without_a_structure() {
      let state = State::new();

      assert_eq!(structure_book_fetch(&state, &Message::ItemSelected(587)), None);
    }

    #[test]
    fn it_toggles_and_closes_the_region_picker() {
      let mut state = State::new();

      update(&mut state, Message::RegionPickerToggled);
      assert!(state.region_picker_open());

      update(&mut state, Message::RegionPickerClosed);
      assert!(!state.region_picker_open());
    }

    #[test]
    fn it_accepts_region_results_for_the_current_generation() {
      let mut state = State::new();

      update(&mut state, Message::RegionSearchChanged("forge".to_owned()));
      let generation = state.region_search.generation();
      update(
        &mut state,
        Message::RegionResultsLoaded(generation, vec![region(THE_FORGE_REGION_ID)]),
      );

      assert_eq!(state.region_results(), &[region(THE_FORGE_REGION_ID)]);
    }
  }

  mod structure_access {
    use pretty_assertions::assert_eq;

    use super::*;

    fn structure(id: i64) -> LocationRef {
      LocationRef {
        context: None,
        id,
        name: "Jita Trade Hub".to_owned(),
        security_status: None,
        tier: Some(LocationTier::Structure),
      }
    }

    fn status(code: u16) -> Option<reqwest::StatusCode> {
      Some(reqwest::StatusCode::from_u16(code).unwrap())
    }

    #[test]
    fn it_maps_a_403_to_no_access() {
      assert_eq!(access_from_status(status(403)), BookAccess::NoAccess);
    }

    #[test]
    fn it_maps_a_404_to_no_access() {
      assert_eq!(access_from_status(status(404)), BookAccess::NoAccess);
    }

    #[test]
    fn it_maps_a_503_to_a_transient_error() {
      assert_eq!(access_from_status(status(503)), BookAccess::Error);
    }

    #[test]
    fn it_maps_a_statusless_failure_to_a_transient_error() {
      assert_eq!(access_from_status(None), BookAccess::Error);
    }

    #[test]
    fn it_defaults_to_ok_access() {
      assert_eq!(State::new().book_access(), BookAccess::Ok);
    }

    #[test]
    fn it_stores_the_no_access_state_from_a_structure_fetch() {
      let mut state = State::new();

      update(&mut state, Message::StructureBookLoaded(StructureBook::NoAccess));

      assert_eq!(state.book_access(), BookAccess::NoAccess);
    }

    #[test]
    fn it_stores_the_transient_error_state_from_a_structure_fetch() {
      let mut state = State::new();

      update(&mut state, Message::StructureBookLoaded(StructureBook::Error));

      assert_eq!(state.book_access(), BookAccess::Error);
    }

    #[test]
    fn it_stores_a_loaded_structure_book_and_clears_no_access() {
      let mut state = State::new();
      update(&mut state, Message::StructureBookLoaded(StructureBook::NoAccess));

      update(
        &mut state,
        Message::StructureBookLoaded(StructureBook::Loaded(Box::default())),
      );

      assert_eq!(state.book_access(), BookAccess::Ok);
      assert!(state.book().is_some());
    }

    #[test]
    fn it_preserves_a_prior_book_when_a_structure_returns_no_access() {
      let mut state = State::new();
      update(&mut state, Message::BookLoaded(Box::default()));

      update(&mut state, Message::StructureBookLoaded(StructureBook::NoAccess));

      assert_eq!(state.book_access(), BookAccess::NoAccess);
      assert!(state.book().is_some());
    }

    #[test]
    fn it_clears_no_access_when_a_region_is_picked() {
      let mut state = State::new();
      update(&mut state, Message::StructureBookLoaded(StructureBook::NoAccess));

      update(
        &mut state,
        Message::RegionPicked(region_location(10_000_002, "The Forge".to_owned())),
      );

      assert_eq!(state.book_access(), BookAccess::Ok);
    }

    #[test]
    fn it_clears_no_access_when_an_accessible_structure_is_picked() {
      let mut state = State::new();
      update(&mut state, Message::StructureBookLoaded(StructureBook::NoAccess));

      update(&mut state, Message::RegionPicked(structure(1_035_000_000_001)));

      assert_eq!(state.book_access(), BookAccess::Ok);
    }

    #[test]
    fn it_clears_no_access_when_a_new_item_is_selected() {
      let mut state = State::new();
      update(&mut state, Message::StructureBookLoaded(StructureBook::NoAccess));

      update(&mut state, Message::ItemSelected(587));

      assert_eq!(state.book_access(), BookAccess::Ok);
    }

    #[test]
    fn it_clears_no_access_when_a_region_book_loads() {
      let mut state = State::new();
      update(&mut state, Message::StructureBookLoaded(StructureBook::NoAccess));

      update(&mut state, Message::BookLoaded(Box::default()));

      assert_eq!(state.book_access(), BookAccess::Ok);
    }
  }

  mod dispatch {
    use super::*;
    use crate::store;

    #[tokio::test]
    async fn it_applies_the_state_reducer_for_a_pure_message() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = dispatch(&mut state, Message::TabSelected(Tab::Orders), &db);

      assert_eq!(state.active_tab(), Tab::Orders);
    }

    #[tokio::test]
    async fn it_launches_a_follow_up_for_every_trigger() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = dispatch(&mut state, Message::ItemSelected(34), &db);
      let _ = dispatch(&mut state, Message::TabSelected(Tab::Watchlist), &db);
      let _ = dispatch(&mut state, Message::WatchRemoved(1), &db);
      let _ = dispatch(&mut state, Message::RegionSearchChanged("forge".to_owned()), &db);

      assert_eq!(state.region_query(), "forge");
    }

    #[tokio::test]
    async fn it_opens_the_picker_and_stores_a_short_query_without_searching() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = dispatch(&mut state, Message::RegionSearchChanged("fo".to_owned()), &db);

      assert_eq!(state.region_query(), "fo");
      assert!(!state.region_searching());
    }

    #[tokio::test]
    async fn it_sets_the_active_region_on_a_region_pick() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = dispatch(
        &mut state,
        Message::RegionPicked(region_location(10_000_043, "Domain".to_owned())),
        &db,
      );

      assert_eq!(state.active_region_id(), Some(10_000_043));
    }
  }

  mod history {
    use super::*;

    fn point() -> super::super::history::HistoryPoint {
      super::super::history::HistoryPoint {
        date: chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
        median: 5.0,
        high: 6.0,
        low: 4.0,
        volume: 12,
        orders: 3,
      }
    }

    fn armed_state() -> State {
      let mut state = State::new();
      update(
        &mut state,
        Message::DefaultMarketResolved(region_location(THE_FORGE_REGION_ID, "The Forge".to_owned())),
      );
      update(&mut state, Message::ItemSelected(587));
      update(&mut state, Message::DetailViewSelected(DetailView::History));
      state
    }

    #[test]
    fn it_arms_a_loading_fetch_when_the_history_view_opens() {
      let state = armed_state();

      assert_eq!(state.history_key, Some((THE_FORGE_REGION_ID, 587)));
      assert!(matches!(state.history_state(), HistoryFetch::Loading));
    }

    #[test]
    fn it_defaults_the_range_to_three_months() {
      assert_eq!(State::new().history_range(), super::super::history::Range::ThreeMonths);
    }

    #[test]
    fn it_selects_a_range_without_disturbing_the_loaded_series() {
      let mut state = armed_state();
      update(
        &mut state,
        Message::HistoryLoaded(THE_FORGE_REGION_ID, 587, Ok(vec![point()])),
      );
      let key = state.history_key;

      update(
        &mut state,
        Message::HistoryRangeSelected(super::super::history::Range::OneYear),
      );

      assert_eq!(state.history_range(), super::super::history::Range::OneYear);
      assert_eq!(state.history_key, key);
      assert!(matches!(state.history_state(), HistoryFetch::Loaded(_)));
    }

    #[test]
    fn it_stores_a_matching_history_response() {
      let mut state = armed_state();

      update(
        &mut state,
        Message::HistoryLoaded(THE_FORGE_REGION_ID, 587, Ok(vec![point()])),
      );

      assert!(matches!(state.history_state(), HistoryFetch::Loaded(points) if points.len() == 1));
    }

    #[test]
    fn it_marks_an_empty_history() {
      let mut state = armed_state();

      update(
        &mut state,
        Message::HistoryLoaded(THE_FORGE_REGION_ID, 587, Ok(Vec::new())),
      );

      assert!(matches!(state.history_state(), HistoryFetch::Empty));
    }

    #[test]
    fn it_marks_a_failed_history() {
      let mut state = armed_state();

      update(
        &mut state,
        Message::HistoryLoaded(THE_FORGE_REGION_ID, 587, Err("boom".to_owned())),
      );

      assert!(matches!(state.history_state(), HistoryFetch::Failed));
    }

    #[test]
    fn it_discards_a_stale_response_for_a_since_changed_selection() {
      let mut state = armed_state();

      update(
        &mut state,
        Message::HistoryLoaded(THE_FORGE_REGION_ID, 999, Ok(vec![point()])),
      );

      assert!(matches!(state.history_state(), HistoryFetch::Loading));
    }

    #[test]
    fn it_rearms_loading_when_the_region_changes_under_the_history_view() {
      let mut state = armed_state();
      update(
        &mut state,
        Message::HistoryLoaded(THE_FORGE_REGION_ID, 587, Ok(vec![point()])),
      );

      update(
        &mut state,
        Message::RegionPicked(region_location(10_000_043, "Domain".to_owned())),
      );

      assert_eq!(state.history_key, Some((10_000_043, 587)));
      assert!(matches!(state.history_state(), HistoryFetch::Loading));
    }
  }

  mod region_resolution {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{Constellation, SolarSystem, Station},
    };

    async fn seed_regions(db: &Database) {
      sqlx::query("INSERT INTO regions (id, name) VALUES (10000002, 'The Forge'), (10000043, 'Domain')")
        .execute(db.writer())
        .await
        .unwrap();
    }

    async fn seed_geo_chain(db: &Database) {
      seed_regions(db).await;
      sde::upsert_constellation(
        db,
        &Constellation {
          id: 20_000_020,
          name: "Kimotoro".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          region_id: THE_FORGE_REGION_ID,
        },
      )
      .await
      .unwrap();
      sde::upsert_solar_system(
        db,
        &SolarSystem {
          constellation_id: 20_000_020,
          id: 30_000_142,
          name: "Jita".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          security_class: None,
          security_status: 0.9,
          star_id: None,
        },
      )
      .await
      .unwrap();
      sqlx::query("INSERT INTO item_categories (id, name, published) VALUES (6, 'Ship', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("INSERT INTO item_groups (id, category_id, name, published) VALUES (25, 6, 'Frigate', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT INTO item_types (id, group_id, description, name, published) VALUES (54678, 25, '', 'Station Type', 1)",
      )
      .execute(db.writer())
      .await
      .unwrap();
      sde::upsert_station(
        db,
        &Station {
          id: 60_003_760,
          max_dockable_ship_volume: 0.0,
          name: "Jita IV - Moon 4 - CNAP".to_owned(),
          office_rental_cost: 0.0,
          owner: None,
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          race_id: None,
          reprocessing_efficiency: 0.0,
          reprocessing_stations_take: 0.0,
          services: String::new(),
          system_id: 30_000_142,
          type_id: 54_678,
        },
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_resolves_a_constellation_to_its_region() {
      let db = store::open_test().await.unwrap();
      seed_geo_chain(&db).await;

      assert_eq!(region_of(&db, 20_000_020).await, Some(THE_FORGE_REGION_ID));
    }

    #[tokio::test]
    async fn it_resolves_a_system_to_its_region() {
      let db = store::open_test().await.unwrap();
      seed_geo_chain(&db).await;

      assert_eq!(region_of(&db, 30_000_142).await, Some(THE_FORGE_REGION_ID));
    }

    #[tokio::test]
    async fn it_resolves_a_station_to_its_region() {
      let db = store::open_test().await.unwrap();
      seed_geo_chain(&db).await;

      assert_eq!(region_of(&db, 60_003_760).await, Some(THE_FORGE_REGION_ID));
    }

    #[tokio::test]
    async fn it_resolves_no_region_for_a_structure() {
      let db = store::open_test().await.unwrap();
      seed_geo_chain(&db).await;

      assert_eq!(region_of(&db, 1_035_000_000_001).await, None);
    }

    #[tokio::test]
    async fn it_resolves_no_region_for_an_unseeded_constellation() {
      let db = store::open_test().await.unwrap();

      assert_eq!(region_of(&db, 20_000_020).await, None);
    }

    #[test]
    fn it_routes_the_browse_query_to_a_browse_location_search() {
      let field = location_search_field(&Message::RegionSearchChanged("forge".to_owned()));

      assert!(matches!(field, Some(LocationSearchField::Browse(query)) if query == "forge"));
    }

    #[test]
    fn it_routes_the_watch_query_to_a_watch_location_search() {
      let field = location_search_field(&Message::WatchRegionSearchChanged("domain".to_owned()));

      assert!(matches!(field, Some(LocationSearchField::Watch(query)) if query == "domain"));
    }

    #[test]
    fn it_ignores_messages_that_are_not_location_searches() {
      assert!(location_search_field(&Message::ItemSelected(34)).is_none());
    }

    #[tokio::test]
    async fn it_labels_book_rows_falling_back_when_the_location_is_unknown() {
      let db = store::open_test().await.unwrap();
      let mut book = book::build_order_book(vec![RegionOrder {
        location_id: 60_003_760,
        is_buy_order: false,
        price: 5.0,
        ..Default::default()
      }]);

      label_book_locations(&db, &mut book).await;

      assert!(!book.sell[0].location_label.is_empty());
    }

    #[test]
    fn it_collects_only_structure_tier_locations_from_the_book() {
      let book = book::build_order_book(vec![
        RegionOrder {
          location_id: 60_003_760,
          ..Default::default()
        },
        RegionOrder {
          location_id: 1_042_509_032_148,
          ..Default::default()
        },
      ]);

      assert_eq!(structure_ids(&book), vec![1_042_509_032_148]);
    }

    #[test]
    fn it_filters_the_region_book_to_a_picked_station_or_system() {
      let orders = || {
        vec![
          RegionOrder {
            location_id: 60_003_760,
            system_id: 30_000_142,
            ..Default::default()
          },
          RegionOrder {
            location_id: 60_003_761,
            system_id: 30_000_144,
            ..Default::default()
          },
        ]
      };

      assert_eq!(apply_place_filter(orders(), None).len(), 2);
      assert_eq!(
        apply_place_filter(orders(), Some(PlaceFilter::Station(60_003_760))).len(),
        1
      );
      assert_eq!(
        apply_place_filter(orders(), Some(PlaceFilter::System(30_000_142))).len(),
        1
      );
    }

    #[test]
    fn it_shows_the_picked_place_and_resolves_its_region_for_a_station() {
      let mut state = State::new();
      let station = place_ref(60_003_760, "Jita IV-4".to_owned(), LocationTier::Station);
      update(&mut state, Message::RegionPicked(station));
      assert_eq!(state.active_location().map(|location| location.id), Some(60_003_760));
      assert!(state.active_structure.is_none());

      update(
        &mut state,
        Message::RegionResolved(region_location(THE_FORGE_REGION_ID, "The Forge".to_owned())),
      );
      assert_eq!(state.active_region_id(), Some(THE_FORGE_REGION_ID));
    }

    #[tokio::test]
    async fn it_resolves_a_region_default_to_itself() {
      let db = store::open_test().await.unwrap();
      seed_regions(&db).await;

      let region_id = region_of(&db, THE_FORGE_REGION_ID).await;

      assert_eq!(region_id, Some(THE_FORGE_REGION_ID));
    }

    fn resolver_clients(db: &Database) -> (Arc<esi::Client>, Arc<eve_sso::Client>) {
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), "http://localhost"));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      (esi, sso)
    }

    #[tokio::test]
    async fn it_falls_back_to_the_forge_for_an_unset_default() {
      let db = store::open_test().await.unwrap();
      seed_regions(&db).await;
      let (esi, sso) = resolver_clients(&db);

      let resolved = resolve_default_market(db.clone(), esi, sso).await;

      assert_eq!(resolved.id, THE_FORGE_REGION_ID);
      assert_eq!(resolved.tier, Some(LocationTier::Region));
    }

    #[tokio::test]
    async fn it_resolves_a_station_default_at_its_full_tier() {
      let db = store::open_test().await.unwrap();
      market_repo::set_default_market(&db, 60_003_760).await.unwrap();
      let (esi, sso) = resolver_clients(&db);

      let resolved = resolve_default_market(db.clone(), esi, sso).await;

      assert_eq!(resolved.id, 60_003_760);
      assert_eq!(resolved.tier, Some(LocationTier::Station));
    }

    #[tokio::test]
    async fn it_resolves_a_structure_default_at_its_full_tier() {
      let db = store::open_test().await.unwrap();
      market_repo::set_default_market(&db, 1_035_000_000_001).await.unwrap();
      let (esi, sso) = resolver_clients(&db);

      let resolved = resolve_default_market(db.clone(), esi, sso).await;

      assert_eq!(resolved.id, 1_035_000_000_001);
      assert_eq!(resolved.tier, Some(LocationTier::Structure));
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_each_tab() {
      for tab in Tab::ORDER {
        let mut state = State::new();
        state.tab = tab;
        let _el: Element<'_, Message> = view(&state);
      }
    }
  }

  mod structure_book {
    use super::*;

    #[test]
    fn it_filters_the_structure_book_to_the_selected_type() {
      let orders = vec![
        RegionOrder {
          type_id: 34,
          is_buy_order: false,
          price: 5.0,
          ..Default::default()
        },
        RegionOrder {
          type_id: 35,
          is_buy_order: false,
          price: 9.0,
          ..Default::default()
        },
      ];

      match shape_structure_response(1, 34, Ok(orders)) {
        StructureBook::Loaded(book) => assert_eq!(book.sell.len(), 1),
        other => panic!("expected a loaded structure book, got {other:?}"),
      }
    }
  }

  mod compare_fanout {
    use pretty_assertions::assert_eq;

    use super::*;

    fn compare_block(id: compare::BlockId, type_id: i64, places: Vec<LocationRef>) -> compare::CompareBlock {
      let columns = places
        .into_iter()
        .map(|place| compare::CompareColumn {
          access: BookAccess::Ok,
          book: None,
          place,
          row: None,
        })
        .collect();
      compare::CompareBlock {
        columns,
        id,
        type_id,
      }
    }

    #[test]
    fn it_maps_each_tier_to_its_region_book_filter() {
      assert!(matches!(
        compare_place_filter(&place_ref(60_003_760, "Jita IV-4".to_owned(), LocationTier::Station)),
        Some(PlaceFilter::Station(60_003_760))
      ));
      assert!(matches!(
        compare_place_filter(&place_ref(30_000_142, "Jita".to_owned(), LocationTier::System)),
        Some(PlaceFilter::System(30_000_142))
      ));
      assert!(compare_place_filter(&region_location(10_000_002, "The Forge".to_owned())).is_none());
    }

    #[test]
    fn it_routes_only_structure_columns_to_the_authed_fetch() {
      let state = State::new();
      let block = compare_block(
        compare::BlockId::Transient,
        34,
        vec![
          place_ref(60_003_760, "Jita IV-4".to_owned(), LocationTier::Station),
          place_ref(1_035_000_000_001, "Trade Hub".to_owned(), LocationTier::Structure),
        ],
      );

      let fetches = compare_structure_fetches(&state, &Message::CompareTransientLoaded(Box::new(block)));

      assert_eq!(fetches, vec![(1_035_000_000_001, 34)]);
    }

    #[test]
    fn it_dedups_the_structure_fan_out_across_loaded_pins() {
      let state = State::new();
      let places = || {
        vec![place_ref(
          1_035_000_000_001,
          "Trade Hub".to_owned(),
          LocationTier::Structure,
        )]
      };
      let blocks = vec![
        compare_block(compare::BlockId::Pin(1), 34, places()),
        compare_block(compare::BlockId::Pin(2), 34, places()),
        compare_block(compare::BlockId::Pin(3), 35, places()),
      ];

      let fetches = compare_structure_fetches(&state, &Message::ComparePinsLoaded(blocks));

      assert_eq!(fetches, vec![(1_035_000_000_001, 34), (1_035_000_000_001, 35)]);
    }

    #[test]
    fn it_keys_a_picked_structure_off_the_open_blocks_item() {
      let mut state = State::new();
      state.compare_transient = Some(compare_block(
        compare::BlockId::Transient,
        34,
        vec![place_ref(60_003_760, "Jita IV-4".to_owned(), LocationTier::Station)],
      ));
      state.compare_add_target = Some(compare::BlockId::Transient);

      let picked = place_ref(1_035_000_000_001, "Trade Hub".to_owned(), LocationTier::Structure);
      let fetches = compare_structure_fetches(&state, &Message::CompareMarketPicked(picked));

      assert_eq!(fetches, vec![(1_035_000_000_001, 34)]);
    }
  }

  mod watch_pricing {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path, query_param},
    };

    use super::*;
    use crate::{
      clients::http,
      store::{
        self,
        model::{
          Alliance, Bloodline, Character, Constellation, Corporation, Gender, NewWatch, Race, Region, SolarSystem,
        },
        repo::{market_watchlist, sde},
      },
    };

    const REGION: i64 = 10_000_002;
    const STATION: i64 = 60_003_760;
    const TYPE: i64 = 34;

    fn order(location_id: i64, system_id: i64, price: f64, is_buy_order: bool) -> RegionOrder {
      RegionOrder {
        is_buy_order,
        location_id,
        price,
        system_id,
        type_id: TYPE,
        ..Default::default()
      }
    }

    fn order_json(location_id: i64, system_id: i64, price: f64, is_buy_order: bool) -> serde_json::Value {
      serde_json::json!({
        "is_buy_order": is_buy_order,
        "location_id": location_id,
        "price": price,
        "system_id": system_id,
        "type_id": TYPE,
      })
    }

    fn scope_of(tier: LocationTier, scope_id: i64, region_id: Option<i64>) -> WatchScope {
      WatchScope {
        region_id,
        scope_id,
        tier,
        type_id: TYPE,
      }
    }

    fn watch_row(location_id: Option<i64>, location_tier: Option<&str>, region_id: Option<i64>) -> MarketWatch {
      MarketWatch {
        location_id,
        location_tier: location_tier.map(str::to_owned),
        region_id,
        type_id: TYPE,
        ..MarketWatch::default()
      }
    }

    fn make_solar_system(id: i64, constellation_id: i64) -> SolarSystem {
      SolarSystem {
        constellation_id,
        id,
        name: format!("System {id}"),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        security_class: None,
        security_status: 0.9,
        star_id: None,
      }
    }

    async fn make_clients(base_url: &str) -> (Database, Arc<esi::Client>, Arc<eve_sso::Client>) {
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), base_url));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      (db, esi, sso)
    }

    async fn create_watch(db: &Database, location_id: Option<i64>, location_tier: Option<&str>) {
      let new = NewWatch {
        direction: WatchDirection::Buy,
        location_id,
        location_tier: location_tier.map(str::to_owned),
        region_id: Some(REGION),
        target_price: Some(1.0),
        type_id: TYPE,
      };
      market_watchlist::create(db, &new).await.unwrap();
    }

    async fn mount_region_orders(server: &MockServer) {
      Mock::given(method("GET"))
        .and(path(format!("/markets/{REGION}/orders/")))
        .and(query_param("order_type", "sell"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(vec![
              order_json(STATION, 30_000_142, 8.0, false),
              order_json(60_000_001, 30_000_005, 5.0, false),
            ]),
        )
        .mount(server)
        .await;
      Mock::given(method("GET"))
        .and(path(format!("/markets/{REGION}/orders/")))
        .and(query_param("order_type", "buy"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(vec![
              order_json(STATION, 30_000_142, 9.0, true),
              order_json(60_000_001, 30_000_005, 12.0, true),
            ]),
        )
        .mount(server)
        .await;
    }

    async fn seed_owner(db: &Database) {
      let corp_id = 98_000_001;
      let alliance_id = 99_000_001;
      let id = 90_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character_repo::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    #[test]
    fn it_back_fills_a_legacy_region_watch_to_a_region_scope() {
      let scope =
        watch_scope(&watch_row(Some(REGION), Some("region"), Some(REGION))).expect("a region watch resolves a scope");

      assert_eq!(scope.tier, LocationTier::Region);
      assert_eq!(scope.scope_id, REGION);
      assert_eq!(scope.region_id, Some(REGION));
    }

    #[test]
    fn it_infers_a_watch_scope_when_the_tier_column_is_null() {
      let scope =
        watch_scope(&watch_row(Some(STATION), None, Some(REGION))).expect("a scope is inferred from the id range");

      assert_eq!(scope.tier, LocationTier::Station);
      assert_eq!(scope.scope_id, STATION);
    }

    #[test]
    fn it_keeps_a_structure_watch_scope() {
      let scope = watch_scope(&watch_row(Some(1_035_000_000_001), Some("structure"), Some(REGION)))
        .expect("a structure watch resolves a scope");

      assert_eq!(scope.tier, LocationTier::Structure);
      assert_eq!(scope.scope_id, 1_035_000_000_001);
    }

    #[tokio::test]
    async fn it_filters_orders_to_a_station() {
      let db = store::open_test().await.unwrap();
      let orders = vec![
        order(STATION, 30_000_142, 8.0, false),
        order(60_000_001, 30_000_005, 5.0, false),
      ];

      let filtered = filter_orders_to_scope(&db, &scope_of(LocationTier::Station, STATION, Some(REGION)), orders).await;

      assert_eq!(filtered.len(), 1);
      assert_eq!(filtered[0].price, 8.0);
    }

    #[tokio::test]
    async fn it_filters_orders_to_a_system() {
      let db = store::open_test().await.unwrap();
      let orders = vec![
        order(STATION, 30_000_142, 8.0, false),
        order(60_000_001, 30_000_005, 5.0, false),
      ];

      let filtered =
        filter_orders_to_scope(&db, &scope_of(LocationTier::System, 30_000_005, Some(REGION)), orders).await;

      assert_eq!(filtered.len(), 1);
      assert_eq!(filtered[0].price, 5.0);
    }

    #[tokio::test]
    async fn it_filters_orders_to_a_constellation() {
      let db = store::open_test().await.unwrap();
      sde::upsert_region(
        &db,
        &Region {
          description: None,
          id: REGION,
          name: "The Forge".to_owned(),
        },
      )
      .await
      .unwrap();
      sde::upsert_constellation(
        &db,
        &Constellation {
          id: 20_000_020,
          name: "Kimotoro".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          region_id: REGION,
        },
      )
      .await
      .unwrap();
      sde::upsert_solar_system(&db, &make_solar_system(30_000_142, 20_000_020))
        .await
        .unwrap();
      let orders = vec![
        order(STATION, 30_000_142, 8.0, false),
        order(60_000_001, 30_000_005, 5.0, false),
      ];

      let filtered = filter_orders_to_scope(
        &db,
        &scope_of(LocationTier::Constellation, 20_000_020, Some(REGION)),
        orders,
      )
      .await;

      assert_eq!(filtered.len(), 1);
      assert_eq!(filtered[0].system_id, 30_000_142);
    }

    #[tokio::test]
    async fn it_degrades_a_structure_scope_to_inaccessible_without_a_usable_grant() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;

      let prices = structure_scope_prices(&db, &esi, &sso, 1_035_000_000_001, TYPE).await;

      assert_eq!(prices.access, watch_eval::PriceAccess::Inaccessible);
      assert_eq!(prices.best_buy, None);
      assert_eq!(prices.best_sell, None);
    }

    #[tokio::test]
    async fn it_prices_a_region_watch_region_wide_and_a_station_watch_within_the_station() {
      let server = MockServer::start().await;
      mount_region_orders(&server).await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owner(&db).await;
      create_watch(&db, Some(REGION), Some("region")).await;
      create_watch(&db, Some(STATION), Some("station")).await;

      let prices = fetch_watch_prices(db, esi, sso).await;

      let region = prices.get(&(TYPE, REGION)).expect("the region watch is priced");
      assert_eq!(region.best_sell, Some(5.0));
      assert_eq!(region.best_buy, Some(12.0));

      let station = prices.get(&(TYPE, STATION)).expect("the station watch is priced");
      assert_eq!(station.best_sell, Some(8.0));
      assert_eq!(station.best_buy, Some(9.0));
    }
  }

  mod scope_place_label {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{Constellation, Region, SolarSystem, Structure},
    };

    const REGION: i64 = 10_000_002;
    const CONSTELLATION: i64 = 20_000_020;
    const SYSTEM: i64 = 30_000_142;
    const STATION: i64 = 60_003_760;
    const STRUCTURE: i64 = 1_035_000_000_001;

    fn watch(location_id: Option<i64>, location_tier: Option<&str>) -> MarketWatch {
      MarketWatch {
        location_id,
        location_tier: location_tier.map(str::to_owned),
        region_id: Some(REGION),
        type_id: 34,
        ..MarketWatch::default()
      }
    }

    async fn seed_geo(db: &Database) {
      sde::upsert_region(
        db,
        &Region {
          description: None,
          id: REGION,
          name: "The Forge".to_owned(),
        },
      )
      .await
      .unwrap();
      sde::upsert_constellation(
        db,
        &Constellation {
          id: CONSTELLATION,
          name: "Kimotoro".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          region_id: REGION,
        },
      )
      .await
      .unwrap();
      sde::upsert_solar_system(
        db,
        &SolarSystem {
          constellation_id: CONSTELLATION,
          id: SYSTEM,
          name: "Jita".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          security_class: None,
          security_status: 0.9,
          star_id: None,
        },
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_is_blank_when_no_location_is_scoped() {
      let db = store::open_test().await.unwrap();

      assert_eq!(scope_place_label(&db, &watch(None, None)).await, "");
    }

    #[tokio::test]
    async fn it_is_blank_for_a_region_scope() {
      let db = store::open_test().await.unwrap();

      assert_eq!(scope_place_label(&db, &watch(Some(REGION), Some("region"))).await, "");
    }

    #[tokio::test]
    async fn it_labels_a_constellation_by_name() {
      let db = store::open_test().await.unwrap();
      seed_geo(&db).await;

      let label = scope_place_label(&db, &watch(Some(CONSTELLATION), Some("constellation"))).await;

      assert_eq!(label, "Kimotoro");
    }

    #[tokio::test]
    async fn it_labels_a_system_by_name() {
      let db = store::open_test().await.unwrap();
      seed_geo(&db).await;

      let label = scope_place_label(&db, &watch(Some(SYSTEM), Some("system"))).await;

      assert_eq!(label, "Jita");
    }

    #[tokio::test]
    async fn it_labels_a_structure_by_name() {
      let db = store::open_test().await.unwrap();
      seed_geo(&db).await;
      sqlx::query(
        "INSERT INTO corporations (id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
        VALUES (98000001, 1, 1, 1, 'Owner Corp', 0.0, 'OWN')",
      )
      .execute(db.writer())
      .await
      .unwrap();
      sde::upsert_structure(
        &db,
        &Structure {
          id: STRUCTURE,
          name: "Jita Trade Hub".to_owned(),
          owner_id: 98_000_001,
          position_x: None,
          position_y: None,
          position_z: None,
          solar_system_id: SYSTEM,
          type_id: None,
        },
      )
      .await
      .unwrap();

      let label = scope_place_label(&db, &watch(Some(STRUCTURE), Some("structure"))).await;

      assert_eq!(label, "Jita Trade Hub");
    }

    #[tokio::test]
    async fn it_falls_back_for_a_station_that_cannot_be_resolved() {
      let db = store::open_test().await.unwrap();

      let label = scope_place_label(&db, &watch(Some(STATION), Some("station"))).await;

      assert!(!label.is_empty());
    }
  }

  mod orders_history {
    use pretty_assertions::assert_eq;

    use super::*;

    fn lot(transaction_id: i64, date: &str) -> inventory_lots::Lot {
      inventory_lots::Lot {
        date: date.to_owned(),
        quantity: 10,
        quantity_remaining: 10,
        target_price: 110.0,
        transaction_id,
        unit_price: 100.0,
      }
    }

    fn card(owner_id: i64, is_corporation: bool, dates: &[&str]) -> LotGroupCard {
      LotGroupCard {
        group: inventory_lots::LotGroup {
          average_cost: 100.0,
          average_target: 110.0,
          estimated_profit: 100.0,
          held_quantity: 10 * dates.len() as i64,
          is_corporation,
          location_id: 60_003_760,
          lots: dates
            .iter()
            .enumerate()
            .map(|(index, date)| lot(index as i64 + 1, date))
            .collect(),
          owner_id,
          type_id: 34,
        },
        owner_name: "Test Pilot".to_owned(),
        region_label: "The Forge".to_owned(),
        system_label: "Jita".to_owned(),
      }
    }

    fn prompt() -> LotDismissPrompt {
      LotDismissPrompt {
        is_corporation: false,
        item_name: "Tritanium".to_owned(),
        owner_id: 90,
        transaction_id: 7,
      }
    }

    #[test]
    fn it_switches_the_orders_sub_tab() {
      let mut state = State::new();

      update(&mut state, Message::OrdersSubTabSelected(OrdersSubTab::History));
      assert_eq!(state.orders_sub(), OrdersSubTab::History);

      update(&mut state, Message::OrdersSubTabSelected(OrdersSubTab::Current));
      assert_eq!(state.orders_sub(), OrdersSubTab::Current);
    }

    #[test]
    fn it_stores_loaded_lot_groups() {
      let mut state = State::new();

      update(
        &mut state,
        Message::LotsLoaded(vec![card(90, false, &["2026-07-01T00:00:00Z"])]),
      );

      assert_eq!(state.lot_groups().len(), 1);
    }

    #[test]
    fn it_opens_and_cancels_the_lot_dismiss_prompt() {
      let mut state = State::new();

      update(&mut state, Message::LotDismissPrompted(Box::new(prompt())));
      assert_eq!(state.lot_dismiss().map(|open| open.transaction_id), Some(7));

      update(&mut state, Message::LotDismissCancelled);
      assert!(state.lot_dismiss().is_none());
    }

    #[test]
    fn it_clears_the_prompt_on_confirm() {
      let mut state = State::new();
      update(&mut state, Message::LotDismissPrompted(Box::new(prompt())));

      update(&mut state, Message::LotDismissConfirmed);

      assert!(state.lot_dismiss().is_none());
    }

    #[test]
    fn it_sorts_cards_with_the_most_recent_purchase_first() {
      let mut cards = vec![
        card(90, false, &["2026-06-01T00:00:00Z"]),
        card(91, false, &["2026-06-10T00:00:00Z", "2026-07-01T00:00:00Z"]),
      ];

      sort_lot_cards(&mut cards);

      assert_eq!(cards[0].group.owner_id, 91);
      assert_eq!(cards[1].group.owner_id, 90);
    }
  }
}
