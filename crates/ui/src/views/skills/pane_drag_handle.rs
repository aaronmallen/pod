//! Drag handle between the left queue pane and the right skill panel.

use iced::{
  Background, Element, Length,
  widget::{Space, container, mouse_area},
};

use super::Message;

pub struct Component;

impl Component {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    mouse_area(
      container(Space::new().width(4.0).height(Length::Fill))
        .width(4.0)
        .height(Length::Fill)
        .style(|_| container::Style {
          background: Some(Background::Color(crate::style::color::border::SUBTLE)),
          ..container::Style::default()
        }),
    )
    .on_press(Message::PaneDragStart)
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .into()
  }
}
