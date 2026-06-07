use iced::{
  Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, container, scrollable, text},
};

use super::Outcome;
use crate::{
  config::{Feature, Settings},
  ui::{
    components::{empty_state, icon::Icon, rule, text_input::TextInput, toggle},
    style::{color, radius, spacing, typography},
  },
};

const DESCRIPTION_MAX_WIDTH: f32 = 560.0;
const PANEL_SIDE_PADDING: f32 = 36.0;
const SEARCH_MAX_WIDTH: f32 = 480.0;

const CATALOG: [Catalog; 10] = [
  Catalog {
    feature: Feature::CloneMonitoring,
    section: Section::Character,
    title: "Clone Monitoring",
    description: "Track jump clones and the implants installed in each.",
  },
  Catalog {
    feature: Feature::Contacts,
    section: Section::Character,
    title: "Contacts",
    description: "Sync your personal contact list and their standings.",
  },
  Catalog {
    feature: Feature::CombatLog,
    section: Section::Character,
    title: "Combat Log",
    description: "Capture combat activity for after-action review.",
  },
  Catalog {
    feature: Feature::EveNotifications,
    section: Section::Character,
    title: "EVE Notifications",
    description: "Sync in-game notifications from EVE Online.",
  },
  Catalog {
    feature: Feature::Standings,
    section: Section::Character,
    title: "Standings",
    description: "Sync your standings toward characters, corporations, and alliances.",
  },
  Catalog {
    feature: Feature::LocationTracking,
    section: Section::World,
    title: "Location Tracking",
    description: "Track each character's current solar system, station, and ship.",
  },
  Catalog {
    feature: Feature::SkillMonitoring,
    section: Section::World,
    title: "Skill Monitoring",
    description: "Monitor trained skills and the active training queue.",
  },
  Catalog {
    feature: Feature::Mail,
    section: Section::World,
    title: "Mail",
    description: "Sync EVE mail headers and message bodies.",
  },
  Catalog {
    feature: Feature::Wallet,
    section: Section::World,
    title: "Wallet",
    description: "Sync wallet balances and the transaction journal.",
  },
  Catalog {
    feature: Feature::AssetTracking,
    section: Section::World,
    title: "Asset Tracking",
    description: "Track assets across stations, structures, and hangars.",
  },
];

#[derive(Clone, Debug)]
pub enum Message {
  SearchChanged(String),
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
  feature: Feature,
  section: Section,
  title: &'static str,
}

#[derive(Clone, Copy, Debug)]
enum Section {
  Character,
  World,
}

impl Section {
  fn label(self) -> &'static str {
    match self {
      Section::Character => "Character",
      Section::World => "World",
    }
  }
}

fn matches(entry: &Catalog, query: &str) -> bool {
  let query = query.trim().to_lowercase();
  if query.is_empty() {
    return true;
  }
  entry.title.to_lowercase().contains(&query) || entry.description.to_lowercase().contains(&query)
}

pub fn update(state: &mut State, message: Message, settings: &mut Settings) -> Outcome {
  match message {
    Message::SearchChanged(query) => {
      state.query = query;
      Outcome::None
    }
    Message::Toggled(feature, value) => {
      settings.features_mut().set_enabled(feature, value);
      Outcome::Persist
    }
  }
}

pub fn badge(settings: &Settings) -> String {
  let total = Feature::ALL.len();
  let on = settings.features().enabled().len();
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
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let blurb = text(
    "Toggle individual Pod capabilities on or off. Changes apply on the next restart and across \
      your linked characters.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::MD)
  .style(|_| text::Style {
    color: Some(color::text::SECONDARY),
  });
  let identity = Column::with_children(vec![title.into(), blurb.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let total = Feature::ALL.len();
  let on = settings.features().enabled().len();
  let count = container(
    text(format!("{on}/{total}"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      }),
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
  let mut sections: Vec<Element<'a, Message>> = Vec::new();
  for section in [Section::Character, Section::World] {
    if let Some(rendered) = section_block(section, state, settings) {
      sections.push(rendered);
    }
  }

  let body: Element<'a, Message> = if sections.is_empty() {
    no_matches(state.query.trim())
  } else {
    Column::with_children(sections).width(Length::Fill).into()
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

fn section_block<'a>(section: Section, state: &'a State, settings: &'a Settings) -> Option<Element<'a, Message>> {
  let rows: Vec<Element<'a, Message>> = CATALOG
    .iter()
    .filter(|entry| entry_in_section(entry, section))
    .filter(|entry| matches(entry, &state.query))
    .map(|entry| feature_row(entry, settings))
    .collect();

  if rows.is_empty() {
    return None;
  }

  let header = container(
    text(section.label())
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6,
    right: 0.0,
    bottom: spacing::SPACE_3,
    left: 0.0,
  });

  let mut children: Vec<Element<'a, Message>> = vec![header.into(), rule::horizontal_alpha(0.18)];
  children.extend(rows);

  Some(Column::with_children(children).width(Length::Fill).into())
}

fn entry_in_section(entry: &Catalog, section: Section) -> bool {
  matches!(
    (entry.section, section),
    (Section::Character, Section::Character) | (Section::World, Section::World)
  )
}

fn feature_row<'a>(entry: &'a Catalog, settings: &'a Settings) -> Element<'a, Message> {
  let on = settings.features().is_enabled(entry.feature);

  let title = text(entry.title)
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let description = text(entry.description)
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::text::SECONDARY),
    });
  let labels = Column::with_children(vec![
    title.into(),
    container(description).max_width(DESCRIPTION_MAX_WIDTH).into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  let row = Row::with_children(vec![
    labels.into(),
    toggle::toggle(on, Message::Toggled(entry.feature, !on)),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_6);

  let cell = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3_5,
    right: spacing::UNIT,
    bottom: spacing::SPACE_3_5,
    left: spacing::UNIT,
  });

  Column::with_children(vec![cell.into(), rule::horizontal()])
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
  use pretty_assertions::assert_eq;

  use super::*;

  fn state() -> State {
    State::from_settings(&Settings::default())
  }

  #[test]
  fn badge_reports_enabled_over_total() {
    let settings = Settings::default();

    assert_eq!(
      badge(&settings),
      format!("{}/{}", Feature::ALL.len(), Feature::ALL.len())
    );
  }

  #[test]
  fn catalog_covers_every_capability_once() {
    for feature in Feature::ALL {
      let count = CATALOG.iter().filter(|entry| entry.feature == feature).count();
      assert_eq!(count, 1, "{feature:?} should appear exactly once in the catalog");
    }
    assert_eq!(CATALOG.len(), Feature::ALL.len());
  }

  #[test]
  fn the_character_capabilities_are_grouped_under_character() {
    let character: Vec<Feature> = CATALOG
      .iter()
      .filter(|entry| entry_in_section(entry, Section::Character))
      .map(|entry| entry.feature)
      .collect();

    assert_eq!(
      character,
      vec![
        Feature::CloneMonitoring,
        Feature::Contacts,
        Feature::CombatLog,
        Feature::EveNotifications,
        Feature::Standings,
      ]
    );
  }

  #[test]
  fn the_five_world_capabilities_are_grouped_under_world() {
    let world: Vec<Feature> = CATALOG
      .iter()
      .filter(|entry| entry_in_section(entry, Section::World))
      .map(|entry| entry.feature)
      .collect();

    assert_eq!(
      world,
      vec![
        Feature::LocationTracking,
        Feature::SkillMonitoring,
        Feature::Mail,
        Feature::Wallet,
        Feature::AssetTracking,
      ]
    );
  }

  #[test]
  fn toggling_a_flag_off_flips_only_that_capability_and_persists() {
    let mut state = state();
    let mut settings = Settings::default();

    let outcome = update(&mut state, Message::Toggled(Feature::Wallet, false), &mut settings);

    assert_eq!(outcome, Outcome::Persist);
    assert!(!settings.features().is_enabled(Feature::Wallet));
    assert!(
      settings.features().is_enabled(Feature::Mail),
      "other capabilities are untouched"
    );
  }

  #[test]
  fn toggling_a_flag_back_on_persists() {
    let mut state = state();
    let mut settings = Settings::default();
    settings.features_mut().set_enabled(Feature::Mail, false);

    let outcome = update(&mut state, Message::Toggled(Feature::Mail, true), &mut settings);

    assert_eq!(outcome, Outcome::Persist);
    assert!(settings.features().is_enabled(Feature::Mail));
  }

  #[test]
  fn editing_the_search_query_updates_state_without_persisting() {
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
  fn search_matches_title_case_insensitively() {
    let wallet = CATALOG.iter().find(|entry| entry.feature == Feature::Wallet).unwrap();

    assert!(matches(wallet, "WALLET"));
    assert!(matches(wallet, "wal"));
    assert!(!matches(wallet, "no-such-feature"));
  }

  #[test]
  fn an_empty_query_matches_every_capability() {
    for entry in &CATALOG {
      assert!(matches(entry, ""), "{:?} should match the empty query", entry.feature);
    }
  }

  #[test]
  fn search_matches_a_word_only_in_the_description() {
    let clones = CATALOG
      .iter()
      .find(|entry| entry.feature == Feature::CloneMonitoring)
      .unwrap();

    assert!(matches(clones, "implants"));
    assert!(!clones.title.to_lowercase().contains("implants"));
  }

  #[test]
  fn view_renders_with_an_empty_query() {
    let settings = Settings::default();
    let state = state();

    let _el: Element<'_, Message> = view(&state, &settings);
  }

  #[test]
  fn view_renders_the_empty_state_for_an_unmatched_query() {
    let settings = Settings::default();
    let mut state = state();
    state.query = "zzz-no-match".to_owned();

    let _el: Element<'_, Message> = view(&state, &settings);
  }
}
