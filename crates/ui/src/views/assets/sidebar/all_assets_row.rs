//! "All assets" aggregate row in the sidebar.

use iced::{
  Element, Length, Padding, Theme,
  widget::{button, row, text},
};

use super::super::Message;
use crate::style::{
  button as btn_style, color,
  typography::{body, mono},
};

fn label(active: bool) -> Element<'static, Message> {
  text("All assets")
    .font(body::REGULAR)
    .size(12.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(if active {
        color::text::PRIMARY
      } else {
        color::text::STRONG
      }),
    })
    .width(Length::Fill)
    .into()
}

/// Builder for the "All assets" aggregate row in the sidebar.
pub struct Component {
  active: bool,
}

impl Component {
  /// Creates a new all-assets row with the given active state.
  pub fn new(active: bool) -> Self {
    Self {
      active,
    }
  }

  /// Renders the all-assets row into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let active = self.active;
    let glyph_color = color::text::SECONDARY;
    let msg = Message::LocationSelected(None);
    let row_children: Vec<Element<'_, Message>> = vec![
      text("∑")
        .font(mono::REGULAR)
        .size(11.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(glyph_color),
        })
        .into(),
      label(active),
    ];

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
