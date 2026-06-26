use iced::Element;

use super::Message;
use crate::ui::{
  components::{icon::Icon, text_input::TextInput},
  style::color,
};

pub fn search_box(query: &str) -> Element<'_, Message> {
  TextInput::new("Search skills\u{2026}", query, Message::SearchChanged)
    .input_id(crate::features::shell::focus_search::skills_search_id())
    .leading_icon(Icon::search())
    .background(color::surface::SUNKEN)
    .render()
}
