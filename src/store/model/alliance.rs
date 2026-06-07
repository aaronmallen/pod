use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::alliance::AllianceInfo;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  creator_corporation_id: i64,
  #[getset(get_copy = "pub")]
  creator_id: i64,
  #[getset(get = "pub")]
  date_founded: String,
  #[getset(get_copy = "pub")]
  executor_corporation_id: Option<i64>,
  #[getset(get_copy = "pub")]
  faction_id: Option<i64>,
  #[getset(get_copy = "pub")]
  id: i64,
  #[getset(get = "pub")]
  name: String,
  #[getset(get = "pub")]
  ticker: String,
}

impl Model {
  pub fn new(
    id: i64,
    creator_corporation_id: i64,
    creator_id: i64,
    date_founded: impl Into<String>,
    name: impl Into<String>,
    ticker: impl Into<String>,
  ) -> Self {
    Self {
      creator_corporation_id,
      creator_id,
      date_founded: date_founded.into(),
      executor_corporation_id: None,
      faction_id: None,
      id,
      name: name.into(),
      ticker: ticker.into(),
    }
  }

  pub fn set_executor_corporation_id(&mut self, id: i64) {
    self.executor_corporation_id = Some(id);
  }

  pub fn set_faction_id(&mut self, id: i64) {
    self.faction_id = Some(id);
  }
}

impl From<(i64, AllianceInfo)> for Model {
  fn from((id, info): (i64, AllianceInfo)) -> Self {
    let mut model = Self::new(
      id,
      info.creator_corporation_id,
      info.creator_id,
      info.date_founded,
      info.name,
      info.ticker,
    );

    if let Some(executor_corporation_id) = info.executor_corporation_id {
      model.set_executor_corporation_id(executor_corporation_id);
    }
    if let Some(faction_id) = info.faction_id {
      model.set_faction_id(faction_id);
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

    fn make_info() -> AllianceInfo {
      AllianceInfo {
        creator_corporation_id: 98,
        creator_id: 99,
        date_founded: "2010-01-01T00:00:00Z".to_owned(),
        executor_corporation_id: None,
        faction_id: None,
        name: "Test Alliance".to_owned(),
        ticker: "TEST".to_owned(),
      }
    }

    #[test]
    fn it_applies_the_path_id_and_required_fields() {
      let model = Model::from((42, make_info()));

      assert_eq!(model.id(), 42);
      assert_eq!(model.creator_corporation_id(), 98);
      assert_eq!(model.name(), "Test Alliance");
    }

    #[test]
    fn it_maps_optional_fields_when_present() {
      let mut info = make_info();
      info.executor_corporation_id = Some(500);
      info.faction_id = Some(600);

      let model = Model::from((42, info));

      assert_eq!(model.executor_corporation_id(), Some(500));
      assert_eq!(model.faction_id(), Some(600));
    }

    #[test]
    fn it_leaves_optional_fields_none_when_absent() {
      let model = Model::from((42, make_info()));

      assert_eq!(model.executor_corporation_id(), None);
      assert_eq!(model.faction_id(), None);
    }
  }
}
