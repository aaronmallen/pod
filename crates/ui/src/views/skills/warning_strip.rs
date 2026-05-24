//! Low-queue warning banner.

use iced::{
  Background, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text},
};

use super::Message;
use crate::style::{color, spacing, typography::body};

pub struct Component {
  low_queue: bool,
}

impl Component {
  pub fn new(low_queue: bool) -> Self {
    Self {
      low_queue,
    }
  }

  pub fn render(self) -> Option<Element<'static, Message>> {
    if !self.low_queue {
      return None;
    }

    let bg_color = color::accent::GOLD_FAINT;
    let border_color = color::accent::GOLD_MUTED;

    let row_items: Vec<Element<'_, Message>> = vec![
      text("⚠")
        .size(14.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::status::CAUTION),
        })
        .into(),
      row([
        text("Queue under 24 hours.")
          .font(body::MEDIUM)
          .size(13.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        text(" Skills queued past 24h continue training; add longer skills so progress doesn't pause.")
          .font(body::REGULAR)
          .size(13.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
      ])
      .into(),
      suggest_btn(),
    ];

    let strip = container(row(row_items).spacing(14.0).align_y(Vertical::Center))
      .width(Length::Fill)
      .padding(Padding {
        top: 10.0,
        bottom: 10.0,
        left: spacing::SPACE_7,
        right: spacing::SPACE_7,
      })
      .style(move |_| container::Style {
        background: Some(Background::Color(bg_color)),
        ..container::Style::default()
      });

    let bottom = container(Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .height(1.0)
      .style(move |_| container::Style {
        background: Some(Background::Color(border_color)),
        ..container::Style::default()
      });

    Some(column([strip.into(), bottom.into()]).width(Length::Fill).into())
  }
}

fn suggest_btn() -> Element<'static, Message> {
  use iced::Border;
  button(
    text("Suggest skills")
      .font(body::MEDIUM)
      .size(12.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::status::CAUTION),
      }),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .style(|_, _| button::Style {
    background: None,
    border: Border {
      color: color::accent::GOLD_DIM,
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: color::status::CAUTION,
    ..button::Style::default()
  })
  .into()
}
