use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::faction::Faction;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub corporation_id: Option<i64>,
  #[getset(get = "pub")]
  pub description: String,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get_copy = "pub")]
  pub is_unique: i32,
  #[getset(get_copy = "pub")]
  pub militia_corporation_id: Option<i64>,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub size_factor: f64,
  #[getset(get_copy = "pub")]
  pub solar_system_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub station_count: i32,
  #[getset(get_copy = "pub")]
  pub station_system_count: i32,
}

impl Model {
  pub fn new(
    id: i64,
    name: impl Into<String>,
    is_unique: bool,
    size_factor: f64,
    station_count: i32,
    station_system_count: i32,
  ) -> Self {
    let name = name.into();
    Self {
      corporation_id: None,
      description: name.clone(),
      id,
      is_unique: i32::from(is_unique),
      militia_corporation_id: None,
      name,
      size_factor,
      solar_system_id: None,
      station_count,
      station_system_count,
    }
  }

  pub fn set_corporation_id(&mut self, id: i64) {
    self.corporation_id = Some(id);
  }

  pub fn set_description(&mut self, description: impl Into<String>) {
    self.description = description.into();
  }

  pub fn set_militia_corporation_id(&mut self, id: i64) {
    self.militia_corporation_id = Some(id);
  }

  pub fn set_solar_system_id(&mut self, id: i64) {
    self.solar_system_id = Some(id);
  }
}

impl From<Faction> for Model {
  fn from(faction: Faction) -> Self {
    let mut model = Self::new(
      faction.faction_id,
      faction.name,
      faction.is_unique,
      faction.size_factor,
      faction.station_count,
      faction.station_system_count,
    );

    model.set_description(faction.description);

    if let Some(corporation_id) = faction.corporation_id {
      model.set_corporation_id(corporation_id);
    }
    if let Some(militia_corporation_id) = faction.militia_corporation_id {
      model.set_militia_corporation_id(militia_corporation_id);
    }
    if let Some(solar_system_id) = faction.solar_system_id {
      model.set_solar_system_id(solar_system_id);
    }

    model
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_faction() -> Faction {
      Faction {
        corporation_id: None,
        description: "The Amarr Empire.".to_owned(),
        faction_id: 500_003,
        is_unique: true,
        militia_corporation_id: None,
        name: "Amarr Empire".to_owned(),
        size_factor: 5.0,
        solar_system_id: None,
        station_count: 1031,
        station_system_count: 508,
      }
    }

    #[test]
    fn it_encodes_is_unique_as_int_and_sets_description() {
      let model = Model::from(make_faction());

      assert_eq!(model.id(), 500_003);
      assert_eq!(model.is_unique(), 1);
      assert_eq!(model.description(), "The Amarr Empire.");
    }

    #[test]
    fn it_maps_optional_corporation_ids_when_present() {
      let mut faction = make_faction();
      faction.corporation_id = Some(1_000_084);
      faction.militia_corporation_id = Some(500_003);
      faction.solar_system_id = Some(30_002_187);

      let model = Model::from(faction);

      assert_eq!(model.corporation_id(), Some(1_000_084));
      assert_eq!(model.militia_corporation_id(), Some(500_003));
      assert_eq!(model.solar_system_id(), Some(30_002_187));
    }
  }
}
