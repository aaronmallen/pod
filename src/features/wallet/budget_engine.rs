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
  model::{
    BudgetEntryAssignment, BudgetEntryKind, BudgetOwner, BudgetScope, MatchMode, Rule, RuleCondition, RuleField, RuleOp,
  },
  repo::{character, finance, org},
};

const DIRECTION_IN: &str = "in";
const DIRECTION_OUT: &str = "out";
const MARKET_BUY_TYPE: &str = "market_buy";
const MARKET_SALE_TYPE: &str = "market_sale";

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

/// Journal `ref_type`s that move ISK between two of the user's *own* wallets and
/// so are only ever a real internal transfer when an opposite-sign counter-leg
/// is found in another owned wallet. On their own (no matching counter-leg) they
/// are ordinary income or expense and classify by sign. EVE mirrors a genuine
/// internal transfer into both wallets under the same journal `id`, so detection
/// groups by `(ref_type, id)` and requires exactly two opposite-sign legs in
/// distinct owners.
const AMBIGUOUS_TRANSFER_REF_TYPES: &[&str] = &[
  "contract_price",
  "corporation_account_withdrawal",
  "player_donation",
  "player_trading",
];

/// Journal `ref_type`s that return ISK previously spent — a refund rather than
/// fresh income. Classified as [`BudgetFlow::Refund`] so a return can be filed
/// back into the envelope it was spent from rather than counted as new income.
const REFUND_REF_TYPES: &[&str] = &[
  "contract_collateral_refund",
  "contract_deposit_refund",
  "contract_reward_refund",
  "industry_job_refund",
  "market_escrow_refund",
  "reaction_refund",
];

/// The largest residual (in ISK) under which an internal transfer's mirrored
/// legs are treated as cancelling. Journal amounts are whole-ISK so any genuine
/// transfer nets to exactly zero; the slack only absorbs float round-off.
const TRANSFER_NET_EPSILON: f64 = 0.5;

/// How a single wallet flow is treated by the budget.
///
/// Income posts to Ready-to-Assign by default; an expense reduces (or wants) an
/// envelope; a refund returns ISK previously spent into the envelope it came
/// from; an internal transfer moves ISK between two of the user's own wallets
/// and is non-budgetable (excluded from RTA activity and needs-review). The flow
/// is derived from a row's `ref_type` plus its signed amount (or market side),
/// with internal-transfer status resolved dynamically by counter-leg matching —
/// see [`internal_transfer_ids`].
// Budget flow taxonomy (child opkvvkkx); consumed by the RTA formula and needs-review count in
// follow-on tasks. Exercised by unit tests until then.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BudgetFlow {
  Expense,
  Income,
  InternalTransfer,
  Refund,
}

impl BudgetFlow {
  /// Classifies a market trade by side: a buy spends working capital
  /// ([`BudgetFlow::Expense`]); a sell brings ISK in ([`BudgetFlow::Income`]).
  // Budget flow taxonomy (child opkvvkkx). Exercised by unit tests until the RTA formula consumes it.
  #[allow(dead_code)]
  pub fn from_market(is_buy: bool) -> Self {
    if is_buy {
      BudgetFlow::Expense
    } else {
      BudgetFlow::Income
    }
  }

  /// Classifies a journal `ref_type` and its signed `amount` into a flow,
  /// treating it as a standalone row (no internal-transfer counter-leg known).
  ///
  /// Refund ref_types are [`BudgetFlow::Refund`]; the ambiguous transfer
  /// ref_types and every other ref_type classify by sign — a positive amount is
  /// [`BudgetFlow::Income`], a negative amount is [`BudgetFlow::Expense`]. A zero
  /// amount falls to [`BudgetFlow::Income`] (it contributes nothing either way).
  /// Internal transfers are never returned here: that status is owner-aware and
  /// is layered on by [`classify_journal`].
  // Budget flow taxonomy (child opkvvkkx). Exercised by unit tests until the RTA formula consumes it.
  #[allow(dead_code)]
  pub fn from_ref_type(ref_type: &str, amount: f64) -> Self {
    if REFUND_REF_TYPES.contains(&ref_type) {
      return BudgetFlow::Refund;
    }
    if amount < 0.0 {
      BudgetFlow::Expense
    } else {
      BudgetFlow::Income
    }
  }
}

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
  /// Σ min(0, available) across the displayed month's categories (≤ 0); fuels
  /// B3's Cover-overspending.
  pub overspent: f64,
  /// Σ liquid balances of the scope's character + corp-division wallets.
  pub pool: f64,
  /// pool − Σ max(0, available) across the scope's categories: the liquid ISK
  /// not held in any envelope, conserving `pool = ready_to_assign + Σ held`.
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
/// categories before the assignment lands. Idempotent on `(scope, owner,
/// entry_kind, entry_id)`: reassigning the same entry replaces its category.
///
/// Cross-owner co-assignment cascades a trade's envelope onto every owner whose
/// wallet mirrors it, so this refuses to write a copy for an `owner` that holds
/// no wallet row for the entry (e.g. a character-owned copy of a corp-only id).
/// Such a row could never match the ledger and would sit silently inert, so it
/// is skipped and `Ok(None)` returned rather than persisting a mis-owned row.
// Per-entry budget assignment (child A); consumed by the Budget UI in child C. Exercised by unit
// tests until then.
#[allow(dead_code)]
pub async fn assign_entry(
  db: &Database,
  scope: BudgetScope,
  owner: BudgetOwner,
  entry_kind: BudgetEntryKind,
  entry_id: i64,
  category_id: i64,
) -> Result<Option<BudgetEntryAssignment>, Error> {
  if !crate::store::repo::budget::owner_holds_entry(db, owner, entry_kind, entry_id).await? {
    return Ok(None);
  }
  seed_scope(db, scope).await?;
  crate::store::repo::budget::upsert_entry_assignment(db, scope, owner, entry_kind, entry_id, category_id)
    .await
    .map(Some)
}

// Dormant auto-categorization helper: the v1 derivation is manual-only, so this
// feeds only the (test-exercised) `ResolutionContext::resolve` fallback path.
#[allow(dead_code)]
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
  pub journal_overrides: HashMap<(BudgetOwner, i64), i64>,
  pub market_overrides: HashMap<(BudgetOwner, i64), i64>,
  pub ref_overrides: HashMap<String, i64>,
  pub rules: Vec<Rule>,
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
      let Some(owner) = BudgetOwner::from_key(assignment.owner_kind(), assignment.owner_id()) else {
        continue;
      };
      match BudgetEntryKind::from_kind(assignment.entry_kind()) {
        Some(BudgetEntryKind::Journal) => {
          journal_overrides.insert((owner, assignment.entry_id()), assignment.category_id());
        }
        Some(BudgetEntryKind::Market) => {
          market_overrides.insert((owner, assignment.entry_id()), assignment.category_id());
        }
        None => {}
      }
    }

    Self {
      journal_overrides,
      market_overrides,
      ref_overrides: ref_type_overrides(db, scope).await,
      rules: crate::store::repo::budget::list_rules(db, scope)
        .await
        .unwrap_or_default(),
      slug_to_id: slug_to_category_id(db, scope).await,
    }
  }

  /// The effective budget category for an already-normalized ledger entry under
  /// the live resolution order: a manual per-entry override wins; else the first
  /// enabled rule (priority order) whose conditions match; else `None`
  /// (Ready-to-Assign). Rules match both outflows and inflows. Type defaults stay
  /// off, so the manual-only behavior is recovered exactly when no rule matches.
  pub fn resolve_target(&self, entry_kind: BudgetEntryKind, entry_id: i64, target: &MatchTarget) -> Option<i64> {
    let owner = target.owner?;
    if let Some(id) = self.override_for(owner, entry_kind, entry_id) {
      return Some(id);
    }
    rule_category_for(target, &self.rules)
  }

  pub fn override_for(&self, owner: BudgetOwner, entry_kind: BudgetEntryKind, entry_id: i64) -> Option<i64> {
    match entry_kind {
      BudgetEntryKind::Journal => self.journal_overrides.get(&(owner, entry_id)).copied(),
      BudgetEntryKind::Market => self.market_overrides.get(&(owner, entry_id)).copied(),
    }
  }

  /// The category an entry contributes its activity to, applying the first-run
  /// disposition for the money-conserving income→Ready-to-Assign model.
  ///
  /// [`resolve_target`](Self::resolve_target) answers which category an entry
  /// resolves to (a manual per-entry override, else a matching rule). For *income*
  /// inflows that resolution is reinterpreted on first run: a rule/auto-derived
  /// inflow assignment defaults to Ready-to-Assign (returns `None`, leaving the
  /// ISK in the pool) rather than being filed into the envelope, because under the
  /// new model genuine income belongs to the pool. An explicit per-entry *manual*
  /// override is always honored, so a user who deliberately filed an inflow into a
  /// category keeps it. Outflows, refunds, and internal transfers are unaffected:
  /// they file wherever they resolve. See [`dispose_inflow_assignment`].
  pub fn resolve_for_activity(
    &self,
    entry_kind: BudgetEntryKind,
    entry_id: i64,
    flow: BudgetFlow,
    target: &MatchTarget,
  ) -> Option<i64> {
    let owner = target.owner?;
    let manual = self.override_for(owner, entry_kind, entry_id);
    let resolved = self.resolve_target(entry_kind, entry_id, target);
    dispose_inflow_assignment(flow, manual, resolved)
  }

  // Dormant auto-categorization path. The v1 derivation and chip are manual-only
  // (they use `override_for`); this full-precedence resolver is retained, and
  // exercised by unit tests, for a future opt-in auto-assign mode.
  #[allow(dead_code)]
  pub fn resolve(
    &self,
    owner: BudgetOwner,
    entry_kind: BudgetEntryKind,
    entry_id: i64,
    ref_type: Option<&str>,
    is_buy: Option<bool>,
  ) -> Option<i64> {
    match entry_kind {
      BudgetEntryKind::Journal => {
        if let Some(&id) = self.journal_overrides.get(&(owner, entry_id)) {
          return Some(id);
        }
        category_for_ref_type(ref_type?, &self.ref_overrides, &self.slug_to_id)
      }
      // Market entries carry a side, not a `ref_type`, so the per-`ref_type` map
      // tier does not apply — they resolve by side once no per-entry override exists.
      BudgetEntryKind::Market => {
        if let Some(&id) = self.market_overrides.get(&(owner, entry_id)) {
          return Some(id);
        }
        self.slug_to_id.get(market_default_slug(is_buy?)).copied()
      }
    }
  }
}

/// First-run disposition of an inflow's resolved budget category under the
/// money-conserving income→Ready-to-Assign model.
///
/// Pre-existing rule/auto-derived assignments that filed income into an envelope
/// must be reinterpreted so genuine income lands in Ready-to-Assign rather than
/// being held in a category (which, under the new RTA formula, would draw the
/// pool down by that inflow's positive available). This is code-level
/// interpretation applied every derivation, not a one-time DB rewrite — the
/// owner-identity repair migration handles persisted cleanup separately and this
/// logic never fights it: it only declines to *file* a non-manual inflow, leaving
/// the stored assignment untouched.
///
/// The rule, given the entry's `flow`, the category an explicit per-entry
/// `manual` override pins it to (if any), and the category it otherwise
/// `resolved` to (manual override or matching rule):
///
/// - A *manual* override is always honored — the user's explicit choice wins for
///   every flow, income included.
/// - A non-manual [`BudgetFlow::Income`] inflow defaults to Ready-to-Assign
///   (`None`): a rule that filed income into an envelope is reinterpreted to leave
///   that ISK in the pool.
/// - Every other flow ([`BudgetFlow::Expense`], [`BudgetFlow::Refund`],
///   [`BudgetFlow::InternalTransfer`]) files wherever it resolved, unchanged.
// First-run income→RTA disposition (child rowluuus); consumed by the activity derivation. Exercised
// by unit tests.
#[allow(dead_code)]
pub fn dispose_inflow_assignment(flow: BudgetFlow, manual: Option<i64>, resolved: Option<i64>) -> Option<i64> {
  if manual.is_some() {
    return resolved;
  }
  if flow == BudgetFlow::Income {
    return None;
  }
  resolved
}

/// How a matched outflow is classified in a rule editor's live preview, relative
/// to the rest of the rule set, the manual override map, and the rule's target
/// category.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PreviewStatus {
  /// Already resolves to this rule's category anyway (no other rule claims it and
  /// it is not manually pinned elsewhere).
  Already,
  /// This rule wins — the entry will move into the target category.
  Assign,
  /// Pinned by a manual per-entry assignment; rules never touch it.
  Manual,
  /// A higher-priority *other* enabled rule claims it first.
  Preempted,
}

/// A ledger row (journal or market) flattened into the uniform shape a rule
/// matches against. Carries the entry's `type` token, signed `direction`,
/// absolute `amount`, owning character/corp, and the per-field text the matcher
/// reads.
///
/// For a journal row the `item`/`location`/`party`/`reference` fields all carry
/// the same enriched journal text (humanized ref_type label ∪ reason ∪
/// description), because journal rows have no resolved party/location/item names
/// to match against individually. Market rows carry distinct `item` and
/// `location` names.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MatchTarget {
  pub amount: f64,
  pub is_outflow: bool,
  pub item: String,
  pub location: String,
  pub owner: Option<BudgetOwner>,
  pub party: String,
  pub reference: String,
  pub type_token: String,
}

impl MatchTarget {
  pub fn journal(owner: BudgetOwner, ref_type: &str, amount: Option<f64>, text: &str) -> Self {
    let amount = amount.unwrap_or(0.0);
    Self {
      amount: amount.abs(),
      is_outflow: amount < 0.0,
      item: text.to_owned(),
      location: text.to_owned(),
      owner: Some(owner),
      party: text.to_owned(),
      reference: text.to_owned(),
      type_token: ref_type.to_owned(),
    }
  }

  pub fn market(owner: BudgetOwner, is_buy: bool, total: f64, item: &str, location: &str) -> Self {
    Self {
      amount: total.abs(),
      is_outflow: is_buy,
      item: item.to_owned(),
      location: location.to_owned(),
      owner: Some(owner),
      party: String::new(),
      reference: item.to_owned(),
      type_token: if is_buy { MARKET_BUY_TYPE } else { MARKET_SALE_TYPE }.to_owned(),
    }
  }

  /// Whether this entry matches a single condition. Text comparisons are
  /// case-insensitive; `Text` joins every text field; amount conditions parse ISK
  /// shorthand and compare against the absolute amount.
  pub fn matches_condition(&self, condition: &RuleCondition) -> bool {
    match condition.field() {
      RuleField::Amount => self.matches_amount(condition),
      RuleField::Direction => (condition.value() == DIRECTION_OUT) == self.is_outflow,
      RuleField::Character => match (self.owner, condition.value().trim().parse::<i64>().ok()) {
        (Some(owner), Some(id)) => {
          let same = owner.owner_id() == id;
          if condition.op() == RuleOp::IsNot { !same } else { same }
        }
        _ => condition.op() == RuleOp::IsNot,
      },
      RuleField::Type => {
        let same = self.type_token == *condition.value();
        if condition.op() == RuleOp::IsNot { !same } else { same }
      }
      RuleField::Text => self.matches_text(&self.any_text(), condition),
      RuleField::Item => self.matches_text(&self.item, condition),
      RuleField::Location => self.matches_text(&self.location, condition),
      RuleField::Party => self.matches_text(&self.party, condition),
      RuleField::Reference => self.matches_text(&self.reference, condition),
    }
  }

  /// Whether this entry matches a rule: inactive conditions are dropped, then the
  /// remaining conditions are joined by the rule's `match_mode`. A rule with no
  /// active conditions matches nothing.
  pub fn matches_rule(&self, rule: &Rule) -> bool {
    let mut active = rule.conditions().iter().filter(|c| is_active_condition(c)).peekable();
    if active.peek().is_none() {
      return false;
    }
    match rule.match_mode() {
      MatchMode::Any => active.any(|c| self.matches_condition(c)),
      MatchMode::All => active.all(|c| self.matches_condition(c)),
    }
  }

  /// The union of every text field, joined by a separator no needle can span, so
  /// a `Text` condition searches the whole searchable string at once.
  fn any_text(&self) -> String {
    [&self.reference, &self.party, &self.location, &self.item]
      .map(String::as_str)
      .join("\u{1}")
  }

  fn matches_amount(&self, condition: &RuleCondition) -> bool {
    let value = crate::ui::format::parse_isk(condition.value());
    match condition.op() {
      RuleOp::GreaterThan => self.amount > value,
      RuleOp::LessThan => self.amount < value,
      RuleOp::Between => {
        let Some(value2) = condition.value2().as_deref() else {
          return false;
        };
        let other = crate::ui::format::parse_isk(value2);
        self.amount >= value.min(other) && self.amount <= value.max(other)
      }
      _ => false,
    }
  }

  fn matches_text(&self, haystack: &str, condition: &RuleCondition) -> bool {
    let needle = condition.value().trim().to_lowercase();
    if needle.is_empty() {
      return condition.op() == RuleOp::NotContains;
    }
    let haystack = haystack.to_lowercase();
    match condition.op() {
      RuleOp::Contains => haystack.contains(&needle),
      RuleOp::NotContains => !haystack.contains(&needle),
      RuleOp::Is => haystack == needle,
      RuleOp::StartsWith => haystack.starts_with(&needle),
      _ => false,
    }
  }
}

/// Whether a condition carries a usable value, so a half-built rule never
/// matches the whole ledger. An unparseable amount is treated as inactive
/// rather than read as 0, which would otherwise match nearly everything.
pub fn is_active_condition(condition: &RuleCondition) -> bool {
  match condition.field() {
    RuleField::Amount if condition.op() == RuleOp::Between => {
      isk_value_parses(condition.value()) && condition.value2().as_deref().is_some_and(isk_value_parses)
    }
    RuleField::Amount => isk_value_parses(condition.value()),
    RuleField::Character => condition.value().trim().parse::<i64>().is_ok(),
    RuleField::Direction => matches!(condition.value().trim(), DIRECTION_IN | DIRECTION_OUT),
    _ => !condition.value().trim().is_empty(),
  }
}

/// Whether `input` is a well-formed ISK figure (mirrors `parse_isk`'s accept
/// set), distinguishing a real `0` from garbage that `parse_isk` would also
/// coerce to `0`.
fn isk_value_parses(input: &str) -> bool {
  let stripped: String = input
    .trim()
    .to_lowercase()
    .chars()
    .filter(|ch| !matches!(ch, ',' | ' ' | '_' | '\u{202f}'))
    .collect();
  if stripped.is_empty() || stripped == "-" {
    return false;
  }
  let number = match stripped.chars().last() {
    Some('t' | 'b' | 'm' | 'k') => &stripped[..stripped.len() - 1],
    _ => stripped.as_str(),
  };
  number.parse::<f64>().is_ok_and(f64::is_finite)
}

/// The number of supplied outflows a rule would catch. Inflows are never passed
/// in (rules only touch spending), so the caller filters to outflows first.
pub fn match_count(rule: &Rule, outflows: &[MatchTarget]) -> usize {
  outflows.iter().filter(|target| target.matches_rule(rule)).count()
}

/// Classifies every outflow a `draft` rule matches, predicting the verdict the
/// engine reaches on save. `live_rules` are the live rules in priority order
/// (including the slot of the rule being edited); the draft is spliced at its
/// real position — substituted for its matching id, or appended when new — and
/// the result is evaluated with the engine's first-enabled-match logic, so the
/// preview agrees with [`rule_category_for`]. `manual` maps an outflow's index
/// in `outflows` to its manually pinned category. `category_id` is the draft's
/// target envelope.
///
/// - `Manual`: a manual override pins the entry.
/// - `Preempted`: a higher-priority rule wins it for a different category.
/// - `Already`: the draft wins, but the entry already targets this category.
/// - `Assign`: the draft wins and moves it into the category.
pub fn preview_entries(
  draft: &Rule,
  live_rules: &[Rule],
  manual: &HashMap<usize, i64>,
  category_id: i64,
  outflows: &[MatchTarget],
) -> Vec<(usize, PreviewStatus)> {
  let mut effective: Vec<&Rule> = Vec::with_capacity(live_rules.len() + 1);
  let mut spliced = false;
  for rule in live_rules {
    if rule.id() == draft.id() {
      effective.push(draft);
      spliced = true;
    } else {
      effective.push(rule);
    }
  }
  if !spliced {
    effective.push(draft);
  }

  outflows
    .iter()
    .enumerate()
    .filter(|(_, target)| target.matches_rule(draft))
    .map(|(index, target)| {
      let status = if manual.contains_key(&index) {
        PreviewStatus::Manual
      } else {
        let winner = effective
          .iter()
          .find(|rule| rule.enabled() && target.matches_rule(rule));
        match winner {
          Some(rule) if std::ptr::eq(*rule, draft) => PreviewStatus::Assign,
          Some(rule) if rule.category_id() == category_id => PreviewStatus::Already,
          Some(_) => PreviewStatus::Preempted,
          None => PreviewStatus::Assign,
        }
      };
      (index, status)
    })
    .collect()
}

/// A short auto-name suggestion derived from a rule's first active condition,
/// for the editor's "name this rule" affordance. Type and character conditions
/// need a resolver (`type_label`/`character_name`) to turn their stored id/key
/// into a label; both default to the raw value when the resolver returns `None`.
/// Returns an empty string when the rule has no active conditions.
pub fn suggest_name(
  rule: &Rule,
  type_label: impl Fn(&str) -> Option<String>,
  character_name: impl Fn(&str) -> Option<String>,
) -> String {
  let Some(condition) = rule.conditions().iter().find(|c| is_active_condition(c)) else {
    return String::new();
  };
  match condition.field() {
    RuleField::Type => type_label(condition.value()).unwrap_or_else(|| condition.value().clone()),
    RuleField::Character => character_name(condition.value()).unwrap_or_else(|| condition.value().clone()),
    RuleField::Amount => format!("Amount {} {}", op_label(condition.op()), condition.value()),
    RuleField::Direction => if condition.value() == DIRECTION_IN {
      "Inflows"
    } else {
      "Outflows"
    }
    .to_owned(),
    _ => condition.value().clone(),
  }
}

/// Humanizes an EVE journal `ref_type` (e.g. `daily_goal_payouts`) into its
/// title-cased display label (`Daily Goal Payouts`). An empty `ref_type` renders
/// as an em dash. Shared by the wallet's row label, the journal search text, and
/// the rule engine's matchable text so all three see the same wording.
pub fn humanize_ref_type(ref_type: &str) -> String {
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

/// The enriched searchable text for a journal entry: the humanized ref_type
/// label ∪ reason ∪ description, joined by spaces. Shared by the rule matcher and
/// the wallet's journal search so searching the label a user actually sees (e.g.
/// "Daily") finds the row even though its raw `ref_type` is `daily_goal_payouts`.
pub fn journal_match_text(ref_type: &str, reason: Option<&str>, description: &str) -> String {
  let mut parts: Vec<&str> = vec![ref_type, reason.unwrap_or(""), description];
  let label = humanize_ref_type(ref_type);
  parts[0] = label.as_str();
  parts
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

pub fn op_label(op: RuleOp) -> &'static str {
  match op {
    RuleOp::Between => "is between",
    RuleOp::Contains => "contains",
    RuleOp::GreaterThan => "is over",
    RuleOp::Is => "is",
    RuleOp::IsNot => "is not",
    RuleOp::LessThan => "is under",
    RuleOp::NotContains => "does not contain",
    RuleOp::StartsWith => "starts with",
  }
}

pub fn field_label(field: RuleField) -> &'static str {
  match field {
    RuleField::Amount => "Amount",
    RuleField::Character => "Character",
    RuleField::Direction => "Direction",
    RuleField::Item => "Item",
    RuleField::Location => "Location",
    RuleField::Party => "Party",
    RuleField::Reference => "Reference",
    RuleField::Text => "Any text",
    RuleField::Type => "Type",
  }
}

/// Every rule field in the editor's field-picker order, mirroring the design's
/// vocabulary (Any text first, then Type, Party, Reference, Location, Item,
/// Amount, Direction, Character).
pub fn rule_fields() -> [RuleField; 9] {
  [
    RuleField::Text,
    RuleField::Type,
    RuleField::Party,
    RuleField::Reference,
    RuleField::Location,
    RuleField::Item,
    RuleField::Amount,
    RuleField::Direction,
    RuleField::Character,
  ]
}

/// The operators the editor offers for a given field, in menu order. The first
/// entry is the field's default operator when a condition switches to it.
pub fn ops_for_field(field: RuleField) -> &'static [RuleOp] {
  match field {
    RuleField::Amount => &[RuleOp::GreaterThan, RuleOp::LessThan, RuleOp::Between],
    RuleField::Character | RuleField::Type => &[RuleOp::Is, RuleOp::IsNot],
    RuleField::Direction => &[RuleOp::Is],
    RuleField::Reference => &[RuleOp::Contains, RuleOp::NotContains, RuleOp::StartsWith],
    RuleField::Item | RuleField::Location | RuleField::Party => &[RuleOp::Contains, RuleOp::NotContains, RuleOp::Is],
    RuleField::Text => &[RuleOp::Contains, RuleOp::NotContains],
  }
}

/// The kind of value editor a field needs: drives whether the rule builder shows
/// a free-text input, an amount input, or a fixed select.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldKind {
  Amount,
  Character,
  Direction,
  Text,
  Type,
}

pub fn field_kind(field: RuleField) -> FieldKind {
  match field {
    RuleField::Amount => FieldKind::Amount,
    RuleField::Character => FieldKind::Character,
    RuleField::Direction => FieldKind::Direction,
    RuleField::Type => FieldKind::Type,
    _ => FieldKind::Text,
  }
}

/// A fresh condition for `field`, seeded with the field's default operator and an
/// empty value (Direction defaults to "out"; a Between amount seeds its upper
/// bound). Mirrors the design's `newCondition`.
pub fn new_condition(field: RuleField) -> RuleCondition {
  let op = ops_for_field(field).first().copied().unwrap_or(RuleOp::Contains);
  let value = if field == RuleField::Direction {
    DIRECTION_OUT.to_owned()
  } else {
    String::new()
  };
  let value2 = (field == RuleField::Amount && op == RuleOp::Between).then(String::new);
  RuleCondition {
    field,
    op,
    value,
    value2,
  }
}

/// The two direction options, as `(stored value, label)` pairs.
pub fn direction_options() -> [(&'static str, &'static str); 2] {
  [(DIRECTION_OUT, "Outflow (spend)"), (DIRECTION_IN, "Inflow (income)")]
}

/// A one-line human summary of a rule's active conditions, joined by "and"/"or"
/// per the rule's match mode (e.g. `Reference contains "Cerberus" or Item
/// contains "Caracal"`). Returns "No conditions yet" for a rule with no active
/// conditions. `type_label`/`character_name` resolve the stored id/key of Type
/// and Character conditions; both fall back to the raw value when `None`.
pub fn summarize_rule(
  rule: &Rule,
  type_label: impl Fn(&str) -> Option<String>,
  character_name: impl Fn(&str) -> Option<String>,
) -> String {
  let parts: Vec<String> = rule
    .conditions()
    .iter()
    .filter(|c| is_active_condition(c))
    .map(|c| condition_text(c, &type_label, &character_name))
    .collect();
  if parts.is_empty() {
    return "No conditions yet".to_owned();
  }
  let joiner = match rule.match_mode() {
    MatchMode::Any => " or ",
    MatchMode::All => " and ",
  };
  parts.join(joiner)
}

fn condition_text(
  condition: &RuleCondition,
  type_label: &impl Fn(&str) -> Option<String>,
  character_name: &impl Fn(&str) -> Option<String>,
) -> String {
  let op = op_label(condition.op());
  match condition.field() {
    RuleField::Amount => match condition.op() {
      RuleOp::Between => format!(
        "Amount is between {} and {}",
        condition.value(),
        condition.value2().as_deref().unwrap_or("")
      ),
      _ => format!("Amount {op} {}", condition.value()),
    },
    RuleField::Direction => {
      let value = if condition.value() == DIRECTION_IN {
        "inflow"
      } else {
        "outflow"
      };
      format!("Direction is {value}")
    }
    RuleField::Type => {
      let value = type_label(condition.value()).unwrap_or_else(|| condition.value().clone());
      format!("Type {op} {value}")
    }
    RuleField::Character => {
      let value = character_name(condition.value()).unwrap_or_else(|| condition.value().clone());
      format!("Character {op} {value}")
    }
    field => format!("{} {op} \u{201c}{}\u{201d}", field_label(field), condition.value()),
  }
}

/// A rule's effective category for a single ledger entry: the first enabled rule
/// in priority order whose conditions match. Rules match both spending (outflows)
/// and income (inflows) — a rule that files an inflow into a category returns it
/// to that envelope and the derivation reserves it out of Ready-to-Assign. An
/// empty rule set (or no match) yields `None`. Pure over a loaded rule list.
fn rule_category_for(target: &MatchTarget, rules: &[Rule]) -> Option<i64> {
  rules
    .iter()
    .find(|rule| rule.enabled() && target.matches_rule(rule))
    .map(Rule::category_id)
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
  owner: BudgetOwner,
  entry_kind: BudgetEntryKind,
  entry_id: i64,
  ref_type: Option<&str>,
  is_buy: Option<bool>,
) -> Option<i64> {
  ResolutionContext::load(db, scope)
    .await
    .resolve(owner, entry_kind, entry_id, ref_type, is_buy)
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

/// Ready-to-Assign and overspending for a scope, money-conserving by
/// construction. Ready-to-Assign is `pool − Σ max(0, available)` over the
/// passed per-category availables, so the liquid pool always splits exactly as
/// `pool = ready_to_assign + Σ max(0, available)`. Each `available = carry +
/// assigned + signed_activity` with `carry` rolling every prior month, so the
/// sum is the global ISK held in envelopes and the remainder is what is free to
/// assign. Only positive availables hold ISK: an overspent envelope (negative
/// available) shows red and is reported in `overspent = Σ min(0, available)`,
/// but it never inflates Ready-to-Assign. Genuine income carries no override or
/// rule, so it never lands in an envelope and stays counted in `pool` as
/// Ready-to-Assign; only an explicit override or a refund files an inflow into a
/// category.
pub fn pool_summary(pool: f64, availables: impl IntoIterator<Item = f64>) -> PoolSummary {
  let mut held = 0.0;
  let mut overspent = 0.0;
  for available in availables {
    if available < 0.0 {
      overspent += available;
    } else {
      held += available;
    }
  }
  PoolSummary {
    overspent,
    pool,
    ready_to_assign: pool - held,
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
  use crate::store::{
    model::{NewCategory, NewGroup},
    repo::budget::{
      create_category, create_group, is_scope_seeded, list_groups, mark_scope_seeded, upsert_ref_type_map,
    },
  };

  // Seed a scope's starter budget exactly once. The persisted marker means
  // deleting every group never resurrects the defaults — the empty budget sticks.
  if is_scope_seeded(db, scope).await? {
    return Ok(());
  }
  // A scope seeded before this marker existed already has groups; adopt it
  // without re-seeding.
  if !list_groups(db, scope).await?.is_empty() {
    mark_scope_seeded(db, scope).await?;
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

  mark_scope_seeded(db, scope).await?;
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
  owner: BudgetOwner,
  ref_type: String,
  /// The enriched searchable text for rule matching: humanized ref_type label ∪
  /// reason ∪ description.
  text: String,
}

/// Every in-scope journal and transaction row, flattened for the rule/override
/// resolver, loaded once alongside the resolution context so both the monthly
/// activity sum and the needs-review count resolve in-memory from one DB pass.
struct ScopeLedger {
  context: ResolutionContext,
  journal_rows: Vec<JournalActivity>,
  transactions: Vec<TransactionActivity>,
}

/// A wallet transaction reduced to the fields the monthly derivation needs, with
/// `amount` already signed (buy = spend/negative, sell = income/positive).
struct TransactionActivity {
  amount: f64,
  date: String,
  is_buy: bool,
  item: String,
  location: String,
  owner: BudgetOwner,
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

/// The flow classification of a journal row, owner-aware for internal transfers.
///
/// A row whose journal id is in `transfer_ids` (an owner-aware internal transfer,
/// see [`internal_transfer_ids`]) is [`BudgetFlow::InternalTransfer`] regardless
/// of its ref_type; every other row classifies statically by ref_type and sign
/// via [`BudgetFlow::from_ref_type`].
// Budget flow taxonomy (child opkvvkkx); consumed by the RTA formula and needs-review count in
// follow-on tasks. Exercised by unit tests until then.
#[allow(dead_code)]
fn classify_journal(row: &JournalActivity, transfer_ids: &HashSet<i64>) -> BudgetFlow {
  if transfer_ids.contains(&row.id) {
    return BudgetFlow::InternalTransfer;
  }
  BudgetFlow::from_ref_type(&row.ref_type, row.amount.unwrap_or(0.0))
}

/// The journal ids that are genuine internal transfers between two of the user's
/// own wallets, detected owner-aware.
///
/// EVE mirrors one internal-transfer event into both wallets under the same
/// journal `id`, once positive and once negative. Only the ambiguous transfer
/// ref_types (donation/withdrawal/contract-price/trading) can be a transfer, and
/// only when a counter-leg actually exists in another owned wallet, so legs are
/// grouped by `(ref_type, id)` — *not* the bare id, which collides a character
/// and corp sharing an EVE id — and a group is an internal transfer only when it
/// holds exactly two legs of opposite sign in two distinct owners. The exactly
/// two requirement rejects 3+-leg pile-ups, and the distinct-owner requirement
/// rejects two legs that landed in the same wallet, so neither is mis-paired.
// Budget flow taxonomy (child opkvvkkx); consumed by the RTA formula and needs-review count in
// follow-on tasks. Exercised by unit tests until then.
#[allow(dead_code)]
fn internal_transfer_ids(rows: &[JournalActivity]) -> HashSet<i64> {
  let mut legs_by_key: HashMap<(&str, i64), Vec<&JournalActivity>> = HashMap::new();
  for row in rows {
    if !AMBIGUOUS_TRANSFER_REF_TYPES.contains(&row.ref_type.as_str()) {
      continue;
    }
    legs_by_key
      .entry((row.ref_type.as_str(), row.id))
      .or_default()
      .push(row);
  }

  let mut ids = HashSet::new();
  for ((_, journal_id), legs) in legs_by_key {
    if legs.len() != 2 {
      continue;
    }
    let amounts: Vec<f64> = legs.iter().filter_map(|leg| leg.amount).collect();
    if amounts.len() != 2 {
      continue;
    }
    let opposite_signs = amounts[0].signum() != amounts[1].signum() && amounts.iter().all(|a| *a != 0.0);
    let distinct_owners = legs[0].owner != legs[1].owner;
    let cancels = (amounts[0] + amounts[1]).abs() < TRANSFER_NET_EPSILON;
    if opposite_signs && distinct_owners && cancels {
      ids.insert(journal_id);
    }
  }
  ids
}

/// Aggregates a scope's signed activity by category for a single UTC calendar
/// `month` (`YYYY-MM`), unioning every in-scope character and covered
/// corp-division journal with the matching wallet transactions. v1 is fully
/// manual: only entries carrying an explicit per-entry override contribute to a
/// category; everything else stays in Ready-to-Assign. Market trades are counted
/// from the transaction source, and their journal twins are de-duplicated away
/// so an assigned trade is counted once.
// Budget activity math (B2); consumed by the Budget Plan/Reflect UI in B3+. Exercised by unit tests
// until then.
#[allow(dead_code)]
pub async fn monthly_activity(db: &Database, scope: BudgetScope, month: &str) -> HashMap<i64, f64> {
  activity_by_month(db, scope).await.remove(month).unwrap_or_default()
}

/// Aggregates a scope's signed activity by category for *every* UTC calendar
/// month in one batched pass, keyed `month → (category id → signed activity)`.
/// Loads each in-scope wallet's journal and transactions once, then groups by
/// month so the multi-month carry chain has real per-month activity without an
/// O(history) query-per-month blow-up. Market-twin de-duplication and the
/// owner-aware manual override resolution match [`monthly_activity`] exactly,
/// applied independently within each month.
// Budget activity math (B2); consumed by the carry chain in the Budget Plan UI. Exercised by unit
// tests until then.
#[allow(dead_code)]
pub async fn activity_by_month(db: &Database, scope: BudgetScope) -> HashMap<String, HashMap<i64, f64>> {
  let ScopeLedger {
    context,
    journal_rows,
    transactions,
  } = load_scope_ledger(db, scope).await;

  // A market trade can surface in several wallets at once: a corp trade is
  // mirrored into the trading character's personal wallet under the SAME
  // transaction_id (a PK in both transaction tables), and each carries a
  // `market_transaction` journal twin. `ingested_by_month` collects every
  // transaction_id per month so those journal twins are suppressed below within
  // their own month.
  let mut ingested_by_month: HashMap<String, HashSet<i64>> = HashMap::new();
  for tx in &transactions {
    if let Some(month) = month_key(&tx.date) {
      ingested_by_month.entry(month).or_default().insert(tx.transaction_id);
    }
  }

  // v1 is fully manual: only entries the user has explicitly assigned (a per-entry
  // override) contribute to a category. Everything else stays in Ready-to-Assign.
  // Overrides are owner-aware, so a character entry and a corp entry sharing an EVE
  // id resolve to their own categories rather than aliasing onto one.
  //
  // An internal transfer is one EVE journal event mirrored into two owned wallets
  // under the same journal id with opposite signs. It moves no ISK in or out of
  // the user's holdings, so both legs are excluded from category activity entirely
  // and never reach Ready-to-Assign or the needs-review count.
  let transfer_ids = internal_transfer_ids(&journal_rows);

  let mut by_month: HashMap<String, HashMap<i64, f64>> = HashMap::new();
  let empty = HashSet::new();
  for row in &journal_rows {
    let Some(month) = month_key(&row.date) else {
      continue;
    };
    let ingested = ingested_by_month.get(&month).unwrap_or(&empty);
    if is_market_twin(row, ingested) {
      continue;
    }
    let flow = classify_journal(row, &transfer_ids);
    if flow == BudgetFlow::InternalTransfer {
      continue;
    }
    let Some(amount) = row.amount else { continue };
    let target = MatchTarget::journal(row.owner, &row.ref_type, Some(amount), &row.text);
    // First-run income→RTA disposition: a non-manual inflow defaults to
    // Ready-to-Assign rather than being held in the rule-resolved envelope.
    if let Some(category_id) = context.resolve_for_activity(BudgetEntryKind::Journal, row.id, flow, &target) {
      *by_month.entry(month).or_default().entry(category_id).or_insert(0.0) += amount;
    }
  }

  // De-duplicate market activity by transaction_id within each month: a corp trade
  // and its character-wallet mirror share one transaction_id and are one event, so
  // the trade contributes once. An id is marked counted only once it resolves to a
  // category, so an unassigned copy never suppresses an assigned one.
  let mut counted_by_month: HashMap<String, HashSet<i64>> = HashMap::new();
  for tx in &transactions {
    let Some(month) = month_key(&tx.date) else {
      continue;
    };
    if counted_by_month
      .entry(month.clone())
      .or_default()
      .contains(&tx.transaction_id)
    {
      continue;
    }
    let target = MatchTarget::market(tx.owner, tx.is_buy, tx.amount, &tx.item, &tx.location);
    let flow = BudgetFlow::from_market(tx.is_buy);
    // First-run income→RTA disposition: a non-manual sell (inflow) defaults to
    // Ready-to-Assign rather than being held in the rule-resolved envelope.
    if let Some(category_id) = context.resolve_for_activity(BudgetEntryKind::Market, tx.transaction_id, flow, &target) {
      *by_month
        .entry(month.clone())
        .or_default()
        .entry(category_id)
        .or_insert(0.0) += tx.amount;
      counted_by_month.entry(month).or_default().insert(tx.transaction_id);
    }
  }

  by_month
}

/// The number of ledger entries in `month` that still need a category — the
/// Review &amp; assign banner's count, sourced from the DB so it reflects every
/// entry in the month, not only the loaded page.
///
/// Only uncategorized *expenses* are reviewable: a row counts when it is an
/// outflow ([`BudgetFlow::Expense`]) that [`ResolutionContext::resolve_target`]
/// leaves unresolved (no manual override and no matching rule), matching the
/// per-row chip the UI renders. Income posts to Ready-to-Assign and internal
/// transfers move no money, so neither ever appears in the count. Journal
/// market-transaction twins are excluded only when their trade was ingested from
/// the Transactions side (mirroring [`is_market_twin`]), so a kept un-twinned
/// market journal row stays reviewable. Market trades are de-duplicated by
/// `transaction_id`, so a corp trade mirrored into a personal wallet counts once
/// — consistent with [`activity_by_month`]'s de-dup.
pub async fn uncategorized_count_for_month(db: &Database, scope: BudgetScope, month: &str) -> usize {
  let ScopeLedger {
    context,
    journal_rows,
    transactions,
  } = load_scope_ledger(db, scope).await;

  let ingested: HashSet<i64> = transactions
    .iter()
    .filter(|tx| month_key(&tx.date).as_deref() == Some(month))
    .map(|tx| tx.transaction_id)
    .collect();
  let transfer_ids = internal_transfer_ids(&journal_rows);

  let mut count = 0;
  for row in &journal_rows {
    if month_key(&row.date).as_deref() != Some(month) {
      continue;
    }
    if is_market_twin(row, &ingested) {
      continue;
    }
    if classify_journal(row, &transfer_ids) != BudgetFlow::Expense {
      continue;
    }
    let Some(amount) = row.amount else { continue };
    let target = MatchTarget::journal(row.owner, &row.ref_type, Some(amount), &row.text);
    if context
      .resolve_target(BudgetEntryKind::Journal, row.id, &target)
      .is_none()
    {
      count += 1;
    }
  }

  let mut seen: HashSet<i64> = HashSet::new();
  for tx in &transactions {
    if month_key(&tx.date).as_deref() != Some(month) {
      continue;
    }
    if BudgetFlow::from_market(tx.is_buy) != BudgetFlow::Expense {
      continue;
    }
    if !seen.insert(tx.transaction_id) {
      continue;
    }
    let target = MatchTarget::market(tx.owner, tx.is_buy, tx.amount, &tx.item, &tx.location);
    if context
      .resolve_target(BudgetEntryKind::Market, tx.transaction_id, &target)
      .is_none()
    {
      count += 1;
    }
  }

  count
}

async fn load_scope_ledger(db: &Database, scope: BudgetScope) -> ScopeLedger {
  let context = ResolutionContext::load(db, scope).await;

  // Item/location names back the rule engine's text matching for market rows;
  // loaded once so per-month resolution stays in-memory. Empty when no rule
  // touches them, so the lookups are harmless on the manual-only path.
  let (type_names, location_names) = match context.rules.is_empty() {
    true => (HashMap::new(), HashMap::new()),
    false => (type_names(db).await, location_names(db).await),
  };

  let mut journal_rows: Vec<JournalActivity> = Vec::new();
  let mut transactions: Vec<TransactionActivity> = Vec::new();
  for character_id in scope_character_ids(db, scope).await {
    let owner = BudgetOwner::Character(character_id);
    for row in finance::wallet_journal(db, character_id).await.unwrap_or_default() {
      journal_rows.push(JournalActivity {
        amount: row.amount(),
        context_id: row.context_id(),
        context_id_type: row.context_id_type().clone(),
        date: row.date().clone(),
        id: row.id(),
        owner,
        ref_type: row.ref_type().clone(),
        text: journal_match_text(row.ref_type(), row.reason().as_deref(), row.description()),
      });
    }
    for tx in finance::wallet_transactions(db, character_id).await.unwrap_or_default() {
      transactions.push(TransactionActivity {
        amount: tx.unit_price() * tx.quantity() as f64 * if tx.is_buy() { -1.0 } else { 1.0 },
        date: tx.date().clone(),
        is_buy: tx.is_buy(),
        item: type_names.get(&tx.type_id()).cloned().unwrap_or_default(),
        location: location_names.get(&tx.location_id()).cloned().unwrap_or_default(),
        owner,
        transaction_id: tx.transaction_id(),
      });
    }
  }
  for corp in scope_corporation_ids(db, scope).await {
    let owner = BudgetOwner::Corporation(corp);
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
          owner,
          ref_type: row.ref_type().clone(),
          text: journal_match_text(row.ref_type(), row.reason().as_deref(), row.description()),
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
          item: type_names.get(&tx.type_id()).cloned().unwrap_or_default(),
          location: location_names.get(&tx.location_id()).cloned().unwrap_or_default(),
          owner,
          transaction_id: tx.transaction_id(),
        });
      }
    }
  }

  ScopeLedger {
    context,
    journal_rows,
    transactions,
  }
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
  // income/spend are raw monthly cashflow (every journal movement), independent
  // of envelope assignment — the Reflect trend tracks money in/out, not budgeting.
  let flows = scope_journal_flows(db, scope).await;
  let ages = fifo_ages_by_month(flows.iter().map(|(d, a)| (d.as_str(), *a)));

  let mut out: Vec<MonthFlow> = Vec::with_capacity(months);
  for step in (0..months as i32).rev() {
    let key = shift_month_key(month, -step);
    let income = flows
      .iter()
      .filter(|(date, a)| *a > 0.0 && month_key(date.as_str()).as_deref() == Some(&key))
      .map(|(_, a)| *a)
      .sum::<f64>();
    let spend = flows
      .iter()
      .filter(|(date, a)| *a < 0.0 && month_key(date.as_str()).as_deref() == Some(&key))
      .map(|(_, a)| -*a)
      .sum::<f64>();
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

/// Station and structure `id → name` map for resolving a row's location name.
/// Shared by the ledger chip and the envelope math so an Item/Location rule
/// matches the same rows in both. Loaded once per pass.
pub(crate) async fn location_names(db: &Database) -> HashMap<i64, String> {
  let mut names = HashMap::new();
  for station in crate::store::repo::sde::all_stations(db).await.unwrap_or_default() {
    names.insert(station.id(), station.name().clone());
  }
  for structure in crate::store::repo::sde::all_structures(db).await.unwrap_or_default() {
    names.insert(structure.id(), structure.name().clone());
  }
  names
}

/// Item-type `id → name` map for resolving a row's item name. Shared by the
/// ledger chip and the envelope math so an Item/Location rule matches the same
/// rows in both. Loaded once per pass.
pub(crate) async fn type_names(db: &Database) -> HashMap<i64, String> {
  crate::store::repo::sde::all_item_types(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|item| (item.id(), item.name().clone()))
    .collect()
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

  fn condition(field: RuleField, op: RuleOp, value: &str) -> RuleCondition {
    RuleCondition {
      field,
      op,
      value: value.to_owned(),
      value2: None,
    }
  }

  fn between(lo: &str, hi: &str) -> RuleCondition {
    RuleCondition {
      field: RuleField::Amount,
      op: RuleOp::Between,
      value: lo.to_owned(),
      value2: Some(hi.to_owned()),
    }
  }

  fn rule(category_id: i64, enabled: bool, match_mode: MatchMode, conditions: Vec<RuleCondition>) -> Rule {
    Rule {
      category_id,
      conditions,
      enabled,
      id: category_id,
      match_mode,
      name: String::new(),
    }
  }

  fn journal_outflow(owner: BudgetOwner, ref_type: &str, amount: f64, text: &str) -> MatchTarget {
    MatchTarget::journal(owner, ref_type, Some(amount), text)
  }

  mod match_target {
    use super::*;

    mod matches_condition {
      use pretty_assertions::{assert_eq, assert_ne};

      use super::*;

      #[test]
      fn it_matches_text_contains_and_not_contains() {
        let target = journal_outflow(BudgetOwner::Character(1), "broker_fee", -10.0, "Daily Goal Payouts");

        assert!(target.matches_condition(&condition(RuleField::Text, RuleOp::Contains, "daily")));
        assert!(!target.matches_condition(&condition(RuleField::Text, RuleOp::Contains, "weekly")));
        assert!(target.matches_condition(&condition(RuleField::Text, RuleOp::NotContains, "weekly")));
        assert!(!target.matches_condition(&condition(RuleField::Text, RuleOp::NotContains, "daily")));
      }

      #[test]
      fn it_matches_text_is_and_starts_with() {
        let target = MatchTarget::market(BudgetOwner::Character(1), true, 5.0, "Hobgoblin II", "Jita IV");

        assert!(target.matches_condition(&condition(RuleField::Item, RuleOp::Is, "hobgoblin ii")));
        assert!(target.matches_condition(&condition(RuleField::Item, RuleOp::StartsWith, "hobgob")));
        assert!(!target.matches_condition(&condition(RuleField::Item, RuleOp::Is, "hobgob")));
      }

      #[test]
      fn it_matches_distinct_market_item_and_location() {
        let target = MatchTarget::market(BudgetOwner::Character(1), true, 5.0, "Caracal", "Amarr VIII");

        assert!(target.matches_condition(&condition(RuleField::Location, RuleOp::Contains, "amarr")));
        assert!(!target.matches_condition(&condition(RuleField::Location, RuleOp::Contains, "caracal")));
        assert!(target.matches_condition(&condition(RuleField::Item, RuleOp::Contains, "caracal")));
      }

      #[test]
      fn it_treats_an_empty_text_needle_as_a_non_match_except_for_not_contains() {
        let target = journal_outflow(BudgetOwner::Character(1), "tax", -10.0, "Sales Tax");

        assert!(!target.matches_condition(&condition(RuleField::Text, RuleOp::Contains, "  ")));
        assert!(target.matches_condition(&condition(RuleField::Text, RuleOp::NotContains, "  ")));
      }

      #[test]
      fn it_matches_amount_over_under_and_between() {
        let target = journal_outflow(BudgetOwner::Character(1), "tax", -150_000_000.0, "Sales Tax");

        assert!(target.matches_condition(&condition(RuleField::Amount, RuleOp::GreaterThan, "100m")));
        assert!(!target.matches_condition(&condition(RuleField::Amount, RuleOp::GreaterThan, "1b")));
        assert!(target.matches_condition(&condition(RuleField::Amount, RuleOp::LessThan, "1b")));
        assert!(target.matches_condition(&between("100m", "200m")));
        assert!(target.matches_condition(&between("200m", "100m")));
        assert!(!target.matches_condition(&between("200m", "300m")));
      }

      #[test]
      fn it_compares_amount_on_the_absolute_value() {
        let target = journal_outflow(BudgetOwner::Character(1), "tax", -500_000_000.0, "Sales Tax");

        assert!(target.matches_condition(&condition(RuleField::Amount, RuleOp::GreaterThan, "100m")));
      }

      #[test]
      fn it_matches_direction_is() {
        let outflow = journal_outflow(BudgetOwner::Character(1), "tax", -10.0, "Sales Tax");
        let inflow = MatchTarget::journal(BudgetOwner::Character(1), "bounty", Some(10.0), "Bounty");

        assert!(outflow.matches_condition(&condition(RuleField::Direction, RuleOp::Is, "out")));
        assert!(!outflow.matches_condition(&condition(RuleField::Direction, RuleOp::Is, "in")));
        assert!(inflow.matches_condition(&condition(RuleField::Direction, RuleOp::Is, "in")));
      }

      #[test]
      fn it_matches_character_is_and_is_not() {
        let target = journal_outflow(BudgetOwner::Character(42), "tax", -10.0, "Sales Tax");

        assert!(target.matches_condition(&condition(RuleField::Character, RuleOp::Is, "42")));
        assert!(!target.matches_condition(&condition(RuleField::Character, RuleOp::Is, "7")));
        assert!(target.matches_condition(&condition(RuleField::Character, RuleOp::IsNot, "7")));
        assert!(!target.matches_condition(&condition(RuleField::Character, RuleOp::IsNot, "42")));
      }

      #[test]
      fn it_matches_type_against_journal_ref_type_and_market_side() {
        let journal = journal_outflow(BudgetOwner::Character(1), "broker_fee", -10.0, "Broker Fee");
        let buy = MatchTarget::market(BudgetOwner::Character(1), true, 5.0, "Caracal", "Jita");
        let sale = MatchTarget::market(BudgetOwner::Character(1), false, 5.0, "Caracal", "Jita");

        assert!(journal.matches_condition(&condition(RuleField::Type, RuleOp::Is, "broker_fee")));
        assert!(journal.matches_condition(&condition(RuleField::Type, RuleOp::IsNot, "tax")));
        assert!(buy.matches_condition(&condition(RuleField::Type, RuleOp::Is, "market_buy")));
        assert!(sale.matches_condition(&condition(RuleField::Type, RuleOp::Is, "market_sale")));

        assert_ne!(buy.type_token, sale.type_token);
        assert_eq!(buy.type_token, "market_buy");
      }
    }

    mod matches_rule {
      use super::*;

      #[test]
      fn it_joins_conditions_with_all() {
        let target = journal_outflow(BudgetOwner::Character(1), "tax", -150_000_000.0, "Sales Tax");
        let all = rule(
          1,
          true,
          MatchMode::All,
          vec![
            condition(RuleField::Text, RuleOp::Contains, "sales"),
            condition(RuleField::Amount, RuleOp::GreaterThan, "100m"),
          ],
        );

        assert!(target.matches_rule(&all));
      }

      #[test]
      fn it_requires_every_condition_under_all() {
        let target = journal_outflow(BudgetOwner::Character(1), "tax", -10_000_000.0, "Sales Tax");
        let all = rule(
          1,
          true,
          MatchMode::All,
          vec![
            condition(RuleField::Text, RuleOp::Contains, "sales"),
            condition(RuleField::Amount, RuleOp::GreaterThan, "100m"),
          ],
        );

        assert!(!target.matches_rule(&all));
      }

      #[test]
      fn it_joins_conditions_with_any() {
        let target = journal_outflow(BudgetOwner::Character(1), "tax", -10_000_000.0, "Sales Tax");
        let any = rule(
          1,
          true,
          MatchMode::Any,
          vec![
            condition(RuleField::Text, RuleOp::Contains, "missile"),
            condition(RuleField::Amount, RuleOp::GreaterThan, "1m"),
          ],
        );

        assert!(target.matches_rule(&any));
      }

      #[test]
      fn it_ignores_inactive_conditions() {
        let target = journal_outflow(BudgetOwner::Character(1), "tax", -10.0, "Sales Tax");
        let with_blank = rule(
          1,
          true,
          MatchMode::All,
          vec![
            condition(RuleField::Text, RuleOp::Contains, "sales"),
            condition(RuleField::Text, RuleOp::Contains, "  "),
          ],
        );

        assert!(target.matches_rule(&with_blank));
      }

      #[test]
      fn it_never_matches_a_rule_with_no_active_conditions() {
        let target = journal_outflow(BudgetOwner::Character(1), "tax", -10.0, "Sales Tax");
        let empty = rule(
          1,
          true,
          MatchMode::All,
          vec![condition(RuleField::Text, RuleOp::Contains, "")],
        );

        assert!(!target.matches_rule(&empty));
      }

      #[test]
      fn it_never_matches_a_rule_whose_only_condition_is_an_unparseable_amount() {
        let target = journal_outflow(BudgetOwner::Character(1), "tax", -10.0, "Sales Tax");
        let garbage_amount = rule(
          1,
          true,
          MatchMode::All,
          vec![condition(RuleField::Amount, RuleOp::GreaterThan, "garbage")],
        );

        assert!(!target.matches_rule(&garbage_amount));
      }
    }
  }

  mod is_active_condition {
    use super::*;

    #[test]
    fn it_treats_a_blank_value_as_inactive() {
      assert!(!is_active_condition(&condition(
        RuleField::Text,
        RuleOp::Contains,
        "   "
      )));
      assert!(is_active_condition(&condition(RuleField::Text, RuleOp::Contains, "x")));
    }

    #[test]
    fn it_requires_both_bounds_for_a_between() {
      assert!(!is_active_condition(&condition(
        RuleField::Amount,
        RuleOp::Between,
        "100m"
      )));
      assert!(is_active_condition(&between("100m", "200m")));
    }

    #[test]
    fn it_treats_an_unparseable_amount_as_inactive() {
      assert!(!is_active_condition(&condition(
        RuleField::Amount,
        RuleOp::GreaterThan,
        "garbage"
      )));
      assert!(!is_active_condition(&condition(
        RuleField::Amount,
        RuleOp::GreaterThan,
        "b"
      )));
    }

    #[test]
    fn it_keeps_a_real_zero_amount_active() {
      assert!(is_active_condition(&condition(
        RuleField::Amount,
        RuleOp::GreaterThan,
        "0"
      )));
    }

    #[test]
    fn it_treats_a_between_with_an_unparseable_bound_as_inactive() {
      assert!(!is_active_condition(&between("100m", "garbage")));
      assert!(!is_active_condition(&between("garbage", "200m")));
    }

    #[test]
    fn it_treats_an_unparseable_character_id_as_inactive() {
      assert!(!is_active_condition(&condition(
        RuleField::Character,
        RuleOp::Is,
        "abc"
      )));
      assert!(is_active_condition(&condition(RuleField::Character, RuleOp::Is, "42")));
    }

    #[test]
    fn it_treats_an_unknown_direction_token_as_inactive() {
      assert!(!is_active_condition(&condition(
        RuleField::Direction,
        RuleOp::Is,
        "sideways"
      )));
      assert!(is_active_condition(&condition(RuleField::Direction, RuleOp::Is, "in")));
      assert!(is_active_condition(&condition(RuleField::Direction, RuleOp::Is, "out")));
    }
  }

  mod rule_category_for {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_first_enabled_matching_rule_by_priority() {
      let target = journal_outflow(BudgetOwner::Character(1), "tax", -10.0, "Sales Tax");
      let rules = vec![
        rule(
          10,
          false,
          MatchMode::All,
          vec![condition(RuleField::Text, RuleOp::Contains, "sales")],
        ),
        rule(
          20,
          true,
          MatchMode::All,
          vec![condition(RuleField::Text, RuleOp::Contains, "sales")],
        ),
        rule(
          30,
          true,
          MatchMode::All,
          vec![condition(RuleField::Text, RuleOp::Contains, "sales")],
        ),
      ];

      assert_eq!(rule_category_for(&target, &rules), Some(20));
    }

    #[test]
    fn it_resolves_an_inflow_a_rule_matches() {
      // Income files into an envelope too — e.g. an inheritance returned to a
      // "Windfall" category. The derivation reserves it out of Ready-to-Assign.
      let inflow = MatchTarget::journal(BudgetOwner::Character(1), "inheritance", Some(10.0), "Inheritance");
      let rules = vec![rule(
        10,
        true,
        MatchMode::All,
        vec![condition(RuleField::Text, RuleOp::Contains, "inheritance")],
      )];

      assert_eq!(rule_category_for(&inflow, &rules), Some(10));
    }

    #[test]
    fn it_matches_an_inflow_by_direction() {
      let inflow = MatchTarget::journal(BudgetOwner::Character(1), "bounty", Some(10.0), "Bounty");
      let outflow = journal_outflow(BudgetOwner::Character(1), "tax", -10.0, "Sales Tax");
      let rules = vec![rule(
        10,
        true,
        MatchMode::All,
        vec![condition(RuleField::Direction, RuleOp::Is, "in")],
      )];

      assert_eq!(rule_category_for(&inflow, &rules), Some(10));
      assert_eq!(rule_category_for(&outflow, &rules), None);
    }
  }

  mod inflow_disposition {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clears_a_non_manual_inflow_to_ready_to_assign() {
      // A rule filed this income into category 10; under the money-conserving
      // model it defaults back to Ready-to-Assign (None) so the pool is not
      // drawn down by the inflow.
      assert_eq!(dispose_inflow_assignment(BudgetFlow::Income, None, Some(10)), None);
    }

    #[test]
    fn it_retains_a_manual_inflow_assignment() {
      // The user explicitly pinned this income to category 7; their choice wins.
      assert_eq!(dispose_inflow_assignment(BudgetFlow::Income, Some(7), Some(7)), Some(7));
    }

    #[test]
    fn it_leaves_non_income_flows_filing_where_they_resolve() {
      // Expenses, refunds, and transfers are unaffected by the inflow disposition.
      assert_eq!(dispose_inflow_assignment(BudgetFlow::Expense, None, Some(5)), Some(5));
      assert_eq!(dispose_inflow_assignment(BudgetFlow::Refund, None, Some(5)), Some(5));
      assert_eq!(
        dispose_inflow_assignment(BudgetFlow::InternalTransfer, None, Some(5)),
        Some(5)
      );
    }

    #[test]
    fn it_disposes_through_the_resolution_context() {
      // Two identical inflows: one carries a manual per-entry override, the other
      // is only matched by a rule. The manual one is retained; the rule-derived
      // one is cleared to Ready-to-Assign.
      let owner = BudgetOwner::Character(1);
      let rules = vec![rule(
        10,
        true,
        MatchMode::All,
        vec![condition(RuleField::Direction, RuleOp::Is, "in")],
      )];
      let mut journal_overrides = HashMap::new();
      journal_overrides.insert((owner, 100_i64), 10_i64);
      let context = ResolutionContext {
        journal_overrides,
        rules,
        ..Default::default()
      };

      let target = MatchTarget::journal(owner, "bounty", Some(10.0), "Bounty");
      // Manual override on entry 100 is honored.
      assert_eq!(
        context.resolve_for_activity(BudgetEntryKind::Journal, 100, BudgetFlow::Income, &target),
        Some(10)
      );
      // Entry 200 is only rule-matched (non-manual inflow) → Ready-to-Assign.
      assert_eq!(
        context.resolve_for_activity(BudgetEntryKind::Journal, 200, BudgetFlow::Income, &target),
        None
      );
    }
  }

  mod match_count {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_counts_the_matching_outflows() {
      let outflows = vec![
        journal_outflow(BudgetOwner::Character(1), "tax", -10.0, "Sales Tax"),
        journal_outflow(BudgetOwner::Character(1), "broker_fee", -10.0, "Broker Fee"),
        journal_outflow(BudgetOwner::Character(1), "tax", -10.0, "Sales Tax"),
      ];
      let counted = rule(
        1,
        true,
        MatchMode::All,
        vec![condition(RuleField::Text, RuleOp::Contains, "sales")],
      );

      assert_eq!(match_count(&counted, &outflows), 2);
    }
  }

  mod preview_entries {
    use pretty_assertions::assert_eq;

    use super::*;

    fn fixture() -> Vec<MatchTarget> {
      vec![
        journal_outflow(BudgetOwner::Character(1), "tax", -10.0, "Sales Tax"),
        journal_outflow(BudgetOwner::Character(1), "tax", -20.0, "Sales Tax"),
        journal_outflow(BudgetOwner::Character(1), "tax", -30.0, "Sales Tax"),
      ]
    }

    #[test]
    fn it_classifies_assign_manual_and_preempted() {
      let outflows = fixture();
      let draft = rule(
        99,
        true,
        MatchMode::All,
        vec![condition(RuleField::Text, RuleOp::Contains, "sales")],
      );
      let higher = rule(
        7,
        true,
        MatchMode::All,
        vec![condition(RuleField::Amount, RuleOp::GreaterThan, "25")],
      );
      let live = vec![higher, draft.clone()];
      let manual = HashMap::from([(0usize, 5i64)]);

      let preview = preview_entries(&draft, &live, &manual, 99, &outflows);

      assert_eq!(
        preview,
        vec![
          (0, PreviewStatus::Manual),
          (1, PreviewStatus::Assign),
          (2, PreviewStatus::Preempted),
        ]
      );
    }

    #[test]
    fn it_lets_the_draft_win_over_a_lower_priority_rule() {
      let outflows = fixture();
      let draft = rule(
        99,
        true,
        MatchMode::All,
        vec![condition(RuleField::Text, RuleOp::Contains, "sales")],
      );
      let lower = rule(
        7,
        true,
        MatchMode::All,
        vec![condition(RuleField::Amount, RuleOp::GreaterThan, "25")],
      );
      let live = vec![draft.clone(), lower];

      let preview = preview_entries(&draft, &live, &HashMap::new(), 99, &outflows);

      assert_eq!(
        preview,
        vec![
          (0, PreviewStatus::Assign),
          (1, PreviewStatus::Assign),
          (2, PreviewStatus::Assign),
        ]
      );
    }

    #[test]
    fn it_appends_a_new_draft_at_lowest_priority() {
      let outflows = fixture();
      let draft = rule(
        0,
        true,
        MatchMode::All,
        vec![condition(RuleField::Text, RuleOp::Contains, "sales")],
      );
      let existing = rule(
        7,
        true,
        MatchMode::All,
        vec![condition(RuleField::Amount, RuleOp::GreaterThan, "25")],
      );
      let live = vec![existing];

      let preview = preview_entries(&draft, &live, &HashMap::new(), 99, &outflows);

      assert_eq!(preview[2], (2, PreviewStatus::Preempted));
    }

    #[test]
    fn it_classifies_already_when_a_higher_priority_same_category_rule_claims_it() {
      let outflows = fixture();
      let draft = rule(
        99,
        true,
        MatchMode::All,
        vec![condition(RuleField::Text, RuleOp::Contains, "sales")],
      );
      let same_category = Rule {
        category_id: 99,
        conditions: vec![condition(RuleField::Amount, RuleOp::GreaterThan, "25")],
        enabled: true,
        id: 7,
        match_mode: MatchMode::All,
        name: String::new(),
      };
      let live = vec![same_category, draft.clone()];

      let preview = preview_entries(&draft, &live, &HashMap::new(), 99, &outflows);

      assert_eq!(preview[2], (2, PreviewStatus::Already));
    }
  }

  mod suggest_name {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_the_first_active_conditions_value() {
      let by_text = rule(
        1,
        true,
        MatchMode::All,
        vec![condition(RuleField::Text, RuleOp::Contains, "Cerberus")],
      );

      assert_eq!(suggest_name(&by_text, |_| None, |_| None), "Cerberus");
    }

    #[test]
    fn it_resolves_type_and_character_labels() {
      let by_type = rule(
        1,
        true,
        MatchMode::All,
        vec![condition(RuleField::Type, RuleOp::Is, "broker_fee")],
      );
      let by_char = rule(
        1,
        true,
        MatchMode::All,
        vec![condition(RuleField::Character, RuleOp::Is, "42")],
      );

      assert_eq!(
        suggest_name(
          &by_type,
          |key| (key == "broker_fee").then(|| "Broker Fees".to_owned()),
          |_| None
        ),
        "Broker Fees"
      );
      assert_eq!(
        suggest_name(&by_char, |_| None, |key| (key == "42").then(|| "Aaron".to_owned())),
        "Aaron"
      );
    }

    #[test]
    fn it_describes_amount_and_direction_conditions() {
      let by_amount = rule(
        1,
        true,
        MatchMode::All,
        vec![condition(RuleField::Amount, RuleOp::GreaterThan, "100m")],
      );
      let by_direction = rule(
        1,
        true,
        MatchMode::All,
        vec![condition(RuleField::Direction, RuleOp::Is, "in")],
      );

      assert_eq!(suggest_name(&by_amount, |_| None, |_| None), "Amount is over 100m");
      assert_eq!(suggest_name(&by_direction, |_| None, |_| None), "Inflows");
    }

    #[test]
    fn it_returns_empty_for_a_rule_with_no_active_conditions() {
      let empty = rule(
        1,
        true,
        MatchMode::All,
        vec![condition(RuleField::Text, RuleOp::Contains, "")],
      );

      assert_eq!(suggest_name(&empty, |_| None, |_| None), "");
    }
  }

  mod summarize_rule {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_joins_active_conditions_with_the_match_mode_word() {
      let any = rule(
        1,
        true,
        MatchMode::Any,
        vec![
          condition(RuleField::Reference, RuleOp::Contains, "Cerberus"),
          condition(RuleField::Item, RuleOp::Contains, "Caracal"),
        ],
      );

      assert_eq!(
        summarize_rule(&any, |_| None, |_| None),
        "Reference contains \u{201c}Cerberus\u{201d} or Item contains \u{201c}Caracal\u{201d}"
      );
    }

    #[test]
    fn it_renders_type_and_character_through_the_resolvers() {
      let typed = rule(
        1,
        true,
        MatchMode::All,
        vec![condition(RuleField::Type, RuleOp::Is, "broker_fee")],
      );
      let by_char = rule(
        1,
        true,
        MatchMode::All,
        vec![condition(RuleField::Character, RuleOp::Is, "42")],
      );

      assert_eq!(
        summarize_rule(
          &typed,
          |key| (key == "broker_fee").then(|| "Broker Fee".to_owned()),
          |_| None
        ),
        "Type is Broker Fee"
      );
      assert_eq!(
        summarize_rule(&by_char, |_| None, |key| (key == "42").then(|| "Aaron".to_owned())),
        "Character is Aaron"
      );
    }

    #[test]
    fn it_summarizes_an_amount_between_with_both_bounds() {
      let amount = rule(1, true, MatchMode::All, vec![between("100m", "1b")]);

      assert_eq!(
        summarize_rule(&amount, |_| None, |_| None),
        "Amount is between 100m and 1b"
      );
    }

    #[test]
    fn it_falls_back_when_there_are_no_active_conditions() {
      let empty = rule(
        1,
        true,
        MatchMode::All,
        vec![condition(RuleField::Text, RuleOp::Contains, "")],
      );

      assert_eq!(summarize_rule(&empty, |_| None, |_| None), "No conditions yet");
    }
  }

  mod new_condition {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_seeds_a_field_with_its_default_operator_and_empty_value() {
      let text = new_condition(RuleField::Text);

      assert_eq!(text.field(), RuleField::Text);
      assert_eq!(text.op(), RuleOp::Contains);
      assert_eq!(text.value(), "");
    }

    #[test]
    fn it_defaults_direction_to_outflow() {
      let direction = new_condition(RuleField::Direction);

      assert_eq!(direction.value(), "out");
    }

    #[test]
    fn it_seeds_an_upper_bound_when_the_default_op_is_between() {
      let amount = new_condition(RuleField::Amount);

      assert_eq!(amount.op(), RuleOp::GreaterThan);
      assert_eq!(amount.value2(), &None);
    }
  }

  mod field_kind {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_each_field_to_its_editor_kind() {
      assert_eq!(field_kind(RuleField::Amount), FieldKind::Amount);
      assert_eq!(field_kind(RuleField::Type), FieldKind::Type);
      assert_eq!(field_kind(RuleField::Character), FieldKind::Character);
      assert_eq!(field_kind(RuleField::Direction), FieldKind::Direction);
      assert_eq!(field_kind(RuleField::Reference), FieldKind::Text);
    }
  }

  mod ops_for_field {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_offers_the_default_op_first_for_each_field() {
      assert_eq!(ops_for_field(RuleField::Amount).first(), Some(&RuleOp::GreaterThan));
      assert_eq!(ops_for_field(RuleField::Type).first(), Some(&RuleOp::Is));
      assert_eq!(ops_for_field(RuleField::Reference).first(), Some(&RuleOp::Contains));
    }
  }

  mod journal_match_text {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_unions_the_humanized_label_reason_and_description() {
      let text = journal_match_text("daily_goal_payouts", Some("project bonus"), "Payout for goals");

      assert_eq!(text, "Daily Goal Payouts project bonus Payout for goals");
    }

    #[test]
    fn it_finds_a_humanized_word_not_present_in_the_raw_ref_type() {
      let text = journal_match_text("daily_goal_payouts", None, "");

      assert!(text.to_lowercase().contains("daily"));
    }

    #[test]
    fn it_skips_an_absent_reason_and_empty_description() {
      let text = journal_match_text("broker_fee", None, "");

      assert_eq!(text, "Broker Fee");
    }
  }

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
    fn it_derives_ready_to_assign_as_pool_minus_held_availables() {
      // 600 held across three envelopes; the rest of the 1,000 liquid is free.
      let summary = pool_summary(1_000.0, [300.0, 200.0, 100.0]);

      assert_eq!(summary.pool, 1_000.0);
      assert_eq!(summary.ready_to_assign, 400.0);
      assert_eq!(summary.overspent, 0.0);
    }

    #[test]
    fn it_conserves_liquid_across_ready_to_assign_and_held() {
      // The invariant: pool = ready_to_assign + Σ max(0, available).
      let availables = [300.0, 250.0, 50.0];
      let summary = pool_summary(1_000.0, availables);

      let held: f64 = availables.iter().filter(|a| **a > 0.0).sum();
      assert_eq!(summary.ready_to_assign + held, summary.pool);
    }

    #[test]
    fn it_excludes_overspent_envelopes_from_held_and_reports_them() {
      // A negative available is overspend: it shows red and never inflates RTA.
      let summary = pool_summary(500.0, [300.0, -150.0, -50.0]);

      assert_eq!(summary.overspent, -200.0);
      // ready = 500 − 300 held; the −200 overspend does not credit back into RTA.
      assert_eq!(summary.ready_to_assign, 200.0);
    }

    #[test]
    fn it_can_report_a_negative_ready_to_assign_when_over_held() {
      // Holding more in envelopes than the liquid pool over-draws RTA.
      let summary = pool_summary(100.0, [80.0, 80.0]);

      assert_eq!(summary.ready_to_assign, -60.0);
    }

    #[test]
    fn it_conserves_money_across_assign_spend_and_overspend() {
      // Three envelopes after a month of activity (a transfer contributes no
      // available, having been excluded upstream):
      //   assigned-and-untouched: carry 0 + assigned 200 + activity 0   = 200
      //   spent-down:             carry 0 + assigned 200 + activity −150 =  50
      //   overspent:              carry 0 + assigned 100 + activity −180 = −80
      let availables = [200.0, 50.0, -80.0];
      let summary = pool_summary(1_000.0, availables);

      // RTA reflects only the ISK still held (200 + 50); overspend is reported
      // separately and never credits back into RTA.
      assert_eq!(summary.ready_to_assign, 750.0);
      assert_eq!(summary.overspent, -80.0);

      let held: f64 = availables.iter().filter(|a| **a > 0.0).sum();
      assert_eq!(summary.ready_to_assign + held, summary.pool);
    }
  }

  mod budget_flow {
    mod from_market {
      use pretty_assertions::assert_eq;

      use super::super::*;

      #[test]
      fn it_classifies_a_buy_as_expense_and_a_sell_as_income() {
        assert_eq!(BudgetFlow::from_market(true), BudgetFlow::Expense);
        assert_eq!(BudgetFlow::from_market(false), BudgetFlow::Income);
      }
    }

    mod from_ref_type {
      use pretty_assertions::assert_eq;

      use super::super::*;

      #[test]
      fn it_classifies_a_positive_amount_as_income_and_a_negative_amount_as_expense() {
        assert_eq!(BudgetFlow::from_ref_type("bounty_prizes", 1_000.0), BudgetFlow::Income);
        assert_eq!(BudgetFlow::from_ref_type("brokers_fee", -120.0), BudgetFlow::Expense);
      }

      #[test]
      fn it_classifies_a_refund_ref_type_as_refund_regardless_of_sign() {
        assert_eq!(
          BudgetFlow::from_ref_type("industry_job_refund", 500.0),
          BudgetFlow::Refund
        );
        assert_eq!(
          BudgetFlow::from_ref_type("contract_reward_refund", 500.0),
          BudgetFlow::Refund
        );
      }

      #[test]
      fn it_classifies_an_ambiguous_transfer_ref_type_by_sign_without_a_counter_leg() {
        assert_eq!(
          BudgetFlow::from_ref_type("player_donation", 1_000.0),
          BudgetFlow::Income
        );
        assert_eq!(
          BudgetFlow::from_ref_type("player_donation", -1_000.0),
          BudgetFlow::Expense
        );
      }

      #[test]
      fn it_classifies_a_zero_amount_as_income() {
        assert_eq!(BudgetFlow::from_ref_type("bounty_prizes", 0.0), BudgetFlow::Income);
      }
    }
  }

  mod classify_journal {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;

    use super::*;

    fn row(id: i64, owner: BudgetOwner, ref_type: &str, amount: f64) -> JournalActivity {
      JournalActivity {
        amount: Some(amount),
        context_id: None,
        context_id_type: None,
        date: "2026-06-01T00:00:00Z".to_owned(),
        id,
        owner,
        ref_type: ref_type.to_owned(),
        text: String::new(),
      }
    }

    #[test]
    fn it_classifies_a_detected_transfer_id_as_internal_transfer() {
      let leg = row(900, BudgetOwner::Character(1), "player_donation", 10.0);
      let transfer_ids = HashSet::from([900]);

      assert_eq!(classify_journal(&leg, &transfer_ids), BudgetFlow::InternalTransfer);
    }

    #[test]
    fn it_classifies_an_undetected_row_statically_by_ref_type_and_sign() {
      let income = row(1, BudgetOwner::Character(1), "bounty_prizes", 1_000.0);
      let expense = row(2, BudgetOwner::Character(1), "brokers_fee", -120.0);
      let refund = row(3, BudgetOwner::Character(1), "industry_job_refund", 500.0);
      let empty = HashSet::new();

      assert_eq!(classify_journal(&income, &empty), BudgetFlow::Income);
      assert_eq!(classify_journal(&expense, &empty), BudgetFlow::Expense);
      assert_eq!(classify_journal(&refund, &empty), BudgetFlow::Refund);
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

  mod internal_transfer_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    fn row(id: i64, owner: BudgetOwner, ref_type: &str, amount: f64) -> JournalActivity {
      JournalActivity {
        amount: Some(amount),
        context_id: None,
        context_id_type: None,
        date: "2026-06-01T00:00:00Z".to_owned(),
        id,
        owner,
        ref_type: ref_type.to_owned(),
        text: String::new(),
      }
    }

    #[test]
    fn it_detects_a_two_leg_opposite_sign_transfer_across_distinct_owners() {
      let rows = vec![
        row(900, BudgetOwner::Character(1), "corporation_account_withdrawal", 10.0),
        row(
          900,
          BudgetOwner::Corporation(98),
          "corporation_account_withdrawal",
          -10.0,
        ),
      ];

      let ids = internal_transfer_ids(&rows);

      assert_eq!(ids, std::collections::HashSet::from([900]));
    }

    #[test]
    fn it_does_not_collide_a_character_and_corp_sharing_an_eve_id() {
      // Two unrelated single legs that happen to share EVE journal id 900: one a
      // character donation in, one a corp donation out. The bare-id key would have
      // paired and cancelled them; grouping by (ref_type, id) plus the
      // distinct-owner pairing must still treat each as one standalone leg.
      let rows = vec![
        row(900, BudgetOwner::Character(1), "player_donation", 10.0),
        row(900, BudgetOwner::Corporation(98), "contract_price", -10.0),
      ];

      let ids = internal_transfer_ids(&rows);

      assert!(ids.is_empty());
    }

    #[test]
    fn it_ignores_a_group_with_three_or_more_legs() {
      let rows = vec![
        row(900, BudgetOwner::Character(1), "player_donation", 10.0),
        row(900, BudgetOwner::Corporation(98), "player_donation", -10.0),
        row(900, BudgetOwner::Character(2), "player_donation", -10.0),
      ];

      let ids = internal_transfer_ids(&rows);

      assert!(ids.is_empty());
    }

    #[test]
    fn it_ignores_two_legs_in_the_same_wallet() {
      let rows = vec![
        row(900, BudgetOwner::Character(1), "player_donation", 10.0),
        row(900, BudgetOwner::Character(1), "player_donation", -10.0),
      ];

      let ids = internal_transfer_ids(&rows);

      assert!(ids.is_empty());
    }

    #[test]
    fn it_ignores_two_same_sign_legs() {
      let rows = vec![
        row(900, BudgetOwner::Character(1), "player_donation", 10.0),
        row(900, BudgetOwner::Corporation(98), "player_donation", 10.0),
      ];

      let ids = internal_transfer_ids(&rows);

      assert!(ids.is_empty());
    }

    #[test]
    fn it_ignores_non_transfer_ref_types() {
      let rows = vec![
        row(900, BudgetOwner::Character(1), "bounty_prizes", 10.0),
        row(900, BudgetOwner::Corporation(98), "bounty_prizes", -10.0),
      ];

      let ids = internal_transfer_ids(&rows);

      assert!(ids.is_empty());
    }
  }

  mod seed_scope {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, model::NewGroup, repo::budget};

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
    async fn it_does_not_reseed_after_every_group_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_scope(&db, BudgetScope::All).await.unwrap();
      for group in budget::list_groups(&db, BudgetScope::All).await.unwrap() {
        budget::delete_group(&db, group.id()).await.unwrap();
      }

      seed_scope(&db, BudgetScope::All).await.unwrap();

      assert!(budget::list_groups(&db, BudgetScope::All).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_adopts_a_legacy_seeded_scope_without_re_seeding() {
      let db = store::open_test().await.unwrap();
      // Simulate a scope seeded before the marker existed: groups present, no marker.
      budget::create_group(
        &db,
        &NewGroup {
          name: "Pre-existing".to_owned(),
          position: 0,
          scope: BudgetScope::All,
        },
      )
      .await
      .unwrap();

      seed_scope(&db, BudgetScope::All).await.unwrap();

      let groups = budget::list_groups(&db, BudgetScope::All).await.unwrap();
      assert_eq!(groups.len(), 1);
      assert!(budget::is_scope_seeded(&db, BudgetScope::All).await.unwrap());
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

    use super::{
      monthly_activity::{journal, seed_character},
      *,
    };
    use crate::store::{
      self,
      repo::{budget, finance},
    };

    #[tokio::test]
    async fn it_lazy_seeds_an_unseeded_scope_on_first_assignment() {
      // A fresh DB seeds a scope's categories in a deterministic order, so the
      // income category id discovered on a probe DB is valid on a second fresh DB
      // where `assign_entry` performs the lazy seed itself.
      let probe = store::open_test().await.unwrap();
      seed_scope(&probe, BudgetScope::Character(1)).await.unwrap();
      let income_id = slug_to_category_id(&probe, BudgetScope::Character(1)).await["income"];

      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      finance::append_wallet_journal(&db, &[journal(5, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z")])
        .await
        .unwrap();
      assert!(
        budget::list_groups(&db, BudgetScope::Character(1))
          .await
          .unwrap()
          .is_empty()
      );

      let saved = assign_entry(
        &db,
        BudgetScope::Character(1),
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        5,
        income_id,
      )
      .await
      .unwrap();

      assert!(
        !budget::list_groups(&db, BudgetScope::Character(1))
          .await
          .unwrap()
          .is_empty()
      );
      assert_eq!(saved.expect("entry held by owner").category_id(), income_id);
      assert_eq!(
        resolve_entry_category(
          &db,
          BudgetScope::Character(1),
          BudgetOwner::Character(1),
          BudgetEntryKind::Journal,
          5,
          Some("manufacturing"),
          None
        )
        .await,
        Some(income_id)
      );
    }

    #[tokio::test]
    async fn it_skips_a_copy_for_an_owner_that_does_not_hold_the_entry() {
      use crate::store::{
        model::{Corporation, CorporationWalletJournal, OwnerType},
        repo::infra,
      };

      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      infra::upsert(&db, 1, OwnerType::Character, "tok", "rt", 9_999, None, None)
        .await
        .unwrap();
      let corp_id = 98_000_020;
      let mut corp = Corporation::new(corp_id, "Owned Corp", "OWN");
      corp.set_ceo_id(1);
      corp.set_creator_id(1);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      store::repo::org::upsert_corporation(&db, &corp).await.unwrap();
      infra::upsert(&db, corp_id, OwnerType::Corporation, "tok", "rt", 9_999, Some(1), None)
        .await
        .unwrap();
      store::repo::finance::upsert_divisions(
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
      seed_scope(&db, BudgetScope::All).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::All).await;

      // Journal id 9 exists only in the corp wallet; no character holds it.
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
          id: 9,
          reason: None,
          ref_type: "industry_job_tax".to_owned(),
          second_party_id: None,
          tax: None,
          tax_receiver_id: None,
        }],
      )
      .await
      .unwrap();

      let mis_owned = assign_entry(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        9,
        slug_to_id["income"],
      )
      .await
      .unwrap();
      let genuine = assign_entry(
        &db,
        BudgetScope::All,
        BudgetOwner::Corporation(corp_id),
        BudgetEntryKind::Journal,
        9,
        slug_to_id["tithe"],
      )
      .await
      .unwrap();

      assert!(mis_owned.is_none());
      assert!(genuine.is_some());
      let assignments = budget::list_entry_assignments(&db, BudgetScope::All).await.unwrap();
      assert_eq!(assignments.len(), 1);
      assert_eq!(assignments[0].owner_kind(), "corporation");
    }

    #[tokio::test]
    async fn it_co_assigns_a_trade_present_in_both_wallets() {
      use crate::store::{
        model::{Corporation, CorporationWalletTransaction, OwnerType},
        repo::infra,
      };

      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      infra::upsert(&db, 1, OwnerType::Character, "tok", "rt", 9_999, None, None)
        .await
        .unwrap();
      let corp_id = 98_000_021;
      let mut corp = Corporation::new(corp_id, "Owned Corp", "OWN");
      corp.set_ceo_id(1);
      corp.set_creator_id(1);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      store::repo::org::upsert_corporation(&db, &corp).await.unwrap();
      infra::upsert(&db, corp_id, OwnerType::Corporation, "tok", "rt", 9_999, Some(1), None)
        .await
        .unwrap();
      store::repo::finance::upsert_divisions(
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
      seed_scope(&db, BudgetScope::All).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::All).await;

      // The same transaction_id 700 lands in BOTH the character and corp wallet.
      finance::append_wallet_transaction(
        &db,
        &[crate::store::model::CharacterWalletTransaction {
          character_id: 1,
          client_id: 1_000_035,
          date: "2026-06-09T00:00:00Z".to_owned(),
          is_buy: false,
          is_personal: false,
          journal_ref_id: 0,
          location_id: 60_003_760,
          quantity: 10,
          transaction_id: 700,
          type_id: 34,
          unit_price: 100.0,
        }],
      )
      .await
      .unwrap();
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
          quantity: 10,
          transaction_id: 700,
          type_id: 34,
          unit_price: 100.0,
        }],
      )
      .await
      .unwrap();

      let character_copy = assign_entry(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Market,
        700,
        slug_to_id["income"],
      )
      .await
      .unwrap();
      let corp_copy = assign_entry(
        &db,
        BudgetScope::All,
        BudgetOwner::Corporation(corp_id),
        BudgetEntryKind::Market,
        700,
        slug_to_id["income"],
      )
      .await
      .unwrap();

      assert!(character_copy.is_some());
      assert!(corp_copy.is_some());
      assert_eq!(
        budget::list_entry_assignments(&db, BudgetScope::All)
          .await
          .unwrap()
          .len(),
        2
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
      // Written through the storage primitive so this resolution test is independent
      // of `assign_entry`'s wallet-ownership guard.
      budget::upsert_entry_assignment(
        &db,
        scope,
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        5,
        slug_to_id["income"],
      )
      .await
      .unwrap();

      assert_eq!(
        resolve_entry_category(
          &db,
          scope,
          BudgetOwner::Character(1),
          BudgetEntryKind::Journal,
          5,
          Some("manufacturing"),
          None
        )
        .await,
        Some(slug_to_id["income"])
      );
    }

    #[tokio::test]
    async fn it_keys_an_override_to_its_owner_under_all_scope() {
      let db = store::open_test().await.unwrap();
      let scope = BudgetScope::All;
      seed_scope(&db, scope).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, scope).await;

      // Two owners share EVE journal id 5; only the character's entry is overridden.
      // Written through the storage primitive so this resolution test is independent
      // of `assign_entry`'s wallet-ownership guard.
      budget::upsert_entry_assignment(
        &db,
        scope,
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        5,
        slug_to_id["income"],
      )
      .await
      .unwrap();

      assert_eq!(
        resolve_entry_category(
          &db,
          scope,
          BudgetOwner::Character(1),
          BudgetEntryKind::Journal,
          5,
          Some("manufacturing"),
          None
        )
        .await,
        Some(slug_to_id["income"])
      );
      // The corporation's same-id entry is untouched and falls back to its ref_type default.
      assert_eq!(
        resolve_entry_category(
          &db,
          scope,
          BudgetOwner::Corporation(2),
          BudgetEntryKind::Journal,
          5,
          Some("manufacturing"),
          None
        )
        .await,
        Some(slug_to_id["industry"])
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
        resolve_entry_category(
          &db,
          scope,
          BudgetOwner::Character(1),
          BudgetEntryKind::Journal,
          9,
          Some("bounty_prizes"),
          None
        )
        .await,
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
        resolve_entry_category(
          &db,
          scope,
          BudgetOwner::Character(1),
          BudgetEntryKind::Journal,
          1,
          Some("bounty_prizes"),
          None
        )
        .await,
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
        resolve_entry_category(
          &db,
          scope,
          BudgetOwner::Character(1),
          BudgetEntryKind::Market,
          10,
          None,
          Some(true)
        )
        .await,
        Some(slug_to_id["trading"])
      );
      assert_eq!(
        resolve_entry_category(
          &db,
          scope,
          BudgetOwner::Character(1),
          BudgetEntryKind::Market,
          11,
          None,
          Some(false)
        )
        .await,
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
          BudgetOwner::Character(1),
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
      repo::{character::insert_with_org, finance},
    };

    pub(super) async fn seed_character(db: &Database, id: i64) {
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

    pub(super) fn journal(
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

    pub(super) fn linked_journal(
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

    pub(super) fn transaction(
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
    async fn it_counts_only_assigned_journal_entries() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      finance::append_wallet_journal(
        &db,
        &[
          journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z"),
          journal(2, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z"),
          // Unassigned — stays in Ready-to-Assign, never counted.
          journal(3, 1, "bounty_prizes", 500.0, "2026-06-20T00:00:00Z"),
        ],
      )
      .await
      .unwrap();
      assign_entry(
        &db,
        BudgetScope::Character(1),
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        1,
        slug_to_id["income"],
      )
      .await
      .unwrap();
      assign_entry(
        &db,
        BudgetScope::Character(1),
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        2,
        slug_to_id["fees"],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["income"]), Some(&1_000.0));
      assert_eq!(activity.get(&slug_to_id["fees"]), Some(&-120.0));
      assert_eq!(activity.len(), 2);
    }

    #[tokio::test]
    async fn it_does_not_count_unassigned_entries() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      finance::append_wallet_journal(&db, &[journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z")])
        .await
        .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;

      assert!(activity.is_empty());
    }

    #[tokio::test]
    async fn it_excludes_an_assigned_entry_outside_the_month() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      finance::append_wallet_journal(&db, &[journal(1, 1, "bounty_prizes", 9_999.0, "2026-05-31T23:59:59Z")])
        .await
        .unwrap();
      assign_entry(
        &db,
        BudgetScope::Character(1),
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        1,
        slug_to_id["income"],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;

      assert!(activity.is_empty());
    }

    #[tokio::test]
    async fn it_assigns_a_journal_entry_to_a_category() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      finance::append_wallet_journal(&db, &[journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z")])
        .await
        .unwrap();
      assign_entry(
        &db,
        BudgetScope::Character(1),
        BudgetOwner::Character(1),
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
    async fn it_counts_assigned_corp_division_journals_for_a_corp_scope() {
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
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Corporation(corp_id)).await;
      assign_entry(
        &db,
        BudgetScope::Corporation(corp_id),
        BudgetOwner::Corporation(corp_id),
        BudgetEntryKind::Journal,
        1,
        slug_to_id["tithe"],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Corporation(corp_id), "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["tithe"]), Some(&-2_000.0));
    }

    #[tokio::test]
    async fn it_counts_an_assigned_market_trade_once() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      // A sale: a transaction row plus its journal twin. Assigning the transaction
      // counts the trade once; the twin is suppressed so it cannot double-count.
      finance::append_wallet_transaction(&db, &[transaction(500, 1, false, 100.0, 10, "2026-06-05T00:00:00Z")])
        .await
        .unwrap();
      finance::append_wallet_journal(
        &db,
        &[linked_journal(
          10,
          1,
          "market_transaction",
          1_000.0,
          500,
          "2026-06-05T00:00:00Z",
        )],
      )
      .await
      .unwrap();
      assign_entry(
        &db,
        BudgetScope::Character(1),
        BudgetOwner::Character(1),
        BudgetEntryKind::Market,
        500,
        slug_to_id["income"],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["income"]), Some(&1_000.0));
      assert_eq!(activity.len(), 1);
    }

    #[tokio::test]
    async fn it_assigns_a_market_trade_to_a_category() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      // A buy spends, so its signed amount is negative.
      finance::append_wallet_transaction(&db, &[transaction(600, 1, true, 100.0, 5, "2026-06-07T00:00:00Z")])
        .await
        .unwrap();
      assign_entry(
        &db,
        BudgetScope::Character(1),
        BudgetOwner::Character(1),
        BudgetEntryKind::Market,
        600,
        slug_to_id["fees"],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["fees"]), Some(&-500.0));
    }

    #[tokio::test]
    async fn it_counts_an_assigned_corp_transaction_once() {
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
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Corporation(corp_id)).await;
      assign_entry(
        &db,
        BudgetScope::Corporation(corp_id),
        BudgetOwner::Corporation(corp_id),
        BudgetEntryKind::Market,
        700,
        slug_to_id["income"],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::Corporation(corp_id), "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["income"]), Some(&1_000.0));
      assert_eq!(activity.len(), 1);
    }

    #[tokio::test]
    async fn it_routes_two_owners_sharing_an_eve_id_to_their_own_categories_under_all_scope() {
      use crate::store::{model::OwnerType, repo::infra};

      let db = store::open_test().await.unwrap();
      // An owned character and an owned corporation, both in the All scope.
      seed_character(&db, 1).await;
      infra::upsert(&db, 1, OwnerType::Character, "tok", "rt", 9_999, None, None)
        .await
        .unwrap();
      let corp_id = 98_000_010;
      let mut corp = Corporation::new(corp_id, "Owned Corp", "OWN");
      corp.set_ceo_id(1);
      corp.set_creator_id(1);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      store::repo::org::upsert_corporation(&db, &corp).await.unwrap();
      infra::upsert(&db, corp_id, OwnerType::Corporation, "tok", "rt", 9_999, Some(1), None)
        .await
        .unwrap();
      store::repo::finance::upsert_divisions(
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
      seed_scope(&db, BudgetScope::All).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::All).await;

      // The character and the corporation both carry journal id 5 — the cross-owner
      // collision. Each is assigned to a different category for its own owner.
      finance::append_wallet_journal(&db, &[journal(5, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z")])
        .await
        .unwrap();
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
          id: 5,
          reason: None,
          ref_type: "industry_job_tax".to_owned(),
          second_party_id: None,
          tax: None,
          tax_receiver_id: None,
        }],
      )
      .await
      .unwrap();
      assign_entry(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        5,
        slug_to_id["income"],
      )
      .await
      .unwrap();
      assign_entry(
        &db,
        BudgetScope::All,
        BudgetOwner::Corporation(corp_id),
        BudgetEntryKind::Journal,
        5,
        slug_to_id["tithe"],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::All, "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["income"]), Some(&1_000.0));
      assert_eq!(activity.get(&slug_to_id["tithe"]), Some(&-2_000.0));
      assert_eq!(activity.len(), 2);
    }

    #[tokio::test]
    async fn it_counts_a_corp_mirrored_market_trade_once_under_all_scope() {
      use crate::store::{model::OwnerType, repo::infra};

      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      infra::upsert(&db, 1, OwnerType::Character, "tok", "rt", 9_999, None, None)
        .await
        .unwrap();
      let corp_id = 98_000_011;
      let mut corp = Corporation::new(corp_id, "Owned Corp", "OWN");
      corp.set_ceo_id(1);
      corp.set_creator_id(1);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      store::repo::org::upsert_corporation(&db, &corp).await.unwrap();
      infra::upsert(&db, corp_id, OwnerType::Corporation, "tok", "rt", 9_999, Some(1), None)
        .await
        .unwrap();
      store::repo::finance::upsert_divisions(
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
      seed_scope(&db, BudgetScope::All).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::All).await;

      // One sell trade surfaces three times: the corp transaction, its mirror in the
      // trader's personal wallet (same transaction_id 700), and the corp journal twin.
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
          quantity: 10,
          transaction_id: 700,
          type_id: 34,
          unit_price: 100.0,
        }],
      )
      .await
      .unwrap();
      finance::append_wallet_transaction(&db, &[transaction(700, 1, false, 100.0, 10, "2026-06-09T00:00:00Z")])
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
      // Both the corp transaction and its character mirror are assigned to the same
      // category; the trade must still contribute exactly once, not twice or thrice.
      assign_entry(
        &db,
        BudgetScope::All,
        BudgetOwner::Corporation(corp_id),
        BudgetEntryKind::Market,
        700,
        slug_to_id["trading"],
      )
      .await
      .unwrap();
      assign_entry(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Market,
        700,
        slug_to_id["trading"],
      )
      .await
      .unwrap();

      let activity = monthly_activity(&db, BudgetScope::All, "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["trading"]), Some(&1_000.0));
      assert_eq!(activity.len(), 1);
    }

    #[tokio::test]
    async fn it_excludes_internal_transfer_legs_sharing_a_journal_id() {
      use crate::store::{model::OwnerType, repo::infra};

      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      infra::upsert(&db, 1, OwnerType::Character, "tok", "rt", 9_999, None, None)
        .await
        .unwrap();
      let corp_id = 98_000_020;
      let mut corp = Corporation::new(corp_id, "Owned Corp", "OWN");
      corp.set_ceo_id(1);
      corp.set_creator_id(1);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      store::repo::org::upsert_corporation(&db, &corp).await.unwrap();
      infra::upsert(&db, corp_id, OwnerType::Corporation, "tok", "rt", 9_999, Some(1), None)
        .await
        .unwrap();
      store::repo::finance::upsert_divisions(
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
      seed_scope(&db, BudgetScope::All).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::All).await;

      // One internal transfer: the SAME EVE journal id 900 mirrored into the corp
      // wallet (-10B leg) and the trading character's personal wallet (+10B leg).
      finance::append_corporation_wallet_journal(
        &db,
        &[CorporationWalletJournal {
          amount: Some(-10_000_000_000.0),
          balance: Some(0.0),
          context_id: None,
          context_id_type: None,
          corporation_id: corp_id,
          date: "2026-06-12T00:00:00Z".to_owned(),
          description: "Transfer out".to_owned(),
          division: 1,
          first_party_id: None,
          id: 900,
          reason: None,
          ref_type: "corporation_account_withdrawal".to_owned(),
          second_party_id: None,
          tax: None,
          tax_receiver_id: None,
        }],
      )
      .await
      .unwrap();
      finance::append_wallet_journal(
        &db,
        &[journal(
          900,
          1,
          "corporation_account_withdrawal",
          10_000_000_000.0,
          "2026-06-12T00:00:00Z",
        )],
      )
      .await
      .unwrap();
      // Even when the inflow leg is filed into an envelope — exactly the bug shape
      // that drove Ready-to-Assign negative — an internal transfer moves no ISK in
      // or out of the user's holdings, so both legs are excluded from category
      // activity entirely rather than reserved as a phantom inflow.
      assign_entry(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        900,
        slug_to_id["income"],
      )
      .await
      .unwrap();

      let by_month = activity_by_month(&db, BudgetScope::All).await;
      let activity = by_month.get("2026-06").cloned().unwrap_or_default();

      assert_eq!(activity.get(&slug_to_id["income"]), None);
      assert!(activity.is_empty());
    }

    #[tokio::test]
    async fn it_groups_assigned_activity_by_month_in_one_batched_pass() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      finance::append_wallet_journal(
        &db,
        &[
          journal(1, 1, "bounty_prizes", 1_000.0, "2026-04-02T00:00:00Z"),
          journal(2, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z"),
        ],
      )
      .await
      .unwrap();
      assign_entry(
        &db,
        BudgetScope::Character(1),
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        1,
        slug_to_id["income"],
      )
      .await
      .unwrap();
      assign_entry(
        &db,
        BudgetScope::Character(1),
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        2,
        slug_to_id["fees"],
      )
      .await
      .unwrap();

      let by_month = activity_by_month(&db, BudgetScope::Character(1)).await;

      assert_eq!(by_month["2026-04"].get(&slug_to_id["income"]), Some(&1_000.0));
      assert_eq!(by_month["2026-06"].get(&slug_to_id["fees"]), Some(&-120.0));
      assert!(!by_month.contains_key("2026-05"));
      // The single-month wrapper agrees with the batched map for that month.
      assert_eq!(
        by_month["2026-04"],
        monthly_activity(&db, BudgetScope::Character(1), "2026-04").await
      );
    }

    pub(super) async fn text_rule(
      db: &Database,
      scope: BudgetScope,
      category_id: i64,
      enabled: bool,
      position: i64,
      needle: &str,
    ) {
      use crate::store::{
        model::NewRule,
        repo::budget::{create_rule, replace_rule_conditions},
      };
      let created = create_rule(
        db,
        &NewRule {
          category_id,
          enabled,
          match_mode: MatchMode::All,
          name: needle.to_owned(),
          position,
          scope,
        },
      )
      .await
      .unwrap();
      replace_rule_conditions(
        db,
        created.id(),
        &[RuleCondition {
          field: RuleField::Text,
          op: RuleOp::Contains,
          value: needle.to_owned(),
          value2: None,
        }],
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_auto_assigns_a_matching_outflow_via_a_rule_retroactively() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      finance::append_wallet_journal(&db, &[journal(1, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z")])
        .await
        .unwrap();

      // No rules yet: the outflow stays in Ready-to-Assign.
      let before = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;
      assert!(before.is_empty());

      text_rule(
        &db,
        BudgetScope::Character(1),
        slug_to_id["fees"],
        true,
        0,
        "Brokers Fee",
      )
      .await;

      let after = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(after.get(&slug_to_id["fees"]), Some(&-120.0));
    }

    #[tokio::test]
    async fn it_never_moves_a_manually_assigned_entry_even_when_a_rule_matches() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      finance::append_wallet_journal(&db, &[journal(1, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z")])
        .await
        .unwrap();
      assign_entry(
        &db,
        BudgetScope::Character(1),
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        1,
        slug_to_id["income"],
      )
      .await
      .unwrap();
      text_rule(
        &db,
        BudgetScope::Character(1),
        slug_to_id["fees"],
        true,
        0,
        "Brokers Fee",
      )
      .await;

      let activity = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["income"]), Some(&-120.0));
      assert_eq!(activity.get(&slug_to_id["fees"]), None);
    }

    #[tokio::test]
    async fn it_ignores_disabled_rules_and_routes_ruled_income_to_ready_to_assign() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      finance::append_wallet_journal(
        &db,
        &[
          journal(1, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z"),
          journal(2, 1, "bounty_prizes", 1_000.0, "2026-06-16T00:00:00Z"),
        ],
      )
      .await
      .unwrap();
      // A disabled fee rule (ignored) plus an enabled inflow-only rule. The
      // disabled rule leaves the fee in Ready-to-Assign; the enabled rule
      // resolves the bounty to the income category, but the income→RTA
      // disposition reinterprets that non-manual inflow back to Ready-to-Assign.
      text_rule(
        &db,
        BudgetScope::Character(1),
        slug_to_id["fees"],
        false,
        0,
        "Brokers Fee",
      )
      .await;
      {
        use crate::store::{
          model::NewRule,
          repo::budget::{create_rule, replace_rule_conditions},
        };
        let created = create_rule(
          &db,
          &NewRule {
            category_id: slug_to_id["income"],
            enabled: true,
            match_mode: MatchMode::All,
            name: "inflows".to_owned(),
            position: 1,
            scope: BudgetScope::Character(1),
          },
        )
        .await
        .unwrap();
        replace_rule_conditions(
          &db,
          created.id(),
          &[RuleCondition {
            field: RuleField::Direction,
            op: RuleOp::Is,
            value: "in".to_owned(),
            value2: None,
          }],
        )
        .await
        .unwrap();
      }

      let activity = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;

      // Genuine income routes to Ready-to-Assign despite the matching rule, and
      // the fee stays unassigned (disabled rule): neither files into an envelope.
      assert!(!activity.contains_key(&slug_to_id["income"]));
      assert!(!activity.contains_key(&slug_to_id["fees"]));
    }

    #[tokio::test]
    async fn it_resolves_to_the_highest_priority_matching_rule() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      finance::append_wallet_journal(&db, &[journal(1, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z")])
        .await
        .unwrap();
      // Two rules match; the lower position (higher priority) wins.
      text_rule(
        &db,
        BudgetScope::Character(1),
        slug_to_id["fees"],
        true,
        0,
        "Brokers Fee",
      )
      .await;
      text_rule(
        &db,
        BudgetScope::Character(1),
        slug_to_id["trading"],
        true,
        1,
        "Brokers Fee",
      )
      .await;

      let activity = monthly_activity(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["fees"]), Some(&-120.0));
      assert_eq!(activity.get(&slug_to_id["trading"]), None);
    }
  }

  mod uncategorized_count_for_month {
    use pretty_assertions::assert_eq;

    use super::{
      monthly_activity::{journal, linked_journal, seed_character, text_rule, transaction},
      *,
    };
    use crate::store::{self, repo::finance};

    #[tokio::test]
    async fn it_counts_only_uncategorized_expenses_for_the_selected_month() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      finance::append_wallet_journal(
        &db,
        &[
          // Income posts to Ready-to-Assign and is never reviewable.
          journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z"),
          // An uncategorized outflow: the only reviewable row.
          journal(2, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z"),
          // A prior month's outflow never counts toward June.
          journal(3, 1, "brokers_fee", -500.0, "2026-05-30T00:00:00Z"),
        ],
      )
      .await
      .unwrap();

      let count = uncategorized_count_for_month(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn it_excludes_manually_assigned_rows() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      finance::append_wallet_journal(
        &db,
        &[
          journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z"),
          journal(2, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z"),
        ],
      )
      .await
      .unwrap();
      assign_entry(
        &db,
        BudgetScope::Character(1),
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        1,
        slug_to_id["income"],
      )
      .await
      .unwrap();

      let count = uncategorized_count_for_month(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn it_excludes_rows_resolved_by_a_rule() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      let slug_to_id = slug_to_category_id(&db, BudgetScope::Character(1)).await;
      finance::append_wallet_journal(&db, &[journal(1, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z")])
        .await
        .unwrap();

      let before = uncategorized_count_for_month(&db, BudgetScope::Character(1), "2026-06").await;
      assert_eq!(before, 1);

      text_rule(
        &db,
        BudgetScope::Character(1),
        slug_to_id["fees"],
        true,
        0,
        "Brokers Fee",
      )
      .await;

      let after = uncategorized_count_for_month(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(after, 0);
    }

    #[tokio::test]
    async fn it_excludes_an_ingested_market_transaction_journal_twin() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      // A buy surfaces as a transaction (an outflow/expense) plus its ingested
      // market_transaction journal twin. The trade is reviewed from the
      // Transactions side, so the twin is suppressed — only the transaction
      // counts, once.
      finance::append_wallet_transaction(&db, &[transaction(500, 1, true, 100.0, 10, "2026-06-05T00:00:00Z")])
        .await
        .unwrap();
      finance::append_wallet_journal(
        &db,
        &[linked_journal(
          10,
          1,
          "market_transaction",
          -1_000.0,
          500,
          "2026-06-05T00:00:00Z",
        )],
      )
      .await
      .unwrap();

      let count = uncategorized_count_for_month(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn it_counts_an_unresolved_mirrored_trade_once_under_all_scope() {
      use crate::store::{
        model::{Corporation, CorporationWalletTransaction, OwnerType},
        repo::infra,
      };

      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      infra::upsert(&db, 1, OwnerType::Character, "tok", "rt", 9_999, None, None)
        .await
        .unwrap();
      let corp_id = 98_000_012;
      let mut corp = Corporation::new(corp_id, "Owned Corp", "OWN");
      corp.set_ceo_id(1);
      corp.set_creator_id(1);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      store::repo::org::upsert_corporation(&db, &corp).await.unwrap();
      infra::upsert(&db, corp_id, OwnerType::Corporation, "tok", "rt", 9_999, Some(1), None)
        .await
        .unwrap();
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
      seed_scope(&db, BudgetScope::All).await.unwrap();
      // One unassigned buy (an expense) mirrored into both wallets under
      // transaction_id 700. It is a single event needing one review, not two.
      finance::append_corporation_wallet_transaction(
        &db,
        &[CorporationWalletTransaction {
          client_id: 1_000_035,
          corporation_id: corp_id,
          date: "2026-06-09T00:00:00Z".to_owned(),
          division: 1,
          is_buy: true,
          journal_ref_id: 0,
          location_id: 60_003_760,
          quantity: 10,
          transaction_id: 700,
          type_id: 34,
          unit_price: 100.0,
        }],
      )
      .await
      .unwrap();
      finance::append_wallet_transaction(&db, &[transaction(700, 1, true, 100.0, 10, "2026-06-09T00:00:00Z")])
        .await
        .unwrap();

      let count = uncategorized_count_for_month(&db, BudgetScope::All, "2026-06").await;

      assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn it_excludes_income() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      finance::append_wallet_journal(&db, &[journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z")])
        .await
        .unwrap();

      let count = uncategorized_count_for_month(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn it_counts_an_un_twinned_market_journal_row() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db, BudgetScope::Character(1)).await.unwrap();
      // A market_transaction journal row whose trade was never ingested as a
      // transaction is not a twin, so it stays reviewable as its own outflow.
      finance::append_wallet_journal(
        &db,
        &[linked_journal(
          10,
          1,
          "market_transaction",
          -1_000.0,
          999,
          "2026-06-05T00:00:00Z",
        )],
      )
      .await
      .unwrap();

      let count = uncategorized_count_for_month(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn it_excludes_an_internal_transfer() {
      use crate::store::{
        model::{Corporation, CorporationWalletJournal, OwnerType},
        repo::infra,
      };

      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      infra::upsert(&db, 1, OwnerType::Character, "tok", "rt", 9_999, None, None)
        .await
        .unwrap();
      let corp_id = 98_000_013;
      let mut corp = Corporation::new(corp_id, "Owned Corp", "OWN");
      corp.set_ceo_id(1);
      corp.set_creator_id(1);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      store::repo::org::upsert_corporation(&db, &corp).await.unwrap();
      infra::upsert(&db, corp_id, OwnerType::Corporation, "tok", "rt", 9_999, Some(1), None)
        .await
        .unwrap();
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
      seed_scope(&db, BudgetScope::All).await.unwrap();
      // One internal transfer: journal id 900 mirrored into the corp wallet (out)
      // and the character's wallet (in). Neither leg is reviewable.
      finance::append_corporation_wallet_journal(
        &db,
        &[CorporationWalletJournal {
          amount: Some(-2_000.0),
          balance: Some(0.0),
          context_id: None,
          context_id_type: None,
          corporation_id: corp_id,
          date: "2026-06-12T00:00:00Z".to_owned(),
          description: "Transfer out".to_owned(),
          division: 1,
          first_party_id: None,
          id: 900,
          reason: None,
          ref_type: "corporation_account_withdrawal".to_owned(),
          second_party_id: None,
          tax: None,
          tax_receiver_id: None,
        }],
      )
      .await
      .unwrap();
      finance::append_wallet_journal(
        &db,
        &[journal(
          900,
          1,
          "corporation_account_withdrawal",
          2_000.0,
          "2026-06-12T00:00:00Z",
        )],
      )
      .await
      .unwrap();

      let count = uncategorized_count_for_month(&db, BudgetScope::All, "2026-06").await;

      assert_eq!(count, 0);
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
