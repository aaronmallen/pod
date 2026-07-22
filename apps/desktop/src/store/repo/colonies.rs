use std::collections::HashSet;

use sqlx::{QueryBuilder, Sqlite};

use crate::store::{
  Database, Error,
  model::{CharacterPlanet, CharacterPlanetLink, CharacterPlanetPin, CharacterPlanetPinContent, CharacterPlanetRoute},
};

const COLONY_WRITE_BATCH_SIZE: usize = 500;

pub async fn list_planets_for_character(db: &Database, character_id: i64) -> Result<Vec<CharacterPlanet>, Error> {
  let rows = sqlx::query_as::<_, CharacterPlanet>(
    "SELECT character_id, last_update, num_pins, planet_id, planet_type, solar_system_id, upgrade_level \
    FROM character_planets WHERE character_id = ? ORDER BY planet_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn list_pins_for_character(db: &Database, character_id: i64) -> Result<Vec<CharacterPlanetPin>, Error> {
  let rows = sqlx::query_as::<_, CharacterPlanetPin>(
    "SELECT character_id, cycle_time, expiry_time, head_radius, install_time, last_cycle_start, latitude, longitude, \
    pin_id, planet_id, product_type_id, qty_per_cycle, schematic_id, type_id \
    FROM character_planet_pins WHERE character_id = ? ORDER BY planet_id, pin_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn list_pin_contents_for_character(
  db: &Database,
  character_id: i64,
) -> Result<Vec<CharacterPlanetPinContent>, Error> {
  let rows = sqlx::query_as::<_, CharacterPlanetPinContent>(
    "SELECT character_id, amount, pin_id, type_id FROM character_planet_pin_contents WHERE character_id = ? \
    ORDER BY pin_id, type_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn list_routes_for_character(db: &Database, character_id: i64) -> Result<Vec<CharacterPlanetRoute>, Error> {
  let rows = sqlx::query_as::<_, CharacterPlanetRoute>(
    "SELECT character_id, content_type_id, destination_pin_id, planet_id, quantity, route_id, source_pin_id \
    FROM character_planet_routes WHERE character_id = ? ORDER BY planet_id, route_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn list_links_for_character(db: &Database, character_id: i64) -> Result<Vec<CharacterPlanetLink>, Error> {
  let rows = sqlx::query_as::<_, CharacterPlanetLink>(
    "SELECT character_id, destination_pin_id, link_level, planet_id, source_pin_id \
    FROM character_planet_links WHERE character_id = ? ORDER BY planet_id, source_pin_id, destination_pin_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn replace_for_character(
  db: &Database,
  character_id: i64,
  planets: &[CharacterPlanet],
  pins: &[CharacterPlanetPin],
  contents: &[CharacterPlanetPinContent],
  routes: &[CharacterPlanetRoute],
  links: &[CharacterPlanetLink],
) -> Result<(), Error> {
  replace_for_character_batched(
    db,
    character_id,
    planets,
    pins,
    contents,
    routes,
    links,
    COLONY_WRITE_BATCH_SIZE,
  )
  .await
}

#[allow(clippy::too_many_arguments)]
pub async fn replace_for_character_batched(
  db: &Database,
  character_id: i64,
  planets: &[CharacterPlanet],
  pins: &[CharacterPlanetPin],
  contents: &[CharacterPlanetPinContent],
  routes: &[CharacterPlanetRoute],
  links: &[CharacterPlanetLink],
  batch_size: usize,
) -> Result<(), Error> {
  reconcile_planets(db, character_id, planets, batch_size).await?;
  reconcile_pins(db, character_id, pins, batch_size).await?;
  reconcile_pin_contents(db, character_id, contents, batch_size).await?;
  reconcile_routes(db, character_id, routes, batch_size).await?;
  reconcile_links(db, character_id, links, batch_size).await?;
  Ok(())
}

async fn reconcile_planets(
  db: &Database,
  character_id: i64,
  planets: &[CharacterPlanet],
  batch_size: usize,
) -> Result<(), Error> {
  let new_ids: HashSet<i64> = planets.iter().map(CharacterPlanet::planet_id).collect();
  let existing: Vec<i64> = sqlx::query_scalar("SELECT planet_id FROM character_planets WHERE character_id = ?")
    .bind(character_id)
    .fetch_all(&db.0)
    .await?;
  let stale: Vec<i64> = existing.into_iter().filter(|id| !new_ids.contains(id)).collect();

  let batch_size = batch_size.max(1);
  for chunk in planets.chunks(batch_size) {
    let mut tx = db.writer().begin().await?;
    for planet in chunk {
      insert_planet(&mut tx, planet).await?;
    }
    tx.commit().await?;
    tokio::task::yield_now().await;
  }
  for chunk in stale.chunks(batch_size) {
    delete_planets(db, character_id, chunk).await?;
    tokio::task::yield_now().await;
  }
  Ok(())
}

async fn reconcile_pins(
  db: &Database,
  character_id: i64,
  pins: &[CharacterPlanetPin],
  batch_size: usize,
) -> Result<(), Error> {
  let new_ids: HashSet<i64> = pins.iter().map(CharacterPlanetPin::pin_id).collect();
  let existing: Vec<i64> = sqlx::query_scalar("SELECT pin_id FROM character_planet_pins WHERE character_id = ?")
    .bind(character_id)
    .fetch_all(&db.0)
    .await?;
  let stale: Vec<i64> = existing.into_iter().filter(|id| !new_ids.contains(id)).collect();

  let batch_size = batch_size.max(1);
  for chunk in pins.chunks(batch_size) {
    let mut tx = db.writer().begin().await?;
    for pin in chunk {
      insert_pin(&mut tx, pin).await?;
    }
    tx.commit().await?;
    tokio::task::yield_now().await;
  }
  for chunk in stale.chunks(batch_size) {
    delete_pins(db, character_id, chunk).await?;
    tokio::task::yield_now().await;
  }
  Ok(())
}

async fn reconcile_routes(
  db: &Database,
  character_id: i64,
  routes: &[CharacterPlanetRoute],
  batch_size: usize,
) -> Result<(), Error> {
  let new_ids: HashSet<i64> = routes.iter().map(CharacterPlanetRoute::route_id).collect();
  let existing: Vec<i64> = sqlx::query_scalar("SELECT route_id FROM character_planet_routes WHERE character_id = ?")
    .bind(character_id)
    .fetch_all(&db.0)
    .await?;
  let stale: Vec<i64> = existing.into_iter().filter(|id| !new_ids.contains(id)).collect();

  let batch_size = batch_size.max(1);
  for chunk in routes.chunks(batch_size) {
    let mut tx = db.writer().begin().await?;
    for route in chunk {
      insert_route(&mut tx, route).await?;
    }
    tx.commit().await?;
    tokio::task::yield_now().await;
  }
  for chunk in stale.chunks(batch_size) {
    delete_routes(db, character_id, chunk).await?;
    tokio::task::yield_now().await;
  }
  Ok(())
}

async fn reconcile_pin_contents(
  db: &Database,
  character_id: i64,
  contents: &[CharacterPlanetPinContent],
  batch_size: usize,
) -> Result<(), Error> {
  let new_keys: HashSet<(i64, i64)> = contents.iter().map(|c| (c.pin_id(), c.type_id())).collect();
  let existing: Vec<(i64, i64)> =
    sqlx::query_as("SELECT pin_id, type_id FROM character_planet_pin_contents WHERE character_id = ?")
      .bind(character_id)
      .fetch_all(&db.0)
      .await?;
  let stale: Vec<(i64, i64)> = existing.into_iter().filter(|key| !new_keys.contains(key)).collect();

  let batch_size = batch_size.max(1);
  for chunk in contents.chunks(batch_size) {
    let mut tx = db.writer().begin().await?;
    for content in chunk {
      insert_pin_content(&mut tx, content).await?;
    }
    tx.commit().await?;
    tokio::task::yield_now().await;
  }
  for chunk in stale.chunks(batch_size) {
    delete_pin_contents(db, character_id, chunk).await?;
    tokio::task::yield_now().await;
  }
  Ok(())
}

async fn reconcile_links(
  db: &Database,
  character_id: i64,
  links: &[CharacterPlanetLink],
  batch_size: usize,
) -> Result<(), Error> {
  let new_keys: HashSet<(i64, i64)> = links
    .iter()
    .map(|l| (l.source_pin_id(), l.destination_pin_id()))
    .collect();
  let existing: Vec<(i64, i64)> =
    sqlx::query_as("SELECT source_pin_id, destination_pin_id FROM character_planet_links WHERE character_id = ?")
      .bind(character_id)
      .fetch_all(&db.0)
      .await?;
  let stale: Vec<(i64, i64)> = existing.into_iter().filter(|key| !new_keys.contains(key)).collect();

  let batch_size = batch_size.max(1);
  for chunk in links.chunks(batch_size) {
    let mut tx = db.writer().begin().await?;
    for link in chunk {
      insert_link(&mut tx, link).await?;
    }
    tx.commit().await?;
    tokio::task::yield_now().await;
  }
  for chunk in stale.chunks(batch_size) {
    delete_links(db, character_id, chunk).await?;
    tokio::task::yield_now().await;
  }
  Ok(())
}

async fn insert_planet(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, planet: &CharacterPlanet) -> Result<(), Error> {
  sqlx::query(
    "INSERT OR REPLACE INTO character_planets \
      (character_id, planet_id, last_update, num_pins, planet_type, solar_system_id, upgrade_level) \
    VALUES (?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(planet.character_id())
  .bind(planet.planet_id())
  .bind(planet.last_update())
  .bind(planet.num_pins())
  .bind(planet.planet_type())
  .bind(planet.solar_system_id())
  .bind(planet.upgrade_level())
  .execute(&mut **tx)
  .await?;
  Ok(())
}

async fn insert_pin(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, pin: &CharacterPlanetPin) -> Result<(), Error> {
  sqlx::query(
    "INSERT OR REPLACE INTO character_planet_pins \
      (character_id, pin_id, planet_id, cycle_time, expiry_time, head_radius, install_time, last_cycle_start, \
      latitude, longitude, product_type_id, qty_per_cycle, schematic_id, type_id) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(pin.character_id())
  .bind(pin.pin_id())
  .bind(pin.planet_id())
  .bind(pin.cycle_time())
  .bind(pin.expiry_time())
  .bind(pin.head_radius())
  .bind(pin.install_time())
  .bind(pin.last_cycle_start())
  .bind(pin.latitude())
  .bind(pin.longitude())
  .bind(pin.product_type_id())
  .bind(pin.qty_per_cycle())
  .bind(pin.schematic_id())
  .bind(pin.type_id())
  .execute(&mut **tx)
  .await?;
  Ok(())
}

async fn insert_pin_content(
  tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
  content: &CharacterPlanetPinContent,
) -> Result<(), Error> {
  sqlx::query(
    "INSERT OR REPLACE INTO character_planet_pin_contents (character_id, pin_id, type_id, amount) \
    VALUES (?, ?, ?, ?)",
  )
  .bind(content.character_id())
  .bind(content.pin_id())
  .bind(content.type_id())
  .bind(content.amount())
  .execute(&mut **tx)
  .await?;
  Ok(())
}

async fn insert_route(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, route: &CharacterPlanetRoute) -> Result<(), Error> {
  sqlx::query(
    "INSERT OR REPLACE INTO character_planet_routes \
      (character_id, route_id, planet_id, content_type_id, destination_pin_id, quantity, source_pin_id) \
    VALUES (?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(route.character_id())
  .bind(route.route_id())
  .bind(route.planet_id())
  .bind(route.content_type_id())
  .bind(route.destination_pin_id())
  .bind(route.quantity())
  .bind(route.source_pin_id())
  .execute(&mut **tx)
  .await?;
  Ok(())
}

async fn insert_link(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, link: &CharacterPlanetLink) -> Result<(), Error> {
  sqlx::query(
    "INSERT OR REPLACE INTO character_planet_links \
      (character_id, source_pin_id, destination_pin_id, planet_id, link_level) \
    VALUES (?, ?, ?, ?, ?)",
  )
  .bind(link.character_id())
  .bind(link.source_pin_id())
  .bind(link.destination_pin_id())
  .bind(link.planet_id())
  .bind(link.link_level())
  .execute(&mut **tx)
  .await?;
  Ok(())
}

async fn delete_planets(db: &Database, character_id: i64, planet_ids: &[i64]) -> Result<(), Error> {
  delete_by_owner_scalar(db, "character_planets", "planet_id", character_id, planet_ids).await
}

async fn delete_pins(db: &Database, character_id: i64, pin_ids: &[i64]) -> Result<(), Error> {
  delete_by_owner_scalar(db, "character_planet_pins", "pin_id", character_id, pin_ids).await
}

async fn delete_routes(db: &Database, character_id: i64, route_ids: &[i64]) -> Result<(), Error> {
  delete_by_owner_scalar(db, "character_planet_routes", "route_id", character_id, route_ids).await
}

async fn delete_by_owner_scalar(
  db: &Database,
  table: &str,
  key_column: &str,
  character_id: i64,
  ids: &[i64],
) -> Result<(), Error> {
  if ids.is_empty() {
    return Ok(());
  }
  let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM ");
  builder.push(table);
  builder.push(" WHERE character_id = ");
  builder.push_bind(character_id);
  builder.push(" AND ");
  builder.push(key_column);
  builder.push(" IN (");
  let mut separated = builder.separated(", ");
  for id in ids {
    separated.push_bind(*id);
  }
  builder.push(")");
  builder.build().execute(db.writer()).await?;
  Ok(())
}

async fn delete_pin_contents(db: &Database, character_id: i64, keys: &[(i64, i64)]) -> Result<(), Error> {
  delete_by_owner_pair(
    db,
    "character_planet_pin_contents",
    "pin_id",
    "type_id",
    character_id,
    keys,
  )
  .await
}

async fn delete_links(db: &Database, character_id: i64, keys: &[(i64, i64)]) -> Result<(), Error> {
  delete_by_owner_pair(
    db,
    "character_planet_links",
    "source_pin_id",
    "destination_pin_id",
    character_id,
    keys,
  )
  .await
}

async fn delete_by_owner_pair(
  db: &Database,
  table: &str,
  first_column: &str,
  second_column: &str,
  character_id: i64,
  keys: &[(i64, i64)],
) -> Result<(), Error> {
  if keys.is_empty() {
    return Ok(());
  }
  let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM ");
  builder.push(table);
  builder.push(" WHERE character_id = ");
  builder.push_bind(character_id);
  builder.push(" AND (");
  for (index, (first, second)) in keys.iter().enumerate() {
    if index > 0 {
      builder.push(" OR ");
    }
    builder.push("(");
    builder.push(first_column);
    builder.push(" = ");
    builder.push_bind(*first);
    builder.push(" AND ");
    builder.push(second_column);
    builder.push(" = ");
    builder.push_bind(*second);
    builder.push(")");
  }
  builder.push(")");
  builder.build().execute(db.writer()).await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  const CHARACTER_ID: i64 = 42;

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 98_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn planet(character_id: i64, planet_id: i64) -> CharacterPlanet {
    CharacterPlanet {
      character_id,
      last_update: "2026-07-13T12:00:00Z".to_owned(),
      num_pins: 6,
      planet_id,
      planet_type: "barren".to_owned(),
      solar_system_id: 30_000_142,
      upgrade_level: 5,
    }
  }

  fn extractor_pin(character_id: i64, planet_id: i64, pin_id: i64) -> CharacterPlanetPin {
    CharacterPlanetPin {
      character_id,
      cycle_time: Some(3_600),
      expiry_time: Some("2026-07-20T00:00:00Z".to_owned()),
      head_radius: Some(0.5),
      install_time: Some("2026-07-13T00:00:00Z".to_owned()),
      last_cycle_start: Some("2026-07-13T01:00:00Z".to_owned()),
      latitude: 1.25,
      longitude: 2.5,
      pin_id,
      planet_id,
      product_type_id: Some(2_268),
      qty_per_cycle: Some(1_500),
      schematic_id: None,
      type_id: 2_848,
    }
  }

  fn storage_pin(character_id: i64, planet_id: i64, pin_id: i64) -> CharacterPlanetPin {
    CharacterPlanetPin {
      character_id,
      cycle_time: None,
      expiry_time: None,
      head_radius: None,
      install_time: None,
      last_cycle_start: None,
      latitude: 3.0,
      longitude: 4.0,
      pin_id,
      planet_id,
      product_type_id: None,
      qty_per_cycle: None,
      schematic_id: Some(127),
      type_id: 2_541,
    }
  }

  fn pin_content(character_id: i64, pin_id: i64, type_id: i64, amount: i64) -> CharacterPlanetPinContent {
    CharacterPlanetPinContent {
      character_id,
      amount,
      pin_id,
      type_id,
    }
  }

  fn route(character_id: i64, planet_id: i64, route_id: i64, source: i64, destination: i64) -> CharacterPlanetRoute {
    CharacterPlanetRoute {
      character_id,
      content_type_id: 2_268,
      destination_pin_id: destination,
      planet_id,
      quantity: 3_000.0,
      route_id,
      source_pin_id: source,
    }
  }

  fn link(character_id: i64, planet_id: i64, source: i64, destination: i64) -> CharacterPlanetLink {
    CharacterPlanetLink {
      character_id,
      destination_pin_id: destination,
      link_level: 1,
      planet_id,
      source_pin_id: source,
    }
  }

  mod list_planets_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_round_trips_every_planet_field() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      let colony = planet(CHARACTER_ID, 40_000_001);

      super::replace_for_character(&db, CHARACTER_ID, std::slice::from_ref(&colony), &[], &[], &[], &[])
        .await
        .unwrap();

      let planets = super::list_planets_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(planets, vec![colony]);
    }
  }

  mod list_pins_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_round_trips_extractor_and_storage_pins() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      let extractor = extractor_pin(CHARACTER_ID, 40_000_001, 1_001);
      let storage = storage_pin(CHARACTER_ID, 40_000_001, 1_002);

      super::replace_for_character(
        &db,
        CHARACTER_ID,
        &[planet(CHARACTER_ID, 40_000_001)],
        &[extractor.clone(), storage.clone()],
        &[],
        &[],
        &[],
      )
      .await
      .unwrap();

      let pins = super::list_pins_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(pins, vec![extractor, storage]);
    }
  }

  mod list_pin_contents_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_round_trips_pin_contents() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      let contents = vec![
        pin_content(CHARACTER_ID, 1_002, 2_268, 500),
        pin_content(CHARACTER_ID, 1_002, 2_270, 250),
      ];

      super::replace_for_character(&db, CHARACTER_ID, &[], &[], &contents, &[], &[])
        .await
        .unwrap();

      let stored = super::list_pin_contents_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(stored, contents);
    }
  }

  mod list_routes_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_round_trips_routes() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      let routes = vec![route(CHARACTER_ID, 40_000_001, 700, 1_001, 1_002)];

      super::replace_for_character(&db, CHARACTER_ID, &[], &[], &[], &routes, &[])
        .await
        .unwrap();

      let stored = super::list_routes_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(stored, routes);
    }
  }

  mod list_links_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_round_trips_links() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      let links = vec![link(CHARACTER_ID, 40_000_001, 1_001, 1_002)];

      super::replace_for_character(&db, CHARACTER_ID, &[], &[], &[], &[], &links)
        .await
        .unwrap();

      let stored = super::list_links_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(stored, links);
    }
  }

  mod replace_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[allow(clippy::type_complexity)]
    fn full_layout(
      character_id: i64,
    ) -> (
      Vec<CharacterPlanet>,
      Vec<CharacterPlanetPin>,
      Vec<CharacterPlanetPinContent>,
      Vec<CharacterPlanetRoute>,
      Vec<CharacterPlanetLink>,
    ) {
      (
        vec![planet(character_id, 40_000_001), planet(character_id, 40_000_002)],
        vec![
          extractor_pin(character_id, 40_000_001, 1_001),
          storage_pin(character_id, 40_000_001, 1_002),
          storage_pin(character_id, 40_000_002, 2_001),
        ],
        vec![
          pin_content(character_id, 1_002, 2_268, 500),
          pin_content(character_id, 2_001, 2_270, 250),
        ],
        vec![
          route(character_id, 40_000_001, 700, 1_001, 1_002),
          route(character_id, 40_000_002, 800, 2_001, 2_001),
        ],
        vec![
          link(character_id, 40_000_001, 1_001, 1_002),
          link(character_id, 40_000_002, 2_001, 2_001),
        ],
      )
    }

    #[tokio::test]
    async fn it_writes_the_full_layout_across_every_table() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      let (planets, pins, contents, routes, links) = full_layout(CHARACTER_ID);

      super::replace_for_character(&db, CHARACTER_ID, &planets, &pins, &contents, &routes, &links)
        .await
        .unwrap();

      assert_eq!(
        super::list_planets_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .len(),
        2
      );
      assert_eq!(
        super::list_pins_for_character(&db, CHARACTER_ID).await.unwrap().len(),
        3
      );
      assert_eq!(
        super::list_pin_contents_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .len(),
        2
      );
      assert_eq!(
        super::list_routes_for_character(&db, CHARACTER_ID).await.unwrap().len(),
        2
      );
      assert_eq!(
        super::list_links_for_character(&db, CHARACTER_ID).await.unwrap().len(),
        2
      );
    }

    #[tokio::test]
    async fn it_prunes_stale_rows_from_every_table_on_re_sync() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      let (planets, pins, contents, routes, links) = full_layout(CHARACTER_ID);
      super::replace_for_character(&db, CHARACTER_ID, &planets, &pins, &contents, &routes, &links)
        .await
        .unwrap();

      super::replace_for_character(
        &db,
        CHARACTER_ID,
        &[planet(CHARACTER_ID, 40_000_001)],
        &[
          extractor_pin(CHARACTER_ID, 40_000_001, 1_001),
          storage_pin(CHARACTER_ID, 40_000_001, 1_002),
        ],
        &[pin_content(CHARACTER_ID, 1_002, 2_268, 500)],
        &[route(CHARACTER_ID, 40_000_001, 700, 1_001, 1_002)],
        &[link(CHARACTER_ID, 40_000_001, 1_001, 1_002)],
      )
      .await
      .unwrap();

      assert_eq!(
        super::list_planets_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .iter()
          .map(CharacterPlanet::planet_id)
          .collect::<Vec<_>>(),
        [40_000_001]
      );
      assert_eq!(
        super::list_pins_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .iter()
          .map(CharacterPlanetPin::pin_id)
          .collect::<Vec<_>>(),
        [1_001, 1_002]
      );
      assert_eq!(
        super::list_pin_contents_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .iter()
          .map(CharacterPlanetPinContent::pin_id)
          .collect::<Vec<_>>(),
        [1_002]
      );
      assert_eq!(
        super::list_routes_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .iter()
          .map(CharacterPlanetRoute::route_id)
          .collect::<Vec<_>>(),
        [700]
      );
      assert_eq!(
        super::list_links_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .iter()
          .map(CharacterPlanetLink::source_pin_id)
          .collect::<Vec<_>>(),
        [1_001]
      );
    }

    #[tokio::test]
    async fn it_updates_an_existing_pin_in_place() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      super::replace_for_character(
        &db,
        CHARACTER_ID,
        &[planet(CHARACTER_ID, 40_000_001)],
        &[storage_pin(CHARACTER_ID, 40_000_001, 1_002)],
        &[],
        &[],
        &[],
      )
      .await
      .unwrap();

      let mut updated = storage_pin(CHARACTER_ID, 40_000_001, 1_002);
      updated.type_id = 2_542;
      super::replace_for_character(
        &db,
        CHARACTER_ID,
        &[planet(CHARACTER_ID, 40_000_001)],
        std::slice::from_ref(&updated),
        &[],
        &[],
        &[],
      )
      .await
      .unwrap();

      let pins = super::list_pins_for_character(&db, CHARACTER_ID).await.unwrap();
      assert_eq!(pins, vec![updated]);
    }

    #[tokio::test]
    async fn it_cascades_when_the_character_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      let (planets, pins, contents, routes, links) = full_layout(CHARACTER_ID);
      super::replace_for_character(&db, CHARACTER_ID, &planets, &pins, &contents, &routes, &links)
        .await
        .unwrap();

      sqlx::query("DELETE FROM characters WHERE id = ?")
        .bind(CHARACTER_ID)
        .execute(db.writer())
        .await
        .unwrap();

      assert!(
        super::list_planets_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .is_empty()
      );
      assert!(
        super::list_pins_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .is_empty()
      );
      assert!(
        super::list_pin_contents_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .is_empty()
      );
      assert!(
        super::list_routes_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .is_empty()
      );
      assert!(
        super::list_links_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .is_empty()
      );
    }
  }

  mod replace_for_character_batched {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_writes_and_prunes_with_a_small_batch_size() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      super::replace_for_character_batched(
        &db,
        CHARACTER_ID,
        &[planet(CHARACTER_ID, 40_000_001), planet(CHARACTER_ID, 40_000_002)],
        &[
          storage_pin(CHARACTER_ID, 40_000_001, 1_002),
          storage_pin(CHARACTER_ID, 40_000_002, 2_001),
        ],
        &[
          pin_content(CHARACTER_ID, 1_002, 2_268, 500),
          pin_content(CHARACTER_ID, 2_001, 2_270, 250),
        ],
        &[],
        &[
          link(CHARACTER_ID, 40_000_001, 1_002, 1_002),
          link(CHARACTER_ID, 40_000_002, 2_001, 2_001),
        ],
        1,
      )
      .await
      .unwrap();

      super::replace_for_character_batched(
        &db,
        CHARACTER_ID,
        &[planet(CHARACTER_ID, 40_000_001)],
        &[storage_pin(CHARACTER_ID, 40_000_001, 1_002)],
        &[pin_content(CHARACTER_ID, 1_002, 2_268, 500)],
        &[],
        &[link(CHARACTER_ID, 40_000_001, 1_002, 1_002)],
        1,
      )
      .await
      .unwrap();

      assert_eq!(
        super::list_planets_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .len(),
        1
      );
      assert_eq!(
        super::list_pins_for_character(&db, CHARACTER_ID).await.unwrap().len(),
        1
      );
      assert_eq!(
        super::list_pin_contents_for_character(&db, CHARACTER_ID)
          .await
          .unwrap()
          .len(),
        1
      );
      assert_eq!(
        super::list_links_for_character(&db, CHARACTER_ID).await.unwrap().len(),
        1
      );
    }
  }
}
