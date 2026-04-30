use iced::{
  Element, Length, Padding,
  widget::{column, container, text},
};

use crate::style::{color, spacing, typography};

pub struct Component<'a> {
  location_name: Option<&'a str>,
}

impl<'a> Component<'a> {
  pub fn new(location_name: Option<&'a str>) -> Self {
    Self {
      location_name,
    }
  }

  pub fn render<MSG: 'a>(self) -> Element<'a, MSG> {
    let value = self.location_name.unwrap_or("Unknown");

    container(
      column([
        text("LOCATION")
          .font(typography::mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        text(value)
          .font(typography::body::REGULAR)
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
    .width(Length::FillPortion(2))
    .into()
  }
}
