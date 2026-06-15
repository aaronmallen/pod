use std::collections::HashSet;

use sqlx::{QueryBuilder, Sqlite};

use crate::store::{
  Database, Error,
  model::{CharacterBlueprint, CorporationBlueprint},
  repo::org,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AllBlueprints {
  pub character_blueprints: Vec<CharacterBlueprint>,
  pub corporation_blueprints: Vec<CorporationBlueprint>,
}

const BLUEPRINT_WRITE_BATCH_SIZE: usize = 500;

pub async fn activity_meta(
  db: &Database,
  blueprint_type_id: i64,
  activity_id: i64,
) -> Result<Option<(i64, i64)>, Error> {
  let row: Option<(i64, i64)> = sqlx::query_as(
    "SELECT time, max_production_limit FROM blueprint_activity_meta \
    WHERE blueprint_type_id = ? AND activity_id = ?",
  )
  .bind(blueprint_type_id)
  .bind(activity_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn list_all(db: &Database) -> Result<AllBlueprints, Error> {
  let character_blueprints = list_all_character(db).await?;
  let corporation_blueprints = list_all_corporation(db).await?;
  Ok(AllBlueprints {
    character_blueprints,
    corporation_blueprints,
  })
}

pub async fn list_for_character(db: &Database, character_id: i64) -> Result<Vec<CharacterBlueprint>, Error> {
  let rows = sqlx::query_as::<_, CharacterBlueprint>(
    "SELECT character_id, item_id, location_flag, location_id, material_efficiency, quantity, runs, time_efficiency, \
    type_id FROM character_blueprints WHERE character_id = ? ORDER BY type_id, item_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn list_for_corporation(db: &Database, corporation_id: i64) -> Result<Vec<CorporationBlueprint>, Error> {
  if !org::corp_is_authorized(db, corporation_id).await? {
    return Ok(Vec::new());
  }
  let rows = sqlx::query_as::<_, CorporationBlueprint>(
    "SELECT corporation_id, item_id, location_flag, location_id, material_efficiency, quantity, runs, \
    time_efficiency, type_id FROM corporation_blueprints WHERE corporation_id = ? ORDER BY type_id, item_id",
  )
  .bind(corporation_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn replace_for_character(
  db: &Database,
  character_id: i64,
  blueprints: &[CharacterBlueprint],
) -> Result<(), Error> {
  replace_for_character_batched(db, character_id, blueprints, BLUEPRINT_WRITE_BATCH_SIZE).await
}

pub async fn replace_for_corporation(
  db: &Database,
  corporation_id: i64,
  blueprints: &[CorporationBlueprint],
) -> Result<(), Error> {
  replace_for_corporation_batched(db, corporation_id, blueprints, BLUEPRINT_WRITE_BATCH_SIZE).await
}

async fn delete_character_blueprints(db: &Database, character_id: i64, item_ids: &[i64]) -> Result<(), Error> {
  if item_ids.is_empty() {
    return Ok(());
  }
  let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM character_blueprints WHERE character_id = ");
  builder.push_bind(character_id);
  builder.push(" AND item_id IN (");
  let mut separated = builder.separated(", ");
  for id in item_ids {
    separated.push_bind(*id);
  }
  builder.push(")");
  builder.build().execute(&db.0).await?;
  Ok(())
}

async fn delete_corporation_blueprints(db: &Database, corporation_id: i64, item_ids: &[i64]) -> Result<(), Error> {
  if item_ids.is_empty() {
    return Ok(());
  }
  let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM corporation_blueprints WHERE corporation_id = ");
  builder.push_bind(corporation_id);
  builder.push(" AND item_id IN (");
  let mut separated = builder.separated(", ");
  for id in item_ids {
    separated.push_bind(*id);
  }
  builder.push(")");
  builder.build().execute(&db.0).await?;
  Ok(())
}

async fn insert_character_blueprint(
  tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
  blueprint: &CharacterBlueprint,
) -> Result<(), Error> {
  sqlx::query(
    "INSERT OR REPLACE INTO character_blueprints \
      (item_id, character_id, type_id, location_id, location_flag, quantity, material_efficiency, time_efficiency, \
      runs) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(blueprint.item_id())
  .bind(blueprint.character_id())
  .bind(blueprint.type_id())
  .bind(blueprint.location_id())
  .bind(blueprint.location_flag())
  .bind(blueprint.quantity())
  .bind(blueprint.material_efficiency())
  .bind(blueprint.time_efficiency())
  .bind(blueprint.runs())
  .execute(&mut **tx)
  .await?;
  Ok(())
}

async fn insert_corporation_blueprint(
  tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
  blueprint: &CorporationBlueprint,
) -> Result<(), Error> {
  sqlx::query(
    "INSERT OR REPLACE INTO corporation_blueprints \
      (item_id, corporation_id, type_id, location_id, location_flag, quantity, material_efficiency, time_efficiency, \
      runs) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(blueprint.item_id())
  .bind(blueprint.corporation_id())
  .bind(blueprint.type_id())
  .bind(blueprint.location_id())
  .bind(blueprint.location_flag())
  .bind(blueprint.quantity())
  .bind(blueprint.material_efficiency())
  .bind(blueprint.time_efficiency())
  .bind(blueprint.runs())
  .execute(&mut **tx)
  .await?;
  Ok(())
}

async fn list_all_character(db: &Database) -> Result<Vec<CharacterBlueprint>, Error> {
  let rows = sqlx::query_as::<_, CharacterBlueprint>(
    "SELECT character_id, item_id, location_flag, location_id, material_efficiency, quantity, runs, time_efficiency, \
    type_id FROM character_blueprints ORDER BY type_id, item_id",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

async fn list_all_corporation(db: &Database) -> Result<Vec<CorporationBlueprint>, Error> {
  let rows = sqlx::query_as::<_, CorporationBlueprint>(
    "SELECT corporation_id, item_id, location_flag, location_id, material_efficiency, quantity, runs, \
    time_efficiency, type_id FROM corporation_blueprints ORDER BY type_id, item_id",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

/// Reconciles a character's owned blueprints to `blueprints`, committing in batches rather than one atomic transaction.
///
/// Upserting the new set before pruning stale ids (instead of deleting all first) and committing each batch releases
/// SQLite's single write lock between batches so interactive writes can interleave. A concurrent reader may transiently
/// observe a superset (a stale row not yet pruned) but never a missing current row; the final state is identical to a
/// delete-all-then-insert-all replace.
async fn replace_for_character_batched(
  db: &Database,
  character_id: i64,
  blueprints: &[CharacterBlueprint],
  batch_size: usize,
) -> Result<(), Error> {
  let new_ids: HashSet<i64> = blueprints.iter().map(CharacterBlueprint::item_id).collect();
  let existing: Vec<i64> = sqlx::query_scalar("SELECT item_id FROM character_blueprints WHERE character_id = ?")
    .bind(character_id)
    .fetch_all(&db.0)
    .await?;
  let stale: Vec<i64> = existing.into_iter().filter(|id| !new_ids.contains(id)).collect();

  let batch_size = batch_size.max(1);
  for chunk in blueprints.chunks(batch_size) {
    let mut tx = db.0.begin().await?;
    for blueprint in chunk {
      insert_character_blueprint(&mut tx, blueprint).await?;
    }
    tx.commit().await?;
    tokio::task::yield_now().await;
  }
  for chunk in stale.chunks(batch_size) {
    delete_character_blueprints(db, character_id, chunk).await?;
    tokio::task::yield_now().await;
  }
  Ok(())
}

async fn replace_for_corporation_batched(
  db: &Database,
  corporation_id: i64,
  blueprints: &[CorporationBlueprint],
  batch_size: usize,
) -> Result<(), Error> {
  let new_ids: HashSet<i64> = blueprints.iter().map(CorporationBlueprint::item_id).collect();
  let existing: Vec<i64> = sqlx::query_scalar("SELECT item_id FROM corporation_blueprints WHERE corporation_id = ?")
    .bind(corporation_id)
    .fetch_all(&db.0)
    .await?;
  let stale: Vec<i64> = existing.into_iter().filter(|id| !new_ids.contains(id)).collect();

  let batch_size = batch_size.max(1);
  for chunk in blueprints.chunks(batch_size) {
    let mut tx = db.0.begin().await?;
    for blueprint in chunk {
      insert_corporation_blueprint(&mut tx, blueprint).await?;
    }
    tx.commit().await?;
    tokio::task::yield_now().await;
  }
  for chunk in stale.chunks(batch_size) {
    delete_corporation_blueprints(db, corporation_id, chunk).await?;
    tokio::task::yield_now().await;
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{Alliance, Bloodline, Character, Corporation, CorporationMemberRole, Gender, OwnerType, Race},
    repo::{character, infra},
  };

  const CHARACTER_ID: i64 = 42;
  const CORPORATION_ID: i64 = 90_000_001;
  const DIRECTOR_ID: i64 = 100;

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = CORPORATION_ID;
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

  async fn authorize_corporation(db: &Database) {
    infra::upsert(
      db,
      CORPORATION_ID,
      OwnerType::Corporation,
      "tok",
      "rt",
      4_102_444_800,
      Some(DIRECTOR_ID),
      None,
    )
    .await
    .unwrap();
    org::replace_for_corporation(
      db,
      CORPORATION_ID,
      &[CorporationMemberRole::from((
        CORPORATION_ID,
        DIRECTOR_ID,
        "Director".to_owned(),
      ))],
    )
    .await
    .unwrap();
  }

  fn character_blueprint(character_id: i64, item_id: i64, type_id: i64) -> CharacterBlueprint {
    CharacterBlueprint {
      character_id,
      item_id,
      location_flag: "Hangar".to_owned(),
      location_id: 60_003_760,
      material_efficiency: 10,
      quantity: -1,
      runs: -1,
      time_efficiency: 20,
      type_id,
    }
  }

  fn corporation_blueprint(corporation_id: i64, item_id: i64, type_id: i64) -> CorporationBlueprint {
    CorporationBlueprint {
      corporation_id,
      item_id,
      location_flag: "CorpSAG1".to_owned(),
      location_id: 60_003_760,
      material_efficiency: 5,
      quantity: -2,
      runs: 30,
      time_efficiency: 14,
      type_id,
    }
  }

  mod replace_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_the_full_blueprint_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;

      super::replace_for_character(
        &db,
        CHARACTER_ID,
        &[
          character_blueprint(CHARACTER_ID, 1, 1_000),
          character_blueprint(CHARACTER_ID, 2, 1_001),
        ],
      )
      .await
      .unwrap();

      let blueprints = super::list_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(
        blueprints.iter().map(CharacterBlueprint::item_id).collect::<Vec<_>>(),
        [1, 2]
      );
    }

    #[tokio::test]
    async fn it_prunes_stale_blueprints_on_re_sync() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      super::replace_for_character(
        &db,
        CHARACTER_ID,
        &[
          character_blueprint(CHARACTER_ID, 1, 1_000),
          character_blueprint(CHARACTER_ID, 2, 1_001),
        ],
      )
      .await
      .unwrap();

      super::replace_for_character(&db, CHARACTER_ID, &[character_blueprint(CHARACTER_ID, 2, 1_001)])
        .await
        .unwrap();

      let blueprints = super::list_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(
        blueprints.iter().map(CharacterBlueprint::item_id).collect::<Vec<_>>(),
        [2]
      );
    }

    #[tokio::test]
    async fn it_round_trips_every_field() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      let blueprint = character_blueprint(CHARACTER_ID, 7, 2_048);

      super::replace_for_character(&db, CHARACTER_ID, std::slice::from_ref(&blueprint))
        .await
        .unwrap();

      let blueprints = super::list_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(blueprints, vec![blueprint]);
    }

    #[tokio::test]
    async fn it_cascades_when_the_character_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      super::replace_for_character(&db, CHARACTER_ID, &[character_blueprint(CHARACTER_ID, 1, 1_000)])
        .await
        .unwrap();

      sqlx::query("DELETE FROM characters WHERE id = ?")
        .bind(CHARACTER_ID)
        .execute(&db.0)
        .await
        .unwrap();

      assert!(super::list_for_character(&db, CHARACTER_ID).await.unwrap().is_empty());
    }
  }

  mod replace_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_then_prunes_for_an_authorized_corp() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corporation(&db).await;

      super::replace_for_corporation(
        &db,
        CORPORATION_ID,
        &[
          corporation_blueprint(CORPORATION_ID, 10, 3_000),
          corporation_blueprint(CORPORATION_ID, 11, 3_001),
        ],
      )
      .await
      .unwrap();
      super::replace_for_corporation(&db, CORPORATION_ID, &[corporation_blueprint(CORPORATION_ID, 11, 3_001)])
        .await
        .unwrap();

      let blueprints = super::list_for_corporation(&db, CORPORATION_ID).await.unwrap();
      assert_eq!(
        blueprints.iter().map(CorporationBlueprint::item_id).collect::<Vec<_>>(),
        [11]
      );
    }
  }

  mod list_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_blueprints_for_an_authorized_corp() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corporation(&db).await;
      super::replace_for_corporation(&db, CORPORATION_ID, &[corporation_blueprint(CORPORATION_ID, 10, 3_000)])
        .await
        .unwrap();

      let blueprints = super::list_for_corporation(&db, CORPORATION_ID).await.unwrap();

      assert_eq!(blueprints.len(), 1);
      assert_eq!(blueprints[0].item_id(), 10);
    }

    #[tokio::test]
    async fn it_hides_blueprints_for_an_unauthorized_corp() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      super::replace_for_corporation(&db, CORPORATION_ID, &[corporation_blueprint(CORPORATION_ID, 10, 3_000)])
        .await
        .unwrap();

      assert!(
        super::list_for_corporation(&db, CORPORATION_ID)
          .await
          .unwrap()
          .is_empty()
      );
    }
  }

  mod activity_meta {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn insert_meta(db: &Database, blueprint_type_id: i64, activity_id: i64, time: i64, max: i64) {
      sqlx::query(
        "INSERT INTO blueprint_activity_meta (blueprint_type_id, activity_id, time, max_production_limit) \
        VALUES (?, ?, ?, ?)",
      )
      .bind(blueprint_type_id)
      .bind(activity_id)
      .bind(time)
      .bind(max)
      .execute(&db.0)
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_returns_time_and_max_run_limit_for_a_blueprint_activity() {
      let db = store::open_test().await.unwrap();
      insert_meta(&db, 939, 1, 600, 300).await;

      let meta = super::activity_meta(&db, 939, 1).await.unwrap();

      assert_eq!(meta, Some((600, 300)));
    }

    #[tokio::test]
    async fn it_returns_none_when_the_activity_is_absent() {
      let db = store::open_test().await.unwrap();
      insert_meta(&db, 939, 1, 600, 300).await;

      let meta = super::activity_meta(&db, 939, 11).await.unwrap();

      assert_eq!(meta, None);
    }
  }

  mod list_all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_both_character_and_corporation_blueprints() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      seed_character(&db, DIRECTOR_ID).await;
      super::replace_for_character(&db, CHARACTER_ID, &[character_blueprint(CHARACTER_ID, 1, 1_000)])
        .await
        .unwrap();
      super::replace_for_corporation(&db, CORPORATION_ID, &[corporation_blueprint(CORPORATION_ID, 10, 3_000)])
        .await
        .unwrap();

      let all = super::list_all(&db).await.unwrap();

      assert_eq!(
        all
          .character_blueprints
          .iter()
          .map(CharacterBlueprint::item_id)
          .collect::<Vec<_>>(),
        [1]
      );
      assert_eq!(
        all
          .corporation_blueprints
          .iter()
          .map(CorporationBlueprint::item_id)
          .collect::<Vec<_>>(),
        [10]
      );
    }
  }
}
