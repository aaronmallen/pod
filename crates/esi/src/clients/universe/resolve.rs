//! Universe name/ID resolution endpoints.

use crate::{
  Error,
  clients::universe::Client,
  models::universe::{ResolvedIds, ResolvedName},
};

impl Client<'_> {
  /// Resolves a list of names to their IDs and categories.
  pub async fn ids(&self, names: &[&str]) -> Result<ResolvedIds, Error> {
    self
      .esi
      .http()
      .post_json_anon(
        &self.esi.url_builder().path("v1/universe/ids/".to_string()).build(),
        &names,
      )
      .await
  }

  /// Resolves a list of IDs to names and categories.
  pub async fn names(&self, ids: &[i64]) -> Result<Vec<ResolvedName>, Error> {
    self
      .esi
      .http()
      .post_json_anon(
        &self.esi.url_builder().path("v3/universe/names/".to_string()).build(),
        &ids,
      )
      .await
  }
}
