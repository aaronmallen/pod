use iced::{
  Background, Border, Element, Padding,
  widget::{container, text},
};

use crate::style::{color, radius, spacing, typography};

pub struct Component {
  value: usize,
}

impl Component {
  pub fn new(value: usize) -> Self {
    Self {
      value,
    }
  }

  pub fn render<'a, MSG: 'static>(self) -> Element<'a, MSG> {
    let label = text(self.value.to_string())
      .font(typography::mono::MEDIUM)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      });

    container(label)
      .padding(Padding {
        top: spacing::SPACE_1,
        bottom: spacing::SPACE_1,
        left: spacing::SPACE_2,
        right: spacing::SPACE_2,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA_SUBTLE)),
        border: Border {
          radius: radius::FULL.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  }
}
