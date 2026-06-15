use chrono::{DateTime, Duration, Utc};

use crate::{
  clients::{Error, zkillboard},
  store::{model::CharacterKillEntry, repo::character},
  sync::{job::JobCtx, outcome::Outcome},
};

// zKillboard etiquette caps us at ~1 req/s, so a single run only drains a bounded slice of the
// recheck queue; the remainder is picked up on the next interval.
const MAX_PER_RUN: usize = 25;

// A kill almost always surfaces on zKillboard within hours, so polling past a week is wasted
// traffic. We finalize a still-absent local kill once it is older than this window OR has been
// rechecked this many times, whichever comes first — both bound the queue so it cannot grow
// without end.
const MAX_RECHECKS: i64 = 10;
const RECHECK_WINDOW_DAYS: i64 = 7;

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  run_with_zkill(ctx, &zkillboard::Client::new(ctx.esi.http())).await
}

async fn run_with_zkill(ctx: &JobCtx<'_>, zkill: &zkillboard::Client) -> Result<Outcome, Error> {
  let pending = character::killmails_needing_recheck(ctx.db).await?;
  let total = pending.len();
  let deferred = total.saturating_sub(MAX_PER_RUN);
  if deferred > 0 {
    tracing::info!(
      total,
      deferred,
      "killmail reconcile: deferring rechecks beyond the per-run cap"
    );
  }

  let now = Utc::now();
  let mut touched = 0usize;
  for kill in pending.into_iter().take(MAX_PER_RUN) {
    match zkill.value_for_kill(kill.killmail_id()).await? {
      Some(value) => {
        character::killmail_upgrade_to_zkill(ctx.db, kill.character_id(), kill.killmail_id(), value).await?;
        touched += 1;
      }
      None => {
        let finalize = is_terminal(&kill, now);
        character::killmail_record_absent_recheck(ctx.db, kill.character_id(), kill.killmail_id(), finalize).await?;
        touched += 1;
      }
    }
  }

  Ok(Outcome::from_rows(touched))
}

fn is_terminal(kill: &CharacterKillEntry, now: DateTime<Utc>) -> bool {
  if kill.value_recheck_count() + 1 >= MAX_RECHECKS {
    return true;
  }
  match DateTime::parse_from_rfc3339(kill.kill_time()) {
    Ok(killed_at) => now.signed_duration_since(killed_at.with_timezone(&Utc)) >= Duration::days(RECHECK_WINDOW_DAYS),
    // An unparseable timestamp can never age out by date, so let the recheck count terminate it.
    Err(_) => false,
  }
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, eve_image, http},
    store::{
      self, Database, images,
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::character::insert_with_org,
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

  fn local_kill(character_id: i64, killmail_id: i64, kill_time: &str, recheck_count: i64) -> CharacterKillEntry {
    CharacterKillEntry {
      attacker_count: 1,
      character_id,
      final_blow: false,
      is_kill: false,
      kill_hash: "hash".to_owned(),
      kill_time: kill_time.to_owned(),
      killmail_id,
      ship_type_id: 587,
      synced_at: "2024-01-01T00:00:00Z".to_owned(),
      system_id: 30_000_142,
      value_destroyed_isk: 100.0,
      value_final: false,
      value_isk: 100.0,
      value_recheck_count: recheck_count,
      value_source: "local".to_owned(),
      victim_alliance_id: None,
      victim_corp_id: None,
      victim_damage_taken: 0,
      victim_id: Some(character_id),
    }
  }

  async fn run_in(db: &Database, zkill_uri: String) {
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = esi::Client::with_base_url(http.clone(), "http://localhost".to_owned());
    let image = eve_image::Client::with_base_url(http.clone(), "http://localhost".to_owned());
    let images_dir = tempfile::tempdir().unwrap();
    let image_store = images::Store::new(images_dir.path().to_path_buf());
    let ctx = JobCtx {
      db,
      esi: &esi,
      image: &image,
      image_store: &image_store,
      key: JobKey::new(JobKind::KillmailReconcile, Subject::Character(0)),
      grant: None,
      sso: None,
    };
    let zkill = zkillboard::Client::with_base_url(http, zkill_uri);
    run_with_zkill(&ctx, &zkill).await.unwrap();
  }

  mod run_with_zkill {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_upgrades_a_local_kill_to_the_zkill_value_when_it_is_now_present() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/killID/100/"))
        .respond_with(
          ResponseTemplate::new(200)
            .set_body_json(serde_json::json!([{"killmail_id": 100, "zkb": {"hash": "hash", "totalValue": 4242.5}}])),
        )
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let recent = Utc::now().to_rfc3339();
      character::upsert_killmail(&db, &local_kill(42, 100, &recent, 0))
        .await
        .unwrap();

      run_in(&db, server.uri()).await;

      let rows = character::killmails(&db, 42).await.unwrap();
      assert_eq!(rows[0].value_source(), "zkill");
      assert_eq!(rows[0].value_isk(), 4242.5);
      assert!(rows[0].value_final());
    }

    #[tokio::test]
    async fn it_increments_the_recheck_count_without_finalizing_a_recent_absent_kill() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/killID/100/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let recent = Utc::now().to_rfc3339();
      character::upsert_killmail(&db, &local_kill(42, 100, &recent, 0))
        .await
        .unwrap();

      run_in(&db, server.uri()).await;

      let rows = character::killmails(&db, 42).await.unwrap();
      assert_eq!(rows[0].value_recheck_count(), 1);
      assert_eq!(rows[0].value_source(), "local");
      assert!(!rows[0].value_final());
    }

    #[tokio::test]
    async fn it_finalizes_a_local_kill_once_it_ages_past_the_recheck_window() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/killID/100/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let stale = (Utc::now() - Duration::days(8)).to_rfc3339();
      character::upsert_killmail(&db, &local_kill(42, 100, &stale, 0))
        .await
        .unwrap();

      run_in(&db, server.uri()).await;

      let rows = character::killmails(&db, 42).await.unwrap();
      assert_eq!(rows[0].value_recheck_count(), 1);
      assert_eq!(rows[0].value_source(), "local");
      assert!(rows[0].value_final());
    }

    #[tokio::test]
    async fn it_finalizes_a_local_kill_once_the_recheck_count_is_exhausted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/killID/100/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let recent = Utc::now().to_rfc3339();
      character::upsert_killmail(&db, &local_kill(42, 100, &recent, MAX_RECHECKS - 1))
        .await
        .unwrap();

      run_in(&db, server.uri()).await;

      let rows = character::killmails(&db, 42).await.unwrap();
      assert!(rows[0].value_final());
    }
  }
}
