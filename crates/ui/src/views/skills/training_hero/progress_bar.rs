//! Thin plasma progress bar for the training hero panel.

use iced::{
  Background, Element, Length,
  widget::{Space, container, row},
};

use super::super::Message;
use crate::style::color;

pub struct Component {
  progress: f32,
}

impl Component {
  pub fn new(progress: f32) -> Self {
    Self {
      progress,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    container(
      row([
        container(Space::new().width(Length::Fill).height(3.0))
          .width(Length::FillPortion((self.progress * 1000.0) as u16))
          .height(3.0)
          .style(|_| container::Style {
            background: Some(Background::Color(color::accent::PLASMA)),
            ..container::Style::default()
          })
          .into(),
        Space::new()
          .width(Length::FillPortion((1000.0 - self.progress * 1000.0) as u16))
          .height(3.0)
          .into(),
      ])
      .height(3.0)
      .spacing(0.0),
    )
    .width(Length::Fill)
    .height(3.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into()
  }
}
