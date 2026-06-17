use crate::store::{
  Database,
  model::OwnerType,
  repo::{calendar, infra},
};

pub(super) async fn respond(db: Database, character_id: i64, event_id: i64, response: String, previous: String) {
  let _ = calendar::set_response(&db, character_id, event_id, &response).await;
  // "calendar.respond" must match OutboxKind::CalendarRespond in src/sync/outbox.rs — no compile-time link.
  let _ = infra::append(
    &db,
    OwnerType::Character,
    character_id,
    "calendar.respond",
    &respond_payload(character_id, event_id, &response, &previous),
    Some(&respond_dedupe(event_id)),
  )
  .await;
}

pub(super) fn respond_dedupe(event_id: i64) -> String {
  format!("respond:{event_id}")
}

pub(super) fn respond_payload(character_id: i64, event_id: i64, response: &str, previous: &str) -> String {
  format!(
    "{{\"character_id\":{character_id},\"event_id\":{event_id},\
      \"response\":\"{response}\",\"previous_response\":\"{previous}\"}}"
  )
}

#[cfg(test)]
mod tests {
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

  mod respond {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_collapses_repeated_responses_onto_one_row() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_event(&db, 42, 7, "not_responded").await;

      respond(db.clone(), 42, 7, "accepted".to_owned(), "not_responded".to_owned()).await;
      respond(db.clone(), 42, 7, "declined".to_owned(), "accepted".to_owned()).await;

      let rows = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox WHERE kind = 'calendar.respond'")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(rows, 1, "the per-event dedupe key collapses the repeated response");
    }

    #[tokio::test]
    async fn it_flips_the_local_mirror_and_enqueues_one_outbox_write() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_event(&db, 42, 7, "not_responded").await;

      respond(db.clone(), 42, 7, "accepted".to_owned(), "not_responded".to_owned()).await;

      assert_eq!(
        calendar::event(&db, 42, 7).await.unwrap().unwrap().response(),
        "accepted"
      );

      let pending = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM outbox WHERE kind = 'calendar.respond' AND status IN ('pending', 'inflight')",
      )
      .fetch_one(&db.0)
      .await
      .unwrap();
      assert_eq!(pending, 1);
    }
  }

  mod respond_dedupe {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keys_by_event() {
      assert_eq!(respond_dedupe(7), "respond:7");
    }
  }

  mod respond_payload {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_carries_the_previous_response_for_compensation() {
      assert_eq!(
        respond_payload(42, 7, "accepted", "tentative"),
        "{\"character_id\":42,\"event_id\":7,\"response\":\"accepted\",\"previous_response\":\"tentative\"}"
      );
    }
  }
}
