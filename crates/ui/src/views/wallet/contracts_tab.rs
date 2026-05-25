//! Contracts table for the wallet main panel.

mod character_cell;
mod collateral_cell;
mod contract_entry_row;
mod counterparty_cell;
mod header_cell;
mod header_row;
mod location_cell;
mod portrait_chip;
mod price_cell;
mod status_cell;
mod title_cell;
mod type_cell;
mod when_cell;

use contract_entry_row::Component as ContractEntryRow;
use header_row::Component as HeaderRow;
use iced::Element;

use crate::{components::DataTable, views::wallet::State};

/// Messages produced by the contracts tab (reserved for future interactions).
#[derive(Clone, Debug)]
pub enum Message {}

/// Builder for the contracts table.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new contracts table component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the contracts table into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let chars = &self.state.characters;
    DataTable::new(self.state.filtered_contracts.iter(), |e, _, _| {
      ContractEntryRow::new(e, chars).render()
    })
    .header(HeaderRow::new().render())
    .empty_message("No contracts match your filter.")
    .render()
  }
}
