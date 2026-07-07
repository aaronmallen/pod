use std::collections::HashMap;

use getset::Getters;
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  pub answers: HashMap<String, String>,
  #[getset(get = "pub")]
  pub blocked: Option<String>,
  #[getset(get = "pub")]
  pub build: Option<String>,
  #[getset(get = "pub")]
  pub combat: Option<String>,
  #[getset(get = "pub")]
  pub created_at: String,
  #[getset(get = "pub")]
  pub date: String,
  #[getset(get = "pub")]
  pub goal: Option<String>,
  #[getset(get = "pub")]
  pub marked_complete: bool,
  #[getset(get = "pub")]
  pub narrative: Option<String>,
  #[getset(get = "pub")]
  pub next: Option<String>,
  #[getset(get = "pub")]
  pub remember: Option<String>,
  #[getset(get = "pub")]
  pub research: Option<String>,
  #[getset(get = "pub")]
  pub skill: Option<String>,
  #[getset(get = "pub")]
  pub updated_at: String,
}

#[allow(dead_code)]
pub const PROMPT_CONFIG_VERSION: u32 = 2;

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptConfig {
  pub version: u32,
  pub sections: Vec<PromptSection>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptSection {
  pub id: String,
  pub kind: PromptSectionKind,
  pub label: String,
  pub i18n_key: String,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub questions: Vec<PromptQuestion>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub triggers: Option<PromptTriggers>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptSectionKind {
  Conditional,
  Free,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptQuestion {
  pub id: String,
  pub kind: PromptQuestionKind,
  pub label: String,
  pub i18n_key: String,
  #[serde(default)]
  pub placeholder: String,
  #[serde(default)]
  pub required: bool,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptQuestionKind {
  #[default]
  Text,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptTriggers {
  #[serde(default = "enabled")]
  pub build: bool,
  #[serde(default = "enabled")]
  pub combat: bool,
  #[serde(default = "enabled")]
  pub skill: bool,
}

fn enabled() -> bool {
  true
}

impl Default for PromptTriggers {
  fn default() -> Self {
    PromptTriggers {
      build: true,
      combat: true,
      skill: true,
    }
  }
}

#[allow(dead_code)]
impl PromptConfig {
  pub fn normalize(&mut self) {
    for section in &mut self.sections {
      if section.kind == PromptSectionKind::Conditional {
        section.triggers.get_or_insert_with(PromptTriggers::default);
      }
    }
  }

  fn free_section(id: &str, i18n_key: &str, questions: Vec<PromptQuestion>) -> PromptSection {
    PromptSection {
      id: id.to_owned(),
      kind: PromptSectionKind::Free,
      label: String::new(),
      i18n_key: i18n_key.to_owned(),
      questions,
      triggers: None,
    }
  }

  fn question(id: &str, i18n_key: &str, required: bool) -> PromptQuestion {
    PromptQuestion {
      id: id.to_owned(),
      kind: PromptQuestionKind::Text,
      label: String::new(),
      i18n_key: i18n_key.to_owned(),
      placeholder: String::new(),
      required,
    }
  }
}

impl Default for PromptConfig {
  fn default() -> Self {
    PromptConfig {
      version: PROMPT_CONFIG_VERSION,
      sections: vec![
        PromptConfig::free_section(
          "core",
          "captains_log.wizard.group_core",
          vec![
            PromptConfig::question("goal", "captains_log.wizard.goal_label", true),
            PromptConfig::question("remember", "captains_log.wizard.remember_label", false),
            PromptConfig::question("blocked", "captains_log.wizard.blocked_label", false),
          ],
        ),
        PromptSection {
          id: "conditional".to_owned(),
          kind: PromptSectionKind::Conditional,
          label: String::new(),
          i18n_key: "captains_log.wizard.group_conditional".to_owned(),
          questions: Vec::new(),
          triggers: Some(PromptTriggers::default()),
        },
        PromptConfig::free_section(
          "forward",
          "captains_log.wizard.group_forward",
          vec![
            PromptConfig::question("next", "captains_log.wizard.next_label", false),
            PromptConfig::question("research", "captains_log.wizard.research_label", false),
          ],
        ),
      ],
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod default {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_ships_three_sections_in_display_order() {
      let config = PromptConfig::default();

      let ids: Vec<&str> = config.sections.iter().map(|section| section.id.as_str()).collect();

      assert_eq!(ids, vec!["core", "conditional", "forward"]);
    }

    #[test]
    fn it_marks_only_the_goal_question_required() {
      let config = PromptConfig::default();

      let required: Vec<&str> = config
        .sections
        .iter()
        .flat_map(|section| &section.questions)
        .filter(|question| question.required)
        .map(|question| question.id.as_str())
        .collect();

      assert_eq!(required, vec!["goal"]);
    }

    #[test]
    fn it_enables_every_conditional_trigger() {
      let config = PromptConfig::default();

      let triggers = config
        .sections
        .iter()
        .find(|section| section.kind == PromptSectionKind::Conditional)
        .and_then(|section| section.triggers)
        .unwrap();

      assert_eq!(triggers, PromptTriggers::default());
      assert!(triggers.build && triggers.combat && triggers.skill);
    }
  }

  mod normalize {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn it_fills_a_missing_conditional_trigger_block() {
      let mut config: PromptConfig = serde_json::from_value(json!({
        "version": 1,
        "sections": [
          { "id": "conditional", "kind": "conditional", "label": "", "i18n_key": "x" }
        ]
      }))
      .unwrap();

      config.normalize();

      assert_eq!(config.sections[0].triggers, Some(PromptTriggers::default()));
    }

    #[test]
    fn it_defaults_absent_individual_triggers_to_enabled() {
      let config: PromptConfig = serde_json::from_value(json!({
        "version": 2,
        "sections": [
          { "id": "conditional", "kind": "conditional", "label": "", "i18n_key": "x", "triggers": { "combat": false } }
        ]
      }))
      .unwrap();

      let triggers = config.sections[0].triggers.unwrap();

      assert!(!triggers.combat);
      assert!(triggers.build);
      assert!(triggers.skill);
    }
  }

  mod round_trip {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_survives_a_json_round_trip() {
      let config = PromptConfig::default();

      let text = serde_json::to_string(&config).unwrap();
      let parsed: PromptConfig = serde_json::from_str(&text).unwrap();

      assert_eq!(parsed, config);
    }
  }
}
