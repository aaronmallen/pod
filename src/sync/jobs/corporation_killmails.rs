use chrono::Utc;

use super::killmail_value::{self, PriceTable};
use crate::{
  clients::{
    Error,
    esi::{models::killmail::Killmail, scopes},
    zkillboard,
  },
  store::{
    model::{CorporationKillEntry, CorporationKillmailAttacker, CorporationKillmailItem},
    repo::{finance, org},
  },
  sync::{job::JobCtx, jobs::names::resolve_names, outcome::Outcome, structure_resolution, subject::Subject},
};

struct Ref {
  hash: String,
  killmail_id: i64,
}

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Corporation(corporation_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "corporation killmails job for {corporation_id} requires a grant"
    )));
  };
  if org::get_corporation(ctx.db, corporation_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  if !grant.has_scope(scopes::CORPORATION_KILLMAILS) {
    return Ok(Outcome::Empty);
  }

  let Some(refs) = discover(ctx, grant, corporation_id).await? else {
    return Ok(Outcome::Empty);
  };

  let zkill = zkillboard::Client::new(ctx.esi.http());
  let known = org::corporation_killmail_ids(ctx.db, corporation_id).await?;
  let prices = PriceTable::from_market_prices(&finance::market_prices_all(ctx.db).await?);
  let synced_at = Utc::now().to_rfc3339();
  let token = grant.access_token();
  let mut synced = 0usize;
  let mut skipped = 0usize;

  for reference in refs {
    if known.contains(&reference.killmail_id) {
      continue;
    }
    match assemble(ctx, &zkill, &prices, corporation_id, &reference, &synced_at, token).await {
      Ok((entry, detail)) => {
        org::upsert_corporation_killmail(ctx.db, &entry).await?;
        synced += 1;
        if let Err(error) = persist_killmail_detail(ctx, corporation_id, reference.killmail_id, &detail, &prices).await
        {
          tracing::warn!(
            corporation_id,
            killmail_id = reference.killmail_id,
            "corporation killmails: summary stored but detail persistence failed: {error}"
          );
        }
      }
      Err(error) => {
        skipped += 1;
        tracing::warn!(
          corporation_id,
          killmail_id = reference.killmail_id,
          "corporation killmails: skipping killmail whose ESI detail failed: {error}"
        );
      }
    }
  }

  Ok(outcome(corporation_id, synced, skipped))
}

fn outcome(corporation_id: i64, synced: usize, skipped: usize) -> Outcome {
  if synced > 0 {
    if skipped > 0 {
      tracing::warn!(
        corporation_id,
        synced,
        skipped,
        "corporation killmails: some killmails failed to assemble and were skipped"
      );
    }
    return Outcome::from_rows(synced);
  }
  if skipped > 0 {
    return Outcome::Skipped {
      reason: format!("{skipped} killmail(s) failed to assemble"),
    };
  }
  Outcome::Empty
}

/// Fetches the corp's recent killmails from ESI. A 403 means the Director role was revoked since
/// auth, so the corp feed is skipped gracefully (`None`) rather than failing the whole job. zKill
/// has no corp discovery feed, so ESI is the only discovery source here.
async fn discover(
  ctx: &JobCtx<'_>,
  grant: &crate::clients::eve_sso::Grant,
  corporation_id: i64,
) -> Result<Option<Vec<Ref>>, Error> {
  match ctx
    .esi
    .corporation_authenticated(grant)
    .recent_killmails(corporation_id)
    .await
  {
    Ok(recent) => Ok(Some(
      recent
        .into_iter()
        .map(|km| Ref {
          hash: km.killmail_hash,
          killmail_id: km.killmail_id,
        })
        .collect(),
    )),
    Err(error) if is_forbidden(&error) => {
      tracing::warn!(
        corporation_id,
        "corporation killmails: recent feed forbidden (Director role revoked), skipping corp"
      );
      Ok(None)
    }
    Err(error) => Err(error),
  }
}

fn is_forbidden(error: &Error) -> bool {
  matches!(error, Error::Http(http) if http.status() == Some(reqwest::StatusCode::FORBIDDEN))
}

async fn assemble(
  ctx: &JobCtx<'_>,
  zkill: &zkillboard::Client,
  prices: &PriceTable,
  corporation_id: i64,
  reference: &Ref,
  synced_at: &str,
  token: &str,
) -> Result<(CorporationKillEntry, Killmail), Error> {
  let detail = ctx
    .esi
    .killmail()
    .detail(reference.killmail_id, &reference.hash, Some(token))
    .await?;
  // A kill unless our own corp is the victim; final blow when one of our corp's pilots lands it.
  let is_kill = detail.victim.corporation_id != Some(corporation_id);
  let final_blow = detail
    .attackers
    .iter()
    .any(|attacker| attacker.final_blow && attacker.corporation_id == Some(corporation_id));

  if let Err(error) = structure_resolution::resolve_solar_system(ctx, detail.solar_system_id).await {
    tracing::warn!(
      corporation_id,
      system_id = detail.solar_system_id,
      "corporation killmails: solar system resolution failed: {error}"
    );
  }

  let resolution = killmail_value::resolve(
    zkill,
    detail.killmail_id,
    &detail.victim.items,
    detail.victim.ship_type_id,
    prices,
  )
  .await?;

  let entry = CorporationKillEntry {
    attacker_count: detail.attackers.len() as i64,
    corporation_id,
    final_blow,
    is_kill,
    kill_hash: reference.hash.clone(),
    kill_time: detail.killmail_time.clone(),
    killmail_id: detail.killmail_id,
    ship_type_id: detail.victim.ship_type_id,
    synced_at: synced_at.to_owned(),
    system_id: detail.solar_system_id,
    value_destroyed_isk: resolution.destroyed,
    value_final: false,
    value_isk: resolution.value,
    value_recheck_count: 0,
    value_source: resolution.source.as_str().to_owned(),
    victim_alliance_id: detail.victim.alliance_id,
    victim_corp_id: detail.victim.corporation_id,
    victim_damage_taken: detail.victim.damage_taken,
    victim_id: detail.victim.character_id,
  };
  Ok((entry, detail))
}

async fn persist_killmail_detail(
  ctx: &JobCtx<'_>,
  corporation_id: i64,
  killmail_id: i64,
  detail: &Killmail,
  prices: &PriceTable,
) -> Result<(), Error> {
  let attackers: Vec<CorporationKillmailAttacker> = detail
    .attackers
    .iter()
    .enumerate()
    .map(|(ordinal, attacker)| CorporationKillmailAttacker {
      alliance_id: attacker.alliance_id,
      attacker_character_id: attacker.character_id,
      attacker_corporation_id: attacker.corporation_id,
      corporation_id,
      damage_done: attacker.damage_done,
      final_blow: attacker.final_blow,
      killmail_id,
      ordinal: ordinal as i64,
      ship_type_id: attacker.ship_type_id,
    })
    .collect();

  let values = killmail_value::item_values(&detail.victim.items, prices);
  let items: Vec<CorporationKillmailItem> = detail
    .victim
    .items
    .iter()
    .zip(values)
    .enumerate()
    .map(|(ordinal, (item, value))| CorporationKillmailItem {
      corporation_id,
      flag: item.flag,
      killmail_id,
      ordinal: ordinal as i64,
      quantity_destroyed: item.quantity_destroyed.unwrap_or(0).max(0),
      quantity_dropped: item.quantity_dropped.unwrap_or(0).max(0),
      type_id: item.type_id,
      value_isk: value.value_isk,
    })
    .collect();

  org::upsert_corporation_killmail_detail(ctx.db, corporation_id, killmail_id, &attackers, &items).await?;
  resolve_third_party_names(ctx, detail).await
}

async fn resolve_third_party_names(ctx: &JobCtx<'_>, detail: &Killmail) -> Result<(), Error> {
  let parties = PartyIds::from_killmail(detail);
  let nameable = resolve_names(ctx, &parties.all()).await?;

  for id in dedupe_ids(parties.corporation_ids.clone()) {
    if nameable.contains_key(&id) {
      structure_resolution::ensure_corporation_present(ctx, id).await?;
    }
  }
  for id in dedupe_ids(parties.alliance_ids.clone()) {
    if nameable.contains_key(&id) {
      structure_resolution::ensure_alliance(ctx, id).await?;
    }
  }
  for id in dedupe_ids(parties.character_ids.clone()) {
    if nameable.contains_key(&id) {
      structure_resolution::ensure_character_present(ctx, id).await?;
    }
  }

  Ok(())
}

struct PartyIds {
  alliance_ids: Vec<i64>,
  character_ids: Vec<i64>,
  corporation_ids: Vec<i64>,
}

impl PartyIds {
  fn from_killmail(detail: &Killmail) -> Self {
    let mut ids = PartyIds {
      alliance_ids: Vec::new(),
      character_ids: Vec::new(),
      corporation_ids: Vec::new(),
    };
    ids.push(
      detail.victim.character_id,
      detail.victim.corporation_id,
      detail.victim.alliance_id,
    );
    for attacker in &detail.attackers {
      ids.push(attacker.character_id, attacker.corporation_id, attacker.alliance_id);
    }
    ids
  }

  fn all(&self) -> Vec<i64> {
    let mut all = Vec::with_capacity(self.character_ids.len() + self.corporation_ids.len() + self.alliance_ids.len());
    all.extend(&self.character_ids);
    all.extend(&self.corporation_ids);
    all.extend(&self.alliance_ids);
    all
  }

  fn push(&mut self, character: Option<i64>, corporation: Option<i64>, alliance: Option<i64>) {
    if let Some(id) = character {
      self.character_ids.push(id);
    }
    if let Some(id) = corporation {
      self.corporation_ids.push(id);
    }
    if let Some(id) = alliance {
      self.alliance_ids.push(id);
    }
  }
}

fn dedupe_ids(mut ids: Vec<i64>) -> Vec<i64> {
  ids.sort_unstable();
  ids.dedup();
  ids
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
    store::{self, images, model::Corporation},
    sync::job::{JobKey, JobKind},
  };

  async fn mount_json(server: &MockServer, route: &str, body: serde_json::Value) {
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

  async fn mount_paginated(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
      .respond_with(
        ResponseTemplate::new(200)
          .insert_header("X-Pages", "1")
          .set_body_json(body),
      )
      .mount(server)
      .await;
  }

  async fn seed_corporation(db: &store::Database, corporation_id: i64) {
    let mut corp = Corporation::new(corporation_id, "Test Corp", "TSC");
    corp.set_ceo_id(100);
    corp.set_creator_id(100);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    org::upsert_corporation(db, &corp).await.unwrap();
  }

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: &'a Grant,
    corporation_id: i64,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::CorporationKillmails, Subject::Corporation(corporation_id)),
      grant: Some(grant),
      sso: None,
    }
  }

  fn corp_grant(corporation_id: i64) -> Grant {
    Grant::new_test_with_scopes(
      "corp-token",
      corporation_id,
      vec![scopes::CORPORATION_KILLMAILS.to_owned()],
    )
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_marks_a_killmail_as_a_loss_when_our_corp_is_the_victim() {
      let server = MockServer::start().await;
      mount_paginated(
        &server,
        "/corporations/2000/killmails/recent/",
        serde_json::json!([{"killmail_id": 600, "killmail_hash": "losshash"}]),
      )
      .await;
      mount_json(
        &server,
        "/killmails/600/losshash/",
        serde_json::json!({
          "killmail_id": 600,
          "killmail_time": "2024-04-01T00:00:00Z",
          "solar_system_id": 30000142,
          "victim": {"character_id": 42, "corporation_id": 2000, "ship_type_id": 670},
          "attackers": [{"character_id": 999, "corporation_id": 8888, "final_blow": true}]
        }),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([
          {"category": "character", "id": 42, "name": "Pilot"},
          {"category": "character", "id": 999, "name": "Enemy"},
          {"category": "corporation", "id": 2000, "name": "Test Corp"},
          {"category": "corporation", "id": 8888, "name": "Enemy Corp"}
        ]),
      )
      .await;
      mount_json(&server, "/killID/600/", serde_json::json!([])).await;

      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 2000).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = corp_grant(2000);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 2000);

      run(&ctx).await.unwrap();

      let rows = org::corporation_killmails(&db, 2000).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert!(!rows[0].is_kill());
      assert!(!rows[0].final_blow());
    }

    #[tokio::test]
    async fn it_persists_a_corp_kill_with_entry_and_detail_rows_when_the_corp_is_an_attacker() {
      let server = MockServer::start().await;
      mount_paginated(
        &server,
        "/corporations/2000/killmails/recent/",
        serde_json::json!([{"killmail_id": 500, "killmail_hash": "corphash"}]),
      )
      .await;
      mount_json(
        &server,
        "/killmails/500/corphash/",
        serde_json::json!({
          "killmail_id": 500,
          "killmail_time": "2024-03-01T00:00:00Z",
          "solar_system_id": 30000142,
          "victim": {"character_id": 7777, "corporation_id": 8888, "alliance_id": 6666,
            "damage_taken": 4242, "ship_type_id": 587,
            "items": [{"flag": 27, "item_type_id": 34, "quantity_destroyed": 2},
              {"flag": 5, "item_type_id": 99, "quantity_dropped": 1}]},
          "attackers": [
            {"character_id": 42, "corporation_id": 2000, "alliance_id": 5555,
              "ship_type_id": 670, "damage_done": 5000, "final_blow": true},
            {"damage_done": 100, "final_blow": false}
          ]
        }),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([
          {"category": "character", "id": 42, "name": "Pilot"},
          {"category": "character", "id": 7777, "name": "Victim"},
          {"category": "corporation", "id": 2000, "name": "Test Corp"},
          {"category": "corporation", "id": 8888, "name": "Enemy Corp"}
        ]),
      )
      .await;
      mount_json(&server, "/killID/500/", serde_json::json!([])).await;

      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 2000).await;
      finance::market_prices_upsert_many(
        &db,
        &[
          store::model::MarketPrice::esi(587, Some(1_000.0), None),
          store::model::MarketPrice::esi(34, Some(50.0), None),
        ],
      )
      .await
      .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = corp_grant(2000);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 2000);

      run(&ctx).await.unwrap();

      let rows = org::corporation_killmails(&db, 2000).await.unwrap();
      assert_eq!(rows.len(), 1);
      let kill = &rows[0];
      assert_eq!(kill.killmail_id(), 500);
      assert!(kill.is_kill());
      assert!(kill.final_blow());
      assert_eq!(kill.victim_alliance_id(), Some(6666));
      assert_eq!(kill.victim_damage_taken(), 4242);

      let attackers = org::corporation_killmail_attackers(&db, 2000, 500).await.unwrap();
      assert_eq!(attackers.len(), 2);
      assert_eq!(attackers[0].attacker_character_id(), Some(42));
      assert_eq!(attackers[0].attacker_corporation_id(), Some(2000));
      assert_eq!(attackers[0].damage_done(), 5000);
      assert!(attackers[0].final_blow());

      let items = org::corporation_killmail_items(&db, 2000, 500).await.unwrap();
      assert_eq!(items.len(), 2);
      assert_eq!(items[0].type_id(), 34);
      assert_eq!(items[0].value_isk(), 100.0);
    }

    #[tokio::test]
    async fn it_returns_empty_when_the_corp_feed_is_empty() {
      let server = MockServer::start().await;
      mount_paginated(&server, "/corporations/2000/killmails/recent/", serde_json::json!([])).await;

      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 2000).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = corp_grant(2000);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 2000);

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(outcome, Outcome::Empty);
      assert!(org::corporation_killmails(&db, 2000).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_skips_gracefully_when_the_corp_feed_is_forbidden() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/corporations/2000/killmails/recent/"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 2000).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = corp_grant(2000);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 2000);

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(outcome, Outcome::Empty);
      assert!(org::corporation_killmails(&db, 2000).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_skips_gracefully_when_the_grant_lacks_the_corporation_killmails_scope() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/corporations/2000/killmails/recent/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;

      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 2000).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", 2000);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 2000);

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(outcome, Outcome::Empty);
      assert!(org::corporation_killmails(&db, 2000).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_corporation_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/corporations/2000/killmails/recent/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;

      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = corp_grant(2000);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 2000);

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
      assert!(org::corporation_killmails(&db, 2000).await.unwrap().is_empty());
    }
  }

  fn killmail_from(value: serde_json::Value) -> Killmail {
    serde_json::from_value(value).expect("deserialize killmail fixture")
  }

  mod party_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collects_victim_and_attacker_ids_per_category() {
      let detail = killmail_from(serde_json::json!({
        "killmail_id": 1,
        "killmail_time": "2024-05-01T00:00:00Z",
        "solar_system_id": 30000142,
        "victim": { "character_id": 7, "corporation_id": 700, "alliance_id": 9000, "ship_type_id": 587 },
        "attackers": [
          { "character_id": 8, "corporation_id": 800, "alliance_id": 9001, "final_blow": true },
          { "damage_done": 1, "final_blow": false }
        ]
      }));

      let parties = PartyIds::from_killmail(&detail);

      assert_eq!(parties.character_ids, vec![7, 8]);
      assert_eq!(parties.corporation_ids, vec![700, 800]);
      assert_eq!(parties.alliance_ids, vec![9000, 9001]);
      assert_eq!(parties.all(), vec![7, 8, 700, 800, 9000, 9001]);
    }

    #[test]
    fn it_skips_missing_ids_and_npc_attackers() {
      let detail = killmail_from(serde_json::json!({
        "killmail_id": 2,
        "killmail_time": "2024-05-01T00:00:00Z",
        "solar_system_id": 30000142,
        "victim": { "ship_type_id": 587 },
        "attackers": [{ "damage_done": 5, "final_blow": true }]
      }));

      let parties = PartyIds::from_killmail(&detail);

      assert!(parties.character_ids.is_empty());
      assert!(parties.corporation_ids.is_empty());
      assert!(parties.alliance_ids.is_empty());
      assert!(parties.all().is_empty());
    }
  }

  mod resolve_third_party_names {
    use super::*;
    use crate::store::repo::character;

    fn offline_ctx<'a>(
      db: &'a store::Database,
      esi: &'a esi::Client,
      image: &'a eve_image::Client,
      image_store: &'a images::Store,
    ) -> JobCtx<'a> {
      JobCtx {
        db,
        esi,
        image,
        image_store,
        key: JobKey::new(JobKind::CorporationKillmails, Subject::Corporation(2000)),
        grant: None,
        sso: None,
      }
    }

    fn offline_esi(db: &store::Database) -> esi::Client {
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      esi::Client::with_base_url(http, "http://127.0.0.1:1/")
    }

    #[tokio::test]
    async fn it_resolves_no_one_when_the_killmail_names_no_third_parties() {
      let db = store::open_test().await.unwrap();
      let esi = offline_esi(&db);
      let image = eve_image::Client::with_base_url(
        http::Client::builder(http::Cache::new(db.clone())).build(),
        "http://127.0.0.1:1/",
      );
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = offline_ctx(&db, &esi, &image, &image_store);
      let detail = killmail_from(serde_json::json!({
        "killmail_id": 700,
        "killmail_time": "2024-05-01T00:00:00Z",
        "solar_system_id": 30000142,
        "victim": { "ship_type_id": 587 },
        "attackers": [{ "damage_done": 100, "final_blow": true }]
      }));

      resolve_third_party_names(&ctx, &detail).await.unwrap();

      assert!(character::get(&db, 42).await.unwrap().is_none());
    }
  }
}
