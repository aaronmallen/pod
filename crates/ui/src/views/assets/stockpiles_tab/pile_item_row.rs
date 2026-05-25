//! Pile item row component: shows fill bar, name, and count for a stockpile item.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, text},
};

use super::{super::StockpileItemStatus, fmt_count};
use crate::{
  style::{
    color,
    typography::{body, mono},
  },
  views::assets::stockpiles_tab::Message,
};

fn fill_bar(pct: f32, bar_color: Color) -> Element<'static, Message> {
  container(
    container(Space::new())
      .width(Length::FillPortion((pct * 1000.0) as u16))
      .height(2.0)
      .style(move |_| container::Style {
        background: Some(Background::Color(bar_color)),
        ..container::Style::default()
      }),
  )
  .width(Length::Fill)
  .height(2.0)
  .style(|_| container::Style {
    background: Some(Background::Color(color::border::SUBTLE)),
    ..container::Style::default()
  })
  .into()
}

fn counts_ok(have_str: &str, target_str: &str) -> Element<'static, Message> {
  column([text(format!("{} / {}", have_str, target_str))
    .font(mono::REGULAR)
    .size(12.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SUCCESS),
    })
    .into()])
  .align_x(iced::alignment::Horizontal::Right)
  .width(Length::Fixed(110.0))
  .into()
}

fn counts_short(have_str: &str, target_str: &str, need: i64) -> Element<'static, Message> {
  column([
    row([
      text(have_str.to_string())
        .font(mono::REGULAR)
        .size(12.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(format!(" / {}", target_str))
        .font(mono::REGULAR)
        .size(12.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
    ])
    .into(),
    text(format!("need {}", fmt_count(need as u64)))
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::DANGER),
      })
      .into(),
  ])
  .align_x(iced::alignment::Horizontal::Right)
  .width(Length::Fixed(110.0))
  .into()
}

fn icon_placeholder() -> Element<'static, Message> {
  container(Space::new().width(22.0).height(22.0))
    .width(22.0)
    .height(22.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::DEFAULT)),
      border: Border {
        radius: 3.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn name_and_bar(type_name: &str, pct: f32, bar_color: Color) -> Element<'_, Message> {
  column([
    text(type_name.to_string())
      .font(body::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    fill_bar(pct, bar_color),
  ])
  .spacing(4.0)
  .width(Length::Fill)
  .into()
}

/// Builder for a stockpile item row.
pub struct Component<'a> {
  item: &'a StockpileItemStatus,
}

impl<'a> Component<'a> {
  /// Creates a new pile item row for the given item status.
  pub fn new(item: &'a StockpileItemStatus) -> Self {
    Self {
      item,
    }
  }

  /// Renders the pile item row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let item = self.item;
    let ok = item.have_quantity >= item.target_quantity as i64;
    let pct = item.pct.clamp(0.0, 1.0);
    let bar_color = if ok {
      color::text::SUCCESS
    } else if pct > 0.5 {
      color::text::WARNING
    } else {
      color::text::DANGER
    };
    let have_str = fmt_count(item.have_quantity as u64);
    let target_str = fmt_count(item.target_quantity as u64);
    let need = (item.target_quantity as i64 - item.have_quantity).max(0);

    let name_bar = name_and_bar(&item.type_name, pct, bar_color);
    let counts_col = if ok {
      counts_ok(&have_str, &target_str)
    } else {
      counts_short(&have_str, &target_str, need)
    };

    container(
      row([
        icon_placeholder(),
        Space::new().width(10.0).into(),
        name_bar,
        Space::new().width(10.0).into(),
        counts_col,
      ])
      .align_y(iced::alignment::Vertical::Center)
      .padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 18.0,
        right: 18.0,
      }),
    )
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
  }
}
