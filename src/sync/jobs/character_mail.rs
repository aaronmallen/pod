use std::collections::{BTreeMap, HashMap, HashSet};

use crate::{
  clients::{
    Error,
    esi::models::{
      character::{MailHeader, MailLabels},
      universe::NameRecord,
    },
  },
  store::{
    images,
    model::{
      CharacterMail, CharacterMailBody, CharacterMailLabel, CharacterMailLabelMembership, CharacterMailRecipient,
      OwnerType,
    },
    repo::{character, infra, mail},
  },
  sync::{job::JobCtx, jobs::names::resolve_names, outcome::Outcome, subject::Subject},
};

const RECIPIENT_TYPE_MAILING_LIST: &str = "mailing_list";
const CATEGORY_CHARACTER: &str = "character";
const CATEGORY_CORPORATION: &str = "corporation";

const OUTBOX_KIND_SET_READ: &str = "mail.set_read";

const SYSTEM_ID_CEILING: i64 = 10_000_000;

const SYSTEM_LABELS: [(i64, &str); 4] = [(1, "Inbox"), (2, "Sent"), (4, "Corp"), (8, "Alliance")];

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
    return Err(Error::NotReady);
  }
  let authenticated = ctx.esi.character_authenticated(grant);

  let headers = authenticated.mail().await?;
  let label_definitions = authenticated.mail_labels().await?;

  let resolver_ids = collect_resolver_ids(&headers);
  let resolved = resolve_names(ctx, &resolver_ids).await?;

  ensure_sender_portraits(ctx, &headers, &resolved).await;

  let pending_read = pending_read_mail_ids(ctx, character_id).await?;

  let mut synced = 0usize;
  let mut persisted = HashSet::new();
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
      is_read: header.is_read.unwrap_or(false) || pending_read.contains(&header.mail_id),
      mail_id: header.mail_id,
      subject: header.subject.clone(),
      timestamp,
    };

    mail::upsert_complete(ctx.db, &mail, &body, &recipients).await?;
    persisted.insert(header.mail_id);
    synced += 1;
  }

  let labels = build_labels(character_id, &label_definitions, &headers, &persisted);
  mail::replace_labels_for_character(ctx.db, character_id, &labels).await?;

  let memberships = build_memberships(character_id, &headers, &persisted);
  mail::replace_membership_for_character(ctx.db, character_id, &memberships).await?;

  Ok(Outcome::from_rows(synced))
}

/// Mail ids with an unflushed mark-read outbox write, used to defend the optimistic read flag.
///
/// ESI still reports these as unread until the outbox write reaches the server, so a sync that
/// landed first would otherwise clobber the local read state back to unread.
async fn pending_read_mail_ids(ctx: &JobCtx<'_>, character_id: i64) -> Result<HashSet<i64>, Error> {
  let payloads =
    infra::outbox_pending_payloads(ctx.db, OwnerType::Character, character_id, OUTBOX_KIND_SET_READ).await?;
  let mut ids = HashSet::new();
  for payload in payloads {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&payload)
      && let Some(mail_id) = value.get("mail_id").and_then(serde_json::Value::as_i64)
    {
      ids.insert(mail_id);
    }
  }
  Ok(ids)
}

fn build_labels(
  character_id: i64,
  definitions: &MailLabels,
  headers: &[MailHeader],
  persisted: &HashSet<i64>,
) -> Vec<CharacterMailLabel> {
  let mut by_id = BTreeMap::new();
  for label in &definitions.labels {
    by_id.insert(
      label.label_id,
      CharacterMailLabel {
        character_id,
        color: label.color.clone(),
        label_id: label.label_id,
        name: label.name.clone().unwrap_or_else(|| label_name(label.label_id)),
      },
    );
  }

  // ESI's mail/labels endpoint omits system labels (ids 1/2/4/8), but message headers still reference
  // them and the membership table has a FK to the labels table, so any header-only id must be synthesized.
  for header in headers {
    if !persisted.contains(&header.mail_id) {
      continue;
    }
    for &label_id in &header.labels {
      by_id.entry(label_id).or_insert_with(|| CharacterMailLabel {
        character_id,
        color: None,
        label_id,
        name: label_name(label_id),
      });
    }
  }

  by_id.into_values().collect()
}

fn build_memberships(
  character_id: i64,
  headers: &[MailHeader],
  persisted: &HashSet<i64>,
) -> Vec<CharacterMailLabelMembership> {
  let mut rows = Vec::new();
  for header in headers {
    if !persisted.contains(&header.mail_id) {
      continue;
    }
    for &label_id in &header.labels {
      rows.push(CharacterMailLabelMembership {
        character_id,
        label_id,
        mail_id: header.mail_id,
      });
    }
  }
  rows
}

fn label_name(label_id: i64) -> String {
  SYSTEM_LABELS
    .iter()
    .find(|(id, _)| *id == label_id)
    .map_or_else(|| format!("Label {label_id}"), |(_, name)| (*name).to_owned())
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

  async fn mount_labels(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path("/characters/42/mail/labels/"))
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
      sso: None,
    }
  }

  struct Fixture {
    db: store::Database,
    esi: esi::Client,
    grant: Grant,
    image: eve_image::Client,
    image_store: images::Store,
    _images_dir: tempfile::TempDir,
    _server: MockServer,
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
        "/characters/42/mail/",
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
        "/characters/42/mail/7/",
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
      mount_labels(
        &server,
        serde_json::json!({ "labels": [{ "label_id": 1, "name": "Inbox", "color": "#ffffff" }] }),
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

      let labels = mail::labels(&fx.db, 42).await.unwrap();
      assert_eq!(labels.len(), 1);
      assert_eq!(labels[0].label_id(), 1);
      assert_eq!(labels[0].name(), "Inbox");
      assert_eq!(labels[0].color().as_deref(), Some("#ffffff"));
      assert_eq!(mail::membership(&fx.db, 42, 7).await.unwrap(), [1]);
    }

    #[tokio::test]
    async fn it_flags_a_corporation_sender_and_flows_it_to_the_unified_model() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/characters/42/mail/",
        serde_json::json!([
          { "mail_id": 7, "from": 98_000_001_i64, "is_read": false, "timestamp": "2026-06-01T10:00:00Z",
            "subject": "Corp", "recipients": [{ "recipient_id": 42, "recipient_type": "character" }] },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/characters/42/mail/7/",
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
      mount_labels(&server, serde_json::json!({ "labels": [] })).await;
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
        "/characters/42/mail/",
        serde_json::json!([
          { "mail_id": 7, "from": 1, "is_read": false, "timestamp": "2026-06-01T10:00:00Z",
            "subject": "System notice", "recipients": [{ "recipient_id": 42, "recipient_type": "character" }] },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/characters/42/mail/7/",
        serde_json::json!({ "body": "<p>System mail</p>" }),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 42, "name": "Pilot" }]),
      )
      .await;
      mount_labels(&server, serde_json::json!({ "labels": [] })).await;
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
        "/characters/42/mail/",
        serde_json::json!([
          { "mail_id": 7, "from": 1001, "is_read": false, "timestamp": "2026-06-01T10:00:00Z", "subject": "Hi",
            "recipients": [{ "recipient_id": 2002, "recipient_type": "character" }] },
        ]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path("/characters/42/mail/7/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "body": "<p>x</p>" })))
        .expect(0)
        .mount(&server)
        .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 1001, "name": "Sender" }]),
      )
      .await;
      mount_labels(&server, serde_json::json!({ "labels": [] })).await;
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
        "/characters/42/mail/",
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
        .and(path("/characters/42/mail/7/"))
        .respond_with(CountingBody(body_hits.clone()))
        .mount(&server)
        .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 1001, "name": "Sender" }]),
      )
      .await;
      mount_labels(&server, serde_json::json!({ "labels": [] })).await;
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
    async fn it_keeps_a_just_read_mail_read_when_the_outbox_write_is_still_pending() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/characters/42/mail/",
        serde_json::json!([
          { "mail_id": 7, "from": 1001, "is_read": false, "timestamp": "2026-06-01T10:00:00Z", "subject": "Hi",
            "recipients": [{ "recipient_id": 1001, "recipient_type": "character" }] },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/characters/42/mail/7/",
        serde_json::json!({ "body": "<p>hi</p>" }),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 1001, "name": "Sender" }]),
      )
      .await;
      mount_labels(&server, serde_json::json!({ "labels": [] })).await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      infra::append(
        &fx.db,
        OwnerType::Character,
        42,
        OUTBOX_KIND_SET_READ,
        "{\"character_id\":42,\"mail_id\":7,\"read\":true}",
        Some("set_read:7"),
      )
      .await
      .unwrap();
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      run(&ctx).await.unwrap();

      assert!(
        mail::headers(&fx.db, 42).await.unwrap()[0].is_read(),
        "a pending mark-read outbox row protects the optimistic read flag against an is_read:false sync"
      );
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/mail/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let fx = fixture(server, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
      assert!(mail::headers(&fx.db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_aborts_without_writing_when_the_mail_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/mail/"))
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

    #[tokio::test]
    async fn it_persists_label_definitions_with_color_and_synthesizes_system_labels_from_headers() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/characters/42/mail/",
        serde_json::json!([
          { "mail_id": 7, "from": 1001, "is_read": false, "timestamp": "2026-06-01T10:00:00Z", "subject": "Hi",
            "labels": [1, 16],
            "recipients": [{ "recipient_id": 42, "recipient_type": "character" }] },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/characters/42/mail/7/",
        serde_json::json!({ "body": "<p>Hello</p>" }),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "character", "id": 1001, "name": "Sender" },
          { "category": "character", "id": 42, "name": "Pilot" },
        ]),
      )
      .await;
      mount_labels(
        &server,
        serde_json::json!({ "labels": [{ "label_id": 16, "name": "Custom", "color": "#660066" }] }),
      )
      .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      run(&ctx).await.unwrap();

      let labels = mail::labels(&fx.db, 42).await.unwrap();
      let custom = labels.iter().find(|l| l.label_id() == 16).unwrap();
      assert_eq!(custom.name(), "Custom");
      assert_eq!(custom.color().as_deref(), Some("#660066"));
      let inbox = labels.iter().find(|l| l.label_id() == 1).unwrap();
      assert_eq!(inbox.name(), "Inbox");
      assert!(inbox.color().is_none());

      assert_eq!(mail::membership(&fx.db, 42, 7).await.unwrap(), [1, 16]);
    }

    #[tokio::test]
    async fn it_reconciles_a_removed_label_and_membership_on_resync() {
      struct Sequenced {
        calls: Arc<AtomicUsize>,
        first: serde_json::Value,
        rest: serde_json::Value,
      }
      impl Respond for Sequenced {
        fn respond(&self, _: &Request) -> ResponseTemplate {
          let body = if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            &self.first
          } else {
            &self.rest
          };
          ResponseTemplate::new(200).set_body_json(body)
        }
      }

      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/mail/"))
        .respond_with(Sequenced {
          first: serde_json::json!([
            { "mail_id": 7, "from": 1001, "is_read": false, "timestamp": "2026-06-01T10:00:00Z", "subject": "Hi",
              "labels": [16, 17],
              "recipients": [{ "recipient_id": 42, "recipient_type": "character" }] },
          ]),
          rest: serde_json::json!([
            { "mail_id": 7, "from": 1001, "is_read": false, "timestamp": "2026-06-01T10:00:00Z", "subject": "Hi",
              "labels": [16],
              "recipients": [{ "recipient_id": 42, "recipient_type": "character" }] },
          ]),
          calls: Arc::new(AtomicUsize::new(0)),
        })
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/characters/42/mail/labels/"))
        .respond_with(Sequenced {
          first: serde_json::json!({ "labels": [
            { "label_id": 16, "name": "Keep", "color": "#660066" },
            { "label_id": 17, "name": "Drop", "color": "#ffffff" },
          ] }),
          rest: serde_json::json!({ "labels": [{ "label_id": 16, "name": "Keep", "color": "#660066" }] }),
          calls: Arc::new(AtomicUsize::new(0)),
        })
        .mount(&server)
        .await;
      mount_json(
        &server,
        "/characters/42/mail/7/",
        serde_json::json!({ "body": "<p>Hello</p>" }),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "character", "id": 1001, "name": "Sender" },
          { "category": "character", "id": 42, "name": "Pilot" },
        ]),
      )
      .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      run(&ctx).await.unwrap();

      assert_eq!(
        mail::labels(&fx.db, 42)
          .await
          .unwrap()
          .iter()
          .map(|l| l.label_id())
          .collect::<Vec<_>>(),
        [16, 17]
      );
      assert_eq!(mail::membership(&fx.db, 42, 7).await.unwrap(), [16, 17]);

      run(&ctx).await.unwrap();

      assert_eq!(
        mail::labels(&fx.db, 42)
          .await
          .unwrap()
          .iter()
          .map(|l| l.label_id())
          .collect::<Vec<_>>(),
        [16]
      );
      assert_eq!(mail::membership(&fx.db, 42, 7).await.unwrap(), [16]);
    }

    #[tokio::test]
    async fn it_preserves_an_optimistic_negative_id_label_through_a_sync() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/characters/42/mail/",
        serde_json::json!([
          { "mail_id": 7, "from": 1001, "is_read": false, "timestamp": "2026-06-01T10:00:00Z", "subject": "Hi",
            "labels": [16],
            "recipients": [{ "recipient_id": 42, "recipient_type": "character" }] },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/characters/42/mail/7/",
        serde_json::json!({ "body": "<p>Hello</p>" }),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "character", "id": 1001, "name": "Sender" },
          { "category": "character", "id": 42, "name": "Pilot" },
        ]),
      )
      .await;
      mount_labels(
        &server,
        serde_json::json!({ "labels": [{ "label_id": 16, "name": "Keep", "color": "#660066" }] }),
      )
      .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      mail::insert_label(
        &fx.db,
        &CharacterMailLabel {
          character_id: 42,
          color: Some("#ff0000".to_owned()),
          label_id: -1,
          name: "Pending".to_owned(),
        },
      )
      .await
      .unwrap();
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      run(&ctx).await.unwrap();

      assert_eq!(
        mail::labels(&fx.db, 42)
          .await
          .unwrap()
          .iter()
          .map(|l| l.label_id())
          .collect::<Vec<_>>(),
        [-1, 16]
      );
    }
  }
}
