//! Category breakdown panel: stacked bar and legend for the values tab.

use iced::{
  Background, Border, Element, Padding, Theme,
  widget::{Space, column, container, text},
};

use super::{
  super::CategoryValue, Message, chart_legend_item::ValuesChartLegendItem, chart_section::ValuesChartSection,
};
use crate::{
  format,
  style::{
    color,
    typography::{body, mono},
  },
};

/// Builder for the category breakdown panel.
pub struct Component<'a> {
  /// The category values to display.
  cats: &'a [CategoryValue],
  /// The pre-computed total asset value.
  total_value: f64,
}

impl<'a> Component<'a> {
  /// Creates a new category panel builder.
  pub fn new(cats: &'a [CategoryValue], total_value: f64) -> Self {
    Self {
      cats,
      total_value,
    }
  }

  /// Renders the category panel into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let stacked_bar = ValuesChartSection::new(self.cats, self.total_value).render();

    let legend_rows: Vec<Element<'static, Message>> = self
      .cats
      .iter()
      .filter(|c| c.value > 0.0)
      .map(|c| ValuesChartLegendItem::new(c).render())
      .collect();

    container(
      column([
        text("By category")
          .font(body::MEDIUM)
          .size(14.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        Space::new().height(4.0).into(),
        text(format!("{} ISK total", format::fmt_isk_full(self.total_value)))
          .font(mono::REGULAR)
          .size(9.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        Space::new().height(14.0).into(),
        stacked_bar,
        Space::new().height(14.0).into(),
        column(legend_rows).spacing(6.0).into(),
      ])
      .padding(Padding {
        top: 16.0,
        bottom: 16.0,
        left: 18.0,
        right: 18.0,
      }),
    )
    .width(360.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }
}
