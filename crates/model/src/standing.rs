//! Domain model for character NPC and player standings.

use getset::Getters;

/// The source entity type for a standing relationship.
#[derive(Clone, Debug, PartialEq)]
pub enum FromType {
  /// A registered NPC agent.
  Agent,
  /// An NPC corporation.
  Corp,
  /// An NPC faction (e.g., Caldari State).
  Faction,
}

/// A standing entry toward an NPC or player entity.
#[derive(Clone, Debug, Getters, PartialEq)]
pub struct Standing {
  /// Effective standing after social skills are applied.
  #[get = "pub"]
  effective: f32,
  /// EVE entity ID of the standing source.
  #[get = "pub"]
  from_id: i32,
  /// Human-readable name of the standing source.
  #[get = "pub"]
  from_name: String,
  /// Category of the entity granting the standing.
  #[get = "pub"]
  from_type: FromType,
  /// Raw standing value before skill modifiers, in the range [-10.0, 10.0].
  #[get = "pub"]
  raw: f32,
}

impl Standing {
  /// Creates a new standing entry.
  pub fn new(from_id: i32, from_type: FromType, from_name: impl Into<String>, raw: f32, effective: f32) -> Self {
    Self {
      effective,
      from_id,
      from_name: from_name.into(),
      from_type,
      raw,
    }
  }

  /// Sets the effective standing value.
  pub fn set_effective(&mut self, effective: f32) -> &mut Self {
    self.effective = effective;
    self
  }

  /// Sets the source entity ID.
  pub fn set_from_id(&mut self, from_id: i32) -> &mut Self {
    self.from_id = from_id;
    self
  }

  /// Sets the source entity name.
  pub fn set_from_name(&mut self, from_name: impl Into<String>) -> &mut Self {
    self.from_name = from_name.into();
    self
  }

  /// Sets the source entity type.
  pub fn set_from_type(&mut self, from_type: FromType) -> &mut Self {
    self.from_type = from_type;
    self
  }

  /// Sets the raw standing value.
  pub fn set_raw(&mut self, raw: f32) -> &mut Self {
    self.raw = raw;
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod standing {
    use super::*;

    mod set_effective {
      use super::*;

      #[test]
      fn it_sets_the_effective_standing() {
        let mut s = Standing::new(3_008_413, FromType::Agent, "Yumi Kikuko", 0.5, 0.75);
        s.set_effective(1.0);
        assert_eq!(*s.effective(), 1.0_f32);
      }
    }

    mod set_from_type {
      use super::*;

      #[test]
      fn it_sets_the_from_type() {
        let mut s = Standing::new(3_008_413, FromType::Agent, "Yumi Kikuko", 0.5, 0.75);
        s.set_from_type(FromType::Corp);
        assert_eq!(*s.from_type(), FromType::Corp);
      }
    }
  }
}
