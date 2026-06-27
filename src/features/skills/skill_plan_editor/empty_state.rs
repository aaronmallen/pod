use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Space, button, container, text},
};

use super::Message;
use crate::ui::style::{color, radius, spacing, typography};

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
  button(
    text(t!("skills.editor.empty_action"))
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::PickerToggled)
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(if hover {
        color::with_alpha(color::accent::PLASMA, 0.12)
      } else {
        iced::Color::TRANSPARENT
      })),
      border: Border {
        color: color::accent::PLASMA_MUTED,
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: color::accent::PLASMA,
      ..button::Style::default()
    }
  })
  .into()
}
