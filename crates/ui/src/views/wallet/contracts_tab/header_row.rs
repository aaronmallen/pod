//! Full header row for the contracts table, composed of HeaderCells.

use iced::{
  Background, Border, Element, Length,
  widget::{container, row},
};

use super::{Message, header_cell::Component as HeaderCell};
use crate::style::color;

const COL_STATUS: f32 = 130.0;
const COL_TYPE: f32 = 120.0;
const COL_COUNTERPARTY: f32 = 136.0;
const COL_LOCATION: f32 = 148.0;
const COL_PRICE: f32 = 96.0;
const COL_COLLATERAL: f32 = 96.0;
const COL_CHARACTER: f32 = 148.0;
const COL_WHEN: f32 = 84.0;

/// Builder for the contracts table header row.
pub struct Component;

impl Component {
  /// Creates a new header row component.
  pub fn new() -> Self {
    Self
  }

  /// Renders the header row into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let inner = row([
      HeaderCell::new("Status", COL_STATUS, false).render(),
      HeaderCell::new("Type", COL_TYPE, false).render(),
      HeaderCell::new("Title", Length::Fill, false).render(),
      HeaderCell::new("Counterparty", COL_COUNTERPARTY, false).render(),
      HeaderCell::new("Route / Loc", COL_LOCATION, false).render(),
      HeaderCell::new("Price", COL_PRICE, true).render(),
      HeaderCell::new("Collateral", COL_COLLATERAL, true).render(),
      HeaderCell::new("Character", COL_CHARACTER, false).render(),
      HeaderCell::new("When", COL_WHEN, true).render(),
    ]);
    container(inner)
      .width(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
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
