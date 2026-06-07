use chrono::{Duration, Utc};
use iced::{
  Background, Border, Color, Element, Length, Padding, Point, Rectangle, Renderer, Theme,
  alignment::{Horizontal, Vertical},
  mouse,
  widget::{Column, Row, canvas, container, text},
};

use super::{HEADER_SIDE_PADDING, Message, Scope, fmt_isk};
use crate::{
  store::{Database, repo::finance as net_worth},
  ui::style::{color, radius, spacing, typography},
};

const WINDOW_DAYS: i64 = 90;
const AVG_WINDOW_DAYS: usize = 30;
const GRAPH_HEIGHT: f32 = 280.0;
const PLOT_PAD_Y: f32 = 16.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NavPoint {
  pub date: String,
  pub value: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NavSeries {
  pub(super) points: Vec<NavPoint>,
}

impl NavSeries {
  fn gridline_ys(height: f32) -> Vec<f32> {
    (0..=4)
      .map(|i| PLOT_PAD_Y + (i as f32 / 4.0) * (height - PLOT_PAD_Y * 2.0))
      .collect()
  }

  fn change(&self) -> Option<(f64, f64)> {
    let first = self.points.first()?.value;
    let last = self.points.last()?.value;
    let delta = last - first;
    let pct = if first.abs() > f64::EPSILON {
      delta / first * 100.0
    } else {
      0.0
    };
    Some((delta, pct))
  }

  fn current(&self) -> Option<f64> {
    self.points.last().map(|p| p.value)
  }

  fn high(&self) -> Option<f64> {
    self
      .points
      .iter()
      .map(|p| p.value)
      .fold(None, |acc, v| Some(acc.map_or(v, |m: f64| m.max(v))))
  }

  fn is_rising(&self) -> bool {
    self.change().map(|(delta, _)| delta >= 0.0).unwrap_or(true)
  }

  fn low(&self) -> Option<f64> {
    self
      .points
      .iter()
      .map(|p| p.value)
      .fold(None, |acc, v| Some(acc.map_or(v, |m: f64| m.min(v))))
  }

  fn plot_points(&self, width: f32, height: f32, range: (f64, f64)) -> Vec<Point> {
    self
      .points
      .iter()
      .enumerate()
      .map(|(index, point)| Point::new(self.x_at(index, width), self.y_at(point.value, height, range)))
      .collect()
  }

  fn thirty_day_avg(&self) -> Option<f64> {
    if self.points.is_empty() {
      return None;
    }
    let tail = &self.points[self.points.len().saturating_sub(AVG_WINDOW_DAYS)..];
    Some(tail.iter().map(|p| p.value).sum::<f64>() / tail.len() as f64)
  }

  fn value_range(&self) -> (f64, f64) {
    let min = self.low().unwrap_or(0.0);
    let max = self.high().unwrap_or(1.0);
    if (max - min).abs() < f64::EPSILON {
      return (min - 1.0, max + 1.0);
    }
    let pad = (max - min) * 0.12;
    (min - pad, max + pad)
  }

  fn x_at(&self, index: usize, width: f32) -> f32 {
    let last = (self.points.len().max(2) - 1) as f32;
    (index as f32 / last) * width
  }

  fn y_at(&self, value: f64, height: f32, range: (f64, f64)) -> f32 {
    let (min, max) = range;
    let plot_h = height - PLOT_PAD_Y * 2.0;
    let t = ((value - min) / (max - min)) as f32;
    PLOT_PAD_Y + plot_h - t.clamp(0.0, 1.0) * plot_h
  }
}

pub(super) async fn load_series(db: &Database, scope: Scope) -> NavSeries {
  let since = (Utc::now() - Duration::days(WINDOW_DAYS))
    .format("%Y-%m-%d")
    .to_string();

  let points = match scope {
    Scope::All => net_worth::combined_series_since(db, &since)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|p| NavPoint {
        date: p.date().clone(),
        value: p.net_worth().unwrap_or(0.0),
      })
      .collect(),
    Scope::Character(id) => net_worth::for_character_since(db, id, &since)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|p| NavPoint {
        date: p.date().clone(),
        value: p.net_worth(),
      })
      .collect(),
    Scope::Corporation(_) => Vec::new(),
  };

  NavSeries {
    points,
  }
}

pub(super) fn body(series: &NavSeries) -> Element<'_, Message> {
  if series.points.is_empty() {
    return empty_state();
  }

  let stats = stat_tiles(series);
  let chart = chart_card(series);

  container(
    Column::with_children(vec![stats, chart])
      .spacing(spacing::SPACE_3_5)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6,
    right: HEADER_SIDE_PADDING,
    bottom: spacing::SPACE_6 + spacing::SPACE_2,
    left: HEADER_SIDE_PADDING,
  })
  .into()
}

fn stat_tiles(series: &NavSeries) -> Element<'_, Message> {
  let current = series.current().unwrap_or(0.0);
  let (delta, pct) = series.change().unwrap_or((0.0, 0.0));
  let change_color = if delta >= 0.0 {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let change_value = format!("{}{}", if delta >= 0.0 { "+" } else { "-" }, fmt_isk(delta.abs()));
  let change_sub = format!("{}{:.2}%", if pct >= 0.0 { "+" } else { "" }, pct);

  Row::with_children(vec![
    stat_tile("Current", fmt_isk(current), None, color::text::PRIMARY),
    stat_tile("90-day change", change_value, Some(change_sub), change_color),
    stat_tile(
      "High",
      fmt_isk(series.high().unwrap_or(0.0)),
      None,
      color::text::SECONDARY,
    ),
    stat_tile(
      "Low",
      fmt_isk(series.low().unwrap_or(0.0)),
      None,
      color::text::SECONDARY,
    ),
    stat_tile(
      "30d avg",
      fmt_isk(series.thirty_day_avg().unwrap_or(0.0)),
      None,
      color::text::SECONDARY,
    ),
  ])
  .spacing(spacing::SPACE_3_5)
  .width(Length::Fill)
  .into()
}

fn stat_tile<'a>(label: &'a str, value: String, sub: Option<String>, value_color: Color) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    text(value)
      .font(typography::mono::MEDIUM)
      .size(typography::size::LG)
      .style(move |_| text::Style {
        color: Some(value_color),
      })
      .into(),
  ];
  if let Some(sub) = sub {
    children.push(
      text(sub)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(move |_| text::Style {
          color: Some(value_color),
        })
        .into(),
    );
  }

  container(Column::with_children(children).spacing(spacing::UNIT + 1.0))
    .width(Length::Fill)
    .padding(spacing::SPACE_3_5)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn chart_card(series: &NavSeries) -> Element<'_, Message> {
  let heading = Row::with_children(vec![
    text("Net asset value \u{b7} 90 days")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text("DAILY SNAPSHOT")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  let chart = canvas(Chart {
    series,
  })
  .width(Length::Fill)
  .height(Length::Fixed(GRAPH_HEIGHT));

  container(
    Column::with_children(vec![heading.into(), chart.into()])
      .spacing(spacing::SPACE_3)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_3_5)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: radius::CARD.into(),
    },
    ..container::Style::default()
  })
  .into()
}

struct Chart<'a> {
  series: &'a NavSeries,
}

impl canvas::Program<Message> for Chart<'_> {
  type State = ();

  fn draw(
    &self,
    _state: &Self::State,
    renderer: &Renderer,
    _theme: &Theme,
    bounds: Rectangle,
    _cursor: mouse::Cursor,
  ) -> Vec<canvas::Geometry> {
    let mut frame = canvas::Frame::new(renderer, bounds.size());
    let width = bounds.width;
    let height = bounds.height;
    if self.series.points.is_empty() {
      return vec![frame.into_geometry()];
    }
    let range = self.series.value_range();
    let plot = self.series.plot_points(width, height, range);
    let line_color = if self.series.is_rising() {
      color::accent::PLASMA
    } else {
      color::status::DANGER
    };

    let grid_stroke = canvas::Stroke::default()
      .with_width(1.0)
      .with_color(color::with_alpha(color::text::PRIMARY, 0.06));
    stroke_gridlines(&mut frame, width, height, grid_stroke);
    frame.fill(
      &area_path(&plot, height - PLOT_PAD_Y),
      color::with_alpha(line_color, 0.12),
    );
    frame.stroke(
      &line_path(&plot),
      canvas::Stroke::default().with_width(1.5).with_color(line_color),
    );
    frame.fill(&canvas::Path::circle(plot[plot.len() - 1], 3.5), line_color);

    vec![frame.into_geometry()]
  }
}

fn stroke_gridlines(frame: &mut canvas::Frame, width: f32, height: f32, stroke: canvas::Stroke) {
  for y in NavSeries::gridline_ys(height) {
    frame.stroke(&canvas::Path::line(Point::new(0.0, y), Point::new(width, y)), stroke);
  }
}

fn area_path(plot: &[Point], baseline: f32) -> canvas::Path {
  canvas::Path::new(|builder| {
    builder.move_to(Point::new(plot[0].x, baseline));
    for point in plot {
      builder.line_to(*point);
    }
    builder.line_to(Point::new(plot[plot.len() - 1].x, baseline));
    builder.close();
  })
}

fn line_path(plot: &[Point]) -> canvas::Path {
  canvas::Path::new(|builder| {
    builder.move_to(plot[0]);
    for point in &plot[1..] {
      builder.line_to(*point);
    }
  })
}

fn empty_state<'a>() -> Element<'a, Message> {
  container(
    text("No net-worth history yet \u{2014} snapshots accrue daily.")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(spacing::SPACE_6 * 2.0)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn point(date: &str, value: f64) -> NavPoint {
    NavPoint {
      date: date.to_owned(),
      value,
    }
  }

  fn sample_series() -> NavSeries {
    NavSeries {
      points: vec![
        point("2026-03-05", 1_000.0),
        point("2026-04-05", 1_500.0),
        point("2026-05-05", 800.0),
        point("2026-06-03", 1_200.0),
      ],
    }
  }

  mod stats {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_computes_current_change_high_and_low() {
      let series = sample_series();

      assert_eq!(series.current(), Some(1_200.0));
      let (delta, pct) = series.change().unwrap();
      assert_eq!(delta, 200.0);
      assert_eq!(pct, 20.0);
      assert_eq!(series.high(), Some(1_500.0));
      assert_eq!(series.low(), Some(800.0));
    }

    #[test]
    fn it_averages_the_trailing_window() {
      let series = sample_series();
      assert_eq!(series.thirty_day_avg(), Some(1_125.0));
    }

    #[test]
    fn it_has_no_stats_for_an_empty_series() {
      let series = NavSeries::default();
      assert_eq!(series.current(), None);
      assert_eq!(series.change(), None);
      assert_eq!(series.thirty_day_avg(), None);
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_the_tracker_body_from_a_sample_series() {
      let series = sample_series();
      let _el: Element<'_, Message> = body(&series);
    }

    #[test]
    fn it_renders_the_empty_tracker_body() {
      let series = NavSeries::default();
      let _el: Element<'_, Message> = body(&series);
    }
  }

  mod geometry {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_plots_every_point_spanning_the_full_width() {
      let series = sample_series();
      let range = series.value_range();
      let plot = series.plot_points(300.0, 280.0, range);

      assert_eq!(plot.len(), 4);
      assert_eq!(plot[0].x, 0.0);
      assert_eq!(plot[3].x, 300.0);
      for point in &plot {
        assert!(point.y >= PLOT_PAD_Y - f32::EPSILON);
        assert!(point.y <= 280.0 - PLOT_PAD_Y + f32::EPSILON);
      }
    }

    #[test]
    fn it_lays_five_evenly_spaced_gridlines() {
      let ys = NavSeries::gridline_ys(280.0);
      assert_eq!(ys.len(), 5);
      assert_eq!(ys[0], PLOT_PAD_Y);
      assert_eq!(ys[4], 280.0 - PLOT_PAD_Y);
    }

    #[test]
    fn it_reads_a_gaining_series_as_rising_and_a_losing_series_as_falling() {
      assert!(sample_series().is_rising());
      let falling = NavSeries {
        points: vec![point("2026-05-01", 2_000.0), point("2026-06-01", 1_000.0)],
      };
      assert!(!falling.is_rising());
      assert!(NavSeries::default().is_rising());
    }

    #[test]
    fn it_builds_the_area_and_line_paths_over_the_plotted_points() {
      let series = sample_series();
      let range = series.value_range();
      let plot = series.plot_points(300.0, 280.0, range);

      let _line = line_path(&plot);
      let _area = area_path(&plot, 280.0 - PLOT_PAD_Y);
    }
  }
}
