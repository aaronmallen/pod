use std::path::Path;

use winreg::{RegKey, enums::HKEY_CURRENT_USER};

use super::{SCHEME, single_instance};

const PROTOCOL_DESCRIPTION: &str = "URL:Pod Protocol";

pub fn install() {
  if let Some(lock) = single_instance::try_become_primary() {
    single_instance::spawn_listener(lock, super::deliver);
  }
  register_scheme();
}

fn register_scheme() {
  if let Err(error) = write_handler() {
    tracing::warn!(%error, "deep-link scheme self-registration skipped");
  }
}

fn write_handler() -> std::io::Result<()> {
  let exe = std::env::current_exe()?;
  let command = handler_command(&exe);

  let hkcu = RegKey::predef(HKEY_CURRENT_USER);
  let command_path = format!(r"Software\Classes\{SCHEME}\shell\open\command");
  let stored = hkcu
    .open_subkey(&command_path)
    .and_then(|key| key.get_value::<String, _>(""))
    .ok();
  if !should_write(stored.as_deref(), &command) {
    return Ok(());
  }

  let (scheme_key, _) = hkcu.create_subkey(format!(r"Software\Classes\{SCHEME}"))?;
  scheme_key.set_value("", &PROTOCOL_DESCRIPTION)?;
  scheme_key.set_value("URL Protocol", &"")?;
  let (command_key, _) = hkcu.create_subkey(&command_path)?;
  command_key.set_value("", &command)?;

  Ok(())
}

fn handler_command(exe: &Path) -> String {
  format!("\"{}\" \"%1\"", exe.display())
}

fn should_write(stored: Option<&str>, desired: &str) -> bool {
  stored != Some(desired)
}

#[cfg(test)]
mod tests {
  use super::*;

  mod handler_command {
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_quotes_the_exe_and_appends_the_url_placeholder() {
      let command = handler_command(&PathBuf::from(r"C:\Program Files\Pod\pod.exe"));

      assert_eq!(command, r#""C:\Program Files\Pod\pod.exe" "%1""#);
    }
  }

  mod should_write {
    use super::*;

    #[test]
    fn it_skips_when_the_stored_command_already_matches() {
      let command = handler_command(&std::path::PathBuf::from(r"C:\Pod\pod.exe"));

      assert!(!should_write(Some(&command), &command));
    }

    #[test]
    fn it_rewrites_when_the_stored_command_points_at_a_different_path() {
      let desired = handler_command(&std::path::PathBuf::from(r"C:\New\pod.exe"));
      let stored = handler_command(&std::path::PathBuf::from(r"C:\Old\pod.exe"));

      assert!(should_write(Some(&stored), &desired));
    }

    #[test]
    fn it_writes_when_nothing_is_registered_yet() {
      let desired = handler_command(&std::path::PathBuf::from(r"C:\Pod\pod.exe"));

      assert!(should_write(None, &desired));
    }
  }
}
