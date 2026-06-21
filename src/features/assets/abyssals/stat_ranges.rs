use iced::{
  Background, Border, Element, Length, Padding, Point, Rectangle, Size, mouse,
  widget::{Canvas, Column, Row, Space, button, canvas, container, text, text_input},
};

use super::format_stat_value;
use crate::{
  features::assets::{Message, SliderEndpoint, State},
  store::model::StatTemplate,
  ui::style::{color, spacing, typography},
};

const EPSILON: f64 = 1e-9;
const RAIL_HEIGHT: f32 = 3.0;
const THUMB_RADIUS: f32 = 7.0;
const TRACK_HEIGHT: f32 = 22.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum DragTarget {
  Max,
  Min,
}

struct RangeSliderProgram {
  attribute_id: i64,
  current_max: f64,
  current_min: f64,
  hi: f64,
  lo: f64,
}

impl RangeSliderProgram {
  fn nearest_target(&self, canvas_x: f32, inner_w: f32) -> DragTarget {
    let x_min = THUMB_RADIUS + fraction_for_value(self.current_min, self.lo, self.hi) * inner_w;
    let x_max = THUMB_RADIUS + fraction_for_value(self.current_max, self.lo, self.hi) * inner_w;
    if (canvas_x - x_min).abs() <= (canvas_x - x_max).abs() {
      DragTarget::Min
    } else {
      DragTarget::Max
    }
  }

  fn segment_color(&self) -> iced::Color {
    let is_filtered = (self.current_min - self.lo).abs() > EPSILON || (self.current_max - self.hi).abs() > EPSILON;
    if is_filtered {
      color::accent::PLASMA
    } else {
      color::with_alpha(color::text::PRIMARY, 0.22)
    }
  }
}

impl canvas::Program<Message> for RangeSliderProgram {
  type State = RangeSliderState;

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
    let inner_w = (bounds.width - THUMB_RADIUS * 2.0).max(1.0);

    match mouse_event {
      mouse::Event::ButtonPressed(mouse::Button::Left) => {
        let padded = Rectangle {
          x: bounds.x - THUMB_RADIUS,
          y: bounds.y,
          width: bounds.width + THUMB_RADIUS * 2.0,
          height: bounds.height,
        };
        let pos = cursor.position_in(padded)?;
        let canvas_x = pos.x - THUMB_RADIUS;
        state.dragging = Some(self.nearest_target(canvas_x, inner_w));
        Some(canvas::Action::request_redraw())
      }
      mouse::Event::ButtonReleased(mouse::Button::Left) => {
        if state.dragging.is_some() {
          state.dragging = None;
          return Some(canvas::Action::request_redraw());
        }
        None
      }
      mouse::Event::CursorMoved {
        ..
      } => {
        let target = state.dragging?;
        let raw = cursor.position().unwrap_or_default();
        let fraction = (raw.x - bounds.x - THUMB_RADIUS) / inner_w;
        let new_val = value_at_fraction(fraction, self.lo, self.hi);
        let message = match target {
          DragTarget::Min => {
            Message::AbyssalStatMinChanged(self.attribute_id, new_val.clamp(self.lo, self.current_max))
          }
          DragTarget::Max => {
            Message::AbyssalStatMaxChanged(self.attribute_id, new_val.clamp(self.current_min, self.hi))
          }
        };
        Some(canvas::Action::publish(message))
      }
      _ => None,
    }
  }

  fn draw(
    &self,
    state: &Self::State,
    renderer: &iced::Renderer,
    _theme: &iced::Theme,
    bounds: Rectangle,
    _cursor: mouse::Cursor,
  ) -> Vec<canvas::Geometry<iced::Renderer>> {
    let mut frame = canvas::Frame::new(renderer, bounds.size());
    let w = frame.width();
    let cy = frame.height() / 2.0;
    let inner_w = (w - THUMB_RADIUS * 2.0).max(0.0);
    let x_for = |value: f64| THUMB_RADIUS + fraction_for_value(value, self.lo, self.hi) * inner_w;
    let x_min = x_for(self.current_min);
    let x_max = x_for(self.current_max);

    let bg_rail = canvas::Path::rectangle(
      Point::new(THUMB_RADIUS, cy - RAIL_HEIGHT / 2.0),
      Size::new((w - THUMB_RADIUS * 2.0).max(0.0), RAIL_HEIGHT),
    );
    frame.fill(&bg_rail, color::with_alpha(color::text::PRIMARY, 0.12));

    let segment_col = self.segment_color();
    let active_w = (x_max - x_min).max(0.0);
    if active_w > 0.0 {
      let active_rail = canvas::Path::rectangle(
        Point::new(x_min, cy - RAIL_HEIGHT / 2.0),
        Size::new(active_w, RAIL_HEIGHT),
      );
      frame.fill(&active_rail, segment_col);
    }

    for (x, dragging) in [
      (x_min, state.dragging == Some(DragTarget::Min)),
      (x_max, state.dragging == Some(DragTarget::Max)),
    ] {
      let (radius, border_col) = thumb_style(dragging);
      if dragging {
        let glow = canvas::Path::circle(Point::new(x, cy), radius + 4.0);
        frame.fill(&glow, color::with_alpha(color::accent::PLASMA, 0.22));
      }
      let thumb = canvas::Path::circle(Point::new(x, cy), radius);
      frame.fill(&thumb, color::surface::BASE);
      frame.stroke(&thumb, canvas::Stroke::default().with_color(border_col).with_width(2.0));
    }

    vec![frame.into_geometry()]
  }

  fn mouse_interaction(&self, state: &Self::State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
    if state.dragging.is_some() || cursor.position_in(bounds).is_some() {
      mouse::Interaction::ResizingHorizontally
    } else {
      mouse::Interaction::default()
    }
  }
}

#[derive(Clone, Default)]
struct RangeSliderState {
  dragging: Option<DragTarget>,
}

pub(super) fn panel(state: &State) -> Element<'_, Message> {
  if state.abyssal_filters().source_type_id.is_none() || state.abyssal_stat_templates().is_empty() {
    return placeholder();
  }

  let editing = state.abyssal_slider_edit();
  let edit_text = state.abyssal_slider_edit_text();
  let filters = &state.abyssal_filters().stat_ranges;

  let mut rows: Vec<Element<'_, Message>> = Vec::new();
  for template in state.abyssal_stat_templates() {
    if template.bound_hi - template.bound_lo < EPSILON {
      continue;
    }
    let filter = filters.get(&template.attribute_id).copied();
    rows.push(stat_slider_row(template, filter, editing, edit_text));
  }

  if rows.is_empty() {
    return placeholder();
  }

  Column::with_children(rows)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

pub(super) fn placeholder() -> Element<'static, Message> {
  container(
    text("Pick a module type to filter by its rolled stats.")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3_5,
  })
  .width(Length::Fill)
  .into()
}

fn stat_slider_row<'a>(
  template: &'a StatTemplate,
  filter: Option<(f64, f64)>,
  editing: Option<(i64, SliderEndpoint)>,
  edit_text: &str,
) -> Element<'a, Message> {
  let (lo, hi) = (template.bound_lo, template.bound_hi);
  let (filter_min, filter_max) = filter.map(|(min, max)| (min.max(lo), max.min(hi))).unwrap_or((lo, hi));
  let is_active = filter.is_some();
  let unit = unit_suffix(template);

  let readout_color = if is_active {
    color::accent::PLASMA
  } else {
    color::text::tertiary()
  };

  let min_el = value_label(
    template.attribute_id,
    SliderEndpoint::Min,
    filter_min,
    unit,
    editing,
    edit_text,
  );
  let max_el = value_label(
    template.attribute_id,
    SliderEndpoint::Max,
    filter_max,
    unit,
    editing,
    edit_text,
  );

  let readout_row = Row::with_children(vec![
    min_el,
    text(" \u{2013} ")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(readout_color),
      })
      .into(),
    max_el,
    text(format!(" {}", unit.trim()))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(readout_color),
      })
      .into(),
  ])
  .align_y(iced::alignment::Vertical::Center);

  let label_row = Row::with_children(vec![
    text(template.display_name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    readout_row.into(),
  ])
  .align_y(iced::alignment::Vertical::Center);

  let track = Canvas::new(RangeSliderProgram {
    attribute_id: template.attribute_id,
    current_max: filter_max,
    current_min: filter_min,
    hi,
    lo,
  })
  .width(Length::Fill)
  .height(TRACK_HEIGHT);

  let base_suffix = if unit.trim() == "%" { unit } else { "" };
  let base_unit = if unit.trim() == "%" { "" } else { unit.trim() };
  let bounds_row = Row::with_children(vec![
    bound_text(format_stat_value(lo, unit)),
    Space::new().width(Length::Fill).into(),
    bound_text(format!(
      "base {}{}",
      format_stat_value(template.base_value, base_suffix),
      base_unit
    )),
    Space::new().width(Length::Fill).into(),
    bound_text(format_stat_value(hi, unit)),
  ]);

  Column::with_children(vec![
    label_row.into(),
    Space::new().height(6.0).into(),
    track.into(),
    Space::new().height(3.0).into(),
    bounds_row.into(),
  ])
  .into()
}

fn value_label<'a>(
  attribute_id: i64,
  endpoint: SliderEndpoint,
  value: f64,
  unit: &str,
  editing: Option<(i64, SliderEndpoint)>,
  edit_text: &str,
) -> Element<'a, Message> {
  let is_editing = editing.is_some_and(|(id, ep)| id == attribute_id && ep == endpoint);
  if is_editing {
    return text_input("", edit_text)
      .on_input(Message::AbyssalSliderEditInput)
      .on_submit(Message::AbyssalSliderEditCommitted(attribute_id, endpoint))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .width(56.0)
      .style(|_, _| text_input::Style {
        background: Background::Color(color::with_alpha(color::accent::PLASMA, 0.08)),
        border: Border {
          color: color::accent::PLASMA,
          radius: 3.0.into(),
          width: 1.0,
        },
        icon: color::accent::PLASMA,
        placeholder: color::text::tertiary(),
        selection: color::with_alpha(color::accent::PLASMA, 0.25),
        value: color::accent::PLASMA,
      })
      .into();
  }

  let label = format!("{}{}", format_stat_value(value, ""), unit.trim());
  button(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding::ZERO)
  .on_press(Message::AbyssalSliderEditStarted(attribute_id, endpoint, value))
  .style(|_, _| button::Style {
    background: None,
    border: Border::default(),
    text_color: color::accent::PLASMA,
    ..button::Style::default()
  })
  .into()
}

fn bound_text(label: String) -> Element<'static, Message> {
  text(label)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    })
    .into()
}

fn unit_suffix(template: &StatTemplate) -> &'static str {
  super::unit_suffix_for_id(template.unit_id)
}

fn fraction_for_value(value: f64, lo: f64, hi: f64) -> f32 {
  let range = (hi - lo).max(EPSILON);
  (((value - lo) / range).clamp(0.0, 1.0)) as f32
}

fn value_at_fraction(fraction: f32, lo: f64, hi: f64) -> f64 {
  let range = (hi - lo).max(EPSILON);
  lo + f64::from(fraction.clamp(0.0, 1.0)) * range
}

fn thumb_style(dragging: bool) -> (f32, iced::Color) {
  if dragging {
    (8.0, color::accent::PLASMA)
  } else {
    (THUMB_RADIUS, color::text::PRIMARY)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn template(attribute_id: i64, lo: f64, hi: f64) -> StatTemplate {
    StatTemplate {
      attribute_id,
      base_value: (lo + hi) / 2.0,
      bound_hi: hi,
      bound_lo: lo,
      display_name: format!("Attr {attribute_id}"),
      high_is_good: true,
      unit_id: Some(115),
    }
  }

  fn slider(min: f64, max: f64) -> RangeSliderProgram {
    RangeSliderProgram {
      attribute_id: 1,
      current_max: max,
      current_min: min,
      hi: 100.0,
      lo: 0.0,
    }
  }

  mod fraction_for_value {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_values_outside_the_bounds() {
      assert_eq!(fraction_for_value(-5.0, 10.0, 50.0), 0.0);
      assert_eq!(fraction_for_value(99.0, 10.0, 50.0), 1.0);
    }

    #[test]
    fn it_maps_the_endpoints_to_zero_and_one() {
      assert_eq!(fraction_for_value(10.0, 10.0, 50.0), 0.0);
      assert_eq!(fraction_for_value(50.0, 10.0, 50.0), 1.0);
    }

    #[test]
    fn it_maps_the_midpoint_to_a_half() {
      assert_eq!(fraction_for_value(30.0, 10.0, 50.0), 0.5);
    }

    #[test]
    fn it_yields_zero_for_a_degenerate_range() {
      assert_eq!(fraction_for_value(10.0, 10.0, 10.0), 0.0);
    }
  }

  mod nearest_target {
    use super::*;

    fn program(min: f64, max: f64) -> RangeSliderProgram {
      slider(min, max)
    }

    #[test]
    fn it_picks_the_max_thumb_when_the_cursor_is_nearer_to_it() {
      let inner_w = 100.0;
      let program = program(20.0, 80.0);

      assert!(program.nearest_target(THUMB_RADIUS + 78.0, inner_w) == DragTarget::Max);
    }

    #[test]
    fn it_picks_the_min_thumb_when_the_cursor_is_nearer_to_it() {
      let inner_w = 100.0;
      let program = program(20.0, 80.0);

      assert!(program.nearest_target(THUMB_RADIUS + 22.0, inner_w) == DragTarget::Min);
    }
  }

  mod panel {
    use super::*;
    use crate::features::assets::{State, abyssals::Filters};

    fn selected_filters() -> Filters {
      Filters {
        source_type_id: Some(2410),
        ..Filters::default()
      }
    }

    #[test]
    fn it_renders_sliders_for_the_selected_type_templates() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_abyssals_for_test(vec![], vec![], selected_filters(), false);
      state.set_abyssal_stat_templates_for_test(vec![template(50, 28.0, 56.0)]);

      let _el: Element<'_, Message> = panel(&state);
    }

    #[test]
    fn it_renders_the_placeholder_when_every_template_is_degenerate() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_abyssals_for_test(vec![], vec![], selected_filters(), false);
      state.set_abyssal_stat_templates_for_test(vec![template(50, 33.0, 33.0)]);

      let _el: Element<'_, Message> = panel(&state);
    }

    #[test]
    fn it_renders_the_placeholder_when_no_type_is_selected() {
      let state = State::new(crate::config::FeatureFlags::default());

      let _el: Element<'_, Message> = panel(&state);
    }
  }

  mod segment_color {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_dims_the_segment_when_both_endpoints_sit_on_their_bounds() {
      assert_eq!(
        slider(0.0, 100.0).segment_color(),
        color::with_alpha(color::text::PRIMARY, 0.22)
      );
    }

    #[test]
    fn it_highlights_the_segment_when_an_endpoint_has_moved() {
      assert_eq!(slider(20.0, 100.0).segment_color(), color::accent::PLASMA);
    }
  }

  mod thumb_style {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_grows_and_recolors_the_dragged_thumb() {
      let (radius, color) = thumb_style(true);

      assert_eq!(radius, 8.0);
      assert_eq!(color, color::accent::PLASMA);
    }

    #[test]
    fn it_uses_the_base_radius_for_an_idle_thumb() {
      let (radius, color) = thumb_style(false);

      assert_eq!(radius, THUMB_RADIUS);
      assert_eq!(color, color::text::PRIMARY);
    }
  }

  mod update {
    use iced::{Event, Point, Rectangle, widget::canvas::Program as _};

    use super::*;

    const BOUNDS: Rectangle = Rectangle {
      x: 0.0,
      y: 0.0,
      width: 120.0,
      height: TRACK_HEIGHT,
    };

    fn program() -> RangeSliderProgram {
      RangeSliderProgram {
        attribute_id: 7,
        current_max: 80.0,
        current_min: 20.0,
        hi: 100.0,
        lo: 0.0,
      }
    }

    fn cursor_at(x: f32) -> mouse::Cursor {
      mouse::Cursor::Available(Point::new(x, BOUNDS.height / 2.0))
    }

    #[test]
    fn it_clears_the_drag_on_a_left_release() {
      let program = program();
      let mut state = RangeSliderState {
        dragging: Some(DragTarget::Max),
      };
      let event = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));

      let action = program.update(&mut state, &event, BOUNDS, cursor_at(50.0));

      assert!(action.is_some());
      assert_eq!(state.dragging, None);
    }

    #[test]
    fn it_ignores_a_cursor_move_when_not_dragging() {
      let program = program();
      let mut state = RangeSliderState::default();
      let event = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(40.0, 10.0),
      });

      assert!(program.update(&mut state, &event, BOUNDS, cursor_at(40.0)).is_none());
    }

    #[test]
    fn it_ignores_non_mouse_events() {
      let program = program();
      let mut state = RangeSliderState::default();
      let event = Event::Window(iced::window::Event::Unfocused);

      assert!(program.update(&mut state, &event, BOUNDS, cursor_at(40.0)).is_none());
    }

    #[test]
    fn it_publishes_a_min_change_while_dragging_the_min_thumb() {
      let program = program();
      let mut state = RangeSliderState {
        dragging: Some(DragTarget::Min),
      };
      let event = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(THUMB_RADIUS + 30.0, 10.0),
      });

      let action = program
        .update(&mut state, &event, BOUNDS, cursor_at(THUMB_RADIUS + 30.0))
        .expect("dragging publishes a stat change");

      match action.into_inner().0 {
        Some(Message::AbyssalStatMinChanged(id, _)) => assert_eq!(id, 7),
        other => panic!("expected AbyssalStatMinChanged, got {other:?}"),
      }
    }

    #[test]
    fn it_starts_dragging_the_nearer_thumb_on_a_left_press() {
      let program = program();
      let mut state = RangeSliderState::default();
      let event = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));

      let action = program.update(&mut state, &event, BOUNDS, cursor_at(THUMB_RADIUS + 10.0));

      assert!(action.is_some());
      assert_eq!(state.dragging, Some(DragTarget::Min));
    }
  }

  mod value_at_fraction {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_fractions_outside_the_unit_interval() {
      assert_eq!(value_at_fraction(-1.0, 10.0, 50.0), 10.0);
      assert_eq!(value_at_fraction(2.0, 10.0, 50.0), 50.0);
    }

    #[test]
    fn it_maps_a_half_to_the_midpoint() {
      assert_eq!(value_at_fraction(0.5, 10.0, 50.0), 30.0);
    }

    #[test]
    fn it_maps_zero_and_one_to_the_endpoints() {
      assert_eq!(value_at_fraction(0.0, 10.0, 50.0), 10.0);
      assert_eq!(value_at_fraction(1.0, 10.0, 50.0), 50.0);
    }
  }
}
