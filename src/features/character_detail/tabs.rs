pub(super) mod clones;
pub(super) mod contacts;
pub(super) mod killlog;
pub(super) mod notifications;
pub(super) mod scope_missing;
mod shared;

use iced::{
  Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, container, scrollable, text},
};

use super::{LoadState, Message, State};
use crate::{
  clients::esi::scopes,
  config::Feature,
  store::model::CharacterStanding,
  ui::{
    components::{
      card,
      empty_state::{LoadStateView, empty_state, load_state_view},
      meter, rule,
      section_header::section_header,
      tab_select::{Tab as SelectTab, TabLayout, tab_select_with},
    },
    style::{color, spacing, typography},
  },
};

const GROUPS: [(&str, &str); 3] = [
  ("faction", "Factions"),
  ("npc_corp", "Corporations"),
  ("agent", "Agents"),
];
const STANDING_BAR_HEIGHT: f32 = 6.0;
const STANDING_BAR_WIDTH: f32 = 160.0;
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
    match self {
      Tab::Clones => Feature::CloneMonitoring,
      Tab::Contacts => Feature::Contacts,
      Tab::Killlog => Feature::CombatLog,
      Tab::Notifications => Feature::EveNotifications,
      Tab::Standings => Feature::Standings,
    }
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

  fn required_scopes(self) -> &'static [&'static str] {
    match self {
      Tab::Clones => &[scopes::CHARACTER_CLONES],
      Tab::Contacts => &[scopes::CHARACTER_CONTACTS],
      Tab::Killlog => &[scopes::CHARACTER_KILLMAILS],
      Tab::Notifications => &[scopes::CHARACTER_NOTIFICATIONS],
      Tab::Standings => &[scopes::CHARACTER_STANDINGS],
    }
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
  let inner: Element<'_, Message> =
    if scope_missing::is_scope_missing(state.granted_scopes(), state.active_tab.required_scopes()) {
      scope_missing::scope_missing(state.active())
    } else {
      match state.active_tab {
        Tab::Clones => clones::body(&state.clones),
        Tab::Contacts => contacts::body(&state.contacts, state.contact_filter, state.contact_sort),
        Tab::Killlog => killlog::body(&state.killlog, state.killlog_filter),
        Tab::Notifications => notifications::body(&state.notifications, state.notifications_filter),
        Tab::Standings => standings_body(&state.standings),
      }
    };

  scrollable(container(inner).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: TAB_BODY_PADDING,
    bottom: spacing::SPACE_6,
    left: TAB_BODY_PADDING,
  }))
  .style(crate::ui::style::control::scrollbar)
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn standings_body(standings: &LoadState<Vec<CharacterStanding>>) -> Element<'_, Message> {
  let rows = match standings {
    LoadState::Loaded(rows) => rows,
    LoadState::Loading => return load_state_view(LoadStateView::Loading("Loading standings\u{2026}")),
    LoadState::Error(error) => return load_state_view(LoadStateView::Error(error)),
  };
  if rows.is_empty() {
    return load_state_view(LoadStateView::Empty(empty_state("No standings recorded")));
  }

  let mut sections: Vec<Element<'_, Message>> = Vec::new();
  for (key, label) in GROUPS {
    let group: Vec<&CharacterStanding> = rows.iter().filter(|row| row.from_type() == key).collect();
    if group.is_empty() {
      continue;
    }
    sections.push(standings_section(label, &group));
  }
  let other: Vec<&CharacterStanding> = rows
    .iter()
    .filter(|row| !GROUPS.iter().any(|(key, _)| *key == row.from_type()))
    .collect();
  if !other.is_empty() {
    sections.push(standings_section("Other", &other));
  }

  Column::with_children(sections)
    .spacing(spacing::SPACE_6)
    .width(Length::Fill)
    .into()
}

fn standings_section<'a>(label: &'a str, rows: &[&'a CharacterStanding]) -> Element<'a, Message> {
  let eyebrow = section_header(label, Some(&format!("{} tracked", rows.len())));

  let mut card_rows: Vec<Element<'a, Message>> = Vec::with_capacity(rows.len());
  for (index, row) in rows.iter().enumerate() {
    card_rows.push(standing_row(row, index == rows.len() - 1));
  }
  let card = card::panel(Column::with_children(card_rows).width(Length::Fill), false);

  Column::with_children(vec![eyebrow, card])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .into()
}

fn standing_row<'a>(row: &CharacterStanding, last: bool) -> Element<'a, Message> {
  let value = row.standing();
  let accent = shared::standing_color(value);

  let name = text(row.from_name().clone())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    })
    .width(Length::Fill);

  let signed = text(format!("{}{:.2}", if value >= 0.0 { "+" } else { "" }, value))
    .font(typography::mono::MEDIUM)
    .size(typography::size::MD)
    .style(move |_| text::Style {
      color: Some(accent),
    });

  let bar = meter::diverging(
    value,
    shared::STANDING_MAX,
    accent,
    STANDING_BAR_WIDTH,
    STANDING_BAR_HEIGHT,
  );
  let inner = Row::with_children(vec![name.into(), signed.into(), bar])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  let border_bottom = if last { 0.0 } else { 1.0 };
  container(inner)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3_5,
    })
    .style(move |_| shared::row_rule_style(border_bottom))
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn standing(from_id: i64, from_type: &str, value: f64) -> CharacterStanding {
    CharacterStanding {
      character_id: 42,
      from_id,
      from_name: format!("Entity {from_id}"),
      from_type: from_type.to_owned(),
      standing: value,
    }
  }

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

  mod tab_body {
    use super::*;

    #[test]
    fn it_renders_the_scope_missing_state_when_the_tab_scope_is_not_granted() {
      let mut state = State::new(42, &Feature::ALL);
      state.granted_scopes = None;
      state.active_tab = Tab::Standings;

      assert!(scope_missing::is_scope_missing(
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

      assert!(!scope_missing::is_scope_missing(
        state.granted_scopes(),
        Tab::Standings.required_scopes()
      ));
      let _el: Element<'_, Message> = tab_body(&state);
    }
  }

  mod standings_body {
    use super::*;

    #[test]
    fn it_renders_grouped_standings() {
      let loaded = LoadState::Loaded(vec![
        standing(500_001, "faction", 6.0),
        standing(1_000_125, "npc_corp", -3.0),
        standing(3_000_100, "agent", 1.2),
        standing(9_999, "unknown_type", 0.0),
      ]);

      let _el: Element<'_, Message> = standings_body(&loaded);
    }

    #[test]
    fn it_renders_the_empty_and_error_states() {
      let empty = LoadState::Loaded(Vec::new());
      let loading: LoadState<Vec<CharacterStanding>> = LoadState::Loading;
      let error: LoadState<Vec<CharacterStanding>> = LoadState::Error("boom".to_owned());

      let _empty: Element<'_, Message> = standings_body(&empty);
      let _loading: Element<'_, Message> = standings_body(&loading);
      let _error: Element<'_, Message> = standings_body(&error);
    }
  }
}
