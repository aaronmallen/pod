//! Stacked bar chart section for the values category breakdown panel.

use iced::{
  Background, Border, Element, Length,
  widget::{Space, container, row},
};

fn bar_segment_for_category(c: &super::super::CategoryValue, total_value: f64) -> Element<'static, Message> {
  let pct = (c.value / total_value * 100.0) as u16;
  let (r, g, b) = cat_color_rgb(&c.category_name);
  let col = iced::Color::from_rgb(r, g, b);
  container(Space::new().width(Length::Fill).height(10.0))
    .width(Length::FillPortion(pct.max(1)))
    .style(move |_| container::Style {
      background: Some(Background::Color(col)),
      ..container::Style::default()
    })
    .into()
}

fn bar_chart_container(bar_segments: Vec<Element<'static, Message>>) -> Element<'static, Message> {
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
    let bar_segments = self.build_bar_segments();
    bar_chart_container(bar_segments)
  }

  fn build_bar_segments(&self) -> Vec<Element<'static, Message>> {
    let mut segments: Vec<Element<'static, Message>> = Vec::new();
    for c in self.cats {
      if c.value <= 0.0 || self.total_value <= 0.0 {
        continue;
      }
      segments.push(bar_segment_for_category(c, self.total_value));
    }
    segments
  }
}
