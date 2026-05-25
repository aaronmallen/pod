//! Tab button component for the skills right panel tab bar.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  widget::{Space, button, column, container, text},
};

use super::super::RightTab;
use crate::{
  style::{color, typography::body},
  views::skills::right_panel::Message,
};

fn tab_text_color(is_active: bool) -> iced::Color {
  if is_active {
    color::text::PRIMARY
  } else {
    color::text::SECONDARY
  }
}

fn tab_btn_style(is_active: bool) -> button::Style {
  button::Style {
    background: None,
    border: Border::default(),
    text_color: tab_text_color(is_active),
    ..button::Style::default()
  }
}

fn tab_underline_bg(is_active: bool) -> iced::Color {
  if is_active {
    color::accent::PLASMA
  } else {
    Color::TRANSPARENT
  }
}

/// A single tab button with an active underline indicator.
pub struct TabButton {
  is_active: bool,
  label: &'static str,
  tab: RightTab,
}

impl TabButton {
  /// Creates a new `TabButton` for the given tab and label.
  pub fn new(tab: RightTab, label: &'static str, is_active: bool) -> Self {
    Self {
      is_active,
      label,
      tab,
    }
  }

  /// Renders the tab button into an `Element`.
  pub fn render(self) -> Element<'static, Message> {
    let is_active = self.is_active;
    let tab_color = tab_text_color(is_active);
    let btn = button(
      text(self.label)
        .font(body::MEDIUM)
        .size(13.0)
        .style(move |_| iced::widget::text::Style {
          color: Some(tab_color),
        }),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      bottom: 12.0,
      left: 14.0,
      right: 14.0,
    })
    .on_press(Message::TabSelected(self.tab))
    .style(move |_, _| tab_btn_style(is_active));

    let underline_bg = tab_underline_bg(is_active);
    let underline = container(Space::new().width(Length::Fill).height(2.0))
      .width(Length::Fill)
      .height(2.0)
      .style(move |_| container::Style {
        background: Some(Background::Color(underline_bg)),
        ..container::Style::default()
      });

    column([btn.into(), underline.into()]).width(Length::Fill).into()
  }
}
