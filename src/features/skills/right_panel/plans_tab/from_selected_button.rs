use iced::Element;

use super::Message;
use crate::ui::components::button::Button;

pub fn from_selected_button<'a>(count: usize) -> Element<'a, Message> {
  let label = format!("{} {}", t!("skills.panel_plans.from_selected"), count);

  Button::primary(label).block().on_press(Message::FromSelected).into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_renders_the_count() {
    let _el: Element<'_, Message> = from_selected_button(4);
  }
}
