use iced::{
  Background, Border, Color, Element, Length,
  widget::{Row, Space, container},
};

use crate::ui::style::{color, radius};

const TRACK_ALPHA: f32 = 0.1;

pub fn progress_bar<'a, M>(fraction: f32, fill: Color, height: f32) -> Element<'a, M>
where
  M: 'a,
{
  let filled = (fraction.clamp(0.0, 1.0) * 1000.0) as u16;
  let remaining = 1000u16.saturating_sub(filled);

  let fill_seg = container(Space::new().width(Length::Fill).height(Length::Fill))
    .width(portion(filled))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let track = Row::with_children(vec![
    fill_seg.into(),
    Space::new().width(portion(remaining)).height(Length::Fill).into(),
  ])
  .height(Length::Fill);

  container(track)
    .width(Length::Fill)
    .height(Length::Fixed(height))
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, TRACK_ALPHA))),
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

// A `FillPortion(0)` segment resolves to the FULL available width (every
// `FillPortion(_)` resolves to the layout max, and a 0-factor child is laid out
// as non-fluid against all remaining space), which would paint a 0% bar fully
// filled and a 100% bar empty. Use a true `Fixed(0.0)` for the empty side.
fn portion(factor: u16) -> Length {
  if factor == 0 {
    Length::Fixed(0.0)
  } else {
    Length::FillPortion(factor)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod portion {
    use iced::Length;

    use super::super::portion;

    #[test]
    fn it_uses_a_fill_portion_for_a_non_empty_segment() {
      assert_eq!(portion(1), Length::FillPortion(1));
      assert_eq!(portion(1000), Length::FillPortion(1000));
    }

    #[test]
    fn it_uses_a_true_zero_width_for_an_empty_segment() {
      assert_eq!(portion(0), Length::Fixed(0.0));
    }
  }

  mod progress_bar {
    use super::*;
    use crate::ui::style::color;

    #[test]
    fn it_clamps_out_of_range_fractions() {
      let _under: Element<'_, ()> = progress_bar(-1.0, color::accent::PLASMA, 4.0);
      let _over: Element<'_, ()> = progress_bar(2.0, color::accent::PLASMA, 4.0);
    }

    #[test]
    fn it_renders_a_partial_fill() {
      let _el: Element<'_, ()> = progress_bar(0.5, color::accent::PLASMA, 2.0);
    }

    #[test]
    fn it_renders_empty_and_full_boundaries() {
      let _empty: Element<'_, ()> = progress_bar(0.0, color::accent::PLASMA, 2.0);
      let _full: Element<'_, ()> = progress_bar(1.0, color::accent::PLASMA, 2.0);
    }
  }
}
