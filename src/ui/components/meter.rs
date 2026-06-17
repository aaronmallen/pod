use iced::{
  Background, Border, Color, Element, Length,
  widget::{Row, Space, container},
};

use crate::ui::style::color;

const RADIUS: f32 = 3.0;
const TRACK_ALPHA: f32 = 0.05;

pub fn diverging<'a, M>(value: f64, max: f64, fill: Color, width: f32, height: f32) -> Element<'a, M>
where
  M: 'a,
{
  let fraction = if max <= 0.0 {
    0.0
  } else {
    (value.abs() / max).clamp(0.0, 1.0) as f32
  };
  let half = width / 2.0;
  let bar = (half * fraction).max(1.0);

  let bar_seg: Element<'a, M> = container(Space::new().width(Length::Fixed(bar)).height(Length::Fixed(height)))
    .height(Length::Fixed(height))
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        radius: RADIUS.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into();

  let left_pad = if value >= 0.0 { half } else { half - bar };

  container(Row::with_children(vec![
    Space::new().width(Length::Fixed(left_pad)).into(),
    bar_seg,
    Space::new().width(Length::Fill).into(),
  ]))
  .width(Length::Fixed(width))
  .height(Length::Fixed(height))
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, TRACK_ALPHA))),
    border: Border {
      radius: RADIUS.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

#[allow(dead_code)]
pub fn single_ended<'a, M>(fraction: f32, fill: Color, height: f32) -> Element<'a, M>
where
  M: 'a,
{
  let filled = (fraction.clamp(0.0, 1.0) * 1000.0) as u16;
  let remaining = 1000u16.saturating_sub(filled);

  let fill_seg = container(Space::new())
    .width(Length::FillPortion(filled))
    .height(Length::Fixed(height))
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      ..container::Style::default()
    });

  container(Row::with_children(vec![
    fill_seg.into(),
    Space::new().width(Length::FillPortion(remaining)).into(),
  ]))
  .width(Length::Fill)
  .height(Length::Fixed(height))
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
    border: Border {
      radius: crate::ui::style::radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod diverging {
    use super::*;
    use crate::ui::style::color;

    #[test]
    fn it_handles_a_zero_max() {
      let _el: Element<'_, ()> = diverging(5.0, 0.0, color::status::ONLINE, 160.0, 6.0);
    }

    #[test]
    fn it_renders_a_negative_value() {
      let _el: Element<'_, ()> = diverging(-8.0, 10.0, color::status::DANGER, 160.0, 6.0);
    }

    #[test]
    fn it_renders_a_positive_value() {
      let _el: Element<'_, ()> = diverging(7.0, 10.0, color::status::ONLINE, 160.0, 6.0);
    }
  }

  mod single_ended {
    use super::*;
    use crate::ui::style::color;

    #[test]
    fn it_clamps_out_of_range_fractions() {
      let _el: Element<'_, ()> = single_ended(1.5, color::accent::PLASMA, 8.0);
    }

    #[test]
    fn it_renders_a_partial_fill() {
      let _el: Element<'_, ()> = single_ended(0.6, color::accent::PLASMA, 8.0);
    }
  }
}
