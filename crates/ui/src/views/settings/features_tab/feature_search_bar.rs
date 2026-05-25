//! Search-pill with icon and result-count chip for the features panel.

use iced::{
  Background, Border, Element, Padding,
  alignment::Vertical,
  widget::{container, row, text, text_input},
};

use super::Message;
use crate::style::{color, radius, spacing};

/// Builder for the feature search bar.
pub struct FeatureSearchBar<'a> {
  query: &'a str,
  total_shown: usize,
}

impl<'a> FeatureSearchBar<'a> {
  /// Create a new search bar builder.
  pub fn new(query: &'a str, total_shown: usize) -> Self {
    Self {
      query,
      total_shown,
    }
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let search_icon = crate::components::Icon::search()
      .size(14.0)
      .color(color::text::SECONDARY)
      .render::<Message>();
    let count_chip = container(
      text(format!("{}", self.total_shown))
        .size(9.0)
        .color(color::text::TERTIARY),
    )
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: 6.0,
      right: 6.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        radius: radius::CHIP.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });
    container(
      row([
        search_icon,
        text_input("Filter features\u{2026}", self.query)
          .on_input(Message::SearchChanged)
          .size(13.0)
          .style(|_, _| text_input::Style {
            background: Background::Color(iced::Color::TRANSPARENT),
            border: Border::default(),
            icon: color::text::SECONDARY,
            placeholder: color::text::TERTIARY,
            selection: color::accent::PLASMA_SUBTLE,
            value: color::text::PRIMARY,
          })
          .into(),
        count_chip.into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
    )
    .max_width(480.0)
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::border::SUBTLE,
        radius: radius::CHIP.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }
}
