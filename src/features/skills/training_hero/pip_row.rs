use iced::{Background, Border, Element, Length, widget::Space};

use super::super::{Message, queue_timing::roman};
use crate::ui::style::{color, spacing, typography};

const PIP_GAP: f32 = 3.0;
const PIP_HEIGHT: f32 = 8.0;
const PIP_WIDTH: f32 = 12.0;

pub(in crate::features::skills) fn pip_ladder<'a>(current: u8, target: u8) -> Element<'a, Message> {
  use iced::widget::{Row, container};

  let cells = (1..=5u8).map(|i| {
    let (bg, border) = if i <= current {
      (color::text::PRIMARY, color::text::PRIMARY)
    } else if i == target {
      (color::accent::PLASMA, color::accent::PLASMA)
    } else if i < target {
      (
        color::with_alpha(color::accent::PLASMA, 0.25),
        color::with_alpha(color::accent::PLASMA, 0.5),
      )
    } else {
      (iced::Color::TRANSPARENT, color::with_alpha(color::text::PRIMARY, 0.10))
    };

    container(Space::new())
      .width(Length::Fixed(PIP_WIDTH))
      .height(Length::Fixed(PIP_HEIGHT))
      .style(move |_| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
          color: border,
          width: 1.0,
          radius: 1.5.into(),
        },
        ..container::Style::default()
      })
      .into()
  });

  Row::with_children(cells).spacing(PIP_GAP).into()
}

pub(in crate::features::skills) fn level_transition<'a>(current: u8, target: u8) -> Element<'a, Message> {
  use iced::{
    alignment::Vertical,
    widget::{Row, text},
  };

  let current_label = if current == 0 {
    "0".to_owned()
  } else {
    roman(i64::from(current))
  };
  Row::with_children(vec![
    text(current_label)
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    text("\u{2192}")
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
    text(roman(i64::from(target)))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Center)
  .into()
}
