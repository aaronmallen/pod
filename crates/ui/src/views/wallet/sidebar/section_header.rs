//! Sidebar section label (e.g. "Quick Filter", "Characters").

use iced::{Element, Length, Padding, Theme, widget::{container, text}};

use crate::{
  style::{color, typography::mono},
  views::wallet::Message,
};

/// Builder for a sidebar section header label.
pub struct Component {
  title: String,
}

impl Component {
  /// Creates a new section header with the given title.
  pub fn new(title: impl Into<String>) -> Self {
    Self { title: title.into() }
  }

  /// Renders the section header into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    container(
      text(self.title.to_uppercase())
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding {
      top: 18.0,
      bottom: 4.0,
      left: 20.0,
      right: 14.0,
    })
    .width(Length::Fill)
    .into()
  }
}
