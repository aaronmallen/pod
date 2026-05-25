//! Stat cell component for skill header stats.

use iced::{
  Color, Element, Length,
  widget::{Space, column, text},
};

use super::super::super::Message;
use crate::style::{color, typography::mono};

/// A small two-row cell showing a label and a coloured value.
pub struct StatCell {
  label: String,
  value: String,
  value_color: Color,
}

impl StatCell {
  pub fn new(label: impl Into<String>, value: impl Into<String>, value_color: Color) -> Self {
    Self {
      label: label.into(),
      value: value.into(),
      value_color,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let label_el = text(self.label.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      });

    let value_color = self.value_color;
    let value_el = text(self.value)
      .font(mono::MEDIUM)
      .size(15.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(value_color),
      });

    column([label_el.into(), Space::new().height(4.0).into(), value_el.into()])
      .width(Length::Shrink)
      .into()
  }
}
