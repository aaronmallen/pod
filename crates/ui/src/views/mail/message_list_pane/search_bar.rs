//! Message list search bar: search input row with icon and separator.

use iced::{
  Border, Color, Element, Length, Padding,
  widget::{column, container, row, text_input},
};

use crate::{
  components::{Icon, Separator},
  style::{color, typography::body},
  views::mail::message_list_pane::Message,
};

/// Builder for the message list search bar.
pub struct Component<'a> {
  query: &'a str,
}

impl<'a> Component<'a> {
  /// Creates a new search bar bound to the current query.
  pub fn new(query: &'a str) -> Self {
    Self {
      query,
    }
  }

  /// Renders the search bar.
  pub fn render(self) -> Element<'a, Message> {
    let inner = container(
      row([
        Icon::search()
          .size(16.0)
          .color(color::text::SECONDARY)
          .render::<Message>(),
        text_input("Search mail", self.query)
          .on_input(Message::SearchChanged)
          .font(body::REGULAR)
          .size(13.0)
          .style(|_, _| text_input::Style {
            background: iced::Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: color::text::SECONDARY,
            placeholder: color::text::TERTIARY,
            value: color::text::PRIMARY,
            selection: color::state::SELECTION,
          })
          .width(Length::Fill)
          .into(),
      ])
      .spacing(10.0)
      .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 14.0,
      bottom: 14.0,
      left: 16.0,
      right: 16.0,
    })
    .width(Length::Fill);
    column([inner.into(), Separator::horizontal().render()]).into()
  }
}
