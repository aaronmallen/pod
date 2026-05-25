//! Tab button with active-state underline for the Characters header.

use iced::{
  Background, Border, Element, Length, Padding, Shadow,
  alignment::Vertical,
  widget::{button, column, container, row, text},
};

use crate::style::{color, spacing, typography};

/// Builder for a single tab entry (label + count + underline).
pub struct Component {
  /// Display label for the tab.
  count: String,
  /// Whether this tab is currently active.
  is_active: bool,
  /// Display label for the tab.
  label: String,
  /// Message emitted when the tab is pressed.
  on_press: super::Message,
}

impl Component {
  /// Creates a new tab component.
  pub fn new(label: impl Into<String>, count: impl Into<String>, is_active: bool, on_press: super::Message) -> Self {
    Self {
      count: count.into(),
      is_active,
      label: label.into(),
      on_press,
    }
  }

  /// Renders the tab (button + underline) into an iced element.
  pub fn render(self) -> Element<'static, super::Message> {
    let tab_btn = tab_button(&self.label, &self.count, self.is_active, self.on_press);
    let underline = tab_underline(self.is_active);
    column([tab_btn.into(), underline.into()]).width(Length::Shrink).into()
  }
}

fn tab_button(
  label: &str,
  count: &str,
  is_active: bool,
  on_press: super::Message,
) -> button::Button<'static, super::Message> {
  let label_owned = label.to_string();
  let count_owned = count.to_string();

  let content = row([
    text(label_owned).font(typography::body::MEDIUM).size(20.0).into(),
    text(count_owned)
      .font(typography::mono::MEDIUM)
      .size(11.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(if is_active {
          color::accent::PLASMA
        } else {
          color::text::TERTIARY
        }),
      })
      .into(),
  ])
  .spacing(10.0)
  .align_y(Vertical::Center);

  let centered = container(content).height(Length::Fill).center_y(Length::Fill);

  button(centered)
    .height(spacing::layout::HEADER_HEIGHT - 2.0)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: 2.0,
      right: 2.0,
    })
    .style(move |_, status| button::Style {
      text_color: match (is_active, status) {
        (true, _) | (_, button::Status::Hovered | button::Status::Pressed) => color::text::PRIMARY,
        _ => color::text::SECONDARY,
      },
      background: None,
      border: Border::default(),
      shadow: Shadow::default(),
      snap: false,
    })
    .on_press(on_press)
}

fn tab_underline(is_active: bool) -> container::Container<'static, super::Message> {
  container(iced::widget::Space::new().width(Length::Fill).height(2.0))
    .width(Length::Fill)
    .height(2.0)
    .style(move |_| container::Style {
      background: if is_active {
        Some(Background::Color(color::accent::PLASMA))
      } else {
        None
      },
      ..container::Style::default()
    })
}
