mod language;

// Re-exported for the sibling i18n tasks (config field, ESI injection, SDE re-seed) that consume it;
// nothing outside this module references it yet, so it reads as unused until those land.
#[allow(unused_imports)]
pub use language::Language;

pub fn set_locale(language: Language) {
  rust_i18n::set_locale(language.esi_code());
}

#[cfg(test)]
mod tests {
  use super::*;

  mod set_locale {
    use super::*;

    #[test]
    fn it_resolves_a_key_in_the_active_locale() {
      set_locale(Language::En);

      assert_eq!(t!("shell.notifications.title"), "Notifications");
    }

    #[test]
    fn it_interpolates_a_named_placeholder() {
      set_locale(Language::En);

      assert_eq!(t!("shell.notifications.footer_unread", count => 3), "3 unread");
    }

    #[test]
    fn it_falls_back_to_en_for_a_missing_locale() {
      set_locale(Language::EnUs);

      assert_eq!(t!("shell.notifications.title"), "Notifications");
    }

    #[test]
    fn it_returns_the_key_for_an_absent_key_without_panicking() {
      set_locale(Language::En);

      assert_eq!(t!("totally.absent.key"), "totally.absent.key");
    }
  }
}
