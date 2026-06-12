use chrono::{DateTime, Datelike, Timelike, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, Stack, button, container, scrollable, text},
};

use super::{CalendarEvent, Message, State, grid, grid::Packed};
use crate::{
  config::CalendarDensity,
  ui::style::{color, radius, spacing, typography},
};

const GUTTER: f32 = 56.0;
const LANE_SPACING: f32 = 2.0;

pub(super) fn view<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let tweaks = state.tweaks();
  let dates = grid::week_dates(state.cursor(), tweaks.week_start(), tweaks.show_weekends());
  let events = state.visible_events();

  let per_day: Vec<Vec<&CalendarEvent>> = dates.iter().map(|day| grid::events_on_day(&events, *day)).collect();
  let has_all_day = per_day.iter().any(|items| items.iter().any(|event| event.is_all_day()));

  let hour_height = hour_height(tweaks.density());

  let mut children: Vec<Element<'a, Message>> = vec![header_row(&dates, now)];
  if has_all_day {
    children.push(all_day_row(state, &dates, &per_day));
  }
  children.push(
    scrollable(time_grid(state, &dates, &per_day, hour_height, now))
      .width(Length::Fill)
      .height(Length::Fill)
      .into(),
  );

  container(Column::with_children(children).width(Length::Fill).height(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn all_day_row<'a>(
  state: &'a State,
  dates: &[DateTime<Utc>],
  per_day: &[Vec<&'a CalendarEvent>],
) -> Element<'a, Message> {
  let mut cells: Vec<Element<'a, Message>> = vec![
    container(
      text("ALL DAY")
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary())),
    )
    .width(Length::Fixed(GUTTER))
    .align_x(Horizontal::Right)
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: 0.0,
      right: spacing::SPACE_2,
    })
    .into(),
  ];

  for (index, _) in dates.iter().enumerate() {
    let chips: Vec<Element<'a, Message>> = per_day[index]
      .iter()
      .filter(|event| event.is_all_day())
      .map(|event| all_day_chip(state, event))
      .collect();

    cells.push(
      container(Column::with_children(chips).spacing(2.0))
        .width(Length::FillPortion(1))
        .padding(spacing::UNIT)
        .style(|_| container::Style {
          border: Border {
            color: color::rule(),
            width: 1.0,
            radius: 0.0.into(),
          },
          ..container::Style::default()
        })
        .into(),
    );
  }

  container(Row::with_children(cells).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn all_day_chip<'a>(state: &'a State, event: &'a CalendarEvent) -> Element<'a, Message> {
  let tint = grid::color_for(state, event);
  button(
    text(event.title.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY)),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: spacing::SPACE_2,
    right: spacing::SPACE_2,
  })
  .on_press(Message::EventOpened(event.character_id, event.event_id))
  .style(move |_, status| block_style(tint, status))
  .into()
}

fn block_style(tint: iced::Color, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: Some(Background::Color(color::with_alpha(
      tint,
      if hovered { 0.27 } else { 0.17 },
    ))),
    border: Border {
      color: color::with_alpha(tint, 0.3),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn day_column<'a>(
  state: &'a State,
  day: DateTime<Utc>,
  items: &[&'a CalendarEvent],
  hour_height: f32,
  now: DateTime<Utc>,
) -> Element<'a, Message> {
  let is_today = grid::day_key(day) == grid::day_key(now);
  let packed = grid::pack_day(items);
  let lanes = packed.iter().map(|span| span.lanes).max().unwrap_or(1).max(1);

  let lane_columns: Vec<Element<'a, Message>> = (0..lanes)
    .map(|lane| lane_column(state, hour_height, &packed, lane))
    .collect();

  let lines: Vec<Element<'a, Message>> = (0..24)
    .map(|hour| {
      container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(hour_height))
        .style(move |_| container::Style {
          border: Border {
            color: color::rule(),
            width: if hour == 0 { 0.0 } else { 1.0 },
            radius: 0.0.into(),
          },
          ..container::Style::default()
        })
        .into()
    })
    .collect();

  let mut layers: Vec<Element<'a, Message>> = vec![
    Column::with_children(lines)
      .width(Length::Fill)
      .height(Length::Fixed(hour_height * 24.0))
      .into(),
    container(Row::with_children(lane_columns).spacing(LANE_SPACING))
      .width(Length::Fill)
      .height(Length::Fixed(hour_height * 24.0))
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: 1.0,
        right: 2.0,
      })
      .into(),
  ];
  if is_today {
    layers.push(now_line(now, hour_height));
  }

  container(
    Stack::with_children(layers)
      .width(Length::Fill)
      .height(Length::Fixed(hour_height * 24.0)),
  )
  .width(Length::FillPortion(1))
  .height(Length::Fixed(hour_height * 24.0))
  .style(move |_| container::Style {
    background: is_today.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.03))),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn day_header<'a>(day: DateTime<Utc>, now: DateTime<Utc>) -> Element<'a, Message> {
  let is_today = grid::day_key(day) == grid::day_key(now);
  let number_color = if is_today {
    color::surface::BASE
  } else {
    color::text::PRIMARY
  };

  let number = container(
    text(day.day().to_string())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(number_color)),
  )
  .width(Length::Fixed(30.0))
  .height(Length::Fixed(30.0))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: is_today.then_some(Background::Color(color::accent::PLASMA)),
    border: Border {
      radius: 999.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  container(
    Column::with_children(vec![
      text(weekday_short(day.weekday().num_days_from_sunday()))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(if is_today {
          color::accent::PLASMA
        } else {
          color::text::tertiary()
        }))
        .into(),
      number.into(),
    ])
    .spacing(spacing::UNIT)
    .align_x(Horizontal::Center),
  )
  .width(Length::FillPortion(1))
  .padding(Padding {
    top: spacing::SPACE_2_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_2,
    right: spacing::SPACE_2,
  })
  .align_x(Horizontal::Center)
  .style(|_| container::Style {
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn event_block<'a>(state: &'a State, packed: Packed<'a>, hour_height: f32) -> Element<'a, Message> {
  let event = packed.event;
  let tint = grid::color_for(state, event);
  let instant = grid::is_instant(event);
  let min_height = if instant { 18.0 } else { 24.0 };
  let height = (((packed.end_minute - packed.start_minute) as f32 / 60.0) * hour_height - LANE_SPACING).max(min_height);
  let tall = height > 38.0;

  let mut lines: Vec<Element<'a, Message>> = vec![
    text(event.title.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if tall && let Some(start) = event.start() {
    lines.push(
      text(grid::hhmm(start))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary()))
        .into(),
    );
  }

  let inner = container(Column::with_children(lines).spacing(1.0))
    .width(Length::Fill)
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: spacing::UNIT + 1.0,
      right: spacing::UNIT + 1.0,
    });

  button(
    Row::with_children(vec![grid::accent_strip(tint), inner.into()])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fixed(height))
  .padding(0.0)
  .on_press(Message::EventOpened(event.character_id, event.event_id))
  .style(move |_, status| block_style(tint, status))
  .into()
}

fn header_row<'a>(dates: &[DateTime<Utc>], now: DateTime<Utc>) -> Element<'a, Message> {
  let mut cells: Vec<Element<'a, Message>> = vec![container(Space::new()).width(Length::Fixed(GUTTER)).into()];
  cells.extend(dates.iter().map(|day| day_header(*day, now)));

  container(Row::with_children(cells).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn hour_gutter<'a>(hour_height: f32) -> Element<'a, Message> {
  let cells: Vec<Element<'a, Message>> = (0..24)
    .map(|hour| {
      container(if hour == 0 {
        Element::from(Space::new())
      } else {
        text(format!("{hour:02}"))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::tertiary()))
          .into()
      })
      .width(Length::Fill)
      .height(Length::Fixed(hour_height))
      .align_x(Horizontal::Right)
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: 0.0,
        right: spacing::SPACE_2,
      })
      .into()
    })
    .collect();

  container(Column::with_children(cells))
    .width(Length::Fixed(GUTTER))
    .height(Length::Fixed(hour_height * 24.0))
    .into()
}

fn hour_height(density: CalendarDensity) -> f32 {
  match density {
    CalendarDensity::Comfortable => 50.0,
    CalendarDensity::Compact => 40.0,
  }
}

/// Builds one vertical lane of timed events using leading `Space` spacers to simulate absolute
/// top-offset positioning, since iced 0.14 has no absolute-position layout primitive. `filled`
/// tracks the running vertical cursor so consecutive events in the same lane stay contiguous.
fn lane_column<'a>(state: &'a State, hour_height: f32, packed: &[Packed<'a>], lane: usize) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = Vec::new();
  let mut filled = 0.0_f32;

  let mut in_lane: Vec<&Packed<'a>> = packed.iter().filter(|span| span.lane == lane).collect();
  in_lane.sort_by_key(|span| span.start_minute);

  for span in in_lane {
    let top = (span.start_minute as f32 / 60.0) * hour_height;
    if top > filled {
      children.push(Space::new().height(Length::Fixed(top - filled)).into());
    }
    let instant = grid::is_instant(span.event);
    let min_height = if instant { 18.0 } else { 24.0 };
    let height = (((span.end_minute - span.start_minute) as f32 / 60.0) * hour_height - LANE_SPACING).max(min_height);
    children.push(event_block(state, *span, hour_height));
    filled = top + height + LANE_SPACING;
  }

  Column::with_children(children)
    .width(Length::FillPortion(1))
    .height(Length::Fixed(hour_height * 24.0))
    .into()
}

fn now_line<'a>(now: DateTime<Utc>, hour_height: f32) -> Element<'a, Message> {
  let minutes = i64::from(now.hour()) * 60 + i64::from(now.minute());
  let top = (minutes as f32 / 60.0) * hour_height;

  let line = container(Space::new())
    .width(Length::Fill)
    .height(Length::Fixed(2.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::PLASMA)),
      ..container::Style::default()
    });

  Column::with_children(vec![
    Space::new().height(Length::Fixed(top.max(0.0))).into(),
    line.into(),
  ])
  .width(Length::Fill)
  .into()
}

fn time_grid<'a>(
  state: &'a State,
  dates: &[DateTime<Utc>],
  per_day: &[Vec<&'a CalendarEvent>],
  hour_height: f32,
  now: DateTime<Utc>,
) -> Element<'a, Message> {
  let mut cells: Vec<Element<'a, Message>> = vec![hour_gutter(hour_height)];
  for (index, day) in dates.iter().enumerate() {
    cells.push(day_column(state, *day, &per_day[index], hour_height, now));
  }

  Row::with_children(cells)
    .width(Length::Fill)
    .height(Length::Fixed(hour_height * 24.0))
    .into()
}

fn weekday_short(weekday: u32) -> &'static str {
  const DAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
  DAYS[(weekday as usize).min(6)]
}

#[cfg(test)]
mod tests {
  use super::*;

  mod hour_height {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_compresses_in_compact_density() {
      assert!(hour_height(CalendarDensity::Compact) < hour_height(CalendarDensity::Comfortable));
    }

    #[test]
    fn it_uses_the_comfortable_default() {
      assert_eq!(hour_height(CalendarDensity::Comfortable), 50.0);
    }
  }
}
