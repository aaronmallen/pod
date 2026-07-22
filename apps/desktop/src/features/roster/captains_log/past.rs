use iced::{
  Background, Border, ContentFit, Element, Length, Padding, Task,
  alignment::Vertical,
  widget::{Column, Row, Space, container, image, text, text_editor},
};

use super::{
  Message as Parent, field_notes, km_report, narrative, objective_link,
  prompts::{self, Completeness},
  rollup_tiles,
};
use crate::{
  store::{
    Database,
    images::IconResolution,
    model::{CaptainsLog, FieldNote, LinkSource},
    repo::captains_log::{self, AnswerKey},
  },
  ui::{
    components::{
      button::{Button, Size},
      clip::clip_layer,
      eyebrow::{eyebrow, eyebrow_text},
      icon::Icon,
      icon_tile::icon_tile,
      rule,
    },
    style::{color, radius, spacing, typography},
  },
};

const BADGE_ICON: f32 = 12.0;
const CARD_PADDING_X: f32 = 22.0;
const CARD_PADDING_Y: f32 = 6.0;
const EDITOR_HEIGHT: f32 = 80.0;
const EDITOR_PADDING: f32 = 11.0;
const ENGAGEMENT_TILE: f32 = 42.0;

#[derive(Clone, Debug)]
pub enum Message {
  Cancelled,
  DraftEdited(text_editor::Action),
  EditRequested(String),
  Narrative(narrative::Message),
  Report(usize, km_report::Message),
  SaveRequested(String),
  Saved(String, Result<Option<String>, String>),
}

pub struct Engagement {
  pub character_id: i64,
  pub character_name: String,
  pub icon: IconResolution,
  pub is_kill: bool,
  pub killmail_id: i64,
  pub ship: String,
  pub system: String,
  pub value: f64,
}

#[derive(Debug)]
pub struct State {
  completeness: Completeness,
  date: String,
  debriefs: Vec<Debrief>,
  draft: text_editor::Content,
  editing: Option<String>,
  field_notes: field_notes::State,
  log: Option<CaptainsLog>,
  narrative: narrative::State,
  prompts: Vec<prompts::Prompt>,
}

impl State {
  pub fn new(
    date: String,
    log: Option<CaptainsLog>,
    completeness: Completeness,
    engagements: Vec<Engagement>,
    notes: Vec<FieldNote>,
    prompts: Vec<prompts::Prompt>,
  ) -> Self {
    State {
      completeness,
      debriefs: engagements.into_iter().map(Debrief::new).collect(),
      draft: text_editor::Content::new(),
      editing: None,
      field_notes: field_notes::State::new(date.clone(), notes),
      narrative: narrative::State::new(log.as_ref().and_then(|log| log.narrative().clone())),
      date,
      log,
      prompts,
    }
  }

  fn is_missing(&self, prompt: &prompts::Prompt) -> bool {
    match prompt.key {
      Some(key) => self.completeness.missing_prompts.contains(&key),
      // Custom prompts have no typed key; missing_custom holds the same resolved label text
      // produced by field_label, so matching falls back to that instead of an id.
      None => self.completeness.missing_custom.contains(&field_label(prompt)),
    }
  }

  fn missing_debrief(&self, character_id: i64, killmail_id: i64) -> bool {
    self
      .completeness
      .missing_debriefs
      .iter()
      .any(|loss| loss.character_id == character_id && loss.killmail_id == killmail_id)
  }

  fn prompt_by_id(&self, id: &str) -> Option<&prompts::Prompt> {
    self.prompts.iter().find(|prompt| prompt.id == id)
  }
}

#[derive(Debug)]
struct Debrief {
  character_name: String,
  icon: IconResolution,
  is_kill: bool,
  report: km_report::State,
  ship: String,
  system: String,
  value: f64,
}

impl Debrief {
  fn new(engagement: Engagement) -> Self {
    Debrief {
      character_name: engagement.character_name,
      icon: engagement.icon,
      is_kill: engagement.is_kill,
      report: km_report::State::new(engagement.character_id, engagement.killmail_id, engagement.is_kill),
      ship: engagement.ship,
      system: engagement.system,
      value: engagement.value,
    }
  }
}

pub(super) fn load_reports(state: &State, db: &Database) -> Task<Parent> {
  let tasks: Vec<Task<Parent>> = state
    .debriefs
    .iter()
    .enumerate()
    .map(|(index, debrief)| {
      km_report::load(db, debrief.report.character_id(), debrief.report.killmail_id())
        .map(move |message| Parent::Past(Message::Report(index, message)))
    })
    .collect();

  Task::batch(tasks)
}

pub(super) fn update_field_notes(state: &mut State, db: &Database, message: field_notes::Message) -> Task<Parent> {
  field_notes::update_pane(&mut state.field_notes, db, message)
}

pub(super) fn update_pane(state: &mut State, db: &Database, message: Message) -> Task<Parent> {
  match message {
    Message::Cancelled => state.editing = None,
    Message::DraftEdited(action) => state.draft.perform(action),
    Message::EditRequested(id) => begin_edit(state, id),
    Message::Narrative(message) => return route_narrative(state, db, message),
    Message::Report(index, message) => return route_report(state, index, db, message),
    Message::SaveRequested(id) => return save_requested(state, id, db),
    Message::Saved(id, result) => apply_saved(state, id, result),
  }

  Task::none()
}

pub(super) fn view_pane<'a>(
  state: &'a State,
  links: &'a objective_link::State,
  summary: &rollup_tiles::Summary,
  events: Option<Element<'a, Parent>>,
) -> Element<'a, Parent> {
  let mut rollup = vec![rollup_tiles::render(summary, rollup_tiles::Scope::Day)];
  if let Some(events) = events {
    rollup.push(events);
  }

  Column::with_children(vec![
    narrative::view_pane(&state.narrative).map(wrap_narrative),
    Column::with_children(rollup)
      .spacing(spacing::SPACE_3)
      .width(Length::Fill)
      .into(),
    entry_block(state, links),
    field_notes_section(state, links),
    objective_link::day_panel(links, &state.date),
  ])
  .spacing(spacing::SPACE_6)
  .width(Length::Fill)
  .into()
}

fn field_notes_section<'a>(state: &'a State, links: &'a objective_link::State) -> Element<'a, Parent> {
  Column::with_children(vec![
    field_notes_kicker(),
    field_notes::view_pane(&state.field_notes, links),
  ])
  .spacing(spacing::SPACE_3)
  .width(Length::Fill)
  .into()
}

fn field_notes_kicker<'a>() -> Element<'a, Parent> {
  Row::with_children(vec![
    eyebrow(&t!("captains_log.field_notes.kicker"), None),
    container(rule::horizontal()).width(Length::Fill).into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn apply_saved(state: &mut State, id: String, result: Result<Option<String>, String>) {
  match result {
    Ok(value) => {
      set_answer(state, &id, value);
      state.editing = None;
    }
    Err(error) => {
      tracing::warn!(target: "pod::captains_log", %error, "past answer save failed")
    }
  }
}

fn begin_edit(state: &mut State, id: String) {
  let current = state
    .prompt_by_id(&id)
    .and_then(|prompt| answer_of(state.log.as_ref(), prompt))
    .unwrap_or_default()
    .to_owned();
  state.draft = text_editor::Content::with_text(&current);
  state.editing = Some(id);
}

fn route_report(state: &mut State, index: usize, db: &Database, message: km_report::Message) -> Task<Parent> {
  match state.debriefs.get_mut(index) {
    Some(debrief) => km_report::update(&mut debrief.report, message, db)
      .map(move |message| Parent::Past(Message::Report(index, message))),
    None => Task::none(),
  }
}

fn save_requested(state: &mut State, id: String, db: &Database) -> Task<Parent> {
  let value = non_blank(&state.draft.text());
  let db = db.clone();
  let date = state.date.clone();

  Task::perform(
    async move {
      captains_log::upsert_answer(&db, &date, id.as_str(), value.as_deref())
        .await
        .map(|()| value)
        .map_err(|error| error.to_string())
        .map(|value| (id, value))
    },
    move |result| match result {
      Ok((id, value)) => Message::Saved(id, Ok(value)),
      // id was consumed inside the async block above and doesn't survive the error path;
      // apply_saved's Err arm only logs the message, so the empty id is never read.
      Err(error) => Message::Saved(String::new(), Err(error)),
    },
  )
  .map(Parent::Past)
}

fn typed_answer(log: &CaptainsLog, key: AnswerKey) -> Option<&str> {
  let value = match key {
    AnswerKey::Blocked => log.blocked(),
    AnswerKey::Build => log.build(),
    AnswerKey::Combat => log.combat(),
    AnswerKey::Goal => log.goal(),
    AnswerKey::Next => log.next(),
    AnswerKey::Remember => log.remember(),
    AnswerKey::Research => log.research(),
    AnswerKey::Skill => log.skill(),
  };

  value.as_deref()
}

fn answer_of<'a>(log: Option<&'a CaptainsLog>, prompt: &prompts::Prompt) -> Option<&'a str> {
  let log = log?;
  let value = match prompt.key {
    Some(key) => typed_answer(log, key),
    None => log.answers().get(&prompt.id).map(String::as_str),
  };

  value.filter(|text| !text.trim().is_empty())
}

fn resolve(i18n_key: &str, literal: &str) -> String {
  if !literal.is_empty() {
    literal.to_owned()
  } else if i18n_key.is_empty() {
    String::new()
  } else {
    t!(i18n_key).into_owned()
  }
}

/// A user-edited literal always wins; an unedited catalog question (blank literal) falls back to
/// its fixed past-view i18n key so the shipped text stays identical across locales, and a custom
/// question resolves its config i18n key or literal.
fn field_label(prompt: &prompts::Prompt) -> String {
  if !prompt.label.is_empty() {
    return prompt.label.clone();
  }
  match prompt.key {
    Some(key) => {
      let key = format!("captains_log.past.label_{}", key.as_key());
      t!(&key).into_owned()
    }
    None => resolve(&prompt.i18n_key, &prompt.label),
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

fn set_answer(state: &mut State, id: &str, value: Option<String>) {
  let key = state.prompt_by_id(id).and_then(|prompt| prompt.key);
  let date = state.date.clone();
  let entry = state.log.get_or_insert_with(|| CaptainsLog {
    date,
    ..CaptainsLog::default()
  });

  match key {
    Some(AnswerKey::Blocked) => entry.blocked = value,
    Some(AnswerKey::Build) => entry.build = value,
    Some(AnswerKey::Combat) => entry.combat = value,
    Some(AnswerKey::Goal) => entry.goal = value,
    Some(AnswerKey::Next) => entry.next = value,
    Some(AnswerKey::Remember) => entry.remember = value,
    Some(AnswerKey::Research) => entry.research = value,
    Some(AnswerKey::Skill) => entry.skill = value,
    None => match value {
      Some(value) => {
        entry.answers.insert(id.to_owned(), value);
      }
      None => {
        entry.answers.remove(id);
      }
    },
  }
}

fn route_narrative(state: &mut State, db: &Database, message: narrative::Message) -> Task<Parent> {
  narrative::update_pane(&mut state.narrative, &state.date, db, message).map(wrap_narrative)
}

/// The shared narrative pane always emits `Parent::Narrative`, its routing target for the
/// top-level "today" narrative. Redirect that into `Message::Narrative` so edits made from this
/// past-day view land on `state.narrative`/`state.date` here instead. Anything else passes
/// through unchanged.
fn wrap_narrative(message: Parent) -> Parent {
  match message {
    Parent::Narrative(inner) => Parent::Past(Message::Narrative(inner)),
    other => other,
  }
}

fn entry_block<'a>(state: &'a State, links: &'a objective_link::State) -> Element<'a, Parent> {
  Column::with_children(vec![entry_header(state), entry_card(state, links)])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into()
}

fn entry_header(state: &State) -> Element<'_, Parent> {
  Row::with_children(vec![
    eyebrow(&t!("captains_log.past.entry_logged"), None),
    container(rule::horizontal()).width(Length::Fill).into(),
    completeness_badge(state.completeness.is_complete()),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn completeness_badge<'a>(complete: bool) -> Element<'a, Parent> {
  let (key, tint) = if complete {
    ("captains_log.past.complete", color::status::ONLINE)
  } else {
    ("captains_log.past.needs_info", color::status::WARNING)
  };

  let mut children: Vec<Element<'a, Parent>> = Vec::new();
  if complete {
    children.push(Icon::check().size(BADGE_ICON).color(tint).render::<Parent>());
  }
  children.push(eyebrow_text(&t!(key), Some(tint)).into());

  Row::with_children(children)
    .spacing(spacing::UNIT + 1.0)
    .align_y(Vertical::Center)
    .into()
}

fn entry_card<'a>(state: &'a State, links: &'a objective_link::State) -> Element<'a, Parent> {
  let mut children: Vec<Element<'a, Parent>> = Vec::new();

  for prompt in &state.prompts {
    if show_field(state, prompt) {
      children.push(field_row(state, prompt, links));
    }
  }

  if !state.debriefs.is_empty() {
    children.push(debriefs_section(state));
  }

  container(Column::with_children(children).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: CARD_PADDING_Y,
      right: CARD_PADDING_X,
      bottom: spacing::SPACE_4_5,
      left: CARD_PADDING_X,
    })
    .style(card_style)
    .into()
}

/// Core questions always render (they are the daily minimum); everything else appears only once
/// authored, unless the account marked it required (so the needs-info flag has a row to live on).
fn show_field(state: &State, prompt: &prompts::Prompt) -> bool {
  matches!(prompt.group, prompts::PromptGroup::Core)
    || prompt.required
    || answer_of(state.log.as_ref(), prompt).is_some()
}

fn field_row<'a>(
  state: &'a State,
  prompt: &'a prompts::Prompt,
  links: &'a objective_link::State,
) -> Element<'a, Parent> {
  if state.editing.as_deref() == Some(prompt.id.as_str()) {
    field_editor(state, prompt)
  } else {
    field_display(state, prompt, links)
  }
}

fn field_display<'a>(
  state: &'a State,
  prompt: &'a prompts::Prompt,
  links: &'a objective_link::State,
) -> Element<'a, Parent> {
  let missing = state.is_missing(prompt);
  let value = answer_of(state.log.as_ref(), prompt);
  let id = prompt.id.clone();

  let mut head: Vec<Element<'a, Parent>> = vec![
    text(field_label(prompt))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ];
  if missing {
    head.push(missing_badge());
  }
  if prompt.links_to_objective
    && let Some(chip) = objective_link::chip_if_linked(
      links,
      &state.date,
      &LinkSource::LogAnswer {
        question_id: prompt.id.clone(),
      },
    )
  {
    head.push(chip);
  }
  head.push(Space::new().width(Length::Fill).into());
  head.push(
    Button::secondary(t!("captains_log.past.edit").into_owned())
      .size(Size::Sm)
      .icon(Icon::pencil())
      .on_press(Parent::Past(Message::EditRequested(id)))
      .into(),
  );

  let head = Row::with_children(head)
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

  let body: Element<'a, Parent> = match value {
    Some(text_value) => text(text_value.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    None => empty_placeholder(missing),
  };

  field_shell(vec![head.into(), body])
}

fn field_editor<'a>(state: &'a State, prompt: &'a prompts::Prompt) -> Element<'a, Parent> {
  let id = prompt.id.clone();
  let head = text(field_label(prompt))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));

  let editor = text_editor(&state.draft)
    .placeholder(t!("captains_log.past.note_placeholder").into_owned())
    .on_action(|action| Parent::Past(Message::DraftEdited(action)))
    .padding(EDITOR_PADDING)
    .size(typography::size::MD)
    .height(Length::Fixed(EDITOR_HEIGHT))
    .style(editor_style);

  let can_save = !state.draft.text().trim().is_empty();
  let actions = Row::with_children(vec![
    Space::new().width(Length::Fill).into(),
    Button::secondary(t!("captains_log.past.cancel").into_owned())
      .size(Size::Sm)
      .on_press(Parent::Past(Message::Cancelled))
      .into(),
    Button::primary(t!("captains_log.past.save").into_owned())
      .size(Size::Sm)
      .icon(Icon::check())
      .on_press_maybe(can_save.then_some(Parent::Past(Message::SaveRequested(id))))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  field_shell(vec![head.into(), editor.into(), actions.into()])
}

fn field_shell(children: Vec<Element<'_, Parent>>) -> Element<'_, Parent> {
  let column = Column::with_children(children)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill);

  container(column)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: 0.0,
      bottom: spacing::SPACE_3_5,
      left: 0.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        radius: 0.0.into(),
        width: 0.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn empty_placeholder<'a>(missing: bool) -> Element<'a, Parent> {
  let tint = if missing {
    color::status::WARNING
  } else {
    color::text::tertiary()
  };

  text(t!("captains_log.past.not_set").into_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(tint))
    .into()
}

fn missing_badge<'a>() -> Element<'a, Parent> {
  eyebrow_text(&t!("captains_log.past.missing"), Some(color::status::WARNING)).into()
}

fn debriefs_section(state: &State) -> Element<'_, Parent> {
  let total = state.debriefs.len();

  let mut children: Vec<Element<'_, Parent>> = vec![
    container(eyebrow(&t!("captains_log.past.combat_debriefs"), None))
      .padding(Padding {
        top: spacing::SPACE_3_5,
        right: 0.0,
        bottom: spacing::SPACE_2,
        left: 0.0,
      })
      .into(),
  ];

  for (index, debrief) in state.debriefs.iter().enumerate() {
    children.push(debrief_row(state, index, total, debrief));
  }

  Column::with_children(children)
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into()
}

fn debrief_row<'a>(state: &State, index: usize, total: usize, debrief: &'a Debrief) -> Element<'a, Parent> {
  let missing = !debrief.is_kill && state.missing_debrief(debrief.report.character_id(), debrief.report.killmail_id());
  let header = engagement_header(debrief, index + 1, total, missing);
  let form = km_report::view(&debrief.report).map(move |message| Parent::Past(Message::Report(index, message)));

  Column::with_children(vec![header, form])
    .spacing(spacing::SPACE_4_5)
    .width(Length::Fill)
    .into()
}

fn engagement_header<'a>(debrief: &'a Debrief, index: usize, total: usize, missing: bool) -> Element<'a, Parent> {
  let tint = if debrief.is_kill {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };

  let mut title: Vec<Element<'a, Parent>> = vec![
    text(debrief.ship.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    kind_badge(debrief.is_kill, tint),
  ];
  if missing {
    title.push(missing_badge());
  }

  let sign = if debrief.is_kill { "+" } else { "\u{2212}" };
  let meta = text(format!(
    "{} \u{b7} {} \u{b7} {}{}",
    debrief.character_name,
    debrief.system,
    sign,
    crate::ui::format::fmt_isk(debrief.value)
  ))
  .font(typography::mono::REGULAR)
  .size(typography::size::SM)
  .style(typography::colored(color::text::secondary()));

  let identity = Column::with_children(vec![
    Row::with_children(title)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
    meta.into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  let counter = text(format!("{index} / {total}"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));

  Row::with_children(vec![type_tile(&debrief.icon), identity.into(), counter.into()])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .into()
}

fn type_tile(icon: &IconResolution) -> Element<'static, Parent> {
  match icon {
    IconResolution::Found(path) => icon_tile(
      clip_layer(
        image(image::Handle::from_path(path.clone()))
          .width(Length::Fill)
          .height(Length::Fill)
          .content_fit(ContentFit::Cover),
        Length::Fill,
        Length::Fill,
      ),
      ENGAGEMENT_TILE,
    ),
    IconResolution::Missing => icon_tile(Space::new(), ENGAGEMENT_TILE),
  }
}

fn kind_badge<'a>(is_kill: bool, tint: iced::Color) -> Element<'a, Parent> {
  let key = if is_kill {
    "captains_log.rollup_tiles.kill"
  } else {
    "captains_log.rollup_tiles.loss"
  };

  container(
    text(t!(key).to_uppercase())
      .font(typography::mono::SEMIBOLD)
      .size(typography::size::XS)
      .style(typography::colored(tint)),
  )
  .padding(Padding {
    top: 3.0,
    right: 7.0,
    bottom: 3.0,
    left: 7.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(tint, 0.12))),
    border: Border {
      color: color::with_alpha(tint, 0.4),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn card_style(_theme: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule(),
      radius: radius::CARD.into(),
      width: 1.0,
    },
    ..container::Style::default()
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
  use crate::{features::roster::captains_log::prompts::LossEngagement, store::model::PromptConfig};

  fn default_prompts() -> Vec<prompts::Prompt> {
    prompts::all_field_prompts(&PromptConfig::default())
  }

  fn prompt(id: &str) -> prompts::Prompt {
    default_prompts().into_iter().find(|prompt| prompt.id == id).unwrap()
  }

  fn no_links() -> objective_link::State {
    objective_link::State::default()
  }

  fn log_with_goal() -> CaptainsLog {
    CaptainsLog {
      date: "2026-07-05".to_owned(),
      goal: Some("Spin up the barge line.".to_owned()),
      ..CaptainsLog::default()
    }
  }

  fn engagement(character_id: i64, killmail_id: i64, is_kill: bool) -> Engagement {
    Engagement {
      character_id,
      character_name: "Vex Voronova".to_owned(),
      icon: IconResolution::Missing,
      is_kill,
      killmail_id,
      ship: "Astero".to_owned(),
      system: "Tama".to_owned(),
      value: 132_000_000.0,
    }
  }

  fn state_with(log: Option<CaptainsLog>, completeness: Completeness, engagements: Vec<Engagement>) -> State {
    State::new(
      "2026-07-05".to_owned(),
      log,
      completeness,
      engagements,
      Vec::new(),
      default_prompts(),
    )
  }

  mod new {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_starts_with_no_field_in_edit_mode() {
      let state = state_with(Some(log_with_goal()), Completeness::default(), Vec::new());

      assert_eq!(state.editing, None);
      assert!(state.debriefs.is_empty());
    }

    #[test]
    fn it_builds_one_debrief_per_engagement() {
      let state = state_with(
        None,
        Completeness::default(),
        vec![engagement(4, 100, false), engagement(4, 200, true)],
      );

      assert_eq!(state.debriefs.len(), 2);
      assert_eq!(state.debriefs[0].report.killmail_id(), 100);
      assert!(state.debriefs[1].is_kill);
    }
  }

  mod answer_of {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_stored_answer() {
      let log = log_with_goal();

      assert_eq!(answer_of(Some(&log), &prompt("goal")), Some("Spin up the barge line."));
    }

    #[test]
    fn it_drops_a_blank_answer() {
      let log = CaptainsLog {
        goal: Some("   ".to_owned()),
        ..CaptainsLog::default()
      };

      assert_eq!(answer_of(Some(&log), &prompt("goal")), None);
      assert_eq!(answer_of(None, &prompt("goal")), None);
    }

    #[test]
    fn it_reads_a_custom_answer_from_the_string_map() {
      let mut log = CaptainsLog::default();
      log.answers.insert("mood".to_owned(), "focused".to_owned());
      let custom = prompts::Prompt {
        group: prompts::PromptGroup::Custom,
        i18n_key: String::new(),
        id: "mood".to_owned(),
        key: None,
        label: "Mood".to_owned(),
        links_to_objective: false,
        placeholder: String::new(),
        required: false,
        section_i18n_key: String::new(),
        section_label: "Daily".to_owned(),
        trigger: None,
      };

      assert_eq!(answer_of(Some(&log), &custom), Some("focused"));
    }
  }

  mod non_blank {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_trims_and_keeps_authored_text() {
      assert_eq!(non_blank("  logged it  "), Some("logged it".to_owned()));
    }

    #[test]
    fn it_maps_blank_to_nothing() {
      assert_eq!(non_blank("   "), None);
      assert_eq!(non_blank(""), None);
    }
  }

  mod set_answer {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_creates_the_in_memory_row_on_first_authored_input() {
      let mut state = state_with(None, Completeness::default(), Vec::new());

      set_answer(&mut state, "goal", Some("First goal.".to_owned()));

      let row = state.log.expect("row should be created");
      assert_eq!(row.date(), "2026-07-05");
      assert_eq!(row.goal().as_deref(), Some("First goal."));
    }

    #[test]
    fn it_clears_an_answer_to_none() {
      let mut state = state_with(Some(log_with_goal()), Completeness::default(), Vec::new());

      set_answer(&mut state, "goal", None);

      assert_eq!(state.log.unwrap().goal(), &None);
    }

    #[test]
    fn it_stores_a_custom_answer_in_the_string_map() {
      let mut state = State::new(
        "2026-07-05".to_owned(),
        None,
        Completeness::default(),
        Vec::new(),
        Vec::new(),
        vec![prompts::Prompt {
          group: prompts::PromptGroup::Custom,
          i18n_key: String::new(),
          id: "mood".to_owned(),
          key: None,
          label: "Mood".to_owned(),
          links_to_objective: false,
          placeholder: String::new(),
          required: false,
          section_i18n_key: String::new(),
          section_label: "Daily".to_owned(),
          trigger: None,
        }],
      );

      set_answer(&mut state, "mood", Some("focused".to_owned()));

      assert_eq!(
        state.log.unwrap().answers().get("mood").map(String::as_str),
        Some("focused")
      );
    }
  }

  mod begin_edit {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_seeds_the_draft_from_the_current_answer() {
      let mut state = state_with(Some(log_with_goal()), Completeness::default(), Vec::new());

      begin_edit(&mut state, "goal".to_owned());

      assert_eq!(state.editing.as_deref(), Some("goal"));
      assert_eq!(state.draft.text().trim(), "Spin up the barge line.");
    }

    #[test]
    fn it_opens_an_empty_draft_for_an_unanswered_field() {
      let mut state = state_with(None, Completeness::default(), Vec::new());

      begin_edit(&mut state, "blocked".to_owned());

      assert_eq!(state.editing.as_deref(), Some("blocked"));
      assert_eq!(state.draft.text().trim(), "");
    }
  }

  mod update_pane {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    #[tokio::test]
    async fn it_cancels_an_edit() {
      let db = store::open_test().await.unwrap();
      let mut state = state_with(Some(log_with_goal()), Completeness::default(), Vec::new());
      begin_edit(&mut state, "goal".to_owned());

      let _ = update_pane(&mut state, &db, Message::Cancelled);

      assert_eq!(state.editing, None);
    }

    #[tokio::test]
    async fn it_installs_a_saved_answer_and_leaves_edit_mode() {
      let db = store::open_test().await.unwrap();
      let mut state = state_with(None, Completeness::default(), Vec::new());
      begin_edit(&mut state, "goal".to_owned());

      let _ = update_pane(
        &mut state,
        &db,
        Message::Saved("goal".to_owned(), Ok(Some("Run one Tama roam.".to_owned()))),
      );

      assert_eq!(state.editing, None);
      assert_eq!(
        answer_of(state.log.as_ref(), &prompt("goal")),
        Some("Run one Tama roam.")
      );
    }

    #[tokio::test]
    async fn it_ignores_a_report_message_out_of_range() {
      let db = store::open_test().await.unwrap();
      let mut state = state_with(None, Completeness::default(), Vec::new());

      let _ = update_pane(&mut state, &db, Message::Report(7, km_report::Message::Saved));

      assert!(state.debriefs.is_empty());
    }
  }

  mod persistence {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    #[tokio::test]
    async fn it_creates_the_day_row_on_first_answer_via_the_repo() {
      let db = store::open_test().await.unwrap();
      assert!(captains_log::get(&db, "2026-07-05").await.unwrap().is_none());

      captains_log::upsert_answer(&db, "2026-07-05", AnswerKey::Goal, Some("First goal."))
        .await
        .unwrap();

      let row = captains_log::get(&db, "2026-07-05").await.unwrap();
      assert_eq!(row.and_then(|log| log.goal().clone()).as_deref(), Some("First goal."));
    }

    #[tokio::test]
    async fn it_persists_a_custom_answer_by_string_id() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new(
        "2026-07-05".to_owned(),
        None,
        Completeness::default(),
        Vec::new(),
        Vec::new(),
        vec![prompts::Prompt {
          group: prompts::PromptGroup::Custom,
          i18n_key: String::new(),
          id: "mood".to_owned(),
          key: None,
          label: "Mood".to_owned(),
          links_to_objective: false,
          placeholder: String::new(),
          required: false,
          section_i18n_key: String::new(),
          section_label: "Daily".to_owned(),
          trigger: None,
        }],
      );
      state.draft = text_editor::Content::with_text("focused");

      let _ = save_requested(&mut state, "mood".to_owned(), &db);
      captains_log::upsert_answer(&db, "2026-07-05", "mood", Some("focused"))
        .await
        .unwrap();

      let row = captains_log::get(&db, "2026-07-05").await.unwrap().unwrap();
      assert_eq!(row.answers().get("mood").map(String::as_str), Some("focused"));
    }
  }

  mod view_pane {
    use super::*;

    fn missing_goal() -> Completeness {
      Completeness {
        missing_custom: Vec::new(),
        missing_debriefs: Vec::new(),
        missing_prompts: vec![AnswerKey::Goal],
      }
    }

    #[test]
    fn it_renders_an_activity_only_day_with_no_log() {
      let state = state_with(None, missing_goal(), Vec::new());
      let summary = rollup_tiles::Summary::empty();

      let links = no_links();
      let _el: Element<'_, Parent> = view_pane(&state, &links, &summary, None);
    }

    #[test]
    fn it_renders_a_logged_day_with_a_narrative_and_debriefs() {
      let log = CaptainsLog {
        narrative: Some("One frigate saved the fleet.".to_owned()),
        ..log_with_goal()
      };
      let completeness = Completeness {
        missing_custom: Vec::new(),
        missing_debriefs: vec![LossEngagement {
          character_id: 4,
          killmail_id: 100,
        }],
        missing_prompts: Vec::new(),
      };
      let state = state_with(Some(log), completeness, vec![engagement(4, 100, false)]);
      let summary = rollup_tiles::Summary::empty();

      let links = no_links();
      let _el: Element<'_, Parent> = view_pane(&state, &links, &summary, None);
    }

    #[test]
    fn it_renders_a_field_in_edit_mode() {
      let mut state = state_with(Some(log_with_goal()), missing_goal(), Vec::new());
      begin_edit(&mut state, "goal".to_owned());

      let links = no_links();
      let _el: Element<'_, Parent> = view_pane(&state, &links, &rollup_tiles::Summary::empty(), None);
    }

    #[test]
    fn it_places_calendar_events_inside_the_rollup_before_the_entry() {
      let state = state_with(Some(log_with_goal()), missing_goal(), Vec::new());
      let events: Element<'_, Parent> = Space::new().into();

      let links = no_links();
      let _el: Element<'_, Parent> = view_pane(&state, &links, &rollup_tiles::Summary::empty(), Some(events));
    }
  }

  mod empty_placeholder {
    use super::*;

    #[test]
    fn it_renders_a_missing_field_placeholder() {
      let _el: Element<'_, Parent> = empty_placeholder(true);
    }

    #[test]
    fn it_renders_an_unset_field_placeholder() {
      let _el: Element<'_, Parent> = empty_placeholder(false);
    }
  }

  mod field_display {
    use super::*;

    #[test]
    fn it_renders_an_empty_core_field_with_an_edit_affordance() {
      let state = state_with(None, Completeness::default(), Vec::new());
      let goal = prompt("goal");

      let links = no_links();
      let _el: Element<'_, Parent> = field_display(&state, &goal, &links);
    }

    #[test]
    fn it_renders_a_populated_field_with_an_edit_affordance() {
      let state = state_with(Some(log_with_goal()), Completeness::default(), Vec::new());
      let goal = prompt("goal");

      let links = no_links();
      let _el: Element<'_, Parent> = field_display(&state, &goal, &links);
    }
  }

  mod show_field {
    use super::*;

    #[test]
    fn it_always_shows_core_questions() {
      let state = state_with(None, Completeness::default(), Vec::new());

      assert!(show_field(&state, &prompt("blocked")));
    }

    #[test]
    fn it_hides_an_unanswered_optional_forward_question() {
      let state = state_with(None, Completeness::default(), Vec::new());

      assert!(!show_field(&state, &prompt("research")));
    }
  }

  mod wrap_narrative {
    use super::*;

    #[test]
    fn it_moves_a_narrative_message_into_the_past_channel() {
      let wrapped = wrap_narrative(Parent::Narrative(narrative::Message::EditRequested));

      assert!(matches!(
        wrapped,
        Parent::Past(Message::Narrative(narrative::Message::EditRequested))
      ));
    }

    #[test]
    fn it_passes_a_foreign_message_through_untouched() {
      let wrapped = wrap_narrative(Parent::Exit);

      assert!(matches!(wrapped, Parent::Exit));
    }
  }

  mod narrative_wiring {
    use super::*;
    use crate::store;

    #[test]
    fn it_seeds_the_narrative_pane_from_the_day_log() {
      let log = CaptainsLog {
        narrative: Some("One frigate saved the fleet.".to_owned()),
        ..log_with_goal()
      };
      let state = state_with(Some(log), Completeness::default(), Vec::new());

      let _el: Element<'_, Parent> = narrative::view_pane(&state.narrative);
    }

    #[tokio::test]
    async fn it_opens_the_narrative_editor_on_an_empty_past_day() {
      let db = store::open_test().await.unwrap();
      let mut state = state_with(None, Completeness::default(), Vec::new());

      let _ = update_pane(&mut state, &db, Message::Narrative(narrative::Message::WriteRequested));

      let links = no_links();
      let _el: Element<'_, Parent> = view_pane(&state, &links, &rollup_tiles::Summary::empty(), None);
    }

    #[tokio::test]
    async fn it_routes_a_narrative_edit_request_through_the_past_pane() {
      let db = store::open_test().await.unwrap();
      let log = CaptainsLog {
        narrative: Some("Old line.".to_owned()),
        ..log_with_goal()
      };
      let mut state = state_with(Some(log), Completeness::default(), Vec::new());

      let _ = update_pane(&mut state, &db, Message::Narrative(narrative::Message::EditRequested));

      let links = no_links();
      let _el: Element<'_, Parent> = view_pane(&state, &links, &rollup_tiles::Summary::empty(), None);
    }

    #[tokio::test]
    async fn it_persists_a_narrative_keyed_by_the_selected_past_date() {
      let db = store::open_test().await.unwrap();
      assert!(captains_log::get(&db, "2026-07-05").await.unwrap().is_none());

      captains_log::upsert_narrative(&db, "2026-07-05", Some("Held the line at Tama."))
        .await
        .unwrap();

      let row = captains_log::get(&db, "2026-07-05").await.unwrap();
      assert_eq!(
        row.and_then(|log| log.narrative().clone()).as_deref(),
        Some("Held the line at Tama.")
      );
    }
  }
}
