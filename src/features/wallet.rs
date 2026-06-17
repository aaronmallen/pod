mod header;
mod hero;
mod loaders;
mod shell;
mod side_filter;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use iced::{Element, Task};

pub use self::{
  loaders::{ContractEntry, JournalEntry, MarketEntry, PartyImage},
  side_filter::Side,
};
pub(crate) use crate::ui::format::fmt_isk_opt as fmt_isk;
use crate::{
  features::contract_detail,
  store::{
    Database, images,
    model::{
      OwnerType, character_financials::CharacterFinancials,
      character_wallet_period_summary::CharacterWalletPeriodSummary,
    },
    repo::{character, finance, infra, org},
  },
  sync::JobKind,
  ui::components::resizable_pane::PaneDrag,
  window_state,
};

const DEFAULT_DIVISION: i64 = 1;
const HEADER_SIDE_PADDING: f32 = 28.0;
pub const PAGE_SIZE: usize = 50;
const RECENT_ACTIVITY_LIMIT: usize = 8;
const RIGHT_RAIL_DEFAULT_WIDTH: f32 = 280.0;
const RIGHT_RAIL_PANE_KEY: &str = "wallet.right_rail";

/// Fraction of the ledger a scroll must reach before the next cursor page is
/// fetched. The window only ever materializes the viewport's rows, so this only
/// gates how early the next DB page starts streaming in behind the scroll.
const SCROLL_LOAD_THRESHOLD: f32 = 0.8;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Scope {
  #[default]
  All,
  Character(i64),
  Corporation(i64),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Tab {
  Contracts,
  Journal,
  #[default]
  Market,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SignFilter {
  #[default]
  All,
  In,
  Out,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Timeframe {
  HalfYear,
  Month,
  #[default]
  Quarter,
  Week,
  Year,
}

impl Timeframe {
  pub fn all() -> [Timeframe; 5] {
    [
      Timeframe::Week,
      Timeframe::Month,
      Timeframe::Quarter,
      Timeframe::HalfYear,
      Timeframe::Year,
    ]
  }

  pub fn days(self) -> usize {
    match self {
      Timeframe::HalfYear => 180,
      Timeframe::Month => 30,
      Timeframe::Quarter => 90,
      Timeframe::Week => 7,
      Timeframe::Year => 365,
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      Timeframe::HalfYear => "6M",
      Timeframe::Month => "1M",
      Timeframe::Quarter => "3M",
      Timeframe::Week => "1W",
      Timeframe::Year => "1Y",
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetWorthPoint {
  pub date: String,
  pub liquid: f64,
  pub net_worth: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RosterPilot {
  pub corp: String,
  pub granted_scopes: Option<String>,
  pub id: i64,
  pub liquid: Option<f64>,
  pub name: String,
  pub portrait: images::ImageState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RosterCorp {
  pub id: i64,
  pub liquid: Option<f64>,
  pub logo: images::ImageState,
  pub name: String,
  pub ticker: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CorpDivision {
  pub balance: Option<f64>,
  pub division: i64,
  pub name: Option<String>,
}

impl CorpDivision {
  pub fn label(&self) -> String {
    self
      .name
      .clone()
      .unwrap_or_else(|| format!("Division {}", self.division))
  }
}

#[derive(Clone, Debug, Default)]
pub struct MorePage {
  contracts: Vec<ContractEntry>,
  journal: Vec<JournalEntry>,
  market: Vec<MarketEntry>,
  scope: Scope,
  tab: Tab,
}

impl MorePage {
  fn contracts(contracts: Vec<ContractEntry>) -> Self {
    Self {
      contracts,
      ..Self::default()
    }
  }

  fn journal(journal: Vec<JournalEntry>) -> Self {
    Self {
      journal,
      ..Self::default()
    }
  }

  fn market(market: Vec<MarketEntry>) -> Self {
    Self {
      market,
      ..Self::default()
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct Loaded {
  contract_total: i64,
  contracts: Vec<ContractEntry>,
  corp_divisions: Vec<CorpDivision>,
  corporations: Vec<RosterCorp>,
  financials: Vec<CharacterFinancials>,
  journal: Vec<JournalEntry>,
  journal_total: i64,
  market: Vec<MarketEntry>,
  market_total: i64,
  net_worth_series: Vec<NetWorthPoint>,
  periods: Vec<CharacterWalletPeriodSummary>,
  right_rail_width: f32,
  roster: Vec<RosterPilot>,
}

#[derive(Clone, Debug)]
pub enum Message {
  ChartHovered(Option<f32>),
  CloseContractDetail,
  ContractDetailLoaded(Box<Option<contract_detail::ContractDetail>>),
  ContractSelected(i64),
  DivisionSelected(i64),
  Loaded(Box<Loaded>),
  MoreLoaded(Box<MorePage>),
  PaneSettled(&'static str, f32),
  PickerToggled,
  RailDragEnd,
  RailDragged(f32),
  RailDragStart,
  ReauthRequested(i64),
  ScopeSelected(Scope),
  SearchChanged(String),
  SideFilterChanged(Side),
  SignFilterChanged(SignFilter),
  /// `relative` is the 0.0–1.0 scroll fraction that drives the cursor-pagination threshold;
  /// `absolute` is the pixel offset stored to window the visible ledger.
  TabScrolled {
    absolute: f32,
    relative: f32,
  },
  TabSelected(Tab),
  TimeframeSelected(Timeframe),
}

impl Message {
  /// Whether handling this message can surface new image-bearing rows, so the shell should recheck for stale
  /// icons/portraits. Interaction-only messages (scroll, hover, filter) return `false` to keep the staleness scan
  /// off the per-frame path.
  pub fn loads_data(&self) -> bool {
    matches!(
      self,
      Message::ContractDetailLoaded(_) | Message::Loaded(_) | Message::MoreLoaded(_)
    )
  }
}

/// Cached filter/derived view data. The `*_indices` fields index into `journal`/`market`/`contracts`, so any mutation
/// of those vecs must be followed by `recompute_derived()` or the indices go stale (out-of-bounds panic or wrong rows).
#[derive(Debug, Default)]
struct Derived {
  category_flows: Vec<CategoryFlow>,
  contract_indices: Vec<usize>,
  journal_flow: JournalFlow,
  journal_indices: Vec<usize>,
  market_indices: Vec<usize>,
  recent_activity_indices: Vec<usize>,
}

#[derive(Debug)]
pub struct State {
  active: Scope,
  active_division: i64,
  chart_hover: Option<f32>,
  contract_total: i64,
  contracts: Vec<ContractEntry>,
  corp_divisions: Vec<CorpDivision>,
  corporations: Vec<RosterCorp>,
  derived: Derived,
  dirty: bool,
  financials: Vec<CharacterFinancials>,
  journal: Vec<JournalEntry>,
  journal_total: i64,
  loading_more: bool,
  market: Vec<MarketEntry>,
  market_total: i64,
  net_worth_series: Vec<NetWorthPoint>,
  periods: Vec<CharacterWalletPeriodSummary>,
  picker_open: bool,
  right_rail: PaneDrag,
  roster: Vec<RosterPilot>,
  search: String,
  selected_contract: Option<contract_detail::ContractDetail>,
  side_filter: Side,
  sign_filter: SignFilter,
  tab: Tab,
  tab_exhausted: bool,
  tab_scroll_offset: f32,
  timeframe: Timeframe,
}

impl State {
  pub fn new() -> Self {
    State {
      active: Scope::default(),
      active_division: DEFAULT_DIVISION,
      chart_hover: None,
      contract_total: 0,
      contracts: Vec::new(),
      corp_divisions: Vec::new(),
      corporations: Vec::new(),
      derived: Derived::default(),
      dirty: false,
      financials: Vec::new(),
      journal: Vec::new(),
      journal_total: 0,
      loading_more: false,
      market: Vec::new(),
      market_total: 0,
      net_worth_series: Vec::new(),
      periods: Vec::new(),
      picker_open: false,
      right_rail: PaneDrag::new(
        RIGHT_RAIL_DEFAULT_WIDTH,
        crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH,
      )
      .right_anchored(true),
      roster: Vec::new(),
      search: String::new(),
      selected_contract: None,
      side_filter: Side::default(),
      sign_filter: SignFilter::default(),
      tab: Tab::default(),
      tab_exhausted: false,
      tab_scroll_offset: 0.0,
      timeframe: Timeframe::default(),
    }
  }

  pub fn with_restored_panes(mut self, ui: &window_state::UiState) -> Self {
    let host_width = ui.host_width("main", crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH);
    self.right_rail =
      PaneDrag::from_store(ui, RIGHT_RAIL_PANE_KEY, RIGHT_RAIL_DEFAULT_WIDTH, host_width).right_anchored(true);
    self
  }

  pub fn set_pane_host_width(&mut self, host_width: f32) {
    self.right_rail.set_host_width(host_width);
  }

  pub fn active(&self) -> Scope {
    self.active
  }

  pub fn drain_dirty(&mut self, db: &Database) -> Option<Task<Message>> {
    if !self.dirty {
      return None;
    }
    self.dirty = false;
    Some(load(db))
  }

  #[cfg(test)]
  pub fn is_dirty(&self) -> bool {
    self.dirty
  }

  pub fn mark_dirty(&mut self, kind: JobKind) {
    if reload_kind(kind) {
      self.dirty = true;
    }
  }

  pub(super) fn scope_gate(&self) -> Option<(i64, &str, Vec<&'static str>)> {
    let Scope::Character(id) = self.active else {
      return None;
    };
    let pilot = self.roster.iter().find(|pilot| pilot.id == id)?;
    let required = crate::features::registry::descriptor(crate::config::Feature::Wallet).scopes;
    let missing = crate::ui::components::forbidden::missing_scopes(pilot.granted_scopes.as_deref(), required);
    if missing.is_empty() {
      return None;
    }
    Some((id, pilot.name.as_str(), missing))
  }

  pub fn active_division(&self) -> i64 {
    self.active_division
  }

  pub(super) fn category_flows(&self) -> &[CategoryFlow] {
    &self.derived.category_flows
  }

  pub fn corp_divisions(&self) -> &[CorpDivision] {
    &self.corp_divisions
  }

  pub fn has_contracts(&self) -> bool {
    !self.contracts.is_empty()
  }

  pub(super) fn journal_flow(&self) -> JournalFlow {
    self.derived.journal_flow
  }

  pub(super) fn recent_activity(&self) -> Vec<&JournalEntry> {
    self
      .derived
      .recent_activity_indices
      .iter()
      .map(|&index| &self.journal[index])
      .collect()
  }

  pub(super) fn selected_contract(&self) -> Option<&contract_detail::ContractDetail> {
    self.selected_contract.as_ref()
  }

  pub fn side_filter(&self) -> Side {
    self.side_filter
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    let mut keys: Vec<(images::ImageKind, i64)> = Vec::new();
    keys.extend(self.roster.iter().filter_map(|pilot| pilot.portrait.stale_key()));
    keys.extend(self.corporations.iter().filter_map(|corp| corp.logo.stale_key()));
    for contract in &self.contracts {
      keys.extend(contract.acceptor_image.stale.iter().copied());
      keys.extend(contract.assignee_image.stale.iter().copied());
      keys.extend(contract.issuer_image.stale.iter().copied());
    }
    if let Some(detail) = &self.selected_contract {
      keys.extend(detail.stale_images());
    }
    let mut seen = std::collections::HashSet::new();
    keys.retain(|key| seen.insert(*key));
    keys
  }

  pub fn tab_scroll_offset(&self) -> f32 {
    self.tab_scroll_offset
  }

  pub fn timeframe(&self) -> Timeframe {
    self.timeframe
  }

  fn corp_balance_total(&self) -> Option<f64> {
    let balances: Vec<f64> = self
      .corp_divisions
      .iter()
      .filter_map(|division| division.balance)
      .collect();
    if balances.is_empty() {
      None
    } else {
      Some(balances.iter().sum())
    }
  }

  fn owned_corp_liquid(&self) -> Option<f64> {
    sum_option(self.corporations.iter().map(|corp| corp.liquid))
  }

  fn recompute_derived(&mut self) {
    let query = self.search.to_lowercase();

    let journal_indices: Vec<usize> = self
      .journal
      .iter()
      .enumerate()
      .filter(|(_, entry)| journal_matches(entry, self.sign_filter, &query))
      .map(|(index, _)| index)
      .collect();

    let market_indices: Vec<usize> = self
      .market
      .iter()
      .enumerate()
      .filter(|(_, entry)| market_matches(entry, self.sign_filter, self.side_filter, &query))
      .map(|(index, _)| index)
      .collect();

    let contract_indices: Vec<usize> = self
      .contracts
      .iter()
      .enumerate()
      .filter(|(_, entry)| contract_matches(entry, self.side_filter, &query))
      .map(|(index, _)| index)
      .collect();

    let matched: Vec<&JournalEntry> = journal_indices.iter().map(|&index| &self.journal[index]).collect();
    let journal_flow = journal_flow(&matched);
    let category_flows = category_flows(&matched);

    let mut recent_activity_indices = journal_indices.clone();
    recent_activity_indices.sort_by(|&a, &b| self.journal[b].date.cmp(&self.journal[a].date));
    recent_activity_indices.truncate(RECENT_ACTIVITY_LIMIT);

    self.derived = Derived {
      category_flows,
      contract_indices,
      journal_flow,
      journal_indices,
      market_indices,
      recent_activity_indices,
    };
  }

  fn scope_ids(&self) -> Vec<i64> {
    match self.active {
      Scope::All => self.roster.iter().map(|pilot| pilot.id).collect(),
      Scope::Character(id) => vec![id],
      Scope::Corporation(_) => Vec::new(),
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CategoryFlow {
  pub income: f64,
  pub ref_type: String,
  pub spend: f64,
}

impl CategoryFlow {
  pub fn label(&self) -> String {
    humanize_ref_type(&self.ref_type)
  }

  pub fn total(&self) -> f64 {
    self.income + self.spend
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Composition {
  pub asset_value: Option<f64>,
  pub escrow: Option<f64>,
  pub liquid: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompositionSlice {
  pub id: i64,
  pub name: String,
  pub net_worth: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JournalFlow {
  pub income: f64,
  pub net: f64,
  pub spend: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PeriodTotals {
  pub income: f64,
  pub net: f64,
  pub spend: f64,
}

pub fn load(db: &Database) -> Task<Message> {
  Task::perform(load_wallet(db.clone(), Scope::All, DEFAULT_DIVISION), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
}

fn reload(db: &Database, scope: Scope, division: i64) -> Task<Message> {
  Task::perform(load_wallet(db.clone(), scope, division), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
}

fn reload_kind(kind: JobKind) -> bool {
  matches!(
    kind,
    JobKind::CharacterWallet | JobKind::CorporationWallet | JobKind::MarketPrices | JobKind::NetWorthSnapshot
  )
}

fn load_more(state: &mut State, db: &Database) -> Task<Message> {
  if state.loading_more || state.tab_exhausted {
    return Task::none();
  }

  let scope = state.active;
  let tab = state.tab;

  if let Scope::Corporation(corp_id) = scope {
    if tab != Tab::Contracts {
      return Task::none();
    }
    let cursor = state
      .contracts
      .last()
      .map(|entry| (entry.date_issued.clone(), entry.contract_id));
    let db = db.clone();
    let limit = PAGE_SIZE as i64;
    state.loading_more = true;
    return Task::perform(
      async move { loaders::load_corp_contracts_page(&db, corp_id, cursor, limit).await },
      move |contracts| more_page(scope, tab, MorePage::contracts(contracts)),
    );
  }

  let scope_ids = state.scope_ids();
  if scope_ids.is_empty() {
    return Task::none();
  }

  let db = db.clone();
  let limit = PAGE_SIZE as i64;
  state.loading_more = true;
  match tab {
    Tab::Journal => {
      let cursor = state.journal.last().map(|entry| entry.id);
      Task::perform(
        async move { loaders::load_journal_page(&db, &scope_ids, cursor, limit).await },
        move |journal| more_page(scope, tab, MorePage::journal(journal)),
      )
    }
    Tab::Market => {
      let cursor = state.market.last().map(|entry| entry.transaction_id);
      Task::perform(
        async move { loaders::load_market_page(&db, &scope_ids, cursor, limit).await },
        move |market| more_page(scope, tab, MorePage::market(market)),
      )
    }
    Tab::Contracts => {
      let cursor = state
        .contracts
        .last()
        .map(|entry| (entry.date_issued.clone(), entry.contract_id));
      Task::perform(
        async move { loaders::load_contracts_page(&db, &scope_ids, cursor, limit).await },
        move |contracts| more_page(scope, tab, MorePage::contracts(contracts)),
      )
    }
  }
}

fn more_page(scope: Scope, tab: Tab, mut page: MorePage) -> Message {
  page.scope = scope;
  page.tab = tab;
  Message::MoreLoaded(Box::new(page))
}

fn handle_close_contract_detail(state: &mut State) -> Task<Message> {
  state.selected_contract = None;
  Task::none()
}

fn handle_contract_detail_loaded(state: &mut State, detail: Option<contract_detail::ContractDetail>) -> Task<Message> {
  state.selected_contract = detail;
  Task::none()
}

fn handle_contract_selected(state: &State, db: &Database, contract_id: i64) -> Task<Message> {
  match contract_loader_target(state, contract_id) {
    Some(ContractLoad::Character(character_id)) => {
      let db = db.clone();
      Task::perform(
        async move { contract_detail::load_for_character(&db, character_id, contract_id).await },
        |detail| Message::ContractDetailLoaded(Box::new(detail)),
      )
    }
    Some(ContractLoad::Corporation(corporation_id)) => {
      let db = db.clone();
      Task::perform(
        async move { contract_detail::load_for_corporation(&db, corporation_id, contract_id).await },
        |detail| Message::ContractDetailLoaded(Box::new(detail)),
      )
    }
    None => Task::none(),
  }
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::ChartHovered(fraction) => {
      state.chart_hover = fraction;
      Task::none()
    }
    Message::CloseContractDetail => handle_close_contract_detail(state),
    Message::ContractDetailLoaded(detail) => handle_contract_detail_loaded(state, *detail),
    Message::ContractSelected(contract_id) => handle_contract_selected(state, db, contract_id),
    Message::DivisionSelected(division) => {
      if !matches!(state.active, Scope::Corporation(_)) || division == state.active_division {
        return Task::none();
      }
      state.active_division = division;
      state.tab_scroll_offset = 0.0;
      reload(db, state.active, division)
    }
    Message::Loaded(loaded) => {
      let Loaded {
        contract_total,
        contracts,
        corp_divisions,
        corporations,
        financials,
        journal,
        journal_total,
        market,
        market_total,
        net_worth_series,
        periods,
        right_rail_width,
        roster,
      } = *loaded;
      state.contract_total = contract_total;
      state.contracts = contracts;
      state.corp_divisions = corp_divisions;
      state.corporations = corporations;
      state.financials = financials;
      state.journal = journal;
      state.journal_total = journal_total;
      state.market = market;
      state.market_total = market_total;
      state.net_worth_series = net_worth_series;
      state.periods = periods;
      if !state.right_rail.is_active() {
        state.right_rail.set_ratio_from_store(right_rail_width);
      }
      state.roster = roster;
      state.loading_more = false;
      state.tab_exhausted = false;
      state.recompute_derived();
      Task::none()
    }
    Message::MoreLoaded(page) => {
      state.loading_more = false;
      let MorePage {
        contracts,
        journal,
        market,
        tab,
        scope,
      } = *page;
      if scope != state.active || tab != state.tab {
        return Task::none();
      }
      let appended = journal.len() + market.len() + contracts.len();
      state.journal.extend(journal);
      state.market.extend(market);
      state.contracts.extend(contracts);
      state.tab_exhausted = appended == 0;
      state.recompute_derived();
      Task::none()
    }
    Message::PaneSettled(..) => Task::none(),
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Task::none()
    }
    Message::RailDragEnd => {
      state.right_rail.end();
      Task::done(Message::PaneSettled(RIGHT_RAIL_PANE_KEY, state.right_rail.ratio()))
    }
    Message::RailDragged(x) => {
      state.right_rail.drag_to(x);
      Task::none()
    }
    Message::RailDragStart => {
      state.right_rail.start();
      Task::none()
    }
    Message::ReauthRequested(_) => Task::none(),
    Message::ScopeSelected(scope) => {
      state.picker_open = false;
      if scope == state.active {
        return Task::none();
      }
      state.active = scope;
      state.active_division = DEFAULT_DIVISION;
      state.corp_divisions = Vec::new();
      state.tab_scroll_offset = 0.0;
      reload(db, scope, state.active_division)
    }
    Message::SearchChanged(query) => {
      state.search = query;
      state.tab_scroll_offset = 0.0;
      state.recompute_derived();
      Task::none()
    }
    Message::SideFilterChanged(side) => {
      state.side_filter = side;
      state.tab_scroll_offset = 0.0;
      state.recompute_derived();
      Task::none()
    }
    Message::SignFilterChanged(filter) => {
      state.sign_filter = filter;
      state.tab_scroll_offset = 0.0;
      state.recompute_derived();
      Task::none()
    }
    Message::TabScrolled {
      absolute,
      relative,
    } => {
      state.tab_scroll_offset = absolute;
      if relative < SCROLL_LOAD_THRESHOLD {
        return Task::none();
      }
      load_more(state, db)
    }
    Message::TabSelected(tab) => {
      state.tab = tab;
      state.tab_scroll_offset = 0.0;
      state.tab_exhausted = false;
      Task::none()
    }
    Message::TimeframeSelected(timeframe) => {
      state.timeframe = timeframe;
      state.chart_hover = None;
      Task::none()
    }
  }
}

pub fn view(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  let base = shell::shell(state, now);
  match state.selected_contract() {
    Some(detail) => contract_detail::overlay(base, detail, Message::CloseContractDetail),
    None => base,
  }
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  let mut subs: Vec<iced::Subscription<Message>> = Vec::new();
  if state.right_rail.is_active() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      crate::ui::components::resizable_pane::drag_event(event, Message::RailDragged, Message::RailDragEnd)
    }));
  }
  if state.selected_contract.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
          key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
          ..
        })
      )
      .then_some(Message::CloseContractDetail)
    }));
  }
  iced::Subscription::batch(subs)
}

async fn load_wallet(db: Database, scope: Scope, division: i64) -> Loaded {
  let roster = load_roster(&db).await;
  let corporations = load_corporations(&db).await;
  let financials = finance::financials_all(&db).await.unwrap_or_default();
  let periods = finance::wallet_period_summaries_all(&db).await.unwrap_or_default();

  let scope_ids = resolve_scope_ids(scope, &roster, &corporations);

  let (journal, market, contracts, corp_divisions, journal_total, market_total, contract_total) = match scope {
    Scope::Corporation(corp_id) => {
      let limit = PAGE_SIZE as i64;
      let journal = loaders::load_corp_journal(&db, corp_id, division).await;
      let market = loaders::load_corp_market(&db, corp_id, division).await;
      let contracts = loaders::load_corp_contracts_page(&db, corp_id, None, limit).await;
      let corp_divisions = load_corp_divisions(&db, corp_id).await;
      let journal_total = finance::count_journal_for_corporation(&db, corp_id, division)
        .await
        .unwrap_or(0);
      let market_total = finance::count_transactions_for_corporation(&db, corp_id, division)
        .await
        .unwrap_or(0);
      let contract_total = finance::count_contracts_for_corporation(&db, corp_id)
        .await
        .unwrap_or(0);
      (
        journal,
        market,
        contracts,
        corp_divisions,
        journal_total,
        market_total,
        contract_total,
      )
    }
    Scope::All | Scope::Character(_) => {
      let limit = PAGE_SIZE as i64;
      let journal = loaders::load_journal_page(&db, &scope_ids, None, limit).await;
      let market = loaders::load_market_page(&db, &scope_ids, None, limit).await;
      let contracts = loaders::load_contracts_page(&db, &scope_ids, None, limit).await;
      let (journal_total, market_total, contract_total) = count_character_totals(&db, &scope_ids).await;
      (
        journal,
        market,
        contracts,
        Vec::new(),
        journal_total,
        market_total,
        contract_total,
      )
    }
  };

  let net_worth_series = load_net_worth_series(&db, scope, &scope_ids, &corporations).await;

  let right_rail_width = window_state::load()
    .panes
    .get(RIGHT_RAIL_PANE_KEY)
    .copied()
    .unwrap_or(RIGHT_RAIL_DEFAULT_WIDTH);

  Loaded {
    contract_total,
    contracts,
    corp_divisions,
    corporations,
    financials,
    journal,
    journal_total,
    market,
    market_total,
    net_worth_series,
    periods,
    right_rail_width,
    roster,
  }
}

async fn count_character_totals(db: &Database, scope_ids: &[i64]) -> (i64, i64, i64) {
  let mut journal_total = 0;
  let mut market_total = 0;
  let mut contract_total = 0;
  for &id in scope_ids {
    journal_total += finance::count_journal_for_character(db, id).await.unwrap_or(0);
    market_total += finance::count_transactions_for_character(db, id).await.unwrap_or(0);
    contract_total += finance::count_contracts_for_character(db, id).await.unwrap_or(0);
  }
  (journal_total, market_total, contract_total)
}

async fn load_corp_divisions(db: &Database, corporation_id: i64) -> Vec<CorpDivision> {
  crate::store::repo::finance::divisions(db, corporation_id)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| CorpDivision {
      balance: row.balance(),
      division: row.division(),
      name: row.name().clone(),
    })
    .collect()
}

async fn load_net_worth_series(
  db: &Database,
  scope: Scope,
  scope_ids: &[i64],
  corporations: &[RosterCorp],
) -> Vec<NetWorthPoint> {
  let since = (Utc::now() - Duration::days(Timeframe::Year.days() as i64))
    .format("%Y-%m-%d")
    .to_string();

  match scope {
    Scope::Character(id) => finance::for_character_since(db, id, &since)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|row| NetWorthPoint {
        date: row.date().clone(),
        liquid: row.liquid(),
        net_worth: row.net_worth(),
      })
      .collect(),
    Scope::All => {
      let _ = scope_ids;
      let mut by_date: std::collections::BTreeMap<String, (f64, f64)> = std::collections::BTreeMap::new();
      for row in finance::combined_series_since(db, &since).await.unwrap_or_default() {
        if let Some(net_worth) = row.net_worth() {
          let entry = by_date.entry(row.date().clone()).or_insert((0.0, 0.0));
          entry.0 += net_worth;
          entry.1 += row.liquid().unwrap_or(0.0);
        }
      }
      for corp in corporations {
        for row in finance::for_corporation_since(db, corp.id, &since)
          .await
          .unwrap_or_default()
        {
          let entry = by_date.entry(row.date().clone()).or_insert((0.0, 0.0));
          entry.0 += row.net_worth();
          entry.1 += row.liquid();
        }
      }
      by_date
        .into_iter()
        .map(|(date, (net_worth, liquid))| NetWorthPoint {
          date,
          liquid,
          net_worth,
        })
        .collect()
    }
    Scope::Corporation(id) => finance::for_corporation_since(db, id, &since)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|row| NetWorthPoint {
        date: row.date().clone(),
        liquid: row.liquid(),
        net_worth: row.net_worth(),
      })
      .collect(),
  }
}

fn resolve_scope_ids(scope: Scope, roster: &[RosterPilot], _corporations: &[RosterCorp]) -> Vec<i64> {
  match scope {
    Scope::All => roster.iter().map(|pilot| pilot.id).collect(),
    Scope::Character(id) => vec![id],
    Scope::Corporation(_) => Vec::new(),
  }
}

async fn load_roster(db: &Database) -> Vec<RosterPilot> {
  let characters = character::all_owned(db).await.unwrap_or_default();
  let financials = finance::financials_all(db).await.unwrap_or_default();
  let liquid_by_id: std::collections::HashMap<i64, Option<f64>> =
    financials.iter().map(|row| (row.character_id, row.liquid)).collect();
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
      liquid: liquid_by_id.get(&character.id()).copied().flatten(),
      name: character.name().to_owned(),
      portrait,
    });
  }
  roster
}

async fn load_corporations(db: &Database) -> Vec<RosterCorp> {
  let corporations = org::all_owned_corporations(db).await.unwrap_or_default();
  let mut roster = Vec::with_capacity(corporations.len());
  for corp in corporations {
    let liquid = sum_option(
      finance::divisions(db, corp.id())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|division| division.balance()),
    );
    let logo = images::resolve(&images::default_store(), images::ImageKind::CorporationLogo, corp.id());
    roster.push(RosterCorp {
      id: corp.id(),
      liquid,
      logo,
      name: corp.name().to_owned(),
      ticker: corp.ticker().to_owned(),
    });
  }
  roster
}

pub fn scope_liquid(state: &State) -> Option<f64> {
  match state.active {
    Scope::All => combined_liquid(state),
    Scope::Character(_) => {
      let ids: std::collections::HashSet<i64> = state.scope_ids().into_iter().collect();
      sum_option(
        state
          .financials
          .iter()
          .filter(|row| ids.contains(&row.character_id))
          .map(|row| row.liquid),
      )
    }
    Scope::Corporation(_) => state.corp_balance_total(),
  }
}

/// Liquid ISK across every owned character and corporation, independent of the active scope.
pub fn combined_liquid(state: &State) -> Option<f64> {
  let ids: std::collections::HashSet<i64> = state.roster.iter().map(|pilot| pilot.id).collect();
  let character_liquid = sum_option(
    state
      .financials
      .iter()
      .filter(|row| ids.contains(&row.character_id))
      .map(|row| row.liquid),
  );
  sum_option([character_liquid, state.owned_corp_liquid()].into_iter())
}

#[allow(dead_code)]
pub fn period_totals(state: &State) -> PeriodTotals {
  let ids: std::collections::HashSet<i64> = state.scope_ids().into_iter().collect();
  let mut totals = PeriodTotals::default();
  for summary in state.periods.iter().filter(|row| ids.contains(&row.character_id)) {
    totals.income += summary.income;
    totals.spend += summary.spend;
  }
  totals.net = totals.income - totals.spend;
  totals
}

pub fn timeframe_window(timeframe: Timeframe, today: NaiveDate) -> (NaiveDate, NaiveDate) {
  let span = timeframe.days().saturating_sub(1) as i64;
  (today - Duration::days(span), today)
}

pub fn sliced_series(state: &State, today: NaiveDate) -> &[NetWorthPoint] {
  let (start, _) = timeframe_window(state.timeframe, today);
  let cutoff = start.format("%Y-%m-%d").to_string();
  let series = &state.net_worth_series;
  let start = series.partition_point(|point| point.date.as_str() < cutoff.as_str());
  &series[start..]
}

pub fn series_change(series: &[NetWorthPoint]) -> f64 {
  match (series.first(), series.last()) {
    (Some(first), Some(last)) if series.len() >= 2 => last.net_worth - first.net_worth,
    _ => 0.0,
  }
}

pub fn series_current(series: &[NetWorthPoint]) -> Option<f64> {
  series.last().map(|point| point.net_worth)
}

pub fn scope_composition(state: &State) -> Composition {
  let ids: std::collections::HashSet<i64> = state.scope_ids().into_iter().collect();
  let rows: Vec<&CharacterFinancials> = state
    .financials
    .iter()
    .filter(|row| ids.contains(&row.character_id))
    .collect();
  Composition {
    asset_value: sum_option(rows.iter().map(|row| row.asset_value)),
    escrow: sum_option(rows.iter().map(|row| row.escrow)),
    liquid: scope_liquid(state),
  }
}

pub fn composition_stack(state: &State) -> Vec<CompositionSlice> {
  if !matches!(state.active, Scope::All) {
    return Vec::new();
  }
  let financials_by_id: std::collections::HashMap<i64, &_> =
    state.financials.iter().map(|row| (row.character_id, row)).collect();
  let mut slices: Vec<CompositionSlice> = state
    .roster
    .iter()
    .filter_map(|pilot| {
      let net_worth = financials_by_id.get(&pilot.id).and_then(|row| row.net_worth)?;
      Some(CompositionSlice {
        id: pilot.id,
        name: pilot.name.clone(),
        net_worth,
      })
    })
    .collect();
  slices.sort_by(|a, b| b.net_worth.total_cmp(&a.net_worth));
  slices
}

fn sum_option(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
  let present: Vec<f64> = values.flatten().collect();
  if present.is_empty() {
    None
  } else {
    Some(present.iter().sum())
  }
}

pub fn filtered_journal(state: &State) -> Vec<&JournalEntry> {
  state
    .derived
    .journal_indices
    .iter()
    .map(|&index| &state.journal[index])
    .collect()
}

pub fn filtered_market(state: &State) -> Vec<&MarketEntry> {
  state
    .derived
    .market_indices
    .iter()
    .map(|&index| &state.market[index])
    .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContractLoad {
  Character(i64),
  Corporation(i64),
}

/// Resolves which detail loader a clicked contract row needs.
///
/// Corporation scope always loads from the corp tables. Under character or all
/// scope the row's own `character_id` is used, so an all-wallets list still
/// loads each contract from the character that owns it.
fn contract_loader_target(state: &State, contract_id: i64) -> Option<ContractLoad> {
  if let Scope::Corporation(corporation_id) = state.active {
    return Some(ContractLoad::Corporation(corporation_id));
  }
  state
    .contracts
    .iter()
    .find(|entry| entry.contract_id == contract_id)
    .map(|entry| ContractLoad::Character(entry.character_id))
}

pub fn filtered_contracts(state: &State) -> Vec<&ContractEntry> {
  state
    .derived
    .contract_indices
    .iter()
    .map(|&index| &state.contracts[index])
    .collect()
}

fn contract_matches(entry: &ContractEntry, side: Side, query: &str) -> bool {
  match side {
    Side::Buy if !entry.is_buy => return false,
    Side::Sell if entry.is_buy => return false,
    _ => {}
  }
  if query.is_empty() {
    return true;
  }
  entry.contract_id.to_string().contains(query)
    || entry.r#type.to_lowercase().contains(query)
    || entry.status.to_lowercase().contains(query)
    || entry
      .issuer
      .as_deref()
      .is_some_and(|name| name.to_lowercase().contains(query))
    || entry
      .assignee
      .as_deref()
      .is_some_and(|name| name.to_lowercase().contains(query))
    || entry
      .acceptor
      .as_deref()
      .is_some_and(|name| name.to_lowercase().contains(query))
}

fn journal_matches(entry: &JournalEntry, sign: SignFilter, query: &str) -> bool {
  match sign {
    SignFilter::In if !entry.is_income() => return false,
    SignFilter::Out if !entry.amount.is_some_and(|amount| amount < 0.0) => return false,
    _ => {}
  }
  if !query.is_empty()
    && !entry.ref_type.to_lowercase().contains(query)
    && !entry.description.to_lowercase().contains(query)
  {
    return false;
  }
  true
}

fn humanize_ref_type(ref_type: &str) -> String {
  if ref_type.is_empty() {
    return "\u{2014}".to_owned();
  }
  ref_type
    .split('_')
    .map(|word| {
      let mut chars = word.chars();
      match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
      }
    })
    .collect::<Vec<_>>()
    .join(" ")
}

pub fn journal_type_glyph(entry: &JournalEntry) -> (&'static str, bool) {
  use crate::ui::components::glyph_badge::{GLYPH_EXPENSE, GLYPH_INCOME};

  let is_in = match entry.amount {
    Some(amount) if amount != 0.0 => amount > 0.0,
    _ => is_income_ref_type(&entry.ref_type),
  };
  if is_in {
    (GLYPH_INCOME, true)
  } else {
    (GLYPH_EXPENSE, false)
  }
}

fn is_income_ref_type(ref_type: &str) -> bool {
  is_known_income_ref_type(ref_type)
    || (!ref_type.contains("fee") && !ref_type.contains("tax") && !ref_type.contains("cost"))
}

fn is_known_income_ref_type(ref_type: &str) -> bool {
  matches!(
    ref_type,
    "bounty_prizes"
      | "agent_mission_reward"
      | "agent_mission_time_bonus_reward"
      | "market_transaction"
      | "insurance"
      | "player_donation"
      | "contract_price_payment_corp"
      | "lp_store"
      | "project_reward"
      | "industry_job_tax"
  )
}

pub fn journal_flow(entries: &[&JournalEntry]) -> JournalFlow {
  let mut flow = JournalFlow::default();
  for entry in entries {
    match entry.amount {
      Some(amount) if amount > 0.0 => flow.income += amount,
      Some(amount) if amount < 0.0 => flow.spend += -amount,
      _ => {}
    }
  }
  flow.net = flow.income - flow.spend;
  flow
}

pub fn category_flows(entries: &[&JournalEntry]) -> Vec<CategoryFlow> {
  let mut by_type: std::collections::HashMap<&str, CategoryFlow> = std::collections::HashMap::new();
  for entry in entries {
    let bucket = by_type.entry(entry.ref_type.as_str()).or_insert_with(|| CategoryFlow {
      ref_type: entry.ref_type.clone(),
      ..CategoryFlow::default()
    });
    match entry.amount {
      Some(amount) if amount > 0.0 => bucket.income += amount,
      Some(amount) if amount < 0.0 => bucket.spend += -amount,
      _ => {}
    }
  }
  let mut flows: Vec<CategoryFlow> = by_type.into_values().filter(|flow| flow.total() > 0.0).collect();
  flows.sort_by(|a, b| b.total().total_cmp(&a.total()));
  flows
}

fn market_matches(entry: &MarketEntry, sign: SignFilter, side: Side, query: &str) -> bool {
  match sign {
    SignFilter::In if entry.is_buy => return false,
    SignFilter::Out if !entry.is_buy => return false,
    _ => {}
  }
  match side {
    Side::Buy if !entry.is_buy => return false,
    Side::Sell if entry.is_buy => return false,
    _ => {}
  }
  query.is_empty() || entry.item.to_lowercase().contains(query) || entry.location.to_lowercase().contains(query)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn pilot(id: i64, liquid: Option<f64>) -> RosterPilot {
    RosterPilot {
      corp: "TST".to_owned(),
      granted_scopes: None,
      id,
      liquid,
      name: format!("Pilot {id}"),
      portrait: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      },
    }
  }

  fn financials(character_id: i64, liquid: Option<f64>) -> CharacterFinancials {
    CharacterFinancials {
      character_id,
      liquid,
      asset_value: None,
      escrow: None,
      net_worth: liquid,
    }
  }

  fn corp_division(division: i64, name: Option<&str>, balance: Option<f64>) -> CorpDivision {
    CorpDivision {
      balance,
      division,
      name: name.map(str::to_owned),
    }
  }

  fn corp(id: i64, name: &str) -> RosterCorp {
    RosterCorp {
      id,
      liquid: None,
      logo: corp_logo_stale(id),
      name: name.to_owned(),
      ticker: "TSTC".to_owned(),
    }
  }

  fn corp_with_liquid(id: i64, liquid: Option<f64>) -> RosterCorp {
    RosterCorp {
      id,
      liquid,
      logo: corp_logo_stale(id),
      name: format!("Corp {id}"),
      ticker: "TSTC".to_owned(),
    }
  }

  fn corp_logo_stale(id: i64) -> images::ImageState {
    images::ImageState::Stale {
      id,
      kind: images::ImageKind::CorporationLogo,
    }
  }

  mod loads_data {
    use super::*;

    #[test]
    fn it_flags_a_load_message_for_an_image_recheck() {
      assert!(Message::Loaded(Box::new(load_wallet_for_test())).loads_data());
    }

    #[test]
    fn it_does_not_flag_an_interaction_message() {
      assert!(!Message::TabSelected(Tab::Market).loads_data());
      assert!(!Message::SearchChanged("rifter".to_owned()).loads_data());
      assert!(!Message::ChartHovered(Some(0.5)).loads_data());
    }
  }

  fn load_wallet_for_test() -> Loaded {
    Loaded {
      contract_total: 0,
      contracts: Vec::new(),
      corp_divisions: Vec::new(),
      corporations: Vec::new(),
      financials: Vec::new(),
      journal: Vec::new(),
      journal_total: 0,
      market: Vec::new(),
      market_total: 0,
      net_worth_series: Vec::new(),
      periods: Vec::new(),
      right_rail_width: 280.0,
      roster: Vec::new(),
    }
  }

  mod mark_dirty {
    use super::*;

    #[test]
    fn it_marks_the_wallet_dirty_for_a_ledger_kind() {
      let mut state = State::new();

      state.mark_dirty(JobKind::CharacterWallet);

      assert!(state.is_dirty());
    }

    #[test]
    fn it_ignores_a_kind_the_wallet_does_not_render() {
      let mut state = State::new();

      state.mark_dirty(JobKind::AssetSync);

      assert!(!state.is_dirty());
    }
  }

  mod reload_kind {
    use super::*;

    #[test]
    fn it_feeds_the_wallet_for_every_ledger_and_derive_kind() {
      assert!(reload_kind(JobKind::CharacterWallet));
      assert!(reload_kind(JobKind::CorporationWallet));
      assert!(reload_kind(JobKind::MarketPrices));
      assert!(reload_kind(JobKind::NetWorthSnapshot));
    }

    #[test]
    fn it_ignores_kinds_the_wallet_does_not_render() {
      assert!(!reload_kind(JobKind::AssetSync));
      assert!(!reload_kind(JobKind::CharacterSkills));
      assert!(!reload_kind(JobKind::CharacterProfile));
    }
  }

  mod scope_gate {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_gates_a_character_scope_missing_the_wallet_scopes() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.active = Scope::Character(1);

      let gate = state.scope_gate().expect("missing scope should gate");

      assert_eq!(gate.0, 1);
      assert!(!gate.2.is_empty());
    }

    #[test]
    fn it_does_not_gate_a_character_with_the_wallet_scopes() {
      let granted = crate::features::registry::descriptor(crate::config::Feature::Wallet)
        .scopes
        .join(" ");
      let mut granted_pilot = pilot(1, None);
      granted_pilot.granted_scopes = Some(granted);
      let mut state = State::new();
      state.roster = vec![granted_pilot];
      state.active = Scope::Character(1);

      assert!(state.scope_gate().is_none());
    }

    #[test]
    fn it_does_not_gate_the_all_or_corporation_scopes() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.active = Scope::All;

      assert!(state.scope_gate().is_none());

      state.active = Scope::Corporation(99);

      assert!(state.scope_gate().is_none());
    }
  }

  mod scope_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_every_pilot_for_all_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.active = Scope::All;

      assert_eq!(state.scope_ids(), vec![1, 2]);
    }

    #[test]
    fn it_returns_the_single_id_for_a_character_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.active = Scope::Character(2);

      assert_eq!(state.scope_ids(), vec![2]);
    }

    #[test]
    fn it_returns_no_character_ids_for_a_corp_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.active = Scope::Corporation(98_000_001);

      assert!(state.scope_ids().is_empty());
    }
  }

  mod stale_images {
    use pretty_assertions::assert_eq;

    use super::*;

    fn fresh_portrait() -> images::ImageState {
      images::ImageState::Fresh(std::path::PathBuf::from("/cache/characters/1.jpg"))
    }

    #[test]
    fn it_is_empty_when_every_model_image_is_fresh() {
      let mut state = State::new();
      state.roster = vec![RosterPilot {
        corp: "TST".to_owned(),
        granted_scopes: None,
        id: 1,
        liquid: None,
        name: "Pilot 1".to_owned(),
        portrait: fresh_portrait(),
      }];
      state.corporations = vec![RosterCorp {
        id: 98_000_001,
        liquid: None,
        logo: images::ImageState::Fresh(std::path::PathBuf::from("/cache/corporations/98000001.png")),
        name: "Corp".to_owned(),
        ticker: "TSTC".to_owned(),
      }];

      assert!(state.stale_images().is_empty());
    }

    #[test]
    fn it_collects_the_stale_portrait_and_logo_keys() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.corporations = vec![corp(98_000_001, "Corp")];

      let keys = state.stale_images();

      assert!(keys.contains(&(images::ImageKind::CharacterPortrait, 1)));
      assert!(keys.contains(&(images::ImageKind::CorporationLogo, 98_000_001)));
    }

    #[test]
    fn it_deduplicates_repeated_keys() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(1, None)];

      assert_eq!(state.stale_images(), vec![(images::ImageKind::CharacterPortrait, 1)]);
    }
  }

  mod load_more {
    use super::*;

    async fn ready_state(tab: Tab) -> State {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.active = Scope::All;
      state.tab = tab;
      state
    }

    #[tokio::test]
    async fn it_no_ops_while_a_page_is_already_loading() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = ready_state(Tab::Journal).await;
      state.loading_more = true;

      let _ = super::load_more(&mut state, &db);

      assert!(state.loading_more);
    }

    #[tokio::test]
    async fn it_no_ops_when_the_tab_is_exhausted() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = ready_state(Tab::Journal).await;
      state.tab_exhausted = true;

      let _ = super::load_more(&mut state, &db);

      assert!(!state.loading_more);
    }

    #[tokio::test]
    async fn it_no_ops_for_a_corporation_scope() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = ready_state(Tab::Journal).await;
      state.active = Scope::Corporation(98_000_001);

      let _ = super::load_more(&mut state, &db);

      assert!(!state.loading_more);
    }

    #[tokio::test]
    async fn it_no_ops_when_no_characters_are_in_scope() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.active = Scope::All;
      state.tab = Tab::Journal;

      let _ = super::load_more(&mut state, &db);

      assert!(!state.loading_more);
    }

    #[tokio::test]
    async fn it_starts_a_page_load_for_each_tab() {
      let db = crate::store::open_test().await.unwrap();

      for tab in [Tab::Journal, Tab::Market, Tab::Contracts] {
        let mut state = ready_state(tab).await;

        let _task = super::load_more(&mut state, &db);

        assert!(state.loading_more, "starting a {tab:?} page marks the state loading");
      }
    }

    #[tokio::test]
    async fn it_starts_a_page_load_from_the_last_entry_cursor_for_each_tab() {
      let db = crate::store::open_test().await.unwrap();

      let mut journal = ready_state(Tab::Journal).await;
      journal.journal = vec![journal_entry(1, Some(1.0), "player_trading", "trade")];
      let _journal_task = super::load_more(&mut journal, &db);
      assert!(journal.loading_more);

      let mut market = ready_state(Tab::Market).await;
      market.market = vec![market_entry(1, true, "Tritanium", "Jita")];
      let _market_task = super::load_more(&mut market, &db);
      assert!(market.loading_more);

      let mut contracts = ready_state(Tab::Contracts).await;
      contracts.contracts = vec![contract_entry(1, false, "finished", "item_exchange")];
      let _contracts_task = super::load_more(&mut contracts, &db);
      assert!(contracts.loading_more);
    }

    #[tokio::test]
    async fn it_starts_a_corp_contracts_page_load() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = ready_state(Tab::Contracts).await;
      state.active = Scope::Corporation(98_000_001);

      let _task = super::load_more(&mut state, &db);

      assert!(state.loading_more);
    }

    #[tokio::test]
    async fn it_starts_a_corp_contracts_page_load_from_the_last_entry_cursor() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = ready_state(Tab::Contracts).await;
      state.active = Scope::Corporation(98_000_001);
      state.contracts = vec![contract_entry(1, false, "finished", "item_exchange")];

      let _task = super::load_more(&mut state, &db);

      assert!(state.loading_more);
    }

    #[tokio::test]
    async fn it_no_ops_for_a_corporation_scope_on_a_non_contract_tab() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = ready_state(Tab::Market).await;
      state.active = Scope::Corporation(98_000_001);

      let _ = super::load_more(&mut state, &db);

      assert!(!state.loading_more);
    }
  }

  mod corp_balance_total {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_the_synced_division_balances() {
      let mut state = State::new();
      state.active = Scope::Corporation(98_000_001);
      state.corp_divisions = vec![
        corp_division(1, Some("Master"), Some(1_000.0)),
        corp_division(2, None, Some(250.0)),
        corp_division(3, None, None),
      ];

      assert_eq!(state.corp_balance_total(), Some(1_250.0));
    }

    #[test]
    fn it_is_none_when_no_division_has_a_balance() {
      let mut state = State::new();
      state.active = Scope::Corporation(98_000_001);
      state.corp_divisions = vec![corp_division(1, Some("Master"), None)];

      assert_eq!(state.corp_balance_total(), None);
    }
  }

  mod corp_division_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_the_synced_name_when_present() {
      assert_eq!(corp_division(2, Some("Trading"), None).label(), "Trading");
    }

    #[test]
    fn it_falls_back_to_a_division_number_label_when_unnamed() {
      assert_eq!(corp_division(4, None, None).label(), "Division 4");
    }
  }

  mod scope_liquid {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_liquid_across_the_in_scope_characters() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0)), pilot(2, Some(50.0))];
      state.financials = vec![financials(1, Some(100.0)), financials(2, Some(50.0))];
      state.active = Scope::All;

      assert_eq!(super::scope_liquid(&state), Some(150.0));
    }

    #[test]
    fn it_excludes_out_of_scope_characters() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0)), pilot(2, Some(50.0))];
      state.financials = vec![financials(1, Some(100.0)), financials(2, Some(50.0))];
      state.active = Scope::Character(1);

      assert_eq!(super::scope_liquid(&state), Some(100.0));
    }

    #[test]
    fn it_uses_summed_division_balances_for_a_corp_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0))];
      state.financials = vec![financials(1, Some(100.0))];
      state.active = Scope::Corporation(98_000_001);
      state.corp_divisions = vec![
        corp_division(1, Some("Master"), Some(500.0)),
        corp_division(2, None, Some(200.0)),
      ];

      assert_eq!(super::scope_liquid(&state), Some(700.0));
    }

    #[test]
    fn it_returns_none_when_no_in_scope_character_has_a_synced_balance() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.financials = vec![financials(1, None)];
      state.active = Scope::All;

      assert_eq!(super::scope_liquid(&state), None);
    }

    #[test]
    fn it_adds_owned_corporation_balances_under_the_all_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0))];
      state.financials = vec![financials(1, Some(100.0))];
      state.corporations = vec![corp_with_liquid(98_000_001, Some(700.0))];
      state.active = Scope::All;

      assert_eq!(super::scope_liquid(&state), Some(800.0));
    }

    #[test]
    fn it_includes_corporation_balances_even_when_no_character_has_liquid() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.financials = vec![financials(1, None)];
      state.corporations = vec![corp_with_liquid(98_000_001, Some(250.0))];
      state.active = Scope::All;

      assert_eq!(super::scope_liquid(&state), Some(250.0));
    }

    #[test]
    fn it_excludes_corporation_balances_under_a_character_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0))];
      state.financials = vec![financials(1, Some(100.0))];
      state.corporations = vec![corp_with_liquid(98_000_001, Some(700.0))];
      state.active = Scope::Character(1);

      assert_eq!(super::scope_liquid(&state), Some(100.0));
    }
  }

  mod load_roster {
    #[tokio::test]
    async fn it_yields_an_empty_roster_against_a_bare_store() {
      let db = crate::store::open_test().await.unwrap();

      assert!(super::super::load_roster(&db).await.is_empty());
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_records_the_loaded_roster_and_ledgers() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::Loaded(Box::new(Loaded {
          contract_total: 0,
          contracts: vec![],
          corp_divisions: vec![],
          corporations: vec![],
          financials: vec![financials(7, Some(10.0))],
          journal: vec![],
          journal_total: 0,
          market: vec![],
          market_total: 0,
          net_worth_series: vec![],
          periods: vec![],
          right_rail_width: 280.0,
          roster: vec![pilot(7, Some(10.0))],
        })),
        &db,
      );

      assert_eq!(state.roster, vec![pilot(7, Some(10.0))]);
      assert_eq!(state.financials.len(), 1);
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
    async fn it_opens_and_closes_the_contract_detail_modal() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::ContractDetailLoaded(Box::new(Some(contract_detail_fixture()))),
        &db,
      );
      assert!(state.selected_contract.is_some());

      let _ = update(&mut state, Message::CloseContractDetail, &db);
      assert!(state.selected_contract.is_none());
    }

    #[tokio::test]
    async fn selecting_a_contract_row_leaves_the_modal_closed_until_the_load_resolves() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.contracts = vec![contract_entry(7, false, "finished", "item_exchange")];

      let _ = update(&mut state, Message::ContractSelected(12_345), &db);

      assert!(state.selected_contract.is_none());
    }

    #[tokio::test]
    async fn selecting_an_unknown_contract_row_is_a_no_op() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::ContractSelected(999), &db);

      assert!(state.selected_contract.is_none());
    }

    #[tokio::test]
    async fn selecting_a_corp_scope_resets_the_active_division_to_the_master() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.active = Scope::Corporation(1);
      state.active_division = 5;
      state.corp_divisions = vec![corp_division(5, None, Some(1.0))];

      let _ = update(&mut state, Message::ScopeSelected(Scope::Corporation(98_000_001)), &db);

      assert_eq!(state.active, Scope::Corporation(98_000_001));
      assert_eq!(state.active_division, DEFAULT_DIVISION);
      assert!(state.corp_divisions.is_empty());
    }

    #[tokio::test]
    async fn it_records_the_selected_division_in_corp_scope() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.active = Scope::Corporation(98_000_001);

      let _ = update(&mut state, Message::DivisionSelected(3), &db);

      assert_eq!(state.active_division, 3);
    }

    #[tokio::test]
    async fn it_ignores_a_division_selection_outside_corp_scope() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.active = Scope::All;

      let _ = update(&mut state, Message::DivisionSelected(3), &db);

      assert_eq!(state.active_division, DEFAULT_DIVISION);
    }

    #[tokio::test]
    async fn it_switches_the_active_tab() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::TabSelected(Tab::Journal), &db);
      assert_eq!(state.tab, Tab::Journal);
    }

    #[tokio::test]
    async fn it_records_the_search_and_sign_filter() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::SearchChanged("tritanium".to_owned()), &db);
      assert_eq!(state.search, "tritanium");

      let _ = update(&mut state, Message::SignFilterChanged(SignFilter::In), &db);
      assert_eq!(state.sign_filter, SignFilter::In);
    }

    #[tokio::test]
    async fn it_tracks_the_absolute_scroll_offset_for_windowing() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::TabScrolled {
          absolute: 1_234.0,
          relative: 0.5,
        },
        &db,
      );

      assert_eq!(
        state.tab_scroll_offset(),
        1_234.0,
        "the pixel offset is stored so the virtual list can window the ledger"
      );
    }

    #[tokio::test]
    async fn it_loads_the_next_page_when_scrolled_near_the_bottom() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.active = Scope::All;

      let _ = update(
        &mut state,
        Message::TabScrolled {
          absolute: 9_000.0,
          relative: 0.9,
        },
        &db,
      );

      assert!(state.loading_more, "a deep scroll starts the next cursor page");
    }

    #[tokio::test]
    async fn it_does_not_load_a_page_for_a_shallow_scroll() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.active = Scope::All;

      let _ = update(
        &mut state,
        Message::TabScrolled {
          absolute: 100.0,
          relative: 0.2,
        },
        &db,
      );

      assert!(!state.loading_more, "a shallow scroll does not page");
    }

    #[tokio::test]
    async fn it_resets_the_scroll_offset_when_the_tab_changes() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.tab_scroll_offset = 4_200.0;

      let _ = update(&mut state, Message::TabSelected(Tab::Journal), &db);
      assert_eq!(state.tab_scroll_offset(), 0.0);
    }

    #[tokio::test]
    async fn it_resizes_the_right_rail_through_a_drag() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      let start = state.right_rail.width();

      let _ = update(&mut state, Message::RailDragStart, &db);
      let _ = update(&mut state, Message::RailDragged(500.0), &db);
      let _ = update(&mut state, Message::RailDragged(540.0), &db);
      let _ = update(&mut state, Message::RailDragEnd, &db);

      assert_eq!(state.right_rail.width(), start - 40.0);
    }

    #[tokio::test]
    async fn it_appends_a_more_page_matching_the_active_scope_and_tab() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.loading_more = true;
      let (tab, scope) = (state.tab, state.active);

      let _ = update(
        &mut state,
        Message::MoreLoaded(Box::new(MorePage {
          contracts: vec![],
          journal: vec![journal_entry(7, Some(5.0), "bounty", "kill")],
          market: vec![],
          tab,
          scope,
        })),
        &db,
      );

      assert_eq!(state.journal.len(), 1);
      assert!(!state.loading_more);
      assert!(!state.tab_exhausted);
    }

    #[tokio::test]
    async fn it_marks_the_tab_exhausted_when_a_more_page_is_empty() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.loading_more = true;
      let (tab, scope) = (state.tab, state.active);

      let _ = update(
        &mut state,
        Message::MoreLoaded(Box::new(MorePage {
          contracts: vec![],
          journal: vec![],
          market: vec![],
          tab,
          scope,
        })),
        &db,
      );

      assert!(state.tab_exhausted);
    }

    #[tokio::test]
    async fn it_drops_a_more_page_for_a_stale_scope_or_tab() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.loading_more = true;
      let tab = state.tab;

      let _ = update(
        &mut state,
        Message::MoreLoaded(Box::new(MorePage {
          contracts: vec![],
          journal: vec![journal_entry(7, Some(5.0), "bounty", "kill")],
          market: vec![],
          tab,
          scope: Scope::Character(99),
        })),
        &db,
      );

      assert!(state.journal.is_empty());
      assert!(!state.loading_more);
    }

    #[tokio::test]
    async fn it_no_ops_when_the_selected_scope_is_already_active() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.picker_open = true;
      let active = state.active;

      let _ = update(&mut state, Message::ScopeSelected(active), &db);

      assert!(!state.picker_open);
      assert_eq!(state.active, active);
    }

    #[tokio::test]
    async fn it_records_the_side_filter() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::SideFilterChanged(Side::Buy), &db);

      assert_eq!(state.side_filter, Side::Buy);
      assert_eq!(state.tab_scroll_offset(), 0.0);
    }

    #[tokio::test]
    async fn it_selects_a_timeframe_and_clears_the_chart_hover() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.chart_hover = Some(0.5);

      let _ = update(&mut state, Message::TimeframeSelected(Timeframe::Year), &db);

      assert_eq!(state.timeframe, Timeframe::Year);
      assert_eq!(state.chart_hover, None);
    }

    #[tokio::test]
    async fn it_records_the_chart_hover_fraction() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::ChartHovered(Some(0.25)), &db);

      assert_eq!(state.chart_hover, Some(0.25));
    }

    #[tokio::test]
    async fn it_no_ops_on_a_settled_pane() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::PaneSettled(RIGHT_RAIL_PANE_KEY, 320.0), &db);

      assert!(!state.right_rail.is_active());
    }
  }

  fn journal_entry(character_id: i64, amount: Option<f64>, ref_type: &str, description: &str) -> JournalEntry {
    JournalEntry {
      amount,
      balance: None,
      character_id,
      date: "2026-05-30T12:00:00Z".to_owned(),
      description: description.to_owned(),
      id: 1,
      ref_type: ref_type.to_owned(),
    }
  }

  fn market_entry(character_id: i64, is_buy: bool, item: &str, location: &str) -> MarketEntry {
    MarketEntry {
      character_id,
      date: "2026-05-30T12:00:00Z".to_owned(),
      is_buy,
      item: item.to_owned(),
      location: location.to_owned(),
      quantity: 1,
      total: 1.0,
      transaction_id: 1,
      type_icon: images::IconResolution::Missing,
      type_id: 34,
      unit_price: 1.0,
    }
  }

  fn contract_entry(character_id: i64, is_buy: bool, status: &str, contract_type: &str) -> ContractEntry {
    ContractEntry {
      acceptor: None,
      acceptor_id: None,
      acceptor_image: PartyImage::default(),
      assignee: Some("Assignee Pilot".to_owned()),
      assignee_id: Some(98_765),
      assignee_image: PartyImage::default(),
      character_id,
      collateral: Some(5_000.0),
      contract_id: 12_345,
      date_expired: None,
      date_issued: "2026-05-30T12:00:00Z".to_owned(),
      is_buy,
      issuer: Some("Issuer Pilot".to_owned()),
      issuer_id: 11_111,
      issuer_image: PartyImage::default(),
      status: status.to_owned(),
      value: Some(200.0),
      r#type: contract_type.to_owned(),
    }
  }

  fn contract_detail_fixture() -> contract_detail::ContractDetail {
    contract_detail::ContractDetail {
      acceptor: None,
      availability: "Public".to_owned(),
      bids: Vec::new(),
      buyout: None,
      collateral: None,
      contract_id: 12_345,
      days_to_complete: Some(0),
      expiry: contract_detail::ExpiryView {
        future: true,
        label: "Open".to_owned(),
        title: "Expires",
      },
      headline: 200.0,
      headline_label: "Price",
      issued_time: "2026-05-30T12:00:00Z".to_owned(),
      issuer: contract_detail::PartyView {
        name: "Issuer Pilot".to_owned(),
        portrait: images::ImageState::Fresh("/tmp/p.jpg".into()),
        role: "Issuer",
        sub: None,
      },
      items: Vec::new(),
      items_value: 0.0,
      kind: contract_detail::ContractKind::ItemExchange,
      location_name: "Jita IV - Moon 4".to_owned(),
      route: None,
      status: "outstanding".to_owned(),
      title: "Test Contract".to_owned(),
      volume: 0.0,
    }
  }

  fn period(character_id: i64, income: f64, spend: f64) -> CharacterWalletPeriodSummary {
    CharacterWalletPeriodSummary {
      character_id,
      period: "2026-05".to_owned(),
      income,
      spend,
      net: income - spend,
    }
  }

  fn nw_point(date: &str, net_worth: f64) -> NetWorthPoint {
    NetWorthPoint {
      date: date.to_owned(),
      liquid: 0.0,
      net_worth,
    }
  }

  mod sliced_series {
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;

    use super::*;

    fn day(date: &str) -> NaiveDate {
      NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn it_keeps_only_points_within_the_timeframe_window() {
      let mut state = State::new();
      state.net_worth_series = (1..=20)
        .map(|d| nw_point(&format!("2026-06-{d:02}"), d as f64))
        .collect();
      state.timeframe = Timeframe::Week;

      let sliced = super::sliced_series(&state, day("2026-06-20"));

      assert_eq!(sliced.len(), 7);
      assert_eq!(sliced[0].net_worth, 14.0);
      assert_eq!(sliced[6].net_worth, 20.0);
    }

    #[test]
    fn it_returns_the_whole_series_when_it_fits_inside_the_window() {
      let mut state = State::new();
      state.net_worth_series = vec![nw_point("2026-06-01", 1.0), nw_point("2026-06-02", 2.0)];
      state.timeframe = Timeframe::Year;

      assert_eq!(super::sliced_series(&state, day("2026-06-02")).len(), 2);
    }

    #[test]
    fn it_does_not_widen_the_window_when_points_are_sparse() {
      let mut state = State::new();
      state.net_worth_series = vec![
        nw_point("2026-01-01", 1.0),
        nw_point("2026-05-30", 2.0),
        nw_point("2026-06-09", 3.0),
      ];
      state.timeframe = Timeframe::Week;

      let sliced = super::sliced_series(&state, day("2026-06-09"));

      assert_eq!(sliced.len(), 1);
      assert_eq!(sliced[0].net_worth, 3.0);
    }
  }

  mod series_change {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_last_minus_first() {
      let series = [nw_point("a", 100.0), nw_point("b", 130.0), nw_point("c", 160.0)];

      assert_eq!(super::series_change(&series), 60.0);
    }

    #[test]
    fn it_is_zero_for_a_single_point_or_empty_series() {
      assert_eq!(super::series_change(&[nw_point("a", 5.0)]), 0.0);
      assert_eq!(super::series_change(&[]), 0.0);
    }

    #[test]
    fn it_is_negative_when_net_worth_falls() {
      let series = [nw_point("a", 200.0), nw_point("b", 150.0)];

      assert_eq!(super::series_change(&series), -50.0);
    }
  }

  mod scope_composition {
    use pretty_assertions::assert_eq;

    use super::*;

    fn financials_full(character_id: i64, liquid: f64, assets: f64, escrow: f64) -> CharacterFinancials {
      CharacterFinancials {
        character_id,
        liquid: Some(liquid),
        asset_value: Some(assets),
        escrow: Some(escrow),
        net_worth: Some(liquid + assets + escrow),
      }
    }

    #[test]
    fn it_sums_each_figure_across_the_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.financials = vec![
        financials_full(1, 100.0, 50.0, 10.0),
        financials_full(2, 200.0, 25.0, 5.0),
      ];
      state.active = Scope::All;

      let composition = super::scope_composition(&state);

      assert_eq!(composition.liquid, Some(300.0));
      assert_eq!(composition.asset_value, Some(75.0));
      assert_eq!(composition.escrow, Some(15.0));
    }

    #[test]
    fn it_is_none_per_figure_when_no_in_scope_character_has_it() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.financials = vec![financials(1, None)];
      state.active = Scope::All;

      let composition = super::scope_composition(&state);

      assert_eq!(composition.liquid, None);
      assert_eq!(composition.asset_value, None);
      assert_eq!(composition.escrow, None);
    }
  }

  mod composition_stack {
    use pretty_assertions::assert_eq;

    use super::*;

    fn financials_nw(character_id: i64, net_worth: Option<f64>) -> CharacterFinancials {
      CharacterFinancials {
        character_id,
        liquid: None,
        asset_value: None,
        escrow: None,
        net_worth,
      }
    }

    #[test]
    fn it_orders_characters_by_net_worth_descending() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None), pilot(3, None)];
      state.financials = vec![
        financials_nw(1, Some(100.0)),
        financials_nw(2, Some(300.0)),
        financials_nw(3, Some(200.0)),
      ];
      state.active = Scope::All;

      let stack = super::composition_stack(&state);

      assert_eq!(stack.iter().map(|s| s.id).collect::<Vec<_>>(), vec![2, 3, 1]);
    }

    #[test]
    fn it_drops_characters_with_no_synced_net_worth() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.financials = vec![financials_nw(1, Some(100.0)), financials_nw(2, None)];
      state.active = Scope::All;

      let stack = super::composition_stack(&state);

      assert_eq!(stack.len(), 1);
      assert_eq!(stack[0].id, 1);
    }

    #[test]
    fn it_is_empty_outside_all_wallets_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.financials = vec![financials_nw(1, Some(100.0))];
      state.active = Scope::Character(1);

      assert!(super::composition_stack(&state).is_empty());
    }
  }

  mod period_totals {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_income_and_spend_across_the_in_scope_characters() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.periods = vec![period(1, 100.0, 40.0), period(1, 50.0, 10.0), period(2, 200.0, 80.0)];
      state.active = Scope::All;

      let totals = super::period_totals(&state);

      assert_eq!(totals.income, 350.0);
      assert_eq!(totals.spend, 130.0);
      assert_eq!(totals.net, 220.0);
    }

    #[test]
    fn it_reflects_only_the_selected_character_in_single_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.periods = vec![period(1, 100.0, 40.0), period(2, 200.0, 80.0)];
      state.active = Scope::Character(1);

      let totals = super::period_totals(&state);

      assert_eq!(totals.income, 100.0);
      assert_eq!(totals.spend, 40.0);
      assert_eq!(totals.net, 60.0);
    }

    #[test]
    fn it_is_zero_when_no_in_scope_character_has_period_rows() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.periods = vec![period(9, 100.0, 40.0)];
      state.active = Scope::All;

      let totals = super::period_totals(&state);

      assert_eq!(totals, PeriodTotals::default());
    }
  }

  mod journal_matches {
    use super::*;

    #[test]
    fn it_keeps_income_for_the_in_sign_and_drops_spend() {
      let income = journal_entry(1, Some(100.0), "bounty_prizes", "Bounty");
      let spend = journal_entry(1, Some(-100.0), "market_transaction", "Buy");

      assert!(super::journal_matches(&income, SignFilter::In, ""));
      assert!(!super::journal_matches(&spend, SignFilter::In, ""));
    }

    #[test]
    fn it_keeps_spend_for_the_out_sign_and_drops_income() {
      let income = journal_entry(1, Some(100.0), "bounty_prizes", "Bounty");
      let spend = journal_entry(1, Some(-100.0), "market_transaction", "Buy");

      assert!(!super::journal_matches(&income, SignFilter::Out, ""));
      assert!(super::journal_matches(&spend, SignFilter::Out, ""));
    }

    #[test]
    fn it_matches_the_query_against_ref_type_and_description() {
      let entry = journal_entry(
        1,
        Some(1.0),
        "agent_mission_reward",
        "Distribution run for Sister Alitura",
      );

      assert!(super::journal_matches(&entry, SignFilter::All, "alitura"));
      assert!(super::journal_matches(&entry, SignFilter::All, "mission"));
      assert!(!super::journal_matches(&entry, SignFilter::All, "tritanium"));
    }

    #[test]
    fn it_composes_sign_and_query() {
      let entry = journal_entry(1, Some(500.0), "bounty_prizes", "Serpentis bounty");

      assert!(super::journal_matches(&entry, SignFilter::In, "serpentis"));
      assert!(!super::journal_matches(&entry, SignFilter::Out, "serpentis"));
    }
  }

  mod market_matches {
    use super::*;

    #[test]
    fn it_keeps_sells_for_in_and_buys_for_out() {
      let buy = market_entry(1, true, "Tritanium", "Jita");
      let sell = market_entry(1, false, "Tritanium", "Jita");

      assert!(super::market_matches(&sell, SignFilter::In, Side::All, ""));
      assert!(!super::market_matches(&buy, SignFilter::In, Side::All, ""));
      assert!(super::market_matches(&buy, SignFilter::Out, Side::All, ""));
      assert!(!super::market_matches(&sell, SignFilter::Out, Side::All, ""));
    }

    #[test]
    fn it_filters_by_side() {
      let buy = market_entry(1, true, "Tritanium", "Jita");
      let sell = market_entry(1, false, "Tritanium", "Jita");

      assert!(super::market_matches(&buy, SignFilter::All, Side::Buy, ""));
      assert!(!super::market_matches(&sell, SignFilter::All, Side::Buy, ""));
      assert!(super::market_matches(&sell, SignFilter::All, Side::Sell, ""));
      assert!(!super::market_matches(&buy, SignFilter::All, Side::Sell, ""));
      assert!(super::market_matches(&buy, SignFilter::All, Side::All, ""));
      assert!(super::market_matches(&sell, SignFilter::All, Side::All, ""));
    }

    #[test]
    fn it_matches_the_query_against_item_and_location() {
      let entry = market_entry(1, false, "Tritanium", "Jita IV - Moon 4");

      assert!(super::market_matches(&entry, SignFilter::All, Side::All, "trit"));
      assert!(super::market_matches(&entry, SignFilter::All, Side::All, "jita"));
      assert!(!super::market_matches(&entry, SignFilter::All, Side::All, "veldspar"));
    }
  }

  mod contract_matches {
    use super::*;

    #[test]
    fn it_filters_by_side() {
      let buy = contract_entry(1, true, "outstanding", "courier");
      let sell = contract_entry(1, false, "outstanding", "item_exchange");

      assert!(super::contract_matches(&buy, Side::Buy, ""));
      assert!(!super::contract_matches(&sell, Side::Buy, ""));
      assert!(super::contract_matches(&sell, Side::Sell, ""));
      assert!(!super::contract_matches(&buy, Side::Sell, ""));
      assert!(super::contract_matches(&buy, Side::All, ""));
      assert!(super::contract_matches(&sell, Side::All, ""));
    }

    #[test]
    fn it_matches_the_query_against_type_status_and_parties() {
      let entry = contract_entry(1, false, "outstanding", "item_exchange");

      assert!(super::contract_matches(&entry, Side::All, "exchange"));
      assert!(super::contract_matches(&entry, Side::All, "outstanding"));
      assert!(super::contract_matches(&entry, Side::All, "issuer"));
      assert!(super::contract_matches(&entry, Side::All, "assignee"));
      assert!(super::contract_matches(&entry, Side::All, "12345"));
      assert!(!super::contract_matches(&entry, Side::All, "courier"));
    }

    #[test]
    fn it_composes_side_and_query() {
      let sell = contract_entry(1, false, "outstanding", "item_exchange");

      assert!(super::contract_matches(&sell, Side::Sell, "exchange"));
      assert!(!super::contract_matches(&sell, Side::Buy, "exchange"));
    }
  }

  mod filtered_contracts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_applies_the_active_side_filter() {
      let mut state = State::new();
      state.contracts = vec![
        contract_entry(1, true, "outstanding", "courier"),
        contract_entry(1, false, "finished", "item_exchange"),
      ];

      state.side_filter = Side::Buy;
      state.recompute_derived();
      assert_eq!(super::filtered_contracts(&state).len(), 1);
      assert!(super::filtered_contracts(&state)[0].is_buy);

      state.side_filter = Side::Sell;
      state.recompute_derived();
      assert_eq!(super::filtered_contracts(&state).len(), 1);
      assert!(!super::filtered_contracts(&state)[0].is_buy);

      state.side_filter = Side::All;
      state.recompute_derived();
      assert_eq!(super::filtered_contracts(&state).len(), 2);
    }

    #[test]
    fn it_is_empty_when_no_contracts_are_synced() {
      let state = State::new();

      assert!(super::filtered_contracts(&state).is_empty());
      assert!(!state.has_contracts());
    }
  }

  mod contract_loader_target {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_targets_the_active_corporation_under_a_corp_scope() {
      let mut state = State::new();
      state.active = Scope::Corporation(98_000_001);

      assert_eq!(
        super::contract_loader_target(&state, 12_345),
        Some(ContractLoad::Corporation(98_000_001))
      );
    }

    #[test]
    fn it_targets_the_owning_character_under_an_all_scope() {
      let mut state = State::new();
      state.contracts = vec![contract_entry(7, false, "finished", "item_exchange")];

      assert_eq!(
        super::contract_loader_target(&state, 12_345),
        Some(ContractLoad::Character(7))
      );
    }

    #[test]
    fn it_is_none_when_no_row_matches() {
      let state = State::new();

      assert_eq!(super::contract_loader_target(&state, 999), None);
    }
  }

  mod journal_flow {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_income_and_spend_and_nets_them() {
      let entries = [
        journal_entry(1, Some(1_000.0), "bounty_prizes", "Bounty"),
        journal_entry(1, Some(250.0), "agent_mission_reward", "Mission"),
        journal_entry(1, Some(-400.0), "market_transaction", "Buy"),
      ];
      let refs: Vec<&JournalEntry> = entries.iter().collect();

      let flow = super::journal_flow(&refs);

      assert_eq!(flow.income, 1_250.0);
      assert_eq!(flow.spend, 400.0);
      assert_eq!(flow.net, 850.0);
    }

    #[test]
    fn it_ignores_null_and_zero_amounts() {
      let entries = [
        journal_entry(1, None, "unknown", "No amount"),
        journal_entry(1, Some(0.0), "zero", "Zero"),
        journal_entry(1, Some(-100.0), "tax", "Tax"),
      ];
      let refs: Vec<&JournalEntry> = entries.iter().collect();

      let flow = super::journal_flow(&refs);

      assert_eq!(flow.income, 0.0);
      assert_eq!(flow.spend, 100.0);
      assert_eq!(flow.net, -100.0);
    }

    #[test]
    fn it_is_zero_for_an_empty_page() {
      assert_eq!(super::journal_flow(&[]), JournalFlow::default());
    }
  }

  mod category_flows {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_groups_by_ref_type_and_orders_by_combined_magnitude() {
      let entries = [
        journal_entry(1, Some(100.0), "bounty_prizes", "Bounty"),
        journal_entry(1, Some(50.0), "bounty_prizes", "Bounty"),
        journal_entry(1, Some(-1_000.0), "market_transaction", "Buy"),
      ];
      let refs: Vec<&JournalEntry> = entries.iter().collect();

      let flows = super::category_flows(&refs);

      assert_eq!(flows.len(), 2);
      assert_eq!(flows[0].ref_type, "market_transaction");
      assert_eq!(flows[0].spend, 1_000.0);
      assert_eq!(flows[0].income, 0.0);
      assert_eq!(flows[1].ref_type, "bounty_prizes");
      assert_eq!(flows[1].income, 150.0);
      assert_eq!(flows[1].total(), 150.0);
    }

    #[test]
    fn it_splits_a_single_category_into_in_and_out() {
      let entries = [
        journal_entry(1, Some(300.0), "corporation_account_withdrawal", "In"),
        journal_entry(1, Some(-100.0), "corporation_account_withdrawal", "Out"),
      ];
      let refs: Vec<&JournalEntry> = entries.iter().collect();

      let flows = super::category_flows(&refs);

      assert_eq!(flows.len(), 1);
      assert_eq!(flows[0].income, 300.0);
      assert_eq!(flows[0].spend, 100.0);
      assert_eq!(flows[0].total(), 400.0);
    }

    #[test]
    fn it_drops_categories_with_no_signed_movement() {
      let entries = [
        journal_entry(1, None, "unknown", "No amount"),
        journal_entry(1, Some(0.0), "unknown", "Zero"),
        journal_entry(1, Some(75.0), "bounty_prizes", "Bounty"),
      ];
      let refs: Vec<&JournalEntry> = entries.iter().collect();

      let flows = super::category_flows(&refs);

      assert_eq!(flows.len(), 1);
      assert_eq!(flows[0].ref_type, "bounty_prizes");
    }

    #[test]
    fn it_humanizes_the_ref_type_label() {
      let entries = [journal_entry(1, Some(10.0), "agent_mission_reward", "Mission")];
      let refs: Vec<&JournalEntry> = entries.iter().collect();

      let flows = super::category_flows(&refs);

      assert_eq!(flows[0].label(), "Agent Mission Reward");
    }
  }

  mod journal_type_glyph {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::ui::components::glyph_badge::{GLYPH_EXPENSE, GLYPH_INCOME};

    #[test]
    fn it_reads_a_positive_amount_as_income() {
      let entry = journal_entry(1, Some(1_000.0), "player_donation", "Gift");

      assert_eq!(super::journal_type_glyph(&entry), (GLYPH_INCOME, true));
    }

    #[test]
    fn it_reads_a_negative_amount_as_expense() {
      let entry = journal_entry(1, Some(-400.0), "market_transaction", "Buy");

      assert_eq!(super::journal_type_glyph(&entry), (GLYPH_EXPENSE, false));
    }

    #[test]
    fn it_falls_back_to_the_ref_type_when_the_amount_is_absent() {
      let income = journal_entry(1, None, "bounty_prizes", "Bounty");
      let expense = journal_entry(1, None, "brokers_fee", "Fee");

      assert_eq!(super::journal_type_glyph(&income), (GLYPH_INCOME, true));
      assert_eq!(super::journal_type_glyph(&expense), (GLYPH_EXPENSE, false));
    }

    #[test]
    fn it_classifies_fee_tax_and_cost_ref_types_as_expense() {
      for ref_type in [
        "brokers_fee",
        "transaction_tax",
        "reprocessing_tax",
        "jump_clone_activation_cost",
      ] {
        let entry = journal_entry(1, None, ref_type, "Charge");

        assert_eq!(
          super::journal_type_glyph(&entry),
          (GLYPH_EXPENSE, false),
          "{ref_type} should read as an expense"
        );
      }
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
      state.roster = vec![pilot(1, Some(100.0))];
      state.financials = vec![financials(1, Some(100.0))];

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_the_hero_graph_with_a_net_worth_series() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0)), pilot(2, Some(50.0))];
      state.financials = vec![financials(1, Some(100.0)), financials(2, Some(50.0))];
      state.net_worth_series = (0..40).map(|i| nw_point("2026-06-01", 100.0 + i as f64)).collect();
      state.chart_hover = Some(0.5);
      state.timeframe = Timeframe::Month;

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_a_corp_scope_with_divisions() {
      let mut state = State::new();
      state.corporations = vec![corp(98_000_001, "Test Corp")];
      state.active = Scope::Corporation(98_000_001);
      state.corp_divisions = vec![
        corp_division(1, Some("Master Wallet"), Some(1_000.0)),
        corp_division(2, None, Some(250.0)),
      ];

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_a_corp_scope_with_no_divisions_synced() {
      let mut state = State::new();
      state.corporations = vec![corp(98_000_001, "Test Corp")];
      state.active = Scope::Corporation(98_000_001);

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }
  }

  mod resolve_scope_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_every_pilot_id_for_all_scope() {
      let roster = [pilot(1, None), pilot(2, None)];

      assert_eq!(super::resolve_scope_ids(Scope::All, &roster, &[]), vec![1, 2]);
    }

    #[test]
    fn it_returns_the_single_id_for_a_character_scope() {
      let roster = [pilot(1, None), pilot(2, None)];

      assert_eq!(super::resolve_scope_ids(Scope::Character(2), &roster, &[]), vec![2]);
    }

    #[test]
    fn it_returns_no_character_ids_for_a_corp_scope() {
      let roster = [pilot(1, None)];

      assert!(super::resolve_scope_ids(Scope::Corporation(98_000_001), &roster, &[]).is_empty());
    }
  }

  mod load_net_worth_series {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_character(db: &Database, id: i64) {
      use crate::store::{
        model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
        repo::character::insert_with_org,
      };

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
      insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_is_empty_for_a_corp_scope() {
      let db = crate::store::open_test().await.unwrap();

      assert!(
        super::load_net_worth_series(&db, Scope::Corporation(1), &[], &[])
          .await
          .is_empty()
      );
    }

    #[tokio::test]
    async fn it_is_empty_for_every_scope_without_snapshots() {
      let db = crate::store::open_test().await.unwrap();

      assert!(super::load_net_worth_series(&db, Scope::All, &[], &[]).await.is_empty());
      assert!(
        super::load_net_worth_series(&db, Scope::Character(42), &[42], &[])
          .await
          .is_empty()
      );
    }

    #[tokio::test]
    async fn it_maps_a_characters_recent_snapshots() {
      let db = crate::store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let today = Utc::now().format("%Y-%m-%d").to_string();
      crate::store::repo::finance::upsert(&db, 42, &today, 100.0, None, None, 175.0)
        .await
        .unwrap();

      let series = super::load_net_worth_series(&db, Scope::Character(42), &[42], &[]).await;

      assert_eq!(series.len(), 1);
      assert_eq!(series[0].net_worth, 175.0);
    }

    #[tokio::test]
    async fn it_maps_a_corporations_recent_snapshots() {
      let db = crate::store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let today = Utc::now().format("%Y-%m-%d").to_string();
      sqlx::query(
        "INSERT INTO corporation_net_worth_snapshot (corporation_id, date, liquid, net_worth) VALUES (?, ?, ?, ?)",
      )
      .bind(90_000_001_i64)
      .bind(&today)
      .bind(1_250.0)
      .bind(1_250.0)
      .execute(&db.0)
      .await
      .unwrap();

      let series = super::load_net_worth_series(&db, Scope::Corporation(90_000_001), &[], &[]).await;

      assert_eq!(series.len(), 1);
      assert_eq!(series[0].net_worth, 1_250.0);
    }

    #[tokio::test]
    async fn it_adds_owned_corp_net_worth_into_the_all_wallets_series() {
      let db = crate::store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let today = Utc::now().format("%Y-%m-%d").to_string();
      crate::store::repo::finance::upsert(&db, 42, &today, 100.0, None, None, 175.0)
        .await
        .unwrap();
      sqlx::query(
        "INSERT INTO corporation_net_worth_snapshot (corporation_id, date, liquid, net_worth) VALUES (?, ?, ?, ?)",
      )
      .bind(90_000_001_i64)
      .bind(&today)
      .bind(1_250.0)
      .bind(1_250.0)
      .execute(&db.0)
      .await
      .unwrap();

      let corporations = vec![corp(90_000_001, "Test Corp")];
      let series = super::load_net_worth_series(&db, Scope::All, &[42], &corporations).await;

      assert_eq!(series.len(), 1);
      assert_eq!(series[0].net_worth, 175.0 + 1_250.0);
    }
  }

  mod integration {
    use super::*;

    #[tokio::test]
    async fn it_drives_every_pane_off_db_only() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::Loaded(Box::new(Loaded {
          contract_total: 0,
          contracts: vec![],
          corp_divisions: vec![],
          corporations: vec![corp(98_000_001, "Test Corp")],
          financials: vec![financials(1, Some(100.0)), financials(2, Some(50.0))],
          journal: vec![journal_entry(1, Some(10.0), "bounty_prizes", "ratting")],
          journal_total: 1,
          market: vec![market_entry(1, true, "Tritanium", "Jita IV")],
          market_total: 1,
          net_worth_series: vec![nw_point("2026-06-01", 150.0), nw_point("2026-06-02", 175.0)],
          periods: vec![period(1, 100.0, 40.0)],
          right_rail_width: 280.0,
          roster: vec![pilot(1, Some(100.0)), pilot(2, Some(50.0))],
        })),
        &db,
      );

      let interactions = [
        Message::PickerToggled,
        Message::ScopeSelected(Scope::Character(1)),
        Message::ScopeSelected(Scope::Corporation(98_000_001)),
        Message::DivisionSelected(2),
        Message::ScopeSelected(Scope::All),
        Message::TabSelected(Tab::Market),
        Message::TabSelected(Tab::Journal),
        Message::SignFilterChanged(SignFilter::In),
        Message::SearchChanged("tritanium".to_owned()),
        Message::TimeframeSelected(Timeframe::Year),
        Message::ChartHovered(Some(0.5)),
      ];
      for message in interactions {
        let _ = update(&mut state, message, &db);
        let _el: Element<'_, Message> = view(&state, Utc::now());
      }
    }

    #[test]
    fn it_renders_across_every_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0)), pilot(2, Some(50.0))];
      state.financials = vec![financials(1, Some(100.0)), financials(2, Some(50.0))];
      state.corporations = vec![corp(98_000_001, "Test Corp")];

      for scope in [Scope::All, Scope::Character(1), Scope::Corporation(98_000_001)] {
        state.active = scope;
        let _el: Element<'_, Message> = view(&state, Utc::now());
      }
    }
  }
}
