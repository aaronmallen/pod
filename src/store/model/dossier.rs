use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, FromRow, PartialEq)]
pub struct Model {
  pub character_id: i64,
  pub created_at: String,
  pub near_term: Option<String>,
  pub purpose: Option<String>,
  pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, FromRow, PartialEq)]
pub struct Order {
  pub character_id: i64,
  pub created_at: String,
  pub id: i64,
  pub objective_id: Option<i64>,
  pub position: i64,
  pub status: String,
  pub text: String,
  pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, FromRow, PartialEq)]
pub struct ObjectiveOrder {
  pub character_id: i64,
  pub character_name: String,
  pub id: i64,
  pub status: String,
  pub text: String,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
  #[default]
  Active,
  Cancelled,
  Complete,
}

#[allow(dead_code)]
impl Status {
  pub const ALL: [Status; 3] = [Status::Active, Status::Complete, Status::Cancelled];

  pub fn as_str(self) -> &'static str {
    match self {
      Status::Active => "active",
      Status::Cancelled => "cancelled",
      Status::Complete => "complete",
    }
  }

  pub fn parse(value: &str) -> Option<Status> {
    match value {
      "active" => Some(Status::Active),
      "cancelled" => Some(Status::Cancelled),
      "complete" => Some(Status::Complete),
      _ => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod status {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_every_variant_through_its_wire_string() {
      for status in Status::ALL {
        assert_eq!(Status::parse(status.as_str()), Some(status));
      }
    }

    #[test]
    fn it_rejects_an_unknown_status() {
      assert_eq!(Status::parse("archived"), None);
      assert_eq!(Status::parse("Active"), None);
      assert_eq!(Status::parse(""), None);
    }
  }
}
