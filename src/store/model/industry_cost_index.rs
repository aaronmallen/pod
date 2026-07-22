use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Default, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub copying: Option<f64>,
  #[getset(get_copy = "pub")]
  pub invention: Option<f64>,
  #[getset(get_copy = "pub")]
  pub manufacturing: Option<f64>,
  #[getset(get_copy = "pub")]
  pub reaction: Option<f64>,
  #[getset(get_copy = "pub")]
  pub researching_material_efficiency: Option<f64>,
  #[getset(get_copy = "pub")]
  pub researching_time_efficiency: Option<f64>,
  #[getset(get_copy = "pub")]
  pub solar_system_id: i64,
}

impl Model {
  /// Returns the cost index for an ESI activity_id integer (1=manufacturing, 3=research time,
  /// 4=research material, 5=copying, 8=invention, 9 and 11=reaction — both map to the same
  /// column because 9 is the legacy pre-Lifeblood reaction id and 11 is the current one).
  pub fn for_activity(&self, activity_id: i64) -> Option<f64> {
    match activity_id {
      1 => self.manufacturing,
      3 => self.researching_time_efficiency,
      4 => self.researching_material_efficiency,
      5 => self.copying,
      8 => self.invention,
      9 | 11 => self.reaction,
      _ => None,
    }
  }

  pub fn set_activity(&mut self, activity: &str, cost_index: f64) {
    match activity {
      "copying" => self.copying = Some(cost_index),
      "invention" => self.invention = Some(cost_index),
      "manufacturing" => self.manufacturing = Some(cost_index),
      "reaction" => self.reaction = Some(cost_index),
      "researching_material_efficiency" => self.researching_material_efficiency = Some(cost_index),
      "researching_time_efficiency" => self.researching_time_efficiency = Some(cost_index),
      _ => {}
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod for_activity {
    use pretty_assertions::assert_eq;

    use super::*;

    fn sample() -> Model {
      Model {
        copying: Some(0.05),
        invention: Some(0.06),
        manufacturing: Some(0.01),
        reaction: Some(0.02),
        researching_material_efficiency: Some(0.04),
        researching_time_efficiency: Some(0.03),
        solar_system_id: 30_000_142,
      }
    }

    #[test]
    fn it_maps_each_known_activity_id_to_its_column() {
      let model = sample();

      assert_eq!(model.for_activity(1), Some(0.01));
      assert_eq!(model.for_activity(3), Some(0.03));
      assert_eq!(model.for_activity(4), Some(0.04));
      assert_eq!(model.for_activity(5), Some(0.05));
      assert_eq!(model.for_activity(8), Some(0.06));
      assert_eq!(model.for_activity(9), Some(0.02));
      assert_eq!(model.for_activity(11), Some(0.02));
    }

    #[test]
    fn it_returns_none_for_an_unmapped_activity_id() {
      assert_eq!(sample().for_activity(99), None);
    }

    #[test]
    fn it_returns_none_when_the_mapped_column_is_absent() {
      let model = Model {
        solar_system_id: 30_000_142,
        ..Model::default()
      };

      assert_eq!(model.for_activity(1), None);
    }
  }

  mod set_activity {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_assigns_each_known_activity_string_to_its_column() {
      let mut model = Model::default();

      model.set_activity("manufacturing", 0.01);
      model.set_activity("reaction", 0.02);
      model.set_activity("researching_time_efficiency", 0.03);
      model.set_activity("researching_material_efficiency", 0.04);
      model.set_activity("copying", 0.05);
      model.set_activity("invention", 0.06);

      assert_eq!(model.manufacturing, Some(0.01));
      assert_eq!(model.reaction, Some(0.02));
      assert_eq!(model.researching_time_efficiency, Some(0.03));
      assert_eq!(model.researching_material_efficiency, Some(0.04));
      assert_eq!(model.copying, Some(0.05));
      assert_eq!(model.invention, Some(0.06));
    }

    #[test]
    fn it_ignores_an_unknown_activity_string() {
      let mut model = Model::default();

      model.set_activity("none", 0.99);

      assert_eq!(model, Model::default());
    }
  }
}
