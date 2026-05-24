//! Left sidebar: account picker + folder list.

pub mod folder_row;

use iced::{
  Background, Element, Length, Padding, Theme,
  widget::{Space, column, container, scrollable, text},
};

use super::{Folder, MailMessage, State};
use crate::{
  components::{self, section_label},
  style::{color, typography::mono},
};

/// Messages produced by the folder pane.
#[derive(Clone, Debug)]
pub enum Message {
  FolderSelected(Folder),
}

fn folder_unified_section<'a>(all_inboxes_btn: Element<'a, Message>, account_count: usize) -> Element<'a, Message> {
  let mailbox_label = text(format!("{account_count} mailboxes combined"))
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    });
  let inner = container(
    column([
      section_label("Unified"),
      all_inboxes_btn,
      Space::new().height(8.0).into(),
      mailbox_label.into(),
    ])
    .spacing(0.0),
  )
  .padding(Padding {
    top: 16.0,
    bottom: 14.0,
    left: 16.0,
    right: 16.0,
  })
  .width(Length::Fill);
  column([inner.into(), components::Separator::horizontal().render()]).into()
}

fn folder_named_section<'a>(title: &'static str, rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
  container(column([
    section_label(title),
    Space::new().height(10.0).into(),
    column(rows).spacing(1.0).into(),
  ]))
  .padding(Padding {
    top: 20.0,
    bottom: 8.0,
    left: 16.0,
    right: 16.0,
  })
  .width(Length::Fill)
  .into()
}

fn folder_labels_opt_section<'a>(messages: &'a [MailMessage], selected: &'a Folder) -> Option<Element<'a, Message>> {
  let rows = folder_row::label_rows(messages, selected);
  if rows.is_empty() {
    None
  } else {
    Some(folder_named_section("Labels", rows))
  }
}

fn standard_folder_rows<'a>(state: &'a State) -> Vec<Element<'a, Message>> {
  let folder_defs: &[(Folder, &'static str)] = &[
    (Folder::Inbox, "Inbox"),
    (Folder::Starred, "Starred"),
    (Folder::Snoozed, "Snoozed"),
    (Folder::Sent, "Sent"),
    (Folder::Drafts, "Drafts"),
    (Folder::Archive, "Archive"),
    (Folder::Trash, "Trash"),
  ];
  folder_defs
    .iter()
    .map(|(folder, label)| {
      let (count, unread) = folder_row::folder_counts(&state.messages, state.current_account_id(), folder);
      let is_active = &state.selected_folder == folder;
      folder_row::Component::new(label, is_active, count, unread, folder.clone()).render()
    })
    .collect()
}

/// Builder for the folder sidebar pane.
pub struct Component<'a> {
  state: &'a State,
  width: f32,
}

impl<'a> Component<'a> {
  /// Create a new folder pane builder.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
      width: 240.0,
    }
  }

  /// Set the pane width.
  pub fn width(mut self, width: f32) -> Self {
    self.width = width;
    self
  }

  /// Render into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let total_unread: u32 = state.accounts.iter().map(|a| a.unread).sum();
    let all_active = state.selected_folder == Folder::All;

    let all_inboxes_btn = folder_row::Component::new("All Inboxes", all_active, 0, 0, Folder::All)
      .all_inboxes(total_unread)
      .render();

    let unified_section = folder_unified_section(all_inboxes_btn, state.accounts.len());
    let folders_section = folder_named_section("Folders", standard_folder_rows(state));
    let labels_section = folder_labels_opt_section(&state.messages, &state.selected_folder);

    let mut sidebar_children: Vec<Element<'_, Message>> = vec![unified_section, folders_section];
    if let Some(ls) = labels_section {
      sidebar_children.push(ls);
    }
    sidebar_children.push(Space::new().height(Length::Fill).into());

    container(scrollable(column(sidebar_children).width(Length::Fill)).height(Length::Fill))
      .width(Length::Fixed(self.width))
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        ..container::Style::default()
      })
      .into()
  }
}
