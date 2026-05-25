//! Resizable-pane drag handle for the wallet right-rail divider.

use iced::{
  Background, Element, Length,
  widget::{Space, container, mouse_area, row},
};

use crate::{
  style::color,
  views::wallet::{DraggingPane, Message},
};

fn handle_inner() -> Element<'static, Message> {
  row([
    Space::new().width(1.5).height(Length::Fill).into(),
    container(Space::new().width(1.0).height(Length::Fill))
      .width(1.0)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      })
      .into(),
    Space::new().width(1.5).height(Length::Fill).into(),
  ])
  .width(4.0)
  .height(Length::Fill)
  .into()
}

/// Builder for the right-rail pane drag handle.
pub struct DragHandle {
  pane: DraggingPane,
}

impl DragHandle {
  /// Create a new drag handle builder targeting the right-rail pane.
  pub fn new() -> Self {
    Self {
      pane: DraggingPane::RightRail,
    }
  }

  /// Set which pane this handle drags.
  pub fn pane(mut self, pane: DraggingPane) -> Self {
    self.pane = pane;
    self
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'static, Message> {
    mouse_area(handle_inner())
      .on_press(Message::PaneDragStart(self.pane))
      .interaction(iced::mouse::Interaction::ResizingHorizontally)
      .into()
  }
}

impl Default for DragHandle {
  fn default() -> Self {
    Self::new()
  }
}
