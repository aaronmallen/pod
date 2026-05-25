use iced::{
  Border, Element, Length, Padding,
  widget::{container, row, text},
};

use super::Message;
use crate::style::{color, typography as font};

pub struct DropdownHeader;

impl DropdownHeader {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    container(
      row([
        text("Switch character")
          .font(font::mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        iced::widget::Space::new().width(Length::Fill).into(),
      ])
      .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 14.0,
      right: 14.0,
    })
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
  }
}

impl Default for DropdownHeader {
  fn default() -> Self {
    Self::new()
  }
}

pub struct CorpSectionHeader;

impl CorpSectionHeader {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    container(
      row([
        text("Corporations")
          .font(font::mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        iced::widget::Space::new().width(Length::Fill).into(),
      ])
      .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 14.0,
      right: 14.0,
    })
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
  }
}

impl Default for CorpSectionHeader {
  fn default() -> Self {
    Self::new()
  }
}
