//! Empty queue idle state component.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, column, container, row, text},
};

use super::super::Message;
use crate::style::{
  color, spacing,
  typography::{body, mono},
};

pub struct Component;

impl Component {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    container(
      row([
        icon(),
        Space::new().width(spacing::SPACE_6).into(),
        container(text_col()).width(Length::Fill).into(),
      ])
      .align_y(Vertical::Center),
    )
    .padding(Padding {
      top: 24.0,
      bottom: 24.0,
      left: spacing::SPACE_7,
      right: spacing::SPACE_7,
    })
    .into()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}

fn icon() -> Element<'static, Message> {
  container(text("⚠").size(20.0).style(|_| iced::widget::text::Style {
    color: Some(color::status::DANGER),
  }))
  .width(48.0)
  .height(48.0)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(|_| container::Style {
    background: Some(Background::Color(Color::from_rgba(0.878, 0.459, 0.349, 0.10))),
    border: Border {
      color: Color::from_rgba(0.878, 0.459, 0.349, 0.35),
      radius: 24.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn text_col() -> Element<'static, Message> {
  column([
    text("Training paused · queue empty")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::status::DANGER),
      })
      .into(),
    Space::new().height(6.0).into(),
    text("No skill is currently training")
      .font(body::MEDIUM)
      .size(22.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().height(4.0).into(),
    text("Drop a skill into the queue from the browser, or apply a plan.")
      .font(body::REGULAR)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .into()
}
