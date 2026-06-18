use chrono::{Duration, NaiveDate};

use crate::store::model::{CharacterNetWorthSnapshot, CombinedNetWorthPoint};

#[derive(Clone, Copy, Debug, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub struct PeriodDelta {
  pub absolute: f64,
  pub end: f64,
  pub percent: f64,
  pub start: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub enum Scope {
  Character(i64),
  Combined,
}

#[derive(Clone, Debug, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub struct SeriesPoint {
  pub asset_value: Option<f64>,
  pub date: String,
  pub escrow: Option<f64>,
  pub liquid: Option<f64>,
  pub net_worth: Option<f64>,
}

impl From<CharacterNetWorthSnapshot> for SeriesPoint {
  fn from(row: CharacterNetWorthSnapshot) -> Self {
    Self {
      asset_value: row.asset_value(),
      date: row.date().clone(),
      escrow: row.escrow(),
      liquid: Some(row.liquid()),
      net_worth: Some(row.net_worth()),
    }
  }
}

impl From<CombinedNetWorthPoint> for SeriesPoint {
  fn from(row: CombinedNetWorthPoint) -> Self {
    Self {
      asset_value: row.asset_value(),
      date: row.date().clone(),
      escrow: row.escrow(),
      liquid: row.liquid(),
      net_worth: row.net_worth(),
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub enum Timeframe {
  HalfYear,
  Month,
  Quarter,
  Week,
  Year,
}

impl Timeframe {
  // Public store API exercised by unit tests; not yet wired into a production call site.
  #[allow(dead_code)]
  pub fn days(self) -> i64 {
    match self {
      Timeframe::Week => 7,
      Timeframe::Month => 30,
      Timeframe::Quarter => 90,
      Timeframe::HalfYear => 180,
      Timeframe::Year => 365,
    }
  }

  // Public store API exercised by unit tests; not yet wired into a production call site.
  #[allow(dead_code)]
  pub fn since(self, today: NaiveDate) -> String {
    let start = today - Duration::days(self.days() - 1);
    start.format("%Y-%m-%d").to_string()
  }
}
