use std::{
  borrow::Cow,
  collections::HashSet,
  sync::{LazyLock, Mutex},
};

use iced::{Border, widget::container};

use crate::ui::style::color;

pub(super) const STANDING_MAX: f64 = 10.0;
pub(super) const STANDING_HIGH: f64 = 5.0;

static INTERNED: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

pub(in crate::features::roster) fn static_text(value: Cow<'static, str>) -> &'static str {
  match value {
    Cow::Borrowed(text) => text,
    Cow::Owned(text) => {
      let mut interned = INTERNED.lock().expect("interned localized-string pool poisoned");
      if let Some(existing) = interned.get(text.as_str()) {
        return existing;
      }
      let leaked: &'static str = Box::leak(text.into_boxed_str());
      interned.insert(leaked);
      leaked
    }
  }
}

pub(super) fn row_rule_style(width: f32) -> container::Style {
  container::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.06),
      width,
      ..Border::default()
    },
    ..container::Style::default()
  }
}

pub(super) fn standing_color(value: f64) -> iced::Color {
  if value >= STANDING_HIGH {
    color::status::ONLINE
  } else if value > 0.0 {
    color::with_alpha(color::status::ONLINE, 0.65)
  } else if value >= 0.0 {
    color::text::secondary()
  } else if value > -STANDING_HIGH {
    color::with_alpha(color::status::DANGER, 0.65)
  } else {
    color::status::DANGER
  }
}

#[cfg(test)]
mod tests {
  mod standing_color {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_reads_highsec_positive_as_green() {
      assert_eq!(standing_color(7.0), color::status::ONLINE);
    }

    #[test]
    fn it_reads_neutral_as_secondary() {
      assert_eq!(standing_color(0.0), color::text::secondary());
    }

    #[test]
    fn it_reads_strong_negative_as_danger() {
      assert_eq!(standing_color(-8.0), color::status::DANGER);
    }
  }
}
