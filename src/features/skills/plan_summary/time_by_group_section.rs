use std::collections::HashMap;

use iced::{Element, widget::Space};

use super::{
  bar_chart::{bar_chart_row, time_chart_section},
  fmt_time_short, group_palette, sorted_time_entries,
};

pub(crate) fn time_by_group_section<'a, M: 'a>(group_sec: &HashMap<String, f64>) -> Element<'a, M> {
  let entries = sorted_time_entries(group_sec);

  let max_sec = entries.first().map(|&(_, s)| *s).unwrap_or(1.0);
  let palette = group_palette();

  let rows: Vec<Element<'a, M>> = entries
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

  time_chart_section(&t!("skills.summary_time.by_skill_group"), rows)
}
