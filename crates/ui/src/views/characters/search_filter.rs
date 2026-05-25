pub mod help_button;
pub mod help_pop_over;
pub mod input;
pub mod search_icon;

pub use help_button::Component as HelpButton;
pub use help_pop_over::Component as HelpPopOver;
use iced::{Background, Element, Length, Padding, Task, alignment::Horizontal, widget::container};
pub use input::Component as Input;
pub use search_icon::Component as SearchIcon;

use crate::{
  components::{Popover, SearchBox},
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
      Message::HelpPopOver(inner) => update_help_popover(self, inner),
      Message::HelpToggle => update_help_toggle(self),
      Message::FocusInput => iced::widget::operation::focus(self.input_id.clone()).map(|_: ()| Message::FocusInput),
    }
  }
}

impl Default for State {
  fn default() -> Self {
    Self::new()
  }
}

fn update_help_popover(state: &mut State, inner: help_pop_over::Message) -> Task<Message> {
  if let help_pop_over::Message::QueryInserted(ref q) = inner {
    let sep = if state.query.is_empty() { "" } else { " " };
    state.query = format!("{}{sep}{q}", state.query);
    let id = state.input_id.clone();
    let _ = state.help_pop_over.update(inner).map(Message::HelpPopOver);
    return iced::widget::operation::focus(id).map(|_: ()| Message::FocusInput);
  }
  state.help_pop_over.update(inner).map(Message::HelpPopOver)
}

fn update_help_toggle(state: &mut State) -> Task<Message> {
  let inner = if state.help_pop_over.visible {
    help_pop_over::Message::Close
  } else {
    help_pop_over::Message::Open
  };
  state.help_pop_over.update(inner).map(Message::HelpPopOver)
}

#[derive(Clone, Debug)]
pub enum Message {
  QueryChanged(String),
  HelpPopOver(help_pop_over::Message),
  HelpToggle,
  FocusInput,
}

pub struct Component<'a> {
  all_tags: &'a [(i32, String, Option<String>)],
  state: &'a State,
}

impl<'a> Component<'a> {
  pub fn new(state: &'a State) -> Self {
    Self {
      all_tags: &[],
      state,
    }
  }

  /// Sets the available tags used to populate the help pop-over.
  pub fn all_tags(mut self, all_tags: &'a [(i32, String, Option<String>)]) -> Self {
    self.all_tags = all_tags;
    self
  }

  pub fn render(self) -> Element<'a, Message> {
    let is_open = self.state.help_pop_over.visible;

    let help_btn = HelpButton::new(is_open).render().map(|_| Message::HelpToggle);

    let search_box = SearchBox::new(
      "Search… try tag:pvp or corp:caldari",
      &self.state.query,
      Message::QueryChanged,
    )
    .height(36.0)
    .horizontal_padding(12.0)
    .icon_spacing(10.0)
    .input_id(self.state.input_id.clone())
    .right_element(help_btn)
    .background(color::surface::BASE)
    .render();

    let anchor = container(
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
    .width(Length::Fill);

    let help_content = HelpPopOver::new(&self.state.help_pop_over, self.all_tags)
      .render()
      .map(Message::HelpPopOver);

    let overlay = container(help_content)
      .height(Length::Fill)
      .width(Length::Fill)
      .padding(Padding {
        top: 64.0,
        right: spacing::SPACE_8,
        ..Padding::ZERO
      })
      .align_x(Horizontal::Right);

    Popover::new(anchor, overlay, is_open).render()
  }
}
