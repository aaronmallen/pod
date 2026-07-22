use reqwest::StatusCode;

use crate::{
  clients::{Error, eve_sso::Grant},
  store::{
    model::{CorporationMarketOrder, OwnerType},
    repo::{finance, infra, org},
  },
  sync::{job::JobCtx, outcome::Outcome, subject::Subject},
};

/// Roles that grant access to `/corporations/{id}/orders`: Accountant and Trader are the specific gate, Director the
/// superset.
const MARKET_ORDER_ROLES: &[&str] = &["Director", "Accountant", "Trader"];

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  match ctx.key.subject {
    Subject::Character(character_id) => Err(Error::Internal(format!(
      "corporation market orders job received a character subject {character_id}"
    ))),
    Subject::Corporation(corporation_id) => run_corporation(ctx, corporation_id).await,
  }
}

async fn run_corporation(ctx: &JobCtx<'_>, corporation_id: i64) -> Result<Outcome, Error> {
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "corporation market orders job for {corporation_id} requires a grant"
    )));
  };
  if org::get_corporation(ctx.db, corporation_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authorized_by = authorizing_character(ctx, corporation_id).await?;
  if !holds_market_role(ctx, grant, corporation_id, authorized_by).await? {
    return Ok(Outcome::Skipped {
      reason: format!(
        "authorizing character {authorized_by} lacks a market-order role in corporation {corporation_id}"
      ),
    });
  }

  let fetched = match ctx.esi.corporation_authenticated(grant).orders(corporation_id).await {
    Ok(orders) => orders,
    Err(error) if is_forbidden(&error) => {
      tracing::warn!(
        corporation_id,
        "skipping corporation market orders: forbidden (Accountant or Trader role required)"
      );
      return Ok(Outcome::Skipped {
        reason: format!("corporation {corporation_id} orders are forbidden (missing market-order role)"),
      });
    }
    Err(error) => return Err(reauth_error(error, corporation_id)),
  };

  let orders: Vec<CorporationMarketOrder> = fetched
    .into_iter()
    .map(|order| CorporationMarketOrder::from((corporation_id, order)))
    .collect();
  finance::replace_orders_for_corporation(ctx.db, corporation_id, &orders).await?;
  Ok(Outcome::from_rows(orders.len()))
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

async fn holds_market_role(
  ctx: &JobCtx<'_>,
  grant: &Grant,
  corporation_id: i64,
  authorized_by: i64,
) -> Result<bool, Error> {
  let roles = ctx
    .esi
    .corporation_authenticated(grant)
    .member_roles(corporation_id)
    .await
    .map_err(|error| reauth_error(error, corporation_id))?;
  Ok(
    roles
      .iter()
      .find(|member| member.character_id == authorized_by)
      .is_some_and(|member| {
        member
          .roles
          .iter()
          .any(|role| MARKET_ORDER_ROLES.contains(&role.as_str()))
      }),
  )
}

fn is_forbidden(error: &Error) -> bool {
  matches!(error, Error::Http(http) if http.status() == Some(StatusCode::FORBIDDEN))
}

fn is_unauthorized(error: &Error) -> bool {
  matches!(error, Error::Http(http) if http.status() == Some(StatusCode::UNAUTHORIZED))
}

fn reauth_error(error: Error, corporation_id: i64) -> Error {
  if is_unauthorized(&error) {
    Error::Internal(format!(
      "corporation {corporation_id} credential was rejected (401); needs re-authentication"
    ))
  } else {
    error
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
    clients::{esi, esi::scopes::CORPORATION_ROLES, eve_image, eve_sso::Grant, http},
    store::{
      self, images,
      model::{Corporation, CorporationMemberRole},
    },
    sync::job::{JobKey, JobKind},
  };

  const CORP: i64 = 90_000_001;

  const DIRECTOR: i64 = 100;

  async fn seed_corporation(db: &store::Database) {
    let mut corporation = Corporation::new(CORP, "Test Corp", "TST");
    corporation.set_ceo_id(DIRECTOR);
    corporation.set_creator_id(DIRECTOR);
    corporation.set_member_count(42);
    corporation.set_tax_rate(0.1);
    org::upsert_corporation(db, &corporation).await.unwrap();
  }

  async fn seed_credential(db: &store::Database) {
    infra::upsert(
      db,
      CORP,
      OwnerType::Corporation,
      "tok",
      "rt",
      4_102_444_800,
      Some(DIRECTOR),
      Some(CORPORATION_ROLES),
    )
    .await
    .unwrap();
  }

  async fn authorize(db: &store::Database, role: &str) {
    seed_corporation(db).await;
    seed_credential(db).await;
    org::replace_for_corporation(
      db,
      CORP,
      &[CorporationMemberRole::from((CORP, DIRECTOR, role.to_owned()))],
    )
    .await
    .unwrap();
  }

  async fn mount_roles(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/corporations/{CORP}/roles/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn mount_orders(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/corporations/{CORP}/orders/")))
      .respond_with(
        ResponseTemplate::new(200)
          .insert_header("X-Pages", "1")
          .set_body_json(body),
      )
      .mount(server)
      .await;
  }

  fn order_json(order_id: i64) -> serde_json::Value {
    serde_json::json!({
      "order_id": order_id, "type_id": 34, "region_id": 10_000_002, "location_id": 60_003_760,
      "range": "region", "is_buy_order": false, "price": 5.5, "volume_remain": 100, "volume_total": 200,
      "escrow": 0.0, "duration": 90, "issued": "2026-06-01T12:00:00Z", "wallet_division": 1,
    })
  }

  async fn build_clients(
    db: &store::Database,
    server: &MockServer,
  ) -> (esi::Client, eve_image::Client, images::Store, tempfile::TempDir) {
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = esi::Client::with_base_url(http.clone(), server.uri());
    let image = eve_image::Client::with_base_url(http, server.uri());
    let images_dir = tempfile::tempdir().unwrap();
    let image_store = images::Store::new(images_dir.path().to_path_buf());
    (esi, image, image_store, images_dir)
  }

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: &'a Grant,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::CorporationMarketOrders, Subject::Corporation(CORP)),
      grant: Some(grant),
      sso: None,
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_rejects_a_character_subject() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let mut ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant);
      ctx.key = JobKey::new(JobKind::CorporationMarketOrders, Subject::Character(7));

      let result = run(&ctx).await;

      assert!(matches!(result, Err(Error::Internal(_))));
    }

    #[tokio::test]
    async fn it_persists_the_corporations_open_orders_when_the_role_is_held() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Trader"] }]),
      )
      .await;
      mount_orders(&server, serde_json::json!([order_json(1001)])).await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Trader").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant);

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 1
        }
      );
      let stored = finance::open_for_corporation(&db, CORP).await.unwrap();
      assert_eq!(stored.len(), 1);
      assert_eq!(stored[0].order_id(), 1001);
      assert_eq!(stored[0].state(), "open");
    }

    #[tokio::test]
    async fn it_short_retries_when_the_corporation_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/orders/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant);

      let result = run(&ctx).await;

      assert!(matches!(result, Err(Error::NotReady)));
    }

    #[tokio::test]
    async fn it_skips_honestly_when_the_authorizing_character_lacks_the_role() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Hangar_Take_1"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/orders/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Hangar_Take_1").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant);

      let outcome = run(&ctx).await.unwrap();

      assert!(
        matches!(outcome, Outcome::Skipped { .. }),
        "a missing market-order role is an honest skip, got {outcome:?}"
      );
      assert!(finance::open_for_corporation(&db, CORP).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_skips_honestly_when_the_orders_endpoint_is_forbidden() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Accountant"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/orders/")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Accountant").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant);

      let outcome = run(&ctx).await.unwrap();

      assert!(
        matches!(outcome, Outcome::Skipped { .. }),
        "a 403 from the orders endpoint is an honest skip, got {outcome:?}"
      );
    }

    #[tokio::test]
    async fn it_surfaces_a_401_as_needs_reauthentication() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Director"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/orders/")))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      authorize(&db, "Director").await;
      let (esi, image, image_store, _dir) = build_clients(&db, &server).await;
      let grant = Grant::new_test("corp-token", CORP);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant);

      let result = run(&ctx).await;

      assert!(
        matches!(&result, Err(Error::Internal(message)) if message.contains("needs re-authentication")),
        "expected a re-authentication error, got {result:?}"
      );
    }
  }
}
