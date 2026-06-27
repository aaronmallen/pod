use iced::{
  Element,
  alignment::Vertical,
  widget::{Row, button, text},
};

use super::Message;
use crate::ui::style::{control, spacing, typography};

pub(super) fn add_character_button<'a>() -> Element<'a, Message> {
  button(
    Row::with_children(vec![
      text("+").size(typography::size::MD).into(),
      text(t!("roster.actions.add_character"))
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(control::padding())
  .on_press(Message::AddCharacterRequested)
  .style(control::ghost_button)
  .into()
}
