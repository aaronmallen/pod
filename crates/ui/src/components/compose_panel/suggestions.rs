//! Suggestions overlay for recipient autocomplete in the compose panel.

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, text},
};

use super::Message;
use crate::style::{color, typography as font};

/// Builder for the suggestions overlay.
pub struct Suggestions<'a> {
  suggestions: &'a [(i64, String)],
  cursor: Option<usize>,
  make_msg: Box<dyn Fn(i64, String) -> Message + 'a>,
  top_padding: f32,
  visible: bool,
}

impl<'a> Suggestions<'a> {
  /// Creates a new suggestions builder.
  pub fn new(
    suggestions: &'a [(i64, String)],
    cursor: Option<usize>,
    make_msg: impl Fn(i64, String) -> Message + 'a,
  ) -> Self {
    Self {
      suggestions,
      cursor,
      make_msg: Box::new(make_msg),
      top_padding: 0.0,
      visible: true,
    }
  }

  /// Sets the top padding for the overlay container.
  pub fn top_padding(mut self, top: f32) -> Self {
    self.top_padding = top;
    self
  }

  /// Sets whether the overlay is visible.
  pub fn visible(mut self, visible: bool) -> Self {
    self.visible = visible;
    self
  }

  /// Renders the suggestions overlay.
  pub fn render(self) -> Element<'a, Message> {
    if !self.visible || self.suggestions.is_empty() {
      return Space::new().into();
    }
    let box_el = suggestions_box(self.suggestions, self.cursor, self.make_msg);
    container(box_el)
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(iced::alignment::Horizontal::Left)
      .align_y(iced::alignment::Vertical::Top)
      .padding(Padding {
        top: self.top_padding,
        left: 16.0,
        right: 16.0,
        bottom: 0.0,
      })
      .into()
  }
}

fn suggestions_box<'a>(
  suggestions: &'a [(i64, String)],
  cursor: Option<usize>,
  make_msg: impl Fn(i64, String) -> Message + 'a,
) -> Element<'a, Message> {
  let rows: Vec<Element<'_, Message>> = suggestions
    .iter()
    .enumerate()
    .map(|(idx, (id, name))| suggestion_row(idx, *id, name.as_str(), cursor, &make_msg))
    .collect();
  container(column(rows).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::DEFAULT,
        radius: 8.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn suggestion_row_bg(selected: bool, status: button::Status) -> Option<Background> {
  if selected {
    Some(Background::Color(color::accent::PLASMA_ACTIVE))
  } else {
    suggestion_row_hover_bg(status)
  }
}

fn suggestion_row_hover_bg(status: button::Status) -> Option<Background> {
  match status {
    button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::SUBTLE_FILL)),
    _ => None,
  }
}

/// Renders a single suggestion row button.
pub fn suggestion_row<'a>(
  idx: usize,
  id: i64,
  name: &'a str,
  cursor: Option<usize>,
  make_msg: impl Fn(i64, String) -> Message,
) -> Element<'a, Message> {
  let selected = cursor == Some(idx);
  let msg_name = name.to_string();
  button(
    text(name)
      .font(font::body::MEDIUM)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 12.0,
    right: 12.0,
  })
  .on_press(make_msg(id, msg_name))
  .style(move |_, status| button::Style {
    background: suggestion_row_bg(selected, status),
    border: Border::default(),
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}
