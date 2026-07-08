use std::{collections::BTreeMap, sync::OnceLock};

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, image, scrollable, text, tooltip},
};

use super::{
  Category, HEADER_SIDE_PADDING, Message, RosterCorp, RosterPilot, State, fmt_count, fmt_isk, fmt_volume, owner_label,
};
use crate::{
  store::{
    images::IconResolution,
    model::{
      ENTITY_TYPE_ASSET, Tag,
      asset_query::{InventoryRow, SortColumn, SortDirection},
    },
  },
  ui::{
    components::{
      add_tag_modal::AddTagMessage,
      avatar::avatar,
      badge::badge,
      button::{Button, Size},
      chip::Chip,
      empty_state::empty_state as shared_empty_state,
      eyebrow::eyebrow,
      icon::Icon,
      icon_tile::icon_tile,
      rule,
      table_cell::TableCell,
      text_input::TextInput,
      virtual_list::{self, VirtualList, VirtualListConfig},
    },
    style::{color, radius, spacing, typography},
  },
};

const ICON_BOX: f32 = 26.0;
const INDENT_STEP: f32 = 16.0;
const OWNER_PORTRAIT: f32 = 22.0;
const TOGGLE_WIDTH: f32 = 16.0;

const REPROC_EDGE_WIDTH: f32 = 2.0;

/// Relative flex widths for the inventory columns, in column order:
/// Item, Group, Category, Qty, Volume, Unit, Value, Owner, Location.
/// Header and data rows share these so they stay aligned.
const COLUMN_PORTIONS: [u16; 9] = [4, 2, 1, 2, 2, 2, 2, 2, 2];

const FILTER_HELP_EXAMPLES: [(&str, &str); 7] = [
  ("category:ship", "all ships"),
  ("region:\"The Forge\"", "in The Forge"),
  ("name:Tritanium", "name contains Tritanium"),
  ("category:ship -name:Rifter", "ships, not Rifters"),
  ("system:Jita type:stack", "stacks in Jita"),
  ("owner:me category:module", "my modules"),
  ("tag:Sell -tag:Junk", "tagged Sell, not Junk"),
];

const FILTER_HELP_KEYS: [(&str, &str, &str); 10] = [
  ("name:", "n:", "type name (partial)"),
  ("group:", "g:", "group name (partial)"),
  ("category:", "cat:", "category key (exact)"),
  ("region:", "r:", "region name (exact)"),
  ("constellation:", "c:", "constellation (exact)"),
  ("system:", "s:", "system name (partial)"),
  ("location:", "loc:", "location name (partial)"),
  ("owner:", "", "character name or \"me\""),
  ("tag:", "", "asset tag name (exact)"),
  ("type:", "", "singleton \u{b7} bpc \u{b7} bpo \u{b7} stack"),
];

const ESTIMATED_ROW_HEIGHT: f32 = 44.0;

pub(super) fn filter_bar(state: &State) -> Element<'_, Message> {
  let search = container(
    Row::with_children(vec![
      container(search_field(state)).width(Length::Fill).into(),
      save_filter_button(state.can_save_filter()),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::SPACE_3_5,
    right: HEADER_SIDE_PADDING,
    bottom: spacing::SPACE_3,
    left: HEADER_SIDE_PADDING,
  });

  let pips = container(
    Row::with_children(vec![
      category_pills(state.category()),
      Space::new().width(Length::Fill).into(),
      totals_summary(state),
    ])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 0.0,
    right: HEADER_SIDE_PADDING,
    bottom: spacing::SPACE_3,
    left: HEADER_SIDE_PADDING,
  });

  container(Column::with_children(vec![search.into(), pips.into()]).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn category_pills<'a>(active: Category) -> Element<'a, Message> {
  let pills: Vec<Element<'a, Message>> = Category::ALL
    .iter()
    .map(|category| category_pill(*category, *category == active))
    .collect();

  Row::with_children(pills)
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into()
}

fn category_pill<'a>(category: Category, selected: bool) -> Element<'a, Message> {
  let text_color = if selected {
    color::accent()
  } else {
    color::text::secondary()
  };

  button(
    text(category.label())
      .font(typography::body::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(text_color)),
  )
  .padding(Padding {
    top: spacing::UNIT + 1.0,
    right: spacing::SPACE_2_5,
    bottom: spacing::UNIT + 1.0,
    left: spacing::SPACE_2_5,
  })
  .on_press(Message::CategorySelected(category))
  .style(move |_, status| {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let background = if selected {
      Some(Background::Color(color::with_alpha(color::accent(), 0.12)))
    } else if hovered {
      Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06)))
    } else {
      None
    };
    button::Style {
      background,
      border: Border {
        color: if selected {
          color::with_alpha(color::accent(), 0.35)
        } else {
          color::with_alpha(color::text::PRIMARY, 0.1)
        },
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn search_field(state: &State) -> Element<'_, Message> {
  let mut right: Vec<Element<'_, Message>> = Vec::new();

  if !state.search().trim().is_empty() {
    let count = state.inventory().len() as i64;
    let label = if count == 1 {
      t!("assets.inventory.match_count_one", count => fmt_count(count)).into_owned()
    } else {
      t!("assets.inventory.match_count_other", count => fmt_count(count)).into_owned()
    };
    right.push(
      text(label)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }

  right.push(help_toggle(state.inventory_help_open()));

  let trailing = Row::with_children(right)
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center);

  TextInput::new(search_placeholder(), state.search(), Message::SearchChanged)
    .input_id(crate::features::shell::focus_search::assets_search_id())
    .leading_icon(Icon::search())
    .on_submit(Message::SearchSubmitted)
    .trailing(trailing.into())
    .render()
}

fn search_placeholder() -> &'static str {
  static PLACEHOLDER: OnceLock<String> = OnceLock::new();
  PLACEHOLDER.get_or_init(|| t!("assets.inventory.search_placeholder").into_owned())
}

fn save_filter_button<'a>(enabled: bool) -> Element<'a, Message> {
  Button::primary(t!("assets.inventory.save").into_owned())
    .icon(Icon::star())
    .size(Size::Sm)
    .on_press_maybe(enabled.then_some(Message::SaveFilterOpened))
    .into()
}

fn help_toggle<'a>(open: bool) -> Element<'a, Message> {
  let color = if open {
    color::accent()
  } else {
    color::text::secondary()
  };
  button(Icon::help().size(15.0).color(color).render())
    .padding(Padding {
      top: spacing::UNIT,
      right: spacing::SPACE_2,
      bottom: spacing::UNIT,
      left: spacing::SPACE_2,
    })
    .on_press(Message::InventoryHelpToggled)
    .style(move |_, _| button::Style {
      background: open.then(|| Background::Color(color::with_alpha(color::accent(), 0.12))),
      border: Border {
        color: if open {
          color::with_alpha(color::accent(), 0.45)
        } else {
          color::with_alpha(color::text::PRIMARY, 0.1)
        },
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..button::Style::default()
    })
    .into()
}

fn totals_summary(state: &State) -> Element<'_, Message> {
  let (label, count, value, volume) = if state.inventory_selection_count() > 0 {
    let (value, volume) = state.inventory_selection_totals();
    (
      t!("assets.inventory.summary_selected").into_owned(),
      state.inventory_selection_count() as i64,
      value,
      volume,
    )
  } else {
    let rows = state.inventory();
    (
      t!("assets.inventory.summary_rows").into_owned(),
      rows.len() as i64,
      rows.iter().map(|r| r.value).sum(),
      rows.iter().map(|r| r.row_volume).sum(),
    )
  };

  container(
    Row::with_children(vec![
      summary_stat(&label, fmt_count(count)),
      summary_stat(&t!("assets.inventory.summary_value"), fmt_isk(value)),
      summary_stat(&t!("assets.inventory.summary_volume"), fmt_volume(volume)),
    ])
    .spacing(spacing::SPACE_6)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::UNIT + 2.0,
    right: spacing::SPACE_3_5,
    bottom: spacing::UNIT + 2.0,
    left: spacing::SPACE_3_5,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn summary_stat<'a>(label: &str, value: String) -> Element<'a, Message> {
  Column::with_children(vec![
    eyebrow(label, Some(color::text::secondary())),
    text(value)
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(1.0)
  .into()
}

pub(super) fn header(state: &State) -> Element<'_, Message> {
  let (sort, dir) = state.inventory_sort();
  column_header(sort, dir)
}

pub(super) fn body(state: &State) -> Element<'_, Message> {
  let rows = state.inventory();
  if rows.is_empty() {
    let message = if state.search().trim().is_empty() {
      empty_scope_message()
    } else {
      empty_filtered_message()
    };
    return empty_state(message);
  }

  let flat = flatten_rows(state);
  let offset = state.inventory_scroll_offset();

  virtual_list::responsive_window(move |viewport_height| {
    let config = VirtualListConfig::new(flat.len(), ESTIMATED_ROW_HEIGHT)
      .viewport_height(viewport_height)
      .scroll_offset(offset);
    let list = VirtualList::new(config, |index| flat_row_view(state, &flat[index])).view();

    // The cursor is tracked at the feature-root base (see `shell`) so the right-click menu anchors at
    // the pointer in the overlay's coordinate space; this scrollable only reports its scroll offset.
    scrollable(list)
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill)
      .on_scroll(|viewport| Message::InventoryScrolled {
        relative: viewport.relative_offset().y,
        absolute: viewport.absolute_offset().y,
      })
      .into()
  })
}

fn flat_row_view<'a>(state: &'a State, flat_row: &FlatRow<'a>) -> Element<'a, Message> {
  match flat_row {
    FlatRow::Item {
      row,
      depth,
    } => {
      let expanded = state.container_is_open(row.item_id);
      let tags = state.asset_tags_for(row.item_id);
      let selected = state.inventory_row_selected(row.item_id);
      let hovered = state.inventory_row_hovered(row.item_id);
      table_row(
        row,
        *depth,
        state.roster(),
        state.corporations(),
        expanded,
        tags,
        selected,
        hovered,
      )
    }
    FlatRow::Division(header) => division_header_row(header),
  }
}

pub(super) fn has_rows(state: &State) -> bool {
  !state.inventory().is_empty()
}

const OFFICE_TYPE_ID: i64 = 27;

enum FlatRow<'a> {
  Division(DivisionHeader),
  Item { row: &'a InventoryRow, depth: i64 },
}

struct DivisionHeader {
  depth: i64,
  division_key: i64,
  name: String,
  office_item_id: i64,
  open: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HangarDivision {
  Deliveries,
  Numbered(i64),
}

impl HangarDivision {
  fn from_location_flag(flag: &str) -> Option<Self> {
    match flag {
      "CorpSAG1" => Some(HangarDivision::Numbered(1)),
      "CorpSAG2" => Some(HangarDivision::Numbered(2)),
      "CorpSAG3" => Some(HangarDivision::Numbered(3)),
      "CorpSAG4" => Some(HangarDivision::Numbered(4)),
      "CorpSAG5" => Some(HangarDivision::Numbered(5)),
      "CorpSAG6" => Some(HangarDivision::Numbered(6)),
      "CorpSAG7" => Some(HangarDivision::Numbered(7)),
      "CorpDeliveries" => Some(HangarDivision::Deliveries),
      _ => None,
    }
  }

  /// A stable per-office key for expand/collapse state; also the display order (deliveries last).
  pub(super) fn expand_key(self) -> i64 {
    match self {
      HangarDivision::Numbered(n) => n,
      HangarDivision::Deliveries => 8,
    }
  }
}

pub(super) fn is_office(row: &InventoryRow) -> bool {
  row.is_container && row.type_id == OFFICE_TYPE_ID
}

/// Buckets an office's children into ordered, populated hangar divisions (SAG1-7 then Deliveries)
/// plus the loose office-root items. Empty divisions are absent from the result.
pub(super) fn group_office_children(
  children: &[InventoryRow],
) -> (Vec<(HangarDivision, Vec<&InventoryRow>)>, Vec<&InventoryRow>) {
  let mut buckets: BTreeMap<i64, (HangarDivision, Vec<&InventoryRow>)> = BTreeMap::new();
  let mut office_root: Vec<&InventoryRow> = Vec::new();
  for child in children {
    match HangarDivision::from_location_flag(&child.location_flag) {
      Some(division) => buckets
        .entry(division.expand_key())
        .or_insert_with(|| (division, Vec::new()))
        .1
        .push(child),
      // OfficeFolder (and any unrecognized flag) is the office's own root, not a named hangar
      // division, so it nests directly under the office with no synthetic header.
      None => office_root.push(child),
    }
  }
  (buckets.into_values().collect(), office_root)
}

fn division_display_name(division: HangarDivision, custom: Option<&str>) -> String {
  match division {
    HangarDivision::Deliveries => t!("assets.inventory.corp_deliveries").into_owned(),
    HangarDivision::Numbered(n) => custom
      .filter(|name| !name.is_empty())
      .map(str::to_owned)
      .unwrap_or_else(|| t!("assets.inventory.division_fallback", n => n).into_owned()),
  }
}

fn flatten_rows(state: &State) -> Vec<FlatRow<'_>> {
  let mut out = Vec::with_capacity(state.inventory().len());
  for inventory_row in state.inventory() {
    push_row(state, &mut out, inventory_row, 0);
  }
  out
}

fn push_row<'a>(state: &'a State, out: &mut Vec<FlatRow<'a>>, inventory_row: &'a InventoryRow, depth: i64) {
  out.push(FlatRow::Item {
    row: inventory_row,
    depth,
  });

  if inventory_row.is_container
    && state.container_is_open(inventory_row.item_id)
    && let Some(children) = state.container_children_of(inventory_row.item_id)
  {
    if is_office(inventory_row) {
      push_office_children(state, out, inventory_row, children, depth + 1);
    } else {
      for child in children {
        push_row(state, out, child, depth + 1);
      }
    }
  }
}

fn push_office_children<'a>(
  state: &'a State,
  out: &mut Vec<FlatRow<'a>>,
  office: &'a InventoryRow,
  children: &'a [InventoryRow],
  depth: i64,
) {
  let (divisions, office_root) = group_office_children(children);
  for (division, items) in divisions {
    let open = state.division_is_open(office.item_id, division.expand_key());
    let custom = match division {
      HangarDivision::Numbered(n) => state.hangar_division_name(office.owner_id, n),
      HangarDivision::Deliveries => None,
    };
    out.push(FlatRow::Division(DivisionHeader {
      depth,
      division_key: division.expand_key(),
      name: division_display_name(division, custom),
      office_item_id: office.item_id,
      open,
    }));
    if open {
      for item in items {
        push_row(state, out, item, depth + 1);
      }
    }
  }
  for item in office_root {
    push_row(state, out, item, depth);
  }
}

fn column_header<'a>(sort: SortColumn, dir: SortDirection) -> Element<'a, Message> {
  let columns: [(String, Option<SortColumn>, bool); 8] = [
    (
      t!("assets.inventory.column_item").into_owned(),
      Some(SortColumn::Name),
      false,
    ),
    (
      t!("assets.inventory.column_group").into_owned(),
      Some(SortColumn::Group),
      false,
    ),
    (
      t!("assets.inventory.column_category").into_owned(),
      Some(SortColumn::Category),
      false,
    ),
    (
      t!("assets.inventory.column_qty").into_owned(),
      Some(SortColumn::Quantity),
      true,
    ),
    (
      t!("assets.inventory.column_volume").into_owned(),
      Some(SortColumn::Volume),
      true,
    ),
    (
      t!("assets.inventory.column_unit").into_owned(),
      Some(SortColumn::UnitPrice),
      true,
    ),
    (
      t!("assets.inventory.column_value").into_owned(),
      Some(SortColumn::Value),
      true,
    ),
    (
      t!("assets.inventory.column_owner").into_owned(),
      Some(SortColumn::Owner),
      false,
    ),
  ];

  let mut cells: Vec<Element<'a, Message>> =
    vec![Space::new().width(Length::Fixed(TOGGLE_WIDTH)).into(), header_spacer()];
  for ((label, column, right), &portion) in columns.into_iter().zip(COLUMN_PORTIONS.iter()) {
    cells.push(portioned(header_cell(label, column, right, sort, dir), portion));
  }
  cells.push(portioned(
    plain_header(t!("assets.inventory.column_location").into_owned()),
    COLUMN_PORTIONS[8],
  ));

  container(
    Row::with_children(cells)
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .height(Length::Fixed(30.0))
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 0.0,
    right: HEADER_SIDE_PADDING,
    bottom: 0.0,
    left: HEADER_SIDE_PADDING,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn header_spacer<'a>() -> Element<'a, Message> {
  Space::new().width(Length::Fixed(ICON_BOX)).into()
}

fn portioned<'a>(cell: Element<'a, Message>, portion: u16) -> Element<'a, Message> {
  container(cell).width(Length::FillPortion(portion)).into()
}

fn plain_header<'a>(label: String) -> Element<'a, Message> {
  container(
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .align_y(Vertical::Center)
  .into()
}

fn header_cell<'a>(
  label: String,
  column: Option<SortColumn>,
  right: bool,
  sort: SortColumn,
  dir: SortDirection,
) -> Element<'a, Message> {
  let active = column == Some(sort);
  let label_color = if active {
    color::accent()
  } else {
    color::text::secondary()
  };

  let mut content: Vec<Element<'a, Message>> = vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(label_color))
      .into(),
  ];
  if active {
    let caret = match dir {
      SortDirection::Ascending => Icon::chevron_up(),
      SortDirection::Descending => Icon::chevron_down(),
    };
    content.push(caret.size(12.0).color(color::accent()).render());
  }

  let inner = Row::with_children(content)
    .spacing(spacing::UNIT)
    .align_y(Vertical::Center);

  let aligned = container(inner)
    .width(Length::Fill)
    .align_x(if right { Horizontal::Right } else { Horizontal::Left });

  match column {
    Some(col) => button(aligned)
      .padding(0)
      .width(Length::Fill)
      .on_press(Message::SortSelected(col))
      .style(|_, _| button::Style::default())
      .into(),
    None => aligned.into(),
  }
}

#[allow(clippy::too_many_arguments)]
fn table_row<'a>(
  inventory_row: &'a InventoryRow,
  depth: i64,
  roster: &[RosterPilot],
  corporations: &[RosterCorp],
  expanded: bool,
  tags: Vec<&'a Tag>,
  selected: bool,
  hovered: bool,
) -> Element<'a, Message> {
  let cells: Vec<Element<'a, Message>> = vec![
    row_prefix(inventory_row, depth, expanded),
    row_icon(inventory_row),
    portioned(name_cell(inventory_row, tags, hovered), COLUMN_PORTIONS[0]),
    portioned(group_cell(inventory_row), COLUMN_PORTIONS[1]),
    portioned(category_cell(inventory_row), COLUMN_PORTIONS[2]),
    portioned(
      numeric_cell(fmt_count(inventory_row.quantity), color::text::PRIMARY),
      COLUMN_PORTIONS[3],
    ),
    portioned(
      numeric_cell(fmt_volume(inventory_row.row_volume), color::text::secondary()),
      COLUMN_PORTIONS[4],
    ),
    portioned(
      numeric_cell(fmt_isk(inventory_row.unit_price), color::text::secondary()),
      COLUMN_PORTIONS[5],
    ),
    portioned(value_cell(inventory_row), COLUMN_PORTIONS[6]),
    portioned(
      owner_cell(inventory_row.owner_id, roster, corporations),
      COLUMN_PORTIONS[7],
    ),
    portioned(
      text_cell(
        inventory_row.location_label.clone().unwrap_or_default(),
        color::text::secondary(),
      ),
      COLUMN_PORTIONS[8],
    ),
  ];

  let row = container(
    Row::with_children(cells)
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    right: HEADER_SIDE_PADDING,
    bottom: spacing::SPACE_2,
    left: HEADER_SIDE_PADDING,
  })
  .style(move |_| container::Style {
    background: selected.then_some(Background::Color(color::with_alpha(color::accent(), 0.14))),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.06),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  });

  let edged: Element<'a, Message> = if inventory_row.worth_reprocessing() {
    reproc_edge(row.into())
  } else {
    row.into()
  };

  select_wrap(edged, inventory_row.item_id)
}

/// The inner interactive widgets — the chip `×` (unassign), the `+ Tag` control, and the container
/// toggle — keep their own presses: `iced`'s `mouse_area` updates its content first and bails out the
/// moment a child captures the event, so a click that lands on the chip's close button unassigns the
/// tag instead of selecting the row.
fn select_wrap<'a>(row: Element<'a, Message>, item_id: i64) -> Element<'a, Message> {
  iced::widget::mouse_area(row)
    .on_press(Message::InventoryRowClicked(item_id))
    .on_right_press(Message::InventoryRowRightPressed(item_id))
    .on_enter(Message::InventoryRowHovered(Some(item_id)))
    .on_exit(Message::InventoryRowHovered(None))
    .into()
}

/// Prefix a row with a discrete warning-colored left edge marking it worth reprocessing. The bar is
/// its own `Length::Fill`-height element so it tracks the row's intrinsic height without bleeding the
/// warning color across the (transparent) row background.
fn reproc_edge<'a>(row: Element<'a, Message>) -> Element<'a, Message> {
  let bar = container(
    Space::new()
      .width(Length::Fixed(REPROC_EDGE_WIDTH))
      .height(Length::Fill),
  )
  .width(Length::Fixed(REPROC_EDGE_WIDTH))
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::status::WARNING)),
    ..container::Style::default()
  });

  Row::with_children(vec![bar.into(), row]).into()
}

fn row_icon<'a>(inventory_row: &'a InventoryRow) -> Element<'a, Message> {
  // An office has no type-icon PNG, so it takes the office glyph (matching its
  // division headers) rather than falling through to the image-missing fallback.
  if is_office(inventory_row) {
    return icon_tile(
      Icon::office()
        .color(color::text::secondary())
        .size(ICON_BOX * 0.55)
        .render(),
      ICON_BOX,
    );
  }
  let content: Element<'a, Message> = match &inventory_row.type_icon {
    IconResolution::Found(path) => image(image::Handle::from_path(path.clone()))
      .width(Length::Fill)
      .height(Length::Fill)
      .content_fit(iced::ContentFit::Contain)
      .into(),
    IconResolution::Missing => Icon::image_missing()
      .color(color::text::tertiary())
      .size(ICON_BOX * 0.45)
      .render(),
  };
  icon_tile(content, ICON_BOX)
}

fn custom_name(inventory_row: &InventoryRow) -> Option<&str> {
  inventory_row.name.as_deref().filter(|name| !name.is_empty())
}

fn name_cell<'a>(inventory_row: &'a InventoryRow, tags: Vec<&'a Tag>, hovered: bool) -> Element<'a, Message> {
  let custom_name = custom_name(inventory_row);

  let mut lines: Vec<Element<'a, Message>> = vec![
    text(custom_name.unwrap_or(&inventory_row.type_name).to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if custom_name.is_some() {
    lines.push(
      text(inventory_row.type_name.clone())
        .font(typography::body::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary()))
        .into(),
    );
  }
  lines.push(tag_strip(inventory_row.item_id, tags, hovered));

  let label = container(Column::with_children(lines).spacing(spacing::UNIT)).width(Length::Fill);

  let mut children: Vec<Element<'a, Message>> = vec![label.into()];
  if inventory_row.is_active_ship {
    children.push(active_ship_badge());
  }
  if inventory_row.worth_reprocessing() {
    children.push(reprocess_badge(inventory_row));
  }

  container(
    Row::with_children(children)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .into()
}

fn tag_strip<'a>(item_id: i64, tags: Vec<&'a Tag>, hovered: bool) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = tags
    .into_iter()
    .map(|tag| {
      Chip::new(tag.name().clone(), tag.color().as_deref().and_then(color::from_hex))
        .on_remove(Message::AssetTagModal(AddTagMessage::Unassign {
          entity_id: item_id,
          entity_type: ENTITY_TYPE_ASSET,
          tag_id: tag.id(),
        }))
        .view()
    })
    .collect();
  if hovered {
    children.push(add_tag_affordance(item_id));
  }

  Row::with_children(children)
    .spacing(spacing::UNIT)
    .align_y(Vertical::Center)
    .wrap()
    .into()
}

fn add_tag_affordance<'a>(item_id: i64) -> Element<'a, Message> {
  button(
    text(t!("assets.inventory.add_tag").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary())),
  )
  .padding([spacing::UNIT / 2.0, spacing::SPACE_2])
  .on_press(Message::OpenAssetTagModal {
    item_id,
  })
  .style(|_, _| button::Style {
    background: None,
    text_color: color::text::secondary(),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: 999.0.into(),
    },
    ..button::Style::default()
  })
  .into()
}

fn active_ship_badge<'a>() -> Element<'a, Message> {
  badge(t!("assets.inventory.active_badge"), Some(color::accent()))
}

fn reprocess_badge<'a>(inventory_row: &'a InventoryRow) -> Element<'a, Message> {
  reproc_tooltip(
    badge(t!("assets.inventory.reprocess_badge"), Some(color::status::WARNING)),
    inventory_row,
  )
}

fn category_cell<'a>(inventory_row: &'a InventoryRow) -> Element<'a, Message> {
  TableCell::new(category_label(&inventory_row.category)).view()
}

fn category_label(category: &str) -> String {
  let mut chars = category.chars();
  match chars.next() {
    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    None => t!("assets.inventory.category_other").into_owned(),
  }
}

fn row_prefix<'a>(inventory_row: &InventoryRow, depth: i64, expanded: bool) -> Element<'a, Message> {
  let indent = depth as f32 * INDENT_STEP;
  if inventory_row.is_container {
    Row::with_children(vec![
      Space::new().width(Length::Fixed(indent)).into(),
      container_toggle(inventory_row.item_id, expanded),
    ])
    .into()
  } else {
    Space::new().width(Length::Fixed(indent + TOGGLE_WIDTH)).into()
  }
}

fn container_toggle<'a>(item_id: i64, expanded: bool) -> Element<'a, Message> {
  let caret = if expanded {
    Icon::chevron_down()
  } else {
    Icon::chevron_right()
  };
  button(caret.size(12.0).color(color::text::secondary()).render())
    .padding(0)
    .width(Length::Fixed(TOGGLE_WIDTH))
    .on_press(Message::ContainerToggled(item_id))
    .style(|_, _| button::Style::default())
    .into()
}

fn division_header_row<'a>(header: &DivisionHeader) -> Element<'a, Message> {
  let indent = header.depth as f32 * INDENT_STEP;
  let caret = if header.open {
    Icon::chevron_down()
  } else {
    Icon::chevron_right()
  };
  let toggle = Message::DivisionToggled(header.office_item_id, header.division_key);

  let content = Row::with_children(vec![
    Space::new().width(Length::Fixed(indent)).into(),
    caret.size(12.0).color(color::text::secondary()).render(),
    icon_tile(
      Icon::office()
        .color(color::text::secondary())
        .size(ICON_BOX * 0.55)
        .render(),
      ICON_BOX,
    ),
    text(header.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  button(content)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: HEADER_SIDE_PADDING,
      bottom: spacing::SPACE_2,
      left: HEADER_SIDE_PADDING,
    })
    .on_press(toggle)
    .style(|_, status| {
      let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: hovered.then_some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.04))),
        border: Border {
          color: color::with_alpha(color::text::PRIMARY, 0.06),
          width: 1.0,
          radius: 0.0.into(),
        },
        ..button::Style::default()
      }
    })
    .into()
}

fn group_cell<'a>(inventory_row: &'a InventoryRow) -> Element<'a, Message> {
  TableCell::new(inventory_row.group_name.clone()).view()
}

fn value_cell<'a>(inventory_row: &'a InventoryRow) -> Element<'a, Message> {
  let primary = numeric_cell(fmt_isk(inventory_row.value), color::text::PRIMARY);
  if !inventory_row.worth_reprocessing() {
    return primary;
  }

  let secondary = container(
    text(format!("\u{21bb} {}", fmt_isk(inventory_row.reproc_value)))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::status::WARNING)),
  )
  .width(Length::Fill)
  .align_x(Horizontal::Right);

  Column::with_children(vec![primary, reproc_tooltip(secondary.into(), inventory_row)])
    .spacing(1.0)
    .width(Length::Fill)
    .into()
}

fn reproc_tooltip<'a>(content: Element<'a, Message>, inventory_row: &'a InventoryRow) -> Element<'a, Message> {
  let body = container(
    text(reproc_tooltip_text(inventory_row))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY)),
  )
  .max_width(280.0)
  .padding(spacing::SPACE_2_5)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  });

  tooltip(content, body, tooltip::Position::Top)
    .gap(spacing::SPACE_2)
    .into()
}

fn reproc_tooltip_text(inventory_row: &InventoryRow) -> String {
  let per_unit_reproc = if inventory_row.quantity > 0 {
    inventory_row.reproc_value / inventory_row.quantity as f64
  } else {
    inventory_row.reproc_value
  };
  let gain_pct = if inventory_row.value > 0.0 {
    ((inventory_row.reproc_value / inventory_row.value - 1.0) * 100.0).round() as i64
  } else {
    0
  };
  t!(
    "assets.inventory.reprocess_tooltip",
    reproc_price => fmt_isk(per_unit_reproc),
    gain_pct => gain_pct,
    sell_price => fmt_isk(inventory_row.unit_price),
  )
  .into_owned()
}

fn numeric_cell<'a>(value: String, text_color: iced::Color) -> Element<'a, Message> {
  TableCell::new(value)
    .font(typography::mono::REGULAR)
    .align(Horizontal::Right)
    .clip(false)
    .wrapping(text::Wrapping::Word)
    .color(text_color)
    .view()
}

fn text_cell<'a>(value: String, text_color: iced::Color) -> Element<'a, Message> {
  TableCell::new(value).color(text_color).view()
}

fn owner_cell<'a>(owner_id: i64, roster: &[RosterPilot], corporations: &[RosterCorp]) -> Element<'a, Message> {
  let pilot = roster.iter().find(|pilot| pilot.id == owner_id);
  let name = owner_label(owner_id, roster, corporations);
  let portrait = pilot.and_then(|pilot| pilot.portrait.path()).or_else(|| {
    corporations
      .iter()
      .find(|corp| corp.id == owner_id)
      .and_then(|corp| corp.logo.path())
  });

  let swatch = container(avatar(
    owner_id,
    &name,
    Length::Fixed(OWNER_PORTRAIT),
    OWNER_PORTRAIT,
    portrait,
  ))
  .width(Length::Fixed(OWNER_PORTRAIT))
  .height(Length::Fixed(OWNER_PORTRAIT))
  .clip(true)
  .style(|_| container::Style {
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  let label = TableCell::new(name).view();

  container(
    Row::with_children(vec![swatch.into(), label])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .clip(true)
  .into()
}

fn empty_state<'a>(message: &'static str) -> Element<'a, Message> {
  shared_empty_state(message).render()
}

fn empty_filtered_message() -> &'static str {
  static MESSAGE: OnceLock<String> = OnceLock::new();
  MESSAGE.get_or_init(|| t!("assets.inventory.empty_filtered").into_owned())
}

fn empty_scope_message() -> &'static str {
  static MESSAGE: OnceLock<String> = OnceLock::new();
  MESSAGE.get_or_init(|| t!("assets.inventory.empty_scope").into_owned())
}

pub(super) fn help_popover<'a>() -> Element<'a, Message> {
  let mut sections: Vec<Element<'a, Message>> = vec![help_section_label(&t!("assets.inventory.help_examples"))];
  for (&(query, _), note) in FILTER_HELP_EXAMPLES.iter().zip(filter_help_example_notes()) {
    sections.push(example_row(query, note));
  }
  sections.push(
    container(rule::horizontal())
      .padding(Padding {
        top: spacing::SPACE_2,
        right: spacing::SPACE_3_5,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_3_5,
      })
      .into(),
  );
  sections.push(help_section_label(&t!("assets.inventory.help_keys")));
  for (&(key, alias, _), desc) in FILTER_HELP_KEYS.iter().zip(filter_help_key_descriptions()) {
    sections.push(key_row(key, alias, desc));
  }

  container(Column::with_children(sections).spacing(2.0))
    .width(Length::Fixed(380.0))
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::UNIT,
      bottom: spacing::SPACE_3,
      left: spacing::UNIT,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn filter_help_example_notes() -> Vec<String> {
  vec![
    t!("assets.inventory.help_example_all_ships").into_owned(),
    t!("assets.inventory.help_example_in_forge").into_owned(),
    t!("assets.inventory.help_example_name_contains").into_owned(),
    t!("assets.inventory.help_example_ships_not_rifters").into_owned(),
    t!("assets.inventory.help_example_stacks_in_jita").into_owned(),
    t!("assets.inventory.help_example_my_modules").into_owned(),
    t!("assets.inventory.help_example_tagged_sell").into_owned(),
  ]
}

fn filter_help_key_descriptions() -> Vec<String> {
  vec![
    t!("assets.inventory.help_key_name").into_owned(),
    t!("assets.inventory.help_key_group").into_owned(),
    t!("assets.inventory.help_key_category").into_owned(),
    t!("assets.inventory.help_key_region").into_owned(),
    t!("assets.inventory.help_key_constellation").into_owned(),
    t!("assets.inventory.help_key_system").into_owned(),
    t!("assets.inventory.help_key_location").into_owned(),
    t!("assets.inventory.help_key_owner").into_owned(),
    t!("assets.inventory.help_key_tag").into_owned(),
    t!("assets.inventory.help_key_type").into_owned(),
  ]
}

fn help_section_label<'a>(label: &str) -> Element<'a, Message> {
  container(eyebrow(label, Some(color::text::tertiary())))
    .padding(Padding {
      top: spacing::UNIT,
      right: spacing::SPACE_3_5,
      bottom: spacing::UNIT + 2.0,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn example_row<'a>(query: &'static str, note: String) -> Element<'a, Message> {
  button(
    Row::with_children(vec![code_chip(query, true), note_text(note)])
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::UNIT + 2.0,
    right: spacing::SPACE_2_5,
    bottom: spacing::UNIT + 2.0,
    left: spacing::SPACE_2_5,
  })
  .on_press(Message::FilterExamplePicked(query))
  .style(|_, _| button::Style::default())
  .into()
}

fn key_row<'a>(key: &'static str, alias: &'static str, desc: String) -> Element<'a, Message> {
  let mut chips: Vec<Element<'a, Message>> = vec![code_chip(key, false)];
  if !alias.is_empty() {
    chips.push(code_chip(alias, false));
  }

  container(
    Row::with_children(vec![
      container(
        Row::with_children(chips)
          .spacing(spacing::UNIT)
          .align_y(Vertical::Center),
      )
      .width(Length::Fixed(140.0))
      .into(),
      note_text(desc),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::UNIT,
    right: spacing::SPACE_2_5,
    bottom: spacing::UNIT,
    left: spacing::SPACE_2_5,
  })
  .into()
}

fn code_chip<'a>(label: &'static str, accent: bool) -> Element<'a, Message> {
  let text_color = if accent {
    color::accent()
  } else {
    color::text::secondary()
  };
  let border_color = if accent {
    color::with_alpha(color::accent(), 0.35)
  } else {
    color::with_alpha(color::text::PRIMARY, 0.1)
  };
  container(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(text_color)),
  )
  .padding(Padding {
    top: 2.0,
    right: spacing::UNIT + 2.0,
    bottom: 2.0,
    left: spacing::UNIT + 2.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(if accent {
      color::with_alpha(color::accent(), 0.1)
    } else {
      color::with_alpha(color::text::PRIMARY, 0.04)
    })),
    border: Border {
      color: border_color,
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn note_text<'a>(note: String) -> Element<'a, Message> {
  container(
    text(note)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{features::assets::Scope, store::images};

  fn sample_row(item_id: i64, type_name: &str, category: &str, owner_id: i64, value: f64) -> InventoryRow {
    InventoryRow {
      category: category.to_owned(),
      container_id: None,
      depth: 0,
      group_name: format!("{type_name} Group"),
      is_active_ship: false,
      is_blueprint_copy: None,
      is_container: false,
      item_id,
      location_flag: "Hangar".to_owned(),
      location_id: 60_003_760,
      location_label: Some("Jita IV - Moon 4".to_owned()),
      name: None,
      owner_id,
      quantity: 10,
      reproc_value: 0.0,
      row_volume: 100.0,
      type_icon: IconResolution::Missing,
      type_id: 587,
      type_name: type_name.to_owned(),
      unit_price: value / 10.0,
      value,
    }
  }

  fn pilot(id: i64, name: &str) -> RosterPilot {
    RosterPilot {
      corp: "TST".to_owned(),
      granted_scopes: None,
      id,
      name: name.to_owned(),
      portrait: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      },
    }
  }

  mod custom_name {
    use pretty_assertions::assert_eq;

    use super::*;

    fn named_row(name: Option<&str>) -> InventoryRow {
      InventoryRow {
        name: name.map(str::to_owned),
        ..sample_row(1, "Giant Secure Container", "ship", 7, 1_000.0)
      }
    }

    #[test]
    fn it_renders_a_renamed_item_with_its_type_subtitle() {
      let row = named_row(Some("Loot Run"));
      let _el: Element<'_, Message> = name_cell(&row, Vec::new(), false);
    }

    #[test]
    fn it_renders_an_unnamed_item_as_the_type_name_alone() {
      let row = named_row(None);
      let _el: Element<'_, Message> = name_cell(&row, Vec::new(), false);
    }

    #[test]
    fn it_returns_none_when_the_name_is_absent() {
      assert_eq!(super::super::custom_name(&named_row(None)), None);
    }

    #[test]
    fn it_returns_the_custom_name_when_present() {
      assert_eq!(
        super::super::custom_name(&named_row(Some("Loot Run"))),
        Some("Loot Run")
      );
    }

    #[test]
    fn it_treats_an_empty_name_as_absent() {
      assert_eq!(super::super::custom_name(&named_row(Some(""))), None);
    }
  }

  mod tags {
    use super::*;
    use crate::store::{
      self,
      model::{TAG_SCOPE_ASSET, Tag},
      repo::infra,
    };

    async fn asset_tags() -> Vec<Tag> {
      let db = store::open_test().await.unwrap();
      infra::create_scoped(&db, "Keep", None, Some("#3FB8DB"), TAG_SCOPE_ASSET)
        .await
        .unwrap();
      infra::create_scoped(&db, "Sell", None, None, TAG_SCOPE_ASSET)
        .await
        .unwrap();
      infra::tag_all_scoped(&db, TAG_SCOPE_ASSET).await.unwrap()
    }

    #[tokio::test]
    async fn it_renders_a_chip_strip_with_the_rows_assigned_tags() {
      let tags = asset_tags().await;
      let assigned: Vec<&Tag> = tags.iter().collect();
      let row = sample_row(7001, "Rifter", "ship", 7, 5_000.0);

      let _el: Element<'_, Message> = name_cell(&row, assigned, true);
    }

    #[tokio::test]
    async fn it_renders_only_the_add_affordance_for_a_hovered_untagged_row() {
      let row = sample_row(7002, "Rifter", "ship", 7, 5_000.0);

      let _el: Element<'_, Message> = tag_strip(row.item_id, Vec::new(), true);
    }

    #[tokio::test]
    async fn the_chip_carries_an_unassign_remove_message() {
      let tags = asset_tags().await;
      let assigned: Vec<&Tag> = tags.iter().collect();
      let _el: Element<'_, Message> = tag_strip(7004, assigned, false);
    }

    #[test]
    fn an_unhovered_untagged_row_hides_the_add_affordance() {
      let _el: Element<'_, Message> = tag_strip(7005, Vec::new(), false);
    }

    #[test]
    fn the_add_affordance_opens_the_modal_keyed_on_the_item_id() {
      let _el: Element<'_, Message> = add_tag_affordance(7003);
    }
  }

  mod help_copy {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reproduces_the_filter_help_examples_verbatim() {
      assert_eq!(
        FILTER_HELP_EXAMPLES,
        [
          ("category:ship", "all ships"),
          ("region:\"The Forge\"", "in The Forge"),
          ("name:Tritanium", "name contains Tritanium"),
          ("category:ship -name:Rifter", "ships, not Rifters"),
          ("system:Jita type:stack", "stacks in Jita"),
          ("owner:me category:module", "my modules"),
          ("tag:Sell -tag:Junk", "tagged Sell, not Junk"),
        ]
      );
    }

    #[test]
    fn it_reproduces_the_filter_help_keys_verbatim() {
      assert_eq!(
        FILTER_HELP_KEYS,
        [
          ("name:", "n:", "type name (partial)"),
          ("group:", "g:", "group name (partial)"),
          ("category:", "cat:", "category key (exact)"),
          ("region:", "r:", "region name (exact)"),
          ("constellation:", "c:", "constellation (exact)"),
          ("system:", "s:", "system name (partial)"),
          ("location:", "loc:", "location name (partial)"),
          ("owner:", "", "character name or \"me\""),
          ("tag:", "", "asset tag name (exact)"),
          ("type:", "", "singleton \u{b7} bpc \u{b7} bpo \u{b7} stack"),
        ]
      );
    }
  }

  mod owner_cell {
    use super::*;

    fn corp(id: i64, name: &str) -> RosterCorp {
      RosterCorp {
        id,
        logo: images::ImageState::Stale {
          id,
          kind: images::ImageKind::CorporationLogo,
        },
        name: name.to_owned(),
        ticker: "TC".to_owned(),
      }
    }

    #[test]
    fn it_renders_a_corporation_owner() {
      let _el: Element<'_, Message> = super::super::owner_cell(2_000, &[], &[corp(2_000, "Test Corp")]);
    }

    #[test]
    fn it_renders_a_pilot_owner() {
      let _el: Element<'_, Message> = super::super::owner_cell(7, &[pilot(7, "Vex")], &[]);
    }

    #[test]
    fn it_renders_an_unknown_owner_without_a_portrait() {
      let _el: Element<'_, Message> = super::super::owner_cell(99, &[], &[]);
    }
  }

  mod owner_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_the_id_for_an_unknown_owner() {
      assert_eq!(owner_label(99, &[], &[]), "Owner 99");
    }

    #[test]
    fn it_resolves_a_corporation_owner_to_its_name() {
      let corp = RosterCorp {
        id: 2_000,
        logo: images::ImageState::Stale {
          id: 2_000,
          kind: images::ImageKind::CorporationLogo,
        },
        name: "Test Corp".to_owned(),
        ticker: "TC".to_owned(),
      };
      assert_eq!(owner_label(2_000, &[], std::slice::from_ref(&corp)), "Test Corp");
    }

    #[test]
    fn it_resolves_a_known_owner_to_its_name() {
      assert_eq!(owner_label(7, &[pilot(7, "Vex")], &[]), "Vex");
    }
  }

  mod render {
    use super::*;
    use crate::features::assets::State;

    fn container_row(item_id: i64, type_name: &str) -> InventoryRow {
      InventoryRow {
        is_container: true,
        ..sample_row(item_id, type_name, "ship", 7, 1_000.0)
      }
    }

    fn filtered_paged_state() -> State {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_for_test(
        Scope::Character(7),
        vec![pilot(7, "Vex")],
        vec![
          sample_row(1, "Rifter", "ship", 7, 1_000.0),
          sample_row(2, "Tritanium", "commodity", 7, 500.0),
        ],
        "category:ship".to_owned(),
      );
      state
    }

    #[test]
    fn it_renders_a_filtered_paged_inventory_body() {
      let state = filtered_paged_state();
      let _el: Element<'_, Message> = body(&state);
      let _bar: Element<'_, Message> = filter_bar(&state);
    }

    #[test]
    fn it_renders_the_empty_scope_state_with_no_inventory() {
      let state = State::new(crate::config::FeatureFlags::default());

      let _el: Element<'_, Message> = body(&state);
      assert!(!has_rows(&state));
    }

    #[test]
    fn it_renders_the_empty_filtered_state_when_a_search_matches_nothing() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_for_test(Scope::Character(7), vec![pilot(7, "Vex")], vec![], "nomatch".to_owned());

      let _el: Element<'_, Message> = body(&state);
      assert!(!has_rows(&state));
    }

    #[test]
    fn it_renders_both_flat_row_variants() {
      let state = State::new(crate::config::FeatureFlags::default());
      let row = sample_row(1, "Rifter", "ship", 7, 1_000.0);
      let item = FlatRow::Item {
        row: &row,
        depth: 0,
      };
      let division = FlatRow::Division(DivisionHeader {
        depth: 1,
        division_key: 1,
        name: "Ammo".to_owned(),
        office_item_id: 5,
        open: true,
      });

      let _item: Element<'_, Message> = flat_row_view(&state, &item);
      let _division: Element<'_, Message> = flat_row_view(&state, &division);
    }

    #[test]
    fn it_renders_an_expanded_container_with_its_lazy_loaded_children() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_for_test(
        Scope::Character(7),
        vec![pilot(7, "Vex")],
        vec![container_row(100, "Station Container")],
        String::new(),
      );
      let mut child = sample_row(101, "Tritanium", "commodity", 7, 50.0);
      child.depth = 1;
      state.set_inventory_children_for_test(100, vec![child]);

      let _el: Element<'_, Message> = body(&state);
    }

    #[test]
    fn it_renders_the_column_header_in_both_sort_directions() {
      let _ascending: Element<'_, Message> = column_header(SortColumn::Name, SortDirection::Ascending);
      let _descending: Element<'_, Message> = column_header(SortColumn::Value, SortDirection::Descending);
      let _unsortable: Element<'_, Message> = header_cell(
        "Location".to_owned(),
        None,
        false,
        SortColumn::Name,
        SortDirection::Ascending,
      );
    }

    #[test]
    fn it_renders_the_empty_states() {
      let state = State::new(crate::config::FeatureFlags::default());
      let _el: Element<'_, Message> = body(&state);
    }

    #[test]
    fn it_renders_the_help_popover() {
      let _el: Element<'_, Message> = help_popover();
    }
  }

  mod reprocess {
    use pretty_assertions::assert_eq;

    use super::*;

    fn worth_row() -> InventoryRow {
      InventoryRow {
        reproc_value: 2_500.0,
        ..sample_row(1, "Tritanium", "commodity", 7, 1_000.0)
      }
    }

    #[test]
    fn it_flags_a_stack_worth_more_reprocessed() {
      assert!(worth_row().worth_reprocessing());
    }

    #[test]
    fn it_does_not_flag_a_stack_worth_more_sold() {
      let row = sample_row(1, "Tritanium", "commodity", 7, 1_000.0);
      assert!(!row.worth_reprocessing());
    }

    #[test]
    fn it_renders_the_badge_and_secondary_value_for_a_worth_row() {
      let row = worth_row();
      let _name: Element<'_, Message> = name_cell(&row, Vec::new(), false);
      let _value: Element<'_, Message> = value_cell(&row);
      let _badge: Element<'_, Message> = reprocess_badge(&row);
    }

    #[test]
    fn it_renders_a_plain_value_cell_for_a_non_worth_row() {
      let row = sample_row(1, "Tritanium", "commodity", 7, 1_000.0);
      let _value: Element<'_, Message> = value_cell(&row);
      let _name: Element<'_, Message> = name_cell(&row, Vec::new(), false);
    }

    #[test]
    fn it_composes_the_tooltip_with_per_unit_reproc_sell_and_gain() {
      assert_eq!(
        reproc_tooltip_text(&worth_row()),
        "Reprocesses to 250/unit \u{2014} 150% above its 100/unit sell price. Worth refining rather than selling."
      );
    }

    #[test]
    fn it_handles_a_zero_quantity_stack_without_dividing_by_zero() {
      let mut row = worth_row();
      row.quantity = 0;
      let _text = reproc_tooltip_text(&row);
      let _value: Element<'_, Message> = value_cell(&row);
    }

    #[test]
    fn it_prefixes_a_left_edge_bar_for_a_worth_row() {
      let probe = text("row").into();
      let _edged: Element<'_, Message> = reproc_edge(probe);
    }

    #[test]
    fn it_renders_a_worth_row_with_the_left_edge_and_a_non_worth_row_without() {
      let worth = worth_row();
      let plain = sample_row(1, "Tritanium", "commodity", 7, 1_000.0);

      let _worth: Element<'_, Message> = table_row(&worth, 0, &[], &[], false, Vec::new(), false, false);
      let _plain: Element<'_, Message> = table_row(&plain, 0, &[], &[], false, Vec::new(), false, false);
    }
  }

  mod office_grouping {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::assets::{Scope, State};

    fn flagged(item_id: i64, flag: &str) -> InventoryRow {
      InventoryRow {
        location_flag: flag.to_owned(),
        ..sample_row(item_id, "Tritanium", "commodity", 2_000, 100.0)
      }
    }

    fn office(item_id: i64) -> InventoryRow {
      InventoryRow {
        is_container: true,
        type_id: 27,
        ..sample_row(item_id, "Office", "structure", 2_000, 0.0)
      }
    }

    #[test]
    fn it_detects_an_office_container_by_type_id() {
      let station_container = InventoryRow {
        is_container: true,
        type_id: 17_366,
        ..sample_row(2, "Station Container", "commodity", 2_000, 0.0)
      };

      assert!(is_office(&office(1)));
      assert!(!is_office(&station_container));
    }

    #[test]
    fn it_buckets_populated_divisions_in_order_and_skips_empty_ones() {
      let children = vec![
        flagged(10, "CorpSAG3"),
        flagged(11, "CorpDeliveries"),
        flagged(12, "CorpSAG1"),
        flagged(13, "CorpSAG1"),
      ];

      let (divisions, office_root) = group_office_children(&children);

      assert_eq!(
        divisions.iter().map(|(d, _)| *d).collect::<Vec<_>>(),
        [
          HangarDivision::Numbered(1),
          HangarDivision::Numbered(3),
          HangarDivision::Deliveries,
        ],
        "only populated divisions appear, hangar slots first then deliveries"
      );
      assert_eq!(
        divisions[0].1.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [12, 13],
        "items nest under their matching division"
      );
      assert!(office_root.is_empty());
    }

    #[test]
    fn it_routes_office_folder_items_to_the_office_root() {
      let children = vec![flagged(20, "OfficeFolder"), flagged(21, "CorpSAG2")];

      let (divisions, office_root) = group_office_children(&children);

      assert_eq!(divisions.len(), 1, "only the SAG2 division surfaces a header");
      assert_eq!(
        office_root.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [20],
        "OfficeFolder items land in the office root, not a division"
      );
    }

    #[test]
    fn it_prefers_a_custom_division_name() {
      assert_eq!(
        division_display_name(HangarDivision::Numbered(1), Some("Ammunition")),
        "Ammunition"
      );
    }

    #[test]
    fn it_falls_back_to_a_numbered_name_for_an_unnamed_division() {
      assert_eq!(division_display_name(HangarDivision::Numbered(4), None), "Division 4");
      assert_eq!(
        division_display_name(HangarDivision::Numbered(4), Some("")),
        "Division 4",
        "an empty custom name is treated as absent"
      );
    }

    #[test]
    fn it_labels_the_deliveries_bucket() {
      assert_eq!(
        division_display_name(HangarDivision::Deliveries, None),
        "Corp Deliveries"
      );
    }

    #[test]
    fn it_renders_division_headers_under_an_expanded_office() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_for_test(Scope::Corporation(2_000), Vec::new(), vec![office(500)], String::new());
      let child = InventoryRow {
        depth: 1,
        ..flagged(501, "CorpSAG1")
      };
      state.set_inventory_children_for_test(500, vec![child]);

      let _el: Element<'_, Message> = body(&state);
    }
  }

  mod row_icon {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::images::IconVariant;

    #[test]
    fn it_selects_the_bpc_variant_for_a_blueprint_copy() {
      let mut row = sample_row(1, "Rifter Blueprint", "blueprint", 7, 1_000.0);
      row.is_blueprint_copy = Some(true);

      assert_eq!(
        IconVariant::from_blueprint_copy(row.is_blueprint_copy),
        IconVariant::Bpc
      );
    }

    #[test]
    fn it_selects_the_bpo_variant_for_a_blueprint_original() {
      let mut row = sample_row(1, "Rifter Blueprint", "blueprint", 7, 1_000.0);
      row.is_blueprint_copy = Some(false);

      assert_eq!(
        IconVariant::from_blueprint_copy(row.is_blueprint_copy),
        IconVariant::Bpo
      );
    }

    #[test]
    fn it_selects_the_plain_icon_for_a_non_blueprint() {
      let row = sample_row(1, "Tritanium", "commodity", 7, 1_000.0);

      assert_eq!(
        IconVariant::from_blueprint_copy(row.is_blueprint_copy),
        IconVariant::Icon
      );
    }
  }
}
