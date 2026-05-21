//! Repository for item type persistence.

use std::collections::HashMap;

use pod_model::{ItemType, ItemTypeSummary};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Order, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::{
    dogma_attribute,
    item_group::{Column as GroupColumn, Entity as GroupEntity},
    item_type::{ActiveModel, Column, Entity},
    ship_mastery_cert::{Column as MasteryColumn, Entity as MasteryEntity},
  },
};

fn skill_reqs_from_attrs(attrs: &[dogma_attribute::Entry]) -> Vec<(i32, u8)> {
  let skill_attr_pairs = [(182, 277), (183, 278), (184, 279), (185, 280), (186, 281)];
  let mut result = Vec::new();
  for (type_attr_id, level_attr_id) in skill_attr_pairs {
    let type_id = attrs.iter().find(|a| a.attribute_id == type_attr_id).map(|a| a.value as i32);
    let level = attrs.iter().find(|a| a.attribute_id == level_attr_id).map(|a| a.value as u8);
    if let (Some(tid), Some(lvl)) = (type_id, level) {
      if tid > 0 && lvl > 0 {
        result.push((tid, lvl));
      }
    }
  }
  result
}

#[cfg(test)]
fn parse_skill_requirements(dogma_json: &str) -> Vec<(i32, u8)> {
  let attrs: Vec<dogma_attribute::Entry> = serde_json::from_str(dogma_json).unwrap_or_default();
  skill_reqs_from_attrs(&attrs)
}

fn parse_cert_ids(json: Option<&String>) -> Vec<i32> {
  match json {
    Some(s) => serde_json::from_str::<Vec<i32>>(s).unwrap_or_default(),
    None => vec![],
  }
}

async fn resolve_skill_names(db: &DatabaseConnection, type_ids: &[i32]) -> HashMap<i32, String> {
  if type_ids.is_empty() {
    return HashMap::new();
  }
  Entity::find()
    .filter(Column::Id.is_in(type_ids.to_vec()))
    .all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.id, r.name))
    .collect()
}

/// Repository for item type CRUD operations.
pub struct Repo<'a> {
  db: &'a DatabaseConnection,
}

impl<'a> Repo<'a> {
  /// Creates a new `Repo` bound to the given database connection.
  pub fn new(db: &'a DatabaseConnection) -> Self {
    Self {
      db,
    }
  }

  /// Returns published ships whose name matches the given search string (case-insensitive).
  pub async fn find_ships(&self, search: &str) -> Result<Vec<ItemTypeSummary>, Error> {
    let ship_groups: HashMap<i32, String> = GroupEntity::find()
      .filter(GroupColumn::ItemCategoryId.eq(6))
      .all(self.db)
      .await?
      .into_iter()
      .map(|g| (g.id, g.name))
      .collect();

    let group_ids: Vec<i32> = ship_groups.keys().copied().collect();
    let mut items = Entity::find()
      .filter(Column::ItemGroupId.is_in(group_ids))
      .filter(Column::Published.eq(true))
      .filter(Column::Name.contains(search))
      .order_by(Column::Name, Order::Asc)
      .all(self.db)
      .await?;
    items.sort_by(|a, b| {
      let ga = ship_groups.get(&a.item_group_id).map(|s| s.as_str()).unwrap_or("");
      let gb = ship_groups.get(&b.item_group_id).map(|s| s.as_str()).unwrap_or("");
      ga.cmp(gb).then_with(|| a.name.cmp(&b.name))
    });

    let type_ids: Vec<i32> = items.iter().map(|t| t.id).collect();
    let mastery_rows = MasteryEntity::find()
      .filter(MasteryColumn::ShipId.is_in(type_ids))
      .all(self.db)
      .await?;

    let mut mastery_map: HashMap<i32, HashMap<i32, String>> = HashMap::new();
    for row in mastery_rows {
      mastery_map.entry(row.ship_id).or_default().insert(row.mastery_level, row.cert_ids_json);
    }

    let raw: Vec<_> = items
      .iter()
      .map(|item| {
        let reqs = skill_reqs_from_attrs(&item.dogma_attributes.0);
        (item, reqs)
      })
      .collect();

    let all_skill_ids: Vec<i32> = raw
      .iter()
      .flat_map(|(_, reqs)| reqs.iter().map(|&(tid, _)| tid))
      .collect::<std::collections::HashSet<_>>()
      .into_iter()
      .collect();
    let names = resolve_skill_names(self.db, &all_skill_ids).await;

    Ok(
      raw
        .into_iter()
        .map(|(item, reqs)| {
          let mastery = mastery_map.get(&item.id);
          ItemTypeSummary {
            id: item.id,
            name: item.name.clone(),
            group_name: ship_groups.get(&item.item_group_id).cloned().unwrap_or_default(),
            skill_requirements: reqs
              .into_iter()
              .filter_map(|(tid, lvl)| names.get(&tid).map(|n| (n.clone(), lvl)))
              .collect(),
            mastery_cert_ids: (1..=5)
              .map(|lvl| parse_cert_ids(mastery.and_then(|m| m.get(&lvl))))
              .collect(),
          }
        })
        .collect(),
    )
  }

  /// Returns published modules whose name matches the given search string (case-insensitive).
  /// Only modules that have at least one skill requirement are returned.
  pub async fn find_modules(&self, search: &str) -> Result<Vec<ItemTypeSummary>, Error> {
    let module_groups: HashMap<i32, String> = GroupEntity::find()
      .filter(GroupColumn::ItemCategoryId.eq(7))
      .all(self.db)
      .await?
      .into_iter()
      .map(|g| (g.id, g.name))
      .collect();

    let group_ids: Vec<i32> = module_groups.keys().copied().collect();
    let mut items = Entity::find()
      .filter(Column::ItemGroupId.is_in(group_ids))
      .filter(Column::Published.eq(true))
      .filter(Column::Name.contains(search))
      .order_by(Column::Name, Order::Asc)
      .all(self.db)
      .await?;
    items.sort_by(|a, b| {
      let ga = module_groups.get(&a.item_group_id).map(|s| s.as_str()).unwrap_or("");
      let gb = module_groups.get(&b.item_group_id).map(|s| s.as_str()).unwrap_or("");
      ga.cmp(gb).then_with(|| a.name.cmp(&b.name))
    });

    let raw: Vec<_> = items
      .iter()
      .filter_map(|item| {
        let reqs = skill_reqs_from_attrs(&item.dogma_attributes.0);
        if reqs.is_empty() { None } else { Some((item, reqs)) }
      })
      .collect();

    let all_skill_ids: Vec<i32> = raw
      .iter()
      .flat_map(|(_, reqs)| reqs.iter().map(|&(tid, _)| tid))
      .collect::<std::collections::HashSet<_>>()
      .into_iter()
      .collect();
    let names = resolve_skill_names(self.db, &all_skill_ids).await;

    Ok(
      raw
        .into_iter()
        .map(|(item, reqs)| ItemTypeSummary {
          id: item.id,
          name: item.name.clone(),
          group_name: module_groups.get(&item.item_group_id).cloned().unwrap_or_default(),
          skill_requirements: reqs
            .into_iter()
            .filter_map(|(tid, lvl)| names.get(&tid).map(|n| (n.clone(), lvl)))
            .collect(),
          mastery_cert_ids: vec![],
        })
        .collect(),
    )
  }

  /// Returns all published skills (EVE category 16) grouped by item group.
  pub async fn find_skill_groups(&self) -> Result<Vec<pod_model::SkillGroupDef>, Error> {
    let skill_groups: HashMap<i32, String> = GroupEntity::find()
      .filter(GroupColumn::ItemCategoryId.eq(16))
      .all(self.db)
      .await?
      .into_iter()
      .map(|g| (g.id, g.name))
      .collect();

    let group_ids: Vec<i32> = skill_groups.keys().copied().collect();
    let mut items = Entity::find()
      .filter(Column::ItemGroupId.is_in(group_ids))
      .filter(Column::Published.eq(true))
      .order_by(Column::Name, Order::Asc)
      .all(self.db)
      .await?;
    items.sort_by(|a, b| {
      let ga = skill_groups.get(&a.item_group_id).map(|s| s.as_str()).unwrap_or("");
      let gb = skill_groups.get(&b.item_group_id).map(|s| s.as_str()).unwrap_or("");
      ga.cmp(gb).then_with(|| a.name.cmp(&b.name))
    });

    let all_prereq_ids: Vec<i32> = items
      .iter()
      .flat_map(|item| skill_reqs_from_attrs(&item.dogma_attributes.0).into_iter().map(|(tid, _)| tid))
      .collect::<std::collections::HashSet<_>>()
      .into_iter()
      .collect();
    let prereq_names = resolve_skill_names(self.db, &all_prereq_ids).await;

    let mut groups: Vec<pod_model::SkillGroupDef> = Vec::new();
    for item in items {
      let attrs = &item.dogma_attributes.0;
      let rank = attrs.iter().find(|a| a.attribute_id == 275).map(|a| a.value as u8).unwrap_or(1);
      let primary_id = attrs.iter().find(|a| a.attribute_id == 180).map(|a| a.value as u8).unwrap_or(167);
      let secondary_id = attrs.iter().find(|a| a.attribute_id == 181).map(|a| a.value as u8).unwrap_or(168);
      let prereqs: Vec<(String, u8)> = skill_reqs_from_attrs(attrs)
        .into_iter()
        .filter_map(|(tid, lvl)| prereq_names.get(&tid).map(|n| (n.clone(), lvl)))
        .collect();

      let skill = pod_model::SkillDef {
        type_id: item.id,
        name: item.name,
        rank,
        level: 0,
        sp: 0,
        primary: pod_model::AttrKey::from_eve_id(primary_id),
        secondary: pod_model::AttrKey::from_eve_id(secondary_id),
        prereqs,
      };

      let group_id_str = item.item_group_id.to_string();
      let group_name = skill_groups.get(&item.item_group_id).cloned().unwrap_or_default();
      match groups.iter_mut().find(|g| g.id == group_id_str) {
        Some(group) => group.skills.push(skill),
        None => groups.push(pod_model::SkillGroupDef {
          id: group_id_str,
          name: group_name,
          skills: vec![skill],
        }),
      }
    }

    Ok(groups)
  }

  /// Returns all item types.
  pub async fn all(&self) -> Result<Vec<ItemType>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Finds an item type by its unique ID.
  pub async fn find(&self, id: i32) -> Result<Option<ItemType>, Error> {
    let row = Entity::find_by_id(id).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Finds an item type by its display name (exact match).
  pub async fn find_by_name(&self, name: &str) -> Result<Option<ItemType>, Error> {
    let row = Entity::find().filter(Column::Name.eq(name)).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Returns raw entity rows for the given type IDs.
  pub async fn find_by_ids(&self, ids: &[i32]) -> Result<Vec<crate::entities::item_type::Model>, Error> {
    let rows = Entity::find()
      .filter(Column::Id.is_in(ids.to_vec()))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Returns `(type_id, attribute_bonus_json, slot, name)` for each given implant type ID.
  ///
  /// The `attribute_bonus_json` is a JSON object mapping neural attribute names to their bonus
  /// amounts (e.g. `{"perception":4,"willpower":4}`). The slot is derived from dogma attribute
  /// 331. Used by the startup ESI sync to build implant DB records without exposing entity types.
  pub async fn implant_data_for_ids(&self, ids: &[i32]) -> Result<Vec<(i32, String, i32, String)>, Error> {
    if ids.is_empty() {
      return Ok(Vec::new());
    }
    let rows = Entity::find()
      .filter(Column::Id.is_in(ids.to_vec()))
      .all(self.db)
      .await?;
    Ok(
      rows
        .iter()
        .map(|row| {
          let mut bonus = serde_json::Map::new();
          let mut slot = 0i32;
          for attr in &row.dogma_attributes.0 {
            match attr.attribute_id {
              175 => {
                bonus.insert("charisma".into(), (attr.value as i64).into());
              }
              176 => {
                bonus.insert("intelligence".into(), (attr.value as i64).into());
              }
              177 => {
                bonus.insert("memory".into(), (attr.value as i64).into());
              }
              178 => {
                bonus.insert("perception".into(), (attr.value as i64).into());
              }
              179 => {
                bonus.insert("willpower".into(), (attr.value as i64).into());
              }
              331 => {
                slot = attr.value as i32;
              }
              _ => {}
            }
          }
          (
            row.id,
            serde_json::Value::Object(bonus).to_string(),
            slot,
            row.name.clone(),
          )
        })
        .collect(),
    )
  }

  /// Inserts or updates an item type row.
  pub async fn upsert(&self, record: &ItemType) -> Result<(), Error> {
    record.validate()?;
    let active: ActiveModel = record.clone().into();
    Entity::insert(active)
      .on_conflict(
        OnConflict::column(Column::Id)
          .update_columns([
            Column::Capacity,
            Column::Description,
            Column::DogmaAttributes,
            Column::DogmaEffects,
            Column::GraphicId,
            Column::IconId,
            Column::ItemGroupId,
            Column::MarketGroupId,
            Column::Mass,
            Column::Name,
            Column::PackagedVolume,
            Column::PortionSize,
            Column::Published,
            Column::Radius,
            Column::Volume,
          ])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Bulk-upserts item type rows in chunks of 200.
  pub async fn upsert_many(&self, records: &[ItemType]) -> Result<(), Error> {
    if records.is_empty() {
      return Ok(());
    }
    let mut active = Vec::with_capacity(records.len());
    for record in records {
      record.validate()?;
      active.push(ActiveModel::from(record.clone()));
    }
    for chunk in active.chunks(200) {
      Entity::insert_many(chunk.to_vec())
        .on_conflict(
          OnConflict::column(Column::Id)
            .update_columns([
              Column::Capacity,
              Column::Description,
              Column::DogmaAttributes,
              Column::DogmaEffects,
              Column::GraphicId,
              Column::IconId,
              Column::ItemGroupId,
              Column::MarketGroupId,
              Column::Mass,
              Column::Name,
              Column::PackagedVolume,
              Column::PortionSize,
              Column::Published,
              Column::Radius,
              Column::Volume,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod parse_skill_requirements {
    use super::*;

    #[test]
    fn returns_empty_for_empty_json() {
      assert!(parse_skill_requirements("[]").is_empty());
    }

    #[test]
    fn returns_empty_for_invalid_json() {
      assert!(parse_skill_requirements("not-json").is_empty());
    }

    #[test]
    fn returns_empty_when_no_skill_attrs_present() {
      let json = r#"[{"attribute_id": 1, "value": 5.0}]"#;
      assert!(parse_skill_requirements(json).is_empty());
    }

    #[test]
    fn returns_single_requirement_when_pair_present() {
      let json = r#"[
        {"attribute_id": 182, "value": 3300.0},
        {"attribute_id": 277, "value": 3.0}
      ]"#;
      let result = parse_skill_requirements(json);
      assert_eq!(result, vec![(3300, 3)]);
    }

    #[test]
    fn skips_pair_when_type_id_is_zero() {
      let json = r#"[
        {"attribute_id": 182, "value": 0.0},
        {"attribute_id": 277, "value": 3.0}
      ]"#;
      assert!(parse_skill_requirements(json).is_empty());
    }

    #[test]
    fn skips_pair_when_level_is_zero() {
      let json = r#"[
        {"attribute_id": 182, "value": 3300.0},
        {"attribute_id": 277, "value": 0.0}
      ]"#;
      assert!(parse_skill_requirements(json).is_empty());
    }

    #[test]
    fn returns_multiple_requirements_for_all_five_pairs() {
      let json = r#"[
        {"attribute_id": 182, "value": 100.0},
        {"attribute_id": 277, "value": 1.0},
        {"attribute_id": 183, "value": 200.0},
        {"attribute_id": 278, "value": 2.0},
        {"attribute_id": 184, "value": 300.0},
        {"attribute_id": 279, "value": 3.0},
        {"attribute_id": 185, "value": 400.0},
        {"attribute_id": 280, "value": 4.0},
        {"attribute_id": 186, "value": 500.0},
        {"attribute_id": 281, "value": 5.0}
      ]"#;
      let result = parse_skill_requirements(json);
      assert_eq!(result.len(), 5);
      assert!(result.contains(&(100, 1)));
      assert!(result.contains(&(200, 2)));
      assert!(result.contains(&(300, 3)));
      assert!(result.contains(&(400, 4)));
      assert!(result.contains(&(500, 5)));
    }

    #[test]
    fn skips_pair_when_type_id_missing() {
      let json = r#"[{"attribute_id": 277, "value": 3.0}]"#;
      assert!(parse_skill_requirements(json).is_empty());
    }

    #[test]
    fn skips_pair_when_level_missing() {
      let json = r#"[{"attribute_id": 182, "value": 3300.0}]"#;
      assert!(parse_skill_requirements(json).is_empty());
    }
  }

  mod parse_cert_ids {
    use super::*;

    #[test]
    fn returns_empty_for_none() {
      assert!(parse_cert_ids(None).is_empty());
    }

    #[test]
    fn returns_ids_for_valid_json() {
      let s = "[1, 2, 3]".to_string();
      assert_eq!(parse_cert_ids(Some(&s)), vec![1, 2, 3]);
    }

    #[test]
    fn returns_empty_for_invalid_json() {
      let s = "not-json".to_string();
      assert!(parse_cert_ids(Some(&s)).is_empty());
    }

    #[test]
    fn returns_empty_for_empty_array() {
      let s = "[]".to_string();
      assert!(parse_cert_ids(Some(&s)).is_empty());
    }
  }
}
