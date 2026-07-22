use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, text},
};

use super::{
  ChainTier, Colony, ColonyFactory, ExtractorHead, LaunchpadBuffer, Message,
  colonies::{commodity_letters, planet_color, planet_label, state_color, tier_badge, tier_color},
  jobs::{progress_bar, sec_pill},
};
use crate::ui::{
  components::icon::Icon,
  format::{fmt_count, fmt_duration, fmt_isk},
  style::{color, radius, spacing, typography},
};

const DRAWER_WIDTH: f32 = 540.0;
const HEAD_TILE: f32 = 30.0;
const HEADER_TILE: f32 = 46.0;
const PROGRESS_HEIGHT: f32 = 7.0;
const SECONDS_PER_DAY: i64 = 86_400;
const TILE_CHAIN: f32 = 22.0;

pub(super) fn drawer<'a>(colony: &'a Colony, now: DateTime<Utc>) -> Element<'a, Message> {
  let panel = Column::with_children(vec![header(colony), body(colony, now)])
    .width(Length::Fixed(DRAWER_WIDTH))
    .height(Length::Fill);

  container(panel)
    .width(Length::Fixed(DRAWER_WIDTH))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: color::rule_strong(),
        radius: 0.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn header<'a>(colony: &'a Colony) -> Element<'a, Message> {
  let bar = Row::with_children(vec![planet_tile(colony), identity(colony), close_button()])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  container(bar)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_4_5,
      bottom: spacing::SPACE_4_5,
      left: spacing::SPACE_6,
      right: spacing::SPACE_6,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        radius: 0.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn identity<'a>(colony: &'a Colony) -> Element<'a, Message> {
  let accent = planet_color(&colony.planet_type);
  let mut meta: Vec<Element<'a, Message>> = vec![type_tag(&colony.planet_type, accent)];
  if let Some(system) = &colony.system_name {
    meta.push(
      text(system.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }
  meta.push(sec_pill(colony.security));

  Column::with_children(vec![
    text(colony.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    Row::with_children(meta)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill)
  .into()
}

fn close_button<'a>() -> Element<'a, Message> {
  button(
    text(t!("industry.colony_detail.close"))
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::ColonyClosed)
  .style(|_, status| {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hovered.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.06))),
      border: Border {
        color: color::rule(),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: color::text::secondary(),
      ..button::Style::default()
    }
  })
  .into()
}

fn body<'a>(colony: &'a Colony, now: DateTime<Utc>) -> Element<'a, Message> {
  let mut sections: Vec<Element<'a, Message>> = vec![summary_band(colony, now), chain_section(colony)];
  if !colony.detail.heads.is_empty() {
    sections.push(heads_section(&colony.detail.heads, now));
  }
  if !colony.detail.factories.is_empty() {
    sections.push(factories_section(&colony.detail.factories));
  }
  sections.push(launchpad_section(&colony.detail.launchpad));

  let content = Column::with_children(sections)
    .spacing(spacing::SPACE_6)
    .width(Length::Fill);

  scrollable(container(content).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_4_5,
    bottom: spacing::SPACE_4_5,
    left: spacing::SPACE_6,
    right: spacing::SPACE_6,
  }))
  .style(crate::ui::style::control::scrollbar)
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn summary_band<'a>(colony: &'a Colony, now: DateTime<Utc>) -> Element<'a, Message> {
  let state = colony.state(now);
  let value = colony.value_per_day(now);
  let (value_text, value_color) = if value > 0.0 {
    (fmt_isk(value), color::status::ONLINE)
  } else {
    ("\u{2014}".to_owned(), color::text::tertiary())
  };

  let cells = [
    summary_cell(
      &t!("industry.colony_detail.summary_status"),
      state.label(),
      state_color(state),
    ),
    summary_cell(
      &t!("industry.colony_detail.summary_cc"),
      t!("industry.colony_detail.summary_cc_value", level => colony.cc_level()).into_owned(),
      color::text::PRIMARY,
    ),
    summary_cell(
      &t!("industry.colony_detail.summary_pins"),
      t!(
        "industry.colony_detail.summary_pins_value",
        used => colony.num_pins,
        max => colony.pin_capacity(),
      )
      .into_owned(),
      color::text::PRIMARY,
    ),
    summary_cell(&t!("industry.colony_detail.summary_value"), value_text, value_color),
  ];

  let mut children: Vec<Element<'a, Message>> = Vec::new();
  for (index, cell) in cells.into_iter().enumerate() {
    if index > 0 {
      children.push(divider());
    }
    children.push(cell);
  }

  container(Row::with_children(children).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn summary_cell<'a>(label: &str, value: String, value_color: Color) -> Element<'a, Message> {
  let cell = Column::with_children(vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    text(value)
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(value_color))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .width(Length::Fill);

  container(cell)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    })
    .into()
}

fn divider<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(1.0))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

fn chain_section<'a>(colony: &'a Colony) -> Element<'a, Message> {
  let mut ribbon: Vec<Element<'a, Message>> = Vec::new();
  let last = colony.detail.chain.len().saturating_sub(1);
  for (index, tier) in colony.detail.chain.iter().enumerate() {
    ribbon.push(chain_tier_card(tier));
    if index < last {
      ribbon.push(chain_arrow());
    }
  }

  Column::with_children(vec![
    section_label(&t!("industry.colony_detail.chain_title")),
    Row::with_children(ribbon)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .width(Length::Fill)
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .width(Length::Fill)
  .into()
}

fn chain_tier_card<'a>(tier: &ChainTier) -> Element<'a, Message> {
  let mut rows: Vec<Element<'a, Message>> = vec![tier_badge(tier.tier)];
  for commodity in &tier.commodities {
    rows.push(
      Row::with_children(vec![
        commodity_tile(Some(&commodity.name), commodity.tier, TILE_CHAIN),
        text(commodity.name.clone())
          .font(typography::body::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::secondary()))
          .into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
    );
  }

  container(
    Column::with_children(rows)
      .spacing(spacing::SPACE_2)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_2_5)
  .style(|_| container::Style {
    border: Border {
      color: color::rule(),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn chain_arrow<'a>() -> Element<'a, Message> {
  Icon::arrow_right()
    .color(color::text::tertiary())
    .size(16.0)
    .render::<Message>()
}

fn heads_section<'a>(heads: &'a [ExtractorHead], now: DateTime<Utc>) -> Element<'a, Message> {
  let mut rows: Vec<Element<'a, Message>> = vec![section_label(
    &t!("industry.colony_detail.heads_title", count => heads.len()),
  )];
  for head in heads {
    rows.push(head_row(head, now));
  }

  Column::with_children(rows)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn head_row<'a>(head: &'a ExtractorHead, now: DateTime<Utc>) -> Element<'a, Message> {
  let sub = t!(
    "industry.colony_detail.head_sub",
    units => fmt_count(head.qty_per_cycle),
    hours => fmt_hours(head.cycle_hours()),
    rate => fmt_count(head.decayed_rate(now).round() as i64),
  )
  .into_owned();

  let remaining = head.time_until_dry(now).unwrap_or(0);
  let (value, caption, accent) = if remaining > 0 {
    (
      fmt_duration(remaining),
      t!("industry.colony_detail.head_until_dry").into_owned(),
      head_accent(remaining),
    )
  } else {
    (
      t!("industry.colony_detail.head_expired").into_owned(),
      t!("industry.colony_detail.head_reset").into_owned(),
      color::status::DANGER,
    )
  };

  pin_row(
    commodity_tile(head.product_name.as_deref(), 0, HEAD_TILE),
    head.product_name.clone().unwrap_or_else(|| "\u{2014}".to_owned()),
    Some(sub),
    stat_block(value, accent, caption),
  )
}

fn factories_section<'a>(factories: &'a [ColonyFactory]) -> Element<'a, Message> {
  let mut rows: Vec<Element<'a, Message>> = vec![section_label(
    &t!("industry.colony_detail.factories_title", count => factories.len()),
  )];
  for factory in factories {
    rows.push(factory_row(factory));
  }

  Column::with_children(rows)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn factory_row<'a>(factory: &'a ColonyFactory) -> Element<'a, Message> {
  let sub = (!factory.input_names.is_empty()).then(|| {
    t!(
      "industry.colony_detail.factory_inputs",
      inputs => factory.input_names.join(" + "),
    )
    .into_owned()
  });

  pin_row(
    commodity_tile(factory.output_name.as_deref(), 2, HEAD_TILE),
    factory.output_name.clone().unwrap_or_else(|| "\u{2014}".to_owned()),
    sub,
    status_badge(factory.active),
  )
}

fn launchpad_section<'a>(launchpad: &'a LaunchpadBuffer) -> Element<'a, Message> {
  let pct = (launchpad.fill_fraction * 100.0).round() as i64;
  let nearly_full = launchpad.is_nearly_full();
  let fill_color = if nearly_full {
    color::status::WARNING
  } else {
    color::accent()
  };

  let meter = Column::with_children(vec![
    Row::with_children(vec![
      text(launchpad.output_name.clone().unwrap_or_else(|| "\u{2014}".to_owned()))
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
      Space::new().width(Length::Fill).into(),
      text(t!("industry.colony_detail.launchpad_fill", pct => pct))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(if nearly_full {
          color::status::WARNING
        } else {
          color::text::secondary()
        }))
        .into(),
    ])
    .width(Length::Fill)
    .into(),
    progress_bar(launchpad.fill_fraction * 100.0, fill_color, PROGRESS_HEIGHT, false),
  ])
  .spacing(spacing::SPACE_2)
  .width(Length::Fill);

  let mut children: Vec<Element<'a, Message>> = vec![
    section_label(&t!("industry.colony_detail.launchpad_title")),
    Row::with_children(vec![
      commodity_tile(launchpad.output_name.as_deref(), 0, HEAD_TILE),
      meter.into(),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .into(),
  ];
  if nearly_full {
    children.push(launchpad_warning());
  }

  Column::with_children(children)
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .into()
}

fn launchpad_warning<'a>() -> Element<'a, Message> {
  Row::with_children(vec![
    Icon::info()
      .color(color::status::WARNING)
      .size(13.0)
      .render::<Message>(),
    text(t!("industry.colony_detail.launchpad_warning"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::status::WARNING))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn pin_row<'a>(
  icon: Element<'a, Message>,
  title: String,
  sub: Option<String>,
  right: Element<'a, Message>,
) -> Element<'a, Message> {
  let mut identity = vec![
    text(title)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if let Some(sub) = sub {
    identity.push(
      text(sub)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }

  let row = Row::with_children(vec![
    icon,
    Column::with_children(identity)
      .spacing(spacing::UNIT)
      .width(Length::Fill)
      .into(),
    right,
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      bottom: spacing::SPACE_2_5,
      left: 0.0,
      right: 0.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        radius: 0.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn stat_block<'a>(value: String, accent: Color, caption: String) -> Element<'a, Message> {
  Column::with_children(vec![
    text(value)
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(accent))
      .into(),
    text(caption)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_x(Horizontal::Right)
  .into()
}

fn status_badge<'a>(active: bool) -> Element<'a, Message> {
  let (label, accent) = if active {
    (t!("industry.colony_detail.factory_active"), color::status::ONLINE)
  } else {
    (t!("industry.colony_detail.factory_stalled"), color::status::DANGER)
  };

  container(
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(accent)),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: spacing::SPACE_2,
    right: spacing::SPACE_2,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(accent, 0.12))),
    border: Border {
      color: color::with_alpha(accent, 0.26),
      radius: radius::SUBTLE.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn section_label<'a>(label: &str) -> Element<'a, Message> {
  text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()))
    .into()
}

fn type_tag<'a>(planet_type: &str, accent: Color) -> Element<'a, Message> {
  container(
    text(planet_label(planet_type).to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(accent)),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: spacing::UNIT + 3.0,
    right: spacing::UNIT + 3.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(accent, 0.12))),
    border: Border {
      color: color::with_alpha(accent, 0.26),
      radius: radius::SUBTLE.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn planet_tile<'a>(colony: &'a Colony) -> Element<'a, Message> {
  let accent = planet_color(&colony.planet_type);
  container(Icon::planet().color(accent).size(HEADER_TILE * 0.5).render::<Message>())
    .width(Length::Fixed(HEADER_TILE))
    .height(Length::Fixed(HEADER_TILE))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(accent, 0.14))),
      border: Border {
        color: color::with_alpha(accent, 0.3),
        radius: (HEADER_TILE / 2.0).into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn commodity_tile<'a>(name: Option<&str>, tier: u8, box_size: f32) -> Element<'a, Message> {
  let accent = tier_color(tier);
  container(
    text(commodity_letters(name))
      .font(typography::body::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(color::text::PRIMARY)),
  )
  .width(Length::Fixed(box_size))
  .height(Length::Fixed(box_size))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(accent, 0.9))),
    border: Border {
      color: color::with_alpha(accent, 0.5),
      radius: radius::SUBTLE.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn fmt_hours(hours: f64) -> String {
  if (hours.fract()).abs() < f64::EPSILON {
    format!("{hours:.0}")
  } else {
    format!("{hours:.1}")
  }
}

fn head_accent(remaining_seconds: i64) -> Color {
  if remaining_seconds < SECONDS_PER_DAY {
    color::status::WARNING
  } else {
    color::accent()
  }
}

#[cfg(test)]
mod tests {
  use super::{
    super::loaders::{ChainCommodity, ChainTier, ColonyDetail, ColonyFactory, ExtractorHead, LaunchpadBuffer},
    *,
  };

  fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-07-14T12:00:00Z")
      .unwrap()
      .with_timezone(&Utc)
  }

  fn colony() -> Colony {
    Colony {
      character_id: 1,
      detail: ColonyDetail {
        chain: vec![
          ChainTier {
            commodities: vec![ChainCommodity {
              name: "Base Metals".to_owned(),
              tier: 0,
            }],
            tier: 0,
          },
          ChainTier {
            commodities: vec![ChainCommodity {
              name: "Reactive Metals".to_owned(),
              tier: 1,
            }],
            tier: 1,
          },
        ],
        factories: vec![ColonyFactory {
          active: true,
          input_names: vec!["Base Metals".to_owned()],
          output_name: Some("Reactive Metals".to_owned()),
        }],
        heads: vec![ExtractorHead {
          cycle_time_seconds: 3_600,
          expiry: Some(now() + chrono::Duration::hours(30)),
          product_name: Some("Base Metals".to_owned()),
          program_start: Some(now() - chrono::Duration::hours(10)),
          qty_per_cycle: 9_100,
        }],
        launchpad: LaunchpadBuffer {
          capacity_m3: 10_000.0,
          fill_fraction: 0.95,
          output_name: Some("Reactive Metals".to_owned()),
          used_m3: 9_500.0,
        },
      },
      extractor_count: 1,
      factory_count: 1,
      name: "Okkamon V".to_owned(),
      num_pins: 6,
      output_name: Some("Reactive Metals".to_owned()),
      output_per_day_nominal: 3_600.0,
      output_tier: 1,
      output_unit_price: 760.0,
      output_volume_m3: 0.15,
      planet_id: 40_000_001,
      planet_type: "barren".to_owned(),
      program_start: Some(now() - chrono::Duration::hours(10)),
      security: Some(0.7),
      soonest_expiry: Some(now() + chrono::Duration::hours(30)),
      system_name: Some("Okkamon".to_owned()),
      upgrade_level: 5,
    }
  }

  mod drawer {
    use super::*;

    #[test]
    fn it_renders_a_full_colony_drawer() {
      let colony = colony();

      let _el: Element<'_, Message> = super::super::drawer(&colony, now());
    }

    #[test]
    fn it_renders_an_import_fed_colony_without_extractors() {
      let mut colony = colony();
      colony.detail.heads.clear();
      colony.extractor_count = 0;
      colony.detail.launchpad.fill_fraction = 0.2;

      let _el: Element<'_, Message> = super::super::drawer(&colony, now());
    }
  }

  mod fmt_hours {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_drops_the_decimal_for_whole_hours() {
      assert_eq!(super::super::fmt_hours(1.0), "1");
      assert_eq!(super::super::fmt_hours(2.0), "2");
    }

    #[test]
    fn it_keeps_one_decimal_for_fractional_hours() {
      assert_eq!(super::super::fmt_hours(0.5), "0.5");
    }
  }

  mod head_accent {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_warns_under_a_day_and_accents_otherwise() {
      assert_eq!(super::super::head_accent(3_600), color::status::WARNING);
      assert_eq!(super::super::head_accent(SECONDS_PER_DAY * 2), color::accent());
    }
  }
}
