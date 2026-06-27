use iced::{
  Background, Border, Element, Padding,
  alignment::Vertical,
  widget::{Row, button, text},
};

use super::Message;
use crate::ui::{
  components::icon::Icon,
  style::{color, radius, spacing, typography},
};

pub fn from_selected_button<'a>(count: usize) -> Element<'a, Message> {
  let label = Row::with_children(vec![
    text(t!("skills.panel_plans.from_selected"))
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
    Icon::chevron_right()
      .size(12.0)
      .color(color::accent::PLASMA)
      .render::<Message>(),
    text(count.to_string())
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  button(label)
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .on_press(Message::FromSelected)
    .style(|_, status| {
      let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: Some(Background::Color(color::with_alpha(
          color::accent::PLASMA,
          if hover { 0.2 } else { 0.12 },
        ))),
        border: Border {
          color: color::with_alpha(color::accent::PLASMA, if hover { 0.6 } else { 0.4 }),
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        text_color: color::accent::PLASMA,
        ..button::Style::default()
      }
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_renders_the_count() {
    let _el: Element<'_, Message> = from_selected_button(4);
  }
}
