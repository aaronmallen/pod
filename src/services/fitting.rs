#![allow(dead_code)]

use std::collections::HashMap;

use crate::store::{
  Database, Error,
  repo::fitting::{self, FittingModuleRow},
};

const QUANTUM_CORE_GROUP_ID: i64 = 4086;
const SERVICE_GROUP_IDS: [i64; 10] = [1321, 1322, 1323, 1324, 1325, 1326, 1415, 1416, 1887, 4603];
const HIGH_SLOT_GROUP_IDS: [i64; 7] = [1327, 1328, 1329, 1330, 1333, 1562, 1974];
const MID_SLOT_GROUP_IDS: [i64; 12] = [1331, 1332, 1429, 1430, 1441, 1442, 1535, 1719, 1962, 1966, 1967, 1968];

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SlotCategory {
  Core,
  High,
  Mid,
  Rig,
  Service,
  Unclassified,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FittedModule {
  pub cpu: f64,
  pub name: String,
  pub power: f64,
  pub slot: SlotCategory,
  pub type_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedModule {
  pub module: Option<FittedModule>,
  pub requested: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FitLoad {
  pub cpu: f64,
  pub power: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HullCapacity {
  pub cpu: f64,
  pub power: f64,
}

pub fn classify(group_id: i64, rig_size: Option<f64>) -> SlotCategory {
  if group_id == QUANTUM_CORE_GROUP_ID {
    SlotCategory::Core
  } else if rig_size.is_some() {
    SlotCategory::Rig
  } else if SERVICE_GROUP_IDS.contains(&group_id) {
    SlotCategory::Service
  } else if HIGH_SLOT_GROUP_IDS.contains(&group_id) {
    SlotCategory::High
  } else if MID_SLOT_GROUP_IDS.contains(&group_id) {
    SlotCategory::Mid
  } else {
    SlotCategory::Unclassified
  }
}

fn draws(slot: SlotCategory, power: Option<f64>, cpu: Option<f64>) -> (f64, f64) {
  match slot {
    SlotCategory::Rig | SlotCategory::Core => (0.0, 0.0),
    _ => (power.unwrap_or(0.0), cpu.unwrap_or(0.0)),
  }
}

fn fitted_module(row: &FittingModuleRow) -> FittedModule {
  let slot = classify(row.group_id, row.rig_size);
  let (power, cpu) = draws(slot, row.power, row.cpu);
  FittedModule {
    cpu,
    name: row.name.clone(),
    power,
    slot,
    type_id: row.type_id,
  }
}

pub fn aggregate_load(modules: &[FittedModule]) -> FitLoad {
  modules.iter().fold(FitLoad::default(), |mut load, module| {
    load.power += module.power;
    load.cpu += module.cpu;
    load
  })
}

pub async fn resolve_by_names(db: &Database, names: &[String]) -> Result<Vec<ResolvedModule>, Error> {
  let rows = fitting::modules_by_names(db, names).await?;
  let by_name: HashMap<String, &FittingModuleRow> = rows.iter().map(|row| (row.name.to_lowercase(), row)).collect();

  let resolved = names
    .iter()
    .map(|name| ResolvedModule {
      module: by_name.get(&name.to_lowercase()).map(|row| fitted_module(row)),
      requested: name.clone(),
    })
    .collect();
  Ok(resolved)
}

pub async fn resolve_by_ids(db: &Database, type_ids: &[i64]) -> Result<Vec<FittedModule>, Error> {
  let rows = fitting::modules_by_ids(db, type_ids).await?;
  Ok(rows.iter().map(fitted_module).collect())
}

pub async fn hull_capacity(db: &Database, hull_type_id: i64) -> Result<Option<HullCapacity>, Error> {
  let capacity = fitting::hull_capacity(db, hull_type_id).await?.map(|row| HullCapacity {
    cpu: row.cpu_output.unwrap_or(0.0),
    power: row.power_output.unwrap_or(0.0),
  });
  Ok(capacity)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn row(
    type_id: i64,
    group_id: i64,
    name: &str,
    power: Option<f64>,
    cpu: Option<f64>,
    rig_size: Option<f64>,
  ) -> FittingModuleRow {
    FittingModuleRow {
      cpu,
      group_id,
      name: name.to_owned(),
      power,
      rig_size,
      type_id,
    }
  }

  mod classify {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_classifies_a_high_slot_weapon() {
      assert_eq!(classify(1327, None), SlotCategory::High);
    }

    #[test]
    fn it_classifies_a_mid_slot_module() {
      assert_eq!(classify(1441, None), SlotCategory::Mid);
    }

    #[test]
    fn it_folds_low_effect_fitting_modules_into_mid() {
      assert_eq!(classify(1430, None), SlotCategory::Mid);
    }

    #[test]
    fn it_classifies_a_service_module() {
      assert_eq!(classify(1321, None), SlotCategory::Service);
    }

    #[test]
    fn it_classifies_a_rig_by_rig_size_attribute() {
      assert_eq!(classify(1816, Some(2.0)), SlotCategory::Rig);
    }

    #[test]
    fn it_classifies_a_quantum_core_unit() {
      assert_eq!(classify(4086, None), SlotCategory::Core);
    }

    #[test]
    fn it_returns_unclassified_for_an_unknown_group() {
      assert_eq!(classify(9, None), SlotCategory::Unclassified);
    }
  }

  mod fitted_module {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_exposes_pg_and_cpu_draw_for_a_high_slot_module() {
      let module = fitted_module(&row(35921, 1327, "Launcher", Some(150000.0), Some(1500.0), None));

      assert_eq!(module.slot, SlotCategory::High);
      assert_eq!(module.power, 150000.0);
      assert_eq!(module.cpu, 1500.0);
    }

    #[test]
    fn it_zeroes_draw_for_rigs() {
      let module = fitted_module(&row(43920, 1816, "Rig", None, None, Some(2.0)));

      assert_eq!(module.slot, SlotCategory::Rig);
      assert_eq!(module.power, 0.0);
      assert_eq!(module.cpu, 0.0);
    }

    #[test]
    fn it_zeroes_draw_for_cores() {
      let module = fitted_module(&row(56201, 4086, "Quantum Core", None, None, None));

      assert_eq!(module.slot, SlotCategory::Core);
      assert_eq!(module.power, 0.0);
      assert_eq!(module.cpu, 0.0);
    }
  }

  mod aggregate_load {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_pg_and_cpu_across_non_rig_modules() {
      let modules = vec![
        fitted_module(&row(1, 1327, "Hi", Some(100.0), Some(10.0), None)),
        fitted_module(&row(2, 1441, "Mid", Some(50.0), Some(25.0), None)),
        fitted_module(&row(3, 1816, "Rig", Some(999.0), Some(999.0), Some(2.0))),
        fitted_module(&row(4, 4086, "Core", None, None, None)),
      ];

      let load = aggregate_load(&modules);

      assert_eq!(load.power, 150.0);
      assert_eq!(load.cpu, 35.0);
    }

    #[test]
    fn it_returns_zero_for_an_empty_fit() {
      assert_eq!(aggregate_load(&[]), FitLoad::default());
    }
  }

  mod resolve_by_names {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    async fn seed(db: &Database, id: i64, group_id: i64, name: &str, dogma: &str) {
      sqlx::query("INSERT OR IGNORE INTO item_categories (id, name, published) VALUES (65, 'Structure', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("INSERT OR IGNORE INTO item_groups (id, category_id, name, published) VALUES (?, 65, 'Grp', 1)")
        .bind(group_id)
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT INTO item_types (id, group_id, description, name, published, dogma_attributes) \
        VALUES (?, ?, '', ?, 1, ?)",
      )
      .bind(id)
      .bind(group_id)
      .bind(name)
      .bind(dogma)
      .execute(db.writer())
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_tolerates_unknown_names_as_unresolved() {
      let db = store::open_test().await.unwrap();
      seed(
        &db,
        35921,
        1327,
        "Standup Missile Launcher I",
        r#"[{"attribute_id":30,"value":100.0},{"attribute_id":50,"value":10.0}]"#,
      )
      .await;

      let resolved = super::resolve_by_names(
        &db,
        &["Standup Missile Launcher I".to_owned(), "Nonexistent Module".to_owned()],
      )
      .await
      .unwrap();

      assert_eq!(resolved.len(), 2);
      assert_eq!(resolved[0].requested, "Standup Missile Launcher I");
      let module = resolved[0].module.as_ref().unwrap();
      assert_eq!(module.slot, SlotCategory::High);
      assert_eq!(module.power, 100.0);
      assert_eq!(resolved[1].requested, "Nonexistent Module");
      assert_eq!(resolved[1].module, None);
    }
  }

  mod hull_capacity {
    use pretty_assertions::assert_eq;

    use crate::store;

    #[tokio::test]
    async fn it_maps_hull_output_attributes_to_capacity() {
      let db = store::open_test().await.unwrap();
      sqlx::query("INSERT OR IGNORE INTO item_categories (id, name, published) VALUES (65, 'Structure', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT OR IGNORE INTO item_groups (id, category_id, name, published) VALUES (1657, 65, 'Citadel', 1)",
      )
      .execute(db.writer())
      .await
      .unwrap();
      sqlx::query(
        "INSERT INTO item_types (id, group_id, description, name, published, dogma_attributes) \
        VALUES (35832, 1657, '', 'Astrahus', 1, ?)",
      )
      .bind(r#"[{"attribute_id":11,"value":1500000.0},{"attribute_id":48,"value":24000.0}]"#)
      .execute(db.writer())
      .await
      .unwrap();

      let capacity = super::hull_capacity(&db, 35832).await.unwrap().unwrap();

      assert_eq!(capacity.power, 1500000.0);
      assert_eq!(capacity.cpu, 24000.0);
    }
  }
}
