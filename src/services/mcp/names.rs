use std::{
  collections::{HashMap, HashSet},
  future::Future,
};

use serde::Serialize;

use crate::{
  clients::{Error as ClientError, esi::models::universe::NameRecord},
  store::{
    Database,
    repo::{character, org, sde},
  },
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error(transparent)]
  Resolver(#[from] ClientError),
  #[error(transparent)]
  Store(#[from] crate::store::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NameKind {
  Alliance,
  Character,
  Corporation,
  Group,
  Location,
  Type,
}

impl NameKind {
  fn from_category(category: &str) -> Self {
    match category {
      "alliance" => NameKind::Alliance,
      "corporation" | "faction" => NameKind::Corporation,
      "constellation" | "region" | "solar_system" | "station" | "structure" => NameKind::Location,
      "inventory_type" => NameKind::Type,
      _ => NameKind::Character,
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedName {
  pub kind: NameKind,
  pub name: String,
}

pub async fn resolve<F, Fut>(
  db: &Database,
  ids: &[i64],
  resolve_parties: F,
) -> Result<HashMap<i64, ResolvedName>, Error>
where
  F: FnOnce(Vec<i64>) -> Fut,
  Fut: Future<Output = Result<HashMap<i64, NameRecord>, ClientError>>,
{
  let wanted = deduped(ids);
  if wanted.is_empty() {
    return Ok(HashMap::new());
  }

  let mut resolved = HashMap::with_capacity(wanted.len());
  insert_types(db, &wanted, &mut resolved).await?;
  insert_groups(db, &remaining(&wanted, &resolved), &mut resolved).await?;
  insert_parties(db, &remaining(&wanted, &resolved), &mut resolved).await?;
  insert_locations(db, &remaining(&wanted, &resolved), &mut resolved).await?;

  let parties = remaining(&wanted, &resolved);
  if !parties.is_empty() {
    for (id, record) in resolve_parties(parties).await? {
      resolved.insert(
        id,
        ResolvedName {
          kind: NameKind::from_category(&record.category),
          name: record.name,
        },
      );
    }
  }
  Ok(resolved)
}

fn deduped(ids: &[i64]) -> Vec<i64> {
  let mut unique = ids.to_vec();
  unique.sort_unstable();
  unique.dedup();
  unique
}

async fn insert_groups(db: &Database, ids: &[i64], resolved: &mut HashMap<i64, ResolvedName>) -> Result<(), Error> {
  for (id, name) in sde::group_names_for(db, ids).await? {
    resolved.insert(
      id,
      ResolvedName {
        kind: NameKind::Group,
        name,
      },
    );
  }
  Ok(())
}

async fn insert_locations(db: &Database, ids: &[i64], resolved: &mut HashMap<i64, ResolvedName>) -> Result<(), Error> {
  if ids.is_empty() {
    return Ok(());
  }
  for station in sde::stations_for(db, ids).await? {
    resolved.insert(
      station.id,
      ResolvedName {
        kind: NameKind::Location,
        name: station.name,
      },
    );
  }
  for structure in sde::structures_for(db, ids).await? {
    resolved.insert(
      structure.id,
      ResolvedName {
        kind: NameKind::Location,
        name: structure.name,
      },
    );
  }
  for system in sde::solar_systems_for(db, ids).await? {
    resolved.insert(
      system.id,
      ResolvedName {
        kind: NameKind::Location,
        name: system.name,
      },
    );
  }
  for id in ids {
    if resolved.contains_key(id) {
      continue;
    }
    if let Some(region) = sde::get_region(db, *id).await? {
      resolved.insert(
        region.id,
        ResolvedName {
          kind: NameKind::Location,
          name: region.name,
        },
      );
    }
  }
  Ok(())
}

async fn insert_parties(db: &Database, ids: &[i64], resolved: &mut HashMap<i64, ResolvedName>) -> Result<(), Error> {
  if ids.is_empty() {
    return Ok(());
  }
  let wanted: HashSet<i64> = ids.iter().copied().collect();
  for (id, name) in org::corporation_names(db).await? {
    if wanted.contains(&id) {
      resolved.insert(
        id,
        ResolvedName {
          kind: NameKind::Corporation,
          name,
        },
      );
    }
  }
  for character in character::all(db).await? {
    if wanted.contains(&character.id()) {
      resolved.insert(
        character.id(),
        ResolvedName {
          kind: NameKind::Character,
          name: character.name().clone(),
        },
      );
    }
  }
  Ok(())
}

async fn insert_types(db: &Database, ids: &[i64], resolved: &mut HashMap<i64, ResolvedName>) -> Result<(), Error> {
  for (id, name, _) in sde::type_details_for(db, ids).await? {
    resolved.insert(
      id,
      ResolvedName {
        kind: NameKind::Type,
        name,
      },
    );
  }
  Ok(())
}

fn remaining(wanted: &[i64], resolved: &HashMap<i64, ResolvedName>) -> Vec<i64> {
  wanted.iter().copied().filter(|id| !resolved.contains_key(id)).collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  async fn forbidden(ids: Vec<i64>) -> Result<HashMap<i64, NameRecord>, ClientError> {
    panic!("the resolver must not run; ids reached it: {ids:?}");
  }

  async fn seed_corporation(db: &Database, id: i64, name: &str) {
    let mut corporation = store::model::Corporation::new(id, name, "TSC");
    corporation.set_ceo_id(1);
    corporation.set_creator_id(1);
    corporation.set_member_count(1);
    corporation.set_tax_rate(0.0);
    org::upsert_corporation(db, &corporation).await.unwrap();
  }

  async fn seed_region(db: &Database, id: i64, name: &str) {
    sqlx::query("INSERT INTO regions (id, description, name) VALUES (?, NULL, ?)")
      .bind(id)
      .bind(name)
      .execute(db.writer())
      .await
      .unwrap();
  }

  async fn seed_solar_system(db: &Database, id: i64, name: &str) {
    sqlx::query("INSERT OR IGNORE INTO regions (id, description, name) VALUES (1, NULL, 'Region')")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query(
      "INSERT OR IGNORE INTO constellations (id, region_id, name, position_x, position_y, position_z) \
        VALUES (1, 1, 'Constellation', 0, 0, 0)",
    )
    .execute(db.writer())
    .await
    .unwrap();
    sqlx::query(
      "INSERT INTO solar_systems (constellation_id, id, name, position_x, position_y, position_z, security_class, \
        security_status, star_id) VALUES (1, ?, ?, 0, 0, 0, NULL, 0, NULL)",
    )
    .bind(id)
    .bind(name)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_station(db: &Database, id: i64, name: &str) {
    seed_solar_system(db, 30_000_001, "System").await;
    seed_type(db, 54, "Station Type").await;
    sqlx::query(
      "INSERT INTO stations (id, max_dockable_ship_volume, name, office_rental_cost, owner, \
        reprocessing_efficiency, reprocessing_stations_take, services, system_id, type_id, position_x, position_y, \
        position_z, race_id) VALUES (?, 0, ?, 0, NULL, 0, 0, '[]', 30000001, 54, 0, 0, 0, NULL)",
    )
    .bind(id)
    .bind(name)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_type(db: &Database, id: i64, name: &str) {
    sqlx::query("INSERT OR IGNORE INTO item_categories (id, name, published) VALUES (6, 'Ship', 1)")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query("INSERT OR IGNORE INTO item_groups (id, category_id, name, published) VALUES (25, 6, 'Frigate', 1)")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query("INSERT INTO item_types (id, group_id, description, name, published) VALUES (?, 25, '', ?, 1)")
      .bind(id)
      .bind(name)
      .execute(db.writer())
      .await
      .unwrap();
  }

  mod resolve {
    use std::sync::{Arc, Mutex};

    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_resolves_an_sde_type_id_without_the_resolver() {
      let db = store::open_test().await.unwrap();
      seed_type(&db, 587, "Rifter").await;

      let resolved = super::super::resolve(&db, &[587], forbidden).await.unwrap();

      assert_eq!(
        resolved[&587],
        ResolvedName {
          kind: NameKind::Type,
          name: "Rifter".to_owned(),
        }
      );
    }

    #[tokio::test]
    async fn it_resolves_a_location_id_without_the_resolver() {
      let db = store::open_test().await.unwrap();
      seed_region(&db, 10_000_002, "The Forge").await;

      let resolved = super::super::resolve(&db, &[10_000_002], forbidden).await.unwrap();

      assert_eq!(
        resolved[&10_000_002],
        ResolvedName {
          kind: NameKind::Location,
          name: "The Forge".to_owned(),
        }
      );
    }

    #[tokio::test]
    async fn it_resolves_a_solar_system_id_without_the_resolver() {
      let db = store::open_test().await.unwrap();
      seed_solar_system(&db, 30_000_142, "Jita").await;

      let resolved = super::super::resolve(&db, &[30_000_142], forbidden).await.unwrap();

      assert_eq!(
        resolved[&30_000_142],
        ResolvedName {
          kind: NameKind::Location,
          name: "Jita".to_owned(),
        }
      );
    }

    #[tokio::test]
    async fn it_resolves_a_station_id_without_the_resolver() {
      let db = store::open_test().await.unwrap();
      seed_station(&db, 60_000_001, "Jita IV - Moon 4").await;

      let resolved = super::super::resolve(&db, &[60_000_001], forbidden).await.unwrap();

      assert_eq!(
        resolved[&60_000_001],
        ResolvedName {
          kind: NameKind::Location,
          name: "Jita IV - Moon 4".to_owned(),
        }
      );
    }

    #[tokio::test]
    async fn it_resolves_a_local_corporation_without_calling_the_resolver() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 98_356_193, "Test Corp").await;

      let resolved = super::super::resolve(&db, &[98_356_193], forbidden).await.unwrap();

      assert_eq!(
        resolved[&98_356_193],
        ResolvedName {
          kind: NameKind::Corporation,
          name: "Test Corp".to_owned(),
        }
      );
    }

    #[tokio::test]
    async fn it_routes_only_unknown_ids_to_the_resolver() {
      let db = store::open_test().await.unwrap();
      seed_type(&db, 587, "Rifter").await;
      let seen = Arc::new(Mutex::new(Vec::new()));
      let recorded = seen.clone();
      let resolver = move |ids: Vec<i64>| {
        let recorded = recorded.clone();
        async move {
          recorded.lock().unwrap().extend(ids.iter().copied());
          Ok::<_, ClientError>(
            ids
              .into_iter()
              .map(|id| {
                (
                  id,
                  NameRecord {
                    category: "character".to_owned(),
                    id,
                    name: format!("Pilot {id}"),
                  },
                )
              })
              .collect::<HashMap<_, _>>(),
          )
        }
      };

      let resolved = super::super::resolve(&db, &[587, 95_465_499], resolver).await.unwrap();

      assert_eq!(*seen.lock().unwrap(), vec![95_465_499]);
      assert_eq!(resolved[&587].kind, NameKind::Type);
      assert_eq!(
        resolved[&95_465_499],
        ResolvedName {
          kind: NameKind::Character,
          name: "Pilot 95465499".to_owned(),
        }
      );
    }
  }
}
