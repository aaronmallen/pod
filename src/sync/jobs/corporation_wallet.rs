use reqwest::StatusCode;

use crate::{
  clients::{Error, esi::corporation::AuthenticatedClient, eve_sso::Grant},
  store::{
    model::{CorporationWalletDivision, CorporationWalletJournal, CorporationWalletTransaction, OwnerType},
    repo::{finance, infra, org},
  },
  sync::{job::JobCtx, outcome::Outcome, structure_resolution, subject::Subject},
};

const ACCOUNTING_ROLES: &[&str] = &["Director", "Accountant", "Junior_Accountant"];

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Corporation(corporation_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "corporation wallet job for {corporation_id} requires a grant"
    )));
  };
  if org::get_corporation(ctx.db, corporation_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authorized_by = authorizing_character(ctx, corporation_id).await?;
  verify_accounting_role(ctx, grant, corporation_id, authorized_by).await?;

  let authenticated = ctx.esi.corporation_authenticated(grant);

  let division_names = reauth_on_unauthorized(authenticated.divisions(corporation_id).await, corporation_id)?;
  let balances = reauth_on_unauthorized(authenticated.wallets(corporation_id).await, corporation_id)?;

  let mut divisions: Vec<CorporationWalletDivision> = division_names
    .wallet
    .into_iter()
    .map(|name| CorporationWalletDivision::from((corporation_id, name)))
    .collect();
  divisions.extend(
    balances
      .into_iter()
      .map(|balance| CorporationWalletDivision::from((corporation_id, balance))),
  );
  finance::upsert_divisions(ctx.db, &divisions).await?;

  let mut rows_touched = divisions.len();
  for division in 1..=7_i32 {
    rows_touched += sync_division(ctx, &authenticated, corporation_id, division).await?;
  }
  Ok(Outcome::from_rows(rows_touched))
}

async fn sync_division(
  ctx: &JobCtx<'_>,
  authenticated: &AuthenticatedClient<'_>,
  corporation_id: i64,
  division: i32,
) -> Result<usize, Error> {
  let division_key = i64::from(division);

  let journal = match authenticated.wallet_journal(corporation_id, division).await {
    Ok(entries) => entries,
    Err(error) if is_forbidden(&error) => {
      tracing::warn!(
        corporation_id,
        division,
        "skipping wallet journal: forbidden for this division"
      );
      return Ok(0);
    }
    Err(error) => return Err(error),
  };
  let journal: Vec<CorporationWalletJournal> = journal
    .into_iter()
    .map(|entry| CorporationWalletJournal::from((corporation_id, division_key, entry)))
    .collect();
  finance::append_corporation_wallet_journal(ctx.db, &journal).await?;

  let transactions = match authenticated.wallet_transactions(corporation_id, division).await {
    Ok(transactions) => transactions,
    Err(error) if is_forbidden(&error) => {
      tracing::warn!(
        corporation_id,
        division,
        "skipping wallet transactions: forbidden for this division"
      );
      return Ok(0);
    }
    Err(error) => return Err(error),
  };
  let transactions: Vec<CorporationWalletTransaction> = transactions
    .into_iter()
    .map(|transaction| CorporationWalletTransaction::from((corporation_id, division_key, transaction)))
    .collect();
  finance::append_corporation_wallet_transaction(ctx.db, &transactions).await?;

  let location_ids: Vec<i64> = transactions
    .iter()
    .map(CorporationWalletTransaction::location_id)
    .collect();
  structure_resolution::resolve_location_ids(ctx, &location_ids).await;

  Ok(journal.len() + transactions.len())
}

fn is_forbidden(error: &Error) -> bool {
  matches!(error, Error::Http(http) if http.status() == Some(StatusCode::FORBIDDEN))
}

fn is_unauthorized(error: &Error) -> bool {
  matches!(error, Error::Http(http) if http.status() == Some(StatusCode::UNAUTHORIZED))
}

fn reauth_on_unauthorized<T>(result: Result<T, Error>, corporation_id: i64) -> Result<T, Error> {
  result.map_err(|error| {
    if is_unauthorized(&error) {
      Error::Internal(format!(
        "corporation {corporation_id} credential was rejected (401); needs re-authentication"
      ))
    } else {
      error
    }
  })
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

async fn verify_accounting_role(
  ctx: &JobCtx<'_>,
  grant: &Grant,
  corporation_id: i64,
  authorized_by: i64,
) -> Result<(), Error> {
  let roles = ctx
    .esi
    .corporation_authenticated(grant)
    .member_roles(corporation_id)
    .await?;
  let holds_accounting_role = roles
    .iter()
    .find(|member| member.character_id == authorized_by)
    .is_some_and(|member| {
      member
        .roles
        .iter()
        .any(|role| ACCOUNTING_ROLES.contains(&role.as_str()))
    });
  if holds_accounting_role {
    Ok(())
  } else {
    Err(Error::Internal(format!(
      "authorizing character {authorized_by} no longer holds an accounting role in corporation \
      {corporation_id}; needs re-authentication"
    )))
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
    store::{self, images, model::Corporation, repo::org},
    sync::job::{JobKey, JobKind},
  };

  const CORP: i64 = 2000;

  const DIRECTOR: i64 = 100;

  async fn mount_json(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
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

  async fn mount_roles(server: &MockServer, body: serde_json::Value) {
    mount_json(server, &format!("/corporations/{CORP}/roles/"), body).await;
  }

  async fn mount_wallets(server: &MockServer) {
    mount_json(
      server,
      &format!("/corporations/{CORP}/divisions/"),
      serde_json::json!({
        "wallet": [{ "division": 1, "name": "Master Wallet" }, { "division": 2, "name": "Logistics" }]
      }),
    )
    .await;
    mount_json(
      server,
      &format!("/corporations/{CORP}/wallets/"),
      serde_json::json!([{ "division": 1, "balance": 1234.56 }, { "division": 2, "balance": 0.0 }]),
    )
    .await;
  }

  async fn mount_division_ledgers(server: &MockServer) {
    for division in 1..=7 {
      mount_paginated(
        server,
        &format!("/corporations/{CORP}/wallets/{division}/journal/"),
        serde_json::json!([
          { "amount": -1000.0, "balance": 5000.0, "date": "2026-05-30T12:00:00Z",
            "description": "Market escrow", "id": 100 + division, "ref_type": "market_escrow" },
        ]),
      )
      .await;
      mount_json(
        server,
        &format!("/corporations/{CORP}/wallets/{division}/transactions/"),
        serde_json::json!([
          { "client_id": 1000035, "date": "2026-05-30T12:00:00Z", "is_buy": true,
            "journal_ref_id": 555, "location_id": 60003760, "quantity": 10,
            "transaction_id": 9000 + division, "type_id": 34, "unit_price": 5.5 },
        ]),
      )
      .await;
    }
  }

  async fn mount_shared_id_division_ledgers(server: &MockServer, shared_id: i64) {
    for division in 1..=7 {
      let amount = if division == 1 {
        -shared_id as f64
      } else {
        shared_id as f64
      };
      mount_paginated(
        server,
        &format!("/corporations/{CORP}/wallets/{division}/journal/"),
        serde_json::json!([
          { "amount": amount, "balance": 5000.0, "date": "2026-05-30T12:00:00Z",
            "description": "Internal transfer", "id": shared_id,
            "ref_type": "corporation_account_withdrawal" },
        ]),
      )
      .await;
      mount_json(
        server,
        &format!("/corporations/{CORP}/wallets/{division}/transactions/"),
        serde_json::json!([]),
      )
      .await;
    }
  }

  async fn seed_corp(db: &store::Database) {
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

  fn ctx<'a>(
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
      key: JobKey::new(JobKind::CorporationWallet, Subject::Corporation(CORP)),
      grant: Some(grant),
      sso: None,
    }
  }

  mod run {
    use super::*;

    #[tokio::test]
    async fn it_errors_and_persists_nothing_when_the_director_lost_the_role() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Hangar_Take_1"] }]),
      )
      .await;
      mount_wallets(&server).await;
      mount_division_ledgers(&server).await;
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      seed_credential(&db).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", CORP);

      let result = run(&ctx(&db, &esi, &image, &image_store, &grant)).await;

      assert!(
        matches!(&result, Err(Error::Internal(message)) if message.contains("needs re-authentication")),
        "expected a re-authentication error, got {result:?}"
      );
      assert!(finance::divisions(&db, CORP).await.unwrap().is_empty());
      assert!(
        finance::corporation_wallet_journal(&db, CORP, 1)
          .await
          .unwrap()
          .is_empty()
      );
    }

    #[tokio::test]
    async fn it_persists_both_legs_of_an_internal_transfer_sharing_one_eve_id_across_divisions() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Director"] }]),
      )
      .await;
      mount_wallets(&server).await;
      mount_shared_id_division_ledgers(&server, 777).await;
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      seed_credential(&db).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", CORP);

      run(&ctx(&db, &esi, &image, &image_store, &grant)).await.unwrap();

      let leg_one = finance::corporation_wallet_journal(&db, CORP, 1).await.unwrap();
      let leg_two = finance::corporation_wallet_journal(&db, CORP, 2).await.unwrap();

      assert_eq!(
        leg_one.len(),
        1,
        "division 1's leg of the transfer must persist under the per-wallet key"
      );
      assert_eq!(
        leg_two.len(),
        1,
        "division 2's leg shares the same EVE id but a different division, so it must persist too, \
        not collide and drop"
      );
      assert_eq!(leg_one[0].id(), leg_two[0].id());
      assert_eq!(leg_one[0].amount(), Some(-777.0));
      assert_eq!(leg_two[0].amount(), Some(777.0));
    }

    #[tokio::test]
    async fn it_persists_per_division_balances_journal_and_transactions_when_the_role_is_held() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Accountant"] }]),
      )
      .await;
      mount_wallets(&server).await;
      mount_division_ledgers(&server).await;
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      seed_credential(&db).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", CORP);

      run(&ctx(&db, &esi, &image, &image_store, &grant)).await.unwrap();

      let divisions = finance::divisions(&db, CORP).await.unwrap();
      assert_eq!(divisions.len(), 2);
      assert_eq!(divisions[0].division(), 1);
      assert_eq!(divisions[0].balance(), Some(1234.56));
      assert_eq!(divisions[0].name(), &Some("Master Wallet".to_owned()));

      assert_eq!(
        finance::corporation_wallet_journal(&db, CORP, 1).await.unwrap().len(),
        1
      );
      assert_eq!(
        finance::corporation_wallet_transactions(&db, CORP, 1)
          .await
          .unwrap()
          .len(),
        1
      );
      assert_eq!(
        finance::corporation_wallet_journal(&db, CORP, 7).await.unwrap().len(),
        1
      );
    }

    #[tokio::test]
    async fn it_recovers_a_collided_leg_idempotently_when_the_forced_re_fetch_re_runs() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Director"] }]),
      )
      .await;
      mount_wallets(&server).await;
      mount_shared_id_division_ledgers(&server, 888).await;
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      seed_credential(&db).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", CORP);

      run(&ctx(&db, &esi, &image, &image_store, &grant)).await.unwrap();
      run(&ctx(&db, &esi, &image, &image_store, &grant)).await.unwrap();

      assert_eq!(
        finance::corporation_wallet_journal(&db, CORP, 1).await.unwrap().len(),
        1,
        "re-running the wallet sync (the forced re-fetch) re-appends the same paginated journal but \
        DO NOTHING keeps it a no-op; the leg stays exactly once"
      );
      assert_eq!(
        finance::corporation_wallet_journal(&db, CORP, 2).await.unwrap().len(),
        1,
        "the second leg recovered by the re-fetch is idempotent across repeated syncs"
      );
    }

    #[tokio::test]
    async fn it_skips_a_division_with_a_warning_when_its_journal_is_forbidden() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Junior_Accountant"] }]),
      )
      .await;
      mount_wallets(&server).await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/wallets/3/journal/")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      mount_division_ledgers(&server).await;
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      seed_credential(&db).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", CORP);

      run(&ctx(&db, &esi, &image, &image_store, &grant)).await.unwrap();

      assert!(
        finance::corporation_wallet_journal(&db, CORP, 3)
          .await
          .unwrap()
          .is_empty()
      );
      assert_eq!(
        finance::corporation_wallet_journal(&db, CORP, 1).await.unwrap().len(),
        1
      );
      assert_eq!(
        finance::corporation_wallet_transactions(&db, CORP, 2)
          .await
          .unwrap()
          .len(),
        1
      );
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_corporation_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/roles/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_credential(&db).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", CORP);

      let result = run(&ctx(&db, &esi, &image, &image_store, &grant)).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
      assert!(finance::divisions(&db, CORP).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_surfaces_a_401_on_divisions_as_needs_reauthentication() {
      let server = MockServer::start().await;
      mount_roles(
        &server,
        serde_json::json!([{ "character_id": DIRECTOR, "roles": ["Accountant"] }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path(format!("/corporations/{CORP}/divisions/")))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      seed_credential(&db).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", CORP);

      let result = run(&ctx(&db, &esi, &image, &image_store, &grant)).await;

      assert!(
        matches!(&result, Err(Error::Internal(message)) if message.contains("needs re-authentication")),
        "expected a re-authentication error, got {result:?}"
      );
      assert!(finance::divisions(&db, CORP).await.unwrap().is_empty());
    }
  }
}
