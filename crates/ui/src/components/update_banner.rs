//! Dismissible top banner that surfaces in-app update state to the user.

use iced::{
  Background, Border, Element, Length, Padding,
  widget::{Space, button, container, row, text},
};

use crate::style::{color, typography};

/// Messages produced by the update banner.
#[derive(Clone, Debug)]
pub enum Message {
  /// User clicked the apply/download button.
  ApplyPressed,
  /// User dismissed the banner.
  DismissPressed,
  /// User clicked Restart Now.
  RestartPressed,
  /// User clicked Retry after an error.
  RetryPressed,
}

/// Display state passed to the banner component.
#[derive(Clone, Debug)]
pub enum BannerState {
  /// Update is downloading and installing.
  Downloading,
  /// Download or install failed; carries a brief error description.
  Error(String),
  /// Update installed; app needs to restart.
  ReadyToRestart,
  /// A newer version is available; carries the version string.
  UpdateAvailable(String),
}

/// Horizontal top-of-window banner driven by [`BannerState`].
pub struct Component {
  state: BannerState,
}

impl Component {
  /// Creates the banner for the given display state.
  pub fn new(state: BannerState) -> Self {
    Self {
      state,
    }
  }

  /// Renders the banner into an [`Element`].
  pub fn render<'a>(self) -> Element<'a, Message> {
    let inner: Element<'a, Message> = match self.state {
      BannerState::Downloading => render_downloading(),
      BannerState::Error(msg) => render_error(msg),
      BannerState::ReadyToRestart => render_ready_to_restart(),
      BannerState::UpdateAvailable(version) => render_update_available(version),
    };

    let bottom_border = container(Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .height(1.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA_MUTED)),
        ..container::Style::default()
      });

    let content = container(inner)
      .width(Length::Fill)
      .height(40.0)
      .align_y(iced::alignment::Vertical::Center)
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: 16.0,
        right: 16.0,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA_SUBTLE)),
        ..container::Style::default()
      });

    iced::widget::column([content.into(), bottom_border.into()])
      .width(Length::Fill)
      .into()
  }
}

fn render_downloading<'a>() -> Element<'a, Message> {
  row([text("Downloading update\u{2026}")
    .font(typography::body::REGULAR)
    .size(13.0)
    .style(|_| text::Style {
      color: Some(color::text::SECONDARY),
    })
    .into()])
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn render_error<'a>(msg: String) -> Element<'a, Message> {
  let preview = if msg.len() > 64 {
    format!("{}\u{2026}", &msg[..64])
  } else {
    msg
  };
  row([
    text(format!("Update failed: {preview}"))
      .font(typography::body::REGULAR)
      .size(13.0)
      .style(|_| text::Style {
        color: Some(color::text::DANGER),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    action_button("Retry", Message::RetryPressed),
    Space::new().width(8.0).into(),
    dismiss_button(),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn render_ready_to_restart<'a>() -> Element<'a, Message> {
  row([
    text("Update ready \u{2014} restart to apply")
      .font(typography::body::REGULAR)
      .size(13.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    action_button("Restart Now", Message::RestartPressed),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn render_update_available<'a>(version: String) -> Element<'a, Message> {
  row([
    text(format!("Pod v{version} is available"))
      .font(typography::body::REGULAR)
      .size(13.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    action_button("Update", Message::ApplyPressed),
    Space::new().width(8.0).into(),
    dismiss_button(),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn action_button<'a>(label: &'a str, msg: Message) -> Element<'a, Message> {
  button(
    text(label)
      .font(typography::body::MEDIUM)
      .size(12.0)
      .style(|_| text::Style {
        color: Some(color::surface::BASE),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 12.0,
    right: 12.0,
  })
  .on_press(msg)
  .style(|_, status| button::Style {
    background: Some(Background::Color(match status {
      button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA_HOVER,
      _ => color::accent::PLASMA,
    })),
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    text_color: color::surface::BASE,
    ..button::Style::default()
  })
  .into()
}

fn dismiss_button<'a>() -> Element<'a, Message> {
  button(
    text("\u{00d7}")
      .font(typography::body::REGULAR)
      .size(14.0)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 6.0,
    right: 6.0,
  })
  .on_press(Message::DismissPressed)
  .style(|_, _| button::Style {
    background: None,
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  })
  .into()
}
