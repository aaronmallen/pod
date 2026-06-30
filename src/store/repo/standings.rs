use sqlx::{QueryBuilder, Sqlite};

use crate::store::{
  Database, Error,
  model::{
    AgentPage, AgentSql, CatalogKind, CatalogRow, CorpSql, FROM_TYPE_AGENT, FROM_TYPE_CORP, FROM_TYPE_FACTION, Facets,
    FactionClass, FactionSql, NameIndex, RawStandings, SecurityClass, SocialSkills,
  },
  repo::infra::like_pattern,
  search::{ParsedQuery, parse_with_keys},
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

// Canonical EVE NPC corporation id range; player corporations start at 98,000,000.
const NPC_CORP_ID_MAX: i64 = 1_999_999;

const NPC_CORP_ID_MIN: i64 = 1_000_000;

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

pub fn accessibility(effective: f64, level: Option<i64>) -> Option<bool> {
  level.map(|level| effective >= required_standing(level))
}

pub fn effective_standing(raw: f64, faction_id: Option<i64>, skills: SocialSkills) -> f64 {
  let class = FactionClass::classify(faction_id);
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
  catalog_from(db, &facets, &raw, skills, force_agents, limit).await
}

/// Builds the combined faction/corporation/agent catalog for a corporation's raw standings.
///
/// Corporations carry no social-skill modifiers (so `effective_standing == raw_standing`) and have no location, so
/// the `near_me` system resolution is skipped: the agent SQL never narrows by the corp's location.
pub async fn corporation_catalog(
  db: &Database,
  corporation_id: i64,
  query: &ParsedQuery,
  force_agents: bool,
  limit: Option<i64>,
) -> Result<Vec<CatalogRow>, Error> {
  let facets = Facets::from_query(query);
  let raw = corporation_raw_standings(db, corporation_id).await?;
  catalog_from(db, &facets, &raw, SocialSkills::default(), force_agents, limit).await
}

async fn catalog_from(
  db: &Database,
  facets: &Facets,
  raw: &RawStandings,
  skills: SocialSkills,
  force_agents: bool,
  limit: Option<i64>,
) -> Result<Vec<CatalogRow>, Error> {
  let names = NameIndex::load(db).await?;

  let mut rows = Vec::new();
  rows.extend(faction_rows(db, raw, skills).await?);
  rows.extend(corporation_rows(db, raw, skills).await?);
  if force_agents || facets.surfaces_agents() {
    let bound = limit.unwrap_or(DEFAULT_LIMIT);
    rows.extend(agent_rows(db, facets, &names, raw, skills, None, bound).await?);
  }

  rows.retain(|row| facets.keeps(row, &names));
  Ok(rows)
}

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
  agent_page_from(db, &facets, &raw, skills, force_agents, after, limit).await
}

pub async fn corporation_agent_page(
  db: &Database,
  corporation_id: i64,
  query: &ParsedQuery,
  force_agents: bool,
  after: Option<(String, i64)>,
  limit: i64,
) -> Result<AgentPage, Error> {
  let facets = Facets::from_query(query);
  if !force_agents && !facets.surfaces_agents() {
    return Ok(AgentPage {
      next_cursor: None,
      rows: Vec::new(),
    });
  }
  let raw = corporation_raw_standings(db, corporation_id).await?;
  agent_page_from(db, &facets, &raw, SocialSkills::default(), force_agents, after, limit).await
}

async fn agent_page_from(
  db: &Database,
  facets: &Facets,
  raw: &RawStandings,
  skills: SocialSkills,
  force_agents: bool,
  after: Option<(String, i64)>,
  limit: i64,
) -> Result<AgentPage, Error> {
  if !force_agents && !facets.surfaces_agents() {
    return Ok(AgentPage {
      next_cursor: None,
      rows: Vec::new(),
    });
  }
  let names = NameIndex::load(db).await?;

  let mut rows = agent_rows(db, facets, &names, raw, skills, after.as_ref(), limit).await?;
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

  push_id_filters(&mut builder, &mut first, facets, names);
  push_name_filters(&mut builder, &mut first, facets);
  push_level_filters(&mut builder, &mut first, facets);
  push_like_group_filters(&mut builder, &mut first, facets);
  push_security_filters(&mut builder, &mut first, facets);
  push_field_filters(&mut builder, &mut first, facets);
  push_near_filter(&mut builder, &mut first, facets);
  push_after_filter(&mut builder, &mut first, after);

  builder.push(" ORDER BY a.name, a.id LIMIT ");
  builder.push_bind(limit);

  let sql_rows = builder.build_query_as::<AgentSql>().fetch_all(&db.0).await?;
  Ok(
    sql_rows
      .into_iter()
      .map(|row| agent_catalog_row(raw, skills, row))
      .collect(),
  )
}

fn agent_catalog_row(raw: &RawStandings, skills: SocialSkills, row: AgentSql) -> CatalogRow {
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
}

fn push_clause(builder: &mut QueryBuilder<Sqlite>, first: &mut bool) {
  builder.push(if *first { " WHERE " } else { " AND " });
  *first = false;
}

fn push_in_clause(builder: &mut QueryBuilder<Sqlite>, first: &mut bool, column: &str, ids: &[i64], empty_to_min: bool) {
  push_clause(builder, first);
  builder.push(column);
  builder.push(" IN (");
  let mut separated = builder.separated(", ");
  if empty_to_min && ids.is_empty() {
    separated.push_bind(i64::MIN);
  }
  for id in ids {
    separated.push_bind(*id);
  }
  separated.push_unseparated(")");
}

fn push_name_like(builder: &mut QueryBuilder<Sqlite>, first: &mut bool, value: &str) {
  push_clause(builder, first);
  builder.push("a.name LIKE ");
  builder.push_bind(like_pattern(value));
  builder.push(" ESCAPE '\\'");
}

fn push_id_filters(builder: &mut QueryBuilder<Sqlite>, first: &mut bool, facets: &Facets, names: &NameIndex) {
  if !facets.faction_positives.is_empty() {
    let faction_ids = names.factions_matching(&facets.faction_positives);
    push_in_clause(builder, first, "c.faction_id", &faction_ids, true);
  }
  if !facets.corp_positives.is_empty() {
    let corp_ids = names.corps_matching(&facets.corp_positives);
    push_in_clause(builder, first, "a.corporation_id", &corp_ids, true);
  }
}

fn push_name_filters(builder: &mut QueryBuilder<Sqlite>, first: &mut bool, facets: &Facets) {
  for value in facets.agent_names.iter().chain(&facets.names).chain(&facets.free_text) {
    push_name_like(builder, first, value);
  }
}

fn push_level_filters(builder: &mut QueryBuilder<Sqlite>, first: &mut bool, facets: &Facets) {
  for levels in &facets.levels {
    push_in_clause(builder, first, "a.level", levels, false);
  }
}

fn push_like_group_filters(builder: &mut QueryBuilder<Sqlite>, first: &mut bool, facets: &Facets) {
  let groups: [(&str, &Vec<Vec<String>>); 4] = [
    ("at.name", &facets.agent_types),
    ("d.name", &facets.divisions),
    ("ss.name", &facets.systems),
    ("r.name", &facets.regions),
  ];
  for (column, values_list) in groups {
    for values in values_list {
      push_or_like_group(builder, first, column, values);
    }
  }
}

fn push_security_filters(builder: &mut QueryBuilder<Sqlite>, first: &mut bool, facets: &Facets) {
  for value in &facets.security_classes {
    push_clause(builder, first);
    push_security_predicate(builder, *value);
  }
}

fn push_field_filters(builder: &mut QueryBuilder<Sqlite>, first: &mut bool, facets: &Facets) {
  for values in &facets.fields {
    push_clause(builder, first);
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
}

fn push_near_filter(builder: &mut QueryBuilder<Sqlite>, first: &mut bool, facets: &Facets) {
  let Some(system_ids) = &facets.near_systems else {
    return;
  };
  push_clause(builder, first);
  if system_ids.is_empty() {
    builder.push("0 = 1");
    return;
  }
  push_separated_ids(builder, "ss.id IN (", system_ids);
}

fn push_separated_ids(builder: &mut QueryBuilder<Sqlite>, prefix: &str, ids: &[i64]) {
  builder.push(prefix);
  let mut separated = builder.separated(", ");
  for id in ids {
    separated.push_bind(*id);
  }
  separated.push_unseparated(")");
}

fn push_after_filter(builder: &mut QueryBuilder<Sqlite>, first: &mut bool, after: Option<&(String, i64)>) {
  let Some((name, agent_id)) = after else {
    return;
  };
  push_clause(builder, first);
  builder.push("(a.name > ");
  builder.push_bind(name.clone());
  builder.push(" OR (a.name = ");
  builder.push_bind(name.clone());
  builder.push(" AND a.id > ");
  builder.push_bind(*agent_id);
  builder.push("))");
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

fn push_or_like_group(builder: &mut QueryBuilder<Sqlite>, first: &mut bool, column: &str, values: &[String]) {
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

async fn corporation_raw_standings(db: &Database, corporation_id: i64) -> Result<RawStandings, Error> {
  let rows = sqlx::query_as::<_, (i64, String, f64)>(
    "SELECT from_id, from_type, standing FROM corporation_standings WHERE corporation_id = ?",
  )
  .bind(corporation_id)
  .fetch_all(&db.0)
  .await?;

  Ok(collect_raw_standings(rows))
}

async fn raw_standings(db: &Database, character_id: i64) -> Result<RawStandings, Error> {
  let rows = sqlx::query_as::<_, (i64, String, f64)>(
    "SELECT from_id, from_type, standing FROM character_standings WHERE character_id = ?",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;

  Ok(collect_raw_standings(rows))
}

fn collect_raw_standings(rows: Vec<(i64, String, f64)>) -> RawStandings {
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
  standings
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

async fn near_me_systems(db: &Database, character_id: i64) -> Result<Vec<i64>, Error> {
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

#[cfg(test)]
mod tests {
  use super::*;

  mod accessibility {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_accessible_when_effective_meets_the_requirement() {
      assert_eq!(accessibility(5.0, Some(4)), Some(true));
    }

    #[test]
    fn it_is_locked_when_effective_falls_short() {
      assert_eq!(accessibility(4.99, Some(4)), Some(false));
    }

    #[test]
    fn it_is_none_without_a_level() {
      assert_eq!(accessibility(10.0, None), None);
    }
  }

  mod catalog {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    #[expect(dead_code)]
    const ANGEL_CARTEL_FACTION: i64 = 500_011;

    #[expect(dead_code)]
    const ANGEL_CORP: i64 = 1_000_002;

    #[expect(dead_code)]
    const CALDARI_FACTION: i64 = 500_001;

    const CHARACTER: i64 = 90_000_001;

    #[expect(dead_code)]
    const NAVY_CORP: i64 = 1_000_001;

    #[expect(dead_code)]
    const SOE_CORP: i64 = 1_000_003;

    async fn exec(db: &store::Database, sql: &'static str) {
      sqlx::query(sql).execute(db.writer()).await.unwrap();
    }

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

    mod agent_page {
      use pretty_assertions::assert_eq;

      use super::*;

      fn page_names(page: &AgentPage) -> Vec<&str> {
        page.rows.iter().map(|row| row.name.as_str()).collect()
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
      async fn it_returns_no_rows_when_the_query_does_not_surface_agents() {
        let db = store::open_test().await.unwrap();
        seed(&db).await;

        let page = agent_page(&db, CHARACTER, &parse(""), false, None, 100).await.unwrap();

        assert!(page.rows.is_empty());
        assert_eq!(page.next_cursor, None);
      }
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
    async fn it_filters_agents_by_division() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "division:Distribution").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent), vec!["Angel Dist Agent"]);
    }

    #[tokio::test]
    async fn it_filters_agents_by_level() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "level:4").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent), vec!["Navy Sec Agent"]);
    }

    #[tokio::test]
    async fn it_filters_agents_by_region() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "region:\"The Forge\"").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent).len(), 4);
    }

    #[tokio::test]
    async fn it_filters_agents_by_research_field() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "field:Caldari").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent), vec!["Navy Researcher"]);
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
    async fn it_filters_agents_by_type() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "type:Research").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent), vec!["Navy Researcher"]);
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
    async fn it_matches_an_agent_name_facet() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = run(&db, "agent:Researcher").await;

      assert_eq!(of_kind(&rows, CatalogKind::Agent), vec!["Navy Researcher"]);
      assert!(of_kind(&rows, CatalogKind::Faction).is_empty());
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
    async fn it_surfaces_agents_on_an_empty_query_when_forced() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = catalog(&db, CHARACTER, &parse(""), true, Some(500)).await.unwrap();

      assert_eq!(of_kind(&rows, CatalogKind::Faction).len(), 2);
      assert_eq!(of_kind(&rows, CatalogKind::Corporation).len(), 3);
      assert_eq!(of_kind(&rows, CatalogKind::Agent).len(), 4);
    }
  }

  mod corporation_catalog {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    const CALDARI_FACTION: i64 = 500_001;

    const CORPORATION: i64 = 98_000_001;

    async fn exec(db: &store::Database, sql: &'static str) {
      sqlx::query(sql).execute(db.writer()).await.unwrap();
    }

    async fn seed(db: &store::Database) {
      exec(
        db,
        "INSERT INTO factions (id, corporation_id, description, is_unique, name, size_factor, station_count, \
        station_system_count) VALUES (500001, NULL, '', 1, 'Caldari State', 1.0, 0, 0)",
      )
      .await;
      exec(
        db,
        "INSERT INTO corporations (id, ceo_id, creator_id, faction_id, member_count, name, tax_rate, ticker) VALUES \
        (1000001, 0, 0, 500001, 0, 'Caldari Navy', 0.0, 'CN'), \
        (98000001, 0, 0, NULL, 0, 'Cobalt Syndicate', 0.0, 'COBSY')",
      )
      .await;
      exec(db, "INSERT INTO agent_types (id, name) VALUES (1, 'BasicAgent')").await;
      exec(
        db,
        "INSERT INTO npc_corporation_divisions (id, name) VALUES (22, 'Security')",
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
        VALUES (30000001, 20000001, 'Jita', 0.0, 0.0, 0.0, 0.9)",
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
        (60000001, 0, 'Jita Station', 0, 0, 0, 0, 0, 0, '[]', 30000001, 11433)",
      )
      .await;
      exec(
        db,
        "INSERT INTO npc_agents (id, agent_type_id, corporation_id, division_id, level, location_id, name) VALUES \
        (3000001, 1, 1000001, 22, 4, 60000001, 'Navy Sec Agent')",
      )
      .await;
    }

    fn of_kind(rows: &[CatalogRow], kind: CatalogKind) -> Vec<&str> {
      rows
        .iter()
        .filter(|row| row.kind == kind)
        .map(|row| row.name.as_str())
        .collect()
    }

    #[tokio::test]
    async fn it_cascades_a_corp_standing_to_an_agent_without_social_skills() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;
      exec(
        &db,
        "INSERT INTO corporation_standings (corporation_id, from_id, from_type, standing, from_name) VALUES \
        (98000001, 1000001, 'npc_corp', 5.0, 'Caldari Navy')",
      )
      .await;

      let rows = corporation_catalog(&db, CORPORATION, &parse("corp:Navy level:4"), false, Some(500))
        .await
        .unwrap();

      let agent = rows.iter().find(|row| row.name == "Navy Sec Agent").unwrap();
      assert_eq!(agent.raw_standing, 5.0);
      assert_eq!(agent.effective_standing, 5.0);
      assert_eq!(agent.accessible, Some(true));
    }

    #[tokio::test]
    async fn it_pages_corporation_agents() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let empty = corporation_agent_page(&db, CORPORATION, &parse(""), false, None, 100)
        .await
        .unwrap();
      assert!(empty.rows.is_empty());

      let forced = corporation_agent_page(&db, CORPORATION, &parse(""), true, None, 100)
        .await
        .unwrap();

      assert_eq!(
        forced.rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
        vec!["Navy Sec Agent"]
      );
    }

    #[tokio::test]
    async fn it_returns_the_full_catalog_with_no_standings_by_default() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;

      let rows = corporation_catalog(&db, CORPORATION, &parse(""), false, Some(500))
        .await
        .unwrap();

      assert_eq!(of_kind(&rows, CatalogKind::Faction), vec!["Caldari State"]);
      assert_eq!(of_kind(&rows, CatalogKind::Corporation), vec!["Caldari Navy"]);
      assert!(of_kind(&rows, CatalogKind::Agent).is_empty());
      assert!(rows.iter().all(|row| row.raw_standing == 0.0));
    }

    #[tokio::test]
    async fn it_uses_the_corporation_raw_standing_and_treats_effective_as_raw() {
      let db = store::open_test().await.unwrap();
      seed(&db).await;
      exec(
        &db,
        "INSERT INTO corporation_standings (corporation_id, from_id, from_type, standing, from_name) VALUES \
        (98000001, 500001, 'faction', 6.0, 'Caldari State')",
      )
      .await;

      let rows = corporation_catalog(&db, CORPORATION, &parse("faction:Caldari"), false, Some(500))
        .await
        .unwrap();

      let faction = rows.iter().find(|row| row.id == CALDARI_FACTION).unwrap();
      assert_eq!(faction.raw_standing, 6.0);
      assert_eq!(faction.effective_standing, 6.0);
    }
  }

  mod effective_standing {
    use pretty_assertions::assert_eq;

    use super::*;

    const CALDARI: i64 = 500_001;

    const ANGEL_CARTEL: i64 = 500_011;

    #[test]
    fn it_ignores_connections_for_a_pirate_faction() {
      let skills = SocialSkills {
        connections: 5,
        ..SocialSkills::default()
      };

      assert_eq!(effective_standing(4.0, Some(ANGEL_CARTEL), skills), 4.0);
    }

    #[test]
    fn it_raises_negative_standings_toward_zero_with_diplomacy() {
      let skills = SocialSkills {
        diplomacy: 5,
        ..SocialSkills::default()
      };

      assert!((effective_standing(-4.0, Some(CALDARI), skills) - -3.2).abs() < 1e-9);
    }

    #[test]
    fn it_raises_positive_empire_standings_with_connections() {
      let skills = SocialSkills {
        connections: 5,
        ..SocialSkills::default()
      };

      assert!((effective_standing(4.0, Some(CALDARI), skills) - 5.2).abs() < 1e-9);
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
    fn it_returns_the_raw_value_with_no_social_skills() {
      assert_eq!(effective_standing(4.0, Some(CALDARI), SocialSkills::default()), 4.0);
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
    fn it_falls_back_to_zero_for_an_unknown_level() {
      assert_eq!(required_standing(9), 0.0);
    }

    #[test]
    fn it_returns_the_threshold_for_each_level() {
      assert_eq!(required_standing(1), -2.0);
      assert_eq!(required_standing(2), 1.0);
      assert_eq!(required_standing(3), 3.0);
      assert_eq!(required_standing(4), 5.0);
      assert_eq!(required_standing(5), 7.0);
    }
  }
}
