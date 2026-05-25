//! Stacked bar chart section for the values category breakdown panel.

use iced::{
  Background, Border, Element, Length,
  widget::{Space, container, row},
};

use super::{
  super::{CategoryValue, cat_color_rgb},
  Message,
};
use crate::style::color;

/// Builder for the stacked bar chart section of the category breakdown panel.
pub struct ValuesChartSection<'a> {
  /// The category values to render as bar segments.
  cats: &'a [CategoryValue],
  /// The pre-computed total asset value.
  total_value: f64,
}

impl<'a> ValuesChartSection<'a> {
  /// Creates a new chart section builder.
  pub fn new(cats: &'a [CategoryValue], total_value: f64) -> Self {
    Self {
      cats,
      total_value,
    }
  }

  /// Renders the stacked bar chart into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let mut bar_segments: Vec<Element<'static, Message>> = Vec::new();
    for c in self.cats {
      if c.value <= 0.0 || self.total_value <= 0.0 {
        continue;
      }
      let pct = (c.value / self.total_value * 100.0) as u16;
      let (r, g, b) = cat_color_rgb(&c.category_name);
      let col = iced::Color::from_rgb(r, g, b);
      bar_segments.push(
        container(Space::new().width(Length::Fill).height(10.0))
          .width(Length::FillPortion(pct.max(1)))
          .style(move |_| container::Style {
            background: Some(Background::Color(col)),
            ..container::Style::default()
          })
          .into(),
      );
    }

    container(row(bar_segments).width(Length::Fill).height(10.0))
      .width(Length::Fill)
      .height(10.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        border: Border {
          radius: 5.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  }
}
