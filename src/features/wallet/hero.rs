use chrono::{DateTime, NaiveDate, Utc};
use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, canvas, container, text},
};

use super::{Composition, Message, NetWorthPoint, Scope, State, Timeframe, fmt_isk};
use crate::ui::{
  components::{
    eyebrow::eyebrow_text,
    line_chart::{self, ChartPoint, LineChart},
    segmented::segment_button_style,
    status,
  },
  style::{color, radius, spacing, typography},
};

const GRAPH_HEIGHT: f32 = 220.0;
const BAR_HEIGHT: f32 = 6.0;
const COMPOSITION_CHIP_WIDTH: f32 = 130.0;

pub(super) fn hero(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  let today = now.date_naive();
  let window = super::timeframe_window(state.timeframe, today);
  let sliced = super::sliced_series(state, today);
  let current = super::series_current(sliced);
  let change = super::series_change(sliced);
  let composition = super::scope_composition(state);

  let displayed = hovered_value(state, sliced, window).or(current);

  let head = Row::with_children(vec![
    big_number(state, displayed, change),
    Space::new().width(Length::Fill).into(),
    composition_chips(composition),
    timeframe_selector(state),
  ])
  .spacing(spacing::SPACE_6)
  .align_y(Vertical::Top);

  let mut children: Vec<Element<'_, Message>> = vec![head.into(), graph(state, sliced, window)];
  if let Some(stack) = composition_stack(state) {
    children.push(stack);
  }

  container(
    Column::with_children(children)
      .spacing(spacing::SPACE_3)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6,
    right: super::HEADER_SIDE_PADDING,
    bottom: spacing::SPACE_3_5,
    left: super::HEADER_SIDE_PADDING,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn chart_points(sliced: &[NetWorthPoint]) -> Vec<ChartPoint> {
  sliced
    .iter()
    .map(|point| ChartPoint {
      date: point.date.clone(),
      liquid: Some(point.liquid),
      value: point.net_worth,
    })
    .collect()
}

fn hovered_value(state: &State, sliced: &[NetWorthPoint], window: (NaiveDate, NaiveDate)) -> Option<f64> {
  let fraction = state.chart_hover?;
  let points = chart_points(sliced);
  line_chart::nearest_index(&points, window, fraction).map(|idx| sliced[idx].net_worth)
}

fn big_number<'a>(state: &'a State, value: Option<f64>, change: f64) -> Element<'a, Message> {
  let scope_label = match state.active {
    Scope::All => "Net worth \u{00b7} all characters".to_owned(),
    _ => "Net worth \u{00b7} est.".to_owned(),
  };

  let up = change >= 0.0;
  let change_color = if up {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let arrow = if up { "\u{25b2}" } else { "\u{25bc}" };
  let sign = if up { "+" } else { "-" };
  let chip = container(
    Row::with_children(vec![
      text(format!("{arrow} {sign}{} ISK", fmt_isk(Some(change.abs()))))
        .font(typography::mono::MEDIUM)
        .size(typography::size::SM)
        .style(move |_| text::Style {
          color: Some(change_color),
        })
        .into(),
      text(state.timeframe.label())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::UNIT,
    right: spacing::SPACE_2_5,
    bottom: spacing::UNIT,
    left: spacing::SPACE_2_5,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(change_color, 0.1))),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  Column::with_children(vec![
    eyebrow_text(&scope_label, None).into(),
    Row::with_children(vec![
      text(fmt_isk(value))
        .font(typography::body::MEDIUM)
        .size(34.0)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text("ISK")
        .font(typography::body::REGULAR)
        .size(typography::size::LG)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Bottom)
    .into(),
    chip.into(),
  ])
  .spacing(spacing::SPACE_2)
  .into()
}

fn composition_chips<'a>(composition: Composition) -> Element<'a, Message> {
  Row::with_children(vec![
    composition_chip("Liquid", composition.liquid, color::accent::PLASMA),
    composition_chip("Assets", composition.asset_value, color::text::SECONDARY),
    composition_chip("Escrow", composition.escrow, color::status::DANGER),
  ])
  .spacing(spacing::SPACE_3)
  .into()
}

fn composition_chip<'a>(label: &str, value: Option<f64>, dot: Color) -> Element<'a, Message> {
  let head = Row::with_children(vec![status::dot(dot), eyebrow_text(label, None).into()])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

  container(
    Column::with_children(vec![
      head.into(),
      text(fmt_isk(value))
        .font(typography::mono::MEDIUM)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .spacing(spacing::UNIT)
    .width(Length::Fixed(COMPOSITION_CHIP_WIDTH)),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3_5,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn timeframe_selector(state: &State) -> Element<'_, Message> {
  let segments: Vec<Element<'_, Message>> = Timeframe::all()
    .into_iter()
    .map(|timeframe| {
      let active = state.timeframe == timeframe;
      iced::widget::button(
        text(timeframe.label())
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(move |_| text::Style {
            color: Some(if active {
              color::accent::PLASMA
            } else {
              color::text::SECONDARY
            }),
          }),
      )
      .padding(Padding {
        top: spacing::SPACE_2,
        right: spacing::SPACE_3,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_3,
      })
      .on_press(Message::TimeframeSelected(timeframe))
      .style(move |_, status| segment_button_style(active, status))
      .into()
    })
    .collect();

  container(Row::with_children(segments))
    .style(|_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn graph<'a>(state: &'a State, sliced: &'a [NetWorthPoint], window: (NaiveDate, NaiveDate)) -> Element<'a, Message> {
  if sliced.len() < 2 {
    return container(
      text("Net-worth history will appear after the next daily aggregation run.")
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .width(Length::Fill)
    .height(Length::Fixed(GRAPH_HEIGHT))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into();
  }

  let line_color = if super::series_change(sliced) >= 0.0 {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };

  canvas(
    LineChart::new(
      chart_points(sliced),
      window,
      line_color,
      |value| fmt_isk(Some(value)),
      Message::ChartHovered,
    )
    .hover(state.chart_hover)
    .liquid("Liquid", color::accent::PLASMA),
  )
  .width(Length::Fill)
  .height(Length::Fixed(GRAPH_HEIGHT))
  .into()
}

fn composition_stack(state: &State) -> Option<Element<'_, Message>> {
  let slices = super::composition_stack(state);
  let total: f64 = slices.iter().map(|slice| slice.net_worth).sum();
  if slices.is_empty() || total <= 0.0 {
    return None;
  }

  let bar_segments: Vec<Element<'_, Message>> = slices
    .iter()
    .enumerate()
    .map(|(index, slice)| {
      let share = ((slice.net_worth / total * 100.0).round() as u16).max(1);
      container(Space::new().width(Length::Fill).height(Length::Fixed(BAR_HEIGHT)))
        .width(Length::FillPortion(share))
        .height(Length::Fixed(BAR_HEIGHT))
        .style(move |_| container::Style {
          background: Some(Background::Color(color::chart::series(index))),
          ..container::Style::default()
        })
        .into()
    })
    .collect();

  let bar = container(
    Row::with_children(bar_segments)
      .width(Length::Fill)
      .height(Length::Fixed(BAR_HEIGHT)),
  )
  .width(Length::Fill)
  .height(Length::Fixed(BAR_HEIGHT))
  .clip(true)
  .style(|_| container::Style {
    border: Border {
      radius: 3.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  let legend: Vec<Element<'_, Message>> = slices
    .iter()
    .enumerate()
    .map(|(index, slice)| {
      let pct = slice.net_worth / total * 100.0;
      Row::with_children(vec![
        container(Space::new().width(Length::Fixed(8.0)).height(Length::Fixed(8.0)))
          .style(move |_| container::Style {
            background: Some(Background::Color(color::chart::series(index))),
            border: Border {
              radius: radius::SUBTLE.into(),
              ..Border::default()
            },
            ..container::Style::default()
          })
          .into(),
        text(slice.name.clone())
          .font(typography::body::MEDIUM)
          .size(typography::size::SM)
          .style(|_| text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        text(format!("{}  \u{00b7} {pct:.1}%", fmt_isk(Some(slice.net_worth))))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(|_| text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into()
    })
    .collect();

  Some(
    Column::with_children(vec![
      eyebrow_text("By character", None).into(),
      bar.into(),
      Row::with_children(legend).spacing(spacing::SPACE_6).wrap().into(),
    ])
    .spacing(spacing::SPACE_2)
    .into(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn day(date: &str) -> NaiveDate {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap()
  }

  fn window() -> (NaiveDate, NaiveDate) {
    (day("2026-06-01"), day("2026-06-05"))
  }

  fn dated(date: &str, net_worth: f64) -> NetWorthPoint {
    NetWorthPoint {
      date: date.to_owned(),
      liquid: 0.0,
      net_worth,
    }
  }

  mod chart_points {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_net_worth_to_value_and_carries_liquid() {
      let sliced = [NetWorthPoint {
        date: "2026-06-03".to_owned(),
        liquid: 25.0,
        net_worth: 100.0,
      }];

      let points = super::chart_points(&sliced);

      assert_eq!(points.len(), 1);
      assert_eq!(points[0].date, "2026-06-03");
      assert_eq!(points[0].value, 100.0);
      assert_eq!(points[0].liquid, Some(25.0));
    }
  }

  mod hovered_value {
    use pretty_assertions::assert_eq;

    use super::*;

    fn state_with_hover(hover: Option<f32>) -> State {
      let mut state = State::new();
      state.chart_hover = hover;
      state
    }

    #[test]
    fn it_is_none_when_not_hovering() {
      let sliced = [dated("2026-06-01", 1.0), dated("2026-06-05", 2.0)];

      assert_eq!(super::hovered_value(&state_with_hover(None), &sliced, window()), None);
    }

    #[test]
    fn it_reads_the_net_worth_at_the_nearest_point() {
      let sliced = [
        dated("2026-06-01", 1.0),
        dated("2026-06-03", 2.0),
        dated("2026-06-05", 3.0),
      ];

      assert_eq!(
        super::hovered_value(&state_with_hover(Some(1.0)), &sliced, window()),
        Some(3.0)
      );
    }
  }
}
