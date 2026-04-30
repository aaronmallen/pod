use iced::{
  Element, Length, Theme,
  widget::{container, text},
};

use crate::style::{color, typography::body};

pub struct Component<'a> {
  kind: Kind<'a>,
}

enum Kind<'a> {
  Empty(&'a str),
  Error(&'a str),
  Loading(&'a str),
}

impl<'a> Component<'a> {
  /// Centred placeholder shown while data is empty.
  pub fn empty(msg: &'a str) -> Self {
    Self {
      kind: Kind::Empty(msg),
    }
  }

  /// Centred error message shown when loading fails.
  pub fn error(msg: &'a str) -> Self {
    Self {
      kind: Kind::Error(msg),
    }
  }

  /// Centred loading message shown while data is being fetched.
  pub fn loading(msg: &'a str) -> Self {
    Self {
      kind: Kind::Loading(msg),
    }
  }

  /// Renders the state placeholder into an iced element.
  pub fn render<MSG: 'a>(self) -> Element<'a, MSG> {
    let (label, text_color) = match self.kind {
      Kind::Empty(msg) | Kind::Loading(msg) => (msg, color::text::SECONDARY),
      Kind::Error(msg) => (msg, color::status::DANGER),
    };
    container(
      text(label)
        .font(body::REGULAR)
        .size(13.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(text_color),
        }),
    )
    .padding(32.0)
    .width(Length::Fill)
    .center_x(Length::Fill)
    .into()
  }
}
