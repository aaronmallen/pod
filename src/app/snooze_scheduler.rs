use chrono::{DateTime, Utc};

use crate::store::{Database, repo::mail};

pub fn now_cutoff(now: DateTime<Utc>) -> String {
  now.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub async fn wake_due_snoozes(db: Database, now: DateTime<Utc>) -> Vec<(i64, i64)> {
  let cutoff = now_cutoff(now);
  let expired = match mail::expired_snoozed_mails(&db, &cutoff).await {
    Ok(rows) => rows,
    Err(error) => {
      tracing::warn!(%error, "snooze wake-scheduler: querying expired snoozes failed");
      return Vec::new();
    }
  };

  let mut woken = Vec::with_capacity(expired.len());
  for snooze in &expired {
    let character_id = snooze.character_id();
    let mail_id = snooze.mail_id();
    if let Err(error) = mail::delete_snoozed_mail(&db, character_id, mail_id).await {
      tracing::warn!(%error, character_id, mail_id, "snooze wake-scheduler: clearing an expired snooze failed");
      continue;
    }
    woken.push((character_id, mail_id));
  }

  if !woken.is_empty() {
    tracing::debug!(count = woken.len(), "snooze wake-scheduler: woke expired snoozes");
  }
  woken
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone as _;

  use super::*;
  use crate::store::{
    Database,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
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
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn at(year: i32, month: u32, day: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap()
  }

  mod now_cutoff {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_a_z_suffixed_seconds_precision_iso_string() {
      let now = Utc.with_ymd_and_hms(2026, 6, 15, 18, 30, 45).unwrap();

      assert_eq!(now_cutoff(now), "2026-06-15T18:30:45Z");
    }
  }

  mod wake_due_snoozes {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, repo::mail};

    #[tokio::test]
    async fn it_wakes_a_due_snooze_and_clears_the_overlay() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      mail::upsert_snoozed_mail(&db, 42, 1, "2026-06-10T08:00:00Z")
        .await
        .unwrap();

      let woken = wake_due_snoozes(db.clone(), at(2026, 6, 15)).await;

      assert_eq!(woken, [(42, 1)]);
      assert!(mail::all_snoozed_mails(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_leaves_a_not_yet_due_snooze_asleep() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      mail::upsert_snoozed_mail(&db, 42, 1, "2026-12-31T08:00:00Z")
        .await
        .unwrap();

      let woken = wake_due_snoozes(db.clone(), at(2026, 6, 15)).await;

      assert!(woken.is_empty());
      let still = mail::all_snoozed_mails(&db, 42).await.unwrap();
      assert_eq!(still.iter().map(|s| s.mail_id()).collect::<Vec<_>>(), [1]);
    }

    #[tokio::test]
    async fn it_wakes_only_the_due_snoozes_in_a_mixed_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      mail::upsert_snoozed_mail(&db, 42, 1, "2026-06-10T08:00:00Z")
        .await
        .unwrap();
      mail::upsert_snoozed_mail(&db, 43, 2, "2026-06-01T08:00:00Z")
        .await
        .unwrap();
      mail::upsert_snoozed_mail(&db, 42, 3, "2026-12-31T08:00:00Z")
        .await
        .unwrap();

      let mut woken = wake_due_snoozes(db.clone(), at(2026, 6, 15)).await;
      woken.sort_unstable();

      assert_eq!(woken, [(42, 1), (43, 2)]);
      assert_eq!(mail::all_snoozed_mails(&db, 42).await.unwrap().len(), 1);
      assert!(mail::all_snoozed_mails(&db, 43).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_wakes_nothing_on_a_quiet_tick_with_no_due_snoozes() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let woken = wake_due_snoozes(db.clone(), at(2026, 6, 15)).await;

      assert!(woken.is_empty());
    }
  }
}
