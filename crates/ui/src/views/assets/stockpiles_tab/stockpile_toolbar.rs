//! Stockpile toolbar: heading with ready/short counts and new stockpile button.

use iced::{
  Border, Element, Length, Padding, Theme,
  widget::{Space, button, container, row, text},
};

use crate::{
  style::{
    color,
    typography::{body, mono},
  },
  views::assets::stockpiles_tab::Message,
};

fn new_btn() -> Element<'static, Message> {
  button(
    text("＋ New stockpile")
      .font(body::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .on_press(Message::NewStockpile)
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: 12.0,
    right: 12.0,
  })
  .style(|_, _| button::Style {
    background: None,
    border: Border {
      color: color::border::DEFAULT,
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  })
  .into()
}

/// Builder for the stockpile tab toolbar.
pub struct Component {
  ready_count: usize,
  short_count: usize,
}

impl Component {
  /// Creates a new toolbar with the given counts.
  pub fn new(ready_count: usize, short_count: usize) -> Self {
    Self {
      ready_count,
      short_count,
    }
  }

  /// Renders the toolbar into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    container(
      row([
        text("Stockpile targets")
          .font(body::MEDIUM)
          .size(16.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        Space::new().width(14.0).into(),
        text(format!("{} ready · {} short", self.ready_count, self.short_count))
          .font(mono::REGULAR)
          .size(10.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        Space::new().width(Length::Fill).into(),
        new_btn(),
      ])
      .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 0.0,
      bottom: 18.0,
      left: 0.0,
      right: 0.0,
    })
    .into()
  }
}
