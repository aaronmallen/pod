//! Monospace uppercase panel section label for the wallet right rail.

use iced::{
  Element, Length, Padding, Theme,
  widget::{container, text},
};

use crate::{
  style::{color, typography::mono},
  views::wallet::Message,
};

/// Builder for a monospace uppercase section label.
pub struct Component {
  title: &'static str,
}

impl Component {
  /// Creates a new section label component.
  pub fn new(title: &'static str) -> Self {
    Self {
      title,
    }
  }

  /// Renders the section label into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    container(
      text(self.title)
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding {
      top: 20.0,
      bottom: 12.0,
      left: 20.0,
      right: 20.0,
    })
    .width(Length::Fill)
    .into()
  }
}
