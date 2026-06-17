use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, image, scrollable, text},
};

use super::{
  Category, HEADER_SIDE_PADDING, Message, RosterCorp, RosterPilot, State, fmt_count, fmt_isk, fmt_volume, owner_label,
};
use crate::{
  store::{
    images::IconResolution,
    model::asset_query::{InventoryRow, SortColumn, SortDirection},
  },
  ui::{
    components::{
      avatar::avatar,
      badge::badge,
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

/// Relative flex widths for the inventory columns, in column order:
/// Item, Group, Category, Qty, Volume, Unit, Value, Owner, Location.
/// Header and data rows share these so they stay aligned.
const COLUMN_PORTIONS: [u16; 9] = [4, 2, 1, 2, 2, 2, 2, 2, 2];

const FILTER_HELP_EXAMPLES: [(&str, &str); 6] = [
  ("category:ship", "all ships"),
  ("region:\"The Forge\"", "in The Forge"),
  ("name:Tritanium", "name contains Tritanium"),
  ("category:ship -name:Rifter", "ships, not Rifters"),
  ("system:Jita type:stack", "stacks in Jita"),
  ("owner:me category:module", "my modules"),
];

const FILTER_HELP_KEYS: [(&str, &str, &str); 9] = [
  ("name:", "n:", "type name (partial)"),
  ("group:", "g:", "group name (partial)"),
  ("category:", "cat:", "category key (exact)"),
  ("region:", "r:", "region name (exact)"),
  ("constellation:", "c:", "constellation (exact)"),
  ("system:", "s:", "system name (partial)"),
  ("location:", "loc:", "location name (partial)"),
  ("owner:", "", "character name or \"me\""),
  ("type:", "", "singleton \u{b7} bpc \u{b7} bpo \u{b7} stack"),
];

/// Nominal height of one inventory row, in pixels.
///
/// Rows are content-driven (one- or two-line name cells), so this is only an
/// estimate for [`VirtualList`] offset math; overscan absorbs the variance.
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
    color::accent::PLASMA
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
      Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.12)))
    } else if hovered {
      Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06)))
    } else {
      None
    };
    button::Style {
      background,
      border: Border {
        color: if selected {
          color::with_alpha(color::accent::PLASMA, 0.35)
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
    let suffix = if count == 1 { "match" } else { "matches" };
    right.push(
      text(format!("{} {suffix}", fmt_count(count)))
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

  TextInput::new(
    "Filter assets\u{2026}  try  name:Rifter  or  category:ship",
    state.search(),
    Message::SearchChanged,
  )
  .leading_icon(Icon::search())
  .on_submit(Message::SearchSubmitted)
  .trailing(trailing.into())
  .render()
}

fn save_filter_button<'a>(enabled: bool) -> Element<'a, Message> {
  let (label_color, glyph_color) = if enabled {
    (color::accent::PLASMA, color::accent::PLASMA)
  } else {
    (color::text::tertiary(), color::text::tertiary())
  };

  let content = Row::with_children(vec![
    text("\u{2605}")
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(glyph_color))
      .into(),
    text("Save")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(label_color))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let mut button = button(content).padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
  });
  if enabled {
    button = button.on_press(Message::SaveFilterOpened);
  }
  button
    .style(move |_, status| {
      if !enabled {
        return button::Style {
          background: Some(Background::Color(color::surface::SUNKEN)),
          border: Border {
            color: color::with_alpha(color::text::PRIMARY, 0.1),
            width: 1.0,
            radius: radius::CONTROL.into(),
          },
          ..button::Style::default()
        };
      }
      let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: Some(Background::Color(color::with_alpha(
          color::accent::PLASMA,
          if hovered { 0.18 } else { 0.1 },
        ))),
        border: Border {
          color: color::with_alpha(color::accent::PLASMA, if hovered { 0.6 } else { 0.4 }),
          width: 1.0,
          radius: radius::CONTROL.into(),
        },
        ..button::Style::default()
      }
    })
    .into()
}

fn help_toggle<'a>(open: bool) -> Element<'a, Message> {
  let color = if open {
    color::accent::PLASMA
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
      background: open.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
      border: Border {
        color: if open {
          color::with_alpha(color::accent::PLASMA, 0.45)
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
  let rows = state.inventory();
  let count = rows.len() as i64;
  let value: f64 = rows.iter().map(|r| r.value).sum();
  let volume: f64 = rows.iter().map(|r| r.row_volume).sum();

  container(
    Row::with_children(vec![
      summary_stat("Rows", fmt_count(count)),
      summary_stat("Value", fmt_isk(value)),
      summary_stat("Volume", fmt_volume(volume)),
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

fn summary_stat<'a>(label: &'a str, value: String) -> Element<'a, Message> {
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
    return empty_state(if state.search().trim().is_empty() {
      "No assets in this scope."
    } else {
      "No assets match the current filters."
    });
  }

  // Flatten the inventory (with any expanded containers' children spliced inline)
  // into a single flat index space, then window over it so only the viewport's
  // rows are materialized regardless of how many pages have loaded.
  let flat = flatten_rows(state);
  let offset = state.inventory_scroll_offset();

  virtual_list::responsive_window(move |viewport_height| {
    let config = VirtualListConfig::new(flat.len(), ESTIMATED_ROW_HEIGHT)
      .viewport_height(viewport_height)
      .scroll_offset(offset);
    let list = VirtualList::new(config, |index| {
      let inventory_row = flat[index];
      let expanded = state.container_is_open(inventory_row.item_id);
      table_row(inventory_row, state.roster(), state.corporations(), expanded)
    })
    .view();

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

pub(super) fn has_rows(state: &State) -> bool {
  !state.inventory().is_empty()
}

/// Splice the inventory and any open containers' loaded children into a single
/// depth-first flat list, the index space [`VirtualList`] windows over.
fn flatten_rows(state: &State) -> Vec<&InventoryRow> {
  let mut out = Vec::with_capacity(state.inventory().len());
  for inventory_row in state.inventory() {
    push_row(state, &mut out, inventory_row);
  }
  out
}

fn push_row<'a>(state: &'a State, out: &mut Vec<&'a InventoryRow>, inventory_row: &'a InventoryRow) {
  out.push(inventory_row);

  if inventory_row.is_container
    && state.container_is_open(inventory_row.item_id)
    && let Some(children) = state.container_children_of(inventory_row.item_id)
  {
    for child in children {
      push_row(state, out, child);
    }
  }
}

fn column_header<'a>(sort: SortColumn, dir: SortDirection) -> Element<'a, Message> {
  let columns: [(&str, Option<SortColumn>, bool); 8] = [
    ("Item", Some(SortColumn::Name), false),
    ("Group", Some(SortColumn::Group), false),
    ("Category", Some(SortColumn::Category), false),
    ("Qty", Some(SortColumn::Quantity), true),
    ("Volume", Some(SortColumn::Volume), true),
    ("Unit", Some(SortColumn::UnitPrice), true),
    ("Value", Some(SortColumn::Value), true),
    ("Owner", Some(SortColumn::Owner), false),
  ];

  let mut cells: Vec<Element<'a, Message>> =
    vec![Space::new().width(Length::Fixed(TOGGLE_WIDTH)).into(), header_spacer()];
  for ((label, column, right), &portion) in columns.into_iter().zip(COLUMN_PORTIONS.iter()) {
    cells.push(portioned(header_cell(label, column, right, sort, dir), portion));
  }
  cells.push(portioned(plain_header("Location"), COLUMN_PORTIONS[8]));

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

fn plain_header<'a>(label: &'a str) -> Element<'a, Message> {
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
  label: &'a str,
  column: Option<SortColumn>,
  right: bool,
  sort: SortColumn,
  dir: SortDirection,
) -> Element<'a, Message> {
  let active = column == Some(sort);
  let label_color = if active {
    color::accent::PLASMA
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
      SortDirection::Ascending => "\u{25b2}",
      SortDirection::Descending => "\u{25bc}",
    };
    content.push(
      text(caret)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::accent::PLASMA))
        .into(),
    );
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

fn table_row<'a>(
  inventory_row: &'a InventoryRow,
  roster: &[RosterPilot],
  corporations: &[RosterCorp],
  expanded: bool,
) -> Element<'a, Message> {
  let cells: Vec<Element<'a, Message>> = vec![
    row_prefix(inventory_row, expanded),
    row_icon(inventory_row),
    portioned(name_cell(inventory_row), COLUMN_PORTIONS[0]),
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
    portioned(
      numeric_cell(fmt_isk(inventory_row.value), color::text::PRIMARY),
      COLUMN_PORTIONS[6],
    ),
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

  container(
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
  .style(|_| container::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.06),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn row_icon<'a>(inventory_row: &'a InventoryRow) -> Element<'a, Message> {
  let content: Element<'a, Message> = match &inventory_row.type_icon {
    IconResolution::Found(path) => image(image::Handle::from_path(path.clone()))
      .width(Length::Fill)
      .height(Length::Fill)
      .content_fit(iced::ContentFit::Contain)
      .into(),
    IconResolution::Missing => Space::new().into(),
  };
  icon_tile(content, ICON_BOX)
}

fn custom_name(inventory_row: &InventoryRow) -> Option<&str> {
  inventory_row.name.as_deref().filter(|name| !name.is_empty())
}

fn name_cell<'a>(inventory_row: &'a InventoryRow) -> Element<'a, Message> {
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

  let label = container(Column::with_children(lines)).width(Length::Fill);

  let mut children: Vec<Element<'a, Message>> = vec![label.into()];
  if inventory_row.is_active_ship {
    children.push(active_ship_badge());
  }

  container(
    Row::with_children(children)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .into()
}

fn active_ship_badge<'a>() -> Element<'a, Message> {
  badge("ACTIVE", Some(color::accent::PLASMA))
}

fn category_cell<'a>(inventory_row: &'a InventoryRow) -> Element<'a, Message> {
  TableCell::new(category_label(&inventory_row.category)).view()
}

fn category_label(category: &str) -> String {
  let mut chars = category.chars();
  match chars.next() {
    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    None => "Other".to_owned(),
  }
}

fn row_prefix<'a>(inventory_row: &InventoryRow, expanded: bool) -> Element<'a, Message> {
  let indent = inventory_row.depth as f32 * INDENT_STEP;
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
  button(
    text(if expanded { "\u{25bc}" } else { "\u{25b6}" })
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(0)
  .width(Length::Fixed(TOGGLE_WIDTH))
  .on_press(Message::ContainerToggled(item_id))
  .style(|_, _| button::Style::default())
  .into()
}

fn group_cell<'a>(inventory_row: &'a InventoryRow) -> Element<'a, Message> {
  TableCell::new(inventory_row.group_name.clone()).view()
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

fn empty_state<'a>(message: &'a str) -> Element<'a, Message> {
  shared_empty_state(message).render()
}

pub(super) fn help_popover<'a>() -> Element<'a, Message> {
  let mut sections: Vec<Element<'a, Message>> = vec![help_section_label("Examples")];
  for &(query, note) in &FILTER_HELP_EXAMPLES {
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
  sections.push(help_section_label("Available keys"));
  for &(key, alias, desc) in &FILTER_HELP_KEYS {
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

fn help_section_label<'a>(label: &'a str) -> Element<'a, Message> {
  container(eyebrow(label, Some(color::text::tertiary())))
    .padding(Padding {
      top: spacing::UNIT,
      right: spacing::SPACE_3_5,
      bottom: spacing::UNIT + 2.0,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn example_row<'a>(query: &'static str, note: &'static str) -> Element<'a, Message> {
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

fn key_row<'a>(key: &'static str, alias: &'static str, desc: &'static str) -> Element<'a, Message> {
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
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  let border_color = if accent {
    color::with_alpha(color::accent::PLASMA, 0.35)
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
      color::with_alpha(color::accent::PLASMA, 0.1)
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

fn note_text<'a>(note: &'static str) -> Element<'a, Message> {
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
      location_id: 60_003_760,
      location_label: Some("Jita IV - Moon 4".to_owned()),
      name: None,
      owner_id,
      quantity: 10,
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
          ("type:", "", "singleton \u{b7} bpc \u{b7} bpo \u{b7} stack"),
        ]
      );
    }
  }

  mod owner_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_a_known_owner_to_its_name() {
      assert_eq!(owner_label(7, &[pilot(7, "Vex")], &[]), "Vex");
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
    fn it_falls_back_to_the_id_for_an_unknown_owner() {
      assert_eq!(owner_label(99, &[], &[]), "Owner 99");
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
    fn it_renders_a_pilot_owner() {
      let _el: Element<'_, Message> = super::super::owner_cell(7, &[pilot(7, "Vex")], &[]);
    }

    #[test]
    fn it_renders_a_corporation_owner() {
      let _el: Element<'_, Message> = super::super::owner_cell(2_000, &[], &[corp(2_000, "Test Corp")]);
    }

    #[test]
    fn it_renders_an_unknown_owner_without_a_portrait() {
      let _el: Element<'_, Message> = super::super::owner_cell(99, &[], &[]);
    }
  }

  mod row_icon {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::images::IconVariant;

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
    fn it_selects_the_bpc_variant_for_a_blueprint_copy() {
      let mut row = sample_row(1, "Rifter Blueprint", "blueprint", 7, 1_000.0);
      row.is_blueprint_copy = Some(true);

      assert_eq!(
        IconVariant::from_blueprint_copy(row.is_blueprint_copy),
        IconVariant::Bpc
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
      let mut state = State::new();
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
    fn it_renders_an_expanded_container_with_its_lazy_loaded_children() {
      let mut state = State::new();
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
    fn it_renders_the_help_popover() {
      let _el: Element<'_, Message> = help_popover();
    }

    #[test]
    fn it_renders_the_empty_states() {
      let state = State::new();
      let _el: Element<'_, Message> = body(&state);
    }

    #[test]
    fn it_renders_the_column_header_in_both_sort_directions() {
      let _ascending: Element<'_, Message> = column_header(SortColumn::Name, SortDirection::Ascending);
      let _descending: Element<'_, Message> = column_header(SortColumn::Value, SortDirection::Descending);
      let _unsortable: Element<'_, Message> =
        header_cell("Location", None, false, SortColumn::Name, SortDirection::Ascending);
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
    fn it_returns_the_custom_name_when_present() {
      assert_eq!(
        super::super::custom_name(&named_row(Some("Loot Run"))),
        Some("Loot Run")
      );
    }

    #[test]
    fn it_returns_none_when_the_name_is_absent() {
      assert_eq!(super::super::custom_name(&named_row(None)), None);
    }

    #[test]
    fn it_treats_an_empty_name_as_absent() {
      assert_eq!(super::super::custom_name(&named_row(Some(""))), None);
    }

    #[test]
    fn it_renders_a_renamed_item_with_its_type_subtitle() {
      let row = named_row(Some("Loot Run"));
      let _el: Element<'_, Message> = name_cell(&row);
    }

    #[test]
    fn it_renders_an_unnamed_item_as_the_type_name_alone() {
      let row = named_row(None);
      let _el: Element<'_, Message> = name_cell(&row);
    }
  }
}
