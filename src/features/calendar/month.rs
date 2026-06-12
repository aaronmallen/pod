use chrono::{DateTime, Datelike, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text},
};

use super::{CalendarEvent, Message, State, grid};
use crate::{
  config::CalendarDensity,
  ui::style::{color, radius, spacing, typography},
};

const CHIP_DOT: f32 = 6.0;
const DOT: f32 = 8.0;
const DAY_NUMBER: f32 = 24.0;

pub(super) fn view<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let tweaks = state.tweaks();
  let cursor = state.cursor();
  let year = cursor.year();
  let month0 = cursor.month0();
  let weekdays = grid::visible_weekdays(tweaks.week_start(), tweaks.show_weekends());
  let events = state.visible_events();
  let matrix = grid::month_matrix(year, month0, tweaks.week_start());

  let header = Row::with_children(
    weekdays
      .iter()
      .map(|weekday| weekday_header(*weekday))
      .collect::<Vec<_>>(),
  )
  .width(Length::Fill);

  let rows: Vec<Element<'a, Message>> = (0..6)
    .map(|week| {
      let cells: Vec<Element<'a, Message>> = matrix[week * 7..week * 7 + 7]
        .iter()
        .filter(|day| weekdays.contains(&day.weekday().num_days_from_sunday()))
        .map(|day| day_cell(state, now, &events, *day, month0))
        .collect();

      Row::with_children(cells)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    })
    .collect();

  let grid_body = Column::with_children(rows).width(Length::Fill).height(Length::Fill);

  container(Column::with_children(vec![header.into(), grid_body.into()]))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn chip<'a>(state: &'a State, event: &'a CalendarEvent) -> Element<'a, Message> {
  let tint = grid::color_for(state, event);
  let mut children: Vec<Element<'a, Message>> = vec![
    container(Space::new())
      .width(Length::Fixed(CHIP_DOT))
      .height(Length::Fixed(CHIP_DOT))
      .style(move |_| container::Style {
        background: Some(Background::Color(tint)),
        border: Border {
          radius: if event.is_all_day() { 1.0 } else { CHIP_DOT / 2.0 }.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
  ];

  if !event.is_all_day()
    && let Some(start) = event.start()
  {
    children.push(
      text(grid::hhmm(start))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }

  children.push(
    text(event.title.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  );

  button(
    Row::with_children(children)
      .spacing(spacing::UNIT + 1.0)
      .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: spacing::UNIT + 1.0,
    right: spacing::UNIT + 1.0,
  })
  .width(Length::Fill)
  .on_press(Message::EventOpened(event.character_id, event.event_id))
  .style(move |_, status| chip_style(tint, status))
  .into()
}

fn chip_style(tint: iced::Color, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: Some(Background::Color(color::with_alpha(
      tint,
      if hovered { 0.26 } else { 0.14 },
    ))),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn day_cell<'a>(
  state: &'a State,
  now: DateTime<Utc>,
  events: &[&'a CalendarEvent],
  day: DateTime<Utc>,
  month0: u32,
) -> Element<'a, Message> {
  let in_month = day.month0() == month0;
  let is_today = grid::day_key(day) == grid::day_key(now);
  let items = grid::events_on_day(events, day);

  let body = Column::with_children(vec![day_number(day, in_month, is_today), day_contents(state, &items)])
    .spacing(spacing::UNIT)
    .width(Length::Fill)
    .height(Length::Fill);

  container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
      top: spacing::UNIT + 2.0,
      bottom: spacing::UNIT,
      left: 7.0,
      right: 7.0,
    })
    .style(move |_| container::Style {
      background: is_today.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.05))),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn day_contents<'a>(state: &'a State, items: &[&'a CalendarEvent]) -> Element<'a, Message> {
  if items.is_empty() {
    return Space::new().width(Length::Fill).into();
  }

  if state.tweaks().month_chips() {
    let max_chips = match state.tweaks().density() {
      CalendarDensity::Comfortable => 3,
      CalendarDensity::Compact => 2,
    };

    let mut children: Vec<Element<'a, Message>> =
      items.iter().take(max_chips).map(|event| chip(state, event)).collect();
    if items.len() > max_chips {
      children.push(more_label(items.len() - max_chips));
    }

    Column::with_children(children).spacing(2.0).width(Length::Fill).into()
  } else {
    dots(state, items)
  }
}

fn day_number<'a>(day: DateTime<Utc>, in_month: bool, is_today: bool) -> Element<'a, Message> {
  let label_color = if is_today {
    color::surface::BASE
  } else if in_month {
    color::text::PRIMARY
  } else {
    color::text::tertiary()
  };

  container(
    text(day.day().to_string())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(label_color)),
  )
  .width(Length::Fixed(DAY_NUMBER))
  .height(Length::Fixed(DAY_NUMBER))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: is_today.then_some(Background::Color(color::accent::PLASMA)),
    border: Border {
      radius: 999.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn dots<'a>(state: &'a State, items: &[&'a CalendarEvent]) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = items
    .iter()
    .take(8)
    .map(|event| {
      let tint = grid::color_for(state, event);
      button(
        container(Space::new())
          .width(Length::Fixed(DOT))
          .height(Length::Fixed(DOT))
          .style(move |_| container::Style {
            background: Some(Background::Color(tint)),
            border: Border {
              radius: (DOT / 2.0).into(),
              ..Border::default()
            },
            ..container::Style::default()
          }),
      )
      .padding(0.0)
      .on_press(Message::EventOpened(event.character_id, event.event_id))
      .style(|_, _| button::Style::default())
      .into()
    })
    .collect();

  if items.len() > 8 {
    children.push(more_label(items.len() - 8));
  }

  Row::with_children(children)
    .spacing(spacing::UNIT)
    .align_y(Vertical::Center)
    .into()
}

fn more_label<'a>(count: usize) -> Element<'a, Message> {
  text(format!("+{count} more"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()))
    .into()
}

fn weekday_header<'a>(weekday: u32) -> Element<'a, Message> {
  container(
    text(weekday_short(weekday))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary())),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_2,
    right: spacing::SPACE_2,
  })
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

fn weekday_short(weekday: u32) -> &'static str {
  const DAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
  DAYS[(weekday as usize).min(6)]
}
