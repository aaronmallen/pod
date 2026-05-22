//! Domain model for EVE Online certificates used in ship mastery tiers.

/// A single EVE Online certificate, defining required skills for a mastery tier.
#[derive(Clone, Debug)]
pub struct Certificate {
  pub id: i32,
  pub name: String,
  pub description: Option<String>,
  pub grade: u8,
  /// `(type_id, [basic, improved, advanced, elite])` — required skill level at each proficiency tier.
  pub skills: Vec<(i32, [u8; 4])>,
}

#[cfg(test)]
mod tests {
  use super::*;

  mod certificate {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_stores_fields() {
      let cert = Certificate {
        id: 42,
        name: "Caldari Frigate Expert".into(),
        description: Some("Mastery in frigates".into()),
        grade: 3,
        skills: vec![(3300, [1, 2, 3, 4])],
      };

      assert_eq!(cert.id, 42);
      assert_eq!(cert.name, "Caldari Frigate Expert");
      assert_eq!(cert.grade, 3);
      assert_eq!(cert.skills.len(), 1);
      assert_eq!(cert.skills[0], (3300, [1, 2, 3, 4]));
    }

    #[test]
    fn it_accepts_empty_skills() {
      let cert = Certificate {
        id: 1,
        name: "Empty".into(),
        description: None,
        grade: 0,
        skills: vec![],
      };

      assert_eq!(cert.skills.len(), 0);
      assert!(cert.description.is_none());
    }
  }
}
