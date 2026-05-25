//! Search bar shown at the top of every picker tab.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{container, row, text_input},
};

use super::super::Message;
use crate::{
  components,
  style::{color, spacing, typography::body},
};

/// Builder for the picker search bar.
pub struct Component<'a> {
  placeholder: &'static str,
  query: &'a str,
}

impl<'a> Component<'a> {
  /// Creates a new search bar with the given query and placeholder text.
  pub fn new(query: &'a str, placeholder: &'static str) -> Self {
    Self {
      placeholder,
      query,
    }
  }

  /// Renders the search bar into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let input = text_input(self.placeholder, self.query)
      .on_input(Message::PickerSearchChanged)
      .padding(Padding::ZERO)
      .size(13.0)
      .font(body::REGULAR)
      .style(|_, _| iced::widget::text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        icon: color::text::SECONDARY,
        placeholder: color::text::TERTIARY,
        value: color::text::PRIMARY,
        selection: color::accent::PLASMA_SUBTLE,
      });

    let search_row = container(
      row([
        components::Icon::search()
          .size(14.0)
          .color(color::text::SECONDARY)
          .render::<Message>(),
        input.into(),
      ])
      .spacing(10.0)
      .align_y(Vertical::Center),
    )
    .height(36.0)
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 8.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

    container(search_row)
      .padding(Padding {
        top: spacing::SPACE_3,
        bottom: spacing::SPACE_3,
        left: spacing::SPACE_4,
        right: spacing::SPACE_4,
      })
      .width(Length::Fill)
      .into()
  }
}
