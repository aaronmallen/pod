//! Legend row for a single category in the values chart.

use iced::{
  Background, Border, Element, Length, Theme,
  widget::{Space, container, row, text},
};

use super::{
  super::{CategoryValue, cat_color_rgb},
  Message,
};
use crate::style::{
  color,
  typography::{body, mono},
};

/// Builder for a single category legend row in the values chart.
pub struct ValuesChartLegendItem<'a> {
  /// The category value to display.
  category: &'a CategoryValue,
}

impl<'a> ValuesChartLegendItem<'a> {
  /// Creates a new legend item builder for the given category value.
  pub fn new(category: &'a CategoryValue) -> Self {
    Self {
      category,
    }
  }

  /// Renders the legend item into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let c = self.category;
    let (r, g, b) = cat_color_rgb(&c.category_name);
    let col = iced::Color::from_rgb(r, g, b);
    let display = category_display_name(&c.category_name);
    let isk = crate::format::fmt_isk(c.value);
    let pct_str = format!("{:.1}%", c.pct * 100.0);

    row([
      container(Space::new().width(10.0).height(10.0))
        .style(move |_| container::Style {
          background: Some(Background::Color(col)),
          border: Border {
            radius: 2.0.into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into(),
      Space::new().width(10.0).into(),
      text(display)
        .font(body::REGULAR)
        .size(12.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .width(Length::Fill)
        .into(),
      text(isk)
        .font(mono::REGULAR)
        .size(11.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      Space::new().width(8.0).into(),
      text(pct_str)
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .width(44.0)
        .into(),
    ])
    .align_y(iced::alignment::Vertical::Center)
    .into()
  }
}

fn category_display_name(key: &str) -> &'static str {
  match key {
    "ship" => "Ships",
    "module" => "Modules",
    "drone" => "Drones",
    "charge" => "Charges",
    "implant" => "Implants",
    "blueprint" => "Blueprints",
    "material" => "Materials",
    "book" => "Skill Books",
    "commodity" => "Commodities",
    _ => "Other",
  }
}
