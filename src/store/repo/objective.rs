use crate::store::{
  Database, Error,
  model::{LinkSource, NewObjective, Objective, ObjectiveLink, ObjectiveStatus, ObjectiveThreadEntry},
};

#[allow(dead_code)]
pub async fn list(db: &Database, status: Option<ObjectiveStatus>) -> Result<Vec<Objective>, Error> {
  let rows = match status {
    Some(status) => {
      sqlx::query_as::<_, Objective>(
        "SELECT accent, cancelled_at, completed_at, created_at, horizon, id, status, target, title, why \
        FROM objectives WHERE status = ? ORDER BY created_at DESC, id DESC",
      )
      .bind(status.as_str())
      .fetch_all(db.reader())
      .await?
    }
    None => {
      sqlx::query_as::<_, Objective>(
        "SELECT accent, cancelled_at, completed_at, created_at, horizon, id, status, target, title, why \
        FROM objectives ORDER BY created_at DESC, id DESC",
      )
      .fetch_all(db.reader())
      .await?
    }
  };
  Ok(rows)
}

#[allow(dead_code)]
pub async fn get(db: &Database, id: i64) -> Result<Option<Objective>, Error> {
  let row = sqlx::query_as::<_, Objective>(
    "SELECT accent, cancelled_at, completed_at, created_at, horizon, id, status, target, title, why \
    FROM objectives WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(db.reader())
  .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn create(db: &Database, input: &NewObjective) -> Result<Objective, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, Objective>(
    "INSERT INTO objectives (accent, created_at, horizon, status, target, title, why) \
    VALUES (?, ?, ?, 'active', ?, ?, ?) \
    RETURNING accent, cancelled_at, completed_at, created_at, horizon, id, status, target, title, why",
  )
  .bind(&input.accent)
  .bind(&now)
  .bind(&input.horizon)
  .bind(&input.target)
  .bind(&input.title)
  .bind(&input.why)
  .fetch_one(db.writer())
  .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn update(db: &Database, id: i64, input: &NewObjective) -> Result<u64, Error> {
  let result =
    sqlx::query("UPDATE objectives SET accent = ?, horizon = ?, target = ?, title = ?, why = ? WHERE id = ?")
      .bind(&input.accent)
      .bind(&input.horizon)
      .bind(&input.target)
      .bind(&input.title)
      .bind(&input.why)
      .bind(id)
      .execute(db.writer())
      .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn complete(db: &Database, id: i64) -> Result<u64, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let result =
    sqlx::query("UPDATE objectives SET status = 'complete', completed_at = ?, cancelled_at = NULL WHERE id = ?")
      .bind(&now)
      .bind(id)
      .execute(db.writer())
      .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn cancel(db: &Database, id: i64) -> Result<u64, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let result =
    sqlx::query("UPDATE objectives SET status = 'cancelled', cancelled_at = ?, completed_at = NULL WHERE id = ?")
      .bind(&now)
      .bind(id)
      .execute(db.writer())
      .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn reopen(db: &Database, id: i64) -> Result<u64, Error> {
  let result =
    sqlx::query("UPDATE objectives SET status = 'active', completed_at = NULL, cancelled_at = NULL WHERE id = ?")
      .bind(id)
      .execute(db.writer())
      .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn delete(db: &Database, id: i64) -> Result<u64, Error> {
  let result = sqlx::query("DELETE FROM objectives WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn assign_pilot(db: &Database, objective_id: i64, character_id: i64) -> Result<(), Error> {
  sqlx::query("INSERT OR IGNORE INTO objective_pilots (objective_id, character_id) VALUES (?, ?)")
    .bind(objective_id)
    .bind(character_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

#[allow(dead_code)]
pub async fn unassign_pilot(db: &Database, objective_id: i64, character_id: i64) -> Result<u64, Error> {
  let result = sqlx::query("DELETE FROM objective_pilots WHERE objective_id = ? AND character_id = ?")
    .bind(objective_id)
    .bind(character_id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn pilots(db: &Database, objective_id: i64) -> Result<Vec<i64>, Error> {
  let rows = sqlx::query_scalar::<_, i64>(
    "SELECT character_id FROM objective_pilots WHERE objective_id = ? ORDER BY character_id",
  )
  .bind(objective_id)
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn set_link(db: &Database, objective_id: i64, date: &str, source: &LinkSource) -> Result<(), Error> {
  sqlx::query(
    "INSERT OR IGNORE INTO objective_links (objective_id, date, source_kind, source_ref) VALUES (?, ?, ?, ?)",
  )
  .bind(objective_id)
  .bind(date)
  .bind(source.source_kind())
  .bind(source.source_ref())
  .execute(db.writer())
  .await?;
  Ok(())
}

#[allow(dead_code)]
pub async fn clear_link(db: &Database, objective_id: i64, date: &str, source: &LinkSource) -> Result<u64, Error> {
  let result = sqlx::query(
    "DELETE FROM objective_links WHERE objective_id = ? AND date = ? AND source_kind = ? AND source_ref = ?",
  )
  .bind(objective_id)
  .bind(date)
  .bind(source.source_kind())
  .bind(source.source_ref())
  .execute(db.writer())
  .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn links_for_day(db: &Database, date: &str) -> Result<Vec<ObjectiveLink>, Error> {
  let rows = sqlx::query_as::<_, ObjectiveLink>(
    "SELECT date, objective_id, source_kind, source_ref FROM objective_links \
    WHERE date = ? ORDER BY objective_id, source_kind, source_ref",
  )
  .bind(date)
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn links_for_objective(db: &Database, objective_id: i64) -> Result<Vec<ObjectiveLink>, Error> {
  let rows = sqlx::query_as::<_, ObjectiveLink>(
    "SELECT date, objective_id, source_kind, source_ref FROM objective_links \
    WHERE objective_id = ? ORDER BY date DESC, source_kind, source_ref",
  )
  .bind(objective_id)
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn thread(db: &Database, objective_id: i64) -> Result<Vec<ObjectiveThreadEntry>, Error> {
  let rows = sqlx::query_as::<_, ObjectiveThreadEntry>(
    "SELECT \
      l.date AS date, \
      l.source_kind AS source_kind, \
      l.source_ref AS source_ref, \
      CASE l.source_kind \
        WHEN 'log_answer' THEN ( \
          SELECT a.value FROM captains_log_answer a WHERE a.date = l.date AND a.question_id = l.source_ref \
        ) \
        WHEN 'field_note' THEN ( \
          SELECT n.text FROM field_notes n WHERE n.id = CAST(l.source_ref AS INTEGER) \
        ) \
        WHEN 'killmail' THEN ( \
          SELECT CAST(k.killmail_id AS TEXT) FROM character_killmails k \
          WHERE k.character_id || ':' || k.killmail_id = l.source_ref LIMIT 1 \
        ) \
        WHEN 'industry' THEN ( \
          SELECT COALESCE(it.name, CAST(ic.product_type_id AS TEXT)) FROM industry_completion ic \
          LEFT JOIN item_types it ON it.id = ic.product_type_id \
          WHERE ic.character_id || ':' || ic.product_type_id = l.source_ref LIMIT 1 \
        ) \
        WHEN 'skill' THEN ( \
          SELECT COALESCE(it.name, CAST(s.skill_id AS TEXT)) FROM skill_completion s \
          LEFT JOIN item_types it ON it.id = s.skill_id \
          WHERE s.character_id || ':' || s.skill_id = l.source_ref LIMIT 1 \
        ) \
      END AS text, \
      ( \
        SELECT c.name FROM characters c \
        WHERE l.source_kind IN ('skill', 'industry', 'killmail') \
          AND c.id = CAST(substr(l.source_ref, 1, instr(l.source_ref, ':') - 1) AS INTEGER) \
      ) AS character \
    FROM objective_links l \
    WHERE l.objective_id = ? \
    ORDER BY l.date DESC, l.source_kind, l.source_ref",
  )
  .bind(objective_id)
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
    repo::{character, infra},
  };

  const PILOT: i64 = 90_000_001;

  fn objective(title: &str) -> NewObjective {
    NewObjective {
      accent: "#FF8800".to_owned(),
      horizon: Some("Q3".to_owned()),
      target: Some("10 kills".to_owned()),
      title: title.to_owned(),
      why: Some("stay sharp".to_owned()),
    }
  }

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 98_000_001;
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

  async fn seed_owned(db: &Database, id: i64) {
    seed_character(db, id).await;
    infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
      .await
      .unwrap();
  }

  mod crud {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_and_reads_back_an_active_objective() {
      let db = store::open_test().await.unwrap();

      let created = create(&db, &objective("Fund a Nyx")).await.unwrap();

      assert!(created.id > 0);
      assert_eq!(created.status, "active");
      assert_eq!(created.title, "Fund a Nyx");
      assert_eq!(created.accent, "#FF8800");
      assert_eq!(created.why.as_deref(), Some("stay sharp"));
      assert!(created.completed_at.is_none());
      assert!(created.cancelled_at.is_none());

      let fetched = get(&db, created.id).await.unwrap().unwrap();
      assert_eq!(fetched, created);
    }

    #[tokio::test]
    async fn it_updates_the_editable_fields_without_touching_status() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, &objective("Draft")).await.unwrap();

      let mut edit = objective("Renamed");
      edit.accent = "#00C2FF".to_owned();
      edit.why = None;
      let affected = update(&db, created.id, &edit).await.unwrap();

      assert_eq!(affected, 1);
      let fetched = get(&db, created.id).await.unwrap().unwrap();
      assert_eq!(fetched.title, "Renamed");
      assert_eq!(fetched.accent, "#00C2FF");
      assert_eq!(fetched.why, None);
      assert_eq!(fetched.status, "active");
    }

    #[tokio::test]
    async fn it_lists_newest_first_and_filters_by_status() {
      let db = store::open_test().await.unwrap();
      let first = create(&db, &objective("One")).await.unwrap();
      let second = create(&db, &objective("Two")).await.unwrap();
      complete(&db, first.id).await.unwrap();

      let all = list(&db, None).await.unwrap();
      assert_eq!(all.len(), 2);
      assert_eq!(all[0].id, second.id);

      let active = list(&db, Some(ObjectiveStatus::Active)).await.unwrap();
      assert_eq!(active.len(), 1);
      assert_eq!(active[0].id, second.id);

      let complete = list(&db, Some(ObjectiveStatus::Complete)).await.unwrap();
      assert_eq!(complete.len(), 1);
      assert_eq!(complete[0].id, first.id);
    }

    #[tokio::test]
    async fn it_deletes_an_objective() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, &objective("Scrap")).await.unwrap();

      let affected = delete(&db, created.id).await.unwrap();

      assert_eq!(affected, 1);
      assert_eq!(get(&db, created.id).await.unwrap(), None);
    }
  }

  mod status_transitions {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_stamps_completed_at_on_complete() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, &objective("Ship it")).await.unwrap();

      complete(&db, created.id).await.unwrap();

      let row = get(&db, created.id).await.unwrap().unwrap();
      assert_eq!(row.status, "complete");
      assert!(row.completed_at.is_some());
      assert!(row.cancelled_at.is_none());
    }

    #[tokio::test]
    async fn it_stamps_cancelled_at_on_cancel() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, &objective("Abandon")).await.unwrap();

      cancel(&db, created.id).await.unwrap();

      let row = get(&db, created.id).await.unwrap().unwrap();
      assert_eq!(row.status, "cancelled");
      assert!(row.cancelled_at.is_some());
      assert!(row.completed_at.is_none());
    }

    #[tokio::test]
    async fn it_clears_both_timestamps_on_reopen() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, &objective("Revive")).await.unwrap();
      complete(&db, created.id).await.unwrap();

      reopen(&db, created.id).await.unwrap();

      let row = get(&db, created.id).await.unwrap().unwrap();
      assert_eq!(row.status, "active");
      assert!(row.completed_at.is_none());
      assert!(row.cancelled_at.is_none());
    }

    #[tokio::test]
    async fn it_clears_the_cancel_stamp_when_completing_a_cancelled_objective() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, &objective("Flip")).await.unwrap();
      cancel(&db, created.id).await.unwrap();

      complete(&db, created.id).await.unwrap();

      let row = get(&db, created.id).await.unwrap().unwrap();
      assert_eq!(row.status, "complete");
      assert!(row.completed_at.is_some());
      assert!(row.cancelled_at.is_none());
    }
  }

  mod pilots {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_assigns_reads_and_unassigns_a_pilot() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT).await;
      let created = create(&db, &objective("Roam")).await.unwrap();

      assign_pilot(&db, created.id, PILOT).await.unwrap();
      assign_pilot(&db, created.id, PILOT).await.unwrap();

      assert_eq!(pilots(&db, created.id).await.unwrap(), vec![PILOT]);

      let removed = unassign_pilot(&db, created.id, PILOT).await.unwrap();
      assert_eq!(removed, 1);
      assert!(pilots(&db, created.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_drops_assignments_when_the_character_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT).await;
      let created = create(&db, &objective("Roam")).await.unwrap();
      assign_pilot(&db, created.id, PILOT).await.unwrap();

      character::delete(&db, PILOT).await.unwrap();

      assert!(pilots(&db, created.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_drops_assignments_when_the_objective_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, PILOT).await;
      let created = create(&db, &objective("Roam")).await.unwrap();
      assign_pilot(&db, created.id, PILOT).await.unwrap();

      delete(&db, created.id).await.unwrap();

      assert!(pilots(&db, created.id).await.unwrap().is_empty());
    }
  }

  mod links {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_sets_reads_and_clears_a_day_link() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, &objective("Log it")).await.unwrap();
      let source = LinkSource::LogAnswer {
        question_id: "goal".to_owned(),
      };

      set_link(&db, created.id, "2026-07-05", &source).await.unwrap();
      set_link(&db, created.id, "2026-07-05", &source).await.unwrap();

      let links = links_for_objective(&db, created.id).await.unwrap();
      assert_eq!(links.len(), 1);
      assert_eq!(links[0].date, "2026-07-05");
      assert_eq!(links[0].source_kind, "log_answer");
      assert_eq!(links[0].source_ref, "goal");

      let cleared = clear_link(&db, created.id, "2026-07-05", &source).await.unwrap();
      assert_eq!(cleared, 1);
      assert!(links_for_objective(&db, created.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_reads_every_link_on_a_given_day_across_objectives() {
      let db = store::open_test().await.unwrap();
      let first = create(&db, &objective("First")).await.unwrap();
      let second = create(&db, &objective("Second")).await.unwrap();
      set_link(
        &db,
        first.id,
        "2026-07-05",
        &LinkSource::LogAnswer {
          question_id: "goal".to_owned(),
        },
      )
      .await
      .unwrap();
      set_link(
        &db,
        second.id,
        "2026-07-05",
        &LinkSource::FieldNote {
          note_id: 7,
        },
      )
      .await
      .unwrap();
      set_link(
        &db,
        first.id,
        "2026-07-04",
        &LinkSource::FieldNote {
          note_id: 9,
        },
      )
      .await
      .unwrap();

      let day = links_for_day(&db, "2026-07-05").await.unwrap();

      assert_eq!(day.len(), 2);
      assert_eq!(day[0].objective_id, first.id);
      assert_eq!(day[1].objective_id, second.id);
    }

    #[tokio::test]
    async fn it_drops_links_when_the_objective_is_deleted() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, &objective("Doomed")).await.unwrap();
      set_link(
        &db,
        created.id,
        "2026-07-05",
        &LinkSource::FieldNote {
          note_id: 1,
        },
      )
      .await
      .unwrap();

      delete(&db, created.id).await.unwrap();

      assert!(links_for_day(&db, "2026-07-05").await.unwrap().is_empty());
    }
  }

  mod thread {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::repo::{captains_log, field_notes};

    #[tokio::test]
    async fn it_resolves_log_answer_and_field_note_text_newest_day_first() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, &objective("Thread")).await.unwrap();

      captains_log::upsert_answer(
        &db,
        "2026-07-04",
        captains_log::AnswerKey::Goal,
        Some("Undock the barge."),
      )
      .await
      .unwrap();
      let note = field_notes::insert(&db, "2026-07-05", "Cyno up in Tama").await.unwrap();

      set_link(
        &db,
        created.id,
        "2026-07-04",
        &LinkSource::LogAnswer {
          question_id: "goal".to_owned(),
        },
      )
      .await
      .unwrap();
      set_link(
        &db,
        created.id,
        "2026-07-05",
        &LinkSource::FieldNote {
          note_id: note.id,
        },
      )
      .await
      .unwrap();

      let thread = thread(&db, created.id).await.unwrap();

      assert_eq!(thread.len(), 2);
      assert_eq!(thread[0].date, "2026-07-05");
      assert_eq!(thread[0].source_kind, "field_note");
      assert_eq!(thread[0].text.as_deref(), Some("Cyno up in Tama"));
      assert_eq!(thread[1].date, "2026-07-04");
      assert_eq!(thread[1].source_kind, "log_answer");
      assert_eq!(thread[1].text.as_deref(), Some("Undock the barge."));
    }

    #[tokio::test]
    async fn it_survives_a_prompt_question_rename_because_links_key_on_identity() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, &objective("Durable")).await.unwrap();
      captains_log::upsert_answer(&db, "2026-07-04", "custom_mood", Some("focused"))
        .await
        .unwrap();
      set_link(
        &db,
        created.id,
        "2026-07-04",
        &LinkSource::LogAnswer {
          question_id: "custom_mood".to_owned(),
        },
      )
      .await
      .unwrap();

      let thread = thread(&db, created.id).await.unwrap();

      assert_eq!(thread.len(), 1);
      assert_eq!(thread[0].text.as_deref(), Some("focused"));
    }

    #[tokio::test]
    async fn it_returns_a_null_text_entry_when_the_underlying_answer_is_gone() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, &objective("Orphaned")).await.unwrap();
      set_link(
        &db,
        created.id,
        "2026-07-04",
        &LinkSource::LogAnswer {
          question_id: "goal".to_owned(),
        },
      )
      .await
      .unwrap();

      let thread = thread(&db, created.id).await.unwrap();

      assert_eq!(thread.len(), 1);
      assert_eq!(thread[0].source_ref, "goal");
      assert_eq!(thread[0].text, None);
    }
  }
}
