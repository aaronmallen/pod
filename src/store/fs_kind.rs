use std::path::Path;

const NETWORK_FSTYPE_NAMES: [&str; 5] = ["afpfs", "nfs", "smbfs", "webdav", "cifs"];

#[cfg(target_os = "linux")]
const NETWORK_MAGICS: [i64; 5] = [
  0x6969,      // NFS_SUPER_MAGIC
  0xFF53_4D42, // CIFS_MAGIC_NUMBER
  0xFE53_4D42, // SMB2_MAGIC_NUMBER
  0x517B,      // SMB_SUPER_MAGIC
  0x564C,      // NCP_SUPER_MAGIC (legacy NetWare, treated as network)
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FsKind {
  Local,
  Network,
}

impl FsKind {
  pub fn is_network(self) -> bool {
    matches!(self, FsKind::Network)
  }
}

pub fn classify_fstype_name(name: &str) -> FsKind {
  let name = name.trim().to_ascii_lowercase();
  if NETWORK_FSTYPE_NAMES
    .iter()
    .any(|known| name == *known || name.starts_with(*known))
  {
    FsKind::Network
  } else {
    FsKind::Local
  }
}

#[cfg(target_os = "linux")]
pub fn classify_fstype_magic(magic: i64) -> FsKind {
  if NETWORK_MAGICS.contains(&magic) {
    FsKind::Network
  } else {
    FsKind::Local
  }
}

pub fn detect(path: &Path) -> FsKind {
  detect_impl(path).unwrap_or(FsKind::Local)
}

#[cfg(target_os = "macos")]
fn detect_impl(path: &Path) -> Option<FsKind> {
  use std::{
    ffi::{CStr, CString},
    os::unix::ffi::OsStrExt,
  };

  let c_path = CString::new(probe_target(path).as_os_str().as_bytes()).ok()?;
  let mut stat: libc::statfs = unsafe { std::mem::zeroed() };

  let rc = unsafe { libc::statfs(c_path.as_ptr(), &mut stat) };
  if rc != 0 {
    return None;
  }

  let name = unsafe { CStr::from_ptr(stat.f_fstypename.as_ptr()) };
  Some(classify_fstype_name(&name.to_string_lossy()))
}

#[cfg(target_os = "linux")]
fn detect_impl(path: &Path) -> Option<FsKind> {
  use std::{ffi::CString, os::unix::ffi::OsStrExt};

  let c_path = CString::new(probe_target(path).as_os_str().as_bytes()).ok()?;
  let mut stat: libc::statfs = unsafe { std::mem::zeroed() };

  let rc = unsafe { libc::statfs(c_path.as_ptr(), &mut stat) };
  if rc != 0 {
    return None;
  }

  Some(classify_fstype_magic(stat.f_type as i64))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn detect_impl(_path: &Path) -> Option<FsKind> {
  None
}

/// Walks up to the nearest existing ancestor so `statfs` has a real target.
///
/// A configured directory may not exist yet on first launch; `statfs` fails on a missing path, so detection probes the
/// closest ancestor that does exist, which shares the same filesystem.
#[cfg(unix)]
fn probe_target(path: &Path) -> std::path::PathBuf {
  let mut candidate = path.to_path_buf();
  while !candidate.exists() {
    match candidate.parent() {
      Some(parent) if parent != candidate => candidate = parent.to_path_buf(),
      _ => break,
    }
  }
  candidate
}

#[cfg(test)]
mod tests {
  use super::*;

  #[cfg(target_os = "linux")]
  mod classify_fstype_magic {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_classifies_the_cifs_magic_as_network() {
      assert_eq!(classify_fstype_magic(0xFF53_4D42), FsKind::Network);
    }

    #[test]
    fn it_classifies_the_ext4_magic_as_local() {
      assert_eq!(classify_fstype_magic(0xEF53), FsKind::Local);
    }

    #[test]
    fn it_classifies_the_nfs_magic_as_network() {
      assert_eq!(classify_fstype_magic(0x6969), FsKind::Network);
    }

    #[test]
    fn it_classifies_the_smb2_magic_as_network() {
      assert_eq!(classify_fstype_magic(0xFE53_4D42), FsKind::Network);
    }
  }

  mod classify_fstype_name {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_classifies_a_local_filesystem_as_local() {
      assert_eq!(classify_fstype_name("apfs"), FsKind::Local);
      assert_eq!(classify_fstype_name("ext4"), FsKind::Local);
      assert_eq!(classify_fstype_name("hfs"), FsKind::Local);
    }

    #[test]
    fn it_classifies_afp_as_network() {
      assert_eq!(classify_fstype_name("afpfs"), FsKind::Network);
    }

    #[test]
    fn it_classifies_cifs_as_network() {
      assert_eq!(classify_fstype_name("cifs"), FsKind::Network);
    }

    #[test]
    fn it_classifies_nfs_as_network() {
      assert_eq!(classify_fstype_name("nfs"), FsKind::Network);
    }

    #[test]
    fn it_classifies_smb_as_network() {
      assert_eq!(classify_fstype_name("smbfs"), FsKind::Network);
    }

    #[test]
    fn it_classifies_webdav_as_network() {
      assert_eq!(classify_fstype_name("webdav"), FsKind::Network);
    }

    #[test]
    fn it_is_case_insensitive_and_trims_whitespace() {
      assert_eq!(classify_fstype_name(" SMBFS "), FsKind::Network);
    }

    #[test]
    fn it_matches_versioned_variants_by_prefix() {
      assert_eq!(classify_fstype_name("nfs4"), FsKind::Network);
    }
  }

  mod detect {
    use super::*;

    #[test]
    fn it_reports_a_not_yet_created_path_as_local() {
      let dir = tempfile::tempdir().unwrap();
      let nested = dir.path().join("does").join("not").join("exist");

      assert_eq!(detect(&nested), FsKind::Local);
    }

    #[test]
    fn it_reports_a_temp_dir_as_local() {
      let dir = tempfile::tempdir().unwrap();

      assert_eq!(detect(dir.path()), FsKind::Local);
    }
  }
}
