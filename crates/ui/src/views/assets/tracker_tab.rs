//! NAV chart + timeframe picker tracker tab.

use chrono::NaiveDate;
use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, row, scrollable, text},
};

use super::{State, TrackerRange};
use crate::{
  components::LineChart,
  format,
  style::{
    color,
    typography::{body, mono},
  },
};

/// Messages produced by the tracker tab.
#[derive(Clone, Debug)]
pub enum Message {
  TrackerRangeChanged(TrackerRange),
}

fn range_row<'a>(active_range: &'a TrackerRange) -> Element<'a, Message> {
  let btns: Vec<Element<'_, Message>> = TrackerRange::all()
    .iter()
    .map(|r| {
      let active = r == active_range;
      let msg = Message::TrackerRangeChanged(r.clone());
      button(
        text(r.label())
          .font(mono::MEDIUM)
          .size(9.0)
          .style(move |_: &Theme| iced::widget::text::Style {
            color: Some(if active {
              color::accent::PLASMA
            } else {
              color::text::SECONDARY
            }),
          }),
      )
      .padding(Padding {
        top: 4.0,
        bottom: 4.0,
        left: 10.0,
        right: 10.0,
      })
      .on_press(msg)
      .style(move |_, status| button::Style {
        background: if active {
          Some(Background::Color(color::accent::PLASMA_HIGHLIGHT))
        } else {
          match status {
            button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
            _ => None,
          }
        },
        border: Border {
          radius: 0.0.into(),
          ..Border::default()
        },
        text_color: if active {
          color::accent::PLASMA
        } else {
          color::text::SECONDARY
        },
        ..button::Style::default()
      })
      .into()
    })
    .collect();
  container(row(btns))
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        radius: 5.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn x_labels_for_dates(dated: &[(NaiveDate, f64)]) -> Vec<String> {
  if dated.len() < 2 {
    return Vec::new();
  }
  let last_date = dated.last().map(|(d, _)| *d).unwrap();
  let step = (dated.len() as f32 / 6.0).ceil() as usize;
  let indices: Vec<usize> = (0..dated.len()).step_by(step.max(1)).collect();
  indices
    .into_iter()
    .map(|i| {
      let d = dated[i].0;
      let diff = (last_date - d).num_days();
      if diff == 0 {
        "now".to_string()
      } else {
        format!("-{}d", diff)
      }
    })
    .collect()
}

fn chart_card<'a>(dated: &'a [(NaiveDate, f64)], active_range: &'a TrackerRange) -> Element<'a, Message> {
  let range_el = range_row(active_range);
  let range_label = active_range.label();
  let chart_header: Element<'_, Message> = row([
    text(format!("Net asset value · {range_label}"))
      .font(body::MEDIUM)
      .size(14.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(10.0).into(),
    text("daily snapshot · ESI")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    range_el,
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into();

  let series: Vec<f64> = dated.iter().map(|(_, v)| *v).collect();
  let x_labels = x_labels_for_dates(dated);
  let chart = LineChart::new(series, color::accent::PLASMA)
    .with_padding(60.0, 24.0, 24.0, 36.0)
    .with_labels(x_labels, format::fmt_isk)
    .render(Length::Fill, 280.0);
  container(
    column([chart_header, Space::new().height(10.0).into(), chart]).padding(Padding {
      top: 18.0,
      bottom: 6.0,
      left: 6.0,
      right: 6.0,
    }),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 10.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn empty_chart_card(active_range: &TrackerRange) -> Element<'_, Message> {
  let range_el = range_row(active_range);
  let chart_header: Element<'_, Message> = row([
    text("Net asset value")
      .font(body::MEDIUM)
      .size(14.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(10.0).into(),
    text("daily snapshot · ESI")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    range_el,
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into();

  let empty_body: Element<'_, Message> = container(
    text("Price history builds up over time — check back after the first full day")
      .font(mono::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(Length::Fill)
  .height(280.0)
  .align_x(iced::alignment::Horizontal::Center)
  .align_y(iced::alignment::Vertical::Center)
  .into();

  container(
    column([chart_header, Space::new().height(10.0).into(), empty_body]).padding(Padding {
      top: 18.0,
      bottom: 6.0,
      left: 6.0,
      right: 6.0,
    }),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 10.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn stat_tile<'a>(label: &str, value: String, sub_value: Option<String>, accent: Color) -> Element<'a, Message> {
  let mut col_children: Vec<Element<'_, Message>> = vec![
    text(label.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(6.0).into(),
    text(value)
      .font(mono::MEDIUM)
      .size(17.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(accent),
      })
      .into(),
  ];
  if let Some(sub) = sub_value {
    col_children.push(
      text(sub)
        .font(mono::REGULAR)
        .size(10.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(accent),
        })
        .into(),
    );
  }
  container(column(col_children))
    .padding(Padding {
      top: 14.0,
      bottom: 14.0,
      left: 16.0,
      right: 16.0,
    })
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 8.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

/// Builder for the NAV tracker tab.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new tracker tab for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the tracker tab into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let dated = state.visible_nav_history();
    let series: Vec<f64> = dated.iter().map(|(_, v)| *v).collect();

    let last = series.last().copied().unwrap_or(0.0);
    let first = series.first().copied().unwrap_or(last);
    let change = last - first;
    let change_pct = if first > 0.0 { change / first * 100.0 } else { 0.0 };
    let high = series.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let low = series.iter().cloned().fold(f64::INFINITY, f64::min);
    let last30: Vec<_> = series.iter().rev().take(30).cloned().collect();
    let avg30 = if last30.is_empty() {
      0.0
    } else {
      last30.iter().sum::<f64>() / last30.len() as f64
    };
    let is_up = change >= 0.0;
    let change_color = if is_up {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };

    let range_label = state.tracker_range.label();
    let stat_tiles: Vec<Element<'_, Message>> = vec![
      stat_tile("Current", format::fmt_isk(last), None, color::text::PRIMARY),
      stat_tile(
        &format!("{range_label} change"),
        format!("{}{}", if is_up { "+" } else { "-" }, format::fmt_isk(change.abs())),
        Some(format!(
          "{}{:.2}%",
          if change_pct >= 0.0 { "+" } else { "" },
          change_pct
        )),
        change_color,
      ),
      stat_tile("High", format::fmt_isk(high), None, color::text::SECONDARY),
      stat_tile("Low", format::fmt_isk(low), None, color::text::SECONDARY),
      stat_tile("30d avg", format::fmt_isk(avg30), None, color::text::SECONDARY),
    ];

    let stats_row: Element<'_, Message> = row(stat_tiles).spacing(14.0).into();

    let chart_card_el = if series.len() >= 2 {
      chart_card(dated, &state.tracker_range)
    } else {
      empty_chart_card(&state.tracker_range)
    };

    scrollable(
      container(column([stats_row, Space::new().height(18.0).into(), chart_card_el]))
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
