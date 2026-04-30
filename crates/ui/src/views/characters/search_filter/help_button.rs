use iced::{Element, Length, widget::container};

use crate::{components, style::color};

#[derive(Clone, Debug)]
pub enum Message {
  Toggle,
}

pub struct Component {
  open: bool,
}

impl Component {
  pub fn new(open: bool) -> Self {
    Self {
      open,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let icon_color = if self.open {
      color::accent::PLASMA
    } else {
      color::text::SECONDARY
    };

    components::Button::ghost(
      container(
        components::Icon::help()
          .size(15.0)
          .color(icon_color)
          .render::<Message>(),
      )
      .center_x(Length::Fill)
      .center_y(Length::Fill),
    )
    .width(28.0)
    .height(26.0)
    .padding(0)
    .on_press(Message::Toggle)
    .into()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new(false)
  }
}
