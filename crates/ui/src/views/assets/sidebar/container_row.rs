//! Container row in the sidebar tree — indented beneath a location node.

use iced::{
  Element, Length, Padding, Theme,
  widget::{button, row, text},
};

use super::super::Message;
use crate::style::{
  button as btn_style, color,
  typography::{body, mono},
};

fn tree_value_el(value: f64, active: bool) -> Element<'static, Message> {
  use crate::format;
  text(format::fmt_isk(value))
    .font(mono::REGULAR)
    .size(9.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(if active {
        color::accent::PLASMA
      } else {
        color::text::TERTIARY
      }),
    })
    .into()
}

/// Builder for a container node button row (double-indented, with container glyph).
pub struct Component<'a> {
  container_name: &'a str,
  filter: String,
  active: bool,
  value: f64,
}

impl<'a> Component<'a> {
  /// Creates a new container row.
  pub fn new(container_name: &'a str, filter: impl Into<String>, active: bool, value: f64) -> Self {
    Self {
      container_name,
      filter: filter.into(),
      active,
      value,
    }
  }

  /// Renders the container row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let active = self.active;
    let filter = self.filter.clone();

    let mut row_children: Vec<Element<'_, Message>> = vec![
      text("▣")
        .font(mono::REGULAR)
        .size(10.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
      text(self.container_name.to_string())
        .font(body::REGULAR)
        .size(12.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(if active {
            color::text::PRIMARY
          } else {
            color::text::MEDIUM
          }),
        })
        .width(Length::Fill)
        .into(),
    ];
    if self.value > 0.0 {
      row_children.push(tree_value_el(self.value, active));
    }

    let msg = Message::LocationSelected(Some(filter));
    button(
      row(row_children)
        .spacing(6.0)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 5.0,
      bottom: 5.0,
      left: 60.0,
      right: 12.0,
    })
    .width(Length::Fill)
    .on_press(msg)
    .style(move |_, status| btn_style::list_item_active(active, status))
    .into()
  }
}
