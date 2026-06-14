#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SlotGroup {
  CargoHold,
  DroneBay,
  High,
  Implant,
  Low,
  Medium,
  Other,
  Rig,
  Subsystem,
}

impl SlotGroup {
  // Render order matching the killmail modal's fitting grouping (high → med → low → rig →
  // subsystem → drone bay → cargo hold → implant), with the Other fallback bucket trailing.
  const DISPLAY_ORDER: [SlotGroup; 9] = [
    SlotGroup::High,
    SlotGroup::Medium,
    SlotGroup::Low,
    SlotGroup::Rig,
    SlotGroup::Subsystem,
    SlotGroup::DroneBay,
    SlotGroup::CargoHold,
    SlotGroup::Implant,
    SlotGroup::Other,
  ];

  pub fn display_order() -> &'static [SlotGroup] {
    &Self::DISPLAY_ORDER
  }

  // Maps an EVE inventory `flag` integer (the killmail item's `flag`, from the `invFlags` table)
  // to its fitting-slot group. Ranges below are the contiguous `invFlags` blocks:
  //   Cargo          = 5
  //   LoSlot0..7     = 11..=18
  //   MedSlot0..7    = 19..=26
  //   HiSlot0..7     = 27..=34
  //   DroneBay       = 87
  //   Implant        = 89
  //   RigSlot0..7    = 92..=99
  //   SubSystemSlot0..7 = 125..=132
  // Anything outside these blocks (hangar, fighter bays, structure flags, etc.) is `Other`.
  pub fn from_flag(flag: i64) -> SlotGroup {
    match flag {
      5 => SlotGroup::CargoHold,
      11..=18 => SlotGroup::Low,
      19..=26 => SlotGroup::Medium,
      27..=34 => SlotGroup::High,
      87 => SlotGroup::DroneBay,
      89 => SlotGroup::Implant,
      92..=99 => SlotGroup::Rig,
      125..=132 => SlotGroup::Subsystem,
      _ => SlotGroup::Other,
    }
  }

  pub fn label(&self) -> &'static str {
    match self {
      SlotGroup::CargoHold => "Cargo hold",
      SlotGroup::DroneBay => "Drone bay",
      SlotGroup::High => "High power",
      SlotGroup::Implant => "Implants",
      SlotGroup::Low => "Low power",
      SlotGroup::Medium => "Medium power",
      SlotGroup::Other => "Other",
      SlotGroup::Rig => "Rigs",
      SlotGroup::Subsystem => "Subsystems",
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod slot_group {
    use super::*;

    mod display_order {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_lists_every_group_once() {
        let order = SlotGroup::display_order();

        let mut unique: Vec<SlotGroup> = order.to_vec();
        unique.sort_by_key(|group| group.label());
        unique.dedup();

        assert_eq!(order.len(), 9);
        assert_eq!(unique.len(), 9);
      }

      #[test]
      fn it_follows_the_modal_fitting_grouping_with_other_last() {
        let order = SlotGroup::display_order();

        assert_eq!(
          order,
          [
            SlotGroup::High,
            SlotGroup::Medium,
            SlotGroup::Low,
            SlotGroup::Rig,
            SlotGroup::Subsystem,
            SlotGroup::DroneBay,
            SlotGroup::CargoHold,
            SlotGroup::Implant,
            SlotGroup::Other,
          ]
        );
      }
    }

    mod from_flag {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_maps_a_representative_flag_from_each_group() {
        assert_eq!(SlotGroup::from_flag(5), SlotGroup::CargoHold);
        assert_eq!(SlotGroup::from_flag(14), SlotGroup::Low);
        assert_eq!(SlotGroup::from_flag(22), SlotGroup::Medium);
        assert_eq!(SlotGroup::from_flag(30), SlotGroup::High);
        assert_eq!(SlotGroup::from_flag(87), SlotGroup::DroneBay);
        assert_eq!(SlotGroup::from_flag(89), SlotGroup::Implant);
        assert_eq!(SlotGroup::from_flag(95), SlotGroup::Rig);
        assert_eq!(SlotGroup::from_flag(128), SlotGroup::Subsystem);
      }

      #[test]
      fn it_maps_the_low_slot_boundary_flags() {
        assert_eq!(SlotGroup::from_flag(11), SlotGroup::Low);
        assert_eq!(SlotGroup::from_flag(18), SlotGroup::Low);
      }

      #[test]
      fn it_maps_the_medium_slot_boundary_flags() {
        assert_eq!(SlotGroup::from_flag(19), SlotGroup::Medium);
        assert_eq!(SlotGroup::from_flag(26), SlotGroup::Medium);
      }

      #[test]
      fn it_maps_the_high_slot_boundary_flags() {
        assert_eq!(SlotGroup::from_flag(27), SlotGroup::High);
        assert_eq!(SlotGroup::from_flag(34), SlotGroup::High);
      }

      #[test]
      fn it_maps_the_rig_slot_boundary_flags() {
        assert_eq!(SlotGroup::from_flag(92), SlotGroup::Rig);
        assert_eq!(SlotGroup::from_flag(99), SlotGroup::Rig);
      }

      #[test]
      fn it_maps_the_subsystem_slot_boundary_flags() {
        assert_eq!(SlotGroup::from_flag(125), SlotGroup::Subsystem);
        assert_eq!(SlotGroup::from_flag(132), SlotGroup::Subsystem);
      }

      #[test]
      fn it_falls_back_to_other_below_the_first_range() {
        assert_eq!(SlotGroup::from_flag(4), SlotGroup::Other);
        assert_eq!(SlotGroup::from_flag(10), SlotGroup::Other);
      }

      #[test]
      fn it_falls_back_to_other_in_the_gaps_between_ranges() {
        assert_eq!(SlotGroup::from_flag(0), SlotGroup::Other);
        assert_eq!(SlotGroup::from_flag(88), SlotGroup::Other);
        assert_eq!(SlotGroup::from_flag(133), SlotGroup::Other);
        assert_eq!(SlotGroup::from_flag(-1), SlotGroup::Other);
      }
    }

    mod label {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_a_human_label_per_group() {
        assert_eq!(SlotGroup::CargoHold.label(), "Cargo hold");
        assert_eq!(SlotGroup::DroneBay.label(), "Drone bay");
        assert_eq!(SlotGroup::High.label(), "High power");
        assert_eq!(SlotGroup::Implant.label(), "Implants");
        assert_eq!(SlotGroup::Low.label(), "Low power");
        assert_eq!(SlotGroup::Medium.label(), "Medium power");
        assert_eq!(SlotGroup::Other.label(), "Other");
        assert_eq!(SlotGroup::Rig.label(), "Rigs");
        assert_eq!(SlotGroup::Subsystem.label(), "Subsystems");
      }
    }
  }
}
