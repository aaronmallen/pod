#![allow(dead_code)]

use iced::{
  Point, Rectangle, Renderer, Theme,
  alignment::{Horizontal, Vertical},
  mouse,
  widget::canvas,
};

use crate::{
  features::market::history::{Channel, HistoryPoint, PriceBounds},
  ui::{
    format,
    style::{color, typography},
  },
};

pub const DESIGN_WIDTH: f32 = 960.0;
pub const DESIGN_HEIGHT: f32 = 380.0;
pub const PAD_LEFT: f32 = 10.0;
pub const PAD_RIGHT: f32 = 72.0;
pub const PAD_TOP: f32 = 16.0;
pub const PRICE_HEIGHT: f32 = 226.0;
pub const PANE_GAP: f32 = 16.0;
pub const VOLUME_HEIGHT: f32 = 66.0;
pub const WHISKER_MAX_POINTS: usize = 130;

const AXIS_LABEL_SIZE: f32 = 10.0;
const GRID_ALPHA: f32 = 0.06;
const LABEL_ALPHA: f32 = 0.4;
const BAND_FILL_ALPHA: f32 = 0.1;
const BAND_EDGE_ALPHA: f32 = 0.4;
const WHISKER_ALPHA: f32 = 0.22;
const MEDIAN_WIDTH: f32 = 1.6;
const GRID_FRACTIONS: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
const DATE_TICKS: usize = 6;
const YEAR_LABEL_SPAN_DAYS: i64 = 180;

pub struct Geometry {
  pub left: f32,
  pub right: f32,
  pub price_top: f32,
  pub price_bottom: f32,
  pub volume_top: f32,
  pub volume_bottom: f32,
}

impl Geometry {
  pub fn new(width: f32, height: f32) -> Self {
    let sx = width / DESIGN_WIDTH;
    let sy = height / DESIGN_HEIGHT;
    let price_top = PAD_TOP * sy;
    let price_bottom = (PAD_TOP + PRICE_HEIGHT) * sy;
    let volume_top = (PAD_TOP + PRICE_HEIGHT + PANE_GAP) * sy;
    let volume_bottom = (PAD_TOP + PRICE_HEIGHT + PANE_GAP + VOLUME_HEIGHT) * sy;
    Self {
      left: PAD_LEFT * sx,
      right: width - PAD_RIGHT * sx,
      price_top,
      price_bottom,
      volume_top,
      volume_bottom,
    }
  }

  pub fn x_at(&self, index: usize, count: usize) -> f32 {
    if count <= 1 {
      return self.left;
    }
    self.left + index as f32 / (count - 1) as f32 * (self.right - self.left)
  }

  pub fn y_price(&self, value: f64, bounds: PriceBounds) -> f32 {
    let span = (bounds.max - bounds.min).max(f64::EPSILON);
    let t = ((value - bounds.min) / span) as f32;
    self.price_bottom - t.clamp(0.0, 1.0) * (self.price_bottom - self.price_top)
  }
}

pub fn show_whiskers(count: usize) -> bool {
  count <= WHISKER_MAX_POINTS
}

pub struct HistoryChart {
  points: Vec<HistoryPoint>,
  channel: Vec<Channel>,
  bounds: PriceBounds,
}

impl HistoryChart {
  pub fn new(points: Vec<HistoryPoint>, channel: Vec<Channel>, bounds: PriceBounds) -> Self {
    Self {
      points,
      channel,
      bounds,
    }
  }

  fn draw_price_gridlines(&self, frame: &mut canvas::Frame, geo: &Geometry) {
    let stroke = canvas::Stroke::default()
      .with_width(1.0)
      .with_color(color::with_alpha(color::text::PRIMARY, GRID_ALPHA));
    let plot_h = geo.price_bottom - geo.price_top;
    for fraction in GRID_FRACTIONS {
      let y = geo.price_top + plot_h * fraction;
      frame.stroke(
        &canvas::Path::line(Point::new(geo.left, y), Point::new(geo.right, y)),
        stroke,
      );
      let value = self.bounds.max - (self.bounds.max - self.bounds.min) * fraction as f64;
      frame.fill_text(canvas::Text {
        content: format::fmt_isk(value),
        position: Point::new(geo.right + 8.0, y),
        color: color::with_alpha(color::text::PRIMARY, LABEL_ALPHA),
        size: AXIS_LABEL_SIZE.into(),
        font: typography::mono::REGULAR,
        align_x: Horizontal::Left.into(),
        align_y: Vertical::Center,
        ..canvas::Text::default()
      });
    }
  }

  fn draw_donchian_band(&self, frame: &mut canvas::Frame, geo: &Geometry) {
    let count = self.points.len();
    let band = canvas::Path::new(|builder| {
      for index in 0..count {
        let point = Point::new(geo.x_at(index, count), geo.y_price(self.channel[index].hi, self.bounds));
        if index == 0 {
          builder.move_to(point);
        } else {
          builder.line_to(point);
        }
      }
      for index in (0..count).rev() {
        builder.line_to(Point::new(
          geo.x_at(index, count),
          geo.y_price(self.channel[index].lw, self.bounds),
        ));
      }
      builder.close();
    });
    frame.fill(&band, color::with_alpha(color::accent(), BAND_FILL_ALPHA));
  }

  fn draw_donchian_edges(&self, frame: &mut canvas::Frame, geo: &Geometry) {
    let stroke = canvas::Stroke::default()
      .with_width(1.0)
      .with_color(color::with_alpha(color::accent(), BAND_EDGE_ALPHA));
    frame.stroke(&self.edge_path(geo, true), stroke);
    frame.stroke(&self.edge_path(geo, false), stroke);
  }

  fn edge_path(&self, geo: &Geometry, upper: bool) -> canvas::Path {
    let count = self.points.len();
    canvas::Path::new(|builder| {
      for index in 0..count {
        let channel = self.channel[index];
        let value = if upper { channel.hi } else { channel.lw };
        let point = Point::new(geo.x_at(index, count), geo.y_price(value, self.bounds));
        if index == 0 {
          builder.move_to(point);
        } else {
          builder.line_to(point);
        }
      }
    })
  }

  fn draw_whiskers(&self, frame: &mut canvas::Frame, geo: &Geometry) {
    let count = self.points.len();
    if !show_whiskers(count) {
      return;
    }
    let stroke = canvas::Stroke::default()
      .with_width(1.0)
      .with_color(color::with_alpha(color::text::PRIMARY, WHISKER_ALPHA));
    for (index, point) in self.points.iter().enumerate() {
      let x = geo.x_at(index, count);
      frame.stroke(
        &canvas::Path::line(
          Point::new(x, geo.y_price(point.low, self.bounds)),
          Point::new(x, geo.y_price(point.high, self.bounds)),
        ),
        stroke,
      );
    }
  }

  fn draw_median(&self, frame: &mut canvas::Frame, geo: &Geometry) {
    let count = self.points.len();
    let line = canvas::Path::new(|builder| {
      for (index, point) in self.points.iter().enumerate() {
        let plotted = Point::new(geo.x_at(index, count), geo.y_price(point.median, self.bounds));
        if index == 0 {
          builder.move_to(plotted);
        } else {
          builder.line_to(plotted);
        }
      }
    });
    frame.stroke(
      &line,
      canvas::Stroke::default()
        .with_width(MEDIAN_WIDTH)
        .with_color(color::chart::GOLD),
    );
  }

  fn draw_date_labels(&self, frame: &mut canvas::Frame, geo: &Geometry) {
    let count = self.points.len();
    let long_span = self.span_days() >= YEAR_LABEL_SPAN_DAYS;
    let baseline = geo.volume_bottom + 18.0;
    for tick in 0..=DATE_TICKS {
      let index = (tick as f32 / DATE_TICKS as f32 * (count.saturating_sub(1)) as f32).round() as usize;
      let Some(point) = self.points.get(index) else {
        continue;
      };
      frame.fill_text(canvas::Text {
        content: date_label(point, long_span),
        position: Point::new(geo.x_at(index, count), baseline),
        color: color::with_alpha(color::text::PRIMARY, LABEL_ALPHA),
        size: AXIS_LABEL_SIZE.into(),
        font: typography::mono::REGULAR,
        align_x: Horizontal::Center.into(),
        align_y: Vertical::Center,
        ..canvas::Text::default()
      });
    }
  }

  fn span_days(&self) -> i64 {
    match (self.points.first(), self.points.last()) {
      (Some(first), Some(last)) => (last.date - first.date).num_days(),
      _ => 0,
    }
  }
}

impl<Message> canvas::Program<Message> for HistoryChart {
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
    if self.points.is_empty() || self.channel.len() != self.points.len() {
      return vec![frame.into_geometry()];
    }
    let geo = Geometry::new(bounds.width, bounds.height);
    self.draw_price_gridlines(&mut frame, &geo);
    self.draw_donchian_band(&mut frame, &geo);
    self.draw_donchian_edges(&mut frame, &geo);
    self.draw_whiskers(&mut frame, &geo);
    self.draw_median(&mut frame, &geo);
    self.draw_date_labels(&mut frame, &geo);
    vec![frame.into_geometry()]
  }
}

fn date_label(point: &HistoryPoint, long_span: bool) -> String {
  use chrono::Datelike;

  let month = format::month_short(point.date.month());
  if long_span {
    format!("{month} {:02}", point.date.year() % 100)
  } else {
    format!("{month} {}", point.date.day())
  }
}

#[cfg(test)]
mod tests {
  use chrono::NaiveDate;

  use super::*;

  fn point_at(date: &str, low: f64, high: f64, median: f64) -> HistoryPoint {
    HistoryPoint {
      date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
      median,
      high,
      low,
      volume: 0,
      orders: 0,
    }
  }

  fn geometry() -> Geometry {
    Geometry::new(DESIGN_WIDTH, DESIGN_HEIGHT)
  }

  mod x_at {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_pins_the_first_index_to_the_left_edge() {
      let geo = geometry();

      assert_eq!(geo.x_at(0, 10), geo.left);
    }

    #[test]
    fn it_pins_the_last_index_to_the_right_edge() {
      let geo = geometry();

      assert_eq!(geo.x_at(9, 10), geo.right);
    }

    #[test]
    fn it_places_the_midpoint_halfway_across_the_plot() {
      let geo = geometry();

      assert_eq!(geo.x_at(1, 3), (geo.left + geo.right) / 2.0);
    }

    #[test]
    fn it_collapses_a_single_point_to_the_left_edge() {
      let geo = geometry();

      assert_eq!(geo.x_at(0, 1), geo.left);
    }
  }

  mod y_price {
    use pretty_assertions::assert_eq;

    use super::*;

    fn bounds() -> PriceBounds {
      PriceBounds {
        min: 100.0,
        max: 200.0,
      }
    }

    #[test]
    fn it_maps_the_maximum_to_the_price_top() {
      let geo = geometry();

      assert_eq!(geo.y_price(200.0, bounds()), geo.price_top);
    }

    #[test]
    fn it_maps_the_minimum_to_the_price_bottom() {
      let geo = geometry();

      assert_eq!(geo.y_price(100.0, bounds()), geo.price_bottom);
    }

    #[test]
    fn it_maps_the_midpoint_to_the_pane_centre() {
      let geo = geometry();

      assert_eq!(geo.y_price(150.0, bounds()), (geo.price_top + geo.price_bottom) / 2.0);
    }
  }

  mod whiskers {
    use super::*;

    #[test]
    fn it_draws_whiskers_at_the_threshold() {
      assert!(show_whiskers(WHISKER_MAX_POINTS));
    }

    #[test]
    fn it_drops_whiskers_past_the_threshold() {
      assert!(!show_whiskers(WHISKER_MAX_POINTS + 1));
    }
  }

  mod date_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_shows_month_and_day_for_short_spans() {
      let point = point_at("2026-07-03", 1.0, 2.0, 1.5);

      assert_eq!(date_label(&point, false), format!("{} 3", format::month_short(7)));
    }

    #[test]
    fn it_shows_month_and_two_digit_year_for_long_spans() {
      let point = point_at("2026-07-03", 1.0, 2.0, 1.5);

      assert_eq!(date_label(&point, true), format!("{} 26", format::month_short(7)));
    }
  }

  mod geometry {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reserves_the_price_and_volume_panes_by_the_design_proportions() {
      let geo = Geometry::new(DESIGN_WIDTH, DESIGN_HEIGHT);

      assert_eq!(geo.price_top, PAD_TOP);
      assert_eq!(geo.price_bottom, PAD_TOP + PRICE_HEIGHT);
      assert_eq!(geo.volume_top, PAD_TOP + PRICE_HEIGHT + PANE_GAP);
      assert_eq!(geo.volume_bottom, PAD_TOP + PRICE_HEIGHT + PANE_GAP + VOLUME_HEIGHT);
      assert_eq!(geo.left, PAD_LEFT);
      assert_eq!(geo.right, DESIGN_WIDTH - PAD_RIGHT);
    }
  }
}
