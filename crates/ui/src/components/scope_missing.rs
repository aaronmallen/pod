use iced::{
  Element, Length, Theme,
  widget::{column, container, text},
};

use crate::{
  components,
  style::{color, spacing, typography::body},
};

/// Message emitted by the ScopeMissing component.
#[derive(Clone, Debug)]
pub enum Message {
  /// The user pressed the Re-authorize button for this character.
  ReauthorizePressed(i64),
}

pub struct Component {
  character_id: i64,
  feature_name: &'static str,
}

impl Component {
  pub fn new(character_id: i64, feature_name: &'static str) -> Self {
    Self {
      character_id,
      feature_name,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let char_id = self.character_id;
    let body_text = format!(
      "Your authorization for this character doesn't include \
      the scopes needed for {}.",
      self.feature_name
    );

    let reauth_btn =
      components::Button::primary(text("Re-authorize").font(body::MEDIUM).size(14.0).style(|_: &Theme| {
        iced::widget::text::Style {
          color: Some(color::surface::BASE),
        }
      }))
      .on_press(Message::ReauthorizePressed(char_id));

    container(
      column([
        text("Re-authorization required")
          .font(body::MEDIUM)
          .size(16.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        text(body_text)
          .font(body::REGULAR)
          .size(13.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        reauth_btn.into(),
      ])
      .spacing(spacing::SPACE_4),
    )
    .padding(40.0)
    .width(Length::Fill)
    .center_x(Length::Fill)
    .into()
  }
}
