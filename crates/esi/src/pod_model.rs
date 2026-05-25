//! Conversions from ESI response models into pod-model domain types.

use pod_model::{
  Bloodline, Constellation, DogmaAttributeEntry, DogmaEffectEntry, Faction, ItemCategory, ItemGroup, ItemType,
  MarketGroup, Planet, Race, Region, SolarSystem, Stargate, Station,
};

use crate::models::{
  market::MarketGroup as EsiMarketGroup,
  universe::{
    Bloodline as EsiBloodline, Category as EsiCategory, Constellation as EsiConstellation, Faction as EsiFaction,
    Group as EsiGroup, Planet as EsiPlanet, Race as EsiRace, Region as EsiRegion, SolarSystem as EsiSolarSystem,
    Stargate as EsiStargate, Station as EsiStation, TypeInfo as EsiTypeInfo,
  },
};

fn parse_dogma_attribute(v: serde_json::Value) -> Option<DogmaAttributeEntry> {
  let attribute_id = v.get("attribute_id")?.as_i64()? as i32;
  let value = v.get("value")?.as_f64()?;
  Some(DogmaAttributeEntry::new(attribute_id, value))
}

fn parse_dogma_effect(v: serde_json::Value) -> Option<DogmaEffectEntry> {
  let effect_id = v.get("effect_id")?.as_i64()? as i32;
  let is_default = v.get("is_default")?.as_bool()?;
  Some(DogmaEffectEntry::new(effect_id, is_default))
}

impl From<EsiBloodline> for Bloodline {
  fn from(esi: EsiBloodline) -> Self {
    let mut m = Self::new(esi.bloodline_id, esi.name);
    m.set_charisma(esi.charisma)
      .set_corporation_id(esi.corporation_id as i32)
      .set_description(esi.description)
      .set_intelligence(esi.intelligence)
      .set_memory(esi.memory)
      .set_perception(esi.perception)
      .set_race_id(esi.race_id)
      .set_ship_item_type_id(esi.ship_type_id)
      .set_will_power(esi.willpower);
    m
  }
}

impl From<EsiConstellation> for Constellation {
  fn from(esi: EsiConstellation) -> Self {
    let mut m = Self::new(esi.constellation_id as i32, esi.name);
    m.set_position(esi.position.x, esi.position.y, esi.position.z)
      .set_region_id(esi.region_id as i32);
    m
  }
}

impl From<EsiFaction> for Faction {
  fn from(esi: EsiFaction) -> Self {
    let mut m = Self::new(esi.faction_id as i32, esi.name);
    m.set_description(esi.description)
      .set_is_unique(esi.is_unique)
      .set_size_factor(esi.size_factor)
      .set_solar_system_id(esi.solar_system_id.map(|id| id as i32));
    m
  }
}

impl From<EsiCategory> for ItemCategory {
  fn from(esi: EsiCategory) -> Self {
    let mut m = Self::new(esi.category_id, esi.name);
    if esi.published {
      m.publish();
    } else {
      m.unpublish();
    }
    m
  }
}

impl From<EsiGroup> for ItemGroup {
  fn from(esi: EsiGroup) -> Self {
    let mut m = Self::new(esi.group_id, esi.category_id, esi.name);
    if esi.published {
      m.publish();
    } else {
      m.unpublish();
    }
    m
  }
}

impl From<EsiTypeInfo> for ItemType {
  fn from(esi: EsiTypeInfo) -> Self {
    let dogma_attributes: Vec<DogmaAttributeEntry> = esi
      .dogma_attributes
      .unwrap_or_default()
      .into_iter()
      .filter_map(parse_dogma_attribute)
      .collect();
    let dogma_effects: Vec<DogmaEffectEntry> = esi
      .dogma_effects
      .unwrap_or_default()
      .into_iter()
      .filter_map(parse_dogma_effect)
      .collect();
    let mut m = Self::new(esi.type_id, esi.name);
    m.set_capacity(esi.capacity)
      .set_description(esi.description)
      .set_graphic_id(esi.graphic_id)
      .set_icon_id(esi.icon_id)
      .set_item_group_id(esi.group_id)
      .set_market_group_id(esi.market_group_id)
      .set_mass(esi.mass)
      .set_packaged_volume(esi.packaged_volume)
      .set_portion_size(esi.portion_size)
      .set_published(esi.published)
      .set_radius(esi.radius)
      .set_volume(esi.volume);
    *m.dogma_attributes_mut() = dogma_attributes;
    *m.dogma_effects_mut() = dogma_effects;
    m
  }
}

impl From<EsiMarketGroup> for MarketGroup {
  fn from(esi: EsiMarketGroup) -> Self {
    let mut m = Self::new(esi.market_group_id, esi.name);
    m.set_description(Some(esi.description))
      .set_parent_market_group_id(esi.parent_group_id);
    m
  }
}

impl From<EsiPlanet> for Planet {
  fn from(esi: EsiPlanet) -> Self {
    let mut m = Self::new(esi.planet_id as i32, esi.name);
    m.set_item_type_id(esi.type_id)
      .set_position(esi.position.x, esi.position.y, esi.position.z)
      .set_solar_system_id(esi.system_id as i32);
    m
  }
}

impl From<EsiRace> for Race {
  fn from(esi: EsiRace) -> Self {
    let mut m = Self::new(esi.race_id, esi.name);
    m.set_alliance_id(esi.alliance_id as i32)
      .set_description(esi.description);
    m
  }
}

impl From<EsiRegion> for Region {
  fn from(esi: EsiRegion) -> Self {
    let mut m = Self::new(esi.region_id as i32, esi.name);
    m.set_description(esi.description);
    m
  }
}

impl From<EsiSolarSystem> for SolarSystem {
  fn from(esi: EsiSolarSystem) -> Self {
    let mut m = Self::new(esi.system_id as i32, esi.name);
    m.set_constellation_id(esi.constellation_id as i32)
      .set_position(esi.position.x, esi.position.y, esi.position.z)
      .set_security_class(esi.security_class)
      .set_security_status(esi.security_status)
      .set_star_id(esi.star_id.map(|id| id as i32));
    m
  }
}

impl From<EsiStargate> for Stargate {
  fn from(esi: EsiStargate) -> Self {
    let mut m = Self::new(esi.stargate_id as i32, esi.name);
    m.set_destination(esi.destination.stargate_id as i32, esi.destination.system_id as i32)
      .set_item_type_id(esi.type_id)
      .set_position(esi.position.x, esi.position.y, esi.position.z)
      .set_solar_system_id(esi.system_id as i32);
    m
  }
}

impl From<EsiStation> for Station {
  fn from(esi: EsiStation) -> Self {
    let mut m = Self::new(esi.station_id as i32, esi.name);
    m.set_item_type_id(esi.type_id)
      .set_max_dockable_ship_volume(esi.max_dockable_ship_volume)
      .set_office_rental_cost(esi.office_rental_cost)
      .set_owner_id(esi.owner.map(|id| id as i32))
      .set_position(esi.position.x, esi.position.y, esi.position.z)
      .set_race_id(esi.race_id)
      .set_reprocessing_efficiency(esi.reprocessing_efficiency)
      .set_reprocessing_stations_take(esi.reprocessing_stations_take)
      .set_solar_system_id(esi.system_id as i32);
    *m.services_mut() = esi.services;
    m
  }
}
