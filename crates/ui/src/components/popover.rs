//! Generic anchor-plus-overlay popover component.

use iced::{Element, widget::stack};

/// A generic overlay component that renders an anchor element and,
/// when open, stacks a floating content element on top of it.
pub struct Component<'a, Message> {
  /// The element rendered as the trigger or reference point.
  anchor: Element<'a, Message>,
  /// The element overlaid on top of the anchor when `is_open` is true.
  content: Element<'a, Message>,
  /// Controls whether the overlay content is visible.
  is_open: bool,
}

impl<'a, Message: 'a> Component<'a, Message> {
  /// Creates a new [`Component`] with the given anchor, content, and
  /// visibility state.
  pub fn new(anchor: impl Into<Element<'a, Message>>, content: impl Into<Element<'a, Message>>, is_open: bool) -> Self {
    Self {
      anchor: anchor.into(),
      content: content.into(),
      is_open,
    }
  }

  /// Consumes the builder and returns an [`Element`].
  ///
  /// The anchor is always rendered. When `is_open` is `true`, the
  /// content element is layered on top via an iced `stack`.
  pub fn render(self) -> Element<'a, Message> {
    if self.is_open {
      stack([self.anchor, self.content]).into()
    } else {
      self.anchor
    }
  }
}
