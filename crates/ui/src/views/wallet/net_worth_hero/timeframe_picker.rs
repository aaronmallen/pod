//! Timeframe picker button row (1W / 1M / 3M / 6M / 1Y).

use iced::{
  Background, Border, Color, Element, Padding, Theme,
  widget::{button, container, row, text},
};

use crate::{
  style::{color, typography::mono},
  views::wallet::{Message, Timeframe},
};

/// Builder for the timeframe picker.
pub struct Component<'a> {
  active: &'a Timeframe,
}

impl<'a> Component<'a> {
  /// Creates a new timeframe picker for the given active timeframe.
  pub fn new(active: &'a Timeframe) -> Self {
    Self {
      active,
    }
  }

  /// Renders the timeframe picker into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let btns: Vec<Element<'_, Message>> = Timeframe::all()
      .iter()
      .map(|tf| {
        let is_active = tf == self.active;
        let msg = Message::TimeframeChanged(tf.clone());
        button(
          text(tf.label())
            .font(mono::MEDIUM)
            .size(10.0)
            .style(move |_: &Theme| iced::widget::text::Style {
              color: Some(if is_active {
                color::accent::PLASMA
              } else {
                color::text::SECONDARY
              }),
            }),
        )
        .padding(Padding {
          top: 6.0,
          bottom: 6.0,
          left: 10.0,
          right: 10.0,
        })
        .on_press(msg)
        .style(move |_, status| button::Style {
          background: if is_active {
            Some(Background::Color(Color::from_rgba(0.247, 0.722, 0.859, 0.12)))
          } else {
            match status {
              button::Status::Hovered | button::Status::Pressed => {
                Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.04)))
              }
              _ => None,
            }
          },
          border: Border {
            color: color::border::SUBTLE,
            radius: 0.0.into(),
            width: 0.0,
          },
          text_color: if is_active {
            color::accent::PLASMA
          } else {
            color::text::SECONDARY
          },
          ..button::Style::default()
        })
        .into()
      })
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
