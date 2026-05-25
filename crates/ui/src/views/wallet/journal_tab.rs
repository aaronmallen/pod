//! Journal entry table for the wallet main panel.

pub mod entry_row;

use iced::Element;

use crate::{
  components::DataTable,
  views::wallet::{SignFilter, State},
};

/// Messages produced by the journal tab.
#[derive(Clone, Debug)]
pub enum Message {
  SignFilterChanged(SignFilter),
}

/// Builder for the journal table.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new journal table component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the journal table into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    DataTable::new(self.state.filtered_journal.iter(), |entry, _, _| {
      entry_row::Component::new(entry).render()
    })
    .empty_message("No journal entries match your filter.")
    .render()
  }
}
