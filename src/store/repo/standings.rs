//! Standings catalog query layer: loads raw standings for a character, computes effective standings
//! with social-skill modifiers, and filters the combined faction/corporation/agent catalog.

#![allow(dead_code)]

use std::collections::HashMap;

use sqlx::{QueryBuilder, Sqlite};

use crate::store::{
  Database, Error,
  repo::infra::like_pattern,
  search::{FilterToken, ParsedQuery, parse_with_keys},
};

pub const AVAILABLE_KEYS: &[&str] = &[
  "faction",
  "corp",
  "agent",
  "name",
  "level",
  "type",
  "division",
  "accessible",
  "system",
  "region",
  "sec",
  "near",
  "field",
  "standing",
];

pub const CONNECTIONS_SKILL_ID: i64 = 3359;
pub const CRIMINAL_CONNECTIONS_SKILL_ID: i64 = 3361;
pub const DIPLOMACY_SKILL_ID: i64 = 3357;

pub const SOCIAL_SKILL_COEFFICIENT: f64 = 0.04;

const DEFAULT_LIMIT: i64 = 500;
const FROM_TYPE_AGENT: &str = "agent";
const FROM_TYPE_CORP: &str = "npc_corp";
const FROM_TYPE_FACTION: &str = "faction";
// Canonical EVE NPC corporation id range; player corporations start at 98,000,000.
const NPC_CORP_ID_MAX: i64 = 1_999_999;
const NPC_CORP_ID_MIN: i64 = 1_000_000;

const PIRATE_FACTION_IDS: &[i64] = &[
  500_010, // Guristas Pirates
  500_011, // Angel Cartel
  500_012, // Blood Raiders
  500_019, // Sansha's Nation
  500_020, // Serpentis
];

const RECOGNIZED_KEYS: &[&str] = &[
  "accessible",
  "agent",
  "agents",
  "corp",
  "corporation",
  "corps",
  "datacore",
  "division",
  "fac",
  "faction",
  "factions",
  "field",
  "level",
  "name",
  "near",
  "region",
  "sec",
  "standing",
  "system",
  "type",
];

// Effective standing required to take a mission from an agent of each level, indexed by level (1..=5).
// Confirmed against live EVE agent access thresholds; correctable as constants without schema change.
const REQUIRED_STANDING_BY_LEVEL: &[(i64, f64)] = &[(1, -2.0), (2, 1.0), (3, 3.0), (4, 5.0), (5, 7.0)];

#[derive(Clone, Debug, PartialEq)]
pub struct AgentPage {
  /// Keyset cursor `(name, agent_id)` of the last raw SQL row when the page filled, used to seek the next page.
  /// `None` once the agent rows are exhausted (the page came back shorter than the requested limit).
  pub next_cursor: Option<(String, i64)>,
  pub rows: Vec<CatalogRow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogKind {
  Agent,
  Corporation,
  Faction,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CatalogRow {
  pub accessible: Option<bool>,
  pub agent_type: Option<String>,
  pub corporation_id: Option<i64>,
  pub division: Option<String>,
  pub effective_standing: f64,
  pub faction_id: Option<i64>,
  pub id: i64,
  pub kind: CatalogKind,
  pub level: Option<i64>,
  pub name: String,
  pub raw_standing: f64,
  pub region_name: Option<String>,
  pub security_status: Option<f64>,
  pub system_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SocialSkills {
  pub connections: i64,
  pub criminal_connections: i64,
  pub diplomacy: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FactionClass {
  Empire,
  Other,
  Pirate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StandingComparison {
  AtLeast,
  AtMost,
  GreaterThan,
  LessThan,
}

#[derive(Clone, Debug, sqlx::FromRow)]
struct AgentSql {
  agent_type: Option<String>,
  corporation_id: Option<i64>,
  division: Option<String>,
  faction_id: Option<i64>,
  id: i64,
  level: Option<i64>,
  name: String,
  region_name: Option<String>,
  security_status: Option<f64>,
  system_name: Option<String>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
struct CorpSql {
  faction_id: Option<i64>,
  id: i64,
  name: String,
}

#[derive(Clone, Debug, sqlx::FromRow)]
struct FactionSql {
  corporation_id: Option<i64>,
  id: i64,
  name: String,
}

pub fn accessibility(effective: f64, level: Option<i64>) -> Option<bool> {
  level.map(|level| effective >= required_standing(level))
}

pub fn effective_standing(raw: f64, faction_id: Option<i64>, skills: SocialSkills) -> f64 {
  let class = classify_faction(faction_id);
  if raw < 0.0 {
    let level = skills.diplomacy;
    return raw + (0.0 - raw) * SOCIAL_SKILL_COEFFICIENT * level as f64;
  }
  let level = match class {
    FactionClass::Empire => skills.connections,
    FactionClass::Pirate => skills.criminal_connections,
    FactionClass::Other => 0,
  };
  raw + (10.0 - raw) * SOCIAL_SKILL_COEFFICIENT * level as f64
}

pub fn parse(input: &str) -> ParsedQuery {
  parse_with_keys(input, RECOGNIZED_KEYS)
}

pub fn required_standing(level: i64) -> f64 {
  REQUIRED_STANDING_BY_LEVEL
    .iter()
    .find(|(candidate, _)| *candidate == level)
    .map(|(_, required)| *required)
    .unwrap_or(0.0)
}

pub async fn catalog(
  db: &Database,
  character_id: i64,
  query: &ParsedQuery,
  force_agents: bool,
  limit: Option<i64>,
) -> Result<Vec<CatalogRow>, Error> {
  let mut facets = Facets::from_query(query);
  if facets.near_me {
    facets.near_systems = Some(near_me_systems(db, character_id).await?);
  }
  let skills = social_skills(db, character_id).await?;
  let raw = raw_standings(db, character_id).await?;
  let names = NameIndex::load(db).await?;

  let mut rows = Vec::new();
  rows.extend(faction_rows(db, &raw, skills).await?);
  rows.extend(corporation_rows(db, &raw, skills).await?);
  if force_agents || facets.surfaces_agents() {
    let bound = limit.unwrap_or(DEFAULT_LIMIT);
    rows.extend(agent_rows(db, &facets, &names, &raw, skills, None, bound).await?);
  }

  rows.retain(|row| facets.keeps(row, &names));
  Ok(rows)
}

/// Fetches a single keyset page of agent rows for the standings catalog, seeking past `after`.
///
/// Factions and corporations are never paginated (the full set is small and always loaded by [`catalog`]); this
/// path bounds only the agent rows. Returns no rows when the query does not surface agents and `force_agents` is
/// false; `force_agents` lets a caller surface the full agent catalog with no narrowing facet (e.g. the All/Agents
/// segment filter).
pub async fn agent_page(
  db: &Database,
  character_id: i64,
  query: &ParsedQuery,
  force_agents: bool,
  after: Option<(String, i64)>,
  limit: i64,
) -> Result<AgentPage, Error> {
  let mut facets = Facets::from_query(query);
  if !force_agents && !facets.surfaces_agents() {
    return Ok(AgentPage {
      next_cursor: None,
      rows: Vec::new(),
    });
  }
  if facets.near_me {
    facets.near_systems = Some(near_me_systems(db, character_id).await?);
  }
  let skills = social_skills(db, character_id).await?;
  let raw = raw_standings(db, character_id).await?;
  let names = NameIndex::load(db).await?;

  let mut rows = agent_rows(db, &facets, &names, &raw, skills, after.as_ref(), limit).await?;
  // The raw page count (pre-`keeps`) drives exhaustion; the cursor seeks past the last raw row so a
  // fully-filtered page still advances. `keeps` then drops rows the SQL predicates could not express.
  let next_cursor = (rows.len() as i64 == limit)
    .then(|| rows.last().map(|row| (row.name.clone(), row.id)))
    .flatten();
  rows.retain(|row| facets.keeps(row, &names));

  Ok(AgentPage {
    next_cursor,
    rows,
  })
}

fn classify_faction(faction_id: Option<i64>) -> FactionClass {
  match faction_id {
    None => FactionClass::Other,
    Some(id) if PIRATE_FACTION_IDS.contains(&id) => FactionClass::Pirate,
    Some(_) => FactionClass::Empire,
  }
}

async fn agent_rows(
  db: &Database,
  facets: &Facets,
  names: &NameIndex,
  raw: &RawStandings,
  skills: SocialSkills,
  after: Option<&(String, i64)>,
  limit: i64,
) -> Result<Vec<CatalogRow>, Error> {
  let mut builder = QueryBuilder::<Sqlite>::new(
    "SELECT \
    a.id AS id, \
    a.name AS name, \
    a.corporation_id AS corporation_id, \
    a.level AS level, \
    c.faction_id AS faction_id, \
    at.name AS agent_type, \
    d.name AS division, \
    ss.name AS system_name, \
    ss.security_status AS security_status, \
    r.name AS region_name \
    FROM npc_agents a \
    LEFT JOIN corporations c ON c.id = a.corporation_id \
    LEFT JOIN agent_types at ON at.id = a.agent_type_id \
    LEFT JOIN npc_corporation_divisions d ON d.id = a.division_id \
    LEFT JOIN stations st ON st.id = a.location_id \
    LEFT JOIN solar_systems ss ON ss.id = st.system_id \
    LEFT JOIN constellations co ON co.id = ss.constellation_id \
    LEFT JOIN regions r ON r.id = co.region_id",
  );

  let mut first = true;
  let mut push_clause = |builder: &mut QueryBuilder<Sqlite>, first: &mut bool| {
    builder.push(if *first { " WHERE " } else { " AND " });
    *first = false;
  };

  if !facets.faction_positives.is_empty() {
    let faction_ids = names.factions_matching(&facets.faction_positives);
    push_clause(&mut builder, &mut first);
    builder.push("c.faction_id IN (");
    let mut separated = builder.separated(", ");
    if faction_ids.is_empty() {
      separated.push_bind(i64::MIN);
    }
    for id in &faction_ids {
      separated.push_bind(*id);
    }
    separated.push_unseparated(")");
  }

  if !facets.corp_positives.is_empty() {
    let corp_ids = names.corps_matching(&facets.corp_positives);
    push_clause(&mut builder, &mut first);
    builder.push("a.corporation_id IN (");
    let mut separated = builder.separated(", ");
    if corp_ids.is_empty() {
      separated.push_bind(i64::MIN);
    }
    for id in &corp_ids {
      separated.push_bind(*id);
    }
    separated.push_unseparated(")");
  }

  for value in &facets.agent_names {
    push_clause(&mut builder, &mut first);
    builder.push("a.name LIKE ");
    builder.push_bind(like_pattern(value));
    builder.push(" ESCAPE '\\'");
  }

  for value in &facets.names {
    push_clause(&mut builder, &mut first);
    builder.push("a.name LIKE ");
    builder.push_bind(like_pattern(value));
    builder.push(" ESCAPE '\\'");
  }

  for word in &facets.free_text {
    push_clause(&mut builder, &mut first);
    builder.push("a.name LIKE ");
    builder.push_bind(like_pattern(word));
    builder.push(" ESCAPE '\\'");
  }

  for levels in &facets.levels {
    push_clause(&mut builder, &mut first);
    builder.push("a.level IN (");
    let mut separated = builder.separated(", ");
    for level in levels {
      separated.push_bind(*level);
    }
    separated.push_unseparated(")");
  }

  for values in &facets.agent_types {
    push_or_like_group(&mut builder, &mut first, &mut push_clause, "at.name", values);
  }

  for values in &facets.divisions {
    push_or_like_group(&mut builder, &mut first, &mut push_clause, "d.name", values);
  }

  for values in &facets.systems {
    push_or_like_group(&mut builder, &mut first, &mut push_clause, "ss.name", values);
  }

  for values in &facets.regions {
    push_or_like_group(&mut builder, &mut first, &mut push_clause, "r.name", values);
  }

  for value in &facets.security_classes {
    push_clause(&mut builder, &mut first);
    push_security_predicate(&mut builder, *value);
  }

  for values in &facets.fields {
    push_clause(&mut builder, &mut first);
    builder.push(
      "EXISTS (SELECT 1 FROM npc_agent_skills ag JOIN item_types it ON it.id = ag.skill_type_id \
      WHERE ag.agent_id = a.id AND (",
    );
    let mut separated = builder.separated(" OR ");
    for value in values {
      separated.push("it.name LIKE ");
      separated.push_bind_unseparated(like_pattern(value));
      separated.push_unseparated(" ESCAPE '\\'");
    }
    builder.push("))");
  }

  if let Some(system_ids) = &facets.near_systems {
    push_clause(&mut builder, &mut first);
    if system_ids.is_empty() {
      builder.push("0 = 1");
    } else {
      builder.push("ss.id IN (");
      let mut separated = builder.separated(", ");
      for id in system_ids {
        separated.push_bind(*id);
      }
      separated.push_unseparated(")");
    }
  }

  if let Some((name, agent_id)) = after {
    push_clause(&mut builder, &mut first);
    builder.push("(a.name > ");
    builder.push_bind(name.clone());
    builder.push(" OR (a.name = ");
    builder.push_bind(name.clone());
    builder.push(" AND a.id > ");
    builder.push_bind(*agent_id);
    builder.push("))");
  }

  builder.push(" ORDER BY a.name, a.id LIMIT ");
  builder.push_bind(limit);

  let sql_rows = builder.build_query_as::<AgentSql>().fetch_all(&db.0).await?;
  Ok(
    sql_rows
      .into_iter()
      .map(|row| {
        let raw_standing = raw.lookup(
          FROM_TYPE_AGENT,
          row.id,
          FROM_TYPE_CORP,
          row.corporation_id,
          row.faction_id,
        );
        let effective = effective_standing(raw_standing, row.faction_id, skills);
        let best = best_effective(raw, row.corporation_id, row.faction_id, skills, effective);
        CatalogRow {
          accessible: accessibility(best, row.level),
          agent_type: row.agent_type,
          corporation_id: row.corporation_id,
          division: row.division,
          effective_standing: effective,
          faction_id: row.faction_id,
          id: row.id,
          kind: CatalogKind::Agent,
          level: row.level,
          name: row.name,
          raw_standing,
          region_name: row.region_name,
          security_status: row.security_status,
          system_name: row.system_name,
        }
      })
      .collect(),
  )
}

/// Returns the highest effective standing across the agent's own, corp, and faction standings.
///
/// Agent accessibility is gated on the best standing in the entity hierarchy, not the agent-specific
/// value alone, so a high corp or faction standing can unlock agents with no direct standing recorded.
fn best_effective(
  raw: &RawStandings,
  corporation_id: Option<i64>,
  faction_id: Option<i64>,
  skills: SocialSkills,
  agent_effective: f64,
) -> f64 {
  let mut best = agent_effective;
  if let Some(corp_id) = corporation_id
    && let Some(value) = raw.corp.get(&corp_id)
  {
    best = best.max(effective_standing(*value, faction_id, skills));
  }
  if let Some(faction) = faction_id
    && let Some(value) = raw.faction.get(&faction)
  {
    best = best.max(effective_standing(*value, faction_id, skills));
  }
  best
}

async fn corporation_rows(db: &Database, raw: &RawStandings, skills: SocialSkills) -> Result<Vec<CatalogRow>, Error> {
  let sql_rows = sqlx::query_as::<_, CorpSql>(
    "SELECT id, name, faction_id FROM corporations WHERE id BETWEEN ? AND ? ORDER BY name",
  )
  .bind(NPC_CORP_ID_MIN)
  .bind(NPC_CORP_ID_MAX)
  .fetch_all(&db.0)
  .await?;

  Ok(
    sql_rows
      .into_iter()
      .map(|row| {
        let raw_standing = raw.lookup(
          FROM_TYPE_CORP,
          row.id,
          FROM_TYPE_FACTION,
          row.faction_id,
          row.faction_id,
        );
        CatalogRow {
          accessible: None,
          agent_type: None,
          corporation_id: Some(row.id),
          division: None,
          effective_standing: effective_standing(raw_standing, row.faction_id, skills),
          faction_id: row.faction_id,
          id: row.id,
          kind: CatalogKind::Corporation,
          level: None,
          name: row.name,
          raw_standing,
          region_name: None,
          security_status: None,
          system_name: None,
        }
      })
      .collect(),
  )
}

async fn faction_rows(db: &Database, raw: &RawStandings, skills: SocialSkills) -> Result<Vec<CatalogRow>, Error> {
  let sql_rows = sqlx::query_as::<_, FactionSql>("SELECT corporation_id, id, name FROM factions ORDER BY name")
    .fetch_all(&db.0)
    .await?;

  Ok(
    sql_rows
      .into_iter()
      .map(|row| {
        let raw_standing = raw.faction.get(&row.id).copied().unwrap_or(0.0);
        CatalogRow {
          accessible: None,
          agent_type: None,
          corporation_id: row.corporation_id,
          division: None,
          effective_standing: effective_standing(raw_standing, Some(row.id), skills),
          faction_id: Some(row.id),
          id: row.id,
          kind: CatalogKind::Faction,
          level: None,
          name: row.name,
          raw_standing,
          region_name: None,
          security_status: None,
          system_name: None,
        }
      })
      .collect(),
  )
}

fn push_or_like_group<F>(
  builder: &mut QueryBuilder<Sqlite>,
  first: &mut bool,
  push_clause: &mut F,
  column: &str,
  values: &[String],
) where
  F: FnMut(&mut QueryBuilder<Sqlite>, &mut bool),
{
  push_clause(builder, first);
  builder.push("(");
  for (index, value) in values.iter().enumerate() {
    if index > 0 {
      builder.push(" OR ");
    }
    builder.push(column);
    builder.push(" LIKE ");
    builder.push_bind(like_pattern(value));
    builder.push(" ESCAPE '\\'");
  }
  builder.push(")");
}

fn push_security_predicate(builder: &mut QueryBuilder<Sqlite>, class: SecurityClass) {
  match class {
    SecurityClass::High => builder.push("ss.security_status >= 0.45"),
    SecurityClass::Low => builder.push("(ss.security_status > 0.0 AND ss.security_status < 0.45)"),
    SecurityClass::Null => builder.push("(ss.security_status IS NULL OR ss.security_status <= 0.0)"),
  };
}

async fn raw_standings(db: &Database, character_id: i64) -> Result<RawStandings, Error> {
  let rows = sqlx::query_as::<_, (i64, String, f64)>(
    "SELECT from_id, from_type, standing FROM character_standings WHERE character_id = ?",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;

  let mut standings = RawStandings::default();
  for (from_id, from_type, standing) in rows {
    match from_type.as_str() {
      FROM_TYPE_AGENT => {
        standings.agent.insert(from_id, standing);
      }
      FROM_TYPE_CORP => {
        standings.corp.insert(from_id, standing);
      }
      FROM_TYPE_FACTION => {
        standings.faction.insert(from_id, standing);
      }
      _ => {}
    }
  }
  Ok(standings)
}

async fn social_skills(db: &Database, character_id: i64) -> Result<SocialSkills, Error> {
  let rows = sqlx::query_as::<_, (i64, i64)>(
    "SELECT skill_id, trained_skill_level FROM character_skills \
    WHERE character_id = ? AND skill_id IN (?, ?, ?)",
  )
  .bind(character_id)
  .bind(CONNECTIONS_SKILL_ID)
  .bind(DIPLOMACY_SKILL_ID)
  .bind(CRIMINAL_CONNECTIONS_SKILL_ID)
  .fetch_all(&db.0)
  .await?;

  let mut skills = SocialSkills::default();
  for (skill_id, level) in rows {
    match skill_id {
      CONNECTIONS_SKILL_ID => skills.connections = level,
      CRIMINAL_CONNECTIONS_SKILL_ID => skills.criminal_connections = level,
      DIPLOMACY_SKILL_ID => skills.diplomacy = level,
      _ => {}
    }
  }
  Ok(skills)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SecurityClass {
  High,
  Low,
  Null,
}

#[derive(Clone, Debug, Default)]
struct Facets {
  accessible: Option<bool>,
  agent_names: Vec<String>,
  agent_types: Vec<Vec<String>>,
  corp_negatives: Vec<String>,
  corp_positives: Vec<String>,
  divisions: Vec<Vec<String>>,
  faction_negatives: Vec<String>,
  faction_positives: Vec<String>,
  fields: Vec<Vec<String>>,
  free_text: Vec<String>,
  has_positive_type: bool,
  levels: Vec<Vec<i64>>,
  names: Vec<String>,
  near_me: bool,
  near_systems: Option<Vec<i64>>,
  regions: Vec<Vec<String>>,
  security_classes: Vec<SecurityClass>,
  standing_thresholds: Vec<(StandingComparison, f64)>,
  systems: Vec<Vec<String>>,
}

impl Facets {
  fn from_query(query: &ParsedQuery) -> Self {
    let mut facets = Facets::default();
    for token in &query.tokens {
      match token {
        FilterToken::FreeText {
          negated: false,
          text,
        } => match text.to_lowercase().as_str() {
          "reachable" | "accessible" => facets.accessible = Some(true),
          "locked" => facets.accessible = Some(false),
          _ => facets.free_text.push(text.clone()),
        },
        FilterToken::FreeText {
          ..
        } => {}
        FilterToken::KeyValue {
          key,
          negated,
          values,
        } => facets.absorb(key, *negated, values),
      }
    }
    facets
  }

  fn absorb(&mut self, key: &str, negated: bool, values: &[String]) {
    match normalize_facet_key(key) {
      "faction" => self.absorb_faction(negated, values),
      "corp" => self.absorb_corp(negated, values),
      "agent" => self.absorb_agent(values),
      "name" => self.names.extend(values.iter().cloned()),
      "level" => self.absorb_level(values),
      "type" => self.agent_types.push(values.to_vec()),
      "division" => self.divisions.push(values.to_vec()),
      "field" => self.fields.push(values.to_vec()),
      "system" => self.systems.push(values.to_vec()),
      "region" => self.regions.push(values.to_vec()),
      "sec" => self.absorb_sec(values),
      "near" => self.absorb_near(values),
      "accessible" => self.absorb_accessible(negated, values),
      "standing" => self.absorb_standing(values),
      _ => {}
    }
  }

  fn absorb_faction(&mut self, negated: bool, values: &[String]) {
    if negated {
      self.faction_negatives.extend(values.iter().cloned());
    } else {
      self.faction_positives.extend(values.iter().cloned());
      self.has_positive_type = true;
    }
  }

  fn absorb_corp(&mut self, negated: bool, values: &[String]) {
    if negated {
      self.corp_negatives.extend(values.iter().cloned());
    } else {
      self.corp_positives.extend(values.iter().cloned());
      self.has_positive_type = true;
    }
  }

  fn absorb_agent(&mut self, values: &[String]) {
    self.agent_names.extend(values.iter().cloned());
    self.has_positive_type = true;
  }

  fn absorb_level(&mut self, values: &[String]) {
    let levels: Vec<i64> = values.iter().filter_map(|value| value.parse().ok()).collect();
    if !levels.is_empty() {
      self.levels.push(levels);
    }
  }

  fn absorb_sec(&mut self, values: &[String]) {
    for value in values {
      if let Some(class) = parse_security_class(value) {
        self.security_classes.push(class);
      }
    }
  }

  fn absorb_near(&mut self, values: &[String]) {
    self.near_me = self.near_me || values.iter().any(|value| value == "me");
  }

  fn absorb_accessible(&mut self, negated: bool, values: &[String]) {
    let wants = values
      .iter()
      .any(|value| matches!(value.as_str(), "true" | "yes" | "1"));
    let denies = values
      .iter()
      .any(|value| matches!(value.as_str(), "false" | "no" | "0"));
    self.accessible = Some(if denies { false } else { wants || !negated });
  }

  fn absorb_standing(&mut self, values: &[String]) {
    for value in values {
      if let Some(threshold) = parse_standing_threshold(value) {
        self.standing_thresholds.push(threshold);
      }
    }
  }

  fn keeps(&self, row: &CatalogRow, names: &NameIndex) -> bool {
    if !self.passes_type_filters(row, names) {
      return false;
    }
    if !self.passes_name_and_free_text(row) {
      return false;
    }
    if !self.passes_accessibility(row) {
      return false;
    }
    if !self.passes_standing(row) {
      return false;
    }
    true
  }

  fn passes_accessibility(&self, row: &CatalogRow) -> bool {
    let Some(required) = self.accessible else {
      return true;
    };
    match row.kind {
      CatalogKind::Agent => row.accessible == Some(required),
      _ => true,
    }
  }

  // `name:` and bare free text are AND-combined cross-cutting filters that match a row's own name
  // (and, for free text, its kind label) regardless of entity kind. The SQL agent query already
  // applies these to agents; re-applying here also bounds the unfiltered faction/corp rows so a
  // `name:` query does not leak the entire default catalog.
  fn passes_name_and_free_text(&self, row: &CatalogRow) -> bool {
    let name = row.name.to_lowercase();
    let kind = kind_label(row.kind);

    if !self.names.is_empty() && !self.names.iter().any(|value| name.contains(value)) {
      return false;
    }
    self
      .free_text
      .iter()
      .all(|word| name.contains(word) || kind.contains(word.as_str()))
  }

  fn passes_standing(&self, row: &CatalogRow) -> bool {
    self.standing_thresholds.iter().all(|(comparison, bound)| {
      let value = row.effective_standing;
      match comparison {
        StandingComparison::AtLeast => value >= *bound,
        StandingComparison::AtMost => value <= *bound,
        StandingComparison::GreaterThan => value > *bound,
        StandingComparison::LessThan => value < *bound,
      }
    })
  }

  // Type facets are relationship-aware: a positive `faction:` keeps the faction, its corps, and its
  // agents; a negative `-corp:` removes the corp row AND that corp's agents. Distinct positive type
  // facets intersect (e.g. `faction:Caldari corp:Navy` keeps only Navy corps/agents inside Caldari);
  // values within one facet are OR. This honors the spec acceptance criteria; the design-mock parser
  // treats positive type facets as a union, which is non-authoritative.
  fn passes_type_filters(&self, row: &CatalogRow, names: &NameIndex) -> bool {
    let name = row.name.to_lowercase();

    for value in &self.faction_negatives {
      if row_in_faction(row, value, names) {
        return false;
      }
    }
    for value in &self.corp_negatives {
      if row_in_corp(row, value, names) {
        return false;
      }
    }

    // A Faction row is "context" for a query: it shows only when a `faction:` facet selects it. Any
    // positive `corp:`/`agent:` facet that is NOT accompanied by a matching `faction:` excludes the
    // faction row, which keeps `corp:navy` from listing every faction.
    if row.kind == CatalogKind::Faction {
      if self.faction_positives.is_empty() {
        return !self.has_positive_type;
      }
      return self.faction_positives.iter().any(|value| name.contains(value));
    }

    if !self.faction_positives.is_empty()
      && !self
        .faction_positives
        .iter()
        .any(|value| row_in_faction(row, value, names))
    {
      return false;
    }

    if !self.corp_positives.is_empty() && !self.corp_positives.iter().any(|value| row_in_corp(row, value, names)) {
      return false;
    }

    if !self.agent_names.is_empty() {
      match row.kind {
        CatalogKind::Agent => {
          if !self.agent_names.iter().any(|value| name.contains(value)) {
            return false;
          }
        }
        _ => return false,
      }
    }

    true
  }

  fn surfaces_agents(&self) -> bool {
    self.has_positive_type
      || !self.names.is_empty()
      || !self.free_text.is_empty()
      || !self.levels.is_empty()
      || !self.agent_types.is_empty()
      || !self.divisions.is_empty()
      || !self.fields.is_empty()
      || !self.systems.is_empty()
      || !self.regions.is_empty()
      || !self.security_classes.is_empty()
      || self.near_me
      || self.accessible.is_some()
      || !self.standing_thresholds.is_empty()
  }
}

fn normalize_facet_key(key: &str) -> &'static str {
  match key {
    "fac" | "faction" | "factions" => "faction",
    "corp" | "corporation" | "corps" => "corp",
    "agent" | "agents" => "agent",
    "name" => "name",
    "level" => "level",
    "type" => "type",
    "division" => "division",
    "field" | "datacore" => "field",
    "system" => "system",
    "region" => "region",
    "sec" => "sec",
    "near" => "near",
    "accessible" => "accessible",
    "standing" => "standing",
    _ => "",
  }
}

fn parse_security_class(value: &str) -> Option<SecurityClass> {
  match value {
    "high" | "hi" | "highsec" => Some(SecurityClass::High),
    "low" | "lowsec" => Some(SecurityClass::Low),
    "null" | "nullsec" | "0.0" => Some(SecurityClass::Null),
    _ => None,
  }
}

fn parse_standing_threshold(value: &str) -> Option<(StandingComparison, f64)> {
  let (comparison, number) = if let Some(rest) = value.strip_prefix(">=") {
    (StandingComparison::AtLeast, rest)
  } else if let Some(rest) = value.strip_prefix("<=") {
    (StandingComparison::AtMost, rest)
  } else if let Some(rest) = value.strip_prefix('>') {
    (StandingComparison::GreaterThan, rest)
  } else if let Some(rest) = value.strip_prefix('<') {
    (StandingComparison::LessThan, rest)
  } else {
    (StandingComparison::AtLeast, value)
  };
  number.trim().parse::<f64>().ok().map(|number| (comparison, number))
}

fn kind_label(kind: CatalogKind) -> &'static str {
  match kind {
    CatalogKind::Agent => "agent",
    CatalogKind::Corporation => "corp",
    CatalogKind::Faction => "faction",
  }
}

async fn near_me_systems(db: &Database, character_id: i64) -> Result<Vec<i64>, Error> {
  // Prefer the character's live solar system; fall back to the active clone's home station system.
  let live = sqlx::query_scalar::<_, Option<i64>>("SELECT solar_system_id FROM character_state WHERE character_id = ?")
    .bind(character_id)
    .fetch_optional(&db.0)
    .await?
    .flatten();
  if let Some(system_id) = live {
    return Ok(vec![system_id]);
  }

  let cloned = sqlx::query_scalar::<_, Option<i64>>(
    "SELECT st.system_id FROM character_clones cc JOIN stations st ON st.id = cc.home_location_id \
    WHERE cc.character_id = ?",
  )
  .bind(character_id)
  .fetch_optional(&db.0)
  .await
  .unwrap_or(None)
  .flatten();
  Ok(cloned.map(|system_id| vec![system_id]).unwrap_or_default())
}

// A row belongs to a corp matching `value` when its own corporation's name (resolved via the corp
// index) contains the value. Faction rows have no corporation, so they never match.
fn row_in_corp(row: &CatalogRow, value: &str, names: &NameIndex) -> bool {
  row
    .corporation_id
    .and_then(|id| names.corps.get(&id))
    .is_some_and(|name| name.contains(value))
}

// A row belongs to a faction matching `value` when its own faction (for a faction row) or its
// corp's faction (for corps/agents) resolves to a name containing the value.
fn row_in_faction(row: &CatalogRow, value: &str, names: &NameIndex) -> bool {
  if row.kind == CatalogKind::Faction {
    return row.name.to_lowercase().contains(value);
  }
  row
    .faction_id
    .and_then(|id| names.factions.get(&id))
    .is_some_and(|name| name.contains(value))
}

#[derive(Clone, Debug, Default)]
struct NameIndex {
  corps: HashMap<i64, String>,
  factions: HashMap<i64, String>,
}

impl NameIndex {
  async fn load(db: &Database) -> Result<Self, Error> {
    let corps = sqlx::query_as::<_, (i64, String)>("SELECT id, name FROM corporations")
      .fetch_all(&db.0)
      .await?
      .into_iter()
      .map(|(id, name)| (id, name.to_lowercase()))
      .collect();
    let factions = sqlx::query_as::<_, (i64, String)>("SELECT id, name FROM factions")
      .fetch_all(&db.0)
      .await?
      .into_iter()
      .map(|(id, name)| (id, name.to_lowercase()))
      .collect();
    Ok(Self {
      corps,
      factions,
    })
  }

  fn corps_matching(&self, values: &[String]) -> Vec<i64> {
    self
      .corps
      .iter()
      .filter(|(_, name)| values.iter().any(|value| name.contains(value)))
      .map(|(id, _)| *id)
      .collect()
  }

  fn factions_matching(&self, values: &[String]) -> Vec<i64> {
    self
      .factions
      .iter()
      .filter(|(_, name)| values.iter().any(|value| name.contains(value)))
      .map(|(id, _)| *id)
      .collect()
  }
}

#[derive(Clone, Debug, Default)]
struct RawStandings {
  agent: HashMap<i64, f64>,
  corp: HashMap<i64, f64>,
  faction: HashMap<i64, f64>,
}

impl RawStandings {
  /// Returns the standing for an entity using EVE's three-tier fallback: own standing first,
  /// then the parent entity (corp for agents, faction for corps), then the faction, defaulting to 0.0.
  fn lookup(&self, kind: &str, id: i64, parent_kind: &str, parent_id: Option<i64>, faction_id: Option<i64>) -> f64 {
    let own = match kind {
      FROM_TYPE_AGENT => self.agent.get(&id),
      FROM_TYPE_CORP => self.corp.get(&id),
      FROM_TYPE_FACTION => self.faction.get(&id),
      _ => None,
    };
    if let Some(value) = own {
      return *value;
    }
    if let Some(parent_id) = parent_id {
      let parent = match parent_kind {
        FROM_TYPE_CORP => self.corp.get(&parent_id),
        FROM_TYPE_FACTION => self.faction.get(&parent_id),
        _ => None,
      };
      if let Some(value) = parent {
        return *value;
      }
    }
    if let Some(faction_id) = faction_id
      && let Some(value) = self.faction.get(&faction_id)
    {
      return *value;
    }
    0.0
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod effective_standing {
    use pretty_assertions::assert_eq;

    use super::*;

    const CALDARI: i64 = 500_001;
    const ANGEL_CARTEL: i64 = 500_011;

    #[test]
    fn it_returns_the_raw_value_with_no_social_skills() {
      assert_eq!(effective_standing(4.0, Some(CALDARI), SocialSkills::default()), 4.0);
    }

    #[test]
    fn it_raises_positive_empire_standings_with_connections() {
      let skills = SocialSkills {
        connections: 5,
        ..SocialSkills::default()
      };

      // 4 + (10 - 4) * 0.04 * 5 = 4 + 1.2 = 5.2
      assert!((effective_standing(4.0, Some(CALDARI), skills) - 5.2).abs() < 1e-9);
    }

    #[test]
    fn it_ignores_connections_for_a_pirate_faction() {
      let skills = SocialSkills {
        connections: 5,
        ..SocialSkills::default()
      };

      assert_eq!(effective_standing(4.0, Some(ANGEL_CARTEL), skills), 4.0);
    }

    #[test]
    fn it_raises_positive_pirate_standings_with_criminal_connections() {
      let skills = SocialSkills {
        criminal_connections: 5,
        ..SocialSkills::default()
      };

      assert!((effective_standing(4.0, Some(ANGEL_CARTEL), skills) - 5.2).abs() < 1e-9);
    }

    #[test]
    fn it_raises_negative_standings_toward_zero_with_diplomacy() {
      let skills = SocialSkills {
        diplomacy: 5,
        ..SocialSkills::default()
      };

      // -4 + (0 - -4) * 0.04 * 5 = -4 + 0.8 = -3.2
      assert!((effective_standing(-4.0, Some(CALDARI), skills) - -3.2).abs() < 1e-9);
    }

    #[test]
    fn it_uses_diplomacy_not_criminal_connections_for_negative_pirate_standings() {
      let skills = SocialSkills {
        criminal_connections: 5,
        diplomacy: 5,
        ..SocialSkills::default()
      };

      assert!((effective_standing(-4.0, Some(ANGEL_CARTEL), skills) - -3.2).abs() < 1e-9);
    }
  }

  mod required_standing {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_threshold_for_each_level() {
      assert_eq!(required_standing(1), -2.0);
      assert_eq!(required_standing(2), 1.0);
      assert_eq!(required_standing(3), 3.0);
      assert_eq!(required_standing(4), 5.0);
      assert_eq!(required_standing(5), 7.0);
    }

    #[test]
    fn it_falls_back_to_zero_for_an_unknown_level() {
      assert_eq!(required_standing(9), 0.0);
    }
  }

  mod accessibility {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_none_without_a_level() {
      assert_eq!(accessibility(10.0, None), None);
    }

    #[test]
    fn it_is_accessible_when_effective_meets_the_requirement() {
      assert_eq!(accessibility(5.0, Some(4)), Some(true));
    }

    #[test]
    fn it_is_locked_when_effective_falls_short() {
      assert_eq!(accessibility(4.99, Some(4)), Some(false));
    }
  }

  mod catalog {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    const ANGEL_CARTEL_FACTION: i64 = 500_011;
    const ANGEL_CORP: i64 = 1_000_002;
    const CALDARI_FACTION: i64 = 500_001;
    const CHARACTER: i64 = 90_000_001;
    const NAVY_CORP: i64 = 1_000_001;
    const SOE_CORP: i64 = 1_000_003;

    async fn exec(db: &store::Database, sql: &'static str) {
      sqlx::query(sql).execute(&db.0).await.unwrap();
    }

    // Seeds a small but coherent standings universe: two factions (one empire, one pirate), three
    // NPC corps, four agents with stations/systems/regions and skills, plus a character with social
    // skills and one explicit corp standing.
    async fn seed(db: &store::Database) {
      exec(
        db,
        "INSERT INTO factions (id, corporation_id, description, is_unique, name, size_factor, station_count, \
        station_system_count) VALUES \
        (500001, NULL, '', 1, 'Caldari State', 1.0, 0, 0), \
        (500011, NULL, '', 1, 'Angel Cartel', 1.0, 0, 0)",
      )
      .await;
      exec(
        db,
        "INSERT INTO corporations (id, ceo_id, creator_id, faction_id, member_count, name, tax_rate, ticker) VALUES \
        (1000001, 0, 0, 500001, 0, 'Caldari Navy', 0.0, 'CN'), \
        (1000002, 0, 0, 500011, 0, 'Angel Cartel Corp', 0.0, 'AC'), \
        (1000003, 0, 0, 500001, 0, 'Sisters of EVE', 0.0, 'SOE')",
      )
      .await;
      exec(
        db,
        "INSERT INTO races (id, alliance_id, description, name) VALUES (2, 0, '', 'Caldari')",
      )
      .await;
      exec(
        db,
        "INSERT INTO bloodlines (id, corporation_id, race_id, charisma, description, intelligence, memory, name, \
        perception, willpower) VALUES (1, 1000001, 2, 4, '', 7, 7, 'Civire', 6, 5)",
      )
      .await;
      exec(
        db,
        "INSERT INTO characters (id, bloodline_id, corporation_id, race_id, birthday, gender, name) \
        VALUES (90000001, 1, 1000001, 2, '2003-05-12', 'male', 'Test Pilot')",
      )
      .await;
      exec(
        db,
        "INSERT INTO agent_types (id, name) VALUES (1, 'BasicAgent'), (5, 'ResearchAgent')",
      )
      .await;
      exec(
        db,
        "INSERT INTO npc_corporation_divisions (id, name) VALUES (22, 'Security'), (24, 'Distribution')",
      )
      .await;
      exec(
        db,
        "INSERT INTO regions (id, description, name) VALUES (10000001, '', 'The Forge')",
      )
      .await;
      exec(
        db,
        "INSERT INTO constellations (id, name, region_id, position_x, position_y, position_z) \
        VALUES (20000001, 'Kimotoro', 10000001, 0, 0, 0)",
      )
      .await;
      exec(
        db,
        "INSERT INTO solar_systems (id, constellation_id, name, position_x, position_y, position_z, security_status) \
        VALUES (30000001, 20000001, 'Jita', 0.0, 0.0, 0.0, 0.9), \
        (30000002, 20000001, 'Rancer', 0.0, 0.0, 0.0, 0.4)",
      )
      .await;
      exec(
        db,
        "INSERT INTO item_categories (id, name, published) VALUES (1, 'Skill', 1)",
      )
      .await;
      exec(
        db,
        "INSERT INTO item_groups (id, category_id, name, published) VALUES (1, 1, 'Science', 1)",
      )
      .await;
      exec(
        db,
        "INSERT INTO item_types (id, group_id, description, name, published) VALUES \
        (11433, 1, '', 'Caldari Starship Engineering', 1)",
      )
      .await;
      exec(
        db,
        "INSERT INTO stations \
        (id, max_dockable_ship_volume, name, office_rental_cost, position_x, position_y, position_z, \
        reprocessing_efficiency, reprocessing_stations_take, services, system_id, type_id) VALUES \
        (60000001, 0, 'Jita Station', 0, 0, 0, 0, 0, 0, '[]', 30000001, 11433), \
        (60000002, 0, 'Rancer Station', 0, 0, 0, 0, 0, 0, '[]', 30000002, 11433)",
      )
      .await;
      exec(
        db,
        "INSERT INTO npc_agents (id, agent_type_id, corporation_id, division_id, level, location_id, name) VALUES \
        (3000001, 1, 1000001, 22, 4, 60000001, 'Navy Sec Agent'), \
        (3000002, 1, 1000002, 24, 1, 60000002, 'Angel Dist Agent'), \
        (3000003, 1, 1000003, 22, 2, 60000001, 'Sisters Agent'), \
        (3000004, 5, 1000001, 22, 1, 60000001, 'Navy Researcher')",
      )
      .await;
      exec(
        db,
        "INSERT INTO npc_agent_skills (agent_id, skill_type_id) VALUES (3000004, 11433)",
      )
      .await;
    }

    async fn run(db: &store::Database, query: &str) -> Vec<CatalogRow> {
      catalog(db, CHARACTER, &parse(query), false, Some(500)).await.unwrap()
    }

    fn names(rows: &[CatalogRow]) -> Vec<&str> {
      rows.iter().map(|row| row.name.as_str()).collect()
    }

    fn of_kind(rows: &[CatalogRow], kind: CatalogKind) -> Vec<&str> {
      rows
        .iter()
        .filter(|row| row.kind == kind)
        .map(|row| row.name.as_str())
        .collect()
    }

    #[tokio::test]
    async fn it_returns_all_factions_and_corps_with_no_agents_by_default() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "").await;

      assert_eq!(of_kind(&rows, CatalogKind::Faction).len(), 2);
      assert_eq!(of_kind(&rows, CatalogKind::Corporation).len(), 3);
      assert!(of_kind(&rows, CatalogKind::Agent).is_empty());
      assert!(rows.iter().all(|row| row.raw_standing == 0.0));
    }

    #[tokio::test]
    async fn it_surfaces_agents_on_an_empty_query_when_forced() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = catalog(&db, CHARACTER, &parse(""), true, Some(500)).await.unwrap();

      // Forced agents bypass the facet gate but must still survive `keeps`, so the full catalog appears.
      assert_eq!(of_kind(&rows, CatalogKind::Faction).len(), 2);
      assert_eq!(of_kind(&rows, CatalogKind::Corporation).len(), 3);
      assert_eq!(of_kind(&rows, CatalogKind::Agent).len(), 4);
    }

    #[tokio::test]
    async fn it_scopes_a_faction_to_its_faction_corps_and_agents() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "faction:Caldari").await;

      assert_eq!(of_kind(&rows, CatalogKind::Faction), vec!["Caldari State"]);
      assert_eq!(
        of_kind(&rows, CatalogKind::Corporation),
        vec!["Caldari Navy", "Sisters of EVE"]
      );
      let agents = of_kind(&rows, CatalogKind::Agent);
      assert!(agents.contains(&"Navy Sec Agent"));
      assert!(!agents.contains(&"Angel Dist Agent"));
    }

    #[tokio::test]
    async fn it_excludes_a_negated_corp_and_its_agents() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "faction:Caldari -corp:\"Sisters of EVE\"").await;

      let corps = of_kind(&rows, CatalogKind::Corporation);
      assert!(!corps.contains(&"Sisters of EVE"));
      assert!(corps.contains(&"Caldari Navy"));
      assert!(!names(&rows).contains(&"Sisters Agent"));
    }

    #[tokio::test]
    async fn it_intersects_a_positive_corp_within_a_faction() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "faction:Caldari corp:Navy").await;

      assert_eq!(of_kind(&rows, CatalogKind::Corporation), vec!["Caldari Navy"]);
      let agents = of_kind(&rows, CatalogKind::Agent);
      assert!(agents.contains(&"Navy Sec Agent"));
      assert!(!agents.contains(&"Sisters Agent"));
    }

    #[tokio::test]
    async fn it_filters_agents_by_level() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "level:4").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent), vec!["Navy Sec Agent"]);
    }

    #[tokio::test]
    async fn it_filters_agents_by_division() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "division:Distribution").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent), vec!["Angel Dist Agent"]);
    }

    #[tokio::test]
    async fn it_filters_agents_by_type() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "type:Research").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent), vec!["Navy Researcher"]);
    }

    #[tokio::test]
    async fn it_filters_agents_by_research_field() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "field:Caldari").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent), vec!["Navy Researcher"]);
    }

    #[tokio::test]
    async fn it_filters_agents_by_region() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "region:\"The Forge\"").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent).len(), 4);
    }

    #[tokio::test]
    async fn it_filters_agents_by_security_class() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "sec:low").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent), vec!["Angel Dist Agent"]);
    }

    #[tokio::test]
    async fn it_filters_agents_by_system() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "system:Rancer").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent), vec!["Angel Dist Agent"]);
    }

    #[tokio::test]
    async fn it_filters_by_effective_standing_threshold() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;
      exec(
        &db,
        "INSERT INTO character_standings (character_id, from_id, from_name, from_type, standing) VALUES \
        (90000001, 500001, 'Caldari State', 'faction', 6.0)",
      )
      .await;

      let rows = run(&db, "faction:Caldari standing:>=6").await;

      assert!(
        rows
          .iter()
          .filter(|r| r.kind == CatalogKind::Corporation)
          .all(|r| r.effective_standing >= 6.0)
      );
      assert!(
        rows
          .iter()
          .any(|r| r.kind == CatalogKind::Faction && r.name == "Caldari State")
      );
    }

    #[tokio::test]
    async fn it_cascades_corp_standing_to_an_agent_and_applies_social_skills() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;
      exec(
        &db,
        "INSERT INTO character_skills (character_id, skill_id, active_skill_level, skillpoints_in_skill, \
        trained_skill_level) VALUES (90000001, 3359, 5, 0, 5)",
      )
      .await;
      exec(
        &db,
        "INSERT INTO character_standings (character_id, from_id, from_name, from_type, standing) VALUES \
        (90000001, 1000001, 'Caldari Navy', 'npc_corp', 5.0)",
      )
      .await;

      let rows = run(&db, "corp:Navy level:4").await;

      let agent = rows.iter().find(|r| r.name == "Navy Sec Agent").unwrap();
      assert_eq!(agent.raw_standing, 5.0);
      assert!((agent.effective_standing - 6.0).abs() < 1e-9);
      assert_eq!(agent.accessible, Some(true));
    }

    #[tokio::test]
    async fn it_filters_accessible_agents_by_best_of_corp_and_faction() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;
      exec(
        &db,
        "INSERT INTO character_standings (character_id, from_id, from_name, from_type, standing) VALUES \
        (90000001, 1000001, 'Caldari Navy', 'npc_corp', 6.0)",
      )
      .await;

      let accessible = run(&db, "corp:Navy reachable").await;
      assert!(accessible.iter().any(|r| r.name == "Navy Sec Agent"));

      let locked = run(&db, "corp:Navy locked").await;
      assert!(!locked.iter().any(|r| r.name == "Navy Sec Agent"));
    }

    #[tokio::test]
    async fn it_matches_an_agent_name_facet() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "agent:Researcher").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent), vec!["Navy Researcher"]);
      assert!(of_kind(&rows, CatalogKind::Faction).is_empty());
    }

    mod agent_page {
      use pretty_assertions::assert_eq;

      use super::*;

      fn page_names(page: &AgentPage) -> Vec<&str> {
        page.rows.iter().map(|row| row.name.as_str()).collect()
      }

      #[tokio::test]
      async fn it_returns_no_rows_when_the_query_does_not_surface_agents() {
        let db = store::open_test().await.unwrap();
        seed(&db).await;

        let page = agent_page(&db, CHARACTER, &parse(""), false, None, 100).await.unwrap();

        assert!(page.rows.is_empty());
        assert_eq!(page.next_cursor, None);
      }

      #[tokio::test]
      async fn it_returns_agents_on_an_empty_query_when_forced() {
        let db = store::open_test().await.unwrap();
        seed(&db).await;

        let page = agent_page(&db, CHARACTER, &parse(""), true, None, 100).await.unwrap();

        assert_eq!(
          page_names(&page),
          vec!["Angel Dist Agent", "Navy Researcher", "Navy Sec Agent", "Sisters Agent"]
        );
      }

      #[tokio::test]
      async fn it_orders_agents_by_name_then_id() {
        let db = store::open_test().await.unwrap();
        seed(&db).await;

        let page = agent_page(&db, CHARACTER, &parse("region:\"The Forge\""), false, None, 100)
          .await
          .unwrap();

        assert_eq!(
          page_names(&page),
          vec!["Angel Dist Agent", "Navy Researcher", "Navy Sec Agent", "Sisters Agent"]
        );
      }

      #[tokio::test]
      async fn it_pages_through_agents_without_overlap_via_the_cursor() {
        let db = store::open_test().await.unwrap();
        seed(&db).await;

        let first = agent_page(&db, CHARACTER, &parse("region:\"The Forge\""), false, None, 2)
          .await
          .unwrap();
        assert_eq!(page_names(&first), vec!["Angel Dist Agent", "Navy Researcher"]);
        assert_eq!(first.next_cursor, Some(("Navy Researcher".to_owned(), 3_000_004)));

        let second = agent_page(
          &db,
          CHARACTER,
          &parse("region:\"The Forge\""),
          false,
          first.next_cursor.clone(),
          2,
        )
        .await
        .unwrap();
        assert_eq!(page_names(&second), vec!["Navy Sec Agent", "Sisters Agent"]);
        // A full page always carries a cursor; the next seek confirms exhaustion with an empty page.
        assert_eq!(second.next_cursor, Some(("Sisters Agent".to_owned(), 3_000_003)));

        let third = agent_page(
          &db,
          CHARACTER,
          &parse("region:\"The Forge\""),
          false,
          second.next_cursor.clone(),
          2,
        )
        .await
        .unwrap();

        assert!(third.rows.is_empty());
        assert_eq!(third.next_cursor, None);
      }

      #[tokio::test]
      async fn it_exhausts_the_cursor_on_a_short_final_page() {
        let db = store::open_test().await.unwrap();
        seed(&db).await;

        let only = agent_page(&db, CHARACTER, &parse("region:\"The Forge\""), false, None, 10)
          .await
          .unwrap();

        assert_eq!(only.rows.len(), 4);
        assert_eq!(only.next_cursor, None);
      }
    }
  }

  mod facets {
    use pretty_assertions::assert_eq;

    use super::*;

    fn vals(items: &[&str]) -> Vec<String> {
      items.iter().map(|item| (*item).to_string()).collect()
    }

    #[test]
    fn it_collects_positive_faction_facets() {
      let mut facets = Facets::default();
      facets.absorb("faction", false, &vals(&["Caldari"]));

      assert_eq!(facets.faction_positives, vec!["Caldari".to_string()]);
      assert!(facets.faction_negatives.is_empty());
      assert!(facets.has_positive_type);
    }

    #[test]
    fn it_collects_negative_faction_facets() {
      let mut facets = Facets::default();
      facets.absorb("faction", true, &vals(&["Gallente"]));

      assert_eq!(facets.faction_negatives, vec!["Gallente".to_string()]);
      assert!(facets.faction_positives.is_empty());
      assert!(!facets.has_positive_type);
    }

    #[test]
    fn it_collects_positive_corp_facets() {
      let mut facets = Facets::default();
      facets.absorb("corp", false, &vals(&["Navy"]));

      assert_eq!(facets.corp_positives, vec!["Navy".to_string()]);
      assert!(facets.corp_negatives.is_empty());
      assert!(facets.has_positive_type);
    }

    #[test]
    fn it_collects_negative_corp_facets() {
      let mut facets = Facets::default();
      facets.absorb("corp", true, &vals(&["Navy"]));

      assert_eq!(facets.corp_negatives, vec!["Navy".to_string()]);
      assert!(facets.corp_positives.is_empty());
      assert!(!facets.has_positive_type);
    }

    #[test]
    fn it_collects_agent_facets() {
      let mut facets = Facets::default();
      facets.absorb("agent", false, &vals(&["Researcher"]));

      assert_eq!(facets.agent_names, vec!["Researcher".to_string()]);
      assert!(facets.has_positive_type);
    }

    #[test]
    fn it_collects_name_facets_without_marking_positive_type() {
      let mut facets = Facets::default();
      facets.absorb("name", false, &vals(&["Kaalakiota"]));

      assert_eq!(facets.names, vec!["Kaalakiota".to_string()]);
      assert!(!facets.has_positive_type);
    }

    #[test]
    fn it_parses_numeric_levels_and_drops_invalid_ones() {
      let mut facets = Facets::default();
      facets.absorb("level", false, &vals(&["3", "x", "4"]));

      assert_eq!(facets.levels, vec![vec![3, 4]]);
    }

    #[test]
    fn it_ignores_a_level_facet_with_no_valid_values() {
      let mut facets = Facets::default();
      facets.absorb("level", false, &vals(&["abc"]));

      assert!(facets.levels.is_empty());
    }

    #[test]
    fn it_collects_type_division_field_system_and_region_facets() {
      let mut facets = Facets::default();
      facets.absorb("type", false, &vals(&["Security"]));
      facets.absorb("division", false, &vals(&["Marketing"]));
      facets.absorb("field", false, &vals(&["Hydromagnetic"]));
      facets.absorb("system", false, &vals(&["Jita"]));
      facets.absorb("region", false, &vals(&["The Forge"]));

      assert_eq!(facets.agent_types, vec![vec!["Security".to_string()]]);
      assert_eq!(facets.divisions, vec![vec!["Marketing".to_string()]]);
      assert_eq!(facets.fields, vec![vec!["Hydromagnetic".to_string()]]);
      assert_eq!(facets.systems, vec![vec!["Jita".to_string()]]);
      assert_eq!(facets.regions, vec![vec!["The Forge".to_string()]]);
    }

    #[test]
    fn it_parses_security_classes_and_drops_unknown_ones() {
      let mut facets = Facets::default();
      facets.absorb("sec", false, &vals(&["high", "bogus", "null"]));

      assert_eq!(facets.security_classes, vec![SecurityClass::High, SecurityClass::Null]);
    }

    #[test]
    fn it_sets_near_me_only_for_the_me_value() {
      let mut facets = Facets::default();
      facets.absorb("near", false, &vals(&["jita"]));
      assert!(!facets.near_me);

      facets.absorb("near", false, &vals(&["me"]));
      assert!(facets.near_me);
    }

    #[test]
    fn it_resolves_accessible_true_yes_one() {
      for value in ["true", "yes", "1"] {
        let mut facets = Facets::default();
        facets.absorb("accessible", false, &vals(&[value]));
        assert_eq!(facets.accessible, Some(true));
      }
    }

    #[test]
    fn it_resolves_accessible_false_no_zero() {
      for value in ["false", "no", "0"] {
        let mut facets = Facets::default();
        facets.absorb("accessible", false, &vals(&[value]));
        assert_eq!(facets.accessible, Some(false));
      }
    }

    #[test]
    fn it_defaults_a_bare_accessible_facet_to_true_when_not_negated() {
      let mut facets = Facets::default();
      facets.absorb("accessible", false, &vals(&["maybe"]));
      assert_eq!(facets.accessible, Some(true));
    }

    #[test]
    fn it_defaults_a_negated_accessible_facet_to_false() {
      let mut facets = Facets::default();
      facets.absorb("accessible", true, &vals(&["maybe"]));
      assert_eq!(facets.accessible, Some(false));
    }

    #[test]
    fn it_parses_standing_thresholds_and_drops_unparseable_ones() {
      let mut facets = Facets::default();
      facets.absorb("standing", false, &vals(&[">=5", "nonsense"]));

      assert_eq!(facets.standing_thresholds.len(), 1);
      assert_eq!(facets.standing_thresholds[0].0, StandingComparison::AtLeast);
      assert!((facets.standing_thresholds[0].1 - 5.0).abs() < 1e-9);
    }

    #[test]
    fn it_ignores_an_unknown_facet_key() {
      let mut facets = Facets::default();
      facets.absorb("bogus", false, &vals(&["whatever"]));

      assert!(facets.names.is_empty());
      assert!(facets.free_text.is_empty());
      assert!(!facets.has_positive_type);
    }

    #[test]
    fn it_normalizes_aliases_through_absorb() {
      let mut facets = Facets::default();
      facets.absorb("fac", false, &vals(&["Caldari"]));
      facets.absorb("corporation", true, &vals(&["Navy"]));
      facets.absorb("agents", false, &vals(&["Researcher"]));
      facets.absorb("datacore", false, &vals(&["Hydromagnetic"]));

      assert_eq!(facets.faction_positives, vec!["Caldari".to_string()]);
      assert_eq!(facets.corp_negatives, vec!["Navy".to_string()]);
      assert_eq!(facets.agent_names, vec!["Researcher".to_string()]);
      assert_eq!(facets.fields, vec![vec!["Hydromagnetic".to_string()]]);
    }

    #[test]
    fn it_builds_facets_from_a_parsed_query() {
      let query = parse("reachable name:Kaalakiota faction:Caldari -corp:Navy level:4");
      let facets = Facets::from_query(&query);

      assert_eq!(facets.accessible, Some(true));
      assert_eq!(facets.names, vec!["kaalakiota".to_string()]);
      assert_eq!(facets.faction_positives, vec!["caldari".to_string()]);
      assert_eq!(facets.corp_negatives, vec!["navy".to_string()]);
      assert_eq!(facets.levels, vec![vec![4]]);
    }

    #[test]
    fn it_maps_locked_free_text_to_inaccessible() {
      let query = parse("locked");
      let facets = Facets::from_query(&query);
      assert_eq!(facets.accessible, Some(false));
    }

    #[test]
    fn it_keeps_unrecognized_free_text() {
      let query = parse("zephyr");
      let facets = Facets::from_query(&query);
      assert_eq!(facets.free_text, vec!["zephyr".to_string()]);
    }
  }
}
