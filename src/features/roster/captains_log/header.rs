use chrono::{NaiveDate, Utc};
use iced::{
  Border, Element, Length, Padding, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, button, container, text},
};

use super::{Message as Parent, State, entries, eve_date};
use crate::ui::{
  components::{
    button::Button,
    icon::Icon,
    picker::{PickerGroup, picker_dropdown, picker_row},
  },
  style::{color, radius, spacing, typography},
};

const BACK_BUTTON_SIZE: f32 = 34.0;
const BACK_ICON_SIZE: f32 = 16.0;
const TITLE_SIZE: f32 = 21.0;

#[derive(Clone, Debug)]
pub enum Message {
  JumpToDay,
}

pub(super) fn update(_state: &mut State, message: Message) -> Task<Parent> {
  match message {
    Message::JumpToDay => Task::none(),
  }
}

pub(super) fn view(state: &State) -> Element<'_, Parent> {
  let date = displayed_date(state);

  let kicker = text(t!("captains_log.header.kicker").to_uppercase())
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

  let jump = Button::secondary(t!("captains_log.jump_to_day"))
    .icon(Icon::calendar())
    .on_press(Parent::Header(Message::JumpToDay));

  let row = Row::with_children(vec![back_button().into(), identity.into(), jump.into()])
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

#[allow(dead_code)] // mounted by the parent once jump-to-day open-state is wired (see parent-wiring note)
pub(super) fn jump_dropdown(state: &State) -> Element<'_, Parent> {
  let mut rows: Vec<Element<'_, Parent>> = vec![picker_row(
    t!("captains_log.today").into_owned(),
    state.selected.is_none(),
    Parent::Entries(entries::Message::Selected(None)),
  )];

  for entry in &state.entries {
    let iso = entry.date_iso.clone();
    let selected = state.selected.as_deref() == Some(iso.as_str());
    let label = parse_iso(&iso).map(human_date).unwrap_or_else(|| iso.clone());
    rows.push(picker_row(
      label,
      selected,
      Parent::Entries(entries::Message::Selected(Some(iso))),
    ));
  }

  picker_dropdown(vec![PickerGroup {
    items: rows,
    title: None,
  }])
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

fn parse_iso(iso: &str) -> Option<NaiveDate> {
  NaiveDate::parse_from_str(iso, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
  use super::{super::EntryDay, *};

  fn state_with(selected: Option<&str>, days: &[&str]) -> State {
    let mut state = State::new();
    state.entries = days
      .iter()
      .map(|day| EntryDay {
        date_iso: (*day).to_owned(),
      })
      .collect();
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

  mod jump_dropdown {
    use super::*;

    #[test]
    fn it_builds_a_picker_scoped_to_today_and_logged_days() {
      let state = state_with(None, &["2026-07-04", "2026-07-03"]);

      let _el: Element<'_, Parent> = jump_dropdown(&state);
    }

    #[test]
    fn it_builds_a_picker_with_only_today_when_no_days_are_logged() {
      let state = state_with(None, &[]);

      let _el: Element<'_, Parent> = jump_dropdown(&state);
    }
  }
}
