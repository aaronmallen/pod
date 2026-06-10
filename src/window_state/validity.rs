use super::WindowGeometry;

pub const MIN_ON_SCREEN_MARGIN: f32 = 24.0;

const MAX_COORD: f32 = 16384.0;

const MIN_COORD: f32 = 0.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
  pub height: f32,
  pub width: f32,
  pub x: f32,
  pub y: f32,
}

impl Rect {
  fn overlap_1d(a: f32, a_len: f32, b: f32, b_len: f32) -> f32 {
    let start = a.max(b);
    let end = (a + a_len).min(b + b_len);
    (end - start).max(0.0)
  }

  fn overlaps_by(&self, other: &Rect, margin: f32) -> bool {
    Self::overlap_1d(self.x, self.width, other.x, other.width) >= margin
      && Self::overlap_1d(self.y, self.height, other.y, other.height) >= margin
  }
}

impl From<&WindowGeometry> for Rect {
  fn from(geometry: &WindowGeometry) -> Self {
    Rect {
      height: geometry.height,
      width: geometry.width,
      x: geometry.x,
      y: geometry.y,
    }
  }
}

pub fn is_in_range(geometry: &WindowGeometry) -> bool {
  coord_in_range(geometry.x) && coord_in_range(geometry.y)
}

pub fn is_size_in_range(geometry: &WindowGeometry) -> bool {
  dimension_in_range(geometry.width) && dimension_in_range(geometry.height)
}

pub fn is_position_valid(geometry: &WindowGeometry, monitors: &[Rect]) -> bool {
  if !coord_in_range(geometry.x) || !coord_in_range(geometry.y) {
    tracing::warn!(
      x = geometry.x,
      y = geometry.y,
      "discarding restored window position: coordinates out of range or non-finite"
    );
    return false;
  }

  let rect = Rect::from(geometry);
  let reachable = monitors
    .iter()
    .any(|monitor| rect.overlaps_by(monitor, MIN_ON_SCREEN_MARGIN));

  if !reachable {
    tracing::warn!(
      x = geometry.x,
      y = geometry.y,
      "discarding restored window position: no connected display admits the window on-screen"
    );
  }

  reachable
}

fn coord_in_range(v: f32) -> bool {
  v.is_finite() && (MIN_COORD..=MAX_COORD).contains(&v)
}

fn dimension_in_range(v: f32) -> bool {
  v.is_finite() && v > 0.0 && v <= MAX_COORD
}

#[cfg(test)]
mod tests {
  use super::*;

  fn primary() -> Rect {
    Rect {
      height: 1080.0,
      width: 1920.0,
      x: 0.0,
      y: 0.0,
    }
  }

  fn geometry(x: f32, y: f32) -> WindowGeometry {
    WindowGeometry {
      height: 800.0,
      width: 1200.0,
      x,
      y,
    }
  }

  mod is_position_valid {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_normal_coordinates() {
      assert_eq!(is_position_valid(&geometry(100.0, 200.0), &[primary()]), true);
    }

    #[test]
    fn it_accepts_zero_origin() {
      assert_eq!(is_position_valid(&geometry(0.0, 0.0), &[primary()]), true);
    }

    #[test]
    fn it_accepts_boundary_value() {
      let monitor = Rect {
        height: 1080.0,
        width: 1920.0,
        x: 16000.0,
        y: 16000.0,
      };
      assert_eq!(is_position_valid(&geometry(16384.0, 16384.0), &[monitor]), true);
    }

    #[test]
    fn it_rejects_negative_x() {
      assert_eq!(is_position_valid(&geometry(-9999.0, 100.0), &[primary()]), false);
    }

    #[test]
    fn it_rejects_negative_y() {
      assert_eq!(is_position_valid(&geometry(100.0, -1.0), &[primary()]), false);
    }

    #[test]
    fn it_rejects_x_above_limit() {
      assert_eq!(is_position_valid(&geometry(16385.0, 0.0), &[primary()]), false);
    }

    #[test]
    fn it_rejects_y_above_limit() {
      assert_eq!(is_position_valid(&geometry(0.0, 16385.0), &[primary()]), false);
    }

    #[test]
    fn it_rejects_nan_x() {
      assert_eq!(is_position_valid(&geometry(f32::NAN, 100.0), &[primary()]), false);
    }

    #[test]
    fn it_rejects_nan_y() {
      assert_eq!(is_position_valid(&geometry(100.0, f32::NAN), &[primary()]), false);
    }

    #[test]
    fn it_rejects_infinite_x() {
      assert_eq!(is_position_valid(&geometry(f32::INFINITY, 100.0), &[primary()]), false);
    }

    #[test]
    fn it_rejects_infinite_y() {
      assert_eq!(is_position_valid(&geometry(100.0, f32::INFINITY), &[primary()]), false);
    }

    #[test]
    fn it_accepts_a_rect_fully_on_one_monitor() {
      assert_eq!(is_position_valid(&geometry(100.0, 100.0), &[primary()]), true);
    }

    #[test]
    fn it_rejects_a_rect_on_a_now_absent_monitor_with_no_present_overlap() {
      assert_eq!(is_position_valid(&geometry(2000.0, 100.0), &[primary()]), false);
    }

    #[test]
    fn it_rejects_a_rect_overlapping_a_monitor_by_less_than_the_margin() {
      assert_eq!(is_position_valid(&geometry(1910.0, 100.0), &[primary()]), false);
    }

    #[test]
    fn it_accepts_a_rect_overlapping_a_monitor_by_exactly_the_margin() {
      let window = WindowGeometry {
        height: 800.0,
        width: 1200.0,
        x: 1920.0 - MIN_ON_SCREEN_MARGIN,
        y: 100.0,
      };
      assert_eq!(is_position_valid(&window, &[primary()]), true);
    }

    #[test]
    fn it_rejects_every_position_when_no_monitor_is_connected() {
      assert_eq!(is_position_valid(&geometry(100.0, 100.0), &[]), false);
    }

    #[test]
    fn it_accepts_a_rect_reachable_on_a_secondary_monitor() {
      let secondary = Rect {
        height: 1080.0,
        width: 1920.0,
        x: 1920.0,
        y: 0.0,
      };
      assert_eq!(
        is_position_valid(&geometry(2000.0, 100.0), &[primary(), secondary]),
        true
      );
    }

    #[test]
    fn it_requires_overlap_on_both_axes_not_just_one() {
      assert_eq!(is_position_valid(&geometry(100.0, 5000.0), &[primary()]), false);
    }
  }

  mod is_in_range {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_an_in_range_position_with_no_monitor_required() {
      assert_eq!(is_in_range(&geometry(100.0, 200.0)), true);
    }

    #[test]
    fn it_rejects_an_out_of_range_position() {
      assert_eq!(is_in_range(&geometry(-1.0, 200.0)), false);
      assert_eq!(is_in_range(&geometry(100.0, 16385.0)), false);
    }

    #[test]
    fn it_rejects_a_non_finite_position() {
      assert_eq!(is_in_range(&geometry(f32::NAN, 0.0)), false);
      assert_eq!(is_in_range(&geometry(0.0, f32::INFINITY)), false);
    }
  }

  mod is_size_in_range {
    use pretty_assertions::assert_eq;

    use super::*;

    fn sized(width: f32, height: f32) -> WindowGeometry {
      WindowGeometry {
        height,
        width,
        x: 0.0,
        y: 0.0,
      }
    }

    #[test]
    fn it_accepts_a_normal_size() {
      assert_eq!(is_size_in_range(&sized(1200.0, 800.0)), true);
    }

    #[test]
    fn it_rejects_a_zero_dimension() {
      assert_eq!(is_size_in_range(&sized(0.0, 800.0)), false);
      assert_eq!(is_size_in_range(&sized(1200.0, 0.0)), false);
    }

    #[test]
    fn it_rejects_a_negative_dimension() {
      assert_eq!(is_size_in_range(&sized(-1200.0, 800.0)), false);
    }

    #[test]
    fn it_rejects_a_non_finite_dimension() {
      assert_eq!(is_size_in_range(&sized(f32::NAN, 800.0)), false);
      assert_eq!(is_size_in_range(&sized(1200.0, f32::INFINITY)), false);
    }

    #[test]
    fn it_rejects_an_absurdly_large_dimension() {
      assert_eq!(is_size_in_range(&sized(16385.0, 800.0)), false);
    }
  }
}
