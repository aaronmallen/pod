//! Level pip ladder + roman numeral transition component.

use iced::{
  Background, Border, Color, Element,
  alignment::Vertical,
  widget::{Space, container, row, text},
};

use super::super::Message;
use crate::style::{color, typography::mono};

pub struct Component {
  from_level: u8,
  to_level: u8,
}

impl Component {
  pub fn new(from_level: u8, to_level: u8) -> Self {
    Self {
      from_level,
      to_level,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    row([
      pip_ladder(self.from_level, self.to_level),
      Space::new().width(14.0).into(),
      text(format!(
        "{} → {}",
        if self.from_level > 0 {
          roman(self.from_level)
        } else {
          "0"
        },
        roman(self.to_level)
      ))
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    ])
    .align_y(Vertical::Center)
    .into()
  }
}

fn pip_colors(i: u8, current: u8, target: u8) -> (Color, Color) {
  if i <= current {
    (color::text::PRIMARY, color::text::PRIMARY)
  } else if i == target {
    (color::accent::PLASMA, color::accent::PLASMA)
  } else if i < target {
    (color::accent::PLASMA_MUTED, color::accent::PLASMA_HALF)
  } else {
    (Color::TRANSPARENT, color::border::SUBTLE)
  }
}

pub fn pip_ladder<'a>(current: u8, target: u8) -> Element<'a, Message> {
  let pips: Vec<Element<'_, Message>> = (1u8..=5)
    .map(|i| {
      let (bg, border) = pip_colors(i, current, target);
      container(Space::new())
        .width(12.0)
        .height(8.0)
        .style(move |_| container::Style {
          background: Some(Background::Color(bg)),
          border: Border {
            color: border,
            radius: 1.5.into(),
            width: 1.0,
          },
          ..container::Style::default()
        })
        .into()
    })
    .collect();

  row(pips).spacing(3.0).into()
}

pub fn roman(n: u8) -> &'static str {
  const NUMERALS: [&str; 6] = ["?", "I", "II", "III", "IV", "V"];
  NUMERALS.get(n as usize).copied().unwrap_or("?")
}
