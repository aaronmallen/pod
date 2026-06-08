use iced::{
  Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, text},
};

use super::{StockpileCard, card};
use crate::{
  features::assets::{HEADER_SIDE_PADDING, Message, fmt_count},
  ui::{
    components::empty_state::empty_state as shared_empty_state,
    style::{color, radius, spacing, typography},
  },
};

pub(super) fn view(cards: &[StockpileCard]) -> Element<'_, Message> {
  let ready = cards.iter().filter(|c| c.is_full()).count();
  let short = cards.len() - ready;

  let content: Element<'_, Message> = if cards.is_empty() {
    shared_empty_state("No stockpiles yet")
      .subtitle("Create one to track target quantities.")
      .render()
  } else {
    let cells: Vec<Element<'_, Message>> = cards.iter().map(card::view).collect();
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

fn header<'a>(ready: usize, short: usize) -> Element<'a, Message> {
  Row::with_children(vec![
    text("Stockpile targets")
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(format!(
      "{} READY \u{b7} {} SHORT",
      fmt_count(ready as i64),
      fmt_count(short as i64)
    ))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::SECONDARY),
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
    text("Import multibuy")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
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
    text("+ New stockpile")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
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
      character_id: None,
      fill_isk: 0.0,
      id,
      items: vec![super::super::StockpileItemLine {
        have: if full { 1000 } else { 400 },
        pct: if full { 1.0 } else { 0.4 },
        target: 1000,
        type_id: 34,
        type_name: "Tritanium".to_owned(),
      }],
      location_id: None,
      location_name: None,
      name: "Cache".to_owned(),
      overall_pct: if full { 1.0 } else { 0.4 },
      target_isk: 0.0,
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_card_grid_with_a_ready_and_short_caption() {
      let cards = vec![card(1, true), card(2, false)];

      let _el: Element<'_, Message> = view(&cards);
    }

    #[test]
    fn it_renders_the_shared_empty_state() {
      let _el: Element<'_, Message> = view(&[]);
    }
  }
}
