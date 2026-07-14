use iced::{
  Background, Border, Color, ContentFit, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, image, scrollable, text},
};

use super::{
  Message, OrderRow, OrdersScope, State,
  i18n::tr_static,
  tree::{MarketNode, MarketTree},
};
use crate::{
  clients::eve_image::Size,
  store::images::{self, IconResolution},
  ui::{
    components::{
      anchored_dropdown::AnchoredDropdown,
      clip::clip_layer,
      header::{header as shared_header, header_divider},
      icon::Icon,
      icon_tile::icon_tile,
      picker::{
        PickerGroup, TriggerPortrait, picker_character_row, picker_dropdown as picker_dropdown_panel, picker_row,
        picker_trigger, trigger_badge_identity, trigger_identity,
      },
    },
    format::{fmt_count, fmt_isk, fmt_isk_full},
    style::{
      color,
      control::{bordered_pane, scrollbar},
      spacing, typography,
    },
  },
};

const ICON_SIZE: Size = Size::S64;
const ICON_TILE: f32 = 28.0;
const ROW_HEIGHT: f32 = 58.0;
const RAIL_WIDTH: f32 = 2.0;
const CELL_PAD: f32 = 16.0;
const DOT: f32 = 5.0;
const EXPIRES_WARN_DAYS: i64 = 14;
const SCOPE_POPOVER_WIDTH: f32 = 320.0;

const SIDE_WIDTH: f32 = 92.0;
const LOCATION_WIDTH: f32 = 188.0;
const FILLED_WIDTH: f32 = 150.0;
const PRICE_WIDTH: f32 = 128.0;
const STATUS_WIDTH: f32 = 172.0;
const EXPIRES_WIDTH: f32 = 74.0;
const CHARACTER_WIDTH: f32 = 168.0;
const ACTION_WIDTH: f32 = 52.0;

const EM_DASH: &str = "\u{2014}";

// ── Header (scope picker + stats) ─────────────────────────────────

pub(super) fn header(state: &State) -> iced::Element<'_, Message> {
  let data = state.orders();
  let active_sub = t!(
    "market.orders_stat_active_sub",
    sell => data.sell_count,
    buy => data.buy_count,
  )
  .into_owned();
  let (outbid_color, outbid_sub) = if data.outbid_count > 0 {
    (
      color::status::DANGER,
      t!("market.orders_stat_outbid_attention").into_owned(),
    )
  } else {
    (
      color::status::ONLINE,
      t!("market.orders_stat_outbid_clear").into_owned(),
    )
  };

  let left: Vec<iced::Element<'_, Message>> = vec![
    scope_picker(state),
    header_divider(),
    orders_stat(
      tr_static("market.orders_stat_active"),
      data.active_count.to_string(),
      color::text::PRIMARY,
      active_sub,
    ),
    header_divider(),
    orders_stat(
      tr_static("market.orders_stat_outbid"),
      data.outbid_count.to_string(),
      outbid_color,
      outbid_sub,
    ),
    header_divider(),
    orders_stat(
      tr_static("market.orders_stat_sell_listed"),
      fmt_isk(data.sell_listed),
      color::text::PRIMARY,
      tr_static("market.orders_stat_sell_listed_sub").to_owned(),
    ),
    header_divider(),
    orders_stat(
      tr_static("market.orders_stat_escrow"),
      fmt_isk(data.buy_escrow),
      color::text::PRIMARY,
      tr_static("market.orders_stat_escrow_sub").to_owned(),
    ),
  ];

  shared_header(left, Vec::new())
}

fn orders_stat<'a>(label: &str, value: String, value_color: Color, sub: String) -> iced::Element<'a, Message> {
  Column::with_children(vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .into(),
    Row::with_children(vec![
      text(value)
        .font(typography::mono::MEDIUM)
        .size(typography::size::LG)
        .style(typography::colored(value_color))
        .into(),
      text(sub.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::secondary()))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Bottom)
    .into(),
  ])
  .spacing(spacing::UNIT)
  .into()
}

pub(super) fn outbid_badge(state: &State) -> String {
  // The badge reflects the synced outbid-alert count so it shows on every tab, not only after the
  // My Orders table has loaded its live rows.
  let count = state.alert_outbid().max(state.outbid_count() as i64);
  if count > 0 {
    t!("market.orders_tab_badge", count => count).into_owned()
  } else {
    String::new()
  }
}

// ── Scope picker ──────────────────────────────────────────────────

fn scope_picker(state: &State) -> iced::Element<'_, Message> {
  let trigger = picker_trigger(
    scope_trigger(state),
    state.orders_picker_open(),
    Message::OrdersScopeToggled,
  );
  let popover = state.orders_picker_open().then(|| scope_dropdown(state));

  AnchoredDropdown::new(trigger, popover)
    .on_dismiss(Message::OrdersScopeDismissed)
    .popover_width(SCOPE_POPOVER_WIDTH)
    .into()
}

fn scope_trigger(state: &State) -> iced::Element<'_, Message> {
  match state.orders_scope() {
    OrdersScope::All => trigger_badge_identity(
      Icon::contracts(),
      t!("market.orders_scope_all").into_owned(),
      t!("market.orders_scope_all_sub", count => state.orders().roster.len()).into_owned(),
    ),
    OrdersScope::Character(id) => match state.orders().roster.iter().find(|pilot| pilot.id == id) {
      Some(pilot) => trigger_identity(
        pilot.name.clone(),
        t!("market.orders_scope_character_sub").into_owned(),
        Some(TriggerPortrait {
          id: pilot.id,
          name: pilot.name.clone(),
          path: pilot.portrait.clone(),
        }),
      ),
      None => trigger_identity(t!("market.orders_scope_character").into_owned(), String::new(), None),
    },
  }
}

fn scope_dropdown(state: &State) -> iced::Element<'_, Message> {
  let mut groups: Vec<PickerGroup<'_, Message>> = vec![PickerGroup {
    title: None,
    items: vec![picker_row(
      t!("market.orders_scope_all").into_owned(),
      matches!(state.orders_scope(), OrdersScope::All),
      Message::OrdersScopeSelected(OrdersScope::All),
    )],
  }];

  let roster = &state.orders().roster;
  if !roster.is_empty() {
    groups.push(PickerGroup {
      title: Some(t!("market.orders_scope_characters").into_owned()),
      items: roster
        .iter()
        .map(|pilot| {
          picker_character_row(
            pilot.id,
            pilot.name.clone(),
            String::new(),
            pilot.portrait.clone(),
            None,
            matches!(state.orders_scope(), OrdersScope::Character(id) if id == pilot.id),
            None,
            Message::OrdersScopeSelected(OrdersScope::Character(pilot.id)),
          )
        })
        .collect(),
    });
  }

  picker_dropdown_panel(groups)
}

// ── Body (table) ──────────────────────────────────────────────────

pub(super) fn surface(state: &State) -> iced::Element<'_, Message> {
  let data = state.orders();
  if data.rows.is_empty() {
    return empty_scope();
  }

  let store = images::default_store();
  let show_char = state.orders_show_character();
  let tree = state.tree();

  let mut rows: Vec<iced::Element<'_, Message>> = vec![header_row(show_char)];
  for row in &data.rows {
    rows.push(order_row(tree, &store, row, show_char));
  }

  let body = scrollable(Column::with_children(rows).width(Length::Fill))
    .style(scrollbar)
    .width(Length::Fill)
    .height(Length::Fill);

  container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(bordered_pane)
    .into()
}

fn empty_scope() -> iced::Element<'static, Message> {
  container(
    text(t!("market.orders_empty_scope").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(spacing::SPACE_6)
  .into()
}

fn header_row<'a>(show_char: bool) -> iced::Element<'a, Message> {
  let mut cells: Vec<iced::Element<'a, Message>> = vec![
    head_cell("market.orders_col_side", Length::Fixed(SIDE_WIDTH), false),
    head_cell("market.orders_col_item", Length::Fill, false),
    head_cell("market.orders_col_location", Length::Fixed(LOCATION_WIDTH), false),
    head_cell("market.orders_col_filled", Length::Fixed(FILLED_WIDTH), true),
    head_cell("market.orders_col_price", Length::Fixed(PRICE_WIDTH), true),
    head_cell("market.orders_col_status", Length::Fixed(STATUS_WIDTH), false),
    head_cell("market.orders_col_expires", Length::Fixed(EXPIRES_WIDTH), false),
  ];
  if show_char {
    cells.push(head_cell(
      "market.orders_col_character",
      Length::Fixed(CHARACTER_WIDTH),
      false,
    ));
  }
  cells.push(container(Space::new()).width(Length::Fixed(ACTION_WIDTH)).into());

  let inner = Row::with_children(vec![row_rail(false), Row::with_children(cells).into()]);
  container(inner)
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: bottom_rule(),
      ..container::Style::default()
    })
    .into()
}

fn head_cell<'a>(label_key: &str, width: Length, align_right: bool) -> iced::Element<'a, Message> {
  let label = text(t!(label_key).into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .wrapping(text::Wrapping::None)
    .style(typography::colored(color::text::tertiary()));

  container(label)
    .width(width)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: CELL_PAD,
      bottom: spacing::SPACE_3,
      left: CELL_PAD,
    })
    .align_x(if align_right {
      Horizontal::Right
    } else {
      Horizontal::Left
    })
    .into()
}

fn order_row<'a>(
  tree: &MarketTree,
  store: &images::Store,
  row: &OrderRow,
  show_char: bool,
) -> iced::Element<'a, Message> {
  let mut cells: Vec<iced::Element<'a, Message>> = vec![
    side_cell(row.is_buy),
    item_cell(tree, store, row.type_id),
    location_cell(row),
    filled_cell(row),
    price_cell(row.price),
    status_cell(row),
    expires_cell(row),
  ];
  if show_char {
    cells.push(character_cell(&row.character_name, row.owner_is_corp));
  }
  cells.push(action_cell(row));

  let content = Row::with_children(cells)
    .align_y(Vertical::Center)
    .height(Length::Fixed(ROW_HEIGHT));
  let inner = Row::with_children(vec![row_rail(row.outbid), content.into()]).align_y(Vertical::Center);

  let outbid = row.outbid;
  container(inner)
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: outbid.then_some(Background::Color(color::with_alpha(color::status::DANGER, 0.05))),
      border: bottom_rule(),
      ..container::Style::default()
    })
    .into()
}

fn row_rail<'a>(outbid: bool) -> iced::Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(RAIL_WIDTH))
    .height(Length::Fixed(ROW_HEIGHT))
    .style(move |_| container::Style {
      background: Some(Background::Color(if outbid {
        color::status::DANGER
      } else {
        Color::TRANSPARENT
      })),
      ..container::Style::default()
    })
    .into()
}

fn side_cell<'a>(is_buy: bool) -> iced::Element<'a, Message> {
  let (label_key, arrow, tint) = if is_buy {
    ("market.orders_side_buy", "\u{2193}", color::status::DANGER)
  } else {
    ("market.orders_side_sell", "\u{2191}", color::status::ONLINE)
  };
  let label = format!("{arrow} {}", t!(label_key).into_owned().to_uppercase());

  let pill = container(
    text(label)
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(tint)),
  )
  .padding(Padding {
    top: 3.0,
    right: 8.0,
    bottom: 3.0,
    left: 8.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(tint, 0.10))),
    border: Border {
      color: color::with_alpha(tint, 0.30),
      radius: 3.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  cell_wrap(pill.into(), Length::Fixed(SIDE_WIDTH), Horizontal::Left)
}

fn item_cell<'a>(tree: &MarketTree, store: &images::Store, type_id: i64) -> iced::Element<'a, Message> {
  let (name, group) = find_identity(tree, type_id);
  let label = Column::with_children(vec![
    text(name)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(group)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::UNIT / 2.0);

  let content = Row::with_children(vec![order_icon(store, type_id), label.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center);

  cell_wrap(content.into(), Length::Fill, Horizontal::Left)
}

fn order_icon<'a>(store: &images::Store, type_id: i64) -> iced::Element<'a, Message> {
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

fn location_cell<'a>(row: &OrderRow) -> iced::Element<'a, Message> {
  let content = Column::with_children(vec![
    text(row.region_label.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::secondary()))
      .into(),
    text(row.system_label.clone())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::UNIT / 2.0);

  cell_wrap(content.into(), Length::Fixed(LOCATION_WIDTH), Horizontal::Left)
}

fn filled_cell<'a>(row: &OrderRow) -> iced::Element<'a, Message> {
  let filled = (row.volume_total - row.volume_remain).max(0);
  let label = text(format!("{}/{}", fmt_count(filled), fmt_count(row.volume_total)))
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .wrapping(text::Wrapping::None)
    .style(typography::colored(color::text::secondary()));

  let content = Column::with_children(vec![
    container(label).width(Length::Fill).align_x(Horizontal::Right).into(),
    progress_bar(row.volume_total, row.volume_remain, row.done),
  ])
  .spacing(spacing::UNIT + 1.0);

  cell_wrap(content.into(), Length::Fixed(FILLED_WIDTH), Horizontal::Right)
}

fn progress_bar<'a>(volume_total: i64, volume_remain: i64, done: bool) -> iced::Element<'a, Message> {
  let filled = (volume_total - volume_remain).max(0);
  let pct = if volume_total > 0 {
    (filled as f32 / volume_total as f32).clamp(0.0, 1.0)
  } else {
    0.0
  };
  let fill_weight = (pct * 1000.0).round() as u16;
  let rest_weight = 1000u16.saturating_sub(fill_weight);
  let fill_color = if done { color::status::ONLINE } else { color::accent() };

  let mut segments: Vec<iced::Element<'a, Message>> = Vec::new();
  if fill_weight > 0 {
    segments.push(
      container(Space::new())
        .width(Length::FillPortion(fill_weight))
        .height(Length::Fill)
        .style(move |_| container::Style {
          background: Some(Background::Color(fill_color)),
          ..container::Style::default()
        })
        .into(),
    );
  }
  if rest_weight > 0 {
    segments.push(container(Space::new()).width(Length::FillPortion(rest_weight)).into());
  }

  container(Row::with_children(segments))
    .width(Length::Fill)
    .height(Length::Fixed(3.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      border: Border {
        radius: 1.5.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn price_cell<'a>(price: f64) -> iced::Element<'a, Message> {
  let label = text(fmt_price(price))
    .font(typography::mono::MEDIUM)
    .size(typography::size::MD)
    .wrapping(text::Wrapping::None)
    .style(typography::colored(color::text::PRIMARY));
  cell_wrap(label.into(), Length::Fixed(PRICE_WIDTH), Horizontal::Right)
}

fn status_cell<'a>(row: &OrderRow) -> iced::Element<'a, Message> {
  let content: iced::Element<'a, Message> = if row.done {
    Row::with_children(vec![
      Icon::check().size(12.0).color(color::status::ONLINE).render(),
      status_label("market.orders_status_filled", color::status::ONLINE),
    ])
    .spacing(spacing::UNIT + 2.0)
    .align_y(Vertical::Center)
    .into()
  } else if row.outbid {
    outbid_status(row)
  } else {
    Row::with_children(vec![
      status_dot(color::status::ONLINE),
      status_label("market.orders_status_best", color::status::ONLINE),
    ])
    .spacing(spacing::UNIT + 2.0)
    .align_y(Vertical::Center)
    .into()
  };

  cell_wrap(content, Length::Fixed(STATUS_WIDTH), Horizontal::Left)
}

fn outbid_status<'a>(row: &OrderRow) -> iced::Element<'a, Message> {
  let pill = container(
    Row::with_children(vec![
      status_dot(color::status::DANGER),
      status_label("market.orders_status_outbid", color::status::DANGER),
    ])
    .spacing(spacing::UNIT + 2.0)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 3.0,
    right: 7.0,
    bottom: 3.0,
    left: 7.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::status::DANGER, 0.12))),
    border: Border {
      color: color::with_alpha(color::status::DANGER, 0.34),
      radius: 4.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  let detail = t!(
    "market.orders_status_outbid_detail",
    price => fmt_price(row.best.unwrap_or(0.0)),
    gap => format!("{:.2}", row.gap_pct.unwrap_or(0.0)),
  )
  .into_owned();

  Column::with_children(vec![
    pill.into(),
    text(detail)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::status::DANGER))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .into()
}

fn status_label<'a>(label_key: &str, tint: Color) -> iced::Element<'a, Message> {
  text(t!(label_key).into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .wrapping(text::Wrapping::None)
    .style(typography::colored(tint))
    .into()
}

fn status_dot<'a>(tint: Color) -> iced::Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(DOT))
    .height(Length::Fixed(DOT))
    .style(move |_| container::Style {
      background: Some(Background::Color(tint)),
      border: Border {
        radius: 999.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn expires_cell<'a>(row: &OrderRow) -> iced::Element<'a, Message> {
  let (label, tint) = if row.done {
    (EM_DASH.to_owned(), color::text::secondary())
  } else if row.expires_days <= EXPIRES_WARN_DAYS {
    (format!("{}d", row.expires_days), color::status::WARNING)
  } else {
    (format!("{}d", row.expires_days), color::text::secondary())
  };

  let content = text(label)
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .wrapping(text::Wrapping::None)
    .style(typography::colored(tint));

  cell_wrap(content.into(), Length::Fixed(EXPIRES_WIDTH), Horizontal::Left)
}

fn character_cell<'a>(name: &str, owner_is_corp: bool) -> iced::Element<'a, Message> {
  let mut children: Vec<iced::Element<'a, Message>> = vec![
    initials_tile(name),
    text(name.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ];
  if owner_is_corp {
    children.push(corp_owner_badge());
  }
  let content = Row::with_children(children)
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

  cell_wrap(content.into(), Length::Fixed(CHARACTER_WIDTH), Horizontal::Left)
}

fn corp_owner_badge<'a>() -> iced::Element<'a, Message> {
  container(
    text(t!("market.orders_owner_corp").into_owned().to_uppercase())
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::accent())),
  )
  .padding(Padding {
    top: 2.0,
    right: 6.0,
    bottom: 2.0,
    left: 6.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent(), 0.10))),
    border: Border {
      color: color::with_alpha(color::accent(), 0.30),
      radius: 3.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn initials_tile<'a>(name: &str) -> iced::Element<'a, Message> {
  container(
    text(initials(name))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fixed(18.0))
  .height(Length::Fixed(18.0))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06))),
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn action_cell<'a>(row: &OrderRow) -> iced::Element<'a, Message> {
  let content: iced::Element<'a, Message> = if row.outbid && !row.owner_is_corp {
    open_in_game_button(row.character_id, row.type_id)
  } else {
    Space::new().into()
  };
  cell_wrap(content, Length::Fixed(ACTION_WIDTH), Horizontal::Right)
}

fn open_in_game_button<'a>(character_id: i64, type_id: i64) -> iced::Element<'a, Message> {
  button(
    container(Icon::arrow_out().size(13.0).color(color::text::secondary()).render())
      .width(Length::Fixed(24.0))
      .height(Length::Fixed(24.0))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .padding(0)
  .on_press(Message::OpenInGame {
    character_id,
    type_id,
  })
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hover.then_some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.05))),
      border: Border {
        color: if hover { color::rule_strong() } else { color::rule() },
        radius: 5.0.into(),
        width: 1.0,
      },
      text_color: if hover {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

// ── Helpers ───────────────────────────────────────────────────────

fn cell_wrap<'a>(content: iced::Element<'a, Message>, width: Length, align: Horizontal) -> iced::Element<'a, Message> {
  container(content)
    .width(width)
    .padding(Padding {
      top: 0.0,
      right: CELL_PAD,
      bottom: 0.0,
      left: CELL_PAD,
    })
    .align_x(align)
    .align_y(Vertical::Center)
    .into()
}

fn bottom_rule() -> Border {
  Border {
    color: color::rule(),
    radius: 0.0.into(),
    width: 0.0,
  }
}

fn find_identity(tree: &MarketTree, type_id: i64) -> (String, String) {
  for node in &tree.roots {
    if let Some(identity) = find_in_node(node, type_id) {
      return identity;
    }
  }
  (
    t!("market.book_item_fallback", id => type_id).into_owned(),
    String::new(),
  )
}

fn find_in_node(node: &MarketNode, type_id: i64) -> Option<(String, String)> {
  for leaf in &node.items {
    if leaf.type_id == type_id {
      return Some((leaf.name.clone(), node.name.clone()));
    }
  }
  for child in &node.children {
    if let Some(identity) = find_in_node(child, type_id) {
      return Some(identity);
    }
  }
  None
}

fn initials(name: &str) -> String {
  name
    .split_whitespace()
    .take(2)
    .filter_map(|word| word.chars().next())
    .collect::<String>()
    .to_uppercase()
}

fn fmt_price(value: f64) -> String {
  if value < 1000.0 {
    format!("{value:.2}")
  } else {
    fmt_isk_full(value)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::features::market::{OrderPilot, OrderRow, OrdersData, OrdersScope};

  fn row(is_buy: bool, outbid: bool, done: bool) -> OrderRow {
    OrderRow {
      character_id: 90,
      character_name: "Test Pilot".to_owned(),
      owner_is_corp: false,
      type_id: 587,
      region_label: "The Forge".to_owned(),
      system_label: "Jita".to_owned(),
      price: 1_842_000.0,
      is_buy,
      volume_remain: if done { 0 } else { 40 },
      volume_total: 100,
      expires_days: 7,
      done,
      outbid,
      best: outbid.then_some(1_800_000.0),
      gap_pct: outbid.then_some(2.34),
    }
  }

  fn corp_row() -> OrderRow {
    OrderRow {
      owner_is_corp: true,
      character_id: 98_000_001,
      character_name: "Test Corp".to_owned(),
      ..row(false, false, false)
    }
  }

  fn populated_state(scope: OrdersScope) -> State {
    let mut state = State::new();
    let data = OrdersData {
      scope,
      rows: vec![
        row(false, true, false),
        row(true, false, false),
        row(false, false, true),
        corp_row(),
      ],
      roster: vec![OrderPilot {
        id: 90,
        name: "Test Pilot".to_owned(),
        portrait: None,
      }],
      active_count: 2,
      sell_count: 1,
      buy_count: 1,
      outbid_count: 1,
      sell_listed: 5_000_000.0,
      buy_escrow: 2_000_000.0,
    };
    super::super::update(&mut state, Message::OrdersScopeSelected(scope));
    super::super::update(&mut state, Message::OrdersLoaded(Box::new(data)));
    state
  }

  #[test]
  fn it_renders_the_empty_scope_state() {
    let state = State::new();
    let _el: iced::Element<'_, Message> = surface(&state);
  }

  #[test]
  fn it_renders_the_table_for_all_characters() {
    let state = populated_state(OrdersScope::All);
    assert!(state.orders_show_character());
    let _el: iced::Element<'_, Message> = surface(&state);
  }

  #[test]
  fn it_renders_the_table_for_a_single_character() {
    let state = populated_state(OrdersScope::Character(90));
    assert!(!state.orders_show_character());
    let _el: iced::Element<'_, Message> = surface(&state);
  }

  #[test]
  fn it_renders_the_header_with_stats_and_picker() {
    let mut state = populated_state(OrdersScope::All);
    {
      let _closed: iced::Element<'_, Message> = header(&state);
    }

    super::super::update(&mut state, Message::OrdersScopeToggled);
    assert!(state.orders_picker_open());
    let _open: iced::Element<'_, Message> = header(&state);
  }

  #[test]
  fn it_shows_the_outbid_badge_only_when_outbid() {
    let state = populated_state(OrdersScope::All);
    assert_eq!(
      outbid_badge(&state),
      t!("market.orders_tab_badge", count => 1_usize).into_owned()
    );

    assert!(outbid_badge(&State::new()).is_empty());
  }

  #[test]
  fn it_builds_two_letter_initials() {
    assert_eq!(initials("Jita Trader"), "JT");
    assert_eq!(initials("Solo"), "S");
    assert_eq!(initials(""), "");
  }

  #[test]
  fn it_formats_small_and_large_prices() {
    assert_eq!(fmt_price(6.4), "6.40");
    assert_eq!(fmt_price(1_842_000.0), "1,842,000");
  }
}
