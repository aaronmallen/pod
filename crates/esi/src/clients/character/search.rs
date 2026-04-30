//! Character search endpoint.

use serde::Deserialize;

use crate::{Error, clients::character::AuthenticatedClient};

#[derive(Debug, Default, Deserialize)]
struct CharacterSearchResult {
  character: Option<Vec<i64>>,
}

impl AuthenticatedClient<'_> {
  /// Searches for characters by name prefix (minimum 3 characters).
  /// Returns up to the first 20 matching character IDs.
  pub async fn search_characters(&self, query: &str) -> Result<Vec<i64>, Error> {
    let url = self
      .esi
      .url_builder()
      .path(format!("v3/characters/{}/search/", self.id))
      .param("categories", "character")
      .param("search", query)
      .build();
    let result: CharacterSearchResult = self.esi.http().get_json(&url, Some(self.grant.access_token())).await?;
    Ok(result.character.unwrap_or_default())
  }
}
