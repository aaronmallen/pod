use chrono::Utc;

use crate::{
  clients::{Error, zkillboard},
  store::{model::CharacterKillEntry, repo::character},
  sync::{job::JobCtx, outcome::Outcome, structure_resolution, subject::Subject},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  run_with_zkill(ctx, &zkillboard::Client::new(ctx.esi.http())).await
}

async fn run_with_zkill(ctx: &JobCtx<'_>, zkill: &zkillboard::Client) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character killmails job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Ok(Outcome::NotReady);
  }

  let kills = zkill.character_kills(character_id).await?;
  let losses = zkill.character_losses(character_id).await?;
  let token = grant.access_token();
  let synced_at = Utc::now().to_rfc3339();
  let mut synced = 0usize;

  for (killmail, is_kill) in kills
    .into_iter()
    .map(|k| (k, true))
    .chain(losses.into_iter().map(|k| (k, false)))
  {
    match assemble(ctx, character_id, &killmail, is_kill, &synced_at, Some(token)).await {
      Ok(entry) => {
        character::upsert_killmail(ctx.db, &entry).await?;
        synced += 1;
      }
      Err(error) => tracing::warn!(
        character_id,
        killmail_id = killmail.killmail_id,
        "character killmails: skipping killmail whose ESI detail failed: {error}"
      ),
    }
  }

  Ok(Outcome::from_rows(synced))
}

async fn assemble(
  ctx: &JobCtx<'_>,
  character_id: i64,
  killmail: &zkillboard::Killmail,
  is_kill: bool,
  synced_at: &str,
  token: Option<&str>,
) -> Result<CharacterKillEntry, Error> {
  let detail = ctx
    .esi
    .killmail()
    .detail(killmail.killmail_id, &killmail.zkb.hash, token)
    .await?;
  let final_blow = detail
    .attackers
    .iter()
    .any(|attacker| attacker.final_blow && attacker.character_id == Some(character_id));

  if let Err(error) = structure_resolution::resolve_solar_system(ctx, detail.solar_system_id).await {
    tracing::warn!(
      character_id,
      system_id = detail.solar_system_id,
      "character killmails: solar system resolution failed: {error}"
    );
  }

  Ok(CharacterKillEntry {
    attacker_count: detail.attackers.len() as i64,
    character_id,
    final_blow,
    is_kill,
    kill_hash: killmail.zkb.hash.clone(),
    kill_time: detail.killmail_time,
    killmail_id: detail.killmail_id,
    ship_type_id: detail.victim.ship_type_id,
    synced_at: synced_at.to_owned(),
    system_id: detail.solar_system_id,
    value_destroyed_isk: 0.0,
    value_final: false,
    value_isk: killmail.zkb.total_value,
    value_recheck_count: 0,
    value_source: "zkill".to_owned(),
    victim_corp_id: detail.victim.corporation_id,
    victim_id: detail.victim.character_id,
  })
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, eve_image, eve_sso::Grant, http},
    store::{self, images},
    sync::job::{JobKey, JobKind},
  };

  async fn seed_character(db: &store::Database, id: i64) {
    use store::{
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::character,
    };
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

  async fn mount_json(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: &'a Grant,
    character_id: i64,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::CharacterKillmails, Subject::Character(character_id)),
      grant: Some(grant),
    }
  }

  mod run_with_zkill {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_kills_and_losses_enriched_from_esi_detail() {
      let esi_server = MockServer::start().await;
      mount_json(
        &esi_server,
        "/killmails/100/killhash/",
        serde_json::json!({
          "killmail_id": 100,
          "killmail_time": "2024-01-01T00:00:00Z",
          "solar_system_id": 30000142,
          "victim": {"character_id": 2002, "corporation_id": 3003, "ship_type_id": 587},
          "attackers": [{"character_id": 42, "final_blow": true}, {"final_blow": false}]
        }),
      )
      .await;
      mount_json(
        &esi_server,
        "/killmails/200/losshash/",
        serde_json::json!({
          "killmail_id": 200,
          "killmail_time": "2024-02-01T00:00:00Z",
          "solar_system_id": 30000142,
          "victim": {"character_id": 42, "corporation_id": 90000001, "ship_type_id": 670},
          "attackers": [{"character_id": 999, "final_blow": true}]
        }),
      )
      .await;

      let zkill_server = MockServer::start().await;
      mount_json(
        &zkill_server,
        "/characterID/42/kills/",
        serde_json::json!([{"killmail_id": 100, "zkb": {"hash": "killhash", "totalValue": 1234.5}}]),
      )
      .await;
      mount_json(
        &zkill_server,
        "/characterID/42/losses/",
        serde_json::json!([{"killmail_id": 200, "zkb": {"hash": "losshash", "totalValue": 50.0}}]),
      )
      .await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      let rows = character::killmails(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 2);
      let kill = rows.iter().find(|k| k.killmail_id() == 100).unwrap();
      assert!(kill.is_kill());
      assert!(kill.final_blow());
      assert_eq!(kill.value_isk(), 1234.5);
      assert_eq!(kill.attacker_count(), 2);
      assert_eq!(kill.ship_type_id(), 587);
      let loss = rows.iter().find(|k| k.killmail_id() == 200).unwrap();
      assert!(!loss.is_kill());
      assert!(!loss.final_blow());
    }

    #[tokio::test]
    async fn it_skips_a_killmail_whose_esi_detail_fails_without_aborting() {
      let esi_server = MockServer::start().await;
      mount_json(
        &esi_server,
        "/killmails/100/goodhash/",
        serde_json::json!({
          "killmail_id": 100,
          "killmail_time": "2024-01-01T00:00:00Z",
          "solar_system_id": 30000142,
          "victim": {"ship_type_id": 587},
          "attackers": []
        }),
      )
      .await;
      Mock::given(method("GET"))
        .and(path("/killmails/300/badhash/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&esi_server)
        .await;

      let zkill_server = MockServer::start().await;
      mount_json(
        &zkill_server,
        "/characterID/42/kills/",
        serde_json::json!([
          {"killmail_id": 100, "zkb": {"hash": "goodhash", "totalValue": 1.0}},
          {"killmail_id": 300, "zkb": {"hash": "badhash", "totalValue": 2.0}}
        ]),
      )
      .await;
      mount_json(&zkill_server, "/characterID/42/losses/", serde_json::json!([])).await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      let rows = character::killmails(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].killmail_id(), 100);
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let esi_server = MockServer::start().await;
      let zkill_server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characterID/42/kills/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&zkill_server)
        .await;

      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      assert!(character::killmails(&db, 42).await.unwrap().is_empty());
    }
  }
}
