//! Individual folder button with unread badge.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{button, row, text},
};

use super::{Folder, Message};
use crate::{
  components::{CountBadge, Icon},
  style::{color, typography::body},
};

fn folder_icon(folder: &Folder, active: bool) -> Element<'static, Message> {
  let color = if active {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };
  match folder {
    Folder::Archive => Icon::archive(),
    Folder::Drafts => Icon::draft(),
    Folder::Inbox => Icon::inbox(),
    Folder::Sent => Icon::send(),
    Folder::Snoozed => Icon::snooze(),
    Folder::Starred => Icon::star(),
    Folder::Trash => Icon::trash(),
    _ => Icon::inbox(),
  }
  .size(16.0)
  .color(color)
  .render::<Message>()
}

fn folder_row_content(
  icon_el: Element<'static, Message>,
  label: &'static str,
  is_active: bool,
  count_el: Element<'static, Message>,
) -> iced::widget::Row<'static, Message> {
  row([
    icon_el,
    text(label)
      .font(if is_active { body::MEDIUM } else { body::REGULAR })
      .size(13.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(if is_active {
          color::text::PRIMARY
        } else {
          color::text::STRONG
        }),
      })
      .width(Length::Fill)
      .into(),
    count_el,
  ])
  .spacing(10.0)
  .align_y(iced::alignment::Vertical::Center)
}

/// Builder for an individual folder row button.
pub struct Component {
  count: u32,
  folder: Folder,
  is_active: bool,
  is_all_inboxes: bool,
  label: &'static str,
  total_unread: u32,
  unread: u32,
}

impl Component {
  /// Create a new folder row button.
  pub fn new(label: &'static str, is_active: bool, count: u32, unread: u32, folder: Folder) -> Self {
    Self {
      count,
      folder,
      is_active,
      is_all_inboxes: false,
      label,
      total_unread: 0,
      unread,
    }
  }

  /// Mark this as the "All Inboxes" unified button.
  pub fn all_inboxes(mut self, total_unread: u32) -> Self {
    self.is_all_inboxes = true;
    self.total_unread = total_unread;
    self
  }

  /// Render into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let is_active = self.is_active;
    if self.is_all_inboxes {
      self.render_all_inboxes()
    } else {
      let icon_el = folder_icon(&self.folder, is_active);
      let count_el = CountBadge::new(self.count).unread(self.unread).render();
      button(folder_row_content(icon_el, self.label, is_active, count_el))
        .padding(Padding {
          top: 7.0,
          bottom: 7.0,
          left: 10.0,
          right: 10.0,
        })
        .width(Length::Fill)
        .on_press(Message::FolderSelected(self.folder))
        .style(move |_, status| button::Style {
          background: if is_active {
            Some(Background::Color(color::accent::PLASMA_SUBTLE))
          } else {
            match status {
              button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
              _ => None,
            }
          },
          border: Border {
            color: Color::TRANSPARENT,
            radius: 6.0.into(),
            width: 0.0,
          },
          text_color: color::text::PRIMARY,
          ..button::Style::default()
        })
        .into()
    }
  }

  fn render_all_inboxes(self) -> Element<'static, Message> {
    let active = self.is_active;
    let total_unread = self.total_unread;
    button(
      row([
        Icon::inbox_all()
          .size(16.0)
          .color(if active {
            color::accent::PLASMA
          } else {
            color::text::SECONDARY
          })
          .render::<Message>(),
        text("All Inboxes")
          .font(body::MEDIUM)
          .size(13.0)
          .style(move |_: &Theme| iced::widget::text::Style {
            color: Some(if active {
              color::text::PRIMARY
            } else {
              color::text::SECONDARY
            }),
          })
          .width(Length::Fill)
          .into(),
        CountBadge::new(0).unread(total_unread).render(),
      ])
      .spacing(10.0)
      .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 9.0,
      bottom: 9.0,
      left: 10.0,
      right: 10.0,
    })
    .width(Length::Fill)
    .on_press(Message::FolderSelected(Folder::All))
    .style(move |_, status| button::Style {
      background: if active {
        Some(Background::Color(color::accent::PLASMA_SUBTLE))
      } else {
        match status {
          button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::SUBTLE_FILL)),
          _ => Some(Background::Color(color::state::HOVER_OVERLAY)),
        }
      },
      border: Border {
        color: if active {
          color::accent::PLASMA_BORDER
        } else {
          color::border::SUBTLE
        },
        radius: 6.0.into(),
        width: 1.0,
      },
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    })
    .into()
  }
}

/// A label row for label-type folders.
pub struct LabelRow<'a> {
  folder: Folder,
  is_active: bool,
  label: &'a str,
}

impl<'a> LabelRow<'a> {
  /// Create a new label row.
  pub fn new(label: &'a str, is_active: bool, folder: Folder) -> Self {
    Self {
      folder,
      is_active,
      label,
    }
  }

  /// Render into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let is_active = self.is_active;
    button(
      text(self.label)
        .font(body::REGULAR)
        .size(13.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(if is_active {
            color::text::PRIMARY
          } else {
            color::text::STRONG
          }),
        })
        .width(Length::Fill),
    )
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: 36.0,
      right: 10.0,
    })
    .width(Length::Fill)
    .on_press(Message::FolderSelected(self.folder))
    .style(move |_, status| button::Style {
      background: if is_active {
        Some(Background::Color(color::accent::PLASMA_SUBTLE))
      } else {
        match status {
          button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
          _ => None,
        }
      },
      border: Border {
        color: Color::TRANSPARENT,
        radius: 6.0.into(),
        width: 0.0,
      },
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    })
    .into()
  }
}

/// Build a vec of label rows from the message list.
pub fn label_rows<'a>(messages: &'a [super::MailMessage], selected: &'a Folder) -> Vec<Element<'a, Message>> {
  let mut seen = std::collections::BTreeSet::new();
  for m in messages {
    for l in &m.labels {
      seen.insert(l.clone());
    }
  }
  seen
    .into_iter()
    .map(|l| {
      let is_active = *selected == Folder::Label(l.clone());
      let folder = Folder::Label(l.clone());
      LabelRow::new(&*Box::leak(l.into_boxed_str()), is_active, folder).render()
    })
    .collect()
}

fn matches_folder_type(m: &super::MailMessage, folder: &Folder) -> bool {
  match folder {
    Folder::All | Folder::Inbox => m.folder == "inbox",
    Folder::Archive => m.folder == "archive",
    Folder::Drafts => m.folder == "drafts",
    Folder::Label(l) => m.labels.contains(l),
    Folder::Sent => m.folder == "sent",
    Folder::Snoozed => m.snoozed.is_some(),
    Folder::Starred => m.starred,
    Folder::Trash => m.folder == "trash",
  }
}

fn message_in_folder_count(m: &super::MailMessage, account_id: i64, folder: &Folder) -> bool {
  if !passes_account_scope(m, folder, account_id) {
    return false;
  }
  matches_folder_type(m, folder)
}

fn passes_account_scope(m: &super::MailMessage, folder: &Folder, account_id: i64) -> bool {
  matches!(folder, Folder::All) || m.character_id == account_id
}

pub fn folder_counts(messages: &[super::MailMessage], account_id: i64, folder: &Folder) -> (u32, u32) {
  messages
    .iter()
    .filter(|m| message_in_folder_count(m, account_id, folder))
    .fold((0u32, 0u32), |(t, u), m| (t + 1, u + u32::from(m.unread)))
}
