//! Single message preview row (avatar, subject, preview, time).

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, image, mouse_area, row, text},
};

use super::{
  super::{MailMessage, State, snooze_picker},
  Message,
};
use crate::{
  components::{
    Icon, Separator,
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

fn subject_prefix_icon(msg: &MailMessage) -> Option<Element<'_, Message>> {
  if msg.pinned {
    Some(Icon::pin().size(12.0).color(color::status::CAUTION).render::<Message>())
  } else if msg.starred {
    Some(
      Icon::star()
        .size(12.0)
        .color(color::status::CAUTION)
        .render::<Message>(),
    )
  } else {
    None
  }
}

fn subject_text_style(unread: bool) -> (iced::font::Font, iced::Color) {
  if unread {
    (body::MEDIUM, color::text::PRIMARY)
  } else {
    (body::REGULAR, color::text::STRONG)
  }
}

fn wrap_subject_with_icon<'a>(
  icon: Element<'a, Message>,
  subject_text: iced::widget::Text<'a>,
) -> Element<'a, Message> {
  row([icon, subject_text.width(Length::Fill).into()])
    .spacing(6.0)
    .align_y(iced::alignment::Vertical::Center)
    .into()
}

fn message_subject_row<'a>(msg: &'a MailMessage) -> Element<'a, Message> {
  let (subject_font, subject_color) = subject_text_style(msg.unread);
  let subject_text = text(&msg.subject)
    .font(subject_font)
    .size(13.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(subject_color),
    });
  match subject_prefix_icon(msg) {
    None => subject_text.into(),
    Some(icon) => wrap_subject_with_icon(icon, subject_text),
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
    background: Some(Background::Color(color::accent::PLASMA_SELECTED)),
    border: Border {
      color: color::state::SELECTION,
      radius: 3.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn sender_name_text<'a>(msg: &'a MailMessage) -> iced::widget::Text<'a> {
  let unread = msg.unread;
  text(&msg.from_name)
    .font(if unread { body::MEDIUM } else { body::REGULAR })
    .size(13.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(if unread {
        color::text::PRIMARY
      } else {
        color::text::SECONDARY
      }),
    })
    .width(Length::Fill)
}

fn message_time_display(msg: &MailMessage) -> String {
  msg
    .snoozed
    .as_deref()
    .map(snooze_picker::format_snooze_expiry)
    .unwrap_or_else(|| msg.time.clone())
}

fn message_body_col<'a>(msg: &'a MailMessage) -> Element<'a, Message> {
  let name_row = row([
    sender_name_text(msg).into(),
    text(message_time_display(msg))
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
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
  portrait_handle: Option<image::Handle>,
  selected: bool,
}

impl<'a> Component<'a> {
  /// Create a new message row builder.
  pub fn new(msg: &'a MailMessage, selected: bool, state: &'a State) -> Self {
    let portrait_handle = msg.from_id.and_then(|id| state.portrait_handles.get(&id).cloned());
    Self {
      msg,
      portrait_handle,
      selected,
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
          Some(Background::Color(color::accent::PLASMA_SELECTED))
        } else {
          match status {
            button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
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
