//! Full market entry row for the market transaction table.

use iced::{
  Border, Element, Length, Padding, Theme,
  alignment::Horizontal,
  widget::{Space, column, container, row, text},
};

use super::{qty_badge::QtyBadge, side_badge::SideBadge, type_icon_cell::TypeIconCell};
use crate::{
  format,
  style::{
    color, spacing,
    typography::{body, mono},
  },
  views::wallet::{MarketEntry, market_tab::Message, ts_label},
};

/// Builder for a single market entry row.
pub struct MarketEntryRow<'a> {
  entry: &'a MarketEntry,
  icon: Option<iced::widget::image::Handle>,
}

impl<'a> MarketEntryRow<'a> {
  /// Creates a new market entry row.
  pub fn new(entry: &'a MarketEntry, icon: Option<iced::widget::image::Handle>) -> Self {
    Self {
      entry,
      icon,
    }
  }

  /// Renders the row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let entry = self.entry;
    let is_sell = entry.side == "sell";
    let side_color = if is_sell {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };

    let item_row: Element<'_, Message> = row([
      QtyBadge::new(entry.qty).render(),
      Space::new().width(6.0).into(),
      text(&entry.item)
        .font(body::REGULAR)
        .size(13.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .width(Length::Fill)
        .into(),
    ])
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .into();

    let left_col: Element<'_, Message> = column([
      item_row,
      text(format!("{} / unit", format::fmt_isk(entry.unit)))
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .width(Length::Fill)
    .into();

    let right_col: Element<'_, Message> = column([
      container(
        text(format::fmt_isk(entry.total))
          .font(mono::MEDIUM)
          .size(13.0)
          .style(move |_: &Theme| iced::widget::text::Style {
            color: Some(side_color),
          }),
      )
      .width(Length::Fill)
      .align_x(Horizontal::Right)
      .into(),
      container(
        text(ts_label(entry.ts_secs))
          .font(mono::REGULAR)
          .size(10.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::TERTIARY),
          }),
      )
      .width(Length::Fill)
      .align_x(Horizontal::Right)
      .into(),
    ])
    .width(96.0)
    .into();

    let badge_cell: Element<'_, Message> = container(SideBadge::new(is_sell).render()).width(44.0).into();

    let inner = row([
      badge_cell,
      Space::new().width(10.0).into(),
      TypeIconCell::new(self.icon).render(),
      Space::new().width(10.0).into(),
      left_col,
      Space::new().width(spacing::SPACE_3).into(),
      right_col,
    ])
    .align_y(iced::alignment::Vertical::Center)
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    });

    container(inner)
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
