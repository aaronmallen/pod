use iced::advanced::widget::Id;

use crate::ui::components::rail::Destination;

const ASSETS_SEARCH: &str = "pod.focus-search.assets";
// Characters has several searchable sub-views (roster, character/corporation detail
// contacts and standings tabs). The roster search is the primary target because it is
// the destination's default landing pane; it reuses the roster's pre-existing id so the
// registry and the live input agree.
const CHARACTERS_SEARCH: &str = "roster-search-input";
const INDUSTRY_SEARCH: &str = "pod.focus-search.industry";
const MAIL_SEARCH: &str = "pod.focus-search.mail";
const SETTINGS_SEARCH: &str = "pod.focus-search.settings";
const SKILLS_SEARCH: &str = "pod.focus-search.skills";
const WALLET_SEARCH: &str = "pod.focus-search.wallet";

pub fn search_id(destination: Destination) -> Option<Id> {
  match destination {
    Destination::Assets => Some(Id::new(ASSETS_SEARCH)),
    Destination::Characters => Some(Id::new(CHARACTERS_SEARCH)),
    Destination::Industry => Some(Id::new(INDUSTRY_SEARCH)),
    Destination::Mail => Some(Id::new(MAIL_SEARCH)),
    Destination::Settings => Some(Id::new(SETTINGS_SEARCH)),
    Destination::Skills => Some(Id::new(SKILLS_SEARCH)),
    Destination::Wallet => Some(Id::new(WALLET_SEARCH)),
    Destination::Calendar => None,
  }
}

pub fn assets_search_id() -> Id {
  Id::new(ASSETS_SEARCH)
}

pub fn characters_search_id() -> Id {
  Id::new(CHARACTERS_SEARCH)
}

pub fn industry_search_id() -> Id {
  Id::new(INDUSTRY_SEARCH)
}

pub fn mail_search_id() -> Id {
  Id::new(MAIL_SEARCH)
}

pub fn settings_search_id() -> Id {
  Id::new(SETTINGS_SEARCH)
}

pub fn skills_search_id() -> Id {
  Id::new(SKILLS_SEARCH)
}

pub fn wallet_search_id() -> Id {
  Id::new(WALLET_SEARCH)
}

#[cfg(test)]
mod tests {
  use super::*;

  mod search_id {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_a_stable_id_for_a_registered_route() {
      assert_eq!(search_id(Destination::Assets), Some(assets_search_id()));
    }

    #[test]
    fn it_resolves_a_stable_id_for_characters() {
      assert_eq!(search_id(Destination::Characters), Some(characters_search_id()));
    }

    #[test]
    fn it_resolves_a_stable_id_for_industry() {
      assert_eq!(search_id(Destination::Industry), Some(industry_search_id()));
    }

    #[test]
    fn it_resolves_a_stable_id_for_mail() {
      assert_eq!(search_id(Destination::Mail), Some(mail_search_id()));
    }

    #[test]
    fn it_resolves_a_stable_id_for_settings() {
      assert_eq!(search_id(Destination::Settings), Some(settings_search_id()));
    }

    #[test]
    fn it_resolves_a_stable_id_for_skills() {
      assert_eq!(search_id(Destination::Skills), Some(skills_search_id()));
    }

    #[test]
    fn it_resolves_a_stable_id_for_wallet() {
      assert_eq!(search_id(Destination::Wallet), Some(wallet_search_id()));
    }

    #[test]
    fn it_resolves_the_same_id_across_calls() {
      assert_eq!(search_id(Destination::Assets), search_id(Destination::Assets));
    }

    #[test]
    fn it_returns_none_for_an_unregistered_route() {
      assert_eq!(search_id(Destination::Calendar), None);
    }
  }
}
