use std::collections::HashMap;

use crate::{
  clients::{Error, esi::models::universe::NameRecord},
  store::{
    model::{CharacterStanding, Faction},
    repo::{character, sde},
  },
  sync::{job::JobCtx, jobs::names::resolve_names, outcome::Outcome, subject::Subject},
};

const FROM_TYPE_FACTION: &str = "faction";

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character standings job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Ok(Outcome::NotReady);
  }
  let authenticated = ctx.esi.character_authenticated(grant);

  let standings = authenticated.standings().await?;

  let resolver_ids: Vec<i64> = standings
    .iter()
    .filter(|standing| standing.from_type != FROM_TYPE_FACTION)
    .map(|standing| standing.from_id)
    .collect();
  let resolved = resolve_names(ctx, &resolver_ids).await?;

  let mut rows = Vec::with_capacity(standings.len());
  for standing in standings {
    let from_name = if standing.from_type == FROM_TYPE_FACTION {
      resolve_faction(ctx, standing.from_id).await?.name().clone()
    } else {
      resolved_name(&resolved, standing.from_id)
    };
    rows.push(CharacterStanding {
      character_id,
      from_id: standing.from_id,
      from_name,
      from_type: standing.from_type,
      standing: standing.standing,
    });
  }

  character::replace_standings_for_character(ctx.db, character_id, &rows).await?;
  Ok(Outcome::from_rows(rows.len()))
}

fn resolved_name(resolved: &HashMap<i64, NameRecord>, id: i64) -> String {
  resolved
    .get(&id)
    .map(|record| record.name.clone())
    .unwrap_or_else(|| format!("Unknown ({id})"))
}

async fn resolve_faction(ctx: &JobCtx<'_>, faction_id: i64) -> Result<Faction, Error> {
  if let Some(faction) = sde::get_faction(ctx.db, faction_id).await? {
    return Ok(faction);
  }
  let faction = ctx
    .esi
    .faction()
    .list()
    .await?
    .into_iter()
    .find(|faction| faction.faction_id == faction_id)
    .map(Faction::from)
    .ok_or_else(|| Error::Internal(format!("faction {faction_id} not in /universe/factions")))?;
  sde::upsert_faction(ctx.db, &faction).await?;
  Ok(faction)
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

  async fn mount_standings(server: &MockServer, character_id: i64) {
    mount_json(
      server,
      &format!("/characters/{character_id}/standings/"),
      serde_json::json!([
        { "from_id": 500_003, "from_type": "faction", "standing": 7.5 },
        { "from_id": 1_000_125, "from_type": "npc_corp", "standing": -2.5 },
        { "from_id": 3_018_900, "from_type": "agent", "standing": 1.25 },
      ]),
    )
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
      key: JobKey::new(JobKind::CharacterStandings, Subject::Character(character_id)),
      grant: Some(grant),
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_standings_resolving_faction_from_sde_and_others_from_the_resolver() {
      let server = MockServer::start().await;
      mount_standings(&server, 42).await;
      mount_json(
        &server,
        "/universe/factions/",
        serde_json::json!([
          { "description": "The Amarr Empire.", "faction_id": 500_003, "is_unique": true, "name": "Amarr Empire",
            "size_factor": 5.0, "station_count": 1000, "station_system_count": 500 },
        ]),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "corporation", "id": 1_000_125, "name": "CONCORD" },
          { "category": "character", "id": 3_018_900, "name": "Some Agent" },
        ]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      let standings = character::standings(&db, 42).await.unwrap();
      assert_eq!(standings.len(), 3);
      let faction = standings.iter().find(|s| s.from_type() == "faction").unwrap();
      assert_eq!(faction.from_name(), "Amarr Empire");
      let corp = standings.iter().find(|s| s.from_type() == "npc_corp").unwrap();
      assert_eq!(corp.from_name(), "CONCORD");
      let agent = standings.iter().find(|s| s.from_type() == "agent").unwrap();
      assert_eq!(agent.from_name(), "Some Agent");
      assert!(sde::get_faction(&db, 500_003).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_resolves_a_faction_from_the_db_without_refetching() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/characters/42/standings/",
        serde_json::json!([{ "from_id": 500_003, "from_type": "faction", "standing": 5.0 }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path("/universe/factions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      sde::upsert_faction(&db, &Faction::new(500_003, "Amarr Empire", true, 5.0, 1000, 500))
        .await
        .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      let standings = character::standings(&db, 42).await.unwrap();
      assert_eq!(standings[0].from_name(), "Amarr Empire");
    }

    #[tokio::test]
    async fn it_aborts_without_writing_when_the_standings_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/standings/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(character::standings(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_aborts_without_writing_when_the_name_resolution_fails() {
      let server = MockServer::start().await;
      mount_standings(&server, 42).await;
      mount_json(
        &server,
        "/universe/factions/",
        serde_json::json!([
          { "description": "The Amarr Empire.", "faction_id": 500_003, "is_unique": true, "name": "Amarr Empire",
            "size_factor": 5.0, "station_count": 1000, "station_system_count": 500 },
        ]),
      )
      .await;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(character::standings(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/standings/"))
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
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      assert!(character::standings(&db, 42).await.unwrap().is_empty());
    }
  }
}
