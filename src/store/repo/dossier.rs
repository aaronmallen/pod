use crate::store::{
  Database, Error,
  model::{Dossier, DossierObjectiveOrder, DossierOrder},
};

#[allow(dead_code)]
pub async fn get_brief(db: &Database, character_id: i64) -> Result<Option<Dossier>, Error> {
  let row = sqlx::query_as::<_, Dossier>(
    "SELECT character_id, created_at, near_term, purpose, updated_at FROM character_dossier WHERE character_id = ?",
  )
  .bind(character_id)
  .fetch_optional(db.reader())
  .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn upsert_brief(
  db: &Database,
  character_id: i64,
  purpose: Option<&str>,
  near_term: Option<&str>,
) -> Result<Dossier, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, Dossier>(
    "INSERT INTO character_dossier (character_id, created_at, near_term, purpose, updated_at) \
    VALUES (?, ?, ?, ?, ?) \
    ON CONFLICT(character_id) DO UPDATE SET \
      near_term = excluded.near_term, \
      purpose = excluded.purpose, \
      updated_at = excluded.updated_at \
    RETURNING character_id, created_at, near_term, purpose, updated_at",
  )
  .bind(character_id)
  .bind(&now)
  .bind(near_term)
  .bind(purpose)
  .bind(&now)
  .fetch_one(db.writer())
  .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn list_orders(db: &Database, character_id: i64) -> Result<Vec<DossierOrder>, Error> {
  let rows = sqlx::query_as::<_, DossierOrder>(
    "SELECT character_id, created_at, id, objective_id, position, status, text, updated_at \
    FROM dossier_orders WHERE character_id = ? ORDER BY position, id",
  )
  .bind(character_id)
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn add_order(db: &Database, character_id: i64, text: &str) -> Result<DossierOrder, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, DossierOrder>(
    "INSERT INTO dossier_orders (character_id, created_at, position, status, text, updated_at) \
    VALUES (?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM dossier_orders WHERE character_id = ?), 'active', ?, ?) \
    RETURNING character_id, created_at, id, objective_id, position, status, text, updated_at",
  )
  .bind(character_id)
  .bind(&now)
  .bind(character_id)
  .bind(text)
  .bind(&now)
  .fetch_one(db.writer())
  .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn edit_order(db: &Database, id: i64, text: &str) -> Result<u64, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let result = sqlx::query("UPDATE dossier_orders SET text = ?, updated_at = ? WHERE id = ?")
    .bind(text)
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn remove_order(db: &Database, id: i64) -> Result<u64, Error> {
  let result = sqlx::query("DELETE FROM dossier_orders WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

async fn set_status(db: &Database, id: i64, status: &str) -> Result<u64, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let result = sqlx::query("UPDATE dossier_orders SET status = ?, updated_at = ? WHERE id = ?")
    .bind(status)
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn complete_order(db: &Database, id: i64) -> Result<u64, Error> {
  set_status(db, id, "complete").await
}

#[allow(dead_code)]
pub async fn cancel_order(db: &Database, id: i64) -> Result<u64, Error> {
  set_status(db, id, "cancelled").await
}

#[allow(dead_code)]
pub async fn reopen_order(db: &Database, id: i64) -> Result<u64, Error> {
  set_status(db, id, "active").await
}

#[allow(dead_code)]
pub async fn set_objective(db: &Database, id: i64, objective_id: i64) -> Result<u64, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let result = sqlx::query("UPDATE dossier_orders SET objective_id = ?, updated_at = ? WHERE id = ?")
    .bind(objective_id)
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn clear_objective(db: &Database, id: i64) -> Result<u64, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let result = sqlx::query("UPDATE dossier_orders SET objective_id = NULL, updated_at = ? WHERE id = ?")
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn orders_for_objective(db: &Database, objective_id: i64) -> Result<Vec<DossierObjectiveOrder>, Error> {
  let rows = sqlx::query_as::<_, DossierObjectiveOrder>(
    "SELECT o.character_id AS character_id, c.name AS character_name, o.id AS id, o.status AS status, o.text AS text \
    FROM dossier_orders o \
    JOIN characters c ON c.id = o.character_id \
    WHERE o.objective_id = ? \
    ORDER BY c.name, o.position, o.id",
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
    model::{Alliance, Bloodline, Character, Corporation, Gender, NewObjective, Race},
    repo::{character, objective},
  };

  async fn seed_character(db: &Database, id: i64, name: &str) {
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
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, name);
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn objective(title: &str) -> NewObjective {
    NewObjective {
      accent: "#FF8800".to_owned(),
      horizon: None,
      target: None,
      title: title.to_owned(),
      why: None,
    }
  }

  const PILOT: i64 = 90_000_001;
  const PILOT_TWO: i64 = 90_000_002;

  mod brief {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_empty_before_any_upsert() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot").await;

      assert_eq!(get_brief(&db, PILOT).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_inserts_then_updates_in_place() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot").await;

      let created = upsert_brief(&db, PILOT, Some("Hunt"), Some("Ratting")).await.unwrap();
      assert_eq!(created.character_id, PILOT);
      assert_eq!(created.purpose.as_deref(), Some("Hunt"));
      assert_eq!(created.near_term.as_deref(), Some("Ratting"));

      let updated = upsert_brief(&db, PILOT, Some("Mine"), None).await.unwrap();
      assert_eq!(updated.purpose.as_deref(), Some("Mine"));
      assert_eq!(updated.near_term, None);
      assert_eq!(updated.created_at, created.created_at);

      let fetched = get_brief(&db, PILOT).await.unwrap().unwrap();
      assert_eq!(fetched, updated);
    }
  }

  mod orders {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_adds_lists_edits_and_removes_orders() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot").await;

      let first = add_order(&db, PILOT, "Fit a Loki").await.unwrap();
      let second = add_order(&db, PILOT, "Learn cyno").await.unwrap();
      assert_eq!(first.position, 0);
      assert_eq!(second.position, 1);
      assert_eq!(first.status, "active");

      let listed = list_orders(&db, PILOT).await.unwrap();
      assert_eq!(listed.len(), 2);
      assert_eq!(listed[0].id, first.id);
      assert_eq!(listed[1].id, second.id);

      let affected = edit_order(&db, first.id, "Fit a Legion").await.unwrap();
      assert_eq!(affected, 1);
      let after_edit = list_orders(&db, PILOT).await.unwrap();
      assert_eq!(after_edit[0].text, "Fit a Legion");

      let removed = remove_order(&db, second.id).await.unwrap();
      assert_eq!(removed, 1);
      assert_eq!(list_orders(&db, PILOT).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_transitions_status_across_complete_cancel_and_reopen() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot").await;
      let order = add_order(&db, PILOT, "Roam").await.unwrap();

      complete_order(&db, order.id).await.unwrap();
      assert_eq!(list_orders(&db, PILOT).await.unwrap()[0].status, "complete");

      cancel_order(&db, order.id).await.unwrap();
      assert_eq!(list_orders(&db, PILOT).await.unwrap()[0].status, "cancelled");

      reopen_order(&db, order.id).await.unwrap();
      assert_eq!(list_orders(&db, PILOT).await.unwrap()[0].status, "active");
    }

    #[tokio::test]
    async fn it_sets_and_clears_an_objective_link() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot").await;
      let target = objective::create(&db, &objective("Fund a Nyx")).await.unwrap();
      let order = add_order(&db, PILOT, "Save ISK").await.unwrap();

      set_objective(&db, order.id, target.id).await.unwrap();
      assert_eq!(list_orders(&db, PILOT).await.unwrap()[0].objective_id, Some(target.id));

      clear_objective(&db, order.id).await.unwrap();
      assert_eq!(list_orders(&db, PILOT).await.unwrap()[0].objective_id, None);
    }
  }

  mod cascade {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_drops_brief_and_orders_when_the_character_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot").await;
      upsert_brief(&db, PILOT, Some("Hunt"), None).await.unwrap();
      add_order(&db, PILOT, "Roam").await.unwrap();

      character::delete(&db, PILOT).await.unwrap();

      assert_eq!(get_brief(&db, PILOT).await.unwrap(), None);
      assert!(list_orders(&db, PILOT).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_nulls_the_link_but_keeps_the_order_when_the_objective_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Pilot").await;
      let target = objective::create(&db, &objective("Fund a Nyx")).await.unwrap();
      let order = add_order(&db, PILOT, "Save ISK").await.unwrap();
      set_objective(&db, order.id, target.id).await.unwrap();

      objective::delete(&db, target.id).await.unwrap();

      let remaining = list_orders(&db, PILOT).await.unwrap();
      assert_eq!(remaining.len(), 1);
      assert_eq!(remaining[0].id, order.id);
      assert_eq!(remaining[0].objective_id, None);
    }
  }

  mod for_objective {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_linked_orders_across_the_roster_with_owning_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT, "Alpha").await;
      seed_character(&db, PILOT_TWO, "Bravo").await;
      let target = objective::create(&db, &objective("Fund a Nyx")).await.unwrap();
      let other = objective::create(&db, &objective("Other")).await.unwrap();

      let alpha = add_order(&db, PILOT, "Alpha saves").await.unwrap();
      let bravo = add_order(&db, PILOT_TWO, "Bravo saves").await.unwrap();
      let unrelated = add_order(&db, PILOT, "Alpha other").await.unwrap();
      set_objective(&db, alpha.id, target.id).await.unwrap();
      set_objective(&db, bravo.id, target.id).await.unwrap();
      set_objective(&db, unrelated.id, other.id).await.unwrap();

      let linked = orders_for_objective(&db, target.id).await.unwrap();

      assert_eq!(linked.len(), 2);
      assert_eq!(linked[0].character_name, "Alpha");
      assert_eq!(linked[0].character_id, PILOT);
      assert_eq!(linked[0].text, "Alpha saves");
      assert_eq!(linked[1].character_name, "Bravo");
      assert_eq!(linked[1].text, "Bravo saves");
    }

    #[tokio::test]
    async fn it_is_empty_for_an_objective_with_no_orders() {
      let db = store::open_test().await.unwrap();
      let target = objective::create(&db, &objective("Lonely")).await.unwrap();

      assert!(orders_for_objective(&db, target.id).await.unwrap().is_empty());
    }
  }
}
