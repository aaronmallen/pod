//! Outermost container shell that applies the surface background colour.

use iced::{Background, Element, Length, widget::container};

use super::Message;
use crate::style::color;

pub struct Component<'a> {
  content: Element<'a, Message>,
}

impl<'a> Component<'a> {
  pub fn new(content: impl Into<Element<'a, Message>>) -> Self {
    Self {
      content: content.into(),
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    container(self.content)
      .width(Length::Fill)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::BASE)),
        ..container::Style::default()
      })
      .into()
  }
}
