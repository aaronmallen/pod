use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Row, container, text, text_input},
};

use super::Message;
use crate::ui::style::{color, radius, spacing, typography};

pub fn search_box(query: &str) -> Element<'_, Message> {
  let field = Row::with_children(vec![
    text("\u{2315}")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    text_input("Search skills\u{2026}", query)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .padding(Padding::ZERO)
      .on_input(Message::SearchChanged)
      .style(input_style)
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  container(field)
    .width(Length::Fill)
    .height(Length::Fixed(36.0))
    .padding(Padding {
      top: 0.0,
      right: spacing::SPACE_3,
      bottom: 0.0,
      left: spacing::SPACE_3,
    })
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(iced::Color::TRANSPARENT),
    border: Border::default(),
    icon: color::text::SECONDARY,
    placeholder: color::text::TERTIARY,
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent::PLASMA, 0.4),
  }
}
