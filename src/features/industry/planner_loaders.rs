use std::collections::HashMap;

use super::{
  Scope,
  planner_model::{Material, REACTION_ACTIVITY_ID},
};
use crate::store::{
  Database,
  model::Facility,
  repo::{blueprints, finance, industry},
};

const MANUFACTURING_ACTIVITY_ID: i64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlueprintRecipe {
  pub activity_id: i64,
  pub blueprint_type_id: i64,
  pub is_reaction: bool,
  pub output_per_run: i64,
  pub product_type_id: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnedBlueprint {
  pub in_scope: bool,
  pub item_id: i64,
  pub material_efficiency: i64,
  pub runs: i64,
  pub time_efficiency: i64,
}

impl OwnedBlueprint {
  pub fn is_original(&self) -> bool {
    self.runs < 0
  }
}

pub async fn average_price(db: &Database, type_id: i64) -> Option<f64> {
  prices(db).await.get(&type_id).copied()
}

pub async fn best_owned_blueprint(db: &Database, blueprint_type_id: i64, scope: Scope) -> Option<OwnedBlueprint> {
  let owned = owned_blueprints(db, blueprint_type_id, scope).await;
  rank_best_owned(owned)
}

pub async fn build_time(db: &Database, blueprint_type_id: i64, activity_id: i64) -> Option<(i64, i64)> {
  blueprints::activity_meta(db, blueprint_type_id, activity_id)
    .await
    .ok()
    .flatten()
}

pub async fn cost_index(db: &Database, solar_system_id: i64, activity_id: i64) -> Option<f64> {
  industry::cost_index_for(db, solar_system_id, activity_id)
    .await
    .ok()
    .flatten()
}

pub async fn facilities(db: &Database) -> Vec<Facility> {
  industry::accessible_facilities(db).await.unwrap_or_default()
}

pub async fn materials_for(db: &Database, blueprint_type_id: i64, activity_id: i64) -> Vec<Material> {
  let rows: Vec<(i64, i64)> = sqlx::query_as(
    "SELECT material_type_id, quantity FROM blueprint_activity_materials \
    WHERE blueprint_type_id = ? AND activity_id = ? ORDER BY material_type_id",
  )
  .bind(blueprint_type_id)
  .bind(activity_id)
  .fetch_all(&db.0)
  .await
  .unwrap_or_default();

  rows
    .into_iter()
    .map(|(type_id, quantity)| Material::new(type_id, quantity))
    .collect()
}

pub async fn output_per_run(db: &Database, blueprint_type_id: i64, activity_id: i64) -> Option<i64> {
  sqlx::query_scalar(
    "SELECT quantity FROM blueprint_activity_products \
    WHERE blueprint_type_id = ? AND activity_id = ? LIMIT 1",
  )
  .bind(blueprint_type_id)
  .bind(activity_id)
  .fetch_optional(&db.0)
  .await
  .ok()
  .flatten()
}

pub async fn prices(db: &Database) -> HashMap<i64, f64> {
  finance::market_prices_all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter_map(|price| price.average_price().map(|value| (price.type_id(), value)))
    .collect()
}

/// Resolves the blueprint that produces `product_type_id` and the activity it is built by, preferring manufacturing
/// (activity 1) over a reaction (activity 11) when a product is reachable both ways.
pub async fn reverse_lookup(db: &Database, product_type_id: i64) -> Option<BlueprintRecipe> {
  if let Some(recipe) = recipe_for_activity(db, product_type_id, MANUFACTURING_ACTIVITY_ID).await {
    return Some(recipe);
  }
  recipe_for_activity(db, product_type_id, REACTION_ACTIVITY_ID).await
}

async fn owned_blueprints(db: &Database, blueprint_type_id: i64, scope: Scope) -> Vec<OwnedBlueprint> {
  let all = blueprints::list_all(db).await.unwrap_or_default();
  let mut owned = Vec::new();
  for row in all.character_blueprints {
    if row.type_id() == blueprint_type_id {
      owned.push(OwnedBlueprint {
        in_scope: matches!(scope, Scope::All) || matches!(scope, Scope::Char(id) if id == row.character_id()),
        item_id: row.item_id(),
        material_efficiency: row.material_efficiency(),
        runs: row.runs(),
        time_efficiency: row.time_efficiency(),
      });
    }
  }
  for row in all.corporation_blueprints {
    if row.type_id() == blueprint_type_id {
      owned.push(OwnedBlueprint {
        in_scope: matches!(scope, Scope::All) || matches!(scope, Scope::Corp(id) if id == row.corporation_id()),
        item_id: row.item_id(),
        material_efficiency: row.material_efficiency(),
        runs: row.runs(),
        time_efficiency: row.time_efficiency(),
      });
    }
  }
  owned
}

/// Picks the blueprint to auto-populate ME/TE from, ranking in-scope before out-of-scope, then BPO before BPC, then
/// higher material efficiency; ties break on the lowest `item_id` for a stable, deterministic pick.
fn rank_best_owned(mut owned: Vec<OwnedBlueprint>) -> Option<OwnedBlueprint> {
  owned.sort_by(|a, b| {
    b.in_scope
      .cmp(&a.in_scope)
      .then(b.is_original().cmp(&a.is_original()))
      .then(b.material_efficiency.cmp(&a.material_efficiency))
      .then(a.item_id.cmp(&b.item_id))
  });
  owned.into_iter().next()
}

async fn recipe_for_activity(db: &Database, product_type_id: i64, activity_id: i64) -> Option<BlueprintRecipe> {
  let row: Option<(i64, i64)> = sqlx::query_as(
    "SELECT blueprint_type_id, quantity FROM blueprint_activity_products \
    WHERE product_type_id = ? AND activity_id = ? ORDER BY blueprint_type_id LIMIT 1",
  )
  .bind(product_type_id)
  .bind(activity_id)
  .fetch_optional(&db.0)
  .await
  .ok()
  .flatten();

  row.map(|(blueprint_type_id, quantity)| BlueprintRecipe {
    activity_id,
    blueprint_type_id,
    is_reaction: activity_id == REACTION_ACTIVITY_ID,
    output_per_run: quantity.max(1),
    product_type_id,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{
      Alliance, Bloodline, Character, CharacterBlueprint, Corporation, CorporationBlueprint, CorporationMemberRole,
      Gender, OwnerType, Race,
    },
    repo::{character, infra, org},
  };

  const CHARACTER_ID: i64 = 42;
  const CORPORATION_ID: i64 = 90_000_001;
  const DIRECTOR_ID: i64 = 100;
  const HULK: i64 = 22_544;
  const HULK_BLUEPRINT: i64 = 22_545;
  const TRITANIUM: i64 = 34;

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = CORPORATION_ID;
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

  async fn authorize_corporation(db: &Database) {
    infra::upsert(
      db,
      CORPORATION_ID,
      OwnerType::Corporation,
      "tok",
      "rt",
      4_102_444_800,
      Some(DIRECTOR_ID),
      None,
    )
    .await
    .unwrap();
    org::replace_for_corporation(
      db,
      CORPORATION_ID,
      &[CorporationMemberRole::from((
        CORPORATION_ID,
        DIRECTOR_ID,
        "Director".to_owned(),
      ))],
    )
    .await
    .unwrap();
  }

  async fn insert_product(
    db: &Database,
    blueprint_type_id: i64,
    activity_id: i64,
    product_type_id: i64,
    quantity: i64,
  ) {
    sqlx::query(
      "INSERT INTO blueprint_activity_products (blueprint_type_id, activity_id, product_type_id, quantity) \
      VALUES (?, ?, ?, ?)",
    )
    .bind(blueprint_type_id)
    .bind(activity_id)
    .bind(product_type_id)
    .bind(quantity)
    .execute(&db.0)
    .await
    .unwrap();
  }

  async fn insert_material(
    db: &Database,
    blueprint_type_id: i64,
    activity_id: i64,
    material_type_id: i64,
    quantity: i64,
  ) {
    sqlx::query(
      "INSERT INTO blueprint_activity_materials (blueprint_type_id, activity_id, material_type_id, quantity) \
      VALUES (?, ?, ?, ?)",
    )
    .bind(blueprint_type_id)
    .bind(activity_id)
    .bind(material_type_id)
    .bind(quantity)
    .execute(&db.0)
    .await
    .unwrap();
  }

  fn owned(in_scope: bool, item_id: i64, runs: i64, material_efficiency: i64) -> OwnedBlueprint {
    OwnedBlueprint {
      in_scope,
      item_id,
      material_efficiency,
      runs,
      time_efficiency: 0,
    }
  }

  fn character_blueprint(character_id: i64, item_id: i64, type_id: i64, runs: i64, me: i64) -> CharacterBlueprint {
    CharacterBlueprint {
      character_id,
      item_id,
      location_flag: "Hangar".to_owned(),
      location_id: 60_003_760,
      material_efficiency: me,
      quantity: -1,
      runs,
      time_efficiency: 0,
      type_id,
    }
  }

  fn corporation_blueprint(
    corporation_id: i64,
    item_id: i64,
    type_id: i64,
    runs: i64,
    me: i64,
  ) -> CorporationBlueprint {
    CorporationBlueprint {
      corporation_id,
      item_id,
      location_flag: "CorpSAG1".to_owned(),
      location_id: 60_003_760,
      material_efficiency: me,
      quantity: -2,
      runs,
      time_efficiency: 0,
      type_id,
    }
  }

  mod rank_best_owned {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_none_when_no_blueprint_is_owned() {
      assert_eq!(super::rank_best_owned(Vec::new()), None);
    }

    #[test]
    fn it_prefers_an_in_scope_blueprint_over_an_out_of_scope_one() {
      let pool = vec![owned(false, 1, -1, 10), owned(true, 2, 30, 0)];

      let best = super::rank_best_owned(pool).unwrap();

      assert_eq!(best.item_id, 2);
    }

    #[test]
    fn it_prefers_a_bpo_over_a_bpc_within_the_same_scope() {
      let pool = vec![owned(true, 1, 30, 10), owned(true, 2, -1, 0)];

      let best = super::rank_best_owned(pool).unwrap();

      assert_eq!(best.item_id, 2);
    }

    #[test]
    fn it_prefers_higher_material_efficiency_to_break_a_tie() {
      let pool = vec![owned(true, 1, -1, 8), owned(true, 2, -1, 10)];

      let best = super::rank_best_owned(pool).unwrap();

      assert_eq!(best.item_id, 2);
    }

    #[test]
    fn it_orders_scope_before_originality_before_material_efficiency() {
      let pool = vec![owned(false, 1, -1, 10), owned(true, 2, 30, 0), owned(true, 3, -1, 5)];

      let best = super::rank_best_owned(pool).unwrap();

      assert_eq!(best.item_id, 3);
    }
  }

  mod reverse_lookup {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_resolves_a_manufacturing_blueprint_for_a_product() {
      let db = store::open_test().await.unwrap();
      insert_product(&db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID, HULK, 1).await;

      let recipe = super::reverse_lookup(&db, HULK).await.unwrap();

      assert_eq!(recipe.blueprint_type_id, HULK_BLUEPRINT);
      assert_eq!(recipe.activity_id, MANUFACTURING_ACTIVITY_ID);
      assert!(!recipe.is_reaction);
      assert_eq!(recipe.output_per_run, 1);
    }

    #[tokio::test]
    async fn it_prefers_manufacturing_over_a_reaction_for_the_same_product() {
      let db = store::open_test().await.unwrap();
      insert_product(&db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID, HULK, 1).await;
      insert_product(&db, 999, REACTION_ACTIVITY_ID, HULK, 1).await;

      let recipe = super::reverse_lookup(&db, HULK).await.unwrap();

      assert_eq!(recipe.blueprint_type_id, HULK_BLUEPRINT);
      assert!(!recipe.is_reaction);
    }

    #[tokio::test]
    async fn it_falls_back_to_a_reaction_when_no_manufacturing_blueprint_exists() {
      let db = store::open_test().await.unwrap();
      insert_product(&db, 999, REACTION_ACTIVITY_ID, HULK, 200).await;

      let recipe = super::reverse_lookup(&db, HULK).await.unwrap();

      assert_eq!(recipe.activity_id, REACTION_ACTIVITY_ID);
      assert!(recipe.is_reaction);
      assert_eq!(recipe.output_per_run, 200);
    }

    #[tokio::test]
    async fn it_returns_none_for_an_unbuildable_product() {
      let db = store::open_test().await.unwrap();

      assert_eq!(super::reverse_lookup(&db, HULK).await, None);
    }
  }

  mod materials_for {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_the_recipe_materials_ordered_by_type_id() {
      let db = store::open_test().await.unwrap();
      insert_material(&db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID, 35, 200).await;
      insert_material(&db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID, TRITANIUM, 100).await;

      let materials = super::materials_for(&db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID).await;

      assert_eq!(materials, vec![Material::new(TRITANIUM, 100), Material::new(35, 200)]);
    }

    #[tokio::test]
    async fn it_is_empty_for_an_activity_with_no_materials() {
      let db = store::open_test().await.unwrap();

      assert!(
        super::materials_for(&db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID)
          .await
          .is_empty()
      );
    }
  }

  mod output_per_run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_the_product_quantity_per_run() {
      let db = store::open_test().await.unwrap();
      insert_product(&db, 999, REACTION_ACTIVITY_ID, HULK, 200).await;

      assert_eq!(super::output_per_run(&db, 999, REACTION_ACTIVITY_ID).await, Some(200));
    }
  }

  mod best_owned_blueprint {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_picks_the_in_scope_blueprint_for_a_character_scope() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corporation(&db).await;
      blueprints::replace_for_character(
        &db,
        CHARACTER_ID,
        &[character_blueprint(CHARACTER_ID, 1, HULK_BLUEPRINT, -1, 10)],
      )
      .await
      .unwrap();
      blueprints::replace_for_corporation(
        &db,
        CORPORATION_ID,
        &[corporation_blueprint(CORPORATION_ID, 2, HULK_BLUEPRINT, -1, 20)],
      )
      .await
      .unwrap();

      let best = super::best_owned_blueprint(&db, HULK_BLUEPRINT, Scope::Char(CHARACTER_ID))
        .await
        .unwrap();

      assert_eq!(best.item_id, 1);
      assert!(best.in_scope);
    }

    #[tokio::test]
    async fn it_falls_back_to_an_out_of_scope_blueprint_when_none_in_scope() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      blueprints::replace_for_character(
        &db,
        CHARACTER_ID,
        &[character_blueprint(CHARACTER_ID, 1, HULK_BLUEPRINT, 30, 5)],
      )
      .await
      .unwrap();

      let best = super::best_owned_blueprint(&db, HULK_BLUEPRINT, Scope::Char(DIRECTOR_ID))
        .await
        .unwrap();

      assert_eq!(best.item_id, 1);
      assert!(!best.in_scope);
    }

    #[tokio::test]
    async fn it_returns_none_when_no_blueprint_is_owned() {
      let db = store::open_test().await.unwrap();

      assert_eq!(super::best_owned_blueprint(&db, HULK_BLUEPRINT, Scope::All).await, None);
    }
  }
}
