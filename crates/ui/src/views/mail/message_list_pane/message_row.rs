//! Single message preview row (avatar, subject, preview, time).

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, image, mouse_area, row, text},
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
    color, radius,
    typography::{body, mono},
  },
};

fn message_portrait_stack<'a>(msg: &'a MailMessage, portrait_handle: Option<image::Handle>) -> Element<'a, Message> {
  let avatar_kind = if msg.from_system {
    AvatarKind::System
  } else if msg.from_corp {
    AvatarKind::Corp
  } else {
    AvatarKind::Person
  };
  let portrait = avatar::Component::new(&msg.from_name, msg.from_tone, 36.0, avatar_kind)
    .portrait(portrait_handle)
    .render();
  if msg.unread {
    let unread_dot = container(Space::new().width(10.0).height(10.0))
      .width(10.0)
      .height(10.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA)),
        border: Border {
          radius: radius::FULL.into(),
          ..Border::default()
        },
        ..container::Style::default()
      });
    iced::widget::stack([
      portrait,
      container(unread_dot)
        .width(36.0)
        .height(36.0)
        .align_x(iced::alignment::Horizontal::Left)
        .align_y(iced::alignment::Vertical::Top)
        .into(),
    ])
    .width(36.0)
    .height(36.0)
    .into()
  } else {
    portrait
  }
}

fn message_subject_row<'a>(msg: &'a MailMessage) -> Element<'a, Message> {
  let mut prefix: Vec<Element<'_, Message>> = Vec::new();
  if msg.pinned {
    prefix.push(
      text("⊕")
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::status::CAUTION),
        })
        .into(),
    );
  } else if msg.starred {
    prefix.push(
      text("★")
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::status::CAUTION),
        })
        .into(),
    );
  }
  let subject_font = if msg.unread { body::MEDIUM } else { body::REGULAR };
  let subject_color = if msg.unread {
    color::text::PRIMARY
  } else {
    Color::from_rgba(0.957, 0.949, 0.925, 0.75)
  };
  if prefix.is_empty() {
    text(&msg.subject)
      .font(subject_font)
      .size(13.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(subject_color),
      })
      .into()
  } else {
    prefix.push(
      text(&msg.subject)
        .font(subject_font)
        .size(13.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(subject_color),
        })
        .width(Length::Fill)
        .into(),
    );
    row(prefix)
      .spacing(6.0)
      .align_y(iced::alignment::Vertical::Center)
      .into()
  }
}

fn message_meta_chips<'a>(msg: &'a MailMessage) -> Vec<Element<'a, Message>> {
  let mut items: Vec<Element<'_, Message>> = msg.labels.iter().map(|l| meta_label_chip(l)).collect();
  if msg.has_attachment {
    items.push(attachment_chip());
  }
  items
}

fn meta_label_chip(l: &str) -> Element<'_, Message> {
  container(
    text(l.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 6.0,
    right: 6.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.05))),
    border: Border {
      color: color::border::SUBTLE,
      radius: 3.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn attachment_chip() -> Element<'static, Message> {
  container(
    text("+1")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 6.0,
    right: 6.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(Color::from_rgba(0.247, 0.722, 0.859, 0.08))),
    border: Border {
      color: Color::from_rgba(0.247, 0.722, 0.859, 0.30),
      radius: 3.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn message_body_col<'a>(msg: &'a MailMessage) -> Element<'a, Message> {
  let unread = msg.unread;
  let name_text = text(&msg.from_name)
    .font(if unread { body::MEDIUM } else { body::REGULAR })
    .size(13.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(if unread {
        color::text::PRIMARY
      } else {
        Color::from_rgba(0.957, 0.949, 0.925, 0.85)
      }),
    })
    .width(Length::Fill);
  let time_text = text(&msg.time)
    .font(mono::REGULAR)
    .size(10.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });
  let name_row = row([name_text.into(), time_text.into()])
    .align_y(iced::alignment::Vertical::Bottom)
    .spacing(8.0);
  let preview_text = text(&msg.preview)
    .font(body::REGULAR)
    .size(12.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
    .wrapping(iced::widget::text::Wrapping::Word);
  let meta_items = message_meta_chips(msg);
  let mut body_children: Vec<Element<'_, Message>> =
    vec![name_row.into(), message_subject_row(msg), preview_text.into()];
  if !meta_items.is_empty() {
    body_children.push(Space::new().height(5.0).into());
    body_children.push(row(meta_items).spacing(4.0).wrap().into());
  }
  column(body_children).spacing(2.0).width(Length::Fill).into()
}

/// Builder for a single message preview row.
pub struct Component<'a> {
  msg: &'a MailMessage,
  selected: bool,
  portrait_handle: Option<image::Handle>,
}

impl<'a> Component<'a> {
  /// Create a new message row builder.
  pub fn new(msg: &'a MailMessage, selected: bool, state: &'a State) -> Self {
    let portrait_handle = msg.from_id.and_then(|id| state.portrait_handles.get(&id).cloned());
    Self {
      msg,
      selected,
      portrait_handle,
    }
  }

  /// Render into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let selected = self.selected;
    let id = self.msg.id.clone();
    let right_click_id = id.clone();
    let inner = row([
      message_portrait_stack(self.msg, self.portrait_handle),
      message_body_col(self.msg),
    ])
    .spacing(12.0)
    .width(Length::Fill);

    let btn = button(inner)
      .padding(Padding {
        top: 12.0,
        bottom: 12.0,
        left: 16.0,
        right: 16.0,
      })
      .width(Length::Fill)
      .on_press(Message::MessageSelected(id))
      .style(move |_, status| button::Style {
        background: if selected {
          Some(Background::Color(Color::from_rgba(0.247, 0.722, 0.859, 0.08)))
        } else {
          match status {
            button::Status::Hovered | button::Status::Pressed => {
              Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.03)))
            }
            _ => None,
          }
        },
        border: Border::default(),
        text_color: color::text::PRIMARY,
        ..button::Style::default()
      });
    let row_with_sep = column([btn.into(), Separator::horizontal().render()]);
    mouse_area(row_with_sep)
      .on_right_press(Message::MessageRightClicked(right_click_id))
      .into()
  }
}
