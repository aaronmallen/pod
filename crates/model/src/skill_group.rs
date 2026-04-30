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
    match self {
      AttrKey::Perception => 27,
      AttrKey::Willpower => 24,
      AttrKey::Intelligence => 21,
      AttrKey::Memory => 19,
      AttrKey::Charisma => 17,
    }
  }

  pub fn implant(self) -> i32 {
    match self {
      AttrKey::Perception => 5,
      AttrKey::Willpower => 5,
      AttrKey::Intelligence => 4,
      AttrKey::Memory => 4,
      AttrKey::Charisma => 3,
    }
  }

  pub fn short(self) -> &'static str {
    match self {
      AttrKey::Perception => "Per",
      AttrKey::Willpower => "Wil",
      AttrKey::Intelligence => "Int",
      AttrKey::Memory => "Mem",
      AttrKey::Charisma => "Cha",
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      AttrKey::Perception => "Perception",
      AttrKey::Willpower => "Willpower",
      AttrKey::Intelligence => "Intelligence",
      AttrKey::Memory => "Memory",
      AttrKey::Charisma => "Charisma",
    }
  }

  /// Convert from EVE dogma attribute value (164–168) to `AttrKey`.
  pub fn from_eve_id(id: u8) -> Self {
    match id {
      164 => AttrKey::Charisma,
      165 => AttrKey::Intelligence,
      166 => AttrKey::Memory,
      167 => AttrKey::Perception,
      168 => AttrKey::Willpower,
      _ => AttrKey::Perception,
    }
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
