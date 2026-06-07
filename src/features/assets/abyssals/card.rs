use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, mouse_area, text},
};

use super::{AbyssalCard, stat_row, tier_badge, type_icon_tile};
use crate::{
  features::assets::{Message, fmt_isk},
  store::images,
  ui::{
    components::{avatar::Avatar, card::card, icon::Icon},
    style::{color, radius, spacing, typography},
  },
};

const AVATAR_SIZE: f32 = 18.0;
const CARD_WIDTH: f32 = 440.0;

pub(super) fn view(card_data: &AbyssalCard) -> Element<'_, Message> {
  let body = Column::with_children(vec![header(card_data), stats(card_data), footer(card_data)]).width(Length::Fill);

  container(card(body)).width(Length::Fixed(CARD_WIDTH)).into()
}

fn footer(card_data: &AbyssalCard) -> Element<'_, Message> {
  let portrait_path = images::default_store().character_portrait_path(card_data.character_id);
  let portrait = portrait_path.exists().then_some(portrait_path);
  let portrait_tile = Avatar::new(
    card_data.character_id,
    card_data.owner_name.clone(),
    Length::Fixed(AVATAR_SIZE),
    AVATAR_SIZE,
    portrait,
  )
  .radius(AVATAR_SIZE / 2.0)
  .view();

  let mut children: Vec<Element<'_, Message>> = vec![
    portrait_tile,
    Space::new().width(spacing::SPACE_2).into(),
    text(card_data.owner_name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ];

  if card_data.location.is_empty() {
    children.push(Space::new().width(Length::Fill).into());
  } else {
    children.push(Space::new().width(spacing::SPACE_2).into());
    children.push(
      text("\u{00b7}")
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
    );
    children.push(Space::new().width(spacing::SPACE_2).into());
    children.push(
      text(card_data.location.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::TERTIARY),
        })
        .width(Length::Fill)
        .into(),
    );
  }

  container(Row::with_children(children).align_y(Vertical::Center))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        radius: radius::SUBTLE.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn header(card_data: &AbyssalCard) -> Element<'_, Message> {
  let title = text(card_data.module_name.clone())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  let title_col = Column::with_children(vec![title.into(), tier_badge::view(&card_data.tier_label)])
    .spacing(spacing::UNIT + 2.0)
    .width(Length::Fill);

  container(
    Row::with_children(vec![
      type_icon_tile::view(card_data.group_type_id, &card_data.module_name),
      Space::new().width(spacing::SPACE_3).into(),
      title_col.into(),
      price_widget(card_data),
    ])
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3_5,
    right: spacing::SPACE_3_5,
  })
  .into()
}

fn price_widget(card_data: &AbyssalCard) -> Element<'static, Message> {
  if card_data.estimate.is_none() && card_data.price_unavailable {
    return text("Price unavailable")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::status::WARNING),
      })
      .into();
  }

  let price_label = card_data.estimate.map(fmt_isk).unwrap_or_else(|| "\u{2014}".to_owned());

  mouse_area(
    Row::with_children(vec![
      Icon::mutamarket()
        .size(14.0)
        .color(color::status::WARNING)
        .render::<Message>(),
      Space::new().width(spacing::UNIT + 1.0).into(),
      text(price_label)
        .font(typography::mono::MEDIUM)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::accent::PLASMA),
        })
        .into(),
    ])
    .align_y(Vertical::Center),
  )
  .interaction(iced::mouse::Interaction::Pointer)
  .on_press(Message::AbyssalMutaMarketOpened(card_data.item_id))
  .into()
}

fn stats(card_data: &AbyssalCard) -> Element<'_, Message> {
  let rows: Vec<Element<'_, Message>> = card_data.stats.iter().map(stat_row::view).collect();

  container(Column::with_children(rows).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::UNIT,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::features::assets::abyssals::AbyssalStat;

  fn card_with(estimate: Option<f64>, price_unavailable: bool, location: &str) -> AbyssalCard {
    AbyssalCard {
      character_id: 7,
      estimate,
      group_type_id: 2410,
      item_id: 99,
      location: location.to_owned(),
      module_name: "Heavy Assault Missile Launcher II".to_owned(),
      owner_name: "Vex".to_owned(),
      price_unavailable,
      stats: vec![AbyssalStat {
        attribute_id: 50,
        base_value: 47.0,
        bound_hi: 56.0,
        bound_lo: 28.0,
        display_name: "Stasis".to_owned(),
        high_is_good: true,
        rolled: 41.0,
        unit_suffix: " tf".to_owned(),
      }],
      tier_label: "Gravid".to_owned(),
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_priced_card_with_a_location() {
      let card = card_with(Some(1_000_000.0), false, "Jita IV - Moon 4");

      let _el: Element<'_, Message> = view(&card);
    }

    #[test]
    fn it_renders_a_price_unavailable_card_without_a_location() {
      let card = card_with(None, true, "");

      let _el: Element<'_, Message> = view(&card);
    }

    #[test]
    fn it_renders_an_em_dash_card_when_price_is_null() {
      let card = card_with(None, false, "");

      let _el: Element<'_, Message> = view(&card);
    }
  }
}
