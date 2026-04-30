use iced::{
  Background, Border, Color, Element, Padding,
  widget::{Id, text_input},
};

use crate::style::{color, typography};

#[derive(Clone, Debug)]
pub enum Message {
  Changed(String),
}

pub struct Component<'a> {
  value: &'a str,
  id: Option<Id>,
  placeholder: &'a str,
}

impl<'a> Component<'a> {
  pub fn new(value: &'a str) -> Self {
    Self {
      value,
      id: None,
      placeholder: "Search… try tag:pvp or corp:caldari",
    }
  }

  pub fn id(mut self, id: Id) -> Self {
    self.id = Some(id);
    self
  }

  pub fn placeholder(mut self, placeholder: &'a str) -> Self {
    self.placeholder = placeholder;
    self
  }

  pub fn render(self) -> Element<'a, Message> {
    let mut widget = text_input(self.placeholder, self.value)
      .on_input(Message::Changed)
      .font(typography::body::REGULAR)
      .size(13.0)
      .style(|_, _| text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        icon: color::text::SECONDARY,
        placeholder: color::text::TERTIARY,
        value: color::text::PRIMARY,
        selection: Color::from_rgba(0.247, 0.722, 0.859, 0.30),
      })
      .padding(Padding::ZERO);

    if let Some(id) = self.id {
      widget = widget.id(id);
    }

    widget.into()
  }
}
