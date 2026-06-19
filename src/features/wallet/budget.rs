use chrono::{Datelike, Utc};

use crate::{
  features::budget as math,
  store::{
    Database,
    model::BudgetScope,
    repo::budget::{self, TargetInput},
  },
};

const AVERAGE_WINDOW: usize = 3;

const TONE_INFO: iced::Color = crate::ui::style::color::chart::VIOLET;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
  #[default]
  Plan,
  Reflect,
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

  pub fn status(&self) -> TargetStatus {
    target_status(&self.target, self.assigned, self.available())
  }

  /// The "Underfunded" quick-assign suggestion: the assignment that satisfies
  /// this month's target. Monthly targets raise the assignment to the amount;
  /// the cumulative targets top the available balance up to the amount.
  pub fn underfunded_assign(&self) -> f64 {
    match self.target.kind {
      TargetKind::Monthly => self.assigned.max(self.target.amount),
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

fn parse_month(month: &str) -> Option<(i32, i32)> {
  let (year, mon) = month.split_once('-')?;
  let year = year.parse::<i32>().ok()?;
  let mon = mon.parse::<i32>().ok()?;
  (1..=12).contains(&mon).then_some((year, mon))
}

/// The target status for an `assigned`/`available` pair, ported verbatim from
/// `targetStatus` in `budget-data.jsx`.
pub fn target_status(target: &Target, assigned: f64, available: f64) -> TargetStatus {
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
        (amount - available).max(0.0),
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
  if budget::list_groups(db, scope).await.unwrap_or_default().is_empty() {
    let _ = math::seed_scope(db, scope).await;
  }

  let activity = math::monthly_activity(db, scope, month).await;
  let pool = math::budgetable_pool(db, scope).await;
  let prev_month = shift_month(month, -1);
  let prev_activity = math::monthly_activity(db, scope, &prev_month).await;

  let mut groups: Vec<Group> = Vec::new();
  let mut availables: Vec<f64> = Vec::new();
  for group_row in budget::list_groups(db, scope).await.unwrap_or_default() {
    let mut categories: Vec<Category> = Vec::new();
    for category_row in budget::list_categories(db, group_row.id()).await.unwrap_or_default() {
      let category = build_category(db, &category_row, month, &activity, &prev_month, &prev_activity).await;
      availables.push(category.available());
      categories.push(category);
    }
    groups.push(Group {
      categories,
      id: group_row.id(),
      name: group_row.name().clone(),
    });
  }

  let summary = math::pool_summary(pool, availables);
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
) -> Category {
  let assignments = budget::list_assignments(db, row.id()).await.unwrap_or_default();
  let assigned_for = |key: &str| {
    assignments
      .iter()
      .find(|a| a.month() == key)
      .map_or(0.0, |a| a.assigned())
  };

  let carry = carry_into(month, &assignments, prev_activity, prev_month, row.id());
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

/// The carry rolled into `month` for a category: every assigned month strictly
/// before `month`, in order, rolled forward through the B2 carry-over math. Only
/// the prior month's activity is known here, so earlier months carry their
/// assignment with zero activity — exact for the common case where the user is
/// budgeting the current or next month from a populated prior month.
fn carry_into(
  month: &str,
  assignments: &[crate::store::model::BudgetAssignment],
  prev_activity: &std::collections::HashMap<i64, f64>,
  prev_month: &str,
  category_id: i64,
) -> f64 {
  let months: Vec<(f64, f64)> = assignments
    .iter()
    .filter(|a| a.month().as_str() < month)
    .map(|a| {
      let act = if a.month() == prev_month {
        prev_activity.get(&category_id).copied().unwrap_or(0.0)
      } else {
        0.0
      };
      (a.assigned(), act)
    })
    .collect();
  if months.is_empty() {
    return 0.0;
  }
  // Already in ascending month order because list_assignments orders by month.
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

/// Persists a single category's assignment for `month`, clamping to a
/// non-negative whole number (the design rounds and floors assignments at 0).
pub async fn persist_assignment(db: &Database, category_id: i64, month: &str, value: f64) {
  let clamped = value.round().max(0.0);
  let _ = budget::upsert_assignment(db, category_id, month, clamped).await;
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
      let status = category.status();
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

  mod target_status {
    use pretty_assertions::assert_eq;

    use super::*;

    fn target(kind: TargetKind, amount: f64) -> Target {
      Target {
        amount,
        by_date: None,
        kind,
      }
    }

    #[test]
    fn it_meets_a_monthly_target_on_assignment() {
      let status = target_status(&target(TargetKind::Monthly, 100.0), 100.0, 100.0);

      assert_eq!(status.state, TargetState::Met);
      assert_eq!(status.needed, 0.0);
      assert_eq!(status.pct, 1.0);
    }

    #[test]
    fn it_reports_a_monthly_shortfall_as_under() {
      let status = target_status(&target(TargetKind::Monthly, 100.0), 40.0, 40.0);

      assert_eq!(status.state, TargetState::Under);
      assert_eq!(status.needed, 60.0);
    }

    #[test]
    fn it_measures_a_balance_target_against_available_not_assigned() {
      // Balance targets track available; assigned alone does not satisfy them.
      let status = target_status(&target(TargetKind::Balance, 1_000.0), 100.0, 800.0);

      assert_eq!(status.needed, 200.0);
      assert_eq!(status.state, TargetState::Under);
    }

    #[test]
    fn it_reports_a_negative_available_as_over() {
      let status = target_status(&target(TargetKind::Refill, 250.0), 250.0, -50.0);

      assert_eq!(status.state, TargetState::Over);
    }
  }

  mod underfunded_assign {
    use pretty_assertions::assert_eq;

    use super::*;

    fn category(kind: TargetKind, amount: f64, assigned: f64, available: f64) -> Category {
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
          by_date: None,
          kind,
        },
        tone: None,
      }
    }

    #[test]
    fn it_raises_a_monthly_assignment_to_the_amount() {
      let category = category(TargetKind::Monthly, 100.0, 40.0, 40.0);

      assert_eq!(category.underfunded_assign(), 100.0);
    }

    #[test]
    fn it_tops_a_balance_available_up_to_the_amount() {
      // available 800, target 1000 → top up by 200 from the current assignment.
      let category = category(TargetKind::Balance, 1_000.0, 100.0, 800.0);

      assert_eq!(category.underfunded_assign(), 300.0);
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
}
