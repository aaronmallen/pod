use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{Folder, Scope, StandardFolder};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub(super) struct NavState {
  #[serde(default)]
  pub active: Scope,
  #[serde(default)]
  pub folder: Folder,
  #[serde(default)]
  pub selected: Option<i64>,
}

impl NavState {
  pub(super) fn capture(active: Scope, folder: Folder, selected: Option<i64>) -> Self {
    NavState {
      active,
      folder,
      selected,
    }
  }
}

pub(super) fn load() -> NavState {
  state_path().map(|path| load_from(&path)).unwrap_or_default()
}

pub(super) fn save(state: &NavState) {
  let Some(path) = state_path() else { return };
  save_to(&path, state);
}

fn load_from(path: &Path) -> NavState {
  let Ok(bytes) = std::fs::read(path) else {
    return NavState::default();
  };
  serde_json::from_slice(&bytes).unwrap_or_default()
}

fn save_to(path: &Path, state: &NavState) {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).ok();
  }
  serde_json::to_vec_pretty(state)
    .ok()
    .and_then(|json| std::fs::write(path, json).ok());
}

fn state_path() -> Option<PathBuf> {
  dir_spec::state_home().map(|path| path.join("pod/mail_nav.json"))
}

impl Serialize for Scope {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    PersistedScope::from(*self).serialize(serializer)
  }
}

impl<'de> Deserialize<'de> for Scope {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    PersistedScope::deserialize(deserializer).map(Scope::from)
  }
}

impl Serialize for Folder {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    PersistedFolder::from(*self).serialize(serializer)
  }
}

impl<'de> Deserialize<'de> for Folder {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    PersistedFolder::deserialize(deserializer).map(Folder::from)
  }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PersistedScope {
  AllInboxes,
  Character { id: i64 },
}

impl From<Scope> for PersistedScope {
  fn from(scope: Scope) -> Self {
    match scope {
      Scope::AllInboxes => PersistedScope::AllInboxes,
      Scope::Character(id) => PersistedScope::Character {
        id,
      },
    }
  }
}

impl From<PersistedScope> for Scope {
  fn from(scope: PersistedScope) -> Self {
    match scope {
      PersistedScope::AllInboxes => Scope::AllInboxes,
      PersistedScope::Character {
        id,
      } => Scope::Character(id),
    }
  }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PersistedFolder {
  Label { id: i64 },
  Standard { name: PersistedStandard },
  Unified,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum PersistedStandard {
  Archive,
  Drafts,
  Inbox,
  Sent,
  Snoozed,
  Starred,
  Trash,
}

impl From<Folder> for PersistedFolder {
  fn from(folder: Folder) -> Self {
    match folder {
      Folder::Unified => PersistedFolder::Unified,
      Folder::Label(id) => PersistedFolder::Label {
        id,
      },
      Folder::Standard(standard_folder) => PersistedFolder::Standard {
        name: standard_folder.into(),
      },
    }
  }
}

impl From<PersistedFolder> for Folder {
  fn from(folder: PersistedFolder) -> Self {
    match folder {
      PersistedFolder::Unified => Folder::Unified,
      PersistedFolder::Label {
        id,
      } => Folder::Label(id),
      PersistedFolder::Standard {
        name,
      } => Folder::Standard(name.into()),
    }
  }
}

impl From<StandardFolder> for PersistedStandard {
  fn from(standard_folder: StandardFolder) -> Self {
    match standard_folder {
      StandardFolder::Archive => PersistedStandard::Archive,
      StandardFolder::Drafts => PersistedStandard::Drafts,
      StandardFolder::Inbox => PersistedStandard::Inbox,
      StandardFolder::Sent => PersistedStandard::Sent,
      StandardFolder::Snoozed => PersistedStandard::Snoozed,
      StandardFolder::Starred => PersistedStandard::Starred,
      StandardFolder::Trash => PersistedStandard::Trash,
    }
  }
}

impl From<PersistedStandard> for StandardFolder {
  fn from(standard_folder: PersistedStandard) -> Self {
    match standard_folder {
      PersistedStandard::Archive => StandardFolder::Archive,
      PersistedStandard::Drafts => StandardFolder::Drafts,
      PersistedStandard::Inbox => StandardFolder::Inbox,
      PersistedStandard::Sent => StandardFolder::Sent,
      PersistedStandard::Snoozed => StandardFolder::Snoozed,
      PersistedStandard::Starred => StandardFolder::Starred,
      PersistedStandard::Trash => StandardFolder::Trash,
    }
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn it_round_trips_an_all_inboxes_default_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mail_nav.json");
    let state = NavState::default();

    save_to(&path, &state);

    assert_eq!(load_from(&path), state);
    assert_eq!(state.active, Scope::AllInboxes);
    assert_eq!(state.folder, Folder::Unified);
    assert_eq!(state.selected, None);
  }

  #[test]
  fn it_round_trips_a_character_scope_with_a_label_folder_and_a_large_mail_id() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mail_nav.json");
    let state = NavState::capture(Scope::Character(95_000_001), Folder::Label(42), Some(900_000_000_001));

    save_to(&path, &state);

    assert_eq!(load_from(&path), state);
  }

  #[test]
  fn it_round_trips_each_standard_folder() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mail_nav.json");
    for standard_folder in [
      StandardFolder::Archive,
      StandardFolder::Drafts,
      StandardFolder::Inbox,
      StandardFolder::Sent,
      StandardFolder::Snoozed,
      StandardFolder::Starred,
      StandardFolder::Trash,
    ] {
      let state = NavState::capture(Scope::AllInboxes, Folder::Standard(standard_folder), None);
      save_to(&path, &state);
      assert_eq!(load_from(&path), state);
    }
  }

  #[test]
  fn it_restores_defaults_for_an_absent_or_garbage_file() {
    let dir = tempfile::tempdir().unwrap();
    let absent = dir.path().join("missing.json");
    assert_eq!(load_from(&absent), NavState::default());

    let garbage = dir.path().join("garbage.json");
    std::fs::write(&garbage, b"{ not json").unwrap();
    assert_eq!(load_from(&garbage), NavState::default());
  }
}
