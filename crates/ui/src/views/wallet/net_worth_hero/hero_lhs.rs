//! Left-hand side of the net worth hero: label, value, and change badge.

use iced::{
  Background, Border, Color, Element, Padding, Theme,
  widget::{Space, column, container, text},
};

use crate::{
  format,
  style::{
    color,
    typography::{body, mono},
  },
  views::wallet::Message,
};

/// Builder for the hero left-hand side (NET WORTH label, value, and change badge).
pub struct HeroLhs {
  /// The absolute change in ISK.
  change: f64,
  /// The percentage change.
  change_pct: f64,
  /// The current net worth value.
  current: f64,
  /// Whether the change is positive (up) or negative (down).
  is_up: bool,
}

impl HeroLhs {
  /// Creates a new `HeroLhs` builder.
  pub fn new(current: f64, change: f64, change_pct: f64, is_up: bool) -> Self {
    Self {
      change,
      change_pct,
      current,
      is_up,
    }
  }

  /// Renders the hero LHS into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let change_color = if self.is_up {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };
    let hero_label = net_worth_label();
    let hero_value = net_worth_value(self.current);
    let change_badge = change_badge(self.change, self.change_pct, self.is_up, change_color);
    column([
      hero_label,
      Space::new().height(6.0).into(),
      hero_value,
      Space::new().height(8.0).into(),
      change_badge,
    ])
    .into()
  }
}

fn net_worth_label<'a>() -> Element<'a, Message> {
  text("NET WORTH")
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
    .into()
}

fn net_worth_value(current: f64) -> Element<'static, Message> {
  text(format::fmt_isk_full(current))
    .font(body::MEDIUM)
    .size(32.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    })
    .into()
}

fn change_badge(change: f64, change_pct: f64, is_up: bool, change_color: Color) -> Element<'static, Message> {
  let change_sign = if is_up { "▲" } else { "▼" };
  let pct_sign = if change_pct >= 0.0 { "+" } else { "-" };
  let change_str = format!(
    "{} {} · {}{:.2}%",
    change_sign,
    format::fmt_isk(change.abs()),
    pct_sign,
    change_pct.abs(),
  );
  container(
    text(change_str)
      .font(mono::MEDIUM)
      .size(11.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(change_color),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 10.0,
    right: 10.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(change_color, 0.10))),
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}
