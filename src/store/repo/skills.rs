use std::collections::HashMap;

use chrono::Utc;
use sqlx::{QueryBuilder, Sqlite};

use crate::{
  clients::esi::models::universe::DogmaAttribute,
  features::skills::browse::{AttrKey, SkillCatalog, SkillCatalogEntry, SkillCatalogGroup},
  store::{
    Database, Error,
    model::{
      Certificate, CertificateSkill, ItemType, ShipMastery, SkillMetadata, SkillPlan, SkillPlanCertProficiency,
      SkillPlanEntry, SkillPlanRemapPoint, SkillPlanShipMastery, sde_picker_item::PickerItem,
    },
    repo::sde::get_item_type,
  },
};

const MODULE_CATEGORY_ID: i64 = 7;
const SHIP_CATEGORY_ID: i64 = 6;
const SKILL_CATEGORY_ID: i64 = 16;

const PREREQ_ATTR_SLOTS: [(i32, i32); 6] = [
  (182, 277),
  (183, 278),
  (184, 279),
  (1285, 1286),
  (1289, 1287),
  (1290, 1288),
];

const SKILL_REQ_ATTR_SLOTS: [(i32, i32); 5] = [(182, 277), (183, 278), (184, 279), (185, 280), (186, 281)];

pub async fn certificate_all(db: &Database) -> Result<Vec<Certificate>, Error> {
  let rows = sqlx::query_as::<_, Certificate>("SELECT description, grade, id, name FROM certificates ORDER BY id")
    .fetch_all(&db.0)
    .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[cfg_attr(not(test), expect(dead_code))]
pub async fn by_ids(db: &Database, ids: &[i64]) -> Result<Vec<Certificate>, Error> {
  if ids.is_empty() {
    return Ok(Vec::new());
  }

  let mut builder = QueryBuilder::<Sqlite>::new("SELECT description, grade, id, name FROM certificates WHERE id IN (");
  let mut separated = builder.separated(", ");
  for id in ids {
    separated.push_bind(*id);
  }
  separated.push_unseparated(") ORDER BY id");

  let rows = builder.build_query_as::<Certificate>().fetch_all(&db.0).await?;
  Ok(rows)
}

pub async fn skills_for(db: &Database, certificate_id: i64) -> Result<Vec<CertificateSkill>, Error> {
  let rows = sqlx::query_as::<_, CertificateSkill>(
    "SELECT advanced, basic, certificate_id, elite, improved, skill_id \
    FROM certificate_skills WHERE certificate_id = ? ORDER BY skill_id",
  )
  .bind(certificate_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn certificate_upsert_many(
  db: &Database,
  certificates: &[Certificate],
  skills: &[CertificateSkill],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  for certificate in certificates {
    sqlx::query(
      "INSERT INTO certificates (id, name, description, grade) VALUES (?, ?, ?, ?) \
      ON CONFLICT(id) DO UPDATE SET \
        name        = excluded.name, \
        description = excluded.description, \
        grade       = excluded.grade",
    )
    .bind(certificate.id())
    .bind(certificate.name())
    .bind(certificate.description())
    .bind(certificate.grade())
    .execute(&mut *tx)
    .await?;
  }

  for skill in skills {
    sqlx::query(
      "INSERT INTO certificate_skills (certificate_id, skill_id, basic, improved, advanced, elite) \
      VALUES (?, ?, ?, ?, ?, ?) \
      ON CONFLICT(certificate_id, skill_id) DO UPDATE SET \
        basic    = excluded.basic, \
        improved = excluded.improved, \
        advanced = excluded.advanced, \
        elite    = excluded.elite",
    )
    .bind(skill.certificate_id())
    .bind(skill.skill_id())
    .bind(skill.basic())
    .bind(skill.improved())
    .bind(skill.advanced())
    .bind(skill.elite())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[cfg_attr(not(test), expect(dead_code))]
pub async fn mastery_all(db: &Database) -> Result<Vec<ShipMastery>, Error> {
  let rows = sqlx::query_as::<_, ShipMastery>(
    "SELECT certificate_id, ship_type_id, tier FROM ship_masteries ORDER BY ship_type_id, tier, certificate_id",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn for_ship(db: &Database, ship_type_id: i64) -> Result<Vec<ShipMastery>, Error> {
  let rows = sqlx::query_as::<_, ShipMastery>(
    "SELECT certificate_id, ship_type_id, tier FROM ship_masteries WHERE ship_type_id = ? \
    ORDER BY tier, certificate_id",
  )
  .bind(ship_type_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn mastery_upsert_many(db: &Database, masteries: &[ShipMastery]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  for mastery in masteries {
    sqlx::query(
      "INSERT INTO ship_masteries (ship_type_id, tier, certificate_id) VALUES (?, ?, ?) \
      ON CONFLICT(ship_type_id, tier, certificate_id) DO NOTHING",
    )
    .bind(mastery.ship_type_id())
    .bind(mastery.tier())
    .bind(mastery.certificate_id())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn get_skill_metadata(db: &Database, skill_id: i64) -> Result<Option<SkillMetadata>, Error> {
  let row = sqlx::query_as::<_, SkillMetadata>(
    "SELECT primary_attribute, rank, secondary_attribute, skill_id FROM skill_metadata \
    WHERE skill_id = ?",
  )
  .bind(skill_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn upsert_skill_metadata(db: &Database, metadata: &SkillMetadata) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO skill_metadata (primary_attribute, rank, secondary_attribute, skill_id) \
    VALUES (?, ?, ?, ?) \
    ON CONFLICT(skill_id) DO UPDATE SET \
      primary_attribute   = excluded.primary_attribute, \
      rank                = excluded.rank, \
      secondary_attribute = excluded.secondary_attribute",
  )
  .bind(metadata.primary_attribute())
  .bind(metadata.rank())
  .bind(metadata.secondary_attribute())
  .bind(metadata.skill_id())
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn skill_catalog(db: &Database) -> Result<SkillCatalog, Error> {
  let groups = sqlx::query_as::<_, (i64, String)>(
    "SELECT id, name FROM item_groups WHERE category_id = ? AND published = 1 ORDER BY name",
  )
  .bind(SKILL_CATEGORY_ID)
  .fetch_all(&db.0)
  .await?;

  let mut catalog_groups = Vec::with_capacity(groups.len());
  for (group_id, group_name) in groups {
    let skill_types = sqlx::query_as::<_, ItemType>(
      "SELECT capacity, description, dogma_attributes, group_id, icon_id, id, market_group_id, name, \
      packaged_volume, portion_size, published, radius, volume FROM item_types \
      WHERE group_id = ? AND published = 1 ORDER BY name",
    )
    .bind(group_id)
    .fetch_all(&db.0)
    .await?;

    let mut skills = Vec::with_capacity(skill_types.len());
    for item_type in &skill_types {
      skills.push(entry_for_skill(db, item_type, group_id, &group_name).await?);
    }

    catalog_groups.push(SkillCatalogGroup {
      id: group_id,
      name: group_name,
      skills,
    });
  }

  Ok(SkillCatalog {
    groups: catalog_groups,
  })
}

async fn entry_for_skill(
  db: &Database,
  item_type: &ItemType,
  group_id: i64,
  group_name: &str,
) -> Result<SkillCatalogEntry, Error> {
  let metadata = get_skill_metadata(db, item_type.id()).await?;
  let rank = metadata
    .as_ref()
    .map(|m| m.rank())
    .unwrap_or(1)
    .clamp(1, i64::from(u8::MAX)) as u8;
  let primary_attr = AttrKey::from_eve_id(metadata.as_ref().map(|m| m.primary_attribute()).unwrap_or(167) as u8);
  let secondary_attr = AttrKey::from_eve_id(metadata.as_ref().map(|m| m.secondary_attribute()).unwrap_or(166) as u8);

  Ok(SkillCatalogEntry {
    group_id,
    group_name: group_name.to_owned(),
    name: item_type.name().clone(),
    primary_attr,
    prereqs: prereqs_for_skill(db, item_type).await?,
    rank,
    secondary_attr,
    type_id: item_type.id(),
  })
}

fn required_skill_ids_from_slots(item_type: &ItemType, slots: &[(i32, i32)]) -> Vec<(i64, u8)> {
  let dogma: Vec<DogmaAttribute> = serde_json::from_str(item_type.dogma_attributes()).unwrap_or_default();
  let value = |attribute_id: i32| {
    dogma
      .iter()
      .find(|attr| attr.attribute_id == attribute_id)
      .map(|attr| attr.value.round() as i64)
  };

  let mut reqs = Vec::new();
  for &(skill_attr, level_attr) in slots {
    let Some(skill_id) = value(skill_attr).filter(|id| *id != 0) else {
      continue;
    };
    let level = value(level_attr).unwrap_or(0).clamp(0, i64::from(u8::MAX)) as u8;
    reqs.push((skill_id, level));
  }
  reqs
}

pub fn required_skills_for_item(item_type: &ItemType) -> Vec<(i64, u8)> {
  required_skill_ids_from_slots(item_type, &SKILL_REQ_ATTR_SLOTS)
}

async fn prereqs_for_skill(db: &Database, item_type: &ItemType) -> Result<Vec<(String, u8)>, Error> {
  let mut prereqs = Vec::new();
  for (prereq_id, level) in required_skill_ids_from_slots(item_type, &PREREQ_ATTR_SLOTS) {
    let Some(prereq) = get_item_type(db, prereq_id).await? else {
      continue;
    };
    prereqs.push((prereq.name().clone(), level));
  }
  Ok(prereqs)
}

pub async fn ships_for_picker(db: &Database) -> Result<Vec<PickerItem>, Error> {
  let ships = published_types_in_category(db, SHIP_CATEGORY_ID).await?;
  let mut items = Vec::with_capacity(ships.len());
  for (item_type, group_name) in ships {
    let reqs = required_skills_for_item(&item_type);
    let skill_requirements = resolve_skill_names(db, &reqs).await?;
    let mastery_cert_ids = mastery_cert_ids_for_ship(db, item_type.id()).await?;
    items.push(PickerItem {
      id: item_type.id(),
      name: item_type.name().clone(),
      group_name,
      skill_requirements,
      mastery_cert_ids,
    });
  }
  Ok(items)
}

pub async fn modules_for_picker(db: &Database) -> Result<Vec<PickerItem>, Error> {
  let modules = published_types_in_category(db, MODULE_CATEGORY_ID).await?;
  let mut items = Vec::new();
  for (item_type, group_name) in modules {
    let reqs = required_skills_for_item(&item_type);
    if reqs.is_empty() {
      continue;
    }
    let skill_requirements = resolve_skill_names(db, &reqs).await?;
    items.push(PickerItem {
      id: item_type.id(),
      name: item_type.name().clone(),
      group_name,
      skill_requirements,
      mastery_cert_ids: Vec::new(),
    });
  }
  Ok(items)
}

async fn published_types_in_category(db: &Database, category_id: i64) -> Result<Vec<(ItemType, String)>, Error> {
  let group_names: std::collections::HashMap<i64, String> =
    sqlx::query_as::<_, (i64, String)>("SELECT id, name FROM item_groups WHERE category_id = ?")
      .bind(category_id)
      .fetch_all(&db.0)
      .await?
      .into_iter()
      .collect();

  let item_types = sqlx::query_as::<_, ItemType>(
    "SELECT t.capacity, t.description, t.dogma_attributes, t.group_id, t.icon_id, t.id, t.market_group_id, t.name, \
    t.packaged_volume, t.portion_size, t.published, t.radius, t.volume \
    FROM item_types t JOIN item_groups g ON g.id = t.group_id \
    WHERE g.category_id = ? AND t.published = 1 \
    ORDER BY g.name, t.name",
  )
  .bind(category_id)
  .fetch_all(&db.0)
  .await?;

  Ok(
    item_types
      .into_iter()
      .map(|item_type| {
        let group_name = group_names.get(&item_type.group_id()).cloned().unwrap_or_default();
        (item_type, group_name)
      })
      .collect(),
  )
}

async fn resolve_skill_names(db: &Database, reqs: &[(i64, u8)]) -> Result<Vec<(String, u8)>, Error> {
  let mut named = Vec::with_capacity(reqs.len());
  for &(skill_id, level) in reqs {
    if let Some(skill) = get_item_type(db, skill_id).await? {
      named.push((skill.name().clone(), level));
    }
  }
  Ok(named)
}

async fn mastery_cert_ids_for_ship(db: &Database, ship_type_id: i64) -> Result<Vec<Vec<i64>>, Error> {
  let masteries = for_ship(db, ship_type_id).await?;
  let mut tiers: Vec<Vec<i64>> = vec![Vec::new(); 5];
  for mastery in masteries {
    let tier = mastery.tier();
    if (1..=5).contains(&tier) {
      tiers[(tier - 1) as usize].push(mastery.certificate_id());
    }
  }
  Ok(tiers)
}

pub async fn create(db: &Database, character_id: i64, name: &str) -> Result<SkillPlan, Error> {
  let now = Utc::now().to_rfc3339();
  let plan = sqlx::query_as::<_, SkillPlan>(
    "INSERT INTO skill_plans (character_id, created_at, name, updated_at) VALUES (?, ?, ?, ?) \
    RETURNING character_id, created_at, id, implant_set, name, sort_mode, updated_at",
  )
  .bind(character_id)
  .bind(&now)
  .bind(name)
  .bind(&now)
  .fetch_one(&db.0)
  .await?;
  Ok(plan)
}

pub async fn delete(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM skill_plans WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn for_character(db: &Database, character_id: i64) -> Result<Vec<SkillPlan>, Error> {
  let rows = sqlx::query_as::<_, SkillPlan>(
    "SELECT character_id, created_at, id, implant_set, name, sort_mode, updated_at FROM skill_plans \
    WHERE character_id = ? ORDER BY created_at, id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn get(db: &Database, id: i64) -> Result<Option<SkillPlan>, Error> {
  let row = sqlx::query_as::<_, SkillPlan>(
    "SELECT character_id, created_at, id, implant_set, name, sort_mode, updated_at FROM skill_plans WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn update(db: &Database, id: i64, name: &str, sort_mode: &str, implant_set: &str) -> Result<(), Error> {
  let now = Utc::now().to_rfc3339();
  sqlx::query("UPDATE skill_plans SET name = ?, sort_mode = ?, implant_set = ?, updated_at = ? WHERE id = ?")
    .bind(name)
    .bind(sort_mode)
    .bind(implant_set)
    .bind(&now)
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn entries(db: &Database, plan_id: i64) -> Result<Vec<SkillPlanEntry>, Error> {
  let rows = sqlx::query_as::<_, SkillPlanEntry>(
    "SELECT id, is_auto, note, plan_id, position, priority, skill_id, to_level FROM skill_plan_entries \
    WHERE plan_id = ? ORDER BY position",
  )
  .bind(plan_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn insert_entry(db: &Database, plan_id: i64, skill_id: i64, to_level: i64) -> Result<SkillPlanEntry, Error> {
  let entry = sqlx::query_as::<_, SkillPlanEntry>(
    "INSERT INTO skill_plan_entries (plan_id, skill_id, to_level, position) \
    VALUES (?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM skill_plan_entries WHERE plan_id = ?)) \
    RETURNING id, is_auto, note, plan_id, position, priority, skill_id, to_level",
  )
  .bind(plan_id)
  .bind(skill_id)
  .bind(to_level)
  .bind(plan_id)
  .fetch_one(&db.0)
  .await?;
  Ok(entry)
}

pub async fn remove_entry(db: &Database, id: i64) -> Result<(), Error> {
  let Some(plan_id) = sqlx::query_scalar::<_, i64>("SELECT plan_id FROM skill_plan_entries WHERE id = ?")
    .bind(id)
    .fetch_optional(&db.0)
    .await?
  else {
    return Ok(());
  };

  reanchor_remap_points(db, plan_id, id).await?;

  let mut tx = db.writer().begin().await?;
  sqlx::query("DELETE FROM skill_plan_entries WHERE id = ?")
    .bind(id)
    .execute(&mut *tx)
    .await?;
  densify(&mut tx, plan_id).await?;
  tx.commit().await?;
  Ok(())
}

pub async fn reorder_entries(db: &Database, ordered_ids: &[i64]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;
  for (position, id) in ordered_ids.iter().enumerate() {
    sqlx::query("UPDATE skill_plan_entries SET position = ? WHERE id = ?")
      .bind(position as i64)
      .bind(id)
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}

pub async fn replace_entries(
  db: &Database,
  plan_id: i64,
  entries: &[(i64, i64, &str, &str, i64)],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;
  sqlx::query("DELETE FROM skill_plan_entries WHERE plan_id = ?")
    .bind(plan_id)
    .execute(&mut *tx)
    .await?;
  for (position, (skill_id, to_level, priority, note, is_auto)) in entries.iter().enumerate() {
    sqlx::query(
      "INSERT INTO skill_plan_entries (plan_id, skill_id, to_level, position, priority, note, is_auto) \
      VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(plan_id)
    .bind(skill_id)
    .bind(to_level)
    .bind(position as i64)
    .bind(priority)
    .bind(note)
    .bind(is_auto)
    .execute(&mut *tx)
    .await?;
  }
  tx.commit().await?;
  Ok(())
}

async fn densify(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, plan_id: i64) -> Result<(), Error> {
  let ids = sqlx::query_scalar::<_, i64>("SELECT id FROM skill_plan_entries WHERE plan_id = ? ORDER BY position, id")
    .bind(plan_id)
    .fetch_all(&mut **tx)
    .await?;
  for (position, id) in ids.into_iter().enumerate() {
    sqlx::query("UPDATE skill_plan_entries SET position = ? WHERE id = ?")
      .bind(position as i64)
      .bind(id)
      .execute(&mut **tx)
      .await?;
  }
  Ok(())
}

pub async fn remap_points(db: &Database, plan_id: i64) -> Result<Vec<SkillPlanRemapPoint>, Error> {
  let rows = sqlx::query_as::<_, SkillPlanRemapPoint>(
    "SELECT after_entry_id, base_charisma, base_intelligence, base_memory, base_perception, base_willpower, id, \
    plan_id FROM skill_plan_remap_points WHERE plan_id = ? ORDER BY after_entry_id IS NOT NULL, after_entry_id",
  )
  .bind(plan_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Arguments map directly to the persisted remap-point columns; bundling them into a struct would only move the fields.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_remap_point(
  db: &Database,
  plan_id: i64,
  after_entry_id: Option<i64>,
  base_perception: i64,
  base_memory: i64,
  base_willpower: i64,
  base_intelligence: i64,
  base_charisma: i64,
) -> Result<SkillPlanRemapPoint, Error> {
  let mut tx = db.writer().begin().await?;
  delete_slot(&mut tx, plan_id, after_entry_id).await?;
  let remap = sqlx::query_as::<_, SkillPlanRemapPoint>(
    "INSERT INTO skill_plan_remap_points \
      (plan_id, after_entry_id, base_perception, base_memory, base_willpower, base_intelligence, base_charisma) \
    VALUES (?, ?, ?, ?, ?, ?, ?) \
    RETURNING after_entry_id, base_charisma, base_intelligence, base_memory, base_perception, base_willpower, id, \
    plan_id",
  )
  .bind(plan_id)
  .bind(after_entry_id)
  .bind(base_perception)
  .bind(base_memory)
  .bind(base_willpower)
  .bind(base_intelligence)
  .bind(base_charisma)
  .fetch_one(&mut *tx)
  .await?;
  tx.commit().await?;
  Ok(remap)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[cfg_attr(not(test), expect(dead_code))]
pub async fn remove_remap_point(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM skill_plan_remap_points WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn reanchor_remap_points(db: &Database, plan_id: i64, entry_id: i64) -> Result<(), Error> {
  let dependents =
    sqlx::query_scalar::<_, i64>("SELECT id FROM skill_plan_remap_points WHERE plan_id = ? AND after_entry_id = ?")
      .bind(plan_id)
      .bind(entry_id)
      .fetch_all(&db.0)
      .await?;
  if dependents.is_empty() {
    return Ok(());
  }

  let predecessor = sqlx::query_scalar::<_, i64>(
    "SELECT id FROM skill_plan_entries WHERE plan_id = ? \
      AND position < (SELECT position FROM skill_plan_entries WHERE id = ?) \
      ORDER BY position DESC LIMIT 1",
  )
  .bind(plan_id)
  .bind(entry_id)
  .fetch_optional(&db.0)
  .await?;

  let mut tx = db.writer().begin().await?;
  let occupied = slot_is_occupied(&mut tx, plan_id, predecessor).await?;
  for remap_id in dependents {
    if occupied {
      sqlx::query("DELETE FROM skill_plan_remap_points WHERE id = ?")
        .bind(remap_id)
        .execute(&mut *tx)
        .await?;
    } else {
      sqlx::query("UPDATE skill_plan_remap_points SET after_entry_id = ? WHERE id = ?")
        .bind(predecessor)
        .bind(remap_id)
        .execute(&mut *tx)
        .await?;
    }
  }
  tx.commit().await?;
  Ok(())
}

async fn delete_slot(
  tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
  plan_id: i64,
  after_entry_id: Option<i64>,
) -> Result<(), Error> {
  match after_entry_id {
    Some(entry_id) => {
      sqlx::query("DELETE FROM skill_plan_remap_points WHERE plan_id = ? AND after_entry_id = ?")
        .bind(plan_id)
        .bind(entry_id)
        .execute(&mut **tx)
        .await?;
    }
    None => {
      sqlx::query("DELETE FROM skill_plan_remap_points WHERE plan_id = ? AND after_entry_id IS NULL")
        .bind(plan_id)
        .execute(&mut **tx)
        .await?;
    }
  }
  Ok(())
}

pub async fn ship_masteries(db: &Database, plan_id: i64) -> Result<Vec<SkillPlanShipMastery>, Error> {
  let rows = sqlx::query_as::<_, SkillPlanShipMastery>(
    "SELECT plan_id, ship_type_id, tier FROM skill_plan_ship_masteries WHERE plan_id = ? ORDER BY ship_type_id",
  )
  .bind(plan_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn replace_ship_masteries(db: &Database, plan_id: i64, masteries: &[(i64, i64)]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;
  sqlx::query("DELETE FROM skill_plan_ship_masteries WHERE plan_id = ?")
    .bind(plan_id)
    .execute(&mut *tx)
    .await?;
  for (ship_type_id, tier) in masteries {
    sqlx::query("INSERT INTO skill_plan_ship_masteries (plan_id, ship_type_id, tier) VALUES (?, ?, ?)")
      .bind(plan_id)
      .bind(ship_type_id)
      .bind(tier)
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}

pub async fn cert_proficiencies(db: &Database, plan_id: i64) -> Result<Vec<SkillPlanCertProficiency>, Error> {
  let rows = sqlx::query_as::<_, SkillPlanCertProficiency>(
    "SELECT plan_id, cert_id, level FROM skill_plan_cert_proficiencies WHERE plan_id = ? ORDER BY cert_id",
  )
  .bind(plan_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn replace_cert_proficiencies(
  db: &Database,
  plan_id: i64,
  proficiencies: &[(i64, i64)],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;
  sqlx::query("DELETE FROM skill_plan_cert_proficiencies WHERE plan_id = ?")
    .bind(plan_id)
    .execute(&mut *tx)
    .await?;
  for (cert_id, level) in proficiencies {
    sqlx::query("INSERT INTO skill_plan_cert_proficiencies (plan_id, cert_id, level) VALUES (?, ?, ?)")
      .bind(plan_id)
      .bind(cert_id)
      .bind(level)
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}

async fn slot_is_occupied(
  tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
  plan_id: i64,
  after_entry_id: Option<i64>,
) -> Result<bool, Error> {
  let count = match after_entry_id {
    Some(entry_id) => {
      sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM skill_plan_remap_points WHERE plan_id = ? AND after_entry_id = ?",
      )
      .bind(plan_id)
      .bind(entry_id)
      .fetch_one(&mut **tx)
      .await?
    }
    None => {
      sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM skill_plan_remap_points WHERE plan_id = ? AND after_entry_id IS NULL",
      )
      .bind(plan_id)
      .fetch_one(&mut **tx)
      .await?
    }
  };
  Ok(count > 0)
}

fn level_at_proficiency(skill: &CertificateSkill, prof_idx: usize) -> u8 {
  let level = match prof_idx {
    0 => skill.basic(),
    1 => skill.improved(),
    2 => skill.advanced(),
    _ => skill.elite(),
  };
  level.clamp(0, i64::from(u8::MAX)) as u8
}

fn merge_highest(by_skill: &mut HashMap<i64, u8>, skill_id: i64, level: u8) {
  let entry = by_skill.entry(skill_id).or_insert(0);
  if level > *entry {
    *entry = level;
  }
}

pub fn skills_for_cert_at_proficiency(cert_skills: &[CertificateSkill], prof_idx: usize) -> Vec<(i64, u8)> {
  cert_skills
    .iter()
    .filter_map(|skill| {
      let level = level_at_proficiency(skill, prof_idx);
      (level > 0).then_some((skill.skill_id(), level))
    })
    .collect()
}

pub fn skills_for_mastery(tier_cert_skills: &[Vec<CertificateSkill>], mastery_level: u8) -> Vec<(i64, u8)> {
  let mut by_skill: HashMap<i64, u8> = HashMap::new();
  let tier_count = (mastery_level.min(5) as usize).min(tier_cert_skills.len());

  for (tier_idx, cert_skills) in tier_cert_skills.iter().take(tier_count).enumerate() {
    let prof_idx = tier_idx.min(3);
    for (skill_id, level) in skills_for_cert_at_proficiency(cert_skills, prof_idx) {
      merge_highest(&mut by_skill, skill_id, level);
    }
  }

  by_skill.into_iter().collect()
}

pub fn skills_for_module(requirements: &[(i64, u8)]) -> Vec<(i64, u8)> {
  let mut by_skill: HashMap<i64, u8> = HashMap::new();
  for &(skill_id, level) in requirements {
    merge_highest(&mut by_skill, skill_id, level);
  }
  by_skill.into_iter().collect()
}

#[cfg(test)]
mod certificate_tests {
  use super::*;
  use crate::store::{
    self,
    model::{ItemCategory, ItemGroup, ItemType},
    repo::sde,
  };

  async fn seed_skill_type(db: &Database, skill_id: i64) {
    sde::upsert_item_category(
      db,
      &ItemCategory {
        id: 16,
        icon_id: None,
        name: "Skill".to_owned(),
        published: true,
      },
    )
    .await
    .unwrap();
    sde::upsert_item_group(
      db,
      &ItemGroup {
        category_id: 16,
        icon_id: None,
        id: 255,
        name: "Gunnery".to_owned(),
        published: true,
      },
    )
    .await
    .unwrap();
    sde::upsert_item_type(
      db,
      &ItemType {
        capacity: None,
        description: Some("A skill.".to_owned()),
        dogma_attributes: "[]".to_owned(),
        group_id: 255,
        icon_id: None,
        id: skill_id,
        market_group_id: None,
        name: "Gunnery".to_owned(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        volume: None,
      },
    )
    .await
    .unwrap();
  }

  fn make_certificate(id: i64, name: &str, grade: i64) -> Certificate {
    Certificate {
      description: Some(format!("{name} certificate")),
      grade,
      id,
      name: name.to_owned(),
    }
  }

  fn make_cert_skill(certificate_id: i64, skill_id: i64) -> CertificateSkill {
    CertificateSkill {
      advanced: 4,
      basic: 1,
      certificate_id,
      elite: 5,
      improved: 3,
      skill_id,
    }
  }

  mod all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_certificates_exist() {
      let db = store::open_test().await.unwrap();

      assert_eq!(certificate_all(&db).await.unwrap(), vec![]);
    }
  }

  mod by_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_for_no_ids() {
      let db = store::open_test().await.unwrap();

      assert_eq!(by_ids(&db, &[]).await.unwrap(), vec![]);
    }

    #[tokio::test]
    async fn it_returns_only_the_requested_certificates() {
      let db = store::open_test().await.unwrap();
      certificate_upsert_many(
        &db,
        &[
          make_certificate(1, "A", 1),
          make_certificate(2, "B", 2),
          make_certificate(3, "C", 3),
        ],
        &[],
      )
      .await
      .unwrap();

      let result = by_ids(&db, &[1, 3]).await.unwrap();

      assert_eq!(result.len(), 2);
      assert_eq!(result[0].id(), 1);
      assert_eq!(result[1].id(), 3);
    }
  }

  mod upsert_many {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_overwrites_existing_rows_on_conflict() {
      let db = store::open_test().await.unwrap();
      seed_skill_type(&db, 3300).await;
      certificate_upsert_many(&db, &[make_certificate(1, "Old Name", 1)], &[make_cert_skill(1, 3300)])
        .await
        .unwrap();

      let mut updated_skill = make_cert_skill(1, 3300);
      updated_skill.basic = 2;
      certificate_upsert_many(&db, &[make_certificate(1, "New Name", 3)], &[updated_skill])
        .await
        .unwrap();

      let cert = by_ids(&db, &[1]).await.unwrap().pop().unwrap();
      assert_eq!(cert.name(), "New Name");
      assert_eq!(cert.grade(), 3);

      let cert_skills = skills_for(&db, 1).await.unwrap();
      assert_eq!(cert_skills.len(), 1);
      assert_eq!(cert_skills[0].basic(), 2);
    }

    #[tokio::test]
    async fn it_round_trips_a_certificate_with_required_skills() {
      let db = store::open_test().await.unwrap();
      seed_skill_type(&db, 3300).await;

      certificate_upsert_many(
        &db,
        &[make_certificate(1, "Gunnery Basics", 1)],
        &[make_cert_skill(1, 3300)],
      )
      .await
      .unwrap();

      let certs = certificate_all(&db).await.unwrap();
      assert_eq!(certs.len(), 1);
      assert_eq!(certs[0].id(), 1);
      assert_eq!(certs[0].grade(), 1);

      let cert_skills = skills_for(&db, 1).await.unwrap();
      assert_eq!(cert_skills.len(), 1);
      assert_eq!(cert_skills[0].skill_id(), 3300);
      assert_eq!(cert_skills[0].basic(), 1);
      assert_eq!(cert_skills[0].elite(), 5);
    }
  }
}

#[cfg(test)]
mod mastery_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Certificate, ItemCategory, ItemGroup, ItemType},
    repo::sde,
  };

  async fn seed_ship_type(db: &Database, ship_type_id: i64) {
    sde::upsert_item_category(
      db,
      &ItemCategory {
        id: 6,
        icon_id: None,
        name: "Ship".to_owned(),
        published: true,
      },
    )
    .await
    .unwrap();
    sde::upsert_item_group(
      db,
      &ItemGroup {
        category_id: 6,
        icon_id: None,
        id: 25,
        name: "Frigate".to_owned(),
        published: true,
      },
    )
    .await
    .unwrap();
    sde::upsert_item_type(
      db,
      &ItemType {
        capacity: None,
        description: Some("A ship.".to_owned()),
        dogma_attributes: "[]".to_owned(),
        group_id: 25,
        icon_id: None,
        id: ship_type_id,
        market_group_id: None,
        name: "Rifter".to_owned(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        volume: None,
      },
    )
    .await
    .unwrap();
  }

  async fn seed_certificate(db: &Database, id: i64) {
    certificate_upsert_many(
      db,
      &[Certificate {
        description: None,
        grade: 1,
        id,
        name: "Cert".to_owned(),
      }],
      &[],
    )
    .await
    .unwrap();
  }

  mod all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_masteries_exist() {
      let db = store::open_test().await.unwrap();

      assert_eq!(mastery_all(&db).await.unwrap(), vec![]);
    }
  }

  mod upsert_many {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_idempotent_on_the_composite_key() {
      let db = store::open_test().await.unwrap();
      seed_ship_type(&db, 587).await;
      seed_certificate(&db, 1).await;
      let row = ShipMastery {
        certificate_id: 1,
        ship_type_id: 587,
        tier: 2,
      };

      mastery_upsert_many(&db, &[row]).await.unwrap();
      mastery_upsert_many(&db, &[row]).await.unwrap();

      assert_eq!(mastery_all(&db).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_round_trips_a_ship_mastery() {
      let db = store::open_test().await.unwrap();
      seed_ship_type(&db, 587).await;
      seed_certificate(&db, 1).await;

      mastery_upsert_many(
        &db,
        &[ShipMastery {
          certificate_id: 1,
          ship_type_id: 587,
          tier: 1,
        }],
      )
      .await
      .unwrap();

      let rows = for_ship(&db, 587).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].ship_type_id(), 587);
      assert_eq!(rows[0].tier(), 1);
      assert_eq!(rows[0].certificate_id(), 1);
    }
  }
}

#[cfg(test)]
mod metadata_tests {
  mod skill_catalog_tests {
    use super::super::*;
    use crate::store::{
      self,
      model::{ItemCategory, ItemGroup},
      repo::sde::{upsert_item_category, upsert_item_group, upsert_item_type},
    };

    async fn seed_skill(db: &Database, group_id: i64, group_name: &str, skill_id: i64, name: &str, published: bool) {
      seed_skill_with_dogma(db, group_id, group_name, skill_id, name, published, "[]").await;
    }

    async fn seed_skill_with_dogma(
      db: &Database,
      group_id: i64,
      group_name: &str,
      skill_id: i64,
      name: &str,
      published: bool,
      dogma_attributes: &str,
    ) {
      upsert_item_group(
        db,
        &ItemGroup {
          category_id: SKILL_CATEGORY_ID,
          icon_id: None,
          id: group_id,
          name: group_name.to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
      upsert_item_type(
        db,
        &ItemType {
          capacity: None,
          description: Some("A skill.".to_owned()),
          dogma_attributes: dogma_attributes.to_owned(),
          group_id,
          icon_id: None,
          id: skill_id,
          market_group_id: None,
          name: name.to_owned(),
          packaged_volume: None,
          portion_size: None,
          published,
          radius: None,
          volume: None,
        },
      )
      .await
      .unwrap();
    }

    async fn seed_skill_category(db: &Database) {
      upsert_item_category(
        db,
        &ItemCategory {
          id: SKILL_CATEGORY_ID,
          icon_id: None,
          name: "Skill".to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
    }

    mod all {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_defaults_rank_to_one_when_metadata_is_absent() {
        let db = store::open_test().await.unwrap();
        seed_skill_category(&db).await;
        seed_skill(&db, 255, "Gunnery", 3300, "Gunnery", true).await;

        let catalog = skill_catalog(&db).await.unwrap();

        assert_eq!(catalog.groups[0].skills[0].rank, 1);
      }

      #[tokio::test]
      async fn it_excludes_unpublished_skills() {
        let db = store::open_test().await.unwrap();
        seed_skill_category(&db).await;
        seed_skill(&db, 255, "Gunnery", 3300, "Gunnery", true).await;
        seed_skill(&db, 255, "Gunnery", 3301, "Removed Skill", false).await;

        let catalog = skill_catalog(&db).await.unwrap();

        assert_eq!(catalog.groups[0].skills.len(), 1);
        assert_eq!(catalog.groups[0].skills[0].name, "Gunnery");
      }

      #[tokio::test]
      async fn it_groups_published_skills_by_item_group_sorted_by_name() {
        let db = store::open_test().await.unwrap();
        seed_skill_category(&db).await;
        seed_skill(&db, 257, "Spaceship Command", 3327, "Spaceship Command", true).await;
        seed_skill(&db, 255, "Gunnery", 3300, "Gunnery", true).await;

        let catalog = skill_catalog(&db).await.unwrap();

        assert_eq!(catalog.groups.len(), 2);
        assert_eq!(catalog.groups[0].name, "Gunnery");
        assert_eq!(catalog.groups[1].name, "Spaceship Command");
      }

      #[tokio::test]
      async fn it_leaves_prereqs_empty_when_no_prereq_dogma_attributes_are_present() {
        let db = store::open_test().await.unwrap();
        seed_skill_category(&db).await;
        seed_skill(&db, 255, "Gunnery", 3300, "Gunnery", true).await;

        let catalog = skill_catalog(&db).await.unwrap();

        assert!(catalog.groups[0].skills[0].prereqs.is_empty());
      }

      #[tokio::test]
      async fn it_reads_rank_and_attrs_from_skill_metadata_when_present() {
        let db = store::open_test().await.unwrap();
        seed_skill_category(&db).await;
        seed_skill(&db, 255, "Gunnery", 3300, "Gunnery", true).await;
        upsert_skill_metadata(
          &db,
          &SkillMetadata {
            primary_attribute: 167,
            rank: 3,
            secondary_attribute: 166,
            skill_id: 3300,
          },
        )
        .await
        .unwrap();

        let catalog = skill_catalog(&db).await.unwrap();

        let skill = &catalog.groups[0].skills[0];
        assert_eq!(skill.rank, 3);
        assert_eq!(skill.primary_attr, AttrKey::Perception);
        assert_eq!(skill.secondary_attr, AttrKey::Memory);
        assert!(skill.prereqs.is_empty());
      }

      #[tokio::test]
      async fn it_resolves_direct_prereqs_from_dogma_attributes_to_names_and_levels() {
        let db = store::open_test().await.unwrap();
        seed_skill_category(&db).await;
        seed_skill(&db, 256, "Spaceship Command", 3327, "Spaceship Command", true).await;
        seed_skill_with_dogma(
          &db,
          255,
          "Gunnery",
          3300,
          "Small Hybrid Turret",
          true,
          r#"[{"attribute_id":182,"value":3327.0},{"attribute_id":277,"value":3.0}]"#,
        )
        .await;

        let catalog = skill_catalog(&db).await.unwrap();

        let skill = catalog
          .groups
          .iter()
          .flat_map(|g| &g.skills)
          .find(|s| s.name == "Small Hybrid Turret")
          .unwrap();
        assert_eq!(skill.prereqs, vec![("Spaceship Command".to_owned(), 3)]);
      }

      #[tokio::test]
      async fn it_resolves_multiple_prereq_slots_in_order() {
        let db = store::open_test().await.unwrap();
        seed_skill_category(&db).await;
        seed_skill(&db, 256, "Spaceship Command", 3327, "Spaceship Command", true).await;
        seed_skill(&db, 256, "Spaceship Command", 3328, "Gallente Frigate", true).await;
        seed_skill_with_dogma(
          &db,
          255,
          "Gunnery",
          3300,
          "Atron",
          true,
          r#"[
            {"attribute_id":182,"value":3327.0},{"attribute_id":277,"value":1.0},
            {"attribute_id":1285,"value":3328.0},{"attribute_id":1286,"value":2.0}
          ]"#,
        )
        .await;

        let catalog = skill_catalog(&db).await.unwrap();

        let skill = catalog
          .groups
          .iter()
          .flat_map(|g| &g.skills)
          .find(|s| s.name == "Atron")
          .unwrap();
        assert_eq!(
          skill.prereqs,
          vec![("Spaceship Command".to_owned(), 1), ("Gallente Frigate".to_owned(), 2)]
        );
      }

      #[tokio::test]
      async fn it_returns_an_empty_catalog_when_no_skill_groups_exist() {
        let db = store::open_test().await.unwrap();

        let catalog = skill_catalog(&db).await.unwrap();

        assert!(catalog.groups.is_empty());
      }

      #[tokio::test]
      async fn it_skips_prereq_slots_whose_skill_id_is_zero() {
        let db = store::open_test().await.unwrap();
        seed_skill_category(&db).await;
        seed_skill_with_dogma(
          &db,
          255,
          "Gunnery",
          3300,
          "Gunnery",
          true,
          r#"[{"attribute_id":182,"value":0.0},{"attribute_id":277,"value":0.0}]"#,
        )
        .await;

        let catalog = skill_catalog(&db).await.unwrap();

        assert!(catalog.groups[0].skills[0].prereqs.is_empty());
      }
    }
  }

  mod skill_metadata_tests {
    use super::super::*;
    use crate::store::{
      self,
      model::{ItemCategory, ItemGroup},
      repo::sde::{upsert_item_category, upsert_item_group, upsert_item_type},
    };

    async fn seed_skill_type(db: &Database, skill_id: i64) {
      upsert_item_category(
        db,
        &ItemCategory {
          id: 16,
          icon_id: None,
          name: "Skill".to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
      upsert_item_group(
        db,
        &ItemGroup {
          category_id: 16,
          icon_id: None,
          id: 255,
          name: "Gunnery".to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
      upsert_item_type(
        db,
        &ItemType {
          capacity: None,
          description: Some("A skill.".to_owned()),
          dogma_attributes: "[]".to_owned(),
          group_id: 255,
          icon_id: None,
          id: skill_id,
          market_group_id: None,
          name: "Gunnery".to_owned(),
          packaged_volume: None,
          portion_size: None,
          published: true,
          radius: None,
          volume: None,
        },
      )
      .await
      .unwrap();
    }

    fn make_metadata(skill_id: i64, rank: i64) -> SkillMetadata {
      SkillMetadata {
        primary_attribute: 167,
        rank,
        secondary_attribute: 166,
        skill_id,
      }
    }

    mod get {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_returns_none_when_no_row_exists() {
        let db = store::open_test().await.unwrap();

        assert_eq!(get_skill_metadata(&db, 3300).await.unwrap(), None);
      }

      #[tokio::test]
      async fn it_round_trips_an_upserted_row() {
        let db = store::open_test().await.unwrap();
        seed_skill_type(&db, 3300).await;
        upsert_skill_metadata(&db, &make_metadata(3300, 1)).await.unwrap();

        let result = get_skill_metadata(&db, 3300).await.unwrap().unwrap();

        assert_eq!(result.skill_id(), 3300);
        assert_eq!(result.rank(), 1);
        assert_eq!(result.primary_attribute(), 167);
        assert_eq!(result.secondary_attribute(), 166);
      }
    }

    mod upsert {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_overwrites_the_existing_row() {
        let db = store::open_test().await.unwrap();
        seed_skill_type(&db, 3300).await;
        upsert_skill_metadata(&db, &make_metadata(3300, 1)).await.unwrap();

        upsert_skill_metadata(&db, &make_metadata(3300, 3)).await.unwrap();

        let result = get_skill_metadata(&db, 3300).await.unwrap().unwrap();
        assert_eq!(result.rank(), 3);
      }
    }
  }

  mod skill_picker_tests {
    use super::super::*;
    use crate::store::{
      self,
      model::{ItemCategory, ItemGroup},
      repo::sde::{upsert_item_category, upsert_item_group, upsert_item_type},
    };

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

    fn item_type_with_dogma(id: i64, group_id: i64, name: &str, dogma: &[(i32, f64)]) -> ItemType {
      let attrs: Vec<_> = dogma
        .iter()
        .map(|&(attribute_id, value)| DogmaAttribute {
          attribute_id,
          value,
        })
        .collect();
      let mut item = make_item_type(id, group_id, name);
      item.dogma_attributes = serde_json::to_string(&attrs).unwrap();
      item
    }

    mod modules_for_picker {
      use pretty_assertions::assert_eq;

      use super::*;

      async fn seed_skill_name(db: &Database, id: i64, name: &str) {
        upsert_item_category(db, &make_category(16, "Skill")).await.unwrap();
        upsert_item_group(db, &make_group(900, 16, "Gunnery")).await.unwrap();
        upsert_item_type(db, &make_item_type(id, 900, name)).await.unwrap();
      }

      #[tokio::test]
      async fn it_lists_modules_with_resolved_requirements_and_skips_skill_free_ones() {
        let db = store::open_test().await.unwrap();
        seed_skill_name(&db, 3300, "Gunnery").await;

        upsert_item_category(&db, &make_category(MODULE_CATEGORY_ID, "Module"))
          .await
          .unwrap();
        upsert_item_group(&db, &make_group(74, MODULE_CATEGORY_ID, "Energy Weapon"))
          .await
          .unwrap();

        let with_skill = item_type_with_dogma(2929, 74, "Gatling Pulse Laser", &[(182, 3300.0), (277, 1.0)]);
        let without_skill = make_item_type(2930, 74, "Civilian Laser");
        upsert_item_type(&db, &with_skill).await.unwrap();
        upsert_item_type(&db, &without_skill).await.unwrap();

        let modules = modules_for_picker(&db).await.unwrap();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].id, 2929);
        assert_eq!(modules[0].group_name, "Energy Weapon");
        assert_eq!(modules[0].skill_requirements, vec![("Gunnery".to_string(), 1)]);
        assert!(modules[0].mastery_cert_ids.is_empty());
      }
    }

    mod required_skills_for_item {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_parses_the_five_required_skill_slots() {
        let item = item_type_with_dogma(
          100,
          25,
          "Rifter",
          &[
            (182, 3330.0),
            (277, 1.0),
            (183, 3300.0),
            (278, 3.0),
            (186, 3301.0),
            (281, 5.0),
          ],
        );

        let mut reqs = required_skills_for_item(&item);
        reqs.sort();
        assert_eq!(reqs, vec![(3300, 3), (3301, 5), (3330, 1)]);
      }

      #[test]
      fn it_returns_empty_for_an_item_with_no_dogma() {
        let item = make_item_type(102, 25, "Plain");
        assert_eq!(required_skills_for_item(&item), vec![]);
      }

      #[test]
      fn it_skips_absent_and_zero_skill_slots() {
        let item = item_type_with_dogma(101, 25, "Empty", &[(277, 4.0), (183, 0.0), (278, 4.0)]);
        assert_eq!(required_skills_for_item(&item), vec![]);
      }
    }

    mod ships_for_picker {
      use pretty_assertions::assert_eq;

      use super::*;
      use crate::store::model::{Certificate, CertificateSkill, ShipMastery};

      #[tokio::test]
      async fn it_lists_ships_grouped_with_mastery_cert_ids_per_tier() {
        let db = store::open_test().await.unwrap();

        upsert_item_category(&db, &make_category(16, "Skill")).await.unwrap();
        upsert_item_group(&db, &make_group(255, 16, "Spaceship Command"))
          .await
          .unwrap();
        upsert_item_type(&db, &make_item_type(3331, 255, "Minmatar Frigate"))
          .await
          .unwrap();

        upsert_item_category(&db, &make_category(SHIP_CATEGORY_ID, "Ship"))
          .await
          .unwrap();
        upsert_item_group(&db, &make_group(25, SHIP_CATEGORY_ID, "Frigate"))
          .await
          .unwrap();
        let rifter = item_type_with_dogma(587, 25, "Rifter", &[(182, 3331.0), (277, 1.0)]);
        upsert_item_type(&db, &rifter).await.unwrap();

        certificate_upsert_many(
          &db,
          &[
            Certificate {
              description: None,
              grade: 1,
              id: 10,
              name: "Cert I".to_string(),
            },
            Certificate {
              description: None,
              grade: 3,
              id: 30,
              name: "Cert III".to_string(),
            },
          ],
          &[
            CertificateSkill {
              advanced: 3,
              basic: 1,
              certificate_id: 10,
              elite: 5,
              improved: 2,
              skill_id: 3331,
            },
            CertificateSkill {
              advanced: 3,
              basic: 1,
              certificate_id: 30,
              elite: 5,
              improved: 2,
              skill_id: 3331,
            },
          ],
        )
        .await
        .unwrap();
        mastery_upsert_many(
          &db,
          &[
            ShipMastery {
              certificate_id: 10,
              ship_type_id: 587,
              tier: 1,
            },
            ShipMastery {
              certificate_id: 30,
              ship_type_id: 587,
              tier: 3,
            },
          ],
        )
        .await
        .unwrap();

        let ships = ships_for_picker(&db).await.unwrap();

        assert_eq!(ships.len(), 1);
        assert_eq!(ships[0].id, 587);
        assert_eq!(ships[0].group_name, "Frigate");
        assert_eq!(ships[0].skill_requirements, vec![("Minmatar Frigate".to_string(), 1)]);
        assert_eq!(ships[0].mastery_cert_ids.len(), 5);
        assert_eq!(ships[0].mastery_cert_ids[0], vec![10]);
        assert_eq!(ships[0].mastery_cert_ids[2], vec![30]);
        assert!(ships[0].mastery_cert_ids[1].is_empty());
        assert!(ships[0].mastery_cert_ids[4].is_empty());
      }
    }
  }
}

#[cfg(test)]
mod plans_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
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

  async fn seed_plan(db: &Database) -> i64 {
    seed_character(db, 42).await;
    create(db, 42, "Plan").await.unwrap().id()
  }

  async fn upsert_after(db: &Database, plan_id: i64, after: Option<i64>) -> SkillPlanRemapPoint {
    upsert_remap_point(db, plan_id, after, 17, 27, 17, 21, 17)
      .await
      .unwrap()
  }

  mod cert_proficiencies {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_replaces_and_reads_back_proficiencies() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;

      replace_cert_proficiencies(&db, plan_id, &[(1, 2), (2, 3)])
        .await
        .unwrap();
      let rows = cert_proficiencies(&db, plan_id).await.unwrap();
      assert_eq!(
        rows.iter().map(|r| (r.cert_id(), r.level())).collect::<Vec<_>>(),
        [(1, 2), (2, 3)]
      );

      replace_cert_proficiencies(&db, plan_id, &[]).await.unwrap();
      assert!(cert_proficiencies(&db, plan_id).await.unwrap().is_empty());
    }
  }

  mod create {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_defaults_sort_mode_and_implant_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let plan = create(&db, 42, "Combat").await.unwrap();

      assert_eq!(plan.character_id(), 42);
      assert_eq!(plan.name(), "Combat");
      assert_eq!(plan.sort_mode(), "manual");
      assert_eq!(plan.implant_set(), "current");
      assert_eq!(plan.created_at(), plan.updated_at());
    }
  }

  mod entries {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_entries_in_position_order() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;
      let a = insert_entry(&db, plan_id, 3300, 5).await.unwrap();
      let b = insert_entry(&db, plan_id, 3301, 5).await.unwrap();
      let c = insert_entry(&db, plan_id, 3302, 5).await.unwrap();

      reorder_entries(&db, &[c.id(), a.id(), b.id()]).await.unwrap();

      assert_eq!(
        entries(&db, plan_id)
          .await
          .unwrap()
          .iter()
          .map(|e| e.skill_id())
          .collect::<Vec<_>>(),
        [3302, 3300, 3301]
      );
    }
  }

  mod for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_only_returns_the_given_characters_plans() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mine = create(&db, 42, "Mine").await.unwrap();

      assert_eq!(
        for_character(&db, 42)
          .await
          .unwrap()
          .iter()
          .map(|p| p.id())
          .collect::<Vec<_>>(),
        [mine.id()]
      );
      assert!(for_character(&db, 99).await.unwrap().is_empty());
    }
  }

  mod insert_entry {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_appends_with_dense_positions() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;

      let first = insert_entry(&db, plan_id, 3300, 5).await.unwrap();
      let second = insert_entry(&db, plan_id, 3301, 4).await.unwrap();

      assert_eq!(first.position(), 0);
      assert_eq!(second.position(), 1);
      assert_eq!(first.skill_id(), 3300);
      assert_eq!(first.to_level(), 5);
    }
  }

  mod reanchor_on_delete {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn deleting_a_middle_entry_repoints_its_remap_to_the_predecessor() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;
      let a = insert_entry(&db, plan_id, 3300, 5).await.unwrap();
      let b = insert_entry(&db, plan_id, 3301, 5).await.unwrap();
      let c = insert_entry(&db, plan_id, 3302, 5).await.unwrap();
      let remap = upsert_after(&db, plan_id, Some(b.id())).await;

      remove_entry(&db, b.id()).await.unwrap();

      let remaps = remap_points(&db, plan_id).await.unwrap();
      assert_eq!(remaps.len(), 1, "remap must survive with no orphan");
      assert_eq!(remaps[0].id(), remap.id());
      assert_eq!(remaps[0].after_entry_id(), Some(a.id()), "re-anchored to predecessor");
      assert!(c.id() > 0);
    }

    #[tokio::test]
    async fn deleting_the_first_entry_moves_its_remap_to_the_start_bucket() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;
      let a = insert_entry(&db, plan_id, 3300, 5).await.unwrap();
      let b = insert_entry(&db, plan_id, 3301, 5).await.unwrap();
      let remap = upsert_after(&db, plan_id, Some(a.id())).await;

      remove_entry(&db, a.id()).await.unwrap();

      let remaps = remap_points(&db, plan_id).await.unwrap();
      assert_eq!(remaps.len(), 1);
      assert_eq!(remaps[0].id(), remap.id());
      assert_eq!(remaps[0].after_entry_id(), None, "re-anchored to the __start bucket");
      assert!(b.id() > 0);
    }

    #[tokio::test]
    async fn it_leaves_no_orphan_rows_after_a_delete() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;
      let a = insert_entry(&db, plan_id, 3300, 5).await.unwrap();
      upsert_after(&db, plan_id, Some(a.id())).await;

      remove_entry(&db, a.id()).await.unwrap();

      let orphans = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM skill_plan_remap_points r \
        WHERE r.after_entry_id IS NOT NULL \
          AND NOT EXISTS (SELECT 1 FROM skill_plan_entries e WHERE e.id = r.after_entry_id)",
      )
      .fetch_one(&db.0)
      .await
      .unwrap();
      assert_eq!(orphans, 0);
    }

    #[tokio::test]
    async fn it_prunes_a_dependent_remap_when_the_target_slot_is_occupied() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;
      let a = insert_entry(&db, plan_id, 3300, 5).await.unwrap();
      let b = insert_entry(&db, plan_id, 3301, 5).await.unwrap();
      let on_a = upsert_after(&db, plan_id, Some(a.id())).await;
      upsert_after(&db, plan_id, Some(b.id())).await;

      remove_entry(&db, b.id()).await.unwrap();

      let remaps = remap_points(&db, plan_id).await.unwrap();
      assert_eq!(remaps.len(), 1, "no orphan, no duplicate slot");
      assert_eq!(remaps[0].id(), on_a.id());
      assert_eq!(remaps[0].after_entry_id(), Some(a.id()));
    }
  }

  mod remove_entry {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_deletes_and_redensifies_positions() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;
      let a = insert_entry(&db, plan_id, 3300, 5).await.unwrap();
      let b = insert_entry(&db, plan_id, 3301, 5).await.unwrap();
      let c = insert_entry(&db, plan_id, 3302, 5).await.unwrap();

      remove_entry(&db, b.id()).await.unwrap();

      let after = entries(&db, plan_id).await.unwrap();
      assert_eq!(after.iter().map(|e| e.id()).collect::<Vec<_>>(), [a.id(), c.id()]);
      assert_eq!(after.iter().map(|e| e.position()).collect::<Vec<_>>(), [0, 1]);
    }

    #[tokio::test]
    async fn it_is_a_no_op_for_a_missing_entry() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;
      insert_entry(&db, plan_id, 3300, 5).await.unwrap();

      remove_entry(&db, 999_999).await.unwrap();

      assert_eq!(entries(&db, plan_id).await.unwrap().len(), 1);
    }
  }

  mod remove_remap_point {
    use super::*;

    #[tokio::test]
    async fn it_deletes_a_remap_by_id() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;
      let remap = upsert_after(&db, plan_id, None).await;

      remove_remap_point(&db, remap.id()).await.unwrap();

      assert!(remap_points(&db, plan_id).await.unwrap().is_empty());
    }
  }

  mod reorder_entries {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_changes_positions_but_keeps_ids_stable() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;
      let a = insert_entry(&db, plan_id, 3300, 5).await.unwrap();
      let b = insert_entry(&db, plan_id, 3301, 5).await.unwrap();
      let c = insert_entry(&db, plan_id, 3302, 5).await.unwrap();

      reorder_entries(&db, &[c.id(), a.id(), b.id()]).await.unwrap();

      let after = entries(&db, plan_id).await.unwrap();
      assert_eq!(
        after.iter().map(|e| e.id()).collect::<Vec<_>>(),
        [c.id(), a.id(), b.id()]
      );
      assert_eq!(after.iter().map(|e| e.position()).collect::<Vec<_>>(), [0, 1, 2]);
      let mut ids = after.iter().map(|e| e.id()).collect::<Vec<_>>();
      ids.sort_unstable();
      let mut original = vec![a.id(), b.id(), c.id()];
      original.sort_unstable();
      assert_eq!(ids, original);
    }
  }

  mod reorder_survival {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn a_remap_keeps_pointing_at_the_same_entry_after_a_reorder() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;
      let a = insert_entry(&db, plan_id, 3300, 5).await.unwrap();
      let b = insert_entry(&db, plan_id, 3301, 5).await.unwrap();
      let c = insert_entry(&db, plan_id, 3302, 5).await.unwrap();
      upsert_after(&db, plan_id, Some(b.id())).await;

      reorder_entries(&db, &[c.id(), a.id(), b.id()]).await.unwrap();

      let remaps = remap_points(&db, plan_id).await.unwrap();
      assert_eq!(remaps.len(), 1);
      assert_eq!(remaps[0].after_entry_id(), Some(b.id()));
    }
  }

  mod replace_entries {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_clears_the_plan_when_given_no_entries() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;
      insert_entry(&db, plan_id, 3300, 5).await.unwrap();

      replace_entries(&db, plan_id, &[]).await.unwrap();

      assert!(entries(&db, plan_id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_overwrites_every_entry_with_dense_positions() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;
      insert_entry(&db, plan_id, 3300, 5).await.unwrap();
      insert_entry(&db, plan_id, 3301, 5).await.unwrap();

      replace_entries(
        &db,
        plan_id,
        &[(4000, 5, "high", "must have", 0), (4001, 3, "normal", "", 1)],
      )
      .await
      .unwrap();

      let after = entries(&db, plan_id).await.unwrap();
      assert_eq!(after.iter().map(|e| e.skill_id()).collect::<Vec<_>>(), [4000, 4001]);
      assert_eq!(after.iter().map(|e| e.position()).collect::<Vec<_>>(), [0, 1]);
      assert_eq!(after[0].priority(), "high");
      assert_eq!(after[0].note(), "must have");
      assert_eq!(after[1].is_auto(), 1);
    }
  }

  mod round_trip {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_reads_updates_lists_and_deletes_a_plan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let plan = create(&db, 42, "Combat").await.unwrap();
      assert_eq!(get(&db, plan.id()).await.unwrap().unwrap(), plan);

      update(&db, plan.id(), "Industry", "optimal", "none").await.unwrap();
      let updated = get(&db, plan.id()).await.unwrap().unwrap();
      assert_eq!(updated.name(), "Industry");
      assert_eq!(updated.sort_mode(), "optimal");
      assert_eq!(updated.implant_set(), "none");
      assert!(updated.updated_at() >= plan.updated_at());

      assert_eq!(for_character(&db, 42).await.unwrap(), vec![updated]);

      delete(&db, plan.id()).await.unwrap();
      assert!(get(&db, plan.id()).await.unwrap().is_none());
      assert!(for_character(&db, 42).await.unwrap().is_empty());
    }
  }

  mod ship_masteries {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_replaces_and_reads_back_masteries() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;

      replace_ship_masteries(&db, plan_id, &[(587, 4), (588, 2)])
        .await
        .unwrap();
      let rows = ship_masteries(&db, plan_id).await.unwrap();
      assert_eq!(
        rows.iter().map(|r| (r.ship_type_id(), r.tier())).collect::<Vec<_>>(),
        [(587, 4), (588, 2)]
      );

      replace_ship_masteries(&db, plan_id, &[(587, 5)]).await.unwrap();
      let rows = ship_masteries(&db, plan_id).await.unwrap();
      assert_eq!(
        rows.iter().map(|r| (r.ship_type_id(), r.tier())).collect::<Vec<_>>(),
        [(587, 5)]
      );
    }
  }

  mod upsert_remap_point {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_replaces_the_remap_in_a_slot() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;
      let entry = insert_entry(&db, plan_id, 3300, 5).await.unwrap();

      upsert_after(&db, plan_id, Some(entry.id())).await;
      let replacement = upsert_remap_point(&db, plan_id, Some(entry.id()), 27, 17, 17, 21, 17)
        .await
        .unwrap();

      let all = remap_points(&db, plan_id).await.unwrap();
      assert_eq!(all.len(), 1);
      assert_eq!(all[0].id(), replacement.id());
      assert_eq!(all[0].base_perception(), 27);
    }

    #[tokio::test]
    async fn it_supports_the_start_bucket() {
      let db = store::open_test().await.unwrap();
      let plan_id = seed_plan(&db).await;

      let remap = upsert_after(&db, plan_id, None).await;

      assert_eq!(remap.after_entry_id(), None);
      assert_eq!(remap_points(&db, plan_id).await.unwrap().len(), 1);
    }
  }
}

#[cfg(test)]
mod requirements_tests {
  use super::*;

  fn cert_skill(skill_id: i64, basic: i64, improved: i64, advanced: i64, elite: i64) -> CertificateSkill {
    CertificateSkill {
      advanced,
      basic,
      certificate_id: 1,
      elite,
      improved,
      skill_id,
    }
  }

  fn sorted(mut reqs: Vec<(i64, u8)>) -> Vec<(i64, u8)> {
    reqs.sort();
    reqs
  }

  mod skills_for_cert_at_proficiency {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_drops_zero_level_requirements() {
      let skills = vec![cert_skill(100, 0, 2, 3, 4)];
      assert_eq!(skills_for_cert_at_proficiency(&skills, 0), vec![]);
    }

    #[test]
    fn it_reads_the_basic_column_at_index_0() {
      let skills = vec![cert_skill(100, 1, 2, 3, 4)];
      assert_eq!(skills_for_cert_at_proficiency(&skills, 0), vec![(100, 1)]);
    }

    #[test]
    fn it_reads_the_elite_column_for_indices_3_and_above() {
      let skills = vec![cert_skill(100, 1, 2, 3, 5)];
      assert_eq!(skills_for_cert_at_proficiency(&skills, 3), vec![(100, 5)]);
      assert_eq!(skills_for_cert_at_proficiency(&skills, 9), vec![(100, 5)]);
    }
  }

  mod skills_for_mastery {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_caps_at_tier_five_and_at_the_available_tier_count() {
      let tiers = vec![vec![cert_skill(100, 1, 2, 3, 5)]];
      assert_eq!(skills_for_mastery(&tiers, 5), vec![(100, 1)]);
    }

    #[test]
    fn it_dedups_keeping_the_highest_level_across_tiers() {
      let tiers = vec![
        vec![cert_skill(100, 1, 2, 3, 5)],
        vec![cert_skill(100, 1, 2, 3, 5)],
        vec![cert_skill(100, 1, 2, 3, 5)],
      ];

      assert_eq!(skills_for_mastery(&tiers, 3), vec![(100, 3)]);
    }

    #[test]
    fn it_returns_empty_for_mastery_zero() {
      let tiers = vec![vec![cert_skill(100, 1, 2, 3, 5)]];
      assert_eq!(skills_for_mastery(&tiers, 0), vec![]);
    }

    #[test]
    fn it_unions_tiers_cumulatively_up_to_the_requested_level() {
      let tiers = vec![vec![cert_skill(100, 1, 2, 3, 4)], vec![cert_skill(200, 1, 2, 3, 4)]];

      assert_eq!(sorted(skills_for_mastery(&tiers, 1)), vec![(100, 1)]);
      assert_eq!(sorted(skills_for_mastery(&tiers, 2)), vec![(100, 1), (200, 2)]);
    }

    #[test]
    fn it_uses_the_elite_column_for_tiers_iv_and_v() {
      let tiers = vec![
        vec![cert_skill(100, 1, 2, 3, 5)],
        vec![cert_skill(100, 1, 2, 3, 5)],
        vec![cert_skill(100, 1, 2, 3, 5)],
        vec![cert_skill(100, 1, 2, 3, 5)],
        vec![cert_skill(100, 1, 2, 3, 5)],
      ];

      assert_eq!(skills_for_mastery(&tiers, 5), vec![(100, 5)]);
    }
  }

  mod skills_for_module {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_dedups_duplicate_skills_keeping_the_highest_level() {
      let out = sorted(skills_for_module(&[(3300, 2), (3301, 1), (3300, 4)]));
      assert_eq!(out, vec![(3300, 4), (3301, 1)]);
    }

    #[test]
    fn it_passes_through_single_requirements() {
      assert_eq!(skills_for_module(&[(3300, 2)]), vec![(3300, 2)]);
    }

    #[test]
    fn it_returns_empty_for_no_requirements() {
      assert_eq!(skills_for_module(&[]), vec![]);
    }
  }
}
