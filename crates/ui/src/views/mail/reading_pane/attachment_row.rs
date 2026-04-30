//! Attachment indicator row.

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, text},
};

use super::Message;
use crate::style::{
  color,
  typography::{body, mono},
};

fn attachment_icon_box() -> Element<'static, Message> {
  container(
    text("PNG")
      .font(mono::MEDIUM)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .width(36.0)
  .height(36.0)
  .center_x(Length::Fill)
  .center_y(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::accent::PLASMA_SUBTLE)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn attachment_file_info() -> Element<'static, Message> {
  column([
    text("contracts-jita.png")
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().height(2.0).into(),
    text("248 KB")
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .into()
}

fn attachment_card() -> Element<'static, Message> {
  crate::components::Card::new(
    row([
      attachment_icon_box(),
      Space::new().width(12.0).into(),
      attachment_file_info(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 16.0,
    right: 16.0,
  })
  .render()
}

/// Builder for the attachment block in the reading pane.
pub struct Component;

impl Component {
  /// Render the attachment block.
  pub fn render<'a>() -> Element<'a, Message> {
    let header = container(
      text("ATTACHMENTS · 1")
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding {
      top: 0.0,
      bottom: 12.0,
      left: 0.0,
      right: 0.0,
    });

    let card = attachment_card();
    let rule = container(Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      });

    container(column([
      rule.into(),
      Space::new().height(24.0).into(),
      header.into(),
      card,
    ]))
    .into()
  }
}
