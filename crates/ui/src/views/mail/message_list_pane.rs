//! Middle pane: scrollable message list grouped by day.

pub mod day_header;
pub mod empty_state;
pub mod message_row;
pub mod search_bar;

use iced::{
  Background, Element, Length,
  widget::{column, container, mouse_area, scrollable},
};

use super::{Folder, MailMessage, State};
use crate::style::color;

/// Messages produced by the message list pane.
#[derive(Clone, Debug)]
pub enum Message {
  ContextMenuClose,
  CursorMoved(f32, f32),
  MessageRightClicked(String),
  MessageSelected(String),
  SearchChanged(String),
}

fn message_passes_folder_filter(m: &MailMessage, folder: &Folder, account_id: i64) -> bool {
  if !passes_account_filter(m, folder, account_id) {
    return false;
  }
  passes_folder_type(m, folder)
}

fn passes_account_filter(m: &MailMessage, folder: &Folder, account_id: i64) -> bool {
  matches!(folder, Folder::All) || m.character_id == account_id
}

fn passes_folder_type(m: &MailMessage, folder: &Folder) -> bool {
  match folder {
    Folder::All | Folder::Inbox => m.folder == "inbox" && m.snoozed.is_none(),
    Folder::Archive => m.folder == "archive",
    Folder::Drafts => m.folder == "drafts",
    Folder::Label(l) => m.labels.contains(l),
    Folder::Sent => m.folder == "sent",
    Folder::Snoozed => m.snoozed.is_some(),
    Folder::Starred => m.starred,
    Folder::Trash => m.folder == "trash",
  }
}

fn message_passes_search(m: &MailMessage, query: &str) -> bool {
  if query.is_empty() {
    return true;
  }
  let q = query.to_lowercase();
  m.subject.to_lowercase().contains(&q)
    || m.from_name.to_lowercase().contains(&q)
    || m.preview.to_lowercase().contains(&q)
}

fn filter_folder_messages(state: &State) -> Vec<&MailMessage> {
  let account_id = state.current_account_id();
  state
    .messages
    .iter()
    .filter(|m| message_passes_folder_filter(m, &state.selected_folder, account_id))
    .filter(|m| message_passes_search(m, &state.search_query))
    .collect()
}

/// Builder for the message list middle pane.
pub struct Component<'a> {
  state: &'a State,
  width: f32,
}

impl<'a> Component<'a> {
  /// Create a new message list builder.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
      width: 380.0,
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
    let visible = filter_folder_messages(state);

    let mut grouped: Vec<(String, Vec<&MailMessage>)> = Vec::new();
    for msg in &visible {
      if let Some(last) = grouped.last_mut()
        && last.0 == msg.date_label
      {
        last.1.push(msg);
        continue;
      }
      grouped.push((msg.date_label.clone(), vec![msg]));
    }

    let mut list_rows: Vec<Element<'_, Message>> = Vec::new();
    for (day, msgs) in grouped {
      list_rows.push(day_header::Component::new(day).render());
      for msg in msgs {
        let selected = state.selected_message_id.as_deref() == Some(msg.id.as_str());
        list_rows.push(message_row::Component::new(msg, selected, state).render());
      }
    }

    if visible.is_empty() && !state.search_query.is_empty() {
      list_rows.push(empty_state::Component::new(&state.search_query).render());
    }

    let list = scrollable(column(list_rows).width(Length::Fill)).height(Length::Fill);

    let pane =
      container(column([search_bar::Component::new(&state.search_query).render(), list.into()]).width(Length::Fill))
        .width(Length::Fixed(self.width))
        .height(Length::Fill)
        .style(|_| container::Style {
          background: Some(Background::Color(color::surface::BASE)),
          ..container::Style::default()
        });

    mouse_area(pane).on_move(|pt| Message::CursorMoved(pt.x, pt.y)).into()
  }
}
