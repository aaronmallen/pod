use std::collections::{HashMap, HashSet};

use iced::{
  Background, Border, Color, ContentFit, Element, Length, Padding, Task,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, image, text, text_editor},
};

use super::{Message as Parent, km_report, prompts};
use crate::{
  store::{
    Database,
    images::IconResolution,
    model::{CaptainsLog, PromptConfig},
    repo::captains_log::AnswerKey,
  },
  ui::{
    components::{
      button::{Button, Size},
      clip::clip_layer,
      eyebrow::eyebrow_text,
      icon::Icon,
      icon_tile::icon_tile,
    },
    format::fmt_isk,
    style::{color, radius, spacing, typography},
  },
};

const BADGE_ALPHA_BORDER: f32 = 0.35;
const BADGE_ALPHA_FILL: f32 = 0.12;
const BODY_PADDING_X: f32 = 24.0;
const BODY_PADDING_Y: f32 = 20.0;
const DOT_ACTIVE_WIDTH: f32 = 22.0;
const DOT_GAP: f32 = 4.0;
const DOT_SIZE: f32 = 8.0;
const EDITOR_HEIGHT: f32 = 84.0;
const EDITOR_PADDING: f32 = 13.0;
const ENGAGEMENT_TILE: f32 = 42.0;
const FORWARD_TINT: Color = Color {
  r: 0.482,
  g: 0.545,
  b: 0.851,
  a: 1.0,
};
const LABEL_SIZE: f32 = 21.0;
const PILL_RADIUS: f32 = 999.0;
const RAIL_PADDING_X: f32 = 20.0;
const RAIL_PADDING_Y: f32 = 13.0;

#[derive(Clone, Debug)]
pub enum Message {
  Back,
  ContinueEditing,
  DraftEdited(text_editor::Action),
  JumpTo(usize),
  NextRequested,
  Report(usize, km_report::Message),
  Saved,
  SkipRequested,
  StepSelected(usize),
}

#[derive(Clone, Debug)]
pub struct Engagement {
  pub character_id: i64,
  pub character_name: String,
  pub icon: IconResolution,
  pub is_kill: bool,
  pub killmail_id: i64,
  pub ship_name: String,
  pub system: String,
  pub value: f64,
}

#[derive(Debug)]
pub struct State {
  answers: HashMap<String, String>,
  draft: text_editor::Content,
  engagements: Vec<Engagement>,
  finished: bool,
  report_saved: Vec<bool>,
  reports: Vec<km_report::State>,
  skipped: HashSet<String>,
  step: usize,
  steps: Vec<Step>,
}

impl State {
  pub(super) fn is_finished(&self) -> bool {
    self.finished
  }

  pub fn new(
    config: &PromptConfig,
    activity: &prompts::DayActivity,
    engagements: Vec<Engagement>,
    log: Option<&CaptainsLog>,
    finished: bool,
  ) -> Self {
    let steps = build_steps(config, activity, engagements.len());
    let answers = load_answers(log);
    let reports = engagements
      .iter()
      .map(|engagement| km_report::State::new(engagement.character_id, engagement.killmail_id, engagement.is_kill))
      .collect();
    let report_saved = vec![false; engagements.len()];
    let draft = draft_for(&steps, 0, &answers);

    State {
      answers,
      draft,
      engagements,
      finished,
      report_saved,
      reports,
      skipped: HashSet::new(),
      step: 0,
      steps,
    }
  }

  fn advance(&mut self) {
    if self.is_last() {
      self.finished = true;
    } else {
      self.step += 1;
      self.reseed_draft();
    }
  }

  fn current_step(&self) -> Option<Step> {
    self.steps.get(self.step).cloned()
  }

  fn is_answered(&self, id: &str) -> bool {
    self.answers.get(id).is_some_and(|value| !value.trim().is_empty())
  }

  fn is_last(&self) -> bool {
    self.step + 1 >= self.steps.len()
  }

  fn reseed_draft(&mut self) {
    self.draft = draft_for(&self.steps, self.step, &self.answers);
  }

  fn set_answer(&mut self, id: &str, value: String) {
    if value.trim().is_empty() {
      self.answers.remove(id);
    } else {
      self.answers.insert(id.to_owned(), value);
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Step {
  /// `index` selects the day's engagement/report; `rail_label` is the conditional section's
  /// resolved progress-rail label.
  Combat {
    index: usize,
    rail_label: String,
  },
  Prompt(prompts::Prompt),
}

pub fn load(state: &State, db: &Database) -> Task<Parent> {
  let tasks = state
    .engagements
    .iter()
    .enumerate()
    .map(|(index, engagement)| {
      km_report::load(db, engagement.character_id, engagement.killmail_id)
        .map(move |message| Parent::Wizard(Message::Report(index, message)))
    })
    .collect::<Vec<_>>();

  Task::batch(tasks)
}

pub fn update_pane(state: &mut State, date: &str, db: &Database, message: Message) -> Task<Parent> {
  match message {
    Message::Saved => Task::none(),
    Message::Back => back(state),
    Message::ContinueEditing => set_composing(state),
    Message::DraftEdited(action) => edit_draft(state, action),
    Message::JumpTo(index) => jump_to(state, index),
    Message::NextRequested => advance_answer(state, date, db),
    Message::Report(index, report) => forward_report(state, index, report, db),
    Message::SkipRequested => skip(state),
    Message::StepSelected(index) => jump_to(state, index),
  }
}

pub fn view_pane(state: &State) -> Element<'_, Parent> {
  if state.finished {
    review_view(state)
  } else {
    composer_view(state)
  }
}

fn advance_answer(state: &mut State, date: &str, db: &Database) -> Task<Parent> {
  match state.current_step() {
    Some(Step::Prompt(prompt)) => {
      let value = state.draft.text().trim().to_owned();
      state.set_answer(&prompt.id, value.clone());
      let task = persist_answer(date, db, prompt.id, value);
      state.advance();
      task
    }
    _ => {
      state.advance();
      Task::none()
    }
  }
}

fn back(state: &mut State) -> Task<Parent> {
  state.step = state.step.saturating_sub(1);
  state.reseed_draft();
  Task::none()
}

/// Expands the day's combat prompt slot into one `Step::Combat` per engagement (rather than a
/// single step), in place of that slot in the prompt order.
fn build_steps(config: &PromptConfig, activity: &prompts::DayActivity, engagement_count: usize) -> Vec<Step> {
  let mut steps = Vec::new();
  for prompt in prompts::prompts_for_day(config, activity) {
    if prompt.key == Some(AnswerKey::Combat) {
      let rail_label = section_label(&prompt);
      steps.extend((0..engagement_count).map(|index| Step::Combat {
        index,
        rail_label: rail_label.clone(),
      }));
    } else {
      steps.push(Step::Prompt(prompt));
    }
  }

  steps
}

fn draft_for(steps: &[Step], index: usize, answers: &HashMap<String, String>) -> text_editor::Content {
  match steps.get(index) {
    Some(Step::Prompt(prompt)) => {
      text_editor::Content::with_text(answers.get(&prompt.id).map(String::as_str).unwrap_or_default())
    }
    _ => text_editor::Content::new(),
  }
}

fn edit_draft(state: &mut State, action: text_editor::Action) -> Task<Parent> {
  state.draft.perform(action);
  Task::none()
}

fn forward_report(state: &mut State, index: usize, message: km_report::Message, db: &Database) -> Task<Parent> {
  note_report_status(state, index, &message);
  match state.reports.get_mut(index) {
    Some(report) => {
      km_report::update(report, message, db).map(move |next| Parent::Wizard(Message::Report(index, next)))
    }
    None => Task::none(),
  }
}

fn jump_to(state: &mut State, index: usize) -> Task<Parent> {
  if index < state.steps.len() {
    state.finished = false;
    state.step = index;
    state.reseed_draft();
  }

  Task::none()
}

fn load_answers(log: Option<&CaptainsLog>) -> HashMap<String, String> {
  let mut answers = HashMap::new();
  let Some(log) = log else {
    return answers;
  };

  for key in AnswerKey::ALL {
    if let Some(value) = answer_text(log, key).filter(|value| !value.trim().is_empty()) {
      answers.insert(key.as_key().to_owned(), value.to_owned());
    }
  }
  // Catalog answers win on id collision: only backfill from the string map when the typed
  // field above didn't already claim this id.
  for (id, value) in log.answers() {
    if !value.trim().is_empty() {
      answers.entry(id.clone()).or_insert_with(|| value.clone());
    }
  }

  answers
}

fn answer_text(log: &CaptainsLog, key: AnswerKey) -> Option<&str> {
  match key {
    AnswerKey::Blocked => log.blocked().as_deref(),
    AnswerKey::Build => log.build().as_deref(),
    AnswerKey::Combat => log.combat().as_deref(),
    AnswerKey::Goal => log.goal().as_deref(),
    AnswerKey::Next => log.next().as_deref(),
    AnswerKey::Remember => log.remember().as_deref(),
    AnswerKey::Research => log.research().as_deref(),
    AnswerKey::Skill => log.skill().as_deref(),
  }
}

fn note_report_status(state: &mut State, index: usize, message: &km_report::Message) {
  let Some(slot) = state.report_saved.get_mut(index) else {
    return;
  };

  match message {
    km_report::Message::Loaded(report) => *slot = report.is_some(),
    km_report::Message::Saved => *slot = true,
    _ => {}
  }
}

fn persist_answer(date: &str, db: &Database, id: String, value: String) -> Task<Parent> {
  let db = db.clone();
  let date = date.to_owned();
  let stored = (!value.trim().is_empty()).then_some(value);

  Task::perform(
    async move {
      // Best-effort write: a failure here isn't surfaced to the user, since the in-memory
      // answer set via `State::set_answer` already reflects the change for this session.
      let _ = crate::store::repo::captains_log::upsert_answer(&db, &date, id.as_str(), stored.as_deref()).await;
    },
    |()| Message::Saved,
  )
  .map(Parent::Wizard)
}

fn set_composing(state: &mut State) -> Task<Parent> {
  state.finished = false;
  state.reseed_draft();
  Task::none()
}

fn skip(state: &mut State) -> Task<Parent> {
  if let Some(Step::Prompt(prompt)) = state.current_step() {
    state.skipped.insert(prompt.id);
  }
  state.advance();
  Task::none()
}

fn composer_view(state: &State) -> Element<'_, Parent> {
  let body = Column::with_children(vec![step_body(state), nav_row(state), hint(state)])
    .spacing(spacing::SPACE_4_5)
    .width(Length::Fill);

  let padded = container(body).width(Length::Fill).padding(Padding {
    top: BODY_PADDING_Y,
    right: BODY_PADDING_X,
    bottom: BODY_PADDING_Y,
    left: BODY_PADDING_X,
  });

  shell(
    Column::with_children(vec![progress_rail(state), padded.into()])
      .width(Length::Fill)
      .into(),
  )
}

fn step_body(state: &State) -> Element<'_, Parent> {
  match state.current_step() {
    Some(Step::Combat {
      index, ..
    }) => combat_step(state, index),
    Some(Step::Prompt(prompt)) => prompt_step(state, &prompt),
    None => Space::new().into(),
  }
}

fn prompt_step<'a>(state: &'a State, prompt: &prompts::Prompt) -> Element<'a, Parent> {
  let mut children: Vec<Element<'_, Parent>> = Vec::new();
  if let (prompts::PromptGroup::Conditional, Some(trigger)) = (prompt.group, prompt.trigger) {
    children.push(trigger_badge(trigger));
  }
  children.push(label_row(prompt));
  children.push(draft_editor(state));

  Column::with_children(children)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn combat_step(state: &State, index: usize) -> Element<'_, Parent> {
  let total = state.reports.len();
  let mut children: Vec<Element<'_, Parent>> = vec![combat_badge(index, total)];
  if let Some(engagement) = state.engagements.get(index) {
    children.push(engagement_header(engagement, index, total));
  }
  if let Some(report) = state.reports.get(index) {
    children.push(km_report::view(report).map(move |message| Parent::Wizard(Message::Report(index, message))));
  }

  Column::with_children(children)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn draft_editor(state: &State) -> Element<'_, Parent> {
  let placeholder = match state.current_step() {
    Some(Step::Prompt(prompt)) => question_placeholder(&prompt),
    _ => String::new(),
  };

  text_editor(&state.draft)
    .placeholder(placeholder)
    .on_action(|action| Parent::Wizard(Message::DraftEdited(action)))
    .padding(EDITOR_PADDING)
    .size(typography::size::LG)
    .height(Length::Fixed(EDITOR_HEIGHT))
    .style(editor_style)
    .into()
}

fn editor_style(_theme: &iced::Theme, _status: text_editor::Status) -> text_editor::Style {
  text_editor::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: color::rule(),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    placeholder: color::text::tertiary(),
    selection: color::accent_muted(),
    value: color::text::PRIMARY,
  }
}

fn label_row(prompt: &prompts::Prompt) -> Element<'static, Parent> {
  let label = text(question_label(prompt))
    .font(typography::body::MEDIUM)
    .size(LABEL_SIZE)
    .style(typography::colored(color::text::PRIMARY));

  let mut children: Vec<Element<'static, Parent>> = vec![label.into()];
  if matches!(prompt.group, prompts::PromptGroup::Forward) {
    children.push(eyebrow_text(&t!("captains_log.wizard.optional"), Some(color::text::tertiary())).into());
  }

  Row::with_children(children)
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center)
    .into()
}

fn trigger_badge(trigger: prompts::Trigger) -> Element<'static, Parent> {
  badge(color::status::WARNING, Icon::spark(), tr(trigger_reason_key(trigger)))
}

fn combat_badge(index: usize, total: usize) -> Element<'static, Parent> {
  let copy = t!(
    "captains_log.wizard.combat_badge",
    index => (index + 1).to_string(),
    total => total.to_string()
  )
  .into_owned();

  badge(color::status::WARNING, Icon::spark(), copy)
}

fn badge(tint: Color, icon: Icon, label: String) -> Element<'static, Parent> {
  let row = Row::with_children(vec![
    icon.size(13.0).color(tint).render::<Parent>(),
    text(label)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(tint))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  container(row)
    .padding(Padding {
      top: 4.0,
      right: spacing::SPACE_3,
      bottom: 4.0,
      left: spacing::SPACE_2_5,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(tint, BADGE_ALPHA_FILL))),
      border: Border {
        color: color::with_alpha(tint, BADGE_ALPHA_BORDER),
        radius: PILL_RADIUS.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn engagement_header(engagement: &Engagement, index: usize, total: usize) -> Element<'static, Parent> {
  let tint = if engagement.is_kill {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let ship = text(engagement.ship_name.clone())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let kind = eyebrow_text(&kind_label(engagement.is_kill), Some(tint));
  let title = Row::with_children(vec![ship.into(), kind.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center);

  let sign = if engagement.is_kill { "+" } else { "\u{2212}" };
  let meta = text(format!(
    "{} \u{00b7} {} \u{00b7} {sign}{}",
    engagement.character_name,
    engagement.system,
    fmt_isk(engagement.value)
  ))
  .font(typography::mono::REGULAR)
  .size(typography::size::XS_PLUS)
  .style(typography::colored(color::text::secondary()));

  let column = Column::with_children(vec![title.into(), meta.into()])
    .spacing(DOT_GAP)
    .width(Length::Fill);

  let counter = text(format!("{} / {total}", index + 1))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::tertiary()));

  Row::with_children(vec![type_tile(&engagement.icon), column.into(), counter.into()])
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

fn progress_rail(state: &State) -> Element<'_, Parent> {
  let (tint, label) = current_group_meta(state);
  let counter = text(format!("{} / {}", state.step + 1, state.steps.len()))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::tertiary()));

  let dots = Row::with_children(
    (0..state.steps.len())
      .map(|index| progress_dot(state, index, tint))
      .collect::<Vec<_>>(),
  )
  .spacing(DOT_GAP);

  let row = Row::with_children(vec![
    eyebrow_text(&label, Some(tint)).into(),
    Space::new().width(Length::Fill).into(),
    counter.into(),
    dots.into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: RAIL_PADDING_Y,
      right: RAIL_PADDING_X,
      bottom: RAIL_PADDING_Y,
      left: RAIL_PADDING_X,
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

fn progress_dot(state: &State, index: usize, active_tint: Color) -> Element<'_, Parent> {
  let current = index == state.step;
  let fill = dot_fill(state, index, current, active_tint);
  let width = if current { DOT_ACTIVE_WIDTH } else { DOT_SIZE };

  button(Space::new())
    .width(Length::Fixed(width))
    .height(Length::Fixed(DOT_SIZE))
    .on_press(Parent::Wizard(Message::StepSelected(index)))
    .style(move |_, _| button::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        color: color::rule(),
        radius: (DOT_SIZE / 2.0).into(),
        width: 0.0,
      },
      ..button::Style::default()
    })
    .into()
}

fn dot_fill(state: &State, index: usize, current: bool, active_tint: Color) -> Color {
  if current {
    return active_tint;
  }
  if step_done(state, index) {
    color::status::ONLINE
  } else {
    color::rule()
  }
}

fn nav_row(state: &State) -> Element<'static, Parent> {
  let last = state.is_last();
  let combat = matches!(state.current_step(), Some(Step::Combat { .. }));

  let back = Button::secondary(t!("captains_log.wizard.back").into_owned())
    .size(Size::Sm)
    .icon(Icon::chevron_left())
    .on_press_maybe((state.step > 0).then_some(Parent::Wizard(Message::Back)));

  let skip = Button::ghost(t!("captains_log.wizard.skip").into_owned())
    .size(Size::Sm)
    .on_press(Parent::Wizard(Message::SkipRequested));

  Row::with_children(vec![
    back.into(),
    Space::new().width(Length::Fill).into(),
    skip.into(),
    next_button(combat, last).into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill)
  .into()
}

fn next_button(combat: bool, last: bool) -> Button<Parent> {
  let label = next_label(combat, last);
  let base = if combat {
    Button::secondary(label)
  } else {
    Button::primary(label)
  };
  let icon = if !combat && last {
    Icon::check()
  } else {
    Icon::chevron_right()
  };

  base
    .size(Size::Sm)
    .icon_right(icon)
    .on_press(Parent::Wizard(Message::NextRequested))
}

fn next_label(combat: bool, last: bool) -> String {
  let key = match (combat, last) {
    (true, true) => "captains_log.wizard.finish",
    (false, true) => "captains_log.wizard.save_entry",
    _ => "captains_log.wizard.next",
  };

  t!(key).into_owned()
}

fn hint(state: &State) -> Element<'static, Parent> {
  if matches!(state.current_step(), Some(Step::Combat { .. })) {
    return Space::new().into();
  }

  text(t!("captains_log.wizard.hint").into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::tertiary()))
    .into()
}

fn review_view(state: &State) -> Element<'_, Parent> {
  let head = Row::with_children(vec![
    Icon::check().size(15.0).color(color::status::ONLINE).render::<Parent>(),
    eyebrow_text(&t!("captains_log.wizard.review_saved"), Some(color::text::secondary())).into(),
    Space::new().width(Length::Fill).into(),
    Button::secondary(t!("captains_log.wizard.continue_editing").into_owned())
      .size(Size::Sm)
      .icon(Icon::reset())
      .on_press(Parent::Wizard(Message::ContinueEditing))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let rows = state
    .steps
    .iter()
    .enumerate()
    .map(|(index, step)| review_row(state, index, step))
    .collect::<Vec<_>>();

  let body = container(Column::with_children(rows).spacing(DOT_GAP).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: DOT_GAP,
      right: spacing::SPACE_6,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_6,
    });

  let rail = container(head).width(Length::Fill).padding(Padding {
    top: RAIL_PADDING_Y,
    right: RAIL_PADDING_X,
    bottom: RAIL_PADDING_Y,
    left: RAIL_PADDING_X,
  });

  shell(
    Column::with_children(vec![rail.into(), body.into()])
      .width(Length::Fill)
      .into(),
  )
}

fn review_row<'a>(state: &'a State, index: usize, step: &Step) -> Element<'a, Parent> {
  let inner = match step {
    Step::Combat {
      index: engagement, ..
    } => review_debrief(state, index, *engagement),
    Step::Prompt(prompt) => review_prompt(state, index, prompt),
  };

  container(inner)
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

fn review_prompt<'a>(state: &'a State, index: usize, prompt: &prompts::Prompt) -> Element<'a, Parent> {
  let label = text(question_label(prompt))
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()));

  let value = state.answers.get(&prompt.id).filter(|value| !value.trim().is_empty());
  let body = match value {
    Some(answer) => answer_text_element(answer),
    None => unanswered_link(index, state.skipped.contains(&prompt.id), false),
  };

  Column::with_children(vec![label.into(), body])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn review_debrief(state: &State, index: usize, engagement_index: usize) -> Element<'_, Parent> {
  let Some(engagement) = state.engagements.get(engagement_index) else {
    return Space::new().into();
  };
  let saved = state.report_saved.get(engagement_index).copied().unwrap_or(false);
  let missing = !engagement.is_kill && !saved;

  let label = text(t!("captains_log.wizard.review_debrief", ship => engagement.ship_name.clone()).into_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()));
  let mut head: Vec<Element<'_, Parent>> = vec![label.into()];
  if missing {
    head.push(missing_tag());
  }
  let header = Row::with_children(head)
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

  let body: Element<'_, Parent> = if saved {
    state
      .reports
      .get(engagement_index)
      .map(|report| {
        km_report::view(report).map(move |message| Parent::Wizard(Message::Report(engagement_index, message)))
      })
      .unwrap_or_else(|| Space::new().into())
  } else {
    unanswered_link(index, false, true)
  };

  Column::with_children(vec![header.into(), body])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn answer_text_element(answer: &str) -> Element<'static, Parent> {
  text(answer.to_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY))
    .into()
}

fn missing_tag() -> Element<'static, Parent> {
  Row::with_children(vec![
    Icon::block()
      .size(11.0)
      .color(color::status::WARNING)
      .render::<Parent>(),
    eyebrow_text(&t!("captains_log.wizard.missing"), Some(color::status::WARNING)).into(),
  ])
  .spacing(DOT_GAP)
  .align_y(Vertical::Center)
  .into()
}

fn unanswered_link(index: usize, skipped: bool, debrief: bool) -> Element<'static, Parent> {
  let key = match (debrief, skipped) {
    (true, true) => "captains_log.wizard.skipped_debrief",
    (true, false) => "captains_log.wizard.not_debriefed",
    (false, true) => "captains_log.wizard.skipped_add",
    (false, false) => "captains_log.wizard.not_answered",
  };

  Button::ghost(t!(key).into_owned())
    .size(Size::Sm)
    .on_press(Parent::Wizard(Message::JumpTo(index)))
    .into()
}

fn shell(body: Element<'_, Parent>) -> Element<'_, Parent> {
  container(body)
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn group_tint(group: prompts::PromptGroup) -> Color {
  match group {
    prompts::PromptGroup::Conditional => color::status::WARNING,
    prompts::PromptGroup::Core | prompts::PromptGroup::Custom => color::accent(),
    prompts::PromptGroup::Forward => FORWARD_TINT,
  }
}

fn current_group_meta(state: &State) -> (Color, String) {
  match state.current_step() {
    Some(Step::Combat {
      rail_label, ..
    }) => (color::status::WARNING, rail_label),
    Some(Step::Prompt(prompt)) => (group_tint(prompt.group), section_label(&prompt)),
    None => (color::accent(), t!("captains_log.wizard.group_core").into_owned()),
  }
}

fn step_done(state: &State, index: usize) -> bool {
  match state.steps.get(index) {
    Some(Step::Combat {
      index: engagement, ..
    }) => state.report_saved.get(*engagement).copied().unwrap_or(false),
    Some(Step::Prompt(prompt)) => state.is_answered(&prompt.id),
    None => false,
  }
}

fn resolve(i18n_key: &str, literal: &str) -> String {
  if i18n_key.is_empty() {
    literal.to_owned()
  } else {
    t!(i18n_key).into_owned()
  }
}

fn section_label(prompt: &prompts::Prompt) -> String {
  resolve(&prompt.section_i18n_key, &prompt.section_label)
}

/// A prompt's question label: catalog questions keep their fixed wizard i18n key (so the shipped
/// experience is byte-for-byte identical across locales); custom questions resolve their config
/// i18n key, falling back to the stored literal.
fn question_label(prompt: &prompts::Prompt) -> String {
  match prompt.key {
    Some(key) => tr(&format!("captains_log.wizard.{}_label", key.as_key())),
    None => resolve(&prompt.i18n_key, &prompt.label),
  }
}

fn question_placeholder(prompt: &prompts::Prompt) -> String {
  match prompt.key {
    Some(key) => tr(&format!("captains_log.wizard.{}_placeholder", key.as_key())),
    None => prompt.placeholder.clone(),
  }
}

fn kind_label(is_kill: bool) -> String {
  let key = if is_kill {
    "captains_log.wizard.kill"
  } else {
    "captains_log.wizard.loss"
  };

  t!(key).into_owned()
}

fn trigger_reason_key(trigger: prompts::Trigger) -> &'static str {
  match trigger {
    prompts::Trigger::Engagement => "captains_log.wizard.reason_losses",
    prompts::Trigger::Industry => "captains_log.wizard.reason_industry",
    prompts::Trigger::Skills => "captains_log.wizard.reason_skills",
  }
}

fn tr(key: &str) -> String {
  t!(key).into_owned()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn engagement(character_id: i64, killmail_id: i64, is_kill: bool) -> Engagement {
    Engagement {
      character_id,
      character_name: "Pilot".to_owned(),
      icon: IconResolution::Missing,
      is_kill,
      killmail_id,
      ship_name: "Caracal".to_owned(),
      system: "Tama".to_owned(),
      value: 12_000_000.0,
    }
  }

  fn quiet_state() -> State {
    State::new(
      &PromptConfig::default(),
      &prompts::DayActivity::default(),
      Vec::new(),
      None,
      false,
    )
  }

  fn combat_activity(count: u32) -> prompts::DayActivity {
    prompts::DayActivity {
      engagement_count: count,
      ..prompts::DayActivity::default()
    }
  }

  fn combat_state(count: u32, engagements: Vec<Engagement>) -> State {
    State::new(
      &PromptConfig::default(),
      &combat_activity(count),
      engagements,
      None,
      false,
    )
  }

  mod build_steps {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_lists_only_core_and_forward_prompts_on_a_quiet_day() {
      let steps = build_steps(&PromptConfig::default(), &prompts::DayActivity::default(), 0);

      assert_eq!(steps.len(), 5);
      assert!(steps.iter().all(|step| matches!(step, Step::Prompt(_))));
    }

    #[test]
    fn it_expands_the_combat_prompt_into_one_step_per_engagement() {
      let steps = build_steps(&PromptConfig::default(), &combat_activity(3), 3);

      let combat = steps.iter().filter(|step| matches!(step, Step::Combat { .. })).count();
      assert_eq!(combat, 3);
      assert_eq!(steps.len(), 8);
    }

    #[test]
    fn it_orders_combat_before_the_forward_prompts() {
      let steps = build_steps(&PromptConfig::default(), &combat_activity(1), 1);

      let combat_at = steps
        .iter()
        .position(|step| matches!(step, Step::Combat { .. }))
        .unwrap();
      let next_at = steps
        .iter()
        .position(|step| matches!(step, Step::Prompt(prompt) if prompt.id == "next"))
        .unwrap();

      assert!(combat_at < next_at);
    }
  }

  mod advance {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_walks_forward_without_finishing_until_the_last_step() {
      let mut state = quiet_state();

      state.advance();
      assert_eq!(state.step, 1);
      assert!(!state.finished);
    }

    #[test]
    fn it_transitions_to_review_after_the_final_step() {
      let mut state = quiet_state();
      state.step = state.steps.len() - 1;

      state.advance();

      assert!(state.finished);
    }
  }

  mod update_pane {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    #[tokio::test]
    async fn it_persists_the_answer_and_advances_on_next() {
      let db = store::open_test().await.unwrap();
      let mut state = quiet_state();
      state.draft = text_editor::Content::with_text("Barge line.");

      let _ = update_pane(&mut state, "2026-07-06", &db, Message::NextRequested);

      assert_eq!(state.answers.get("goal").map(String::as_str), Some("Barge line."));
      assert_eq!(state.step, 1);
    }

    #[tokio::test]
    async fn it_marks_a_prompt_skipped_and_advances() {
      let db = store::open_test().await.unwrap();
      let mut state = quiet_state();

      let _ = update_pane(&mut state, "2026-07-06", &db, Message::SkipRequested);

      assert!(state.skipped.contains("goal"));
      assert_eq!(state.step, 1);
    }

    #[tokio::test]
    async fn it_steps_back_without_dropping_below_zero() {
      let db = store::open_test().await.unwrap();
      let mut state = quiet_state();
      state.step = 2;

      let _ = update_pane(&mut state, "2026-07-06", &db, Message::Back);
      assert_eq!(state.step, 1);

      state.step = 0;
      let _ = update_pane(&mut state, "2026-07-06", &db, Message::Back);
      assert_eq!(state.step, 0);
    }

    #[tokio::test]
    async fn it_returns_to_composing_from_review_via_jump() {
      let db = store::open_test().await.unwrap();
      let mut state = quiet_state();
      state.finished = true;

      let _ = update_pane(&mut state, "2026-07-06", &db, Message::JumpTo(1));

      assert!(!state.finished);
      assert_eq!(state.step, 1);
    }

    #[tokio::test]
    async fn it_persists_a_custom_question_answer_by_id() {
      let db = store::open_test().await.unwrap();
      let mut config = PromptConfig::default();
      config.sections[0].questions.push(crate::store::model::PromptQuestion {
        id: "mood".to_owned(),
        kind: crate::store::model::PromptQuestionKind::Text,
        label: "Mood".to_owned(),
        i18n_key: String::new(),
        placeholder: String::new(),
        required: false,
      });
      let mut state = State::new(&config, &prompts::DayActivity::default(), Vec::new(), None, false);
      state.step = 3;
      state.draft = text_editor::Content::with_text("focused");

      let _ = update_pane(&mut state, "2026-07-06", &db, Message::NextRequested);

      assert_eq!(state.answers.get("mood").map(String::as_str), Some("focused"));
    }
  }

  mod reports {
    use super::*;

    #[test]
    fn it_records_a_saved_debrief_from_a_report_message() {
      let mut state = combat_state(1, vec![engagement(4, 100, false)]);

      note_report_status(&mut state, 0, &km_report::Message::Saved);

      assert!(state.report_saved[0]);
      assert!(step_done(&state, combat_step_index(&state)));
    }

    #[test]
    fn it_clears_a_debrief_when_a_load_finds_nothing() {
      let mut state = combat_state(1, vec![engagement(4, 100, false)]);
      state.report_saved[0] = true;

      note_report_status(&mut state, 0, &km_report::Message::Loaded(Box::new(None)));

      assert!(!state.report_saved[0]);
    }

    fn combat_step_index(state: &State) -> usize {
      state
        .steps
        .iter()
        .position(|step| matches!(step, Step::Combat { .. }))
        .unwrap()
    }
  }

  mod load_answers {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_seeds_authored_answers_and_ignores_blanks() {
      let log = CaptainsLog {
        goal: Some("Spin up the barge line.".to_owned()),
        remember: Some("   ".to_owned()),
        ..CaptainsLog::default()
      };

      let answers = load_answers(Some(&log));

      assert_eq!(answers.get("goal").map(String::as_str), Some("Spin up the barge line."));
      assert!(!answers.contains_key("remember"));
    }

    #[test]
    fn it_seeds_custom_answers_from_the_string_map() {
      let mut log = CaptainsLog::default();
      log.answers.insert("mood".to_owned(), "focused".to_owned());

      let answers = load_answers(Some(&log));

      assert_eq!(answers.get("mood").map(String::as_str), Some("focused"));
    }
  }

  mod review_debrief {
    use super::*;

    #[test]
    fn it_renders_saved_missing_and_out_of_range_debriefs() {
      let mut state = combat_state(2, vec![engagement(4, 100, false), engagement(5, 200, true)]);

      // A loss with no saved report flags the missing debrief.
      let _ = review_debrief(&state, 0, 0);
      // A kill with no report is not flagged missing but still links back.
      let _ = review_debrief(&state, 1, 1);

      // A saved report renders the read view inline.
      state.report_saved[0] = true;
      let _ = review_debrief(&state, 0, 0);

      // An out-of-range engagement collapses to empty space.
      let _ = review_debrief(&state, 0, 99);
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_the_composer_for_a_prompt_step() {
      let state = quiet_state();

      let _el: Element<'_, Parent> = view_pane(&state);
    }

    #[test]
    fn it_renders_the_combat_step() {
      let state = combat_state(1, vec![engagement(4, 100, false)]);

      let _el: Element<'_, Parent> = view_pane(&state);
    }

    #[test]
    fn it_renders_the_review_state() {
      let mut state = quiet_state();
      state.finished = true;

      let _el: Element<'_, Parent> = view_pane(&state);
    }
  }
}
