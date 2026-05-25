//! Timeframe picker button row (1W / 1M / 3M / 6M / 1Y).

use iced::{
  Background, Border, Color, Element, Padding, Theme,
  widget::{button, container, row, text},
};

use crate::{
  style::{color, typography::mono},
  views::wallet::{Message, Timeframe},
};

fn timeframe_text_color(is_active: bool) -> Color {
  if is_active {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  }
}

fn timeframe_button_background(is_active: bool, status: button::Status) -> Option<Background> {
  if is_active {
    return Some(Background::Color(color::accent::PLASMA_HIGHLIGHT));
  }
  match status {
    button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
    _ => None,
  }
}

fn timeframe_button_style(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| button::Style {
    background: timeframe_button_background(is_active, status),
    border: Border {
      color: color::border::SUBTLE,
      radius: 0.0.into(),
      width: 0.0,
    },
    text_color: timeframe_text_color(is_active),
    ..button::Style::default()
  }
}

fn timeframe_button(tf: &Timeframe, is_active: bool) -> Element<'_, Message> {
  let msg = Message::TimeframeChanged(tf.clone());
  button(
    text(tf.label())
      .font(mono::MEDIUM)
      .size(10.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(timeframe_text_color(is_active)),
      }),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(msg)
  .style(timeframe_button_style(is_active))
  .into()
}

/// Builder for the timeframe picker.
pub struct Component<'a> {
  active: &'a Timeframe,
}

impl<'a> Component<'a> {
  pub fn new(active: &'a Timeframe) -> Self {
    Self {
      active,
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    let btns: Vec<Element<'_, Message>> = Timeframe::all()
      .iter()
      .map(|tf| timeframe_button(tf, tf == self.active))
      .collect();
    container(row(btns))
      .style(|_| container::Style {
        border: Border {
          color: color::border::SUBTLE,
          radius: 6.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }
}
