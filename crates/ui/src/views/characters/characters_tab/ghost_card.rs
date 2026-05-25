//! Styled card container used to display a dragged character ghost.

use iced::{Background, Border, Element, Length, widget::container};

use super::Message;
use crate::style::{color, radius, shadow, spacing};

/// Styled container that wraps ghost card column content during a drag operation.
pub struct Component<'a> {
  content: iced::widget::Column<'a, Message>,
}

impl<'a> Component<'a> {
  /// Creates a new `GhostCard` wrapping the given column content.
  pub fn new(content: iced::widget::Column<'a, Message>) -> Self {
    Self {
      content,
    }
  }

  /// Renders the ghost card into an element.
  pub fn render(self) -> Element<'a, Message> {
    let mut bg = color::surface::RAISED;
    bg.a = 0.96;

    container(self.content)
      .width(Length::Fill)
      .height(spacing::layout::CHARACTER_CARD_HEIGHT)
      .style(move |_| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
          color: color::border::DEFAULT,
          radius: radius::PANEL.into(),
          width: 1.0,
        },
        shadow: shadow::POPOVER,
        ..container::Style::default()
      })
      .into()
  }
}
