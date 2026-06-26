use iced::Element;

use super::super::Message;
use crate::ui::{
  components::{icon::Icon, text_input::TextInput},
  style::color,
};

pub(in crate::features::skills::skill_plan_editor) fn search_bar<'a>(
  query: &'a str,
  placeholder: &'a str,
) -> Element<'a, Message> {
  TextInput::new(placeholder, query, Message::PickerSearchChanged)
    .leading_icon(Icon::search())
    .background(color::surface::SUNKEN)
    .render()
}
