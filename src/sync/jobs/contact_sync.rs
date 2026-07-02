use std::{
  collections::{BTreeMap, HashMap, HashSet},
  hash::{DefaultHasher, Hash, Hasher},
};

use crate::{
  clients::{Error, esi::scopes},
  features::roster::needs_reauthorization,
  store::{
    Database,
    model::{CharacterContact, OwnerType, SyncListContact, SyncPushedContact},
    repo::{character, contact_sync, infra},
  },
  sync::{job::JobCtx, outcome::Outcome, subject::Subject},
};

const ADD_BATCH_MAX: usize = 100;

const CONTACT_CEILING: usize = 1000;

const OUTBOX_KINDS_PENDING: [&str; 4] = ["contact.add", "contact.edit", "contact.remove", "contact.sync_add"];

type EntityKey = (String, i64);

struct AddBatch {
  entities: Vec<EntityKey>,
  standing: i64,
}

struct EditOp {
  previous: CharacterContact,
  standing: i64,
}

struct Plan {
  adds: Vec<AddBatch>,
  edits: Vec<EditOp>,
  removes: Vec<RemoveOp>,
}

struct RemoveOp {
  entity_id: i64,
  entity_type: String,
  previous: Option<CharacterContact>,
  pushed_standing: i64,
}

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "contact sync job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let desired = desired_set(&contact_sync::contacts_for_target(ctx.db, character_id).await?);
  let pushed = contact_sync::pushed_contacts(ctx.db, character_id).await?;
  if desired.is_empty() && pushed.is_empty() {
    return Ok(Outcome::Empty);
  }
  let granted = grant.scopes().join(" ");
  if needs_reauthorization(Some(&granted), &[scopes::CHARACTER_CONTACTS_WRITE]) {
    return Ok(Outcome::Blocked {
      reason: "write-contacts scope not granted; needs re-authorization".to_string(),
    });
  }
  let mirror = character::contacts(ctx.db, character_id).await?.contacts;
  let protected = pending_contact_ids(ctx.db, character_id).await?;
  let plan = plan(&desired, &mirror, &pushed, &protected);
  let enqueued = enqueue_plan(ctx.db, character_id, &plan).await?;
  Ok(Outcome::from_rows(enqueued))
}

fn add_dedupe_key(batch: &AddBatch) -> String {
  let mut hasher = DefaultHasher::new();
  batch.standing.hash(&mut hasher);
  for (entity_type, entity_id) in &batch.entities {
    entity_type.hash(&mut hasher);
    entity_id.hash(&mut hasher);
  }
  format!("contact-sync:add:{:016x}", hasher.finish())
}

fn batch_adds(adds: Vec<(EntityKey, i64)>, capacity: usize) -> Vec<AddBatch> {
  let mut tiers: BTreeMap<i64, Vec<EntityKey>> = BTreeMap::new();
  for (key, standing) in adds.into_iter().take(capacity) {
    tiers.entry(standing).or_default().push(key);
  }
  let mut batches = Vec::new();
  for (standing, entities) in tiers {
    for chunk in entities.chunks(ADD_BATCH_MAX) {
      batches.push(AddBatch {
        entities: chunk.to_vec(),
        standing,
      });
    }
  }
  batches
}

fn contact_state_json(row: &CharacterContact) -> serde_json::Value {
  let label_ids: Vec<i64> = serde_json::from_str(row.label_ids()).unwrap_or_default();
  serde_json::json!({
    "contact_id": row.contact_id(),
    "contact_name": row.contact_name(),
    "contact_type": row.contact_type(),
    "is_blocked": row.is_blocked(),
    "label_ids": label_ids,
    "standing": row.standing(),
    "watched": row.is_watched(),
  })
}

fn desired_set(contacts: &[SyncListContact]) -> HashMap<EntityKey, i64> {
  let mut desired: HashMap<EntityKey, i64> = HashMap::new();
  for contact in contacts {
    desired
      .entry((contact.entity_type().clone(), contact.entity_id()))
      .and_modify(|standing| *standing = (*standing).min(contact.standing()))
      .or_insert_with(|| contact.standing());
  }
  desired
}

fn edit_payload(character_id: i64, edit: &EditOp) -> String {
  let mut target = contact_state_json(&edit.previous);
  target["standing"] = serde_json::json!(edit.standing as f64);
  serde_json::json!({
    "character_id": character_id,
    "previous": contact_state_json(&edit.previous),
    "target": target,
  })
  .to_string()
}

async fn enqueue_adds(db: &Database, character_id: i64, batches: &[AddBatch]) -> Result<usize, Error> {
  for batch in batches {
    let payload = sync_add_payload(character_id, batch);
    let key = add_dedupe_key(batch);
    infra::append(
      db,
      OwnerType::Character,
      character_id,
      "contact.sync_add",
      &payload,
      Some(&key),
    )
    .await?;
    for (entity_type, entity_id) in &batch.entities {
      contact_sync::record_pushed(db, character_id, entity_type, *entity_id, batch.standing).await?;
    }
  }
  Ok(batches.len())
}

async fn enqueue_edits(db: &Database, character_id: i64, edits: &[EditOp]) -> Result<usize, Error> {
  for edit in edits {
    let payload = edit_payload(character_id, edit);
    let key = format!("contact-sync:edit:{}", edit.previous.contact_id());
    infra::append(
      db,
      OwnerType::Character,
      character_id,
      "contact.edit",
      &payload,
      Some(&key),
    )
    .await?;
    contact_sync::record_pushed(
      db,
      character_id,
      edit.previous.contact_type(),
      edit.previous.contact_id(),
      edit.standing,
    )
    .await?;
  }
  Ok(edits.len())
}

async fn enqueue_plan(db: &Database, character_id: i64, plan: &Plan) -> Result<usize, Error> {
  let mut rows = 0;
  rows += enqueue_adds(db, character_id, &plan.adds).await?;
  rows += enqueue_edits(db, character_id, &plan.edits).await?;
  rows += enqueue_removes(db, character_id, &plan.removes).await?;
  Ok(rows)
}

async fn enqueue_removes(db: &Database, character_id: i64, removes: &[RemoveOp]) -> Result<usize, Error> {
  for remove in removes {
    let payload = remove_payload(character_id, remove);
    let key = format!("contact-sync:remove:{}", remove.entity_id);
    infra::append(
      db,
      OwnerType::Character,
      character_id,
      "contact.remove",
      &payload,
      Some(&key),
    )
    .await?;
    contact_sync::delete_pushed(db, character_id, &remove.entity_type, remove.entity_id).await?;
  }
  Ok(removes.len())
}

async fn pending_contact_ids(db: &Database, character_id: i64) -> Result<HashSet<i64>, Error> {
  let mut ids = HashSet::new();
  for kind in OUTBOX_KINDS_PENDING {
    for payload in infra::outbox_pending_payloads(db, OwnerType::Character, character_id, kind).await? {
      ids.extend(payload_contact_ids(&payload));
    }
  }
  Ok(ids)
}

fn payload_contact_ids(payload: &str) -> Vec<i64> {
  let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
    return Vec::new();
  };
  let mut ids = Vec::new();
  for key in ["previous", "target"] {
    if let Some(id) = value
      .get(key)
      .and_then(|entry| entry.get("contact_id"))
      .and_then(serde_json::Value::as_i64)
    {
      ids.push(id);
    }
  }
  if let Some(entries) = value.get("contacts").and_then(serde_json::Value::as_array) {
    ids.extend(
      entries
        .iter()
        .filter_map(|entry| entry.get("contact_id").and_then(serde_json::Value::as_i64)),
    );
  }
  ids
}

fn plan(
  desired: &HashMap<EntityKey, i64>,
  mirror: &[CharacterContact],
  pushed: &[SyncPushedContact],
  protected: &HashSet<i64>,
) -> Plan {
  let mirror_index: HashMap<EntityKey, &CharacterContact> = mirror
    .iter()
    .map(|row| ((row.contact_type().clone(), row.contact_id()), row))
    .collect();
  let pushed_index: HashMap<EntityKey, i64> = pushed
    .iter()
    .map(|row| ((row.entity_type().clone(), row.entity_id()), row.pushed_standing()))
    .collect();
  let capacity = CONTACT_CEILING.saturating_sub(mirror.len());
  Plan {
    adds: batch_adds(plan_adds(desired, &mirror_index, &pushed_index, protected), capacity),
    edits: plan_edits(desired, &mirror_index, &pushed_index, protected),
    removes: plan_removes(desired, &mirror_index, pushed, protected),
  }
}

fn plan_adds(
  desired: &HashMap<EntityKey, i64>,
  mirror_index: &HashMap<EntityKey, &CharacterContact>,
  pushed_index: &HashMap<EntityKey, i64>,
  protected: &HashSet<i64>,
) -> Vec<(EntityKey, i64)> {
  let mut adds: Vec<(EntityKey, i64)> = desired
    .iter()
    .filter(|(key, standing)| {
      !mirror_index.contains_key(*key) && !protected.contains(&key.1) && pushed_index.get(*key) != Some(*standing)
    })
    .map(|(key, standing)| (key.clone(), *standing))
    .collect();
  adds.sort();
  adds
}

fn plan_edits(
  desired: &HashMap<EntityKey, i64>,
  mirror_index: &HashMap<EntityKey, &CharacterContact>,
  pushed_index: &HashMap<EntityKey, i64>,
  protected: &HashSet<i64>,
) -> Vec<EditOp> {
  let mut edits: Vec<EditOp> = desired
    .iter()
    .filter_map(|(key, standing)| {
      let row = mirror_index.get(key)?;
      if row.standing() == *standing as f64 || protected.contains(&key.1) || pushed_index.get(key) == Some(standing) {
        return None;
      }
      Some(EditOp {
        previous: (*row).clone(),
        standing: *standing,
      })
    })
    .collect();
  edits.sort_by_key(|edit| edit.previous.contact_id());
  edits
}

fn plan_removes(
  desired: &HashMap<EntityKey, i64>,
  mirror_index: &HashMap<EntityKey, &CharacterContact>,
  pushed: &[SyncPushedContact],
  protected: &HashSet<i64>,
) -> Vec<RemoveOp> {
  let mut removes: Vec<RemoveOp> = pushed
    .iter()
    .filter(|row| {
      !desired.contains_key(&(row.entity_type().clone(), row.entity_id())) && !protected.contains(&row.entity_id())
    })
    .map(|row| RemoveOp {
      entity_id: row.entity_id(),
      entity_type: row.entity_type().clone(),
      previous: mirror_index
        .get(&(row.entity_type().clone(), row.entity_id()))
        .map(|found| (*found).clone()),
      pushed_standing: row.pushed_standing(),
    })
    .collect();
  removes.sort_by_key(|remove| remove.entity_id);
  removes
}

fn remove_payload(character_id: i64, remove: &RemoveOp) -> String {
  let previous = match &remove.previous {
    Some(row) => contact_state_json(row),
    None => serde_json::json!({
      "contact_id": remove.entity_id,
      "contact_name": format!("Unknown ({})", remove.entity_id),
      "contact_type": remove.entity_type,
      "is_blocked": false,
      "label_ids": [],
      "standing": remove.pushed_standing as f64,
      "watched": false,
    }),
  };
  serde_json::json!({
    "character_id": character_id,
    "previous": previous,
  })
  .to_string()
}

fn sync_add_payload(character_id: i64, batch: &AddBatch) -> String {
  let contacts: Vec<serde_json::Value> = batch
    .entities
    .iter()
    .map(|(entity_type, entity_id)| {
      serde_json::json!({
        "contact_id": entity_id,
        "contact_type": entity_type,
      })
    })
    .collect();
  serde_json::json!({
    "character_id": character_id,
    "contacts": contacts,
    "standing": batch.standing as f64,
  })
  .to_string()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    clients::{esi, eve_image, eve_sso::Grant, http},
    store::{
      self,
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    },
    sync::job::{JobKey, JobKind},
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

  async fn seed_mirror_contact(db: &Database, character_id: i64, contact_id: i64, standing: f64, label_ids: &str) {
    character::upsert_contact(
      db,
      &CharacterContact {
        character_id,
        contact_id,
        contact_name: format!("Contact {contact_id}"),
        contact_type: "character".to_string(),
        is_blocked: false,
        is_watched: true,
        label_ids: label_ids.to_string(),
        standing,
      },
    )
    .await
    .unwrap();
  }

  async fn seed_targeted_list(db: &Database, character_id: i64, entities: &[(i64, i64)]) -> i64 {
    let list = contact_sync::create_list(db, "List").await.unwrap();
    for (entity_id, standing) in entities {
      contact_sync::add_contact(db, list.id(), "character", *entity_id, *standing)
        .await
        .unwrap();
    }
    contact_sync::set_targets(db, list.id(), &[character_id]).await.unwrap();
    list.id()
  }

  async fn outbox_payloads(db: &Database, kind: &str) -> Vec<String> {
    sqlx::query_scalar::<_, String>("SELECT payload FROM outbox WHERE kind = ? ORDER BY id")
      .bind(kind)
      .fetch_all(&db.0)
      .await
      .unwrap()
  }

  async fn outbox_count(db: &Database) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox")
      .fetch_one(&db.0)
      .await
      .unwrap()
  }

  fn write_grant(character_id: i64) -> Grant {
    Grant::new_test_with_scopes(
      "token",
      character_id,
      vec![scopes::CHARACTER_CONTACTS_WRITE.to_string()],
    )
  }

  struct Harness {
    db: Database,
    esi: esi::Client,
    image: eve_image::Client,
    image_store: store::images::Store,
    _images_dir: tempfile::TempDir,
  }

  impl Harness {
    async fn new() -> Self {
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), "http://127.0.0.1:9".to_string());
      let image = eve_image::Client::with_base_url(http, "http://127.0.0.1:9".to_string());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = store::images::Store::new(images_dir.path().to_path_buf());
      Self {
        db,
        esi,
        image,
        image_store,
        _images_dir: images_dir,
      }
    }

    fn ctx<'a>(&'a self, grant: &'a Grant, character_id: i64) -> JobCtx<'a> {
      JobCtx {
        db: &self.db,
        esi: &self.esi,
        grant: Some(grant),
        image: &self.image,
        image_store: &self.image_store,
        key: JobKey::new(JobKind::CharacterContactSync, Subject::Character(character_id)),
        sso: None,
      }
    }
  }

  mod batch_adds {
    use pretty_assertions::assert_eq;

    use super::*;

    fn entities(count: usize, standing: i64) -> Vec<(EntityKey, i64)> {
      (0..count)
        .map(|index| (("character".to_string(), 1000 + index as i64), standing))
        .collect()
    }

    #[test]
    fn it_chunks_a_tier_at_one_hundred_ids() {
      let batches = batch_adds(entities(150, -10), usize::MAX);

      assert_eq!(batches.len(), 2);
      assert_eq!(batches[0].entities.len(), 100);
      assert_eq!(batches[1].entities.len(), 50);
    }

    #[test]
    fn it_groups_batches_by_standing_tier() {
      let mut adds = entities(2, -10);
      adds.extend(entities(1, 5));

      let batches = batch_adds(adds, usize::MAX);

      assert_eq!(batches.len(), 2);
      assert_eq!(batches[0].standing, -10);
      assert_eq!(batches[1].standing, 5);
    }

    #[test]
    fn it_truncates_at_the_remaining_contact_capacity() {
      let batches = batch_adds(entities(3, -10), 1);

      assert_eq!(batches.len(), 1);
      assert_eq!(batches[0].entities.len(), 1);
    }
  }

  mod desired_set {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_takes_the_most_hostile_standing_across_lists() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_targeted_list(&db, 42, &[(1001, -5)]).await;
      seed_targeted_list(&db, 42, &[(1001, -10)]).await;
      let contacts = contact_sync::contacts_for_target(&db, 42).await.unwrap();

      let desired = desired_set(&contacts);

      assert_eq!(desired.get(&("character".to_string(), 1001)), Some(&-10));
    }

    #[tokio::test]
    async fn it_unions_distinct_entities() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_targeted_list(&db, 42, &[(1001, -10), (1002, 5)]).await;
      let contacts = contact_sync::contacts_for_target(&db, 42).await.unwrap();

      let desired = desired_set(&contacts);

      assert_eq!(desired.len(), 2);
      assert_eq!(desired.get(&("character".to_string(), 1002)), Some(&5));
    }
  }

  mod payload_contact_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reads_batched_contacts_entries() {
      let ids = payload_contact_ids(r#"{"contacts":[{"contact_id":1},{"contact_id":2}],"standing":-10.0}"#);

      assert_eq!(ids, [1, 2]);
    }

    #[test]
    fn it_reads_target_and_previous_ids() {
      let ids = payload_contact_ids(r#"{"previous":{"contact_id":7},"target":{"contact_id":8}}"#);

      assert_eq!(ids, [7, 8]);
    }

    #[test]
    fn it_returns_nothing_for_garbage() {
      assert!(payload_contact_ids("not json").is_empty());
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_blocks_without_enqueueing_when_the_write_scope_is_missing() {
      let harness = Harness::new().await;
      seed_character(&harness.db, 42).await;
      seed_targeted_list(&harness.db, 42, &[(1001, -10)]).await;
      let grant = Grant::new_test("token", 42);

      let outcome = run(&harness.ctx(&grant, 42)).await.unwrap();

      assert!(
        matches!(outcome, Outcome::Blocked { .. }),
        "a missing write-contacts grant surfaces as blocked, got {outcome:?}"
      );
      assert_eq!(outbox_count(&harness.db).await, 0);
    }

    #[tokio::test]
    async fn it_does_not_re_add_a_pushed_entity_while_the_mirror_is_stale() {
      let harness = Harness::new().await;
      seed_character(&harness.db, 42).await;
      seed_targeted_list(&harness.db, 42, &[(1001, -10)]).await;
      contact_sync::record_pushed(&harness.db, 42, "character", 1001, -10)
        .await
        .unwrap();
      let grant = write_grant(42);

      let outcome = run(&harness.ctx(&grant, 42)).await.unwrap();

      assert_eq!(outcome, Outcome::Empty);
      assert_eq!(outbox_count(&harness.db).await, 0);
    }

    #[tokio::test]
    async fn it_edits_only_the_standing_and_preserves_labels_and_watch() {
      let harness = Harness::new().await;
      seed_character(&harness.db, 42).await;
      seed_targeted_list(&harness.db, 42, &[(1001, -10)]).await;
      seed_mirror_contact(&harness.db, 42, 1001, 0.0, "[1,2]").await;
      let grant = write_grant(42);

      run(&harness.ctx(&grant, 42)).await.unwrap();

      let payloads = outbox_payloads(&harness.db, "contact.edit").await;
      assert_eq!(payloads.len(), 1);
      let payload: serde_json::Value = serde_json::from_str(&payloads[0]).unwrap();
      assert_eq!(payload["target"]["standing"], serde_json::json!(-10.0));
      assert_eq!(payload["target"]["label_ids"], serde_json::json!([1, 2]));
      assert_eq!(payload["target"]["watched"], serde_json::json!(true));
      assert_eq!(payload["previous"]["standing"], serde_json::json!(0.0));

      let pushed = contact_sync::pushed_contacts(&harness.db, 42).await.unwrap();
      assert_eq!(pushed.len(), 1);
      assert_eq!(pushed[0].pushed_standing(), -10);
    }

    #[tokio::test]
    async fn it_enqueues_batched_adds_by_standing_tier_and_records_provenance() {
      let harness = Harness::new().await;
      seed_character(&harness.db, 42).await;
      seed_targeted_list(&harness.db, 42, &[(1001, -10), (1002, -10), (2001, 5)]).await;
      let grant = write_grant(42);

      let outcome = run(&harness.ctx(&grant, 42)).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 2
        }
      );
      let payloads = outbox_payloads(&harness.db, "contact.sync_add").await;
      assert_eq!(payloads.len(), 2, "one batch per standing tier");
      let hostile: serde_json::Value = serde_json::from_str(&payloads[0]).unwrap();
      assert_eq!(hostile["standing"], serde_json::json!(-10.0));
      assert_eq!(hostile["contacts"].as_array().unwrap().len(), 2);

      let pushed = contact_sync::pushed_contacts(&harness.db, 42).await.unwrap();
      assert_eq!(pushed.len(), 3, "every enqueued entity is recorded as pushed");
    }

    #[tokio::test]
    async fn it_is_empty_when_nothing_targets_the_character() {
      let harness = Harness::new().await;
      seed_character(&harness.db, 42).await;
      let grant = write_grant(42);

      let outcome = run(&harness.ctx(&grant, 42)).await.unwrap();

      assert_eq!(outcome, Outcome::Empty);
    }

    #[tokio::test]
    async fn it_is_idempotent_across_back_to_back_runs() {
      let harness = Harness::new().await;
      seed_character(&harness.db, 42).await;
      seed_targeted_list(&harness.db, 42, &[(1001, -10), (2001, 5)]).await;
      seed_mirror_contact(&harness.db, 42, 3001, 0.0, "[]").await;
      let grant = write_grant(42);

      run(&harness.ctx(&grant, 42)).await.unwrap();
      let after_first = outbox_count(&harness.db).await;
      let second = run(&harness.ctx(&grant, 42)).await.unwrap();

      assert_eq!(outbox_count(&harness.db).await, after_first, "no duplicate outbox rows");
      assert_eq!(second, Outcome::Empty, "a steady-state run enqueues nothing");
    }

    #[tokio::test]
    async fn it_removes_a_pushed_contact_missing_from_the_mirror_with_a_synthesized_previous() {
      let harness = Harness::new().await;
      seed_character(&harness.db, 42).await;
      contact_sync::record_pushed(&harness.db, 42, "character", 5001, -10)
        .await
        .unwrap();
      let grant = write_grant(42);

      run(&harness.ctx(&grant, 42)).await.unwrap();

      let payloads = outbox_payloads(&harness.db, "contact.remove").await;
      assert_eq!(payloads.len(), 1);
      let payload: serde_json::Value = serde_json::from_str(&payloads[0]).unwrap();
      assert_eq!(payload["previous"]["contact_id"], serde_json::json!(5001));
      assert_eq!(payload["previous"]["standing"], serde_json::json!(-10.0));
      assert!(contact_sync::pushed_contacts(&harness.db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_removes_only_contacts_pod_pushed() {
      let harness = Harness::new().await;
      seed_character(&harness.db, 42).await;
      seed_mirror_contact(&harness.db, 42, 5001, -10.0, "[]").await;
      seed_mirror_contact(&harness.db, 42, 6001, 5.0, "[]").await;
      contact_sync::record_pushed(&harness.db, 42, "character", 5001, -10)
        .await
        .unwrap();
      let grant = write_grant(42);

      run(&harness.ctx(&grant, 42)).await.unwrap();

      let payloads = outbox_payloads(&harness.db, "contact.remove").await;
      assert_eq!(payloads.len(), 1, "the manually-added contact is left untouched");
      let payload: serde_json::Value = serde_json::from_str(&payloads[0]).unwrap();
      assert_eq!(payload["previous"]["contact_id"], serde_json::json!(5001));
      assert!(contact_sync::pushed_contacts(&harness.db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_returns_not_ready_when_the_character_is_not_persisted() {
      let harness = Harness::new().await;
      let grant = write_grant(42);

      let result = run(&harness.ctx(&grant, 42)).await;

      assert!(matches!(result, Err(Error::NotReady)));
    }

    #[tokio::test]
    async fn it_skips_entities_with_a_pending_outbox_mutation() {
      let harness = Harness::new().await;
      seed_character(&harness.db, 42).await;
      seed_targeted_list(&harness.db, 42, &[(1001, -10)]).await;
      infra::append(
        &harness.db,
        OwnerType::Character,
        42,
        "contact.edit",
        "{\"character_id\":42,\"target\":{\"contact_id\":1001}}",
        None,
      )
      .await
      .unwrap();
      let grant = write_grant(42);

      let outcome = run(&harness.ctx(&grant, 42)).await.unwrap();

      assert_eq!(outcome, Outcome::Empty);
      assert!(outbox_payloads(&harness.db, "contact.sync_add").await.is_empty());
    }
  }
}
