//! Main panel — tab strip + active tab dispatch.

use iced::{
  Background, Element, Length,
  widget::{Space, column, container, mouse_area, row},
};

use super::{
  Message, State, Tab, inventory_tab::Component as Inventory, stockpiles_tab::Component as Stockpiles,
  tracker_tab::Component as Tracker, values_tab::Component as Values,
};
use crate::components::{TabStrip, tab_strip::TabItem};

fn tab_strip_el<'a>(state: &'a State) -> Element<'a, Message> {
  let inv_count = state.visible_assets().count();
  let tabs = vec![
    TabItem {
      label: "Inventory".to_string(),
      count: Some(inv_count),
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
  ];
  let active_index = match state.active_tab {
    Tab::Inventory => 0,
    Tab::Stockpiles => 1,
    Tab::Values => 2,
    Tab::Tracker => 3,
  };
  TabStrip::new(tabs).active(active_index).render(|i| {
    Message::TabSelected(match i {
      0 => Tab::Inventory,
      1 => Tab::Stockpiles,
      2 => Tab::Values,
      _ => Tab::Tracker,
    })
  })
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
    let body: Element<'_, Message> = match state.active_tab {
      Tab::Inventory => row([
        super::sidebar::Component::new(state).render(),
        pane_drag_handle(),
        Inventory::new(state).render().map(Message::InventoryTab),
      ])
      .width(Length::Fill)
      .height(Length::Fill)
      .into(),
      Tab::Stockpiles => Stockpiles::new(state).render().map(Message::StockpilesTab),
      Tab::Values => Values::new(state).render().map(Message::ValuesTab),
      Tab::Tracker => Tracker::new(state).render().map(Message::TrackerTab),
    };

    column([tabs_el, body]).width(Length::Fill).height(Length::Fill).into()
  }
}
