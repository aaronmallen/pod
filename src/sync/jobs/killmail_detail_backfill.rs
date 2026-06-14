//! Backfills attacker and item detail for killmail summaries that were synced before the child
//! tables existed. Idempotent: only summaries with no child rows are selected, so the job
//! converges to a no-op once all historical records are filled. The ESI killmail detail endpoint
//! is public — no bearer token is sent and no hash is ever submitted to zKillboard.

use super::{character_killmails::persist_killmail_detail, killmail_value::PriceTable};
use crate::{
  clients::Error,
  store::repo::{character, finance},
  sync::{job::JobCtx, outcome::Outcome},
};

// Limits ESI calls per scheduler tick to avoid saturating the HTTP client under a large backlog.
const MAX_PER_RUN: usize = 50;

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let total = character::killmails_needing_detail_backfill_count(ctx.db).await? as usize;
  let deferred = total.saturating_sub(MAX_PER_RUN);
  if deferred > 0 {
    tracing::info!(
      total,
      deferred,
      "killmail detail backfill: deferring killmails beyond the per-run cap"
    );
  }

  let pending = character::killmails_needing_detail_backfill(ctx.db, MAX_PER_RUN as i64).await?;
  let prices = PriceTable::from_market_prices(&finance::market_prices_all(ctx.db).await?);
  let mut filled = 0usize;

  for (character_id, killmail_id, hash) in pending {
    // None = no bearer token; the killmail detail endpoint is public.
    let detail = match ctx.esi.killmail().detail(killmail_id, &hash, None).await {
      Ok(detail) => detail,
      Err(error) => {
        tracing::warn!(
          character_id,
          killmail_id,
          "killmail detail backfill: ESI detail fetch failed: {error}"
        );
        continue;
      }
    };

    if let Err(error) = persist_killmail_detail(ctx, character_id, killmail_id, &detail, &prices).await {
      tracing::warn!(
        character_id,
        killmail_id,
        "killmail detail backfill: detail persistence failed: {error}"
      );
      continue;
    }
    filled += 1;
  }

  Ok(Outcome::from_rows(filled))
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
      model::{Alliance, Bloodline, Character, CharacterKillEntry, Corporation, Gender, Race},
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
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn summary(character_id: i64, killmail_id: i64, hash: &str) -> CharacterKillEntry {
    CharacterKillEntry {
      attacker_count: 1,
      character_id,
      final_blow: true,
      is_kill: true,
      kill_hash: hash.to_owned(),
      kill_time: "2024-01-01T00:00:00Z".to_owned(),
      killmail_id,
      ship_type_id: 587,
      synced_at: "2024-01-01T00:00:00Z".to_owned(),
      system_id: 30_000_142,
      value_destroyed_isk: 0.0,
      value_final: false,
      value_isk: 1.0,
      value_recheck_count: 0,
      value_source: "local".to_owned(),
      victim_alliance_id: None,
      victim_corp_id: None,
      victim_damage_taken: 0,
      victim_id: None,
    }
  }

  async fn mount_detail(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn mount_names(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("POST"))
      .and(path("/universe/names/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn run_against(db: &Database, esi_uri: &str) -> Outcome {
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = esi::Client::with_base_url(http.clone(), esi_uri.to_owned());
    let image = eve_image::Client::with_base_url(http, esi_uri.to_owned());
    let images_dir = tempfile::tempdir().unwrap();
    let image_store = images::Store::new(images_dir.path().to_path_buf());
    let ctx = JobCtx {
      db,
      esi: &esi,
      image: &image,
      image_store: &image_store,
      key: JobKey::new(JobKind::KillmailDetailBackfill, Subject::Character(0)),
      grant: None,
    };
    run(&ctx).await.unwrap()
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    fn detail_body() -> serde_json::Value {
      serde_json::json!({
        "killmail_id": 100,
        "killmail_time": "2024-01-01T00:00:00Z",
        "solar_system_id": 30000142,
        "victim": {"character_id": 2002, "corporation_id": 90000001, "alliance_id": 99000001,
          "damage_taken": 7821, "ship_type_id": 587,
          "items": [{"flag": 27, "item_type_id": 34, "quantity_destroyed": 2}]},
        "attackers": [
          {"character_id": 42, "corporation_id": 90000001, "ship_type_id": 670,
            "damage_done": 5000, "final_blow": true},
          {"damage_done": 100, "final_blow": false}
        ]
      })
    }

    #[tokio::test]
    async fn it_backfills_attacker_and_item_detail_for_a_summary_without_detail() {
      let server = MockServer::start().await;
      mount_detail(&server, "/killmails/100/killhash/", detail_body()).await;
      mount_names(
        &server,
        serde_json::json!([
          {"category": "character", "id": 42, "name": "Pilot"},
          {"category": "character", "id": 2002, "name": "Victim"},
          {"category": "corporation", "id": 90000001, "name": "Test Corp"},
          {"category": "alliance", "id": 99000001, "name": "Test Alliance"}
        ]),
      )
      .await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      finance::market_prices_upsert_many(
        &db,
        &[store::model::MarketPrice {
          adjusted_price: Some(50.0),
          average_price: None,
          type_id: 34,
        }],
      )
      .await
      .unwrap();
      character::upsert_killmail(&db, &summary(42, 100, "killhash"))
        .await
        .unwrap();

      let outcome = run_against(&db, &server.uri()).await;

      assert_eq!(outcome, Outcome::from_rows(1));
      let attackers = character::killmail_attackers(&db, 42, 100).await.unwrap();
      assert_eq!(attackers.len(), 2);
      assert_eq!(attackers[0].attacker_character_id(), Some(42));
      let items = character::killmail_items(&db, 42, 100).await.unwrap();
      assert_eq!(items.len(), 1);
      assert_eq!(items[0].type_id(), 34);
      assert_eq!(items[0].value_isk(), 100.0);
    }

    #[tokio::test]
    async fn it_does_not_refetch_or_duplicate_on_a_second_run() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/killmails/100/killhash/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(detail_body()))
        .expect(1)
        .mount(&server)
        .await;
      mount_names(&server, serde_json::json!([])).await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      character::upsert_killmail(&db, &summary(42, 100, "killhash"))
        .await
        .unwrap();

      let first = run_against(&db, &server.uri()).await;
      let second = run_against(&db, &server.uri()).await;

      assert_eq!(first, Outcome::from_rows(1));
      assert_eq!(second, Outcome::Empty);

      assert_eq!(character::killmail_attackers(&db, 42, 100).await.unwrap().len(), 2);
      assert_eq!(character::killmail_items(&db, 42, 100).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_fetches_the_detail_endpoint_without_a_bearer_token() {
      struct NoAuthorizationHeader;
      impl wiremock::Match for NoAuthorizationHeader {
        fn matches(&self, request: &wiremock::Request) -> bool {
          !request.headers.contains_key("Authorization")
        }
      }

      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/killmails/100/killhash/"))
        .and(NoAuthorizationHeader)
        .respond_with(ResponseTemplate::new(200).set_body_json(detail_body()))
        .expect(1)
        .mount(&server)
        .await;
      mount_names(&server, serde_json::json!([])).await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      character::upsert_killmail(&db, &summary(42, 100, "killhash"))
        .await
        .unwrap();

      run_against(&db, &server.uri()).await;

      assert_eq!(character::killmail_attackers(&db, 42, 100).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn it_never_posts_a_kill_hash_to_zkillboard() {
      let server = MockServer::start().await;
      mount_detail(&server, "/killmails/100/killhash/", detail_body()).await;
      mount_names(&server, serde_json::json!([])).await;
      Mock::given(method("POST"))
        .and(wiremock::matchers::path_regex(r".*killID.*"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      character::upsert_killmail(&db, &summary(42, 100, "killhash"))
        .await
        .unwrap();

      run_against(&db, &server.uri()).await;
    }

    #[tokio::test]
    async fn it_is_a_no_op_when_no_killmail_needs_backfill() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let outcome = run_against(&db, &server.uri()).await;

      assert_eq!(outcome, Outcome::Empty);
    }
  }
}
