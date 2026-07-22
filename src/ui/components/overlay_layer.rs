pub enum OverlayLayer {
  Dropdown,
  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "wired into the app.rs overlay layers by a later task")
  )]
  Modal,
  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "wired into the app.rs overlay layers by a later task")
  )]
  Notifications,
  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "wired into the app.rs overlay layers by a later task")
  )]
  Palette,
  RailCascade,
}

impl OverlayLayer {
  pub fn z(self) -> f32 {
    match self {
      OverlayLayer::Dropdown => 10.0,
      OverlayLayer::RailCascade => 20.0,
      OverlayLayer::Palette => 30.0,
      OverlayLayer::Notifications => 40.0,
      OverlayLayer::Modal => 50.0,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod z {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_a_strictly_increasing_back_to_front_scale() {
      let scale = [
        OverlayLayer::Dropdown.z(),
        OverlayLayer::RailCascade.z(),
        OverlayLayer::Palette.z(),
        OverlayLayer::Notifications.z(),
        OverlayLayer::Modal.z(),
      ];

      assert!(scale.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn it_places_the_rail_cascade_above_the_content_dropdown() {
      assert!(OverlayLayer::RailCascade.z() > OverlayLayer::Dropdown.z());
    }

    #[test]
    fn it_maps_each_layer_to_its_locked_value() {
      assert_eq!(OverlayLayer::Dropdown.z(), 10.0);
      assert_eq!(OverlayLayer::RailCascade.z(), 20.0);
      assert_eq!(OverlayLayer::Palette.z(), 30.0);
      assert_eq!(OverlayLayer::Notifications.z(), 40.0);
      assert_eq!(OverlayLayer::Modal.z(), 50.0);
    }
  }
}
