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

// Authoritative cross-owner unassign, as a single static SQL statement so it passes
// to `sqlx::query` without a dynamic-string audit (it carries only bound leg
// parameters). The `legs` CTE mirrors `RECONCILE_SPLIT_OWNER_ASSIGNMENTS_SQL`: every
// concrete wallet leg of a market event keyed by its `transaction_id` (a market row
// by its own `transaction_id`; a `market_transaction` journal twin and the
// broker-fee / transaction-tax legs by `context_id`). `event` is the
// transaction_id(s) the cleared leg belongs to; the DELETE removes every All-scope
// assignment for any leg of those events across ALL owners — including owners whose
// wallet is unsynced and absent from the in-memory cascade — so the reconciler has
// nothing left to resurrect. The cleared leg's own row is OR'd in so a leg the CTE
// does not represent (a non-market journal entry) is still removed.
const DELETE_EVENT_ASSIGNMENTS_SQL: &str = "WITH legs AS ( \
    SELECT transaction_id AS transaction_id, 'character' AS owner_kind, character_id AS owner_id, \
            'market' AS entry_kind, transaction_id AS entry_id \
      FROM character_wallet_transaction \
    UNION ALL \
    SELECT transaction_id, 'corporation', corporation_id, 'market', transaction_id \
      FROM corporation_wallet_transaction \
    UNION ALL \
    SELECT context_id, 'character', character_id, 'journal', id \
      FROM character_wallet_journal \
      WHERE context_id IS NOT NULL \
        AND ref_type IN ('market_transaction', 'brokers_fee', 'transaction_tax') \
    UNION ALL \
    SELECT context_id, 'corporation', corporation_id, 'journal', id \
      FROM corporation_wallet_journal \
      WHERE context_id IS NOT NULL \
        AND ref_type IN ('market_transaction', 'brokers_fee', 'transaction_tax') \
  ), \
    event AS ( \
      SELECT DISTINCT transaction_id FROM legs \
        WHERE owner_kind = ?1 AND owner_id = ?2 AND entry_kind = ?3 AND entry_id = ?4 \
    ) \
    DELETE FROM budget_entry_assignments \
      WHERE scope_kind = 'all' AND scope_id IS NULL \
        AND ( \
          EXISTS ( \
            SELECT 1 FROM legs l JOIN event e ON e.transaction_id = l.transaction_id \
            WHERE l.owner_kind = budget_entry_assignments.owner_kind \
              AND l.owner_id = budget_entry_assignments.owner_id \
              AND l.entry_kind = budget_entry_assignments.entry_kind \
              AND l.entry_id = budget_entry_assignments.entry_id \
          ) \
          OR (owner_kind = ?1 AND owner_id = ?2 AND entry_kind = ?3 AND entry_id = ?4) \
        )";

/// Authoritative cross-owner unassign for a single budget event. Given the leg the
/// user cleared, this deletes the All-scope assignment for every leg of that event
/// (the market mirror, its journal twin, and the broker-fee / transaction-tax legs)
/// across ALL owners sharing its `transaction_id` — DB-side, regardless of what is
/// loaded in memory — so a mark cleared while a sibling owner's wallet is unsynced
/// leaves no orphan copy behind. Keyed by the event, not by `(owner, entry_id)`:
/// reuses the same linkage as [`reconcile_split_owner_assignments`] so the two stay
/// in step, and combined with the reconciler's `updated_at` guard guarantees a
/// cleared mark is not resurrected on the next sync.
// Authoritative cross-owner unassign; called from the wallet chip- and bulk-clear paths.
pub async fn delete_event_assignments(
  db: &Database,
  owner: BudgetOwner,
  entry_kind: BudgetEntryKind,
  entry_id: i64,
) -> Result<(), Error> {
  sqlx::query(DELETE_EVENT_ASSIGNMENTS_SQL)
    .bind(owner.owner_kind())
    .bind(owner.owner_id())
    .bind(entry_kind.as_str())
    .bind(entry_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

/// Forward GC for per-entry overrides whose entry has since been pruned from
/// every live ledger. The one-time migration heals the historical backlog; this
/// keeps it clean going forward by dropping any assignment whose `entry_id` no
/// longer resolves to a wallet row for its own `(owner_kind, owner_id)` — a
/// journal id for Journal rows, a transaction id for Market rows. It is
/// owner-keyed so a corp and a character sharing an EVE id are never confused,
/// and set-based so a single call cleans the whole table. Returns the number of
/// overrides removed.
// Per-entry budget assignment GC; run from the BudgetAssignmentReconcile post-sync job so a
// reconciled copy whose wallet row later disappears is collected on the next pass.
pub async fn prune_orphan_entry_assignments(db: &Database) -> Result<u64, Error> {
  let result = sqlx::query(
    "DELETE FROM budget_entry_assignments \
    WHERE (entry_kind = 'journal' AND owner_kind = 'character' AND NOT EXISTS ( \
        SELECT 1 FROM character_wallet_journal cwj \
        WHERE cwj.id = budget_entry_assignments.entry_id AND cwj.character_id = budget_entry_assignments.owner_id)) \
      OR (entry_kind = 'journal' AND owner_kind = 'corporation' AND NOT EXISTS ( \
        SELECT 1 FROM corporation_wallet_journal cwj \
        WHERE cwj.id = budget_entry_assignments.entry_id AND cwj.corporation_id = budget_entry_assignments.owner_id)) \
      OR (entry_kind = 'market' AND owner_kind = 'character' AND NOT EXISTS ( \
        SELECT 1 FROM character_wallet_transaction cwt \
        WHERE cwt.transaction_id = budget_entry_assignments.entry_id AND cwt.character_id = budget_entry_assignments.owner_id)) \
      OR (entry_kind = 'market' AND owner_kind = 'corporation' AND NOT EXISTS ( \
        SELECT 1 FROM corporation_wallet_transaction cwt \
        WHERE cwt.transaction_id = budget_entry_assignments.entry_id AND cwt.corporation_id = budget_entry_assignments.owner_id))",
  )
  .execute(db.writer())
  .await?;
  Ok(result.rows_affected())
}

// Post-sync cross-owner reconciliation, as a single static SQL statement so it
// passes to `sqlx::query` without a dynamic-string audit (it carries no caller
// data — only the bound `updated_at`/`created_at` parameter). The `legs` CTE is
// every concrete wallet leg of a market event keyed by its `transaction_id`,
// stamped with its owner: a market row is keyed by its own `transaction_id`; a
// journal twin (`market_transaction`) and the broker-fee / transaction-tax legs
// are keyed by `context_id`, which EVE sets to the trade's `transaction_id`. That
// is the same linkage the in-memory cascade encodes (wallet.rs:3609-3729), set in
// SQL so it sees every locally-held leg, not just the loaded/scope-filtered page.
const RECONCILE_SPLIT_OWNER_ASSIGNMENTS_SQL: &str = "WITH legs AS ( \
    SELECT transaction_id AS transaction_id, 'character' AS owner_kind, character_id AS owner_id, \
            'market' AS entry_kind, transaction_id AS entry_id \
      FROM character_wallet_transaction \
    UNION ALL \
    SELECT transaction_id, 'corporation', corporation_id, 'market', transaction_id \
      FROM corporation_wallet_transaction \
    UNION ALL \
    SELECT context_id, 'character', character_id, 'journal', id \
      FROM character_wallet_journal \
      WHERE context_id IS NOT NULL \
        AND ref_type IN ('market_transaction', 'brokers_fee', 'transaction_tax') \
    UNION ALL \
    SELECT context_id, 'corporation', corporation_id, 'journal', id \
      FROM corporation_wallet_journal \
      WHERE context_id IS NOT NULL \
        AND ref_type IN ('market_transaction', 'brokers_fee', 'transaction_tax') \
  ), \
    sources AS ( \
      SELECT l.transaction_id AS transaction_id, a.category_id AS category_id, a.updated_at AS updated_at, a.id AS id \
        FROM budget_entry_assignments a \
        JOIN legs l \
          ON l.owner_kind = a.owner_kind AND l.owner_id = a.owner_id \
          AND l.entry_kind = a.entry_kind AND l.entry_id = a.entry_id \
        WHERE a.scope_kind = 'all' AND a.scope_id IS NULL \
    ), \
    winners AS ( \
      SELECT s.transaction_id AS transaction_id, s.category_id AS category_id \
        FROM sources s \
        JOIN ( \
          SELECT transaction_id, MAX(updated_at) AS updated_at FROM sources GROUP BY transaction_id \
        ) latest ON latest.transaction_id = s.transaction_id AND latest.updated_at = s.updated_at \
        JOIN ( \
          SELECT transaction_id, updated_at, MAX(id) AS id FROM sources GROUP BY transaction_id, updated_at \
        ) tiebreak ON tiebreak.transaction_id = s.transaction_id AND tiebreak.updated_at = s.updated_at \
                  AND tiebreak.id = s.id \
    ) \
    INSERT INTO budget_entry_assignments \
      (scope_kind, scope_id, owner_kind, owner_id, entry_kind, entry_id, category_id, created_at, updated_at) \
    SELECT 'all', NULL, l.owner_kind, l.owner_id, l.entry_kind, l.entry_id, w.category_id, ?1, ?1 \
      FROM legs l \
      JOIN winners w ON w.transaction_id = l.transaction_id \
      WHERE NOT EXISTS ( \
        SELECT 1 FROM budget_entry_assignments a \
        WHERE a.scope_kind = 'all' AND a.scope_id IS NULL \
          AND a.owner_kind = l.owner_kind AND a.owner_id = l.owner_id \
          AND a.entry_kind = l.entry_kind AND a.entry_id = l.entry_id \
      ) \
      GROUP BY l.owner_kind, l.owner_id, l.entry_kind, l.entry_id, w.category_id \
    ON CONFLICT(scope_kind, COALESCE(scope_id, -1), owner_kind, owner_id, entry_kind, entry_id) DO NOTHING";

/// Post-sync cross-owner budget reconciliation. For every market event whose legs
/// (a market row, its journal twin, and its broker-fee / transaction-tax legs)
/// span more than one owner, this materializes the missing per-owner assignment
/// copies so a mark placed on one owner's fast-arriving copy reaches the slow
/// sibling legs that synced later. See ADR draft xnszopnu / spec mqzpprvw.
///
/// It is fill-only, override-respecting, idempotent, and guarded against
/// resurrection:
///   - fill-only / override-respecting: a target leg that already carries its own
///     assignment is never touched (`NOT EXISTS` against the assignment table), so
///     a deliberate corp-side mark survives.
///   - resurrection guard: each event's category is taken from the *newest*
///     assignment in the group (max `updated_at`, ties broken by max id). A leg
///     whose owner deliberately unassigned has no row, so it can only be re-filled
///     by a source mark that is strictly newer — a stale source can never win.
///   - idempotent: writes route through the owner-aware unique index via the same
///     `ON CONFLICT ... DO NOTHING`-shaped upsert, so a second pass is a no-op.
///
/// Operates over the All scope only (resolution is always `scope_kind='all'`).
/// Returns the number of sibling copies written. This SQL is the source of truth
/// the one-time backfill migration mirrors.
// Cross-owner budget reconciliation (post-sync job); wired into BudgetAssignmentReconcile.
pub async fn reconcile_split_owner_assignments(db: &Database) -> Result<u64, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let result = sqlx::query(RECONCILE_SPLIT_OWNER_ASSIGNMENTS_SQL)
    .bind(&now)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
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

  mod delete_event_assignments {
    use pretty_assertions::assert_eq;

    use super::*;

    // The cross-owner delete resolves an event's legs from real wallet rows, so the
    // test seeds the market/journal twins directly. Wallet tables carry an owner FK
    // (characters/corporations); the linkage query never reads those parents, so the
    // FK is disabled to keep the fixture to just the legs under test.
    async fn disable_foreign_keys(db: &Database) {
      sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(db.writer())
        .await
        .unwrap();
    }

    async fn seed_market(db: &Database, table: &str, owner_col: &str, owner_id: i64, transaction_id: i64, twin: i64) {
      let (extra_col, extra_val) = if owner_col == "character_id" {
        (", is_personal", ", 0")
      } else {
        (", division", ", 1")
      };
      // Closed set of literal table / column names from this test module, never caller
      // data, so the dynamically-built statement is safe to assert.
      sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {table} \
          (transaction_id, {owner_col}, client_id, date, is_buy, journal_ref_id, location_id, quantity, type_id, unit_price{extra_col}) \
          VALUES (?, ?, 0, '2026-06-01T00:00:00Z', 0, ?, 0, 1, 34, 1.0{extra_val})",
      )))
      .bind(transaction_id)
      .bind(owner_id)
      .bind(twin)
      .execute(db.writer())
      .await
      .unwrap();
    }

    async fn seed_journal(
      db: &Database,
      table: &str,
      owner_col: &str,
      owner_id: i64,
      id: i64,
      ref_type: &str,
      context_id: Option<i64>,
    ) {
      let division = if owner_col == "corporation_id" {
        ", division"
      } else {
        ""
      };
      let division_val = if owner_col == "corporation_id" { ", 1" } else { "" };
      // Closed set of literal table / column names (see seed_market) — safe to assert.
      sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {table} (id, {owner_col}{division}, date, description, ref_type, amount, context_id) \
          VALUES (?, ?{division_val}, '2026-06-01T00:00:00Z', '', ?, -1.0, ?)"
      )))
      .bind(id)
      .bind(owner_id)
      .bind(ref_type)
      .bind(context_id)
      .execute(db.writer())
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_removes_both_owners_copies_for_a_shared_market_event() {
      let db = store::open_test().await.unwrap();
      disable_foreign_keys(&db).await;
      let grp = group(&db, BudgetScope::All, "Trade").await;
      let cat = category(&db, grp.id(), "Sales").await;
      // One trade mirrored into a character and a corporation, sharing transaction_id 500.
      seed_market(&db, "character_wallet_transaction", "character_id", 1, 500, 9001).await;
      seed_market(&db, "corporation_wallet_transaction", "corporation_id", 2, 500, 9002).await;
      // Both owners hold a mark on their market copy (as if reconciliation had filled
      // the corp copy while its wallet was loaded earlier).
      for owner in [BudgetOwner::Character(1), BudgetOwner::Corporation(2)] {
        upsert_entry_assignment(&db, BudgetScope::All, owner, BudgetEntryKind::Market, 500, cat.id())
          .await
          .unwrap();
      }

      // Clear via the character leg only — the sibling corp wallet is not "loaded".
      delete_event_assignments(&db, BudgetOwner::Character(1), BudgetEntryKind::Market, 500)
        .await
        .unwrap();

      let remaining = list_entry_assignments(&db, BudgetScope::All).await.unwrap();
      assert!(
        remaining.is_empty(),
        "both owners' copies must be removed, leaving none: {remaining:?}"
      );
    }

    #[tokio::test]
    async fn it_removes_the_journal_and_fee_legs_across_owners() {
      let db = store::open_test().await.unwrap();
      disable_foreign_keys(&db).await;
      let grp = group(&db, BudgetScope::All, "Trade").await;
      let cat = category(&db, grp.id(), "Sales").await;
      // Market mirror plus the corp journal twin and a tax fee leg, all linked by
      // transaction_id 500 / context_id 500.
      seed_market(&db, "character_wallet_transaction", "character_id", 1, 500, 9001).await;
      seed_market(&db, "corporation_wallet_transaction", "corporation_id", 2, 500, 9002).await;
      seed_journal(
        &db,
        "corporation_wallet_journal",
        "corporation_id",
        2,
        9002,
        "market_transaction",
        Some(500),
      )
      .await;
      seed_journal(
        &db,
        "corporation_wallet_journal",
        "corporation_id",
        2,
        9003,
        "transaction_tax",
        Some(500),
      )
      .await;
      for (owner, kind, entry_id) in [
        (BudgetOwner::Character(1), BudgetEntryKind::Market, 500),
        (BudgetOwner::Corporation(2), BudgetEntryKind::Market, 500),
        (BudgetOwner::Corporation(2), BudgetEntryKind::Journal, 9002),
        (BudgetOwner::Corporation(2), BudgetEntryKind::Journal, 9003),
      ] {
        upsert_entry_assignment(&db, BudgetScope::All, owner, kind, entry_id, cat.id())
          .await
          .unwrap();
      }

      delete_event_assignments(&db, BudgetOwner::Character(1), BudgetEntryKind::Market, 500)
        .await
        .unwrap();

      assert!(list_entry_assignments(&db, BudgetScope::All).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_leaves_an_unrelated_events_assignment_intact() {
      let db = store::open_test().await.unwrap();
      disable_foreign_keys(&db).await;
      let grp = group(&db, BudgetScope::All, "Trade").await;
      let cat = category(&db, grp.id(), "Sales").await;
      seed_market(&db, "character_wallet_transaction", "character_id", 1, 500, 9001).await;
      seed_market(&db, "character_wallet_transaction", "character_id", 1, 600, 9005).await;
      upsert_entry_assignment(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Market,
        500,
        cat.id(),
      )
      .await
      .unwrap();
      upsert_entry_assignment(
        &db,
        BudgetScope::All,
        BudgetOwner::Character(1),
        BudgetEntryKind::Market,
        600,
        cat.id(),
      )
      .await
      .unwrap();

      delete_event_assignments(&db, BudgetOwner::Character(1), BudgetEntryKind::Market, 500)
        .await
        .unwrap();

      let remaining = list_entry_assignments(&db, BudgetScope::All).await.unwrap();
      assert_eq!(remaining.len(), 1);
      assert_eq!(remaining[0].entry_id(), 600);
    }

    #[tokio::test]
    async fn it_survives_reconciliation_with_no_resurrection() {
      let db = store::open_test().await.unwrap();
      disable_foreign_keys(&db).await;
      let grp = group(&db, BudgetScope::All, "Trade").await;
      let cat = category(&db, grp.id(), "Sales").await;
      seed_market(&db, "character_wallet_transaction", "character_id", 1, 500, 9001).await;
      seed_market(&db, "corporation_wallet_transaction", "corporation_id", 2, 500, 9002).await;
      // Mark under both owners, then authoritatively clear via the character leg.
      for owner in [BudgetOwner::Character(1), BudgetOwner::Corporation(2)] {
        upsert_entry_assignment(&db, BudgetScope::All, owner, BudgetEntryKind::Market, 500, cat.id())
          .await
          .unwrap();
      }
      delete_event_assignments(&db, BudgetOwner::Character(1), BudgetEntryKind::Market, 500)
        .await
        .unwrap();

      // The reconciler must find no surviving source mark to propagate, so the cleared
      // event stays gone (the updated_at guard has nothing newer to resurrect from).
      reconcile_split_owner_assignments(&db).await.unwrap();

      assert!(
        list_entry_assignments(&db, BudgetScope::All).await.unwrap().is_empty(),
        "a cleared mark must not be resurrected by reconciliation"
      );
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
