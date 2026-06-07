use iced::{
  Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container, text},
};

use super::{Message, from_queue_button::from_queue_button, new_plan_button::new_plan_button};
use crate::ui::style::{color, spacing, typography};

pub fn empty_state<'a>() -> Element<'a, Message> {
  container(
    Column::with_children(vec![
      text("No skill plans yet")
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      Space::new().height(Length::Fixed(spacing::UNIT)).into(),
      text("Create your first plan to start optimizing your skill queue.")
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      Space::new().height(Length::Fixed(spacing::SPACE_6)).into(),
      Row::with_children(vec![
        new_plan_button(),
        Space::new().width(Length::Fixed(spacing::SPACE_2)).into(),
        from_queue_button(),
      ])
      .align_y(Vertical::Center)
      .into(),
    ])
    .align_x(Horizontal::Center),
  )
  .align_x(Horizontal::Center)
  .width(Length::Fill)
  .padding(Padding {
    top: 36.0,
    bottom: 36.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .into()
}
