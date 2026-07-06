use std::collections::HashSet;

use chrono::{NaiveDate, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, button, container, mouse_area, text},
};

use super::{Message as Parent, State, entries, eve_date, wizard};
use crate::ui::{
  components::{button::Button, icon::Icon},
  format::month_long,
  style::{color, radius, spacing, typography},
};

const BACK_BUTTON_SIZE: f32 = 34.0;
const BACK_ICON_SIZE: f32 = 16.0;
const CALENDAR_WIDTH: f32 = 304.0;
const DAY_CELL_HEIGHT: f32 = 30.0;
const TITLE_SIZE: f32 = 21.0;

#[derive(Clone, Debug)]
pub enum Message {
  JumpToDay,
  NextMonth,
  PrevMonth,
}

pub(super) fn view(state: &State) -> Element<'_, Parent> {
  let date = displayed_date(state);

  let kicker = text(kicker_text(state).to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::accent()),
    });
  let human = text(human_date(date))
    .font(typography::body::MEDIUM)
    .size(TITLE_SIZE)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let eve = text(eve_date::label(date))
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });
  let date_line = Row::with_children(vec![human.into(), eve.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Bottom);
  let identity = Column::with_children(vec![kicker.into(), date_line.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let mut controls: Vec<Element<'_, Parent>> = vec![
    Button::secondary(t!("captains_log.jump_to_day"))
      .icon(Icon::calendar())
      .on_press(Parent::Header(Message::JumpToDay))
      .into(),
  ];
  if let Some(action) = mode_button(state) {
    controls.push(action);
  }
  let controls = Row::with_children(controls)
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center);

  let row = Row::with_children(vec![back_button(), identity.into(), controls.into()])
    .spacing(spacing::SPACE_4_5)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .height(Length::Fixed(spacing::layout::HEADER_HEIGHT))
    .padding(Padding {
      top: 0.0,
      right: spacing::SPACE_6,
      bottom: 0.0,
      left: spacing::SPACE_6,
    })
    .align_y(Vertical::Center)
    .into()
}

pub(super) fn jump_calendar(state: &State) -> Element<'_, Parent> {
  let today_iso = state.today_date.format("%Y-%m-%d").to_string();
  let available: HashSet<&str> = state.all_dates.iter().map(String::as_str).collect();
  let selected_iso = state.selected.clone().unwrap_or_else(|| today_iso.clone());

  let nav = Row::with_children(vec![
    nav_button(Icon::chevron_left(), Message::PrevMonth),
    container(
      text(format!(
        "{} {}",
        month_long(state.jump_view_month0 + 1),
        state.jump_view_year
      ))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      }),
    )
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .into(),
    nav_button(Icon::chevron_right(), Message::NextMonth),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Center);

  let mut grid = Column::new().spacing(2.0);
  let cells = month_cells(state.jump_view_year, state.jump_view_month0);
  for week in cells.chunks(7) {
    let mut row = Row::new().spacing(2.0);
    for cell in week {
      let iso = cell.iso();
      let enabled = cell.in_month && (iso == today_iso || available.contains(iso.as_str()));
      let selected = iso == selected_iso;
      row = row.push(day_cell(*cell, enabled, selected, &iso, &today_iso));
    }
    grid = grid.push(row);
  }

  let column = Column::with_children(vec![
    container(nav)
      .padding(Padding {
        top: spacing::UNIT,
        bottom: spacing::SPACE_2,
        left: spacing::UNIT,
        right: spacing::UNIT,
      })
      .into(),
    weekday_header(),
    grid.into(),
  ]);

  container(column)
    .width(Length::Fixed(CALENDAR_WIDTH))
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

#[derive(Clone, Copy, Debug)]
struct DayCell {
  day: u32,
  in_month: bool,
  month0: u32,
  year: i32,
}

impl DayCell {
  fn iso(&self) -> String {
    format!("{:04}-{:02}-{:02}", self.year, self.month0 + 1, self.day)
  }
}

fn back_button<'a>() -> Element<'a, Parent> {
  button(
    container(
      Icon::chevron_left()
        .size(BACK_ICON_SIZE)
        .color(color::text::secondary())
        .render(),
    )
    .width(Length::Fixed(BACK_BUTTON_SIZE))
    .height(Length::Fixed(BACK_BUTTON_SIZE))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center),
  )
  .padding(0)
  .on_press(Parent::Exit)
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: None,
      border: Border {
        color: if hover { color::rule_strong() } else { color::rule() },
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: if hover {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn day_cell(cell: DayCell, enabled: bool, selected: bool, iso: &str, today_iso: &str) -> Element<'static, Parent> {
  let day_color = if selected {
    color::surface::BASE
  } else if enabled {
    color::text::PRIMARY
  } else if cell.in_month {
    color::text::tertiary()
  } else {
    color::with_alpha(color::text::tertiary(), 0.45)
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
  .height(Length::Fixed(DAY_CELL_HEIGHT))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: if selected {
      Some(Background::Color(color::accent()))
    } else if enabled {
      Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.04)))
    } else {
      None
    },
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  if !enabled {
    return container(label).width(Length::Fill).into();
  }

  let message = if iso == today_iso {
    Parent::Entries(entries::Message::Selected(None))
  } else {
    Parent::Entries(entries::Message::Selected(Some(iso.to_owned())))
  };

  container(mouse_area(label).on_press(message))
    .width(Length::Fill)
    .into()
}

fn displayed_date(state: &State) -> NaiveDate {
  state
    .selected
    .as_deref()
    .and_then(parse_iso)
    .unwrap_or_else(|| Utc::now().date_naive())
}

fn human_date(date: NaiveDate) -> String {
  date.format("%A, %-d %B").to_string()
}

fn kicker_text(state: &State) -> String {
  if state.selected.is_some() {
    t!("captains_log.header.kicker_past").into_owned()
  } else {
    t!("captains_log.header.kicker").into_owned()
  }
}

fn mode_button(state: &State) -> Option<Element<'_, Parent>> {
  if state.selected.is_some() {
    return Some(
      Button::secondary(t!("captains_log.header.back_to_today"))
        .icon(Icon::chevron_left())
        .on_press(Parent::Entries(entries::Message::Selected(None)))
        .into(),
    );
  }
  if state.wizard.is_finished() {
    return Some(
      Button::secondary(t!("captains_log.header.log_the_day"))
        .icon(Icon::captains_log())
        .on_press(Parent::Wizard(wizard::Message::ContinueEditing))
        .into(),
    );
  }
  None
}

fn month_cells(year: i32, month0: u32) -> Vec<DayCell> {
  let first = NaiveDate::from_ymd_opt(year, month0 + 1, 1).expect("valid month");
  let first_dow = chrono::Datelike::weekday(&first).num_days_from_monday();
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
        day: (prev_dim as i32 + day_index + 1) as u32,
        in_month: false,
        month0: prev_month0,
        year: prev_year,
      }
    } else if day_index >= dim as i32 {
      DayCell {
        day: (day_index - dim as i32 + 1) as u32,
        in_month: false,
        month0: next_month0,
        year: next_year,
      }
    } else {
      DayCell {
        day: (day_index + 1) as u32,
        in_month: true,
        month0,
        year,
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
  let first_next = NaiveDate::from_ymd_opt(next_year, next_month0 + 1, 1).expect("valid month");
  chrono::Datelike::day(&first_next.pred_opt().expect("non-min date"))
}

fn weekday_header<'a>() -> Element<'a, Parent> {
  const WEEKDAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
  let mut row = Row::new().spacing(2.0);
  for label in WEEKDAYS {
    row = row.push(
      container(
        text(label)
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
      left: 0.0,
      right: 0.0,
    })
    .into()
}

fn nav_button<'a>(icon: Icon, message: Message) -> Element<'a, Parent> {
  mouse_area(
    container(icon.size(16.0).color(color::text::secondary()).render::<Parent>())
      .width(Length::Fixed(24.0))
      .height(Length::Fixed(24.0))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .on_press(Parent::Header(message))
  .into()
}

fn parse_iso(iso: &str) -> Option<NaiveDate> {
  NaiveDate::parse_from_str(iso, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::features::roster::captains_log::stub_day;

  fn state_with(selected: Option<&str>, days: &[&str]) -> State {
    let mut state = State::new();
    state.days = days.iter().map(|day| stub_day(day)).collect();
    state.selected = selected.map(str::to_owned);
    state
  }

  mod displayed_date {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_the_selected_day_when_one_is_chosen() {
      let state = state_with(Some("2026-07-04"), &["2026-07-04"]);

      assert_eq!(displayed_date(&state), NaiveDate::from_ymd_opt(2026, 7, 4).unwrap());
    }

    #[test]
    fn it_falls_back_to_today_when_the_selection_is_unparseable() {
      let state = state_with(Some("not-a-date"), &[]);

      assert_eq!(displayed_date(&state), Utc::now().date_naive());
    }

    #[test]
    fn it_falls_back_to_today_when_nothing_is_selected() {
      let state = state_with(None, &["2026-07-04"]);

      assert_eq!(displayed_date(&state), Utc::now().date_naive());
    }
  }

  mod human_date {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_the_weekday_day_and_month() {
      let date = NaiveDate::from_ymd_opt(2026, 7, 5).unwrap();

      assert_eq!(human_date(date), "Sunday, 5 July");
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_today_header() {
      let state = state_with(None, &["2026-07-04"]);

      let _el: Element<'_, Parent> = view(&state);
    }

    #[test]
    fn it_renders_a_selected_past_header() {
      let state = state_with(Some("2026-07-04"), &["2026-07-04"]);

      let _el: Element<'_, Parent> = view(&state);
    }
  }

  mod mode_button {
    use super::*;

    #[test]
    fn it_offers_back_to_today_on_a_past_day() {
      let state = state_with(Some("2026-07-04"), &["2026-07-04"]);

      assert!(mode_button(&state).is_some());
    }

    #[test]
    fn it_offers_nothing_while_composing_today() {
      let state = state_with(None, &["2026-07-04"]);

      assert!(mode_button(&state).is_none());
    }
  }

  mod jump_calendar {
    use super::*;

    #[test]
    fn it_builds_a_calendar_with_logged_days() {
      let state = state_with(None, &["2026-07-04", "2026-07-03"]);

      let _el: Element<'_, Parent> = jump_calendar(&state);
    }

    #[test]
    fn it_builds_a_calendar_with_only_today_enabled() {
      let state = state_with(None, &[]);

      let _el: Element<'_, Parent> = jump_calendar(&state);
    }
  }

  mod month_cells {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_always_yields_six_weeks() {
      assert_eq!(month_cells(2026, 6).len(), 42);
    }

    #[test]
    fn it_marks_out_of_month_leaders_and_trailers() {
      let cells = month_cells(2026, 6);

      assert!(!cells[0].in_month);
      assert!(cells.iter().filter(|cell| cell.in_month).count() == 31);
    }

    #[test]
    fn it_formats_cell_isos_with_zero_padding() {
      let cell = DayCell {
        day: 4,
        in_month: true,
        month0: 6,
        year: 2026,
      };

      assert_eq!(cell.iso(), "2026-07-04");
    }
  }
}
