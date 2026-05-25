//! Values tab — character × location value matrix, category breakdown, and top items.

pub mod category_panel;
pub mod chart_legend_item;
pub mod chart_section;
pub mod empty_panel;
pub mod matrix_panel;
pub mod stat_cell;
pub mod stat_row;
pub mod top_item_row;
pub mod top_items_panel;

pub use category_panel::Component as CategoryPanel;
pub use empty_panel::Component as EmptyPanel;
use iced::{
  Element, Length, Padding,
  widget::{Space, column, container, row, scrollable},
};
pub use matrix_panel::Component as MatrixPanel;
pub use top_items_panel::Component as TopItemsPanel;

use super::State;

/// Messages produced by the values tab.
#[derive(Clone, Debug)]
pub enum Message {}

/// Builder for the values tab.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new values tab for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the values tab into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let Some(data) = &state.asset_values_data else {
      return scrollable(
        container(EmptyPanel::new().render())
          .padding(Padding {
            top: 20.0,
            bottom: 32.0,
            left: 28.0,
            right: 28.0,
          })
          .width(Length::Fill)
          .height(Length::Fill),
      )
      .height(Length::Fill)
      .into();
    };

    let matrix = MatrixPanel::new(&data.character_structure_cells, data.total_value).render();
    let right_col: Element<'a, Message> = column([
      CategoryPanel::new(&data.category_breakdown, data.total_value).render(),
      Space::new().height(16.0).into(),
      TopItemsPanel::new(&data.top_items, &state.item_icons).render(),
    ])
    .width(360.0)
    .into();

    scrollable(
      container(
        row([matrix, Space::new().width(20.0).into(), right_col])
          .width(Length::Fill)
          .align_y(iced::alignment::Vertical::Center),
      )
      .padding(Padding {
        top: 20.0,
        bottom: 32.0,
        left: 28.0,
        right: 28.0,
      })
      .width(Length::Fill),
    )
    .height(Length::Fill)
    .into()
  }
}
