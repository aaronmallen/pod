//! Individual location button in the sidebar tree.

use iced::{
  Color, Element, Length, Padding, Theme,
  widget::{button, row, text},
};

use super::super::{Message, struct_glyph};
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

/// Builder for a location button row (indented, with station glyph).
pub struct Component<'a> {
  loc_name: &'a str,
  loc_filter: String,
  active: bool,
  value: f64,
}

impl<'a> Component<'a> {
  /// Creates a new location row.
  pub fn new(loc_name: &'a str, loc_filter: impl Into<String>, active: bool, value: f64) -> Self {
    Self {
      loc_name,
      loc_filter: loc_filter.into(),
      active,
      value,
    }
  }

  /// Renders the location row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let active = self.active;
    let glyph = struct_glyph("station");
    let glyph_color = color::accent::PLASMA;
    let loc_filter = self.loc_filter.clone();

    let mut row_children: Vec<Element<'_, Message>> = vec![
      text(glyph)
        .font(mono::REGULAR)
        .size(11.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(glyph_color),
        })
        .into(),
      text(self.loc_name.to_string())
        .font(body::REGULAR)
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

    let msg = Message::LocationSelected(Some(loc_filter));
    button(
      row(row_children)
        .spacing(6.0)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 5.0,
      bottom: 5.0,
      left: 24.0,
      right: 12.0,
    })
    .width(Length::Fill)
    .on_press(msg)
    .style(move |_, status| btn_style::list_item_active(active, status))
    .into()
  }
}
