use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct Model {
  pub fuel_expires: Option<String>,
  pub next_reinforce_apply: Option<String>,
  pub next_reinforce_hour: Option<i64>,
  pub next_reinforce_weekday: Option<i64>,
  pub reinforce_hour: Option<i64>,
  pub services: String,
  pub state: Option<String>,
  pub state_timer_end: Option<String>,
  pub state_timer_start: Option<String>,
  pub structure_id: i64,
  pub synced_at: String,
  pub unanchors_at: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Service {
  pub name: String,
  pub state: String,
}

#[allow(dead_code)]
impl Model {
  pub fn new(structure_id: i64, synced_at: impl Into<String>) -> Self {
    Self {
      fuel_expires: None,
      next_reinforce_apply: None,
      next_reinforce_hour: None,
      next_reinforce_weekday: None,
      reinforce_hour: None,
      services: "[]".to_owned(),
      state: None,
      state_timer_end: None,
      state_timer_start: None,
      structure_id,
      synced_at: synced_at.into(),
      unanchors_at: None,
    }
  }

  pub fn service_list(&self) -> Vec<Service> {
    serde_json::from_str(&self.services).unwrap_or_default()
  }

  pub fn set_service_list(&mut self, services: &[Service]) {
    self.services = serde_json::to_string(services).unwrap_or_else(|_| "[]".to_owned());
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod service_list {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_an_empty_list_for_the_default_services() {
      let model = Model::new(1_030_000_000_001, "2026-07-15T00:00:00Z");

      assert_eq!(model.service_list(), Vec::new());
    }

    #[test]
    fn it_round_trips_services_through_the_json_column() {
      let mut model = Model::new(1_030_000_000_001, "2026-07-15T00:00:00Z");
      let services = vec![
        Service {
          name: "Clone Bay".to_owned(),
          state: "online".to_owned(),
        },
        Service {
          name: "Manufacturing".to_owned(),
          state: "offline".to_owned(),
        },
      ];

      model.set_service_list(&services);

      assert_eq!(model.service_list(), services);
    }

    #[test]
    fn it_falls_back_to_an_empty_list_for_unparseable_json() {
      let mut model = Model::new(1_030_000_000_001, "2026-07-15T00:00:00Z");
      model.services = "not json".to_owned();

      assert_eq!(model.service_list(), Vec::new());
    }
  }
}
