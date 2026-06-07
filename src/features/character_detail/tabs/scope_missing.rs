use std::collections::HashSet;

use iced::Element;

use super::super::Message;
use crate::ui::components::empty_state::empty_state;

pub fn missing_scopes<'a>(granted: Option<&str>, required: &[&'a str]) -> Vec<&'a str> {
  let granted: HashSet<&str> = granted.unwrap_or_default().split_whitespace().collect();
  required
    .iter()
    .copied()
    .filter(|scope| !granted.contains(scope))
    .collect()
}

pub fn is_scope_missing(granted: Option<&str>, required: &[&str]) -> bool {
  !missing_scopes(granted, required).is_empty()
}

pub fn scope_missing<'a>(character_id: i64) -> Element<'a, Message> {
  empty_state("Additional access required")
    .subtitle("This tab needs an EVE access scope you haven't granted yet. Reauthorize to enable it.")
    .action("Reauthorize", Message::ReauthRequested(character_id))
    .render()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::clients::esi::scopes;

  mod missing_scopes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_lists_every_required_scope_absent_from_the_grant() {
      let granted = scopes::CHARACTER_CLONES;
      let required = [scopes::CHARACTER_CLONES, scopes::CHARACTER_STANDINGS];

      assert_eq!(
        missing_scopes(Some(granted), &required),
        vec![scopes::CHARACTER_STANDINGS]
      );
    }

    #[test]
    fn it_is_empty_when_the_grant_covers_every_required_scope() {
      let granted = format!("{} {}", scopes::CHARACTER_CLONES, scopes::CHARACTER_STANDINGS);
      let required = [scopes::CHARACTER_CLONES];

      assert!(missing_scopes(Some(&granted), &required).is_empty());
    }

    #[test]
    fn it_reports_all_required_scopes_when_the_grant_is_absent() {
      let required = [scopes::CHARACTER_CONTACTS];

      assert_eq!(missing_scopes(None, &required), vec![scopes::CHARACTER_CONTACTS]);
    }
  }

  mod is_scope_missing {
    use super::*;

    #[test]
    fn it_is_false_when_nothing_is_required() {
      assert!(!is_scope_missing(None, &[]));
      assert!(!is_scope_missing(Some(scopes::CHARACTER_CLONES), &[]));
    }

    #[test]
    fn it_is_true_when_a_required_scope_is_not_granted() {
      assert!(is_scope_missing(
        Some(scopes::CHARACTER_CLONES),
        &[scopes::CHARACTER_STANDINGS]
      ));
      assert!(is_scope_missing(None, &[scopes::CHARACTER_KILLMAILS]));
      assert!(is_scope_missing(Some("   "), &[scopes::CHARACTER_KILLMAILS]));
    }

    #[test]
    fn it_is_false_when_the_grant_is_a_superset() {
      let granted = format!("{} {}", scopes::CHARACTER_CLONES, scopes::CHARACTER_STANDINGS);
      assert!(!is_scope_missing(Some(&granted), &[scopes::CHARACTER_CLONES]));
    }
  }

  mod scope_missing {
    use super::*;

    #[test]
    fn it_renders_with_a_reauthorize_action() {
      let _el: Element<'_, Message> = super::scope_missing(42);
    }
  }
}
