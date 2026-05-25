//! Column header text cell for the contracts table.

use iced::{
  Element, Length, Padding, Theme,
  alignment::Horizontal,
  widget::{container, text},
};

use super::Message;
use crate::style::{color, spacing, typography::mono};

const ROW_PAD_H: f32 = spacing::SPACE_4;

/// Builder for a column header text cell.
pub struct Component {
  align_right: bool,
  label: String,
  width: Length,
}

impl Component {
  /// Creates a new header cell component.
  pub fn new(label: impl Into<String>, width: impl Into<Length>, align_right: bool) -> Self {
    Self {
      align_right,
      label: label.into(),
      width: width.into(),
    }
  }

  /// Renders the header cell into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let t = text(self.label.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      });
    let inner: Element<'static, Message> = if self.align_right {
      container(t).width(Length::Fill).align_x(Horizontal::Right).into()
    } else {
      t.into()
    };
    container(inner)
      .width(self.width)
      .padding(Padding {
        top: 10.0,
        bottom: 10.0,
        left: ROW_PAD_H,
        right: ROW_PAD_H,
      })
      .into()
  }
}
