use iced::{
  Background, Border, Element, Length,
  alignment::{Horizontal, Vertical},
  widget::container,
};

use crate::ui::style::{color, radius};

pub fn icon_tile<'a, M>(content: impl Into<Element<'a, M>>, size: f32) -> Element<'a, M>
where
  M: 'a,
{
  container(content)
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod icon_tile {
    use iced::widget::text;

    use super::*;

    #[test]
    fn it_wraps_content_in_a_square_surface() {
      let _el: Element<'_, ()> = icon_tile(text("x"), 40.0);
    }
  }
}
