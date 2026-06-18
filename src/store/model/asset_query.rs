use sqlx::FromRow;

use crate::{
  clients::eve_image::Size,
  store::images::{self, IconResolution},
};

const INVENTORY_ICON_SIZE: Size = Size::S64;

#[derive(Clone, Debug, Default, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub struct AssetCompleteness {
  pub distinct_type_ids: i64,
  pub resolved: i64,
  pub unresolved: Vec<i64>,
}

impl AssetCompleteness {
  // Public store API exercised by unit tests; not yet wired into a production call site.
  #[allow(dead_code)]
  pub fn is_complete(&self) -> bool {
    self.unresolved.is_empty()
  }
}

#[derive(Clone, Debug, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
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
  pub row_volume: f64,
  pub type_icon: IconResolution,
  pub type_id: i64,
  pub type_name: String,
  pub unit_price: f64,
  pub value: f64,
}

impl InventoryRow {
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
#[allow(dead_code)]
pub struct NodeRollup {
  pub items: i64,
  pub value: f64,
}

#[derive(FromRow)]
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub struct NodeRollupSql {
  pub items: Option<i64>,
  pub value: Option<f64>,
}

#[derive(Clone, Debug, Eq, FromRow, Hash, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub struct ReferencedLocation {
  pub location_id: i64,
  pub location_type: String,
}

#[derive(FromRow)]
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
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
  #[allow(dead_code)]
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
  }
}
