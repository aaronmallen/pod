use std::{collections::HashSet, path::Path, sync::Arc};

use iced::Task;

use crate::{clients::eve_image, store::images::ImageKind};

pub type Key = (ImageKind, i64);

pub fn dispatch_fetches<Message: Send + 'static>(
  pending: &mut HashSet<Key>,
  client: &Arc<eve_image::Client>,
  keys: Vec<Key>,
  to_message: impl Fn(Key, bool) -> Message + Copy + Send + 'static,
) -> Task<Message> {
  let mut tasks = Vec::new();
  for (kind, id) in keys {
    if !pending.insert((kind, id)) {
      continue;
    }
    let client = Arc::clone(client);
    tasks.push(Task::perform(
      async move { ensure(client, kind, id).await },
      move |ready| to_message((kind, id), ready),
    ));
  }
  Task::batch(tasks)
}

pub async fn ensure(client: Arc<eve_image::Client>, kind: ImageKind, id: i64) -> bool {
  use crate::store::images;

  let store = images::default_store();
  let path = store.image_path(kind, id);
  if images::is_fresh(&path, images::STALE_AFTER) {
    return true;
  }

  match client.fetch(&url(&client, kind, id)).await {
    Ok(bytes) => write_refetched(&store, &path, &bytes, kind, id),
    Err(error) => {
      tracing::warn!(target: "pod::images", %error, ?kind, id, "refetching an evicted image failed");
      false
    }
  }
}

fn url(client: &eve_image::Client, kind: ImageKind, id: i64) -> String {
  use crate::store::images;

  match kind {
    ImageKind::AllianceLogo => client.alliance_logo_url(id, images::LOGO_SIZE),
    ImageKind::CharacterPortrait => client.character_portrait_url(id, images::PORTRAIT_SIZE),
    ImageKind::CorporationLogo => client.corporation_logo_url(id, images::LOGO_SIZE),
  }
}

fn write_refetched(store: &crate::store::images::Store, path: &Path, bytes: &[u8], kind: ImageKind, id: i64) -> bool {
  match store.write(path, bytes) {
    Ok(()) => true,
    Err(error) => {
      tracing::warn!(target: "pod::images", %error, ?kind, id, "writing a refetched image failed");
      false
    }
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::*;
  use crate::{clients::http, store};

  async fn client() -> Arc<eve_image::Client> {
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db)).build();
    Arc::new(eve_image::Client::new(http))
  }

  mod dispatch_fetches {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_claims_each_key_in_flight_exactly_once() {
      let mut pending = HashSet::new();
      let client = client().await;

      let _task = dispatch_fetches(
        &mut pending,
        &client,
        vec![(ImageKind::CharacterPortrait, 42), (ImageKind::CharacterPortrait, 42)],
        |key, ready| (key, ready),
      );

      assert!(pending.contains(&(ImageKind::CharacterPortrait, 42)));
      assert_eq!(pending.len(), 1);
    }

    #[tokio::test]
    async fn it_does_not_redispatch_a_key_already_in_flight() {
      let mut pending = HashSet::from([(ImageKind::CorporationLogo, 7)]);
      let client = client().await;

      let _task = dispatch_fetches(
        &mut pending,
        &client,
        vec![(ImageKind::CorporationLogo, 7)],
        |key, ready| (key, ready),
      );

      assert_eq!(pending.len(), 1);
    }
  }

  mod url {
    use super::*;

    #[tokio::test]
    async fn it_builds_a_url_for_every_image_kind() {
      let client = client().await;

      assert!(!super::url(&client, ImageKind::AllianceLogo, 1).is_empty());
      assert!(!super::url(&client, ImageKind::CharacterPortrait, 1).is_empty());
      assert!(!super::url(&client, ImageKind::CorporationLogo, 1).is_empty());
    }
  }
}
