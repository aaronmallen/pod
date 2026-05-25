//! To recipient field row for the compose panel.

use iced::{
  Background, Border, Color, Element, Padding, Theme,
  widget::{button, container, row, text, text_input},
};

use super::{Component, Message};
use crate::style::{color, typography as font};

/// Builder for the compose To field row.
pub struct ComposeToField<'a> {
  panel: &'a Component,
}

impl<'a> ComposeToField<'a> {
  /// Creates a new To field builder.
  pub fn new(panel: &'a Component) -> Self {
    Self {
      panel,
    }
  }

  /// Renders the To field row.
  pub fn render(self) -> Element<'a, Message> {
    to_field(self.panel)
  }
}

pub(super) fn to_field(panel: &Component) -> Element<'_, Message> {
  let to_chips: Vec<Element<'_, Message>> = panel
    .to
    .iter()
    .enumerate()
    .map(|(i, r)| recipient_chip(r.name.as_str(), Message::ToRemove(i)))
    .collect();

  let to_input = text_input(
    if panel.to.is_empty() { "Add recipient…" } else { "" },
    &panel.to_search,
  )
  .on_input(Message::ToSearchChanged)
  .on_submit(Message::ToAdd)
  .size(13.0)
  .font(font::body::REGULAR)
  .style(|_, _| text_input::Style {
    background: Background::Color(Color::TRANSPARENT),
    border: Border::default(),
    icon: color::text::SECONDARY,
    placeholder: color::text::TERTIARY,
    value: color::text::PRIMARY,
    selection: color::state::SELECTION,
  });

  let mut row_children: Vec<Element<'_, Message>> = to_chips;
  row_children.push(to_input.into());
  if !panel.cc_visible {
    row_children.push(cc_toggle_btn());
  }

  super::compose_field_row(
    "To",
    row(row_children)
      .spacing(6.0)
      .align_y(iced::alignment::Vertical::Center)
      .into(),
  )
}

/// Renders the Cc toggle button shown in the To field row.
pub fn cc_toggle_btn() -> Element<'static, Message> {
  button(
    text("Cc")
      .font(font::body::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding::from([0.0, 0.0]))
  .on_press(Message::CcToggle)
  .style(|_, status| button::Style {
    background: None,
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::text::PRIMARY,
      _ => color::text::SECONDARY,
    },
    ..button::Style::default()
  })
  .into()
}

/// Renders a recipient chip with a remove button.
pub fn recipient_chip(name: &str, remove_msg: Message) -> Element<'_, Message> {
  container(
    row([
      text(name)
        .font(font::body::REGULAR)
        .size(12.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      recipient_remove_btn(remove_msg),
    ])
    .spacing(4.0)
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 8.0,
    right: 6.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::state::SUBTLE_FILL)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 999.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

/// Renders the remove (×) button inside a recipient chip.
pub fn recipient_remove_btn(remove_msg: Message) -> Element<'static, Message> {
  button(
    text("✕")
      .font(font::mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding::from([0.0, 0.0]))
  .on_press(remove_msg)
  .style(|_, status| button::Style {
    background: None,
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::text::PRIMARY,
      _ => color::text::SECONDARY,
    },
    ..button::Style::default()
  })
  .into()
}
