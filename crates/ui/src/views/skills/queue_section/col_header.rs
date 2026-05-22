//! Column label row component.

use iced::{
  Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, column, container, row, text},
};

use super::super::Message;
use crate::{
  components,
  style::{color, spacing, typography::mono},
};

pub struct Component;

impl Component {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    let header = container(labels())
      .padding(Padding {
        top: 12.0,
        bottom: 12.0,
        left: spacing::SPACE_4,
        right: spacing::SPACE_4,
      })
      .width(Length::Fill);
    column([header.into(), components::Separator::horizontal().render()]).into()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}

fn labels<'a>() -> Element<'a, Message> {
  row([
    container(
      text("Completes")
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .width(Length::Fixed(135.0))
    .into(),
    Space::new().width(28.0).into(),
    container(
      text("Skill")
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .width(Length::Fill)
    .into(),
    container(
      text("SP")
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .width(Length::Fixed(80.0))
    .align_x(Horizontal::Right)
    .into(),
    container(
      text("Duration")
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .width(Length::Fixed(110.0))
    .align_x(Horizontal::Right)
    .into(),
    Space::new().width(36.0).into(),
  ])
  .align_y(Vertical::Center)
  .into()
}
