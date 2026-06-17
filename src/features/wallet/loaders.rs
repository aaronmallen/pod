use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::{
  clients::{
    eve_image::{self, Size},
    http,
  },
  store::{
    Database,
    images::{self, IconResolution},
    repo::finance,
  },
};

const MARKET_ICON_SIZE: Size = Size::S64;

#[derive(Clone, Debug, PartialEq)]
pub struct ContractEntry {
  pub acceptor: Option<String>,
  pub acceptor_id: Option<i64>,
  pub acceptor_image: PartyImage,
  pub assignee: Option<String>,
  pub assignee_id: Option<i64>,
  pub assignee_image: PartyImage,
  pub character_id: i64,
  pub collateral: Option<f64>,
  pub contract_id: i64,
  pub date_expired: Option<String>,
  pub date_issued: String,
  pub is_buy: bool,
  pub issuer: Option<String>,
  pub issuer_id: i64,
  pub issuer_image: PartyImage,
  pub status: String,
  pub r#type: String,
  pub value: Option<f64>,
}

impl ContractEntry {
  /// Status as the modal maps it, with `expired` derived from the expiry timestamp.
  ///
  /// ESI never emits an `expired` status: an unaccepted contract simply stays
  /// `outstanding` past its expiry. Outbid is not derivable here (the list view
  /// carries no bid data), so an `outbid` status is only ever passed through.
  pub fn derived_status(&self, now: DateTime<Utc>) -> String {
    if matches!(self.status.as_str(), "outstanding" | "in_progress")
      && self
        .date_expired
        .as_deref()
        .and_then(|iso| DateTime::parse_from_rfc3339(iso).ok())
        .is_some_and(|expiry| expiry.with_timezone(&Utc) < now)
    {
      return "expired".to_owned();
    }
    self.status.clone()
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct JournalEntry {
  pub amount: Option<f64>,
  pub balance: Option<f64>,
  pub character_id: i64,
  pub date: String,
  pub description: String,
  pub id: i64,
  pub ref_type: String,
}

impl JournalEntry {
  pub fn is_income(&self) -> bool {
    self.amount.is_some_and(|amount| amount > 0.0)
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarketEntry {
  pub character_id: i64,
  pub date: String,
  pub is_buy: bool,
  pub item: String,
  pub location: String,
  pub quantity: i64,
  pub total: f64,
  pub transaction_id: i64,
  pub type_icon: IconResolution,
  pub type_id: i64,
  pub unit_price: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartyImage {
  pub path: Option<std::path::PathBuf>,
  pub stale: Vec<(images::ImageKind, i64)>,
}

/// Resolves a contract party's portrait/logo once at load time so the row never stats the filesystem in `view`,
/// and `stale_images` reads the cached candidate keys instead of re-resolving. A non-positive `id` (no party, or a
/// missing acceptor/assignee passed as `0`) yields an empty result.
pub(super) fn party_image(store: &images::Store, id: i64) -> PartyImage {
  if id <= 0 {
    return PartyImage::default();
  }
  let portrait = images::resolve(store, images::ImageKind::CharacterPortrait, id);
  let logo = images::resolve(store, images::ImageKind::CorporationLogo, id);
  let path = portrait.path().or_else(|| logo.path());
  let stale = match path {
    Some(_) => Vec::new(),
    None => [portrait.stale_key(), logo.stale_key()].into_iter().flatten().collect(),
  };
  PartyImage {
    path,
    stale,
  }
}

fn map_contract_row(row: &crate::store::model::CharacterContract) -> ContractEntry {
  let price = row.price();
  let store = images::default_store();
  ContractEntry {
    acceptor: row.acceptor_name().clone(),
    acceptor_id: row.acceptor_id(),
    acceptor_image: party_image(&store, row.acceptor_id().unwrap_or(0)),
    assignee: row.assignee_name().clone(),
    assignee_id: row.assignee_id(),
    assignee_image: party_image(&store, row.assignee_id().unwrap_or(0)),
    character_id: row.character_id(),
    collateral: row.collateral(),
    contract_id: row.contract_id(),
    date_expired: row.date_expired().clone(),
    date_issued: row.date_issued().clone(),
    is_buy: !price.is_some_and(|value| value > 0.0),
    issuer: row.issuer_name().clone(),
    issuer_id: row.issuer_id(),
    issuer_image: party_image(&store, row.issuer_id()),
    status: row.status().clone(),
    value: price.or_else(|| row.reward()),
    r#type: row.r#type().clone(),
  }
}

fn map_corp_contract_row(row: &crate::store::model::CorporationContract) -> ContractEntry {
  let price = row.price();
  let store = images::default_store();
  ContractEntry {
    acceptor: row.acceptor_name().clone(),
    acceptor_id: row.acceptor_id(),
    acceptor_image: party_image(&store, row.acceptor_id().unwrap_or(0)),
    assignee: row.assignee_name().clone(),
    assignee_id: row.assignee_id(),
    assignee_image: party_image(&store, row.assignee_id().unwrap_or(0)),
    character_id: row.corporation_id(),
    collateral: row.collateral(),
    contract_id: row.contract_id(),
    date_expired: row.date_expired().clone(),
    date_issued: row.date_issued().clone(),
    is_buy: !price.is_some_and(|value| value > 0.0),
    issuer: row.issuer_name().clone(),
    issuer_id: row.issuer_id(),
    issuer_image: party_image(&store, row.issuer_id()),
    status: row.status().clone(),
    value: price.or_else(|| row.reward()),
    r#type: row.r#type().clone(),
  }
}

pub async fn load_journal_page(db: &Database, scope: &[i64], cursor: Option<i64>, limit: i64) -> Vec<JournalEntry> {
  let mut entries = Vec::new();
  for &character_id in scope {
    let rows = finance::wallet_journal_page(db, character_id, cursor, limit)
      .await
      .unwrap_or_default();
    entries.extend(rows.into_iter().map(|row| map_journal_row(&row)));
  }
  entries.sort_by_key(|entry| std::cmp::Reverse(entry.id));
  entries.truncate(limit as usize);
  entries
}

pub async fn load_market_page(db: &Database, scope: &[i64], cursor: Option<i64>, limit: i64) -> Vec<MarketEntry> {
  let type_names = load_type_names(db).await;
  let location_names = load_location_names(db).await;

  let mut entries = Vec::new();
  for &character_id in scope {
    let rows = finance::wallet_transactions_page(db, character_id, cursor, limit)
      .await
      .unwrap_or_default();
    entries.extend(
      rows
        .into_iter()
        .filter_map(|row| map_txn_row(&row, &type_names, &location_names)),
    );
  }
  entries.sort_by_key(|entry| std::cmp::Reverse(entry.transaction_id));
  entries.truncate(limit as usize);
  entries
}

pub async fn load_contracts_page(
  db: &Database,
  scope: &[i64],
  cursor: Option<(String, i64)>,
  limit: i64,
) -> Vec<ContractEntry> {
  let after = cursor.as_ref().map(|(date, id)| (date.as_str(), *id));
  let mut entries = Vec::new();
  for &character_id in scope {
    let rows = finance::contracts_page(db, character_id, after, limit)
      .await
      .unwrap_or_default();
    entries.extend(rows.iter().map(map_contract_row));
  }
  entries.sort_by(|a, b| {
    b.date_issued
      .cmp(&a.date_issued)
      .then_with(|| b.contract_id.cmp(&a.contract_id))
  });
  entries.truncate(limit as usize);
  cache_contract_portraits(db, &entries).await;
  entries
}

pub async fn load_corp_contracts_page(
  db: &Database,
  corporation_id: i64,
  cursor: Option<(String, i64)>,
  limit: i64,
) -> Vec<ContractEntry> {
  let after = cursor.as_ref().map(|(date, id)| (date.as_str(), *id));
  let rows = finance::corporation_contracts_page(db, corporation_id, after, limit)
    .await
    .unwrap_or_default();
  let entries: Vec<ContractEntry> = rows.iter().map(map_corp_contract_row).collect();
  cache_contract_portraits(db, &entries).await;
  entries
}

fn contract_party_ids(entries: &[ContractEntry]) -> Vec<i64> {
  let mut ids: Vec<i64> = entries
    .iter()
    .flat_map(|entry| [Some(entry.issuer_id), entry.acceptor_id, entry.assignee_id])
    .flatten()
    .filter(|id| *id > 0)
    .collect();
  ids.sort_unstable();
  ids.dedup();
  ids
}

async fn cache_contract_portraits(db: &Database, entries: &[ContractEntry]) {
  let ids = contract_party_ids(entries);
  if ids.is_empty() {
    return;
  }

  let store = images::default_store();
  let pending = pending_party_ids(&store, ids);
  if pending.is_empty() {
    return;
  }

  let http = http::Client::builder(http::Cache::new(db.clone())).build();
  let eve_image = eve_image::Client::new(http);
  for id in pending {
    fetch_party_image(&store, &eve_image, id).await;
  }
}

fn pending_party_ids(store: &images::Store, ids: Vec<i64>) -> Vec<i64> {
  ids
    .into_iter()
    .filter(|id| {
      !images::is_fresh(&store.character_portrait_path(*id), images::STALE_AFTER)
        && !images::is_fresh(&store.corporation_logo_path(*id), images::STALE_AFTER)
    })
    .collect()
}

async fn fetch_party_image(store: &images::Store, eve_image: &eve_image::Client, id: i64) {
  let portrait_path = store.character_portrait_path(id);
  let portrait_url = eve_image.character_portrait_url(id, images::PORTRAIT_SIZE);
  if let Ok(bytes) = eve_image.fetch(&portrait_url).await {
    let _ = store.write(&portrait_path, &bytes);
    return;
  }
  let logo_path = store.corporation_logo_path(id);
  let logo_url = eve_image.corporation_logo_url(id, images::LOGO_SIZE);
  if let Ok(bytes) = eve_image.fetch(&logo_url).await {
    let _ = store.write(&logo_path, &bytes);
  }
}

pub async fn load_corp_journal(db: &Database, corporation_id: i64, division: i64) -> Vec<JournalEntry> {
  finance::corporation_wallet_journal(db, corporation_id, division)
    .await
    .unwrap_or_default()
    .iter()
    .map(map_corp_journal_row)
    .collect()
}

pub async fn load_corp_market(db: &Database, corporation_id: i64, division: i64) -> Vec<MarketEntry> {
  let type_names = load_type_names(db).await;
  let location_names = load_location_names(db).await;
  finance::corporation_wallet_transactions(db, corporation_id, division)
    .await
    .unwrap_or_default()
    .iter()
    .filter_map(|row| map_corp_txn_row(row, &type_names, &location_names))
    .collect()
}

fn map_corp_journal_row(row: &crate::store::model::CorporationWalletJournal) -> JournalEntry {
  JournalEntry {
    amount: row.amount(),
    balance: row.balance(),
    character_id: row.corporation_id(),
    date: row.date().clone(),
    description: row.description().clone(),
    id: row.id(),
    ref_type: row.ref_type().clone(),
  }
}

fn map_corp_txn_row(
  row: &crate::store::model::CorporationWalletTransaction,
  type_names: &HashMap<i64, String>,
  location_names: &HashMap<i64, String>,
) -> Option<MarketEntry> {
  let item = type_names.get(&row.type_id()).cloned()?;
  let location = location_names.get(&row.location_id()).cloned()?;

  Some(MarketEntry {
    character_id: row.corporation_id(),
    date: row.date().clone(),
    is_buy: row.is_buy(),
    item,
    location,
    quantity: row.quantity(),
    total: row.unit_price() * row.quantity() as f64,
    transaction_id: row.transaction_id(),
    type_icon: images::default_store().resolve_type_icon(row.type_id(), None, MARKET_ICON_SIZE),
    type_id: row.type_id(),
    unit_price: row.unit_price(),
  })
}

fn map_journal_row(row: &crate::store::model::CharacterWalletJournal) -> JournalEntry {
  JournalEntry {
    amount: row.amount(),
    balance: row.balance(),
    character_id: row.character_id(),
    date: row.date().clone(),
    description: row.description().clone(),
    id: row.id(),
    ref_type: row.ref_type().clone(),
  }
}

fn map_txn_row(
  row: &crate::store::model::CharacterWalletTransaction,
  type_names: &HashMap<i64, String>,
  location_names: &HashMap<i64, String>,
) -> Option<MarketEntry> {
  let item = type_names.get(&row.type_id()).cloned()?;
  let location = location_names.get(&row.location_id()).cloned()?;

  Some(MarketEntry {
    character_id: row.character_id(),
    date: row.date().clone(),
    is_buy: row.is_buy(),
    item,
    location,
    quantity: row.quantity(),
    total: row.unit_price() * row.quantity() as f64,
    transaction_id: row.transaction_id(),
    type_icon: images::default_store().resolve_type_icon(row.type_id(), None, MARKET_ICON_SIZE),
    type_id: row.type_id(),
    unit_price: row.unit_price(),
  })
}

async fn load_type_names(db: &Database) -> HashMap<i64, String> {
  crate::store::repo::sde::all_item_types(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|item| (item.id(), item.name().clone()))
    .collect()
}

async fn load_location_names(db: &Database) -> HashMap<i64, String> {
  let mut names = HashMap::new();
  for station in crate::store::repo::sde::all_stations(db).await.unwrap_or_default() {
    names.insert(station.id(), station.name().clone());
  }
  for structure in crate::store::repo::sde::all_structures(db).await.unwrap_or_default() {
    names.insert(structure.id(), structure.name().clone());
  }
  names
}

#[cfg(test)]
mod tests {
  use super::*;

  fn journal_row(
    id: i64,
    character_id: i64,
    amount: Option<f64>,
    balance: Option<f64>,
  ) -> crate::store::model::CharacterWalletJournal {
    crate::store::model::CharacterWalletJournal {
      amount,
      balance,
      character_id,
      context_id: None,
      context_id_type: None,
      date: "2026-05-30T12:00:00Z".to_owned(),
      description: "Bounty payout".to_owned(),
      first_party_id: None,
      id,
      reason: None,
      ref_type: "bounty_prizes".to_owned(),
      second_party_id: None,
      tax: None,
      tax_receiver_id: None,
    }
  }

  fn txn_row(
    transaction_id: i64,
    type_id: i64,
    location_id: i64,
    is_buy: bool,
    quantity: i64,
    unit_price: f64,
  ) -> crate::store::model::CharacterWalletTransaction {
    crate::store::model::CharacterWalletTransaction {
      character_id: 42,
      client_id: 1_000_035,
      date: "2026-05-30T12:00:00Z".to_owned(),
      is_buy,
      is_personal: true,
      journal_ref_id: 1,
      location_id,
      quantity,
      transaction_id,
      type_id,
      unit_price,
    }
  }

  mod contract_party_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    fn entry(issuer_id: i64, acceptor_id: Option<i64>, assignee_id: Option<i64>) -> ContractEntry {
      ContractEntry {
        acceptor: None,
        acceptor_id,
        acceptor_image: PartyImage::default(),
        assignee: None,
        assignee_id,
        assignee_image: PartyImage::default(),
        character_id: 42,
        collateral: None,
        contract_id: 1,
        date_expired: None,
        date_issued: "2026-05-30T12:00:00Z".to_owned(),
        is_buy: false,
        issuer: None,
        issuer_id,
        issuer_image: PartyImage::default(),
        status: "outstanding".to_owned(),
        value: None,
        r#type: "item_exchange".to_owned(),
      }
    }

    #[test]
    fn it_collects_every_present_party_id_deduplicated() {
      let entries = vec![entry(11, Some(22), Some(33)), entry(11, None, Some(44))];

      assert_eq!(super::contract_party_ids(&entries), vec![11, 22, 33, 44]);
    }

    #[test]
    fn it_drops_missing_and_non_positive_ids() {
      let entries = vec![entry(0, Some(-1), Some(55)), entry(66, None, None)];

      assert_eq!(super::contract_party_ids(&entries), vec![55, 66]);
    }
  }

  mod corp_mapping {
    use pretty_assertions::assert_eq;

    use super::*;

    fn corp_journal_row(id: i64, amount: Option<f64>) -> crate::store::model::CorporationWalletJournal {
      crate::store::model::CorporationWalletJournal {
        amount,
        balance: Some(50_000.0),
        context_id: None,
        context_id_type: None,
        corporation_id: 98_000_001,
        date: "2026-05-30T12:00:00Z".to_owned(),
        description: "Office rental".to_owned(),
        division: 1,
        first_party_id: Some(90_000_001),
        id,
        reason: None,
        ref_type: "office_rental_fee".to_owned(),
        second_party_id: None,
        tax: None,
        tax_receiver_id: None,
      }
    }

    fn corp_txn_row(
      transaction_id: i64,
      type_id: i64,
      location_id: i64,
      is_buy: bool,
      quantity: i64,
      unit_price: f64,
    ) -> crate::store::model::CorporationWalletTransaction {
      crate::store::model::CorporationWalletTransaction {
        client_id: 1_000_035,
        corporation_id: 98_000_001,
        date: "2026-05-30T12:00:00Z".to_owned(),
        division: 1,
        is_buy,
        journal_ref_id: 1,
        location_id,
        quantity,
        transaction_id,
        type_id,
        unit_price,
      }
    }

    #[test]
    fn it_maps_a_corp_journal_row_carrying_the_corporation_id_as_owner() {
      let entry = map_corp_journal_row(&corp_journal_row(7, Some(-1_000.0)));

      assert_eq!(entry.id, 7);
      assert_eq!(entry.amount, Some(-1_000.0));
      assert_eq!(entry.balance, Some(50_000.0));
      assert_eq!(entry.ref_type, "office_rental_fee");
      assert_eq!(entry.character_id, 98_000_001);
      assert!(!entry.is_income());
    }

    #[test]
    fn it_maps_a_corp_txn_row_resolving_names_and_deriving_total() {
      let type_names = HashMap::from([(34, "Tritanium".to_owned())]);
      let location_names = HashMap::from([(60_003_760, "Jita IV - Moon 4".to_owned())]);

      let entry = map_corp_txn_row(
        &corp_txn_row(9, 34, 60_003_760, false, 100, 4.0),
        &type_names,
        &location_names,
      )
      .expect("a fully resolved row is kept");

      assert_eq!(entry.item, "Tritanium");
      assert_eq!(entry.location, "Jita IV - Moon 4");
      assert_eq!(entry.total, 400.0);
      assert!(!entry.is_buy);
      assert_eq!(entry.character_id, 98_000_001);
    }

    #[test]
    fn it_withholds_a_corp_row_with_unresolved_ids() {
      let entry = map_corp_txn_row(
        &corp_txn_row(9, 999, 999, true, 1, 1.0),
        &HashMap::new(),
        &HashMap::new(),
      );

      assert!(entry.is_none(), "an unresolved corp transaction withholds the row");
    }
  }

  mod derived_status {
    use pretty_assertions::assert_eq;

    use super::*;

    fn entry(status: &str, date_expired: Option<&str>) -> ContractEntry {
      ContractEntry {
        acceptor: None,
        acceptor_id: None,
        acceptor_image: PartyImage::default(),
        assignee: None,
        assignee_id: None,
        assignee_image: PartyImage::default(),
        character_id: 42,
        collateral: None,
        contract_id: 1,
        date_expired: date_expired.map(str::to_owned),
        date_issued: "2026-05-30T12:00:00Z".to_owned(),
        is_buy: false,
        issuer: None,
        issuer_id: 11,
        issuer_image: PartyImage::default(),
        status: status.to_owned(),
        value: None,
        r#type: "item_exchange".to_owned(),
      }
    }

    #[test]
    fn it_derives_expired_for_an_outstanding_contract_past_its_expiry() {
      let now = DateTime::parse_from_rfc3339("2026-06-16T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

      assert_eq!(
        entry("outstanding", Some("2026-06-01T00:00:00Z")).derived_status(now),
        "expired"
      );
    }

    #[test]
    fn it_keeps_outstanding_before_its_expiry() {
      let now = DateTime::parse_from_rfc3339("2026-06-16T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

      assert_eq!(
        entry("outstanding", Some("2026-07-01T00:00:00Z")).derived_status(now),
        "outstanding"
      );
    }

    #[test]
    fn it_passes_through_a_terminal_status_even_when_expired() {
      let now = DateTime::parse_from_rfc3339("2026-06-16T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

      assert_eq!(
        entry("finished", Some("2026-06-01T00:00:00Z")).derived_status(now),
        "finished"
      );
    }
  }

  mod load_contracts_page {
    #[tokio::test]
    async fn it_returns_no_entries_for_a_character_with_no_contracts() {
      let db = crate::store::open_test().await.unwrap();

      let entries = super::load_contracts_page(&db, &[42], None, 10).await;

      assert!(entries.is_empty());
    }
  }

  mod load_corp_data {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, model::Corporation, repo::org};

    const CORP: i64 = 98_000_001;

    async fn seed_corp(db: &Database) {
      let mut corp = Corporation::new(CORP, "Test Corp", "TSTC");
      corp.set_ceo_id(100);
      corp.set_creator_id(100);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      org::upsert_corporation(db, &corp).await.unwrap();
    }

    fn corp_journal_dated(id: i64, division: i64, date: &str) -> crate::store::model::CorporationWalletJournal {
      crate::store::model::CorporationWalletJournal {
        amount: Some(1.0),
        balance: Some(1.0),
        context_id: None,
        context_id_type: None,
        corporation_id: CORP,
        date: date.to_owned(),
        description: "Entry".to_owned(),
        division,
        first_party_id: None,
        id,
        reason: None,
        ref_type: "player_donation".to_owned(),
        second_party_id: None,
        tax: None,
        tax_receiver_id: None,
      }
    }

    #[tokio::test]
    async fn it_reads_back_a_divisions_corp_journal_newest_first() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      finance::append_corporation_wallet_journal(
        &db,
        &[
          corp_journal_dated(1, 1, "2026-01-01T00:00:00Z"),
          corp_journal_dated(2, 1, "2026-03-01T00:00:00Z"),
        ],
      )
      .await
      .unwrap();
      finance::append_corporation_wallet_journal(&db, &[corp_journal_dated(3, 2, "2026-02-01T00:00:00Z")])
        .await
        .unwrap();

      let entries = load_corp_journal(&db, CORP, 1).await;

      assert_eq!(entries.iter().map(|e| e.id).collect::<Vec<_>>(), vec![2, 1]);
    }
  }

  mod load_journal {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::character::insert_with_org,
    };

    async fn seed_character(db: &Database, id: i64) {
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
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    async fn append(db: &Database, id: i64, character_id: i64, date: &str) {
      finance::append_wallet_journal(db, &[journal_row_dated(id, character_id, date)])
        .await
        .unwrap();
    }

    fn journal_row_dated(id: i64, character_id: i64, date: &str) -> crate::store::model::CharacterWalletJournal {
      let mut row = journal_row(id, character_id, Some(1.0), Some(1.0));
      row.date = date.to_owned();
      row
    }

    #[tokio::test]
    async fn it_pages_the_union_after_a_cursor_id() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_character(&db, 2).await;
      append(&db, 1, 1, "2026-01-01T00:00:00Z").await;
      append(&db, 2, 2, "2026-03-01T00:00:00Z").await;
      append(&db, 3, 1, "2026-02-01T00:00:00Z").await;

      let first = load_journal_page(&db, &[1, 2], None, 2).await;
      let cursor = first.last().map(|e| e.id);
      let next = load_journal_page(&db, &[1, 2], cursor, 2).await;

      assert_eq!(first.iter().map(|e| e.id).collect::<Vec<_>>(), [3, 2]);
      assert_eq!(next.iter().map(|e| e.id).collect::<Vec<_>>(), [1]);
    }

    #[tokio::test]
    async fn it_scopes_to_a_single_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_character(&db, 2).await;
      append(&db, 1, 1, "2026-01-01T00:00:00Z").await;
      append(&db, 2, 2, "2026-03-01T00:00:00Z").await;

      let entries = load_journal_page(&db, &[1], None, 50).await;

      assert_eq!(entries.len(), 1);
      assert_eq!(entries[0].character_id, 1);
    }

    #[tokio::test]
    async fn it_unions_rows_across_all_in_scope_characters_newest_id_first() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_character(&db, 2).await;
      append(&db, 1, 1, "2026-01-01T00:00:00Z").await;
      append(&db, 2, 2, "2026-03-01T00:00:00Z").await;
      append(&db, 3, 1, "2026-02-01T00:00:00Z").await;

      let entries = load_journal_page(&db, &[1, 2], None, 50).await;

      assert_eq!(entries.iter().map(|e| e.id).collect::<Vec<_>>(), [3, 2, 1]);
    }
  }

  mod load_market {
    use super::*;
    use crate::store::{
      self,
      model::{Alliance, Bloodline, Character, Corporation, Gender, ItemCategory, ItemGroup, ItemType, Race},
      repo::{character::insert_with_org, sde},
    };

    async fn seed_character(db: &Database, id: i64) {
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
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    async fn seed_item_type(db: &Database, id: i64, name: &str) {
      let category = ItemCategory {
        id: 1,
        icon_id: None,
        name: "Material".to_owned(),
        published: true,
      };
      let group = ItemGroup {
        category_id: 1,
        icon_id: None,
        id: 1,
        name: "Mineral".to_owned(),
        published: true,
      };
      let item = ItemType {
        capacity: None,
        description: Some("Test item".to_owned()),
        dogma_attributes: "[]".to_owned(),
        group_id: 1,
        icon_id: None,
        id,
        market_group_id: None,
        name: name.to_owned(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        volume: None,
      };
      sde::insert_item_type_with_hierarchy(db, &item, &group, &category)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_withholds_a_transaction_whose_location_is_unresolved() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 34, "Tritanium").await;
      finance::append_wallet_transaction(&db, &[txn_row(1, 34, 60_003_760, false, 10, 5.0)])
        .await
        .unwrap();

      let entries = load_market_page(&db, &[42], None, 50).await;

      assert!(
        entries.is_empty(),
        "a transaction with an unresolved location is withheld, never shown as Unknown"
      );
    }
  }

  mod map_contract_row {
    use pretty_assertions::assert_eq;

    use super::*;

    fn contract(price: Option<f64>, reward: Option<f64>) -> crate::store::model::CharacterContract {
      crate::store::model::CharacterContract {
        acceptor_id: None,
        acceptor_name: None,
        assignee_id: Some(95_002),
        assignee_name: Some("Assignee Pilot".to_owned()),
        availability: None,
        character_id: 42,
        collateral: Some(5_000.0),
        contract_id: 7,
        date_accepted: None,
        date_completed: None,
        date_expired: None,
        date_issued: "2026-05-30T12:00:00Z".to_owned(),
        days_to_complete: None,
        end_location_id: None,
        for_corporation: false,
        issuer_corporation_id: None,
        issuer_id: 95_001,
        issuer_name: Some("Issuer Pilot".to_owned()),
        price,
        reward,
        start_location_id: None,
        status: "outstanding".to_owned(),
        title: None,
        r#type: "item_exchange".to_owned(),
        volume: Some(1_000.0),
      }
    }

    #[test]
    fn it_reads_a_priced_contract_as_a_sell() {
      let entry = map_contract_row(&contract(Some(200.0), None));

      assert!(!entry.is_buy);
      assert_eq!(entry.value, Some(200.0));
      assert_eq!(entry.character_id, 42);
      assert_eq!(entry.issuer.as_deref(), Some("Issuer Pilot"));
    }

    #[test]
    fn it_reads_a_rewarded_or_unpriced_contract_as_a_buy() {
      let courier = map_contract_row(&contract(None, Some(150.0)));
      let want = map_contract_row(&contract(Some(0.0), None));

      assert!(courier.is_buy);
      assert_eq!(courier.value, Some(150.0));
      assert!(want.is_buy);
    }
  }

  mod map_journal_row {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_carries_the_owning_character_through() {
      let entry = map_journal_row(&journal_row(4, 7, Some(10.0), Some(10.0)));

      assert_eq!(entry.character_id, 7);
      assert_eq!(entry.id, 4);
      assert_eq!(entry.ref_type, "bounty_prizes");
    }

    #[test]
    fn it_marks_a_negative_amount_as_spend() {
      let entry = map_journal_row(&journal_row(2, 42, Some(-400.0), Some(4_600.0)));

      assert!(!entry.is_income());
    }

    #[test]
    fn it_marks_a_positive_amount_as_income() {
      let entry = map_journal_row(&journal_row(1, 42, Some(1_000.0), Some(5_000.0)));

      assert!(entry.is_income());
      assert_eq!(entry.amount, Some(1_000.0));
      assert_eq!(entry.balance, Some(5_000.0));
    }

    #[test]
    fn it_treats_a_null_amount_as_neither_income_nor_spend() {
      let entry = map_journal_row(&journal_row(3, 42, None, None));

      assert!(!entry.is_income());
      assert_eq!(entry.amount, None);
    }
  }

  mod map_txn_row {
    use pretty_assertions::assert_eq;

    use super::*;

    fn type_names() -> HashMap<i64, String> {
      HashMap::from([(34, "Tritanium".to_owned())])
    }

    fn location_names() -> HashMap<i64, String> {
      HashMap::from([(60_003_760, "Jita IV - Moon 4".to_owned())])
    }

    #[test]
    fn it_carries_the_buy_and_sell_side_through() {
      let buy = map_txn_row(
        &txn_row(4, 34, 60_003_760, true, 1, 1.0),
        &type_names(),
        &location_names(),
      )
      .expect("a fully resolved row is kept");
      let sell = map_txn_row(
        &txn_row(5, 34, 60_003_760, false, 1, 1.0),
        &type_names(),
        &location_names(),
      )
      .expect("a fully resolved row is kept");

      assert!(buy.is_buy);
      assert!(!sell.is_buy);
    }

    #[test]
    fn it_derives_total_as_unit_price_times_quantity() {
      let entry = map_txn_row(
        &txn_row(3, 34, 60_003_760, false, 250, 4.0),
        &type_names(),
        &location_names(),
      )
      .expect("a fully resolved row is kept");

      assert_eq!(entry.total, 1_000.0);
    }

    #[test]
    fn it_resolves_the_type_and_location_names() {
      let entry = map_txn_row(
        &txn_row(1, 34, 60_003_760, false, 100, 5.0),
        &type_names(),
        &location_names(),
      )
      .expect("a fully resolved row is kept");

      assert_eq!(entry.item, "Tritanium");
      assert_eq!(entry.location, "Jita IV - Moon 4");
    }

    #[test]
    fn it_withholds_a_row_with_an_unresolved_type_or_location() {
      let unresolved_type = map_txn_row(
        &txn_row(2, 999, 60_003_760, true, 1, 1.0),
        &type_names(),
        &location_names(),
      );
      let unresolved_location = map_txn_row(&txn_row(3, 34, 999_999, true, 1, 1.0), &type_names(), &location_names());

      assert!(unresolved_type.is_none(), "an unresolved item type withholds the row");
      assert!(
        unresolved_location.is_none(),
        "an unresolved location withholds the row"
      );
    }
  }

  mod party_image {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_prefers_a_cached_portrait_over_a_logo() {
      let dir = tempfile::tempdir().unwrap();
      let store = images::Store::new(dir.path().to_path_buf());
      let portrait = store.character_portrait_path(42);
      store.write(&portrait, &[1]).unwrap();

      let resolved = super::party_image(&store, 42);

      assert_eq!(resolved.path, Some(portrait));
      assert!(resolved.stale.is_empty());
    }

    #[test]
    fn it_surfaces_both_candidate_keys_when_neither_image_is_cached() {
      let dir = tempfile::tempdir().unwrap();
      let store = images::Store::new(dir.path().to_path_buf());

      let resolved = super::party_image(&store, 42);

      assert_eq!(resolved.path, None);
      assert!(resolved.stale.contains(&(images::ImageKind::CharacterPortrait, 42)));
      assert!(resolved.stale.contains(&(images::ImageKind::CorporationLogo, 42)));
    }

    #[test]
    fn it_yields_no_path_and_no_stale_keys_for_a_non_positive_id() {
      let dir = tempfile::tempdir().unwrap();
      let store = images::Store::new(dir.path().to_path_buf());

      let resolved = super::party_image(&store, 0);

      assert_eq!(resolved.path, None);
      assert!(resolved.stale.is_empty());
    }
  }

  mod pending_party_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keeps_ids_with_no_cached_image() {
      let dir = tempfile::tempdir().unwrap();
      let store = images::Store::new(dir.path().to_path_buf());

      assert_eq!(super::pending_party_ids(&store, vec![11, 22]), vec![11, 22]);
    }
  }
}
