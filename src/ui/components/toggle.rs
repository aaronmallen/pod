use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, container},
};

use crate::ui::style::color;

const THUMB_INSET: f32 = 2.0;
const THUMB_SIZE: f32 = 14.0;
const TRACK_HEIGHT: f32 = 22.0;
const TRACK_WIDTH: f32 = 38.0;

pub fn toggle<'a, M>(on: bool, on_toggle: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let thumb_left = if on {
    TRACK_WIDTH - THUMB_SIZE - THUMB_INSET
  } else {
    THUMB_INSET
  };
  let thumb_color = if on {
    color::surface::NAVIGATION
  } else {
    color::with_alpha(color::text::PRIMARY, 0.65)
  };

  let thumb = container(
    container(Space::new())
      .width(Length::Fixed(THUMB_SIZE))
      .height(Length::Fixed(THUMB_SIZE))
      .style(move |_| container::Style {
        background: Some(Background::Color(thumb_color)),
        border: Border {
          radius: (THUMB_SIZE / 2.0).into(),
          ..Border::default()
        },
        ..container::Style::default()
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Left)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 0.0,
    right: 0.0,
    bottom: 0.0,
    left: thumb_left,
  });

  let track = container(thumb)
    .width(Length::Fixed(TRACK_WIDTH))
    .height(Length::Fixed(TRACK_HEIGHT))
    .style(move |_| {
      let (background, border_color) = if on {
        (color::accent::PLASMA, color::accent::PLASMA)
      } else {
        (color::with_alpha(color::text::PRIMARY, 0.08), color::rule_strong())
      };
      container::Style {
        background: Some(Background::Color(background)),
        border: Border {
          color: border_color,
          width: 1.0,
          radius: (TRACK_HEIGHT / 2.0).into(),
        },
        ..container::Style::default()
      }
    });

  button(track)
    .padding(0)
    .on_press(on_toggle)
    .style(|_, _| button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      ..button::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod toggle {
    use super::*;

    #[test]
    fn it_renders_an_on_switch() {
      let _el: Element<'_, i32> = toggle(true, 1);
    }

    #[test]
    fn it_renders_an_off_switch() {
      let _el: Element<'_, i32> = toggle(false, 0);
    }
  }
}
