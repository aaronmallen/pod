use crate::store::{
  Database, Error,
  model::{CaptainsLog, KillmailReport, PromptConfig, PromptSection, PromptSectionKind, PromptTriggers},
  repo::{captains_log, captains_log::AnswerKey, killmail_report},
};

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Completeness {
  pub missing_custom: Vec<String>,
  pub missing_debriefs: Vec<LossEngagement>,
  pub missing_prompts: Vec<AnswerKey>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DayActivity {
  pub engagement_count: u32,
  pub industry: Vec<IndustryEvidence>,
  pub industry_count: u32,
  pub losses: Vec<LossEngagement>,
  pub skill_count: u32,
  pub skills: Vec<SkillEvidence>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LossEngagement {
  pub character_id: i64,
  pub killmail_id: i64,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SkillEvidence {
  pub character_id: i64,
  pub character_name: String,
  pub level: i64,
  pub skill: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IndustryEvidence {
  pub character_id: i64,
  pub character_name: String,
  pub product: String,
  pub product_type_id: Option<i64>,
  pub runs: i64,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Prompt {
  pub group: PromptGroup,
  pub i18n_key: String,
  pub id: String,
  pub key: Option<AnswerKey>,
  pub label: String,
  pub placeholder: String,
  pub required: bool,
  pub section_i18n_key: String,
  pub section_label: String,
  pub trigger: Option<Trigger>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptGroup {
  Conditional,
  Core,
  Custom,
  Forward,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Trigger {
  Engagement,
  Industry,
  Skills,
}

#[allow(dead_code)]
impl Completeness {
  pub fn is_complete(&self) -> bool {
    self.missing_custom.is_empty() && self.missing_debriefs.is_empty() && self.missing_prompts.is_empty()
  }
}

impl Prompt {
  fn applies(&self, activity: &DayActivity) -> bool {
    match self.trigger {
      None => true,
      Some(trigger) => trigger.fired(activity),
    }
  }
}

impl Trigger {
  fn fired(self, activity: &DayActivity) -> bool {
    match self {
      Trigger::Engagement => activity.engagement_count > 0,
      Trigger::Industry => activity.industry_count > 0,
      Trigger::Skills => activity.skill_count > 0,
    }
  }
}

#[allow(dead_code)]
pub fn completeness(
  config: &PromptConfig,
  activity: &DayActivity,
  log: Option<&CaptainsLog>,
  reports: &[KillmailReport],
) -> Completeness {
  if log.is_some_and(|log| log.marked_complete) {
    return Completeness::default();
  }

  let (missing_prompts, missing_custom) = missing_required_prompts(config, log);
  Completeness {
    missing_custom,
    missing_debriefs: missing_loss_debriefs(config, activity, reports),
    missing_prompts,
  }
}

#[allow(dead_code)]
pub async fn completeness_for_day(
  db: &Database,
  date: &str,
  character_ids: &[i64],
  activity: &DayActivity,
) -> Result<Completeness, Error> {
  let config = captains_log::load_prompt_config(db).await?;
  let log = captains_log::get(db, date).await?;
  let reports = killmail_report::list_for_day(db, character_ids, date).await?;

  Ok(completeness(&config, activity, log.as_ref(), &reports))
}

/// The wizard's ordered prompts for the day: every configured question, plus the enabled
/// conditional built-ins gated by both their account trigger and the day's activity.
#[allow(dead_code)]
pub fn prompts_for_day(config: &PromptConfig, activity: &DayActivity) -> Vec<Prompt> {
  config
    .sections
    .iter()
    .flat_map(section_prompts)
    .filter(|prompt| prompt.applies(activity))
    .collect()
}

/// The past view's ordered prompts: every configured question plus the enabled non-combat
/// conditional built-ins (combat is judged by loss debriefs, not a text answer), with no
/// activity gate so an answered field always renders.
#[allow(dead_code)]
pub fn all_field_prompts(config: &PromptConfig) -> Vec<Prompt> {
  config
    .sections
    .iter()
    .flat_map(section_prompts)
    .filter(|prompt| prompt.key != Some(AnswerKey::Combat))
    .collect()
}

fn section_prompts(section: &PromptSection) -> Vec<Prompt> {
  match section.kind {
    PromptSectionKind::Conditional => conditional_prompts(section),
    PromptSectionKind::Free => free_prompts(section),
  }
}

fn free_prompts(section: &PromptSection) -> Vec<Prompt> {
  let group = free_group(&section.id);
  section
    .questions
    .iter()
    .map(|question| Prompt {
      group,
      i18n_key: question.i18n_key.clone(),
      id: question.id.clone(),
      key: AnswerKey::from_key(&question.id),
      label: question.label.clone(),
      placeholder: question.placeholder.clone(),
      required: question.required,
      section_i18n_key: section.i18n_key.clone(),
      section_label: section.label.clone(),
      trigger: None,
    })
    .collect()
}

fn conditional_prompts(section: &PromptSection) -> Vec<Prompt> {
  let triggers = section.triggers.unwrap_or_default();
  let mut out = Vec::new();
  if triggers.combat {
    out.push(builtin(section, AnswerKey::Combat, Trigger::Engagement, true));
  }
  if triggers.build {
    out.push(builtin(section, AnswerKey::Build, Trigger::Industry, false));
  }
  if triggers.skill {
    out.push(builtin(section, AnswerKey::Skill, Trigger::Skills, false));
  }
  out
}

/// Label/placeholder/i18n_key stay blank: the wizard resolves a built-in's display text from a
/// fixed `{key}_label`/`_placeholder` i18n key instead, so these fields go unused for built-ins.
fn builtin(section: &PromptSection, key: AnswerKey, trigger: Trigger, required: bool) -> Prompt {
  Prompt {
    group: PromptGroup::Conditional,
    i18n_key: String::new(),
    id: key.as_key().to_owned(),
    key: Some(key),
    label: String::new(),
    placeholder: String::new(),
    required,
    section_i18n_key: section.i18n_key.clone(),
    section_label: section.label.clone(),
    trigger: Some(trigger),
  }
}

/// Matches `PromptConfig::default()`'s hardcoded section ids ("core"/"forward"); a config using
/// different ids for those roles would silently land its questions in `Custom` instead.
fn free_group(section_id: &str) -> PromptGroup {
  match section_id {
    "core" => PromptGroup::Core,
    "forward" => PromptGroup::Forward,
    _ => PromptGroup::Custom,
  }
}

fn combat_enabled(config: &PromptConfig) -> bool {
  config
    .sections
    .iter()
    .find(|section| section.kind == PromptSectionKind::Conditional)
    .map(|section| section.triggers.unwrap_or_default())
    .unwrap_or(PromptTriggers {
      build: false,
      combat: false,
      skill: false,
    })
    .combat
}

fn answer_text(log: &CaptainsLog, key: AnswerKey) -> Option<&str> {
  match key {
    AnswerKey::Blocked => log.blocked().as_deref(),
    AnswerKey::Build => log.build().as_deref(),
    AnswerKey::Combat => log.combat().as_deref(),
    AnswerKey::Goal => log.goal().as_deref(),
    AnswerKey::Next => log.next().as_deref(),
    AnswerKey::Remember => log.remember().as_deref(),
    AnswerKey::Research => log.research().as_deref(),
    AnswerKey::Skill => log.skill().as_deref(),
  }
}

fn has_report(reports: &[KillmailReport], loss: &LossEngagement) -> bool {
  reports
    .iter()
    .any(|report| report.character_id() == loss.character_id && report.killmail_id() == loss.killmail_id)
}

fn is_answered(log: Option<&CaptainsLog>, id: &str, key: Option<AnswerKey>) -> bool {
  let Some(log) = log else {
    return false;
  };
  let text = match key {
    Some(key) => answer_text(log, key),
    None => log.answers().get(id).map(String::as_str),
  };
  text.is_some_and(|text| !text.trim().is_empty())
}

fn missing_loss_debriefs(
  config: &PromptConfig,
  activity: &DayActivity,
  reports: &[KillmailReport],
) -> Vec<LossEngagement> {
  if !combat_enabled(config) {
    return Vec::new();
  }
  activity
    .losses
    .iter()
    .filter(|loss| !has_report(reports, loss))
    .copied()
    .collect()
}

/// A required free-section question left blank drives the needs-info flag exactly like the
/// default required goal. Catalog questions land in `missing_prompts` (for the MCP surface and
/// the goal-specific rail label); custom questions carry their resolved label in `missing_custom`.
fn missing_required_prompts(config: &PromptConfig, log: Option<&CaptainsLog>) -> (Vec<AnswerKey>, Vec<String>) {
  let mut prompts = Vec::new();
  let mut custom = Vec::new();

  for section in &config.sections {
    if section.kind != PromptSectionKind::Free {
      continue;
    }
    for question in &section.questions {
      let key = AnswerKey::from_key(&question.id);
      if !question.required || is_answered(log, &question.id, key) {
        continue;
      }
      match key {
        Some(key) => prompts.push(key),
        None => custom.push(missing_label(question)),
      }
    }
  }

  (prompts, custom)
}

fn missing_label(question: &crate::store::model::PromptQuestion) -> String {
  if question.i18n_key.is_empty() {
    question.label.clone()
  } else {
    t!(&question.i18n_key).into_owned()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn loss(character_id: i64, killmail_id: i64) -> LossEngagement {
    LossEngagement {
      character_id,
      killmail_id,
    }
  }

  fn report(character_id: i64, killmail_id: i64) -> KillmailReport {
    KillmailReport {
      character_id,
      created_at: "2026-07-06T00:00:00Z".to_owned(),
      different: None,
      happened: "Warped in too hot.".to_owned(),
      killmail_id,
      outcome: "learning".to_owned(),
      takeaway: None,
      updated_at: "2026-07-06T00:00:00Z".to_owned(),
    }
  }

  fn with_goal() -> CaptainsLog {
    CaptainsLog {
      goal: Some("Spin up the barge line.".to_owned()),
      ..CaptainsLog::default()
    }
  }

  mod prompts_for_day {
    use pretty_assertions::assert_eq;

    use super::*;

    fn keys(prompts: &[Prompt]) -> Vec<&str> {
      prompts.iter().map(|prompt| prompt.id.as_str()).collect()
    }

    #[test]
    fn it_shows_only_core_and_forward_prompts_on_a_quiet_day() {
      let prompts = prompts_for_day(&PromptConfig::default(), &DayActivity::default());

      assert_eq!(keys(&prompts), vec!["goal", "remember", "blocked", "next", "research"]);
    }

    #[test]
    fn it_adds_the_combat_prompt_when_any_engagement_happened() {
      let activity = DayActivity {
        engagement_count: 2,
        ..DayActivity::default()
      };

      let prompts = prompts_for_day(&PromptConfig::default(), &activity);

      assert!(keys(&prompts).contains(&"combat"));
      assert!(!keys(&prompts).contains(&"build"));
      assert!(!keys(&prompts).contains(&"skill"));
    }

    #[test]
    fn it_adds_every_conditional_prompt_on_a_busy_day() {
      let activity = DayActivity {
        engagement_count: 3,
        industry_count: 1,
        losses: vec![loss(4, 100)],
        skill_count: 2,
        ..DayActivity::default()
      };

      let prompts = prompts_for_day(&PromptConfig::default(), &activity);

      assert_eq!(
        keys(&prompts),
        vec![
          "goal", "remember", "blocked", "combat", "build", "skill", "next", "research"
        ]
      );
    }

    #[test]
    fn it_honors_a_disabled_conditional_trigger() {
      let mut config = PromptConfig::default();
      if let Some(section) = config
        .sections
        .iter_mut()
        .find(|section| section.kind == PromptSectionKind::Conditional)
      {
        section.triggers = Some(PromptTriggers {
          build: true,
          combat: false,
          skill: true,
        });
      }
      let activity = DayActivity {
        engagement_count: 2,
        industry_count: 1,
        skill_count: 1,
        ..DayActivity::default()
      };

      let prompts = prompts_for_day(&config, &activity);

      assert!(!keys(&prompts).contains(&"combat"));
      assert!(keys(&prompts).contains(&"build"));
    }

    #[test]
    fn it_appends_custom_questions_in_config_order() {
      let mut config = PromptConfig::default();
      config.sections[0].questions.push(crate::store::model::PromptQuestion {
        id: "custom_mood".to_owned(),
        kind: crate::store::model::PromptQuestionKind::Text,
        label: "How did it feel?".to_owned(),
        i18n_key: String::new(),
        placeholder: String::new(),
        required: false,
      });

      let prompts = prompts_for_day(&config, &DayActivity::default());
      let custom = prompts.iter().find(|prompt| prompt.id == "custom_mood").unwrap();

      assert_eq!(custom.key, None);
      assert_eq!(custom.group, PromptGroup::Core);
      assert_eq!(custom.label, "How did it feel?");
    }
  }

  mod all_field_prompts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_lists_every_question_except_combat_regardless_of_activity() {
      let prompts = all_field_prompts(&PromptConfig::default());
      let ids: Vec<&str> = prompts.iter().map(|prompt| prompt.id.as_str()).collect();

      assert_eq!(
        ids,
        vec!["goal", "remember", "blocked", "build", "skill", "next", "research"]
      );
    }
  }

  mod marked_complete {
    use super::*;

    #[test]
    fn it_short_circuits_every_check_when_the_day_is_marked_complete() {
      let activity = DayActivity {
        engagement_count: 1,
        losses: vec![loss(1, 2)],
        ..DayActivity::default()
      };
      let log = CaptainsLog {
        date: "2026-07-01".to_owned(),
        marked_complete: true,
        ..CaptainsLog::default()
      };

      let result = completeness(&PromptConfig::default(), &activity, Some(&log), &[]);

      assert!(result.is_complete());
    }
  }

  mod completeness {
    use pretty_assertions::assert_eq;

    use super::*;

    fn require_custom(config: &mut PromptConfig, id: &str, label: &str) {
      config.sections[0].questions.push(crate::store::model::PromptQuestion {
        id: id.to_owned(),
        kind: crate::store::model::PromptQuestionKind::Text,
        label: label.to_owned(),
        i18n_key: String::new(),
        placeholder: String::new(),
        required: true,
      });
    }

    #[test]
    fn it_flags_a_missing_goal_on_a_quiet_day() {
      let result = completeness(&PromptConfig::default(), &DayActivity::default(), None, &[]);

      assert_eq!(result.missing_prompts, vec![AnswerKey::Goal]);
      assert!(result.missing_custom.is_empty());
      assert!(result.missing_debriefs.is_empty());
      assert!(!result.is_complete());
    }

    #[test]
    fn it_reports_a_quiet_day_with_a_goal_as_complete() {
      let result = completeness(
        &PromptConfig::default(),
        &DayActivity::default(),
        Some(&with_goal()),
        &[],
      );

      assert!(result.is_complete());
    }

    #[test]
    fn it_ignores_blank_and_whitespace_goals() {
      let log = CaptainsLog {
        goal: Some("   ".to_owned()),
        ..CaptainsLog::default()
      };

      let result = completeness(&PromptConfig::default(), &DayActivity::default(), Some(&log), &[]);

      assert_eq!(result.missing_prompts, vec![AnswerKey::Goal]);
    }

    #[test]
    fn it_flags_a_required_custom_question_left_blank() {
      let mut config = PromptConfig::default();
      require_custom(&mut config, "mood", "Daily mood");

      let result = completeness(&config, &DayActivity::default(), Some(&with_goal()), &[]);

      assert_eq!(result.missing_custom, vec!["Daily mood".to_owned()]);
      assert!(!result.is_complete());
    }

    #[test]
    fn it_clears_a_custom_question_once_answered() {
      let mut config = PromptConfig::default();
      require_custom(&mut config, "mood", "Daily mood");
      let mut log = with_goal();
      log.answers.insert("mood".to_owned(), "focused".to_owned());

      let result = completeness(&config, &DayActivity::default(), Some(&log), &[]);

      assert!(result.is_complete());
    }

    #[test]
    fn it_flags_a_loss_without_a_debrief() {
      let activity = DayActivity {
        engagement_count: 1,
        losses: vec![loss(4, 100)],
        ..DayActivity::default()
      };

      let result = completeness(&PromptConfig::default(), &activity, Some(&with_goal()), &[]);

      assert_eq!(result.missing_debriefs, vec![loss(4, 100)]);
      assert!(!result.is_complete());
    }

    #[test]
    fn it_clears_a_loss_once_its_debrief_exists() {
      let activity = DayActivity {
        engagement_count: 1,
        losses: vec![loss(4, 100)],
        ..DayActivity::default()
      };

      let result = completeness(
        &PromptConfig::default(),
        &activity,
        Some(&with_goal()),
        &[report(4, 100)],
      );

      assert!(result.missing_debriefs.is_empty());
      assert!(result.is_complete());
    }

    #[test]
    fn it_does_not_flag_a_loss_when_the_combat_trigger_is_off() {
      let mut config = PromptConfig::default();
      if let Some(section) = config
        .sections
        .iter_mut()
        .find(|section| section.kind == PromptSectionKind::Conditional)
      {
        section.triggers = Some(PromptTriggers {
          build: true,
          combat: false,
          skill: true,
        });
      }
      let activity = DayActivity {
        engagement_count: 1,
        losses: vec![loss(4, 100)],
        ..DayActivity::default()
      };

      let result = completeness(&config, &activity, Some(&with_goal()), &[]);

      assert!(result.is_complete());
    }

    #[test]
    fn it_does_not_require_a_debrief_for_a_kills_only_day() {
      let activity = DayActivity {
        engagement_count: 2,
        ..DayActivity::default()
      };

      let result = completeness(&PromptConfig::default(), &activity, Some(&with_goal()), &[]);

      assert!(result.is_complete());
    }

    #[test]
    fn it_does_not_flag_optional_conditional_answers() {
      let activity = DayActivity {
        industry_count: 1,
        skill_count: 1,
        ..DayActivity::default()
      };

      let result = completeness(&PromptConfig::default(), &activity, Some(&with_goal()), &[]);

      assert!(result.missing_prompts.is_empty());
      assert!(result.is_complete());
    }
  }
}
