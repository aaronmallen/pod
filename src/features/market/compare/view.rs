use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, mouse_area, scrollable, text},
};

use super::{
  super::{BookAccess, Message, State, Tab, book_view, browse, i18n::tr_static, shell},
  Arbitrage, CompareColumn, CompareMenu, arbitrage, cheapest_sell, richest_buy,
};
use crate::ui::{
  components::{
    backdrop,
    button::{Button, Size},
    context_menu::{self, Item},
    icon::Icon,
    location_combobox::{LocationCombobox, sec_pill, tier_color, tier_tag},
    modal_overlay,
    resizable_pane::pane_handle,
  },
  format::{fmt_count, fmt_isk, fmt_isk_opt},
  style::{color, control, radius, spacing, typography},
};

const EM_DASH: &str = "\u{2014}";
const PANE_PAD_X: f32 = 20.0;
const COLUMN_WIDTH: f32 = 300.0;
const ADD_MODAL_WIDTH: f32 = 480.0;
const PRICE_SIZE: f32 = 19.0;
const MARGIN_SIZE: f32 = 17.0;
const NOACCESS_HEIGHT: f32 = 196.0;
const DOT: f32 = 8.0;
const VLINE_HEIGHT: f32 = 44.0;

pub(in crate::features::market) fn surface(state: &State) -> Element<'_, Message> {
  Row::with_children(vec![
    browse::tree_pane(state),
    pane_handle(Message::PaneDragStart),
    right_pane(state),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn right_pane(state: &State) -> Element<'_, Message> {
  match state.selected_type_id() {
    None => empty_body(),
    Some(type_id) => populated(state, type_id),
  }
}

fn empty_body<'a>() -> Element<'a, Message> {
  shell::empty_state(
    Icon::market_tree(),
    "market.compare_empty_title",
    "market.compare_empty_body",
  )
}

fn populated(state: &State, type_id: i64) -> Element<'_, Message> {
  let columns = state.compare_columns();

  let mut sections: Vec<Element<'_, Message>> = Vec::new();
  if columns.len() >= 2 {
    sections.push(arb_strip(columns));
  }
  sections.push(columns_grid(columns));

  let body = scrollable(
    Column::with_children(sections)
      .spacing(spacing::SPACE_3)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_3_5,
        right: PANE_PAD_X,
        bottom: spacing::SPACE_6,
        left: PANE_PAD_X,
      }),
  )
  .style(control::scrollbar)
  .width(Length::Fill)
  .height(Length::Fill);

  container(
    Column::with_children(vec![item_toolbar(state, type_id, columns.len()), body.into()])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    ..container::Style::default()
  })
  .into()
}

fn item_toolbar(state: &State, type_id: i64, count: usize) -> Element<'_, Message> {
  let identity = book_view::find_identity(state.tree(), type_id);

  let title = Column::with_children(vec![
    text(identity.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(comparing_label(&identity.group, count))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::UNIT);

  let add: Element<'_, Message> = Button::secondary(tr_static("market.compare_add_market"))
    .icon(Icon::plus())
    .on_press(Message::CompareAddPickerToggled)
    .into();

  let row = Row::with_children(vec![
    book_view::item_icon(type_id),
    title.into(),
    Space::new().width(Length::Fill).into(),
    add,
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_4_5,
      right: PANE_PAD_X,
      bottom: spacing::SPACE_4_5,
      left: PANE_PAD_X,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: color::rule(),
        width: 0.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn comparing_label(group: &str, count: usize) -> String {
  let comparing = if count == 1 {
    t!("market.compare_comparing_one", count => count)
  } else {
    t!("market.compare_comparing_many", count => count)
  }
  .into_owned();

  if group.is_empty() {
    comparing
  } else {
    format!("{group} \u{b7} {comparing}")
  }
}

fn arb_strip(columns: &[CompareColumn]) -> Element<'_, Message> {
  match arbitrage(columns) {
    Arbitrage::Margin {
      buy_at,
      margin,
      margin_pct,
      sell_at,
    } => arb_margin(columns, buy_at, sell_at, margin, margin_pct),
    Arbitrage::None => arb_none(),
  }
}

fn arb_margin(
  columns: &[CompareColumn],
  buy_at: usize,
  sell_at: usize,
  margin: f64,
  margin_pct: f64,
) -> Element<'_, Message> {
  let phrase = Row::with_children(vec![
    arb_leg(
      tr_static("market.compare_arb_buy"),
      &columns[buy_at].place.name,
      columns[buy_at].best_sell(),
    ),
    Icon::arrow_right()
      .size(14.0)
      .color(color::text::secondary())
      .render::<Message>(),
    arb_leg(
      tr_static("market.compare_arb_sell"),
      &columns[sell_at].place.name,
      columns[sell_at].best_buy(),
    ),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .wrap();

  let value = Row::with_children(vec![
    text(fmt_isk(margin))
      .font(typography::mono::MEDIUM)
      .size(MARGIN_SIZE)
      .style(typography::colored(color::status::ONLINE))
      .into(),
    text(format!("{margin_pct:.1}%"))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let margin_block = Column::with_children(vec![
    eyebrow(tr_static("market.compare_arb_margin_label")),
    value.into(),
  ])
  .spacing(spacing::UNIT)
  .align_x(iced::alignment::Horizontal::Right);

  let content = Row::with_children(vec![
    eyebrow(tr_static("market.compare_arbitrage")),
    phrase.into(),
    Space::new().width(Length::Fill).into(),
    margin_block.into(),
  ])
  .spacing(spacing::SPACE_4_5)
  .align_y(Vertical::Center);

  strip_container(content.into(), true)
}

fn arb_leg<'a>(verb: &str, place: &str, price: Option<f64>) -> Element<'a, Message> {
  Row::with_children(vec![
    text(verb.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(place.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::accent()))
      .into(),
    text("@")
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    text(fmt_isk_opt(price))
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn arb_none<'a>() -> Element<'a, Message> {
  let content = Row::with_children(vec![
    eyebrow(tr_static("market.compare_arbitrage")),
    text(t!("market.compare_arb_none").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
    Space::new().width(Length::Fill).into(),
    text(EM_DASH)
      .font(typography::mono::REGULAR)
      .size(MARGIN_SIZE)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::SPACE_4_5)
  .align_y(Vertical::Center);

  strip_container(content.into(), false)
}

fn strip_container(content: Element<'_, Message>, positive: bool) -> Element<'_, Message> {
  let border_color = if positive {
    color::with_alpha(color::status::ONLINE, 0.4)
  } else {
    color::rule()
  };

  container(content)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: PANE_PAD_X,
      bottom: spacing::SPACE_3_5,
      left: PANE_PAD_X,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn columns_grid(columns: &[CompareColumn]) -> Element<'_, Message> {
  let cheapest = cheapest_sell(columns);
  let richest = richest_buy(columns);
  let multi = columns.len() > 1;

  let cards: Vec<Element<'_, Message>> = columns
    .iter()
    .enumerate()
    .map(|(index, column)| {
      market_column(
        column,
        multi && cheapest == Some(index),
        multi && richest == Some(index),
        multi,
      )
    })
    .collect();

  Row::with_children(cards).spacing(spacing::SPACE_3_5).wrap().into()
}

fn market_column(column: &CompareColumn, is_cheap: bool, is_rich: bool, removable: bool) -> Element<'_, Message> {
  let body = match column.access {
    BookAccess::NoAccess => noaccess_body(),
    _ => column_stats(column, is_cheap, is_rich),
  };

  let card = Column::with_children(vec![column_header(column, removable), body]).width(Length::Fixed(COLUMN_WIDTH));

  let lit = is_cheap || is_rich;
  let border_color = if lit {
    color::with_alpha(color::accent(), 0.45)
  } else {
    color::rule()
  };

  let styled = container(card)
    .width(Length::Fixed(COLUMN_WIDTH))
    .clip(true)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    });

  mouse_area(styled)
    .on_right_press(Message::CompareMenuOpened(column.place.id))
    .into()
}

fn column_header(column: &CompareColumn, removable: bool) -> Element<'_, Message> {
  let dot_color = tier_color(column.place.tier);
  let dot = container(Space::new())
    .width(Length::Fixed(DOT))
    .height(Length::Fixed(DOT))
    .style(move |_| container::Style {
      background: Some(Background::Color(dot_color)),
      border: Border {
        radius: (DOT / 2.0).into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let mut heading = Row::new()
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .push(tier_tag(column.place.tier))
    .push(
      text(column.place.name.clone())
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .wrapping(text::Wrapping::Word)
        .style(typography::colored(color::text::PRIMARY))
        .width(Length::Fill),
    );
  if let Some(pill) = sec_pill(column.place.tier, column.place.security_status) {
    heading = heading.push(pill);
  }

  let close: Element<'_, Message> = Button::ghost_icon(Icon::close())
    .size(Size::Sm)
    .on_press_maybe(removable.then_some(Message::CompareMarketRemoved(column.place.id)))
    .into();

  let row = Row::with_children(vec![dot.into(), heading.into(), close])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 0.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn column_stats(column: &CompareColumn, is_cheap: bool, is_rich: bool) -> Element<'_, Message> {
  let sell_accent = if is_cheap {
    color::accent()
  } else {
    color::status::ONLINE
  };
  let buy_accent = if is_rich { color::accent() } else { color::text::PRIMARY };

  let metrics = Row::with_children(vec![
    metric_cell(tr_static("market.compare_col_spread"), spread_label(column)),
    vline(),
    metric_cell(tr_static("market.compare_col_volume"), volume_label(column)),
  ])
  .width(Length::Fill);

  Column::with_children(vec![
    price_block(
      tr_static("market.compare_col_best_sell"),
      column.best_sell(),
      sell_accent,
      is_cheap.then(|| tr_static("market.compare_badge_cheapest")),
    ),
    hairline(),
    price_block(
      tr_static("market.compare_col_best_buy"),
      column.best_buy(),
      buy_accent,
      is_rich.then(|| tr_static("market.compare_badge_highest")),
    ),
    hairline(),
    metrics.into(),
  ])
  .width(Length::Fill)
  .into()
}

fn price_block<'a>(label: &str, value: Option<f64>, accent: iced::Color, badge: Option<&str>) -> Element<'a, Message> {
  let mut head = Row::new()
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .push(eyebrow(label));
  if let Some(badge) = badge {
    head = head.push(mini_badge(badge, accent));
  }

  container(
    Column::with_children(vec![
      head.into(),
      text(fmt_isk_opt(value))
        .font(typography::mono::MEDIUM)
        .size(PRICE_SIZE)
        .style(typography::colored(accent))
        .into(),
    ])
    .spacing(spacing::UNIT + 2.0),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3_5,
  })
  .into()
}

fn mini_badge<'a>(label: &str, accent: iced::Color) -> Element<'a, Message> {
  container(
    text(label.to_uppercase())
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(accent)),
  )
  .padding(Padding {
    top: 1.0,
    bottom: 1.0,
    left: spacing::UNIT + 2.0,
    right: spacing::UNIT + 2.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(accent, 0.14))),
    border: Border {
      color: color::with_alpha(accent, 0.45),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn metric_cell<'a>(label: &str, value: String) -> Element<'a, Message> {
  container(
    Column::with_children(vec![
      eyebrow(label),
      text(value)
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
    ])
    .spacing(spacing::UNIT),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2_5,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_3_5,
  })
  .into()
}

fn spread_label(column: &CompareColumn) -> String {
  column
    .spread_pct()
    .map_or_else(|| EM_DASH.to_owned(), |pct| format!("{pct:.1}%"))
}

fn volume_label(column: &CompareColumn) -> String {
  column.book_volume().map_or_else(|| EM_DASH.to_owned(), fmt_count)
}

fn noaccess_body<'a>() -> Element<'a, Message> {
  container(book_view::no_access_detail())
    .width(Length::Fill)
    .height(Length::Fixed(NOACCESS_HEIGHT))
    .into()
}

fn eyebrow<'a>(label: &str) -> Element<'a, Message> {
  text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()))
    .into()
}

fn hairline<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fill)
    .height(Length::Fixed(1.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

fn vline<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(1.0))
    .height(Length::Fixed(VLINE_HEIGHT))
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

pub(in crate::features::market) fn mount<'a>(base: Element<'a, Message>, state: &'a State) -> Element<'a, Message> {
  let base: Element<'a, Message> = if state.active_tab() == Tab::Compare {
    mouse_area(base).on_move(Message::CompareCursorMoved).into()
  } else {
    base
  };

  let layers = if let Some(menu) = state.compare_menu.as_ref() {
    vec![
      backdrop::click_catcher(Message::CompareMenuDismissed),
      menu_overlay(menu, state.compare_columns()),
    ]
  } else if state.compare_add_open() {
    modal_overlay::modal_layers(Message::CompareAddPickerToggled, add_modal(state))
  } else {
    Vec::new()
  };

  modal_overlay::stable_overlay(base, layers)
}

fn menu_overlay<'a>(menu: &CompareMenu, columns: &[CompareColumn]) -> Element<'a, Message> {
  let removable = columns.len() > 1;
  let title = columns
    .iter()
    .find(|column| column.place.id == menu.place_id)
    .map_or("", |column| column.place.name.as_str());

  let item = if removable {
    Item::danger(
      tr_static("market.compare_remove"),
      Message::CompareMarketRemoved(menu.place_id),
    )
  } else {
    Item::disabled(tr_static("market.compare_remove"))
  };

  context_menu::context_menu(title, vec![item], menu.anchor)
}

fn add_modal(state: &State) -> Element<'_, Message> {
  let picker = LocationCombobox::new()
    .placeholder(tr_static("market.compare_add_search_placeholder"))
    .query(state.compare_query())
    .results(state.compare_results().to_vec())
    .highlight(state.compare_highlight())
    .searching(state.compare_searching())
    .on_input(Message::CompareAddSearchChanged)
    .on_pick(Message::CompareMarketPicked)
    .width(Length::Fill)
    .popover();

  let title = Column::with_children(vec![
    text(tr_static("market.compare_add_title"))
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    eyebrow(tr_static("market.compare_add_kicker")),
  ])
  .spacing(spacing::UNIT + 2.0)
  .width(Length::Fill);

  let close: Element<'_, Message> = Button::secondary_icon(Icon::close())
    .size(Size::Sm)
    .on_press(Message::CompareAddPickerToggled)
    .into();

  let header = Row::with_children(vec![title.into(), close])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center);

  container(
    Column::with_children(vec![header.into(), picker])
      .spacing(spacing::SPACE_3)
      .width(Length::Fill),
  )
  .width(Length::Fixed(ADD_MODAL_WIDTH))
  .padding(spacing::SPACE_4_5)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: radius::CARD.into(),
    },
    ..container::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use iced::Point;

  use super::*;
  use crate::{
    clients::esi::models::market::RegionOrder,
    features::market::book,
    services::location_search::{LocationRef, LocationTier},
  };

  fn place(id: i64, tier: LocationTier, security_status: Option<f64>) -> LocationRef {
    LocationRef {
      context: None,
      id,
      name: "Market".to_owned(),
      security_status,
      tier: Some(tier),
    }
  }

  fn column(id: i64, tier: LocationTier, sell: Option<f64>, buy: Option<f64>) -> CompareColumn {
    let mut orders = Vec::new();
    if let Some(price) = sell {
      orders.push(RegionOrder {
        is_buy_order: false,
        price,
        volume_remain: 100,
        ..Default::default()
      });
    }
    if let Some(price) = buy {
      orders.push(RegionOrder {
        is_buy_order: true,
        price,
        volume_remain: 100,
        ..Default::default()
      });
    }
    let book = (!orders.is_empty()).then(|| book::build_order_book(orders));
    CompareColumn {
      access: BookAccess::Ok,
      book,
      place: place(id, tier, security_status_for(tier)),
    }
  }

  fn security_status_for(tier: LocationTier) -> Option<f64> {
    matches!(tier, LocationTier::Station | LocationTier::System).then_some(0.9)
  }

  fn selected_state(columns: Vec<CompareColumn>) -> State {
    let mut state = State::new();
    state.tab = Tab::Compare;
    state.selected = Some(34);
    state.compare = columns;
    state
  }

  #[test]
  fn it_renders_the_empty_state_without_a_selection() {
    let mut state = State::new();
    state.tab = Tab::Compare;

    let _el: Element<'_, Message> = surface(&state);
  }

  #[test]
  fn it_renders_a_column_grid_for_a_selected_item() {
    let state = selected_state(vec![
      column(60_003_760, LocationTier::Station, Some(5.0), Some(4.0)),
      column(60_008_494, LocationTier::Station, Some(9.0), Some(12.0)),
      column(10_000_002, LocationTier::Region, Some(7.0), Some(6.0)),
    ]);

    let _el: Element<'_, Message> = surface(&state);
  }

  #[test]
  fn it_renders_a_noaccess_column() {
    let mut columns = vec![column(60_003_760, LocationTier::Station, Some(5.0), Some(4.0))];
    columns.push(CompareColumn {
      access: BookAccess::NoAccess,
      book: None,
      place: place(1_035_000_000_001, LocationTier::Structure, None),
    });

    let state = selected_state(columns);
    let _el: Element<'_, Message> = surface(&state);
  }

  #[test]
  fn it_renders_the_no_margin_arbitrage_strip() {
    let state = selected_state(vec![
      column(60_003_760, LocationTier::Station, Some(10.0), Some(4.0)),
      column(60_008_494, LocationTier::Station, Some(12.0), Some(6.0)),
    ]);

    let _el: Element<'_, Message> = surface(&state);
  }

  #[test]
  fn it_renders_a_single_column_without_badges() {
    let state = selected_state(vec![column(60_003_760, LocationTier::Station, Some(5.0), Some(4.0))]);

    let _el: Element<'_, Message> = surface(&state);
  }

  #[test]
  fn it_mounts_the_add_market_modal() {
    let mut state = selected_state(vec![column(60_003_760, LocationTier::Station, Some(5.0), Some(4.0))]);
    state.compare_add_open = true;

    let _el: Element<'_, Message> = mount(Space::new().into(), &state);
  }

  #[test]
  fn it_mounts_the_column_context_menu() {
    let mut state = selected_state(vec![
      column(60_003_760, LocationTier::Station, Some(5.0), Some(4.0)),
      column(60_008_494, LocationTier::Station, Some(9.0), Some(12.0)),
    ]);
    state.compare_menu = Some(CompareMenu {
      anchor: Point::new(40.0, 60.0),
      place_id: 60_003_760,
    });

    let _el: Element<'_, Message> = mount(Space::new().into(), &state);
  }

  #[test]
  fn it_labels_a_single_market_in_the_singular() {
    assert_eq!(comparing_label("Tritanium", 1), "Tritanium \u{b7} comparing 1 market");
    assert_eq!(comparing_label("", 3), "comparing 3 markets");
  }
}
