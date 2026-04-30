//! Wallet header — character picker button, stats row, and bottom border.

pub mod stats_row;

use iced::{
  Background, Element, Length, Padding,
  widget::{Space, column, container, row},
};
pub use stats_row::Component as StatsRow;

use crate::{
  style::{color, spacing},
  views::wallet::{Message, State},
};

fn separator_v<'a>() -> Element<'a, Message> {
  use iced::widget::Space;

  container(Space::new().width(1.0).height(32.0))
    .width(1.0)
    .height(32.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into()
}

/// Builder for the wallet header.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new header component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the wallet header into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let picker_el = self.state.picker.render().map(Message::CharacterPicker);
    let stats_row = StatsRow::new(self.state).render();
    let content = container(
      row([picker_el, separator_v(), stats_row])
        .align_y(iced::alignment::Vertical::Center)
        .spacing(spacing::SPACE_8),
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
