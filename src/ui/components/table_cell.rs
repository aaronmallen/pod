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
      clip: true,
      color: color::text::SECONDARY,
      content: content.into(),
      font: typography::body::REGULAR,
      size: typography::size::SM,
      wrapping: text::Wrapping::None,
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

  mod table_cell {
    use super::*;

    #[test]
    fn it_renders_a_default_cell() {
      let _el: Element<'_, ()> = TableCell::new("Tritanium").view();
    }

    #[test]
    fn it_renders_a_right_aligned_numeric_cell() {
      let _el: Element<'_, ()> = TableCell::new("1,024")
        .font(typography::mono::REGULAR)
        .align(Horizontal::Right)
        .clip(false)
        .wrapping(text::Wrapping::Word)
        .color(color::text::PRIMARY)
        .view();
    }
  }
}
