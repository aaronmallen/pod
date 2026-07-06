use iced::{Element, Task, widget::Space};

use super::{Message as Parent, State};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Message {
  AnswerChanged(String),
}

pub(super) fn update(_state: &mut State, message: Message) -> Task<Parent> {
  match message {
    Message::AnswerChanged(_) => Task::none(),
  }
}

pub(super) fn view(_state: &State) -> Element<'_, Parent> {
  Space::new().into()
}
