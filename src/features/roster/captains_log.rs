mod entries;
mod eve_date;
mod events;
mod header;
pub mod km_report;
mod narrative;
mod past;
pub mod prompts;
pub mod rollup;
mod rollup_tiles;
mod wizard;

use iced::{
  Element, Length, Task,
  widget::{Column, Row, container, scrollable},
};

use crate::{
  store::images::ImageKind,
  ui::{
    components::rule,
    style::{color, control, spacing},
  },
};

const ENTRIES_WIDTH: f32 = 276.0;
const MAIN_MAX_WIDTH: f32 = 760.0;
const SCREEN_PADDING: f32 = 32.0;

#[derive(Clone, Debug)]
pub enum Message {
  Entries(entries::Message),
  #[allow(dead_code)]
  Events(events::Message),
  Exit,
  Header(header::Message),
  Loaded(Box<Snapshot>),
  #[allow(dead_code)]
  Narrative(narrative::Message),
  #[allow(dead_code)]
  Past(past::Message),
  #[allow(dead_code)]
  Wizard(wizard::Message),
}

impl Message {
  pub fn loads_data(&self) -> bool {
    matches!(self, Message::Loaded(_))
  }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
  entries: Vec<EntryDay>,
}

impl Snapshot {
  fn empty() -> Self {
    Snapshot {
      entries: Vec::new(),
    }
  }
}

#[derive(Debug)]
pub struct State {
  entries: Vec<EntryDay>,
  selected: Option<String>,
}

impl State {
  pub fn new() -> Self {
    State {
      entries: Vec::new(),
      selected: None,
    }
  }

  pub fn stale_images(&self) -> Vec<(ImageKind, i64)> {
    Vec::new()
  }
}

#[derive(Clone, Debug)]
struct EntryDay {
  date_iso: String,
}

pub fn load() -> Task<Message> {
  Task::perform(async { Box::new(Snapshot::empty()) }, Message::Loaded)
}

pub fn update(state: &mut State, message: Message) -> Task<Message> {
  match message {
    Message::Entries(msg) => entries::update(state, msg),
    Message::Events(msg) => events::update(state, msg),
    Message::Exit => Task::none(),
    Message::Header(msg) => header::update(state, msg),
    Message::Loaded(snapshot) => {
      state.entries = snapshot.entries;
      Task::none()
    }
    Message::Narrative(msg) => narrative::update(state, msg),
    Message::Past(msg) => past::update(state, msg),
    Message::Wizard(msg) => wizard::update(state, msg),
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  let main = scrollable(
    container(main_body(state))
      .width(Length::Fill)
      .max_width(MAIN_MAX_WIDTH)
      .padding(SCREEN_PADDING),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(control::scrollbar);

  let panes = Row::with_children(vec![
    container(entries::view(state))
      .width(Length::Fixed(ENTRIES_WIDTH))
      .height(Length::Fill)
      .padding(spacing::SPACE_4_5)
      .into(),
    rule::vertical_fill(1.0),
    main.into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  let base = container(
    Column::with_children(vec![header::view(state), rule::horizontal(), panes.into()])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(iced::Background::Color(color::surface::BASE)),
    ..container::Style::default()
  });

  base.into()
}

pub fn escape_dismiss(_state: &State) -> Option<Message> {
  Some(Message::Exit)
}

fn main_body(state: &State) -> Element<'_, Message> {
  if state.selected.is_some() {
    past::view(state)
  } else {
    today_body(state)
  }
}

fn today_body(state: &State) -> Element<'_, Message> {
  Column::with_children(vec![
    narrative::view(state),
    rollup_tiles::view(state),
    wizard::view(state),
    events::view(state),
  ])
  .spacing(spacing::SPACE_6)
  .width(Length::Fill)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn entry(date_iso: &str) -> EntryDay {
    EntryDay {
      date_iso: date_iso.to_owned(),
    }
  }

  fn loaded_state() -> State {
    let mut state = State::new();
    state.entries = vec![entry("2026-07-05"), entry("2026-07-04")];
    state
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_installs_a_loaded_snapshot() {
      let mut state = State::new();
      let snapshot = Snapshot {
        entries: vec![entry("2026-07-05")],
      };

      let _ = update(&mut state, Message::Loaded(Box::new(snapshot)));

      assert_eq!(state.entries.len(), 1);
    }

    #[test]
    fn it_selects_a_past_day_and_returns_to_today() {
      let mut state = loaded_state();

      let _ = update(
        &mut state,
        Message::Entries(entries::Message::Selected(Some("2026-07-05".to_owned()))),
      );
      assert_eq!(state.selected.as_deref(), Some("2026-07-05"));

      let _ = update(&mut state, Message::Entries(entries::Message::Selected(None)));
      assert_eq!(state.selected, None);
    }

    #[test]
    fn it_ignores_exit_at_the_feature_level() {
      let mut state = State::new();

      let _ = update(&mut state, Message::Exit);

      assert!(state.entries.is_empty());
    }
  }

  mod escape {
    use super::*;

    #[test]
    fn it_dismisses_the_route_back_to_the_roster() {
      let state = State::new();

      assert!(matches!(escape_dismiss(&state), Some(Message::Exit)));
    }

    #[test]
    fn it_reports_only_the_snapshot_as_a_data_load() {
      assert!(Message::Loaded(Box::new(Snapshot::empty())).loads_data());
      assert!(!Message::Exit.loads_data());
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_the_today_skeleton() {
      let state = loaded_state();

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_a_selected_past_day() {
      let mut state = loaded_state();
      state.selected = Some("2026-07-05".to_owned());

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_collects_no_stale_images_yet() {
      let state = loaded_state();

      assert!(state.stale_images().is_empty());
    }
  }
}
