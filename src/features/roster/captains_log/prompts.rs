use crate::store::{
  Database, Error,
  model::{CaptainsLog, KillmailReport},
  repo::{captains_log, captains_log::AnswerKey, killmail_report},
};

/// Order is the display order shown to the player, grouped by `PromptGroup` but not alphabetical within it.
#[allow(dead_code)]
pub const CATALOG: [Prompt; 8] = [
  Prompt {
    group: PromptGroup::Core,
    key: AnswerKey::Goal,
    required: true,
    trigger: None,
  },
  Prompt {
    group: PromptGroup::Core,
    key: AnswerKey::Remember,
    required: false,
    trigger: None,
  },
  Prompt {
    group: PromptGroup::Core,
    key: AnswerKey::Blocked,
    required: false,
    trigger: None,
  },
  Prompt {
    group: PromptGroup::Conditional,
    key: AnswerKey::Combat,
    required: true,
    trigger: Some(Trigger::Engagement),
  },
  Prompt {
    group: PromptGroup::Conditional,
    key: AnswerKey::Build,
    required: false,
    trigger: Some(Trigger::Industry),
  },
  Prompt {
    group: PromptGroup::Conditional,
    key: AnswerKey::Skill,
    required: false,
    trigger: Some(Trigger::Skills),
  },
  Prompt {
    group: PromptGroup::Forward,
    key: AnswerKey::Next,
    required: false,
    trigger: None,
  },
  Prompt {
    group: PromptGroup::Forward,
    key: AnswerKey::Research,
    required: false,
    trigger: None,
  },
];

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Completeness {
  pub missing_debriefs: Vec<LossEngagement>,
  pub missing_prompts: Vec<AnswerKey>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DayActivity {
  pub engagement_count: u32,
  pub industry_count: u32,
  pub losses: Vec<LossEngagement>,
  pub skill_count: u32,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LossEngagement {
  pub character_id: i64,
  pub killmail_id: i64,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Prompt {
  pub group: PromptGroup,
  pub key: AnswerKey,
  pub required: bool,
  pub trigger: Option<Trigger>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptGroup {
  Conditional,
  Core,
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
    self.missing_debriefs.is_empty() && self.missing_prompts.is_empty()
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
pub fn completeness(activity: &DayActivity, log: Option<&CaptainsLog>, reports: &[KillmailReport]) -> Completeness {
  Completeness {
    missing_debriefs: missing_loss_debriefs(activity, reports),
    missing_prompts: missing_required_prompts(activity, log),
  }
}

#[allow(dead_code)]
pub async fn completeness_for_day(
  db: &Database,
  date: &str,
  character_ids: &[i64],
  activity: &DayActivity,
) -> Result<Completeness, Error> {
  let log = captains_log::get(db, date).await?;
  let reports = killmail_report::list_for_day(db, character_ids, date).await?;

  Ok(completeness(activity, log.as_ref(), &reports))
}

#[allow(dead_code)]
pub fn prompts_for_day(activity: &DayActivity) -> Vec<Prompt> {
  CATALOG.into_iter().filter(|prompt| prompt.applies(activity)).collect()
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

fn is_answered(log: Option<&CaptainsLog>, key: AnswerKey) -> bool {
  log
    .and_then(|log| answer_text(log, key))
    .is_some_and(|text| !text.trim().is_empty())
}

fn missing_loss_debriefs(activity: &DayActivity, reports: &[KillmailReport]) -> Vec<LossEngagement> {
  activity
    .losses
    .iter()
    .filter(|loss| !has_report(reports, loss))
    .copied()
    .collect()
}

fn missing_required_prompts(activity: &DayActivity, log: Option<&CaptainsLog>) -> Vec<AnswerKey> {
  CATALOG
    .iter()
    // Combat is marked `required` in the catalog to highlight it, but engagement-day completeness is
    // judged via loss debriefs (see `missing_loss_debriefs`), not this freeform answer, so it's excluded here.
    .filter(|prompt| prompt.required && prompt.key != AnswerKey::Combat)
    .filter(|prompt| prompt.applies(activity) && !is_answered(log, prompt.key))
    .map(|prompt| prompt.key)
    .collect()
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

    #[test]
    fn it_shows_only_core_and_forward_prompts_on_a_quiet_day() {
      let keys: Vec<AnswerKey> = prompts_for_day(&DayActivity::default())
        .iter()
        .map(|prompt| prompt.key)
        .collect();

      assert_eq!(
        keys,
        vec![
          AnswerKey::Goal,
          AnswerKey::Remember,
          AnswerKey::Blocked,
          AnswerKey::Next,
          AnswerKey::Research,
        ]
      );
    }

    #[test]
    fn it_adds_the_combat_prompt_when_any_engagement_happened() {
      let activity = DayActivity {
        engagement_count: 2,
        ..DayActivity::default()
      };

      let keys: Vec<AnswerKey> = prompts_for_day(&activity).iter().map(|prompt| prompt.key).collect();

      assert!(keys.contains(&AnswerKey::Combat));
      assert!(!keys.contains(&AnswerKey::Build));
      assert!(!keys.contains(&AnswerKey::Skill));
    }

    #[test]
    fn it_adds_every_conditional_prompt_on_a_busy_day() {
      let activity = DayActivity {
        engagement_count: 3,
        industry_count: 1,
        losses: vec![loss(4, 100)],
        skill_count: 2,
      };

      let keys: Vec<AnswerKey> = prompts_for_day(&activity).iter().map(|prompt| prompt.key).collect();

      assert_eq!(
        keys,
        vec![
          AnswerKey::Goal,
          AnswerKey::Remember,
          AnswerKey::Blocked,
          AnswerKey::Combat,
          AnswerKey::Build,
          AnswerKey::Skill,
          AnswerKey::Next,
          AnswerKey::Research,
        ]
      );
    }
  }

  mod completeness {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_flags_a_missing_goal_on_a_quiet_day() {
      let result = completeness(&DayActivity::default(), None, &[]);

      assert_eq!(result.missing_prompts, vec![AnswerKey::Goal]);
      assert!(result.missing_debriefs.is_empty());
      assert!(!result.is_complete());
    }

    #[test]
    fn it_reports_a_quiet_day_with_a_goal_as_complete() {
      let log = with_goal();

      let result = completeness(&DayActivity::default(), Some(&log), &[]);

      assert!(result.is_complete());
    }

    #[test]
    fn it_ignores_blank_and_whitespace_goals() {
      let log = CaptainsLog {
        goal: Some("   ".to_owned()),
        ..CaptainsLog::default()
      };

      let result = completeness(&DayActivity::default(), Some(&log), &[]);

      assert_eq!(result.missing_prompts, vec![AnswerKey::Goal]);
    }

    #[test]
    fn it_flags_a_loss_without_a_debrief() {
      let log = with_goal();
      let activity = DayActivity {
        engagement_count: 1,
        losses: vec![loss(4, 100)],
        ..DayActivity::default()
      };

      let result = completeness(&activity, Some(&log), &[]);

      assert_eq!(result.missing_debriefs, vec![loss(4, 100)]);
      assert!(!result.is_complete());
    }

    #[test]
    fn it_clears_a_loss_once_its_debrief_exists() {
      let log = with_goal();
      let activity = DayActivity {
        engagement_count: 1,
        losses: vec![loss(4, 100)],
        ..DayActivity::default()
      };

      let result = completeness(&activity, Some(&log), &[report(4, 100)]);

      assert!(result.missing_debriefs.is_empty());
      assert!(result.is_complete());
    }

    #[test]
    fn it_does_not_require_a_debrief_for_a_kills_only_day() {
      let log = with_goal();
      let activity = DayActivity {
        engagement_count: 2,
        ..DayActivity::default()
      };

      let result = completeness(&activity, Some(&log), &[]);

      assert!(result.is_complete());
    }

    #[test]
    fn it_does_not_flag_optional_conditional_answers() {
      let log = with_goal();
      let activity = DayActivity {
        industry_count: 1,
        skill_count: 1,
        ..DayActivity::default()
      };

      let result = completeness(&activity, Some(&log), &[]);

      assert!(result.missing_prompts.is_empty());
      assert!(result.is_complete());
    }
  }
}
