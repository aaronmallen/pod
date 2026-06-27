use serde::Deserialize;

use super::outbox::{HandlerFuture, KindHandler, OutboxKind, Registry};
#[cfg(test)]
use crate::store::model::CharacterMailLabel;
use crate::{
  clients::{
    self, esi,
    esi::models::character::{CreateMailLabelRequest, MarkReadRequest, SendMailRecipient, SendMailRequest},
    eve_sso::Grant,
  },
  store::{Database, model::MailSnapshot, repo::mail},
};

struct DeleteHandler;

impl KindHandler for DeleteHandler {
  fn kind(&self) -> OutboxKind {
    OutboxKind::MailDelete
  }

  #[cfg(test)]
  fn apply<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let snapshot = parse_snapshot(payload)?;
      mail::purge_mail(db, snapshot.character_id, snapshot.mail_id).await?;
      Ok(())
    })
  }

  fn execute<'a>(
    &'a self,
    _db: &'a Database,
    esi: &'a esi::Client,
    grant: &'a Grant,
    payload: &'a str,
  ) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let snapshot = parse_snapshot(payload)?;
      esi.character_authenticated(grant).delete_mail(snapshot.mail_id).await
    })
  }

  fn compensate<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let snapshot = parse_snapshot(payload)?;
      mail::restore_mail(db, &snapshot).await?;
      Ok(())
    })
  }
}

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

  #[cfg(test)]
  fn apply<'a>(&'a self, _db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      SendPayload::parse(payload)?;
      Ok(())
    })
  }

  fn execute<'a>(
    &'a self,
    _db: &'a Database,
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

  fn compensate<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = SendPayload::parse(payload)?;
      mail::purge_mail(db, p.from_character_id, p.optimistic_mail_id).await?;
      Ok(())
    })
  }
}

#[derive(Debug, Deserialize)]
struct SendPayload {
  body: String,
  from_character_id: i64,
  #[serde(default)]
  optimistic_mail_id: i64,
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
  name: String,
  #[serde(default)]
  recipient_type: Option<String>,
}

struct SetReadHandler;

impl KindHandler for SetReadHandler {
  fn kind(&self) -> OutboxKind {
    OutboxKind::MailSetRead
  }

  #[cfg(test)]
  fn apply<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = SetReadPayload::parse(payload)?;
      mail::set_read(db, p.character_id, p.mail_id, p.read).await?;
      Ok(())
    })
  }

  fn execute<'a>(
    &'a self,
    _db: &'a Database,
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

struct CreateLabelHandler;

impl KindHandler for CreateLabelHandler {
  fn kind(&self) -> OutboxKind {
    OutboxKind::MailCreateLabel
  }

  #[cfg(test)]
  fn apply<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = CreateLabelPayload::parse(payload)?;
      let label = CharacterMailLabel {
        character_id: p.character_id,
        color: p.color,
        label_id: p.label_id,
        name: p.name,
      };
      mail::insert_label(db, &label).await?;
      Ok(())
    })
  }

  fn execute<'a>(
    &'a self,
    db: &'a Database,
    esi: &'a esi::Client,
    grant: &'a Grant,
    payload: &'a str,
  ) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = CreateLabelPayload::parse(payload)?;
      let request = CreateMailLabelRequest {
        color: p.color,
        name: p.name,
      };
      let server_label_id = esi.character_authenticated(grant).create_mail_label(&request).await?;
      mail::remap_label_id(db, p.character_id, p.label_id, server_label_id).await?;
      Ok(())
    })
  }

  fn compensate<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = CreateLabelPayload::parse(payload)?;
      mail::delete_label(db, p.character_id, p.label_id).await?;
      Ok(())
    })
  }
}

#[derive(Debug, Deserialize)]
struct CreateLabelPayload {
  character_id: i64,
  #[serde(default)]
  color: Option<String>,
  label_id: i64,
  name: String,
}

impl CreateLabelPayload {
  fn parse(payload: &str) -> Result<Self, clients::Error> {
    Ok(serde_json::from_str(payload)?)
  }
}

struct DeleteLabelHandler;

impl KindHandler for DeleteLabelHandler {
  fn kind(&self) -> OutboxKind {
    OutboxKind::MailDeleteLabel
  }

  #[cfg(test)]
  fn apply<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = DeleteLabelPayload::parse(payload)?;
      mail::delete_label(db, p.character_id, p.label_id).await?;
      Ok(())
    })
  }

  fn execute<'a>(
    &'a self,
    _db: &'a Database,
    esi: &'a esi::Client,
    grant: &'a Grant,
    payload: &'a str,
  ) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = DeleteLabelPayload::parse(payload)?;
      esi.character_authenticated(grant).delete_mail_label(p.label_id).await
    })
  }

  fn compensate<'a>(&'a self, _db: &'a Database, _payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move { Ok(()) })
  }
}

#[derive(Debug, Deserialize)]
struct DeleteLabelPayload {
  #[cfg(test)]
  character_id: i64,
  label_id: i64,
}

impl DeleteLabelPayload {
  fn parse(payload: &str) -> Result<Self, clients::Error> {
    Ok(serde_json::from_str(payload)?)
  }
}

struct SetLabelsHandler;

impl SetLabelsHandler {
  async fn write_membership(db: &Database, p: &SetLabelsPayload, labels: &[i64]) -> Result<(), clients::Error> {
    let current = mail::membership(db, p.character_id, p.mail_id).await?;

    for label_id in &current {
      if !labels.contains(label_id) {
        mail::remove_membership(db, p.character_id, p.mail_id, *label_id).await?;
      }
    }

    for label_id in labels {
      if !current.contains(label_id) {
        mail::add_membership(db, p.character_id, p.mail_id, *label_id).await?;
      }
    }

    Ok(())
  }
}

impl KindHandler for SetLabelsHandler {
  fn kind(&self) -> OutboxKind {
    OutboxKind::MailSetLabels
  }

  #[cfg(test)]
  fn apply<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = SetLabelsPayload::parse(payload)?;
      Self::write_membership(db, &p, &p.labels).await
    })
  }

  fn execute<'a>(
    &'a self,
    _db: &'a Database,
    esi: &'a esi::Client,
    grant: &'a Grant,
    payload: &'a str,
  ) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = SetLabelsPayload::parse(payload)?;
      let request = MarkReadRequest {
        labels: Some(p.labels),
        read: None,
      };
      esi.character_authenticated(grant).mark_read(p.mail_id, &request).await
    })
  }

  fn compensate<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = SetLabelsPayload::parse(payload)?;
      Self::write_membership(db, &p, &p.previous).await
    })
  }
}

#[derive(Debug, Deserialize)]
struct SetLabelsPayload {
  character_id: i64,
  labels: Vec<i64>,
  mail_id: i64,
  #[serde(default)]
  previous: Vec<i64>,
}

impl SetLabelsPayload {
  fn parse(payload: &str) -> Result<Self, clients::Error> {
    Ok(serde_json::from_str(payload)?)
  }
}

fn parse_snapshot(payload: &str) -> Result<MailSnapshot, clients::Error> {
  Ok(serde_json::from_str(payload)?)
}

pub(super) fn registry() -> Registry {
  Registry::new()
    .with(Box::new(CreateLabelHandler))
    .with(Box::new(DeleteHandler))
    .with(Box::new(DeleteLabelHandler))
    .with(Box::new(SetLabelsHandler))
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

  mod create_label {
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

    fn payload(character_id: i64, label_id: i64, name: &str, color: &str) -> String {
      format!("{{\"character_id\":{character_id},\"label_id\":{label_id},\"name\":\"{name}\",\"color\":\"{color}\"}}")
    }

    #[tokio::test]
    async fn it_fails_a_malformed_payload() {
      let db = store::open_test().await.unwrap();

      let result = CreateLabelHandler.apply(&db, "not json").await;

      assert!(matches!(result, Err(clients::Error::Json(_))));
    }

    #[tokio::test]
    async fn it_inserts_the_optimistic_label_on_apply() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      CreateLabelHandler
        .apply(&db, &payload(42, -1, "Bills", "#ffffcd"))
        .await
        .unwrap();

      let labels = mail::labels(&db, 42).await.unwrap();
      assert_eq!(labels.len(), 1);
      assert_eq!(labels[0].label_id(), -1);
      assert_eq!(labels[0].name(), "Bills");
      assert_eq!(labels[0].color().as_deref(), Some("#ffffcd"));
    }

    #[test]
    fn it_is_registered() {
      let registry = registry();

      let handler = registry.handler(OutboxKind::MailCreateLabel).expect("registered");

      assert_eq!(handler.kind(), OutboxKind::MailCreateLabel);
    }

    #[tokio::test]
    async fn it_reconciles_the_temp_id_to_the_server_id_on_execute() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      CreateLabelHandler
        .apply(&db, &payload(42, -1, "Bills", "#ffffcd"))
        .await
        .unwrap();
      mail::add_membership(&db, 42, 7, -1).await.unwrap();
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/characters/42/mail/labels/"))
        .respond_with(ResponseTemplate::new(201).set_body_json(17))
        .mount(&server)
        .await;
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      CreateLabelHandler
        .execute(&db, &esi, &grant, &payload(42, -1, "Bills", "#ffffcd"))
        .await
        .unwrap();

      let labels = mail::labels(&db, 42).await.unwrap();
      assert_eq!(labels.iter().map(|l| l.label_id()).collect::<Vec<_>>(), [17]);
      assert_eq!(mail::membership(&db, 42, 7).await.unwrap(), [17]);
    }

    #[tokio::test]
    async fn it_removes_the_optimistic_label_on_compensate() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      CreateLabelHandler
        .apply(&db, &payload(42, -1, "Bills", "#ffffcd"))
        .await
        .unwrap();

      CreateLabelHandler
        .compensate(&db, &payload(42, -1, "Bills", "#ffffcd"))
        .await
        .unwrap();

      assert!(mail::labels(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_surfaces_an_esi_rejection_for_the_drainer_to_compensate() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/characters/42/mail/labels/"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      let result = CreateLabelHandler
        .execute(&db, &esi, &grant, &payload(42, -1, "Bills", "#ffffcd"))
        .await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }
  }

  mod delete_label {
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

    async fn seed_label(db: &Database, character_id: i64, label_id: i64) {
      let label = CharacterMailLabel {
        character_id,
        color: Some("#ffffcd".to_owned()),
        label_id,
        name: "Bills".to_owned(),
      };
      mail::insert_label(db, &label).await.unwrap();
    }

    fn payload(character_id: i64, label_id: i64) -> String {
      format!("{{\"character_id\":{character_id},\"label_id\":{label_id}}}")
    }

    #[tokio::test]
    async fn it_compensates_as_a_no_op() {
      let db = store::open_test().await.unwrap();

      DeleteLabelHandler.compensate(&db, &payload(42, 17)).await.unwrap();
    }

    #[tokio::test]
    async fn it_deletes_at_esi_on_execute() {
      let db = store::open_test().await.unwrap();
      let server = MockServer::start().await;
      Mock::given(method("DELETE"))
        .and(path("/characters/42/mail/labels/17/"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      DeleteLabelHandler
        .execute(&db, &esi, &grant, &payload(42, 17))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_deletes_the_label_and_its_membership_on_apply() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      seed_label(&db, 42, 17).await;
      mail::add_membership(&db, 42, 7, 17).await.unwrap();

      DeleteLabelHandler.apply(&db, &payload(42, 17)).await.unwrap();

      assert!(mail::labels(&db, 42).await.unwrap().is_empty());
      assert!(mail::membership(&db, 42, 7).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_fails_a_malformed_payload() {
      let db = store::open_test().await.unwrap();

      let result = DeleteLabelHandler.apply(&db, "not json").await;

      assert!(matches!(result, Err(clients::Error::Json(_))));
    }

    #[test]
    fn it_is_registered() {
      let registry = registry();

      let handler = registry.handler(OutboxKind::MailDeleteLabel).expect("registered");

      assert_eq!(handler.kind(), OutboxKind::MailDeleteLabel);
    }

    #[tokio::test]
    async fn it_surfaces_an_esi_rejection_for_the_drainer() {
      let db = store::open_test().await.unwrap();
      let server = MockServer::start().await;
      Mock::given(method("DELETE"))
        .and(path("/characters/42/mail/labels/17/"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      let result = DeleteLabelHandler.execute(&db, &esi, &grant, &payload(42, 17)).await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }
  }

  mod delete_mail {
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

    async fn snapshot_payload(db: &Database, character_id: i64, mail_id: i64) -> String {
      let snapshot = mail::snapshot_mail(db, character_id, mail_id)
        .await
        .unwrap()
        .expect("snapshot");
      serde_json::to_string(&snapshot).unwrap()
    }

    #[tokio::test]
    async fn it_deletes_at_esi_on_execute() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      let payload = snapshot_payload(&db, 42, 7).await;
      let server = MockServer::start().await;
      Mock::given(method("DELETE"))
        .and(path("/characters/42/mail/7/"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      DeleteHandler.execute(&db, &esi, &grant, &payload).await.unwrap();
    }

    #[tokio::test]
    async fn it_fails_a_malformed_payload() {
      let db = store::open_test().await.unwrap();

      let result = DeleteHandler.apply(&db, "not json").await;

      assert!(matches!(result, Err(clients::Error::Json(_))));
    }

    #[test]
    fn it_is_registered() {
      let registry = registry();

      let handler = registry.handler(OutboxKind::MailDelete).expect("registered");

      assert_eq!(handler.kind(), OutboxKind::MailDelete);
    }

    #[tokio::test]
    async fn it_purges_the_local_mail_on_apply() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      let payload = snapshot_payload(&db, 42, 7).await;

      DeleteHandler.apply(&db, &payload).await.unwrap();

      assert!(mail::snapshot_mail(&db, 42, 7).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_restores_the_local_mail_on_compensate() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      let payload = snapshot_payload(&db, 42, 7).await;
      DeleteHandler.apply(&db, &payload).await.unwrap();

      DeleteHandler.compensate(&db, &payload).await.unwrap();

      assert!(mail::snapshot_mail(&db, 42, 7).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_surfaces_an_esi_rejection_for_the_drainer_to_compensate() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      let payload = snapshot_payload(&db, 42, 7).await;
      let server = MockServer::start().await;
      Mock::given(method("DELETE"))
        .and(path("/characters/42/mail/7/"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      let result = DeleteHandler.execute(&db, &esi, &grant, &payload).await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }
  }

  #[tokio::test]
  async fn it_accepts_a_well_formed_send_payload_on_apply() {
    let db = store::open_test().await.unwrap();
    let payload = r#"{"from_character_id":42,"recipients":[{"name":"Vex","id":95000001,"recipient_type":"character"}],"subject":"Hi","body":"There"}"#;

    SendHandler.apply(&db, payload).await.unwrap();
  }

  #[tokio::test]
  async fn it_fails_a_malformed_payload() {
    let db = store::open_test().await.unwrap();

    let result = SetReadHandler.apply(&db, "not json").await;

    assert!(matches!(result, Err(clients::Error::Json(_))));
  }

  #[tokio::test]
  async fn it_fails_a_malformed_send_payload() {
    let db = store::open_test().await.unwrap();

    let result = SendHandler.apply(&db, "not json").await;

    assert!(matches!(result, Err(clients::Error::Json(_))));
  }

  #[tokio::test]
  async fn it_purges_the_optimistic_sent_mail_on_compensate() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    let header = CharacterMail {
      character_id: 42,
      from_id: 42,
      from_name: "Pilot".to_owned(),
      is_read: true,
      mail_id: -99,
      subject: Some("Hi".to_owned()),
      timestamp: "2026-06-01T10:00:00Z".to_owned(),
      ..Default::default()
    };
    let body = CharacterMailBody {
      body: "There".to_owned(),
      character_id: 42,
      mail_id: -99,
    };
    mail::upsert_complete(&db, &header, &body, &[]).await.unwrap();
    let payload = r#"{"from_character_id":42,"optimistic_mail_id":-99,"recipients":[],"subject":"Hi","body":"There"}"#;

    SendHandler.compensate(&db, payload).await.unwrap();

    assert!(mail::snapshot_mail(&db, 42, -99).await.unwrap().is_none());
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

  #[test]
  fn it_registers_the_send_handler() {
    let registry = registry();

    let handler = registry.handler(OutboxKind::MailSend).expect("registered");

    assert_eq!(handler.kind(), OutboxKind::MailSend);
  }

  #[test]
  fn it_registers_the_set_read_handler() {
    let registry = registry();

    let handler = registry.handler(OutboxKind::MailSetRead).expect("registered");

    assert_eq!(handler.kind(), OutboxKind::MailSetRead);
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
    async fn it_defaults_a_typeless_id_carrying_recipient_to_character() {
      let server = MockServer::start().await;
      let esi = esi_client(&server).await;
      let recipients = [recipient("Vex", Some(95_000_001), None)];

      let resolved = SendHandler::resolve_recipients(&esi, &recipients).await.unwrap();

      assert_eq!(resolved[0].recipient_type, "character");
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
  }

  mod set_labels {
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

    async fn seed_label(db: &Database, character_id: i64, label_id: i64) {
      let label = CharacterMailLabel {
        character_id,
        color: Some("#ffffcd".to_owned()),
        label_id,
        name: format!("Label {label_id}"),
      };
      mail::insert_label(db, &label).await.unwrap();
    }

    fn payload(character_id: i64, mail_id: i64, labels: &[i64], previous: &[i64]) -> String {
      let render = |ids: &[i64]| ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
      format!(
        "{{\"character_id\":{character_id},\"mail_id\":{mail_id},\"labels\":[{}],\"previous\":[{}]}}",
        render(labels),
        render(previous)
      )
    }

    #[tokio::test]
    async fn it_adds_one_label_while_preserving_the_rest_on_apply() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      seed_label(&db, 42, 10).await;
      seed_label(&db, 42, 20).await;
      mail::add_membership(&db, 42, 7, 10).await.unwrap();

      SetLabelsHandler
        .apply(&db, &payload(42, 7, &[10, 20], &[10]))
        .await
        .unwrap();

      assert_eq!(mail::membership(&db, 42, 7).await.unwrap(), [10, 20]);
    }

    #[tokio::test]
    async fn it_fails_a_malformed_payload() {
      let db = store::open_test().await.unwrap();

      let result = SetLabelsHandler.apply(&db, "not json").await;

      assert!(matches!(result, Err(clients::Error::Json(_))));
    }

    #[test]
    fn it_is_registered() {
      let registry = registry();

      let handler = registry.handler(OutboxKind::MailSetLabels).expect("registered");

      assert_eq!(handler.kind(), OutboxKind::MailSetLabels);
    }

    #[tokio::test]
    async fn it_puts_the_full_set_to_esi_on_execute() {
      let db = store::open_test().await.unwrap();
      let server = MockServer::start().await;
      Mock::given(method("PUT"))
        .and(path("/characters/42/mail/7/"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      SetLabelsHandler
        .execute(&db, &esi, &grant, &payload(42, 7, &[10, 20], &[10]))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_removes_one_label_while_preserving_the_rest_on_apply() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      seed_label(&db, 42, 10).await;
      seed_label(&db, 42, 20).await;
      mail::add_membership(&db, 42, 7, 10).await.unwrap();
      mail::add_membership(&db, 42, 7, 20).await.unwrap();

      SetLabelsHandler
        .apply(&db, &payload(42, 7, &[10], &[10, 20]))
        .await
        .unwrap();

      assert_eq!(mail::membership(&db, 42, 7).await.unwrap(), [10]);
    }

    #[tokio::test]
    async fn it_restores_the_previous_set_on_compensate() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      seed_label(&db, 42, 10).await;
      seed_label(&db, 42, 20).await;
      mail::add_membership(&db, 42, 7, 10).await.unwrap();
      SetLabelsHandler
        .apply(&db, &payload(42, 7, &[10, 20], &[10]))
        .await
        .unwrap();

      SetLabelsHandler
        .compensate(&db, &payload(42, 7, &[10, 20], &[10]))
        .await
        .unwrap();

      assert_eq!(mail::membership(&db, 42, 7).await.unwrap(), [10]);
    }

    #[tokio::test]
    async fn it_surfaces_an_esi_rejection_for_the_drainer_to_compensate() {
      let db = store::open_test().await.unwrap();
      let server = MockServer::start().await;
      Mock::given(method("PUT"))
        .and(path("/characters/42/mail/7/"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      let result = SetLabelsHandler
        .execute(&db, &esi, &grant, &payload(42, 7, &[10, 20], &[10]))
        .await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }
  }
}
