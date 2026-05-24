//! Body paragraph rendering.

use iced::{Element, Theme, widget::text};

use super::{super::MailMessage, Message};
use crate::style::{color, typography::body};

/// Builder for a single body paragraph element.
pub struct Component;

impl Component {
  /// Render all paragraphs of a message body.
  pub fn render<'a>(msg: &'a MailMessage) -> Vec<Element<'a, Message>> {
    if !msg.body.is_empty() {
      msg
        .body
        .iter()
        .map(|p| {
          text(p.as_str())
            .font(body::REGULAR)
            .size(15.0)
            .style(|_: &Theme| iced::widget::text::Style {
              color: Some(color::text::PRIMARY),
            })
            .wrapping(iced::widget::text::Wrapping::Word)
            .into()
        })
        .collect()
    } else if !msg.body_loaded {
      vec![
        text("Loading message…")
          .font(body::REGULAR)
          .size(15.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      ]
    } else {
      vec![
        text("No message body available.")
          .font(body::REGULAR)
          .size(15.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::TERTIARY),
          })
          .into(),
      ]
    }
  }
}
