//! Wraps ComposePanel with open/close positioning.

use iced::{Element, Length, Padding, widget::container};

use super::Message;
use crate::components::ComposePanel;

/// Builder for the compose panel overlay.
pub struct Component<'a> {
  compose: &'a ComposePanel,
  expanded: bool,
}

impl<'a> Component<'a> {
  /// Create a new compose overlay builder.
  pub fn new(compose: &'a ComposePanel, expanded: bool) -> Self {
    Self {
      compose,
      expanded,
    }
  }

  /// Render into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let panel = self.compose.render().map(Message::Compose);
    if self.expanded {
      container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .into()
    } else {
      container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Bottom)
        .padding(Padding {
          top: 0.0,
          bottom: 16.0,
          left: 0.0,
          right: 24.0,
        })
        .into()
    }
  }
}
