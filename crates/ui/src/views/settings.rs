//! Settings view: feature-flag toggles and preferences.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Space, button, column, container, row, scrollable, text, text_input},
};

use crate::style::{color, radius, spacing};

/// A single toggleable feature flag.
#[derive(Clone, Debug, PartialEq, Eq)]
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

/// Messages produced by the settings view.
#[derive(Clone, Debug)]
pub enum Message {
  ResetDefaults,
  SearchChanged(String),
  ToggleFeature(Feature),
}

/// Runtime state for the settings view.
pub struct State {
  pub asset_tracking: bool,
  pub clone_monitoring: bool,
  pub combat_log: bool,
  pub contacts: bool,
  pub eve_notifications: bool,
  pub location_tracking: bool,
  pub mail: bool,
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

/// Builder for the settings view.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Create a new settings view builder for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;

    let header = render_header();
    let categories = render_categories_pane(state);
    let features = render_features_panel(state);

    let body: Element<'_, Message> = row([categories, features])
      .width(Length::Fill)
      .height(Length::Fill)
      .into();

    container(column([header, body]).width(Length::Fill).height(Length::Fill))
      .width(Length::Fill)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::BASE)),
        ..container::Style::default()
      })
      .into()
  }
}

fn render_header() -> Element<'static, Message> {
  let eyebrow = text("Pod · Preferences").size(9.0).color(color::text::SECONDARY);
  let title = text("Settings").size(22.0).color(color::text::PRIMARY);
  let left_col: Element<'_, Message> = column([eyebrow.into(), Space::new().height(6.0).into(), title.into()]).into();

  let reset_icon = crate::components::Icon::settings()
    .size(14.0)
    .color(color::text::SECONDARY)
    .render::<Message>();

  let reset_btn = crate::components::Button::ghost(
    row([
      reset_icon,
      text("Reset to defaults")
        .size(13.0)
        .color(color::text::SECONDARY)
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .on_press(Message::ResetDefaults);

  let header_row: Element<'_, Message> = row([left_col, Space::new().width(Length::Fill).into(), reset_btn.into()])
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 20.0,
      bottom: 20.0,
      left: spacing::SPACE_7,
      right: spacing::SPACE_7,
    })
    .into();

  let border = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });

  column([header_row, border.into()])
    .height(spacing::layout::HEADER_HEIGHT)
    .into()
}

fn render_categories_pane(state: &State) -> Element<'_, Message> {
  let enabled = state.enabled_count();
  let total = State::total_count();

  let label = text("Categories").size(9.0).color(color::text::SECONDARY);
  let count_badge = text(format!("{enabled}/{total}"))
    .size(10.0)
    .color(color::accent::PLASMA);

  let active_indicator: Element<'_, Message> =
    container(
      container(Space::new())
        .width(2.0)
        .height(24.0)
        .style(|_| container::Style {
          background: Some(Background::Color(color::accent::PLASMA)),
          border: Border {
            radius: Radius {
              top_left: 0.0,
              top_right: radius::SUBTLE,
              bottom_right: radius::SUBTLE,
              bottom_left: 0.0,
            },
            ..Border::default()
          },
          ..container::Style::default()
        }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Left)
    .align_y(Vertical::Center)
    .into();

  let features_row: Element<'_, Message> = container(
    iced::widget::stack([
      container(
        row([
          text("Features").size(13.0).color(color::text::PRIMARY).into(),
          Space::new().width(Length::Fill).into(),
          count_badge.into(),
        ])
        .align_y(Vertical::Center)
        .padding(Padding {
          top: 10.0,
          bottom: 10.0,
          left: spacing::SPACE_3,
          right: spacing::SPACE_3,
        }),
      )
      .width(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA_SUBTLE)),
        border: Border {
          radius: radius::CHIP.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
      active_indicator,
    ])
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .into();

  let categories_col: Element<'_, Message> = column([
    container(label)
      .padding(Padding {
        top: 18.0,
        bottom: 10.0,
        left: spacing::SPACE_1 + 2.0,
        right: 0.0,
      })
      .into(),
    features_row,
  ])
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: spacing::SPACE_3_5,
    right: spacing::SPACE_3_5,
  })
  .into();

  let right_border = container(Space::new().width(1.0).height(Length::Fill))
    .width(1.0)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });

  row([
    container(categories_col)
      .width(220.0)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        ..container::Style::default()
      })
      .into(),
    right_border.into(),
  ])
  .into()
}

fn render_features_panel(state: &State) -> Element<'_, Message> {
  let panel_title = text("Features").size(18.0).color(color::text::PRIMARY);

  let panel_desc = text(
    "Toggle individual Pod capabilities on or off. Changes apply \
    immediately and sync across your linked characters; reload any \
    view to see the result.",
  )
  .size(13.0)
  .color(color::text::SECONDARY);

  let flags = build_visible_flags(state);
  let total_shown = flags.len();

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

  let search_row: Element<'_, Message> = container(
    row([
      search_icon,
      text_input("Filter features\u{2026}", &state.search_query)
        .on_input(Message::SearchChanged)
        .size(13.0)
        .style(|_, _| text_input::Style {
          background: Background::Color(Color::TRANSPARENT),
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
  .into();

  let panel_inner_header: Element<'_, Message> = column([
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
  .into();

  let inner_header_border = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });

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

  let scrollable_body: Element<'_, Message> = scrollable(column(scroll_content).width(Length::Fill).padding(Padding {
    top: 0.0,
    bottom: 60.0,
    left: 36.0,
    right: 36.0,
  }))
  .width(Length::Fill)
  .height(Length::Fill)
  .into();

  column([panel_inner_header, inner_header_border.into(), scrollable_body])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

struct FlagData {
  feature: Feature,
  title: &'static str,
  description: &'static str,
  enabled: bool,
}

fn build_visible_flags(state: &State) -> Vec<FlagData> {
  let q = state.search_query.trim().to_lowercase();

  let all = vec![
    FlagData {
      feature: Feature::CloneMonitoring,
      title: "Clone Monitoring",
      description: "Sync jump-clone locations and active-clone implants",
      enabled: state.clone_monitoring,
    },
    FlagData {
      feature: Feature::Contacts,
      title: "Contacts",
      description: "Read character contacts and contact labels",
      enabled: state.contacts,
    },
    FlagData {
      feature: Feature::CombatLog,
      title: "Combat Log",
      description: "Read recent character killmails",
      enabled: state.combat_log,
    },
    FlagData {
      feature: Feature::EveNotifications,
      title: "EVE Notifications",
      description: "Read EVE notification feed",
      enabled: state.eve_notifications,
    },
    FlagData {
      feature: Feature::Standings,
      title: "Standings",
      description: "Read character standings toward NPCs and other players",
      enabled: state.standings,
    },
    FlagData {
      feature: Feature::LocationTracking,
      title: "Location Tracking",
      description: "Poll the character\u{2019}s current solar-system location",
      enabled: state.location_tracking,
    },
    FlagData {
      feature: Feature::SkillMonitoring,
      title: "Skill Monitoring",
      description: "Sync skill levels and active skill-training queue",
      enabled: state.skill_monitoring,
    },
    FlagData {
      feature: Feature::Mail,
      title: "Mail",
      description: "Read, organise, and send EVE mail",
      enabled: state.mail,
    },
    FlagData {
      feature: Feature::Wallet,
      title: "Wallet",
      description: "Read character wallet balance, journal, and transactions",
      enabled: state.wallet,
    },
    FlagData {
      feature: Feature::AssetTracking,
      title: "Asset Tracking",
      description: "Read character assets and resolve player-owned structure names",
      enabled: state.asset_tracking,
    },
  ];

  if q.is_empty() {
    return all;
  }

  all
    .into_iter()
    .filter(|f| f.title.to_lowercase().contains(&q) || f.description.to_lowercase().contains(&q))
    .collect()
}

fn render_feature_row(flag: FlagData) -> Element<'static, Message> {
  let title = text(flag.title).size(14.0).color(color::text::PRIMARY);

  let esi_chip = container(text("ESI").size(9.0).color(color::accent::PLASMA))
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: 6.0,
      right: 6.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(Color {
        r: color::accent::PLASMA.r,
        g: color::accent::PLASMA.g,
        b: color::accent::PLASMA.b,
        a: 0.06,
      })),
      border: Border {
        color: Color {
          r: color::accent::PLASMA.r,
          g: color::accent::PLASMA.g,
          b: color::accent::PLASMA.b,
          a: 0.30,
        },
        radius: radius::CHIP.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  let description = text(flag.description).size(12.0).color(color::text::SECONDARY);
  let toggle = render_toggle(flag.enabled, flag.feature);

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
        row([title.into(), esi_chip.into()])
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

fn render_toggle(on: bool, feature: Feature) -> Element<'static, Message> {
  let thumb_color = if on {
    Color {
      r: 0.039,
      g: 0.106,
      b: 0.133,
      a: 1.0,
    }
  } else {
    Color {
      r: color::text::PRIMARY.r,
      g: color::text::PRIMARY.g,
      b: color::text::PRIMARY.b,
      a: 0.65,
    }
  };

  let bg_color = if on {
    color::accent::PLASMA
  } else {
    Color {
      r: color::text::PRIMARY.r,
      g: color::text::PRIMARY.g,
      b: color::text::PRIMARY.b,
      a: 0.08,
    }
  };

  let border_color = if on {
    color::accent::PLASMA
  } else {
    color::border::DEFAULT
  };

  let thumb_offset = if on { 17.0_f32 } else { 2.0_f32 };

  let thumb = container(Space::new())
    .width(14.0)
    .height(14.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(thumb_color)),
      border: Border {
        radius: radius::FULL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let track = container(
    container(thumb)
      .padding(Padding {
        top: 2.0,
        bottom: 2.0,
        left: thumb_offset,
        right: 0.0,
      })
      .align_y(Vertical::Center),
  )
  .width(38.0)
  .height(22.0)
  .style(move |_| container::Style {
    background: Some(Background::Color(bg_color)),
    border: Border {
      color: border_color,
      radius: radius::FULL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  button(track)
    .padding(Padding::ZERO)
    .style(|_, _| button::Style {
      background: None,
      ..button::Style::default()
    })
    .on_press(Message::ToggleFeature(feature))
    .into()
}
