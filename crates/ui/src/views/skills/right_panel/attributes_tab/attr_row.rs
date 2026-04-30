//! Single attribute row component.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, text},
};

use super::{super::super::skill_data::AttrKey, Message};
use crate::style::{
  color, spacing,
  typography::{body, mono},
};

pub struct Component {
  key: AttrKey,
  total_val: u32,
  accent: Color,
  is_primary: bool,
  is_secondary: bool,
}

impl Component {
  pub fn new(key: AttrKey, total_val: u32, accent: Color, is_primary: bool, is_secondary: bool) -> Self {
    Self {
      key,
      total_val,
      accent,
      is_primary,
      is_secondary,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let bar_base = (self.total_val * 1000 / 35) as u16;

    let bar_row = row([
      container(
        text(self.key.short())
          .font(mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::TERTIARY),
          }),
      )
      .width(Length::Fixed(38.0))
      .into(),
      progress_bar(bar_base, self.accent),
    ])
    .align_y(Vertical::Center)
    .spacing(10.0);

    container(column([
      label_row(self.key, self.total_val, self.is_primary, self.is_secondary),
      Space::new().height(6.0).into(),
      bar_row.into(),
    ]))
    .padding(Padding {
      top: 14.0,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
  }
}

fn label_row(key: AttrKey, total_val: u32, is_primary: bool, is_secondary: bool) -> Element<'static, Message> {
  row([
    text(key.label())
      .font(body::MEDIUM)
      .size(12.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    if is_primary || is_secondary {
      container(
        text(if is_primary { "PRIMARY" } else { "SECONDARY" })
          .font(mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::accent::PLASMA),
          }),
      )
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: 8.0,
        right: 0.0,
      })
      .into()
    } else {
      Space::new().width(0.0).into()
    },
    Space::new().width(Length::Fill).into(),
    text(total_val.to_string())
      .font(mono::MEDIUM)
      .size(14.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn progress_bar(bar_base: u16, accent: Color) -> Element<'static, Message> {
  let bar_rest = 1000u16.saturating_sub(bar_base);

  container(
    row([
      container(Space::new().width(Length::Fill).height(8.0))
        .width(Length::FillPortion(bar_base))
        .height(8.0)
        .style(move |_| container::Style {
          background: Some(Background::Color(accent)),
          ..container::Style::default()
        })
        .into(),
      if bar_rest > 0 {
        Space::new().width(Length::FillPortion(bar_rest)).height(8.0).into()
      } else {
        Space::new().width(0.0).into()
      },
    ])
    .height(8.0)
    .spacing(0.0),
  )
  .height(8.0)
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
