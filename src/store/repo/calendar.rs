use crate::store::{
  Database, Error,
  model::{AttendeeTally, CharacterCalendarAttendee, CharacterCalendarEvent},
};

pub async fn upsert_complete(
  db: &Database,
  event: &CharacterCalendarEvent,
  attendees: &[CharacterCalendarAttendee],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query(
    "INSERT INTO character_calendar \
      (character_id, event_id, timestamp, duration_minutes, importance, owner_id, owner_name, \
      owner_type, response, title, body, fetched_at) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT (character_id, event_id) DO UPDATE SET \
      timestamp = excluded.timestamp, duration_minutes = excluded.duration_minutes, \
      importance = excluded.importance, owner_id = excluded.owner_id, owner_name = excluded.owner_name, \
      owner_type = excluded.owner_type, response = excluded.response, title = excluded.title, \
      body = excluded.body, fetched_at = excluded.fetched_at",
  )
  .bind(event.character_id())
  .bind(event.event_id())
  .bind(event.timestamp())
  .bind(event.duration_minutes())
  .bind(event.importance())
  .bind(event.owner_id())
  .bind(event.owner_name())
  .bind(event.owner_type())
  .bind(event.response())
  .bind(event.title())
  .bind(event.body())
  .bind(event.fetched_at())
  .execute(&mut *tx)
  .await?;

  sqlx::query("DELETE FROM character_calendar_attendees WHERE character_id = ? AND event_id = ?")
    .bind(event.character_id())
    .bind(event.event_id())
    .execute(&mut *tx)
    .await?;

  for attendee in attendees {
    sqlx::query(
      "INSERT INTO character_calendar_attendees \
        (character_id, event_id, attendee_id, event_response) VALUES (?, ?, ?, ?)",
    )
    .bind(attendee.character_id())
    .bind(attendee.event_id())
    .bind(attendee.attendee_id())
    .bind(attendee.event_response())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn set_response(db: &Database, character_id: i64, event_id: i64, response: &str) -> Result<(), Error> {
  sqlx::query("UPDATE character_calendar SET response = ? WHERE character_id = ? AND event_id = ?")
    .bind(response)
    .bind(character_id)
    .bind(event_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn events(db: &Database, character_id: i64) -> Result<Vec<CharacterCalendarEvent>, Error> {
  let rows = sqlx::query_as::<_, CharacterCalendarEvent>(
    "SELECT body, character_id, duration_minutes, event_id, fetched_at, importance, owner_id, owner_name, \
      owner_type, response, timestamp, title FROM character_calendar \
    WHERE character_id = ? ORDER BY timestamp, event_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn combined(db: &Database) -> Result<Vec<CharacterCalendarEvent>, Error> {
  let rows = sqlx::query_as::<_, CharacterCalendarEvent>(
    "SELECT body, character_id, duration_minutes, event_id, fetched_at, importance, owner_id, owner_name, \
      owner_type, response, timestamp, title FROM character_calendar \
    ORDER BY timestamp, character_id, event_id",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn event(db: &Database, character_id: i64, event_id: i64) -> Result<Option<CharacterCalendarEvent>, Error> {
  let row = sqlx::query_as::<_, CharacterCalendarEvent>(
    "SELECT body, character_id, duration_minutes, event_id, fetched_at, importance, owner_id, owner_name, \
      owner_type, response, timestamp, title FROM character_calendar \
    WHERE character_id = ? AND event_id = ?",
  )
  .bind(character_id)
  .bind(event_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn attendees(
  db: &Database,
  character_id: i64,
  event_id: i64,
) -> Result<Vec<CharacterCalendarAttendee>, Error> {
  let rows = sqlx::query_as::<_, CharacterCalendarAttendee>(
    "SELECT attendee_id, character_id, event_id, event_response FROM character_calendar_attendees \
    WHERE character_id = ? AND event_id = ? ORDER BY attendee_id",
  )
  .bind(character_id)
  .bind(event_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn attendee_tally(db: &Database, character_id: i64, event_id: i64) -> Result<AttendeeTally, Error> {
  let tally = sqlx::query_as::<_, AttendeeTally>(
    "SELECT \
      COUNT(*) FILTER (WHERE event_response = 'accepted')  AS accepted, \
      COUNT(*) FILTER (WHERE event_response = 'declined')  AS declined, \
      COUNT(*) AS invited, \
      COUNT(*) FILTER (WHERE event_response = 'tentative') AS tentative \
    FROM character_calendar_attendees WHERE character_id = ? AND event_id = ?",
  )
  .bind(character_id)
  .bind(event_id)
  .fetch_one(&db.0)
  .await?;
  Ok(tally)
}

pub async fn attention_count(db: &Database, now: &str) -> Result<i64, Error> {
  // Attention = upcoming events that still want an RSVP from the pilot: those whose owner type can
  // be responded to (corp/alliance/faction) and that haven't been answered yet. Events you can't
  // respond to (personal, EVE-server, Pod overlays) and ones you've already answered never count,
  // so acknowledging an invite clears the rail badge.
  let count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM character_calendar \
    WHERE timestamp >= ? AND response = 'not_responded' \
    AND owner_type IN ('corporation', 'alliance', 'faction')",
  )
  .bind(now)
  .fetch_one(&db.0)
  .await?;
  Ok(count)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
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

  fn make_event(character_id: i64, event_id: i64, ts: &str, importance: i64, response: &str) -> CharacterCalendarEvent {
    CharacterCalendarEvent {
      body: Some("<p>Form up at the Keepstar.</p>".to_owned()),
      character_id,
      duration_minutes: 90,
      event_id,
      fetched_at: "2026-06-12T00:00:00Z".to_owned(),
      importance,
      owner_id: 98_000_001,
      owner_name: "Test Corp".to_owned(),
      owner_type: "corporation".to_owned(),
      response: response.to_owned(),
      timestamp: ts.to_owned(),
      title: "Doctrine refit night".to_owned(),
    }
  }

  fn attendee(character_id: i64, event_id: i64, attendee_id: i64, response: &str) -> CharacterCalendarAttendee {
    CharacterCalendarAttendee {
      attendee_id,
      character_id,
      event_id,
      event_response: response.to_owned(),
    }
  }

  mod attendee_tally {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_each_response_and_totals_invited() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert_complete(
        &db,
        &make_event(42, 1, "2026-06-20T19:00:00Z", 0, "accepted"),
        &[
          attendee(42, 1, 1, "accepted"),
          attendee(42, 1, 2, "accepted"),
          attendee(42, 1, 3, "tentative"),
          attendee(42, 1, 4, "declined"),
          attendee(42, 1, 5, "not_responded"),
        ],
      )
      .await
      .unwrap();

      let tally = super::attendee_tally(&db, 42, 1).await.unwrap();

      assert_eq!(tally.accepted, 2);
      assert_eq!(tally.tentative, 1);
      assert_eq!(tally.declined, 1);
      assert_eq!(tally.invited, 5);
    }
  }

  mod attention_count {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_upcoming_respondable_unanswered_events() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let now = "2026-06-12T00:00:00Z";

      // Future corp invite, already answered — excluded (acknowledging clears it).
      super::upsert_complete(&db, &make_event(42, 1, "2026-06-20T19:00:00Z", 1, "accepted"), &[])
        .await
        .unwrap();
      // Future corp invite, unanswered — the one that counts.
      super::upsert_complete(&db, &make_event(42, 2, "2026-06-21T19:00:00Z", 0, "not_responded"), &[])
        .await
        .unwrap();
      // Future non-respondable (EVE server) downtime, unanswered — excluded, nothing to answer.
      let mut downtime = make_event(42, 3, "2026-06-22T11:00:00Z", 1, "not_responded");
      downtime.owner_type = "eve_server".to_owned();
      super::upsert_complete(&db, &downtime, &[]).await.unwrap();
      // Past corp invite, unanswered — excluded, already elapsed.
      super::upsert_complete(&db, &make_event(42, 4, "2026-06-01T19:00:00Z", 1, "not_responded"), &[])
        .await
        .unwrap();

      let count = super::attention_count(&db, now).await.unwrap();

      assert_eq!(count, 1);
    }
  }

  mod combined {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_merges_all_characters_events_chronologically() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      super::upsert_complete(&db, &make_event(42, 1, "2026-06-22T19:00:00Z", 0, "accepted"), &[])
        .await
        .unwrap();
      super::upsert_complete(&db, &make_event(43, 2, "2026-06-20T19:00:00Z", 0, "accepted"), &[])
        .await
        .unwrap();

      let combined = super::combined(&db).await.unwrap();

      assert_eq!(
        combined
          .iter()
          .map(|e| (e.character_id(), e.event_id()))
          .collect::<Vec<_>>(),
        [(43, 2), (42, 1)]
      );
    }
  }

  mod events {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_orders_a_characters_events_chronologically() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert_complete(&db, &make_event(42, 2, "2026-06-22T19:00:00Z", 0, "accepted"), &[])
        .await
        .unwrap();
      super::upsert_complete(&db, &make_event(42, 1, "2026-06-20T19:00:00Z", 0, "accepted"), &[])
        .await
        .unwrap();

      let events = super::events(&db, 42).await.unwrap();

      assert_eq!(events.iter().map(|e| e.event_id()).collect::<Vec<_>>(), [1, 2]);
    }
  }

  mod set_response {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_updates_the_viewers_rsvp_in_place() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert_complete(&db, &make_event(42, 1, "2026-06-20T19:00:00Z", 0, "not_responded"), &[])
        .await
        .unwrap();

      super::set_response(&db, 42, 1, "tentative").await.unwrap();

      assert_eq!(super::event(&db, 42, 1).await.unwrap().unwrap().response(), "tentative");
    }
  }

  mod upsert_complete {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_cascades_when_the_character_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert_complete(
        &db,
        &make_event(42, 1, "2026-06-20T19:00:00Z", 0, "accepted"),
        &[attendee(42, 1, 1, "accepted")],
      )
      .await
      .unwrap();

      sqlx::query("DELETE FROM characters WHERE id = 42")
        .execute(db.writer())
        .await
        .unwrap();

      assert!(super::events(&db, 42).await.unwrap().is_empty());
      assert!(super::attendees(&db, 42, 1).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_persists_event_and_attendees_together_and_round_trips() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::upsert_complete(
        &db,
        &make_event(42, 1, "2026-06-20T19:00:00Z", 1, "accepted"),
        &[
          attendee(42, 1, 95_000_001, "accepted"),
          attendee(42, 1, 95_000_002, "declined"),
        ],
      )
      .await
      .unwrap();

      let events = super::events(&db, 42).await.unwrap();
      assert_eq!(events.len(), 1);
      assert_eq!(events[0].title(), "Doctrine refit night");
      assert_eq!(events[0].importance(), 1);
      assert_eq!(events[0].response(), "accepted");
      assert_eq!(events[0].body().as_deref(), Some("<p>Form up at the Keepstar.</p>"));

      let attendees = super::attendees(&db, 42, 1).await.unwrap();
      assert_eq!(
        attendees.iter().map(|a| a.attendee_id()).collect::<Vec<_>>(),
        [95_000_001, 95_000_002]
      );
    }

    #[tokio::test]
    async fn it_replaces_the_attendee_set_on_re_sync() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::upsert_complete(
        &db,
        &make_event(42, 1, "2026-06-20T19:00:00Z", 0, "accepted"),
        &[attendee(42, 1, 1, "accepted")],
      )
      .await
      .unwrap();
      super::upsert_complete(
        &db,
        &make_event(42, 1, "2026-06-20T19:00:00Z", 0, "accepted"),
        &[attendee(42, 1, 2, "tentative")],
      )
      .await
      .unwrap();

      let attendees = super::attendees(&db, 42, 1).await.unwrap();
      assert_eq!(attendees.len(), 1);
      assert_eq!(attendees[0].attendee_id(), 2);
    }
  }
}
