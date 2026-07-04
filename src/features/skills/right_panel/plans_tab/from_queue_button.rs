use iced::Element;

use super::Message;
use crate::ui::components::{
  button::{Button, Size},
  icon::Icon,
};

pub fn from_queue_button<'a>() -> Element<'a, Message> {
  Button::primary(t!("skills.panel_plans.from_queue"))
    .icon(Icon::plus())
    .size(Size::Sm)
    .on_press(Message::FromQueue)
    .into()
}
