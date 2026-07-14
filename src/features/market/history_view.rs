use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, canvas, container, text},
};

use super::{
  Message, history,
  history::{HistoryPoint, PriceBounds, Range},
  history_chart::{self, HistoryChart},
};
use crate::ui::{
  components::{eyebrow::eyebrow_text, segmented::segment_button},
  format::{fmt_count, fmt_isk},
  style::{color, radius, spacing, typography},
};

const OUTER_PAD: Padding = Padding {
  top: 16.0,
  right: 20.0,
  bottom: 24.0,
  left: 20.0,
};
const RANGE_PAD: Padding = Padding {
  top: 5.0,
  right: 12.0,
  bottom: 5.0,
  left: 12.0,
};
const STRIP_GAP: f32 = 26.0;
const STRIP_BOTTOM: f32 = 14.0;
const LEGEND_GAP: f32 = 18.0;
const LEGEND_BOTTOM: f32 = 8.0;
const SWATCH_GAP: f32 = 7.0;
const MEDIAN_SWATCH: (f32, f32) = (16.0, 2.0);
const DONCHIAN_SWATCH: (f32, f32) = (16.0, 9.0);
const VOLUME_SWATCH: (f32, f32) = (10.0, 9.0);
const BAND_FILL_ALPHA: f32 = 0.18;
const BAND_EDGE_ALPHA: f32 = 0.45;
const VOLUME_SWATCH_ALPHA: f32 = 0.4;

pub(super) fn graph(points: &[HistoryPoint], range: Range) -> Element<'static, Message> {
  container(assemble(points, range))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(OUTER_PAD)
    .into()
}

fn assemble(points: &[HistoryPoint], range: Range) -> Element<'static, Message> {
  match build(points, range) {
    Some(body) => body,
    None => range_selector(range),
  }
}

fn build(points: &[HistoryPoint], range: Range) -> Option<Element<'static, Message>> {
  let sliced = history::slice_range(points, range);
  let last = sliced.last()?;
  let bounds = history::price_bounds(sliced)?;
  let (low, high) = extremes(sliced)?;

  Some(
    Column::with_children(vec![
      padded_bottom(stat_strip(last, high, low, range), STRIP_BOTTOM),
      padded_bottom(legend(), LEGEND_BOTTOM),
      chart(sliced, bounds),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into(),
  )
}

fn stat_strip(last: &HistoryPoint, high: f64, low: f64, range: Range) -> Element<'static, Message> {
  let days = range.days();
  Row::with_children(vec![
    stat_chip(
      t!("market.history_stat_median").into_owned(),
      fmt_isk(last.median),
      color::chart::GOLD,
    ),
    stat_chip(
      t!("market.history_stat_volume").into_owned(),
      fmt_count(last.volume),
      color::text::PRIMARY,
    ),
    stat_chip(
      t!("market.history_stat_high", days => days).into_owned(),
      fmt_isk(high),
      color::status::ONLINE,
    ),
    stat_chip(
      t!("market.history_stat_low", days => days).into_owned(),
      fmt_isk(low),
      color::status::DANGER,
    ),
    Space::new().width(Length::Fill).into(),
    range_selector(range),
  ])
  .spacing(STRIP_GAP)
  .align_y(Vertical::Center)
  .into()
}

fn stat_chip(label: String, value: String, accent: Color) -> Element<'static, Message> {
  Column::with_children(vec![
    eyebrow_text(&label, None).into(),
    text(value)
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(accent))
      .into(),
  ])
  .spacing(spacing::UNIT / 2.0)
  .into()
}

fn range_selector(active: Range) -> Element<'static, Message> {
  let segments: Vec<Element<'static, Message>> = Range::ORDER
    .into_iter()
    .map(|range| {
      segment_button(
        range.label(),
        range == active,
        RANGE_PAD,
        Message::HistoryRangeSelected(range),
      )
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

fn legend() -> Element<'static, Message> {
  Row::with_children(vec![
    legend_item(
      swatch(MEDIAN_SWATCH, color::chart::GOLD, None, radius::SUBTLE / 2.0),
      t!("market.history_legend_median").into_owned(),
    ),
    legend_item(
      swatch(
        DONCHIAN_SWATCH,
        color::with_alpha(color::accent(), BAND_FILL_ALPHA),
        Some(color::with_alpha(color::accent(), BAND_EDGE_ALPHA)),
        radius::SUBTLE,
      ),
      t!("market.history_legend_donchian").into_owned(),
    ),
    legend_item(
      swatch(
        VOLUME_SWATCH,
        color::with_alpha(color::accent(), VOLUME_SWATCH_ALPHA),
        None,
        radius::SUBTLE / 2.0,
      ),
      t!("market.history_legend_volume").into_owned(),
    ),
  ])
  .spacing(LEGEND_GAP)
  .align_y(Vertical::Center)
  .into()
}

fn legend_item(swatch: Element<'static, Message>, label: String) -> Element<'static, Message> {
  Row::with_children(vec![swatch, eyebrow_text(&label, None).into()])
    .spacing(SWATCH_GAP)
    .align_y(Vertical::Center)
    .into()
}

fn swatch(size: (f32, f32), fill: Color, edge: Option<Color>, corner: f32) -> Element<'static, Message> {
  container(Space::new().width(Length::Fixed(size.0)).height(Length::Fixed(size.1)))
    .width(Length::Fixed(size.0))
    .height(Length::Fixed(size.1))
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        color: edge.unwrap_or(Color::TRANSPARENT),
        width: if edge.is_some() { 1.0 } else { 0.0 },
        radius: corner.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn chart(sliced: &[HistoryPoint], bounds: PriceBounds) -> Element<'static, Message> {
  let points = sliced.to_vec();
  let channel = history::donchian(&points, history::DONCHIAN_WINDOW);
  canvas(HistoryChart::new(points, channel, bounds))
    .width(Length::Fill)
    .height(Length::Fixed(history_chart::DESIGN_HEIGHT))
    .into()
}

fn padded_bottom(content: Element<'static, Message>, bottom: f32) -> Element<'static, Message> {
  container(content)
    .padding(Padding {
      top: 0.0,
      right: 0.0,
      bottom,
      left: 0.0,
    })
    .into()
}

fn extremes(points: &[HistoryPoint]) -> Option<(f64, f64)> {
  let mut low = f64::INFINITY;
  let mut high = f64::NEG_INFINITY;
  for point in points {
    low = low.min(point.low);
    high = high.max(point.high);
  }
  (low.is_finite() && high.is_finite()).then_some((low, high))
}

#[cfg(test)]
mod tests {
  use chrono::NaiveDate;

  use super::*;

  fn point(day: u32, low: f64, high: f64, median: f64, volume: i64) -> HistoryPoint {
    HistoryPoint {
      date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() + chrono::Duration::days(day as i64),
      median,
      high,
      low,
      volume,
      orders: 0,
    }
  }

  fn series(count: u32) -> Vec<HistoryPoint> {
    (0..count)
      .map(|day| point(day, day as f64, day as f64 + 5.0, day as f64 + 2.0, day as i64 * 10))
      .collect()
  }

  #[test]
  fn it_assembles_the_graph_for_a_loaded_series() {
    let points = series(120);

    let _el: Element<'static, Message> = graph(&points, Range::ThreeMonths);
  }

  #[test]
  fn it_assembles_the_graph_for_every_range() {
    let points = series(365);

    for range in Range::ORDER {
      let _el: Element<'static, Message> = graph(&points, range);
    }
  }

  #[test]
  fn it_falls_back_to_the_range_selector_for_an_empty_series() {
    let _el: Element<'static, Message> = graph(&[], Range::ThreeMonths);
  }

  #[test]
  fn it_reads_the_true_low_and_high_of_the_slice() {
    let points = vec![
      point(0, 3.0, 8.0, 5.0, 1),
      point(1, 1.0, 6.0, 4.0, 2),
      point(2, 4.0, 9.0, 6.0, 3),
    ];

    assert_eq!(extremes(&points), Some((1.0, 9.0)));
  }

  #[test]
  fn it_has_no_extremes_for_an_empty_slice() {
    assert_eq!(extremes(&[]), None);
  }
}
