use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use super::{budget, budget_engine as engine};
use crate::{
  services::pod_pack,
  store::{
    Database,
    model::{MatchMode, NewCategory, NewGroup, NewRule, Rule, RuleCondition, RuleField, RuleOp},
  },
};

pub const PACK_EXTENSION: &str = "pbr";
pub const PACK_VERSION: u32 = 1;

const DEFAULT_TONE: &str = "muted";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CategoryTarget {
  Create(usize),
  Existing(i64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupTarget {
  Create(usize),
  Existing(i64),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportPlan {
  pub created_categories: Vec<PlannedCategory>,
  pub created_groups: Vec<PlannedGroup>,
  pub items: Vec<PlanItem>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PackEnvelope {
  #[serde(default)]
  pub author: String,
  #[serde(default)]
  pub exported: i64,
  #[serde(default)]
  pub format: String,
  #[serde(default)]
  pub name: String,
  #[serde(default)]
  pub note: String,
  #[serde(default)]
  pub rules: Vec<PortableRule>,
  #[serde(default)]
  pub version: u32,
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum ParseError {
  #[error("pack contains no rules")]
  Empty,
  #[error("pack rules have no usable conditions")]
  NoUsableConditions,
  #[error("not a pod budget rule pack")]
  NotAPack,
  #[error("unsupported pack version")]
  UnsupportedVersion,
  #[error("wrong pack format")]
  WrongFormat,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanItem {
  pub conditions: Vec<RuleCondition>,
  pub is_duplicate: bool,
  pub match_mode: MatchMode,
  pub name: String,
  pub target: CategoryTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlannedCategory {
  pub group: GroupTarget,
  pub name: String,
  pub tone: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlannedGroup {
  pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PortableCategory {
  #[serde(default)]
  pub group: Option<i64>,
  #[serde(default, rename = "groupLabel")]
  pub group_label: String,
  #[serde(default)]
  pub id: Option<i64>,
  #[serde(default)]
  pub name: String,
  #[serde(default)]
  pub tone: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PortableCondition {
  #[serde(default)]
  pub field: String,
  #[serde(default)]
  pub op: String,
  #[serde(default)]
  pub value: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub value2: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PortableRule {
  pub cat: PortableCategory,
  #[serde(default)]
  pub conditions: Vec<PortableCondition>,
  #[serde(default, rename = "match")]
  pub match_mode: String,
  #[serde(default)]
  pub name: String,
}

struct Resolver<'a> {
  groups: &'a [budget::Group],
  imported_group: Option<usize>,
  memo: HashMap<String, CategoryTarget>,
  plan: ImportPlan,
}

impl PortableCondition {
  pub fn to_condition(&self) -> RuleCondition {
    RuleCondition {
      field: RuleField::from_key(&self.field),
      op: RuleOp::from_key(&self.op),
      value: self.value.clone(),
      value2: self.value2.clone(),
    }
  }
}

impl PortableRule {
  pub fn conditions(&self) -> Vec<RuleCondition> {
    self.conditions.iter().map(PortableCondition::to_condition).collect()
  }

  pub fn match_mode(&self) -> MatchMode {
    MatchMode::from_key(&self.match_mode)
  }
}

impl<'a> Resolver<'a> {
  fn new(groups: &'a [budget::Group]) -> Self {
    Self {
      groups,
      imported_group: None,
      memo: HashMap::new(),
      plan: ImportPlan::default(),
    }
  }

  fn imported_group_target(&mut self) -> GroupTarget {
    let index = *self.imported_group.get_or_insert_with(|| {
      self.plan.created_groups.push(PlannedGroup {
        name: t!("wallet.budget.pack_imported_group").into_owned(),
      });
      self.plan.created_groups.len() - 1
    });
    GroupTarget::Create(index)
  }

  fn plan_creation(&mut self, cat: &PortableCategory) -> CategoryTarget {
    let group = self
      .groups
      .iter()
      .find(|group| Some(group.id) == cat.group)
      .or_else(|| {
        self
          .groups
          .iter()
          .find(|group| group.name.eq_ignore_ascii_case(&cat.group_label))
      })
      .map(|group| GroupTarget::Existing(group.id))
      .unwrap_or_else(|| self.imported_group_target());
    self.plan.created_categories.push(PlannedCategory {
      group,
      name: cat.name.clone(),
      tone: cat.tone.clone(),
    });
    CategoryTarget::Create(self.plan.created_categories.len() - 1)
  }

  fn resolve(&mut self, cat: &PortableCategory) -> CategoryTarget {
    let key = match cat.id {
      Some(id) => format!("id:{id}"),
      None => format!("name:{}", cat.name.to_lowercase()),
    };
    if let Some(target) = self.memo.get(&key) {
      return *target;
    }
    let target = self.find_existing(cat).unwrap_or_else(|| self.plan_creation(cat));
    self.memo.insert(key, target);
    target
  }

  fn find_existing(&self, cat: &PortableCategory) -> Option<CategoryTarget> {
    let categories = self.groups.iter().flat_map(|group| &group.categories);
    if let Some(id) = cat.id
      && self
        .groups
        .iter()
        .flat_map(|group| &group.categories)
        .any(|category| category.id == id)
    {
      return Some(CategoryTarget::Existing(id));
    }
    categories
      .into_iter()
      .find(|category| category.name.eq_ignore_ascii_case(&cat.name))
      .map(|category| CategoryTarget::Existing(category.id))
  }
}

pub fn build_pack(rules: Vec<PortableRule>, name: &str, note: &str) -> PackEnvelope {
  let name = name.trim();
  PackEnvelope {
    author: String::new(),
    exported: chrono::Utc::now().timestamp_millis(),
    format: pod_pack::TAG_BUDGET_RULES.to_owned(),
    name: if name.is_empty() {
      t!("wallet.budget.pack_default_name").into_owned()
    } else {
      name.to_owned()
    },
    note: note.trim().to_owned(),
    rules,
    version: PACK_VERSION,
  }
}

/// Not atomic: each created group, category, and rule commits independently, so a failure
/// partway through can leave earlier steps' groups or categories behind.
pub async fn commit_import(
  db: &Database,
  plan: &ImportPlan,
  skipped: &BTreeSet<usize>,
  group_position_base: i64,
  rule_position_base: i64,
) -> Result<usize, crate::store::Error> {
  let kept: Vec<&PlanItem> = plan
    .items
    .iter()
    .enumerate()
    .filter(|(index, _)| !skipped.contains(index))
    .map(|(_, item)| item)
    .collect();
  let needed_categories: BTreeSet<usize> = kept
    .iter()
    .filter_map(|item| match item.target {
      CategoryTarget::Create(index) => Some(index),
      CategoryTarget::Existing(_) => None,
    })
    .collect();

  let group_ids = commit_groups(db, plan, &needed_categories, group_position_base).await?;
  let category_ids = commit_categories(db, plan, &needed_categories, &group_ids).await?;

  for (offset, item) in kept.iter().enumerate() {
    let category_id = match item.target {
      CategoryTarget::Create(index) => category_ids[&index],
      CategoryTarget::Existing(id) => id,
    };
    let rule = crate::store::repo::budget::create_rule(
      db,
      &NewRule {
        category_id,
        enabled: true,
        match_mode: item.match_mode,
        name: item.name.clone(),
        position: rule_position_base + offset as i64,
      },
    )
    .await?;
    crate::store::repo::budget::replace_rule_conditions(db, rule.id(), &item.conditions).await?;
  }
  Ok(kept.len())
}

pub fn encode_pack(pack: &PackEnvelope) -> Result<String, pod_pack::EncodeError> {
  pod_pack::encode(pod_pack::TAG_BUDGET_RULES, PACK_VERSION, pack)
}

pub fn is_duplicate(existing: &[Rule], category_id: i64, match_mode: MatchMode, conditions: &[RuleCondition]) -> bool {
  let signature = rule_signature(category_id, match_mode, conditions);
  existing
    .iter()
    .any(|rule| rule_signature(rule.category_id(), rule.match_mode(), rule.conditions()) == signature)
}

pub fn pack_file_name(name: &str) -> String {
  let mut slug = String::new();
  for ch in name.to_lowercase().chars() {
    if ch.is_ascii_alphanumeric() {
      slug.push(ch);
    } else if !slug.ends_with('-') {
      slug.push('-');
    }
  }
  let slug = slug.trim_matches('-');
  let slug = if slug.is_empty() { "budget-rules" } else { slug };
  format!("{slug}.{PACK_EXTENSION}")
}

pub fn parse_pack(input: &str) -> Result<PackEnvelope, ParseError> {
  let mut pack: PackEnvelope = pod_pack::decode(pod_pack::TAG_BUDGET_RULES, PACK_VERSION, input)?;
  // pod_pack::decode already checked the outer envelope's framing tag; this checks the payload's
  // own `format` field, which that framing doesn't constrain and can disagree with it.
  if pack.format != pod_pack::TAG_BUDGET_RULES {
    return Err(ParseError::WrongFormat);
  }
  if pack.rules.is_empty() {
    return Err(ParseError::Empty);
  }
  for rule in &mut pack.rules {
    normalize_rule(rule);
  }
  pack.rules.retain(|rule| !rule.conditions.is_empty());
  if pack.rules.is_empty() {
    return Err(ParseError::NoUsableConditions);
  }
  Ok(pack)
}

pub fn plan_import(pack: &PackEnvelope, existing_rules: &[Rule], groups: &[budget::Group]) -> ImportPlan {
  let mut resolver = Resolver::new(groups);
  let items = pack
    .rules
    .iter()
    .map(|rule| {
      let target = resolver.resolve(&rule.cat);
      let conditions = rule.conditions();
      let is_duplicate = match target {
        CategoryTarget::Existing(id) => is_duplicate(existing_rules, id, rule.match_mode(), &conditions),
        CategoryTarget::Create(_) => false,
      };
      PlanItem {
        conditions,
        is_duplicate,
        match_mode: rule.match_mode(),
        name: rule.name.clone(),
        target,
      }
    })
    .collect();
  let mut plan = resolver.plan;
  plan.items = items;
  plan
}

pub fn portable_rule(rule: &Rule, display_name: String, groups: &[budget::Group]) -> PortableRule {
  let category = groups
    .iter()
    .flat_map(|group| &group.categories)
    .find(|category| category.id == rule.category_id());
  let group = groups.iter().find(|group| {
    group
      .categories
      .iter()
      .any(|category| category.id == rule.category_id())
  });
  PortableRule {
    cat: PortableCategory {
      group: group.map(|group| group.id),
      group_label: group.map_or_else(
        || t!("wallet.budget.pack_imported_category").into_owned(),
        |group| group.name.clone(),
      ),
      id: Some(rule.category_id()),
      name: category.map_or_else(
        || t!("wallet.budget.pack_imported_category").into_owned(),
        |category| category.name.clone(),
      ),
      tone: category
        .and_then(|category| category.tone.clone())
        .unwrap_or_else(|| DEFAULT_TONE.to_owned()),
    },
    conditions: rule
      .conditions()
      .iter()
      .map(|condition| PortableCondition {
        field: condition.field().as_str().to_owned(),
        op: condition.op().as_str().to_owned(),
        value: condition.value().clone(),
        value2: condition.value2().clone(),
      })
      .collect(),
    match_mode: rule.match_mode().as_str().to_owned(),
    name: display_name,
  }
}

/// Joins fields with control characters rather than visible punctuation, so a condition value
/// containing the delimiter can't be crafted into a false signature match.
pub fn rule_signature(category_id: i64, match_mode: MatchMode, conditions: &[RuleCondition]) -> String {
  let mut parts: Vec<String> = conditions
    .iter()
    .filter(|condition| engine::is_active_condition(condition))
    .map(condition_signature)
    .collect();
  parts.sort();
  format!("{category_id}\u{3}{}\u{3}{}", match_mode.as_str(), parts.join("\u{2}"))
}

async fn commit_categories(
  db: &Database,
  plan: &ImportPlan,
  needed: &BTreeSet<usize>,
  group_ids: &HashMap<usize, i64>,
) -> Result<HashMap<usize, i64>, crate::store::Error> {
  let mut category_ids = HashMap::new();
  for index in needed {
    let planned = &plan.created_categories[*index];
    let group_id = match planned.group {
      GroupTarget::Create(group_index) => group_ids[&group_index],
      GroupTarget::Existing(id) => id,
    };
    let position = crate::store::repo::budget::list_categories(db, group_id).await?.len() as i64;
    let category = crate::store::repo::budget::create_category(
      db,
      &NewCategory {
        group_id,
        name: planned.name.clone(),
        note: Some(t!("wallet.budget.pack_imported_category_note").into_owned()),
        position,
        tone: Some(planned.tone.clone()),
      },
    )
    .await?;
    category_ids.insert(*index, category.id());
  }
  Ok(category_ids)
}

async fn commit_groups(
  db: &Database,
  plan: &ImportPlan,
  needed_categories: &BTreeSet<usize>,
  group_position_base: i64,
) -> Result<HashMap<usize, i64>, crate::store::Error> {
  let needed_groups: BTreeSet<usize> = needed_categories
    .iter()
    .filter_map(|index| match plan.created_categories[*index].group {
      GroupTarget::Create(group_index) => Some(group_index),
      GroupTarget::Existing(_) => None,
    })
    .collect();
  let mut group_ids = HashMap::new();
  for (offset, index) in needed_groups.into_iter().enumerate() {
    let group = crate::store::repo::budget::create_group(
      db,
      &NewGroup {
        name: plan.created_groups[index].name.clone(),
        position: group_position_base + offset as i64,
      },
    )
    .await?;
    group_ids.insert(index, group.id());
  }
  Ok(group_ids)
}

fn condition_signature(condition: &RuleCondition) -> String {
  let mut part = format!(
    "{}\u{1}{}\u{1}{}",
    condition.field().as_str(),
    condition.op().as_str(),
    condition.value().trim().to_lowercase()
  );
  if condition.op() == RuleOp::Between {
    part.push('\u{1}');
    part.push_str(&condition.value2().as_deref().unwrap_or_default().trim().to_lowercase());
  }
  part
}

fn normalize_rule(rule: &mut PortableRule) {
  rule
    .conditions
    .retain(|condition| engine::is_active_condition(&condition.to_condition()));
  rule.match_mode = MatchMode::from_key(&rule.match_mode).as_str().to_owned();
  if rule.name.trim().is_empty() {
    rule.name = t!("wallet.budget.rule_untitled").into_owned();
  }
  if !budget::tone_options().contains(&rule.cat.tone.as_str()) {
    rule.cat.tone = DEFAULT_TONE.to_owned();
  }
  if rule.cat.name.trim().is_empty() {
    rule.cat.name = t!("wallet.budget.pack_imported_category").into_owned();
  }
}

impl From<pod_pack::DecodeError> for ParseError {
  fn from(error: pod_pack::DecodeError) -> Self {
    match error {
      pod_pack::DecodeError::UnsupportedVersion {
        ..
      } => ParseError::UnsupportedVersion,
      pod_pack::DecodeError::WrongFormat {
        ..
      } => ParseError::WrongFormat,
      _ => ParseError::NotAPack,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn category(id: i64, name: &str) -> budget::Category {
    budget::Category {
      activity: 0.0,
      assigned: 0.0,
      avg_assigned: 0.0,
      carry: 0.0,
      id,
      last_assigned: 0.0,
      name: name.to_owned(),
      note: None,
      spent_last: 0.0,
      target: budget::Target::default(),
      tone: Some("plasma".to_owned()),
    }
  }

  fn groups() -> Vec<budget::Group> {
    vec![budget::Group {
      categories: vec![category(1, "Ammo"), category(2, "Fees")],
      id: 10,
      name: "Operations".to_owned(),
    }]
  }

  fn sample_rule(id: i64, category_id: i64, value: &str) -> Rule {
    Rule {
      category_id,
      conditions: vec![RuleCondition {
        field: RuleField::Text,
        op: RuleOp::Contains,
        value: value.to_owned(),
        value2: None,
      }],
      enabled: true,
      id,
      match_mode: MatchMode::All,
      name: format!("Rule {id}"),
    }
  }

  fn sample_pack() -> PackEnvelope {
    let rules = vec![
      portable_rule(&sample_rule(1, 1, "Missile"), "Ammo restock".to_owned(), &groups()),
      portable_rule(&sample_rule(2, 2, "broker"), "Broker fees".to_owned(), &groups()),
    ];
    build_pack(rules, "Test pack", "For the corp")
  }

  mod round_trip {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_a_pack_through_the_codec() {
      let pack = sample_pack();
      let encoded = encode_pack(&pack).unwrap();

      let decoded = parse_pack(&encoded).unwrap();

      assert_eq!(decoded, pack);
    }

    #[test]
    fn it_preserves_rule_names_match_modes_conditions_and_envelopes() {
      let mut rule = sample_rule(1, 1, "Missile");
      rule.match_mode = MatchMode::Any;
      rule.conditions.push(RuleCondition {
        field: RuleField::Amount,
        op: RuleOp::Between,
        value: "100m".to_owned(),
        value2: Some("1b".to_owned()),
      });
      let pack = build_pack(
        vec![portable_rule(&rule, "Ammo restock".to_owned(), &groups())],
        "Pack",
        "",
      );

      let decoded = parse_pack(&encode_pack(&pack).unwrap()).unwrap();

      let out = &decoded.rules[0];
      assert_eq!(out.name, "Ammo restock");
      assert_eq!(out.match_mode(), MatchMode::Any);
      assert_eq!(out.conditions(), rule.conditions);
      assert_eq!(out.cat.name, "Ammo");
      assert_eq!(out.cat.tone, "plasma");
      assert_eq!(out.cat.group_label, "Operations");
    }
  }

  mod parse_pack {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_rejects_plain_text() {
      assert_eq!(parse_pack("just some text"), Err(ParseError::NotAPack));
    }

    #[test]
    fn it_rejects_bare_json() {
      let json = serde_json::to_string(&sample_pack()).unwrap();

      assert_eq!(parse_pack(&json), Err(ParseError::NotAPack));
    }

    #[test]
    fn it_rejects_a_truncated_pack() {
      let encoded = encode_pack(&sample_pack()).unwrap();

      let result = parse_pack(&encoded[..encoded.len() / 2]);

      assert_eq!(result, Err(ParseError::NotAPack));
    }

    #[test]
    fn it_rejects_a_wrong_format_tag() {
      let encoded = pod_pack::encode(pod_pack::TAG_SKILL_PLAN, PACK_VERSION, &sample_pack()).unwrap();

      assert_eq!(parse_pack(&encoded), Err(ParseError::WrongFormat));
    }

    #[test]
    fn it_rejects_a_mismatched_envelope_format_field() {
      let mut pack = sample_pack();
      pack.format = "pod.skill-plan".to_owned();
      let encoded = encode_pack(&pack).unwrap();

      assert_eq!(parse_pack(&encoded), Err(ParseError::WrongFormat));
    }

    #[test]
    fn it_rejects_an_unsupported_version() {
      let encoded = pod_pack::encode(pod_pack::TAG_BUDGET_RULES, PACK_VERSION + 1, &sample_pack()).unwrap();

      assert_eq!(parse_pack(&encoded), Err(ParseError::UnsupportedVersion));
    }

    #[test]
    fn it_rejects_an_empty_pack() {
      let encoded = encode_pack(&build_pack(Vec::new(), "Empty", "")).unwrap();

      assert_eq!(parse_pack(&encoded), Err(ParseError::Empty));
    }

    #[test]
    fn it_rejects_a_pack_whose_rules_have_no_usable_conditions() {
      let mut pack = sample_pack();
      for rule in &mut pack.rules {
        for condition in &mut rule.conditions {
          condition.value = "   ".to_owned();
        }
      }
      let encoded = encode_pack(&pack).unwrap();

      assert_eq!(parse_pack(&encoded), Err(ParseError::NoUsableConditions));
    }

    #[test]
    fn it_normalizes_unknown_tones_and_blank_names() {
      let mut pack = sample_pack();
      pack.rules[0].cat.tone = "sparkle".to_owned();
      pack.rules[0].name = "  ".to_owned();
      let encoded = encode_pack(&pack).unwrap();

      let decoded = parse_pack(&encoded).unwrap();

      assert_eq!(decoded.rules[0].cat.tone, "muted");
      assert!(!decoded.rules[0].name.trim().is_empty());
    }

    #[test]
    fn it_is_not_plain_text_readable() {
      let encoded = encode_pack(&sample_pack()).unwrap();

      assert!(!encoded.contains("Ammo restock"));
      assert!(!encoded.contains("pod.budget-rules"));
    }
  }

  mod plan_import {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reuses_a_category_matched_by_id() {
      let plan = plan_import(&sample_pack(), &[], &groups());

      assert_eq!(plan.items[0].target, CategoryTarget::Existing(1));
      assert!(plan.created_categories.is_empty());
      assert!(plan.created_groups.is_empty());
    }

    #[test]
    fn it_reuses_a_category_matched_by_name_when_the_id_is_unknown() {
      let mut pack = sample_pack();
      pack.rules[0].cat.id = Some(999);
      pack.rules[0].cat.name = "FEES".to_owned();

      let plan = plan_import(&pack, &[], &groups());

      assert_eq!(plan.items[0].target, CategoryTarget::Existing(2));
    }

    #[test]
    fn it_creates_a_missing_category_in_the_matching_group() {
      let mut pack = sample_pack();
      pack.rules[0].cat.id = Some(999);
      pack.rules[0].cat.name = "SRP".to_owned();

      let plan = plan_import(&pack, &[], &groups());

      assert_eq!(plan.items[0].target, CategoryTarget::Create(0));
      assert_eq!(plan.created_categories[0].group, GroupTarget::Existing(10));
      assert_eq!(plan.created_categories[0].name, "SRP");
      assert!(plan.created_groups.is_empty());
    }

    #[test]
    fn it_creates_an_imported_rules_group_when_no_group_matches() {
      let mut pack = sample_pack();
      pack.rules[0].cat = PortableCategory {
        group: Some(77),
        group_label: "Elsewhere".to_owned(),
        id: Some(999),
        name: "SRP".to_owned(),
        tone: "danger".to_owned(),
      };

      let plan = plan_import(&pack, &[], &groups());

      assert_eq!(plan.created_groups.len(), 1);
      assert_eq!(plan.created_categories[0].group, GroupTarget::Create(0));
    }

    #[test]
    fn it_shares_one_created_envelope_between_rules() {
      let mut pack = sample_pack();
      let missing = PortableCategory {
        group: None,
        group_label: "Elsewhere".to_owned(),
        id: Some(999),
        name: "SRP".to_owned(),
        tone: "danger".to_owned(),
      };
      pack.rules[0].cat = missing.clone();
      pack.rules[1].cat = missing;

      let plan = plan_import(&pack, &[], &groups());

      assert_eq!(plan.items[0].target, CategoryTarget::Create(0));
      assert_eq!(plan.items[1].target, CategoryTarget::Create(0));
      assert_eq!(plan.created_categories.len(), 1);
      assert_eq!(plan.created_groups.len(), 1);
    }

    #[test]
    fn it_flags_a_likely_duplicate() {
      let existing = vec![sample_rule(50, 1, "Missile")];

      let plan = plan_import(&sample_pack(), &existing, &groups());

      assert!(plan.items[0].is_duplicate);
      assert!(!plan.items[1].is_duplicate);
    }

    #[test]
    fn it_ignores_condition_order_and_case_for_duplicates() {
      let mut existing = sample_rule(50, 1, "Missile");
      existing.conditions.push(RuleCondition {
        field: RuleField::Party,
        op: RuleOp::Is,
        value: "Jita Trader".to_owned(),
        value2: None,
      });
      let mut rule = sample_rule(1, 1, "party");
      rule.conditions = vec![
        RuleCondition {
          field: RuleField::Party,
          op: RuleOp::Is,
          value: "  JITA TRADER ".to_owned(),
          value2: None,
        },
        RuleCondition {
          field: RuleField::Text,
          op: RuleOp::Contains,
          value: "missile".to_owned(),
          value2: None,
        },
      ];
      let pack = build_pack(vec![portable_rule(&rule, "Dup".to_owned(), &groups())], "P", "");

      let plan = plan_import(&pack, &[existing], &groups());

      assert!(plan.items[0].is_duplicate);
    }
  }

  mod pack_file_name {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_slugifies_the_pack_name() {
      assert_eq!(pack_file_name("Goon SRP doctrine v3"), "goon-srp-doctrine-v3.pbr");
    }

    #[test]
    fn it_falls_back_for_an_unusable_name() {
      assert_eq!(pack_file_name("  ·· "), "budget-rules.pbr");
    }
  }

  mod commit_import {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seeded_db() -> (Database, i64) {
      let db = crate::store::open_test().await.unwrap();
      let group = crate::store::repo::budget::create_group(
        &db,
        &NewGroup {
          name: "Operations".to_owned(),
          position: 0,
        },
      )
      .await
      .unwrap();
      (db, group.id())
    }

    #[tokio::test]
    async fn it_imports_rules_creating_missing_envelopes() {
      let (db, group_id) = seeded_db().await;
      let mut pack = sample_pack();
      pack.rules[0].cat = PortableCategory {
        group: Some(group_id),
        group_label: "Operations".to_owned(),
        id: Some(999),
        name: "SRP".to_owned(),
        tone: "danger".to_owned(),
      };
      pack.rules[1].cat = PortableCategory {
        group: None,
        group_label: "Elsewhere".to_owned(),
        id: None,
        name: "Skins".to_owned(),
        tone: "info".to_owned(),
      };
      let live_groups = vec![budget::Group {
        categories: Vec::new(),
        id: group_id,
        name: "Operations".to_owned(),
      }];
      let plan = plan_import(&pack, &[], &live_groups);

      let added = commit_import(&db, &plan, &BTreeSet::new(), 1, 0).await.unwrap();

      assert_eq!(added, 2);
      let rules = crate::store::repo::budget::list_rules(&db).await.unwrap();
      assert_eq!(rules.len(), 2);
      assert_eq!(rules[0].name(), "Ammo restock");
      assert_eq!(rules[0].conditions().len(), 1);
      let groups = crate::store::repo::budget::list_groups(&db).await.unwrap();
      assert_eq!(groups.len(), 2);
      let created = crate::store::repo::budget::list_categories(&db, group_id)
        .await
        .unwrap();
      assert_eq!(created[0].name(), "SRP");
      assert_eq!(created[0].tone().as_deref(), Some("danger"));
    }

    #[tokio::test]
    async fn it_skips_rules_and_their_unneeded_envelopes() {
      let (db, group_id) = seeded_db().await;
      let mut pack = sample_pack();
      pack.rules[0].cat = PortableCategory {
        group: None,
        group_label: "Elsewhere".to_owned(),
        id: None,
        name: "SRP".to_owned(),
        tone: "danger".to_owned(),
      };
      pack.rules[1].cat = pack.rules[0].cat.clone();
      let live_groups = vec![budget::Group {
        categories: Vec::new(),
        id: group_id,
        name: "Operations".to_owned(),
      }];
      let plan = plan_import(&pack, &[], &live_groups);
      let skipped: BTreeSet<usize> = [0, 1].into();

      let added = commit_import(&db, &plan, &skipped, 1, 0).await.unwrap();

      assert_eq!(added, 0);
      assert!(crate::store::repo::budget::list_rules(&db).await.unwrap().is_empty());
      assert_eq!(crate::store::repo::budget::list_groups(&db).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_round_trips_export_to_import() {
      let (db, group_id) = seeded_db().await;
      let category = crate::store::repo::budget::create_category(
        &db,
        &NewCategory {
          group_id,
          name: "Ammo".to_owned(),
          note: None,
          position: 0,
          tone: Some("plasma".to_owned()),
        },
      )
      .await
      .unwrap();
      let mut rule = sample_rule(0, category.id(), "Missile");
      rule.match_mode = MatchMode::Any;
      let live_groups = vec![budget::Group {
        categories: vec![budget::Category {
          activity: 0.0,
          assigned: 0.0,
          avg_assigned: 0.0,
          carry: 0.0,
          id: category.id(),
          last_assigned: 0.0,
          name: "Ammo".to_owned(),
          note: None,
          spent_last: 0.0,
          target: budget::Target::default(),
          tone: Some("plasma".to_owned()),
        }],
        id: group_id,
        name: "Operations".to_owned(),
      }];
      let pack = build_pack(
        vec![portable_rule(&rule, "Ammo restock".to_owned(), &live_groups)],
        "Pack",
        "",
      );
      let decoded = parse_pack(&encode_pack(&pack).unwrap()).unwrap();
      let plan = plan_import(&decoded, &[], &live_groups);

      commit_import(&db, &plan, &BTreeSet::new(), 1, 0).await.unwrap();

      let rules = crate::store::repo::budget::list_rules(&db).await.unwrap();
      assert_eq!(rules.len(), 1);
      assert_eq!(rules[0].name(), "Ammo restock");
      assert_eq!(rules[0].match_mode(), MatchMode::Any);
      assert_eq!(rules[0].category_id(), category.id());
      assert_eq!(rules[0].conditions(), &rule.conditions);
    }
  }
}
