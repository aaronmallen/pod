use serde::{Deserialize, Serialize};
use sqlx::FromRow;

pub const SOURCE_LOG_ANSWER: &str = "log_answer";
pub const SOURCE_FIELD_NOTE: &str = "field_note";
pub const SOURCE_KILLMAIL: &str = "killmail";
pub const SOURCE_INDUSTRY: &str = "industry";
pub const SOURCE_SKILL: &str = "skill";

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, FromRow, PartialEq)]
pub struct Model {
  pub accent: String,
  pub cancelled_at: Option<String>,
  pub completed_at: Option<String>,
  pub created_at: String,
  pub horizon: Option<String>,
  pub id: i64,
  pub status: String,
  pub target: Option<String>,
  pub title: String,
  pub why: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NewObjective {
  pub accent: String,
  pub horizon: Option<String>,
  pub target: Option<String>,
  pub title: String,
  pub why: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
  #[default]
  Active,
  Cancelled,
  Complete,
}

#[allow(dead_code)]
impl Status {
  pub const ALL: [Status; 3] = [Status::Active, Status::Complete, Status::Cancelled];

  pub fn as_str(self) -> &'static str {
    match self {
      Status::Active => "active",
      Status::Cancelled => "cancelled",
      Status::Complete => "complete",
    }
  }

  pub fn parse(value: &str) -> Option<Status> {
    match value {
      "active" => Some(Status::Active),
      "cancelled" => Some(Status::Cancelled),
      "complete" => Some(Status::Complete),
      _ => None,
    }
  }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, Eq, FromRow, PartialEq)]
pub struct Pilot {
  pub character_id: i64,
  pub objective_id: i64,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, FromRow, PartialEq)]
pub struct Link {
  pub date: String,
  pub objective_id: i64,
  pub source_kind: String,
  pub source_ref: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, FromRow, PartialEq)]
pub struct ThreadEntry {
  pub date: String,
  pub source_kind: String,
  pub source_ref: String,
  pub text: Option<String>,
  pub character: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinkSource {
  FieldNote { note_id: i64 },
  Industry { character_id: i64, product_type_id: i64 },
  Killmail { character_id: i64, killmail_id: i64 },
  LogAnswer { question_id: String },
  Skill { character_id: i64, skill_id: i64 },
}

#[allow(dead_code)]
impl LinkSource {
  pub fn source_kind(&self) -> &'static str {
    match self {
      LinkSource::FieldNote {
        ..
      } => SOURCE_FIELD_NOTE,
      LinkSource::Industry {
        ..
      } => SOURCE_INDUSTRY,
      LinkSource::Killmail {
        ..
      } => SOURCE_KILLMAIL,
      LinkSource::LogAnswer {
        ..
      } => SOURCE_LOG_ANSWER,
      LinkSource::Skill {
        ..
      } => SOURCE_SKILL,
    }
  }

  pub fn source_ref(&self) -> String {
    match self {
      LinkSource::FieldNote {
        note_id,
      } => note_id.to_string(),
      LinkSource::Industry {
        character_id,
        product_type_id,
      } => format!("{character_id}:{product_type_id}"),
      LinkSource::Killmail {
        character_id,
        killmail_id,
      } => format!("{character_id}:{killmail_id}"),
      LinkSource::LogAnswer {
        question_id,
      } => question_id.clone(),
      LinkSource::Skill {
        character_id,
        skill_id,
      } => format!("{character_id}:{skill_id}"),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod status {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_every_variant_through_its_wire_string() {
      for status in Status::ALL {
        assert_eq!(Status::parse(status.as_str()), Some(status));
      }
    }

    #[test]
    fn it_rejects_an_unknown_status() {
      assert_eq!(Status::parse("archived"), None);
      assert_eq!(Status::parse("Active"), None);
      assert_eq!(Status::parse(""), None);
    }
  }

  mod link_source {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_encodes_a_log_answer_as_the_question_id() {
      let source = LinkSource::LogAnswer {
        question_id: "goal".to_owned(),
      };

      assert_eq!(source.source_kind(), "log_answer");
      assert_eq!(source.source_ref(), "goal");
    }

    #[test]
    fn it_encodes_a_field_note_as_its_row_id() {
      let source = LinkSource::FieldNote {
        note_id: 42,
      };

      assert_eq!(source.source_kind(), "field_note");
      assert_eq!(source.source_ref(), "42");
    }

    #[test]
    fn it_encodes_a_killmail_as_character_and_killmail_id() {
      let source = LinkSource::Killmail {
        character_id: 90_000_001,
        killmail_id: 501,
      };

      assert_eq!(source.source_kind(), "killmail");
      assert_eq!(source.source_ref(), "90000001:501");
    }

    #[test]
    fn it_encodes_industry_as_character_and_product_type() {
      let source = LinkSource::Industry {
        character_id: 90_000_001,
        product_type_id: 22_544,
      };

      assert_eq!(source.source_kind(), "industry");
      assert_eq!(source.source_ref(), "90000001:22544");
    }

    #[test]
    fn it_encodes_a_skill_as_character_and_skill_id() {
      let source = LinkSource::Skill {
        character_id: 90_000_001,
        skill_id: 3300,
      };

      assert_eq!(source.source_kind(), "skill");
      assert_eq!(source.source_ref(), "90000001:3300");
    }
  }
}
