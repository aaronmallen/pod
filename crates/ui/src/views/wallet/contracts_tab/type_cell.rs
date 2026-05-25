//! Contract type label cell for a contracts table row.

use iced::{
  Element, Length, Padding, Theme,
  alignment::Vertical,
  widget::{container, text},
};

use super::Message;
use crate::style::{color, spacing, typography::mono};

const ROW_PAD_H: f32 = spacing::SPACE_4;
const COL_TYPE: f32 = 120.0;

fn type_label_for(kind: &str) -> String {
  match kind {
    "item_exchange" => "Item Exchange".to_string(),
    "courier" => "Courier".to_string(),
    "auction" => "Auction".to_string(),
    other => other.replace('_', " "),
  }
}

/// Builder for the contract type label cell.
pub struct Component {
  kind: String,
}

impl Component {
  /// Creates a new type cell component.
  pub fn new(kind: impl Into<String>) -> Self {
    Self {
      kind: kind.into(),
    }
  }

  /// Renders the type cell into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    container(
      text(type_label_for(&self.kind).to_uppercase())
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .width(COL_TYPE)
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
