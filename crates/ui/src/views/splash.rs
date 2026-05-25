pub mod footer;
pub mod logo;
pub mod status;

pub use footer::Component as Footer;
use iced::{
  Background, Border, Element, Length,
  widget::{column, container, mouse_area},
};
pub use logo::Component as Logo;
pub use status::Component as Status;

use crate::style::color;

#[derive(Debug, PartialEq)]
pub enum Phase {
  Done,
  Expanding,
  Loading,
}

#[derive(Clone, Debug)]
pub enum Message {
  DragWindow,
  ExpandComplete,
  LoadingComplete,
  Tick,
}

#[derive(Debug)]
pub struct State {
  pub expand: f32,
  pub phase: Phase,
  pub progress: f32,
  pub progress_target: f32,
  pub pulse: f32,
  pub rotation: f32,
}

impl Default for State {
  fn default() -> Self {
    Self {
      expand: 0.0,
      phase: Phase::Loading,
      progress: 0.0,
      progress_target: 0.0,
      pulse: 0.0,
      rotation: 0.0,
    }
  }
}

pub fn update(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::DragWindow | Message::ExpandComplete => iced::Task::none(),
    Message::LoadingComplete => {
      state.phase = Phase::Expanding;
      iced::Task::none()
    }
    Message::Tick => tick(state),
  }
}

fn tick(state: &mut State) -> iced::Task<Message> {
  state.progress += (state.progress_target - state.progress) * 0.08;
  state.pulse += 0.05;

  match state.phase {
    Phase::Done => {}
    Phase::Loading => {
      state.rotation = (state.rotation + 2.0) % 360.0;
    }
    Phase::Expanding => {
      if let Some(task) = advance_expand(state) {
        return task;
      }
    }
  }

  iced::Task::none()
}

fn advance_expand(state: &mut State) -> Option<iced::Task<Message>> {
  state.expand += 0.025;
  if state.expand >= 1.0 {
    state.expand = 1.0;
    state.phase = Phase::Done;
    return Some(iced::Task::done(Message::ExpandComplete));
  }
  None
}

pub struct Component<'a> {
  state: &'a State,
  step_label: &'a str,
  version: &'a str,
}

impl<'a> Component<'a> {
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
      step_label: "",
      version: "",
    }
  }

  pub fn step_label(mut self, label: &'a str) -> Self {
    self.step_label = label;
    self
  }

  pub fn version(mut self, v: &'a str) -> Self {
    self.version = v;
    self
  }

  pub fn render(self) -> Element<'a, Message> {
    let label = match self.state.phase {
      Phase::Done | Phase::Expanding => "READY.",
      Phase::Loading => self.step_label,
    };

    let mut status = Status::new(label);
    if matches!(self.state.phase, Phase::Loading) {
      status = status.progress(self.state.progress);
    }

    let logo_el = Logo::new(self.state.rotation, self.state.pulse, self.state.expand).render::<Message>();

    let inner = container(
      column([
        logo_el,
        iced::widget::Space::new()
          .width(Length::Fill)
          .height(Length::Fill)
          .into(),
        status.render::<Message>(),
      ])
      .align_x(iced::alignment::Horizontal::Center)
      .spacing(14),
    )
    .padding(iced::Padding {
      top: 0.0,
      right: 0.0,
      bottom: 22.0,
      left: 0.0,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center);

    let footer_el = Footer::new(&self.state.phase, self.version).render::<Message>();

    let panel = container(column([inner.into(), footer_el]))
      .center_x(Length::Fill)
      .center_y(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        border: Border {
          radius: 14.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      });

    mouse_area(panel).on_press(Message::DragWindow).into()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_increments_rotation_and_pulse_on_tick_during_loading() {
      let mut state = State::default();

      let _ = update(&mut state, Message::Tick);

      assert!(state.rotation > 0.0);
      assert!(state.pulse > 0.0);
    }

    #[test]
    fn it_increments_expand_on_tick_during_expanding() {
      let mut state = State::default();
      state.phase = Phase::Expanding;

      let _ = update(&mut state, Message::Tick);

      assert!(state.expand > 0.0);
    }

    #[test]
    fn it_transitions_to_done_and_clamps_expand_on_final_tick() {
      let mut state = State::default();
      state.phase = Phase::Expanding;
      state.expand = 0.99;

      let _ = update(&mut state, Message::Tick);

      assert_eq!(state.expand, 1.0);
      assert_eq!(state.phase, Phase::Done);
    }

    #[test]
    fn it_transitions_to_expanding_on_loading_complete() {
      let mut state = State::default();

      let _ = update(&mut state, Message::LoadingComplete);

      assert_eq!(state.phase, Phase::Expanding);
    }
  }
}
