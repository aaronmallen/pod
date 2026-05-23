//! Persistence for main window geometry between sessions.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WindowGeometry {
  pub width: f32,
  pub height: f32,
  pub x: f32,
  pub y: f32,
  #[serde(default)]
  pub skills_left_pane_width: Option<f32>,
  #[serde(default)]
  pub mail_folder_pane_width: Option<f32>,
  #[serde(default)]
  pub mail_message_list_width: Option<f32>,
  #[serde(default)]
  pub wallet_right_rail_width: Option<f32>,
  #[serde(default)]
  pub plan_window_width: Option<f32>,
  #[serde(default)]
  pub plan_window_height: Option<f32>,
  #[serde(default)]
  pub plan_window_x: Option<f32>,
  #[serde(default)]
  pub plan_window_y: Option<f32>,
  #[serde(default)]
  pub plan_picker_pane_width: Option<f32>,
  #[serde(default)]
  pub plan_summary_pane_width: Option<f32>,
}

impl WindowGeometry {
  /// Returns `false` if x or y would place the window off-screen or is
  /// otherwise unusable (negative, greater than 16 384, NaN, or infinite).
  pub fn is_position_valid(&self) -> bool {
    let valid = |v: f32| v.is_finite() && (0.0..=16384.0).contains(&v);
    valid(self.x) && valid(self.y)
  }
}

pub fn load() -> Option<WindowGeometry> {
  let bytes = std::fs::read(state_path()?).ok()?;
  serde_json::from_slice(&bytes).ok()
}

pub fn save(geometry: &WindowGeometry) {
  let Some(path) = state_path() else { return };
  if let Some(parent) = path.parent() {
    // non-critical; window state loss is preferable to a crash on save
    std::fs::create_dir_all(parent).ok();
  }
  serde_json::to_vec(geometry)
    .ok()
    .and_then(|json| std::fs::write(path, json).ok());
}

fn state_path() -> Option<PathBuf> {
  dir_spec::state_home().map(|path| path.join("pod/window.json"))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn geometry(x: f32, y: f32) -> WindowGeometry {
    WindowGeometry {
      width: 1200.0,
      height: 800.0,
      x,
      y,
      skills_left_pane_width: None,
      mail_folder_pane_width: None,
      mail_message_list_width: None,
      wallet_right_rail_width: None,
      plan_window_width: None,
      plan_window_height: None,
      plan_window_x: None,
      plan_window_y: None,
      plan_picker_pane_width: None,
      plan_summary_pane_width: None,
    }
  }

  mod window_geometry {
    use super::*;

    mod is_position_valid {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_accepts_normal_coordinates() {
        assert_eq!(geometry(100.0, 200.0).is_position_valid(), true);
      }

      #[test]
      fn it_accepts_zero_origin() {
        assert_eq!(geometry(0.0, 0.0).is_position_valid(), true);
      }

      #[test]
      fn it_accepts_boundary_value() {
        assert_eq!(geometry(16384.0, 16384.0).is_position_valid(), true);
      }

      #[test]
      fn it_rejects_negative_x() {
        assert_eq!(geometry(-9999.0, 100.0).is_position_valid(), false);
      }

      #[test]
      fn it_rejects_negative_y() {
        assert_eq!(geometry(100.0, -1.0).is_position_valid(), false);
      }

      #[test]
      fn it_rejects_x_above_limit() {
        assert_eq!(geometry(16385.0, 0.0).is_position_valid(), false);
      }

      #[test]
      fn it_rejects_y_above_limit() {
        assert_eq!(geometry(0.0, 16385.0).is_position_valid(), false);
      }

      #[test]
      fn it_rejects_nan_x() {
        assert_eq!(geometry(f32::NAN, 100.0).is_position_valid(), false);
      }

      #[test]
      fn it_rejects_nan_y() {
        assert_eq!(geometry(100.0, f32::NAN).is_position_valid(), false);
      }

      #[test]
      fn it_rejects_infinite_x() {
        assert_eq!(geometry(f32::INFINITY, 100.0).is_position_valid(), false);
      }

      #[test]
      fn it_rejects_infinite_y() {
        assert_eq!(geometry(100.0, f32::INFINITY).is_position_valid(), false);
      }
    }
  }
}
