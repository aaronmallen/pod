use std::collections::HashMap;

use iced::{Element, widget::Space};

use super::{
  bar_chart::{bar_chart_row, time_chart_section},
  fmt_time_short,
};
use crate::{features::skills::skill_plan_editor::Message, ui::style::color};

pub(super) fn time_by_pair_section(pair_sec: &HashMap<String, f64>) -> Element<'static, Message> {
  let mut entries: Vec<(&String, &f64)> = pair_sec.iter().collect();
  entries.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

  let max_sec = entries.first().map(|&(_, s)| *s).unwrap_or(1.0);
  let bar_color = color::accent::PLASMA;

  let rows: Vec<Element<'static, Message>> = entries
    .iter()
    .flat_map(|&(name, sec)| {
      let sec = *sec;
      let fraction = if max_sec > 0.0 { (sec / max_sec) as f32 } else { 0.0 };
      [
        bar_chart_row(name.clone(), fmt_time_short(sec), fraction, bar_color),
        Space::new().height(6.0).into(),
      ]
    })
    .collect();

  time_chart_section("TIME BY ATTRIBUTE PAIR", rows)
}
