mod book;
mod book_view;
mod browse;
mod history;
mod history_chart;
mod i18n;
mod my_orders;
mod outbid;
mod shell;
mod tree;
mod watch_eval;
mod watchlist;

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use iced::{Element, Task};

use crate::{
  clients::{self, esi, esi::models::market::RegionOrder, eve_sso, http},
  features::assets::{LocationRef, LocationTier},
  store::{
    Database,
    model::{MarketOrder, MarketWatch, OwnerType, WatchDirection},
    repo::{character as character_repo, finance, market as market_repo, sde},
  },
  ui::components::location_combobox::LocationSearch,
};

const THE_FORGE_REGION_ID: i64 = 10_000_002;
const MAX_REGION_RESULTS: usize = 25;

#[derive(Clone, Debug)]
pub enum Message {
  TabSelected(Tab),
  TreeLoaded(Box<tree::MarketTree>),
  BookLoaded(Box<book::OrderBook>),
  NodeToggled(i64),
  FilterChanged(String),
  ItemSelected(i64),
  DefaultMarketResolved(LocationRef),
  RegionPickerToggled,
  RegionPickerClosed,
  RegionSearchChanged(String),
  RegionResultsLoaded(u64, Vec<LocationRef>),
  RegionPicked(LocationRef),
  OrdersLoaded(Box<OrdersData>),
  OrdersScopeToggled,
  OrdersScopeDismissed,
  OrdersScopeSelected(OrdersScope),
  OpenInGame {
    character_id: i64,
    type_id: i64,
  },
  MarketWindowOpened(Result<(), String>),
  WatchNew,
  #[allow(dead_code)]
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
  OwnOrdersLoaded(Vec<MarketOrder>),
  DetailViewSelected(DetailView),
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
pub struct OrderPilot {
  pub id: i64,
  pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrderRow {
  pub character_id: i64,
  pub character_name: String,
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
  tab: Tab,
  tree: tree::MarketTree,
  book: Option<book::OrderBook>,
  expanded: HashSet<i64>,
  filter: String,
  selected: Option<i64>,
  active_region: Option<LocationRef>,
  region_search: LocationSearch,
  region_picker_open: bool,
  orders_scope: OrdersScope,
  orders_picker_open: bool,
  orders: OrdersData,
  watch_modal: Option<watchlist::WatchForm>,
  watch_prices: watch_eval::PriceMap,
  own_orders: Vec<MarketOrder>,
  detail_view: DetailView,
  active_structure: Option<LocationRef>,
}

impl State {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn active_tab(&self) -> Tab {
    self.tab
  }

  pub fn tree(&self) -> &tree::MarketTree {
    &self.tree
  }

  pub fn filter(&self) -> &str {
    &self.filter
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
    self.active_structure.as_ref().or(self.active_region.as_ref())
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

  pub fn outbid_count(&self) -> usize {
    self.orders.outbid_count
  }

  #[allow(dead_code)]
  pub fn watch_prices(&self) -> &watch_eval::PriceMap {
    &self.watch_prices
  }

  pub fn own_orders(&self) -> &[MarketOrder] {
    &self.own_orders
  }

  pub fn detail_view(&self) -> DetailView {
    self.detail_view
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
  ])
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

async fn search_regions(db: Database, query: String, generation: u64) -> (u64, Vec<LocationRef>) {
  let needle = query.trim().to_lowercase();
  let mut results: Vec<LocationRef> = sde::all_regions(&db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter(|region| region.name().to_lowercase().contains(&needle))
    .map(|region| region_location(region.id(), region.name().to_owned()))
    .collect();
  results.sort_by(|left, right| left.name.cmp(&right.name));
  results.truncate(MAX_REGION_RESULTS);
  (generation, results)
}

fn fetch_book_task(state: &State, db: &Database) -> Task<Message> {
  // A structure book needs an authed grant, so it is fetched at the app layer via
  // `fetch_structure_book_task`; skip the tokenless region path while a structure is active.
  if state.active_structure.is_some() {
    return Task::none();
  }
  match (state.active_region_id(), state.selected_type_id()) {
    (Some(region_id), Some(type_id)) => load_book(db, region_id, type_id),
    _ => Task::none(),
  }
}

pub fn load_book(db: &Database, region_id: i64, type_id: i64) -> Task<Message> {
  Task::perform(fetch_book(db.clone(), region_id, type_id), |book| {
    Message::BookLoaded(Box::new(book))
  })
}

async fn fetch_book(db: Database, region_id: i64, type_id: i64) -> book::OrderBook {
  let Ok(esi) = public_esi(&db) else {
    return book::OrderBook::default();
  };
  let mut orders = esi.market().sell_orders(region_id, type_id).await.unwrap_or_default();
  orders.extend(esi.market().buy_orders(region_id, type_id).await.unwrap_or_default());
  book::build_order_book(orders)
}

fn load_watch_prices(db: &Database) -> Task<Message> {
  Task::perform(fetch_watch_prices(db.clone()), Message::WatchPricesLoaded)
}

// Consume the Phase 1-2 order book: read one region best buy/sell per distinct (type_id, region)
// pair so N watches on the same item and region trigger a single price read.
async fn fetch_watch_prices(db: Database) -> watch_eval::PriceMap {
  let watches = crate::store::repo::market_watchlist::list(&db)
    .await
    .unwrap_or_default();
  let mut pairs = HashSet::new();
  for watch in &watches {
    if let Some(region_id) = watch.region_id {
      pairs.insert((watch.type_id, region_id));
    }
  }
  let mut prices = watch_eval::PriceMap::new();
  for (type_id, region_id) in pairs {
    let book = fetch_book(db.clone(), region_id, type_id).await;
    prices.insert(
      (type_id, region_id),
      watch_eval::BestPrices {
        best_buy: book.best_buy,
        best_sell: book.best_sell,
      },
    );
  }
  prices
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
  let raw = load_raw_orders(&db, scope).await;
  let quotes = fetch_quotes(&db, &raw).await;
  let annotations = outbid::annotate_all(&raw, &quotes);
  let roster = load_roster(&db).await;
  let names: HashMap<i64, String> = roster.iter().map(|pilot| (pilot.id, pilot.name.clone())).collect();

  let mut rows = Vec::with_capacity(raw.len());
  for (order, annotation) in raw.iter().zip(annotations.iter()) {
    rows.push(build_order_row(&db, order, annotation, &names).await);
  }
  sort_order_rows(&mut rows);

  let char_id = scope.character_id();
  OrdersData {
    scope,
    active_count: raw.iter().filter(|order| order.volume_remain() > 0).count(),
    sell_count: order_side_count(&raw, false),
    buy_count: order_side_count(&raw, true),
    outbid_count: annotations.iter().filter(|annotation| annotation.outbid).count(),
    sell_listed: finance::open_sell_value(&db, char_id).await.unwrap_or(0.0),
    buy_escrow: finance::open_buy_escrow(&db, char_id).await.unwrap_or(0.0),
    roster,
    rows,
  }
}

async fn load_raw_orders(db: &Database, scope: OrdersScope) -> Vec<MarketOrder> {
  match scope.character_id() {
    Some(id) => finance::open_for_character(db, id).await,
    None => finance::open_all(db).await,
  }
  .unwrap_or_default()
}

async fn load_roster(db: &Database) -> Vec<OrderPilot> {
  character_repo::all_owned(db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|character| OrderPilot {
      id: character.id(),
      name: character.name().clone(),
    })
    .collect()
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
  OrderRow {
    character_id: order.character_id(),
    character_name: names.get(&order.character_id()).cloned().unwrap_or_default(),
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
    |book| Message::BookLoaded(Box::new(book)),
  )
}

async fn fetch_structure_book(
  db: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  structure_id: i64,
  type_id: i64,
) -> book::OrderBook {
  let Some(grant) = first_owned_grant(&db, &sso).await else {
    return book::OrderBook::default();
  };
  // The structure endpoint returns the full book with no server-side type filter; graceful no-access
  // handling (403/404) is a separate Phase 6 task, so a failure logs and yields an empty book here.
  let orders = match esi.market().structure_orders(structure_id, &grant).await {
    Ok(orders) => orders,
    Err(error) => {
      tracing::warn!(target: "pod::market", %error, structure_id, "structure order book fetch failed");
      return book::OrderBook::default();
    }
  };
  let filtered = orders.into_iter().filter(|order| order.type_id == type_id).collect();
  book::build_order_book(filtered)
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

// State-only reducer, kept free of the store so it stays synchronously testable. Side effects that
// need the database (region search, order-book fetch) are layered on by `dispatch`.
pub fn update(state: &mut State, message: Message) {
  match message {
    Message::TabSelected(tab) => state.tab = tab,
    Message::TreeLoaded(tree) => state.tree = *tree,
    Message::BookLoaded(book) => state.book = Some(*book),
    Message::OwnOrdersLoaded(orders) => state.own_orders = orders,
    Message::NodeToggled(id) => {
      if !state.expanded.remove(&id) {
        state.expanded.insert(id);
      }
    }
    Message::FilterChanged(query) => state.filter = query,
    Message::ItemSelected(type_id) => {
      state.selected = Some(type_id);
      state.detail_view = DetailView::default();
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
    | Message::RegionPicked(_) => update_region(state, message),
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
    Message::DetailViewSelected(view) => state.detail_view = view,
    other => watchlist::reduce(state, other),
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
      match location.tier {
        Some(LocationTier::Region) => {
          state.active_region = Some(location);
          state.active_structure = None;
        }
        Some(LocationTier::Structure) => state.active_structure = Some(location),
        _ => {}
      }
    }
    _ => {}
  }
}

fn update_orders(state: &mut State, message: Message) {
  match message {
    Message::OrdersLoaded(data)
      if data.scope == state.orders_scope => {
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

  enum Follow {
    Book,
    None,
    Orders,
    Search(String),
    WatchPrices,
  }

  let follow = match &message {
    Message::RegionSearchChanged(query) => Follow::Search(query.clone()),
    Message::DefaultMarketResolved(_) | Message::ItemSelected(_) | Message::RegionPicked(_) => Follow::Book,
    Message::TabSelected(Tab::Orders) | Message::OrdersScopeSelected(_) => Follow::Orders,
    Message::TabSelected(Tab::Watchlist) => Follow::WatchPrices,
    _ => Follow::None,
  };

  update(state, message);

  match follow {
    Follow::None => Task::none(),
    Follow::Book => fetch_book_task(state, db),
    Follow::Orders => load_orders_task(state, db),
    Follow::WatchPrices => load_watch_prices(db),
    Follow::Search(query) => {
      if !state.region_search.searchable() {
        return Task::none();
      }
      let generation = state.region_search.generation();
      Task::perform(
        search_regions(db.clone(), query, generation),
        |(generation, results)| Message::RegionResultsLoaded(generation, results),
      )
    }
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  watchlist::mount(shell::shell(state), state)
}

pub fn subscription(_state: &State) -> iced::Subscription<Message> {
  iced::Subscription::none()
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

    #[tokio::test]
    async fn it_matches_regions_by_name_case_insensitively() {
      let db = store::open_test().await.unwrap();
      seed_regions(&db).await;

      let (generation, results) = search_regions(db, "forge".to_owned(), 7).await;

      assert_eq!(generation, 7);
      let ids: Vec<i64> = results.iter().map(|region| region.id).collect();
      assert_eq!(ids, vec![THE_FORGE_REGION_ID]);
      assert_eq!(results[0].tier, Some(LocationTier::Region));
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
}
