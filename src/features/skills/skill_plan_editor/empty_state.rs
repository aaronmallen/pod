use iced::{
  Element, Length,
  alignment::{Horizontal, Vertical},
  widget::{Column, Space, container, text},
};

use super::Message;
use crate::ui::{
  components::button::{Button, Size},
  style::{color, spacing, typography},
};

const CONTENT_SPACING: f32 = spacing::UNIT;
const SECTION_GAP: f32 = spacing::SPACE_6;

pub(super) fn empty_state<'a>() -> Element<'a, Message> {
  let children: Vec<Element<'a, Message>> = vec![
    text(t!("skills.editor.empty_title"))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    Space::new().height(Length::Fixed(CONTENT_SPACING)).into(),
    text(t!("skills.editor.empty_subtitle"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
    Space::new().height(Length::Fixed(SECTION_GAP)).into(),
    action_button(),
  ];

  container(Column::with_children(children).align_x(Horizontal::Center))
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .padding(SECTION_GAP)
    .into()
}

fn action_button<'a>() -> Element<'a, Message> {
  Button::secondary(t!("skills.editor.empty_action"))
    .size(Size::Sm)
    .on_press(Message::PickerToggled)
    .into()
}
