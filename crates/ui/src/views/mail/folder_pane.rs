//! Left sidebar: account picker + folder list.

pub mod folder_row;
pub mod group_header;
pub mod unified_section;

use iced::{
  Background, Element, Length,
  widget::{Space, column, container, scrollable},
};

use super::{Folder, MailMessage, State};
use crate::style::color;

/// Messages produced by the folder pane.
#[derive(Clone, Debug)]
pub enum Message {
  FolderSelected(Folder),
}

fn folder_labels_opt_section<'a>(messages: &'a [MailMessage], selected: &'a Folder) -> Option<Element<'a, Message>> {
  let rows = folder_row::label_rows(messages, selected);
  if rows.is_empty() {
    None
  } else {
    Some(group_header::Component::new("Labels", rows).render())
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

    let unified_section = unified_section::Component::new(all_inboxes_btn, state.accounts.len()).render();
    let folders_section = group_header::Component::new("Folders", standard_folder_rows(state)).render();
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
