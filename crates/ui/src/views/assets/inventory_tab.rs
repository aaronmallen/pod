//! Sortable asset inventory table with search and category filter.

pub mod help_pop_over;

use std::collections::{HashMap, HashSet};

use iced::{
  Background, Border, Color, ContentFit, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, image, row, scrollable, text},
};
use tracing;

use super::{AssetRecord, Category, SortCol, State, asset_value, asset_volume, fmt_qty, fmt_vol};
use crate::{
  components::{self, PillFilter, SearchBox},
  format,
  style::{
    color,
    typography::{body, mono},
  },
};

/// Messages produced by the inventory tab.
#[derive(Clone, Debug)]
pub enum Message {
  CategoryChanged(Category),
  HelpPopOver(help_pop_over::Message),
  HelpToggle,
  ScrollUpdate(f32),
  SearchChanged(String),
  SortChanged(SortCol),
  ToggleContainer(i64),
}

fn empty_state(msg: &str) -> Element<'_, Message> {
  container(
    text(msg)
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(48.0)
  .width(Length::Fill)
  .center_x(Length::Fill)
  .height(Length::Fill)
  .center_y(Length::Fill)
  .into()
}

fn help_button(open: bool) -> Element<'static, Message> {
  let icon_color = if open {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };
  components::Button::ghost(
    container(
      components::Icon::help()
        .size(13.0)
        .color(icon_color)
        .render::<Message>(),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill),
  )
  .width(26.0)
  .height(24.0)
  .padding(0)
  .on_press(Message::HelpToggle)
  .into()
}

fn search_box<'a>(query: &'a str, help_visible: bool) -> Element<'a, Message> {
  SearchBox::new(
    "Filter assets…  try name:Rifter or category:ship",
    query,
    Message::SearchChanged,
  )
  .width(Length::Fill)
  .height(36.0)
  .font_size(12.0)
  .icon_size(13.0)
  .horizontal_padding(8.0)
  .right_element(help_button(help_visible))
  .render()
}

fn category_row<'a>(active_cat: &'a Category) -> Element<'a, Message> {
  let options: Vec<(&str, Category)> = Category::all().iter().map(|c| (c.label(), c.clone())).collect();
  PillFilter::new(options, active_cat, Message::CategoryChanged).render()
}

fn stats_pill<'a>(state: &'a State) -> Element<'a, Message> {
  let total_value = state.visible_assets().map(asset_value).sum::<f64>();
  let total_volume = state.visible_assets().map(asset_volume).sum::<f64>();
  let total_rows = state.visible_assets().count();
  container(
    row([
      stat_label("Rows", total_rows.to_string()),
      Space::new().width(18.0).into(),
      stat_label("Value", format::fmt_isk(total_value)),
      Space::new().width(18.0).into(),
      stat_label("Volume", format::fmt_vol(total_volume)),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 14.0,
    right: 14.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn stat_label<'a>(label: &'static str, value: String) -> Element<'a, Message> {
  column([
    text(label)
      .font(mono::REGULAR)
      .size(8.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    text(value)
      .font(mono::MEDIUM)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(1.0)
  .into()
}

fn filter_bar<'a>(state: &'a State) -> Element<'a, Message> {
  container(
    column([
      search_box(&state.search_query, state.help_pop_over.visible),
      row([
        category_row(&state.category),
        Space::new().width(Length::Fill).into(),
        stats_pill(state),
      ])
      .align_y(iced::alignment::Vertical::Center)
      .into(),
    ])
    .spacing(8.0),
  )
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 20.0,
    right: 20.0,
  })
  .width(Length::Fill)
  .style(|_| container::Style {
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn col_hdr_label_text(label: &'static str, is_active: bool, asc: bool) -> String {
  if is_active {
    format!("{} {}", label, if asc { "▲" } else { "▼" })
  } else {
    label.to_string()
  }
}

fn table_col_hdr<'a>(
  label: &'static str,
  col: SortCol,
  active: &SortCol,
  asc: bool,
  width: f32,
  fill: bool,
) -> Element<'a, Message> {
  let is_active = *active == col;
  let label_text = col_hdr_label_text(label, is_active, asc);

  let btn = button(
    text(label_text)
      .font(mono::REGULAR)
      .size(9.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(if is_active {
          color::accent::PLASMA
        } else {
          color::text::SECONDARY
        }),
      }),
  )
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: 0.0,
    right: 8.0,
  })
  .on_press(Message::SortChanged(col))
  .style(|_, _| button::Style {
    background: None,
    border: Border::default(),
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  });

  if fill {
    btn.width(Length::Fill).into()
  } else {
    btn.width(Length::Fixed(width)).into()
  }
}

fn table_header<'a>(active_col: &'a SortCol, asc: bool) -> Element<'a, Message> {
  container(
    row([
      table_col_hdr("Type", SortCol::Category, active_col, asc, 36.0, false),
      table_col_hdr("Name", SortCol::Name, active_col, asc, 0.0, true),
      table_col_hdr("Qty", SortCol::Qty, active_col, asc, 70.0, false),
      table_col_hdr("Unit", SortCol::UnitValue, active_col, asc, 110.0, false),
      table_col_hdr("Value", SortCol::TotalValue, active_col, asc, 120.0, false),
      table_col_hdr("Volume", SortCol::Volume, active_col, asc, 90.0, false),
      table_col_hdr("Location", SortCol::Location, active_col, asc, 200.0, false),
      table_col_hdr("Owner", SortCol::Owner, active_col, asc, 120.0, false),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: 16.0,
    right: 16.0,
  })
  .height(30.0)
  .align_y(iced::alignment::Vertical::Center)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn asset_icon_cell<'a>(
  type_id: i32,
  variant: &str,
  icons: &'a HashMap<(i32, String), image::Handle>,
) -> Element<'a, Message> {
  const SIZE: f32 = 24.0;
  let handle = icons
    .get(&(type_id, variant.to_string()))
    .or_else(|| icons.get(&(type_id, "icon".to_string())));
  if let Some(handle) = handle {
    container(
      image(handle.clone())
        .width(SIZE)
        .height(SIZE)
        .content_fit(ContentFit::Cover),
    )
    .width(SIZE)
    .height(SIZE)
    .style(|_| container::Style {
      border: Border {
        radius: 4.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .clip(true)
    .into()
  } else {
    container(Space::new().width(SIZE).height(SIZE))
      .width(SIZE)
      .height(SIZE)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        border: Border {
          radius: 4.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  }
}

fn asset_name_col(a: &AssetRecord) -> Element<'_, Message> {
  column([
    text(a.type_name.clone())
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(a.group_name.clone())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .width(Length::Fill)
  .into()
}

fn asset_mono_cell(value: String, size: f32, color: Color, width: f32) -> Element<'static, Message> {
  text(value)
    .font(mono::REGULAR)
    .size(size)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(color),
    })
    .width(Length::Fixed(width))
    .into()
}

fn asset_grotesk_cell(value: String, size: f32, color: Color, width: f32) -> Element<'static, Message> {
  text(value)
    .font(body::REGULAR)
    .size(size)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(color),
    })
    .width(Length::Fixed(width))
    .into()
}

fn container_toggle(item_id: i64, expanded: bool) -> Element<'static, Message> {
  tracing::debug!("inventory_tab: rendering toggle button for container {item_id}, expanded={expanded}");
  button(
    text(if expanded { "▼" } else { "▶" })
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .on_press(Message::ToggleContainer(item_id))
  .padding(Padding::ZERO)
  .style(|_, _| button::Style {
    background: None,
    border: Border::default(),
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  })
  .width(Length::Fixed(16.0))
  .into()
}

fn asset_row_prefix(a: &AssetRecord, expanded: bool) -> Element<'_, Message> {
  let indent = a.depth as f32 * 16.0;
  if a.is_container {
    row([Space::new().width(indent).into(), container_toggle(a.item_id, expanded)]).into()
  } else {
    Space::new().width(indent + 16.0).into()
  }
}

fn asset_value_cell(val: f64) -> Element<'static, Message> {
  text(format::fmt_isk(val))
    .font(mono::MEDIUM)
    .size(12.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::accent::PLASMA),
    })
    .width(Length::Fixed(120.0))
    .into()
}

fn asset_location_cell(a: &AssetRecord) -> Element<'_, Message> {
  let loc = if a.container_path.is_empty() {
    a.location_name.clone()
  } else {
    a.container_path.clone()
  };
  asset_grotesk_cell(loc, 11.0, color::text::SECONDARY, 200.0)
}

fn asset_table_row<'a>(
  a: &'a AssetRecord,
  char_name: &'a str,
  icons: &'a HashMap<(i32, String), image::Handle>,
  expanded: bool,
) -> Element<'a, Message> {
  let val = asset_value(a);
  let vol = asset_volume(a);
  let prefix = asset_row_prefix(a, expanded);

  container(
    row([
      prefix,
      asset_icon_cell(a.type_id, &a.icon_variant, icons),
      Space::new().width(8.0).into(),
      asset_name_col(a),
      asset_mono_cell(fmt_qty(a.quantity as u64), 12.0, color::text::PRIMARY, 70.0),
      asset_mono_cell(format::fmt_isk(0.0), 11.0, color::text::SECONDARY, 110.0),
      asset_value_cell(val),
      asset_mono_cell(fmt_vol(vol), 11.0, color::text::SECONDARY, 90.0),
      asset_location_cell(a),
      asset_grotesk_cell(char_name.to_string(), 11.0, color::text::SECONDARY, 120.0),
    ])
    .align_y(iced::alignment::Vertical::Center)
    .padding(Padding {
      top: 8.0,
      bottom: 8.0,
      left: 8.0,
      right: 16.0,
    }),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn build_tree_rows<'a>(
  sorted: Vec<&'a AssetRecord>,
  all_assets: &'a [AssetRecord],
  expanded: &HashSet<i64>,
  loaded_containers: &'a std::collections::HashMap<i64, Vec<AssetRecord>>,
) -> Vec<&'a AssetRecord> {
  let mut children_map: HashMap<i64, Vec<&'a AssetRecord>> = HashMap::new();
  // First, add children from all_assets (for nested containers that were loaded inline)
  for a in all_assets {
    if a.container_id != 0 {
      children_map.entry(a.location_id).or_default().push(a);
    }
  }
  // Then, add lazily-loaded container contents
  for (container_item_id, assets) in loaded_containers {
    for a in assets {
      children_map.entry(*container_item_id).or_default().push(a);
    }
  }
  for children in children_map.values_mut() {
    children.sort_by(|a, b| a.type_name.cmp(&b.type_name));
  }

  let visible_ids: HashSet<i64> = sorted.iter().map(|a| a.item_id).collect();
  let roots: Vec<&AssetRecord> = sorted
    .into_iter()
    .filter(|a| a.container_id == 0 || !visible_ids.contains(&a.location_id))
    .collect();

  let mut result = Vec::new();
  for root in roots {
    append_subtree(root, &children_map, expanded, &mut result);
  }
  result
}

fn append_subtree<'a>(
  item: &'a AssetRecord,
  children_map: &HashMap<i64, Vec<&'a AssetRecord>>,
  expanded: &HashSet<i64>,
  out: &mut Vec<&'a AssetRecord>,
) {
  out.push(item);
  if item.is_container
    && expanded.contains(&item.item_id)
    && let Some(children) = children_map.get(&item.item_id)
  {
    for child in children {
      append_subtree(child, children_map, expanded, out);
    }
  }
}

/// Builder for the inventory tab (filter bar + sortable asset table).
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new inventory tab for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the inventory tab into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let filter_bar_el = filter_bar(state);

    let sorted = state.sorted_assets();
    let table: Element<'_, Message> = if sorted.is_empty() {
      empty_state("No assets match the current filters.")
    } else {
      let header_row = table_header(&state.sort_col, state.sort_asc);
      let all_assets = state.all_assets();
      let visible = state.visible_count.min(sorted.len());
      let page: Vec<&AssetRecord> = sorted[..visible].to_vec();
      let tree_rows = build_tree_rows(page, all_assets, &state.expanded_containers, &state.loaded_container_assets);
      let data_rows: Vec<Element<'_, Message>> = tree_rows
        .into_iter()
        .map(|a| {
          let owner_name = state
            .characters
            .iter()
            .find(|c| *c.id() == a.character_id)
            .map(|c| c.name().as_str())
            .or_else(|| {
              state
                .corporations
                .iter()
                .find(|c| *c.id() == a.character_id)
                .map(|c| c.name().as_str())
            })
            .unwrap_or("Unknown");
          let expanded = state.expanded_containers.contains(&a.item_id);
          asset_table_row(a, owner_name, &state.item_icons, expanded)
        })
        .collect();
      let data = scrollable(column(data_rows).width(Length::Fill))
        .height(Length::Fill)
        .on_scroll(|vp| Message::ScrollUpdate(vp.relative_offset().y));
      column([header_row, data.into()])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    };

    column([filter_bar_el, table])
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
  }
}
