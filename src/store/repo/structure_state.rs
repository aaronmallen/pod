use crate::store::{Database, Error, model::StructureState};

#[allow(dead_code)]
pub async fn read(db: &Database, structure_id: i64) -> Result<Option<StructureState>, Error> {
  let row = sqlx::query_as::<_, StructureState>("SELECT * FROM structure_state WHERE structure_id = ?")
    .bind(structure_id)
    .fetch_optional(db.reader())
    .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn upsert(db: &Database, state: &StructureState) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO structure_state \
      (structure_id, fuel_expires, state, services, reinforce_hour, next_reinforce_apply, \
       next_reinforce_hour, next_reinforce_weekday, state_timer_start, state_timer_end, unanchors_at, synced_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(structure_id) DO UPDATE SET \
      fuel_expires           = excluded.fuel_expires, \
      state                  = excluded.state, \
      services               = excluded.services, \
      reinforce_hour         = excluded.reinforce_hour, \
      next_reinforce_apply   = excluded.next_reinforce_apply, \
      next_reinforce_hour    = excluded.next_reinforce_hour, \
      next_reinforce_weekday = excluded.next_reinforce_weekday, \
      state_timer_start      = excluded.state_timer_start, \
      state_timer_end        = excluded.state_timer_end, \
      unanchors_at           = excluded.unanchors_at, \
      synced_at              = excluded.synced_at",
  )
  .bind(state.structure_id)
  .bind(&state.fuel_expires)
  .bind(&state.state)
  .bind(&state.services)
  .bind(state.reinforce_hour)
  .bind(&state.next_reinforce_apply)
  .bind(state.next_reinforce_hour)
  .bind(state.next_reinforce_weekday)
  .bind(&state.state_timer_start)
  .bind(&state.state_timer_end)
  .bind(&state.unanchors_at)
  .bind(&state.synced_at)
  .execute(db.writer())
  .await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{self, model::StructureService};

  const STRUCTURE: i64 = 1_030_000_000_001;

  async fn seed_structure(db: &Database, id: i64) {
    sqlx::query("INSERT INTO regions (id, name) VALUES (10000001, 'Region')")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query(
      "INSERT INTO constellations (id, region_id, name, position_x, position_y, position_z) \
      VALUES (20000001, 10000001, 'Constellation', 0, 0, 0)",
    )
    .execute(db.writer())
    .await
    .unwrap();
    sqlx::query(
      "INSERT INTO solar_systems (id, constellation_id, name, position_x, position_y, position_z, security_status) \
      VALUES (30000001, 20000001, 'System', 0, 0, 0, 0.5)",
    )
    .execute(db.writer())
    .await
    .unwrap();
    sqlx::query(
      "INSERT INTO corporations (id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
      VALUES (98000001, 1, 1, 1, 'Owner Corp', 0.0, 'OWN')",
    )
    .execute(db.writer())
    .await
    .unwrap();
    sqlx::query(
      "INSERT INTO structures (id, solar_system_id, owner_id, name) \
      VALUES (?, 30000001, 98000001, 'Test Citadel')",
    )
    .bind(id)
    .execute(db.writer())
    .await
    .unwrap();
  }

  fn make_state(id: i64) -> StructureState {
    let mut state = StructureState::new(id, "2026-07-15T00:00:00Z");
    state.fuel_expires = Some("2026-07-20T00:00:00Z".to_owned());
    state.state = Some("shield_vulnerable".to_owned());
    state.reinforce_hour = Some(18);
    state.state_timer_start = Some("2026-07-18T18:00:00Z".to_owned());
    state.state_timer_end = Some("2026-07-19T18:00:00Z".to_owned());
    state.unanchors_at = Some("2026-08-01T00:00:00Z".to_owned());
    state.set_service_list(&[StructureService {
      name: "Clone Bay".to_owned(),
      state: "online".to_owned(),
    }]);
    state
  }

  mod read {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_before_any_upsert() {
      let db = store::open_test().await.unwrap();
      seed_structure(&db, STRUCTURE).await;

      assert_eq!(read(&db, STRUCTURE).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_returns_the_persisted_state() {
      let db = store::open_test().await.unwrap();
      seed_structure(&db, STRUCTURE).await;
      upsert(&db, &make_state(STRUCTURE)).await.unwrap();

      let state = read(&db, STRUCTURE).await.unwrap().unwrap();

      assert_eq!(state.structure_id, STRUCTURE);
      assert_eq!(state.state.as_deref(), Some("shield_vulnerable"));
      assert_eq!(state.reinforce_hour, Some(18));
      assert_eq!(state.unanchors_at.as_deref(), Some("2026-08-01T00:00:00Z"));
      assert_eq!(state.service_list()[0].name, "Clone Bay");
    }
  }

  mod upsert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_overwrites_an_existing_row_on_conflict() {
      let db = store::open_test().await.unwrap();
      seed_structure(&db, STRUCTURE).await;
      upsert(&db, &make_state(STRUCTURE)).await.unwrap();

      let mut updated = StructureState::new(STRUCTURE, "2026-07-16T00:00:00Z");
      updated.state = Some("online".to_owned());
      upsert(&db, &updated).await.unwrap();

      let state = read(&db, STRUCTURE).await.unwrap().unwrap();

      assert_eq!(state.state.as_deref(), Some("online"));
      assert_eq!(state.synced_at, "2026-07-16T00:00:00Z");
      assert_eq!(state.fuel_expires, None);
      assert_eq!(state.service_list(), Vec::new());
    }

    #[tokio::test]
    async fn it_is_dropped_when_the_structure_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_structure(&db, STRUCTURE).await;
      upsert(&db, &make_state(STRUCTURE)).await.unwrap();

      sqlx::query("DELETE FROM structures WHERE id = ?")
        .bind(STRUCTURE)
        .execute(db.writer())
        .await
        .unwrap();

      assert_eq!(read(&db, STRUCTURE).await.unwrap(), None);
    }
  }
}
