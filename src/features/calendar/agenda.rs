use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, text},
};

use super::{
  CalendarEvent, Message, State,
  palette::{self, OwnerType},
};
use crate::{
  config::CalendarDensity,
  ui::style::{color, radius, spacing, typography},
};

const SPINE_GLYPH: f32 = 28.0;
const TIME_COLUMN: f32 = 112.0;

pub(super) fn view<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let from = start_of_day(state.cursor());

  let mut upcoming: Vec<&CalendarEvent> = state
    .visible_events()
    .into_iter()
    .filter(|event| event.start().map(|start| start >= from).unwrap_or(false))
    .collect();
  upcoming.sort_by_key(|event| event.start().map(|s| s.timestamp()).unwrap_or(0));

  if upcoming.is_empty() {
    return empty_state();
  }

  let mut groups: Vec<(DateTime<Utc>, Vec<&CalendarEvent>)> = Vec::new();
  for event in upcoming {
    let Some(day) = event.start().map(start_of_day) else {
      continue;
    };
    match groups.last_mut() {
      Some((existing, items)) if *existing == day => items.push(event),
      _ => groups.push((day, vec![event])),
    }
  }

  let rows: Vec<Element<'a, Message>> = groups
    .into_iter()
    .map(|(day, items)| day_group(state, now, day, items))
    .collect();

  let content = Column::with_children(rows).width(Length::Fill).padding(Padding {
    bottom: spacing::SPACE_6,
    ..Padding::ZERO
  });

  scrollable(container(content).width(Length::Fill).align_x(Horizontal::Center))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn agenda_row<'a>(state: &'a State, event: &'a CalendarEvent) -> Element<'a, Message> {
  let tweaks = state.tweaks();
  let owner = event.owner_kind();
  let color_value = if tweaks.color_by_pilot() {
    palette::pilot_color(state.color_index_for(event.character_id))
  } else {
    owner.color()
  };
  let padding = match tweaks.density() {
    CalendarDensity::Comfortable => Padding {
      top: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    },
    CalendarDensity::Compact => Padding {
      top: spacing::SPACE_2_5,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    },
  };

  let content = Row::with_children(vec![
    time_column(event, tweaks.local_time()),
    spine(owner, color_value),
    details(state, event, owner),
    Space::new().width(Length::Fill).into(),
    right_meta(event, owner),
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Center);

  button(content)
    .padding(padding)
    .width(Length::Fill)
    .on_press(Message::EventOpened(event.character_id, event.event_id))
    .style(|_, status| row_style(status))
    .into()
}

fn day_group<'a>(
  state: &'a State,
  now: DateTime<Utc>,
  day: DateTime<Utc>,
  items: Vec<&'a CalendarEvent>,
) -> Element<'a, Message> {
  let is_today = start_of_day(now) == day;
  let count = items.len();

  let header = Row::with_children(vec![
    text(format!("{:02}", day.day()))
      .font(typography::body::MEDIUM)
      .size(30.0)
      .style(typography::colored(if is_today {
        color::accent::PLASMA
      } else {
        color::text::PRIMARY
      }))
      .into(),
    Column::with_children(vec![
      text(weekday(day))
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
      text(format!("{} {}", month(day), day.year()))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::secondary()))
        .into(),
    ])
    .spacing(spacing::UNIT)
    .into(),
    Space::new().width(Length::Fill).into(),
    text(format!("{count} {}", if count == 1 { "event" } else { "events" }))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: spacing::SPACE_6,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  });

  let mut children: Vec<Element<'a, Message>> = vec![
    container(header)
      .width(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::BASE)),
        border: Border {
          color: color::rule(),
          width: 1.0,
          radius: 0.0.into(),
        },
        ..container::Style::default()
      })
      .into(),
  ];
  children.extend(items.into_iter().map(|event| agenda_row(state, event)));

  Column::with_children(children).width(Length::Fill).into()
}

fn details<'a>(state: &'a State, event: &'a CalendarEvent, owner: OwnerType) -> Element<'a, Message> {
  let mut title_row: Vec<Element<'a, Message>> = vec![
    text(event.title.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if event.importance >= 1 {
    title_row.push(dot(color::status::DANGER));
  }
  if event.owner_type == "pod" {
    title_row.push(pod_tag());
  }

  let owner_line = Row::with_children(vec![
    owner
      .icon()
      .color(color::text::secondary())
      .size(12.0)
      .render::<Message>(),
    text(format!("{} \u{00B7} {}", event.owner_name, owner.label()))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let mut lines: Vec<Element<'a, Message>> = vec![
    Row::with_children(title_row)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
    owner_line.into(),
  ];

  if let Some(pilot) = state.pilot(event.character_id) {
    lines.push(
      Row::with_children(vec![
        dot(palette::pilot_color(state.color_index_for(event.character_id))),
        text(pilot.name.clone())
          .font(typography::body::REGULAR)
          .size(typography::size::SM)
          .style(typography::colored(color::text::secondary()))
          .into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
    );
  }

  Column::with_children(lines).spacing(spacing::UNIT).into()
}

fn dot<'a>(fill: iced::Color) -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(7.0))
    .height(Length::Fixed(7.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        radius: 3.5.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn empty_state<'a>() -> Element<'a, Message> {
  container(
    text("No upcoming events.")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .into()
}

fn hhmm(dt: DateTime<Utc>) -> String {
  format!("{:02}:{:02}", dt.hour(), dt.minute())
}

fn month(day: DateTime<Utc>) -> &'static str {
  const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
  ];
  MONTHS[(day.month0() as usize).min(11)]
}

fn pod_tag<'a>() -> Element<'a, Message> {
  container(
    text("POD")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::status::ONLINE)),
  )
  .padding(Padding {
    top: 1.0,
    bottom: 1.0,
    left: spacing::UNIT,
    right: spacing::UNIT,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::with_alpha(color::status::ONLINE, 0.4),
      radius: radius::SUBTLE.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn response_pill<'a>(response: palette::Response) -> Element<'a, Message> {
  let tint = response.color();
  let active = response != palette::Response::NotResponded;

  container(
    text(response.pill_label().to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(if active {
        tint
      } else {
        color::text::secondary()
      })),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: spacing::SPACE_2,
    right: spacing::SPACE_2,
  })
  .style(move |_| container::Style {
    background: active.then(|| Background::Color(color::with_alpha(tint, 0.12))),
    border: Border {
      color: if active { tint } else { color::rule() },
      radius: 999.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn right_meta<'a>(event: &'a CalendarEvent, owner: OwnerType) -> Element<'a, Message> {
  if owner.respondable() {
    response_pill(event.response_kind())
  } else {
    container(
      text(owner.short_label())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary())),
    )
    .padding(Padding {
      top: 3.0,
      bottom: 3.0,
      left: spacing::SPACE_2,
      right: spacing::SPACE_2,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        radius: 999.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }
}

fn row_style(status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: hovered.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.03))),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 0.0.into(),
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn spine<'a>(owner: OwnerType, color_value: iced::Color) -> Element<'a, Message> {
  let glyph = container(owner.icon().color(color_value).size(15.0).render::<Message>())
    .width(Length::Fixed(SPINE_GLYPH))
    .height(Length::Fixed(SPINE_GLYPH))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(color_value, 0.16))),
      border: Border {
        color: color::with_alpha(color_value, 0.34),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  glyph.into()
}

fn time_column<'a>(event: &'a CalendarEvent, local_time: bool) -> Element<'a, Message> {
  let mut lines: Vec<Element<'a, Message>> = Vec::new();

  if event.is_all_day() {
    lines.push(
      text("All day")
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
    );
  } else if let Some(start) = event.start() {
    lines.push(
      text(hhmm(start))
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
    );
    if let Some(end) = event.end()
      && end != start
    {
      lines.push(
        text(format!("\u{2013} {}", hhmm(end)))
          .font(typography::mono::REGULAR)
          .size(typography::size::SM)
          .style(typography::colored(color::text::secondary()))
          .into(),
      );
    }
    if local_time {
      lines.push(
        text(format!("{} LT", hhmm(start)))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(typography::colored(color::text::tertiary()))
          .into(),
      );
    }
  }

  container(Column::with_children(lines).spacing(spacing::UNIT))
    .width(Length::Fixed(TIME_COLUMN))
    .into()
}

fn start_of_day(dt: DateTime<Utc>) -> DateTime<Utc> {
  dt - Duration::seconds(i64::from(dt.num_seconds_from_midnight()))
}

fn weekday(day: DateTime<Utc>) -> &'static str {
  const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
  ];
  DAYS[day.weekday().num_days_from_monday() as usize]
}

#[cfg(test)]
mod tests {
  use super::*;

  fn at(timestamp: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(timestamp).unwrap().with_timezone(&Utc)
  }

  mod hhmm {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_pads_the_eve_clock() {
      assert_eq!(hhmm(at("2026-06-12T09:05:00Z")), "09:05");
    }
  }

  mod start_of_day {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_truncates_to_midnight_utc() {
      assert_eq!(start_of_day(at("2026-06-12T14:22:31Z")), at("2026-06-12T00:00:00Z"));
    }
  }

  mod weekday {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_names_the_utc_weekday() {
      assert_eq!(weekday(at("2026-06-12T00:00:00Z")), "Friday");
    }
  }
}
