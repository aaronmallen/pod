use iced::{Background, Element, Length, widget::container};

use super::Message;
use crate::ui::style::color;

pub(super) fn layout_shell<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
  container(content)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}
