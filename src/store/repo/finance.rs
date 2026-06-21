use chrono::{NaiveDate, Utc};
use sqlx::{QueryBuilder, Sqlite};

use crate::store::{
  Database, Error,
  model::{
    CharacterContract, CharacterContractBid, CharacterContractItem, CharacterNetWorthSnapshot, CharacterWalletJournal,
    CharacterWalletTransaction, CombinedNetWorthPoint, ContractEscrow, CorporationContract, CorporationContractBid,
    CorporationContractItem, CorporationNetWorthSnapshot, CorporationWalletDivision, CorporationWalletJournal,
    CorporationWalletTransaction, MarketOrder, MarketPrice, TypePriceHistory,
    character_financials::CharacterFinancials,
    character_net_worth_series::{PeriodDelta, Scope, SeriesPoint, Timeframe},
    character_wallet_period_summary::CharacterWalletPeriodSummary,
  },
};

pub const RETENTION_DAYS: i64 = 365;
const SQLITE_MAX_BIND_PARAMS: usize = 999;
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const STATE_OPEN: &str = "open";

fn now_iso() -> String {
  Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub async fn replace_for_character(
  db: &Database,
  character_id: i64,
  contracts: &[CharacterContract],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query("DELETE FROM character_contracts WHERE character_id = ?")
    .bind(character_id)
    .execute(&mut *tx)
    .await?;

  for chunk in contracts.chunks(SQLITE_MAX_BIND_PARAMS / 25) {
    let mut builder = QueryBuilder::<Sqlite>::new(
      "INSERT INTO character_contracts \
        (character_id, contract_id, type, status, issuer_id, issuer_name, assignee_id, assignee_name, acceptor_id, \
        acceptor_name, price, reward, collateral, volume, for_corporation, date_issued, date_expired, date_completed, \
        title, availability, days_to_complete, start_location_id, end_location_id, date_accepted, \
        issuer_corporation_id) ",
    );
    builder.push_values(chunk, |mut row, contract| {
      row
        .push_bind(contract.character_id())
        .push_bind(contract.contract_id())
        .push_bind(contract.r#type())
        .push_bind(contract.status())
        .push_bind(contract.issuer_id())
        .push_bind(contract.issuer_name())
        .push_bind(contract.assignee_id())
        .push_bind(contract.assignee_name())
        .push_bind(contract.acceptor_id())
        .push_bind(contract.acceptor_name())
        .push_bind(contract.price())
        .push_bind(contract.reward())
        .push_bind(contract.collateral())
        .push_bind(contract.volume())
        .push_bind(contract.for_corporation())
        .push_bind(contract.date_issued())
        .push_bind(contract.date_expired())
        .push_bind(contract.date_completed())
        .push_bind(contract.title())
        .push_bind(contract.availability())
        .push_bind(contract.days_to_complete())
        .push_bind(contract.start_location_id())
        .push_bind(contract.end_location_id())
        .push_bind(contract.date_accepted())
        .push_bind(contract.issuer_corporation_id());
    });
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn replace_for_corporation(
  db: &Database,
  corporation_id: i64,
  contracts: &[CorporationContract],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query("DELETE FROM corporation_contracts WHERE corporation_id = ?")
    .bind(corporation_id)
    .execute(&mut *tx)
    .await?;

  for chunk in contracts.chunks(SQLITE_MAX_BIND_PARAMS / 25) {
    let mut builder = QueryBuilder::<Sqlite>::new(
      "INSERT INTO corporation_contracts \
        (corporation_id, contract_id, type, status, issuer_id, issuer_name, assignee_id, assignee_name, acceptor_id, \
        acceptor_name, price, reward, collateral, volume, for_corporation, date_issued, date_expired, date_completed, \
        title, availability, days_to_complete, start_location_id, end_location_id, date_accepted, \
        issuer_corporation_id) ",
    );
    builder.push_values(chunk, |mut row, contract| {
      row
        .push_bind(contract.corporation_id())
        .push_bind(contract.contract_id())
        .push_bind(contract.r#type())
        .push_bind(contract.status())
        .push_bind(contract.issuer_id())
        .push_bind(contract.issuer_name())
        .push_bind(contract.assignee_id())
        .push_bind(contract.assignee_name())
        .push_bind(contract.acceptor_id())
        .push_bind(contract.acceptor_name())
        .push_bind(contract.price())
        .push_bind(contract.reward())
        .push_bind(contract.collateral())
        .push_bind(contract.volume())
        .push_bind(contract.for_corporation())
        .push_bind(contract.date_issued())
        .push_bind(contract.date_expired())
        .push_bind(contract.date_completed())
        .push_bind(contract.title())
        .push_bind(contract.availability())
        .push_bind(contract.days_to_complete())
        .push_bind(contract.start_location_id())
        .push_bind(contract.end_location_id())
        .push_bind(contract.date_accepted())
        .push_bind(contract.issuer_corporation_id());
    });
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn replace_contract_bids_for_character(
  db: &Database,
  character_id: i64,
  contract_id: i64,
  bids: &[CharacterContractBid],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query("DELETE FROM character_contract_bids WHERE character_id = ? AND contract_id = ?")
    .bind(character_id)
    .bind(contract_id)
    .execute(&mut *tx)
    .await?;

  for bid in bids {
    sqlx::query(
      "INSERT INTO character_contract_bids (character_id, contract_id, bid_id, bidder_id, amount, date_bid) \
        VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(bid.character_id())
    .bind(bid.contract_id())
    .bind(bid.bid_id())
    .bind(bid.bidder_id())
    .bind(bid.amount())
    .bind(bid.date_bid())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn replace_contract_bids_for_corporation(
  db: &Database,
  corporation_id: i64,
  contract_id: i64,
  bids: &[CorporationContractBid],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query("DELETE FROM corporation_contract_bids WHERE corporation_id = ? AND contract_id = ?")
    .bind(corporation_id)
    .bind(contract_id)
    .execute(&mut *tx)
    .await?;

  for bid in bids {
    sqlx::query(
      "INSERT INTO corporation_contract_bids (corporation_id, contract_id, bid_id, bidder_id, amount, date_bid) \
        VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(bid.corporation_id())
    .bind(bid.contract_id())
    .bind(bid.bid_id())
    .bind(bid.bidder_id())
    .bind(bid.amount())
    .bind(bid.date_bid())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn replace_contract_items_for_character(
  db: &Database,
  character_id: i64,
  contract_id: i64,
  items: &[CharacterContractItem],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query("DELETE FROM character_contract_items WHERE character_id = ? AND contract_id = ?")
    .bind(character_id)
    .bind(contract_id)
    .execute(&mut *tx)
    .await?;

  for item in items {
    sqlx::query(
      "INSERT INTO character_contract_items \
        (character_id, contract_id, record_id, type_id, quantity, raw_quantity, is_singleton, is_included, value_isk) \
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(item.character_id())
    .bind(item.contract_id())
    .bind(item.record_id())
    .bind(item.type_id())
    .bind(item.quantity())
    .bind(item.raw_quantity())
    .bind(item.is_singleton())
    .bind(item.is_included())
    .bind(item.value_isk())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn replace_contract_items_for_corporation(
  db: &Database,
  corporation_id: i64,
  contract_id: i64,
  items: &[CorporationContractItem],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query("DELETE FROM corporation_contract_items WHERE corporation_id = ? AND contract_id = ?")
    .bind(corporation_id)
    .bind(contract_id)
    .execute(&mut *tx)
    .await?;

  for item in items {
    sqlx::query(
      "INSERT INTO corporation_contract_items \
        (corporation_id, contract_id, record_id, type_id, quantity, raw_quantity, is_singleton, is_included, \
        value_isk) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(item.corporation_id())
    .bind(item.contract_id())
    .bind(item.record_id())
    .bind(item.type_id())
    .bind(item.quantity())
    .bind(item.raw_quantity())
    .bind(item.is_singleton())
    .bind(item.is_included())
    .bind(item.value_isk())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn contracts(db: &Database, character_id: i64) -> Result<Vec<CharacterContract>, Error> {
  let rows = sqlx::query_as::<_, CharacterContract>(
    "SELECT acceptor_id, acceptor_name, assignee_id, assignee_name, availability, character_id, collateral, \
    contract_id, date_accepted, date_completed, date_expired, date_issued, days_to_complete, end_location_id, \
    for_corporation, issuer_corporation_id, issuer_id, issuer_name, price, reward, start_location_id, status, title, \
    type, volume FROM character_contracts WHERE character_id = ? ORDER BY date_issued DESC, contract_id DESC",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn contracts_page(
  db: &Database,
  character_id: i64,
  after: Option<(&str, i64)>,
  limit: i64,
) -> Result<Vec<CharacterContract>, Error> {
  let (after_date, after_id) = match after {
    Some((date, id)) => (Some(date.to_owned()), Some(id)),
    None => (None, None),
  };
  let rows = sqlx::query_as::<_, CharacterContract>(
    "SELECT acceptor_id, acceptor_name, assignee_id, assignee_name, availability, character_id, collateral, \
    contract_id, date_accepted, date_completed, date_expired, date_issued, days_to_complete, end_location_id, \
    for_corporation, issuer_corporation_id, issuer_id, issuer_name, price, reward, start_location_id, status, title, \
    type, volume FROM character_contracts \
    WHERE character_id = ? AND (\
      ? IS NULL \
      OR date_issued < ? \
      OR (date_issued = ? AND contract_id < ?)\
    ) \
    ORDER BY date_issued DESC, contract_id DESC LIMIT ?",
  )
  .bind(character_id)
  .bind(after_date.clone())
  .bind(after_date.clone())
  .bind(after_date)
  .bind(after_id)
  .bind(limit)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn count_contracts_for_character(db: &Database, character_id: i64) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM character_contracts WHERE character_id = ?")
    .bind(character_id)
    .fetch_one(&db.0)
    .await?;
  Ok(count)
}

pub async fn count_contracts_for_corporation(db: &Database, corporation_id: i64) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM corporation_contracts WHERE corporation_id = ?")
    .bind(corporation_id)
    .fetch_one(&db.0)
    .await?;
  Ok(count)
}

pub async fn contract_bids(
  db: &Database,
  character_id: i64,
  contract_id: i64,
) -> Result<Vec<CharacterContractBid>, Error> {
  let rows = sqlx::query_as::<_, CharacterContractBid>(
    "SELECT amount, bid_id, bidder_id, character_id, contract_id, date_bid FROM character_contract_bids \
    WHERE character_id = ? AND contract_id = ? ORDER BY bid_id",
  )
  .bind(character_id)
  .bind(contract_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn contract_items(
  db: &Database,
  character_id: i64,
  contract_id: i64,
) -> Result<Vec<CharacterContractItem>, Error> {
  let rows = sqlx::query_as::<_, CharacterContractItem>(
    "SELECT character_id, contract_id, is_included, is_singleton, quantity, raw_quantity, record_id, type_id, \
    value_isk FROM character_contract_items WHERE character_id = ? AND contract_id = ? ORDER BY record_id",
  )
  .bind(character_id)
  .bind(contract_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn corporation_contract_bids(
  db: &Database,
  corporation_id: i64,
  contract_id: i64,
) -> Result<Vec<CorporationContractBid>, Error> {
  let rows = sqlx::query_as::<_, CorporationContractBid>(
    "SELECT amount, bid_id, bidder_id, contract_id, corporation_id, date_bid FROM corporation_contract_bids \
    WHERE corporation_id = ? AND contract_id = ? ORDER BY bid_id",
  )
  .bind(corporation_id)
  .bind(contract_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn corporation_contract_items(
  db: &Database,
  corporation_id: i64,
  contract_id: i64,
) -> Result<Vec<CorporationContractItem>, Error> {
  let rows = sqlx::query_as::<_, CorporationContractItem>(
    "SELECT contract_id, corporation_id, is_included, is_singleton, quantity, raw_quantity, record_id, type_id, \
    value_isk FROM corporation_contract_items WHERE corporation_id = ? AND contract_id = ? ORDER BY record_id",
  )
  .bind(corporation_id)
  .bind(contract_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn corporation_contracts(db: &Database, corporation_id: i64) -> Result<Vec<CorporationContract>, Error> {
  let rows = sqlx::query_as::<_, CorporationContract>(
    "SELECT acceptor_id, acceptor_name, assignee_id, assignee_name, availability, collateral, contract_id, \
    corporation_id, date_accepted, date_completed, date_expired, date_issued, days_to_complete, end_location_id, \
    for_corporation, issuer_corporation_id, issuer_id, issuer_name, price, reward, start_location_id, status, title, \
    type, volume FROM corporation_contracts WHERE corporation_id = ? ORDER BY date_issued DESC, contract_id DESC",
  )
  .bind(corporation_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn corporation_contracts_page(
  db: &Database,
  corporation_id: i64,
  after: Option<(&str, i64)>,
  limit: i64,
) -> Result<Vec<CorporationContract>, Error> {
  let (after_date, after_id) = match after {
    Some((date, id)) => (Some(date.to_owned()), Some(id)),
    None => (None, None),
  };
  let rows = sqlx::query_as::<_, CorporationContract>(
    "SELECT acceptor_id, acceptor_name, assignee_id, assignee_name, availability, collateral, contract_id, \
    corporation_id, date_accepted, date_completed, date_expired, date_issued, days_to_complete, end_location_id, \
    for_corporation, issuer_corporation_id, issuer_id, issuer_name, price, reward, start_location_id, status, title, \
    type, volume FROM corporation_contracts \
    WHERE corporation_id = ? AND (\
      ? IS NULL \
      OR date_issued < ? \
      OR (date_issued = ? AND contract_id < ?)\
    ) \
    ORDER BY date_issued DESC, contract_id DESC LIMIT ?",
  )
  .bind(corporation_id)
  .bind(after_date.clone())
  .bind(after_date.clone())
  .bind(after_date)
  .bind(after_id)
  .bind(limit)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn count_journal_for_character(db: &Database, character_id: i64) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM character_wallet_journal WHERE character_id = ?")
    .bind(character_id)
    .fetch_one(&db.0)
    .await?;
  Ok(count)
}

pub async fn count_journal_for_corporation(db: &Database, corporation_id: i64, division: i64) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM corporation_wallet_journal WHERE corporation_id = ? AND division = ?",
  )
  .bind(corporation_id)
  .bind(division)
  .fetch_one(&db.0)
  .await?;
  Ok(count)
}

pub async fn count_journal_for_corporation_all_divisions(db: &Database, corporation_id: i64) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM corporation_wallet_journal WHERE corporation_id = ?")
    .bind(corporation_id)
    .fetch_one(&db.0)
    .await?;
  Ok(count)
}

pub async fn count_transactions_for_character(db: &Database, character_id: i64) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM character_wallet_transaction WHERE character_id = ?")
    .bind(character_id)
    .fetch_one(&db.0)
    .await?;
  Ok(count)
}

pub async fn count_transactions_for_corporation(
  db: &Database,
  corporation_id: i64,
  division: i64,
) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM corporation_wallet_transaction WHERE corporation_id = ? AND division = ?",
  )
  .bind(corporation_id)
  .bind(division)
  .fetch_one(&db.0)
  .await?;
  Ok(count)
}

pub async fn count_transactions_for_corporation_all_divisions(
  db: &Database,
  corporation_id: i64,
) -> Result<i64, Error> {
  let count =
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM corporation_wallet_transaction WHERE corporation_id = ?")
      .bind(corporation_id)
      .fetch_one(&db.0)
      .await?;
  Ok(count)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn escrow(db: &Database, character_id: i64) -> Result<Option<ContractEscrow>, Error> {
  let row = sqlx::query_as::<_, ContractEscrow>(
    "SELECT character_id, escrow, escrow_collateral, escrow_price FROM character_contract_escrow \
    WHERE character_id = ?",
  )
  .bind(character_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn corporation_backfill_liquid_from_journal(db: &Database, corporation_id: i64) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO corporation_net_worth_snapshot (corporation_id, date, liquid, net_worth) \
    SELECT corporation_id, day, SUM(balance), SUM(balance) FROM ( \
      SELECT \
        corporation_id, \
        division, \
        substr(date, 1, 10) AS day, \
        balance, \
        ROW_NUMBER() OVER (PARTITION BY division, substr(date, 1, 10) ORDER BY date DESC, id DESC) AS rn \
      FROM corporation_wallet_journal \
      WHERE corporation_id = ? AND balance IS NOT NULL \
    ) WHERE rn = 1 \
    GROUP BY corporation_id, day \
    ON CONFLICT(corporation_id, date) DO UPDATE SET \
      liquid = excluded.liquid, \
      net_worth = excluded.net_worth",
  )
  .bind(corporation_id)
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn for_corporation_since(
  db: &Database,
  corporation_id: i64,
  since: &str,
) -> Result<Vec<CorporationNetWorthSnapshot>, Error> {
  let rows = sqlx::query_as::<_, CorporationNetWorthSnapshot>(
    "SELECT corporation_id, date, id, liquid, net_worth \
    FROM corporation_net_worth_snapshot \
    WHERE corporation_id = ? AND date >= ? ORDER BY date",
  )
  .bind(corporation_id)
  .bind(since)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn record_today(db: &Database, corporation_id: i64, date: &str) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO corporation_net_worth_snapshot (corporation_id, date, liquid, net_worth) \
    SELECT ?, ?, total, total FROM ( \
      SELECT SUM(balance) AS total FROM corporation_wallet_division \
      WHERE corporation_id = ? AND balance IS NOT NULL \
    ) WHERE total IS NOT NULL \
    ON CONFLICT(corporation_id, date) DO UPDATE SET \
      liquid = excluded.liquid, \
      net_worth = excluded.net_worth",
  )
  .bind(corporation_id)
  .bind(date)
  .bind(corporation_id)
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn upsert_divisions(db: &Database, divisions: &[CorporationWalletDivision]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  for division in divisions {
    sqlx::query(
      "INSERT INTO corporation_wallet_division (corporation_id, division, name, balance) \
      VALUES (?, ?, ?, ?) \
      ON CONFLICT(corporation_id, division) DO UPDATE SET \
        name    = COALESCE(excluded.name, corporation_wallet_division.name), \
        balance = COALESCE(excluded.balance, corporation_wallet_division.balance)",
    )
    .bind(division.corporation_id())
    .bind(division.division())
    .bind(division.name())
    .bind(division.balance())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn divisions(db: &Database, corporation_id: i64) -> Result<Vec<CorporationWalletDivision>, Error> {
  let rows = sqlx::query_as::<_, CorporationWalletDivision>(
    "SELECT balance, corporation_id, division, name FROM corporation_wallet_division \
    WHERE corporation_id = ? ORDER BY division ASC",
  )
  .bind(corporation_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn division(
  db: &Database,
  corporation_id: i64,
  division: i64,
) -> Result<Option<CorporationWalletDivision>, Error> {
  let row = sqlx::query_as::<_, CorporationWalletDivision>(
    "SELECT balance, corporation_id, division, name FROM corporation_wallet_division \
    WHERE corporation_id = ? AND division = ?",
  )
  .bind(corporation_id)
  .bind(division)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn append_corporation_wallet_journal(
  db: &Database,
  entries: &[CorporationWalletJournal],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  for chunk in entries.chunks(SQLITE_MAX_BIND_PARAMS / 15) {
    let mut builder = QueryBuilder::<Sqlite>::new(
      "INSERT INTO corporation_wallet_journal \
        (amount, balance, context_id, context_id_type, corporation_id, date, description, division, \
        first_party_id, id, reason, ref_type, second_party_id, tax, tax_receiver_id) ",
    );
    builder.push_values(chunk, |mut row, entry| {
      row
        .push_bind(entry.amount())
        .push_bind(entry.balance())
        .push_bind(entry.context_id())
        .push_bind(entry.context_id_type())
        .push_bind(entry.corporation_id())
        .push_bind(entry.date())
        .push_bind(entry.description())
        .push_bind(entry.division())
        .push_bind(entry.first_party_id())
        .push_bind(entry.id())
        .push_bind(entry.reason())
        .push_bind(entry.ref_type())
        .push_bind(entry.second_party_id())
        .push_bind(entry.tax())
        .push_bind(entry.tax_receiver_id());
    });
    builder.push(" ON CONFLICT(id) DO NOTHING");
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn corporation_wallet_journal(
  db: &Database,
  corporation_id: i64,
  division: i64,
) -> Result<Vec<CorporationWalletJournal>, Error> {
  let rows = sqlx::query_as::<_, CorporationWalletJournal>(
    "SELECT amount, balance, context_id, context_id_type, corporation_id, date, description, division, \
    first_party_id, id, reason, ref_type, second_party_id, tax, tax_receiver_id \
    FROM corporation_wallet_journal WHERE corporation_id = ? AND division = ? ORDER BY id DESC",
  )
  .bind(corporation_id)
  .bind(division)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn append_corporation_wallet_transaction(
  db: &Database,
  transactions: &[CorporationWalletTransaction],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  for chunk in transactions.chunks(SQLITE_MAX_BIND_PARAMS / 11) {
    let mut builder = QueryBuilder::<Sqlite>::new(
      "INSERT INTO corporation_wallet_transaction \
        (client_id, corporation_id, date, division, is_buy, journal_ref_id, location_id, \
        quantity, transaction_id, type_id, unit_price) ",
    );
    builder.push_values(chunk, |mut row, transaction| {
      row
        .push_bind(transaction.client_id())
        .push_bind(transaction.corporation_id())
        .push_bind(transaction.date())
        .push_bind(transaction.division())
        .push_bind(transaction.is_buy())
        .push_bind(transaction.journal_ref_id())
        .push_bind(transaction.location_id())
        .push_bind(transaction.quantity())
        .push_bind(transaction.transaction_id())
        .push_bind(transaction.type_id())
        .push_bind(transaction.unit_price());
    });
    builder.push(" ON CONFLICT(transaction_id) DO NOTHING");
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn corporation_wallet_transactions(
  db: &Database,
  corporation_id: i64,
  division: i64,
) -> Result<Vec<CorporationWalletTransaction>, Error> {
  let rows = sqlx::query_as::<_, CorporationWalletTransaction>(
    "SELECT client_id, corporation_id, date, division, is_buy, journal_ref_id, location_id, \
    quantity, transaction_id, type_id, unit_price FROM corporation_wallet_transaction \
    WHERE corporation_id = ? AND division = ? ORDER BY transaction_id DESC",
  )
  .bind(corporation_id)
  .bind(division)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

/// Pages a corporation's wallet journal across *all* divisions (unlike the per-division
/// `corporation_wallet_transactions` sibling), cursoring on a descending `id`.
pub async fn corporation_wallet_journal_page(
  db: &Database,
  corporation_id: i64,
  after_id: Option<i64>,
  limit: i64,
) -> Result<Vec<CorporationWalletJournal>, Error> {
  let rows = sqlx::query_as::<_, CorporationWalletJournal>(
    "SELECT amount, balance, context_id, context_id_type, corporation_id, date, description, division, \
    first_party_id, id, reason, ref_type, second_party_id, tax, tax_receiver_id \
    FROM corporation_wallet_journal \
    WHERE corporation_id = ? AND (? IS NULL OR id < ?) \
    ORDER BY id DESC LIMIT ?",
  )
  .bind(corporation_id)
  .bind(after_id)
  .bind(after_id)
  .bind(limit)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

/// Pages a corporation's wallet transactions across *all* divisions (unlike the per-division
/// `corporation_wallet_transactions` sibling), cursoring on a descending `transaction_id`.
pub async fn corporation_wallet_transactions_page(
  db: &Database,
  corporation_id: i64,
  after_id: Option<i64>,
  limit: i64,
) -> Result<Vec<CorporationWalletTransaction>, Error> {
  let rows = sqlx::query_as::<_, CorporationWalletTransaction>(
    "SELECT client_id, corporation_id, date, division, is_buy, journal_ref_id, location_id, \
    quantity, transaction_id, type_id, unit_price FROM corporation_wallet_transaction \
    WHERE corporation_id = ? AND (? IS NULL OR transaction_id < ?) \
    ORDER BY transaction_id DESC LIMIT ?",
  )
  .bind(corporation_id)
  .bind(after_id)
  .bind(after_id)
  .bind(limit)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn financials_all(db: &Database) -> Result<Vec<CharacterFinancials>, Error> {
  let rows = sqlx::query_as::<_, CharacterFinancials>(
    "SELECT character_id, liquid, asset_value, escrow, net_worth \
    FROM character_financials ORDER BY character_id",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn financials_get(db: &Database, character_id: i64) -> Result<Option<CharacterFinancials>, Error> {
  let row = sqlx::query_as::<_, CharacterFinancials>(
    "SELECT character_id, liquid, asset_value, escrow, net_worth \
    FROM character_financials WHERE character_id = ?",
  )
  .bind(character_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn for_character(db: &Database, character_id: i64) -> Result<Vec<MarketOrder>, Error> {
  let rows = sqlx::query_as::<_, MarketOrder>(
    "SELECT character_id, duration, escrow, is_buy_order, issued, location_id, order_id, price, \
    range, region_id, state, type_id, volume_remain, volume_total FROM market_orders \
    WHERE character_id = ? ORDER BY order_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn open_escrow(db: &Database, character_id: i64) -> Result<f64, Error> {
  let total: f64 =
    sqlx::query_scalar("SELECT COALESCE(SUM(escrow), 0.0) FROM market_orders WHERE character_id = ? AND state = ?")
      .bind(character_id)
      .bind(STATE_OPEN)
      .fetch_one(&db.0)
      .await?;
  Ok(total)
}

pub async fn replace(db: &Database, character_id: i64, orders: &[MarketOrder]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query("DELETE FROM market_orders WHERE character_id = ?")
    .bind(character_id)
    .execute(&mut *tx)
    .await?;

  for chunk in orders.chunks(SQLITE_MAX_BIND_PARAMS / 14) {
    let mut builder = QueryBuilder::<Sqlite>::new(
      "INSERT INTO market_orders \
        (character_id, duration, escrow, is_buy_order, issued, location_id, order_id, price, \
        range, region_id, state, type_id, volume_remain, volume_total) ",
    );
    builder.push_values(chunk, |mut row, order| {
      row
        .push_bind(order.character_id())
        .push_bind(order.duration())
        .push_bind(order.escrow())
        .push_bind(order.is_buy_order())
        .push_bind(order.issued())
        .push_bind(order.location_id())
        .push_bind(order.order_id())
        .push_bind(order.price())
        .push_bind(order.range())
        .push_bind(order.region_id())
        .push_bind(order.state())
        .push_bind(order.type_id())
        .push_bind(order.volume_remain())
        .push_bind(order.volume_total());
    });
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn market_prices_all(db: &Database) -> Result<Vec<MarketPrice>, Error> {
  let rows = sqlx::query_as::<_, MarketPrice>(
    "SELECT adjusted_price, average_price, source, type_id FROM market_prices ORDER BY type_id",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn market_prices_upsert_many(db: &Database, prices: &[MarketPrice]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  for chunk in prices.chunks(SQLITE_MAX_BIND_PARAMS / 4) {
    let mut builder = QueryBuilder::<Sqlite>::new(
      "INSERT INTO market_prices (adjusted_price, average_price, fetched_at, source, type_id) ",
    );
    builder.push_values(chunk, |mut row, price| {
      row
        .push_bind(price.adjusted_price())
        .push_bind(price.average_price())
        .push_bind(now_iso())
        .push_bind(price.source().clone())
        .push_bind(price.type_id());
    });
    builder.push(
      " ON CONFLICT(type_id) DO UPDATE SET \
        adjusted_price = excluded.adjusted_price, \
        average_price = excluded.average_price, \
        fetched_at = excluded.fetched_at, \
        source = excluded.source",
    );
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(())
}

/// Held types whose canonical type price is unresolved and should be refreshed from zKillboard.
///
/// Keyed off `source`, not the stored price value: a row is in the set when it is absent, when it
/// was previously filled by zKill (so a non-zero zKill `average_price` is still re-fetched and does
/// not go permanently stale), or when ESI priced it to a resolved 0. Blueprint copies are excluded.
pub async fn market_prices_zkill_gap_type_ids(db: &Database) -> Result<Vec<i64>, Error> {
  let rows = sqlx::query_scalar::<_, i64>(
    "WITH held AS ( \
        SELECT DISTINCT type_id FROM character_assets WHERE COALESCE(is_blueprint_copy, 0) = 0 \
        UNION \
        SELECT DISTINCT type_id FROM corporation_assets WHERE COALESCE(is_blueprint_copy, 0) = 0 \
      ) \
      SELECT held.type_id FROM held \
      LEFT JOIN market_prices mp ON mp.type_id = held.type_id \
      WHERE mp.type_id IS NULL \
        OR mp.source = 'zkill' \
        OR (mp.source = 'esi' AND COALESCE(mp.adjusted_price, mp.average_price, 0) = 0) \
      ORDER BY held.type_id",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn backfill_liquid_from_journal(db: &Database, character_id: i64) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO character_net_worth_snapshot (character_id, date, liquid, net_worth) \
    SELECT character_id, day, balance, balance FROM ( \
      SELECT \
        character_id, \
        substr(date, 1, 10) AS day, \
        balance, \
        ROW_NUMBER() OVER (PARTITION BY substr(date, 1, 10) ORDER BY date DESC, id DESC) AS rn \
      FROM character_wallet_journal \
      WHERE character_id = ? AND balance IS NOT NULL \
    ) WHERE rn = 1 \
    ON CONFLICT(character_id, date) DO UPDATE SET \
      liquid = excluded.liquid, \
      net_worth = excluded.liquid + COALESCE(asset_value, 0) + COALESCE(escrow, 0)",
  )
  .bind(character_id)
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn combined_series_since(db: &Database, since: &str) -> Result<Vec<CombinedNetWorthPoint>, Error> {
  let rows = sqlx::query_as::<_, CombinedNetWorthPoint>(
    "SELECT asset_value, date, escrow, liquid, net_worth \
    FROM character_net_worth_snapshot_combined \
    WHERE date >= ? ORDER BY date",
  )
  .bind(since)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn for_character_since(
  db: &Database,
  character_id: i64,
  since: &str,
) -> Result<Vec<CharacterNetWorthSnapshot>, Error> {
  let rows = sqlx::query_as::<_, CharacterNetWorthSnapshot>(
    "SELECT asset_value, character_id, date, escrow, id, liquid, net_worth \
    FROM character_net_worth_snapshot \
    WHERE character_id = ? AND date >= ? ORDER BY date",
  )
  .bind(character_id)
  .bind(since)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn latest(db: &Database, scope: Scope) -> Result<Option<SeriesPoint>, Error> {
  let point = match scope {
    Scope::Character(id) => sqlx::query_as::<_, CharacterNetWorthSnapshot>(
      "SELECT asset_value, character_id, date, escrow, id, liquid, net_worth \
        FROM character_net_worth_snapshot \
        WHERE character_id = ? ORDER BY date DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&db.0)
    .await?
    .map(SeriesPoint::from),
    Scope::Combined => sqlx::query_as::<_, CombinedNetWorthPoint>(
      "SELECT asset_value, date, escrow, liquid, net_worth \
      FROM character_net_worth_snapshot_combined \
      ORDER BY date DESC LIMIT 1",
    )
    .fetch_optional(&db.0)
    .await?
    .map(SeriesPoint::from),
  };
  Ok(point)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub fn period_delta(series: &[SeriesPoint]) -> Option<PeriodDelta> {
  let mut figures = series.iter().filter_map(|point| point.net_worth);
  let start = figures.next()?;
  let end = figures.next_back()?;
  let absolute = end - start;
  let percent = if start > 0.0 { absolute / start * 100.0 } else { 0.0 };
  Some(PeriodDelta {
    absolute,
    end,
    percent,
    start,
  })
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn series_since(
  db: &Database,
  scope: Scope,
  timeframe: Timeframe,
  today: NaiveDate,
) -> Result<Vec<SeriesPoint>, Error> {
  let since = timeframe.since(today);
  let points = match scope {
    Scope::Character(id) => for_character_since(db, id, &since)
      .await?
      .into_iter()
      .map(SeriesPoint::from)
      .collect(),
    Scope::Combined => combined_series_since(db, &since)
      .await?
      .into_iter()
      .map(SeriesPoint::from)
      .collect(),
  };
  Ok(points)
}

// Arguments map directly to the persisted row columns; bundling them into a struct would only move the fields.
#[allow(clippy::too_many_arguments)]
pub async fn upsert(
  db: &Database,
  character_id: i64,
  date: &str,
  liquid: f64,
  asset_value: Option<f64>,
  escrow: Option<f64>,
  net_worth: f64,
) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO character_net_worth_snapshot (character_id, date, liquid, asset_value, escrow, net_worth) \
    VALUES (?, ?, ?, ?, ?, ?) \
    ON CONFLICT(character_id, date) DO UPDATE SET \
      liquid = excluded.liquid, \
      asset_value = excluded.asset_value, \
      escrow = excluded.escrow, \
      net_worth = excluded.net_worth",
  )
  .bind(character_id)
  .bind(date)
  .bind(liquid)
  .bind(asset_value)
  .bind(escrow)
  .bind(net_worth)
  .execute(db.writer())
  .await?;
  Ok(())
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn series(db: &Database, type_id: i64) -> Result<Vec<TypePriceHistory>, Error> {
  let rows = sqlx::query_as::<_, TypePriceHistory>(
    "SELECT close, date, high, low, open, type_id FROM type_price_histories \
    WHERE type_id = ? ORDER BY date",
  )
  .bind(type_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn close_as_of(db: &Database, type_id: i64, date: &str) -> Result<Option<f64>, Error> {
  let close = sqlx::query_scalar::<_, f64>(
    "SELECT close FROM type_price_histories WHERE type_id = ? AND date <= ? ORDER BY date DESC LIMIT 1",
  )
  .bind(type_id)
  .bind(date)
  .fetch_optional(&db.0)
  .await?;
  Ok(close)
}

pub async fn price_history_upsert_many(db: &Database, histories: &[TypePriceHistory]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  for chunk in histories.chunks(SQLITE_MAX_BIND_PARAMS / 6) {
    let mut builder =
      QueryBuilder::<Sqlite>::new("INSERT INTO type_price_histories (close, date, high, low, open, type_id) ");
    builder.push_values(chunk, |mut row, history| {
      row
        .push_bind(history.close())
        .push_bind(history.date())
        .push_bind(history.high())
        .push_bind(history.low())
        .push_bind(history.open())
        .push_bind(history.type_id());
    });
    builder.push(
      " ON CONFLICT(type_id, date) DO UPDATE SET \
        close = excluded.close, \
        high = MAX(type_price_histories.high, excluded.high), \
        low = MIN(type_price_histories.low, excluded.low)",
    );
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn prune_before(db: &Database, cutoff: &str) -> Result<u64, Error> {
  let result = sqlx::query("DELETE FROM type_price_histories WHERE date < ?")
    .bind(cutoff)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

pub async fn append_wallet_journal(db: &Database, entries: &[CharacterWalletJournal]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  for chunk in entries.chunks(SQLITE_MAX_BIND_PARAMS / 14) {
    let mut builder = QueryBuilder::<Sqlite>::new(
      "INSERT INTO character_wallet_journal \
        (amount, balance, character_id, context_id, context_id_type, date, description, \
        first_party_id, id, reason, ref_type, second_party_id, tax, tax_receiver_id) ",
    );
    builder.push_values(chunk, |mut row, entry| {
      row
        .push_bind(entry.amount())
        .push_bind(entry.balance())
        .push_bind(entry.character_id())
        .push_bind(entry.context_id())
        .push_bind(entry.context_id_type())
        .push_bind(entry.date())
        .push_bind(entry.description())
        .push_bind(entry.first_party_id())
        .push_bind(entry.id())
        .push_bind(entry.reason())
        .push_bind(entry.ref_type())
        .push_bind(entry.second_party_id())
        .push_bind(entry.tax())
        .push_bind(entry.tax_receiver_id());
    });
    builder.push(" ON CONFLICT(id) DO NOTHING");
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(())
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn wallet_journal(db: &Database, character_id: i64) -> Result<Vec<CharacterWalletJournal>, Error> {
  let rows = sqlx::query_as::<_, CharacterWalletJournal>(
    "SELECT amount, balance, character_id, context_id, context_id_type, date, description, \
    first_party_id, id, reason, ref_type, second_party_id, tax, tax_receiver_id \
    FROM character_wallet_journal WHERE character_id = ? ORDER BY id DESC",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn wallet_journal_page(
  db: &Database,
  character_id: i64,
  after_id: Option<i64>,
  limit: i64,
) -> Result<Vec<CharacterWalletJournal>, Error> {
  let rows = sqlx::query_as::<_, CharacterWalletJournal>(
    "SELECT amount, balance, character_id, context_id, context_id_type, date, description, \
    first_party_id, id, reason, ref_type, second_party_id, tax, tax_receiver_id \
    FROM character_wallet_journal \
    WHERE character_id = ? AND (? IS NULL OR id < ?) \
    ORDER BY id DESC LIMIT ?",
  )
  .bind(character_id)
  .bind(after_id)
  .bind(after_id)
  .bind(limit)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn append_wallet_transaction(
  db: &Database,
  transactions: &[CharacterWalletTransaction],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  for chunk in transactions.chunks(SQLITE_MAX_BIND_PARAMS / 11) {
    let mut builder = QueryBuilder::<Sqlite>::new(
      "INSERT INTO character_wallet_transaction \
        (character_id, client_id, date, is_buy, is_personal, journal_ref_id, location_id, \
        quantity, transaction_id, type_id, unit_price) ",
    );
    builder.push_values(chunk, |mut row, transaction| {
      row
        .push_bind(transaction.character_id())
        .push_bind(transaction.client_id())
        .push_bind(transaction.date())
        .push_bind(transaction.is_buy())
        .push_bind(transaction.is_personal())
        .push_bind(transaction.journal_ref_id())
        .push_bind(transaction.location_id())
        .push_bind(transaction.quantity())
        .push_bind(transaction.transaction_id())
        .push_bind(transaction.type_id())
        .push_bind(transaction.unit_price());
    });
    builder.push(" ON CONFLICT(transaction_id) DO NOTHING");
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(())
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn wallet_transactions(db: &Database, character_id: i64) -> Result<Vec<CharacterWalletTransaction>, Error> {
  let rows = sqlx::query_as::<_, CharacterWalletTransaction>(
    "SELECT character_id, client_id, date, is_buy, is_personal, journal_ref_id, location_id, \
    quantity, transaction_id, type_id, unit_price FROM character_wallet_transaction \
    WHERE character_id = ? ORDER BY transaction_id DESC",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn wallet_transactions_page(
  db: &Database,
  character_id: i64,
  after_id: Option<i64>,
  limit: i64,
) -> Result<Vec<CharacterWalletTransaction>, Error> {
  let rows = sqlx::query_as::<_, CharacterWalletTransaction>(
    "SELECT character_id, client_id, date, is_buy, is_personal, journal_ref_id, location_id, \
    quantity, transaction_id, type_id, unit_price FROM character_wallet_transaction \
    WHERE character_id = ? AND (? IS NULL OR transaction_id < ?) \
    ORDER BY transaction_id DESC LIMIT ?",
  )
  .bind(character_id)
  .bind(after_id)
  .bind(after_id)
  .bind(limit)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn wallet_period_summaries_all(db: &Database) -> Result<Vec<CharacterWalletPeriodSummary>, Error> {
  let rows = sqlx::query_as::<_, CharacterWalletPeriodSummary>(
    "SELECT character_id, period, income, spend, net \
    FROM character_wallet_period_summary ORDER BY character_id, period",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn wallet_period_summaries_get(
  db: &Database,
  character_id: i64,
) -> Result<Vec<CharacterWalletPeriodSummary>, Error> {
  let rows = sqlx::query_as::<_, CharacterWalletPeriodSummary>(
    "SELECT character_id, period, income, spend, net \
    FROM character_wallet_period_summary WHERE character_id = ? ORDER BY period",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

#[cfg(test)]
mod contract_tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
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
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  async fn seed_contract(
    db: &Database,
    character_id: i64,
    contract_id: i64,
    status: &str,
    date_issued: &str,
    price: Option<f64>,
    collateral: Option<f64>,
  ) {
    sqlx::query(
      "INSERT INTO character_contracts \
        (character_id, contract_id, type, status, issuer_id, issuer_name, assignee_id, assignee_name, price, \
        reward, collateral, volume, for_corporation, date_issued) \
      VALUES (?, ?, 'courier', ?, ?, 'Issuer Pilot', ?, 'Assignee Pilot', ?, NULL, ?, 1000.0, 0, ?)",
    )
    .bind(character_id)
    .bind(contract_id)
    .bind(status)
    .bind(95_001_i64)
    .bind(95_002_i64)
    .bind(price)
    .bind(collateral)
    .bind(date_issued)
    .execute(db.writer())
    .await
    .unwrap();
  }

  mod contracts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_an_empty_vec_when_no_contracts_exist() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      assert_eq!(super::contracts(&db, 42).await.unwrap(), Vec::new());
    }

    #[tokio::test]
    async fn it_returns_contracts_newest_first_with_resolved_names() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_contract(&db, 42, 1, "finished", "2026-01-01T00:00:00Z", Some(100.0), None).await;
      seed_contract(
        &db,
        42,
        2,
        "outstanding",
        "2026-03-01T00:00:00Z",
        Some(200.0),
        Some(5000.0),
      )
      .await;

      let result = super::contracts(&db, 42).await.unwrap();

      assert_eq!(result.iter().map(|c| c.contract_id()).collect::<Vec<_>>(), [2, 1]);
      let newest = &result[0];
      assert_eq!(newest.status(), "outstanding");
      assert_eq!(newest.issuer_name().as_deref(), Some("Issuer Pilot"));
      assert_eq!(newest.assignee_name().as_deref(), Some("Assignee Pilot"));
      assert_eq!(newest.collateral(), Some(5000.0));
    }
  }

  mod contracts_page {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_breaks_ties_on_contract_id_within_the_same_issue_date() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_contract(&db, 42, 10, "finished", "2026-03-01T00:00:00Z", Some(1.0), None).await;
      seed_contract(&db, 42, 20, "finished", "2026-03-01T00:00:00Z", Some(1.0), None).await;

      let page = super::contracts_page(&db, 42, Some(("2026-03-01T00:00:00Z", 20)), 5)
        .await
        .unwrap();

      assert_eq!(page.iter().map(|c| c.contract_id()).collect::<Vec<_>>(), [10]);
    }

    #[tokio::test]
    async fn it_returns_the_first_page_newest_first_when_no_cursor_is_given() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_contract(&db, 42, 1, "finished", "2026-01-01T00:00:00Z", Some(1.0), None).await;
      seed_contract(&db, 42, 2, "finished", "2026-02-01T00:00:00Z", Some(1.0), None).await;
      seed_contract(&db, 42, 3, "finished", "2026-03-01T00:00:00Z", Some(1.0), None).await;

      let page = super::contracts_page(&db, 42, None, 2).await.unwrap();

      assert_eq!(page.iter().map(|c| c.contract_id()).collect::<Vec<_>>(), [3, 2]);
    }

    #[tokio::test]
    async fn it_seeks_past_the_cursor_for_the_next_page() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_contract(&db, 42, 1, "finished", "2026-01-01T00:00:00Z", Some(1.0), None).await;
      seed_contract(&db, 42, 2, "finished", "2026-02-01T00:00:00Z", Some(1.0), None).await;
      seed_contract(&db, 42, 3, "finished", "2026-03-01T00:00:00Z", Some(1.0), None).await;

      let page = super::contracts_page(&db, 42, Some(("2026-03-01T00:00:00Z", 3)), 2)
        .await
        .unwrap();

      assert_eq!(page.iter().map(|c| c.contract_id()).collect::<Vec<_>>(), [2, 1]);
    }
  }

  mod count_contracts_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_only_the_given_characters_contracts() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      seed_contract(&db, 42, 1, "finished", "2026-01-01T00:00:00Z", Some(100.0), None).await;
      seed_contract(&db, 42, 2, "outstanding", "2026-02-01T00:00:00Z", Some(200.0), None).await;
      seed_contract(&db, 43, 3, "finished", "2026-03-01T00:00:00Z", Some(300.0), None).await;

      assert_eq!(super::count_contracts_for_character(&db, 42).await.unwrap(), 2);
      assert_eq!(super::count_contracts_for_character(&db, 43).await.unwrap(), 1);
      assert_eq!(super::count_contracts_for_character(&db, 99).await.unwrap(), 0);
    }
  }

  mod escrow {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_without_outstanding_contracts() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_contract(
        &db,
        42,
        1,
        "finished",
        "2026-01-01T00:00:00Z",
        Some(100.0),
        Some(9999.0),
      )
      .await;

      assert_eq!(super::escrow(&db, 42).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_sums_outstanding_collateral() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_contract(
        &db,
        42,
        1,
        "outstanding",
        "2026-01-01T00:00:00Z",
        Some(50.0),
        Some(1000.0),
      )
      .await;
      seed_contract(
        &db,
        42,
        2,
        "outstanding",
        "2026-02-01T00:00:00Z",
        Some(75.0),
        Some(2500.0),
      )
      .await;
      seed_contract(&db, 42, 3, "finished", "2026-03-01T00:00:00Z", Some(10.0), Some(9999.0)).await;

      let result = super::escrow(&db, 42).await.unwrap().unwrap();

      assert!((result.escrow() - 3500.0).abs() < f64::EPSILON);
      assert!((result.escrow_collateral() - 3500.0).abs() < f64::EPSILON);
      assert!((result.escrow_price() - 125.0).abs() < f64::EPSILON);
    }
  }

  mod replace_for_character {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::CharacterContract;

    fn contract(character_id: i64, contract_id: i64, status: &str, collateral: Option<f64>) -> CharacterContract {
      CharacterContract {
        acceptor_id: None,
        acceptor_name: None,
        assignee_id: Some(95_002),
        assignee_name: Some("Assignee Pilot".to_owned()),
        availability: Some("personal".to_owned()),
        character_id,
        collateral,
        contract_id,
        date_accepted: Some("2026-03-02T00:00:00Z".to_owned()),
        date_completed: None,
        date_expired: None,
        date_issued: "2026-03-01T00:00:00Z".to_owned(),
        days_to_complete: Some(7),
        end_location_id: Some(60_003_761),
        for_corporation: false,
        issuer_corporation_id: Some(98_000_001),
        issuer_id: 95_001,
        issuer_name: Some("Issuer Pilot".to_owned()),
        price: Some(200.0),
        reward: None,
        start_location_id: Some(60_003_760),
        status: status.to_owned(),
        title: Some("Haul to Jita".to_owned()),
        r#type: "courier".to_owned(),
        volume: Some(1000.0),
      }
    }

    #[tokio::test]
    async fn it_clears_existing_rows_when_given_an_empty_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_contract(&db, 42, 1, "outstanding", "2026-01-01T00:00:00Z", Some(1.0), Some(1.0)).await;

      super::replace_for_character(&db, 42, &[]).await.unwrap();

      assert!(super::contracts(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_replaces_contracts_atomically_and_feeds_the_escrow_view() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_contract(&db, 42, 999, "finished", "2025-01-01T00:00:00Z", Some(1.0), Some(1.0)).await;

      super::replace_for_character(
        &db,
        42,
        &[
          contract(42, 1, "outstanding", Some(5000.0)),
          contract(42, 2, "finished", None),
        ],
      )
      .await
      .unwrap();

      let result = super::contracts(&db, 42).await.unwrap();
      assert_eq!(result.iter().map(|c| c.contract_id()).collect::<Vec<_>>(), [2, 1]);
      assert!(result.iter().all(|c| c.contract_id() != 999));
      let escrow = super::escrow(&db, 42).await.unwrap().unwrap();
      assert!((escrow.escrow() - 5000.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn it_round_trips_all_new_header_fields() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::replace_for_character(&db, 42, &[contract(42, 7, "outstanding", Some(5000.0))])
        .await
        .unwrap();

      let result = super::contracts(&db, 42).await.unwrap();
      assert_eq!(result, vec![contract(42, 7, "outstanding", Some(5000.0))]);
    }
  }
}

#[cfg(test)]
mod corporation_net_worth_tests {
  use super::*;
  use crate::store::{self, model::Corporation, repo::org};

  const CORP: i64 = 90_000_001;

  async fn seed_corp(db: &Database, id: i64) {
    let mut corporation = Corporation::new(id, "Test Corporation", "TSTC");
    corporation.set_ceo_id(12_345_678);
    corporation.set_creator_id(12_345_678);
    corporation.set_member_count(100);
    corporation.set_tax_rate(0.1);
    org::upsert_corporation(db, &corporation).await.unwrap();
  }

  async fn insert_journal(
    db: &Database,
    id: i64,
    corporation_id: i64,
    division: i64,
    date: &str,
    balance: Option<f64>,
  ) {
    sqlx::query(
      "INSERT INTO corporation_wallet_journal (id, corporation_id, division, date, description, ref_type, amount, balance) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(corporation_id)
    .bind(division)
    .bind(date)
    .bind("Test")
    .bind("test")
    .bind(1.0)
    .bind(balance)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn insert_division(db: &Database, corporation_id: i64, division: i64, balance: f64) {
    sqlx::query(
      "INSERT INTO corporation_wallet_division (corporation_id, division, name, balance) VALUES (?, ?, ?, ?)",
    )
    .bind(corporation_id)
    .bind(division)
    .bind("Master")
    .bind(balance)
    .execute(db.writer())
    .await
    .unwrap();
  }

  mod backfill_liquid_from_journal {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_breaks_same_timestamp_ties_with_the_highest_id() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db, CORP).await;
      insert_journal(&db, 1, CORP, 1, "2026-06-01T12:00:00Z", Some(100.0)).await;
      insert_journal(&db, 2, CORP, 1, "2026-06-01T12:00:00Z", Some(175.0)).await;

      corporation_backfill_liquid_from_journal(&db, CORP).await.unwrap();

      let rows = for_corporation_since(&db, CORP, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].liquid(), 175.0);
    }

    #[tokio::test]
    async fn it_excludes_other_corporations() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db, CORP).await;
      seed_corp(&db, 90_000_002).await;
      insert_journal(&db, 1, CORP, 1, "2026-06-01T03:00:00Z", Some(100.0)).await;
      insert_journal(&db, 2, 90_000_002, 1, "2026-06-01T03:00:00Z", Some(900.0)).await;

      corporation_backfill_liquid_from_journal(&db, CORP).await.unwrap();

      let rows = for_corporation_since(&db, CORP, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].liquid(), 100.0);
    }

    #[tokio::test]
    async fn it_ignores_entries_without_a_balance() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db, CORP).await;
      insert_journal(&db, 1, CORP, 1, "2026-06-01T03:00:00Z", Some(100.0)).await;
      insert_journal(&db, 2, CORP, 1, "2026-06-01T21:00:00Z", None).await;

      corporation_backfill_liquid_from_journal(&db, CORP).await.unwrap();

      let rows = for_corporation_since(&db, CORP, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].liquid(), 100.0);
    }

    #[tokio::test]
    async fn it_is_idempotent_across_re_runs() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db, CORP).await;
      insert_journal(&db, 1, CORP, 1, "2026-06-01T03:00:00Z", Some(100.0)).await;

      corporation_backfill_liquid_from_journal(&db, CORP).await.unwrap();
      corporation_backfill_liquid_from_journal(&db, CORP).await.unwrap();

      let rows = for_corporation_since(&db, CORP, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].liquid(), 100.0);
    }

    #[tokio::test]
    async fn it_sums_each_divisions_last_balance_per_utc_day() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db, CORP).await;
      insert_journal(&db, 1, CORP, 1, "2026-06-01T03:00:00Z", Some(100.0)).await;
      insert_journal(&db, 2, CORP, 1, "2026-06-01T21:00:00Z", Some(150.0)).await;
      insert_journal(&db, 3, CORP, 2, "2026-06-01T10:00:00Z", Some(40.0)).await;
      insert_journal(&db, 4, CORP, 1, "2026-06-02T09:00:00Z", Some(220.0)).await;

      corporation_backfill_liquid_from_journal(&db, CORP).await.unwrap();

      let rows = for_corporation_since(&db, CORP, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 2);
      assert_eq!(rows[0].date(), "2026-06-01");
      assert_eq!(rows[0].liquid(), 190.0);
      assert_eq!(rows[0].net_worth(), 190.0);
      assert_eq!(rows[1].date(), "2026-06-02");
      assert_eq!(rows[1].liquid(), 220.0);
    }
  }

  mod record_today {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_overwrites_an_existing_today_row_from_backfill() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db, CORP).await;
      insert_journal(&db, 1, CORP, 1, "2026-06-05T03:00:00Z", Some(100.0)).await;
      insert_division(&db, CORP, 1, 999.0).await;

      corporation_backfill_liquid_from_journal(&db, CORP).await.unwrap();
      record_today(&db, CORP, "2026-06-05").await.unwrap();

      let rows = for_corporation_since(&db, CORP, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].liquid(), 999.0);
    }

    #[tokio::test]
    async fn it_sums_current_division_balances_into_todays_row() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db, CORP).await;
      insert_division(&db, CORP, 1, 1_000.0).await;
      insert_division(&db, CORP, 2, 250.0).await;

      record_today(&db, CORP, "2026-06-05").await.unwrap();

      let rows = for_corporation_since(&db, CORP, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].date(), "2026-06-05");
      assert_eq!(rows[0].liquid(), 1_250.0);
      assert_eq!(rows[0].net_worth(), 1_250.0);
    }

    #[tokio::test]
    async fn it_writes_nothing_when_no_division_balances_exist() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db, CORP).await;

      record_today(&db, CORP, "2026-06-05").await.unwrap();

      let rows = for_corporation_since(&db, CORP, "2026-01-01").await.unwrap();

      assert!(rows.is_empty());
    }
  }
}

#[cfg(test)]
mod corporation_wallet_tests {
  use super::*;
  use crate::{
    store,
    store::{model::Corporation, repo::org},
  };

  const CORP: i64 = 90_000_001;

  async fn seed_corp(db: &Database) {
    let mut corporation = Corporation::new(CORP, "Test Corporation", "TSTC");
    corporation.set_ceo_id(12_345_678);
    corporation.set_creator_id(12_345_678);
    corporation.set_member_count(100);
    corporation.set_tax_rate(0.1);
    org::upsert_corporation(db, &corporation).await.unwrap();
  }

  fn division_with_name(division: i64, name: &str) -> CorporationWalletDivision {
    CorporationWalletDivision {
      balance: None,
      corporation_id: CORP,
      division,
      name: Some(name.to_owned()),
    }
  }

  fn division_with_balance(division: i64, balance: f64) -> CorporationWalletDivision {
    CorporationWalletDivision {
      balance: Some(balance),
      corporation_id: CORP,
      division,
      name: None,
    }
  }

  fn journal_entry(id: i64, division: i64) -> CorporationWalletJournal {
    CorporationWalletJournal {
      amount: Some(-1_000.5),
      balance: Some(50_000.25),
      context_id: None,
      context_id_type: None,
      corporation_id: CORP,
      date: "2026-05-30T12:00:00Z".to_owned(),
      description: "Market escrow".to_owned(),
      division,
      first_party_id: Some(90_000_001),
      id,
      reason: None,
      ref_type: "market_escrow".to_owned(),
      second_party_id: None,
      tax: None,
      tax_receiver_id: None,
    }
  }

  fn transaction(transaction_id: i64, division: i64) -> CorporationWalletTransaction {
    CorporationWalletTransaction {
      client_id: 1_000_035,
      corporation_id: CORP,
      date: "2026-05-30T12:00:00Z".to_owned(),
      division,
      is_buy: true,
      journal_ref_id: 123_456_789,
      location_id: 60_003_760,
      quantity: 10,
      transaction_id,
      type_id: 34,
      unit_price: 5.5,
    }
  }

  mod count_journal_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_only_the_given_division() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      append_corporation_wallet_journal(&db, &[journal_entry(1, 1), journal_entry(2, 1), journal_entry(3, 2)])
        .await
        .unwrap();

      assert_eq!(count_journal_for_corporation(&db, CORP, 1).await.unwrap(), 2);
      assert_eq!(count_journal_for_corporation(&db, CORP, 2).await.unwrap(), 1);
      assert_eq!(count_journal_for_corporation(&db, CORP, 3).await.unwrap(), 0);
    }
  }

  mod count_transactions_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_only_the_given_division() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      append_corporation_wallet_transaction(&db, &[transaction(1, 1), transaction(2, 1), transaction(3, 2)])
        .await
        .unwrap();

      assert_eq!(count_transactions_for_corporation(&db, CORP, 1).await.unwrap(), 2);
      assert_eq!(count_transactions_for_corporation(&db, CORP, 2).await.unwrap(), 1);
    }
  }

  mod upsert_divisions {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_merges_name_and_balance_writes_without_clobbering() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;

      upsert_divisions(&db, &[division_with_name(1, "Master Wallet")])
        .await
        .unwrap();
      upsert_divisions(&db, &[division_with_balance(1, 1_234.5)])
        .await
        .unwrap();

      let row = division(&db, CORP, 1).await.unwrap().unwrap();
      assert_eq!(row.name(), &Some("Master Wallet".to_owned()));
      assert_eq!(row.balance(), Some(1_234.5));
    }

    #[tokio::test]
    async fn it_returns_none_for_an_unsynced_division() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;

      assert_eq!(division(&db, CORP, 7).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_round_trips_divisions_ordered_by_division() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;

      upsert_divisions(&db, &[division_with_balance(2, 200.0), division_with_balance(1, 100.0)])
        .await
        .unwrap();

      let rows = divisions(&db, CORP).await.unwrap();
      assert_eq!(rows.len(), 2);
      assert_eq!(rows[0].division(), 1);
      assert_eq!(rows[0].balance(), Some(100.0));
      assert_eq!(rows[1].division(), 2);
      assert_eq!(rows[1].balance(), Some(200.0));
    }
  }

  mod wallet_journal {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_appends_and_reads_back_per_division_descending() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;

      append_corporation_wallet_journal(&db, &[journal_entry(1, 1), journal_entry(2, 1), journal_entry(3, 2)])
        .await
        .unwrap();

      let division_one = corporation_wallet_journal(&db, CORP, 1).await.unwrap();
      assert_eq!(division_one.iter().map(|e| e.id()).collect::<Vec<_>>(), vec![2, 1]);

      let division_two = corporation_wallet_journal(&db, CORP, 2).await.unwrap();
      assert_eq!(division_two.iter().map(|e| e.id()).collect::<Vec<_>>(), vec![3]);
    }

    #[tokio::test]
    async fn it_ignores_duplicate_ids_on_re_append() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;

      append_corporation_wallet_journal(&db, &[journal_entry(1, 1)])
        .await
        .unwrap();
      append_corporation_wallet_journal(&db, &[journal_entry(1, 1)])
        .await
        .unwrap();

      assert_eq!(corporation_wallet_journal(&db, CORP, 1).await.unwrap().len(), 1);
    }
  }

  mod wallet_transactions {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_appends_and_reads_back_per_division_descending() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;

      append_corporation_wallet_transaction(&db, &[transaction(1, 1), transaction(2, 1), transaction(3, 2)])
        .await
        .unwrap();

      let division_one = corporation_wallet_transactions(&db, CORP, 1).await.unwrap();
      assert_eq!(
        division_one.iter().map(|t| t.transaction_id()).collect::<Vec<_>>(),
        vec![2, 1]
      );

      let division_two = corporation_wallet_transactions(&db, CORP, 2).await.unwrap();
      assert_eq!(
        division_two.iter().map(|t| t.transaction_id()).collect::<Vec<_>>(),
        vec![3]
      );
    }

    #[tokio::test]
    async fn it_ignores_duplicate_transaction_ids_on_re_append() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;

      append_corporation_wallet_transaction(&db, &[transaction(1, 1)])
        .await
        .unwrap();
      append_corporation_wallet_transaction(&db, &[transaction(1, 1)])
        .await
        .unwrap();

      assert_eq!(corporation_wallet_transactions(&db, CORP, 1).await.unwrap().len(), 1);
    }
  }
}

#[cfg(test)]
mod financials_tests {
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

  async fn insert_journal(db: &Database, id: i64, character_id: i64, amount: f64, balance: f64) {
    sqlx::query("INSERT INTO character_wallet_journal (id, character_id, date, description, ref_type, amount, balance) VALUES (?, ?, ?, ?, ?, ?, ?)")
      .bind(id)
      .bind(character_id)
      .bind("2026-01-01")
      .bind("Test")
      .bind("test")
      .bind(amount)
      .bind(balance)
      .execute(db.writer())
      .await
      .unwrap();
  }

  async fn insert_asset(db: &Database, item_id: i64, character_id: i64, type_id: i64, quantity: i64) {
    sqlx::query(
      "INSERT INTO character_assets (item_id, character_id, type_id, location_id, location_type, location_flag, quantity) \
      VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(item_id)
    .bind(character_id)
    .bind(type_id)
    .bind(60_003_760)
    .bind("station")
    .bind("Hangar")
    .bind(quantity)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn insert_price(db: &Database, type_id: i64, adjusted: Option<f64>, average: Option<f64>) {
    sqlx::query("INSERT INTO market_prices (type_id, adjusted_price, average_price) VALUES (?, ?, ?)")
      .bind(type_id)
      .bind(adjusted)
      .bind(average)
      .execute(db.writer())
      .await
      .unwrap();
  }

  async fn insert_order(db: &Database, order_id: i64, character_id: i64, escrow: f64, state: &str) {
    sqlx::query(
      "INSERT INTO market_orders \
      (order_id, character_id, type_id, region_id, location_id, is_buy_order, price, volume_remain, volume_total, escrow, range, duration, issued, state) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(order_id)
    .bind(character_id)
    .bind(34)
    .bind(10_000_002)
    .bind(60_003_760)
    .bind(1)
    .bind(5.0)
    .bind(10)
    .bind(10)
    .bind(escrow)
    .bind("station")
    .bind(90)
    .bind("2026-01-01T00:00:00Z")
    .bind(state)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn insert_contract(db: &Database, contract_id: i64, character_id: i64, collateral: f64, status: &str) {
    sqlx::query(
      "INSERT INTO character_contracts \
      (character_id, contract_id, type, status, issuer_id, collateral, date_issued) \
      VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(character_id)
    .bind(contract_id)
    .bind("courier")
    .bind(status)
    .bind(character_id)
    .bind(collateral)
    .bind("2026-01-01T00:00:00Z")
    .execute(db.writer())
    .await
    .unwrap();
  }

  mod all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_characters_exist() {
      let db = store::open_test().await.unwrap();
      assert!(financials_all(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_returns_one_row_per_character_ordered_by_id() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 2).await;
      seed_character(&db, 1).await;
      insert_journal(&db, 1, 1, 100.0, 100.0).await;

      let rows = financials_all(&db).await.unwrap();

      assert_eq!(rows.iter().map(|r| r.character_id).collect::<Vec<_>>(), [1, 2]);
      assert_eq!(rows[0].liquid, Some(100.0));
      assert_eq!(rows[1].liquid, None);
    }
  }

  mod get {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_order_escrow_alone_when_no_contracts_exist() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_order(&db, 1, 1, 80.0, "open").await;

      let row = financials_get(&db, 1).await.unwrap().unwrap();

      assert_eq!(row.escrow, Some(80.0));
    }

    #[tokio::test]
    async fn it_excludes_other_characters_figures() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_character(&db, 2).await;
      insert_journal(&db, 1, 2, 500.0, 500.0).await;

      let row = financials_get(&db, 1).await.unwrap().unwrap();

      assert_eq!(row.liquid, None);
      assert_eq!(row.net_worth, None);
    }

    #[tokio::test]
    async fn it_prices_assets_with_the_adjusted_price_fallback_to_average() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_asset(&db, 1, 1, 100, 3).await;
      insert_asset(&db, 2, 1, 200, 2).await;
      insert_price(&db, 100, Some(10.0), Some(8.0)).await;
      insert_price(&db, 200, None, Some(5.0)).await;

      let row = financials_get(&db, 1).await.unwrap().unwrap();

      assert_eq!(row.asset_value, Some(40.0));
      assert_eq!(row.net_worth, Some(40.0));
    }

    #[tokio::test]
    async fn it_returns_all_nulls_for_a_fully_unsynced_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;

      let row = financials_get(&db, 1).await.unwrap().unwrap();

      assert_eq!(row.character_id, 1);
      assert_eq!(row.liquid, None);
      assert_eq!(row.asset_value, None);
      assert_eq!(row.escrow, None);
      assert_eq!(row.net_worth, None);
    }

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_character() {
      let db = store::open_test().await.unwrap();
      assert!(financials_get(&db, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_reuses_the_character_state_wallet_balance_for_liquid() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_journal(&db, 1, 1, 100.0, 100.0).await;
      insert_journal(&db, 2, 1, 50.0, 150.0).await;

      let row = financials_get(&db, 1).await.unwrap().unwrap();

      assert_eq!(row.liquid, Some(150.0));
      assert_eq!(row.asset_value, None);
      assert_eq!(row.escrow, None);
      assert_eq!(row.net_worth, Some(150.0));
    }

    #[tokio::test]
    async fn it_sums_all_four_terms_into_net_worth() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_journal(&db, 1, 1, 100.0, 100.0).await;
      insert_asset(&db, 1, 1, 100, 4).await;
      insert_price(&db, 100, Some(10.0), None).await;
      insert_order(&db, 1, 1, 30.0, "open").await;
      insert_contract(&db, 1, 1, 20.0, "outstanding").await;

      let row = financials_get(&db, 1).await.unwrap().unwrap();

      assert_eq!(row.liquid, Some(100.0));
      assert_eq!(row.asset_value, Some(40.0));
      assert_eq!(row.escrow, Some(50.0));
      assert_eq!(row.net_worth, Some(190.0));
    }

    #[tokio::test]
    async fn it_sums_open_order_escrow_and_outstanding_contract_collateral() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_order(&db, 1, 1, 80.0, "open").await;
      insert_order(&db, 2, 1, 999.0, "cancelled").await;
      insert_contract(&db, 1, 1, 20.0, "outstanding").await;
      insert_contract(&db, 2, 1, 999.0, "finished").await;

      let row = financials_get(&db, 1).await.unwrap().unwrap();

      assert_eq!(row.escrow, Some(100.0));
      assert_eq!(row.net_worth, Some(100.0));
    }

    #[tokio::test]
    async fn it_treats_an_unpriced_asset_type_as_zero_without_nulling_the_sum() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_asset(&db, 1, 1, 100, 3).await;
      insert_asset(&db, 2, 1, 999, 5).await;
      insert_price(&db, 100, Some(10.0), None).await;

      let row = financials_get(&db, 1).await.unwrap().unwrap();

      assert_eq!(row.asset_value, Some(30.0));
    }
  }

  mod integration {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::character_net_worth_series::{Scope, Timeframe},
      repo::finance,
    };

    fn date(iso: &str) -> chrono::NaiveDate {
      chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d").unwrap()
    }

    async fn snapshot_financials(db: &Database, character_id: i64, date: &str) -> bool {
      let fin = financials_get(db, character_id).await.unwrap().unwrap();
      let Some(net_worth) = fin.net_worth else {
        return false;
      };
      let liquid = fin.liquid.unwrap_or(0.0);
      finance::upsert(db, character_id, date, liquid, fin.asset_value, fin.escrow, net_worth)
        .await
        .unwrap();
      true
    }

    #[tokio::test]
    async fn it_composes_snapshots_across_characters_into_the_combined_series() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_character(&db, 2).await;

      insert_journal(&db, 1, 1, 100.0, 100.0).await;
      insert_asset(&db, 1, 1, 100, 5).await;
      insert_price(&db, 100, Some(10.0), None).await;
      insert_order(&db, 1, 2, 40.0, "open").await;

      assert!(snapshot_financials(&db, 1, "2026-06-03").await);
      assert!(snapshot_financials(&db, 2, "2026-06-03").await);

      let combined = finance::series_since(&db, Scope::Combined, Timeframe::Week, date("2026-06-03"))
        .await
        .unwrap();
      assert_eq!(combined.len(), 1);
      assert_eq!(combined[0].date, "2026-06-03");
      assert_eq!(combined[0].liquid, Some(100.0));
      assert_eq!(combined[0].asset_value, Some(50.0));
      assert_eq!(combined[0].escrow, Some(40.0));
      assert_eq!(combined[0].net_worth, Some(190.0));
    }

    #[tokio::test]
    async fn it_counts_escrow_orders_only_until_contracts_are_present_then_adds_collateral() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_order(&db, 1, 1, 80.0, "open").await;

      let before = financials_get(&db, 1).await.unwrap().unwrap();
      assert_eq!(before.escrow, Some(80.0));
      assert_eq!(before.net_worth, Some(80.0));

      insert_contract(&db, 1, 1, 25.0, "outstanding").await;
      let after = financials_get(&db, 1).await.unwrap().unwrap();
      assert_eq!(after.escrow, Some(105.0));
      assert_eq!(after.net_worth, Some(105.0));
    }

    #[tokio::test]
    async fn it_degrades_a_null_priced_asset_to_zero_value_and_carries_it_through_the_whole_chain() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_journal(&db, 1, 1, 1_000.0, 1_000.0).await;
      insert_asset(&db, 1, 1, 100, 2).await;
      insert_asset(&db, 2, 1, 777, 9).await;
      insert_price(&db, 100, Some(10.0), None).await;

      let fin = financials_get(&db, 1).await.unwrap().unwrap();
      assert_eq!(fin.asset_value, Some(20.0));
      assert_eq!(fin.net_worth, Some(1_020.0));

      assert!(snapshot_financials(&db, 1, "2026-06-03").await);
      let latest = finance::latest(&db, Scope::Character(1)).await.unwrap().unwrap();
      assert_eq!(latest.asset_value, Some(20.0));
      assert_eq!(latest.net_worth, Some(1_020.0));
    }

    #[tokio::test]
    async fn it_derives_the_period_summary_from_the_same_money_the_journal_feeds_the_financials_view() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_journal(&db, 1, 1, 200.0, 200.0).await;
      insert_journal(&db, 2, 1, -50.0, 150.0).await;

      let fin = financials_get(&db, 1).await.unwrap().unwrap();
      assert_eq!(fin.liquid, Some(150.0));

      let periods = finance::wallet_period_summaries_get(&db, 1).await.unwrap();
      assert_eq!(periods.len(), 1);
      assert_eq!(periods[0].period, "2026-01");
      assert_eq!(periods[0].income, 200.0);
      assert_eq!(periods[0].spend, 50.0);
      assert_eq!(periods[0].net, 150.0);
    }

    #[tokio::test]
    async fn it_flows_a_view_reading_intact_through_a_snapshot_into_the_series_with_a_real_delta() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;

      insert_journal(&db, 1, 1, 100.0, 100.0).await;
      assert!(snapshot_financials(&db, 1, "2026-06-01").await);

      insert_asset(&db, 1, 1, 100, 4).await;
      insert_price(&db, 100, Some(10.0), None).await;
      insert_order(&db, 1, 1, 30.0, "open").await;
      insert_contract(&db, 1, 1, 20.0, "outstanding").await;
      assert!(snapshot_financials(&db, 1, "2026-06-03").await);

      let day_two = financials_get(&db, 1).await.unwrap().unwrap();
      assert_eq!(day_two.liquid, Some(100.0));
      assert_eq!(day_two.asset_value, Some(40.0));
      assert_eq!(day_two.escrow, Some(50.0));
      assert_eq!(day_two.net_worth, Some(190.0));

      let series = finance::series_since(&db, Scope::Character(1), Timeframe::Week, date("2026-06-03"))
        .await
        .unwrap();
      assert_eq!(
        series.iter().map(|p| p.date.clone()).collect::<Vec<_>>(),
        ["2026-06-01", "2026-06-03"]
      );
      assert_eq!(series[0].asset_value, None);
      assert_eq!(series[0].net_worth, Some(100.0));
      assert_eq!(series[1].asset_value, Some(40.0));
      assert_eq!(series[1].escrow, Some(50.0));
      assert_eq!(series[1].net_worth, Some(190.0));

      let latest = finance::latest(&db, Scope::Character(1)).await.unwrap().unwrap();
      assert_eq!(latest.date, "2026-06-03");
      assert_eq!(latest.net_worth, Some(190.0));

      let delta = finance::period_delta(&series).unwrap();
      assert_eq!(delta.start, 100.0);
      assert_eq!(delta.end, 190.0);
      assert_eq!(delta.absolute, 90.0);
      assert_eq!(delta.percent, 90.0);
    }

    #[tokio::test]
    async fn it_keeps_a_fully_unsynced_character_an_em_dash_all_the_way_down_and_never_snapshots_zero() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;

      let fin = financials_get(&db, 1).await.unwrap().unwrap();
      assert_eq!(fin.liquid, None);
      assert_eq!(fin.asset_value, None);
      assert_eq!(fin.escrow, None);
      assert_eq!(fin.net_worth, None);

      assert!(!snapshot_financials(&db, 1, "2026-06-03").await);
      assert_eq!(finance::latest(&db, Scope::Character(1)).await.unwrap(), None);
      let series = finance::series_since(&db, Scope::Character(1), Timeframe::Year, date("2026-06-03"))
        .await
        .unwrap();
      assert!(series.is_empty());
    }

    #[tokio::test]
    async fn it_resolves_every_financials_series_and_period_read_from_the_db_alone_no_esi() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_journal(&db, 1, 1, 100.0, 100.0).await;
      insert_asset(&db, 1, 1, 100, 2).await;
      insert_price(&db, 100, Some(10.0), None).await;
      insert_order(&db, 1, 1, 30.0, "open").await;
      insert_contract(&db, 1, 1, 20.0, "outstanding").await;
      snapshot_financials(&db, 1, "2026-06-03").await;

      assert!(financials_get(&db, 1).await.unwrap().is_some());
      assert!(financials_all(&db).await.is_ok());
      assert!(
        finance::series_since(&db, Scope::Character(1), Timeframe::Year, date("2026-06-03"))
          .await
          .is_ok()
      );
      assert!(
        finance::series_since(&db, Scope::Combined, Timeframe::Year, date("2026-06-03"))
          .await
          .is_ok()
      );
      assert!(finance::latest(&db, Scope::Character(1)).await.unwrap().is_some());
      assert!(finance::latest(&db, Scope::Combined).await.unwrap().is_some());
      assert!(finance::wallet_period_summaries_get(&db, 1).await.is_ok());
      assert!(finance::wallet_period_summaries_all(&db).await.is_ok());
    }

    #[tokio::test]
    async fn it_toggles_the_asset_term_with_the_presence_of_asset_rows() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_journal(&db, 1, 1, 500.0, 500.0).await;
      insert_price(&db, 100, Some(10.0), None).await;

      let before = financials_get(&db, 1).await.unwrap().unwrap();
      assert_eq!(before.asset_value, None);
      assert_eq!(before.net_worth, Some(500.0));

      insert_asset(&db, 1, 1, 100, 3).await;
      let after = financials_get(&db, 1).await.unwrap().unwrap();
      assert_eq!(after.asset_value, Some(30.0));
      assert_eq!(after.net_worth, Some(530.0));
    }

    #[tokio::test]
    async fn it_yields_a_real_zero_escrow_not_an_em_dash_for_a_zero_collateral_outstanding_contract() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_contract(&db, 1, 1, 0.0, "outstanding").await;

      let fin = financials_get(&db, 1).await.unwrap().unwrap();
      assert_eq!(fin.escrow, Some(0.0));
      assert_eq!(fin.net_worth, Some(0.0));

      assert!(snapshot_financials(&db, 1, "2026-06-03").await);
      let latest = finance::latest(&db, Scope::Character(1)).await.unwrap().unwrap();
      assert_eq!(latest.net_worth, Some(0.0));
    }
  }
}

#[cfg(test)]
mod market_tests {
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

  fn order(character_id: i64, order_id: i64, escrow: f64, state: &str) -> MarketOrder {
    MarketOrder {
      character_id,
      duration: 90,
      escrow,
      is_buy_order: escrow > 0.0,
      issued: "2026-06-01T12:00:00Z".to_owned(),
      location_id: 60_003_760,
      order_id,
      price: 5.5,
      range: "region".to_owned(),
      region_id: 10_000_002,
      state: state.to_owned(),
      type_id: 34,
      volume_remain: 100,
      volume_total: 200,
    }
  }

  fn price(type_id: i64, adjusted: Option<f64>, average: Option<f64>) -> MarketPrice {
    MarketPrice::esi(type_id, adjusted, average)
  }

  mod all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_preserves_independently_null_prices() {
      let db = store::open_test().await.unwrap();
      market_prices_upsert_many(&db, &[price(34, None, Some(6.0)), price(35, Some(11.0), None)])
        .await
        .unwrap();

      let result = market_prices_all(&db).await.unwrap();

      assert_eq!(result[0].adjusted_price(), None);
      assert_eq!(result[0].average_price(), Some(6.0));
      assert_eq!(result[1].adjusted_price(), Some(11.0));
      assert_eq!(result[1].average_price(), None);
    }

    #[tokio::test]
    async fn it_returns_the_stored_rows_in_type_id_order() {
      let db = store::open_test().await.unwrap();
      market_prices_upsert_many(&db, &[price(34, Some(5.5), Some(6.0)), price(35, Some(11.0), None)])
        .await
        .unwrap();

      let result = market_prices_all(&db).await.unwrap();

      assert_eq!(result.iter().map(MarketPrice::type_id).collect::<Vec<_>>(), [34, 35]);
    }
  }

  mod for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_the_stored_rows_in_order_id_order() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace(
        &db,
        42,
        &[order(42, 200, 100.0, STATE_OPEN), order(42, 100, 50.0, STATE_OPEN)],
      )
      .await
      .unwrap();

      let result = for_character(&db, 42).await.unwrap();

      assert_eq!(result.iter().map(MarketOrder::order_id).collect::<Vec<_>>(), [100, 200]);
    }
  }

  mod open_escrow {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_zero_when_no_orders_exist() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      assert_eq!(open_escrow(&db, 42).await.unwrap(), 0.0);
    }

    #[tokio::test]
    async fn it_sums_escrow_only_over_open_orders() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace(
        &db,
        42,
        &[
          order(42, 100, 50.0, STATE_OPEN),
          order(42, 200, 75.0, STATE_OPEN),
          order(42, 300, 999.0, "expired"),
        ],
      )
      .await
      .unwrap();

      assert_eq!(open_escrow(&db, 42).await.unwrap(), 125.0);
    }
  }

  mod replace {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_leaves_other_characters_untouched() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      replace(&db, 42, &[order(42, 100, 50.0, STATE_OPEN)]).await.unwrap();
      replace(&db, 43, &[order(43, 300, 60.0, STATE_OPEN)]).await.unwrap();

      replace(&db, 42, &[]).await.unwrap();

      assert_eq!(for_character(&db, 43).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_yields_the_current_set_not_duplicates_on_re_replace() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace(
        &db,
        42,
        &[order(42, 100, 50.0, STATE_OPEN), order(42, 200, 75.0, STATE_OPEN)],
      )
      .await
      .unwrap();

      replace(&db, 42, &[order(42, 200, 80.0, STATE_OPEN)]).await.unwrap();

      let result = for_character(&db, 42).await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].order_id(), 200);
      assert_eq!(result[0].escrow(), 80.0);
    }
  }

  mod upsert_many {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_overwrites_existing_price_facts_without_duplicating() {
      let db = store::open_test().await.unwrap();
      market_prices_upsert_many(&db, &[price(34, Some(5.5), Some(6.0))])
        .await
        .unwrap();

      market_prices_upsert_many(&db, &[price(34, Some(7.25), Some(8.0))])
        .await
        .unwrap();

      let result = market_prices_all(&db).await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].adjusted_price(), Some(7.25));
      assert_eq!(result[0].average_price(), Some(8.0));
    }
  }

  mod zkill_gap_type_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn insert_char_asset(db: &Database, item_id: i64, type_id: i64, is_blueprint_copy: Option<i64>) {
      sqlx::query(
        "INSERT INTO character_assets \
          (item_id, character_id, type_id, location_id, location_type, location_flag, quantity, is_blueprint_copy) \
          VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
      )
      .bind(item_id)
      .bind(42_i64)
      .bind(type_id)
      .bind(60_003_760_i64)
      .bind("station")
      .bind("Hangar")
      .bind(1_i64)
      .bind(is_blueprint_copy)
      .execute(db.writer())
      .await
      .unwrap();
    }

    async fn insert_corp_asset(db: &Database, item_id: i64, type_id: i64, is_blueprint_copy: Option<i64>) {
      sqlx::query(
        "INSERT INTO corporation_assets \
          (item_id, corporation_id, type_id, location_id, location_type, location_flag, quantity, is_blueprint_copy) \
          VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
      )
      .bind(item_id)
      .bind(90_000_001_i64)
      .bind(type_id)
      .bind(60_003_760_i64)
      .bind("station")
      .bind("Hangar")
      .bind(1_i64)
      .bind(is_blueprint_copy)
      .execute(db.writer())
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_includes_absent_zkill_and_zero_esi_but_not_priced_or_blueprint_copies() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      // 100: absent from market_prices -> in the gap set
      insert_char_asset(&db, 1, 100, None).await;
      // 200: priced by ESI non-zero -> excluded
      insert_char_asset(&db, 2, 200, None).await;
      market_prices_upsert_many(&db, &[MarketPrice::esi(200, Some(5.0), None)])
        .await
        .unwrap();
      // 300: ESI row but resolved price 0 -> in the gap set
      insert_char_asset(&db, 3, 300, None).await;
      market_prices_upsert_many(&db, &[MarketPrice::esi(300, None, Some(0.0))])
        .await
        .unwrap();
      // 400: corp-held type with an existing zkill row -> re-fetched even though non-zero
      insert_corp_asset(&db, 4, 400, None).await;
      market_prices_upsert_many(&db, &[MarketPrice::zkill(400, 9_000.0)])
        .await
        .unwrap();
      // 500: blueprint copy -> excluded
      insert_char_asset(&db, 5, 500, Some(1)).await;

      let gaps = market_prices_zkill_gap_type_ids(&db).await.unwrap();

      assert_eq!(gaps, vec![100, 300, 400]);
    }
  }
}

#[cfg(test)]
mod net_worth_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character::insert_with_org,
  };

  fn date(iso: &str) -> NaiveDate {
    NaiveDate::parse_from_str(iso, "%Y-%m-%d").unwrap()
  }

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

  async fn insert_journal(db: &Database, id: i64, character_id: i64, date: &str, balance: Option<f64>) {
    sqlx::query(
      "INSERT INTO character_wallet_journal (id, character_id, date, description, ref_type, amount, balance) \
      VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(character_id)
    .bind(date)
    .bind("Test")
    .bind("test")
    .bind(1.0)
    .bind(balance)
    .execute(db.writer())
    .await
    .unwrap();
  }

  mod backfill_liquid_from_journal {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_breaks_same_timestamp_ties_with_the_highest_id() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      insert_journal(&db, 1, 42, "2026-06-01T12:00:00Z", Some(100.0)).await;
      insert_journal(&db, 2, 42, "2026-06-01T12:00:00Z", Some(175.0)).await;

      backfill_liquid_from_journal(&db, 42).await.unwrap();

      let rows = for_character_since(&db, 42, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].liquid(), 175.0);
    }

    #[tokio::test]
    async fn it_excludes_other_characters() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      insert_journal(&db, 1, 42, "2026-06-01T03:00:00Z", Some(100.0)).await;
      insert_journal(&db, 2, 43, "2026-06-01T03:00:00Z", Some(900.0)).await;

      backfill_liquid_from_journal(&db, 42).await.unwrap();

      let rows = for_character_since(&db, 42, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].liquid(), 100.0);
    }

    #[tokio::test]
    async fn it_ignores_entries_without_a_balance() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      insert_journal(&db, 1, 42, "2026-06-01T03:00:00Z", Some(100.0)).await;
      insert_journal(&db, 2, 42, "2026-06-01T21:00:00Z", None).await;

      backfill_liquid_from_journal(&db, 42).await.unwrap();

      let rows = for_character_since(&db, 42, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].liquid(), 100.0);
    }

    #[tokio::test]
    async fn it_is_idempotent_across_re_runs() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      insert_journal(&db, 1, 42, "2026-06-01T03:00:00Z", Some(100.0)).await;

      backfill_liquid_from_journal(&db, 42).await.unwrap();
      backfill_liquid_from_journal(&db, 42).await.unwrap();

      let rows = for_character_since(&db, 42, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].liquid(), 100.0);
    }

    #[tokio::test]
    async fn it_overwrites_liquid_only_and_recomputes_net_worth_preserving_composition() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(&db, 42, "2026-06-01", 100.0, Some(40.0), Some(8.0), 148.0)
        .await
        .unwrap();
      insert_journal(&db, 1, 42, "2026-06-01T21:00:00Z", Some(130.0)).await;

      backfill_liquid_from_journal(&db, 42).await.unwrap();

      let rows = for_character_since(&db, 42, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].liquid(), 130.0);
      assert_eq!(rows[0].asset_value(), Some(40.0));
      assert_eq!(rows[0].escrow(), Some(8.0));
      assert_eq!(rows[0].net_worth(), 178.0);
    }

    #[tokio::test]
    async fn it_writes_one_liquid_only_point_per_utc_day_using_the_last_balance() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      insert_journal(&db, 1, 42, "2026-06-01T03:00:00Z", Some(100.0)).await;
      insert_journal(&db, 2, 42, "2026-06-01T21:00:00Z", Some(150.0)).await;
      insert_journal(&db, 3, 42, "2026-06-02T09:00:00Z", Some(220.0)).await;

      backfill_liquid_from_journal(&db, 42).await.unwrap();

      let rows = for_character_since(&db, 42, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 2);
      assert_eq!(rows[0].date(), "2026-06-01");
      assert_eq!(rows[0].liquid(), 150.0);
      assert_eq!(rows[0].net_worth(), 150.0);
      assert_eq!(rows[0].asset_value(), None);
      assert_eq!(rows[0].escrow(), None);
      assert_eq!(rows[1].date(), "2026-06-02");
      assert_eq!(rows[1].liquid(), 220.0);
    }
  }

  mod combined_series_since {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_skips_nulls_when_summing_asset_value_and_escrow() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      upsert(&db, 42, "2026-06-01", 100.0, Some(40.0), Some(8.0), 148.0)
        .await
        .unwrap();
      upsert(&db, 43, "2026-06-01", 200.0, None, None, 200.0).await.unwrap();

      let series = combined_series_since(&db, "2026-06-01").await.unwrap();

      assert_eq!(series.len(), 1);
      assert_eq!(series[0].liquid(), Some(300.0));
      assert_eq!(series[0].asset_value(), Some(40.0));
      assert_eq!(series[0].escrow(), Some(8.0));
    }

    #[tokio::test]
    async fn it_sums_each_figure_across_characters_per_date() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      upsert(&db, 42, "2026-06-01", 100.0, Some(50.0), Some(10.0), 160.0)
        .await
        .unwrap();
      upsert(&db, 43, "2026-06-01", 200.0, Some(25.0), Some(5.0), 230.0)
        .await
        .unwrap();
      upsert(&db, 43, "2026-06-02", 300.0, None, None, 300.0).await.unwrap();

      let series = combined_series_since(&db, "2026-06-01").await.unwrap();

      assert_eq!(series.len(), 2);
      assert_eq!(series[0].date(), "2026-06-01");
      assert_eq!(series[0].liquid(), Some(300.0));
      assert_eq!(series[0].asset_value(), Some(75.0));
      assert_eq!(series[0].escrow(), Some(15.0));
      assert_eq!(series[0].net_worth(), Some(390.0));
      assert_eq!(series[1].date(), "2026-06-02");
      assert_eq!(series[1].net_worth(), Some(300.0));
    }
  }

  mod for_character_since {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_excludes_other_characters() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      upsert(&db, 42, "2026-06-01", 1.0, None, None, 1.0).await.unwrap();
      upsert(&db, 43, "2026-06-01", 9.0, None, None, 9.0).await.unwrap();

      let rows = for_character_since(&db, 42, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].liquid(), 1.0);
    }

    #[tokio::test]
    async fn it_returns_only_dates_in_range_oldest_first() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(&db, 42, "2026-01-01", 1.0, None, None, 1.0).await.unwrap();
      upsert(&db, 42, "2026-06-02", 3.0, None, None, 3.0).await.unwrap();
      upsert(&db, 42, "2026-06-01", 2.0, None, None, 2.0).await.unwrap();

      let rows = for_character_since(&db, 42, "2026-06-01").await.unwrap();

      assert_eq!(
        rows.iter().map(|r| r.date().clone()).collect::<Vec<_>>(),
        ["2026-06-01", "2026-06-02"]
      );
    }
  }

  mod latest {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_when_the_scope_has_no_snapshots() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      assert_eq!(latest(&db, Scope::Character(42)).await.unwrap(), None);
      assert_eq!(latest(&db, Scope::Combined).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_returns_the_newest_combined_point() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      upsert(&db, 42, "2026-06-02", 100.0, None, None, 100.0).await.unwrap();
      upsert(&db, 43, "2026-06-02", 200.0, None, None, 200.0).await.unwrap();

      let point = latest(&db, Scope::Combined).await.unwrap().unwrap();

      assert_eq!(point.date, "2026-06-02");
      assert_eq!(point.net_worth, Some(300.0));
    }

    #[tokio::test]
    async fn it_returns_the_newest_per_character_point() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(&db, 42, "2026-06-01", 1.0, None, None, 1.0).await.unwrap();
      upsert(&db, 42, "2026-06-03", 3.0, Some(1.0), Some(1.0), 5.0)
        .await
        .unwrap();
      upsert(&db, 42, "2026-06-02", 2.0, None, None, 2.0).await.unwrap();

      let point = latest(&db, Scope::Character(42)).await.unwrap().unwrap();

      assert_eq!(point.date, "2026-06-03");
      assert_eq!(point.net_worth, Some(5.0));
    }
  }

  mod period_delta {
    use pretty_assertions::assert_eq;

    use super::*;

    fn point(net_worth: Option<f64>) -> SeriesPoint {
      SeriesPoint {
        asset_value: None,
        date: "2026-06-01".to_string(),
        escrow: None,
        liquid: None,
        net_worth,
      }
    }

    #[test]
    fn it_measures_first_to_last_net_worth_change_and_percent() {
      let series = [point(Some(100.0)), point(Some(120.0)), point(Some(150.0))];

      let delta = super::super::period_delta(&series).unwrap();

      assert_eq!(delta.start, 100.0);
      assert_eq!(delta.end, 150.0);
      assert_eq!(delta.absolute, 50.0);
      assert_eq!(delta.percent, 50.0);
    }

    #[test]
    fn it_returns_none_without_two_plottable_points() {
      assert_eq!(super::super::period_delta(&[]), None);
      assert_eq!(super::super::period_delta(&[point(Some(100.0))]), None);
      assert_eq!(super::super::period_delta(&[point(None), point(Some(100.0))]), None);
    }

    #[test]
    fn it_skips_null_endpoints_so_the_span_uses_real_readings() {
      let series = [point(None), point(Some(200.0)), point(Some(250.0)), point(None)];

      let delta = super::super::period_delta(&series).unwrap();

      assert_eq!(delta.start, 200.0);
      assert_eq!(delta.end, 250.0);
    }

    #[test]
    fn it_yields_zero_percent_when_the_start_is_not_positive() {
      let series = [point(Some(0.0)), point(Some(40.0))];

      let delta = super::super::period_delta(&series).unwrap();

      assert_eq!(delta.absolute, 40.0);
      assert_eq!(delta.percent, 0.0);
    }
  }

  mod series_since {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_a_per_character_window_ending_on_the_reference_date() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(&db, 42, "2026-05-28", 10.0, Some(1.0), Some(2.0), 13.0)
        .await
        .unwrap();
      upsert(&db, 42, "2026-06-03", 20.0, None, None, 20.0).await.unwrap();
      upsert(&db, 42, "2026-05-27", 99.0, None, None, 99.0).await.unwrap();

      let points = series_since(&db, Scope::Character(42), Timeframe::Week, date("2026-06-03"))
        .await
        .unwrap();

      assert_eq!(
        points.iter().map(|p| p.date.clone()).collect::<Vec<_>>(),
        ["2026-05-28", "2026-06-03"]
      );
      assert_eq!(points[0].liquid, Some(10.0));
      assert_eq!(points[0].asset_value, Some(1.0));
      assert_eq!(points[1].asset_value, None);
      assert_eq!(points[1].net_worth, Some(20.0));
    }

    #[tokio::test]
    async fn it_returns_the_combined_series_summed_over_a_multi_character_fixture() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      upsert(&db, 42, "2026-06-01", 100.0, Some(50.0), Some(10.0), 160.0)
        .await
        .unwrap();
      upsert(&db, 43, "2026-06-01", 200.0, Some(25.0), Some(5.0), 230.0)
        .await
        .unwrap();
      upsert(&db, 43, "2026-06-02", 300.0, None, None, 300.0).await.unwrap();
      upsert(&db, 42, "2026-01-01", 9.0, None, None, 9.0).await.unwrap();

      let points = series_since(&db, Scope::Combined, Timeframe::Month, date("2026-06-03"))
        .await
        .unwrap();

      assert_eq!(points.len(), 2);
      assert_eq!(points[0].date, "2026-06-01");
      assert_eq!(points[0].liquid, Some(300.0));
      assert_eq!(points[0].asset_value, Some(75.0));
      assert_eq!(points[0].net_worth, Some(390.0));
      assert_eq!(points[1].date, "2026-06-02");
      assert_eq!(points[1].net_worth, Some(300.0));
    }
  }

  mod timeframe {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_computes_an_inclusive_window_of_exactly_days_calendar_days() {
      let today = date("2026-06-03");

      assert_eq!(Timeframe::Week.since(today), "2026-05-28");
      assert_eq!(Timeframe::Year.since(today), "2025-06-04");
    }

    #[test]
    fn it_maps_each_window_to_the_design_day_count() {
      assert_eq!(Timeframe::Week.days(), 7);
      assert_eq!(Timeframe::Month.days(), 30);
      assert_eq!(Timeframe::Quarter.days(), 90);
      assert_eq!(Timeframe::HalfYear.days(), 180);
      assert_eq!(Timeframe::Year.days(), 365);
    }
  }

  mod upsert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_idempotent_on_character_id_and_date() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      upsert(&db, 42, "2026-06-01", 100.0, Some(50.0), Some(10.0), 160.0)
        .await
        .unwrap();
      upsert(&db, 42, "2026-06-01", 200.0, Some(75.0), Some(20.0), 295.0)
        .await
        .unwrap();

      let rows = for_character_since(&db, 42, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].liquid(), 200.0);
      assert_eq!(rows[0].asset_value(), Some(75.0));
      assert_eq!(rows[0].escrow(), Some(20.0));
      assert_eq!(rows[0].net_worth(), 295.0);
    }

    #[tokio::test]
    async fn it_stores_null_asset_value_and_escrow_for_backfilled_liquid_only_days() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      upsert(&db, 42, "2026-05-01", 500.0, None, None, 500.0).await.unwrap();

      let rows = for_character_since(&db, 42, "2026-01-01").await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].asset_value(), None);
      assert_eq!(rows[0].escrow(), None);
      assert_eq!(rows[0].net_worth(), 500.0);
    }
  }
}

#[cfg(test)]
mod price_history_tests {
  use super::*;
  use crate::store::{self, model::TypePriceHistory};

  fn history(type_id: i64, date: &str, open: f64, high: f64, low: f64, close: f64) -> TypePriceHistory {
    TypePriceHistory {
      close,
      date: date.to_owned(),
      high,
      low,
      open,
      type_id,
    }
  }

  mod close_as_of {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_when_no_row_is_on_or_before_the_date() {
      let db = store::open_test().await.unwrap();
      price_history_upsert_many(&db, &[history(34, "2026-06-05", 5.0, 6.0, 4.0, 5.5)])
        .await
        .unwrap();

      assert_eq!(close_as_of(&db, 34, "2026-06-01").await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_returns_the_latest_close_on_or_before_the_date() {
      let db = store::open_test().await.unwrap();
      price_history_upsert_many(
        &db,
        &[
          history(34, "2026-06-01", 1.0, 2.0, 0.5, 1.5),
          history(34, "2026-06-03", 3.0, 4.0, 2.5, 3.5),
          history(34, "2026-06-05", 5.0, 6.0, 4.0, 5.5),
        ],
      )
      .await
      .unwrap();

      assert_eq!(close_as_of(&db, 34, "2026-06-04").await.unwrap(), Some(3.5));
      assert_eq!(close_as_of(&db, 34, "2026-06-05").await.unwrap(), Some(5.5));
    }
  }

  mod prune_before {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_drops_rows_older_than_the_cutoff_and_keeps_the_rest() {
      let db = store::open_test().await.unwrap();
      price_history_upsert_many(
        &db,
        &[
          history(34, "2025-01-01", 1.0, 1.0, 1.0, 1.0),
          history(34, "2026-06-04", 2.0, 2.0, 2.0, 2.0),
          history(34, "2026-06-05", 3.0, 3.0, 3.0, 3.0),
        ],
      )
      .await
      .unwrap();

      let pruned = prune_before(&db, "2026-06-05").await.unwrap();

      assert_eq!(pruned, 2, "both rows before the cutoff are removed");
      let remaining = series(&db, 34).await.unwrap();
      assert_eq!(
        remaining
          .iter()
          .map(TypePriceHistory::date)
          .cloned()
          .collect::<Vec<_>>(),
        ["2026-06-05"],
        "the cutoff date itself is kept"
      );
    }

    #[tokio::test]
    async fn it_is_idempotent() {
      let db = store::open_test().await.unwrap();
      price_history_upsert_many(
        &db,
        &[
          history(34, "2025-01-01", 1.0, 1.0, 1.0, 1.0),
          history(34, "2026-06-05", 3.0, 3.0, 3.0, 3.0),
        ],
      )
      .await
      .unwrap();

      assert_eq!(prune_before(&db, "2026-06-01").await.unwrap(), 1);
      assert_eq!(
        prune_before(&db, "2026-06-01").await.unwrap(),
        0,
        "nothing left to prune"
      );
    }
  }

  mod series {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_only_the_requested_type() {
      let db = store::open_test().await.unwrap();
      price_history_upsert_many(
        &db,
        &[
          history(34, "2026-06-01", 1.0, 2.0, 0.5, 1.5),
          history(35, "2026-06-01", 10.0, 12.0, 9.0, 11.0),
        ],
      )
      .await
      .unwrap();

      let result = series(&db, 35).await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].close(), 11.0);
    }

    #[tokio::test]
    async fn it_returns_the_type_series_in_chronological_order() {
      let db = store::open_test().await.unwrap();
      price_history_upsert_many(
        &db,
        &[
          history(34, "2026-06-03", 5.0, 6.0, 4.0, 5.5),
          history(34, "2026-06-01", 1.0, 2.0, 0.5, 1.5),
          history(34, "2026-06-02", 2.0, 3.0, 1.0, 2.5),
        ],
      )
      .await
      .unwrap();

      let result = series(&db, 34).await.unwrap();

      assert_eq!(
        result.iter().map(TypePriceHistory::date).cloned().collect::<Vec<_>>(),
        ["2026-06-01", "2026-06-02", "2026-06-03"]
      );
    }
  }

  mod upsert_many {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_accumulates_a_correct_ohlc_across_several_same_day_samples() {
      let db = store::open_test().await.unwrap();
      let samples = [5.0_f64, 8.0, 3.0, 6.0];
      for price in samples {
        price_history_upsert_many(&db, &[history(34, "2026-06-01", price, price, price, price)])
          .await
          .unwrap();
      }

      let result = series(&db, 34).await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].open(), 5.0, "open is the first sample");
      assert_eq!(result[0].high(), 8.0, "high is the max sample");
      assert_eq!(result[0].low(), 3.0, "low is the min sample");
      assert_eq!(result[0].close(), 6.0, "close is the last sample");
    }

    #[tokio::test]
    async fn it_is_idempotent_when_the_same_sample_is_replayed() {
      let db = store::open_test().await.unwrap();
      price_history_upsert_many(
        &db,
        &[
          history(34, "2026-06-01", 5.0, 5.0, 5.0, 5.0),
          history(34, "2026-06-01", 8.0, 8.0, 8.0, 8.0),
          history(34, "2026-06-01", 6.0, 6.0, 6.0, 6.0),
        ],
      )
      .await
      .unwrap();
      let before = series(&db, 34).await.unwrap();

      price_history_upsert_many(&db, &[history(34, "2026-06-01", 6.0, 6.0, 6.0, 6.0)])
        .await
        .unwrap();

      let after = series(&db, 34).await.unwrap();
      assert_eq!(after, before, "replaying the same close yields the same row");
    }

    #[tokio::test]
    async fn it_rolls_a_same_day_sample_into_the_existing_ohlc_without_duplicating() {
      let db = store::open_test().await.unwrap();
      price_history_upsert_many(&db, &[history(34, "2026-06-01", 1.0, 2.0, 0.5, 1.5)])
        .await
        .unwrap();

      price_history_upsert_many(&db, &[history(34, "2026-06-01", 7.25, 7.25, 7.25, 7.25)])
        .await
        .unwrap();

      let result = series(&db, 34).await.unwrap();

      assert_eq!(result.len(), 1, "the same day stays a single bucket");
      assert_eq!(result[0].open(), 1.0, "open holds the first sample of the day");
      assert_eq!(result[0].high(), 7.25, "high widens to the running max");
      assert_eq!(result[0].low(), 0.5, "low holds the running min");
      assert_eq!(result[0].close(), 7.25, "close adopts the latest sample");
    }
  }
}

#[cfg(test)]
mod wallet_tests {
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

  fn make_entry(character_id: i64, id: i64, amount: Option<f64>, balance: Option<f64>) -> CharacterWalletJournal {
    CharacterWalletJournal {
      amount,
      balance,
      character_id,
      context_id: None,
      context_id_type: None,
      date: "2026-05-30T12:00:00Z".to_owned(),
      description: "Entry".to_owned(),
      first_party_id: None,
      id,
      reason: None,
      ref_type: "player_donation".to_owned(),
      second_party_id: None,
      tax: None,
      tax_receiver_id: None,
    }
  }

  fn make_transaction(character_id: i64, transaction_id: i64, unit_price: f64) -> CharacterWalletTransaction {
    CharacterWalletTransaction {
      character_id,
      client_id: 1_000_035,
      date: "2026-05-30T12:00:00Z".to_owned(),
      is_buy: true,
      is_personal: true,
      journal_ref_id: transaction_id + 1_000,
      location_id: 60_003_760,
      quantity: 10,
      transaction_id,
      type_id: 34,
      unit_price,
    }
  }

  async fn insert_journal(db: &Database, id: i64, character_id: i64, date: &str, amount: Option<f64>) {
    sqlx::query("INSERT INTO character_wallet_journal (id, character_id, date, description, ref_type, amount, balance) VALUES (?, ?, ?, ?, ?, ?, ?)")
      .bind(id)
      .bind(character_id)
      .bind(date)
      .bind("Test")
      .bind("test")
      .bind(amount)
      .bind(0.0)
      .execute(db.writer())
      .await
      .unwrap();
  }

  mod append_wallet_journal {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_idempotent_on_re_appending_the_same_batch() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let batch = [
        make_entry(42, 1, Some(10.0), Some(110.0)),
        make_entry(42, 2, Some(20.0), Some(130.0)),
      ];

      super::append_wallet_journal(&db, &batch).await.unwrap();
      super::append_wallet_journal(&db, &batch).await.unwrap();

      assert_eq!(wallet_journal(&db, 42).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn it_keeps_the_original_row_on_id_conflict() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::append_wallet_journal(&db, &[make_entry(42, 1, Some(10.0), Some(110.0))])
        .await
        .unwrap();

      super::append_wallet_journal(&db, &[make_entry(42, 1, Some(99.0), Some(999.0))])
        .await
        .unwrap();

      let rows = wallet_journal(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].amount(), Some(10.0));
      assert_eq!(rows[0].balance(), Some(110.0));
    }

    #[tokio::test]
    async fn it_round_trips_null_amount_and_balance() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::append_wallet_journal(&db, &[make_entry(42, 1, None, None)])
        .await
        .unwrap();

      let rows = wallet_journal(&db, 42).await.unwrap();
      assert_eq!(rows[0].amount(), None);
      assert_eq!(rows[0].balance(), None);
    }
  }

  mod append_wallet_transaction {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_idempotent_on_re_appending_the_same_batch() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let batch = [make_transaction(42, 1, 5.0), make_transaction(42, 2, 6.0)];

      super::append_wallet_transaction(&db, &batch).await.unwrap();
      super::append_wallet_transaction(&db, &batch).await.unwrap();

      assert_eq!(wallet_transactions(&db, 42).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn it_keeps_the_original_row_on_id_conflict() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::append_wallet_transaction(&db, &[make_transaction(42, 1, 5.0)])
        .await
        .unwrap();

      super::append_wallet_transaction(&db, &[make_transaction(42, 1, 99.0)])
        .await
        .unwrap();

      let rows = wallet_transactions(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].unit_price(), 5.0);
    }
  }

  mod count_journal_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_only_the_given_characters_journal_rows() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      super::append_wallet_journal(
        &db,
        &[
          make_entry(42, 1, Some(10.0), Some(110.0)),
          make_entry(42, 2, Some(20.0), Some(130.0)),
          make_entry(43, 3, Some(5.0), Some(5.0)),
        ],
      )
      .await
      .unwrap();

      assert_eq!(super::count_journal_for_character(&db, 42).await.unwrap(), 2);
      assert_eq!(super::count_journal_for_character(&db, 43).await.unwrap(), 1);
      assert_eq!(super::count_journal_for_character(&db, 99).await.unwrap(), 0);
    }
  }

  mod count_transactions_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_only_the_given_characters_transactions() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      super::append_wallet_transaction(
        &db,
        &[
          make_transaction(42, 1, 5.0),
          make_transaction(42, 2, 6.0),
          make_transaction(43, 3, 7.0),
        ],
      )
      .await
      .unwrap();

      assert_eq!(super::count_transactions_for_character(&db, 42).await.unwrap(), 2);
      assert_eq!(super::count_transactions_for_character(&db, 43).await.unwrap(), 1);
    }
  }

  mod period_all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_orders_by_character_then_period() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_character(&db, 2).await;
      insert_journal(&db, 1, 2, "2026-02-01T00:00:00Z", Some(10.0)).await;
      insert_journal(&db, 2, 1, "2026-03-01T00:00:00Z", Some(10.0)).await;
      insert_journal(&db, 3, 1, "2026-01-01T00:00:00Z", Some(10.0)).await;

      let rows = wallet_period_summaries_all(&db).await.unwrap();

      let keys: Vec<_> = rows.iter().map(|r| (r.character_id, r.period.as_str())).collect();
      assert_eq!(keys, [(1, "2026-01"), (1, "2026-03"), (2, "2026-02")]);
    }

    #[tokio::test]
    async fn it_returns_empty_when_no_journal_rows_exist() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      assert!(wallet_period_summaries_all(&db).await.unwrap().is_empty());
    }
  }

  mod period_get {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_buckets_rows_by_calendar_month() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_journal(&db, 1, 1, "2026-01-05T00:00:00Z", Some(100.0)).await;
      insert_journal(&db, 2, 1, "2026-01-25T00:00:00Z", Some(50.0)).await;
      insert_journal(&db, 3, 1, "2026-02-10T00:00:00Z", Some(7.0)).await;

      let rows = wallet_period_summaries_get(&db, 1).await.unwrap();

      assert_eq!(rows.len(), 2);
      assert_eq!(rows[0].period, "2026-01");
      assert_eq!(rows[0].income, 150.0);
      assert_eq!(rows[1].period, "2026-02");
      assert_eq!(rows[1].income, 7.0);
    }

    #[tokio::test]
    async fn it_excludes_other_characters_rows() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_character(&db, 2).await;
      insert_journal(&db, 1, 2, "2026-01-01T00:00:00Z", Some(500.0)).await;

      assert!(wallet_period_summaries_get(&db, 1).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_returns_empty_for_a_character_with_no_journal_rows() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      assert!(wallet_period_summaries_get(&db, 1).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_splits_positive_amounts_into_income_and_negative_into_spend() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_journal(&db, 1, 1, "2026-01-01T00:00:00Z", Some(1_000.0)).await;
      insert_journal(&db, 2, 1, "2026-01-02T00:00:00Z", Some(-400.0)).await;

      let row = &wallet_period_summaries_get(&db, 1).await.unwrap()[0];

      assert_eq!(row.income, 1_000.0);
      assert_eq!(row.spend, 400.0);
      assert_eq!(row.net, 600.0);
    }

    #[tokio::test]
    async fn it_treats_a_null_amount_as_zero() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_journal(&db, 1, 1, "2026-01-01T00:00:00Z", None).await;
      insert_journal(&db, 2, 1, "2026-01-02T00:00:00Z", Some(25.0)).await;

      let row = &wallet_period_summaries_get(&db, 1).await.unwrap()[0];

      assert_eq!(row.income, 25.0);
      assert_eq!(row.spend, 0.0);
      assert_eq!(row.net, 25.0);
    }
  }

  mod wallet_journal {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_entries_newest_first() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      append_wallet_journal(
        &db,
        &[
          make_entry(42, 1, Some(10.0), Some(110.0)),
          make_entry(42, 3, Some(30.0), Some(170.0)),
          make_entry(42, 2, Some(20.0), Some(130.0)),
        ],
      )
      .await
      .unwrap();

      let rows = super::wallet_journal(&db, 42).await.unwrap();

      assert_eq!(rows.iter().map(|e| e.id()).collect::<Vec<_>>(), [3, 2, 1]);
    }
  }

  mod wallet_journal_page {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_an_empty_page_past_the_last_row() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      append_wallet_journal(&db, &[make_entry(42, 1, Some(10.0), Some(110.0))])
        .await
        .unwrap();

      assert!(
        super::wallet_journal_page(&db, 42, Some(1), 5)
          .await
          .unwrap()
          .is_empty()
      );
    }

    #[tokio::test]
    async fn it_returns_the_first_page_newest_first_when_no_cursor_is_given() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      append_wallet_journal(
        &db,
        &[
          make_entry(42, 1, Some(10.0), Some(110.0)),
          make_entry(42, 2, Some(20.0), Some(130.0)),
          make_entry(42, 3, Some(30.0), Some(170.0)),
        ],
      )
      .await
      .unwrap();

      let page = super::wallet_journal_page(&db, 42, None, 2).await.unwrap();

      assert_eq!(page.iter().map(|e| e.id()).collect::<Vec<_>>(), [3, 2]);
    }

    #[tokio::test]
    async fn it_seeks_past_the_cursor_for_the_next_page() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      append_wallet_journal(
        &db,
        &[
          make_entry(42, 1, Some(10.0), Some(110.0)),
          make_entry(42, 2, Some(20.0), Some(130.0)),
          make_entry(42, 3, Some(30.0), Some(170.0)),
        ],
      )
      .await
      .unwrap();

      let page = super::wallet_journal_page(&db, 42, Some(3), 2).await.unwrap();

      assert_eq!(page.iter().map(|e| e.id()).collect::<Vec<_>>(), [2, 1]);
    }
  }

  mod wallet_transactions {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_transactions_newest_first() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      append_wallet_transaction(
        &db,
        &[
          make_transaction(42, 1, 5.0),
          make_transaction(42, 3, 7.0),
          make_transaction(42, 2, 6.0),
        ],
      )
      .await
      .unwrap();

      let rows = super::wallet_transactions(&db, 42).await.unwrap();

      assert_eq!(rows.iter().map(|t| t.transaction_id()).collect::<Vec<_>>(), [3, 2, 1]);
    }
  }

  mod wallet_transactions_page {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_the_first_page_newest_first_when_no_cursor_is_given() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      append_wallet_transaction(
        &db,
        &[
          make_transaction(42, 1, 5.0),
          make_transaction(42, 2, 6.0),
          make_transaction(42, 3, 7.0),
        ],
      )
      .await
      .unwrap();

      let page = super::wallet_transactions_page(&db, 42, None, 2).await.unwrap();

      assert_eq!(page.iter().map(|t| t.transaction_id()).collect::<Vec<_>>(), [3, 2]);
    }

    #[tokio::test]
    async fn it_seeks_past_the_cursor_for_the_next_page() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      append_wallet_transaction(
        &db,
        &[
          make_transaction(42, 1, 5.0),
          make_transaction(42, 2, 6.0),
          make_transaction(42, 3, 7.0),
        ],
      )
      .await
      .unwrap();

      let page = super::wallet_transactions_page(&db, 42, Some(3), 2).await.unwrap();

      assert_eq!(page.iter().map(|t| t.transaction_id()).collect::<Vec<_>>(), [2, 1]);
    }
  }
}

#[cfg(test)]
mod contract_detail_tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{
      Alliance, Bloodline, Character, CharacterContractBid, CharacterContractItem, Corporation, CorporationContract,
      CorporationContractBid, CorporationContractItem, Gender, Race,
    },
    repo::{character, org},
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
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  async fn seed_corporation(db: &Database, id: i64) {
    let mut corporation = Corporation::new(id, "Test Corporation", "TSTC");
    corporation.set_ceo_id(12_345_678);
    corporation.set_creator_id(12_345_678);
    corporation.set_member_count(100);
    corporation.set_tax_rate(0.1);
    org::upsert_corporation(db, &corporation).await.unwrap();
  }

  fn corp_contract(corporation_id: i64, contract_id: i64, status: &str, date_issued: &str) -> CorporationContract {
    CorporationContract {
      acceptor_id: None,
      acceptor_name: None,
      assignee_id: Some(95_002),
      assignee_name: Some("Assignee Pilot".to_owned()),
      availability: Some("corporation".to_owned()),
      collateral: Some(5000.0),
      contract_id,
      corporation_id,
      date_accepted: Some("2026-03-02T00:00:00Z".to_owned()),
      date_completed: None,
      date_expired: Some("2026-04-01T00:00:00Z".to_owned()),
      date_issued: date_issued.to_owned(),
      days_to_complete: Some(7),
      end_location_id: Some(60_003_761),
      for_corporation: true,
      issuer_corporation_id: Some(98_000_001),
      issuer_id: 95_001,
      issuer_name: Some("Issuer Pilot".to_owned()),
      price: Some(200.0),
      reward: Some(10.0),
      start_location_id: Some(60_003_760),
      status: status.to_owned(),
      title: Some("Haul to Jita".to_owned()),
      r#type: "courier".to_owned(),
      volume: Some(1000.0),
    }
  }

  mod corporation_contracts_page {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_pages_newest_first_and_seeks_past_a_cursor() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 98_000_002).await;
      super::replace_for_corporation(
        &db,
        98_000_002,
        &[
          corp_contract(98_000_002, 1, "finished", "2026-01-01T00:00:00Z"),
          corp_contract(98_000_002, 2, "finished", "2026-02-01T00:00:00Z"),
          corp_contract(98_000_002, 3, "finished", "2026-03-01T00:00:00Z"),
        ],
      )
      .await
      .unwrap();

      let first = super::corporation_contracts_page(&db, 98_000_002, None, 2)
        .await
        .unwrap();
      let next = super::corporation_contracts_page(&db, 98_000_002, Some(("2026-02-01T00:00:00Z", 2)), 2)
        .await
        .unwrap();

      assert_eq!(first.iter().map(|c| c.contract_id()).collect::<Vec<_>>(), [3, 2]);
      assert_eq!(next.iter().map(|c| c.contract_id()).collect::<Vec<_>>(), [1]);
    }
  }

  mod count_contracts_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_only_the_given_corporations_contracts() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 98_000_002).await;
      seed_corporation(&db, 98_000_003).await;
      super::replace_for_corporation(
        &db,
        98_000_002,
        &[
          corp_contract(98_000_002, 1, "finished", "2026-01-01T00:00:00Z"),
          corp_contract(98_000_002, 2, "finished", "2026-02-01T00:00:00Z"),
        ],
      )
      .await
      .unwrap();
      super::replace_for_corporation(
        &db,
        98_000_003,
        &[corp_contract(98_000_003, 3, "finished", "2026-03-01T00:00:00Z")],
      )
      .await
      .unwrap();

      assert_eq!(
        super::count_contracts_for_corporation(&db, 98_000_002).await.unwrap(),
        2
      );
      assert_eq!(
        super::count_contracts_for_corporation(&db, 98_000_003).await.unwrap(),
        1
      );
      assert_eq!(super::count_contracts_for_corporation(&db, 99).await.unwrap(), 0);
    }
  }

  mod replace_contract_bids_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    fn bid(character_id: i64, contract_id: i64, bid_id: i64) -> CharacterContractBid {
      CharacterContractBid {
        amount: 1500.0,
        bid_id,
        bidder_id: 95_010,
        character_id,
        contract_id,
        date_bid: "2026-03-01T00:00:00Z".to_owned(),
      }
    }

    #[tokio::test]
    async fn it_round_trips_bids_and_replaces_per_contract() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_contract_bids_for_character(&db, 42, 1, &[bid(42, 1, 10), bid(42, 1, 11)])
        .await
        .unwrap();

      super::replace_contract_bids_for_character(&db, 42, 1, &[bid(42, 1, 20)])
        .await
        .unwrap();

      let result = super::contract_bids(&db, 42, 1).await.unwrap();
      assert_eq!(result, vec![bid(42, 1, 20)]);
    }
  }

  mod replace_contract_bids_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    fn bid(corporation_id: i64, contract_id: i64, bid_id: i64) -> CorporationContractBid {
      CorporationContractBid {
        amount: 2500.0,
        bid_id,
        bidder_id: 95_010,
        contract_id,
        corporation_id,
        date_bid: "2026-03-01T00:00:00Z".to_owned(),
      }
    }

    #[tokio::test]
    async fn it_round_trips_bids() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 98_000_002).await;

      super::replace_contract_bids_for_corporation(&db, 98_000_002, 1, &[bid(98_000_002, 1, 10)])
        .await
        .unwrap();

      let result = super::corporation_contract_bids(&db, 98_000_002, 1).await.unwrap();
      assert_eq!(result, vec![bid(98_000_002, 1, 10)]);
    }
  }

  mod replace_contract_items_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    fn item(character_id: i64, contract_id: i64, record_id: i64) -> CharacterContractItem {
      CharacterContractItem {
        character_id,
        contract_id,
        is_included: true,
        is_singleton: false,
        quantity: 5,
        raw_quantity: Some(-1),
        record_id,
        type_id: 34,
        value_isk: 12.5,
      }
    }

    #[tokio::test]
    async fn it_round_trips_items_and_replaces_per_contract() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_contract_items_for_character(&db, 42, 1, &[item(42, 1, 100), item(42, 1, 101)])
        .await
        .unwrap();

      super::replace_contract_items_for_character(&db, 42, 1, &[item(42, 1, 200)])
        .await
        .unwrap();

      let result = super::contract_items(&db, 42, 1).await.unwrap();
      assert_eq!(result, vec![item(42, 1, 200)]);
    }
  }

  mod replace_contract_items_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    fn item(corporation_id: i64, contract_id: i64, record_id: i64) -> CorporationContractItem {
      CorporationContractItem {
        contract_id,
        corporation_id,
        is_included: false,
        is_singleton: true,
        quantity: 1,
        raw_quantity: None,
        record_id,
        type_id: 587,
        value_isk: 99.0,
      }
    }

    #[tokio::test]
    async fn it_round_trips_items() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 98_000_002).await;

      super::replace_contract_items_for_corporation(&db, 98_000_002, 1, &[item(98_000_002, 1, 100)])
        .await
        .unwrap();

      let result = super::corporation_contract_items(&db, 98_000_002, 1).await.unwrap();
      assert_eq!(result, vec![item(98_000_002, 1, 100)]);
    }
  }

  mod replace_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_replaces_existing_rows() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 98_000_002).await;
      super::replace_for_corporation(
        &db,
        98_000_002,
        &[corp_contract(98_000_002, 1, "finished", "2025-01-01T00:00:00Z")],
      )
      .await
      .unwrap();

      super::replace_for_corporation(
        &db,
        98_000_002,
        &[corp_contract(98_000_002, 2, "outstanding", "2026-01-01T00:00:00Z")],
      )
      .await
      .unwrap();

      let result = super::corporation_contracts(&db, 98_000_002).await.unwrap();
      assert_eq!(result.iter().map(|c| c.contract_id()).collect::<Vec<_>>(), [2]);
    }

    #[tokio::test]
    async fn it_round_trips_all_header_fields() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 98_000_002).await;

      super::replace_for_corporation(
        &db,
        98_000_002,
        &[corp_contract(98_000_002, 7, "outstanding", "2026-03-01T00:00:00Z")],
      )
      .await
      .unwrap();

      let result = super::corporation_contracts(&db, 98_000_002).await.unwrap();
      assert_eq!(
        result,
        vec![corp_contract(98_000_002, 7, "outstanding", "2026-03-01T00:00:00Z")]
      );
    }
  }
}
