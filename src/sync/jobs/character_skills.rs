use std::collections::BTreeSet;

use chrono::{DateTime, Utc};

use crate::{
  clients::{Error, esi::models::universe::DogmaAttribute},
  store::{
    model::{CharacterAttributes, CharacterImplant, CharacterSkill, CharacterSkillqueue},
    repo::{character, skill_completion},
  },
  sync::{job::JobCtx, jobs::resolve, outcome::Outcome, subject::Subject},
};

const IMPLANT_BONUS_ATTR_BASE: i32 = 175;

const NEURAL_ATTRIBUTE_IDS: [i64; 5] = [164, 165, 166, 167, 168];

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character skills job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authenticated = ctx.esi.character_authenticated(grant);

  let sheet = authenticated.skills().await?;
  let queue_entries = authenticated.skill_queue().await?;
  let attributes = authenticated.attributes().await?;
  let implant_type_ids = authenticated.implants().await?;

  let unallocated_sp = sheet.unallocated_sp.unwrap_or(0);
  let skills: Vec<CharacterSkill> = sheet
    .skills
    .into_iter()
    .map(|skill| CharacterSkill::from((character_id, skill)))
    .collect();
  let queue: Vec<CharacterSkillqueue> = queue_entries
    .into_iter()
    .map(|entry| CharacterSkillqueue::from((character_id, entry)))
    .collect();
  let attributes = CharacterAttributes::from((character_id, attributes, unallocated_sp));

  let mut skill_ids: BTreeSet<i64> = skills.iter().map(|skill| skill.skill_id()).collect();
  skill_ids.extend(queue.iter().map(|entry| entry.skill_id()));
  for skill_id in skill_ids {
    resolve::resolve_item_type(ctx, skill_id).await?;
  }

  let implants = resolve_implant_bonuses(ctx, character_id, &implant_type_ids).await?;

  character::replace_skills(ctx.db, character_id, &skills).await?;
  character::replace_skillqueue(ctx.db, character_id, &queue).await?;
  character::upsert_attributes(ctx.db, &attributes).await?;
  character::replace_implants(ctx.db, character_id, &implants).await?;
  reconcile_skill_completions(ctx, character_id, &queue).await?;
  Ok(Outcome::from_rows(skills.len() + queue.len() + implants.len()))
}

async fn resolve_implant_bonuses(
  ctx: &JobCtx<'_>,
  character_id: i64,
  implant_type_ids: &[i64],
) -> Result<Vec<CharacterImplant>, Error> {
  let mut bonuses = [0_i64; 5];
  for &type_id in implant_type_ids {
    let lookup_id = i32::try_from(type_id)
      .map_err(|_| Error::Internal(format!("implant type id {type_id} out of range for ESI lookup")))?;
    let item_type = ctx.esi.universe().item_type(lookup_id).await?;
    for (index, bonus) in bonuses.iter_mut().enumerate() {
      *bonus += dogma_value(&item_type.dogma_attributes, IMPLANT_BONUS_ATTR_BASE + index as i32);
    }
  }

  Ok(
    NEURAL_ATTRIBUTE_IDS
      .into_iter()
      .zip(bonuses)
      .map(|(attribute_id, bonus)| CharacterImplant {
        attribute_id,
        bonus,
        character_id,
      })
      .collect(),
  )
}

fn dogma_value(dogma_attributes: &[DogmaAttribute], attribute_id: i32) -> i64 {
  dogma_attributes
    .iter()
    .find(|attr| attr.attribute_id == attribute_id)
    .map_or(0, |attr| attr.value.round() as i64)
}

fn queue_contradicts(queue: &[CharacterSkillqueue], skill_id: i64, level: i64, now: DateTime<Utc>) -> bool {
  queue.iter().any(|entry| {
    entry.skill_id() == skill_id
      && entry.finished_level() == level
      && pending_at_future_finish(entry.finish_date().as_deref(), now)
  })
}

/// True unless the finish date has already passed; a missing or unparseable date counts as
/// pending so it is never mistaken for a confirmed completion.
fn pending_at_future_finish(finish_date: Option<&str>, now: DateTime<Utc>) -> bool {
  match finish_date {
    None => true,
    Some(raw) => DateTime::parse_from_rfc3339(raw).map_or(true, |finish| finish.with_timezone(&Utc) > now),
  }
}

async fn reconcile_skill_completions(
  ctx: &JobCtx<'_>,
  character_id: i64,
  queue: &[CharacterSkillqueue],
) -> Result<(), Error> {
  let now = Utc::now();
  for completion in skill_completion::unverified(ctx.db, character_id).await? {
    if queue_contradicts(queue, completion.skill_id, completion.level, now) {
      skill_completion::delete(ctx.db, completion.id).await?;
    } else {
      skill_completion::mark_verified(ctx.db, completion.id).await?;
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, eve_image, eve_sso::Grant, http},
    store::{self, images, repo::skills},
    sync::job::{JobKey, JobKind},
  };

  async fn mount_json(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn seed_character(db: &store::Database, id: i64) {
    use store::{
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::character,
    };
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

  async fn mount_skills(server: &MockServer, character_id: i64) {
    mount_json(
      server,
      &format!("/characters/{character_id}/skills/"),
      serde_json::json!({
        "skills": [
          { "active_skill_level": 5, "skill_id": 3300, "skillpoints_in_skill": 256000, "trained_skill_level": 5 },
        ],
        "total_sp": 256000,
        "unallocated_sp": 15000,
      }),
    )
    .await;
  }

  async fn mount_skill_queue(server: &MockServer, character_id: i64) {
    mount_json(
      server,
      &format!("/characters/{character_id}/skillqueue/"),
      serde_json::json!([
        { "finish_date": "2026-06-01T00:00:00Z", "finished_level": 5, "queue_position": 0, "skill_id": 3300 },
      ]),
    )
    .await;
  }

  async fn mount_attributes(server: &MockServer, character_id: i64) {
    mount_json(
      server,
      &format!("/characters/{character_id}/attributes/"),
      serde_json::json!({
        "charisma": 20, "intelligence": 22, "memory": 21, "perception": 20, "willpower": 20,
        "bonus_remaps": 2, "last_remap_date": "2023-04-01T12:00:00Z",
        "accrued_remap_cooldown_date": "2024-04-01T12:00:00Z",
      }),
    )
    .await;
  }

  async fn mount_implants(server: &MockServer, character_id: i64) {
    mount_json(
      server,
      &format!("/characters/{character_id}/implants/"),
      serde_json::json!([9899]),
    )
    .await;
    mount_json(
      server,
      "/universe/types/9899/",
      serde_json::json!({
        "description": "A memory implant.", "group_id": 300, "name": "Memory Augmentation - Basic",
        "published": true, "type_id": 9899,
        "dogma_attributes": [
          { "attribute_id": 177, "value": 3.0 },
          { "attribute_id": 331, "value": 6.0 },
        ],
      }),
    )
    .await;
  }

  async fn mount_skill_type(server: &MockServer) {
    mount_json(
      server,
      "/universe/types/3300/",
      serde_json::json!({
        "description": "Gunnery.", "group_id": 255, "market_group_id": 1112, "name": "Gunnery",
        "published": true, "type_id": 3300,
        "dogma_attributes": [
          { "attribute_id": 275, "value": 1.0 },
          { "attribute_id": 180, "value": 167.0 },
          { "attribute_id": 181, "value": 168.0 },
        ],
      }),
    )
    .await;
  }

  async fn mount_market_groups(server: &MockServer) {
    mount_json(
      server,
      "/markets/groups/1112/",
      serde_json::json!({
        "description": "Skill books.", "market_group_id": 1112, "name": "Skills",
        "parent_group_id": 1111, "types": [3300],
      }),
    )
    .await;
    mount_json(
      server,
      "/markets/groups/1111/",
      serde_json::json!({
        "description": "All market groups.", "market_group_id": 1111, "name": "Market", "types": [],
      }),
    )
    .await;
  }

  async fn mount_skill_group_and_category(server: &MockServer) {
    mount_json(
      server,
      "/universe/groups/255/",
      serde_json::json!({ "category_id": 16, "group_id": 255, "name": "Gunnery", "published": true, "types": [3300] }),
    )
    .await;
    mount_json(
      server,
      "/universe/categories/16/",
      serde_json::json!({ "category_id": 16, "groups": [255], "name": "Skill", "published": true }),
    )
    .await;
  }

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: &'a Grant,
    character_id: i64,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::CharacterSkills, Subject::Character(character_id)),
      grant: Some(grant),
      sso: None,
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_aborts_without_writing_when_a_skill_is_missing_a_dogma_attribute() {
      let server = MockServer::start().await;
      mount_skills(&server, 42).await;
      mount_skill_queue(&server, 42).await;
      mount_attributes(&server, 42).await;
      mount_implants(&server, 42).await;
      mount_json(
        &server,
        "/universe/types/3300/",
        serde_json::json!({
          "description": "Gunnery.", "group_id": 255, "market_group_id": 1112, "name": "Gunnery",
          "published": true, "type_id": 3300,
          "dogma_attributes": [
            { "attribute_id": 275, "value": 1.0 },
            { "attribute_id": 180, "value": 167.0 },
          ],
        }),
      )
      .await;
      mount_skill_group_and_category(&server).await;
      mount_market_groups(&server).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(character::skills(&db, 42, chrono::Utc::now()).await.unwrap().is_empty());
      assert!(character::skillqueue(&db, 42).await.unwrap().is_empty());
      assert_eq!(character::attributes(&db, 42).await.unwrap(), None);
      assert!(character::implants(&db, 42).await.unwrap().is_empty());
      assert_eq!(skills::get_skill_metadata(&db, 3300).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_aborts_without_writing_when_the_attributes_fetch_fails() {
      let server = MockServer::start().await;
      mount_skills(&server, 42).await;
      mount_skill_queue(&server, 42).await;
      Mock::given(method("GET"))
        .and(path("/characters/42/attributes/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      mount_implants(&server, 42).await;
      mount_skill_type(&server).await;
      mount_skill_group_and_category(&server).await;
      mount_market_groups(&server).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(character::skills(&db, 42, chrono::Utc::now()).await.unwrap().is_empty());
      assert!(character::skillqueue(&db, 42).await.unwrap().is_empty());
      assert_eq!(character::attributes(&db, 42).await.unwrap(), None);
      assert!(character::implants(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_assembles_and_persists_the_full_skill_picture() {
      let server = MockServer::start().await;
      mount_skills(&server, 42).await;
      mount_skill_queue(&server, 42).await;
      mount_attributes(&server, 42).await;
      mount_implants(&server, 42).await;
      mount_skill_type(&server).await;
      mount_skill_group_and_category(&server).await;
      mount_market_groups(&server).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      assert_eq!(character::skills(&db, 42, chrono::Utc::now()).await.unwrap().len(), 1);
      assert_eq!(character::skillqueue(&db, 42).await.unwrap().len(), 1);

      let attributes = character::attributes(&db, 42).await.unwrap().unwrap();
      assert_eq!(attributes.unallocated_sp(), 15_000);
      assert_eq!(attributes.intelligence(), 22);

      let implants = character::implants(&db, 42).await.unwrap();
      assert_eq!(implants.len(), 5);
      let memory_bonus = implants.iter().find(|i| i.attribute_id() == 166).unwrap();
      assert_eq!(memory_bonus.bonus(), 3);
      assert!(
        implants
          .iter()
          .filter(|i| i.attribute_id() != 166)
          .all(|i| i.bonus() == 0)
      );

      assert!(skills::get_skill_metadata(&db, 3300).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_defaults_unallocated_sp_to_zero_when_absent() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/characters/42/skills/",
        serde_json::json!({ "skills": [], "total_sp": 0 }),
      )
      .await;
      mount_json(&server, "/characters/42/skillqueue/", serde_json::json!([])).await;
      mount_attributes(&server, 42).await;
      mount_json(&server, "/characters/42/implants/", serde_json::json!([])).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      let attributes = character::attributes(&db, 42).await.unwrap().unwrap();
      assert_eq!(attributes.unallocated_sp(), 0);

      let implants = character::implants(&db, 42).await.unwrap();
      assert_eq!(implants.len(), 5);
      assert!(implants.iter().all(|i| i.bonus() == 0));
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/skills/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "skills": [], "total_sp": 0 })))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
      assert!(character::skills(&db, 42, chrono::Utc::now()).await.unwrap().is_empty());
    }
  }

  mod reconcile_skill_completions {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{clients::esi::models::character::SkillQueueEntry, store::repo::skill_completion};

    fn queue_entry(skill_id: i64, finished_level: i64, finish_date: Option<&str>) -> CharacterSkillqueue {
      CharacterSkillqueue::from((
        42,
        SkillQueueEntry {
          finish_date: finish_date.map(str::to_owned),
          finished_level: i32::try_from(finished_level).unwrap(),
          level_end_sp: None,
          level_start_sp: None,
          queue_position: 0,
          skill_id: i32::try_from(skill_id).unwrap(),
          start_date: None,
          training_start_sp: None,
        },
      ))
    }

    async fn reconcile(db: &store::Database, character_id: i64, queue: &[CharacterSkillqueue]) {
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), "http://localhost".to_owned());
      let image = eve_image::Client::with_base_url(http, "http://localhost".to_owned());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", character_id);
      let ctx = ctx_with_grant(db, &esi, &image, &image_store, &grant, character_id);

      super::super::reconcile_skill_completions(&ctx, character_id, queue)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_removes_a_completion_the_fresh_queue_still_shows_pending() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      skill_completion::insert_if_absent(&db, 42, 3300, 5, "2026-07-06T08:00:00+00:00")
        .await
        .unwrap();
      let queue = vec![queue_entry(3300, 5, Some("2099-01-01T00:00:00Z"))];

      reconcile(&db, 42, &queue).await;

      assert!(
        skill_completion::unverified(&db, 42).await.unwrap().is_empty(),
        "a completion the queue still trains to a future finish is a false positive and removed"
      );
    }

    #[tokio::test]
    async fn it_verifies_a_completion_the_queue_no_longer_contradicts() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      skill_completion::insert_if_absent(&db, 42, 3300, 5, "2026-07-06T08:00:00+00:00")
        .await
        .unwrap();
      let queue = vec![queue_entry(3301, 4, Some("2099-01-01T00:00:00Z"))];

      reconcile(&db, 42, &queue).await;

      let rows = skill_completion::for_day(&db, "2026-07-06", &[42]).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert!(
        rows[0].verified,
        "a completion absent from the pending queue is marked verified"
      );
      assert!(
        skill_completion::unverified(&db, 42).await.unwrap().is_empty(),
        "the verified row drops out of the unverified set"
      );
    }

    #[tokio::test]
    async fn it_is_a_no_op_for_a_character_with_no_unverified_rows() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let queue = vec![queue_entry(3300, 5, Some("2099-01-01T00:00:00Z"))];

      reconcile(&db, 42, &queue).await;

      assert!(
        skill_completion::for_day(&db, "2026-07-06", &[42])
          .await
          .unwrap()
          .is_empty()
      );
    }

    #[tokio::test]
    async fn it_leaves_already_verified_rows_untouched() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      skill_completion::insert_if_absent(&db, 42, 3300, 5, "2026-07-06T08:00:00+00:00")
        .await
        .unwrap();
      let verified = skill_completion::unverified(&db, 42).await.unwrap();
      skill_completion::mark_verified(&db, verified[0].id).await.unwrap();
      let queue = vec![queue_entry(3300, 5, Some("2099-01-01T00:00:00Z"))];

      reconcile(&db, 42, &queue).await;

      let rows = skill_completion::for_day(&db, "2026-07-06", &[42]).await.unwrap();
      assert_eq!(rows.len(), 1, "the reconcile never revisits an already-verified row");
      assert!(rows[0].verified);
    }
  }
}
