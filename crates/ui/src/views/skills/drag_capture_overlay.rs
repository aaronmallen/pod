//! Full-screen mouse-capture overlay active while dragging the pane divider.

use iced::{
  Element, Length,
  widget::{Space, mouse_area},
};

use super::Message;

pub struct Component;

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}

impl Component {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
      .on_move(|pt| Message::PaneDrag(pt.x))
      .on_release(Message::PaneDragEnd)
      .interaction(iced::mouse::Interaction::ResizingHorizontally)
      .into()
  }
}
