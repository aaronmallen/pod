//! Derives held inventory lots by replaying the wallet transaction ledger through FIFO on every call; there is no
//! materialized lot table, so a dismissal takes effect immediately without any stored state to update.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration};

use crate::store::{
  Database, Error,
  model::{CharacterWalletTransaction, CorporationWalletTransaction, ObservedMarketOrder},
  repo::{finance, org},
};

/// Worst-case observation gap (one 300s sync interval plus a full 300s retry backoff, ~10 minutes) with margin, so a
/// buy isn't unmatched just because its order wasn't re-observed by the latest sync.
const GRACE_MINUTES: i64 = 15;

/// Float tolerance for comparing an order's price to a transaction's unit price; ISK prices are f64, so exact
/// equality is unreliable.
const PRICE_TOLERANCE: f64 = 0.005;

/// Fixed 10% markup applied to a lot's unit cost to derive its resale target price; not configurable per
/// character, corporation, or type.
const TARGET_MARGIN: f64 = 1.10;

type GroupKey = (i64, i64, i64, bool);
type DismissalKey = (i64, i64, bool);
type MatchKey = (i64, bool, i64, i64);

#[derive(Clone, Debug, PartialEq)]
pub struct Lot {
  pub date: String,
  pub quantity: i64,
  pub quantity_remaining: i64,
  pub target_price: f64,
  pub transaction_id: i64,
  pub unit_price: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LotGroup {
  pub average_cost: f64,
  pub average_target: f64,
  pub estimated_profit: f64,
  pub held_quantity: i64,
  pub is_corporation: bool,
  pub location_id: i64,
  pub lots: Vec<Lot>,
  pub owner_id: i64,
  pub type_id: i64,
}

#[derive(Debug)]
struct BuyOrderMatcher {
  windows: HashMap<MatchKey, Vec<OrderWindow>>,
}

impl BuyOrderMatcher {
  fn from_orders(orders: Vec<ObservedMarketOrder>) -> Self {
    let mut windows: HashMap<MatchKey, Vec<OrderWindow>> = HashMap::new();
    for order in orders {
      windows.entry(match_key(&order)).or_default().push(order_window(&order));
    }
    Self {
      windows,
    }
  }

  fn matches(&self, entry: &LedgerEntry) -> bool {
    self
      .windows
      .get(&(entry.owner_id, entry.is_corporation, entry.type_id, entry.location_id))
      .is_some_and(|candidates| {
        candidates
          .iter()
          .any(|window| window.covers(&entry.date, entry.unit_price))
      })
  }
}

#[derive(Clone, Debug, PartialEq)]
struct LedgerEntry {
  date: String,
  is_buy: bool,
  is_corporation: bool,
  location_id: i64,
  owner_id: i64,
  quantity: i64,
  transaction_id: i64,
  type_id: i64,
  unit_price: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct OrderWindow {
  end: String,
  price: f64,
  start: String,
}

impl OrderWindow {
  /// Matches when `date` falls within the window and `price` is within [`PRICE_TOLERANCE`] of it.
  ///
  /// The date comparison is lexicographic, which is safe only because dates share one zero-padded UTC format. The
  /// window's price is the order's latest observed price, not its price history, so a fill made before the order's
  /// price changed is deliberately treated as unmatched.
  fn covers(&self, date: &str, unit_price: f64) -> bool {
    self.start.as_str() <= date && date <= self.end.as_str() && (self.price - unit_price).abs() <= PRICE_TOLERANCE
  }
}

pub async fn derive(db: &Database) -> Result<Vec<LotGroup>, Error> {
  let entries = load_entries(db).await?;
  let matcher = load_matcher(db, &entries).await?;
  let entries = retain_matched_buys(entries, &matcher);
  let dismissed: HashSet<DismissalKey> = finance::dismissed_lots(db).await?.into_iter().collect();
  Ok(derive_groups(entries, &dismissed))
}

async fn load_matcher(db: &Database, entries: &[LedgerEntry]) -> Result<BuyOrderMatcher, Error> {
  let owners: HashSet<(i64, bool)> = entries
    .iter()
    .filter(|entry| entry.is_buy)
    .map(|entry| (entry.owner_id, entry.is_corporation))
    .collect();
  let mut orders = Vec::new();
  for (owner_id, is_corporation) in owners {
    orders.extend(finance::observed_orders_for_owner(db, owner_id, is_corporation, Some(true)).await?);
  }
  Ok(BuyOrderMatcher::from_orders(orders))
}

fn retain_matched_buys(mut entries: Vec<LedgerEntry>, matcher: &BuyOrderMatcher) -> Vec<LedgerEntry> {
  entries.retain(|entry| !entry.is_buy || matcher.matches(entry));
  entries
}

fn match_key(order: &ObservedMarketOrder) -> MatchKey {
  (
    order.owner_id(),
    order.is_corporation(),
    order.type_id(),
    order.location_id(),
  )
}

fn order_window(order: &ObservedMarketOrder) -> OrderWindow {
  // ESI resets `issued` whenever an order is modified, so `first_seen` (our own earliest observation) can predate
  // it; take whichever is earlier so a transaction from before the modification still falls in the window.
  let start = if order.issued() < order.first_seen() {
    order.issued().clone()
  } else {
    order.first_seen().clone()
  };
  OrderWindow {
    end: window_end(order.last_seen()),
    price: order.price(),
    start,
  }
}

fn window_end(last_seen: &str) -> String {
  DateTime::parse_from_rfc3339(last_seen)
    .map(|seen| {
      (seen + Duration::minutes(GRACE_MINUTES))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
    })
    .unwrap_or_else(|_| last_seen.to_owned())
}

async fn load_entries(db: &Database) -> Result<Vec<LedgerEntry>, Error> {
  let mut entries = character_entries(db).await?;
  append_corporation_entries(db, &mut entries).await?;
  Ok(entries)
}

async fn character_entries(db: &Database) -> Result<Vec<LedgerEntry>, Error> {
  Ok(
    finance::wallet_transactions_all(db)
      .await?
      .iter()
      .map(character_entry)
      .collect(),
  )
}

async fn append_corporation_entries(db: &Database, entries: &mut Vec<LedgerEntry>) -> Result<(), Error> {
  // Seeded from the character entries already loaded, so a transaction_id present in both feeds keeps the
  // character-side copy; the corporation-side duplicate is silently dropped.
  let mut seen: HashSet<i64> = entries.iter().map(|entry| entry.transaction_id).collect();
  for corporation in org::all_owned_corporations(db).await? {
    let transactions = finance::corporation_wallet_transactions_all_divisions(db, corporation.id()).await?;
    merge_new_entries(entries, &mut seen, &transactions);
  }
  Ok(())
}

fn merge_new_entries(
  entries: &mut Vec<LedgerEntry>,
  seen: &mut HashSet<i64>,
  transactions: &[CorporationWalletTransaction],
) {
  for transaction in transactions {
    if seen.insert(transaction.transaction_id()) {
      entries.push(corporation_entry(transaction));
    }
  }
}

fn character_entry(transaction: &CharacterWalletTransaction) -> LedgerEntry {
  LedgerEntry {
    date: transaction.date().clone(),
    is_buy: transaction.is_buy(),
    is_corporation: false,
    location_id: transaction.location_id(),
    owner_id: transaction.character_id(),
    quantity: transaction.quantity(),
    transaction_id: transaction.transaction_id(),
    type_id: transaction.type_id(),
    unit_price: transaction.unit_price(),
  }
}

fn corporation_entry(transaction: &CorporationWalletTransaction) -> LedgerEntry {
  LedgerEntry {
    date: transaction.date().clone(),
    is_buy: transaction.is_buy(),
    is_corporation: true,
    location_id: transaction.location_id(),
    owner_id: transaction.corporation_id(),
    quantity: transaction.quantity(),
    transaction_id: transaction.transaction_id(),
    type_id: transaction.type_id(),
    unit_price: transaction.unit_price(),
  }
}

fn derive_groups(entries: Vec<LedgerEntry>, dismissed: &HashSet<DismissalKey>) -> Vec<LotGroup> {
  let mut buckets: HashMap<GroupKey, Vec<LedgerEntry>> = HashMap::new();
  for entry in entries {
    buckets.entry(group_key(&entry)).or_default().push(entry);
  }
  let mut groups: Vec<LotGroup> = buckets
    .into_iter()
    .filter_map(|(key, bucket)| derive_group(key, bucket, dismissed))
    .collect();
  groups.sort_by_key(|group| (group.type_id, group.location_id, group.owner_id, group.is_corporation));
  groups
}

fn group_key(entry: &LedgerEntry) -> GroupKey {
  (entry.type_id, entry.location_id, entry.owner_id, entry.is_corporation)
}

/// Dismissing a buy removes it from the lot pool, but the sold quantity below still counts every sell regardless of
/// dismissal — so FIFO consumption re-attributes the dismissed lot's sold-out share to the next remaining lot.
fn derive_group(key: GroupKey, mut bucket: Vec<LedgerEntry>, dismissed: &HashSet<DismissalKey>) -> Option<LotGroup> {
  bucket.sort_by(|a, b| a.date.cmp(&b.date).then(a.transaction_id.cmp(&b.transaction_id)));
  let mut lots: Vec<Lot> = bucket
    .iter()
    .filter(|entry| entry.is_buy && !is_dismissed(entry, dismissed))
    .map(lot_from_entry)
    .collect();
  let sold: i64 = bucket
    .iter()
    .filter(|entry| !entry.is_buy)
    .map(|entry| entry.quantity)
    .sum();
  consume_fifo(&mut lots, sold);
  lots.retain(|lot| lot.quantity_remaining > 0);
  if lots.is_empty() {
    return None;
  }
  Some(aggregate(key, lots))
}

fn is_dismissed(entry: &LedgerEntry, dismissed: &HashSet<DismissalKey>) -> bool {
  dismissed.contains(&(entry.transaction_id, entry.owner_id, entry.is_corporation))
}

fn lot_from_entry(entry: &LedgerEntry) -> Lot {
  Lot {
    date: entry.date.clone(),
    quantity: entry.quantity,
    quantity_remaining: entry.quantity,
    target_price: entry.unit_price * TARGET_MARGIN,
    transaction_id: entry.transaction_id,
    unit_price: entry.unit_price,
  }
}

/// Consumes `sold` units oldest-lot-first; if `sold` exceeds the total quantity held, the excess is silently
/// dropped rather than going negative or erroring.
fn consume_fifo(lots: &mut [Lot], mut sold: i64) {
  for lot in lots.iter_mut() {
    if sold == 0 {
      break;
    }
    let consumed = sold.min(lot.quantity_remaining);
    lot.quantity_remaining -= consumed;
    sold -= consumed;
  }
}

fn aggregate(key: GroupKey, lots: Vec<Lot>) -> LotGroup {
  let held: i64 = lots.iter().map(|lot| lot.quantity_remaining).sum();
  let cost: f64 = lots
    .iter()
    .map(|lot| lot.quantity_remaining as f64 * lot.unit_price)
    .sum();
  let target: f64 = lots
    .iter()
    .map(|lot| lot.quantity_remaining as f64 * lot.target_price)
    .sum();
  LotGroup {
    average_cost: cost / held as f64,
    average_target: target / held as f64,
    estimated_profit: target - cost,
    held_quantity: held,
    is_corporation: key.3,
    location_id: key.1,
    lots,
    owner_id: key.2,
    type_id: key.0,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const OWNER: i64 = 90_000_042;
  const TYPE: i64 = 34;
  const STATION: i64 = 60_003_760;

  fn entry(transaction_id: i64, date: &str, is_buy: bool, quantity: i64, unit_price: f64) -> LedgerEntry {
    LedgerEntry {
      date: date.to_owned(),
      is_buy,
      is_corporation: false,
      location_id: STATION,
      owner_id: OWNER,
      quantity,
      transaction_id,
      type_id: TYPE,
      unit_price,
    }
  }

  fn buy(transaction_id: i64, date: &str, quantity: i64, unit_price: f64) -> LedgerEntry {
    entry(transaction_id, date, true, quantity, unit_price)
  }

  fn sell(transaction_id: i64, date: &str, quantity: i64) -> LedgerEntry {
    entry(transaction_id, date, false, quantity, 999.0)
  }

  fn order(price: f64, first_seen: &str, last_seen: &str) -> ObservedMarketOrder {
    ObservedMarketOrder {
      first_seen: first_seen.to_owned(),
      is_buy_order: true,
      is_corporation: false,
      issued: first_seen.to_owned(),
      last_seen: last_seen.to_owned(),
      location_id: STATION,
      order_id: 5_000_000_000,
      owner_id: OWNER,
      price,
      region_id: 10_000_002,
      type_id: TYPE,
    }
  }

  mod retain_matched_buys {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keeps_a_buy_matching_an_observed_order() {
      let entries = vec![buy(1, "2026-01-02T00:00:00Z", 10, 100.0)];
      let matcher = BuyOrderMatcher::from_orders(vec![order(100.0, "2026-01-01T00:00:00Z", "2026-01-03T00:00:00Z")]);

      let kept = retain_matched_buys(entries, &matcher);

      assert_eq!(kept.len(), 1);
      assert_eq!(kept[0].transaction_id, 1);
    }

    #[test]
    fn it_hides_a_buy_with_no_observed_order() {
      let entries = vec![buy(1, "2026-01-02T00:00:00Z", 10, 100.0)];
      let matcher = BuyOrderMatcher::from_orders(vec![]);

      let kept = retain_matched_buys(entries, &matcher);

      assert_eq!(kept, vec![]);
    }

    #[test]
    fn it_hides_a_buy_before_the_order_window() {
      let entries = vec![buy(1, "2025-12-31T00:00:00Z", 10, 100.0)];
      let matcher = BuyOrderMatcher::from_orders(vec![order(100.0, "2026-01-01T00:00:00Z", "2026-01-03T00:00:00Z")]);

      let kept = retain_matched_buys(entries, &matcher);

      assert_eq!(kept, vec![]);
    }

    #[test]
    fn it_keeps_a_buy_within_the_grace_period_after_last_seen() {
      let entries = vec![buy(1, "2026-01-03T00:10:00Z", 10, 100.0)];
      let matcher = BuyOrderMatcher::from_orders(vec![order(100.0, "2026-01-01T00:00:00Z", "2026-01-03T00:00:00Z")]);

      let kept = retain_matched_buys(entries, &matcher);

      assert_eq!(kept.len(), 1);
    }

    #[test]
    fn it_hides_a_buy_after_the_grace_period() {
      let entries = vec![buy(1, "2026-01-03T00:16:00Z", 10, 100.0)];
      let matcher = BuyOrderMatcher::from_orders(vec![order(100.0, "2026-01-01T00:00:00Z", "2026-01-03T00:00:00Z")]);

      let kept = retain_matched_buys(entries, &matcher);

      assert_eq!(kept, vec![]);
    }

    #[test]
    fn it_hides_a_buy_with_a_price_mismatch() {
      let entries = vec![buy(1, "2026-01-02T00:00:00Z", 10, 99.0)];
      let matcher = BuyOrderMatcher::from_orders(vec![order(100.0, "2026-01-01T00:00:00Z", "2026-01-03T00:00:00Z")]);

      let kept = retain_matched_buys(entries, &matcher);

      assert_eq!(kept, vec![]);
    }

    #[test]
    fn it_uses_the_issued_date_when_it_precedes_first_seen() {
      let entries = vec![buy(1, "2026-01-01T06:00:00Z", 10, 100.0)];
      let mut observed = order(100.0, "2026-01-01T12:00:00Z", "2026-01-03T00:00:00Z");
      observed.issued = "2026-01-01T00:00:00Z".to_owned();
      let matcher = BuyOrderMatcher::from_orders(vec![observed]);

      let kept = retain_matched_buys(entries, &matcher);

      assert_eq!(kept.len(), 1);
    }

    #[test]
    fn it_matches_corporation_buys_only_against_corporation_orders() {
      let mut corp_buy = buy(1, "2026-01-02T00:00:00Z", 10, 100.0);
      corp_buy.is_corporation = true;
      let character_order = order(100.0, "2026-01-01T00:00:00Z", "2026-01-03T00:00:00Z");
      let mut corp_order = order(100.0, "2026-01-01T00:00:00Z", "2026-01-03T00:00:00Z");
      corp_order.is_corporation = true;

      let unmatched = retain_matched_buys(
        vec![corp_buy.clone()],
        &BuyOrderMatcher::from_orders(vec![character_order]),
      );
      assert_eq!(unmatched, vec![]);

      let matched = retain_matched_buys(vec![corp_buy], &BuyOrderMatcher::from_orders(vec![corp_order]));
      assert_eq!(matched.len(), 1);
    }

    #[test]
    fn it_keeps_sells_for_fifo_consumption_of_matched_lots() {
      let entries = vec![
        buy(1, "2026-01-01T12:00:00Z", 10, 100.0),
        buy(2, "2026-01-04T00:00:00Z", 10, 100.0),
        sell(3, "2026-01-02T00:00:00Z", 4),
      ];
      let matcher = BuyOrderMatcher::from_orders(vec![order(100.0, "2026-01-01T00:00:00Z", "2026-01-03T00:00:00Z")]);

      let kept = retain_matched_buys(entries, &matcher);
      let groups = derive_groups(kept, &HashSet::new());

      assert_eq!(groups.len(), 1);
      assert_eq!(groups[0].lots.len(), 1);
      assert_eq!(groups[0].lots[0].transaction_id, 1);
      assert_eq!(groups[0].lots[0].quantity_remaining, 6);
    }
  }

  mod derive_groups {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_consumes_the_oldest_lot_first() {
      let entries = vec![
        buy(2, "2026-01-02T00:00:00Z", 10, 200.0),
        buy(1, "2026-01-01T00:00:00Z", 10, 100.0),
        sell(3, "2026-01-03T00:00:00Z", 10),
      ];

      let groups = derive_groups(entries, &HashSet::new());

      assert_eq!(groups.len(), 1);
      assert_eq!(groups[0].lots.len(), 1);
      assert_eq!(groups[0].lots[0].transaction_id, 2);
      assert_eq!(groups[0].lots[0].quantity_remaining, 10);
    }

    #[test]
    fn it_keeps_the_remainder_of_a_partially_sold_lot() {
      let entries = vec![
        buy(1, "2026-01-01T00:00:00Z", 10, 100.0),
        sell(2, "2026-01-02T00:00:00Z", 4),
      ];

      let groups = derive_groups(entries, &HashSet::new());

      assert_eq!(groups.len(), 1);
      assert_eq!(groups[0].lots[0].quantity, 10);
      assert_eq!(groups[0].lots[0].quantity_remaining, 6);
      assert_eq!(groups[0].held_quantity, 6);
    }

    #[test]
    fn it_drops_fully_consumed_groups() {
      let entries = vec![
        buy(1, "2026-01-01T00:00:00Z", 10, 100.0),
        sell(2, "2026-01-02T00:00:00Z", 10),
      ];

      let groups = derive_groups(entries, &HashSet::new());

      assert_eq!(groups, vec![]);
    }

    #[test]
    fn it_excludes_dismissed_buy_transactions() {
      let entries = vec![
        buy(1, "2026-01-01T00:00:00Z", 10, 100.0),
        buy(2, "2026-01-02T00:00:00Z", 10, 200.0),
      ];
      let dismissed: HashSet<DismissalKey> = [(1, OWNER, false)].into_iter().collect();

      let groups = derive_groups(entries, &dismissed);

      assert_eq!(groups.len(), 1);
      assert_eq!(groups[0].lots.len(), 1);
      assert_eq!(groups[0].lots[0].transaction_id, 2);
    }

    #[test]
    fn it_reattributes_sells_to_remaining_lots_when_a_partially_sold_lot_is_dismissed() {
      let entries = vec![
        buy(1, "2026-01-01T00:00:00Z", 10, 100.0),
        buy(2, "2026-01-02T00:00:00Z", 10, 200.0),
        sell(3, "2026-01-03T00:00:00Z", 5),
      ];

      let before = derive_groups(entries.clone(), &HashSet::new());
      assert_eq!(before[0].lots[0].transaction_id, 1);
      assert_eq!(before[0].lots[0].quantity_remaining, 5);
      assert_eq!(before[0].lots[1].quantity_remaining, 10);

      let dismissed: HashSet<DismissalKey> = [(1, OWNER, false)].into_iter().collect();
      let after = derive_groups(entries, &dismissed);

      assert_eq!(after.len(), 1);
      assert_eq!(after[0].lots.len(), 1);
      assert_eq!(after[0].lots[0].transaction_id, 2);
      assert_eq!(after[0].lots[0].quantity_remaining, 5);
    }

    #[test]
    fn it_isolates_lots_across_locations() {
      let mut elsewhere = sell(2, "2026-01-02T00:00:00Z", 10);
      elsewhere.location_id = STATION + 1;
      let entries = vec![buy(1, "2026-01-01T00:00:00Z", 10, 100.0), elsewhere];

      let groups = derive_groups(entries, &HashSet::new());

      assert_eq!(groups.len(), 1);
      assert_eq!(groups[0].location_id, STATION);
      assert_eq!(groups[0].lots[0].quantity_remaining, 10);
    }

    #[test]
    fn it_isolates_lots_across_owners() {
      let mut other = sell(2, "2026-01-02T00:00:00Z", 10);
      other.owner_id = OWNER + 1;
      let entries = vec![buy(1, "2026-01-01T00:00:00Z", 10, 100.0), other];

      let groups = derive_groups(entries, &HashSet::new());

      assert_eq!(groups.len(), 1);
      assert_eq!(groups[0].lots[0].quantity_remaining, 10);
    }

    #[test]
    fn it_computes_weighted_aggregates_and_target_prices() {
      let entries = vec![
        buy(1, "2026-01-01T00:00:00Z", 10, 100.0),
        buy(2, "2026-01-02T00:00:00Z", 10, 200.0),
      ];

      let groups = derive_groups(entries, &HashSet::new());

      assert_eq!(groups[0].held_quantity, 20);
      assert!((groups[0].average_cost - 150.0).abs() < 1e-9);
      assert!((groups[0].lots[0].target_price - 110.0).abs() < 1e-9);
      assert!((groups[0].average_target - 165.0).abs() < 1e-9);
      assert!((groups[0].estimated_profit - 300.0).abs() < 1e-9);
    }
  }

  mod consume_fifo {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_stops_when_sells_exceed_held_quantity() {
      let mut lots = vec![lot_from_entry(&buy(1, "2026-01-01T00:00:00Z", 10, 100.0))];

      consume_fifo(&mut lots, 25);

      assert_eq!(lots[0].quantity_remaining, 0);
    }
  }
}
