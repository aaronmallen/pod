//! Universe ESI response models.

use serde::{Deserialize, Serialize};

/// An asteroid belt.
#[derive(Debug, Deserialize, Serialize)]
pub struct AsteroidBelt {
  pub name: String,
  pub position: Position,
  pub system_id: i64,
}

/// 3D position in space.
#[derive(Debug, Deserialize, Serialize)]
pub struct Position {
  pub x: f64,
  pub y: f64,
  pub z: f64,
}

/// An NPC faction.
#[derive(Debug, Deserialize, Serialize)]
pub struct Faction {
  pub corporation_id: Option<i64>,
  pub description: String,
  pub faction_id: i64,
  pub is_unique: bool,
  pub militia_corporation_id: Option<i64>,
  pub name: String,
  pub size_factor: f64,
  pub solar_system_id: Option<i64>,
  pub station_count: i32,
  pub station_system_count: i32,
}

/// A moon.
#[derive(Debug, Deserialize, Serialize)]
pub struct Moon {
  pub moon_id: i64,
  pub name: String,
  pub position: Position,
  pub system_id: i64,
}

/// A planet.
#[derive(Debug, Deserialize, Serialize)]
pub struct Planet {
  pub name: String,
  pub planet_id: i64,
  pub position: Position,
  pub system_id: i64,
  pub type_id: i32,
}

/// A star.
#[derive(Debug, Deserialize, Serialize)]
pub struct Star {
  pub age: i64,
  pub luminosity: f64,
  pub name: String,
  pub radius: i64,
  pub solar_system_id: i64,
  pub spectral_class: String,
  pub temperature: i32,
  pub type_id: i32,
}

/// A stargate.
#[derive(Debug, Deserialize, Serialize)]
pub struct Stargate {
  pub destination: StargateDestination,
  pub name: String,
  pub position: Position,
  pub stargate_id: i64,
  pub system_id: i64,
  pub type_id: i32,
}

/// The destination of a stargate.
#[derive(Debug, Deserialize, Serialize)]
pub struct StargateDestination {
  pub stargate_id: i64,
  pub system_id: i64,
}

/// A station.
#[derive(Debug, Deserialize, Serialize)]
pub struct Station {
  pub max_dockable_ship_volume: f64,
  pub name: String,
  pub office_rental_cost: f64,
  pub owner: Option<i64>,
  pub position: Position,
  pub race_id: Option<i32>,
  pub reprocessing_efficiency: f64,
  pub reprocessing_stations_take: f64,
  pub services: Vec<String>,
  pub station_id: i64,
  pub system_id: i64,
  pub type_id: i32,
}

/// A player-owned structure.
#[derive(Debug, Deserialize, Serialize)]
pub struct UniverseStructure {
  pub name: String,
  pub owner_id: i64,
  pub position: Option<Position>,
  pub solar_system_id: i64,
  pub type_id: Option<i32>,
}

/// A constellation.
#[derive(Debug, Deserialize, Serialize)]
pub struct Constellation {
  pub constellation_id: i64,
  pub name: String,
  pub position: Position,
  pub region_id: i64,
  pub systems: Vec<i64>,
}

/// A region.
#[derive(Debug, Deserialize, Serialize)]
pub struct Region {
  pub constellations: Vec<i64>,
  pub description: Option<String>,
  pub name: String,
  pub region_id: i64,
}

/// A solar system.
#[derive(Debug, Deserialize, Serialize)]
pub struct SolarSystem {
  pub constellation_id: i64,
  pub name: String,
  pub planets: Option<Vec<serde_json::Value>>,
  pub position: Position,
  pub security_class: Option<String>,
  pub security_status: f64,
  pub star_id: Option<i64>,
  pub stargates: Option<Vec<i64>>,
  pub stations: Option<Vec<i64>>,
  pub system_id: i64,
}

/// Jump statistics for a solar system.
#[derive(Debug, Deserialize, Serialize)]
pub struct SystemJump {
  pub ship_jumps: i32,
  pub system_id: i64,
}

/// Kill statistics for a solar system.
#[derive(Debug, Deserialize, Serialize)]
pub struct SystemKill {
  pub npc_kills: i32,
  pub pod_kills: i32,
  pub ship_kills: i32,
  pub system_id: i64,
}

/// An ancestry.
#[derive(Debug, Deserialize, Serialize)]
pub struct Ancestry {
  pub bloodline_id: i32,
  pub description: String,
  pub icon_id: Option<i32>,
  pub id: i32,
  pub name: String,
  pub short_description: Option<String>,
}

/// A bloodline.
#[derive(Debug, Deserialize, Serialize)]
pub struct Bloodline {
  pub bloodline_id: i32,
  pub charisma: i32,
  pub corporation_id: i64,
  pub description: String,
  pub intelligence: i32,
  pub memory: i32,
  pub name: String,
  pub perception: i32,
  pub race_id: i32,
  pub ship_type_id: i32,
  pub willpower: i32,
}

/// An item category.
#[derive(Debug, Deserialize, Serialize)]
pub struct Category {
  pub category_id: i32,
  pub groups: Vec<i32>,
  pub name: String,
  pub published: bool,
}

/// A graphic asset.
#[derive(Debug, Deserialize, Serialize)]
pub struct Graphic {
  pub collision_file: Option<String>,
  pub graphic_file: Option<String>,
  pub graphic_id: i32,
  pub icon_folder: Option<String>,
  pub sof_dna: Option<String>,
  pub sof_fation_name: Option<String>,
  pub sof_hull_name: Option<String>,
  pub sof_race_name: Option<String>,
}

/// An item group.
#[derive(Debug, Deserialize, Serialize)]
pub struct Group {
  pub category_id: i32,
  pub group_id: i32,
  pub name: String,
  pub published: bool,
  pub types: Vec<i32>,
}

/// A playable race.
#[derive(Debug, Deserialize, Serialize)]
pub struct Race {
  pub alliance_id: i64,
  pub description: String,
  pub name: String,
  pub race_id: i32,
}

/// A planetary industry schematic.
#[derive(Debug, Deserialize, Serialize)]
pub struct Schematic {
  pub cycle_time: i32,
  pub schematic_id: i32,
  pub schematic_name: String,
}

/// Full type information for an item.
#[derive(Debug, Deserialize, Serialize)]
pub struct TypeInfo {
  pub capacity: Option<f64>,
  pub description: String,
  pub dogma_attributes: Option<Vec<serde_json::Value>>,
  pub dogma_effects: Option<Vec<serde_json::Value>>,
  pub graphic_id: Option<i32>,
  pub group_id: i32,
  pub icon_id: Option<i32>,
  pub market_group_id: Option<i32>,
  pub mass: Option<f64>,
  pub name: String,
  pub packaged_volume: Option<f64>,
  pub portion_size: Option<i32>,
  pub published: bool,
  pub radius: Option<f64>,
  pub type_id: i32,
  pub volume: Option<f64>,
}

/// IDs resolved from names.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ResolvedIds {
  pub agents: Option<Vec<serde_json::Value>>,
  pub alliances: Option<Vec<serde_json::Value>>,
  pub characters: Option<Vec<serde_json::Value>>,
  pub constellations: Option<Vec<serde_json::Value>>,
  pub corporations: Option<Vec<serde_json::Value>>,
  pub factions: Option<Vec<serde_json::Value>>,
  pub inventory_types: Option<Vec<serde_json::Value>>,
  pub regions: Option<Vec<serde_json::Value>>,
  pub solar_systems: Option<Vec<serde_json::Value>>,
  pub stations: Option<Vec<serde_json::Value>>,
}

/// A resolved name entry.
#[derive(Debug, Deserialize, Serialize)]
pub struct ResolvedName {
  pub category: String,
  pub id: i64,
  pub name: String,
}
