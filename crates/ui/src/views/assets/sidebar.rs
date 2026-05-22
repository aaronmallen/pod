//! Location tree sidebar — scrollable container with system/location rows.

pub mod container_row;
pub mod location_row;
pub mod system_row;

use std::collections::{BTreeMap, BTreeSet};

pub use container_row::Component as ContainerRow;
use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{button, column, container, row, scrollable, text},
};
pub use location_row::Component as LocationRow;
pub use system_row::Component as SystemRow;

use super::{Message, State, asset_value};
use crate::{
  components::section_label,
  style::{
    button as btn_style, color,
    typography::{body, mono},
  },
};

fn all_assets_row_label(active: bool) -> Element<'static, Message> {
  text("All assets")
    .font(body::REGULAR)
    .size(12.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(if active {
        color::text::PRIMARY
      } else {
        iced::Color::from_rgba(0.957, 0.949, 0.925, 0.78)
      }),
    })
    .width(Length::Fill)
    .into()
}

fn all_assets_row<'a>(active: bool) -> Element<'a, Message> {
  let glyph_color = color::text::SECONDARY;
  let msg = Message::LocationSelected(None);
  let row_children: Vec<Element<'_, Message>> = vec![
    text("∑")
      .font(mono::REGULAR)
      .size(11.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(glyph_color),
      })
      .into(),
    all_assets_row_label(active),
  ];

  button(
    row(row_children)
      .spacing(6.0)
      .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 5.0,
    bottom: 5.0,
    left: 12.0,
    right: 12.0,
  })
  .width(Length::Fill)
  .on_press(msg)
  .style(move |_, status| btn_style::list_item_active(active, status))
  .into()
}

fn locations_label() -> Element<'static, Message> {
  container(section_label("Locations"))
    .padding(Padding {
      top: 16.0,
      bottom: 5.0,
      left: 18.0,
      right: 14.0,
    })
    .width(Length::Fill)
    .into()
}

fn collect_containers<'a>(
  source: &'a [super::AssetRecord],
  loc_name: &str,
  char_id: Option<i64>,
) -> BTreeMap<i64, &'a str> {
  source
    .iter()
    .filter(|a| {
      if let Some(id) = char_id
        && a.character_id != id
      {
        return false;
      }
      a.container_id != 0 && a.location_name.as_str() == loc_name
    })
    .fold(BTreeMap::new(), |mut map, a| {
      if let Some(path) = a.container_path.rsplit(" · ").next() {
        map.entry(a.container_id).or_insert(path);
      }
      map
    })
}

fn push_location_rows<'a>(
  items: &mut Vec<Element<'a, Message>>,
  source: &'a [super::AssetRecord],
  loc_name: &'a str,
  char_id: Option<i64>,
  selected_loc: Option<&str>,
) {
  let loc_filter = format!("location:{}", loc_name);
  let loc_active = selected_loc == Some(loc_filter.as_str());
  let loc_value: f64 = source
    .iter()
    .filter(|a| a.location_name.as_str() == loc_name)
    .map(asset_value)
    .sum();

  items.push(LocationRow::new(loc_name, loc_filter, loc_active, loc_value).render());

  let containers = collect_containers(source, loc_name, char_id);
  for (cid, cname) in &containers {
    let cfilter = format!("container:{}", cid);
    let cactive = selected_loc == Some(cfilter.as_str());
    let cvalue: f64 = source.iter().filter(|a| a.container_id == *cid).map(asset_value).sum();
    items.push(ContainerRow::new(cname, cfilter, cactive, cvalue).render());
  }
}

fn push_system_rows<'a>(
  items: &mut Vec<Element<'a, Message>>,
  source: &'a [super::AssetRecord],
  sys_name: &'a str,
  char_id: Option<i64>,
  selected_loc: Option<&str>,
) {
  let sys_filter = format!("system:{}", sys_name);
  let sys_active = selected_loc == Some(sys_filter.as_str());
  let sys_value: f64 = source
    .iter()
    .filter(|a| a.system_name.as_str() == sys_name)
    .map(asset_value)
    .sum();

  items.push(SystemRow::new(sys_name, sys_filter, sys_active, sys_value).render());

  let locs: BTreeSet<&str> = source
    .iter()
    .filter(|a| {
      if let Some(id) = char_id
        && a.character_id != id
      {
        return false;
      }
      a.system_name.as_str() == sys_name
    })
    .map(|a| a.location_name.as_str())
    .collect();

  for loc_name in &locs {
    push_location_rows(items, source, loc_name, char_id, selected_loc);
  }
}

fn is_system_asset(a: &super::AssetRecord, char_id: Option<i64>) -> bool {
  if let Some(id) = char_id
    && a.character_id != id
  {
    return false;
  }
  !a.system_name.is_empty()
}

fn is_structure_loc_asset(a: &super::AssetRecord, char_id: Option<i64>) -> bool {
  if let Some(id) = char_id
    && a.character_id != id
  {
    return false;
  }
  a.system_name.is_empty() && !a.location_name.is_empty() && a.container_id == 0
}

fn collect_systems<'a>(source: &'a [super::AssetRecord], char_id: Option<i64>) -> BTreeSet<&'a str> {
  source
    .iter()
    .filter(|a| is_system_asset(a, char_id))
    .map(|a| a.system_name.as_str())
    .collect()
}

fn collect_structure_locs<'a>(source: &'a [super::AssetRecord], char_id: Option<i64>) -> BTreeSet<&'a str> {
  source
    .iter()
    .filter(|a| is_structure_loc_asset(a, char_id))
    .map(|a| a.location_name.as_str())
    .collect()
}

fn build_sidebar_items<'a>(state: &'a State) -> Vec<Element<'a, Message>> {
  let mut items: Vec<Element<'_, Message>> = Vec::new();
  items.push(locations_label());
  items.push(all_assets_row(state.selected_loc.is_none()));

  let source: &[super::AssetRecord] = if state.selected_corporation().is_some() {
    &state.corp_assets
  } else {
    &state.assets
  };
  let char_id = state.selected_character();
  let selected_loc = state.selected_loc.as_deref();

  for sys_name in &collect_systems(source, char_id) {
    push_system_rows(&mut items, source, sys_name, char_id, selected_loc);
  }

  for loc_name in &collect_structure_locs(source, char_id) {
    push_location_rows(&mut items, source, loc_name, char_id, selected_loc);
  }

  items
}

/// Builder for the assets location tree sidebar.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new sidebar for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the sidebar into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let items = build_sidebar_items(self.state);
    let content = scrollable(column(items).width(Length::Fill)).height(Length::Fill);
    container(content)
      .width(Length::Fixed(232.0))
      .height(Length::Fill)
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
}
