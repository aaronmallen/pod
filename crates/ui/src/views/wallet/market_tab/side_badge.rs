//! Colored SELL / BUY badge chip for market entry rows.

use iced::{
  Background, Border, Element, Padding, Theme,
  widget::{container, text},
};

use crate::{
  style::{color, radius, typography::mono},
  views::wallet::market_tab::Message,
};

fn badge_fg_color(is_sell: bool) -> iced::Color {
  if is_sell {
    color::status::ONLINE
  } else {
    color::status::DANGER
  }
}

fn badge_bg_color(is_sell: bool) -> iced::Color {
  if is_sell {
    color::status::ONLINE_SUBTLE
  } else {
    color::status::DANGER_SUBTLE
  }
}

/// Builder for a sell / buy side badge.
pub struct SideBadge {
  is_sell: bool,
}

impl SideBadge {
  /// Creates a new side badge.
  ///
  /// Pass `true` for a sell badge, `false` for a buy badge.
  pub fn new(is_sell: bool) -> Self {
    Self {
      is_sell,
    }
  }

  /// Renders the badge into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let is_sell = self.is_sell;
    let fg = badge_fg_color(is_sell);
    let bg = badge_bg_color(is_sell);
    let label = if is_sell { "SELL" } else { "BUY" };
    container(
      text(label)
        .font(mono::MEDIUM)
        .size(9.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(fg),
        }),
    )
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: 6.0,
      right: 6.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(bg)),
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
  }
}
