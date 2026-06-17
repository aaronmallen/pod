use iced::{
  Background, Border, Color, ContentFit, Element, Length, Theme,
  widget::{container, image, text},
};

use crate::{
  features::assets::Message,
  store::images::IconResolution,
  ui::style::{color, radius, typography},
};

const TILE: f32 = 42.0;

pub(super) fn view(type_icon: &IconResolution, type_id: i64, base_type_name: &str) -> Element<'static, Message> {
  match type_icon {
    IconResolution::Found(path) => container(
      image(image::Handle::from_path(path.clone()))
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Contain),
    )
    .width(Length::Fixed(TILE))
    .height(Length::Fixed(TILE))
    .clip(true)
    .style(|_| container::Style {
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into(),
    IconResolution::Missing => monogram_tile(base_type_name, type_id),
  }
}

fn hue_to_color(hue: f32, lightness: f32, saturation: f32) -> Color {
  Color::from([
    lightness + saturation * hue.to_radians().cos(),
    lightness + saturation * (hue.to_radians() + 2.094).cos(),
    lightness + saturation * (hue.to_radians() + 4.189).cos(),
    1.0,
  ])
}

fn is_alphabetic_word(word: &str) -> bool {
  word.chars().next().is_some_and(char::is_alphabetic)
}

fn monogram_tile(base_type_name: &str, type_id: i64) -> Element<'static, Message> {
  let label = type_monogram(base_type_name, type_id);
  let hue = (type_id.unsigned_abs() % 360) as f32;
  let fill = hue_to_color(hue, 0.5, 0.3);

  container(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(|_: &Theme| text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .width(Length::Fixed(TILE))
  .height(Length::Fixed(TILE))
  .center(Length::Fill)
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(fill, 0.8))),
    border: Border {
      radius: radius::CONTROL.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn type_monogram(base_type_name: &str, type_id: i64) -> String {
  let letters: String = base_type_name
    .split_whitespace()
    .filter(|word| is_alphabetic_word(word))
    .take(2)
    .map(word_initial)
    .collect();
  if letters.is_empty() {
    format!("{}", type_id % 100)
  } else {
    letters
  }
}

fn word_initial(word: &str) -> char {
  word.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?')
}

#[cfg(test)]
mod tests {
  use super::*;

  mod is_alphabetic_word {
    use super::*;

    #[test]
    fn it_returns_false_for_empty_string() {
      assert!(!is_alphabetic_word(""));
    }

    #[test]
    fn it_returns_false_for_numeric_start() {
      assert!(!is_alphabetic_word("123"));
    }

    #[test]
    fn it_returns_true_for_alphabetic_start() {
      assert!(is_alphabetic_word("Shield"));
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

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_monogram_fallback_when_no_icon_is_present() {
      let _el: Element<'static, Message> = view(&IconResolution::Missing, -1, "Shield Booster");
    }
  }

  mod word_initial {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_question_mark_for_empty_string() {
      assert_eq!(word_initial(""), '?');
    }

    #[test]
    fn it_returns_uppercase_first_char() {
      assert_eq!(word_initial("shield"), 'S');
    }
  }
}
