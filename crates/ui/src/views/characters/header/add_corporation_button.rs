//! "Add corporation" ghost button for the Characters header.

use iced::{
  Element,
  alignment::Vertical,
  widget::{row, text},
};

use crate::{
  components,
  style::{spacing, typography},
};

/// Message emitted by [`Component`].
#[derive(Clone, Debug)]
pub enum Message {
  /// The button was pressed.
  Pressed,
}

/// Builder for the "Add corporation" ghost button.
pub struct Component;

impl Component {
  /// Creates a new `AddCorporationButton`.
  pub fn new() -> Self {
    Self
  }

  /// Renders the button into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    components::Button::ghost(
      row([
        text("+").font(typography::body::MEDIUM).size(13.0).into(),
        text("Add corporation").font(typography::body::MEDIUM).size(13.0).into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
    )
    .on_press(Message::Pressed)
    .into()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}
