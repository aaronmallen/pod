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
    let btn = button(
      text(self.label)
        .font(body::MEDIUM)
        .size(13.0)
        .style(move |_| iced::widget::text::Style {
          color: Some(if is_active {
            color::text::PRIMARY
          } else {
            color::text::SECONDARY
          }),
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
    .style(move |_, _| button::Style {
      background: None,
      border: Border::default(),
      text_color: if is_active {
        color::text::PRIMARY
      } else {
        color::text::SECONDARY
      },
      ..button::Style::default()
    });

    let underline = container(Space::new().width(Length::Fill).height(2.0))
      .width(Length::Fill)
      .height(2.0)
      .style(move |_| container::Style {
        background: Some(Background::Color(if is_active {
          color::accent::PLASMA
        } else {
          Color::TRANSPARENT
        })),
        ..container::Style::default()
      });

    column([btn.into(), underline.into()]).width(Length::Fill).into()
  }
}
