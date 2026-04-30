//! Middle pane: scrollable message list grouped by day.

pub mod day_header;
pub mod message_row;

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{column, container, mouse_area, row, scrollable, text, text_input},
};

use super::{Folder, MailMessage, State};
use crate::{
  components::{Icon, Separator},
  style::{color, typography::body},
};

/// Messages produced by the message list pane.
#[derive(Clone, Debug)]
pub enum Message {
  ContextMenuClose,
  CursorMoved(f32, f32),
  MessageRightClicked(String),
  MessageSelected(String),
  SearchChanged(String),
}

fn filter_folder_messages<'a>(state: &'a State) -> Vec<&'a MailMessage> {
  state
    .messages
    .iter()
    .filter(|m| match &state.selected_folder {
      Folder::All => true,
      _ => m.character_id == state.current_account_id(),
    })
    .filter(|m| match &state.selected_folder {
      Folder::All => m.folder == "inbox",
      Folder::Inbox => m.folder == "inbox",
      Folder::Starred => m.starred,
      Folder::Snoozed => m.snoozed.is_some(),
      Folder::Sent => m.folder == "sent",
      Folder::Drafts => m.folder == "drafts",
      Folder::Archive => m.folder == "archive",
      Folder::Trash => m.folder == "trash",
      Folder::Label(l) => m.labels.contains(l),
    })
    .filter(|m| {
      if state.search_query.is_empty() {
        return true;
      }
      let q = state.search_query.to_lowercase();
      m.subject.to_lowercase().contains(&q)
        || m.from_name.to_lowercase().contains(&q)
        || m.preview.to_lowercase().contains(&q)
    })
    .collect()
}

fn list_search_bar<'a>(query: &'a str) -> Element<'a, Message> {
  let inner = container(
    row([
      Icon::search()
        .size(16.0)
        .color(color::text::SECONDARY)
        .render::<Message>(),
      text_input("Search mail", query)
        .on_input(Message::SearchChanged)
        .font(body::REGULAR)
        .size(13.0)
        .style(|_, _| text_input::Style {
          background: iced::Background::Color(Color::TRANSPARENT),
          border: Border::default(),
          icon: color::text::SECONDARY,
          placeholder: color::text::TERTIARY,
          value: color::text::PRIMARY,
          selection: Color::from_rgba(0.247, 0.722, 0.859, 0.30),
        })
        .width(Length::Fill)
        .into(),
    ])
    .spacing(10.0)
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 14.0,
    bottom: 14.0,
    left: 16.0,
    right: 16.0,
  })
  .width(Length::Fill);
  column([inner.into(), Separator::horizontal().render()]).into()
}

fn list_empty_state(query: &str) -> Element<'_, Message> {
  container(
    text(format!("No messages match \"{}\".", query))
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(32.0)
  .width(Length::Fill)
  .center_x(Length::Fill)
  .into()
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
      list_rows.push(list_empty_state(&state.search_query));
    }

    let list = scrollable(column(list_rows).width(Length::Fill)).height(Length::Fill);

    let pane = container(column([list_search_bar(&state.search_query).into(), list.into()]).width(Length::Fill))
      .width(Length::Fixed(self.width))
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::BASE)),
        ..container::Style::default()
      });

    mouse_area(pane).on_move(|pt| Message::CursorMoved(pt.x, pt.y)).into()
  }
}
