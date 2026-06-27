use iced::{
  Border, Element, Padding,
  widget::{button, text},
};

use super::Message;
use crate::ui::style::{color, radius, spacing, typography};

pub fn from_queue_button<'a>() -> Element<'a, Message> {
  button(
    text(t!("skills.panel_plans.from_queue"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::FromQueue)
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: None,
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, if hover { 0.25 } else { 0.1 }),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: if hover {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}
