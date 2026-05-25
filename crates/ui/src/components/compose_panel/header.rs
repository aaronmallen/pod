//! Panel header for the compose panel.

use iced::{
  Background, Border, Element, Padding, Theme,
  widget::{button, container, text},
};

use super::Message;
use crate::{
  components::PanelHeader,
  style::{color, typography as font},
};

/// Builder for the compose panel header.
pub struct ComposePanelHeader {
  expand_sym: &'static str,
}

impl ComposePanelHeader {
  /// Creates a new header builder with the given expand symbol.
  pub fn new(expand_sym: &'static str) -> Self {
    Self {
      expand_sym,
    }
  }

  /// Renders the compose panel header.
  pub fn render(self) -> Element<'static, Message> {
    panel_header(self.expand_sym)
  }
}

pub(super) fn panel_header(expand_sym: &'static str) -> Element<'static, Message> {
  let close_btn = icon_btn("–", Message::Close);
  let expand_btn = icon_btn(expand_sym, Message::Expand);
  let dismiss_btn = icon_btn("✕", Message::Close);

  PanelHeader::new("NEW MESSAGE")
    .action(close_btn)
    .action(expand_btn)
    .action(dismiss_btn)
    .render()
}

/// Renders an icon button for the panel header.
pub fn icon_btn(label: &'static str, msg: Message) -> Element<'static, Message> {
  button(
    container(
      text(label)
        .font(font::mono::REGULAR)
        .size(13.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .center_x(16.0)
    .center_y(24.0),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 6.0,
    right: 6.0,
  })
  .on_press(msg)
  .style(|_, status| icon_btn_style(status))
  .into()
}

fn icon_btn_style(status: button::Status) -> button::Style {
  let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: if active {
      Some(Background::Color(color::state::HOVER_OVERLAY))
    } else {
      None
    },
    border: Border {
      radius: 5.0.into(),
      ..Border::default()
    },
    text_color: if active {
      color::text::PRIMARY
    } else {
      color::text::SECONDARY
    },
    ..button::Style::default()
  }
}
