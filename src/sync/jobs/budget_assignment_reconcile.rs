use crate::{
  clients::Error,
  store::repo::budget,
  sync::{job::JobCtx, outcome::Outcome},
};

// Post-sync cross-owner budget mark reconciliation. A corp-on-behalf trade lands in the fast
// character wallet and the slow corp journal at very different times; the mark-time cascade only
// fills copies already in memory, so legs that sync later land unmarked. This global job re-runs the
// cross-owner fill after every wallet sync (chained off CharacterWallet | CorporationWallet), then
// GCs any copy whose wallet row has since disappeared, keeping the assignment table self-healing.
pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let filled = budget::reconcile_split_owner_assignments(ctx.db).await?;
  let pruned = budget::prune_orphan_entry_assignments(ctx.db).await?;
  let touched = usize::try_from(filled + pruned).unwrap_or(usize::MAX);
  Ok(Outcome::from_rows(touched))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    clients::{esi, eve_image, http},
    store::{
      self, Database, images,
      model::{
        Alliance, Bloodline, BudgetCategory, BudgetCategoryGroup, BudgetEntryKind, BudgetOwner, BudgetScope, Character,
        Corporation, Gender, Race,
      },
      repo::{
        budget::{
          NewCategory, NewGroup, create_category, create_group, list_entry_assignments, upsert_entry_assignment,
        },
        character, org,
      },
    },
    sync::{
      job::{JobCtx, JobKey, JobKind},
      subject::Subject,
    },
  };

  // Wallet rows carry an owner FK to characters / corporations (enforced in the test DB), so the
  // owning character (id 1) and corporation (id 2) must exist before any leg is seeded.
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
    // A character transaction carries an `is_personal` flag; a corporation one a `division` — both
    // NOT NULL, so the column the owner kind requires is appended with a 0 value.
    let (extra_col, extra_val) = if owner_col == "character_id" {
      (", is_personal", ", 0")
    } else {
      (", division", ", 1")
    };
    // The table / column names come from a closed set of literals in this test module, never caller
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

  async fn run_job(db: &Database) -> Outcome {
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = esi::Client::with_base_url(http.clone(), "http://localhost".to_owned());
    let image = eve_image::Client::with_base_url(http, "http://localhost".to_owned());
    let images_dir = tempfile::tempdir().unwrap();
    let image_store = images::Store::new(images_dir.path().to_path_buf());
    let ctx = JobCtx {
      db,
      esi: &esi,
      image: &image,
      image_store: &image_store,
      key: JobKey::new(JobKind::BudgetAssignmentReconcile, Subject::Character(0)),
      grant: None,
      sso: None,
    };
    run(&ctx).await.unwrap()
  }

  fn category_for(
    db_rows: &[crate::store::model::BudgetEntryAssignment],
    owner: BudgetOwner,
    kind: BudgetEntryKind,
    entry_id: i64,
  ) -> Option<i64> {
    db_rows
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
  async fn it_materializes_the_corp_market_copy_and_journal_leg_after_a_late_corp_sync() {
    let db = store::open_test().await.unwrap();
    seed_owners(&db).await;
    let grp = group(&db, "Trade").await;
    let cat = category(&db, grp.id(), "Sales").await;
    // The same trade mirrored into a character (fast) and a corporation (slow). Twin journal ids and a
    // tax fee leg link via context_id == transaction_id.
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
    // The pilot marks only the fast character market copy.
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

    run_job(&db).await;

    let rows = list_entry_assignments(&db, BudgetScope::All).await.unwrap();
    assert_eq!(
      category_for(&rows, BudgetOwner::Corporation(2), BudgetEntryKind::Market, 500),
      Some(cat.id())
    );
    assert_eq!(
      category_for(&rows, BudgetOwner::Corporation(2), BudgetEntryKind::Journal, 9002),
      Some(cat.id())
    );
    assert_eq!(
      category_for(&rows, BudgetOwner::Corporation(2), BudgetEntryKind::Journal, 9003),
      Some(cat.id())
    );
  }

  #[tokio::test]
  async fn it_reconciles_a_fee_leg_linked_only_by_journal_context_id() {
    let db = store::open_test().await.unwrap();
    seed_owners(&db).await;
    let grp = group(&db, "Trade").await;
    let cat = category(&db, grp.id(), "Sales").await;
    seed_market(&db, "character_wallet_transaction", "character_id", 1, 700, 8001).await;
    seed_market(&db, "corporation_wallet_transaction", "corporation_id", 2, 700, 8002).await;
    // A broker fee leg that exists ONLY as a journal row keyed by context_id — no market row of its
    // own. This proves the journal-id linkage path, not just transaction_id.
    seed_journal(
      &db,
      "corporation_wallet_journal",
      "corporation_id",
      2,
      8003,
      "brokers_fee",
      Some(700),
    )
    .await;
    upsert_entry_assignment(
      &db,
      BudgetScope::All,
      BudgetOwner::Character(1),
      BudgetEntryKind::Market,
      700,
      cat.id(),
    )
    .await
    .unwrap();

    run_job(&db).await;

    let rows = list_entry_assignments(&db, BudgetScope::All).await.unwrap();
    assert_eq!(
      category_for(&rows, BudgetOwner::Corporation(2), BudgetEntryKind::Journal, 8003),
      Some(cat.id())
    );
  }

  #[tokio::test]
  async fn it_never_overwrites_an_owner_that_already_holds_its_own_assignment() {
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
    // The corp owner already chose a deliberately different category.
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

    run_job(&db).await;

    let rows = list_entry_assignments(&db, BudgetScope::All).await.unwrap();
    assert_eq!(
      category_for(&rows, BudgetOwner::Corporation(2), BudgetEntryKind::Market, 900),
      Some(corp_cat.id())
    );
  }

  #[tokio::test]
  async fn it_does_not_resurrect_a_deliberately_unassigned_mark() {
    let db = store::open_test().await.unwrap();
    seed_owners(&db).await;
    let grp = group(&db, "Trade").await;
    let cat = category(&db, grp.id(), "Sales").await;
    seed_market(&db, "character_wallet_transaction", "character_id", 1, 111, 1).await;
    seed_market(&db, "corporation_wallet_transaction", "corporation_id", 2, 111, 2).await;
    // The pilot marks the character copy, then unassigns it before the corp wallet syncs. With no
    // remaining source assignment in the group, the late corp sync must NOT resurrect the mark.
    upsert_entry_assignment(
      &db,
      BudgetScope::All,
      BudgetOwner::Character(1),
      BudgetEntryKind::Market,
      111,
      cat.id(),
    )
    .await
    .unwrap();
    crate::store::repo::budget::delete_entry_assignment(
      &db,
      BudgetScope::All,
      BudgetOwner::Character(1),
      BudgetEntryKind::Market,
      111,
    )
    .await
    .unwrap();

    run_job(&db).await;

    let rows = list_entry_assignments(&db, BudgetScope::All).await.unwrap();
    assert!(
      rows.is_empty(),
      "no copy should be materialized once the only mark is gone, got {rows:?}"
    );
  }

  #[tokio::test]
  async fn it_is_idempotent_across_repeated_runs() {
    let db = store::open_test().await.unwrap();
    seed_owners(&db).await;
    let grp = group(&db, "Trade").await;
    let cat = category(&db, grp.id(), "Sales").await;
    seed_market(&db, "character_wallet_transaction", "character_id", 1, 222, 1).await;
    seed_market(&db, "corporation_wallet_transaction", "corporation_id", 2, 222, 2).await;
    upsert_entry_assignment(
      &db,
      BudgetScope::All,
      BudgetOwner::Character(1),
      BudgetEntryKind::Market,
      222,
      cat.id(),
    )
    .await
    .unwrap();

    run_job(&db).await;
    let after_first = list_entry_assignments(&db, BudgetScope::All).await.unwrap();
    run_job(&db).await;
    let after_second = list_entry_assignments(&db, BudgetScope::All).await.unwrap();

    assert_eq!(after_first.len(), 2, "the corp copy is filled exactly once");
    assert_eq!(
      after_first.len(),
      after_second.len(),
      "a second pass writes no duplicate rows"
    );
  }

  #[tokio::test]
  async fn it_gcs_an_orphan_copy_whose_wallet_row_has_disappeared() {
    let db = store::open_test().await.unwrap();
    let grp = group(&db, "Trade").await;
    let cat = category(&db, grp.id(), "Sales").await;
    // An assignment whose entry_id resolves to no live wallet row for its owner: the prune step in the
    // same job must collect it.
    upsert_entry_assignment(
      &db,
      BudgetScope::All,
      BudgetOwner::Character(1),
      BudgetEntryKind::Market,
      333,
      cat.id(),
    )
    .await
    .unwrap();

    run_job(&db).await;

    assert!(list_entry_assignments(&db, BudgetScope::All).await.unwrap().is_empty());
  }
}
