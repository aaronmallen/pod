#![allow(dead_code)]

use chrono::{Datelike, NaiveDate};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, container, mouse_area, text},
};

use crate::ui::{
  components::{eyebrow::eyebrow_text, icon::Icon},
  format::month_long,
  style::{color, radius, spacing, typography},
};

const PANEL_WIDTH: f32 = 304.0;
const DAY_CELL_HEIGHT: f32 = 30.0;
const NAV_BUTTON_SIZE: f32 = 24.0;
const NAV_ICON_SIZE: f32 = 16.0;
const STEP_BUTTON_SIZE: f32 = 22.0;
const STEP_VALUE_WIDTH: f32 = 30.0;
const AVAILABLE_ALPHA: f32 = 0.04;
const DISABLED_DAY_ALPHA: f32 = 0.45;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DatePickerState {
  selection: NaiveDate,
  time: Option<(u32, u32)>,
  view_year: i32,
  view_month0: u32,
}

impl DatePickerState {
  pub fn new(selection: NaiveDate, time: Option<(u32, u32)>) -> Self {
    Self {
      selection,
      time,
      view_year: selection.year(),
      view_month0: selection.month0(),
    }
  }

  pub fn selection(&self) -> NaiveDate {
    self.selection
  }

  pub fn time(&self) -> Option<(u32, u32)> {
    self.time
  }

  pub fn view_year(&self) -> i32 {
    self.view_year
  }

  pub fn view_month0(&self) -> u32 {
    self.view_month0
  }

  pub fn select(&mut self, date: NaiveDate) {
    self.selection = date;
  }

  pub fn show_month(&mut self, year: i32, month0: u32) {
    self.view_year = year;
    self.view_month0 = month0;
  }

  pub fn prev_month(&mut self) {
    if self.view_month0 == 0 {
      self.view_month0 = 11;
      self.view_year -= 1;
    } else {
      self.view_month0 -= 1;
    }
  }

  pub fn next_month(&mut self) {
    if self.view_month0 == 11 {
      self.view_month0 = 0;
      self.view_year += 1;
    } else {
      self.view_month0 += 1;
    }
  }

  pub fn set_time(&mut self, hour: u32, minute: u32) {
    self.time = Some((hour, minute));
  }

  pub fn hour_up(&mut self) {
    if let Some((hour, minute)) = self.time {
      self.time = Some(((hour + 1) % 24, minute));
    }
  }

  pub fn hour_down(&mut self) {
    if let Some((hour, minute)) = self.time {
      self.time = Some(((hour + 23) % 24, minute));
    }
  }

  pub fn minute_up(&mut self) {
    if let Some((hour, minute)) = self.time {
      let next = minute + 5;
      if next >= 60 {
        self.time = Some(((hour + 1) % 24, next - 60));
      } else {
        self.time = Some((hour, next));
      }
    }
  }

  pub fn minute_down(&mut self) {
    if let Some((hour, minute)) = self.time {
      if minute < 5 {
        self.time = Some(((hour + 23) % 24, 60 + minute - 5));
      } else {
        self.time = Some((hour, minute - 5));
      }
    }
  }
}

pub struct TimeControls<M> {
  pub label: String,
  pub on_hour_up: M,
  pub on_hour_down: M,
  pub on_minute_up: M,
  pub on_minute_down: M,
}

pub struct DatePicker<'a, M> {
  state: &'a DatePickerState,
  on_select: Box<dyn Fn(NaiveDate) -> M + 'a>,
  on_prev_month: M,
  on_next_month: M,
  enabled: Option<Box<dyn Fn(NaiveDate) -> bool + 'a>>,
  eve_tag: Option<String>,
  highlight_available: bool,
  compact: bool,
  in_month_only: bool,
  time: Option<TimeControls<M>>,
  footer: Option<Element<'a, M>>,
}

impl<'a, M> DatePicker<'a, M>
where
  M: Clone + 'static,
{
  pub fn new(
    state: &'a DatePickerState,
    on_select: impl Fn(NaiveDate) -> M + 'a,
    on_prev_month: M,
    on_next_month: M,
  ) -> Self {
    Self {
      state,
      on_select: Box::new(on_select),
      on_prev_month,
      on_next_month,
      enabled: None,
      eve_tag: None,
      highlight_available: false,
      compact: false,
      in_month_only: false,
      time: None,
      footer: None,
    }
  }

  pub fn enabled(mut self, predicate: impl Fn(NaiveDate) -> bool + 'a) -> Self {
    self.enabled = Some(Box::new(predicate));
    self
  }

  pub fn eve_tag(mut self, label: impl Into<String>) -> Self {
    self.eve_tag = Some(label.into());
    self
  }

  pub fn highlight_available(mut self, highlight: bool) -> Self {
    self.highlight_available = highlight;
    self
  }

  pub fn compact(mut self, compact: bool) -> Self {
    self.compact = compact;
    self
  }

  pub fn in_month_only(mut self, in_month_only: bool) -> Self {
    self.in_month_only = in_month_only;
    self
  }

  pub fn time(mut self, controls: TimeControls<M>) -> Self {
    self.time = Some(controls);
    self
  }

  pub fn footer(mut self, footer: Element<'a, M>) -> Self {
    self.footer = Some(footer);
    self
  }

  pub fn view(self) -> Element<'a, M> {
    let mut children: Vec<Element<'a, M>> = vec![self.header(), weekday_header(self.compact), self.day_grid()];

    if let (Some((hour, minute)), Some(controls)) = (self.state.time, self.time) {
      children.push(time_stepper(hour, minute, controls));
    }

    if let Some(footer) = self.footer {
      children.push(footer);
    }

    container(Column::with_children(children))
      .width(Length::Fixed(PANEL_WIDTH))
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

  fn header(&self) -> Element<'a, M> {
    let title = container(
      text(format!(
        "{} {}",
        month_long(self.state.view_month0 + 1),
        self.state.view_year
      ))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      }),
    )
    .width(Length::Fill)
    .align_x(Horizontal::Center);

    let mut row = Row::with_children(vec![
      nav_button(Icon::chevron_left(), self.on_prev_month.clone()),
      title.into(),
      nav_button(Icon::chevron_right(), self.on_next_month.clone()),
    ])
    .spacing(spacing::UNIT)
    .align_y(Vertical::Center);

    if let Some(label) = &self.eve_tag {
      row = row.push(eve_tag(label));
    }

    let side = if self.compact {
      spacing::UNIT
    } else {
      spacing::SPACE_2_5
    };
    container(row)
      .padding(Padding {
        top: side,
        bottom: spacing::SPACE_2,
        left: side,
        right: side,
      })
      .into()
  }

  fn day_grid(&self) -> Element<'a, M> {
    let cells = month_grid(self.state.view_year, self.state.view_month0);
    let mut grid = Column::new().spacing(2.0);
    for week in cells.chunks(7) {
      let mut row = Row::new().spacing(2.0);
      for cell in week {
        let date = cell.date();
        let enabled =
          (!self.in_month_only || cell.in_month) && self.enabled.as_ref().is_none_or(|predicate| predicate(date));
        let selected = cell.in_month && date == self.state.selection;
        row = row.push(day_cell(
          *cell,
          selected,
          enabled,
          self.highlight_available,
          date,
          &self.on_select,
        ));
      }
      grid = grid.push(row);
    }

    let (bottom, side) = if self.compact {
      (0.0, 0.0)
    } else {
      (spacing::UNIT, spacing::SPACE_2_5)
    };
    container(grid)
      .padding(Padding {
        top: 0.0,
        bottom,
        left: side,
        right: side,
      })
      .into()
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DayCell {
  day: u32,
  in_month: bool,
  month0: u32,
  year: i32,
}

impl DayCell {
  fn date(&self) -> NaiveDate {
    NaiveDate::from_ymd_opt(self.year, self.month0 + 1, self.day).expect("valid day cell")
  }
}

fn month_grid(year: i32, month0: u32) -> Vec<DayCell> {
  let first = NaiveDate::from_ymd_opt(year, month0 + 1, 1).expect("valid month");
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
  first_next.pred_opt().expect("non-min date").day()
}

fn day_cell<'a, M>(
  cell: DayCell,
  selected: bool,
  enabled: bool,
  highlight_available: bool,
  date: NaiveDate,
  on_select: &impl Fn(NaiveDate) -> M,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let day_color = if selected {
    color::surface::BASE
  } else if cell.in_month && enabled {
    color::text::PRIMARY
  } else if cell.in_month || enabled {
    color::text::tertiary()
  } else {
    color::with_alpha(color::text::tertiary(), DISABLED_DAY_ALPHA)
  };

  let background = if selected {
    Some(Background::Color(color::accent()))
  } else if enabled && highlight_available {
    Some(Background::Color(color::with_alpha(
      color::text::PRIMARY,
      AVAILABLE_ALPHA,
    )))
  } else {
    None
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
    background,
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  if !enabled {
    return container(label).width(Length::Fill).into();
  }

  container(mouse_area(label).on_press(on_select(date)))
    .width(Length::Fill)
    .into()
}

fn weekday_header<'a, M>(compact: bool) -> Element<'a, M>
where
  M: 'a,
{
  const WEEKDAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
  let mut row = Row::new().spacing(2.0);
  for weekday in WEEKDAYS {
    row = row.push(
      container(
        text(weekday)
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
  let side = if compact { 0.0 } else { spacing::SPACE_2_5 };
  container(row)
    .padding(Padding {
      top: 0.0,
      bottom: spacing::UNIT,
      left: side,
      right: side,
    })
    .into()
}

fn nav_button<'a, M>(icon: Icon, message: M) -> Element<'a, M>
where
  M: Clone + 'static,
{
  mouse_area(
    container(icon.size(NAV_ICON_SIZE).color(color::text::secondary()).render::<M>())
      .width(Length::Fixed(NAV_BUTTON_SIZE))
      .height(Length::Fixed(NAV_BUTTON_SIZE))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .on_press(message)
  .into()
}

fn eve_tag<'a, M>(label: &str) -> Element<'a, M>
where
  M: 'a,
{
  container(
    text(label.to_owned())
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

fn time_stepper<'a, M>(hour: u32, minute: u32, controls: TimeControls<M>) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let block = Row::with_children(vec![
    eyebrow_text(&controls.label, None).width(Length::Fill).into(),
    stepper(hour, controls.on_hour_up, controls.on_hour_down),
    text(":")
      .font(typography::mono::REGULAR)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    stepper(minute, controls.on_minute_up, controls.on_minute_down),
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

fn stepper<'a, M>(value: u32, up: M, down: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
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
    .width(Length::Fixed(STEP_VALUE_WIDTH))
    .align_x(Horizontal::Center)
    .into(),
    step_button("+", up),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn step_button<'a, M>(glyph: &str, message: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  mouse_area(
    container(
      text(glyph.to_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        }),
    )
    .width(Length::Fixed(STEP_BUTTON_SIZE))
    .height(Length::Fixed(STEP_BUTTON_SIZE))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center),
  )
  .on_press(message)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod state {
    use pretty_assertions::assert_eq;

    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
      NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn it_seeds_the_view_month_from_the_selection() {
      let state = DatePickerState::new(date(2026, 6, 15), None);
      assert_eq!((state.view_year(), state.view_month0()), (2026, 5));
      assert_eq!(state.selection(), date(2026, 6, 15));
      assert_eq!(state.time(), None);
    }

    #[test]
    fn month_nav_wraps_the_year() {
      let mut state = DatePickerState::new(date(2026, 1, 1), None);
      state.prev_month();
      assert_eq!((state.view_year(), state.view_month0()), (2025, 11));
      state.next_month();
      assert_eq!((state.view_year(), state.view_month0()), (2026, 0));
    }

    #[test]
    fn select_moves_the_selection_without_moving_the_view() {
      let mut state = DatePickerState::new(date(2026, 6, 1), None);
      state.select(date(2026, 8, 9));
      assert_eq!(state.selection(), date(2026, 8, 9));
      assert_eq!((state.view_year(), state.view_month0()), (2026, 5));
    }

    #[test]
    fn minute_up_wraps_and_carries_the_hour() {
      let mut state = DatePickerState::new(date(2026, 6, 1), Some((10, 55)));
      state.minute_up();
      assert_eq!(state.time(), Some((11, 0)));
    }

    #[test]
    fn minute_down_borrows_the_hour() {
      let mut state = DatePickerState::new(date(2026, 6, 1), Some((11, 0)));
      state.minute_down();
      assert_eq!(state.time(), Some((10, 55)));
    }

    #[test]
    fn hour_up_wraps_at_midnight() {
      let mut state = DatePickerState::new(date(2026, 6, 1), Some((23, 30)));
      state.hour_up();
      assert_eq!(state.time(), Some((0, 30)));
    }

    #[test]
    fn hour_down_wraps_at_midnight() {
      let mut state = DatePickerState::new(date(2026, 6, 1), Some((0, 30)));
      state.hour_down();
      assert_eq!(state.time(), Some((23, 30)));
    }

    #[test]
    fn time_steppers_are_inert_without_a_time() {
      let mut state = DatePickerState::new(date(2026, 6, 1), None);
      state.minute_up();
      state.hour_down();
      assert_eq!(state.time(), None);
    }

    #[test]
    fn set_time_installs_a_time() {
      let mut state = DatePickerState::new(date(2026, 6, 1), None);
      state.set_time(9, 0);
      assert_eq!(state.time(), Some((9, 0)));
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

    #[test]
    fn it_marks_thirty_one_in_month_days_for_a_long_month() {
      let cells = month_grid(2026, 6);
      assert_eq!(cells.iter().filter(|cell| cell.in_month).count(), 31);
    }

    #[test]
    fn it_resolves_out_of_month_cells_to_real_dates() {
      let cells = month_grid(2026, 5);
      assert_eq!(cells[30].date(), NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
    }
  }

  mod render {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
      NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn it_renders_a_grid_only_picker() {
      let state = DatePickerState::new(date(2026, 6, 15), None);
      let el: Element<'_, ()> = DatePicker::new(&state, |_| (), (), ()).view();
      let mut tree = iced::advanced::widget::Tree::new(&el);
      tree.diff(&el);
      assert!(!tree.children.is_empty());
    }

    #[test]
    fn it_renders_the_full_picker_with_time_footer_and_eve_tag() {
      let state = DatePickerState::new(date(2026, 6, 15), Some((9, 0)));
      let footer: Element<'_, ()> = container(text("footer")).into();
      let el: Element<'_, ()> = DatePicker::new(&state, |_| (), (), ())
        .eve_tag("EVE")
        .time(TimeControls {
          label: "Time".to_owned(),
          on_hour_up: (),
          on_hour_down: (),
          on_minute_up: (),
          on_minute_down: (),
        })
        .footer(footer)
        .view();
      let mut tree = iced::advanced::widget::Tree::new(&el);
      tree.diff(&el);
      assert!(!tree.children.is_empty());
    }

    #[test]
    fn it_omits_the_time_stepper_when_state_has_no_time() {
      let state = DatePickerState::new(date(2026, 6, 15), None);
      let _el: Element<'_, ()> = DatePicker::new(&state, |_| (), (), ())
        .time(TimeControls {
          label: "Time".to_owned(),
          on_hour_up: (),
          on_hour_down: (),
          on_minute_up: (),
          on_minute_down: (),
        })
        .view();
    }

    #[test]
    fn it_gates_interactivity_with_the_enabled_predicate() {
      let state = DatePickerState::new(date(2026, 6, 15), None);
      let _el: Element<'_, ()> = DatePicker::new(&state, |_| (), (), ())
        .enabled(|day| day == NaiveDate::from_ymd_opt(2026, 6, 15).unwrap())
        .highlight_available(true)
        .view();
    }

    #[test]
    fn it_renders_a_compact_in_month_only_picker() {
      let state = DatePickerState::new(date(2026, 6, 15), None);
      let el: Element<'_, ()> = DatePicker::new(&state, |_| (), (), ())
        .compact(true)
        .in_month_only(true)
        .enabled(|_| true)
        .highlight_available(true)
        .view();
      let mut tree = iced::advanced::widget::Tree::new(&el);
      tree.diff(&el);
      assert!(!tree.children.is_empty());
    }
  }
}
