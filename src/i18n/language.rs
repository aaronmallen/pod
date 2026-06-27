use std::{
  fmt::{self, Display, Formatter},
  str::FromStr,
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Language {
  #[serde(rename = "de")]
  De,
  #[serde(rename = "en")]
  En,
  #[default]
  #[serde(rename = "en-us")]
  EnUs,
  #[serde(rename = "es")]
  Es,
  #[serde(rename = "fr")]
  Fr,
  #[serde(rename = "ja")]
  Ja,
  #[serde(rename = "ko")]
  Ko,
  #[serde(rename = "ru")]
  Ru,
  #[serde(rename = "zh")]
  Zh,
}

impl Language {
  pub const ALL: [Language; 9] = [
    Language::De,
    Language::En,
    Language::EnUs,
    Language::Es,
    Language::Fr,
    Language::Ja,
    Language::Ko,
    Language::Ru,
    Language::Zh,
  ];

  // Consumed by sibling i18n tasks (config field, ESI injection, SDE re-seed); only the tests
  // exercise the parser today, so it is dead until those land.
  #[allow(dead_code)]
  pub fn from_code(code: &str) -> Option<Language> {
    Language::ALL.into_iter().find(|language| language.esi_code() == code)
  }

  /// The display label, in the language's own script, for the selector UI.
  // Reached by the settings selector (sibling task); unused until that UI lands.
  #[allow(dead_code)]
  pub fn native_label(self) -> &'static str {
    match self {
      Language::De => "Deutsch",
      Language::En => "English",
      Language::EnUs => "English (US)",
      Language::Es => "Español",
      Language::Fr => "Français",
      Language::Ja => "日本語",
      Language::Ko => "한국어",
      Language::Ru => "Русский",
      Language::Zh => "中文",
    }
  }

  /// The ESI `?language=` query-parameter code and the serde wire form persisted to `config.toml`.
  // ESI injection is a sibling task; only the tests read this today.
  #[allow(dead_code)]
  pub fn esi_code(self) -> &'static str {
    match self {
      Language::De => "de",
      Language::En => "en",
      Language::EnUs => "en-us",
      Language::Es => "es",
      Language::Fr => "fr",
      Language::Ja => "ja",
      Language::Ko => "ko",
      Language::Ru => "ru",
      Language::Zh => "zh",
    }
  }

  /// The English display label for the selector UI.
  // Reached by the settings selector (sibling task); unused until that UI lands.
  #[allow(dead_code)]
  pub fn label(self) -> &'static str {
    match self {
      Language::De => "German",
      Language::En => "English",
      Language::EnUs => "English (US)",
      Language::Es => "Spanish",
      Language::Fr => "French",
      Language::Ja => "Japanese",
      Language::Ko => "Korean",
      Language::Ru => "Russian",
      Language::Zh => "Chinese",
    }
  }

  /// The SDE localization key. `EnUs` collapses to `en` because the SDE ships no US-English column.
  // SDE selection is a sibling task; only the tests read this today.
  #[allow(dead_code)]
  pub fn sde_code(self) -> &'static str {
    match self {
      Language::De => "de",
      Language::En | Language::EnUs => "en",
      Language::Es => "es",
      Language::Fr => "fr",
      Language::Ja => "ja",
      Language::Ko => "ko",
      Language::Ru => "ru",
      Language::Zh => "zh",
    }
  }
}

impl Display for Language {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.write_str(self.esi_code())
  }
}

impl FromStr for Language {
  type Err = ();

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Language::from_code(s).ok_or(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod all {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn it_lists_nine_unique_variants() {
      let unique: HashSet<Language> = Language::ALL.into_iter().collect();

      assert_eq!(unique.len(), 9);
      assert_eq!(Language::ALL.len(), 9);
    }
  }

  mod from_code {
    use super::*;

    #[test]
    fn it_round_trips_every_esi_code() {
      for language in Language::ALL {
        assert_eq!(Language::from_code(language.esi_code()), Some(language));
      }
    }

    #[test]
    fn it_rejects_an_unknown_code() {
      assert_eq!(Language::from_code("xx"), None);
    }
  }

  mod from_str {
    use super::*;

    #[test]
    fn it_parses_a_known_code() {
      assert_eq!("de".parse::<Language>(), Ok(Language::De));
    }

    #[test]
    fn it_errors_on_an_unknown_code() {
      assert_eq!("nope".parse::<Language>(), Err(()));
    }
  }

  mod default {
    use super::*;

    #[test]
    fn it_defaults_to_en_us() {
      assert_eq!(Language::default(), Language::EnUs);
    }
  }

  mod esi_code {
    use super::*;

    #[test]
    fn it_gives_every_variant_a_nonempty_code() {
      for language in Language::ALL {
        assert!(!language.esi_code().is_empty(), "{language:?} must have an ESI code");
      }
    }
  }

  mod label {
    use super::*;

    #[test]
    fn it_gives_every_variant_a_nonempty_label() {
      for language in Language::ALL {
        assert!(!language.label().is_empty(), "{language:?} must have a label");
        assert!(
          !language.native_label().is_empty(),
          "{language:?} must have a native label"
        );
      }
    }
  }

  mod sde_code {
    use super::*;

    #[test]
    fn it_collapses_en_us_to_en() {
      assert_eq!(Language::EnUs.sde_code(), "en");
      assert_eq!(Language::En.sde_code(), "en");
    }

    #[test]
    fn it_gives_every_variant_a_nonempty_code() {
      for language in Language::ALL {
        assert!(!language.sde_code().is_empty(), "{language:?} must have an SDE code");
      }
    }
  }

  mod serde {
    use super::*;

    #[test]
    fn it_serializes_to_the_esi_wire_code() {
      let toml = toml::to_string(&Wrapper {
        language: Language::EnUs,
      })
      .unwrap();

      assert!(toml.contains("language = \"en-us\""), "{toml}");
    }

    #[test]
    fn it_round_trips_every_variant_through_toml() {
      for language in Language::ALL {
        let toml = toml::to_string(&Wrapper {
          language,
        })
        .unwrap();
        let back: Wrapper = toml::from_str(&toml).unwrap();

        assert_eq!(back.language, language);
      }
    }

    #[derive(Deserialize, Serialize)]
    struct Wrapper {
      language: Language,
    }
  }
}
