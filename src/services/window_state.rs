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
