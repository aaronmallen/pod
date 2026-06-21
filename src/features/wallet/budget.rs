use chrono::{Datelike, Utc};

use crate::{
  features::budget as math,
  store::{
    Database,
    model::BudgetScope,
    repo::budget::{self, NewCategory, NewGroup, TargetInput},
  },
};

const AVERAGE_WINDOW: usize = 3;
const DEFAULT_CATEGORY_NAME: &str = "New Category";
const DEFAULT_GROUP_NAME: &str = "New Group";
const DEFAULT_TONE: &str = "plasma";

const TONE_INFO: iced::Color = crate::ui::style::color::chart::VIOLET;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
  #[default]
  Plan,
  Reflect,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InspectorTab {
  #[default]
  Detail,
  Automation,
}

/// RTA is derived (pool − Σ assigned), not stored; a move to it just sheds the
/// amount from the source rather than writing a second assignment row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveDest {
  Category(i64),
  ReadyToAssign,
}

/// The Reflect flow chart's trailing window, mirroring the design's 3M/6M
/// toggle. `SixMonths` is the default (the wireframe opens on 6).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BudgetRange {
  #[default]
  SixMonths,
  ThreeMonths,
}

impl BudgetRange {
  pub fn months(self) -> usize {
    match self {
      BudgetRange::SixMonths => 6,
      BudgetRange::ThreeMonths => 3,
    }
  }
}

/// The single active inline editor: which category's Assigned cell is open and
/// the in-progress draft text. Only one cell edits at a time.
#[derive(Clone, Debug, PartialEq)]
pub struct EditingCell {
  pub category_id: i64,
  pub draft: String,
}

/// The inspector's category/target editor working copy. Edits stay local until
/// committed, so the draft mirrors every field the editor can change. Reused by
/// B5's bulk edit mode.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoryDraft {
  pub by_date: String,
  pub category_id: i64,
  pub group_id: i64,
  pub name: String,
  pub note: String,
  pub position: i64,
  pub target_amount: f64,
  pub target_amount_text: String,
  pub target_kind: TargetKind,
  pub tone: Option<String>,
}

impl CategoryDraft {
  /// Builds a fresh draft from a loaded category and its owning group.
  pub fn from_category(group_id: i64, position: i64, category: &Category) -> Self {
    use crate::ui::format::fmt_isk;
    CategoryDraft {
      by_date: category.target.by_date.clone().unwrap_or_default(),
      category_id: category.id,
      group_id,
      name: category.name.clone(),
      note: category.note.clone().unwrap_or_default(),
      position,
      target_amount: category.target.amount,
      target_amount_text: fmt_isk(category.target.amount),
      target_kind: category.target.kind,
      tone: category.tone.clone(),
    }
  }

  /// The persistable category row for this draft.
  pub fn to_category_row(&self, created_at: String, updated_at: String) -> crate::store::model::BudgetCategory {
    crate::store::model::BudgetCategory {
      created_at,
      group_id: self.group_id,
      id: self.category_id,
      name: self.name.clone(),
      note: (!self.note.is_empty()).then(|| self.note.clone()),
      position: self.position,
      tone: self.tone.clone(),
      updated_at,
    }
  }

  /// The target this draft would persist.
  pub fn to_target(&self) -> Target {
    Target {
      amount: self.target_amount,
      by_date: (self.target_kind == TargetKind::GoalBy && !self.by_date.is_empty()).then(|| self.by_date.clone()),
      kind: self.target_kind,
    }
  }
}

/// Which advanced-mode select dropdown is open in the rule editor, by condition
/// row index and slot. Only one is open at a time; the value-editor select is the
/// field's keyed picker (Type/Character/Direction).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleSelectKey {
  Field(usize),
  Op(usize),
  Value(usize),
}

/// The rule editor's working copy: a new or existing automation rule edited
/// locally until committed. `rule_id` is `None` for a brand-new rule and
/// `Some(id)` when editing an existing one. `conditions` carries the live,
/// possibly-incomplete condition rows; inactive rows are dropped at match time.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleDraft {
  pub category_id: i64,
  pub conditions: Vec<crate::store::model::RuleCondition>,
  pub enabled: bool,
  pub match_mode: crate::store::model::MatchMode,
  pub name: String,
  pub name_edited: bool,
  pub open_select: Option<RuleSelectKey>,
  pub rule_id: Option<i64>,
  pub show_advanced: bool,
}

impl RuleDraft {
  /// A fresh draft filing into `category_id`, seeded with one empty text-contains
  /// condition so the search box has a row to bind to.
  pub fn new(category_id: i64) -> Self {
    RuleDraft {
      category_id,
      conditions: vec![math::new_condition(crate::store::model::RuleField::Text)],
      enabled: true,
      match_mode: crate::store::model::MatchMode::All,
      name: String::new(),
      name_edited: false,
      open_select: None,
      rule_id: None,
      show_advanced: false,
    }
  }

  /// A draft seeded from an existing rule, opening advanced mode unless the rule
  /// is a single text-contains condition (the search-box-first shape).
  pub fn from_rule(rule: &crate::store::model::Rule) -> Self {
    use crate::store::model::{RuleField, RuleOp};
    let conditions = rule.conditions().clone();
    let simple = matches!(
      conditions.as_slice(),
      [only] if only.field() == RuleField::Text && only.op() == RuleOp::Contains
    );
    RuleDraft {
      category_id: rule.category_id(),
      conditions,
      enabled: rule.enabled(),
      match_mode: rule.match_mode(),
      name: rule.name().clone(),
      name_edited: !rule.name().is_empty(),
      open_select: None,
      rule_id: Some(rule.id()),
      show_advanced: !simple,
    }
  }

  pub fn search_index(&self) -> Option<usize> {
    use crate::store::model::{RuleField, RuleOp};
    self
      .conditions
      .iter()
      .position(|c| c.field() == RuleField::Text && c.op() == RuleOp::Contains)
  }

  pub fn search_value(&self) -> &str {
    self
      .search_index()
      .map(|index| self.conditions[index].value())
      .map_or("", String::as_str)
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetKind {
  Balance,
  Goal,
  GoalBy,
  Monthly,
  Refill,
}

impl TargetKind {
  pub fn all() -> [TargetKind; 5] {
    [
      TargetKind::Monthly,
      TargetKind::Refill,
      TargetKind::Balance,
      TargetKind::Goal,
      TargetKind::GoalBy,
    ]
  }

  pub fn from_storage(kind: &str) -> TargetKind {
    match kind {
      "refill" => TargetKind::Refill,
      "balance" => TargetKind::Balance,
      "goal" => TargetKind::Goal,
      "goalby" => TargetKind::GoalBy,
      _ => TargetKind::Monthly,
    }
  }

  pub fn amount_label(self) -> &'static str {
    match self {
      TargetKind::Monthly => "Amount per month",
      TargetKind::Refill => "Refill up to",
      _ => "Target amount",
    }
  }

  pub fn hint(self) -> &'static str {
    match self {
      TargetKind::Monthly => "Assign a set amount every month, then spend it down.",
      TargetKind::Refill => "Top the Available balance back up to a number each month.",
      TargetKind::Balance => "Build a standing reserve and hold it there. Open-ended.",
      TargetKind::Goal => "Save toward a number. No deadline.",
      TargetKind::GoalBy => "Save a number by a deadline.",
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      TargetKind::Monthly => "Monthly",
      TargetKind::Refill => "Refill",
      TargetKind::Balance => "Balance",
      TargetKind::Goal => "Goal",
      TargetKind::GoalBy => "By date",
    }
  }

  pub fn to_storage(self) -> &'static str {
    match self {
      TargetKind::Monthly => "monthly",
      TargetKind::Refill => "refill",
      TargetKind::Balance => "balance",
      TargetKind::Goal => "goal",
      TargetKind::GoalBy => "goalby",
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetState {
  Met,
  Over,
  Under,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Target {
  pub amount: f64,
  pub by_date: Option<String>,
  pub kind: TargetKind,
}

impl Default for Target {
  fn default() -> Self {
    Target {
      amount: 0.0,
      by_date: None,
      kind: TargetKind::Monthly,
    }
  }
}

/// A category's status against its target, mirroring `targetStatus` in
/// `budget-data.jsx`: a 0.0–1.0 progress `pct`, the `needed` shortfall, the
/// `state`, a descriptive `label`, and a per-month progress `month_label`.
#[derive(Clone, Debug, PartialEq)]
pub struct TargetStatus {
  pub label: String,
  pub month_label: String,
  pub needed: f64,
  pub pct: f64,
  pub state: TargetState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Category {
  pub activity: f64,
  pub assigned: f64,
  pub avg_assigned: f64,
  pub carry: f64,
  pub id: i64,
  pub last_assigned: f64,
  pub name: String,
  pub note: Option<String>,
  pub spent_last: f64,
  pub target: Target,
  pub tone: Option<String>,
}

impl Category {
  pub fn available(&self) -> f64 {
    self.carry + self.assigned + self.activity
  }

  pub fn status(&self, month: &str) -> TargetStatus {
    target_status(&self.target, self.assigned, self.available(), month)
  }

  /// The "Underfunded" quick-assign suggestion as of `month`: the assignment
  /// that satisfies this month's target. Monthly targets raise the assignment to
  /// the amount; dated goals top up only the paced slice; the other cumulative
  /// targets top the available balance up to the amount.
  pub fn underfunded_assign(&self, month: &str) -> f64 {
    match self.target.kind {
      TargetKind::Monthly => self.assigned.max(self.target.amount),
      TargetKind::GoalBy => self.assigned + goal_needed(&self.target, self.available(), month),
      _ => self.assigned + (self.target.amount - self.available()).max(0.0),
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
  pub categories: Vec<Category>,
  pub id: i64,
  pub name: String,
}

impl Group {
  pub fn totals(&self) -> GroupTotals {
    let mut totals = GroupTotals::default();
    for category in &self.categories {
      totals.activity += category.activity;
      totals.assigned += category.assigned;
      totals.available += category.available();
    }
    totals
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GroupTotals {
  pub activity: f64,
  pub assigned: f64,
  pub available: f64,
}

/// The fully-derived Budget Plan view-model for one scope and month: the
/// envelope groups with live carry/activity figures and the budgetable pool's
/// Ready-to-Assign / overspending top-line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BudgetView {
  pub groups: Vec<Group>,
  pub month: String,
  pub overspent: f64,
  pub pool: f64,
  pub ready_to_assign: f64,
}

impl BudgetView {
  pub fn category(&self, id: i64) -> Option<&Category> {
    self
      .groups
      .iter()
      .flat_map(|group| &group.categories)
      .find(|c| c.id == id)
  }

  pub fn first_category_id(&self) -> Option<i64> {
    self
      .groups
      .iter()
      .flat_map(|group| &group.categories)
      .map(|c| c.id)
      .next()
  }

  /// Reorders the in-memory groups so `dragged` lands in `target_group`,
  /// inserted before `before` (or appended when `before` is `None`), mirroring
  /// the design's `moveCat`. A no-op when the drag would not change the order
  /// (dropping a category onto itself). Returns `true` when the order changed.
  pub fn move_category(&mut self, dragged: i64, target_group: i64, before: Option<i64>) -> bool {
    if before == Some(dragged) {
      return false;
    }
    let mut moving = None;
    for group in &mut self.groups {
      if let Some(index) = group.categories.iter().position(|c| c.id == dragged) {
        moving = Some(group.categories.remove(index));
        break;
      }
    }
    let Some(moving) = moving else {
      return false;
    };
    for group in &mut self.groups {
      if group.id != target_group {
        continue;
      }
      let index = before
        .and_then(|id| group.categories.iter().position(|c| c.id == id))
        .unwrap_or(group.categories.len());
      group.categories.insert(index, moving);
      return true;
    }
    false
  }

  /// Reorders the in-memory groups so `dragged` lands before `before` (or is
  /// appended when `before` is `None`), mirroring [`move_category`] at the group
  /// level. A no-op when the drag would not change the order (dropping a group
  /// onto itself). Returns `true` when the order changed.
  pub fn move_group(&mut self, dragged: i64, before: Option<i64>) -> bool {
    if before == Some(dragged) {
      return false;
    }
    let Some(index) = self.groups.iter().position(|g| g.id == dragged) else {
      return false;
    };
    let moving = self.groups.remove(index);
    let insert = before
      .and_then(|id| self.groups.iter().position(|g| g.id == id))
      .unwrap_or(self.groups.len());
    self.groups.insert(insert, moving);
    true
  }
}

/// The fully-derived Reflect (reporting) view-model for one scope and month: the
/// stat-band totals, the trailing monthly history (for the flow chart and
/// age-of-ISK sparkline), the spend-by-category rows, and the target-health
/// tally. All figures come from the live [`BudgetView`] and B2's history so the
/// reports reflect Plan edits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReflectView {
  pub age: f64,
  pub age_delta: f64,
  pub assigned: f64,
  pub history: Vec<crate::features::budget::MonthFlow>,
  pub income: f64,
  pub prev_label: String,
  pub spend: f64,
  pub spend_rows: Vec<SpendRow>,
  pub tally: TargetTally,
}

/// One spend-by-category row for the Reflect view: the category's display name,
/// tone, and the ISK spent this month (the absolute negative activity), already
/// sorted descending by `spend`.
#[derive(Clone, Debug, PartialEq)]
pub struct SpendRow {
  pub name: String,
  pub spend: f64,
  pub tone: Option<String>,
}

/// A single "Needs attention" entry: a category that is underfunded or
/// overspent, with the figure the design surfaces (shortfall for under, the
/// negative available for over).
#[derive(Clone, Debug, PartialEq)]
pub struct TargetAlert {
  pub amount: f64,
  pub name: String,
  pub over: bool,
}

/// The target-health tally for the Reflect view: how many categories are met
/// (funded), underfunded, or overspent, plus the worst few that "need
/// attention" with their shortfall (under) or overspend available (over).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TargetTally {
  pub attention: Vec<TargetAlert>,
  pub met: usize,
  pub over: usize,
  pub under: usize,
}

impl ReflectView {
  pub fn net(&self) -> f64 {
    self.income - self.spend
  }
}

/// Derives the Reflect view-model from a live [`BudgetView`] and a trailing
/// `history` (oldest first, current month last). The stat band, spend rows, and
/// target tally come from the current month's categories so they track Plan
/// edits; the age-of-ISK figure and its delta come from the FIFO history.
pub fn reflect(view: &BudgetView, history: Vec<crate::features::budget::MonthFlow>) -> ReflectView {
  let mut assigned = 0.0;
  let mut income = 0.0;
  let mut spend = 0.0;
  let mut spend_rows: Vec<SpendRow> = Vec::new();
  let mut tally = TargetTally::default();

  for category in view.groups.iter().flat_map(|group| &group.categories) {
    assigned += category.assigned;
    if category.activity > 0.0 {
      income += category.activity;
    } else if category.activity < 0.0 {
      spend += -category.activity;
      spend_rows.push(SpendRow {
        name: category.name.clone(),
        spend: -category.activity,
        tone: category.tone.clone(),
      });
    }
    let status = category.status(&view.month);
    match status.state {
      TargetState::Met => tally.met += 1,
      TargetState::Over => {
        tally.over += 1;
        tally.attention.push(TargetAlert {
          amount: category.available(),
          name: category.name.clone(),
          over: true,
        });
      }
      TargetState::Under => {
        tally.under += 1;
        tally.attention.push(TargetAlert {
          amount: status.needed,
          name: category.name.clone(),
          over: false,
        });
      }
    }
  }
  spend_rows.sort_by(|a, b| b.spend.total_cmp(&a.spend));

  let age = history.last().map_or(0.0, |m| m.age);
  let age_delta = match history.len() {
    0 | 1 => 0.0,
    n => age - history[n - 2].age,
  };
  let prev_label = match history.len() {
    0 | 1 => String::new(),
    n => month_short_label(&history[n - 2].month),
  };

  ReflectView {
    age,
    age_delta,
    assigned,
    history,
    income,
    prev_label,
    spend,
    spend_rows,
    tally,
  }
}

/// A short month label (e.g. `Jun`) for a `YYYY-MM` key, used by the flow chart
/// axis and the age delta caption. Falls back to the key verbatim.
pub fn month_short_label(month: &str) -> String {
  const NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
  ];
  match parse_month(month) {
    Some((_, mon)) => NAMES[(mon - 1) as usize].to_owned(),
    None => month.to_owned(),
  }
}

/// Maps a category's stored `tone` slug to a render colour, matching the
/// design's `toneColor`. Unknown or absent tones fall back to muted text.
pub fn tone_color(tone: Option<&str>) -> iced::Color {
  use crate::ui::style::color;
  match tone {
    Some("plasma") => color::accent::PLASMA,
    Some("success") => color::status::ONLINE,
    Some("danger") => color::status::DANGER,
    Some("warning") => color::status::WARNING,
    Some("info") => TONE_INFO,
    _ => color::text::secondary(),
  }
}

/// The tone slugs offered by the category editor, in the design's order.
pub fn tone_options() -> [&'static str; 6] {
  ["plasma", "success", "warning", "danger", "info", "muted"]
}

/// The current UTC calendar month key (`YYYY-MM`).
pub fn current_month() -> String {
  let now = Utc::now();
  format!("{:04}-{:02}", now.year(), now.month())
}

/// The month key `delta` months away from `month` (`YYYY-MM`). Negative steps
/// move into the past; positive into the future. Returns `month` unchanged if it
/// is not a valid key.
pub fn shift_month(month: &str, delta: i32) -> String {
  let Some((year, mon)) = parse_month(month) else {
    return month.to_owned();
  };
  let zero_based = (year * 12 + (mon - 1)) + delta;
  let new_year = zero_based.div_euclid(12);
  let new_month = zero_based.rem_euclid(12) + 1;
  format!("{new_year:04}-{new_month:02}")
}

/// A human month label (e.g. `June 2026`) for a `YYYY-MM` key, or the key
/// verbatim when it cannot be parsed.
pub fn month_label(month: &str) -> String {
  const NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
  ];
  match parse_month(month) {
    Some((year, mon)) => format!("{} {year}", NAMES[(mon - 1) as usize]),
    None => month.to_owned(),
  }
}

/// A relative descriptor for a `YYYY-MM` key against the current UTC month:
/// "This month" only when it *is* the current month, otherwise "Last month" /
/// "Next month" or "N months ago" / "In N months". Empty when unparseable.
pub fn month_relative_label(month: &str) -> String {
  let (Some((year, mon)), Some((current_year, current_mon))) = (parse_month(month), parse_month(&current_month()))
  else {
    return String::new();
  };
  let delta = (year * 12 + (mon - 1)) - (current_year * 12 + (current_mon - 1));
  match delta {
    0 => "This month".to_owned(),
    -1 => "Last month".to_owned(),
    1 => "Next month".to_owned(),
    months if months < 0 => format!("{} months ago", -months),
    months => format!("In {months} months"),
  }
}

fn parse_month(month: &str) -> Option<(i32, i32)> {
  let (year, mon) = month.split_once('-')?;
  let year = year.parse::<i32>().ok()?;
  let mon = mon.parse::<i32>().ok()?;
  (1..=12).contains(&mon).then_some((year, mon))
}

/// The target status for an `assigned`/`available` pair as of `month`
/// (`YYYY-MM`), ported from `targetStatus` in `budget-data.jsx`. `month` only
/// affects dated goals, whose monthly `needed` is paced toward `by_date`.
pub fn target_status(target: &Target, assigned: f64, available: f64, month: &str) -> TargetStatus {
  use crate::ui::format::fmt_isk;

  let amount = target.amount;
  let (pct, needed, met, label, month_label) = match target.kind {
    TargetKind::Monthly => (
      progress(assigned, amount),
      (amount - assigned).max(0.0),
      assigned >= amount - 1.0,
      format!("Assign {} every month", fmt_isk(amount)),
      format!("{} of {} assigned", fmt_isk(assigned), fmt_isk(amount)),
    ),
    TargetKind::Refill => (
      progress(available, amount),
      (amount - available).max(0.0),
      available >= amount - 1.0,
      format!("Refill up to {} each month", fmt_isk(amount)),
      format!("{} of {} available", fmt_isk(available), fmt_isk(amount)),
    ),
    TargetKind::Balance => (
      progress(available, amount),
      (amount - available).max(0.0),
      available >= amount - 1.0,
      format!("Build a balance of {}", fmt_isk(amount)),
      format!("{} of {} saved", fmt_isk(available), fmt_isk(amount)),
    ),
    TargetKind::Goal | TargetKind::GoalBy => {
      let pct = progress(available, amount);
      let label = if target.kind == TargetKind::GoalBy {
        format!(
          "Save {} by {}",
          fmt_isk(amount),
          target.by_date.as_deref().unwrap_or("\u{2014}")
        )
      } else {
        format!("Save toward {}", fmt_isk(amount))
      };
      (
        pct,
        goal_needed(target, available, month),
        available >= amount - 1.0,
        label,
        format!(
          "{} of {} \u{b7} {}%",
          fmt_isk(available),
          fmt_isk(amount),
          (pct * 100.0).round() as i64
        ),
      )
    }
  };

  let state = if available < 0.0 {
    TargetState::Over
  } else if met {
    TargetState::Met
  } else {
    TargetState::Under
  };

  TargetStatus {
    label,
    month_label,
    needed,
    pct,
    state,
  }
}

/// This month's shortfall for a save-toward target. Open-ended goals demand the
/// whole remainder; dated goals pace it across the months left until `by_date`,
/// so `(amount - available) / months_remaining` shrinks as the goal funds and
/// grows as the deadline nears, collapsing to the full remainder in and after
/// the final month.
fn goal_needed(target: &Target, available: f64, month: &str) -> f64 {
  let remainder = (target.amount - available).max(0.0);
  if target.kind != TargetKind::GoalBy {
    return remainder;
  }
  match target.by_date.as_deref().and_then(|by| months_remaining(by, month)) {
    Some(months) => remainder / months as f64,
    None => remainder,
  }
}

/// The count of months from `month` through the `by_date` deadline, inclusive of
/// both ends, so the deadline month itself is the final pacing slice. `None`
/// when either side is unparseable; clamped to at least 1 so a past-due or
/// final-month goal demands its whole remainder.
fn months_remaining(by_date: &str, month: &str) -> Option<usize> {
  let (by_year, by_mon) = parse_by_date(by_date)?;
  let (year, mon) = parse_month(month)?;
  let span = (by_year * 12 + (by_mon - 1)) - (year * 12 + (mon - 1)) + 1;
  Some(span.max(1) as usize)
}

/// Parses a `by_date` label into `(year, month)`. Accepts the editor's
/// `Mon YYYY` form (e.g. `Jan 2028`) as well as ISO `YYYY-MM` / `YYYY-MM-DD`,
/// since the field is free text and no single stored format is guaranteed.
fn parse_by_date(by_date: &str) -> Option<(i32, i32)> {
  const NAMES: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
  ];
  let trimmed = by_date.trim();
  if let Some((year, rest)) = trimmed.split_once('-') {
    let year = year.parse::<i32>().ok()?;
    let mon = rest.split('-').next()?.parse::<i32>().ok()?;
    return (1..=12).contains(&mon).then_some((year, mon));
  }
  let (name, year) = trimmed.split_once(char::is_whitespace)?;
  let mon = NAMES.iter().position(|n| name.to_ascii_lowercase().starts_with(n))? as i32 + 1;
  let year = year.trim().parse::<i32>().ok()?;
  Some((year, mon))
}

fn progress(numerator: f64, denominator: f64) -> f64 {
  if denominator > 0.0 {
    (numerator / denominator).clamp(0.0, 1.0)
  } else {
    1.0
  }
}

/// Loads the fully-derived Budget Plan view-model for `scope` and `month`,
/// seeding the scope's starter envelopes on first use. Carry, activity and the
/// budgetable pool come from the B2 math; assignments and targets from B1.
pub async fn load(db: &Database, scope: BudgetScope, month: &str) -> BudgetView {
  // seed_scope is once-only (persisted marker), so deleting every group never
  // re-seeds the defaults on the next load.
  let _ = math::seed_scope(db, scope).await;

  // One batched pass yields every month's activity, so the multi-month carry
  // chain reads real per-month activity without a query per (category, month).
  let activity_by_month = math::activity_by_month(db, scope).await;
  let empty_month = std::collections::HashMap::new();
  let activity = activity_by_month.get(month).unwrap_or(&empty_month);
  let pool = math::budgetable_pool(db, scope).await;
  let assigned_total = budget::scope_assigned_total(db, scope).await.unwrap_or(0.0);
  // Income a rule/override has filed into an envelope is reserved out of the pool
  // so it does not also show as assignable (see `pool_summary`).
  let categorized_inflows = math::categorized_inflow_total(&activity_by_month);
  let prev_month = shift_month(month, -1);
  let prev_activity = activity_by_month.get(&prev_month).unwrap_or(&empty_month);

  let mut groups: Vec<Group> = Vec::new();
  let mut availables: Vec<f64> = Vec::new();
  for group_row in budget::list_groups(db, scope).await.unwrap_or_default() {
    let mut categories: Vec<Category> = Vec::new();
    for category_row in budget::list_categories(db, group_row.id()).await.unwrap_or_default() {
      let category = build_category(
        db,
        &category_row,
        month,
        activity,
        &prev_month,
        prev_activity,
        &activity_by_month,
      )
      .await;
      availables.push(category.available());
      categories.push(category);
    }
    groups.push(Group {
      categories,
      id: group_row.id(),
      name: group_row.name().clone(),
    });
  }

  let summary = math::pool_summary(pool, assigned_total, categorized_inflows, availables);
  BudgetView {
    groups,
    month: month.to_owned(),
    overspent: summary.overspent,
    pool: summary.pool,
    ready_to_assign: summary.ready_to_assign,
  }
}

async fn build_category(
  db: &Database,
  row: &crate::store::model::BudgetCategory,
  month: &str,
  activity: &std::collections::HashMap<i64, f64>,
  prev_month: &str,
  prev_activity: &std::collections::HashMap<i64, f64>,
  activity_by_month: &std::collections::HashMap<String, std::collections::HashMap<i64, f64>>,
) -> Category {
  let assignments = budget::list_assignments(db, row.id()).await.unwrap_or_default();
  let assigned_for = |key: &str| {
    assignments
      .iter()
      .find(|a| a.month() == key)
      .map_or(0.0, |a| a.assigned())
  };

  let carry = carry_into(month, &assignments, activity_by_month, row.id());
  let last_assigned = assigned_for(prev_month);
  let avg_assigned = trailing_average(month, &assignments);
  let spent_last = (-prev_activity.get(&row.id()).copied().unwrap_or(0.0)).max(0.0);

  let target = budget::load_target(db, row.id())
    .await
    .unwrap_or_default()
    .map_or_else(Target::default, |t| Target {
      amount: t.amount(),
      by_date: t.by_date().clone(),
      kind: TargetKind::from_storage(t.kind()),
    });

  Category {
    activity: activity.get(&row.id()).copied().unwrap_or(0.0),
    assigned: assigned_for(month),
    avg_assigned,
    carry,
    id: row.id(),
    last_assigned,
    name: row.name().clone(),
    note: row.note().clone(),
    spent_last,
    target,
    tone: row.tone().clone(),
  }
}

/// The carry rolled into `month` for a category: every month strictly before
/// `month` that has an assignment or real activity, in chronological order,
/// rolled forward through the B2 carry-over math. Each prior month contributes
/// its own assigned amount and its own real per-month activity (sourced from the
/// batched `activity_by_month` map), so spending in a non-adjacent prior month
/// reduces the carry exactly — not only the immediately preceding month.
fn carry_into(
  month: &str,
  assignments: &[crate::store::model::BudgetAssignment],
  activity_by_month: &std::collections::HashMap<String, std::collections::HashMap<i64, f64>>,
  category_id: i64,
) -> f64 {
  let mut keys: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
  for assignment in assignments {
    if assignment.month().as_str() < month {
      keys.insert(assignment.month().as_str());
    }
  }
  for key in activity_by_month.keys() {
    if key.as_str() < month && activity_by_month[key].contains_key(&category_id) {
      keys.insert(key.as_str());
    }
  }
  if keys.is_empty() {
    return 0.0;
  }

  let months: Vec<(f64, f64)> = keys
    .iter()
    .map(|&key| {
      let assigned = assignments
        .iter()
        .find(|a| a.month() == key)
        .map_or(0.0, |a| a.assigned());
      let activity = activity_by_month
        .get(key)
        .and_then(|m| m.get(&category_id))
        .copied()
        .unwrap_or(0.0);
      (assigned, activity)
    })
    .collect();
  // BTreeSet yields keys in ascending month order, matching roll_carry's contract.
  let rolled = math::roll_carry(0.0, &months);
  math::carry_from(rolled.last().map(|m| m.available()))
}

fn trailing_average(month: &str, assignments: &[crate::store::model::BudgetAssignment]) -> f64 {
  let mut prior: Vec<f64> = assignments
    .iter()
    .filter(|a| a.month().as_str() < month)
    .map(crate::store::model::BudgetAssignment::assigned)
    .collect();
  if prior.is_empty() {
    return 0.0;
  }
  let take = prior.len().min(AVERAGE_WINDOW);
  let window = &prior.split_off(prior.len() - take);
  window.iter().sum::<f64>() / take as f64
}

/// The value MAY be negative: assigning below zero is the YNAB mechanism for
/// moving rolled-over (carry) or income out of a category. There is no `>= 0`
/// clamp; a negative available surfaces in the Cover-Overspending flow.
pub async fn persist_assignment(db: &Database, category_id: i64, month: &str, value: f64) {
  let _ = budget::upsert_assignment(db, category_id, month, value.round()).await;
}

/// Auto-assigns the Ready-to-Assign pool to underfunded categories in order,
/// persisting each top-up, exactly as the design's `autoAssign`.
pub async fn auto_assign(db: &Database, view: &BudgetView) {
  let mut pool = view.ready_to_assign;
  for group in &view.groups {
    for category in &group.categories {
      if pool <= 0.0 {
        return;
      }
      let status = category.status(&view.month);
      if status.needed > 0.0 {
        let give = pool.min(status.needed);
        pool -= give;
        persist_assignment(db, category.id, &view.month, category.assigned + give).await;
      }
    }
  }
}

/// Covers every overspent category by raising its assignment to clear the
/// negative available, persisting each change, as the design's
/// `coverOverspending`.
pub async fn cover_overspending(db: &Database, view: &BudgetView) {
  for group in &view.groups {
    for category in &group.categories {
      let available = category.available();
      if available < 0.0 {
        persist_assignment(db, category.id, &view.month, category.assigned - available).await;
      }
    }
  }
}

/// The source assignment MAY go negative (carry moves the full available into
/// another envelope). Nothing is silently drawn from RTA; conservation is exact:
/// whatever leaves the source arrives at the destination or returns to the pool.
pub async fn move_money(db: &Database, view: &BudgetView, from_id: i64, to: MoveDest, amount: f64) {
  let amount = amount.round();
  if amount <= 0.0 {
    return;
  }
  let Some(source) = view.category(from_id) else {
    return;
  };
  let _ = budget::upsert_assignment(db, from_id, &view.month, source.assigned - amount).await;
  if let MoveDest::Category(to_id) = to
    && let Some(dest) = view.category(to_id)
  {
    let _ = budget::upsert_assignment(db, to_id, &view.month, dest.assigned + amount).await;
  }
}

/// Creates an empty category at the end of `group_id`, seeded with a default
/// name, tone and a zero monthly target, and returns its new id for selection.
pub async fn add_category(db: &Database, group_id: i64, position: i64) -> Option<i64> {
  let category = budget::create_category(
    db,
    &NewCategory {
      group_id,
      name: DEFAULT_CATEGORY_NAME.to_owned(),
      note: None,
      position,
      tone: Some(DEFAULT_TONE.to_owned()),
    },
  )
  .await
  .ok()?;
  let _ = budget::set_target(
    db,
    category.id(),
    &TargetInput {
      amount: 0.0,
      by_date: None,
      kind: TargetKind::Monthly.to_storage().to_owned(),
    },
  )
  .await;
  Some(category.id())
}

/// Creates an empty category group at the end of `scope`, seeded with a default
/// name, and returns its new id.
pub async fn add_group(db: &Database, scope: BudgetScope, position: i64) -> Option<i64> {
  budget::create_group(
    db,
    &NewGroup {
      name: DEFAULT_GROUP_NAME.to_owned(),
      position,
      scope,
    },
  )
  .await
  .ok()
  .map(|group| group.id())
}

/// Deletes a category. The B1 schema cascades its target, assignments and
/// ref-type maps.
pub async fn delete_category(db: &Database, category_id: i64) {
  let _ = budget::delete_category(db, category_id).await;
}

/// Deletes a category group. The B1 schema cascades every category it holds.
pub async fn delete_group(db: &Database, group_id: i64) {
  let _ = budget::delete_group(db, group_id).await;
}

/// Persists the current group/category ordering: each category's `position`
/// within its group and its owning `group_id`, so a drag-reorder survives a
/// reload. Only the position and group change; other fields are read back from
/// the row so concurrent target edits are preserved.
pub async fn persist_order(db: &Database, view: &BudgetView) {
  let now = chrono::Utc::now().to_rfc3339();
  for group in &view.groups {
    for (position, category) in group.categories.iter().enumerate() {
      let row = crate::store::model::BudgetCategory {
        created_at: now.clone(),
        group_id: group.id,
        id: category.id,
        name: category.name.clone(),
        note: category.note.clone(),
        position: position as i64,
        tone: category.tone.clone(),
        updated_at: now.clone(),
      };
      let _ = budget::update_category(db, &row).await;
    }
  }
}

/// Persists the current group ordering: each group's `position` in display order
/// so a drag-reorder of groups survives a reload. `update_group` writes only the
/// name and position, so the scope fields are placeholders and never reach the row.
pub async fn persist_group_order(db: &Database, view: &BudgetView) {
  let now = chrono::Utc::now().to_rfc3339();
  for (position, group) in view.groups.iter().enumerate() {
    let row = crate::store::model::BudgetCategoryGroup {
      created_at: now.clone(),
      id: group.id,
      name: group.name.clone(),
      position: position as i64,
      scope_id: None,
      scope_kind: "all".to_owned(),
      updated_at: now.clone(),
    };
    let _ = budget::update_group(db, &row).await;
  }
}

/// Renames a category group, preserving its position.
pub async fn rename_group(db: &Database, group_id: i64, name: &str) {
  let _ = budget::rename_group(db, group_id, name).await;
}

/// Persists the category metadata edits and target from the inspector editor.
pub async fn persist_category_edit(db: &Database, category: &crate::store::model::BudgetCategory, target: &Target) {
  let _ = budget::update_category(db, category).await;
  let _ = budget::set_target(
    db,
    category.id(),
    &TargetInput {
      amount: target.amount,
      by_date: target.by_date.clone(),
      kind: target.kind.to_storage().to_owned(),
    },
  )
  .await;
}

#[cfg(test)]
mod tests {
  use super::*;

  mod rule_draft {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{MatchMode, Rule, RuleCondition, RuleField, RuleOp};

    fn rule(conditions: Vec<RuleCondition>) -> Rule {
      Rule {
        category_id: 7,
        conditions,
        enabled: true,
        id: 11,
        match_mode: MatchMode::All,
        name: "Broker fees".to_owned(),
      }
    }

    fn contains(value: &str) -> RuleCondition {
      RuleCondition {
        field: RuleField::Text,
        op: RuleOp::Contains,
        value: value.to_owned(),
        value2: None,
      }
    }

    #[test]
    fn it_seeds_a_new_draft_with_one_empty_text_condition() {
      let draft = RuleDraft::new(3);

      assert_eq!(draft.category_id, 3);
      assert_eq!(draft.conditions.len(), 1);
      assert_eq!(draft.conditions[0].field(), RuleField::Text);
      assert_eq!(draft.rule_id, None);
      assert!(!draft.show_advanced);
    }

    #[test]
    fn it_opens_advanced_for_a_multi_condition_rule() {
      let draft = RuleDraft::from_rule(&rule(vec![
        contains("Cerberus"),
        RuleCondition {
          field: RuleField::Type,
          op: RuleOp::Is,
          value: "broker_fee".to_owned(),
          value2: None,
        },
      ]));

      assert_eq!(draft.rule_id, Some(11));
      assert!(draft.show_advanced);
      assert!(draft.name_edited);
    }

    #[test]
    fn it_keeps_simple_mode_for_a_single_text_contains_rule() {
      let draft = RuleDraft::from_rule(&rule(vec![contains("SKIN")]));

      assert!(!draft.show_advanced);
      assert_eq!(draft.search_value(), "SKIN");
    }

    #[test]
    fn it_reports_an_empty_search_when_no_text_contains_condition_exists() {
      let draft = RuleDraft::from_rule(&rule(vec![RuleCondition {
        field: RuleField::Amount,
        op: RuleOp::GreaterThan,
        value: "100m".to_owned(),
        value2: None,
      }]));

      assert_eq!(draft.search_index(), None);
      assert_eq!(draft.search_value(), "");
    }
  }

  mod move_category {
    use pretty_assertions::assert_eq;

    use super::*;

    fn category(id: i64) -> Category {
      Category {
        activity: 0.0,
        assigned: 0.0,
        avg_assigned: 0.0,
        carry: 0.0,
        id,
        last_assigned: 0.0,
        name: format!("Cat {id}"),
        note: None,
        spent_last: 0.0,
        target: Target::default(),
        tone: None,
      }
    }

    fn view() -> BudgetView {
      BudgetView {
        groups: vec![
          Group {
            categories: vec![category(1), category(2), category(3)],
            id: 10,
            name: "Bills".to_owned(),
          },
          Group {
            categories: vec![category(4)],
            id: 20,
            name: "Wants".to_owned(),
          },
        ],
        month: "2026-06".to_owned(),
        overspent: 0.0,
        pool: 0.0,
        ready_to_assign: 0.0,
      }
    }

    fn ids(group: &Group) -> Vec<i64> {
      group.categories.iter().map(|c| c.id).collect()
    }

    #[test]
    fn it_reorders_within_a_group_before_the_target() {
      let mut view = view();

      let moved = view.move_category(3, 10, Some(1));

      assert!(moved);
      assert_eq!(ids(&view.groups[0]), [3, 1, 2]);
    }

    #[test]
    fn it_moves_a_category_across_groups() {
      let mut view = view();

      let moved = view.move_category(2, 20, Some(4));

      assert!(moved);
      assert_eq!(ids(&view.groups[0]), [1, 3]);
      assert_eq!(ids(&view.groups[1]), [2, 4]);
    }

    #[test]
    fn it_appends_to_a_group_when_dropped_on_its_header() {
      let mut view = view();

      let moved = view.move_category(1, 20, None);

      assert!(moved);
      assert_eq!(ids(&view.groups[0]), [2, 3]);
      assert_eq!(ids(&view.groups[1]), [4, 1]);
    }

    #[test]
    fn it_is_a_no_op_when_dropped_on_itself() {
      let mut view = view();

      let moved = view.move_category(2, 10, Some(2));

      assert!(!moved);
      assert_eq!(ids(&view.groups[0]), [1, 2, 3]);
    }
  }

  mod move_group {
    use pretty_assertions::assert_eq;

    use super::*;

    fn group(id: i64) -> Group {
      Group {
        categories: Vec::new(),
        id,
        name: format!("Group {id}"),
      }
    }

    fn view() -> BudgetView {
      BudgetView {
        groups: vec![group(10), group(20), group(30)],
        month: "2026-06".to_owned(),
        overspent: 0.0,
        pool: 0.0,
        ready_to_assign: 0.0,
      }
    }

    fn ids(view: &BudgetView) -> Vec<i64> {
      view.groups.iter().map(|g| g.id).collect()
    }

    #[test]
    fn it_reorders_a_group_before_the_target() {
      let mut view = view();

      let moved = view.move_group(30, Some(10));

      assert!(moved);
      assert_eq!(ids(&view), [30, 10, 20]);
    }

    #[test]
    fn it_appends_a_group_when_dropped_with_no_target() {
      let mut view = view();

      let moved = view.move_group(10, None);

      assert!(moved);
      assert_eq!(ids(&view), [20, 30, 10]);
    }

    #[test]
    fn it_is_a_no_op_when_dropped_on_itself() {
      let mut view = view();

      let moved = view.move_group(20, Some(20));

      assert!(!moved);
      assert_eq!(ids(&view), [10, 20, 30]);
    }
  }

  mod crud {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      repo::budget::{NewGroup, create_group, list_categories, list_groups},
    };

    async fn seed_group(db: &Database, name: &str) -> i64 {
      create_group(
        db,
        &NewGroup {
          name: name.to_owned(),
          position: 0,
          scope: BudgetScope::All,
        },
      )
      .await
      .unwrap()
      .id()
    }

    #[tokio::test]
    async fn it_adds_a_category_with_a_default_target() {
      let db = store::open_test().await.unwrap();
      let group_id = seed_group(&db, "Bills").await;

      let id = add_category(&db, group_id, 0).await.unwrap();

      let categories = list_categories(&db, group_id).await.unwrap();
      assert_eq!(categories.len(), 1);
      assert_eq!(categories[0].id(), id);
      assert_eq!(budget::load_target(&db, id).await.unwrap().unwrap().kind(), "monthly");
    }

    #[tokio::test]
    async fn it_adds_and_deletes_a_group() {
      let db = store::open_test().await.unwrap();

      let id = add_group(&db, BudgetScope::All, 0).await.unwrap();
      assert_eq!(list_groups(&db, BudgetScope::All).await.unwrap().len(), 1);

      delete_group(&db, id).await;
      assert!(list_groups(&db, BudgetScope::All).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_renames_a_group_in_place() {
      let db = store::open_test().await.unwrap();
      let id = seed_group(&db, "Old").await;

      rename_group(&db, id, "New").await;

      let groups = list_groups(&db, BudgetScope::All).await.unwrap();
      assert_eq!(groups[0].name(), "New");
    }

    #[tokio::test]
    async fn it_persists_a_reordered_view() {
      let db = store::open_test().await.unwrap();
      let group_id = seed_group(&db, "Bills").await;
      let first = add_category(&db, group_id, 0).await.unwrap();
      let second = add_category(&db, group_id, 1).await.unwrap();

      let view = BudgetView {
        groups: vec![Group {
          categories: vec![category_row(second), category_row(first)],
          id: group_id,
          name: "Bills".to_owned(),
        }],
        month: "2026-06".to_owned(),
        overspent: 0.0,
        pool: 0.0,
        ready_to_assign: 0.0,
      };
      persist_order(&db, &view).await;

      let reloaded = list_categories(&db, group_id).await.unwrap();
      assert_eq!(reloaded.iter().map(|c| c.id()).collect::<Vec<_>>(), [second, first]);
    }

    #[tokio::test]
    async fn it_persists_a_reordered_group_list() {
      let db = store::open_test().await.unwrap();
      let first = seed_group(&db, "Bills").await;
      let second = seed_group(&db, "Wants").await;

      let view = BudgetView {
        groups: vec![
          Group {
            categories: Vec::new(),
            id: second,
            name: "Wants".to_owned(),
          },
          Group {
            categories: Vec::new(),
            id: first,
            name: "Bills".to_owned(),
          },
        ],
        month: "2026-06".to_owned(),
        overspent: 0.0,
        pool: 0.0,
        ready_to_assign: 0.0,
      };
      persist_group_order(&db, &view).await;

      let reloaded = list_groups(&db, BudgetScope::All).await.unwrap();
      assert_eq!(reloaded.iter().map(|g| g.id()).collect::<Vec<_>>(), [second, first]);
    }

    fn category_row(id: i64) -> Category {
      Category {
        activity: 0.0,
        assigned: 0.0,
        avg_assigned: 0.0,
        carry: 0.0,
        id,
        last_assigned: 0.0,
        name: format!("Cat {id}"),
        note: None,
        spent_last: 0.0,
        target: Target::default(),
        tone: None,
      }
    }
  }

  mod target_kind {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_every_kind_through_its_storage_key() {
      for kind in TargetKind::all() {
        assert_eq!(TargetKind::from_storage(kind.to_storage()), kind);
      }
    }

    #[test]
    fn it_falls_back_to_monthly_for_an_unknown_key() {
      assert_eq!(TargetKind::from_storage("nonsense"), TargetKind::Monthly);
    }
  }

  mod shift_month {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_steps_back_across_a_year_boundary() {
      assert_eq!(shift_month("2026-01", -1), "2025-12");
    }

    #[test]
    fn it_steps_forward_across_a_year_boundary() {
      assert_eq!(shift_month("2026-12", 1), "2027-01");
    }

    #[test]
    fn it_steps_within_a_year() {
      assert_eq!(shift_month("2026-06", -1), "2026-05");
      assert_eq!(shift_month("2026-06", 1), "2026-07");
    }

    #[test]
    fn it_returns_an_unparseable_key_unchanged() {
      assert_eq!(shift_month("not-a-month", 1), "not-a-month");
    }
  }

  mod month_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_a_human_month_and_year() {
      assert_eq!(month_label("2026-06"), "June 2026");
      assert_eq!(month_label("2025-12"), "December 2025");
    }

    #[test]
    fn it_returns_an_unparseable_key_verbatim() {
      assert_eq!(month_label("nope"), "nope");
    }
  }

  mod month_relative_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_says_this_month_only_for_the_current_month() {
      let current = current_month();

      assert_eq!(month_relative_label(&current), "This month");
      assert_ne!(month_relative_label(&shift_month(&current, -1)), "This month");
      assert_ne!(month_relative_label(&shift_month(&current, 1)), "This month");
    }

    #[test]
    fn it_describes_adjacent_and_distant_months_relatively() {
      let current = current_month();

      assert_eq!(month_relative_label(&shift_month(&current, -1)), "Last month");
      assert_eq!(month_relative_label(&shift_month(&current, 1)), "Next month");
      assert_eq!(month_relative_label(&shift_month(&current, -3)), "3 months ago");
      assert_eq!(month_relative_label(&shift_month(&current, 4)), "In 4 months");
    }
  }

  mod target_status {
    use pretty_assertions::assert_eq;

    use super::*;

    const MONTH: &str = "2026-06";

    fn dated(amount: f64, by_date: &str) -> Target {
      Target {
        amount,
        by_date: Some(by_date.to_owned()),
        kind: TargetKind::GoalBy,
      }
    }

    fn target(kind: TargetKind, amount: f64) -> Target {
      Target {
        amount,
        by_date: None,
        kind,
      }
    }

    #[test]
    fn it_meets_a_monthly_target_on_assignment() {
      let status = target_status(&target(TargetKind::Monthly, 100.0), 100.0, 100.0, MONTH);

      assert_eq!(status.state, TargetState::Met);
      assert_eq!(status.needed, 0.0);
      assert_eq!(status.pct, 1.0);
    }

    #[test]
    fn it_reports_a_monthly_shortfall_as_under() {
      let status = target_status(&target(TargetKind::Monthly, 100.0), 40.0, 40.0, MONTH);

      assert_eq!(status.state, TargetState::Under);
      assert_eq!(status.needed, 60.0);
    }

    #[test]
    fn it_measures_a_balance_target_against_available_not_assigned() {
      // Balance targets track available; assigned alone does not satisfy them.
      let status = target_status(&target(TargetKind::Balance, 1_000.0), 100.0, 800.0, MONTH);

      assert_eq!(status.needed, 200.0);
      assert_eq!(status.state, TargetState::Under);
    }

    #[test]
    fn it_reports_a_negative_available_as_over() {
      let status = target_status(&target(TargetKind::Refill, 250.0), 250.0, -50.0, MONTH);

      assert_eq!(status.state, TargetState::Over);
    }

    #[test]
    fn it_demands_the_whole_remainder_for_an_open_ended_goal() {
      // A dateless goal is unchanged: the full shortfall is needed every month.
      let status = target_status(&target(TargetKind::Goal, 1_200.0), 0.0, 200.0, MONTH);

      assert_eq!(status.needed, 1_000.0);
    }

    #[test]
    fn it_paces_a_dated_goal_across_the_months_until_due() {
      // 1000 remaining over Jun..Dec inclusive (7 months) → ~142.86 per month.
      let status = target_status(&dated(1_200.0, "Dec 2026"), 0.0, 200.0, MONTH);

      assert_eq!(status.needed, 1_000.0 / 7.0);
    }

    #[test]
    fn it_shrinks_the_dated_slice_as_the_goal_funds() {
      // Same horizon, but 700 already available → only 500 left over 7 months.
      let status = target_status(&dated(1_200.0, "Dec 2026"), 0.0, 700.0, MONTH);

      assert_eq!(status.needed, 500.0 / 7.0);
    }

    #[test]
    fn it_demands_the_full_remainder_in_the_final_month() {
      // The deadline month itself is the last slice: pace divides by 1.
      let status = target_status(&dated(1_200.0, "Jun 2026"), 0.0, 200.0, MONTH);

      assert_eq!(status.needed, 1_000.0);
    }

    #[test]
    fn it_demands_the_full_remainder_when_past_due() {
      // A deadline already behind us clamps months_remaining to 1.
      let status = target_status(&dated(1_200.0, "Jan 2026"), 0.0, 200.0, MONTH);

      assert_eq!(status.needed, 1_000.0);
    }

    #[test]
    fn it_falls_back_to_the_full_remainder_for_an_unparseable_date() {
      let status = target_status(&dated(1_200.0, "someday"), 0.0, 200.0, MONTH);

      assert_eq!(status.needed, 1_000.0);
    }

    #[test]
    fn it_paces_an_iso_dated_goal() {
      // ISO YYYY-MM-DD is accepted alongside the editor's "Mon YYYY" form.
      let status = target_status(&dated(1_200.0, "2026-12-01"), 0.0, 200.0, MONTH);

      assert_eq!(status.needed, 1_000.0 / 7.0);
    }
  }

  mod underfunded_assign {
    use pretty_assertions::assert_eq;

    use super::*;

    const MONTH: &str = "2026-06";

    fn category(kind: TargetKind, amount: f64, assigned: f64, available: f64) -> Category {
      dated_category(kind, amount, assigned, available, None)
    }

    fn dated_category(kind: TargetKind, amount: f64, assigned: f64, available: f64, by_date: Option<&str>) -> Category {
      Category {
        activity: available - assigned, // carry 0: available = assigned + activity
        assigned,
        avg_assigned: 0.0,
        carry: 0.0,
        id: 1,
        last_assigned: 0.0,
        name: "Test".to_owned(),
        note: None,
        spent_last: 0.0,
        target: Target {
          amount,
          by_date: by_date.map(str::to_owned),
          kind,
        },
        tone: None,
      }
    }

    #[test]
    fn it_raises_a_monthly_assignment_to_the_amount() {
      let category = category(TargetKind::Monthly, 100.0, 40.0, 40.0);

      assert_eq!(category.underfunded_assign(MONTH), 100.0);
    }

    #[test]
    fn it_tops_a_balance_available_up_to_the_amount() {
      // available 800, target 1000 → top up by 200 from the current assignment.
      let category = category(TargetKind::Balance, 1_000.0, 100.0, 800.0);

      assert_eq!(category.underfunded_assign(MONTH), 300.0);
    }

    #[test]
    fn it_tops_a_dated_goal_by_only_the_paced_slice() {
      // 1000 remaining over Jun..Dec (7 months); current assignment 50 → +1000/7.
      let category = dated_category(TargetKind::GoalBy, 1_200.0, 50.0, 200.0, Some("Dec 2026"));

      assert_eq!(category.underfunded_assign(MONTH), 50.0 + 1_000.0 / 7.0);
    }
  }

  mod load {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
      repo::{character::insert_with_org, finance, infra},
    };

    async fn seed_pilot(db: &Database, id: i64, liquid: f64) {
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
          // An unmapped ref_type: it sets the wallet balance (the pool) without
          // routing any activity into a seeded envelope.
          ref_type: "unmapped_seed_ref".to_owned(),
          second_party_id: None,
          tax: None,
          tax_receiver_id: None,
        }],
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_seeds_and_derives_ready_to_assign_from_the_pool() {
      let db = store::open_test().await.unwrap();
      seed_pilot(&db, 1, 10_000.0).await;

      let view = load(&db, BudgetScope::Character(1), "2026-06").await;

      // Fresh scope: nothing assigned yet, so the whole pool is ready to assign.
      assert!(!view.groups.is_empty());
      assert_eq!(view.pool, 10_000.0);
      assert_eq!(view.ready_to_assign, 10_000.0);
    }

    #[tokio::test]
    async fn it_reflects_a_persisted_assignment_in_ready_to_assign() {
      let db = store::open_test().await.unwrap();
      seed_pilot(&db, 1, 10_000.0).await;
      let view = load(&db, BudgetScope::Character(1), "2026-06").await;
      let category_id = view.first_category_id().unwrap();

      persist_assignment(&db, category_id, "2026-06", 2_500.0).await;
      let after = load(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(after.category(category_id).unwrap().assigned, 2_500.0);
      assert_eq!(after.ready_to_assign, 7_500.0);
    }

    #[tokio::test]
    async fn it_draws_down_ready_to_assign_for_a_future_month_assignment() {
      let db = store::open_test().await.unwrap();
      seed_pilot(&db, 1, 10_000.0).await;
      let view = load(&db, BudgetScope::Character(1), "2026-06").await;
      let category_id = view.first_category_id().unwrap();

      // Assign the same ISK in the current month and a future one. Global RTA is
      // a single pool, so both draws count and the displayed month sees them.
      persist_assignment(&db, category_id, "2026-06", 8_000.0).await;
      persist_assignment(&db, category_id, "2026-08", 8_000.0).await;
      let after = load(&db, BudgetScope::Character(1), "2026-06").await;

      // 10_000 pool − 16_000 assigned across months = −6_000: the same ISK
      // cannot be assigned twice without RTA going negative.
      assert_eq!(after.ready_to_assign, -6_000.0);
    }
  }

  mod carry_into {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    fn assignment(month: &str, assigned: f64) -> crate::store::model::BudgetAssignment {
      crate::store::model::BudgetAssignment {
        assigned,
        category_id: 1,
        id: 0,
        month: month.to_owned(),
      }
    }

    fn activity_map(entries: &[(&str, f64)]) -> HashMap<String, HashMap<i64, f64>> {
      entries
        .iter()
        .map(|(month, activity)| ((*month).to_owned(), HashMap::from([(1, *activity)])))
        .collect()
    }

    #[test]
    fn it_applies_real_activity_for_a_non_adjacent_prior_month() {
      // Spend lands in N-2 (April), not the adjacent month. The carry into June
      // must reflect that real April spend, not treat it as zero.
      // April: 0 + 100 assigned − 70 spend = 30 available → carries 30.
      // May:   30 + 0 assigned + 0 = 30 available → carries 30.
      let assignments = [assignment("2026-04", 100.0)];
      let activity = activity_map(&[("2026-04", -70.0)]);

      let carry = carry_into("2026-06", &assignments, &activity, 1);

      assert_eq!(carry, 30.0);
    }

    #[test]
    fn it_includes_a_month_with_activity_but_no_assignment() {
      // May has spend but no assignment row; it still belongs in the chain.
      // April: 0 + 100 − 0 = 100 → carries 100.
      // May:   100 + 0 − 40 = 60 → carries 60.
      let assignments = [assignment("2026-04", 100.0)];
      let activity = activity_map(&[("2026-05", -40.0)]);

      let carry = carry_into("2026-06", &assignments, &activity, 1);

      assert_eq!(carry, 60.0);
    }

    #[test]
    fn it_resets_carry_to_zero_on_an_overspent_prior_month() {
      // April overspends: 0 + 50 − 200 = −150 available → carries 0, and the
      // 150 loss is absorbed by the pool (RTA), never by next month's carry.
      let assignments = [assignment("2026-04", 50.0)];
      let activity = activity_map(&[("2026-04", -200.0)]);

      let carry = carry_into("2026-06", &assignments, &activity, 1);

      assert_eq!(carry, 0.0);
    }

    #[test]
    fn it_carries_zero_with_no_prior_months() {
      let carry = carry_into("2026-06", &[], &HashMap::new(), 1);

      assert_eq!(carry, 0.0);
    }
  }

  mod auto_assign {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      repo::budget::{NewCategory, NewGroup, TargetInput, create_category, create_group, set_target},
    };

    async fn category_with_target(db: &Database, group_id: i64, name: &str, amount: f64) -> i64 {
      let cat = create_category(
        db,
        &NewCategory {
          group_id,
          name: name.to_owned(),
          note: None,
          position: 0,
          tone: None,
        },
      )
      .await
      .unwrap();
      set_target(
        db,
        cat.id(),
        &TargetInput {
          amount,
          by_date: None,
          kind: "monthly".to_owned(),
        },
      )
      .await
      .unwrap();
      cat.id()
    }

    #[tokio::test]
    async fn it_fills_underfunded_categories_until_the_pool_runs_out() {
      let db = store::open_test().await.unwrap();
      let group = create_group(
        &db,
        &NewGroup {
          name: "Bills".to_owned(),
          position: 0,
          scope: BudgetScope::Character(1),
        },
      )
      .await
      .unwrap();
      let first = category_with_target(&db, group.id(), "First", 100.0).await;
      let second = category_with_target(&db, group.id(), "Second", 100.0).await;

      // A pool of 150 with two 100-monthly targets: fill the first fully, the
      // second partially. We feed a hand-built view so the pool is deterministic.
      let view = BudgetView {
        groups: vec![Group {
          categories: vec![category(first, 100.0), category(second, 100.0)],
          id: group.id(),
          name: "Bills".to_owned(),
        }],
        month: "2026-06".to_owned(),
        overspent: 0.0,
        pool: 150.0,
        ready_to_assign: 150.0,
      };

      auto_assign(&db, &view).await;

      let reloaded = load(&db, BudgetScope::Character(1), "2026-06").await;
      assert_eq!(reloaded.category(first).unwrap().assigned, 100.0);
      assert_eq!(reloaded.category(second).unwrap().assigned, 50.0);
    }

    fn category(id: i64, amount: f64) -> Category {
      Category {
        activity: 0.0,
        assigned: 0.0,
        avg_assigned: 0.0,
        carry: 0.0,
        id,
        last_assigned: 0.0,
        name: "Cat".to_owned(),
        note: None,
        spent_last: 0.0,
        target: Target {
          amount,
          by_date: None,
          kind: TargetKind::Monthly,
        },
        tone: None,
      }
    }
  }

  mod persist_assignment {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      repo::budget::{NewCategory, NewGroup, create_category, create_group},
    };

    #[tokio::test]
    async fn it_stores_a_negative_assignment_below_zero() {
      let db = store::open_test().await.unwrap();
      let group = create_group(
        &db,
        &NewGroup {
          name: "Bills".to_owned(),
          position: 0,
          scope: BudgetScope::Character(1),
        },
      )
      .await
      .unwrap();
      let cat = create_category(
        &db,
        &NewCategory {
          group_id: group.id(),
          name: "First".to_owned(),
          note: None,
          position: 0,
          tone: None,
        },
      )
      .await
      .unwrap();

      persist_assignment(&db, cat.id(), "2026-06", -750.4).await;
      let view = load(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(view.category(cat.id()).unwrap().assigned, -750.0);
    }
  }

  mod move_money {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
      repo::{
        budget::{NewCategory, NewGroup, create_category, create_group},
        character::insert_with_org,
        finance, infra,
      },
    };

    async fn seed_pilot(db: &Database, liquid: f64) {
      let id = 1;
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
          ref_type: "unmapped_seed_ref".to_owned(),
          second_party_id: None,
          tax: None,
          tax_receiver_id: None,
        }],
      )
      .await
      .unwrap();
    }

    async fn two_categories(db: &Database) -> (i64, i64) {
      let group = create_group(
        db,
        &NewGroup {
          name: "Bills".to_owned(),
          position: 0,
          scope: BudgetScope::Character(1),
        },
      )
      .await
      .unwrap();
      let mut ids = Vec::new();
      for name in ["First", "Second"] {
        let cat = create_category(
          db,
          &NewCategory {
            group_id: group.id(),
            name: name.to_owned(),
            note: None,
            position: 0,
            tone: None,
          },
        )
        .await
        .unwrap();
        ids.push(cat.id());
      }
      (ids[0], ids[1])
    }

    fn conservation(view: &BudgetView) -> f64 {
      view.ready_to_assign
        + view
          .groups
          .iter()
          .flat_map(|g| &g.categories)
          .map(Category::available)
          .sum::<f64>()
    }

    #[tokio::test]
    async fn it_transfers_assigned_between_two_categories() {
      let db = store::open_test().await.unwrap();
      seed_pilot(&db, 10_000.0).await;
      let (first, second) = two_categories(&db).await;
      persist_assignment(&db, first, "2026-06", 3_000.0).await;
      let view = load(&db, BudgetScope::Character(1), "2026-06").await;

      move_money(&db, &view, first, MoveDest::Category(second), 1_200.0).await;
      let after = load(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(after.category(first).unwrap().available(), 1_800.0);
      assert_eq!(after.category(second).unwrap().available(), 1_200.0);
      assert_eq!(after.ready_to_assign, 7_000.0);
    }

    #[tokio::test]
    async fn it_returns_money_to_ready_to_assign() {
      let db = store::open_test().await.unwrap();
      seed_pilot(&db, 10_000.0).await;
      let (first, _second) = two_categories(&db).await;
      persist_assignment(&db, first, "2026-06", 3_000.0).await;
      let view = load(&db, BudgetScope::Character(1), "2026-06").await;

      move_money(&db, &view, first, MoveDest::ReadyToAssign, 1_200.0).await;
      let after = load(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(after.category(first).unwrap().available(), 1_800.0);
      assert_eq!(after.ready_to_assign, 8_200.0);
    }

    #[tokio::test]
    async fn it_drives_the_source_assigned_negative_on_a_full_carry_move() {
      let db = store::open_test().await.unwrap();
      seed_pilot(&db, 10_000.0).await;
      let (first, second) = two_categories(&db).await;
      persist_assignment(&db, first, "2026-05", 2_000.0).await;
      let view = load(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(view.category(first).unwrap().assigned, 0.0);
      assert_eq!(view.category(first).unwrap().available(), 2_000.0);

      move_money(&db, &view, first, MoveDest::Category(second), 2_000.0).await;
      let after = load(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(after.category(first).unwrap().assigned, -2_000.0);
      assert_eq!(after.category(first).unwrap().available(), 0.0);
      assert_eq!(after.category(second).unwrap().available(), 2_000.0);
      assert_eq!(after.ready_to_assign, view.ready_to_assign);
    }

    #[tokio::test]
    async fn it_raises_ready_to_assign_when_a_carry_move_targets_the_pool() {
      let db = store::open_test().await.unwrap();
      seed_pilot(&db, 10_000.0).await;
      let (first, _second) = two_categories(&db).await;
      persist_assignment(&db, first, "2026-05", 2_000.0).await;
      let view = load(&db, BudgetScope::Character(1), "2026-06").await;

      move_money(&db, &view, first, MoveDest::ReadyToAssign, 2_000.0).await;
      let after = load(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(after.category(first).unwrap().assigned, -2_000.0);
      assert_eq!(after.ready_to_assign, view.ready_to_assign + 2_000.0);
    }

    #[tokio::test]
    async fn it_conserves_total_funds_across_a_transfer() {
      let db = store::open_test().await.unwrap();
      seed_pilot(&db, 10_000.0).await;
      let (first, second) = two_categories(&db).await;
      persist_assignment(&db, first, "2026-06", 4_000.0).await;
      let before = load(&db, BudgetScope::Character(1), "2026-06").await;
      let funds = conservation(&before);

      move_money(&db, &before, first, MoveDest::Category(second), 4_000.0).await;
      let cat_to_cat = load(&db, BudgetScope::Character(1), "2026-06").await;

      move_money(&db, &cat_to_cat, second, MoveDest::ReadyToAssign, 2_500.0).await;
      let to_pool = load(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(conservation(&cat_to_cat), funds);
      assert_eq!(conservation(&to_pool), funds);
    }

    #[tokio::test]
    async fn it_ignores_a_non_positive_amount() {
      let db = store::open_test().await.unwrap();
      seed_pilot(&db, 10_000.0).await;
      let (first, second) = two_categories(&db).await;
      persist_assignment(&db, first, "2026-06", 3_000.0).await;
      let view = load(&db, BudgetScope::Character(1), "2026-06").await;

      move_money(&db, &view, first, MoveDest::Category(second), 0.0).await;
      let after = load(&db, BudgetScope::Character(1), "2026-06").await;

      assert_eq!(after.category(first).unwrap().assigned, 3_000.0);
      assert_eq!(after.category(second).unwrap().assigned, 0.0);
    }
  }

  mod month_short_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_the_short_month_name() {
      assert_eq!(month_short_label("2026-06"), "Jun");
      assert_eq!(month_short_label("2026-01"), "Jan");
    }

    #[test]
    fn it_returns_an_unparseable_key_verbatim() {
      assert_eq!(month_short_label("nope"), "nope");
    }
  }

  mod budget_range {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_each_range_to_its_month_span() {
      assert_eq!(BudgetRange::SixMonths.months(), 6);
      assert_eq!(BudgetRange::ThreeMonths.months(), 3);
      assert_eq!(BudgetRange::default(), BudgetRange::SixMonths);
    }
  }

  mod reflect {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::budget::MonthFlow;

    fn cat(id: i64, name: &str, assigned: f64, activity: f64, kind: TargetKind, amount: f64) -> Category {
      Category {
        activity,
        assigned,
        avg_assigned: 0.0,
        carry: 0.0,
        id,
        last_assigned: 0.0,
        name: name.to_owned(),
        note: None,
        spent_last: 0.0,
        target: Target {
          amount,
          by_date: None,
          kind,
        },
        tone: Some("plasma".to_owned()),
      }
    }

    fn view_with(categories: Vec<Category>) -> BudgetView {
      BudgetView {
        groups: vec![Group {
          categories,
          id: 1,
          name: "Group".to_owned(),
        }],
        month: "2026-06".to_owned(),
        overspent: 0.0,
        pool: 0.0,
        ready_to_assign: 0.0,
      }
    }

    fn month(key: &str, age: f64) -> MonthFlow {
      MonthFlow {
        age,
        assigned: 0.0,
        income: 0.0,
        month: key.to_owned(),
        spend: 0.0,
      }
    }

    #[test]
    fn it_splits_income_spend_and_net_from_activity() {
      let view = view_with(vec![
        cat(1, "Bounties", 0.0, 1_000.0, TargetKind::Monthly, 0.0),
        cat(2, "Fees", 100.0, -400.0, TargetKind::Monthly, 100.0),
      ]);

      let reflect = reflect(&view, Vec::new());

      assert_eq!(reflect.income, 1_000.0);
      assert_eq!(reflect.spend, 400.0);
      assert_eq!(reflect.net(), 600.0);
      assert_eq!(reflect.assigned, 100.0);
    }

    #[test]
    fn it_sorts_spend_rows_descending_and_skips_income() {
      let view = view_with(vec![
        cat(1, "Small", 0.0, -50.0, TargetKind::Monthly, 0.0),
        cat(2, "Big", 0.0, -900.0, TargetKind::Monthly, 0.0),
        cat(3, "Income", 0.0, 200.0, TargetKind::Monthly, 0.0),
      ]);

      let reflect = reflect(&view, Vec::new());

      assert_eq!(reflect.spend_rows.len(), 2);
      assert_eq!(reflect.spend_rows[0].name, "Big");
      assert_eq!(reflect.spend_rows[1].name, "Small");
    }

    #[test]
    fn it_tallies_target_health_and_flags_attention() {
      let view = view_with(vec![
        // Met: monthly target fully assigned.
        cat(1, "Met", 100.0, 0.0, TargetKind::Monthly, 100.0),
        // Under: monthly target underfunded.
        cat(2, "Under", 40.0, 0.0, TargetKind::Monthly, 100.0),
        // Over: negative available.
        cat(3, "Over", 0.0, -50.0, TargetKind::Monthly, 100.0),
      ]);

      let reflect = reflect(&view, Vec::new());

      assert_eq!(reflect.tally.met, 1);
      assert_eq!(reflect.tally.under, 1);
      assert_eq!(reflect.tally.over, 1);
      assert_eq!(reflect.tally.attention.len(), 2);
    }

    #[test]
    fn it_takes_age_and_delta_from_the_history_tail() {
      let view = view_with(Vec::new());

      let reflect = reflect(&view, vec![month("2026-05", 45.0), month("2026-06", 47.0)]);

      assert_eq!(reflect.age, 47.0);
      assert_eq!(reflect.age_delta, 2.0);
      assert_eq!(reflect.prev_label, "May");
    }

    #[test]
    fn it_handles_an_empty_history() {
      let view = view_with(Vec::new());

      let reflect = reflect(&view, Vec::new());

      assert_eq!(reflect.age, 0.0);
      assert_eq!(reflect.age_delta, 0.0);
      assert_eq!(reflect.prev_label, "");
    }
  }
}
