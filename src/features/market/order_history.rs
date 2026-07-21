use iced::{
  Background, Border, Color, ContentFit, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, image, scrollable, text},
};

use super::{LotDismissPrompt, LotGroupCard, Message, OrdersScope, OrdersSubTab, State, my_orders};
use crate::{
  clients::eve_image::Size,
  services::inventory_lots,
  store::images::{self, IconResolution},
  ui::{
    components::{clip::clip_layer, confirm_modal, icon::Icon, icon_tile::icon_tile, modal_overlay, rule},
    format::{fmt_count, fmt_isk},
    style::{color, control::scrollbar, radius, spacing, typography},
  },
};

const CARD_GAP: f32 = spacing::SPACE_3_5;
const CARD_ICON_IMAGE: Size = Size::S64;
const CARD_ICON_TILE: f32 = 32.0;
const CARD_WIDTH: f32 = 440.0;
const CHIP_RADIUS: f32 = 4.0;
const EMPTY_COPY_WIDTH: f32 = 380.0;
const EMPTY_HORIZONTAL_PADDING: f32 = 32.0;
const EMPTY_VERTICAL_PADDING: f32 = 56.0;
const LOT_AGE_WIDTH: f32 = 84.0;
const LOT_QTY_WIDTH: f32 = 72.0;
const REMOVE_SIZE: f32 = 24.0;
const SEGMENT_RADIUS: f32 = 7.0;
const SIDE_PADDING: f32 = 28.0;
const STRIP_RADIUS: f32 = 9.0;

pub(super) fn mount<'a>(base: Element<'a, Message>, state: &'a State) -> Element<'a, Message> {
  let layers = match state.lot_dismiss() {
    Some(prompt) => modal_overlay::modal_layers(Message::LotDismissCancelled, dismiss_modal(prompt)),
    None => Vec::new(),
  };
  modal_overlay::stable_overlay(base, layers)
}

pub(super) fn sub_tabs(state: &State) -> Element<'_, Message> {
  let history_count = lot_count(&visible_groups(state.lot_groups(), state.orders_scope()));
  let active = state.orders_sub();

  let control = Row::with_children(vec![
    segment(
      tr("market.orders_sub_current"),
      state.orders().active_count,
      active == OrdersSubTab::Current,
      OrdersSubTab::Current,
    ),
    segment(
      tr("market.orders_sub_history"),
      history_count,
      active == OrdersSubTab::History,
      OrdersSubTab::History,
    ),
  ])
  .spacing(3.0);

  let boxed = container(control).padding(3.0).style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: STRIP_RADIUS.into(),
    },
    ..container::Style::default()
  });

  let strip = container(Row::with_children(vec![boxed.into()]))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: SIDE_PADDING,
      bottom: spacing::SPACE_3,
      left: SIDE_PADDING,
    });

  Column::with_children(vec![strip.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

pub(super) fn surface(state: &State) -> Element<'_, Message> {
  let store = images::default_store();
  let visible = visible_groups(state.lot_groups(), state.orders_scope());

  let body: Element<'_, Message> = if visible.is_empty() {
    empty_card()
  } else {
    grid(&visible, state, &store)
  };

  let inner = Column::with_children(vec![history_header(), body])
    .spacing(spacing::SPACE_4_5)
    .width(Length::Fill);

  scrollable(container(inner).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_4_5,
    right: SIDE_PADDING,
    bottom: 36.0,
    left: SIDE_PADDING,
  }))
  .style(scrollbar)
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn card_header<'a>(
  card: &'a LotGroupCard,
  item_name: String,
  item_group: String,
  split: bool,
  show_char: bool,
  store: &images::Store,
) -> Element<'a, Message> {
  let mut title_children: Vec<Element<'a, Message>> = vec![
    text(item_name)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if split {
    title_children.push(stacks_badge(card.group.lots.len()));
  }

  let identity = Column::with_children(vec![
    Row::with_children(title_children)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
    text(card_subtitle(&item_group, &card.region_label, &card.system_label))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::UNIT / 2.0)
  .width(Length::Fill);

  let mut children: Vec<Element<'a, Message>> = vec![card_icon(store, card.group.type_id), identity.into()];
  if show_char {
    children.push(owner_badge(card));
  }

  container(
    Row::with_children(children)
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 13.0,
    right: 16.0,
    bottom: 13.0,
    left: 16.0,
  })
  .into()
}

fn card_icon<'a>(store: &images::Store, type_id: i64) -> Element<'a, Message> {
  let content: Element<'a, Message> = match store.resolve_type_icon(type_id, None, CARD_ICON_IMAGE) {
    IconResolution::Found(path) => clip_layer(
      image(image::Handle::from_path(path))
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Cover),
      Length::Fill,
      Length::Fill,
    ),
    IconResolution::Missing => Space::new().into(),
  };
  icon_tile(content, CARD_ICON_TILE)
}

fn card_subtitle(group: &str, region: &str, system: &str) -> String {
  [group, region, system]
    .iter()
    .filter(|part| !part.is_empty())
    .copied()
    .collect::<Vec<_>>()
    .join(" \u{b7} ")
}

/// `group.average_cost` is always a quantity-weighted average across remaining lots; the label only
/// reads "avg" once a card holds more than one stack, since a single-lot group's average equals that
/// lot's own unit price.
fn cost_label_key(split: bool) -> &'static str {
  if split {
    "market.orders_history_stat_cost_avg"
  } else {
    "market.orders_history_stat_cost"
  }
}

/// Clamps to 0 for a future/skewed timestamp and falls back to 0 (rather than propagating an error)
/// when `date` fails to parse as RFC3339.
fn days_since(date: &str, now: chrono::DateTime<chrono::Utc>) -> i64 {
  chrono::DateTime::parse_from_rfc3339(date)
    .map(|parsed| (now - parsed.with_timezone(&chrono::Utc)).num_days().max(0))
    .unwrap_or(0)
}

fn dismiss_modal(prompt: &LotDismissPrompt) -> Element<'_, Message> {
  confirm_modal::confirm_modal(
    t!("market.orders_lot_dismiss_title").into_owned(),
    t!("market.orders_lot_dismiss_body", item => prompt.item_name).into_owned(),
    t!("market.orders_lot_dismiss_explanation").into_owned(),
    t!("market.orders_lot_dismiss_confirm").into_owned(),
    Message::LotDismissConfirmed,
    Message::LotDismissCancelled,
  )
}

fn empty_card<'a>() -> Element<'a, Message> {
  let stack = Column::with_children(vec![
    container(
      Icon::contracts()
        .size(28.0)
        .color(color::with_alpha(color::text::PRIMARY, 0.24))
        .render(),
    )
    .padding(Padding {
      top: 0.0,
      right: 0.0,
      bottom: spacing::SPACE_2,
      left: 0.0,
    })
    .into(),
    text(t!("market.orders_history_empty_title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    container(
      text(t!("market.orders_history_empty_body").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .wrapping(text::Wrapping::Word)
        .style(typography::colored(color::text::secondary())),
    )
    .max_width(EMPTY_COPY_WIDTH)
    .align_x(Horizontal::Center)
    .into(),
  ])
  .spacing(spacing::UNIT + 2.0)
  .align_x(Horizontal::Center);

  container(stack)
    .width(Length::Fill)
    .padding(Padding {
      top: EMPTY_VERTICAL_PADDING,
      right: EMPTY_HORIZONTAL_PADDING,
      bottom: EMPTY_VERTICAL_PADDING,
      left: EMPTY_HORIZONTAL_PADDING,
    })
    .align_x(Horizontal::Center)
    .style(|_| container::Style {
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::PANEL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn grid<'a>(cards: &[&'a LotGroupCard], state: &'a State, store: &images::Store) -> Element<'a, Message> {
  let cells: Vec<Element<'a, Message>> = cards.iter().map(|card| group_card(card, state, store)).collect();
  Row::with_children(cells).spacing(CARD_GAP).wrap().into()
}

fn group_card<'a>(card: &'a LotGroupCard, state: &'a State, store: &images::Store) -> Element<'a, Message> {
  let (item_name, item_group) = my_orders::find_identity(state.tree(), card.group.type_id);
  let split = card.group.lots.len() > 1;

  let content = Column::with_children(vec![
    card_header(
      card,
      item_name.clone(),
      item_group,
      split,
      state.orders_show_character(),
      store,
    ),
    stat_strip(&card.group, split),
    lots_list(card, item_name),
  ])
  .width(Length::Fill);

  container(content)
    .width(Length::Fixed(CARD_WIDTH))
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn history_header<'a>() -> Element<'a, Message> {
  Row::with_children(vec![
    text(t!("market.orders_history_title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    Space::new().width(Length::Fill).into(),
    text(t!("market.orders_history_caption").into_owned().to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .into()
}

fn lot_count(cards: &[&LotGroupCard]) -> usize {
  cards.iter().map(|card| card.group.lots.len()).sum()
}

fn lot_price_block<'a>(label_key: &str, value: String, tint: Color) -> Element<'a, Message> {
  Column::with_children(vec![
    text(t!(label_key).into_owned().to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    text(value)
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(tint))
      .into(),
  ])
  .spacing(spacing::UNIT / 2.0)
  .width(Length::Fill)
  .into()
}

fn lot_row<'a>(
  card: &'a LotGroupCard,
  lot: &'a inventory_lots::Lot,
  item_name: String,
  now: chrono::DateTime<chrono::Utc>,
) -> Element<'a, Message> {
  let qty = container(
    text(format!("\u{d7}{}", fmt_count(lot.quantity_remaining)))
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::PRIMARY)),
  )
  .width(Length::Fixed(LOT_QTY_WIDTH));

  let age = container(
    text(t!("market.orders_history_days_ago", days => days_since(&lot.date, now)).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::tertiary())),
  )
  .width(Length::Fixed(LOT_AGE_WIDTH))
  .align_x(Horizontal::Right);

  let cells = Row::with_children(vec![
    qty.into(),
    lot_price_block(
      "market.orders_history_bought_at",
      my_orders::fmt_price(lot.unit_price),
      color::text::secondary(),
    ),
    lot_price_block(
      "market.orders_history_sell_at",
      my_orders::fmt_price(lot.target_price),
      color::status::ONLINE,
    ),
    age.into(),
    remove_button(card, lot, item_name),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  container(cells)
    .width(Length::Fill)
    .padding(Padding {
      top: 9.0,
      right: 12.0,
      bottom: 9.0,
      left: 12.0,
    })
    .into()
}

fn lots_list<'a>(card: &'a LotGroupCard, item_name: String) -> Element<'a, Message> {
  let now = chrono::Utc::now();
  let rows: Vec<Element<'a, Message>> = card
    .group
    .lots
    .iter()
    // Lots are stored oldest-first (FIFO consumption order, per inventory_lots); reverse for a
    // newest-first display order.
    .rev()
    .map(|lot| lot_row(card, lot, item_name.clone(), now))
    .collect();

  container(Column::with_children(rows))
    .width(Length::Fill)
    .padding(Padding {
      top: 6.0,
      right: 4.0,
      bottom: 6.0,
      left: 4.0,
    })
    .into()
}

fn owner_badge<'a>(card: &'a LotGroupCard) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![
    my_orders::initials_tile(&card.owner_name),
    text(card.owner_name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ];
  if card.group.is_corporation {
    children.push(my_orders::corp_owner_badge());
  }
  Row::with_children(children)
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into()
}

fn remove_button<'a>(card: &LotGroupCard, lot: &inventory_lots::Lot, item_name: String) -> Element<'a, Message> {
  let prompt = LotDismissPrompt {
    is_corporation: card.group.is_corporation,
    item_name,
    owner_id: card.group.owner_id,
    transaction_id: lot.transaction_id,
  };
  let icon = container(Icon::close().size(12.0).color(color::text::tertiary()).render())
    .width(Length::Fixed(REMOVE_SIZE))
    .height(Length::Fixed(REMOVE_SIZE))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center);

  button(icon)
    .padding(0)
    .on_press(Message::LotDismissPrompted(Box::new(prompt)))
    .style(|_, status| {
      let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        border: Border {
          color: if hover {
            color::with_alpha(color::status::DANGER, 0.4)
          } else {
            color::rule()
          },
          radius: 5.0.into(),
          width: 1.0,
        },
        text_color: if hover {
          color::status::DANGER
        } else {
          color::text::tertiary()
        },
        ..button::Style::default()
      }
    })
    .into()
}

fn segment<'a>(label: &'a str, count: usize, active: bool, sub: OrdersSubTab) -> Element<'a, Message> {
  let label_color = if active {
    color::accent()
  } else {
    color::text::secondary()
  };

  let content = Row::with_children(vec![
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(label_color))
      .into(),
    segment_count(count, active),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  button(content)
    .padding(Padding {
      top: 7.0,
      right: 14.0,
      bottom: 7.0,
      left: 14.0,
    })
    .on_press_maybe((!active).then_some(Message::OrdersSubTabSelected(sub)))
    .style(move |_, _| button::Style {
      background: active.then(|| Background::Color(color::with_alpha(color::accent(), 0.12))),
      border: Border {
        radius: SEGMENT_RADIUS.into(),
        ..Border::default()
      },
      text_color: label_color,
      ..button::Style::default()
    })
    .into()
}

fn segment_count<'a>(count: usize, active: bool) -> Element<'a, Message> {
  let tint = if active {
    color::accent()
  } else {
    color::text::tertiary()
  };
  container(
    text(fmt_count(count as i64))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(tint)),
  )
  .padding(Padding {
    top: 1.0,
    right: 5.0,
    bottom: 1.0,
    left: 5.0,
  })
  .style(move |_| container::Style {
    background: (!active).then_some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.05))),
    border: Border {
      color: if active { Color::TRANSPARENT } else { color::rule() },
      width: 1.0,
      radius: CHIP_RADIUS.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn stacks_badge<'a>(count: usize) -> Element<'a, Message> {
  container(
    text(
      t!("market.orders_history_stacks", count => count)
        .into_owned()
        .to_uppercase(),
    )
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS)
    .wrapping(text::Wrapping::None)
    .style(typography::colored(color::status::WARNING)),
  )
  .padding(Padding {
    top: 2.0,
    right: 6.0,
    bottom: 2.0,
    left: 6.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.12))),
    border: Border {
      color: color::with_alpha(color::status::WARNING, 0.32),
      radius: CHIP_RADIUS.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn stat_cell<'a>(label: String, value: String, tint: Color) -> Element<'a, Message> {
  let block = Column::with_children(vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::secondary()))
      .into(),
    text(value)
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(tint))
      .into(),
  ])
  .spacing(spacing::UNIT + 1.0);

  container(block)
    .width(Length::FillPortion(1))
    .padding(Padding {
      top: 10.0,
      right: 16.0,
      bottom: 10.0,
      left: 16.0,
    })
    .into()
}

fn stat_rule<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(1.0))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

fn stat_strip<'a>(group: &inventory_lots::LotGroup, split: bool) -> Element<'a, Message> {
  let cells = vec![
    stat_cell(
      t!("market.orders_history_stat_held").into_owned(),
      format!("\u{d7}{}", fmt_count(group.held_quantity)),
      color::text::PRIMARY,
    ),
    stat_cell(
      t!(cost_label_key(split)).into_owned(),
      my_orders::fmt_price(group.average_cost),
      color::text::secondary(),
    ),
    stat_cell(
      t!(target_label_key(split)).into_owned(),
      my_orders::fmt_price(group.average_target),
      color::status::ONLINE,
    ),
    stat_cell(
      t!("market.orders_history_stat_profit").into_owned(),
      format!("+{}", fmt_isk(group.estimated_profit)),
      color::status::ONLINE,
    ),
  ];

  let mut children: Vec<Element<'a, Message>> = Vec::new();
  for (index, cell) in cells.into_iter().enumerate() {
    if index > 0 {
      children.push(stat_rule());
    }
    children.push(cell);
  }

  let row = container(Row::with_children(children))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    });

  Column::with_children(vec![rule::horizontal(), row.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

/// See [`cost_label_key`]: `average_target` is likewise a weighted average, worded as "avg" only
/// when the card holds more than one stack.
fn target_label_key(split: bool) -> &'static str {
  if split {
    "market.orders_history_stat_target_avg"
  } else {
    "market.orders_history_stat_target"
  }
}

fn tr(key: &str) -> &'static str {
  super::i18n::tr_static(key)
}

fn visible_groups(cards: &[LotGroupCard], scope: OrdersScope) -> Vec<&LotGroupCard> {
  cards
    .iter()
    .filter(|card| match scope {
      OrdersScope::All => true,
      OrdersScope::Character(id) => !card.group.is_corporation && card.group.owner_id == id,
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn lot(transaction_id: i64, date: &str, quantity_remaining: i64) -> inventory_lots::Lot {
    inventory_lots::Lot {
      date: date.to_owned(),
      quantity: 10,
      quantity_remaining,
      target_price: 110.0,
      transaction_id,
      unit_price: 100.0,
    }
  }

  fn card(owner_id: i64, is_corporation: bool, lots: Vec<inventory_lots::Lot>) -> LotGroupCard {
    LotGroupCard {
      group: inventory_lots::LotGroup {
        average_cost: 100.0,
        average_target: 110.0,
        estimated_profit: 100.0,
        held_quantity: lots.iter().map(|entry| entry.quantity_remaining).sum(),
        is_corporation,
        location_id: 60_003_760,
        lots,
        owner_id,
        type_id: 34,
      },
      owner_name: "Test Pilot".to_owned(),
      region_label: "The Forge".to_owned(),
      system_label: "Jita".to_owned(),
    }
  }

  fn populated_state(scope: OrdersScope) -> State {
    let mut state = State::new();
    super::super::update(&mut state, Message::OrdersScopeSelected(scope));
    super::super::update(
      &mut state,
      Message::LotsLoaded(vec![
        card(
          90,
          false,
          vec![lot(1, "2026-07-01T00:00:00Z", 10), lot(2, "2026-07-10T00:00:00Z", 4)],
        ),
        card(98_000_001, true, vec![lot(3, "2026-07-05T00:00:00Z", 7)]),
      ]),
    );
    super::super::update(&mut state, Message::OrdersSubTabSelected(OrdersSubTab::History));
    state
  }

  mod visible_groups {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_includes_corp_groups_in_the_all_scope() {
      let cards = vec![
        card(90, false, vec![lot(1, "2026-07-01T00:00:00Z", 10)]),
        card(98_000_001, true, vec![lot(2, "2026-07-02T00:00:00Z", 5)]),
      ];

      let visible = visible_groups(&cards, OrdersScope::All);

      assert_eq!(visible.len(), 2);
    }

    #[test]
    fn it_narrows_a_character_scope_to_that_pilots_personal_groups() {
      let cards = vec![
        card(90, false, vec![lot(1, "2026-07-01T00:00:00Z", 10)]),
        card(91, false, vec![lot(2, "2026-07-02T00:00:00Z", 5)]),
        card(90, true, vec![lot(3, "2026-07-03T00:00:00Z", 5)]),
      ];

      let visible = visible_groups(&cards, OrdersScope::Character(90));

      assert_eq!(visible.len(), 1);
      assert_eq!(visible[0].group.owner_id, 90);
      assert!(!visible[0].group.is_corporation);
    }
  }

  mod lot_count {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_lots_across_visible_groups() {
      let cards = vec![
        card(
          90,
          false,
          vec![lot(1, "2026-07-01T00:00:00Z", 10), lot(2, "2026-07-02T00:00:00Z", 5)],
        ),
        card(91, false, vec![lot(3, "2026-07-03T00:00:00Z", 7)]),
      ];
      let visible = visible_groups(&cards, OrdersScope::All);

      assert_eq!(lot_count(&visible), 3);
    }
  }

  mod stat_labels {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_switches_to_average_labels_when_a_card_has_multiple_stacks() {
      assert_eq!(cost_label_key(false), "market.orders_history_stat_cost");
      assert_eq!(cost_label_key(true), "market.orders_history_stat_cost_avg");

      assert_eq!(target_label_key(false), "market.orders_history_stat_target");
      assert_eq!(target_label_key(true), "market.orders_history_stat_target_avg");
    }
  }

  mod card_subtitle {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_joins_the_group_region_and_system() {
      assert_eq!(
        card_subtitle("Minerals", "The Forge", "Jita"),
        "Minerals \u{b7} The Forge \u{b7} Jita"
      );
    }

    #[test]
    fn it_skips_empty_parts() {
      assert_eq!(card_subtitle("", "The Forge", "Jita"), "The Forge \u{b7} Jita");
      assert_eq!(card_subtitle("", "", ""), "");
    }
  }

  mod days_since {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_counts_whole_days_since_the_purchase() {
      let now = chrono::DateTime::parse_from_rfc3339("2026-07-21T12:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

      assert_eq!(days_since("2026-07-01T00:00:00Z", now), 20);
      assert_eq!(days_since("2026-07-21T11:00:00Z", now), 0);
    }

    #[test]
    fn it_falls_back_to_zero_for_an_unparseable_date() {
      assert_eq!(days_since("nope", chrono::Utc::now()), 0);
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_the_empty_history_state() {
      let mut state = State::new();
      super::super::super::update(&mut state, Message::OrdersSubTabSelected(OrdersSubTab::History));

      let _el: Element<'_, Message> = surface(&state);
    }

    #[test]
    fn it_renders_the_history_grid_with_character_badges_in_the_all_scope() {
      let state = populated_state(OrdersScope::All);

      assert!(state.orders_show_character());
      let _el: Element<'_, Message> = surface(&state);
    }

    #[test]
    fn it_renders_the_history_grid_for_a_single_character() {
      let state = populated_state(OrdersScope::Character(90));

      let _el: Element<'_, Message> = surface(&state);
    }

    #[test]
    fn it_renders_the_sub_tab_strip() {
      let state = populated_state(OrdersScope::All);

      let _el: Element<'_, Message> = sub_tabs(&state);
    }

    #[test]
    fn it_mounts_the_dismiss_confirmation_over_the_base() {
      let mut state = populated_state(OrdersScope::All);
      super::super::super::update(
        &mut state,
        Message::LotDismissPrompted(Box::new(LotDismissPrompt {
          is_corporation: false,
          item_name: "Tritanium".to_owned(),
          owner_id: 90,
          transaction_id: 1,
        })),
      );

      assert!(state.lot_dismiss().is_some());
      let _el: Element<'_, Message> = mount(Space::new().into(), &state);
    }
  }
}
