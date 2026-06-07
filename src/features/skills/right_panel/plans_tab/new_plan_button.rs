use iced::{
  Background, Border, Element, Padding,
  widget::{button, text},
};

use super::Message;
use crate::ui::style::{color, radius, spacing, typography};

pub fn new_plan_button<'a>() -> Element<'a, Message> {
  button(
    text("New plan")
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::NewPlan)
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(if hover {
        color::with_alpha(color::accent::PLASMA, 0.12)
      } else {
        iced::Color::TRANSPARENT
      })),
      border: Border {
        color: color::accent::PLASMA_MUTED,
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: color::accent::PLASMA,
      ..button::Style::default()
    }
  })
  .into()
}
