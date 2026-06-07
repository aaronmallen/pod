use crate::{
  clients::Error,
  store::{
    images,
    model::{Alliance, Bloodline, Character, Corporation, Faction, Race},
    repo::{character, sde},
  },
  sync::{job::JobCtx, outcome::Outcome, subject::Subject},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };

  let info = ctx.esi.character().public_info(character_id).await?;
  let alliance_id = info.alliance_id;
  let bloodline_id = info.bloodline_id;
  let corporation_id = info.corporation_id;
  let faction_id = info.faction_id;
  let race_id = info.race_id;
  let character = Character::from((character_id, info));

  let corporation = Corporation::from((corporation_id, ctx.esi.corporation().info(corporation_id).await?));

  let alliance = match alliance_id {
    Some(id) => Some(Alliance::from((id, ctx.esi.alliance().info(id).await?))),
    None => None,
  };
  let faction = match faction_id {
    Some(id) => Some(resolve_faction(ctx, id).await?),
    None => None,
  };
  let race = resolve_race(ctx, race_id).await?;
  let bloodline = resolve_bloodline(ctx, bloodline_id).await?;

  let portrait_url = ctx.image.character_portrait_url(character_id, images::PORTRAIT_SIZE);
  let portrait = ctx.image.fetch(&portrait_url).await?;
  let portrait_path = ctx.image_store.character_portrait_path(character_id);
  ctx
    .image_store
    .write(&portrait_path, &portrait)
    .map_err(|error| Error::Internal(format!("write portrait for character {character_id}: {error}")))?;

  character::upsert_with_org(
    ctx.db,
    &character,
    &bloodline,
    &race,
    &corporation,
    alliance.as_ref(),
    faction.as_ref(),
  )
  .await?;
  Ok(Outcome::Synced {
    rows_touched: 1,
  })
}

async fn resolve_bloodline(ctx: &JobCtx<'_>, bloodline_id: i32) -> Result<Bloodline, Error> {
  if let Some(bloodline) = sde::get_bloodline(ctx.db, i64::from(bloodline_id)).await? {
    return Ok(bloodline);
  }
  ctx
    .esi
    .bloodlines()
    .list()
    .await?
    .into_iter()
    .find(|bloodline| bloodline.bloodline_id == bloodline_id)
    .map(Bloodline::from)
    .ok_or_else(|| Error::Internal(format!("bloodline {bloodline_id} not in /universe/bloodlines")))
}

async fn resolve_faction(ctx: &JobCtx<'_>, faction_id: i64) -> Result<Faction, Error> {
  if let Some(faction) = sde::get_faction(ctx.db, faction_id).await? {
    return Ok(faction);
  }
  ctx
    .esi
    .faction()
    .list()
    .await?
    .into_iter()
    .find(|faction| faction.faction_id == faction_id)
    .map(Faction::from)
    .ok_or_else(|| Error::Internal(format!("faction {faction_id} not in /universe/factions")))
}

async fn resolve_race(ctx: &JobCtx<'_>, race_id: i32) -> Result<Race, Error> {
  if let Some(race) = sde::get_race(ctx.db, i64::from(race_id)).await? {
    return Ok(race);
  }
  ctx
    .esi
    .races()
    .list()
    .await?
    .into_iter()
    .find(|race| race.race_id == race_id)
    .map(Race::from)
    .ok_or_else(|| Error::Internal(format!("race {race_id} not in /universe/races")))
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
    store,
    sync::job::{JobKey, JobKind},
  };

  async fn mount_json(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn seed_ship_type(db: &store::Database) {
    sqlx::query("INSERT INTO item_categories (id, name, published) VALUES (6, 'Ship', 1)")
      .execute(&db.0)
      .await
      .unwrap();
    sqlx::query("INSERT INTO item_groups (id, category_id, name, published) VALUES (25, 6, 'Frigate', 1)")
      .execute(&db.0)
      .await
      .unwrap();
    sqlx::query(
      "INSERT INTO item_types (id, group_id, description, name, published) VALUES (601, 25, 'Merlin', 'Merlin', 1)",
    )
    .execute(&db.0)
    .await
    .unwrap();
  }

  mod run {
    use super::*;

    #[tokio::test]
    async fn it_errors_and_persists_nothing_when_the_character_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v5/characters/42/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = JobCtx {
        db: &db,
        esi: &esi,
        image: &image,
        image_store: &image_store,
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(42)),
        grant: None,
      };

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(character::get(&db, 42).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_resolves_reference_data_from_the_db_on_resync() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/v5/characters/100/",
        serde_json::json!({
          "alliance_id": 300, "birthday": "2010-01-01T00:00:00Z", "bloodline_id": 5,
          "corporation_id": 200, "gender": "male", "name": "Test Pilot", "race_id": 1,
        }),
      )
      .await;
      mount_json(
        &server,
        "/v5/corporations/200/",
        serde_json::json!({
          "alliance_id": 300, "ceo_id": 100, "creator_id": 100, "member_count": 42,
          "name": "Test Corp", "tax_rate": 0.1, "ticker": "TST",
        }),
      )
      .await;
      mount_json(
        &server,
        "/v4/alliances/300/",
        serde_json::json!({
          "creator_corporation_id": 200, "creator_id": 100,
          "date_founded": "2005-01-01T00:00:00Z", "name": "Test Alliance", "ticker": "TSTA",
        }),
      )
      .await;
      Mock::given(method("GET"))
        .and(path("/v1/universe/races/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          { "alliance_id": 300, "description": "The Caldari.", "name": "Caldari", "race_id": 1 },
        ])))
        .expect(1)
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/v1/universe/bloodlines/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          { "bloodline_id": 5, "charisma": 6, "corporation_id": 200, "description": "The Civire.",
            "intelligence": 7, "memory": 5, "name": "Civire", "perception": 5, "race_id": 1,
            "ship_type_id": 601, "willpower": 5 },
        ])))
        .expect(1)
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/characters/100/portrait"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(vec![9u8, 9, 9], "image/jpeg"))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_ship_type(&db).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = JobCtx {
        db: &db,
        esi: &esi,
        image: &image,
        image_store: &image_store,
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(100)),
        grant: None,
      };

      run(&ctx).await.unwrap();
      run(&ctx).await.unwrap();

      assert!(character::get(&db, 100).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_errors_and_persists_nothing_when_the_portrait_fetch_fails() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/v5/characters/100/",
        serde_json::json!({
          "alliance_id": 300, "birthday": "2010-01-01T00:00:00Z", "bloodline_id": 5,
          "corporation_id": 200, "gender": "male", "name": "Test Pilot", "race_id": 1,
        }),
      )
      .await;
      mount_json(
        &server,
        "/v5/corporations/200/",
        serde_json::json!({
          "alliance_id": 300, "ceo_id": 100, "creator_id": 100, "member_count": 42,
          "name": "Test Corp", "tax_rate": 0.1, "ticker": "TST",
        }),
      )
      .await;
      mount_json(
        &server,
        "/v4/alliances/300/",
        serde_json::json!({
          "creator_corporation_id": 200, "creator_id": 100,
          "date_founded": "2005-01-01T00:00:00Z", "name": "Test Alliance", "ticker": "TSTA",
        }),
      )
      .await;
      mount_json(
        &server,
        "/v1/universe/races/",
        serde_json::json!([
          { "alliance_id": 300, "description": "The Caldari.", "name": "Caldari", "race_id": 1 },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/v1/universe/bloodlines/",
        serde_json::json!([
          { "bloodline_id": 5, "charisma": 6, "corporation_id": 200, "description": "The Civire.",
            "intelligence": 7, "memory": 5, "name": "Civire", "perception": 5, "race_id": 1,
            "ship_type_id": 601, "willpower": 5 },
        ]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path("/characters/100/portrait"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_ship_type(&db).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = JobCtx {
        db: &db,
        esi: &esi,
        image: &image,
        image_store: &image_store,
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(100)),
        grant: None,
      };

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(character::get(&db, 100).await.unwrap().is_none());
    }
  }

  mod resolve_faction {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_the_faction_from_the_db_on_a_hit() {
      let db = store::open_test().await.unwrap();
      sde::upsert_faction(&db, &Faction::new(500_001, "Caldari State", true, 2.0, 100, 50))
        .await
        .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), "http://127.0.0.1:1");
      let image = eve_image::Client::with_base_url(http, "http://127.0.0.1:1");
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = JobCtx {
        db: &db,
        esi: &esi,
        image: &image,
        image_store: &image_store,
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(1)),
        grant: None,
      };

      let faction = resolve_faction(&ctx, 500_001).await.unwrap();

      assert_eq!(faction.id(), 500_001);
    }
  }
}
