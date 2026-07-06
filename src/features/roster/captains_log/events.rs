use std::{
  collections::HashMap,
  sync::{OnceLock, RwLock},
};

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container, text},
};

use super::Message as Parent;
use crate::{
  store::repo::captains_log_rollup::CalendarEntry,
  ui::{
    components::{button::Button, eyebrow::eyebrow, icon::Icon, rule, text_input::TextInput},
    style::{color, control, radius, spacing, typography},
  },
};

const CHIP_RADIUS: f32 = 7.0;
const CHIP_SIZE: f32 = 28.0;
const NOTE_INDENT: f32 = 54.0;
const TIME_WIDTH: f32 = 42.0;

#[derive(Clone, Debug)]
pub enum Message {
  DraftChanged(String),
  EditCancelled,
  EditRequested(i64),
  NoteChanged(i64, String),
  NoteSaved,
}

#[derive(Clone, Debug)]
pub struct Editing {
  pub draft: String,
  pub event_id: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Response {
  Accepted,
  Declined,
  Pending,
  Tentative,
}

impl Response {
  fn classify(raw: &str) -> Self {
    match raw {
      "accepted" => Response::Accepted,
      "declined" => Response::Declined,
      "tentative" => Response::Tentative,
      _ => Response::Pending,
    }
  }

  fn accent(self) -> Color {
    match self {
      Response::Accepted => color::status::ONLINE,
      Response::Declined => color::status::DANGER,
      Response::Pending => color::text::tertiary(),
      Response::Tentative => color::status::WARNING,
    }
  }

  fn glyph(self) -> Icon {
    match self {
      Response::Accepted => Icon::check(),
      Response::Declined => Icon::close(),
      Response::Pending => Icon::clock(),
      Response::Tentative => Icon::help(),
    }
  }

  fn label(self) -> String {
    match self {
      Response::Accepted => t!("captains_log.events.response_accepted"),
      Response::Declined => t!("captains_log.events.response_declined"),
      Response::Pending => t!("captains_log.events.response_pending"),
      Response::Tentative => t!("captains_log.events.response_tentative"),
    }
    .into_owned()
  }
}

pub fn begin_edit(notes: &HashMap<i64, String>, event_id: i64) -> Editing {
  Editing {
    draft: notes.get(&event_id).cloned().unwrap_or_default(),
    event_id,
  }
}

pub fn section<'a>(
  events: &'a [CalendarEntry],
  notes: &'a HashMap<i64, String>,
  editing: Option<&'a Editing>,
) -> Element<'a, Parent> {
  if events.is_empty() {
    return section_shell(0, empty_body());
  }

  let mut rows: Vec<Element<'a, Parent>> = Vec::new();
  for (index, event) in events.iter().enumerate() {
    if index > 0 {
      rows.push(rule::horizontal());
    }
    let note = notes.get(&event.event_id).map(String::as_str);
    rows.push(event_row(event, note, editing));
  }

  section_shell(events.len(), Column::with_children(rows).width(Length::Fill).into())
}

fn add_note_button<'a>(event_id: i64) -> Element<'a, Parent> {
  Button::secondary(t!("captains_log.events.add_note").into_owned())
    .icon(Icon::plus())
    .on_press(Parent::Events(Message::EditRequested(event_id)))
    .into()
}

fn count_label(count: usize) -> String {
  if count == 1 {
    t!("captains_log.events.count_one", count => count)
  } else {
    t!("captains_log.events.count_other", count => count)
  }
  .into_owned()
}

fn empty_body<'a>() -> Element<'a, Parent> {
  container(
    text(t!("captains_log.events.empty").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::tertiary())),
  )
  .padding(spacing::SPACE_4_5)
  .into()
}

fn event_headline<'a>(event: &'a CalendarEntry, response: Response) -> Element<'a, Parent> {
  Column::with_children(vec![
    text(event.title.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    eyebrow(&response.label(), None),
  ])
  .spacing(2.0)
  .width(Length::Fill)
  .into()
}

fn event_row<'a>(event: &'a CalendarEntry, note: Option<&'a str>, editing: Option<&'a Editing>) -> Element<'a, Parent> {
  let response = Response::classify(&event.response);
  let is_editing = editing.is_some_and(|edit| edit.event_id == event.event_id);

  let mut top: Vec<Element<'a, Parent>> = vec![
    text(event_time(&event.timestamp))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .width(Length::Fixed(TIME_WIDTH))
      .into(),
    response_chip(response),
    event_headline(event, response),
  ];
  if !is_editing && note.is_none() {
    top.push(add_note_button(event.event_id));
  }

  let mut children: Vec<Element<'a, Parent>> = vec![
    Row::with_children(top)
      .align_y(Vertical::Center)
      .spacing(spacing::SPACE_3)
      .into(),
  ];
  if let Some(edit) = editing.filter(|edit| edit.event_id == event.event_id) {
    children.push(note_editor(edit));
  } else if let Some(existing) = note {
    children.push(note_display(event.event_id, existing));
  }

  container(Column::with_children(children).spacing(spacing::SPACE_2))
    .padding(row_padding())
    .into()
}

fn event_time(timestamp: &str) -> String {
  chrono::DateTime::parse_from_rfc3339(timestamp)
    .map(|moment| moment.format("%H:%M").to_string())
    .unwrap_or_else(|_| timestamp.chars().skip(11).take(5).collect())
}

fn note_display<'a>(event_id: i64, note: &str) -> Element<'a, Parent> {
  let body = Row::with_children(vec![
    Icon::journal().size(13.0).color(color::accent::PLASMA).render(),
    text(note.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .width(Length::Fill)
      .into(),
    Button::ghost_icon(Icon::pencil())
      .on_press(Parent::Events(Message::EditRequested(event_id)))
      .into(),
  ])
  .align_y(Vertical::Top)
  .spacing(spacing::SPACE_2);

  let box_ = container(body).padding(note_padding()).style(control::sunken_pane);

  container(box_).padding(indent_padding()).into()
}

fn note_editor<'a>(editing: &'a Editing) -> Element<'a, Parent> {
  let event_id = editing.event_id;
  let trimmed = editing.draft.trim();
  let save = (!trimmed.is_empty()).then(|| Parent::Events(Message::NoteChanged(event_id, trimmed.to_owned())));

  let input = TextInput::new(
    tr_static("captains_log.events.note_placeholder"),
    &editing.draft,
    move |value| Parent::Events(Message::DraftChanged(value)),
  )
  .background(color::surface::SUNKEN)
  .render();

  let actions = Row::with_children(vec![
    Space::new().width(Length::Fill).into(),
    Button::secondary(t!("captains_log.events.cancel").into_owned())
      .on_press(Parent::Events(Message::EditCancelled))
      .into(),
    Button::primary(t!("captains_log.events.save").into_owned())
      .icon(Icon::check())
      .on_press_maybe(save)
      .into(),
  ])
  .spacing(spacing::SPACE_2);

  container(Column::with_children(vec![input, actions.into()]).spacing(spacing::SPACE_2))
    .padding(indent_padding())
    .into()
}

fn indent_padding() -> Padding {
  Padding {
    top: 0.0,
    right: 0.0,
    bottom: 0.0,
    left: NOTE_INDENT,
  }
}

fn note_padding() -> Padding {
  Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
  }
}

fn response_chip<'a>(response: Response) -> Element<'a, Parent> {
  let accent = response.accent();

  container(response.glyph().size(15.0).color(accent).render())
    .width(Length::Fixed(CHIP_SIZE))
    .height(Length::Fixed(CHIP_SIZE))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(accent, 0.12))),
      border: Border {
        color: color::with_alpha(accent, 0.30),
        radius: CHIP_RADIUS.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn row_padding() -> Padding {
  Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_4_5,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_4_5,
  }
}

fn section_shell<'a>(count: usize, body: Element<'a, Parent>) -> Element<'a, Parent> {
  let header = Row::with_children(vec![
    Icon::calendar().size(16.0).color(color::text::secondary()).render(),
    text(t!("captains_log.events.title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    eyebrow(&count_label(count), None),
    Space::new().width(Length::Fill).into(),
    eyebrow(&t!("captains_log.events.hint").into_owned(), None),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_2_5)
  .padding(row_padding());

  let content = Column::with_children(vec![header.into(), rule::horizontal(), body]).width(Length::Fill);

  container(content)
    .width(Length::Fill)
    .clip(true)
    .style(|theme| container::Style {
      border: Border {
        radius: radius::CARD.into(),
        ..control::card(theme).border
      },
      ..control::card(theme)
    })
    .into()
}

fn tr_static(key: &str) -> &'static str {
  static CACHE: OnceLock<RwLock<HashMap<String, &'static str>>> = OnceLock::new();
  let cache = CACHE.get_or_init(|| RwLock::new(HashMap::new()));
  if let Some(&interned) = cache.read().expect("events i18n cache poisoned").get(key) {
    return interned;
  }

  let resolved: &'static str = Box::leak(t!(key).into_owned().into_boxed_str());
  cache
    .write()
    .expect("events i18n cache poisoned")
    .insert(key.to_owned(), resolved);
  resolved
}

#[cfg(test)]
mod tests {
  use super::*;

  fn event(event_id: i64, response: &str, timestamp: &str, title: &str) -> CalendarEntry {
    CalendarEntry {
      event_id,
      response: response.to_owned(),
      timestamp: timestamp.to_owned(),
      title: title.to_owned(),
    }
  }

  mod response {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_classifies_the_known_esi_states() {
      assert_eq!(Response::classify("accepted"), Response::Accepted);
      assert_eq!(Response::classify("declined"), Response::Declined);
      assert_eq!(Response::classify("tentative"), Response::Tentative);
    }

    #[test]
    fn it_falls_back_to_pending_for_unknown_states() {
      assert_eq!(Response::classify("not_responded"), Response::Pending);
      assert_eq!(Response::classify(""), Response::Pending);
    }
  }

  mod event_time {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_renders_the_hour_and_minute_from_an_rfc3339_stamp() {
      assert_eq!(super::event_time("2026-07-05T20:30:00Z"), "20:30");
    }

    #[test]
    fn it_falls_back_to_the_raw_time_slice_when_unparseable() {
      assert_eq!(super::event_time("2026-07-05T18:00"), "18:00");
    }
  }

  mod count_label {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_selects_the_singular_form_for_one_event() {
      assert_eq!(super::count_label(1), "1 event");
    }

    #[test]
    fn it_selects_the_plural_form_otherwise() {
      assert_eq!(super::count_label(0), "0 events");
      assert_eq!(super::count_label(3), "3 events");
    }
  }

  mod begin_edit {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_seeds_the_draft_from_an_existing_note() {
      let mut notes = HashMap::new();
      notes.insert(7, "Bring the logi".to_owned());

      let editing = begin_edit(&notes, 7);

      assert_eq!(editing.event_id, 7);
      assert_eq!(editing.draft, "Bring the logi");
    }

    #[test]
    fn it_starts_empty_when_no_note_exists() {
      let editing = begin_edit(&HashMap::new(), 7);

      assert!(editing.draft.is_empty());
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_the_empty_state_without_events() {
      let notes = HashMap::new();

      let _el: Element<'_, Parent> = section(&[], &notes, None);
    }

    #[test]
    fn it_renders_a_row_with_a_saved_note() {
      let events = vec![event(1, "accepted", "2026-07-05T20:00:00Z", "Tama roam")];
      let mut notes = HashMap::new();
      notes.insert(1, "Form up on Ash".to_owned());

      let _el: Element<'_, Parent> = section(&events, &notes, None);
    }

    #[test]
    fn it_renders_a_row_in_the_editing_state() {
      let events = vec![event(1, "tentative", "2026-07-05T21:30:00Z", "Corp SRP payout")];
      let notes = HashMap::new();
      let editing = Editing {
        draft: "Ask about the ".to_owned(),
        event_id: 1,
      };

      let _el: Element<'_, Parent> = section(&events, &notes, Some(&editing));
    }
  }
}
