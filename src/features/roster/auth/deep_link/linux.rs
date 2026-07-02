use std::{
  path::{Path, PathBuf},
  process::Command,
};

use super::{SCHEME, single_instance};

const APP_ID: &str = "dev.aaronmallen.pod";
const PACK_TYPES: [PackType; 3] = [
  PackType {
    comment: "Pod Budget Rules Pack",
    extension: "pbr",
    mime_type: "application/x-pod-budget-rules",
  },
  PackType {
    comment: "Pod Facility Intel Pack",
    extension: "pfi",
    mime_type: "application/x-pod-facility-intel",
  },
  PackType {
    comment: "Pod Skill Plan Pack",
    extension: "psp",
    mime_type: "application/x-pod-skill-plan",
  },
];

struct PackType {
  comment: &'static str,
  extension: &'static str,
  mime_type: &'static str,
}

pub fn install() {
  if let Some(lock) = single_instance::try_become_primary() {
    single_instance::spawn_listener(lock, super::deliver);
  }
  register_scheme();
  register_file_associations();
}

fn desktop_entry(exec: &Path) -> String {
  [
    "[Desktop Entry]".to_owned(),
    "Type=Application".to_owned(),
    "Name=Pod".to_owned(),
    format!("Exec={} %u", exec.display()),
    "NoDisplay=true".to_owned(),
    "StartupNotify=false".to_owned(),
    mime_type_line(),
    String::new(),
  ]
  .join("\n")
}

fn mime_type_line() -> String {
  let mut line = format!("MimeType=x-scheme-handler/{SCHEME};");
  for pack in PACK_TYPES {
    line.push_str(pack.mime_type);
    line.push(';');
  }
  line
}

fn mime_info() -> String {
  let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
  xml.push_str("<mime-info xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\">\n");
  for pack in PACK_TYPES {
    xml.push_str(&format!("  <mime-type type=\"{}\">\n", pack.mime_type));
    xml.push_str(&format!("    <comment>{}</comment>\n", pack.comment));
    xml.push_str(&format!("    <glob pattern=\"*.{}\"/>\n", pack.extension));
    xml.push_str("  </mime-type>\n");
  }
  xml.push_str("</mime-info>\n");
  xml
}

fn register_scheme() {
  if let Err(error) = write_handler() {
    tracing::warn!(%error, "deep-link scheme self-registration skipped");
  }
}

fn register_file_associations() {
  if let Err(error) = write_mime_info() {
    tracing::warn!(%error, "pack file-association self-registration skipped");
  }
}

fn write_handler() -> std::io::Result<()> {
  let applications_dir = resolve_applications_dir()?;
  let exec = handler_exec()?;
  write_desktop_entry(&applications_dir, &exec)?;
  refresh_database("update-desktop-database", &applications_dir);

  Ok(())
}

fn write_mime_info() -> std::io::Result<()> {
  let mime_dir = resolve_mime_dir()?;
  let packages_dir = mime_dir.join("packages");
  std::fs::create_dir_all(&packages_dir)?;
  std::fs::write(packages_dir.join(format!("{APP_ID}.xml")), mime_info())?;
  refresh_database("update-mime-database", &mime_dir);

  Ok(())
}

/// The path the OS should launch to route a deep link.
///
/// Inside an AppImage `current_exe()` points into the ephemeral `/tmp/.mount_*`
/// FUSE mount, which is gone once the app exits — a handler baked with that path
/// silently fails on the next launch. The AppImage runtime exports `$APPIMAGE`
/// with the persistent path to the bundle itself, so prefer it when present.
fn handler_exec() -> std::io::Result<PathBuf> {
  match resolve_handler_exec(std::env::var_os("APPIMAGE")) {
    Some(exec) => Ok(exec),
    None => std::env::current_exe(),
  }
}

fn resolve_handler_exec(appimage: Option<std::ffi::OsString>) -> Option<PathBuf> {
  appimage.filter(|value| !value.is_empty()).map(PathBuf::from)
}

fn write_desktop_entry(applications_dir: &Path, exec: &Path) -> std::io::Result<()> {
  std::fs::create_dir_all(applications_dir)?;
  std::fs::write(applications_dir.join(format!("{APP_ID}.desktop")), desktop_entry(exec))
}

fn resolve_applications_dir() -> std::io::Result<PathBuf> {
  applications_dir().ok_or_else(|| {
    std::io::Error::new(
      std::io::ErrorKind::NotFound,
      "could not resolve the user data directory",
    )
  })
}

fn resolve_mime_dir() -> std::io::Result<PathBuf> {
  mime_dir().ok_or_else(|| {
    std::io::Error::new(
      std::io::ErrorKind::NotFound,
      "could not resolve the user data directory",
    )
  })
}

fn applications_dir() -> Option<PathBuf> {
  dir_spec::data_home().map(|dir| dir.join("applications"))
}

fn mime_dir() -> Option<PathBuf> {
  dir_spec::data_home().map(|dir| dir.join("mime"))
}

fn refresh_database(tool: &str, target: &Path) {
  let outcome = Command::new(tool).arg(target).status();
  log_database_refresh(tool, outcome);
}

fn log_database_refresh(tool: &str, outcome: std::io::Result<std::process::ExitStatus>) {
  match outcome {
    Ok(status) => log_nonzero_exit(tool, status),
    Err(error) => {
      tracing::warn!(%error, tool, "database refresh tool unavailable; associations may route only after a refresh")
    }
  }
}

fn log_nonzero_exit(tool: &str, status: std::process::ExitStatus) {
  if !status.success() {
    tracing::warn!(%status, tool, "database refresh returned a non-zero status");
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod desktop_entry {
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_declares_the_scheme_handler_mime_type_from_the_scheme_constant() {
      let entry = desktop_entry(&PathBuf::from("/opt/pod/pod"));

      assert!(entry.contains(&format!("MimeType=x-scheme-handler/{SCHEME};")));
      assert!(entry.contains("MimeType=x-scheme-handler/eveauth-pod;"));
    }

    #[test]
    fn it_generates_identical_content_for_the_same_binary() {
      let exec = PathBuf::from("/opt/pod/pod");

      assert_eq!(desktop_entry(&exec), desktop_entry(&exec));
    }

    #[test]
    fn it_uses_an_absolute_exec_with_a_url_placeholder() {
      let entry = desktop_entry(&PathBuf::from("/opt/pod/pod"));

      assert!(entry.contains("Exec=/opt/pod/pod %u"));
    }
  }

  mod mime_type_line {
    use super::*;

    #[test]
    fn it_keeps_the_scheme_handler_and_appends_the_three_pack_mime_types() {
      let line = mime_type_line();

      assert!(line.contains(&format!("MimeType=x-scheme-handler/{SCHEME};")));
      assert!(line.contains("application/x-pod-budget-rules;"));
      assert!(line.contains("application/x-pod-facility-intel;"));
      assert!(line.contains("application/x-pod-skill-plan;"));
    }
  }

  mod mime_info {
    use super::*;

    #[test]
    fn it_defines_a_glob_and_comment_for_each_pack_type() {
      let xml = mime_info();

      for pack in PACK_TYPES {
        assert!(xml.contains(&format!("<mime-type type=\"{}\">", pack.mime_type)));
        assert!(xml.contains(&format!("<comment>{}</comment>", pack.comment)));
        assert!(xml.contains(&format!("<glob pattern=\"*.{}\"/>", pack.extension)));
      }
    }

    #[test]
    fn it_wraps_the_definitions_in_a_shared_mime_info_root() {
      let xml = mime_info();

      assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
      assert!(xml.contains("<mime-info xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\">"));
      assert!(xml.trim_end().ends_with("</mime-info>"));
    }

    #[test]
    fn it_generates_identical_content_across_calls() {
      assert_eq!(mime_info(), mime_info());
    }
  }

  mod resolve_handler_exec {
    use std::ffi::OsString;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_current_exe_when_appimage_is_empty() {
      assert_eq!(resolve_handler_exec(Some(OsString::new())), None);
    }

    #[test]
    fn it_falls_back_to_current_exe_when_appimage_is_unset() {
      assert_eq!(resolve_handler_exec(None), None);
    }

    #[test]
    fn it_prefers_the_appimage_path_when_set() {
      let resolved = resolve_handler_exec(Some(OsString::from("/home/me/Pod.AppImage")));

      assert_eq!(resolved, Some(PathBuf::from("/home/me/Pod.AppImage")));
    }
  }

  mod write_handler {
    use pretty_assertions::assert_eq;

    use super::*;

    fn write_once(applications_dir: &Path) {
      write_desktop_entry(applications_dir, &PathBuf::from("/opt/pod/pod")).unwrap();
    }

    #[test]
    fn it_leaves_a_single_handler_file_with_unchanged_content_when_run_twice() {
      let dir = tempfile::tempdir().unwrap();
      let applications_dir = dir.path().join("applications");

      write_once(&applications_dir);
      let after_first = std::fs::read_to_string(applications_dir.join(format!("{APP_ID}.desktop"))).unwrap();
      write_once(&applications_dir);
      let after_second = std::fs::read_to_string(applications_dir.join(format!("{APP_ID}.desktop"))).unwrap();

      assert_eq!(after_first, after_second);

      let entries: Vec<_> = std::fs::read_dir(&applications_dir).unwrap().collect();
      assert_eq!(entries.len(), 1, "re-running does not create a second handler file");
    }
  }
}
