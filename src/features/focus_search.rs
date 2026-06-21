use iced::advanced::widget::Id;

use crate::ui::components::rail::Destination;

const ASSETS_SEARCH: &str = "pod.focus-search.assets";

pub fn search_id(destination: Destination) -> Option<Id> {
  match destination {
    Destination::Assets => Some(Id::new(ASSETS_SEARCH)),
    Destination::Calendar
    | Destination::Characters
    | Destination::Industry
    | Destination::Mail
    | Destination::Settings
    | Destination::Skills
    | Destination::Wallet => None,
  }
}

pub fn assets_search_id() -> Id {
  Id::new(ASSETS_SEARCH)
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
    fn it_resolves_the_same_id_across_calls() {
      assert_eq!(search_id(Destination::Assets), search_id(Destination::Assets));
    }

    #[test]
    fn it_returns_none_for_an_unregistered_route() {
      assert_eq!(search_id(Destination::Mail), None);
    }
  }
}
