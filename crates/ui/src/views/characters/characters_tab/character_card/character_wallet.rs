use iced::{
  Element, Length, Padding,
  widget::{column, container, text},
};
use pod_model::Character;

use crate::style::{color, spacing, typography};

pub struct Component<'a> {
  character: &'a Character,
}

impl<'a> Component<'a> {
  pub fn new(character: &'a Character) -> Self {
    Self {
      character,
    }
  }

  pub fn render<MSG: 'a>(self) -> Element<'a, MSG> {
    container(
      column([
        text("ISK")
          .font(typography::mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        text(self.character.isk_formatted())
          .font(typography::mono::MEDIUM)
          .size(13.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
      ])
      .spacing(2.0),
    )
    .padding(Padding {
      top: 12.0,
      bottom: 12.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::FillPortion(1))
    .into()
  }
}
