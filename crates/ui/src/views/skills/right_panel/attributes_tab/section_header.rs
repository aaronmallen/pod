//! Section header component for the neural attributes list.

use iced::{
  Element, Length, Padding,
  widget::{Space, column, container, text},
};

use super::Message;
use crate::style::{color, spacing, typography::mono};

/// Header displayed above the attribute bar list, showing the section title
/// and the total points allocated across all attributes.
pub struct SectionHeader {
  /// Total attribute points allocated across all attributes.
  total_pts: u32,
}

impl SectionHeader {
  /// Create a new [`SectionHeader`] with the given total allocated points.
  pub fn new(total_pts: u32) -> Self {
    Self {
      total_pts,
    }
  }

  /// Set the total allocated points.
  pub fn total_pts(mut self, total_pts: u32) -> Self {
    self.total_pts = total_pts;
    self
  }

  /// Render the component into an [`Element`].
  pub fn render(self) -> Element<'static, Message> {
    container(
      column([
        text("Neural attributes")
          .font(mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        Space::new().height(4.0).into(),
        text(format!("{} pts allocated", self.total_pts))
          .font(mono::REGULAR)
          .size(11.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
      ])
      .width(Length::Fill),
    )
    .padding(Padding {
      top: 14.0,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
  }
}
