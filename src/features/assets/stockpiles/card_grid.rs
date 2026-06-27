use std::collections::HashSet;

use iced::{
  Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text},
};

use super::{StockpileCard, card};
use crate::{
  features::assets::{HEADER_SIDE_PADDING, Message, fmt_count},
  ui::{
    components::icon::Icon,
    style::{color, radius, spacing, typography},
  },
};

const EMPTY_COPY_WIDTH: f32 = 360.0;
const EMPTY_ICON_SIZE: f32 = 30.0;
const EMPTY_VERTICAL_PADDING: f32 = 56.0;

pub(super) fn view<'a>(cards: &'a [StockpileCard], expanded: &HashSet<i64>) -> Element<'a, Message> {
  let ready = cards.iter().filter(|c| c.is_full()).count();
  let short = cards.len() - ready;

  let content: Element<'a, Message> = if cards.is_empty() {
    empty_state()
  } else {
    let cells: Vec<Element<'a, Message>> = cards.iter().map(|card| card::view(card, expanded)).collect();
    Row::with_children(cells).spacing(spacing::SPACE_3_5).wrap().into()
  };

  container(
    Column::with_children(vec![header(ready, short), content])
      .spacing(spacing::SPACE_6)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6,
    right: HEADER_SIDE_PADDING,
    bottom: spacing::SPACE_6 + spacing::SPACE_2,
    left: HEADER_SIDE_PADDING,
  })
  .into()
}

fn empty_state<'a>() -> Element<'a, Message> {
  let copy = Column::with_children(vec![
    Icon::stockpiles()
      .color(color::text::tertiary())
      .size(EMPTY_ICON_SIZE)
      .render(),
    text(t!("assets.stockpiles.empty_title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(t!("assets.stockpiles.empty_description").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .width(Length::Fixed(EMPTY_COPY_WIDTH))
      .align_x(Horizontal::Center)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    Row::with_children(vec![new_button(), import_button()])
      .spacing(spacing::SPACE_2_5)
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_x(Horizontal::Center);

  container(copy)
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .padding(Padding {
      top: EMPTY_VERTICAL_PADDING,
      bottom: EMPTY_VERTICAL_PADDING,
      left: spacing::SPACE_6,
      right: spacing::SPACE_6,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.12),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn header<'a>(ready: usize, short: usize) -> Element<'a, Message> {
  Row::with_children(vec![
    text(t!("assets.stockpiles.targets_title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(
      t!(
        "assets.stockpiles.ready_short_summary",
        ready => fmt_count(ready as i64),
        short => fmt_count(short as i64)
      )
      .into_owned(),
    )
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    })
    .into(),
    Space::new().width(Length::Fill).into(),
    new_button(),
    import_button(),
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Center)
  .into()
}

fn import_button<'a>() -> Element<'a, Message> {
  button(
    text(t!("assets.stockpiles.import_multibuy").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: spacing::UNIT + 3.0,
    right: spacing::SPACE_3,
    bottom: spacing::UNIT + 3.0,
    left: spacing::SPACE_3,
  })
  .on_press(Message::StockpileImportOpened)
  .style(|_, _| button::Style {
    background: Some(iced::Background::Color(color::with_alpha(color::text::PRIMARY, 0.04))),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.12),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..button::Style::default()
  })
  .into()
}

fn new_button<'a>() -> Element<'a, Message> {
  button(
    text(t!("assets.stockpiles.new_stockpile_button").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: spacing::UNIT + 3.0,
    right: spacing::SPACE_3,
    bottom: spacing::UNIT + 3.0,
    left: spacing::SPACE_3,
  })
  .on_press(Message::StockpileNew)
  .style(|_, _| button::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.28),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..button::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn card(id: i64, full: bool) -> StockpileCard {
    StockpileCard {
      character_scope: None,
      fill_isk: 0.0,
      id,
      items: vec![super::super::StockpileItemLine {
        have: if full { 1000 } else { 400 },
        pct: if full { 1.0 } else { 0.4 },
        target: 1000,
        type_icon: crate::store::images::IconResolution::Missing,
        type_id: 34,
        type_name: "Tritanium".to_owned(),
      }],
      location_id: None,
      location_name: None,
      name: "Cache".to_owned(),
      overall_pct: if full { 1.0 } else { 0.4 },
      scope_pilots: 0,
      target_isk: 0.0,
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_card_grid_with_a_ready_and_short_caption() {
      let cards = vec![card(1, true), card(2, false)];

      let _el: Element<'_, Message> = view(&cards, &HashSet::new());
    }

    #[test]
    fn it_renders_the_empty_state() {
      let _el: Element<'_, Message> = view(&[], &HashSet::new());
    }
  }
}
