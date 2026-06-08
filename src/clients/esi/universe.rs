use crate::clients::{
  self,
  esi::{
    Client as EsiClient,
    models::universe::{
      Constellation, ItemCategory, ItemGroup, ItemType, MarketGroup, NameRecord, Region, ResolvedIds, SearchResult,
      SolarSystem, Station, Structure,
    },
  },
  eve_sso::Grant,
};

pub struct Client<'a> {
  esi: &'a EsiClient,
}

impl<'a> Client<'a> {
  pub fn new(esi: &'a EsiClient) -> Self {
    Self {
      esi,
    }
  }

  pub async fn constellation(&self, constellation_id: i64) -> Result<Constellation, clients::Error> {
    let url = self.esi.url(&format!("universe/constellations/{constellation_id}/"));
    self.esi.get_json(&url, None).await
  }

  pub async fn ids(&self, names: &[String]) -> Result<ResolvedIds, clients::Error> {
    let url = self.esi.url("universe/ids/");
    self.esi.post_json_anon(&url, &names).await
  }

  pub async fn item_category(&self, category_id: i32) -> Result<ItemCategory, clients::Error> {
    let url = self.esi.url(&format!("universe/categories/{category_id}/"));
    self.esi.get_json(&url, None).await
  }

  pub async fn item_group(&self, group_id: i32) -> Result<ItemGroup, clients::Error> {
    let url = self.esi.url(&format!("universe/groups/{group_id}/"));
    self.esi.get_json(&url, None).await
  }

  pub async fn item_type(&self, type_id: i32) -> Result<ItemType, clients::Error> {
    let url = self.esi.url(&format!("universe/types/{type_id}/"));
    self.esi.get_json(&url, None).await
  }

  pub async fn market_group(&self, market_group_id: i32) -> Result<MarketGroup, clients::Error> {
    let url = self.esi.url(&format!("markets/groups/{market_group_id}/"));
    self.esi.get_json(&url, None).await
  }

  pub async fn names(&self, ids: &[i64]) -> Result<Vec<NameRecord>, clients::Error> {
    let url = self.esi.url("universe/names/");
    self.esi.post_json_anon(&url, &ids).await
  }

  pub async fn region(&self, region_id: i64) -> Result<Region, clients::Error> {
    let url = self.esi.url(&format!("universe/regions/{region_id}/"));
    self.esi.get_json(&url, None).await
  }

  pub async fn search(&self, query: &str, grant: &Grant) -> Result<SearchResult, clients::Error> {
    self
      .search_with_categories(query, &["character", "corporation", "alliance"], grant)
      .await
  }

  pub async fn search_with_categories(
    &self,
    query: &str,
    categories: &[&str],
    grant: &Grant,
  ) -> Result<SearchResult, clients::Error> {
    let base = self.esi.url(&format!("characters/{}/search/", grant.character_id()));
    let url = reqwest::Url::parse_with_params(
      &base,
      &[("categories", categories.join(",").as_str()), ("search", query)],
    )
    .map_err(|e| clients::Error::Internal(format!("invalid search url: {e}")))?;
    self.esi.get_json(url.as_str(), Some(grant.access_token())).await
  }

  pub async fn solar_system(&self, system_id: i64) -> Result<SolarSystem, clients::Error> {
    let url = self.esi.url(&format!("universe/systems/{system_id}/"));
    self.esi.get_json(&url, None).await
  }

  pub async fn station(&self, station_id: i64) -> Result<Station, clients::Error> {
    let url = self.esi.url(&format!("universe/stations/{station_id}/"));
    self.esi.get_json(&url, None).await
  }

  pub async fn structure(&self, structure_id: i64, grant: &Grant) -> Result<Structure, clients::Error> {
    let url = self.esi.url(&format!("universe/structures/{structure_id}/"));
    self.esi.get_json(&url, Some(grant.access_token())).await
  }
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
  };

  use super::*;
  use crate::{clients::http, store};

  async fn make_esi(base_url: &str) -> EsiClient {
    let db = store::open_test().await.unwrap();
    let cache = http::Cache::new(db);
    let http = http::Client::builder(cache).build();
    EsiClient::with_base_url(http, base_url)
  }

  mod ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_posts_names_and_returns_bucketed_ids() {
      let server = MockServer::start().await;
      let body = r#"{"characters":[{"id":95465499,"name":"CCP Bartender"}],"corporations":[{"id":98356193,"name":"Test Corp"}],"alliances":[{"id":99005338,"name":"Test Alliance"}]}"#;
      Mock::given(method("POST"))
        .and(path("/universe/ids/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let resolved = esi
        .universe()
        .ids(&[
          "CCP Bartender".to_owned(),
          "Test Corp".to_owned(),
          "Test Alliance".to_owned(),
        ])
        .await
        .unwrap();

      assert_eq!(resolved.characters.len(), 1);
      assert_eq!(resolved.characters[0].id, 95465499);
      assert_eq!(resolved.corporations[0].name, "Test Corp");
      assert_eq!(resolved.alliances[0].id, 99005338);
    }

    #[tokio::test]
    async fn it_defaults_missing_buckets_to_empty() {
      let server = MockServer::start().await;
      let body = r#"{"characters":[{"id":42,"name":"Solo"}]}"#;
      Mock::given(method("POST"))
        .and(path("/universe/ids/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let resolved = esi.universe().ids(&["Solo".to_owned()]).await.unwrap();

      assert_eq!(resolved.characters.len(), 1);
      assert!(resolved.corporations.is_empty());
      assert!(resolved.alliances.is_empty());
      assert!(resolved.inventory_types.is_empty());
    }

    #[tokio::test]
    async fn it_resolves_inventory_types() {
      let server = MockServer::start().await;
      let body = r#"{"inventory_types":[{"id":34,"name":"Tritanium"},{"id":35,"name":"Pyerite"}]}"#;
      Mock::given(method("POST"))
        .and(path("/universe/ids/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let resolved = esi
        .universe()
        .ids(&["Tritanium".to_owned(), "Pyerite".to_owned()])
        .await
        .unwrap();

      assert_eq!(resolved.inventory_types.len(), 2);
      assert_eq!(resolved.inventory_types[0].id, 34);
      assert_eq!(resolved.inventory_types[0].name, "Tritanium");
      assert_eq!(resolved.inventory_types[1].id, 35);
      assert!(resolved.characters.is_empty());
    }
  }

  mod item_type {
    use pretty_assertions::assert_eq;

    use super::*;

    const TYPE_3300_FIXTURE: &str = include_str!("../../../test/fixtures/esi/universe_types_3300.json");
    const TYPE_9899_FIXTURE: &str = include_str!("../../../test/fixtures/esi/universe_types_9899.json");

    fn attr_value(item: &ItemType, attribute_id: i32) -> Option<f64> {
      item
        .dogma_attributes
        .iter()
        .find(|a| a.attribute_id == attribute_id)
        .map(|a| a.value)
    }

    #[tokio::test]
    async fn it_returns_item_type_with_optional_fields() {
      let server = MockServer::start().await;
      let body = r#"{"capacity":0.0,"description":"Tritanium.","group_id":18,"mass":0.0,"name":"Tritanium","packaged_volume":0.01,"portion_size":1,"published":true,"radius":1.0,"type_id":34,"volume":0.01}"#;
      Mock::given(method("GET"))
        .and(path("/universe/types/34/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let item = esi.universe().item_type(34).await.unwrap();

      assert_eq!(item.name, "Tritanium");
      assert_eq!(item.group_id, 18);
      assert_eq!(item.market_group_id, None);
      assert!(item.dogma_attributes.is_empty());
    }

    #[tokio::test]
    async fn it_deserializes_real_gunnery_dogma_attributes() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/universe/types/3300/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TYPE_3300_FIXTURE, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let item = esi.universe().item_type(3300).await.unwrap();

      assert_eq!(item.name, "Gunnery");
      assert_eq!(attr_value(&item, 275), Some(1.0));
      assert_eq!(attr_value(&item, 180), Some(167.0));
      assert_eq!(attr_value(&item, 181), Some(168.0));
    }

    #[tokio::test]
    async fn it_deserializes_real_attribute_implant_dogma() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/universe/types/9899/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TYPE_9899_FIXTURE, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let item = esi.universe().item_type(9899).await.unwrap();

      assert_eq!(item.name, "Memory Augmentation - Basic");
      assert_eq!(attr_value(&item, 177), Some(3.0));
    }
  }

  mod names {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_posts_ids_and_returns_name_records() {
      let server = MockServer::start().await;
      let body = r#"[{"category":"character","id":95465499,"name":"CCP Bartender"},{"category":"corporation","id":98356193,"name":"Test Corp"},{"category":"solar_system","id":30000142,"name":"Jita"}]"#;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let records = esi.universe().names(&[95465499, 98356193, 30000142]).await.unwrap();

      assert_eq!(records.len(), 3);
      assert_eq!(records[0].category, "character");
      assert_eq!(records[0].name, "CCP Bartender");
      assert_eq!(records[2].category, "solar_system");
      assert_eq!(records[2].id, 30000142);
    }

    #[tokio::test]
    async fn it_returns_http_error_on_4xx() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let result = esi.universe().names(&[1]).await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }
  }

  mod region {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_region() {
      let server = MockServer::start().await;
      let body =
        r#"{"constellations":[20000001,20000002],"description":"The Forge.","name":"The Forge","region_id":10000002}"#;
      Mock::given(method("GET"))
        .and(path("/universe/regions/10000002/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let region = esi.universe().region(10000002).await.unwrap();

      assert_eq!(region.name, "The Forge");
      assert_eq!(region.constellations.len(), 2);
    }

    #[tokio::test]
    async fn it_returns_http_error_on_4xx() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/universe/regions/10000002/"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let result = esi.universe().region(10000002).await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }
  }

  mod search {
    use wiremock::matchers::query_param;

    use super::*;

    #[tokio::test]
    async fn it_searches_with_categories_and_query_and_bearer_token() {
      let server = MockServer::start().await;
      let body = r#"{"character":[95465499,90000001],"corporation":[98356193],"alliance":[99005338]}"#;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .and(query_param("categories", "character,corporation,alliance"))
        .and(query_param("search", "Test"))
        .and(header("Authorization", "Bearer search-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;
      let grant = Grant::new_test("search-token", 42);

      let result = esi.universe().search("Test", &grant).await.unwrap();

      assert_eq!(result.character.len(), 2);
      assert_eq!(result.corporation, vec![98356193]);
      assert_eq!(result.alliance, vec![99005338]);
    }

    #[tokio::test]
    async fn it_url_encodes_queries_with_spaces() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .and(query_param("search", "CCP Bartender"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"character":[95465499]}"#, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;
      let grant = Grant::new_test("search-token", 42);

      let result = esi.universe().search("CCP Bartender", &grant).await.unwrap();

      assert_eq!(result.character, vec![95465499]);
    }

    #[tokio::test]
    async fn it_defaults_missing_categories_to_empty() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"character":[1]}"#, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;
      let grant = Grant::new_test("search-token", 42);

      let result = esi.universe().search("Sol", &grant).await.unwrap();

      assert_eq!(result.character, vec![1]);
      assert!(result.corporation.is_empty());
      assert!(result.alliance.is_empty());
    }

    #[tokio::test]
    async fn it_searches_location_categories_and_returns_station_structure_system_ids() {
      let server = MockServer::start().await;
      let body = r#"{"solar_system":[30000142],"station":[60003760],"structure":[1234567890]}"#;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .and(query_param("categories", "station,structure,solar_system"))
        .and(query_param("search", "Jita"))
        .and(header("Authorization", "Bearer search-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;
      let grant = Grant::new_test("search-token", 42);

      let result = esi
        .universe()
        .search_with_categories("Jita", &["station", "structure", "solar_system"], &grant)
        .await
        .unwrap();

      assert_eq!(result.station, vec![60003760]);
      assert_eq!(result.structure, vec![1234567890]);
      assert_eq!(result.solar_system, vec![30000142]);
      assert!(result.inventory_type.is_empty());
    }

    #[tokio::test]
    async fn it_searches_inventory_type_category_and_returns_type_ids() {
      let server = MockServer::start().await;
      let body = r#"{"inventory_type":[34,35]}"#;
      Mock::given(method("GET"))
        .and(path("/characters/42/search/"))
        .and(query_param("categories", "inventory_type"))
        .and(query_param("search", "Trit"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;
      let grant = Grant::new_test("search-token", 42);

      let result = esi
        .universe()
        .search_with_categories("Trit", &["inventory_type"], &grant)
        .await
        .unwrap();

      assert_eq!(result.inventory_type, vec![34, 35]);
      assert!(result.station.is_empty());
      assert!(result.character.is_empty());
    }
  }

  mod solar_system {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_solar_system() {
      let server = MockServer::start().await;
      let body = r#"{"constellation_id":20000020,"name":"Jita","position":{"x":1.0,"y":2.0,"z":3.0},"security_class":"B","security_status":0.946,"star_id":40000001,"system_id":30000142}"#;
      Mock::given(method("GET"))
        .and(path("/universe/systems/30000142/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let system = esi.universe().solar_system(30000142).await.unwrap();

      assert_eq!(system.name, "Jita");
      assert_eq!(system.constellation_id, 20000020);
    }
  }

  mod structure {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_sends_the_bearer_token() {
      let server = MockServer::start().await;
      let body = r#"{"name":"A Player Structure","owner_id":98000001,"solar_system_id":30000142,"type_id":35833}"#;
      Mock::given(method("GET"))
        .and(path("/universe/structures/1234567890/"))
        .and(header("Authorization", "Bearer structure-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;
      let grant = Grant::new_test("structure-token", 42);

      let structure = esi.universe().structure(1234567890, &grant).await.unwrap();

      assert_eq!(structure.name, "A Player Structure");
      assert_eq!(structure.owner_id, 98000001);
    }
  }
}
