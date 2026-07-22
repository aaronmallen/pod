use iced::{
  Background, Border, Element, Length, Padding, Task,
  alignment::Vertical,
  keyboard::{self, key::Named},
  widget::{Column, Row, Space, container, text, text_editor},
};

use super::{Message as Parent, objective_link};
use crate::{
  store::{
    Database,
    model::{FieldNote, LinkSource},
    repo::field_notes,
  },
  ui::{
    components::{
      button::{Button, Size},
      eyebrow::eyebrow,
      icon::Icon,
      rule,
    },
    style::{color, control, radius, spacing, typography},
  },
};

const EDITOR_HEIGHT: f32 = 72.0;
const EDITOR_PADDING: f32 = 11.0;
const STAMP_WIDTH: f32 = 46.0;

#[derive(Clone, Debug)]
pub enum Message {
  Added(Result<FieldNote, String>),
  ComposeEdited(text_editor::Action),
  ComposeSubmitted,
  ComposeToggled,
  DeleteRequested(i64),
  Deleted(Result<(), String>),
  EditCancelled,
  EditEdited(text_editor::Action),
  EditRequested(i64),
  EditSubmitted(i64),
  Saved(i64, Result<String, String>),
}

#[derive(Debug)]
pub struct State {
  compose: text_editor::Content,
  composing: bool,
  date: String,
  editing: Option<Editing>,
  notes: Vec<FieldNote>,
}

#[derive(Debug)]
struct Editing {
  draft: text_editor::Content,
  id: i64,
}

impl State {
  pub fn new(date: String, notes: Vec<FieldNote>) -> Self {
    State {
      compose: text_editor::Content::new(),
      composing: false,
      date,
      editing: None,
      notes,
    }
  }
}

pub(super) fn update_pane(state: &mut State, db: &Database, message: Message) -> Task<Parent> {
  match message {
    Message::Added(result) => apply_added(state, result),
    Message::ComposeEdited(action) => state.compose.perform(action),
    Message::ComposeSubmitted => return submit_compose(state, db),
    Message::ComposeToggled => toggle_compose(state),
    Message::DeleteRequested(id) => return delete_note(state, id, db),
    Message::Deleted(result) => log_error(result, "field note delete failed"),
    Message::EditCancelled => state.editing = None,
    Message::EditEdited(action) => {
      if let Some(edit) = state.editing.as_mut() {
        edit.draft.perform(action);
      }
    }
    Message::EditRequested(id) => begin_edit(state, id),
    Message::EditSubmitted(id) => return submit_edit(state, id, db),
    Message::Saved(id, result) => apply_saved(state, id, result),
  }

  Task::none()
}

pub(super) fn view_pane<'a>(state: &'a State, links: &'a objective_link::State) -> Element<'a, Parent> {
  let mut children: Vec<Element<'a, Parent>> = vec![header(state)];

  if state.composing {
    children.push(rule::horizontal());
    children.push(compose_box(state));
  }

  for note in &state.notes {
    children.push(rule::horizontal());
    children.push(note_row(state, note, links));
  }

  if !state.composing && state.notes.is_empty() {
    children.push(empty_body());
  }

  container(Column::with_children(children).width(Length::Fill))
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

fn apply_added(state: &mut State, result: Result<FieldNote, String>) {
  match result {
    Ok(note) => {
      state.notes.insert(0, note);
      state.composing = false;
      state.compose = text_editor::Content::new();
    }
    Err(error) => tracing::warn!(target: "pod::captains_log", %error, "field note add failed"),
  }
}

fn apply_saved(state: &mut State, id: i64, result: Result<String, String>) {
  match result {
    Ok(value) => {
      if let Some(note) = state.notes.iter_mut().find(|note| note.id == id) {
        note.text = value;
      }
      state.editing = None;
    }
    Err(error) => tracing::warn!(target: "pod::captains_log", %error, "field note edit failed"),
  }
}

fn begin_edit(state: &mut State, id: i64) {
  let current = state
    .notes
    .iter()
    .find(|note| note.id == id)
    .map(|note| note.text.clone())
    .unwrap_or_default();
  state.editing = Some(Editing {
    draft: text_editor::Content::with_text(&current),
    id,
  });
}

fn delete_note(state: &mut State, id: i64, db: &Database) -> Task<Parent> {
  state.notes.retain(|note| note.id != id);
  if state.editing.as_ref().is_some_and(|edit| edit.id == id) {
    state.editing = None;
  }
  let db = db.clone();
  Task::perform(
    async move {
      field_notes::delete(&db, id)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
    },
    Message::Deleted,
  )
  .map(Parent::FieldNotes)
}

fn log_error(result: Result<(), String>, context: &'static str) {
  if let Err(error) = result {
    tracing::warn!(target: "pod::captains_log", %error, "{context}");
  }
}

fn non_blank(value: &str) -> Option<String> {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    None
  } else {
    Some(trimmed.to_owned())
  }
}

fn submit_compose(state: &mut State, db: &Database) -> Task<Parent> {
  let Some(text) = non_blank(&state.compose.text()) else {
    return Task::none();
  };
  let db = db.clone();
  let date = state.date.clone();
  Task::perform(
    async move {
      field_notes::insert(&db, &date, &text)
        .await
        .map_err(|error| error.to_string())
    },
    Message::Added,
  )
  .map(Parent::FieldNotes)
}

fn submit_edit(state: &mut State, id: i64, db: &Database) -> Task<Parent> {
  let Some(edit) = state.editing.as_ref().filter(|edit| edit.id == id) else {
    return Task::none();
  };
  let Some(text) = non_blank(&edit.draft.text()) else {
    return Task::none();
  };
  let db = db.clone();
  let saved = text.clone();
  Task::perform(
    async move {
      field_notes::update(&db, id, &text)
        .await
        .map(|_| saved)
        .map_err(|error| error.to_string())
    },
    move |result| Message::Saved(id, result),
  )
  .map(Parent::FieldNotes)
}

fn toggle_compose(state: &mut State) {
  state.composing = !state.composing;
  state.compose = text_editor::Content::new();
}

fn count_label(count: usize) -> String {
  if count == 1 {
    t!("captains_log.field_notes.count_one", count => count)
  } else {
    t!("captains_log.field_notes.count_other", count => count)
  }
  .into_owned()
}

fn header(state: &State) -> Element<'_, Parent> {
  let (label, icon) = if state.composing {
    (t!("captains_log.field_notes.close"), Icon::close())
  } else {
    (t!("captains_log.field_notes.add_note"), Icon::plus())
  };

  Row::with_children(vec![
    Icon::journal().size(16.0).color(color::text::secondary()).render(),
    text(t!("captains_log.field_notes.title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    eyebrow(&count_label(state.notes.len()), None),
    Space::new().width(Length::Fill).into(),
    Button::secondary(label.into_owned())
      .size(Size::Sm)
      .icon(icon)
      .on_press(Parent::FieldNotes(Message::ComposeToggled))
      .into(),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_2_5)
  .padding(row_padding())
  .into()
}

fn compose_box(state: &State) -> Element<'_, Parent> {
  let editor = text_editor(&state.compose)
    .placeholder(t!("captains_log.field_notes.compose_placeholder").into_owned())
    .on_action(|action| Parent::FieldNotes(Message::ComposeEdited(action)))
    .key_binding(|press| submit_binding(press, Message::ComposeSubmitted))
    .padding(EDITOR_PADDING)
    .size(typography::size::MD)
    .height(Length::Fixed(EDITOR_HEIGHT))
    .style(editor_style);

  let can_save = !state.compose.text().trim().is_empty();
  let hint = text(t!("captains_log.field_notes.compose_hint").into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));

  let actions = Row::with_children(vec![
    hint.into(),
    Space::new().width(Length::Fill).into(),
    Button::secondary(t!("captains_log.field_notes.cancel").into_owned())
      .size(Size::Sm)
      .on_press(Parent::FieldNotes(Message::ComposeToggled))
      .into(),
    Button::primary(t!("captains_log.field_notes.add_note").into_owned())
      .size(Size::Sm)
      .icon(Icon::plus())
      .on_press_maybe(can_save.then_some(Parent::FieldNotes(Message::ComposeSubmitted)))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  container(
    Column::with_children(vec![editor.into(), actions.into()])
      .spacing(spacing::SPACE_2_5)
      .width(Length::Fill),
  )
  .padding(row_padding())
  .into()
}

fn note_row<'a>(state: &'a State, note: &'a FieldNote, links: &'a objective_link::State) -> Element<'a, Parent> {
  let stamp = text(note_time(&note.created_at))
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::accent::PLASMA))
    .width(Length::Fixed(STAMP_WIDTH));

  let editing = state.editing.as_ref().filter(|edit| edit.id == note.id);
  let body = match editing {
    Some(edit) => note_editor(edit),
    None => note_display(note),
  };
  let content: Element<'a, Parent> = match editing {
    Some(_) => body,
    None => Column::with_children(vec![
      body,
      objective_link::picker(
        links,
        &state.date,
        LinkSource::FieldNote {
          note_id: note.id,
        },
        true,
      ),
    ])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into(),
  };

  container(
    Row::with_children(vec![stamp.into(), content])
      .align_y(Vertical::Top)
      .spacing(spacing::SPACE_2),
  )
  .padding(row_padding())
  .into()
}

fn note_display(note: &FieldNote) -> Element<'_, Parent> {
  Row::with_children(vec![
    text(note.text.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .width(Length::Fill)
      .into(),
    Button::ghost_icon(Icon::pencil())
      .on_press(Parent::FieldNotes(Message::EditRequested(note.id)))
      .into(),
    Button::ghost_icon(Icon::trash())
      .on_press(Parent::FieldNotes(Message::DeleteRequested(note.id)))
      .into(),
  ])
  .align_y(Vertical::Top)
  .spacing(spacing::UNIT)
  .into()
}

fn note_editor(edit: &Editing) -> Element<'_, Parent> {
  let id = edit.id;
  let editor = text_editor(&edit.draft)
    .on_action(|action| Parent::FieldNotes(Message::EditEdited(action)))
    .key_binding(move |press| submit_binding(press, Message::EditSubmitted(id)))
    .padding(EDITOR_PADDING)
    .size(typography::size::MD)
    .height(Length::Fixed(EDITOR_HEIGHT))
    .style(editor_style);

  let can_save = !edit.draft.text().trim().is_empty();
  let actions = Row::with_children(vec![
    Space::new().width(Length::Fill).into(),
    Button::secondary(t!("captains_log.field_notes.cancel").into_owned())
      .size(Size::Sm)
      .on_press(Parent::FieldNotes(Message::EditCancelled))
      .into(),
    Button::primary(t!("captains_log.field_notes.save").into_owned())
      .size(Size::Sm)
      .icon(Icon::check())
      .on_press_maybe(can_save.then_some(Parent::FieldNotes(Message::EditSubmitted(id))))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  Column::with_children(vec![editor.into(), actions.into()])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .into()
}

fn empty_body<'a>() -> Element<'a, Parent> {
  container(
    text(t!("captains_log.field_notes.empty").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::tertiary())),
  )
  .padding(spacing::SPACE_4_5)
  .into()
}

fn submit_binding(press: text_editor::KeyPress, message: Message) -> Option<text_editor::Binding<Parent>> {
  if press.modifiers.command() && matches!(press.key, keyboard::Key::Named(Named::Enter)) {
    Some(text_editor::Binding::Custom(Parent::FieldNotes(message)))
  } else {
    text_editor::Binding::from_key_press(press)
  }
}

fn note_time(created_at: &str) -> String {
  chrono::DateTime::parse_from_rfc3339(created_at)
    .map(|moment| moment.with_timezone(&chrono::Utc).format("%H:%M").to_string())
    .unwrap_or_else(|_| created_at.chars().skip(11).take(5).collect())
}

fn row_padding() -> Padding {
  Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_4_5,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_4_5,
  }
}

fn editor_style(_theme: &iced::Theme, _status: text_editor::Status) -> text_editor::Style {
  text_editor::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    placeholder: color::text::tertiary(),
    selection: color::accent_muted(),
    value: color::text::PRIMARY,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  fn note(id: i64, text: &str) -> FieldNote {
    FieldNote {
      created_at: "2026-07-05T20:30:00+00:00".to_owned(),
      date: "2026-07-05".to_owned(),
      id,
      text: text.to_owned(),
      updated_at: "2026-07-05T20:30:00+00:00".to_owned(),
    }
  }

  fn state_with(notes: Vec<FieldNote>) -> State {
    State::new("2026-07-05".to_owned(), notes)
  }

  mod note_time {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_renders_the_utc_hour_and_minute() {
      assert_eq!(super::note_time("2026-07-05T20:30:00+00:00"), "20:30");
    }

    #[test]
    fn it_normalizes_an_offset_stamp_to_utc() {
      assert_eq!(super::note_time("2026-07-05T20:30:00+02:00"), "18:30");
    }

    #[test]
    fn it_falls_back_to_the_raw_slice_when_unparseable() {
      assert_eq!(super::note_time("2026-07-05T18:00"), "18:00");
    }
  }

  mod count_label {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_selects_the_singular_form_for_one_note() {
      assert_eq!(super::count_label(1), "1 note");
    }

    #[test]
    fn it_selects_the_plural_form_otherwise() {
      assert_eq!(super::count_label(0), "0 notes");
      assert_eq!(super::count_label(3), "3 notes");
    }
  }

  mod toggle_compose {
    use super::*;

    #[test]
    fn it_opens_and_closes_the_compose_box() {
      let mut state = state_with(Vec::new());

      toggle_compose(&mut state);
      assert!(state.composing);

      toggle_compose(&mut state);
      assert!(!state.composing);
    }
  }

  mod begin_edit {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_seeds_the_draft_from_the_note_text() {
      let mut state = state_with(vec![note(7, "Cyno up")]);

      begin_edit(&mut state, 7);

      let edit = state.editing.expect("an edit should be armed");
      assert_eq!(edit.id, 7);
      assert_eq!(edit.draft.text().trim(), "Cyno up");
    }
  }

  mod apply_added {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_prepends_the_new_note_and_closes_the_compose_box() {
      let mut state = state_with(vec![note(1, "first")]);
      state.composing = true;

      apply_added(&mut state, Ok(note(2, "second")));

      assert_eq!(state.notes.iter().map(|note| note.id).collect::<Vec<_>>(), [2, 1]);
      assert!(!state.composing);
    }
  }

  mod apply_saved {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_updates_the_note_text_in_place_and_leaves_edit_mode() {
      let mut state = state_with(vec![note(7, "draft")]);
      begin_edit(&mut state, 7);

      apply_saved(&mut state, 7, Ok("final".to_owned()));

      assert_eq!(state.notes[0].text, "final");
      assert!(state.editing.is_none());
    }
  }

  mod delete_note {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_removes_the_note_immediately() {
      let db = store::open_test().await.unwrap();
      let mut state = state_with(vec![note(1, "keep"), note(2, "drop")]);

      let _ = delete_note(&mut state, 2, &db);

      assert_eq!(state.notes.iter().map(|note| note.id).collect::<Vec<_>>(), [1]);
    }
  }

  mod view_pane {
    use super::*;

    #[test]
    fn it_renders_the_empty_state() {
      let state = state_with(Vec::new());

      let links = objective_link::State::default();
      let _el: Element<'_, Parent> = view_pane(&state, &links);
    }

    #[test]
    fn it_renders_notes_and_the_compose_box() {
      let mut state = state_with(vec![note(1, "first"), note(2, "second")]);
      state.composing = true;
      begin_edit(&mut state, 1);

      let links = objective_link::State::default();
      let _el: Element<'_, Parent> = view_pane(&state, &links);
    }
  }

  mod update_pane {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    fn edit_insert(ch: char) -> text_editor::Action {
      text_editor::Action::Edit(text_editor::Edit::Insert(ch))
    }

    #[tokio::test]
    async fn it_dispatches_each_message_variant_through_its_handler() {
      let db = store::open_test().await.unwrap();
      let mut state = state_with(vec![note(1, "first"), note(2, "second")]);

      let _ = update_pane(&mut state, &db, Message::ComposeToggled);
      assert!(state.composing);

      let _ = update_pane(&mut state, &db, Message::ComposeEdited(edit_insert('x')));
      let _ = update_pane(&mut state, &db, Message::ComposeSubmitted);

      let _ = update_pane(&mut state, &db, Message::Added(Ok(note(3, "third"))));
      assert_eq!(state.notes.first().map(|note| note.id), Some(3));

      let _ = update_pane(&mut state, &db, Message::EditRequested(1));
      assert!(state.editing.is_some());

      let _ = update_pane(&mut state, &db, Message::EditEdited(edit_insert('y')));
      let _ = update_pane(&mut state, &db, Message::EditSubmitted(1));
      let _ = update_pane(&mut state, &db, Message::Saved(1, Ok("edited".to_owned())));
      assert_eq!(
        state
          .notes
          .iter()
          .find(|note| note.id == 1)
          .map(|note| note.text.as_str()),
        Some("edited")
      );

      let _ = update_pane(&mut state, &db, Message::EditRequested(3));
      let _ = update_pane(&mut state, &db, Message::EditCancelled);
      assert!(state.editing.is_none());

      let _ = update_pane(&mut state, &db, Message::DeleteRequested(2));
      assert!(state.notes.iter().all(|note| note.id != 2));

      let _ = update_pane(&mut state, &db, Message::Deleted(Ok(())));
    }
  }
}
