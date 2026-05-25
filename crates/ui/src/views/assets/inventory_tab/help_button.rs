//! Help toggle button for the inventory filter bar.

use iced::{Element, Length};

use super::Message;
use crate::{components, style::color};

/// Builder for the help toggle button.
pub struct HelpButton {
  open: bool,
}

impl HelpButton {
  /// Creates a new help button with the given open state.
  pub fn new(open: bool) -> Self {
    Self {
      open,
    }
  }

  /// Renders the help button into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let icon_color = if self.open {
      color::accent::PLASMA
    } else {
      color::text::SECONDARY
    };
    components::Button::ghost(
      iced::widget::container(
        components::Icon::help()
          .size(13.0)
          .color(icon_color)
          .render::<Message>(),
      )
      .center_x(Length::Fill)
      .center_y(Length::Fill),
    )
    .width(26.0)
    .height(24.0)
    .padding(0)
    .on_press(Message::HelpToggle)
    .into()
  }
}
