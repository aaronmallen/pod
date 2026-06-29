use std::collections::{BTreeMap, HashMap};

use super::{
  Scope,
  planner_model::{Material, REACTION_ACTIVITY_ID},
};
use crate::{
  clients::eve_image::Size,
  store::{
    Database,
    images::{self, IconIndex, IconResolution},
    model::{
      Facility,
      character_clone_view::{CharacterClones, CloneWithImplants},
    },
    repo::{blueprints, character, finance, industry, sde},
  },
  ui::components::facility_combobox::{FacilityRef, MIN_STRUCTURE_ID},
};

/// EVE dogma attribute id for a hardwiring implant's manufacturing-time reduction (Zainou 'Beancounter' BX
/// series). The stored value is a negative percent (BX-804 = -4.0), taken as a positive reduction.
pub const ATTR_MANUFACTURING_TIME_BONUS: i64 = 440;
/// EVE dogma attribute id for a hardwiring implant's reaction-time reduction. Stored as a negative percent.
pub const ATTR_REACTION_TIME_BONUS: i64 = 2660;
/// SDE type ids for the manufacturing-time hardwiring implants, mapped to their percent reduction. Used only as
/// a fallback when an implant's dogma-attribute lookup comes back empty (an unsynced SDE), so the BX series
/// still reduces build time. Mirrors the SDE values for [`ATTR_MANUFACTURING_TIME_BONUS`].
pub const CURATED_MANUFACTURING_IMPLANTS: [(i64, f64); 6] = [
  (27_170, 1.0), // Zainou 'Beancounter' Industry BX-801
  (27_167, 2.0), // Zainou 'Beancounter' Industry BX-802
  (27_171, 4.0), // Zainou 'Beancounter' Industry BX-804
  (59_797, 8.0), // Serenity Zainou 'Beancounter' Manufacturing RP-108
  (59_799, 8.0), // Serenity Zainou 'Beancounter' Manufacturing RP-308
  (59_801, 8.0), // Serenity Zainou 'Beancounter' Manufacturing RP-708
];
/// SDE type ids for the reaction-time hardwiring implants, mapped to their percent reduction. Fallback for
/// [`ATTR_REACTION_TIME_BONUS`] when the dogma-attribute lookup is empty.
pub const CURATED_REACTION_IMPLANTS: [(i64, f64); 1] = [
  (45_746, 4.0), // 'Beancounter' Reactions hardwiring
];
/// NPC-station facility tax, the fixed 0.25% of EIV CCP levies on jobs installed at an NPC station. Player
/// structures set their own rate (capped at 10%); the planner has no per-structure tax data, so every job
/// falls back to this NPC default rather than guessing a structure's configured rate.
pub const FACILITY_TAX_RATE: f64 = 0.0025;
/// EVE skill type id for Advanced Industry: -3% to ALL manufacturing and reaction time per level.
pub const SKILL_ADVANCED_INDUSTRY: i64 = 3388;
/// EVE skill type id for Industry: -4% to manufacturing time per level (does not affect reactions).
pub const SKILL_INDUSTRY: i64 = 3380;
const MANUFACTURING_ACTIVITY_ID: i64 = 1;
/// SCC (Secure Commerce Commission) surcharge, the flat 4% of EIV CCP adds to every industry job
/// regardless of facility. Single-sourced here so the install-fee model carries no magic numbers.
pub const SCC_SURCHARGE_RATE: f64 = 0.04;
const TILE_ICON_SIZE: Size = Size::S64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlueprintRecipe {
  pub activity_id: i64,
  pub blueprint_type_id: i64,
  pub is_reaction: bool,
  pub output_per_run: i64,
  pub product_type_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CatalogEntry {
  pub category: Category,
  pub group_name: String,
  pub is_reaction: bool,
  pub name: String,
  pub type_id: i64,
  pub volume: f64,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum Category {
  Ammo,
  Component,
  Fuel,
  Module,
  #[default]
  Other,
  Reaction,
  Ship,
}

impl Category {
  pub const PICKER: [Category; 6] = [
    Category::Ship,
    Category::Module,
    Category::Ammo,
    Category::Component,
    Category::Fuel,
    Category::Reaction,
  ];

  fn classify(category_name: &str, group_name: &str, is_reaction: bool) -> Self {
    if is_reaction {
      return Category::Reaction;
    }
    let category = category_name.to_lowercase();
    let group = group_name.to_lowercase();
    if category == "ship" {
      Category::Ship
    } else if category == "module" {
      Category::Module
    } else if category == "charge" {
      Category::Ammo
    } else if group.contains("fuel") || group.contains("ice product") {
      Category::Fuel
    } else if category == "commodity" || category == "material" || group.contains("component") {
      Category::Component
    } else {
      Category::Other
    }
  }

  pub fn label(self) -> String {
    match self {
      Category::Ammo => t!("industry.category.ammo"),
      Category::Component => t!("industry.category.component"),
      Category::Fuel => t!("industry.category.fuel"),
      Category::Module => t!("industry.category.module"),
      Category::Other => t!("industry.category.other"),
      Category::Reaction => t!("industry.category.reaction"),
      Category::Ship => t!("industry.category.ship"),
    }
    .into_owned()
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImplantTimeBonuses {
  pub manufacturing: HashMap<i64, f64>,
  pub reaction: HashMap<i64, f64>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnedSummary {
  pub in_scope: bool,
  pub is_original: bool,
  pub material_efficiency: i64,
  pub time_efficiency: i64,
}

/// One installable clone of a pilot: an implant set the build-time math reads. `id` is the ESI `jump_clone_id`;
/// `None` marks the pilot's active clone. `location` is shown as context only — it never constrains the build
/// facility (EVE installs jobs remotely).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanClone {
  pub id: Option<i64>,
  pub implant_names: Vec<String>,
  pub location: Option<String>,
  pub manufacturing_time_bonus: f64,
  pub name: String,
  pub reaction_time_bonus: f64,
}

impl PlanClone {
  pub fn time_bonus(&self, is_reaction: bool) -> f64 {
    if is_reaction {
      self.reaction_time_bonus
    } else {
      self.manufacturing_time_bonus
    }
  }

  pub fn implant_summary(&self) -> String {
    match self.implant_names.first() {
      None => "no implants".to_owned(),
      Some(first) => {
        let count = self.implant_names.len();
        format!("{count} implant{} \u{00B7} {first}", if count == 1 { "" } else { "s" })
      }
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanPilot {
  pub advanced_industry: i64,
  pub clones: Vec<PlanClone>,
  pub id: i64,
  pub industry: i64,
  pub name: String,
  pub portrait: Option<std::path::PathBuf>,
}

impl PlanPilot {
  pub fn clone_named(&self, clone_id: Option<i64>) -> Option<&PlanClone> {
    self.clones.iter().find(|clone| clone.id == clone_id)
  }

  pub fn skill_time_multiplier(&self, is_reaction: bool) -> f64 {
    let advanced = 1.0 - self.advanced_industry as f64 * 0.03;
    if is_reaction {
      advanced
    } else {
      advanced * (1.0 - self.industry as f64 * 0.04)
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct PlannerData {
  // EIV input: CCP adjusted prices, distinct from `prices` (market average). Falls back to average per type.
  pub adjusted_prices: HashMap<i64, f64>,
  pub blueprint_icons: HashMap<(i64, bool), IconResolution>,
  pub catalog: Vec<CatalogEntry>,
  pub facilities: Vec<PlannerFacility>,
  pub names: HashMap<i64, String>,
  pub owned: HashMap<i64, OwnedSummary>,
  pub prices: HashMap<i64, f64>,
  pub recipes: HashMap<i64, Recipe>,
  pub type_icons: HashMap<i64, IconResolution>,
  pub volumes: HashMap<i64, f64>,
}

impl PlannerData {
  pub fn adjusted_price(&self, type_id: i64) -> f64 {
    self
      .adjusted_prices
      .get(&type_id)
      .copied()
      .unwrap_or_else(|| self.price(type_id))
  }

  pub fn blueprint_icon(&self, blueprint_type_id: i64, is_copy: bool) -> &IconResolution {
    self
      .blueprint_icons
      .get(&(blueprint_type_id, is_copy))
      .unwrap_or(&IconResolution::Missing)
  }

  pub fn name(&self, type_id: i64) -> String {
    self
      .names
      .get(&type_id)
      .cloned()
      .unwrap_or_else(|| format!("Type {type_id}"))
  }

  pub fn price(&self, type_id: i64) -> f64 {
    self.prices.get(&type_id).copied().unwrap_or(0.0)
  }

  pub fn recipe(&self, type_id: i64) -> Option<&Recipe> {
    self.recipes.get(&type_id)
  }

  pub fn type_icon(&self, type_id: i64) -> &IconResolution {
    self.type_icons.get(&type_id).unwrap_or(&IconResolution::Missing)
  }

  pub fn volume(&self, type_id: i64) -> f64 {
    self.volumes.get(&type_id).copied().unwrap_or(0.0)
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlannerFacility {
  pub id: i64,
  pub manufacturing_index: Option<f64>,
  pub name: String,
  pub reaction_index: Option<f64>,
  pub region: Option<String>,
  pub security_status: Option<f64>,
  pub solar_system: Option<String>,
  pub solar_system_id: i64,
  pub type_id: Option<i64>,
  pub type_label: Option<String>,
}

impl PlannerFacility {
  pub fn index_for(&self, is_reaction: bool) -> Option<f64> {
    if is_reaction {
      self.reaction_index
    } else {
      self.manufacturing_index
    }
  }

  pub fn to_ref(&self, is_reaction: bool) -> FacilityRef {
    FacilityRef {
      cost_index: self.index_for(is_reaction),
      id: self.id,
      name: self.name.clone(),
      region: self.region.clone(),
      security_status: self.security_status,
      solar_system: self.solar_system.clone().unwrap_or_default(),
      solar_system_id: self.solar_system_id,
      type_id: self.type_id,
      type_label: self.type_label.clone(),
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
  pub activity_id: i64,
  pub blueprint_type_id: i64,
  pub is_reaction: bool,
  pub materials: Vec<Material>,
  pub output_per_run: i64,
  pub time_per_run: i64,
}

#[derive(Clone, Debug, Default)]
pub struct StaticCatalog {
  pub blueprint_icons: HashMap<(i64, bool), IconResolution>,
  pub catalog: Vec<CatalogEntry>,
  pub names: HashMap<i64, String>,
  pub recipes: HashMap<i64, Recipe>,
  pub type_icons: HashMap<i64, IconResolution>,
  pub volumes: HashMap<i64, f64>,
}

impl StaticCatalog {
  pub fn from_planner_data(data: &PlannerData) -> Self {
    StaticCatalog {
      blueprint_icons: data.blueprint_icons.clone(),
      catalog: data.catalog.clone(),
      names: data.names.clone(),
      recipes: data.recipes.clone(),
      type_icons: data.type_icons.clone(),
      volumes: data.volumes.clone(),
    }
  }
}

#[expect(dead_code, reason = "Foundation for the not-yet-wired build planner UI.")]
pub async fn average_price(db: &Database, type_id: i64) -> Option<f64> {
  prices(db).await.get(&type_id).copied()
}

#[cfg_attr(
  not(test),
  expect(dead_code, reason = "Foundation for the not-yet-wired build planner UI.")
)]
pub async fn best_owned_blueprint(db: &Database, blueprint_type_id: i64, scope: Scope) -> Option<OwnedBlueprint> {
  let owned = owned_blueprints(db, blueprint_type_id, scope).await;
  rank_best_owned(owned)
}

#[expect(dead_code, reason = "Foundation for the not-yet-wired build planner UI.")]
pub async fn build_time(db: &Database, blueprint_type_id: i64, activity_id: i64) -> Option<(i64, i64)> {
  blueprints::activity_meta(db, blueprint_type_id, activity_id)
    .await
    .ok()
    .flatten()
}

pub async fn adjusted_prices(db: &Database) -> HashMap<i64, f64> {
  finance::market_prices_all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter_map(|price| {
      price
        .adjusted_price()
        .or_else(|| price.average_price())
        .map(|value| (price.type_id(), value))
    })
    .collect()
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

#[cfg_attr(
  not(test),
  expect(dead_code, reason = "Foundation for the not-yet-wired build planner UI.")
)]
pub async fn materials_for(db: &Database, blueprint_type_id: i64, activity_id: i64) -> Vec<Material> {
  blueprints::materials_for_activity(db, blueprint_type_id, activity_id)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(type_id, quantity)| Material::new(type_id, quantity))
    .collect()
}

#[cfg_attr(
  not(test),
  expect(dead_code, reason = "Foundation for the not-yet-wired build planner UI.")
)]
pub async fn output_per_run(db: &Database, blueprint_type_id: i64, activity_id: i64) -> Option<i64> {
  blueprints::output_per_run(db, blueprint_type_id, activity_id)
    .await
    .ok()
    .flatten()
}

pub async fn plan_pilots(db: &Database, identities: &[(i64, String, Option<std::path::PathBuf>)]) -> Vec<PlanPilot> {
  let bonuses = implant_time_bonuses(db).await;
  let mut pilots = Vec::with_capacity(identities.len());
  for (id, name, portrait) in identities {
    let clones = match character::clones(db, *id).await {
      Ok(Some(clones)) => plan_clones(&clones, &bonuses),
      _ => Vec::new(),
    };
    let skills = character::skills(db, *id).await.unwrap_or_default();
    let level = |skill_id: i64| {
      skills
        .iter()
        .find(|skill| skill.skill_id() == skill_id)
        .map(|skill| skill.trained_skill_level())
        .unwrap_or(0)
    };
    pilots.push(PlanPilot {
      advanced_industry: level(SKILL_ADVANCED_INDUSTRY),
      clones,
      id: *id,
      industry: level(SKILL_INDUSTRY),
      name: name.clone(),
      portrait: portrait.clone(),
    });
  }
  pilots
}

pub async fn implant_time_bonuses(db: &Database) -> ImplantTimeBonuses {
  let mut bonuses = ImplantTimeBonuses::default();
  for (type_id, percent) in CURATED_MANUFACTURING_IMPLANTS {
    bonuses.manufacturing.insert(type_id, percent);
  }
  for (type_id, percent) in CURATED_REACTION_IMPLANTS {
    bonuses.reaction.insert(type_id, percent);
  }
  let attributes = [ATTR_MANUFACTURING_TIME_BONUS, ATTR_REACTION_TIME_BONUS];
  for row in sde::implant_time_bonuses(db, &attributes).await.unwrap_or_default() {
    let target = if row.attribute_id == ATTR_REACTION_TIME_BONUS {
      &mut bonuses.reaction
    } else {
      &mut bonuses.manufacturing
    };
    target.insert(row.type_id, row.value.abs());
  }
  bonuses
}

fn plan_clones(clones: &CharacterClones, bonuses: &ImplantTimeBonuses) -> Vec<PlanClone> {
  let mut out = vec![PlanClone {
    id: None,
    implant_names: implant_names(&clones.active),
    location: clones.active.clone.home_location_name().clone(),
    manufacturing_time_bonus: best_bonus(&clones.active, &bonuses.manufacturing),
    name: "Active clone".to_owned(),
    reaction_time_bonus: best_bonus(&clones.active, &bonuses.reaction),
  }];
  for jump in &clones.jump_clones {
    let label = jump
      .clone
      .name()
      .clone()
      .filter(|name| !name.is_empty())
      .or_else(|| jump.clone.location_name().clone())
      .unwrap_or_else(|| format!("Clone {}", jump.clone.jump_clone_id()));
    out.push(PlanClone {
      id: Some(jump.clone.jump_clone_id()),
      implant_names: implant_names(jump),
      location: jump.clone.location_name().clone(),
      manufacturing_time_bonus: best_bonus(jump, &bonuses.manufacturing),
      name: label,
      reaction_time_bonus: best_bonus(jump, &bonuses.reaction),
    });
  }
  out
}

fn best_bonus<C>(clone: &CloneWithImplants<C>, table: &HashMap<i64, f64>) -> f64 {
  clone
    .implants
    .iter()
    .filter_map(|implant| table.get(&implant.type_id()).copied())
    .fold(0.0, f64::max)
}

fn implant_names<C>(clone: &CloneWithImplants<C>) -> Vec<String> {
  clone.implants.iter().map(|implant| implant.name().clone()).collect()
}

pub async fn prices(db: &Database) -> HashMap<i64, f64> {
  finance::market_prices_all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter_map(|price| price.average_price().map(|value| (price.type_id(), value)))
    .collect()
}

#[cfg_attr(
  not(test),
  expect(dead_code, reason = "Foundation for the not-yet-wired build planner UI.")
)]
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

pub async fn load_data(db: &Database, scope: Scope) -> PlannerData {
  let catalog = load_static_catalog(db).await;
  load_data_with_catalog(db, scope, catalog).await
}

pub async fn load_data_with_catalog(db: &Database, scope: Scope, catalog: StaticCatalog) -> PlannerData {
  let adjusted_prices = adjusted_prices(db).await;
  let owned = owned_index(db, &catalog.recipes, scope).await;
  let facilities = planner_facilities(db).await;
  let prices = prices(db).await;

  PlannerData {
    adjusted_prices,
    blueprint_icons: catalog.blueprint_icons,
    catalog: catalog.catalog,
    facilities,
    names: catalog.names,
    owned,
    prices,
    recipes: catalog.recipes,
    type_icons: catalog.type_icons,
    volumes: catalog.volumes,
  }
}

pub async fn load_static_catalog(db: &Database) -> StaticCatalog {
  let started = std::time::Instant::now();
  let recipes = recipes(db).await;
  let referenced: Vec<i64> = referenced_type_ids(&recipes).into_iter().collect();
  let types = sde::catalog_types(db, &referenced).await.unwrap_or_default();

  let mut names: HashMap<i64, String> = HashMap::new();
  let mut volumes: HashMap<i64, f64> = HashMap::new();
  let mut categories: HashMap<i64, (String, String)> = HashMap::new();
  for row in types {
    names.insert(row.id, row.name);
    volumes.insert(row.id, row.packaged_volume.or(row.volume).unwrap_or(0.0));
    categories.insert(row.id, (row.group_name, row.category_name));
  }

  let catalog = catalog(&recipes, &names, &volumes, &categories);
  let icons = images::default_store().icon_index();
  let type_icons = type_icons(&icons, &names);
  let blueprint_icons = blueprint_icons(&icons, &recipes);

  tracing::debug!(
    target: "pod::industry",
    elapsed_ms = started.elapsed().as_millis(),
    recipes = recipes.len(),
    referenced = referenced.len(),
    "built planner static catalog"
  );

  StaticCatalog {
    blueprint_icons,
    catalog,
    names,
    recipes,
    type_icons,
    volumes,
  }
}

fn blueprint_icons(icons: &IconIndex, recipes: &HashMap<i64, Recipe>) -> HashMap<(i64, bool), IconResolution> {
  let mut resolved = HashMap::new();
  for recipe in recipes.values() {
    let blueprint_type_id = recipe.blueprint_type_id;
    for is_copy in [false, true] {
      resolved
        .entry((blueprint_type_id, is_copy))
        .or_insert_with(|| icons.resolve_type_icon(blueprint_type_id, Some(is_copy), TILE_ICON_SIZE));
    }
  }
  resolved
}

fn type_icons(icons: &IconIndex, names: &HashMap<i64, String>) -> HashMap<i64, IconResolution> {
  names
    .keys()
    .map(|&type_id| (type_id, icons.resolve_type_icon(type_id, None, TILE_ICON_SIZE)))
    .collect()
}

fn catalog(
  recipes: &HashMap<i64, Recipe>,
  names: &HashMap<i64, String>,
  volumes: &HashMap<i64, f64>,
  categories: &HashMap<i64, (String, String)>,
) -> Vec<CatalogEntry> {
  let mut catalog: Vec<CatalogEntry> = recipes
    .iter()
    .map(|(&type_id, recipe)| {
      let (group_name, category_name) = categories
        .get(&type_id)
        .cloned()
        .unwrap_or_else(|| (String::new(), String::new()));
      CatalogEntry {
        category: Category::classify(&category_name, &group_name, recipe.is_reaction),
        group_name,
        is_reaction: recipe.is_reaction,
        name: names
          .get(&type_id)
          .cloned()
          .unwrap_or_else(|| format!("Type {type_id}")),
        type_id,
        volume: volumes.get(&type_id).copied().unwrap_or(0.0),
      }
    })
    .collect();
  catalog.sort_by(|a, b| {
    a.name
      .to_lowercase()
      .cmp(&b.name.to_lowercase())
      .then(a.type_id.cmp(&b.type_id))
  });
  catalog
}

async fn owned_index(db: &Database, recipes: &HashMap<i64, Recipe>, scope: Scope) -> HashMap<i64, OwnedSummary> {
  let blueprint_to_products = blueprint_products(recipes);
  let all = blueprints::list_all(db).await.unwrap_or_default();
  let mut owned: HashMap<i64, OwnedSummary> = HashMap::new();

  let mut absorb = |blueprint_type_id: i64, in_scope: bool, is_original: bool, me: i64, te: i64| {
    let Some(products) = blueprint_to_products.get(&blueprint_type_id) else {
      return;
    };
    for &product in products {
      let candidate = OwnedSummary {
        in_scope,
        is_original,
        material_efficiency: me,
        time_efficiency: te,
      };
      owned
        .entry(product)
        .and_modify(|current| {
          if owned_rank(&candidate) > owned_rank(current) {
            *current = candidate;
          }
        })
        .or_insert(candidate);
    }
  };

  for row in all.character_blueprints {
    let in_scope = matches!(scope, Scope::All) || matches!(scope, Scope::Char(id) if id == row.character_id());
    absorb(
      row.type_id(),
      in_scope,
      row.runs() < 0,
      row.material_efficiency(),
      row.time_efficiency(),
    );
  }
  for row in all.corporation_blueprints {
    let in_scope = matches!(scope, Scope::All) || matches!(scope, Scope::Corp(id) if id == row.corporation_id());
    absorb(
      row.type_id(),
      in_scope,
      row.runs() < 0,
      row.material_efficiency(),
      row.time_efficiency(),
    );
  }
  owned
}

async fn planner_facilities(db: &Database) -> Vec<PlannerFacility> {
  let raw = facilities(db).await;
  let labels = facility_type_labels(db, &raw).await;
  let mut reaction_indices: BTreeMap<i64, Option<f64>> = BTreeMap::new();
  let mut out = Vec::new();
  for facility in raw {
    let system = facility.solar_system_id();
    let reaction_index = match reaction_indices.get(&system) {
      Some(index) => *index,
      None => {
        let index = cost_index(db, system, REACTION_ACTIVITY_ID).await;
        reaction_indices.insert(system, index);
        index
      }
    };
    out.push(PlannerFacility {
      id: facility.id(),
      manufacturing_index: facility.manufacturing_index(),
      name: facility.name().clone(),
      reaction_index,
      region: facility.region().clone(),
      security_status: facility.security_status(),
      solar_system: facility.solar_system().clone(),
      solar_system_id: system,
      type_id: facility.type_id(),
      type_label: facility_label(facility.id(), facility.type_id(), &labels),
    });
  }
  out
}

async fn facility_type_labels(db: &Database, facilities: &[Facility]) -> HashMap<i64, String> {
  let type_ids: Vec<i64> = facilities
    .iter()
    .filter(|facility| facility.id() >= MIN_STRUCTURE_ID)
    .filter_map(Facility::type_id)
    .collect();
  sde::type_details_for(db, &type_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, name, _group_id)| (id, name))
    .collect()
}

fn facility_label(id: i64, type_id: Option<i64>, labels: &HashMap<i64, String>) -> Option<String> {
  if id < MIN_STRUCTURE_ID {
    return Some("Station".to_owned());
  }
  type_id.and_then(|type_id| labels.get(&type_id).cloned())
}

/// Builds the product→Recipe map; rows are fetched ORDER BY activity_id so manufacturing (1) arrives before
/// reaction (11) — the early-continue guard then silently drops any reaction row for a product that already has
/// a manufacturing recipe, making manufacturing the canonical path wherever both exist.
async fn recipes(db: &Database) -> HashMap<i64, Recipe> {
  let activities = [MANUFACTURING_ACTIVITY_ID, REACTION_ACTIVITY_ID];
  let products = blueprints::products_for_activities(db, &activities)
    .await
    .unwrap_or_default();
  let materials_by_activity = materials_by_activity(db, &activities).await;
  let time_by_activity = time_by_activity(db, &activities).await;

  let mut recipes: HashMap<i64, Recipe> = HashMap::new();
  for product in products {
    if product.activity_id == REACTION_ACTIVITY_ID && recipes.contains_key(&product.product_type_id) {
      continue;
    }
    let key = (product.blueprint_type_id, product.activity_id);
    let materials = materials_by_activity.get(&key).cloned().unwrap_or_default();
    let time_per_run = time_by_activity.get(&key).copied().unwrap_or(0);
    recipes.insert(
      product.product_type_id,
      Recipe {
        activity_id: product.activity_id,
        blueprint_type_id: product.blueprint_type_id,
        is_reaction: product.activity_id == REACTION_ACTIVITY_ID,
        materials,
        output_per_run: product.quantity.max(1),
        time_per_run,
      },
    );
  }
  recipes
}

async fn materials_by_activity(db: &Database, activity_ids: &[i64]) -> HashMap<(i64, i64), Vec<Material>> {
  let rows = blueprints::materials_for_activities(db, activity_ids)
    .await
    .unwrap_or_default();
  let mut out: HashMap<(i64, i64), Vec<Material>> = HashMap::new();
  for row in rows {
    out
      .entry((row.blueprint_type_id, row.activity_id))
      .or_default()
      .push(Material::new(row.material_type_id, row.quantity));
  }
  out
}

async fn time_by_activity(db: &Database, activity_ids: &[i64]) -> HashMap<(i64, i64), i64> {
  blueprints::activity_meta_for_activities(db, activity_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|meta| ((meta.blueprint_type_id, meta.activity_id), meta.time))
    .collect()
}

fn blueprint_products(recipes: &HashMap<i64, Recipe>) -> HashMap<i64, Vec<i64>> {
  let mut out: HashMap<i64, Vec<i64>> = HashMap::new();
  for (&product, recipe) in recipes {
    out.entry(recipe.blueprint_type_id).or_default().push(product);
  }
  out
}

fn owned_rank(summary: &OwnedSummary) -> (bool, bool, i64) {
  (summary.in_scope, summary.is_original, summary.material_efficiency)
}

fn referenced_type_ids(recipes: &HashMap<i64, Recipe>) -> std::collections::HashSet<i64> {
  let mut ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
  for (&product, recipe) in recipes {
    ids.insert(product);
    for material in &recipe.materials {
      ids.insert(material.type_id);
    }
  }
  ids
}

async fn recipe_for_activity(db: &Database, product_type_id: i64, activity_id: i64) -> Option<BlueprintRecipe> {
  let row = blueprints::recipe_for_activity(db, product_type_id, activity_id)
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
    .execute(db.writer())
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
    .execute(db.writer())
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

  mod best_owned_blueprint {
    use pretty_assertions::assert_eq;

    use super::*;

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
    async fn it_returns_none_when_no_blueprint_is_owned() {
      let db = store::open_test().await.unwrap();

      assert_eq!(super::best_owned_blueprint(&db, HULK_BLUEPRINT, Scope::All).await, None);
    }
  }

  mod category {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_classifies_a_reaction_regardless_of_its_category() {
      assert_eq!(
        Category::classify("Commodity", "Composite Reaction", true),
        Category::Reaction
      );
    }

    #[test]
    fn it_falls_back_to_group_hints_for_fuel_and_components() {
      assert_eq!(Category::classify("Commodity", "Fuel Block", false), Category::Fuel);
      assert_eq!(
        Category::classify("Commodity", "Construction Component", false),
        Category::Component
      );
    }

    #[test]
    fn it_maps_named_categories_to_picker_facets() {
      assert_eq!(Category::classify("Ship", "Mining Barge", false), Category::Ship);
      assert_eq!(Category::classify("Module", "Mining Laser", false), Category::Module);
      assert_eq!(Category::classify("Charge", "Mining Crystal", false), Category::Ammo);
    }
  }

  mod load_data {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_builds_recipes_owned_index_and_catalog() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      insert_product(&db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID, HULK, 1).await;
      insert_material(&db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID, TRITANIUM, 100).await;
      sqlx::query("INSERT INTO item_categories (id, name, published) VALUES (6, 'Ship', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("INSERT INTO item_groups (id, category_id, name, published) VALUES (463, 6, 'Mining Barge', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT INTO item_types (id, group_id, description, name, published, volume) VALUES \
        (?, 463, 'A Hulk.', 'Hulk', 1, 3750.0)",
      )
      .bind(HULK)
      .execute(db.writer())
      .await
      .unwrap();
      blueprints::replace_for_character(
        &db,
        CHARACTER_ID,
        &[character_blueprint(CHARACTER_ID, 1, HULK_BLUEPRINT, -1, 9)],
      )
      .await
      .unwrap();

      let data = super::load_data(&db, Scope::All).await;

      let recipe = data.recipe(HULK).unwrap();
      assert_eq!(recipe.blueprint_type_id, HULK_BLUEPRINT);
      assert_eq!(recipe.materials, vec![Material::new(TRITANIUM, 100)]);

      let owned = data.owned.get(&HULK).unwrap();
      assert_eq!(owned.material_efficiency, 9);
      assert!(owned.is_original);

      let entry = data.catalog.iter().find(|entry| entry.type_id == HULK).unwrap();
      assert_eq!(entry.category, Category::Ship);
      assert_eq!(entry.name, "Hulk");
    }
  }

  mod load_data_with_catalog {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_hulk(db: &Database) {
      insert_product(db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID, HULK, 1).await;
      insert_material(db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID, TRITANIUM, 100).await;
      sqlx::query("INSERT INTO item_categories (id, name, published) VALUES (6, 'Ship', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("INSERT INTO item_groups (id, category_id, name, published) VALUES (463, 6, 'Mining Barge', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT INTO item_types (id, group_id, description, name, published, volume) VALUES \
        (?, 463, 'A Hulk.', 'Hulk', 1, 3750.0)",
      )
      .bind(HULK)
      .execute(db.writer())
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_reuses_the_static_catalog_and_refreshes_the_owned_index() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER_ID).await;
      seed_hulk(&db).await;

      let catalog = super::load_static_catalog(&db).await;
      assert!(catalog.recipes.contains_key(&HULK));
      assert_eq!(catalog.names.get(&HULK).map(String::as_str), Some("Hulk"));

      let before = super::load_data_with_catalog(&db, Scope::All, catalog.clone()).await;
      assert!(before.recipe(HULK).is_some());
      assert!(!before.owned.contains_key(&HULK));

      blueprints::replace_for_character(
        &db,
        CHARACTER_ID,
        &[character_blueprint(CHARACTER_ID, 1, HULK_BLUEPRINT, -1, 9)],
      )
      .await
      .unwrap();

      let after = super::load_data_with_catalog(&db, Scope::All, catalog).await;
      let owned = after.owned.get(&HULK).unwrap();
      assert_eq!(owned.material_efficiency, 9);
    }
  }

  mod implant_time_bonuses {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_implant(db: &Database, type_id: i64, name: &str, dogma: &str) {
      sqlx::query("INSERT OR IGNORE INTO item_categories (id, name, published) VALUES (20, 'Implant', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("INSERT OR IGNORE INTO item_groups (id, category_id, name, published) VALUES (300, 20, 'Cyber', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT INTO item_types (id, group_id, description, name, published, dogma_attributes) \
        VALUES (?, 300, '', ?, 1, ?)",
      )
      .bind(type_id)
      .bind(name)
      .bind(dogma)
      .execute(db.writer())
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_reads_the_manufacturing_and_reaction_bonus_from_dogma_attributes() {
      let db = store::open_test().await.unwrap();
      seed_implant(&db, 99_001, "Custom BX", r#"[{"attribute_id":440,"value":-6.0}]"#).await;
      seed_implant(&db, 99_002, "Custom RF", r#"[{"attribute_id":2660,"value":-3.0}]"#).await;

      let bonuses = super::implant_time_bonuses(&db).await;

      assert_eq!(bonuses.manufacturing.get(&99_001), Some(&6.0));
      assert_eq!(bonuses.reaction.get(&99_002), Some(&3.0));
    }

    #[tokio::test]
    async fn it_keeps_the_curated_fallback_when_the_sde_has_no_dogma_row() {
      let db = store::open_test().await.unwrap();

      let bonuses = super::implant_time_bonuses(&db).await;

      assert_eq!(bonuses.manufacturing.get(&27_171), Some(&4.0));
      assert_eq!(bonuses.reaction.get(&45_746), Some(&4.0));
    }
  }

  mod plan_clone {
    use super::*;

    mod time_bonus {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_the_reaction_bonus_for_a_reaction_and_the_manufacturing_bonus_otherwise() {
        let clone = PlanClone {
          manufacturing_time_bonus: 4.0,
          reaction_time_bonus: 2.0,
          ..PlanClone::default()
        };

        assert_eq!(clone.time_bonus(false), 4.0);
        assert_eq!(clone.time_bonus(true), 2.0);
      }
    }

    mod implant_summary {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_reads_no_implants_for_an_empty_clone() {
        let clone = PlanClone::default();

        assert_eq!(clone.implant_summary(), "no implants");
      }

      #[test]
      fn it_singularizes_a_lone_implant() {
        let clone = PlanClone {
          implant_names: vec!["Ocular Filter".to_owned()],
          ..PlanClone::default()
        };

        assert_eq!(clone.implant_summary(), "1 implant \u{00B7} Ocular Filter");
      }

      #[test]
      fn it_counts_and_names_the_first_of_several() {
        let clone = PlanClone {
          implant_names: vec!["Ocular Filter".to_owned(), "Memory Augmentation".to_owned()],
          ..PlanClone::default()
        };

        assert_eq!(clone.implant_summary(), "2 implants \u{00B7} Ocular Filter");
      }
    }
  }

  mod plan_clones {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{
      CharacterClone, CharacterCloneImplant, CharacterJumpClone, character_clone_view::CharacterClones,
    };

    fn implant(name: &str) -> CharacterCloneImplant {
      CharacterCloneImplant {
        character_id: 42,
        clone_id: None,
        icon: None,
        name: name.to_owned(),
        resolved_icon: IconResolution::Missing,
        type_id: 9899,
      }
    }

    fn clones() -> CharacterClones {
      CharacterClones {
        active: CloneWithImplants {
          clone: CharacterClone {
            character_id: 42,
            home_location_id: 60_003_760,
            home_location_name: Some("Jita IV - Moon 4".to_owned()),
            home_location_type: "station".to_owned(),
            last_clone_jump_date: None,
            last_station_change_date: None,
          },
          implants: vec![implant("Ocular Filter")],
        },
        jump_clones: vec![CloneWithImplants {
          clone: CharacterJumpClone {
            character_id: 42,
            jump_clone_id: 7,
            location_id: 60_008_494,
            location_name: Some("Amarr VIII".to_owned()),
            location_type: "station".to_owned(),
            name: Some("Industry clone".to_owned()),
          },
          implants: Vec::new(),
        }],
      }
    }

    #[test]
    fn it_puts_the_active_clone_first_with_a_null_id() {
      let projected = super::plan_clones(&clones(), &ImplantTimeBonuses::default());

      assert_eq!(projected[0].id, None);
      assert_eq!(projected[0].name, "Active clone");
      assert_eq!(projected[0].implant_names, vec!["Ocular Filter".to_owned()]);
      assert_eq!(projected[0].location, Some("Jita IV - Moon 4".to_owned()));
    }

    #[test]
    fn it_keys_jump_clones_by_their_jump_clone_id() {
      let projected = super::plan_clones(&clones(), &ImplantTimeBonuses::default());

      assert_eq!(projected[1].id, Some(7));
      assert_eq!(projected[1].name, "Industry clone");
    }

    #[test]
    fn it_resolves_the_best_time_bonus_implant_into_the_clone() {
      let mut bonuses = ImplantTimeBonuses::default();
      bonuses.manufacturing.insert(9899, 5.0);

      let projected = super::plan_clones(&clones(), &bonuses);

      assert_eq!(projected[0].manufacturing_time_bonus, 5.0);
      assert_eq!(projected[0].reaction_time_bonus, 0.0);
      assert_eq!(projected[1].manufacturing_time_bonus, 0.0);
    }
  }

  mod plan_pilots {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{CharacterClone, CharacterCloneImplant, CharacterJumpClone, CharacterSkill};

    const PILOT_A: i64 = 42;

    const PILOT_B: i64 = 43;

    fn identity(id: i64, name: &str) -> (i64, String, Option<std::path::PathBuf>) {
      (id, name.to_owned(), None)
    }

    async fn seed_clones_and_skills(db: &Database) {
      seed_character(db, PILOT_A).await;
      let active = CharacterClone {
        character_id: PILOT_A,
        home_location_id: 60_003_760,
        home_location_name: Some("Jita IV - Moon 4".to_owned()),
        home_location_type: "station".to_owned(),
        last_clone_jump_date: None,
        last_station_change_date: None,
      };
      let jump = CharacterJumpClone {
        character_id: PILOT_A,
        jump_clone_id: 7,
        location_id: 60_008_494,
        location_name: Some("Amarr VIII".to_owned()),
        location_type: "station".to_owned(),
        name: Some("Industry clone".to_owned()),
      };
      let implant = CharacterCloneImplant {
        character_id: PILOT_A,
        clone_id: None,
        icon: None,
        name: "Ocular Filter".to_owned(),
        resolved_icon: IconResolution::Missing,
        type_id: 9899,
      };
      character::replace_clones_for_character(db, PILOT_A, &active, &[jump], &[implant])
        .await
        .unwrap();

      let skills = [
        CharacterSkill {
          active_skill_level: 4,
          character_id: PILOT_A,
          skill_id: SKILL_INDUSTRY,
          skillpoints_in_skill: 256_000,
          trained_skill_level: 5,
        },
        CharacterSkill {
          active_skill_level: 3,
          character_id: PILOT_A,
          skill_id: SKILL_ADVANCED_INDUSTRY,
          skillpoints_in_skill: 256_000,
          trained_skill_level: 4,
        },
      ];
      character::replace_skills(db, PILOT_A, &skills).await.unwrap();
    }

    #[tokio::test]
    async fn it_builds_a_pilot_from_synced_clones_and_skills() {
      let db = store::open_test().await.unwrap();
      seed_clones_and_skills(&db).await;

      let pilots = super::plan_pilots(&db, &[identity(PILOT_A, "Miner Joe")]).await;

      assert_eq!(pilots.len(), 1);
      let pilot = &pilots[0];
      assert_eq!(pilot.id, PILOT_A);
      assert_eq!(pilot.name, "Miner Joe");
      assert_eq!(pilot.industry, 5);
      assert_eq!(pilot.advanced_industry, 4);
      assert_eq!(pilot.clones.len(), 2);
      assert_eq!(pilot.clones[0].id, None);
      assert_eq!(pilot.clones[1].id, Some(7));
    }

    #[tokio::test]
    async fn it_still_lists_a_pilot_with_no_synced_clones_or_skills() {
      let db = store::open_test().await.unwrap();

      let pilots = super::plan_pilots(&db, &[identity(PILOT_B, "Hauler Sue")]).await;

      assert_eq!(pilots.len(), 1);
      let pilot = &pilots[0];
      assert_eq!(pilot.id, PILOT_B);
      assert_eq!(pilot.name, "Hauler Sue");
      assert!(pilot.clones.is_empty());
      assert_eq!(pilot.industry, 0);
      assert_eq!(pilot.advanced_industry, 0);
    }

    #[tokio::test]
    async fn it_is_empty_for_no_identities() {
      let db = store::open_test().await.unwrap();

      let pilots = super::plan_pilots(&db, &[]).await;

      assert!(pilots.is_empty());
    }
  }

  mod plan_pilot {
    use super::*;

    mod skill_time_multiplier {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_stacks_industry_and_advanced_industry_for_manufacturing() {
        let pilot = PlanPilot {
          advanced_industry: 5,
          industry: 5,
          ..PlanPilot::default()
        };

        assert!((pilot.skill_time_multiplier(false) - 0.68).abs() < 1e-9);
      }

      #[test]
      fn it_applies_only_advanced_industry_to_reactions() {
        let pilot = PlanPilot {
          advanced_industry: 4,
          industry: 5,
          ..PlanPilot::default()
        };

        assert!((pilot.skill_time_multiplier(true) - 0.88).abs() < 1e-9);
      }

      #[test]
      fn it_is_neutral_with_no_skills() {
        let pilot = PlanPilot::default();

        assert_eq!(pilot.skill_time_multiplier(false), 1.0);
        assert_eq!(pilot.skill_time_multiplier(true), 1.0);
      }
    }
  }

  mod best_bonus {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{CharacterCloneImplant, CharacterJumpClone, character_clone_view::CloneWithImplants};

    fn implant(type_id: i64) -> CharacterCloneImplant {
      CharacterCloneImplant {
        character_id: 42,
        clone_id: None,
        icon: None,
        name: format!("Implant {type_id}"),
        resolved_icon: IconResolution::Missing,
        type_id,
      }
    }

    fn clone_with(type_ids: &[i64]) -> CloneWithImplants<CharacterJumpClone> {
      CloneWithImplants {
        clone: CharacterJumpClone {
          character_id: 42,
          jump_clone_id: 1,
          location_id: 0,
          location_name: None,
          location_type: "station".to_owned(),
          name: None,
        },
        implants: type_ids.iter().copied().map(implant).collect(),
      }
    }

    #[test]
    fn it_keeps_the_strongest_bonus_since_implants_do_not_stack() {
      let mut table = HashMap::new();
      table.insert(10, 2.0);
      table.insert(20, 4.0);
      let clone = clone_with(&[10, 20]);

      assert_eq!(super::best_bonus(&clone, &table), 4.0);
    }

    #[test]
    fn it_is_zero_when_no_implant_carries_the_bonus() {
      let table: HashMap<i64, f64> = HashMap::new();
      let clone = clone_with(&[10, 20]);

      assert_eq!(super::best_bonus(&clone, &table), 0.0);
    }
  }

  mod materials_for {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_empty_for_an_activity_with_no_materials() {
      let db = store::open_test().await.unwrap();

      assert!(
        super::materials_for(&db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID)
          .await
          .is_empty()
      );
    }

    #[tokio::test]
    async fn it_returns_the_recipe_materials_ordered_by_type_id() {
      let db = store::open_test().await.unwrap();
      insert_material(&db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID, 35, 200).await;
      insert_material(&db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID, TRITANIUM, 100).await;

      let materials = super::materials_for(&db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID).await;

      assert_eq!(materials, vec![Material::new(TRITANIUM, 100), Material::new(35, 200)]);
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

  mod owned_rank {
    use super::*;

    fn summary(in_scope: bool, is_original: bool, me: i64) -> OwnedSummary {
      OwnedSummary {
        in_scope,
        is_original,
        material_efficiency: me,
        time_efficiency: 0,
      }
    }

    #[test]
    fn it_orders_scope_then_originality_then_efficiency() {
      let in_scope_bpc = summary(true, false, 0);
      let out_scope_bpo = summary(false, true, 10);
      let in_scope_bpo_low = summary(true, true, 5);
      let in_scope_bpo_high = summary(true, true, 8);

      assert!(super::owned_rank(&in_scope_bpc) > super::owned_rank(&out_scope_bpo));
      assert!(super::owned_rank(&in_scope_bpo_low) > super::owned_rank(&in_scope_bpc));
      assert!(super::owned_rank(&in_scope_bpo_high) > super::owned_rank(&in_scope_bpo_low));
    }
  }

  mod planner_facilities {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::IndustryCostIndex;

    const CHEAP_STATION: i64 = 60_000_002;

    const CHEAP_SYSTEM: i64 = 30_002_187;

    const PRICEY_STATION: i64 = 60_000_001;

    const PRICEY_SYSTEM: i64 = 30_000_142;

    const STATION_TYPE_ID: i64 = 54;

    async fn seed_station(db: &Database, id: i64, solar_system_id: i64, name: &str) {
      sqlx::query("INSERT OR IGNORE INTO regions (id, name) VALUES (10000001, 'Region')")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT OR IGNORE INTO constellations (id, name, position_x, position_y, position_z, region_id) \
        VALUES (20000001, 'Constellation', 0, 0, 0, 10000001)",
      )
      .execute(db.writer())
      .await
      .unwrap();
      sqlx::query(
        "INSERT OR IGNORE INTO solar_systems \
          (id, constellation_id, name, position_x, position_y, position_z, security_status) \
        VALUES (?, 20000001, 'System', 0, 0, 0, 1.0)",
      )
      .bind(solar_system_id)
      .execute(db.writer())
      .await
      .unwrap();
      sqlx::query("INSERT OR IGNORE INTO item_categories (id, name, published) VALUES (3, 'Station', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("INSERT OR IGNORE INTO item_groups (id, category_id, name, published) VALUES (15, 3, 'Station', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT OR IGNORE INTO item_types (id, group_id, name, description, published) \
        VALUES (?, 15, 'Station', '', 1)",
      )
      .bind(STATION_TYPE_ID)
      .execute(db.writer())
      .await
      .unwrap();
      sqlx::query(
        "INSERT INTO stations \
          (id, system_id, type_id, name, max_dockable_ship_volume, office_rental_cost, \
          reprocessing_efficiency, reprocessing_stations_take, services, position_x, position_y, position_z) \
        VALUES (?, ?, ?, ?, 0, 0, 0.5, 0.05, '[]', 0, 0, 0)",
      )
      .bind(id)
      .bind(solar_system_id)
      .bind(STATION_TYPE_ID)
      .bind(name)
      .execute(db.writer())
      .await
      .unwrap();
    }

    fn cost_index(solar_system_id: i64, manufacturing: f64, reaction: f64) -> IndustryCostIndex {
      IndustryCostIndex {
        manufacturing: Some(manufacturing),
        reaction: Some(reaction),
        solar_system_id,
        ..IndustryCostIndex::default()
      }
    }

    #[tokio::test]
    async fn it_carries_facility_id_name_and_type_id_cheapest_first() {
      let db = store::open_test().await.unwrap();
      seed_station(&db, PRICEY_STATION, PRICEY_SYSTEM, "Pricey Station").await;
      seed_station(&db, CHEAP_STATION, CHEAP_SYSTEM, "Cheap Station").await;
      industry::replace_cost_indices(
        &db,
        &[
          cost_index(PRICEY_SYSTEM, 0.09, 0.07),
          cost_index(CHEAP_SYSTEM, 0.02, 0.01),
        ],
      )
      .await
      .unwrap();

      let facilities = super::planner_facilities(&db).await;

      assert_eq!(
        facilities.iter().map(|f| f.id).collect::<Vec<_>>(),
        [CHEAP_STATION, PRICEY_STATION]
      );
      assert_eq!(facilities[0].name, "Cheap Station");
      assert_eq!(facilities[0].type_id, Some(STATION_TYPE_ID));
      assert_eq!(facilities[0].manufacturing_index, Some(0.02));
      assert_eq!(facilities[0].reaction_index, Some(0.01));
      assert_eq!(facilities[1].name, "Pricey Station");
    }
  }

  mod rank_best_owned {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_orders_scope_before_originality_before_material_efficiency() {
      let pool = vec![owned(false, 1, -1, 10), owned(true, 2, 30, 0), owned(true, 3, -1, 5)];

      let best = super::rank_best_owned(pool).unwrap();

      assert_eq!(best.item_id, 3);
    }

    #[test]
    fn it_prefers_a_bpo_over_a_bpc_within_the_same_scope() {
      let pool = vec![owned(true, 1, 30, 10), owned(true, 2, -1, 0)];

      let best = super::rank_best_owned(pool).unwrap();

      assert_eq!(best.item_id, 2);
    }

    #[test]
    fn it_prefers_an_in_scope_blueprint_over_an_out_of_scope_one() {
      let pool = vec![owned(false, 1, -1, 10), owned(true, 2, 30, 0)];

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
    fn it_returns_none_when_no_blueprint_is_owned() {
      assert_eq!(super::rank_best_owned(Vec::new()), None);
    }
  }

  mod reverse_lookup {
    use pretty_assertions::assert_eq;

    use super::*;

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
    async fn it_prefers_manufacturing_over_a_reaction_for_the_same_product() {
      let db = store::open_test().await.unwrap();
      insert_product(&db, HULK_BLUEPRINT, MANUFACTURING_ACTIVITY_ID, HULK, 1).await;
      insert_product(&db, 999, REACTION_ACTIVITY_ID, HULK, 1).await;

      let recipe = super::reverse_lookup(&db, HULK).await.unwrap();

      assert_eq!(recipe.blueprint_type_id, HULK_BLUEPRINT);
      assert!(!recipe.is_reaction);
    }

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
    async fn it_returns_none_for_an_unbuildable_product() {
      let db = store::open_test().await.unwrap();

      assert_eq!(super::reverse_lookup(&db, HULK).await, None);
    }
  }
}
