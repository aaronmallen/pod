use iced::{
  Background, Border, Element, Length, Padding, Task,
  alignment::Vertical,
  widget::{Column, Row, button, container, scrollable, text, text_input},
};

use super::Outcome;
use crate::{
  store::{
    Database,
    model::{PromptConfig, PromptQuestion, PromptQuestionKind, PromptSection, PromptSectionKind, PromptTriggers},
    repo::captains_log,
  },
  ui::{
    components::{
      button::{Button, Size},
      icon::Icon,
      rule, toggle,
    },
    style::{color, radius, spacing, typography},
  },
};

const CARD_MAX_WIDTH: f32 = 720.0;
#[cfg(test)]
const CONDITIONAL_ID: &str = "conditional";
const DESCRIPTION_MAX_WIDTH: f32 = 620.0;
const MINI_BUTTON_SIZE: f32 = 28.0;
const PANEL_SIDE_PADDING: f32 = 36.0;

/// Listed in display order for the conditional card, not alphabetically like the enum.
const TRIGGERS: [Trigger; 3] = [Trigger::Combat, Trigger::Build, Trigger::Skill];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
  Down,
  Up,
}

impl Direction {
  fn delta(self) -> isize {
    match self {
      Direction::Down => 1,
      Direction::Up => -1,
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditTarget {
  QuestionLabel(String, String),
  QuestionPlaceholder(String, String),
  SectionLabel(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Trigger {
  Build,
  Combat,
  Skill,
}

impl Trigger {
  fn description_key(self) -> &'static str {
    match self {
      Trigger::Build => "settings.captains_log.trigger_industry_desc",
      Trigger::Combat => "settings.captains_log.trigger_combat_desc",
      Trigger::Skill => "settings.captains_log.trigger_skill_desc",
    }
  }

  fn get(self, triggers: &PromptTriggers) -> bool {
    match self {
      Trigger::Build => triggers.build,
      Trigger::Combat => triggers.combat,
      Trigger::Skill => triggers.skill,
    }
  }

  fn label_key(self) -> &'static str {
    match self {
      Trigger::Build => "settings.captains_log.trigger_industry_label",
      Trigger::Combat => "settings.captains_log.trigger_combat_label",
      Trigger::Skill => "settings.captains_log.trigger_skill_label",
    }
  }

  fn set(self, triggers: &mut PromptTriggers, value: bool) {
    match self {
      Trigger::Build => triggers.build = value,
      Trigger::Combat => triggers.combat = value,
      Trigger::Skill => triggers.skill = value,
    }
  }
}

#[derive(Clone, Debug)]
pub enum Message {
  AddQuestion(String),
  AddSection,
  DeleteQuestion(String, String),
  DeleteSection(String),
  EditChanged(String),
  EditCommitted,
  Loaded(Result<PromptConfig, String>),
  MoveQuestion(String, String, Direction),
  MoveSection(String, Direction),
  Reset,
  Saved(Result<(), String>),
  StartEdit(EditTarget),
  ToggleQuestionObjective(String, String, bool),
  ToggleQuestionRequired(String, String, bool),
  ToggleTrigger(String, Trigger, bool),
}

#[derive(Clone, Debug)]
struct Editing {
  draft: String,
  target: EditTarget,
}

#[derive(Debug)]
pub struct State {
  config: PromptConfig,
  db: Option<Database>,
  editing: Option<Editing>,
  error: Option<String>,
  seq: u64,
}

impl State {
  pub fn new(db: Database) -> Self {
    State {
      config: PromptConfig::default(),
      db: Some(db),
      editing: None,
      error: None,
      seq: 0,
    }
  }

  #[cfg(test)]
  fn config(&self) -> &PromptConfig {
    &self.config
  }
}

pub fn load(db: &Database) -> Task<Message> {
  Task::perform(load_config(db.clone()), Message::Loaded)
}

async fn load_config(db: Database) -> Result<PromptConfig, String> {
  captains_log::load_prompt_config(&db)
    .await
    .map_err(|err| err.to_string())
}

pub fn update(state: &mut State, message: Message) -> (Outcome, Task<Message>) {
  let task = match message {
    Message::AddQuestion(section_id) => add_question(state, &section_id),
    Message::AddSection => add_section(state),
    Message::DeleteQuestion(section_id, question_id) => delete_question(state, &section_id, &question_id),
    Message::DeleteSection(section_id) => delete_section(state, &section_id),
    Message::EditChanged(draft) => {
      if let Some(editing) = state.editing.as_mut() {
        editing.draft = draft;
      }
      Task::none()
    }
    Message::EditCommitted => commit_edit(state),
    Message::Loaded(result) => {
      match result {
        Ok(config) => {
          state.config = config;
          state.error = None;
        }
        Err(error) => state.error = Some(error),
      }
      Task::none()
    }
    Message::MoveQuestion(section_id, question_id, direction) => {
      move_question(state, &section_id, &question_id, direction)
    }
    Message::MoveSection(section_id, direction) => move_section(state, &section_id, direction),
    Message::Reset => reset_to_defaults(state),
    Message::Saved(result) => {
      if let Err(error) = result {
        state.error = Some(error);
      }
      Task::none()
    }
    Message::StartEdit(target) => {
      state.editing = Some(Editing {
        draft: edit_source(&state.config, &target),
        target,
      });
      Task::none()
    }
    Message::ToggleQuestionObjective(section_id, question_id, value) => {
      set_links_to_objective(state, &section_id, &question_id, value)
    }
    Message::ToggleQuestionRequired(section_id, question_id, value) => {
      set_required(state, &section_id, &question_id, value)
    }
    Message::ToggleTrigger(section_id, trigger, value) => set_trigger(state, &section_id, trigger, value),
  };
  (Outcome::None, task)
}

pub fn reset_to_defaults(state: &mut State) -> Task<Message> {
  state.config = PromptConfig::default();
  state.editing = None;
  persist(state)
}

pub fn badge(state: &State) -> String {
  let questions: usize = state
    .config
    .sections
    .iter()
    .filter(|section| section.kind == PromptSectionKind::Free)
    .map(|section| section.questions.len())
    .sum();
  questions.to_string()
}

fn add_question(state: &mut State, section_id: &str) -> Task<Message> {
  let id = state.next_id("q");
  if let Some(section) = free_section_mut(&mut state.config, section_id) {
    section.questions.push(PromptQuestion {
      id,
      kind: PromptQuestionKind::Text,
      label: t!("settings.captains_log.new_question").into_owned(),
      i18n_key: String::new(),
      placeholder: String::new(),
      required: false,
      links_to_objective: false,
    });
    persist(state)
  } else {
    Task::none()
  }
}

fn add_section(state: &mut State) -> Task<Message> {
  let id = state.next_id("section");
  state.config.sections.push(PromptSection {
    id,
    kind: PromptSectionKind::Free,
    label: t!("settings.captains_log.new_section").into_owned(),
    i18n_key: String::new(),
    questions: Vec::new(),
    triggers: None,
  });
  persist(state)
}

fn commit_edit(state: &mut State) -> Task<Message> {
  let Some(editing) = state.editing.take() else {
    return Task::none();
  };
  let draft = editing.draft.trim().to_owned();
  let applied = match &editing.target {
    EditTarget::QuestionLabel(section_id, question_id) => {
      if let Some(question) = question_mut(&mut state.config, section_id, question_id) {
        question.label = draft;
        question.i18n_key = String::new();
        true
      } else {
        false
      }
    }
    EditTarget::QuestionPlaceholder(section_id, question_id) => {
      if let Some(question) = question_mut(&mut state.config, section_id, question_id) {
        question.placeholder = draft;
        true
      } else {
        false
      }
    }
    EditTarget::SectionLabel(section_id) => {
      if let Some(section) = free_section_mut(&mut state.config, section_id) {
        section.label = draft;
        section.i18n_key = String::new();
        true
      } else {
        false
      }
    }
  };
  if applied { persist(state) } else { Task::none() }
}

fn delete_question(state: &mut State, section_id: &str, question_id: &str) -> Task<Message> {
  let Some(section) = free_section_mut(&mut state.config, section_id) else {
    return Task::none();
  };
  let before = section.questions.len();
  section.questions.retain(|question| question.id != question_id);
  if section.questions.len() == before {
    Task::none()
  } else {
    persist(state)
  }
}

fn delete_section(state: &mut State, section_id: &str) -> Task<Message> {
  let index = state
    .config
    .sections
    .iter()
    .position(|section| section.id == section_id && section.kind == PromptSectionKind::Free);
  match index {
    Some(index) => {
      state.config.sections.remove(index);
      persist(state)
    }
    None => Task::none(),
  }
}

fn move_question(state: &mut State, section_id: &str, question_id: &str, direction: Direction) -> Task<Message> {
  let Some(section) = free_section_mut(&mut state.config, section_id) else {
    return Task::none();
  };
  let Some(from) = section.questions.iter().position(|question| question.id == question_id) else {
    return Task::none();
  };
  if swap(&mut section.questions, from, direction) {
    persist(state)
  } else {
    Task::none()
  }
}

fn move_section(state: &mut State, section_id: &str, direction: Direction) -> Task<Message> {
  let Some(from) = state
    .config
    .sections
    .iter()
    .position(|section| section.id == section_id)
  else {
    return Task::none();
  };
  if swap(&mut state.config.sections, from, direction) {
    persist(state)
  } else {
    Task::none()
  }
}

fn set_required(state: &mut State, section_id: &str, question_id: &str, value: bool) -> Task<Message> {
  if let Some(question) = question_mut(&mut state.config, section_id, question_id) {
    question.required = value;
    persist(state)
  } else {
    Task::none()
  }
}

fn set_links_to_objective(state: &mut State, section_id: &str, question_id: &str, value: bool) -> Task<Message> {
  if let Some(question) = question_mut(&mut state.config, section_id, question_id) {
    question.links_to_objective = value;
    persist(state)
  } else {
    Task::none()
  }
}

fn set_trigger(state: &mut State, section_id: &str, trigger: Trigger, value: bool) -> Task<Message> {
  let section = state
    .config
    .sections
    .iter_mut()
    .find(|section| section.id == section_id && section.kind == PromptSectionKind::Conditional);
  if let Some(section) = section {
    let triggers = section.triggers.get_or_insert_with(PromptTriggers::default);
    trigger.set(triggers, value);
    persist(state)
  } else {
    Task::none()
  }
}

fn persist(state: &State) -> Task<Message> {
  match state.db.clone() {
    Some(db) => {
      let config = state.config.clone();
      Task::perform(
        async move {
          captains_log::save_prompt_config(&db, &config)
            .await
            .map_err(|err| err.to_string())
        },
        Message::Saved,
      )
    }
    None => Task::none(),
  }
}

/// Only matches `Free` sections, so callers that route through this (renaming the section, or
/// adding/editing/deleting/reordering its questions) silently no-op on the conditional section.
fn free_section_mut<'a>(config: &'a mut PromptConfig, section_id: &str) -> Option<&'a mut PromptSection> {
  config
    .sections
    .iter_mut()
    .find(|section| section.id == section_id && section.kind == PromptSectionKind::Free)
}

fn question_mut<'a>(
  config: &'a mut PromptConfig,
  section_id: &str,
  question_id: &str,
) -> Option<&'a mut PromptQuestion> {
  free_section_mut(config, section_id)?
    .questions
    .iter_mut()
    .find(|question| question.id == question_id)
}

fn edit_source(config: &PromptConfig, target: &EditTarget) -> String {
  match target {
    EditTarget::QuestionLabel(section_id, question_id) => config
      .sections
      .iter()
      .find(|section| section.id == *section_id)
      .and_then(|section| section.questions.iter().find(|question| question.id == *question_id))
      .map(|question| resolve(&question.label, &question.i18n_key))
      .unwrap_or_default(),
    EditTarget::QuestionPlaceholder(section_id, question_id) => config
      .sections
      .iter()
      .find(|section| section.id == *section_id)
      .and_then(|section| section.questions.iter().find(|question| question.id == *question_id))
      .map(|question| question.placeholder.clone())
      .unwrap_or_default(),
    EditTarget::SectionLabel(section_id) => config
      .sections
      .iter()
      .find(|section| section.id == *section_id)
      .map(|section| resolve(&section.label, &section.i18n_key))
      .unwrap_or_default(),
  }
}

fn resolve(label: &str, i18n_key: &str) -> String {
  if !label.is_empty() {
    label.to_owned()
  } else if !i18n_key.is_empty() {
    t!(i18n_key).into_owned()
  } else {
    String::new()
  }
}

fn swap<T>(items: &mut [T], from: usize, direction: Direction) -> bool {
  let to = from as isize + direction.delta();
  if to < 0 || to as usize >= items.len() {
    return false;
  }
  items.swap(from, to as usize);
  true
}

impl State {
  fn next_id(&mut self, prefix: &str) -> String {
    self.seq = self.seq.wrapping_add(1);
    let nanos = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_nanos())
      .unwrap_or(0);
    format!("{prefix}-{nanos}-{}", self.seq)
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  let header = panel_header();
  let body = scrollable(scroll_body(state))
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill);

  Column::with_children(vec![header, body.into()])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn panel_header<'a>() -> Element<'a, Message> {
  let title = text(t!("settings.captains_log.title"))
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(t!("settings.captains_log.panel_blurb"))
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![title.into(), container(blurb).max_width(640.0).into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let band = container(identity).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_3_5,
    left: PANEL_SIDE_PADDING,
  });

  Column::with_children(vec![band.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn scroll_body(state: &State) -> Element<'_, Message> {
  let count = state.config.sections.len();
  let is_default = state.config == PromptConfig::default();

  let mut children: Vec<Element<'_, Message>> = vec![section_head(is_default)];
  for (index, section) in state.config.sections.iter().enumerate() {
    children.push(section_card(state, section, index, count));
  }
  children.push(add_section_button());

  let inner = container(
    Column::with_children(children)
      .width(Length::Fill)
      .spacing(spacing::UNIT),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 0.0,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_6,
    left: PANEL_SIDE_PADDING,
  });

  inner.into()
}

fn section_head<'a>(is_default: bool) -> Element<'a, Message> {
  let eyebrow = text(t!("settings.captains_log.section_sections"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::accent()));
  let note = text(t!("settings.captains_log.section_sections_note"))
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()));
  let labels = Column::with_children(vec![
    eyebrow.into(),
    container(note).max_width(DESCRIPTION_MAX_WIDTH).into(),
  ])
  .spacing(spacing::UNIT + 2.0)
  .width(Length::Fill);

  let reset = Button::secondary(t!("settings.captains_log.reset"))
    .icon(Icon::reset())
    .size(Size::Sm)
    .on_press_maybe((!is_default).then_some(Message::Reset));

  let row = Row::with_children(vec![labels.into(), reset.into()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3_5);

  let band = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: 0.0,
    bottom: spacing::SPACE_3,
    left: 0.0,
  });

  Column::with_children(vec![band.into(), rule::horizontal_alpha(0.18)])
    .width(Length::Fill)
    .into()
}

fn section_card<'a>(state: &'a State, section: &'a PromptSection, index: usize, count: usize) -> Element<'a, Message> {
  let conditional = section.kind == PromptSectionKind::Conditional;
  let header = section_header(state, section, index, count, conditional);
  let body = if conditional {
    conditional_body(section)
  } else {
    free_body(state, section)
  };

  let card = Column::with_children(vec![header, body]).width(Length::Fill);

  container(card)
    .width(Length::Fill)
    .max_width(CARD_MAX_WIDTH)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: if conditional {
          color::with_alpha(color::status::WARNING, 0.28)
        } else {
          color::with_alpha(color::text::PRIMARY, 0.1)
        },
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn section_header<'a>(
  state: &'a State,
  section: &'a PromptSection,
  index: usize,
  count: usize,
  conditional: bool,
) -> Element<'a, Message> {
  let glyph = if conditional { Icon::spark() } else { Icon::journal() };
  let icon_color = if conditional {
    color::status::WARNING
  } else {
    color::accent()
  };

  let name: Element<'a, Message> = if conditional {
    text(resolve(&section.label, &section.i18n_key))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into()
  } else {
    editable_label(
      state,
      EditTarget::SectionLabel(section.id.clone()),
      resolve(&section.label, &section.i18n_key),
      super::i18n::tr_static("settings.captains_log.untitled_section"),
      super::i18n::tr_static("settings.captains_log.section_name_placeholder"),
      typography::body::MEDIUM,
      typography::size::MD,
    )
  };

  let mut children: Vec<Element<'a, Message>> = vec![
    glyph.size(17.0).color(icon_color).render(),
    container(name).width(Length::Fill).into(),
  ];
  if conditional {
    children.push(automatic_badge());
  }
  children.push(chevron(
    Message::MoveSection(section.id.clone(), Direction::Up),
    index > 0,
    Icon::chevron_up(),
  ));
  children.push(chevron(
    Message::MoveSection(section.id.clone(), Direction::Down),
    index + 1 < count,
    Icon::chevron_down(),
  ));
  children.push(mini_button(
    Icon::trash(),
    (!conditional).then(|| Message::DeleteSection(section.id.clone())),
    true,
  ));

  let row = Row::with_children(children)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(if conditional {
        color::with_alpha(color::status::WARNING, 0.05)
      } else {
        color::surface::SUNKEN
      })),
      ..container::Style::default()
    })
    .into()
}

fn conditional_body(section: &PromptSection) -> Element<'_, Message> {
  let triggers = section.triggers.unwrap_or_default();
  let note = text(t!("settings.captains_log.conditional_note"))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));

  let mut children: Vec<Element<'_, Message>> = vec![container(note).max_width(DESCRIPTION_MAX_WIDTH).into()];
  for (index, trigger) in TRIGGERS.into_iter().enumerate() {
    children.push(trigger_row(section, trigger, triggers, index + 1 < TRIGGERS.len()));
  }

  container(Column::with_children(children).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_4_5,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_4_5,
    })
    .into()
}

fn trigger_row(
  section: &PromptSection,
  trigger: Trigger,
  triggers: PromptTriggers,
  divider: bool,
) -> Element<'_, Message> {
  let on = trigger.get(&triggers);
  let label = text(t!(trigger.label_key()))
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let desc = text(t!(trigger.description_key()))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let labels = Column::with_children(vec![label.into(), container(desc).max_width(560.0).into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let row = Row::with_children(vec![
    labels.into(),
    toggle::toggle(on, Message::ToggleTrigger(section.id.clone(), trigger, !on)),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_3_5);

  let cell = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3_5,
    right: 0.0,
    bottom: spacing::SPACE_3_5,
    left: 0.0,
  });

  if divider {
    Column::with_children(vec![cell.into(), rule::horizontal_alpha(0.08)])
      .width(Length::Fill)
      .into()
  } else {
    cell.into()
  }
}

fn free_body<'a>(state: &'a State, section: &'a PromptSection) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = Vec::new();
  if section.questions.is_empty() {
    children.push(
      text(t!("settings.captains_log.no_questions"))
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  } else {
    let total = section.questions.len();
    for (index, question) in section.questions.iter().enumerate() {
      children.push(question_row(state, section, question, index, total));
    }
  }
  children.push(add_question_button(section));

  container(
    Column::with_children(children)
      .width(Length::Fill)
      .spacing(spacing::SPACE_2),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_4_5,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_4_5,
  })
  .into()
}

fn question_row<'a>(
  state: &'a State,
  section: &'a PromptSection,
  question: &'a PromptQuestion,
  index: usize,
  count: usize,
) -> Element<'a, Message> {
  let number = text(format!("{:02}", index + 1))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::tertiary()));

  let label = editable_label(
    state,
    EditTarget::QuestionLabel(section.id.clone(), question.id.clone()),
    resolve(&question.label, &question.i18n_key),
    super::i18n::tr_static("settings.captains_log.untitled_question"),
    super::i18n::tr_static("settings.captains_log.question_label_placeholder"),
    typography::body::MEDIUM,
    typography::size::MD,
  );
  let placeholder = editable_label(
    state,
    EditTarget::QuestionPlaceholder(section.id.clone(), question.id.clone()),
    question.placeholder.clone(),
    super::i18n::tr_static("settings.captains_log.add_placeholder"),
    super::i18n::tr_static("settings.captains_log.placeholder_placeholder"),
    typography::body::REGULAR,
    typography::size::SM,
  );

  let required = flag_row(
    question.required,
    Message::ToggleQuestionRequired(section.id.clone(), question.id.clone(), !question.required),
    "settings.captains_log.required",
    if question.required {
      color::status::WARNING
    } else {
      color::text::tertiary()
    },
    if question.required {
      t!("settings.captains_log.required_on").into_owned()
    } else {
      t!("settings.captains_log.required_off").into_owned()
    },
  );

  let objective = flag_row(
    question.links_to_objective,
    Message::ToggleQuestionObjective(section.id.clone(), question.id.clone(), !question.links_to_objective),
    "settings.captains_log.objective",
    if question.links_to_objective {
      color::accent()
    } else {
      color::text::tertiary()
    },
    if question.links_to_objective {
      t!("settings.captains_log.objective_on").into_owned()
    } else {
      t!("settings.captains_log.objective_off").into_owned()
    },
  );

  let fields = Column::with_children(vec![label, placeholder, required, objective])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let controls = Row::with_children(vec![
    chevron(
      Message::MoveQuestion(section.id.clone(), question.id.clone(), Direction::Up),
      index > 0,
      Icon::chevron_up(),
    ),
    chevron(
      Message::MoveQuestion(section.id.clone(), question.id.clone(), Direction::Down),
      index + 1 < count,
      Icon::chevron_down(),
    ),
    mini_button(
      Icon::trash(),
      Some(Message::DeleteQuestion(section.id.clone(), question.id.clone())),
      true,
    ),
  ])
  .spacing(spacing::UNIT);

  let row = Row::with_children(vec![
    container(number).width(Length::Fixed(20.0)).into(),
    fields.into(),
    controls.into(),
  ])
  .spacing(spacing::SPACE_2_5);

  let cell = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2_5,
    right: 0.0,
    bottom: spacing::SPACE_2_5,
    left: 0.0,
  });

  if index == 0 {
    cell.into()
  } else {
    Column::with_children(vec![rule::horizontal_alpha(0.08), cell.into()])
      .width(Length::Fill)
      .into()
  }
}

fn flag_row<'a>(
  on: bool,
  message: Message,
  tag_key: &'static str,
  tag_color: iced::Color,
  note: String,
) -> Element<'a, Message> {
  Row::with_children(vec![
    toggle::toggle(on, message),
    text(t!(tag_key))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(tag_color))
      .into(),
    text(note)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_2_5)
  .into()
}

fn editable_label<'a>(
  state: &'a State,
  target: EditTarget,
  value: String,
  empty_label: &'a str,
  placeholder: &'a str,
  font: iced::Font,
  size: f32,
) -> Element<'a, Message> {
  if let Some(editing) = state.editing.as_ref()
    && editing.target == target
  {
    return text_input(placeholder, &editing.draft)
      .font(font)
      .size(size)
      .padding(Padding {
        top: spacing::UNIT + 2.0,
        right: spacing::SPACE_2_5,
        bottom: spacing::UNIT + 2.0,
        left: spacing::SPACE_2_5,
      })
      .on_input(Message::EditChanged)
      .on_submit(Message::EditCommitted)
      .style(edit_input_style)
      .into();
  }

  let has_value = !value.is_empty();
  let display = if has_value { value } else { empty_label.to_owned() };
  let label_color = if has_value {
    color::text::PRIMARY
  } else {
    color::text::tertiary()
  };

  button(
    text(display)
      .font(font)
      .size(size)
      .style(typography::colored(label_color)),
  )
  .padding(Padding {
    top: spacing::UNIT + 2.0,
    right: spacing::SPACE_2_5,
    bottom: spacing::UNIT + 2.0,
    left: spacing::SPACE_2_5,
  })
  .width(Length::Fill)
  .on_press(Message::StartEdit(target))
  .style(|_, status| button::Style {
    background: matches!(status, button::Status::Hovered)
      .then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.02))),
    border: Border {
      color: match status {
        button::Status::Hovered => color::with_alpha(color::text::PRIMARY, 0.1),
        _ => iced::Color::TRANSPARENT,
      },
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..button::Style::default()
  })
  .into()
}

fn automatic_badge<'a>() -> Element<'a, Message> {
  container(
    text(t!("settings.captains_log.automatic_badge"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::status::WARNING)),
  )
  .padding(Padding {
    top: spacing::UNIT - 1.0,
    right: spacing::SPACE_2,
    bottom: spacing::UNIT - 1.0,
    left: spacing::SPACE_2,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.1))),
    border: Border {
      color: color::with_alpha(color::status::WARNING, 0.3),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn add_question_button(section: &PromptSection) -> Element<'_, Message> {
  container(
    Button::secondary(t!("settings.captains_log.add_question"))
      .icon(Icon::plus())
      .size(Size::Sm)
      .on_press(Message::AddQuestion(section.id.clone())),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    right: 0.0,
    bottom: 0.0,
    left: 0.0,
  })
  .into()
}

fn add_section_button<'a>() -> Element<'a, Message> {
  container(
    Button::secondary(t!("settings.captains_log.add_section"))
      .icon(Icon::plus())
      .on_press(Message::AddSection),
  )
  .padding(Padding {
    top: spacing::SPACE_3,
    right: 0.0,
    bottom: 0.0,
    left: 0.0,
  })
  .into()
}

fn chevron<'a>(message: Message, enabled: bool, icon: Icon) -> Element<'a, Message> {
  mini_button(icon, enabled.then_some(message), false)
}

fn mini_button<'a>(icon: Icon, message: Option<Message>, danger: bool) -> Element<'a, Message> {
  let icon_color = if message.is_some() {
    color::text::secondary()
  } else {
    color::with_alpha(color::text::PRIMARY, 0.25)
  };

  button(
    container(icon.size(14.0).color(icon_color).render())
      .center_x(Length::Fill)
      .center_y(Length::Fill),
  )
  .width(Length::Fixed(MINI_BUTTON_SIZE))
  .height(Length::Fixed(MINI_BUTTON_SIZE))
  .padding(Padding::ZERO)
  .on_press_maybe(message)
  .style(move |_, status| {
    let border_color = match status {
      button::Status::Hovered if danger => color::with_alpha(color::status::DANGER, 0.5),
      button::Status::Hovered => color::with_alpha(color::text::PRIMARY, 0.3),
      _ => color::with_alpha(color::text::PRIMARY, 0.1),
    };
    button::Style {
      background: Some(Background::Color(iced::Color::TRANSPARENT)),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn edit_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: color::accent(),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent(), 0.4),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  async fn tab() -> State {
    let db = store::open_test().await.unwrap();
    State::new(db)
  }

  fn free_section_id(state: &State) -> String {
    state
      .config
      .sections
      .iter()
      .find(|section| section.kind == PromptSectionKind::Free)
      .unwrap()
      .id
      .clone()
  }

  async fn reload(state: &State) -> PromptConfig {
    let db = state.db.clone().unwrap();
    captains_log::load_prompt_config(&db).await.unwrap()
  }

  mod rename_section {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_writes_a_literal_label_and_persists() {
      let mut state = tab().await;
      let section_id = free_section_id(&state);

      let _ = update(
        &mut state,
        Message::StartEdit(EditTarget::SectionLabel(section_id.clone())),
      );
      let _ = update(&mut state, Message::EditChanged("Morning".to_owned()));
      let _ = update(&mut state, Message::EditCommitted);

      let section = state.config.sections.iter().find(|s| s.id == section_id).unwrap();
      assert_eq!(section.label, "Morning");
      assert!(
        section.i18n_key.is_empty(),
        "editing clears the i18n key so the label is literal"
      );

      captains_log::save_prompt_config(&state.db.clone().unwrap(), state.config())
        .await
        .unwrap();
      assert_eq!(reload(&state).await, *state.config());
    }

    #[tokio::test]
    async fn it_refuses_to_rename_the_conditional_section() {
      let mut state = tab().await;

      let _ = update(
        &mut state,
        Message::StartEdit(EditTarget::SectionLabel(CONDITIONAL_ID.to_owned())),
      );
      let _ = update(&mut state, Message::EditChanged("Nope".to_owned()));
      let _ = update(&mut state, Message::EditCommitted);

      let conditional = state
        .config
        .sections
        .iter()
        .find(|s| s.kind == PromptSectionKind::Conditional)
        .unwrap();
      assert!(
        conditional.label.is_empty(),
        "the conditional label stays its translated default"
      );
    }
  }

  mod start_edit {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn state_with_question() -> (State, String, String) {
      let mut state = tab().await;
      let section_id = free_section_id(&state);
      let _ = update(&mut state, Message::AddQuestion(section_id.clone()));
      let question_id = state
        .config
        .sections
        .iter()
        .find(|section| section.id == section_id)
        .and_then(|section| section.questions.last())
        .unwrap()
        .id
        .clone();
      (state, section_id, question_id)
    }

    #[tokio::test]
    async fn it_seeds_the_draft_from_a_question_label() {
      let (mut state, section_id, question_id) = state_with_question().await;
      let expected = state
        .config
        .sections
        .iter()
        .find(|section| section.id == section_id)
        .and_then(|section| section.questions.iter().find(|question| question.id == question_id))
        .map(|question| question.label.clone())
        .unwrap();

      let _ = update(
        &mut state,
        Message::StartEdit(EditTarget::QuestionLabel(section_id, question_id)),
      );

      assert_eq!(state.editing.as_ref().unwrap().draft, expected);
    }

    #[tokio::test]
    async fn it_seeds_the_draft_from_a_question_placeholder() {
      let (mut state, section_id, question_id) = state_with_question().await;

      let _ = update(
        &mut state,
        Message::StartEdit(EditTarget::QuestionPlaceholder(section_id, question_id)),
      );

      assert_eq!(state.editing.as_ref().unwrap().draft, "");
    }

    #[tokio::test]
    async fn it_seeds_an_empty_draft_for_a_missing_target() {
      let mut state = tab().await;

      let _ = update(
        &mut state,
        Message::StartEdit(EditTarget::QuestionLabel("missing".to_owned(), "missing".to_owned())),
      );

      assert_eq!(state.editing.as_ref().unwrap().draft, "");
    }
  }

  mod result_messages {
    use super::*;

    #[tokio::test]
    async fn it_clears_the_error_when_a_config_loads() {
      let mut state = tab().await;
      state.error = Some("stale".to_owned());

      let _ = update(&mut state, Message::Loaded(Ok(PromptConfig::default())));

      assert!(state.error.is_none());
    }

    #[tokio::test]
    async fn it_records_a_load_error() {
      let mut state = tab().await;

      let _ = update(&mut state, Message::Loaded(Err("load failed".to_owned())));

      assert_eq!(state.error.as_deref(), Some("load failed"));
    }

    #[tokio::test]
    async fn it_records_a_save_error() {
      let mut state = tab().await;

      let _ = update(&mut state, Message::Saved(Err("save failed".to_owned())));

      assert_eq!(state.error.as_deref(), Some("save failed"));
    }

    #[tokio::test]
    async fn it_ignores_an_edit_change_without_an_active_edit() {
      let mut state = tab().await;

      let _ = update(&mut state, Message::EditChanged("orphan".to_owned()));

      assert!(state.editing.is_none());
    }
  }

  mod questions {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn add_question_appends_a_free_question_and_persists() {
      let mut state = tab().await;
      let section_id = free_section_id(&state);
      let before = state
        .config
        .sections
        .iter()
        .find(|s| s.id == section_id)
        .unwrap()
        .questions
        .len();

      let _ = update(&mut state, Message::AddQuestion(section_id.clone()));

      let after = state.config.sections.iter().find(|s| s.id == section_id).unwrap();
      assert_eq!(after.questions.len(), before + 1);

      captains_log::save_prompt_config(&state.db.clone().unwrap(), state.config())
        .await
        .unwrap();
      assert_eq!(reload(&state).await, *state.config());
    }

    #[tokio::test]
    async fn toggle_required_flips_the_flag() {
      let mut state = tab().await;
      let section_id = free_section_id(&state);
      let question_id = state
        .config
        .sections
        .iter()
        .find(|s| s.id == section_id)
        .unwrap()
        .questions
        .last()
        .unwrap()
        .id
        .clone();

      let _ = update(
        &mut state,
        Message::ToggleQuestionRequired(section_id.clone(), question_id.clone(), true),
      );

      let question = question_mut(&mut state.config, &section_id, &question_id).unwrap();
      assert!(question.required);
    }

    #[tokio::test]
    async fn toggle_objective_flips_the_flag_and_persists() {
      let mut state = tab().await;
      let section_id = free_section_id(&state);
      let question_id = state
        .config
        .sections
        .iter()
        .find(|s| s.id == section_id)
        .unwrap()
        .questions
        .last()
        .unwrap()
        .id
        .clone();

      let _ = update(
        &mut state,
        Message::ToggleQuestionObjective(section_id.clone(), question_id.clone(), true),
      );

      assert!(
        question_mut(&mut state.config, &section_id, &question_id)
          .unwrap()
          .links_to_objective
      );

      captains_log::save_prompt_config(&state.db.clone().unwrap(), state.config())
        .await
        .unwrap();
      assert_eq!(reload(&state).await, *state.config());
    }

    #[tokio::test]
    async fn delete_question_removes_it() {
      let mut state = tab().await;
      let section_id = free_section_id(&state);
      let question_id = state
        .config
        .sections
        .iter()
        .find(|s| s.id == section_id)
        .unwrap()
        .questions[0]
        .id
        .clone();

      let _ = update(
        &mut state,
        Message::DeleteQuestion(section_id.clone(), question_id.clone()),
      );

      let section = state.config.sections.iter().find(|s| s.id == section_id).unwrap();
      assert!(section.questions.iter().all(|q| q.id != question_id));
    }

    #[tokio::test]
    async fn move_question_down_swaps_order() {
      let mut state = tab().await;
      let section_id = free_section_id(&state);
      let first = state
        .config
        .sections
        .iter()
        .find(|s| s.id == section_id)
        .unwrap()
        .questions[0]
        .id
        .clone();

      let _ = update(
        &mut state,
        Message::MoveQuestion(section_id.clone(), first.clone(), Direction::Down),
      );

      let section = state.config.sections.iter().find(|s| s.id == section_id).unwrap();
      assert_eq!(section.questions[1].id, first);
    }
  }

  mod sections {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn add_section_appends_a_free_section() {
      let mut state = tab().await;
      let before = state.config.sections.len();

      let _ = update(&mut state, Message::AddSection);

      assert_eq!(state.config.sections.len(), before + 1);
      assert_eq!(state.config.sections.last().unwrap().kind, PromptSectionKind::Free);
    }

    #[tokio::test]
    async fn delete_section_refuses_the_conditional_card() {
      let mut state = tab().await;

      let _ = update(&mut state, Message::DeleteSection(CONDITIONAL_ID.to_owned()));

      assert!(
        state
          .config
          .sections
          .iter()
          .any(|s| s.kind == PromptSectionKind::Conditional),
        "the conditional section cannot be deleted"
      );
    }

    #[tokio::test]
    async fn delete_section_removes_a_free_card() {
      let mut state = tab().await;
      let section_id = free_section_id(&state);

      let _ = update(&mut state, Message::DeleteSection(section_id.clone()));

      assert!(state.config.sections.iter().all(|s| s.id != section_id));
    }

    #[tokio::test]
    async fn move_section_down_reorders() {
      let mut state = tab().await;
      let first = state.config.sections[0].id.clone();

      let _ = update(&mut state, Message::MoveSection(first.clone(), Direction::Down));

      assert_eq!(state.config.sections[1].id, first);
    }
  }

  mod triggers {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn toggle_trigger_disables_a_conditional_prompt_and_persists() {
      let mut state = tab().await;
      let conditional_id = state
        .config
        .sections
        .iter()
        .find(|s| s.kind == PromptSectionKind::Conditional)
        .unwrap()
        .id
        .clone();

      let _ = update(
        &mut state,
        Message::ToggleTrigger(conditional_id.clone(), Trigger::Combat, false),
      );

      let triggers = state
        .config
        .sections
        .iter()
        .find(|s| s.id == conditional_id)
        .unwrap()
        .triggers
        .unwrap();
      assert!(!triggers.combat);
      assert!(triggers.build && triggers.skill, "only the combat trigger flips");

      captains_log::save_prompt_config(&state.db.clone().unwrap(), state.config())
        .await
        .unwrap();
      assert_eq!(reload(&state).await, *state.config());
    }
  }

  mod reset {
    use pretty_assertions::{assert_eq, assert_ne};

    use super::*;

    #[tokio::test]
    async fn reset_to_defaults_restores_the_shipped_structure() {
      let mut state = tab().await;
      let _ = update(&mut state, Message::AddSection);
      assert_ne!(state.config, PromptConfig::default());

      let _ = reset_to_defaults(&mut state);

      assert_eq!(state.config, PromptConfig::default());
    }
  }

  mod badge {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_free_questions() {
      let state = tab().await;

      assert_eq!(badge(&state), "5");
    }
  }

  mod view {
    use super::*;

    #[tokio::test]
    async fn it_renders_the_panel() {
      let state = tab().await;
      let _el: Element<'_, Message> = view(&state);
    }

    #[tokio::test]
    async fn it_renders_while_editing_a_label() {
      let mut state = tab().await;
      let section_id = free_section_id(&state);
      let _ = update(&mut state, Message::StartEdit(EditTarget::SectionLabel(section_id)));

      let _el: Element<'_, Message> = view(&state);
    }
  }
}
