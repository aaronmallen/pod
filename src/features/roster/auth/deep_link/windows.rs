use std::path::Path;

use winreg::{RegKey, enums::HKEY_CURRENT_USER};

use super::{PACK_EXTENSIONS, SCHEME, single_instance};

const PACK_PROGID: &str = "Pod.PackFile";
const PACK_PROGID_DESCRIPTION: &str = "Pod Pack File";
const PROTOCOL_DESCRIPTION: &str = "URL:Pod Protocol";

pub fn install() {
  if let Some(lock) = single_instance::try_become_primary() {
    single_instance::spawn_listener(lock, super::deliver);
  }
  register_scheme();
  register_file_associations();
}

fn register_scheme() {
  if let Err(error) = write_handler() {
    tracing::warn!(%error, "deep-link scheme self-registration skipped");
  }
}

fn register_file_associations() {
  if let Err(error) = write_file_associations() {
    tracing::warn!(%error, "pack file-association self-registration skipped");
  }
}

fn write_file_associations() -> std::io::Result<()> {
  let exe = std::env::current_exe()?;
  let command = handler_command(&exe);

  let hkcu = RegKey::predef(HKEY_CURRENT_USER);
  write_progid(&hkcu, &command)?;
  for ext in PACK_EXTENSIONS {
    claim_extension(&hkcu, ext)?;
  }
  Ok(())
}

fn write_progid(hkcu: &RegKey, command: &str) -> std::io::Result<()> {
  let command_path = format!(r"Software\Classes\{PACK_PROGID}\shell\open\command");
  let stored = hkcu
    .open_subkey(&command_path)
    .and_then(|key| key.get_value::<String, _>(""))
    .ok();
  if !should_write(stored.as_deref(), command) {
    return Ok(());
  }

  let (progid_key, _) = hkcu.create_subkey(format!(r"Software\Classes\{PACK_PROGID}"))?;
  progid_key.set_value("", &PACK_PROGID_DESCRIPTION)?;
  write_command_key(hkcu, &command_path, command)
}

fn claim_extension(hkcu: &RegKey, ext: &str) -> std::io::Result<()> {
  let ext_path = format!(r"Software\Classes\.{ext}");
  let stored = hkcu
    .open_subkey(&ext_path)
    .and_then(|key| key.get_value::<String, _>(""))
    .ok();
  if !should_claim_extension(stored.as_deref()) {
    return Ok(());
  }

  let (ext_key, _) = hkcu.create_subkey(&ext_path)?;
  ext_key.set_value("", &PACK_PROGID)?;
  Ok(())
}

/// Claims a pack extension only when nothing is registered yet or an earlier write left an
/// empty ProgID; never overwrites an extension a different app has already claimed for itself.
fn should_claim_extension(stored: Option<&str>) -> bool {
  match stored {
    None => true,
    Some(value) => value.is_empty(),
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

  write_scheme_root(&hkcu)?;
  write_command_key(&hkcu, &command_path, &command)
}

fn write_scheme_root(hkcu: &RegKey) -> std::io::Result<()> {
  let (scheme_key, _) = hkcu.create_subkey(format!(r"Software\Classes\{SCHEME}"))?;
  scheme_key.set_value("", &PROTOCOL_DESCRIPTION)?;
  scheme_key.set_value("URL Protocol", &"")?;
  Ok(())
}

fn write_command_key(hkcu: &RegKey, command_path: &str, command: &str) -> std::io::Result<()> {
  let (command_key, _) = hkcu.create_subkey(command_path)?;
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

  mod should_claim_extension {
    use super::*;

    #[test]
    fn it_claims_an_extension_with_no_registered_handler() {
      assert!(should_claim_extension(None));
    }

    #[test]
    fn it_claims_an_extension_whose_handler_is_empty() {
      assert!(should_claim_extension(Some("")));
    }

    #[test]
    fn it_leaves_a_deliberate_reassignment_to_another_app_untouched() {
      assert!(!should_claim_extension(Some("Other.App.PackFile")));
    }

    #[test]
    fn it_does_not_reclaim_an_extension_already_pointed_at_pod() {
      assert!(!should_claim_extension(Some(PACK_PROGID)));
    }
  }

  mod should_write {
    use super::*;

    #[test]
    fn it_rewrites_when_the_stored_command_points_at_a_different_path() {
      let desired = handler_command(&std::path::PathBuf::from(r"C:\New\pod.exe"));
      let stored = handler_command(&std::path::PathBuf::from(r"C:\Old\pod.exe"));

      assert!(should_write(Some(&stored), &desired));
    }

    #[test]
    fn it_skips_when_the_stored_command_already_matches() {
      let command = handler_command(&std::path::PathBuf::from(r"C:\Pod\pod.exe"));

      assert!(!should_write(Some(&command), &command));
    }

    #[test]
    fn it_writes_when_nothing_is_registered_yet() {
      let desired = handler_command(&std::path::PathBuf::from(r"C:\Pod\pod.exe"));

      assert!(should_write(None, &desired));
    }
  }
}
