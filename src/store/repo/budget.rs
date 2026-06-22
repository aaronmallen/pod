use sqlx::FromRow;

use crate::store::{
  Database, Error,
  model::{
    BudgetAssignment, BudgetCategory, BudgetCategoryGroup, BudgetEntryAssignment, BudgetEntryKind, BudgetOwner,
    BudgetRefTypeMap, BudgetScope, BudgetTarget, MatchMode, Rule, RuleCondition, RuleField, RuleOp,
  },
};

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct NewCategory {
  pub group_id: i64,
  pub name: String,
  pub note: Option<String>,
  pub position: i64,
  pub tone: Option<String>,
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct NewGroup {
  pub name: String,
  pub position: i64,
  pub scope: BudgetScope,
}

// Budget automation rule storage (child A); consumed by the matching engine in child B and the
// inspector UI in child C. Exercised only by unit tests until then.
#[derive(Clone, Debug, PartialEq)]
pub struct NewRule {
  pub category_id: i64,
  pub enabled: bool,
  pub match_mode: MatchMode,
  pub name: String,
  pub position: i64,
  pub scope: BudgetScope,
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct TargetInput {
  pub amount: f64,
  pub by_date: Option<String>,
  pub kind: String,
}

#[derive(Clone, Debug, FromRow, PartialEq)]
struct RuleConditionRow {
  field: String,
  op: String,
  rule_id: i64,
  value: String,
  value2: Option<String>,
}

#[derive(Clone, Debug, FromRow, PartialEq)]
struct RuleRow {
  category_id: i64,
  enabled: i64,
  id: i64,
  match_mode: String,
  name: String,
}

impl RuleConditionRow {
  fn into_condition(self) -> RuleCondition {
    RuleCondition {
      field: RuleField::from_key(&self.field),
      op: RuleOp::from_key(&self.op),
      value: self.value,
      value2: self.value2,
    }
  }
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn create_category(db: &Database, category: &NewCategory) -> Result<BudgetCategory, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, BudgetCategory>(
    "INSERT INTO budget_categories (group_id, name, note, tone, position, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?) \
    RETURNING id, group_id, name, note, tone, position, created_at, updated_at",
  )
  .bind(category.group_id)
  .bind(&category.name)
  .bind(&category.note)
  .bind(&category.tone)
  .bind(category.position)
  .bind(&now)
  .bind(&now)
  .fetch_one(&db.0)
  .await?;
  Ok(row)
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn create_group(db: &Database, group: &NewGroup) -> Result<BudgetCategoryGroup, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, BudgetCategoryGroup>(
    "INSERT INTO budget_category_groups (scope_kind, scope_id, name, position, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?) \
    RETURNING id, scope_kind, scope_id, name, position, created_at, updated_at",
  )
  .bind(group.scope.scope_kind())
  .bind(group.scope.scope_id())
  .bind(&group.name)
  .bind(group.position)
  .bind(&now)
  .bind(&now)
  .fetch_one(&db.0)
  .await?;
  Ok(row)
}

// Budget automation rule storage (child A); consumed by the matching engine in child B and the
// inspector UI in child C. Exercised only by unit tests until then.
pub async fn create_rule(db: &Database, rule: &NewRule) -> Result<Rule, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, RuleRow>(
    "INSERT INTO budget_rules \
    (scope_kind, scope_id, category_id, name, enabled, match_mode, position, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
    RETURNING id, category_id, name, enabled, match_mode",
  )
  .bind(rule.scope.scope_kind())
  .bind(rule.scope.scope_id())
  .bind(rule.category_id)
  .bind(&rule.name)
  .bind(i64::from(rule.enabled))
  .bind(rule.match_mode.as_str())
  .bind(rule.position)
  .bind(&now)
  .bind(&now)
  .fetch_one(&db.0)
  .await?;
  Ok(Rule {
    category_id: row.category_id,
    conditions: Vec::new(),
    enabled: row.enabled != 0,
    id: row.id,
    match_mode: MatchMode::from_key(&row.match_mode),
    name: row.name,
  })
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn delete_category(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM budget_categories WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

// Per-entry budget assignment storage (child A); consumed by the Budget derivation/UI in children B/C.
// Exercised by unit tests until then.
#[allow(dead_code)]
pub async fn delete_entry_assignment(
  db: &Database,
  scope: BudgetScope,
  owner: BudgetOwner,
  entry_kind: BudgetEntryKind,
  entry_id: i64,
) -> Result<(), Error> {
  sqlx::query(
    "DELETE FROM budget_entry_assignments \
    WHERE scope_kind = ? AND scope_id IS ? AND owner_kind = ? AND owner_id = ? AND entry_kind = ? AND entry_id = ?",
  )
  .bind(scope.scope_kind())
  .bind(scope.scope_id())
  .bind(owner.owner_kind())
  .bind(owner.owner_id())
  .bind(entry_kind.as_str())
  .bind(entry_id)
  .execute(db.writer())
  .await?;
  Ok(())
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn delete_group(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM budget_category_groups WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

// Budget automation rule storage (child A); deleting a rule cascades its conditions via the FK.
// Exercised only by unit tests until child B/C wire it.
pub async fn delete_rule(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM budget_rules WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn delete_target(db: &Database, category_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM budget_targets WHERE category_id = ?")
    .bind(category_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn list_assignments(db: &Database, category_id: i64) -> Result<Vec<BudgetAssignment>, Error> {
  let rows = sqlx::query_as::<_, BudgetAssignment>(
    "SELECT id, category_id, month, assigned FROM budget_assignments WHERE category_id = ? ORDER BY month",
  )
  .bind(category_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn list_categories(db: &Database, group_id: i64) -> Result<Vec<BudgetCategory>, Error> {
  let rows = sqlx::query_as::<_, BudgetCategory>(
    "SELECT id, group_id, name, note, tone, position, created_at, updated_at \
    FROM budget_categories WHERE group_id = ? ORDER BY position, id",
  )
  .bind(group_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Budget storage foundation; consumed by the Budget seed path. Exercised by unit tests until wired.
#[allow(dead_code)]
pub async fn is_scope_seeded(db: &Database, scope: BudgetScope) -> Result<bool, Error> {
  let row: Option<i64> = sqlx::query_scalar("SELECT 1 FROM budget_scope_seeded WHERE scope_kind = ? AND scope_id IS ?")
    .bind(scope.scope_kind())
    .bind(scope.scope_id())
    .fetch_optional(&db.0)
    .await?;
  Ok(row.is_some())
}

// Budget storage foundation; consumed by the Budget seed path. Exercised by unit tests until wired.
#[allow(dead_code)]
pub async fn mark_scope_seeded(db: &Database, scope: BudgetScope) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  sqlx::query(
    "INSERT INTO budget_scope_seeded (scope_kind, scope_id, seeded_at) VALUES (?, ?, ?) \
    ON CONFLICT(scope_kind, COALESCE(scope_id, -1)) DO NOTHING",
  )
  .bind(scope.scope_kind())
  .bind(scope.scope_id())
  .bind(&now)
  .execute(db.writer())
  .await?;
  Ok(())
}

// Per-entry budget assignment storage (child A); consumed by the Budget derivation/UI in children B/C.
// Exercised by unit tests until then.
#[allow(dead_code)]
pub async fn list_entry_assignments(db: &Database, scope: BudgetScope) -> Result<Vec<BudgetEntryAssignment>, Error> {
  let rows = sqlx::query_as::<_, BudgetEntryAssignment>(
    "SELECT id, scope_kind, scope_id, owner_kind, owner_id, entry_kind, entry_id, category_id, created_at, updated_at \
    FROM budget_entry_assignments \
    WHERE scope_kind = ? AND scope_id IS ? ORDER BY entry_kind, entry_id",
  )
  .bind(scope.scope_kind())
  .bind(scope.scope_id())
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn list_groups(db: &Database, scope: BudgetScope) -> Result<Vec<BudgetCategoryGroup>, Error> {
  let rows = sqlx::query_as::<_, BudgetCategoryGroup>(
    "SELECT id, scope_kind, scope_id, name, position, created_at, updated_at \
    FROM budget_category_groups \
    WHERE scope_kind = ? AND scope_id IS ? ORDER BY position, id",
  )
  .bind(scope.scope_kind())
  .bind(scope.scope_id())
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn list_ref_type_maps(db: &Database, scope: BudgetScope) -> Result<Vec<BudgetRefTypeMap>, Error> {
  let rows = sqlx::query_as::<_, BudgetRefTypeMap>(
    "SELECT id, scope_kind, scope_id, ref_type, category_id FROM budget_ref_type_maps \
    WHERE scope_kind = ? AND scope_id IS ? ORDER BY ref_type",
  )
  .bind(scope.scope_kind())
  .bind(scope.scope_id())
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Budget automation rule loader (child A): every rule for the active scope, in priority order, with
// its conditions nested in position order — the exact shape the matching engine in child B consumes.
pub async fn list_rules(db: &Database, scope: BudgetScope) -> Result<Vec<Rule>, Error> {
  let rule_rows = sqlx::query_as::<_, RuleRow>(
    "SELECT id, category_id, name, enabled, match_mode FROM budget_rules \
    WHERE scope_kind = ? AND scope_id IS ? ORDER BY position, id",
  )
  .bind(scope.scope_kind())
  .bind(scope.scope_id())
  .fetch_all(&db.0)
  .await?;

  let condition_rows = sqlx::query_as::<_, RuleConditionRow>(
    "SELECT c.rule_id, c.field, c.op, c.value, c.value2 FROM budget_rule_conditions c \
    JOIN budget_rules r ON r.id = c.rule_id \
    WHERE r.scope_kind = ? AND r.scope_id IS ? ORDER BY c.rule_id, c.position, c.id",
  )
  .bind(scope.scope_kind())
  .bind(scope.scope_id())
  .fetch_all(&db.0)
  .await?;

  let mut rules = rule_rows
    .into_iter()
    .map(|row| {
      (
        row.id,
        Rule {
          category_id: row.category_id,
          conditions: Vec::new(),
          enabled: row.enabled != 0,
          id: row.id,
          match_mode: MatchMode::from_key(&row.match_mode),
          name: row.name,
        },
      )
    })
    .collect::<Vec<_>>();

  for condition in condition_rows {
    let rule_id = condition.rule_id;
    if let Some((_, rule)) = rules.iter_mut().find(|(id, _)| *id == rule_id) {
      rule.conditions.push(condition.into_condition());
    }
  }

  Ok(rules.into_iter().map(|(_, rule)| rule).collect())
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn load_target(db: &Database, category_id: i64) -> Result<Option<BudgetTarget>, Error> {
  let row = sqlx::query_as::<_, BudgetTarget>(
    "SELECT category_id, kind, amount, by_date FROM budget_targets WHERE category_id = ?",
  )
  .bind(category_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

// Budget activity math (B2): the global Ready-to-Assign basis. Consumed by the Budget Plan UI.
// Exercised by unit tests until then.
#[allow(dead_code)]
pub async fn scope_assigned_total(db: &Database, scope: BudgetScope) -> Result<f64, Error> {
  let total: Option<f64> = sqlx::query_scalar(
    "SELECT SUM(a.assigned) FROM budget_assignments a \
    JOIN budget_categories c ON c.id = a.category_id \
    JOIN budget_category_groups g ON g.id = c.group_id \
    WHERE g.scope_kind = ? AND g.scope_id IS ?",
  )
  .bind(scope.scope_kind())
  .bind(scope.scope_id())
  .fetch_one(&db.0)
  .await?;
  Ok(total.unwrap_or(0.0))
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn set_target(db: &Database, category_id: i64, target: &TargetInput) -> Result<BudgetTarget, Error> {
  let row = sqlx::query_as::<_, BudgetTarget>(
    "INSERT INTO budget_targets (category_id, kind, amount, by_date) VALUES (?, ?, ?, ?) \
    ON CONFLICT(category_id) DO UPDATE SET kind = excluded.kind, amount = excluded.amount, by_date = excluded.by_date \
    RETURNING category_id, kind, amount, by_date",
  )
  .bind(category_id)
  .bind(&target.kind)
  .bind(target.amount)
  .bind(&target.by_date)
  .fetch_one(&db.0)
  .await?;
  Ok(row)
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn rename_group(db: &Database, id: i64, name: &str) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  sqlx::query("UPDATE budget_category_groups SET name = ?, updated_at = ? WHERE id = ?")
    .bind(name)
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

// Budget automation rule storage (child A): rewrite the priority order by persisting each rule's new
// position from its index in `ordered_ids`, in one transaction (cf. skills::reorder_entries).
#[allow(dead_code)]
pub async fn reorder_rules(db: &Database, ordered_ids: &[i64]) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let mut tx = db.writer().begin().await?;
  for (position, id) in ordered_ids.iter().enumerate() {
    sqlx::query("UPDATE budget_rules SET position = ?, updated_at = ? WHERE id = ?")
      .bind(position as i64)
      .bind(&now)
      .bind(id)
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}

// Budget automation rule storage (child A): replace a rule's full condition set in one transaction,
// re-numbering positions from slice order so the engine reads them in the builder's order.
pub async fn replace_rule_conditions(db: &Database, rule_id: i64, conditions: &[RuleCondition]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;
  sqlx::query("DELETE FROM budget_rule_conditions WHERE rule_id = ?")
    .bind(rule_id)
    .execute(&mut *tx)
    .await?;
  for (position, condition) in conditions.iter().enumerate() {
    sqlx::query(
      "INSERT INTO budget_rule_conditions (rule_id, field, op, value, value2, position) \
      VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(rule_id)
    .bind(condition.field().as_str())
    .bind(condition.op().as_str())
    .bind(condition.value())
    .bind(condition.value2())
    .bind(position as i64)
    .execute(&mut *tx)
    .await?;
  }
  tx.commit().await?;
  Ok(())
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn update_category(db: &Database, category: &BudgetCategory) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  sqlx::query(
    "UPDATE budget_categories SET group_id = ?, name = ?, note = ?, tone = ?, position = ?, updated_at = ? \
    WHERE id = ?",
  )
  .bind(category.group_id())
  .bind(category.name())
  .bind(category.note())
  .bind(category.tone())
  .bind(category.position())
  .bind(&now)
  .bind(category.id())
  .execute(db.writer())
  .await?;
  Ok(())
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn update_group(db: &Database, group: &BudgetCategoryGroup) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  sqlx::query("UPDATE budget_category_groups SET name = ?, position = ?, updated_at = ? WHERE id = ?")
    .bind(group.name())
    .bind(group.position())
    .bind(&now)
    .bind(group.id())
    .execute(db.writer())
    .await?;
  Ok(())
}

// Budget automation rule storage (child A): update a rule's editable fields by id. Position is owned
// by reorder_rules and conditions by replace_rule_conditions, so neither is touched here.
pub async fn update_rule(db: &Database, rule: &Rule) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  sqlx::query(
    "UPDATE budget_rules SET category_id = ?, name = ?, enabled = ?, match_mode = ?, updated_at = ? WHERE id = ?",
  )
  .bind(rule.category_id())
  .bind(rule.name())
  .bind(i64::from(rule.enabled()))
  .bind(rule.match_mode().as_str())
  .bind(&now)
  .bind(rule.id())
  .execute(db.writer())
  .await?;
  Ok(())
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn upsert_assignment(
  db: &Database,
  category_id: i64,
  month: &str,
  assigned: f64,
) -> Result<BudgetAssignment, Error> {
  let row = sqlx::query_as::<_, BudgetAssignment>(
    "INSERT INTO budget_assignments (category_id, month, assigned) VALUES (?, ?, ?) \
    ON CONFLICT(category_id, month) DO UPDATE SET assigned = excluded.assigned \
    RETURNING id, category_id, month, assigned",
  )
  .bind(category_id)
  .bind(month)
  .bind(assigned)
  .fetch_one(&db.0)
  .await?;
  Ok(row)
}

// Per-entry budget assignment storage (child A); consumed by the Budget derivation/UI in children B/C.
// Exercised by unit tests until then.
#[allow(dead_code)]
pub async fn owner_holds_entry(
  db: &Database,
  owner: BudgetOwner,
  entry_kind: BudgetEntryKind,
  entry_id: i64,
) -> Result<bool, Error> {
  // entry_id is the journal `id` for a Journal entry and the `transaction_id` for
  // a Market entry; each owner kind keys its wallet rows by its own id column, so
  // a corp-only id has no character row and vice-versa.
  let query = match (owner, entry_kind) {
    (BudgetOwner::Character(_), BudgetEntryKind::Journal) => {
      "SELECT EXISTS(SELECT 1 FROM character_wallet_journal WHERE character_id = ? AND id = ?)"
    }
    (BudgetOwner::Character(_), BudgetEntryKind::Market) => {
      "SELECT EXISTS(SELECT 1 FROM character_wallet_transaction WHERE character_id = ? AND transaction_id = ?)"
    }
    (BudgetOwner::Corporation(_), BudgetEntryKind::Journal) => {
      "SELECT EXISTS(SELECT 1 FROM corporation_wallet_journal WHERE corporation_id = ? AND id = ?)"
    }
    (BudgetOwner::Corporation(_), BudgetEntryKind::Market) => {
      "SELECT EXISTS(SELECT 1 FROM corporation_wallet_transaction WHERE corporation_id = ? AND transaction_id = ?)"
    }
  };
  let exists = sqlx::query_scalar::<_, i64>(query)
    .bind(owner.owner_id())
    .bind(entry_id)
    .fetch_one(&db.0)
    .await?;
  Ok(exists != 0)
}

// Per-entry budget assignment storage (child A); consumed by the Budget derivation/UI in children B/C.
// Exercised by unit tests until then.
#[allow(dead_code)]
pub async fn upsert_entry_assignment(
  db: &Database,
  scope: BudgetScope,
  owner: BudgetOwner,
  entry_kind: BudgetEntryKind,
  entry_id: i64,
  category_id: i64,
) -> Result<BudgetEntryAssignment, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, BudgetEntryAssignment>(
    "INSERT INTO budget_entry_assignments \
    (scope_kind, scope_id, owner_kind, owner_id, entry_kind, entry_id, category_id, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(scope_kind, COALESCE(scope_id, -1), owner_kind, owner_id, entry_kind, entry_id) \
    DO UPDATE SET category_id = excluded.category_id, updated_at = excluded.updated_at \
    RETURNING id, scope_kind, scope_id, owner_kind, owner_id, entry_kind, entry_id, category_id, created_at, updated_at",
  )
  .bind(scope.scope_kind())
  .bind(scope.scope_id())
  .bind(owner.owner_kind())
  .bind(owner.owner_id())
  .bind(entry_kind.as_str())
  .bind(entry_id)
  .bind(category_id)
  .bind(&now)
  .bind(&now)
  .fetch_one(&db.0)
  .await?;
  Ok(row)
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn upsert_ref_type_map(
  db: &Database,
  scope: BudgetScope,
  ref_type: &str,
  category_id: i64,
) -> Result<BudgetRefTypeMap, Error> {
  let row = sqlx::query_as::<_, BudgetRefTypeMap>(
    "INSERT INTO budget_ref_type_maps (scope_kind, scope_id, ref_type, category_id) VALUES (?, ?, ?, ?) \
    ON CONFLICT(scope_kind, COALESCE(scope_id, -1), ref_type) DO UPDATE SET category_id = excluded.category_id \
    RETURNING id, scope_kind, scope_id, ref_type, category_id",
  )
  .bind(scope.scope_kind())
  .bind(scope.scope_id())
  .bind(ref_type)
  .bind(category_id)
  .fetch_one(&db.0)
  .await?;
  Ok(row)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  async fn group(db: &Database, scope: BudgetScope, name: &str) -> BudgetCategoryGroup {
    create_group(
      db,
      &NewGroup {
        name: name.to_owned(),
        position: 0,
        scope,
      },
    )
    .await
    .unwrap()
  }

  async fn category(db: &Database, group_id: i64, name: &str) -> BudgetCategory {
    create_category(
      db,
      &NewCategory {
        group_id,
        name: name.to_owned(),
        note: None,
        position: 0,
        tone: Some("plasma".to_owned()),
      },
    )
    .await
    .unwrap()
  }

  async fn rule(db: &Database, category_id: i64, name: &str, position: i64) -> Rule {
    create_rule(
      db,
      &NewRule {
        category_id,
        enabled: true,
        match_mode: MatchMode::All,
        name: name.to_owned(),
        position,
        scope: BudgetScope::All,
      },
    )
    .await
    .unwrap()
  }

  mod create_category {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_cascades_when_the_group_is_deleted() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      category(&db, grp.id(), "Rent").await;

      delete_group(&db, grp.id()).await.unwrap();

      assert!(list_categories(&db, grp.id()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_records_the_category_fields() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;

      let cat = category(&db, grp.id(), "Rent").await;

      assert_eq!(cat.group_id(), grp.id());
      assert_eq!(cat.name(), "Rent");
      assert_eq!(cat.tone(), &Some("plasma".to_owned()));
    }
  }

  mod list_groups {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_does_not_confuse_character_and_corporation_with_the_same_id() {
      let db = store::open_test().await.unwrap();
      group(&db, BudgetScope::Character(5), "Pilot").await;
      group(&db, BudgetScope::Corporation(5), "Corp").await;

      let pilot = list_groups(&db, BudgetScope::Character(5)).await.unwrap();
      let corp = list_groups(&db, BudgetScope::Corporation(5)).await.unwrap();

      assert_eq!(
        pilot.iter().map(BudgetCategoryGroup::name).collect::<Vec<_>>(),
        ["Pilot"]
      );
      assert_eq!(corp.iter().map(BudgetCategoryGroup::name).collect::<Vec<_>>(), ["Corp"]);
    }

    #[tokio::test]
    async fn it_isolates_each_scope() {
      let db = store::open_test().await.unwrap();
      group(&db, BudgetScope::Character(1), "Alpha").await;
      group(&db, BudgetScope::Character(2), "Beta").await;
      group(&db, BudgetScope::All, "Shared").await;

      let character_one = list_groups(&db, BudgetScope::Character(1)).await.unwrap();
      let character_two = list_groups(&db, BudgetScope::Character(2)).await.unwrap();
      let all = list_groups(&db, BudgetScope::All).await.unwrap();

      assert_eq!(
        character_one.iter().map(BudgetCategoryGroup::name).collect::<Vec<_>>(),
        ["Alpha"]
      );
      assert_eq!(
        character_two.iter().map(BudgetCategoryGroup::name).collect::<Vec<_>>(),
        ["Beta"]
      );
      assert_eq!(
        all.iter().map(BudgetCategoryGroup::name).collect::<Vec<_>>(),
        ["Shared"]
      );
    }

    #[tokio::test]
    async fn it_orders_by_position() {
      let db = store::open_test().await.unwrap();
      create_group(
        &db,
        &NewGroup {
          name: "Second".to_owned(),
          position: 1,
          scope: BudgetScope::All,
        },
      )
      .await
      .unwrap();
      create_group(
        &db,
        &NewGroup {
          name: "First".to_owned(),
          position: 0,
          scope: BudgetScope::All,
        },
      )
      .await
      .unwrap();

      let groups = list_groups(&db, BudgetScope::All).await.unwrap();

      assert_eq!(
        groups.iter().map(BudgetCategoryGroup::name).collect::<Vec<_>>(),
        ["First", "Second"]
      );
    }
  }

  mod rename_group {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_changes_the_name_without_touching_position() {
      let db = store::open_test().await.unwrap();
      let grp = create_group(
        &db,
        &NewGroup {
          name: "Old".to_owned(),
          position: 4,
          scope: BudgetScope::All,
        },
      )
      .await
      .unwrap();

      rename_group(&db, grp.id(), "New").await.unwrap();

      let groups = list_groups(&db, BudgetScope::All).await.unwrap();
      assert_eq!(groups[0].name(), "New");
      assert_eq!(groups[0].position(), 4);
    }
  }

  mod set_target {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_replaces_an_existing_target() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      set_target(
        &db,
        cat.id(),
        &TargetInput {
          amount: 100.0,
          by_date: None,
          kind: "monthly".to_owned(),
        },
      )
      .await
      .unwrap();

      let updated = set_target(
        &db,
        cat.id(),
        &TargetInput {
          amount: 250.0,
          by_date: Some("2026-12-01".to_owned()),
          kind: "goalby".to_owned(),
        },
      )
      .await
      .unwrap();

      assert_eq!(updated.kind(), "goalby");
      assert_eq!(updated.amount(), 250.0);
      assert_eq!(updated.by_date(), &Some("2026-12-01".to_owned()));
      assert_eq!(load_target(&db, cat.id()).await.unwrap(), Some(updated));
    }
  }

  mod upsert_assignment {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_idempotent_per_category_and_month() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;

      upsert_assignment(&db, cat.id(), "2026-06", 100.0).await.unwrap();
      upsert_assignment(&db, cat.id(), "2026-06", 175.0).await.unwrap();

      let assignments = list_assignments(&db, cat.id()).await.unwrap();

      assert_eq!(assignments.len(), 1);
      assert_eq!(assignments[0].month(), "2026-06");
      assert_eq!(assignments[0].assigned(), 175.0);
    }

    #[tokio::test]
    async fn it_keeps_separate_rows_per_month() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;

      upsert_assignment(&db, cat.id(), "2026-05", 100.0).await.unwrap();
      upsert_assignment(&db, cat.id(), "2026-06", 120.0).await.unwrap();

      let assignments = list_assignments(&db, cat.id()).await.unwrap();

      assert_eq!(
        assignments.iter().map(BudgetAssignment::month).collect::<Vec<_>>(),
        ["2026-05", "2026-06"]
      );
    }
  }

  mod scope_assigned_total {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_sums_every_assignment_across_categories_and_months_for_the_scope() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::Character(1), "Bills").await;
      let rent = category(&db, grp.id(), "Rent").await;
      let food = category(&db, grp.id(), "Food").await;
      upsert_assignment(&db, rent.id(), "2026-05", 100.0).await.unwrap();
      upsert_assignment(&db, rent.id(), "2026-06", 120.0).await.unwrap();
      upsert_assignment(&db, food.id(), "2026-06", 80.0).await.unwrap();
      // A different scope's assignment must not leak into the total.
      let other = group(&db, BudgetScope::Character(2), "Other").await;
      let other_cat = category(&db, other.id(), "Misc").await;
      upsert_assignment(&db, other_cat.id(), "2026-06", 999.0).await.unwrap();

      let total = scope_assigned_total(&db, BudgetScope::Character(1)).await.unwrap();

      assert_eq!(total, 300.0);
    }

    #[tokio::test]
    async fn it_is_zero_for_a_scope_with_no_assignments() {
      let db = store::open_test().await.unwrap();
      group(&db, BudgetScope::All, "Bills").await;

      let total = scope_assigned_total(&db, BudgetScope::All).await.unwrap();

      assert_eq!(total, 0.0);
    }
  }

  mod upsert_ref_type_map {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_idempotent_per_scope_and_ref_type_for_all() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let first = category(&db, grp.id(), "Rent").await;
      let second = category(&db, grp.id(), "Power").await;

      upsert_ref_type_map(&db, BudgetScope::All, "market_escrow", first.id())
        .await
        .unwrap();
      upsert_ref_type_map(&db, BudgetScope::All, "market_escrow", second.id())
        .await
        .unwrap();

      let maps = list_ref_type_maps(&db, BudgetScope::All).await.unwrap();

      assert_eq!(maps.len(), 1);
      assert_eq!(maps[0].category_id(), second.id());
    }

    #[tokio::test]
    async fn it_isolates_ref_type_maps_per_scope() {
      let db = store::open_test().await.unwrap();
      let all_group = group(&db, BudgetScope::All, "Shared").await;
      let all_cat = category(&db, all_group.id(), "Rent").await;
      let char_group = group(&db, BudgetScope::Character(1), "Pilot").await;
      let char_cat = category(&db, char_group.id(), "Fuel").await;

      upsert_ref_type_map(&db, BudgetScope::All, "bounty_prizes", all_cat.id())
        .await
        .unwrap();
      upsert_ref_type_map(&db, BudgetScope::Character(1), "bounty_prizes", char_cat.id())
        .await
        .unwrap();

      let all = list_ref_type_maps(&db, BudgetScope::All).await.unwrap();
      let pilot = list_ref_type_maps(&db, BudgetScope::Character(1)).await.unwrap();

      assert_eq!(all.len(), 1);
      assert_eq!(all[0].category_id(), all_cat.id());
      assert_eq!(pilot.len(), 1);
      assert_eq!(pilot[0].category_id(), char_cat.id());
    }
  }

  mod upsert_entry_assignment {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_round_trips_and_is_idempotent_on_the_unique_key() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let first = category(&db, grp.id(), "Rent").await;
      let second = category(&db, grp.id(), "Power").await;

      upsert_entry_assignment(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        42,
        first.id(),
      )
      .await
      .unwrap();
      upsert_entry_assignment(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        42,
        second.id(),
      )
      .await
      .unwrap();

      let assignments = list_entry_assignments(&db, BudgetScope::All).await.unwrap();

      assert_eq!(assignments.len(), 1);
      assert_eq!(assignments[0].entry_kind(), "journal");
      assert_eq!(assignments[0].entry_id(), 42);
      assert_eq!(assignments[0].category_id(), second.id());
    }

    #[tokio::test]
    async fn it_separates_two_owners_sharing_an_eve_id_under_all_scope() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let pilot = category(&db, grp.id(), "Rent").await;
      let corp = category(&db, grp.id(), "Power").await;

      upsert_entry_assignment(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Market,
        500,
        pilot.id(),
      )
      .await
      .unwrap();
      upsert_entry_assignment(
        &db,
        BudgetScope::All,
        BudgetOwner::Corporation(2),
        BudgetEntryKind::Market,
        500,
        corp.id(),
      )
      .await
      .unwrap();

      let assignments = list_entry_assignments(&db, BudgetScope::All).await.unwrap();

      assert_eq!(assignments.len(), 2);
      assert_eq!(
        assignments
          .iter()
          .find(|a| a.owner_kind() == "character")
          .map(BudgetEntryAssignment::category_id),
        Some(pilot.id())
      );
      assert_eq!(
        assignments
          .iter()
          .find(|a| a.owner_kind() == "corporation")
          .map(BudgetEntryAssignment::category_id),
        Some(corp.id())
      );
    }

    #[tokio::test]
    async fn it_separates_entry_kinds_and_scopes_with_the_same_entry_id() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::Character(1), "Pilot").await;
      let cat = category(&db, grp.id(), "Fuel").await;

      upsert_entry_assignment(
        &db,
        BudgetScope::Character(1),
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        7,
        cat.id(),
      )
      .await
      .unwrap();
      upsert_entry_assignment(
        &db,
        BudgetScope::Character(1),
        BudgetOwner::Character(1),
        BudgetEntryKind::Market,
        7,
        cat.id(),
      )
      .await
      .unwrap();

      let assignments = list_entry_assignments(&db, BudgetScope::Character(1)).await.unwrap();

      assert_eq!(
        assignments
          .iter()
          .map(BudgetEntryAssignment::entry_kind)
          .collect::<Vec<_>>(),
        ["journal", "market"]
      );
      assert!(list_entry_assignments(&db, BudgetScope::All).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_cascades_when_the_category_is_deleted() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      upsert_entry_assignment(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Market,
        99,
        cat.id(),
      )
      .await
      .unwrap();

      delete_category(&db, cat.id()).await.unwrap();

      assert!(list_entry_assignments(&db, BudgetScope::All).await.unwrap().is_empty());
    }
  }

  mod delete_entry_assignment {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_removes_only_the_matching_assignment() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      upsert_entry_assignment(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        1,
        cat.id(),
      )
      .await
      .unwrap();
      upsert_entry_assignment(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        2,
        cat.id(),
      )
      .await
      .unwrap();

      delete_entry_assignment(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Journal,
        1,
      )
      .await
      .unwrap();

      let remaining = list_entry_assignments(&db, BudgetScope::All).await.unwrap();

      assert_eq!(remaining.len(), 1);
      assert_eq!(remaining[0].entry_id(), 2);
    }
  }

  mod list_rules {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_round_trips_a_multi_condition_rule_with_order_preserved() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      let created = rule(&db, cat.id(), "Landlord", 0).await;
      let conditions = vec![
        RuleCondition {
          field: RuleField::Party,
          op: RuleOp::Contains,
          value: "Estate".to_owned(),
          value2: None,
        },
        RuleCondition {
          field: RuleField::Amount,
          op: RuleOp::Between,
          value: "100".to_owned(),
          value2: Some("500".to_owned()),
        },
        RuleCondition {
          field: RuleField::Direction,
          op: RuleOp::Is,
          value: "out".to_owned(),
          value2: None,
        },
      ];
      replace_rule_conditions(&db, created.id(), &conditions).await.unwrap();

      let rules = list_rules(&db, BudgetScope::All).await.unwrap();

      assert_eq!(rules.len(), 1);
      assert_eq!(rules[0].category_id(), cat.id());
      assert_eq!(rules[0].name(), "Landlord");
      assert!(rules[0].enabled());
      assert_eq!(rules[0].match_mode(), MatchMode::All);
      assert_eq!(rules[0].conditions(), &conditions);
    }

    #[tokio::test]
    async fn it_orders_rules_by_position() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      rule(&db, cat.id(), "Second", 1).await;
      rule(&db, cat.id(), "First", 0).await;

      let rules = list_rules(&db, BudgetScope::All).await.unwrap();

      assert_eq!(rules.iter().map(Rule::name).collect::<Vec<_>>(), ["First", "Second"]);
    }

    #[tokio::test]
    async fn it_isolates_rules_per_scope() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      rule(&db, cat.id(), "Shared", 0).await;
      let char_grp = group(&db, BudgetScope::Character(1), "Pilot").await;
      let char_cat = category(&db, char_grp.id(), "Fuel").await;
      create_rule(
        &db,
        &NewRule {
          category_id: char_cat.id(),
          enabled: true,
          match_mode: MatchMode::Any,
          name: "Scoped".to_owned(),
          position: 0,
          scope: BudgetScope::Character(1),
        },
      )
      .await
      .unwrap();

      let all = list_rules(&db, BudgetScope::All).await.unwrap();
      let scoped = list_rules(&db, BudgetScope::Character(1)).await.unwrap();

      assert_eq!(all.iter().map(Rule::name).collect::<Vec<_>>(), ["Shared"]);
      assert_eq!(scoped.iter().map(Rule::name).collect::<Vec<_>>(), ["Scoped"]);
    }
  }

  mod delete_rule {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_cascades_conditions() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      let created = rule(&db, cat.id(), "Landlord", 0).await;
      replace_rule_conditions(
        &db,
        created.id(),
        &[RuleCondition {
          field: RuleField::Party,
          op: RuleOp::Contains,
          value: "Estate".to_owned(),
          value2: None,
        }],
      )
      .await
      .unwrap();

      delete_rule(&db, created.id()).await.unwrap();

      let orphans: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM budget_rule_conditions")
        .fetch_one(&db.0)
        .await
        .unwrap();

      assert_eq!(orphans, 0);
      assert!(list_rules(&db, BudgetScope::All).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_cascades_when_the_category_is_deleted() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      rule(&db, cat.id(), "Landlord", 0).await;

      delete_category(&db, cat.id()).await.unwrap();

      assert!(list_rules(&db, BudgetScope::All).await.unwrap().is_empty());
    }
  }

  mod replace_rule_conditions {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_replaces_the_whole_condition_set() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      let created = rule(&db, cat.id(), "Landlord", 0).await;
      replace_rule_conditions(
        &db,
        created.id(),
        &[
          RuleCondition {
            field: RuleField::Party,
            op: RuleOp::Contains,
            value: "Estate".to_owned(),
            value2: None,
          },
          RuleCondition {
            field: RuleField::Reference,
            op: RuleOp::StartsWith,
            value: "RENT".to_owned(),
            value2: None,
          },
        ],
      )
      .await
      .unwrap();

      let replacement = vec![RuleCondition {
        field: RuleField::Amount,
        op: RuleOp::GreaterThan,
        value: "1000".to_owned(),
        value2: None,
      }];
      replace_rule_conditions(&db, created.id(), &replacement).await.unwrap();

      let rules = list_rules(&db, BudgetScope::All).await.unwrap();
      assert_eq!(rules[0].conditions(), &replacement);
    }
  }

  mod reorder_rules {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_rewrites_priority_order() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      let a = rule(&db, cat.id(), "A", 0).await;
      let b = rule(&db, cat.id(), "B", 1).await;
      let c = rule(&db, cat.id(), "C", 2).await;

      reorder_rules(&db, &[c.id(), a.id(), b.id()]).await.unwrap();

      let rules = list_rules(&db, BudgetScope::All).await.unwrap();
      assert_eq!(rules.iter().map(Rule::name).collect::<Vec<_>>(), ["C", "A", "B"]);
    }
  }

  mod update_rule {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_changes_the_editable_fields() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, BudgetScope::All, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      let other = category(&db, grp.id(), "Power").await;
      let mut created = rule(&db, cat.id(), "Landlord", 0).await;

      created.category_id = other.id();
      created.enabled = false;
      created.match_mode = MatchMode::Any;
      created.name = "Utilities".to_owned();
      update_rule(&db, &created).await.unwrap();

      let rules = list_rules(&db, BudgetScope::All).await.unwrap();

      assert_eq!(rules[0].category_id(), other.id());
      assert!(!rules[0].enabled());
      assert_eq!(rules[0].match_mode(), MatchMode::Any);
      assert_eq!(rules[0].name(), "Utilities");
    }
  }
}
