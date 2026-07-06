use std::{
  collections::HashMap,
  sync::{OnceLock, RwLock},
};

use iced::{
  Background, Border, Element, Length, Padding, Task,
  alignment::Vertical,
  widget::{Column, Row, Space, container, text},
};

use super::Message as Parent;
use crate::{
  store::{Database, repo::captains_log},
  ui::{
    components::{
      button::{Button, Size},
      eyebrow::eyebrow_text,
      icon::Icon,
      text_input::TextInput,
    },
    style::{color, radius, spacing, typography},
  },
};

const ACTIVE_BORDER_ALPHA: f32 = 0.4;
const ACTIVE_FILL_ALPHA: f32 = 0.1;
const BAR_WIDTH: f32 = 3.0;
const BODY_PADDING_X: f32 = 24.0;
const BODY_PADDING_Y: f32 = 20.0;
const EDIT_TEXT_SIZE: f32 = 17.0;
const HERO_TEXT_SIZE: f32 = 22.0;
const KICKER_ICON_SIZE: f32 = 15.0;

#[derive(Clone, Debug)]
pub enum Message {
  Cancelled,
  DraftChanged(String),
  EditRequested,
  SaveRequested,
  Saved(Result<Option<String>, String>),
  WriteRequested,
}

#[derive(Debug, Default)]
pub struct State {
  draft: String,
  editing: bool,
  text: Option<String>,
}

impl State {
  pub fn new(narrative: Option<String>) -> Self {
    State {
      draft: String::new(),
      editing: false,
      text: narrative.filter(|value| !value.trim().is_empty()),
    }
  }

  fn has_text(&self) -> bool {
    self.text.as_deref().is_some_and(|value| !value.trim().is_empty())
  }
}

pub(super) fn update_pane(state: &mut State, date: &str, db: &Database, message: Message) -> Task<Parent> {
  match message {
    Message::Cancelled => cancel(state),
    Message::DraftChanged(value) => state.draft = value,
    Message::EditRequested => begin_edit(state, state.text.clone().unwrap_or_default()),
    Message::SaveRequested => return save_requested(state, date, db),
    Message::Saved(result) => apply_saved(state, result),
    Message::WriteRequested => begin_edit(state, String::new()),
  }

  Task::none()
}

pub(super) fn view_pane(state: &State) -> Element<'_, Parent> {
  if state.editing {
    edit_hero(state)
  } else if state.has_text() {
    display_hero(state)
  } else {
    placeholder_hero()
  }
}

fn apply_saved(state: &mut State, result: Result<Option<String>, String>) {
  match result {
    Ok(value) => {
      state.text = value.filter(|value| !value.trim().is_empty());
      state.editing = false;
      state.draft.clear();
    }
    Err(error) => tracing::warn!(target: "pod::captains_log", %error, "narrative save failed"),
  }
}

fn begin_edit(state: &mut State, draft: String) {
  state.draft = draft;
  state.editing = true;
}

fn cancel(state: &mut State) {
  state.editing = false;
  state.draft.clear();
}

fn draft_value(draft: &str) -> Option<String> {
  let trimmed = draft.trim();
  if trimmed.is_empty() {
    None
  } else {
    Some(trimmed.to_owned())
  }
}

fn save_requested(state: &mut State, date: &str, db: &Database) -> Task<Parent> {
  let value = draft_value(&state.draft);
  let db = db.clone();
  let date = date.to_owned();

  Task::perform(
    async move {
      captains_log::upsert_narrative(&db, &date, value.as_deref())
        .await
        .map(|()| value)
        .map_err(|error| error.to_string())
    },
    Message::Saved,
  )
  .map(Parent::Narrative)
}

fn display_hero(state: &State) -> Element<'_, Parent> {
  let quote = format!("“{}”", state.text.as_deref().unwrap_or_default());
  let body = text(quote)
    .font(typography::body::REGULAR)
    .size(HERO_TEXT_SIZE)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  let edit = Button::ghost_icon(Icon::pencil()).on_press(Parent::Narrative(Message::EditRequested));

  let column = Column::with_children(vec![kicker_row(true), body.into()])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill);

  let row = Row::with_children(vec![column.into(), edit.into()]).spacing(spacing::SPACE_3);

  hero_shell(true, row.into())
}

fn edit_hero(state: &State) -> Element<'_, Parent> {
  let input = TextInput::new(
    tr_static("captains_log.narrative.input_placeholder"),
    &state.draft,
    |value| Parent::Narrative(Message::DraftChanged(value)),
  )
  .background(color::surface::SUNKEN)
  .font_size(EDIT_TEXT_SIZE)
  .on_submit(Parent::Narrative(Message::SaveRequested))
  .render();

  let cancel = Button::secondary(t!("captains_log.narrative.cancel").into_owned())
    .size(Size::Sm)
    .on_press(Parent::Narrative(Message::Cancelled));
  let save = Button::primary(t!("captains_log.narrative.save").into_owned())
    .size(Size::Sm)
    .icon(Icon::check())
    .on_press(Parent::Narrative(Message::SaveRequested));

  let actions = Row::with_children(vec![
    Space::new().width(Length::Fill).into(),
    cancel.into(),
    save.into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let column = Column::with_children(vec![kicker_row(true), input, actions.into()])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill);

  hero_shell(true, column.into())
}

fn placeholder_hero<'a>() -> Element<'a, Parent> {
  let cta = Button::ghost(t!("captains_log.narrative.placeholder").into_owned())
    .icon_right(Icon::chevron_right())
    .on_press(Parent::Narrative(Message::WriteRequested));

  let column = Column::with_children(vec![kicker_row(false), cta.into()])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill);

  hero_shell(false, column.into())
}

fn kicker_row<'a>(active: bool) -> Element<'a, Parent> {
  let tint = if active {
    color::accent()
  } else {
    color::text::secondary()
  };

  Row::with_children(vec![
    Icon::journal().size(KICKER_ICON_SIZE).color(tint).render::<Parent>(),
    eyebrow_text(&t!("captains_log.narrative.kicker"), Some(tint)).into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn hero_shell<'a>(active: bool, body: Element<'a, Parent>) -> Element<'a, Parent> {
  let (background, border) = if active {
    (
      color::with_alpha(color::accent(), ACTIVE_FILL_ALPHA),
      color::with_alpha(color::accent(), ACTIVE_BORDER_ALPHA),
    )
  } else {
    (color::surface::RAISED, color::rule())
  };
  let bar_color = if active { color::accent() } else { color::rule() };

  let accent_bar = container(Space::new())
    .width(Length::Fixed(BAR_WIDTH))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(bar_color)),
      ..container::Style::default()
    });

  let content = container(body).width(Length::Fill).padding(Padding {
    top: BODY_PADDING_Y,
    right: BODY_PADDING_X,
    bottom: BODY_PADDING_Y,
    left: BODY_PADDING_X,
  });

  let row = Row::with_children(vec![accent_bar.into(), content.into()]).width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(background)),
      border: Border {
        color: border,
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn tr_static(key: &str) -> &'static str {
  static CACHE: OnceLock<RwLock<HashMap<String, &'static str>>> = OnceLock::new();
  let cache = CACHE.get_or_init(|| RwLock::new(HashMap::new()));
  if let Some(&interned) = cache.read().expect("narrative i18n cache poisoned").get(key) {
    return interned;
  }

  let resolved: &'static str = Box::leak(t!(key).into_owned().into_boxed_str());
  cache
    .write()
    .expect("narrative i18n cache poisoned")
    .entry(key.to_owned())
    .or_insert(resolved)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn authored() -> State {
    State::new(Some("One frigate saved a fleet.".to_owned()))
  }

  mod new {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keeps_a_non_blank_narrative() {
      let state = State::new(Some("Clean roam.".to_owned()));

      assert!(state.has_text());
      assert_eq!(state.text.as_deref(), Some("Clean roam."));
    }

    #[test]
    fn it_drops_a_blank_narrative() {
      let state = State::new(Some("   ".to_owned()));

      assert!(!state.has_text());
      assert_eq!(state.text, None);
    }
  }

  mod update_pane {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    #[tokio::test]
    async fn it_enters_a_blank_draft_on_write() {
      let db = store::open_test().await.unwrap();
      let mut state = State::default();

      let _ = update_pane(&mut state, "2026-07-06", &db, Message::WriteRequested);

      assert!(state.editing);
      assert_eq!(state.draft, "");
    }

    #[tokio::test]
    async fn it_seeds_the_draft_from_existing_text_on_edit() {
      let db = store::open_test().await.unwrap();
      let mut state = authored();

      let _ = update_pane(&mut state, "2026-07-06", &db, Message::EditRequested);

      assert!(state.editing);
      assert_eq!(state.draft, "One frigate saved a fleet.");
    }

    #[tokio::test]
    async fn it_tracks_draft_edits() {
      let db = store::open_test().await.unwrap();
      let mut state = State::default();
      begin_edit(&mut state, String::new());

      let _ = update_pane(
        &mut state,
        "2026-07-06",
        &db,
        Message::DraftChanged("halfway".to_owned()),
      );

      assert_eq!(state.draft, "halfway");
    }

    #[tokio::test]
    async fn it_discards_the_draft_on_cancel() {
      let db = store::open_test().await.unwrap();
      let mut state = authored();
      begin_edit(&mut state, "scratch".to_owned());

      let _ = update_pane(&mut state, "2026-07-06", &db, Message::Cancelled);

      assert!(!state.editing);
      assert_eq!(state.draft, "");
      assert_eq!(state.text.as_deref(), Some("One frigate saved a fleet."));
    }

    #[tokio::test]
    async fn it_installs_a_saved_narrative_and_leaves_edit_mode() {
      let db = store::open_test().await.unwrap();
      let mut state = State::default();
      begin_edit(&mut state, "Sold into strength.".to_owned());

      let _ = update_pane(
        &mut state,
        "2026-07-06",
        &db,
        Message::Saved(Ok(Some("Sold into strength.".to_owned()))),
      );

      assert!(!state.editing);
      assert_eq!(state.text.as_deref(), Some("Sold into strength."));
    }

    #[tokio::test]
    async fn it_clears_the_text_when_a_save_stores_nothing() {
      let db = store::open_test().await.unwrap();
      let mut state = authored();
      begin_edit(&mut state, String::new());

      let _ = update_pane(&mut state, "2026-07-06", &db, Message::Saved(Ok(None)));

      assert!(!state.has_text());
      assert_eq!(state.text, None);
    }
  }

  mod draft_value {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_trims_authored_input() {
      assert_eq!(draft_value("  logged it  "), Some("logged it".to_owned()));
    }

    #[test]
    fn it_maps_a_blank_draft_to_nothing() {
      assert_eq!(draft_value("   "), None);
      assert_eq!(draft_value(""), None);
    }
  }

  mod view_pane {
    use super::*;

    #[test]
    fn it_renders_the_placeholder_when_empty() {
      let state = State::default();

      let _el: Element<'_, Parent> = view_pane(&state);
    }

    #[test]
    fn it_renders_the_display_hero_with_text() {
      let state = authored();

      let _el: Element<'_, Parent> = view_pane(&state);
    }

    #[test]
    fn it_renders_the_edit_hero_while_editing() {
      let mut state = authored();
      begin_edit(&mut state, "in progress".to_owned());

      let _el: Element<'_, Parent> = view_pane(&state);
    }
  }

  mod persistence {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    #[tokio::test]
    async fn it_creates_the_row_on_first_authored_input() {
      let db = store::open_test().await.unwrap();
      assert!(captains_log::get(&db, "2026-07-06").await.unwrap().is_none());

      captains_log::upsert_narrative(&db, "2026-07-06", draft_value("First line.").as_deref())
        .await
        .unwrap();

      assert!(captains_log::get(&db, "2026-07-06").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_round_trips_and_then_clears_a_narrative() {
      let db = store::open_test().await.unwrap();

      captains_log::upsert_narrative(&db, "2026-07-06", draft_value(" Clean roam. ").as_deref())
        .await
        .unwrap();
      let saved = captains_log::get(&db, "2026-07-06").await.unwrap().unwrap();
      assert_eq!(saved.narrative().as_deref(), Some("Clean roam."));

      captains_log::upsert_narrative(&db, "2026-07-06", draft_value("   ").as_deref())
        .await
        .unwrap();
      let cleared = captains_log::get(&db, "2026-07-06").await.unwrap().unwrap();
      assert_eq!(cleared.narrative().as_deref(), None);
    }
  }
}
