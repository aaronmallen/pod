//! Stockpiles tab — grid of stockpile cards with CRUD.

pub mod pile_item_row;
pub mod stockpile_card;
pub mod stockpile_empty_state;
pub mod stockpile_form_panel;
pub mod stockpile_toolbar;

use iced::{
  Element, Length, Padding,
  widget::{column, container, row, scrollable},
};

use super::State;

/// Messages produced by the stockpiles tab.
#[derive(Clone, Debug)]
pub enum Message {
  ConfirmDelete(i64),
  DeleteStockpile(i64),
  EditStockpile(i64),
  FormAddItem,
  FormCancel,
  FormItemQtyChanged(usize, String),
  FormItemTypeChanged(usize, String),
  FormLocationChanged(String),
  FormNameChanged(String),
  FormRemoveItem(usize),
  FormSave,
  NewStockpile,
}

/// Format a quantity as a compact string (K/M suffixes).
pub(super) fn fmt_count(n: u64) -> String {
  if n >= 1_000_000 {
    format!("{:.1}M", n as f64 / 1_000_000.0)
  } else if n >= 1_000 {
    format!("{:.1}K", n as f64 / 1_000.0)
  } else {
    n.to_string()
  }
}

/// Builder for the stockpiles tab.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new stockpiles tab for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the stockpiles tab into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;

    let ready_count = state.stockpiles.iter().filter(|p| p.ready).count();
    let short_count = state.stockpiles.iter().filter(|p| !p.ready).count();

    let toolbar = stockpile_toolbar::Component::new(ready_count, short_count).render();

    let grid: Element<'_, Message> = if state.stockpiles.is_empty() {
      stockpile_empty_state::Component::new().render()
    } else {
      let cards: Vec<Element<'_, Message>> = state
        .stockpiles
        .iter()
        .map(|pile| stockpile_card::Component::new(pile).render())
        .collect();
      scrollable(column(cards).spacing(14.0).width(Length::Fill))
        .height(Length::Fill)
        .into()
    };

    let content = container(column([toolbar, grid]).width(Length::Fill).height(Length::Fill))
      .padding(Padding {
        top: 20.0,
        bottom: 32.0,
        left: 28.0,
        right: 28.0,
      })
      .width(Length::Fill)
      .height(Length::Fill);

    if let Some(form) = &state.stockpile_form {
      row([content.into(), stockpile_form_panel::Component::new(form).render()])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
      content.into()
    }
  }
}
