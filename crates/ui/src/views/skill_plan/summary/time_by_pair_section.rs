//! Time-by-attribute-pair section: bar chart of training time per attribute pair.

use std::collections::HashMap;

use iced::{Element, widget::Space};

use super::{
  super::Message,
  bar_chart::{bar_chart_row, time_chart_section},
  fmt_time_short,
};
use crate::style::color;

/// Builder for the time-by-attribute-pair bar chart section.
pub struct TimeByPairSection<'a> {
  /// Map of attribute pair label to seconds.
  pair_sec: &'a HashMap<String, f64>,
}

impl<'a> TimeByPairSection<'a> {
  /// Creates a new section builder.
  pub fn new(pair_sec: &'a HashMap<String, f64>) -> Self {
    Self {
      pair_sec,
    }
  }

  /// Renders the section into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let mut entries: Vec<(&String, &f64)> = self.pair_sec.iter().collect();
    entries.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

    let max_sec = entries.first().map(|&(_, s)| *s).unwrap_or(1.0);
    let bar_color = color::accent::PLASMA;

    let rows: Vec<Element<'static, Message>> = entries
      .iter()
      .flat_map(|&(name, sec)| {
        let sec = *sec;
        let time_str = fmt_time_short(sec);
        let fraction = if max_sec > 0.0 { (sec / max_sec) as f32 } else { 0.0 };
        let name_str = name.as_str().to_string();
        [
          bar_chart_row(name_str, time_str, fraction, bar_color),
          Space::new().height(6.0).into(),
        ]
      })
      .collect();

    time_chart_section("TIME BY ATTRIBUTE PAIR", rows)
  }
}
