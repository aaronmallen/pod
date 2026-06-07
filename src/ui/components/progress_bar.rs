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
    .width(Length::FillPortion(filled))
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
    Space::new()
      .width(Length::FillPortion(remaining))
      .height(Length::Fill)
      .into(),
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

#[cfg(test)]
mod tests {
  use super::*;

  mod progress_bar {
    use super::*;
    use crate::ui::style::color;

    #[test]
    fn it_renders_a_partial_fill() {
      let _el: Element<'_, ()> = progress_bar(0.5, color::accent::PLASMA, 2.0);
    }

    #[test]
    fn it_clamps_out_of_range_fractions() {
      let _under: Element<'_, ()> = progress_bar(-1.0, color::accent::PLASMA, 4.0);
      let _over: Element<'_, ()> = progress_bar(2.0, color::accent::PLASMA, 4.0);
    }
  }
}
