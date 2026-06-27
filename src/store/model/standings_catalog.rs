use std::collections::HashMap;

use crate::store::{
  Database, Error,
  search::{FilterToken, ParsedQuery},
};

pub(crate) const FROM_TYPE_AGENT: &str = "agent";

pub(crate) const FROM_TYPE_CORP: &str = "npc_corp";

pub(crate) const FROM_TYPE_FACTION: &str = "faction";

const PIRATE_FACTION_IDS: &[i64] = &[
  500_010, // Guristas Pirates
  500_011, // Angel Cartel
  500_012, // Blood Raiders
  500_019, // Sansha's Nation
  500_020, // Serpentis
];

#[derive(Clone, Debug, PartialEq)]
pub struct AgentPage {
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

#[derive(Clone, Debug, sqlx::FromRow)]
pub(crate) struct AgentSql {
  pub agent_type: Option<String>,
  pub corporation_id: Option<i64>,
  pub division: Option<String>,
  pub faction_id: Option<i64>,
  pub id: i64,
  pub level: Option<i64>,
  pub name: String,
  pub region_name: Option<String>,
  pub security_status: Option<f64>,
  pub system_name: Option<String>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub(crate) struct CorpSql {
  pub faction_id: Option<i64>,
  pub id: i64,
  pub name: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Facets {
  pub accessible: Option<bool>,
  pub agent_names: Vec<String>,
  pub agent_types: Vec<Vec<String>>,
  pub corp_negatives: Vec<String>,
  pub corp_positives: Vec<String>,
  pub divisions: Vec<Vec<String>>,
  pub faction_negatives: Vec<String>,
  pub faction_positives: Vec<String>,
  pub fields: Vec<Vec<String>>,
  pub free_text: Vec<String>,
  pub has_positive_type: bool,
  pub levels: Vec<Vec<i64>>,
  pub names: Vec<String>,
  pub near_me: bool,
  pub near_systems: Option<Vec<i64>>,
  pub regions: Vec<Vec<String>>,
  pub security_classes: Vec<SecurityClass>,
  pub standing_thresholds: Vec<(StandingComparison, f64)>,
  pub systems: Vec<Vec<String>>,
}

impl Facets {
  pub(crate) fn from_query(query: &ParsedQuery) -> Self {
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

  pub(crate) fn absorb(&mut self, key: &str, negated: bool, values: &[String]) {
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

  pub(crate) fn keeps(&self, row: &CatalogRow, names: &NameIndex) -> bool {
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

  pub(crate) fn surfaces_agents(&self) -> bool {
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

  fn absorb_accessible(&mut self, negated: bool, values: &[String]) {
    let wants = values
      .iter()
      .any(|value| matches!(value.as_str(), "true" | "yes" | "1"));
    let denies = values
      .iter()
      .any(|value| matches!(value.as_str(), "false" | "no" | "0"));
    self.accessible = Some(if denies { false } else { wants || !negated });
  }

  fn absorb_agent(&mut self, values: &[String]) {
    self.agent_names.extend(values.iter().cloned());
    self.has_positive_type = true;
  }

  fn absorb_corp(&mut self, negated: bool, values: &[String]) {
    if negated {
      self.corp_negatives.extend(values.iter().cloned());
    } else {
      self.corp_positives.extend(values.iter().cloned());
      self.has_positive_type = true;
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

  fn absorb_level(&mut self, values: &[String]) {
    let levels: Vec<i64> = values.iter().filter_map(|value| value.parse().ok()).collect();
    if !levels.is_empty() {
      self.levels.push(levels);
    }
  }

  fn absorb_near(&mut self, values: &[String]) {
    self.near_me = self.near_me || values.iter().any(|value| value == "me");
  }

  fn absorb_sec(&mut self, values: &[String]) {
    for value in values {
      if let Some(class) = parse_security_class(value) {
        self.security_classes.push(class);
      }
    }
  }

  fn absorb_standing(&mut self, values: &[String]) {
    for value in values {
      if let Some(threshold) = parse_standing_threshold(value) {
        self.standing_thresholds.push(threshold);
      }
    }
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FactionClass {
  Empire,
  Other,
  Pirate,
}

impl FactionClass {
  pub(crate) fn classify(faction_id: Option<i64>) -> Self {
    match faction_id {
      None => FactionClass::Other,
      Some(id) if PIRATE_FACTION_IDS.contains(&id) => FactionClass::Pirate,
      Some(_) => FactionClass::Empire,
    }
  }
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub(crate) struct FactionSql {
  pub corporation_id: Option<i64>,
  pub id: i64,
  pub name: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct NameIndex {
  pub corps: HashMap<i64, String>,
  pub factions: HashMap<i64, String>,
}

impl NameIndex {
  pub(crate) async fn load(db: &Database) -> Result<Self, Error> {
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

  pub(crate) fn corps_matching(&self, values: &[String]) -> Vec<i64> {
    self
      .corps
      .iter()
      .filter(|(_, name)| values.iter().any(|value| name.contains(value)))
      .map(|(id, _)| *id)
      .collect()
  }

  pub(crate) fn factions_matching(&self, values: &[String]) -> Vec<i64> {
    self
      .factions
      .iter()
      .filter(|(_, name)| values.iter().any(|value| name.contains(value)))
      .map(|(id, _)| *id)
      .collect()
  }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RawStandings {
  pub agent: HashMap<i64, f64>,
  pub corp: HashMap<i64, f64>,
  pub faction: HashMap<i64, f64>,
}

impl RawStandings {
  /// Returns the standing for an entity using EVE's three-tier fallback: own standing first,
  /// then the parent entity (corp for agents, faction for corps), then the faction, defaulting to 0.0.
  pub(crate) fn lookup(
    &self,
    kind: &str,
    id: i64,
    parent_kind: &str,
    parent_id: Option<i64>,
    faction_id: Option<i64>,
  ) -> f64 {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SecurityClass {
  High,
  Low,
  Null,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StandingComparison {
  AtLeast,
  AtMost,
  GreaterThan,
  LessThan,
}

fn kind_label(kind: CatalogKind) -> &'static str {
  match kind {
    CatalogKind::Agent => "agent",
    CatalogKind::Corporation => "corp",
    CatalogKind::Faction => "faction",
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

#[cfg(test)]
mod tests {
  use super::*;

  mod effective_standing_inputs {
    use pretty_assertions::assert_eq;

    use super::*;

    const ANGEL_CARTEL: i64 = 500_011;

    const CALDARI: i64 = 500_001;

    #[test]
    fn it_classifies_a_missing_faction_as_other() {
      assert_eq!(FactionClass::classify(None), FactionClass::Other);
    }

    #[test]
    fn it_classifies_a_pirate_faction() {
      assert_eq!(FactionClass::classify(Some(ANGEL_CARTEL)), FactionClass::Pirate);
    }

    #[test]
    fn it_classifies_an_empire_faction() {
      assert_eq!(FactionClass::classify(Some(CALDARI)), FactionClass::Empire);
    }
  }

  mod facets {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::search::parse_with_keys;

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

    fn parse(input: &str) -> ParsedQuery {
      parse_with_keys(input, RECOGNIZED_KEYS)
    }

    fn vals(items: &[&str]) -> Vec<String> {
      items.iter().map(|item| (*item).to_string()).collect()
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
    fn it_collects_negative_corp_facets() {
      let mut facets = Facets::default();
      facets.absorb("corp", true, &vals(&["Navy"]));

      assert_eq!(facets.corp_negatives, vec!["Navy".to_string()]);
      assert!(facets.corp_positives.is_empty());
      assert!(!facets.has_positive_type);
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
    fn it_collects_positive_faction_facets() {
      let mut facets = Facets::default();
      facets.absorb("faction", false, &vals(&["Caldari"]));

      assert_eq!(facets.faction_positives, vec!["Caldari".to_string()]);
      assert!(facets.faction_negatives.is_empty());
      assert!(facets.has_positive_type);
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
    fn it_ignores_a_level_facet_with_no_valid_values() {
      let mut facets = Facets::default();
      facets.absorb("level", false, &vals(&["abc"]));

      assert!(facets.levels.is_empty());
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
    fn it_keeps_unrecognized_free_text() {
      let query = parse("zephyr");
      let facets = Facets::from_query(&query);
      assert_eq!(facets.free_text, vec!["zephyr".to_string()]);
    }

    #[test]
    fn it_maps_locked_free_text_to_inaccessible() {
      let query = parse("locked");
      let facets = Facets::from_query(&query);
      assert_eq!(facets.accessible, Some(false));
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
    fn it_parses_numeric_levels_and_drops_invalid_ones() {
      let mut facets = Facets::default();
      facets.absorb("level", false, &vals(&["3", "x", "4"]));

      assert_eq!(facets.levels, vec![vec![3, 4]]);
    }

    #[test]
    fn it_parses_security_classes_and_drops_unknown_ones() {
      let mut facets = Facets::default();
      facets.absorb("sec", false, &vals(&["high", "bogus", "null"]));

      assert_eq!(facets.security_classes, vec![SecurityClass::High, SecurityClass::Null]);
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
    fn it_resolves_accessible_false_no_zero() {
      for value in ["false", "no", "0"] {
        let mut facets = Facets::default();
        facets.absorb("accessible", false, &vals(&[value]));
        assert_eq!(facets.accessible, Some(false));
      }
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
    fn it_sets_near_me_only_for_the_me_value() {
      let mut facets = Facets::default();
      facets.absorb("near", false, &vals(&["jita"]));
      assert!(!facets.near_me);

      facets.absorb("near", false, &vals(&["me"]));
      assert!(facets.near_me);
    }
  }
}
