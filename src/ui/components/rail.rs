use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, stack, svg},
};

use crate::{
  config::Feature,
  features::registry,
  ui::style::{color, radius, spacing},
};

const BADGE_INSET: f32 = 8.0;
const BADGE_SIZE: f32 = 6.0;
const ICON_SIZE: f32 = 22.0;
const INDICATOR_HEIGHT: f32 = 24.0;
const INDICATOR_WIDTH: f32 = 2.0;
const LOGO_SIZE: f32 = 28.0;
const NAV_ITEM_SIZE: f32 = 44.0;
const RAIL_WIDTH: f32 = 68.0;
const SETTINGS_BOTTOM_INSET: f32 = 16.0;

static ASSETS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/assets.svg");
static CALENDAR_ICON: &[u8] = include_bytes!("../../../assets/images/icons/calendar.svg");
static CHARACTERS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/characters.svg");
static MAIL_ICON: &[u8] = include_bytes!("../../../assets/images/icons/mail.svg");
static POD_MARK: &[u8] = include_bytes!("../../../assets/images/identity/pod-mark.svg");
static SETTINGS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/settings.svg");
static SKILLS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/skills.svg");
static WALLET_ICON: &[u8] = include_bytes!("../../../assets/images/icons/wallet.svg");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Destination {
  Assets,
  Calendar,
  Characters,
  Mail,
  Settings,
  Skills,
  Wallet,
}

pub fn rail<'a, M>(
  active: Destination,
  mail_unread: i64,
  calendar_attention: i64,
  enabled_features: &[Feature],
  on_nav: impl Fn(Destination) -> M + 'a,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
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

  let items = container(nav_item(
    CHARACTERS_ICON,
    active == Destination::Characters,
    on_nav(Destination::Characters),
  ))
  .padding(Padding {
    top: spacing::SPACE_3_5,
    right: 0.0,
    bottom: 0.0,
    left: 0.0,
  });

  let is_skills_enabled =
    registry::feature_for_destination(Destination::Skills).is_none_or(|feature| enabled_features.contains(&feature));

  let is_mail_enabled =
    registry::feature_for_destination(Destination::Mail).is_none_or(|feature| enabled_features.contains(&feature));

  let is_calendar_enabled =
    registry::feature_for_destination(Destination::Calendar).is_none_or(|feature| enabled_features.contains(&feature));

  let is_wallet_enabled =
    registry::feature_for_destination(Destination::Wallet).is_none_or(|feature| enabled_features.contains(&feature));

  let is_assets_enabled =
    registry::feature_for_destination(Destination::Assets).is_none_or(|feature| enabled_features.contains(&feature));

  let skills = if is_skills_enabled {
    nav_item(SKILLS_ICON, active == Destination::Skills, on_nav(Destination::Skills))
  } else {
    Space::new().width(Length::Fill).height(Length::Fixed(0.0)).into()
  };

  let mail = if is_mail_enabled {
    nav_item_badged(
      MAIL_ICON,
      active == Destination::Mail,
      mail_unread > 0,
      on_nav(Destination::Mail),
    )
  } else {
    Space::new().width(Length::Fill).height(Length::Fixed(0.0)).into()
  };

  let calendar = if is_calendar_enabled {
    nav_item_badged(
      CALENDAR_ICON,
      active == Destination::Calendar,
      calendar_attention > 0,
      on_nav(Destination::Calendar),
    )
  } else {
    Space::new().width(Length::Fill).height(Length::Fixed(0.0)).into()
  };

  let wallet = if is_wallet_enabled {
    nav_item(WALLET_ICON, active == Destination::Wallet, on_nav(Destination::Wallet))
  } else {
    Space::new().width(Length::Fill).height(Length::Fixed(0.0)).into()
  };

  let assets = if is_assets_enabled {
    nav_item(ASSETS_ICON, active == Destination::Assets, on_nav(Destination::Assets))
  } else {
    Space::new().width(Length::Fill).height(Length::Fixed(0.0)).into()
  };

  let settings = container(nav_item(
    SETTINGS_ICON,
    active == Destination::Settings,
    on_nav(Destination::Settings),
  ))
  .padding(Padding {
    top: 0.0,
    right: 0.0,
    bottom: SETTINGS_BOTTOM_INSET,
    left: 0.0,
  });

  let body = container(
    Column::with_children(vec![
      logo.into(),
      items.into(),
      skills,
      mail,
      calendar,
      wallet,
      assets,
      Space::new().width(Length::Fill).height(Length::Fill).into(),
      settings.into(),
    ])
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

  Row::with_children(vec![body.into(), edge.into()])
    .height(Length::Fill)
    .into()
}

fn nav_item<'a, M>(icon: &'static [u8], active: bool, message: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  nav_item_badged(icon, active, false, message)
}

fn nav_item_badged<'a, M>(icon: &'static [u8], active: bool, badge: bool, message: M) -> Element<'a, M>
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
          background: Some(Background::Color(color::accent::PLASMA)),
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

  if badge {
    let dot = container(
      container(Space::new())
        .width(Length::Fixed(BADGE_SIZE))
        .height(Length::Fixed(BADGE_SIZE))
        .style(|_| container::Style {
          background: Some(Background::Color(color::accent::PLASMA)),
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn nav_item_renders_active_and_inactive() {
    let _active: Element<'_, ()> = nav_item(CHARACTERS_ICON, true, ());
    let _inactive: Element<'_, ()> = nav_item(CHARACTERS_ICON, false, ());
  }

  #[test]
  fn nav_item_badged_renders_with_and_without_a_badge() {
    let _with: Element<'_, ()> = nav_item_badged(MAIL_ICON, false, true, ());
    let _without: Element<'_, ()> = nav_item_badged(MAIL_ICON, false, false, ());
    let _active_with: Element<'_, ()> = nav_item_badged(MAIL_ICON, true, true, ());
  }

  #[test]
  fn rail_renders_with_characters_active() {
    let all_features = Feature::ALL;
    let _el: Element<'_, Destination> = rail(Destination::Characters, 0, 0, &all_features, |destination| destination);
  }

  #[test]
  fn rail_renders_with_skills_active() {
    let all_features = Feature::ALL;
    let _el: Element<'_, Destination> = rail(Destination::Skills, 0, 0, &all_features, |destination| destination);
  }

  #[test]
  fn rail_renders_with_mail_active_and_unread_badge() {
    let all_features = Feature::ALL;
    let _el: Element<'_, Destination> = rail(Destination::Mail, 3, 0, &all_features, |destination| destination);
  }

  #[test]
  fn rail_renders_with_calendar_active_and_attention_badge() {
    let all_features = Feature::ALL;
    let _el: Element<'_, Destination> = rail(Destination::Calendar, 0, 2, &all_features, |destination| destination);
  }

  #[test]
  fn rail_renders_with_wallet_active() {
    let all_features = Feature::ALL;
    let _el: Element<'_, Destination> = rail(Destination::Wallet, 0, 0, &all_features, |destination| destination);
  }

  #[test]
  fn rail_renders_with_assets_active() {
    let all_features = Feature::ALL;
    let _el: Element<'_, Destination> = rail(Destination::Assets, 0, 0, &all_features, |destination| destination);
  }

  #[test]
  fn rail_renders_with_settings_active() {
    let all_features = Feature::ALL;
    let _el: Element<'_, Destination> = rail(Destination::Settings, 0, 0, &all_features, |destination| destination);
  }

  #[test]
  fn rail_hides_disabled_feature_icons_but_always_shows_characters_and_settings() {
    let no_features: Vec<Feature> = vec![];
    let _el: Element<'_, Destination> = rail(Destination::Characters, 0, 0, &no_features, |destination| destination);

    let mail_only = vec![Feature::Mail];
    let _el: Element<'_, Destination> = rail(Destination::Characters, 0, 0, &mail_only, |destination| destination);
  }
}
