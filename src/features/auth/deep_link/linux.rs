use std::{
  path::{Path, PathBuf},
  process::Command,
};

use super::{SCHEME, single_instance};

const APP_ID: &str = "dev.aaronmallen.pod";

pub fn install() {
  if let Some(lock) = single_instance::try_become_primary() {
    single_instance::spawn_listener(lock, super::deliver);
  }
  register_scheme();
}

fn desktop_entry(exec: &Path) -> String {
  [
    "[Desktop Entry]".to_owned(),
    "Type=Application".to_owned(),
    "Name=Pod".to_owned(),
    format!("Exec={} %u", exec.display()),
    "NoDisplay=true".to_owned(),
    "StartupNotify=false".to_owned(),
    format!("MimeType=x-scheme-handler/{SCHEME};"),
    String::new(),
  ]
  .join("\n")
}

fn register_scheme() {
  if let Err(error) = write_handler() {
    tracing::warn!(%error, "deep-link scheme self-registration skipped");
  }
}

fn write_handler() -> std::io::Result<()> {
  let applications_dir = resolve_applications_dir()?;
  let exec = std::env::current_exe()?;
  write_desktop_entry(&applications_dir, &exec)?;
  update_desktop_database(&applications_dir);

  Ok(())
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

fn applications_dir() -> Option<PathBuf> {
  dir_spec::data_home().map(|dir| dir.join("applications"))
}

fn update_desktop_database(applications_dir: &Path) {
  let outcome = Command::new("update-desktop-database").arg(applications_dir).status();
  log_database_refresh(outcome);
}

fn log_database_refresh(outcome: std::io::Result<std::process::ExitStatus>) {
  match outcome {
    Ok(status) => log_nonzero_exit(status),
    Err(error) => tracing::warn!(%error, "update-desktop-database unavailable; scheme may route only after a refresh"),
  }
}

fn log_nonzero_exit(status: std::process::ExitStatus) {
  if !status.success() {
    tracing::warn!(%status, "update-desktop-database returned a non-zero status");
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
    fn it_uses_an_absolute_exec_with_a_url_placeholder() {
      let entry = desktop_entry(&PathBuf::from("/opt/pod/pod"));

      assert!(entry.contains("Exec=/opt/pod/pod %u"));
    }

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
