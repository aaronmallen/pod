use chrono::{DateTime, Datelike, Duration, TimeZone, Utc, Weekday};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container, mouse_area, text},
};

use super::{Message, labels};
use crate::{
  store::{Database, repo::mail},
  ui::{
    components::{eyebrow::eyebrow_text, icon::Icon},
    format::{month_long, month_short, weekday_short},
    style::{color, radius, spacing, typography},
  },
};

const MENU_WIDTH: f32 = 240.0;

const CALENDAR_WIDTH: f32 = 304.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Calendar {
  pub hour: u32,
  pub minute: u32,
  pub sel_day: u32,
  pub sel_month0: u32,
  pub sel_year: i32,
  pub view_month0: u32,
  pub view_year: i32,
}

impl Calendar {
  pub(super) fn open(now: DateTime<Utc>) -> Self {
    let tomorrow = (now + Duration::days(1)).date_naive();
    Calendar {
      view_year: tomorrow.year(),
      view_month0: tomorrow.month0(),
      sel_year: tomorrow.year(),
      sel_month0: tomorrow.month0(),
      sel_day: tomorrow.day(),
      hour: 9,
      minute: 0,
    }
  }

  pub(super) fn prev_month(&mut self) {
    if self.view_month0 == 0 {
      self.view_month0 = 11;
      self.view_year -= 1;
    } else {
      self.view_month0 -= 1;
    }
  }

  pub(super) fn next_month(&mut self) {
    if self.view_month0 == 11 {
      self.view_month0 = 0;
      self.view_year += 1;
    } else {
      self.view_month0 += 1;
    }
  }

  pub(super) fn select_day(&mut self, year: i32, month0: u32, day: u32) {
    self.sel_year = year;
    self.sel_month0 = month0;
    self.sel_day = day;
  }

  pub(super) fn hour_up(&mut self) {
    self.hour = (self.hour + 1) % 24;
  }

  pub(super) fn hour_down(&mut self) {
    self.hour = (self.hour + 23) % 24;
  }

  pub(super) fn minute_up(&mut self) {
    let next = self.minute + 5;
    if next >= 60 {
      self.minute = next - 60;
      self.hour_up();
    } else {
      self.minute = next;
    }
  }

  pub(super) fn minute_down(&mut self) {
    if self.minute < 5 {
      self.minute = 60 + self.minute - 5;
      self.hour_down();
    } else {
      self.minute -= 5;
    }
  }

  pub(super) fn set_time(&mut self, hour: u32, minute: u32) {
    self.hour = hour;
    self.minute = minute;
  }

  pub(super) fn resolved(&self) -> Option<DateTime<Utc>> {
    Utc
      .with_ymd_and_hms(
        self.sel_year,
        self.sel_month0 + 1,
        self.sel_day,
        self.hour,
        self.minute,
        0,
      )
      .single()
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Preset {
  LaterToday,
  NextWeek,
  ThisWeekend,
  Tomorrow,
}

impl Preset {
  pub(super) fn label(self) -> String {
    match self {
      Preset::LaterToday => t!("mail.snooze.preset.later_today"),
      Preset::Tomorrow => t!("mail.snooze.preset.tomorrow"),
      Preset::ThisWeekend => t!("mail.snooze.preset.this_weekend"),
      Preset::NextWeek => t!("mail.snooze.preset.next_week"),
    }
    .into_owned()
  }

  pub(super) fn hint(self) -> String {
    match self {
      Preset::LaterToday => t!("mail.snooze.preset.later_today_hint"),
      Preset::Tomorrow => t!("mail.snooze.preset.tomorrow_hint"),
      Preset::ThisWeekend => t!("mail.snooze.preset.this_weekend_hint"),
      Preset::NextWeek => t!("mail.snooze.preset.next_week_hint"),
    }
    .into_owned()
  }

  pub(super) fn all() -> [Preset; 4] {
    [
      Preset::LaterToday,
      Preset::Tomorrow,
      Preset::ThisWeekend,
      Preset::NextWeek,
    ]
  }

  pub(super) fn resolve(self, now: DateTime<Utc>) -> DateTime<Utc> {
    match self {
      Preset::LaterToday => {
        let today_18 = at_time(now, 18, 0);
        if now < today_18 {
          today_18
        } else {
          at_time(now + Duration::days(1), 18, 0)
        }
      }
      Preset::Tomorrow => at_time(now + Duration::days(1), 9, 0),
      Preset::ThisWeekend => {
        let sat_today = at_time(now, 9, 0);
        if now.weekday() == Weekday::Sat && now < sat_today {
          sat_today
        } else {
          at_time(next_weekday(now, Weekday::Sat), 9, 0)
        }
      }
      Preset::NextWeek => at_time(next_weekday(now, Weekday::Mon), 9, 0),
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DayCell {
  pub day: u32,
  pub in_month: bool,
  pub month0: u32,
  pub year: i32,
}

fn at_time(from: DateTime<Utc>, hour: u32, minute: u32) -> DateTime<Utc> {
  let date = from.date_naive();
  Utc
    .with_ymd_and_hms(date.year(), date.month(), date.day(), hour, minute, 0)
    .single()
    .unwrap_or(from)
}

fn next_weekday(now: DateTime<Utc>, target: Weekday) -> DateTime<Utc> {
  let mut d = now + Duration::days(1);
  while d.weekday() != target {
    d += Duration::days(1);
  }
  d
}

pub(super) fn canonical_until(until: &str) -> String {
  match DateTime::parse_from_rfc3339(until) {
    Ok(parsed) => parsed.with_timezone(&Utc).format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    Err(_) => until.to_owned(),
  }
}

pub(super) async fn snooze_until(db: Database, character_id: i64, mail_id: i64, until: String) {
  let until = canonical_until(&until);
  let _ = mail::upsert_snoozed_mail(&db, character_id, mail_id, &until).await;
  labels::enqueue_snooze_flip(db, character_id, mail_id).await;
}

pub(super) async fn unsnooze(db: Database, character_id: i64, mail_id: i64) {
  let _ = mail::delete_snoozed_mail(&db, character_id, mail_id).await;
  labels::enqueue_wake_flip(db, character_id, mail_id).await;
}

pub(super) fn month_grid(year: i32, month0: u32) -> Vec<DayCell> {
  let first = chrono::NaiveDate::from_ymd_opt(year, month0 + 1, 1).expect("valid month");
  let first_dow = first.weekday().num_days_from_monday();
  let dim = days_in_month(year, month0);
  let (prev_year, prev_month0) = if month0 == 0 {
    (year - 1, 11)
  } else {
    (year, month0 - 1)
  };
  let (next_year, next_month0) = if month0 == 11 {
    (year + 1, 0)
  } else {
    (year, month0 + 1)
  };
  let prev_dim = days_in_month(prev_year, prev_month0);

  let mut cells = Vec::with_capacity(42);
  for i in 0..42i32 {
    let day_index = i - first_dow as i32;
    let cell = if day_index < 0 {
      DayCell {
        year: prev_year,
        month0: prev_month0,
        day: (prev_dim as i32 + day_index + 1) as u32,
        in_month: false,
      }
    } else if day_index >= dim as i32 {
      DayCell {
        year: next_year,
        month0: next_month0,
        day: (day_index - dim as i32 + 1) as u32,
        in_month: false,
      }
    } else {
      DayCell {
        year,
        month0,
        day: (day_index + 1) as u32,
        in_month: true,
      }
    };
    cells.push(cell);
  }
  cells
}

fn days_in_month(year: i32, month0: u32) -> u32 {
  let (next_year, next_month0) = if month0 == 11 {
    (year + 1, 0)
  } else {
    (year, month0 + 1)
  };
  let first_next = chrono::NaiveDate::from_ymd_opt(next_year, next_month0 + 1, 1).expect("valid month");
  first_next.pred_opt().expect("non-min date").day()
}

pub(super) fn presets_menu<'a>(is_snoozed: bool, selected: Option<i64>) -> Element<'a, Message> {
  let mut column = Column::new()
    .spacing(spacing::UNIT / 2.0)
    .push(menu_header(&t!("mail.snooze.snooze_until")));

  for preset in Preset::all() {
    column = column.push(preset_row(preset, selected.is_some()));
  }

  column = column.push(menu_divider());
  column = column.push(action_row(
    &t!("mail.snooze.pick_date"),
    Message::SnoozeCalendarOpened,
    false,
  ));
  if is_snoozed && let Some(mail_id) = selected {
    column = column.push(action_row(
      &t!("mail.snooze.unsnooze"),
      Message::Unsnooze(mail_id),
      true,
    ));
  }

  menu_panel(column.into(), MENU_WIDTH)
}

fn preset_row<'a>(preset: Preset, enabled: bool) -> Element<'a, Message> {
  let row = Row::with_children(vec![
    text(preset.label())
      .size(typography::size::MD)
      .width(Length::Fill)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(preset.hint())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .align_y(Vertical::Center);

  let cell = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2_5,
    right: spacing::SPACE_2_5,
  });

  if enabled {
    mouse_area(cell).on_press(Message::SnoozePreset(preset)).into()
  } else {
    cell.into()
  }
}

pub(super) fn calendar_menu(cal: &Calendar) -> Element<'_, Message> {
  let header = Row::with_children(vec![
    nav_button(Icon::chevron_left(), Message::SnoozeCalendarPrevMonth),
    container(
      text(format!("{} {}", month_long(cal.view_month0 + 1), cal.view_year))
        .size(typography::size::MD)
        .font(typography::body::MEDIUM)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        }),
    )
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .into(),
    nav_button(Icon::chevron_right(), Message::SnoozeCalendarNextMonth),
    eve_tag(),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Center);

  let column = Column::with_children(vec![
    container(header)
      .padding(Padding {
        top: spacing::SPACE_2_5,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_2_5,
        right: spacing::SPACE_2_5,
      })
      .into(),
    weekday_header(),
    day_grid(cal),
    time_stepper(cal),
    footer(cal),
  ]);

  menu_panel(column.into(), CALENDAR_WIDTH)
}

fn weekday_header<'a>() -> Element<'a, Message> {
  const WEEKDAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
  let mut row = Row::new().spacing(2.0);
  for w in WEEKDAYS {
    row = row.push(
      container(
        text(w)
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(|_| text::Style {
            color: Some(color::text::tertiary()),
          }),
      )
      .width(Length::Fill)
      .align_x(Horizontal::Center),
    );
  }
  container(row)
    .padding(Padding {
      top: 0.0,
      bottom: spacing::UNIT,
      left: spacing::SPACE_2_5,
      right: spacing::SPACE_2_5,
    })
    .into()
}

fn day_grid(cal: &Calendar) -> Element<'_, Message> {
  let cells = month_grid(cal.view_year, cal.view_month0);
  let mut grid = Column::new().spacing(2.0);
  for week in cells.chunks(7) {
    let mut row = Row::new().spacing(2.0);
    for cell in week {
      let selected =
        cell.year == cal.sel_year && cell.month0 == cal.sel_month0 && cell.day == cal.sel_day && cell.in_month;
      row = row.push(day_cell(*cell, selected));
    }
    grid = grid.push(row);
  }
  container(grid)
    .padding(Padding {
      top: 0.0,
      bottom: spacing::UNIT,
      left: spacing::SPACE_2_5,
      right: spacing::SPACE_2_5,
    })
    .into()
}

fn day_cell<'a>(cell: DayCell, selected: bool) -> Element<'a, Message> {
  let day_color = if selected {
    color::surface::BASE
  } else if cell.in_month {
    color::text::PRIMARY
  } else {
    color::text::tertiary()
  };

  let label = container(
    text(cell.day.to_string())
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(move |_| text::Style {
        color: Some(day_color),
      }),
  )
  .width(Length::Fill)
  .height(Length::Fixed(30.0))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: selected.then_some(Background::Color(color::accent())),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  container(mouse_area(label).on_press(Message::SnoozeCalendarDaySelected(cell.year, cell.month0, cell.day)))
    .width(Length::Fill)
    .into()
}

fn time_stepper(cal: &Calendar) -> Element<'_, Message> {
  let block = Row::with_children(vec![
    eyebrow_text(&t!("mail.snooze.time"), None).width(Length::Fill).into(),
    stepper(cal.hour, Message::SnoozeCalendarHourUp, Message::SnoozeCalendarHourDown),
    text(":")
      .font(typography::mono::REGULAR)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    stepper(
      cal.minute,
      Message::SnoozeCalendarMinuteUp,
      Message::SnoozeCalendarMinuteDown,
    ),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  container(block)
    .padding(spacing::SPACE_2_5)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn stepper<'a>(value: u32, up: Message, down: Message) -> Element<'a, Message> {
  Row::with_children(vec![
    step_button("\u{2013}", down),
    container(
      text(format!("{value:02}"))
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        }),
    )
    .width(Length::Fixed(30.0))
    .align_x(Horizontal::Center)
    .into(),
    step_button("+", up),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn step_button<'a>(glyph: &str, message: Message) -> Element<'a, Message> {
  mouse_area(
    container(
      text(glyph.to_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        }),
    )
    .width(Length::Fixed(22.0))
    .height(Length::Fixed(22.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center),
  )
  .on_press(message)
  .into()
}

fn footer(cal: &Calendar) -> Element<'_, Message> {
  let resolved = cal
    .resolved()
    .map(|d| {
      format!(
        "{} {:02} {} · {:02}:{:02}",
        weekday_short(d.weekday()),
        d.day(),
        month_short(cal.sel_month0 + 1),
        cal.hour,
        cal.minute
      )
    })
    .unwrap_or_else(|| t!("mail.snooze.empty_summary").into_owned());

  let summary = Column::with_children(vec![
    eyebrow_text(&t!("mail.snooze.summary_eyebrow"), Some(color::text::tertiary())).into(),
    text(resolved)
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::UNIT / 2.0)
  .width(Length::Fill);

  let row = Row::with_children(vec![
    summary.into(),
    footer_button(&t!("mail.snooze.back"), Message::SnoozeCalendarBack, false),
    footer_button(&t!("mail.snooze.confirm"), Message::SnoozeCalendarConfirmed, true),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(spacing::SPACE_2_5)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: top_rule(),
      ..container::Style::default()
    })
    .into()
}

fn footer_button<'a>(label: &str, message: Message, primary: bool) -> Element<'a, Message> {
  let (fg, bg, border) = if primary {
    (color::surface::BASE, Some(color::accent()), color::accent())
  } else {
    (
      color::text::secondary(),
      None,
      color::with_alpha(color::text::PRIMARY, 0.1),
    )
  };

  mouse_area(
    container(
      text(label.to_owned())
        .size(typography::size::MD)
        .font(typography::body::MEDIUM)
        .style(move |_| text::Style {
          color: Some(fg),
        }),
    )
    .padding(Padding {
      top: spacing::SPACE_2 - 2.0,
      bottom: spacing::SPACE_2 - 2.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .style(move |_| container::Style {
      background: bg.map(Background::Color),
      border: Border {
        color: border,
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    }),
  )
  .on_press(message)
  .into()
}

fn nav_button<'a>(icon: Icon, message: Message) -> Element<'a, Message> {
  mouse_area(
    container(icon.size(16.0).color(color::text::secondary()).render::<Message>())
      .width(Length::Fixed(24.0))
      .height(Length::Fixed(24.0))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .on_press(message)
  .into()
}

fn eve_tag<'a>() -> Element<'a, Message> {
  container(
    text(t!("mail.snooze.eve"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::accent()),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: spacing::UNIT + 2.0,
    right: spacing::UNIT + 2.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent(), 0.16))),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn menu_header<'a>(label: &str) -> Element<'a, Message> {
  container(eyebrow_text(label, None))
    .padding(Padding {
      top: spacing::SPACE_2,
      bottom: spacing::UNIT,
      left: spacing::SPACE_2_5,
      right: spacing::SPACE_2_5,
    })
    .into()
}

fn action_row<'a>(label: &str, message: Message, danger: bool) -> Element<'a, Message> {
  let tone = if danger {
    color::status::DANGER
  } else {
    color::text::secondary()
  };
  mouse_area(
    container(
      text(label.to_owned())
        .size(typography::size::MD)
        .style(move |_| text::Style {
          color: Some(tone),
        }),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_2_5,
      right: spacing::SPACE_2_5,
    }),
  )
  .on_press(message)
  .into()
}

fn menu_divider<'a>() -> Element<'a, Message> {
  container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .padding(Padding {
      top: spacing::UNIT,
      bottom: spacing::UNIT,
      left: spacing::UNIT,
      right: spacing::UNIT,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
      ..container::Style::default()
    })
    .into()
}

fn menu_panel<'a>(content: Element<'a, Message>, width: f32) -> Element<'a, Message> {
  container(content)
    .width(Length::Fixed(width))
    .padding(spacing::UNIT + 2.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.16),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn top_rule() -> Border {
  Border {
    color: color::with_alpha(color::text::PRIMARY, 0.1),
    radius: 0.0.into(),
    width: 0.0,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 1, 14, 22, 0).unwrap()
  }

  mod calendar {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_opens_defaulted_to_tomorrow_09_00() {
      let cal = Calendar::open(now());
      assert_eq!((cal.sel_year, cal.sel_month0, cal.sel_day), (2026, 5, 2));
      assert_eq!((cal.hour, cal.minute), (9, 0));
    }

    #[test]
    fn minute_down_borrows_the_hour() {
      let mut cal = Calendar::open(now());
      cal.set_time(11, 0);
      cal.minute_down();
      assert_eq!((cal.hour, cal.minute), (10, 55));
    }

    #[test]
    fn minute_up_wraps_and_carries_the_hour() {
      let mut cal = Calendar::open(now());
      cal.set_time(10, 55);
      cal.minute_up();
      assert_eq!((cal.hour, cal.minute), (11, 0));
    }

    #[test]
    fn month_nav_wraps_the_year() {
      let mut cal = Calendar::open(now());
      cal.view_year = 2026;
      cal.view_month0 = 0;
      cal.prev_month();
      assert_eq!((cal.view_year, cal.view_month0), (2025, 11));
      cal.next_month();
      assert_eq!((cal.view_year, cal.view_month0), (2026, 0));
    }

    #[test]
    fn resolved_builds_the_selected_utc_instant() {
      let mut cal = Calendar::open(now());
      cal.select_day(2026, 5, 10);
      cal.set_time(11, 0);
      assert_eq!(
        cal.resolved(),
        Some(Utc.with_ymd_and_hms(2026, 6, 10, 11, 0, 0).unwrap())
      );
    }
  }

  mod canonical_until {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_leaves_an_already_canonical_string_untouched() {
      assert_eq!(canonical_until("2026-06-06T05:55:00Z"), "2026-06-06T05:55:00Z");
    }

    #[test]
    fn it_normalises_a_non_utc_offset_to_utc() {
      assert_eq!(canonical_until("2026-06-06T07:55:00+02:00"), "2026-06-06T05:55:00Z");
    }

    #[test]
    fn it_passes_through_an_unparseable_string() {
      assert_eq!(canonical_until("not-a-date"), "not-a-date");
    }

    #[test]
    fn it_rewrites_an_offset_timestamp_to_a_z_suffixed_seconds_string() {
      assert_eq!(canonical_until("2026-06-06T05:55:00+00:00"), "2026-06-06T05:55:00Z");
    }
  }

  mod grid {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_builds_a_full_42_cell_monday_first_grid() {
      let cells = month_grid(2026, 5);
      assert_eq!(cells.len(), 42);
      assert_eq!((cells[0].day, cells[0].in_month), (1, true));
      assert_eq!((cells[30].day, cells[30].in_month, cells[30].month0), (1, false, 6));
    }

    #[test]
    fn it_counts_february_in_a_leap_year() {
      assert_eq!(days_in_month(2024, 1), 29);
      assert_eq!(days_in_month(2026, 1), 28);
    }

    #[test]
    fn it_pads_with_the_previous_month_when_the_first_is_not_monday() {
      let cells = month_grid(2026, 4);
      assert!(!cells[0].in_month);
      assert_eq!(cells[0].month0, 3);
      assert!(cells[4].in_month);
      assert_eq!(cells[4].day, 1);
    }
  }

  mod presets {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn every_preset_is_strictly_in_the_future() {
      let n = now();
      for preset in Preset::all() {
        assert!(preset.resolve(n) > n, "{} must resolve to the future", preset.label());
      }
    }

    #[test]
    fn it_lists_presets_in_chronological_render_order() {
      assert_eq!(
        Preset::all(),
        [
          Preset::LaterToday,
          Preset::Tomorrow,
          Preset::ThisWeekend,
          Preset::NextWeek
        ]
      );
    }

    #[test]
    fn later_today_resolves_to_18_00_utc_today_when_before_18() {
      let r = Preset::LaterToday.resolve(now());
      assert_eq!(r, Utc.with_ymd_and_hms(2026, 6, 1, 18, 0, 0).unwrap());
    }

    #[test]
    fn later_today_rolls_to_tomorrow_18_00_when_already_past_18() {
      let evening = Utc.with_ymd_and_hms(2026, 6, 1, 19, 0, 0).unwrap();
      let r = Preset::LaterToday.resolve(evening);
      assert_eq!(r, Utc.with_ymd_and_hms(2026, 6, 2, 18, 0, 0).unwrap());
    }

    #[test]
    fn next_week_resolves_to_the_upcoming_monday_09_00() {
      let r = Preset::NextWeek.resolve(now());
      assert_eq!(r, Utc.with_ymd_and_hms(2026, 6, 8, 9, 0, 0).unwrap());
    }

    #[test]
    fn this_weekend_on_saturday_afternoon_rolls_to_next_saturday() {
      let sat_pm = Utc.with_ymd_and_hms(2026, 6, 6, 15, 0, 0).unwrap();
      let r = Preset::ThisWeekend.resolve(sat_pm);
      assert_eq!(r, Utc.with_ymd_and_hms(2026, 6, 13, 9, 0, 0).unwrap());
    }

    #[test]
    fn this_weekend_resolves_to_the_upcoming_saturday_09_00() {
      let r = Preset::ThisWeekend.resolve(now());
      assert_eq!(r, Utc.with_ymd_and_hms(2026, 6, 6, 9, 0, 0).unwrap());
    }

    #[test]
    fn tomorrow_resolves_to_tomorrow_09_00_utc() {
      let r = Preset::Tomorrow.resolve(now());
      assert_eq!(r, Utc.with_ymd_and_hms(2026, 6, 2, 9, 0, 0).unwrap());
    }
  }
}
