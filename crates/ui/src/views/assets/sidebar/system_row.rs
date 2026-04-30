//! System group header row in the sidebar tree.

use iced::{
  Color, Element, Length, Padding, Theme,
  widget::{button, row, text},
};

use super::super::Message;
use crate::style::{button as btn_style, color, typography::mono};

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

/// Builder for a system group header row in the sidebar.
pub struct Component<'a> {
  sys_name: &'a str,
  sys_filter: String,
  active: bool,
  value: f64,
}

impl<'a> Component<'a> {
  /// Creates a new system row.
  pub fn new(sys_name: &'a str, sys_filter: impl Into<String>, active: bool, value: f64) -> Self {
    Self {
      sys_name,
      sys_filter: sys_filter.into(),
      active,
      value,
    }
  }

  /// Renders the system row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let active = self.active;
    let glyph_color = color::text::SECONDARY;
    let sys_filter = self.sys_filter.clone();

    let mut row_children: Vec<Element<'_, Message>> = vec![
      text("◉")
        .font(mono::REGULAR)
        .size(11.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(glyph_color),
        })
        .into(),
      text(self.sys_name.to_string())
        .font(mono::REGULAR)
        .size(12.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(if active {
            color::text::PRIMARY
          } else {
            Color::from_rgba(0.957, 0.949, 0.925, 0.78)
          }),
        })
        .width(Length::Fill)
        .into(),
    ];
    if self.value > 0.0 {
      row_children.push(tree_value_el(self.value, active));
    }

    let msg = Message::LocationSelected(Some(sys_filter));
    button(
      row(row_children)
        .spacing(6.0)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 5.0,
      bottom: 5.0,
      left: 12.0,
      right: 12.0,
    })
    .width(Length::Fill)
    .on_press(msg)
    .style(move |_, status| btn_style::list_item_active(active, status))
    .into()
  }
}
