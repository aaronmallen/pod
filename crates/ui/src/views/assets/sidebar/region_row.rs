//! Region group header row in the sidebar tree.

use iced::{
  Color, Element, Length, Padding, Theme,
  widget::{button, row, text},
};

use super::super::{Message, fmt_qty};
use crate::style::{button as btn_style, color, typography::mono};

fn count_badge(count: u64) -> Element<'static, Message> {
  text(fmt_qty(count))
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    })
    .into()
}

/// Builder for a region group header row in the sidebar.
pub struct Component {
  asset_count: u64,
  collapsed: bool,
  group_key: String,
  region_name: String,
}

impl Component {
  /// Creates a new region row.
  pub fn new(region_name: impl Into<String>, group_key: impl Into<String>, collapsed: bool, asset_count: u64) -> Self {
    Self {
      asset_count,
      collapsed,
      group_key: group_key.into(),
      region_name: region_name.into(),
    }
  }

  /// Renders the region row into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let toggle_glyph = if self.collapsed { "▶" } else { "▼" };
    let group_key = self.group_key.clone();

    let mut row_children: Vec<Element<'_, Message>> = vec![
      text(toggle_glyph)
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      text(self.region_name.clone())
        .font(mono::REGULAR)
        .size(11.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(Color::from_rgba(0.957, 0.949, 0.925, 0.60)),
        })
        .width(Length::Fill)
        .into(),
    ];
    if self.asset_count > 0 {
      row_children.push(count_badge(self.asset_count));
    }

    button(
      row(row_children)
        .spacing(5.0)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 4.0,
      bottom: 4.0,
      left: 12.0,
      right: 12.0,
    })
    .width(Length::Fill)
    .on_press(Message::ToggleSidebarGroup(group_key))
    .style(|_, status| btn_style::list_item_active(false, status))
    .into()
  }
}
