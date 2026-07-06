use iced::{
  Background, Border, ContentFit, Element, Length, Padding, Task,
  alignment::Vertical,
  widget::{Column, Row, Space, container, image, text, text_editor},
};

use super::{Message as Parent, km_report, prompts::Completeness, rollup_tiles};
use crate::{
  store::{
    Database,
    images::IconResolution,
    model::CaptainsLog,
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
const CORE: [AnswerKey; 3] = [AnswerKey::Goal, AnswerKey::Remember, AnswerKey::Blocked];
const EDITOR_HEIGHT: f32 = 80.0;
const EDITOR_PADDING: f32 = 11.0;
const ENGAGEMENT_TILE: f32 = 42.0;
const HERO_BAR_WIDTH: f32 = 3.0;
const HERO_PADDING_X: f32 = 26.0;
const HERO_PADDING_Y: f32 = 20.0;
const HERO_TEXT_SIZE: f32 = 22.0;
const REST: [AnswerKey; 4] = [AnswerKey::Build, AnswerKey::Skill, AnswerKey::Next, AnswerKey::Research];

#[derive(Clone, Debug)]
pub enum Message {
  Cancelled,
  DraftEdited(text_editor::Action),
  EditRequested(AnswerKey),
  Report(usize, km_report::Message),
  SaveRequested(AnswerKey),
  Saved(AnswerKey, Result<Option<String>, String>),
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
  editing: Option<AnswerKey>,
  log: Option<CaptainsLog>,
}

impl State {
  pub fn new(date: String, log: Option<CaptainsLog>, completeness: Completeness, engagements: Vec<Engagement>) -> Self {
    State {
      completeness,
      date,
      debriefs: engagements.into_iter().map(Debrief::new).collect(),
      draft: text_editor::Content::new(),
      editing: None,
      log,
    }
  }

  fn is_missing(&self, key: AnswerKey) -> bool {
    self.completeness.missing_prompts.contains(&key)
  }

  fn missing_debrief(&self, character_id: i64, killmail_id: i64) -> bool {
    self
      .completeness
      .missing_debriefs
      .iter()
      .any(|loss| loss.character_id == character_id && loss.killmail_id == killmail_id)
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

pub(super) fn update_pane(state: &mut State, db: &Database, message: Message) -> Task<Parent> {
  match message {
    Message::Cancelled => state.editing = None,
    Message::DraftEdited(action) => state.draft.perform(action),
    Message::EditRequested(key) => begin_edit(state, key),
    Message::Report(index, message) => return route_report(state, index, db, message),
    Message::SaveRequested(key) => return save_requested(state, key, db),
    Message::Saved(key, result) => apply_saved(state, key, result),
  }

  Task::none()
}

pub(super) fn view_pane<'a>(state: &'a State, summary: &rollup_tiles::Summary) -> Element<'a, Parent> {
  Column::with_children(vec![
    narrative_block(state),
    rollup_tiles::render(summary),
    entry_block(state),
  ])
  .spacing(spacing::SPACE_6)
  .width(Length::Fill)
  .into()
}

fn apply_saved(state: &mut State, key: AnswerKey, result: Result<Option<String>, String>) {
  match result {
    Ok(value) => {
      set_answer(&mut state.log, &state.date, key, value);
      state.editing = None;
    }
    Err(error) => {
      tracing::warn!(target: "pod::captains_log", %error, "past answer save failed")
    }
  }
}

fn begin_edit(state: &mut State, key: AnswerKey) {
  let current = answer_of(state.log.as_ref(), key).unwrap_or_default();
  state.draft = text_editor::Content::with_text(current);
  state.editing = Some(key);
}

fn route_report(state: &mut State, index: usize, db: &Database, message: km_report::Message) -> Task<Parent> {
  match state.debriefs.get_mut(index) {
    Some(debrief) => km_report::update(&mut debrief.report, message, db)
      .map(move |message| Parent::Past(Message::Report(index, message))),
    None => Task::none(),
  }
}

fn save_requested(state: &mut State, key: AnswerKey, db: &Database) -> Task<Parent> {
  let value = non_blank(&state.draft.text());
  let db = db.clone();
  let date = state.date.clone();

  Task::perform(
    async move {
      captains_log::upsert_answer(&db, &date, key, value.as_deref())
        .await
        .map(|()| value)
        .map_err(|error| error.to_string())
    },
    move |result| Message::Saved(key, result),
  )
  .map(Parent::Past)
}

fn answer_of(log: Option<&CaptainsLog>, key: AnswerKey) -> Option<&str> {
  let log = log?;
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

  value.as_deref().filter(|text| !text.trim().is_empty())
}

fn field_label(key: AnswerKey) -> String {
  let name = match key {
    AnswerKey::Blocked => "captains_log.past.label_blocked",
    AnswerKey::Build => "captains_log.past.label_build",
    AnswerKey::Combat => "captains_log.past.label_combat",
    AnswerKey::Goal => "captains_log.past.label_goal",
    AnswerKey::Next => "captains_log.past.label_next",
    AnswerKey::Remember => "captains_log.past.label_remember",
    AnswerKey::Research => "captains_log.past.label_research",
    AnswerKey::Skill => "captains_log.past.label_skill",
  };

  t!(name).into_owned()
}

fn non_blank(value: &str) -> Option<String> {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    None
  } else {
    Some(trimmed.to_owned())
  }
}

fn set_answer(log: &mut Option<CaptainsLog>, date: &str, key: AnswerKey, value: Option<String>) {
  let entry = log.get_or_insert_with(|| CaptainsLog {
    date: date.to_owned(),
    ..CaptainsLog::default()
  });

  match key {
    AnswerKey::Blocked => entry.blocked = value,
    AnswerKey::Build => entry.build = value,
    AnswerKey::Combat => entry.combat = value,
    AnswerKey::Goal => entry.goal = value,
    AnswerKey::Next => entry.next = value,
    AnswerKey::Remember => entry.remember = value,
    AnswerKey::Research => entry.research = value,
    AnswerKey::Skill => entry.skill = value,
  }
}

fn narrative_block(state: &State) -> Element<'_, Parent> {
  match state
    .log
    .as_ref()
    .and_then(|log| non_blank(log.narrative().as_deref().unwrap_or("")))
  {
    Some(narrative) => narrative_hero(narrative),
    None => narrative_empty(),
  }
}

fn narrative_hero<'a>(narrative: String) -> Element<'a, Parent> {
  let kicker = Row::with_children(vec![
    Icon::journal().size(15.0).color(color::accent()).render::<Parent>(),
    eyebrow_text(&t!("captains_log.narrative.kicker"), Some(color::accent())).into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let quote = text(format!("\u{201c}{narrative}\u{201d}"))
    .font(typography::body::REGULAR)
    .size(HERO_TEXT_SIZE)
    .style(typography::colored(color::text::PRIMARY));

  let body = Column::with_children(vec![kicker.into(), quote.into()])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill);

  hero_shell(true, body.into())
}

fn narrative_empty<'a>() -> Element<'a, Parent> {
  let kicker = Row::with_children(vec![
    Icon::journal()
      .size(15.0)
      .color(color::text::secondary())
      .render::<Parent>(),
    eyebrow_text(&t!("captains_log.narrative.kicker"), Some(color::text::secondary())).into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let empty = text(t!("captains_log.past.no_narrative").into_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::tertiary()));

  let body = Column::with_children(vec![kicker.into(), empty.into()])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill);

  hero_shell(false, body.into())
}

fn hero_shell<'a>(active: bool, body: Element<'a, Parent>) -> Element<'a, Parent> {
  let (background, border, bar) = if active {
    (
      color::with_alpha(color::accent(), 0.1),
      color::with_alpha(color::accent(), 0.4),
      color::accent(),
    )
  } else {
    (color::surface::RAISED, color::rule(), color::rule())
  };

  let accent_bar = container(Space::new())
    .width(Length::Fixed(HERO_BAR_WIDTH))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(bar)),
      ..container::Style::default()
    });

  let content = container(body).width(Length::Fill).padding(Padding {
    top: HERO_PADDING_Y,
    right: HERO_PADDING_X,
    bottom: HERO_PADDING_Y,
    left: HERO_PADDING_X,
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

fn entry_block(state: &State) -> Element<'_, Parent> {
  Column::with_children(vec![entry_header(state), entry_card(state)])
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

fn entry_card(state: &State) -> Element<'_, Parent> {
  let mut children: Vec<Element<'_, Parent>> = CORE.iter().map(|key| field_row(state, *key)).collect();

  for key in REST {
    if answer_of(state.log.as_ref(), key).is_some() {
      children.push(field_row(state, key));
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

fn field_row(state: &State, key: AnswerKey) -> Element<'_, Parent> {
  if state.editing == Some(key) {
    field_editor(state, key)
  } else {
    field_display(state, key)
  }
}

fn field_display(state: &State, key: AnswerKey) -> Element<'_, Parent> {
  let missing = state.is_missing(key);
  let value = answer_of(state.log.as_ref(), key);

  let mut head: Vec<Element<'_, Parent>> = vec![
    text(field_label(key))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ];
  if missing {
    head.push(missing_badge());
  }
  head.push(Space::new().width(Length::Fill).into());
  if value.is_some() {
    head.push(
      Button::secondary(t!("captains_log.past.edit").into_owned())
        .size(Size::Sm)
        .icon(Icon::pencil())
        .on_press(Parent::Past(Message::EditRequested(key)))
        .into(),
    );
  }

  let head = Row::with_children(head)
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

  let body: Element<'_, Parent> = match value {
    Some(text_value) => text(text_value.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    None => cta_button(missing, key),
  };

  field_shell(vec![head.into(), body])
}

fn field_editor(state: &State, key: AnswerKey) -> Element<'_, Parent> {
  let head = text(field_label(key))
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
      .on_press_maybe(can_save.then_some(Parent::Past(Message::SaveRequested(key))))
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

fn cta_button<'a>(missing: bool, key: AnswerKey) -> Element<'a, Parent> {
  let label = if missing {
    t!("captains_log.past.add_now_missing")
  } else {
    t!("captains_log.past.add_now")
  };

  Button::ghost(label.into_owned())
    .size(Size::Sm)
    .on_press(Parent::Past(Message::EditRequested(key)))
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
  use crate::features::roster::captains_log::prompts::LossEngagement;

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
    State::new("2026-07-05".to_owned(), log, completeness, engagements)
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

      assert_eq!(answer_of(Some(&log), AnswerKey::Goal), Some("Spin up the barge line."));
    }

    #[test]
    fn it_drops_a_blank_answer() {
      let log = CaptainsLog {
        goal: Some("   ".to_owned()),
        ..CaptainsLog::default()
      };

      assert_eq!(answer_of(Some(&log), AnswerKey::Goal), None);
      assert_eq!(answer_of(None, AnswerKey::Goal), None);
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
      let mut log = None;

      set_answer(&mut log, "2026-07-05", AnswerKey::Goal, Some("First goal.".to_owned()));

      let row = log.expect("row should be created");
      assert_eq!(row.date(), "2026-07-05");
      assert_eq!(row.goal().as_deref(), Some("First goal."));
    }

    #[test]
    fn it_clears_an_answer_to_none() {
      let mut log = Some(log_with_goal());

      set_answer(&mut log, "2026-07-05", AnswerKey::Goal, None);

      assert_eq!(log.unwrap().goal(), &None);
    }
  }

  mod begin_edit {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_seeds_the_draft_from_the_current_answer() {
      let mut state = state_with(Some(log_with_goal()), Completeness::default(), Vec::new());

      begin_edit(&mut state, AnswerKey::Goal);

      assert_eq!(state.editing, Some(AnswerKey::Goal));
      assert_eq!(state.draft.text().trim(), "Spin up the barge line.");
    }

    #[test]
    fn it_opens_an_empty_draft_for_an_unanswered_field() {
      let mut state = state_with(None, Completeness::default(), Vec::new());

      begin_edit(&mut state, AnswerKey::Blocked);

      assert_eq!(state.editing, Some(AnswerKey::Blocked));
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
      begin_edit(&mut state, AnswerKey::Goal);

      let _ = update_pane(&mut state, &db, Message::Cancelled);

      assert_eq!(state.editing, None);
    }

    #[tokio::test]
    async fn it_installs_a_saved_answer_and_leaves_edit_mode() {
      let db = store::open_test().await.unwrap();
      let mut state = state_with(None, Completeness::default(), Vec::new());
      begin_edit(&mut state, AnswerKey::Goal);

      let _ = update_pane(
        &mut state,
        &db,
        Message::Saved(AnswerKey::Goal, Ok(Some("Run one Tama roam.".to_owned()))),
      );

      assert_eq!(state.editing, None);
      assert_eq!(
        answer_of(state.log.as_ref(), AnswerKey::Goal),
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
  }

  mod view_pane {
    use super::*;

    fn missing_goal() -> Completeness {
      Completeness {
        missing_debriefs: Vec::new(),
        missing_prompts: vec![AnswerKey::Goal],
      }
    }

    #[test]
    fn it_renders_an_activity_only_day_with_no_log() {
      let state = state_with(None, missing_goal(), Vec::new());
      let summary = rollup_tiles::Summary::empty();

      let _el: Element<'_, Parent> = view_pane(&state, &summary);
    }

    #[test]
    fn it_renders_a_logged_day_with_a_narrative_and_debriefs() {
      let log = CaptainsLog {
        narrative: Some("One frigate saved the fleet.".to_owned()),
        ..log_with_goal()
      };
      let completeness = Completeness {
        missing_debriefs: vec![LossEngagement {
          character_id: 4,
          killmail_id: 100,
        }],
        missing_prompts: Vec::new(),
      };
      let state = state_with(Some(log), completeness, vec![engagement(4, 100, false)]);
      let summary = rollup_tiles::Summary::empty();

      let _el: Element<'_, Parent> = view_pane(&state, &summary);
    }

    #[test]
    fn it_renders_a_field_in_edit_mode() {
      let mut state = state_with(Some(log_with_goal()), missing_goal(), Vec::new());
      begin_edit(&mut state, AnswerKey::Goal);

      let _el: Element<'_, Parent> = view_pane(&state, &rollup_tiles::Summary::empty());
    }
  }
}
