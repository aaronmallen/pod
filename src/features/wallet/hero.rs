use chrono::{DateTime, NaiveDate, Utc};
use iced::{
  Background, Border, Color, Element, Length, Padding, Point, Rectangle, Renderer, Theme,
  alignment::{Horizontal, Vertical},
  mouse,
  widget::{Column, Row, Space, canvas, container, text},
};

use super::{Composition, Message, NetWorthPoint, Scope, State, Timeframe, fmt_isk};
use crate::ui::{
  components::{eyebrow::eyebrow_text, segmented::segment_button_style, status},
  style::{color, radius, spacing, typography},
};

const GRAPH_HEIGHT: f32 = 220.0;
const PLOT_PAD_TOP: f32 = 14.0;
const PLOT_PAD_BOTTOM: f32 = 24.0;
const AXIS_LABEL_SIZE: f32 = 9.0;
const BAR_HEIGHT: f32 = 6.0;
const COMPOSITION_CHIP_WIDTH: f32 = 130.0;
const HOVER_DASH: [f32; 2] = [3.0, 3.0];

struct Chart<'a> {
  hover: Option<f32>,
  points: &'a [NetWorthPoint],
  window: (NaiveDate, NaiveDate),
}

impl Chart<'_> {
  fn draw_area(&self, frame: &mut canvas::Frame, width: f32, height: f32, range: (f64, f64), line_color: Color) {
    let area = canvas::Path::new(|builder| {
      builder.move_to(Point::new(self.x_at(0, width), height - PLOT_PAD_BOTTOM));
      for (i, point) in self.points.iter().enumerate() {
        builder.line_to(Point::new(
          self.x_at(i, width),
          self.y_at(point.net_worth, height, range),
        ));
      }
      builder.line_to(Point::new(
        self.x_at(self.points.len() - 1, width),
        height - PLOT_PAD_BOTTOM,
      ));
      builder.close();
    });
    frame.fill(&area, color::with_alpha(line_color, 0.12));
  }

  fn draw_axis_labels(&self, frame: &mut canvas::Frame, width: f32, height: f32) {
    let (start, end) = self.window;
    let span_days = (end - start).num_days();
    for i in 0..=4 {
      let t = i as f32 / 4.0;
      let days_ago = ((1.0 - t) * span_days as f32).round() as i64;
      let label = axis_label(days_ago, span_days);
      let align_x = tick_alignment(i);
      frame.fill_text(canvas::Text {
        content: label,
        position: Point::new(t * width, height - 4.0),
        color: color::text::DIM,
        size: AXIS_LABEL_SIZE.into(),
        font: typography::mono::REGULAR,
        align_x: align_x.into(),
        align_y: Vertical::Bottom,
        ..canvas::Text::default()
      });
    }
  }

  fn draw_gridlines(&self, frame: &mut canvas::Frame, width: f32, height: f32, range: (f64, f64)) {
    let (min, max) = range;
    let plot_h = height - PLOT_PAD_TOP - PLOT_PAD_BOTTOM;
    let grid_stroke = canvas::Stroke::default()
      .with_width(1.0)
      .with_color(color::with_alpha(color::text::PRIMARY, 0.06));
    for i in 0..=4 {
      let t = i as f32 / 4.0;
      let y = PLOT_PAD_TOP + t * plot_h;
      frame.stroke(
        &canvas::Path::line(Point::new(0.0, y), Point::new(width, y)),
        grid_stroke,
      );
      let grid_value = max - (max - min) * t as f64;
      frame.fill_text(canvas::Text {
        content: fmt_isk(Some(grid_value)),
        position: Point::new(width - 4.0, y - 3.0),
        color: color::text::TERTIARY,
        size: AXIS_LABEL_SIZE.into(),
        font: typography::mono::REGULAR,
        align_x: Horizontal::Right.into(),
        align_y: Vertical::Bottom,
        ..canvas::Text::default()
      });
    }
  }

  fn draw_line(&self, frame: &mut canvas::Frame, width: f32, height: f32, range: (f64, f64), line_color: Color) {
    let line = canvas::Path::new(|builder| {
      for (i, point) in self.points.iter().enumerate() {
        let p = Point::new(self.x_at(i, width), self.y_at(point.net_worth, height, range));
        if i == 0 {
          builder.move_to(p);
        } else {
          builder.line_to(p);
        }
      }
    });
    frame.stroke(&line, canvas::Stroke::default().with_width(2.0).with_color(line_color));
  }

  fn draw_liquid(&self, frame: &mut canvas::Frame, width: f32, height: f32, range: (f64, f64)) {
    if !self.has_liquid() {
      return;
    }
    let baseline = height - PLOT_PAD_BOTTOM;

    let fill = canvas::Path::new(|builder| {
      builder.move_to(Point::new(self.x_at(0, width), baseline));
      for (i, point) in self.points.iter().enumerate() {
        builder.line_to(Point::new(self.x_at(i, width), self.y_at(point.liquid, height, range)));
      }
      builder.line_to(Point::new(self.x_at(self.points.len() - 1, width), baseline));
      builder.close();
    });
    frame.fill(
      &fill,
      canvas::gradient::Linear::new(Point::new(0.0, PLOT_PAD_TOP), Point::new(0.0, baseline))
        .add_stop(0.0, color::with_alpha(color::accent::PLASMA, 0.2))
        .add_stop(1.0, color::with_alpha(color::accent::PLASMA, 0.0)),
    );

    let line = canvas::Path::new(|builder| {
      for (i, point) in self.points.iter().enumerate() {
        let p = Point::new(self.x_at(i, width), self.y_at(point.liquid, height, range));
        if i == 0 {
          builder.move_to(p);
        } else {
          builder.line_to(p);
        }
      }
    });
    frame.stroke(
      &line,
      canvas::Stroke::default()
        .with_width(1.75)
        .with_color(color::with_alpha(color::accent::PLASMA, 0.92)),
    );

    let last = self.points.len() - 1;
    frame.fill(
      &canvas::Path::circle(
        Point::new(
          self.x_at(last, width),
          self.y_at(self.points[last].liquid, height, range),
        ),
        3.5,
      ),
      color::accent::PLASMA,
    );
  }

  fn draw_marker(&self, frame: &mut canvas::Frame, width: f32, height: f32, range: (f64, f64), line_color: Color) {
    let idx = self.marker_index();
    let x = self.x_at(idx, width);
    let y = self.y_at(self.points[idx].net_worth, height, range);

    if self.hovered().is_some() {
      frame.stroke(
        &canvas::Path::line(Point::new(x, PLOT_PAD_TOP), Point::new(x, height - PLOT_PAD_BOTTOM)),
        canvas::Stroke {
          line_dash: canvas::LineDash {
            segments: &HOVER_DASH,
            offset: 0,
          },
          ..canvas::Stroke::default()
            .with_width(1.0)
            .with_color(color::with_alpha(line_color, 0.6))
        },
      );
      frame.fill(&canvas::Path::circle(Point::new(x, y), 5.0), color::surface::BASE);
      frame.stroke(
        &canvas::Path::circle(Point::new(x, y), 5.0),
        canvas::Stroke::default().with_width(2.0).with_color(line_color),
      );
    } else {
      frame.fill(&canvas::Path::circle(Point::new(x, y), 4.0), line_color);
    }
  }

  fn draw_tooltip(&self, frame: &mut canvas::Frame, width: f32, height: f32, range: (f64, f64)) {
    let Some(idx) = self.hovered() else {
      return;
    };
    let point = &self.points[idx];
    let x = self.x_at(idx, width);
    let y = self.y_at(point.net_worth, height, range);

    let date = point.date.split('T').next().unwrap_or(&point.date).to_owned();
    let value = format!("{} ISK", fmt_isk(Some(point.net_worth)));

    let delta = (idx > 0).then(|| point.net_worth - self.points[idx - 1].net_worth);
    let liquid = self.has_liquid().then_some(point.liquid);

    let card_w = 150.0_f32;
    let card_h = tooltip_card_height(liquid.is_some(), delta.is_some());
    let card_x = (x + 12.0).min(width - card_w).max(0.0);
    let card_y = (y - card_h - 12.0).max(PLOT_PAD_TOP);

    let card = canvas::Path::rounded_rectangle(Point::new(card_x, card_y), iced::Size::new(card_w, card_h), 6.0.into());
    frame.fill(&card, color::surface::RAISED);
    frame.stroke(
      &card,
      canvas::Stroke::default()
        .with_width(1.0)
        .with_color(color::with_alpha(color::text::PRIMARY, 0.18)),
    );

    frame.fill_text(canvas::Text {
      content: date.to_uppercase(),
      position: Point::new(card_x + 10.0, card_y + 8.0),
      color: color::text::SECONDARY,
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
        color::accent::PLASMA,
      );
      frame.fill_text(canvas::Text {
        content: "Liquid".to_owned(),
        position: Point::new(card_x + 22.0, row_y),
        color: color::text::SECONDARY,
        size: 9.0.into(),
        font: typography::mono::REGULAR,
        ..canvas::Text::default()
      });
      frame.fill_text(canvas::Text {
        content: fmt_isk(Some(liquid)),
        position: Point::new(card_x + card_w - 10.0, row_y),
        color: color::accent::PLASMA,
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
        content: format!("{sign}{} ISK", fmt_isk(Some(delta.abs()))),
        position: Point::new(card_x + 10.0, row_y),
        color: delta_color,
        size: 10.0.into(),
        font: typography::mono::REGULAR,
        ..canvas::Text::default()
      });
    }
  }

  fn has_liquid(&self) -> bool {
    self.points.iter().any(|point| point.liquid != 0.0)
  }

  fn hovered(&self) -> Option<usize> {
    self
      .hover
      .and_then(|fraction| nearest_index(self.points, self.window, fraction))
  }

  fn is_up(&self) -> bool {
    super::series_change(self.points) >= 0.0
  }

  fn marker_index(&self) -> usize {
    self.hovered().unwrap_or_else(|| self.points.len() - 1)
  }

  fn value_range(&self) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for point in self.points {
      min = min.min(point.net_worth);
      max = max.max(point.net_worth);
    }
    if !min.is_finite() || !max.is_finite() {
      return (0.0, 1.0);
    }
    if (max - min).abs() < f64::EPSILON {
      return (min - 1.0, max + 1.0);
    }
    let pad = (max - min) * 0.08;
    (min - pad, max + pad)
  }

  fn x_at(&self, i: usize, width: f32) -> f32 {
    date_fraction(self.window, &self.points[i].date) * width
  }

  fn y_at(&self, value: f64, height: f32, range: (f64, f64)) -> f32 {
    let (min, max) = range;
    let plot_h = height - PLOT_PAD_TOP - PLOT_PAD_BOTTOM;
    let t = ((value - min) / (max - min)) as f32;
    PLOT_PAD_TOP + plot_h - t.clamp(0.0, 1.0) * plot_h
  }
}

impl canvas::Program<Message> for Chart<'_> {
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
          return Some(canvas::Action::publish(Message::ChartHovered(fraction)));
        }
        None
      }
      mouse::Event::CursorLeft if self.hover.is_some() => Some(canvas::Action::publish(Message::ChartHovered(None))),
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
    let width = bounds.width;
    let height = bounds.height;
    let range = self.value_range();

    let line_color = if self.is_up() {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };

    self.draw_gridlines(&mut frame, width, height, range);
    self.draw_area(&mut frame, width, height, range, line_color);
    self.draw_liquid(&mut frame, width, height, range);
    self.draw_line(&mut frame, width, height, range, line_color);
    self.draw_marker(&mut frame, width, height, range, line_color);
    self.draw_axis_labels(&mut frame, width, height);
    self.draw_tooltip(&mut frame, width, height, range);

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

fn hovered_value(state: &State, sliced: &[NetWorthPoint], window: (NaiveDate, NaiveDate)) -> Option<f64> {
  let fraction = state.chart_hover?;
  nearest_index(sliced, window, fraction).map(|idx| sliced[idx].net_worth)
}

fn axis_label(days_ago: i64, span_days: i64) -> String {
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

fn date_fraction(window: (NaiveDate, NaiveDate), date: &str) -> f32 {
  let (start, end) = window;
  let span = (end - start).num_days().max(1) as f32;
  let Some(day) = parse_day(date) else {
    return 0.0;
  };
  (((day - start).num_days() as f32) / span).clamp(0.0, 1.0)
}

fn nearest_index(sliced: &[NetWorthPoint], window: (NaiveDate, NaiveDate), fraction: f32) -> Option<usize> {
  let target = fraction.clamp(0.0, 1.0);
  sliced
    .iter()
    .enumerate()
    .min_by(|(_, a), (_, b)| {
      let da = (date_fraction(window, &a.date) - target).abs();
      let db = (date_fraction(window, &b.date) - target).abs();
      da.total_cmp(&db)
    })
    .map(|(idx, _)| idx)
}

fn parse_day(date: &str) -> Option<NaiveDate> {
  let prefix = date.split('T').next().unwrap_or(date);
  NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok()
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

  canvas(Chart {
    hover: state.chart_hover,
    points: sliced,
    window,
  })
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
          background: Some(Background::Color(slice_color(index))),
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
            background: Some(Background::Color(slice_color(index))),
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

fn slice_color(index: usize) -> Color {
  let hues = [
    color::accent::PLASMA,
    color::status::ONLINE,
    color::status::DANGER,
    color::chart::GOLD,
    color::chart::VIOLET,
  ];
  hues[index % hues.len()]
}

fn delta_style(delta: f64) -> (&'static str, Color) {
  if delta >= 0.0 {
    ("+", color::status::ONLINE)
  } else {
    ("-", color::status::DANGER)
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

fn tick_alignment(tick: usize) -> Horizontal {
  if tick == 0 {
    Horizontal::Left
  } else if tick == 4 {
    Horizontal::Right
  } else {
    Horizontal::Center
  }
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

  fn point(net_worth: f64) -> NetWorthPoint {
    NetWorthPoint {
      date: "2026-06-01".to_owned(),
      liquid: 0.0,
      net_worth,
    }
  }

  fn liquid_point(net_worth: f64, liquid: f64) -> NetWorthPoint {
    NetWorthPoint {
      date: "2026-06-01".to_owned(),
      liquid,
      net_worth,
    }
  }

  fn dated(date: &str, net_worth: f64) -> NetWorthPoint {
    NetWorthPoint {
      date: date.to_owned(),
      liquid: 0.0,
      net_worth,
    }
  }

  mod axis_label {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_labels_the_present_as_today() {
      assert_eq!(super::axis_label(0, 90), "today");
    }

    #[test]
    fn it_labels_short_spans_in_days() {
      assert_eq!(super::axis_label(5, 30), "5d ago");
    }

    #[test]
    fn it_labels_medium_spans_in_weeks() {
      assert_eq!(super::axis_label(21, 90), "3w ago");
    }

    #[test]
    fn it_labels_long_spans_in_months() {
      assert_eq!(super::axis_label(60, 365), "2mo ago");
    }
  }

  mod date_fraction {
    use super::*;

    #[test]
    fn it_places_the_window_edges_at_zero_and_one() {
      assert_eq!(super::date_fraction(window(), "2026-06-01"), 0.0);
      assert_eq!(super::date_fraction(window(), "2026-06-05"), 1.0);
    }

    #[test]
    fn it_places_a_midpoint_date_partway_across() {
      let frac = super::date_fraction(window(), "2026-06-03");

      assert!((frac - 0.5).abs() < 1e-6);
    }

    #[test]
    fn it_clamps_dates_outside_the_window() {
      assert_eq!(super::date_fraction(window(), "2026-05-01"), 0.0);
      assert_eq!(super::date_fraction(window(), "2026-07-01"), 1.0);
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

  mod delta_style {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_marks_gains_green_and_losses_red() {
      assert_eq!(super::delta_style(5.0), ("+", color::status::ONLINE));
      assert_eq!(super::delta_style(-5.0), ("-", color::status::DANGER));
    }
  }

  mod tooltip_card_height {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_grows_a_row_for_the_liquid_and_delta_lines() {
      assert_eq!(super::tooltip_card_height(false, false), 40.0);
      assert_eq!(super::tooltip_card_height(false, true), 56.0);
      assert_eq!(super::tooltip_card_height(true, false), 56.0);
      assert_eq!(super::tooltip_card_height(true, true), 72.0);
    }
  }

  mod has_liquid {
    use super::*;

    #[test]
    fn it_is_false_when_every_point_has_zero_liquid() {
      let points = [point(100.0), point(200.0)];
      let chart = Chart {
        hover: None,
        points: &points,
        window: window(),
      };

      assert!(!chart.has_liquid());
    }

    #[test]
    fn it_is_true_when_any_point_has_liquid() {
      let points = [liquid_point(100.0, 0.0), liquid_point(200.0, 25.0)];
      let chart = Chart {
        hover: None,
        points: &points,
        window: window(),
      };

      assert!(chart.has_liquid());
    }
  }

  mod nearest_index {
    use pretty_assertions::assert_eq;

    use super::*;

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

    #[test]
    fn it_is_none_for_an_empty_series() {
      assert_eq!(super::nearest_index(&[], window(), 0.5), None);
    }

    #[test]
    fn it_clamps_out_of_range_fractions() {
      let series = [dated("2026-06-01", 1.0), dated("2026-06-05", 2.0)];

      assert_eq!(super::nearest_index(&series, window(), -1.0), Some(0));
      assert_eq!(super::nearest_index(&series, window(), 2.0), Some(1));
    }
  }

  mod value_range {
    use super::*;

    #[test]
    fn it_pads_a_non_flat_series() {
      let points = [point(100.0), point(200.0)];
      let chart = Chart {
        hover: None,
        points: &points,
        window: window(),
      };

      let (min, max) = chart.value_range();

      assert!(min < 100.0);
      assert!(max > 200.0);
    }

    #[test]
    fn it_returns_a_non_degenerate_range_for_a_flat_series() {
      let points = [point(50.0), point(50.0)];
      let chart = Chart {
        hover: None,
        points: &points,
        window: window(),
      };

      let (min, max) = chart.value_range();

      assert!(max > min);
    }
  }

  mod marker {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_up_for_a_rising_or_flat_series() {
      let rising = [point(1.0), point(2.0)];
      assert!(
        Chart {
          hover: None,
          points: &rising,
          window: window(),
        }
        .is_up()
      );
    }

    #[test]
    fn it_is_not_up_for_a_falling_series() {
      let falling = [point(2.0), point(1.0)];
      assert!(
        !Chart {
          hover: None,
          points: &falling,
          window: window(),
        }
        .is_up()
      );
    }

    #[test]
    fn it_resolves_the_hovered_index_only_while_hovering() {
      let points = [
        dated("2026-06-01", 1.0),
        dated("2026-06-03", 2.0),
        dated("2026-06-05", 3.0),
      ];

      let idle = Chart {
        hover: None,
        points: &points,
        window: window(),
      };
      assert_eq!(idle.hovered(), None);
      assert_eq!(idle.marker_index(), 2);

      let hovering = Chart {
        hover: Some(0.0),
        points: &points,
        window: window(),
      };
      assert_eq!(hovering.hovered(), Some(0));
      assert_eq!(hovering.marker_index(), 0);
    }
  }

  mod chart_update {
    use iced::{Event, Point, Rectangle, widget::canvas::Program as _};

    use super::*;

    const BOUNDS: Rectangle = Rectangle {
      x: 0.0,
      y: 0.0,
      width: 200.0,
      height: GRAPH_HEIGHT,
    };

    fn cursor_at(x: f32) -> mouse::Cursor {
      mouse::Cursor::Available(Point::new(x, 10.0))
    }

    #[test]
    fn it_publishes_the_hover_fraction_on_a_cursor_move() {
      let points = [point(1.0), point(2.0)];
      let chart = Chart {
        hover: None,
        points: &points,
        window: window(),
      };

      let event = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(100.0, 10.0),
      });
      let action = chart
        .update(&mut (), &event, BOUNDS, cursor_at(100.0))
        .expect("a cursor move over a fresh chart publishes a hover");

      match action.into_inner().0 {
        Some(Message::ChartHovered(Some(fraction))) => assert!((fraction - 0.5).abs() < 1e-6),
        other => panic!("expected ChartHovered(Some(0.5)), got {other:?}"),
      }
    }

    #[test]
    fn it_does_not_republish_an_unchanged_hover() {
      let points = [point(1.0), point(2.0)];
      let chart = Chart {
        hover: Some(0.5),
        points: &points,
        window: window(),
      };

      let event = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(100.0, 10.0),
      });

      assert!(chart.update(&mut (), &event, BOUNDS, cursor_at(100.0)).is_none());
    }

    #[test]
    fn it_clears_the_hover_when_the_cursor_leaves() {
      let points = [point(1.0), point(2.0)];
      let chart = Chart {
        hover: Some(0.5),
        points: &points,
        window: window(),
      };

      let event = Event::Mouse(mouse::Event::CursorLeft);
      let action = chart
        .update(&mut (), &event, BOUNDS, mouse::Cursor::Unavailable)
        .expect("leaving a hovered chart clears the hover");

      match action.into_inner().0 {
        Some(Message::ChartHovered(None)) => {}
        other => panic!("expected ChartHovered(None), got {other:?}"),
      }
    }

    #[test]
    fn it_ignores_a_cursor_leave_when_not_hovering() {
      let points = [point(1.0), point(2.0)];
      let chart = Chart {
        hover: None,
        points: &points,
        window: window(),
      };

      let event = Event::Mouse(mouse::Event::CursorLeft);

      assert!(
        chart
          .update(&mut (), &event, BOUNDS, mouse::Cursor::Unavailable)
          .is_none()
      );
    }

    #[test]
    fn it_ignores_non_mouse_events() {
      let points = [point(1.0), point(2.0)];
      let chart = Chart {
        hover: None,
        points: &points,
        window: window(),
      };

      let event = Event::Window(iced::window::Event::Unfocused);

      assert!(chart.update(&mut (), &event, BOUNDS, cursor_at(50.0)).is_none());
    }
  }
}
