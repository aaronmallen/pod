use sqlx::{QueryBuilder, Sqlite};

use crate::store::{Database, Error, model::CaptainsLog};

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

  fn column(self) -> &'static str {
    self.as_key()
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
pub async fn upsert_answer(db: &Database, date: &str, key: AnswerKey, value: Option<&str>) -> Result<(), Error> {
  let column = key.column();
  let now = chrono::Utc::now().to_rfc3339();

  let mut builder = QueryBuilder::<Sqlite>::new("INSERT INTO captains_log (date, ");
  builder.push(column);
  builder.push(", created_at, updated_at) VALUES (");
  builder.push_bind(date);
  builder.push(", ");
  builder.push_bind(value);
  builder.push(", ");
  builder.push_bind(&now);
  builder.push(", ");
  builder.push_bind(&now);
  builder.push(") ON CONFLICT (date) DO UPDATE SET ");
  builder.push(column);
  builder.push(" = excluded.");
  builder.push(column);
  builder.push(", updated_at = excluded.updated_at");
  builder.build().execute(db.writer()).await?;
  Ok(())
}

#[allow(dead_code)]
pub async fn get(db: &Database, date: &str) -> Result<Option<CaptainsLog>, Error> {
  let row = sqlx::query_as::<_, CaptainsLog>(
    "SELECT blocked, build, combat, created_at, date, goal, narrative, next, remember, research, skill, updated_at FROM captains_log WHERE date = ?",
  )
  .bind(date)
  .fetch_optional(db.reader())
  .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn dates(db: &Database) -> Result<Vec<String>, Error> {
  let rows = sqlx::query_scalar::<_, String>("SELECT date FROM captains_log ORDER BY date DESC")
    .fetch_all(db.reader())
    .await?;
  Ok(rows)
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
  }
}
