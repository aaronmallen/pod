//! Stat cells shown in the assets header (value, volume, items, locations).

use iced::{
  Element, Theme,
  widget::{Space, row},
};

use super::super::{Message, State};
use crate::{
  components::HeadStat,
  format,
  style::{color, spacing},
};

fn separator_v<'a>() -> Element<'a, Message> {
  use iced::{
    Background,
    widget::{Space as Sp, container},
  };
  container(Sp::new().width(1.0).height(32.0))
    .width(1.0)
    .height(32.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into()
}

fn change_accent(seed: i64) -> Element<'static, Message> {
  use iced::widget::text;

  use crate::style::typography::mono;

  let change30 = ((seed * 173) % 14) as f64 / 100.0 * if seed % 2 == 0 { 1.0 } else { -1.0 } + 0.0612;
  let change_color = if change30 >= 0.0 {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let change_glyph = if change30 >= 0.0 { "▲" } else { "▼" };
  let change_str = format!("{} {}", change_glyph, format::fmt_pct(change30.abs()));
  text(change_str)
    .font(mono::MEDIUM)
    .size(11.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(change_color),
    })
    .into()
}

/// Builder for the assets header stat cells row.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new stat cells row for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the stat cells into a row element.
  pub fn render(self) -> Element<'a, Message> {
    let seed = self.state.selected_character().unwrap_or(0);
    let value_accent = change_accent(seed);
    let value_el = HeadStat::new("Asset Value", format::fmt_isk(self.state.total_value()))
      .accent(value_accent)
      .render();
    let vol_el = HeadStat::new("Volume", format::fmt_vol(self.state.total_volume())).render();
    let cnt_el = HeadStat::new("Items", format::fmt_count(self.state.total_count())).render();
    let locs_el = HeadStat::new("Locations", self.state.total_locations().to_string()).render();

    row([
      separator_v(),
      value_el,
      separator_v(),
      vol_el,
      separator_v(),
      cnt_el,
      separator_v(),
      locs_el,
      Space::new().width(iced::Length::Fill).into(),
    ])
    .spacing(spacing::SPACE_8)
    .align_y(iced::alignment::Vertical::Center)
    .into()
  }
}
