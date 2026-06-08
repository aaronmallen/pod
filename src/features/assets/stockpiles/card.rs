use std::collections::HashSet;

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, mouse_area, text},
};

use super::{StockpileCard, item_row};
use crate::{
  features::assets::{Message, fmt_isk},
  ui::{
    components::{
      eyebrow::{eyebrow, eyebrow_text},
      progress_bar::progress_bar,
      rule,
      status::dot_sized,
    },
    style::{color, radius, spacing, typography},
  },
};

const BAR_HEIGHT: f32 = 6.0;
const CARD_WIDTH: f32 = 440.0;
const DOT_SIZE: f32 = 8.0;
const ITEM_LIMIT: usize = 5;

pub(super) fn view<'a>(model: &'a StockpileCard, expanded: &HashSet<i64>) -> Element<'a, Message> {
  let ready = model.is_full();
  let pct = model.overall_pct;
  let is_expanded = expanded.contains(&model.id);

  let dot_color = if ready {
    color::status::ONLINE
  } else if pct > 0.6 {
    color::status::WARNING
  } else {
    color::status::DANGER
  };
  let pct_color = if ready {
    color::status::ONLINE
  } else {
    color::status::WARNING
  };
  let bar_color = if ready {
    color::status::ONLINE
  } else {
    color::accent::PLASMA
  };

  let body = Column::with_children(vec![
    meta(model, dot_color, pct_color, bar_color),
    rule::horizontal(),
    status_strip(model, ready),
    rule::horizontal(),
    items(model, is_expanded),
  ])
  .width(Length::Fill);

  let card = container(body)
    .width(Length::Fixed(CARD_WIDTH))
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: if ready {
          color::with_alpha(color::status::ONLINE, 0.35)
        } else {
          color::with_alpha(color::text::PRIMARY, 0.1)
        },
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    });

  mouse_area(card)
    .on_right_press(Message::StockpileCardRightPressed(model.id))
    .into()
}

fn expand_toggle<'a>(id: i64, label: String) -> Element<'a, Message> {
  button(eyebrow_text(&label, Some(color::text::SECONDARY)).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      bottom: spacing::SPACE_2,
      ..Padding::ZERO
    })
    .on_press(Message::StockpileItemsToggled(id))
    .style(|_, _| button::Style {
      background: None,
      ..button::Style::default()
    })
    .into()
}

fn items<'a>(model: &'a StockpileCard, expanded: bool) -> Element<'a, Message> {
  let total = model.items.len();
  let show = if expanded { total } else { total.min(ITEM_LIMIT) };
  let mut rows: Vec<Element<'a, Message>> = model.items.iter().take(show).map(item_row::view).collect();

  let hidden = total - show;
  if hidden > 0 {
    let suffix = if hidden == 1 { "" } else { "s" };
    rows.push(expand_toggle(model.id, format!("+ {hidden} more item{suffix}")));
  } else if expanded && total > ITEM_LIMIT {
    rows.push(expand_toggle(model.id, "\u{2212} Collapse".to_owned()));
  }

  container(Column::with_children(rows).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: 0.0,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn location_caption(model: &StockpileCard) -> (String, iced::Color) {
  match (&model.location_name, model.location_id) {
    (Some(name), _) => (name.to_uppercase(), color::text::SECONDARY),
    (None, Some(_)) => ("Unknown location".to_uppercase(), color::text::TERTIARY),
    (None, None) => ("Any location".to_uppercase(), color::text::TERTIARY),
  }
}

fn location_line(model: &StockpileCard) -> Element<'_, Message> {
  let (caption, caption_color) = location_caption(model);

  Row::with_children(vec![
    eyebrow(&caption, Some(caption_color)),
    Space::new().width(Length::Fill).into(),
    eyebrow(
      &format!("est {} ISK", fmt_isk(model.target_isk)),
      Some(color::text::TERTIARY),
    ),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .width(Length::Fill)
  .into()
}

fn meta(
  model: &StockpileCard,
  dot_color: iced::Color,
  pct_color: iced::Color,
  bar_color: iced::Color,
) -> Element<'_, Message> {
  let head = Row::with_children(vec![
    dot_sized(dot_color, DOT_SIZE),
    text(model.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    text(format!("{}%", (model.overall_pct * 100.0).round() as i64))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(pct_color),
      })
      .into(),
    overflow_button(model.id),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  container(
    Column::with_children(vec![
      head.into(),
      location_line(model),
      progress_bar(model.overall_pct as f32, bar_color, BAR_HEIGHT),
    ])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_3_5)
  .into()
}

fn multibuy_button<'a>(id: i64) -> Element<'a, Message> {
  button(eyebrow_text("Multibuy \u{2192}", Some(color::accent::PLASMA)))
    .padding(Padding::ZERO)
    .on_press(Message::StockpileMultibuyExportOpened(id))
    .style(|_, _| button::Style {
      background: None,
      ..button::Style::default()
    })
    .into()
}

fn overflow_button<'a>(id: i64) -> Element<'a, Message> {
  button(
    text("\u{22ef}")
      .font(typography::mono::REGULAR)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .padding(Padding {
    left: spacing::UNIT,
    right: spacing::UNIT,
    ..Padding::ZERO
  })
  .on_press(Message::StockpileCardRightPressed(id))
  .style(|_, _| button::Style {
    background: None,
    ..button::Style::default()
  })
  .into()
}

fn status_strip<'a>(model: &'a StockpileCard, ready: bool) -> Element<'a, Message> {
  let (content, tint): (Element<'a, Message>, iced::Color) = if ready {
    (
      eyebrow("\u{2713} Ready to ship", Some(color::status::ONLINE)),
      color::with_alpha(color::status::ONLINE, 0.06),
    )
  } else {
    let short = model.short_items();
    let row = Row::with_children(vec![
      eyebrow(&format!("{short} short"), Some(color::status::DANGER)),
      eyebrow("\u{b7}", Some(color::text::TERTIARY)),
      eyebrow(
        &format!("{} ISK to fill", fmt_isk(model.fill_isk)),
        Some(color::text::SECONDARY),
      ),
      Space::new().width(Length::Fill).into(),
      multibuy_button(model.id),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .width(Length::Fill);
    (row.into(), color::with_alpha(color::status::DANGER, 0.05))
  };

  container(content)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(tint)),
      ..container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn card_model(location_name: Option<&str>, location_id: Option<i64>) -> StockpileCard {
    StockpileCard {
      character_id: None,
      fill_isk: 0.0,
      id: 1,
      items: vec![super::super::StockpileItemLine {
        have: 400,
        pct: 0.4,
        target: 1000,
        type_id: 34,
        type_name: "Tritanium".to_owned(),
      }],
      location_id,
      location_name: location_name.map(str::to_owned),
      name: "Cache".to_owned(),
      overall_pct: 0.4,
      target_isk: 0.0,
    }
  }

  fn card_with_items(count: usize) -> StockpileCard {
    let items = (0..count)
      .map(|index| super::super::StockpileItemLine {
        have: 0,
        pct: 0.0,
        target: 100,
        type_id: 34 + index as i64,
        type_name: format!("Item {index}"),
      })
      .collect();
    StockpileCard {
      character_id: None,
      fill_isk: 0.0,
      id: 7,
      items,
      location_id: None,
      location_name: None,
      name: "Big cache".to_owned(),
      overall_pct: 0.0,
      target_isk: 0.0,
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_card_with_a_resolved_location() {
      let model = card_model(Some("Jita IV - Moon 4"), Some(60_003_760));

      let _el: Element<'_, Message> = view(&model, &HashSet::new());
    }

    #[test]
    fn it_renders_a_card_with_an_unresolved_location_fallback() {
      let model = card_model(None, Some(60_003_760));

      let _el: Element<'_, Message> = view(&model, &HashSet::new());
    }

    #[test]
    fn it_renders_an_unscoped_card() {
      let model = card_model(None, None);

      let _el: Element<'_, Message> = view(&model, &HashSet::new());
    }

    #[test]
    fn it_renders_the_short_status_strip() {
      let model = card_model(Some("Jita IV"), Some(1));

      let _el: Element<'_, Message> = view(&model, &HashSet::new());
    }

    #[test]
    fn it_renders_the_ready_status_strip() {
      let mut model = card_model(None, None);
      model.items[0].have = model.items[0].target;
      model.overall_pct = 1.0;

      let _el: Element<'_, Message> = view(&model, &HashSet::new());
    }

    #[test]
    fn it_renders_a_collapsed_and_an_expanded_item_list() {
      let model = card_with_items(8);

      let _collapsed: Element<'_, Message> = view(&model, &HashSet::new());

      let mut expanded = HashSet::new();
      expanded.insert(model.id);
      let _expanded: Element<'_, Message> = view(&model, &expanded);
    }
  }

  mod location_caption {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uppercases_a_resolved_name() {
      let (caption, _) = super::super::location_caption(&card_model(Some("Jita IV"), Some(1)));

      assert_eq!(caption, "JITA IV");
    }

    #[test]
    fn it_falls_back_for_an_unresolved_or_unscoped_location() {
      let (unresolved, _) = super::super::location_caption(&card_model(None, Some(1)));
      let (unscoped, _) = super::super::location_caption(&card_model(None, None));

      assert_eq!(unresolved, "UNKNOWN LOCATION");
      assert_eq!(unscoped, "ANY LOCATION");
    }
  }
}
