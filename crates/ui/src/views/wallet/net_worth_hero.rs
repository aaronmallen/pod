//! Net worth hero section — value display, change badge, chart, and timeframe.

pub mod composition_chip;
pub mod timeframe_picker;

pub use composition_chip::Component as CompositionChip;
use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, text},
};
pub use timeframe_picker::Component as TimeframePicker;

use crate::{
  components::LineChart,
  format,
  style::{
    color,
    typography::{body, mono},
  },
  views::wallet::{Message, State},
};

fn hero_lhs(current: f64, change: f64, change_pct: f64, is_up: bool) -> Element<'static, Message> {
  let change_color = if is_up {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let hero_label: Element<'_, Message> = text("NET WORTH")
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
    .into();
  let hero_value: Element<'_, Message> = text(format::fmt_isk_full(current))
    .font(body::MEDIUM)
    .size(32.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    })
    .into();
  let change_sign = if is_up { "▲" } else { "▼" };
  let change_str = format!(
    "{} {} · {}{:.2}%",
    change_sign,
    format::fmt_isk(change.abs()),
    if change_pct >= 0.0 { "+" } else { "-" },
    change_pct.abs(),
  );
  let change_badge: Element<'_, Message> = container(text(change_str).font(mono::MEDIUM).size(11.0).style(
    move |_: &Theme| iced::widget::text::Style {
      color: Some(change_color),
    },
  ))
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 10.0,
    right: 10.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(Color {
      a: 0.10,
      ..change_color
    })),
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into();
  column([
    hero_label,
    Space::new().height(6.0).into(),
    hero_value,
    Space::new().height(8.0).into(),
    change_badge,
  ])
  .into()
}

fn chart_section<'a>(top_row: Element<'a, Message>, series: Vec<f64>, is_up: bool) -> Element<'a, Message> {
  use crate::style::spacing;

  let chart_color = if is_up {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let bottom_rule: Element<'_, Message> = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into();
  column([
    container(column([
      top_row,
      Space::new().height(16.0).into(),
      LineChart::new(series, chart_color).render::<Message>(Length::Fill, 180.0),
    ]))
    .width(Length::Fill)
    .padding(Padding {
      top: 24.0,
      bottom: 12.0,
      left: spacing::SPACE_8,
      right: spacing::SPACE_8,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into(),
    bottom_rule,
  ])
  .width(Length::Fill)
  .into()
}

/// Builder for the net worth hero section.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new net worth hero component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the net worth hero into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let series = state.chart_series.clone();
    let current = state.total_liquid() + state.total_assets() + state.total_escrow();
    let change = state.net_worth_change;
    let start_balance = (current - change).max(0.0);
    let change_pct = if start_balance > 0.01 {
      change / start_balance * 100.0
    } else {
      0.0
    };
    let is_up = change >= 0.0;
    let left_col = hero_lhs(current, change, change_pct, is_up);
    let comp_chips: Element<'_, Message> = row([
      CompositionChip::new("Liquid", state.total_liquid(), color::accent::PLASMA).render(),
      Space::new().width(10.0).into(),
      CompositionChip::new(
        "Assets",
        state.total_assets(),
        Color::from_rgba(0.957, 0.949, 0.925, 0.55),
      )
      .render(),
      Space::new().width(10.0).into(),
      CompositionChip::new("Escrow", state.total_escrow(), color::status::CAUTION).render(),
    ])
    .into();
    let top_row: Element<'_, Message> = row([
      left_col,
      Space::new().width(Length::Fill).into(),
      comp_chips,
      Space::new().width(24.0).into(),
      TimeframePicker::new(&state.timeframe).render(),
    ])
    .align_y(iced::alignment::Vertical::Center)
    .into();
    chart_section(top_row, series, is_up)
  }
}
