pub(super) mod clones;
pub(in crate::features::roster) mod contact_modal;
pub(in crate::features::roster) mod contacts;
pub(super) mod killlog;
pub(super) mod notifications;
pub(in crate::features::roster) mod shared;
pub(crate) mod standings;

use std::sync::LazyLock;

use iced::{
  Element, Length, Padding,
  widget::{Column, container, responsive, scrollable},
};

use super::{Message, State};
use crate::{
  clients::esi::scopes,
  config::Feature,
  features::shell::registry,
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
    static LABELS: LazyLock<[String; 5]> = LazyLock::new(|| {
      [
        t!("roster.tabs.clones").into_owned(),
        t!("roster.tabs.contacts").into_owned(),
        t!("roster.tabs.killlog").into_owned(),
        t!("roster.tabs.notifications").into_owned(),
        t!("roster.tabs.standings").into_owned(),
      ]
    });
    match self {
      Tab::Clones => LABELS[0].as_str(),
      Tab::Contacts => LABELS[1].as_str(),
      Tab::Killlog => LABELS[2].as_str(),
      Tab::Notifications => LABELS[3].as_str(),
      Tab::Standings => LABELS[4].as_str(),
    }
  }

  fn noun(self) -> &'static str {
    self.feature().noun()
  }

  /// The tab's read scopes only, so the forbidden wall gates visibility on reads — a missing write scope (e.g.
  /// `write_contacts`) must never block viewing the tab.
  fn read_scopes(self) -> Vec<&'static str> {
    self
      .required_scopes()
      .iter()
      .copied()
      .filter(|scope| !scopes::is_write_scope(scope))
      .collect()
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
        icon: None,
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
  let missing = forbidden::missing_scopes(state.granted_scopes(), &state.active_tab.read_scopes());
  if !missing.is_empty() {
    let forbidden = forbidden::forbidden(
      state.active_tab.noun(),
      state.active_name(),
      &missing,
      Message::ReauthRequested(state.active()),
    );
    return plain_scroll(state.active_tab, forbidden);
  }

  match state.active_tab {
    Tab::Clones => plain_scroll(Tab::Clones, clones::body(&state.clones)),
    Tab::Notifications => plain_scroll(
      Tab::Notifications,
      notifications::body(&state.notifications, state.notifications_filter),
    ),
    Tab::Contacts => {
      let write_enabled = state.contacts_write_enabled();
      windowed_tab(
        Tab::Contacts,
        Some(contacts::header(
          &state.contacts,
          state.contact_filter,
          state.contacts_query(),
          write_enabled,
        )),
        move |viewport_height| {
          contacts::body(
            &state.contacts,
            state.contact_sort,
            write_enabled,
            viewport_height,
            state.contacts_scroll_offset(),
            contacts::ContactColumns::default(),
          )
        },
      )
    }
    Tab::Killlog => windowed_tab(
      Tab::Killlog,
      killlog::header(&state.killlog, state.killlog_filter),
      move |viewport_height| {
        killlog::body(
          &state.killlog,
          state.killlog_filter,
          viewport_height,
          state.killlog_scroll_offset(),
        )
      },
    ),
    Tab::Standings => windowed_tab(
      Tab::Standings,
      Some(standings::header(
        state.standings_query(),
        state.standings_filter,
        state.standings_has_filters(),
      )),
      move |viewport_height| {
        standings::body(
          &state.standings,
          state.standings_filter,
          state.standings_has_filters(),
          viewport_height,
          state.standings_scroll_offset(),
        )
      },
    ),
  }
}

fn plain_scroll(active_tab: Tab, inner: Element<'_, Message>) -> Element<'_, Message> {
  scrollable(container(inner).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: TAB_BODY_PADDING,
    bottom: spacing::SPACE_6,
    left: TAB_BODY_PADDING,
  }))
  .style(crate::ui::style::control::scrollbar)
  .width(Length::Fill)
  .height(Length::Fill)
  .on_scroll(move |viewport| scroll_message(active_tab, viewport.relative_offset().y, viewport.absolute_offset().y))
  .into()
}

/// Lays out a windowed tab: a hoisted, non-scrolling `header` above a height-filling scrollable whose sole content
/// is the virtualized body. The scrollable is nested inside `responsive` so the body builder receives the real
/// viewport height (which reaches the virtual window) rather than the unbounded height a wrapping scrollable would
/// impose. The scrollbar's offset still drives both the pagination threshold and the virtual window.
fn windowed_tab<'a>(
  active_tab: Tab,
  header: Option<Element<'a, Message>>,
  body_builder: impl Fn(f32) -> Element<'a, Message> + 'a,
) -> Element<'a, Message> {
  let side = Padding {
    top: 0.0,
    right: TAB_BODY_PADDING,
    bottom: 0.0,
    left: TAB_BODY_PADDING,
  };

  let scroll = responsive(move |size| {
    scrollable(
      container(body_builder(size.height))
        .width(Length::Fill)
        .padding(Padding {
          top: 0.0,
          right: TAB_BODY_PADDING,
          bottom: spacing::SPACE_6,
          left: TAB_BODY_PADDING,
        }),
    )
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .on_scroll(move |viewport| scroll_message(active_tab, viewport.relative_offset().y, viewport.absolute_offset().y))
    .into()
  });

  let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(2);
  if let Some(header) = header {
    children.push(
      container(header)
        .width(Length::Fill)
        .padding(Padding {
          top: spacing::SPACE_6,
          bottom: spacing::SPACE_3_5,
          ..side
        })
        .into(),
    );
  }
  children.push(scroll.into());

  Column::with_children(children)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn scroll_message(tab: Tab, relative: f32, absolute: f32) -> Message {
  match tab {
    Tab::Killlog => Message::KilllogScrolled {
      absolute,
      relative,
    },
    Tab::Standings => Message::StandingsScrolled {
      absolute,
      relative,
    },
    Tab::Clones | Tab::Contacts | Tab::Notifications => Message::ContactsScrolled {
      absolute,
      relative,
    },
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
    fn it_drops_gated_tabs_whose_feature_is_disabled() {
      let tabs = enabled_tabs(&[Feature::Standings]);

      assert_eq!(tabs, vec![Tab::Standings]);
    }

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
    fn it_leaves_no_tabs_with_no_features() {
      assert_eq!(enabled_tabs(&[]), Vec::<Tab>::new());
    }
  }

  mod read_scopes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_drops_the_write_scope_so_the_forbidden_wall_only_gates_reads() {
      assert_eq!(Tab::Contacts.read_scopes(), vec![scopes::CHARACTER_CONTACTS]);
    }

    #[test]
    fn it_keeps_read_only_tabs_unchanged() {
      assert_eq!(Tab::Standings.read_scopes(), vec![scopes::CHARACTER_STANDINGS]);
    }
  }

  mod required_scopes {
    use pretty_assertions::assert_eq;

    use super::*;

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

    #[test]
    fn it_maps_each_gated_tab_to_its_esi_scope() {
      assert_eq!(Tab::Clones.required_scopes(), &[scopes::CHARACTER_CLONES]);
      assert_eq!(
        Tab::Contacts.required_scopes(),
        &[scopes::CHARACTER_CONTACTS, scopes::CHARACTER_CONTACTS_WRITE]
      );
      assert_eq!(Tab::Killlog.required_scopes(), &[scopes::CHARACTER_KILLMAILS]);
      assert_eq!(Tab::Notifications.required_scopes(), &[scopes::CHARACTER_NOTIFICATIONS]);
      assert_eq!(Tab::Standings.required_scopes(), &[scopes::CHARACTER_STANDINGS]);
    }
  }

  mod resolve_first_tab {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_clones_for_an_empty_list() {
      assert_eq!(resolve_first_tab(&[]), Tab::Clones);
    }

    #[test]
    fn it_picks_clones_first_when_all_enabled() {
      assert_eq!(resolve_first_tab(&enabled_tabs(&Feature::ALL)), Tab::Clones);
    }

    #[test]
    fn it_picks_the_first_enabled_gated_tab() {
      assert_eq!(resolve_first_tab(&enabled_tabs(&[Feature::Standings])), Tab::Standings);
    }
  }

  mod scroll_message {
    use super::*;

    #[test]
    fn it_routes_each_paginated_tab_to_its_scroll_message() {
      assert!(matches!(
        scroll_message(Tab::Contacts, 0.9, 120.0),
        Message::ContactsScrolled {
          relative: 0.9,
          absolute: 120.0
        }
      ));
      assert!(matches!(
        scroll_message(Tab::Killlog, 0.9, 120.0),
        Message::KilllogScrolled {
          relative: 0.9,
          absolute: 120.0
        }
      ));
      assert!(matches!(
        scroll_message(Tab::Standings, 0.9, 120.0),
        Message::StandingsScrolled {
          relative: 0.9,
          absolute: 120.0
        }
      ));
    }
  }

  mod tab_body {
    use super::*;

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
  }
}
