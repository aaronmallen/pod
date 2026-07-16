use crate::{
  clients::Error,
  store::{
    model::CorporationMemberRole,
    repo::{character, org},
  },
  sync::{job::JobCtx, outcome::Outcome, subject::Subject},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character roles job for {character_id} requires a grant"
    )));
  };
  let Some(character) = character::get(ctx.db, character_id).await? else {
    return Err(Error::NotReady);
  };
  let corporation_id = character.corporation_id();

  let roles = ctx.esi.character_authenticated(grant).roles().await?;

  let rows: Vec<CorporationMemberRole> = roles
    .roles
    .into_iter()
    .map(|role| CorporationMemberRole::from((corporation_id, character_id, role)))
    .collect();
  org::replace_for_corporation(ctx.db, corporation_id, &rows).await?;
  Ok(Outcome::from_rows(rows.len()))
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

  async fn mount_roles(server: &MockServer, character_id: i64, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/characters/{character_id}/roles/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn seed_character(db: &store::Database, id: i64, corp_id: i64) {
    use store::{
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::character,
    };
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
      key: JobKey::new(JobKind::CharacterRoles, Subject::Character(character_id)),
      grant: Some(grant),
      sso: None,
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_the_characters_own_corp_roles_keyed_to_the_character() {
      let corp_id = 90_000_001;
      let server = MockServer::start().await;
      mount_roles(
        &server,
        42,
        serde_json::json!({ "roles": ["Director", "Station_Manager"] }),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, corp_id).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      let roles = org::for_corporation(&db, corp_id).await.unwrap();
      let mut names: Vec<&str> = roles.iter().map(|r| r.role().as_str()).collect();
      names.sort_unstable();
      assert_eq!(names, ["Director", "Station_Manager"]);
      assert!(roles.iter().all(|r| r.character_id() == 42));
    }

    #[tokio::test]
    async fn it_leaves_a_second_characters_roles_intact() {
      let corp_id = 90_000_001;
      let server = MockServer::start().await;
      mount_roles(&server, 42, serde_json::json!({ "roles": ["Director"] })).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, corp_id).await;
      org::replace_for_corporation(
        &db,
        corp_id,
        &[CorporationMemberRole::from((corp_id, 7, "Accountant".to_string()))],
      )
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

      let roles = org::for_corporation(&db, corp_id).await.unwrap();
      assert!(roles.iter().any(|r| r.character_id() == 7 && r.role() == "Accountant"));
      assert!(roles.iter().any(|r| r.character_id() == 42 && r.role() == "Director"));
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/roles/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "roles": [] })))
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

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
    }
  }
}
