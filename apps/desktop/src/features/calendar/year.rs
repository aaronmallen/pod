use chrono::{DateTime, Datelike, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text},
};

use super::{CalendarEvent, Message, State, View, grid};
use crate::ui::{
  components::rule,
  format::month_long,
  style::{color, radius, spacing, typography},
};

const COLUMNS: usize = 4;
const DAY_DOT: f32 = 4.0;
const YEAR_MAX_WIDTH: f32 = 1760.0;

pub(super) fn view<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let tweaks = state.tweaks();
  let year = state.cursor().year();
  let events = state.visible_events();

  let mut rows: Vec<Element<'a, Message>> = Vec::new();
  let mut row: Vec<Element<'a, Message>> = Vec::new();
  for month0 in 0..12u32 {
    row.push(mini_month(state, now, &events, year, month0, tweaks.week_start()));
    if row.len() == COLUMNS {
      rows.push(
        Row::with_children(std::mem::take(&mut row))
          .spacing(spacing::SPACE_6 + spacing::SPACE_2)
          .into(),
      );
    }
  }
  if !row.is_empty() {
    while row.len() < COLUMNS {
      row.push(Space::new().width(Length::FillPortion(1)).into());
    }
    rows.push(
      Row::with_children(row)
        .spacing(spacing::SPACE_6 + spacing::SPACE_2)
        .into(),
    );
  }

  let grid_body = Column::with_children(rows)
    .spacing(spacing::SPACE_6 + spacing::UNIT)
    .padding(Padding {
      top: spacing::SPACE_6,
      bottom: spacing::SPACE_6 * 2.0,
      left: spacing::SPACE_6 + spacing::SPACE_2,
      right: spacing::SPACE_6 + spacing::SPACE_2,
    })
    .max_width(YEAR_MAX_WIDTH);

  container(
    iced::widget::scrollable(container(grid_body).width(Length::Fill).align_x(Horizontal::Center))
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    ..container::Style::default()
  })
  .into()
}

fn day_button<'a>(
  state: &'a State,
  now: DateTime<Utc>,
  events: &[&'a CalendarEvent],
  day: DateTime<Utc>,
  month0: u32,
) -> Element<'a, Message> {
  let in_month = day.month0() == month0;
  let is_today = grid::day_key(day) == grid::day_key(now);
  let items = if in_month {
    grid::events_on_day(events, day)
  } else {
    Vec::new()
  };
  let dot = items.first().map(|event| grid::color_for(state, event));

  let label_color = if is_today {
    color::surface::BASE
  } else if in_month {
    color::text::secondary()
  } else {
    color::with_alpha(color::text::PRIMARY, 0.0)
  };

  let glyph: Element<'a, Message> = if let Some(tint) = dot {
    Column::with_children(vec![
      text(day.day().to_string())
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(label_color))
        .into(),
      container(Space::new())
        .width(Length::Fixed(DAY_DOT))
        .height(Length::Fixed(DAY_DOT))
        .style(move |_| container::Style {
          background: Some(Background::Color(if is_today { color::surface::BASE } else { tint })),
          border: Border {
            radius: (DAY_DOT / 2.0).into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into(),
    ])
    .spacing(1.0)
    .align_x(Horizontal::Center)
    .into()
  } else {
    text(if in_month { day.day().to_string() } else { String::new() })
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(label_color))
      .into()
  };

  let cell = container(glyph)
    .width(Length::FillPortion(1))
    .height(Length::Fixed(22.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: is_today.then_some(Background::Color(color::accent())),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  if in_month {
    button(cell)
      .padding(0.0)
      .width(Length::FillPortion(1))
      .on_press(Message::DatePicked(day, View::Day))
      .style(|_, _| button::Style::default())
      .into()
  } else {
    container(cell).width(Length::FillPortion(1)).into()
  }
}

fn mini_month<'a>(
  state: &'a State,
  now: DateTime<Utc>,
  events: &[&'a CalendarEvent],
  year: i32,
  month0: u32,
  week_start: crate::config::CalendarWeekStart,
) -> Element<'a, Message> {
  let matrix = grid::month_matrix(year, month0, week_start);
  let weekdays = grid::visible_weekdays(week_start, true);
  let count = events
    .iter()
    .filter(|event| {
      event
        .start()
        .map(|start| start.year() == year && start.month0() == month0)
        .unwrap_or(false)
    })
    .count();

  let dow_header = Row::with_children(
    weekdays
      .iter()
      .map(|weekday| {
        container(
          text(weekday_min(*weekday))
            .font(typography::mono::REGULAR)
            .size(typography::size::XS)
            .style(typography::colored(color::text::tertiary())),
        )
        .width(Length::FillPortion(1))
        .align_x(Horizontal::Center)
        .into()
      })
      .collect::<Vec<_>>(),
  )
  .spacing(2.0)
  .width(Length::Fill);

  let week_rows: Vec<Element<'a, Message>> = (0..6)
    .map(|week| {
      let cells: Vec<Element<'a, Message>> = matrix[week * 7..week * 7 + 7]
        .iter()
        .map(|day| day_button(state, now, events, *day, month0))
        .collect();
      Row::with_children(cells).spacing(2.0).width(Length::Fill).into()
    })
    .collect();

  let grid_body = Column::with_children(week_rows).spacing(2.0).width(Length::Fill);

  Column::with_children(vec![
    month_title(year, month0, count),
    rule::horizontal(),
    dow_header.into(),
    grid_body.into(),
  ])
  .spacing(spacing::SPACE_2)
  .width(Length::FillPortion(1))
  .into()
}

fn month_title<'a>(year: i32, month0: u32, count: usize) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![
    text(month_long(month0 + 1))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    Space::new().width(Length::Fill).into(),
  ];
  if count > 0 {
    children.push(
      text(count.to_string())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }

  button(
    Row::with_children(children)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 2.0,
    bottom: spacing::UNIT,
    left: 2.0,
    right: 2.0,
  })
  .width(Length::Fill)
  .on_press(Message::DatePicked(month_cursor(year, month0), View::Month))
  .style(|_, status| {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      text_color: if hovered { color::accent() } else { color::text::PRIMARY },
      ..button::Style::default()
    }
  })
  .into()
}

fn month_cursor(year: i32, month0: u32) -> DateTime<Utc> {
  chrono::NaiveDate::from_ymd_opt(year, month0 + 1, 1)
    .and_then(|date| date.and_hms_opt(0, 0, 0))
    .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
    .unwrap_or_else(Utc::now)
}

fn weekday_min(weekday: u32) -> &'static str {
  const DAYS: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];
  DAYS[(weekday as usize).min(6)]
}
