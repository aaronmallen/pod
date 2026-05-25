//! Quantity badge chip for market entry rows.

use iced::{
  Background, Border, Element, Padding, Theme,
  widget::{container, text},
};

use crate::{
  format,
  style::{color, radius, typography::mono},
  views::wallet::market_tab::Message,
};

/// Builder for a quantity badge.
pub struct QtyBadge {
  qty: u64,
}

impl QtyBadge {
  /// Creates a new quantity badge for the given count.
  pub fn new(qty: u64) -> Self {
    Self {
      qty,
    }
  }

  /// Renders the badge into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    container(
      text(format::fmt_count(self.qty))
        .font(mono::MEDIUM)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: 6.0,
      right: 6.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::state::SUBTLE_FILL)),
      border: Border {
        radius: radius::CHIP.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
  }
}
