//! Wallet controller: startup and background data fetches.

use std::{
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

/// Creates the initial wallet state and kicks off background data fetches.
pub fn new(
  characters: Vec<Character>,
  corporations: Vec<Corporation>,
  services: &Services,
  right_rail_width: f32,
) -> (State, iced::Task<Message>) {
  let wallet_chars: Vec<WalletCharacter> = characters
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
    .collect();

  let picker_entries = build_picker_entries(&characters);
  let corp_entries = build_corp_picker_entries(&corporations);
  let picker = CharacterPicker::new()
    .entries(picker_entries)
    .corp_entries(corp_entries)
    .show_all(true);

  let state = State {
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
  };

  let tasks = if let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) {
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
    let esi_a = esi.clone();
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
        async move { fetch_asset_values(chars_a, esi_a, db_a).await },
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
  } else {
    iced::Task::none()
  };

  (state, tasks)
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
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
      let type_names: std::collections::HashMap<i32, String> = db
        .universe()
        .item_types()
        .find_by_ids(&type_ids)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|t| (t.id, t.name))
        .collect();
      for row in rows {
        let ts = parse_iso_to_unix(&row.date);
        let total = row.unit_price * row.quantity as f64;
        let item = type_names
          .get(&row.type_id)
          .cloned()
          .unwrap_or_else(|| format!("Type #{}", row.type_id));
        all.push(MarketEntry {
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
        });
      }
    }
  }
  all.sort_by_key(|e| e.ts_secs);
  all
}

async fn fetch_contracts(characters: Vec<Character>, esi: pod_esi::Client, db: pod_db::Repo) -> Vec<ContractEntry> {
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
      let names = resolve_entity_names(&rows, &esi).await;
      let locations = resolve_location_names(&rows, &esi, &db).await;
      for row in rows {
        let ts = parse_iso_to_unix(&row.date_issued);
        let counterparty_id = if row.acceptor_id != 0 {
          row.acceptor_id
        } else {
          row.assignee_id
        };
        let counterparty = if counterparty_id != 0 {
          names
            .get(&counterparty_id)
            .cloned()
            .unwrap_or_else(|| format!("#{counterparty_id}"))
        } else {
          String::new()
        };
        let location = row
          .start_location_id
          .and_then(|id| locations.get(&id).cloned())
          .unwrap_or_default();
        all.push(ContractEntry {
          id: format!("{}-{}", row.character_id, row.contract_id),
          who: row.character_id,
          kind: row.contract_type,
          status: row.status,
          title: if row.title.is_empty() {
            format!("Contract #{}", row.contract_id)
          } else {
            row.title
          },
          counterparty,
          price: row.price.unwrap_or(0.0),
          collateral: row.collateral.unwrap_or(0.0),
          ts_secs: secs_ago(ts),
          location,
        });
      }
    }
  }
  all.sort_by_key(|e| e.ts_secs);
  all
}

async fn resolve_entity_names(rows: &[CharacterContract], esi: &pod_esi::Client) -> HashMap<i64, String> {
  let ids: Vec<i64> = rows
    .iter()
    .flat_map(|r| [r.acceptor_id, r.assignee_id])
    .filter(|&id| id != 0)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  if ids.is_empty() {
    return HashMap::new();
  }
  esi
    .universe()
    .names(&ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.id, r.name))
    .collect()
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

  // NPC stations have IDs < 100_000_000 and are in the SDE station table.
  let station_ids: Vec<i32> = loc_ids
    .iter()
    .filter(|&&id| id < 100_000_000)
    .filter_map(|&id| i32::try_from(id).ok())
    .collect();
  if !station_ids.is_empty()
    && let Ok(stations) = db.universe().stations().find_by_ids(&station_ids).await
  {
    for s in stations {
      names.insert(*s.id() as i64, s.name().clone());
    }
  }

  // For any remaining IDs not resolved from the DB, try the universe names API.
  // This covers NPC stations not yet in the local DB and may cover some structures.
  let unresolved: Vec<i64> = loc_ids.iter().filter(|id| !names.contains_key(id)).copied().collect();
  if !unresolved.is_empty()
    && let Ok(resolved) = esi.universe().names(&unresolved).await
  {
    for r in resolved {
      names.insert(r.id, r.name);
    }
  }

  names
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
    .map(|e| {
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
    })
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
  let corp_type_names: HashMap<i32, String> = db
    .universe()
    .item_types()
    .find_by_ids(&corp_type_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|t| (t.id, t.name))
    .collect();

  let market: Vec<MarketEntry> = raw_txns
    .into_iter()
    .map(|t| {
      let ts = parse_iso_to_unix(&t.date);
      let total = t.unit_price * t.quantity as f64;
      let item = corp_type_names
        .get(&t.type_id)
        .cloned()
        .unwrap_or_else(|| format!("Type #{}", t.type_id));
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
    })
    .collect();

  (divisions, journal, market)
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
  // Sort oldest-first (ts_secs = seconds-ago, so largest value = oldest entry)
  entries.sort_by(|a, b| b.ts_secs.cmp(&a.ts_secs));
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

fn recompute_filters(state: &mut State) {
  let who = state.selected_character();
  let q = state.search_query.to_lowercase();
  let corp_selected = state.is_corp_selected();

  let sign = state.sign_filter.clone();
  let source = if corp_selected {
    &state.corp_journal
  } else {
    &state.journal
  };
  state.filtered_journal = source
    .iter()
    .filter(|j| {
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
        && !j.reference.to_lowercase().contains(&q)
        && !j.party.to_lowercase().contains(&q)
        && !j.location.to_lowercase().contains(&q)
      {
        return false;
      }
      true
    })
    .cloned()
    .collect();

  let side = state.side_filter.clone();
  let source = if corp_selected {
    &state.corp_market
  } else {
    &state.market
  };
  state.filtered_market = source
    .iter()
    .filter(|m| {
      if !corp_selected
        && let Some(id) = who
        && m.who != id
      {
        return false;
      }
      match side {
        SideFilter::Buy if m.side != "buy" => return false,
        SideFilter::Sell if m.side != "sell" => return false,
        _ => {}
      }
      if !q.is_empty() && !m.item.to_lowercase().contains(&q) {
        return false;
      }
      true
    })
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

fn recompute_derived(state: &mut State) {
  recompute_filters(state);
  recompute_series(state);
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

fn secs_ago(ts: i64) -> u64 {
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs() as i64;
  (now - ts).max(0) as u64
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

async fn fetch_asset_values(characters: Vec<Character>, esi: pod_esi::Client, db: pod_db::Repo) -> Vec<(i64, f64)> {
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

  let prices: std::collections::HashMap<i32, f64> = esi
    .market()
    .prices()
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|p| (p.type_id, p.adjusted_price.or(p.average_price).unwrap_or(0.0)))
    .collect();

  let mut totals: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
  for asset in &asset_rows {
    let price = prices.get(&asset.type_id).copied().unwrap_or(0.0);
    let value = price * asset.quantity as f64;
    *totals.entry(asset.character_id).or_insert(0.0) += value;
  }

  totals.into_iter().collect()
}

fn format_party_id(id: Option<i64>) -> String {
  match id {
    Some(n) => format!("#{n}"),
    None => String::new(),
  }
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
