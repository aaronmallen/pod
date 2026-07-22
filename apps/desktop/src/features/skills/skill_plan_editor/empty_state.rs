use iced::{
  Element, Length,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container, text},
};

use super::Message;
use crate::ui::{
  components::button::{Button, Size},
  style::{color, spacing, typography},
};

const CONTENT_SPACING: f32 = spacing::UNIT;
const SECTION_GAP: f32 = spacing::SPACE_6;
const ACTION_GAP: f32 = spacing::SPACE_2;

pub(super) fn empty_state<'a>(picker_open: bool) -> Element<'a, Message> {
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
    actions(picker_open),
  ];

  container(Column::with_children(children).align_x(Horizontal::Center))
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .padding(SECTION_GAP)
    .into()
}

fn actions<'a>(picker_open: bool) -> Element<'a, Message> {
  let mut buttons: Vec<Element<'a, Message>> = Vec::with_capacity(2);
  if !picker_open {
    buttons.push(
      Button::secondary(t!("skills.editor.empty_action"))
        .size(Size::Sm)
        .on_press(Message::PickerToggled)
        .into(),
    );
  }
  buttons.push(
    Button::primary(t!("skills.editor.empty_action_milestone"))
      .size(Size::Sm)
      .on_press(Message::RemapInserted(None))
      .into(),
  );

  Row::with_children(buttons)
    .spacing(ACTION_GAP)
    .align_y(Vertical::Center)
    .into()
}
