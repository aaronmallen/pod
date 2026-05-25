//! Colored tag preview chip rendered as a pill-shaped badge.

use iced::{Background, Border, Element, Padding};

use super::Message;
use crate::style::{color, radius, typography};

/// Converts a CSS hex color string to an [`iced::Color`].
pub fn hex_to_iced_color(hex: &str) -> Option<iced::Color> {
  let hex = hex.trim_start_matches('#');
  if hex.len() != 6 {
    return None;
  }
  let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
  let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
  let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
  Some(iced::Color {
    r,
    g,
    b,
    a: 1.0,
  })
}

/// Builder for a pill-shaped tag preview chip with an optional color.
pub struct TagColorSwatch<'a> {
  /// Hex color string, e.g. `"#ff8800"`.
  color_hex: Option<&'a str>,
  /// Display name rendered inside the chip.
  name: &'a str,
}

impl<'a> TagColorSwatch<'a> {
  /// Create a new swatch builder for the given tag name and optional hex color.
  pub fn new(name: &'a str, color_hex: Option<&'a str>) -> Self {
    Self {
      color_hex,
      name,
    }
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let (bg, fg, bd) = match self.color_hex.and_then(hex_to_iced_color) {
      Some(c) => (
        iced::Color {
          a: 0.12,
          ..c
        },
        c,
        iced::Color {
          a: 0.45,
          ..c
        },
      ),
      None => (color::state::TAG_FILL, color::text::SECONDARY, color::border::SUBTLE),
    };
    iced::widget::container(
      iced::widget::text(self.name)
        .font(typography::body::MEDIUM)
        .size(11.0)
        .style(move |_| iced::widget::text::Style {
          color: Some(fg),
        }),
    )
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: 8.0,
      right: 8.0,
    })
    .style(move |_| iced::widget::container::Style {
      background: Some(Background::Color(bg)),
      border: Border {
        color: bd,
        radius: radius::FULL.into(),
        width: 1.0,
      },
      ..iced::widget::container::Style::default()
    })
    .into()
  }
}
