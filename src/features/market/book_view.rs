use iced::{
  Background, Border, Color, ContentFit, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container, image, scrollable, text},
};

use super::{
  Message, State,
  book::{BookRow, OrderBook},
  shell,
  tree::{MarketNode, MarketTree},
};
use crate::{
  clients::eve_image::Size,
  store::images::{self, IconResolution},
  ui::{
    components::{clip::clip_layer, icon::Icon, icon_tile::icon_tile},
    format::{fmt_count, fmt_isk_full},
    style::{
      color,
      control::{bordered_pane, scrollbar},
      spacing, typography,
    },
  },
};

const ICON_TILE: f32 = 42.0;
const ICON_SIZE: Size = Size::S64;
const ROW_HEIGHT: f32 = 38.0;
const RAIL_WIDTH: f32 = 2.0;
const STAT_DIVIDER: f32 = 34.0;
const DOT: f32 = 7.0;
const CELL_PAD: f32 = 14.0;
const QTY_WIDTH: f32 = 88.0;
const PRICE_WIDTH: f32 = 128.0;
const JUMPS_WIDTH: f32 = 64.0;
const RANGE_WIDTH: f32 = 96.0;
const EXPIRES_WIDTH: f32 = 80.0;
const EXPIRES_WARN_DAYS: i64 = 14;
const EM_DASH: &str = "\u{2014}";
const STAT_VALUE_SIZE: f32 = 14.0;
const PANE_PAD_X: f32 = 20.0;
const SECTION_PAD_X: f32 = 16.0;

struct Identity {
  name: String,
  group: String,
}

struct RowFlags {
  mine: bool,
  outbid: bool,
}

// Own-order highlight seam. Phase 5 (My Orders) replaces this to flag the
// player's live orders in place; this spec has no own-order data, so every
// row renders as a foreign order.
fn own_order_flags(_row: &BookRow) -> RowFlags {
  RowFlags {
    mine: false,
    outbid: false,
  }
}

pub(super) fn detail(state: &State) -> iced::Element<'_, Message> {
  let pane = match state.selected_type_id() {
    None => empty_detail(),
    Some(type_id) => match state.book() {
      None => loading_detail(),
      Some(book) => order_book(state, type_id, book),
    },
  };

  container(pane)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(bordered_pane)
    .into()
}

fn empty_detail() -> iced::Element<'static, Message> {
  shell::empty_state(
    Icon::contracts(),
    "market.browse_empty_title",
    "market.browse_empty_body",
  )
}

fn loading_detail() -> iced::Element<'static, Message> {
  shell::empty_state(Icon::market(), "market.book_loading_title", "market.book_loading_body")
}

fn order_book<'a>(state: &'a State, type_id: i64, book: &'a OrderBook) -> iced::Element<'a, Message> {
  let identity = find_identity(state.tree(), type_id);
  let region = state
    .active_region()
    .map(|region| region.name.clone())
    .unwrap_or_default();

  let body = scrollable(
    Column::with_children(vec![
      book_section("market.book_sell_title", color::status::ONLINE, &book.sell),
      section_divider(),
      book_section("market.book_buy_title", color::status::DANGER, &book.buy),
      Space::new().height(Length::Fixed(spacing::SPACE_6)).into(),
    ])
    .width(Length::Fill),
  )
  .style(scrollbar)
  .width(Length::Fill)
  .height(Length::Fill);

  Column::with_children(vec![
    item_header(type_id, &identity, &region, book),
    view_toggle(),
    body.into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn item_header<'a>(type_id: i64, identity: &Identity, region: &str, book: &OrderBook) -> iced::Element<'a, Message> {
  let title = Column::with_children(vec![
    text(identity.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(subtitle(identity, region))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::UNIT / 2.0);

  let content = Row::with_children(vec![
    item_icon(type_id),
    title.into(),
    Space::new().width(Length::Fill).into(),
    head_stat(
      "market.book_stat_best_sell",
      fmt_price_opt(book.best_sell),
      color::status::ONLINE,
    ),
    stat_divider(),
    head_stat(
      "market.book_stat_best_buy",
      fmt_price_opt(book.best_buy),
      color::text::PRIMARY,
    ),
    stat_divider(),
    head_stat(
      "market.book_stat_spread",
      fmt_spread(book.spread_pct),
      color::text::PRIMARY,
    ),
    stat_divider(),
    head_stat("market.book_stat_your_orders", "0".to_owned(), color::accent()),
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(content)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_4_5,
      right: PANE_PAD_X,
      bottom: spacing::SPACE_4_5,
      left: PANE_PAD_X,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: bottom_rule(),
      ..container::Style::default()
    })
    .into()
}

fn subtitle(identity: &Identity, region: &str) -> String {
  let mut parts: Vec<&str> = Vec::new();
  if !identity.group.is_empty() {
    parts.push(&identity.group);
  }
  if !region.is_empty() {
    parts.push(region);
  }
  parts.join(" \u{b7} ")
}

fn item_icon<'a>(type_id: i64) -> iced::Element<'a, Message> {
  let store = images::default_store();
  let content: iced::Element<'a, Message> = match store.resolve_type_icon(type_id, None, ICON_SIZE) {
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
  icon_tile(content, ICON_TILE)
}

fn head_stat<'a>(label_key: &str, value: String, accent: Color) -> iced::Element<'a, Message> {
  Column::with_children(vec![
    text(t!(label_key).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::secondary()))
      .into(),
    text(value)
      .font(typography::mono::REGULAR)
      .size(STAT_VALUE_SIZE)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(accent))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .into()
}

fn stat_divider<'a>() -> iced::Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(1.0))
    .height(Length::Fixed(STAT_DIVIDER))
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

fn view_toggle<'a>() -> iced::Element<'a, Message> {
  let chips = Row::with_children(vec![
    toggle_chip(Icon::contracts(), "market.book_view_orders", true),
    toggle_chip(Icon::tracker(), "market.book_view_history", false),
  ]);

  let group = container(chips).style(|_| container::Style {
    border: Border {
      color: color::rule(),
      radius: 7.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  container(group)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: PANE_PAD_X,
      bottom: spacing::SPACE_2_5,
      left: PANE_PAD_X,
    })
    .style(|_| container::Style {
      border: bottom_rule(),
      ..container::Style::default()
    })
    .into()
}

fn toggle_chip<'a>(icon: Icon, label_key: &str, active: bool) -> iced::Element<'a, Message> {
  let tint = if active {
    color::accent()
  } else {
    color::text::secondary()
  };
  let content = Row::with_children(vec![
    icon.size(14.0).color(tint).render(),
    text(t!(label_key).into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(tint))
      .into(),
  ])
  .spacing(spacing::UNIT + 3.0)
  .align_y(Vertical::Center);

  container(content)
    .padding(Padding {
      top: spacing::UNIT + 3.0,
      right: spacing::SPACE_3_5,
      bottom: spacing::UNIT + 3.0,
      left: spacing::SPACE_3_5,
    })
    .style(move |_| container::Style {
      background: active.then_some(Background::Color(color::with_alpha(color::accent(), 0.12))),
      ..container::Style::default()
    })
    .into()
}

fn book_section<'a>(title_key: &str, accent: Color, rows: &'a [BookRow]) -> iced::Element<'a, Message> {
  let mut children: Vec<iced::Element<'a, Message>> =
    vec![section_header(title_key, accent, rows.len()), column_headers()];
  for (index, row) in rows.iter().enumerate() {
    children.push(book_row(row, index == 0, own_order_flags(row)));
  }

  Column::with_children(children).width(Length::Fill).into()
}

fn section_header<'a>(title_key: &str, accent: Color, count: usize) -> iced::Element<'a, Message> {
  let content = Row::with_children(vec![
    container(Space::new())
      .width(Length::Fixed(DOT))
      .height(Length::Fixed(DOT))
      .style(move |_| container::Style {
        background: Some(Background::Color(accent)),
        border: Border {
          radius: 999.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    text(t!(title_key).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(t!("market.book_orders_count", count => count).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  container(content)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: SECTION_PAD_X,
      bottom: spacing::SPACE_2,
      left: SECTION_PAD_X,
    })
    .into()
}

fn column_headers<'a>() -> iced::Element<'a, Message> {
  let content = Row::with_children(vec![
    header_cell("market.book_col_qty", Length::Fixed(QTY_WIDTH), false),
    header_cell("market.book_col_price", Length::Fixed(PRICE_WIDTH), true),
    header_cell("market.book_col_location", Length::Fill, false),
    header_cell("market.book_col_jumps", Length::Fixed(JUMPS_WIDTH), false),
    header_cell("market.book_col_range", Length::Fixed(RANGE_WIDTH), false),
    header_cell("market.book_col_expires", Length::Fixed(EXPIRES_WIDTH), true),
  ])
  .align_y(Vertical::Center);

  container(content)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::UNIT + 2.0,
      right: 0.0,
      bottom: spacing::UNIT + 2.0,
      left: 0.0,
    })
    .style(|_| container::Style {
      border: bottom_rule(),
      ..container::Style::default()
    })
    .into()
}

fn header_cell<'a>(label_key: &str, width: Length, align_right: bool) -> iced::Element<'a, Message> {
  let label = text(t!(label_key).into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .wrapping(text::Wrapping::None)
    .style(typography::colored(color::text::tertiary()));

  container(label)
    .width(width)
    .padding(Padding {
      top: 0.0,
      right: CELL_PAD,
      bottom: 0.0,
      left: CELL_PAD,
    })
    .align_x(if align_right {
      Horizontal::Right
    } else {
      Horizontal::Left
    })
    .into()
}

fn book_row<'a>(row: &BookRow, best: bool, flags: RowFlags) -> iced::Element<'a, Message> {
  let price_color = if flags.mine {
    color::accent()
  } else if best {
    color::status::ONLINE
  } else {
    color::text::PRIMARY
  };
  let price_font = if flags.mine || best {
    typography::mono::MEDIUM
  } else {
    typography::mono::REGULAR
  };

  let cells = Row::with_children(vec![
    text_cell(
      fmt_qty(row.volume_remain),
      Length::Fixed(QTY_WIDTH),
      false,
      color::text::secondary(),
    ),
    price_cell(fmt_price(row.price), price_font, price_color),
    location_cell(row, &flags),
    text_cell(
      EM_DASH.to_owned(),
      Length::Fixed(JUMPS_WIDTH),
      false,
      color::text::secondary(),
    ),
    text_cell(
      fmt_range(&row.range),
      Length::Fixed(RANGE_WIDTH),
      false,
      color::text::tertiary(),
    ),
    expires_cell(row),
  ])
  .align_y(Vertical::Center)
  .height(Length::Fixed(ROW_HEIGHT));

  let inner = Row::with_children(vec![row_rail(flags.mine), cells.into()]).align_y(Vertical::Center);

  container(inner)
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: flags
        .mine
        .then_some(Background::Color(color::with_alpha(color::accent(), 0.10))),
      border: bottom_rule(),
      ..container::Style::default()
    })
    .into()
}

fn row_rail<'a>(mine: bool) -> iced::Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(RAIL_WIDTH))
    .height(Length::Fixed(ROW_HEIGHT))
    .style(move |_| container::Style {
      background: Some(Background::Color(if mine {
        color::accent()
      } else {
        Color::TRANSPARENT
      })),
      ..container::Style::default()
    })
    .into()
}

fn text_cell<'a>(value: String, width: Length, align_right: bool, tint: Color) -> iced::Element<'a, Message> {
  let label = text(value)
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .wrapping(text::Wrapping::None)
    .style(typography::colored(tint));

  container(label)
    .width(width)
    .padding(Padding {
      top: 0.0,
      right: CELL_PAD,
      bottom: 0.0,
      left: CELL_PAD,
    })
    .align_x(if align_right {
      Horizontal::Right
    } else {
      Horizontal::Left
    })
    .into()
}

fn price_cell<'a>(value: String, font: iced::Font, tint: Color) -> iced::Element<'a, Message> {
  container(
    text(value)
      .font(font)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(tint)),
  )
  .width(Length::Fixed(PRICE_WIDTH))
  .padding(Padding {
    top: 0.0,
    right: CELL_PAD,
    bottom: 0.0,
    left: CELL_PAD,
  })
  .align_x(Horizontal::Right)
  .into()
}

fn location_cell<'a>(row: &BookRow, flags: &RowFlags) -> iced::Element<'a, Message> {
  let mut children: Vec<iced::Element<'a, Message>> = Vec::new();
  if flags.mine {
    children.push(mine_badge());
  }
  if flags.mine && flags.outbid {
    children.push(
      text(t!("market.book_badge_outbid").into_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::status::DANGER))
        .into(),
    );
  }
  children.push(
    text(t!("market.book_location_fallback", id => row.location_id).into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::secondary()))
      .into(),
  );

  container(
    Row::with_children(children)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 0.0,
    right: CELL_PAD,
    bottom: 0.0,
    left: CELL_PAD,
  })
  .into()
}

fn mine_badge<'a>() -> iced::Element<'a, Message> {
  container(
    text(t!("market.book_badge_you").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::accent())),
  )
  .padding(Padding {
    top: 2.0,
    right: 5.0,
    bottom: 2.0,
    left: 5.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent(), 0.14))),
    border: Border {
      color: color::with_alpha(color::accent(), 0.4),
      radius: 4.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn expires_cell<'a>(row: &BookRow) -> iced::Element<'a, Message> {
  let days = expires_days(row);
  let tint = if days <= EXPIRES_WARN_DAYS {
    color::status::WARNING
  } else {
    color::text::secondary()
  };
  text_cell(format!("{days}d"), Length::Fixed(EXPIRES_WIDTH), true, tint)
}

fn section_divider<'a>() -> iced::Element<'a, Message> {
  container(Space::new())
    .width(Length::Fill)
    .height(Length::Fixed(1.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule_strong())),
      ..container::Style::default()
    })
    .into()
}

fn bottom_rule() -> Border {
  Border {
    color: color::rule(),
    radius: 0.0.into(),
    width: 0.0,
  }
}

fn find_identity(tree: &MarketTree, type_id: i64) -> Identity {
  for node in &tree.roots {
    if let Some(identity) = find_in_node(node, type_id) {
      return identity;
    }
  }
  Identity {
    name: t!("market.book_item_fallback", id => type_id).into_owned(),
    group: String::new(),
  }
}

fn find_in_node(node: &MarketNode, type_id: i64) -> Option<Identity> {
  for leaf in &node.items {
    if leaf.type_id == type_id {
      return Some(Identity {
        name: leaf.name.clone(),
        group: node.name.clone(),
      });
    }
  }
  for child in &node.children {
    if let Some(identity) = find_in_node(child, type_id) {
      return Some(identity);
    }
  }
  None
}

fn expires_days(row: &BookRow) -> i64 {
  match chrono::DateTime::parse_from_rfc3339(&row.issued) {
    Ok(issued) => {
      let expiry = issued + chrono::Duration::days(row.duration);
      (expiry.with_timezone(&chrono::Utc) - chrono::Utc::now())
        .num_days()
        .max(0)
    }
    Err(_) => row.duration.max(0),
  }
}

fn fmt_price(value: f64) -> String {
  if value < 1000.0 {
    format!("{value:.2}")
  } else {
    fmt_isk_full(value)
  }
}

fn fmt_price_opt(value: Option<f64>) -> String {
  match value {
    Some(value) => fmt_price(value),
    None => EM_DASH.to_owned(),
  }
}

fn fmt_spread(pct: Option<f64>) -> String {
  match pct {
    Some(pct) => format!("{pct:.1}%"),
    None => EM_DASH.to_owned(),
  }
}

fn fmt_qty(qty: i64) -> String {
  let magnitude = qty.unsigned_abs() as f64;
  if magnitude >= 1e9 {
    format!("{:.2}B", qty as f64 / 1e9)
  } else if magnitude >= 1e6 {
    format!("{:.2}M", qty as f64 / 1e6)
  } else if magnitude >= 1e4 {
    format!("{:.1}K", qty as f64 / 1e3)
  } else {
    fmt_count(qty)
  }
}

fn fmt_range(range: &str) -> String {
  match range {
    "station" => t!("market.book_range_station").into_owned(),
    "region" => t!("market.book_range_region").into_owned(),
    "solarsystem" => t!("market.book_range_system").into_owned(),
    other => match other.parse::<i64>() {
      Ok(jumps) => t!("market.book_range_jumps", count => jumps).into_owned(),
      Err(_) => other.to_owned(),
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    features::{
      assets::{LocationRef, LocationTier},
      market::{Message, book, tree},
    },
    store::model::{ItemType, MarketGroup},
  };

  fn row(order_id: i64, is_buy_order: bool, price: f64) -> BookRow {
    BookRow {
      order_id,
      location_id: 60_003_760,
      system_id: 30_000_142,
      price,
      volume_remain: 1_250_000,
      min_volume: 1,
      range: "region".to_owned(),
      duration: 90,
      issued: "2026-07-01T12:00:00Z".to_owned(),
      is_buy_order,
    }
  }

  fn selected_state() -> State {
    let groups = vec![
      MarketGroup {
        description: String::new(),
        has_types: false,
        icon_id: None,
        id: 1,
        name: "Ships".to_owned(),
        parent_id: None,
      },
      MarketGroup {
        description: String::new(),
        has_types: false,
        icon_id: None,
        id: 2,
        name: "Frigate".to_owned(),
        parent_id: Some(1),
      },
    ];
    let items = vec![ItemType {
      capacity: None,
      description: None,
      dogma_attributes: "[]".to_owned(),
      group_id: 0,
      icon_id: None,
      id: 587,
      market_group_id: Some(2),
      name: "Rifter".to_owned(),
      packaged_volume: None,
      portion_size: None,
      published: true,
      radius: None,
      volume: None,
    }];

    let mut state = State::new();
    super::super::update(
      &mut state,
      Message::TreeLoaded(Box::new(tree::build_market_tree(&groups, &items))),
    );
    super::super::update(
      &mut state,
      Message::DefaultMarketResolved(LocationRef {
        context: None,
        id: 10_000_002,
        name: "The Forge".to_owned(),
        security_status: None,
        tier: Some(LocationTier::Region),
      }),
    );
    super::super::update(&mut state, Message::ItemSelected(587));
    state
  }

  #[test]
  fn it_renders_the_empty_state_before_a_selection() {
    let state = State::new();
    let _el: iced::Element<'_, Message> = detail(&state);
  }

  #[test]
  fn it_renders_the_loading_state_while_the_book_is_absent() {
    let state = selected_state();
    assert!(state.book().is_none());
    let _el: iced::Element<'_, Message> = detail(&state);
  }

  #[test]
  fn it_renders_the_order_book_once_loaded() {
    let mut state = selected_state();
    let book = book::build_order_book(Vec::new());
    super::super::update(&mut state, Message::BookLoaded(Box::new(book)));

    let _el: iced::Element<'_, Message> = detail(&state);
  }

  #[test]
  fn it_renders_a_populated_book_with_both_sides() {
    let mut state = selected_state();
    let orders = vec![row(1, false, 5.5), row(2, false, 6.5), row(3, true, 4.0)];
    let mut book = book::OrderBook::default();
    for order in orders {
      if order.is_buy_order {
        book.buy.push(order);
      } else {
        book.sell.push(order);
      }
    }
    book.best_sell = Some(5.5);
    book.best_buy = Some(4.0);
    book.spread_pct = Some(27.0);
    super::super::update(&mut state, Message::BookLoaded(Box::new(book)));

    let _el: iced::Element<'_, Message> = detail(&state);
  }

  #[test]
  fn it_lights_up_a_row_through_the_mine_seam() {
    let flags = RowFlags {
      mine: true,
      outbid: true,
    };
    let _el: iced::Element<'_, Message> = book_row(&row(1, false, 1_842_000.0), true, flags);
  }

  #[test]
  fn it_stubs_every_row_as_foreign_this_spec() {
    let flags = own_order_flags(&row(1, false, 5.0));

    assert!(!flags.mine);
    assert!(!flags.outbid);
  }

  #[test]
  fn it_resolves_the_item_identity_from_the_tree() {
    let state = selected_state();

    let identity = find_identity(state.tree(), 587);

    assert_eq!(identity.name, "Rifter");
    assert_eq!(identity.group, "Frigate");
  }

  #[test]
  fn it_falls_back_when_the_type_is_absent_from_the_tree() {
    let state = selected_state();

    let identity = find_identity(state.tree(), 999);

    assert!(identity.group.is_empty());
  }

  #[test]
  fn it_formats_small_unit_prices_with_two_decimals() {
    assert_eq!(fmt_price(6.4), "6.40");
    assert_eq!(fmt_price(1_842_000.0), "1,842,000");
  }

  #[test]
  fn it_formats_optional_prices_and_spread() {
    assert_eq!(fmt_price_opt(None), EM_DASH);
    assert_eq!(fmt_spread(None), EM_DASH);
    assert_eq!(fmt_spread(Some(4.27)), "4.3%");
  }

  #[test]
  fn it_compacts_quantities() {
    assert_eq!(fmt_qty(1_250_000), "1.25M");
    assert_eq!(fmt_qty(12_400), "12.4K");
    assert_eq!(fmt_qty(420), "420");
  }

  #[test]
  fn it_maps_order_ranges_to_labels() {
    crate::services::i18n::set_locale(crate::services::i18n::Language::En);

    assert_eq!(fmt_range("station"), "Station");
    assert_eq!(fmt_range("region"), "Region");
    assert_eq!(fmt_range("solarsystem"), "System");
    assert_eq!(fmt_range("10"), "10 jumps");
  }
}
