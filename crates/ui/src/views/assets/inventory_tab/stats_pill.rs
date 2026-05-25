//! Aggregate stats pill shown in the inventory filter bar.

use iced::{
  Background, Border, Element, Padding,
  widget::{Space, container, row},
};

use super::{
  super::{State, asset_value, asset_volume},
  Message,
  stat_label::StatLabel,
};
use crate::{format, style::color};

/// Builder for the aggregate stats pill.
pub struct StatsPill<'a> {
  state: &'a State,
}

impl<'a> StatsPill<'a> {
  /// Creates a new stats pill for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the stats pill into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let total_value = self.state.visible_assets().map(asset_value).sum::<f64>();
    let total_volume = self.state.visible_assets().map(asset_volume).sum::<f64>();
    let total_rows = self.state.visible_assets().count();
    container(
      row([
        StatLabel::new("Rows", total_rows.to_string()).render(),
        Space::new().width(18.0).into(),
        StatLabel::new("Value", format::fmt_isk(total_value)).render(),
        Space::new().width(18.0).into(),
        StatLabel::new("Volume", format::fmt_vol(total_volume)).render(),
      ])
      .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 6.0,
      bottom: 6.0,
      left: 14.0,
      right: 14.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 6.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }
}
