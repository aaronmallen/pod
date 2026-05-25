//! Status badge cell for a contracts table row.

use iced::{Element, Length, Padding, alignment::Vertical, widget::container};

use super::Message;
use crate::{
  components::StatusBadge,
  style::spacing,
  views::wallet::{ContractEntry, mappings},
};

const ROW_PAD_H: f32 = spacing::SPACE_4;
const COL_STATUS: f32 = 130.0;

/// Builder for the status badge cell.
pub struct Component<'a> {
  entry: &'a ContractEntry,
}

impl<'a> Component<'a> {
  /// Creates a new status cell component.
  pub fn new(entry: &'a ContractEntry) -> Self {
    Self {
      entry,
    }
  }

  /// Renders the status cell into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let badge = StatusBadge::new(
      mappings::status_color_for(&self.entry.status),
      mappings::status_label_for(&self.entry.status),
    );
    container(badge.render())
      .width(COL_STATUS)
      .height(Length::Fill)
      .align_y(Vertical::Center)
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: ROW_PAD_H,
        right: ROW_PAD_H,
      })
      .into()
  }
}
