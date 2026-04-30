//! Empty queue placeholder component.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Horizontal,
  widget::{Space, column, container, text},
};

use super::super::Message;
use crate::style::{
  color, spacing,
  typography::{body, mono},
};

pub struct Component;

impl Component {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    container(
      column([
        text("Queue · 0 skills".to_string())
          .font(mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        Space::new().height(8.0).into(),
        text("Empty queue")
          .font(body::MEDIUM)
          .size(16.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        Space::new().height(6.0).into(),
        text("Add a skill from the browser, drop a planned set, or pick from your saved plans on the right.")
          .font(body::REGULAR)
          .size(13.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      ])
      .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: 40.0,
      bottom: 40.0,
      left: spacing::SPACE_7,
      right: spacing::SPACE_7,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}
