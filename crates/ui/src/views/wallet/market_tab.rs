//! Market transaction table for the wallet main panel.

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  alignment::Horizontal,
  widget::{Space, column, container, image, row, text},
};

use crate::{
  components::DataTable,
  format,
  style::{
    color, radius,
    typography::{body, mono},
  },
  views::wallet::{MarketEntry, SideFilter, State, ts_label},
};

/// Messages produced by the market tab.
#[derive(Clone, Debug)]
pub enum Message {
  SideFilterChanged(SideFilter),
}

fn side_badge(is_sell: bool) -> Element<'static, Message> {
  let side_color = if is_sell {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let side_label = if is_sell { "SELL" } else { "BUY" };
  container(
    text(side_label)
      .font(mono::MEDIUM)
      .size(9.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(side_color),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 6.0,
    right: 6.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(if is_sell {
      color::status::ONLINE_SUBTLE
    } else {
      color::status::DANGER_SUBTLE
    })),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn qty_badge(qty: u64) -> Element<'static, Message> {
  container(
    text(format::fmt_count(qty))
      .font(mono::MEDIUM)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 6.0,
    right: 6.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::state::SUBTLE_FILL)),
    border: Border {
      radius: radius::CHIP.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn type_icon_cell(handle: Option<image::Handle>) -> Element<'static, Message> {
  let size = 32.0f32;
  if let Some(h) = handle {
    container(image::Image::new(h).width(size).height(size))
      .width(size)
      .height(size)
      .into()
  } else {
    container(Space::new().width(size).height(size))
      .width(size)
      .height(size)
      .style(|_| container::Style {
        background: Some(iced::Background::Color(color::state::HOVER_OVERLAY)),
        border: Border {
          radius: 4.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  }
}

fn entry_row<'a>(entry: &'a MarketEntry, icon: Option<image::Handle>) -> Element<'a, Message> {
  use crate::style::spacing;

  let is_sell = entry.side == "sell";
  let side_color = if is_sell {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };

  let item_row: Element<'_, Message> = row([
    qty_badge(entry.qty),
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

  let badge_cell: Element<'_, Message> = container(side_badge(is_sell)).width(44.0).into();

  let inner = row([
    badge_cell,
    Space::new().width(10.0).into(),
    type_icon_cell(icon),
    Space::new().width(10.0).into(),
    left_col,
    Space::new().width(spacing::SPACE_3).into(),
    right_col,
  ])
  .align_y(iced::alignment::Vertical::Center)
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: spacing::SPACE_7,
    right: spacing::SPACE_7,
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

/// Builder for the market transaction table.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new market table component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the market table into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let icons = &self.state.item_icons;
    DataTable::new(self.state.filtered_market.iter(), |e, _, _| {
      entry_row(e, icons.get(&e.type_id).cloned())
    })
    .empty_message("No market entries match your filter.")
    .render()
  }
}
