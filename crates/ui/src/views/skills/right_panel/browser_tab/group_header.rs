//! Collapsible group header row component.

use iced::{
  Background, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text},
};

use super::{super::super::fmt_sp, Message};
use crate::style::{color, spacing, typography::body};

pub struct Component {
  name: String,
  is_expanded: bool,
  trained_count: usize,
  total_skills: usize,
  total_sp: u64,
  group_id: String,
}

impl Component {
  pub fn new(
    name: &str,
    is_expanded: bool,
    trained_count: usize,
    total_skills: usize,
    total_sp: u64,
    group_id: String,
  ) -> Self {
    Self {
      name: name.to_owned(),
      is_expanded,
      trained_count,
      total_skills,
      total_sp,
      group_id,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let caret_char = if self.is_expanded { "▼" } else { "▶" };
    let rule = container(Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .height(1.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      })
      .into();
    column([
      rule,
      group_header_btn(
        self.name,
        caret_char,
        self.trained_count,
        self.total_skills,
        self.total_sp,
        self.group_id,
      ),
    ])
    .into()
  }
}

fn group_header_btn(
  name: String,
  caret_char: &'static str,
  trained_count: usize,
  total_skills: usize,
  total_sp: u64,
  group_id: String,
) -> Element<'static, Message> {
  button(
    row([
      text(caret_char)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      Space::new().width(10.0).into(),
      text(name)
        .font(body::MEDIUM)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .width(Length::Fill)
        .into(),
      text(format!("{}/{} · {} SP", trained_count, total_skills, fmt_sp(total_sp)))
        .font(crate::style::typography::mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::GroupToggle(group_id))
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
      _ => None,
    },
    border: iced::Border::default(),
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}
