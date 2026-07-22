use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc, Weekday};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, mouse_area, text},
};

use super::{Message, labels};
use crate::{
  store::{Database, repo::mail},
  ui::{
    components::{
      date_picker::{DatePicker, DatePickerState, TimeControls},
      eyebrow::eyebrow_text,
    },
    format::{month_short, weekday_short},
    style::{color, radius, spacing, typography},
  },
};

const MENU_WIDTH: f32 = 240.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Calendar {
  state: DatePickerState,
}

impl Calendar {
  pub(super) fn open(now: DateTime<Utc>) -> Self {
    let tomorrow = (now + Duration::days(1)).date_naive();
    Calendar {
      state: DatePickerState::new(tomorrow, Some((9, 0))),
    }
  }

  pub(super) fn prev_month(&mut self) {
    self.state.prev_month();
  }

  pub(super) fn next_month(&mut self) {
    self.state.next_month();
  }

  pub(super) fn select_day(&mut self, year: i32, month0: u32, day: u32) {
    if let Some(date) = NaiveDate::from_ymd_opt(year, month0 + 1, day) {
      self.state.select(date);
    }
  }

  pub(super) fn hour_up(&mut self) {
    self.state.hour_up();
  }

  pub(super) fn hour_down(&mut self) {
    self.state.hour_down();
  }

  pub(super) fn minute_up(&mut self) {
    self.state.minute_up();
  }

  pub(super) fn minute_down(&mut self) {
    self.state.minute_down();
  }

  pub(super) fn set_time(&mut self, hour: u32, minute: u32) {
    self.state.set_time(hour, minute);
  }

  pub(super) fn resolved(&self) -> Option<DateTime<Utc>> {
    let date = self.state.selection();
    let (hour, minute) = self.state.time()?;
    Utc
      .with_ymd_and_hms(date.year(), date.month(), date.day(), hour, minute, 0)
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
  DatePicker::new(
    &cal.state,
    |date| Message::SnoozeCalendarDaySelected(date.year(), date.month0(), date.day()),
    Message::SnoozeCalendarPrevMonth,
    Message::SnoozeCalendarNextMonth,
  )
  .eve_tag(t!("mail.snooze.eve"))
  .time(TimeControls {
    label: t!("mail.snooze.time").into_owned(),
    on_hour_up: Message::SnoozeCalendarHourUp,
    on_hour_down: Message::SnoozeCalendarHourDown,
    on_minute_up: Message::SnoozeCalendarMinuteUp,
    on_minute_down: Message::SnoozeCalendarMinuteDown,
  })
  .footer(footer(cal))
  .view()
}

fn footer(cal: &Calendar) -> Element<'_, Message> {
  let resolved = cal
    .resolved()
    .map(|d| {
      format!(
        "{} {:02} {} · {:02}:{:02}",
        weekday_short(d.weekday()),
        d.day(),
        month_short(d.month()),
        d.hour(),
        d.minute()
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
      assert_eq!(cal.resolved(), Some(Utc.with_ymd_and_hms(2026, 6, 2, 9, 0, 0).unwrap()));
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
