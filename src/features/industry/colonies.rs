use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, responsive, scrollable, text},
};

use super::{
  Colony, ColonySort, ColonyState, Message, State,
  jobs::{progress_bar, sec_pill},
};
use crate::ui::{
  components::icon::Icon,
  format::{fmt_count, fmt_duration, fmt_isk},
  style::{color, radius, spacing, typography},
};

const CARD_GAP: f32 = 18.0;
const CARD_MIN_WIDTH: f32 = 380.0;
const CONTENT_PADDING: f32 = 24.0;
const PIP_SIZE: f32 = 6.0;
const TILE_BOX: f32 = 42.0;
const TILE_COMMODITY: f32 = 34.0;

pub(super) fn tab<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let colonies = sorted_colonies(state, now);

  let body = Column::with_children(vec![
    summary_band(&colonies, now),
    toolbar(state, colonies.len()),
    grid(colonies, now),
  ])
  .spacing(spacing::SPACE_3_5)
  .width(Length::Fill);

  scrollable(container(body).width(Length::Fill).padding(CONTENT_PADDING))
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn sorted_colonies(state: &State, now: DateTime<Utc>) -> Vec<&Colony> {
  let mut colonies = state.visible_colonies();
  match state.colony_sort() {
    ColonySort::Expiry => colonies.sort_by_key(|a| expiry_key(a, now)),
    ColonySort::Tier => colonies.sort_by_key(|colony| std::cmp::Reverse(colony.output_tier)),
    ColonySort::Value => colonies.sort_by(|a, b| {
      b.value_per_day(now)
        .partial_cmp(&a.value_per_day(now))
        .unwrap_or(std::cmp::Ordering::Equal)
    }),
  }
  colonies
}

fn expiry_key(colony: &Colony, now: DateTime<Utc>) -> i64 {
  colony.expiry_seconds(now).unwrap_or(i64::MAX)
}

fn summary_band<'a>(colonies: &[&'a Colony], now: DateTime<Utc>) -> Element<'a, Message> {
  let count = colonies.len();
  let import_fed = colonies.iter().filter(|colony| colony.is_import_fed()).count();
  let value_per_day: f64 = colonies.iter().map(|colony| colony.value_per_day(now)).sum();
  let expiring = colonies
    .iter()
    .filter(|colony| colony.state(now) == ColonyState::ExpiringSoon)
    .count();
  let idle = colonies
    .iter()
    .filter(|colony| colony.state(now) == ColonyState::Idle)
    .count();

  let cells = [
    summary_cell(
      &t!("industry.colonies.summary_colonies"),
      count.to_string(),
      color::text::PRIMARY,
      t!("industry.colonies.summary_colonies_sub", count => import_fed).into_owned(),
    ),
    summary_cell(
      &t!("industry.colonies.summary_output"),
      format!("{} ISK", fmt_isk(value_per_day)),
      color::status::ONLINE,
      t!("industry.colonies.summary_output_sub").into_owned(),
    ),
    summary_cell(
      &t!("industry.colonies.summary_expiring"),
      expiring.to_string(),
      if expiring > 0 {
        color::status::WARNING
      } else {
        color::text::PRIMARY
      },
      t!("industry.colonies.summary_expiring_sub").into_owned(),
    ),
    summary_cell(
      &t!("industry.colonies.summary_idle"),
      idle.to_string(),
      if idle > 0 {
        color::status::DANGER
      } else {
        color::text::PRIMARY
      },
      t!("industry.colonies.summary_idle_sub").into_owned(),
    ),
  ];

  let mut children: Vec<Element<'a, Message>> = Vec::new();
  for (index, cell) in cells.into_iter().enumerate() {
    if index > 0 {
      children.push(cell_divider());
    }
    children.push(cell);
  }

  container(Row::with_children(children).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn summary_cell<'a>(label: &str, value: String, value_color: Color, sub: String) -> Element<'a, Message> {
  let cell = Column::with_children(vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    text(value)
      .font(typography::mono::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(value_color))
      .into(),
    text(sub)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  container(cell)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_4_5,
      bottom: spacing::SPACE_4_5,
      left: spacing::SPACE_4_5,
      right: spacing::SPACE_4_5,
    })
    .into()
}

fn cell_divider<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(1.0))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

fn toolbar<'a>(state: &'a State, count: usize) -> Element<'a, Message> {
  Row::with_children(vec![
    text(t!("industry.colonies.header_title").to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(t!("industry.colonies.header_count", count => count))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    Space::new().width(Length::Fill).into(),
    text(t!("industry.colonies.sort").to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    sort_control(state.colony_sort()),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .into()
}

fn sort_control<'a>(active: ColonySort) -> Element<'a, Message> {
  let options = [
    (ColonySort::Expiry, t!("industry.colonies.sort_expiry")),
    (ColonySort::Value, t!("industry.colonies.sort_value")),
    (ColonySort::Tier, t!("industry.colonies.sort_tier")),
  ];
  let buttons: Vec<Element<'a, Message>> = options
    .into_iter()
    .enumerate()
    .map(|(index, (sort, label))| sort_button(&label, sort, active == sort, index > 0))
    .collect();

  container(Row::with_children(buttons))
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

fn sort_button<'a>(label: &str, sort: ColonySort, active: bool, divider: bool) -> Element<'a, Message> {
  let text_color = if active {
    color::accent()
  } else {
    color::text::secondary()
  };

  button(
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(text_color)),
  )
  .padding(Padding {
    top: spacing::UNIT + 2.0,
    bottom: spacing::UNIT + 2.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::ColonySortSelected(sort))
  .style(move |_, _| button::Style {
    background: active.then(|| Background::Color(color::with_alpha(color::accent(), 0.12))),
    border: Border {
      color: if divider { color::rule() } else { Color::TRANSPARENT },
      radius: 0.0.into(),
      width: 0.0,
    },
    text_color,
    ..button::Style::default()
  })
  .into()
}

fn grid<'a>(colonies: Vec<&'a Colony>, now: DateTime<Utc>) -> Element<'a, Message> {
  if colonies.is_empty() {
    return container(
      text(t!("industry.colonies.empty"))
        .font(typography::body::REGULAR)
        .size(typography::size::LG)
        .style(typography::colored(color::text::tertiary())),
    )
    .width(Length::Fill)
    .padding(spacing::SPACE_6)
    .align_x(Horizontal::Center)
    .into();
  }

  responsive(move |size| {
    let per_row = per_row(size.width);
    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    for chunk in colonies.chunks(per_row) {
      let mut cells: Vec<Element<'a, Message>> = chunk.iter().map(|colony| card(colony, now)).collect();
      while cells.len() < per_row {
        cells.push(Space::new().width(Length::Fill).into());
      }
      rows.push(Row::with_children(cells).spacing(CARD_GAP).width(Length::Fill).into());
    }
    Column::with_children(rows).spacing(CARD_GAP).width(Length::Fill).into()
  })
  .into()
}

fn card<'a>(colony: &'a Colony, now: DateTime<Utc>) -> Element<'a, Message> {
  let state = colony.state(now);
  let accent = state_color(state);

  let card = Column::with_children(vec![
    card_header(colony, state, accent),
    card_body(colony, now, state, accent),
  ])
  .width(Length::Fill);

  let border = match state {
    ColonyState::Idle => color::with_alpha(color::status::DANGER, 0.45),
    ColonyState::ExpiringSoon => color::with_alpha(color::status::WARNING, 0.4),
    _ => color::rule(),
  };

  container(card)
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: border,
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn card_header<'a>(colony: &'a Colony, state: ColonyState, accent: Color) -> Element<'a, Message> {
  let header = Row::with_children(vec![
    planet_tile(colony),
    identity(colony),
    Column::with_children(vec![state_badge(state, accent), cc_pips(colony.cc_level())])
      .spacing(spacing::SPACE_2)
      .align_x(Horizontal::Right)
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(header)
    .width(Length::Fill)
    .padding(spacing::SPACE_3_5)
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

fn card_body<'a>(colony: &'a Colony, now: DateTime<Utc>, state: ColonyState, accent: Color) -> Element<'a, Message> {
  let footer = if colony.is_import_fed() {
    import_note(colony)
  } else {
    timer(colony, now, state, accent)
  };

  Column::with_children(vec![output_row(colony, now), footer])
    .spacing(spacing::SPACE_3_5)
    .padding(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn output_row<'a>(colony: &'a Colony, now: DateTime<Utc>) -> Element<'a, Message> {
  let per_day = colony.output_per_day(now);
  let value = colony.value_per_day(now);

  let flow = if per_day > 0.0 {
    t!("industry.colonies.units_per_day", units => fmt_count(per_day.round() as i64)).into_owned()
  } else {
    t!("industry.colonies.output_halted").into_owned()
  };

  let name_row = Row::with_children(vec![
    text(colony.output_name.clone().unwrap_or_else(|| "\u{2014}".to_owned()))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    tier_badge(colony.output_tier),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let identity = Column::with_children(vec![
    name_row.into(),
    text(flow)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  let value_color = if value > 0.0 {
    color::status::ONLINE
  } else {
    color::text::tertiary()
  };
  let value_text = if value > 0.0 {
    fmt_isk(value)
  } else {
    "\u{2014}".to_owned()
  };
  let value_block = Column::with_children(vec![
    text(value_text)
      .font(typography::mono::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(value_color))
      .into(),
    text(t!("industry.colonies.isk_per_day").to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_x(Horizontal::Right);

  Row::with_children(vec![
    commodity_tile(colony, TILE_COMMODITY),
    identity.into(),
    value_block.into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill)
  .into()
}

fn timer<'a>(colony: &'a Colony, now: DateTime<Utc>, state: ColonyState, accent: Color) -> Element<'a, Message> {
  let remaining = colony.expiry_seconds(now).unwrap_or(0);
  let (label, value) = if remaining > 0 {
    (t!("industry.colonies.timer_expires"), fmt_duration(remaining))
  } else {
    (
      t!("industry.colonies.timer_expired"),
      t!("industry.colonies.timer_reset").into_owned(),
    )
  };

  let head = Row::with_children(vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    Space::new().width(Length::Fill).into(),
    text(value)
      .font(typography::mono::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(accent))
      .into(),
  ])
  .align_y(Vertical::Bottom)
  .width(Length::Fill);

  Column::with_children(vec![
    head.into(),
    progress_bar(colony.progress(now), accent, 7.0, state != ColonyState::Idle),
    footer_counts(colony),
  ])
  .spacing(spacing::SPACE_2)
  .width(Length::Fill)
  .into()
}

fn footer_counts<'a>(colony: &'a Colony) -> Element<'a, Message> {
  text(t!(
    "industry.colonies.heads_factory",
    heads => colony.extractor_count,
    heads_plural => plural(colony.extractor_count),
    factories => colony.factory_count,
  ))
  .font(typography::mono::REGULAR)
  .size(typography::size::XS)
  .style(typography::colored(color::text::tertiary()))
  .into()
}

fn import_note<'a>(colony: &'a Colony) -> Element<'a, Message> {
  let note = Row::with_children(vec![
    Icon::industry()
      .color(color::status::ONLINE)
      .size(15.0)
      .render::<Message>(),
    text(t!(
      "industry.colonies.factory_note",
      count => colony.factory_count,
      plural => plural(colony.factory_count),
    ))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::secondary()))
    .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  container(note)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.03))),
      border: Border {
        color: color::rule(),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn identity<'a>(colony: &'a Colony) -> Element<'a, Message> {
  let mut meta: Vec<Element<'a, Message>> = vec![type_tag(colony)];
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

fn planet_tile<'a>(colony: &'a Colony) -> Element<'a, Message> {
  let accent = planet_color(&colony.planet_type);
  container(Icon::planet().color(accent).size(TILE_BOX * 0.5).render::<Message>())
    .width(Length::Fixed(TILE_BOX))
    .height(Length::Fixed(TILE_BOX))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(accent, 0.14))),
      border: Border {
        color: color::with_alpha(accent, 0.3),
        radius: (TILE_BOX / 2.0).into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn commodity_tile<'a>(colony: &'a Colony, box_size: f32) -> Element<'a, Message> {
  let accent = tier_color(colony.output_tier);
  let letters = commodity_letters(colony.output_name.as_deref());
  container(
    text(letters)
      .font(typography::body::MEDIUM)
      .size(typography::size::XS_PLUS)
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

fn commodity_letters(name: Option<&str>) -> String {
  let Some(name) = name else {
    return "\u{2014}".to_owned();
  };
  let letters: String = name
    .split(|ch: char| ch.is_whitespace() || ch == '-')
    .filter(|word| !word.is_empty())
    .take(2)
    .filter_map(|word| word.chars().next())
    .collect();
  letters.to_uppercase()
}

fn cc_pips<'a>(level: i64) -> Element<'a, Message> {
  let filled = level.clamp(0, 5);
  let pips: Vec<Element<'a, Message>> = (1..=5)
    .map(|index| {
      let on = i64::from(index) <= filled;
      let fill = if on {
        color::accent()
      } else {
        color::with_alpha(color::text::PRIMARY, 0.16)
      };
      container(Space::new())
        .width(Length::Fixed(PIP_SIZE))
        .height(Length::Fixed(PIP_SIZE))
        .style(move |_| container::Style {
          background: Some(Background::Color(fill)),
          border: Border {
            radius: (PIP_SIZE / 2.0).into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into()
    })
    .collect();

  Row::with_children(pips).spacing(3.0).align_y(Vertical::Center).into()
}

fn state_badge<'a>(state: ColonyState, accent: Color) -> Element<'a, Message> {
  container(
    text(state.label().to_uppercase())
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

fn tier_badge<'a>(tier: u8) -> Element<'a, Message> {
  let accent = tier_color(tier);
  container(
    text(format!("P{tier}"))
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(accent)),
  )
  .padding(Padding {
    top: 1.0,
    bottom: 1.0,
    left: spacing::UNIT + 2.0,
    right: spacing::UNIT + 2.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(accent, 0.14))),
    border: Border {
      color: color::with_alpha(accent, 0.28),
      radius: radius::SUBTLE.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn type_tag<'a>(colony: &'a Colony) -> Element<'a, Message> {
  let accent = planet_color(&colony.planet_type);
  container(
    text(planet_label(&colony.planet_type).to_uppercase())
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

fn per_row(width: f32) -> usize {
  if width < CARD_MIN_WIDTH {
    return 1;
  }
  (((width + CARD_GAP) / (CARD_MIN_WIDTH + CARD_GAP)).floor() as usize).max(1)
}

fn plural(count: usize) -> &'static str {
  if count == 1 { "" } else { "s" }
}

fn state_color(state: ColonyState) -> Color {
  match state {
    ColonyState::Extracting => color::accent(),
    ColonyState::ExpiringSoon => color::status::WARNING,
    ColonyState::Idle => color::status::DANGER,
    ColonyState::Processing => color::status::ONLINE,
  }
}

fn planet_label(planet_type: &str) -> String {
  planet_meta(planet_type).0
}

fn planet_color(planet_type: &str) -> Color {
  planet_meta(planet_type).1
}

fn planet_meta(planet_type: &str) -> (String, Color) {
  let (label, hex) = match planet_type {
    "temperate" => ("Temperate", "#5BB97E"),
    "barren" => ("Barren", "#C9A36B"),
    "oceanic" => ("Oceanic", "#3FB8DB"),
    "ice" => ("Ice", "#9FD2E0"),
    "gas" => ("Gas", "#C9743E"),
    "lava" => ("Lava", "#E07559"),
    "storm" => ("Storm", "#7B8BD9"),
    "plasma" => ("Plasma", "#B98BD9"),
    other => return (title_case(other), color::text::secondary()),
  };
  (
    label.to_owned(),
    color::from_hex(hex).unwrap_or(color::text::secondary()),
  )
}

fn tier_color(tier: u8) -> Color {
  let hex = match tier {
    0 => "#7A8694",
    1 => "#5BB97E",
    2 => "#3FB8DB",
    3 => "#9B7BD9",
    _ => "#D9B252",
  };
  color::from_hex(hex).unwrap_or(color::text::secondary())
}

fn title_case(value: &str) -> String {
  let mut chars = value.chars();
  match chars.next() {
    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    None => String::new(),
  }
}

#[cfg(test)]
mod tests {
  use super::{
    super::{EMPTY_INDUSTRY_SELECTION, FacilityDefaults, Tab},
    *,
  };

  fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-07-14T12:00:00Z")
      .unwrap()
      .with_timezone(&Utc)
  }

  fn colony(planet_id: i64, expiry: Option<&str>, tier: u8, import_fed: bool) -> Colony {
    Colony {
      character_id: 1,
      extractor_count: if import_fed { 0 } else { 2 },
      factory_count: 2,
      name: "Okkamon V".to_owned(),
      output_name: Some("Precious Metals".to_owned()),
      output_per_day_nominal: 3_600.0,
      output_tier: tier,
      output_unit_price: 980.0,
      planet_id,
      planet_type: "barren".to_owned(),
      program_start: Some(now() - chrono::Duration::hours(24)),
      security: Some(0.7),
      soonest_expiry: expiry.map(|value| DateTime::parse_from_rfc3339(value).unwrap().with_timezone(&Utc)),
      system_name: Some("Okkamon".to_owned()),
      upgrade_level: 5,
    }
  }

  fn state_with(colonies: Vec<Colony>) -> State {
    let mut state = State::new(
      EMPTY_INDUSTRY_SELECTION,
      Vec::new(),
      crate::config::FeatureFlags::default(),
      FacilityDefaults::default(),
      None,
      false,
    );
    state.seed_colonies(colonies);
    state.seed_tab(Tab::Colonies);
    state
  }

  mod commodity_letters {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_takes_the_first_letter_of_the_first_two_words() {
      assert_eq!(super::super::commodity_letters(Some("Precious Metals")), "PM");
    }

    #[test]
    fn it_splits_on_hyphens() {
      assert_eq!(super::super::commodity_letters(Some("Nano-Factory")), "NF");
    }

    #[test]
    fn it_renders_a_dash_when_no_output_is_known() {
      assert_eq!(super::super::commodity_letters(None), "\u{2014}");
    }
  }

  mod planet_meta {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_labels_a_known_planet_type() {
      assert_eq!(super::super::planet_label("lava"), "Lava");
    }

    #[test]
    fn it_title_cases_an_unknown_planet_type() {
      assert_eq!(super::super::planet_label("shattered"), "Shattered");
    }
  }

  mod per_row {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_fits_one_card_below_the_minimum_width() {
      assert_eq!(super::super::per_row(300.0), 1);
    }

    #[test]
    fn it_fits_multiple_cards_across_a_wide_viewport() {
      assert_eq!(super::super::per_row(900.0), 2);
    }
  }

  mod sorted_colonies {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_orders_by_soonest_expiry_first() {
      let mut state = state_with(vec![
        colony(1, Some("2026-07-20T00:00:00Z"), 1, false),
        colony(2, Some("2026-07-15T00:00:00Z"), 1, false),
      ]);
      state.colony_sort = ColonySort::Expiry;

      let ordered = super::super::sorted_colonies(&state, now());

      assert_eq!(ordered[0].planet_id, 2);
    }

    #[test]
    fn it_orders_import_fed_colonies_last_by_expiry() {
      let mut state = state_with(vec![
        colony(1, None, 2, true),
        colony(2, Some("2026-07-15T00:00:00Z"), 1, false),
      ]);
      state.colony_sort = ColonySort::Expiry;

      let ordered = super::super::sorted_colonies(&state, now());

      assert_eq!(ordered[0].planet_id, 2);
    }

    #[test]
    fn it_orders_by_highest_tier() {
      let mut state = state_with(vec![
        colony(1, Some("2026-07-20T00:00:00Z"), 1, false),
        colony(2, Some("2026-07-15T00:00:00Z"), 3, false),
      ]);
      state.colony_sort = ColonySort::Tier;

      let ordered = super::super::sorted_colonies(&state, now());

      assert_eq!(ordered[0].planet_id, 2);
    }
  }

  mod tab {
    use super::*;

    #[test]
    fn it_renders_colonies_in_each_derived_state() {
      let state = state_with(vec![
        colony(1, Some("2026-07-20T00:00:00Z"), 1, false),
        colony(2, Some("2026-07-14T18:00:00Z"), 2, false),
        colony(3, Some("2026-07-13T00:00:00Z"), 1, false),
        colony(4, None, 3, true),
      ]);

      let _el: Element<'_, Message> = tab(&state, now());
    }

    #[test]
    fn it_renders_the_empty_state() {
      let state = state_with(Vec::new());

      let _el: Element<'_, Message> = tab(&state, now());
    }
  }
}
