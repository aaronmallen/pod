use std::{collections::HashMap, f32::consts::FRAC_PI_2};

use iced::{
  Background, Border, Color, Element, Length, Padding, Point, Radians, Rotation,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, Stack, button, container, mouse_area, scrollable, svg, text},
};

use super::{
  DropTarget, Filtered, Message, SquadGroup, State, card,
  card::{CardModel, format_isk, format_sp},
  card_failure, cursor, dragging_card, dragging_squad, drop_target, groups, is_squad_collapsed, load_error,
  squad_drop_target, unassigned, unassigned_squad_id,
};
use crate::{
  sync::SyncStatus,
  ui::{
    components::{eyebrow::eyebrow, rule, status},
    style::{color, control, radius, spacing, typography},
  },
};

const COLUMNS: usize = 3;
const GHOST_CARD_WIDTH: f32 = 320.0;
const GHOST_CARD_HEIGHT: f32 = spacing::layout::CARD_HEIGHT;
const GHOST_GRAB_FRACTION: f32 = 0.3;
const DROP_BORDER_ALPHA: f32 = 0.45;
const DROP_BORDER_WIDTH: f32 = 1.0;
const DROP_HIGHLIGHT_ALPHA: f32 = 0.08;
const EMPTY_CELL_HEIGHT: f32 = spacing::layout::CARD_HEIGHT;

static SQUADS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/squads.svg");
static CHEVRON_ICON: &[u8] = include_bytes!("../../../assets/images/icons/chevron.svg");
static DROP_ICON: &[u8] = include_bytes!("../../../assets/images/icons/drop.svg");
static SEARCH_ICON: &[u8] = include_bytes!("../../../assets/images/icons/search.svg");

const NO_MATCH_ICON: f32 = 28.0;

const SQUAD_STRIPE_WIDTH: f32 = 4.0;
const SQUAD_ICON_TILE: f32 = 40.0;
const SQUAD_ICON_GLYPH: f32 = 22.0;
const SQUAD_ICON_RADIUS: f32 = 9.0;
const SQUAD_CHEVRON_CELL: f32 = 44.0;
const SQUAD_CHEVRON_GLYPH: f32 = 14.0;
const SQUAD_NAME_SIZE: f32 = 19.0;
const SQUAD_ICON_BG_ALPHA: f32 = 0.14;
const SQUAD_ICON_BORDER_ALPHA: f32 = 0.4;
const SQUAD_BAR_PAD_Y: f32 = 16.0;
const BAR_STAT_PAD_X: f32 = 18.0;
const BAR_STAT_VALUE_SIZE: f32 = 15.0;
const EMPTY_DROP_PAD_Y: f32 = 28.0;
const EMPTY_DROP_ICON: f32 = 22.0;
const EMPTY_DROP_GAP: f32 = 6.0;
const BAR_STAT_RULE_HEIGHT: f32 = 34.0;
const SQUAD_KEBAB_CELL: f32 = 48.0;
const SQUAD_KEBAB_DOT: f32 = 3.0;
const SQUAD_KEBAB_DOT_GAP: f32 = 4.0;
const SQUAD_KEBAB_RULE_HEIGHT: f32 = 34.0;

pub(super) fn body<'a>(state: &'a State, sync: &SyncStatus) -> Element<'a, Message> {
  if let Some(error) = load_error(state) {
    return centered(message_text(
      format!("Couldn't load characters: {error}"),
      color::status::DANGER,
    ));
  }

  if state.is_filtered() {
    return filtered_body(state, sync);
  }

  if groups(state).is_empty() && unassigned(state).is_empty() {
    return empty_state();
  }

  let drag = DragContext {
    dragging: dragging_card(state),
    hovered: drop_target(state),
    squad: dragging_squad(state),
    squad_insert: squad_drop_target(state),
  };

  let mut sections: Vec<Element<'a, Message>> = Vec::new();
  for (index, group) in groups(state).iter().enumerate() {
    let collapsed = is_squad_collapsed(state, group.squad_id);
    sections.push(squad_section(group, index, collapsed, sync, drag));
  }
  if !unassigned(state).is_empty() {
    sections.push(unassigned_section(
      unassigned(state),
      unassigned_squad_id(state),
      !groups(state).is_empty(),
      sync,
      drag,
    ));
  }

  let content = Column::with_children(sections)
    .spacing(spacing::SPACE_6)
    .width(Length::Fill);

  let capped = container(content)
    .width(Length::Fill)
    .max_width(spacing::layout::GRID_MAX_WIDTH)
    .padding(spacing::SPACE_6);
  let centered = container(capped).width(Length::Fill).align_x(Horizontal::Center);

  let scroll = scrollable(centered)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill);

  let tracked = mouse_area(scroll).on_move(Message::DragMoved);

  let Some(dragged_id) = drag.dragging else {
    return tracked.into();
  };

  match (cursor(state), find_card(state, dragged_id)) {
    (Some(point), Some(model)) => Stack::with_children(vec![tracked.into(), ghost_layer(model, point)])
      .width(Length::Fill)
      .height(Length::Fill)
      .into(),
    _ => tracked.into(),
  }
}

fn find_card(state: &State, character_id: i64) -> Option<&CardModel> {
  groups(state)
    .iter()
    .flat_map(|group| group.cards.iter())
    .chain(unassigned(state).iter())
    .find(|card| card.character_id == character_id)
}

fn ghost_layer(model: &CardModel, cursor: Point) -> Element<'_, Message> {
  let top = (cursor.y - GHOST_CARD_HEIGHT * GHOST_GRAB_FRACTION).max(0.0);
  let left = (cursor.x - GHOST_CARD_WIDTH / 2.0).max(0.0);

  container(container(card::ghost(model)).width(Length::Fixed(GHOST_CARD_WIDTH)))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
      top,
      left,
      ..Padding::ZERO
    })
    .into()
}

#[derive(Clone, Copy)]
struct DragContext {
  dragging: Option<i64>,
  hovered: Option<DropTarget>,
  squad: Option<i64>,
  squad_insert: Option<usize>,
}

fn squad_section<'a>(
  group: &'a SquadGroup,
  index: usize,
  collapsed: bool,
  sync: &SyncStatus,
  drag: DragContext,
) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(3);
  if drag.squad.is_some() && drag.squad_insert == Some(index) && drag.squad != Some(group.squad_id) {
    children.push(insertion_rule());
  }
  children.push(squad_bar_source(group, index, collapsed, drag));

  if !collapsed {
    let body: Element<'a, Message> = if group.cards.is_empty() {
      empty_drop(
        &format!("No pilots in {} yet — drag a pilot here to assign them.", group.name),
        group.squad_id,
        drag,
      )
    } else {
      grid(&group.cards, group.squad_id, sync, drag)
    };
    children.push(body);
  }

  Column::with_children(children)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn insertion_rule<'a>() -> Element<'a, Message> {
  container(Space::new().width(Length::Fill).height(Length::Fixed(2.0)))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::PLASMA)),
      ..container::Style::default()
    })
    .into()
}

fn squad_bar_source<'a>(
  group: &'a SquadGroup,
  index: usize,
  collapsed: bool,
  drag: DragContext,
) -> Element<'a, Message> {
  let is_dragged = drag.squad == Some(group.squad_id);
  let bar = squad_bar(group, collapsed, is_dragged);

  let mut area = mouse_area(bar).on_press(Message::PickUpSquad(group.squad_id));
  if drag.squad.is_some() {
    area = area
      .on_enter(Message::HoverSquadSlot(index))
      .on_exit(Message::LeaveSquadSlot(index));
  }
  area.into()
}

fn squad_bar<'a>(group: &'a SquadGroup, collapsed: bool, dragged: bool) -> Element<'a, Message> {
  let accent = group.accent;

  let chevron_glyph = svg(svg::Handle::from_memory(CHEVRON_ICON))
    .width(Length::Fixed(SQUAD_CHEVRON_GLYPH))
    .height(Length::Fixed(SQUAD_CHEVRON_GLYPH))
    .rotation(Rotation::Floating(Radians(if collapsed { -FRAC_PI_2 } else { 0.0 })))
    .style(|_, _| svg::Style {
      color: Some(color::text::SECONDARY),
    });
  let chevron = mouse_area(
    container(chevron_glyph)
      .width(Length::Fixed(SQUAD_CHEVRON_CELL))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .on_press(Message::ToggleSquadCollapse(group.squad_id));

  let icon_tile = container(
    svg(svg::Handle::from_memory(SQUADS_ICON))
      .width(Length::Fixed(SQUAD_ICON_GLYPH))
      .height(Length::Fixed(SQUAD_ICON_GLYPH))
      .style(move |_, _| svg::Style {
        color: Some(accent),
      }),
  )
  .width(Length::Fixed(SQUAD_ICON_TILE))
  .height(Length::Fixed(SQUAD_ICON_TILE))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(accent, SQUAD_ICON_BG_ALPHA))),
    border: Border {
      color: color::with_alpha(accent, SQUAD_ICON_BORDER_ALPHA),
      width: 1.0,
      radius: SQUAD_ICON_RADIUS.into(),
    },
    ..container::Style::default()
  });
  let icon_cell = container(icon_tile).padding(Padding {
    right: spacing::SPACE_3_5,
    ..Padding::ZERO
  });

  let name_block = container(squad_name_block(group)).width(Length::Fill).padding(Padding {
    top: SQUAD_BAR_PAD_Y,
    bottom: SQUAD_BAR_PAD_Y,
    ..Padding::ZERO
  });

  let mut cells: Vec<Element<'a, Message>> = vec![
    Space::new().width(Length::Fixed(SQUAD_STRIPE_WIDTH)).into(),
    chevron.into(),
    icon_cell.into(),
    name_block.into(),
  ];
  if !group.cards.is_empty() {
    cells.push(squad_stats(&group.cards));
  }
  cells.push(squad_kebab(group.squad_id));
  let content = Row::with_children(cells).align_y(Vertical::Center).width(Length::Fill);

  let stripe = container(
    container(Space::new())
      .width(Length::Fixed(SQUAD_STRIPE_WIDTH))
      .height(Length::Fill)
      .style(move |_| container::Style {
        background: Some(Background::Color(accent)),
        ..container::Style::default()
      }),
  )
  .align_x(Horizontal::Left);

  let bar = Stack::with_children(vec![content.into(), stripe.into()]).width(Length::Fill);

  container(bar)
    .width(Length::Fill)
    .clip(true)
    .style(squad_bar_surface(dragged))
    .into()
}

fn squad_kebab<'a>(squad_id: i64) -> Element<'a, Message> {
  let dot = || status::dot_sized(color::text::SECONDARY, SQUAD_KEBAB_DOT);
  let dots = Column::with_children(vec![dot(), dot(), dot()])
    .spacing(SQUAD_KEBAB_DOT_GAP)
    .align_x(Horizontal::Center);

  let cell = container(dots)
    .width(Length::Fixed(SQUAD_KEBAB_CELL))
    .align_x(Horizontal::Center);

  mouse_area(Row::with_children(vec![rule::vertical(SQUAD_KEBAB_RULE_HEIGHT), cell.into()]).align_y(Vertical::Center))
    .on_press(Message::OpenSquadMenu(squad_id))
    .into()
}

fn squad_name_block<'a>(group: &'a SquadGroup) -> Element<'a, Message> {
  let accent = group.accent;
  let count = group.cards.len();

  let name = text(group.name.as_str())
    .font(typography::body::MEDIUM)
    .size(SQUAD_NAME_SIZE)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let pilots = text(format!("{count} {}", if count == 1 { "pilot" } else { "pilots" }))
    .font(typography::mono::MEDIUM)
    .size(typography::size::SM)
    .style(move |_| text::Style {
      color: Some(accent),
    });

  let title = Row::with_children(vec![name.into(), pilots.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Bottom);

  let mut children: Vec<Element<'a, Message>> = vec![title.into()];
  if let Some(description) = group.description.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
    children.push(
      text(description.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    );
  }

  Column::with_children(children).spacing(spacing::UNIT).into()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SquadStats {
  combined_isk: f64,
  combined_sp: i64,
  idle: usize,
  training: usize,
}

fn aggregate_stats(cards: &[CardModel]) -> SquadStats {
  let combined_isk = cards.iter().map(|card| card.wallet_balance.unwrap_or(0.0)).sum();
  let combined_sp = cards.iter().map(|card| card.total_sp.unwrap_or(0)).sum();
  let idle = cards.iter().filter(|card| card.training.is_none()).count();
  SquadStats {
    combined_isk,
    combined_sp,
    idle,
    training: cards.len() - idle,
  }
}

fn squad_stats<'a>(cards: &'a [CardModel]) -> Element<'a, Message> {
  let stats = aggregate_stats(cards);

  Row::with_children(vec![
    bar_stat(
      "Combined ISK",
      bar_stat_value(format_isk(Some(stats.combined_isk)), color::text::PRIMARY),
    ),
    bar_stat(
      "Combined SP",
      bar_stat_value(format_sp(Some(stats.combined_sp)), color::text::PRIMARY),
    ),
    bar_stat("Readiness", readiness(stats.training, stats.idle)),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn bar_stat<'a>(label: &'a str, value: Element<'a, Message>) -> Element<'a, Message> {
  let label = eyebrow(label, Some(color::text::TERTIARY));

  let body = container(Column::with_children(vec![label, value]).spacing(spacing::UNIT)).padding(Padding {
    left: BAR_STAT_PAD_X,
    right: BAR_STAT_PAD_X,
    ..Padding::ZERO
  });

  Row::with_children(vec![rule::vertical(BAR_STAT_RULE_HEIGHT), body.into()])
    .align_y(Vertical::Center)
    .into()
}

fn bar_stat_value<'a>(value: String, fill: Color) -> Element<'a, Message> {
  text(value)
    .font(typography::mono::MEDIUM)
    .size(BAR_STAT_VALUE_SIZE)
    .style(move |_| text::Style {
      color: Some(fill),
    })
    .into()
}

fn readiness<'a>(training: usize, idle: usize) -> Element<'a, Message> {
  let trained = bar_stat_value(format!("{training} training"), color::text::PRIMARY);
  if idle == 0 {
    return trained;
  }

  Row::with_children(vec![
    trained,
    bar_stat_value(format!(" · {idle} idle"), color::status::DANGER),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn squad_bar_surface(dragged: bool) -> impl Fn(&iced::Theme) -> container::Style {
  move |_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: if dragged {
        color::accent::PLASMA
      } else {
        color::with_alpha(color::text::PRIMARY, 0.1)
      },
      width: 1.0,
      radius: radius::CARD.into(),
    },
    ..container::Style::default()
  }
}

fn empty_drop<'a>(label: &str, squad_id: i64, drag: DragContext) -> Element<'a, Message> {
  let target = DropTarget {
    position: 0,
    squad_id,
  };
  let highlighted = drag.hovered == Some(target);

  let glyph = svg(svg::Handle::from_memory(DROP_ICON))
    .width(Length::Fixed(EMPTY_DROP_ICON))
    .height(Length::Fixed(EMPTY_DROP_ICON))
    .style(|_, _| svg::Style {
      color: Some(color::text::TERTIARY),
    });
  let caption = text(label.to_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::SECONDARY),
    });
  let column = Column::with_children(vec![glyph.into(), caption.into()])
    .spacing(EMPTY_DROP_GAP)
    .align_x(Horizontal::Center);

  let panel = container(column)
    .width(Length::Fill)
    .padding(Padding {
      top: EMPTY_DROP_PAD_Y,
      right: spacing::SPACE_6,
      bottom: EMPTY_DROP_PAD_Y,
      left: spacing::SPACE_6,
    })
    .align_x(Horizontal::Center)
    .style(move |_| container::Style {
      background: highlighted
        .then(|| Background::Color(color::with_alpha(color::accent::PLASMA, DROP_HIGHLIGHT_ALPHA))),
      border: Border {
        color: if highlighted {
          color::with_alpha(color::accent::PLASMA, DROP_BORDER_ALPHA)
        } else {
          color::with_alpha(color::text::PRIMARY, 0.18)
        },
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    });

  drop_cell(panel.into(), target, drag.dragging.is_some())
}

fn unassigned_section<'a>(
  cards: &'a [CardModel],
  squad_id: i64,
  show_header: bool,
  sync: &SyncStatus,
  drag: DragContext,
) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(2);
  if show_header {
    children.push(header_row("Unassigned", cards.len(), color::text::TERTIARY));
  }
  children.push(grid(cards, squad_id, sync, drag));

  Column::with_children(children)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn drop_cell<'a>(inner: Element<'a, Message>, target: DropTarget, dragging: bool) -> Element<'a, Message> {
  if !dragging {
    return inner;
  }

  mouse_area(inner)
    .on_enter(Message::HoverTarget(target))
    .on_exit(Message::LeaveTarget(target))
    .into()
}

fn header_row<'a>(label: &'a str, count: usize, count_color: Color) -> Element<'a, Message> {
  let label = text(label)
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::text::TERTIARY),
    });
  let count = text(count.to_string())
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(move |_| text::Style {
      color: Some(count_color),
    });
  let rule = container(Space::new())
    .width(Length::Fill)
    .height(Length::Fixed(1.0))
    .style(|_| container::Style {
      background: Some(color::with_alpha(color::text::PRIMARY, 0.1).into()),
      ..container::Style::default()
    });

  Row::with_children(vec![
    label.into(),
    count.into(),
    container(rule).width(Length::Fill).align_y(Vertical::Center).into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .into()
}

fn resolve_slots(cards: &[CardModel]) -> HashMap<i64, &CardModel> {
  let mut ordered: Vec<&CardModel> = cards.iter().collect();
  ordered.sort_by_key(|card| (card.position, card.character_id));

  let mut by_slot: HashMap<i64, &CardModel> = HashMap::with_capacity(cards.len());
  for card in ordered {
    let mut slot = card.position;
    while by_slot.contains_key(&slot) {
      slot += 1;
    }
    by_slot.insert(slot, card);
  }
  by_slot
}

fn grid<'a>(cards: &'a [CardModel], squad_id: i64, sync: &SyncStatus, drag: DragContext) -> Element<'a, Message> {
  let card_at = resolve_slots(cards);
  let max_slot = card_at.keys().copied().max().unwrap_or(0);
  let trailing = if drag.dragging.is_some() { 2 } else { 1 };
  let row_count = (max_slot / COLUMNS as i64 + trailing) as usize;

  let mut rows: Vec<Element<'a, Message>> = Vec::with_capacity(row_count);
  for row_idx in 0..row_count {
    let mut cells: Vec<Element<'a, Message>> = Vec::with_capacity(COLUMNS);
    for col_idx in 0..COLUMNS {
      let slot = row_idx as i64 * COLUMNS as i64 + col_idx as i64;
      let target = DropTarget {
        position: slot,
        squad_id,
      };
      let cell: Element<'a, Message> = match card_at.get(&slot) {
        Some(model) => card::card(
          model,
          card_failure(sync, model.character_id),
          drag.dragging == Some(model.character_id),
        ),
        None if drag.dragging.is_some() => empty_cell(drag.hovered == Some(target)),
        None => empty_spacer(),
      };
      cells.push(drop_cell(cell, target, drag.dragging.is_some()));
    }
    rows.push(Row::with_children(cells).spacing(spacing::SPACE_3_5).into());
  }

  Column::with_children(rows)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn empty_spacer<'a>() -> Element<'a, Message> {
  Space::new()
    .width(Length::Fill)
    .height(Length::Fixed(EMPTY_CELL_HEIGHT))
    .into()
}

fn empty_cell<'a>(highlighted: bool) -> Element<'a, Message> {
  container(Space::new().width(Length::Fill).height(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fixed(EMPTY_CELL_HEIGHT))
    .style(move |_| container::Style {
      background: highlighted
        .then(|| Background::Color(color::with_alpha(color::accent::PLASMA, DROP_HIGHLIGHT_ALPHA))),
      border: Border {
        color: if highlighted {
          color::with_alpha(color::accent::PLASMA, DROP_BORDER_ALPHA)
        } else {
          color::with_alpha(color::text::PRIMARY, 0.08)
        },
        width: DROP_BORDER_WIDTH,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn filtered_body<'a>(state: &'a State, sync: &SyncStatus) -> Element<'a, Message> {
  match state.filtered() {
    Some(Filtered::Loaded(cards)) if cards.is_empty() => no_matches(),
    Some(Filtered::Loaded(cards)) => {
      let capped = container(filtered_grid(cards, sync))
        .width(Length::Fill)
        .max_width(spacing::layout::GRID_MAX_WIDTH)
        .padding(spacing::SPACE_6);
      let centered = container(capped).width(Length::Fill).align_x(Horizontal::Center);
      let scroll = scrollable(centered)
        .style(crate::ui::style::control::scrollbar)
        .width(Length::Fill)
        .height(Length::Fill);
      mouse_area(scroll).on_move(Message::DragMoved).into()
    }
    Some(Filtered::Error(error)) => centered(message_text(format!("Search failed: {error}"), color::status::DANGER)),
    Some(Filtered::Loading) | None => centered(message_text("Searching…".to_owned(), color::text::SECONDARY)),
  }
}

fn filtered_grid<'a>(cards: &'a [CardModel], sync: &SyncStatus) -> Element<'a, Message> {
  let mut rows: Vec<Element<'a, Message>> = Vec::with_capacity(cards.len() / COLUMNS + 1);
  for chunk in cards.chunks(COLUMNS) {
    let mut cells: Vec<Element<'a, Message>> = Vec::with_capacity(COLUMNS);
    for model in chunk {
      cells.push(card::card(model, card_failure(sync, model.character_id), false));
    }
    while cells.len() < COLUMNS {
      cells.push(empty_spacer());
    }
    rows.push(Row::with_children(cells).spacing(spacing::SPACE_3_5).into());
  }

  Column::with_children(rows)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn no_matches<'a>() -> Element<'a, Message> {
  centered(
    Column::with_children(vec![
      svg(svg::Handle::from_memory(SEARCH_ICON))
        .width(Length::Fixed(NO_MATCH_ICON))
        .height(Length::Fixed(NO_MATCH_ICON))
        .style(|_, _| svg::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
      message_text("No capsuleers match".to_owned(), color::text::PRIMARY),
      text("Try a different search or clear filters")
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      button(
        text("Clear filters")
          .font(typography::body::REGULAR)
          .size(typography::size::SM),
      )
      .padding(control::padding())
      .on_press(Message::ClearSearch)
      .style(control::primary_button)
      .into(),
    ])
    .spacing(spacing::SPACE_3)
    .align_x(Horizontal::Center)
    .into(),
  )
}

fn empty_state<'a>() -> Element<'a, Message> {
  centered(
    Column::with_children(vec![
      message_text("No characters yet".to_owned(), color::text::PRIMARY),
      text("Add a character to start syncing.")
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_3)
    .align_x(Horizontal::Center)
    .into(),
  )
}

fn centered(content: Element<'_, Message>) -> Element<'_, Message> {
  container(content)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn message_text<'a>(content: String, fill: Color) -> Element<'a, Message> {
  text(content)
    .size(typography::size::MD)
    .style(move |_| text::Style {
      color: Some(fill),
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::{super::Drag, *};
  use crate::features::character_manager::card::TagChip;

  fn card_model(id: i64) -> CardModel {
    card_model_at(id, 0)
  }

  fn card_model_at(id: i64, position: i64) -> CardModel {
    CardModel {
      accent: None,
      character_id: id,
      corp_ticker: "CORP1".to_owned(),
      docked: Some(true),
      has_portrait: false,
      location: None,
      name: "Pilot".to_owned(),
      needs_reauth: false,
      position,
      tags: vec![TagChip {
        color: None,
        id: 1,
        name: "Main".to_owned(),
      }],
      total_sp: None,
      training: None,
      wallet_balance: None,
    }
  }

  fn squad_group(squad_id: i64, name: &str, cards: Vec<CardModel>) -> SquadGroup {
    SquadGroup {
      accent: color::accent::PLASMA,
      cards,
      color_hex: Some("#3FB8DB".to_owned()),
      description: Some("Fleet anchor squad".to_owned()),
      name: name.to_owned(),
      squad_id,
    }
  }

  fn no_drag() -> DragContext {
    DragContext {
      dragging: None,
      hovered: None,
      squad: None,
      squad_insert: None,
    }
  }

  fn stat_card(id: i64, wallet: Option<f64>, sp: Option<i64>, training: bool) -> CardModel {
    let mut card = card_model(id);
    card.wallet_balance = wallet;
    card.total_sp = sp;
    card.training = training.then(|| super::super::card::Training {
      level: 4,
      progress: 0.5,
      remaining: "1d".to_owned(),
      skill: "Skill".to_owned(),
    });
    card
  }

  mod aggregate {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_isk_and_sp_and_splits_idle_from_training() {
      let cards = [
        stat_card(1, Some(4_000_000_000.0), Some(80_000_000), true),
        stat_card(2, Some(1_000_000_000.0), Some(20_000_000), false),
      ];

      let stats = aggregate_stats(&cards);

      assert_eq!(stats.combined_isk, 5_000_000_000.0);
      assert_eq!(stats.combined_sp, 100_000_000);
      assert_eq!(stats.idle, 1);
      assert_eq!(stats.training, 1);
    }

    #[test]
    fn it_treats_absent_isk_and_sp_as_zero() {
      let cards = [
        stat_card(1, Some(2_000_000.0), Some(5_000_000), true),
        stat_card(2, None, None, true),
      ];

      let stats = aggregate_stats(&cards);

      assert_eq!(stats.combined_isk, 2_000_000.0);
      assert_eq!(stats.combined_sp, 5_000_000);
      assert_eq!(stats.idle, 0);
      assert_eq!(stats.training, 2);
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_the_empty_state() {
      let state = State::new();
      let sync = SyncStatus::new();

      let _el: Element<'_, Message> = body(&state, &sync);
    }

    #[test]
    fn it_renders_a_squad_group_and_the_unassigned_bucket() {
      let mut state = State::new();
      state.groups = vec![squad_group(1, "Supers", vec![card_model(1)])];
      state.unassigned = vec![card_model(2)];
      let sync = SyncStatus::new();

      let _el: Element<'_, Message> = body(&state, &sync);
    }

    #[test]
    fn it_renders_the_flat_filtered_grid_bypassing_the_squad_layout() {
      let mut state = State::new();
      state.groups = vec![squad_group(1, "Supers", vec![card_model(1)])];
      state.filtered = Some(Filtered::Loaded(vec![
        card_model(2),
        card_model(3),
        card_model(4),
        card_model(5),
      ]));
      let sync = SyncStatus::new();

      let _el: Element<'_, Message> = body(&state, &sync);
    }

    #[test]
    fn it_renders_the_no_matches_state_for_an_empty_filter() {
      let mut state = State::new();
      state.groups = vec![squad_group(1, "Supers", vec![card_model(1)])];
      state.filtered = Some(Filtered::Loaded(vec![]));
      let sync = SyncStatus::new();

      let _el: Element<'_, Message> = body(&state, &sync);
    }

    #[test]
    fn it_renders_the_loading_filter_state() {
      let mut state = State::new();
      state.filtered = Some(Filtered::Loading);
      let sync = SyncStatus::new();

      let _el: Element<'_, Message> = body(&state, &sync);
    }

    #[test]
    fn it_renders_the_error_filter_state() {
      let mut state = State::new();
      state.filtered = Some(Filtered::Error("boom".to_owned()));
      let sync = SyncStatus::new();

      let _el: Element<'_, Message> = body(&state, &sync);
    }

    #[test]
    fn it_renders_a_squad_group_as_the_squad_bar() {
      let group = squad_group(1, "Supers", vec![card_model(1), card_model(2)]);

      let _bar: Element<'_, Message> = squad_bar(&group, false, false);
      let _name_block: Element<'_, Message> = squad_name_block(&group);
      let _section: Element<'_, Message> = squad_section(&group, 0, false, &SyncStatus::new(), no_drag());
    }

    #[test]
    fn it_renders_a_squad_bar_without_a_description() {
      let mut group = squad_group(1, "Solo", vec![card_model(1)]);
      group.description = None;
      {
        let _bar: Element<'_, Message> = squad_bar(&group, false, false);
      }

      group.description = Some("   ".to_owned());
      let _blank: Element<'_, Message> = squad_bar(&group, false, false);
    }

    #[test]
    fn it_renders_an_empty_squad_with_the_dashed_drop_affordance() {
      use iced::advanced::widget::Tree;
      use pretty_assertions::assert_eq;

      let group = squad_group(2, "Reserves", Vec::new());

      let drop: Element<'_, Message> = empty_drop(
        "No pilots in Reserves yet — drag a pilot here to assign them.",
        2,
        no_drag(),
      );
      assert_eq!(Tree::new(drop.as_widget()).children.len(), 2);

      let _section: Element<'_, Message> = squad_section(&group, 0, false, &SyncStatus::new(), no_drag());
    }

    #[test]
    fn it_renders_the_unassigned_bucket_with_the_lighter_header() {
      let cards = [card_model(1)];
      let _section: Element<'_, Message> = unassigned_section(&cards, 99, true, &SyncStatus::new(), no_drag());
    }

    #[test]
    fn it_gates_the_unassigned_header_on_squads_being_present() {
      use iced::advanced::widget::Tree;
      use pretty_assertions::assert_eq;

      let cards = [card_model(1)];

      let with_header: Element<'_, Message> = unassigned_section(&cards, 99, true, &SyncStatus::new(), no_drag());
      let without_header: Element<'_, Message> = unassigned_section(&cards, 99, false, &SyncStatus::new(), no_drag());

      assert_eq!(Tree::new(with_header.as_widget()).children.len(), 2);
      assert_eq!(Tree::new(without_header.as_widget()).children.len(), 1);
    }

    #[test]
    fn it_renders_both_cards_when_two_share_a_position() {
      use pretty_assertions::assert_eq;

      let cards = [card_model_at(1, 0), card_model_at(2, 0)];

      let slots = resolve_slots(&cards);

      assert_eq!(slots.len(), 2);
      assert_eq!(slots[&0].character_id, 1);
      assert_eq!(slots[&1].character_id, 2);
    }

    #[test]
    fn it_renders_a_collapsed_squad_as_the_bar_without_its_member_grid() {
      use iced::advanced::widget::Tree;
      use pretty_assertions::assert_eq;

      let group = squad_group(1, "Supers", vec![card_model(1), card_model(2)]);

      let expanded: Element<'_, Message> = squad_section(&group, 0, false, &SyncStatus::new(), no_drag());
      let collapsed: Element<'_, Message> = squad_section(&group, 0, true, &SyncStatus::new(), no_drag());

      assert_eq!(Tree::new(expanded.as_widget()).children.len(), 2);
      assert_eq!(Tree::new(collapsed.as_widget()).children.len(), 1);
    }

    #[test]
    fn it_renders_a_grid_with_a_card_at_a_non_zero_slot_without_panicking() {
      let cards = [card_model_at(7, 4)];
      let _grid: Element<'_, Message> = grid(&cards, 3, &SyncStatus::new(), no_drag());

      let _hovered: Element<'_, Message> = grid(
        &cards,
        3,
        &SyncStatus::new(),
        DragContext {
          dragging: Some(99),
          hovered: Some(DropTarget {
            position: 2,
            squad_id: 3,
          }),
          squad: None,
          squad_insert: None,
        },
      );

      let _squad_drag: Element<'_, Message> = grid(
        &cards,
        3,
        &SyncStatus::new(),
        DragContext {
          dragging: None,
          hovered: None,
          squad: Some(3),
          squad_insert: None,
        },
      );
    }

    #[test]
    fn it_declares_the_fixed_card_height_for_empty_cells() {
      use iced::advanced::Widget;
      use pretty_assertions::assert_eq;

      let cell = empty_cell(false);
      assert_eq!(
        Widget::<Message, _, _>::size(cell.as_widget()).height,
        Length::Fixed(spacing::layout::CARD_HEIGHT),
      );

      let spacer = empty_spacer();
      assert_eq!(
        Widget::<Message, _, _>::size(spacer.as_widget()).height,
        Length::Fixed(spacing::layout::CARD_HEIGHT),
      );
    }

    #[test]
    fn it_renders_a_card_with_a_sync_error() {
      use crate::sync::{Event, JobKey, JobKind, Subject};

      let mut state = State::new();
      state.unassigned = vec![card_model(7)];
      let mut sync = SyncStatus::new();
      sync.apply(&Event::Failed {
        key: JobKey::new(JobKind::CharacterWallet, Subject::Character(7)),
        reason: "boom".to_owned(),
      });

      let _el: Element<'_, Message> = body(&state, &sync);
    }

    #[test]
    fn it_renders_the_drag_ghost_overlay_without_panicking() {
      let mut state = State::new();
      state.unassigned = vec![card_model(1), card_model(2)];
      state.dragging = Some(Drag::Card(2));
      state.cursor = Some(iced::Point::new(120.0, 240.0));
      let sync = SyncStatus::new();

      let _el: Element<'_, Message> = body(&state, &sync);
    }

    #[test]
    fn it_renders_the_tracked_body_before_the_first_cursor_move() {
      let mut state = State::new();
      state.unassigned = vec![card_model(1)];
      state.dragging = Some(Drag::Card(1));
      let sync = SyncStatus::new();

      let _el: Element<'_, Message> = body(&state, &sync);
    }
  }

  mod sizing {
    use iced::{Length, advanced::Widget};
    use pretty_assertions::assert_ne;

    use super::*;

    #[test]
    fn the_squad_bar_does_not_declare_a_fill_height() {
      let group = squad_group(1, "Supers", vec![card_model(1)]);
      let bar = squad_bar(&group, false, false);

      assert_ne!(Widget::<Message, _, _>::size(bar.as_widget()).height, Length::Fill);
    }
  }

  mod stats {
    use iced::advanced::widget::Tree;
    use pretty_assertions::assert_eq;

    use super::*;

    fn content_row_children(bar: &Element<'_, Message>) -> usize {
      let tree = Tree::new(bar.as_widget());
      tree.children[0].children.len()
    }

    #[test]
    fn it_appends_the_idle_run_only_when_a_pilot_is_idle() {
      let all_training: Element<'_, Message> = readiness(3, 0);
      assert_eq!(Tree::new(all_training.as_widget()).children.len(), 0);

      let with_idle: Element<'_, Message> = readiness(2, 1);
      assert_eq!(Tree::new(with_idle.as_widget()).children.len(), 2);
    }

    #[test]
    fn it_renders_one_cell_per_combined_stat() {
      let cards = [stat_card(1, Some(1.0e9), Some(50_000_000), true)];
      let group: Element<'_, Message> = squad_stats(&cards);

      assert_eq!(Tree::new(group.as_widget()).children.len(), 3);
    }

    #[test]
    fn it_renders_the_three_stat_cells_for_a_non_empty_squad_and_none_for_an_empty_one() {
      let populated = squad_group(1, "Supers", vec![stat_card(1, Some(1.0e9), Some(50_000_000), true)]);
      let empty = squad_group(2, "Reserves", Vec::new());

      let populated_bar: Element<'_, Message> = squad_bar(&populated, false, false);
      let empty_bar: Element<'_, Message> = squad_bar(&empty, false, false);

      assert_eq!(
        content_row_children(&populated_bar),
        content_row_children(&empty_bar) + 1
      );
    }
  }
}
