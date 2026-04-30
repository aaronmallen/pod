//! Total skills + duration footer component.

use iced::{
  Background, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, text},
};

use super::super::Message;
use crate::{
  components,
  format::{fmt_dur, fmt_eta},
  style::{color, spacing, typography::mono},
};

pub struct Component {
  total_n: usize,
  total_secs: f32,
}

impl Component {
  pub fn new(total_n: usize, total_secs: f32) -> Self {
    Self {
      total_n,
      total_secs,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let footer = container(
      row([
        text(format!("Total · {} skills", self.total_n))
          .font(mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        Space::new().width(Length::Fill).into(),
        text(fmt_dur(self.total_secs as u64))
          .font(mono::MEDIUM)
          .size(13.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        Space::new().width(spacing::SPACE_4).into(),
        text(format!("finishes {} EVE", fmt_eta(self.total_secs as u64)))
          .font(mono::REGULAR)
          .size(11.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      ])
      .align_y(Vertical::Center),
    )
    .padding(Padding {
      top: 14.0,
      bottom: 14.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    });

    column([components::Separator::horizontal().render(), footer.into()]).into()
  }
}
