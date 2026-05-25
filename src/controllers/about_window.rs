//! About window controller: version, build info, and GitHub link.

use iced::{
  Background, Border, Element, Length, Size,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, column, container, text},
  window,
};
use pod_ui::style::{color, spacing, typography::body};

pub struct State {
  pub version: &'static str,
  pub build_date: &'static str,
  pub git_sha: &'static str,
}

impl Default for State {
  fn default() -> Self {
    Self {
      version: env!("CARGO_PKG_VERSION"),
      build_date: env!("BUILD_DATE"),
      git_sha: env!("GIT_SHA"),
    }
  }
}

#[derive(Debug, Clone)]
pub enum Message {
  OpenGitHub,
}

pub fn settings() -> window::Settings {
  window::Settings {
    size: Size::new(360.0, 240.0),
    resizable: false,
    position: window::Position::Centered,
    ..window::Settings::default()
  }
}

pub fn update(_state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::OpenGitHub => {
      let _ = open::that("https://github.com/aaronmallen/pod");
      iced::Task::none()
    }
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  let title = text("Pod").font(body::SEMIBOLD).size(24.0).style(text_primary_style);

  let version = text(format!("v{}", state.version))
    .font(body::MEDIUM)
    .size(14.0)
    .style(text_secondary_style);

  let build_info = text(format!("Build {} · {}", state.git_sha, state.build_date))
    .font(body::REGULAR)
    .size(11.0)
    .style(text_tertiary_style);

  let separator = container(Space::new())
    .width(Length::Fixed(240.0))
    .height(1.0)
    .style(separator_style);

  let license = text("MIT License")
    .font(body::REGULAR)
    .size(11.0)
    .style(text_tertiary_style);

  let github_link = button(
    text("github.com/aaronmallen/pod")
      .font(body::REGULAR)
      .size(11.0)
      .style(text_accent_style),
  )
  .padding(0)
  .on_press(Message::OpenGitHub)
  .style(transparent_button_style);

  let content = column([
    title.into(),
    Space::new().height(spacing::SPACE_1).into(),
    version.into(),
    Space::new().height(spacing::SPACE_1).into(),
    build_info.into(),
    Space::new().height(spacing::SPACE_3).into(),
    separator.into(),
    Space::new().height(spacing::SPACE_3).into(),
    license.into(),
    Space::new().height(spacing::SPACE_1).into(),
    github_link.into(),
  ])
  .align_x(Horizontal::Center);

  container(content)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(panel_style)
    .into()
}

fn text_primary_style(_: &iced::Theme) -> iced::widget::text::Style {
  iced::widget::text::Style {
    color: Some(color::text::PRIMARY),
  }
}

fn text_secondary_style(_: &iced::Theme) -> iced::widget::text::Style {
  iced::widget::text::Style {
    color: Some(color::text::SECONDARY),
  }
}

fn text_tertiary_style(_: &iced::Theme) -> iced::widget::text::Style {
  iced::widget::text::Style {
    color: Some(color::text::TERTIARY),
  }
}

fn text_accent_style(_: &iced::Theme) -> iced::widget::text::Style {
  iced::widget::text::Style {
    color: Some(color::text::ACCENT),
  }
}

fn separator_style(_: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::border::DEFAULT)),
    ..container::Style::default()
  }
}

fn transparent_button_style(_: &iced::Theme, _: button::Status) -> button::Style {
  button::Style {
    background: None,
    border: Border::default(),
    ..button::Style::default()
  }
}

fn panel_style(_: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    ..container::Style::default()
  }
}
