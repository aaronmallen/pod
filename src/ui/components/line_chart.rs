use chrono::NaiveDate;
use iced::{
  Color, Point, Rectangle, Renderer, Size, Theme,
  alignment::{Horizontal, Vertical},
  mouse,
  widget::canvas,
};

use crate::ui::style::{color, typography};

const AXIS_LABEL_SIZE: f32 = 9.0;
const HOVER_DASH: [f32; 2] = [3.0, 3.0];
const TOOLTIP_CARD_WIDTH: f32 = 150.0;

#[derive(Clone)]
pub struct ChartPoint {
  pub date: String,
  pub liquid: Option<f64>,
  pub value: f64,
}

pub struct LineChart<'a, Message> {
  date_label: Box<dyn Fn(i64, i64) -> String + 'a>,
  hover: Option<f32>,
  line_color: Color,
  liquid_color: Color,
  liquid_label: &'a str,
  on_hover: Box<dyn Fn(Option<f32>) -> Message + 'a>,
  pad_bottom: f32,
  pad_top: f32,
  points: Vec<ChartPoint>,
  show_axis_labels: bool,
  show_tooltip: bool,
  value_label: Box<dyn Fn(f64) -> String + 'a>,
  value_pad: f64,
  window: (NaiveDate, NaiveDate),
}

impl<'a, Message> LineChart<'a, Message> {
  pub fn new(
    points: Vec<ChartPoint>,
    window: (NaiveDate, NaiveDate),
    line_color: Color,
    value_label: impl Fn(f64) -> String + 'a,
    on_hover: impl Fn(Option<f32>) -> Message + 'a,
  ) -> Self {
    Self {
      date_label: Box::new(time_ago_label),
      hover: None,
      line_color,
      liquid_color: color::accent::PLASMA,
      liquid_label: "Liquid",
      on_hover: Box::new(on_hover),
      pad_bottom: 24.0,
      pad_top: 14.0,
      points,
      show_axis_labels: true,
      show_tooltip: true,
      value_label: Box::new(value_label),
      value_pad: 0.08,
      window,
    }
  }

  // Consumed by the asset tracker (next consumer); kept on the shared API.
  #[expect(dead_code)]
  /// Overrides the default x-axis tick formatter.
  ///
  /// The closure receives `(days_ago, span_days)` and returns a display string.
  pub fn date_label(mut self, date_label: impl Fn(i64, i64) -> String + 'a) -> Self {
    self.date_label = Box::new(date_label);
    self
  }

  pub fn hover(mut self, hover: Option<f32>) -> Self {
    self.hover = hover;
    self
  }

  pub fn liquid(mut self, label: &'a str, dot: Color) -> Self {
    self.liquid_label = label;
    self.liquid_color = dot;
    self
  }

  pub fn padding(mut self, top: f32, bottom: f32) -> Self {
    self.pad_top = top;
    self.pad_bottom = bottom;
    self
  }

  /// Sets the fractional whitespace added above and below the data range.
  ///
  /// For example, `0.08` adds 8% of `(max - min)` to each end of the Y axis.
  pub fn value_pad(mut self, value_pad: f64) -> Self {
    self.value_pad = value_pad;
    self
  }

  fn draw_area(&self, frame: &mut canvas::Frame, width: f32, height: f32, range: (f64, f64)) {
    let baseline = height - self.pad_bottom;
    let area = canvas::Path::new(|builder| {
      builder.move_to(Point::new(self.x_at(0, width), baseline));
      for (i, point) in self.points.iter().enumerate() {
        builder.line_to(Point::new(self.x_at(i, width), self.y_at(point.value, height, range)));
      }
      builder.line_to(Point::new(self.x_at(self.points.len() - 1, width), baseline));
      builder.close();
    });
    frame.fill(&area, color::with_alpha(self.line_color, 0.12));
  }

  fn draw_axis_labels(&self, frame: &mut canvas::Frame, width: f32, height: f32) {
    let (start, end) = self.window;
    let span_days = (end - start).num_days();
    for i in 0..=4 {
      let t = i as f32 / 4.0;
      let days_ago = ((1.0 - t) * span_days as f32).round() as i64;
      let label = (self.date_label)(days_ago, span_days);
      frame.fill_text(canvas::Text {
        content: label,
        position: Point::new(t * width, height - 4.0),
        color: color::text::dim(),
        size: AXIS_LABEL_SIZE.into(),
        font: typography::mono::REGULAR,
        align_x: tick_alignment(i).into(),
        align_y: Vertical::Bottom,
        ..canvas::Text::default()
      });
    }
  }

  fn draw_gridlines(&self, frame: &mut canvas::Frame, width: f32, height: f32, range: (f64, f64)) {
    let (min, max) = range;
    let plot_h = height - self.pad_top - self.pad_bottom;
    let grid_stroke = canvas::Stroke::default()
      .with_width(1.0)
      .with_color(color::with_alpha(color::text::PRIMARY, 0.06));
    for i in 0..=4 {
      let t = i as f32 / 4.0;
      let y = self.pad_top + t * plot_h;
      frame.stroke(
        &canvas::Path::line(Point::new(0.0, y), Point::new(width, y)),
        grid_stroke,
      );
      let grid_value = max - (max - min) * t as f64;
      frame.fill_text(canvas::Text {
        content: (self.value_label)(grid_value),
        position: Point::new(width - 4.0, y - 3.0),
        color: color::text::tertiary(),
        size: AXIS_LABEL_SIZE.into(),
        font: typography::mono::REGULAR,
        align_x: Horizontal::Right.into(),
        align_y: Vertical::Bottom,
        ..canvas::Text::default()
      });
    }
  }

  fn draw_line(&self, frame: &mut canvas::Frame, width: f32, height: f32, range: (f64, f64)) {
    let line = canvas::Path::new(|builder| {
      for (i, point) in self.points.iter().enumerate() {
        let p = Point::new(self.x_at(i, width), self.y_at(point.value, height, range));
        if i == 0 {
          builder.move_to(p);
        } else {
          builder.line_to(p);
        }
      }
    });
    frame.stroke(
      &line,
      canvas::Stroke::default().with_width(2.0).with_color(self.line_color),
    );
  }

  fn draw_liquid(&self, frame: &mut canvas::Frame, width: f32, height: f32, range: (f64, f64)) {
    if !self.has_liquid() {
      return;
    }
    let baseline = height - self.pad_bottom;

    frame.fill(
      &self.liquid_fill_path(width, height, range, baseline),
      canvas::gradient::Linear::new(Point::new(0.0, self.pad_top), Point::new(0.0, baseline))
        .add_stop(0.0, color::with_alpha(self.liquid_color, 0.2))
        .add_stop(1.0, color::with_alpha(self.liquid_color, 0.0)),
    );

    frame.stroke(
      &self.liquid_line_path(width, height, range),
      canvas::Stroke::default()
        .with_width(1.75)
        .with_color(color::with_alpha(self.liquid_color, 0.92)),
    );

    let last = self.points.len() - 1;
    frame.fill(
      &canvas::Path::circle(
        Point::new(self.x_at(last, width), self.y_at(self.liquid_at(last), height, range)),
        3.5,
      ),
      self.liquid_color,
    );
  }

  fn draw_marker(&self, frame: &mut canvas::Frame, width: f32, height: f32, range: (f64, f64)) {
    let idx = self.marker_index();
    let x = self.x_at(idx, width);
    let y = self.y_at(self.points[idx].value, height, range);

    if self.hovered().is_some() {
      frame.stroke(
        &canvas::Path::line(Point::new(x, self.pad_top), Point::new(x, height - self.pad_bottom)),
        canvas::Stroke {
          line_dash: canvas::LineDash {
            segments: &HOVER_DASH,
            offset: 0,
          },
          ..canvas::Stroke::default()
            .with_width(1.0)
            .with_color(color::with_alpha(self.line_color, 0.6))
        },
      );
      frame.fill(&canvas::Path::circle(Point::new(x, y), 5.0), color::surface::BASE);
      frame.stroke(
        &canvas::Path::circle(Point::new(x, y), 5.0),
        canvas::Stroke::default().with_width(2.0).with_color(self.line_color),
      );
    } else {
      frame.fill(&canvas::Path::circle(Point::new(x, y), 4.0), self.line_color);
    }
  }

  fn draw_tooltip(&self, frame: &mut canvas::Frame, width: f32, height: f32, range: (f64, f64)) {
    let Some(idx) = self.hovered() else {
      return;
    };
    let point = &self.points[idx];
    let x = self.x_at(idx, width);
    let y = self.y_at(point.value, height, range);

    let date = point.date.split('T').next().unwrap_or(&point.date).to_owned();
    let value = format!("{} ISK", (self.value_label)(point.value));

    let delta = (idx > 0).then(|| point.value - self.points[idx - 1].value);
    let liquid = self.has_liquid().then_some(self.liquid_at(idx));

    let card_w = TOOLTIP_CARD_WIDTH;
    let card_h = tooltip_card_height(liquid.is_some(), delta.is_some());
    let card_x = (x + 12.0).min(width - card_w).max(0.0);
    let card_y = (y - card_h - 12.0).max(self.pad_top);

    let card = canvas::Path::rounded_rectangle(Point::new(card_x, card_y), Size::new(card_w, card_h), 6.0.into());
    frame.fill(&card, color::surface::RAISED);
    frame.stroke(
      &card,
      canvas::Stroke::default()
        .with_width(1.0)
        .with_color(color::rule_strong()),
    );

    frame.fill_text(canvas::Text {
      content: date.to_uppercase(),
      position: Point::new(card_x + 10.0, card_y + 8.0),
      color: color::text::secondary(),
      size: 9.0.into(),
      font: typography::mono::REGULAR,
      ..canvas::Text::default()
    });
    frame.fill_text(canvas::Text {
      content: value,
      position: Point::new(card_x + 10.0, card_y + 20.0),
      color: color::text::PRIMARY,
      size: 13.0.into(),
      font: typography::mono::MEDIUM,
      ..canvas::Text::default()
    });
    let mut row_y = card_y + 38.0;
    if let Some(liquid) = liquid {
      frame.fill(
        &canvas::Path::circle(Point::new(card_x + 13.0, row_y + 4.0), 3.0),
        self.liquid_color,
      );
      frame.fill_text(canvas::Text {
        content: self.liquid_label.to_owned(),
        position: Point::new(card_x + 22.0, row_y),
        color: color::text::secondary(),
        size: 9.0.into(),
        font: typography::mono::REGULAR,
        ..canvas::Text::default()
      });
      frame.fill_text(canvas::Text {
        content: (self.value_label)(liquid),
        position: Point::new(card_x + card_w - 10.0, row_y),
        color: self.liquid_color,
        size: 10.0.into(),
        font: typography::mono::REGULAR,
        align_x: Horizontal::Right.into(),
        ..canvas::Text::default()
      });
      row_y += 16.0;
    }
    if let Some(delta) = delta {
      let (sign, delta_color) = delta_style(delta);
      frame.fill_text(canvas::Text {
        content: format!("{sign}{} ISK", (self.value_label)(delta.abs())),
        position: Point::new(card_x + 10.0, row_y),
        color: delta_color,
        size: 10.0.into(),
        font: typography::mono::REGULAR,
        ..canvas::Text::default()
      });
    }
  }

  fn has_liquid(&self) -> bool {
    self.points.iter().any(|point| point.liquid.unwrap_or(0.0) != 0.0)
  }

  fn hovered(&self) -> Option<usize> {
    self
      .hover
      .and_then(|fraction| nearest_index(&self.points, self.window, fraction))
  }

  fn liquid_at(&self, idx: usize) -> f64 {
    self.points[idx].liquid.unwrap_or(0.0)
  }

  fn liquid_fill_path(&self, width: f32, height: f32, range: (f64, f64), baseline: f32) -> canvas::Path {
    canvas::Path::new(|builder| {
      builder.move_to(Point::new(self.x_at(0, width), baseline));
      for (i, _) in self.points.iter().enumerate() {
        builder.line_to(Point::new(
          self.x_at(i, width),
          self.y_at(self.liquid_at(i), height, range),
        ));
      }
      builder.line_to(Point::new(self.x_at(self.points.len() - 1, width), baseline));
      builder.close();
    })
  }

  fn liquid_line_path(&self, width: f32, height: f32, range: (f64, f64)) -> canvas::Path {
    canvas::Path::new(|builder| {
      for (i, _) in self.points.iter().enumerate() {
        let p = Point::new(self.x_at(i, width), self.y_at(self.liquid_at(i), height, range));
        if i == 0 {
          builder.move_to(p);
        } else {
          builder.line_to(p);
        }
      }
    })
  }

  fn marker_index(&self) -> usize {
    self.hovered().unwrap_or_else(|| self.points.len() - 1)
  }

  fn value_range(&self) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for point in &self.points {
      min = min.min(point.value);
      max = max.max(point.value);
    }
    if !min.is_finite() || !max.is_finite() {
      return (0.0, 1.0);
    }
    if (max - min).abs() < f64::EPSILON {
      return (min - 1.0, max + 1.0);
    }
    let pad = (max - min) * self.value_pad;
    (min - pad, max + pad)
  }

  fn x_at(&self, i: usize, width: f32) -> f32 {
    date_fraction(self.window, &self.points[i].date) * width
  }

  fn y_at(&self, value: f64, height: f32, range: (f64, f64)) -> f32 {
    let (min, max) = range;
    let plot_h = height - self.pad_top - self.pad_bottom;
    let t = ((value - min) / (max - min)) as f32;
    self.pad_top + plot_h - t.clamp(0.0, 1.0) * plot_h
  }
}

impl<Message> canvas::Program<Message> for LineChart<'_, Message> {
  type State = ();

  fn update(
    &self,
    _state: &mut Self::State,
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
      } => {
        let fraction = cursor
          .position_in(bounds)
          .map(|pos| (pos.x / bounds.width).clamp(0.0, 1.0));
        if fraction != self.hover {
          return Some(canvas::Action::publish((self.on_hover)(fraction)));
        }
        None
      }
      mouse::Event::CursorLeft if self.hover.is_some() => Some(canvas::Action::publish((self.on_hover)(None))),
      _ => None,
    }
  }

  fn draw(
    &self,
    _state: &Self::State,
    renderer: &Renderer,
    _theme: &Theme,
    bounds: Rectangle,
    _cursor: mouse::Cursor,
  ) -> Vec<canvas::Geometry> {
    let mut frame = canvas::Frame::new(renderer, bounds.size());
    if self.points.is_empty() {
      return vec![frame.into_geometry()];
    }
    let width = bounds.width;
    let height = bounds.height;
    let range = self.value_range();

    self.draw_gridlines(&mut frame, width, height, range);
    self.draw_area(&mut frame, width, height, range);
    self.draw_liquid(&mut frame, width, height, range);
    self.draw_line(&mut frame, width, height, range);
    self.draw_marker(&mut frame, width, height, range);
    if self.show_axis_labels {
      self.draw_axis_labels(&mut frame, width, height);
    }
    if self.show_tooltip {
      self.draw_tooltip(&mut frame, width, height, range);
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

pub fn time_ago_label(days_ago: i64, span_days: i64) -> String {
  if days_ago <= 0 {
    return "today".to_owned();
  }
  if span_days <= 30 {
    format!("{days_ago}d ago")
  } else if span_days <= 90 {
    format!("{}w ago", ((days_ago + 3) / 7).max(1))
  } else {
    format!("{}mo ago", ((days_ago + 14) / 30).max(1))
  }
}

pub fn nearest_index(points: &[ChartPoint], window: (NaiveDate, NaiveDate), fraction: f32) -> Option<usize> {
  let target = fraction.clamp(0.0, 1.0);
  points
    .iter()
    .enumerate()
    .min_by(|(_, a), (_, b)| {
      let da = (date_fraction(window, &a.date) - target).abs();
      let db = (date_fraction(window, &b.date) - target).abs();
      da.total_cmp(&db)
    })
    .map(|(idx, _)| idx)
}

fn date_fraction(window: (NaiveDate, NaiveDate), date: &str) -> f32 {
  let (start, end) = window;
  let span = (end - start).num_days().max(1) as f32;
  let Some(day) = parse_day(date) else {
    return 0.0;
  };
  (((day - start).num_days() as f32) / span).clamp(0.0, 1.0)
}

fn delta_style(delta: f64) -> (&'static str, Color) {
  if delta >= 0.0 {
    ("+", color::status::ONLINE)
  } else {
    ("-", color::status::DANGER)
  }
}

fn parse_day(date: &str) -> Option<NaiveDate> {
  let prefix = date.split('T').next().unwrap_or(date);
  NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok()
}

fn tick_alignment(tick: usize) -> Horizontal {
  if tick == 0 {
    Horizontal::Left
  } else if tick == 4 {
    Horizontal::Right
  } else {
    Horizontal::Center
  }
}

fn tooltip_card_height(has_liquid: bool, has_delta: bool) -> f32 {
  let mut height = 40.0;
  if has_liquid {
    height += 16.0;
  }
  if has_delta {
    height += 16.0;
  }
  height
}

#[cfg(test)]
mod tests {
  use super::*;

  fn window() -> (NaiveDate, NaiveDate) {
    (day("2026-06-01"), day("2026-06-05"))
  }

  fn day(date: &str) -> NaiveDate {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap()
  }

  fn point(value: f64) -> ChartPoint {
    ChartPoint {
      date: "2026-06-01".to_owned(),
      liquid: None,
      value,
    }
  }

  fn dated(date: &str, value: f64) -> ChartPoint {
    ChartPoint {
      date: date.to_owned(),
      liquid: None,
      value,
    }
  }

  fn chart(points: Vec<ChartPoint>, hover: Option<f32>) -> LineChart<'static, Option<f32>> {
    LineChart::new(points, window(), color::status::ONLINE, |v| format!("{v}"), |f| f).hover(hover)
  }

  mod date_fraction {
    use super::*;

    #[test]
    fn it_clamps_dates_outside_the_window() {
      assert_eq!(super::date_fraction(window(), "2026-05-01"), 0.0);
      assert_eq!(super::date_fraction(window(), "2026-07-01"), 1.0);
    }

    #[test]
    fn it_places_a_midpoint_date_partway_across() {
      let frac = super::date_fraction(window(), "2026-06-03");

      assert!((frac - 0.5).abs() < 1e-6);
    }

    #[test]
    fn it_places_the_window_edges_at_zero_and_one() {
      assert_eq!(super::date_fraction(window(), "2026-06-01"), 0.0);
      assert_eq!(super::date_fraction(window(), "2026-06-05"), 1.0);
    }
  }

  mod delta_style {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_marks_gains_green_and_losses_red() {
      assert_eq!(super::delta_style(5.0), ("+", color::status::ONLINE));
      assert_eq!(super::delta_style(-5.0), ("-", color::status::DANGER));
    }
  }

  mod has_liquid {
    use super::*;

    #[test]
    fn it_is_false_when_every_point_has_zero_or_no_liquid() {
      let points = vec![point(100.0), point(200.0)];

      assert!(!chart(points, None).has_liquid());
    }

    #[test]
    fn it_is_true_when_any_point_has_liquid() {
      let points = vec![
        ChartPoint {
          date: "2026-06-01".to_owned(),
          liquid: Some(25.0),
          value: 200.0,
        },
        point(100.0),
      ];

      assert!(chart(points, None).has_liquid());
    }
  }

  mod marker_index {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_the_hovered_index_only_while_hovering() {
      let points = vec![
        dated("2026-06-01", 1.0),
        dated("2026-06-03", 2.0),
        dated("2026-06-05", 3.0),
      ];

      let idle = chart(points.clone(), None);
      assert_eq!(idle.hovered(), None);
      assert_eq!(idle.marker_index(), 2);

      let hovering = chart(points, Some(0.0));
      assert_eq!(hovering.hovered(), Some(0));
      assert_eq!(hovering.marker_index(), 0);
    }
  }

  mod nearest_index {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_out_of_range_fractions() {
      let series = [dated("2026-06-01", 1.0), dated("2026-06-05", 2.0)];

      assert_eq!(super::nearest_index(&series, window(), -1.0), Some(0));
      assert_eq!(super::nearest_index(&series, window(), 2.0), Some(1));
    }

    #[test]
    fn it_is_none_for_an_empty_series() {
      assert_eq!(super::nearest_index(&[], window(), 0.5), None);
    }

    #[test]
    fn it_maps_a_fraction_to_the_point_nearest_by_date() {
      let series = [
        dated("2026-06-01", 1.0),
        dated("2026-06-03", 2.0),
        dated("2026-06-05", 3.0),
      ];

      assert_eq!(super::nearest_index(&series, window(), 0.0), Some(0));
      assert_eq!(super::nearest_index(&series, window(), 0.5), Some(1));
      assert_eq!(super::nearest_index(&series, window(), 1.0), Some(2));
    }

    #[test]
    fn it_snaps_an_empty_left_region_to_the_earliest_data_point() {
      let series = [dated("2026-06-04", 1.0), dated("2026-06-05", 2.0)];

      assert_eq!(super::nearest_index(&series, window(), 0.0), Some(0));
    }
  }

  mod tick_alignment {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_anchors_the_edge_ticks_and_centers_the_rest() {
      assert_eq!(super::tick_alignment(0), Horizontal::Left);
      assert_eq!(super::tick_alignment(4), Horizontal::Right);
      assert_eq!(super::tick_alignment(2), Horizontal::Center);
    }
  }

  mod time_ago_label {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_labels_long_spans_in_months() {
      assert_eq!(super::super::time_ago_label(60, 365), "2mo ago");
    }

    #[test]
    fn it_labels_medium_spans_in_weeks() {
      assert_eq!(super::super::time_ago_label(21, 90), "3w ago");
    }

    #[test]
    fn it_labels_short_spans_in_days() {
      assert_eq!(super::super::time_ago_label(5, 30), "5d ago");
    }

    #[test]
    fn it_labels_the_present_as_today() {
      assert_eq!(super::super::time_ago_label(0, 90), "today");
    }
  }

  mod tooltip_card_height {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_grows_a_row_for_the_liquid_and_delta_lines() {
      assert_eq!(super::super::tooltip_card_height(false, false), 40.0);
      assert_eq!(super::super::tooltip_card_height(false, true), 56.0);
      assert_eq!(super::super::tooltip_card_height(true, false), 56.0);
      assert_eq!(super::super::tooltip_card_height(true, true), 72.0);
    }
  }

  mod update {
    use iced::{Event, Point, Rectangle, widget::canvas::Program as _};
    use pretty_assertions::assert_eq;

    use super::*;

    const BOUNDS: Rectangle = Rectangle {
      x: 0.0,
      y: 0.0,
      width: 200.0,
      height: 220.0,
    };

    fn cursor_at(x: f32) -> mouse::Cursor {
      mouse::Cursor::Available(Point::new(x, 10.0))
    }

    #[test]
    fn it_clears_the_hover_when_the_cursor_leaves() {
      let chart = chart(vec![point(1.0), point(2.0)], Some(0.5));

      let event = Event::Mouse(mouse::Event::CursorLeft);
      let action = chart
        .update(&mut (), &event, BOUNDS, mouse::Cursor::Unavailable)
        .expect("leaving a hovered chart clears the hover");

      assert_eq!(action.into_inner().0, Some(None));
    }

    #[test]
    fn it_does_not_republish_an_unchanged_hover() {
      let chart = chart(vec![point(1.0), point(2.0)], Some(0.5));

      let event = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(100.0, 10.0),
      });

      assert!(chart.update(&mut (), &event, BOUNDS, cursor_at(100.0)).is_none());
    }

    #[test]
    fn it_ignores_a_cursor_leave_when_not_hovering() {
      let chart = chart(vec![point(1.0), point(2.0)], None);

      let event = Event::Mouse(mouse::Event::CursorLeft);

      assert!(
        chart
          .update(&mut (), &event, BOUNDS, mouse::Cursor::Unavailable)
          .is_none()
      );
    }

    #[test]
    fn it_ignores_non_mouse_events() {
      let chart = chart(vec![point(1.0), point(2.0)], None);

      let event = Event::Window(iced::window::Event::Unfocused);

      assert!(chart.update(&mut (), &event, BOUNDS, cursor_at(50.0)).is_none());
    }

    #[test]
    fn it_publishes_the_hover_fraction_on_a_cursor_move() {
      let chart = chart(vec![point(1.0), point(2.0)], None);

      let event = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(100.0, 10.0),
      });
      let action = chart
        .update(&mut (), &event, BOUNDS, cursor_at(100.0))
        .expect("a cursor move over a fresh chart publishes a hover");

      match action.into_inner().0 {
        Some(Some(fraction)) => assert!((fraction - 0.5).abs() < 1e-6),
        other => panic!("expected Some(Some(0.5)), got {other:?}"),
      }
    }
  }

  mod value_range {
    use super::*;

    #[test]
    fn it_pads_a_non_flat_series() {
      let points = vec![point(100.0), point(200.0)];

      let (min, max) = chart(points, None).value_range();

      assert!(min < 100.0);
      assert!(max > 200.0);
    }

    #[test]
    fn it_returns_a_non_degenerate_range_for_a_flat_series() {
      let points = vec![point(50.0), point(50.0)];

      let (min, max) = chart(points, None).value_range();

      assert!(max > min);
    }
  }
}
