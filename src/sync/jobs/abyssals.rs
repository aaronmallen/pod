use std::collections::{HashMap, HashSet};

use crate::{
  clients::{Error, eve_image::Size, muta_market},
  store::{
    model::{AbyssalItem, CorporationAbyssalItem},
    repo::{assets, character, org},
  },
  sync::{job::JobCtx, outcome::Outcome, subject::Subject},
};

const ICON_SIZE: Size = Size::S64;

const PRICE_TTL: i64 = 24 * 3600;

const SYNC_TTL: i64 = 12 * 3600;

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  run_with_muta(ctx, &muta_market::Client::new()).await
}

async fn run_with_muta(ctx: &JobCtx<'_>, muta: &muta_market::Client) -> Result<Outcome, Error> {
  match ctx.key.subject {
    Subject::Character(character_id) => run_character(ctx, muta, character_id).await,
    Subject::Corporation(corporation_id) => run_corporation(ctx, muta, corporation_id).await,
  }
}

async fn run_character(ctx: &JobCtx<'_>, muta: &muta_market::Client, character_id: i64) -> Result<Outcome, Error> {
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }

  let assets = assets::for_character(ctx.db, character_id).await?;
  let singletons: Vec<(i64, i64)> = assets
    .iter()
    .filter(|asset| asset.is_singleton())
    .map(|asset| (asset.type_id(), asset.item_id()))
    .collect();
  if singletons.is_empty() {
    assets::delete_stale(ctx.db, character_id, &[]).await?;
    return Ok(Outcome::Empty);
  }

  let Some(catalog) = catalog(ctx).await? else {
    return Ok(blocked());
  };

  let pairs: Vec<(i64, i64)> = singletons
    .into_iter()
    .filter(|(type_id, _)| catalog.contains(type_id))
    .collect();
  let keep_ids: Vec<i64> = pairs.iter().map(|(_, item_id)| *item_id).collect();

  let now = now_unix();
  sync_dogma(ctx, character_id, &pairs, now).await?;
  assets::delete_stale(ctx.db, character_id, &keep_ids).await?;
  refresh_prices(ctx, character_id, muta, now).await?;

  Ok(Outcome::from_rows(pairs.len()))
}

async fn run_corporation(ctx: &JobCtx<'_>, muta: &muta_market::Client, corporation_id: i64) -> Result<Outcome, Error> {
  if org::get_corporation(ctx.db, corporation_id).await?.is_none() {
    return Err(Error::NotReady);
  }

  let assets = assets::for_corporation(ctx.db, corporation_id).await?;
  let singletons: Vec<(i64, i64)> = assets
    .iter()
    .filter(|asset| asset.is_singleton())
    .map(|asset| (asset.type_id(), asset.item_id()))
    .collect();
  if singletons.is_empty() {
    assets::delete_stale_corporation(ctx.db, corporation_id, &[]).await?;
    return Ok(Outcome::Empty);
  }

  let Some(catalog) = catalog(ctx).await? else {
    return Ok(blocked());
  };

  let pairs: Vec<(i64, i64)> = singletons
    .into_iter()
    .filter(|(type_id, _)| catalog.contains(type_id))
    .collect();
  let keep_ids: Vec<i64> = pairs.iter().map(|(_, item_id)| *item_id).collect();

  let now = now_unix();
  sync_dogma_corporation(ctx, corporation_id, &pairs, now).await?;
  assets::delete_stale_corporation(ctx.db, corporation_id, &keep_ids).await?;
  refresh_prices_corporation(ctx, corporation_id, muta, now).await?;

  Ok(Outcome::from_rows(pairs.len()))
}

async fn catalog(ctx: &JobCtx<'_>) -> Result<Option<HashSet<i64>>, Error> {
  let catalog: HashSet<i64> = assets::abyssal_type_ids(ctx.db).await?.into_iter().collect();
  Ok((!catalog.is_empty()).then_some(catalog))
}

fn blocked() -> Outcome {
  Outcome::Blocked {
    reason: "abyssal type catalog is not seeded".to_string(),
  }
}

async fn cache_icon(ctx: &JobCtx<'_>, type_id: i64) {
  let icon_path = ctx.image_store.type_icon_path(type_id, ICON_SIZE);
  if icon_path.exists() {
    return;
  }
  let icon_url = ctx.image.type_icon_url(type_id, ICON_SIZE);
  let icon_bytes = match ctx.image.fetch(&icon_url).await {
    Ok(bytes) => bytes,
    Err(error) => {
      tracing::warn!(type_id, "abyssals: icon fetch failed; continuing: {error}");
      return;
    }
  };
  if let Err(error) = ctx.image_store.write(&icon_path, &icon_bytes) {
    tracing::warn!(type_id, "abyssals: icon write failed; continuing: {error}");
  }
}

fn now_unix() -> i64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map_or(0, |d| d.as_secs() as i64)
}

async fn refresh_prices(
  ctx: &JobCtx<'_>,
  character_id: i64,
  muta: &muta_market::Client,
  now: i64,
) -> Result<(), Error> {
  let items = assets::for_character_abyssal(ctx.db, character_id).await?;
  let stale_before = now - PRICE_TTL;

  for item in items {
    if item.muta_price_synced().unwrap_or(0) >= stale_before {
      continue;
    }
    match muta.item_price(item.item_id()).await {
      Ok(price) => assets::update_price(ctx.db, item.item_id(), price, now).await?,
      Err(error) => tracing::warn!(
        character_id,
        item_id = item.item_id(),
        "abyssals: MutaMarket price fetch failed: {error}"
      ),
    }
  }
  Ok(())
}

async fn sync_dogma(ctx: &JobCtx<'_>, character_id: i64, pairs: &[(i64, i64)], now: i64) -> Result<(), Error> {
  let existing: HashMap<i64, i64> = assets::for_character_abyssal(ctx.db, character_id)
    .await?
    .into_iter()
    .map(|row| (row.item_id(), row.synced_at()))
    .collect();
  let stale_before = now - SYNC_TTL;

  for &(type_id, item_id) in pairs {
    let last_synced = existing.get(&item_id).copied().unwrap_or(0);
    if last_synced >= stale_before {
      continue;
    }
    if let Err(error) = sync_one(ctx, character_id, type_id, item_id, now).await {
      tracing::warn!(character_id, type_id, item_id, "abyssals: dogma sync failed: {error}");
    }
  }
  Ok(())
}

async fn sync_one(ctx: &JobCtx<'_>, character_id: i64, type_id: i64, item_id: i64, now: i64) -> Result<(), Error> {
  let dynamic = ctx.esi.dogma().dynamic_item(type_id, item_id).await?;

  cache_icon(ctx, type_id).await;
  cache_icon(ctx, dynamic.source_type_id).await;

  let dogma_attributes = serde_json::to_string(&dynamic.dogma_attributes)?;
  let item = AbyssalItem::new(
    item_id,
    character_id,
    type_id,
    dynamic.source_type_id,
    dynamic.mutator_type_id,
    dogma_attributes,
    now,
  );
  assets::upsert(ctx.db, &item).await?;
  Ok(())
}

async fn refresh_prices_corporation(
  ctx: &JobCtx<'_>,
  corporation_id: i64,
  muta: &muta_market::Client,
  now: i64,
) -> Result<(), Error> {
  let items = assets::for_corporation_abyssal(ctx.db, corporation_id).await?;
  let stale_before = now - PRICE_TTL;

  for item in items {
    if item.muta_price_synced().unwrap_or(0) >= stale_before {
      continue;
    }
    match muta.item_price(item.item_id()).await {
      Ok(price) => assets::update_price_corporation(ctx.db, item.item_id(), price, now).await?,
      Err(error) => tracing::warn!(
        corporation_id,
        item_id = item.item_id(),
        "abyssals: MutaMarket price fetch failed: {error}"
      ),
    }
  }
  Ok(())
}

async fn sync_dogma_corporation(
  ctx: &JobCtx<'_>,
  corporation_id: i64,
  pairs: &[(i64, i64)],
  now: i64,
) -> Result<(), Error> {
  let existing: HashMap<i64, i64> = assets::for_corporation_abyssal(ctx.db, corporation_id)
    .await?
    .into_iter()
    .map(|row| (row.item_id(), row.synced_at()))
    .collect();
  let stale_before = now - SYNC_TTL;

  for &(type_id, item_id) in pairs {
    let last_synced = existing.get(&item_id).copied().unwrap_or(0);
    if last_synced >= stale_before {
      continue;
    }
    if let Err(error) = sync_one_corporation(ctx, corporation_id, type_id, item_id, now).await {
      tracing::warn!(corporation_id, type_id, item_id, "abyssals: dogma sync failed: {error}");
    }
  }
  Ok(())
}

async fn sync_one_corporation(
  ctx: &JobCtx<'_>,
  corporation_id: i64,
  type_id: i64,
  item_id: i64,
  now: i64,
) -> Result<(), Error> {
  let dynamic = ctx.esi.dogma().dynamic_item(type_id, item_id).await?;

  cache_icon(ctx, type_id).await;
  cache_icon(ctx, dynamic.source_type_id).await;

  let dogma_attributes = serde_json::to_string(&dynamic.dogma_attributes)?;
  let item = CorporationAbyssalItem::new(
    item_id,
    corporation_id,
    type_id,
    dynamic.source_type_id,
    dynamic.mutator_type_id,
    dogma_attributes,
    now,
  );
  assets::upsert_corporation(ctx.db, &item).await?;
  Ok(())
}

#[cfg(test)]
#[allow(clippy::needless_raw_string_hashes)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, eve_image, eve_sso::Grant, http},
    store::{
      self, images,
      model::{
        AbyssalModuleStat, Alliance, Bloodline, CharacterAsset, Corporation, CorporationAsset, CorporationMemberRole,
        Gender, OwnerType, Race,
      },
      repo::{character::insert_with_org, infra},
    },
    sync::job::{JobKey, JobKind},
  };

  const ABYSSAL_TYPE_ID: i64 = 47_408;

  const SOURCE_TYPE_ID: i64 = 5975;

  const MUTATOR_TYPE_ID: i64 = 47_297;

  async fn seed_character(db: &store::Database, id: i64) {
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
    let character = crate::store::model::Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  async fn seed_abyssal_type(db: &store::Database) {
    assets::upsert_module_stats(db, &[AbyssalModuleStat::new(ABYSSAL_TYPE_ID, 6, 0.8, 1.2)])
      .await
      .unwrap();
  }

  fn asset(character_id: i64, item_id: i64, type_id: i64, is_singleton: bool) -> CharacterAsset {
    CharacterAsset {
      character_id,
      container_id: None,
      depth: 0,
      is_active_ship: false,
      is_blueprint_copy: None,
      is_container: false,
      is_singleton,
      item_id,
      location_flag: "Hangar".to_owned(),
      location_id: 60_003_760,
      location_type: "station".to_owned(),
      name: None,
      quantity: 1,
      type_id,
    }
  }

  fn dynamic_item_body() -> serde_json::Value {
    serde_json::json!({
      "created_by": 90000001,
      "dogma_attributes": [
        { "attribute_id": 6, "value": 450.0 },
        { "attribute_id": 50, "value": 85.0 }
      ],
      "dogma_effects": [],
      "mutator_type_id": MUTATOR_TYPE_ID,
      "source_type_id": SOURCE_TYPE_ID
    })
  }

  async fn mount_dynamic_item(server: &MockServer, type_id: i64, item_id: i64) {
    Mock::given(method("GET"))
      .and(path(format!("/dogma/dynamic/items/{type_id}/{item_id}/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(dynamic_item_body()))
      .mount(server)
      .await;
  }

  async fn mount_icon(server: &MockServer, type_id: i64) {
    Mock::given(method("GET"))
      .and(path(format!("/types/{type_id}/icon")))
      .respond_with(ResponseTemplate::new(200).set_body_raw(vec![1u8, 2, 3], "image/png"))
      .mount(server)
      .await;
  }

  async fn mount_muta_price(server: &MockServer, item_id: i64, estimated_value: Option<f64>) {
    let body = serde_json::json!({ "estimated_value": estimated_value });
    Mock::given(method("GET"))
      .and(path(format!("/{item_id}")))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn mount_muta_unlisted(server: &MockServer, item_id: i64) {
    Mock::given(method("GET"))
      .and(path(format!("/{item_id}")))
      .respond_with(ResponseTemplate::new(404))
      .mount(server)
      .await;
  }

  fn muta(server: &MockServer) -> muta_market::Client {
    muta_market::Client::with_base_url(server.uri())
  }

  struct Harness {
    db: store::Database,
    esi: esi::Client,
    grant: Grant,
    image: eve_image::Client,
    image_store: images::Store,
    _images_dir: tempfile::TempDir,
  }

  impl Harness {
    async fn new(server: &MockServer, character_id: i64) -> Self {
      let db = store::open_test().await.unwrap();
      seed_character(&db, character_id).await;
      seed_abyssal_type(&db).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", character_id);
      Self {
        _images_dir: images_dir,
        db,
        esi,
        image,
        image_store,
        grant,
      }
    }

    fn ctx(&self, character_id: i64) -> JobCtx<'_> {
      JobCtx {
        db: &self.db,
        esi: &self.esi,
        image: &self.image,
        image_store: &self.image_store,
        key: JobKey::new(JobKind::CharacterAbyssals, Subject::Character(character_id)),
        grant: Some(&self.grant),
        sso: None,
      }
    }
  }

  mod outcome {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_blocks_when_the_abyssal_catalog_is_not_seeded() {
      let server = MockServer::start().await;
      let harness = Harness::new(&server, 42).await;
      assets::upsert_character_asset(&harness.db, &asset(42, 100, ABYSSAL_TYPE_ID, true))
        .await
        .unwrap();
      sqlx::query("DELETE FROM abyssal_module_stats")
        .execute(&harness.db.0)
        .await
        .unwrap();
      let ctx = harness.ctx(42);

      let outcome = run_with_muta(&ctx, &muta(&server)).await.unwrap();

      assert!(
        matches!(outcome, Outcome::Blocked { .. }),
        "an unseeded catalog must block rather than report a clean sync, got {outcome:?}"
      );
    }

    #[tokio::test]
    async fn it_is_empty_when_the_character_owns_no_singletons() {
      let server = MockServer::start().await;
      let harness = Harness::new(&server, 42).await;
      assets::upsert_character_asset(&harness.db, &asset(42, 200, 34, false))
        .await
        .unwrap();
      let ctx = harness.ctx(42);

      let outcome = run_with_muta(&ctx, &muta(&server)).await.unwrap();

      assert_eq!(outcome, Outcome::Empty);
    }

    #[tokio::test]
    async fn it_is_not_ready_when_the_character_is_not_synced() {
      let server = MockServer::start().await;
      let harness = Harness::new(&server, 42).await;
      let ctx = harness.ctx(999);

      let result = run_with_muta(&ctx, &muta(&server)).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
    }

    #[tokio::test]
    async fn it_syncs_with_a_row_count_when_a_catalogued_abyssal_is_owned() {
      let server = MockServer::start().await;
      mount_dynamic_item(&server, ABYSSAL_TYPE_ID, 100).await;
      mount_icon(&server, ABYSSAL_TYPE_ID).await;
      mount_icon(&server, SOURCE_TYPE_ID).await;
      mount_muta_price(&server, 100, Some(5_000_000.0)).await;
      let harness = Harness::new(&server, 42).await;
      assets::upsert_character_asset(&harness.db, &asset(42, 100, ABYSSAL_TYPE_ID, true))
        .await
        .unwrap();
      let ctx = harness.ctx(42);

      let outcome = run_with_muta(&ctx, &muta(&server)).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 1
        }
      );
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_prunes_records_no_longer_owned() {
      let server = MockServer::start().await;
      let harness = Harness::new(&server, 42).await;
      let stored = AbyssalItem::new(
        999,
        42,
        ABYSSAL_TYPE_ID,
        SOURCE_TYPE_ID,
        MUTATOR_TYPE_ID,
        r#"[{"attribute_id":6,"value":1.0}]"#.to_owned(),
        now_unix(),
      );
      assets::upsert(&harness.db, &stored).await.unwrap();
      let ctx = harness.ctx(42);

      run_with_muta(&ctx, &muta(&server)).await.unwrap();

      let rows = assets::for_character_abyssal(&harness.db, 42).await.unwrap();
      assert!(rows.is_empty(), "the unowned record is pruned");
    }

    #[tokio::test]
    async fn it_skips_dogma_refetch_for_a_recently_synced_item() {
      let server = MockServer::start().await;
      mount_muta_price(&server, 100, Some(1.0)).await;
      let harness = Harness::new(&server, 42).await;
      assets::upsert_character_asset(&harness.db, &asset(42, 100, ABYSSAL_TYPE_ID, true))
        .await
        .unwrap();
      let mut fresh = AbyssalItem::new(
        100,
        42,
        ABYSSAL_TYPE_ID,
        SOURCE_TYPE_ID,
        MUTATOR_TYPE_ID,
        r#"[{"attribute_id":6,"value":2.0}]"#.to_owned(),
        now_unix(),
      );
      fresh.set_muta_price(Some(9.0), now_unix());
      assets::upsert(&harness.db, &fresh).await.unwrap();
      let ctx = harness.ctx(42);

      run_with_muta(&ctx, &muta(&server)).await.unwrap();

      let rows = assets::for_character_abyssal(&harness.db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert!(rows[0].dogma_attributes().contains("\"value\":2.0"));
    }

    #[tokio::test]
    async fn it_stamps_a_none_price_for_an_unlisted_item() {
      let server = MockServer::start().await;
      mount_dynamic_item(&server, ABYSSAL_TYPE_ID, 100).await;
      mount_icon(&server, ABYSSAL_TYPE_ID).await;
      mount_icon(&server, SOURCE_TYPE_ID).await;
      mount_muta_unlisted(&server, 100).await;
      let harness = Harness::new(&server, 42).await;
      assets::upsert_character_asset(&harness.db, &asset(42, 100, ABYSSAL_TYPE_ID, true))
        .await
        .unwrap();
      let ctx = harness.ctx(42);

      run_with_muta(&ctx, &muta(&server)).await.unwrap();

      let rows = assets::for_character_abyssal(&harness.db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].muta_price_isk(), None);
      assert!(rows[0].muta_price_synced().is_some(), "the None price is stamped");
    }

    #[tokio::test]
    async fn it_still_upserts_the_item_when_an_icon_fetch_fails() {
      let server = MockServer::start().await;
      mount_dynamic_item(&server, ABYSSAL_TYPE_ID, 100).await;
      Mock::given(method("GET"))
        .and(path(format!("/types/{ABYSSAL_TYPE_ID}/icon")))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path(format!("/types/{SOURCE_TYPE_ID}/icon")))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      mount_muta_price(&server, 100, Some(5_000_000.0)).await;
      let harness = Harness::new(&server, 42).await;
      assets::upsert_character_asset(&harness.db, &asset(42, 100, ABYSSAL_TYPE_ID, true))
        .await
        .unwrap();
      let ctx = harness.ctx(42);

      run_with_muta(&ctx, &muta(&server)).await.unwrap();

      let rows = assets::for_character_abyssal(&harness.db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].muta_price_isk(), Some(5_000_000.0));
      assert!(!harness.image_store.type_icon_path(ABYSSAL_TYPE_ID, ICON_SIZE).exists());
    }

    #[tokio::test]
    async fn it_upserts_rolled_dogma_caches_icons_and_prices_the_item() {
      let server = MockServer::start().await;
      mount_dynamic_item(&server, ABYSSAL_TYPE_ID, 100).await;
      mount_icon(&server, ABYSSAL_TYPE_ID).await;
      mount_icon(&server, SOURCE_TYPE_ID).await;
      mount_muta_price(&server, 100, Some(5_000_000.0)).await;
      let harness = Harness::new(&server, 42).await;
      assets::upsert_character_asset(&harness.db, &asset(42, 100, ABYSSAL_TYPE_ID, true))
        .await
        .unwrap();
      let ctx = harness.ctx(42);

      run_with_muta(&ctx, &muta(&server)).await.unwrap();

      let rows = assets::for_character_abyssal(&harness.db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].item_id(), 100);
      assert_eq!(rows[0].source_type_id(), SOURCE_TYPE_ID);
      assert_eq!(rows[0].mutator_type_id(), MUTATOR_TYPE_ID);
      assert!(rows[0].dogma_attributes().contains("\"attribute_id\":6"));
      assert_eq!(rows[0].muta_price_isk(), Some(5_000_000.0));
      assert!(harness.image_store.type_icon_path(ABYSSAL_TYPE_ID, ICON_SIZE).exists());
      assert!(harness.image_store.type_icon_path(SOURCE_TYPE_ID, ICON_SIZE).exists());
    }
  }

  mod corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    const CORP_ID: i64 = 90_000_001;

    const DIRECTOR_ID: i64 = 42;

    async fn authorize_corp(db: &store::Database) {
      infra::upsert(
        db,
        CORP_ID,
        OwnerType::Corporation,
        "tok",
        "rt",
        4_102_444_800,
        Some(DIRECTOR_ID),
        None,
      )
      .await
      .unwrap();
      crate::store::repo::org::replace_for_corporation(
        db,
        CORP_ID,
        &[CorporationMemberRole::from((
          CORP_ID,
          DIRECTOR_ID,
          "Director".to_string(),
        ))],
      )
      .await
      .unwrap();
    }

    fn corp_asset(item_id: i64, type_id: i64, is_singleton: bool) -> CorporationAsset {
      CorporationAsset {
        container_id: None,
        corporation_id: CORP_ID,
        depth: 0,
        is_blueprint_copy: None,
        is_container: false,
        is_singleton,
        item_id,
        location_flag: "CorpDeliveries".to_owned(),
        location_id: 60_003_760,
        location_type: "station".to_owned(),
        name: None,
        quantity: 1,
        type_id,
      }
    }

    fn corp_ctx(harness: &Harness) -> JobCtx<'_> {
      JobCtx {
        db: &harness.db,
        esi: &harness.esi,
        image: &harness.image,
        image_store: &harness.image_store,
        key: JobKey::new(JobKind::CorporationAbyssals, Subject::Corporation(CORP_ID)),
        grant: Some(&harness.grant),
        sso: None,
      }
    }

    #[tokio::test]
    async fn it_is_not_ready_when_the_corporation_is_not_synced() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();
      seed_abyssal_type(&db).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", DIRECTOR_ID);
      let harness = Harness {
        _images_dir: images_dir,
        db,
        esi,
        image,
        image_store,
        grant,
      };

      let result = run_with_muta(&corp_ctx(&harness), &muta(&server)).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing corporation parent row must surface NotReady, got {result:?}"
      );
    }

    #[tokio::test]
    async fn it_upserts_corp_rolled_dogma_and_prices_the_item() {
      let server = MockServer::start().await;
      mount_dynamic_item(&server, ABYSSAL_TYPE_ID, 100).await;
      mount_icon(&server, ABYSSAL_TYPE_ID).await;
      mount_icon(&server, SOURCE_TYPE_ID).await;
      mount_muta_price(&server, 100, Some(5_000_000.0)).await;
      let harness = Harness::new(&server, DIRECTOR_ID).await;
      authorize_corp(&harness.db).await;
      assets::replace_for_corporation(&harness.db, CORP_ID, &[corp_asset(100, ABYSSAL_TYPE_ID, true)])
        .await
        .unwrap();

      let outcome = run_with_muta(&corp_ctx(&harness), &muta(&server)).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 1
        }
      );
      let rows = assets::for_corporation_abyssal(&harness.db, CORP_ID).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].item_id(), 100);
      assert_eq!(rows[0].source_type_id(), SOURCE_TYPE_ID);
      assert_eq!(rows[0].mutator_type_id(), MUTATOR_TYPE_ID);
      assert_eq!(rows[0].muta_price_isk(), Some(5_000_000.0));
    }

    #[tokio::test]
    async fn it_prunes_corp_records_no_longer_owned() {
      let server = MockServer::start().await;
      let harness = Harness::new(&server, DIRECTOR_ID).await;
      authorize_corp(&harness.db).await;
      let stored = CorporationAbyssalItem::new(
        999,
        CORP_ID,
        ABYSSAL_TYPE_ID,
        SOURCE_TYPE_ID,
        MUTATOR_TYPE_ID,
        r#"[{"attribute_id":6,"value":1.0}]"#.to_owned(),
        now_unix(),
      );
      assets::upsert_corporation(&harness.db, &stored).await.unwrap();

      run_with_muta(&corp_ctx(&harness), &muta(&server)).await.unwrap();

      let rows = assets::for_corporation_abyssal(&harness.db, CORP_ID).await.unwrap();
      assert!(rows.is_empty(), "the unowned corp record is pruned");
    }

    #[tokio::test]
    async fn it_stamps_a_none_price_for_an_unlisted_corp_item() {
      let server = MockServer::start().await;
      mount_dynamic_item(&server, ABYSSAL_TYPE_ID, 100).await;
      mount_icon(&server, ABYSSAL_TYPE_ID).await;
      mount_icon(&server, SOURCE_TYPE_ID).await;
      mount_muta_unlisted(&server, 100).await;
      let harness = Harness::new(&server, DIRECTOR_ID).await;
      authorize_corp(&harness.db).await;
      assets::replace_for_corporation(&harness.db, CORP_ID, &[corp_asset(100, ABYSSAL_TYPE_ID, true)])
        .await
        .unwrap();

      run_with_muta(&corp_ctx(&harness), &muta(&server)).await.unwrap();

      let rows = assets::for_corporation_abyssal(&harness.db, CORP_ID).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].muta_price_isk(), None);
      assert!(rows[0].muta_price_synced().is_some(), "the None price is stamped");
    }
  }
}
