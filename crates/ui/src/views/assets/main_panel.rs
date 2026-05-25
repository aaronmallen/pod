//! Main panel — tab strip + active tab dispatch.

use iced::{
  Background, Element, Length,
  widget::{Space, column, container, mouse_area, row},
};

use super::{
  Message, State, Tab, abyssals_tab::Component as Abyssals, inventory_tab::Component as Inventory,
  stockpiles_tab::Component as Stockpiles, tracker_tab::Component as Tracker, values_tab::Component as Values,
};
use crate::components::{TabStrip, tab_strip::TabItem};

fn tab_items(state: &State) -> Vec<TabItem> {
  vec![
    TabItem {
      label: "Inventory".to_string(),
      count: Some(state.visible_assets().count()),
    },
    TabItem {
      label: "Stockpiles".to_string(),
      count: Some(0),
    },
    TabItem {
      label: "Values".to_string(),
      count: None,
    },
    TabItem {
      label: "Tracker".to_string(),
      count: None,
    },
    TabItem {
      label: "Abyssals".to_string(),
      count: Some(state.abyssals.abyssals.len()),
    },
  ]
}

fn tab_active_index(tab: &Tab) -> usize {
  match tab {
    Tab::Inventory => 0,
    Tab::Stockpiles => 1,
    Tab::Values => 2,
    tab => tab_active_index_ext(tab),
  }
}

fn tab_active_index_ext(tab: &Tab) -> usize {
  match tab {
    Tab::Tracker => 3,
    _ => 4,
  }
}

fn tab_from_low_index(i: usize) -> Option<Tab> {
  match i {
    0 => Some(Tab::Inventory),
    1 => Some(Tab::Stockpiles),
    _ => None,
  }
}

fn tab_from_index(i: usize) -> Tab {
  if let Some(tab) = tab_from_low_index(i) {
    return tab;
  }
  match i {
    2 => Tab::Values,
    3 => Tab::Tracker,
    _ => Tab::Abyssals,
  }
}

fn render_active_tab(state: &State) -> Element<'_, Message> {
  render_primary_tab(state).unwrap_or_else(|| render_secondary_tab(state))
}

fn render_primary_tab(state: &State) -> Option<Element<'_, Message>> {
  match state.active_tab {
    Tab::Inventory => Some(render_inventory_tab(state)),
    Tab::Stockpiles => Some(Stockpiles::new(state).render().map(Message::StockpilesTab)),
    _ => None,
  }
}

fn render_secondary_tab(state: &State) -> Element<'_, Message> {
  match state.active_tab {
    Tab::Abyssals => Abyssals::new(state).render().map(Message::AbyssalsTab),
    Tab::Tracker => Tracker::new(state).render().map(Message::TrackerTab),
    _ => Values::new(state).render().map(Message::ValuesTab),
  }
}

fn render_inventory_tab(state: &State) -> Element<'_, Message> {
  row([
    super::sidebar::Component::new(state).render(),
    pane_drag_handle(),
    Inventory::new(state).render().map(Message::InventoryTab),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn tab_strip_el<'a>(state: &'a State) -> Element<'a, Message> {
  let active = tab_active_index(&state.active_tab);
  TabStrip::new(tab_items(state))
    .active(active)
    .render(|i| Message::TabSelected(tab_from_index(i)))
}

fn pane_drag_handle() -> Element<'static, Message> {
  mouse_area(
    container(Space::new().width(4.0).height(Length::Fill))
      .width(4.0)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(crate::style::color::border::SUBTLE)),
        ..container::Style::default()
      }),
  )
  .on_press(Message::PaneDragStart)
  .interaction(iced::mouse::Interaction::ResizingHorizontally)
  .into()
}

/// Builder for the main panel (tab strip + active tab body).
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new main panel for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the main panel into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let tabs_el = tab_strip_el(state);
    let body = render_active_tab(state);
    column([tabs_el, body]).width(Length::Fill).height(Length::Fill).into()
  }
}
