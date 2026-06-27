use iced::{
  Background, Element, Length, Padding,
  alignment::Horizontal,
  widget::{button, container, mouse_area, text},
};

use super::Message;
use crate::ui::{
  components::badge::badge,
  style::{color, typography},
};

pub(super) fn insertion_gap<'a>(after_entry_id: Option<i64>, gap_key: i64, hovered: bool) -> Element<'a, Message> {
  let inner: Element<'a, Message> = if hovered {
    pill_button(after_entry_id)
  } else {
    container(text("")).width(Length::Fill).height(12.0).into()
  };

  mouse_area(inner)
    .on_enter(Message::GapHovered(gap_key))
    .on_exit(Message::GapUnhovered)
    .into()
}

fn pill_button<'a>(after_entry_id: Option<i64>) -> Element<'a, Message> {
  let pill = badge(
    t!("skills.editor_remap.remap_here").into_owned(),
    Some(color::accent::PLASMA),
  );

  button(container(pill).width(Length::Fill).align_x(Horizontal::Center))
    .padding(Padding {
      top: 4.0,
      bottom: 4.0,
      left: 0.0,
      right: 0.0,
    })
    .width(Length::Fill)
    .on_press(Message::RemapInserted(after_entry_id))
    .style(|_, status| button::Style {
      background: match status {
        button::Status::Hovered | button::Status::Pressed => {
          Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.05)))
        }
        _ => None,
      },
      ..button::Style::default()
    })
    .into()
}

pub(super) fn remap_exhausted<'a>(reason: &'a str) -> Element<'a, Message> {
  container(
    text(reason.to_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .width(Length::Fill)
  .align_x(Horizontal::Center)
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 0.0,
    right: 0.0,
  })
  .into()
}
