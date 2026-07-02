use chrono::Utc;

use crate::store::{
  Database, Error,
  model::{SyncList, SyncListContact, SyncListTarget, SyncPushedContact},
};

#[cfg_attr(not(test), expect(dead_code))]
pub struct SyncListDetail {
  pub contacts: Vec<SyncListContact>,
  pub list: SyncList,
  pub targets: Vec<SyncListTarget>,
}

pub async fn create_list(db: &Database, name: &str) -> Result<SyncList, Error> {
  let now = Utc::now().to_rfc3339();
  let list = sqlx::query_as::<_, SyncList>(
    "INSERT INTO sync_lists (created_at, name, updated_at) VALUES (?, ?, ?) \
    RETURNING created_at, id, name, updated_at",
  )
  .bind(&now)
  .bind(name)
  .bind(&now)
  .fetch_one(&db.0)
  .await?;
  Ok(list)
}

pub async fn rename_list(db: &Database, id: i64, name: &str) -> Result<(), Error> {
  let now = Utc::now().to_rfc3339();
  sqlx::query("UPDATE sync_lists SET name = ?, updated_at = ? WHERE id = ?")
    .bind(name)
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn delete_list(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM sync_lists WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn lists(db: &Database) -> Result<Vec<SyncList>, Error> {
  let rows =
    sqlx::query_as::<_, SyncList>("SELECT created_at, id, name, updated_at FROM sync_lists ORDER BY created_at, id")
      .fetch_all(&db.0)
      .await?;
  Ok(rows)
}

pub async fn get_list(db: &Database, id: i64) -> Result<Option<SyncList>, Error> {
  let row = sqlx::query_as::<_, SyncList>("SELECT created_at, id, name, updated_at FROM sync_lists WHERE id = ?")
    .bind(id)
    .fetch_optional(&db.0)
    .await?;
  Ok(row)
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn list_detail(db: &Database, id: i64) -> Result<Option<SyncListDetail>, Error> {
  let Some(list) = get_list(db, id).await? else {
    return Ok(None);
  };
  let contacts = list_contacts(db, id).await?;
  let targets = list_targets(db, id).await?;
  Ok(Some(SyncListDetail {
    contacts,
    list,
    targets,
  }))
}

pub async fn add_contact(
  db: &Database,
  list_id: i64,
  entity_type: &str,
  entity_id: i64,
  standing: i64,
) -> Result<SyncListContact, Error> {
  let now = Utc::now().to_rfc3339();
  let contact = sqlx::query_as::<_, SyncListContact>(
    "INSERT INTO sync_list_contacts (created_at, entity_id, entity_type, list_id, standing, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?) \
    ON CONFLICT(list_id, entity_type, entity_id) DO UPDATE SET \
      standing   = excluded.standing, \
      updated_at = excluded.updated_at \
    RETURNING created_at, entity_id, entity_type, id, list_id, standing, updated_at",
  )
  .bind(&now)
  .bind(entity_id)
  .bind(entity_type)
  .bind(list_id)
  .bind(standing)
  .bind(&now)
  .fetch_one(&db.0)
  .await?;
  Ok(contact)
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn set_contact_standing(db: &Database, id: i64, standing: i64) -> Result<(), Error> {
  let now = Utc::now().to_rfc3339();
  sqlx::query("UPDATE sync_list_contacts SET standing = ?, updated_at = ? WHERE id = ?")
    .bind(standing)
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn remove_contact(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM sync_list_contacts WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn list_contacts(db: &Database, list_id: i64) -> Result<Vec<SyncListContact>, Error> {
  let rows = sqlx::query_as::<_, SyncListContact>(
    "SELECT created_at, entity_id, entity_type, id, list_id, standing, updated_at FROM sync_list_contacts \
    WHERE list_id = ? ORDER BY entity_type, entity_id",
  )
  .bind(list_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn set_targets(db: &Database, list_id: i64, character_ids: &[i64]) -> Result<(), Error> {
  let now = Utc::now().to_rfc3339();
  let mut tx = db.writer().begin().await?;
  sqlx::query("DELETE FROM sync_list_targets WHERE list_id = ?")
    .bind(list_id)
    .execute(&mut *tx)
    .await?;
  for character_id in character_ids {
    sqlx::query("INSERT INTO sync_list_targets (character_id, created_at, list_id) VALUES (?, ?, ?)")
      .bind(character_id)
      .bind(&now)
      .bind(list_id)
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}

pub async fn list_targets(db: &Database, list_id: i64) -> Result<Vec<SyncListTarget>, Error> {
  let rows = sqlx::query_as::<_, SyncListTarget>(
    "SELECT character_id, created_at, list_id FROM sync_list_targets WHERE list_id = ? ORDER BY character_id",
  )
  .bind(list_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn pushed_contacts(db: &Database, character_id: i64) -> Result<Vec<SyncPushedContact>, Error> {
  let rows = sqlx::query_as::<_, SyncPushedContact>(
    "SELECT character_id, created_at, entity_id, entity_type, pushed_standing, updated_at FROM sync_pushed_contacts \
    WHERE character_id = ? ORDER BY entity_type, entity_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn record_pushed(
  db: &Database,
  character_id: i64,
  entity_type: &str,
  entity_id: i64,
  pushed_standing: i64,
) -> Result<SyncPushedContact, Error> {
  let now = Utc::now().to_rfc3339();
  let pushed = sqlx::query_as::<_, SyncPushedContact>(
    "INSERT INTO sync_pushed_contacts (character_id, created_at, entity_id, entity_type, pushed_standing, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?) \
    ON CONFLICT(character_id, entity_type, entity_id) DO UPDATE SET \
      pushed_standing = excluded.pushed_standing, \
      updated_at      = excluded.updated_at \
    RETURNING character_id, created_at, entity_id, entity_type, pushed_standing, updated_at",
  )
  .bind(character_id)
  .bind(&now)
  .bind(entity_id)
  .bind(entity_type)
  .bind(pushed_standing)
  .bind(&now)
  .fetch_one(&db.0)
  .await?;
  Ok(pushed)
}

#[cfg_attr(not(test), expect(dead_code))]
pub async fn delete_pushed(db: &Database, character_id: i64, entity_type: &str, entity_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM sync_pushed_contacts WHERE character_id = ? AND entity_type = ? AND entity_id = ?")
    .bind(character_id)
    .bind(entity_type)
    .bind(entity_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
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

  mod create_list {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_a_named_list_with_matching_timestamps() {
      let db = store::open_test().await.unwrap();

      let list = create_list(&db, "Gankers").await.unwrap();

      assert_eq!(list.name(), "Gankers");
      assert_eq!(list.created_at(), list.updated_at());
    }
  }

  mod rename_list {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_updates_the_name() {
      let db = store::open_test().await.unwrap();
      let list = create_list(&db, "Old").await.unwrap();

      rename_list(&db, list.id(), "New").await.unwrap();

      assert_eq!(get_list(&db, list.id()).await.unwrap().unwrap().name(), "New");
    }
  }

  mod delete_list {
    use super::*;

    #[tokio::test]
    async fn it_cascades_to_contacts_and_targets() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let list = create_list(&db, "Blues").await.unwrap();
      add_contact(&db, list.id(), "character", 1001, 10).await.unwrap();
      set_targets(&db, list.id(), &[42]).await.unwrap();

      delete_list(&db, list.id()).await.unwrap();

      assert!(get_list(&db, list.id()).await.unwrap().is_none());
      assert!(list_contacts(&db, list.id()).await.unwrap().is_empty());
      assert!(list_targets(&db, list.id()).await.unwrap().is_empty());
    }
  }

  mod lists {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_every_account_global_list() {
      let db = store::open_test().await.unwrap();
      create_list(&db, "A").await.unwrap();
      create_list(&db, "B").await.unwrap();

      assert_eq!(lists(&db).await.unwrap().len(), 2);
    }
  }

  mod add_contact {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_a_contact_at_a_standing() {
      let db = store::open_test().await.unwrap();
      let list = create_list(&db, "Gankers").await.unwrap();

      let contact = add_contact(&db, list.id(), "corporation", 2001, -10).await.unwrap();

      assert_eq!(contact.entity_type(), "corporation");
      assert_eq!(contact.entity_id(), 2001);
      assert_eq!(contact.standing(), -10);
    }

    #[tokio::test]
    async fn it_upserts_the_standing_on_a_duplicate_entity() {
      let db = store::open_test().await.unwrap();
      let list = create_list(&db, "Gankers").await.unwrap();
      add_contact(&db, list.id(), "character", 3001, -5).await.unwrap();

      add_contact(&db, list.id(), "character", 3001, -10).await.unwrap();

      let contacts = list_contacts(&db, list.id()).await.unwrap();
      assert_eq!(contacts.len(), 1);
      assert_eq!(contacts[0].standing(), -10);
    }
  }

  mod set_contact_standing {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_changes_only_the_standing() {
      let db = store::open_test().await.unwrap();
      let list = create_list(&db, "Gankers").await.unwrap();
      let contact = add_contact(&db, list.id(), "character", 3001, 0).await.unwrap();

      set_contact_standing(&db, contact.id(), 5).await.unwrap();

      assert_eq!(list_contacts(&db, list.id()).await.unwrap()[0].standing(), 5);
    }
  }

  mod remove_contact {
    use super::*;

    #[tokio::test]
    async fn it_deletes_the_contact() {
      let db = store::open_test().await.unwrap();
      let list = create_list(&db, "Gankers").await.unwrap();
      let contact = add_contact(&db, list.id(), "character", 3001, 0).await.unwrap();

      remove_contact(&db, contact.id()).await.unwrap();

      assert!(list_contacts(&db, list.id()).await.unwrap().is_empty());
    }
  }

  mod set_targets {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_replaces_the_target_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      let list = create_list(&db, "Blues").await.unwrap();

      set_targets(&db, list.id(), &[42, 43]).await.unwrap();
      set_targets(&db, list.id(), &[43]).await.unwrap();

      let targets = list_targets(&db, list.id()).await.unwrap();
      assert_eq!(targets.iter().map(|t| t.character_id()).collect::<Vec<_>>(), [43]);
    }
  }

  mod list_detail {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_reads_the_list_with_contacts_and_targets() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let list = create_list(&db, "Blues").await.unwrap();
      add_contact(&db, list.id(), "character", 1001, 10).await.unwrap();
      set_targets(&db, list.id(), &[42]).await.unwrap();

      let detail = list_detail(&db, list.id()).await.unwrap().unwrap();

      assert_eq!(detail.list.name(), "Blues");
      assert_eq!(detail.contacts.len(), 1);
      assert_eq!(detail.targets.len(), 1);
    }

    #[tokio::test]
    async fn it_returns_none_for_a_missing_list() {
      let db = store::open_test().await.unwrap();

      assert!(list_detail(&db, 999).await.unwrap().is_none());
    }
  }

  mod record_pushed {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_upserts_the_pushed_standing() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      record_pushed(&db, 42, "character", 5001, -5).await.unwrap();
      record_pushed(&db, 42, "character", 5001, -10).await.unwrap();

      let pushed = pushed_contacts(&db, 42).await.unwrap();
      assert_eq!(pushed.len(), 1);
      assert_eq!(pushed[0].pushed_standing(), -10);
    }
  }

  mod delete_pushed {
    use super::*;

    #[tokio::test]
    async fn it_removes_a_pushed_row() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      record_pushed(&db, 42, "character", 5001, -5).await.unwrap();

      delete_pushed(&db, 42, "character", 5001).await.unwrap();

      assert!(pushed_contacts(&db, 42).await.unwrap().is_empty());
    }
  }
}
