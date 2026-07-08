use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::corporation::CorporationDivisionName;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub corporation_id: i64,
  #[getset(get_copy = "pub")]
  pub division: i64,
  #[getset(get = "pub")]
  pub name: Option<String>,
}

impl From<(i64, CorporationDivisionName)> for Model {
  fn from((corporation_id, division): (i64, CorporationDivisionName)) -> Self {
    Self {
      corporation_id,
      division: i64::from(division.division),
      name: division.name,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_a_named_division_entry() {
      let entry = CorporationDivisionName {
        division: 3,
        name: Some("Reactions".to_owned()),
      };

      let model = Model::from((90_000_001, entry));

      assert_eq!(model.corporation_id(), 90_000_001);
      assert_eq!(model.division(), 3);
      assert_eq!(model.name(), &Some("Reactions".to_owned()));
    }

    #[test]
    fn it_maps_an_unnamed_division_entry() {
      let entry = CorporationDivisionName {
        division: 5,
        name: None,
      };

      let model = Model::from((90_000_001, entry));

      assert_eq!(model.corporation_id(), 90_000_001);
      assert_eq!(model.division(), 5);
      assert_eq!(model.name(), &None);
    }
  }
}
