//! Full-screen container that positions the character-picker dropdown.

use iced::{Element, Length, Padding, widget::container};

use super::Message;
use crate::{components::CharacterPicker, style::spacing};

/// Builder for the picker-dropdown overlay.
pub struct PickerOverlay<'a> {
  /// The character picker whose dropdown should be shown.
  picker: &'a CharacterPicker,
}

impl<'a> PickerOverlay<'a> {
  /// Creates a new overlay builder wrapping the given picker.
  pub fn new(picker: &'a CharacterPicker) -> Self {
    Self {
      picker,
    }
  }

  /// Returns the overlay element when the picker is open, or `None` otherwise.
  pub fn render(self) -> Option<Element<'a, Message>> {
    if !self.picker.is_open {
      return None;
    }
    let dropdown = self.picker.dropdown().map(Message::Picker);
    Some(
      container(dropdown)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Left)
        .padding(Padding {
          top: spacing::layout::HEADER_HEIGHT + 8.0,
          left: spacing::SPACE_8,
          ..Padding::ZERO
        })
        .into(),
    )
  }
}
