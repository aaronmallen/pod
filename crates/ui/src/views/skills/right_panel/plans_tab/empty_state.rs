//! Empty state placeholder for the plans tab.

use iced::{
  Element, Length, Padding,
  alignment::Horizontal,
  widget::{Space, column, container, text},
};

use super::{FromQueueButton, Message, NewPlanButton};
use crate::style::{color, spacing, typography::body};

pub struct Component;

impl Component {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    container(
      column([
        text("No skill plans yet")
          .font(body::MEDIUM)
          .size(15.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        Space::new().height(4.0).into(),
        text("Create your first plan to start optimizing your skill queue.")
          .font(body::REGULAR)
          .size(13.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        Space::new().height(spacing::SPACE_4).into(),
        iced::widget::row([
          NewPlanButton::new().render(),
          Space::new().width(spacing::SPACE_2).into(),
          FromQueueButton::new().render(),
        ])
        .align_y(iced::alignment::Vertical::Center)
        .into(),
      ])
      .align_x(Horizontal::Center),
    )
    .align_x(Horizontal::Center)
    .width(Length::Fill)
    .padding(Padding {
      top: 36.0,
      bottom: 36.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .into()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}
