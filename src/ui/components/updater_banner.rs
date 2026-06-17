use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text},
};

use crate::{
  services::updater::State,
  ui::style::{color, control, radius, shadow, spacing, typography},
};

const BANNER_PAD_X: f32 = spacing::SPACE_3_5;
const BANNER_PAD_Y: f32 = spacing::SPACE_2_5;
const DISMISS_SIZE: f32 = 20.0;
const TOAST_MARGIN: f32 = spacing::SPACE_3_5;
const TOAST_WIDTH: f32 = 300.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
  Apply,
  Restart,
}

impl Action {
  fn label(self) -> &'static str {
    match self {
      Action::Apply => "Update now",
      Action::Restart => "Restart",
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Presentation {
  pub action: Option<Action>,
  pub eyebrow: &'static str,
  pub is_error: bool,
  pub message: String,
}

pub fn banner<'a, M, F>(state: &State, on_action: F) -> Option<Element<'a, M>>
where
  M: Clone + 'a,
  F: Fn(Action) -> M,
{
  let presentation = presentation(state)?;
  let accent = if presentation.is_error {
    color::status::DANGER
  } else {
    color::accent::PLASMA
  };

  let label = Column::with_children(vec![
    text(presentation.eyebrow)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(accent),
      })
      .into(),
    text(presentation.message)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2 / 2.0);

  let mut children: Vec<Element<'a, M>> = vec![label.into(), Space::new().width(Length::Fill).into()];
  if let Some(action) = presentation.action {
    children.push(action_button(action, on_action(action)));
  }

  let row = Row::with_children(children)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center);

  Some(
    container(row)
      .width(Length::Fill)
      .padding(Padding {
        top: BANNER_PAD_Y,
        right: BANNER_PAD_X,
        bottom: BANNER_PAD_Y,
        left: BANNER_PAD_X,
      })
      .style(move |_| container::Style {
        background: Some(Background::Color(color::with_alpha(accent, 0.12))),
        border: Border {
          color: color::with_alpha(accent, 0.4),
          width: 1.0,
          radius: 0.0.into(),
        },
        ..container::Style::default()
      })
      .into(),
  )
}

pub fn presentation(state: &State) -> Option<Presentation> {
  match state {
    State::Idle => None,
    State::UpdateAvailable {
      version,
    } => Some(Presentation {
      action: Some(Action::Apply),
      eyebrow: "UPDATE",
      is_error: false,
      message: format!("Version {version} is available."),
    }),
    State::Downloading {
      version,
    } => Some(Presentation {
      action: None,
      eyebrow: "UPDATE",
      is_error: false,
      message: format!("Downloading version {version}\u{2026}"),
    }),
    State::ReadyToRestart {
      version,
    } => Some(Presentation {
      action: Some(Action::Restart),
      eyebrow: "UPDATE",
      is_error: false,
      message: format!("Version {version} is ready. Restart to finish."),
    }),
    State::Error {
      message,
    } => Some(Presentation {
      action: None,
      eyebrow: "UPDATE FAILED",
      is_error: true,
      message: message.clone(),
    }),
  }
}

pub fn toast<'a, M, F>(state: &State, on_action: F, on_dismiss: M) -> Option<Element<'a, M>>
where
  M: Clone + 'a,
  F: Fn(Action) -> M,
{
  let presentation = presentation(state)?;
  let accent = if presentation.is_error {
    color::status::DANGER
  } else {
    color::accent::PLASMA
  };

  let header = Row::with_children(vec![
    text(presentation.eyebrow)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(accent),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    dismiss_button(on_dismiss),
  ])
  .align_y(Vertical::Center);

  let mut column = vec![
    header.into(),
    text(presentation.message)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ];
  if let Some(action) = presentation.action {
    column.push(
      Row::with_children(vec![
        Space::new().width(Length::Fill).into(),
        action_button(action, on_action(action)),
      ])
      .into(),
    );
  }

  let card = container(Column::with_children(column).spacing(spacing::SPACE_2))
    .width(Length::Fixed(TOAST_WIDTH))
    .padding(spacing::SPACE_3)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      shadow: shadow::CARD,
      ..container::Style::default()
    });

  Some(
    container(card)
      .width(Length::Fill)
      .height(Length::Fill)
      .padding(TOAST_MARGIN)
      .align_x(Horizontal::Right)
      .align_y(Vertical::Bottom)
      .into(),
  )
}

fn action_button<'a, M>(action: Action, on_press: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  button(
    text(action.label())
      .font(typography::body::MEDIUM)
      .size(typography::size::SM),
  )
  .padding(control::padding())
  .on_press(on_press)
  .style(control::primary_button)
  .into()
}

fn dismiss_button<'a, M>(on_press: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  button(
    text("\u{2715}")
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .width(Length::Fixed(DISMISS_SIZE))
  .height(Length::Fixed(DISMISS_SIZE))
  .padding(0)
  .on_press(on_press)
  .style(control::ghost_button)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone, Debug, Eq, PartialEq)]
  enum Message {
    Action(Action),
    Dismiss,
  }

  mod presentation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_flags_the_error_state_and_surfaces_its_message() {
      let p = presentation(&State::Error {
        message: "download failed".to_owned(),
      })
      .expect("error state has chrome");
      assert_eq!(p.action, None);
      assert!(p.is_error);
      assert_eq!(p.message, "download failed");
    }

    #[test]
    fn it_offers_apply_when_an_update_is_available() {
      let p = presentation(&State::UpdateAvailable {
        version: "1.2.3".to_owned(),
      })
      .expect("available state has chrome");
      assert_eq!(p.action, Some(Action::Apply));
      assert!(!p.is_error);
      assert!(p.message.contains("1.2.3"));
    }

    #[test]
    fn it_offers_no_action_while_downloading() {
      let p = presentation(&State::Downloading {
        version: "1.2.3".to_owned(),
      })
      .expect("downloading state has chrome");
      assert_eq!(p.action, None);
      assert!(!p.is_error);
    }

    #[test]
    fn it_offers_restart_when_ready() {
      let p = presentation(&State::ReadyToRestart {
        version: "1.2.3".to_owned(),
      })
      .expect("ready state has chrome");
      assert_eq!(p.action, Some(Action::Restart));
      assert!(!p.is_error);
    }

    #[test]
    fn it_shows_no_chrome_when_idle() {
      assert_eq!(presentation(&State::Idle), None);
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_nothing_for_an_idle_banner_and_toast() {
      assert!(banner::<Message, _>(&State::Idle, Message::Action).is_none());
      assert!(toast::<Message, _>(&State::Idle, Message::Action, Message::Dismiss).is_none());
    }

    #[test]
    fn it_renders_the_banner_and_toast_for_each_active_state() {
      for state in [
        State::UpdateAvailable {
          version: "1.0.0".to_owned(),
        },
        State::Downloading {
          version: "1.0.0".to_owned(),
        },
        State::ReadyToRestart {
          version: "1.0.0".to_owned(),
        },
        State::Error {
          message: "boom".to_owned(),
        },
      ] {
        let _: Element<'_, Message> = banner(&state, Message::Action).expect("active state renders a banner");
        let _: Element<'_, Message> =
          toast(&state, Message::Action, Message::Dismiss).expect("active state renders a toast");
      }
    }
  }
}
