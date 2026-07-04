use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, container, mouse_area, text},
};

use super::Message;
use crate::ui::style::{color, typography};

pub(super) fn milestone_insertion<'a>(
  after_entry_id: Option<i64>,
  gap_key: i64,
  hovered: bool,
) -> Element<'a, Message> {
  let inner: Element<'a, Message> = if hovered {
    pill()
  } else {
    Space::new().width(Length::Fill).height(4.0).into()
  };

  let clickable = button(
    container(inner)
      .width(Length::Fill)
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 0.0,
    right: 0.0,
  })
  .width(Length::Fill)
  .on_press(Message::RemapInserted(after_entry_id))
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(color::with_alpha(color::accent(), 0.05)))
      }
      _ => None,
    },
    ..button::Style::default()
  });

  mouse_area(clickable)
    .on_enter(Message::GapHovered(gap_key))
    .on_exit(Message::GapUnhovered)
    .into()
}

fn pill<'a>() -> Element<'a, Message> {
  container(
    text(t!("skills.editor_milestone.add_here").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::accent()),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 10.0,
    right: 10.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent(), 0.10))),
    border: Border {
      color: color::with_alpha(color::accent(), 0.40),
      radius: 3.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}
