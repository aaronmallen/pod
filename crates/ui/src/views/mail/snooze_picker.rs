//! Snooze dropdown overlay with time presets.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  alignment::Vertical,
  widget::{button, column, container, row, text},
};

use super::{MailMessage, reading_pane::Message};
use crate::style::{
  color,
  typography::{body, mono},
};

fn snooze_preset_rows() -> Vec<Element<'static, Message>> {
  let presets: &[(&str, &str)] = &[
    ("Later today", "18:00 EVE"),
    ("Tomorrow", "09:00 EVE"),
    ("After downtime", "11:30 EVE"),
    ("Next week", "Mon 09:00"),
  ];
  presets
    .iter()
    .map(|(label, hint)| {
      let label_str = label.to_string();
      button(
        row([
          text(*label)
            .font(body::REGULAR)
            .size(13.0)
            .style(|_: &Theme| iced::widget::text::Style {
              color: Some(color::text::PRIMARY),
            })
            .width(Length::Fill)
            .into(),
          text(*hint)
            .font(mono::REGULAR)
            .size(10.0)
            .style(|_: &Theme| iced::widget::text::Style {
              color: Some(color::text::SECONDARY),
            })
            .into(),
        ])
        .align_y(Vertical::Center),
      )
      .width(Length::Fill)
      .padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 10.0,
        right: 10.0,
      })
      .on_press(Message::SnoozeSet(label_str))
      .style(|_, status| button::Style {
        background: match status {
          button::Status::Hovered | button::Status::Pressed => {
            Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.05)))
          }
          _ => None,
        },
        border: Border {
          radius: 6.0.into(),
          ..Border::default()
        },
        text_color: color::text::PRIMARY,
        ..button::Style::default()
      })
      .into()
    })
    .collect()
}

fn snooze_unsnooze_btn() -> Element<'static, Message> {
  button(
    text("Unsnooze")
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::status::DANGER),
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(Message::SnoozeSet(String::new()))
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(Color::from_rgba(0.878, 0.459, 0.349, 0.08)))
      }
      _ => None,
    },
    border: Border {
      radius: 6.0.into(),
      ..Border::default()
    },
    text_color: color::status::DANGER,
    ..button::Style::default()
  })
  .into()
}

/// Builder for the snooze dropdown overlay.
pub struct Component<'a> {
  msg: &'a MailMessage,
}

impl<'a> Component<'a> {
  /// Create a new snooze picker for the given message.
  pub fn new(msg: &'a MailMessage) -> Self {
    Self {
      msg,
    }
  }

  /// Render into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let header_row: Element<'_, Message> = container(text("SNOOZE UNTIL").font(mono::REGULAR).size(9.0).style(
      |_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      },
    ))
    .padding(Padding {
      top: 8.0,
      bottom: 6.0,
      left: 10.0,
      right: 10.0,
    })
    .into();

    let mut dropdown_children: Vec<Element<'_, Message>> = vec![header_row];
    dropdown_children.extend(snooze_preset_rows());
    if self.msg.snoozed.is_some() {
      dropdown_children.push(
        container(iced::widget::Space::new().width(Length::Fill).height(1.0))
          .width(Length::Fill)
          .style(|_| container::Style {
            background: Some(Background::Color(color::border::SUBTLE)),
            ..container::Style::default()
          })
          .into(),
      );
      dropdown_children.push(snooze_unsnooze_btn());
    }

    let dropdown = crate::components::Card::new(column(dropdown_children).width(Length::Fixed(240.0)))
      .padding(6.0)
      .render();

    container(dropdown)
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(iced::alignment::Horizontal::Left)
      .align_y(iced::alignment::Vertical::Top)
      .padding(Padding {
        top: 50.0,
        left: 310.0,
        bottom: 0.0,
        right: 0.0,
      })
      .into()
  }
}
