//! From/to/subject/time/labels header block.

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Space, column, container, image, row, text},
};

use super::{
  super::{MailMessage, State},
  Message,
};
use crate::{
  components::{
    Separator,
    avatar::{self, AvatarKind},
  },
  style::{
    color,
    typography::{body, mono},
  },
};

fn sender_system_suffix(from_system: bool) -> Element<'static, Message> {
  if from_system {
    text(" · System message")
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into()
  } else {
    Space::new().width(0.0).into()
  }
}

fn sender_meta_col<'a>(msg: &'a MailMessage, to_name: &'a str) -> Element<'a, Message> {
  column([
    text(&msg.from_name)
      .font(body::MEDIUM)
      .size(15.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    row([
      text("to ")
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      text(to_name)
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::STRONG),
        })
        .into(),
      sender_system_suffix(msg.from_system),
    ])
    .into(),
  ])
  .spacing(4.0)
  .width(Length::Fill)
  .into()
}

/// Builder for the message header block (sender info).
pub struct Component<'a> {
  msg: &'a MailMessage,
  to_name: &'a str,
  portrait_handle: Option<image::Handle>,
}

impl<'a> Component<'a> {
  /// Create a new message header builder.
  pub fn new(msg: &'a MailMessage, to_name: &'a str, state: &'a State) -> Self {
    let portrait_handle = msg.from_id.and_then(|id| state.portrait_handles.get(&id).cloned());
    Self {
      msg,
      to_name,
      portrait_handle,
    }
  }

  /// Render into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let msg = self.msg;
    let avatar_kind = if msg.from_system {
      AvatarKind::System
    } else if msg.from_corp {
      AvatarKind::Corp
    } else {
      AvatarKind::Person
    };
    let portrait = avatar::Component::new(&msg.from_name, msg.from_tone, 44.0, avatar_kind)
      .portrait(self.portrait_handle)
      .render();
    let sender_meta = sender_meta_col(msg, self.to_name);
    let time_el = text(&msg.time)
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      });
    let inner = container(
      row([portrait, sender_meta, time_el.into()])
        .spacing(14.0)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 0.0,
      bottom: 24.0,
      left: 0.0,
      right: 0.0,
    })
    .width(Length::Fill);
    column([inner.into(), Separator::horizontal().render()]).into()
  }
}

/// Build the labels row for the reading pane.
pub fn labels_row<'a>(msg: &'a MailMessage) -> Vec<Element<'a, Message>> {
  let mut items: Vec<Element<'_, Message>> = msg.labels.iter().map(|l| label_chip(l)).collect();
  if msg.important {
    items.push(priority_chip());
  }
  items
}

fn label_chip(l: &str) -> Element<'_, Message> {
  container(
    text(l.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 8.0,
    right: 8.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::state::HOVER_OVERLAY)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 3.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn priority_chip() -> Element<'static, Message> {
  container(
    text("PRIORITY")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::status::DANGER),
      }),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 8.0,
    right: 8.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::status::DANGER_FAINT)),
    border: Border {
      color: color::status::DANGER_BORDER,
      radius: 3.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}
