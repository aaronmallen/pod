use sqlx::FromRow;

use crate::{
  clients::eve_image::Size,
  store::images::{self, IconResolution},
};

const INVENTORY_ICON_SIZE: Size = Size::S64;

#[derive(Clone, Debug, Default, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
pub struct AssetCompleteness {
  pub distinct_type_ids: i64,
  pub resolved: i64,
  pub unresolved: Vec<i64>,
}

impl AssetCompleteness {
  // Public store API exercised by unit tests; not yet wired into a production call site.
  pub fn is_complete(&self) -> bool {
    self.unresolved.is_empty()
  }
}

#[derive(Clone, Debug, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
pub struct AssetRenderRow {
  pub category: String,
  pub container_id: Option<i64>,
  pub depth: i64,
  pub group_name: String,
  pub icon_id: Option<i64>,
  pub is_container: bool,
  pub item_id: i64,
  pub location_flag: String,
  pub location_id: i64,
  pub location_label: Option<String>,
  pub name: Option<String>,
  pub quantity: i64,
  pub type_id: i64,
  pub type_name: String,
  pub volume: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct ChildFilter<'a> {
  pub filter: &'a str,
  pub me_id: Option<i64>,
  pub path_container_ids: &'a [i64],
  pub reproc_yield: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoConstellationNode {
  pub constellation_id: i64,
  pub constellation_name: String,
  pub item_count: i64,
  pub systems: Vec<GeoSystemNode>,
  pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoLocation {
  pub constellation_id: Option<i64>,
  pub constellation_name: Option<String>,
  pub item_count: i64,
  pub location_id: i64,
  pub location_label: Option<String>,
  pub location_type: String,
  pub region_id: Option<i64>,
  pub region_name: Option<String>,
  pub security_status: Option<f64>,
  pub system_id: Option<i64>,
  pub system_name: Option<String>,
  pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoLocationNode {
  pub item_count: i64,
  pub location_id: i64,
  pub location_label: Option<String>,
  pub location_type: String,
  pub value: f64,
}

#[derive(FromRow)]
pub struct GeoLocationSql {
  pub constellation_id: Option<i64>,
  pub constellation_name: Option<String>,
  pub item_count: i64,
  pub location_id: i64,
  pub location_label: Option<String>,
  pub location_type: String,
  pub region_id: Option<i64>,
  pub region_name: Option<String>,
  pub security_status: Option<f64>,
  pub system_id: Option<i64>,
  pub system_name: Option<String>,
  pub value: f64,
}

impl GeoLocationSql {
  pub fn into_geo(self) -> GeoLocation {
    GeoLocation {
      constellation_id: self.constellation_id,
      constellation_name: self.constellation_name,
      item_count: self.item_count,
      location_id: self.location_id,
      location_label: self.location_label,
      location_type: self.location_type,
      region_id: self.region_id,
      region_name: self.region_name,
      security_status: self.security_status,
      system_id: self.system_id,
      system_name: self.system_name,
      value: self.value,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoRegionNode {
  pub constellations: Vec<GeoConstellationNode>,
  pub item_count: i64,
  pub region_id: i64,
  pub region_name: String,
  pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoSystemNode {
  pub item_count: i64,
  pub locations: Vec<GeoLocationNode>,
  pub security_status: Option<f64>,
  pub system_id: i64,
  pub system_name: String,
  pub value: f64,
}

/// Ordering applied to every tier of the location tree.
///
/// `Value` ranks nodes by their rolled-up ISK descending (today's default);
/// `Alpha` ranks them alphabetically by name. Both fall back to the node id as
/// a deterministic tiebreaker so equal-value or equal-name nodes never reshuffle
/// between renders.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GeoSort {
  Alpha,
  #[default]
  Value,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeoTree {
  pub orphans: Vec<GeoLocationNode>,
  pub regions: Vec<GeoRegionNode>,
}

impl GeoTree {
  pub fn from_locations(rows: &[GeoLocation]) -> Self {
    use std::collections::HashMap;

    struct ConstellationAcc {
      name: String,
      systems: HashMap<i64, SystemAcc>,
    }
    struct RegionAcc {
      constellations: HashMap<i64, ConstellationAcc>,
      name: String,
    }
    struct SystemAcc {
      locations: Vec<GeoLocationNode>,
      name: String,
      security_status: Option<f64>,
    }

    let mut orphans: Vec<GeoLocationNode> = Vec::new();
    let mut regions: HashMap<i64, RegionAcc> = HashMap::new();

    for row in rows {
      let node = GeoLocationNode {
        item_count: row.item_count,
        location_id: row.location_id,
        location_label: row.location_label.clone(),
        location_type: row.location_type.clone(),
        value: row.value,
      };

      match (row.region_id, row.constellation_id, row.system_id) {
        (Some(region_id), Some(constellation_id), Some(system_id)) => {
          let region = regions.entry(region_id).or_insert_with(|| RegionAcc {
            constellations: HashMap::new(),
            name: row.region_name.clone().unwrap_or_default(),
          });
          let constellation = region
            .constellations
            .entry(constellation_id)
            .or_insert_with(|| ConstellationAcc {
              name: row.constellation_name.clone().unwrap_or_default(),
              systems: HashMap::new(),
            });
          let system = constellation.systems.entry(system_id).or_insert_with(|| SystemAcc {
            locations: Vec::new(),
            name: row.system_name.clone().unwrap_or_default(),
            security_status: row.security_status,
          });
          system.locations.push(node);
        }
        _ => orphans.push(node),
      }
    }

    let mut region_nodes: Vec<GeoRegionNode> = regions
      .into_iter()
      .map(|(region_id, region)| {
        let mut constellation_nodes: Vec<GeoConstellationNode> = region
          .constellations
          .into_iter()
          .map(|(constellation_id, constellation)| {
            let mut system_nodes: Vec<GeoSystemNode> = constellation
              .systems
              .into_iter()
              .map(|(system_id, system)| {
                let mut locations = system.locations;
                locations.sort_by(|a, b| {
                  a.location_label
                    .cmp(&b.location_label)
                    .then(a.location_id.cmp(&b.location_id))
                });
                let item_count = locations.iter().map(|l| l.item_count).sum();
                let value = locations.iter().map(|l| l.value).sum();
                GeoSystemNode {
                  item_count,
                  locations,
                  security_status: system.security_status,
                  system_id,
                  system_name: system.name,
                  value,
                }
              })
              .collect();
            system_nodes.sort_by(|a, b| a.system_name.cmp(&b.system_name).then(a.system_id.cmp(&b.system_id)));
            let item_count = system_nodes.iter().map(|s| s.item_count).sum();
            let value = system_nodes.iter().map(|s| s.value).sum();
            GeoConstellationNode {
              constellation_id,
              constellation_name: constellation.name,
              item_count,
              systems: system_nodes,
              value,
            }
          })
          .collect();
        constellation_nodes.sort_by(|a, b| {
          a.constellation_name
            .cmp(&b.constellation_name)
            .then(a.constellation_id.cmp(&b.constellation_id))
        });
        let item_count = constellation_nodes.iter().map(|c| c.item_count).sum();
        let value = constellation_nodes.iter().map(|c| c.value).sum();
        GeoRegionNode {
          constellations: constellation_nodes,
          item_count,
          region_id,
          region_name: region.name,
          value,
        }
      })
      .collect();
    region_nodes.sort_by(|a, b| a.region_name.cmp(&b.region_name).then(a.region_id.cmp(&b.region_id)));

    orphans.sort_by(|a, b| {
      a.location_label
        .cmp(&b.location_label)
        .then(a.location_id.cmp(&b.location_id))
    });

    GeoTree {
      orphans,
      regions: region_nodes,
    }
  }

  /// Re-sorts every tier of the already-built tree in place to match `mode`.
  ///
  /// `from_locations` leaves each tier in `Alpha` order; this lets a toggle
  /// re-order the cached tree without a DB round-trip. Both modes resolve ties
  /// by id so equal-name / equal-value nodes keep a stable order between renders.
  pub fn sort_by(&mut self, mode: GeoSort) {
    for region in &mut self.regions {
      for constellation in &mut region.constellations {
        for system in &mut constellation.systems {
          system.locations.sort_by(|a, b| match mode {
            GeoSort::Alpha => a
              .location_label
              .cmp(&b.location_label)
              .then(a.location_id.cmp(&b.location_id)),
            GeoSort::Value => b
              .value
              .total_cmp(&a.value)
              .then(a.location_label.cmp(&b.location_label))
              .then(a.location_id.cmp(&b.location_id)),
          });
        }
        constellation.systems.sort_by(|a, b| match mode {
          GeoSort::Alpha => a.system_name.cmp(&b.system_name).then(a.system_id.cmp(&b.system_id)),
          GeoSort::Value => b
            .value
            .total_cmp(&a.value)
            .then(a.system_name.cmp(&b.system_name))
            .then(a.system_id.cmp(&b.system_id)),
        });
      }
      region.constellations.sort_by(|a, b| match mode {
        GeoSort::Alpha => a
          .constellation_name
          .cmp(&b.constellation_name)
          .then(a.constellation_id.cmp(&b.constellation_id)),
        GeoSort::Value => b
          .value
          .total_cmp(&a.value)
          .then(a.constellation_name.cmp(&b.constellation_name))
          .then(a.constellation_id.cmp(&b.constellation_id)),
      });
    }
    self.regions.sort_by(|a, b| match mode {
      GeoSort::Alpha => a.region_name.cmp(&b.region_name).then(a.region_id.cmp(&b.region_id)),
      GeoSort::Value => b
        .value
        .total_cmp(&a.value)
        .then(a.region_name.cmp(&b.region_name))
        .then(a.region_id.cmp(&b.region_id)),
    });
    self.orphans.sort_by(|a, b| match mode {
      GeoSort::Alpha => a
        .location_label
        .cmp(&b.location_label)
        .then(a.location_id.cmp(&b.location_id)),
      GeoSort::Value => b
        .value
        .total_cmp(&a.value)
        .then(a.location_label.cmp(&b.location_label))
        .then(a.location_id.cmp(&b.location_id)),
    });
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InventoryCursor {
  pub item_id: i64,
  pub sort_value: SortValue,
}

#[derive(Clone, Debug)]
pub struct InventoryQuery<'a> {
  pub cursor: Option<InventoryCursor>,
  pub direction: SortDirection,
  pub filter: &'a str,
  pub limit: i64,
  pub location_ids: &'a [i64],
  pub me_id: Option<i64>,
  pub reproc_yield: f64,
  pub sort: SortColumn,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InventoryRow {
  pub category: String,
  pub container_id: Option<i64>,
  pub depth: i64,
  pub group_name: String,
  pub is_active_ship: bool,
  pub is_blueprint_copy: Option<bool>,
  pub is_container: bool,
  pub item_id: i64,
  pub location_id: i64,
  pub location_label: Option<String>,
  pub name: Option<String>,
  pub owner_id: i64,
  pub quantity: i64,
  pub reproc_value: f64,
  pub row_volume: f64,
  pub type_icon: IconResolution,
  pub type_id: i64,
  pub type_name: String,
  pub unit_price: f64,
  pub value: f64,
}

impl InventoryRow {
  // The reproc-vs-sell verdict; surfaced on inventory rows by the assets UI.
  pub fn worth_reprocessing(&self) -> bool {
    self.reproc_value > self.value
  }

  pub fn cursor(&self, sort: SortColumn) -> InventoryCursor {
    let sort_value = match sort {
      SortColumn::Category => SortValue::Text(self.category.clone()),
      SortColumn::Group => SortValue::Text(self.group_name.clone()),
      SortColumn::Name => SortValue::Text(self.name.clone().unwrap_or_else(|| self.type_name.clone())),
      SortColumn::Owner => SortValue::Int(self.owner_id),
      SortColumn::Quantity => SortValue::Int(self.quantity),
      SortColumn::UnitPrice => SortValue::Real(self.unit_price),
      SortColumn::Value => SortValue::Real(self.value),
      SortColumn::Volume => SortValue::Real(self.row_volume),
    };
    InventoryCursor {
      item_id: self.item_id,
      sort_value,
    }
  }
}

#[derive(FromRow)]
pub struct InventoryRowSql {
  pub category: String,
  pub container_id: Option<i64>,
  pub depth: i64,
  pub group_name: String,
  pub is_active_ship: bool,
  pub is_blueprint_copy: Option<bool>,
  pub is_container: bool,
  pub item_id: i64,
  pub location_id: i64,
  pub location_label: Option<String>,
  pub name: Option<String>,
  pub owner_id: i64,
  pub quantity: i64,
  pub reproc_value: f64,
  pub row_volume: f64,
  pub type_id: i64,
  pub type_name: String,
  pub unit_price: f64,
  pub value: f64,
}

impl InventoryRowSql {
  pub fn into_row(self) -> InventoryRow {
    let type_icon =
      images::default_store().resolve_type_icon(self.type_id, self.is_blueprint_copy, INVENTORY_ICON_SIZE);
    InventoryRow {
      category: self.category,
      container_id: self.container_id,
      depth: self.depth,
      group_name: self.group_name,
      is_active_ship: self.is_active_ship,
      is_blueprint_copy: self.is_blueprint_copy,
      is_container: self.is_container,
      item_id: self.item_id,
      location_id: self.location_id,
      location_label: self.location_label,
      name: self.name,
      owner_id: self.owner_id,
      quantity: self.quantity,
      reproc_value: self.reproc_value,
      row_volume: self.row_volume,
      type_icon,
      type_id: self.type_id,
      type_name: self.type_name,
      unit_price: self.unit_price,
      value: self.value,
    }
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InventoryTotals {
  pub items: i64,
  pub locations: i64,
  pub value: f64,
  pub volume: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
pub struct NodeRollup {
  pub items: i64,
  pub value: f64,
}

#[derive(FromRow)]
// Public store API exercised by unit tests; not yet wired into a production call site.
pub struct NodeRollupSql {
  pub items: Option<i64>,
  pub value: Option<f64>,
}

#[derive(Clone, Debug, Eq, FromRow, Hash, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
pub struct ReferencedLocation {
  pub location_id: i64,
  pub location_type: String,
}

#[derive(FromRow)]
// Public store API exercised by unit tests; not yet wired into a production call site.
pub struct RenderRowSql {
  pub category: String,
  pub container_id: Option<i64>,
  pub depth: i64,
  pub group_name: String,
  pub icon_id: Option<i64>,
  pub is_container: bool,
  pub item_id: i64,
  pub location_flag: String,
  pub location_id: i64,
  pub location_label: Option<String>,
  pub name: Option<String>,
  pub quantity: i64,
  pub type_id: i64,
  pub type_name: String,
  pub volume: Option<f64>,
}

impl RenderRowSql {
  // Public store API exercised by unit tests; not yet wired into a production call site.
  pub fn into_row(self) -> AssetRenderRow {
    AssetRenderRow {
      category: self.category,
      container_id: self.container_id,
      depth: self.depth,
      group_name: self.group_name,
      icon_id: self.icon_id,
      is_container: self.is_container,
      item_id: self.item_id,
      location_flag: self.location_flag,
      location_id: self.location_id,
      location_label: self.location_label,
      name: self.name,
      quantity: self.quantity,
      type_id: self.type_id,
      type_name: self.type_name,
      volume: self.volume,
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortColumn {
  Category,
  Group,
  Name,
  Owner,
  Quantity,
  UnitPrice,
  Value,
  Volume,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortDirection {
  #[default]
  Ascending,
  Descending,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SortValue {
  Int(i64),
  Real(f64),
  Text(String),
}

#[derive(FromRow)]
pub struct TotalsRowSql {
  pub items: Option<i64>,
  pub locations: i64,
  pub value: Option<f64>,
  pub volume: Option<f64>,
}

#[cfg(test)]
mod tests {
  use super::*;

  mod geo_tree {
    use pretty_assertions::assert_eq;

    use super::*;

    fn nested(location_id: i64, label: &str, location_type: &str, item_count: i64, value: f64) -> GeoLocation {
      GeoLocation {
        constellation_id: Some(20_000_020),
        constellation_name: Some("Kimotoro".to_owned()),
        item_count,
        location_id,
        location_label: Some(label.to_owned()),
        location_type: location_type.to_owned(),
        region_id: Some(10_000_002),
        region_name: Some("The Forge".to_owned()),
        security_status: Some(0.9),
        system_id: Some(30_000_142),
        system_name: Some("Jita".to_owned()),
        value,
      }
    }

    #[test]
    fn it_buckets_a_location_with_no_resolvable_system_as_an_orphan() {
      let mut orphan = nested(1_022_000_000_000, "Inaccessible Structure", "structure", 1, 10.0);
      orphan.constellation_id = None;
      orphan.constellation_name = None;
      orphan.region_id = None;
      orphan.region_name = None;
      orphan.system_id = None;
      orphan.system_name = None;
      let rows = vec![nested(60_003_760, "Jita IV - Moon 4", "station", 2, 100.0), orphan];

      let tree = GeoTree::from_locations(&rows);

      assert_eq!(tree.regions.len(), 1, "the resolvable station nests under its region");
      assert_eq!(tree.orphans.len(), 1);
      assert_eq!(tree.orphans[0].location_id, 1_022_000_000_000);
      assert_eq!(tree.orphans[0].value, 10.0);
    }

    #[test]
    fn it_rolls_value_and_count_up_from_locations_to_the_region() {
      let rows = vec![
        nested(60_003_760, "Jita IV - Moon 4", "station", 2, 100.0),
        nested(1_021_000_000_000, "Jita Citadel", "structure", 3, 50.0),
      ];

      let tree = GeoTree::from_locations(&rows);

      assert_eq!(tree.regions.len(), 1);
      let region = &tree.regions[0];
      assert_eq!(region.item_count, 5);
      assert_eq!(region.value, 150.0);

      let constellation = &region.constellations[0];
      assert_eq!(constellation.item_count, 5);
      assert_eq!(constellation.value, 150.0);

      let system = &constellation.systems[0];
      assert_eq!(system.item_count, 5);
      assert_eq!(system.value, 150.0);
      assert_eq!(
        system.locations.iter().map(|l| l.location_id).collect::<Vec<_>>(),
        [1_021_000_000_000, 60_003_760],
        "locations sort by label then id"
      );
    }

    #[test]
    fn it_sums_two_systems_under_one_constellation() {
      let mut amarr = nested(60_008_494, "Amarr VIII", "station", 4, 400.0);
      amarr.system_id = Some(30_002_187);
      amarr.system_name = Some("Amarr".to_owned());
      let rows = vec![nested(60_003_760, "Jita IV - Moon 4", "station", 2, 100.0), amarr];

      let tree = GeoTree::from_locations(&rows);

      let constellation = &tree.regions[0].constellations[0];
      assert_eq!(constellation.item_count, 6);
      assert_eq!(constellation.value, 500.0);
      assert_eq!(
        constellation
          .systems
          .iter()
          .map(|s| s.system_name.as_str())
          .collect::<Vec<_>>(),
        ["Amarr", "Jita"],
        "systems sort by name"
      );
    }

    #[test]
    fn it_yields_an_empty_tree_for_no_rows() {
      assert_eq!(GeoTree::from_locations(&[]), GeoTree::default());
    }

    /// A second region (Domain → Throne Worlds → Amarr) so region-tier ordering
    /// is observable. `value` controls the Value-mode rank, `region_id` the tie
    /// break.
    fn other_region(region_id: i64, name: &str, value: f64) -> GeoLocation {
      GeoLocation {
        constellation_id: Some(20_000_322),
        constellation_name: Some("Throne Worlds".to_owned()),
        item_count: 1,
        location_id: 60_008_494,
        location_label: Some("Amarr VIII".to_owned()),
        location_type: "station".to_owned(),
        region_id: Some(region_id),
        region_name: Some(name.to_owned()),
        security_status: Some(1.0),
        system_id: Some(30_002_187),
        system_name: Some("Amarr".to_owned()),
        value,
      }
    }

    #[test]
    fn it_orders_regions_alphabetically_in_alpha_mode() {
      // "The Forge" rolls up a far larger value than "Domain", yet Alpha ignores
      // value and ranks "Domain" first by name.
      let rows = vec![
        nested(60_003_760, "Jita IV - Moon 4", "station", 2, 9_999.0),
        other_region(10_000_043, "Domain", 1.0),
      ];
      let mut tree = GeoTree::from_locations(&rows);

      tree.sort_by(GeoSort::Alpha);

      assert_eq!(
        tree.regions.iter().map(|r| r.region_name.as_str()).collect::<Vec<_>>(),
        ["Domain", "The Forge"],
        "Alpha orders regions by name regardless of value"
      );
    }

    #[test]
    fn it_orders_regions_by_descending_value_in_value_mode() {
      // "The Forge" is the higher-value region but sorts after "Domain"
      // alphabetically — Value must put it first.
      let rows = vec![
        nested(60_003_760, "Jita IV - Moon 4", "station", 2, 9_999.0),
        other_region(10_000_043, "Domain", 1.0),
      ];
      let mut tree = GeoTree::from_locations(&rows);

      tree.sort_by(GeoSort::Value);

      assert_eq!(
        tree.regions.iter().map(|r| r.region_name.as_str()).collect::<Vec<_>>(),
        ["The Forge", "Domain"],
        "Value orders regions by rolled-up ISK descending"
      );
    }

    #[test]
    fn it_orders_locations_within_a_system_by_descending_value_in_value_mode() {
      // Two stations in Jita: the cheaper one sorts first alphabetically but must
      // sort last by value.
      let rows = vec![
        nested(60_003_760, "Aaa Station", "station", 1, 10.0),
        nested(60_000_001, "Zzz Station", "station", 1, 1_000.0),
      ];
      let mut tree = GeoTree::from_locations(&rows);

      tree.sort_by(GeoSort::Value);
      let locations = &tree.regions[0].constellations[0].systems[0].locations;
      assert_eq!(
        locations.iter().map(|l| l.value).collect::<Vec<_>>(),
        [1_000.0, 10.0],
        "Value orders locations within a system by descending value"
      );

      tree.sort_by(GeoSort::Alpha);
      let locations = &tree.regions[0].constellations[0].systems[0].locations;
      assert_eq!(
        locations
          .iter()
          .map(|l| l.location_label.as_deref().unwrap())
          .collect::<Vec<_>>(),
        ["Aaa Station", "Zzz Station"],
        "Alpha orders locations within a system by label"
      );
    }

    #[test]
    fn it_breaks_equal_value_ties_by_id_deterministically() {
      // Two regions with identical rolled-up value; Value mode must fall back to a
      // stable name+id order so they never reshuffle between renders.
      let rows = vec![
        nested(60_003_760, "Jita IV - Moon 4", "station", 1, 100.0),
        other_region(10_000_043, "Domain", 100.0),
      ];
      let mut tree = GeoTree::from_locations(&rows);

      tree.sort_by(GeoSort::Value);

      assert_eq!(
        tree.regions.iter().map(|r| r.region_id).collect::<Vec<_>>(),
        [10_000_043, 10_000_002],
        "equal-value regions resolve by name (Domain < The Forge) then id"
      );
    }

    /// A station in a named, *sibling* constellation of The Forge so constellation-tier ordering is
    /// observable. `value` controls Value rank; `constellation_id` the tie break.
    fn other_constellation(
      constellation_id: i64,
      name: &str,
      system_id: i64,
      system_name: &str,
      value: f64,
    ) -> GeoLocation {
      GeoLocation {
        constellation_id: Some(constellation_id),
        constellation_name: Some(name.to_owned()),
        item_count: 1,
        location_id: 60_000_900,
        location_label: Some("Sibling Station".to_owned()),
        location_type: "station".to_owned(),
        region_id: Some(10_000_002),
        region_name: Some("The Forge".to_owned()),
        security_status: Some(0.8),
        system_id: Some(system_id),
        system_name: Some(system_name.to_owned()),
        value,
      }
    }

    #[test]
    fn it_orders_constellations_within_a_region_by_both_modes() {
      // Kimotoro (Jita) rolls up a far larger value than the sibling "Aaa Constellation".
      let rows = vec![
        nested(60_003_760, "Jita IV - Moon 4", "station", 1, 9_999.0),
        other_constellation(20_000_001, "Aaa Constellation", 30_000_001, "Aaa System", 1.0),
      ];
      let mut tree = GeoTree::from_locations(&rows);

      tree.sort_by(GeoSort::Alpha);
      assert_eq!(
        tree.regions[0]
          .constellations
          .iter()
          .map(|c| c.constellation_name.as_str())
          .collect::<Vec<_>>(),
        ["Aaa Constellation", "Kimotoro"],
        "Alpha orders constellations by name regardless of value"
      );

      tree.sort_by(GeoSort::Value);
      assert_eq!(
        tree.regions[0]
          .constellations
          .iter()
          .map(|c| c.constellation_name.as_str())
          .collect::<Vec<_>>(),
        ["Kimotoro", "Aaa Constellation"],
        "Value orders constellations by rolled-up ISK descending"
      );
    }

    #[test]
    fn it_breaks_equal_value_constellation_ties_by_name_then_id() {
      // Two constellations under The Forge with identical value; Value must fall back to name+id so
      // the order is stable across renders.
      let rows = vec![
        nested(60_003_760, "Jita IV - Moon 4", "station", 1, 100.0),
        other_constellation(20_000_001, "Aaa Constellation", 30_000_001, "Aaa System", 100.0),
      ];
      let mut tree = GeoTree::from_locations(&rows);

      tree.sort_by(GeoSort::Value);

      assert_eq!(
        tree.regions[0]
          .constellations
          .iter()
          .map(|c| c.constellation_name.as_str())
          .collect::<Vec<_>>(),
        ["Aaa Constellation", "Kimotoro"],
        "equal-value constellations resolve by name (Aaa < Kimotoro)"
      );
    }

    /// A second system inside Kimotoro so system-tier ordering is observable without changing the
    /// constellation. `value` controls Value rank; `system_id` the tie break.
    fn other_system(system_id: i64, system_name: &str, location_id: i64, value: f64) -> GeoLocation {
      GeoLocation {
        constellation_id: Some(20_000_020),
        constellation_name: Some("Kimotoro".to_owned()),
        item_count: 1,
        location_id,
        location_label: Some("Other Station".to_owned()),
        location_type: "station".to_owned(),
        region_id: Some(10_000_002),
        region_name: Some("The Forge".to_owned()),
        security_status: Some(0.7),
        system_id: Some(system_id),
        system_name: Some(system_name.to_owned()),
        value,
      }
    }

    #[test]
    fn it_orders_systems_within_a_constellation_by_both_modes() {
      // Jita rolls up more value than the sibling "Aaa System" but sorts after it alphabetically.
      let rows = vec![
        nested(60_003_760, "Jita IV - Moon 4", "station", 1, 9_999.0),
        other_system(30_000_001, "Aaa System", 60_000_777, 1.0),
      ];
      let mut tree = GeoTree::from_locations(&rows);

      tree.sort_by(GeoSort::Alpha);
      assert_eq!(
        tree.regions[0].constellations[0]
          .systems
          .iter()
          .map(|s| s.system_name.as_str())
          .collect::<Vec<_>>(),
        ["Aaa System", "Jita"],
        "Alpha orders systems by name regardless of value"
      );

      tree.sort_by(GeoSort::Value);
      assert_eq!(
        tree.regions[0].constellations[0]
          .systems
          .iter()
          .map(|s| s.system_name.as_str())
          .collect::<Vec<_>>(),
        ["Jita", "Aaa System"],
        "Value orders systems by rolled-up ISK descending"
      );
    }

    #[test]
    fn it_breaks_equal_value_system_ties_by_name_then_id() {
      // Two systems in Kimotoro with identical value; Value falls back to name then id.
      let rows = vec![
        nested(60_003_760, "Jita IV - Moon 4", "station", 1, 100.0),
        other_system(30_000_001, "Aaa System", 60_000_777, 100.0),
      ];
      let mut tree = GeoTree::from_locations(&rows);

      tree.sort_by(GeoSort::Value);

      assert_eq!(
        tree.regions[0].constellations[0]
          .systems
          .iter()
          .map(|s| s.system_name.as_str())
          .collect::<Vec<_>>(),
        ["Aaa System", "Jita"],
        "equal-value systems resolve by name (Aaa < Jita)"
      );
    }

    #[test]
    fn it_breaks_equal_name_and_value_locations_by_id() {
      // Two stations with the same label *and* value in the same system: only the location id can
      // order them, exercising the final `.then(id.cmp)` tiebreak in both modes.
      let rows = vec![
        nested(60_000_050, "Twin Station", "station", 1, 100.0),
        nested(60_000_049, "Twin Station", "station", 1, 100.0),
      ];
      let mut tree = GeoTree::from_locations(&rows);

      tree.sort_by(GeoSort::Alpha);
      assert_eq!(
        tree.regions[0].constellations[0].systems[0]
          .locations
          .iter()
          .map(|l| l.location_id)
          .collect::<Vec<_>>(),
        [60_000_049, 60_000_050],
        "Alpha breaks the equal-label tie by ascending id"
      );

      tree.sort_by(GeoSort::Value);
      assert_eq!(
        tree.regions[0].constellations[0].systems[0]
          .locations
          .iter()
          .map(|l| l.location_id)
          .collect::<Vec<_>>(),
        [60_000_049, 60_000_050],
        "Value breaks the equal-value, equal-label tie by ascending id"
      );
    }

    /// A location that resolves to no region/constellation/system, so it lands in `orphans`.
    fn orphan(location_id: i64, label: &str, value: f64) -> GeoLocation {
      let mut row = nested(location_id, label, "structure", 1, value);
      row.constellation_id = None;
      row.constellation_name = None;
      row.region_id = None;
      row.region_name = None;
      row.system_id = None;
      row.system_name = None;
      row
    }

    #[test]
    fn it_orders_orphans_by_both_modes_and_breaks_label_ties_by_id() {
      let rows = vec![
        orphan(1_022_000_000_001, "Zzz Orphan", 10.0),
        orphan(1_022_000_000_002, "Aaa Orphan", 1_000.0),
        orphan(1_022_000_000_004, "Mid Orphan", 50.0),
        orphan(1_022_000_000_003, "Mid Orphan", 50.0),
      ];
      let mut tree = GeoTree::from_locations(&rows);

      tree.sort_by(GeoSort::Alpha);
      assert_eq!(
        tree
          .orphans
          .iter()
          .map(|o| (o.location_label.as_deref().unwrap(), o.location_id))
          .collect::<Vec<_>>(),
        [
          ("Aaa Orphan", 1_022_000_000_002),
          ("Mid Orphan", 1_022_000_000_003),
          ("Mid Orphan", 1_022_000_000_004),
          ("Zzz Orphan", 1_022_000_000_001),
        ],
        "Alpha orders orphans by label, breaking ties by ascending id"
      );

      tree.sort_by(GeoSort::Value);
      assert_eq!(
        tree.orphans.iter().map(|o| o.value).collect::<Vec<_>>(),
        [1_000.0, 50.0, 50.0, 10.0],
        "Value orders orphans by descending value"
      );
      // The two equal-value "Mid Orphan" entries resolve by label then ascending id.
      let mids: Vec<i64> = tree
        .orphans
        .iter()
        .filter(|o| o.location_label.as_deref() == Some("Mid Orphan"))
        .map(|o| o.location_id)
        .collect();
      assert_eq!(
        mids,
        [1_022_000_000_003, 1_022_000_000_004],
        "equal-value orphans resolve by ascending id"
      );
    }
  }
}
