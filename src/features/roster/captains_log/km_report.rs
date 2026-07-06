#![allow(dead_code)]

use std::{
  collections::HashMap,
  sync::{OnceLock, RwLock},
};

use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Color, Element, Length, Padding, Task,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, text, text_editor},
};

use crate::{
  store::{
    Database,
    model::KillmailReport,
    repo::killmail_report::{self, ReportInput},
  },
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

const DIFFERENT_HEIGHT: f32 = 76.0;
const EDITOR_PADDING: f32 = 11.0;
const HAPPENED_HEIGHT: f32 = 98.0;
const OUTCOME_ORDER: [Outcome; 3] = [Outcome::Clean, Outcome::Costly, Outcome::Learning];
const PILL_ALPHA_BORDER: f32 = 0.4;
const PILL_ALPHA_FILL: f32 = 0.12;
const RULE_ALPHA: f32 = 0.18;

#[derive(Clone, Debug)]
pub enum Message {
  Cancelled,
  DifferentEdited(text_editor::Action),
  EditRequested,
  HappenedEdited(text_editor::Action),
  Loaded(Box<Option<KillmailReport>>),
  OutcomeSelected(Outcome),
  SaveRequested,
  Saved,
  TakeawayEdited(String),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Outcome {
  #[default]
  Clean,
  Costly,
  Learning,
}

impl Outcome {
  fn from_db(value: &str) -> Self {
    match value {
      "costly" => Outcome::Costly,
      "learning" => Outcome::Learning,
      _ => Outcome::Clean,
    }
  }

  fn as_str(self) -> &'static str {
    match self {
      Outcome::Clean => "clean",
      Outcome::Costly => "costly",
      Outcome::Learning => "learning",
    }
  }

  fn tint(self) -> Color {
    match self {
      Outcome::Clean => color::status::ONLINE,
      Outcome::Costly => color::status::DANGER,
      Outcome::Learning => color::status::WARNING,
    }
  }

  fn label(self) -> String {
    match self {
      Outcome::Clean => t!("captains_log.km_report.outcome_clean").into_owned(),
      Outcome::Costly => t!("captains_log.km_report.outcome_costly").into_owned(),
      Outcome::Learning => t!("captains_log.km_report.outcome_learning").into_owned(),
    }
  }
}

/// One report row exists per `(character_id, killmail_id)` pair: a killmail witnessed by several of the
/// player's characters gets a separate log entry for each, not one entry shared across them.
#[derive(Debug)]
pub struct State {
  character_id: i64,
  different: text_editor::Content,
  editing: bool,
  happened: text_editor::Content,
  /// Only seeds the default outcome and copy for a not-yet-saved report; ignored once a saved report loads its
  /// own outcome.
  is_kill: bool,
  killmail_id: i64,
  outcome: Outcome,
  saved: Option<SavedReport>,
  takeaway: String,
}

impl State {
  pub fn new(character_id: i64, killmail_id: i64, is_kill: bool) -> Self {
    State {
      character_id,
      different: text_editor::Content::new(),
      editing: true,
      happened: text_editor::Content::new(),
      is_kill,
      killmail_id,
      outcome: if is_kill { Outcome::Clean } else { Outcome::Costly },
      saved: None,
      takeaway: String::new(),
    }
  }

  pub fn character_id(&self) -> i64 {
    self.character_id
  }

  pub fn killmail_id(&self) -> i64 {
    self.killmail_id
  }

  fn apply_loaded(&mut self, report: Option<KillmailReport>) {
    match report {
      Some(report) => {
        self.outcome = Outcome::from_db(report.outcome());
        self.happened = text_editor::Content::with_text(report.happened());
        self.different = text_editor::Content::with_text(report.different().as_deref().unwrap_or(""));
        self.takeaway = report.takeaway().clone().unwrap_or_default();
        self.saved = Some(SavedReport::from_model(&report));
        self.editing = false;
      }
      None => {
        self.editing = true;
      }
    }
  }

  fn begin_edit(&mut self) {
    if let Some(saved) = &self.saved {
      self.outcome = saved.outcome;
      self.happened = text_editor::Content::with_text(&saved.happened);
      self.different = text_editor::Content::with_text(saved.different.as_deref().unwrap_or(""));
      self.takeaway = saved.takeaway.clone().unwrap_or_default();
    }
    self.editing = true;
  }

  fn build_input(&self) -> Option<ReportInput> {
    let happened = self.happened.text();
    let happened = happened.trim();
    if happened.is_empty() {
      return None;
    }

    Some(ReportInput {
      different: non_blank(&self.different.text()),
      happened: happened.to_owned(),
      outcome: self.outcome.as_str().to_owned(),
      takeaway: non_blank(&self.takeaway),
    })
  }

  fn can_save(&self) -> bool {
    !self.happened.text().trim().is_empty()
  }

  fn commit(&mut self, db: &Database) -> Task<Message> {
    let Some(input) = self.build_input() else {
      return Task::none();
    };

    self.saved = Some(SavedReport::from_input(&input, Utc::now().to_rfc3339()));
    self.editing = false;
    persist(db, self.character_id, self.killmail_id, input)
  }
}

#[derive(Clone, Debug)]
struct SavedReport {
  different: Option<String>,
  happened: String,
  outcome: Outcome,
  takeaway: Option<String>,
  updated_at: String,
}

impl SavedReport {
  fn from_input(input: &ReportInput, updated_at: String) -> Self {
    SavedReport {
      different: input.different.clone(),
      happened: input.happened.clone(),
      outcome: Outcome::from_db(&input.outcome),
      takeaway: input.takeaway.clone(),
      updated_at,
    }
  }

  fn from_model(report: &KillmailReport) -> Self {
    SavedReport {
      different: report.different().clone(),
      happened: report.happened().clone(),
      outcome: Outcome::from_db(report.outcome()),
      takeaway: report.takeaway().clone(),
      updated_at: report.updated_at().clone(),
    }
  }
}

pub fn load(db: &Database, character_id: i64, killmail_id: i64) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      killmail_report::get(&db, character_id, killmail_id)
        .await
        .ok()
        .flatten()
    },
    |report| Message::Loaded(Box::new(report)),
  )
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::Cancelled => {
      if state.saved.is_some() {
        state.editing = false;
      }
      Task::none()
    }
    Message::DifferentEdited(action) => {
      state.different.perform(action);
      Task::none()
    }
    Message::EditRequested => {
      state.begin_edit();
      Task::none()
    }
    Message::HappenedEdited(action) => {
      state.happened.perform(action);
      Task::none()
    }
    Message::Loaded(report) => {
      state.apply_loaded(*report);
      Task::none()
    }
    Message::OutcomeSelected(outcome) => {
      state.outcome = outcome;
      Task::none()
    }
    Message::SaveRequested => state.commit(db),
    Message::Saved => Task::none(),
    Message::TakeawayEdited(value) => {
      state.takeaway = value;
      Task::none()
    }
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  match &state.saved {
    Some(saved) if !state.editing => read_view(saved),
    _ => capture_form(state),
  }
}

fn persist(db: &Database, character_id: i64, killmail_id: i64, input: ReportInput) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      let _ = killmail_report::upsert(&db, character_id, killmail_id, &input).await;
    },
    |()| Message::Saved,
  )
}

fn tr_static(key: &str) -> &'static str {
  static CACHE: OnceLock<RwLock<HashMap<String, &'static str>>> = OnceLock::new();
  let cache = CACHE.get_or_init(|| RwLock::new(HashMap::new()));
  if let Some(&interned) = cache.read().expect("km_report i18n cache poisoned").get(key) {
    return interned;
  }

  let resolved: &'static str = Box::leak(t!(key).into_owned().into_boxed_str());
  cache
    .write()
    .expect("km_report i18n cache poisoned")
    .entry(key.to_owned())
    .or_insert(resolved)
}

fn non_blank(value: &str) -> Option<String> {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    None
  } else {
    Some(trimmed.to_owned())
  }
}

fn read_view(saved: &SavedReport) -> Element<'_, Message> {
  let head = Row::with_children(vec![
    outcome_badge(saved.outcome),
    Space::new().width(Length::Fill).into(),
    text(t!("captains_log.km_report.logged", when => logged_stamp(&saved.updated_at)))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
    Button::secondary(t!("captains_log.km_report.edit").into_owned())
      .size(Size::Sm)
      .icon(Icon::pencil())
      .on_press(Message::EditRequested)
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let mut sections: Vec<Element<'_, Message>> = vec![
    head.into(),
    read_panel(&t!("captains_log.km_report.read_happened"), &saved.happened, false),
  ];
  if let Some(different) = &saved.different {
    sections.push(read_panel(
      &t!("captains_log.km_report.read_different"),
      different,
      false,
    ));
  }
  if let Some(takeaway) = &saved.takeaway {
    sections.push(read_panel(&t!("captains_log.km_report.read_takeaway"), takeaway, true));
  }

  Column::with_children(sections)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn read_panel<'a>(label: &str, body: &str, accent: bool) -> Element<'a, Message> {
  let tint = if accent { color::accent() } else { color::text::PRIMARY };
  let value = text(body.to_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(move |_| text::Style {
      color: Some(tint),
    });

  Column::with_children(vec![
    eyebrow_text(label, Some(color::text::tertiary())).into(),
    value.into(),
  ])
  .spacing(spacing::SPACE_2)
  .width(Length::Fill)
  .into()
}

fn capture_form(state: &State) -> Element<'_, Message> {
  let intro_copy = if state.is_kill {
    t!("captains_log.km_report.intro_kill")
  } else {
    t!("captains_log.km_report.intro_loss")
  };
  let happened_placeholder = if state.is_kill {
    t!("captains_log.km_report.happened_placeholder_kill")
  } else {
    t!("captains_log.km_report.happened_placeholder_loss")
  };

  let intro = text(intro_copy.into_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let takeaway = TextInput::new(
    tr_static("captains_log.km_report.takeaway_placeholder"),
    &state.takeaway,
    Message::TakeawayEdited,
  )
  .background(color::surface::SUNKEN)
  .render();

  Column::with_children(vec![
    intro.into(),
    field(&t!("captains_log.km_report.outcome_label"), outcome_row(state.outcome)),
    field(
      &t!("captains_log.km_report.happened_label"),
      editor(
        &state.happened,
        &happened_placeholder,
        HAPPENED_HEIGHT,
        Message::HappenedEdited,
      ),
    ),
    field(
      &t!("captains_log.km_report.different_label"),
      editor(
        &state.different,
        &t!("captains_log.km_report.different_placeholder"),
        DIFFERENT_HEIGHT,
        Message::DifferentEdited,
      ),
    ),
    field(&t!("captains_log.km_report.takeaway_label"), takeaway),
    action_row(state),
  ])
  .spacing(spacing::SPACE_4_5)
  .width(Length::Fill)
  .into()
}

fn action_row(state: &State) -> Element<'_, Message> {
  let mut children: Vec<Element<'_, Message>> = vec![Space::new().width(Length::Fill).into()];
  if state.saved.is_some() {
    children.push(
      Button::secondary(t!("captains_log.km_report.cancel").into_owned())
        .size(Size::Sm)
        .on_press(Message::Cancelled)
        .into(),
    );
  }
  children.push(
    Button::primary(t!("captains_log.km_report.save").into_owned())
      .size(Size::Sm)
      .icon(Icon::check())
      .on_press_maybe(state.can_save().then_some(Message::SaveRequested))
      .into(),
  );

  Row::with_children(children)
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .into()
}

fn editor<'a>(
  content: &'a text_editor::Content,
  placeholder: &str,
  height: f32,
  on_action: impl Fn(text_editor::Action) -> Message + 'a,
) -> Element<'a, Message> {
  text_editor(content)
    .placeholder(placeholder.to_owned())
    .on_action(on_action)
    .padding(EDITOR_PADDING)
    .size(typography::size::MD)
    .height(Length::Fixed(height))
    .style(editor_style)
    .into()
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

fn field<'a>(label: &str, child: Element<'a, Message>) -> Element<'a, Message> {
  Column::with_children(vec![eyebrow_text(label, Some(color::text::secondary())).into(), child])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn outcome_row(selected: Outcome) -> Element<'static, Message> {
  Row::with_children(
    OUTCOME_ORDER
      .into_iter()
      .map(|outcome| outcome_pill(outcome, outcome == selected))
      .collect::<Vec<_>>(),
  )
  .spacing(spacing::SPACE_2)
  .into()
}

fn outcome_pill(outcome: Outcome, selected: bool) -> Element<'static, Message> {
  let tint = outcome.tint();
  let label = text(outcome.label())
    .font(typography::body::MEDIUM)
    .size(typography::size::SM);

  button(container(label).padding(Padding {
    top: spacing::SPACE_2 - 1.0,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2 - 1.0,
    left: spacing::SPACE_3,
  }))
  .on_press(Message::OutcomeSelected(outcome))
  .style(move |_, status| pill_style(tint, selected, status))
  .into()
}

fn pill_style(tint: Color, selected: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let (background, border, foreground) = if selected {
    (
      color::with_alpha(tint, PILL_ALPHA_FILL),
      color::with_alpha(tint, PILL_ALPHA_BORDER),
      tint,
    )
  } else if hovered {
    (
      Color::TRANSPARENT,
      color::with_alpha(color::text::PRIMARY, RULE_ALPHA),
      color::text::PRIMARY,
    )
  } else {
    (
      Color::TRANSPARENT,
      color::with_alpha(color::text::PRIMARY, RULE_ALPHA),
      color::text::secondary(),
    )
  };

  button::Style {
    background: Some(Background::Color(background)),
    border: Border {
      color: border,
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    text_color: foreground,
    ..button::Style::default()
  }
}

fn outcome_badge(outcome: Outcome) -> Element<'static, Message> {
  let tint = outcome.tint();
  container(eyebrow_text(&outcome.label(), Some(tint)))
    .padding(Padding {
      top: 4.0,
      right: spacing::SPACE_2_5,
      bottom: 4.0,
      left: spacing::SPACE_2_5,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(tint, PILL_ALPHA_FILL))),
      border: Border {
        color: color::with_alpha(tint, PILL_ALPHA_BORDER),
        radius: radius::SUBTLE.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn logged_stamp(updated_at: &str) -> String {
  match DateTime::parse_from_rfc3339(updated_at) {
    Ok(parsed) => parsed.format("%-d %b · %I:%M %p").to_string(),
    Err(_) => updated_at.to_owned(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn seeded_state(is_kill: bool) -> State {
    State::new(42, 100, is_kill)
  }

  mod build_input {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_rejects_a_blank_what_happened() {
      let mut state = seeded_state(true);
      state.happened = text_editor::Content::with_text("   \n ");

      assert!(state.build_input().is_none());
      assert!(!state.can_save());
    }

    #[test]
    fn it_trims_fields_and_drops_empty_optionals() {
      let mut state = seeded_state(false);
      state.outcome = Outcome::Learning;
      state.happened = text_editor::Content::with_text("  Lost the tackle.  ");
      state.different = text_editor::Content::with_text("   ");
      state.takeaway = "  Warp sooner.  ".to_owned();

      let input = state.build_input().expect("a valid report");

      assert_eq!(input.outcome, "learning");
      assert_eq!(input.happened, "Lost the tackle.");
      assert_eq!(input.different, None);
      assert_eq!(input.takeaway.as_deref(), Some("Warp sooner."));
    }
  }

  mod outcome {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_every_variant_through_the_db_string() {
      for outcome in OUTCOME_ORDER {
        assert_eq!(Outcome::from_db(outcome.as_str()), outcome);
      }
    }

    #[test]
    fn it_falls_back_to_clean_for_an_unknown_string() {
      assert_eq!(Outcome::from_db("mystery"), Outcome::Clean);
    }

    #[test]
    fn it_defaults_by_kind() {
      assert_eq!(seeded_state(true).outcome, Outcome::Clean);
      assert_eq!(seeded_state(false).outcome, Outcome::Costly);
    }
  }

  mod round_trip {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self, Database,
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::character,
    };

    async fn seed_character(db: &Database, id: i64) {
      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_saves_the_form_and_reloads_it_into_a_fresh_state() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let mut editor = State::new(42, 100, false);
      editor.outcome = Outcome::Learning;
      editor.happened = text_editor::Content::with_text("Reshipped and won.");
      editor.takeaway = "Bring a scout.".to_owned();
      let input = editor.build_input().expect("valid report");
      killmail_report::upsert(&db, 42, 100, &input).await.unwrap();

      let reloaded = killmail_report::get(&db, 42, 100).await.unwrap();
      let mut fresh = State::new(42, 100, false);
      fresh.apply_loaded(reloaded);

      let saved = fresh.saved.expect("a persisted report");
      assert_eq!(saved.outcome, Outcome::Learning);
      assert_eq!(saved.happened, "Reshipped and won.");
      assert_eq!(saved.takeaway.as_deref(), Some("Bring a scout."));
      assert!(!fresh.editing);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_capture_form_before_a_report_exists() {
      let state = seeded_state(true);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_read_view_after_a_report_loads() {
      let mut state = seeded_state(false);
      state.apply_loaded(Some(KillmailReport {
        character_id: 42,
        created_at: "2026-07-06T00:00:00Z".to_owned(),
        different: Some("Held the tackle too long.".to_owned()),
        happened: "Lost the tackle.".to_owned(),
        killmail_id: 100,
        outcome: "learning".to_owned(),
        takeaway: Some("Warp sooner.".to_owned()),
        updated_at: "2026-07-06T14:30:00Z".to_owned(),
      }));

      let _el: Element<'_, Message> = view(&state);
    }
  }
}
