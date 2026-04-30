use iced::{
  Element, Length, Padding,
  widget::{container, row, text},
};

use crate::style::{color, spacing, typography};

pub fn view(eve_time: &str) -> Element<'static, super::Message> {
  container(
    row([
      text("EVE")
        .font(typography::mono::REGULAR)
        .size(10.0)
        .style(|_| text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
      text(eve_time.to_owned())
        .font(typography::mono::MEDIUM)
        .size(10.0)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: 14.0,
    right: 14.0,
  })
  .height(Length::Fill)
  .align_y(iced::alignment::Vertical::Center)
  .into()
}
