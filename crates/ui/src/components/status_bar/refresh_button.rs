use iced::{
  Color, Element, Length, Padding,
  widget::{button, container, row, text},
};

use crate::style::{color, spacing, typography};

pub fn view(is_syncing: bool) -> Element<'static, super::Message> {
  let btn = button(
    container(
      row([
        text("Refresh").font(typography::mono::REGULAR).size(10.0).into(),
        text("↻").font(typography::mono::REGULAR).size(11.0).into(),
      ])
      .spacing(spacing::SPACE_1)
      .align_y(iced::alignment::Vertical::Center),
    )
    .height(Length::Fill)
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: 14.0,
    right: 14.0,
  })
  .height(Length::Fill)
  .style(move |_, status| button::Style {
    text_color: if is_syncing {
      Color::from_rgba(0.957, 0.949, 0.925, 0.25)
    } else {
      match status {
        button::Status::Hovered => color::text::PRIMARY,
        _ => color::text::SECONDARY,
      }
    },
    background: None,
    ..button::Style::default()
  });

  if is_syncing {
    btn.into()
  } else {
    btn.on_press(super::Message::RefreshPressed).into()
  }
}
