use chrono::Utc;
use sqlx::{QueryBuilder, Sqlite};

use crate::store::{Database, Error, model::CalendarEventNote};

#[cfg_attr(not(test), expect(dead_code))]
pub async fn get(db: &Database, character_id: i64, event_id: i64) -> Result<Option<CalendarEventNote>, Error> {
  let row = sqlx::query_as::<_, CalendarEventNote>(
    "SELECT character_id, created_at, event_id, note, updated_at FROM calendar_event_notes \
    WHERE character_id = ? AND event_id = ?",
  )
  .bind(character_id)
  .bind(event_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn list_for_events(
  db: &Database,
  character_id: i64,
  event_ids: &[i64],
) -> Result<Vec<CalendarEventNote>, Error> {
  if event_ids.is_empty() {
    return Ok(Vec::new());
  }
  let mut builder = QueryBuilder::<Sqlite>::new(
    "SELECT character_id, created_at, event_id, note, updated_at FROM calendar_event_notes WHERE character_id = ",
  );
  builder.push_bind(character_id).push(" AND event_id IN (");
  let mut separated = builder.separated(", ");
  for event_id in event_ids {
    separated.push_bind(*event_id);
  }
  separated.push_unseparated(") ");
  builder.push("ORDER BY event_id");
  let rows = builder.build_query_as::<CalendarEventNote>().fetch_all(&db.0).await?;
  Ok(rows)
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn upsert(db: &Database, character_id: i64, event_id: i64, note: &str) -> Result<(), Error> {
  let now = Utc::now().to_rfc3339();
  sqlx::query(
    "INSERT INTO calendar_event_notes (character_id, created_at, event_id, note, updated_at) \
      VALUES (?, ?, ?, ?, ?) \
    ON CONFLICT (character_id, event_id) DO UPDATE SET \
      note = excluded.note, updated_at = excluded.updated_at",
  )
  .bind(character_id)
  .bind(&now)
  .bind(event_id)
  .bind(note)
  .bind(&now)
  .execute(db.writer())
  .await?;
  Ok(())
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

  mod get {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_when_no_note_exists() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      assert_eq!(super::get(&db, 42, 1).await.unwrap(), None);
    }
  }

  mod list_for_events {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_for_an_empty_id_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert(&db, 42, 1, "Form up early").await.unwrap();

      assert!(super::list_for_events(&db, 42, &[]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_returns_only_notes_for_the_requested_events() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert(&db, 42, 1, "Form up early").await.unwrap();
      super::upsert(&db, 42, 2, "Bring the logi").await.unwrap();
      super::upsert(&db, 42, 3, "Moon pops").await.unwrap();

      let notes = super::list_for_events(&db, 42, &[3, 1]).await.unwrap();

      assert_eq!(notes.iter().map(|n| n.event_id).collect::<Vec<_>>(), [1, 3]);
      assert_eq!(notes[0].note, "Form up early");
      assert_eq!(notes[1].note, "Moon pops");
    }
  }

  mod upsert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_then_round_trips_through_get() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::upsert(&db, 42, 7, "Refit before the roam").await.unwrap();

      let note = super::get(&db, 42, 7).await.unwrap().unwrap();
      assert_eq!(note.character_id, 42);
      assert_eq!(note.event_id, 7);
      assert_eq!(note.note, "Refit before the roam");
      assert_eq!(note.created_at, note.updated_at);
    }

    #[tokio::test]
    async fn it_overwrites_the_note_in_place_on_conflict() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::upsert(&db, 42, 7, "First draft").await.unwrap();
      let created = super::get(&db, 42, 7).await.unwrap().unwrap().created_at;
      super::upsert(&db, 42, 7, "Final draft").await.unwrap();

      let notes = super::list_for_events(&db, 42, &[7]).await.unwrap();
      assert_eq!(notes.len(), 1);
      assert_eq!(notes[0].note, "Final draft");
      assert_eq!(notes[0].created_at, created);
    }
  }
}
