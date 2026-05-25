//! Search bar component for the skill browser tab.

use iced::{Element, Length, Padding, widget::container};

use super::Message;
use crate::{
  components::SearchBox,
  style::{color, spacing},
};

/// Search bar displayed at the top of the skill browser tab.
pub struct SearchBar<'a> {
  query: &'a str,
}

impl<'a> SearchBar<'a> {
  /// Creates a new [`SearchBar`] bound to the given search query string.
  pub fn new(query: &'a str) -> Self {
    Self {
      query,
    }
  }

  /// Renders the search bar into an [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let search_box = SearchBox::new("Search skills…", self.query, Message::SearchChanged)
      .height(36.0)
      .icon_size(14.0)
      .icon_spacing(10.0)
      .horizontal_padding(spacing::SPACE_3)
      .background(color::surface::BASE)
      .render();

    container(search_box)
      .padding(Padding {
        top: 14.0,
        bottom: spacing::SPACE_3,
        left: spacing::SPACE_4,
        right: spacing::SPACE_4,
      })
      .width(Length::Fill)
      .into()
  }
}
