use iced::{
  Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, container, scrollable, text},
};

use super::Outcome;
use crate::{
  config::{Feature, Settings, SubFeature},
  ui::{
    components::{empty_state, icon::Icon, rule, text_input::TextInput, toggle},
    style::{color, radius, spacing, typography},
  },
};

const CHILD_INDENT: f32 = spacing::SPACE_6;
const DESCRIPTION_MAX_WIDTH: f32 = 560.0;
const PANEL_BLURB: &str = "Toggle individual Pod capabilities on or off. Changes apply live across every window and your \
  linked characters \u{2014} no restart.";
const PANEL_SIDE_PADDING: f32 = 36.0;
const SEARCH_MAX_WIDTH: f32 = 480.0;

const CATALOG: [Catalog; 23] = [
  Catalog {
    sub: SubFeature::CloneMonitoring,
    title: "Clone Monitoring",
    description: "Track jump clones and the implants installed in each.",
  },
  Catalog {
    sub: SubFeature::Contacts,
    title: "Contacts",
    description: "Sync your personal contact list and their standings.",
  },
  Catalog {
    sub: SubFeature::KillLog,
    title: "Kill Log",
    description: "Capture combat activity for after-action review.",
  },
  Catalog {
    sub: SubFeature::Notifications,
    title: "EVE Notifications",
    description: "Sync in-game notifications from EVE Online.",
  },
  Catalog {
    sub: SubFeature::Standings,
    title: "Standings",
    description: "Sync your standings toward characters, corporations, and alliances.",
  },
  Catalog {
    sub: SubFeature::LocationTracking,
    title: "Location Tracking",
    description: "Track each character's current solar system, station, and ship.",
  },
  Catalog {
    sub: SubFeature::SkillQueue,
    title: "Skill Queue",
    description: "Monitor trained skills and the active training queue.",
  },
  Catalog {
    sub: SubFeature::JobMonitoring,
    title: "Job Monitoring",
    description: "Monitor running manufacturing, research, and reaction jobs.",
  },
  Catalog {
    sub: SubFeature::Blueprints,
    title: "Blueprints",
    description: "Track owned blueprints with their material and time efficiency.",
  },
  Catalog {
    sub: SubFeature::Planner,
    title: "Build Planner",
    description: "Plan recursive build orders with materials, costs, and facilities.",
  },
  Catalog {
    sub: SubFeature::Extractions,
    title: "Moon Extractions",
    description: "Track corporation moon-extraction timers.",
  },
  Catalog {
    sub: SubFeature::Mail,
    title: "Mail",
    description: "Sync EVE mail headers and message bodies.",
  },
  Catalog {
    sub: SubFeature::Calendar,
    title: "Calendar",
    description: "Sync calendar events and respond to invitations.",
  },
  Catalog {
    sub: SubFeature::Wallets,
    title: "Wallets",
    description: "Show character and corporation wallet balances.",
  },
  Catalog {
    sub: SubFeature::Journal,
    title: "Journal",
    description: "Sync the wallet transaction journal.",
  },
  Catalog {
    sub: SubFeature::Transactions,
    title: "Market Transactions",
    description: "Sync market orders and traded items.",
  },
  Catalog {
    sub: SubFeature::Contracts,
    title: "Contracts",
    description: "Sync outstanding and historical contracts.",
  },
  Catalog {
    sub: SubFeature::Budget,
    title: "Budget",
    description: "Plan a zero-based budget over your wallet activity.",
  },
  Catalog {
    sub: SubFeature::Inventory,
    title: "Inventory",
    description: "Track assets across stations, structures, and hangars.",
  },
  Catalog {
    sub: SubFeature::Abyssals,
    title: "Abyssals",
    description: "Appraise abyssal modules against MutaMarket pricing.",
  },
  Catalog {
    sub: SubFeature::Stockpiles,
    title: "Stockpiles",
    description: "Watch curated stockpiles against target quantities.",
  },
  Catalog {
    sub: SubFeature::Values,
    title: "Values",
    description: "Value owned assets at market pricing.",
  },
  Catalog {
    sub: SubFeature::Tracker,
    title: "Net Worth Tracker",
    description: "Chart net worth over time across every owner.",
  },
];

#[derive(Clone, Debug)]
pub enum Message {
  GroupToggled(Group, bool),
  SearchChanged(String),
  SubToggled(SubFeature, bool),
  // Cascade a single top-level config Feature on or off. The Features tab itself no longer renders a
  // per-Feature master (the display now groups by `Group`), but other surfaces — the rail context
  // menus and reset flows in `app.rs` — drive feature enablement through this same message.
  Toggled(Feature, bool),
}

#[derive(Debug, Default)]
pub struct State {
  query: String,
}

impl State {
  pub fn from_settings(_settings: &Settings) -> Self {
    State::default()
  }
}

struct Catalog {
  description: &'static str,
  sub: SubFeature,
  title: &'static str,
}

/// A display-only grouping of sub-features in the Features tab. These four groups exist purely for
/// presentation: each renders a Plasma-blue master-toggle header over its child rows. They do NOT
/// mirror the [`config::Feature`](crate::config::Feature) model one-to-one — the Characters group
/// folds several standalone top-level Features (Skill Queue, Mail, Calendar, and the
/// character-status features) under a single header, and its master cascades over all of them at
/// the display layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Group {
  Characters,
  Industry,
  Wallet,
  Assets,
}

impl Group {
  const ALL: [Group; 4] = [Group::Characters, Group::Industry, Group::Wallet, Group::Assets];

  fn title(self) -> &'static str {
    match self {
      Group::Assets => "Assets",
      Group::Characters => "Characters",
      Group::Industry => "Industry",
      Group::Wallet => "Wallet",
    }
  }

  /// The sub-features displayed under this group, in render order. Every [`SubFeature`] appears under
  /// exactly one group (asserted in tests), so the four groups partition the catalog.
  fn sub_features(self) -> &'static [SubFeature] {
    match self {
      Group::Characters => &[
        SubFeature::LocationTracking,
        SubFeature::SkillQueue,
        SubFeature::CloneMonitoring,
        SubFeature::Contacts,
        SubFeature::KillLog,
        SubFeature::Notifications,
        SubFeature::Standings,
        SubFeature::Mail,
        SubFeature::Calendar,
      ],
      Group::Industry => &[
        SubFeature::JobMonitoring,
        SubFeature::Blueprints,
        SubFeature::Planner,
        SubFeature::Extractions,
      ],
      Group::Wallet => &[
        SubFeature::Wallets,
        SubFeature::Transactions,
        SubFeature::Contracts,
        SubFeature::Journal,
        SubFeature::Budget,
      ],
      Group::Assets => &[
        SubFeature::Inventory,
        SubFeature::Abyssals,
        SubFeature::Stockpiles,
        SubFeature::Values,
        SubFeature::Tracker,
      ],
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GroupState {
  Empty,
  Full,
  Partial,
}

impl GroupState {
  fn of(group: Group, settings: &Settings) -> GroupState {
    let flags = settings.features();
    let subs = group.sub_features();
    let enabled = subs.iter().filter(|&&sub| flags.is_sub_enabled(sub)).count();

    if enabled == 0 {
      GroupState::Empty
    } else if enabled == subs.len() {
      GroupState::Full
    } else {
      GroupState::Partial
    }
  }

  fn is_on(self) -> bool {
    !matches!(self, GroupState::Empty)
  }
}

fn entry(sub: SubFeature) -> &'static Catalog {
  CATALOG
    .iter()
    .find(|entry| entry.sub == sub)
    .expect("every sub-feature is listed in the catalog")
}

fn matches(entry: &Catalog, query: &str) -> bool {
  let query = query.trim().to_lowercase();
  if query.is_empty() {
    return true;
  }
  entry.title.to_lowercase().contains(&query) || entry.description.to_lowercase().contains(&query)
}

fn group_matches(group: Group, query: &str) -> bool {
  let trimmed = query.trim().to_lowercase();
  if trimmed.is_empty() {
    return true;
  }
  group.title().to_lowercase().contains(&trimmed)
}

pub fn update(state: &mut State, message: Message, settings: &mut Settings) -> Outcome {
  match message {
    Message::GroupToggled(group, value) => {
      // The display group can span several standalone top-level Features (Characters folds in Skill
      // Queue, Mail, and Calendar), so cascade at the sub-feature level over exactly the displayed
      // children. Render order keeps Budget last, after Journal, so its coupling stays satisfied when
      // turning the whole group on.
      for &sub in group.sub_features() {
        settings.features_mut().set_sub_enabled(sub, value);
      }
      Outcome::Persist
    }
    Message::SearchChanged(query) => {
      state.query = query;
      Outcome::None
    }
    Message::SubToggled(sub, value) => {
      settings.features_mut().set_sub_enabled(sub, value);
      Outcome::Persist
    }
    Message::Toggled(feature, value) => {
      settings.features_mut().set_enabled(feature, value);
      Outcome::Persist
    }
  }
}

pub fn badge(settings: &Settings) -> String {
  let total = SubFeature::ALL.len();
  let on = settings.features().enabled_sub_features().len();
  format!("{on}/{total}")
}

pub fn view<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let header = panel_header(state, settings);
  let list = feature_list(state, settings);

  Column::with_children(vec![header, list])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn panel_header<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let title = text("Features")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(PANEL_BLURB)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![title.into(), blurb.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let count = container(
    text(badge(settings))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(Padding {
    top: spacing::UNIT / 2.0,
    right: spacing::SPACE_2,
    bottom: spacing::UNIT / 2.0,
    left: spacing::SPACE_2,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  });

  let top = Row::with_children(vec![identity.into(), count.into()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3_5);

  let column = Column::with_children(vec![top.into(), search_well(state)])
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill);

  let band = container(column).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_3_5,
    left: PANEL_SIDE_PADDING,
  });

  Column::with_children(vec![band.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn search_well(state: &State) -> Element<'_, Message> {
  let input = TextInput::new("Filter features\u{2026}", &state.query, Message::SearchChanged)
    .leading_icon(Icon::search())
    .background(color::surface::SUNKEN)
    .font_size(typography::size::MD)
    .render();

  container(input).width(Length::Fill).max_width(SEARCH_MAX_WIDTH).into()
}

fn feature_list<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let groups: Vec<Element<'a, Message>> = Group::ALL
    .into_iter()
    .filter_map(|group| group_block(group, state, settings))
    .collect();

  let body: Element<'a, Message> = if groups.is_empty() {
    no_matches(state.query.trim())
  } else {
    Column::with_children(groups).width(Length::Fill).into()
  };

  let inner = container(body).width(Length::Fill).padding(Padding {
    top: 0.0,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_6,
    left: PANEL_SIDE_PADDING,
  });

  scrollable(inner)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn group_block<'a>(group: Group, state: &'a State, settings: &'a Settings) -> Option<Element<'a, Message>> {
  let query = state.query.as_str();
  let group_matched = group_matches(group, query);

  let children: Vec<&'static Catalog> = group
    .sub_features()
    .iter()
    .map(|&sub| entry(sub))
    .filter(|entry| group_matched || matches(entry, query))
    .collect();

  if children.is_empty() {
    return None;
  }

  let state = GroupState::of(group, settings);
  let master = master_row(group, state);

  let mut rows: Vec<Element<'a, Message>> = vec![master];
  for entry in children {
    rows.push(child_row(entry, settings));
  }

  Some(Column::with_children(rows).width(Length::Fill).into())
}

fn master_row<'a>(group: Group, state: GroupState) -> Element<'a, Message> {
  let on = state.is_on();
  // Plasma-blue master-toggle header: the group title carries the accent so each of the four groups
  // reads as a section header that also toggles its whole displayed child list on or off.
  let title = text(group.title())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::accent::PLASMA));
  let status = text(master_status(state))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::secondary()));
  let labels = Column::with_children(vec![title.into(), status.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let row = Row::with_children(vec![
    labels.into(),
    toggle::toggle(on, Message::GroupToggled(group, !on)),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_6);

  let cell = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: spacing::UNIT,
    bottom: spacing::SPACE_3_5,
    left: spacing::UNIT,
  });

  Column::with_children(vec![cell.into(), rule::horizontal_alpha(0.18)])
    .width(Length::Fill)
    .into()
}

fn master_status(group: GroupState) -> &'static str {
  match group {
    GroupState::Empty => "Off",
    GroupState::Full => "On",
    GroupState::Partial => "Some on",
  }
}

fn child_row<'a>(entry: &'a Catalog, settings: &'a Settings) -> Element<'a, Message> {
  let on = settings.features().is_sub_enabled(entry.sub);

  let title = text(entry.title)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let description = text(entry.description)
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let labels = Column::with_children(vec![
    title.into(),
    container(description).max_width(DESCRIPTION_MAX_WIDTH).into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  let row = Row::with_children(vec![
    labels.into(),
    toggle::toggle(on, Message::SubToggled(entry.sub, !on)),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_6);

  let cell = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::UNIT,
    bottom: spacing::SPACE_3,
    left: CHILD_INDENT,
  });

  Column::with_children(vec![cell.into(), rule::horizontal_alpha(0.08)])
    .width(Length::Fill)
    .into()
}

fn no_matches(query: &str) -> Element<'_, Message> {
  empty_state::empty_state("No features match this search.")
    .subtitle(query)
    .render()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn state() -> State {
    State::from_settings(&Settings::default())
  }

  mod badge {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reports_enabled_sub_features_over_the_total() {
      let settings = Settings::default();

      assert_eq!(
        badge(&settings),
        format!("{}/{}", SubFeature::ALL.len(), SubFeature::ALL.len())
      );
    }

    #[test]
    fn it_drops_the_count_when_a_child_is_disabled() {
      let mut settings = Settings::default();
      settings.features_mut().set_sub_enabled(SubFeature::Budget, false);

      assert_eq!(
        badge(&settings),
        format!("{}/{}", SubFeature::ALL.len() - 1, SubFeature::ALL.len())
      );
    }
  }

  mod catalog {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_covers_every_sub_feature_exactly_once() {
      for sub in SubFeature::ALL {
        let count = CATALOG.iter().filter(|entry| entry.sub == sub).count();
        assert_eq!(count, 1, "{sub:?} should appear exactly once in the catalog");
      }

      assert_eq!(CATALOG.len(), SubFeature::ALL.len());
    }

    #[test]
    fn it_matches_the_empty_query_for_every_entry() {
      for entry in &CATALOG {
        assert!(matches(entry, ""), "{:?} should match the empty query", entry.sub);
      }
    }
  }

  mod group_state {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_full_when_every_child_is_enabled() {
      let settings = Settings::default();

      assert_eq!(GroupState::of(Group::Wallet, &settings), GroupState::Full);
    }

    #[test]
    fn it_is_partial_when_only_some_children_are_enabled() {
      let mut settings = Settings::default();
      settings.features_mut().set_sub_enabled(SubFeature::Budget, false);

      assert_eq!(GroupState::of(Group::Wallet, &settings), GroupState::Partial);
    }

    #[test]
    fn it_is_empty_when_every_child_is_disabled() {
      let mut settings = Settings::default();
      for &sub in Group::Wallet.sub_features() {
        settings.features_mut().set_sub_enabled(sub, false);
      }

      assert_eq!(GroupState::of(Group::Wallet, &settings), GroupState::Empty);
    }

    #[test]
    fn it_reflects_state_across_a_display_group_that_spans_several_features() {
      // The Characters group folds in Skill Queue, Mail, and Calendar, which are standalone top-level
      // Features in the model. Its state must roll up across all of them, not just one Feature.
      let mut settings = Settings::default();
      assert_eq!(GroupState::of(Group::Characters, &settings), GroupState::Full);

      settings.features_mut().set_sub_enabled(SubFeature::Mail, false);
      assert_eq!(
        GroupState::of(Group::Characters, &settings),
        GroupState::Partial,
        "disabling a folded Mail child moves the Characters master to partial"
      );

      for &sub in Group::Characters.sub_features() {
        settings.features_mut().set_sub_enabled(sub, false);
      }
      assert_eq!(GroupState::of(Group::Characters, &settings), GroupState::Empty);
    }

    #[test]
    fn its_on_state_is_a_simple_any_child_predicate() {
      let mut settings = Settings::default();

      assert!(GroupState::of(Group::Assets, &settings).is_on());

      for &sub in Group::Assets.sub_features() {
        settings.features_mut().set_sub_enabled(sub, false);
      }
      assert!(!GroupState::of(Group::Assets, &settings).is_on());
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_updates_the_query_without_persisting() {
      let mut state = state();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::SearchChanged("wallet".to_owned()), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(state.query, "wallet");
      assert_eq!(
        settings.features(),
        &Settings::default().features().to_owned(),
        "a search edit must not touch the flags"
      );
    }

    #[test]
    fn a_master_toggle_off_cascades_to_every_displayed_child_and_persists() {
      let mut state = state();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::GroupToggled(Group::Wallet, false), &mut settings);

      assert_eq!(outcome, Outcome::Persist);
      assert!(
        Group::Wallet
          .sub_features()
          .iter()
          .all(|&sub| !settings.features().is_sub_enabled(sub)),
        "a master toggle off clears every displayed child"
      );
      assert!(
        Group::Assets
          .sub_features()
          .iter()
          .all(|&sub| settings.features().is_sub_enabled(sub)),
        "other groups are untouched"
      );
    }

    #[test]
    fn a_master_toggle_on_cascades_to_every_displayed_child_and_persists() {
      let mut state = state();
      let mut settings = Settings::default();
      for &sub in Group::Industry.sub_features() {
        settings.features_mut().set_sub_enabled(sub, false);
      }

      let outcome = update(&mut state, Message::GroupToggled(Group::Industry, true), &mut settings);

      assert_eq!(outcome, Outcome::Persist);
      assert!(
        Group::Industry
          .sub_features()
          .iter()
          .all(|&sub| settings.features().is_sub_enabled(sub)),
        "a master toggle on enables every displayed child"
      );
    }

    #[test]
    fn the_characters_master_cascades_to_mail_calendar_and_skill_queue() {
      // Mail, Calendar, and Skill Queue are standalone top-level Features in the model but render
      // under the Characters group; the master must cascade to all of them at the display layer.
      let mut state = state();
      let mut settings = Settings::default();

      let outcome = update(
        &mut state,
        Message::GroupToggled(Group::Characters, false),
        &mut settings,
      );

      assert_eq!(outcome, Outcome::Persist);
      for sub in [
        SubFeature::Mail,
        SubFeature::Calendar,
        SubFeature::SkillQueue,
        SubFeature::LocationTracking,
        SubFeature::Standings,
      ] {
        assert!(
          !settings.features().is_sub_enabled(sub),
          "{sub:?} must follow the Characters master off"
        );
      }

      let outcome = update(
        &mut state,
        Message::GroupToggled(Group::Characters, true),
        &mut settings,
      );
      assert_eq!(outcome, Outcome::Persist);
      for sub in [SubFeature::Mail, SubFeature::Calendar, SubFeature::SkillQueue] {
        assert!(
          settings.features().is_sub_enabled(sub),
          "{sub:?} must follow the Characters master on"
        );
      }
    }

    #[test]
    fn enabling_the_wallet_master_satisfies_budgets_coupling() {
      // Render order keeps Budget last, after Journal, so a whole-group enable leaves Budget on.
      let mut state = state();
      let mut settings = Settings::default();
      for &sub in Group::Wallet.sub_features() {
        settings.features_mut().set_sub_enabled(sub, false);
      }

      update(&mut state, Message::GroupToggled(Group::Wallet, true), &mut settings);

      assert!(
        settings.features().is_sub_enabled(SubFeature::Budget),
        "Budget stays on because Journal is enabled before it"
      );
    }

    #[test]
    fn a_child_toggle_flips_only_that_child_and_updates_the_master_state() {
      let mut state = state();
      let mut settings = Settings::default();

      assert_eq!(GroupState::of(Group::Wallet, &settings), GroupState::Full);

      let outcome = update(
        &mut state,
        Message::SubToggled(SubFeature::Budget, false),
        &mut settings,
      );

      assert_eq!(outcome, Outcome::Persist);
      assert!(!settings.features().is_sub_enabled(SubFeature::Budget));
      assert!(
        settings.features().is_sub_enabled(SubFeature::Journal),
        "siblings are untouched"
      );
      assert_eq!(
        GroupState::of(Group::Wallet, &settings),
        GroupState::Partial,
        "disabling one child moves the master to a partial state"
      );
    }

    #[test]
    fn disabling_the_last_child_empties_the_master() {
      let mut state = state();
      let mut settings = Settings::default();

      let subs = Group::Assets.sub_features();
      for &sub in &subs[..subs.len() - 1] {
        settings.features_mut().set_sub_enabled(sub, false);
      }
      let last = *subs.last().unwrap();

      update(&mut state, Message::SubToggled(last, false), &mut settings);

      assert_eq!(GroupState::of(Group::Assets, &settings), GroupState::Empty);
    }
  }

  mod search {
    use super::*;

    #[test]
    fn it_matches_a_word_only_in_the_description() {
      let clones = entry(SubFeature::CloneMonitoring);

      assert!(matches(clones, "implants"));
      assert!(!clones.title.to_lowercase().contains("implants"));
    }

    #[test]
    fn it_matches_a_group_title() {
      assert!(group_matches(Group::Wallet, "wallet"));
      assert!(group_matches(Group::Assets, "ASSET"));
      assert!(group_matches(Group::Characters, "character"));
      assert!(!group_matches(Group::Wallet, "no-such-group"));
    }

    #[test]
    fn it_matches_a_child_title_case_insensitively() {
      let budget = entry(SubFeature::Budget);

      assert!(matches(budget, "BUDGET"));
      assert!(matches(budget, "bud"));
      assert!(!matches(budget, "no-such-feature"));
    }
  }

  mod group {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn the_four_display_groups_partition_every_sub_feature_exactly_once() {
      assert_eq!(Group::ALL.len(), 4, "exactly four display groups");

      for sub in SubFeature::ALL {
        let count = Group::ALL
          .into_iter()
          .filter(|group| group.sub_features().contains(&sub))
          .count();
        assert_eq!(count, 1, "{sub:?} must belong to exactly one display group");
      }

      let total: usize = Group::ALL.into_iter().map(|group| group.sub_features().len()).sum();
      assert_eq!(total, SubFeature::ALL.len(), "the groups cover the whole catalog");
    }

    #[test]
    fn the_characters_group_folds_in_mail_calendar_and_skill_queue() {
      let chars = Group::Characters.sub_features();
      for sub in [
        SubFeature::LocationTracking,
        SubFeature::SkillQueue,
        SubFeature::CloneMonitoring,
        SubFeature::Contacts,
        SubFeature::KillLog,
        SubFeature::Notifications,
        SubFeature::Standings,
        SubFeature::Mail,
        SubFeature::Calendar,
      ] {
        assert!(chars.contains(&sub), "{sub:?} must render under Characters");
      }
      assert_eq!(chars.len(), 9, "Characters shows exactly its nine children");
    }

    #[test]
    fn every_group_title_is_one_of_the_four_expected_labels() {
      let titles: Vec<&str> = Group::ALL.into_iter().map(Group::title).collect();
      assert_eq!(titles, vec!["Characters", "Industry", "Wallet", "Assets"]);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_with_an_empty_query() {
      let settings = Settings::default();
      let state = state();

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_the_empty_state_for_an_unmatched_query() {
      let settings = Settings::default();
      let mut state = state();
      state.query = "zzz-no-match".to_owned();

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_with_a_group_fully_disabled() {
      let mut settings = Settings::default();
      for &sub in Group::Wallet.sub_features() {
        settings.features_mut().set_sub_enabled(sub, false);
      }
      let state = state();

      let _el: Element<'_, Message> = view(&state, &settings);
    }
  }

  #[test]
  fn the_panel_blurb_describes_live_behavior_not_a_restart() {
    assert!(
      PANEL_BLURB.contains("live") && PANEL_BLURB.contains("no restart"),
      "the features blurb must reflect the live toggle behavior"
    );
    assert!(
      !PANEL_BLURB.contains("next restart"),
      "the stale 'applies on next restart' copy must be gone"
    );
  }
}
