//! Snooze dropdown overlay: preset buttons and a custom date/time calendar.

use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc, Weekday};
use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text},
};

use super::{MailMessage, reading_pane::Message};
use crate::{
  components::Icon,
  style::{
    color,
    typography::{body, mono},
  },
};

const WEEKDAY_LABELS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
const MONTH_NAMES: [&str; 12] = [
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

/// EVE downtime hour (UTC).
const DOWNTIME_HOUR: u32 = 11;
/// EVE downtime minute (UTC).
const DOWNTIME_MINUTE: u32 = 0;

/// Mutable calendar state for the custom date/time picker.
#[derive(Clone, Debug)]
pub struct CalendarState {
  /// Hour of the selected time (0–23 UTC).
  pub hour: u32,
  /// Minute of the selected time (0–59 UTC).
  pub minute: u32,
  /// Day of the selected date (1-based).
  pub sel_day: u32,
  /// Month of the selected date (0-based, 0 = January).
  pub sel_month: u32,
  /// Year of the selected date.
  pub sel_year: i32,
  /// Month currently shown in the calendar grid (0-based).
  pub view_month: u32,
  /// Year currently shown in the calendar grid.
  pub view_year: i32,
}

impl CalendarState {
  /// Creates a new calendar state defaulting to tomorrow at 09:00 UTC.
  pub fn new() -> Self {
    let now = Utc::now();
    let tomorrow = now.date_naive().succ_opt().unwrap_or(now.date_naive());
    Self {
      hour: 9,
      minute: 0,
      sel_day: tomorrow.day(),
      sel_month: tomorrow.month0(),
      sel_year: tomorrow.year(),
      view_month: tomorrow.month0(),
      view_year: tomorrow.year(),
    }
  }

  /// Formats the currently-selected date and time as an ISO 8601 UTC string.
  pub fn to_iso(&self) -> Option<String> {
    let date = NaiveDate::from_ymd_opt(self.sel_year, self.sel_month + 1, self.sel_day)?;
    let dt = date.and_hms_opt(self.hour, self.minute, 0)?;
    Some(format!("{}Z", dt.format("%Y-%m-%dT%H:%M:%S")))
  }

  /// Returns a human-readable summary of the selected date and time.
  pub fn display_label(&self) -> String {
    let day_name = NaiveDate::from_ymd_opt(self.sel_year, self.sel_month + 1, self.sel_day)
      .map(|d| weekday_abbr(d.weekday()))
      .unwrap_or("?");
    format!(
      "{} {:02} {} · {:02}:{:02}",
      day_name,
      self.sel_day,
      &MONTH_NAMES[self.sel_month as usize][..3],
      self.hour,
      self.minute,
    )
  }
}

impl Default for CalendarState {
  fn default() -> Self {
    Self::new()
  }
}

/// Compute the ISO timestamp for a named preset.
///
/// Returns `None` when the preset label is not recognised or the target
/// date is unrepresentable.
pub fn preset_to_iso(label: &str) -> Option<String> {
  let now = Utc::now();
  let today = now.date_naive();
  Some(
    preset_target_time(label, today, &now)?
      .format("%Y-%m-%dT%H:%M:%SZ")
      .to_string(),
  )
}

fn preset_target_time(label: &str, today: NaiveDate, now: &DateTime<Utc>) -> Option<DateTime<Utc>> {
  match label {
    "Later today" => preset_later_today_target(today),
    "Tomorrow" => preset_tomorrow_target(today),
    "After downtime" => preset_after_downtime_target(today, now),
    "Next week" => preset_next_week_target(today),
    _ => None,
  }
}

fn preset_later_today_target(today: NaiveDate) -> Option<DateTime<Utc>> {
  Some(today.and_hms_opt(18, 0, 0)?.and_utc())
}

fn preset_tomorrow_target(today: NaiveDate) -> Option<DateTime<Utc>> {
  Some(today.succ_opt()?.and_hms_opt(9, 0, 0)?.and_utc())
}

fn preset_after_downtime_target(today: chrono::NaiveDate, now: &DateTime<Utc>) -> Option<DateTime<Utc>> {
  let downtime = today.and_hms_opt(DOWNTIME_HOUR, DOWNTIME_MINUTE, 0)?.and_utc();
  if *now >= downtime {
    today
      .succ_opt()?
      .and_hms_opt(DOWNTIME_HOUR, DOWNTIME_MINUTE, 0)?
      .and_utc()
      .into()
  } else {
    downtime.into()
  }
}

fn preset_next_week_target(today: chrono::NaiveDate) -> Option<DateTime<Utc>> {
  let days_until_mon = match today.weekday() {
    Weekday::Mon => 7,
    d => (8 - d.number_from_monday() as i64).rem_euclid(7).max(1),
  };
  (today + chrono::Duration::days(days_until_mon))
    .and_hms_opt(9, 0, 0)?
    .and_utc()
    .into()
}

/// Maps a `Weekday` to its 3-letter abbreviation.
fn weekday_abbr(weekday: Weekday) -> &'static str {
  match weekday {
    Weekday::Mon => "Mon",
    Weekday::Tue => "Tue",
    Weekday::Wed => "Wed",
    Weekday::Thu => "Thu",
    Weekday::Fri => "Fri",
    Weekday::Sat => "Sat",
    Weekday::Sun => "Sun",
  }
}

/// Formats a snooze date relative to today: "Today", "Tomorrow", or
/// "Mon DD MMM".
fn format_snooze_date_part(
  snooze_date: chrono::NaiveDate,
  today: chrono::NaiveDate,
  day_name: &str,
  day: u32,
  month_abbr: &str,
) -> String {
  let tomorrow = today.succ_opt().unwrap_or(today);
  if snooze_date == today {
    "Today".to_string()
  } else if snooze_date == tomorrow {
    "Tomorrow".to_string()
  } else {
    format!("{} {} {}", day_name, day, month_abbr)
  }
}

/// Format an ISO 8601 UTC snooze timestamp into a human-readable label.
///
/// Returns labels relative to today:
/// - "Today at HH:MM" when the expiry falls on the current UTC day
/// - "Tomorrow at HH:MM" when the expiry falls on the next UTC day
/// - "Mon DD MMM at HH:MM" for dates further out
///
/// Returns the raw ISO string on parse failure.
pub fn format_snooze_expiry(iso: &str) -> String {
  use chrono::DateTime;
  let Ok(dt) = iso.parse::<DateTime<Utc>>() else {
    return iso.to_string();
  };
  let today = Utc::now().date_naive();
  let snooze_date = dt.date_naive();
  let day_name = weekday_abbr(dt.weekday());
  let month_abbr = &MONTH_NAMES[dt.month0() as usize][..3];
  let time_str = format!("{:02}:{:02}", dt.hour(), dt.minute());
  let date_part = format_snooze_date_part(snooze_date, today, day_name, dt.day(), month_abbr);
  format!("{date_part} at {time_str}")
}

fn preset_button(label: &'static str, hint: &'static str) -> Element<'static, Message> {
  let iso = preset_to_iso(label).unwrap_or_default();
  button(
    row([
      text(label)
        .font(body::REGULAR)
        .size(13.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .width(Length::Fill)
        .into(),
      text(hint)
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(Message::SnoozeSet(iso))
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
      _ => None,
    },
    border: Border {
      radius: 6.0.into(),
      ..Border::default()
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn pick_datetime_button() -> Element<'static, Message> {
  button(
    row([
      Icon::snooze()
        .size(13.0)
        .color(color::text::SECONDARY)
        .render::<Message>(),
      text("Pick date & time…")
        .font(body::REGULAR)
        .size(13.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .spacing(8.0)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(Message::SnoozeCalendarOpen)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
      _ => None,
    },
    border: Border {
      radius: 6.0.into(),
      ..Border::default()
    },
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  })
  .into()
}

fn unsnooze_button() -> Element<'static, Message> {
  button(
    text("Unsnooze")
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::status::DANGER),
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(Message::SnoozeSet(String::new()))
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::status::DANGER_FAINT)),
      _ => None,
    },
    border: Border {
      radius: 6.0.into(),
      ..Border::default()
    },
    text_color: color::status::DANGER,
    ..button::Style::default()
  })
  .into()
}

fn thin_rule() -> Element<'static, Message> {
  container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into()
}

fn preset_dropdown(is_snoozed: bool) -> Element<'static, Message> {
  let header: Element<'_, Message> = container(text("SNOOZE UNTIL").font(mono::REGULAR).size(9.0).style(
    |_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    },
  ))
  .padding(Padding {
    top: 8.0,
    bottom: 6.0,
    left: 10.0,
    right: 10.0,
  })
  .into();

  let mut children: Vec<Element<'_, Message>> = vec![
    header,
    preset_button("Later today", "18:00 EVE"),
    preset_button("Tomorrow", "09:00 EVE"),
    preset_button("After downtime", "11:00 EVE"),
    preset_button("Next week", "Mon 09:00"),
    thin_rule(),
    pick_datetime_button(),
  ];

  if is_snoozed {
    children.push(thin_rule());
    children.push(unsnooze_button());
  }

  crate::components::Card::new(column(children).width(Length::Fixed(240.0)))
    .padding(6.0)
    .render()
}

fn calendar_nav_button(label: &'static str, msg: Message) -> Element<'static, Message> {
  button(
    text(label)
      .font(mono::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 6.0,
    right: 6.0,
  })
  .on_press(msg)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
      _ => None,
    },
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  })
  .into()
}

fn day_cell_text_color(selected: bool, in_month: bool, is_today: bool) -> Color {
  if selected {
    Color::WHITE
  } else if in_month && is_today {
    color::accent::PLASMA
  } else if in_month {
    color::text::PRIMARY
  } else {
    color::text::TERTIARY
  }
}

fn day_cell_background(selected: bool, status: button::Status) -> Option<Background> {
  if selected {
    Some(Background::Color(color::accent::PLASMA))
  } else {
    match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
      _ => None,
    }
  }
}

fn calendar_day_cell(
  day: u32,
  month: u32,
  year: i32,
  in_month: bool,
  selected: bool,
  is_today: bool,
) -> Element<'static, Message> {
  let label = format!("{day:2}");
  let text_col = day_cell_text_color(selected, in_month, is_today);
  button(
    container(
      text(label)
        .font(mono::REGULAR)
        .size(11.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(text_col),
        }),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill),
  )
  .width(Length::Fixed(28.0))
  .height(Length::Fixed(26.0))
  .padding(0)
  .on_press(Message::SnoozeCalendarSelectDay(year, month, day))
  .style(move |_, status| button::Style {
    background: day_cell_background(selected, status),
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    text_color: text_col,
    ..button::Style::default()
  })
  .into()
}

fn stepper_arrow_button(arrow: &'static str, msg: Message, top_pad: f32, bot_pad: f32) -> Element<'static, Message> {
  button(
    container(
      text(arrow)
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .center_x(Length::Fill),
  )
  .width(Length::Fixed(32.0))
  .padding(Padding {
    top: top_pad,
    bottom: bot_pad,
    left: 0.0,
    right: 0.0,
  })
  .on_press(msg)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
      _ => None,
    },
    border: Border {
      radius: 3.0.into(),
      ..Border::default()
    },
    ..button::Style::default()
  })
  .into()
}

fn time_stepper(label: &'static str, value: u32, up_msg: Message, down_msg: Message) -> Element<'static, Message> {
  column([
    stepper_arrow_button("▲", up_msg, 3.0, 2.0),
    container(
      text(format!("{value:02}"))
        .font(mono::REGULAR)
        .size(18.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        }),
    )
    .center_x(Length::Fixed(32.0))
    .into(),
    stepper_arrow_button("▼", down_msg, 2.0, 3.0),
    text(label)
      .font(mono::REGULAR)
      .size(8.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
  ])
  .align_x(iced::alignment::Horizontal::Center)
  .spacing(2.0)
  .into()
}

fn quick_chip(label: &'static str, hint: &'static str, msg: Message) -> Element<'static, Message> {
  button(
    row([
      text(label)
        .font(body::REGULAR)
        .size(11.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(hint)
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .spacing(4.0)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 8.0,
    right: 8.0,
  })
  .on_press(msg)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
      _ => None,
    },
    border: Border {
      color: color::border::SUBTLE,
      radius: 5.0.into(),
      width: 1.0,
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn prev_month_info(month: u32, year: i32) -> (u32, i32) {
  if month == 0 {
    (11u32, year - 1)
  } else {
    (month - 1, year)
  }
}

fn next_month_info(month: u32, year: i32) -> (u32, i32) {
  if month == 11 {
    (0u32, year + 1)
  } else {
    (month + 1, year)
  }
}

fn build_month_cells(month: u32, year: i32, first_dow: i64) -> Vec<(u32, u32, i32, bool)> {
  let month_days = days_in_month(year, month);
  let (pm, py) = prev_month_info(month, year);
  let prev_days = days_in_month(py, pm);
  let (nm, ny) = next_month_info(month, year);

  let mut cells: Vec<(u32, u32, i32, bool)> = Vec::new();
  for i in 0..first_dow {
    let d = (prev_days as i64 - first_dow + i + 1) as u32;
    cells.push((d, pm, py, false));
  }
  for d in 1..=month_days {
    cells.push((d, month, year, true));
  }
  let remaining = 42 - cells.len();
  for d in 1..=remaining as u32 {
    cells.push((d, nm, ny, false));
  }
  cells
}

fn build_day_grid(state: &CalendarState) -> Vec<Vec<Element<'static, Message>>> {
  let today = Utc::now().date_naive();
  let year = state.view_year;
  let month = state.view_month;

  let first_of_month =
    NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap_or_else(|| NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
  let first_dow = first_of_month.weekday().number_from_monday() as i64 - 1;
  let cells = build_month_cells(month, year, first_dow);

  cells
    .chunks(7)
    .map(|week| {
      week
        .iter()
        .map(|&(d, m, y, in_month)| {
          let is_today = NaiveDate::from_ymd_opt(y, m + 1, d) == Some(today);
          let selected = y == state.sel_year && m == state.sel_month && d == state.sel_day;
          calendar_day_cell(d, m, y, in_month, selected, is_today)
        })
        .collect()
    })
    .collect()
}

fn days_in_month(year: i32, month: u32) -> u32 {
  let next_month = if month == 11 {
    NaiveDate::from_ymd_opt(year + 1, 1, 1)
  } else {
    NaiveDate::from_ymd_opt(year, month + 2, 1)
  };
  next_month.and_then(|d| d.pred_opt()).map(|d| d.day()).unwrap_or(28)
}

fn calendar_back_button() -> Element<'static, Message> {
  button(
    text("Back")
      .font(body::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(Message::SnoozeCalendarClose)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
      _ => None,
    },
    border: Border {
      color: color::border::SUBTLE,
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  })
  .into()
}

fn calendar_confirm_button() -> Element<'static, Message> {
  button(
    text("Snooze")
      .font(body::MEDIUM)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(Color::WHITE),
      }),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 12.0,
    right: 12.0,
  })
  .on_press(Message::SnoozeCalendarConfirm)
  .style(|_, status| button::Style {
    background: Some(Background::Color(match status {
      button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA_ACTIVE,
      _ => color::accent::PLASMA,
    })),
    border: Border {
      radius: 6.0.into(),
      ..Border::default()
    },
    text_color: Color::WHITE,
    ..button::Style::default()
  })
  .into()
}

fn calendar_eve_badge() -> Element<'static, Message> {
  container(
    text("EVE")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 6.0,
    right: 6.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::accent::PLASMA_SUBTLE)),
    border: Border {
      color: color::accent::PLASMA_BORDER,
      radius: 4.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn calendar_header(state: &CalendarState) -> Element<'_, Message> {
  let month_name = MONTH_NAMES[state.view_month as usize];
  let view_year = state.view_year;
  container(
    row([
      calendar_nav_button("◀", Message::SnoozeCalendarPrevMonth),
      Space::new().width(Length::Fill).into(),
      text(format!("{month_name} {view_year}"))
        .font(body::MEDIUM)
        .size(13.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      Space::new().width(Length::Fill).into(),
      calendar_eve_badge(),
      calendar_nav_button("▶", Message::SnoozeCalendarNextMonth),
    ])
    .align_y(Vertical::Center)
    .spacing(4.0),
  )
  .padding(Padding {
    top: 10.0,
    bottom: 8.0,
    left: 10.0,
    right: 10.0,
  })
  .width(Length::Fill)
  .into()
}

fn calendar_grid_section(state: &CalendarState) -> Element<'_, Message> {
  let weekday_row = row(
    WEEKDAY_LABELS
      .iter()
      .map(|w| {
        container(
          text(*w)
            .font(mono::REGULAR)
            .size(9.0)
            .style(|_: &Theme| iced::widget::text::Style {
              color: Some(color::text::TERTIARY),
            }),
        )
        .width(Length::Fixed(28.0))
        .center_x(Length::Fixed(28.0))
        .into()
      })
      .collect::<Vec<_>>(),
  )
  .spacing(2.0);

  let day_grid: Vec<Element<'_, Message>> = build_day_grid(state)
    .into_iter()
    .map(|week_cells| row(week_cells).spacing(2.0).into())
    .collect();

  container(
    column([weekday_row.into()])
      .extend(day_grid)
      .spacing(2.0)
      .padding(Padding {
        top: 4.0,
        bottom: 4.0,
        left: 0.0,
        right: 0.0,
      }),
  )
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: 10.0,
    right: 10.0,
  })
  .width(Length::Fill)
  .into()
}

fn calendar_time_section(state: &CalendarState) -> Element<'_, Message> {
  container(
    row([
      text("TIME")
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      Space::new().width(Length::Fill).into(),
      time_stepper(
        "HH",
        state.hour,
        Message::SnoozeCalendarHourUp,
        Message::SnoozeCalendarHourDown,
      ),
      text(":")
        .font(mono::REGULAR)
        .size(18.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      time_stepper(
        "MM",
        state.minute,
        Message::SnoozeCalendarMinuteUp,
        Message::SnoozeCalendarMinuteDown,
      ),
    ])
    .align_y(Vertical::Center)
    .spacing(4.0),
  )
  .padding(10.0)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 8.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .width(Length::Fill)
  .into()
}

fn calendar_chips_row() -> Element<'static, Message> {
  row([
    quick_chip("Morning", "09:00", Message::SnoozeCalendarChipMorning),
    quick_chip("Downtime", "11:00", Message::SnoozeCalendarChipDowntime),
    quick_chip("Evening", "19:00", Message::SnoozeCalendarChipEvening),
  ])
  .spacing(6.0)
  .into()
}

fn calendar_footer(state: &CalendarState) -> Element<'_, Message> {
  container(
    row([
      text(state.display_label())
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .width(Length::Fill)
        .into(),
      calendar_back_button(),
      calendar_confirm_button(),
    ])
    .spacing(8.0)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 10.0,
    right: 10.0,
  })
  .width(Length::Fill)
  .style(|_| container::Style {
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn calendar_widget(state: &CalendarState) -> Element<'_, Message> {
  crate::components::Card::new(
    column([
      container(thin_rule()).width(Length::Fill).into(),
      calendar_header(state),
      container(thin_rule()).width(Length::Fill).into(),
      container(calendar_grid_section(state))
        .padding(Padding {
          top: 8.0,
          bottom: 4.0,
          left: 0.0,
          right: 0.0,
        })
        .into(),
      container(
        container(calendar_time_section(state))
          .padding(Padding {
            top: 0.0,
            bottom: 0.0,
            left: 10.0,
            right: 10.0,
          })
          .width(Length::Fill),
      )
      .into(),
      container(
        container(calendar_chips_row())
          .padding(Padding {
            top: 8.0,
            bottom: 8.0,
            left: 10.0,
            right: 10.0,
          })
          .width(Length::Fill),
      )
      .into(),
      calendar_footer(state),
    ])
    .width(Length::Fixed(304.0)),
  )
  .padding(0.0)
  .render()
}

/// Builder for the snooze dropdown overlay.
pub struct Component<'a> {
  calendar: Option<&'a CalendarState>,
  msg: &'a MailMessage,
}

impl<'a> Component<'a> {
  /// Create a new snooze picker for the given message.
  ///
  /// Pass `calendar` as `Some` to show the calendar widget instead of the
  /// preset list.
  pub fn new(msg: &'a MailMessage, calendar: Option<&'a CalendarState>) -> Self {
    Self {
      calendar,
      msg,
    }
  }

  /// Render into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let dropdown: Element<'_, Message> = match self.calendar {
      Some(cal) => calendar_widget(cal),
      None => preset_dropdown(self.msg.snoozed.is_some()),
    };

    container(dropdown)
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(iced::alignment::Horizontal::Left)
      .align_y(iced::alignment::Vertical::Top)
      .padding(Padding {
        top: 50.0,
        left: 310.0,
        bottom: 0.0,
        right: 0.0,
      })
      .into()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod calendar_state {
    use super::*;

    mod to_iso {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_formats_selection_as_iso_utc_string() {
        let mut state = CalendarState::new();
        state.sel_year = 2026;
        state.sel_month = 0;
        state.sel_day = 15;
        state.hour = 9;
        state.minute = 30;

        let result = state.to_iso();

        assert_eq!(result, Some("2026-01-15T09:30:00Z".to_string()));
      }

      #[test]
      fn it_zero_pads_single_digit_values() {
        let mut state = CalendarState::new();
        state.sel_year = 2026;
        state.sel_month = 2;
        state.sel_day = 5;
        state.hour = 7;
        state.minute = 4;

        let result = state.to_iso();

        assert_eq!(result, Some("2026-03-05T07:04:00Z".to_string()));
      }
    }

    mod display_label {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_weekday_date_and_time() {
        let mut state = CalendarState::new();
        state.sel_year = 2026;
        state.sel_month = 4;
        state.sel_day = 25;
        state.hour = 14;
        state.minute = 22;

        let result = state.display_label();

        assert_eq!(result, "Mon 25 May · 14:22");
      }
    }
  }

  mod format_snooze_expiry {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_iso_string_on_parse_failure() {
      let result = format_snooze_expiry("not-a-date");

      assert_eq!(result, "not-a-date");
    }

    #[test]
    fn it_formats_a_known_future_date_with_weekday() {
      // 2030-07-15 is a Monday
      let result = format_snooze_expiry("2030-07-15T09:00:00Z");

      assert_eq!(result, "Mon 15 Jul at 09:00");
    }

    #[test]
    fn it_zero_pads_single_digit_time_values() {
      // 2030-07-15 is a Monday
      let result = format_snooze_expiry("2030-07-15T07:05:00Z");

      assert_eq!(result, "Mon 15 Jul at 07:05");
    }

    #[test]
    fn it_returns_some_string_for_valid_iso() {
      let result = format_snooze_expiry("2030-01-01T09:00:00Z");

      assert!(!result.is_empty());
    }
  }

  mod preset_to_iso {
    use super::*;

    #[test]
    fn it_returns_none_for_unknown_preset() {
      let result = preset_to_iso("Unknown preset");

      assert!(result.is_none());
    }

    #[test]
    fn it_returns_some_for_later_today() {
      let result = preset_to_iso("Later today");

      assert!(result.is_some());
      assert!(result.unwrap().contains("T18:00:00Z"));
    }

    #[test]
    fn it_returns_some_for_tomorrow() {
      let result = preset_to_iso("Tomorrow");

      assert!(result.is_some());
      assert!(result.unwrap().contains("T09:00:00Z"));
    }

    #[test]
    fn it_returns_some_for_after_downtime() {
      let result = preset_to_iso("After downtime");

      assert!(result.is_some());
    }

    #[test]
    fn it_returns_some_for_next_week() {
      let result = preset_to_iso("Next week");

      assert!(result.is_some());
      assert!(result.unwrap().contains("T09:00:00Z"));
    }
  }
}
