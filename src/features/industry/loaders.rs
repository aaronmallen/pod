use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::Scope;
use crate::store::{
  Database, images,
  model::{
    CharacterBlueprint, CharacterIndustryJob, CorporationBlueprint, CorporationIndustryJob,
    OwnerType as CredentialOwner,
  },
  repo::{assets, blueprints, character, finance, industry, org, sde},
};

/// Manufacturing activity id in the seeded `blueprint_activity_products` reference; resolves a blueprint's product.
const MANUFACTURING_ACTIVITY_ID: i64 = 1;
/// Reaction activity id in the seeded reference (the SDE seed maps "reaction" to 11, not the ESI job id 9); a
/// blueprint whose only product is a reaction renders "n/a" for ME/TE.
const REACTION_ACTIVITY_ID: i64 = 11;
const SECONDS_PER_DAY: i64 = 86_400;

// EVE skill type ids. Each slot bucket sums one base + one advanced skill (see `slot_caps`):
// manufacturing = Mass Production + Advanced, reactions = Mass Reactions + Advanced, science = Laboratory Operation + Advanced.
const ADVANCED_LABORATORY_OPERATION: i64 = 24_624;
const ADVANCED_MASS_PRODUCTION: i64 = 24_625;
const ADVANCED_MASS_REACTIONS: i64 = 45_749;
const LABORATORY_OPERATION: i64 = 3_406;
const MASS_PRODUCTION: i64 = 3_387;
const MASS_REACTIONS: i64 = 45_748;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Activity {
  Copy,
  Invention,
  Manufacturing,
  MaterialEfficiency,
  #[default]
  Other,
  Reactions,
  TimeEfficiency,
}

impl Activity {
  pub fn from_id(id: i64) -> Self {
    match id {
      1 => Activity::Manufacturing,
      3 => Activity::TimeEfficiency,
      4 => Activity::MaterialEfficiency,
      5 => Activity::Copy,
      8 => Activity::Invention,
      9 => Activity::Reactions,
      _ => Activity::Other,
    }
  }

  pub fn bucket(self) -> SlotBucket {
    match self {
      Activity::Manufacturing => SlotBucket::Manufacturing,
      Activity::Reactions => SlotBucket::Reactions,
      Activity::Copy | Activity::Invention | Activity::MaterialEfficiency | Activity::TimeEfficiency => {
        SlotBucket::Science
      }
      Activity::Other => SlotBucket::Science,
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      Activity::Copy => "Copying",
      Activity::Invention => "Invention",
      Activity::Manufacturing => "Manufacturing",
      Activity::MaterialEfficiency => "Material Research",
      Activity::Other => "Other",
      Activity::Reactions => "Reactions",
      Activity::TimeEfficiency => "Time Research",
    }
  }

  pub fn short(self) -> &'static str {
    match self {
      Activity::Copy => "COPY",
      Activity::Invention => "INVENT",
      Activity::Manufacturing => "MANUF",
      Activity::MaterialEfficiency => "ME",
      Activity::Other => "JOB",
      Activity::Reactions => "REACT",
      Activity::TimeEfficiency => "TE",
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Owner {
  Character(i64),
  Corporation(i64),
}

impl Owner {
  pub fn id(self) -> i64 {
    match self {
      Owner::Character(id) | Owner::Corporation(id) => id,
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SlotBucket {
  #[default]
  Manufacturing,
  Reactions,
  Science,
}

#[derive(Clone, Debug, Default)]
pub struct Loaded {
  pub blueprints: Vec<Blueprint>,
  pub extractions: Vec<Extraction>,
  pub jobs: Vec<IndustryJob>,
  pub roster: Vec<RosterOwner>,
  pub scope: Scope,
}

/// A single corporation moon-mining extraction resolved for the Extractions tab.
#[derive(Clone, Debug)]
pub struct Extraction {
  pub chunk_arrival_time: Option<String>,
  pub corporation_id: i64,
  pub extraction_start_time: Option<String>,
  pub moon_id: i64,
  pub moon_name: Option<String>,
  pub natural_decay_time: Option<String>,
  pub security: Option<f64>,
  pub structure: String,
  pub system_name: Option<String>,
}

impl Extraction {
  pub fn arrival(&self) -> Option<DateTime<Utc>> {
    self.chunk_arrival_time.as_deref().and_then(parse_time)
  }

  pub fn decay(&self) -> Option<DateTime<Utc>> {
    self.natural_decay_time.as_deref().and_then(parse_time)
  }

  pub fn moon_label(&self) -> String {
    self
      .moon_name
      .clone()
      .unwrap_or_else(|| format!("Moon {}", self.moon_id))
  }

  pub fn start(&self) -> Option<DateTime<Utc>> {
    self.extraction_start_time.as_deref().and_then(parse_time)
  }

  /// Progress (0..=100) from extraction start to chunk arrival. Missing or zero-length spans render full.
  pub fn progress(&self, now: DateTime<Utc>) -> f32 {
    let (Some(start), Some(arrival)) = (self.start(), self.arrival()) else {
      return 100.0;
    };
    let span = (arrival - start).num_seconds();
    if span <= 0 {
      return 100.0;
    }
    let elapsed = (now - start).num_seconds().max(0);
    ((elapsed as f32 / span as f32) * 100.0).clamp(0.0, 100.0)
  }

  pub fn state(&self, now: DateTime<Utc>) -> ExtractionState {
    match (self.arrival(), self.decay()) {
      (_, Some(decay)) if now >= decay => ExtractionState::Fractured,
      (Some(arrival), _) if now >= arrival => ExtractionState::Ready,
      (Some(arrival), _) if (arrival - now).num_seconds() < SECONDS_PER_DAY => ExtractionState::Imminent,
      _ => ExtractionState::Extracting,
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtractionState {
  Extracting,
  Fractured,
  Imminent,
  Ready,
}

impl ExtractionState {
  pub fn label(self) -> &'static str {
    match self {
      ExtractionState::Extracting => "Extracting",
      ExtractionState::Fractured => "Auto-fractured",
      ExtractionState::Imminent => "Arriving soon",
      ExtractionState::Ready => "Ready to fracture",
    }
  }
}

#[derive(Clone, Debug)]
pub struct IndustryJob {
  pub activity: Activity,
  pub blueprint_type_id: i64,
  pub cost: f64,
  pub end_date: String,
  pub facility: String,
  pub installer: String,
  pub job_id: i64,
  pub owner: Owner,
  pub owner_name: String,
  pub probability: Option<f64>,
  pub product_name: String,
  pub runs: i64,
  pub security: Option<f64>,
  pub start_date: String,
  pub system_name: Option<String>,
  pub value: Option<f64>,
}

impl IndustryJob {
  pub fn end(&self) -> Option<DateTime<Utc>> {
    parse_time(&self.end_date)
  }

  pub fn is_ready(&self, now: DateTime<Utc>) -> bool {
    self.end().map(|end| end <= now).unwrap_or(false)
  }

  pub fn progress(&self, now: DateTime<Utc>) -> f32 {
    let (Some(start), Some(end)) = (parse_time(&self.start_date), self.end()) else {
      return 100.0;
    };
    let span = (end - start).num_seconds();
    if span <= 0 {
      return 100.0;
    }
    let elapsed = (now - start).num_seconds().max(0);
    ((elapsed as f32 / span as f32) * 100.0).clamp(0.0, 100.0)
  }

  pub fn remaining_seconds(&self, now: DateTime<Utc>) -> i64 {
    self.end().map(|end| (end - now).num_seconds().max(0)).unwrap_or(0)
  }
}

/// A single owned blueprint (BPO or BPC) resolved for the Blueprints tab.
#[derive(Clone, Debug)]
pub struct Blueprint {
  /// Blueprint group name (e.g. "Frigate Blueprint"), used for the row subtitle.
  pub group_name: String,
  /// Stable in-game item id, used as the list/window key.
  pub item_id: i64,
  /// Station / structure / system display name.
  pub location: String,
  pub material_efficiency: i64,
  pub name: String,
  pub owner: Owner,
  /// Manufacturing product name (the "makes {Product}" half of the subtitle).
  pub product_name: Option<String>,
  /// True when the blueprint manufactures via a reaction; ME/TE render "n/a".
  pub reaction: bool,
  /// `-1` ⇒ BPO (infinite runs); otherwise remaining runs on a BPC.
  pub runs: i64,
  pub system_name: Option<String>,
  pub time_efficiency: i64,
  pub type_id: i64,
}

impl Blueprint {
  /// A BPO has unlimited runs (`runs == -1`); anything else is a finite-run BPC.
  pub fn is_original(&self) -> bool {
    self.runs < 0
  }
}

/// Manufacturing product + reaction flag for a blueprint type, sourced from the SDE reference tables.
#[derive(Clone, Default)]
struct BlueprintReference {
  product_type_id: Option<i64>,
  reaction: bool,
}

/// Per-load cache of SDE blueprint-product lookups (`blueprint_activity_products`) keyed by blueprint type id.
struct BlueprintProducts {
  cache: HashMap<i64, BlueprintReference>,
}

impl BlueprintProducts {
  fn new() -> Self {
    BlueprintProducts {
      cache: HashMap::new(),
    }
  }

  async fn resolve(&mut self, db: &Database, blueprint_type_id: i64) -> BlueprintReference {
    if let Some(hit) = self.cache.get(&blueprint_type_id) {
      return hit.clone();
    }
    let reference = blueprint_reference(db, blueprint_type_id).await;
    self.cache.insert(blueprint_type_id, reference.clone());
    reference
  }
}

/// Looks up the manufacturing product (and reaction flag) for a blueprint type from the seeded SDE reference.
///
/// `Database.0` (the pool) is public; the Blueprints tab reads the `blueprint_activity_products` reference table
/// directly here rather than through a repo, mirroring how the loaders already join other SDE tables. A blueprint that
/// has only a reaction product (activity id 9) is flagged so the row hides ME/TE.
async fn blueprint_reference(db: &Database, blueprint_type_id: i64) -> BlueprintReference {
  let manufacturing: Option<i64> = sqlx::query_scalar(
    "SELECT product_type_id FROM blueprint_activity_products WHERE blueprint_type_id = ? AND activity_id = ? LIMIT 1",
  )
  .bind(blueprint_type_id)
  .bind(MANUFACTURING_ACTIVITY_ID)
  .fetch_optional(&db.0)
  .await
  .ok()
  .flatten();

  if let Some(product_type_id) = manufacturing {
    return BlueprintReference {
      product_type_id: Some(product_type_id),
      reaction: false,
    };
  }

  let reaction: Option<i64> = sqlx::query_scalar(
    "SELECT product_type_id FROM blueprint_activity_products WHERE blueprint_type_id = ? AND activity_id = ? LIMIT 1",
  )
  .bind(blueprint_type_id)
  .bind(REACTION_ACTIVITY_ID)
  .fetch_optional(&db.0)
  .await
  .ok()
  .flatten();

  BlueprintReference {
    product_type_id: reaction,
    reaction: reaction.is_some(),
  }
}

#[derive(Clone, Debug)]
pub struct RosterOwner {
  pub corp: String,
  /// The character's corporation id; `None` for corporation roster entries.
  pub corporation_id: Option<i64>,
  pub granted_scopes: Option<String>,
  pub id: i64,
  pub is_corporation: bool,
  pub logo: Option<images::ImageState>,
  pub name: String,
  pub portrait: Option<images::ImageState>,
  pub slots: SlotCaps,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SlotCaps {
  pub manufacturing: i64,
  pub reactions: i64,
  pub science: i64,
}

struct LocationNames {
  cache: HashMap<i64, ResolvedLocation>,
}

#[derive(Clone, Default)]
struct ResolvedLocation {
  name: Option<String>,
  security: Option<f64>,
  system_name: Option<String>,
}

impl LocationNames {
  fn new() -> Self {
    LocationNames {
      cache: HashMap::new(),
    }
  }

  async fn resolve(&mut self, db: &Database, location_id: i64) -> ResolvedLocation {
    if let Some(hit) = self.cache.get(&location_id) {
      return hit.clone();
    }
    let resolved = resolve_location(db, location_id).await;
    self.cache.insert(location_id, resolved.clone());
    resolved
  }
}

struct TypeNames {
  cache: HashMap<i64, Option<String>>,
}

impl TypeNames {
  fn new() -> Self {
    TypeNames {
      cache: HashMap::new(),
    }
  }

  async fn resolve(&mut self, db: &Database, type_id: i64) -> Option<String> {
    if let Some(name) = self.cache.get(&type_id) {
      return name.clone();
    }
    let name = sde::get_item_type(db, type_id)
      .await
      .ok()
      .flatten()
      .map(|item| item.name().to_owned());
    self.cache.insert(type_id, name.clone());
    name
  }
}

/// Per-load cache resolving a type id to its item-group name (e.g. "Frigate Blueprint").
struct GroupNames {
  cache: HashMap<i64, Option<String>>,
}

impl GroupNames {
  fn new() -> Self {
    GroupNames {
      cache: HashMap::new(),
    }
  }

  async fn resolve(&mut self, db: &Database, type_id: i64) -> Option<String> {
    if let Some(name) = self.cache.get(&type_id) {
      return name.clone();
    }
    let name = resolve_group_name(db, type_id).await;
    self.cache.insert(type_id, name.clone());
    name
  }
}

async fn resolve_group_name(db: &Database, type_id: i64) -> Option<String> {
  let item = sde::get_item_type(db, type_id).await.ok().flatten()?;
  sde::get_item_group(db, item.group_id())
    .await
    .ok()
    .flatten()
    .map(|group| group.name().to_owned())
}

/// The SDE / name lookup caches shared while resolving a batch of blueprints.
struct BlueprintResolvers {
  group_names: GroupNames,
  locations: LocationNames,
  products: BlueprintProducts,
  type_names: TypeNames,
}

impl BlueprintResolvers {
  fn new() -> Self {
    BlueprintResolvers {
      group_names: GroupNames::new(),
      locations: LocationNames::new(),
      products: BlueprintProducts::new(),
      type_names: TypeNames::new(),
    }
  }
}

async fn collect_blueprints(db: &Database, scope: Scope) -> Vec<Blueprint> {
  let mut resolvers = BlueprintResolvers::new();
  let mut out = Vec::new();
  match scope {
    Scope::All => {
      let all = blueprints::list_all(db).await.unwrap_or_default();
      for row in all.character_blueprints {
        out.push(build_blueprint(db, &mut resolvers, BlueprintInput::character(&row)).await);
      }
      for row in all.corporation_blueprints {
        out.push(build_blueprint(db, &mut resolvers, BlueprintInput::corporation(&row)).await);
      }
    }
    Scope::Char(id) => {
      for row in blueprints::list_for_character(db, id).await.unwrap_or_default() {
        out.push(build_blueprint(db, &mut resolvers, BlueprintInput::character(&row)).await);
      }
    }
    Scope::Corp(id) => {
      for row in blueprints::list_for_corporation(db, id).await.unwrap_or_default() {
        out.push(build_blueprint(db, &mut resolvers, BlueprintInput::corporation(&row)).await);
      }
    }
  }
  out
}

struct BlueprintInput {
  item_id: i64,
  location_id: i64,
  material_efficiency: i64,
  owner: Owner,
  runs: i64,
  time_efficiency: i64,
  type_id: i64,
}

impl BlueprintInput {
  fn character(row: &CharacterBlueprint) -> Self {
    BlueprintInput {
      item_id: row.item_id(),
      location_id: row.location_id(),
      material_efficiency: row.material_efficiency(),
      owner: Owner::Character(row.character_id()),
      runs: row.runs(),
      time_efficiency: row.time_efficiency(),
      type_id: row.type_id(),
    }
  }

  fn corporation(row: &CorporationBlueprint) -> Self {
    BlueprintInput {
      item_id: row.item_id(),
      location_id: row.location_id(),
      material_efficiency: row.material_efficiency(),
      owner: Owner::Corporation(row.corporation_id()),
      runs: row.runs(),
      time_efficiency: row.time_efficiency(),
      type_id: row.type_id(),
    }
  }
}

async fn build_blueprint(db: &Database, resolvers: &mut BlueprintResolvers, input: BlueprintInput) -> Blueprint {
  let name = resolvers
    .type_names
    .resolve(db, input.type_id)
    .await
    .unwrap_or_else(|| format!("Type {}", input.type_id));
  let group_name = resolvers
    .group_names
    .resolve(db, input.type_id)
    .await
    .unwrap_or_default();
  let reference = resolvers.products.resolve(db, input.type_id).await;
  let product_name = match reference.product_type_id {
    Some(id) => resolvers.type_names.resolve(db, id).await,
    None => None,
  };
  let location = resolvers.locations.resolve(db, input.location_id).await;
  let location_label = location
    .name
    .clone()
    .or_else(|| location.system_name.clone())
    .unwrap_or_else(|| format!("Location {}", input.location_id));

  Blueprint {
    group_name,
    item_id: input.item_id,
    location: location_label,
    material_efficiency: input.material_efficiency,
    name,
    owner: input.owner,
    product_name,
    reaction: reference.reaction,
    runs: input.runs,
    system_name: location.system_name,
    time_efficiency: input.time_efficiency,
    type_id: input.type_id,
  }
}

pub(super) async fn load(db: Database, scope: Scope) -> Loaded {
  let db = &db;
  let roster = load_roster(db).await;
  let prices = load_prices(db).await;
  let mut type_names = TypeNames::new();
  let mut locations = LocationNames::new();

  let mut jobs = Vec::new();
  match scope {
    Scope::All => {
      let all = industry::list_all(db).await.unwrap_or_default();
      for row in all.character_jobs {
        jobs.push(character_job(db, &mut type_names, &mut locations, &prices, &roster, row).await);
      }
      for row in all.corporation_jobs {
        jobs.push(corporation_job(db, &mut type_names, &mut locations, &prices, &roster, row).await);
      }
    }
    Scope::Char(id) => {
      for row in industry::list_for_character(db, id).await.unwrap_or_default() {
        jobs.push(character_job(db, &mut type_names, &mut locations, &prices, &roster, row).await);
      }
    }
    Scope::Corp(id) => {
      for row in industry::list_for_corporation(db, id).await.unwrap_or_default() {
        jobs.push(corporation_job(db, &mut type_names, &mut locations, &prices, &roster, row).await);
      }
    }
  }

  let blueprints = collect_blueprints(db, scope).await;
  let extractions = collect_extractions(db, &mut locations).await;

  Loaded {
    blueprints,
    extractions,
    jobs,
    roster,
    scope,
  }
}

/// Collects every owned corporation's moon-mining extractions, resolving each structure / system name.
///
/// Extractions are corp-only data shown regardless of the active scope; the Extractions tab filters them down by scope
/// itself. The structure name falls back to the raw id, and the system name / security come from the moon's joined
/// solar system when present.
async fn collect_extractions(db: &Database, locations: &mut LocationNames) -> Vec<Extraction> {
  let corporations = org::all_owned_corporations(db).await.unwrap_or_default();
  let mut out = Vec::new();
  for corporation in &corporations {
    let rows = org::corporation_mining_extractions(db, corporation.id())
      .await
      .unwrap_or_default();
    for row in rows {
      let location = locations.resolve(db, row.structure_id()).await;
      let structure = location
        .name
        .clone()
        .or_else(|| location.system_name.clone())
        .unwrap_or_else(|| format!("Structure {}", row.structure_id()));
      let system_name = match row.solar_system_id() {
        Some(system_id) => system_meta(db, system_id).await.0,
        None => location.system_name.clone(),
      };
      out.push(Extraction {
        chunk_arrival_time: row.chunk_arrival_time().clone(),
        corporation_id: row.corporation_id(),
        extraction_start_time: row.extraction_start_time().clone(),
        moon_id: row.moon_id(),
        moon_name: row.moon_name().clone(),
        natural_decay_time: row.natural_decay_time().clone(),
        security: row.security_status(),
        structure,
        system_name,
      });
    }
  }
  out
}

async fn character_job(
  db: &Database,
  type_names: &mut TypeNames,
  locations: &mut LocationNames,
  prices: &HashMap<i64, f64>,
  roster: &[RosterOwner],
  row: CharacterIndustryJob,
) -> IndustryJob {
  let owner = Owner::Character(row.character_id());
  let owner_name = owner_name(roster, owner);
  let installer = installer_name(db, type_names, row.installer_id()).await;
  build_job(
    db,
    type_names,
    locations,
    prices,
    JobInput {
      activity_id: row.activity_id(),
      blueprint_type_id: row.blueprint_type_id(),
      cost: row.cost().unwrap_or(0.0),
      end_date: row.end_date().to_owned(),
      facility_id: row.facility_id(),
      installer,
      job_id: row.job_id(),
      owner,
      owner_name,
      probability: row.probability(),
      product_type_id: row.product_type_id(),
      runs: row.runs(),
      start_date: row.start_date().to_owned(),
    },
  )
  .await
}

async fn corporation_job(
  db: &Database,
  type_names: &mut TypeNames,
  locations: &mut LocationNames,
  prices: &HashMap<i64, f64>,
  roster: &[RosterOwner],
  row: CorporationIndustryJob,
) -> IndustryJob {
  let owner = Owner::Corporation(row.corporation_id());
  let owner_name = owner_name(roster, owner);
  let installer = installer_name(db, type_names, row.installer_id()).await;
  build_job(
    db,
    type_names,
    locations,
    prices,
    JobInput {
      activity_id: row.activity_id(),
      blueprint_type_id: row.blueprint_type_id(),
      cost: row.cost().unwrap_or(0.0),
      end_date: row.end_date().to_owned(),
      facility_id: row.facility_id(),
      installer,
      job_id: row.job_id(),
      owner,
      owner_name,
      probability: row.probability(),
      product_type_id: row.product_type_id(),
      runs: row.runs(),
      start_date: row.start_date().to_owned(),
    },
  )
  .await
}

struct JobInput {
  activity_id: i64,
  blueprint_type_id: i64,
  cost: f64,
  end_date: String,
  facility_id: i64,
  installer: String,
  job_id: i64,
  owner: Owner,
  owner_name: String,
  probability: Option<f64>,
  product_type_id: Option<i64>,
  runs: i64,
  start_date: String,
}

async fn build_job(
  db: &Database,
  type_names: &mut TypeNames,
  locations: &mut LocationNames,
  prices: &HashMap<i64, f64>,
  input: JobInput,
) -> IndustryJob {
  let activity = Activity::from_id(input.activity_id);
  let product_name = match input.product_type_id {
    Some(id) => type_names.resolve(db, id).await.unwrap_or_else(|| format!("Type {id}")),
    None => "Unknown product".to_owned(),
  };
  let location = locations.resolve(db, input.facility_id).await;
  let facility = location
    .name
    .clone()
    .or_else(|| location.system_name.clone())
    .unwrap_or_else(|| format!("Facility {}", input.facility_id));
  let value = job_value(activity, input.product_type_id, input.runs, prices);

  IndustryJob {
    activity,
    blueprint_type_id: input.blueprint_type_id,
    cost: input.cost,
    end_date: input.end_date,
    facility,
    installer: input.installer,
    job_id: input.job_id,
    owner: input.owner,
    owner_name: input.owner_name,
    probability: input.probability,
    product_name,
    runs: input.runs,
    security: location.security,
    start_date: input.start_date,
    system_name: location.system_name,
    value,
  }
}

async fn installer_name(db: &Database, type_names: &mut TypeNames, installer_id: i64) -> String {
  let _ = type_names;
  character::get(db, installer_id)
    .await
    .ok()
    .flatten()
    .map(|character| character.name().to_owned())
    .unwrap_or_else(|| format!("Pilot {installer_id}"))
}

fn job_value(activity: Activity, product_type_id: Option<i64>, runs: i64, prices: &HashMap<i64, f64>) -> Option<f64> {
  // Copy and Invention yield blueprint copies / datacores, not a salable product, so they have no market value.
  if matches!(activity, Activity::Copy | Activity::Invention) {
    return None;
  }
  let product = product_type_id?;
  let unit = prices.get(&product).copied()?;
  Some(unit * runs.max(0) as f64)
}

async fn load_prices(db: &Database) -> HashMap<i64, f64> {
  finance::market_prices_all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter_map(|price| price.average_price().map(|value| (price.type_id(), value)))
    .collect()
}

async fn load_roster(db: &Database) -> Vec<RosterOwner> {
  let characters = character::all_owned(db).await.unwrap_or_default();
  let corporations = org::all_owned_corporations(db).await.unwrap_or_default();
  let credentials = crate::store::repo::infra::all(db).await.unwrap_or_default();
  let scopes_by_id: HashMap<i64, Option<String>> = credentials
    .into_iter()
    .filter(|cred| cred.owner_type() == CredentialOwner::Character)
    .map(|cred| (cred.owner_id(), cred.scopes().clone()))
    .collect();

  let mut roster = Vec::with_capacity(characters.len() + corporations.len());
  let mut caps_by_character: HashMap<i64, SlotCaps> = HashMap::new();
  for character in &characters {
    let corp = org::get_corporation(db, character.corporation_id())
      .await
      .ok()
      .flatten()
      .map(|corp| corp.ticker().to_owned())
      .unwrap_or_default();
    let caps = slot_caps(db, character.id()).await;
    caps_by_character.insert(character.id(), caps);
    let portrait = images::resolve(
      &images::default_store(),
      images::ImageKind::CharacterPortrait,
      character.id(),
    );
    roster.push(RosterOwner {
      corp,
      corporation_id: Some(character.corporation_id()),
      granted_scopes: scopes_by_id.get(&character.id()).cloned().flatten(),
      id: character.id(),
      is_corporation: false,
      logo: None,
      name: character.name().to_owned(),
      portrait: Some(portrait),
      slots: caps,
    });
  }

  let members_by_corp = corporation_members(&characters);
  for corp in &corporations {
    let members = members_by_corp.get(&corp.id());
    let caps = aggregate_corp_caps(members.map(Vec::as_slice).unwrap_or_default(), &caps_by_character);
    let logo = images::resolve(&images::default_store(), images::ImageKind::CorporationLogo, corp.id());
    roster.push(RosterOwner {
      corp: corp.ticker().to_owned(),
      corporation_id: None,
      granted_scopes: None,
      id: corp.id(),
      is_corporation: true,
      logo: Some(logo),
      name: corp.name().to_owned(),
      portrait: None,
      slots: caps,
    });
  }

  roster
}

fn aggregate_corp_caps(members: &[i64], caps_by_character: &HashMap<i64, SlotCaps>) -> SlotCaps {
  let mut caps = SlotCaps::default();
  for member in members {
    if let Some(member_caps) = caps_by_character.get(member) {
      caps.manufacturing += member_caps.manufacturing;
      caps.reactions += member_caps.reactions;
      caps.science += member_caps.science;
    }
  }
  caps
}

fn corporation_members(characters: &[crate::store::model::Character]) -> HashMap<i64, Vec<i64>> {
  let mut map: HashMap<i64, Vec<i64>> = HashMap::new();
  for character in characters {
    map.entry(character.corporation_id()).or_default().push(character.id());
  }
  map
}

fn owner_name(roster: &[RosterOwner], owner: Owner) -> String {
  let is_corporation = matches!(owner, Owner::Corporation(_));
  roster
    .iter()
    .find(|entry| entry.id == owner.id() && entry.is_corporation == is_corporation)
    .map(|entry| entry.name.clone())
    .unwrap_or_else(|| match owner {
      Owner::Character(id) => format!("Pilot {id}"),
      Owner::Corporation(id) => format!("Corp {id}"),
    })
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
  DateTime::parse_from_rfc3339(value)
    .ok()
    .map(|dt| dt.with_timezone(&Utc))
}

async fn resolve_location(db: &Database, location_id: i64) -> ResolvedLocation {
  if let Some(resolved) = resolve_direct_location(db, location_id).await {
    return resolved;
  }

  // The id may be a container or ship nested inside the owner's assets (e.g. a blueprint in a
  // station-hangar container). Walk up to the enclosing station / structure / system and resolve
  // that, mirroring how the Assets view names container contents — otherwise we'd show the raw id.
  if let Ok(Some(parent_id)) = assets::enclosing_location_id(db, location_id).await
    && parent_id != location_id
    && let Some(resolved) = resolve_direct_location(db, parent_id).await
  {
    return resolved;
  }

  ResolvedLocation {
    name: assets::location_name(db, location_id).await.ok().flatten(),
    security: None,
    system_name: None,
  }
}

/// Resolve a location id that is directly a station / structure / solar system. `None` when the id
/// is something else (e.g. a container item), which the caller resolves via the container walk.
async fn resolve_direct_location(db: &Database, location_id: i64) -> Option<ResolvedLocation> {
  if let Ok(Some(station)) = sde::get_station(db, location_id).await {
    let (system_name, security) = system_meta(db, station.system_id()).await;
    return Some(ResolvedLocation {
      name: Some(station.name().to_owned()),
      security,
      system_name,
    });
  }
  if let Ok(Some(structure)) = sde::get_structure(db, location_id).await {
    let (system_name, security) = system_meta(db, structure.solar_system_id()).await;
    return Some(ResolvedLocation {
      name: Some(structure.name().to_owned()),
      security,
      system_name,
    });
  }
  if let Ok(Some(system)) = sde::get_solar_system(db, location_id).await {
    return Some(ResolvedLocation {
      name: Some(system.name().to_owned()),
      security: Some(system.security_status()),
      system_name: Some(system.name().to_owned()),
    });
  }
  None
}

async fn slot_caps(db: &Database, character_id: i64) -> SlotCaps {
  let skills = character::skills(db, character_id).await.unwrap_or_default();
  let level = |id: i64| {
    skills
      .iter()
      .find(|skill| skill.skill_id() == id)
      .map(|skill| skill.trained_skill_level())
      .unwrap_or(0)
  };
  // Every character has one free base slot per bucket before any skill levels add more.
  SlotCaps {
    manufacturing: 1 + level(MASS_PRODUCTION) + level(ADVANCED_MASS_PRODUCTION),
    reactions: 1 + level(MASS_REACTIONS) + level(ADVANCED_MASS_REACTIONS),
    science: 1 + level(LABORATORY_OPERATION) + level(ADVANCED_LABORATORY_OPERATION),
  }
}

async fn system_meta(db: &Database, system_id: i64) -> (Option<String>, Option<f64>) {
  match sde::get_solar_system(db, system_id).await {
    Ok(Some(system)) => (Some(system.name().to_owned()), Some(system.security_status())),
    _ => (None, None),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-06-13T12:00:00Z")
      .unwrap()
      .with_timezone(&Utc)
  }

  fn job(start: &str, end: &str) -> IndustryJob {
    IndustryJob {
      activity: Activity::Manufacturing,
      blueprint_type_id: 1,
      cost: 0.0,
      end_date: end.to_owned(),
      facility: "Jita IV".to_owned(),
      installer: "Pilot".to_owned(),
      job_id: 1,
      owner: Owner::Character(1),
      owner_name: "Pilot".to_owned(),
      probability: None,
      product_name: "Rifter".to_owned(),
      runs: 10,
      security: Some(0.9),
      start_date: start.to_owned(),
      system_name: Some("Jita".to_owned()),
      value: None,
    }
  }

  mod progress {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_fifty_percent_at_the_midpoint() {
      let job = job("2026-06-13T11:00:00Z", "2026-06-13T13:00:00Z");

      assert_eq!(job.progress(now()), 50.0);
    }

    #[test]
    fn it_clamps_to_one_hundred_when_past_the_end() {
      let job = job("2026-06-13T10:00:00Z", "2026-06-13T11:00:00Z");

      assert_eq!(job.progress(now()), 100.0);
    }
  }

  mod is_ready {
    use super::*;

    #[test]
    fn it_is_ready_once_the_end_is_reached() {
      let ready = job("2026-06-13T10:00:00Z", "2026-06-13T11:00:00Z");
      let running = job("2026-06-13T11:00:00Z", "2026-06-13T13:00:00Z");

      assert!(ready.is_ready(now()));
      assert!(!running.is_ready(now()));
    }
  }

  mod activity {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_ids_to_activities() {
      assert_eq!(Activity::from_id(1), Activity::Manufacturing);
      assert_eq!(Activity::from_id(5), Activity::Copy);
      assert_eq!(Activity::from_id(8), Activity::Invention);
      assert_eq!(Activity::from_id(9), Activity::Reactions);
      assert_eq!(Activity::from_id(99), Activity::Other);
    }

    #[test]
    fn it_buckets_activities_for_slot_usage() {
      assert_eq!(Activity::Manufacturing.bucket(), SlotBucket::Manufacturing);
      assert_eq!(Activity::Reactions.bucket(), SlotBucket::Reactions);
      assert_eq!(Activity::Invention.bucket(), SlotBucket::Science);
    }
  }

  mod job_value {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_multiplies_average_price_by_runs() {
      let prices = HashMap::from([(587, 100.0)]);

      assert_eq!(
        super::job_value(Activity::Manufacturing, Some(587), 10, &prices),
        Some(1_000.0)
      );
    }

    #[test]
    fn it_is_none_for_invention_and_copy() {
      let prices = HashMap::from([(587, 100.0)]);

      assert_eq!(super::job_value(Activity::Invention, Some(587), 10, &prices), None);
      assert_eq!(super::job_value(Activity::Copy, Some(587), 10, &prices), None);
    }

    #[test]
    fn it_is_none_when_the_product_has_no_price() {
      let prices = HashMap::new();

      assert_eq!(super::job_value(Activity::Manufacturing, Some(587), 10, &prices), None);
    }
  }

  fn roster_owner(id: i64, is_corporation: bool, name: &str) -> RosterOwner {
    RosterOwner {
      corp: "CORP".to_owned(),
      corporation_id: (!is_corporation).then_some(98_000_000),
      granted_scopes: None,
      id,
      is_corporation,
      logo: None,
      name: name.to_owned(),
      portrait: None,
      slots: SlotCaps::default(),
    }
  }

  mod aggregate_corp_caps {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_member_caps_and_skips_unknown_members() {
      let caps_by_character = HashMap::from([
        (
          1,
          SlotCaps {
            manufacturing: 2,
            reactions: 1,
            science: 3,
          },
        ),
        (
          2,
          SlotCaps {
            manufacturing: 4,
            reactions: 0,
            science: 1,
          },
        ),
      ]);

      let caps = super::aggregate_corp_caps(&[1, 2, 99], &caps_by_character);

      assert_eq!(
        caps,
        SlotCaps {
          manufacturing: 6,
          reactions: 1,
          science: 4,
        }
      );
    }
  }

  mod build_job {
    use super::*;

    #[tokio::test]
    async fn it_falls_back_to_placeholder_names_when_unresolved() {
      let db = crate::store::open_test().await.unwrap();
      let mut type_names = TypeNames::new();
      let mut locations = LocationNames::new();
      let prices = HashMap::new();
      let input = JobInput {
        activity_id: 1,
        blueprint_type_id: 1,
        cost: 0.0,
        end_date: "2026-06-13T13:00:00Z".to_owned(),
        facility_id: 60_003_760,
        installer: "Pilot".to_owned(),
        job_id: 1,
        owner: Owner::Character(1),
        owner_name: "Pilot".to_owned(),
        probability: None,
        product_type_id: Some(587),
        runs: 10,
        start_date: "2026-06-13T11:00:00Z".to_owned(),
      };

      let job = super::build_job(&db, &mut type_names, &mut locations, &prices, input).await;

      assert_eq!(job.product_name, "Type 587");
      assert_eq!(job.facility, "Facility 60003760");
    }

    #[tokio::test]
    async fn it_uses_an_unknown_product_label_without_a_type() {
      let db = crate::store::open_test().await.unwrap();
      let mut type_names = TypeNames::new();
      let mut locations = LocationNames::new();
      let prices = HashMap::new();
      let input = JobInput {
        activity_id: 5,
        blueprint_type_id: 1,
        cost: 0.0,
        end_date: "2026-06-13T13:00:00Z".to_owned(),
        facility_id: 1,
        installer: "Pilot".to_owned(),
        job_id: 2,
        owner: Owner::Corporation(9),
        owner_name: "Corp".to_owned(),
        probability: None,
        product_type_id: None,
        runs: 1,
        start_date: "2026-06-13T11:00:00Z".to_owned(),
      };

      let job = super::build_job(&db, &mut type_names, &mut locations, &prices, input).await;

      assert_eq!(job.product_name, "Unknown product");
    }
  }

  mod owner_name {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_the_matching_roster_entry() {
      let roster = vec![roster_owner(1, false, "Pilot One"), roster_owner(1, true, "Corp One")];

      assert_eq!(super::owner_name(&roster, Owner::Character(1)), "Pilot One");
      assert_eq!(super::owner_name(&roster, Owner::Corporation(1)), "Corp One");
    }

    #[test]
    fn it_falls_back_to_placeholder_labels_when_absent() {
      let roster: Vec<RosterOwner> = Vec::new();

      assert_eq!(super::owner_name(&roster, Owner::Character(42)), "Pilot 42");
      assert_eq!(super::owner_name(&roster, Owner::Corporation(7)), "Corp 7");
    }
  }

  mod load {
    use super::*;

    #[tokio::test]
    async fn it_loads_each_scope_against_an_empty_store() {
      let db = crate::store::open_test().await.unwrap();

      let all = super::load(db.clone(), Scope::All).await;
      assert!(all.jobs.is_empty());
      assert!(all.blueprints.is_empty());
      assert!(super::load(db.clone(), Scope::Char(1)).await.jobs.is_empty());
      assert!(super::load(db.clone(), Scope::Corp(1)).await.jobs.is_empty());
    }
  }

  mod blueprint_reference {

    #[tokio::test]
    async fn it_returns_no_product_for_an_unseeded_blueprint() {
      let db = crate::store::open_test().await.unwrap();

      let reference = super::blueprint_reference(&db, 681).await;

      assert!(reference.product_type_id.is_none());
      assert!(!reference.reaction);
    }
  }

  mod build_blueprint {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_falls_back_to_placeholder_labels_when_nothing_resolves() {
      let db = crate::store::open_test().await.unwrap();
      let mut resolvers = BlueprintResolvers::new();
      let input = BlueprintInput {
        item_id: 100,
        location_id: 60_003_760,
        material_efficiency: 10,
        owner: Owner::Character(7),
        runs: 5,
        time_efficiency: 20,
        type_id: 587,
      };

      let blueprint = super::build_blueprint(&db, &mut resolvers, input).await;

      assert_eq!(blueprint.name, "Type 587");
      assert_eq!(blueprint.location, "Location 60003760");
      assert!(blueprint.product_name.is_none());
      assert!(blueprint.group_name.is_empty());
      assert!(!blueprint.reaction);
      assert_eq!(blueprint.item_id, 100);
      assert_eq!(blueprint.material_efficiency, 10);
      assert_eq!(blueprint.time_efficiency, 20);
      assert_eq!(blueprint.runs, 5);
      assert_eq!(blueprint.owner, Owner::Character(7));
    }
  }
}
