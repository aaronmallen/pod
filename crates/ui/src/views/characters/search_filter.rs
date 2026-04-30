pub mod help_button;
pub mod help_pop_over;
pub mod input;
pub mod search_icon;

pub use help_button::Component as HelpButton;
pub use help_pop_over::Component as HelpPopOver;
use iced::{Background, Element, Length, Padding, Task, widget::container};
pub use input::Component as Input;
pub use search_icon::Component as SearchIcon;

use crate::{
  components::SearchBox,
  style::{color, spacing},
};

pub struct State {
  pub query: String,
  pub help_pop_over: help_pop_over::State,
  pub input_id: iced::widget::Id,
}

impl State {
  pub fn new() -> Self {
    Self {
      query: String::new(),
      help_pop_over: help_pop_over::State::default(),
      input_id: iced::widget::Id::unique(),
    }
  }

  pub fn update(&mut self, msg: Message) -> Task<Message> {
    match msg {
      Message::QueryChanged(s) => {
        self.query = s;
        Task::none()
      }
      Message::HelpPopOver(inner) => {
        if let help_pop_over::Message::QueryInserted(ref q) = inner {
          let sep = if self.query.is_empty() { "" } else { " " };
          self.query = format!("{}{sep}{q}", self.query);
          let id = self.input_id.clone();
          let _ = self.help_pop_over.update(inner).map(Message::HelpPopOver);
          return iced::widget::operation::focus(id).map(|_: ()| Message::FocusInput);
        }
        self.help_pop_over.update(inner).map(Message::HelpPopOver)
      }
      Message::HelpToggle => {
        let inner = if self.help_pop_over.visible {
          help_pop_over::Message::Close
        } else {
          help_pop_over::Message::Open
        };
        self.help_pop_over.update(inner).map(Message::HelpPopOver)
      }
      Message::FocusInput => iced::widget::operation::focus(self.input_id.clone()).map(|_: ()| Message::FocusInput),
    }
  }
}

impl Default for State {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Clone, Debug)]
pub enum Message {
  QueryChanged(String),
  HelpPopOver(help_pop_over::Message),
  HelpToggle,
  FocusInput,
}

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
    let help_btn = HelpButton::new(self.state.help_pop_over.visible)
      .render()
      .map(|_| Message::HelpToggle);

    let search_box = SearchBox::new(
      "Search… try tag:pvp or corp:caldari",
      &self.state.query,
      Message::QueryChanged,
    )
    .height(36.0)
    .horizontal_padding(0.0)
    .icon_spacing(6.0)
    .input_id(self.state.input_id.clone())
    .right_element(help_btn)
    .background(color::surface::BASE)
    .render();

    container(
      container(search_box)
        .padding(Padding {
          top: 14.0,
          bottom: 14.0,
          left: spacing::SPACE_8,
          right: spacing::SPACE_8,
        })
        .width(Length::Fill),
    )
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .width(Length::Fill)
    .into()
  }
}
