//! "Locations" section label in the sidebar.

use iced::{Element, Length, Padding, widget::container};

use super::super::Message;
use crate::components::section_label;

/// Builder for the "Locations" section label in the sidebar.
pub struct Component;

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}

impl Component {
  /// Creates a new locations label component.
  pub fn new() -> Self {
    Self
  }

  /// Renders the locations label into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    container(section_label("Locations"))
      .padding(Padding {
        top: 16.0,
        bottom: 5.0,
        left: 18.0,
        right: 14.0,
      })
      .width(Length::Fill)
      .into()
  }
}
