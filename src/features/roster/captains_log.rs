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

use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use iced::{
  Element, Length, Padding, Task,
  alignment::Horizontal,
  widget::{Column, Row, Space, container, scrollable},
};

use crate::{
  clients::eve_image::Size,
  store::{
    Database, images,
    images::{IconResolution, ImageKind},
    model::SkillCompletion,
    repo::{
      calendar_event_note, captains_log,
      captains_log_rollup::{CalendarEntry, CombatKill, DayMoney, IndustryDelivery, NetWorthDelta},
      character, killmail_report, sde,
    },
  },
  ui::{
    components::{
      eyebrow::eyebrow, modal_overlay::stable_overlay, positioned_dropdown::positioned_dropdown_right, rule,
    },
    style::{color, control, spacing},
  },
};

const ENGAGEMENT_ICON_SIZE: Size = Size::S64;
const ENTRIES_WIDTH: f32 = 276.0;
const JUMP_OVERLAY_RIGHT: f32 = 28.0;
const JUMP_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;
const MAIN_MAX_WIDTH: f32 = 940.0;

#[derive(Clone, Debug)]
pub enum Message {
  Entries(entries::Message),
  Events(events::Message),
  Exit,
  Header(header::Message),
  Loaded(Box<Snapshot>),
  Narrative(narrative::Message),
  Past(past::Message),
  Wizard(wizard::Message),
}

impl Message {
  pub fn loads_data(&self) -> bool {
    matches!(self, Message::Loaded(_))
  }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
  days: Vec<Day>,
  event_notes: HashMap<i64, String>,
  event_owners: HashMap<i64, i64>,
  today_date: NaiveDate,
}

impl Snapshot {
  #[cfg(test)]
  fn empty() -> Self {
    Snapshot {
      days: Vec::new(),
      event_notes: HashMap::new(),
      event_owners: HashMap::new(),
      today_date: Utc::now().date_naive(),
    }
  }
}

pub struct State {
  days: Vec<Day>,
  event_editing: Option<events::Editing>,
  event_notes: HashMap<i64, String>,
  event_owners: HashMap<i64, i64>,
  jump_open: bool,
  loading: bool,
  narrative: narrative::State,
  past: Option<past::State>,
  selected: Option<String>,
  today_date: NaiveDate,
  wizard: wizard::State,
}

impl State {
  pub fn new() -> Self {
    let today = Utc::now().date_naive();
    State {
      days: Vec::new(),
      event_editing: None,
      event_notes: HashMap::new(),
      event_owners: HashMap::new(),
      jump_open: false,
      loading: true,
      narrative: narrative::State::new(None),
      past: None,
      selected: None,
      today_date: today,
      wizard: empty_wizard(),
    }
  }

  pub fn stale_images(&self) -> Vec<(ImageKind, i64)> {
    Vec::new()
  }

  fn day_of(&self, iso: &str) -> Option<&Day> {
    self.days.iter().find(|day| day.date_iso == iso)
  }

  fn today_day(&self) -> Option<&Day> {
    let iso = iso_of(self.today_date);
    self.day_of(&iso)
  }
}

#[derive(Clone, Debug)]
struct Day {
  activity: prompts::DayActivity,
  completeness: prompts::Completeness,
  date_iso: String,
  engagements: Vec<EngagementData>,
  events: Vec<CalendarEntry>,
  industry: Vec<String>,
  kill_count: usize,
  log: Option<crate::store::model::CaptainsLog>,
  loss_count: usize,
  loss_value: f64,
  money: DayMoney,
  narrative: Option<String>,
  net_worth: Option<NetWorthDelta>,
  pilot_count: usize,
  skill_count: usize,
  skills: Vec<(String, i64)>,
}

#[derive(Clone, Debug)]
struct EngagementData {
  character_id: i64,
  character_name: String,
  icon: IconResolution,
  is_kill: bool,
  killmail_id: i64,
  ship: String,
  system: String,
  value: f64,
}

pub fn load(db: &Database, character_ids: Vec<i64>) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move { Box::new(build_snapshot(&db, &character_ids).await) },
    Message::Loaded,
  )
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::Entries(entries::Message::Selected(day)) => select_day(state, db, day),
    Message::Events(msg) => route_events(state, db, msg),
    Message::Exit => Task::none(),
    Message::Header(header::Message::JumpToDay) => toggle_jump(state),
    Message::Loaded(snapshot) => install_snapshot(state, db, *snapshot),
    Message::Narrative(msg) => route_narrative(state, db, msg),
    Message::Past(msg) => route_past(state, db, msg),
    Message::Wizard(msg) => route_wizard(state, db, msg),
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  if state.loading {
    return status_view(&t!("captains_log.loading"));
  }
  if state.days.is_empty() {
    return status_view(&t!("captains_log.empty"));
  }

  let main = scrollable(
    container(main_body(state))
      .width(Length::Fill)
      .max_width(MAIN_MAX_WIDTH)
      .padding(Padding {
        top: 26.0,
        right: 34.0,
        bottom: 60.0,
        left: 34.0,
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(control::scrollbar);

  let panes = Row::with_children(vec![
    container(entries::render(&log_of(state), state.selected.as_deref()))
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

  stable_overlay(base.into(), overlay_layers(state))
}

pub fn escape_dismiss(state: &State) -> Option<Message> {
  if state.jump_open {
    return Some(Message::Header(header::Message::JumpToDay));
  }
  Some(Message::Exit)
}

async fn build_day(db: &Database, character_ids: &[i64], iso: &str) -> Day {
  let log = captains_log::get(db, iso).await.ok().flatten();
  let reports = killmail_report::list_for_day(db, character_ids, iso)
    .await
    .unwrap_or_default();

  let rollup = rollup::for_date(db, iso).await.ok();
  let combat = rollup
    .as_ref()
    .map(|day| day.combat.clone())
    .unwrap_or_else(empty_combat);
  let industry_rows = rollup.as_ref().map(|day| day.industry.clone()).unwrap_or_default();
  let skill_rows = rollup.as_ref().map(|day| day.skills.clone()).unwrap_or_default();
  let events = rollup.as_ref().map(|day| day.events.clone()).unwrap_or_default();
  let money = rollup.as_ref().map(|day| day.money).unwrap_or_default();
  let net_worth = rollup.as_ref().and_then(|day| day.net_worth);

  let engagements = build_engagements(db, &combat.engagements).await;
  let industry = resolve_industry(db, &industry_rows).await;
  let skills = resolve_skills(db, &skill_rows).await;

  let activity = day_activity(&combat, industry_rows.len(), skill_rows.len());
  let completeness = prompts::completeness(&activity, log.as_ref(), &reports);
  let pilot_count = pilot_count(&combat, &industry_rows, &skill_rows, character_ids);
  let narrative = log
    .as_ref()
    .and_then(|entry| non_blank(entry.narrative().as_deref().unwrap_or("")));

  Day {
    activity,
    completeness,
    date_iso: iso.to_owned(),
    engagements,
    events,
    industry,
    kill_count: combat.kill_count,
    log,
    loss_count: combat.loss_count,
    loss_value: combat.loss_value,
    money,
    narrative,
    net_worth,
    pilot_count,
    skill_count: skills.len(),
    skills,
  }
}

async fn build_engagements(db: &Database, kills: &[CombatKill]) -> Vec<EngagementData> {
  let mut out = Vec::with_capacity(kills.len());
  for kill in kills {
    out.push(EngagementData {
      character_id: kill.character_id,
      character_name: character_name(db, kill.character_id).await,
      icon: images::default_store().resolve_type_icon(kill.ship_type_id, None, ENGAGEMENT_ICON_SIZE),
      is_kill: kill.is_kill,
      killmail_id: kill.killmail_id,
      ship: type_name(db, kill.ship_type_id).await,
      system: system_name(db, kill.system_id).await,
      value: kill.value_isk,
    });
  }
  out
}

async fn build_snapshot(db: &Database, character_ids: &[i64]) -> Snapshot {
  let today = Utc::now().date_naive();
  let today_iso = iso_of(today);

  let logged = captains_log::dates(db).await.unwrap_or_default();
  let active = rollup::active_dates(db).await.unwrap_or_default();
  let mut day_isos = entries::merged_days(logged, active);
  // Today must always render, even with zero logged/rollup activity, so the entry is there to log against.
  if !day_isos.iter().any(|iso| iso == &today_iso) {
    day_isos.insert(0, today_iso);
  }

  let mut days = Vec::with_capacity(day_isos.len());
  for iso in &day_isos {
    days.push(build_day(db, character_ids, iso).await);
  }

  let (event_notes, event_owners) = load_event_notes(db, &days).await;

  Snapshot {
    days,
    event_notes,
    event_owners,
    today_date: today,
  }
}

async fn character_name(db: &Database, id: i64) -> String {
  character::get(db, id)
    .await
    .ok()
    .flatten()
    .map(|character| character.name().to_owned())
    .unwrap_or_else(|| t!("roster.fallback.pilot", id => id).into_owned())
}

fn day_activity(combat: &rollup::Combat, industry_count: usize, skill_count: usize) -> prompts::DayActivity {
  let losses = combat
    .engagements
    .iter()
    .filter(|kill| !kill.is_kill)
    .map(|kill| prompts::LossEngagement {
      character_id: kill.character_id,
      killmail_id: kill.killmail_id,
    })
    .collect();

  prompts::DayActivity {
    engagement_count: combat.engagements.len() as u32,
    industry_count: industry_count as u32,
    losses,
    skill_count: skill_count as u32,
  }
}

fn empty_combat() -> rollup::Combat {
  rollup::Combat {
    engagements: Vec::new(),
    kill_count: 0,
    kill_value: 0.0,
    loss_count: 0,
    loss_value: 0.0,
  }
}

fn empty_wizard() -> wizard::State {
  wizard::State::new(&prompts::DayActivity::default(), Vec::new(), None, false)
}

fn entry_section(wizard: &wizard::State) -> Element<'_, Message> {
  Column::with_children(vec![
    section_kicker(&t!("captains_log.your_entry")),
    wizard::view_pane(wizard),
  ])
  .spacing(spacing::SPACE_3)
  .width(Length::Fill)
  .into()
}

fn install_snapshot(state: &mut State, db: &Database, snapshot: Snapshot) -> Task<Message> {
  state.days = snapshot.days;
  state.event_notes = snapshot.event_notes;
  state.event_owners = snapshot.event_owners;
  state.event_editing = None;
  state.jump_open = false;
  state.loading = false;
  state.past = None;
  state.selected = None;
  state.today_date = snapshot.today_date;
  rebuild_today(state);

  match state.today_day().is_some() {
    true => wizard::load(&state.wizard, db),
    false => Task::none(),
  }
}

fn iso_of(date: NaiveDate) -> String {
  date.format("%Y-%m-%d").to_string()
}

async fn load_event_notes(db: &Database, days: &[Day]) -> (HashMap<i64, String>, HashMap<i64, i64>) {
  let mut owners: HashMap<i64, i64> = HashMap::new();
  for day in days {
    for event in &day.events {
      if !owners.contains_key(&event.event_id)
        && let Ok(Some(owner)) = rollup::event_owner(db, event.event_id).await
      {
        owners.insert(event.event_id, owner);
      }
    }
  }

  let mut by_owner: HashMap<i64, Vec<i64>> = HashMap::new();
  for (event_id, owner) in &owners {
    by_owner.entry(*owner).or_default().push(*event_id);
  }

  let mut notes = HashMap::new();
  for (owner, ids) in &by_owner {
    if let Ok(rows) = calendar_event_note::list_for_events(db, *owner, ids).await {
      for row in rows {
        notes.insert(row.event_id, row.note);
      }
    }
  }

  (notes, owners)
}

fn log_of(state: &State) -> entries::Log {
  let today_iso = iso_of(state.today_date);
  let today = today_row(state, &today_iso);
  let past = state
    .days
    .iter()
    .filter(|day| day.date_iso != today_iso)
    .map(|day| entries::DayEntry {
      completeness: day.completeness.clone(),
      date_iso: day.date_iso.clone(),
      narrative: day.narrative.clone(),
      summary: past_summary(day),
    })
    .collect();

  entries::Log {
    past,
    today,
  }
}

fn main_body(state: &State) -> Element<'_, Message> {
  match &state.selected {
    Some(iso) => past_body(state, iso),
    None => today_body(state),
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

fn overlay_layers(state: &State) -> Vec<Element<'_, Message>> {
  if state.jump_open {
    let dropdown = positioned_dropdown_right(header::jump_dropdown(state), JUMP_OVERLAY_TOP, JUMP_OVERLAY_RIGHT);
    return vec![
      crate::ui::components::backdrop::click_catcher(Message::Header(header::Message::JumpToDay)),
      dropdown,
    ];
  }

  Vec::new()
}

fn past_body<'a>(state: &'a State, iso: &str) -> Element<'a, Message> {
  let (Some(day), Some(past)) = (state.day_of(iso), state.past.as_ref()) else {
    return Space::new().into();
  };

  let summary = summary_of(day);
  let events =
    (!day.events.is_empty()).then(|| events::section(&day.events, &state.event_notes, state.event_editing.as_ref()));

  past::view_pane(past, &summary, events)
}

fn past_engagements(day: &Day) -> Vec<past::Engagement> {
  day
    .engagements
    .iter()
    .map(|engagement| past::Engagement {
      character_id: engagement.character_id,
      character_name: engagement.character_name.clone(),
      icon: engagement.icon.clone(),
      is_kill: engagement.is_kill,
      killmail_id: engagement.killmail_id,
      ship: engagement.ship.clone(),
      system: engagement.system.clone(),
      value: engagement.value,
    })
    .collect()
}

fn past_summary(day: &Day) -> String {
  format!(
    "{} \u{b7} {} \u{b7} {}",
    t!("captains_log.entries.skills", count => day.skill_count),
    signed_isk(day.money.net()),
    t!("captains_log.entries.combat_tally", kills => day.kill_count, losses => day.loss_count)
  )
}

fn pilot_count(
  combat: &rollup::Combat,
  industry: &[IndustryDelivery],
  skills: &[SkillCompletion],
  character_ids: &[i64],
) -> usize {
  let mut ids: Vec<i64> = Vec::new();
  ids.extend(combat.engagements.iter().map(|kill| kill.character_id));
  ids.extend(industry.iter().map(|job| job.character_id));
  ids.extend(skills.iter().map(|skill| skill.character_id));
  ids.sort_unstable();
  ids.dedup();

  // No recorded activity doesn't mean zero pilots were present; fall back to the full roster count.
  if ids.is_empty() { character_ids.len() } else { ids.len() }
}

fn rebuild_today(state: &mut State) {
  let today_iso = iso_of(state.today_date);
  let Some(day) = state.days.iter().find(|day| day.date_iso == today_iso) else {
    state.narrative = narrative::State::new(None);
    state.wizard = empty_wizard();
    return;
  };

  let narrative = narrative::State::new(day.narrative.clone());
  let wizard = wizard::State::new(
    &day.activity,
    wizard_engagements(day),
    day.log.as_ref(),
    day.completeness.is_complete(),
  );

  state.narrative = narrative;
  state.wizard = wizard;
}

async fn resolve_industry(db: &Database, rows: &[IndustryDelivery]) -> Vec<String> {
  let mut out = Vec::with_capacity(rows.len());
  for row in rows {
    let name = match row.product_type_id {
      Some(type_id) => type_name(db, type_id).await,
      None => t!("roster.fallback.unknown").into_owned(),
    };
    out.push(name);
  }
  out
}

async fn resolve_skills(db: &Database, rows: &[SkillCompletion]) -> Vec<(String, i64)> {
  let mut out = Vec::with_capacity(rows.len());
  for row in rows {
    out.push((type_name(db, row.skill_id).await, row.level));
  }
  out
}

fn rollup_section<'a>(
  summary: &rollup_tiles::Summary,
  events: &'a [CalendarEntry],
  notes: &'a HashMap<i64, String>,
  editing: Option<&'a events::Editing>,
) -> Element<'a, Message> {
  Column::with_children(vec![
    rollup_tiles::render(summary, rollup_tiles::Scope::Account),
    events::section(events, notes, editing),
  ])
  .spacing(spacing::SPACE_3)
  .width(Length::Fill)
  .into()
}

fn route_events(state: &mut State, db: &Database, message: events::Message) -> Task<Message> {
  match message {
    events::Message::DraftChanged(value) => {
      if let Some(edit) = state.event_editing.as_mut() {
        edit.draft = value;
      }
      Task::none()
    }
    events::Message::EditCancelled => {
      state.event_editing = None;
      Task::none()
    }
    events::Message::EditRequested(event_id) => {
      state.event_editing = Some(events::begin_edit(&state.event_notes, event_id));
      Task::none()
    }
    events::Message::NoteChanged(event_id, note) => save_event_note(state, db, event_id, note),
    events::Message::NoteSaved => Task::none(),
  }
}

fn route_narrative(state: &mut State, db: &Database, message: narrative::Message) -> Task<Message> {
  let iso = iso_of(state.today_date);
  narrative::update_pane(&mut state.narrative, &iso, db, message)
}

fn route_past(state: &mut State, db: &Database, message: past::Message) -> Task<Message> {
  match state.past.as_mut() {
    Some(past) => past::update_pane(past, db, message),
    None => Task::none(),
  }
}

fn route_wizard(state: &mut State, db: &Database, message: wizard::Message) -> Task<Message> {
  let iso = iso_of(state.today_date);
  wizard::update_pane(&mut state.wizard, &iso, db, message)
}

fn save_event_note(state: &mut State, db: &Database, event_id: i64, note: String) -> Task<Message> {
  state.event_notes.insert(event_id, note.clone());
  state.event_editing = None;

  let Some(&owner) = state.event_owners.get(&event_id) else {
    return Task::none();
  };
  let db = db.clone();

  Task::perform(
    async move {
      let _ = calendar_event_note::upsert(&db, owner, event_id, &note).await;
    },
    |()| Message::Events(events::Message::NoteSaved),
  )
}

fn section_kicker(label: &str) -> Element<'static, Message> {
  Row::with_children(vec![
    eyebrow(label, None),
    container(rule::horizontal()).width(Length::Fill).into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn select_day(state: &mut State, db: &Database, day: Option<String>) -> Task<Message> {
  state.jump_open = false;
  state.selected = day.clone();

  match day {
    Some(iso) => build_past(state, db, &iso),
    None => {
      state.past = None;
      Task::none()
    }
  }
}

fn build_past(state: &mut State, db: &Database, iso: &str) -> Task<Message> {
  let Some(day) = state.days.iter().find(|day| day.date_iso == iso) else {
    state.past = None;
    return Task::none();
  };

  let past = past::State::new(
    iso.to_owned(),
    day.log.clone(),
    day.completeness.clone(),
    past_engagements(day),
  );
  let task = past::load_reports(&past, db);
  state.past = Some(past);
  task
}

fn signed_isk(value: f64) -> String {
  let sign = if value < 0.0 { "\u{2212}" } else { "+" };
  format!("{sign}{}", crate::ui::format::fmt_isk(value.abs()))
}

fn status_view<'a>(message: &str) -> Element<'a, Message> {
  container(crate::ui::components::eyebrow::eyebrow(message, None))
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center)
    .style(|_| container::Style {
      background: Some(iced::Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn summary_of(day: &Day) -> rollup_tiles::Summary {
  rollup_tiles::Summary {
    engagements: Vec::new(),
    industry: day.industry.clone(),
    kill_count: day.kill_count,
    loss_count: day.loss_count,
    loss_value: day.loss_value,
    money: day.money,
    net_worth: day.net_worth,
    pilot_count: day.pilot_count,
    skills: day
      .skills
      .iter()
      .map(|(skill, level)| rollup_tiles::SkillLine {
        level: *level,
        skill: skill.clone(),
      })
      .collect(),
  }
}

async fn system_name(db: &Database, system_id: i64) -> String {
  sde::get_solar_system(db, system_id)
    .await
    .ok()
    .flatten()
    .map(|system| system.name().clone())
    .unwrap_or_else(|| t!("roster.fallback.unknown").into_owned())
}

fn today_body(state: &State) -> Element<'_, Message> {
  let Some(day) = state.today_day() else {
    return Space::new().into();
  };
  let summary = summary_of(day);

  Column::with_children(vec![
    narrative::view_pane(&state.narrative),
    rollup_section(&summary, &day.events, &state.event_notes, state.event_editing.as_ref()),
    entry_section(&state.wizard),
  ])
  .spacing(spacing::SPACE_6)
  .width(Length::Fill)
  .into()
}

fn today_row(state: &State, today_iso: &str) -> entries::Today {
  match state.day_of(today_iso) {
    Some(day) => entries::Today {
      completeness: day.completeness.clone(),
      date: state.today_date,
      kill_count: day.kill_count,
      loss_count: day.loss_count,
      net_isk: day.money.net(),
      skill_count: day.skill_count,
    },
    None => entries::Today {
      completeness: prompts::Completeness::default(),
      date: state.today_date,
      kill_count: 0,
      loss_count: 0,
      net_isk: 0.0,
      skill_count: 0,
    },
  }
}

fn toggle_jump(state: &mut State) -> Task<Message> {
  state.jump_open = !state.jump_open;
  Task::none()
}

async fn type_name(db: &Database, type_id: i64) -> String {
  sde::get_item_type(db, type_id)
    .await
    .ok()
    .flatten()
    .map(|item| item.name().clone())
    .unwrap_or_else(|| t!("roster.fallback.ship_type", id => type_id).into_owned())
}

fn wizard_engagements(day: &Day) -> Vec<wizard::Engagement> {
  day
    .engagements
    .iter()
    .map(|engagement| wizard::Engagement {
      character_id: engagement.character_id,
      character_name: engagement.character_name.clone(),
      icon: engagement.icon.clone(),
      is_kill: engagement.is_kill,
      killmail_id: engagement.killmail_id,
      ship_name: engagement.ship.clone(),
      system: engagement.system.clone(),
      value: engagement.value,
    })
    .collect()
}

#[cfg(test)]
fn stub_day(date_iso: &str) -> Day {
  Day {
    activity: prompts::DayActivity::default(),
    completeness: prompts::Completeness::default(),
    date_iso: date_iso.to_owned(),
    engagements: Vec::new(),
    events: Vec::new(),
    industry: Vec::new(),
    kill_count: 0,
    log: None,
    loss_count: 0,
    loss_value: 0.0,
    money: DayMoney::default(),
    narrative: None,
    net_worth: None,
    pilot_count: 1,
    skill_count: 0,
    skills: Vec::new(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn day(date_iso: &str) -> Day {
    stub_day(date_iso)
  }

  fn loaded_state() -> State {
    let mut state = State::new();
    state.loading = false;
    state.today_date = NaiveDate::from_ymd_opt(2026, 7, 5).unwrap();
    state.days = vec![day("2026-07-05"), day("2026-07-04")];
    rebuild_today(&mut state);
    state
  }

  mod install_snapshot {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_installs_days_and_clears_loading() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      let snapshot = Snapshot {
        days: vec![day("2026-07-05")],
        event_notes: HashMap::new(),
        event_owners: HashMap::new(),
        today_date: NaiveDate::from_ymd_opt(2026, 7, 5).unwrap(),
      };

      let _ = install_snapshot(&mut state, &db, snapshot);

      assert_eq!(state.days.len(), 1);
      assert!(!state.loading);
      assert!(state.selected.is_none());
    }
  }

  mod select_day {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_opens_a_past_day_and_returns_to_today() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state();

      let _ = select_day(&mut state, &db, Some("2026-07-04".to_owned()));
      assert_eq!(state.selected.as_deref(), Some("2026-07-04"));
      assert!(state.past.is_some());

      let _ = select_day(&mut state, &db, None);
      assert_eq!(state.selected, None);
      assert!(state.past.is_none());
    }
  }

  mod jump {
    use super::*;

    #[test]
    fn it_toggles_the_jump_dropdown() {
      let mut state = loaded_state();

      let _ = toggle_jump(&mut state);
      assert!(state.jump_open);

      let _ = toggle_jump(&mut state);
      assert!(!state.jump_open);
    }
  }

  mod escape {
    use super::*;

    #[test]
    fn it_closes_the_jump_dropdown_before_dismissing() {
      let mut state = loaded_state();
      state.jump_open = true;

      assert!(matches!(
        escape_dismiss(&state),
        Some(Message::Header(header::Message::JumpToDay))
      ));
    }

    #[test]
    fn it_dismisses_the_route_back_to_the_roster() {
      let state = loaded_state();

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
    fn it_renders_the_loading_state() {
      let state = State::new();

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_today_view() {
      let state = loaded_state();

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_a_selected_past_day() {
      let mut state = loaded_state();
      state.selected = Some("2026-07-04".to_owned());
      state.past = Some(past::State::new(
        "2026-07-04".to_owned(),
        None,
        prompts::Completeness::default(),
        Vec::new(),
      ));

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_collects_no_stale_images() {
      let state = loaded_state();

      assert!(state.stale_images().is_empty());
    }
  }
}
