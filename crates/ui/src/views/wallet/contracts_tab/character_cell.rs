//! Character portrait and name cell for a contracts table row.

use iced::{
  Element, Length, Padding, Theme,
  alignment::Vertical,
  widget::{Space, container, row, text},
};

use super::{Message, portrait_chip::Component as PortraitChip};
use crate::{
  style::{color, spacing, typography::body},
  views::wallet::WalletCharacter,
};

const ROW_PAD_H: f32 = spacing::SPACE_4;
const COL_CHARACTER: f32 = 148.0;

/// Builder for the character portrait and name cell.
pub struct Component<'a> {
  char_info: Option<&'a WalletCharacter>,
}

impl<'a> Component<'a> {
  /// Creates a new character cell component.
  pub fn new(char_info: Option<&'a WalletCharacter>) -> Self {
    Self {
      char_info,
    }
  }

  /// Renders the character cell into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let inner: Element<'_, Message> = match self.char_info {
      Some(c) => row([
        PortraitChip::new(&c.name, c.portrait_tone)
          .handle(c.portrait_handle.as_ref())
          .render(),
        Space::new().width(8.0).into(),
        text(&c.name)
          .font(body::REGULAR)
          .size(12.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      ])
      .align_y(Vertical::Center)
      .into(),
      None => Space::new().width(Length::Fill).into(),
    };
    container(inner)
      .width(COL_CHARACTER)
      .height(Length::Fill)
      .align_y(Vertical::Center)
      .clip(true)
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: ROW_PAD_H,
        right: ROW_PAD_H,
      })
      .into()
  }
}
