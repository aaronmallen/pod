use iced::{
  Element, Length, Padding,
  widget::{button, container, row, text},
};

use crate::style::{color, spacing, typography};

fn refresh_btn_text_color(is_syncing: bool, status: button::Status) -> iced::Color {
  if is_syncing {
    color::text::TERTIARY
  } else {
    refresh_btn_hover_color(status)
  }
}

fn refresh_btn_hover_color(status: button::Status) -> iced::Color {
  match status {
    button::Status::Hovered => color::text::PRIMARY,
    _ => color::text::SECONDARY,
  }
}

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
    text_color: refresh_btn_text_color(is_syncing, status),
    background: None,
    ..button::Style::default()
  });

  if is_syncing {
    btn.into()
  } else {
    btn.on_press(super::Message::RefreshPressed).into()
  }
}
