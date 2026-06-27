use std::sync::OnceLock;

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, mouse_area, scrollable, text},
};

use super::{GeoNodeKey, GeoSelection, Message, Owner, Scope, State, fmt_count, fmt_isk, resolve_scope_owner};
use crate::{
  store::{
    Database,
    model::asset_query::{GeoConstellationNode, GeoLocationNode, GeoRegionNode, GeoSort, GeoSystemNode, GeoTree},
    repo::assets,
  },
  ui::{
    components::{
      context_menu::{self, Item},
      eyebrow::eyebrow,
      icon::Icon,
      rule,
      text_input::TextInput,
    },
    style::{color, radius, spacing, typography},
  },
};

const BASE_INDENT: f32 = spacing::SPACE_3;

const INDENT_STEP: f32 = 12.0;

const CARET_SLOT: f32 = 12.0;

const CARET_ICON: f32 = 12.0;

const TIER_SLOT: f32 = 16.0;

const REGION_ICON: f32 = 13.0;

const TIER_ICON: f32 = 15.0;

const RAIL_WIDTH: f32 = 2.0;

struct RowSpec<'a> {
  caret: Option<(GeoNodeKey, bool)>,
  depth: usize,
  metric: Option<String>,
  name: &'a str,
  on_press: Message,
  sec: Option<f64>,
  selected: bool,
  tier: Tier,
}

#[derive(Clone, Copy)]
enum Tier {
  All,
  Constellation,
  Region,
  Station,
  System,
}

impl Tier {
  fn icon(self) -> Icon {
    match self {
      Self::All => Icon::tier_all(),
      Self::Constellation => Icon::tier_constellation(),
      Self::Region => Icon::tier_region(),
      Self::Station => Icon::tier_station(),
      Self::System => Icon::tier_system(),
    }
  }

  fn icon_color(self, selected: bool) -> Color {
    if selected {
      return color::accent::PLASMA;
    }
    match self {
      Self::Region => color::text::tertiary(),
      Self::All | Self::Constellation | Self::System => color::text::secondary(),
      Self::Station => color::accent::PLASMA,
    }
  }

  fn icon_size(self) -> f32 {
    match self {
      Self::Region => REGION_ICON,
      _ => TIER_ICON,
    }
  }

  fn is_context(self) -> bool {
    matches!(self, Self::Region | Self::Constellation | Self::System)
  }
}

pub(super) async fn load_geo_tree(
  db: &Database,
  scope: Scope,
  roster: &[super::RosterPilot],
  corporations: &[super::RosterCorp],
) -> GeoTree {
  let Some(owner) = resolve_scope_owner(scope, roster, corporations) else {
    return GeoTree::default();
  };
  let rows = match &owner {
    Owner::Character(id) => assets::geo_locations_for_character(db, *id).await,
    Owner::Combined {
      character_ids,
      corporation_ids,
    } => assets::geo_locations_for_combined(db, character_ids, corporation_ids).await,
    Owner::Corporation(id) => assets::geo_locations_for_corporation(db, *id).await,
  }
  .unwrap_or_default();
  GeoTree::from_locations(&rows)
}

pub(super) fn pane(state: &State) -> Element<'_, Message> {
  let tree = state.geo_tree();
  let header = container(
    Row::with_children(vec![
      eyebrow(&t!("assets.tree.locations"), Some(color::text::secondary())),
      Space::new().width(Length::Fill).into(),
      eyebrow(
        &t!("assets.tree.region_count", count => fmt_count(tree.regions.len() as i64)),
        Some(color::text::tertiary()),
      ),
      loc_sort_toggle(state.geo_sort()),
    ])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3_5,
  });

  let mut rows: Vec<Element<'_, Message>> = vec![all_assets_row(matches!(state.geo_selected(), GeoSelection::All))];
  for region in &tree.regions {
    push_region(state, &mut rows, region);
  }
  for orphan in &tree.orphans {
    rows.push(location_row(state, orphan, 0));
  }

  let body = scrollable(Column::with_children(rows).width(Length::Fill))
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill);

  let column = Column::with_children(vec![
    saved_filters_section(state),
    rule_divider(),
    header.into(),
    body.into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  // Track the cursor so a right-click on a saved-filter row can anchor its menu.
  mouse_area(column).on_move(Message::SidebarCursorMoved).into()
}

/// The Value / A–Z segmented toggle shown in the Locations header. The active
/// option is highlighted in plasma; clicking the inactive one emits
/// `LocSortSelected` to re-sort the tree.
fn loc_sort_toggle<'a>(active: GeoSort) -> Element<'a, Message> {
  let options = [
    (GeoSort::Value, t!("assets.tree.sort_value").into_owned()),
    (GeoSort::Alpha, t!("assets.tree.sort_alpha").into_owned()),
  ];
  let mut segments: Vec<Element<'a, Message>> = Vec::with_capacity(options.len());
  for (mode, label) in options {
    let selected = mode == active;
    let text_color = if selected {
      color::accent::PLASMA
    } else {
      color::text::tertiary()
    };
    segments.push(
      button(
        text(label)
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(move |_| text::Style {
            color: Some(text_color),
          }),
      )
      .padding(Padding {
        top: spacing::UNIT,
        right: spacing::UNIT + 2.0,
        bottom: spacing::UNIT,
        left: spacing::UNIT + 2.0,
      })
      .on_press(Message::LocSortSelected(mode))
      .style(move |_, status| {
        let background = if selected {
          Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.12)))
        } else if matches!(status, button::Status::Hovered) {
          Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.04)))
        } else {
          None
        };
        button::Style {
          background,
          border: Border {
            radius: 0.0.into(),
            ..Border::default()
          },
          ..button::Style::default()
        }
      })
      .into(),
    );
  }

  container(Row::with_children(segments))
    .style(|_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.12),
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn rule_divider<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fill)
    .height(Length::Fixed(1.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
      ..container::Style::default()
    })
    .into()
}

/// Labels of the action buttons rendered in the saved-filters section header.
///
/// The redundant "+ new" affordance was removed in favour of the "★ Save"
/// button in the inventory filter bar, so the header currently exposes no
/// actions. Kept as a pure helper so tests can assert this without trying to
/// introspect the (opaque) iced widget tree.
fn saved_filters_header_actions() -> Vec<&'static str> {
  Vec::new()
}

fn saved_filters_section(state: &State) -> Element<'_, Message> {
  let mut header_children: Vec<Element<'_, Message>> = vec![
    eyebrow(&t!("assets.tree.saved_filters"), Some(color::text::secondary())),
    Space::new().width(Length::Fill).into(),
  ];
  for label in saved_filters_header_actions() {
    header_children.push(eyebrow(label, Some(color::accent::PLASMA)));
  }
  let header = Row::with_children(header_children)
    .width(Length::Fill)
    .align_y(Vertical::Center);

  let mut children: Vec<Element<'_, Message>> = vec![
    container(header)
      .padding(Padding {
        top: spacing::SPACE_2,
        right: spacing::SPACE_3_5,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_3_5,
      })
      .into(),
  ];

  for filter in state.saved_filters() {
    children.push(saved_filter_row(
      filter,
      state.saved_filter_active() == Some(filter.id()),
    ));
  }

  Column::with_children(children).width(Length::Fill).into()
}

fn saved_filter_row(filter: &crate::store::model::SavedAssetFilter, active: bool) -> Element<'_, Message> {
  let chip_bg = if active {
    color::accent::PLASMA
  } else {
    color::with_alpha(color::text::PRIMARY, 0.1)
  };
  let chip_color = if active {
    color::surface::BASE
  } else {
    color::text::secondary()
  };
  let chip = container(
    text("\u{2605}")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(chip_color),
      }),
  )
  .width(Length::Fixed(14.0))
  .height(Length::Fixed(14.0))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: Some(Background::Color(chip_bg)),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  let name_color = if active {
    color::text::PRIMARY
  } else {
    color::with_alpha(color::text::PRIMARY, 0.78)
  };

  let hint = saved_filter_hint(filter);

  let row = Row::with_children(vec![
    chip.into(),
    text(filter.name().to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(name_color),
      })
      .width(Length::Fill)
      .into(),
    text(hint)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let id = filter.id();
  let pressable = button(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::UNIT + 1.0,
      right: spacing::SPACE_3_5,
      bottom: spacing::UNIT + 1.0,
      left: spacing::SPACE_3_5,
    })
    .on_press(Message::SavedFilterSelected(id))
    .style(move |_, status| {
      let background = if active {
        Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.1)))
      } else if matches!(status, button::Status::Hovered) {
        Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.04)))
      } else {
        None
      };
      button::Style {
        background,
        border: Border {
          radius: 0.0.into(),
          ..Border::default()
        },
        ..button::Style::default()
      }
    });

  mouse_area(pressable)
    .on_right_press(Message::SavedFilterRightPressed(id))
    .into()
}

fn saved_filter_hint(filter: &crate::store::model::SavedAssetFilter) -> String {
  const MAX: usize = 22;
  let query = filter.query().trim();
  if !query.is_empty() {
    return if query.chars().count() > MAX {
      let truncated: String = query.chars().take(MAX - 1).collect();
      format!("{truncated}\u{2026}")
    } else {
      query.to_owned()
    };
  }
  match filter.category() {
    Some(category) => category.clone(),
    None => t!("assets.tree.all_assets_hint").into_owned(),
  }
}

fn push_region<'a>(state: &State, rows: &mut Vec<Element<'a, Message>>, region: &'a GeoRegionNode) {
  let key = GeoNodeKey::Region(region.region_id);
  let selection = GeoSelection::Region(region.region_id);
  let collapsed = state.geo_is_collapsed(key);
  rows.push(node_row(RowSpec {
    depth: 0,
    tier: Tier::Region,
    caret: Some((key, collapsed)),
    name: &region.region_name,
    metric: (region.value > 0.0).then(|| fmt_isk(region.value)),
    sec: None,
    selected: state.geo_selected() == selection,
    on_press: Message::GeoNodeSelected(selection),
  }));

  if collapsed {
    return;
  }
  for constellation in &region.constellations {
    push_constellation(state, rows, constellation);
  }
}

fn push_constellation<'a>(
  state: &State,
  rows: &mut Vec<Element<'a, Message>>,
  constellation: &'a GeoConstellationNode,
) {
  let key = GeoNodeKey::Constellation(constellation.constellation_id);
  let selection = GeoSelection::Constellation(constellation.constellation_id);
  let collapsed = state.geo_is_collapsed(key);
  rows.push(node_row(RowSpec {
    depth: 1,
    tier: Tier::Constellation,
    caret: Some((key, collapsed)),
    name: &constellation.constellation_name,
    metric: (constellation.value > 0.0).then(|| fmt_isk(constellation.value)),
    sec: None,
    selected: state.geo_selected() == selection,
    on_press: Message::GeoNodeSelected(selection),
  }));

  if collapsed {
    return;
  }
  for system in &constellation.systems {
    push_system(state, rows, system);
  }
}

fn push_system<'a>(state: &State, rows: &mut Vec<Element<'a, Message>>, system: &'a GeoSystemNode) {
  let selection = GeoSelection::System(system.system_id);
  rows.push(node_row(RowSpec {
    depth: 2,
    tier: Tier::System,
    caret: None,
    name: &system.system_name,
    metric: (system.value > 0.0).then(|| fmt_isk(system.value)),
    sec: system.security_status,
    selected: state.geo_selected() == selection,
    on_press: Message::GeoNodeSelected(selection),
  }));

  for location in &system.locations {
    rows.push(location_row(state, location, 3));
  }
}

fn location_row<'a>(state: &State, location: &'a GeoLocationNode, depth: usize) -> Element<'a, Message> {
  let unknown = t!("assets.tree.unknown_location").into_owned();
  let label = location.location_label.clone().unwrap_or(unknown);
  let selection = GeoSelection::Location(location.location_id);
  node_row(RowSpec {
    depth,
    tier: Tier::Station,
    caret: None,
    name: &label,
    metric: (location.value > 0.0).then(|| fmt_isk(location.value)),
    sec: None,
    selected: state.geo_selected() == selection,
    on_press: Message::GeoNodeSelected(selection),
  })
}

fn all_assets_row<'a>(selected: bool) -> Element<'a, Message> {
  let name = t!("assets.tree.all_assets").into_owned();
  node_row(RowSpec {
    depth: 0,
    tier: Tier::All,
    caret: None,
    name: &name,
    metric: None,
    sec: None,
    selected,
    on_press: Message::GeoNodeSelected(GeoSelection::All),
  })
}

fn caret_slot<'a>(caret: Option<(GeoNodeKey, bool)>) -> Element<'a, Message> {
  let inner: Element<'a, Message> = match caret {
    Some((key, collapsed)) => {
      let chevron = if collapsed {
        Icon::chevron_right()
      } else {
        Icon::chevron()
      };
      button(chevron.size(CARET_ICON).color(color::text::tertiary()).render())
        .padding(0)
        .on_press(Message::GeoNodeToggled(key))
        .style(|_, _| button::Style::default())
        .into()
    }
    None => Space::new().into(),
  };
  container(inner)
    .width(Length::Fixed(CARET_SLOT))
    .height(Length::Fixed(CARET_SLOT))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn tier_slot<'a>(tier: Tier, selected: bool) -> Element<'a, Message> {
  container(
    tier
      .icon()
      .size(tier.icon_size())
      .color(tier.icon_color(selected))
      .render(),
  )
  .width(Length::Fixed(TIER_SLOT))
  .height(Length::Fixed(TIER_SLOT))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .into()
}

fn sec_pill<'a>(sec: f64) -> Element<'a, Message> {
  let pill_color = if sec >= 0.5 {
    color::status::ONLINE
  } else if sec > 0.0 {
    color::status::WARNING
  } else {
    color::status::DANGER
  };
  text(format!("{sec:.1}"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(move |_| text::Style {
      color: Some(pill_color),
    })
    .into()
}

fn node_row<'a>(spec: RowSpec<'_>) -> Element<'a, Message> {
  let RowSpec {
    depth,
    tier,
    caret,
    name,
    metric,
    sec,
    selected,
    on_press,
  } = spec;
  let indent = BASE_INDENT + depth as f32 * INDENT_STEP;

  let name_color = if selected {
    color::text::PRIMARY
  } else if tier.is_context() {
    color::text::secondary()
  } else {
    color::with_alpha(color::text::PRIMARY, 0.78)
  };
  let region = matches!(tier, Tier::Region);
  let name_text = if region { name.to_uppercase() } else { name.to_owned() };

  let mut content: Vec<Element<'_, Message>> = vec![
    caret_slot(caret),
    tier_slot(tier, selected),
    container(
      text(name_text)
        .font(if region {
          typography::body::MEDIUM
        } else {
          typography::body::REGULAR
        })
        .size(typography::size::SM)
        .style(move |_| text::Style {
          color: Some(name_color),
        }),
    )
    .width(Length::Fill)
    .into(),
  ];

  if let Some(sec) = sec {
    content.push(sec_pill(sec));
  }

  if let Some(metric) = metric {
    let metric_color = if selected {
      color::accent::PLASMA
    } else {
      color::text::tertiary()
    };
    content.push(
      text(metric)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(move |_| text::Style {
          color: Some(metric_color),
        })
        .into(),
    );
  }

  let rail = container(Space::new())
    .width(Length::Fixed(RAIL_WIDTH))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(if selected {
        color::accent::PLASMA
      } else {
        Color::TRANSPARENT
      })),
      ..container::Style::default()
    });

  let body = container(
    Row::with_children(content)
      .spacing(spacing::UNIT + 2.0)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::UNIT + 1.0,
    right: spacing::SPACE_3,
    bottom: spacing::UNIT + 1.0,
    left: indent,
  });

  button(Row::with_children(vec![rail.into(), body.into()]).align_y(Vertical::Center))
    .width(Length::Fill)
    .on_press(on_press)
    .style(move |_, status| {
      let background = if selected {
        Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.1)))
      } else if matches!(status, button::Status::Hovered) {
        Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.04)))
      } else {
        None
      };
      button::Style {
        background,
        border: Border {
          radius: 0.0.into(),
          ..Border::default()
        },
        ..button::Style::default()
      }
    })
    .into()
}

pub(super) fn save_filter_modal(state: &State) -> Element<'_, Message> {
  let valid = !state.saved_filter_draft_name().trim().is_empty();
  let capture = state.save_filter_capture();

  let title = Row::with_children(vec![
    container(
      text("\u{2605}")
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::accent::PLASMA),
        }),
    )
    .width(Length::Fixed(22.0))
    .height(Length::Fixed(22.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.14))),
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into(),
    text(t!("assets.tree.save_filter_title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    modal_close_button(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let name_field = Column::with_children(vec![
    eyebrow(&t!("assets.tree.filter_name"), Some(color::text::secondary())),
    TextInput::new(
      save_filter_name_placeholder(),
      state.saved_filter_draft_name(),
      Message::SaveFilterNameChanged,
    )
    .font_size(typography::size::MD)
    .padding(spacing::SPACE_2)
    .on_submit(Message::SaveFilterConfirmed)
    .render(),
  ])
  .spacing(spacing::UNIT + 1.0)
  .width(Length::Fill);

  let mut body_children: Vec<Element<'_, Message>> = vec![name_field.into()];
  if !capture.is_empty() {
    body_children.push(
      Column::with_children(vec![
        eyebrow(&t!("assets.tree.captures"), Some(color::text::secondary())),
        container(
          text(capture)
            .font(typography::mono::REGULAR)
            .size(typography::size::SM)
            .style(|_| text::Style {
              color: Some(color::accent::PLASMA),
            }),
        )
        .width(Length::Fill)
        .padding(Padding {
          top: spacing::UNIT,
          right: spacing::SPACE_2_5,
          bottom: spacing::UNIT,
          left: spacing::SPACE_2_5,
        })
        .style(|_| container::Style {
          background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.08))),
          border: Border {
            color: color::with_alpha(color::accent::PLASMA, 0.3),
            width: 1.0,
            radius: radius::SUBTLE.into(),
          },
          ..container::Style::default()
        })
        .into(),
      ])
      .spacing(spacing::UNIT + 1.0)
      .width(Length::Fill)
      .into(),
    );
  }

  let body = Column::with_children(body_children)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill);

  let footer = Row::with_children(vec![
    Space::new().width(Length::Fill).into(),
    modal_secondary_button(t!("assets.tree.cancel").into_owned(), Message::SaveFilterCancelled),
    modal_primary_button(t!("assets.tree.save_filter_title").into_owned(), valid),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let panel = container(
    Column::with_children(vec![
      modal_section(title.into()),
      rule::horizontal(),
      modal_section(body.into()),
      rule::horizontal(),
      modal_section(footer.into()),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fixed(420.0))
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.12),
      width: 1.0,
      radius: radius::CARD.into(),
    },
    ..container::Style::default()
  });

  container(panel)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

pub(super) fn context_menu_view(menu: &super::SavedFilterContextMenu) -> Element<'_, Message> {
  let items = vec![Item::danger(
    t!("assets.tree.delete"),
    Message::SavedFilterDeleted(menu.id),
  )];
  context_menu::context_menu(&menu.name, items, menu.anchor)
}

fn save_filter_name_placeholder() -> &'static str {
  static PLACEHOLDER: OnceLock<String> = OnceLock::new();
  PLACEHOLDER.get_or_init(|| t!("assets.tree.filter_name_placeholder").into_owned())
}

fn modal_section<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
  container(content)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn modal_close_button<'a>() -> Element<'a, Message> {
  button(
    text("\u{2715}")
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: spacing::UNIT + 1.0,
    right: spacing::SPACE_2,
    bottom: spacing::UNIT + 1.0,
    left: spacing::SPACE_2,
  })
  .on_press(Message::SaveFilterCancelled)
  .style(|_, status| {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hovered.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.06))),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.12),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      text_color: color::text::secondary(),
      ..button::Style::default()
    }
  })
  .into()
}

fn modal_secondary_button<'a>(label: String, message: Message) -> Element<'a, Message> {
  button(
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
  })
  .on_press(message)
  .style(|_, status| {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, if hovered { 0.28 } else { 0.1 }),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn modal_primary_button<'a>(label: String, enabled: bool) -> Element<'a, Message> {
  let label_color = if enabled {
    color::surface::BASE
  } else {
    color::text::tertiary()
  };
  let mut button = button(
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(label_color),
      }),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
  });
  if enabled {
    button = button.on_press(Message::SaveFilterConfirmed);
  }
  button
    .style(move |_, _| button::Style {
      background: Some(Background::Color(if enabled {
        color::accent::PLASMA
      } else {
        color::with_alpha(color::text::PRIMARY, 0.1)
      })),
      border: Border {
        color: if enabled {
          color::accent::PLASMA
        } else {
          color::with_alpha(color::text::PRIMARY, 0.1)
        },
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..button::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod db {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      features::assets::RosterPilot,
      store::{
        self, images,
        model::{
          Alliance, Bloodline, Character, CharacterAsset, Constellation, Corporation, Gender, ItemCategory, ItemGroup,
          Race, Region, SolarSystem, Station,
        },
        repo::{assets::replace_for_character, character::insert_with_org, sde},
      },
    };

    const CHARACTER_ID: i64 = 42;

    const CONSTELLATION_ID: i64 = 20_000_020;

    const CORP_ID: i64 = 90_000_001;

    const GROUP_ID: i64 = 25;

    const REGION_ID: i64 = 10_000_002;

    const STATION_ID: i64 = 60_003_760;

    const SYSTEM_ID: i64 = 30_000_142;

    async fn seed_character(db: &Database) {
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, CORP_ID, CHARACTER_ID, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(CORP_ID, "Test Corp", "TSC");
      corp.set_ceo_id(CHARACTER_ID);
      corp.set_creator_id(CHARACTER_ID);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, CORP_ID, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(CHARACTER_ID, 1, CORP_ID, 2, "2003-05-12", Gender::Male, "Pilot");
      insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    async fn seed_station(db: &Database) {
      sde::upsert_region(
        db,
        &Region {
          description: None,
          id: REGION_ID,
          name: "The Forge".to_owned(),
        },
      )
      .await
      .unwrap();
      sde::upsert_constellation(
        db,
        &Constellation {
          id: CONSTELLATION_ID,
          name: "Kimotoro".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          region_id: REGION_ID,
        },
      )
      .await
      .unwrap();
      sde::upsert_solar_system(
        db,
        &SolarSystem {
          constellation_id: CONSTELLATION_ID,
          id: SYSTEM_ID,
          name: "Jita".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          security_class: None,
          security_status: 0.9,
          star_id: None,
        },
      )
      .await
      .unwrap();
      sde::upsert_station(
        db,
        &Station {
          id: STATION_ID,
          max_dockable_ship_volume: 0.0,
          name: "Jita IV - Moon 4".to_owned(),
          office_rental_cost: 0.0,
          owner: None,
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          race_id: None,
          reprocessing_efficiency: 0.0,
          reprocessing_stations_take: 0.0,
          services: "[]".to_owned(),
          system_id: SYSTEM_ID,
          type_id: 587,
        },
      )
      .await
      .unwrap();
    }

    async fn seed_item_type(db: &Database, type_id: i64, name: &str) {
      let category = ItemCategory {
        id: GROUP_ID * 10,
        icon_id: None,
        name: "Ship".to_owned(),
        published: true,
      };
      let group = ItemGroup {
        category_id: category.id(),
        icon_id: None,
        id: GROUP_ID,
        name: "Frigate".to_owned(),
        published: true,
      };
      sde::upsert_item_category(db, &category).await.unwrap();
      sde::upsert_item_group(db, &group).await.unwrap();
      sqlx::query(
        "INSERT INTO item_types (id, group_id, description, name, published, icon_id, packaged_volume, volume) \
        VALUES (?, ?, 'Test item', ?, 1, ?, 2.5, 10.0)",
      )
      .bind(type_id)
      .bind(GROUP_ID)
      .bind(name)
      .bind(type_id + 1000)
      .execute(db.writer())
      .await
      .unwrap();
    }

    fn asset(item_id: i64, container_id: Option<i64>, is_container: bool) -> CharacterAsset {
      CharacterAsset {
        character_id: CHARACTER_ID,
        container_id,
        depth: container_id.map_or(0, |_| 1),
        is_active_ship: false,
        is_blueprint_copy: None,
        is_container,
        is_singleton: false,
        item_id,
        location_flag: "Hangar".to_owned(),
        location_id: container_id.unwrap_or(STATION_ID),
        location_type: container_id.map_or("station", |_| "item").to_owned(),
        name: None,
        quantity: 1,
        type_id: 587,
      }
    }

    fn pilot() -> RosterPilot {
      RosterPilot {
        corp: "TSC".to_owned(),
        granted_scopes: None,
        id: CHARACTER_ID,
        name: "Pilot".to_owned(),
        portrait: images::ImageState::Stale {
          id: CHARACTER_ID,
          kind: images::ImageKind::CharacterPortrait,
        },
      }
    }

    #[tokio::test]
    async fn it_builds_a_geo_tree_with_only_locations_no_containers() {
      let db = store::open_test().await.unwrap();
      seed_character(&db).await;
      seed_item_type(&db, 587, "Station Container").await;
      seed_station(&db).await;
      replace_for_character(
        &db,
        CHARACTER_ID,
        &[asset(100, None, true), asset(101, Some(100), false)],
      )
      .await
      .unwrap();

      let tree = load_geo_tree(&db, Scope::Character(CHARACTER_ID), &[pilot()], &[]).await;

      assert_eq!(tree.regions.len(), 1);
      let region = &tree.regions[0];
      assert_eq!(region.region_name, "The Forge");
      let system = &region.constellations[0].systems[0];
      assert_eq!(system.system_name, "Jita");
      assert_eq!(
        system.security_status,
        Some(0.9),
        "the system's security status threads from the geo query through to the tree node"
      );
      assert_eq!(
        system.locations.iter().map(|l| l.location_id).collect::<Vec<_>>(),
        [STATION_ID],
        "the station is the only location node; the container item is not a sidebar row"
      );
      assert_eq!(
        system.locations[0].item_count, 1,
        "only the top-level item at the station counts; the nested child is excluded"
      );
    }

    #[tokio::test]
    async fn it_builds_an_empty_tree_for_an_unresolvable_scope() {
      let db = store::open_test().await.unwrap();

      let tree = load_geo_tree(&db, Scope::Corporation(404), &[], &[]).await;

      assert_eq!(tree, GeoTree::default());
    }
  }

  mod render {
    use super::*;
    use crate::{
      features::assets::{GeoNodeKey, State},
      store::model::asset_query::{GeoConstellationNode, GeoLocationNode, GeoRegionNode, GeoSystemNode},
    };

    fn location(location_id: i64, label: &str) -> GeoLocationNode {
      GeoLocationNode {
        item_count: 1,
        location_id,
        location_label: Some(label.to_owned()),
        location_type: "station".to_owned(),
        value: 1_000.0,
      }
    }

    fn region(name: &str) -> GeoRegionNode {
      GeoRegionNode {
        constellations: vec![GeoConstellationNode {
          constellation_id: 20_000_020,
          constellation_name: "Kimotoro".to_owned(),
          item_count: 12,
          systems: vec![GeoSystemNode {
            item_count: 12,
            locations: vec![location(60_003_760, "Jita IV - Moon 4")],
            security_status: Some(0.9),
            system_id: 30_000_142,
            system_name: "Jita".to_owned(),
            value: 5_000.0,
          }],
          value: 5_000.0,
        }],
        item_count: 12,
        region_id: 10_000_002,
        region_name: name.to_owned(),
        value: 5_000.0,
      }
    }

    fn orphan() -> GeoLocationNode {
      GeoLocationNode {
        item_count: 1,
        location_id: 1_022_000_000_000,
        location_label: Some("Inaccessible Structure".to_owned()),
        location_type: "structure".to_owned(),
        value: 10.0,
      }
    }

    /// A bare region with an explicit id and rolled-up value, used to assert
    /// region-tier ordering under each sort mode.
    fn region_with(region_id: i64, name: &str, value: f64) -> GeoRegionNode {
      GeoRegionNode {
        constellations: Vec::new(),
        item_count: 1,
        region_id,
        region_name: name.to_owned(),
        value,
      }
    }

    fn saved_filter(id: i64, name: &str, query: &str, category: Option<&str>) -> crate::store::model::SavedAssetFilter {
      crate::store::model::SavedAssetFilter {
        category: category.map(str::to_owned),
        id,
        name: name.to_owned(),
        query: query.to_owned(),
      }
    }

    #[test]
    fn it_renders_a_disabled_modal_primary_button() {
      let _el: Element<'_, Message> = modal_primary_button("Save".to_owned(), false);
    }

    #[test]
    fn it_renders_no_action_buttons_in_the_saved_filters_header() {
      let actions = saved_filters_header_actions();

      assert!(actions.is_empty(), "expected no header actions, got {actions:?}");
      assert!(
        !actions.iter().any(|label| label.eq_ignore_ascii_case("+ new")),
        "the redundant '+ new' button must not appear in the saved-filters header",
      );
    }

    #[test]
    fn it_renders_the_saved_filters_section() {
      let state = State::new(crate::config::FeatureFlags::default());

      let _el: Element<'_, Message> = saved_filters_section(&state);
    }

    #[test]
    fn it_renders_a_saved_filter_row_with_a_truncated_long_query_hint() {
      let filter = saved_filter(4, "Long", "a very long search query that exceeds the hint limit", None);

      let _el: Element<'_, Message> = saved_filter_row(&filter, true);
    }

    #[test]
    fn it_renders_a_saved_filter_row_with_an_all_assets_hint() {
      let filter = saved_filter(3, "Everything", "", None);

      let _el: Element<'_, Message> = saved_filter_row(&filter, false);
    }

    #[test]
    fn it_renders_an_active_saved_filter_row_with_a_query_hint() {
      let filter = saved_filter(1, "Ships", "ship", None);

      let _el: Element<'_, Message> = saved_filter_row(&filter, true);
    }

    #[test]
    fn it_renders_an_empty_geo_tree_pane() {
      let state = State::new(crate::config::FeatureFlags::default());

      let _el: Element<'_, Message> = pane(&state);
    }

    #[test]
    fn it_renders_an_enabled_modal_primary_button() {
      let _el: Element<'_, Message> = modal_primary_button("Save".to_owned(), true);
    }

    #[tokio::test]
    async fn it_renders_an_expanded_region_with_its_descendants() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_geo_tree_for_test(GeoTree {
        orphans: Vec::new(),
        regions: vec![region("The Forge")],
      });
      let _ = crate::features::assets::update(&mut state, Message::GeoNodeToggled(GeoNodeKey::Region(10_000_002)), &db);

      let _el: Element<'_, Message> = pane(&state);
    }

    #[test]
    fn it_renders_an_inactive_saved_filter_row_with_a_category_hint() {
      let filter = saved_filter(2, "Modules", "", Some("module"));

      let _el: Element<'_, Message> = saved_filter_row(&filter, false);
    }

    #[test]
    fn it_renders_the_geo_tree_pane_with_regions_and_orphans() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_geo_tree_for_test(GeoTree {
        orphans: vec![orphan()],
        regions: vec![region("The Forge")],
      });

      let _el: Element<'_, Message> = pane(&state);
    }

    /// "Aaa" has the lower value but the alphabetically-first name; "Zzz" is the
    /// higher value. Each sort mode must rank them oppositely.
    fn two_regions() -> GeoTree {
      GeoTree {
        orphans: Vec::new(),
        regions: vec![
          region_with(10_000_001, "Zzz Region", 9_000.0),
          region_with(10_000_002, "Aaa Region", 1.0),
        ],
      }
    }

    #[tokio::test]
    async fn it_orders_locations_alphabetically_when_a_to_z_is_picked() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_geo_tree_for_test(two_regions());

      let _ = crate::features::assets::update(&mut state, Message::LocSortSelected(GeoSort::Alpha), &db);

      assert_eq!(
        state.geo_sort(),
        GeoSort::Alpha,
        "picking A\u{2013}Z makes Alpha the active mode"
      );
      assert_eq!(
        state
          .geo_tree()
          .regions
          .iter()
          .map(|r| r.region_name.as_str())
          .collect::<Vec<_>>(),
        ["Aaa Region", "Zzz Region"],
        "A\u{2013}Z orders regions alphabetically regardless of value"
      );
    }

    #[tokio::test]
    async fn it_orders_locations_by_descending_value_when_value_is_picked() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_geo_tree_for_test(two_regions());
      // Flip to Alpha first so the Value pick is an observable change.
      let _ = crate::features::assets::update(&mut state, Message::LocSortSelected(GeoSort::Alpha), &db);

      let _ = crate::features::assets::update(&mut state, Message::LocSortSelected(GeoSort::Value), &db);

      assert_eq!(
        state.geo_sort(),
        GeoSort::Value,
        "picking Value makes Value the active mode"
      );
      assert_eq!(
        state
          .geo_tree()
          .regions
          .iter()
          .map(|r| r.region_name.as_str())
          .collect::<Vec<_>>(),
        ["Zzz Region", "Aaa Region"],
        "Value orders regions by rolled-up ISK descending"
      );
    }
  }
}
