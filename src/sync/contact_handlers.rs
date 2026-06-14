use serde::Deserialize;

use super::outbox::{HandlerFuture, KindHandler, OutboxKind, Registry};
use crate::{
  clients::{self, esi, eve_sso::Grant},
  store::{Database, model::CharacterContact, repo::character},
};

struct AddHandler;

impl KindHandler for AddHandler {
  fn kind(&self) -> OutboxKind {
    OutboxKind::ContactAdd
  }

  fn apply<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = AddPayload::parse(payload)?;
      character::upsert_contact(db, &p.target.contact(p.character_id)?).await?;
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
      let p = AddPayload::parse(payload)?;
      esi
        .character_authenticated(grant)
        .add_contacts(
          &[p.target.contact_id],
          p.target.standing,
          &p.target.label_ids,
          p.target.watched,
        )
        .await
        .map(|_| ())
    })
  }

  fn compensate<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = AddPayload::parse(payload)?;
      character::delete_contact(db, p.character_id, p.target.contact_id).await?;
      Ok(())
    })
  }
}

#[derive(Debug, Deserialize)]
struct AddPayload {
  character_id: i64,
  target: ContactState,
}

impl AddPayload {
  fn parse(payload: &str) -> Result<Self, clients::Error> {
    Ok(serde_json::from_str(payload)?)
  }
}

struct EditHandler;

impl KindHandler for EditHandler {
  fn kind(&self) -> OutboxKind {
    OutboxKind::ContactEdit
  }

  fn apply<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = EditPayload::parse(payload)?;
      character::upsert_contact(db, &p.target.contact(p.character_id)?).await?;
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
      let p = EditPayload::parse(payload)?;
      esi
        .character_authenticated(grant)
        .edit_contacts(
          &[p.target.contact_id],
          p.target.standing,
          &p.target.label_ids,
          p.target.watched,
        )
        .await
    })
  }

  fn compensate<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = EditPayload::parse(payload)?;
      character::upsert_contact(db, &p.previous.contact(p.character_id)?).await?;
      Ok(())
    })
  }
}

#[derive(Debug, Deserialize)]
struct EditPayload {
  character_id: i64,
  previous: ContactState,
  target: ContactState,
}

impl EditPayload {
  fn parse(payload: &str) -> Result<Self, clients::Error> {
    Ok(serde_json::from_str(payload)?)
  }
}

struct RemoveHandler;

impl KindHandler for RemoveHandler {
  fn kind(&self) -> OutboxKind {
    OutboxKind::ContactRemove
  }

  fn apply<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = RemovePayload::parse(payload)?;
      character::delete_contact(db, p.character_id, p.previous.contact_id).await?;
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
      let p = RemovePayload::parse(payload)?;
      esi
        .character_authenticated(grant)
        .remove_contacts(&[p.previous.contact_id])
        .await
    })
  }

  fn compensate<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = RemovePayload::parse(payload)?;
      character::upsert_contact(db, &p.previous.contact(p.character_id)?).await?;
      Ok(())
    })
  }
}

#[derive(Debug, Deserialize)]
struct RemovePayload {
  character_id: i64,
  previous: ContactState,
}

impl RemovePayload {
  fn parse(payload: &str) -> Result<Self, clients::Error> {
    Ok(serde_json::from_str(payload)?)
  }
}

#[derive(Debug, Deserialize)]
struct ContactState {
  contact_id: i64,
  contact_name: String,
  contact_type: String,
  #[serde(default)]
  is_blocked: bool,
  #[serde(default)]
  label_ids: Vec<i64>,
  standing: f64,
  #[serde(default)]
  watched: bool,
}

impl ContactState {
  fn contact(&self, character_id: i64) -> Result<CharacterContact, clients::Error> {
    Ok(CharacterContact {
      character_id,
      contact_id: self.contact_id,
      contact_name: self.contact_name.clone(),
      contact_type: self.contact_type.clone(),
      is_blocked: self.is_blocked,
      is_watched: self.watched,
      label_ids: serde_json::to_string(&self.label_ids)?,
      standing: self.standing,
    })
  }
}

pub(super) fn registry() -> Registry {
  Registry::new()
    .with(Box::new(AddHandler))
    .with(Box::new(EditHandler))
    .with(Box::new(RemoveHandler))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
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

  async fn contact_row(db: &Database, character_id: i64, contact_id: i64) -> Option<CharacterContact> {
    character::contacts(db, character_id)
      .await
      .unwrap()
      .contacts
      .into_iter()
      .find(|c| c.contact_id() == contact_id)
  }

  fn previous_state() -> serde_json::Value {
    serde_json::json!({
      "contact_id": 95_001,
      "contact_name": "Trusted Pilot",
      "contact_type": "character",
      "is_blocked": false,
      "label_ids": [1],
      "standing": 5.0,
      "watched": false,
    })
  }

  fn add_payload() -> String {
    serde_json::json!({
      "character_id": 42,
      "target": {
        "contact_id": 95_001,
        "contact_name": "Trusted Pilot",
        "contact_type": "character",
        "label_ids": [1, 2],
        "standing": 5.0,
        "watched": true,
      },
    })
    .to_string()
  }

  fn edit_payload() -> String {
    serde_json::json!({
      "character_id": 42,
      "previous": previous_state(),
      "target": {
        "contact_id": 95_001,
        "contact_name": "Trusted Pilot",
        "contact_type": "character",
        "label_ids": [2],
        "standing": -10.0,
        "watched": true,
      },
    })
    .to_string()
  }

  fn remove_payload() -> String {
    serde_json::json!({
      "character_id": 42,
      "previous": previous_state(),
    })
    .to_string()
  }

  #[test]
  fn it_registers_the_three_contact_handlers() {
    use pretty_assertions::assert_eq;

    let registry = registry();

    assert_eq!(
      registry.handler(OutboxKind::ContactAdd).expect("add").kind(),
      OutboxKind::ContactAdd
    );
    assert_eq!(
      registry.handler(OutboxKind::ContactEdit).expect("edit").kind(),
      OutboxKind::ContactEdit
    );
    assert_eq!(
      registry.handler(OutboxKind::ContactRemove).expect("remove").kind(),
      OutboxKind::ContactRemove
    );
  }

  mod add_handler {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_mirrors_the_new_contact_on_apply() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      AddHandler.apply(&db, &add_payload()).await.unwrap();

      let row = contact_row(&db, 42, 95_001).await.expect("contact mirrored");
      assert_eq!(row.contact_name(), "Trusted Pilot");
      assert_eq!(row.standing(), 5.0);
      assert!(row.is_watched());
      assert_eq!(row.label_ids(), "[1,2]");
    }

    #[tokio::test]
    async fn it_is_idempotent_against_a_full_replace_sync_on_apply() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      AddHandler.apply(&db, &add_payload()).await.unwrap();

      AddHandler.apply(&db, &add_payload()).await.unwrap();

      let rows = character::contacts(&db, 42).await.unwrap().contacts;
      assert_eq!(rows.iter().filter(|c| c.contact_id() == 95_001).count(), 1);
    }

    #[tokio::test]
    async fn it_drops_the_optimistic_row_on_compensate() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      AddHandler.apply(&db, &add_payload()).await.unwrap();

      AddHandler.compensate(&db, &add_payload()).await.unwrap();

      assert!(contact_row(&db, 42, 95_001).await.is_none());
    }

    #[tokio::test]
    async fn it_fails_a_malformed_payload() {
      let db = store::open_test().await.unwrap();

      let result = AddHandler.apply(&db, "not json").await;

      assert!(matches!(result, Err(clients::Error::Json(_))));
    }
  }

  mod edit_handler {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_overwrites_the_contact_on_apply() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      AddHandler.apply(&db, &add_payload()).await.unwrap();

      EditHandler.apply(&db, &edit_payload()).await.unwrap();

      let row = contact_row(&db, 42, 95_001).await.expect("contact present");
      assert_eq!(row.standing(), -10.0);
      assert_eq!(row.label_ids(), "[2]");
    }

    #[tokio::test]
    async fn it_restores_the_previous_state_on_compensate() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      AddHandler.apply(&db, &add_payload()).await.unwrap();
      EditHandler.apply(&db, &edit_payload()).await.unwrap();

      EditHandler.compensate(&db, &edit_payload()).await.unwrap();

      let row = contact_row(&db, 42, 95_001).await.expect("contact restored");
      assert_eq!(row.standing(), 5.0);
      assert!(!row.is_watched());
      assert_eq!(row.label_ids(), "[1]");
    }
  }

  mod remove_handler {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_drops_the_contact_on_apply() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      AddHandler.apply(&db, &add_payload()).await.unwrap();

      RemoveHandler.apply(&db, &remove_payload()).await.unwrap();

      assert!(contact_row(&db, 42, 95_001).await.is_none());
    }

    #[tokio::test]
    async fn it_reinstates_the_contact_on_compensate() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      AddHandler.apply(&db, &add_payload()).await.unwrap();
      RemoveHandler.apply(&db, &remove_payload()).await.unwrap();

      RemoveHandler.compensate(&db, &remove_payload()).await.unwrap();

      let row = contact_row(&db, 42, 95_001).await.expect("contact reinstated");
      assert_eq!(row.standing(), 5.0);
      assert_eq!(row.label_ids(), "[1]");
    }
  }

  mod execute {
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;
    use crate::clients::http;

    async fn esi_client(server: &MockServer) -> esi::Client {
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db)).build();
      esi::Client::with_base_url(http, server.uri())
    }

    #[tokio::test]
    async fn it_posts_an_add_to_esi() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/characters/42/contacts/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([95_001])))
        .expect(1)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      AddHandler.execute(&db, &esi, &grant, &add_payload()).await.unwrap();
    }

    #[tokio::test]
    async fn it_puts_an_edit_to_esi() {
      let server = MockServer::start().await;
      Mock::given(method("PUT"))
        .and(path("/characters/42/contacts/"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      EditHandler.execute(&db, &esi, &grant, &edit_payload()).await.unwrap();
    }

    #[tokio::test]
    async fn it_deletes_a_remove_from_esi() {
      let server = MockServer::start().await;
      Mock::given(method("DELETE"))
        .and(path("/characters/42/contacts/"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      RemoveHandler
        .execute(&db, &esi, &grant, &remove_payload())
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_surfaces_an_esi_rejection_for_the_drainer_to_compensate() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/characters/42/contacts/"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      let result = AddHandler.execute(&db, &esi, &grant, &add_payload()).await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }
  }
}
