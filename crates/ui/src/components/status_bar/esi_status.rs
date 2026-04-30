use iced::{
  Background, Border, Color, Element, Length, Padding,
  widget::{container, row, text},
};

use crate::style::{color, spacing, typography};

pub fn view(esi_connected: bool) -> Element<'static, super::Message> {
  let dot_color = if esi_connected {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let dot = container(iced::widget::Space::new().width(6.0).height(6.0))
    .width(6.0)
    .height(6.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(dot_color)),
      border: Border {
        radius: 3.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  container(
    row([
      dot.into(),
      text("ESI")
        .font(typography::mono::REGULAR)
        .size(10.0)
        .style(|_| text::Style {
          color: Some(Color::from_rgba(0.957, 0.949, 0.925, 0.45)),
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
