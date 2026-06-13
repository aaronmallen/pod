pub(super) mod clones;
pub(super) mod contacts;
pub(super) mod killlog;
pub(super) mod notifications;
mod shared;
pub(crate) mod standings;

use iced::{
  Element, Length, Padding,
  widget::{Column, container, scrollable},
};

use super::{Message, State};
use crate::{
  config::Feature,
  features::registry,
  ui::{
    components::{
      forbidden, rule,
      tab_select::{Tab as SelectTab, TabLayout, tab_select_with},
    },
    style::spacing,
  },
};

const TAB_BODY_PADDING: f32 = 28.0;
const TAB_STRIP_HEIGHT: f32 = 48.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
  Clones,
  Contacts,
  Killlog,
  Notifications,
  Standings,
}

impl Tab {
  const ORDER: [Tab; 5] = [
    Tab::Clones,
    Tab::Contacts,
    Tab::Killlog,
    Tab::Notifications,
    Tab::Standings,
  ];

  fn feature(self) -> Feature {
    registry::feature_for_tab(self).expect("every gated tab maps to a feature")
  }

  fn label(self) -> &'static str {
    match self {
      Tab::Clones => "Clones",
      Tab::Contacts => "Contacts",
      Tab::Killlog => "Kill Log",
      Tab::Notifications => "Notifications",
      Tab::Standings => "Standings",
    }
  }

  fn noun(self) -> &'static str {
    self.feature().noun()
  }

  fn required_scopes(self) -> &'static [&'static str] {
    registry::descriptor(self.feature()).scopes
  }
}

pub(super) fn enabled_tabs(features: &[Feature]) -> Vec<Tab> {
  Tab::ORDER
    .into_iter()
    .filter(|tab| features.contains(&tab.feature()))
    .collect()
}

pub(super) fn resolve_first_tab(enabled: &[Tab]) -> Tab {
  enabled.first().copied().unwrap_or(Tab::Clones)
}

pub(super) fn tab_strip(enabled: &[Tab], active: Tab) -> Element<'_, Message> {
  let tabs: Vec<SelectTab<'_, Message>> = enabled
    .iter()
    .map(|&tab| {
      let selected = tab == active;
      SelectTab {
        count: String::new(),
        label: tab.label(),
        on_press: (!selected).then_some(Message::TabChanged(tab)),
        selected,
      }
    })
    .collect();

  let strip = container(tab_select_with(tabs, TabLayout::Start))
    .width(Length::Fill)
    .height(Length::Fixed(TAB_STRIP_HEIGHT))
    .padding(Padding {
      top: 0.0,
      right: TAB_BODY_PADDING,
      bottom: 0.0,
      left: TAB_BODY_PADDING,
    });

  Column::with_children(vec![strip.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

pub(super) fn tab_body(state: &State) -> Element<'_, Message> {
  let missing = forbidden::missing_scopes(state.granted_scopes(), state.active_tab.required_scopes());
  let inner: Element<'_, Message> = if missing.is_empty() {
    match state.active_tab {
      Tab::Clones => clones::body(&state.clones),
      Tab::Contacts => contacts::body(
        &state.contacts,
        state.contact_filter,
        state.contact_sort,
        state.contacts_visible(),
      ),
      Tab::Killlog => killlog::body(&state.killlog, state.killlog_filter),
      Tab::Notifications => notifications::body(&state.notifications, state.notifications_filter),
      Tab::Standings => standings::body(
        &state.standings,
        state.standings_query(),
        state.standings_filter,
        state.standings_has_filters(),
      ),
    }
  } else {
    forbidden::forbidden(
      state.active_tab.noun(),
      state.active_name(),
      &missing,
      Message::ReauthRequested(state.active()),
    )
  };

  // The detail view shares one scrollbar across every tab; route its scroll offset to the active tab's
  // pagination message so each list grows independently as the user nears the bottom.
  let active_tab = state.active_tab;
  scrollable(container(inner).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: TAB_BODY_PADDING,
    bottom: spacing::SPACE_6,
    left: TAB_BODY_PADDING,
  }))
  .style(crate::ui::style::control::scrollbar)
  .width(Length::Fill)
  .height(Length::Fill)
  .on_scroll(move |viewport| scroll_message(active_tab, viewport.relative_offset().y))
  .into()
}

fn scroll_message(tab: Tab, offset: f32) -> Message {
  match tab {
    Tab::Contacts => Message::ContactsScrolled(offset),
    Tab::Killlog => Message::KilllogScrolled(offset),
    Tab::Standings => Message::StandingsScrolled(offset),
    Tab::Clones | Tab::Notifications => Message::ContactsScrolled(offset),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::clients::esi::scopes;

  mod enabled_tabs {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keeps_strip_order_with_all_features_enabled() {
      let tabs = enabled_tabs(&Feature::ALL);

      assert_eq!(
        tabs,
        vec![
          Tab::Clones,
          Tab::Contacts,
          Tab::Killlog,
          Tab::Notifications,
          Tab::Standings
        ]
      );
    }

    #[test]
    fn it_drops_gated_tabs_whose_feature_is_disabled() {
      let tabs = enabled_tabs(&[Feature::Standings]);

      assert_eq!(tabs, vec![Tab::Standings]);
    }

    #[test]
    fn it_leaves_no_tabs_with_no_features() {
      assert_eq!(enabled_tabs(&[]), Vec::<Tab>::new());
    }
  }

  mod resolve_first_tab {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_picks_clones_first_when_all_enabled() {
      assert_eq!(resolve_first_tab(&enabled_tabs(&Feature::ALL)), Tab::Clones);
    }

    #[test]
    fn it_picks_the_first_enabled_gated_tab() {
      assert_eq!(resolve_first_tab(&enabled_tabs(&[Feature::Standings])), Tab::Standings);
    }

    #[test]
    fn it_falls_back_to_clones_for_an_empty_list() {
      assert_eq!(resolve_first_tab(&[]), Tab::Clones);
    }
  }

  mod required_scopes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_each_gated_tab_to_its_esi_scope() {
      assert_eq!(Tab::Clones.required_scopes(), &[scopes::CHARACTER_CLONES]);
      assert_eq!(Tab::Contacts.required_scopes(), &[scopes::CHARACTER_CONTACTS]);
      assert_eq!(Tab::Killlog.required_scopes(), &[scopes::CHARACTER_KILLMAILS]);
      assert_eq!(Tab::Notifications.required_scopes(), &[scopes::CHARACTER_NOTIFICATIONS]);
      assert_eq!(Tab::Standings.required_scopes(), &[scopes::CHARACTER_STANDINGS]);
    }

    #[test]
    fn every_tab_requires_at_least_one_scope() {
      for tab in Tab::ORDER {
        assert!(
          !tab.required_scopes().is_empty(),
          "{:?} must gate on a scope",
          tab.label()
        );
      }
    }
  }

  mod scroll_message {
    use super::*;

    #[test]
    fn it_routes_each_paginated_tab_to_its_scroll_message() {
      assert!(matches!(
        scroll_message(Tab::Contacts, 0.9),
        Message::ContactsScrolled(0.9)
      ));
      assert!(matches!(
        scroll_message(Tab::Killlog, 0.9),
        Message::KilllogScrolled(0.9)
      ));
      assert!(matches!(
        scroll_message(Tab::Standings, 0.9),
        Message::StandingsScrolled(0.9)
      ));
    }
  }

  mod tab_body {
    use super::*;

    #[test]
    fn it_renders_the_scope_missing_state_when_the_tab_scope_is_not_granted() {
      let mut state = State::new(42, &Feature::ALL);
      state.granted_scopes = None;
      state.active_tab = Tab::Standings;

      assert!(forbidden::is_scope_missing(
        state.granted_scopes(),
        Tab::Standings.required_scopes()
      ));
      let _el: Element<'_, Message> = tab_body(&state);
    }

    #[test]
    fn it_renders_the_content_branch_when_the_scope_is_granted() {
      let mut state = State::new(42, &Feature::ALL);
      state.granted_scopes = Some(scopes::CHARACTER_STANDINGS.to_owned());
      state.active_tab = Tab::Standings;

      assert!(!forbidden::is_scope_missing(
        state.granted_scopes(),
        Tab::Standings.required_scopes()
      ));
      let _el: Element<'_, Message> = tab_body(&state);
    }
  }
}
