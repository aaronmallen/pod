#![allow(dead_code)]

use chrono::Utc;
use sqlx::{QueryBuilder, Sqlite};

use crate::store::{
  Database, Error,
  model::{
    AgentType, Bloodline, Constellation, DogmaAttribute, Faction, InaccessibleStructure, ItemCategory, ItemGroup,
    ItemType, MarketGroup, NpcAgent, NpcAgentSkill, NpcCorporationDivision, OwnerType, Race, Region, SolarSystem,
    Station, Structure,
  },
};

const SELECT_COLUMNS: &str = "attribute_id, default_value, description, display_name, high_is_good, icon_id, name, \
  published, stackable, unit_id";

pub async fn get_bloodline(db: &Database, id: i64) -> Result<Option<Bloodline>, Error> {
  let row = sqlx::query_as::<_, Bloodline>(
    "SELECT charisma, corporation_id, description, id, intelligence, memory, name, \
    perception, race_id, ship_type_id, willpower FROM bloodlines WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn get_faction(db: &Database, id: i64) -> Result<Option<Faction>, Error> {
  let row = sqlx::query_as::<_, Faction>(
    "SELECT corporation_id, description, id, is_unique, militia_corporation_id, name, \
    size_factor, solar_system_id, station_count, station_system_count FROM factions WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn get_race(db: &Database, id: i64) -> Result<Option<Race>, Error> {
  let row = sqlx::query_as::<_, Race>("SELECT alliance_id, description, id, name FROM races WHERE id = ?")
    .bind(id)
    .fetch_optional(&db.0)
    .await?;
  Ok(row)
}

pub async fn upsert_bloodline(db: &Database, bloodline: &Bloodline) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO bloodlines \
      (id, charisma, corporation_id, description, intelligence, memory, name, \
      perception, race_id, ship_type_id, willpower) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      charisma        = excluded.charisma, \
      corporation_id  = excluded.corporation_id, \
      description     = excluded.description, \
      intelligence    = excluded.intelligence, \
      memory          = excluded.memory, \
      name            = excluded.name, \
      perception      = excluded.perception, \
      race_id         = excluded.race_id, \
      ship_type_id    = excluded.ship_type_id, \
      willpower       = excluded.willpower",
  )
  .bind(bloodline.id())
  .bind(bloodline.charisma())
  .bind(bloodline.corporation_id())
  .bind(bloodline.description())
  .bind(bloodline.intelligence())
  .bind(bloodline.memory())
  .bind(bloodline.name())
  .bind(bloodline.perception())
  .bind(bloodline.race_id())
  .bind(bloodline.ship_type_id())
  .bind(bloodline.willpower())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn upsert_faction(db: &Database, faction: &Faction) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO factions \
      (id, corporation_id, description, is_unique, militia_corporation_id, name, \
      size_factor, solar_system_id, station_count, station_system_count) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      corporation_id         = excluded.corporation_id, \
      description            = excluded.description, \
      is_unique              = excluded.is_unique, \
      militia_corporation_id = excluded.militia_corporation_id, \
      name                   = excluded.name, \
      size_factor            = excluded.size_factor, \
      solar_system_id        = excluded.solar_system_id, \
      station_count          = excluded.station_count, \
      station_system_count   = excluded.station_system_count",
  )
  .bind(faction.id())
  .bind(faction.corporation_id())
  .bind(faction.description())
  .bind(faction.is_unique())
  .bind(faction.militia_corporation_id())
  .bind(faction.name())
  .bind(faction.size_factor())
  .bind(faction.solar_system_id())
  .bind(faction.station_count())
  .bind(faction.station_system_count())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn upsert_race(db: &Database, race: &Race) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO races (id, alliance_id, description, name) VALUES (?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      alliance_id = excluded.alliance_id, \
      description = excluded.description, \
      name        = excluded.name",
  )
  .bind(race.id())
  .bind(race.alliance_id())
  .bind(race.description())
  .bind(race.name())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn get_dogma_attribute(db: &Database, attribute_id: i64) -> Result<Option<DogmaAttribute>, Error> {
  let row = sqlx::query_as::<_, DogmaAttribute>(
    "SELECT attribute_id, default_value, description, display_name, high_is_good, icon_id, name, published, \
    stackable, unit_id FROM dogma_attributes WHERE attribute_id = ?",
  )
  .bind(attribute_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn is_seeded(db: &Database) -> Result<bool, Error> {
  let exists = sqlx::query_scalar::<_, i64>("SELECT EXISTS(SELECT 1 FROM dogma_attributes)")
    .fetch_one(&db.0)
    .await?;
  Ok(exists == 1)
}

pub async fn get_dogma_attributes(db: &Database, attribute_ids: &[i64]) -> Result<Vec<DogmaAttribute>, Error> {
  if attribute_ids.is_empty() {
    return Ok(Vec::new());
  }

  let mut builder = QueryBuilder::<Sqlite>::new(format!(
    "SELECT {SELECT_COLUMNS} FROM dogma_attributes WHERE attribute_id IN ("
  ));
  let mut separated = builder.separated(", ");
  for id in attribute_ids {
    separated.push_bind(*id);
  }
  separated.push_unseparated(")");

  let rows = builder.build_query_as::<DogmaAttribute>().fetch_all(&db.0).await?;
  Ok(rows)
}

pub async fn upsert_many_dogma_attributes(db: &Database, attributes: &[DogmaAttribute]) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  for attribute in attributes {
    sqlx::query(
      "INSERT INTO dogma_attributes \
        (attribute_id, default_value, description, display_name, high_is_good, icon_id, name, published, \
        stackable, unit_id) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
      ON CONFLICT(attribute_id) DO UPDATE SET \
        default_value = excluded.default_value, \
        description = excluded.description, \
        display_name = excluded.display_name, \
        high_is_good = excluded.high_is_good, \
        icon_id = excluded.icon_id, \
        name = excluded.name, \
        published = excluded.published, \
        stackable = excluded.stackable, \
        unit_id = excluded.unit_id",
    )
    .bind(attribute.attribute_id())
    .bind(attribute.default_value())
    .bind(attribute.description())
    .bind(attribute.display_name())
    .bind(attribute.high_is_good())
    .bind(attribute.icon_id())
    .bind(attribute.name())
    .bind(attribute.published())
    .bind(attribute.stackable())
    .bind(attribute.unit_id())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn all_item_categories(db: &Database) -> Result<Vec<ItemCategory>, Error> {
  let rows = sqlx::query_as::<_, ItemCategory>("SELECT id, NULL AS icon_id, name, published FROM item_categories")
    .fetch_all(&db.0)
    .await?;
  Ok(rows)
}

pub async fn all_item_groups(db: &Database) -> Result<Vec<ItemGroup>, Error> {
  let rows =
    sqlx::query_as::<_, ItemGroup>("SELECT category_id, NULL AS icon_id, id, name, published FROM item_groups")
      .fetch_all(&db.0)
      .await?;
  Ok(rows)
}

pub async fn all_item_types(db: &Database) -> Result<Vec<ItemType>, Error> {
  let rows = sqlx::query_as::<_, ItemType>(
    "SELECT capacity, description, dogma_attributes, group_id, icon_id, id, market_group_id, name, \
    packaged_volume, portion_size, published, radius, volume FROM item_types",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn all_market_groups(db: &Database) -> Result<Vec<MarketGroup>, Error> {
  let rows = sqlx::query_as::<_, MarketGroup>(
    "SELECT description, 0 AS has_types, NULL AS icon_id, id, name, \
    parent_group_id AS parent_id FROM market_groups",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn get_item_category(db: &Database, id: i64) -> Result<Option<ItemCategory>, Error> {
  let row =
    sqlx::query_as::<_, ItemCategory>("SELECT id, NULL AS icon_id, name, published FROM item_categories WHERE id = ?")
      .bind(id)
      .fetch_optional(&db.0)
      .await?;
  Ok(row)
}

pub async fn get_item_group(db: &Database, id: i64) -> Result<Option<ItemGroup>, Error> {
  let row = sqlx::query_as::<_, ItemGroup>(
    "SELECT category_id, NULL AS icon_id, id, name, published FROM item_groups WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn get_item_type(db: &Database, id: i64) -> Result<Option<ItemType>, Error> {
  let row = sqlx::query_as::<_, ItemType>(
    "SELECT capacity, description, dogma_attributes, group_id, icon_id, id, market_group_id, name, \
    packaged_volume, portion_size, published, radius, volume FROM item_types WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn get_market_group(db: &Database, id: i64) -> Result<Option<MarketGroup>, Error> {
  let row = sqlx::query_as::<_, MarketGroup>(
    "SELECT description, 0 AS has_types, NULL AS icon_id, id, name, \
    parent_group_id AS parent_id FROM market_groups WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn insert_item_type_with_hierarchy(
  db: &Database,
  item_type: &ItemType,
  item_group: &ItemGroup,
  item_category: &ItemCategory,
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("INSERT OR IGNORE INTO item_categories (id, name, published) VALUES (?, ?, ?)")
    .bind(item_category.id())
    .bind(item_category.name())
    .bind(item_category.published())
    .execute(&mut *tx)
    .await?;

  sqlx::query("INSERT OR IGNORE INTO item_groups (id, category_id, name, published) VALUES (?, ?, ?, ?)")
    .bind(item_group.id())
    .bind(item_group.category_id())
    .bind(item_group.name())
    .bind(item_group.published())
    .execute(&mut *tx)
    .await?;

  sqlx::query(
    "INSERT OR IGNORE INTO item_types \
      (id, group_id, market_group_id, description, name, published, capacity, \
      dogma_attributes, packaged_volume, portion_size, radius, volume) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(item_type.id())
  .bind(item_type.group_id())
  .bind(item_type.market_group_id())
  .bind(item_type.description())
  .bind(item_type.name())
  .bind(item_type.published())
  .bind(item_type.capacity())
  .bind(item_type.dogma_attributes())
  .bind(item_type.packaged_volume())
  .bind(item_type.portion_size())
  .bind(item_type.radius())
  .bind(item_type.volume())
  .execute(&mut *tx)
  .await?;

  tx.commit().await?;
  Ok(())
}

pub async fn upsert_item_category(db: &Database, category: &ItemCategory) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO item_categories (id, name, published) VALUES (?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET name = excluded.name, published = excluded.published",
  )
  .bind(category.id())
  .bind(category.name())
  .bind(category.published())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn upsert_item_group(db: &Database, group: &ItemGroup) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO item_groups (id, category_id, name, published) VALUES (?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      category_id = excluded.category_id, \
      name        = excluded.name, \
      published   = excluded.published",
  )
  .bind(group.id())
  .bind(group.category_id())
  .bind(group.name())
  .bind(group.published())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn upsert_item_type(db: &Database, item_type: &ItemType) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO item_types \
      (id, group_id, market_group_id, description, name, published, capacity, \
      dogma_attributes, packaged_volume, portion_size, radius, volume) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      group_id         = excluded.group_id, \
      market_group_id  = excluded.market_group_id, \
      description      = excluded.description, \
      name             = excluded.name, \
      published        = excluded.published, \
      capacity         = excluded.capacity, \
      dogma_attributes = excluded.dogma_attributes, \
      packaged_volume  = excluded.packaged_volume, \
      portion_size     = excluded.portion_size, \
      radius           = excluded.radius, \
      volume           = excluded.volume",
  )
  .bind(item_type.id())
  .bind(item_type.group_id())
  .bind(item_type.market_group_id())
  .bind(item_type.description())
  .bind(item_type.name())
  .bind(item_type.published())
  .bind(item_type.capacity())
  .bind(item_type.dogma_attributes())
  .bind(item_type.packaged_volume())
  .bind(item_type.portion_size())
  .bind(item_type.radius())
  .bind(item_type.volume())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn upsert_market_group(db: &Database, group: &MarketGroup) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO market_groups (id, description, name, parent_group_id) VALUES (?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      description     = excluded.description, \
      name            = excluded.name, \
      parent_group_id = excluded.parent_group_id",
  )
  .bind(group.id())
  .bind(group.description())
  .bind(group.name())
  .bind(group.parent_id())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn upsert_many_item_categories(db: &Database, categories: &[ItemCategory]) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  for category in categories {
    sqlx::query(
      "INSERT INTO item_categories (id, name, published) VALUES (?, ?, ?) \
      ON CONFLICT(id) DO UPDATE SET name = excluded.name, published = excluded.published",
    )
    .bind(category.id())
    .bind(category.name())
    .bind(category.published())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn upsert_many_item_groups(db: &Database, groups: &[ItemGroup]) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  for group in groups {
    sqlx::query(
      "INSERT INTO item_groups (id, category_id, name, published) VALUES (?, ?, ?, ?) \
      ON CONFLICT(id) DO UPDATE SET \
        category_id = excluded.category_id, \
        name        = excluded.name, \
        published   = excluded.published",
    )
    .bind(group.id())
    .bind(group.category_id())
    .bind(group.name())
    .bind(group.published())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn upsert_many_market_groups(db: &Database, groups: &[MarketGroup]) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("PRAGMA defer_foreign_keys = ON").execute(&mut *tx).await?;

  for group in groups {
    sqlx::query(
      "INSERT INTO market_groups (id, description, name, parent_group_id) VALUES (?, ?, ?, ?) \
      ON CONFLICT(id) DO UPDATE SET \
        description     = excluded.description, \
        name            = excluded.name, \
        parent_group_id = excluded.parent_group_id",
    )
    .bind(group.id())
    .bind(group.description())
    .bind(group.name())
    .bind(group.parent_id())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn upsert_many_item_types(db: &Database, item_types: &[ItemType]) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  for item_type in item_types {
    sqlx::query(
      "INSERT INTO item_types \
        (id, group_id, market_group_id, description, name, published, capacity, \
        dogma_attributes, packaged_volume, portion_size, radius, volume) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
      ON CONFLICT(id) DO UPDATE SET \
        group_id         = excluded.group_id, \
        market_group_id  = excluded.market_group_id, \
        description      = excluded.description, \
        name             = excluded.name, \
        published        = excluded.published, \
        capacity         = excluded.capacity, \
        dogma_attributes = excluded.dogma_attributes, \
        packaged_volume  = excluded.packaged_volume, \
        portion_size     = excluded.portion_size, \
        radius           = excluded.radius, \
        volume           = excluded.volume",
    )
    .bind(item_type.id())
    .bind(item_type.group_id())
    .bind(item_type.market_group_id())
    .bind(item_type.description())
    .bind(item_type.name())
    .bind(item_type.published())
    .bind(item_type.capacity())
    .bind(item_type.dogma_attributes())
    .bind(item_type.packaged_volume())
    .bind(item_type.portion_size())
    .bind(item_type.radius())
    .bind(item_type.volume())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn all_constellations(db: &Database) -> Result<Vec<Constellation>, Error> {
  let rows = sqlx::query_as::<_, Constellation>(
    "SELECT id, name, position_x, position_y, position_z, region_id FROM constellations",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn all_regions(db: &Database) -> Result<Vec<Region>, Error> {
  let rows = sqlx::query_as::<_, Region>("SELECT description, id, name FROM regions")
    .fetch_all(&db.0)
    .await?;
  Ok(rows)
}

pub async fn all_solar_systems(db: &Database) -> Result<Vec<SolarSystem>, Error> {
  let rows = sqlx::query_as::<_, SolarSystem>(
    "SELECT constellation_id, id, name, position_x, position_y, position_z, \
    security_class, security_status, star_id FROM solar_systems",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn all_stations(db: &Database) -> Result<Vec<Station>, Error> {
  let rows = sqlx::query_as::<_, Station>(
    "SELECT id, max_dockable_ship_volume, name, office_rental_cost, owner, \
    position_x, position_y, position_z, race_id, reprocessing_efficiency, \
    reprocessing_stations_take, services, system_id, type_id FROM stations",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn all_structures(db: &Database) -> Result<Vec<Structure>, Error> {
  let rows = sqlx::query_as::<_, Structure>(
    "SELECT id, name, owner_id, position_x, position_y, position_z, \
    solar_system_id, type_id FROM structures",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn get_constellation(db: &Database, id: i64) -> Result<Option<Constellation>, Error> {
  let row = sqlx::query_as::<_, Constellation>(
    "SELECT id, name, position_x, position_y, position_z, region_id \
    FROM constellations WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn get_region(db: &Database, id: i64) -> Result<Option<Region>, Error> {
  let row = sqlx::query_as::<_, Region>("SELECT description, id, name FROM regions WHERE id = ?")
    .bind(id)
    .fetch_optional(&db.0)
    .await?;
  Ok(row)
}

pub async fn get_solar_system(db: &Database, id: i64) -> Result<Option<SolarSystem>, Error> {
  let row = sqlx::query_as::<_, SolarSystem>(
    "SELECT constellation_id, id, name, position_x, position_y, position_z, \
    security_class, security_status, star_id FROM solar_systems WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn get_station(db: &Database, id: i64) -> Result<Option<Station>, Error> {
  let row = sqlx::query_as::<_, Station>(
    "SELECT id, max_dockable_ship_volume, name, office_rental_cost, owner, \
    position_x, position_y, position_z, race_id, reprocessing_efficiency, \
    reprocessing_stations_take, services, system_id, type_id FROM stations WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn get_structure(db: &Database, id: i64) -> Result<Option<Structure>, Error> {
  let row = sqlx::query_as::<_, Structure>(
    "SELECT id, name, owner_id, position_x, position_y, position_z, \
    solar_system_id, type_id FROM structures WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn inaccessible_structures_for_owner(
  db: &Database,
  owner_id: i64,
  owner_type: OwnerType,
) -> Result<Vec<InaccessibleStructure>, Error> {
  let rows = sqlx::query_as::<_, InaccessibleStructure>(
    "SELECT id, marked_at, owner_id, owner_type FROM inaccessible_structures \
    WHERE owner_id = ? AND owner_type = ?",
  )
  .bind(owner_id)
  .bind(owner_type)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn insert_station_with_geography(
  db: &Database,
  station: &Station,
  system: &SolarSystem,
  constellation: &Constellation,
  region: &Region,
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("INSERT OR IGNORE INTO regions (id, description, name) VALUES (?, ?, ?)")
    .bind(region.id())
    .bind(region.description())
    .bind(region.name())
    .execute(&mut *tx)
    .await?;

  sqlx::query(
    "INSERT OR IGNORE INTO constellations \
      (id, name, position_x, position_y, position_z, region_id) \
    VALUES (?, ?, ?, ?, ?, ?)",
  )
  .bind(constellation.id())
  .bind(constellation.name())
  .bind(constellation.position_x())
  .bind(constellation.position_y())
  .bind(constellation.position_z())
  .bind(constellation.region_id())
  .execute(&mut *tx)
  .await?;

  sqlx::query(
    "INSERT OR IGNORE INTO solar_systems \
      (id, constellation_id, name, position_x, position_y, position_z, \
      security_class, security_status, star_id) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(system.id())
  .bind(system.constellation_id())
  .bind(system.name())
  .bind(system.position_x())
  .bind(system.position_y())
  .bind(system.position_z())
  .bind(system.security_class())
  .bind(system.security_status())
  .bind(system.star_id())
  .execute(&mut *tx)
  .await?;

  sqlx::query(
    "INSERT OR IGNORE INTO stations \
      (id, max_dockable_ship_volume, name, office_rental_cost, owner, \
      position_x, position_y, position_z, race_id, reprocessing_efficiency, \
      reprocessing_stations_take, services, system_id, type_id) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(station.id())
  .bind(station.max_dockable_ship_volume())
  .bind(station.name())
  .bind(station.office_rental_cost())
  .bind(station.owner())
  .bind(station.position_x())
  .bind(station.position_y())
  .bind(station.position_z())
  .bind(station.race_id())
  .bind(station.reprocessing_efficiency())
  .bind(station.reprocessing_stations_take())
  .bind(station.services())
  .bind(station.system_id())
  .bind(station.type_id())
  .execute(&mut *tx)
  .await?;

  tx.commit().await?;
  Ok(())
}

pub async fn is_structure_inaccessible(
  db: &Database,
  owner_id: i64,
  owner_type: OwnerType,
  id: i64,
) -> Result<bool, Error> {
  let row = sqlx::query_scalar::<_, i64>(
    "SELECT 1 FROM inaccessible_structures WHERE owner_id = ? AND owner_type = ? AND id = ?",
  )
  .bind(owner_id)
  .bind(owner_type)
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row.is_some())
}

pub async fn mark_inaccessible_structure(
  db: &Database,
  owner_id: i64,
  owner_type: OwnerType,
  id: i64,
) -> Result<(), Error> {
  let marked_at = Utc::now().to_rfc3339();
  sqlx::query(
    "INSERT INTO inaccessible_structures (owner_id, owner_type, id, marked_at) \
    VALUES (?, ?, ?, ?) \
    ON CONFLICT(owner_id, owner_type, id) DO UPDATE SET marked_at = excluded.marked_at",
  )
  .bind(owner_id)
  .bind(owner_type)
  .bind(id)
  .bind(marked_at)
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn upsert_constellation(db: &Database, constellation: &Constellation) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO constellations \
      (id, name, position_x, position_y, position_z, region_id) \
    VALUES (?, ?, ?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      name       = excluded.name, \
      position_x = excluded.position_x, \
      position_y = excluded.position_y, \
      position_z = excluded.position_z, \
      region_id  = excluded.region_id",
  )
  .bind(constellation.id())
  .bind(constellation.name())
  .bind(constellation.position_x())
  .bind(constellation.position_y())
  .bind(constellation.position_z())
  .bind(constellation.region_id())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn upsert_region(db: &Database, region: &Region) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO regions (id, description, name) VALUES (?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET description = excluded.description, name = excluded.name",
  )
  .bind(region.id())
  .bind(region.description())
  .bind(region.name())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn upsert_solar_system(db: &Database, system: &SolarSystem) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO solar_systems \
      (id, constellation_id, name, position_x, position_y, position_z, \
      security_class, security_status, star_id) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      constellation_id = excluded.constellation_id, \
      name             = excluded.name, \
      position_x       = excluded.position_x, \
      position_y       = excluded.position_y, \
      position_z       = excluded.position_z, \
      security_class   = excluded.security_class, \
      security_status  = excluded.security_status, \
      star_id          = excluded.star_id",
  )
  .bind(system.id())
  .bind(system.constellation_id())
  .bind(system.name())
  .bind(system.position_x())
  .bind(system.position_y())
  .bind(system.position_z())
  .bind(system.security_class())
  .bind(system.security_status())
  .bind(system.star_id())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn upsert_station(db: &Database, station: &Station) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO stations \
      (id, max_dockable_ship_volume, name, office_rental_cost, owner, \
      position_x, position_y, position_z, race_id, reprocessing_efficiency, \
      reprocessing_stations_take, services, system_id, type_id) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      max_dockable_ship_volume   = excluded.max_dockable_ship_volume, \
      name                       = excluded.name, \
      office_rental_cost         = excluded.office_rental_cost, \
      owner                      = excluded.owner, \
      position_x                 = excluded.position_x, \
      position_y                 = excluded.position_y, \
      position_z                 = excluded.position_z, \
      race_id                    = excluded.race_id, \
      reprocessing_efficiency    = excluded.reprocessing_efficiency, \
      reprocessing_stations_take = excluded.reprocessing_stations_take, \
      services                   = excluded.services, \
      system_id                  = excluded.system_id, \
      type_id                    = excluded.type_id",
  )
  .bind(station.id())
  .bind(station.max_dockable_ship_volume())
  .bind(station.name())
  .bind(station.office_rental_cost())
  .bind(station.owner())
  .bind(station.position_x())
  .bind(station.position_y())
  .bind(station.position_z())
  .bind(station.race_id())
  .bind(station.reprocessing_efficiency())
  .bind(station.reprocessing_stations_take())
  .bind(station.services())
  .bind(station.system_id())
  .bind(station.type_id())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn solar_system_names(db: &Database) -> Result<std::collections::HashMap<i64, String>, Error> {
  let rows = sqlx::query_as::<_, (i64, String)>("SELECT id, name FROM solar_systems")
    .fetch_all(&db.0)
    .await?;
  Ok(rows.into_iter().collect())
}

pub async fn upsert_many_agent_types(db: &Database, agent_types: &[AgentType]) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  for agent_type in agent_types {
    sqlx::query(
      "INSERT INTO agent_types (id, name) VALUES (?, ?) \
      ON CONFLICT(id) DO UPDATE SET name = excluded.name",
    )
    .bind(agent_type.id())
    .bind(agent_type.name())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn upsert_many_constellations(db: &Database, constellations: &[Constellation]) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  for constellation in constellations {
    sqlx::query(
      "INSERT INTO constellations \
        (id, name, position_x, position_y, position_z, region_id) \
      VALUES (?, ?, ?, ?, ?, ?) \
      ON CONFLICT(id) DO UPDATE SET \
        name       = excluded.name, \
        position_x = excluded.position_x, \
        position_y = excluded.position_y, \
        position_z = excluded.position_z, \
        region_id  = excluded.region_id",
    )
    .bind(constellation.id())
    .bind(constellation.name())
    .bind(constellation.position_x())
    .bind(constellation.position_y())
    .bind(constellation.position_z())
    .bind(constellation.region_id())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn upsert_many_npc_corporation_divisions(
  db: &Database,
  divisions: &[NpcCorporationDivision],
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  for division in divisions {
    sqlx::query(
      "INSERT INTO npc_corporation_divisions (id, name) VALUES (?, ?) \
      ON CONFLICT(id) DO UPDATE SET name = excluded.name",
    )
    .bind(division.id())
    .bind(division.name())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn upsert_many_regions(db: &Database, regions: &[Region]) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  for region in regions {
    sqlx::query(
      "INSERT INTO regions (id, description, name) VALUES (?, ?, ?) \
      ON CONFLICT(id) DO UPDATE SET name = excluded.name",
    )
    .bind(region.id())
    .bind(region.description())
    .bind(region.name())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn upsert_many_solar_systems(db: &Database, systems: &[SolarSystem]) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  for system in systems {
    sqlx::query(
      "INSERT INTO solar_systems \
        (id, constellation_id, name, position_x, position_y, position_z, \
        security_class, security_status, star_id) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
      ON CONFLICT(id) DO UPDATE SET \
        constellation_id = excluded.constellation_id, \
        name             = excluded.name, \
        position_x       = excluded.position_x, \
        position_y       = excluded.position_y, \
        position_z       = excluded.position_z, \
        security_class   = excluded.security_class, \
        security_status  = excluded.security_status, \
        star_id          = excluded.star_id",
    )
    .bind(system.id())
    .bind(system.constellation_id())
    .bind(system.name())
    .bind(system.position_x())
    .bind(system.position_y())
    .bind(system.position_z())
    .bind(system.security_class())
    .bind(system.security_status())
    .bind(system.star_id())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn seed_many_stations(db: &Database, stations: &[Station]) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;
  sqlx::query("PRAGMA defer_foreign_keys = ON").execute(&mut *tx).await?;

  for station in stations {
    sqlx::query(
      "INSERT INTO stations \
        (id, max_dockable_ship_volume, name, office_rental_cost, owner, \
        position_x, position_y, position_z, race_id, reprocessing_efficiency, \
        reprocessing_stations_take, services, system_id, type_id) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
      ON CONFLICT(id) DO UPDATE SET \
        name                       = excluded.name, \
        owner                      = excluded.owner, \
        position_x                 = excluded.position_x, \
        position_y                 = excluded.position_y, \
        position_z                 = excluded.position_z, \
        reprocessing_efficiency    = excluded.reprocessing_efficiency, \
        reprocessing_stations_take = excluded.reprocessing_stations_take, \
        system_id                  = excluded.system_id, \
        type_id                    = excluded.type_id",
    )
    .bind(station.id())
    .bind(station.max_dockable_ship_volume())
    .bind(station.name())
    .bind(station.office_rental_cost())
    .bind(station.owner())
    .bind(station.position_x())
    .bind(station.position_y())
    .bind(station.position_z())
    .bind(station.race_id())
    .bind(station.reprocessing_efficiency())
    .bind(station.reprocessing_stations_take())
    .bind(station.services())
    .bind(station.system_id())
    .bind(station.type_id())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn seed_many_npc_agents(db: &Database, agents: &[NpcAgent], skills: &[NpcAgentSkill]) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;
  sqlx::query("PRAGMA defer_foreign_keys = ON").execute(&mut *tx).await?;

  for agent in agents {
    sqlx::query(
      "INSERT INTO npc_agents \
        (id, agent_type_id, corporation_id, division_id, is_locator, level, location_id, name) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
      ON CONFLICT(id) DO UPDATE SET \
        agent_type_id  = excluded.agent_type_id, \
        corporation_id = excluded.corporation_id, \
        division_id    = excluded.division_id, \
        is_locator     = excluded.is_locator, \
        level          = excluded.level, \
        location_id    = excluded.location_id, \
        name           = excluded.name",
    )
    .bind(agent.id())
    .bind(agent.agent_type_id())
    .bind(agent.corporation_id())
    .bind(agent.division_id())
    .bind(agent.is_locator())
    .bind(agent.level())
    .bind(agent.location_id())
    .bind(agent.name())
    .execute(&mut *tx)
    .await?;
  }

  for skill in skills {
    sqlx::query("INSERT OR IGNORE INTO npc_agent_skills (agent_id, skill_type_id) VALUES (?, ?)")
      .bind(skill.agent_id())
      .bind(skill.skill_type_id())
      .execute(&mut *tx)
      .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn upsert_structure(db: &Database, structure: &Structure) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO structures \
      (id, name, owner_id, position_x, position_y, position_z, solar_system_id, type_id) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      name            = excluded.name, \
      owner_id        = excluded.owner_id, \
      position_x      = excluded.position_x, \
      position_y      = excluded.position_y, \
      position_z      = excluded.position_z, \
      solar_system_id = excluded.solar_system_id, \
      type_id         = excluded.type_id",
  )
  .bind(structure.id())
  .bind(structure.name())
  .bind(structure.owner_id())
  .bind(structure.position_x())
  .bind(structure.position_y())
  .bind(structure.position_z())
  .bind(structure.solar_system_id())
  .bind(structure.type_id())
  .execute(&db.0)
  .await?;
  Ok(())
}

#[cfg(test)]
mod dogma_tests {
  use super::*;
  use crate::store;

  fn attribute(attribute_id: i64, name: &str) -> DogmaAttribute {
    DogmaAttribute {
      attribute_id,
      default_value: Some(1.0),
      description: Some(format!("{name} description")),
      display_name: Some(format!("{name} Display")),
      high_is_good: true,
      icon_id: Some(42),
      name: name.to_owned(),
      published: true,
      stackable: false,
      unit_id: Some(115),
    }
  }

  mod get_dogma_attribute {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_the_matching_attribute() {
      let db = store::open_test().await.unwrap();
      upsert_many_dogma_attributes(&db, &[attribute(50, "cpuOutput")])
        .await
        .unwrap();

      let row = get_dogma_attribute(&db, 50).await.unwrap().unwrap();

      assert_eq!(row.attribute_id(), 50);
      assert_eq!(row.name(), "cpuOutput");
      assert_eq!(row.display_name().as_deref(), Some("cpuOutput Display"));
      assert_eq!(row.high_is_good(), true);
      assert_eq!(row.unit_id(), Some(115));
    }

    #[tokio::test]
    async fn it_returns_none_when_absent() {
      let db = store::open_test().await.unwrap();

      let row = get_dogma_attribute(&db, 999).await.unwrap();

      assert_eq!(row, None);
    }
  }

  mod get_dogma_attributes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_only_the_requested_ids() {
      let db = store::open_test().await.unwrap();
      upsert_many_dogma_attributes(
        &db,
        &[
          attribute(50, "cpuOutput"),
          attribute(51, "powerOutput"),
          attribute(52, "mass"),
        ],
      )
      .await
      .unwrap();

      let mut rows = get_dogma_attributes(&db, &[50, 52]).await.unwrap();
      rows.sort_by_key(|r| r.attribute_id());

      assert_eq!(rows.len(), 2);
      assert_eq!(rows[0].attribute_id(), 50);
      assert_eq!(rows[1].attribute_id(), 52);
    }

    #[tokio::test]
    async fn it_returns_empty_for_no_ids() {
      let db = store::open_test().await.unwrap();
      upsert_many_dogma_attributes(&db, &[attribute(50, "cpuOutput")])
        .await
        .unwrap();

      let rows = get_dogma_attributes(&db, &[]).await.unwrap();

      assert_eq!(rows.len(), 0);
    }
  }

  mod upsert_many_dogma_attributes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_updates_on_conflict() {
      let db = store::open_test().await.unwrap();
      upsert_many_dogma_attributes(&db, &[attribute(50, "cpuOutput")])
        .await
        .unwrap();

      let mut updated = attribute(50, "cpuOutputRenamed");
      updated.high_is_good = false;
      upsert_many_dogma_attributes(&db, &[updated]).await.unwrap();

      let row = get_dogma_attribute(&db, 50).await.unwrap().unwrap();

      assert_eq!(row.name(), "cpuOutputRenamed");
      assert_eq!(row.high_is_good(), false);
    }
  }
}

#[cfg(test)]
mod items_tests {
  use super::*;
  use crate::store;

  fn make_category(id: i64, name: &str) -> ItemCategory {
    ItemCategory {
      id,
      icon_id: None,
      name: name.to_string(),
      published: true,
    }
  }

  fn make_group(id: i64, category_id: i64, name: &str) -> ItemGroup {
    ItemGroup {
      category_id,
      icon_id: None,
      id,
      name: name.to_string(),
      published: true,
    }
  }

  fn make_item_type(id: i64, group_id: i64, name: &str) -> ItemType {
    ItemType {
      capacity: None,
      description: Some("Test item".to_string()),
      dogma_attributes: "[]".to_string(),
      group_id,
      icon_id: None,
      id,
      market_group_id: None,
      name: name.to_string(),
      packaged_volume: None,
      portion_size: None,
      published: true,
      radius: None,
      volume: None,
    }
  }

  fn make_market_group(id: i64, name: &str) -> MarketGroup {
    MarketGroup {
      description: name.to_string(),
      has_types: false,
      icon_id: None,
      id,
      name: name.to_string(),
      parent_id: None,
    }
  }

  mod all_item_categories {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_categories_exist() {
      let db = store::open_test().await.unwrap();

      let result = all_item_categories(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }

    #[tokio::test]
    async fn it_returns_all_stored_categories() {
      let db = store::open_test().await.unwrap();
      upsert_item_category(&db, &make_category(1, "Ships")).await.unwrap();
      upsert_item_category(&db, &make_category(2, "Modules")).await.unwrap();

      let result = all_item_categories(&db).await.unwrap();

      assert_eq!(result.len(), 2);
    }
  }

  mod all_item_groups {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_groups_exist() {
      let db = store::open_test().await.unwrap();

      let result = all_item_groups(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }
  }

  mod all_item_types {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_types_exist() {
      let db = store::open_test().await.unwrap();

      let result = all_item_types(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }
  }

  mod all_market_groups {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_market_groups_exist() {
      let db = store::open_test().await.unwrap();

      let result = all_market_groups(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }

    #[tokio::test]
    async fn it_returns_all_stored_market_groups() {
      let db = store::open_test().await.unwrap();
      upsert_market_group(&db, &make_market_group(1, "Frigates"))
        .await
        .unwrap();
      upsert_market_group(&db, &make_market_group(2, "Cruisers"))
        .await
        .unwrap();

      let result = all_market_groups(&db).await.unwrap();

      assert_eq!(result.len(), 2);
    }
  }

  mod get_item_category {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_id() {
      let db = store::open_test().await.unwrap();

      let result = get_item_category(&db, 9999).await.unwrap();

      assert_eq!(result, None);
    }

    #[tokio::test]
    async fn it_returns_the_category_for_a_known_id() {
      let db = store::open_test().await.unwrap();
      upsert_item_category(&db, &make_category(6, "Ships")).await.unwrap();

      let result = get_item_category(&db, 6).await.unwrap();

      assert!(result.is_some());
      assert_eq!(result.unwrap().id(), 6);
    }
  }

  mod get_item_group {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_id() {
      let db = store::open_test().await.unwrap();

      let result = get_item_group(&db, 9999).await.unwrap();

      assert_eq!(result, None);
    }
  }

  mod get_item_type {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_id() {
      let db = store::open_test().await.unwrap();

      let result = get_item_type(&db, 9999).await.unwrap();

      assert_eq!(result, None);
    }
  }

  mod get_market_group {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_id() {
      let db = store::open_test().await.unwrap();

      let result = get_market_group(&db, 9999).await.unwrap();

      assert_eq!(result, None);
    }
  }

  mod insert_item_type_with_hierarchy {
    use super::*;

    #[tokio::test]
    async fn it_inserts_the_full_hierarchy_without_fk_violation() {
      let db = store::open_test().await.unwrap();
      let category = make_category(6, "Ships");
      let group = make_group(25, 6, "Frigate");
      let item_type = make_item_type(587, 25, "Rifter");

      insert_item_type_with_hierarchy(&db, &item_type, &group, &category)
        .await
        .unwrap();

      assert!(get_item_type(&db, 587).await.unwrap().is_some());
      assert!(get_item_group(&db, 25).await.unwrap().is_some());
      assert!(get_item_category(&db, 6).await.unwrap().is_some());
    }
  }

  mod upsert_item_category {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_stores_a_new_category() {
      let db = store::open_test().await.unwrap();

      upsert_item_category(&db, &make_category(1, "Ships")).await.unwrap();

      let result = get_item_category(&db, 1).await.unwrap();
      assert!(result.is_some());
    }

    #[tokio::test]
    async fn it_updates_an_existing_category() {
      let db = store::open_test().await.unwrap();
      upsert_item_category(&db, &make_category(1, "Old Name")).await.unwrap();

      let updated = ItemCategory {
        id: 1,
        icon_id: None,
        name: "New Name".to_string(),
        published: true,
      };
      upsert_item_category(&db, &updated).await.unwrap();

      let result = get_item_category(&db, 1).await.unwrap().unwrap();
      assert_eq!(result.name(), "New Name");
    }
  }

  mod upsert_item_group {
    use super::*;

    #[tokio::test]
    async fn it_stores_a_new_group() {
      let db = store::open_test().await.unwrap();
      upsert_item_category(&db, &make_category(6, "Ships")).await.unwrap();

      upsert_item_group(&db, &make_group(25, 6, "Frigate")).await.unwrap();

      assert!(get_item_group(&db, 25).await.unwrap().is_some());
    }
  }

  mod upsert_item_type {
    use super::*;

    #[tokio::test]
    async fn it_stores_a_new_item_type() {
      let db = store::open_test().await.unwrap();
      upsert_item_category(&db, &make_category(6, "Ships")).await.unwrap();
      upsert_item_group(&db, &make_group(25, 6, "Frigate")).await.unwrap();

      upsert_item_type(&db, &make_item_type(587, 25, "Rifter")).await.unwrap();

      assert!(get_item_type(&db, 587).await.unwrap().is_some());
    }
  }

  mod upsert_market_group {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_stores_a_new_market_group() {
      let db = store::open_test().await.unwrap();

      upsert_market_group(&db, &make_market_group(157, "Frigates"))
        .await
        .unwrap();

      assert!(get_market_group(&db, 157).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_handles_self_referencing_parent_group_id() {
      let db = store::open_test().await.unwrap();
      upsert_market_group(&db, &make_market_group(157, "Frigates"))
        .await
        .unwrap();

      let child = MarketGroup {
        description: "T1 Frigates".to_string(),
        has_types: true,
        icon_id: None,
        id: 158,
        name: "Standard Frigates".to_string(),
        parent_id: Some(157),
      };
      upsert_market_group(&db, &child).await.unwrap();

      let result = get_market_group(&db, 158).await.unwrap().unwrap();
      assert_eq!(result.parent_id(), Some(157));
    }
  }

  mod upsert_many_item_categories {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_all_categories_in_one_transaction() {
      let db = store::open_test().await.unwrap();

      upsert_many_item_categories(&db, &[make_category(6, "Ship"), make_category(16, "Skill")])
        .await
        .unwrap();

      let result = all_item_categories(&db).await.unwrap();
      assert_eq!(result.len(), 2);
    }
  }

  mod upsert_many_item_groups {
    use super::*;

    #[tokio::test]
    async fn it_inserts_all_groups_in_one_transaction() {
      let db = store::open_test().await.unwrap();
      upsert_item_category(&db, &make_category(16, "Skill")).await.unwrap();

      upsert_many_item_groups(
        &db,
        &[
          make_group(255, 16, "Gunnery"),
          make_group(256, 16, "Missile Launcher Operation"),
        ],
      )
      .await
      .unwrap();

      assert!(get_item_group(&db, 255).await.unwrap().is_some());
      assert!(get_item_group(&db, 256).await.unwrap().is_some());
    }
  }

  mod upsert_many_market_groups {
    use super::*;

    #[tokio::test]
    async fn it_inserts_all_market_groups_in_one_transaction() {
      let db = store::open_test().await.unwrap();

      upsert_many_market_groups(
        &db,
        &[make_market_group(150, "Ships"), make_market_group(151, "Modules")],
      )
      .await
      .unwrap();

      assert!(get_market_group(&db, 150).await.unwrap().is_some());
      assert!(get_market_group(&db, 151).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_seeds_a_child_listed_before_its_parent() {
      let db = store::open_test().await.unwrap();
      let child = MarketGroup {
        description: "Standard Frigates".to_string(),
        has_types: true,
        icon_id: None,
        id: 158,
        name: "Standard Frigates".to_string(),
        parent_id: Some(157),
      };

      upsert_many_market_groups(&db, &[child, make_market_group(157, "Frigates")])
        .await
        .unwrap();

      assert_eq!(
        get_market_group(&db, 158).await.unwrap().unwrap().parent_id(),
        Some(157)
      );
    }
  }

  mod upsert_many_item_types {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_round_trips_dogma_attributes_for_a_seeded_skill_type() {
      let db = store::open_test().await.unwrap();
      upsert_item_category(&db, &make_category(16, "Skill")).await.unwrap();
      upsert_item_group(&db, &make_group(255, 16, "Gunnery")).await.unwrap();

      let dogma = "[{\"attribute_id\":275,\"value\":1.0},{\"attribute_id\":180,\"value\":3340.0},\
        {\"attribute_id\":181,\"value\":0.0}]";
      let mut skill_type = make_item_type(3300, 255, "Gunnery");
      skill_type.dogma_attributes = dogma.to_string();

      upsert_many_item_types(&db, &[skill_type, make_item_type(3301, 255, "Small Hybrid Turret")])
        .await
        .unwrap();

      let stored = get_item_type(&db, 3300).await.unwrap().unwrap();
      assert_eq!(stored.dogma_attributes(), dogma);
      assert!(get_item_type(&db, 3301).await.unwrap().is_some());
    }
  }
}

#[cfg(test)]
mod universe_tests {
  use super::*;
  use crate::store::{
    self,
    model::{ItemCategory, ItemGroup, ItemType},
    repo::sde::{upsert_item_category, upsert_item_group, upsert_item_type},
  };

  fn make_constellation(id: i64, region_id: i64) -> Constellation {
    Constellation {
      id,
      name: "Test Constellation".to_string(),
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      region_id,
    }
  }

  fn make_item_category() -> ItemCategory {
    ItemCategory {
      id: 1,
      icon_id: None,
      name: "Ship".to_string(),
      published: true,
    }
  }

  fn make_item_group() -> ItemGroup {
    ItemGroup {
      category_id: 1,
      icon_id: None,
      id: 1,
      name: "Frigate".to_string(),
      published: true,
    }
  }

  fn make_item_type(id: i64) -> ItemType {
    ItemType {
      capacity: None,
      description: Some("Test Item Type".to_string()),
      dogma_attributes: "[]".to_string(),
      group_id: 1,
      icon_id: None,
      id,
      market_group_id: None,
      name: "Caldari Station".to_string(),
      packaged_volume: None,
      portion_size: None,
      published: true,
      radius: None,
      volume: None,
    }
  }

  fn make_region(id: i64) -> Region {
    Region {
      description: None,
      id,
      name: "Test Region".to_string(),
    }
  }

  fn make_solar_system(id: i64, constellation_id: i64) -> SolarSystem {
    SolarSystem {
      constellation_id,
      id,
      name: "Test System".to_string(),
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      security_class: None,
      security_status: 1.0,
      star_id: None,
    }
  }

  fn make_station(id: i64, system_id: i64, type_id: i64) -> Station {
    Station {
      id,
      max_dockable_ship_volume: 1_000_000.0,
      name: "Test Station".to_string(),
      office_rental_cost: 10_000.0,
      owner: None,
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      race_id: None,
      reprocessing_efficiency: 0.5,
      reprocessing_stations_take: 0.05,
      services: "[]".to_string(),
      system_id,
      type_id,
    }
  }

  async fn seed_item_type(db: &store::Database, type_id: i64) {
    upsert_item_category(db, &make_item_category()).await.unwrap();
    upsert_item_group(db, &make_item_group()).await.unwrap();
    upsert_item_type(db, &make_item_type(type_id)).await.unwrap();
  }

  mod all_constellations {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_constellations_exist() {
      let db = store::open_test().await.unwrap();

      let result = all_constellations(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }
  }

  mod all_regions {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_regions_exist() {
      let db = store::open_test().await.unwrap();

      let result = all_regions(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }

    #[tokio::test]
    async fn it_returns_all_stored_regions() {
      let db = store::open_test().await.unwrap();
      upsert_region(&db, &make_region(1)).await.unwrap();
      upsert_region(&db, &make_region(2)).await.unwrap();

      let result = all_regions(&db).await.unwrap();

      assert_eq!(result.len(), 2);
    }
  }

  mod all_solar_systems {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_systems_exist() {
      let db = store::open_test().await.unwrap();

      let result = all_solar_systems(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }
  }

  mod all_stations {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_stations_exist() {
      let db = store::open_test().await.unwrap();

      let result = all_stations(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }
  }

  mod get_region {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_id() {
      let db = store::open_test().await.unwrap();

      let result = get_region(&db, 9999).await.unwrap();

      assert_eq!(result, None);
    }

    #[tokio::test]
    async fn it_returns_the_region_for_a_known_id() {
      let db = store::open_test().await.unwrap();
      upsert_region(&db, &make_region(10000001)).await.unwrap();

      let result = get_region(&db, 10000001).await.unwrap();

      assert!(result.is_some());
      assert_eq!(result.unwrap().id(), 10000001);
    }
  }

  mod get_station {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_id() {
      let db = store::open_test().await.unwrap();

      let result = get_station(&db, 9999).await.unwrap();

      assert_eq!(result, None);
    }
  }

  mod inaccessible_structures_for_owner {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_the_owner_has_no_marks() {
      let db = store::open_test().await.unwrap();

      let result = inaccessible_structures_for_owner(&db, 1, OwnerType::Character)
        .await
        .unwrap();

      assert_eq!(result, vec![]);
    }

    #[tokio::test]
    async fn it_lists_only_the_marks_for_the_given_owner() {
      let db = store::open_test().await.unwrap();
      mark_inaccessible_structure(&db, 1, OwnerType::Character, 100)
        .await
        .unwrap();
      mark_inaccessible_structure(&db, 1, OwnerType::Character, 200)
        .await
        .unwrap();
      mark_inaccessible_structure(&db, 1, OwnerType::Corporation, 300)
        .await
        .unwrap();
      mark_inaccessible_structure(&db, 2, OwnerType::Character, 400)
        .await
        .unwrap();

      let result = inaccessible_structures_for_owner(&db, 1, OwnerType::Character)
        .await
        .unwrap();

      let mut ids = result.iter().map(InaccessibleStructure::id).collect::<Vec<_>>();
      ids.sort_unstable();
      assert_eq!(ids, [100, 200]);
    }
  }

  mod insert_station_with_geography {
    use super::*;

    #[tokio::test]
    async fn it_inserts_the_full_geography_chain_without_fk_violation() {
      let db = store::open_test().await.unwrap();
      seed_item_type(&db, 54).await;
      let region = make_region(10000001);
      let constellation = make_constellation(20000001, 10000001);
      let system = make_solar_system(30000001, 20000001);
      let station = make_station(60000001, 30000001, 54);

      insert_station_with_geography(&db, &station, &system, &constellation, &region)
        .await
        .unwrap();

      assert!(get_station(&db, 60000001).await.unwrap().is_some());
      assert!(get_solar_system(&db, 30000001).await.unwrap().is_some());
      assert!(get_constellation(&db, 20000001).await.unwrap().is_some());
      assert!(get_region(&db, 10000001).await.unwrap().is_some());
    }
  }

  mod is_structure_inaccessible {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_false_when_the_structure_is_unmarked() {
      let db = store::open_test().await.unwrap();

      let result = is_structure_inaccessible(&db, 1, OwnerType::Character, 100)
        .await
        .unwrap();

      assert_eq!(result, false);
    }

    #[tokio::test]
    async fn it_is_true_after_the_structure_is_marked() {
      let db = store::open_test().await.unwrap();
      mark_inaccessible_structure(&db, 1, OwnerType::Character, 100)
        .await
        .unwrap();

      let result = is_structure_inaccessible(&db, 1, OwnerType::Character, 100)
        .await
        .unwrap();

      assert_eq!(result, true);
    }

    #[tokio::test]
    async fn it_is_scoped_to_the_marking_owner() {
      let db = store::open_test().await.unwrap();
      mark_inaccessible_structure(&db, 1, OwnerType::Character, 100)
        .await
        .unwrap();

      assert_eq!(
        is_structure_inaccessible(&db, 1, OwnerType::Corporation, 100)
          .await
          .unwrap(),
        false
      );
      assert_eq!(
        is_structure_inaccessible(&db, 2, OwnerType::Character, 100)
          .await
          .unwrap(),
        false
      );
    }
  }

  mod mark_inaccessible_structure {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_marks_a_structure_for_an_owner() {
      let db = store::open_test().await.unwrap();

      mark_inaccessible_structure(&db, 1, OwnerType::Character, 100)
        .await
        .unwrap();

      assert!(
        is_structure_inaccessible(&db, 1, OwnerType::Character, 100)
          .await
          .unwrap()
      );
    }

    #[tokio::test]
    async fn it_is_idempotent_on_the_composite_key() {
      let db = store::open_test().await.unwrap();

      mark_inaccessible_structure(&db, 1, OwnerType::Character, 100)
        .await
        .unwrap();
      mark_inaccessible_structure(&db, 1, OwnerType::Character, 100)
        .await
        .unwrap();

      let result = inaccessible_structures_for_owner(&db, 1, OwnerType::Character)
        .await
        .unwrap();
      assert_eq!(result.len(), 1);
    }
  }

  mod upsert_region {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_stores_a_new_region() {
      let db = store::open_test().await.unwrap();

      upsert_region(&db, &make_region(10000002)).await.unwrap();

      assert!(get_region(&db, 10000002).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_updates_an_existing_region() {
      let db = store::open_test().await.unwrap();
      upsert_region(&db, &make_region(10000003)).await.unwrap();

      let updated = Region {
        description: Some("Updated".to_string()),
        id: 10000003,
        name: "Updated Region".to_string(),
      };
      upsert_region(&db, &updated).await.unwrap();

      let result = get_region(&db, 10000003).await.unwrap().unwrap();
      assert_eq!(result.name(), "Updated Region");
    }
  }
}
