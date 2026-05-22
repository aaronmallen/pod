//! Right pane: full message view.

pub mod action_bar;
pub mod attachment_row;
pub mod message_body;
pub mod message_header;

use iced::{
  Background, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, scrollable, stack, text},
};

use super::{MailMessage, State};
use crate::style::{color, typography::body};

/// Messages produced by the reading pane.
#[derive(Clone, Debug)]
pub enum Message {
  ArchivePressed,
  CheckSnoozed,
  DeletePressed,
  ForwardPressed,
  ReplyAllPressed,
  ReplyPressed,
  SnoozeFailed(String),
  SnoozeSet(String),
  SnoozedExpired(Vec<(i64, i64)>),
  SnoozeToggle,
  StarToggle,
}

fn reading_scrollable_content<'a>(msg: &'a MailMessage, to_name: &'a str, state: &'a State) -> Element<'a, Message> {
  let labels = message_header::labels_row(msg);
  let subject_el = text(&msg.subject)
    .font(body::MEDIUM)
    .size(26.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    })
    .wrapping(iced::widget::text::Wrapping::Word);
  let sender_block = message_header::Component::new(msg, to_name, state).render();
  let body_paras = message_body::Component::render(msg);

  let mut content_children: Vec<Element<'_, Message>> = Vec::new();
  if !labels.is_empty() {
    content_children.push(row(labels).spacing(6.0).wrap().into());
    content_children.push(Space::new().height(16.0).into());
  }
  content_children.push(subject_el.into());
  content_children.push(Space::new().height(24.0).into());
  content_children.push(sender_block);
  content_children.push(Space::new().height(28.0).into());
  content_children.extend(
    body_paras
      .into_iter()
      .flat_map(|p| [p, Space::new().height(18.0).into()]),
  );
  if msg.has_attachment {
    content_children.push(Space::new().height(14.0).into());
    content_children.push(attachment_row::Component::render());
  }
  let content_col = column(content_children).width(Length::Fixed(720.0));
  scrollable(container(content_col).center_x(Length::Fill).padding(Padding {
    top: 32.0,
    bottom: 48.0,
    left: 48.0,
    right: 48.0,
  }))
  .height(Length::Fill)
  .width(Length::Fill)
  .into()
}

fn reading_pane_content<'a>(
  msg: &'a MailMessage,
  to_name: &'a str,
  snooze_open: bool,
  state: &'a State,
) -> Element<'a, Message> {
  let snooze_label = msg
    .snoozed
    .as_deref()
    .map(|s| format!("Until {s}"))
    .unwrap_or_else(|| "Snooze".to_string());
  let toolbar = action_bar::Component::new(
    msg.starred,
    msg.snoozed.is_some(),
    &snooze_label,
    &msg.date_label,
    &msg.time,
  )
  .render();
  let scrollable_content = reading_scrollable_content(msg, to_name, state);
  let base: Element<'_, Message> = container(column([toolbar, scrollable_content.into()]))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into();
  if snooze_open {
    stack([base, super::snooze_picker::Component::new(msg).render()]).into()
  } else {
    base
  }
}

/// Builder for the reading pane.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Create a new reading pane builder.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Render into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let msg = state
      .selected_message_id
      .as_ref()
      .and_then(|id| state.messages.iter().find(|m| &m.id == id));

    match msg {
      None => container(
        text("Select a message")
          .font(body::REGULAR)
          .size(14.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::BASE)),
        ..container::Style::default()
      })
      .into(),

      Some(msg) => {
        let to_name: &str = if msg.folder == "sent" && !msg.recipients_display.is_empty() {
          &msg.recipients_display
        } else {
          state
            .accounts
            .iter()
            .find(|a| a.id == msg.character_id)
            .map(|a| a.name.as_str())
            .unwrap_or("me")
        };
        reading_pane_content(msg, to_name, state.snooze_popover_open, state)
      }
    }
  }
}
