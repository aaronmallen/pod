#![cfg_attr(not(test), allow(dead_code))]

use chrono::NaiveDate;

use crate::clients::esi::models::market::MarketHistory;

pub const DONCHIAN_WINDOW: usize = 5;

const PRICE_PAD_RATIO: f64 = 0.08;
const FLAT_PAD_RATIO: f64 = 0.05;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Range {
  OneMonth,
  #[default]
  ThreeMonths,
  SixMonths,
  OneYear,
}

impl Range {
  pub const ORDER: [Range; 4] = [Range::OneMonth, Range::ThreeMonths, Range::SixMonths, Range::OneYear];

  pub fn days(self) -> usize {
    match self {
      Range::OneMonth => 30,
      Range::ThreeMonths => 90,
      Range::SixMonths => 180,
      Range::OneYear => 365,
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      Range::OneMonth => "1M",
      Range::ThreeMonths => "3M",
      Range::SixMonths => "6M",
      Range::OneYear => "1Y",
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HistoryPoint {
  pub date: NaiveDate,
  pub median: f64,
  pub high: f64,
  pub low: f64,
  pub volume: i64,
  pub orders: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Channel {
  pub hi: f64,
  pub lw: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PriceBounds {
  pub min: f64,
  pub max: f64,
}

pub fn series(history: &[MarketHistory]) -> Vec<HistoryPoint> {
  history.iter().filter_map(point).collect()
}

fn point(entry: &MarketHistory) -> Option<HistoryPoint> {
  Some(HistoryPoint {
    date: parse_day(&entry.date)?,
    median: entry.average,
    high: entry.highest,
    low: entry.lowest,
    volume: entry.volume,
    orders: entry.order_count,
  })
}

fn parse_day(date: &str) -> Option<NaiveDate> {
  let prefix = date.split('T').next().unwrap_or(date);
  NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok()
}

pub fn donchian(points: &[HistoryPoint], window: usize) -> Vec<Channel> {
  (0..points.len())
    .map(|index| channel_at(points, index, window))
    .collect()
}

fn channel_at(points: &[HistoryPoint], index: usize, window: usize) -> Channel {
  let start = index.saturating_sub(window.saturating_sub(1));
  let mut hi = f64::NEG_INFINITY;
  let mut lw = f64::INFINITY;
  for point in &points[start..=index] {
    hi = hi.max(point.high);
    lw = lw.min(point.low);
  }
  Channel {
    hi,
    lw,
  }
}

pub fn slice(points: &[HistoryPoint], days: usize) -> &[HistoryPoint] {
  let start = points.len().saturating_sub(days);
  &points[start..]
}

pub fn slice_range(points: &[HistoryPoint], range: Range) -> &[HistoryPoint] {
  slice(points, range.days())
}

pub fn price_bounds(points: &[HistoryPoint]) -> Option<PriceBounds> {
  let mut min = f64::INFINITY;
  let mut max = f64::NEG_INFINITY;
  for point in points {
    min = min.min(point.low);
    max = max.max(point.high);
  }
  if !min.is_finite() || !max.is_finite() {
    return None;
  }
  let pad = match (max - min) * PRICE_PAD_RATIO {
    span if span > 0.0 => span,
    _ => max * FLAT_PAD_RATIO,
  };
  Some(PriceBounds {
    min: min - pad,
    max: max + pad,
  })
}

pub fn max_volume(points: &[HistoryPoint]) -> i64 {
  points.iter().map(|point| point.volume).max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn entry(date: &str, low: f64, high: f64, average: f64, volume: i64, orders: i64) -> MarketHistory {
    MarketHistory {
      average,
      date: date.to_owned(),
      highest: high,
      lowest: low,
      order_count: orders,
      volume,
    }
  }

  fn point_at(date: &str, low: f64, high: f64) -> HistoryPoint {
    HistoryPoint {
      date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
      median: (low + high) / 2.0,
      high,
      low,
      volume: 0,
      orders: 0,
    }
  }

  mod range {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_each_range_to_its_day_count() {
      assert_eq!(Range::OneMonth.days(), 30);
      assert_eq!(Range::ThreeMonths.days(), 90);
      assert_eq!(Range::SixMonths.days(), 180);
      assert_eq!(Range::OneYear.days(), 365);
    }

    #[test]
    fn it_labels_each_range() {
      assert_eq!(Range::OneMonth.label(), "1M");
      assert_eq!(Range::ThreeMonths.label(), "3M");
      assert_eq!(Range::SixMonths.label(), "6M");
      assert_eq!(Range::OneYear.label(), "1Y");
    }

    #[test]
    fn it_defaults_to_three_months() {
      assert_eq!(Range::default(), Range::ThreeMonths);
    }

    #[test]
    fn it_orders_the_ranges_from_shortest_to_longest() {
      let days: Vec<usize> = Range::ORDER.iter().map(|range| range.days()).collect();

      assert_eq!(days, vec![30, 90, 180, 365]);
    }
  }

  mod series {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_empty_for_no_history() {
      assert!(series(&[]).is_empty());
    }

    #[test]
    fn it_maps_esi_fields_onto_the_render_point() {
      let points = series(&[entry("2026-07-01", 4.0, 6.0, 5.0, 120, 8)]);

      assert_eq!(points.len(), 1);
      assert_eq!(points[0].date, NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
      assert_eq!(points[0].median, 5.0);
      assert_eq!(points[0].high, 6.0);
      assert_eq!(points[0].low, 4.0);
      assert_eq!(points[0].volume, 120);
      assert_eq!(points[0].orders, 8);
    }

    #[test]
    fn it_tolerates_an_iso_timestamp_suffix() {
      let points = series(&[entry("2026-07-01T00:00:00Z", 4.0, 6.0, 5.0, 1, 1)]);

      assert_eq!(points[0].date, NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
    }

    #[test]
    fn it_drops_unparseable_dates() {
      let points = series(&[entry("not-a-date", 4.0, 6.0, 5.0, 1, 1)]);

      assert!(points.is_empty());
    }
  }

  mod donchian {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_empty_for_no_points() {
      assert!(donchian(&[], DONCHIAN_WINDOW).is_empty());
    }

    #[test]
    fn it_returns_the_single_day_envelope_for_one_point() {
      let channels = donchian(&[point_at("2026-07-01", 4.0, 6.0)], DONCHIAN_WINDOW);

      assert_eq!(
        channels,
        vec![Channel {
          hi: 6.0,
          lw: 4.0
        }]
      );
    }

    #[test]
    fn it_rolls_the_high_and_low_over_a_trailing_window() {
      let points = vec![
        point_at("2026-07-01", 4.0, 6.0),
        point_at("2026-07-02", 3.0, 7.0),
        point_at("2026-07-03", 5.0, 6.5),
      ];

      let channels = donchian(&points, 2);

      assert_eq!(
        channels,
        vec![
          Channel {
            hi: 6.0,
            lw: 4.0
          },
          Channel {
            hi: 7.0,
            lw: 3.0
          },
          Channel {
            hi: 7.0,
            lw: 3.0
          },
        ]
      );
    }

    #[test]
    fn it_widens_the_window_only_up_to_the_available_history() {
      let points = vec![point_at("2026-07-01", 4.0, 6.0), point_at("2026-07-02", 1.0, 9.0)];

      let channels = donchian(&points, 10);

      assert_eq!(
        channels,
        vec![
          Channel {
            hi: 6.0,
            lw: 4.0
          },
          Channel {
            hi: 9.0,
            lw: 1.0
          }
        ]
      );
    }
  }

  mod slice {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_trailing_days() {
      let points: Vec<HistoryPoint> = (1..=5)
        .map(|day| point_at(&format!("2026-07-0{day}"), day as f64, day as f64 + 1.0))
        .collect();

      let tail = slice(&points, 2);

      assert_eq!(tail.len(), 2);
      assert_eq!(tail[0].low, 4.0);
      assert_eq!(tail[1].low, 5.0);
    }

    #[test]
    fn it_returns_the_whole_series_when_the_window_exceeds_the_data() {
      let points = vec![point_at("2026-07-01", 1.0, 2.0)];

      assert_eq!(slice(&points, 90).len(), 1);
    }

    #[test]
    fn it_is_empty_for_an_empty_series() {
      assert!(slice(&[], 30).is_empty());
    }

    #[test]
    fn it_slices_by_a_named_range() {
      let points: Vec<HistoryPoint> = (0..100)
        .map(|day| point_at("2026-07-01", day as f64, day as f64 + 1.0))
        .collect();

      assert_eq!(slice_range(&points, Range::OneMonth).len(), 30);
      assert_eq!(slice_range(&points, Range::OneYear).len(), 100);
    }
  }

  mod bounds {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_has_no_bounds_for_an_empty_series() {
      assert_eq!(price_bounds(&[]), None);
    }

    #[test]
    fn it_pads_the_price_range_by_eight_percent() {
      let points = vec![point_at("2026-07-01", 10.0, 20.0)];

      let bounds = price_bounds(&points).unwrap();

      assert_eq!(bounds.min, 10.0 - 0.8);
      assert_eq!(bounds.max, 20.0 + 0.8);
    }

    #[test]
    fn it_falls_back_to_a_percentage_of_the_peak_when_flat() {
      let points = vec![point_at("2026-07-01", 50.0, 50.0), point_at("2026-07-02", 50.0, 50.0)];

      let bounds = price_bounds(&points).unwrap();

      assert_eq!(bounds.min, 50.0 - 2.5);
      assert_eq!(bounds.max, 50.0 + 2.5);
    }

    #[test]
    fn it_takes_the_peak_volume() {
      let mut points = vec![point_at("2026-07-01", 1.0, 2.0), point_at("2026-07-02", 1.0, 2.0)];
      points[0].volume = 30;
      points[1].volume = 90;

      assert_eq!(max_volume(&points), 90);
    }

    #[test]
    fn it_reports_zero_peak_volume_for_no_points() {
      assert_eq!(max_volume(&[]), 0);
    }
  }
}
