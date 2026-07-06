use iced::{Element, Length, Task, widget::Column};

use super::{Message as Parent, State};
use crate::ui::{components::button::Button, style::spacing};

#[derive(Clone, Debug)]
pub enum Message {
  Selected(Option<String>),
}

pub(super) fn update(state: &mut State, message: Message) -> Task<Parent> {
  match message {
    Message::Selected(day) => state.selected = day,
  }
  Task::none()
}

pub(super) fn view(state: &State) -> Element<'_, Parent> {
  let mut rows: Vec<Element<'_, Parent>> = vec![day_row(
    t!("captains_log.today").into_owned(),
    None,
    state.selected.is_none(),
  )];

  for entry in &state.entries {
    let day = entry.date_iso.clone();
    let selected = state.selected.as_deref() == Some(day.as_str());
    rows.push(day_row(day.clone(), Some(day), selected));
  }

  Column::with_children(rows)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn day_row<'a>(label: String, day: Option<String>, selected: bool) -> Element<'a, Parent> {
  let button = if selected {
    Button::secondary(label)
  } else {
    Button::ghost(label)
  };
  button.on_press(Parent::Entries(Message::Selected(day))).into()
}
