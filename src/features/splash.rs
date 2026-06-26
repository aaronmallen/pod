pub mod animation;
pub mod seed;
pub mod status_message;
pub mod version;

use chrono::{DateTime, Utc};
use iced::{
  Element, Length, Padding, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Space, button, container, mouse_area, text},
};

use crate::{
  features::shell::window_chrome,
  ui::{
    components::{eve_time::eve_time, status_bar::status_bar},
    style::{color, control, spacing, typography},
  },
};

#[derive(Clone, Debug)]
pub enum Message {
  DragWindow,
  ExpandComplete,
  Failed(String),
  LoadingComplete,
  Retry,
  StepChanged { label: String, progress: f32 },
  Tick,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Phase {
  Done,
  Expanding,
  Loading,
}

#[derive(Debug)]
pub struct State {
  pub error: Option<String>,
  pub expand: f32,
  pub phase: Phase,
  pub progress: f32,
  pub progress_target: f32,
  pub pulse: f32,
  pub rotation: f32,
  pub step_label: String,
}

impl Default for State {
  fn default() -> Self {
    Self {
      error: None,
      expand: 0.0,
      phase: Phase::Loading,
      progress: 0.0,
      progress_target: 0.0,
      pulse: 0.0,
      rotation: 0.0,
      step_label: "Starting up\u{2026}".to_string(),
    }
  }
}

pub fn update(state: &mut State, message: Message) -> Task<Message> {
  match message {
    Message::DragWindow | Message::ExpandComplete => Task::none(),
    Message::Failed(error) => {
      state.error = Some(error);
      Task::none()
    }
    Message::LoadingComplete => {
      state.phase = Phase::Expanding;
      state.progress_target = 1.0;
      Task::none()
    }
    Message::Retry => {
      *state = State::default();
      Task::none()
    }
    Message::StepChanged {
      label,
      progress,
    } => {
      state.step_label = label;
      state.progress_target = state.progress_target.max(progress).clamp(0.0, 0.99);
      Task::none()
    }
    Message::Tick => tick(state),
  }
}

pub fn view<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let label = match state.phase {
    Phase::Done | Phase::Expanding => "READY.",
    Phase::Loading => state.step_label.as_str(),
  };
  let progress = matches!(state.phase, Phase::Loading).then_some(state.progress);

  let logo = animation::logo(state.rotation, state.pulse, state.expand);
  let status = match &state.error {
    Some(error) => error_view(error),
    None => status_message::status_message(label, progress),
  };

  let inner = container(
    Column::with_children(vec![
      logo,
      Space::new().width(Length::Fill).height(Length::Fill).into(),
      status,
    ])
    .align_x(Horizontal::Center)
    .spacing(spacing::SPACE_3_5),
  )
  .padding(Padding {
    top: 0.0,
    right: 0.0,
    bottom: spacing::SPACE_3,
    left: 0.0,
  })
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center);

  let bar = status_bar(vec![eve_time(now)], vec![version::version()]);

  let panel = container(Column::with_children(vec![inner.into(), bar]))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(window_chrome::panel_style);

  mouse_area(panel).on_press(Message::DragWindow).into()
}

fn error_view<'a>(error: &str) -> Element<'a, Message> {
  let message = text(format!("Couldn\u{2019}t start Pod: {error}"))
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::status::DANGER),
    });

  let retry = button(text("Retry").font(typography::body::MEDIUM).size(typography::size::MD))
    .padding(control::padding())
    .on_press(Message::Retry)
    .style(control::primary_button);

  Column::with_children(vec![message.into(), retry.into()])
    .align_x(Horizontal::Center)
    .spacing(spacing::SPACE_3)
    .into()
}

fn tick(state: &mut State) -> Task<Message> {
  state.progress += (state.progress_target - state.progress) * 0.08;
  state.pulse += 0.05;

  match state.phase {
    Phase::Done => Task::none(),
    Phase::Expanding => advance_expand(state),
    Phase::Loading => {
      state.rotation = (state.rotation + 2.0) % 360.0;
      Task::none()
    }
  }
}

fn advance_expand(state: &mut State) -> Task<Message> {
  state.expand += 0.025;
  if state.expand >= 1.0 {
    state.expand = 1.0;
    state.phase = Phase::Done;
    return Task::done(Message::ExpandComplete);
  }
  Task::none()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_increments_expand_on_tick_during_expanding() {
      let mut state = State {
        phase: Phase::Expanding,
        ..State::default()
      };

      let _ = update(&mut state, Message::Tick);

      assert!(state.expand > 0.0);
    }

    #[test]
    fn it_increments_rotation_and_pulse_on_tick_during_loading() {
      let mut state = State::default();

      let _ = update(&mut state, Message::Tick);

      assert!(state.rotation > 0.0);
      assert!(state.pulse > 0.0);
    }

    #[test]
    fn it_never_moves_the_target_backwards_on_step_changed() {
      let mut state = State {
        progress_target: 0.6,
        ..State::default()
      };

      let _ = update(
        &mut state,
        Message::StepChanged {
          label: "Later\u{2026}".to_string(),
          progress: 0.3,
        },
      );

      assert_eq!(state.progress_target, 0.6);
    }

    #[test]
    fn it_records_the_error_on_failed() {
      let mut state = State::default();

      let _ = update(&mut state, Message::Failed("seed boom".to_string()));

      assert_eq!(state.error.as_deref(), Some("seed boom"));
    }

    #[test]
    fn it_records_the_label_and_target_on_step_changed() {
      let mut state = State::default();

      let _ = update(
        &mut state,
        Message::StepChanged {
          label: "Loading\u{2026}".to_string(),
          progress: 0.4,
        },
      );

      assert_eq!(state.step_label, "Loading\u{2026}");
      assert_eq!(state.progress_target, 0.4);
    }

    #[test]
    fn it_resets_to_a_fresh_loading_state_on_retry() {
      let mut state = State {
        error: Some("seed boom".to_string()),
        phase: Phase::Done,
        progress: 0.8,
        progress_target: 0.9,
        ..State::default()
      };

      let _ = update(&mut state, Message::Retry);

      assert_eq!(state.error, None);
      assert_eq!(state.phase, Phase::Loading);
      assert_eq!(state.progress_target, 0.0);
    }

    #[test]
    fn it_transitions_to_done_and_clamps_expand_on_the_final_tick() {
      let mut state = State {
        expand: 0.99,
        phase: Phase::Expanding,
        ..State::default()
      };

      let _ = update(&mut state, Message::Tick);

      assert_eq!(state.expand, 1.0);
      assert_eq!(state.phase, Phase::Done);
    }

    #[test]
    fn it_transitions_to_expanding_on_loading_complete() {
      let mut state = State::default();

      let _ = update(&mut state, Message::LoadingComplete);

      assert_eq!(state.phase, Phase::Expanding);
      assert_eq!(state.progress_target, 1.0);
    }
  }
}
