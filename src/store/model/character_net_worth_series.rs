use chrono::{Duration, NaiveDate};

use crate::store::model::{CharacterNetWorthSnapshot, CombinedNetWorthPoint};

#[derive(Clone, Copy, Debug, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
pub struct PeriodDelta {
  pub absolute: f64,
  pub end: f64,
  pub percent: f64,
  pub start: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
pub enum Scope {
  #[cfg_attr(not(test), expect(dead_code))]
  Character(i64),
  #[cfg_attr(not(test), expect(dead_code))]
  Combined,
}

#[derive(Clone, Debug, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
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
pub enum Timeframe {
  #[cfg_attr(not(test), expect(dead_code))]
  HalfYear,
  #[cfg_attr(not(test), expect(dead_code))]
  Month,
  #[cfg_attr(not(test), expect(dead_code))]
  Quarter,
  #[cfg_attr(not(test), expect(dead_code))]
  Week,
  #[cfg_attr(not(test), expect(dead_code))]
  Year,
}

impl Timeframe {
  // Public store API exercised by unit tests; not yet wired into a production call site.
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
  pub fn since(self, today: NaiveDate) -> String {
    let start = today - Duration::days(self.days() - 1);
    start.format("%Y-%m-%d").to_string()
  }
}
