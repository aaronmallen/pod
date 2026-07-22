use crate::{
  clients::{Error, eve_sso::Grant},
  store::{
    images,
    model::{Alliance, Corporation, CorporationMemberRole, OwnerType},
    repo::{infra, org},
  },
  sync::{job::JobCtx, outcome::Outcome, subject::Subject},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Corporation(corporation_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "corporation profile job for {corporation_id} requires a grant"
    )));
  };
  let authorized_by = authorizing_character(ctx, corporation_id).await?;

  let info = ctx.esi.corporation().info(corporation_id).await?;
  let alliance_id = info.alliance_id;
  let corporation = Corporation::from((corporation_id, info));

  let alliance = match alliance_id {
    Some(id) => Some(Alliance::from((id, ctx.esi.alliance().info(id).await?))),
    None => None,
  };

  let authorizing_roles = fetch_authorizing_roles(ctx, grant, corporation_id, authorized_by).await?;

  let logo_path = ctx.image_store.corporation_logo_path(corporation_id);
  if !images::is_fresh(&logo_path, images::STALE_AFTER) {
    let logo_url = ctx.image.corporation_logo_url(corporation_id, images::LOGO_SIZE);
    let logo = ctx.image.fetch(&logo_url).await?;
    ctx
      .image_store
      .write(&logo_path, &logo)
      .map_err(|error| Error::Internal(format!("write logo for corporation {corporation_id}: {error}")))?;
  }

  if let Some(alliance) = alliance.as_ref() {
    org::upsert_alliance(ctx.db, alliance).await?;
  }
  org::upsert_corporation(ctx.db, &corporation).await?;
  persist_authorizing_roles(ctx, corporation_id, authorized_by, &authorizing_roles).await?;
  Ok(Outcome::Synced {
    rows_touched: 1,
  })
}

async fn persist_authorizing_roles(
  ctx: &JobCtx<'_>,
  corporation_id: i64,
  authorized_by: i64,
  roles: &[String],
) -> Result<(), Error> {
  let rows: Vec<CorporationMemberRole> = roles
    .iter()
    .map(|role| CorporationMemberRole::from((corporation_id, authorized_by, role.clone())))
    .collect();
  org::replace_for_corporation(ctx.db, corporation_id, &rows).await?;
  Ok(())
}

async fn authorizing_character(ctx: &JobCtx<'_>, corporation_id: i64) -> Result<i64, Error> {
  let credential = infra::get(ctx.db, corporation_id, OwnerType::Corporation)
    .await?
    .ok_or_else(|| Error::Internal(format!("no corporation credential for {corporation_id}")))?;
  credential.authorized_by().ok_or_else(|| {
    Error::Internal(format!(
      "corporation credential for {corporation_id} has no authorizing character"
    ))
  })
}

async fn fetch_authorizing_roles(
  ctx: &JobCtx<'_>,
  grant: &Grant,
  corporation_id: i64,
  authorized_by: i64,
) -> Result<Vec<String>, Error> {
  let members = ctx
    .esi
    .corporation_authenticated(grant)
    .member_roles(corporation_id)
    .await?;
  Ok(
    members
      .into_iter()
      .find(|member| member.character_id == authorized_by)
      .map(|member| member.roles)
      .unwrap_or_default(),
  )
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, esi::scopes::CORPORATION_ROLES, eve_image, eve_sso::Grant, http},
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

  async fn mount_corporation(server: &MockServer, corporation_id: i64) {
    mount_json(
      server,
      &format!("/corporations/{corporation_id}/"),
      serde_json::json!({
        "ceo_id": 100, "creator_id": 100, "member_count": 42, "name": "Test Corp",
        "tax_rate": 0.1, "ticker": "TST",
      }),
    )
    .await;
  }

  async fn mount_logo(server: &MockServer, corporation_id: i64) {
    Mock::given(method("GET"))
      .and(path(format!("/corporations/{corporation_id}/logo")))
      .respond_with(ResponseTemplate::new(200).set_body_raw(vec![1u8, 2, 3], "image/png"))
      .mount(server)
      .await;
  }

  async fn mount_roles(server: &MockServer, corporation_id: i64, body: serde_json::Value) {
    mount_json(server, &format!("/corporations/{corporation_id}/roles/"), body).await;
  }

  async fn seed_credential(db: &store::Database, corporation_id: i64, director_id: i64) {
    infra::upsert(
      db,
      corporation_id,
      OwnerType::Corporation,
      "tok",
      "rt",
      4_102_444_800,
      Some(director_id),
      Some(CORPORATION_ROLES),
    )
    .await
    .unwrap();
  }

  fn ctx<'a>(
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
      key: JobKey::new(JobKind::CorporationProfile, Subject::Corporation(corporation_id)),
      grant: Some(grant),
      sso: None,
    }
  }

  mod run {
    use super::*;

    #[tokio::test]
    async fn it_errors_and_persists_nothing_when_the_corp_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/corporations/2000/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_credential(&db, 2000, 100).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", 2000);

      let result = run(&ctx(&db, &esi, &image, &image_store, &grant, 2000)).await;

      assert!(result.is_err());
      assert!(org::get_corporation(&db, 2000).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_persists_the_authorizing_roles_even_without_the_director_role() {
      let server = MockServer::start().await;
      mount_corporation(&server, 2000).await;
      mount_logo(&server, 2000).await;
      mount_roles(
        &server,
        2000,
        serde_json::json!([{ "character_id": 100, "roles": ["Station_Manager"] }]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_credential(&db, 2000, 100).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", 2000);

      run(&ctx(&db, &esi, &image, &image_store, &grant, 2000)).await.unwrap();

      assert!(org::get_corporation(&db, 2000).await.unwrap().is_some());
      let roles = org::for_corporation(&db, 2000).await.unwrap();
      let role_names: Vec<&str> = roles.iter().map(|r| r.role().as_str()).collect();
      assert_eq!(role_names, ["Station_Manager"], "the station manager roles persist");
    }

    #[tokio::test]
    async fn it_errors_and_persists_nothing_when_the_roles_endpoint_forbids() {
      let server = MockServer::start().await;
      mount_corporation(&server, 2000).await;
      Mock::given(method("GET"))
        .and(path("/corporations/2000/roles/"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_credential(&db, 2000, 100).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", 2000);

      let result = run(&ctx(&db, &esi, &image, &image_store, &grant, 2000)).await;

      assert!(matches!(result, Err(Error::Http(_))));
      assert!(org::get_corporation(&db, 2000).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_persists_the_alliance_so_the_corp_alliance_fk_is_satisfied() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/corporations/2000/",
        serde_json::json!({
          "alliance_id": 300, "ceo_id": 100, "creator_id": 100, "member_count": 42,
          "name": "Test Corp", "tax_rate": 0.1, "ticker": "TST",
        }),
      )
      .await;
      mount_json(
        &server,
        "/alliances/300/",
        serde_json::json!({
          "creator_corporation_id": 2000, "creator_id": 100,
          "date_founded": "2005-01-01T00:00:00Z", "name": "Test Alliance", "ticker": "TSTA",
        }),
      )
      .await;
      mount_logo(&server, 2000).await;
      mount_roles(
        &server,
        2000,
        serde_json::json!([{ "character_id": 100, "roles": ["Director"] }]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_credential(&db, 2000, 100).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", 2000);

      run(&ctx(&db, &esi, &image, &image_store, &grant, 2000)).await.unwrap();

      let corp = org::get_corporation(&db, 2000).await.unwrap().expect("corp persisted");
      assert_eq!(corp.alliance_id(), Some(300));
      assert!(org::get_alliance(&db, 300).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_persists_the_corp_identity_when_the_director_still_holds_the_role() {
      let server = MockServer::start().await;
      mount_corporation(&server, 2000).await;
      mount_logo(&server, 2000).await;
      mount_roles(
        &server,
        2000,
        serde_json::json!([{ "character_id": 100, "roles": ["Director", "Accountant"] }]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_credential(&db, 2000, 100).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", 2000);

      run(&ctx(&db, &esi, &image, &image_store, &grant, 2000)).await.unwrap();

      let corp = org::get_corporation(&db, 2000).await.unwrap().expect("corp persisted");
      assert_eq!(corp.name(), "Test Corp");
      assert_eq!(corp.ticker(), "TST");

      assert!(image_store.corporation_logo_path(2000).exists(), "logo written");

      let roles = org::for_corporation(&db, 2000).await.unwrap();
      let mut role_names: Vec<&str> = roles.iter().map(|r| r.role().as_str()).collect();
      role_names.sort_unstable();
      assert_eq!(role_names, ["Accountant", "Director"], "authorizing roles persisted");
      assert!(roles.iter().all(|r| r.character_id() == 100));
    }
  }
}
