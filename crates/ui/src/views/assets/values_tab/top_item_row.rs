//! Row builder for a single item in the top-items-by-value list.

use std::collections::HashMap;

use iced::{
  Background, Border, Color, ContentFit, Element, Length, Padding, Theme,
  widget::{Space, column, container, image, row, text},
};

use super::{
  super::{TopItem, cat_color_rgb},
  Message,
};
use crate::{
  format,
  style::{
    color,
    typography::{body, mono},
  },
};

/// Builder for a single row in the top-items-by-value list.
pub struct TopItemRow<'a> {
  /// The cached item icon handles keyed by (type_id, variant).
  icons: &'a HashMap<(i32, String), image::Handle>,
  /// The item to display.
  item: &'a TopItem,
  /// The 0-based rank of this item in the list.
  rank: usize,
}

impl<'a> TopItemRow<'a> {
  /// Creates a new row builder for the given item and rank.
  pub fn new(rank: usize, item: &'a TopItem, icons: &'a HashMap<(i32, String), image::Handle>) -> Self {
    Self {
      icons,
      item,
      rank,
    }
  }

  /// Renders the row into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let rank_str = format!("{:02}", self.rank + 1);
    let group_label = format!(
      "{} · ×{}",
      category_display_name(&self.item.category_name),
      format::fmt_count(self.item.total_quantity as u64)
    );

    container(
      row([
        top_item_rank_cell(rank_str),
        Space::new().width(4.0).into(),
        top_item_icon(self.item, self.icons),
        Space::new().width(10.0).into(),
        top_item_name_col(self.item.type_name.clone(), group_label),
        top_item_isk_cell(format::fmt_isk(self.item.value)),
      ])
      .align_y(iced::alignment::Vertical::Center)
      .padding(Padding {
        top: 10.0,
        bottom: 10.0,
        left: 18.0,
        right: 18.0,
      }),
    )
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
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

fn top_item_icon(item: &TopItem, icons: &HashMap<(i32, String), image::Handle>) -> Element<'static, Message> {
  let (r, g, b) = cat_color_rgb(&item.category_name);
  let col = Color::from_rgb(r, g, b);
  if let Some(handle) = icons.get(&(item.type_id, "icon".to_string())) {
    container(
      image(handle.clone())
        .width(24.0)
        .height(24.0)
        .content_fit(ContentFit::Cover),
    )
    .width(24.0)
    .height(24.0)
    .style(|_| container::Style {
      border: Border {
        radius: 4.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .clip(true)
    .into()
  } else {
    container(Space::new().width(24.0).height(24.0))
      .style(move |_| container::Style {
        background: Some(Background::Color(color::with_alpha(col, 0.18))),
        border: Border {
          radius: 4.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  }
}

fn top_item_name_col(type_name: String, group_label: String) -> Element<'static, Message> {
  column([
    text(type_name)
      .font(body::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(group_label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .width(Length::Fill)
  .into()
}

fn top_item_isk_cell(isk: String) -> Element<'static, Message> {
  text(isk)
    .font(mono::MEDIUM)
    .size(12.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::accent::PLASMA),
    })
    .into()
}

fn top_item_rank_cell(rank_str: String) -> Element<'static, Message> {
  text(rank_str)
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    })
    .width(18.0)
    .into()
}
