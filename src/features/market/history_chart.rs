#![allow(dead_code)]

use iced::{
  Point, Rectangle, Renderer, Size, Theme,
  alignment::{Horizontal, Vertical},
  mouse,
  widget::canvas,
};

use crate::{
  features::market::history::{Channel, HistoryPoint, PriceBounds, max_volume},
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

const VOLUME_EYEBROW_SIZE: f32 = 9.0;
const VOLUME_EYEBROW_ALPHA: f32 = 0.35;
const VOLUME_EYEBROW_OFFSET: f32 = 4.0;
const VOLUME_BASELINE_ALPHA: f32 = 0.1;
const VOLUME_TOP_ALPHA: f32 = 0.5;
const VOLUME_BOTTOM_ALPHA: f32 = 0.12;
const VOLUME_BAR_RATIO: f32 = 0.68;
const VOLUME_BAR_MIN: f32 = 1.0;
const VOLUME_BAR_MAX: f32 = 7.0;

const CROSSHAIR_ALPHA: f32 = 0.3;
const CROSSHAIR_DASH: [f32; 2] = [3.0, 3.0];
const MEDIAN_DOT_RADIUS: f32 = 3.5;
const MEDIAN_DOT_BORDER: f32 = 1.5;

const TOOLTIP_CARD_WIDTH: f32 = 150.0;
const TOOLTIP_PAD_X: f32 = 12.0;
const TOOLTIP_PAD_Y: f32 = 9.0;
const TOOLTIP_HEADER_SIZE: f32 = 9.0;
const TOOLTIP_LABEL_SIZE: f32 = 10.0;
const TOOLTIP_VALUE_SIZE: f32 = 11.0;
const TOOLTIP_HEADER_GAP: f32 = 18.0;
const TOOLTIP_ROW_HEIGHT: f32 = 15.0;
const TOOLTIP_ROWS: f32 = 5.0;
const TOOLTIP_TOP: f32 = 4.0;
const TOOLTIP_GAP: f32 = 8.0;
const TOOLTIP_CORNER: f32 = 8.0;
const TOOLTIP_FLIP_FRACTION: f32 = 0.62;

pub struct Geometry {
  pub width: f32,
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
      width,
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

  pub fn y_volume(&self, value: i64, peak: i64) -> f32 {
    if peak <= 0 {
      return self.volume_bottom;
    }
    let t = (value as f32 / peak as f32).clamp(0.0, 1.0);
    self.volume_bottom - t * (self.volume_bottom - self.volume_top)
  }

  pub fn bar_width(&self, count: usize) -> f32 {
    if count == 0 {
      return VOLUME_BAR_MIN;
    }
    ((self.right - self.left) / count as f32 * VOLUME_BAR_RATIO).clamp(VOLUME_BAR_MIN, VOLUME_BAR_MAX)
  }

  pub fn index_at(&self, x: f32, count: usize) -> usize {
    if count <= 1 {
      return 0;
    }
    let span = (self.right - self.left).max(f32::EPSILON);
    let fraction = ((x - self.left) / span).clamp(0.0, 1.0);
    (fraction * (count - 1) as f32).round() as usize
  }
}

#[derive(Default)]
pub struct HoverState {
  index: Option<usize>,
}

fn clear_hover<Message>(state: &mut HoverState) -> Option<canvas::Action<Message>> {
  state.index.take().map(|_| canvas::Action::request_redraw())
}

fn hover_at<Message>(
  points: &[HistoryPoint],
  state: &mut HoverState,
  bounds: Rectangle,
  cursor: mouse::Cursor,
) -> Option<canvas::Action<Message>> {
  if points.is_empty() {
    return None;
  }
  let Some(pos) = cursor.position_in(bounds) else {
    return clear_hover(state);
  };
  let index = Geometry::new(bounds.width, bounds.height).index_at(pos.x, points.len());
  if state.index == Some(index) {
    return None;
  }
  state.index = Some(index);
  Some(canvas::Action::request_redraw())
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

  fn draw_volume(&self, frame: &mut canvas::Frame, geo: &Geometry, peak: i64) {
    frame.stroke(
      &canvas::Path::line(
        Point::new(geo.left, geo.volume_bottom),
        Point::new(geo.right, geo.volume_bottom),
      ),
      canvas::Stroke::default()
        .with_width(1.0)
        .with_color(color::with_alpha(color::text::PRIMARY, VOLUME_BASELINE_ALPHA)),
    );
    frame.fill_text(canvas::Text {
      content: t!("market.chart_volume_eyebrow").into_owned(),
      position: Point::new(geo.left, geo.volume_top - VOLUME_EYEBROW_OFFSET),
      color: color::with_alpha(color::text::PRIMARY, VOLUME_EYEBROW_ALPHA),
      size: VOLUME_EYEBROW_SIZE.into(),
      font: typography::mono::REGULAR,
      align_x: Horizontal::Left.into(),
      align_y: Vertical::Bottom,
      ..canvas::Text::default()
    });
    let count = self.points.len();
    let width = geo.bar_width(count);
    for (index, point) in self.points.iter().enumerate() {
      let top = geo.y_volume(point.volume, peak);
      let height = geo.volume_bottom - top;
      if height <= 0.0 {
        continue;
      }
      let origin = Point::new(geo.x_at(index, count) - width / 2.0, top);
      frame.fill(
        &canvas::Path::rectangle(origin, Size::new(width, height)),
        canvas::gradient::Linear::new(Point::new(0.0, top), Point::new(0.0, geo.volume_bottom))
          .add_stop(0.0, color::with_alpha(color::accent(), VOLUME_TOP_ALPHA))
          .add_stop(1.0, color::with_alpha(color::accent(), VOLUME_BOTTOM_ALPHA)),
      );
    }
  }

  fn draw_crosshair(&self, frame: &mut canvas::Frame, geo: &Geometry, index: usize) {
    let Some(point) = self.points.get(index) else {
      return;
    };
    let x = geo.x_at(index, self.points.len());
    frame.stroke(
      &canvas::Path::line(Point::new(x, geo.price_top), Point::new(x, geo.volume_bottom)),
      canvas::Stroke {
        line_dash: canvas::LineDash {
          segments: &CROSSHAIR_DASH,
          offset: 0,
        },
        ..canvas::Stroke::default()
          .with_width(1.0)
          .with_color(color::with_alpha(color::text::PRIMARY, CROSSHAIR_ALPHA))
      },
    );
    let dot = Point::new(x, geo.y_price(point.median, self.bounds));
    frame.fill(&canvas::Path::circle(dot, MEDIAN_DOT_RADIUS), color::chart::GOLD);
    frame.stroke(
      &canvas::Path::circle(dot, MEDIAN_DOT_RADIUS),
      canvas::Stroke::default()
        .with_width(MEDIAN_DOT_BORDER)
        .with_color(color::surface::BASE),
    );
  }

  fn draw_tooltip(&self, frame: &mut canvas::Frame, geo: &Geometry, index: usize) {
    let Some(point) = self.points.get(index) else {
      return;
    };
    let x = geo.x_at(index, self.points.len());
    let flip = x / geo.width > TOOLTIP_FLIP_FRACTION;
    let raw_x = if flip {
      x - TOOLTIP_CARD_WIDTH - TOOLTIP_GAP
    } else {
      x + TOOLTIP_GAP
    };
    let card_x = raw_x.min(geo.width - TOOLTIP_CARD_WIDTH).max(0.0);
    let card_y = TOOLTIP_TOP;
    let card_h = TOOLTIP_PAD_Y * 2.0 + TOOLTIP_HEADER_GAP + TOOLTIP_ROWS * TOOLTIP_ROW_HEIGHT;

    let card = canvas::Path::rounded_rectangle(
      Point::new(card_x, card_y),
      Size::new(TOOLTIP_CARD_WIDTH, card_h),
      TOOLTIP_CORNER.into(),
    );
    frame.fill(&card, color::surface::RAISED);
    frame.stroke(
      &card,
      canvas::Stroke::default()
        .with_width(1.0)
        .with_color(color::rule_strong()),
    );
    frame.fill_text(canvas::Text {
      content: tooltip_date_label(point),
      position: Point::new(card_x + TOOLTIP_PAD_X, card_y + TOOLTIP_PAD_Y),
      color: color::text::secondary(),
      size: TOOLTIP_HEADER_SIZE.into(),
      font: typography::mono::REGULAR,
      ..canvas::Text::default()
    });

    let mut row_y = card_y + TOOLTIP_PAD_Y + TOOLTIP_HEADER_GAP;
    for (label, value, tint) in tooltip_rows(point) {
      frame.fill_text(canvas::Text {
        content: label,
        position: Point::new(card_x + TOOLTIP_PAD_X, row_y),
        color: color::text::secondary(),
        size: TOOLTIP_LABEL_SIZE.into(),
        font: typography::mono::REGULAR,
        ..canvas::Text::default()
      });
      frame.fill_text(canvas::Text {
        content: value,
        position: Point::new(card_x + TOOLTIP_CARD_WIDTH - TOOLTIP_PAD_X, row_y),
        color: tint,
        size: TOOLTIP_VALUE_SIZE.into(),
        font: typography::mono::REGULAR,
        align_x: Horizontal::Right.into(),
        ..canvas::Text::default()
      });
      row_y += TOOLTIP_ROW_HEIGHT;
    }
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
  type State = HoverState;

  fn update(
    &self,
    state: &mut Self::State,
    event: &canvas::Event,
    bounds: Rectangle,
    cursor: mouse::Cursor,
  ) -> Option<canvas::Action<Message>> {
    let iced::Event::Mouse(mouse_event) = event else {
      return None;
    };
    match mouse_event {
      mouse::Event::CursorMoved {
        ..
      } => hover_at(&self.points, state, bounds, cursor),
      mouse::Event::CursorLeft => clear_hover(state),
      _ => None,
    }
  }

  fn draw(
    &self,
    state: &Self::State,
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
    self.draw_volume(&mut frame, &geo, max_volume(&self.points));
    self.draw_date_labels(&mut frame, &geo);
    if let Some(index) = state.index {
      self.draw_crosshair(&mut frame, &geo, index);
      self.draw_tooltip(&mut frame, &geo, index);
    }
    vec![frame.into_geometry()]
  }

  fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
    if cursor.is_over(bounds) {
      mouse::Interaction::Crosshair
    } else {
      mouse::Interaction::default()
    }
  }
}

fn tooltip_rows(point: &HistoryPoint) -> [(String, String, iced::Color); 5] {
  [
    (
      t!("market.chart_tooltip_median").into_owned(),
      format::fmt_isk(point.median),
      color::chart::GOLD,
    ),
    (
      t!("market.chart_tooltip_high").into_owned(),
      format::fmt_isk(point.high),
      color::status::ONLINE,
    ),
    (
      t!("market.chart_tooltip_low").into_owned(),
      format::fmt_isk(point.low),
      color::status::DANGER,
    ),
    (
      t!("market.chart_tooltip_volume").into_owned(),
      format::fmt_count(point.volume),
      color::text::PRIMARY,
    ),
    (
      t!("market.chart_tooltip_orders").into_owned(),
      format::fmt_count(point.orders),
      color::text::secondary(),
    ),
  ]
}

fn tooltip_date_label(point: &HistoryPoint) -> String {
  use chrono::Datelike;

  format!(
    "{} {}, {}",
    format::month_short(point.date.month()),
    point.date.day(),
    point.date.year()
  )
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

  mod y_volume {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_the_peak_to_the_volume_top() {
      let geo = geometry();

      assert_eq!(geo.y_volume(90, 90), geo.volume_top);
    }

    #[test]
    fn it_maps_zero_to_the_volume_bottom() {
      let geo = geometry();

      assert_eq!(geo.y_volume(0, 90), geo.volume_bottom);
    }

    #[test]
    fn it_maps_the_midpoint_to_the_pane_centre() {
      let geo = geometry();

      assert_eq!(geo.y_volume(45, 90), (geo.volume_top + geo.volume_bottom) / 2.0);
    }

    #[test]
    fn it_flattens_to_the_baseline_when_the_peak_is_zero() {
      let geo = geometry();

      assert_eq!(geo.y_volume(0, 0), geo.volume_bottom);
    }
  }

  mod bar_width {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_to_the_maximum_when_days_are_sparse() {
      let geo = geometry();

      assert_eq!(geo.bar_width(4), VOLUME_BAR_MAX);
    }

    #[test]
    fn it_clamps_to_the_minimum_when_days_are_dense() {
      let geo = geometry();

      assert_eq!(geo.bar_width(2000), VOLUME_BAR_MIN);
    }

    #[test]
    fn it_falls_back_to_the_minimum_for_no_days() {
      let geo = geometry();

      assert_eq!(geo.bar_width(0), VOLUME_BAR_MIN);
    }
  }

  mod index_at {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_snaps_the_left_edge_to_the_first_index() {
      let geo = geometry();

      assert_eq!(geo.index_at(geo.left, 10), 0);
    }

    #[test]
    fn it_snaps_the_right_edge_to_the_last_index() {
      let geo = geometry();

      assert_eq!(geo.index_at(geo.right, 10), 9);
    }

    #[test]
    fn it_snaps_the_midpoint_to_the_middle_index() {
      let geo = geometry();

      assert_eq!(geo.index_at((geo.left + geo.right) / 2.0, 3), 1);
    }

    #[test]
    fn it_clamps_beyond_the_right_edge_to_the_last_index() {
      let geo = geometry();

      assert_eq!(geo.index_at(geo.right + 500.0, 10), 9);
    }

    #[test]
    fn it_collapses_a_single_point_to_the_first_index() {
      let geo = geometry();

      assert_eq!(geo.index_at(geo.right, 1), 0);
    }
  }

  mod tooltip {
    use pretty_assertions::assert_eq;

    use super::*;

    fn point() -> HistoryPoint {
      let mut point = point_at("2026-07-13", 90.0, 110.0, 100.0);
      point.volume = 1_234;
      point.orders = 7;
      point
    }

    #[test]
    fn it_reads_every_field_of_the_hovered_day() {
      let rows = tooltip_rows(&point());

      let values: Vec<String> = rows.iter().map(|(_, value, _)| value.clone()).collect();
      assert_eq!(
        values,
        vec![
          format::fmt_isk(100.0),
          format::fmt_isk(110.0),
          format::fmt_isk(90.0),
          format::fmt_count(1_234),
          format::fmt_count(7),
        ]
      );
    }

    #[test]
    fn it_tints_the_median_gold() {
      let rows = tooltip_rows(&point());

      assert_eq!(rows[0].2, color::chart::GOLD);
    }

    #[test]
    fn it_tints_the_high_and_low_by_direction() {
      let rows = tooltip_rows(&point());

      assert_eq!(rows[1].2, color::status::ONLINE);
      assert_eq!(rows[2].2, color::status::DANGER);
    }

    #[test]
    fn it_labels_the_header_with_the_day() {
      assert_eq!(
        tooltip_date_label(&point()),
        format!("{} 13, 2026", format::month_short(7))
      );
    }
  }
}
