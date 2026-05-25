//! Group header rows and separators used by all four picker tabs.

use iced::{
  Background, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text},
};

use super::super::Message;
use crate::style::{
  color, spacing,
  typography::{body, mono},
};

/// Builder for a group header that shows a trained/total count.
pub struct Component {
  count_label: String,
  is_expanded: bool,
  name: String,
}

impl Component {
  /// Creates a skill-group header with a trained/total ratio label.
  pub fn new(name: &str, is_expanded: bool, trained_count: usize, total_skills: usize) -> Self {
    Self {
      count_label: format!("{}/{}", trained_count, total_skills),
      is_expanded,
      name: name.to_string(),
    }
  }

  /// Creates a dynamic group header for item groups using a plain item count.
  pub fn dynamic(name: &str, count: usize, is_expanded: bool) -> Self {
    Self {
      count_label: count.to_string(),
      is_expanded,
      name: name.to_string(),
    }
  }

  /// Renders the group header into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let caret = if self.is_expanded { "\u{25bc}" } else { "\u{25b6}" };
    let rule = separator();
    let btn = group_btn(&self.name, caret, self.count_label);
    column([rule.into(), btn.into()]).into()
  }
}

/// Renders a thin horizontal separator line.
pub fn separator<'a>() -> iced::widget::Container<'a, Message> {
  container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
}

fn group_btn_style(status: button::Status) -> button::Style {
  let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: if active {
      Some(Background::Color(color::state::HOVER_OVERLAY))
    } else {
      None
    },
    border: iced::Border::default(),
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn group_btn(name: &str, caret: &str, count_label: String) -> button::Button<'static, Message> {
  button(
    row([
      text(caret.to_string())
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      Space::new().width(10.0).into(),
      text(name.to_string())
        .font(body::MEDIUM)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .width(Length::Fill)
        .into(),
      text(count_label)
        .font(mono::REGULAR)
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
  .on_press(Message::PickerGroupToggled(name.to_string()))
  .style(|_, status| group_btn_style(status))
}
