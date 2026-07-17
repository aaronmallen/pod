mod book;
mod book_view;
mod browse;
mod history;
mod history_chart;
mod history_view;
mod i18n;
mod my_orders;
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
  services::location_search::{LocationRef, LocationTier},
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
  OpenInGame { character_id: i64, type_id: i64 },
  MarketWindowOpened(Result<(), String>),
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
  WatchSaved,
  WatchPricesLoaded(watch_eval::PriceMap),
  WatchesLoaded(Vec<WatchCard>),
  WatchCursorMoved(Point),
  WatchMenuOpened(i64),
  WatchMenuDismissed,
  WatchRemoved(i64),
  OwnOrdersLoaded(Vec<MarketOrder>),
  AlertOutbidLoaded(i64),
  DetailViewSelected(DetailView),
  HistoryLoaded(i64, i64, Result<Vec<history::HistoryPoint>, String>),
  HistoryRangeSelected(history::Range),
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
}

impl State {
  pub fn new() -> Self {
    State {
      tab: Tab::default(),
      tree: tree::MarketTree::default(),
      tree_pane: PaneDrag::new(MARKET_TREE_PANE_DEFAULT, spacing::layout::WINDOW_DEFAULT_WIDTH),
      book: None,
      book_access: BookAccess::default(),
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
    }
  }

  pub fn with_restored_panes(mut self, ui: &UiState) -> Self {
    let host_width = ui.host_width("main", spacing::layout::WINDOW_DEFAULT_WIDTH);
    self.tree_pane = PaneDrag::from_store(ui, MARKET_TREE_PANE_KEY, MARKET_TREE_PANE_DEFAULT, host_width);
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

  pub fn alert_outbid(&self) -> i64 {
    self.alert_outbid
  }

  pub fn outbid_count(&self) -> usize {
    self.orders.outbid_count
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
  Orders,
  Watchlist,
}

impl Tab {
  pub const ORDER: [Tab; 3] = [Tab::Browse, Tab::Orders, Tab::Watchlist];

  pub fn from_id(id: &str) -> Option<Tab> {
    match id {
      "browse" => Some(Tab::Browse),
      "orders" => Some(Tab::Orders),
      "watchlist" => Some(Tab::Watchlist),
      _ => None,
    }
  }

  pub fn id(self) -> &'static str {
    match self {
      Tab::Browse => "browse",
      Tab::Orders => "orders",
      Tab::Watchlist => "watchlist",
    }
  }
}

pub fn load(db: &Database) -> Task<Message> {
  Task::batch([
    Task::perform(load_tree(db.clone()), |tree| Message::TreeLoaded(Box::new(tree))),
    Task::perform(resolve_default_region(db.clone()), Message::DefaultMarketResolved),
    Task::perform(load_own_orders(db.clone()), Message::OwnOrdersLoaded),
    Task::perform(fetch_alert_outbid(db.clone()), Message::AlertOutbidLoaded),
  ])
}

async fn fetch_alert_outbid(db: Database) -> i64 {
  crate::store::repo::market_alert_state::count_alerted(&db, crate::store::model::MarketAlertKind::Outbid)
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

async fn resolve_default_region(db: Database) -> LocationRef {
  let region_id = match market_repo::default_market(&db).await {
    Ok(Some(place)) => region_of(&db, place).await.unwrap_or(THE_FORGE_REGION_ID),
    _ => THE_FORGE_REGION_ID,
  };
  region_ref(&db, region_id).await
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
    // Structures resolve only through an authenticated ESI lookup; that is deferred to Phase 5, so a
    // structure default falls back to Jita / The Forge for now.
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
  LocationRef {
    context: None,
    id,
    name,
    security_status: None,
    tier: Some(LocationTier::Region),
  }
}

#[cfg(test)]
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
  Watch(String),
}

pub fn location_search_field(message: &Message) -> Option<LocationSearchField> {
  match message {
    Message::RegionSearchChanged(query) => Some(LocationSearchField::Browse(query.clone())),
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
// state. The live current/met status is layered on in the view from `watch_prices`. The scope picks
// the union of every character's watches (`list`) or a single pilot's (`list_for_character`).
async fn fetch_watches(db: Database) -> Vec<WatchCard> {
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

async fn fetch_orders(db: Database, scope: OrdersScope) -> OrdersData {
  let char_orders = load_char_orders(&db, scope).await;
  let corp_orders = load_corp_orders(&db, scope).await;
  let corp_owner_ids: HashSet<i64> = corp_orders.iter().map(MarketOrder::character_id).collect();

  let mut raw = char_orders;
  raw.extend(corp_orders.iter().cloned());

  let quotes = fetch_quotes(&db, &raw).await;
  let annotations = outbid::annotate_all(&raw, &quotes);
  let roster = load_roster(&db).await;
  let mut names: HashMap<i64, String> = roster.iter().map(|pilot| (pilot.id, pilot.name.clone())).collect();
  names.extend(load_corp_names(&db).await);

  let mut rows = Vec::with_capacity(raw.len());
  for (order, annotation) in raw.iter().zip(annotations.iter()) {
    rows.push(build_order_row(&db, order, annotation, &names, &corp_owner_ids).await);
  }
  sort_order_rows(&mut rows);

  let char_id = scope.character_id();
  OrdersData {
    scope,
    active_count: raw.iter().filter(|order| order.volume_remain() > 0).count(),
    sell_count: order_side_count(&raw, false),
    buy_count: order_side_count(&raw, true),
    outbid_count: annotations.iter().filter(|annotation| annotation.outbid).count(),
    sell_listed: finance::open_sell_value(&db, char_id).await.unwrap_or(0.0) + corp_sell_value(&corp_orders),
    buy_escrow: finance::open_buy_escrow(&db, char_id).await.unwrap_or(0.0) + corp_buy_escrow(&corp_orders),
    roster,
    rows,
  }
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

// Corp orders ride the same character-keyed pipeline as personal orders (roster/name lookup,
// corp_owner_ids membership, outbid annotation), so the corporation id is stored in `character_id`.
fn corp_order_as_market(order: &CorporationMarketOrder) -> MarketOrder {
  MarketOrder {
    character_id: order.corporation_id(),
    duration: order.duration(),
    escrow: order.escrow(),
    is_buy_order: order.is_buy_order(),
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

fn corp_sell_value(orders: &[MarketOrder]) -> f64 {
  orders
    .iter()
    .filter(|order| !order.is_buy_order() && order.volume_remain() > 0)
    .map(|order| order.price() * order.volume_remain() as f64)
    .sum()
}

fn corp_buy_escrow(orders: &[MarketOrder]) -> f64 {
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
  corp_owner_ids: &HashSet<i64>,
) -> OrderRow {
  let (region_label, system_label) = location_labels(db, order.region_id(), order.location_id()).await;
  OrderRow {
    character_id: order.character_id(),
    character_name: names.get(&order.character_id()).cloned().unwrap_or_default(),
    owner_is_corp: corp_owner_ids.contains(&order.character_id()),
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
) -> Result<(), String> {
  let grant = fresh_character_grant(&db, &sso, character_id).await?;
  esi
    .market()
    .open_market_window(type_id, &grant)
    .await
    .map_err(|error| error.to_string())
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
    }
    Message::DefaultMarketResolved(region) => {
      // A user pick made before this async default resolves wins; only adopt the default once.
      if state.active_region.is_none() {
        state.active_region = Some(region);
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
    | Message::OrdersScopeSelected(_) => update_orders(state, message),
    Message::OpenInGame {
      ..
    } => {}
    Message::MarketWindowOpened(result) => {
      if let Err(error) = result {
        tracing::warn!(%error, "failed to open the in-game market window");
      }
    }
    Message::WatchPricesLoaded(prices) => state.watch_prices = prices,
    Message::WatchesLoaded(watches) => state.watches = watches,
    Message::DetailViewSelected(view) => state.detail_view = view,
    // A range change only re-slices the already-fetched 365-day series in the view; the fetch holds
    // every day, so it drives no follow-up task.
    Message::HistoryRangeSelected(range) => state.history_range = range,
    Message::HistoryLoaded(region_id, type_id, result) => {
      apply_history_loaded(state, region_id, type_id, result);
    }
    other => watchlist::reduce(state, other),
  }
  sync_history_target(state);
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
      state.active_place = Some(location.clone());
      match location.tier {
        Some(LocationTier::Region) => {
          state.active_region = Some(location);
          state.active_structure = None;
        }
        Some(LocationTier::Structure) => state.active_structure = Some(location),
        // Constellation/System/Station: the owning region is resolved asynchronously (RegionResolved)
        // since the order book is region-scoped; the picked place drives the header label + jumps.
        _ => state.active_structure = None,
      }
    }
    Message::RegionResolved(region) => {
      state.active_region = Some(region);
      state.active_structure = None;
    }
    _ => {}
  }
}

fn update_orders(state: &mut State, message: Message) {
  match message {
    Message::OrdersLoaded(data) if data.scope == state.orders_scope => {
      state.orders = *data;
    }
    Message::OrdersScopeToggled => {
      state.orders_picker_open = !state.orders_picker_open;
    }
    Message::OrdersScopeDismissed => state.orders_picker_open = false,
    Message::OrdersScopeSelected(scope) => {
      state.orders_picker_open = false;
      state.orders_scope = scope;
    }
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
pub fn dispatch(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  // Watchlist-modal messages carry their own reducer and follow-ups; peel them off here so the
  // browse/orders reducer below stays focused on the tree-and-book flow.
  let message = match watchlist::try_dispatch(state, message, db) {
    Ok(task) => return task,
    Err(message) => message,
  };

  // Tree-pane drag messages mutate only the pane geometry and, on release, bubble the settled ratio
  // for persistence; peel them off before the browse/orders reducer so `update` stays free of them.
  if let Some(task) = try_pane(state, &message) {
    return task;
  }

  enum Follow {
    Book,
    None,
    Orders,
    RemoveWatch(i64),
    ResolvePlace(i64),
    WatchPrices,
  }

  let follow = match &message {
    Message::DefaultMarketResolved(_) | Message::ItemSelected(_) | Message::RegionResolved(_) => Follow::Book,
    // A region pick fetches its book directly; a structure pick is fetched at the app layer; any other
    // tier (constellation/system/station) resolves to its region first, then fetches on RegionResolved.
    Message::RegionPicked(location) => match location.tier {
      Some(LocationTier::Region) => Follow::Book,
      Some(LocationTier::Structure) => Follow::None,
      _ => Follow::ResolvePlace(location.id),
    },
    Message::TabSelected(Tab::Orders) | Message::OrdersScopeSelected(_) => Follow::Orders,
    Message::TabSelected(Tab::Watchlist) => Follow::WatchPrices,
    Message::WatchRemoved(id) => Follow::RemoveWatch(*id),
    _ => Follow::None,
  };

  let prev_history_key = state.history_key;
  update(state, message);
  let history = history_follow_task(state, prev_history_key, db);

  let base = match follow {
    Follow::None => Task::none(),
    Follow::Book => fetch_book_task(state, db),
    Follow::Orders => load_orders_task(state, db),
    Follow::WatchPrices => load_watches(db),
    Follow::RemoveWatch(id) => remove_watch_task(db, id),
    Follow::ResolvePlace(place_id) => {
      Task::perform(resolve_place_region(db.clone(), place_id), Message::RegionResolved)
    }
  };

  Task::batch([base, history])
}

pub fn view(state: &State) -> Element<'_, Message> {
  watchlist::mount(shell::shell(state), state)
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  let mut subs = Vec::new();

  // The Market route has no app-level Escape hook, so the open card context menu listens for Escape
  // itself and dismisses; an outside click falls through to its backdrop.
  if state.watch_menu.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      is_escape_pressed(&event).then_some(Message::WatchMenuDismissed)
    }));
  }

  if state.tree_pane.is_active() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      resizable_pane::drag_event(event, Message::PaneDrag, Message::PaneDragEnd)
    }));
  }

  iced::Subscription::batch(subs)
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
    }

    #[test]
    fn it_keeps_a_user_region_over_a_late_default() {
      let mut state = State::new();

      update(&mut state, Message::RegionPicked(region(10_000_043)));
      update(&mut state, Message::DefaultMarketResolved(region(THE_FORGE_REGION_ID)));

      assert_eq!(state.active_region_id(), Some(10_000_043));
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
    use crate::store;

    async fn seed_regions(db: &Database) {
      sqlx::query("INSERT INTO regions (id, name) VALUES (10000002, 'The Forge'), (10000043, 'Domain')")
        .execute(db.writer())
        .await
        .unwrap();
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

    #[tokio::test]
    async fn it_falls_back_to_the_forge_for_an_unset_default() {
      let db = store::open_test().await.unwrap();
      seed_regions(&db).await;

      let resolved = resolve_default_region(db).await;

      assert_eq!(resolved.id, THE_FORGE_REGION_ID);
      assert_eq!(resolved.tier, Some(LocationTier::Region));
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
        character_id: 90_000_001,
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
}
