//! Wallet controller: navigation and DB-backed data reads.

use std::{
  cmp::Reverse,
  collections::{HashMap, HashSet},
  time::{SystemTime, UNIX_EPOCH},
};

use iced::widget::image;
use pod_model::{Character, CharacterContract, Corporation, WalletJournalEntry, WalletTransaction};
use pod_ui::{
  components::{
    CharacterPicker,
    character_picker::{CharacterEntry, CorporationEntry, PickerSelection},
  },
  views::wallet::{
    ContractEntry, JournalEntry, MarketEntry, Message, SideFilter, SignFilter, State, Tab, Timeframe, WalletCharacter,
  },
};

use crate::services::{Services, corporation as corporation_service};

/// Creates the initial wallet state and spawns background tasks to load
/// journal entries, transactions, and contracts from the database.
pub fn new(
  characters: Vec<Character>,
  corporations: Vec<Corporation>,
  services: &Services,
  right_rail_width: f32,
) -> (State, iced::Task<Message>) {
  let state = build_wallet_state(&characters, &corporations, right_rail_width);
  let task = if let Some(db) = services.db.clone() {
    let char_ids: Vec<i64> = characters.iter().map(|c| *c.id()).collect();
    let db_j = db.clone();
    let db_t = db.clone();
    let db_c = db.clone();
    let ids_j = char_ids.clone();
    let ids_t = char_ids.clone();
    let ids_c = char_ids;
    let journal_task = iced::Task::perform(
      async move { load_journal_from_db(ids_j, db_j).await },
      Message::JournalLoaded,
    );
    let transactions_task = iced::Task::perform(
      async move { load_transactions_from_db(ids_t, db_t).await },
      Message::TransactionsLoaded,
    );
    let contracts_task = iced::Task::perform(
      async move { load_contracts_from_db(ids_c, db_c).await },
      Message::ContractsLoaded,
    );
    iced::Task::batch([journal_task, transactions_task, contracts_task])
  } else {
    iced::Task::none()
  };
  (state, task)
}

async fn load_journal_from_db(char_ids: Vec<i64>, db: pod_db::Repo) -> Vec<JournalEntry> {
  let mut entries: Vec<JournalEntry> = Vec::new();
  for id in char_ids {
    let rows = db.wallet().journal_for_character(id).await.unwrap_or_default();
    entries.extend(rows.into_iter().map(map_journal_row));
  }
  entries.sort_by_key(|e| e.ts_secs);
  entries
}

async fn load_transactions_from_db(char_ids: Vec<i64>, db: pod_db::Repo) -> Vec<MarketEntry> {
  let mut entries: Vec<MarketEntry> = Vec::new();
  let type_ids: Vec<i32> = {
    let mut all: Vec<WalletTransaction> = Vec::new();
    for id in &char_ids {
      all.extend(db.wallet().transactions_for_character(*id).await.unwrap_or_default());
    }
    all
      .iter()
      .map(|t| t.type_id)
      .collect::<HashSet<_>>()
      .into_iter()
      .collect()
  };
  let type_names = load_type_names(&type_ids, &db).await;
  for id in char_ids {
    let rows = db.wallet().transactions_for_character(id).await.unwrap_or_default();
    entries.extend(rows.into_iter().map(|r| map_txn_row(r, &type_names)));
  }
  entries.sort_by_key(|e| e.ts_secs);
  entries
}

async fn load_contracts_from_db(char_ids: Vec<i64>, db: pod_db::Repo) -> Result<Vec<ContractEntry>, String> {
  let names: HashMap<i64, String> = HashMap::new();
  let locations: HashMap<i64, String> = HashMap::new();
  let mut contracts: Vec<ContractEntry> = Vec::new();
  for id in char_ids {
    let rows = db.wallet().contracts_for_character(id).await.unwrap_or_default();
    contracts.extend(rows.into_iter().map(|r| build_contract_entry(r, &names, &locations)));
  }
  contracts.sort_by_key(|e| e.ts_secs);
  Ok(contracts)
}

fn map_journal_row(row: WalletJournalEntry) -> JournalEntry {
  let ts = parse_iso_to_unix(&row.date);
  JournalEntry {
    id: format!("{}-{}", row.character_id, row.entry_id),
    who: row.character_id,
    entry_type: row.ref_type,
    delta: row.amount.unwrap_or(0.0),
    ts_secs: secs_ago(ts),
    reference: row.description,
    party: row.first_party_id.map(|id| format!("#{id}")).unwrap_or_default(),
    location: String::new(),
  }
}

/// Returns a task that reloads journal entries, transactions, and contracts
/// from the database and emits the corresponding `Loaded` messages, refreshing
/// an already-active wallet view.
pub fn reload_task(characters: Vec<Character>, services: &Services) -> iced::Task<Message> {
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  let char_ids: Vec<i64> = characters.iter().map(|c| *c.id()).collect();
  let db_j = db.clone();
  let db_t = db.clone();
  let db_c = db.clone();
  let ids_j = char_ids.clone();
  let ids_t = char_ids.clone();
  let ids_c = char_ids;
  let journal_task = iced::Task::perform(
    async move { load_journal_from_db(ids_j, db_j).await },
    Message::JournalLoaded,
  );
  let transactions_task = iced::Task::perform(
    async move { load_transactions_from_db(ids_t, db_t).await },
    Message::TransactionsLoaded,
  );
  let contracts_task = iced::Task::perform(
    async move { load_contracts_from_db(ids_c, db_c).await },
    Message::ContractsLoaded,
  );
  iced::Task::batch([journal_task, transactions_task, contracts_task])
}

/// Processes a wallet message, performing ESI fetches when a corporation is
/// selected or a division is changed.
pub fn update(
  state: &mut State,
  message: Message,
  services: &Services,
  corporations: &[Corporation],
) -> iced::Task<Message> {
  let icon_type_ids = compute_icon_type_ids(&message, state);
  match message {
    Message::CharacterPicker(msg) => update_character_picker(state, msg, corporations, services, icon_type_ids),
    Message::DivisionSelected(div) => update_division_selected(state, div, corporations, services, icon_type_ids),
    other => {
      log_wallet_message(&other);
      let base = pod_ui::views::wallet::update(state, other);
      recompute_derived(state);
      attach_icon_task(base, icon_type_ids, services)
    }
  }
}

fn log_wallet_message(message: &Message) {
  log_data_loaded_message(message);
  log_ui_navigation_message(message);
  log_filter_change_message(message);
}

fn log_data_loaded_message(message: &Message) {
  match message {
    Message::JournalLoaded(entries) => {
      tracing::debug!("wallet: {} journal entries loaded", entries.len())
    }
    Message::TransactionsLoaded(entries) => {
      tracing::debug!("wallet: {} transactions loaded", entries.len())
    }
    Message::AllCorpBalancesLoaded(b) => {
      tracing::debug!("wallet: corp balances loaded for {} corp(s)", b.len())
    }
    msg => log_data_loaded_message_mid(msg),
  }
}

fn log_data_loaded_message_mid(message: &Message) {
  match message {
    Message::AssetValuesLoaded(v) => {
      tracing::debug!("wallet: asset values loaded for {} character(s)", v.len())
    }
    msg => log_data_loaded_message_ext(msg),
  }
}

fn log_data_loaded_message_ext(message: &Message) {
  match message {
    Message::ContractsLoaded(Ok(c)) => tracing::debug!("wallet: {} contracts loaded", c.len()),
    Message::ContractsLoaded(Err(e)) => tracing::warn!("wallet: contracts load failed — {e}"),
    Message::CorpDataLoaded {
      divisions,
      journal,
      market,
    } => tracing::debug!(
      "wallet: corp data loaded — {} division(s), {} journal entries, {} market entries",
      divisions.len(),
      journal.len(),
      market.len()
    ),
    _ => {}
  }
}

fn log_ui_navigation_message(message: &Message) {
  match message {
    Message::TabSelected(t) => tracing::info!("wallet: tab selected — {t:?}"),
    Message::TimeframeChanged(tf) => tracing::info!("wallet: timeframe changed — {tf:?}"),
    _ => {}
  }
}

fn log_filter_change_message(message: &Message) {
  match message {
    Message::JournalTab(pod_ui::views::wallet::journal_tab::Message::SignFilterChanged(sign)) => {
      tracing::info!("wallet: sign filter changed — {sign:?}");
    }
    Message::MarketTab(pod_ui::views::wallet::market_tab::Message::SideFilterChanged(side)) => {
      tracing::info!("wallet: side filter changed — {side:?}");
    }
    _ => {}
  }
}

fn attach_icon_task(
  base: iced::Task<Message>,
  icon_type_ids: Option<Vec<i32>>,
  services: &Services,
) -> iced::Task<Message> {
  if let (Some(type_ids), Some(esi), Some(db)) = (icon_type_ids, services.esi_client.clone(), services.db.clone()) {
    let icon_task = iced::Task::perform(
      async move { fetch_type_icons(type_ids, esi, db).await },
      Message::ItemIconsLoaded,
    );
    iced::Task::batch([base, icon_task])
  } else {
    base
  }
}

fn contract_counterparty_id(row: &CharacterContract) -> i64 {
  if row.issuer_id == row.character_id {
    if row.acceptor_id != 0 {
      row.acceptor_id
    } else {
      row.assignee_id
    }
  } else {
    row.issuer_id
  }
}

fn contract_counterparty_name(id: i64, names: &HashMap<i64, String>) -> String {
  if id != 0 {
    names.get(&id).cloned().unwrap_or_else(|| format!("#{id}"))
  } else {
    String::new()
  }
}

fn contract_title(row: &CharacterContract) -> String {
  if row.title.is_empty() {
    format!("Contract #{}", row.contract_id)
  } else {
    row.title.clone()
  }
}

fn build_contract_entry(
  row: CharacterContract,
  names: &HashMap<i64, String>,
  locations: &HashMap<i64, String>,
) -> ContractEntry {
  let ts = parse_iso_to_unix(&row.date_issued);
  let counterparty_id = contract_counterparty_id(&row);
  let counterparty = contract_counterparty_name(counterparty_id, names);
  let location = row
    .start_location_id
    .and_then(|id| locations.get(&id).cloned())
    .unwrap_or_default();
  let title = contract_title(&row);
  ContractEntry {
    id: format!("{}-{}", row.character_id, row.contract_id),
    who: row.character_id,
    kind: row.contract_type,
    status: row.status,
    title,
    counterparty,
    price: row.price.unwrap_or(0.0),
    collateral: row.collateral.unwrap_or(0.0),
    ts_secs: secs_ago(ts),
    location,
  }
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

fn build_net_worth_series(journal: &[JournalEntry]) -> Vec<f64> {
  if journal.is_empty() {
    return Vec::new();
  }
  let mut running = 0.0f64;
  journal
    .iter()
    .map(|e| {
      running += e.delta;
      running
    })
    .collect()
}

fn build_picker_entries(characters: &[Character]) -> Vec<CharacterEntry> {
  let mut v = vec![CharacterEntry {
    id: None,
    name: "All Wallets".to_string(),
    corp_name: format!("{} accounts", characters.len()),
    tone: 200,
    portrait_handle: None,
  }];
  for c in characters {
    let portrait_handle = c.portrait_data().as_ref().map(|b| image::Handle::from_bytes(b.clone()));
    v.push(CharacterEntry {
      id: Some(*c.id()),
      name: c.name().clone(),
      corp_name: c.corp_name().clone(),
      tone: *c.portrait_tone() as u16,
      portrait_handle,
    });
  }
  v
}

fn build_wallet_chars(characters: &[Character]) -> Vec<WalletCharacter> {
  characters
    .iter()
    .map(|c| WalletCharacter {
      id: *c.id(),
      name: c.name().clone(),
      corp: c.corp_name().clone(),
      liquid: c.isk_balance().unwrap_or(0.0),
      assets: 0.0,
      escrow: 0.0,
      granted_scopes: c.granted_scopes().clone(),
      portrait_tone: *c.portrait_tone() as u16,
      portrait_handle: c.portrait_data().as_ref().map(|b| image::Handle::from_bytes(b.clone())),
    })
    .collect()
}

fn build_wallet_state(characters: &[Character], corporations: &[Corporation], right_rail_width: f32) -> State {
  let wallet_chars = build_wallet_chars(characters);
  let picker_entries = build_picker_entries(characters);
  let corp_entries = build_corp_picker_entries(corporations);
  let picker = CharacterPicker::new()
    .entries(picker_entries)
    .corp_entries(corp_entries)
    .show_all(true);
  State {
    active_division: 1,
    active_tab: Tab::Market,
    all_corp_balances: Vec::new(),
    chart_hover: None,
    characters: wallet_chars,
    chart_series: Vec::new(),
    contracts: Vec::new(),
    corp_divisions: Vec::new(),
    corp_journal: Vec::new(),
    corp_market: Vec::new(),
    drag_origin: None,
    dragging_pane: None,
    filtered_contracts: Vec::new(),
    filtered_journal: Vec::new(),
    filtered_market: Vec::new(),
    item_icons: HashMap::new(),
    journal: Vec::new(),
    journal_income: 0.0,
    journal_spend: 0.0,
    market: Vec::new(),
    net_worth_change: 0.0,
    net_worth_series: Vec::new(),
    picker,
    right_rail_width,
    search_query: String::new(),
    side_filter: SideFilter::All,
    sign_filter: SignFilter::All,
    timeframe: Timeframe::M3,
  }
}

fn collect_station_ids(loc_ids: &[i64]) -> Vec<i32> {
  loc_ids
    .iter()
    .filter(|&&id| id < 100_000_000)
    .filter_map(|&id| i32::try_from(id).ok())
    .collect()
}

fn market_entries_from_message(message: &Message) -> Option<&Vec<MarketEntry>> {
  match message {
    Message::TransactionsLoaded(entries) => Some(entries),
    Message::CorpDataLoaded {
      market, ..
    } => Some(market),
    _ => None,
  }
}

fn compute_icon_type_ids(message: &Message, state: &State) -> Option<Vec<i32>> {
  let ids: Vec<i32> = market_entries_from_message(message)?
    .iter()
    .map(|e| e.type_id)
    .collect::<HashSet<_>>()
    .into_iter()
    .filter(|id| !state.item_icons.contains_key(id))
    .collect();
  if ids.is_empty() { None } else { Some(ids) }
}

async fn build_corp_market_entries(
  raw_txns: Vec<pod_esi::models::corporation::CorporationWalletTransaction>,
  corp_id: i64,
  division: u8,
  db: &pod_db::Repo,
) -> Vec<MarketEntry> {
  let corp_type_ids: Vec<i32> = raw_txns
    .iter()
    .map(|t| t.type_id)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  let corp_type_names = load_type_names(&corp_type_ids, db).await;
  raw_txns
    .into_iter()
    .map(|t| map_corp_txn_entry(t, corp_id, division, &corp_type_names))
    .collect()
}

#[tracing::instrument(skip(corporations, esi, db))]
async fn fetch_corp_data(
  corp_id: i64,
  division: u8,
  corporations: Vec<Corporation>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> (Vec<(u8, f64)>, Vec<JournalEntry>, Vec<MarketEntry>) {
  let Some(corp) = corporations.iter().find(|c| *c.id() == corp_id) else {
    return (Vec::new(), Vec::new(), Vec::new());
  };
  let Some(token) = corporation_service::ensure_valid_token(corp, &esi, &db).await else {
    return (Vec::new(), Vec::new(), Vec::new());
  };
  let grant = corporation_service::refresh_grant(corp, &token);
  let corp_esi = esi.corporation(corp_id);
  let corp_client = corp_esi.auth(&grant);
  let divisions: Vec<(u8, f64)> = corp_client
    .wallets()
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|w| (w.division as u8, w.balance))
    .collect();
  let journal: Vec<JournalEntry> = corp_client
    .wallet_journal(i32::from(division))
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|e| map_corp_journal_entry(e, corp_id, division))
    .collect();
  let raw_txns = corp_client
    .wallet_transactions(i32::from(division))
    .await
    .unwrap_or_default();
  let market = build_corp_market_entries(raw_txns, corp_id, division, &db).await;
  (divisions, journal, market)
}

#[tracing::instrument(skip(esi, db))]
async fn fetch_type_icons(type_ids: Vec<i32>, esi: pod_esi::Client, db: pod_db::Repo) -> Vec<(i32, Vec<u8>)> {
  let cached = db
    .universe()
    .type_icons()
    .find_by_ids(&type_ids, "icon")
    .await
    .unwrap_or_default();

  let cached_ids: HashSet<i32> = cached.iter().map(|(id, _)| *id).collect();
  let missing: Vec<i32> = type_ids.into_iter().filter(|id| !cached_ids.contains(id)).collect();

  let mut results = cached;
  for type_id in missing {
    if let Ok(bytes) = esi.images().type_icon(type_id as i64, 32).await {
      let _ = db.universe().type_icons().upsert(type_id, "icon", bytes.clone()).await;
      results.push((type_id, bytes));
    }
  }
  results
}

fn format_party_id(id: Option<i64>) -> String {
  match id {
    Some(n) => format!("#{n}"),
    None => String::new(),
  }
}

fn journal_matches(j: &JournalEntry, corp_selected: bool, who: Option<i64>, sign: &SignFilter, q: &str) -> bool {
  if !corp_selected
    && let Some(id) = who
    && j.who != id
  {
    return false;
  }
  match sign {
    SignFilter::In if j.delta <= 0.0 => return false,
    SignFilter::Out if j.delta >= 0.0 => return false,
    _ => {}
  }
  if !q.is_empty()
    && !j.reference.to_lowercase().contains(q)
    && !j.party.to_lowercase().contains(q)
    && !j.location.to_lowercase().contains(q)
  {
    return false;
  }
  true
}

async fn load_type_names(type_ids: &[i32], db: &pod_db::Repo) -> HashMap<i32, String> {
  db.universe()
    .item_types()
    .find_by_ids(type_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|t| (t.id, t.name))
    .collect()
}

fn map_corp_journal_entry(
  e: pod_esi::models::corporation::CorporationWalletJournalEntry,
  corp_id: i64,
  division: u8,
) -> JournalEntry {
  let ts = parse_iso_to_unix(&e.date);
  JournalEntry {
    id: format!("corp-{corp_id}-{division}-{}", e.id),
    who: e.first_party_id.unwrap_or(corp_id),
    entry_type: e.ref_type.clone(),
    delta: e.amount.unwrap_or(0.0),
    ts_secs: secs_ago(ts),
    reference: e.description,
    party: format_party_id(e.first_party_id.or(e.second_party_id)),
    location: String::new(),
  }
}

fn map_corp_txn_entry(
  t: pod_esi::models::corporation::CorporationWalletTransaction,
  corp_id: i64,
  division: u8,
  type_names: &HashMap<i32, String>,
) -> MarketEntry {
  let ts = parse_iso_to_unix(&t.date);
  let total = t.unit_price * t.quantity as f64;
  let item = type_names
    .get(&t.type_id)
    .cloned()
    .unwrap_or_else(|| format!("Unknown type {}", t.type_id));
  MarketEntry {
    id: format!("corp-{corp_id}-{division}-{}", t.transaction_id),
    who: corp_id,
    type_id: t.type_id,
    side: if t.is_buy {
      "buy".to_string()
    } else {
      "sell".to_string()
    },
    qty: t.quantity as u64,
    item,
    unit: t.unit_price,
    total,
    fee: 0.0,
    ts_secs: secs_ago(ts),
    location: format!("Location #{}", t.location_id),
  }
}

fn map_txn_row(row: WalletTransaction, type_names: &HashMap<i32, String>) -> MarketEntry {
  let ts = parse_iso_to_unix(&row.date);
  let total = row.unit_price * row.quantity as f64;
  let item = type_names
    .get(&row.type_id)
    .cloned()
    .unwrap_or_else(|| format!("Unknown type {}", row.type_id));
  MarketEntry {
    id: format!("{}-{}", row.character_id, row.transaction_id),
    who: row.character_id,
    type_id: row.type_id,
    side: if row.is_buy {
      "buy".to_string()
    } else {
      "sell".to_string()
    },
    qty: row.quantity as u64,
    item,
    unit: row.unit_price,
    total,
    fee: 0.0,
    ts_secs: secs_ago(ts),
    location: format!("Location #{}", row.location_id),
  }
}

fn market_matches(m: &MarketEntry, corp_selected: bool, who: Option<i64>, side: &SideFilter, q: &str) -> bool {
  market_owner_matches(m, corp_selected, who) && market_side_matches(m, side) && market_query_matches(m, q)
}

fn market_owner_matches(m: &MarketEntry, corp_selected: bool, who: Option<i64>) -> bool {
  corp_selected || who.is_none_or(|id| m.who == id)
}

fn market_query_matches(m: &MarketEntry, q: &str) -> bool {
  q.is_empty() || m.item.to_lowercase().contains(&q.to_lowercase())
}

fn market_side_matches(m: &MarketEntry, side: &SideFilter) -> bool {
  match side {
    SideFilter::All => true,
    SideFilter::Buy => m.side == "buy",
    SideFilter::Sell => m.side == "sell",
  }
}

fn parse_iso_to_unix(s: &str) -> i64 {
  let s = s.trim_end_matches('Z');
  let Some((date, time)) = s.split_once('T') else {
    return 0;
  };
  let mut dp = date.split('-');
  let y: i64 = dp.next().and_then(|v| v.parse().ok()).unwrap_or(0);
  let mo: i64 = dp.next().and_then(|v| v.parse().ok()).unwrap_or(0);
  let d: i64 = dp.next().and_then(|v| v.parse().ok()).unwrap_or(0);
  let mut tp = time.split(':');
  let h: i64 = tp.next().and_then(|v| v.parse().ok()).unwrap_or(0);
  let mi: i64 = tp.next().and_then(|v| v.parse().ok()).unwrap_or(0);
  let sec: i64 = tp.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) as i64;
  let y = if mo <= 2 { y - 1 } else { y };
  let mo = if mo > 2 { mo - 3 } else { mo + 9 };
  let c = y / 100;
  let ya = y - 100 * c;
  let j = (146097 * c) / 4 + (1461 * ya) / 4 + (153 * mo + 2) / 5 + d + 1721119;
  let unix_days = j - 2440588;
  unix_days * 86400 + h * 3600 + mi * 60 + sec
}

fn recompute_derived(state: &mut State) {
  recompute_filters(state);
  recompute_series(state);
}

fn contract_matches_filter(c: &ContractEntry, who: Option<i64>, q: &str) -> bool {
  if let Some(id) = who
    && c.who != id
  {
    return false;
  }
  if !q.is_empty() && !c.title.to_lowercase().contains(q) {
    return false;
  }
  true
}

fn select_journal_source(state: &State, corp_selected: bool) -> Vec<JournalEntry> {
  if corp_selected {
    state.corp_journal.clone()
  } else if state.selected_character().is_none() {
    let mut all = state.journal.clone();
    all.extend(state.corp_journal.iter().cloned());
    all
  } else {
    state.journal.clone()
  }
}

fn select_market_source(state: &State, corp_selected: bool) -> Vec<MarketEntry> {
  if corp_selected {
    state.corp_market.clone()
  } else if state.selected_character().is_none() {
    let mut all = state.market.clone();
    all.extend(state.corp_market.iter().cloned());
    all
  } else {
    state.market.clone()
  }
}

fn recompute_filters(state: &mut State) {
  let who = state.selected_character();
  let q = state.search_query.to_lowercase();
  let corp_selected = state.is_corp_selected();
  let sign = state.sign_filter.clone();
  let side = state.side_filter.clone();
  let journal_source = select_journal_source(state, corp_selected);
  state.filtered_journal = journal_source
    .iter()
    .filter(|j| journal_matches(j, corp_selected, who, &sign, &q))
    .cloned()
    .collect();
  let market_source = select_market_source(state, corp_selected);
  state.filtered_market = market_source
    .iter()
    .filter(|m| market_matches(m, corp_selected, who, &side, &q))
    .cloned()
    .collect();
  state.filtered_contracts = state
    .contracts
    .iter()
    .filter(|c| contract_matches_filter(c, who, &q))
    .cloned()
    .collect();
  update_journal_totals(state);
}

fn collect_series_journal(state: &State) -> Vec<JournalEntry> {
  if state.is_corp_selected() {
    return state.corp_journal.clone();
  }
  let selected_char = state.selected_character();
  let mut entries: Vec<JournalEntry> = state
    .journal
    .iter()
    .filter(|e| selected_char.is_none_or(|id| e.who == id))
    .cloned()
    .collect();
  if selected_char.is_none() {
    entries.extend(state.corp_journal.iter().cloned());
  }
  entries
}

fn trim_series_to_days(series: &[f64], days: usize) -> Vec<f64> {
  let total = series.len();
  if total <= days {
    series.to_vec()
  } else {
    series[total - days..].to_vec()
  }
}

fn net_worth_change(chart: &[f64]) -> f64 {
  if chart.len() < 2 {
    0.0
  } else {
    chart[chart.len() - 1] - chart[0]
  }
}

fn recompute_series(state: &mut State) {
  let mut entries = collect_series_journal(state);
  entries.sort_by_key(|e| Reverse(e.ts_secs));
  state.net_worth_series = build_net_worth_series(&entries);
  state.chart_series = trim_series_to_days(&state.net_worth_series, state.timeframe.days());
  state.net_worth_change = net_worth_change(&state.chart_series);
}

fn secs_ago(ts: i64) -> u64 {
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs() as i64;
  (now - ts).max(0) as u64
}

fn log_picker_selection(sel: &PickerSelection) {
  match sel {
    PickerSelection::All => tracing::info!("wallet: all wallets selected"),
    PickerSelection::Character(id) => tracing::info!("wallet: character selected — character_id: {id}"),
    PickerSelection::Corporation(id) => tracing::info!("wallet: corporation selected — corp_id: {id}"),
  }
}

fn try_dispatch_corp_fetch(
  state: &mut State,
  corp_id: i64,
  corporations: &[Corporation],
  esi: pod_esi::Client,
  db: pod_db::Repo,
  msg: pod_ui::components::character_picker::Message,
) -> iced::Task<Message> {
  let corps = corporations.to_vec();
  let _ = pod_ui::views::wallet::update(state, Message::CharacterPicker(msg));
  recompute_derived(state);
  iced::Task::perform(
    async move { fetch_corp_data(corp_id, 1, corps, esi, db).await },
    |(divisions, journal, market)| Message::CorpDataLoaded {
      divisions,
      journal,
      market,
    },
  )
}

fn update_character_picker(
  state: &mut State,
  msg: pod_ui::components::character_picker::Message,
  corporations: &[Corporation],
  services: &Services,
  icon_type_ids: Option<Vec<i32>>,
) -> iced::Task<Message> {
  if let pod_ui::components::character_picker::Message::Select(sel) = &msg {
    log_picker_selection(sel);
  }
  if let pod_ui::components::character_picker::Message::Select(PickerSelection::Corporation(corp_id)) = &msg
    && let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone())
  {
    return try_dispatch_corp_fetch(state, *corp_id, corporations, esi, db, msg);
  }
  let base = pod_ui::views::wallet::update(state, Message::CharacterPicker(msg));
  recompute_derived(state);
  attach_icon_task(base, icon_type_ids, services)
}

fn update_division_selected(
  state: &mut State,
  div: u8,
  corporations: &[Corporation],
  services: &Services,
  icon_type_ids: Option<Vec<i32>>,
) -> iced::Task<Message> {
  tracing::info!("wallet: division selected — {div}");
  if let Some(corp_id) = state.selected_corporation()
    && let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone())
  {
    let corps = corporations.to_vec();
    let _ = pod_ui::views::wallet::update(state, Message::DivisionSelected(div));
    recompute_derived(state);
    return iced::Task::perform(
      async move { fetch_corp_data(corp_id, div, corps, esi, db).await },
      |(divisions, journal, market)| Message::CorpDataLoaded {
        divisions,
        journal,
        market,
      },
    );
  }
  let base = pod_ui::views::wallet::update(state, Message::DivisionSelected(div));
  recompute_derived(state);
  attach_icon_task(base, icon_type_ids, services)
}

fn update_journal_totals(state: &mut State) {
  state.journal_income = state
    .filtered_journal
    .iter()
    .filter(|j| j.delta > 0.0)
    .map(|j| j.delta)
    .sum();
  state.journal_spend = state
    .filtered_journal
    .iter()
    .filter(|j| j.delta < 0.0)
    .map(|j| j.delta.abs())
    .sum();
}

#[cfg(test)]
mod tests {
  use super::*;

  mod build_contract_entry {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_contract(acceptor_id: i64, assignee_id: i64, title: &str) -> CharacterContract {
      CharacterContract {
        character_id: 1,
        contract_id: 7,
        contract_type: "item_exchange".to_string(),
        status: "outstanding".to_string(),
        title: title.to_string(),
        issuer_id: 1,
        assignee_id,
        acceptor_id,
        price: Some(100.0),
        collateral: Some(0.0),
        date_issued: "2024-01-01T00:00:00Z".to_string(),
        date_expired: "2024-02-01T00:00:00Z".to_string(),
        start_location_id: None,
      }
    }

    #[test]
    fn it_uses_acceptor_as_counterparty_when_nonzero() {
      let row = make_contract(500, 200, "deal");
      let mut names = HashMap::new();
      names.insert(500_i64, "Alice".to_string());

      let entry = build_contract_entry(row, &names, &HashMap::new());

      assert_eq!(entry.counterparty, "Alice");
    }

    #[test]
    fn it_uses_issuer_as_counterparty_when_issued_to_you() {
      let row = CharacterContract {
        character_id: 1,
        issuer_id: 99,
        acceptor_id: 1,
        assignee_id: 1,
        ..make_contract(1, 1, "incoming")
      };
      let mut names = HashMap::new();
      names.insert(99_i64, "Eve".to_string());

      let entry = build_contract_entry(row, &names, &HashMap::new());

      assert_eq!(entry.counterparty, "Eve");
    }

    #[test]
    fn it_falls_back_to_assignee_when_acceptor_is_zero() {
      let row = make_contract(0, 200, "deal");
      let mut names = HashMap::new();
      names.insert(200_i64, "Bob".to_string());

      let entry = build_contract_entry(row, &names, &HashMap::new());

      assert_eq!(entry.counterparty, "Bob");
    }

    #[test]
    fn it_generates_default_title_when_title_is_empty() {
      let row = make_contract(0, 0, "");

      let entry = build_contract_entry(row, &HashMap::new(), &HashMap::new());

      assert_eq!(entry.title, "Contract #7");
    }

    #[test]
    fn it_preserves_non_empty_title() {
      let row = make_contract(0, 0, "My contract");

      let entry = build_contract_entry(row, &HashMap::new(), &HashMap::new());

      assert_eq!(entry.title, "My contract");
    }
  }

  mod collect_station_ids {
    use super::*;

    #[test]
    fn it_returns_empty_for_empty_input() {
      let result = collect_station_ids(&[]);

      assert!(result.is_empty());
    }

    #[test]
    fn it_includes_ids_below_100_000_000() {
      let result = collect_station_ids(&[60_003_760]);

      assert_eq!(result, vec![60_003_760_i32]);
    }

    #[test]
    fn it_excludes_ids_at_or_above_100_000_000() {
      let result = collect_station_ids(&[100_000_000, 1_023_000_000]);

      assert!(result.is_empty());
    }

    #[test]
    fn it_filters_mixed_ids_keeping_only_npc_stations() {
      let result = collect_station_ids(&[60_003_760, 1_023_000_000, 60_000_004]);

      assert_eq!(result.len(), 2);
      assert!(result.contains(&60_003_760_i32));
      assert!(result.contains(&60_000_004_i32));
    }
  }

  mod journal_matches {
    use super::*;

    fn make_journal(delta: f64) -> JournalEntry {
      JournalEntry {
        id: "1".to_string(),
        who: 10,
        entry_type: "player_trading".to_string(),
        delta,
        ts_secs: 0,
        reference: "ref".to_string(),
        party: "party".to_string(),
        location: String::new(),
      }
    }

    #[test]
    fn it_excludes_wrong_character_when_corp_not_selected() {
      let j = make_journal(100.0);

      let result = journal_matches(&j, false, Some(99), &SignFilter::All, "");

      assert!(!result);
    }

    #[test]
    fn it_includes_matching_character_id() {
      let j = make_journal(100.0);

      let result = journal_matches(&j, false, Some(10), &SignFilter::All, "");

      assert!(result);
    }

    #[test]
    fn it_excludes_negative_delta_for_sign_filter_in() {
      let j = make_journal(-50.0);

      let result = journal_matches(&j, true, None, &SignFilter::In, "");

      assert!(!result);
    }

    #[test]
    fn it_excludes_positive_delta_for_sign_filter_out() {
      let j = make_journal(50.0);

      let result = journal_matches(&j, true, None, &SignFilter::Out, "");

      assert!(!result);
    }
  }

  mod map_txn_row {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_txn(type_id: i32, qty: i32, unit_price: f64, is_buy: bool) -> WalletTransaction {
      WalletTransaction {
        character_id: 1,
        transaction_id: 99,
        type_id,
        quantity: qty,
        unit_price,
        is_buy,
        date: "2024-01-01T00:00:00Z".to_string(),
        location_id: 60_000_000,
        client_id: 0,
      }
    }

    #[test]
    fn it_computes_total_as_unit_price_times_qty() {
      let mut names = HashMap::new();
      names.insert(34, "Tritanium".to_string());
      let row = make_txn(34, 100, 5.5, false);

      let entry = map_txn_row(row, &names);

      assert_eq!(entry.total, 550.0);
    }

    #[test]
    fn it_uses_type_name_from_map_when_present() {
      let mut names = HashMap::new();
      names.insert(34, "Tritanium".to_string());
      let row = make_txn(34, 1, 1.0, false);

      let entry = map_txn_row(row, &names);

      assert_eq!(entry.item, "Tritanium");
    }

    #[test]
    fn it_uses_type_name_when_present_in_map() {
      let mut names = HashMap::new();
      names.insert(34, "Tritanium".to_string());
      let row = make_txn(34, 1, 1.0, false);

      let entry = map_txn_row(row, &names);

      assert_eq!(entry.item, "Tritanium");
    }

    #[test]
    fn it_sets_side_to_buy_when_is_buy_is_true() {
      let mut names = HashMap::new();
      names.insert(34, "Tritanium".to_string());
      let row = make_txn(34, 1, 1.0, true);

      let entry = map_txn_row(row, &names);

      assert_eq!(entry.side, "buy");
    }

    #[test]
    fn it_sets_side_to_sell_when_is_buy_is_false() {
      let mut names = HashMap::new();
      names.insert(34, "Tritanium".to_string());
      let row = make_txn(34, 1, 1.0, false);

      let entry = map_txn_row(row, &names);

      assert_eq!(entry.side, "sell");
    }
  }

  mod market_owner_matches {
    use super::*;

    fn make_entry(who: i64) -> MarketEntry {
      MarketEntry {
        id: String::new(),
        who,
        type_id: 34,
        side: "buy".to_string(),
        qty: 1,
        item: "Tritanium".to_string(),
        unit: 5.0,
        total: 5.0,
        fee: 0.0,
        ts_secs: 0,
        location: String::new(),
      }
    }

    #[test]
    fn it_allows_any_owner_when_corp_selected() {
      let m = make_entry(999);

      assert!(market_owner_matches(&m, true, Some(10)));
    }

    #[test]
    fn it_allows_any_owner_when_who_is_none() {
      let m = make_entry(999);

      assert!(market_owner_matches(&m, false, None));
    }

    #[test]
    fn it_includes_matching_owner() {
      let m = make_entry(10);

      assert!(market_owner_matches(&m, false, Some(10)));
    }

    #[test]
    fn it_excludes_wrong_owner_when_not_corp_selected() {
      let m = make_entry(999);

      assert!(!market_owner_matches(&m, false, Some(10)));
    }
  }

  mod market_query_matches {
    use super::*;

    fn make_entry(item: &str) -> MarketEntry {
      MarketEntry {
        id: String::new(),
        who: 1,
        type_id: 34,
        side: "sell".to_string(),
        qty: 1,
        item: item.to_string(),
        unit: 1.0,
        total: 1.0,
        fee: 0.0,
        ts_secs: 0,
        location: String::new(),
      }
    }

    #[test]
    fn it_matches_when_query_is_empty() {
      let m = make_entry("Tritanium");

      assert!(market_query_matches(&m, ""));
    }

    #[test]
    fn it_matches_case_insensitively() {
      let m = make_entry("Tritanium");

      assert!(market_query_matches(&m, "trit"));
    }

    #[test]
    fn it_excludes_non_matching_item() {
      let m = make_entry("Tritanium");

      assert!(!market_query_matches(&m, "veldspar"));
    }
  }

  mod market_side_matches {
    use super::*;

    fn make_entry(side: &str) -> MarketEntry {
      MarketEntry {
        id: String::new(),
        who: 1,
        type_id: 34,
        side: side.to_string(),
        qty: 1,
        item: "Tritanium".to_string(),
        unit: 1.0,
        total: 1.0,
        fee: 0.0,
        ts_secs: 0,
        location: String::new(),
      }
    }

    #[test]
    fn it_matches_buy_entry_for_all_filter() {
      let m = make_entry("buy");

      assert!(market_side_matches(&m, &SideFilter::All));
    }

    #[test]
    fn it_matches_sell_entry_for_all_filter() {
      let m = make_entry("sell");

      assert!(market_side_matches(&m, &SideFilter::All));
    }

    #[test]
    fn it_matches_buy_entry_for_buy_filter() {
      let m = make_entry("buy");

      assert!(market_side_matches(&m, &SideFilter::Buy));
    }

    #[test]
    fn it_excludes_sell_entry_for_buy_filter() {
      let m = make_entry("sell");

      assert!(!market_side_matches(&m, &SideFilter::Buy));
    }

    #[test]
    fn it_matches_sell_entry_for_sell_filter() {
      let m = make_entry("sell");

      assert!(market_side_matches(&m, &SideFilter::Sell));
    }

    #[test]
    fn it_excludes_buy_entry_for_sell_filter() {
      let m = make_entry("buy");

      assert!(!market_side_matches(&m, &SideFilter::Sell));
    }
  }

  mod update_journal_totals {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_journal_entries(deltas: Vec<f64>) -> Vec<JournalEntry> {
      deltas
        .into_iter()
        .enumerate()
        .map(|(i, delta)| JournalEntry {
          id: i.to_string(),
          who: 0,
          entry_type: String::new(),
          delta,
          ts_secs: 0,
          reference: String::new(),
          party: String::new(),
          location: String::new(),
        })
        .collect()
    }

    #[test]
    fn it_sums_only_positive_deltas_for_income() {
      let mut state = build_wallet_state(&[], &[], 0.0);
      state.filtered_journal = make_journal_entries(vec![100.0, -50.0, 200.0]);

      update_journal_totals(&mut state);

      assert_eq!(state.journal_income, 300.0);
    }

    #[test]
    fn it_sums_absolute_values_of_negative_deltas_for_spend() {
      let mut state = build_wallet_state(&[], &[], 0.0);
      state.filtered_journal = make_journal_entries(vec![100.0, -50.0, -80.0]);

      update_journal_totals(&mut state);

      assert_eq!(state.journal_spend, 130.0);
    }
  }
}
