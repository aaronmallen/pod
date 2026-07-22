use iced::Element;

use super::Message;
use crate::ui::components::button::Button;

pub fn from_queue_button<'a>() -> Element<'a, Message> {
  Button::secondary(t!("skills.panel_plans.from_queue"))
    .block()
    .on_press(Message::FromQueue)
    .into()
}
