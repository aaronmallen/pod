use iced::{
  Element, Length, Padding,
  widget::{column, container, text},
};

use crate::style::{color, spacing, typography};

pub struct Component {
  title: String,
  subtitle: String,
}

impl Component {
  pub fn new(title: impl ToString, subtitle: impl ToString) -> Self {
    Self {
      title: title.to_string(),
      subtitle: subtitle.to_string(),
    }
  }

  pub fn render<'a, MSG: 'static>(self) -> Element<'a, MSG> {
    let title_text = text(self.title)
      .font(typography::body::MEDIUM)
      .size(22.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      });

    let subtitle_text = text(self.subtitle.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      });

    container(
      column([title_text.into(), subtitle_text.into()])
        .spacing(4.0)
        .padding(Padding {
          top: 0.0,
          bottom: 0.0,
          left: spacing::SPACE_8,
          right: spacing::SPACE_8,
        }),
    )
    .width(Length::Fill)
    .into()
  }
}
