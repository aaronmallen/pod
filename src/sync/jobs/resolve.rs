use crate::{
  clients::{Error, esi::models::universe::DogmaAttribute},
  store::{
    model::{MarketGroup, SkillMetadata},
    repo::{sde, skills},
  },
  sync::job::JobCtx,
};

const SKILL_CATEGORY_ID: i32 = 16;

const SKILL_PRIMARY_ATTR_ID: i32 = 180;
const SKILL_RANK_ATTR_ID: i32 = 275;
const SKILL_SECONDARY_ATTR_ID: i32 = 181;

pub async fn resolve_item_type(ctx: &JobCtx<'_>, type_id: i64) -> Result<(), Error> {
  if sde::get_item_type(ctx.db, type_id).await?.is_some() {
    tracing::trace!(item_type_id = type_id, "resolved item type from db");
    return Ok(());
  }
  let lookup_id = i32::try_from(type_id)
    .map_err(|_| Error::Internal(format!("item type id {type_id} out of range for ESI lookup")))?;
  tracing::debug!(item_type_id = type_id, "fetching item type from esi");
  let item_type = ctx.esi.universe().item_type(lookup_id).await?;
  if let Some(market_group_id) = item_type.market_group_id {
    resolve_market_group(ctx, i64::from(market_group_id)).await?;
  }
  let group = ctx.esi.universe().item_group(item_type.group_id).await?;
  let category = ctx.esi.universe().item_category(group.category_id).await?;

  let metadata = if category.category_id == SKILL_CATEGORY_ID {
    Some(extract_skill_metadata(type_id, &item_type.dogma_attributes)?)
  } else {
    None
  };

  sde::insert_item_type_with_hierarchy(ctx.db, &item_type.into(), &group.into(), &category.into()).await?;
  if let Some(metadata) = metadata {
    skills::upsert_skill_metadata(ctx.db, &metadata).await?;
  }
  Ok(())
}

pub async fn resolve_market_group(ctx: &JobCtx<'_>, market_group_id: i64) -> Result<(), Error> {
  let mut pending: Vec<MarketGroup> = Vec::new();
  let mut next = Some(market_group_id);

  while let Some(id) = next {
    if sde::get_market_group(ctx.db, id).await?.is_some() {
      tracing::trace!(market_group_id = id, "resolved market group from db");
      break;
    }
    let lookup_id =
      i32::try_from(id).map_err(|_| Error::Internal(format!("market group id {id} out of range for ESI lookup")))?;
    tracing::debug!(market_group_id = id, "fetching market group from esi");
    let group = MarketGroup::from(ctx.esi.universe().market_group(lookup_id).await?);
    next = group.parent_id();
    pending.push(group);
  }

  for group in pending.into_iter().rev() {
    sde::upsert_market_group(ctx.db, &group).await?;
  }
  Ok(())
}

fn extract_skill_metadata(skill_id: i64, dogma_attributes: &[DogmaAttribute]) -> Result<SkillMetadata, Error> {
  let attr = |attribute_id: i32| {
    dogma_attributes
      .iter()
      .find(|attr| attr.attribute_id == attribute_id)
      .map(|attr| attr.value.round() as i64)
      .ok_or_else(|| Error::Internal(format!("skill type {skill_id} missing dogma attribute {attribute_id}")))
  };

  Ok(SkillMetadata {
    primary_attribute: attr(SKILL_PRIMARY_ATTR_ID)?,
    rank: attr(SKILL_RANK_ATTR_ID)?,
    secondary_attribute: attr(SKILL_SECONDARY_ATTR_ID)?,
    skill_id,
  })
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
    store::{
      self, images,
      repo::{sde, skills},
    },
    sync::{
      job::{JobKey, JobKind},
      subject::Subject,
    },
  };

  async fn mount_json(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
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

  mod resolve_item_type {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_skill_metadata_and_dogma_for_a_skill_type() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/universe/types/3300/",
        serde_json::json!({
          "description": "Gunnery.", "group_id": 255, "name": "Gunnery", "published": true, "type_id": 3300,
          "dogma_attributes": [
            { "attribute_id": 275, "value": 1.0 },
            { "attribute_id": 180, "value": 167.0 },
            { "attribute_id": 181, "value": 168.0 },
          ],
        }),
      )
      .await;
      mount_skill_group_and_category(&server).await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = build_ctx(&db, &esi, &image, &image_store);

      resolve_item_type(&ctx, 3300).await.unwrap();

      let metadata = skills::get_skill_metadata(&db, 3300).await.unwrap().unwrap();
      assert_eq!(metadata.rank(), 1);
      assert_eq!(metadata.primary_attribute(), 167);
      assert_eq!(metadata.secondary_attribute(), 168);

      let item_type = sde::get_item_type(&db, 3300).await.unwrap().unwrap();
      let dogma: serde_json::Value = serde_json::from_str(item_type.dogma_attributes()).unwrap();
      assert_eq!(dogma.as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn it_does_not_write_skill_metadata_for_a_non_skill_type() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/universe/types/34/",
        serde_json::json!({
          "description": "Tritanium.", "group_id": 18, "name": "Tritanium", "published": true, "type_id": 34,
        }),
      )
      .await;
      mount_json(
        &server,
        "/universe/groups/18/",
        serde_json::json!({ "category_id": 4, "group_id": 18, "name": "Mineral", "published": true, "types": [34] }),
      )
      .await;
      mount_json(
        &server,
        "/universe/categories/4/",
        serde_json::json!({ "category_id": 4, "groups": [18], "name": "Material", "published": true }),
      )
      .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = build_ctx(&db, &esi, &image, &image_store);

      resolve_item_type(&ctx, 34).await.unwrap();

      assert_eq!(skills::get_skill_metadata(&db, 34).await.unwrap(), None);
      assert!(sde::get_item_type(&db, 34).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_aborts_without_writing_when_a_skill_is_missing_a_dogma_attribute() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/universe/types/3300/",
        serde_json::json!({
          "description": "Gunnery.", "group_id": 255, "name": "Gunnery", "published": true, "type_id": 3300,
          "dogma_attributes": [
            { "attribute_id": 275, "value": 1.0 },
            { "attribute_id": 180, "value": 167.0 },
          ],
        }),
      )
      .await;
      mount_skill_group_and_category(&server).await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = build_ctx(&db, &esi, &image, &image_store);

      let result = resolve_item_type(&ctx, 3300).await;

      assert!(result.is_err());
      assert_eq!(skills::get_skill_metadata(&db, 3300).await.unwrap(), None);
    }
  }
}
