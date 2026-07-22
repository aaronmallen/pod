use iced::Element;

use super::Message;
use crate::ui::{
  components::{icon::Icon, text_input::TextInput},
  style::color,
};

pub fn search_box(query: &str) -> Element<'_, Message> {
  // `TextInput` borrows its placeholder for the returned element's lifetime, so the resolved string
  // must outlive this function; cache it once to hand the widget a `&'static str`.
  static SEARCH_PLACEHOLDER: std::sync::OnceLock<String> = std::sync::OnceLock::new();
  let placeholder = SEARCH_PLACEHOLDER.get_or_init(|| t!("skills.panel_browser.search_placeholder").into_owned());

  TextInput::new(placeholder, query, Message::SearchChanged)
    .input_id(crate::features::shell::focus_search::skills_search_id())
    .leading_icon(Icon::search())
    .background(color::surface::SUNKEN)
    .render()
}
