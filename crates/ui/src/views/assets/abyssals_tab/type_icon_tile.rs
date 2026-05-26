//! EVE module type icon tile — shows a live icon or a generated monogram fallback.

use iced::{
  Background, Border, Color, Element, Length, Theme,
  widget::{container, image, text},
};

use super::Message;
use crate::style::{color, typography::mono};

/// Builder for a type icon tile widget.
///
/// Renders the loaded EVE icon for a module type when available; falls back to
/// a generated monogram tile seeded from the type ID and name.
pub struct Component {
  /// The human-readable base type name (used for monogram generation).
  pub base_type_name: String,
  /// The height of the tile in logical pixels.
  pub height: f32,
  /// Pre-loaded icon handle for this type, if available.
  pub icon: Option<image::Handle>,
  /// The EVE type ID (used for monogram seed and fallback colour).
  pub type_id: i32,
  /// The width of the tile in logical pixels.
  pub width: f32,
}

impl Component {
  /// Creates a new type icon tile.
  pub fn new(base_type_name: impl Into<String>, type_id: i32, width: f32, height: f32) -> Self {
    Self {
      base_type_name: base_type_name.into(),
      height,
      icon: None,
      type_id,
      width,
    }
  }

  /// Attaches a pre-loaded icon handle to the tile.
  pub fn icon(mut self, handle: Option<image::Handle>) -> Self {
    self.icon = handle;
    self
  }

  /// Renders the tile into a static iced element.
  pub fn render(self) -> Element<'static, Message> {
    let w = self.width;
    let h = self.height;
    let radius = if w >= 28.0 { 6.0 } else { 4.0 };
    if let Some(handle) = self.icon {
      container(image::Image::new(handle).width(w).height(h))
        .width(w)
        .height(h)
        .style(move |_| container::Style {
          border: Border {
            radius: radius.into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .clip(true)
        .into()
    } else {
      let letters = type_monogram(&self.base_type_name, self.type_id);
      let font_size = if w >= 28.0 { 12.0 } else { 9.0 };
      monogram_tile(&letters, self.type_id, w, h, font_size)
    }
  }
}

fn hue_to_color(hue: f32, lightness: f32, saturation: f32) -> Color {
  Color::from([
    lightness + saturation * (hue.to_radians()).cos(),
    lightness + saturation * (hue.to_radians() + 2.094).cos(),
    lightness + saturation * (hue.to_radians() + 4.189).cos(),
    1.0,
  ])
}

fn is_alphabetic_word(w: &&str) -> bool {
  w.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false)
}

fn monogram_tile(label: &str, seed: i32, width: f32, height: f32, font_size: f32) -> Element<'static, Message> {
  let hue = (seed.unsigned_abs() % 360) as f32;
  let col = hue_to_color(hue, 0.5, 0.3);
  container(
    text(label.to_string())
      .font(mono::REGULAR)
      .size(font_size)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(Color::WHITE),
      }),
  )
  .width(width)
  .height(height)
  .center(Length::Fill)
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(col, 0.8))),
    border: Border {
      radius: 6.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn type_monogram(base_type_name: &str, type_id: i32) -> String {
  let letters: String = base_type_name
    .split_whitespace()
    .filter(is_alphabetic_word)
    .take(2)
    .map(word_initial)
    .collect();
  if letters.is_empty() {
    format!("{}", type_id % 100)
  } else {
    letters
  }
}

fn word_initial(w: &str) -> char {
  w.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?')
}

#[cfg(test)]
mod tests {
  use super::*;

  mod is_alphabetic_word {
    use super::*;

    #[test]
    fn it_returns_true_for_alphabetic_start() {
      assert!(is_alphabetic_word(&&"Shield"));
    }

    #[test]
    fn it_returns_false_for_numeric_start() {
      assert!(!is_alphabetic_word(&&"123"));
    }

    #[test]
    fn it_returns_false_for_empty_string() {
      assert!(!is_alphabetic_word(&&""));
    }
  }

  mod type_monogram {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_initials_for_multi_word_name() {
      assert_eq!(type_monogram("Shield Booster", 47785), "SB");
    }

    #[test]
    fn it_returns_single_initial_for_one_word_name() {
      assert_eq!(type_monogram("Afterburner", 47749), "A");
    }

    #[test]
    fn it_returns_type_id_mod_100_for_non_alphabetic_name() {
      assert_eq!(type_monogram("123 456", 12345), "45");
    }

    #[test]
    fn it_takes_at_most_two_words() {
      assert_eq!(type_monogram("Shield Booster X-Large Extra", 1), "SB");
    }
  }

  mod word_initial {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_uppercase_first_char() {
      assert_eq!(word_initial("shield"), 'S');
    }

    #[test]
    fn it_returns_question_mark_for_empty_string() {
      assert_eq!(word_initial(""), '?');
    }
  }
}
