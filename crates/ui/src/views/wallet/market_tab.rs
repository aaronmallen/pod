//! Market transaction table for the wallet main panel.

use iced::Element;

use crate::views::wallet::{SideFilter, State};

pub mod entry_row;
pub mod qty_badge;
pub mod side_badge;
pub mod type_icon_cell;

use entry_row::MarketEntryRow;

/// Messages produced by the market tab.
#[derive(Clone, Debug)]
pub enum Message {
  SideFilterChanged(SideFilter),
}

/// Builder for the market transaction table.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new market table component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the market table into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    use crate::components::DataTable;

    let icons = &self.state.item_icons;
    DataTable::new(self.state.filtered_market.iter(), |e, _, _| {
      MarketEntryRow::new(e, icons.get(&e.type_id).cloned()).render()
    })
    .empty_message("No market entries match your filter.")
    .render()
  }
}
