use iced::{
  Element, Padding,
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
        text(self.character.name())
          .font(typography::body::MEDIUM)
          .size(17.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        text(self.character.corp_name().to_uppercase())
          .font(typography::mono::REGULAR)
          .size(10.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      ])
      .spacing(2.0),
    )
    .padding(Padding {
      top: 14.0,
      bottom: 10.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .into()
  }
}
