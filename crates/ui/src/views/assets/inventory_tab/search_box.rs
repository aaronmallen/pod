//! Search box with help toggle for the inventory filter bar.

use iced::{Element, Length};

use super::{Message, help_button::HelpButton};
use crate::components;

/// Builder for the inventory search box.
pub struct SearchBox<'a> {
  help_visible: bool,
  query: &'a str,
}

impl<'a> SearchBox<'a> {
  /// Creates a new search box for the given query and help visibility state.
  pub fn new(query: &'a str, help_visible: bool) -> Self {
    Self {
      help_visible,
      query,
    }
  }

  /// Renders the search box into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    components::SearchBox::new(
      "Filter assets…  try name:Rifter or category:ship",
      self.query,
      Message::SearchChanged,
    )
    .width(Length::Fill)
    .height(44.0)
    .font_size(13.0)
    .icon_size(18.0)
    .icon_spacing(10.0)
    .horizontal_padding(14.0)
    .right_element(HelpButton::new(self.help_visible).render())
    .render()
  }
}
