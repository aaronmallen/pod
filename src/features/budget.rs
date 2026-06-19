//! Budget B2: activity derivation and the budgetable pool.
//!
//! Pure, DB-derived logic for the YNAB-style zero-based budget. This module owns
//! the *math* — seeding a fresh scope's starter envelopes, mapping EVE journal
//! `ref_type`s onto categories, summing monthly activity, rolling carry-over
//! forward, and deriving the budgetable pool / Ready-to-Assign. Rendering
//! (B3/B4), drag-and-drop CRUD (B5), and the assign/cover *writes* (B3) live
//! elsewhere; B2 only provides figures.

use std::collections::{HashMap, HashSet};

use crate::store::{
  Database, Error,
  model::{BudgetEntryAssignment, BudgetEntryKind, BudgetScope},
  repo::{character, finance, org},
};

/// A starter category in a seeded group: a stable `slug` (used to attach
/// default `ref_type` maps without hard-coding row ids), a display `name`, and
/// an optional `tone` for the eventual UI.
struct SeedCategory {
  name: &'static str,
  slug: &'static str,
  tone: Option<&'static str>,
}

/// A starter group of categories, in the spirit of `budget-data.jsx` GROUPS.
struct SeedGroup {
  cats: &'static [SeedCategory],
  name: &'static str,
}

/// The default starter layout for a fresh budget scope. Mirrors the wireframe's
/// GROUPS but trimmed to the envelopes the default `ref_type` map actually
/// targets, so a freshly-seeded scope has somewhere to route every mapped flow.
const SEED_GROUPS: &[SeedGroup] = &[
  SeedGroup {
    name: "Income",
    cats: &[
      SeedCategory {
        name: "Bounties & rewards",
        slug: "income",
        tone: Some("success"),
      },
      SeedCategory {
        name: "Transfers in/out",
        slug: "transfers",
        tone: Some("muted"),
      },
    ],
  },
  SeedGroup {
    name: "Trading",
    cats: &[
      SeedCategory {
        name: "Market trading",
        slug: "trading",
        tone: Some("plasma"),
      },
      SeedCategory {
        name: "Sales tax & broker fees",
        slug: "fees",
        tone: Some("danger"),
      },
    ],
  },
  SeedGroup {
    name: "Obligations",
    cats: &[
      SeedCategory {
        name: "Corp tithe & tax",
        slug: "tithe",
        tone: Some("muted"),
      },
      SeedCategory {
        name: "Contracts",
        slug: "contracts",
        tone: Some("info"),
      },
      SeedCategory {
        name: "Industry",
        slug: "industry",
        tone: Some("warning"),
      },
    ],
  },
];

/// Default mapping from EVE journal `ref_type` to a starter category slug. Built
/// on the same `ref_type` vocabulary the wallet uses (`is_known_income_ref_type`
/// & friends). User overrides in `budget_ref_type_maps` (B1) win over these.
const DEFAULT_REF_TYPE_MAP: &[(&str, &str)] = &[
  // Income
  ("bounty_prizes", "income"),
  ("bounty_prize", "income"),
  ("agent_mission_reward", "income"),
  ("agent_mission_time_bonus_reward", "income"),
  ("insurance", "income"),
  ("lp_store", "income"),
  ("project_reward", "income"),
  ("reprocessing_tax", "income"),
  ("ess_escrow_transfer", "income"),
  // Trading
  ("market_transaction", "trading"),
  ("market_escrow", "trading"),
  ("market_provider_tax", "fees"),
  // Fees
  ("brokers_fee", "fees"),
  ("broker_fee", "fees"),
  ("transaction_tax", "fees"),
  // Corp tithe / tax
  ("corporation_account_withdrawal", "tithe"),
  ("corporation_dividend_payment", "tithe"),
  ("bounty_prizes_tax", "tithe"),
  ("industry_job_tax", "tithe"),
  // Contracts
  ("contract_price_payment_corp", "contracts"),
  ("contract_price", "contracts"),
  ("contract_reward", "contracts"),
  ("contract_reward_refund", "contracts"),
  ("contract_collateral", "contracts"),
  ("contract_brokers_fee", "fees"),
  ("contract_deposit", "contracts"),
  // Transfers
  ("player_donation", "transfers"),
  ("player_trading", "transfers"),
  ("corporation_payment", "transfers"),
  // Industry
  ("manufacturing", "industry"),
  ("industry_job_refund", "industry"),
  ("structure_gate_jump", "industry"),
  ("jump_clone_installation_fee", "industry"),
  ("office_rental_fee", "industry"),
];

/// Default category slug for a market transaction by side — the canonical
/// translation of the design's `defaultBudgetCatForMarket` (a buy is working
/// capital flowing out, a sell is income flowing in) onto the seeded slug
/// vocabulary. A per-entry assignment takes precedence over this default.
const MARKET_BUY_SLUG: &str = "trading";

const MARKET_SELL_SLUG: &str = "income";

/// The `context_id_type` EVE stamps on a market-trade journal entry; its
/// `context_id` is the linked `transaction_id`. Used to de-duplicate the journal
/// twin against the ingested transaction so a trade is counted once.
const MARKET_TRANSACTION_CONTEXT_ID_TYPE: &str = "market_transaction_id";

/// The journal `ref_type` of a market-trade principal entry — the twin of a
/// wallet transaction row. Broker/tax fees carry their own ref_types and are
/// never suppressed by de-duplication.
const MARKET_TRANSACTION_REF_TYPE: &str = "market_transaction";

/// A single category's month figures, derived live.
///
/// `available = carry + assigned + activity`. `activity` is the signed sum of
/// journal `amount` mapped to this category for the month (positive = in,
/// negative = out). `carry` is last month's positive available rolled forward.
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CategoryMonth {
  pub activity: f64,
  pub assigned: f64,
  pub carry: f64,
}

impl CategoryMonth {
  // Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
  // until then.
  #[allow(dead_code)]
  pub fn available(self) -> f64 {
    self.carry + self.assigned + self.activity
  }
}

/// A single month's reporting figures for the Reflect view.
///
/// `age` is the ISK-quantity-weighted mean age in days of ISK spent that month,
/// computed by the FIFO lot model in [`fifo_ages_by_month`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MonthFlow {
  pub age: f64,
  pub assigned: f64,
  pub income: f64,
  pub month: String,
  pub spend: f64,
}

/// The budgetable pool for a scope and the derived YNAB top-line figures.
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoolSummary {
  /// Σ min(0, available) across categories (≤ 0); fuels B3's Cover-overspending.
  pub overspent: f64,
  /// Σ liquid balances of the scope's character + corp-division wallets.
  pub pool: f64,
  /// pool − Σ available(all categories).
  pub ready_to_assign: f64,
}

/// Returns the seed `(ref_type, slug)` defaults as an owned map for lookups and
/// tests, deduplicated on `ref_type` (later entries win, though there are none).
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub fn default_ref_type_slugs() -> HashMap<&'static str, &'static str> {
  DEFAULT_REF_TYPE_MAP.iter().copied().collect()
}

/// Normalises an RFC3339 / `YYYY-MM-DD...` timestamp to its UTC calendar month
/// key `YYYY-MM`. EVE journal dates are already UTC, so a lexical slice of the
/// leading `YYYY-MM` is exact and avoids a parse on the hot path.
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub fn month_key(date: &str) -> Option<String> {
  let bytes = date.as_bytes();
  if bytes.len() < 7 || bytes[4] != b'-' {
    return None;
  }
  let head = &date[..7];
  if head[..4].bytes().all(|b| b.is_ascii_digit()) && head[5..7].bytes().all(|b| b.is_ascii_digit()) {
    Some(head.to_owned())
  } else {
    None
  }
}

/// Carry rolled into `month` from the prior month's available: YNAB rollover,
/// where only a *positive* available carries forward (overspending does not
/// follow ISK into next month as a positive balance). Returns 0 for the first
/// month or any gap with no prior data.
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub fn carry_from(prior_available: Option<f64>) -> f64 {
  prior_available.map_or(0.0, |available| available.max(0.0))
}

/// Rolls carry-over across an ordered month series for a single category.
///
/// `months` is `(carry-bearing month key, assigned, activity)` for each month in
/// chronological order. The first element's carry is computed from `seed_carry`
/// (0 for a brand-new category). Each subsequent month carries the prior month's
/// `max(0, available)`. Gap months simply do not appear in `months`; the next
/// present month carries from whatever the previous *present* month resolved to.
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub fn roll_carry(seed_carry: f64, months: &[(f64, f64)]) -> Vec<CategoryMonth> {
  let mut out = Vec::with_capacity(months.len());
  let mut carry = seed_carry;
  for &(assigned, activity) in months {
    let month = CategoryMonth {
      activity,
      assigned,
      carry,
    };
    carry = carry_from(Some(month.available()));
    out.push(month);
  }
  out
}

/// Resolves a `ref_type` to a category id for a scope, overlaying the persisted
/// user overrides (`maps`) on top of the seeded defaults (`slug_to_id` maps a
/// default slug to its concrete category id for this scope). Returns `None` when
/// neither an override nor a default applies (the flow is unmapped).
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub fn category_for_ref_type(
  ref_type: &str,
  overrides: &HashMap<String, i64>,
  slug_to_id: &HashMap<&str, i64>,
) -> Option<i64> {
  if let Some(&id) = overrides.get(ref_type) {
    return Some(id);
  }
  let slug = default_ref_type_slugs().get(ref_type).copied()?;
  slug_to_id.get(slug).copied()
}

/// Persists a per-entry budget envelope override for a single ledger entry,
/// lazy-seeding the scope's default budget first so an unseeded scope gains real
/// categories before the assignment lands. Idempotent on `(scope, entry_kind,
/// entry_id)`: reassigning the same entry replaces its category.
// Per-entry budget assignment (child A); consumed by the Budget UI in child C. Exercised by unit
// tests until then.
#[allow(dead_code)]
pub async fn assign_entry(
  db: &Database,
  scope: BudgetScope,
  entry_kind: BudgetEntryKind,
  entry_id: i64,
  category_id: i64,
) -> Result<BudgetEntryAssignment, Error> {
  seed_scope(db, scope).await?;
  crate::store::repo::budget::upsert_entry_assignment(db, scope, entry_kind, entry_id, category_id).await
}

fn market_default_slug(is_buy: bool) -> &'static str {
  if is_buy { MARKET_BUY_SLUG } else { MARKET_SELL_SLUG }
}

/// A scope's loaded resolution inputs — per-entry overrides (keyed by entry id
/// within each entry kind), the per-`ref_type` map overlay, and the seeded
/// slug→category map. Loaded once, then `resolve` runs in-memory so the
/// derivation resolves thousands of entries without re-querying per entry.
// Budget activity derivation (child B); consumed by the Budget Plan/Reflect UI and the per-entry
// chip in child C. Exercised by unit tests until then.
#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct ResolutionContext {
  pub journal_overrides: HashMap<i64, i64>,
  pub market_overrides: HashMap<i64, i64>,
  pub ref_overrides: HashMap<String, i64>,
  pub slug_to_id: HashMap<&'static str, i64>,
}

impl ResolutionContext {
  pub async fn load(db: &Database, scope: BudgetScope) -> Self {
    let mut journal_overrides = HashMap::new();
    let mut market_overrides = HashMap::new();
    for assignment in crate::store::repo::budget::list_entry_assignments(db, scope)
      .await
      .unwrap_or_default()
    {
      match BudgetEntryKind::from_kind(assignment.entry_kind()) {
        Some(BudgetEntryKind::Journal) => {
          journal_overrides.insert(assignment.entry_id(), assignment.category_id());
        }
        Some(BudgetEntryKind::Market) => {
          market_overrides.insert(assignment.entry_id(), assignment.category_id());
        }
        None => {}
      }
    }

    Self {
      journal_overrides,
      market_overrides,
      ref_overrides: ref_type_overrides(db, scope).await,
      slug_to_id: slug_to_category_id(db, scope).await,
    }
  }

  pub fn resolve(
    &self,
    entry_kind: BudgetEntryKind,
    entry_id: i64,
    ref_type: Option<&str>,
    is_buy: Option<bool>,
  ) -> Option<i64> {
    match entry_kind {
      BudgetEntryKind::Journal => {
        if let Some(&id) = self.journal_overrides.get(&entry_id) {
          return Some(id);
        }
        category_for_ref_type(ref_type?, &self.ref_overrides, &self.slug_to_id)
      }
      // Market entries carry a side, not a `ref_type`, so the per-`ref_type` map
      // tier does not apply — they resolve by side once no per-entry override exists.
      BudgetEntryKind::Market => {
        if let Some(&id) = self.market_overrides.get(&entry_id) {
          return Some(id);
        }
        self.slug_to_id.get(market_default_slug(is_buy?)).copied()
      }
    }
  }
}

/// Resolves a single ledger entry to its effective budget category for a scope,
/// honoring the precedence: per-entry override → per-`ref_type` map → seed
/// default. Journal entries resolve through their `ref_type`; market entries
/// resolve through their side (`is_buy`). Returns `None` when nothing maps
/// (e.g. an unseeded scope, or an unmapped `ref_type`).
// Per-entry budget assignment (child A); consumed by the Budget derivation/UI in children B/C.
// Exercised by unit tests until then.
#[allow(dead_code)]
pub async fn resolve_entry_category(
  db: &Database,
  scope: BudgetScope,
  entry_kind: BudgetEntryKind,
  entry_id: i64,
  ref_type: Option<&str>,
  is_buy: Option<bool>,
) -> Option<i64> {
  ResolutionContext::load(db, scope)
    .await
    .resolve(entry_kind, entry_id, ref_type, is_buy)
}

/// Aggregates signed journal `amount` by mapped category id for a set of
/// entries, using `resolve` to map each entry — by its id and `ref_type` — to a
/// category. The entry id lets per-entry overrides win over the `ref_type`
/// default. Entries with no amount, or no resolved category, are skipped.
/// Positive amounts add to activity (income/in), negative subtract (spend/out).
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub fn aggregate_activity<'a>(
  entries: impl IntoIterator<Item = (i64, &'a str, Option<f64>)>,
  mut resolve: impl FnMut(i64, &str) -> Option<i64>,
) -> HashMap<i64, f64> {
  let mut by_category: HashMap<i64, f64> = HashMap::new();
  for (entry_id, ref_type, amount) in entries {
    let Some(amount) = amount else { continue };
    let Some(category_id) = resolve(entry_id, ref_type) else {
      continue;
    };
    *by_category.entry(category_id).or_insert(0.0) += amount;
  }
  by_category
}

/// Ready-to-Assign and overspending from a pool and the categories' `available`
/// figures. `ready_to_assign = pool − Σ available`; `overspent = Σ min(0,
/// available)`.
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub fn pool_summary(pool: f64, availables: impl IntoIterator<Item = f64>) -> PoolSummary {
  let mut total_available = 0.0;
  let mut overspent = 0.0;
  for available in availables {
    total_available += available;
    if available < 0.0 {
      overspent += available;
    }
  }
  PoolSummary {
    overspent,
    pool,
    ready_to_assign: pool - total_available,
  }
}

/// The character ids whose journals/balances a scope covers.
///
/// `All` is every owned character; `Character(id)` is just that pilot;
/// `Corporation` covers no character wallets (its money lives in divisions).
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub async fn scope_character_ids(db: &Database, scope: BudgetScope) -> Vec<i64> {
  match scope {
    BudgetScope::All => character::all_owned(db)
      .await
      .unwrap_or_default()
      .iter()
      .map(crate::store::model::Character::id)
      .collect(),
    BudgetScope::Character(id) => vec![id],
    BudgetScope::Corporation(_) => Vec::new(),
  }
}

/// The corporation ids whose division wallets a scope covers: every owned
/// corporation for `All`, the one corporation for `Corporation`, and none for a
/// single-character scope.
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub async fn scope_corporation_ids(db: &Database, scope: BudgetScope) -> Vec<i64> {
  match scope {
    BudgetScope::All => org::all_owned_corporations(db)
      .await
      .unwrap_or_default()
      .iter()
      .map(crate::store::model::OwnedCorporation::id)
      .collect(),
    BudgetScope::Corporation(id) => vec![id],
    BudgetScope::Character(_) => Vec::new(),
  }
}

/// The budgetable pool: Σ liquid balances across the scope's character wallets
/// plus the corp division wallets it covers (mirrors the wallet's `Scope`
/// liquid roll-up). Missing balances are treated as 0.
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub async fn budgetable_pool(db: &Database, scope: BudgetScope) -> f64 {
  let mut pool = 0.0;
  for id in scope_character_ids(db, scope).await {
    if let Ok(Some(row)) = finance::financials_get(db, id).await {
      pool += row.liquid.unwrap_or(0.0);
    }
  }
  for corp in scope_corporation_ids(db, scope).await {
    for division in finance::divisions(db, corp).await.unwrap_or_default() {
      pool += division.balance().unwrap_or(0.0);
    }
  }
  pool
}

/// The persisted `ref_type` → category-id overrides for a scope.
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub async fn ref_type_overrides(db: &Database, scope: BudgetScope) -> HashMap<String, i64> {
  crate::store::repo::budget::list_ref_type_maps(db, scope)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| (row.ref_type().clone(), row.category_id()))
    .collect()
}

/// Maps a seeded scope's default category slugs to their concrete row ids by
/// matching seed category names. Used to resolve default `ref_type` mappings
/// after seeding without persisting a slug column.
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub async fn slug_to_category_id(db: &Database, scope: BudgetScope) -> HashMap<&'static str, i64> {
  let name_to_slug: HashMap<&str, &str> = SEED_GROUPS
    .iter()
    .flat_map(|group| group.cats.iter())
    .map(|cat| (cat.name, cat.slug))
    .collect();

  let mut out = HashMap::new();
  let groups = crate::store::repo::budget::list_groups(db, scope)
    .await
    .unwrap_or_default();
  for group in &groups {
    let categories = crate::store::repo::budget::list_categories(db, group.id())
      .await
      .unwrap_or_default();
    for category in &categories {
      if let Some(&slug) = name_to_slug.get(category.name().as_str()) {
        out.insert(slug, category.id());
      }
    }
  }
  out
}

/// Seeds a fresh scope with the starter groups/categories and the default
/// `ref_type` map. Idempotent: a second call adds nothing. "Fresh" means the
/// scope has no groups yet; once any group exists the seed is a no-op so a user
/// who deletes a starter envelope never has it resurrected.
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub async fn seed_scope(db: &Database, scope: BudgetScope) -> Result<(), Error> {
  use crate::store::repo::budget::{
    NewCategory, NewGroup, create_category, create_group, list_groups, upsert_ref_type_map,
  };

  if !list_groups(db, scope).await?.is_empty() {
    return Ok(());
  }

  let mut slug_to_id: HashMap<&str, i64> = HashMap::new();
  for (group_position, group) in SEED_GROUPS.iter().enumerate() {
    let created_group = create_group(
      db,
      &NewGroup {
        name: group.name.to_owned(),
        position: group_position as i64,
        scope,
      },
    )
    .await?;
    for (cat_position, cat) in group.cats.iter().enumerate() {
      let created = create_category(
        db,
        &NewCategory {
          group_id: created_group.id(),
          name: cat.name.to_owned(),
          note: None,
          position: cat_position as i64,
          tone: cat.tone.map(str::to_owned),
        },
      )
      .await?;
      slug_to_id.insert(cat.slug, created.id());
    }
  }

  for &(ref_type, slug) in DEFAULT_REF_TYPE_MAP {
    if let Some(&category_id) = slug_to_id.get(slug) {
      upsert_ref_type_map(db, scope, ref_type, category_id).await?;
    }
  }

  Ok(())
}

/// A journal row reduced to the fields the monthly derivation needs: its id (for
/// per-entry overrides), `date`/`ref_type`/`amount` for the sum, and
/// `context_id`/`context_id_type` for market-twin de-duplication.
struct JournalActivity {
  amount: Option<f64>,
  context_id: Option<i64>,
  context_id_type: Option<String>,
  date: String,
  id: i64,
  ref_type: String,
}

/// A wallet transaction reduced to the fields the monthly derivation needs, with
/// `amount` already signed (buy = spend/negative, sell = income/positive).
struct TransactionActivity {
  amount: f64,
  date: String,
  is_buy: bool,
  transaction_id: i64,
}

/// True when a journal row is the twin of an ingested market transaction —
/// the trade-principal entry (`ref_type = market_transaction`,
/// `context_id_type = market_transaction_id`) whose `context_id` links to a
/// transaction already counted from the transaction source. Such rows are
/// suppressed to avoid double-counting; broker/tax fees carry other ref_types
/// and are never matched here. A `market_transaction` row with no linked
/// transaction (unsynced) is kept so its activity is not lost.
fn is_market_twin(row: &JournalActivity, ingested: &HashSet<i64>) -> bool {
  row.ref_type == MARKET_TRANSACTION_REF_TYPE
    && row.context_id_type.as_deref() == Some(MARKET_TRANSACTION_CONTEXT_ID_TYPE)
    && row.context_id.is_some_and(|id| ingested.contains(&id))
}

/// Aggregates a scope's signed activity by category for a single UTC calendar
/// `month` (`YYYY-MM`), unioning every in-scope character and covered
/// corp-division journal with the matching wallet transactions. Each entry
/// resolves through the precedence per-entry override → `ref_type` map → seed
/// default; market trades are counted from the transaction source (by side),
/// and their journal twins are de-duplicated away so a trade is counted once.
/// Unmapped flows are dropped.
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub async fn monthly_activity(db: &Database, scope: BudgetScope, month: &str) -> HashMap<i64, f64> {
  let context = ResolutionContext::load(db, scope).await;

  let mut journal_rows: Vec<JournalActivity> = Vec::new();
  let mut transactions: Vec<TransactionActivity> = Vec::new();
  for character_id in scope_character_ids(db, scope).await {
    for row in finance::wallet_journal(db, character_id).await.unwrap_or_default() {
      journal_rows.push(JournalActivity {
        amount: row.amount(),
        context_id: row.context_id(),
        context_id_type: row.context_id_type().clone(),
        date: row.date().clone(),
        id: row.id(),
        ref_type: row.ref_type().clone(),
      });
    }
    for tx in finance::wallet_transactions(db, character_id).await.unwrap_or_default() {
      transactions.push(TransactionActivity {
        amount: tx.unit_price() * tx.quantity() as f64 * if tx.is_buy() { -1.0 } else { 1.0 },
        date: tx.date().clone(),
        is_buy: tx.is_buy(),
        transaction_id: tx.transaction_id(),
      });
    }
  }
  for corp in scope_corporation_ids(db, scope).await {
    for division in finance::divisions(db, corp).await.unwrap_or_default() {
      for row in finance::corporation_wallet_journal(db, corp, division.division())
        .await
        .unwrap_or_default()
      {
        journal_rows.push(JournalActivity {
          amount: row.amount(),
          context_id: row.context_id(),
          context_id_type: row.context_id_type().clone(),
          date: row.date().clone(),
          id: row.id(),
          ref_type: row.ref_type().clone(),
        });
      }
      for tx in finance::corporation_wallet_transactions(db, corp, division.division())
        .await
        .unwrap_or_default()
      {
        transactions.push(TransactionActivity {
          amount: tx.unit_price() * tx.quantity() as f64 * if tx.is_buy() { -1.0 } else { 1.0 },
          date: tx.date().clone(),
          is_buy: tx.is_buy(),
          transaction_id: tx.transaction_id(),
        });
      }
    }
  }

  // EVE wallet ref ids — journal `id` and `transaction_id` — are globally unique
  // across the cluster, so a flat id set never collides between two characters in
  // an All scope; a journal twin's context_id matches only its own transaction.
  let ingested: HashSet<i64> = transactions
    .iter()
    .filter(|tx| month_key(&tx.date).as_deref() == Some(month))
    .map(|tx| tx.transaction_id)
    .collect();

  let journal_in_month = journal_rows
    .iter()
    .filter(|row| month_key(&row.date).as_deref() == Some(month))
    .filter(|row| !is_market_twin(row, &ingested))
    .map(|row| (row.id, row.ref_type.as_str(), row.amount));

  let mut by_category = aggregate_activity(journal_in_month, |id, ref_type| {
    context.resolve(BudgetEntryKind::Journal, id, Some(ref_type), None)
  });

  for tx in &transactions {
    if month_key(&tx.date).as_deref() != Some(month) {
      continue;
    }
    if let Some(category_id) = context.resolve(BudgetEntryKind::Market, tx.transaction_id, None, Some(tx.is_buy)) {
      *by_category.entry(category_id).or_insert(0.0) += tx.amount;
    }
  }

  by_category
}

fn epoch_day(date: &str) -> Option<i64> {
  use chrono::Datelike;
  let head = date.get(..10)?;
  let parsed = chrono::NaiveDate::parse_from_str(head, "%Y-%m-%d").ok()?;
  Some(i64::from(parsed.num_days_from_ce()))
}

/// Self-contained mirror of the wallet view-model's `shift_month` so the B2 math stays independent of the UI layer.
fn shift_month_key(month: &str, delta: i32) -> String {
  let Some((year, mon)) = month.split_once('-') else {
    return month.to_owned();
  };
  let (Ok(year), Ok(mon)) = (year.parse::<i32>(), mon.parse::<i32>()) else {
    return month.to_owned();
  };
  if !(1..=12).contains(&mon) {
    return month.to_owned();
  }
  let zero_based = (year * 12 + (mon - 1)) + delta;
  format!("{:04}-{:02}", zero_based.div_euclid(12), zero_based.rem_euclid(12) + 1)
}

/// The FIFO age-of-ISK, by month, for a scope's signed journal flows.
///
/// Models the wallet as a FIFO queue of timestamped ISK lots: every positive
/// flow enqueues a lot dated on its journal day; every negative flow (a spend)
/// consumes ISK from the oldest lot first. The age of each spent unit of ISK is
/// the number of days between the lot it came from and the spend. A month's
/// age-of-ISK is the ISK-quantity-weighted mean age (in days) of all ISK spent
/// in that month — directly "how long ISK sat before being spent".
///
/// `flows` are `(date, signed amount)` in any order; they are sorted by day
/// here. Spends with no ISK left to draw on (the queue is empty) contribute no
/// age. Returns a map of month key (`YYYY-MM`) to weighted-mean spend age.
fn fifo_ages_by_month<'a>(flows: impl IntoIterator<Item = (&'a str, f64)>) -> HashMap<String, f64> {
  let mut dated: Vec<(i64, String, f64)> = flows
    .into_iter()
    .filter_map(|(date, amount)| {
      let day = epoch_day(date)?;
      let month = month_key(date)?;
      (amount != 0.0).then_some((day, month, amount))
    })
    .collect();
  dated.sort_by_key(|&(day, _, _)| day);

  let mut lots: std::collections::VecDeque<(i64, f64)> = std::collections::VecDeque::new();
  let mut weighted_age: HashMap<String, f64> = HashMap::new();
  let mut spent_isk: HashMap<String, f64> = HashMap::new();

  for (day, month, amount) in dated {
    if amount > 0.0 {
      lots.push_back((day, amount));
      continue;
    }
    let mut remaining = -amount;
    while remaining > 0.0 {
      let Some(&(lot_day, lot_amount)) = lots.front() else {
        break;
      };
      let drawn = remaining.min(lot_amount);
      let age = (day - lot_day).max(0) as f64;
      *weighted_age.entry(month.clone()).or_insert(0.0) += age * drawn;
      *spent_isk.entry(month.clone()).or_insert(0.0) += drawn;
      remaining -= drawn;
      if drawn >= lot_amount {
        lots.pop_front();
      } else {
        lots.front_mut().expect("front exists").1 -= drawn;
      }
    }
  }

  weighted_age
    .into_iter()
    .filter_map(|(month, total_age)| {
      let isk = spent_isk.get(&month).copied().unwrap_or(0.0);
      (isk > 0.0).then(|| (month, total_age / isk))
    })
    .collect()
}

/// The scope's signed journal flows as `(date, amount)`, unioning every in-scope
/// character journal and covered corp-division journal. Drives the trailing
/// history and FIFO age-of-ISK without re-querying per month.
async fn scope_journal_flows(db: &Database, scope: BudgetScope) -> Vec<(String, f64)> {
  let mut flows: Vec<(String, f64)> = Vec::new();
  for character_id in scope_character_ids(db, scope).await {
    for row in finance::wallet_journal(db, character_id).await.unwrap_or_default() {
      if let Some(amount) = row.amount() {
        flows.push((row.date().clone(), amount));
      }
    }
  }
  for corp in scope_corporation_ids(db, scope).await {
    for division in finance::divisions(db, corp).await.unwrap_or_default() {
      for row in finance::corporation_wallet_journal(db, corp, division.division())
        .await
        .unwrap_or_default()
      {
        if let Some(amount) = row.amount() {
          flows.push((row.date().clone(), amount));
        }
      }
    }
  }
  flows
}

/// Months with no data are still emitted as zeros so the Reflect charts always have a full series.
// Budget reporting history (B4); consumed by the Budget Reflect UI. Exercised by unit tests.
#[allow(dead_code)]
pub async fn monthly_history(db: &Database, scope: BudgetScope, month: &str, months: usize) -> Vec<MonthFlow> {
  let ages = fifo_ages_by_month(
    scope_journal_flows(db, scope)
      .await
      .iter()
      .map(|(d, a)| (d.as_str(), *a)),
  );

  let mut out: Vec<MonthFlow> = Vec::with_capacity(months);
  for step in (0..months as i32).rev() {
    let key = shift_month_key(month, -step);
    let activity = monthly_activity(db, scope, &key).await;
    let income = activity.values().filter(|&&v| v > 0.0).sum::<f64>();
    let spend = activity.values().filter(|&&v| v < 0.0).map(|v| -v).sum::<f64>();
    let assigned = month_assigned(db, scope, &key).await;
    out.push(MonthFlow {
      age: ages.get(&key).copied().unwrap_or(0.0),
      assigned,
      income,
      month: key.clone(),
      spend,
    });
  }
  out
}

async fn month_assigned(db: &Database, scope: BudgetScope, month: &str) -> f64 {
  use crate::store::repo::budget;
  let mut total = 0.0;
  for group in budget::list_groups(db, scope).await.unwrap_or_default() {
    for category in budget::list_categories(db, group.id()).await.unwrap_or_default() {
      let assignment = budget::list_assignments(db, category.id())
        .await
        .unwrap_or_default()
        .into_iter()
        .find(|a| a.month() == month)
        .map_or(0.0, |a| a.assigned());
      total += assignment;
    }
  }
  total
}

#[cfg(test)]
mod tests {
  use super::*;

  mod month_key {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_slices_the_utc_month_from_an_rfc3339_timestamp() {
      assert_eq!(month_key("2026-06-18T12:34:56Z").as_deref(), Some("2026-06"));
    }

    #[test]
    fn it_accepts_a_bare_date() {
      assert_eq!(month_key("2026-01-01").as_deref(), Some("2026-01"));
    }

    #[test]
    fn it_rejects_a_malformed_date() {
      assert_eq!(month_key("not-a-date"), None);
      assert_eq!(month_key("2026/06/18"), None);
      assert_eq!(month_key("206-6"), None);
    }
  }

  mod carry_from {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_carries_a_positive_available_forward() {
      assert_eq!(carry_from(Some(150.0)), 150.0);
    }

    #[test]
    fn it_does_not_carry_a_negative_available_as_positive() {
      assert_eq!(carry_from(Some(-90.0)), 0.0);
    }

    #[test]
    fn it_carries_zero_when_there_is_no_prior_month() {
      assert_eq!(carry_from(None), 0.0);
    }
  }

  mod roll_carry {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_rolls_positive_available_across_three_months() {
      // Month 1: 0 carry + 100 assigned − 40 spend = 60 available.
      // Month 2: 60 carry + 100 assigned − 40 spend = 120 available.
      // Month 3: 120 carry + 0 assigned − 20 spend = 100 available.
      let months = roll_carry(0.0, &[(100.0, -40.0), (100.0, -40.0), (0.0, -20.0)]);

      assert_eq!(months[0].carry, 0.0);
      assert_eq!(months[0].available(), 60.0);
      assert_eq!(months[1].carry, 60.0);
      assert_eq!(months[1].available(), 120.0);
      assert_eq!(months[2].carry, 120.0);
      assert_eq!(months[2].available(), 100.0);
    }

    #[test]
    fn it_does_not_carry_a_negative_available_into_the_next_month() {
      // Month 1: 0 + 50 − 200 = −150 available (overspent) → carries 0, not −150.
      // Month 2: 0 carry + 100 assigned + 0 = 100 available.
      let months = roll_carry(0.0, &[(50.0, -200.0), (100.0, 0.0)]);

      assert_eq!(months[0].available(), -150.0);
      assert_eq!(months[1].carry, 0.0);
      assert_eq!(months[1].available(), 100.0);
    }

    #[test]
    fn it_starts_from_a_seed_carry_for_an_existing_balance() {
      let months = roll_carry(500.0, &[(0.0, -100.0)]);

      assert_eq!(months[0].carry, 500.0);
      assert_eq!(months[0].available(), 400.0);
    }

    #[test]
    fn it_treats_a_gap_month_as_the_previous_present_month_rolling_forward() {
      // A series with only the months that have data (Jan, then Mar; Feb is a
      // gap and simply absent). Mar carries Jan's positive available.
      let months = roll_carry(0.0, &[(80.0, 0.0), (0.0, -30.0)]);

      assert_eq!(months[0].available(), 80.0);
      assert_eq!(months[1].carry, 80.0);
      assert_eq!(months[1].available(), 50.0);
    }
  }

  mod category_for_ref_type {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_a_default_through_its_slug() {
      let slug_to_id = HashMap::from([("income", 7)]);
      let overrides = HashMap::new();

      assert_eq!(category_for_ref_type("bounty_prizes", &overrides, &slug_to_id), Some(7));
    }

    #[test]
    fn it_lets_a_user_override_win_over_the_default() {
      let slug_to_id = HashMap::from([("income", 7)]);
      let overrides = HashMap::from([("bounty_prizes".to_owned(), 99)]);

      assert_eq!(
        category_for_ref_type("bounty_prizes", &overrides, &slug_to_id),
        Some(99)
      );
    }

    #[test]
    fn it_maps_an_override_for_a_ref_type_with_no_default() {
      let slug_to_id = HashMap::new();
      let overrides = HashMap::from([("some_exotic_ref".to_owned(), 12)]);

      assert_eq!(
        category_for_ref_type("some_exotic_ref", &overrides, &slug_to_id),
        Some(12)
      );
    }

    #[test]
    fn it_returns_none_for_an_unmapped_ref_type() {
      let slug_to_id = HashMap::from([("income", 7)]);
      let overrides = HashMap::new();

      assert_eq!(
        category_for_ref_type("never_heard_of_it", &overrides, &slug_to_id),
        None
      );
    }

    #[test]
    fn it_returns_none_when_the_default_slug_was_not_seeded() {
      // The ref_type has a default slug, but that category does not exist in
      // this scope (e.g. the user deleted it).
      let slug_to_id = HashMap::new();
      let overrides = HashMap::new();

      assert_eq!(category_for_ref_type("bounty_prizes", &overrides, &slug_to_id), None);
    }
  }

  mod aggregate_activity {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_signed_amounts_per_mapped_category() {
      let entries = vec![
        (1, "bounty_prizes", Some(1_000.0)),
        (2, "bounty_prizes", Some(500.0)),
        (3, "brokers_fee", Some(-120.0)),
      ];
      let resolve = |_id: i64, ref_type: &str| match ref_type {
        "bounty_prizes" => Some(1),
        "brokers_fee" => Some(2),
        _ => None,
      };

      let by_category = aggregate_activity(entries, resolve);

      assert_eq!(by_category.get(&1), Some(&1_500.0));
      assert_eq!(by_category.get(&2), Some(&-120.0));
    }

    #[test]
    fn it_lets_the_entry_id_steer_resolution() {
      let entries = vec![(1, "bounty_prizes", Some(1_000.0)), (2, "bounty_prizes", Some(500.0))];
      // Entry 2 is pinned to a different category despite the same ref_type.
      let resolve = |id: i64, _ref_type: &str| if id == 2 { Some(9) } else { Some(1) };

      let by_category = aggregate_activity(entries, resolve);

      assert_eq!(by_category.get(&1), Some(&1_000.0));
      assert_eq!(by_category.get(&9), Some(&500.0));
    }

    #[test]
    fn it_skips_entries_with_no_amount_or_no_mapping() {
      let entries = vec![
        (1, "bounty_prizes", None),
        (2, "unmapped_ref", Some(999.0)),
        (3, "bounty_prizes", Some(10.0)),
      ];
      let resolve = |_id: i64, ref_type: &str| (ref_type == "bounty_prizes").then_some(1);

      let by_category = aggregate_activity(entries, resolve);

      assert_eq!(by_category.len(), 1);
      assert_eq!(by_category.get(&1), Some(&10.0));
    }
  }

  mod pool_summary {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_derives_ready_to_assign_as_pool_minus_total_available() {
      let summary = pool_summary(1_000.0, [300.0, 200.0, 100.0]);

      assert_eq!(summary.pool, 1_000.0);
      assert_eq!(summary.ready_to_assign, 400.0);
      assert_eq!(summary.overspent, 0.0);
    }

    #[test]
    fn it_sums_only_negative_availables_into_overspent() {
      let summary = pool_summary(500.0, [300.0, -150.0, -50.0]);

      assert_eq!(summary.overspent, -200.0);
      // ready = 500 − (300 − 150 − 50) = 500 − 100 = 400.
      assert_eq!(summary.ready_to_assign, 400.0);
    }

    #[test]
    fn it_can_report_a_negative_ready_to_assign_when_over_assigned() {
      let summary = pool_summary(100.0, [80.0, 80.0]);

      assert_eq!(summary.ready_to_assign, -60.0);
    }
  }

  mod default_ref_type_slugs {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_the_documented_ref_types_onto_the_expected_slugs() {
      let map = default_ref_type_slugs();

      assert_eq!(map.get("bounty_prizes"), Some(&"income"));
      assert_eq!(map.get("market_transaction"), Some(&"trading"));
      assert_eq!(map.get("brokers_fee"), Some(&"fees"));
      assert_eq!(map.get("transaction_tax"), Some(&"fees"));
      assert_eq!(map.get("industry_job_tax"), Some(&"tithe"));
      assert_eq!(map.get("contract_price_payment_corp"), Some(&"contracts"));
      assert_eq!(map.get("player_donation"), Some(&"transfers"));
      assert_eq!(map.get("manufacturing"), Some(&"industry"));
    }

    #[test]
    fn it_only_targets_slugs_that_the_seed_groups_define() {
      let defined: std::collections::HashSet<&str> = SEED_GROUPS
        .iter()
        .flat_map(|group| group.cats.iter())
        .map(|cat| cat.slug)
        .collect();

      for (ref_type, slug) in DEFAULT_REF_TYPE_MAP {
        assert!(
          defined.contains(slug),
          "{ref_type} targets slug {slug} which no seed group defines"
        );
      }
    }
  }

  mod seed_scope {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, repo::budget};

    #[tokio::test]
    async fn it_seeds_starter_groups_categories_and_the_default_ref_type_map() {
      let db = store::open_test().await.unwrap();

      seed_scope(&db, BudgetScope::All).await.unwrap();

      let groups = budget::list_groups(&db, BudgetScope::All).await.unwrap();
      assert_eq!(groups.len(), SEED_GROUPS.len());

      let maps = budget::list_ref_type_maps(&db, BudgetScope::All).await.unwrap();
      assert_eq!(maps.len(), DEFAULT_REF_TYPE_MAP.len());

      // Every default ref_type resolves to a real, seeded category.
      let slug_to_id = slug_to_category_id(&db, BudgetScope::All).await;
      let overrides = HashMap::new();
      for (ref_type, _) in DEFAULT_REF_TYPE_MAP {
        assert!(
          category_for_ref_type(ref_type, &overrides, &slug_to_id).is_some(),
          "{ref_type} did not resolve after seeding"
        );
      }
    }

    #[tokio::test]
    async fn it_is_idempotent() {
      let db = store::open_test().await.unwrap();

      seed_scope(&db, BudgetScope::All).await.unwrap();
      seed_scope(&db, BudgetScope::All).await.unwrap();

      let groups = budget::list_groups(&db, BudgetScope::All).await.unwrap();
      let maps = budget::list_ref_type_maps(&db, BudgetScope::All).await.unwrap();

      assert_eq!(groups.len(), SEED_GROUPS.len());
      assert_eq!(maps.len(), DEFAULT_REF_TYPE_MAP.len());
    }

    #[tokio::test]
    async fn it_does_not_resurrect_a_deleted_starter_category() {
      let db = store::open_test().await.unwrap();
      seed_scope(&db, BudgetScope::All).await.unwrap();
      let groups = budget::list_groups(&db, BudgetScope::All).await.unwrap();
      budget::delete_group(&db, groups[0].id()).await.unwrap();

      seed_scope(&db, BudgetScope::All).await.unwrap();

      let after = budget::list_groups(&db, BudgetScope::All).await.unwrap();
      assert_eq!(after.len(), SEED_GROUPS.len() - 1);
    }

    #[tokio::test]
    async fn it_isolates_seeds_per_scope() {
      let db = store::open_test().await.unwrap();

      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();

      assert!(budget::list_groups(&db, BudgetScope::All).await.unwrap().is_empty());
      assert_eq!(
        budget::list_groups(&db, BudgetScope::Character(1)).await.unwrap().len(),
        SEED_GROUPS.len()
      );
    }
  }

  mod assign_entry {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, repo::budget};

    #[tokio::test]
    async fn it_lazy_seeds_an_unseeded_scope_on_first_assignment() {
      // A fresh DB seeds a scope's categories in a deterministic order, so the
      // income category id discovered on a probe DB is valid on a second fresh DB
      // where `assign_entry` performs the lazy seed itself.
      let probe = store::open_test().await.unwrap();
      seed_scope(&probe, BudgetScope::Character(1)).await.unwrap();
      let income_id = slug_to_category_id(&probe, BudgetScope::Character(1)).await["income"];

      let db = store::open_test().await.unwrap();
      assert!(
        budget::list_groups(&db, BudgetScope::Character(1))
          .await
          .unwrap()
          .is_empty()
      );

      let saved = assign_entry(&db, BudgetScope::Character(1), BudgetEntryKind::Journal, 5, income_id)
        .await
        .unwrap();

      assert!(
        !budget::list_groups(&db, BudgetScope::Character(1))
          .await
          .unwrap()
          .is_empty()
      );
      assert_eq!(saved.category_id(), income_id);
      assert_eq!(
        resolve_entry_category(
          &db,
          BudgetScope::Character(1),
          BudgetEntryKind::Journal,
          5,
          Some("manufacturing"),
          None
        )
        .await,
        Some(income_id)
      );
    }
  }

  mod resolve_entry_category {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, repo::budget};

    #[tokio::test]
    async fn it_prefers_a_per_entry_override_over_the_ref_type_default() {
      let db = store::open_test().await.unwrap();
      let scope = BudgetScope::All;
      seed_scope(&db, scope).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, scope).await;

      // `manufacturing` defaults to the industry envelope; override entry 5 to income.
      assign_entry(&db, scope, BudgetEntryKind::Journal, 5, slug_to_id["income"])
        .await
        .unwrap();

      assert_eq!(
        resolve_entry_category(&db, scope, BudgetEntryKind::Journal, 5, Some("manufacturing"), None).await,
        Some(slug_to_id["income"])
      );
    }

    #[tokio::test]
    async fn it_falls_back_to_the_ref_type_map_when_there_is_no_override() {
      let db = store::open_test().await.unwrap();
      let scope = BudgetScope::All;
      seed_scope(&db, scope).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, scope).await;

      // `bounty_prizes` seeds to income; remap the ref_type to fees with no per-entry override.
      budget::upsert_ref_type_map(&db, scope, "bounty_prizes", slug_to_id["fees"])
        .await
        .unwrap();

      assert_eq!(
        resolve_entry_category(&db, scope, BudgetEntryKind::Journal, 9, Some("bounty_prizes"), None).await,
        Some(slug_to_id["fees"])
      );
    }

    #[tokio::test]
    async fn it_falls_back_to_the_seed_default() {
      let db = store::open_test().await.unwrap();
      let scope = BudgetScope::All;
      seed_scope(&db, scope).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, scope).await;

      assert_eq!(
        resolve_entry_category(&db, scope, BudgetEntryKind::Journal, 1, Some("bounty_prizes"), None).await,
        Some(slug_to_id["income"])
      );
    }

    #[tokio::test]
    async fn it_resolves_a_market_entry_by_side() {
      let db = store::open_test().await.unwrap();
      let scope = BudgetScope::All;
      seed_scope(&db, scope).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, scope).await;

      assert_eq!(
        resolve_entry_category(&db, scope, BudgetEntryKind::Market, 10, None, Some(true)).await,
        Some(slug_to_id["trading"])
      );
      assert_eq!(
        resolve_entry_category(&db, scope, BudgetEntryKind::Market, 11, None, Some(false)).await,
        Some(slug_to_id["income"])
      );
    }

    #[tokio::test]
    async fn it_returns_none_on_an_unseeded_scope() {
      let db = store::open_test().await.unwrap();

      assert_eq!(
        resolve_entry_category(
          &db,
          BudgetScope::All,
          BudgetEntryKind::Journal,
          1,
          Some("bounty_prizes"),
          None
        )
        .await,
        None
      );
    }
  }

  mod monthly_activity {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{
        Alliance, Bloodline, Character, CharacterWalletTransaction, Corporation, CorporationWalletJournal,
        CorporationWalletTransaction, Gender, Race,
      },
      repo::{budget, character::insert_with_org, finance},
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

    fn journal(
      id: i64,
      character_id: i64,
      ref_type: &str,
      amount: f64,
      date: &str,
    ) -> store::model::CharacterWalletJournal {
      store::model::CharacterWalletJournal {
        amount: Some(amount),
        balance: Some(amount),
        character_id,
        context_id: None,
        context_id_type: None,
        date: date.to_owned(),
        description: "Entry".to_owned(),
        first_party_id: None,
        id,
        reason: None,
        ref_type: ref_type.to_owned(),
        second_party_id: None,
        tax: None,
        tax_receiver_id: None,
      }
    }

    fn linked_journal(
      id: i64,
      character_id: i64,
      ref_type: &str,
      amount: f64,
      transaction_id: i64,
      date: &str,
    ) -> store::model::CharacterWalletJournal {
      let mut entry = journal(id, character_id, ref_type, amount, date);
      entry.context_id = Some(transaction_id);
      entry.context_id_type = Some("market_transaction_id".to_owned());
      entry
    }

    fn transaction(
      transaction_id: i64,
      character_id: i64,
      is_buy: bool,
      unit_price: f64,
      quantity: i64,
      date: &str,
    ) -> CharacterWalletTransaction {
      CharacterWalletTransaction {
        character_id,
        client_id: 1_000_035,
        date: date.to_owned(),
        is_buy,
        is_personal: true,
        journal_ref_id: 0,
        location_id: 60_003_760,
        quantity,
        transaction_id,
        type_id: 34,
        unit_price,
      }
    }

    #[tokio::test]
    async fn it_sums_journal_amounts_by_mapped_category_for_the_month() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      finance::append_wallet_journal(
        &db,
        &[
          journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z"),
          journal(2, 1, "bounty_prizes", 500.0, "2026-06-20T00:00:00Z"),
          journal(3, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z"),
          // Out of the month — must be excluded.
          journal(4, 1, "bounty_prizes", 9_999.0, "2026-05-31T23:59:59Z"),
        ],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;

      assert_eq!(activity.get(&slug_to_id["income"]), Some(&1_500.0));
      assert_eq!(activity.get(&slug_to_id["fees"]), Some(&-120.0));
    }

    #[tokio::test]
    async fn it_lets_a_user_override_reroute_a_ref_type() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      // Re-route bounties from income to the trading category.
      budget::upsert_ref_type_map(&db, BudgetScope::Character(1), "bounty_prizes", slug_to_id["trading"])
        .await
        .unwrap();
      finance::append_wallet_journal(&db, &[journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z")])
        .await
        .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["income"]), None);
      assert_eq!(activity.get(&slug_to_id["trading"]), Some(&1_000.0));
    }

    #[tokio::test]
    async fn it_includes_corp_division_journals_for_a_corp_scope() {
      let db = store::open_test().await.unwrap();
      let corp_id = 98_000_001;
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSTC");
      corp.set_ceo_id(100);
      corp.set_creator_id(100);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      store::repo::org::upsert_corporation(&db, &corp).await.unwrap();
      finance::upsert_divisions(
        &db,
        &[store::model::CorporationWalletDivision {
          balance: Some(0.0),
          corporation_id: corp_id,
          division: 1,
          name: Some("Master".to_owned()),
        }],
      )
      .await
      .unwrap();
      seed_scope(&db, BudgetScope::Corporation(corp_id)).await.unwrap();
      finance::append_corporation_wallet_journal(
        &db,
        &[CorporationWalletJournal {
          amount: Some(-2_000.0),
          balance: Some(0.0),
          context_id: None,
          context_id_type: None,
          corporation_id: corp_id,
          date: "2026-06-10T00:00:00Z".to_owned(),
          description: "Tax".to_owned(),
          division: 1,
          first_party_id: None,
          id: 1,
          reason: None,
          ref_type: "industry_job_tax".to_owned(),
          second_party_id: None,
          tax: None,
          tax_receiver_id: None,
        }],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Corporation(corp_id), "2026-06").await;
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Corporation(corp_id)).await;

      assert_eq!(activity.get(&slug_to_id["tithe"]), Some(&-2_000.0));
    }

    #[tokio::test]
    async fn it_lets_a_per_entry_override_win_over_the_ref_type_default() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      finance::append_wallet_journal(&db, &[journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z")])
        .await
        .unwrap();
      // Bounties default to income; pin this one entry to trading.
      assign_entry(
        &db,
        BudgetScope::Character(1),
        BudgetEntryKind::Journal,
        1,
        slug_to_id["trading"],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["income"]), None);
      assert_eq!(activity.get(&slug_to_id["trading"]), Some(&1_000.0));
    }

    #[tokio::test]
    async fn it_counts_a_market_trade_once_and_keeps_its_fees() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      // A sale: a transaction row, its journal twin, and a separate broker fee.
      finance::append_wallet_transaction(&db, &[transaction(500, 1, false, 100.0, 10, "2026-06-05T00:00:00Z")])
        .await
        .unwrap();
      finance::append_wallet_journal(
        &db,
        &[
          linked_journal(10, 1, "market_transaction", 1_000.0, 500, "2026-06-05T00:00:00Z"),
          linked_journal(11, 1, "brokers_fee", -50.0, 500, "2026-06-05T00:00:00Z"),
        ],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["income"]), Some(&1_000.0));
      assert_eq!(activity.get(&slug_to_id["fees"]), Some(&-50.0));
      assert_eq!(activity.get(&slug_to_id["trading"]), None);
    }

    #[tokio::test]
    async fn it_lets_a_per_entry_override_win_for_a_market_trade() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      // A buy defaults to trading; pin this trade to fees.
      finance::append_wallet_transaction(&db, &[transaction(600, 1, true, 100.0, 5, "2026-06-07T00:00:00Z")])
        .await
        .unwrap();
      assign_entry(
        &db,
        BudgetScope::Character(1),
        BudgetEntryKind::Market,
        600,
        slug_to_id["fees"],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["fees"]), Some(&-500.0));
      assert_eq!(activity.get(&slug_to_id["trading"]), None);
    }

    #[tokio::test]
    async fn it_ingests_corp_transactions_and_dedups_the_journal_twin() {
      let db = store::open_test().await.unwrap();
      let corp_id = 98_000_002;
      let mut corp = Corporation::new(corp_id, "Trade Corp", "TRDC");
      corp.set_ceo_id(100);
      corp.set_creator_id(100);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      store::repo::org::upsert_corporation(&db, &corp).await.unwrap();
      finance::upsert_divisions(
        &db,
        &[store::model::CorporationWalletDivision {
          balance: Some(0.0),
          corporation_id: corp_id,
          division: 1,
          name: Some("Master".to_owned()),
        }],
      )
      .await
      .unwrap();
      seed_scope(&db, BudgetScope::Corporation(corp_id)).await.unwrap();
      finance::append_corporation_wallet_transaction(
        &db,
        &[CorporationWalletTransaction {
          client_id: 1_000_035,
          corporation_id: corp_id,
          date: "2026-06-09T00:00:00Z".to_owned(),
          division: 1,
          is_buy: false,
          journal_ref_id: 0,
          location_id: 60_003_760,
          quantity: 4,
          transaction_id: 700,
          type_id: 34,
          unit_price: 250.0,
        }],
      )
      .await
      .unwrap();
      finance::append_corporation_wallet_journal(
        &db,
        &[CorporationWalletJournal {
          amount: Some(1_000.0),
          balance: Some(0.0),
          context_id: Some(700),
          context_id_type: Some("market_transaction_id".to_owned()),
          corporation_id: corp_id,
          date: "2026-06-09T00:00:00Z".to_owned(),
          description: "Sale".to_owned(),
          division: 1,
          first_party_id: None,
          id: 20,
          reason: None,
          ref_type: "market_transaction".to_owned(),
          second_party_id: None,
          tax: None,
          tax_receiver_id: None,
        }],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Corporation(corp_id), "2026-06").await;
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Corporation(corp_id)).await;

      assert_eq!(activity.get(&slug_to_id["income"]), Some(&1_000.0));
      assert_eq!(activity.get(&slug_to_id["trading"]), None);
    }
  }

  mod budgetable_pool {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
      repo::{character::insert_with_org, finance, infra, org},
    };

    async fn seed_character_with_liquid(db: &Database, id: i64, liquid: f64) {
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
      // A character credential is what marks the pilot "owned" for the All scope.
      infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9_999, None, None)
        .await
        .unwrap();
      // The character's liquid figure is the latest journal balance.
      finance::append_wallet_journal(
        db,
        &[store::model::CharacterWalletJournal {
          amount: Some(liquid),
          balance: Some(liquid),
          character_id: id,
          context_id: None,
          context_id_type: None,
          date: "2026-06-18T00:00:00Z".to_owned(),
          description: "Seed".to_owned(),
          first_party_id: None,
          id,
          reason: None,
          ref_type: "player_donation".to_owned(),
          second_party_id: None,
          tax: None,
          tax_receiver_id: None,
        }],
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_sums_character_liquid_for_a_character_scope() {
      let db = store::open_test().await.unwrap();
      seed_character_with_liquid(&db, 1, 5_000.0).await;

      let pool = budgetable_pool(&db, BudgetScope::Character(1)).await;

      assert_eq!(pool, 5_000.0);
    }

    #[tokio::test]
    async fn it_sums_corp_division_balances_for_a_corp_scope() {
      let db = store::open_test().await.unwrap();
      let corp_id = 98_000_001;
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSTC");
      corp.set_ceo_id(100);
      corp.set_creator_id(100);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      org::upsert_corporation(&db, &corp).await.unwrap();
      finance::upsert_divisions(
        &db,
        &[
          store::model::CorporationWalletDivision {
            balance: Some(3_000.0),
            corporation_id: corp_id,
            division: 1,
            name: Some("Master".to_owned()),
          },
          store::model::CorporationWalletDivision {
            balance: Some(2_000.0),
            corporation_id: corp_id,
            division: 2,
            name: Some("Second".to_owned()),
          },
        ],
      )
      .await
      .unwrap();

      let pool = budgetable_pool(&db, BudgetScope::Corporation(corp_id)).await;

      assert_eq!(pool, 5_000.0);
    }

    #[tokio::test]
    async fn it_sums_characters_and_corps_for_the_all_scope() {
      let db = store::open_test().await.unwrap();
      seed_character_with_liquid(&db, 1, 5_000.0).await;
      let corp_id = 98_000_001;
      let mut corp = Corporation::new(corp_id, "Owned Corp", "OWN");
      corp.set_ceo_id(1);
      corp.set_creator_id(1);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      org::upsert_corporation(&db, &corp).await.unwrap();
      // A corp is "owned" when it holds a corporation credential authorized by a member.
      infra::upsert(&db, corp_id, OwnerType::Corporation, "tok", "rt", 9_999, Some(1), None)
        .await
        .unwrap();
      finance::upsert_divisions(
        &db,
        &[store::model::CorporationWalletDivision {
          balance: Some(1_000.0),
          corporation_id: corp_id,
          division: 1,
          name: Some("Master".to_owned()),
        }],
      )
      .await
      .unwrap();

      let pool = budgetable_pool(&db, BudgetScope::All).await;

      // 5_000 character liquid + 1_000 owned-corp division balance.
      assert_eq!(pool, 6_000.0);
    }
  }

  mod shift_month_key {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_steps_across_year_boundaries() {
      assert_eq!(shift_month_key("2026-01", -1), "2025-12");
      assert_eq!(shift_month_key("2026-12", 1), "2027-01");
    }

    #[test]
    fn it_returns_an_unparseable_key_unchanged() {
      assert_eq!(shift_month_key("nope", -1), "nope");
    }
  }

  mod epoch_day {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_counts_whole_days_between_two_dates() {
      let a = epoch_day("2026-06-01T08:00:00Z").unwrap();
      let b = epoch_day("2026-06-08T23:59:59Z").unwrap();

      assert_eq!(b - a, 7);
    }

    #[test]
    fn it_rejects_a_malformed_date() {
      assert_eq!(epoch_day("not-a-date"), None);
    }
  }

  mod fifo_ages_by_month {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_ages_spent_isk_against_the_oldest_lot_first() {
      // Deposit 100 on Jan 1, then spend 100 on Jan 11 → every ISK is 10 days old.
      let ages = fifo_ages_by_month([("2026-01-01T00:00:00Z", 100.0), ("2026-01-11T00:00:00Z", -100.0)]);

      assert_eq!(ages.get("2026-01"), Some(&10.0));
    }

    #[test]
    fn it_weights_age_by_isk_drawn_from_each_lot() {
      // Two lots: 100 on day 0, 100 on day 10. Spend 150 on day 20:
      //   100 from the day-0 lot (age 20) + 50 from the day-10 lot (age 10).
      //   weighted mean = (100*20 + 50*10) / 150 = 2500/150 ≈ 16.67 days.
      let ages = fifo_ages_by_month([
        ("2026-03-01T00:00:00Z", 100.0),
        ("2026-03-11T00:00:00Z", 100.0),
        ("2026-03-21T00:00:00Z", -150.0),
      ]);

      let age = ages.get("2026-03").copied().unwrap();
      assert!((age - 2_500.0 / 150.0).abs() < 1e-9, "age was {age}");
    }

    #[test]
    fn it_records_no_age_for_a_month_with_no_spend() {
      let ages = fifo_ages_by_month([("2026-04-01T00:00:00Z", 100.0)]);

      assert_eq!(ages.get("2026-04"), None);
    }

    #[test]
    fn it_only_ages_isk_it_can_draw_from_the_queue() {
      // Spend with an empty queue contributes nothing (no negative ages).
      let ages = fifo_ages_by_month([("2026-05-05T00:00:00Z", -100.0)]);

      assert_eq!(ages.get("2026-05"), None);
    }

    #[test]
    fn it_sorts_flows_chronologically_before_aging() {
      // Feed the spend before the deposit: the function must sort by day first,
      // so the deposit on day 0 still ages by 10 days at the day-10 spend.
      let ages = fifo_ages_by_month([("2026-06-11T00:00:00Z", -100.0), ("2026-06-01T00:00:00Z", 100.0)]);

      assert_eq!(ages.get("2026-06"), Some(&10.0));
    }
  }

  mod monthly_history {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{Alliance, Bloodline, Character, Gender, Race},
      repo::{character::insert_with_org, finance},
    };

    async fn seed_character(db: &Database, id: i64) {
      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = store::model::Corporation::new(corp_id, "Test Corp", "TSC");
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

    fn journal(id: i64, ref_type: &str, amount: f64, date: &str) -> store::model::CharacterWalletJournal {
      store::model::CharacterWalletJournal {
        amount: Some(amount),
        balance: Some(amount),
        character_id: 1,
        context_id: None,
        context_id_type: None,
        date: date.to_owned(),
        description: "Entry".to_owned(),
        first_party_id: None,
        id,
        reason: None,
        ref_type: ref_type.to_owned(),
        second_party_id: None,
        tax: None,
        tax_receiver_id: None,
      }
    }

    #[tokio::test]
    async fn it_emits_one_entry_per_trailing_month_oldest_first() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();

      let history = monthly_history(&db, BudgetScope::Character(1), "2026-06", 3).await;

      assert_eq!(history.len(), 3);
      assert_eq!(history[0].month, "2026-04");
      assert_eq!(history[2].month, "2026-06");
    }

    #[tokio::test]
    async fn it_splits_income_and_spend_and_ages_the_spend() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      finance::append_wallet_journal(
        &db,
        &[
          journal(1, "bounty_prizes", 1_000.0, "2026-06-01T00:00:00Z"),
          journal(2, "brokers_fee", -400.0, "2026-06-11T00:00:00Z"),
        ],
      )
      .await
      .unwrap();

      let history = monthly_history(&db, BudgetScope::Character(1), "2026-06", 1).await;

      assert_eq!(history.len(), 1);
      assert_eq!(history[0].income, 1_000.0);
      assert_eq!(history[0].spend, 400.0);
      // 400 ISK drawn from the Jun-1 lot, spent Jun-11 → 10 days old.
      assert_eq!(history[0].age, 10.0);
    }
  }
}
