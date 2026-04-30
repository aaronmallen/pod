use iced::{Background, Element, Length, widget::container};

use crate::style::color;

pub struct Component {
  vertical: bool,
}

impl Component {
  pub fn horizontal() -> Self {
    Self {
      vertical: false,
    }
  }

  pub fn vertical() -> Self {
    Self {
      vertical: true,
    }
  }

  pub fn render<'a, MSG: 'static>(self) -> Element<'a, MSG> {
    if self.vertical {
      container(iced::widget::Space::new().width(1.0).height(Length::Fill))
        .width(1.0)
        .height(Length::Fill)
        .style(|_| container::Style {
          background: Some(Background::Color(color::border::SUBTLE)),
          ..container::Style::default()
        })
        .into()
    } else {
      container(iced::widget::Space::new().width(Length::Fill).height(1.0))
        .width(Length::Fill)
        .height(1.0)
        .style(|_| container::Style {
          background: Some(Background::Color(color::border::SUBTLE)),
          ..container::Style::default()
        })
        .into()
    }
  }
}
