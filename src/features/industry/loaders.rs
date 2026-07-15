use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Utc};

use super::{Scope, planner::FacilityDefaults};
use crate::{
  clients::eve_image::Size,
  store::{
    Database,
    images::{self, IconResolution},
    model::{
      CharacterBlueprint, CharacterIndustryJob, CharacterPlanetLink, CharacterPlanetPin, CharacterPlanetPinContent,
      CharacterPlanetRoute, CorporationBlueprint, CorporationIndustryJob, OwnerType as CredentialOwner, Station,
      Structure,
    },
    repo::{assets, blueprints, character, colonies, finance, industry, org, sde},
  },
};

const JOB_TILE_ICON_SIZE: Size = Size::S64;

const MANUFACTURING_ACTIVITY_ID: i64 = 1;

/// Reaction activity id in the seeded reference (the SDE seed maps "reaction" to 11, not the ESI job id 9); a
/// blueprint whose only product is a reaction renders "n/a" for ME/TE.
const REACTION_ACTIVITY_ID: i64 = 11;

const SECONDS_PER_DAY: i64 = 86_400;

const SECONDS_PER_HOUR: f64 = 3_600.0;

const COMMAND_CENTER_CAPACITY_M3: f64 = 500.0;

const DECAY_BAR_WIDTH_SECONDS: f64 = 900.0;

const EXTRACTOR_DECAY_FACTOR: f64 = 0.012;

const LAUNCHPAD_CAPACITY_M3: f64 = 10_000.0;

const LAUNCHPAD_WARNING_FILL: f32 = 0.9;

const STORAGE_FACILITY_CAPACITY_M3: f64 = 12_000.0;

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

  pub fn label(self) -> String {
    match self {
      Activity::Copy => t!("industry.activity.copy"),
      Activity::Invention => t!("industry.activity.invention"),
      Activity::Manufacturing => t!("industry.activity.manufacturing"),
      Activity::MaterialEfficiency => t!("industry.activity.material_research"),
      Activity::Other => t!("industry.activity.other"),
      Activity::Reactions => t!("industry.activity.reactions"),
      Activity::TimeEfficiency => t!("industry.activity.time_research"),
    }
    .into_owned()
  }

  pub fn short(self) -> String {
    match self {
      Activity::Copy => t!("industry.activity_short.copy"),
      Activity::Invention => t!("industry.activity_short.invention"),
      Activity::Manufacturing => t!("industry.activity_short.manufacturing"),
      Activity::MaterialEfficiency => t!("industry.activity_short.material_efficiency"),
      Activity::Other => t!("industry.activity_short.other"),
      Activity::Reactions => t!("industry.activity_short.reactions"),
      Activity::TimeEfficiency => t!("industry.activity_short.time_efficiency"),
    }
    .into_owned()
  }
}

#[derive(Clone, Debug)]
pub struct Blueprint {
  pub group_name: String,
  pub item_id: i64,
  pub location: String,
  pub material_efficiency: i64,
  pub name: String,
  pub owner: Owner,
  pub product_name: Option<String>,
  pub reaction: bool,
  /// `-1` ⇒ BPO (infinite runs); otherwise remaining runs on a BPC.
  pub runs: i64,
  pub system_name: Option<String>,
  pub time_efficiency: i64,
  pub type_icon: IconResolution,
  pub type_id: i64,
}

impl Blueprint {
  pub fn is_original(&self) -> bool {
    self.runs < 0
  }
}

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
  pub fn label(self) -> String {
    match self {
      ExtractionState::Extracting => t!("industry.extraction_state.extracting"),
      ExtractionState::Fractured => t!("industry.extraction_state.auto_fractured"),
      ExtractionState::Imminent => t!("industry.extraction_state.imminent"),
      ExtractionState::Ready => t!("industry.extraction_state.ready"),
    }
    .into_owned()
  }
}

#[derive(Clone, Debug)]
pub struct Colony {
  pub character_id: i64,
  pub detail: ColonyDetail,
  pub extractor_count: usize,
  pub factory_count: usize,
  pub name: String,
  pub num_pins: i64,
  pub output_name: Option<String>,
  pub output_per_day_nominal: f64,
  pub output_tier: u8,
  pub output_unit_price: f64,
  pub planet_id: i64,
  pub planet_type: String,
  pub program_start: Option<DateTime<Utc>>,
  pub security: Option<f64>,
  pub soonest_expiry: Option<DateTime<Utc>>,
  pub system_name: Option<String>,
  pub upgrade_level: i64,
}

impl Colony {
  pub fn cc_level(&self) -> i64 {
    self.upgrade_level.clamp(0, 5)
  }

  pub fn pin_capacity(&self) -> i64 {
    pin_capacity(self.upgrade_level)
  }

  pub fn expiry_seconds(&self, now: DateTime<Utc>) -> Option<i64> {
    self.soonest_expiry.map(|expiry| (expiry - now).num_seconds())
  }

  pub fn is_import_fed(&self) -> bool {
    self.extractor_count == 0
  }

  pub fn output_per_day(&self, now: DateTime<Utc>) -> f64 {
    match self.state(now) {
      ColonyState::Idle => 0.0,
      _ => self.output_per_day_nominal,
    }
  }

  pub fn progress(&self, now: DateTime<Utc>) -> f32 {
    let (Some(start), Some(expiry)) = (self.program_start, self.soonest_expiry) else {
      return 100.0;
    };
    let span = (expiry - start).num_seconds();
    if span <= 0 {
      return 100.0;
    }
    let elapsed = (now - start).num_seconds().max(0);
    ((elapsed as f32 / span as f32) * 100.0).clamp(0.0, 100.0)
  }

  pub fn state(&self, now: DateTime<Utc>) -> ColonyState {
    if self.is_import_fed() {
      return ColonyState::Processing;
    }
    match self.soonest_expiry {
      None => ColonyState::Processing,
      Some(expiry) => {
        let remaining = (expiry - now).num_seconds();
        if remaining <= 0 {
          ColonyState::Idle
        } else if remaining < SECONDS_PER_DAY {
          ColonyState::ExpiringSoon
        } else {
          ColonyState::Extracting
        }
      }
    }
  }

  pub fn value_per_day(&self, now: DateTime<Utc>) -> f64 {
    self.output_per_day(now) * self.output_unit_price
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColonyState {
  Extracting,
  ExpiringSoon,
  Idle,
  Processing,
}

impl ColonyState {
  pub fn label(self) -> String {
    match self {
      ColonyState::Extracting => t!("industry.colony_state.extracting"),
      ColonyState::ExpiringSoon => t!("industry.colony_state.expiring_soon"),
      ColonyState::Idle => t!("industry.colony_state.idle"),
      ColonyState::Processing => t!("industry.colony_state.processing"),
    }
    .into_owned()
  }
}

#[derive(Clone, Debug, Default)]
pub struct ChainCommodity {
  pub name: String,
  pub tier: u8,
}

#[derive(Clone, Debug, Default)]
pub struct ChainTier {
  pub commodities: Vec<ChainCommodity>,
  pub tier: u8,
}

#[derive(Clone, Debug, Default)]
pub struct ColonyDetail {
  pub chain: Vec<ChainTier>,
  pub factories: Vec<ColonyFactory>,
  pub heads: Vec<ExtractorHead>,
  pub launchpad: LaunchpadBuffer,
}

#[derive(Clone, Debug, Default)]
pub struct ColonyFactory {
  pub active: bool,
  pub input_names: Vec<String>,
  pub output_name: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ExtractorHead {
  pub cycle_time_seconds: i64,
  pub expiry: Option<DateTime<Utc>>,
  pub product_name: Option<String>,
  pub program_start: Option<DateTime<Utc>>,
  pub qty_per_cycle: i64,
}

impl ExtractorHead {
  pub fn cycle_hours(&self) -> f64 {
    self.cycle_time_seconds as f64 / SECONDS_PER_HOUR
  }

  pub fn decayed_rate(&self, now: DateTime<Utc>) -> f64 {
    if let Some(expiry) = self.expiry
      && now >= expiry
    {
      return 0.0;
    }
    let elapsed = self
      .program_start
      .map(|start| (now - start).num_seconds().max(0))
      .unwrap_or(0);
    self.peak_rate() * extractor_decay_ratio(self.cycle_time_seconds, elapsed)
  }

  pub fn peak_rate(&self) -> f64 {
    if self.cycle_time_seconds > 0 {
      self.qty_per_cycle as f64 * SECONDS_PER_HOUR / self.cycle_time_seconds as f64
    } else {
      0.0
    }
  }

  pub fn time_until_dry(&self, now: DateTime<Utc>) -> Option<i64> {
    self.expiry.map(|expiry| (expiry - now).num_seconds())
  }
}

#[derive(Clone, Debug, Default)]
pub struct LaunchpadBuffer {
  pub fill_fraction: f32,
  pub output_name: Option<String>,
}

impl LaunchpadBuffer {
  pub fn is_nearly_full(&self) -> bool {
    self.fill_fraction >= LAUNCHPAD_WARNING_FILL
  }
}

#[derive(Clone, Debug)]
pub struct IndustryJob {
  pub activity: Activity,
  pub blueprint_icon: IconResolution,
  pub cost: f64,
  pub end_date: String,
  pub facility: String,
  pub installer: String,
  pub installer_id: i64,
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

#[derive(Clone, Debug, Default)]
pub struct Loaded {
  pub blueprints: Vec<Blueprint>,
  pub colonies: Vec<Colony>,
  pub extractions: Vec<Extraction>,
  pub facility_defaults: FacilityDefaults,
  pub jobs: Vec<IndustryJob>,
  pub roster: Vec<RosterOwner>,
  pub scope: Scope,
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

#[derive(Clone, Debug)]
pub struct RosterOwner {
  pub corp: String,
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
pub enum SlotBucket {
  #[default]
  Manufacturing,
  Reactions,
  Science,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SlotCaps {
  pub manufacturing: i64,
  pub reactions: i64,
  pub science: i64,
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

  fn seed(&mut self, blueprint_type_id: i64, reference: BlueprintReference) {
    self.cache.entry(blueprint_type_id).or_insert(reference);
  }
}

#[derive(Clone, Default)]
struct BlueprintReference {
  product_type_id: Option<i64>,
  reaction: bool,
}

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

  async fn prefetch(&mut self, db: &Database, inputs: &[BlueprintInput]) {
    let type_ids = distinct(inputs.iter().map(|input| input.type_id));
    let location_ids = distinct(inputs.iter().map(|input| input.location_id));

    let details = sde::type_details_for(db, &type_ids).await.unwrap_or_default();
    for (id, name, _group_id) in &details {
      self.type_names.seed(*id, Some(name.clone()));
    }
    let group_ids = distinct(details.iter().map(|(_, _, group_id)| *group_id));
    let mut group_name_by_id: HashMap<i64, String> = HashMap::new();
    for (id, name) in sde::group_names_for(db, &group_ids).await.unwrap_or_default() {
      group_name_by_id.insert(id, name);
    }
    for (id, _name, group_id) in &details {
      self.group_names.seed(*id, group_name_by_id.get(group_id).cloned());
    }

    self.prefetch_products(db, &type_ids).await;
    self.prefetch_locations(db, &location_ids).await;
  }

  async fn prefetch_locations(&mut self, db: &Database, location_ids: &[i64]) {
    let stations = sde::stations_for(db, location_ids).await.unwrap_or_default();
    let structures = sde::structures_for(db, location_ids).await.unwrap_or_default();
    let direct_systems = sde::solar_systems_for(db, location_ids).await.unwrap_or_default();

    let mut system_ids: Vec<i64> = stations.iter().map(Station::system_id).collect();
    system_ids.extend(structures.iter().map(Structure::solar_system_id));
    let system_ids = distinct(system_ids.into_iter());
    let mut systems: HashMap<i64, (Option<String>, Option<f64>)> = HashMap::new();
    for system in sde::solar_systems_for(db, &system_ids).await.unwrap_or_default() {
      systems.insert(
        system.id(),
        (Some(system.name().to_owned()), Some(system.security_status())),
      );
    }

    for station in stations {
      let (system_name, security) = systems.get(&station.system_id()).cloned().unwrap_or_default();
      self.locations.seed(
        station.id(),
        ResolvedLocation {
          name: Some(station.name().to_owned()),
          security,
          system_name,
        },
      );
    }
    for structure in structures {
      let (system_name, security) = systems.get(&structure.solar_system_id()).cloned().unwrap_or_default();
      self.locations.seed(
        structure.id(),
        ResolvedLocation {
          name: Some(structure.name().to_owned()),
          security,
          system_name,
        },
      );
    }
    for system in direct_systems {
      let name = system.name().to_owned();
      self.locations.seed(
        system.id(),
        ResolvedLocation {
          name: Some(name.clone()),
          security: Some(system.security_status()),
          system_name: Some(name),
        },
      );
    }
  }

  async fn prefetch_products(&mut self, db: &Database, blueprint_type_ids: &[i64]) {
    let products = blueprints::products_for_blueprints(
      db,
      blueprint_type_ids,
      &[MANUFACTURING_ACTIVITY_ID, REACTION_ACTIVITY_ID],
    )
    .await
    .unwrap_or_default();

    let mut references: HashMap<i64, BlueprintReference> = HashMap::new();
    let mut product_ids = Vec::new();
    for product in products {
      product_ids.push(product.product_type_id);
      let reaction = product.activity_id == REACTION_ACTIVITY_ID;
      let candidate = BlueprintReference {
        product_type_id: Some(product.product_type_id),
        reaction,
      };
      match references.get(&product.blueprint_type_id) {
        Some(current) if !current.reaction => {}
        _ => {
          references.insert(product.blueprint_type_id, candidate);
        }
      }
    }

    for &blueprint_type_id in blueprint_type_ids {
      let reference = references.get(&blueprint_type_id).cloned().unwrap_or_default();
      self.products.seed(blueprint_type_id, reference);
    }

    let product_ids = distinct(product_ids.into_iter());
    for (id, name, _group_id) in sde::type_details_for(db, &product_ids).await.unwrap_or_default() {
      self.type_names.seed(id, Some(name));
    }
  }
}

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

  fn seed(&mut self, type_id: i64, name: Option<String>) {
    self.cache.entry(type_id).or_insert(name);
  }
}

struct JobInput {
  activity_id: i64,
  blueprint_type_id: i64,
  cost: f64,
  end_date: String,
  facility_id: i64,
  installer: String,
  installer_id: i64,
  job_id: i64,
  owner: Owner,
  owner_name: String,
  probability: Option<f64>,
  product_type_id: Option<i64>,
  runs: i64,
  start_date: String,
}

struct LocationNames {
  cache: HashMap<i64, ResolvedLocation>,
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

  fn seed(&mut self, location_id: i64, resolved: ResolvedLocation) {
    self.cache.entry(location_id).or_insert(resolved);
  }
}

#[derive(Clone, Default)]
struct ResolvedLocation {
  name: Option<String>,
  security: Option<f64>,
  system_name: Option<String>,
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

  fn seed(&mut self, type_id: i64, name: Option<String>) {
    self.cache.entry(type_id).or_insert(name);
  }
}

async fn blueprint_reference(db: &Database, blueprint_type_id: i64) -> BlueprintReference {
  let manufacturing = blueprints::blueprint_product(db, blueprint_type_id, MANUFACTURING_ACTIVITY_ID)
    .await
    .ok()
    .flatten();

  if let Some(product_type_id) = manufacturing {
    return BlueprintReference {
      product_type_id: Some(product_type_id),
      reaction: false,
    };
  }

  let reaction = blueprints::blueprint_product(db, blueprint_type_id, REACTION_ACTIVITY_ID)
    .await
    .ok()
    .flatten();

  BlueprintReference {
    product_type_id: reaction,
    reaction: reaction.is_some(),
  }
}

fn distinct(ids: impl Iterator<Item = i64>) -> Vec<i64> {
  let mut seen = HashSet::new();
  ids.filter(|id| seen.insert(*id)).collect()
}

async fn resolve_group_name(db: &Database, type_id: i64) -> Option<String> {
  let item = sde::get_item_type(db, type_id).await.ok().flatten()?;
  sde::get_item_group(db, item.group_id())
    .await
    .ok()
    .flatten()
    .map(|group| group.name().to_owned())
}

async fn collect_blueprints(db: &Database, scope: Scope) -> Vec<Blueprint> {
  let mut inputs = Vec::new();
  match scope {
    Scope::All => {
      let all = blueprints::list_all(db).await.unwrap_or_default();
      inputs.extend(all.character_blueprints.iter().map(BlueprintInput::character));
      inputs.extend(all.corporation_blueprints.iter().map(BlueprintInput::corporation));
    }
    Scope::Char(id) => {
      let rows = blueprints::list_for_character(db, id).await.unwrap_or_default();
      inputs.extend(rows.iter().map(BlueprintInput::character));
    }
    Scope::Corp(id) => {
      let rows = blueprints::list_for_corporation(db, id).await.unwrap_or_default();
      inputs.extend(rows.iter().map(BlueprintInput::corporation));
    }
  }

  let mut resolvers = BlueprintResolvers::new();
  resolvers.prefetch(db, &inputs).await;

  let mut out = Vec::with_capacity(inputs.len());
  for input in inputs {
    out.push(build_blueprint(db, &mut resolvers, input).await);
  }
  out
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

  let is_copy = input.runs >= 0;

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
    type_icon: images::default_store().resolve_type_icon(input.type_id, Some(is_copy), JOB_TILE_ICON_SIZE),
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
  let colonies = collect_colonies(db, &roster, &prices).await;
  let extractions = collect_extractions(db, &mut locations).await;
  let facility_defaults = load_facility_defaults(db).await;

  Loaded {
    blueprints,
    colonies,
    extractions,
    facility_defaults,
    jobs,
    roster,
    scope,
  }
}

async fn load_facility_defaults(db: &Database) -> FacilityDefaults {
  FacilityDefaults {
    manufacturing: industry::default_facility(db, industry::MANUFACTURING_ACTIVITY_ID)
      .await
      .ok()
      .flatten(),
    reactions: industry::default_facility(db, industry::REACTION_ACTIVITY_ID)
      .await
      .ok()
      .flatten(),
  }
}

async fn collect_extractions(db: &Database, locations: &mut LocationNames) -> Vec<Extraction> {
  let corporations = org::all_owned_corporations(db).await.unwrap_or_default();
  let mut out = Vec::new();
  for corporation in &corporations {
    let rows = org::corporation_mining_extractions(db, corporation.id())
      .await
      .unwrap_or_default();
    for row in rows {
      let location = locations.resolve(db, row.structure_id()).await;
      let structure = structure_label(&location, row.structure_id());
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

fn structure_label(location: &ResolvedLocation, structure_id: i64) -> String {
  location
    .name
    .clone()
    .or_else(|| location.system_name.clone())
    .unwrap_or_else(|| format!("Structure {structure_id}"))
}

async fn collect_colonies(db: &Database, roster: &[RosterOwner], prices: &HashMap<i64, f64>) -> Vec<Colony> {
  let index = SchematicIndex::load(db).await;
  let mut type_names = TypeNames::new();
  let mut volumes: HashMap<i64, f64> = HashMap::new();
  let mut out = Vec::new();
  for owner in roster.iter().filter(|owner| !owner.is_corporation) {
    let planets = colonies::list_planets_for_character(db, owner.id)
      .await
      .unwrap_or_default();
    if planets.is_empty() {
      continue;
    }
    let pins = colonies::list_pins_for_character(db, owner.id)
      .await
      .unwrap_or_default();
    let contents = colonies::list_pin_contents_for_character(db, owner.id)
      .await
      .unwrap_or_default();
    let content_type_ids = distinct(contents.iter().map(CharacterPlanetPinContent::type_id));
    for (id, volume) in sde::type_volumes_for(db, &content_type_ids).await.unwrap_or_default() {
      volumes.entry(id).or_insert(volume.unwrap_or(0.0));
    }
    let routes = colonies::list_routes_for_character(db, owner.id)
      .await
      .unwrap_or_default();
    let links = colonies::list_links_for_character(db, owner.id)
      .await
      .unwrap_or_default();
    for planet in &planets {
      let planet_pins: Vec<&CharacterPlanetPin> = pins
        .iter()
        .filter(|pin| pin.planet_id() == planet.planet_id())
        .collect();
      let planet_pin_ids: HashSet<i64> = planet_pins.iter().map(|pin| pin.pin_id()).collect();
      let planet_contents: Vec<&CharacterPlanetPinContent> = contents
        .iter()
        .filter(|content| planet_pin_ids.contains(&content.pin_id()))
        .collect();
      let planet_routes: Vec<&CharacterPlanetRoute> = routes
        .iter()
        .filter(|route| route.planet_id() == planet.planet_id())
        .collect();
      let planet_links: Vec<&CharacterPlanetLink> = links
        .iter()
        .filter(|link| link.planet_id() == planet.planet_id())
        .collect();
      let colony = build_colony(
        db,
        &index,
        &ColonyMarket {
          prices,
          volumes: &volumes,
        },
        &mut type_names,
        owner.id,
        planet,
        &ColonyPins {
          contents: &planet_contents,
          links: &planet_links,
          pins: &planet_pins,
          routes: &planet_routes,
        },
      )
      .await;
      out.push(colony);
    }
  }
  out
}

struct ColonyMarket<'a> {
  prices: &'a HashMap<i64, f64>,
  volumes: &'a HashMap<i64, f64>,
}

struct ColonyPins<'a> {
  contents: &'a [&'a CharacterPlanetPinContent],
  links: &'a [&'a CharacterPlanetLink],
  pins: &'a [&'a CharacterPlanetPin],
  routes: &'a [&'a CharacterPlanetRoute],
}

async fn build_colony(
  db: &Database,
  index: &SchematicIndex,
  market: &ColonyMarket<'_>,
  type_names: &mut TypeNames,
  character_id: i64,
  planet: &crate::store::model::CharacterPlanet,
  pins: &ColonyPins<'_>,
) -> Colony {
  let extractors: Vec<&CharacterPlanetPin> = pins
    .pins
    .iter()
    .copied()
    .filter(|pin| pin.product_type_id().is_some())
    .collect();
  let factories: Vec<&CharacterPlanetPin> = pins
    .pins
    .iter()
    .copied()
    .filter(|pin| pin.schematic_id().is_some())
    .collect();
  let storage: Vec<&CharacterPlanetPin> = pins
    .pins
    .iter()
    .copied()
    .filter(|pin| pin.product_type_id().is_none() && pin.schematic_id().is_none())
    .collect();

  let (output_type_id, output_tier) = match colony_output(index, &extractors, &factories) {
    Some((type_id, tier)) => (Some(type_id), tier),
    None => (None, 0),
  };
  let output_name = match output_type_id {
    Some(type_id) => type_names.resolve(db, type_id).await,
    None => None,
  };
  let output_per_day_nominal = output_type_id
    .map(|type_id| colony_output_per_day(index, type_id, &extractors, &factories))
    .unwrap_or(0.0);
  let output_unit_price = output_type_id
    .and_then(|type_id| market.prices.get(&type_id).copied())
    .unwrap_or(0.0);

  let (soonest_expiry, program_start) = colony_expiry(&extractors);
  let (system_name, security) = system_meta(db, planet.solar_system_id()).await;
  let name = system_name
    .clone()
    .unwrap_or_else(|| format!("Planet {}", planet.planet_id()));

  let detail = ColonyDetail {
    chain: build_chain(db, index, type_names, &extractors, &factories, output_type_id).await,
    factories: build_factories(db, index, type_names, &factories, pins.routes, pins.links).await,
    heads: build_heads(db, type_names, &extractors).await,
    launchpad: build_launchpad(db, type_names, output_type_id, &storage, pins.contents, market.volumes).await,
  };

  Colony {
    character_id,
    detail,
    extractor_count: extractors.len(),
    factory_count: factories.len(),
    name,
    num_pins: planet.num_pins(),
    output_name,
    output_per_day_nominal,
    output_tier,
    output_unit_price,
    planet_id: planet.planet_id(),
    planet_type: planet.planet_type().to_owned(),
    program_start,
    security,
    soonest_expiry,
    system_name,
    upgrade_level: planet.upgrade_level(),
  }
}

fn pin_capacity(upgrade_level: i64) -> i64 {
  match upgrade_level.clamp(0, 5) {
    0 => 3,
    1 => 5,
    2 => 6,
    3 => 8,
    4 => 9,
    _ => 12,
  }
}

async fn build_chain(
  db: &Database,
  index: &SchematicIndex,
  type_names: &mut TypeNames,
  extractors: &[&CharacterPlanetPin],
  factories: &[&CharacterPlanetPin],
  output_type_id: Option<i64>,
) -> Vec<ChainTier> {
  let ids = chain_type_ids(index, extractors, factories, output_type_id);
  let mut commodities = Vec::with_capacity(ids.len());
  for id in ids {
    let name = type_names.resolve(db, id).await.unwrap_or_else(|| format!("Type {id}"));
    commodities.push(ChainCommodity {
      name,
      tier: index.tier(id),
    });
  }
  group_tiers(commodities)
}

fn chain_type_ids(
  index: &SchematicIndex,
  extractors: &[&CharacterPlanetPin],
  factories: &[&CharacterPlanetPin],
  output_type_id: Option<i64>,
) -> Vec<i64> {
  let mut seen = HashSet::new();
  let mut ids = Vec::new();
  for pin in extractors {
    if let Some(id) = pin.product_type_id()
      && seen.insert(id)
    {
      ids.push(id);
    }
  }
  for pin in factories {
    let Some(out_id) = pin.schematic_id().and_then(|id| index.output_type(id)) else {
      continue;
    };
    if seen.insert(out_id) {
      ids.push(out_id);
    }
    for input in index.recipe_inputs(out_id) {
      if seen.insert(input) {
        ids.push(input);
      }
    }
  }
  if let Some(id) = output_type_id
    && seen.insert(id)
  {
    ids.push(id);
  }
  ids
}

fn group_tiers(commodities: Vec<ChainCommodity>) -> Vec<ChainTier> {
  let mut by_tier: BTreeMap<u8, Vec<ChainCommodity>> = BTreeMap::new();
  for commodity in commodities {
    by_tier.entry(commodity.tier).or_default().push(commodity);
  }
  by_tier
    .into_iter()
    .map(|(tier, commodities)| ChainTier {
      commodities,
      tier,
    })
    .collect()
}

async fn build_heads(
  db: &Database,
  type_names: &mut TypeNames,
  extractors: &[&CharacterPlanetPin],
) -> Vec<ExtractorHead> {
  let mut heads = Vec::with_capacity(extractors.len());
  for pin in extractors {
    let product_name = match pin.product_type_id() {
      Some(id) => type_names.resolve(db, id).await,
      None => None,
    };
    heads.push(ExtractorHead {
      cycle_time_seconds: pin.cycle_time().unwrap_or(0),
      expiry: pin.expiry_time().as_deref().and_then(parse_time),
      product_name,
      program_start: pin
        .install_time()
        .as_deref()
        .or(pin.last_cycle_start().as_deref())
        .and_then(parse_time),
      qty_per_cycle: pin.qty_per_cycle().unwrap_or(0),
    });
  }
  heads
}

async fn build_factories(
  db: &Database,
  index: &SchematicIndex,
  type_names: &mut TypeNames,
  factories: &[&CharacterPlanetPin],
  routes: &[&CharacterPlanetRoute],
  links: &[&CharacterPlanetLink],
) -> Vec<ColonyFactory> {
  let mut out = Vec::with_capacity(factories.len());
  for pin in factories {
    let output_type_id = pin.schematic_id().and_then(|id| index.output_type(id));
    let output_name = match output_type_id {
      Some(id) => type_names.resolve(db, id).await,
      None => None,
    };
    let mut input_names = Vec::new();
    if let Some(id) = output_type_id {
      for input in index.recipe_inputs(id) {
        if let Some(name) = type_names.resolve(db, input).await {
          input_names.push(name);
        }
      }
    }
    out.push(ColonyFactory {
      active: factory_active(pin.pin_id(), routes, links),
      input_names,
      output_name,
    });
  }
  out
}

fn factory_active(pin_id: i64, routes: &[&CharacterPlanetRoute], links: &[&CharacterPlanetLink]) -> bool {
  let fed = routes.iter().any(|route| route.destination_pin_id() == pin_id);
  let wired = links
    .iter()
    .any(|link| link.source_pin_id() == pin_id || link.destination_pin_id() == pin_id);
  fed && wired
}

async fn build_launchpad(
  db: &Database,
  type_names: &mut TypeNames,
  output_type_id: Option<i64>,
  storage: &[&CharacterPlanetPin],
  contents: &[&CharacterPlanetPinContent],
  volumes: &HashMap<i64, f64>,
) -> LaunchpadBuffer {
  let output_name = match output_type_id {
    Some(id) => type_names.resolve(db, id).await,
    None => None,
  };
  LaunchpadBuffer {
    fill_fraction: storage_fill(storage, contents, volumes),
    output_name,
  }
}

fn storage_fill(
  storage: &[&CharacterPlanetPin],
  contents: &[&CharacterPlanetPinContent],
  volumes: &HashMap<i64, f64>,
) -> f32 {
  let mut used = 0.0;
  let mut capacity = 0.0;
  for pin in storage {
    let Some(pin_capacity) = structure_capacity(pin.type_id()) else {
      continue;
    };
    capacity += pin_capacity;
    for content in contents.iter().filter(|content| content.pin_id() == pin.pin_id()) {
      let volume = volumes.get(&content.type_id()).copied().unwrap_or(0.0);
      used += content.amount().max(0) as f64 * volume;
    }
  }
  if capacity <= 0.0 {
    return 0.0;
  }
  ((used / capacity) as f32).clamp(0.0, 1.0)
}

fn structure_capacity(type_id: i64) -> Option<f64> {
  match type_id {
    2256 | 2542 | 2543 | 2544 | 2552 | 2555 | 2556 | 2557 => Some(LAUNCHPAD_CAPACITY_M3),
    2257 | 2535 | 2536 | 2541 | 2558 | 2560 | 2561 | 2562 => Some(STORAGE_FACILITY_CAPACITY_M3),
    2254 | 2524 | 2525 | 2533 | 2534 | 2549 | 2550 | 2551 => Some(COMMAND_CENTER_CAPACITY_M3),
    _ => None,
  }
}

fn colony_output(
  index: &SchematicIndex,
  extractors: &[&CharacterPlanetPin],
  factories: &[&CharacterPlanetPin],
) -> Option<(i64, u8)> {
  let mut products: Vec<(i64, u8)> = Vec::new();
  for pin in factories {
    if let Some(type_id) = pin.schematic_id().and_then(|id| index.output_type(id)) {
      products.push((type_id, index.tier(type_id)));
    }
  }
  for pin in extractors {
    if let Some(type_id) = pin.product_type_id() {
      products.push((type_id, index.tier(type_id)));
    }
  }
  products.into_iter().max_by_key(|(_, tier)| *tier)
}

fn colony_output_per_day(
  index: &SchematicIndex,
  output_type_id: i64,
  extractors: &[&CharacterPlanetPin],
  factories: &[&CharacterPlanetPin],
) -> f64 {
  if let Some(per_run) = index.per_day(output_type_id) {
    let runners = factories
      .iter()
      .filter(|pin| pin.schematic_id().and_then(|id| index.output_type(id)) == Some(output_type_id))
      .count();
    return per_run * runners as f64;
  }
  extractors
    .iter()
    .filter(|pin| pin.product_type_id() == Some(output_type_id))
    .filter_map(|pin| extractor_per_day(pin))
    .sum()
}

fn colony_expiry(extractors: &[&CharacterPlanetPin]) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
  let mut soonest: Option<DateTime<Utc>> = None;
  let mut start: Option<DateTime<Utc>> = None;
  for pin in extractors {
    let Some(expiry) = pin.expiry_time().as_deref().and_then(parse_time) else {
      continue;
    };
    if soonest.is_none_or(|current| expiry < current) {
      soonest = Some(expiry);
      start = pin
        .install_time()
        .as_deref()
        .or(pin.last_cycle_start().as_deref())
        .and_then(parse_time);
    }
  }
  (soonest, start)
}

fn extractor_per_day(pin: &CharacterPlanetPin) -> Option<f64> {
  let qty = pin.qty_per_cycle()? as f64;
  let cycle = pin.cycle_time()?;
  (cycle > 0).then(|| qty * SECONDS_PER_DAY as f64 / cycle as f64)
}

fn extractor_decay_ratio(cycle_time_seconds: i64, elapsed_seconds: i64) -> f64 {
  if cycle_time_seconds <= 0 {
    return 1.0;
  }
  let bar_width = cycle_time_seconds as f64 / DECAY_BAR_WIDTH_SECONDS;
  let cycle = (elapsed_seconds.max(0) / cycle_time_seconds) as f64;
  let t_now = (cycle + 0.5) * bar_width;
  let t_start = 0.5 * bar_width;
  (1.0 + t_start * EXTRACTOR_DECAY_FACTOR) / (1.0 + t_now * EXTRACTOR_DECAY_FACTOR)
}

struct SchematicIndex {
  output_of: HashMap<i64, i64>,
  recipes: HashMap<i64, SchematicRecipe>,
}

impl SchematicIndex {
  async fn load(db: &Database) -> Self {
    let schematics = sde::all_planet_schematics(db).await.unwrap_or_default();
    let types = sde::all_planet_schematic_types(db).await.unwrap_or_default();
    let cycle_by_id: HashMap<i64, i64> = schematics.iter().map(|row| (row.id, row.cycle_time)).collect();

    let mut inputs: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut outputs: HashMap<i64, (i64, i64)> = HashMap::new();
    for row in &types {
      if row.is_input {
        inputs.entry(row.schematic_id).or_default().push(row.type_id);
      } else {
        outputs.insert(row.schematic_id, (row.type_id, row.quantity));
      }
    }

    let mut output_of = HashMap::new();
    let mut recipes = HashMap::new();
    for (schematic_id, (output_type_id, quantity)) in outputs {
      output_of.insert(schematic_id, output_type_id);
      recipes.insert(
        output_type_id,
        SchematicRecipe {
          cycle_time: cycle_by_id.get(&schematic_id).copied().unwrap_or_default(),
          inputs: inputs.remove(&schematic_id).unwrap_or_default(),
          quantity,
        },
      );
    }
    SchematicIndex {
      output_of,
      recipes,
    }
  }

  fn output_type(&self, schematic_id: i64) -> Option<i64> {
    self.output_of.get(&schematic_id).copied()
  }

  fn recipe_inputs(&self, output_type_id: i64) -> Vec<i64> {
    self
      .recipes
      .get(&output_type_id)
      .map(|recipe| recipe.inputs.clone())
      .unwrap_or_default()
  }

  fn per_day(&self, output_type_id: i64) -> Option<f64> {
    self
      .recipes
      .get(&output_type_id)
      .filter(|recipe| recipe.cycle_time > 0)
      .map(|recipe| recipe.quantity as f64 * SECONDS_PER_DAY as f64 / recipe.cycle_time as f64)
  }

  fn tier(&self, type_id: i64) -> u8 {
    self.tier_depth(type_id, 0)
  }

  fn tier_depth(&self, type_id: i64, depth: u8) -> u8 {
    if depth >= 8 {
      return depth;
    }
    match self.recipes.get(&type_id) {
      None => 0,
      Some(recipe) => {
        1 + recipe
          .inputs
          .iter()
          .map(|input| self.tier_depth(*input, depth + 1))
          .max()
          .unwrap_or(0)
      }
    }
  }
}

struct SchematicRecipe {
  cycle_time: i64,
  inputs: Vec<i64>,
  quantity: i64,
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
  let installer_id = row.installer_id();
  let installer = installer_name(db, type_names, installer_id).await;
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
      installer_id,
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
  let installer_id = row.installer_id();
  let installer = installer_name(db, type_names, installer_id).await;
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
      installer_id,
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
    blueprint_icon: images::default_store().resolve_type_icon(input.blueprint_type_id, Some(false), JOB_TILE_ICON_SIZE),
    cost: input.cost,
    end_date: input.end_date,
    facility,
    installer: input.installer,
    installer_id: input.installer_id,
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
  for character in &characters {
    let corp = org::get_corporation(db, character.corporation_id())
      .await
      .ok()
      .flatten()
      .map(|corp| corp.ticker().to_owned())
      .unwrap_or_default();
    let caps = slot_caps(db, character.id()).await;
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

  for corp in &corporations {
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
      slots: SlotCaps::default(),
    });
  }

  roster
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
  let skills = character::skills(db, character_id, Utc::now())
    .await
    .unwrap_or_default();
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
      blueprint_icon: IconResolution::Missing,
      cost: 0.0,
      end_date: end.to_owned(),
      facility: "Jita IV".to_owned(),
      installer: "Pilot".to_owned(),
      installer_id: 1,
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

  fn extraction(arrival: Option<&str>, decay: Option<&str>) -> Extraction {
    Extraction {
      chunk_arrival_time: arrival.map(str::to_owned),
      corporation_id: 98,
      extraction_start_time: Some("2026-06-10T00:00:00Z".to_owned()),
      moon_id: 40_000_001,
      moon_name: Some("Moon I".to_owned()),
      natural_decay_time: decay.map(str::to_owned),
      security: Some(0.4),
      structure: "Athanor".to_owned(),
      system_name: Some("Tama".to_owned()),
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

  mod activity {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_buckets_activities_for_slot_usage() {
      assert_eq!(Activity::Manufacturing.bucket(), SlotBucket::Manufacturing);
      assert_eq!(Activity::Reactions.bucket(), SlotBucket::Reactions);
      assert_eq!(Activity::Invention.bucket(), SlotBucket::Science);
    }

    #[test]
    fn it_maps_ids_to_activities() {
      assert_eq!(Activity::from_id(1), Activity::Manufacturing);
      assert_eq!(Activity::from_id(5), Activity::Copy);
      assert_eq!(Activity::from_id(8), Activity::Invention);
      assert_eq!(Activity::from_id(9), Activity::Reactions);
      assert_eq!(Activity::from_id(99), Activity::Other);
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
        installer_id: 1,
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
        installer_id: 9,
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

  mod collect_colonies {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::{Alliance, Bloodline, Character, CharacterPlanet, CharacterPlanetPin, Corporation, Gender, Race},
      repo::{character, colonies},
    };

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

    #[tokio::test]
    async fn it_builds_a_colony_with_extractor_derived_output() {
      let db = crate::store::open_test().await.unwrap();
      let character_id = 42;
      seed_character(&db, character_id).await;
      let planets = vec![planet(character_id, 40_000_001)];
      let pins = vec![extractor_pin(character_id, 40_000_001, 1_001)];
      colonies::replace_for_character(&db, character_id, &planets, &pins, &[], &[], &[])
        .await
        .unwrap();
      let roster = vec![roster_owner(character_id, false, "Pilot")];
      let prices = HashMap::new();

      let colonies = super::collect_colonies(&db, &roster, &prices).await;

      assert_eq!(colonies.len(), 1);
      let colony = &colonies[0];
      assert_eq!(colony.character_id, character_id);
      assert_eq!(colony.planet_id, 40_000_001);
      assert_eq!(colony.extractor_count, 1);
      assert!(colony.output_per_day_nominal > 0.0);
    }

    #[tokio::test]
    async fn it_skips_characters_without_planets() {
      let db = crate::store::open_test().await.unwrap();
      let character_id = 42;
      seed_character(&db, character_id).await;
      let roster = vec![roster_owner(character_id, false, "Pilot")];
      let prices = HashMap::new();

      let colonies = super::collect_colonies(&db, &roster, &prices).await;

      assert!(colonies.is_empty());
    }

    #[tokio::test]
    async fn it_populates_the_colony_detail_from_pins_routes_and_contents() {
      use crate::store::model::{
        CharacterPlanetLink, CharacterPlanetPin, CharacterPlanetPinContent, CharacterPlanetRoute,
      };

      let db = crate::store::open_test().await.unwrap();
      let character_id = 42;
      seed_character(&db, character_id).await;
      let planets = vec![planet(character_id, 40_000_001)];
      let extractor = extractor_pin(character_id, 40_000_001, 1_001);
      let factory = CharacterPlanetPin {
        schematic_id: Some(127),
        product_type_id: None,
        pin_id: 1_002,
        ..extractor_pin(character_id, 40_000_001, 1_002)
      };
      let launchpad = CharacterPlanetPin {
        schematic_id: None,
        product_type_id: None,
        qty_per_cycle: None,
        cycle_time: None,
        pin_id: 1_003,
        ..extractor_pin(character_id, 40_000_001, 1_003)
      };
      let pins = vec![extractor.clone(), factory, launchpad];
      let contents = vec![CharacterPlanetPinContent {
        character_id,
        amount: 400,
        pin_id: 1_003,
        type_id: 2_268,
      }];
      let routes = vec![CharacterPlanetRoute {
        character_id,
        content_type_id: 2_268,
        destination_pin_id: 1_002,
        planet_id: 40_000_001,
        quantity: 3_000.0,
        route_id: 700,
        source_pin_id: 1_001,
      }];
      let links = vec![CharacterPlanetLink {
        character_id,
        destination_pin_id: 1_002,
        link_level: 1,
        planet_id: 40_000_001,
        source_pin_id: 1_001,
      }];
      colonies::replace_for_character(&db, character_id, &planets, &pins, &contents, &routes, &links)
        .await
        .unwrap();
      let roster = vec![roster_owner(character_id, false, "Pilot")];
      let prices = HashMap::new();

      let colonies = super::collect_colonies(&db, &roster, &prices).await;

      assert_eq!(colonies.len(), 1);
      let detail = &colonies[0].detail;
      assert_eq!(detail.heads.len(), 1);
      assert_eq!(detail.factories.len(), 1);
      assert!(detail.factories[0].active);
      assert!(!detail.chain.is_empty());
    }
  }

  fn pin(pin_id: i64, product_type_id: Option<i64>, schematic_id: Option<i64>) -> CharacterPlanetPin {
    CharacterPlanetPin {
      character_id: 1,
      cycle_time: Some(1_800),
      expiry_time: Some("2026-07-20T00:00:00Z".to_owned()),
      head_radius: Some(0.5),
      install_time: Some("2026-07-13T00:00:00Z".to_owned()),
      last_cycle_start: Some("2026-07-13T01:00:00Z".to_owned()),
      latitude: 0.0,
      longitude: 0.0,
      pin_id,
      planet_id: 40_000_001,
      product_type_id,
      qty_per_cycle: Some(9_100),
      schematic_id,
      type_id: 2_848,
    }
  }

  fn schematic_index() -> SchematicIndex {
    let mut output_of = HashMap::new();
    output_of.insert(127, 2_001);
    let mut recipes = HashMap::new();
    recipes.insert(
      2_001,
      SchematicRecipe {
        cycle_time: 1_800,
        inputs: vec![2_268],
        quantity: 20,
      },
    );
    SchematicIndex {
      output_of,
      recipes,
    }
  }

  mod pin_capacity {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_grows_with_the_command_center_level() {
      assert_eq!(super::super::pin_capacity(0), 3);
      assert_eq!(super::super::pin_capacity(5), 12);
      assert!(super::super::pin_capacity(2) < super::super::pin_capacity(4));
    }

    #[test]
    fn it_clamps_out_of_range_levels() {
      assert_eq!(super::super::pin_capacity(9), 12);
      assert_eq!(super::super::pin_capacity(-3), 3);
    }
  }

  mod chain_type_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collects_extractor_factory_input_and_output_types_without_duplicates() {
      let index = schematic_index();
      let extractors = [pin(1, Some(2_268), None)];
      let factories = [pin(2, None, Some(127))];
      let extractor_refs: Vec<&CharacterPlanetPin> = extractors.iter().collect();
      let factory_refs: Vec<&CharacterPlanetPin> = factories.iter().collect();

      let ids = super::super::chain_type_ids(&index, &extractor_refs, &factory_refs, Some(2_001));

      assert!(ids.contains(&2_268));
      assert!(ids.contains(&2_001));
      assert_eq!(ids.iter().filter(|id| **id == 2_001).count(), 1);
    }
  }

  mod group_tiers {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_groups_commodities_by_ascending_tier() {
      let commodities = vec![
        ChainCommodity {
          name: "Reactive Metals".to_owned(),
          tier: 1,
        },
        ChainCommodity {
          name: "Base Metals".to_owned(),
          tier: 0,
        },
      ];

      let tiers = super::super::group_tiers(commodities);

      assert_eq!(tiers.len(), 2);
      assert_eq!(tiers[0].tier, 0);
      assert_eq!(tiers[1].tier, 1);
    }
  }

  mod factory_active {
    use super::*;

    #[test]
    fn it_is_active_only_when_fed_and_wired() {
      let route = CharacterPlanetRoute {
        character_id: 1,
        content_type_id: 2_268,
        destination_pin_id: 2,
        planet_id: 40_000_001,
        quantity: 3_000.0,
        route_id: 700,
        source_pin_id: 1,
      };
      let link = CharacterPlanetLink {
        character_id: 1,
        destination_pin_id: 2,
        link_level: 1,
        planet_id: 40_000_001,
        source_pin_id: 1,
      };
      let routes = vec![&route];
      let links = vec![&link];

      assert!(super::super::factory_active(2, &routes, &links));
      assert!(!super::super::factory_active(2, &[], &links));
      assert!(!super::super::factory_active(2, &routes, &[]));
    }
  }

  mod storage_fill {
    use super::*;

    fn structure_pin(pin_id: i64, type_id: i64) -> CharacterPlanetPin {
      CharacterPlanetPin {
        character_id: 1,
        cycle_time: None,
        expiry_time: None,
        head_radius: None,
        install_time: None,
        last_cycle_start: None,
        latitude: 0.0,
        longitude: 0.0,
        pin_id,
        planet_id: 40_000_001,
        product_type_id: None,
        qty_per_cycle: None,
        schematic_id: None,
        type_id,
      }
    }

    fn pin_content(pin_id: i64, type_id: i64, amount: i64) -> CharacterPlanetPinContent {
      CharacterPlanetPinContent {
        character_id: 1,
        amount,
        pin_id,
        type_id,
      }
    }

    #[test]
    fn it_divides_used_volume_by_the_structure_capacity() {
      let launchpad = structure_pin(1, 2_544);
      let storage = vec![&launchpad];
      let stack = pin_content(1, 2_398, 5_000);
      let contents = vec![&stack];
      let volumes = HashMap::from([(2_398, 0.38)]);

      let fill = super::super::storage_fill(&storage, &contents, &volumes);

      assert!((fill - 0.19).abs() < 1e-6);
    }

    #[test]
    fn it_sums_capacity_across_every_buffer_structure() {
      let launchpad = structure_pin(1, 2_544);
      let facility = structure_pin(2, 2_541);
      let storage = vec![&launchpad, &facility];
      let a = pin_content(1, 2_398, 10_000);
      let b = pin_content(2, 2_398, 10_000);
      let contents = vec![&a, &b];
      let volumes = HashMap::from([(2_398, 1.0)]);

      let fill = super::super::storage_fill(&storage, &contents, &volumes);

      assert!((fill - (20_000.0 / 22_000.0_f32)).abs() < 1e-6);
    }

    #[test]
    fn it_clamps_an_overfull_buffer_to_one() {
      let launchpad = structure_pin(1, 2_544);
      let storage = vec![&launchpad];
      let stack = pin_content(1, 2_398, 1_000_000);
      let contents = vec![&stack];
      let volumes = HashMap::from([(2_398, 1.0)]);

      let fill = super::super::storage_fill(&storage, &contents, &volumes);

      assert!((fill - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn it_skips_pins_without_a_known_structure_capacity() {
      let extractor = structure_pin(1, 2_848);
      let storage = vec![&extractor];
      let stack = pin_content(1, 2_398, 5_000);
      let contents = vec![&stack];
      let volumes = HashMap::from([(2_398, 0.38)]);

      let fill = super::super::storage_fill(&storage, &contents, &volumes);

      assert!(fill.abs() < f32::EPSILON);
    }
  }

  mod structure_capacity {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_maps_each_structure_kind_to_its_capacity() {
      assert_eq!(super::super::structure_capacity(2_544), Some(10_000.0));
      assert_eq!(super::super::structure_capacity(2_541), Some(12_000.0));
      assert_eq!(super::super::structure_capacity(2_524), Some(500.0));
    }

    #[test]
    fn it_returns_none_for_a_non_buffer_pin() {
      assert_eq!(super::super::structure_capacity(2_848), None);
    }
  }

  mod extractor_decay_ratio {
    #[test]
    fn it_starts_at_one_and_falls_across_the_program() {
      let start = super::super::extractor_decay_ratio(3_600, 0);
      let later = super::super::extractor_decay_ratio(3_600, 5 * 3_600);

      assert!((start - 1.0).abs() < 1e-9);
      assert!(later < start);
      assert!(later > 0.0);
    }

    #[test]
    fn it_returns_one_for_a_non_positive_cycle_time() {
      assert!((super::super::extractor_decay_ratio(0, 10_000) - 1.0).abs() < 1e-9);
    }
  }

  mod extractor_head {
    use pretty_assertions::assert_eq;

    use super::*;

    fn head() -> ExtractorHead {
      ExtractorHead {
        cycle_time_seconds: 3_600,
        expiry: Some(super::now() + chrono::Duration::hours(10)),
        product_name: Some("Base Metals".to_owned()),
        program_start: Some(super::now()),
        qty_per_cycle: 9_000,
      }
    }

    #[test]
    fn it_reports_peak_hourly_rate_from_the_cycle() {
      assert_eq!(head().peak_rate(), 9_000.0);
      assert_eq!(head().cycle_hours(), 1.0);
    }

    #[test]
    fn it_decays_below_peak_across_the_program() {
      let head = head();
      let start = head.decayed_rate(super::now());
      let later = head.decayed_rate(super::now() + chrono::Duration::hours(5));

      assert_eq!(start, 9_000.0);
      assert!(later < start);
    }

    #[test]
    fn it_reports_seconds_until_dry() {
      assert_eq!(head().time_until_dry(super::now()), Some(36_000));
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

  mod job_value {
    use pretty_assertions::assert_eq;

    use super::*;

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

    #[test]
    fn it_multiplies_average_price_by_runs() {
      let prices = HashMap::from([(587, 100.0)]);

      assert_eq!(
        super::job_value(Activity::Manufacturing, Some(587), 10, &prices),
        Some(1_000.0)
      );
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

  mod owner_name {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_placeholder_labels_when_absent() {
      let roster: Vec<RosterOwner> = Vec::new();

      assert_eq!(super::owner_name(&roster, Owner::Character(42)), "Pilot 42");
      assert_eq!(super::owner_name(&roster, Owner::Corporation(7)), "Corp 7");
    }

    #[test]
    fn it_resolves_the_matching_roster_entry() {
      let roster = vec![roster_owner(1, false, "Pilot One"), roster_owner(1, true, "Corp One")];

      assert_eq!(super::owner_name(&roster, Owner::Character(1)), "Pilot One");
      assert_eq!(super::owner_name(&roster, Owner::Corporation(1)), "Corp One");
    }
  }

  mod progress {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_to_one_hundred_when_past_the_end() {
      let job = job("2026-06-13T10:00:00Z", "2026-06-13T11:00:00Z");

      assert_eq!(job.progress(now()), 100.0);
    }

    #[test]
    fn it_is_fifty_percent_at_the_midpoint() {
      let job = job("2026-06-13T11:00:00Z", "2026-06-13T13:00:00Z");

      assert_eq!(job.progress(now()), 50.0);
    }
  }

  mod state {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_extracting_when_arrival_is_more_than_a_day_out() {
      let extraction = extraction(Some("2026-06-20T00:00:00Z"), Some("2026-06-22T00:00:00Z"));

      assert_eq!(extraction.state(now()), ExtractionState::Extracting);
    }

    #[test]
    fn it_is_extracting_when_no_timestamps_are_known() {
      let extraction = extraction(None, None);

      assert_eq!(extraction.state(now()), ExtractionState::Extracting);
    }

    #[test]
    fn it_is_fractured_once_the_decay_time_has_passed() {
      let extraction = extraction(Some("2026-06-13T06:00:00Z"), Some("2026-06-13T10:00:00Z"));

      assert_eq!(extraction.state(now()), ExtractionState::Fractured);
    }

    #[test]
    fn it_is_imminent_when_arrival_is_under_a_day_out() {
      let extraction = extraction(Some("2026-06-13T18:00:00Z"), Some("2026-06-15T00:00:00Z"));

      assert_eq!(extraction.state(now()), ExtractionState::Imminent);
    }

    #[test]
    fn it_is_ready_when_the_chunk_has_arrived_but_decay_is_future() {
      let extraction = extraction(Some("2026-06-13T06:00:00Z"), Some("2026-06-14T00:00:00Z"));

      assert_eq!(extraction.state(now()), ExtractionState::Ready);
    }
  }

  mod structure_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_a_synthetic_structure_label() {
      let location = ResolvedLocation::default();

      assert_eq!(super::super::structure_label(&location, 1_000), "Structure 1000");
    }

    #[test]
    fn it_falls_back_to_the_system_name() {
      let location = ResolvedLocation {
        name: None,
        security: None,
        system_name: Some("Tama".to_owned()),
      };

      assert_eq!(super::super::structure_label(&location, 1_000), "Tama");
    }

    #[test]
    fn it_prefers_the_resolved_name() {
      let location = ResolvedLocation {
        name: Some("Athanor Alpha".to_owned()),
        security: None,
        system_name: Some("Tama".to_owned()),
      };

      assert_eq!(super::super::structure_label(&location, 1_000), "Athanor Alpha");
    }
  }
}
