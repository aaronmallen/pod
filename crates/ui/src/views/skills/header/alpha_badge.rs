//! Alpha clone indicator badge.

use iced::{
  Background, Border, Element, Padding,
  widget::{container, text},
};

use crate::style::{color, typography::mono};

pub struct Component;

impl Component {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, super::super::Message> {
    container(
      text("α  24h queue cap · ½ training rate")
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding {
      top: 6.0,
      bottom: 6.0,
      left: 10.0,
      right: 10.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::state::HOVER_OVERLAY)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 4.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}
