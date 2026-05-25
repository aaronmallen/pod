//! Shared bar-chart row helpers used by time-by-group and time-by-pair sections.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, text},
};

use super::super::Message;
use crate::style::{
  color,
  typography::{body, mono},
};

/// Renders a single bar-chart row: label + time string, then a filled bar track.
pub fn bar_chart_row(label: String, time_str: String, fraction: f32, bar_color: Color) -> Element<'static, Message> {
  let filled = (fraction * 1000.0) as u16;
  let rest = 1000u16.saturating_sub(filled);

  column([
    bar_label_row(label, time_str),
    Space::new().height(4.0).into(),
    bar_track(filled, rest, bar_color),
  ])
  .width(Length::Fill)
  .into()
}

fn bar_label_row(label: String, time_str: String) -> Element<'static, Message> {
  row([
    text(label)
      .font(body::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    text(time_str)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn bar_track(filled: u16, rest: u16, bar_color: Color) -> Element<'static, Message> {
  container(
    row([
      container(Space::new().width(Length::Fill).height(4.0))
        .width(Length::FillPortion(filled))
        .height(4.0)
        .style(move |_| container::Style {
          background: Some(Background::Color(bar_color)),
          border: Border {
            radius: 2.0.into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into(),
      if rest > 0 {
        Space::new().width(Length::FillPortion(rest)).height(4.0).into()
      } else {
        Space::new().width(0.0).into()
      },
    ])
    .height(4.0)
    .spacing(0.0),
  )
  .height(4.0)
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::border::SUBTLE)),
    border: Border {
      radius: 2.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

/// Renders the titled container that wraps a set of bar chart rows.
pub fn time_chart_section(title: &'static str, rows: Vec<Element<'static, Message>>) -> Element<'static, Message> {
  use crate::components::section_label;

  container(
    column([
      container(section_label(title))
        .padding(Padding {
          top: 0.0,
          bottom: crate::style::spacing::SPACE_3,
          left: 0.0,
          right: 0.0,
        })
        .width(Length::Fill)
        .into(),
      column(rows).width(Length::Fill).into(),
    ])
    .width(Length::Fill),
  )
  .padding(Padding {
    top: crate::style::spacing::SPACE_3,
    bottom: crate::style::spacing::SPACE_4,
    left: crate::style::spacing::SPACE_4,
    right: crate::style::spacing::SPACE_4,
  })
  .width(Length::Fill)
  .into()
}
