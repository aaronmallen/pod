use serde::Deserialize;

use super::outbox::{HandlerFuture, KindHandler, OutboxKind, Registry};
use crate::{
  clients::{self, esi, esi::models::character::RespondRequest, eve_sso::Grant},
  store::{Database, repo::calendar},
};

struct RespondHandler;

impl KindHandler for RespondHandler {
  fn kind(&self) -> OutboxKind {
    OutboxKind::CalendarRespond
  }

  #[cfg(test)]
  fn apply<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = RespondPayload::parse(payload)?;
      calendar::set_response(db, p.character_id, p.event_id, &p.response).await?;
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
      let p = RespondPayload::parse(payload)?;
      let request = RespondRequest {
        response: p.response,
      };
      esi
        .character_authenticated(grant)
        .respond_to_event(p.event_id, &request)
        .await
    })
  }

  fn compensate<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
    Box::pin(async move {
      let p = RespondPayload::parse(payload)?;
      calendar::set_response(db, p.character_id, p.event_id, &p.previous_response).await?;
      Ok(())
    })
  }
}

#[derive(Debug, Deserialize)]
struct RespondPayload {
  character_id: i64,
  event_id: i64,
  previous_response: String,
  response: String,
}

impl RespondPayload {
  fn parse(payload: &str) -> Result<Self, clients::Error> {
    Ok(serde_json::from_str(payload)?)
  }
}

pub(super) fn registry() -> Registry {
  Registry::new().with(Box::new(RespondHandler))
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, CharacterCalendarEvent, Corporation, Gender, Race},
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

  async fn store_event(db: &Database, character_id: i64, event_id: i64, response: &str) {
    let event = CharacterCalendarEvent {
      body: Some("<p>Form up.</p>".to_owned()),
      character_id,
      duration_minutes: 90,
      event_id,
      fetched_at: "2026-06-12T00:00:00Z".to_owned(),
      importance: 0,
      owner_id: 98_000_001,
      owner_name: "Test Corp".to_owned(),
      owner_type: "corporation".to_owned(),
      response: response.to_owned(),
      timestamp: "2026-06-20T19:00:00Z".to_owned(),
      title: "Doctrine refit night".to_owned(),
    };
    calendar::upsert_complete(db, &event, &[]).await.unwrap();
  }

  fn payload(character_id: i64, event_id: i64, response: &str, previous: &str) -> String {
    format!(
      "{{\"character_id\":{character_id},\"event_id\":{event_id},\
        \"response\":\"{response}\",\"previous_response\":\"{previous}\"}}"
    )
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
    async fn it_puts_the_chosen_response_to_esi() {
      let server = MockServer::start().await;
      Mock::given(method("PUT"))
        .and(path("/characters/42/calendar/7/"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      RespondHandler
        .execute(&db, &esi, &grant, &payload(42, 7, "accepted", "not_responded"))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_surfaces_an_esi_rejection_for_the_drainer_to_compensate() {
      let server = MockServer::start().await;
      Mock::given(method("PUT"))
        .and(path("/characters/42/calendar/7/"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let esi = esi_client(&server).await;
      let grant = Grant::new_test("tok", 42);

      let result = RespondHandler
        .execute(&db, &esi, &grant, &payload(42, 7, "accepted", "not_responded"))
        .await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }
  }

  #[tokio::test]
  async fn it_fails_a_malformed_payload() {
    let db = store::open_test().await.unwrap();

    let result = RespondHandler.apply(&db, "not json").await;

    assert!(matches!(result, Err(clients::Error::Json(_))));
  }

  #[tokio::test]
  async fn it_optimistically_writes_the_chosen_response_on_apply() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    store_event(&db, 42, 7, "not_responded").await;

    RespondHandler
      .apply(&db, &payload(42, 7, "accepted", "not_responded"))
      .await
      .unwrap();

    assert_eq!(
      calendar::event(&db, 42, 7).await.unwrap().unwrap().response(),
      "accepted"
    );
  }

  #[test]
  fn it_registers_the_respond_handler() {
    let registry = registry();

    let handler = registry.handler(OutboxKind::CalendarRespond).expect("registered");

    assert_eq!(handler.kind(), OutboxKind::CalendarRespond);
  }

  #[tokio::test]
  async fn it_restores_the_previous_response_on_compensate() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    store_event(&db, 42, 7, "not_responded").await;
    RespondHandler
      .apply(&db, &payload(42, 7, "accepted", "not_responded"))
      .await
      .unwrap();

    RespondHandler
      .compensate(&db, &payload(42, 7, "accepted", "not_responded"))
      .await
      .unwrap();

    assert_eq!(
      calendar::event(&db, 42, 7).await.unwrap().unwrap().response(),
      "not_responded"
    );
  }
}
