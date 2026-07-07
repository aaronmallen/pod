use chrono::Utc;

use crate::store::{Database, Error, model::FieldNote};

pub async fn insert(db: &Database, date: &str, text: &str) -> Result<FieldNote, Error> {
  let now = Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, FieldNote>(
    "INSERT INTO field_notes (created_at, date, text, updated_at) VALUES (?, ?, ?, ?) \
    RETURNING created_at, date, id, text, updated_at",
  )
  .bind(&now)
  .bind(date)
  .bind(text)
  .bind(&now)
  .fetch_one(db.writer())
  .await?;
  Ok(row)
}

pub async fn list_for_date(db: &Database, date: &str) -> Result<Vec<FieldNote>, Error> {
  let rows = sqlx::query_as::<_, FieldNote>(
    "SELECT created_at, date, id, text, updated_at FROM field_notes WHERE date = ? ORDER BY id DESC",
  )
  .bind(date)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn update(db: &Database, id: i64, text: &str) -> Result<u64, Error> {
  let now = Utc::now().to_rfc3339();
  let result = sqlx::query("UPDATE field_notes SET text = ?, updated_at = ? WHERE id = ?")
    .bind(text)
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

pub async fn delete(db: &Database, id: i64) -> Result<u64, Error> {
  let result = sqlx::query("DELETE FROM field_notes WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  mod insert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_stamps_and_returns_the_new_note() {
      let db = store::open_test().await.unwrap();

      let note = insert(&db, "2026-07-05", "Cyno up in Tama").await.unwrap();

      assert!(note.id > 0);
      assert_eq!(note.date, "2026-07-05");
      assert_eq!(note.text, "Cyno up in Tama");
      assert_eq!(note.created_at, note.updated_at);
    }
  }

  mod list_for_date {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_lists_a_days_notes_newest_first() {
      let db = store::open_test().await.unwrap();
      insert(&db, "2026-07-05", "first").await.unwrap();
      insert(&db, "2026-07-05", "second").await.unwrap();
      insert(&db, "2026-07-06", "other day").await.unwrap();

      let notes = list_for_date(&db, "2026-07-05").await.unwrap();

      assert_eq!(notes.len(), 2);
      assert_eq!(notes[0].text, "second");
      assert_eq!(notes[1].text, "first");
    }

    #[tokio::test]
    async fn it_is_empty_for_a_day_with_no_notes() {
      let db = store::open_test().await.unwrap();

      assert!(list_for_date(&db, "2026-07-05").await.unwrap().is_empty());
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_edits_the_text_and_keeps_the_created_stamp() {
      let db = store::open_test().await.unwrap();
      let note = insert(&db, "2026-07-05", "draft").await.unwrap();

      let affected = update(&db, note.id, "final").await.unwrap();

      assert_eq!(affected, 1);
      let stored = list_for_date(&db, "2026-07-05").await.unwrap();
      assert_eq!(stored[0].text, "final");
      assert_eq!(stored[0].created_at, note.created_at);
    }
  }

  mod delete {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_removes_a_single_note_by_id() {
      let db = store::open_test().await.unwrap();
      let keep = insert(&db, "2026-07-05", "keep").await.unwrap();
      let drop = insert(&db, "2026-07-05", "drop").await.unwrap();

      let affected = delete(&db, drop.id).await.unwrap();

      assert_eq!(affected, 1);
      let remaining = list_for_date(&db, "2026-07-05").await.unwrap();
      assert_eq!(remaining.len(), 1);
      assert_eq!(remaining[0].id, keep.id);
    }
  }
}
