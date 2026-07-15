use crate::store::{Database, Error, model::CustomsOffice};

#[allow(dead_code)]
pub async fn list_for_corporation(db: &Database, corporation_id: i64) -> Result<Vec<CustomsOffice>, Error> {
  let rows =
    sqlx::query_as::<_, CustomsOffice>("SELECT * FROM customs_offices WHERE corporation_id = ? ORDER BY office_id")
      .bind(corporation_id)
      .fetch_all(db.reader())
      .await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn read(db: &Database, office_id: i64) -> Result<Option<CustomsOffice>, Error> {
  let row = sqlx::query_as::<_, CustomsOffice>("SELECT * FROM customs_offices WHERE office_id = ?")
    .bind(office_id)
    .fetch_optional(db.reader())
    .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn upsert(db: &Database, office: &CustomsOffice) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO customs_offices \
      (office_id, corporation_id, system_id, planet_id, standing_level, reinforce_exit_start, \
       reinforce_exit_end, allow_alliance_access, allow_access_with_standings, alliance_tax_rate, \
       corporation_tax_rate, excellent_standing_tax_rate, good_standing_tax_rate, neutral_standing_tax_rate, \
       bad_standing_tax_rate, terrible_standing_tax_rate, synced_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(office_id) DO UPDATE SET \
      corporation_id              = excluded.corporation_id, \
      system_id                   = excluded.system_id, \
      planet_id                   = excluded.planet_id, \
      standing_level              = excluded.standing_level, \
      reinforce_exit_start        = excluded.reinforce_exit_start, \
      reinforce_exit_end          = excluded.reinforce_exit_end, \
      allow_alliance_access       = excluded.allow_alliance_access, \
      allow_access_with_standings = excluded.allow_access_with_standings, \
      alliance_tax_rate           = excluded.alliance_tax_rate, \
      corporation_tax_rate        = excluded.corporation_tax_rate, \
      excellent_standing_tax_rate = excluded.excellent_standing_tax_rate, \
      good_standing_tax_rate      = excluded.good_standing_tax_rate, \
      neutral_standing_tax_rate   = excluded.neutral_standing_tax_rate, \
      bad_standing_tax_rate       = excluded.bad_standing_tax_rate, \
      terrible_standing_tax_rate  = excluded.terrible_standing_tax_rate, \
      synced_at                   = excluded.synced_at",
  )
  .bind(office.office_id)
  .bind(office.corporation_id)
  .bind(office.system_id)
  .bind(office.planet_id)
  .bind(&office.standing_level)
  .bind(office.reinforce_exit_start)
  .bind(office.reinforce_exit_end)
  .bind(office.allow_alliance_access)
  .bind(office.allow_access_with_standings)
  .bind(office.alliance_tax_rate)
  .bind(office.corporation_tax_rate)
  .bind(office.excellent_standing_tax_rate)
  .bind(office.good_standing_tax_rate)
  .bind(office.neutral_standing_tax_rate)
  .bind(office.bad_standing_tax_rate)
  .bind(office.terrible_standing_tax_rate)
  .bind(&office.synced_at)
  .execute(db.writer())
  .await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  const CORP: i64 = 98_000_001;

  const OFFICE: i64 = 1_026_000_000_001;

  const SYSTEM: i64 = 30_000_001;

  async fn seed_refs(db: &Database) {
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
      VALUES (?, 20000001, 'System', 0, 0, 0, 0.5)",
    )
    .bind(SYSTEM)
    .execute(db.writer())
    .await
    .unwrap();
    sqlx::query(
      "INSERT INTO corporations (id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
      VALUES (?, 1, 1, 1, 'Owner Corp', 0.0, 'OWN')",
    )
    .bind(CORP)
    .execute(db.writer())
    .await
    .unwrap();
  }

  fn make_office(office_id: i64) -> CustomsOffice {
    CustomsOffice {
      alliance_tax_rate: Some(0.02),
      allow_access_with_standings: true,
      allow_alliance_access: false,
      bad_standing_tax_rate: Some(0.2),
      corporation_id: CORP,
      corporation_tax_rate: Some(0.05),
      excellent_standing_tax_rate: Some(0.01),
      good_standing_tax_rate: Some(0.02),
      neutral_standing_tax_rate: Some(0.05),
      office_id,
      planet_id: Some(40_000_001),
      reinforce_exit_end: 22,
      reinforce_exit_start: 18,
      standing_level: "neutral".to_owned(),
      synced_at: "2026-07-15T00:00:00Z".to_owned(),
      system_id: SYSTEM,
      terrible_standing_tax_rate: Some(0.3),
    }
  }

  mod list_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_every_office_owned_by_the_corporation() {
      let db = store::open_test().await.unwrap();
      seed_refs(&db).await;
      upsert(&db, &make_office(OFFICE)).await.unwrap();
      upsert(&db, &make_office(OFFICE + 1)).await.unwrap();

      let offices = list_for_corporation(&db, CORP).await.unwrap();

      assert_eq!(offices.len(), 2);
      assert_eq!(offices[0].office_id, OFFICE);
      assert_eq!(offices[1].office_id, OFFICE + 1);
    }
  }

  mod read {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_before_any_upsert() {
      let db = store::open_test().await.unwrap();
      seed_refs(&db).await;

      assert_eq!(read(&db, OFFICE).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_returns_the_persisted_office() {
      let db = store::open_test().await.unwrap();
      seed_refs(&db).await;
      upsert(&db, &make_office(OFFICE)).await.unwrap();

      let office = read(&db, OFFICE).await.unwrap().unwrap();

      assert_eq!(office.office_id, OFFICE);
      assert_eq!(office.system_id, SYSTEM);
      assert_eq!(office.standing_level, "neutral");
      assert_eq!(office.reinforce_exit_start, 18);
      assert!(office.allow_access_with_standings);
      assert_eq!(office.corporation_tax_rate, Some(0.05));
      assert_eq!(office.planet_id, Some(40_000_001));
    }
  }

  mod upsert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_overwrites_an_existing_row_on_conflict() {
      let db = store::open_test().await.unwrap();
      seed_refs(&db).await;
      upsert(&db, &make_office(OFFICE)).await.unwrap();

      let mut updated = make_office(OFFICE);
      updated.standing_level = "excellent".to_owned();
      updated.corporation_tax_rate = None;
      updated.synced_at = "2026-07-16T00:00:00Z".to_owned();
      upsert(&db, &updated).await.unwrap();

      let office = read(&db, OFFICE).await.unwrap().unwrap();

      assert_eq!(office.standing_level, "excellent");
      assert_eq!(office.corporation_tax_rate, None);
      assert_eq!(office.synced_at, "2026-07-16T00:00:00Z");
    }

    #[tokio::test]
    async fn it_is_dropped_when_the_owning_corporation_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_refs(&db).await;
      upsert(&db, &make_office(OFFICE)).await.unwrap();

      sqlx::query("DELETE FROM corporations WHERE id = ?")
        .bind(CORP)
        .execute(db.writer())
        .await
        .unwrap();

      assert_eq!(read(&db, OFFICE).await.unwrap(), None);
    }
  }
}
