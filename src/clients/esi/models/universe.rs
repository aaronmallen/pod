use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Constellation {
  pub constellation_id: i64,
  pub name: String,
  pub position: Position,
  pub region_id: i64,
  pub systems: Vec<i64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DogmaAttribute {
  pub attribute_id: i32,
  pub value: f64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ItemCategory {
  pub category_id: i32,
  pub groups: Vec<i32>,
  pub name: String,
  pub published: bool,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ItemGroup {
  pub category_id: i32,
  pub group_id: i32,
  pub name: String,
  pub published: bool,
  pub types: Vec<i32>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ItemType {
  #[serde(default)]
  pub capacity: Option<f64>,
  pub description: String,
  #[serde(default)]
  pub dogma_attributes: Vec<DogmaAttribute>,
  #[serde(default)]
  pub graphic_id: Option<i32>,
  pub group_id: i32,
  #[serde(default)]
  pub icon_id: Option<i32>,
  #[serde(default)]
  pub market_group_id: Option<i32>,
  #[serde(default)]
  pub mass: Option<f64>,
  pub name: String,
  #[serde(default)]
  pub packaged_volume: Option<f64>,
  #[serde(default)]
  pub portion_size: Option<i32>,
  pub published: bool,
  #[serde(default)]
  pub radius: Option<f64>,
  pub type_id: i32,
  #[serde(default)]
  pub volume: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct MarketGroup {
  pub description: String,
  pub market_group_id: i32,
  pub name: String,
  #[serde(default)]
  pub parent_group_id: Option<i32>,
  pub types: Vec<i32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NameRecord {
  pub category: String,
  pub id: i64,
  pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Position {
  pub x: f64,
  pub y: f64,
  pub z: f64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Region {
  pub constellations: Vec<i64>,
  #[serde(default)]
  pub description: Option<String>,
  pub name: String,
  pub region_id: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedId {
  pub id: i64,
  pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedIds {
  #[serde(default)]
  pub alliances: Vec<ResolvedId>,
  #[serde(default)]
  pub characters: Vec<ResolvedId>,
  #[serde(default)]
  pub corporations: Vec<ResolvedId>,
  #[serde(default)]
  pub inventory_types: Vec<ResolvedId>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SearchResult {
  #[serde(default)]
  pub alliance: Vec<i64>,
  #[serde(default)]
  pub character: Vec<i64>,
  #[serde(default)]
  pub corporation: Vec<i64>,
  #[serde(default)]
  pub inventory_type: Vec<i64>,
  #[serde(default)]
  pub solar_system: Vec<i64>,
  #[serde(default)]
  pub station: Vec<i64>,
  #[serde(default)]
  pub structure: Vec<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SolarSystem {
  pub constellation_id: i64,
  pub name: String,
  pub position: Position,
  #[serde(default)]
  pub security_class: Option<String>,
  pub security_status: f64,
  #[serde(default)]
  pub star_id: Option<i64>,
  #[serde(default)]
  pub stargates: Option<Vec<i64>>,
  #[serde(default)]
  pub stations: Option<Vec<i64>>,
  pub system_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct Station {
  pub max_dockable_ship_volume: f64,
  pub name: String,
  pub office_rental_cost: f64,
  #[serde(default)]
  pub owner: Option<i64>,
  pub position: Position,
  #[serde(default)]
  pub race_id: Option<i32>,
  pub reprocessing_efficiency: f64,
  pub reprocessing_stations_take: f64,
  pub services: Vec<String>,
  pub station_id: i64,
  pub system_id: i64,
  pub type_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct Structure {
  pub name: String,
  pub owner_id: i64,
  #[serde(default)]
  pub position: Option<Position>,
  pub solar_system_id: i64,
  #[serde(default)]
  pub type_id: Option<i32>,
}
