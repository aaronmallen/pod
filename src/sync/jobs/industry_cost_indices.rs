use crate::{
  clients::Error,
  store::{model::IndustryCostIndex, repo::industry},
  sync::{job::JobCtx, outcome::Outcome},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let indices: Vec<IndustryCostIndex> = ctx
    .esi
    .industry()
    .system_cost_indices()
    .await?
    .into_iter()
    .map(|system| {
      let mut index = IndustryCostIndex {
        solar_system_id: system.solar_system_id,
        ..IndustryCostIndex::default()
      };
      for entry in system.cost_indices {
        index.set_activity(&entry.activity, entry.cost_index);
      }
      index
    })
    .collect();
  industry::replace_cost_indices(ctx.db, &indices).await?;
  Ok(Outcome::from_rows(indices.len()))
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, eve_image, http},
    store::{self, images, repo::industry},
    sync::{
      job::{JobKey, JobKind},
      subject::Subject,
    },
  };

  async fn mount_systems(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path("/industry/systems/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  fn ctx<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::IndustryCostIndices, Subject::Character(0)),
      grant: None,
      sso: None,
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_errors_and_persists_nothing_when_the_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/industry/systems/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(
        industry::cost_indices_for_system(&db, 30_000_142)
          .await
          .unwrap()
          .is_none()
      );
    }

    #[tokio::test]
    async fn it_preserves_existing_indices_when_the_response_is_an_empty_set() {
      let server = MockServer::start().await;
      mount_systems(&server, serde_json::json!([])).await;
      let db = store::open_test().await.unwrap();
      industry::replace_cost_indices(
        &db,
        &[IndustryCostIndex {
          manufacturing: Some(0.07),
          solar_system_id: 30_000_142,
          ..IndustryCostIndex::default()
        }],
      )
      .await
      .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);

      run(&ctx).await.unwrap();

      assert_eq!(industry::cost_index_for(&db, 30_000_142, 1).await.unwrap(), Some(0.07));
    }

    #[tokio::test]
    async fn it_upserts_each_systems_indices_without_a_grant() {
      let server = MockServer::start().await;
      mount_systems(
        &server,
        serde_json::json!([
          {
            "solar_system_id": 30000142,
            "cost_indices": [
              { "activity": "manufacturing", "cost_index": 0.05 },
              { "activity": "reaction", "cost_index": 0.01 },
            ],
          },
          {
            "solar_system_id": 30002187,
            "cost_indices": [{ "activity": "copying", "cost_index": 0.02 }],
          },
        ]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);

      run(&ctx).await.unwrap();

      let row = industry::cost_indices_for_system(&db, 30_000_142)
        .await
        .unwrap()
        .unwrap();
      assert_eq!(row.manufacturing(), Some(0.05));
      assert_eq!(row.reaction(), Some(0.01));
      assert_eq!(row.copying(), None);
      assert_eq!(industry::cost_index_for(&db, 30_002_187, 5).await.unwrap(), Some(0.02));
    }

    #[tokio::test]
    async fn it_wholesale_replaces_systems_on_a_re_run() {
      let server = MockServer::start().await;
      mount_systems(
        &server,
        serde_json::json!([
          { "solar_system_id": 30002187, "cost_indices": [{ "activity": "manufacturing", "cost_index": 0.09 }] },
        ]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      industry::replace_cost_indices(
        &db,
        &[IndustryCostIndex {
          manufacturing: Some(0.01),
          solar_system_id: 30_000_142,
          ..IndustryCostIndex::default()
        }],
      )
      .await
      .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);

      run(&ctx).await.unwrap();

      assert!(
        industry::cost_indices_for_system(&db, 30_000_142)
          .await
          .unwrap()
          .is_none()
      );
      assert_eq!(industry::cost_index_for(&db, 30_002_187, 1).await.unwrap(), Some(0.09));
    }
  }
}
