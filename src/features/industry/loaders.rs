use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::Scope;
use crate::store::{
  Database, images,
  model::{CharacterIndustryJob, CorporationIndustryJob, OwnerType as CredentialOwner},
  repo::{assets, character, finance, industry, org, sde},
};

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
  pub jobs: Vec<IndustryJob>,
  pub roster: Vec<RosterOwner>,
  pub scope: Scope,
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

#[derive(Clone, Debug)]
pub struct RosterOwner {
  pub corp: String,
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

  Loaded {
    jobs,
    roster,
    scope,
  }
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
    let mut caps = SlotCaps::default();
    for member in members_by_corp.get(&corp.id()).into_iter().flatten() {
      if let Some(member_caps) = caps_by_character.get(member) {
        caps.manufacturing += member_caps.manufacturing;
        caps.reactions += member_caps.reactions;
        caps.science += member_caps.science;
      }
    }
    let logo = images::resolve(&images::default_store(), images::ImageKind::CorporationLogo, corp.id());
    roster.push(RosterOwner {
      corp: corp.ticker().to_owned(),
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
  let name = assets::location_name(db, location_id).await.ok().flatten();

  if let Ok(Some(station)) = sde::get_station(db, location_id).await {
    let (system_name, security) = system_meta(db, station.system_id()).await;
    return ResolvedLocation {
      name: Some(station.name().to_owned()),
      security,
      system_name,
    };
  }
  if let Ok(Some(structure)) = sde::get_structure(db, location_id).await {
    let (system_name, security) = system_meta(db, structure.solar_system_id()).await;
    return ResolvedLocation {
      name: Some(structure.name().to_owned()),
      security,
      system_name,
    };
  }
  if let Ok(Some(system)) = sde::get_solar_system(db, location_id).await {
    return ResolvedLocation {
      name: Some(system.name().to_owned()),
      security: Some(system.security_status()),
      system_name: Some(system.name().to_owned()),
    };
  }

  ResolvedLocation {
    name,
    security: None,
    system_name: None,
  }
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

  mod load {
    use super::*;

    #[tokio::test]
    async fn it_loads_each_scope_against_an_empty_store() {
      let db = crate::store::open_test().await.unwrap();

      assert!(super::load(db.clone(), Scope::All).await.jobs.is_empty());
      assert!(super::load(db.clone(), Scope::Char(1)).await.jobs.is_empty());
      assert!(super::load(db.clone(), Scope::Corp(1)).await.jobs.is_empty());
    }
  }
}
