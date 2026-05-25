//! Interactive line chart with optional hover crosshair and tooltip data.

use iced::{
  Color, Element, Length, Pixels, Point, alignment, mouse,
  widget::canvas::{self, Action, Canvas, Frame, Geometry, LineDash, Path, Stroke, Text, fill, gradient},
};

use crate::style::{color, typography};

/// Data emitted while the pointer hovers over the chart.
#[derive(Clone, Debug)]
pub struct HoverData {
  /// Data index under the cursor.
  pub index: usize,
  /// Horizontal position as a fraction of total canvas width (0.0–1.0).
  pub x_frac: f32,
  /// Series value at the hovered index.
  pub value: f64,
}

/// Builder for an interactive line chart component.
pub struct Component<M> {
  data: Vec<f64>,
  line_color: Color,
  on_hover: Option<Box<dyn Fn(Option<HoverData>) -> M + 'static>>,
  pad_bottom: f32,
  pad_left: f32,
  pad_right: f32,
  pad_top: f32,
  x_labels: Vec<String>,
  y_formatter: fn(f64) -> String,
}

impl<M: Clone + 'static> Component<M> {
  /// Creates a new line chart with the given data series and line color.
  pub fn new(data: Vec<f64>, line_color: Color) -> Self {
    Self {
      data,
      line_color,
      on_hover: None,
      pad_bottom: 24.0,
      pad_left: 0.0,
      pad_right: 0.0,
      pad_top: 18.0,
      x_labels: Vec::new(),
      y_formatter: default_fmt,
    }
  }

  /// Attaches x-axis labels and a y-axis value formatter.
  pub fn with_labels(mut self, x_labels: Vec<String>, y_formatter: fn(f64) -> String) -> Self {
    self.x_labels = x_labels;
    self.y_formatter = y_formatter;
    self
  }

  /// Registers a callback that fires when the hover position changes.
  pub fn with_hover<F>(mut self, f: F) -> Self
  where
    F: Fn(Option<HoverData>) -> M + 'static,
  {
    self.on_hover = Some(Box::new(f));
    self
  }

  /// Sets per-edge padding in pixels.
  pub fn with_padding(mut self, left: f32, right: f32, top: f32, bottom: f32) -> Self {
    self.pad_bottom = bottom;
    self.pad_left = left;
    self.pad_right = right;
    self.pad_top = top;
    self
  }

  /// Consumes the builder and returns a sized canvas element.
  pub fn render(self, width: Length, height: f32) -> Element<'static, M> {
    Canvas::new(self).width(width).height(height).into()
  }
}

const CROSSHAIR_DASH: &[f32] = &[3.0, 3.0];

fn default_fmt(v: f64) -> String {
  crate::format::fmt_isk(v)
}

impl<M: Clone + 'static> canvas::Program<M> for Component<M> {
  type State = Option<HoverData>;

  fn update(
    &self,
    state: &mut Self::State,
    event: &canvas::Event,
    bounds: iced::Rectangle,
    cursor: mouse::Cursor,
  ) -> Option<Action<M>> {
    if self.data.len() < 2 {
      return None;
    }
    let n = self.data.len();
    let chart_w = bounds.width - self.pad_left - self.pad_right;

    match event {
      canvas::Event::Mouse(mouse::Event::CursorMoved {
        ..
      }) => {
        if let Some(pos) = cursor.position_in(bounds) {
          let raw_x = pos.x - self.pad_left;
          let frac = (raw_x / chart_w).clamp(0.0, 1.0);
          let raw_idx = (frac * (n - 1) as f32).round() as usize;
          let index = raw_idx.clamp(0, n - 1);
          let x_frac = pos.x / bounds.width;
          let value = self.data[index];
          let hover = HoverData {
            index,
            value,
            x_frac,
          };
          *state = Some(hover.clone());
          if let Some(cb) = &self.on_hover {
            return Some(Action::publish(cb(Some(hover))));
          }
          return Some(Action::request_redraw());
        }
        None
      }
      canvas::Event::Mouse(mouse::Event::CursorLeft) => {
        *state = None;
        if let Some(cb) = &self.on_hover {
          return Some(Action::publish(cb(None)));
        }
        Some(Action::request_redraw())
      }
      _ => None,
    }
  }

  fn draw(
    &self,
    state: &Self::State,
    renderer: &iced::Renderer,
    _theme: &iced::Theme,
    bounds: iced::Rectangle,
    _cursor: mouse::Cursor,
  ) -> Vec<Geometry<iced::Renderer>> {
    if self.data.len() < 2 {
      return vec![];
    }
    let mut frame = Frame::new(renderer, bounds.size());
    let w = frame.width();
    let h = frame.height();
    let chart_w = w - self.pad_left - self.pad_right;
    let chart_h = h - self.pad_top - self.pad_bottom;
    let min_v = self.data.iter().cloned().fold(f64::INFINITY, f64::min) * 0.985;
    let max_v = self.data.iter().cloned().fold(f64::NEG_INFINITY, f64::max) * 1.015;
    let range = max_v - min_v;
    let x_at = |i: usize| -> f32 { self.pad_left + (i as f32 / (self.data.len() - 1) as f32) * chart_w };
    let y_at = |v: f64| -> f32 { self.pad_top + chart_h - ((v - min_v) / range) as f32 * chart_h };
    let bottom_y = self.pad_top + chart_h;
    self.draw_grid(&mut frame, w, chart_h, min_v, max_v, &y_at);
    self.draw_fill_and_line(&mut frame, bottom_y, &x_at, &y_at);
    if state.is_none() {
      self.draw_end_dot(&mut frame, &x_at, &y_at);
    }
    self.draw_x_labels(&mut frame, h, &x_at);
    if let Some(hover) = state {
      self.draw_crosshair(&mut frame, hover, chart_h, bottom_y, &x_at, &y_at);
    }
    vec![frame.into_geometry()]
  }
}

impl<M: Clone + 'static> Component<M> {
  fn build_fill_path(&self, bottom_y: f32, x_at: &impl Fn(usize) -> f32, y_at: &impl Fn(f64) -> f32) -> Path {
    Path::new(|p| {
      for (i, &v) in self.data.iter().enumerate() {
        let pt = Point::new(x_at(i), y_at(v));
        if i == 0 {
          p.move_to(pt);
        } else {
          p.line_to(pt);
        }
      }
      p.line_to(Point::new(x_at(self.data.len() - 1), bottom_y));
      p.line_to(Point::new(x_at(0), bottom_y));
      p.close();
    })
  }

  fn build_line_path(&self, x_at: &impl Fn(usize) -> f32, y_at: &impl Fn(f64) -> f32) -> Path {
    Path::new(|p| {
      for (i, &v) in self.data.iter().enumerate() {
        let pt = Point::new(x_at(i), y_at(v));
        if i == 0 {
          p.move_to(pt);
        } else {
          p.line_to(pt);
        }
      }
    })
  }

  fn draw_crosshair(
    &self,
    frame: &mut Frame,
    hover: &HoverData,
    chart_h: f32,
    bottom_y: f32,
    x_at: &impl Fn(usize) -> f32,
    y_at: &impl Fn(f64) -> f32,
  ) {
    let cx = x_at(hover.index);
    let cy = y_at(hover.value);
    let dash_color = color::with_alpha(self.line_color, 0.6);
    let vline = Path::new(|p| {
      p.move_to(Point::new(cx, self.pad_top));
      p.line_to(Point::new(cx, bottom_y));
    });
    frame.stroke(
      &vline,
      Stroke {
        style: canvas::Style::Solid(dash_color),
        width: 1.0,
        line_dash: LineDash {
          segments: CROSSHAIR_DASH,
          offset: 0,
        },
        ..Stroke::default()
      },
    );
    let open_circle_outer = Path::circle(Point::new(cx, cy), 5.0);
    frame.fill(&open_circle_outer, color::surface::BASE);
    let open_circle_ring = Path::circle(Point::new(cx, cy), 5.0);
    frame.stroke(
      &open_circle_ring,
      Stroke::default().with_color(self.line_color).with_width(2.0),
    );
    let _ = chart_h;
  }

  fn draw_end_dot(&self, frame: &mut Frame, x_at: &impl Fn(usize) -> f32, y_at: &impl Fn(f64) -> f32) {
    let last_idx = self.data.len() - 1;
    let end_dot = Path::circle(Point::new(x_at(last_idx), y_at(self.data[last_idx])), 3.5);
    frame.fill(&end_dot, self.line_color);
  }

  fn draw_fill_and_line(
    &self,
    frame: &mut Frame,
    bottom_y: f32,
    x_at: &impl Fn(usize) -> f32,
    y_at: &impl Fn(f64) -> f32,
  ) {
    let line_path = self.build_line_path(x_at, y_at);
    let fill_path = self.build_fill_path(bottom_y, x_at, y_at);
    let c = self.line_color;
    let fill_gradient = gradient::Linear::new(Point::new(0.0, self.pad_top), Point::new(0.0, bottom_y))
      .add_stop(0.0, color::with_alpha(c, 0.22))
      .add_stop(1.0, color::with_alpha(c, 0.0));
    frame.fill(
      &fill_path,
      fill::Fill {
        style: canvas::Style::Gradient(fill_gradient.into()),
        rule: fill::Rule::NonZero,
      },
    );
    frame.stroke(
      &line_path,
      Stroke::default()
        .with_color(self.line_color)
        .with_width(1.5)
        .with_line_cap(canvas::LineCap::Round)
        .with_line_join(canvas::LineJoin::Round),
    );
  }

  fn draw_grid(&self, frame: &mut Frame, w: f32, chart_h: f32, min_v: f64, max_v: f64, y_at: &impl Fn(f64) -> f32) {
    let grid_color = color::state::SUBTLE_FILL;
    for i in 0..=4 {
      let t = i as f32 / 4.0;
      let gy = self.pad_top + t * chart_h;
      let line = Path::new(|p| {
        p.move_to(Point::new(self.pad_left, gy));
        p.line_to(Point::new(w - self.pad_right, gy));
      });
      frame.stroke(&line, Stroke::default().with_color(grid_color).with_width(1.0));
      if !self.x_labels.is_empty() {
        let grid_v = max_v - (max_v - min_v) * t as f64;
        let label = (self.y_formatter)(grid_v);
        frame.fill_text(Text {
          content: label,
          position: Point::new(w - self.pad_right - 4.0, y_at(grid_v) - 3.0),
          color: color::text::TERTIARY,
          size: Pixels(9.0),
          font: typography::mono::REGULAR,
          align_x: iced::alignment::Horizontal::Right.into(),
          align_y: alignment::Vertical::Bottom,
          ..Text::default()
        });
      }
    }
  }

  fn draw_x_labels(&self, frame: &mut Frame, h: f32, x_at: &impl Fn(usize) -> f32) {
    if self.x_labels.is_empty() {
      return;
    }
    let tick_count = self.x_labels.len();
    for (i, label) in self.x_labels.iter().enumerate() {
      let idx = (i as f32 / (tick_count - 1) as f32 * (self.data.len() - 1) as f32).round() as usize;
      let idx = idx.min(self.data.len() - 1);
      let align_x = if i == 0 {
        iced::alignment::Horizontal::Left
      } else if i == tick_count - 1 {
        iced::alignment::Horizontal::Right
      } else {
        iced::alignment::Horizontal::Center
      };
      frame.fill_text(Text {
        content: label.clone(),
        position: Point::new(x_at(idx), h - 4.0),
        color: color::text::DIM,
        size: Pixels(9.0),
        font: typography::mono::REGULAR,
        align_x: align_x.into(),
        align_y: alignment::Vertical::Bottom,
        ..Text::default()
      });
    }
  }
}
