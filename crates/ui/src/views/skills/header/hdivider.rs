//! Vertical hair-line separator used between header stat cells.

use iced::{
  Background, Element,
  widget::{Space, container},
};

use super::super::Message;
use crate::style::color;

pub struct HDivider;

impl Default for HDivider {
  fn default() -> Self {
    Self::new()
  }
}

impl HDivider {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    container(Space::new().width(1.0).height(44.0))
      .width(1.0)
      .height(44.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      })
      .into()
  }
}
