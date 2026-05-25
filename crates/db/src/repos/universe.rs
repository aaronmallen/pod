use sea_orm::DatabaseConnection;

pub mod abyssal_module_stats;
pub mod bloodlines;
pub mod certificates;
pub mod constellations;
pub mod dogma_attrs;
pub mod factions;
pub mod item_categories;
pub mod item_groups;
pub mod item_types;
pub mod market_groups;
pub mod planets;
pub mod races;
pub mod regions;
pub mod solar_systems;
pub mod stargates;
pub mod stars;
pub mod stations;
pub mod structure_cache;
pub mod type_icons;

pub struct Repo<'a> {
  connection: &'a DatabaseConnection,
}

impl<'a> Repo<'a> {
  pub fn new(connection: &'a DatabaseConnection) -> Self {
    Self {
      connection,
    }
  }

  pub fn abyssal_module_stats(&self) -> abyssal_module_stats::Repo<'_> {
    abyssal_module_stats::Repo::new(self.connection)
  }

  pub fn bloodlines(&self) -> bloodlines::Repo<'_> {
    bloodlines::Repo::new(self.connection)
  }

  pub fn certificates(&self) -> certificates::Repo<'_> {
    certificates::Repo::new(self.connection)
  }

  pub fn constellations(&self) -> constellations::Repo<'_> {
    constellations::Repo::new(self.connection)
  }

  pub fn dogma_attrs(&self) -> dogma_attrs::Repo<'_> {
    dogma_attrs::Repo::new(self.connection)
  }

  pub fn factions(&self) -> factions::Repo<'_> {
    factions::Repo::new(self.connection)
  }

  pub fn item_categories(&self) -> item_categories::Repo<'_> {
    item_categories::Repo::new(self.connection)
  }

  pub fn item_groups(&self) -> item_groups::Repo<'_> {
    item_groups::Repo::new(self.connection)
  }

  pub fn item_types(&self) -> item_types::Repo<'_> {
    item_types::Repo::new(self.connection)
  }

  pub fn market_groups(&self) -> market_groups::Repo<'_> {
    market_groups::Repo::new(self.connection)
  }

  pub fn planets(&self) -> planets::Repo<'_> {
    planets::Repo::new(self.connection)
  }

  pub fn races(&self) -> races::Repo<'_> {
    races::Repo::new(self.connection)
  }

  pub fn regions(&self) -> regions::Repo<'_> {
    regions::Repo::new(self.connection)
  }

  pub fn solar_systems(&self) -> solar_systems::Repo<'_> {
    solar_systems::Repo::new(self.connection)
  }

  pub fn stargates(&self) -> stargates::Repo<'_> {
    stargates::Repo::new(self.connection)
  }

  pub fn stars(&self) -> stars::Repo<'_> {
    stars::Repo::new(self.connection)
  }

  pub fn stations(&self) -> stations::Repo<'_> {
    stations::Repo::new(self.connection)
  }

  pub fn structure_cache(&self) -> structure_cache::Repo<'_> {
    structure_cache::Repo::new(self.connection)
  }

  pub fn type_icons(&self) -> type_icons::Repo<'_> {
    type_icons::Repo::new(self.connection)
  }
}

#[cfg(test)]
mod tests {
  use sea_orm::Database;

  use super::*;

  mod new {
    use super::*;

    #[tokio::test]
    async fn it_constructs_with_db_connection() {
      let db = Database::connect("sqlite::memory:").await.unwrap();
      crate::migrations::run(&db).await.unwrap();

      let repo = Repo::new(&db);
      let result = repo.bloodlines().all().await.unwrap();

      assert!(result.is_empty());
    }
  }
}
