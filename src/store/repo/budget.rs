use crate::store::{
  Database, Error,
  model::{
    BudgetAssignment, BudgetCategory, BudgetCategoryGroup, BudgetEntryAssignment, BudgetOwner, BudgetTarget, MatchMode,
    NewCategory, NewGroup, NewRule, Rule, RuleCondition, RuleConditionRow, RuleRow, TargetInput,
  },
};

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

pub async fn create_group(db: &Database, group: &NewGroup) -> Result<BudgetCategoryGroup, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, BudgetCategoryGroup>(
    "INSERT INTO budget_category_groups (name, position, created_at, updated_at) \
    VALUES (?, ?, ?, ?) \
    RETURNING id, name, position, created_at, updated_at",
  )
  .bind(&group.name)
  .bind(group.position)
  .bind(&now)
  .bind(&now)
  .fetch_one(&db.0)
  .await?;
  Ok(row)
}

pub async fn create_rule(db: &Database, rule: &NewRule) -> Result<Rule, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, RuleRow>(
    "INSERT INTO budget_rules \
    (category_id, name, enabled, match_mode, position, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?) \
    RETURNING id, category_id, name, enabled, match_mode",
  )
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

pub async fn delete_category(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM budget_categories WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn delete_entry_assignment(db: &Database, owner: BudgetOwner, entry_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM budget_entry_assignments WHERE owner_kind = ? AND owner_id = ? AND entry_id = ?")
    .bind(owner.owner_kind())
    .bind(owner.owner_id())
    .bind(entry_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn delete_group(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM budget_category_groups WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn delete_rule(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM budget_rules WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

#[expect(dead_code)]
pub async fn delete_target(db: &Database, category_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM budget_targets WHERE category_id = ?")
    .bind(category_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn list_assignments(db: &Database, category_id: i64) -> Result<Vec<BudgetAssignment>, Error> {
  let rows = sqlx::query_as::<_, BudgetAssignment>(
    "SELECT id, category_id, month, assigned FROM budget_assignments WHERE category_id = ? ORDER BY month",
  )
  .bind(category_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

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

pub async fn list_entry_assignments(db: &Database) -> Result<Vec<BudgetEntryAssignment>, Error> {
  let rows = sqlx::query_as::<_, BudgetEntryAssignment>(
    "SELECT id, owner_kind, owner_id, entry_id, category_id, created_at, updated_at \
    FROM budget_entry_assignments ORDER BY owner_kind, owner_id, entry_id",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn list_groups(db: &Database) -> Result<Vec<BudgetCategoryGroup>, Error> {
  let rows = sqlx::query_as::<_, BudgetCategoryGroup>(
    "SELECT id, name, position, created_at, updated_at \
    FROM budget_category_groups ORDER BY position, id",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn list_rules(db: &Database) -> Result<Vec<Rule>, Error> {
  let rule_rows = sqlx::query_as::<_, RuleRow>(
    "SELECT id, category_id, name, enabled, match_mode FROM budget_rules ORDER BY position, id",
  )
  .fetch_all(&db.0)
  .await?;

  let condition_rows = sqlx::query_as::<_, RuleConditionRow>(
    "SELECT c.rule_id, c.field, c.op, c.value, c.value2 FROM budget_rule_conditions c \
    JOIN budget_rules r ON r.id = c.rule_id \
    ORDER BY c.rule_id, c.position, c.id",
  )
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

pub async fn load_target(db: &Database, category_id: i64) -> Result<Option<BudgetTarget>, Error> {
  let row = sqlx::query_as::<_, BudgetTarget>(
    "SELECT category_id, kind, amount, by_date FROM budget_targets WHERE category_id = ?",
  )
  .bind(category_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn assigned_total(db: &Database) -> Result<f64, Error> {
  let total: Option<f64> = sqlx::query_scalar("SELECT SUM(assigned) FROM budget_assignments")
    .fetch_one(&db.0)
    .await?;
  Ok(total.unwrap_or(0.0))
}

pub async fn owner_holds_entry(db: &Database, owner: BudgetOwner, entry_id: i64) -> Result<bool, Error> {
  let query = match owner {
    BudgetOwner::Character(_) => {
      "SELECT EXISTS(SELECT 1 FROM character_wallet_journal WHERE character_id = ? AND id = ?)"
    }
    BudgetOwner::Corporation(_) => {
      "SELECT EXISTS(SELECT 1 FROM corporation_wallet_journal WHERE corporation_id = ? AND id = ?)"
    }
  };
  let exists = sqlx::query_scalar::<_, i64>(query)
    .bind(owner.owner_id())
    .bind(entry_id)
    .fetch_one(&db.0)
    .await?;
  Ok(exists != 0)
}

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

pub async fn upsert_entry_assignment(
  db: &Database,
  owner: BudgetOwner,
  entry_id: i64,
  category_id: i64,
) -> Result<BudgetEntryAssignment, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, BudgetEntryAssignment>(
    "INSERT INTO budget_entry_assignments \
    (owner_kind, owner_id, entry_id, category_id, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?) \
    ON CONFLICT(owner_kind, owner_id, entry_id) \
    DO UPDATE SET category_id = excluded.category_id, updated_at = excluded.updated_at \
    RETURNING id, owner_kind, owner_id, entry_id, category_id, created_at, updated_at",
  )
  .bind(owner.owner_kind())
  .bind(owner.owner_id())
  .bind(entry_id)
  .bind(category_id)
  .bind(&now)
  .bind(&now)
  .fetch_one(&db.0)
  .await?;
  Ok(row)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{RuleField, RuleOp},
  };

  async fn group(db: &Database, name: &str) -> BudgetCategoryGroup {
    create_group(
      db,
      &NewGroup {
        name: name.to_owned(),
        position: 0,
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
      let grp = group(&db, "Bills").await;
      category(&db, grp.id(), "Rent").await;

      delete_group(&db, grp.id()).await.unwrap();

      assert!(list_categories(&db, grp.id()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_records_the_category_fields() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, "Bills").await;

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
    async fn it_orders_by_position() {
      let db = store::open_test().await.unwrap();
      create_group(
        &db,
        &NewGroup {
          name: "Second".to_owned(),
          position: 1,
        },
      )
      .await
      .unwrap();
      create_group(
        &db,
        &NewGroup {
          name: "First".to_owned(),
          position: 0,
        },
      )
      .await
      .unwrap();

      let groups = list_groups(&db).await.unwrap();

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
        },
      )
      .await
      .unwrap();

      rename_group(&db, grp.id(), "New").await.unwrap();

      let groups = list_groups(&db).await.unwrap();
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
      let grp = group(&db, "Bills").await;
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
      let grp = group(&db, "Bills").await;
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
      let grp = group(&db, "Bills").await;
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

  mod assigned_total {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_sums_every_assignment_across_categories_and_months() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, "Bills").await;
      let rent = category(&db, grp.id(), "Rent").await;
      let food = category(&db, grp.id(), "Food").await;
      upsert_assignment(&db, rent.id(), "2026-05", 100.0).await.unwrap();
      upsert_assignment(&db, rent.id(), "2026-06", 120.0).await.unwrap();
      upsert_assignment(&db, food.id(), "2026-06", 80.0).await.unwrap();

      let total = assigned_total(&db).await.unwrap();

      assert_eq!(total, 300.0);
    }

    #[tokio::test]
    async fn it_is_zero_with_no_assignments() {
      let db = store::open_test().await.unwrap();
      group(&db, "Bills").await;

      let total = assigned_total(&db).await.unwrap();

      assert_eq!(total, 0.0);
    }
  }

  mod upsert_entry_assignment {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_round_trips_and_is_idempotent_on_the_unique_key() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, "Bills").await;
      let first = category(&db, grp.id(), "Rent").await;
      let second = category(&db, grp.id(), "Power").await;

      upsert_entry_assignment(&db, BudgetOwner::Character(1), 42, first.id())
        .await
        .unwrap();
      upsert_entry_assignment(&db, BudgetOwner::Character(1), 42, second.id())
        .await
        .unwrap();

      let assignments = list_entry_assignments(&db).await.unwrap();

      assert_eq!(assignments.len(), 1);
      assert_eq!(assignments[0].entry_id(), 42);
      assert_eq!(assignments[0].category_id(), second.id());
    }

    #[tokio::test]
    async fn it_separates_two_owners_sharing_an_eve_id() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, "Bills").await;
      let pilot = category(&db, grp.id(), "Rent").await;
      let corp = category(&db, grp.id(), "Power").await;

      upsert_entry_assignment(&db, BudgetOwner::Character(1), 500, pilot.id())
        .await
        .unwrap();
      upsert_entry_assignment(&db, BudgetOwner::Corporation(1), 500, corp.id())
        .await
        .unwrap();

      let assignments = list_entry_assignments(&db).await.unwrap();

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
    async fn it_cascades_when_the_category_is_deleted() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      upsert_entry_assignment(&db, BudgetOwner::Character(1), 99, cat.id())
        .await
        .unwrap();

      delete_category(&db, cat.id()).await.unwrap();

      assert!(list_entry_assignments(&db).await.unwrap().is_empty());
    }
  }

  mod delete_entry_assignment {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_removes_only_the_matching_assignment() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      upsert_entry_assignment(&db, BudgetOwner::Character(1), 1, cat.id())
        .await
        .unwrap();
      upsert_entry_assignment(&db, BudgetOwner::Character(1), 2, cat.id())
        .await
        .unwrap();

      delete_entry_assignment(&db, BudgetOwner::Character(1), 1)
        .await
        .unwrap();

      let remaining = list_entry_assignments(&db).await.unwrap();

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
      let grp = group(&db, "Bills").await;
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

      let rules = list_rules(&db).await.unwrap();

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
      let grp = group(&db, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      rule(&db, cat.id(), "Second", 1).await;
      rule(&db, cat.id(), "First", 0).await;

      let rules = list_rules(&db).await.unwrap();

      assert_eq!(rules.iter().map(Rule::name).collect::<Vec<_>>(), ["First", "Second"]);
    }
  }

  mod delete_rule {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_cascades_conditions() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, "Bills").await;
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
      assert!(list_rules(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_cascades_when_the_category_is_deleted() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      rule(&db, cat.id(), "Landlord", 0).await;

      delete_category(&db, cat.id()).await.unwrap();

      assert!(list_rules(&db).await.unwrap().is_empty());
    }
  }

  mod replace_rule_conditions {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_replaces_the_whole_condition_set() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, "Bills").await;
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

      let rules = list_rules(&db).await.unwrap();
      assert_eq!(rules[0].conditions(), &replacement);
    }
  }

  mod reorder_rules {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_rewrites_priority_order() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      let a = rule(&db, cat.id(), "A", 0).await;
      let b = rule(&db, cat.id(), "B", 1).await;
      let c = rule(&db, cat.id(), "C", 2).await;

      reorder_rules(&db, &[c.id(), a.id(), b.id()]).await.unwrap();

      let rules = list_rules(&db).await.unwrap();
      assert_eq!(rules.iter().map(Rule::name).collect::<Vec<_>>(), ["C", "A", "B"]);
    }
  }

  mod update_rule {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_changes_the_editable_fields() {
      let db = store::open_test().await.unwrap();
      let grp = group(&db, "Bills").await;
      let cat = category(&db, grp.id(), "Rent").await;
      let other = category(&db, grp.id(), "Power").await;
      let mut created = rule(&db, cat.id(), "Landlord", 0).await;

      created.category_id = other.id();
      created.enabled = false;
      created.match_mode = MatchMode::Any;
      created.name = "Utilities".to_owned();
      update_rule(&db, &created).await.unwrap();

      let rules = list_rules(&db).await.unwrap();

      assert_eq!(rules[0].category_id(), other.id());
      assert!(!rules[0].enabled());
      assert_eq!(rules[0].match_mode(), MatchMode::Any);
      assert_eq!(rules[0].name(), "Utilities");
    }
  }
}
