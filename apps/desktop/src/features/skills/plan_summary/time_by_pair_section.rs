use std::collections::HashMap;

use iced::{Element, widget::Space};

use super::{
  bar_chart::{bar_chart_row, time_chart_section},
  fmt_time_short, sorted_time_entries,
};
use crate::ui::style::color;

pub(crate) fn time_by_pair_section<'a, M: 'a>(pair_sec: &HashMap<String, f64>) -> Element<'a, M> {
  let entries = sorted_time_entries(pair_sec);

  let max_sec = entries.first().map(|&(_, s)| *s).unwrap_or(1.0);
  let bar_color = color::accent();

  let rows: Vec<Element<'a, M>> = entries
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

  time_chart_section(&t!("skills.summary_time.by_attribute_pair"), rows)
}
