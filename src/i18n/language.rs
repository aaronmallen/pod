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

  pub fn from_code(code: &str) -> Option<Language> {
    Language::ALL.into_iter().find(|language| language.esi_code() == code)
  }

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

  pub fn label(self) -> String {
    match self {
      Language::De => t!("language.name.de").into_owned(),
      Language::En => t!("language.name.en").into_owned(),
      Language::EnUs => t!("language.name.en_us").into_owned(),
      Language::Es => t!("language.name.es").into_owned(),
      Language::Fr => t!("language.name.fr").into_owned(),
      Language::Ja => t!("language.name.ja").into_owned(),
      Language::Ko => t!("language.name.ko").into_owned(),
      Language::Ru => t!("language.name.ru").into_owned(),
      Language::Zh => t!("language.name.zh").into_owned(),
    }
  }

  /// The SDE localization key. `EnUs` collapses to `en` because the SDE ships no US-English column.
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
