use iced::{
  Background, Border, Element, Event, Length, Padding, Rectangle, Size, Vector,
  advanced::{
    Clipboard, Layout, Shell, Widget,
    layout::{Limits, Node},
    mouse,
    overlay::{self, Element as OverlayElement},
    renderer,
    widget::{Operation, Tree},
  },
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, mouse_area, stack, svg, text},
};
use serde::{Deserialize, Serialize};

use crate::{
  config::{CascadeMode, Feature, FeatureFlags, NavLocation},
  features::shell::{nav_catalog, registry},
  ui::{
    components::overlay_layer::OverlayLayer,
    style::{color, radius, spacing, typography},
  },
};

const BADGE_INSET: f32 = 8.0;
const BADGE_SIZE: f32 = 6.0;
const FLYOUT_GAP: f32 = 10.0;
const FLYOUT_MIN_WIDTH: f32 = 212.0;
/// Upper bound on the flyout panel width so it stays a narrow side-anchored panel (matching the
/// design's `minWidth: 212`) instead of stretching to fill the viewport.
const FLYOUT_MAX_WIDTH: f32 = 260.0;
const ICON_SIZE: f32 = 22.0;
const INDICATOR_HEIGHT: f32 = 24.0;
const INDICATOR_WIDTH: f32 = 2.0;
const LOGO_SIZE: f32 = 28.0;
const NAV_ITEM_SIZE: f32 = 44.0;
pub const RAIL_WIDTH: f32 = 68.0;
const SETTINGS_BOTTOM_INSET: f32 = 16.0;
const SUB_ICON_SIZE: f32 = 16.0;
const SUB_RAIL_HEADER_HEIGHT: f32 = 92.0;
const SUB_RAIL_INDICATOR_HEIGHT: f32 = 22.0;
const SUB_RAIL_INDICATOR_WIDTH: f32 = 2.0;
const SUB_RAIL_ROW_ICON_SIZE: f32 = 17.0;
const SUB_RAIL_WIDTH: f32 = 206.0;

static ASSETS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/assets.svg");
static BELL_ICON: &[u8] = include_bytes!("../../../assets/images/icons/bell.svg");
static CALENDAR_ICON: &[u8] = include_bytes!("../../../assets/images/icons/calendar.svg");
static INDUSTRY_ICON: &[u8] = include_bytes!("../../../assets/images/icons/industry.svg");
static MAIL_ICON: &[u8] = include_bytes!("../../../assets/images/icons/mail.svg");
static MARKET_ICON: &[u8] = include_bytes!("../../../assets/images/icons/storefront.svg");
static PALETTE_ICON: &[u8] = include_bytes!("../../../assets/images/icons/slash.svg");
static POD_MARK: &[u8] = include_bytes!("../../../assets/images/identity/pod-mark.svg");
static ROSTER_ICON: &[u8] = include_bytes!("../../../assets/images/icons/roster.svg");
static SETTINGS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/settings.svg");
static SKILLS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/skills.svg");
static WALLET_ICON: &[u8] = include_bytes!("../../../assets/images/icons/wallet.svg");

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Destination {
  Assets,
  Calendar,
  Roster,
  Industry,
  Mail,
  Market,
  Settings,
  Skills,
  Wallet,
}

impl Destination {
  pub const REORDERABLE: [Destination; 8] = [
    Destination::Roster,
    Destination::Skills,
    Destination::Industry,
    Destination::Mail,
    Destination::Calendar,
    Destination::Wallet,
    Destination::Market,
    Destination::Assets,
  ];

  pub fn icon(self) -> &'static [u8] {
    icon_for(self)
  }

  pub fn label(self) -> String {
    match self {
      Destination::Assets => t!("nav.destination.assets"),
      Destination::Calendar => t!("nav.destination.calendar"),
      Destination::Roster => t!("nav.destination.roster"),
      Destination::Industry => t!("nav.destination.industry"),
      Destination::Mail => t!("nav.destination.mail"),
      Destination::Market => t!("nav.destination.market"),
      Destination::Settings => t!("nav.destination.settings"),
      Destination::Skills => t!("nav.destination.skills"),
      Destination::Wallet => t!("nav.destination.wallet"),
    }
    .into_owned()
  }
}

pub struct RailProps<'a> {
  pub active: Destination,
  pub active_sub: Option<&'static str>,
  pub calendar_attention: i64,
  pub cascade_mode: CascadeMode,
  pub enabled_features: &'a [Feature],
  pub feature_flags: FeatureFlags,
  pub hovered: Option<Destination>,
  pub mail_unread: i64,
  pub market_outbid: i64,
  pub nav_location: NavLocation,
  pub notifications_unread: i64,
  pub rail_order: &'a [Destination],
}

pub fn rail<'a, M>(
  props: RailProps<'_>,
  on_nav: impl Fn(Destination) -> M + 'a,
  on_hover: impl Fn(Option<Destination>) -> M + Clone + 'a,
  on_sub_nav: impl Fn(Destination, &'static str) -> M + Clone + 'a,
  on_notifications: M,
  on_palette: M,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let RailProps {
    active,
    active_sub,
    calendar_attention,
    cascade_mode,
    enabled_features,
    feature_flags,
    hovered,
    mail_unread,
    market_outbid,
    nav_location,
    notifications_unread,
    rail_order,
  } = props;

  let flyouts_enabled = cascade_mode == CascadeMode::Flyout;

  let logo_cell = container(
    svg(svg::Handle::from_memory(POD_MARK))
      .width(LOGO_SIZE)
      .height(LOGO_SIZE),
  )
  .width(Length::Fill)
  .height(Length::Fixed(spacing::layout::HEADER_HEIGHT))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center);

  let header_rule = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .height(Length::Fixed(1.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
      ..container::Style::default()
    });

  let logo = container(Column::with_children(vec![logo_cell.into(), header_rule.into()]).width(Length::Fill))
    .width(Length::Fixed(RAIL_WIDTH));

  let mut column_children: Vec<Element<'a, M>> = vec![logo.into()];
  let mut is_first_item = true;

  for &destination in rail_order {
    let is_enabled =
      registry::feature_for_destination(destination).is_none_or(|feature| enabled_features.contains(&feature));

    if !is_enabled {
      continue;
    }

    let badge = match destination {
      Destination::Calendar if calendar_attention > 0 => Some(color::accent()),
      Destination::Mail if mail_unread > 0 => Some(color::accent()),
      // Outbid is a lose-money alert, so its rail dot is danger red rather than the informational blue.
      Destination::Market if market_outbid > 0 => Some(color::status::DANGER),
      _ => None,
    };

    let is_active = active == destination;
    let icon = nav_item_badged(icon_for(destination), is_active, badge, on_nav(destination));
    let item = if flyouts_enabled {
      wrap_with_flyout(
        icon,
        destination,
        hovered == Some(destination),
        is_active.then_some(active_sub).flatten(),
        feature_flags,
        false,
        nav_location,
        on_hover.clone(),
        on_sub_nav.clone(),
      )
    } else {
      icon
    };

    let cell = if is_first_item {
      container(item).padding(Padding {
        top: spacing::SPACE_3_5,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
      })
    } else {
      container(item)
    };

    is_first_item = false;
    column_children.push(cell.into());
  }

  let settings_active = active == Destination::Settings;
  let settings_icon = nav_item(SETTINGS_ICON, settings_active, on_nav(Destination::Settings));
  let settings_item = if flyouts_enabled {
    wrap_with_flyout(
      settings_icon,
      Destination::Settings,
      hovered == Some(Destination::Settings),
      settings_active.then_some(active_sub).flatten(),
      feature_flags,
      true,
      nav_location,
      on_hover.clone(),
      on_sub_nav,
    )
  } else {
    settings_icon
  };
  let settings = container(settings_item).padding(Padding {
    top: 0.0,
    right: 0.0,
    bottom: SETTINGS_BOTTOM_INSET,
    left: 0.0,
  });

  column_children.push(Space::new().width(Length::Fill).height(Length::Fill).into());
  column_children.push(nav_item_count_badged(
    BELL_ICON,
    false,
    notifications_unread,
    on_notifications,
  ));
  column_children.push(nav_item(PALETTE_ICON, false, on_palette));
  column_children.push(settings.into());

  let body = container(
    Column::with_children(column_children)
      .width(Length::Fill)
      .align_x(Horizontal::Center),
  )
  .width(Length::Fixed(RAIL_WIDTH))
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::NAVIGATION)),
    ..container::Style::default()
  });

  let edge = container(Space::new().width(Length::Fixed(1.0)).height(Length::Fill))
    .width(Length::Fixed(1.0))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::state::OVERLAY_DARK)),
      ..container::Style::default()
    });

  let children: Vec<Element<'a, M>> = match nav_location {
    NavLocation::Left => vec![body.into(), edge.into()],
    NavLocation::Right => vec![edge.into(), body.into()],
  };

  Row::with_children(children).height(Length::Fill).into()
}

pub fn sub_rail<'a, M>(
  active: Destination,
  active_sub: Option<&'static str>,
  feature_flags: FeatureFlags,
  nav_location: NavLocation,
  on_sub_nav: impl Fn(Destination, &'static str) -> M + Clone + 'a,
) -> Option<Element<'a, M>>
where
  M: Clone + 'a,
{
  let section = nav_catalog::section(active)?;

  let head_icon = svg(svg::Handle::from_memory(section.icon()))
    .width(Length::Fixed(SUB_RAIL_ROW_ICON_SIZE))
    .height(Length::Fixed(SUB_RAIL_ROW_ICON_SIZE))
    .style(|_, _| svg::Style {
      color: Some(color::accent()),
    });
  let head_label = text(section.label())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let title = Row::with_children(vec![head_icon.into(), head_label.into()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2_5);
  let kicker = text(section.kicker())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let head_rule = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .height(Length::Fixed(1.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    });
  let head_cell = container(
    Column::with_children(vec![title.into(), kicker.into()])
      .spacing(spacing::UNIT)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fixed(SUB_RAIL_HEADER_HEIGHT))
  .padding(Padding {
    top: 0.0,
    right: spacing::SPACE_4_5,
    bottom: 0.0,
    left: spacing::SPACE_4_5,
  })
  .align_y(Vertical::Center);
  let head = Column::with_children(vec![head_cell.into(), head_rule.into()]).width(Length::Fill);

  let visible: Vec<&'static nav_catalog::SubSection> = section
    .sub_sections
    .iter()
    .filter(|sub| sub.is_enabled(&feature_flags))
    .collect();
  let mut rows: Vec<Element<'a, M>> = Vec::new();
  if visible.is_empty() {
    rows.push(sub_rail_empty_state());
  } else {
    for sub in visible {
      let is_active = active_sub == Some(sub.id);
      rows.push(sub_rail_row(sub, is_active, active, nav_location, on_sub_nav.clone()));
    }
  }

  let list = container(Column::with_children(rows).spacing(2.0).width(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_2_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_2_5,
    });

  let body = container(Column::with_children(vec![head.into(), list.into()]).width(Length::Fill))
    .width(Length::Fixed(SUB_RAIL_WIDTH))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    });

  let edge = container(Space::new().width(Length::Fixed(1.0)).height(Length::Fill))
    .width(Length::Fixed(1.0))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    });

  let children: Vec<Element<'a, M>> = match nav_location {
    NavLocation::Left => vec![body.into(), edge.into()],
    NavLocation::Right => vec![edge.into(), body.into()],
  };

  Some(Row::with_children(children).height(Length::Fill).into())
}

fn sub_rail_empty_state<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  container(
    text(t!("common.rail.no_sub_sections"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary())),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3_5,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_3,
  })
  .into()
}

fn sub_rail_row<'a, M>(
  sub: &'static nav_catalog::SubSection,
  active: bool,
  destination: Destination,
  nav_location: NavLocation,
  on_sub_nav: impl Fn(Destination, &'static str) -> M + 'a,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let icon_color = if active {
    color::accent()
  } else {
    color::text::secondary()
  };
  let label_color = if active {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  };

  let icon = svg(svg::Handle::from_memory(sub.icon))
    .width(Length::Fixed(SUB_RAIL_ROW_ICON_SIZE))
    .height(Length::Fixed(SUB_RAIL_ROW_ICON_SIZE))
    .style(move |_, _| svg::Style {
      color: Some(icon_color),
    });
  let label = text(sub.label())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(label_color));

  let row = container(
    Row::with_children(vec![icon.into(), label.into()])
      .align_y(Vertical::Center)
      .spacing(spacing::SPACE_3),
  )
  .width(Length::Fill);

  let inner: Element<'a, M> = if active {
    let strip = container(
      container(Space::new())
        .width(Length::Fixed(SUB_RAIL_INDICATOR_WIDTH))
        .height(Length::Fixed(SUB_RAIL_INDICATOR_HEIGHT))
        .style(|_| container::Style {
          background: Some(Background::Color(color::accent())),
          border: Border {
            radius: radius::SUBTLE.into(),
            ..Border::default()
          },
          ..container::Style::default()
        }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(match nav_location {
      NavLocation::Left => Horizontal::Left,
      NavLocation::Right => Horizontal::Right,
    })
    .align_y(Vertical::Center);
    stack(vec![row.into(), strip.into()]).width(Length::Fill).into()
  } else {
    row.into()
  };

  button(inner)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3,
    })
    .on_press(on_sub_nav(destination, sub.id))
    .style(move |_, status| {
      let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
      let background = if active {
        Some(Background::Color(color::with_alpha(color::accent(), 0.12)))
      } else if hovered {
        Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.05)))
      } else {
        None
      };
      button::Style {
        background,
        border: Border {
          radius: radius::SUBTLE.into(),
          ..Border::default()
        },
        ..button::Style::default()
      }
    })
    .into()
}

#[allow(clippy::too_many_arguments)]
fn wrap_with_flyout<'a, M>(
  icon: Element<'a, M>,
  destination: Destination,
  is_hovered: bool,
  active_sub: Option<&'static str>,
  feature_flags: FeatureFlags,
  open_up: bool,
  nav_location: NavLocation,
  on_hover: impl Fn(Option<Destination>) -> M + Clone + 'a,
  on_sub_nav: impl Fn(Destination, &'static str) -> M + Clone + 'a,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let Some(section) = nav_catalog::section(destination) else {
    return icon;
  };
  if section.sub_sections.is_empty() {
    return icon;
  }

  let trigger: Element<'a, M> = mouse_area(icon)
    .on_enter(on_hover(Some(destination)))
    .on_exit(on_hover(None))
    .into();

  if !is_hovered {
    return trigger;
  }

  let panel = flyout_panel(
    section,
    active_sub,
    destination,
    feature_flags,
    nav_location,
    on_hover.clone(),
    on_sub_nav,
  );

  SideFlyout::new(trigger, panel, open_up).into()
}

fn flyout_panel<'a, M>(
  section: &'static nav_catalog::Section,
  active_id: Option<&'static str>,
  destination: Destination,
  feature_flags: FeatureFlags,
  nav_location: NavLocation,
  on_hover: impl Fn(Option<Destination>) -> M + Clone + 'a,
  on_sub_nav: impl Fn(Destination, &'static str) -> M + Clone + 'a,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let head_icon = svg(svg::Handle::from_memory(section.icon()))
    .width(Length::Fixed(SUB_ICON_SIZE))
    .height(Length::Fixed(SUB_ICON_SIZE))
    .style(|_, _| svg::Style {
      color: Some(color::accent()),
    });
  let head_label = text(section.label())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let kicker = text(section.kicker())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let head = container(
    Row::with_children(vec![
      head_icon.into(),
      head_label.into(),
      Space::new().width(Length::Fill).into(),
      kicker.into(),
    ])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2_5),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_2_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_2_5,
  });

  let mut children: Vec<Element<'a, M>> = vec![head.into(), divider()];
  for sub in section.sub_sections.iter().filter(|sub| sub.is_enabled(&feature_flags)) {
    let active = active_id == Some(sub.id);
    children.push(flyout_row(sub, active, destination, on_sub_nav.clone()));
  }

  let panel = container(Column::with_children(children).spacing(2.0).width(Length::Fill))
    .padding(spacing::UNIT)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    });

  // Pad the rail-facing side with the visual gap so the transparent gutter between the icon and the
  // panel is part of the hover region: moving the pointer from the icon onto the panel never crosses
  // dead space, so the flyout keeps the hover alive instead of snapping shut mid-gap.
  let (pad_left, pad_right) = match nav_location {
    NavLocation::Left => (FLYOUT_GAP, 0.0),
    NavLocation::Right => (0.0, FLYOUT_GAP),
  };
  let gutter = container(panel).padding(Padding {
    top: 0.0,
    right: pad_right,
    bottom: 0.0,
    left: pad_left,
  });

  mouse_area(gutter)
    .on_enter(on_hover(Some(destination)))
    .on_exit(on_hover(None))
    .into()
}

fn divider<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .padding(Padding {
      top: 0.0,
      right: 0.0,
      bottom: 4.0,
      left: 0.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.08))),
      ..container::Style::default()
    })
    .into()
}

fn flyout_row<'a, M>(
  sub: &'static nav_catalog::SubSection,
  active: bool,
  destination: Destination,
  on_sub_nav: impl Fn(Destination, &'static str) -> M + 'a,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let icon_color = if active {
    color::accent()
  } else {
    color::text::secondary()
  };
  let label_color = if active {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  };

  let icon = svg(svg::Handle::from_memory(sub.icon))
    .width(Length::Fixed(SUB_ICON_SIZE))
    .height(Length::Fixed(SUB_ICON_SIZE))
    .style(move |_, _| svg::Style {
      color: Some(icon_color),
    });
  let label = text(sub.label())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(label_color));

  let row = container(
    Row::with_children(vec![icon.into(), label.into()])
      .align_y(Vertical::Center)
      .spacing(spacing::SPACE_2_5),
  )
  .width(Length::Fill);

  button(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_2_5,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_2_5,
    })
    .on_press(on_sub_nav(destination, sub.id))
    .style(move |_, status| {
      let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
      let background = if active {
        Some(Background::Color(color::with_alpha(color::accent(), 0.12)))
      } else if hovered {
        Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.05)))
      } else {
        None
      };
      button::Style {
        background,
        border: Border {
          radius: radius::SUBTLE.into(),
          ..Border::default()
        },
        ..button::Style::default()
      }
    })
    .into()
}

fn icon_for(destination: Destination) -> &'static [u8] {
  match destination {
    Destination::Assets => ASSETS_ICON,
    Destination::Calendar => CALENDAR_ICON,
    Destination::Roster => ROSTER_ICON,
    Destination::Industry => INDUSTRY_ICON,
    Destination::Mail => MAIL_ICON,
    Destination::Market => MARKET_ICON,
    Destination::Settings => SETTINGS_ICON,
    Destination::Skills => SKILLS_ICON,
    Destination::Wallet => WALLET_ICON,
  }
}

fn nav_item<'a, M>(icon: &'static [u8], active: bool, message: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  nav_item_badged(icon, active, None, message)
}

fn nav_item_badged<'a, M>(icon: &'static [u8], active: bool, badge: Option<iced::Color>, message: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let icon_color = if active {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  };

  let cell = container(
    svg(svg::Handle::from_memory(icon))
      .width(Length::Fixed(ICON_SIZE))
      .height(Length::Fixed(ICON_SIZE))
      .style(move |_, _| svg::Style {
        color: Some(icon_color),
      }),
  )
  .width(Length::Fixed(NAV_ITEM_SIZE))
  .height(Length::Fixed(NAV_ITEM_SIZE))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: active.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
    border: Border {
      radius: radius::CONTROL.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  let mut layers: Vec<Element<'a, M>> = vec![cell.into()];

  if active {
    let indicator = container(
      container(Space::new())
        .width(Length::Fixed(INDICATOR_WIDTH))
        .height(Length::Fixed(INDICATOR_HEIGHT))
        .style(|_| container::Style {
          background: Some(Background::Color(color::accent())),
          border: Border {
            radius: radius::SUBTLE.into(),
            ..Border::default()
          },
          ..container::Style::default()
        }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Left)
    .align_y(Vertical::Center);
    layers.push(indicator.into());
  }

  if let Some(badge_color) = badge {
    let dot = container(
      container(Space::new())
        .width(Length::Fixed(BADGE_SIZE))
        .height(Length::Fixed(BADGE_SIZE))
        .style(move |_| container::Style {
          background: Some(Background::Color(badge_color)),
          border: Border {
            radius: (BADGE_SIZE / 2.0).into(),
            ..Border::default()
          },
          ..container::Style::default()
        }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Right)
    .align_y(Vertical::Top)
    .padding(Padding {
      top: BADGE_INSET,
      right: BADGE_INSET,
      bottom: 0.0,
      left: 0.0,
    });
    layers.push(dot.into());
  }

  let inner: Element<'a, M> = if layers.len() == 1 {
    layers.into_iter().next().expect("one layer")
  } else {
    stack(layers)
      .width(Length::Fixed(NAV_ITEM_SIZE))
      .height(Length::Fixed(NAV_ITEM_SIZE))
      .into()
  };

  button(inner)
    .padding(0)
    .on_press(message)
    .style(|_, _| button::Style {
      background: Some(Background::Color(iced::Color::TRANSPARENT)),
      ..button::Style::default()
    })
    .into()
}

fn nav_item_count_badged<'a, M>(icon: &'static [u8], active: bool, count: i64, message: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  if count <= 0 {
    return nav_item(icon, active, message);
  }

  let icon_color = if active {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  };
  let cell = container(
    svg(svg::Handle::from_memory(icon))
      .width(Length::Fixed(ICON_SIZE))
      .height(Length::Fixed(ICON_SIZE))
      .style(move |_, _| svg::Style {
        color: Some(icon_color),
      }),
  )
  .width(Length::Fixed(NAV_ITEM_SIZE))
  .height(Length::Fixed(NAV_ITEM_SIZE))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center);

  let label = if count > 9 { "9+".to_owned() } else { count.to_string() };
  let pill = container(
    container(
      text(label)
        .font(typography::mono::MEDIUM)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::surface::NAVIGATION),
        }),
    )
    .padding(Padding {
      top: 0.0,
      right: 4.0,
      bottom: 0.0,
      left: 4.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent())),
      border: Border {
        radius: 999.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Right)
  .align_y(Vertical::Top)
  .padding(Padding {
    top: 4.0,
    right: 4.0,
    bottom: 0.0,
    left: 0.0,
  });

  let inner = stack(vec![cell.into(), pill.into()])
    .width(Length::Fixed(NAV_ITEM_SIZE))
    .height(Length::Fixed(NAV_ITEM_SIZE));

  button(inner)
    .padding(0)
    .on_press(message)
    .style(|_, _| button::Style {
      background: Some(Background::Color(iced::Color::TRANSPARENT)),
      ..button::Style::default()
    })
    .into()
}

/// The flyout's keep-open predicate: the popover must stay open while the cursor rests anywhere over
/// the panel (its full layout bounds, including the transparent icon→panel gutter). When this is
/// true the overlay captures the event so the rail icon's `on_exit` close never fires; when it is
/// false (cursor over neither icon nor panel) the rail's leave-grace timer is left to close it.
fn flyout_keeps_open(cursor: mouse::Cursor, panel_bounds: Rectangle) -> bool {
  cursor.is_over(panel_bounds)
}

struct SideFlyout<'a, M, Theme, Renderer> {
  open_up: bool,
  popover: Element<'a, M, Theme, Renderer>,
  underlay: Element<'a, M, Theme, Renderer>,
}

impl<'a, M, Theme, Renderer> SideFlyout<'a, M, Theme, Renderer>
where
  Renderer: iced::advanced::Renderer,
{
  fn new(
    underlay: impl Into<Element<'a, M, Theme, Renderer>>,
    popover: impl Into<Element<'a, M, Theme, Renderer>>,
    open_up: bool,
  ) -> Self {
    Self {
      open_up,
      popover: popover.into(),
      underlay: underlay.into(),
    }
  }
}

impl<M, Theme, Renderer> Widget<M, Theme, Renderer> for SideFlyout<'_, M, Theme, Renderer>
where
  M: Clone,
  Renderer: iced::advanced::Renderer,
{
  fn children(&self) -> Vec<Tree> {
    vec![Tree::new(&self.underlay), Tree::new(&self.popover)]
  }

  fn diff(&self, tree: &mut Tree) {
    tree.diff_children(&[&self.underlay, &self.popover]);
  }

  fn size(&self) -> Size<Length> {
    self.underlay.as_widget().size()
  }

  fn size_hint(&self) -> Size<Length> {
    self.underlay.as_widget().size_hint()
  }

  fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
    self
      .underlay
      .as_widget_mut()
      .layout(&mut tree.children[0], renderer, limits)
  }

  fn update(
    &mut self,
    tree: &mut Tree,
    event: &Event,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    renderer: &Renderer,
    clipboard: &mut dyn Clipboard,
    shell: &mut Shell<'_, M>,
    viewport: &Rectangle,
  ) {
    self.underlay.as_widget_mut().update(
      &mut tree.children[0],
      event,
      layout,
      cursor,
      renderer,
      clipboard,
      shell,
      viewport,
    );
  }

  fn draw(
    &self,
    tree: &Tree,
    renderer: &mut Renderer,
    theme: &Theme,
    style: &renderer::Style,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
  ) {
    self
      .underlay
      .as_widget()
      .draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
  }

  fn mouse_interaction(
    &self,
    tree: &Tree,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
    renderer: &Renderer,
  ) -> mouse::Interaction {
    self
      .underlay
      .as_widget()
      .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
  }

  fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
    self
      .underlay
      .as_widget_mut()
      .operate(&mut tree.children[0], layout, renderer, operation);
  }

  fn overlay<'b>(
    &'b mut self,
    tree: &'b mut Tree,
    layout: Layout<'b>,
    _renderer: &Renderer,
    viewport: &Rectangle,
    translation: Vector,
  ) -> Option<OverlayElement<'b, M, Theme, Renderer>> {
    let (_, popover_tree) = tree.children.split_at_mut(1);

    Some(OverlayElement::new(Box::new(FlyoutOverlay {
      bounds: layout.bounds() + translation,
      open_up: self.open_up,
      popover: &mut self.popover,
      tree: &mut popover_tree[0],
      viewport: *viewport,
    })))
  }
}

struct FlyoutOverlay<'a, 'b, M, Theme, Renderer> {
  bounds: Rectangle,
  open_up: bool,
  popover: &'a mut Element<'b, M, Theme, Renderer>,
  tree: &'a mut Tree,
  viewport: Rectangle,
}

impl<M, Theme, Renderer> overlay::Overlay<M, Theme, Renderer> for FlyoutOverlay<'_, '_, M, Theme, Renderer>
where
  M: Clone,
  Renderer: iced::advanced::Renderer,
{
  fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
    let max_height = if self.open_up {
      self.bounds.y + self.bounds.height
    } else {
      bounds.height - self.bounds.y
    }
    .max(1.0);
    // Constrain the panel to a narrow side-anchored width; never let it stretch to the viewport. The
    // popover carries the icon→panel gap as transparent internal padding (a contiguous hover
    // region), so allow that extra `FLYOUT_GAP` on top of the visible panel width.
    let max_width = (FLYOUT_MAX_WIDTH + FLYOUT_GAP).min(bounds.width).max(FLYOUT_MIN_WIDTH);
    let limits = Limits::new(Size::new(FLYOUT_MIN_WIDTH, 0.0), Size::new(max_width, max_height));

    let node = self.popover.as_widget_mut().layout(self.tree, renderer, &limits);
    let size = node.size();

    let right_x = self.bounds.x + self.bounds.width;
    let x = if right_x + size.width <= bounds.width {
      right_x
    } else {
      (self.bounds.x - size.width).max(0.0)
    };
    let y = if self.open_up {
      (self.bounds.y + self.bounds.height - size.height).max(0.0)
    } else {
      self.bounds.y.min((bounds.height - size.height).max(0.0))
    };

    node.move_to(iced::Point::new(x, y))
  }

  fn draw(
    &self,
    renderer: &mut Renderer,
    theme: &Theme,
    style: &renderer::Style,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
  ) {
    self
      .popover
      .as_widget()
      .draw(self.tree, renderer, theme, style, layout, cursor, &layout.bounds());
  }

  fn update(
    &mut self,
    event: &Event,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    renderer: &Renderer,
    clipboard: &mut dyn Clipboard,
    shell: &mut Shell<'_, M>,
  ) {
    let viewport = self.viewport;
    self
      .popover
      .as_widget_mut()
      .update(self.tree, event, layout, cursor, renderer, clipboard, shell, &viewport);

    // Keep the flyout open while the cursor rests anywhere over the panel. The panel's own
    // `mouse_area` publishes the keep-open message via `on_enter`, but that is not enough on its own:
    // in the same frame iced still feeds the *base* rail tree the live cursor, and the rail icon's
    // `mouse_area` — now that the pointer sits over the panel rather than the icon — fires `on_exit`
    // (`RailHover(None)`) which lands *after* the overlay's `on_enter` and wins, snapping the flyout
    // shut. Capturing the event whenever the cursor is over the panel makes iced skip that base-tree
    // update (see `UserInterface::update`: a `Captured` overlay status short-circuits the base
    // pass), so the spurious close never fires and the steady "cursor on panel" state stays open.
    if flyout_keeps_open(cursor, layout.bounds()) {
      shell.capture_event();
    }
  }

  fn mouse_interaction(&self, layout: Layout<'_>, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
    let inner = self
      .popover
      .as_widget()
      .mouse_interaction(self.tree, layout, cursor, &layout.bounds(), renderer);
    // Report a non-`None` interaction whenever the cursor is over the panel so iced masks the base
    // cursor (`Cursor::Unavailable`) for the rail tree, reinforcing that the hover belongs to the
    // flyout and not the icon underneath.
    if inner == mouse::Interaction::None && flyout_keeps_open(cursor, layout.bounds()) {
      mouse::Interaction::Idle
    } else {
      inner
    }
  }

  fn index(&self) -> f32 {
    OverlayLayer::RailCascade.z()
  }
}

impl<'a, M, Theme, Renderer> From<SideFlyout<'a, M, Theme, Renderer>> for Element<'a, M, Theme, Renderer>
where
  M: Clone + 'a,
  Theme: 'a,
  Renderer: iced::advanced::Renderer + 'a,
{
  fn from(flyout: SideFlyout<'a, M, Theme, Renderer>) -> Self {
    Element::new(flyout)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn props<'a>(active: Destination, features: &'a [Feature], order: &'a [Destination]) -> RailProps<'a> {
    RailProps {
      market_outbid: 0,
      active,
      active_sub: None,
      calendar_attention: 0,
      cascade_mode: CascadeMode::Flyout,
      enabled_features: features,
      feature_flags: FeatureFlags::default(),
      hovered: None,
      mail_unread: 0,
      nav_location: NavLocation::Left,
      notifications_unread: 0,
      rail_order: order,
    }
  }

  fn render(props: RailProps<'_>) -> Element<'_, Destination> {
    rail(
      props,
      |d| d,
      |_| Destination::Roster,
      |d, _| d,
      Destination::Roster,
      Destination::Roster,
    )
  }

  #[test]
  fn nav_item_badged_renders_with_and_without_a_badge() {
    let _with: Element<'_, ()> = nav_item_badged(MAIL_ICON, false, Some(color::accent()), ());
    let _without: Element<'_, ()> = nav_item_badged(MAIL_ICON, false, None, ());
    let _active_with: Element<'_, ()> = nav_item_badged(MAIL_ICON, true, Some(color::status::DANGER), ());
  }

  #[test]
  fn nav_item_renders_active_and_inactive() {
    let _active: Element<'_, ()> = nav_item(ROSTER_ICON, true, ());
    let _inactive: Element<'_, ()> = nav_item(ROSTER_ICON, false, ());
  }

  #[test]
  fn nav_item_count_badged_renders_with_and_without_a_count() {
    let _none: Element<'_, ()> = nav_item_count_badged(BELL_ICON, false, 0, ());
    let _some: Element<'_, ()> = nav_item_count_badged(BELL_ICON, false, 3, ());
    let _overflow: Element<'_, ()> = nav_item_count_badged(BELL_ICON, false, 42, ());
  }

  #[test]
  fn rail_renders_the_notification_bell_with_an_unread_badge() {
    let all_features = Feature::ALL;
    let order = Destination::REORDERABLE;
    let mut props = props(Destination::Roster, &all_features, &order);
    props.notifications_unread = 12;

    let _el: Element<'_, Destination> = render(props);
  }

  #[test]
  fn rail_docks_to_either_side() {
    let all_features = Feature::ALL;
    let order = Destination::REORDERABLE;
    let _left: Element<'_, Destination> = render(props(Destination::Roster, &all_features, &order));
    let mut right = props(Destination::Roster, &all_features, &order);
    right.nav_location = NavLocation::Right;
    let _right: Element<'_, Destination> = render(right);
  }

  #[test]
  fn rail_hides_disabled_feature_icons_but_always_shows_characters_and_settings() {
    let order = Destination::REORDERABLE;

    let no_features: Vec<Feature> = vec![];
    let _el: Element<'_, Destination> = render(props(Destination::Roster, &no_features, &order));

    let mail_only = vec![Feature::Mail];
    let _el: Element<'_, Destination> = render(props(Destination::Roster, &mail_only, &order));
  }

  #[test]
  fn rail_renders_a_reordered_rail() {
    let all_features = Feature::ALL;
    let order = [
      Destination::Wallet,
      Destination::Assets,
      Destination::Roster,
      Destination::Skills,
      Destination::Industry,
      Destination::Mail,
      Destination::Calendar,
    ];
    let _el: Element<'_, Destination> = render(props(Destination::Wallet, &all_features, &order));
  }

  #[test]
  fn rail_renders_with_a_disabled_item_in_the_middle_of_the_order() {
    let features: Vec<Feature> = Feature::ALL
      .into_iter()
      .filter(|&feature| feature != Feature::Industry)
      .collect();
    let order = Destination::REORDERABLE;
    let _el: Element<'_, Destination> = render(props(Destination::Roster, &features, &order));
  }

  #[test]
  fn rail_renders_each_active_destination() {
    let all_features = Feature::ALL;
    let order = Destination::REORDERABLE;
    for active in [
      Destination::Assets,
      Destination::Calendar,
      Destination::Roster,
      Destination::Industry,
      Destination::Mail,
      Destination::Market,
      Destination::Settings,
      Destination::Skills,
      Destination::Wallet,
    ] {
      let _el: Element<'_, Destination> = render(props(active, &all_features, &order));
    }
  }

  #[test]
  fn rail_renders_badges_for_mail_and_calendar() {
    let all_features = Feature::ALL;
    let order = Destination::REORDERABLE;
    let mut mail = props(Destination::Mail, &all_features, &order);
    mail.mail_unread = 3;
    let _mail: Element<'_, Destination> = render(mail);

    let mut calendar = props(Destination::Calendar, &all_features, &order);
    calendar.calendar_attention = 2;
    let _calendar: Element<'_, Destination> = render(calendar);
  }

  #[test]
  fn rail_opens_a_flyout_for_a_hovered_section_with_sub_sections() {
    let all_features = Feature::ALL;
    let order = Destination::REORDERABLE;
    let mut hovered = props(Destination::Wallet, &all_features, &order);
    hovered.hovered = Some(Destination::Wallet);
    hovered.active_sub = Some("budget");

    let _el: Element<'_, Destination> = render(hovered);
  }

  #[test]
  fn flyout_width_is_bounded_not_full_screen() {
    const {
      assert!(FLYOUT_MIN_WIDTH > 0.0);
      assert!(
        FLYOUT_MAX_WIDTH >= FLYOUT_MIN_WIDTH,
        "max width must not undercut the min width"
      );
      assert!(
        FLYOUT_MAX_WIDTH <= 320.0,
        "flyout must remain a narrow panel, not a full-width sheet"
      );
    }
  }

  #[test]
  fn flyout_stays_open_while_the_cursor_is_over_the_panel() {
    let panel = Rectangle {
      x: 68.0,
      y: 100.0,
      width: 222.0,
      height: 240.0,
    };

    let over_panel = mouse::Cursor::Available(iced::Point::new(150.0, 200.0));
    assert!(
      flyout_keeps_open(over_panel, panel),
      "resting on the panel keeps the flyout open"
    );

    let panel_edge = mouse::Cursor::Available(iced::Point::new(panel.x + 1.0, panel.y + 1.0));
    assert!(
      flyout_keeps_open(panel_edge, panel),
      "the rail-facing gutter edge of the panel still counts as over the panel"
    );

    let off_panel = mouse::Cursor::Available(iced::Point::new(500.0, 500.0));
    assert!(
      !flyout_keeps_open(off_panel, panel),
      "cursor over neither icon nor panel lets the flyout close"
    );

    assert!(
      !flyout_keeps_open(mouse::Cursor::Unavailable, panel),
      "an unavailable cursor cannot keep the flyout open"
    );
  }

  #[test]
  fn flyout_overlay_reports_the_rail_cascade_layer() {
    use iced::advanced::overlay::Overlay;
    use pretty_assertions::assert_eq;

    let mut popover: Element<'_, ()> = Space::new().into();
    let mut tree = Tree::empty();
    let flyout = FlyoutOverlay {
      bounds: Rectangle {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
      },
      open_up: false,
      popover: &mut popover,
      tree: &mut tree,
      viewport: Rectangle {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
      },
    };

    assert_eq!(flyout.index(), OverlayLayer::RailCascade.z());
  }

  #[test]
  fn flyout_panel_builds_for_either_dock_side() {
    let section = nav_catalog::section(Destination::Wallet).expect("wallet section");
    let _left: Element<'_, Destination> = flyout_panel(
      section,
      Some("budget"),
      Destination::Wallet,
      FeatureFlags::default(),
      NavLocation::Left,
      |_| Destination::Wallet,
      |d, _| d,
    );
    let _right: Element<'_, Destination> = flyout_panel(
      section,
      Some("budget"),
      Destination::Wallet,
      FeatureFlags::default(),
      NavLocation::Right,
      |_| Destination::Wallet,
      |d, _| d,
    );
  }

  #[test]
  fn rail_opens_the_settings_flyout_upward() {
    let all_features = Feature::ALL;
    let order = Destination::REORDERABLE;
    let mut hovered = props(Destination::Settings, &all_features, &order);
    hovered.hovered = Some(Destination::Settings);

    let _el: Element<'_, Destination> = render(hovered);
  }

  #[test]
  fn rail_renders_a_palette_button() {
    let all_features = Feature::ALL;
    let order = Destination::REORDERABLE;
    let _el: Element<'_, Destination> = render(props(Destination::Roster, &all_features, &order));
  }

  #[test]
  fn rail_with_cascade_off_is_a_plain_rail() {
    let all_features = Feature::ALL;
    let order = Destination::REORDERABLE;
    let mut off = props(Destination::Wallet, &all_features, &order);
    off.cascade_mode = CascadeMode::None;
    off.hovered = Some(Destination::Wallet);

    let _el: Element<'_, Destination> = render(off);
  }

  #[test]
  fn rail_with_sub_rail_mode_does_not_open_a_flyout() {
    let all_features = Feature::ALL;
    let order = Destination::REORDERABLE;
    let mut sub_rail = props(Destination::Wallet, &all_features, &order);
    sub_rail.cascade_mode = CascadeMode::SubRail;
    sub_rail.hovered = Some(Destination::Wallet);

    let _el: Element<'_, Destination> = render(sub_rail);
  }

  #[test]
  fn sub_rail_renders_the_active_sections_sub_sections() {
    let el = sub_rail(
      Destination::Wallet,
      Some("budget"),
      FeatureFlags::default(),
      NavLocation::Left,
      |d, _| d,
    );

    assert!(el.is_some(), "a section with sub-sections renders a sub-rail");
  }

  #[test]
  fn sub_rail_renders_each_destinations_sub_sections() {
    for active in [
      Destination::Assets,
      Destination::Calendar,
      Destination::Roster,
      Destination::Industry,
      Destination::Mail,
      Destination::Market,
      Destination::Settings,
      Destination::Skills,
      Destination::Wallet,
    ] {
      let el = sub_rail(active, None, FeatureFlags::default(), NavLocation::Left, |d, _| d);
      assert!(el.is_some(), "{active:?} has a catalog section so its sub-rail renders");
    }
  }

  #[test]
  fn sub_rail_renders_an_empty_state_for_a_sectionless_destination() {
    let el = sub_rail(
      Destination::Mail,
      None,
      FeatureFlags::default(),
      NavLocation::Left,
      |d, _| d,
    );

    assert!(el.is_some(), "Mail renders the empty-state sub-rail");
  }

  #[test]
  fn sub_rail_docks_to_either_side() {
    let left = sub_rail(
      Destination::Wallet,
      Some("journal"),
      FeatureFlags::default(),
      NavLocation::Left,
      |d, _| d,
    );
    let right = sub_rail(
      Destination::Wallet,
      Some("journal"),
      FeatureFlags::default(),
      NavLocation::Right,
      |d, _| d,
    );

    assert!(left.is_some());
    assert!(right.is_some());
  }

  #[test]
  fn sub_rail_renders_without_an_active_sub_section() {
    let el = sub_rail(
      Destination::Wallet,
      None,
      FeatureFlags::default(),
      NavLocation::Left,
      |d, _| d,
    );

    assert!(el.is_some());
  }
}
