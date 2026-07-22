use iced::{
  Background, Border, Element, Theme,
  widget::{container, text},
};

use crate::ui::style::{color, typography};

pub const GLYPH_EXPENSE: &str = "\u{2193}";
pub const GLYPH_INCOME: &str = "\u{2191}";

const BADGE_RADIUS: f32 = 4.0;
const BADGE_SIZE: f32 = 24.0;
const GLYPH_SIZE: f32 = 12.0;
const SUBTLE_ALPHA: f32 = 0.12;

pub struct GlyphBadge<'a> {
  glyph: &'a str,
  is_in: bool,
}

impl<'a> GlyphBadge<'a> {
  pub fn new(glyph: &'a str, is_in: bool) -> Self {
    Self {
      glyph,
      is_in,
    }
  }

  pub fn render<MSG: 'a>(self) -> Element<'a, MSG> {
    let glyph_color = if self.is_in {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };
    let bg_color = color::with_alpha(glyph_color, SUBTLE_ALPHA);
    let glyph = self.glyph.to_owned();

    container(
      text(glyph)
        .font(typography::mono::MEDIUM)
        .size(GLYPH_SIZE)
        .style(move |_: &Theme| text::Style {
          color: Some(glyph_color),
        }),
    )
    .width(BADGE_SIZE)
    .height(BADGE_SIZE)
    .center_x(BADGE_SIZE)
    .center_y(BADGE_SIZE)
    .style(move |_| container::Style {
      background: Some(Background::Color(bg_color)),
      border: Border {
        radius: BADGE_RADIUS.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
  }
}

#[cfg(test)]
mod tests {
  mod render {
    use super::super::*;

    #[test]
    fn it_builds_an_expense_badge() {
      let _el: iced::Element<'_, ()> = GlyphBadge::new(GLYPH_EXPENSE, false).render();
    }

    #[test]
    fn it_builds_an_income_badge() {
      let _el: iced::Element<'_, ()> = GlyphBadge::new(GLYPH_INCOME, true).render();
    }
  }
}
