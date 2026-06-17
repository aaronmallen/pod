use std::{
  fs, io,
  path::{Path, PathBuf},
  sync::OnceLock,
  time::Duration,
};

use crate::{clients::eve_image::Size, config};

pub const LOGO_SIZE: Size = Size::S256;

pub const PORTRAIT_SIZE: Size = Size::S512;

pub const STALE_AFTER: Duration = Duration::from_secs(60 * 60 * 24 * 7);

static DEFAULT_STORE: OnceLock<Store> = OnceLock::new();

static IMAGE_ROOT: OnceLock<PathBuf> = OnceLock::new();

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum IconResolution {
  Found(PathBuf),
  #[default]
  Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IconVariant {
  Bpc,
  Bpo,
  Icon,
}

impl IconVariant {
  pub fn from_blueprint_copy(is_blueprint_copy: Option<bool>) -> Self {
    match is_blueprint_copy {
      Some(true) => Self::Bpc,
      Some(false) => Self::Bpo,
      None => Self::Icon,
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImageKind {
  AllianceLogo,
  CharacterPortrait,
  CorporationLogo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageState {
  Fresh(PathBuf),
  Stale { id: i64, kind: ImageKind },
}

impl ImageState {
  pub fn path(&self) -> Option<PathBuf> {
    match self {
      ImageState::Fresh(path) => Some(path.clone()),
      ImageState::Stale {
        ..
      } => None,
    }
  }

  pub fn stale_key(&self) -> Option<(ImageKind, i64)> {
    match self {
      ImageState::Fresh(_) => None,
      ImageState::Stale {
        id,
        kind,
      } => Some((*kind, *id)),
    }
  }
}

#[derive(Clone, Debug)]
pub struct Store {
  committed_items: Option<PathBuf>,
  root: PathBuf,
}

impl Store {
  pub fn new(root: PathBuf) -> Self {
    Self {
      committed_items: None,
      root,
    }
  }

  pub fn alliance_logo_path(&self, alliance_id: i64) -> PathBuf {
    self.root.join("alliances").join(format!("{alliance_id}.png"))
  }

  pub fn character_portrait_path(&self, character_id: i64) -> PathBuf {
    self.root.join("characters").join(format!("{character_id}.jpg"))
  }

  pub fn corporation_logo_path(&self, corporation_id: i64) -> PathBuf {
    self.root.join("corporations").join(format!("{corporation_id}.png"))
  }

  pub fn image_path(&self, kind: ImageKind, id: i64) -> PathBuf {
    match kind {
      ImageKind::AllianceLogo => self.alliance_logo_path(id),
      ImageKind::CharacterPortrait => self.character_portrait_path(id),
      ImageKind::CorporationLogo => self.corporation_logo_path(id),
    }
  }

  pub fn resolve_type_icon(&self, type_id: i64, is_blueprint_copy: Option<bool>, size: Size) -> IconResolution {
    let variant = IconVariant::from_blueprint_copy(is_blueprint_copy);

    let variant_path = self.type_icon_variant_path(type_id, variant, size);
    if variant_path.exists() {
      return IconResolution::Found(variant_path);
    }

    if variant != IconVariant::Icon {
      let icon_path = self.type_icon_variant_path(type_id, IconVariant::Icon, size);
      if icon_path.exists() {
        return IconResolution::Found(icon_path);
      }
    }

    if size == Size::S64
      && let Some(committed) = &self.committed_items
    {
      if variant == IconVariant::Bpc {
        let bpc_path = committed.join(format!("{type_id}_bpc.png"));
        if bpc_path.exists() {
          return IconResolution::Found(bpc_path);
        }
      }
      let committed_path = committed.join(format!("{type_id}.png"));
      if committed_path.exists() {
        return IconResolution::Found(committed_path);
      }
    }

    IconResolution::Missing
  }

  pub fn type_icon_path(&self, type_id: i64, size: Size) -> PathBuf {
    self.type_icon_variant_path(type_id, IconVariant::Icon, size)
  }

  pub fn type_icon_variant_path(&self, type_id: i64, _variant: IconVariant, _size: Size) -> PathBuf {
    self.root.join("types").join(format!("{type_id}.png"))
  }

  pub fn with_committed_items(mut self, dir: PathBuf) -> Self {
    self.committed_items = Some(dir);
    self
  }

  pub fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
  }
}

/// Initializes the process-wide image cache root from the loaded settings. Called once at boot; subsequent calls are ignored.
pub fn init_root(root: PathBuf) {
  let _ = IMAGE_ROOT.set(root);
}

pub fn default_store() -> Store {
  DEFAULT_STORE.get_or_init(build_default_store).clone()
}

fn build_default_store() -> Store {
  let root = IMAGE_ROOT.get().cloned().unwrap_or_else(|| {
    config::load()
      .map(|settings| settings.storage().resolved_cache_dir())
      .unwrap_or_else(|_| config::cache_dir())
      .join("images")
  });
  Store::new(root).with_committed_items(config::resource_dir().join("assets").join("images").join("items"))
}

/// A missing or unreadable file is treated as stale (callers should refetch); a file whose mtime is in the future is
/// treated as fresh.
pub fn is_fresh(path: &Path, max_age: Duration) -> bool {
  fs::metadata(path)
    .and_then(|meta| meta.modified())
    .is_ok_and(|modified| modified.elapsed().map_or(true, |age| age < max_age))
}

pub fn resolve(store: &Store, kind: ImageKind, id: i64) -> ImageState {
  let path = store.image_path(kind, id);
  if is_fresh(&path, STALE_AFTER) {
    ImageState::Fresh(path)
  } else {
    ImageState::Stale {
      id,
      kind,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod alliance_logo_path {
    use super::*;

    #[test]
    fn it_derives_a_flat_per_alliance_png_path() {
      let store = Store::new(PathBuf::from("/data/images"));

      let path = store.alliance_logo_path(99_000_001);

      assert!(path.ends_with("alliances/99000001.png"), "got {path:?}");
    }
  }

  mod character_portrait_path {
    use super::*;

    #[test]
    fn it_derives_a_flat_per_character_path() {
      let store = Store::new(PathBuf::from("/data/images"));

      let path = store.character_portrait_path(42);

      assert!(path.ends_with("characters/42.jpg"), "got {path:?}");
    }
  }

  mod corporation_logo_path {
    use super::*;

    #[test]
    fn it_derives_a_flat_per_corporation_png_path() {
      let store = Store::new(PathBuf::from("/data/images"));

      let path = store.corporation_logo_path(98_000_001);

      assert!(path.ends_with("corporations/98000001.png"), "got {path:?}");
    }
  }

  mod icon_variant {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_the_blueprint_copy_flag_to_a_variant() {
      assert_eq!(IconVariant::from_blueprint_copy(Some(true)), IconVariant::Bpc);
      assert_eq!(IconVariant::from_blueprint_copy(Some(false)), IconVariant::Bpo);
      assert_eq!(IconVariant::from_blueprint_copy(None), IconVariant::Icon);
    }
  }

  mod is_fresh {
    use super::*;

    #[test]
    fn it_treats_a_file_older_than_the_max_age_as_stale() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("portrait.jpg");
      std::fs::write(&path, [0u8]).unwrap();

      assert!(!super::super::is_fresh(&path, Duration::ZERO));
    }

    #[test]
    fn it_treats_a_missing_file_as_stale() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("missing.jpg");

      assert!(!super::super::is_fresh(&path, STALE_AFTER));
    }

    #[test]
    fn it_treats_a_recently_written_file_as_fresh() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("portrait.jpg");
      std::fs::write(&path, [0u8]).unwrap();

      assert!(super::super::is_fresh(&path, STALE_AFTER));
    }
  }

  mod resolve {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_fresh_for_a_present_recent_file() {
      let dir = tempfile::tempdir().unwrap();
      let store = Store::new(dir.path().to_path_buf());
      let path = store.character_portrait_path(42);
      store.write(&path, &[1]).unwrap();

      let state = super::super::resolve(&store, ImageKind::CharacterPortrait, 42);

      assert_eq!(state, ImageState::Fresh(path.clone()));
      assert_eq!(state.path(), Some(path));
      assert_eq!(state.stale_key(), None);
    }

    #[test]
    fn it_resolves_stale_for_a_missing_file() {
      let dir = tempfile::tempdir().unwrap();
      let store = Store::new(dir.path().to_path_buf());

      let state = super::super::resolve(&store, ImageKind::CorporationLogo, 98_000_001);

      assert_eq!(
        state,
        ImageState::Stale {
          id: 98_000_001,
          kind: ImageKind::CorporationLogo,
        }
      );
      assert_eq!(state.path(), None);
      assert_eq!(state.stale_key(), Some((ImageKind::CorporationLogo, 98_000_001)));
    }
  }

  mod resolve_type_icon {
    use super::*;

    #[test]
    fn it_does_not_use_a_bpc_render_for_a_blueprint_original() {
      let data = tempfile::tempdir().unwrap();
      let committed = tempfile::tempdir().unwrap();
      let store = Store::new(data.path().to_path_buf()).with_committed_items(committed.path().to_path_buf());
      let shared = committed.path().join("587.png");
      std::fs::write(&shared, [1]).unwrap();
      std::fs::write(committed.path().join("587_bpc.png"), [2]).unwrap();

      let resolved = store.resolve_type_icon(587, Some(false), Size::S64);

      assert_eq!(resolved, IconResolution::Found(shared));
    }

    #[test]
    fn it_falls_back_to_the_committed_tier_when_the_data_dir_has_no_match() {
      let data = tempfile::tempdir().unwrap();
      let committed = tempfile::tempdir().unwrap();
      let store = Store::new(data.path().to_path_buf()).with_committed_items(committed.path().to_path_buf());
      let committed_icon = committed.path().join("587.png");
      std::fs::write(&committed_icon, [1]).unwrap();

      let resolved = store.resolve_type_icon(587, None, Size::S64);

      assert_eq!(resolved, IconResolution::Found(committed_icon));
    }

    #[test]
    fn it_falls_back_to_the_shared_blueprint_icon_when_no_bpc_render_is_committed() {
      let data = tempfile::tempdir().unwrap();
      let committed = tempfile::tempdir().unwrap();
      let store = Store::new(data.path().to_path_buf()).with_committed_items(committed.path().to_path_buf());
      let shared = committed.path().join("587.png");
      std::fs::write(&shared, [1]).unwrap();

      let resolved = store.resolve_type_icon(587, Some(true), Size::S64);

      assert_eq!(resolved, IconResolution::Found(shared));
    }

    #[test]
    fn it_ignores_the_committed_tier_for_sizes_other_than_64px() {
      let data = tempfile::tempdir().unwrap();
      let committed = tempfile::tempdir().unwrap();
      let store = Store::new(data.path().to_path_buf()).with_committed_items(committed.path().to_path_buf());
      std::fs::write(committed.path().join("587.png"), [1]).unwrap();

      let resolved = store.resolve_type_icon(587, None, Size::S128);

      assert_eq!(resolved, IconResolution::Missing);
    }

    #[test]
    fn it_only_consults_the_committed_tier_at_s64() {
      let data = tempfile::tempdir().unwrap();
      let committed = tempfile::tempdir().unwrap();
      let store = Store::new(data.path().to_path_buf()).with_committed_items(committed.path().to_path_buf());
      std::fs::write(committed.path().join("587.png"), [1]).unwrap();

      assert_eq!(store.resolve_type_icon(587, None, Size::S32), IconResolution::Missing);
      assert!(matches!(
        store.resolve_type_icon(587, None, Size::S64),
        IconResolution::Found(_)
      ));
    }

    #[test]
    fn it_prefers_a_committed_bpc_render_over_the_shared_blueprint_icon() {
      let data = tempfile::tempdir().unwrap();
      let committed = tempfile::tempdir().unwrap();
      let store = Store::new(data.path().to_path_buf()).with_committed_items(committed.path().to_path_buf());
      std::fs::write(committed.path().join("587.png"), [1]).unwrap();
      let bpc = committed.path().join("587_bpc.png");
      std::fs::write(&bpc, [2]).unwrap();

      let resolved = store.resolve_type_icon(587, Some(true), Size::S64);

      assert_eq!(resolved, IconResolution::Found(bpc));
    }

    #[test]
    fn it_prefers_the_data_dir_tier_over_the_committed_tier() {
      let data = tempfile::tempdir().unwrap();
      let committed = tempfile::tempdir().unwrap();
      let store = Store::new(data.path().to_path_buf()).with_committed_items(committed.path().to_path_buf());
      let data_icon = store.type_icon_path(587, Size::S64);
      store.write(&data_icon, &[1]).unwrap();
      std::fs::write(committed.path().join("587.png"), [2]).unwrap();

      let resolved = store.resolve_type_icon(587, None, Size::S64);

      assert_eq!(resolved, IconResolution::Found(data_icon));
    }

    #[test]
    fn it_resolves_a_non_blueprint_directly_from_the_icon_variant() {
      let dir = tempfile::tempdir().unwrap();
      let store = Store::new(dir.path().to_path_buf());
      let icon = store.type_icon_path(587, Size::S64);
      store.write(&icon, &[1]).unwrap();

      let resolved = store.resolve_type_icon(587, None, Size::S64);

      assert_eq!(resolved, IconResolution::Found(icon));
    }

    #[test]
    fn it_resolves_the_runtime_icon_when_present() {
      let dir = tempfile::tempdir().unwrap();
      let store = Store::new(dir.path().to_path_buf());
      let icon = store.type_icon_path(587, Size::S64);
      store.write(&icon, &[1]).unwrap();

      let resolved = store.resolve_type_icon(587, Some(true), Size::S64);

      assert_eq!(resolved, IconResolution::Found(icon));
    }

    #[test]
    fn it_signals_missing_when_neither_the_variant_nor_the_icon_is_on_disk() {
      let dir = tempfile::tempdir().unwrap();
      let store = Store::new(dir.path().to_path_buf());

      let resolved = store.resolve_type_icon(587, Some(true), Size::S64);

      assert_eq!(resolved, IconResolution::Missing);
    }

    #[test]
    fn it_signals_missing_when_neither_tier_has_the_icon() {
      let data = tempfile::tempdir().unwrap();
      let committed = tempfile::tempdir().unwrap();
      let store = Store::new(data.path().to_path_buf()).with_committed_items(committed.path().to_path_buf());

      let resolved = store.resolve_type_icon(587, None, Size::S64);

      assert_eq!(resolved, IconResolution::Missing);
    }
  }

  mod type_icon_path {
    use super::*;

    #[test]
    fn it_derives_a_flat_per_type_png_path() {
      let store = Store::new(PathBuf::from("/data/images"));

      let path = store.type_icon_path(587, Size::S64);

      assert!(path.ends_with("types/587.png"), "got {path:?}");
    }
  }

  mod type_icon_variant_path {
    use super::*;

    #[test]
    fn it_matches_type_icon_path_for_the_icon_variant() {
      let store = Store::new(PathBuf::from("/data/images"));

      assert_eq!(
        store.type_icon_variant_path(587, IconVariant::Icon, Size::S64),
        store.type_icon_path(587, Size::S64),
      );
    }

    #[test]
    fn it_uses_a_flat_path_regardless_of_variant() {
      let store = Store::new(PathBuf::from("/data/images"));

      let path = store.type_icon_variant_path(587, IconVariant::Bpc, Size::S64);

      assert!(path.ends_with("types/587.png"), "got {path:?}");
    }
  }

  mod write {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_writes_bytes_creating_parent_directories() {
      let dir = tempfile::tempdir().unwrap();
      let store = Store::new(dir.path().to_path_buf());
      let path = store.character_portrait_path(7);

      store.write(&path, &[1, 2, 3]).unwrap();

      assert_eq!(std::fs::read(&path).unwrap(), vec![1, 2, 3]);
    }
  }
}
