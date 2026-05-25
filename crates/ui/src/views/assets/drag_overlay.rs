//! Invisible full-screen mouse capture layer shown while dragging the pane divider.

use iced::{
  Element, Length,
  widget::{Space, mouse_area},
};

use super::Message;

/// Builder for the drag-capture overlay shown during pane resize.
pub struct DragOverlay {
  /// Whether the drag is currently active.
  active: bool,
}

impl DragOverlay {
  /// Creates a new overlay builder.
  pub fn new(active: bool) -> Self {
    Self {
      active,
    }
  }

  /// Returns the overlay element when the drag is active, or `None` otherwise.
  pub fn render(self) -> Option<Element<'static, Message>> {
    if !self.active {
      return None;
    }
    Some(
      mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
        .on_move(|pt| Message::PaneDrag(pt.x))
        .on_release(Message::PaneDragEnd)
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .into(),
    )
  }
}
