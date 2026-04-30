//! Assets window header — picker button + stats row + bottom border.

pub mod stats_row;

use iced::{
  Background, Element, Length, Padding,
  widget::{Space, column, container, row},
};
pub use stats_row::Component as StatsRow;

use super::{Message, State};
use crate::style::{color, spacing};

/// Builder for the assets window header.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new header for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the header into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let picker_el = self.state.picker.render().map(Message::Picker);
    let stats_el = StatsRow::new(self.state).render();

    let content = container(
      row([picker_el, stats_el])
        .spacing(spacing::SPACE_8)
        .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .center_y(spacing::layout::HEADER_HEIGHT)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: spacing::SPACE_8,
      right: spacing::SPACE_8,
    });
    let border_line = container(Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .height(1.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      });
    column([content.into(), border_line.into()]).width(Length::Fill).into()
  }
}
