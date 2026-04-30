//! Individual folder button with unread badge.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{button, row, text},
};

use super::{Folder, Message};
use crate::{
  components::CountBadge,
  style::{
    color,
    typography::{body, mono},
  },
};

fn folder_row_content(
  icon: &'static str,
  label: &'static str,
  is_active: bool,
  count_el: Element<'static, Message>,
) -> iced::widget::Row<'static, Message> {
  row([
    text(icon)
      .font(mono::REGULAR)
      .size(14.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(if is_active {
          color::accent::PLASMA
        } else {
          color::text::SECONDARY
        }),
      })
      .width(18.0)
      .into(),
    text(label)
      .font(if is_active { body::MEDIUM } else { body::REGULAR })
      .size(13.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(if is_active {
          color::text::PRIMARY
        } else {
          Color::from_rgba(0.957, 0.949, 0.925, 0.75)
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
  label: &'static str,
  icon: &'static str,
  is_active: bool,
  count: u32,
  unread: u32,
  folder: Folder,
  is_all_inboxes: bool,
  total_unread: u32,
}

impl Component {
  /// Create a new folder row button.
  pub fn new(
    label: &'static str,
    icon: &'static str,
    is_active: bool,
    count: u32,
    unread: u32,
    folder: Folder,
  ) -> Self {
    Self {
      label,
      icon,
      is_active,
      count,
      unread,
      folder,
      is_all_inboxes: false,
      total_unread: 0,
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
      let count_el = CountBadge::new(self.count).unread(self.unread).render();
      button(folder_row_content(self.icon, self.label, is_active, count_el))
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
              button::Status::Hovered | button::Status::Pressed => {
                Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.04)))
              }
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
        crate::components::Icon::mail()
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
              Color::from_rgba(0.957, 0.949, 0.925, 0.85)
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
          button::Status::Hovered | button::Status::Pressed => {
            Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.06)))
          }
          _ => Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.03))),
        }
      },
      border: Border {
        color: if active {
          Color::from_rgba(0.247, 0.722, 0.859, 0.35)
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
  label: &'a str,
  is_active: bool,
  folder: Folder,
}

impl<'a> LabelRow<'a> {
  /// Create a new label row.
  pub fn new(label: &'a str, is_active: bool, folder: Folder) -> Self {
    Self {
      label,
      is_active,
      folder,
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
            Color::from_rgba(0.957, 0.949, 0.925, 0.75)
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
          button::Status::Hovered | button::Status::Pressed => {
            Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.04)))
          }
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

pub fn folder_counts(messages: &[super::MailMessage], account_id: i64, folder: &Folder) -> (u32, u32) {
  messages
    .iter()
    .filter(|m| match folder {
      Folder::All => true,
      _ => m.character_id == account_id,
    })
    .fold((0u32, 0u32), |(t, u), m| {
      let matches = match folder {
        Folder::All => m.folder == "inbox",
        Folder::Inbox => m.folder == "inbox",
        Folder::Starred => m.starred,
        Folder::Snoozed => m.snoozed.is_some(),
        Folder::Sent => m.folder == "sent",
        Folder::Drafts => m.folder == "drafts",
        Folder::Archive => m.folder == "archive",
        Folder::Trash => m.folder == "trash",
        Folder::Label(l) => m.labels.contains(l),
      };
      if matches {
        (t + 1, u + u32::from(m.unread))
      } else {
        (t, u)
      }
    })
}

pub fn folder_icon_char(folder: &str) -> &'static str {
  match folder {
    "Inbox" => "▤",
    "Starred" => "★",
    "Snoozed" => "◷",
    "Sent" => "▶",
    "Drafts" => "◧",
    "Archive" => "▣",
    "Trash" => "▥",
    _ => "·",
  }
}
