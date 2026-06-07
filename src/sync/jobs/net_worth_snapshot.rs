use chrono::Utc;

use crate::{
  clients::Error,
  store::{
    model::OwnerType,
    repo::{finance, infra, org},
  },
  sync::{job::JobCtx, outcome::Outcome},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let date = today_utc();
  let mut rows_touched = 0usize;
  for character_id in owned_character_ids(ctx).await? {
    if let Some(financials) = finance::financials_get(ctx.db, character_id).await?
      && let Some(net_worth) = financials.net_worth
    {
      finance::upsert(
        ctx.db,
        character_id,
        &date,
        financials.liquid.unwrap_or(0.0),
        financials.asset_value,
        financials.escrow,
        net_worth,
      )
      .await?;
      rows_touched += 1;
    }
    finance::backfill_liquid_from_journal(ctx.db, character_id).await?;
  }
  for corporation_id in owned_corporation_ids(ctx).await? {
    finance::corporation_backfill_liquid_from_journal(ctx.db, corporation_id).await?;
    finance::record_today(ctx.db, corporation_id, &date).await?;
    rows_touched += 1;
  }
  Ok(Outcome::from_rows(rows_touched))
}

async fn owned_character_ids(ctx: &JobCtx<'_>) -> Result<Vec<i64>, Error> {
  let owned = infra::all(ctx.db)
    .await?
    .into_iter()
    .filter(|credential| credential.owner_type() == OwnerType::Character)
    .map(|credential| credential.owner_id())
    .collect();
  Ok(owned)
}

async fn owned_corporation_ids(ctx: &JobCtx<'_>) -> Result<Vec<i64>, Error> {
  let owned = org::all_owned_corporations(ctx.db)
    .await?
    .into_iter()
    .map(|corporation| corporation.id())
    .collect();
  Ok(owned)
}

fn today_utc() -> String {
  Utc::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    clients::{esi, eve_image, http},
    store::{
      self, Database, images,
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::{character::insert_with_org, finance},
    },
    sync::{
      job::{JobKey, JobKind},
      subject::Subject,
    },
  };

  async fn seed_character(db: &Database, id: i64) {
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

  async fn own(db: &Database, id: i64) {
    infra::upsert(db, id, OwnerType::Character, "tok", "rt", 4_102_444_800, None, None)
      .await
      .unwrap();
  }

  async fn own_corp(db: &Database, id: i64) {
    infra::upsert(db, id, OwnerType::Corporation, "tok", "rt", 4_102_444_800, None, None)
      .await
      .unwrap();
  }

  async fn insert_corp_journal(db: &Database, id: i64, corporation_id: i64, division: i64, date: &str, balance: f64) {
    sqlx::query("INSERT INTO corporation_wallet_journal (id, corporation_id, division, date, description, ref_type, amount, balance) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
      .bind(id)
      .bind(corporation_id)
      .bind(division)
      .bind(date)
      .bind("Test")
      .bind("test")
      .bind(1.0)
      .bind(balance)
      .execute(&db.0)
      .await
      .unwrap();
  }

  async fn insert_corp_division(db: &Database, corporation_id: i64, division: i64, balance: f64) {
    sqlx::query(
      "INSERT INTO corporation_wallet_division (corporation_id, division, name, balance) VALUES (?, ?, ?, ?)",
    )
    .bind(corporation_id)
    .bind(division)
    .bind("Master")
    .bind(balance)
    .execute(&db.0)
    .await
    .unwrap();
  }

  async fn insert_journal(db: &Database, id: i64, character_id: i64, amount: f64, balance: f64) {
    sqlx::query("INSERT INTO character_wallet_journal (id, character_id, date, description, ref_type, amount, balance) VALUES (?, ?, ?, ?, ?, ?, ?)")
      .bind(id)
      .bind(character_id)
      .bind("2026-01-01")
      .bind("Test")
      .bind("test")
      .bind(amount)
      .bind(balance)
      .execute(&db.0)
      .await
      .unwrap();
  }

  async fn insert_journal_on(db: &Database, id: i64, character_id: i64, date: &str, balance: f64) {
    sqlx::query("INSERT INTO character_wallet_journal (id, character_id, date, description, ref_type, amount, balance) VALUES (?, ?, ?, ?, ?, ?, ?)")
      .bind(id)
      .bind(character_id)
      .bind(date)
      .bind("Test")
      .bind("test")
      .bind(1.0)
      .bind(balance)
      .execute(&db.0)
      .await
      .unwrap();
  }

  async fn insert_order(db: &Database, order_id: i64, character_id: i64, escrow: f64) {
    sqlx::query(
      "INSERT INTO market_orders \
      (order_id, character_id, type_id, region_id, location_id, is_buy_order, price, volume_remain, volume_total, escrow, range, duration, issued, state) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(order_id)
    .bind(character_id)
    .bind(34)
    .bind(10_000_002)
    .bind(60_003_760)
    .bind(1)
    .bind(5.0)
    .bind(10)
    .bind(10)
    .bind(escrow)
    .bind("station")
    .bind(90)
    .bind("2026-01-01T00:00:00Z")
    .bind("open")
    .execute(&db.0)
    .await
    .unwrap();
  }

  fn ctx<'a>(
    db: &'a Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::NetWorthSnapshot, Subject::Character(0)),
      grant: None,
    }
  }

  async fn run_in(db: &Database) {
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = esi::Client::with_base_url(http.clone(), "http://localhost".to_owned());
    let image = eve_image::Client::with_base_url(http, "http://localhost".to_owned());
    let images_dir = tempfile::tempdir().unwrap();
    let image_store = images::Store::new(images_dir.path().to_path_buf());
    let ctx = ctx(db, &esi, &image, &image_store);
    run(&ctx).await.unwrap();
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_writes_one_row_per_owned_character_for_todays_utc_date() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      own(&db, 1).await;
      insert_journal(&db, 1, 1, 100.0, 100.0).await;

      run_in(&db).await;

      let date = today_utc();
      let rows = finance::for_character_since(&db, 1, &date).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].date(), &date);
      assert_eq!(rows[0].liquid(), 100.0);
      assert_eq!(rows[0].net_worth(), 100.0);
      assert_eq!(rows[0].asset_value(), None);
      assert_eq!(rows[0].escrow(), None);
    }

    #[tokio::test]
    async fn it_defaults_liquid_to_zero_when_only_escrow_is_synced() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      own(&db, 1).await;
      insert_order(&db, 1, 1, 80.0).await;

      run_in(&db).await;

      let date = today_utc();
      let rows = finance::for_character_since(&db, 1, &date).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].liquid(), 0.0);
      assert_eq!(rows[0].escrow(), Some(80.0));
      assert_eq!(rows[0].net_worth(), 80.0);
    }

    #[tokio::test]
    async fn it_skips_a_fully_unsynced_owned_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      own(&db, 1).await;

      run_in(&db).await;

      let rows = finance::for_character_since(&db, 1, "2000-01-01").await.unwrap();
      assert!(
        rows.is_empty(),
        "a character with no synced figures has nothing to snapshot"
      );
    }

    #[tokio::test]
    async fn it_skips_a_non_owned_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_journal(&db, 1, 1, 500.0, 500.0).await;

      run_in(&db).await;

      let rows = finance::for_character_since(&db, 1, "2000-01-01").await.unwrap();
      assert!(rows.is_empty(), "only owned (credentialed) characters are snapshotted");
    }

    #[tokio::test]
    async fn it_backfills_historical_liquid_only_points_from_journal_history() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      own(&db, 1).await;
      insert_journal_on(&db, 1, 1, "2026-05-01T03:00:00Z", 100.0).await;
      insert_journal_on(&db, 2, 1, "2026-05-02T03:00:00Z", 250.0).await;

      run_in(&db).await;

      let rows = finance::for_character_since(&db, 1, "2026-01-01").await.unwrap();
      assert!(rows.len() >= 2, "journal history must produce a multi-point curve");
      let may_first = rows.iter().find(|r| r.date() == "2026-05-01").unwrap();
      assert_eq!(may_first.liquid(), 100.0);
      assert_eq!(may_first.net_worth(), 100.0);
      assert_eq!(may_first.asset_value(), None);
      assert_eq!(may_first.escrow(), None);
    }

    #[tokio::test]
    async fn it_keeps_todays_full_composition_when_backfilling() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      own(&db, 1).await;
      let date = today_utc();
      insert_journal_on(&db, 1, 1, &format!("{date}T03:00:00Z"), 100.0).await;
      insert_order(&db, 1, 1, 80.0).await;

      run_in(&db).await;

      let rows = finance::for_character_since(&db, 1, &date).await.unwrap();
      let today = rows.iter().find(|r| r.date() == &date).unwrap();
      assert_eq!(today.escrow(), Some(80.0), "forward escrow composition is preserved");
      assert_eq!(today.liquid(), 100.0);
      assert_eq!(today.net_worth(), 180.0);
    }

    #[tokio::test]
    async fn it_is_idempotent_on_same_day_re_run() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      own(&db, 1).await;
      insert_journal(&db, 1, 1, 100.0, 100.0).await;

      run_in(&db).await;
      insert_journal(&db, 2, 1, 50.0, 150.0).await;
      run_in(&db).await;

      let rows = finance::for_character_since(&db, 1, "2000-01-01").await.unwrap();
      assert_eq!(
        rows.len(),
        2,
        "re-running overwrites each UTC day's row rather than appending"
      );

      let today = rows.iter().find(|r| r.date() == &today_utc()).unwrap();
      assert_eq!(today.liquid(), 150.0);
      assert_eq!(today.net_worth(), 150.0);
    }

    #[tokio::test]
    async fn it_backfills_and_records_todays_corporation_net_worth() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      own_corp(&db, 90_000_001).await;
      insert_corp_journal(&db, 1, 90_000_001, 1, "2026-05-01T03:00:00Z", 400.0).await;
      insert_corp_journal(&db, 2, 90_000_001, 2, "2026-05-01T03:00:00Z", 100.0).await;
      insert_corp_division(&db, 90_000_001, 1, 1_000.0).await;
      insert_corp_division(&db, 90_000_001, 2, 250.0).await;

      run_in(&db).await;

      let rows = finance::for_corporation_since(&db, 90_000_001, "2026-01-01")
        .await
        .unwrap();
      assert!(rows.len() >= 2, "corp journal history must produce a multi-point curve");

      let backfilled = rows.iter().find(|r| r.date() == "2026-05-01").unwrap();
      assert_eq!(backfilled.liquid(), 500.0);
      assert_eq!(backfilled.net_worth(), 500.0);

      let today = rows.iter().find(|r| r.date() == &today_utc()).unwrap();
      assert_eq!(today.liquid(), 1_250.0);
    }

    #[tokio::test]
    async fn it_skips_a_non_owned_corporation() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      insert_corp_journal(&db, 1, 90_000_001, 1, "2026-05-01T03:00:00Z", 400.0).await;

      run_in(&db).await;

      let rows = finance::for_corporation_since(&db, 90_000_001, "2000-01-01")
        .await
        .unwrap();
      assert!(rows.is_empty(), "only owned corporations are snapshotted");
    }
  }
}
