use std::collections::HashMap;

use crate::{
  clients::{Error, esi::models::universe::NameRecord},
  sync::job::JobCtx,
};

const MAX_IDS_PER_REQUEST: usize = 1000;

pub async fn resolve_names(ctx: &JobCtx<'_>, ids: &[i64]) -> Result<HashMap<i64, NameRecord>, Error> {
  let mut unique: Vec<i64> = ids.to_vec();
  unique.sort_unstable();
  unique.dedup();

  let mut resolved = HashMap::with_capacity(unique.len());
  for chunk in unique.chunks(MAX_IDS_PER_REQUEST) {
    resolve_chunk(ctx, chunk, &mut resolved).await?;
  }
  Ok(resolved)
}

async fn resolve_chunk(ctx: &JobCtx<'_>, chunk: &[i64], resolved: &mut HashMap<i64, NameRecord>) -> Result<(), Error> {
  if chunk.is_empty() {
    return Ok(());
  }
  match ctx.esi.universe().names(chunk).await {
    Ok(records) => {
      resolved.extend(records.into_iter().map(|record| (record.id, record)));
      Ok(())
    }
    Err(Error::Http(error)) if error.status() == Some(reqwest::StatusCode::NOT_FOUND) => {
      if chunk.len() == 1 {
        tracing::debug!(id = chunk[0], "name unresolvable (404); omitting from results");
        return Ok(());
      }
      let mid = chunk.len() / 2;
      Box::pin(resolve_chunk(ctx, &chunk[..mid], resolved)).await?;
      Box::pin(resolve_chunk(ctx, &chunk[mid..], resolved)).await?;
      Ok(())
    }
    Err(error) => Err(error),
  }
}

#[cfg(test)]
mod tests {
  use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  };

  use wiremock::{
    Mock, MockServer, Request, Respond, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, eve_image, http},
    store::{self, images},
    sync::{
      job::{JobKey, JobKind},
      subject::Subject,
    },
  };

  fn build_ctx<'a>(
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
      key: JobKey::new(JobKind::CharacterWallet, Subject::Character(42)),
      grant: None,
    }
  }

  async fn make_esi(base_url: &str, db: &store::Database) -> esi::Client {
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    esi::Client::with_base_url(http, base_url)
  }

  fn name_record(category: &str, id: i64, name: &str) -> serde_json::Value {
    serde_json::json!({ "category": category, "id": id, "name": name })
  }

  mod resolve_names {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_resolves_ids_to_name_records() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/v3/universe/names/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          name_record("character", 95465499, "CCP Bartender"),
          name_record("corporation", 98356193, "Test Corp"),
        ])))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let esi = make_esi(&server.uri(), &db).await;
      let image = eve_image::Client::with_base_url(
        http::Client::builder(http::Cache::new(db.clone())).build(),
        server.uri(),
      );
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = build_ctx(&db, &esi, &image, &image_store);

      let resolved = resolve_names(&ctx, &[95465499, 98356193]).await.unwrap();

      assert_eq!(resolved.len(), 2);
      assert_eq!(resolved[&95465499].name, "CCP Bartender");
      assert_eq!(resolved[&95465499].category, "character");
      assert_eq!(resolved[&98356193].name, "Test Corp");
    }

    #[tokio::test]
    async fn it_chunks_requests_within_the_1000_id_cap() {
      let server = MockServer::start().await;
      let request_count = Arc::new(AtomicUsize::new(0));
      let max_batch = Arc::new(AtomicUsize::new(0));

      struct CountingResponder {
        request_count: Arc<AtomicUsize>,
        max_batch: Arc<AtomicUsize>,
      }
      impl Respond for CountingResponder {
        fn respond(&self, request: &Request) -> ResponseTemplate {
          self.request_count.fetch_add(1, Ordering::SeqCst);
          let ids: Vec<i64> = serde_json::from_slice(&request.body).unwrap();
          self.max_batch.fetch_max(ids.len(), Ordering::SeqCst);
          let body: Vec<serde_json::Value> = ids
            .iter()
            .map(|id| name_record("character", *id, &format!("Pilot {id}")))
            .collect();
          ResponseTemplate::new(200).set_body_json(body)
        }
      }
      Mock::given(method("POST"))
        .and(path("/v3/universe/names/"))
        .respond_with(CountingResponder {
          request_count: request_count.clone(),
          max_batch: max_batch.clone(),
        })
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let esi = make_esi(&server.uri(), &db).await;
      let image = eve_image::Client::with_base_url(
        http::Client::builder(http::Cache::new(db.clone())).build(),
        server.uri(),
      );
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = build_ctx(&db, &esi, &image, &image_store);
      let ids: Vec<i64> = (1..=2500).collect();

      let resolved = resolve_names(&ctx, &ids).await.unwrap();

      assert_eq!(resolved.len(), 2500);
      assert_eq!(request_count.load(Ordering::SeqCst), 3);
      assert!(max_batch.load(Ordering::SeqCst) <= MAX_IDS_PER_REQUEST);
    }

    #[tokio::test]
    async fn it_deduplicates_ids_before_requesting() {
      let server = MockServer::start().await;
      let seen_ids = Arc::new(std::sync::Mutex::new(Vec::<i64>::new()));

      struct CapturingResponder {
        seen_ids: Arc<std::sync::Mutex<Vec<i64>>>,
      }
      impl Respond for CapturingResponder {
        fn respond(&self, request: &Request) -> ResponseTemplate {
          let ids: Vec<i64> = serde_json::from_slice(&request.body).unwrap();
          self.seen_ids.lock().unwrap().extend(ids.iter().copied());
          let body: Vec<serde_json::Value> = ids
            .iter()
            .map(|id| name_record("character", *id, &format!("Pilot {id}")))
            .collect();
          ResponseTemplate::new(200).set_body_json(body)
        }
      }
      Mock::given(method("POST"))
        .and(path("/v3/universe/names/"))
        .respond_with(CapturingResponder {
          seen_ids: seen_ids.clone(),
        })
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let esi = make_esi(&server.uri(), &db).await;
      let image = eve_image::Client::with_base_url(
        http::Client::builder(http::Cache::new(db.clone())).build(),
        server.uri(),
      );
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = build_ctx(&db, &esi, &image, &image_store);

      let resolved = resolve_names(&ctx, &[7, 7, 7, 9]).await.unwrap();

      assert_eq!(resolved.len(), 2);
      let mut sent = seen_ids.lock().unwrap().clone();
      sent.sort_unstable();
      assert_eq!(sent, vec![7, 9]);
    }

    #[tokio::test]
    async fn it_partitions_unresolvable_ids_out_of_a_batch() {
      let server = MockServer::start().await;
      struct PartitioningResponder;
      impl Respond for PartitioningResponder {
        fn respond(&self, request: &Request) -> ResponseTemplate {
          let ids: Vec<i64> = serde_json::from_slice(&request.body).unwrap();
          if ids.contains(&999) {
            return ResponseTemplate::new(404);
          }
          let body: Vec<serde_json::Value> = ids
            .iter()
            .map(|id| name_record("character", *id, &format!("Pilot {id}")))
            .collect();
          ResponseTemplate::new(200).set_body_json(body)
        }
      }
      Mock::given(method("POST"))
        .and(path("/v3/universe/names/"))
        .respond_with(PartitioningResponder)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let esi = make_esi(&server.uri(), &db).await;
      let image = eve_image::Client::with_base_url(
        http::Client::builder(http::Cache::new(db.clone())).build(),
        server.uri(),
      );
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = build_ctx(&db, &esi, &image, &image_store);

      let resolved = resolve_names(&ctx, &[1, 2, 999, 3]).await.unwrap();

      assert_eq!(resolved.len(), 3);
      assert!(resolved.contains_key(&1));
      assert!(resolved.contains_key(&2));
      assert!(resolved.contains_key(&3));
      assert!(!resolved.contains_key(&999));
    }

    #[tokio::test]
    async fn it_returns_err_on_a_non_404_esi_error() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/v3/universe/names/"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let esi = make_esi(&server.uri(), &db).await;
      let image = eve_image::Client::with_base_url(
        http::Client::builder(http::Cache::new(db.clone())).build(),
        server.uri(),
      );
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = build_ctx(&db, &esi, &image, &image_store);

      let result = resolve_names(&ctx, &[1, 2, 3]).await;

      assert!(matches!(result, Err(Error::Http(_))));
    }

    #[tokio::test]
    async fn it_returns_empty_for_no_ids() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();
      let esi = make_esi(&server.uri(), &db).await;
      let image = eve_image::Client::with_base_url(
        http::Client::builder(http::Cache::new(db.clone())).build(),
        server.uri(),
      );
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = build_ctx(&db, &esi, &image, &image_store);

      let resolved = resolve_names(&ctx, &[]).await.unwrap();

      assert!(resolved.is_empty());
    }
  }
}
