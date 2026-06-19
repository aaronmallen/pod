use crate::store::{
  Database, Error,
  model::{BudgetAssignment, BudgetCategory, BudgetCategoryGroup, BudgetRefTypeMap, BudgetScope, BudgetTarget},
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

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct TargetInput {
  pub amount: f64,
  pub by_date: Option<String>,
  pub kind: String,
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

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn delete_category(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM budget_categories WHERE id = ?")
    .bind(id)
    .execute(&db.0)
    .await?;
  Ok(())
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn delete_group(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM budget_category_groups WHERE id = ?")
    .bind(id)
    .execute(&db.0)
    .await?;
  Ok(())
}

// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
#[allow(dead_code)]
pub async fn delete_target(db: &Database, category_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM budget_targets WHERE category_id = ?")
    .bind(category_id)
    .execute(&db.0)
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
    .execute(&db.0)
    .await?;
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
  .execute(&db.0)
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
    .execute(&db.0)
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
}
