//! Label + colored value stat row for the wallet right rail.

use iced::{
  Color, Element, Length, Padding, Theme,
  widget::{container, row, text},
};

use crate::{
  style::{color, typography::mono},
  views::wallet::Message,
};

/// Builder for a summary stat row showing a label and a colored value.
pub struct Component {
  label: &'static str,
  value: String,
  value_color: Color,
}

impl Component {
  /// Creates a new summary stat row component.
  pub fn new(label: &'static str, value: String, value_color: Color) -> Self {
    Self {
      label,
      value,
      value_color,
    }
  }

  /// Renders the summary stat row into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let value_color = self.value_color;
    container(
      row([
        text(self.label.to_uppercase())
          .font(mono::REGULAR)
          .size(10.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .width(Length::Fill)
          .into(),
        text(self.value)
          .font(mono::MEDIUM)
          .size(10.0)
          .style(move |_: &Theme| iced::widget::text::Style {
            color: Some(value_color),
          })
          .into(),
      ])
      .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 6.0,
      bottom: 6.0,
      left: 20.0,
      right: 20.0,
    })
    .width(Length::Fill)
    .into()
  }
}
