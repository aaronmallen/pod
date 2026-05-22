//! Lightweight neural attribute value structs shared between the DB and UI
//! layers.

/// Five core neural attributes for a character or an implant bonus.
///
/// Used as the return type for both character effective attributes and active-
/// clone implant bonus calculations.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NeuralAttributes {
  /// Charisma attribute value.
  pub charisma: i32,
  /// Intelligence attribute value.
  pub intelligence: i32,
  /// Memory attribute value.
  pub memory: i32,
  /// Perception attribute value.
  pub perception: i32,
  /// Willpower attribute value.
  pub willpower: i32,
}

#[cfg(test)]
mod tests {
  mod default {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_zeroes_all_attributes() {
      let attrs = NeuralAttributes::default();

      assert_eq!(attrs.charisma, 0);
      assert_eq!(attrs.intelligence, 0);
      assert_eq!(attrs.memory, 0);
      assert_eq!(attrs.perception, 0);
      assert_eq!(attrs.willpower, 0);
    }
  }

  mod partial_eq {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_considers_identical_instances_equal() {
      let a = NeuralAttributes {
        charisma: 20,
        intelligence: 25,
        memory: 21,
        perception: 22,
        willpower: 22,
      };
      let b = NeuralAttributes {
        charisma: 20,
        intelligence: 25,
        memory: 21,
        perception: 22,
        willpower: 22,
      };

      assert_eq!(a, b);
    }

    #[test]
    fn it_considers_differing_instances_not_equal() {
      let a = NeuralAttributes {
        charisma: 20,
        ..NeuralAttributes::default()
      };
      let b = NeuralAttributes::default();

      assert_ne!(a, b);
    }
  }
}
