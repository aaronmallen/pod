// Coverage for the one-time `0104_backfill_split_owner_assignments` migration. The
// migration is the mirror of the runtime reconciler in `budget.rs`; these tests
// prove it heals a pre-existing split mark on upgrade and is a no-op on re-run.
// `open_test` already applies the migration once, so the tests re-execute its exact
// SQL text (via `include_str!`) against a seeded split state to exercise the upgrade
// path and then re-run it to assert idempotence.

#[cfg(test)]
mod tests {
  use crate::store::{
    self, Database,
    model::{
      Alliance, Bloodline, BudgetCategory, BudgetCategoryGroup, BudgetEntryKind, BudgetOwner, BudgetScope, Character,
      Corporation, Gender, Race,
    },
    repo::{
      budget::{NewCategory, NewGroup, create_category, create_group, list_entry_assignments, upsert_entry_assignment},
      character, org,
    },
  };

  const MIGRATION_SQL: &str = include_str!("../../../migrations/0104_backfill_split_owner_assignments.sql");

  async fn apply_migration(db: &Database) {
    sqlx::query(MIGRATION_SQL).execute(db.writer()).await.unwrap();
  }

  async fn seed_owners(db: &Database) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, 1, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut home_corp = Corporation::new(corp_id, "Home Corp", "HOME");
    home_corp.set_ceo_id(1);
    home_corp.set_creator_id(1);
    home_corp.set_member_count(1);
    home_corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(1, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &home_corp, Some(&alliance), None)
      .await
      .unwrap();

    let mut owning_corp = Corporation::new(2, "Owning Corp", "OWN");
    owning_corp.set_ceo_id(1);
    owning_corp.set_creator_id(1);
    owning_corp.set_member_count(1);
    owning_corp.set_tax_rate(0.0);
    org::upsert_corporation(db, &owning_corp).await.unwrap();
  }

  async fn group(db: &Database, name: &str) -> BudgetCategoryGroup {
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
  }

  async fn category(db: &Database, group_id: i64, name: &str) -> BudgetCategory {
    create_category(
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
    .unwrap()
  }

  async fn seed_market(db: &Database, table: &str, owner_col: &str, owner_id: i64, transaction_id: i64, twin: i64) {
    let (extra_col, extra_val) = if owner_col == "character_id" {
      (", is_personal", ", 0")
    } else {
      (", division", ", 1")
    };
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

  fn category_for(
    rows: &[crate::store::model::BudgetEntryAssignment],
    owner: BudgetOwner,
    kind: BudgetEntryKind,
    entry_id: i64,
  ) -> Option<i64> {
    rows
      .iter()
      .find(|a| {
        a.owner_kind() == owner.owner_kind()
          && a.owner_id() == owner.owner_id()
          && a.entry_kind() == kind.as_str()
          && a.entry_id() == entry_id
      })
      .map(crate::store::model::BudgetEntryAssignment::category_id)
  }

  #[tokio::test]
  async fn it_heals_a_pre_existing_split_mark_and_is_a_no_op_on_re_run() {
    use pretty_assertions::assert_eq;

    let db = store::open_test().await.unwrap();
    seed_owners(&db).await;
    let grp = group(&db, "Trade").await;
    let cat = category(&db, grp.id(), "Sales").await;
    // A corp-on-behalf trade mirrored into the fast character wallet (market + journal twin) and the
    // slow corp wallet (market + journal twin + a transaction-tax fee leg). Journal twins and the fee
    // leg link to the trade via context_id == transaction_id.
    seed_market(&db, "character_wallet_transaction", "character_id", 1, 500, 9001).await;
    seed_journal(
      &db,
      "character_wallet_journal",
      "character_id",
      1,
      9001,
      "market_transaction",
      Some(500),
    )
    .await;
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
    // The split: only the fast character market copy carries the mark; the corp-side legs synced later
    // and were left unmarked by the broken cascade.
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

    apply_migration(&db).await;

    let healed = list_entry_assignments(&db, BudgetScope::All).await.unwrap();
    assert_eq!(
      category_for(&healed, BudgetOwner::Corporation(2), BudgetEntryKind::Market, 500),
      Some(cat.id()),
      "the corp market copy is materialized"
    );
    assert_eq!(
      category_for(&healed, BudgetOwner::Corporation(2), BudgetEntryKind::Journal, 9002),
      Some(cat.id()),
      "the corp journal twin is materialized"
    );
    assert_eq!(
      category_for(&healed, BudgetOwner::Corporation(2), BudgetEntryKind::Journal, 9003),
      Some(cat.id()),
      "the corp tax fee leg is materialized"
    );

    apply_migration(&db).await;

    let after_rerun = list_entry_assignments(&db, BudgetScope::All).await.unwrap();
    assert_eq!(
      healed.len(),
      after_rerun.len(),
      "applying the migration twice writes no duplicate rows"
    );
  }

  #[tokio::test]
  async fn it_never_overwrites_an_owner_that_already_holds_its_own_assignment() {
    use pretty_assertions::assert_eq;

    let db = store::open_test().await.unwrap();
    seed_owners(&db).await;
    let grp = group(&db, "Trade").await;
    let pilot_cat = category(&db, grp.id(), "Pilot").await;
    let corp_cat = category(&db, grp.id(), "Corp").await;
    seed_market(&db, "character_wallet_transaction", "character_id", 1, 900, 1).await;
    seed_market(&db, "corporation_wallet_transaction", "corporation_id", 2, 900, 2).await;
    upsert_entry_assignment(
      &db,
      BudgetScope::All,
      BudgetOwner::Character(1),
      BudgetEntryKind::Market,
      900,
      pilot_cat.id(),
    )
    .await
    .unwrap();
    // The corp owner already chose a deliberately different category for its own copy.
    upsert_entry_assignment(
      &db,
      BudgetScope::All,
      BudgetOwner::Corporation(2),
      BudgetEntryKind::Market,
      900,
      corp_cat.id(),
    )
    .await
    .unwrap();

    apply_migration(&db).await;

    let rows = list_entry_assignments(&db, BudgetScope::All).await.unwrap();
    assert_eq!(
      category_for(&rows, BudgetOwner::Corporation(2), BudgetEntryKind::Market, 900),
      Some(corp_cat.id()),
      "the owner's own assignment is preserved, not overwritten by the source mark"
    );
  }
}
