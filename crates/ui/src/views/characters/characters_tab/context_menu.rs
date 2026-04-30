use iced::{
  Element, Length, Padding,
  widget::{container, text},
};

use crate::{
  components,
  style::{color, typography},
};

#[derive(Clone, Debug)]
pub struct State {
  pub character_id: i64,
  pub character_name: String,
  pub x: f32,
  pub y: f32,
}

#[derive(Clone, Debug)]
pub enum Message {
  Close,
  CopyName,
  EditTags,
  RemoveRequested,
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
    let items: Vec<Element<'a, Message>> = vec![
      name_header(self.state.character_name.as_str()),
      components::Button::row(text("Copy name").size(13.0).font(typography::body::REGULAR))
        .width(Length::Fill)
        .on_press(Message::CopyName)
        .into(),
      components::Button::row(text("Edit tags").size(13.0).font(typography::body::REGULAR))
        .width(Length::Fill)
        .on_press(Message::EditTags)
        .into(),
      components::Separator::horizontal().render(),
      components::Button::danger_ghost(text("Remove from app").size(13.0).font(typography::body::REGULAR))
        .width(Length::Fill)
        .on_press(Message::RemoveRequested)
        .into(),
    ];

    components::ContextMenu::new(items)
      .position(self.state.x, self.state.y)
      .render()
  }
}

fn name_header<'a>(name: &'a str) -> Element<'a, Message> {
  container(
    text(name)
      .size(9.0)
      .font(typography::mono::REGULAR)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .padding(Padding {
    top: 8.0,
    bottom: 6.0,
    left: 10.0,
    right: 10.0,
  })
  .width(Length::Fill)
  .into()
}
