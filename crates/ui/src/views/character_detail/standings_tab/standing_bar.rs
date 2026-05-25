//! Bar visualisation for a single standing value.

use iced::{
  Background, Border, Color, Element, Theme,
  widget::{Space, container, row},
};

use crate::{style::color, views::character_detail::Message};

/// Builder for the standing bar visualisation.
pub struct StandingBar {
  value: f64,
}

impl StandingBar {
  /// Creates a new standing bar for the given standing value.
  pub fn new(value: f64) -> Self {
    Self {
      value,
    }
  }

  /// Renders the standing bar into an iced element.
  pub fn render<'a>(self) -> Element<'a, Message> {
    standing_bar(self.value)
  }
}

fn standing_color(v: f64) -> Color {
  if v >= 5.0 {
    color::status::ONLINE
  } else if v > 0.0 {
    color::status::ONLINE_STRONG
  } else if v >= -0.01 {
    color::text::SECONDARY
  } else if v > -5.0 {
    color::status::DANGER_STRONG
  } else {
    color::status::DANGER
  }
}

fn standing_bar_fill<'a>(bar_color: Color, fill_width: f32) -> Element<'a, Message> {
  container(Space::new().width(fill_width).height(6.0))
    .width(fill_width)
    .height(6.0)
    .style(move |_: &Theme| container::Style {
      background: Some(Background::Color(bar_color)),
      border: Border {
        radius: 3.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn standing_bar<'a>(value: f64) -> Element<'a, Message> {
  let bar_color = standing_color(value);
  let pct = (value.abs() / 10.0 * 50.0).min(50.0) as f32;
  let positive = value >= 0.0;
  let fill_width = (220.0 * pct / 100.0).max(0.0);
  let fill = standing_bar_fill(bar_color, fill_width);
  let bar_inner: Element<'_, Message> = if positive {
    row([Space::new().width(110.0).height(6.0).into(), fill])
      .height(6.0)
      .into()
  } else {
    let fill_start = 110.0 - fill_width;
    row([Space::new().width(fill_start).height(6.0).into(), fill])
      .height(6.0)
      .into()
  };
  container(bar_inner)
    .width(220.0)
    .height(6.0)
    .style(|_: &Theme| container::Style {
      background: Some(Background::Color(color::state::HOVER_OVERLAY)),
      border: Border {
        radius: 3.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}
