use std::collections::HashMap;

use chrono::Utc;

use crate::store::{
  Database, Error,
  model::{Credential, EntityTag, HttpCacheEntry, Outbox, OwnerType, TAG_SCOPE_ENTITY, Tag},
};

pub async fn all(db: &Database) -> Result<Vec<Credential>, Error> {
  let rows = sqlx::query_as::<_, Credential>(
    "SELECT access_token, authorized_by, created_at, expires_at, last_checked_at, needs_reauth, owner_id, \
    owner_type, refresh_token, scopes, updated_at FROM credentials",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn delete(db: &Database, owner_id: i64, owner_type: OwnerType) -> Result<(), Error> {
  sqlx::query("DELETE FROM credentials WHERE owner_id = ? AND owner_type = ?")
    .bind(owner_id)
    .bind(owner_type)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn get(db: &Database, owner_id: i64, owner_type: OwnerType) -> Result<Option<Credential>, Error> {
  let row = sqlx::query_as::<_, Credential>(
    "SELECT access_token, authorized_by, created_at, expires_at, last_checked_at, needs_reauth, owner_id, \
    owner_type, refresh_token, scopes, updated_at FROM credentials \
    WHERE owner_id = ? AND owner_type = ?",
  )
  .bind(owner_id)
  .bind(owner_type)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

// Arguments map directly to the persisted credential columns; bundling them into a struct would only move the fields.
#[allow(clippy::too_many_arguments)]
pub async fn upsert(
  db: &Database,
  owner_id: i64,
  owner_type: OwnerType,
  access_token: &str,
  refresh_token: &str,
  expires_at: i64,
  authorized_by: Option<i64>,
  scopes: Option<&str>,
) -> Result<(), Error> {
  let now = Utc::now().timestamp();
  sqlx::query(
    "INSERT INTO credentials \
      (owner_id, owner_type, access_token, refresh_token, expires_at, authorized_by, scopes, \
      created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(owner_id, owner_type) DO UPDATE SET \
      access_token  = excluded.access_token, \
      refresh_token = excluded.refresh_token, \
      expires_at    = excluded.expires_at, \
      authorized_by = excluded.authorized_by, \
      scopes        = excluded.scopes, \
      updated_at    = excluded.updated_at",
  )
  .bind(owner_id)
  .bind(owner_type)
  .bind(access_token)
  .bind(refresh_token)
  .bind(expires_at)
  .bind(authorized_by)
  .bind(scopes)
  .bind(now)
  .bind(now)
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn mark_needs_reauth(db: &Database, owner_id: i64, owner_type: OwnerType) -> Result<(), Error> {
  let now = Utc::now().timestamp();
  sqlx::query("UPDATE credentials SET needs_reauth = 1, last_checked_at = ? WHERE owner_id = ? AND owner_type = ?")
    .bind(now)
    .bind(owner_id)
    .bind(owner_type)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn clear_needs_reauth(db: &Database, owner_id: i64, owner_type: OwnerType) -> Result<(), Error> {
  let now = Utc::now().timestamp();
  sqlx::query("UPDATE credentials SET needs_reauth = 0, last_checked_at = ? WHERE owner_id = ? AND owner_type = ?")
    .bind(now)
    .bind(owner_id)
    .bind(owner_type)
    .execute(db.writer())
    .await?;
  Ok(())
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn http_cache_delete(db: &Database, url: &str) -> Result<(), Error> {
  sqlx::query("DELETE FROM http_cache WHERE url = ?")
    .bind(url)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn http_cache_get(db: &Database, url: &str) -> Result<Option<HttpCacheEntry>, Error> {
  let row =
    sqlx::query_as::<_, HttpCacheEntry>("SELECT body, cached_at, etag, expires_at, url FROM http_cache WHERE url = ?")
      .bind(url)
      .fetch_optional(&db.0)
      .await?;
  Ok(row)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn purge_expired(db: &Database) -> Result<u64, Error> {
  let result = sqlx::query("DELETE FROM http_cache WHERE expires_at IS NOT NULL AND expires_at < ?")
    .bind(Utc::now().timestamp())
    .execute(db.writer())
    .await?;

  Ok(result.rows_affected())
}

pub async fn http_cache_upsert(db: &Database, entry: &HttpCacheEntry) -> Result<(), Error> {
  sqlx::query(
    "INSERT OR REPLACE INTO http_cache (body, cached_at, etag, expires_at, url) \
    VALUES (?, ?, ?, ?, ?)",
  )
  .bind(entry.body())
  .bind(entry.cached_at())
  .bind(entry.etag().as_deref())
  .bind(entry.expires_at())
  .bind(entry.url().as_str())
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn append(
  db: &Database,
  subject_type: OwnerType,
  subject_id: i64,
  kind: &str,
  payload: &str,
  dedupe_key: Option<&str>,
) -> Result<Outbox, Error> {
  let now = Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, Outbox>(
    "INSERT INTO outbox \
      (subject_type, subject_id, kind, payload, dedupe_key, next_attempt_at, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(subject_id, kind, dedupe_key) WHERE dedupe_key IS NOT NULL AND status IN ('pending', 'inflight') \
    DO UPDATE SET \
      payload = excluded.payload, status = 'pending', attempts = 0, next_attempt_at = excluded.next_attempt_at, \
      last_error = NULL, updated_at = excluded.updated_at \
    RETURNING attempts, created_at, dedupe_key, id, kind, last_error, next_attempt_at, payload, status, subject_id, \
      subject_type, updated_at",
  )
  .bind(subject_type)
  .bind(subject_id)
  .bind(kind)
  .bind(payload)
  .bind(dedupe_key)
  .bind(&now)
  .bind(&now)
  .bind(&now)
  .fetch_one(&db.0)
  .await?;
  Ok(row)
}

pub async fn claim_due(db: &Database, now: &str, limit: i64) -> Result<Vec<Outbox>, Error> {
  let mut tx = db.writer().begin().await?;
  let ids = sqlx::query_scalar::<_, i64>(
    "SELECT id FROM outbox \
    WHERE status IN ('pending', 'inflight') AND next_attempt_at <= ? \
    ORDER BY created_at, id LIMIT ?",
  )
  .bind(now)
  .bind(limit)
  .fetch_all(&mut *tx)
  .await?;

  let mut claimed = Vec::with_capacity(ids.len());
  for id in ids {
    let row = sqlx::query_as::<_, Outbox>(
      "UPDATE outbox SET status = 'inflight', updated_at = ? WHERE id = ? \
      RETURNING attempts, created_at, dedupe_key, id, kind, last_error, next_attempt_at, payload, status, subject_id, \
        subject_type, updated_at",
    )
    .bind(now)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    claimed.push(row);
  }
  tx.commit().await?;
  Ok(claimed)
}

pub async fn mark_done(db: &Database, id: i64) -> Result<(), Error> {
  let now = Utc::now().to_rfc3339();
  sqlx::query("UPDATE outbox SET status = 'done', last_error = NULL, updated_at = ? WHERE id = ?")
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn mark_failed(db: &Database, id: i64, error: &str) -> Result<(), Error> {
  let now = Utc::now().to_rfc3339();
  sqlx::query("UPDATE outbox SET status = 'failed', last_error = ?, updated_at = ? WHERE id = ?")
    .bind(error)
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn reschedule(db: &Database, id: i64, next_attempt_at: &str, last_error: &str) -> Result<(), Error> {
  let now = Utc::now().to_rfc3339();
  sqlx::query(
    "UPDATE outbox SET status = 'pending', attempts = attempts + 1, next_attempt_at = ?, last_error = ?, \
    updated_at = ? WHERE id = ?",
  )
  .bind(next_attempt_at)
  .bind(last_error)
  .bind(&now)
  .bind(id)
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn prune_done(db: &Database, before: &str) -> Result<u64, Error> {
  let result = sqlx::query("DELETE FROM outbox WHERE status = 'done' AND updated_at < ?")
    .bind(before)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

pub async fn outbox_failed_by_kind(
  db: &Database,
  kind_prefix: &str,
) -> Result<Vec<(i64, String, Option<String>)>, Error> {
  let pattern = format!("{}%", escape_like(kind_prefix));
  let rows = sqlx::query_as::<_, (i64, String, Option<String>)>(
    "SELECT id, kind, last_error FROM outbox WHERE kind LIKE ? AND status = 'failed' ORDER BY updated_at DESC",
  )
  .bind(pattern)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn outbox_pending_count_by_kind(db: &Database, kind_prefix: &str) -> Result<i64, Error> {
  let pattern = format!("{}%", escape_like(kind_prefix));
  let count =
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox WHERE kind LIKE ? AND status IN ('pending', 'inflight')")
      .bind(pattern)
      .fetch_one(&db.0)
      .await?;
  Ok(count)
}

pub async fn outbox_pending_payloads(
  db: &Database,
  subject_type: OwnerType,
  subject_id: i64,
  kind: &str,
) -> Result<Vec<String>, Error> {
  let rows = sqlx::query_scalar::<_, String>(
    "SELECT payload FROM outbox \
    WHERE subject_type = ? AND subject_id = ? AND kind = ? AND status IN ('pending', 'inflight')",
  )
  .bind(subject_type)
  .bind(subject_id)
  .bind(kind)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub fn escape_like(value: &str) -> String {
  value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

pub fn like_pattern(value: &str) -> String {
  format!("%{}%", escape_like(value))
}

pub async fn tag_all(db: &Database) -> Result<Vec<Tag>, Error> {
  tag_all_scoped(db, TAG_SCOPE_ENTITY).await
}

// Public store API consumed by the asset-tag UI tasks (filter/modal/chips/settings); exercised by unit tests
// until those callers land.
#[allow(dead_code)]
pub async fn tag_all_scoped(db: &Database, scope: &str) -> Result<Vec<Tag>, Error> {
  let rows = sqlx::query_as::<_, Tag>(
    "SELECT color, created_at, description, id, name, position, updated_at FROM tags WHERE scope = ? ORDER BY position",
  )
  .bind(scope)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn assign(db: &Database, entity_type: &str, entity_id: i64, tag_id: i64) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO entity_tags (tag_id, entity_type, entity_id) VALUES (?, ?, ?) \
    ON CONFLICT(tag_id, entity_type, entity_id) DO NOTHING",
  )
  .bind(tag_id)
  .bind(entity_type)
  .bind(entity_id)
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn create(db: &Database, name: &str, description: Option<&str>, color: Option<&str>) -> Result<Tag, Error> {
  create_scoped(db, name, description, color, TAG_SCOPE_ENTITY).await
}

// Public store API consumed by the asset-tag UI tasks (settings/modal); exercised by unit tests until those
// callers land.
#[allow(dead_code)]
pub async fn create_scoped(
  db: &Database,
  name: &str,
  description: Option<&str>,
  color: Option<&str>,
  scope: &str,
) -> Result<Tag, Error> {
  let now = Utc::now().timestamp();
  let tag = sqlx::query_as::<_, Tag>(
    "INSERT INTO tags (color, created_at, description, name, position, scope, updated_at) \
    VALUES (?, ?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM tags WHERE scope = ?), ?, ?) \
    RETURNING color, created_at, description, id, name, position, updated_at",
  )
  .bind(color)
  .bind(now)
  .bind(description)
  .bind(name)
  .bind(scope)
  .bind(scope)
  .bind(now)
  .fetch_one(&db.0)
  .await?;
  Ok(tag)
}

// Tag-scope seed marker, mirroring budget's once-only seed guard: a seeded scope stays seeded so a deleted
// default is never resurrected. Consumed by the asset-registry seed path; exercised by unit tests until wired.
#[allow(dead_code)]
pub async fn is_tag_scope_seeded(db: &Database, scope: &str) -> Result<bool, Error> {
  let row: Option<i64> = sqlx::query_scalar("SELECT 1 FROM tag_scope_seeded WHERE scope = ?")
    .bind(scope)
    .fetch_optional(&db.0)
    .await?;
  Ok(row.is_some())
}

// Tag-scope seed marker companion to is_tag_scope_seeded. Consumed by the asset-registry seed path; exercised
// by unit tests until wired.
#[allow(dead_code)]
pub async fn mark_tag_scope_seeded(db: &Database, scope: &str) -> Result<(), Error> {
  let now = Utc::now().to_rfc3339();
  sqlx::query("INSERT INTO tag_scope_seeded (scope, seeded_at) VALUES (?, ?) ON CONFLICT(scope) DO NOTHING")
    .bind(scope)
    .bind(&now)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn tag_delete(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM tags WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn tag_get(db: &Database, id: i64) -> Result<Option<Tag>, Error> {
  let row = sqlx::query_as::<_, Tag>(
    "SELECT color, created_at, description, id, name, position, updated_at FROM tags WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn members(db: &Database, tag_id: i64, entity_type: &str) -> Result<Vec<i64>, Error> {
  let ids = sqlx::query_scalar::<_, i64>(
    "SELECT entity_id FROM entity_tags WHERE tag_id = ? AND entity_type = ? ORDER BY entity_id",
  )
  .bind(tag_id)
  .bind(entity_type)
  .fetch_all(&db.0)
  .await?;
  Ok(ids)
}

pub async fn memberships(db: &Database, entity_type: &str) -> Result<Vec<EntityTag>, Error> {
  let rows = sqlx::query_as::<_, EntityTag>(
    "SELECT entity_id, entity_type, tag_id FROM entity_tags WHERE entity_type = ? ORDER BY entity_id, tag_id",
  )
  .bind(entity_type)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Per-entity tag membership map (entity_id -> tag_ids, in tag position order) for one entity type, so a view
// can resolve every row's chips from a single query instead of a per-row scan. Consumed by the asset inventory
// tag chips; exercised by unit tests until that caller lands.
#[allow(dead_code)]
pub async fn membership_map(db: &Database, entity_type: &str) -> Result<HashMap<i64, Vec<i64>>, Error> {
  let rows = sqlx::query_as::<_, (i64, i64)>(
    "SELECT et.entity_id, et.tag_id FROM entity_tags et \
    JOIN tags t ON t.id = et.tag_id \
    WHERE et.entity_type = ? ORDER BY et.entity_id, t.position",
  )
  .bind(entity_type)
  .fetch_all(&db.0)
  .await?;

  let mut map: HashMap<i64, Vec<i64>> = HashMap::new();
  for (entity_id, tag_id) in rows {
    map.entry(entity_id).or_default().push(tag_id);
  }
  Ok(map)
}

pub async fn reorder(db: &Database, ordered_ids: &[i64]) -> Result<(), Error> {
  let now = Utc::now().timestamp();
  let mut tx = db.writer().begin().await?;
  for (position, id) in ordered_ids.iter().enumerate() {
    sqlx::query("UPDATE tags SET position = ?, updated_at = ? WHERE id = ?")
      .bind(position as i64)
      .bind(now)
      .bind(id)
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}

pub async fn unassign(db: &Database, entity_type: &str, entity_id: i64, tag_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM entity_tags WHERE tag_id = ? AND entity_type = ? AND entity_id = ?")
    .bind(tag_id)
    .bind(entity_type)
    .bind(entity_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn update(
  db: &Database,
  id: i64,
  name: &str,
  description: Option<&str>,
  color: Option<&str>,
) -> Result<(), Error> {
  let now = Utc::now().timestamp();
  sqlx::query("UPDATE tags SET color = ?, description = ?, name = ?, updated_at = ? WHERE id = ?")
    .bind(color)
    .bind(description)
    .bind(name)
    .bind(now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

#[cfg(test)]
mod credential_tests {
  use super::*;
  use crate::store;

  mod all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_all_stored_credentials() {
      let db = store::open_test().await.unwrap();
      upsert(&db, 111, OwnerType::Character, "tok1", "rt1", 1000, None, None)
        .await
        .unwrap();
      upsert(
        &db,
        222,
        OwnerType::Corporation,
        "tok2",
        "rt2",
        2000,
        Some(111),
        Some("esi-corps.read.v1"),
      )
      .await
      .unwrap();

      let result = all(&db).await.unwrap();

      assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_credentials_exist() {
      let db = store::open_test().await.unwrap();

      let result = all(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }
  }

  mod delete {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_a_noop_for_an_unknown_owner() {
      let db = store::open_test().await.unwrap();

      delete(&db, 999, OwnerType::Character).await.unwrap();
    }

    #[tokio::test]
    async fn it_removes_the_row() {
      let db = store::open_test().await.unwrap();
      upsert(&db, 111, OwnerType::Character, "tok", "rt", 1000, None, None)
        .await
        .unwrap();

      delete(&db, 111, OwnerType::Character).await.unwrap();

      let result = get(&db, 111, OwnerType::Character).await.unwrap();
      assert_eq!(result, None);
    }
  }

  mod get {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_owner() {
      let db = store::open_test().await.unwrap();

      let result = get(&db, 999, OwnerType::Character).await.unwrap();

      assert_eq!(result, None);
    }

    #[tokio::test]
    async fn it_returns_the_credential_for_a_known_owner() {
      let db = store::open_test().await.unwrap();
      upsert(
        &db,
        111,
        OwnerType::Character,
        "tok",
        "rt",
        9999,
        None,
        Some("esi-skills.read.v1"),
      )
      .await
      .unwrap();

      let result = get(&db, 111, OwnerType::Character).await.unwrap();

      assert!(result.is_some());
      let cred = result.unwrap();
      assert_eq!(cred.owner_id(), 111);
      assert_eq!(cred.owner_type(), OwnerType::Character);
      assert_eq!(cred.access_token(), "tok");
      assert_eq!(cred.refresh_token(), "rt");
      assert_eq!(cred.expires_at(), 9999);
      assert_eq!(cred.authorized_by(), None);
      assert_eq!(cred.scopes().as_deref(), Some("esi-skills.read.v1"));
    }
  }

  mod needs_reauth {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn a_fresh_credential_defaults_to_healthy() {
      let db = store::open_test().await.unwrap();
      upsert(&db, 111, OwnerType::Character, "tok", "rt", 1000, None, None)
        .await
        .unwrap();

      let cred = get(&db, 111, OwnerType::Character).await.unwrap().unwrap();

      assert!(!cred.needs_reauth());
      assert_eq!(cred.last_checked_at(), None);
    }

    #[tokio::test]
    async fn it_round_trips_marking_clearing_and_survives_a_token_refresh() {
      let db = store::open_test().await.unwrap();
      upsert(&db, 111, OwnerType::Character, "tok", "rt", 1000, None, None)
        .await
        .unwrap();

      mark_needs_reauth(&db, 111, OwnerType::Character).await.unwrap();
      let marked = get(&db, 111, OwnerType::Character).await.unwrap().unwrap();
      assert!(marked.needs_reauth());
      assert!(marked.last_checked_at().is_some());

      clear_needs_reauth(&db, 111, OwnerType::Character).await.unwrap();
      let cleared = get(&db, 111, OwnerType::Character).await.unwrap().unwrap();
      assert!(!cleared.needs_reauth());
      assert!(cleared.last_checked_at().is_some());

      mark_needs_reauth(&db, 111, OwnerType::Character).await.unwrap();
      upsert(
        &db,
        111,
        OwnerType::Character,
        "new-tok",
        "new-rt",
        9999,
        None,
        Some("esi-skills.read.v1"),
      )
      .await
      .unwrap();

      let after_refresh = get(&db, 111, OwnerType::Character).await.unwrap().unwrap();
      assert!(after_refresh.needs_reauth());
      assert_eq!(after_refresh.access_token(), "new-tok");
    }

    #[tokio::test]
    async fn mark_and_clear_target_only_the_keyed_owner() {
      let db = store::open_test().await.unwrap();
      upsert(&db, 111, OwnerType::Character, "tok", "rt", 1000, None, None)
        .await
        .unwrap();
      upsert(&db, 111, OwnerType::Corporation, "tok", "rt", 1000, Some(111), None)
        .await
        .unwrap();

      mark_needs_reauth(&db, 111, OwnerType::Character).await.unwrap();

      let character = get(&db, 111, OwnerType::Character).await.unwrap().unwrap();
      let corporation = get(&db, 111, OwnerType::Corporation).await.unwrap().unwrap();
      assert!(character.needs_reauth());
      assert!(!corporation.needs_reauth());
    }
  }

  mod upsert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_overwrites_authorized_by_on_a_re_add_by_another_director() {
      let db = store::open_test().await.unwrap();
      upsert(&db, 2000, OwnerType::Corporation, "tok", "rt", 1000, Some(111), None)
        .await
        .unwrap();

      upsert(&db, 2000, OwnerType::Corporation, "tok2", "rt2", 2000, Some(222), None)
        .await
        .unwrap();

      let cred = get(&db, 2000, OwnerType::Corporation).await.unwrap().unwrap();
      assert_eq!(cred.authorized_by(), Some(222));
    }

    #[tokio::test]
    async fn it_round_trips_authorized_by_for_a_corporation_credential() {
      let db = store::open_test().await.unwrap();

      upsert(&db, 2000, OwnerType::Corporation, "tok", "rt", 1000, Some(111), None)
        .await
        .unwrap();

      let cred = get(&db, 2000, OwnerType::Corporation).await.unwrap().unwrap();
      assert_eq!(cred.authorized_by(), Some(111));
    }

    #[tokio::test]
    async fn it_stores_a_new_credential() {
      let db = store::open_test().await.unwrap();

      upsert(
        &db,
        111,
        OwnerType::Character,
        "tok",
        "rt",
        1000,
        None,
        Some("esi-skills.read.v1"),
      )
      .await
      .unwrap();

      let result = get(&db, 111, OwnerType::Character).await.unwrap();
      assert!(result.is_some());
    }

    #[tokio::test]
    async fn it_updates_token_fields_without_changing_created_at() {
      let db = store::open_test().await.unwrap();
      upsert(&db, 111, OwnerType::Character, "old-tok", "old-rt", 1000, None, None)
        .await
        .unwrap();
      let original = get(&db, 111, OwnerType::Character).await.unwrap().unwrap();

      upsert(
        &db,
        111,
        OwnerType::Character,
        "new-tok",
        "new-rt",
        9999,
        None,
        Some("esi-skills.read.v1"),
      )
      .await
      .unwrap();

      let updated = get(&db, 111, OwnerType::Character).await.unwrap().unwrap();
      assert_eq!(updated.created_at(), original.created_at());
      assert_eq!(updated.access_token(), "new-tok");
      assert_eq!(updated.refresh_token(), "new-rt");
      assert_eq!(updated.expires_at(), 9999);
      assert_eq!(updated.scopes().as_deref(), Some("esi-skills.read.v1"));
    }
  }
}

#[cfg(test)]
mod http_cache_tests {
  use super::*;
  use crate::{store, store::model::HttpCacheEntry};

  mod delete {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_a_noop_for_an_unknown_url() {
      let db = store::open_test().await.unwrap();

      http_cache_delete(&db, "https://example.com/unknown").await.unwrap();
    }

    #[tokio::test]
    async fn it_removes_the_row() {
      let db = store::open_test().await.unwrap();
      let entry = HttpCacheEntry::new(b"hello".to_vec(), 1000, "https://example.com/");
      http_cache_upsert(&db, &entry).await.unwrap();

      http_cache_delete(&db, "https://example.com/").await.unwrap();

      let result = http_cache_get(&db, "https://example.com/").await.unwrap();
      assert_eq!(result, None);
    }
  }

  mod get {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_url() {
      let db = store::open_test().await.unwrap();

      let result = http_cache_get(&db, "https://example.com/unknown").await.unwrap();

      assert_eq!(result, None);
    }

    #[tokio::test]
    async fn it_returns_the_entry_for_a_known_url() {
      let db = store::open_test().await.unwrap();
      let mut entry = HttpCacheEntry::new(b"response body".to_vec(), 1_000_000, "https://example.com/resource");
      entry.set_etag("\"abc123\"");
      entry.set_expires_at(9_999_999);
      http_cache_upsert(&db, &entry).await.unwrap();

      let result = http_cache_get(&db, "https://example.com/resource").await.unwrap();

      assert_eq!(result, Some(entry));
    }

    #[tokio::test]
    async fn it_supports_concurrent_access_from_clones() {
      let db = store::open_test().await.unwrap();
      let db2 = db.clone();

      let (r1, r2) = tokio::join!(
        http_cache_get(&db, "https://example.com/a"),
        http_cache_get(&db2, "https://example.com/b"),
      );

      r1.unwrap();
      r2.unwrap();
    }
  }

  mod purge_expired {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_keeps_rows_that_have_not_yet_expired() {
      let db = store::open_test().await.unwrap();
      let mut entry = HttpCacheEntry::new(b"body".to_vec(), 0, "https://example.com/future");
      entry.set_expires_at(i64::MAX);
      http_cache_upsert(&db, &entry).await.unwrap();

      let deleted = purge_expired(&db).await.unwrap();

      assert_eq!(deleted, 0);
      assert!(
        http_cache_get(&db, "https://example.com/future")
          .await
          .unwrap()
          .is_some()
      );
    }

    #[tokio::test]
    async fn it_keeps_rows_with_null_expires_at() {
      let db = store::open_test().await.unwrap();
      let entry = HttpCacheEntry::new(b"body".to_vec(), 0, "https://example.com/no-expiry");
      http_cache_upsert(&db, &entry).await.unwrap();

      let deleted = purge_expired(&db).await.unwrap();

      assert_eq!(deleted, 0);
      assert!(
        http_cache_get(&db, "https://example.com/no-expiry")
          .await
          .unwrap()
          .is_some()
      );
    }

    #[tokio::test]
    async fn it_removes_expired_rows() {
      let db = store::open_test().await.unwrap();
      let mut entry = HttpCacheEntry::new(b"body".to_vec(), 0, "https://example.com/expired");
      entry.set_expires_at(1_000_000);
      http_cache_upsert(&db, &entry).await.unwrap();

      let deleted = purge_expired(&db).await.unwrap();

      assert_eq!(deleted, 1);
      assert!(
        http_cache_get(&db, "https://example.com/expired")
          .await
          .unwrap()
          .is_none()
      );
    }
  }

  mod upsert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_replaces_an_existing_entry() {
      let db = store::open_test().await.unwrap();
      let mut original = HttpCacheEntry::new(b"original".to_vec(), 1000, "https://example.com/resource");
      original.set_etag("\"v1\"");
      http_cache_upsert(&db, &original).await.unwrap();

      let mut updated = HttpCacheEntry::new(b"updated".to_vec(), 2000, "https://example.com/resource");
      updated.set_etag("\"v2\"");
      updated.set_expires_at(9_999_999);
      http_cache_upsert(&db, &updated).await.unwrap();

      let result = http_cache_get(&db, "https://example.com/resource")
        .await
        .unwrap()
        .unwrap();
      assert_eq!(result, updated);
    }

    #[tokio::test]
    async fn it_stores_a_new_entry() {
      let db = store::open_test().await.unwrap();
      let mut entry = HttpCacheEntry::new(b"body content".to_vec(), 1_234_567_890, "https://example.com/path");
      entry.set_etag("\"etag-value\"");
      entry.set_expires_at(9_999_999_999);
      http_cache_upsert(&db, &entry).await.unwrap();

      let result = http_cache_get(&db, "https://example.com/path").await.unwrap();
      assert_eq!(result, Some(entry));
    }
  }
}

#[cfg(test)]
mod migration_cascade_tests {
  use pretty_assertions::assert_eq;

  use crate::store::{
    self, Database,
    model::{
      Alliance, Bloodline, Character, Corporation, Gender, Race, SkillPlan, SkillPlanEntry, SkillPlanRemapPoint,
    },
    repo::character,
  };

  const ATTRIBUTES_COUNT: &str = "SELECT COUNT(*) FROM character_attributes WHERE character_id = ?";

  const IMPLANTS_COUNT: &str = "SELECT COUNT(*) FROM character_implants WHERE character_id = ?";

  const PLANS_COUNT: &str = "SELECT COUNT(*) FROM skill_plans WHERE character_id = ?";

  const ENTRIES_COUNT: &str = "SELECT COUNT(*) FROM skill_plan_entries WHERE plan_id = ?";

  const REMAP_POINTS_COUNT: &str = "SELECT COUNT(*) FROM skill_plan_remap_points WHERE plan_id = ?";

  async fn count(db: &Database, query: &'static str, character_id: i64) -> i64 {
    sqlx::query_scalar(query)
      .bind(character_id)
      .fetch_one(&db.0)
      .await
      .unwrap()
  }

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

  #[tokio::test]
  async fn it_cascades_to_attributes_and_implants_when_a_character_is_deleted() {
    let db = store::open_test().await.unwrap();
    let id = 42;
    seed_character(&db, id).await;

    sqlx::query(
      "INSERT INTO character_attributes \
        (character_id, charisma, intelligence, memory, perception, willpower, bonus_remaps) \
      VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(20_i64)
    .bind(21_i64)
    .bind(22_i64)
    .bind(23_i64)
    .bind(24_i64)
    .bind(2_i64)
    .execute(db.writer())
    .await
    .unwrap();

    for attribute_id in [164_i64, 165, 166] {
      sqlx::query("INSERT INTO character_implants (character_id, attribute_id, bonus) VALUES (?, ?, ?)")
        .bind(id)
        .bind(attribute_id)
        .bind(3_i64)
        .execute(db.writer())
        .await
        .unwrap();
    }

    assert_eq!(count(&db, ATTRIBUTES_COUNT, id).await, 1);
    assert_eq!(count(&db, IMPLANTS_COUNT, id).await, 3);

    character::delete(&db, id).await.unwrap();

    assert_eq!(count(&db, ATTRIBUTES_COUNT, id).await, 0);
    assert_eq!(count(&db, IMPLANTS_COUNT, id).await, 0);
  }

  #[tokio::test]
  async fn it_cascades_to_skill_plans_entries_and_remap_points_when_a_character_is_deleted() {
    let db = store::open_test().await.unwrap();
    let id = 77;
    seed_character(&db, id).await;

    let plan_id: i64 = sqlx::query_scalar(
      "INSERT INTO skill_plans (character_id, name, sort_mode, implant_set, created_at, updated_at) \
        VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(id)
    .bind("Caldari Carrier")
    .bind("manual")
    .bind("current")
    .bind("2026-06-02T00:00:00Z")
    .bind("2026-06-02T00:00:00Z")
    .fetch_one(&db.0)
    .await
    .unwrap();

    let entry_id: i64 = sqlx::query_scalar(
      "INSERT INTO skill_plan_entries (plan_id, skill_id, to_level, position, priority, note, is_auto) \
        VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(plan_id)
    .bind(3300_i64)
    .bind(5_i64)
    .bind(0_i64)
    .bind("normal")
    .bind("")
    .bind(0_i64)
    .fetch_one(&db.0)
    .await
    .unwrap();

    sqlx::query(
      "INSERT INTO skill_plan_remap_points \
        (plan_id, after_entry_id, base_perception, base_memory, base_willpower, base_intelligence, base_charisma) \
        VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(plan_id)
    .bind(entry_id)
    .bind(21_i64)
    .bind(21_i64)
    .bind(21_i64)
    .bind(19_i64)
    .bind(17_i64)
    .execute(db.writer())
    .await
    .unwrap();

    let plan = sqlx::query_as::<_, SkillPlan>("SELECT * FROM skill_plans WHERE id = ?")
      .bind(plan_id)
      .fetch_one(&db.0)
      .await
      .unwrap();
    assert_eq!(plan.id(), plan_id);
    assert_eq!(plan.character_id(), id);
    assert_eq!(plan.name(), "Caldari Carrier");
    assert_eq!(plan.sort_mode(), "manual");
    assert_eq!(plan.implant_set(), "current");
    assert_eq!(plan.created_at(), "2026-06-02T00:00:00Z");
    assert_eq!(plan.updated_at(), "2026-06-02T00:00:00Z");

    let entry = sqlx::query_as::<_, SkillPlanEntry>("SELECT * FROM skill_plan_entries WHERE id = ?")
      .bind(entry_id)
      .fetch_one(&db.0)
      .await
      .unwrap();
    assert_eq!(entry.id(), entry_id);
    assert_eq!(entry.plan_id(), plan_id);
    assert_eq!(entry.skill_id(), 3300);
    assert_eq!(entry.to_level(), 5);
    assert_eq!(entry.position(), 0);
    assert_eq!(entry.priority(), "normal");
    assert_eq!(entry.note(), "");
    assert_eq!(entry.is_auto(), 0);

    let remap = sqlx::query_as::<_, SkillPlanRemapPoint>("SELECT * FROM skill_plan_remap_points WHERE plan_id = ?")
      .bind(plan_id)
      .fetch_one(&db.0)
      .await
      .unwrap();
    assert_eq!(remap.plan_id(), plan_id);
    assert_eq!(remap.after_entry_id(), Some(entry_id));
    assert_eq!(remap.base_perception(), 21);
    assert_eq!(remap.base_memory(), 21);
    assert_eq!(remap.base_willpower(), 21);
    assert_eq!(remap.base_intelligence(), 19);
    assert_eq!(remap.base_charisma(), 17);

    assert_eq!(count(&db, PLANS_COUNT, id).await, 1);
    assert_eq!(count(&db, ENTRIES_COUNT, plan_id).await, 1);
    assert_eq!(count(&db, REMAP_POINTS_COUNT, plan_id).await, 1);

    character::delete(&db, id).await.unwrap();

    assert_eq!(count(&db, PLANS_COUNT, id).await, 0);
    assert_eq!(count(&db, ENTRIES_COUNT, plan_id).await, 0);
    assert_eq!(count(&db, REMAP_POINTS_COUNT, plan_id).await, 0);
  }
}

#[cfg(test)]
mod outbox_tests {
  use super::*;
  use crate::store;

  const SUBJECT: i64 = 95_465_499;

  async fn append_read(db: &Database, dedupe: Option<&str>) -> Outbox {
    append(
      db,
      OwnerType::Character,
      SUBJECT,
      "mail.set_read",
      "{\"mail_id\":1,\"is_read\":true}",
      dedupe,
    )
    .await
    .unwrap()
  }

  async fn reload(db: &Database, id: i64) -> Outbox {
    sqlx::query_as::<_, Outbox>(
      "SELECT attempts, created_at, dedupe_key, id, kind, last_error, next_attempt_at, payload, status, \
      subject_id, subject_type, updated_at FROM outbox WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&db.0)
    .await
    .unwrap()
  }

  fn future() -> String {
    "2999-01-01T00:00:00+00:00".to_string()
  }

  fn past() -> String {
    "2000-01-01T00:00:00+00:00".to_string()
  }

  mod append {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_collapses_a_redundant_dedupable_mutation_onto_the_existing_row() {
      let db = store::open_test().await.unwrap();
      let first = append_read(&db, Some("mail:1:read")).await;

      let second = append(
        &db,
        OwnerType::Character,
        SUBJECT,
        "mail.set_read",
        "{\"mail_id\":1,\"is_read\":false}",
        Some("mail:1:read"),
      )
      .await
      .unwrap();

      assert_eq!(second.id(), first.id());
      assert_eq!(second.payload(), "{\"mail_id\":1,\"is_read\":false}");
      assert_eq!(claim_due(&db, &future(), 10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_does_not_collapse_onto_a_done_row() {
      let db = store::open_test().await.unwrap();
      let first = append_read(&db, Some("mail:1:read")).await;
      mark_done(&db, first.id()).await.unwrap();

      let second = append_read(&db, Some("mail:1:read")).await;

      assert_ne!(second.id(), first.id());
    }

    #[tokio::test]
    async fn it_inserts_a_pending_row_drainable_now() {
      let db = store::open_test().await.unwrap();

      let row = append_read(&db, Some("mail:1:read")).await;

      assert_eq!(row.status(), "pending");
      assert_eq!(row.attempts(), 0);
      assert_eq!(row.subject_type(), OwnerType::Character);
      assert_eq!(row.subject_id(), SUBJECT);
      assert_eq!(row.kind(), "mail.set_read");
      assert_eq!(row.last_error(), &None);
      assert_eq!(row.created_at(), row.updated_at());
    }

    #[tokio::test]
    async fn it_never_collapses_rows_with_a_null_dedupe_key() {
      let db = store::open_test().await.unwrap();

      let first = append(
        &db,
        OwnerType::Character,
        SUBJECT,
        "mail.send",
        "{\"body\":\"a\"}",
        None,
      )
      .await
      .unwrap();
      let second = append(
        &db,
        OwnerType::Character,
        SUBJECT,
        "mail.send",
        "{\"body\":\"b\"}",
        None,
      )
      .await
      .unwrap();

      assert_ne!(first.id(), second.id());
      assert_eq!(claim_due(&db, &future(), 10).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn it_reasserts_drainability_when_collapsing_onto_a_failed_retry_row() {
      let db = store::open_test().await.unwrap();
      let first = append_read(&db, Some("mail:1:read")).await;
      reschedule(&db, first.id(), &future(), "boom").await.unwrap();

      let collapsed = append_read(&db, Some("mail:1:read")).await;

      assert_eq!(collapsed.id(), first.id());
      assert_eq!(collapsed.attempts(), 0);
      assert_eq!(collapsed.status(), "pending");
      assert_eq!(collapsed.last_error(), &None);
    }
  }

  mod claim_due {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_excludes_done_and_failed_rows() {
      let db = store::open_test().await.unwrap();
      let done = append(&db, OwnerType::Character, SUBJECT, "mail.send", "{}", None)
        .await
        .unwrap();
      let failed = append(&db, OwnerType::Character, SUBJECT, "mail.send", "{}", None)
        .await
        .unwrap();
      mark_done(&db, done.id()).await.unwrap();
      mark_failed(&db, failed.id(), "nope").await.unwrap();

      assert!(claim_due(&db, &future(), 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_flips_eligible_rows_to_inflight_in_created_at_order() {
      let db = store::open_test().await.unwrap();
      let first = append(&db, OwnerType::Character, SUBJECT, "mail.send", "{}", None)
        .await
        .unwrap();
      let second = append(&db, OwnerType::Character, SUBJECT, "mail.send", "{}", None)
        .await
        .unwrap();

      let claimed = claim_due(&db, &future(), 10).await.unwrap();

      assert_eq!(
        claimed.iter().map(|r| r.id()).collect::<Vec<_>>(),
        [first.id(), second.id()]
      );
      assert!(claimed.iter().all(|r| r.status() == "inflight"));
    }

    #[tokio::test]
    async fn it_honors_the_limit() {
      let db = store::open_test().await.unwrap();
      for _ in 0..3 {
        append(&db, OwnerType::Character, SUBJECT, "mail.send", "{}", None)
          .await
          .unwrap();
      }

      assert_eq!(claim_due(&db, &future(), 2).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn it_reclaims_a_stuck_inflight_row_whose_attempt_time_has_passed() {
      let db = store::open_test().await.unwrap();
      let row = append_read(&db, Some("mail:1:read")).await;
      claim_due(&db, &future(), 10).await.unwrap();

      let reclaimed = claim_due(&db, &future(), 10).await.unwrap();

      assert_eq!(reclaimed.iter().map(|r| r.id()).collect::<Vec<_>>(), [row.id()]);
    }

    #[tokio::test]
    async fn it_skips_rows_not_yet_due() {
      let db = store::open_test().await.unwrap();
      let row = append_read(&db, Some("mail:1:read")).await;
      reschedule(&db, row.id(), &future(), "later").await.unwrap();

      assert!(claim_due(&db, &past(), 10).await.unwrap().is_empty());
    }
  }

  mod outbox_failed_by_kind {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_failed_rows_matching_the_kind_prefix() {
      let db = store::open_test().await.unwrap();
      let mail_failed = append(&db, OwnerType::Character, SUBJECT, "mail.set_read", "{}", Some("a"))
        .await
        .unwrap();
      mark_failed(&db, mail_failed.id(), "boom").await.unwrap();
      let skill_failed = append(&db, OwnerType::Character, SUBJECT, "skill.queue", "{}", Some("b"))
        .await
        .unwrap();
      mark_failed(&db, skill_failed.id(), "nope").await.unwrap();
      append(&db, OwnerType::Character, SUBJECT, "mail.send", "{}", Some("c"))
        .await
        .unwrap();

      let rows = outbox_failed_by_kind(&db, "mail.").await.unwrap();

      assert_eq!(
        rows,
        vec![(mail_failed.id(), "mail.set_read".to_owned(), Some("boom".to_owned()))]
      );
    }
  }

  mod outbox_pending_count_by_kind {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_pending_and_inflight_rows_matching_the_prefix() {
      let db = store::open_test().await.unwrap();
      append(&db, OwnerType::Character, SUBJECT, "mail.send", "{}", Some("a"))
        .await
        .unwrap();
      append(&db, OwnerType::Character, SUBJECT, "mail.set_read", "{}", Some("b"))
        .await
        .unwrap();
      claim_due(&db, &future(), 10).await.unwrap();
      let failed = append(&db, OwnerType::Character, SUBJECT, "mail.delete", "{}", Some("c"))
        .await
        .unwrap();
      mark_failed(&db, failed.id(), "boom").await.unwrap();
      append(&db, OwnerType::Character, SUBJECT, "skill.queue", "{}", Some("d"))
        .await
        .unwrap();

      let count = outbox_pending_count_by_kind(&db, "mail.").await.unwrap();

      assert_eq!(count, 2);
    }
  }

  mod prune_done {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn count(db: &Database) -> i64 {
      sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox")
        .fetch_one(&db.0)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn it_keeps_done_rows_at_or_after_the_cutoff() {
      let db = store::open_test().await.unwrap();
      let row = append_read(&db, Some("mail:1:read")).await;
      mark_done(&db, row.id()).await.unwrap();

      let pruned = prune_done(&db, &past()).await.unwrap();

      assert_eq!(pruned, 0);
      assert_eq!(count(&db).await, 1);
    }

    #[tokio::test]
    async fn it_never_prunes_pending_inflight_or_failed_rows() {
      let db = store::open_test().await.unwrap();
      let inflight = append(&db, OwnerType::Character, SUBJECT, "mail.send", "{}", None)
        .await
        .unwrap();
      claim_due(&db, &future(), 10).await.unwrap();
      let pending = append(&db, OwnerType::Character, SUBJECT, "mail.send", "{}", None)
        .await
        .unwrap();
      let failed = append(&db, OwnerType::Character, SUBJECT, "mail.send", "{}", None)
        .await
        .unwrap();
      mark_failed(&db, failed.id(), "nope").await.unwrap();

      let pruned = prune_done(&db, &future()).await.unwrap();

      assert_eq!(pruned, 0);
      assert_eq!(count(&db).await, 3);
      assert_eq!(reload(&db, pending.id()).await.status(), "pending");
      assert_eq!(reload(&db, inflight.id()).await.status(), "inflight");
      assert_eq!(reload(&db, failed.id()).await.status(), "failed");
    }

    #[tokio::test]
    async fn it_removes_done_rows_older_than_the_cutoff() {
      let db = store::open_test().await.unwrap();
      let row = append_read(&db, Some("mail:1:read")).await;
      mark_done(&db, row.id()).await.unwrap();

      let pruned = prune_done(&db, &future()).await.unwrap();

      assert_eq!(pruned, 1);
      assert_eq!(count(&db).await, 0);
    }
  }

  mod transitions {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn mark_done_clears_the_error() {
      let db = store::open_test().await.unwrap();
      let row = append_read(&db, Some("mail:1:read")).await;
      mark_failed(&db, row.id(), "transient").await.unwrap();

      mark_done(&db, row.id()).await.unwrap();

      let after = reload(&db, row.id()).await;
      assert_eq!(after.status(), "done");
      assert_eq!(after.last_error(), &None);
    }

    #[tokio::test]
    async fn mark_failed_records_the_error() {
      let db = store::open_test().await.unwrap();
      let row = append_read(&db, Some("mail:1:read")).await;

      mark_failed(&db, row.id(), "403 forbidden").await.unwrap();

      let after = reload(&db, row.id()).await;
      assert_eq!(after.status(), "failed");
      assert_eq!(after.last_error().as_deref(), Some("403 forbidden"));
    }

    #[tokio::test]
    async fn reschedule_increments_attempts_and_keeps_the_row_drainable() {
      let db = store::open_test().await.unwrap();
      let row = append_read(&db, Some("mail:1:read")).await;

      reschedule(&db, row.id(), &past(), "rate limited").await.unwrap();

      let after = reload(&db, row.id()).await;
      assert_eq!(after.status(), "pending");
      assert_eq!(after.attempts(), 1);
      assert_eq!(after.next_attempt_at(), &past());
      assert_eq!(after.last_error().as_deref(), Some("rate limited"));
      assert_eq!(claim_due(&db, &future(), 10).await.unwrap().len(), 1);
    }
  }
}

#[cfg(test)]
mod search_helpers_tests {
  use super::*;

  mod escape_like {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_escapes_like_metacharacters_and_the_escape_char() {
      assert_eq!(escape_like("100%_x\\y"), "100\\%\\_x\\\\y");
    }

    #[test]
    fn it_leaves_plain_text_untouched() {
      assert_eq!(escape_like("Cobalt Edge"), "Cobalt Edge");
    }
  }

  mod like_pattern {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_wraps_the_escaped_value_as_a_substring_pattern() {
      assert_eq!(like_pattern("50%"), "%50\\%%");
    }
  }
}

#[cfg(test)]
mod tag_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, ENTITY_TYPE_CHARACTER, Gender, Race},
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

  mod scope {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{TAG_SCOPE_ASSET, TAG_SCOPE_ENTITY};

    #[tokio::test]
    async fn pre_existing_tags_backfill_to_the_entity_scope() {
      let db = store::open_test().await.unwrap();
      sqlx::query("INSERT INTO tags (color, created_at, description, name, position, updated_at) VALUES (NULL, 0, NULL, 'Legacy', 0, 0)")
        .execute(db.writer())
        .await
        .unwrap();

      let entity = tag_all_scoped(&db, TAG_SCOPE_ENTITY).await.unwrap();
      let asset = tag_all_scoped(&db, TAG_SCOPE_ASSET).await.unwrap();

      assert_eq!(entity.iter().map(|t| t.name()).collect::<Vec<_>>(), ["Legacy"]);
      assert!(asset.is_empty());
    }

    #[tokio::test]
    async fn listing_a_scope_excludes_the_other_scope() {
      let db = store::open_test().await.unwrap();
      create_scoped(&db, "Pilot", None, None, TAG_SCOPE_ENTITY).await.unwrap();
      create_scoped(&db, "Keep", None, None, TAG_SCOPE_ASSET).await.unwrap();

      assert_eq!(
        tag_all_scoped(&db, TAG_SCOPE_ENTITY)
          .await
          .unwrap()
          .iter()
          .map(|t| t.name())
          .collect::<Vec<_>>(),
        ["Pilot"]
      );
      assert_eq!(
        tag_all_scoped(&db, TAG_SCOPE_ASSET)
          .await
          .unwrap()
          .iter()
          .map(|t| t.name())
          .collect::<Vec<_>>(),
        ["Keep"]
      );
    }

    #[tokio::test]
    async fn the_entity_default_helpers_exclude_asset_tags() {
      let db = store::open_test().await.unwrap();
      create(&db, "Pilot", None, None).await.unwrap();
      create_scoped(&db, "Keep", None, None, TAG_SCOPE_ASSET).await.unwrap();

      assert_eq!(
        tag_all(&db).await.unwrap().iter().map(|t| t.name()).collect::<Vec<_>>(),
        ["Pilot"]
      );
    }

    #[tokio::test]
    async fn positions_number_independently_per_scope() {
      let db = store::open_test().await.unwrap();

      let entity_first = create_scoped(&db, "Pilot", None, None, TAG_SCOPE_ENTITY).await.unwrap();
      let asset_first = create_scoped(&db, "Keep", None, None, TAG_SCOPE_ASSET).await.unwrap();
      let entity_second = create_scoped(&db, "Hauler", None, None, TAG_SCOPE_ENTITY)
        .await
        .unwrap();
      let asset_second = create_scoped(&db, "Sell", None, None, TAG_SCOPE_ASSET).await.unwrap();

      assert_eq!(entity_first.position(), 0);
      assert_eq!(entity_second.position(), 1);
      assert_eq!(asset_first.position(), 0);
      assert_eq!(asset_second.position(), 1);
    }

    #[tokio::test]
    async fn the_seed_marker_round_trips_and_is_idempotent() {
      let db = store::open_test().await.unwrap();

      assert!(!is_tag_scope_seeded(&db, TAG_SCOPE_ASSET).await.unwrap());

      mark_tag_scope_seeded(&db, TAG_SCOPE_ASSET).await.unwrap();
      mark_tag_scope_seeded(&db, TAG_SCOPE_ASSET).await.unwrap();

      assert!(is_tag_scope_seeded(&db, TAG_SCOPE_ASSET).await.unwrap());
      assert!(!is_tag_scope_seeded(&db, TAG_SCOPE_ENTITY).await.unwrap());
    }
  }

  mod all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_tags_ordered_by_position() {
      let db = store::open_test().await.unwrap();
      create(&db, "First", None, None).await.unwrap();
      create(&db, "Second", None, None).await.unwrap();

      let tags = tag_all(&db).await.unwrap();

      assert_eq!(tags.iter().map(|t| t.name()).collect::<Vec<_>>(), ["First", "Second"]);
    }
  }

  mod assign {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_adds_a_tag_without_removing_existing_tags() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 12_345_678).await;
      let first = create(&db, "First", None, None).await.unwrap();
      let second = create(&db, "Second", None, None).await.unwrap();
      assign(&db, ENTITY_TYPE_CHARACTER, 12_345_678, first.id())
        .await
        .unwrap();

      assign(&db, ENTITY_TYPE_CHARACTER, 12_345_678, second.id())
        .await
        .unwrap();

      assert_eq!(
        members(&db, first.id(), ENTITY_TYPE_CHARACTER).await.unwrap(),
        vec![12_345_678]
      );
      assert_eq!(
        members(&db, second.id(), ENTITY_TYPE_CHARACTER).await.unwrap(),
        vec![12_345_678]
      );
      assert_eq!(memberships(&db, ENTITY_TYPE_CHARACTER).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn it_is_idempotent_when_reassigning_the_same_tag() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 12_345_678).await;
      let tag = create(&db, "Tag", None, None).await.unwrap();

      assign(&db, ENTITY_TYPE_CHARACTER, 12_345_678, tag.id()).await.unwrap();
      assign(&db, ENTITY_TYPE_CHARACTER, 12_345_678, tag.id()).await.unwrap();

      assert_eq!(memberships(&db, ENTITY_TYPE_CHARACTER).await.unwrap().len(), 1);
    }
  }

  mod create {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_appends_with_increasing_positions() {
      let db = store::open_test().await.unwrap();

      let first = create(&db, "First", Some("the first"), Some("#3FB8DB")).await.unwrap();
      let second = create(&db, "Second", None, None).await.unwrap();

      assert_eq!(first.position(), 0);
      assert_eq!(second.position(), 1);
      assert_eq!(first.description().as_deref(), Some("the first"));
      assert_eq!(first.color().as_deref(), Some("#3FB8DB"));
    }
  }

  mod delete {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_cascades_membership() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 12_345_678).await;
      let doomed = create(&db, "Doomed", None, None).await.unwrap();
      let kept = create(&db, "Kept", None, None).await.unwrap();
      assign(&db, ENTITY_TYPE_CHARACTER, 12_345_678, doomed.id())
        .await
        .unwrap();
      assign(&db, ENTITY_TYPE_CHARACTER, 12_345_678, kept.id()).await.unwrap();

      tag_delete(&db, doomed.id()).await.unwrap();

      assert!(tag_get(&db, doomed.id()).await.unwrap().is_none());
      assert_eq!(
        members(&db, kept.id(), ENTITY_TYPE_CHARACTER).await.unwrap(),
        vec![12_345_678]
      );
      assert_eq!(memberships(&db, ENTITY_TYPE_CHARACTER).await.unwrap().len(), 1);
    }
  }

  mod members {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_lists_every_character_bearing_the_tag() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 100).await;
      seed_character(&db, 200).await;
      let tag = create(&db, "Crew", None, None).await.unwrap();
      assign(&db, ENTITY_TYPE_CHARACTER, 200, tag.id()).await.unwrap();
      assign(&db, ENTITY_TYPE_CHARACTER, 100, tag.id()).await.unwrap();

      assert_eq!(
        members(&db, tag.id(), ENTITY_TYPE_CHARACTER).await.unwrap(),
        vec![100, 200]
      );
    }
  }

  mod membership_map {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{ENTITY_TYPE_ASSET, TAG_SCOPE_ASSET};

    #[tokio::test]
    async fn it_groups_tag_ids_by_asset_item_id_in_position_order() {
      let db = store::open_test().await.unwrap();
      let keep = create_scoped(&db, "Keep", None, None, TAG_SCOPE_ASSET).await.unwrap();
      let sell = create_scoped(&db, "Sell", None, None, TAG_SCOPE_ASSET).await.unwrap();
      // Assign out of position order to prove the map orders by tag position, not insert order.
      assign(&db, ENTITY_TYPE_ASSET, 1001, sell.id()).await.unwrap();
      assign(&db, ENTITY_TYPE_ASSET, 1001, keep.id()).await.unwrap();
      assign(&db, ENTITY_TYPE_ASSET, 2002, keep.id()).await.unwrap();

      let map = membership_map(&db, ENTITY_TYPE_ASSET).await.unwrap();

      assert_eq!(map.get(&1001), Some(&vec![keep.id(), sell.id()]));
      assert_eq!(map.get(&2002), Some(&vec![keep.id()]));
      assert_eq!(map.get(&3003), None);
    }

    #[tokio::test]
    async fn it_excludes_other_entity_types() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 100).await;
      let asset_tag = create_scoped(&db, "Keep", None, None, TAG_SCOPE_ASSET).await.unwrap();
      let char_tag = create(&db, "Crew", None, None).await.unwrap();
      assign(&db, ENTITY_TYPE_ASSET, 1001, asset_tag.id()).await.unwrap();
      assign(&db, ENTITY_TYPE_CHARACTER, 100, char_tag.id()).await.unwrap();

      let map = membership_map(&db, ENTITY_TYPE_ASSET).await.unwrap();

      assert_eq!(map.len(), 1);
      assert_eq!(map.get(&1001), Some(&vec![asset_tag.id()]));
    }

    #[tokio::test]
    async fn it_round_trips_an_assign_then_unassign_for_an_item_id() {
      let db = store::open_test().await.unwrap();
      let keep = create_scoped(&db, "Keep", None, None, TAG_SCOPE_ASSET).await.unwrap();

      assign(&db, ENTITY_TYPE_ASSET, 1001, keep.id()).await.unwrap();
      assert_eq!(
        membership_map(&db, ENTITY_TYPE_ASSET).await.unwrap().get(&1001),
        Some(&vec![keep.id()])
      );

      unassign(&db, ENTITY_TYPE_ASSET, 1001, keep.id()).await.unwrap();
      assert!(
        !membership_map(&db, ENTITY_TYPE_ASSET)
          .await
          .unwrap()
          .contains_key(&1001)
      );
    }
  }

  mod reorder {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_rewrites_positions_to_the_given_order() {
      let db = store::open_test().await.unwrap();
      let a = create(&db, "A", None, None).await.unwrap();
      let b = create(&db, "B", None, None).await.unwrap();
      let c = create(&db, "C", None, None).await.unwrap();

      reorder(&db, &[c.id(), a.id(), b.id()]).await.unwrap();

      assert_eq!(
        tag_all(&db).await.unwrap().iter().map(|t| t.name()).collect::<Vec<_>>(),
        ["C", "A", "B"]
      );
    }
  }

  mod unassign {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_removes_only_the_named_tag() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 12_345_678).await;
      let kept = create(&db, "Kept", None, None).await.unwrap();
      let dropped = create(&db, "Dropped", None, None).await.unwrap();
      assign(&db, ENTITY_TYPE_CHARACTER, 12_345_678, kept.id()).await.unwrap();
      assign(&db, ENTITY_TYPE_CHARACTER, 12_345_678, dropped.id())
        .await
        .unwrap();

      unassign(&db, ENTITY_TYPE_CHARACTER, 12_345_678, dropped.id())
        .await
        .unwrap();

      assert_eq!(
        members(&db, kept.id(), ENTITY_TYPE_CHARACTER).await.unwrap(),
        vec![12_345_678]
      );
      assert_eq!(
        members(&db, dropped.id(), ENTITY_TYPE_CHARACTER).await.unwrap(),
        Vec::<i64>::new()
      );
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_changes_name_description_and_color() {
      let db = store::open_test().await.unwrap();
      let tag = create(&db, "Old", None, None).await.unwrap();

      update(&db, tag.id(), "New", Some("renamed"), Some("#E07559"))
        .await
        .unwrap();

      let updated = tag_get(&db, tag.id()).await.unwrap().unwrap();
      assert_eq!(updated.name(), "New");
      assert_eq!(updated.description().as_deref(), Some("renamed"));
      assert_eq!(updated.color().as_deref(), Some("#E07559"));
    }
  }
}
