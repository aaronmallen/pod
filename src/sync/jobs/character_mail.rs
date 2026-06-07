use std::collections::{HashMap, HashSet};

use crate::{
  clients::{
    Error,
    esi::models::{character::MailHeader, universe::NameRecord},
  },
  store::{
    images,
    model::{CharacterMail, CharacterMailBody, CharacterMailRecipient},
    repo::{character, mail},
  },
  sync::{job::JobCtx, jobs::names::resolve_names, outcome::Outcome, subject::Subject},
};

const RECIPIENT_TYPE_MAILING_LIST: &str = "mailing_list";
const CATEGORY_CHARACTER: &str = "character";
const CATEGORY_CORPORATION: &str = "corporation";

const SYSTEM_ID_CEILING: i64 = 10_000_000;

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character mail job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Ok(Outcome::NotReady);
  }
  let authenticated = ctx.esi.character_authenticated(grant);

  let headers = authenticated.mail().await?;

  let resolver_ids = collect_resolver_ids(&headers);
  let resolved = resolve_names(ctx, &resolver_ids).await?;

  ensure_sender_portraits(ctx, &headers, &resolved).await;

  let mut synced = 0usize;
  for header in &headers {
    let Some(timestamp) = header.timestamp.clone() else {
      continue;
    };

    let Some(recipients) = build_recipients(character_id, header, &resolved) else {
      continue;
    };
    let Some(from_name) = sender_name(header, &resolved) else {
      continue;
    };

    let body = if let Some(existing) = mail::body(ctx.db, character_id, header.mail_id).await? {
      existing
    } else {
      let fetched = authenticated.mail_body(header.mail_id).await?;
      CharacterMailBody {
        body: fetched.body,
        character_id,
        mail_id: header.mail_id,
      }
    };

    let from_id = header.from.unwrap_or(0);
    let (from_corp, from_system) = classify_sender(from_id, &resolved);

    let mail = CharacterMail {
      character_id,
      from_id,
      from_name,
      has_attachment: false,
      important: false,
      from_corp,
      from_system,
      is_read: header.is_read.unwrap_or(false),
      mail_id: header.mail_id,
      subject: header.subject.clone(),
      timestamp,
    };

    mail::upsert_complete(ctx.db, &mail, &body, &recipients).await?;
    synced += 1;
  }

  Ok(Outcome::from_rows(synced))
}

async fn ensure_sender_portraits(ctx: &JobCtx<'_>, headers: &[MailHeader], resolved: &HashMap<i64, NameRecord>) {
  let mut seen = HashSet::new();
  for header in headers {
    let Some(from_id) = header.from else {
      continue;
    };
    if from_id <= 0 || !seen.insert(from_id) {
      continue;
    }
    let Some(record) = resolved.get(&from_id) else {
      continue;
    };
    if record.category != CATEGORY_CHARACTER {
      continue;
    }
    let path = ctx.image_store.character_portrait_path(from_id);
    if path.exists() {
      continue;
    }
    let url = ctx.image.character_portrait_url(from_id, images::PORTRAIT_SIZE);
    if let Ok(bytes) = ctx.image.fetch(&url).await {
      let _ = ctx.image_store.write(&path, &bytes);
    }
  }
}

fn build_recipients(
  character_id: i64,
  header: &MailHeader,
  resolved: &HashMap<i64, NameRecord>,
) -> Option<Vec<CharacterMailRecipient>> {
  let mut rows = Vec::with_capacity(header.recipients.len());
  for recipient in &header.recipients {
    let recipient_name = if recipient.recipient_type == RECIPIENT_TYPE_MAILING_LIST {
      format!("Mailing List ({})", recipient.recipient_id)
    } else {
      resolved
        .get(&recipient.recipient_id)
        .map(|record| record.name.clone())?
    };
    rows.push(CharacterMailRecipient {
      character_id,
      mail_id: header.mail_id,
      recipient_id: recipient.recipient_id,
      recipient_name,
      recipient_type: recipient.recipient_type.clone(),
    });
  }
  Some(rows)
}

fn collect_resolver_ids(headers: &[MailHeader]) -> Vec<i64> {
  let mut ids = Vec::new();
  for header in headers {
    if let Some(from) = header.from {
      ids.push(from);
    }
    for recipient in &header.recipients {
      if recipient.recipient_type != RECIPIENT_TYPE_MAILING_LIST {
        ids.push(recipient.recipient_id);
      }
    }
  }
  ids
}

fn classify_sender(from_id: i64, resolved: &HashMap<i64, NameRecord>) -> (bool, bool) {
  match resolved.get(&from_id) {
    Some(record) => (record.category == CATEGORY_CORPORATION, false),
    None => (false, from_id < SYSTEM_ID_CEILING),
  }
}

fn sender_name(header: &MailHeader, resolved: &HashMap<i64, NameRecord>) -> Option<String> {
  match header.from {
    None => Some(String::new()),
    Some(from_id) => match resolved.get(&from_id) {
      Some(record) => Some(record.name.clone()),
      None if from_id < SYSTEM_ID_CEILING => Some("EVE System".to_owned()),
      None => None,
    },
  }
}

#[cfg(test)]
mod tests {
  use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  };

  use wiremock::{
    Mock, MockServer, Request, Respond, ResponseTemplate,
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
      .and(path("/v3/universe/names/"))
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
      key: JobKey::new(JobKind::CharacterMail, Subject::Character(character_id)),
      grant: Some(grant),
    }
  }

  struct Fixture {
    _server: MockServer,
    db: store::Database,
    esi: esi::Client,
    image: eve_image::Client,
    image_store: images::Store,
    _images_dir: tempfile::TempDir,
    grant: Grant,
  }

  async fn fixture(server: MockServer, character_id: i64) -> Fixture {
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = esi::Client::with_base_url(http.clone(), server.uri());
    let image = eve_image::Client::with_base_url(http, server.uri());
    let images_dir = tempfile::tempdir().unwrap();
    let image_store = images::Store::new(images_dir.path().to_path_buf());
    let grant = Grant::new_test("token", character_id);
    Fixture {
      _server: server,
      db,
      esi,
      image,
      image_store,
      _images_dir: images_dir,
      grant,
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_commits_header_body_and_resolved_recipients_together() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/v1/characters/42/mail/",
        serde_json::json!([
          { "mail_id": 7, "from": 1001, "is_read": false, "timestamp": "2026-06-01T10:00:00Z", "subject": "Hi",
            "labels": [1],
            "recipients": [
              { "recipient_id": 2002, "recipient_type": "character" },
              { "recipient_id": 9009, "recipient_type": "mailing_list" }
            ] },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/v1/characters/42/mail/7/",
        serde_json::json!({ "body": "<p>Hello</p>", "from": 1001, "labels": [1], "read": false, "subject": "Hi",
          "timestamp": "2026-06-01T10:00:00Z" }),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "character", "id": 1001, "name": "Sender" },
          { "category": "character", "id": 2002, "name": "Recipient" },
        ]),
      )
      .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      run(&ctx).await.unwrap();

      let render = mail::mail(&fx.db, 42, 7).await.unwrap().unwrap();
      assert_eq!(render.header.from_name(), "Sender");
      assert_eq!(render.body.body(), "<p>Hello</p>");
      assert!(!render.header.from_corp());
      assert!(!render.header.from_system());
      assert!(!render.header.has_attachment());
      assert!(!render.header.important());
      assert_eq!(
        render
          .recipients
          .iter()
          .map(|r| r.recipient_name().clone())
          .collect::<Vec<_>>(),
        ["Recipient", "Mailing List (9009)"]
      );
    }

    #[tokio::test]
    async fn it_flags_a_corporation_sender_and_flows_it_to_the_unified_model() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/v1/characters/42/mail/",
        serde_json::json!([
          { "mail_id": 7, "from": 98_000_001_i64, "is_read": false, "timestamp": "2026-06-01T10:00:00Z",
            "subject": "Corp", "recipients": [{ "recipient_id": 42, "recipient_type": "character" }] },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/v1/characters/42/mail/7/",
        serde_json::json!({ "body": "<p>Corp mail</p>" }),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "corporation", "id": 98_000_001_i64, "name": "Test Corp" },
          { "category": "character", "id": 42, "name": "Pilot" },
        ]),
      )
      .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      run(&ctx).await.unwrap();

      let render = mail::mail(&fx.db, 42, 7).await.unwrap().unwrap();
      assert!(render.header.from_corp());
      assert!(!render.header.from_system());

      let unified = mail::unified(&fx.db).await.unwrap();
      let row = unified.iter().find(|m| m.mail_id == 7).unwrap();
      assert!(row.from_corp);
      assert!(!row.from_system);
      assert!(!row.has_attachment);
      assert!(!row.important);
    }

    #[tokio::test]
    async fn it_flags_a_system_sender_even_when_the_name_is_unresolvable() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/v1/characters/42/mail/",
        serde_json::json!([
          { "mail_id": 7, "from": 1, "is_read": false, "timestamp": "2026-06-01T10:00:00Z",
            "subject": "System notice", "recipients": [{ "recipient_id": 42, "recipient_type": "character" }] },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/v1/characters/42/mail/7/",
        serde_json::json!({ "body": "<p>System mail</p>" }),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 42, "name": "Pilot" }]),
      )
      .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      run(&ctx).await.unwrap();

      let render = mail::mail(&fx.db, 42, 7).await.unwrap().unwrap();
      assert_eq!(render.header.from_name(), "EVE System");
      assert!(render.header.from_system());
      assert!(!render.header.from_corp());

      let unified = mail::unified(&fx.db).await.unwrap();
      let row = unified.iter().find(|m| m.mail_id == 7).unwrap();
      assert!(row.from_system);
      assert!(!row.from_corp);
    }

    #[tokio::test]
    async fn it_withholds_a_mail_with_an_unresolved_participant() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/v1/characters/42/mail/",
        serde_json::json!([
          { "mail_id": 7, "from": 1001, "is_read": false, "timestamp": "2026-06-01T10:00:00Z", "subject": "Hi",
            "recipients": [{ "recipient_id": 2002, "recipient_type": "character" }] },
        ]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path("/v1/characters/42/mail/7/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "body": "<p>x</p>" })))
        .expect(0)
        .mount(&server)
        .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 1001, "name": "Sender" }]),
      )
      .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      run(&ctx).await.unwrap();

      assert!(mail::headers(&fx.db, 42).await.unwrap().is_empty());
      assert!(!mail::has_body(&fx.db, 42, 7).await.unwrap());
    }

    #[tokio::test]
    async fn it_does_not_refetch_the_body_and_reconciles_read_state_on_re_run() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/v1/characters/42/mail/",
        serde_json::json!([
          { "mail_id": 7, "from": 1001, "is_read": true, "timestamp": "2026-06-01T10:00:00Z", "subject": "Hi",
            "recipients": [{ "recipient_id": 1001, "recipient_type": "character" }] },
        ]),
      )
      .await;
      let body_hits = Arc::new(AtomicUsize::new(0));
      struct CountingBody(Arc<AtomicUsize>);
      impl Respond for CountingBody {
        fn respond(&self, _: &Request) -> ResponseTemplate {
          self.0.fetch_add(1, Ordering::SeqCst);
          ResponseTemplate::new(200).set_body_json(serde_json::json!({ "body": "<p>SHOULD NOT FETCH</p>" }))
        }
      }
      Mock::given(method("GET"))
        .and(path("/v1/characters/42/mail/7/"))
        .respond_with(CountingBody(body_hits.clone()))
        .mount(&server)
        .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 1001, "name": "Sender" }]),
      )
      .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      mail::upsert_complete(
        &fx.db,
        &CharacterMail {
          character_id: 42,
          from_id: 1001,
          from_name: "Sender".to_owned(),
          is_read: false,
          mail_id: 7,
          subject: Some("Hi".to_owned()),
          timestamp: "2026-06-01T10:00:00Z".to_owned(),
          ..Default::default()
        },
        &CharacterMailBody {
          body: "<p>original</p>".to_owned(),
          character_id: 42,
          mail_id: 7,
        },
        &[],
      )
      .await
      .unwrap();
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      run(&ctx).await.unwrap();

      assert_eq!(body_hits.load(Ordering::SeqCst), 0);
      assert_eq!(
        mail::body(&fx.db, 42, 7).await.unwrap().unwrap().body(),
        "<p>original</p>"
      );
      assert!(mail::headers(&fx.db, 42).await.unwrap()[0].is_read());
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/characters/42/mail/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let fx = fixture(server, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      run(&ctx).await.unwrap();

      assert!(mail::headers(&fx.db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_aborts_without_writing_when_the_mail_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/characters/42/mail/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(mail::headers(&fx.db, 42).await.unwrap().is_empty());
    }
  }
}
