use std::collections::HashMap;

use crate::store::{
  Database, Error,
  model::{BudgetEntryAssignment, BudgetOwner, MatchMode, Rule, RuleCondition, RuleField, RuleOp},
  repo::{character, finance, org},
};

const DIRECTION_IN: &str = "in";
const DIRECTION_OUT: &str = "out";
const MARKET_BUY_TYPE: &str = "market_buy";
const MARKET_SALE_TYPE: &str = "market_sale";

const MARKET_TRANSACTION_CONTEXT_ID_TYPE: &str = "market_transaction_id";

struct SeedCategory {
  name: &'static str,
  slug: &'static str,
  tone: Option<&'static str>,
}

struct SeedGroup {
  cats: &'static [SeedCategory],
  name: &'static str,
}

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

const REFUND_REF_TYPES: &[&str] = &[
  "contract_collateral_refund",
  "contract_deposit_refund",
  "contract_reward_refund",
  "industry_job_refund",
  "market_escrow_refund",
  "reaction_refund",
];

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BudgetFlow {
  Expense,
  Income,
  Refund,
}

impl BudgetFlow {
  pub fn from_market(is_buy: bool) -> Self {
    if is_buy {
      BudgetFlow::Expense
    } else {
      BudgetFlow::Income
    }
  }

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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CategoryMonth {
  pub activity: f64,
  pub assigned: f64,
  pub carry: f64,
}

impl CategoryMonth {
  pub fn available(self) -> f64 {
    self.carry + self.assigned + self.activity
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MonthFlow {
  pub age: f64,
  pub assigned: f64,
  pub income: f64,
  pub month: String,
  pub spend: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoolSummary {
  pub overspent: f64,
  pub pool: f64,
  pub ready_to_assign: f64,
}

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

pub fn carry_from(prior_available: Option<f64>) -> f64 {
  prior_available.map_or(0.0, |available| available.max(0.0))
}

/// Rolls carry-over across an ordered month series for a single category.
///
/// `months` is `(assigned, activity)` for each month in chronological order. The
/// first element's carry is computed from `seed_carry` (0 for a brand-new
/// category). Each subsequent month carries the prior month's `max(0,
/// available)`.
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

pub async fn assign_entry(
  db: &Database,
  owner: BudgetOwner,
  entry_id: i64,
  category_id: i64,
) -> Result<Option<BudgetEntryAssignment>, Error> {
  if !crate::store::repo::budget::owner_holds_entry(db, owner, entry_id).await? {
    return Ok(None);
  }
  seed_scope(db).await?;
  crate::store::repo::budget::upsert_entry_assignment(db, owner, entry_id, category_id)
    .await
    .map(Some)
}

#[derive(Clone, Debug, Default)]
pub struct ResolutionContext {
  pub journal_overrides: HashMap<(BudgetOwner, i64), i64>,
  pub rules: Vec<Rule>,
}

impl ResolutionContext {
  pub async fn load(db: &Database) -> Self {
    let mut journal_overrides = HashMap::new();
    for assignment in crate::store::repo::budget::list_entry_assignments(db)
      .await
      .unwrap_or_default()
    {
      if let Some(owner) = BudgetOwner::from_key(assignment.owner_kind(), assignment.owner_id()) {
        journal_overrides.insert((owner, assignment.entry_id()), assignment.category_id());
      }
    }

    Self {
      journal_overrides,
      rules: crate::store::repo::budget::list_rules(db).await.unwrap_or_default(),
    }
  }

  pub fn resolve_target(&self, entry_id: i64, target: &MatchTarget) -> Option<i64> {
    let owner = target.owner?;
    if let Some(id) = self.override_for(owner, entry_id) {
      return Some(id);
    }
    rule_category_for(target, &self.rules)
  }

  pub fn override_for(&self, owner: BudgetOwner, entry_id: i64) -> Option<i64> {
    self.journal_overrides.get(&(owner, entry_id)).copied()
  }

  pub fn resolve_for_activity(&self, entry_id: i64, flow: BudgetFlow, target: &MatchTarget) -> Option<i64> {
    let owner = target.owner?;
    let manual = self.override_for(owner, entry_id);
    let resolved = self.resolve_target(entry_id, target);
    dispose_inflow_assignment(flow, manual, resolved)
  }
}

/// First-run disposition of an inflow's resolved budget category under the
/// money-conserving income→Ready-to-Assign model. A manual override is always
/// honored; a non-manual income defaults to Ready-to-Assign (`None`); every
/// other flow files wherever it resolved.
pub fn dispose_inflow_assignment(flow: BudgetFlow, manual: Option<i64>, resolved: Option<i64>) -> Option<i64> {
  if manual.is_some() {
    return resolved;
  }
  if flow == BudgetFlow::Income {
    return None;
  }
  resolved
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PreviewStatus {
  Already,
  Assign,
  Manual,
  Preempted,
}

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
      item: String::new(),
      location: String::new(),
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

pub fn match_count(rule: &Rule, outflows: &[MatchTarget]) -> usize {
  outflows.iter().filter(|target| target.matches_rule(rule)).count()
}

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
    RuleField::Amount => {
      t!("wallet.budget.suggest_amount", op => op_label(condition.op()), value => condition.value()).into_owned()
    }
    RuleField::Direction => if condition.value() == DIRECTION_IN {
      super::i18n::tr_static("wallet.budget.suggest_inflows")
    } else {
      super::i18n::tr_static("wallet.budget.suggest_outflows")
    }
    .to_owned(),
    _ => condition.value().clone(),
  }
}

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
    RuleOp::Between => super::i18n::tr_static("wallet.budget.op_between"),
    RuleOp::Contains => super::i18n::tr_static("wallet.budget.op_contains"),
    RuleOp::GreaterThan => super::i18n::tr_static("wallet.budget.op_over"),
    RuleOp::Is => super::i18n::tr_static("wallet.budget.op_is"),
    RuleOp::IsNot => super::i18n::tr_static("wallet.budget.op_is_not"),
    RuleOp::LessThan => super::i18n::tr_static("wallet.budget.op_under"),
    RuleOp::NotContains => super::i18n::tr_static("wallet.budget.op_not_contains"),
    RuleOp::StartsWith => super::i18n::tr_static("wallet.budget.op_starts_with"),
  }
}

pub fn field_label(field: RuleField) -> &'static str {
  match field {
    RuleField::Amount => super::i18n::tr_static("wallet.budget.field_amount"),
    RuleField::Character => super::i18n::tr_static("wallet.budget.field_character"),
    RuleField::Direction => super::i18n::tr_static("wallet.budget.field_direction"),
    RuleField::Item => super::i18n::tr_static("wallet.budget.field_item"),
    RuleField::Location => super::i18n::tr_static("wallet.budget.field_location"),
    RuleField::Party => super::i18n::tr_static("wallet.budget.field_party"),
    RuleField::Reference => super::i18n::tr_static("wallet.budget.field_reference"),
    RuleField::Text => super::i18n::tr_static("wallet.budget.field_any_text"),
    RuleField::Type => super::i18n::tr_static("wallet.budget.field_type"),
  }
}

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

pub fn direction_options() -> [(&'static str, &'static str); 2] {
  [
    (DIRECTION_OUT, super::i18n::tr_static("wallet.budget.direction_outflow")),
    (DIRECTION_IN, super::i18n::tr_static("wallet.budget.direction_inflow")),
  ]
}

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
    return t!("wallet.budget.summary_no_conditions").into_owned();
  }
  let joiner = match rule.match_mode() {
    MatchMode::Any => super::i18n::tr_static("wallet.budget.summary_join_or"),
    MatchMode::All => super::i18n::tr_static("wallet.budget.summary_join_and"),
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
      RuleOp::Between => t!(
        "wallet.budget.condition_amount_between",
        low => condition.value(),
        high => condition.value2().as_deref().unwrap_or("")
      )
      .into_owned(),
      _ => t!("wallet.budget.condition_amount", op => op, value => condition.value()).into_owned(),
    },
    RuleField::Direction => {
      let value = if condition.value() == DIRECTION_IN {
        super::i18n::tr_static("wallet.budget.condition_direction_inflow")
      } else {
        super::i18n::tr_static("wallet.budget.condition_direction_outflow")
      };
      t!("wallet.budget.condition_direction", value => value).into_owned()
    }
    RuleField::Type => {
      let value = type_label(condition.value()).unwrap_or_else(|| condition.value().clone());
      t!("wallet.budget.condition_type", op => op, value => value).into_owned()
    }
    RuleField::Character => {
      let value = character_name(condition.value()).unwrap_or_else(|| condition.value().clone());
      t!("wallet.budget.condition_character", op => op, value => value).into_owned()
    }
    field => t!(
      "wallet.budget.condition_field",
      field => field_label(field),
      op => op,
      value => condition.value()
    )
    .into_owned(),
  }
}

fn rule_category_for(target: &MatchTarget, rules: &[Rule]) -> Option<i64> {
  rules
    .iter()
    .find(|rule| rule.enabled() && target.matches_rule(rule))
    .map(Rule::category_id)
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn resolve_entry_category(
  db: &Database,
  owner: BudgetOwner,
  entry_id: i64,
  ref_type: &str,
  amount: Option<f64>,
  text: &str,
) -> Option<i64> {
  let context = ResolutionContext::load(db).await;
  let target = MatchTarget::journal(owner, ref_type, amount, text);
  context.resolve_target(entry_id, &target)
}

#[cfg_attr(not(test), expect(dead_code))]
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

/// Ready-to-Assign and overspending for the budget, money-conserving by
/// construction. Ready-to-Assign is `pool − Σ max(0, available)` over the passed
/// per-category availables, so the liquid pool always splits exactly as `pool =
/// ready_to_assign + Σ max(0, available)`.
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

async fn owned_character_ids(db: &Database) -> Vec<i64> {
  character::all_owned(db)
    .await
    .unwrap_or_default()
    .iter()
    .map(crate::store::model::Character::id)
    .collect()
}

async fn owned_corporation_ids(db: &Database) -> Vec<i64> {
  org::all_owned_corporations(db)
    .await
    .unwrap_or_default()
    .iter()
    .map(crate::store::model::OwnedCorporation::id)
    .collect()
}

/// The budgetable pool: Σ live liquid balances across every owned character
/// wallet plus the owned corp division wallets. Missing balances are 0.
pub async fn budgetable_pool(db: &Database) -> f64 {
  let mut pool = 0.0;
  for id in owned_character_ids(db).await {
    if let Ok(Some(row)) = finance::financials_get(db, id).await {
      pool += row.liquid.unwrap_or(0.0);
    }
  }
  for corp in owned_corporation_ids(db).await {
    for division in finance::divisions(db, corp).await.unwrap_or_default() {
      pool += division.balance().unwrap_or(0.0);
    }
  }
  pool
}

pub async fn slug_to_category_id(db: &Database) -> HashMap<&'static str, i64> {
  let name_to_slug: HashMap<&str, &str> = SEED_GROUPS
    .iter()
    .flat_map(|group| group.cats.iter())
    .map(|cat| (cat.name, cat.slug))
    .collect();

  let mut out = HashMap::new();
  let groups = crate::store::repo::budget::list_groups(db).await.unwrap_or_default();
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

/// Seeds the budget with the starter groups and categories. Idempotent: seeds
/// only when no group exists yet, so a user who deletes a starter envelope never
/// has it resurrected and an already-populated budget is left untouched.
pub async fn seed_scope(db: &Database) -> Result<(), Error> {
  use crate::store::{
    model::{NewCategory, NewGroup},
    repo::budget::{create_category, create_group, list_groups},
  };

  if !list_groups(db).await?.is_empty() {
    return Ok(());
  }

  for (group_position, group) in SEED_GROUPS.iter().enumerate() {
    let created_group = create_group(
      db,
      &NewGroup {
        name: group.name.to_owned(),
        position: group_position as i64,
      },
    )
    .await?;
    for (cat_position, cat) in group.cats.iter().enumerate() {
      create_category(
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
    }
  }

  Ok(())
}

struct JournalActivity {
  amount: Option<f64>,
  date: String,
  id: i64,
  item: String,
  location: String,
  owner: BudgetOwner,
  ref_type: String,
  text: String,
}

struct ScopeLedger {
  context: ResolutionContext,
  journal_rows: Vec<JournalActivity>,
}

fn journal_target(row: &JournalActivity) -> MatchTarget {
  let mut target = MatchTarget::journal(row.owner, &row.ref_type, row.amount, &row.text);
  target.item = row.item.clone();
  target.location = row.location.clone();
  target
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn monthly_activity(db: &Database, month: &str) -> HashMap<i64, f64> {
  activity_by_month(db).await.remove(month).unwrap_or_default()
}

/// Category activity per month: the signed sum of the journal entries assigned
/// to each category. The transaction ledger is never read here — a trade counts
/// once, through its journal entry, and both legs of a transfer net to zero.
pub async fn activity_by_month(db: &Database) -> HashMap<String, HashMap<i64, f64>> {
  let ledger = load_scope_ledger(db).await;

  let mut by_month: HashMap<String, HashMap<i64, f64>> = HashMap::new();
  for row in &ledger.journal_rows {
    let Some((month, category_id, amount)) = resolve_journal_activity(row, &ledger.context) else {
      continue;
    };
    *by_month.entry(month).or_default().entry(category_id).or_insert(0.0) += amount;
  }
  by_month
}

fn resolve_journal_activity(row: &JournalActivity, context: &ResolutionContext) -> Option<(String, i64, f64)> {
  let month = month_key(&row.date)?;
  let amount = row.amount?;
  let flow = BudgetFlow::from_ref_type(&row.ref_type, amount);
  let target = journal_target(row);
  let category_id = context.resolve_for_activity(row.id, flow, &target)?;
  Some((month, category_id, amount))
}

pub async fn uncategorized_count_for_month(db: &Database, month: &str) -> usize {
  let ledger = load_scope_ledger(db).await;
  ledger
    .journal_rows
    .iter()
    .filter(|row| is_uncategorized_journal(row, &ledger.context, month))
    .count()
}

fn is_uncategorized_journal(row: &JournalActivity, context: &ResolutionContext, month: &str) -> bool {
  if month_key(&row.date).as_deref() != Some(month) {
    return false;
  }
  let Some(amount) = row.amount else {
    return false;
  };
  if BudgetFlow::from_ref_type(&row.ref_type, amount) != BudgetFlow::Expense {
    return false;
  }
  let target = journal_target(row);
  context.resolve_target(row.id, &target).is_none()
}

struct ActivityNames {
  type_names: HashMap<i64, String>,
  location_names: HashMap<i64, String>,
}

impl ActivityNames {
  fn item(&self, type_id: i64) -> String {
    self.type_names.get(&type_id).cloned().unwrap_or_default()
  }

  fn location(&self, location_id: i64) -> String {
    self.location_names.get(&location_id).cloned().unwrap_or_default()
  }
}

/// The `(item, location)` names a journal entry inherits from its linked
/// transaction, so an item/location rule can match a trade through its journal
/// row. A journal entry with no linked transaction inherits empty names and so
/// is never matched by an item rule.
fn linked_item(
  context_id_type: Option<&str>,
  context_id: Option<i64>,
  tx_items: &HashMap<i64, (String, String)>,
) -> (String, String) {
  if context_id_type == Some(MARKET_TRANSACTION_CONTEXT_ID_TYPE)
    && let Some(id) = context_id
    && let Some(pair) = tx_items.get(&id)
  {
    return pair.clone();
  }
  (String::new(), String::new())
}

async fn load_scope_ledger(db: &Database) -> ScopeLedger {
  let context = ResolutionContext::load(db).await;

  let names = if context.rules.is_empty() {
    ActivityNames {
      type_names: HashMap::new(),
      location_names: HashMap::new(),
    }
  } else {
    ActivityNames {
      type_names: type_names(db).await,
      location_names: location_names(db).await,
    }
  };

  let mut journal_rows: Vec<JournalActivity> = Vec::new();
  for character_id in owned_character_ids(db).await {
    collect_character_ledger(db, character_id, &names, &mut journal_rows).await;
  }
  for corp in owned_corporation_ids(db).await {
    collect_corporation_ledger(db, corp, &names, &mut journal_rows).await;
  }

  ScopeLedger {
    context,
    journal_rows,
  }
}

async fn collect_character_ledger(
  db: &Database,
  character_id: i64,
  names: &ActivityNames,
  journal_rows: &mut Vec<JournalActivity>,
) {
  let owner = BudgetOwner::Character(character_id);
  let mut tx_items: HashMap<i64, (String, String)> = HashMap::new();
  for tx in finance::wallet_transactions(db, character_id).await.unwrap_or_default() {
    tx_items.insert(
      tx.transaction_id(),
      (names.item(tx.type_id()), names.location(tx.location_id())),
    );
  }
  for row in finance::wallet_journal(db, character_id).await.unwrap_or_default() {
    let (item, location) = linked_item(row.context_id_type().as_deref(), row.context_id(), &tx_items);
    journal_rows.push(JournalActivity {
      amount: row.amount(),
      date: row.date().clone(),
      id: row.id(),
      item,
      location,
      owner,
      ref_type: row.ref_type().clone(),
      text: journal_match_text(row.ref_type(), row.reason().as_deref(), row.description()),
    });
  }
}

async fn collect_corporation_ledger(
  db: &Database,
  corp: i64,
  names: &ActivityNames,
  journal_rows: &mut Vec<JournalActivity>,
) {
  let owner = BudgetOwner::Corporation(corp);
  let divisions = finance::divisions(db, corp).await.unwrap_or_default();
  let mut tx_items: HashMap<i64, (String, String)> = HashMap::new();
  for division in &divisions {
    for tx in finance::corporation_wallet_transactions(db, corp, division.division())
      .await
      .unwrap_or_default()
    {
      tx_items.insert(
        tx.transaction_id(),
        (names.item(tx.type_id()), names.location(tx.location_id())),
      );
    }
  }
  for division in &divisions {
    for row in finance::corporation_wallet_journal(db, corp, division.division())
      .await
      .unwrap_or_default()
    {
      let (item, location) = linked_item(row.context_id_type().as_deref(), row.context_id(), &tx_items);
      journal_rows.push(JournalActivity {
        amount: row.amount(),
        date: row.date().clone(),
        id: row.id(),
        item,
        location,
        owner,
        ref_type: row.ref_type().clone(),
        text: journal_match_text(row.ref_type(), row.reason().as_deref(), row.description()),
      });
    }
  }
}

fn epoch_day(date: &str) -> Option<i64> {
  use chrono::Datelike;
  let head = date.get(..10)?;
  let parsed = chrono::NaiveDate::parse_from_str(head, "%Y-%m-%d").ok()?;
  Some(i64::from(parsed.num_days_from_ce()))
}

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

async fn journal_flows(db: &Database) -> Vec<(String, f64)> {
  let mut flows: Vec<(String, f64)> = Vec::new();
  for character_id in owned_character_ids(db).await {
    push_character_flows(db, character_id, &mut flows).await;
  }
  for corp in owned_corporation_ids(db).await {
    push_corporation_flows(db, corp, &mut flows).await;
  }
  flows
}

async fn push_character_flows(db: &Database, character_id: i64, flows: &mut Vec<(String, f64)>) {
  for row in finance::wallet_journal(db, character_id).await.unwrap_or_default() {
    if let Some(amount) = row.amount() {
      flows.push((row.date().clone(), amount));
    }
  }
}

async fn push_corporation_flows(db: &Database, corp: i64, flows: &mut Vec<(String, f64)>) {
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

pub async fn monthly_history(db: &Database, month: &str, months: usize) -> Vec<MonthFlow> {
  let flows = journal_flows(db).await;
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
    let assigned = month_assigned(db, &key).await;
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

pub(crate) async fn type_names(db: &Database) -> HashMap<i64, String> {
  crate::store::repo::sde::all_item_types(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|item| (item.id(), item.name().clone()))
    .collect()
}

async fn month_assigned(db: &Database, month: &str) -> f64 {
  use crate::store::repo::budget;
  let mut total = 0.0;
  for group in budget::list_groups(db).await.unwrap_or_default() {
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

      #[test]
      fn it_matches_an_item_rule_only_on_a_journal_row_with_a_linked_item() {
        let bare = journal_outflow(
          BudgetOwner::Character(1),
          "market_transaction",
          -100.0,
          "Market Transaction",
        );
        let mut linked = bare.clone();
        linked.item = "Tritanium".to_owned();
        linked.location = "Jita IV - Moon 4".to_owned();

        assert!(!bare.matches_condition(&condition(RuleField::Item, RuleOp::Contains, "tritanium")));
        assert!(linked.matches_condition(&condition(RuleField::Item, RuleOp::Contains, "tritanium")));
        assert!(linked.matches_condition(&condition(RuleField::Location, RuleOp::Contains, "jita")));
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
      assert_eq!(dispose_inflow_assignment(BudgetFlow::Income, None, Some(10)), None);
    }

    #[test]
    fn it_retains_a_manual_inflow_assignment() {
      assert_eq!(dispose_inflow_assignment(BudgetFlow::Income, Some(7), Some(7)), Some(7));
    }

    #[test]
    fn it_leaves_non_income_flows_filing_where_they_resolve() {
      assert_eq!(dispose_inflow_assignment(BudgetFlow::Expense, None, Some(5)), Some(5));
      assert_eq!(dispose_inflow_assignment(BudgetFlow::Refund, None, Some(5)), Some(5));
    }

    #[test]
    fn it_disposes_through_the_resolution_context() {
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
      };

      let target = MatchTarget::journal(owner, "bounty", Some(10.0), "Bounty");
      assert_eq!(context.resolve_for_activity(100, BudgetFlow::Income, &target), Some(10));
      assert_eq!(context.resolve_for_activity(200, BudgetFlow::Income, &target), None);
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
      let months = roll_carry(0.0, &[(80.0, 0.0), (0.0, -30.0)]);

      assert_eq!(months[0].available(), 80.0);
      assert_eq!(months[1].carry, 80.0);
      assert_eq!(months[1].available(), 50.0);
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
      let summary = pool_summary(1_000.0, [300.0, 200.0, 100.0]);

      assert_eq!(summary.pool, 1_000.0);
      assert_eq!(summary.ready_to_assign, 400.0);
      assert_eq!(summary.overspent, 0.0);
    }

    #[test]
    fn it_conserves_liquid_across_ready_to_assign_and_held() {
      let availables = [300.0, 250.0, 50.0];
      let summary = pool_summary(1_000.0, availables);

      let held: f64 = availables.iter().filter(|a| **a > 0.0).sum();
      assert_eq!(summary.ready_to_assign + held, summary.pool);
    }

    #[test]
    fn it_excludes_overspent_envelopes_from_held_and_reports_them() {
      let summary = pool_summary(500.0, [300.0, -150.0, -50.0]);

      assert_eq!(summary.overspent, -200.0);
      assert_eq!(summary.ready_to_assign, 200.0);
    }

    #[test]
    fn it_can_report_a_negative_ready_to_assign_when_over_held() {
      let summary = pool_summary(100.0, [80.0, 80.0]);

      assert_eq!(summary.ready_to_assign, -60.0);
    }

    #[test]
    fn it_conserves_money_across_assign_spend_and_overspend() {
      let availables = [200.0, 50.0, -80.0];
      let summary = pool_summary(1_000.0, availables);

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
      fn it_classifies_a_transfer_ref_type_by_sign() {
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

  mod support {
    use super::*;
    use crate::store::{
      self,
      model::{
        Alliance, Bloodline, Character, CharacterWalletJournal, Corporation, CorporationWalletDivision,
        CorporationWalletJournal, Gender, OwnerType, Race,
      },
      repo::{character::insert_with_org, finance, infra, org},
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
      infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9_999, None, None)
        .await
        .unwrap();
    }

    pub(super) async fn seed_character_with_liquid(db: &Database, id: i64, liquid: f64) {
      seed_character(db, id).await;
      finance::append_wallet_journal(
        db,
        &[journal(id, id, "player_donation", liquid, "2026-06-18T00:00:00Z")],
      )
      .await
      .unwrap();
    }

    pub(super) async fn seed_owned_corp(db: &Database, corp_id: i64, creator_id: i64, divisions: &[(i64, f64)]) {
      let mut corp = Corporation::new(corp_id, "Owned Corp", "OWN");
      corp.set_ceo_id(creator_id);
      corp.set_creator_id(creator_id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      org::upsert_corporation(db, &corp).await.unwrap();
      infra::upsert(
        db,
        corp_id,
        OwnerType::Corporation,
        "tok",
        "rt",
        9_999,
        Some(creator_id),
        None,
      )
      .await
      .unwrap();
      let rows: Vec<CorporationWalletDivision> = divisions
        .iter()
        .map(|&(division, balance)| CorporationWalletDivision {
          balance: Some(balance),
          corporation_id: corp_id,
          division,
          name: Some("Division".to_owned()),
        })
        .collect();
      finance::upsert_divisions(db, &rows).await.unwrap();
    }

    pub(super) fn journal(
      id: i64,
      character_id: i64,
      ref_type: &str,
      amount: f64,
      date: &str,
    ) -> CharacterWalletJournal {
      CharacterWalletJournal {
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

    pub(super) fn corp_journal(
      id: i64,
      corporation_id: i64,
      division: i64,
      ref_type: &str,
      amount: f64,
      date: &str,
    ) -> CorporationWalletJournal {
      CorporationWalletJournal {
        amount: Some(amount),
        balance: Some(0.0),
        context_id: None,
        context_id_type: None,
        corporation_id,
        date: date.to_owned(),
        description: "Entry".to_owned(),
        division,
        first_party_id: None,
        id,
        reason: None,
        ref_type: ref_type.to_owned(),
        second_party_id: None,
        tax: None,
        tax_receiver_id: None,
      }
    }

    pub(super) async fn text_rule(db: &Database, category_id: i64, enabled: bool, position: i64, needle: &str) {
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
  }

  mod seed_scope {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, model::NewGroup, repo::budget};

    #[tokio::test]
    async fn it_seeds_starter_groups_and_categories() {
      let db = store::open_test().await.unwrap();

      seed_scope(&db).await.unwrap();

      let groups = budget::list_groups(&db).await.unwrap();
      assert_eq!(groups.len(), SEED_GROUPS.len());

      let slug_to_id = slug_to_category_id(&db).await;
      for group in SEED_GROUPS {
        for cat in group.cats {
          assert!(slug_to_id.contains_key(cat.slug), "{} slug was not seeded", cat.slug);
        }
      }
    }

    #[tokio::test]
    async fn it_is_idempotent() {
      let db = store::open_test().await.unwrap();

      seed_scope(&db).await.unwrap();
      seed_scope(&db).await.unwrap();

      assert_eq!(budget::list_groups(&db).await.unwrap().len(), SEED_GROUPS.len());
    }

    #[tokio::test]
    async fn it_does_not_seed_when_a_group_already_exists() {
      let db = store::open_test().await.unwrap();
      budget::create_group(
        &db,
        &NewGroup {
          name: "Pre-existing".to_owned(),
          position: 0,
        },
      )
      .await
      .unwrap();

      seed_scope(&db).await.unwrap();

      assert_eq!(budget::list_groups(&db).await.unwrap().len(), 1);
    }
  }

  mod assign_entry {
    use pretty_assertions::assert_eq;

    use super::{
      support::{corp_journal, journal, seed_character, seed_owned_corp},
      *,
    };
    use crate::store::{
      self,
      repo::{budget, finance},
    };

    #[tokio::test]
    async fn it_seeds_and_persists_a_journal_assignment() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      finance::append_wallet_journal(&db, &[journal(5, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z")])
        .await
        .unwrap();
      seed_scope(&db).await.unwrap();
      let income = slug_to_category_id(&db).await["income"];

      let saved = assign_entry(&db, BudgetOwner::Character(1), 5, income).await.unwrap();

      assert_eq!(saved.expect("entry held by owner").category_id(), income);
      assert_eq!(budget::list_entry_assignments(&db).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_skips_a_copy_for_an_owner_that_does_not_hold_the_entry() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      let corp_id = 98_000_020;
      seed_owned_corp(&db, corp_id, 1, &[(1, 0.0)]).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;
      finance::append_corporation_wallet_journal(
        &db,
        &[corp_journal(
          9,
          corp_id,
          1,
          "industry_job_tax",
          -2_000.0,
          "2026-06-10T00:00:00Z",
        )],
      )
      .await
      .unwrap();

      let mis_owned = assign_entry(&db, BudgetOwner::Character(1), 9, slug_to_id["income"])
        .await
        .unwrap();
      let genuine = assign_entry(&db, BudgetOwner::Corporation(corp_id), 9, slug_to_id["tithe"])
        .await
        .unwrap();

      assert!(mis_owned.is_none());
      assert!(genuine.is_some());
      let assignments = budget::list_entry_assignments(&db).await.unwrap();
      assert_eq!(assignments.len(), 1);
      assert_eq!(assignments[0].owner_kind(), "corporation");
    }
  }

  mod monthly_activity {
    use pretty_assertions::assert_eq;

    use super::{
      support::{corp_journal, journal, seed_character, seed_owned_corp, text_rule},
      *,
    };
    use crate::store::{self, repo::finance};

    #[tokio::test]
    async fn it_counts_only_assigned_journal_entries() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;
      finance::append_wallet_journal(
        &db,
        &[
          journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z"),
          journal(2, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z"),
          journal(3, 1, "bounty_prizes", 500.0, "2026-06-20T00:00:00Z"),
        ],
      )
      .await
      .unwrap();
      assign_entry(&db, BudgetOwner::Character(1), 1, slug_to_id["income"])
        .await
        .unwrap();
      assign_entry(&db, BudgetOwner::Character(1), 2, slug_to_id["fees"])
        .await
        .unwrap();

      let activity = monthly_activity(&db, "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["income"]), Some(&1_000.0));
      assert_eq!(activity.get(&slug_to_id["fees"]), Some(&-120.0));
      assert_eq!(activity.len(), 2);
    }

    #[tokio::test]
    async fn it_does_not_count_unassigned_entries() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();
      finance::append_wallet_journal(&db, &[journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z")])
        .await
        .unwrap();

      let activity = monthly_activity(&db, "2026-06").await;

      assert!(activity.is_empty());
    }

    #[tokio::test]
    async fn it_excludes_an_assigned_entry_outside_the_month() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;
      finance::append_wallet_journal(&db, &[journal(1, 1, "bounty_prizes", 9_999.0, "2026-05-31T23:59:59Z")])
        .await
        .unwrap();
      assign_entry(&db, BudgetOwner::Character(1), 1, slug_to_id["income"])
        .await
        .unwrap();

      let activity = monthly_activity(&db, "2026-06").await;

      assert!(activity.is_empty());
    }

    #[tokio::test]
    async fn it_counts_assigned_corp_division_journals() {
      let db = store::open_test().await.unwrap();
      let corp_id = 98_000_001;
      seed_owned_corp(&db, corp_id, 100, &[(1, 0.0)]).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;
      finance::append_corporation_wallet_journal(
        &db,
        &[corp_journal(
          1,
          corp_id,
          1,
          "industry_job_tax",
          -2_000.0,
          "2026-06-10T00:00:00Z",
        )],
      )
      .await
      .unwrap();
      assign_entry(&db, BudgetOwner::Corporation(corp_id), 1, slug_to_id["tithe"])
        .await
        .unwrap();

      let activity = monthly_activity(&db, "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["tithe"]), Some(&-2_000.0));
    }

    #[tokio::test]
    async fn it_routes_two_owners_sharing_an_eve_id_to_their_own_categories() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      let corp_id = 98_000_010;
      seed_owned_corp(&db, corp_id, 1, &[(1, 0.0)]).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;

      finance::append_wallet_journal(&db, &[journal(5, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z")])
        .await
        .unwrap();
      finance::append_corporation_wallet_journal(
        &db,
        &[corp_journal(
          5,
          corp_id,
          1,
          "industry_job_tax",
          -2_000.0,
          "2026-06-10T00:00:00Z",
        )],
      )
      .await
      .unwrap();
      assign_entry(&db, BudgetOwner::Character(1), 5, slug_to_id["income"])
        .await
        .unwrap();
      assign_entry(&db, BudgetOwner::Corporation(corp_id), 5, slug_to_id["tithe"])
        .await
        .unwrap();

      let activity = monthly_activity(&db, "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["income"]), Some(&1_000.0));
      assert_eq!(activity.get(&slug_to_id["tithe"]), Some(&-2_000.0));
      assert_eq!(activity.len(), 2);
    }

    #[tokio::test]
    async fn it_nets_a_corp_to_corp_transfer_to_zero() {
      let db = store::open_test().await.unwrap();
      let corp_a = 98_000_030;
      let corp_b = 98_000_031;
      seed_owned_corp(&db, corp_a, 1, &[(1, 0.0)]).await;
      seed_owned_corp(&db, corp_b, 1, &[(1, 0.0)]).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;

      finance::append_corporation_wallet_journal(
        &db,
        &[corp_journal(
          900,
          corp_a,
          1,
          "corporation_account_withdrawal",
          -10_000_000_000.0,
          "2026-06-12T00:00:00Z",
        )],
      )
      .await
      .unwrap();
      finance::append_corporation_wallet_journal(
        &db,
        &[corp_journal(
          900,
          corp_b,
          1,
          "corporation_account_withdrawal",
          10_000_000_000.0,
          "2026-06-12T00:00:00Z",
        )],
      )
      .await
      .unwrap();
      assign_entry(&db, BudgetOwner::Corporation(corp_a), 900, slug_to_id["transfers"])
        .await
        .unwrap();
      assign_entry(&db, BudgetOwner::Corporation(corp_b), 900, slug_to_id["transfers"])
        .await
        .unwrap();

      let activity = monthly_activity(&db, "2026-06").await;

      // Both legs are ordinary journal entries filed to the same envelope, so the
      // transfer nets to exactly zero across the budget with no +2N phantom.
      assert_eq!(activity.get(&slug_to_id["transfers"]).copied().unwrap_or(0.0), 0.0);
    }

    #[tokio::test]
    async fn it_auto_assigns_a_matching_outflow_via_a_rule() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;
      finance::append_wallet_journal(&db, &[journal(1, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z")])
        .await
        .unwrap();

      let before = monthly_activity(&db, "2026-06").await;
      assert!(before.is_empty());

      text_rule(&db, slug_to_id["fees"], true, 0, "Brokers Fee").await;

      let after = monthly_activity(&db, "2026-06").await;

      assert_eq!(after.get(&slug_to_id["fees"]), Some(&-120.0));
    }

    #[tokio::test]
    async fn it_never_moves_a_manually_assigned_entry_even_when_a_rule_matches() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;
      finance::append_wallet_journal(&db, &[journal(1, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z")])
        .await
        .unwrap();
      assign_entry(&db, BudgetOwner::Character(1), 1, slug_to_id["income"])
        .await
        .unwrap();
      text_rule(&db, slug_to_id["fees"], true, 0, "Brokers Fee").await;

      let activity = monthly_activity(&db, "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["income"]), Some(&-120.0));
      assert_eq!(activity.get(&slug_to_id["fees"]), None);
    }

    #[tokio::test]
    async fn it_ignores_disabled_rules_and_routes_ruled_income_to_ready_to_assign() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;
      finance::append_wallet_journal(
        &db,
        &[
          journal(1, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z"),
          journal(2, 1, "bounty_prizes", 1_000.0, "2026-06-16T00:00:00Z"),
        ],
      )
      .await
      .unwrap();
      text_rule(&db, slug_to_id["fees"], false, 0, "Brokers Fee").await;
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

      let activity = monthly_activity(&db, "2026-06").await;

      assert!(!activity.contains_key(&slug_to_id["income"]));
      assert!(!activity.contains_key(&slug_to_id["fees"]));
    }

    #[tokio::test]
    async fn it_resolves_to_the_highest_priority_matching_rule() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;
      finance::append_wallet_journal(&db, &[journal(1, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z")])
        .await
        .unwrap();
      text_rule(&db, slug_to_id["fees"], true, 0, "Brokers Fee").await;
      text_rule(&db, slug_to_id["trading"], true, 1, "Brokers Fee").await;

      let activity = monthly_activity(&db, "2026-06").await;

      assert_eq!(activity.get(&slug_to_id["fees"]), Some(&-120.0));
      assert_eq!(activity.get(&slug_to_id["trading"]), None);
    }

    #[tokio::test]
    async fn it_groups_assigned_activity_by_month() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;
      finance::append_wallet_journal(
        &db,
        &[
          journal(1, 1, "bounty_prizes", 1_000.0, "2026-04-02T00:00:00Z"),
          journal(2, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z"),
        ],
      )
      .await
      .unwrap();
      assign_entry(&db, BudgetOwner::Character(1), 1, slug_to_id["income"])
        .await
        .unwrap();
      assign_entry(&db, BudgetOwner::Character(1), 2, slug_to_id["fees"])
        .await
        .unwrap();

      let by_month = activity_by_month(&db).await;

      assert_eq!(by_month["2026-04"].get(&slug_to_id["income"]), Some(&1_000.0));
      assert_eq!(by_month["2026-06"].get(&slug_to_id["fees"]), Some(&-120.0));
      assert!(!by_month.contains_key("2026-05"));
      assert_eq!(by_month["2026-04"], monthly_activity(&db, "2026-04").await);
    }
  }

  mod ready_to_assign {
    use pretty_assertions::assert_eq;

    use super::{
      support::{corp_journal, seed_character_with_liquid, seed_owned_corp},
      *,
    };
    use crate::store::{
      self,
      repo::{budget, finance},
    };

    #[tokio::test]
    async fn it_matches_the_wallet_across_characters_and_corp_divisions() {
      let db = store::open_test().await.unwrap();
      seed_character_with_liquid(&db, 1, 8_000.0).await;
      let corp_id = 98_000_040;
      seed_owned_corp(&db, corp_id, 1, &[(1, 3_000.0), (2, 2_000.0)]).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;

      // A corp spend filed to an envelope, funded above the spend.
      finance::append_corporation_wallet_journal(
        &db,
        &[corp_journal(
          50,
          corp_id,
          1,
          "industry_job_tax",
          -2_000.0,
          "2026-06-10T00:00:00Z",
        )],
      )
      .await
      .unwrap();
      assign_entry(&db, BudgetOwner::Corporation(corp_id), 50, slug_to_id["tithe"])
        .await
        .unwrap();
      budget::upsert_assignment(&db, slug_to_id["tithe"], "2026-06", 5_000.0)
        .await
        .unwrap();

      let pool = budgetable_pool(&db).await;
      assert_eq!(pool, 13_000.0);

      let activity = monthly_activity(&db, "2026-06").await;
      assert_eq!(activity.get(&slug_to_id["tithe"]), Some(&-2_000.0));

      let available = 5_000.0 + activity.get(&slug_to_id["tithe"]).copied().unwrap_or(0.0);
      let summary = pool_summary(pool, [available]);

      assert_eq!(available, 3_000.0);
      assert_eq!(summary.ready_to_assign, 10_000.0);
      // The envelope invariant: wallet balance = Ready-to-Assign + Σ held.
      assert_eq!(summary.ready_to_assign + available, pool);
    }
  }

  mod uncategorized_count_for_month {
    use pretty_assertions::assert_eq;

    use super::{
      support::{journal, seed_character, text_rule},
      *,
    };
    use crate::store::{self, repo::finance};

    #[tokio::test]
    async fn it_counts_only_uncategorized_expenses_for_the_selected_month() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();
      finance::append_wallet_journal(
        &db,
        &[
          journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z"),
          journal(2, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z"),
          journal(3, 1, "brokers_fee", -500.0, "2026-05-30T00:00:00Z"),
        ],
      )
      .await
      .unwrap();

      let count = uncategorized_count_for_month(&db, "2026-06").await;

      assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn it_excludes_manually_assigned_rows() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;
      finance::append_wallet_journal(&db, &[journal(2, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z")])
        .await
        .unwrap();
      assign_entry(&db, BudgetOwner::Character(1), 2, slug_to_id["fees"])
        .await
        .unwrap();

      let count = uncategorized_count_for_month(&db, "2026-06").await;

      assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn it_excludes_rows_resolved_by_a_rule() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();
      let slug_to_id = slug_to_category_id(&db).await;
      finance::append_wallet_journal(&db, &[journal(1, 1, "brokers_fee", -120.0, "2026-06-15T00:00:00Z")])
        .await
        .unwrap();

      let before = uncategorized_count_for_month(&db, "2026-06").await;
      assert_eq!(before, 1);

      text_rule(&db, slug_to_id["fees"], true, 0, "Brokers Fee").await;

      let after = uncategorized_count_for_month(&db, "2026-06").await;

      assert_eq!(after, 0);
    }

    #[tokio::test]
    async fn it_excludes_income() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();
      finance::append_wallet_journal(&db, &[journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-02T00:00:00Z")])
        .await
        .unwrap();

      let count = uncategorized_count_for_month(&db, "2026-06").await;

      assert_eq!(count, 0);
    }
  }

  mod budgetable_pool {
    use pretty_assertions::assert_eq;

    use super::{
      support::{seed_character_with_liquid, seed_owned_corp},
      *,
    };
    use crate::store::{self};

    #[tokio::test]
    async fn it_sums_owned_characters_and_corps() {
      let db = store::open_test().await.unwrap();
      seed_character_with_liquid(&db, 1, 5_000.0).await;
      let corp_id = 98_000_050;
      seed_owned_corp(&db, corp_id, 1, &[(1, 1_000.0)]).await;

      let pool = budgetable_pool(&db).await;

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
      let ages = fifo_ages_by_month([("2026-01-01T00:00:00Z", 100.0), ("2026-01-11T00:00:00Z", -100.0)]);

      assert_eq!(ages.get("2026-01"), Some(&10.0));
    }

    #[test]
    fn it_weights_age_by_isk_drawn_from_each_lot() {
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
      let ages = fifo_ages_by_month([("2026-05-05T00:00:00Z", -100.0)]);

      assert_eq!(ages.get("2026-05"), None);
    }

    #[test]
    fn it_sorts_flows_chronologically_before_aging() {
      let ages = fifo_ages_by_month([("2026-06-11T00:00:00Z", -100.0), ("2026-06-01T00:00:00Z", 100.0)]);

      assert_eq!(ages.get("2026-06"), Some(&10.0));
    }
  }

  mod monthly_history {
    use pretty_assertions::assert_eq;

    use super::{
      support::{journal, seed_character},
      *,
    };
    use crate::store::{self, repo::finance};

    #[tokio::test]
    async fn it_emits_one_entry_per_trailing_month_oldest_first() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();

      let history = monthly_history(&db, "2026-06", 3).await;

      assert_eq!(history.len(), 3);
      assert_eq!(history[0].month, "2026-04");
      assert_eq!(history[2].month, "2026-06");
    }

    #[tokio::test]
    async fn it_splits_income_and_spend_and_ages_the_spend() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_scope(&db).await.unwrap();
      finance::append_wallet_journal(
        &db,
        &[
          journal(1, 1, "bounty_prizes", 1_000.0, "2026-06-01T00:00:00Z"),
          journal(2, 1, "brokers_fee", -400.0, "2026-06-11T00:00:00Z"),
        ],
      )
      .await
      .unwrap();

      let history = monthly_history(&db, "2026-06", 1).await;

      assert_eq!(history.len(), 1);
      assert_eq!(history[0].income, 1_000.0);
      assert_eq!(history[0].spend, 400.0);
      assert_eq!(history[0].age, 10.0);
    }
  }
}
