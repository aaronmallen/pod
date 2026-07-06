use sqlx::FromRow;

use crate::store::{Database, Error, model::SkillCompletion};

#[allow(dead_code)]
#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct CalendarEntry {
  pub event_id: i64,
  pub response: String,
  pub timestamp: String,
  pub title: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct CombatKill {
  pub character_id: i64,
  pub is_kill: bool,
  pub kill_time: String,
  pub killmail_id: i64,
  pub ship_type_id: i64,
  pub system_id: i64,
  pub value_isk: f64,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, FromRow, PartialEq)]
pub struct DayMoney {
  pub earned: f64,
  pub spent: f64,
}

#[allow(dead_code)]
impl DayMoney {
  pub fn net(self) -> f64 {
    self.earned - self.spent
  }
}

#[allow(dead_code)]
#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct IndustryDelivery {
  pub character_id: i64,
  pub product_type_id: Option<i64>,
  pub runs: i64,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NetWorthDelta {
  pub isk: f64,
  pub percent: f64,
}

#[allow(dead_code)]
pub async fn active_dates(db: &Database) -> Result<Vec<String>, Error> {
  let rows = sqlx::query_scalar::<_, String>(
    "SELECT day FROM ( \
       SELECT DISTINCT substr(date, 1, 10) AS day FROM character_wallet_journal \
         WHERE character_id IN (SELECT id FROM owned_characters) \
       UNION SELECT DISTINCT substr(kill_time, 1, 10) FROM character_killmails \
         WHERE character_id IN (SELECT id FROM owned_characters) \
       UNION SELECT DISTINCT substr(completed_at, 1, 10) FROM skill_completion \
         WHERE character_id IN (SELECT id FROM owned_characters) \
       UNION SELECT DISTINCT substr(COALESCE(completed_date, end_date), 1, 10) FROM character_industry_jobs \
         WHERE status = 'delivered' AND character_id IN (SELECT id FROM owned_characters) \
       UNION SELECT DISTINCT substr(timestamp, 1, 10) FROM character_calendar \
         WHERE character_id IN (SELECT id FROM owned_characters) \
     ) ORDER BY day DESC",
  )
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn combat(db: &Database, date: &str) -> Result<Vec<CombatKill>, Error> {
  let rows = sqlx::query_as::<_, CombatKill>(
    "SELECT character_id, is_kill, kill_time, killmail_id, ship_type_id, system_id, value_isk \
     FROM character_killmails \
     WHERE substr(kill_time, 1, 10) = ? AND character_id IN (SELECT id FROM owned_characters) \
     ORDER BY kill_time, killmail_id",
  )
  .bind(date)
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn events(db: &Database, date: &str) -> Result<Vec<CalendarEntry>, Error> {
  let rows = sqlx::query_as::<_, CalendarEntry>(
    "SELECT event_id, MIN(response) AS response, MIN(timestamp) AS timestamp, MIN(title) AS title \
     FROM character_calendar \
     WHERE substr(timestamp, 1, 10) = ? AND character_id IN (SELECT id FROM owned_characters) \
     GROUP BY event_id ORDER BY MIN(timestamp), event_id",
  )
  .bind(date)
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn event_owner(db: &Database, event_id: i64) -> Result<Option<i64>, Error> {
  let owner = sqlx::query_scalar::<_, Option<i64>>(
    "SELECT MIN(character_id) FROM character_calendar \
     WHERE event_id = ? AND character_id IN (SELECT id FROM owned_characters)",
  )
  .bind(event_id)
  .fetch_optional(db.reader())
  .await?
  .flatten();
  Ok(owner)
}

#[allow(dead_code)]
pub async fn has_activity(db: &Database, date: &str) -> Result<bool, Error> {
  let found = sqlx::query_scalar::<_, i64>(
    "SELECT EXISTS( \
       SELECT 1 FROM character_wallet_journal \
         WHERE substr(date, 1, 10) = ? AND character_id IN (SELECT id FROM owned_characters) \
       UNION ALL SELECT 1 FROM character_killmails \
         WHERE substr(kill_time, 1, 10) = ? AND character_id IN (SELECT id FROM owned_characters) \
       UNION ALL SELECT 1 FROM skill_completion \
         WHERE substr(completed_at, 1, 10) = ? AND character_id IN (SELECT id FROM owned_characters) \
       UNION ALL SELECT 1 FROM character_industry_jobs \
         WHERE status = 'delivered' AND substr(COALESCE(completed_date, end_date), 1, 10) = ? \
           AND character_id IN (SELECT id FROM owned_characters) \
       UNION ALL SELECT 1 FROM character_calendar \
         WHERE substr(timestamp, 1, 10) = ? AND character_id IN (SELECT id FROM owned_characters) \
     )",
  )
  .bind(date)
  .bind(date)
  .bind(date)
  .bind(date)
  .bind(date)
  .fetch_one(db.reader())
  .await?;
  Ok(found != 0)
}

#[allow(dead_code)]
pub async fn industry(db: &Database, date: &str) -> Result<Vec<IndustryDelivery>, Error> {
  let rows = sqlx::query_as::<_, IndustryDelivery>(
    "SELECT character_id, product_type_id, runs \
     FROM character_industry_jobs \
     WHERE status = 'delivered' AND substr(COALESCE(completed_date, end_date), 1, 10) = ? \
       AND character_id IN (SELECT id FROM owned_characters) \
     ORDER BY character_id, job_id",
  )
  .bind(date)
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn money(db: &Database, date: &str) -> Result<DayMoney, Error> {
  let money = sqlx::query_as::<_, DayMoney>(
    "SELECT \
       COALESCE(SUM(CASE WHEN amount > 0 THEN amount ELSE 0.0 END), 0.0) AS earned, \
       COALESCE(SUM(CASE WHEN amount < 0 THEN -amount ELSE 0.0 END), 0.0) AS spent \
     FROM character_wallet_journal \
     WHERE substr(date, 1, 10) = ? AND character_id IN (SELECT id FROM owned_characters)",
  )
  .bind(date)
  .fetch_one(db.reader())
  .await?;
  Ok(money)
}

#[allow(dead_code)]
pub async fn net_worth_delta(db: &Database, date: &str) -> Result<Option<NetWorthDelta>, Error> {
  let current = combined_net_worth_on(db, date).await?;
  let prior = combined_net_worth_before(db, date).await?;

  Ok(match (current, prior) {
    (Some(now), Some(before)) => Some(NetWorthDelta {
      isk: now - before,
      percent: if before != 0.0 {
        (now - before) / before * 100.0
      } else {
        0.0
      },
    }),
    _ => None,
  })
}

#[allow(dead_code)]
pub async fn skills(db: &Database, date: &str) -> Result<Vec<SkillCompletion>, Error> {
  let rows = sqlx::query_as::<_, SkillCompletion>(
    "SELECT character_id, completed_at, created_at, id, level, skill_id, updated_at, verified \
     FROM skill_completion \
     WHERE substr(completed_at, 1, 10) = ? AND character_id IN (SELECT id FROM owned_characters) \
     ORDER BY character_id, completed_at, skill_id, level",
  )
  .bind(date)
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

async fn combined_net_worth_before(db: &Database, date: &str) -> Result<Option<f64>, Error> {
  let value = sqlx::query_scalar::<_, Option<f64>>(
    "SELECT net_worth FROM character_net_worth_snapshot_combined WHERE date < ? ORDER BY date DESC LIMIT 1",
  )
  .bind(date)
  .fetch_optional(db.reader())
  .await?;
  Ok(value.flatten())
}

async fn combined_net_worth_on(db: &Database, date: &str) -> Result<Option<f64>, Error> {
  let value = sqlx::query_scalar::<_, Option<f64>>(
    "SELECT net_worth FROM character_net_worth_snapshot_combined WHERE date = ? LIMIT 1",
  )
  .bind(date)
  .fetch_optional(db.reader())
  .await?;
  Ok(value.flatten())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
    repo::{character, infra, skill_completion},
  };

  const OTHER: i64 = 90_000_002;
  const PILOT: i64 = 90_000_001;
  const UNOWNED: i64 = 90_000_003;

  async fn seed_owned(db: &Database, id: i64) {
    seed_character(db, id).await;
    infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
      .await
      .unwrap();
  }

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 98_000_001;
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
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  async fn seed_journal(db: &Database, id: i64, character_id: i64, date: &str, amount: f64) {
    sqlx::query(
      "INSERT INTO character_wallet_journal (id, character_id, date, description, ref_type, amount) \
       VALUES (?, ?, ?, '', 'player_trading', ?)",
    )
    .bind(id)
    .bind(character_id)
    .bind(date)
    .bind(amount)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_kill(db: &Database, character_id: i64, killmail_id: i64, is_kill: bool, kill_time: &str, value: f64) {
    sqlx::query(
      "INSERT INTO character_killmails \
         (character_id, killmail_id, kill_hash, is_kill, ship_type_id, system_id, value_isk, kill_time, synced_at) \
       VALUES (?, ?, 'hash', ?, 670, 30000142, ?, ?, '2026-07-05T00:00:00Z')",
    )
    .bind(character_id)
    .bind(killmail_id)
    .bind(is_kill)
    .bind(value)
    .bind(kill_time)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_industry(db: &Database, job_id: i64, character_id: i64, product: i64, runs: i64, completed: &str) {
    sqlx::query(
      "INSERT INTO character_industry_jobs \
         (job_id, character_id, activity_id, blueprint_id, blueprint_location_id, blueprint_type_id, \
          duration, end_date, facility_id, installer_id, output_location_id, product_type_id, runs, \
          start_date, status, completed_date) \
       VALUES (?, ?, 1, 1, 1, 1, 0, ?, 1, ?, 1, ?, ?, '2026-07-05T00:00:00Z', 'delivered', ?)",
    )
    .bind(job_id)
    .bind(character_id)
    .bind(completed)
    .bind(character_id)
    .bind(product)
    .bind(runs)
    .bind(completed)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_calendar(db: &Database, character_id: i64, event_id: i64, timestamp: &str, title: &str) {
    sqlx::query(
      "INSERT INTO character_calendar \
         (character_id, event_id, timestamp, owner_id, owner_name, owner_type, response, title, fetched_at) \
       VALUES (?, ?, ?, 1, 'Corp', 'corporation', 'accepted', ?, '2026-07-05T00:00:00Z')",
    )
    .bind(character_id)
    .bind(event_id)
    .bind(timestamp)
    .bind(title)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_snapshot(db: &Database, character_id: i64, date: &str, net_worth: f64) {
    sqlx::query("INSERT INTO character_net_worth_snapshot (character_id, date, liquid, net_worth) VALUES (?, ?, ?, ?)")
      .bind(character_id)
      .bind(date)
      .bind(net_worth)
      .bind(net_worth)
      .execute(db.writer())
      .await
      .unwrap();
  }

  mod money {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_splits_income_and_spend_across_the_roster() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT).await;
      seed_owned(&db, OTHER).await;
      seed_journal(&db, 1, PILOT, "2026-07-05T10:00:00Z", 1_000.0).await;
      seed_journal(&db, 2, PILOT, "2026-07-05T11:00:00Z", -400.0).await;
      seed_journal(&db, 3, OTHER, "2026-07-05T12:00:00Z", 250.0).await;
      seed_journal(&db, 4, PILOT, "2026-07-04T12:00:00Z", 9_999.0).await;

      let money = super::super::money(&db, "2026-07-05").await.unwrap();

      assert_eq!(money.earned, 1_250.0);
      assert_eq!(money.spent, 400.0);
      assert_eq!(money.net(), 850.0);
    }

    #[tokio::test]
    async fn it_excludes_unowned_characters() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT).await;
      seed_character(&db, UNOWNED).await;
      seed_journal(&db, 1, PILOT, "2026-07-05T10:00:00Z", 100.0).await;
      seed_journal(&db, 2, UNOWNED, "2026-07-05T10:00:00Z", 5_000.0).await;

      let money = super::super::money(&db, "2026-07-05").await.unwrap();

      assert_eq!(money.earned, 100.0);
    }
  }

  mod combat {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_lists_engagements_ordered_by_kill_time() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT).await;
      seed_owned(&db, OTHER).await;
      seed_kill(&db, PILOT, 100, true, "2026-07-05T21:00:00Z", 612_000_000.0).await;
      seed_kill(&db, OTHER, 101, false, "2026-07-05T20:00:00Z", 132_000_000.0).await;
      seed_kill(&db, PILOT, 102, true, "2026-07-04T20:00:00Z", 1.0).await;

      let rows = super::super::combat(&db, "2026-07-05").await.unwrap();

      assert_eq!(rows.len(), 2);
      assert_eq!(rows[0].killmail_id, 101);
      assert!(!rows[0].is_kill);
      assert_eq!(rows[1].killmail_id, 100);
    }
  }

  mod net_worth_delta {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_diffs_the_combined_series_against_the_prior_day() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT).await;
      seed_owned(&db, OTHER).await;
      seed_snapshot(&db, PILOT, "2026-07-04", 600.0).await;
      seed_snapshot(&db, OTHER, "2026-07-04", 400.0).await;
      seed_snapshot(&db, PILOT, "2026-07-05", 700.0).await;
      seed_snapshot(&db, OTHER, "2026-07-05", 324.0).await;

      let delta = super::super::net_worth_delta(&db, "2026-07-05").await.unwrap().unwrap();

      assert_eq!(delta.isk, 24.0);
      assert!((delta.percent - 2.4).abs() < 1e-9);
    }

    #[tokio::test]
    async fn it_reports_none_without_a_prior_snapshot() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT).await;
      seed_snapshot(&db, PILOT, "2026-07-05", 700.0).await;

      let delta = super::super::net_worth_delta(&db, "2026-07-05").await.unwrap();

      assert_eq!(delta, None);
    }
  }

  mod progression {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_collects_industry_and_events_for_the_day() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT).await;
      seed_owned(&db, OTHER).await;
      seed_industry(&db, 1, PILOT, 22_544, 3, "2026-07-05T18:00:00Z").await;
      seed_industry(&db, 2, OTHER, 20_183, 1, "2026-07-04T18:00:00Z").await;
      seed_calendar(&db, PILOT, 5, "2026-07-05T20:00:00Z", "Tama roam").await;
      seed_calendar(&db, OTHER, 5, "2026-07-05T20:00:00Z", "Tama roam").await;

      let industry = super::super::industry(&db, "2026-07-05").await.unwrap();
      let events = super::super::events(&db, "2026-07-05").await.unwrap();

      assert_eq!(industry.len(), 1);
      assert_eq!(industry[0].runs, 3);
      assert_eq!(events.len(), 1, "a shared calendar event is deduped across the roster");
      assert_eq!(events[0].title, "Tama roam");
    }
  }

  mod skills {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_lists_completions_across_the_roster_for_the_day() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT).await;
      seed_owned(&db, OTHER).await;
      skill_completion::insert_if_absent(&db, PILOT, 3300, 5, "2026-07-05T08:00:00Z")
        .await
        .unwrap();
      skill_completion::insert_if_absent(&db, OTHER, 3301, 4, "2026-07-05T20:00:00Z")
        .await
        .unwrap();
      skill_completion::insert_if_absent(&db, PILOT, 3302, 3, "2026-07-04T23:59:00Z")
        .await
        .unwrap();

      let rows = super::super::skills(&db, "2026-07-05").await.unwrap();

      assert_eq!(rows.len(), 2);
      assert_eq!(rows[0].character_id, PILOT);
      assert_eq!(rows[1].character_id, OTHER);
    }
  }

  mod activity {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_reports_active_dates_across_every_source() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT).await;
      seed_journal(&db, 1, PILOT, "2026-07-05T10:00:00Z", 100.0).await;
      seed_kill(&db, PILOT, 100, true, "2026-07-03T21:00:00Z", 1.0).await;
      skill_completion::insert_if_absent(&db, PILOT, 3300, 5, "2026-07-04T08:00:00Z")
        .await
        .unwrap();

      let dates = super::super::active_dates(&db).await.unwrap();

      assert_eq!(dates, vec!["2026-07-05", "2026-07-04", "2026-07-03"]);
    }

    #[tokio::test]
    async fn it_flags_a_day_with_any_activity() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT).await;
      seed_industry(&db, 1, PILOT, 22_544, 1, "2026-07-05T18:00:00Z").await;

      assert!(super::super::has_activity(&db, "2026-07-05").await.unwrap());
    }

    #[tokio::test]
    async fn it_reports_no_activity_for_an_empty_day() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT).await;

      assert!(!super::super::has_activity(&db, "2026-07-05").await.unwrap());
      assert!(super::super::active_dates(&db).await.unwrap().is_empty());
    }
  }
}
