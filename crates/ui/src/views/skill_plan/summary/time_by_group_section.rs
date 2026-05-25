//! Time-by-skill-group section: bar chart of training time per skill group.

use std::collections::HashMap;

use iced::{Element, widget::Space};

use super::{
  super::Message,
  GROUP_PALETTE,
  bar_chart::{bar_chart_row, time_chart_section},
  fmt_time_short,
};

/// Builder for the time-by-skill-group bar chart section.
pub struct TimeByGroupSection<'a> {
  /// Map of skill group name to seconds.
  group_sec: &'a HashMap<String, f64>,
}

impl<'a> TimeByGroupSection<'a> {
  /// Creates a new section builder.
  pub fn new(group_sec: &'a HashMap<String, f64>) -> Self {
    Self {
      group_sec,
    }
  }

  /// Renders the section into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let mut entries: Vec<(&String, &f64)> = self.group_sec.iter().collect();
    entries.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

    let max_sec = entries.first().map(|&(_, s)| *s).unwrap_or(1.0);

    let rows: Vec<Element<'static, Message>> = entries
      .iter()
      .enumerate()
      .flat_map(|(i, &(name, sec))| {
        let sec = *sec;
        let color = GROUP_PALETTE[i % GROUP_PALETTE.len()];
        let time_str = fmt_time_short(sec);
        let fraction = if max_sec > 0.0 { (sec / max_sec) as f32 } else { 0.0 };
        let name_str = name.as_str().to_string();
        [
          bar_chart_row(name_str, time_str, fraction, color),
          Space::new().height(6.0).into(),
        ]
      })
      .collect();

    time_chart_section("TIME BY SKILL GROUP", rows)
  }
}
