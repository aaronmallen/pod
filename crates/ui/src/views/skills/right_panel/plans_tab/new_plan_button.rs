//! "New plan" action button.

use iced::{
  Background, Border, Element, Padding,
  widget::{button, text},
};

use super::Message;
use crate::style::{color, spacing, typography::body};

pub struct Component;

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}

impl Component {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    button(
      text("New plan")
        .font(body::MEDIUM)
        .size(12.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::accent::PLASMA),
        }),
    )
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .on_press(Message::NewPlan)
    .style(|_, status| button::Style {
      background: Some(Background::Color(match status {
        button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA_SUBTLE,
        _ => iced::Color::TRANSPARENT,
      })),
      border: Border {
        color: color::accent::PLASMA_MUTED,
        radius: 6.0.into(),
        width: 1.0,
      },
      text_color: color::accent::PLASMA,
      ..button::Style::default()
    })
    .into()
  }
}
