use std::collections::HashMap;

use crate::store::{
  Database, Error,
  model::{CaptainsLog, PromptConfig},
};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnswerKey {
  Blocked,
  Build,
  Combat,
  Goal,
  Next,
  Remember,
  Research,
  Skill,
}

#[allow(dead_code)]
impl AnswerKey {
  pub const ALL: [AnswerKey; 8] = [
    AnswerKey::Goal,
    AnswerKey::Remember,
    AnswerKey::Blocked,
    AnswerKey::Build,
    AnswerKey::Skill,
    AnswerKey::Combat,
    AnswerKey::Next,
    AnswerKey::Research,
  ];

  pub fn from_key(key: &str) -> Option<AnswerKey> {
    match key {
      "blocked" => Some(AnswerKey::Blocked),
      "build" => Some(AnswerKey::Build),
      "combat" => Some(AnswerKey::Combat),
      "goal" => Some(AnswerKey::Goal),
      "next" => Some(AnswerKey::Next),
      "remember" => Some(AnswerKey::Remember),
      "research" => Some(AnswerKey::Research),
      "skill" => Some(AnswerKey::Skill),
      _ => None,
    }
  }

  pub fn as_key(self) -> &'static str {
    match self {
      AnswerKey::Blocked => "blocked",
      AnswerKey::Build => "build",
      AnswerKey::Combat => "combat",
      AnswerKey::Goal => "goal",
      AnswerKey::Next => "next",
      AnswerKey::Remember => "remember",
      AnswerKey::Research => "research",
      AnswerKey::Skill => "skill",
    }
  }
}

/// Resolves to the storage key for an answer; implemented for the fixed `AnswerKey` catalog and for
/// arbitrary `&str` so `PromptConfig`'s dynamic, user-defined questions can be written the same way.
pub trait AnswerId {
  fn question_id(&self) -> &str;
}

impl AnswerId for AnswerKey {
  fn question_id(&self) -> &str {
    self.as_key()
  }
}

impl AnswerId for &str {
  fn question_id(&self) -> &str {
    self
  }
}

#[allow(dead_code)]
pub async fn upsert_narrative(db: &Database, date: &str, narrative: Option<&str>) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  sqlx::query(
    "INSERT INTO captains_log (date, narrative, created_at, updated_at) VALUES (?, ?, ?, ?) ON CONFLICT (date) DO UPDATE SET narrative = excluded.narrative, updated_at = excluded.updated_at",
  )
  .bind(date)
  .bind(narrative)
  .bind(&now)
  .bind(&now)
  .execute(db.writer())
  .await?;
  Ok(())
}

#[allow(dead_code)]
pub async fn upsert_answer<K: AnswerId>(db: &Database, date: &str, key: K, value: Option<&str>) -> Result<(), Error> {
  let question_id = key.question_id();
  let now = chrono::Utc::now().to_rfc3339();

  // Ensure the day row exists before writing an answer: `get` looks up captains_log by date and
  // returns None if it's absent, so an answer alone would otherwise be invisible.
  sqlx::query(
    "INSERT INTO captains_log (date, created_at, updated_at) VALUES (?, ?, ?) ON CONFLICT (date) DO UPDATE SET updated_at = excluded.updated_at",
  )
  .bind(date)
  .bind(&now)
  .bind(&now)
  .execute(db.writer())
  .await?;

  match value {
    Some(value) => {
      sqlx::query(
        "INSERT INTO captains_log_answer (date, question_id, value, created_at, updated_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT (date, question_id) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
      )
      .bind(date)
      .bind(question_id)
      .bind(value)
      .bind(&now)
      .bind(&now)
      .execute(db.writer())
      .await?;
    }
    // Clearing an answer deletes the row rather than storing a null value, so `answers()` only
    // ever contains questions that have content.
    None => {
      sqlx::query("DELETE FROM captains_log_answer WHERE date = ? AND question_id = ?")
        .bind(date)
        .bind(question_id)
        .execute(db.writer())
        .await?;
    }
  }
  Ok(())
}

#[allow(dead_code)]
pub async fn mark_complete(db: &Database, date: &str) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  sqlx::query(
    "INSERT INTO captains_log (date, marked_complete, created_at, updated_at) VALUES (?, 1, ?, ?) ON CONFLICT (date) DO UPDATE SET marked_complete = 1, updated_at = excluded.updated_at",
  )
  .bind(date)
  .bind(&now)
  .bind(&now)
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn get(db: &Database, date: &str) -> Result<Option<CaptainsLog>, Error> {
  let base = sqlx::query_as::<_, (String, String, bool, Option<String>, String)>(
    "SELECT created_at, date, marked_complete, narrative, updated_at FROM captains_log WHERE date = ?",
  )
  .bind(date)
  .fetch_optional(db.reader())
  .await?;

  let Some((created_at, date, marked_complete, narrative, updated_at)) = base else {
    return Ok(None);
  };

  let rows =
    sqlx::query_as::<_, (String, Option<String>)>("SELECT question_id, value FROM captains_log_answer WHERE date = ?")
      .bind(&date)
      .fetch_all(db.reader())
      .await?;

  let mut answers: HashMap<String, String> = HashMap::new();
  for (question_id, value) in rows {
    if let Some(value) = value {
      answers.insert(question_id, value);
    }
  }

  Ok(Some(CaptainsLog {
    blocked: answers.get("blocked").cloned(),
    build: answers.get("build").cloned(),
    combat: answers.get("combat").cloned(),
    goal: answers.get("goal").cloned(),
    next: answers.get("next").cloned(),
    remember: answers.get("remember").cloned(),
    research: answers.get("research").cloned(),
    skill: answers.get("skill").cloned(),
    created_at,
    date,
    marked_complete,
    narrative,
    updated_at,
    answers,
  }))
}

#[allow(dead_code)]
pub async fn dates(db: &Database) -> Result<Vec<String>, Error> {
  let rows = sqlx::query_scalar::<_, String>(
    "SELECT DISTINCT day FROM ( \
      SELECT date AS day FROM captains_log WHERE TRIM(COALESCE(narrative, '')) <> '' \
      UNION SELECT date FROM captains_log_answer WHERE TRIM(COALESCE(value, '')) <> '' \
      UNION SELECT date FROM field_notes WHERE TRIM(COALESCE(text, '')) <> '' \
    ) ORDER BY day DESC",
  )
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn load_prompt_config(db: &Database) -> Result<PromptConfig, Error> {
  let document = sqlx::query_scalar::<_, String>("SELECT document FROM captains_log_prompt_config WHERE id = 1")
    .fetch_optional(db.reader())
    .await?;

  let mut config = document
    .and_then(|document| serde_json::from_str::<PromptConfig>(&document).ok())
    .unwrap_or_default();
  config.normalize();
  Ok(config)
}

#[allow(dead_code)]
pub async fn save_prompt_config(db: &Database, config: &PromptConfig) -> Result<(), Error> {
  let document = serde_json::to_string(config)?;
  let now = chrono::Utc::now().to_rfc3339();
  sqlx::query(
    "INSERT INTO captains_log_prompt_config (id, version, document, created_at, updated_at) VALUES (1, ?, ?, ?, ?) ON CONFLICT (id) DO UPDATE SET version = excluded.version, document = excluded.document, updated_at = excluded.updated_at",
  )
  .bind(i64::from(config.version))
  .bind(&document)
  .bind(&now)
  .bind(&now)
  .execute(db.writer())
  .await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  mod answer_key {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_every_catalog_key() {
      for key in AnswerKey::ALL {
        assert_eq!(AnswerKey::from_key(key.as_key()), Some(key));
      }
    }

    #[test]
    fn it_rejects_an_unknown_key() {
      assert_eq!(AnswerKey::from_key("narrative"), None);
      assert_eq!(AnswerKey::from_key(""), None);
      assert_eq!(AnswerKey::from_key("Goal"), None);
    }

    #[test]
    fn it_lists_the_eight_catalog_keys_once_each() {
      let mut keys: Vec<&str> = AnswerKey::ALL.iter().map(|key| key.as_key()).collect();
      keys.sort_unstable();
      keys.dedup();

      assert_eq!(keys.len(), 8);
    }
  }

  mod mark_complete {
    use super::*;

    #[tokio::test]
    async fn it_creates_the_day_row_when_absent() {
      let db = crate::store::open_test().await.unwrap();

      mark_complete(&db, "2026-07-01").await.unwrap();

      let row = get(&db, "2026-07-01").await.unwrap().unwrap();
      assert!(row.marked_complete);
    }

    #[tokio::test]
    async fn it_survives_a_later_answer_upsert() {
      let db = crate::store::open_test().await.unwrap();

      mark_complete(&db, "2026-07-01").await.unwrap();
      upsert_answer(&db, "2026-07-01", AnswerKey::Goal, Some("undock"))
        .await
        .unwrap();

      let row = get(&db, "2026-07-01").await.unwrap().unwrap();
      assert!(row.marked_complete);
      assert_eq!(row.goal.as_deref(), Some("undock"));
    }
  }

  mod upsert_narrative {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_stores_and_reads_back_the_narrative() {
      let db = store::open_test().await.unwrap();
      super::upsert_narrative(&db, "2026-07-06", Some("Clean roam, two kills."))
        .await
        .unwrap();

      let row = super::get(&db, "2026-07-06").await.unwrap().unwrap();

      assert_eq!(row.narrative().as_deref(), Some("Clean roam, two kills."));
    }

    #[tokio::test]
    async fn it_updates_an_existing_day_without_duplicating_rows() {
      let db = store::open_test().await.unwrap();
      super::upsert_narrative(&db, "2026-07-06", Some("First pass."))
        .await
        .unwrap();
      super::upsert_narrative(&db, "2026-07-06", Some("Revised."))
        .await
        .unwrap();

      let row = super::get(&db, "2026-07-06").await.unwrap().unwrap();

      assert_eq!(row.narrative().as_deref(), Some("Revised."));
      assert_eq!(super::dates(&db).await.unwrap(), vec!["2026-07-06".to_owned()]);
    }
  }

  mod upsert_answer {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_writes_the_column_matching_the_key() {
      let db = store::open_test().await.unwrap();
      super::upsert_answer(&db, "2026-07-06", AnswerKey::Goal, Some("Spin up the barge line."))
        .await
        .unwrap();
      super::upsert_answer(&db, "2026-07-06", AnswerKey::Combat, Some("Lost the Caracal."))
        .await
        .unwrap();

      let row = super::get(&db, "2026-07-06").await.unwrap().unwrap();

      assert_eq!(row.goal().as_deref(), Some("Spin up the barge line."));
      assert_eq!(row.combat().as_deref(), Some("Lost the Caracal."));
      assert_eq!(row.blocked(), &None);
    }

    #[tokio::test]
    async fn it_leaves_a_prior_answer_untouched_when_writing_another_key() {
      let db = store::open_test().await.unwrap();
      super::upsert_answer(&db, "2026-07-06", AnswerKey::Goal, Some("Original goal."))
        .await
        .unwrap();
      super::upsert_answer(&db, "2026-07-06", AnswerKey::Next, Some("Next op."))
        .await
        .unwrap();

      let row = super::get(&db, "2026-07-06").await.unwrap().unwrap();

      assert_eq!(row.goal().as_deref(), Some("Original goal."));
      assert_eq!(row.next().as_deref(), Some("Next op."));
    }

    #[tokio::test]
    async fn it_stores_and_reads_an_arbitrary_string_question_id() {
      let db = store::open_test().await.unwrap();
      super::upsert_answer(&db, "2026-07-06", "custom_mood", Some("focused"))
        .await
        .unwrap();

      let row = super::get(&db, "2026-07-06").await.unwrap().unwrap();

      assert_eq!(row.answers().get("custom_mood").map(String::as_str), Some("focused"));
      assert_eq!(row.goal(), &None);
    }

    #[tokio::test]
    async fn it_clears_an_answer_when_written_with_none() {
      let db = store::open_test().await.unwrap();
      super::upsert_answer(&db, "2026-07-06", AnswerKey::Goal, Some("Undock."))
        .await
        .unwrap();
      super::upsert_answer(&db, "2026-07-06", AnswerKey::Goal, None)
        .await
        .unwrap();

      let row = super::get(&db, "2026-07-06").await.unwrap().unwrap();

      assert_eq!(row.goal(), &None);
      assert!(!row.answers().contains_key("goal"));
    }

    #[tokio::test]
    async fn it_reads_the_typed_accessor_from_the_string_keyed_store() {
      let db = store::open_test().await.unwrap();
      super::upsert_answer(&db, "2026-07-06", "goal", Some("Reship."))
        .await
        .unwrap();

      let row = super::get(&db, "2026-07-06").await.unwrap().unwrap();

      assert_eq!(row.goal().as_deref(), Some("Reship."));
    }
  }

  mod get {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_absent_day_without_creating_a_row() {
      let db = store::open_test().await.unwrap();

      let row = super::get(&db, "2026-07-06").await.unwrap();

      assert_eq!(row, None);
      assert!(super::dates(&db).await.unwrap().is_empty());
    }
  }

  mod dates {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_lists_authored_days_newest_first() {
      let db = store::open_test().await.unwrap();
      super::upsert_narrative(&db, "2026-07-04", Some("a")).await.unwrap();
      super::upsert_narrative(&db, "2026-07-06", Some("b")).await.unwrap();
      super::upsert_narrative(&db, "2026-07-05", Some("c")).await.unwrap();

      let dates = super::dates(&db).await.unwrap();

      assert_eq!(
        dates,
        vec![
          "2026-07-06".to_owned(),
          "2026-07-05".to_owned(),
          "2026-07-04".to_owned()
        ]
      );
    }

    #[tokio::test]
    async fn it_excludes_a_content_less_marked_complete_day() {
      let db = store::open_test().await.unwrap();
      super::mark_complete(&db, "2026-06-20").await.unwrap();

      assert!(super::dates(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_includes_an_answer_only_day() {
      let db = store::open_test().await.unwrap();
      super::upsert_answer(&db, "2026-07-04", AnswerKey::Goal, Some("Undock."))
        .await
        .unwrap();

      assert_eq!(super::dates(&db).await.unwrap(), vec!["2026-07-04".to_owned()]);
    }

    #[tokio::test]
    async fn it_includes_a_field_note_only_day() {
      let db = store::open_test().await.unwrap();
      crate::store::repo::field_notes::insert(&db, "2026-07-04", "Scouted the pipe.")
        .await
        .unwrap();

      assert_eq!(super::dates(&db).await.unwrap(), vec!["2026-07-04".to_owned()]);
    }

    #[tokio::test]
    async fn it_excludes_a_day_with_a_marked_complete_row_but_no_content() {
      let db = store::open_test().await.unwrap();
      super::upsert_answer(&db, "2026-07-04", AnswerKey::Goal, Some("Undock."))
        .await
        .unwrap();
      super::mark_complete(&db, "2026-06-20").await.unwrap();

      assert_eq!(super::dates(&db).await.unwrap(), vec!["2026-07-04".to_owned()]);
    }
  }

  mod prompt_config {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_loads_the_seeded_default_on_a_fresh_database() {
      let db = store::open_test().await.unwrap();

      let config = super::load_prompt_config(&db).await.unwrap();

      assert_eq!(config, PromptConfig::default());
    }

    #[tokio::test]
    async fn it_round_trips_a_saved_config() {
      let db = store::open_test().await.unwrap();
      let mut config = PromptConfig::default();
      config.sections[0].label = "Morning".to_owned();

      super::save_prompt_config(&db, &config).await.unwrap();
      let loaded = super::load_prompt_config(&db).await.unwrap();

      assert_eq!(loaded, config);
    }
  }
}
