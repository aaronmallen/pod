//! Main panel — tab strip, division strip, filter bar, and active tab body.

pub mod division_strip;
pub mod filter_bar;

pub use division_strip::Component as DivisionStrip;
pub use filter_bar::Component as FilterBar;
use iced::{
  Background, Element, Length,
  widget::{column, container},
};

use crate::{
  components::{TabStrip, tab_strip::TabItem},
  style::color,
  views::wallet::{Message, State, Tab},
};

fn tab_items(state: &State) -> Vec<TabItem> {
  vec![
    TabItem {
      label: "Market".to_string(),
      count: Some(state.filtered_market.len()),
    },
    TabItem {
      label: "Contracts".to_string(),
      count: Some(state.filtered_contracts.len()),
    },
    TabItem {
      label: "Journal".to_string(),
      count: Some(state.filtered_journal.len()),
    },
  ]
}

fn tab_index_to_tab(i: usize) -> Tab {
  match i {
    0 => Tab::Market,
    1 => Tab::Contracts,
    _ => Tab::Journal,
  }
}

fn tab_to_index(tab: &Tab) -> usize {
  match tab {
    Tab::Market => 0,
    Tab::Contracts => 1,
    Tab::Journal => 2,
  }
}

fn active_tab_body(state: &State) -> Element<'_, Message> {
  match state.active_tab {
    Tab::Contracts => crate::views::wallet::contracts_tab::Component::new(state)
      .render()
      .map(Message::ContractsTab),
    Tab::Journal => crate::views::wallet::journal_tab::Component::new(state)
      .render()
      .map(Message::JournalTab),
    Tab::Market => crate::views::wallet::market_tab::Component::new(state)
      .render()
      .map(Message::MarketTab),
  }
}

fn tab_bar(state: &State) -> Element<'_, Message> {
  let active_index = tab_to_index(&state.active_tab);
  TabStrip::new(tab_items(state))
    .active(active_index)
    .render(|i| Message::TabSelected(tab_index_to_tab(i)))
}

/// Builder for the wallet main panel.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new main panel component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the main panel into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let tab_bar_el = tab_bar(state);
    let filter_bar_el = FilterBar::new(state).render();
    let table = active_tab_body(state);
    let mut cols: Vec<Element<'_, Message>> = vec![tab_bar_el];
    if state.is_corp_selected() {
      cols.push(DivisionStrip::new(state).render());
    }
    cols.push(filter_bar_el);
    cols.push(table);
    container(column(cols))
      .width(Length::Fill)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::BASE)),
        ..container::Style::default()
      })
      .into()
  }
}
