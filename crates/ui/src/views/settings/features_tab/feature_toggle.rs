//! Toggle control for a single feature flag.

use iced::{
  Background, Border, Element, Padding,
  alignment::Vertical,
  widget::{Space, button, container},
};

use super::{Feature, Message};
use crate::style::{color, component, radius};

/// Builder for the feature toggle control.
pub struct FeatureToggle {
  feature: Feature,
  on: bool,
}

impl FeatureToggle {
  /// Create a new [`FeatureToggle`] builder for the given state and feature.
  pub fn new(on: bool, feature: Feature) -> Self {
    Self {
      feature,
      on,
    }
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'static, Message> {
    render_toggle(self.on, self.feature)
  }
}

fn toggle_thumb(on: bool) -> container::Container<'static, Message> {
  let thumb_color = if on {
    color::state::TOGGLE_THUMB
  } else {
    color::text::MEDIUM
  };
  container(Space::new())
    .width(component::toggle::THUMB_SIZE)
    .height(component::toggle::THUMB_SIZE)
    .style(move |_| container::Style {
      background: Some(Background::Color(thumb_color)),
      border: Border {
        radius: radius::FULL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
}

fn toggle_track(on: bool) -> container::Container<'static, Message> {
  let bg_color = if on {
    color::accent::PLASMA
  } else {
    color::state::PRESSED_OVERLAY
  };
  let border_color = if on {
    color::accent::PLASMA
  } else {
    color::border::DEFAULT
  };
  let thumb_offset = if on {
    component::toggle::THUMB_ON_OFFSET
  } else {
    component::toggle::THUMB_OFF_OFFSET
  };
  let thumb = toggle_thumb(on);
  container(
    container(thumb)
      .padding(Padding {
        top: 2.0,
        bottom: 2.0,
        left: thumb_offset,
        right: 0.0,
      })
      .align_y(Vertical::Center),
  )
  .width(component::toggle::TRACK_WIDTH)
  .height(component::toggle::TRACK_HEIGHT)
  .style(move |_| container::Style {
    background: Some(Background::Color(bg_color)),
    border: Border {
      color: border_color,
      radius: radius::FULL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
}

fn render_toggle(on: bool, feature: Feature) -> Element<'static, Message> {
  let track = toggle_track(on);
  button(track)
    .padding(Padding::ZERO)
    .style(|_, _| button::Style {
      background: None,
      ..button::Style::default()
    })
    .on_press(Message::ToggleFeature(feature))
    .into()
}
