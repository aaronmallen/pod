//! Settings view: feature-flag toggles and preferences.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Space, button, column, container, mouse_area, row, scrollable, text, text_input},
};

use crate::style::{color, component, radius, shadow, spacing, typography};

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

/// Messages produced by the settings view.
#[derive(Clone, Debug)]
pub enum Message {
  /// A settings category was selected in the sidebar.
  CategorySelected(Category),
  /// All settings were reset to their defaults.
  ResetDefaults,
  /// The feature search query changed.
  SearchChanged(String),
  /// The color picker for a tag was closed.
  TagColorClose,
  /// The hex draft in the color picker changed.
  TagColorHexChanged(String),
  /// The hex draft was committed (Enter key).
  TagColorHexCommit,
  /// The color picker for a tag was opened.
  TagColorOpen(i32),
  /// The color-set DB operation returned a result.
  TagColorSet(Result<(i32, String, Option<String>), String>),
  /// Create a new tag was requested.
  TagCreate,
  /// The tag-create DB operation returned a result.
  TagCreated(Result<(i32, String, Option<String>), String>),
  /// Delete a tag was requested.
  TagDelete(i32),
  /// The tag-delete DB operation returned a result.
  TagDeleted(Result<i32, String>),
  /// A drag was released (drop or cancel).
  TagDragEnd,
  /// A drag was started on the tag with the given id.
  TagDragStart(i32),
  /// The rename draft for the currently-edited tag changed.
  TagDraftChanged(String),
  /// The drag was dropped; drop target is in state.tag_drag_over.
  TagDrop,
  /// Inline editing of a tag name was initiated.
  TagEditStart(i32),
  /// The new-tag name input changed.
  TagNewNameChanged(String),
  /// The rename of the currently-edited tag was committed.
  TagRename,
  /// The tag-rename DB operation returned a result.
  TagRenamed(Result<(i32, String, Option<String>), String>),
  /// The tag-reorder DB operation returned a result.
  TagReordered(Result<(), String>),
  /// The tag list filter query changed.
  TagSearchChanged(String),
  /// A color was selected or cleared for a tag.
  TagSetColor(i32, Option<String>),
  /// The cursor entered a tag row's bounds during a drag.
  TagSlotEntered(i32),
  /// The sort mode for the tag list changed.
  TagSortModeChanged(TagSortMode),
  /// The full tag list was loaded from the database.
  TagsLoaded(Vec<(i32, String, Option<String>)>),
  /// A feature flag was toggled.
  ToggleFeature(Feature),
}

/// Runtime state for the settings view.
pub struct State {
  /// The currently active settings category.
  pub active_category: Category,
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
  /// Current hex draft in the color picker text input.
  pub tag_color_hex_draft: String,
  /// Id of the tag whose color picker is currently open, if any.
  pub tag_color_open: Option<i32>,
  /// Current name draft for the inline rename input.
  pub tag_draft: String,
  /// Id of the tag currently acting as the drop target during a drag.
  pub tag_drag_over: Option<i32>,
  /// Id of the tag currently being dragged, if any.
  pub tag_dragging: Option<i32>,
  /// Id of the tag currently being renamed inline, if any.
  pub tag_editing: Option<i32>,
  /// Text in the "Create a tag" input.
  pub tag_new_name: String,
  /// Filter query for the tag list.
  pub tag_search: String,
  /// Current sort mode for the tag list.
  pub tag_sort_mode: TagSortMode,
  /// All tags, in manual sort order.
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
      tag_color_hex_draft: String::new(),
      tag_color_open: None,
      tag_draft: String::new(),
      tag_drag_over: None,
      tag_dragging: None,
      tag_editing: None,
      tag_new_name: String::new(),
      tag_search: String::new(),
      tag_sort_mode: TagSortMode::default(),
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

/// Sort mode for the tag list.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum TagSortMode {
  /// Colored tags first (grouped by hex), then alphabetical.
  Color,
  /// Manual drag-and-drop order.
  #[default]
  Manual,
  /// Alphabetical by name.
  Name,
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

fn color_picker_popover<'a>(tag_id: i32, current_hex: Option<&'a str>, hex_draft: &'a str) -> Element<'a, Message> {
  let header = text("PICK A COLOR")
    .font(typography::mono::REGULAR)
    .size(9.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });

  let swatches: Vec<Element<'static, Message>> = TAG_PALETTE
    .iter()
    .map(|&(_name, hex)| {
      let Some(swatch_color) = hex_to_iced_color(hex) else {
        return Space::new().width(30.0).height(30.0).into();
      };
      let is_selected = current_hex == Some(hex);
      let hex_owned = hex.to_string();
      button(Space::new().width(Length::Fill).height(Length::Fill))
        .padding(Padding::ZERO)
        .width(30.0)
        .height(30.0)
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
            radius: Radius::from(5.0),
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

  let palette_row = row(swatches).spacing(6.0).wrap();

  let hex_label = text("HEX")
    .font(typography::mono::REGULAR)
    .size(11.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::GHOST),
    });

  let hex_input = text_input("#RRGGBB", hex_draft)
    .on_input(Message::TagColorHexChanged)
    .on_submit(Message::TagColorHexCommit)
    .font(typography::mono::REGULAR)
    .size(12.0)
    .padding(Padding::ZERO)
    .style(|_, _| text_input::Style {
      background: Background::Color(Color::TRANSPARENT),
      border: Border::default(),
      icon: color::text::SECONDARY,
      placeholder: color::text::TERTIARY,
      value: color::text::PRIMARY,
      selection: color::state::SELECTION,
    });

  let preview_color = normalize_hex(hex_draft)
    .and_then(|h| hex_to_iced_color(&h))
    .unwrap_or(Color::TRANSPARENT);
  let hex_preview = container(Space::new())
    .width(18.0)
    .height(18.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(preview_color)),
      border: Border {
        color: color::border::SUBTLE,
        radius: Radius::from(4.0),
        width: 1.0,
      },
      ..container::Style::default()
    });

  let hex_row = container(
    row([hex_label.into(), hex_input.into(), hex_preview.into()])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 10.0,
    right: 8.0,
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

  let divider = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });

  let clear_btn = button(
    text("Clear color")
      .font(typography::body::REGULAR)
      .size(12.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
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
      header.into(),
      Space::new().height(10.0).into(),
      palette_row.into(),
      Space::new().height(12.0).into(),
      hex_row.into(),
      Space::new().height(12.0).into(),
      divider.into(),
      Space::new().height(10.0).into(),
      clear_btn.into(),
    ])
    .spacing(0.0),
  )
  .width(256.0)
  .padding(12.0)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::border::DEFAULT,
      radius: radius::PANEL.into(),
      width: 1.0,
    },
    shadow: shadow::POPOVER,
    ..container::Style::default()
  })
  .into()
}

fn drag_handle<'a, MSG: 'a>(draggable: bool) -> Element<'a, MSG> {
  let dot_color = if draggable {
    color::text::DIM
  } else {
    color::text::GHOST
  };
  container(column([
    drag_handle_pair(dot_color),
    Space::new().height(5.0).into(),
    drag_handle_pair(dot_color),
    Space::new().height(5.0).into(),
    drag_handle_pair(dot_color),
  ]))
  .width(18.0)
  .height(24.0)
  .center_x(18.0)
  .center_y(24.0)
  .into()
}

fn drag_handle_pair<'a, MSG: 'a>(dot_color: Color) -> Element<'a, MSG> {
  row([
    container(Space::new())
      .width(2.4)
      .height(2.4)
      .style(move |_| container::Style {
        background: Some(Background::Color(dot_color)),
        border: Border {
          radius: radius::FULL.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    Space::new().width(6.0).into(),
    container(Space::new())
      .width(2.4)
      .height(2.4)
      .style(move |_| container::Style {
        background: Some(Background::Color(dot_color)),
        border: Border {
          radius: radius::FULL.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
  ])
  .into()
}

fn drop_indicator<'a>() -> Element<'a, Message> {
  container(Space::new().width(Length::Fill).height(2.0))
    .width(Length::Fill)
    .height(2.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::PLASMA)),
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

fn normalize_hex(raw: &str) -> Option<String> {
  let s = raw.trim().trim_start_matches('#');
  if s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit()) {
    Some(format!("#{}", s.to_uppercase()))
  } else if s.len() == 3 && s.chars().all(|c| c.is_ascii_hexdigit()) {
    let expanded: String = s.chars().flat_map(|c| [c, c]).collect();
    Some(format!("#{}", expanded.to_uppercase()))
  } else {
    None
  }
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
  let colored_count = state.tags.iter().filter(|(_, _, c)| c.is_some()).count();
  let tags_row = categories_item_row(
    "Tags",
    Some(colored_count.to_string()),
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
  draggable: bool,
  editing: bool,
  draft: &'a str,
  color_open: bool,
  hex_draft: &'a str,
  is_dragging: bool,
  is_drop_above: bool,
) -> Vec<Element<'a, Message>> {
  let swatch_color = color_hex.and_then(hex_to_iced_color).unwrap_or(color::state::TAG_FILL);

  let handle = drag_handle(draggable);

  let swatch_msg = if color_open {
    Message::TagColorClose
  } else {
    Message::TagColorOpen(id)
  };
  let swatch_btn = button(Space::new().width(Length::Fill).height(Length::Fill))
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

  let delete_btn = button(
    container(text("\u{00D7}").font(typography::body::REGULAR).size(14.0))
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
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

  let drag_handle_el: Element<'a, Message> = if draggable {
    mouse_area(handle).on_press(Message::TagDragStart(id)).into()
  } else {
    handle
  };

  let row_bg = if is_dragging {
    Some(Background::Color(Color {
      a: 0.04,
      ..color::accent::PLASMA
    }))
  } else {
    None
  };

  let tag_row: Element<'a, Message> = container(
    row([
      drag_handle_el,
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
    .spacing(10.0)
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    }),
  )
  .width(Length::Fill)
  .style(move |_| container::Style {
    background: row_bg,
    ..container::Style::default()
  })
  .into();

  let row_border: Element<'a, Message> = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into();

  let mut result: Vec<Element<'a, Message>> = Vec::new();
  if is_drop_above {
    result.push(drop_indicator());
  }
  result.push(tag_row);
  result.push(row_border);
  if color_open {
    result.push(
      container(color_picker_popover(id, color_hex, hex_draft))
        .padding(Padding {
          top: 0.0,
          bottom: 12.0,
          left: spacing::SPACE_4,
          right: spacing::SPACE_4,
        })
        .width(Length::Fill)
        .into(),
    );
  }
  result
}

fn render_tags_panel(state: &State) -> Element<'_, Message> {
  let header = tag_panel_header(state);
  let border = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });
  let list = tag_list_body(state);
  let panel: Element<'_, Message> = column([header, border.into(), list])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  if state.tag_dragging.is_some() {
    mouse_area(panel).on_release(Message::TagDrop).into()
  } else {
    panel
  }
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

fn sort_mode_button(label: &'static str, is_active: bool, msg: Message) -> Element<'static, Message> {
  let text_color = if is_active {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };
  let bg = if is_active {
    Some(Background::Color(color::accent::PLASMA_SUBTLE))
  } else {
    None
  };
  button(
    text(label)
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(text_color),
      }),
  )
  .padding(Padding {
    top: 5.0,
    bottom: 5.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(msg)
  .style(move |_, _| button::Style {
    background: bg,
    border: Border {
      radius: Radius::from(4.0),
      ..Border::default()
    },
    snap: false,
    text_color,
    shadow: iced::Shadow::default(),
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

  let search = state.tag_search.trim().to_lowercase();
  let draggable = state.tag_sort_mode == TagSortMode::Manual && search.is_empty();

  let mut filtered: Vec<&(i32, String, Option<String>)> = state
    .tags
    .iter()
    .filter(|(_, name, _)| search.is_empty() || name.to_lowercase().contains(&search))
    .collect();

  match state.tag_sort_mode {
    TagSortMode::Manual => {}
    TagSortMode::Name => filtered.sort_by(|(_, a, _), (_, b, _)| a.cmp(b)),
    TagSortMode::Color => filtered.sort_by(|(_, a_name, a_color), (_, b_name, b_color)| match (a_color, b_color) {
      (Some(_), None) => std::cmp::Ordering::Less,
      (None, Some(_)) => std::cmp::Ordering::Greater,
      (Some(ca), Some(cb)) => ca.cmp(cb).then(a_name.cmp(b_name)),
      (None, None) => a_name.cmp(b_name),
    }),
  }

  if filtered.is_empty() {
    return scrollable(
      container(
        text(format!("No tags match \"{}\".", state.tag_search))
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

  let mut items: Vec<Element<'_, Message>> = Vec::new();
  let is_active_drag = state.tag_dragging.is_some();

  for (id, name, color_hex) in &filtered {
    let editing = state.tag_editing == Some(*id);
    let color_open = state.tag_color_open == Some(*id);
    let is_dragging_this = state.tag_dragging == Some(*id);
    let is_drop_above = is_active_drag && !is_dragging_this && state.tag_drag_over == Some(*id);

    let row_elements = render_tag_row(
      *id,
      name,
      color_hex.as_deref(),
      draggable,
      editing,
      &state.tag_draft,
      color_open,
      &state.tag_color_hex_draft,
      is_dragging_this,
      is_drop_above,
    );

    if is_active_drag && !is_dragging_this {
      let id_copy = *id;
      items.push(
        mouse_area(column(row_elements).width(Length::Fill))
          .on_enter(Message::TagSlotEntered(id_copy))
          .into(),
      );
    } else {
      items.extend(row_elements);
    }
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

fn tag_panel_header(state: &State) -> Element<'_, Message> {
  let title = text("Tags").size(18.0).color(color::text::PRIMARY);
  let desc = text(
    "Assign a color to any tag and it will render that way everywhere it appears \
    on a character card. Drag rows to reorder; tags follow their manual order on cards.",
  )
  .size(13.0)
  .color(color::text::SECONDARY);

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
  .width(Length::Fill)
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

  let sort_control = container(row([
    sort_mode_button(
      "Manual",
      state.tag_sort_mode == TagSortMode::Manual,
      Message::TagSortModeChanged(TagSortMode::Manual),
    ),
    sort_mode_button(
      "A–Z",
      state.tag_sort_mode == TagSortMode::Name,
      Message::TagSortModeChanged(TagSortMode::Name),
    ),
    sort_mode_button(
      "Color",
      state.tag_sort_mode == TagSortMode::Color,
      Message::TagSortModeChanged(TagSortMode::Color),
    ),
  ]))
  .padding(2.0)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      radius: radius::CHIP.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  let search_icon = crate::components::Icon::search()
    .size(14.0)
    .color(color::text::SECONDARY)
    .render::<Message>();

  let filter_input = container(
    row([
      search_icon,
      text_input("Filter\u{2026}", &state.tag_search)
        .on_input(Message::TagSearchChanged)
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
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .max_width(200.0)
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

  let create_row: Element<'_, Message> = row([
    input_pill.into(),
    add_btn.into(),
    Space::new().width(Length::Fill).into(),
    sort_control.into(),
    filter_input.into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into();

  let colored = state.tags.iter().filter(|(_, _, c)| c.is_some()).count();
  let draggable = state.tag_sort_mode == TagSortMode::Manual && state.tag_search.trim().is_empty();
  let mut stats_parts: Vec<Element<'_, Message>> = vec![
    text(format!("{}", state.tags.len()))
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    text(" tags")
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::GHOST),
      })
      .into(),
    Space::new().width(10.0).into(),
    container(Space::new())
      .width(3.0)
      .height(3.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::text::GHOST)),
        border: Border {
          radius: radius::FULL.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    Space::new().width(10.0).into(),
    text(format!("{colored}"))
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
    text(" colored")
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::GHOST),
      })
      .into(),
  ];
  if !draggable {
    let warning = if state.tag_search.trim().is_empty() {
      "Reorder disabled in sorted view"
    } else {
      "Reorder disabled while filtering"
    };
    stats_parts.extend([
      Space::new().width(10.0).into(),
      container(Space::new())
        .width(3.0)
        .height(3.0)
        .style(|_| container::Style {
          background: Some(Background::Color(color::text::GHOST)),
          border: Border {
            radius: radius::FULL.into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into(),
      Space::new().width(10.0).into(),
      text(warning)
        .font(typography::mono::REGULAR)
        .size(10.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::WARNING),
        })
        .into(),
    ]);
  }
  let stats_row: Element<'_, Message> = row(stats_parts).align_y(Vertical::Center).into();

  column([
    row([title.into(), Space::new().width(Length::Fill).into()])
      .align_y(Vertical::Center)
      .into(),
    Space::new().height(4.0).into(),
    desc.into(),
    Space::new().height(spacing::SPACE_3_5).into(),
    create_row,
    Space::new().height(8.0).into(),
    stats_row,
  ])
  .padding(Padding {
    top: 24.0,
    bottom: spacing::SPACE_3_5,
    left: 36.0,
    right: 36.0,
  })
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
