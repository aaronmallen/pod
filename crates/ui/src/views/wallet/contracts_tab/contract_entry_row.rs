//! Full contract entry row for the contracts table.

use iced::{
  Border, Element, Length,
  alignment::Vertical,
  widget::{container, row},
};

use super::{
  Message, character_cell::Component as CharacterCell, collateral_cell::Component as CollateralCell,
  counterparty_cell::Component as CounterpartyCell, location_cell::Component as LocationCell,
  price_cell::Component as PriceCell, status_cell::Component as StatusCell, title_cell::Component as TitleCell,
  type_cell::Component as TypeCell, when_cell::Component as WhenCell,
};
use crate::{
  style::color,
  views::wallet::{ContractEntry, WalletCharacter},
};

/// Builder for a full contract entry row.
pub struct Component<'a> {
  characters: &'a [WalletCharacter],
  entry: &'a ContractEntry,
}

impl<'a> Component<'a> {
  /// Creates a new contract entry row component.
  pub fn new(entry: &'a ContractEntry, characters: &'a [WalletCharacter]) -> Self {
    Self {
      characters,
      entry,
    }
  }

  /// Renders the contract entry row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let char_info = self.characters.iter().find(|c| c.id == self.entry.who);
    let inner = row([
      StatusCell::new(self.entry).render(),
      TypeCell::new(&self.entry.kind).render(),
      TitleCell::new(&self.entry.title).render(),
      CounterpartyCell::new(&self.entry.counterparty).render(),
      LocationCell::new(&self.entry.location).render(),
      PriceCell::new(self.entry.price).render(),
      CollateralCell::new(self.entry.collateral).render(),
      CharacterCell::new(char_info).render(),
      WhenCell::new(self.entry.ts_secs).render(),
    ])
    .height(52.0)
    .align_y(Vertical::Center);

    container(inner)
      .width(Length::Fill)
      .style(|_| container::Style {
        border: Border {
          color: color::border::SUBTLE,
          width: 1.0,
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  }
}
