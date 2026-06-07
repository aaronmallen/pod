use iced::{
  Background, Border, Color, Element, Padding,
  widget::{container, text},
};

use crate::ui::style::{color, spacing, typography};

const PILL_RADIUS: f32 = 999.0;

pub fn count_badge<'a, M>(count: i64, fill: Color) -> Element<'a, M>
where
  M: 'a,
{
  container(
    text(count.to_string())
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(color::surface::BASE),
      }),
  )
  .padding(Padding {
    top: spacing::UNIT / 2.0,
    right: spacing::UNIT + 3.0,
    bottom: spacing::UNIT / 2.0,
    left: spacing::UNIT + 3.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(fill)),
    border: Border {
      radius: PILL_RADIUS.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod count_badge {
    use super::*;
    use crate::ui::style::color;

    #[test]
    fn it_renders_a_numeric_pill() {
      let _el: Element<'_, ()> = count_badge(7, color::accent::PLASMA);
    }
  }
}
