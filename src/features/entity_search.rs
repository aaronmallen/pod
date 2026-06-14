#![allow(dead_code)]

use std::{
  fmt::{self, Display, Formatter},
  sync::Arc,
  time::Duration,
};

use crate::{
  clients::{esi, eve_image, eve_image::Size, eve_sso, eve_sso::Grant},
  store::{Database, images, model::OwnerType, repo::character},
};

pub const SEARCH_DEBOUNCE: Duration = Duration::from_millis(200);
pub const SEARCH_MIN_CHARS: usize = 3;

const MAX_RESULTS: usize = 20;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EntityCategory {
  Alliance,
  Character,
  Corporation,
}

impl EntityCategory {
  pub fn from_esi(category: &str) -> Option<Self> {
    match category {
      "alliance" => Some(Self::Alliance),
      "character" => Some(Self::Character),
      "corporation" => Some(Self::Corporation),
      _ => None,
    }
  }

  pub fn esi_category(self) -> &'static str {
    match self {
      Self::Alliance => "alliance",
      Self::Character => "character",
      Self::Corporation => "corporation",
    }
  }

  pub fn image_kind(self) -> images::ImageKind {
    match self {
      Self::Alliance => images::ImageKind::AllianceLogo,
      Self::Character => images::ImageKind::CharacterPortrait,
      Self::Corporation => images::ImageKind::CorporationLogo,
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      Self::Alliance => "Alliance",
      Self::Character => "Character",
      Self::Corporation => "Corporation",
    }
  }
}

impl Display for EntityCategory {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.write_str(self.label())
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityResult {
  pub category: EntityCategory,
  pub id: i64,
  pub name: String,
}

pub async fn search_entities(
  db: Database,
  esi: Arc<esi::Client>,
  eve_image: Arc<eve_image::Client>,
  sso: Arc<eve_sso::Client>,
  categories: Vec<EntityCategory>,
  query: String,
) -> Vec<EntityResult> {
  // Sleep first so rapid keystrokes cancel this task before the guard runs, coalescing requests.
  tokio::time::sleep(SEARCH_DEBOUNCE).await;
  if query.trim().chars().count() < SEARCH_MIN_CHARS || categories.is_empty() {
    return Vec::new();
  }

  let Some(grant) = first_owned_grant(&db, &sso).await else {
    return Vec::new();
  };

  let results = resolve_entities(&esi, &grant, &categories, &query).await;
  cache_result_portraits(&eve_image, &results).await;
  results
}

async fn cache_result_portraits(eve_image: &eve_image::Client, results: &[EntityResult]) {
  let store = images::default_store();
  for result in results {
    let path = store.image_path(result.category.image_kind(), result.id);
    if images::is_fresh(&path, images::STALE_AFTER) {
      continue;
    }
    let url = match result.category {
      EntityCategory::Alliance => eve_image.alliance_logo_url(result.id, images::LOGO_SIZE),
      EntityCategory::Character => eve_image.character_portrait_url(result.id, Size::S64),
      EntityCategory::Corporation => eve_image.corporation_logo_url(result.id, images::LOGO_SIZE),
    };
    if let Ok(bytes) = eve_image.fetch(&url).await {
      let _ = store.write(&path, &bytes);
    }
  }
}

/// Borrows a token from any owned character to authenticate the ESI search request.
///
/// ESI's search endpoint requires a valid character token but returns universe-wide results;
/// the identity of the authenticating character does not affect what comes back.
async fn first_owned_grant(db: &Database, sso: &eve_sso::Client) -> Option<Grant> {
  let owner = character::all_owned(db).await.unwrap_or_default().into_iter().next()?;
  match crate::sync::token::fresh_token(db, sso, owner.id(), OwnerType::Character).await {
    Ok(grant) => grant,
    Err(error) => {
      tracing::warn!(target: "pod::entity_search", %error, "entity search: no usable token");
      None
    }
  }
}

async fn resolve_entities(
  esi: &esi::Client,
  grant: &Grant,
  categories: &[EntityCategory],
  query: &str,
) -> Vec<EntityResult> {
  let category_args: Vec<&str> = categories.iter().map(|c| c.esi_category()).collect();
  let result = match esi
    .universe()
    .search_with_categories(query, &category_args, grant)
    .await
  {
    Ok(result) => result,
    Err(error) => {
      tracing::warn!(target: "pod::entity_search", %error, query = %query, "entity search failed");
      return Vec::new();
    }
  };

  let mut ids: Vec<i64> = Vec::new();
  for category in categories {
    let bucket = match category {
      EntityCategory::Alliance => &result.alliance,
      EntityCategory::Character => &result.character,
      EntityCategory::Corporation => &result.corporation,
    };
    ids.extend(bucket.iter().copied());
  }
  ids.truncate(MAX_RESULTS);
  if ids.is_empty() {
    return Vec::new();
  }

  match esi.universe().names(&ids).await {
    Ok(names) => names
      .into_iter()
      .filter_map(|record| {
        EntityCategory::from_esi(&record.category)
          .filter(|category| categories.contains(category))
          .map(|category| EntityResult {
            category,
            id: record.id,
            name: record.name,
          })
      })
      .collect(),
    Err(error) => {
      tracing::warn!(target: "pod::entity_search", %error, "entity name resolution failed");
      Vec::new()
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod entity_category {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_through_the_esi_category_string() {
      for category in [
        EntityCategory::Alliance,
        EntityCategory::Character,
        EntityCategory::Corporation,
      ] {
        assert_eq!(EntityCategory::from_esi(category.esi_category()), Some(category));
      }
    }

    #[test]
    fn it_rejects_unsupported_esi_categories() {
      assert_eq!(EntityCategory::from_esi("solar_system"), None);
      assert_eq!(EntityCategory::from_esi(""), None);
    }

    #[test]
    fn it_maps_each_category_to_its_image_kind() {
      assert_eq!(EntityCategory::Alliance.image_kind(), images::ImageKind::AllianceLogo);
      assert_eq!(
        EntityCategory::Character.image_kind(),
        images::ImageKind::CharacterPortrait
      );
      assert_eq!(
        EntityCategory::Corporation.image_kind(),
        images::ImageKind::CorporationLogo
      );
    }
  }

  mod search_entities {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path, query_param},
    };

    use super::*;
    use crate::{
      clients::http,
      store::{
        self,
        model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
        repo::infra,
      },
    };

    const CHAR: i64 = 42;

    async fn make_clients(
      base_url: &str,
    ) -> (Database, Arc<esi::Client>, Arc<eve_image::Client>, Arc<eve_sso::Client>) {
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), base_url));
      let eve_image = Arc::new(eve_image::Client::with_base_url(http.clone(), base_url));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      (db, esi, eve_image, sso)
    }

    async fn seed_owned_character(db: &Database) {
      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, CHAR, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(CHAR);
      corp.set_creator_id(CHAR);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(CHAR, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
      let far_future = chrono::Utc::now().timestamp() + 86_400;
      infra::upsert(db, CHAR, OwnerType::Character, "tok", "rt", far_future, None, None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_resolves_results_across_the_caller_specified_categories_with_their_category() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .and(query_param("categories", "character,corporation"))
        .and(query_param("search", "Vex"))
        .respond_with(
          ResponseTemplate::new(200).set_body_raw(r#"{"character":[95],"corporation":[96]}"#, "application/json"),
        )
        .mount(&server)
        .await;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"[{"id":95,"name":"Vex Voronova","category":"character"},{"id":96,"name":"Vex Holdings","category":"corporation"}]"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (db, esi, eve_image, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;

      let results = search_entities(
        db,
        esi,
        eve_image,
        sso,
        vec![EntityCategory::Character, EntityCategory::Corporation],
        "Vex".to_owned(),
      )
      .await;

      assert_eq!(
        results,
        vec![
          EntityResult {
            category: EntityCategory::Character,
            id: 95,
            name: "Vex Voronova".to_owned(),
          },
          EntityResult {
            category: EntityCategory::Corporation,
            id: 96,
            name: "Vex Holdings".to_owned(),
          },
        ]
      );
    }

    #[tokio::test]
    async fn it_passes_through_only_the_requested_categories() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .and(query_param("categories", "character"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"character":[95]}"#, "application/json"))
        .mount(&server)
        .await;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"[{"id":95,"name":"Vex Voronova","category":"character"}]"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (db, esi, eve_image, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;

      let results = search_entities(
        db,
        esi,
        eve_image,
        sso,
        vec![EntityCategory::Character],
        "Vex".to_owned(),
      )
      .await;

      assert_eq!(results.len(), 1);
      assert_eq!(results[0].category, EntityCategory::Character);
    }

    #[tokio::test]
    async fn it_yields_nothing_below_the_min_chars_threshold() {
      let server = MockServer::start().await;
      let (db, esi, eve_image, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;

      let results = search_entities(
        db,
        esi,
        eve_image,
        sso,
        vec![EntityCategory::Character],
        "Ve".to_owned(),
      )
      .await;

      assert!(results.is_empty());
    }

    #[tokio::test]
    async fn it_yields_nothing_without_a_credentialed_character() {
      let server = MockServer::start().await;
      let (db, esi, eve_image, sso) = make_clients(&server.uri()).await;

      let results = search_entities(
        db,
        esi,
        eve_image,
        sso,
        vec![EntityCategory::Character],
        "Vex".to_owned(),
      )
      .await;

      assert!(results.is_empty());
    }
  }
}
