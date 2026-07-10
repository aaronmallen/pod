use std::{collections::HashMap, sync::Arc};

use crate::{
  clients::esi,
  store::{Database, model::ItemType, repo::sde},
};

const RESOLVE_NAMES_CHUNK: usize = 1000;

pub struct EsiResolver {
  esi: Arc<esi::Client>,
}

impl EsiResolver {
  pub fn new(esi: Arc<esi::Client>) -> Self {
    Self {
      esi,
    }
  }
}

impl Resolver for EsiResolver {
  async fn resolve(&self, names: &[String]) -> Resolution {
    let mut matched: HashMap<String, i64> = HashMap::new();
    for chunk in names.chunks(RESOLVE_NAMES_CHUNK) {
      let ids = match self.esi.universe().ids(chunk).await {
        Ok(ids) => ids,
        Err(error) => {
          tracing::warn!(target: "pod::parsing", %error, "esi name resolution failed");
          continue;
        }
      };
      for record in ids.inventory_types {
        matched.insert(record.name.to_lowercase(), record.id);
      }
    }

    let unmatched = unmatched_names(names, &matched);
    Resolution {
      matched,
      unmatched,
    }
  }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Resolution {
  pub matched: HashMap<String, i64>,
  pub unmatched: Vec<String>,
}

#[allow(async_fn_in_trait)]
pub trait Resolver {
  async fn resolve(&self, names: &[String]) -> Resolution;
}

pub struct SdeResolver {
  db: Database,
}

impl SdeResolver {
  pub fn new(db: Database) -> Self {
    Self {
      db,
    }
  }

  pub async fn item_types(&self, names: &[String]) -> Vec<ItemType> {
    sde::item_types_by_names_ci(&self.db, names).await.unwrap_or_default()
  }
}

impl Resolver for SdeResolver {
  async fn resolve(&self, names: &[String]) -> Resolution {
    let mut matched: HashMap<String, i64> = HashMap::new();
    for item_type in self.item_types(names).await {
      matched.entry(item_type.name().to_lowercase()).or_insert(item_type.id());
    }

    let unmatched = unmatched_names(names, &matched);
    Resolution {
      matched,
      unmatched,
    }
  }
}

fn unmatched_names(names: &[String], matched: &HashMap<String, i64>) -> Vec<String> {
  names
    .iter()
    .filter(|name| !matched.contains_key(&name.to_lowercase()))
    .cloned()
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod unmatched_names {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reports_input_names_absent_from_the_matched_map() {
      let matched = HashMap::from([("tritanium".to_owned(), 34)]);

      let unmatched = unmatched_names(&["Tritanium".to_owned(), "Notathing".to_owned()], &matched);

      assert_eq!(unmatched, vec!["Notathing".to_owned()]);
    }

    #[test]
    fn it_preserves_duplicates_and_original_casing() {
      let matched = HashMap::new();

      let unmatched = unmatched_names(&["Foo".to_owned(), "foo".to_owned()], &matched);

      assert_eq!(unmatched, vec!["Foo".to_owned(), "foo".to_owned()]);
    }
  }

  mod esi_resolver {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;
    use crate::{clients::http, store};

    #[tokio::test]
    async fn it_resolves_names_to_type_ids_case_insensitively() {
      let server = MockServer::start().await;
      let body = r#"{"inventory_types":[{"id":34,"name":"Tritanium"},{"id":35,"name":"Pyerite"}]}"#;
      Mock::given(method("POST"))
        .and(path("/universe/ids/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http, server.uri()));
      let resolver = EsiResolver::new(esi);

      let resolution = resolver
        .resolve(&["tritanium".to_owned(), "Pyerite".to_owned(), "Notathing".to_owned()])
        .await;

      assert_eq!(resolution.matched.get("tritanium"), Some(&34));
      assert_eq!(resolution.matched.get("pyerite"), Some(&35));
      assert_eq!(resolution.unmatched, vec!["Notathing".to_owned()]);
    }
  }

  mod sde_resolver {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    async fn seed_item_type(db: &Database, id: i64, name: &str) {
      sqlx::query("INSERT OR IGNORE INTO item_categories (id, name, published) VALUES (6, 'Ship', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("INSERT OR IGNORE INTO item_groups (id, category_id, name, published) VALUES (25, 6, 'Frigate', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT INTO item_types (id, group_id, description, name, published, dogma_attributes) \
        VALUES (?, 25, '', ?, 1, '[]')",
      )
      .bind(id)
      .bind(name)
      .execute(db.writer())
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_resolves_known_names_and_reports_unmatched_ones() {
      let db = store::open_test().await.unwrap();
      seed_item_type(&db, 587, "Rifter").await;
      let resolver = SdeResolver::new(db);

      let resolution = resolver.resolve(&["rifter".to_owned(), "Notathing".to_owned()]).await;

      assert_eq!(resolution.matched.get("rifter"), Some(&587));
      assert_eq!(resolution.unmatched, vec!["Notathing".to_owned()]);
    }

    #[tokio::test]
    async fn it_returns_the_full_item_type_rows() {
      let db = store::open_test().await.unwrap();
      seed_item_type(&db, 587, "Rifter").await;
      let resolver = SdeResolver::new(db);

      let rows = resolver.item_types(&["Rifter".to_owned()]).await;

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].id(), 587);
    }
  }
}
