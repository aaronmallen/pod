use iced::{
  Background, Border, Element,
  widget::{container, text},
};

use crate::ui::style::{color, typography};

const BADGE_RADIUS: f32 = 4.0;
const BADGE_SIZE: f32 = 26.0;
const BORDER_ALPHA: f32 = 0.3;
const BORDER_WIDTH: f32 = 1.0;
const FILL_ALPHA: f32 = 0.1;

pub fn step_badge<'a, M>(count: usize) -> Element<'a, M>
where
  M: 'a,
{
  let accent = color::accent();

  container(
    text(count.to_string())
      .font(typography::mono::SEMIBOLD)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(accent),
      }),
  )
  .width(BADGE_SIZE)
  .height(BADGE_SIZE)
  .center_x(BADGE_SIZE)
  .center_y(BADGE_SIZE)
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(accent, FILL_ALPHA))),
    border: Border {
      color: color::with_alpha(accent, BORDER_ALPHA),
      width: BORDER_WIDTH,
      radius: BADGE_RADIUS.into(),
    },
    ..container::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod step_badge {
    use super::*;

    #[test]
    fn it_renders_a_step_count() {
      let _el: Element<'_, ()> = step_badge(12);
    }

    #[test]
    fn it_renders_a_zero_count() {
      let _el: Element<'_, ()> = step_badge(0);
    }
  }
}
