#![allow(dead_code)]

use iced::{
  Color, Element, Font, Length,
  alignment::Horizontal,
  widget::{container, text},
};

use crate::ui::style::{color, typography};

pub struct TableCell {
  align: Horizontal,
  clip: bool,
  color: Color,
  content: String,
  font: Font,
  size: f32,
  wrapping: text::Wrapping,
}

impl TableCell {
  pub fn new(content: impl Into<String>) -> Self {
    Self {
      align: Horizontal::Left,
      clip: false,
      color: color::text::secondary(),
      content: content.into(),
      font: typography::body::REGULAR,
      size: typography::size::SM,
      wrapping: text::Wrapping::Word,
    }
  }

  pub fn align(mut self, align: Horizontal) -> Self {
    self.align = align;
    self
  }

  pub fn clip(mut self, clip: bool) -> Self {
    self.clip = clip;
    self
  }

  pub fn color(mut self, color: Color) -> Self {
    self.color = color;
    self
  }

  pub fn font(mut self, font: Font) -> Self {
    self.font = font;
    self
  }

  pub fn size(mut self, size: f32) -> Self {
    self.size = size;
    self
  }

  pub fn view<'a, M>(self) -> Element<'a, M>
  where
    M: 'a,
  {
    container(
      text(self.content)
        .font(self.font)
        .size(self.size)
        .wrapping(self.wrapping)
        .style(typography::colored(self.color)),
    )
    .width(Length::Fill)
    .align_x(self.align)
    .clip(self.clip)
    .into()
  }

  pub fn wrapping(mut self, wrapping: text::Wrapping) -> Self {
    self.wrapping = wrapping;
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod new {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_to_wrapping_and_not_clipping() {
      let cell = TableCell::new("Tritanium");

      assert!(!cell.clip);
      assert_eq!(cell.wrapping, text::Wrapping::Word);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_default_cell() {
      let _el: Element<'_, ()> = TableCell::new("Tritanium").view();
    }

    #[test]
    fn it_renders_a_right_aligned_numeric_cell_with_explicit_overrides() {
      let _el: Element<'_, ()> = TableCell::new("1,024")
        .font(typography::mono::REGULAR)
        .align(Horizontal::Right)
        .clip(true)
        .wrapping(text::Wrapping::None)
        .color(color::text::PRIMARY)
        .view();
    }
  }
}
