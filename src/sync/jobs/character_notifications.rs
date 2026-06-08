use chrono::Utc;

use crate::{
  clients::Error,
  store::{model::CharacterNotification, repo::character},
  sync::{job::JobCtx, outcome::Outcome, subject::Subject},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character notifications job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Ok(Outcome::NotReady);
  }

  let notifications = ctx.esi.character_authenticated(grant).notifications().await?;
  let count = notifications.len();
  let synced_at = Utc::now().to_rfc3339();

  for notification in notifications {
    let row = CharacterNotification {
      character_id,
      is_read: notification.is_read.unwrap_or(false),
      notif_type: notification.notif_type,
      notification_id: notification.notification_id,
      sender_id: notification.sender_id,
      sender_type: notification.sender_type,
      synced_at: synced_at.clone(),
      text: notification.text,
      timestamp: notification.timestamp,
    };
    character::upsert_notification(ctx.db, &row).await?;
  }

  Ok(Outcome::from_rows(count))
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
    store::{self, images},
    sync::job::{JobKey, JobKind},
  };

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
      key: JobKey::new(JobKind::CharacterNotifications, Subject::Character(character_id)),
      grant: Some(grant),
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_the_notification_feed() {
      let server = MockServer::start().await;
      let body = serde_json::json!([
        {
          "notification_id": 7, "type": "KillReportFinalBlow", "sender_id": 1001, "sender_type": "character",
          "timestamp": "2024-01-01T00:00:00Z", "is_read": true, "text": "body"
        },
        {
          "notification_id": 8, "type": "StructureUnderAttack", "sender_id": 2002, "sender_type": "corporation",
          "timestamp": "2024-02-01T00:00:00Z"
        }
      ]);
      Mock::given(method("GET"))
        .and(path("/characters/42/notifications/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
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

      let rows = character::notifications(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 2);
      let first = rows.iter().find(|n| n.notification_id() == 7).unwrap();
      assert_eq!(first.notif_type(), "KillReportFinalBlow");
      assert!(first.is_read());
      let second = rows.iter().find(|n| n.notification_id() == 8).unwrap();
      assert!(!second.is_read());
      assert!(second.text().is_none());
    }

    #[tokio::test]
    async fn it_aborts_without_writing_when_the_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/notifications/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
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
      assert!(character::notifications(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/notifications/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
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

      run(&ctx).await.unwrap();

      assert!(character::notifications(&db, 42).await.unwrap().is_empty());
    }
  }
}
