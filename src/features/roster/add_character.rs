use iced::Element;

use super::Message;
use crate::ui::components::{button::Button, icon::Icon};

pub(super) fn add_character_button<'a>() -> Element<'a, Message> {
  Button::primary(t!("roster.actions.add_character"))
    .icon(Icon::plus())
    .on_press(Message::AddCharacterRequested)
    .into()
}
