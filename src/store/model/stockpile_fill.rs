use crate::store::model::{Stockpile, StockpileItem};

#[derive(Clone, Debug, PartialEq)]
pub struct StockpileFill {
  pub items: Vec<StockpileItemFill>,
  pub stockpile_id: i64,
}

impl StockpileFill {
  #[allow(dead_code)]
  pub fn is_full(&self) -> bool {
    self.items.iter().all(|i| i.have_quantity >= i.target_quantity)
  }

  pub fn overall_pct(&self) -> f64 {
    let total_target: i64 = self.items.iter().map(|i| i.target_quantity).sum();
    let total_have: i64 = self.items.iter().map(|i| i.have_quantity.min(i.target_quantity)).sum();
    fill_ratio(total_have, total_target)
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StockpileItemFill {
  pub have_quantity: i64,
  pub target_quantity: i64,
  pub type_id: i64,
}

impl StockpileItemFill {
  pub fn pct(&self) -> f64 {
    fill_ratio(self.have_quantity, self.target_quantity)
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StockpileWithItems {
  pub items: Vec<StockpileItem>,
  pub stockpile: Stockpile,
}

fn fill_ratio(have: i64, target: i64) -> f64 {
  if target <= 0 {
    return 1.0;
  }
  (have as f64 / target as f64).clamp(0.0, 1.0)
}
