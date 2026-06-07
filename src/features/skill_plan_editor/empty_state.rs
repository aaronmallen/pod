use iced::Element;

use super::Message;
use crate::ui::components::empty_state::empty_state as shared_empty_state;

pub(super) fn empty_state<'a>() -> Element<'a, Message> {
  shared_empty_state("No skills in this plan yet")
    .subtitle("Add your first skill using the skill picker.")
    .action("Open skill picker", Message::PickerToggled)
    .render()
}
