use std::collections::HashSet;

use chrono::Utc;

use super::killmail_value::{self, PriceTable};
use crate::{
  clients::{
    Error,
    esi::{models::killmail::Killmail, scopes},
    zkillboard,
  },
  store::{
    model::{CharacterKillEntry, KillmailAttacker, KillmailItem},
    repo::{character, finance},
  },
  sync::{job::JobCtx, jobs::names::resolve_names, outcome::Outcome, structure_resolution, subject::Subject},
};

const DIRECTOR_ROLE: &str = "Director";

struct Ref {
  hash: String,
  is_kill: Option<bool>,
  killmail_id: i64,
}

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
  let Some(character) = character::get(ctx.db, character_id).await? else {
    return Err(Error::NotReady);
  };

  let mut refs = discover_character(ctx, zkill, grant, character_id).await?;
  discover_corporation(ctx, grant, character_id, character.corporation_id(), &mut refs).await?;

  let known = character::killmail_ids(ctx.db, character_id).await?;
  let prices = PriceTable::from_market_prices(&finance::market_prices_all(ctx.db).await?);
  let synced_at = Utc::now().to_rfc3339();
  let token = grant.access_token();
  let mut synced = 0usize;
  let mut skipped = 0usize;
  // A fallback feed (notably the zKill character endpoint during an ESI outage) can answer with a
  // global "recent kills" firehose rather than this character's mails; track how many of a batch are
  // discarded as non-participants so a contaminated feed is loud in the logs.
  let mut candidates = 0usize;
  let mut non_participants = 0usize;

  for reference in dedupe(refs) {
    if known.contains(&reference.killmail_id) {
      continue;
    }
    candidates += 1;
    match assemble(ctx, zkill, &prices, character_id, &reference, &synced_at, token).await {
      Ok((entry, detail)) => {
        if !is_participant(character_id, &detail) {
          non_participants += 1;
          skipped += 1;
          tracing::warn!(
            character_id,
            killmail_id = reference.killmail_id,
            "character killmails: skipping killmail the character is not a participant of (victim or attacker)"
          );
          continue;
        }
        character::upsert_killmail(ctx.db, &entry).await?;
        synced += 1;
        if let Err(error) = persist_killmail_detail(ctx, character_id, reference.killmail_id, &detail, &prices).await {
          tracing::warn!(
            character_id,
            killmail_id = reference.killmail_id,
            "character killmails: summary stored but detail persistence failed: {error}"
          );
        }
      }
      Err(error) => {
        skipped += 1;
        tracing::warn!(
          character_id,
          killmail_id = reference.killmail_id,
          "character killmails: skipping killmail whose ESI detail failed: {error}"
        );
      }
    }
  }

  warn_on_suspected_global_feed(character_id, candidates, non_participants);
  Ok(outcome(character_id, synced, skipped))
}

/// A discovered killmail belongs on a character's board only when that character is the victim or
/// one of the attackers; discovery feeds are not trusted to be character-scoped (a zKill outage
/// fallback can return an unrelated global feed), so participation is confirmed against the ESI
/// detail before the killmail is persisted.
fn is_participant(character_id: i64, detail: &Killmail) -> bool {
  detail.victim.character_id == Some(character_id)
    || detail
      .attackers
      .iter()
      .any(|attacker| attacker.character_id == Some(character_id))
}

/// Threshold above which a batch that is (nearly) entirely non-participants is treated as a likely
/// global-feed contamination rather than a few stray mails, and the WARN is emitted.
const SUSPECTED_GLOBAL_FEED_MIN_BATCH: usize = 25;

/// Emits a loud WARN when a fallback batch is implausibly large and (nearly) all of its mails were
/// discarded as non-participants — the signature of a zKill global "recent kills" firehose served in
/// place of the character's own feed during an ESI outage — so the contamination is diagnosable from
/// logs. The participant guard already prevented any of these mails from being written.
fn warn_on_suspected_global_feed(character_id: i64, candidates: usize, non_participants: usize) {
  if candidates >= SUSPECTED_GLOBAL_FEED_MIN_BATCH && non_participants * 4 >= candidates * 3 {
    tracing::warn!(
      character_id,
      candidates,
      non_participants,
      "character killmails: suspected global-feed contamination — a large discovery batch was almost entirely non-participant mails (likely a zKillboard global firehose during an ESI outage); all such mails were discarded"
    );
  }
}

/// Returns `Synced` when at least one killmail was stored, even if others were skipped; the skip
/// count is warned rather than downgrading the outcome so the ledger reflects real progress.
fn outcome(character_id: i64, synced: usize, skipped: usize) -> Outcome {
  if synced > 0 {
    if skipped > 0 {
      tracing::warn!(
        character_id,
        synced,
        skipped,
        "character killmails: some killmails failed to assemble and were skipped"
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

async fn discover_character(
  ctx: &JobCtx<'_>,
  zkill: &zkillboard::Client,
  grant: &crate::clients::eve_sso::Grant,
  character_id: i64,
) -> Result<Vec<Ref>, Error> {
  // ESI is authoritative; the zKill character feed is a fallback used only when the killmails
  // scope is absent or ESI itself errors (so a kill window is still captured, valued locally).
  if grant.has_scope(scopes::CHARACTER_KILLMAILS) {
    match ctx.esi.character_authenticated(grant).recent_killmails().await {
      Ok(recent) => {
        return Ok(
          recent
            .into_iter()
            .map(|km| Ref {
              hash: km.killmail_hash,
              is_kill: None,
              killmail_id: km.killmail_id,
            })
            .collect(),
        );
      }
      Err(error) => tracing::warn!(
        character_id,
        "character killmails: ESI recent feed failed, falling back to zKillboard: {error}"
      ),
    }
  }

  zkill_character_refs(zkill, character_id).await
}

async fn discover_corporation(
  ctx: &JobCtx<'_>,
  grant: &crate::clients::eve_sso::Grant,
  character_id: i64,
  corporation_id: i64,
  refs: &mut Vec<Ref>,
) -> Result<(), Error> {
  if !grant.has_scope(scopes::CORPORATION_KILLMAILS) {
    return Ok(());
  }
  if !holds_director_role(ctx, grant, character_id, corporation_id).await? {
    return Ok(());
  }

  // Corp recent still requires the Director role on the server side; a 403 means the role was
  // revoked since auth, so skip the corp feed rather than failing the whole job. zKill has no
  // corp feed, so absent ESI access the corp portion is simply skipped (corp losses involving
  // this character still surface through the character feed).
  match ctx
    .esi
    .corporation_authenticated(grant)
    .recent_killmails(corporation_id)
    .await
  {
    Ok(recent) => {
      refs.extend(recent.into_iter().map(|km| Ref {
        hash: km.killmail_hash,
        is_kill: None,
        killmail_id: km.killmail_id,
      }));
      Ok(())
    }
    Err(error) if is_forbidden(&error) => {
      tracing::warn!(
        character_id,
        corporation_id,
        "character killmails: corporation recent feed forbidden (Director role revoked), skipping corp"
      );
      Ok(())
    }
    Err(error) => Err(error),
  }
}

async fn holds_director_role(
  ctx: &JobCtx<'_>,
  grant: &crate::clients::eve_sso::Grant,
  character_id: i64,
  corporation_id: i64,
) -> Result<bool, Error> {
  match ctx
    .esi
    .corporation_authenticated(grant)
    .member_roles(corporation_id)
    .await
  {
    Ok(roles) => Ok(
      roles
        .iter()
        .find(|member| member.character_id == character_id)
        .is_some_and(|member| member.roles.iter().any(|role| role == DIRECTOR_ROLE)),
    ),
    Err(error) if is_forbidden(&error) => {
      tracing::warn!(
        character_id,
        corporation_id,
        "character killmails: corporation roles forbidden, skipping corp discovery"
      );
      Ok(false)
    }
    Err(error) => Err(error),
  }
}

async fn zkill_character_refs(zkill: &zkillboard::Client, character_id: i64) -> Result<Vec<Ref>, Error> {
  let kills = zkill.character_kills(character_id).await?;
  let losses = zkill.character_losses(character_id).await?;
  Ok(
    kills
      .into_iter()
      .map(|km| (km, true))
      .chain(losses.into_iter().map(|km| (km, false)))
      .map(|(km, is_kill)| Ref {
        hash: km.zkb.hash,
        is_kill: Some(is_kill),
        killmail_id: km.killmail_id,
      })
      .collect(),
  )
}

fn dedupe(refs: Vec<Ref>) -> Vec<Ref> {
  let mut seen = HashSet::new();
  refs.into_iter().filter(|r| seen.insert(r.killmail_id)).collect()
}

fn is_forbidden(error: &Error) -> bool {
  matches!(error, Error::Http(http) if http.status() == Some(reqwest::StatusCode::FORBIDDEN))
}

async fn assemble(
  ctx: &JobCtx<'_>,
  zkill: &zkillboard::Client,
  prices: &PriceTable,
  character_id: i64,
  reference: &Ref,
  synced_at: &str,
  token: &str,
) -> Result<(CharacterKillEntry, Killmail), Error> {
  let detail = ctx
    .esi
    .killmail()
    .detail(reference.killmail_id, &reference.hash, Some(token))
    .await?;
  let final_blow = detail
    .attackers
    .iter()
    .any(|attacker| attacker.final_blow && attacker.character_id == Some(character_id));
  // A discovery feed (zKill) tells us kill-vs-loss directly; an ESI recent reference does not, so
  // it is a loss only when this character is the victim.
  let is_kill = reference
    .is_kill
    .unwrap_or_else(|| detail.victim.character_id != Some(character_id));

  if let Err(error) = structure_resolution::resolve_solar_system(ctx, detail.solar_system_id).await {
    tracing::warn!(
      character_id,
      system_id = detail.solar_system_id,
      "character killmails: solar system resolution failed: {error}"
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

  let entry = CharacterKillEntry {
    attacker_count: detail.attackers.len() as i64,
    character_id,
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

/// Persists the attacker and victim-item child rows for an already-fetched killmail detail and
/// ensures third-party pilot/corp/alliance name rows exist locally, so the killmail modal can render
/// names rather than raw ids. Per-item ISK is snapshotted from `prices` at persistence time.
///
/// This is intentionally free of discovery-feed assumptions (it takes the detail directly) so the
/// one-time backfill job can reuse it with a detail it fetched itself.
pub async fn persist_killmail_detail(
  ctx: &JobCtx<'_>,
  character_id: i64,
  killmail_id: i64,
  detail: &Killmail,
  prices: &PriceTable,
) -> Result<(), Error> {
  let attackers: Vec<KillmailAttacker> = detail
    .attackers
    .iter()
    .enumerate()
    .map(|(ordinal, attacker)| KillmailAttacker {
      alliance_id: attacker.alliance_id,
      attacker_character_id: attacker.character_id,
      character_id,
      corporation_id: attacker.corporation_id,
      damage_done: attacker.damage_done,
      final_blow: attacker.final_blow,
      killmail_id,
      ordinal: ordinal as i64,
      ship_type_id: attacker.ship_type_id,
    })
    .collect();

  let values = killmail_value::item_values(&detail.victim.items, prices);
  let items: Vec<KillmailItem> = detail
    .victim
    .items
    .iter()
    .zip(values)
    .enumerate()
    .map(|(ordinal, (item, value))| KillmailItem {
      character_id,
      flag: item.flag,
      killmail_id,
      ordinal: ordinal as i64,
      quantity_destroyed: item.quantity_destroyed.unwrap_or(0).max(0),
      quantity_dropped: item.quantity_dropped.unwrap_or(0).max(0),
      type_id: item.type_id,
      value_isk: value.value_isk,
    })
    .collect();

  character::upsert_killmail_detail(ctx.db, character_id, killmail_id, &attackers, &items).await?;
  resolve_third_party_names(ctx, detail).await
}

/// Ensures name-bearing local rows exist for every third party referenced by the killmail —
/// attacker (and victim) characters, corporations, and alliances — that is not already tracked.
/// A single bulk `resolve_names` call filters out ids the universe cannot name (NPCs, structures)
/// before the per-id `ensure_*` resolutions, so unresolvable ids are skipped without a wasted fetch.
async fn resolve_third_party_names(ctx: &JobCtx<'_>, detail: &Killmail) -> Result<(), Error> {
  let mut alliance_ids = Vec::new();
  let mut character_ids = Vec::new();
  let mut corporation_ids = Vec::new();

  let mut collect = |character: Option<i64>, corporation: Option<i64>, alliance: Option<i64>| {
    if let Some(id) = character {
      character_ids.push(id);
    }
    if let Some(id) = corporation {
      corporation_ids.push(id);
    }
    if let Some(id) = alliance {
      alliance_ids.push(id);
    }
  };

  collect(
    detail.victim.character_id,
    detail.victim.corporation_id,
    detail.victim.alliance_id,
  );
  for attacker in &detail.attackers {
    collect(attacker.character_id, attacker.corporation_id, attacker.alliance_id);
  }

  let mut all_ids = Vec::new();
  all_ids.extend(&character_ids);
  all_ids.extend(&corporation_ids);
  all_ids.extend(&alliance_ids);
  let nameable = resolve_names(ctx, &all_ids).await?;

  for id in dedupe_ids(corporation_ids) {
    if nameable.contains_key(&id) {
      structure_resolution::ensure_corporation_present(ctx, id).await?;
    }
  }
  for id in dedupe_ids(alliance_ids) {
    if nameable.contains_key(&id) {
      structure_resolution::ensure_alliance(ctx, id).await?;
    }
  }
  for id in dedupe_ids(character_ids) {
    if nameable.contains_key(&id) {
      structure_resolution::ensure_character_present(ctx, id).await?;
    }
  }

  Ok(())
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
      sso: None,
    }
  }

  mod run_with_zkill {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_discovers_via_esi_and_values_locally_when_the_kill_is_absent_from_zkill() {
      let esi_server = MockServer::start().await;
      mount_paginated(
        &esi_server,
        "/characters/42/killmails/recent/",
        serde_json::json!([{"killmail_id": 100, "killmail_hash": "killhash"}]),
      )
      .await;
      mount_json(
        &esi_server,
        "/killmails/100/killhash/",
        serde_json::json!({
          "killmail_id": 100,
          "killmail_time": "2024-01-01T00:00:00Z",
          "solar_system_id": 30000142,
          "victim": {"character_id": 2002, "corporation_id": 3003, "ship_type_id": 587,
            "items": [{"flag": 5, "item_type_id": 34, "quantity_destroyed": 2}]},
          "attackers": [{"character_id": 42, "final_blow": true}, {"final_blow": false}]
        }),
      )
      .await;

      let zkill_server = MockServer::start().await;
      mount_json(&zkill_server, "/killID/100/", serde_json::json!([])).await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
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
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes("token", 42, vec![scopes::CHARACTER_KILLMAILS.to_owned()]);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      let rows = character::killmails(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      let kill = &rows[0];
      assert_eq!(kill.killmail_id(), 100);
      assert!(kill.is_kill());
      assert!(kill.final_blow());
      assert_eq!(kill.value_source(), "local");
      assert_eq!(kill.value_isk(), 1_100.0);
      assert_eq!(kill.value_destroyed_isk(), 1_100.0);
      assert!(!kill.value_final());
    }

    #[tokio::test]
    async fn it_falls_back_to_the_zkill_feed_when_the_killmails_scope_is_missing() {
      let esi_server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/killmails/recent/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&esi_server)
        .await;
      mount_json(
        &esi_server,
        "/killmails/100/killhash/",
        serde_json::json!({
          "killmail_id": 100,
          "killmail_time": "2024-01-01T00:00:00Z",
          "solar_system_id": 30000142,
          "victim": {"character_id": 2002, "ship_type_id": 587},
          "attackers": [{"character_id": 42, "final_blow": true}]
        }),
      )
      .await;

      let zkill_server = MockServer::start().await;
      mount_json(
        &zkill_server,
        "/characterID/42/kills/",
        serde_json::json!([{"killmail_id": 100, "zkb": {"hash": "killhash", "totalValue": 4242.0}}]),
      )
      .await;
      mount_json(&zkill_server, "/characterID/42/losses/", serde_json::json!([])).await;
      mount_json(
        &zkill_server,
        "/killID/100/",
        serde_json::json!([{"killmail_id": 100, "zkb": {"hash": "killhash", "totalValue": 4242.0}}]),
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
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].killmail_id(), 100);
      assert!(rows[0].is_kill());
      assert_eq!(rows[0].value_source(), "zkill");
      assert_eq!(rows[0].value_isk(), 4242.0);
    }

    #[tokio::test]
    async fn it_includes_corp_killmails_when_the_character_is_a_director() {
      let esi_server = MockServer::start().await;
      mount_paginated(&esi_server, "/characters/42/killmails/recent/", serde_json::json!([])).await;
      mount_json(
        &esi_server,
        "/corporations/90000001/roles/",
        serde_json::json!([{"character_id": 42, "roles": ["Director"]}]),
      )
      .await;
      mount_paginated(
        &esi_server,
        "/corporations/90000001/killmails/recent/",
        serde_json::json!([{"killmail_id": 500, "killmail_hash": "corphash"}]),
      )
      .await;
      mount_json(
        &esi_server,
        "/killmails/500/corphash/",
        serde_json::json!({
          "killmail_id": 500,
          "killmail_time": "2024-03-01T00:00:00Z",
          "solar_system_id": 30000142,
          "victim": {"character_id": 7777, "corporation_id": 8888, "ship_type_id": 587},
          "attackers": [{"character_id": 42, "final_blow": true}]
        }),
      )
      .await;

      let zkill_server = MockServer::start().await;
      mount_json(&zkill_server, "/killID/500/", serde_json::json!([])).await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes(
        "token",
        42,
        vec![
          scopes::CHARACTER_KILLMAILS.to_owned(),
          scopes::CORPORATION_KILLMAILS.to_owned(),
        ],
      );
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      let rows = character::killmails(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].killmail_id(), 500);
      assert!(rows[0].is_kill());
    }

    #[tokio::test]
    async fn it_writes_no_rows_when_the_discovery_feed_returns_killmails_the_character_is_not_on() {
      // Simulates the zKill global "recent kills" firehose served during an ESI outage: the feed
      // hands back a killmail the syncing character (42) neither dealt nor took, so the participant
      // guard must discard it without writing any summary or child rows.
      let esi_server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/killmails/recent/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&esi_server)
        .await;
      mount_json(
        &esi_server,
        "/killmails/777/globalhash/",
        serde_json::json!({
          "killmail_id": 777,
          "killmail_time": "2026-06-24T11:00:00Z",
          "solar_system_id": 30000142,
          "victim": {"character_id": 5005, "corporation_id": 6006, "ship_type_id": 587},
          "attackers": [{"character_id": 8008, "final_blow": true}, {"character_id": 9009, "final_blow": false}]
        }),
      )
      .await;

      let zkill_server = MockServer::start().await;
      mount_json(
        &zkill_server,
        "/characterID/42/kills/",
        serde_json::json!([{"killmail_id": 777, "zkb": {"hash": "globalhash", "totalValue": 4242.0}}]),
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

      let outcome = run_with_zkill(&ctx, &zkill).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Skipped {
          reason: "1 killmail(s) failed to assemble".to_owned()
        }
      );
      assert!(character::killmails(&db, 42).await.unwrap().is_empty());
      assert!(character::killmail_attackers(&db, 42, 777).await.unwrap().is_empty());
      assert!(character::killmail_items(&db, 42, 777).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_persists_attacker_and_item_detail_with_victim_alliance_and_damage() {
      let esi_server = MockServer::start().await;
      mount_paginated(
        &esi_server,
        "/characters/42/killmails/recent/",
        serde_json::json!([{"killmail_id": 100, "killmail_hash": "killhash"}]),
      )
      .await;
      mount_json(
        &esi_server,
        "/killmails/100/killhash/",
        serde_json::json!({
          "killmail_id": 100,
          "killmail_time": "2024-01-01T00:00:00Z",
          "solar_system_id": 30000142,
          "victim": {"character_id": 2002, "corporation_id": 90000001, "alliance_id": 99000001,
            "damage_taken": 7821, "ship_type_id": 587,
            "items": [{"flag": 27, "item_type_id": 34, "quantity_destroyed": 2},
              {"flag": 5, "item_type_id": 99, "quantity_dropped": 1}]},
          "attackers": [
            {"character_id": 42, "corporation_id": 90000001, "alliance_id": 99000001,
              "ship_type_id": 670, "damage_done": 5000, "final_blow": true},
            {"damage_done": 100, "final_blow": false}
          ]
        }),
      )
      .await;
      mount_names(
        &esi_server,
        serde_json::json!([
          {"category": "character", "id": 42, "name": "Pilot"},
          {"category": "character", "id": 2002, "name": "Victim"},
          {"category": "corporation", "id": 90000001, "name": "Test Corp"},
          {"category": "alliance", "id": 99000001, "name": "Test Alliance"}
        ]),
      )
      .await;

      let zkill_server = MockServer::start().await;
      mount_json(&zkill_server, "/killID/100/", serde_json::json!([])).await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
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
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes("token", 42, vec![scopes::CHARACTER_KILLMAILS.to_owned()]);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      let summary = &character::killmails(&db, 42).await.unwrap()[0];
      assert_eq!(summary.victim_alliance_id(), Some(99000001));
      assert_eq!(summary.victim_damage_taken(), 7821);

      let attackers = character::killmail_attackers(&db, 42, 100).await.unwrap();
      assert_eq!(attackers.len(), 2);
      assert_eq!(attackers[0].attacker_character_id(), Some(42));
      assert_eq!(attackers[0].damage_done(), 5000);
      assert!(attackers[0].final_blow());
      assert_eq!(attackers[1].attacker_character_id(), None);

      let items = character::killmail_items(&db, 42, 100).await.unwrap();
      assert_eq!(items.len(), 2);
      assert_eq!(items[0].type_id(), 34);
      assert_eq!(items[0].quantity_destroyed(), 2);
      assert_eq!(items[0].value_isk(), 100.0);
      assert_eq!(items[1].type_id(), 99);
      assert_eq!(items[1].value_isk(), 0.0);
    }

    #[tokio::test]
    async fn it_reports_a_skipped_outcome_when_every_discovered_killmail_fails_to_assemble() {
      let esi_server = MockServer::start().await;
      mount_paginated(
        &esi_server,
        "/characters/42/killmails/recent/",
        serde_json::json!([{"killmail_id": 100, "killmail_hash": "killhash"}]),
      )
      .await;
      mount_json(
        &esi_server,
        "/killmails/100/killhash/",
        serde_json::json!({
          "killmail_id": 100,
          "solar_system_id": 30000142,
          "victim": {"ship_type_id": 587},
          "attackers": []
        }),
      )
      .await;

      let zkill_server = MockServer::start().await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes("token", 42, vec![scopes::CHARACTER_KILLMAILS.to_owned()]);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      let outcome = run_with_zkill(&ctx, &zkill).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Skipped {
          reason: "1 killmail(s) failed to assemble".to_owned()
        }
      );
      assert!(character::killmails(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_resolves_name_rows_for_an_untracked_attacker_character_corp_and_alliance() {
      let esi_server = MockServer::start().await;
      mount_paginated(
        &esi_server,
        "/characters/42/killmails/recent/",
        serde_json::json!([{"killmail_id": 100, "killmail_hash": "killhash"}]),
      )
      .await;
      mount_json(
        &esi_server,
        "/killmails/100/killhash/",
        serde_json::json!({
          "killmail_id": 100,
          "killmail_time": "2024-01-01T00:00:00Z",
          "solar_system_id": 30000142,
          "victim": {"character_id": 42, "corporation_id": 90000001, "ship_type_id": 670},
          "attackers": [
            {"character_id": 7001, "corporation_id": 80001, "alliance_id": 70001,
              "ship_type_id": 17738, "damage_done": 9000, "final_blow": true}
          ]
        }),
      )
      .await;
      mount_names(
        &esi_server,
        serde_json::json!([
          {"category": "character", "id": 42, "name": "Pilot"},
          {"category": "character", "id": 7001, "name": "Enemy Pilot"},
          {"category": "corporation", "id": 80001, "name": "Enemy Corp"},
          {"category": "corporation", "id": 90000001, "name": "Test Corp"},
          {"category": "alliance", "id": 70001, "name": "Enemy Alliance"}
        ]),
      )
      .await;
      mount_json(
        &esi_server,
        "/characters/7001/",
        serde_json::json!({
          "birthday": "2010-01-01T00:00:00Z", "bloodline_id": 5, "corporation_id": 80001,
          "gender": "male", "name": "Enemy Pilot", "race_id": 1,
        }),
      )
      .await;
      mount_json(
        &esi_server,
        "/corporations/80001/",
        serde_json::json!({
          "alliance_id": 70001, "ceo_id": 7001, "creator_id": 7001, "member_count": 50,
          "name": "Enemy Corp", "tax_rate": 0.1, "ticker": "ENMY",
        }),
      )
      .await;
      mount_json(
        &esi_server,
        "/alliances/70001/",
        serde_json::json!({
          "creator_corporation_id": 80001, "creator_id": 7001, "date_founded": "2009-01-01T00:00:00Z",
          "name": "Enemy Alliance", "ticker": "ENA",
        }),
      )
      .await;
      mount_json(
        &esi_server,
        "/universe/races/",
        serde_json::json!([
          {"alliance_id": 500001, "description": "The Caldari.", "name": "Caldari", "race_id": 1}
        ]),
      )
      .await;
      mount_json(
        &esi_server,
        "/universe/bloodlines/",
        serde_json::json!([
          {"bloodline_id": 5, "charisma": 6, "corporation_id": 80001, "description": "The Civire.",
            "intelligence": 7, "memory": 5, "name": "Civire", "perception": 5, "race_id": 1,
            "ship_type_id": 601, "willpower": 5}
        ]),
      )
      .await;

      let zkill_server = MockServer::start().await;
      mount_json(&zkill_server, "/killID/100/", serde_json::json!([])).await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes("token", 42, vec![scopes::CHARACTER_KILLMAILS.to_owned()]);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      use store::repo::org;
      assert_eq!(
        character::get(&db, 7001).await.unwrap().map(|c| c.name().to_owned()),
        Some("Enemy Pilot".to_owned())
      );
      assert_eq!(
        org::get_corporation(&db, 80001)
          .await
          .unwrap()
          .map(|c| c.name().to_owned()),
        Some("Enemy Corp".to_owned())
      );
      assert_eq!(
        org::get_alliance(&db, 70001)
          .await
          .unwrap()
          .map(|a| a.name().to_owned()),
        Some("Enemy Alliance".to_owned())
      );
    }

    #[tokio::test]
    async fn it_skips_already_stored_killmails_without_refetching_detail() {
      let esi_server = MockServer::start().await;
      mount_paginated(
        &esi_server,
        "/characters/42/killmails/recent/",
        serde_json::json!([{"killmail_id": 100, "killmail_hash": "killhash"}]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path("/killmails/100/killhash/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "killmail_id": 100,
          "killmail_time": "2024-01-01T00:00:00Z",
          "solar_system_id": 30000142,
          "victim": {"ship_type_id": 587},
          "attackers": []
        })))
        .expect(0)
        .mount(&esi_server)
        .await;

      let zkill_server = MockServer::start().await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      character::upsert_killmail(
        &db,
        &CharacterKillEntry {
          attacker_count: 1,
          character_id: 42,
          final_blow: true,
          is_kill: true,
          kill_hash: "killhash".to_owned(),
          kill_time: "2024-01-01T00:00:00Z".to_owned(),
          killmail_id: 100,
          ship_type_id: 587,
          synced_at: "2024-01-01T00:00:00Z".to_owned(),
          system_id: 30000142,
          value_destroyed_isk: 0.0,
          value_final: false,
          value_isk: 1.0,
          value_recheck_count: 0,
          value_source: "local".to_owned(),
          victim_alliance_id: None,
          victim_corp_id: None,
          victim_damage_taken: 0,
          victim_id: None,
        },
      )
      .await
      .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes("token", 42, vec![scopes::CHARACTER_KILLMAILS.to_owned()]);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      assert_eq!(character::killmails(&db, 42).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_skips_corp_discovery_when_the_character_lacks_director_role() {
      let esi_server = MockServer::start().await;
      mount_paginated(&esi_server, "/characters/42/killmails/recent/", serde_json::json!([])).await;
      mount_json(
        &esi_server,
        "/corporations/90000001/roles/",
        serde_json::json!([{"character_id": 42, "roles": ["Accountant"]}]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path("/corporations/90000001/killmails/recent/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&esi_server)
        .await;

      let zkill_server = MockServer::start().await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes(
        "token",
        42,
        vec![
          scopes::CHARACTER_KILLMAILS.to_owned(),
          scopes::CORPORATION_KILLMAILS.to_owned(),
        ],
      );
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      assert!(character::killmails(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let esi_server = MockServer::start().await;
      let zkill_server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/killmails/recent/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&esi_server)
        .await;

      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes("token", 42, vec![scopes::CHARACTER_KILLMAILS.to_owned()]);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      let result = run_with_zkill(&ctx, &zkill).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
      assert!(character::killmails(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_takes_the_zkill_value_when_the_kill_is_present_on_zkill() {
      let esi_server = MockServer::start().await;
      mount_paginated(
        &esi_server,
        "/characters/42/killmails/recent/",
        serde_json::json!([{"killmail_id": 200, "killmail_hash": "losshash"}]),
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
        "/killID/200/",
        serde_json::json!([{"killmail_id": 200, "zkb": {"hash": "losshash", "totalValue": 9876.5}}]),
      )
      .await;

      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test_with_scopes("token", 42, vec![scopes::CHARACTER_KILLMAILS.to_owned()]);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      let rows = character::killmails(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      let loss = &rows[0];
      assert!(!loss.is_kill());
      assert_eq!(loss.value_source(), "zkill");
      assert_eq!(loss.value_isk(), 9876.5);
    }
  }
}
