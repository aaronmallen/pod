//! Location tree sidebar — scrollable container with the 5-level region/constellation/system/location/container tree.

pub mod all_assets_row;
pub mod constellation_row;
pub mod container_row;
pub mod location_row;
pub mod locations_label;
pub mod region_row;
pub mod system_row;

use std::collections::{BTreeMap, BTreeSet};

pub use all_assets_row::Component as AllAssetsRow;
pub use constellation_row::Component as ConstellationRow;
pub use container_row::Component as ContainerRow;
use iced::{
  Background, Border, Element, Length,
  widget::{column, container, scrollable},
};
pub use location_row::Component as LocationRow;
pub use locations_label::Component as LocationsLabel;
pub use region_row::Component as RegionRow;
pub use system_row::Component as SystemRow;

use super::{Message, State, asset_value};
use crate::{asset_filter_query::AssetFilterQuery, style::color};

fn is_container_asset(a: &super::AssetRecord, loc_name: &str, char_id: Option<i64>) -> bool {
  is_owned_by(a, char_id) && a.container_id != 0 && a.location_name.as_str() == loc_name
}

fn insert_container_entry<'a>(map: &mut BTreeMap<i64, &'a str>, a: &'a super::AssetRecord) {
  if let Some(path) = a.container_path.rsplit(" · ").next() {
    map.entry(a.container_id).or_insert(path);
  }
}

fn collect_containers<'a>(
  source: &'a [super::AssetRecord],
  loc_name: &str,
  char_id: Option<i64>,
) -> BTreeMap<i64, &'a str> {
  let mut map = BTreeMap::new();
  for a in source.iter().filter(|a| is_container_asset(a, loc_name, char_id)) {
    insert_container_entry(&mut map, a);
  }
  map
}

fn push_constellation_rows<'a>(
  items: &mut Vec<Element<'a, Message>>,
  source: &'a [super::AssetRecord],
  constellations: &BTreeMap<&'a str, BTreeSet<&'a str>>,
  char_id: Option<i64>,
  selected_loc: Option<&str>,
  collapsed_groups: &std::collections::HashSet<String>,
  query: &AssetFilterQuery,
) {
  for (constellation_name, systems) in constellations {
    let constellation_key = format!("constellation:{}", constellation_name);
    let constellation_collapsed = collapsed_groups.contains(&constellation_key);
    let constellation_count = count_filtered(source, char_id, query, |a| a.constellation_name == *constellation_name);
    items.push(
      ConstellationRow::new(
        *constellation_name,
        constellation_key,
        constellation_collapsed,
        constellation_count,
      )
      .render(),
    );
    if constellation_collapsed {
      continue;
    }
    for sys_name in systems {
      push_system_rows(items, source, sys_name, char_id, selected_loc);
    }
  }
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

fn collect_system_locations<'a>(
  source: &'a [super::AssetRecord],
  sys_name: &str,
  char_id: Option<i64>,
) -> BTreeSet<&'a str> {
  source
    .iter()
    .filter(|a| is_owned_by(a, char_id) && a.system_name.as_str() == sys_name)
    .map(|a| a.location_name.as_str())
    .collect()
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

  for loc_name in &collect_system_locations(source, sys_name, char_id) {
    push_location_rows(items, source, loc_name, char_id, selected_loc);
  }
}

fn is_owned_by(a: &super::AssetRecord, char_id: Option<i64>) -> bool {
  char_id.is_none_or(|id| a.character_id == id)
}

fn has_location_only(a: &super::AssetRecord) -> bool {
  a.system_name.is_empty() && a.region_name.is_empty() && !a.location_name.is_empty() && a.container_id == 0
}

fn is_system_asset(a: &super::AssetRecord, char_id: Option<i64>) -> bool {
  is_owned_by(a, char_id) && !a.system_name.is_empty()
}

fn is_structure_loc_asset(a: &super::AssetRecord, char_id: Option<i64>) -> bool {
  is_owned_by(a, char_id) && has_location_only(a)
}

fn collect_structure_locs(source: &[super::AssetRecord], char_id: Option<i64>) -> BTreeSet<&str> {
  source
    .iter()
    .filter(|a| is_structure_loc_asset(a, char_id))
    .map(|a| a.location_name.as_str())
    .collect()
}

fn asset_in_region_tree(a: &super::AssetRecord, char_id: Option<i64>) -> bool {
  !a.region_name.is_empty() && !a.system_name.is_empty() && is_owned_by(a, char_id)
}

fn build_region_tree<'a>(
  source: &'a [super::AssetRecord],
  char_id: Option<i64>,
) -> BTreeMap<&'a str, BTreeMap<&'a str, BTreeSet<&'a str>>> {
  let mut tree: BTreeMap<&str, BTreeMap<&str, BTreeSet<&str>>> = BTreeMap::new();
  for a in source.iter().filter(|a| asset_in_region_tree(a, char_id)) {
    tree
      .entry(a.region_name.as_str())
      .or_default()
      .entry(a.constellation_name.as_str())
      .or_default()
      .insert(a.system_name.as_str());
  }
  tree
}

fn push_region_row<'a>(
  items: &mut Vec<Element<'a, Message>>,
  source: &'a [super::AssetRecord],
  region_name: &'a str,
  constellations: &BTreeMap<&'a str, BTreeSet<&'a str>>,
  char_id: Option<i64>,
  selected_loc: Option<&str>,
  collapsed_groups: &std::collections::HashSet<String>,
  query: &AssetFilterQuery,
) {
  let region_key = format!("region:{}", region_name);
  let region_collapsed = collapsed_groups.contains(&region_key);
  let region_count = count_filtered(source, char_id, query, |a| a.region_name == region_name);
  items.push(RegionRow::new(region_name, region_key, region_collapsed, region_count).render());
  if !region_collapsed {
    push_constellation_rows(
      items,
      source,
      constellations,
      char_id,
      selected_loc,
      collapsed_groups,
      query,
    );
  }
}

fn push_region_tree<'a>(
  items: &mut Vec<Element<'a, Message>>,
  source: &'a [super::AssetRecord],
  char_id: Option<i64>,
  selected_loc: Option<&str>,
  collapsed_groups: &std::collections::HashSet<String>,
  query: &AssetFilterQuery,
) {
  let tree = build_region_tree(source, char_id);
  for (region_name, constellations) in &tree {
    push_region_row(
      items,
      source,
      region_name,
      constellations,
      char_id,
      selected_loc,
      collapsed_groups,
      query,
    );
  }
}

fn asset_passes_count_filter(
  a: &super::AssetRecord,
  char_id: Option<i64>,
  query: &AssetFilterQuery,
  group_filter: &impl Fn(&super::AssetRecord) -> bool,
) -> bool {
  is_owned_by(a, char_id) && group_filter(a) && query.matches(a)
}

fn count_filtered(
  source: &[super::AssetRecord],
  char_id: Option<i64>,
  query: &AssetFilterQuery,
  group_filter: impl Fn(&super::AssetRecord) -> bool,
) -> u64 {
  source
    .iter()
    .filter(|a| asset_passes_count_filter(a, char_id, query, &group_filter))
    .map(|a| a.quantity as u64)
    .sum()
}

fn is_ungrouped_system(a: &super::AssetRecord, owner_id: Option<i64>) -> bool {
  is_system_asset(a, owner_id) && a.region_name.is_empty()
}

fn collect_ungrouped_systems<'a>(source: &'a [super::AssetRecord], owner_id: Option<i64>) -> BTreeSet<&'a str> {
  source
    .iter()
    .filter(|a| is_ungrouped_system(a, owner_id))
    .map(|a| a.system_name.as_str())
    .collect()
}

fn build_sidebar_items<'a>(state: &'a State) -> Vec<Element<'a, Message>> {
  let mut items: Vec<Element<'_, Message>> = Vec::new();
  items.push(LocationsLabel::new().render());
  items.push(AllAssetsRow::new(state.selected_loc.is_none()).render());

  let source: &[super::AssetRecord] = &state.assets;
  let owner_id = state.selected_corporation().or_else(|| state.selected_character());
  let selected_loc = state.selected_loc.as_deref();
  let query = AssetFilterQuery::parse(&state.search_query).with_me(state.selected_character());

  push_region_tree(
    &mut items,
    source,
    owner_id,
    selected_loc,
    &state.collapsed_sidebar_groups,
    &query,
  );

  for loc_name in &collect_structure_locs(source, owner_id) {
    push_location_rows(&mut items, source, loc_name, owner_id, selected_loc);
  }

  for sys_name in &collect_ungrouped_systems(source, owner_id) {
    push_system_rows(&mut items, source, sys_name, owner_id, selected_loc);
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
      .width(Length::Fixed(self.state.sidebar_width))
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
