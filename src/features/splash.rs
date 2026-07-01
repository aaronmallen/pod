pub mod animation;
pub mod preflight;
pub mod seed;
pub mod status_message;
pub mod version;

use chrono::{DateTime, Utc};
use iced::{
  Element, Length, Padding, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container, mouse_area, text},
};

use crate::{
  features::shell::window_chrome,
  ui::{
    components::{button::Button, eve_time::eve_time, status::dot, status_bar::status_bar},
    style::{color, spacing, typography},
  },
};

const STAGE_PADDING: f32 = 56.0;

#[derive(Clone, Debug)]
pub enum Message {
  BeginChecking,
  DownloadProgress(f32),
  DragWindow,
  ExpandComplete,
  Failed(String),
  Later,
  LoadingComplete,
  Retry,
  StepChanged { label: String, progress: f32 },
  Tick,
  Update,
  UpdateAvailable(String),
  UpdateFailed(String),
  UpdateNotAvailable,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Phase {
  CheckingUpdate,
  Done,
  Expanding,
  Loading,
  Update,
  Updating,
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
  pub update_error: Option<String>,
  pub update_progress: f32,
  pub update_version: Option<String>,
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
      step_label: t!("splash.status.starting_up").into_owned(),
      update_error: None,
      update_progress: 0.0,
      update_version: None,
    }
  }
}

pub fn update(state: &mut State, message: Message) -> Task<Message> {
  match message {
    Message::BeginChecking => {
      state.phase = Phase::CheckingUpdate;
      state.step_label = t!("splash.status.checking_update").into_owned();
      Task::none()
    }
    Message::DownloadProgress(progress) => {
      if state.phase == Phase::Updating {
        state.update_progress = progress.clamp(0.0, 1.0);
      }
      Task::none()
    }
    Message::DragWindow | Message::ExpandComplete => Task::none(),
    Message::Failed(error) => {
      state.error = Some(error);
      Task::none()
    }
    Message::Later => {
      if state.phase == Phase::Update {
        state.phase = Phase::Loading;
      }
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
    Message::Update => {
      if state.phase == Phase::Update {
        state.phase = Phase::Updating;
        state.update_progress = 0.0;
      }
      Task::none()
    }
    Message::UpdateAvailable(version) => {
      if state.phase == Phase::CheckingUpdate {
        state.phase = Phase::Update;
        state.update_version = Some(version);
      }
      Task::none()
    }
    Message::UpdateFailed(error) => {
      if state.phase == Phase::Updating {
        state.update_error = Some(error);
        state.phase = Phase::Loading;
      }
      Task::none()
    }
    Message::UpdateNotAvailable => {
      if state.phase == Phase::CheckingUpdate {
        state.phase = Phase::Loading;
      }
      Task::none()
    }
  }
}

pub fn view<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let label = match state.phase {
    Phase::CheckingUpdate => t!("splash.status.checking_update").into_owned(),
    Phase::Done | Phase::Expanding => t!("splash.status.ready").into_owned(),
    Phase::Loading | Phase::Update | Phase::Updating => state.step_label.clone(),
  };
  let progress = match state.phase {
    Phase::CheckingUpdate | Phase::Loading => Some(state.progress),
    Phase::Updating => Some(state.update_progress),
    Phase::Done | Phase::Expanding | Phase::Update => None,
  };

  let logo = mouse_area(animation::logo(
    state.rotation,
    state.pulse,
    state.expand,
    logo_height(&state.phase),
  ))
  .on_press(Message::DragWindow)
  .into();
  let slot = match (&state.error, &state.phase) {
    (Some(error), _) => error_view(error),
    (None, Phase::Update) => update_view(state),
    (None, Phase::Updating) => updating_view(state),
    (None, _) => status_message::status_message(&label, progress, Horizontal::Center),
  };

  let stage = container(
    Column::with_children(vec![
      Space::new().width(Length::Fill).height(Length::FillPortion(5)).into(),
      logo,
      Space::new().width(Length::Fill).height(Length::FillPortion(4)).into(),
      slot,
    ])
    .align_x(Horizontal::Center),
  )
  .padding(Padding {
    top: 0.0,
    right: STAGE_PADDING,
    bottom: spacing::SPACE_6,
    left: STAGE_PADDING,
  })
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center);

  let status = status_bar(vec![eve_time(now)], vec![version::version(footer_update(state))]);
  let bar = mouse_area(status).on_press(Message::DragWindow).into();

  container(Column::with_children(vec![stage.into(), bar]))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(window_chrome::panel_style)
    .into()
}

fn logo_height(phase: &Phase) -> f32 {
  match phase {
    Phase::Update => animation::UPDATE_HEIGHT,
    _ => animation::HEIGHT,
  }
}

fn footer_update(state: &State) -> Option<&str> {
  match state.phase {
    Phase::Update | Phase::Updating => state.update_version.as_deref(),
    _ => None,
  }
}

fn error_view<'a>(error: &str) -> Element<'a, Message> {
  let message = text(t!("splash.view.error", error => error).into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::status::DANGER),
    });

  let retry = Button::primary(t!("splash.view.retry").into_owned()).on_press(Message::Retry);

  Column::with_children(vec![message.into(), retry.into()])
    .align_x(Horizontal::Center)
    .spacing(spacing::SPACE_3)
    .into()
}

fn update_view<'a>(state: &'a State) -> Element<'a, Message> {
  let current = env!("CARGO_PKG_VERSION");
  let next = state.update_version.as_deref().unwrap_or("");

  let eyebrow = Row::with_children(vec![
    dot(color::accent::PLASMA),
    text(t!("splash.update.eyebrow").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let title = text(t!("splash.update.available").into_owned())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG);

  let versions = Row::with_children(vec![
    text(t!("splash.version.current", version => current).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    text("\u{2192}")
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
    text(t!("splash.version.current", version => next).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let copy = Column::with_children(vec![eyebrow.into(), title.into(), versions.into()])
    .align_x(Horizontal::Left)
    .spacing(spacing::SPACE_2);

  let later = Button::ghost(t!("splash.update.later").into_owned()).on_press(Message::Later);
  let update = Button::primary(t!("splash.update.update_and_restart").into_owned()).on_press(Message::Update);

  let actions = Row::with_children(vec![
    Space::new().width(Length::Fill).height(Length::Shrink).into(),
    later.into(),
    update.into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  Column::with_children(vec![copy.into(), actions.into()])
    .width(Length::Fill)
    .spacing(spacing::SPACE_3_5)
    .into()
}

fn updating_view<'a>(state: &'a State) -> Element<'a, Message> {
  let next = state.update_version.as_deref().unwrap_or("");
  let label = updating_label(state.update_progress, next);
  status_message::status_message(&label, Some(state.update_progress), Horizontal::Left)
}

fn updating_label(progress: f32, next: &str) -> String {
  if progress >= 1.0 {
    t!("splash.status.restarting").into_owned()
  } else if progress > 0.7 {
    t!("splash.status.installing", version => next).into_owned()
  } else {
    t!("splash.status.downloading_update").into_owned()
  }
}

fn tick(state: &mut State) -> Task<Message> {
  state.progress += (state.progress_target - state.progress) * 0.08;
  state.pulse += 0.05;

  match state.phase {
    Phase::Done | Phase::Update => Task::none(),
    Phase::Expanding => advance_expand(state),
    Phase::CheckingUpdate | Phase::Loading | Phase::Updating => {
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

  mod logo_height {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_shrinks_the_logo_for_the_update_prompt() {
      assert_eq!(logo_height(&Phase::Update), animation::UPDATE_HEIGHT);
    }

    #[test]
    fn it_keeps_the_full_logo_for_the_other_phases() {
      assert_eq!(logo_height(&Phase::Loading), animation::HEIGHT);
      assert_eq!(logo_height(&Phase::Updating), animation::HEIGHT);
      assert_eq!(logo_height(&Phase::CheckingUpdate), animation::HEIGHT);
      assert_eq!(logo_height(&Phase::Expanding), animation::HEIGHT);
      assert_eq!(logo_height(&Phase::Done), animation::HEIGHT);
    }
  }

  mod view {
    use super::*;

    fn now() -> DateTime<Utc> {
      DateTime::from_timestamp(0, 0).expect("epoch is a valid timestamp")
    }

    #[test]
    fn it_builds_the_update_prompt_without_panicking() {
      let state = State {
        phase: Phase::Update,
        update_version: Some("0.7.0".to_string()),
        ..State::default()
      };

      let _: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_builds_the_error_prompt_without_panicking() {
      let state = State {
        error: Some("seed boom".to_string()),
        ..State::default()
      };

      let _: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_builds_the_loading_and_updating_views_without_panicking() {
      let loading = State::default();
      let updating = State {
        phase: Phase::Updating,
        update_version: Some("0.7.0".to_string()),
        ..State::default()
      };

      let _: Element<'_, Message> = view(&loading, now());
      let _: Element<'_, Message> = view(&updating, now());
    }
  }

  mod footer_update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_exposes_the_version_during_update_and_updating() {
      let update = State {
        phase: Phase::Update,
        update_version: Some("0.7.0".to_string()),
        ..State::default()
      };
      let updating = State {
        phase: Phase::Updating,
        update_version: Some("0.7.0".to_string()),
        ..State::default()
      };

      assert_eq!(footer_update(&update), Some("0.7.0"));
      assert_eq!(footer_update(&updating), Some("0.7.0"));
    }

    #[test]
    fn it_hides_the_version_outside_the_update_phases() {
      let loading = State {
        update_version: Some("0.7.0".to_string()),
        ..State::default()
      };

      assert_eq!(footer_update(&loading), None);
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_enters_checking_update_on_begin_checking() {
      let mut state = State::default();

      let _ = update(&mut state, Message::BeginChecking);

      assert_eq!(state.phase, Phase::CheckingUpdate);
    }

    #[test]
    fn it_enters_loading_when_no_update_is_available() {
      let mut state = State {
        phase: Phase::CheckingUpdate,
        ..State::default()
      };

      let _ = update(&mut state, Message::UpdateNotAvailable);

      assert_eq!(state.phase, Phase::Loading);
    }

    #[test]
    fn it_enters_updating_when_the_user_chooses_update() {
      let mut state = State {
        phase: Phase::Update,
        ..State::default()
      };

      let _ = update(&mut state, Message::Update);

      assert_eq!(state.phase, Phase::Updating);
    }

    #[test]
    fn it_falls_through_to_loading_when_the_update_fails() {
      let mut state = State {
        phase: Phase::Updating,
        ..State::default()
      };

      let _ = update(&mut state, Message::UpdateFailed("install boom".to_string()));

      assert_eq!(state.phase, Phase::Loading);
      assert_eq!(state.update_error.as_deref(), Some("install boom"));
    }

    #[test]
    fn it_ignores_a_late_update_available_after_leaving_checking() {
      let mut state = State {
        phase: Phase::Loading,
        ..State::default()
      };

      let _ = update(&mut state, Message::UpdateAvailable("9.9.9".to_string()));

      assert_eq!(state.phase, Phase::Loading);
      assert_eq!(state.update_version, None);
    }

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
    fn it_proceeds_to_loading_when_the_user_chooses_later() {
      let mut state = State {
        phase: Phase::Update,
        ..State::default()
      };

      let _ = update(&mut state, Message::Later);

      assert_eq!(state.phase, Phase::Loading);
    }

    #[test]
    fn it_records_download_progress_during_updating() {
      let mut state = State {
        phase: Phase::Updating,
        ..State::default()
      };

      let _ = update(&mut state, Message::DownloadProgress(0.42));

      assert_eq!(state.update_progress, 0.42);
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

    #[test]
    fn it_transitions_to_update_when_an_update_is_available() {
      let mut state = State {
        phase: Phase::CheckingUpdate,
        ..State::default()
      };

      let _ = update(&mut state, Message::UpdateAvailable("9.9.9".to_string()));

      assert_eq!(state.phase, Phase::Update);
      assert_eq!(state.update_version.as_deref(), Some("9.9.9"));
    }
  }

  mod updating_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_installs_the_target_version_past_the_download() {
      assert_eq!(updating_label(0.85, "0.7.0"), "Installing v0.7.0\u{2026}");
    }

    #[test]
    fn it_reports_downloading_while_in_the_early_progress() {
      assert_eq!(updating_label(0.2, "0.7.0"), "Downloading update\u{2026}");
    }

    #[test]
    fn it_restarts_once_the_install_completes() {
      assert_eq!(updating_label(1.0, "0.7.0"), "Restarting\u{2026}");
    }
  }
}
