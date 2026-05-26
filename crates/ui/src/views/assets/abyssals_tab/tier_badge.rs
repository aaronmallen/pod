//! Tier badge component for abyssal module cards.

use iced::{
  Background, Border, Color, Element, Padding, Theme,
  widget::{container, text},
};

use super::Message;
use crate::style::{color, typography::mono};

/// Builder for a colored tier badge pill.
pub struct Component {
  tier: String,
}

impl Component {
  /// Creates a new tier badge for the given tier name.
  pub fn new(tier: &str) -> Self {
    Self {
      tier: tier.to_string(),
    }
  }

  /// Renders the tier badge into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let col = tier_badge_color(&self.tier);
    let tier_label = self.tier.to_uppercase();
    container(
      text(tier_label)
        .font(mono::REGULAR)
        .size(9.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(col),
        }),
    )
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: 7.0,
      right: 7.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(col, 0.12))),
      border: Border {
        color: color::with_alpha(col, 0.45),
        radius: 3.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }
}

fn base_tier_color(lower: &str) -> Color {
  if lower.contains("unstable") {
    Color::from_rgb(0.878, 0.459, 0.349)
  } else if lower.contains("gravid") {
    Color::from_rgb(0.612, 0.408, 0.839)
  } else {
    Color::from_rgb(0.247, 0.600, 0.780)
  }
}

fn glorified_tier_color(lower: &str) -> Option<Color> {
  if lower.contains("unstable") {
    Some(Color::from_rgb(0.741, 0.490, 0.133))
  } else if lower.contains("gravid") {
    Some(Color::from_rgb(0.588, 0.349, 0.792))
  } else if lower.contains("decayed") {
    Some(Color::from_rgb(0.247, 0.557, 0.859))
  } else {
    None
  }
}

fn tier_badge_color(tier: &str) -> Color {
  let lower = tier.to_lowercase();
  if lower.contains("glorified") {
    glorified_tier_color(&lower).unwrap_or_else(|| base_tier_color(&lower))
  } else {
    base_tier_color(&lower)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod tier_badge_color {
    use super::*;

    #[test]
    fn it_returns_a_color_for_decayed() {
      let col = tier_badge_color("Decayed");

      assert!(col.r + col.g + col.b > 0.0);
    }

    #[test]
    fn it_returns_a_color_for_glorified_unstable() {
      let col = tier_badge_color("Glorified Unstable");

      assert!(col.r + col.g + col.b > 0.0);
    }

    #[test]
    fn it_returns_a_color_for_unstable() {
      let col = tier_badge_color("Unstable");

      assert!(col.r > 0.5);
    }
  }
}
