use iced::{Element, Length, widget::column};

use crate::components::PopOver;

const MENU_WIDTH: f32 = 220.0;

pub struct Component<'a, MSG> {
  items: Vec<Element<'a, MSG>>,
  x: f32,
  y: f32,
}

impl<'a, MSG: 'static> Component<'a, MSG> {
  pub fn new(items: Vec<Element<'a, MSG>>) -> Self {
    Self {
      items,
      x: 0.0,
      y: 0.0,
    }
  }

  pub fn position(mut self, x: f32, y: f32) -> Self {
    self.x = x;
    self.y = y;
    self
  }

  pub fn render(self) -> Element<'a, MSG> {
    PopOver::new(column(self.items).width(Length::Fixed(MENU_WIDTH)))
      .position(self.x, self.y)
      .render()
  }
}
