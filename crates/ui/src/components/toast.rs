//! Floating toast notification overlay.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, container, row, text},
};

use crate::style::{color, radius, shadow, spacing};

/// Builder for a floating toast notification.
pub struct Component {
  message: String,
}

impl Component {
  /// Create a new toast with the given message.
  pub fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }

  /// Consume the builder and return a full-size overlay element that
  /// positions the toast pill in the bottom-right corner.
  pub fn render<MSG: 'static>(self) -> Element<'static, MSG> {
    let dot = container(Space::new().width(6.0).height(6.0))
      .width(6.0)
      .height(6.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::text::SUCCESS)),
        border: Border {
          radius: radius::FULL.into(),
          ..Border::default()
        },
        ..container::Style::default()
      });

    let pill = container(
      row([
        dot.into(),
        text(self.message).size(12.0).color(color::text::PRIMARY).into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
    )
    .padding(Padding {
      top: 9.0,
      bottom: 9.0,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::DEFAULT,
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      shadow: shadow::CARD,
      ..container::Style::default()
    });

    container(pill)
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(Horizontal::Right)
      .align_y(Vertical::Bottom)
      .padding(Padding {
        top: 0.0,
        bottom: spacing::SPACE_5,
        left: 0.0,
        right: spacing::SPACE_5,
      })
      .into()
  }
}
