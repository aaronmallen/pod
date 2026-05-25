//! Skill group and definition types loaded from the EVE SDE.

/// One of the five EVE neural training attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttrKey {
  Perception,
  Willpower,
  Intelligence,
  Memory,
  Charisma,
}

impl AttrKey {
  pub const ALL: [AttrKey; 5] = [
    AttrKey::Perception,
    AttrKey::Willpower,
    AttrKey::Intelligence,
    AttrKey::Memory,
    AttrKey::Charisma,
  ];

  pub fn value(self) -> u32 {
    self.attr_tuple().1
  }

  pub fn implant(self) -> i32 {
    self.attr_tuple().2
  }

  pub fn short(self) -> &'static str {
    self.attr_tuple().3
  }

  pub fn label(self) -> &'static str {
    self.attr_tuple().4
  }

  fn attr_tuple(self) -> (u8, u32, i32, &'static str, &'static str) {
    match self {
      AttrKey::Perception => (167, 27, 5, "Per", "Perception"),
      AttrKey::Willpower => (168, 24, 5, "Wil", "Willpower"),
      AttrKey::Intelligence => (165, 21, 4, "Int", "Intelligence"),
      AttrKey::Memory => (166, 19, 4, "Mem", "Memory"),
      AttrKey::Charisma => (164, 17, 3, "Cha", "Charisma"),
    }
  }

  /// Convert from EVE dogma attribute value (164–168) to `AttrKey`.
  pub fn from_eve_id(id: u8) -> Self {
    AttrKey::ALL
      .iter()
      .find(|k| k.attr_tuple().0 == id)
      .copied()
      .unwrap_or(AttrKey::Perception)
  }
}

/// One skill definition loaded from the EVE SDE.
#[derive(Clone, Debug)]
pub struct SkillDef {
  pub type_id: i32,
  pub name: String,
  pub rank: u8,
  /// Character's trained level for this skill (0 = untrained).
  pub level: u8,
  /// SP already invested in the current partial level.
  pub sp: u64,
  pub primary: AttrKey,
  pub secondary: AttrKey,
  /// Direct prerequisites as `(skill_name, required_level)` pairs.
  pub prereqs: Vec<(String, u8)>,
}

/// A named group of skills.
#[derive(Clone, Debug)]
pub struct SkillGroupDef {
  pub id: String,
  pub name: String,
  pub skills: Vec<SkillDef>,
}

#[cfg(test)]
mod tests {
  use super::*;

  mod attr_key {
    use super::*;

    mod from_eve_id {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_charisma_for_164() {
        assert_eq!(AttrKey::from_eve_id(164), AttrKey::Charisma);
      }

      #[test]
      fn it_returns_intelligence_for_165() {
        assert_eq!(AttrKey::from_eve_id(165), AttrKey::Intelligence);
      }

      #[test]
      fn it_returns_memory_for_166() {
        assert_eq!(AttrKey::from_eve_id(166), AttrKey::Memory);
      }

      #[test]
      fn it_returns_perception_for_167() {
        assert_eq!(AttrKey::from_eve_id(167), AttrKey::Perception);
      }

      #[test]
      fn it_returns_perception_for_unknown_id() {
        assert_eq!(AttrKey::from_eve_id(0), AttrKey::Perception);
      }

      #[test]
      fn it_returns_willpower_for_168() {
        assert_eq!(AttrKey::from_eve_id(168), AttrKey::Willpower);
      }
    }

    mod implant {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_correct_implant_bonus_for_each_attr() {
        assert_eq!(AttrKey::Charisma.implant(), 3);
        assert_eq!(AttrKey::Intelligence.implant(), 4);
        assert_eq!(AttrKey::Memory.implant(), 4);
        assert_eq!(AttrKey::Perception.implant(), 5);
        assert_eq!(AttrKey::Willpower.implant(), 5);
      }
    }

    mod label {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_display_label_for_each_attr() {
        assert_eq!(AttrKey::Charisma.label(), "Charisma");
        assert_eq!(AttrKey::Intelligence.label(), "Intelligence");
        assert_eq!(AttrKey::Memory.label(), "Memory");
        assert_eq!(AttrKey::Perception.label(), "Perception");
        assert_eq!(AttrKey::Willpower.label(), "Willpower");
      }
    }

    mod short {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_short_label_for_each_attr() {
        assert_eq!(AttrKey::Charisma.short(), "Cha");
        assert_eq!(AttrKey::Intelligence.short(), "Int");
        assert_eq!(AttrKey::Memory.short(), "Mem");
        assert_eq!(AttrKey::Perception.short(), "Per");
        assert_eq!(AttrKey::Willpower.short(), "Wil");
      }
    }

    mod value {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_base_value_for_each_attr() {
        assert_eq!(AttrKey::Charisma.value(), 17);
        assert_eq!(AttrKey::Intelligence.value(), 21);
        assert_eq!(AttrKey::Memory.value(), 19);
        assert_eq!(AttrKey::Perception.value(), 27);
        assert_eq!(AttrKey::Willpower.value(), 24);
      }
    }
  }
}
