//! Settings view: feature-flag toggles and preferences.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Space, button, column, container, row, scrollable, text, text_input},
};

use crate::style::{color, component, radius, spacing, typography};

const TAG_PALETTE: &[(&str, &str)] = &[
  ("Plasma", "#3FB8DB"),
  ("Jade", "#5BB97E"),
  ("Gold", "#D9B252"),
  ("Ember", "#E07559"),
  ("Coral", "#E08AA5"),
  ("Orchid", "#C07AD9"),
  ("Violet", "#8A8FD9"),
  ("Cyan", "#5BC9BC"),
  ("Lime", "#A8C97A"),
  ("Rust", "#C97A5B"),
  ("Slate", "#8A95A6"),
];

/// Which settings category is currently shown.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Category {
  #[default]
  Features,
  Tags,
}

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
  CategorySelected(Category),
  ResetDefaults,
  SearchChanged(String),
  TagColorClose,
  TagColorOpen(i32),
  TagColorSet(Result<(i32, String, Option<String>), String>),
  TagCreate,
  TagCreated(Result<(i32, String, Option<String>), String>),
  TagDelete(i32),
  TagDeleted(Result<i32, String>),
  TagDraftChanged(String),
  TagEditStart(i32),
  TagMoveDown(i32),
  TagMoveUp(i32),
  TagNewNameChanged(String),
  TagRename,
  TagRenamed(Result<(i32, String, Option<String>), String>),
  TagReordered(Result<(), String>),
  TagSetColor(i32, Option<String>),
  TagsLoaded(Vec<(i32, String, Option<String>)>),
  ToggleFeature(Feature),
}

/// Runtime state for the settings view.
pub struct State {
  pub active_category: Category,
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
  pub tag_color_open: Option<i32>,
  pub tag_draft: String,
  pub tag_editing: Option<i32>,
  pub tag_new_name: String,
  pub tags: Vec<(i32, String, Option<String>)>,
  pub wallet: bool,
}

impl Default for State {
  fn default() -> Self {
    Self {
      active_category: Category::default(),
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
      tag_color_open: None,
      tag_draft: String::new(),
      tag_editing: None,
      tag_new_name: String::new(),
      tags: Vec::new(),
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
    let panel = match &state.active_category {
      Category::Features => render_features_panel(state),
      Category::Tags => render_tags_panel(state),
    };
    let body: Element<'_, Message> = row([categories, panel]).width(Length::Fill).height(Length::Fill).into();
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

fn categories_active_indicator() -> Element<'static, Message> {
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
  .into()
}

fn categories_item_row(
  label: impl ToString,
  badge: Option<String>,
  is_active: bool,
  msg: Message,
) -> Element<'static, Message> {
  let label_color = if is_active {
    color::text::PRIMARY
  } else {
    color::text::SECONDARY
  };
  let badge_color = if is_active {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };

  let badge_el: Element<'static, Message> = match badge {
    Some(b) => text(b)
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(badge_color),
      })
      .into(),
    None => Space::new().into(),
  };

  let inner = container(
    row([
      text(label.to_string())
        .size(13.0)
        .style(move |_| iced::widget::text::Style {
          color: Some(label_color),
        })
        .into(),
      Space::new().width(Length::Fill).into(),
      badge_el,
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
  .style(move |_| container::Style {
    background: if is_active {
      Some(Background::Color(color::accent::PLASMA_SUBTLE))
    } else {
      None
    },
    border: Border {
      radius: radius::CHIP.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  let indicator: Element<'static, Message> = if is_active {
    categories_active_indicator()
  } else {
    Space::new().width(Length::Fill).height(Length::Fill).into()
  };

  button(iced::widget::stack([inner.into(), indicator]).width(Length::Fill))
    .padding(Padding::ZERO)
    .on_press(msg)
    .style(|_, _| button::Style::default())
    .into()
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

fn color_picker_inline(tag_id: i32, current_hex: Option<String>) -> Element<'static, Message> {
  let swatches: Vec<Element<'static, Message>> = TAG_PALETTE
    .iter()
    .map(|&(_name, hex)| {
      let Some(swatch_color) = hex_to_iced_color(hex) else {
        return Space::new().width(22.0).height(22.0).into();
      };
      let is_selected = current_hex.as_deref() == Some(hex);
      let hex_owned = hex.to_string();
      button(Space::new().width(22.0).height(22.0))
        .padding(Padding::ZERO)
        .width(22.0)
        .height(22.0)
        .on_press(Message::TagSetColor(tag_id, Some(hex_owned)))
        .style(move |_, status| button::Style {
          background: Some(Background::Color(swatch_color)),
          border: Border {
            color: if is_selected {
              color::accent::PLASMA
            } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
              Color {
                a: 0.8,
                ..swatch_color
              }
            } else {
              Color {
                a: 0.5,
                ..swatch_color
              }
            },
            radius: radius::CHIP.into(),
            width: if is_selected { 2.0 } else { 1.0 },
          },
          shadow: if is_selected {
            iced::Shadow {
              color: Color {
                a: 0.3,
                ..color::accent::PLASMA
              },
              offset: iced::Vector::ZERO,
              blur_radius: 4.0,
            }
          } else {
            iced::Shadow::default()
          },
          snap: false,
          text_color: Color::TRANSPARENT,
        })
        .into()
    })
    .collect();

  let clear_btn = button(
    text("Clear color")
      .font(typography::body::REGULAR)
      .size(12.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::TagSetColor(tag_id, None))
  .style(|_, status| button::Style {
    background: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
      Some(Background::Color(color::state::HOVER_OVERLAY))
    } else {
      None
    },
    border: Border {
      color: color::border::SUBTLE,
      radius: radius::CHIP.into(),
      width: 1.0,
    },
    snap: false,
    text_color: color::text::SECONDARY,
    shadow: iced::Shadow::default(),
  });

  container(
    column([
      row(swatches).spacing(6.0).wrap().into(),
      Space::new().height(10.0).into(),
      clear_btn.into(),
    ])
    .spacing(0.0),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 10.0,
    bottom: 12.0,
    left: 58.0,
    right: spacing::SPACE_4,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  })
  .into()
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
  .into()
}

fn hex_to_iced_color(hex: &str) -> Option<Color> {
  let hex = hex.trim_start_matches('#');
  if hex.len() != 6 {
    return None;
  }
  let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
  let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
  let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
  Some(Color {
    r,
    g,
    b,
    a: 1.0,
  })
}

fn render_categories_pane(state: &State) -> Element<'_, Message> {
  let enabled = state.enabled_count();
  let total = State::total_count();
  let label = text("Categories").size(9.0).color(color::text::SECONDARY);

  let features_row = categories_item_row(
    "Features",
    Some(format!("{enabled}/{total}")),
    state.active_category == Category::Features,
    Message::CategorySelected(Category::Features),
  );
  let tags_row = categories_item_row(
    "Tags",
    Some(state.tags.len().to_string()),
    state.active_category == Category::Tags,
    Message::CategorySelected(Category::Tags),
  );

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
    tags_row,
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

fn render_feature_row(flag: FlagData) -> Element<'static, Message> {
  let title = text(flag.title).size(14.0).color(color::text::PRIMARY);
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

#[allow(clippy::too_many_arguments)]
fn render_tag_row<'a>(
  id: i32,
  name: &'a str,
  color_hex: Option<&'a str>,
  index: usize,
  total: usize,
  editing: bool,
  draft: &'a str,
  color_open: bool,
) -> Vec<Element<'a, Message>> {
  let swatch_color = color_hex.and_then(hex_to_iced_color).unwrap_or(color::state::TAG_FILL);

  let up_btn = {
    let b = button(
      text("\u{25B2}")
        .font(typography::mono::REGULAR)
        .size(8.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding::new(4.0))
    .width(22.0)
    .style(|_, status| button::Style {
      background: None,
      border: Border::default(),
      text_color: if matches!(status, button::Status::Disabled) {
        color::text::TERTIARY
      } else {
        color::text::SECONDARY
      },
      snap: false,
      shadow: iced::Shadow::default(),
    });
    if index > 0 {
      b.on_press(Message::TagMoveUp(id))
    } else {
      b
    }
  };

  let down_btn = {
    let b = button(
      text("\u{25BC}")
        .font(typography::mono::REGULAR)
        .size(8.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding::new(4.0))
    .width(22.0)
    .style(|_, status| button::Style {
      background: None,
      border: Border::default(),
      text_color: if matches!(status, button::Status::Disabled) {
        color::text::TERTIARY
      } else {
        color::text::SECONDARY
      },
      snap: false,
      shadow: iced::Shadow::default(),
    });
    if index + 1 < total {
      b.on_press(Message::TagMoveDown(id))
    } else {
      b
    }
  };

  let swatch_msg = if color_open {
    Message::TagColorClose
  } else {
    Message::TagColorOpen(id)
  };
  let swatch_btn = button(Space::new().width(22.0).height(22.0))
    .padding(Padding::ZERO)
    .width(22.0)
    .height(22.0)
    .on_press(swatch_msg)
    .style(move |_, status| button::Style {
      background: Some(Background::Color(swatch_color)),
      border: Border {
        color: if color_open {
          color::accent::PLASMA
        } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
          color::accent::PLASMA_MUTED
        } else if color_hex.is_some() {
          Color {
            a: 0.5,
            ..swatch_color
          }
        } else {
          color::border::SUBTLE
        },
        radius: radius::CHIP.into(),
        width: if color_open { 2.0 } else { 1.0 },
      },
      snap: false,
      text_color: Color::TRANSPARENT,
      shadow: iced::Shadow::default(),
    });

  let name_el: Element<'a, Message> = if editing {
    text_input("", draft)
      .on_input(Message::TagDraftChanged)
      .on_submit(Message::TagRename)
      .font(typography::body::REGULAR)
      .size(14.0)
      .style(|_, _| text_input::Style {
        background: Background::Color(color::surface::SUNKEN),
        border: Border {
          color: color::accent::PLASMA,
          radius: radius::CHIP.into(),
          width: 1.0,
        },
        icon: color::text::SECONDARY,
        placeholder: color::text::TERTIARY,
        value: color::text::PRIMARY,
        selection: color::state::SELECTION,
      })
      .into()
  } else {
    button(
      text(name)
        .font(typography::body::REGULAR)
        .size(14.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        }),
    )
    .padding(Padding::ZERO)
    .on_press(Message::TagEditStart(id))
    .style(|_, _| button::Style {
      background: None,
      border: Border::default(),
      snap: false,
      text_color: color::text::PRIMARY,
      shadow: iced::Shadow::default(),
    })
    .into()
  };

  let preview = tag_preview_chip(name, color_hex);

  let delete_btn = button(text("\u{00D7}").font(typography::body::REGULAR).size(14.0))
    .width(26.0)
    .height(26.0)
    .padding(Padding::ZERO)
    .on_press(Message::TagDelete(id))
    .style(|_, status| button::Style {
      background: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        Some(Background::Color(color::status::DANGER_SUBTLE))
      } else {
        None
      },
      border: Border {
        color: color::border::SUBTLE,
        radius: radius::CHIP.into(),
        width: 1.0,
      },
      snap: false,
      text_color: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        color::status::DANGER
      } else {
        color::text::SECONDARY
      },
      shadow: iced::Shadow::default(),
    });

  let tag_row: Element<'a, Message> = container(
    row([
      up_btn.into(),
      down_btn.into(),
      swatch_btn.into(),
      container(name_el)
        .width(Length::Fill)
        .padding(Padding {
          left: spacing::SPACE_3,
          right: spacing::SPACE_3,
          ..Padding::ZERO
        })
        .into(),
      preview,
      delete_btn.into(),
    ])
    .spacing(0.0)
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 9.0,
      bottom: 9.0,
      left: spacing::SPACE_5,
      right: spacing::SPACE_4,
    }),
  )
  .width(Length::Fill)
  .into();

  let row_border: Element<'a, Message> = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into();

  let mut result: Vec<Element<'a, Message>> = vec![tag_row, row_border];

  if color_open {
    result.push(color_picker_inline(id, color_hex.map(|s| s.to_string())));
  }

  result
}

fn render_tags_panel(state: &State) -> Element<'_, Message> {
  let header = tag_header_section();
  let border = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });
  let create = tag_create_section(state);
  let create_border = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });
  let list = tag_list_body(state);
  column([header, border.into(), create, create_border.into(), list])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn render_toggle(on: bool, feature: Feature) -> Element<'static, Message> {
  let track = toggle_track(on);
  button(track)
    .padding(Padding::ZERO)
    .style(|_, _| button::Style {
      background: None,
      ..button::Style::default()
    })
    .on_press(Message::ToggleFeature(feature))
    .into()
}

fn tag_create_section(state: &State) -> Element<'_, Message> {
  let plus = text("+")
    .size(14.0)
    .font(typography::body::MEDIUM)
    .style(|_| iced::widget::text::Style {
      color: Some(color::accent::PLASMA),
    });

  let input = text_input("Create a tag\u{2026}", &state.tag_new_name)
    .on_input(Message::TagNewNameChanged)
    .on_submit(Message::TagCreate)
    .font(typography::body::REGULAR)
    .size(13.0)
    .style(|_, _| text_input::Style {
      background: Background::Color(Color::TRANSPARENT),
      border: Border::default(),
      icon: color::text::SECONDARY,
      placeholder: color::text::TERTIARY,
      value: color::text::PRIMARY,
      selection: color::state::SELECTION,
    })
    .padding(Padding::ZERO);

  let input_pill = container(
    row([plus.into(), input.into()])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .max_width(360.0)
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
  });

  let can_create = !state.tag_new_name.trim().is_empty();
  let add_btn = {
    let b = button(
      text("Add")
        .font(typography::body::MEDIUM)
        .size(13.0)
        .style(move |_| iced::widget::text::Style {
          color: if can_create {
            Some(color::surface::SUNKEN)
          } else {
            Some(color::text::TERTIARY)
          },
        }),
    )
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    })
    .style(move |_, _| button::Style {
      background: if can_create {
        Some(Background::Color(color::accent::PLASMA))
      } else {
        Some(Background::Color(color::state::HOVER_OVERLAY))
      },
      border: Border {
        color: if can_create {
          color::accent::PLASMA
        } else {
          color::border::SUBTLE
        },
        radius: radius::CHIP.into(),
        width: 1.0,
      },
      snap: false,
      text_color: if can_create {
        color::surface::SUNKEN
      } else {
        color::text::TERTIARY
      },
      shadow: iced::Shadow::default(),
    });
    if can_create { b.on_press(Message::TagCreate) } else { b }
  };

  let colored = state.tags.iter().filter(|(_, _, c)| c.is_some()).count();
  let stats = text(format!("{} tags  ·  {} colored", state.tags.len(), colored))
    .font(typography::mono::REGULAR)
    .size(10.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    });

  container(column([
    row([input_pill.into(), add_btn.into()])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
    Space::new().height(8.0).into(),
    stats.into(),
  ]))
  .padding(Padding {
    top: spacing::SPACE_3_5,
    bottom: spacing::SPACE_3_5,
    left: 36.0,
    right: 36.0,
  })
  .width(Length::Fill)
  .into()
}

fn tag_header_section() -> Element<'static, Message> {
  let title = text("Tags").size(18.0).color(color::text::PRIMARY);
  let desc = text(
    "Assign a color to any tag and it will render that way everywhere it appears \
    on a character card. Use the arrows to reorder; tags follow this order on cards.",
  )
  .size(13.0)
  .color(color::text::SECONDARY);
  column([
    row([title.into(), Space::new().width(Length::Fill).into()])
      .align_y(Vertical::Center)
      .into(),
    Space::new().height(4.0).into(),
    desc.into(),
  ])
  .padding(Padding {
    top: 24.0,
    bottom: spacing::SPACE_3_5,
    left: 36.0,
    right: 36.0,
  })
  .into()
}

fn tag_list_body(state: &State) -> Element<'_, Message> {
  if state.tags.is_empty() {
    return scrollable(
      container(
        text("No tags yet. Create one above.")
          .font(typography::body::REGULAR)
          .size(13.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .width(Length::Fill)
      .padding(Padding::new(80.0))
      .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  let total = state.tags.len();
  let mut items: Vec<Element<'_, Message>> = Vec::new();

  for (index, (id, name, color_hex)) in state.tags.iter().enumerate() {
    let editing = state.tag_editing == Some(*id);
    let color_open = state.tag_color_open == Some(*id);
    let rows = render_tag_row(
      *id,
      name,
      color_hex.as_deref(),
      index,
      total,
      editing,
      &state.tag_draft,
      color_open,
    );
    items.extend(rows);
  }

  scrollable(column(items).width(Length::Fill).padding(Padding {
    top: 0.0,
    bottom: 60.0,
    left: 0.0,
    right: 0.0,
  }))
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn tag_preview_chip<'a>(name: &'a str, color_hex: Option<&'a str>) -> Element<'a, Message> {
  let (bg, fg, bd) = match color_hex.and_then(hex_to_iced_color) {
    Some(c) => (
      Color {
        a: 0.12,
        ..c
      },
      c,
      Color {
        a: 0.45,
        ..c
      },
    ),
    None => (color::state::TAG_FILL, color::text::SECONDARY, color::border::SUBTLE),
  };
  container(
    text(name)
      .font(typography::body::MEDIUM)
      .size(11.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(fg),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 8.0,
    right: 8.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(bg)),
    border: Border {
      color: bd,
      radius: radius::FULL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn toggle_thumb(on: bool) -> container::Container<'static, Message> {
  let thumb_color = if on {
    color::state::TOGGLE_THUMB
  } else {
    color::text::MEDIUM
  };
  container(Space::new())
    .width(component::toggle::THUMB_SIZE)
    .height(component::toggle::THUMB_SIZE)
    .style(move |_| container::Style {
      background: Some(Background::Color(thumb_color)),
      border: Border {
        radius: radius::FULL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
}

fn toggle_track(on: bool) -> container::Container<'static, Message> {
  let bg_color = if on {
    color::accent::PLASMA
  } else {
    color::state::PRESSED_OVERLAY
  };
  let border_color = if on {
    color::accent::PLASMA
  } else {
    color::border::DEFAULT
  };
  let thumb_offset = if on {
    component::toggle::THUMB_ON_OFFSET
  } else {
    component::toggle::THUMB_OFF_OFFSET
  };
  let thumb = toggle_thumb(on);
  container(
    container(thumb)
      .padding(Padding {
        top: 2.0,
        bottom: 2.0,
        left: thumb_offset,
        right: 0.0,
      })
      .align_y(Vertical::Center),
  )
  .width(component::toggle::TRACK_WIDTH)
  .height(component::toggle::TRACK_HEIGHT)
  .style(move |_| container::Style {
    background: Some(Background::Color(bg_color)),
    border: Border {
      color: border_color,
      radius: radius::FULL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
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
