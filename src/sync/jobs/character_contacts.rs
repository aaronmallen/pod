use std::collections::{HashMap, HashSet};

use crate::{
  clients::{Error, esi::models::universe::NameRecord},
  store::{
    model::{CharacterContact, CharacterContactLabel, Faction, OwnerType},
    repo::{character, infra, sde},
  },
  sync::{job::JobCtx, jobs::names::resolve_names, outcome::Outcome, subject::Subject},
};

const CONTACT_TYPE_FACTION: &str = "faction";

const OUTBOX_KINDS_CONTACT: [&str; 3] = ["contact.add", "contact.edit", "contact.remove"];

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character contacts job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authenticated = ctx.esi.character_authenticated(grant);

  let contacts = authenticated.contacts().await?;
  let label_entries = authenticated.contact_labels().await?;

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
    rows.push(CharacterContact {
      character_id,
      contact_id: contact.contact_id,
      contact_name,
      contact_type: contact.contact_type,
      is_blocked: contact.is_blocked.unwrap_or(false),
      is_watched: contact.is_watched.unwrap_or(false),
      label_ids,
      standing: contact.standing.unwrap_or(0.0),
    });
  }

  let labels: Vec<CharacterContactLabel> = label_entries
    .into_iter()
    .map(|label| CharacterContactLabel {
      character_id,
      label_id: label.label_id,
      label_name: label.label_name,
    })
    .collect();

  let protected = pending_contact_ids(ctx, character_id).await?;

  character::replace_contacts_for_character(ctx.db, character_id, &rows, &protected).await?;
  character::replace_labels_for_character(ctx.db, character_id, &labels).await?;
  Ok(Outcome::from_rows(rows.len()))
}

/// Contact ids with an unsent outbox mutation, so the full-replace sync does not clobber a local change in flight.
///
/// Each contact outbox payload nests the affected id under `target` (add/edit) and/or `previous` (a remove or
/// re-point), so both keys are inspected; unparseable payloads are skipped rather than failing the sync.
async fn pending_contact_ids(ctx: &JobCtx<'_>, character_id: i64) -> Result<HashSet<i64>, Error> {
  let mut ids = HashSet::new();
  for kind in OUTBOX_KINDS_CONTACT {
    let payloads = infra::outbox_pending_payloads(ctx.db, OwnerType::Character, character_id, kind).await?;
    for payload in payloads {
      let Ok(value) = serde_json::from_str::<serde_json::Value>(&payload) else {
        continue;
      };
      for key in ["target", "previous"] {
        if let Some(contact_id) = value
          .get(key)
          .and_then(|entry| entry.get("contact_id"))
          .and_then(serde_json::Value::as_i64)
        {
          ids.insert(contact_id);
        }
      }
    }
  }
  Ok(ids)
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

  async fn mount_contacts(server: &MockServer, character_id: i64) {
    mount_json(
      server,
      &format!("/characters/{character_id}/contacts/"),
      serde_json::json!([
        { "contact_id": 95_001, "contact_type": "character", "is_watched": true, "label_ids": [1], "standing": 7.5 },
        { "contact_id": 98_001, "contact_type": "corporation", "standing": -10.0 },
      ]),
    )
    .await;
  }

  async fn mount_labels(server: &MockServer, character_id: i64) {
    mount_json(
      server,
      &format!("/characters/{character_id}/contacts/labels/"),
      serde_json::json!([
        { "label_id": 1, "label_name": "Friendlies" },
        { "label_id": 2, "label_name": "Watchlist" },
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
      key: JobKey::new(JobKind::CharacterContacts, Subject::Character(character_id)),
      grant: Some(grant),
      sso: None,
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_empty_when_the_character_has_no_contacts() {
      let server = MockServer::start().await;
      mount_json(&server, "/characters/42/contacts/", serde_json::json!([])).await;
      mount_json(&server, "/characters/42/contacts/labels/", serde_json::json!([])).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Empty,
        "a character with no contacts reads as Empty, not green"
      );
    }

    #[tokio::test]
    async fn it_does_not_reinsert_a_contact_with_a_pending_remove() {
      let server = MockServer::start().await;
      mount_contacts(&server, 42).await;
      mount_labels(&server, 42).await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "character", "id": 95_001, "name": "Trusted Pilot" },
          { "category": "corporation", "id": 98_001, "name": "Hostile Corp" },
        ]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      infra::append(
        &db,
        OwnerType::Character,
        42,
        "contact.remove",
        "{\"character_id\":42,\"previous\":{\"contact_id\":95001}}",
        None,
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

      let result = character::contacts(&db, 42).await.unwrap();
      let ids = result.contacts.iter().map(|c| c.contact_id()).collect::<Vec<_>>();
      assert!(
        !ids.contains(&95_001),
        "a contact with a pending remove is not resurrected by the full-replace sync"
      );
      assert!(ids.contains(&98_001), "unprotected server contacts are still synced");
    }

    #[tokio::test]
    async fn it_persists_contacts_and_labels_with_resolved_names() {
      let server = MockServer::start().await;
      mount_contacts(&server, 42).await;
      mount_labels(&server, 42).await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "character", "id": 95_001, "name": "Trusted Pilot" },
          { "category": "corporation", "id": 98_001, "name": "Hostile Corp" },
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

      let outcome = run(&ctx).await.unwrap();
      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 2
        }
      );

      let result = character::contacts(&db, 42).await.unwrap();
      assert_eq!(
        result.contacts.iter().map(|c| c.contact_id()).collect::<Vec<_>>(),
        [95_001, 98_001]
      );
      let trusted = &result.contacts[0];
      assert_eq!(trusted.contact_name(), "Trusted Pilot");
      assert!(trusted.is_watched());
      assert_eq!(trusted.label_ids(), "[1]");
      let hostile = &result.contacts[1];
      assert_eq!(hostile.contact_name(), "Hostile Corp");
      assert!(!hostile.is_watched());
      assert_eq!(hostile.label_ids(), "[]");
      assert_eq!(
        result
          .labels
          .iter()
          .map(|l| l.label_name().as_str())
          .collect::<Vec<_>>(),
        ["Friendlies", "Watchlist"]
      );
    }

    #[tokio::test]
    async fn it_resolves_a_faction_contact_name_from_sde() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/characters/42/contacts/",
        serde_json::json!([{ "contact_id": 500_003, "contact_type": "faction", "standing": 5.0 }]),
      )
      .await;
      mount_json(&server, "/characters/42/contacts/labels/", serde_json::json!([])).await;
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
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      let result = character::contacts(&db, 42).await.unwrap();
      assert_eq!(result.contacts[0].contact_name(), "Amarr Empire");
      assert!(sde::get_faction(&db, 500_003).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_aborts_without_writing_when_the_contacts_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/contacts/"))
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
      let contacts = character::contacts(&db, 42).await.unwrap();
      assert!(contacts.contacts.is_empty());
      assert!(contacts.labels.is_empty());
    }

    #[tokio::test]
    async fn it_aborts_without_writing_when_the_name_resolution_fails() {
      let server = MockServer::start().await;
      mount_contacts(&server, 42).await;
      mount_labels(&server, 42).await;
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
      assert!(character::contacts(&db, 42).await.unwrap().contacts.is_empty());
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/contacts/"))
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

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
      assert!(character::contacts(&db, 42).await.unwrap().contacts.is_empty());
    }
  }
}
