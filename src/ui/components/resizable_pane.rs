use iced::{
  Background, Element, Length,
  widget::{Space, container, mouse_area, row},
};

use crate::ui::style::color;

pub const MIN_PANE_WIDTH: f32 = 100.0;

const HANDLE_WIDTH: f32 = 4.0;

const RULE_WIDTH: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaneDrag {
  active: bool,
  last_x: Option<f32>,
  min_width: f32,
  right_anchored: bool,
  width: f32,
}

impl PaneDrag {
  pub fn from_store(state: &crate::window_state::UiState, key: &str, default: f32) -> Self {
    Self::new(state.panes.get(key).copied().unwrap_or(default))
  }

  pub fn new(width: f32) -> Self {
    Self::with_min_width(width, MIN_PANE_WIDTH)
  }

  pub fn with_min_width(width: f32, min_width: f32) -> Self {
    let min_width = min_width.max(0.0);
    Self {
      active: false,
      last_x: None,
      min_width,
      right_anchored: false,
      width: width.max(min_width),
    }
  }

  pub fn drag_to(&mut self, x: f32) -> f32 {
    if !self.active {
      return self.width;
    }
    if let Some(last_x) = self.last_x {
      let delta = x - last_x;
      let signed = if self.right_anchored { -delta } else { delta };
      self.width = (self.width + signed).max(self.min_width);
    }
    self.last_x = Some(x);
    self.width
  }

  pub fn end(&mut self) {
    self.active = false;
    self.last_x = None;
  }

  pub fn is_active(&self) -> bool {
    self.active
  }

  pub fn right_anchored(mut self, right_anchored: bool) -> Self {
    self.right_anchored = right_anchored;
    self
  }

  pub fn start(&mut self) {
    self.active = true;
    self.last_x = None;
  }

  pub fn width(&self) -> f32 {
    self.width
  }
}

pub fn drag_event<M>(event: iced::Event, on_drag: impl Fn(f32) -> M, on_drag_end: M) -> Option<M> {
  match event {
    iced::Event::Mouse(iced::mouse::Event::CursorMoved {
      position,
    }) => Some(on_drag(position.x)),
    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => Some(on_drag_end),
    _ => None,
  }
}

pub fn pane_handle<'a, M>(on_drag_start: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  mouse_area(handle_visual())
    .on_press(on_drag_start)
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .into()
}

fn handle_visual<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  let side = (HANDLE_WIDTH - RULE_WIDTH) / 2.0;
  row![
    Space::new().width(Length::Fixed(side)).height(Length::Fill),
    container(Space::new().width(Length::Fixed(RULE_WIDTH)).height(Length::Fill))
      .width(Length::Fixed(RULE_WIDTH))
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.12))),
        ..container::Style::default()
      }),
    Space::new().width(Length::Fixed(side)).height(Length::Fill),
  ]
  .width(Length::Fixed(HANDLE_WIDTH))
  .height(Length::Fill)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod pane_handle {
    use super::*;

    #[test]
    fn it_builds_a_handle_element_for_an_arbitrary_message_type() {
      let _el: Element<'_, ()> = pane_handle(());
    }

    #[test]
    fn it_is_generic_over_the_feature_message() {
      #[derive(Clone)]
      enum Msg {
        Start,
      }
      let _el: Element<'_, Msg> = pane_handle(Msg::Start);
    }
  }

  mod with_min_width {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_a_below_minimum_starting_width_up_to_the_minimum() {
      let drag = PaneDrag::with_min_width(40.0, 100.0);
      assert_eq!(drag.width(), 100.0);
    }

    #[test]
    fn it_keeps_an_above_minimum_starting_width() {
      let drag = PaneDrag::with_min_width(250.0, 100.0);
      assert_eq!(drag.width(), 250.0);
    }

    #[test]
    fn it_floors_a_negative_minimum_at_zero() {
      let drag = PaneDrag::with_min_width(-5.0, -100.0);
      assert_eq!(drag.width(), 0.0);
    }
  }

  mod from_store {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::window_state::UiState;

    #[test]
    fn it_reads_a_stored_pane_width_by_key() {
      let mut state = UiState::default();
      state.panes.insert("skills.left".to_owned(), 320.0);

      let drag = PaneDrag::from_store(&state, "skills.left", 240.0);

      assert_eq!(drag.width(), 320.0);
    }

    #[test]
    fn it_falls_back_to_the_default_for_an_unsized_pane() {
      let state = UiState::default();

      let drag = PaneDrag::from_store(&state, "skills.left", 240.0);

      assert_eq!(drag.width(), 240.0);
    }

    #[test]
    fn it_clamps_a_stored_width_below_the_minimum() {
      let mut state = UiState::default();
      state.panes.insert("skills.left".to_owned(), 10.0);

      let drag = PaneDrag::from_store(&state, "skills.left", 240.0);

      assert_eq!(drag.width(), MIN_PANE_WIDTH);
    }
  }

  mod drag_to {
    use pretty_assertions::assert_eq;

    use super::*;

    fn started(width: f32) -> PaneDrag {
      let mut drag = PaneDrag::new(width);
      drag.start();
      drag
    }

    #[test]
    fn it_does_not_resize_on_the_first_move_that_only_sets_the_anchor() {
      let mut drag = started(200.0);

      let width = drag.drag_to(500.0);

      assert_eq!(width, 200.0);
      assert_eq!(drag.width(), 200.0);
    }

    #[test]
    fn it_grows_the_pane_by_a_rightward_delta() {
      let mut drag = started(200.0);
      drag.drag_to(500.0);

      let width = drag.drag_to(530.0);

      assert_eq!(width, 230.0);
    }

    #[test]
    fn it_shrinks_the_pane_by_a_leftward_delta() {
      let mut drag = started(200.0);
      drag.drag_to(500.0);

      let width = drag.drag_to(460.0);

      assert_eq!(width, 160.0);
    }

    #[test]
    fn it_shrinks_a_right_anchored_pane_on_a_rightward_delta() {
      let mut drag = PaneDrag::new(200.0).right_anchored(true);
      drag.start();
      drag.drag_to(500.0);

      let width = drag.drag_to(530.0);

      assert_eq!(width, 170.0);
    }

    #[test]
    fn it_grows_a_right_anchored_pane_on_a_leftward_delta() {
      let mut drag = PaneDrag::new(200.0).right_anchored(true);
      drag.start();
      drag.drag_to(500.0);

      let width = drag.drag_to(470.0);

      assert_eq!(width, 230.0);
    }

    #[test]
    fn it_accumulates_deltas_across_successive_moves() {
      let mut drag = started(200.0);
      drag.drag_to(500.0);
      drag.drag_to(520.0);
      drag.drag_to(515.0);

      assert_eq!(drag.width(), 215.0);
    }

    #[test]
    fn it_clamps_at_the_minimum_when_dragged_past_it() {
      let mut drag = started(120.0);
      drag.drag_to(500.0);

      let width = drag.drag_to(300.0);

      assert_eq!(width, MIN_PANE_WIDTH);
    }

    #[test]
    fn it_resizes_relative_to_the_clamped_width_after_overshooting_the_minimum() {
      let mut drag = started(120.0);
      drag.drag_to(500.0);
      drag.drag_to(300.0);

      let width = drag.drag_to(330.0);

      assert_eq!(width, 130.0);
    }

    #[test]
    fn it_ignores_moves_when_no_drag_is_active() {
      let mut drag = PaneDrag::new(200.0);

      let width = drag.drag_to(500.0);

      assert_eq!(width, 200.0);
      assert!(!drag.is_active());
    }

    #[test]
    fn it_respects_a_custom_minimum() {
      let mut drag = PaneDrag::with_min_width(200.0, 150.0);
      drag.start();
      drag.drag_to(500.0);

      let width = drag.drag_to(400.0);

      assert_eq!(width, 150.0);
    }
  }

  mod lifecycle {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_inactive_until_started() {
      let drag = PaneDrag::new(200.0);
      assert!(!drag.is_active());
    }

    #[test]
    fn it_is_active_while_dragging_and_inactive_after_end() {
      let mut drag = PaneDrag::new(200.0);

      drag.start();
      assert!(drag.is_active());

      drag.end();
      assert!(!drag.is_active());
    }

    #[test]
    fn it_clears_the_anchor_on_start_so_a_new_drag_does_not_jump() {
      let mut drag = PaneDrag::new(200.0);
      drag.start();
      drag.drag_to(500.0);
      drag.drag_to(540.0);
      drag.end();

      drag.start();
      let width = drag.drag_to(900.0);

      assert_eq!(width, 240.0);
    }

    #[test]
    fn it_keeps_the_settled_width_through_end() {
      let mut drag = PaneDrag::new(200.0);
      drag.start();
      drag.drag_to(500.0);
      drag.drag_to(560.0);
      drag.end();

      assert_eq!(drag.width(), 260.0);
    }
  }

  mod drag_event {
    use pretty_assertions::assert_eq;

    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum Msg {
      Drag(f32),
      End,
    }

    #[test]
    fn it_maps_a_cursor_move_to_the_drag_message_with_the_cursor_x() {
      let event = iced::Event::Mouse(iced::mouse::Event::CursorMoved {
        position: iced::Point::new(420.0, 100.0),
      });

      let msg = drag_event(event, Msg::Drag, Msg::End);

      assert_eq!(msg, Some(Msg::Drag(420.0)));
    }

    #[test]
    fn it_maps_a_left_button_release_to_the_drag_end_message() {
      let event = iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left));

      let msg = drag_event(event, Msg::Drag, Msg::End);

      assert_eq!(msg, Some(Msg::End));
    }

    #[test]
    fn it_ignores_a_non_left_button_release() {
      let event = iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Right));

      let msg = drag_event(event, Msg::Drag, Msg::End);

      assert_eq!(msg, None);
    }

    #[test]
    fn it_ignores_unrelated_events() {
      let event = iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left));

      let msg = drag_event(event, Msg::Drag, Msg::End);

      assert_eq!(msg, None);
    }
  }

  mod persistence_roundtrip {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::window_state::UiState;

    #[test]
    fn it_reconstructs_the_settled_width_from_the_keyed_store() {
      let mut store = UiState::default();
      let mut drag = PaneDrag::from_store(&store, "skills.left", 240.0);

      drag.start();
      drag.drag_to(500.0);
      drag.drag_to(580.0);
      drag.end();

      store.panes.insert("skills.left".to_owned(), drag.width());

      let restored = PaneDrag::from_store(&store, "skills.left", 240.0);

      assert_eq!(restored.width(), 320.0);
    }
  }
}
