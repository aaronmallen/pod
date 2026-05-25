//! Features settings panel: feature-flag toggles with search.

pub mod feature_toggle;

use feature_toggle::FeatureToggle;
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, scrollable, text, text_input},
};

use crate::style::{color, radius, spacing};

/// Builder for the features settings panel.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Create a new features panel builder for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    render_features_panel(self.state)
  }
}

/// A single toggleable feature flag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Feature {
  AssetTracking,
  CloneMonitoring,
  CombatLog,
  Contacts,
  EveNotifications,
  LocationTracking,
  Mail,
  SkillMonitoring,
  Standings,
  Wallet,
}

/// Messages produced by the features panel.
#[derive(Clone, Debug)]
pub enum Message {
  /// The feature search query changed.
  SearchChanged(String),
  /// A feature flag was toggled.
  ToggleFeature(Feature),
}

/// Runtime state for the features settings panel.
pub struct State {
  pub asset_tracking: bool,
  pub clone_monitoring: bool,
  pub combat_log: bool,
  pub contacts: bool,
  pub eve_notifications: bool,
  pub location_tracking: bool,
  pub mail: bool,
  /// Search query for the features list.
  pub search_query: String,
  pub skill_monitoring: bool,
  pub standings: bool,
  pub wallet: bool,
}

impl Default for State {
  fn default() -> Self {
    Self {
      asset_tracking: true,
      clone_monitoring: true,
      combat_log: true,
      contacts: true,
      eve_notifications: true,
      location_tracking: true,
      mail: true,
      search_query: String::new(),
      skill_monitoring: true,
      standings: true,
      wallet: true,
    }
  }
}

impl State {
  /// Count how many feature flags are currently enabled.
  pub fn enabled_count(&self) -> usize {
    [
      self.asset_tracking,
      self.clone_monitoring,
      self.combat_log,
      self.contacts,
      self.eve_notifications,
      self.location_tracking,
      self.mail,
      self.skill_monitoring,
      self.standings,
      self.wallet,
    ]
    .iter()
    .filter(|&&v| v)
    .count()
  }

  /// Total number of feature flags.
  pub const fn total_count() -> usize {
    10
  }
}

struct FlagData {
  description: &'static str,
  enabled: bool,
  feature: Feature,
  title: &'static str,
}

fn all_flags(state: &State) -> Vec<FlagData> {
  let mut flags = character_flags(state);
  flags.extend(world_flags(state));
  flags
}

fn build_visible_flags(state: &State) -> Vec<FlagData> {
  let q = state.search_query.trim().to_lowercase();
  let all = all_flags(state);
  if q.is_empty() {
    return all;
  }
  all
    .into_iter()
    .filter(|f| f.title.to_lowercase().contains(&q) || f.description.to_lowercase().contains(&q))
    .collect()
}

fn character_flags(state: &State) -> Vec<FlagData> {
  vec![
    FlagData {
      description: "Sync jump-clone locations and active-clone implants",
      enabled: state.clone_monitoring,
      feature: Feature::CloneMonitoring,
      title: "Clone Monitoring",
    },
    FlagData {
      description: "Read character contacts and contact labels",
      enabled: state.contacts,
      feature: Feature::Contacts,
      title: "Contacts",
    },
    FlagData {
      description: "Read recent character killmails",
      enabled: state.combat_log,
      feature: Feature::CombatLog,
      title: "Combat Log",
    },
    FlagData {
      description: "Read EVE notification feed",
      enabled: state.eve_notifications,
      feature: Feature::EveNotifications,
      title: "EVE Notifications",
    },
    FlagData {
      description: "Read character standings toward NPCs and other players",
      enabled: state.standings,
      feature: Feature::Standings,
      title: "Standings",
    },
  ]
}

fn feature_esi_chip() -> Element<'static, Message> {
  container(text("ESI").size(9.0).color(color::accent::PLASMA))
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: 6.0,
      right: 6.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::PLASMA_SUBTLE)),
      border: Border {
        color: color::state::SELECTION,
        radius: radius::CHIP.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn features_panel_header(state: &State, total_shown: usize) -> Element<'_, Message> {
  let panel_title = text("Features").size(18.0).color(color::text::PRIMARY);
  let panel_desc = text(
    "Toggle individual Pod capabilities on or off. Changes apply \
    immediately and sync across your linked characters; reload any \
    view to see the result.",
  )
  .size(13.0)
  .color(color::text::SECONDARY);
  let search_row = features_search_row(state, total_shown);
  column([
    row([panel_title.into(), Space::new().width(Length::Fill).into()])
      .align_y(Vertical::Center)
      .into(),
    Space::new().height(4.0).into(),
    panel_desc.into(),
    Space::new().height(spacing::SPACE_3_5).into(),
    search_row,
  ])
  .padding(Padding {
    top: 24.0,
    bottom: spacing::SPACE_3_5,
    left: 36.0,
    right: 36.0,
  })
  .into()
}

fn features_scroll_body<'a>(state: &'a State, flags: Vec<FlagData>) -> Element<'a, Message> {
  let scroll_content: Vec<Element<'_, Message>> = if flags.is_empty() {
    vec![
      container(
        text(format!("No features match \"{}\".", state.search_query))
          .size(13.0)
          .color(color::text::SECONDARY),
      )
      .width(Length::Fill)
      .padding(Padding::new(80.0))
      .into(),
    ]
  } else {
    flags.into_iter().map(render_feature_row).collect()
  };
  scrollable(column(scroll_content).width(Length::Fill).padding(Padding {
    top: 0.0,
    bottom: 60.0,
    left: 36.0,
    right: 36.0,
  }))
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn features_search_row(state: &State, total_shown: usize) -> Element<'_, Message> {
  let search_icon = crate::components::Icon::search()
    .size(14.0)
    .color(color::text::SECONDARY)
    .render::<Message>();
  let count_chip = container(text(format!("{total_shown}")).size(9.0).color(color::text::TERTIARY))
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: 6.0,
      right: 6.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        radius: radius::CHIP.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });
  container(
    row([
      search_icon,
      text_input("Filter features\u{2026}", &state.search_query)
        .on_input(Message::SearchChanged)
        .size(13.0)
        .style(|_, _| text_input::Style {
          background: Background::Color(iced::Color::TRANSPARENT),
          border: Border::default(),
          icon: color::text::SECONDARY,
          placeholder: color::text::TERTIARY,
          selection: color::accent::PLASMA_SUBTLE,
          value: color::text::PRIMARY,
        })
        .into(),
      count_chip.into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .max_width(480.0)
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      radius: radius::CHIP.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn render_feature_row(flag: FlagData) -> Element<'static, Message> {
  let title = text(flag.title).size(14.0).color(color::text::PRIMARY);
  let description = text(flag.description).size(12.0).color(color::text::SECONDARY);
  let toggle = FeatureToggle::new(flag.enabled, flag.feature).render();
  let bottom_border = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });
  column([
    row([
      column([
        row([title.into(), feature_esi_chip()])
          .spacing(10.0)
          .align_y(Vertical::Center)
          .into(),
        Space::new().height(4.0).into(),
        description.into(),
      ])
      .into(),
      Space::new().width(Length::Fill).into(),
      container(toggle).align_y(Vertical::Center).height(Length::Fill).into(),
    ])
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 16.0,
      bottom: 16.0,
      left: 4.0,
      right: 4.0,
    })
    .into(),
    bottom_border.into(),
  ])
  .into()
}

fn render_features_panel(state: &State) -> Element<'_, Message> {
  let flags = build_visible_flags(state);
  let panel_inner_header = features_panel_header(state, flags.len());
  let inner_header_border = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });
  let scrollable_body = features_scroll_body(state, flags);
  column([panel_inner_header, inner_header_border.into(), scrollable_body])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn world_flags(state: &State) -> Vec<FlagData> {
  vec![
    FlagData {
      description: "Poll the character\u{2019}s current solar-system location",
      enabled: state.location_tracking,
      feature: Feature::LocationTracking,
      title: "Location Tracking",
    },
    FlagData {
      description: "Sync skill levels and active skill-training queue",
      enabled: state.skill_monitoring,
      feature: Feature::SkillMonitoring,
      title: "Skill Monitoring",
    },
    FlagData {
      description: "Read, organise, and send EVE mail",
      enabled: state.mail,
      feature: Feature::Mail,
      title: "Mail",
    },
    FlagData {
      description: "Read character wallet balance, journal, and transactions",
      enabled: state.wallet,
      feature: Feature::Wallet,
      title: "Wallet",
    },
    FlagData {
      description: "Read character assets and resolve player-owned structure names",
      enabled: state.asset_tracking,
      feature: Feature::AssetTracking,
      title: "Asset Tracking",
    },
  ]
}
