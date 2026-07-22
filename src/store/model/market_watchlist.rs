use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Clone, Debug, Default, FromRow, PartialEq)]
pub struct Model {
  pub created_at: String,
  pub direction: String,
  pub id: i64,
  pub location_id: Option<i64>,
  pub location_tier: Option<String>,
  pub region_id: Option<i64>,
  pub target_price: Option<f64>,
  pub type_id: i64,
  pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NewWatch {
  pub direction: Direction,
  pub location_id: Option<i64>,
  pub location_tier: Option<String>,
  pub region_id: Option<i64>,
  pub target_price: Option<f64>,
  pub type_id: i64,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
  #[default]
  Buy,
  Sell,
}

#[allow(dead_code)]
impl Direction {
  pub const ALL: [Direction; 2] = [Direction::Buy, Direction::Sell];

  pub fn as_str(self) -> &'static str {
    match self {
      Direction::Buy => "buy",
      Direction::Sell => "sell",
    }
  }

  pub fn parse(value: &str) -> Option<Direction> {
    match value {
      "buy" => Some(Direction::Buy),
      "sell" => Some(Direction::Sell),
      _ => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod direction {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_every_variant_through_its_wire_string() {
      for direction in Direction::ALL {
        assert_eq!(Direction::parse(direction.as_str()), Some(direction));
      }
    }

    #[test]
    fn it_rejects_an_unknown_direction() {
      assert_eq!(Direction::parse("hold"), None);
      assert_eq!(Direction::parse("Buy"), None);
      assert_eq!(Direction::parse(""), None);
    }
  }
}
