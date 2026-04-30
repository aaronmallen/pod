use iced::{
  Element,
  widget::{row, text},
};

use crate::{
  components,
  style::{spacing, typography},
};

#[derive(Clone, Debug)]
pub enum Message {
  Pressed,
}

pub struct Component;

impl Component {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    components::Button::ghost(
      row([
        text("+").font(typography::body::MEDIUM).size(13.0).into(),
        text("Add character").font(typography::body::MEDIUM).size(13.0).into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(iced::alignment::Vertical::Center),
    )
    .on_press(Message::Pressed)
    .into()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}
