use crate::store::{
  Database, Error,
  model::SkillCompletion,
  repo::captains_log_rollup::{self, CalendarEntry, CombatKill, DayMoney, IndustryDelivery, NetWorthDelta},
};

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Combat {
  pub engagements: Vec<CombatKill>,
  pub kill_count: usize,
  pub kill_value: f64,
  pub loss_count: usize,
  pub loss_value: f64,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct DayRollup {
  pub combat: Combat,
  pub date: String,
  pub events: Vec<CalendarEntry>,
  pub industry: Vec<IndustryDelivery>,
  pub money: DayMoney,
  pub net_worth: Option<NetWorthDelta>,
  pub skills: Vec<SkillCompletion>,
}

#[allow(dead_code)]
pub async fn active_dates(db: &Database) -> Result<Vec<String>, Error> {
  captains_log_rollup::active_dates(db).await
}

#[allow(dead_code)]
pub async fn for_date(db: &Database, date: &str) -> Result<DayRollup, Error> {
  let money = captains_log_rollup::money(db, date).await?;
  let net_worth = captains_log_rollup::net_worth_delta(db, date).await?;
  let combat = summarize_combat(captains_log_rollup::combat(db, date).await?);
  let industry = captains_log_rollup::industry(db, date).await?;
  let events = captains_log_rollup::events(db, date).await?;
  let skills = captains_log_rollup::skills(db, date).await?;

  Ok(DayRollup {
    combat,
    date: date.to_owned(),
    events,
    industry,
    money,
    net_worth,
    skills,
  })
}

#[allow(dead_code)]
pub async fn has_activity(db: &Database, date: &str) -> Result<bool, Error> {
  captains_log_rollup::has_activity(db, date).await
}

fn summarize_combat(engagements: Vec<CombatKill>) -> Combat {
  let mut kill_count = 0;
  let mut kill_value = 0.0;
  let mut loss_count = 0;
  let mut loss_value = 0.0;

  for engagement in &engagements {
    if engagement.is_kill {
      kill_count += 1;
      kill_value += engagement.value_isk;
    } else {
      loss_count += 1;
      loss_value += engagement.value_isk;
    }
  }

  Combat {
    engagements,
    kill_count,
    kill_value,
    loss_count,
    loss_value,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
    repo::{character, infra},
  };

  const OTHER: i64 = 90_000_002;
  const PILOT: i64 = 90_000_001;

  async fn seed_owned(db: &Database, id: i64) {
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
    infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
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

  #[test]
  fn it_summarizes_kills_and_losses_split_by_is_kill() {
    let engagements = vec![
      CombatKill {
        character_id: PILOT,
        is_kill: true,
        kill_time: "2026-07-05T21:00:00Z".to_owned(),
        killmail_id: 100,
        ship_type_id: 670,
        system_id: 30_000_142,
        value_isk: 612_000_000.0,
      },
      CombatKill {
        character_id: OTHER,
        is_kill: false,
        kill_time: "2026-07-05T20:00:00Z".to_owned(),
        killmail_id: 101,
        ship_type_id: 670,
        system_id: 30_000_142,
        value_isk: 132_000_000.0,
      },
    ];

    let combat = summarize_combat(engagements);

    assert_eq!(combat.kill_count, 1);
    assert_eq!(combat.loss_count, 1);
    assert_eq!(combat.kill_value, 612_000_000.0);
    assert_eq!(combat.loss_value, 132_000_000.0);
    assert_eq!(combat.engagements.len(), 2);
  }

  #[tokio::test]
  async fn it_assembles_a_day_rollup_across_the_roster() {
    let db = store::open_test().await.unwrap();
    seed_owned(&db, PILOT).await;
    seed_owned(&db, OTHER).await;
    seed_journal(&db, 1, PILOT, "2026-07-05T10:00:00Z", 1_000.0).await;
    seed_journal(&db, 2, OTHER, "2026-07-05T11:00:00Z", -300.0).await;
    seed_kill(&db, PILOT, 100, true, "2026-07-05T21:00:00Z", 50.0).await;
    seed_kill(&db, OTHER, 101, false, "2026-07-05T20:00:00Z", 10.0).await;

    let rollup = for_date(&db, "2026-07-05").await.unwrap();

    assert_eq!(rollup.date, "2026-07-05");
    assert_eq!(rollup.money.net(), 700.0);
    assert_eq!(rollup.combat.kill_count, 1);
    assert_eq!(rollup.combat.loss_count, 1);
    assert_eq!(rollup.net_worth, None);
  }

  #[tokio::test]
  async fn it_reports_an_empty_day_as_inactive() {
    let db = store::open_test().await.unwrap();
    seed_owned(&db, PILOT).await;

    assert!(!has_activity(&db, "2026-07-05").await.unwrap());

    let rollup = for_date(&db, "2026-07-05").await.unwrap();

    assert_eq!(rollup.money.net(), 0.0);
    assert!(rollup.combat.engagements.is_empty());
    assert!(rollup.skills.is_empty());
  }
}
