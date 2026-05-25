use iced::{
  Element, Length,
  alignment::Horizontal,
  widget::{column, container, text},
};

use crate::style::{color, spacing, typography};

pub struct Component<'a> {
  query: Option<&'a str>,
  add_status: Option<&'a str>,
}

impl<'a> Default for Component<'a> {
  fn default() -> Self {
    Self::new()
  }
}

impl<'a> Component<'a> {
  pub fn new() -> Self {
    Self {
      query: None,
      add_status: None,
    }
  }

  pub fn filtered(mut self, query: &'a str) -> Self {
    self.query = Some(query);
    self
  }

  pub fn add_status(mut self, status: Option<&'a str>) -> Self {
    self.add_status = status;
    self
  }

  pub fn render<MSG: 'static>(self) -> Element<'a, MSG> {
    let msg = self.add_status.unwrap_or("Add your first character to get started");

    match self.query {
      None => char_empty_placeholder(msg),
      Some(q) => char_no_results(q),
    }
  }
}

fn char_empty_placeholder<'a, MSG: 'static>(msg: &'a str) -> Element<'a, MSG> {
  container(
    text(msg)
      .font(typography::body::REGULAR)
      .size(15.0)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .center_x(Length::Fill)
  .center_y(Length::Fill)
  .into()
}

fn char_no_results<'a, MSG: 'static>(q: &'a str) -> Element<'a, MSG> {
  container(
    column([
      text("No results")
        .font(typography::body::MEDIUM)
        .size(15.0)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(format!("No characters match \"{q}\""))
        .font(typography::body::REGULAR)
        .size(13.0)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_1)
    .align_x(Horizontal::Center),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .center_x(Length::Fill)
  .center_y(Length::Fill)
  .into()
}
