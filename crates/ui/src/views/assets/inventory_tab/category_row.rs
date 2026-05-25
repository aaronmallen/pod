//! Category filter pill row for the inventory tab.

use iced::Element;

use super::{super::Category, Message};
use crate::components::PillFilter;

/// Builder for the category filter pill row.
pub struct CategoryRow<'a> {
  active: &'a Category,
}

impl<'a> CategoryRow<'a> {
  /// Creates a new category row for the given active category.
  pub fn new(active: &'a Category) -> Self {
    Self {
      active,
    }
  }

  /// Renders the category row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let options: Vec<(&str, Category)> = Category::all().iter().map(|c| (c.label(), c.clone())).collect();
    PillFilter::new(options, self.active, Message::CategoryChanged).render()
  }
}
