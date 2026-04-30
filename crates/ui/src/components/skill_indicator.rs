use iced::{
  Background, Border, Element,
  widget::{container, row},
};

use crate::style::color;

pub struct Component {
  level: u8,
}

impl Component {
  pub fn new(level: u8) -> Self {
    Self {
      level,
    }
  }

  pub fn render<'a, MSG: 'a>(self) -> Element<'a, MSG> {
    let level = self.level;
    let pips: Vec<Element<'_, MSG>> = (1u8..=5)
      .map(|i| {
        container(iced::widget::Space::new())
          .width(6.0)
          .height(6.0)
          .style(move |_| {
            let pip_color = if i <= level {
              color::text::PRIMARY
            } else {
              color::border::SUBTLE
            };
            container::Style {
              background: Some(Background::Color(pip_color)),
              border: Border {
                radius: 1.5.into(),
                ..Border::default()
              },
              ..container::Style::default()
            }
          })
          .into()
      })
      .collect();

    row(pips).spacing(3.0).into()
  }
}
