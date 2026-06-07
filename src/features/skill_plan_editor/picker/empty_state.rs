use iced::Element;

use super::super::Message;
use crate::ui::components::empty_state::empty_state as shared_empty_state;

pub(in crate::features::skill_plan_editor) fn empty_state<'a>(message: &'a str) -> Element<'a, Message> {
  shared_empty_state(message).render()
}
