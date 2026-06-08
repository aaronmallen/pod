use serde::Deserialize;

use super::outbox::{HandlerFuture, KindHandler, OutboxKind, Registry};
use crate::{
  clients::{
    self, esi,
    esi::models::character::{MarkReadRequest, SendMailRecipient, SendMailRequest},
    eve_sso::Grant,
  },
  store::{Database, repo::mail},
};

struct SendHandler;

impl SendHandler {
  async fn resolve_recipients(
    esi: &esi::Client,
    recipients: &[SendRecipient],
  ) -> Result<Vec<SendMailRecipient>, clients::Error> {
    let mut resolved: Vec<SendMailRecipient> = Vec::new();
    let mut unresolved_names: Vec<String> = Vec::new();

    for recipient in recipients {
      match (recipient.id, &recipient.recipient_type) {
        (Some(id), Some(kind)) => resolved.push(SendMailRecipient {
          recipient_id: id,
          recipient_type: kind.clone(),
        }),
        (Some(id), None) => resolved.push(SendMailRecipient {
          recipient_id: id,
          recipient_type: "character".to_owned(),
        }),
        (None, _) => unresolved_names.push(recipient.name.clone()),
      }
    }

    if !unresolved_names.is_empty() {
      let ids = esi.universe().ids(&unresolved_names).await?;
      for c in ids.characters {
        resolved.push(SendMailRecipient {
          recipient_id: c.id,
          recipient_type: "character".to_owned(),
        });
      }
      for c in ids.corporations {
        resolved.push(SendMailRecipient {
          recipient_id: c.id,
          recipient_type: "corporation".to_owned(),
        });
      }
      for a in ids.alliances {
        resolved.push(SendMailRecipient {
          recipient_id: a.id,
          recipient_type: "alliance".to_owned(),
        });
      }
    }

    if resolved.is_empty() {
      return Err(clients::Error::Internal(
        "mail.send: no recipients resolved (check the names)".to_owned(),
      ));
    }
    Ok(resolved)
  }
}

impl KindHandler for SendHandler {
  fn kind(&self) -> OutboxKind {
    OutboxKind::MailSend
  }

  fn apply<'a>(&'a self, _db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      SendPayload::parse(payload)?;
      Ok(())
    })
  }

  fn execute<'a>(
    &'a self,
    esi: &'a esi::Client,
    grant: &'a Grant,
    payload: &'a str,
  ) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = SendPayload::parse(payload)?;
      let recipients = Self::resolve_recipients(esi, &p.recipients).await?;
      let request = SendMailRequest {
        approved_cost: None,
        body: p.body,
        recipients,
        subject: p.subject,
      };
      esi.character_authenticated(grant).send_mail(&request).await.map(|_| ())
    })
  }

  fn compensate<'a>(&'a self, _db: &'a Database, _payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move { Ok(()) })
  }
}

#[derive(Debug, Deserialize)]
struct SendPayload {
  body: String,
  from_character_id: i64,
  recipients: Vec<SendRecipient>,
  subject: String,
}

impl SendPayload {
  fn parse(payload: &str) -> Result<Self, clients::Error> {
    Ok(serde_json::from_str(payload)?)
  }
}

#[derive(Debug, Deserialize)]
struct SendRecipient {
  #[serde(default)]
  id: Option<i64>,
  #[allow(dead_code)]
  name: String,
  #[serde(default)]
  recipient_type: Option<String>,
}

struct SetReadHandler;

impl KindHandler for SetReadHandler {
  fn kind(&self) -> OutboxKind {
    OutboxKind::MailSetRead
  }

  fn apply<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = SetReadPayload::parse(payload)?;
      mail::set_read(db, p.character_id, p.mail_id, p.read).await?;
      Ok(())
    })
  }

  fn execute<'a>(
    &'a self,
    esi: &'a esi::Client,
    grant: &'a Grant,
    payload: &'a str,
  ) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = SetReadPayload::parse(payload)?;
      let request = MarkReadRequest {
        labels: None,
        read: Some(p.read),
      };
      esi.character_authenticated(grant).mark_read(p.mail_id, &request).await
    })
  }

  fn compensate<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = SetReadPayload::parse(payload)?;
      mail::set_read(db, p.character_id, p.mail_id, !p.read).await?;
      Ok(())
    })
  }
}

#[derive(Debug, Deserialize)]
struct SetReadPayload {
  character_id: i64,
  mail_id: i64,
  read: bool,
}

impl SetReadPayload {
  fn parse(payload: &str) -> Result<Self, clients::Error> {
    Ok(serde_json::from_str(payload)?)
  }
}

pub(super) fn registry() -> Registry {
  Registry::new()
    .with(Box::new(SetReadHandler))
    .with(Box::new(SendHandler))
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, CharacterMail, CharacterMailBody, Corporation, Gender, Race},
    repo::character,
  };

  async fn seed_character(db: &Database, id: i64) {
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

  async fn store_unread(db: &Database, character_id: i64, mail_id: i64) {
    let header = CharacterMail {
      character_id,
      from_id: 95_000_001,
      from_name: "Sender".to_owned(),
      is_read: false,
      mail_id,
      subject: Some("Subject".to_owned()),
      timestamp: "2026-06-01T10:00:00Z".to_owned(),
      ..Default::default()
    };
    let body = CharacterMailBody {
      body: "<p>hi</p>".to_owned(),
      character_id,
      mail_id,
    };
    mail::upsert_complete(db, &header, &body, &[]).await.unwrap();
  }

  fn payload(character_id: i64, mail_id: i64, read: bool) -> String {
    format!("{{\"character_id\":{character_id},\"mail_id\":{mail_id},\"read\":{read}}}")
  }

  #[test]
  fn it_registers_the_set_read_handler() {
    let registry = registry();

    let handler = registry.handler(OutboxKind::MailSetRead).expect("registered");

    assert_eq!(handler.kind(), OutboxKind::MailSetRead);
  }

  #[test]
  fn it_registers_the_send_handler() {
    let registry = registry();

    let handler = registry.handler(OutboxKind::MailSend).expect("registered");

    assert_eq!(handler.kind(), OutboxKind::MailSend);
  }

  #[tokio::test]
  async fn it_accepts_a_well_formed_send_payload_on_apply() {
    let db = store::open_test().await.unwrap();
    let payload = r#"{"from_character_id":42,"recipients":[{"name":"Vex","id":95000001,"recipient_type":"character"}],"subject":"Hi","body":"There"}"#;

    SendHandler.apply(&db, payload).await.unwrap();
  }

  #[tokio::test]
  async fn it_fails_a_malformed_send_payload() {
    let db = store::open_test().await.unwrap();

    let result = SendHandler.apply(&db, "not json").await;

    assert!(matches!(result, Err(clients::Error::Json(_))));
  }

  #[tokio::test]
  async fn it_optimistically_flips_the_mirror_on_apply() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    store_unread(&db, 42, 7).await;

    SetReadHandler.apply(&db, &payload(42, 7, true)).await.unwrap();

    let headers = mail::headers(&db, 42).await.unwrap();
    assert!(headers.iter().find(|h| h.mail_id() == 7).unwrap().is_read());
  }

  #[tokio::test]
  async fn it_reverts_the_optimistic_flip_on_compensate() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    store_unread(&db, 42, 7).await;
    SetReadHandler.apply(&db, &payload(42, 7, true)).await.unwrap();

    SetReadHandler.compensate(&db, &payload(42, 7, true)).await.unwrap();

    let headers = mail::headers(&db, 42).await.unwrap();
    assert!(!headers.iter().find(|h| h.mail_id() == 7).unwrap().is_read());
  }

  #[tokio::test]
  async fn it_fails_a_malformed_payload() {
    let db = store::open_test().await.unwrap();

    let result = SetReadHandler.apply(&db, "not json").await;

    assert!(matches!(result, Err(clients::Error::Json(_))));
  }

  mod resolve_recipients {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;
    use crate::clients::{esi, http};

    async fn esi_client(server: &MockServer) -> esi::Client {
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db)).build();
      esi::Client::with_base_url(http, server.uri())
    }

    fn recipient(name: &str, id: Option<i64>, kind: Option<&str>) -> SendRecipient {
      SendRecipient {
        name: name.to_owned(),
        id,
        recipient_type: kind.map(ToOwned::to_owned),
      }
    }

    #[tokio::test]
    async fn it_passes_through_already_resolved_recipients_without_fetching() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/universe/ids/"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
      let esi = esi_client(&server).await;
      let recipients = [recipient("Vex", Some(95_000_001), Some("character"))];

      let resolved = SendHandler::resolve_recipients(&esi, &recipients).await.unwrap();

      assert_eq!(resolved.len(), 1);
      assert_eq!(resolved[0].recipient_id, 95_000_001);
      assert_eq!(resolved[0].recipient_type, "character");
    }

    #[tokio::test]
    async fn it_defaults_a_typeless_id_carrying_recipient_to_character() {
      let server = MockServer::start().await;
      let esi = esi_client(&server).await;
      let recipients = [recipient("Vex", Some(95_000_001), None)];

      let resolved = SendHandler::resolve_recipients(&esi, &recipients).await.unwrap();

      assert_eq!(resolved[0].recipient_type, "character");
    }

    #[tokio::test]
    async fn it_resolves_id_less_names_into_their_entity_buckets() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/universe/ids/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "alliances": [{ "id": 99_000_001, "name": "An Alliance" }],
          "characters": [{ "id": 95_000_001, "name": "A Pilot" }],
          "corporations": [{ "id": 98_000_001, "name": "A Corp" }],
        })))
        .mount(&server)
        .await;
      let esi = esi_client(&server).await;
      let recipients = [
        recipient("A Pilot", None, None),
        recipient("A Corp", None, None),
        recipient("An Alliance", None, None),
      ];

      let resolved = SendHandler::resolve_recipients(&esi, &recipients).await.unwrap();

      let mut kinds: Vec<(i64, &str)> = resolved
        .iter()
        .map(|r| (r.recipient_id, r.recipient_type.as_str()))
        .collect();
      kinds.sort_unstable();
      assert_eq!(
        kinds,
        [
          (95_000_001, "character"),
          (98_000_001, "corporation"),
          (99_000_001, "alliance"),
        ]
      );
    }

    #[tokio::test]
    async fn it_errors_when_no_recipient_resolves() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/universe/ids/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;
      let esi = esi_client(&server).await;
      let recipients = [recipient("Unknown Pilot", None, None)];

      let result = SendHandler::resolve_recipients(&esi, &recipients).await;

      assert!(matches!(result, Err(clients::Error::Internal(_))));
    }
  }
}
