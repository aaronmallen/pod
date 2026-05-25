//! Top items by value panel for the values tab.

use std::collections::HashMap;

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{column, container, image, text},
};

use super::{super::TopItem, Message, top_item_row::TopItemRow};
use crate::style::{color, typography::body};

/// Builder for the top items by value panel.
pub struct Component<'a> {
  /// The cached item icon handles keyed by (type_id, variant).
  icons: &'a HashMap<(i32, String), image::Handle>,
  /// The top items list.
  items: &'a [TopItem],
}

impl<'a> Component<'a> {
  /// Creates a new top items panel builder.
  pub fn new(items: &'a [TopItem], icons: &'a HashMap<(i32, String), image::Handle>) -> Self {
    Self {
      icons,
      items,
    }
  }

  /// Renders the top items panel into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let title_row: Element<'static, Message> = container(
      text("Top items by value")
        .font(body::MEDIUM)
        .size(14.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        }),
    )
    .padding(Padding {
      top: 14.0,
      bottom: 14.0,
      left: 18.0,
      right: 18.0,
    })
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into();

    let item_rows: Vec<Element<'static, Message>> = self
      .items
      .iter()
      .enumerate()
      .map(|(i, item)| TopItemRow::new(i, item, self.icons).render())
      .collect();

    let mut all_rows: Vec<Element<'static, Message>> = vec![title_row];
    all_rows.extend(item_rows);

    container(column(all_rows))
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
