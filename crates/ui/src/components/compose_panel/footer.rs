//! Footer row with send/discard actions for the compose panel.

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Space, button, container, row, text},
};

use super::Message;
use crate::style::{color, typography as font};

/// Builder for the compose panel footer.
pub struct ComposeFooter<'a> {
  can_send: bool,
  sending: bool,
  from_trigger: Element<'a, Message>,
}

impl<'a> ComposeFooter<'a> {
  /// Creates a new footer builder.
  pub fn new(can_send: bool, sending: bool, from_trigger: Element<'a, Message>) -> Self {
    Self {
      can_send,
      sending,
      from_trigger,
    }
  }

  /// Renders the compose footer.
  pub fn render(self) -> Element<'a, Message> {
    send_footer_inner(self.can_send, self.sending, self.from_trigger)
  }
}

pub(super) fn send_footer_inner<'a>(
  can_send: bool,
  sending: bool,
  from_trigger: Element<'a, Message>,
) -> Element<'a, Message> {
  container(
    row([
      from_trigger,
      Space::new().width(Length::Fill).into(),
      send_btn(can_send, sending),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .center_y(52.0)
  .width(Length::Fill)
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: 12.0,
    right: 12.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

/// Renders the send button in the appropriate state.
pub fn send_btn(can_send: bool, sending: bool) -> Element<'static, Message> {
  if can_send && !sending {
    send_btn_active()
  } else {
    send_btn_disabled(sending)
  }
}

/// Renders the active (clickable) send button.
pub fn send_btn_active() -> Element<'static, Message> {
  button(
    text("Send")
      .font(font::body::MEDIUM)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::surface::BASE),
      }),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: 16.0,
    right: 16.0,
  })
  .on_press(Message::SendPressed)
  .style(|_, status| button::Style {
    background: Some(Background::Color(match status {
      button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA_HOVER,
      _ => color::accent::PLASMA,
    })),
    border: Border {
      radius: 6.0.into(),
      ..Border::default()
    },
    text_color: color::surface::BASE,
    ..button::Style::default()
  })
  .into()
}

/// Renders the disabled send button (when sending or missing fields).
pub fn send_btn_disabled(sending: bool) -> Element<'static, Message> {
  let label = if sending { "Sending…" } else { "Send" };
  button(
    text(label)
      .font(font::body::MEDIUM)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::DIM),
      }),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: 16.0,
    right: 16.0,
  })
  .style(|_, _| button::Style {
    background: Some(Background::Color(color::accent::PLASMA_MUTED)),
    border: Border {
      radius: 6.0.into(),
      ..Border::default()
    },
    text_color: color::surface::BASE,
    ..button::Style::default()
  })
  .into()
}

/// Renders the discard button.
pub fn discard_btn() -> Element<'static, Message> {
  button(
    text("Discard")
      .font(font::body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: 16.0,
    right: 16.0,
  })
  .on_press(Message::Close)
  .style(|_, status| button::Style {
    background: None,
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::text::PRIMARY,
      _ => color::text::SECONDARY,
    },
    ..button::Style::default()
  })
  .into()
}
