use iced::{Background, Border, Color, Element, Length, widget::container};

use crate::style::color;

pub struct Component {
  value: f32,
  fill_color: Color,
  height: f32,
  radius: f32,
  total_width: Length,
}

impl Component {
  pub fn new(value: f32) -> Self {
    Self {
      value,
      fill_color: color::accent::PLASMA,
      height: 4.0,
      radius: 2.0,
      total_width: Length::Fill,
    }
  }

  pub fn fill_color(mut self, c: Color) -> Self {
    self.fill_color = c;
    self
  }

  pub fn height(mut self, h: f32) -> Self {
    self.height = h;
    self
  }

  pub fn radius(mut self, r: f32) -> Self {
    self.radius = r;
    self
  }

  pub fn total_width(mut self, w: Length) -> Self {
    self.total_width = w;
    self
  }

  pub fn render<'a, MSG: 'a>(self) -> Element<'a, MSG> {
    let pct = (self.value.clamp(0.0, 1.0) * 100.0) as u16;
    let rest = 100u16.saturating_sub(pct);
    let fill_color = self.fill_color;
    let height = self.height;
    let total_width = self.total_width;
    let radius = self.radius;

    let fill = container(iced::widget::Space::new().width(Length::Fill).height(height))
      .width(Length::FillPortion(pct))
      .height(height)
      .style(move |_| container::Style {
        background: Some(Background::Color(fill_color)),
        ..container::Style::default()
      });

    let remainder = container(iced::widget::Space::new().width(Length::Fill).height(height))
      .width(Length::FillPortion(rest))
      .height(height);

    container(iced::widget::row![fill, remainder].width(total_width).height(height))
      .width(total_width)
      .height(height)
      .style(move |_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        border: Border {
          radius: radius.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  }
}
