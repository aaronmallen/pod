use iced::{Color, Element, Length, Padding, widget::container};

use crate::{components, style::color};

pub struct Component {
  size: f32,
  color: Color,
}

impl Component {
  pub fn new() -> Self {
    Self {
      size: 14.0,
      color: color::text::SECONDARY,
    }
  }

  pub fn size(mut self, size: f32) -> Self {
    self.size = size;
    self
  }

  pub fn color(mut self, color: Color) -> Self {
    self.color = color;
    self
  }

  pub fn render<'a, MSG: 'static>(self) -> Element<'a, MSG> {
    container(
      components::Icon::search()
        .size(self.size)
        .color(self.color)
        .render::<MSG>(),
    )
    .center_y(Length::Fill)
    .padding(Padding {
      left: 10.0,
      right: 6.0,
      ..Padding::ZERO
    })
    .into()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}
