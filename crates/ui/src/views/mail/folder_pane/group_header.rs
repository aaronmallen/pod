//! Folder group header component: titled folder group with section label and rows.

use iced::{
  Element, Length, Padding,
  widget::{Space, column, container},
};

use crate::{components::section_label, views::mail::folder_pane::Message};

/// Builder for a named folder group section in the sidebar.
pub struct Component<'a> {
  rows: Vec<Element<'a, Message>>,
  title: &'static str,
}

impl<'a> Component<'a> {
  /// Creates a new folder group header with the given title and row elements.
  pub fn new(title: &'static str, rows: Vec<Element<'a, Message>>) -> Self {
    Self {
      rows,
      title,
    }
  }

  /// Renders the folder group section.
  pub fn render(self) -> Element<'a, Message> {
    container(column([
      section_label(self.title),
      Space::new().height(10.0).into(),
      column(self.rows).spacing(1.0).into(),
    ]))
    .padding(Padding {
      top: 20.0,
      bottom: 8.0,
      left: 16.0,
      right: 16.0,
    })
    .width(Length::Fill)
    .into()
  }
}
