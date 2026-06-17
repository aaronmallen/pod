pub mod coalesce;
pub mod validity;

use std::{
  collections::BTreeMap,
  path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct UiState {
  #[serde(default)]
  pub panes: BTreeMap<String, f32>,
  #[serde(default)]
  pub windows: BTreeMap<String, WindowGeometry>,
}

impl UiState {
  pub fn host_width(&self, window_key: &str, default: f32) -> f32 {
    self
      .windows
      .get(window_key)
      .map(|geometry| geometry.width)
      .unwrap_or(default)
  }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct WindowGeometry {
  pub height: f32,
  pub width: f32,
  pub x: f32,
  pub y: f32,
}

#[derive(Deserialize)]
struct FlatGeometry {
  #[serde(default)]
  abyssals_filter_pane_width: Option<f32>,
  #[serde(default)]
  assets_sidebar_width: Option<f32>,
  height: f32,
  #[serde(default)]
  mail_folder_pane_width: Option<f32>,
  #[serde(default)]
  mail_message_list_width: Option<f32>,
  #[serde(default)]
  plan_picker_pane_width: Option<f32>,
  #[serde(default)]
  plan_summary_pane_width: Option<f32>,
  #[serde(default)]
  plan_window_height: Option<f32>,
  #[serde(default)]
  plan_window_width: Option<f32>,
  #[serde(default)]
  plan_window_x: Option<f32>,
  #[serde(default)]
  plan_window_y: Option<f32>,
  #[serde(default)]
  skills_left_pane_width: Option<f32>,
  #[serde(default)]
  wallet_right_rail_width: Option<f32>,
  width: f32,
  x: f32,
  y: f32,
}

impl FlatGeometry {
  fn into_keyed(self) -> UiState {
    let mut state = UiState::default();

    state.windows.insert(
      "main".to_owned(),
      WindowGeometry {
        height: self.height,
        width: self.width,
        x: self.x,
        y: self.y,
      },
    );

    if let (Some(width), Some(height), Some(x), Some(y)) = (
      self.plan_window_width,
      self.plan_window_height,
      self.plan_window_x,
      self.plan_window_y,
    ) {
      state.windows.insert(
        "skill_plan_editor".to_owned(),
        WindowGeometry {
          height,
          width,
          x,
          y,
        },
      );
    }

    for (key, value) in [
      ("assets.abyssals_filter", self.abyssals_filter_pane_width),
      ("assets.sidebar", self.assets_sidebar_width),
      ("mail.folder", self.mail_folder_pane_width),
      ("mail.message_list", self.mail_message_list_width),
      ("plan.picker", self.plan_picker_pane_width),
      ("plan.summary", self.plan_summary_pane_width),
      ("skills.left", self.skills_left_pane_width),
      ("wallet.right_rail", self.wallet_right_rail_width),
    ] {
      if let Some(value) = value {
        state.panes.insert(key.to_owned(), value);
      }
    }

    state
  }
}

pub fn load() -> UiState {
  state_path().map(|path| load_from(&path)).unwrap_or_default()
}

pub fn save(state: &UiState) {
  let Some(path) = state_path() else { return };
  save_to(&path, state);
}

pub fn state_path() -> Option<PathBuf> {
  dir_spec::state_home().map(|path| path.join("pod/window.json"))
}

/// Loads window state, first attempting to migrate a legacy flat-geometry file before falling back to the current keyed format.
fn load_from(path: &Path) -> UiState {
  let Ok(bytes) = std::fs::read(path) else {
    return UiState::default();
  };
  if let Ok(flat) = serde_json::from_slice::<FlatGeometry>(&bytes) {
    return flat.into_keyed();
  }
  serde_json::from_slice(&bytes).unwrap_or_default()
}

fn save_to(path: &Path, state: &UiState) {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).ok();
  }
  serde_json::to_vec_pretty(state)
    .ok()
    .and_then(|json| std::fs::write(path, json).ok());
}

#[cfg(test)]
mod tests {
  use super::*;

  fn sample_state() -> UiState {
    let mut state = UiState::default();
    state.windows.insert(
      "main".to_owned(),
      WindowGeometry {
        height: 800.0,
        width: 1200.0,
        x: 100.0,
        y: 200.0,
      },
    );
    state.panes.insert("skills.left".to_owned(), 240.0);
    state.panes.insert("wallet.right_rail".to_owned(), 320.0);
    state
  }

  mod generalizes_to_any_pane_key {
    use pretty_assertions::assert_eq;

    use super::*;

    const PANE_KEYS: [&str; 8] = [
      "skills.left",
      "mail.folder",
      "mail.message_list",
      "wallet.right_rail",
      "plan.picker",
      "plan.summary",
      "assets.sidebar",
      "assets.abyssals_filter",
    ];

    #[test]
    fn it_round_trips_a_synthetic_never_seen_pane_key() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("window.json");

      let mut state = UiState::default();
      state.panes.insert("future.synthetic_pane".to_owned(), 137.0);

      save_to(&path, &state);
      let loaded = load_from(&path);

      assert_eq!(loaded.panes.get("future.synthetic_pane"), Some(&137.0));
    }

    #[test]
    fn it_round_trips_every_pane_key_through_save_then_load() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("window.json");

      let mut state = UiState::default();
      for (index, key) in PANE_KEYS.iter().enumerate() {
        state.panes.insert((*key).to_owned(), 200.0 + index as f32);
      }

      save_to(&path, &state);
      let loaded = load_from(&path);

      assert_eq!(loaded, state);
      for (index, key) in PANE_KEYS.iter().enumerate() {
        assert_eq!(loaded.panes.get(*key), Some(&(200.0 + index as f32)));
      }
    }

    #[test]
    fn it_treats_a_not_yet_wired_mail_key_identically_to_an_in_src_key() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("window.json");

      let mut state = UiState::default();
      state.panes.insert("skills.left".to_owned(), 333.0);
      state.panes.insert("mail.folder".to_owned(), 333.0);

      save_to(&path, &state);
      let loaded = load_from(&path);

      assert_eq!(loaded.panes.get("mail.folder"), loaded.panes.get("skills.left"));
      assert_eq!(loaded.panes.get("mail.folder"), Some(&333.0));
    }
  }

  mod load_from {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_loads_an_already_keyed_file_unchanged_without_re_migrating() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("window.json");
      let original = sample_state();
      save_to(&path, &original);

      let state = load_from(&path);

      assert_eq!(state, original);
    }

    #[test]
    fn it_maps_each_known_flat_pane_width_to_its_namespaced_pane_key() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("window.json");
      std::fs::write(
        &path,
        br#"{
          "width":1200.0,"height":800.0,"x":0.0,"y":0.0,
          "skills_left_pane_width":240.0,
          "mail_folder_pane_width":180.0,
          "mail_message_list_width":300.0,
          "wallet_right_rail_width":320.0,
          "plan_picker_pane_width":280.0,
          "plan_summary_pane_width":260.0,
          "assets_sidebar_width":220.0,
          "abyssals_filter_pane_width":160.0
        }"#,
      )
      .unwrap();

      let state = load_from(&path);

      assert_eq!(state.panes.get("skills.left"), Some(&240.0));
      assert_eq!(state.panes.get("mail.folder"), Some(&180.0));
      assert_eq!(state.panes.get("mail.message_list"), Some(&300.0));
      assert_eq!(state.panes.get("wallet.right_rail"), Some(&320.0));
      assert_eq!(state.panes.get("plan.picker"), Some(&280.0));
      assert_eq!(state.panes.get("plan.summary"), Some(&260.0));
      assert_eq!(state.panes.get("assets.sidebar"), Some(&220.0));
      assert_eq!(state.panes.get("assets.abyssals_filter"), Some(&160.0));
    }

    #[test]
    fn it_migrates_a_flat_files_plan_window_geometry_to_the_editor_window() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("window.json");
      std::fs::write(
        &path,
        br#"{"width":1200.0,"height":800.0,"x":0.0,"y":0.0,"plan_window_width":900.0,"plan_window_height":600.0,"plan_window_x":50.0,"plan_window_y":60.0}"#,
      )
      .unwrap();

      let state = load_from(&path);

      assert_eq!(
        state.windows.get("skill_plan_editor"),
        Some(&WindowGeometry {
          height: 600.0,
          width: 900.0,
          x: 50.0,
          y: 60.0,
        })
      );
    }

    #[test]
    fn it_migrates_a_flat_prototype_files_top_level_geometry_to_the_main_window() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("window.json");
      std::fs::write(&path, br#"{"width":1200.0,"height":800.0,"x":100.0,"y":200.0}"#).unwrap();

      let state = load_from(&path);

      assert_eq!(
        state.windows.get("main"),
        Some(&WindowGeometry {
          height: 800.0,
          width: 1200.0,
          x: 100.0,
          y: 200.0,
        })
      );
    }

    #[test]
    fn it_omits_pane_keys_absent_from_the_flat_file() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("window.json");
      std::fs::write(
        &path,
        br#"{"width":1200.0,"height":800.0,"x":0.0,"y":0.0,"skills_left_pane_width":240.0}"#,
      )
      .unwrap();

      let state = load_from(&path);

      assert_eq!(state.panes.get("skills.left"), Some(&240.0));
      assert_eq!(state.panes.get("wallet.right_rail"), None);
    }

    #[test]
    fn it_omits_the_editor_window_when_the_flat_file_lacks_plan_geometry() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("window.json");
      std::fs::write(&path, br#"{"width":1200.0,"height":800.0,"x":0.0,"y":0.0}"#).unwrap();

      let state = load_from(&path);

      assert_eq!(state.windows.get("skill_plan_editor"), None);
    }

    #[test]
    fn it_returns_defaults_for_foreign_content() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("window.json");
      std::fs::write(&path, br#"{"unrelated":[1,2,3]}"#).unwrap();

      let state = load_from(&path);

      assert_eq!(state, UiState::default());
    }

    #[test]
    fn it_returns_defaults_when_the_file_is_absent() {
      let dir = tempfile::tempdir().unwrap();

      let state = load_from(&dir.path().join("window.json"));

      assert_eq!(state, UiState::default());
    }

    #[test]
    fn it_returns_defaults_when_the_file_is_unparseable() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("window.json");
      std::fs::write(&path, b"{ this is not valid json").unwrap();

      let state = load_from(&path);

      assert_eq!(state, UiState::default());
    }
  }

  mod save_to {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_preserves_arbitrary_window_and_pane_keys() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("window.json");
      let mut state = UiState::default();
      state.windows.insert(
        "skill_plan_editor".to_owned(),
        WindowGeometry {
          height: 600.0,
          width: 900.0,
          x: 0.0,
          y: 0.0,
        },
      );
      state.panes.insert("plan.picker".to_owned(), 280.0);

      save_to(&path, &state);
      let loaded = load_from(&path);

      assert_eq!(
        loaded.windows.get("skill_plan_editor"),
        state.windows.get("skill_plan_editor")
      );
      assert_eq!(loaded.panes.get("plan.picker"), Some(&280.0));
    }

    #[test]
    fn it_roundtrips_through_the_file() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("nested").join("window.json");
      let state = sample_state();

      save_to(&path, &state);

      assert_eq!(load_from(&path), state);
    }
  }
}
