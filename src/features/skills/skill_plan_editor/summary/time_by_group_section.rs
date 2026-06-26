use std::collections::HashMap;

use iced::{Element, widget::Space};

use super::{
  bar_chart::{bar_chart_row, time_chart_section},
  fmt_time_short, group_palette,
};
use crate::features::skills::skill_plan_editor::Message;

pub(super) fn time_by_group_section(group_sec: &HashMap<String, f64>) -> Element<'static, Message> {
  let mut entries: Vec<(&String, &f64)> = group_sec.iter().collect();
  entries.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

  let max_sec = entries.first().map(|&(_, s)| *s).unwrap_or(1.0);
  let palette = group_palette();

  let rows: Vec<Element<'static, Message>> = entries
    .iter()
    .enumerate()
    .flat_map(|(i, &(name, sec))| {
      let sec = *sec;
      let bar_color = palette[i % palette.len()];
      let fraction = if max_sec > 0.0 { (sec / max_sec) as f32 } else { 0.0 };
      [
        bar_chart_row(name.clone(), fmt_time_short(sec), fraction, bar_color),
        Space::new().height(6.0).into(),
      ]
    })
    .collect();

  time_chart_section("TIME BY SKILL GROUP", rows)
}
