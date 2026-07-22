use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, FromRow, PartialEq)]
pub struct Model {
  pub alerted: bool,
  pub character_id: i64,
  pub created_at: String,
  pub kind: String,
  pub marker: String,
  pub subject_id: i64,
  pub updated_at: String,
}

#[allow(dead_code)]
impl Model {
  pub fn dedup_key(&self) -> String {
    format!(
      "{}:{}:{}:{}",
      self.kind, self.character_id, self.subject_id, self.marker
    )
  }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
  #[default]
  Outbid,
  Target,
}

#[allow(dead_code)]
impl Kind {
  pub const ALL: [Kind; 2] = [Kind::Outbid, Kind::Target];

  pub fn as_str(self) -> &'static str {
    match self {
      Kind::Outbid => "outbid",
      Kind::Target => "target",
    }
  }

  pub fn parse(value: &str) -> Option<Kind> {
    match value {
      "outbid" => Some(Kind::Outbid),
      "target" => Some(Kind::Target),
      _ => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod kind {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_every_variant_through_its_wire_string() {
      for kind in Kind::ALL {
        assert_eq!(Kind::parse(kind.as_str()), Some(kind));
      }
    }

    #[test]
    fn it_rejects_an_unknown_kind() {
      assert_eq!(Kind::parse("bid"), None);
      assert_eq!(Kind::parse("Outbid"), None);
      assert_eq!(Kind::parse(""), None);
    }
  }

  mod dedup_key {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_folds_the_marker_into_the_key() {
      let state = Model {
        alerted: true,
        character_id: 90_000_001,
        created_at: "2026-07-13T00:00:00+00:00".to_owned(),
        kind: "outbid".to_owned(),
        marker: "4999.0".to_owned(),
        subject_id: 6_001_002_003,
        updated_at: "2026-07-13T00:00:00+00:00".to_owned(),
      };

      assert_eq!(state.dedup_key(), "outbid:90000001:6001002003:4999.0");
    }

    #[test]
    fn it_changes_when_the_marker_changes() {
      let mut state = Model {
        alerted: true,
        character_id: 42,
        created_at: String::new(),
        kind: "target".to_owned(),
        marker: "a".to_owned(),
        subject_id: 7,
        updated_at: String::new(),
      };
      let first = state.dedup_key();
      state.marker = "b".to_owned();

      assert_ne!(first, state.dedup_key());
    }
  }
}
