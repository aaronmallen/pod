use std::sync::Arc;

use crate::{
  clients::{esi, eve_image, eve_sso},
  services::{
    location_search::first_owned_grant,
    parsing::{
      dispatch::{Parsed, Resolved},
      resolve::EsiResolver,
    },
  },
  store::{Database, images},
};

const ITEM_SEARCH_CATEGORIES: &[&str] = &["inventory_type"];
const MAX_ITEM_RESULTS: usize = 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultibuyMatch {
  pub name: String,
  pub quantity: u64,
  pub type_id: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MultibuyResolution {
  pub matched: Vec<MultibuyMatch>,
  pub unmatched: Vec<String>,
}

pub async fn resolve_multibuy(
  db: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  entries: Vec<(String, u64)>,
) -> MultibuyResolution {
  if entries.is_empty() {
    return MultibuyResolution::default();
  }

  let _ = first_owned_grant(&db, &sso).await;

  let resolver = EsiResolver::new(esi);
  match Parsed::Multibuy(entries).resolve(&resolver).await {
    Some(Resolved::Multibuy(resolution)) => resolution,
    _ => MultibuyResolution::default(),
  }
}

pub async fn search_item_types(
  db: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  query: String,
) -> Vec<(i64, String)> {
  let Some(grant) = first_owned_grant(&db, &sso).await else {
    return Vec::new();
  };

  let result = match esi
    .universe()
    .search_with_categories(&query, ITEM_SEARCH_CATEGORIES, &grant)
    .await
  {
    Ok(result) => result,
    Err(error) => {
      tracing::warn!(target: "pod::assets", %error, query = %query, "item search failed");
      return Vec::new();
    }
  };

  let ids: Vec<i64> = result.inventory_type.into_iter().take(MAX_ITEM_RESULTS).collect();
  if ids.is_empty() {
    return Vec::new();
  }

  match esi.universe().names(&ids).await {
    Ok(names) => names
      .into_iter()
      .filter(|record| record.category == "inventory_type")
      .map(|record| (record.id, record.name))
      .collect(),
    Err(error) => {
      tracing::warn!(target: "pod::assets", %error, "item name resolution failed");
      Vec::new()
    }
  }
}

pub async fn resolve_location(
  db: Database,
  esi: Arc<esi::Client>,
  image: Arc<eve_image::Client>,
  sso: Arc<eve_sso::Client>,
  location_id: i64,
) {
  let Some(grant) = first_owned_grant(&db, &sso).await else {
    return;
  };
  if let Err(error) =
    crate::sync::resolve_stockpile_location(&db, &esi, &image, &images::default_store(), &grant, location_id).await
  {
    tracing::warn!(target: "pod::assets", %error, location_id, "stockpile location resolution failed");
  }
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
  };

  use super::*;
  use crate::{
    clients::{eve_sso, http},
    store::{
      self,
      model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
      repo::{character, infra},
    },
  };

  const CHAR: i64 = 42;

  async fn make_clients(base_url: &str) -> (Database, Arc<esi::Client>, Arc<eve_sso::Client>) {
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = Arc::new(esi::Client::with_base_url(http.clone(), base_url));
    let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
    (db, esi, sso)
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

  mod resolve_multibuy {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_matches_known_names_and_reports_unmatched_ones() {
      let server = MockServer::start().await;
      let body = r#"{"inventory_types":[{"id":34,"name":"Tritanium"},{"id":35,"name":"Pyerite"}]}"#;
      Mock::given(method("POST"))
        .and(path("/universe/ids/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;

      let resolution = resolve_multibuy(
        db,
        esi,
        sso,
        vec![
          ("tritanium".to_owned(), 100),
          ("Pyerite".to_owned(), 50),
          ("Notathing".to_owned(), 5),
        ],
      )
      .await;

      assert_eq!(
        resolution.matched,
        vec![
          MultibuyMatch {
            name: "tritanium".to_owned(),
            quantity: 100,
            type_id: 34,
          },
          MultibuyMatch {
            name: "Pyerite".to_owned(),
            quantity: 50,
            type_id: 35,
          },
        ]
      );
      assert_eq!(resolution.unmatched, vec!["Notathing".to_owned()]);
    }

    #[tokio::test]
    async fn it_returns_an_empty_resolution_for_no_entries() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;

      let resolution = resolve_multibuy(db, esi, sso, Vec::new()).await;

      assert_eq!(resolution, MultibuyResolution::default());
    }
  }

  mod search_item_types {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_credentialed_character_exists() {
      let server = MockServer::start().await;
      let (db, esi, sso) = make_clients(&server.uri()).await;

      let results = search_item_types(db, esi, sso, "Trit".to_owned()).await;

      assert!(results.is_empty());
    }

    #[tokio::test]
    async fn it_returns_resolved_type_ids_and_names() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .and(query_param("categories", "inventory_type"))
        .and(query_param("search", "Trit"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"inventory_type":[34,35]}"#, "application/json"))
        .mount(&server)
        .await;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"[{"category":"inventory_type","id":34,"name":"Tritanium"},{"category":"inventory_type","id":35,"name":"Pyerite"}]"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (db, esi, sso) = make_clients(&server.uri()).await;
      seed_owned_character(&db).await;

      let results = search_item_types(db, esi, sso, "Trit".to_owned()).await;

      assert_eq!(results, vec![(34, "Tritanium".to_owned()), (35, "Pyerite".to_owned())]);
    }
  }
}
