//! Floating overlay that renders the character-picker dropdown.

use iced::{Element, Padding, widget::container};

use super::{Message, State};

pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    let dropdown = self.state.picker.dropdown().map(Message::Picker);
    container(dropdown)
      .padding(Padding {
        top: 98.0,
        left: 28.0,
        right: 0.0,
        bottom: 0.0,
      })
      .into()
  }
}
