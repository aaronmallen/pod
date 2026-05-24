//! Wallet controller: startup and background data fetches.

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

use crate::services::{Services, character as character_service, corporation as corporation_service};

/// Creates the initial wallet state and kicks off background data fetches.
pub fn new(
  characters: Vec<Character>,
  corporations: Vec<Corporation>,
  services: &Services,
  right_rail_width: f32,
) -> (State, iced::Task<Message>) {
  let state = build_wallet_state(&characters, &corporations, right_rail_width);
  let tasks = if let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) {
    build_initial_tasks(characters, corporations, esi, db)
  } else {
    iced::Task::none()
  };
  (state, tasks)
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
      let base = pod_ui::views::wallet::update(state, other);
      recompute_derived(state);
      attach_icon_task(base, icon_type_ids, services)
    }
  }
}

async fn apply_esi_names(loc_ids: &[i64], names: &mut HashMap<i64, String>, esi: &pod_esi::Client) {
  let unresolved: Vec<i64> = loc_ids.iter().filter(|id| !names.contains_key(id)).copied().collect();
  if !unresolved.is_empty()
    && let Ok(resolved) = esi.universe().names(&unresolved).await
  {
    for r in resolved {
      names.insert(r.id, r.name);
    }
  }
}

async fn apply_station_names(loc_ids: &[i64], names: &mut HashMap<i64, String>, db: &pod_db::Repo) {
  let station_ids = collect_station_ids(loc_ids);
  if !station_ids.is_empty()
    && let Ok(stations) = db.universe().stations().find_by_ids(&station_ids).await
  {
    for s in stations {
      names.insert(*s.id() as i64, s.name().clone());
    }
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

fn build_contract_entry(
  row: CharacterContract,
  names: &HashMap<i64, String>,
  locations: &HashMap<i64, String>,
) -> ContractEntry {
  let ts = parse_iso_to_unix(&row.date_issued);
  let counterparty_id = if row.issuer_id == row.character_id {
    if row.acceptor_id != 0 {
      row.acceptor_id
    } else {
      row.assignee_id
    }
  } else {
    row.issuer_id
  };
  let counterparty = if counterparty_id != 0 {
    names
      .get(&counterparty_id)
      .cloned()
      .expect("counterparty name must be resolved by ESI")
  } else {
    String::new()
  };
  let location = row
    .start_location_id
    .and_then(|id| locations.get(&id).cloned())
    .unwrap_or_default();
  let title = if row.title.is_empty() {
    format!("Contract #{}", row.contract_id)
  } else {
    row.title
  };
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

fn build_initial_tasks(
  characters: Vec<Character>,
  corporations: Vec<Corporation>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> iced::Task<Message> {
  let chars_j = characters.clone();
  let db_j = db.clone();
  let esi_j = esi.clone();
  let chars_t = characters.clone();
  let db_t = db.clone();
  let esi_t = esi.clone();
  let corps_b = corporations.clone();
  let db_b = db.clone();
  let esi_b = esi.clone();
  let chars_a = characters.clone();
  let db_a = db.clone();
  let chars_c = characters.clone();
  let db_c = db.clone();
  let esi_c = esi.clone();
  let db_i = db.clone();
  iced::Task::batch([
    iced::Task::perform(
      async move { fetch_journal(chars_j, esi_j, db_j).await },
      Message::JournalLoaded,
    ),
    iced::Task::perform(
      async move { fetch_transactions(chars_t, esi_t, db_t).await },
      Message::TransactionsLoaded,
    ),
    iced::Task::perform(
      async move { fetch_all_corp_totals(corps_b, esi_b, db_b).await },
      Message::AllCorpBalancesLoaded,
    ),
    iced::Task::perform(
      async move { fetch_asset_values(chars_a, db_a).await },
      Message::AssetValuesLoaded,
    ),
    iced::Task::perform(
      async move { fetch_contracts(chars_c, esi_c, db_c).await },
      Message::ContractsLoaded,
    ),
    iced::Task::perform(
      async move { load_all_cached_icons(db_i).await },
      Message::ItemIconsLoaded,
    ),
  ])
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

fn compute_icon_type_ids(message: &Message, state: &State) -> Option<Vec<i32>> {
  let market_entries: Option<&Vec<MarketEntry>> = match message {
    Message::TransactionsLoaded(entries) => Some(entries),
    Message::CorpDataLoaded {
      market, ..
    } => Some(market),
    _ => None,
  };
  market_entries
    .map(|entries| {
      entries
        .iter()
        .map(|e| e.type_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .filter(|id| !state.item_icons.contains_key(id))
        .collect::<Vec<_>>()
    })
    .filter(|ids| !ids.is_empty())
}

async fn fetch_all_corp_totals(
  corporations: Vec<Corporation>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Vec<(i64, f64)> {
  let mut totals = Vec::new();
  for corp in &corporations {
    let Some(token) = corporation_service::ensure_valid_token(corp, &esi, &db).await else {
      continue;
    };
    let grant = corporation_service::refresh_grant(corp, &token);
    let corp_id = *corp.id();
    let corp_esi = esi.corporation(corp_id);
    let corp_client = corp_esi.auth(&grant);
    if let Ok(wallets) = corp_client.wallets().await {
      let total: f64 = wallets.iter().map(|w| w.balance).sum();
      totals.push((corp_id, total));
    }
  }
  totals
}

async fn fetch_asset_values(characters: Vec<Character>, db: pod_db::Repo) -> Vec<(i64, f64)> {
  let char_ids: Vec<i64> = characters.iter().map(|c| *c.id()).collect();
  if char_ids.is_empty() {
    return Vec::new();
  }

  let asset_rows = db
    .characters()
    .assets_for_character_ids(&char_ids)
    .await
    .unwrap_or_default();

  if asset_rows.is_empty() {
    return Vec::new();
  }

  let type_ids: Vec<i32> = asset_rows.iter().map(|a| a.type_id).collect();
  let prices = db.prices().latest_prices(&type_ids).await.unwrap_or_default();

  let mut totals: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
  for asset in &asset_rows {
    let price = prices.get(&asset.type_id).copied().unwrap_or(0.0);
    let value = price * asset.quantity as f64;
    *totals.entry(asset.character_id).or_insert(0.0) += value;
  }

  totals.into_iter().collect()
}

async fn fetch_contracts(
  characters: Vec<Character>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Result<Vec<ContractEntry>, String> {
  let mut all: Vec<ContractEntry> = Vec::new();
  for character in &characters {
    let Some(token) = character_service::ensure_valid_token(character, &esi, &db).await else {
      continue;
    };
    let grant = character_service::refresh_grant(character, &token);
    let char_client = esi.character(&grant);
    if let Ok(contracts) = char_client.contracts().await {
      let db_rows: Vec<_> = contracts
        .iter()
        .map(|c| CharacterContract {
          character_id: *character.id(),
          contract_id: c.contract_id,
          contract_type: c.r#type.clone(),
          status: c.status.clone(),
          title: c.title.clone().unwrap_or_default(),
          issuer_id: c.issuer_id,
          assignee_id: c.assignee_id,
          acceptor_id: c.acceptor_id,
          price: c.price,
          collateral: c.collateral,
          date_issued: c.date_issued.clone(),
          date_expired: c.date_expired.clone(),
          start_location_id: c.start_location_id,
        })
        .collect();
      let _ = db.characters().upsert_contracts(*character.id(), &db_rows).await;
    }
    if let Ok(rows) = db.characters().contracts(*character.id()).await {
      let names = resolve_entity_names(&rows, &esi).await?;
      let locations = resolve_location_names(&rows, &esi, &db).await;
      for row in rows {
        all.push(build_contract_entry(row, &names, &locations));
      }
    }
  }
  all.sort_by_key(|e| e.ts_secs);
  Ok(all)
}

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
  let corp_type_ids: Vec<i32> = raw_txns
    .iter()
    .map(|t| t.type_id)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  let corp_type_names = load_type_names(&corp_type_ids, &db).await;
  let market: Vec<MarketEntry> = raw_txns
    .into_iter()
    .map(|t| map_corp_txn_entry(t, corp_id, division, &corp_type_names))
    .collect();
  (divisions, journal, market)
}

async fn fetch_journal(characters: Vec<Character>, esi: pod_esi::Client, db: pod_db::Repo) -> Vec<JournalEntry> {
  let mut all: Vec<JournalEntry> = Vec::new();
  for character in &characters {
    let Some(token) = character_service::ensure_valid_token(character, &esi, &db).await else {
      continue;
    };
    let grant = character_service::refresh_grant(character, &token);
    let char_client = esi.character(&grant);
    if let Ok(entries) = char_client.wallet_journal().await {
      let db_rows: Vec<_> = entries
        .iter()
        .map(|e| WalletJournalEntry {
          character_id: *character.id(),
          entry_id: e.id,
          ref_type: e.ref_type.clone(),
          amount: e.amount,
          balance: e.balance,
          date: e.date.clone(),
          description: e.description.clone(),
          first_party_id: e.first_party_id,
          second_party_id: e.second_party_id,
        })
        .collect();
      let _ = db.characters().upsert_journal_entries(*character.id(), &db_rows).await;
    }
    if let Ok(rows) = db.characters().journal_entries(*character.id()).await {
      for row in rows {
        let ts = parse_iso_to_unix(&row.date);
        all.push(JournalEntry {
          id: format!("{}-{}", row.character_id, row.entry_id),
          who: row.character_id,
          entry_type: row.ref_type,
          delta: row.amount.unwrap_or(0.0),
          ts_secs: secs_ago(ts),
          reference: row.description,
          party: format_party_id(row.first_party_id.or(row.second_party_id)),
          location: String::new(),
        });
      }
    }
  }
  all.sort_by_key(|e| e.ts_secs);
  all
}

async fn fetch_transactions(characters: Vec<Character>, esi: pod_esi::Client, db: pod_db::Repo) -> Vec<MarketEntry> {
  let mut all: Vec<MarketEntry> = Vec::new();
  for character in &characters {
    let Some(token) = character_service::ensure_valid_token(character, &esi, &db).await else {
      continue;
    };
    let grant = character_service::refresh_grant(character, &token);
    let char_client = esi.character(&grant);
    if let Ok(txns) = char_client.wallet_transactions().await {
      let db_rows: Vec<_> = txns
        .iter()
        .map(|t| WalletTransaction {
          character_id: *character.id(),
          transaction_id: t.transaction_id,
          type_id: t.type_id,
          quantity: t.quantity,
          unit_price: t.unit_price,
          is_buy: t.is_buy,
          date: t.date.clone(),
          location_id: t.location_id,
          client_id: t.client_id,
        })
        .collect();
      let _ = db.characters().upsert_transactions(*character.id(), &db_rows).await;
    }
    if let Ok(rows) = db.characters().transactions(*character.id()).await {
      let type_ids: Vec<i32> = rows
        .iter()
        .map(|r| r.type_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
      let type_names = load_type_names(&type_ids, &db).await;
      for row in rows {
        all.push(map_txn_row(row, &type_names));
      }
    }
  }
  all.sort_by_key(|e| e.ts_secs);
  all
}

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

async fn load_all_cached_icons(db: pod_db::Repo) -> Vec<(i32, Vec<u8>)> {
  db.universe()
    .type_icons()
    .find_all()
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(type_id, _variant, data)| (type_id, data))
    .collect()
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
    .expect("item type must exist in SDE");
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
    .expect("item type must exist in SDE");
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

fn recompute_filters(state: &mut State) {
  let who = state.selected_character();
  let q = state.search_query.to_lowercase();
  let corp_selected = state.is_corp_selected();
  let sign = state.sign_filter.clone();
  let side = state.side_filter.clone();
  let journal_source = if corp_selected {
    &state.corp_journal
  } else {
    &state.journal
  };
  state.filtered_journal = journal_source
    .iter()
    .filter(|j| journal_matches(j, corp_selected, who, &sign, &q))
    .cloned()
    .collect();
  let market_source = if corp_selected {
    &state.corp_market
  } else {
    &state.market
  };
  state.filtered_market = market_source
    .iter()
    .filter(|m| market_matches(m, corp_selected, who, &side, &q))
    .cloned()
    .collect();
  state.filtered_contracts = state
    .contracts
    .iter()
    .filter(|c| {
      if let Some(id) = who
        && c.who != id
      {
        return false;
      }
      if !q.is_empty() && !c.title.to_lowercase().contains(&q) {
        return false;
      }
      true
    })
    .cloned()
    .collect();
  update_journal_totals(state);
}

fn recompute_series(state: &mut State) {
  let corp_selected = state.is_corp_selected();
  let selected_char = state.selected_character();
  let mut entries: Vec<JournalEntry> = if corp_selected {
    state.corp_journal.clone()
  } else {
    state
      .journal
      .iter()
      .filter(|e| selected_char.is_none_or(|id| e.who == id))
      .cloned()
      .collect()
  };
  entries.sort_by_key(|e| Reverse(e.ts_secs));
  state.net_worth_series = build_net_worth_series(&entries);

  let days = state.timeframe.days();
  let total = state.net_worth_series.len();
  state.chart_series = if total <= days {
    state.net_worth_series.clone()
  } else {
    state.net_worth_series[total - days..].to_vec()
  };

  state.net_worth_change = if state.chart_series.len() < 2 {
    0.0
  } else {
    let s = &state.chart_series;
    s[s.len() - 1] - s[0]
  };
}

async fn resolve_entity_names(
  rows: &[CharacterContract],
  esi: &pod_esi::Client,
) -> Result<HashMap<i64, String>, String> {
  let ids: Vec<i64> = rows
    .iter()
    .flat_map(|r| [r.acceptor_id, r.assignee_id])
    .filter(|&id| id != 0)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  if ids.is_empty() {
    return Ok(HashMap::new());
  }
  let resolved: HashMap<i64, String> = esi
    .universe()
    .names(&ids)
    .await
    .map_err(|e| format!("ESI name resolution failed: {e}"))?
    .into_iter()
    .map(|r| (r.id, r.name))
    .collect();
  let still_missing: Vec<i64> = ids.into_iter().filter(|id| !resolved.contains_key(id)).collect();
  if still_missing.is_empty() {
    Ok(resolved)
  } else {
    Err(format!(
      "could not resolve ESI names for contract counterparty IDs: {still_missing:?}"
    ))
  }
}

async fn resolve_location_names(
  rows: &[CharacterContract],
  esi: &pod_esi::Client,
  db: &pod_db::Repo,
) -> HashMap<i64, String> {
  let loc_ids: Vec<i64> = rows
    .iter()
    .filter_map(|r| r.start_location_id)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  if loc_ids.is_empty() {
    return HashMap::new();
  }
  let mut names: HashMap<i64, String> = HashMap::new();
  apply_station_names(&loc_ids, &mut names, db).await;
  apply_esi_names(&loc_ids, &mut names, esi).await;
  names
}

fn secs_ago(ts: i64) -> u64 {
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs() as i64;
  (now - ts).max(0) as u64
}

fn update_character_picker(
  state: &mut State,
  msg: pod_ui::components::character_picker::Message,
  corporations: &[Corporation],
  services: &Services,
  icon_type_ids: Option<Vec<i32>>,
) -> iced::Task<Message> {
  if let pod_ui::components::character_picker::Message::Select(PickerSelection::Corporation(corp_id)) = &msg
    && let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone())
  {
    let corp_id = *corp_id;
    let corps = corporations.to_vec();
    let _ = pod_ui::views::wallet::update(state, Message::CharacterPicker(msg));
    recompute_derived(state);
    return iced::Task::perform(
      async move { fetch_corp_data(corp_id, 1, corps, esi, db).await },
      |(divisions, journal, market)| Message::CorpDataLoaded {
        divisions,
        journal,
        market,
      },
    );
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
