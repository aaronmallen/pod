//! Reading content component: scrollable body with subject, sender, and body paragraphs.

use iced::{
  Element, Length, Padding, Theme,
  widget::{Space, column, container, row, scrollable, text},
};

use super::{action_bar, attachment_row, message_body, message_header};
use crate::{
  style::{color, typography::body},
  views::mail::{MailMessage, State, reading_pane::Message},
};

/// Builder for the full reading pane content (toolbar + scrollable body).
pub struct Component<'a> {
  msg: &'a MailMessage,
  snooze_open: bool,
  state: &'a State,
  to_name: &'a str,
}

impl<'a> Component<'a> {
  /// Creates a new reading content component.
  pub fn new(msg: &'a MailMessage, to_name: &'a str, snooze_open: bool, state: &'a State) -> Self {
    Self {
      msg,
      snooze_open,
      state,
      to_name,
    }
  }

  /// Renders the reading pane content.
  pub fn render(self) -> Element<'a, Message> {
    let snooze_label = self
      .msg
      .snoozed
      .as_deref()
      .map(|s| format!("Until {s}"))
      .unwrap_or_else(|| "Snooze".to_string());
    let toolbar = action_bar::Component::new(
      self.msg.starred,
      self.msg.snoozed.is_some(),
      &snooze_label,
      &self.msg.date_label,
      &self.msg.time,
    )
    .render();
    let scrollable_content = reading_scrollable_content(self.msg, self.to_name, self.state);
    let base: Element<'_, Message> = container(column([toolbar, scrollable_content]))
      .width(Length::Fill)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(iced::Background::Color(color::surface::BASE)),
        ..container::Style::default()
      })
      .into();
    if self.snooze_open {
      let calendar = self.state.snooze_calendar.as_ref();
      iced::widget::stack([
        base,
        super::super::snooze_picker::Component::new(self.msg, calendar).render(),
      ])
      .into()
    } else {
      base
    }
  }
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
