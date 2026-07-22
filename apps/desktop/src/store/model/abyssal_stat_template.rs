#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StatRange {
  pub max: f64,
  pub min: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StatTemplate {
  pub attribute_id: i64,
  pub base_value: f64,
  pub bound_hi: f64,
  pub bound_lo: f64,
  pub display_name: String,
  pub high_is_good: bool,
  pub unit_id: Option<i64>,
}
