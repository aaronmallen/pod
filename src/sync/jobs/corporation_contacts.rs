use std::collections::HashMap;

use crate::{
  clients::{Error, esi::models::universe::NameRecord},
  store::{
    model::{CorporationContact, CorporationContactLabel, Faction},
    repo::{org, sde},
  },
  sync::{job::JobCtx, jobs::names::resolve_names, outcome::Outcome, subject::Subject},
};

const CONTACT_TYPE_FACTION: &str = "faction";

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Corporation(corporation_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "corporation contacts job for {corporation_id} requires a grant"
    )));
  };
  if org::get_corporation(ctx.db, corporation_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authenticated = ctx.esi.corporation_authenticated(grant);

  let contacts = authenticated.contacts(corporation_id).await?;
  let label_entries = authenticated.contact_labels(corporation_id).await?;

  let resolver_ids: Vec<i64> = contacts
    .iter()
    .filter(|contact| contact.contact_type != CONTACT_TYPE_FACTION)
    .map(|contact| contact.contact_id)
    .collect();
  let resolved = resolve_names(ctx, &resolver_ids).await?;

  let mut rows = Vec::with_capacity(contacts.len());
  for contact in contacts {
    let contact_name = if contact.contact_type == CONTACT_TYPE_FACTION {
      resolve_faction(ctx, contact.contact_id).await?.name().clone()
    } else {
      resolved_name(&resolved, contact.contact_id)
    };
    let label_ids = serde_json::to_string(&contact.label_ids)
      .map_err(|error| Error::Internal(format!("serialize contact {} label_ids: {error}", contact.contact_id)))?;
    rows.push(CorporationContact {
      corporation_id,
      contact_id: contact.contact_id,
      contact_name,
      contact_type: contact.contact_type,
      is_blocked: contact.is_blocked.unwrap_or(false),
      is_watched: contact.is_watched.unwrap_or(false),
      label_ids,
      standing: contact.standing.unwrap_or(0.0),
    });
  }

  let labels: Vec<CorporationContactLabel> = label_entries
    .into_iter()
    .map(|label| CorporationContactLabel {
      corporation_id,
      label_id: label.label_id,
      label_name: label.label_name,
    })
    .collect();

  org::replace_contacts_for_corporation(ctx.db, corporation_id, &rows).await?;
  org::replace_labels_for_corporation(ctx.db, corporation_id, &labels).await?;
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

  async fn mount_contacts(server: &MockServer, corporation_id: i64) {
    mount_json(
      server,
      &format!("/corporations/{corporation_id}/contacts/"),
      serde_json::json!([
        { "contact_id": 95_001, "contact_type": "character", "is_watched": true, "label_ids": [1], "standing": 7.5 },
        { "contact_id": 98_001, "contact_type": "corporation", "standing": -10.0 },
      ]),
    )
    .await;
  }

  async fn mount_labels(server: &MockServer, corporation_id: i64) {
    mount_json(
      server,
      &format!("/corporations/{corporation_id}/contacts/labels/"),
      serde_json::json!([
        { "label_id": 1, "label_name": "Friendlies" },
        { "label_id": 2, "label_name": "Watchlist" },
      ]),
    )
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

  async fn fetch_contacts(db: &store::Database, corporation_id: i64) -> Vec<CorporationContact> {
    sqlx::query_as::<_, CorporationContact>(
      "SELECT corporation_id, contact_id, contact_name, contact_type, is_blocked, is_watched, label_ids, standing \
      FROM corporation_contacts WHERE corporation_id = ? ORDER BY contact_id",
    )
    .bind(corporation_id)
    .fetch_all(&db.0)
    .await
    .unwrap()
  }

  async fn fetch_labels(db: &store::Database, corporation_id: i64) -> Vec<CorporationContactLabel> {
    sqlx::query_as::<_, CorporationContactLabel>(
      "SELECT corporation_id, label_id, label_name FROM corporation_contact_labels \
      WHERE corporation_id = ? ORDER BY label_id",
    )
    .bind(corporation_id)
    .fetch_all(&db.0)
    .await
    .unwrap()
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
      key: JobKey::new(JobKind::CorporationContacts, Subject::Corporation(corporation_id)),
      grant: Some(grant),
      sso: None,
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_empty_when_the_corporation_has_no_contacts() {
      let server = MockServer::start().await;
      mount_json(&server, "/corporations/2000/contacts/", serde_json::json!([])).await;
      mount_json(&server, "/corporations/2000/contacts/labels/", serde_json::json!([])).await;
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

      assert_eq!(
        outcome,
        Outcome::Empty,
        "a corporation with no contacts reads as Empty, not green"
      );
    }

    #[tokio::test]
    async fn it_persists_contacts_and_labels_with_resolved_names() {
      let server = MockServer::start().await;
      mount_contacts(&server, 2000).await;
      mount_labels(&server, 2000).await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "character", "id": 95_001, "name": "Trusted Pilot" },
          { "category": "corporation", "id": 98_001, "name": "Hostile Corp" },
        ]),
      )
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
      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 2
        }
      );

      let contacts = fetch_contacts(&db, 2000).await;
      assert_eq!(
        contacts.iter().map(|c| c.contact_id()).collect::<Vec<_>>(),
        [95_001, 98_001]
      );
      let trusted = &contacts[0];
      assert_eq!(trusted.contact_name(), "Trusted Pilot");
      assert!(trusted.is_watched());
      assert_eq!(trusted.label_ids(), "[1]");
      let hostile = &contacts[1];
      assert_eq!(hostile.contact_name(), "Hostile Corp");
      assert!(!hostile.is_watched());
      assert_eq!(hostile.label_ids(), "[]");
      let labels = fetch_labels(&db, 2000).await;
      assert_eq!(
        labels.iter().map(|l| l.label_name().as_str()).collect::<Vec<_>>(),
        ["Friendlies", "Watchlist"]
      );
    }

    #[tokio::test]
    async fn it_resolves_a_faction_contact_name_from_sde() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/corporations/2000/contacts/",
        serde_json::json!([{ "contact_id": 500_003, "contact_type": "faction", "standing": 5.0 }]),
      )
      .await;
      mount_json(&server, "/corporations/2000/contacts/labels/", serde_json::json!([])).await;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      mount_json(
        &server,
        "/universe/factions/",
        serde_json::json!([
          { "description": "The Amarr Empire.", "faction_id": 500_003, "is_unique": true, "name": "Amarr Empire",
            "size_factor": 5.0, "station_count": 1000, "station_system_count": 500 },
        ]),
      )
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

      run(&ctx).await.unwrap();

      let contacts = fetch_contacts(&db, 2000).await;
      assert_eq!(contacts[0].contact_name(), "Amarr Empire");
      assert!(sde::get_faction(&db, 500_003).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_aborts_without_writing_when_the_contacts_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/corporations/2000/contacts/"))
        .respond_with(ResponseTemplate::new(500))
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

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(fetch_contacts(&db, 2000).await.is_empty());
      assert!(fetch_labels(&db, 2000).await.is_empty());
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_corporation_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/corporations/2000/contacts/"))
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
      let grant = Grant::new_test("corp-token", 2000);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 2000);

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
      assert!(fetch_contacts(&db, 2000).await.is_empty());
    }
  }
}
