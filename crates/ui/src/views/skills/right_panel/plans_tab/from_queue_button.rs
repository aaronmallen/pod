//! "From queue" action button.

use iced::{
  Border, Element, Padding,
  widget::{button, text},
};

use super::Message;
use crate::style::{color, spacing, typography::body};

pub struct Component;

impl Component {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    button(
      text("From queue")
        .font(body::REGULAR)
        .size(12.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .on_press(Message::FromQueue)
    .style(|_, status| button::Style {
      background: None,
      border: Border {
        color: match status {
          button::Status::Hovered | button::Status::Pressed => color::border::DEFAULT,
          _ => color::border::SUBTLE,
        },
        radius: 6.0.into(),
        width: 1.0,
      },
      text_color: match status {
        button::Status::Hovered | button::Status::Pressed => color::text::PRIMARY,
        _ => color::text::SECONDARY,
      },
      ..button::Style::default()
    })
    .into()
  }
}
