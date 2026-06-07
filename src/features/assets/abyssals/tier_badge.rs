use iced::{Color, Element};

use crate::{features::assets::Message, ui::components::badge::badge};

pub(super) fn view(tier: &str) -> Element<'static, Message> {
  badge(tier.to_uppercase(), Some(tier_color(tier)))
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

fn tier_color(tier: &str) -> Color {
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

  mod tier_color {
    use super::*;

    #[test]
    fn it_returns_a_color_for_decayed() {
      let col = tier_color("Decayed");

      assert!(col.r + col.g + col.b > 0.0);
    }

    #[test]
    fn it_returns_a_color_for_glorified_unstable() {
      let col = tier_color("Glorified Unstable");

      assert!(col.r + col.g + col.b > 0.0);
    }

    #[test]
    fn it_returns_a_color_for_unstable() {
      let col = tier_color("Unstable");

      assert!(col.r > 0.5);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_badge() {
      let _el: Element<'static, Message> = view("Gravid");
    }
  }
}
