//! Feature row component for the features settings panel.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, text},
};

use super::{FlagData, Message};
use crate::style::{color, radius};

/// Builder for a single feature-flag row in the features settings panel.
pub struct FeatureRow {
  flag: FlagData,
}

impl FeatureRow {
  /// Create a new feature row builder for the given flag data.
  pub fn new(flag: FlagData) -> Self {
    Self {
      flag,
    }
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'static, Message> {
    render_feature_row(self.flag)
  }
}

fn feature_esi_chip() -> Element<'static, Message> {
  container(text("ESI").size(9.0).color(color::accent::PLASMA))
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: 6.0,
      right: 6.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::PLASMA_SUBTLE)),
      border: Border {
        color: color::state::SELECTION,
        radius: radius::CHIP.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn render_feature_row(flag: FlagData) -> Element<'static, Message> {
  let title = text(flag.title).size(14.0).color(color::text::PRIMARY);
  let description = text(flag.description).size(12.0).color(color::text::SECONDARY);
  let toggle = super::render_toggle(flag.enabled, flag.feature);
  let bottom_border = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });
  column([
    row([
      column([
        row([title.into(), feature_esi_chip()])
          .spacing(10.0)
          .align_y(Vertical::Center)
          .into(),
        Space::new().height(4.0).into(),
        description.into(),
      ])
      .into(),
      Space::new().width(Length::Fill).into(),
      container(toggle).align_y(Vertical::Center).height(Length::Fill).into(),
    ])
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 16.0,
      bottom: 16.0,
      left: 4.0,
      right: 4.0,
    })
    .into(),
    bottom_border.into(),
  ])
  .into()
}
