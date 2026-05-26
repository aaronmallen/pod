//! Abyssal module card — header, stat rows, and footer for one abyssal item.

use std::collections::HashMap;

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Space, column, container, image, row, text},
};
use pod_model::AbyssalViewModel;

use super::Message;
use crate::{
  components::avatar::{self, AvatarKind},
  format,
  style::{
    color,
    typography::{body, mono},
  },
};

fn abyssal_card_header<'a>(
  item: &'a AbyssalViewModel,
  type_icons: &HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  let price_label = item
    .muta_price_isk
    .map(format::fmt_isk)
    .unwrap_or_else(|| "\u{2014}".to_string());
  container(
    row([
      super::type_icon_tile::Component::new(&item.base_type_name, item.source_type_id, 42.0, 42.0)
        .icon(type_icons.get(&item.source_type_id).cloned())
        .render(),
      Space::new().width(12.0).into(),
      column([
        row([
          text(item.base_type_name.clone())
            .font(body::MEDIUM)
            .size(13.0)
            .style(|_: &Theme| iced::widget::text::Style {
              color: Some(color::text::PRIMARY),
            })
            .into(),
          super::tier_badge::Component::new(&item.mutaplasmid_tier).render(),
          Space::new().width(Length::Fill).into(),
        ])
        .align_y(iced::alignment::Vertical::Center)
        .spacing(8.0)
        .into(),
        Space::new().height(2.0).into(),
        text(format!("{} Mutaplasmid", item.mutaplasmid_tier))
          .font(body::REGULAR)
          .size(11.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      ])
      .width(Length::Fill)
      .into(),
      column([text(price_label)
        .font(mono::MEDIUM)
        .size(14.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::ACCENT),
        })
        .into()])
      .align_x(iced::alignment::Horizontal::Right)
      .into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 14.0,
    bottom: 12.0,
    left: 16.0,
    right: 16.0,
  })
  .width(Length::Fill)
  .into()
}

fn abyssal_card_stats(item: &AbyssalViewModel) -> Element<'_, Message> {
  let stat_rows: Vec<Element<'_, Message>> = item
    .stats
    .iter()
    .map(|s| super::stat_row::Component::new(s).render())
    .collect();
  container(column(stat_rows).spacing(2.0))
    .padding(Padding {
      top: 6.0,
      bottom: 14.0,
      left: 16.0,
      right: 16.0,
    })
    .width(Length::Fill)
    .into()
}

fn abyssal_card_footer<'a>(
  item: &'a AbyssalViewModel,
  char_name: &'a str,
  portrait: Option<image::Handle>,
) -> Element<'a, Message> {
  let avatar = avatar::Component::new(
    char_name,
    (item.character_id.unsigned_abs() % 360) as u16,
    18.0,
    AvatarKind::Person,
  )
  .portrait(portrait)
  .render::<Message>();
  let mut row_items: Vec<Element<'_, Message>> = vec![
    avatar,
    Space::new().width(8.0).into(),
    text(char_name.to_string())
      .font(body::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ];
  if !item.location.is_empty() {
    row_items.push(Space::new().width(8.0).into());
    row_items.push(
      text("\u{00b7}")
        .size(11.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
    );
    row_items.push(Space::new().width(8.0).into());
    row_items.push(
      text(item.location.clone())
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .width(Length::Fill)
        .into(),
    );
  } else {
    row_items.push(Space::new().width(Length::Fill).into());
  }
  container(row(row_items).align_y(iced::alignment::Vertical::Center))
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 16.0,
      right: 16.0,
    })
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn abyssal_card<'a>(
  item: &'a AbyssalViewModel,
  char_name: &'a str,
  type_icons: &HashMap<i32, image::Handle>,
  portrait: Option<image::Handle>,
) -> Element<'a, Message> {
  let header = abyssal_card_header(item, type_icons);
  let stats_area = abyssal_card_stats(item);
  let footer = abyssal_card_footer(item, char_name, portrait);
  container(column([header, stats_area, footer]))
    .width(Length::Fill)
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

/// Builder for an abyssal module card.
pub struct Component<'a> {
  char_name: &'a str,
  item: &'a AbyssalViewModel,
  portrait: Option<image::Handle>,
  type_icons: &'a HashMap<i32, image::Handle>,
}

impl<'a> Component<'a> {
  /// Creates a new abyssal card builder.
  pub fn new(
    item: &'a AbyssalViewModel,
    char_name: &'a str,
    type_icons: &'a HashMap<i32, image::Handle>,
    portrait: Option<image::Handle>,
  ) -> Self {
    Self {
      char_name,
      item,
      portrait,
      type_icons,
    }
  }

  /// Renders the abyssal card into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    abyssal_card(self.item, self.char_name, self.type_icons, self.portrait)
  }
}
