mod entries;
mod eve_date;
mod events;
mod field_notes;
mod header;
pub mod km_report;
mod narrative;
mod objective_link;
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

use super::standing_orders;
use crate::{
  clients::eve_image::Size,
  features::shell::window_state::UiState,
  store::{
    Database, images,
    images::{IconResolution, ImageKind},
    model::{FieldNote, PromptConfig, SkillCompletion},
    repo::{
      calendar_event_note, captains_log, captains_log_rollup,
      captains_log_rollup::{CalendarEntry, CombatKill, DayMoney, IndustryDelivery, NetWorthDelta},
      character, field_notes as field_notes_repo, killmail_report, objective, sde,
    },
  },
  ui::{
    components::{
      date_picker::DatePickerState,
      eyebrow::eyebrow,
      modal_overlay::stable_overlay,
      positioned_dropdown::positioned_dropdown_right,
      resizable_pane::{self, PaneDrag, pane_handle},
      rule,
    },
    style::{color, control, spacing},
  },
};

const ENGAGEMENT_ICON_SIZE: Size = Size::S64;
const DAYS_PAGE: usize = 30;
const ENTRIES_MIN_WIDTH: f32 = 220.0;
const ENTRIES_PANE_KEY: &str = "captains_log.entries";
const ENTRIES_WIDTH: f32 = 276.0;
const JUMP_OVERLAY_RIGHT: f32 = 28.0;
const SCROLL_LOAD_THRESHOLD: f32 = 0.85;
const JUMP_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;
const MAIN_MAX_WIDTH: f32 = 940.0;

#[derive(Clone, Debug)]
pub enum Message {
  Entries(entries::Message),
  Events(events::Message),
  Exit,
  FieldNotes(field_notes::Message),
  Header(header::Message),
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart,
  PaneSettled(&'static str, f32),
  EntriesScrolled(f32),
  Loaded(Box<Snapshot>),
  MoreDays(Box<MorePage>),
  Narrative(narrative::Message),
  ObjectiveLink(objective_link::Message),
  Past(past::Message),
  StandingOrders(standing_orders::Message),
  Wizard(wizard::Message),
}

impl Message {
  pub fn loads_data(&self) -> bool {
    matches!(self, Message::Loaded(_))
  }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
  all_dates: Vec<String>,
  character_ids: Vec<i64>,
  config: PromptConfig,
  days: Vec<Day>,
  event_notes: HashMap<i64, String>,
  event_owners: HashMap<i64, i64>,
  flagged_total: usize,
  objectives: Vec<objective_link::ObjectiveOption>,
  today_date: NaiveDate,
}

impl Snapshot {
  #[cfg(test)]
  fn empty() -> Self {
    Snapshot {
      all_dates: Vec::new(),
      character_ids: Vec::new(),
      config: PromptConfig::default(),
      flagged_total: 0,
      days: Vec::new(),
      event_notes: HashMap::new(),
      event_owners: HashMap::new(),
      objectives: Vec::new(),
      today_date: Utc::now().date_naive(),
    }
  }
}

pub struct State {
  all_dates: Vec<String>,
  board_mode: bool,
  character_ids: Vec<i64>,
  config: PromptConfig,
  days: Vec<Day>,
  event_editing: Option<events::Editing>,
  event_notes: HashMap<i64, String>,
  field_notes: field_notes::State,
  entries_pane: PaneDrag,
  event_owners: HashMap<i64, i64>,
  flagged_total: usize,
  jump_open: bool,
  jump_picker: DatePickerState,
  loading: bool,
  loading_more: bool,
  narrative: narrative::State,
  objective_link: objective_link::State,
  past: Option<past::State>,
  selected: Option<String>,
  standing_orders: standing_orders::State,
  today_date: NaiveDate,
  today_iso: String,
  wizard: wizard::State,
}

impl State {
  pub fn new() -> Self {
    let today = Utc::now().date_naive();
    State {
      all_dates: Vec::new(),
      board_mode: false,
      character_ids: Vec::new(),
      config: PromptConfig::default(),
      days: Vec::new(),
      event_editing: None,
      event_notes: HashMap::new(),
      field_notes: field_notes::State::new(iso_of(today), Vec::new()),
      entries_pane: PaneDrag::with_min_width(ENTRIES_WIDTH, ENTRIES_MIN_WIDTH, spacing::layout::WINDOW_DEFAULT_WIDTH),
      event_owners: HashMap::new(),
      flagged_total: 0,
      jump_open: false,
      jump_picker: DatePickerState::new(today, None),
      loading: true,
      loading_more: false,
      narrative: narrative::State::new(None),
      objective_link: objective_link::State::default(),
      past: None,
      selected: None,
      standing_orders: standing_orders::State::new(),
      today_date: today,
      today_iso: iso_of(today),
      wizard: empty_wizard(),
    }
  }

  pub fn set_pane_host_width(&mut self, host_width: f32) {
    self.entries_pane.set_host_width(host_width);
  }

  pub fn stale_images(&self) -> Vec<(ImageKind, i64)> {
    Vec::new()
  }

  pub fn with_restored_panes(mut self, ui: &UiState) -> Self {
    let host_width = ui.host_width("main", spacing::layout::WINDOW_DEFAULT_WIDTH);
    self.entries_pane =
      PaneDrag::from_store_with_min(ui, ENTRIES_PANE_KEY, ENTRIES_WIDTH, ENTRIES_MIN_WIDTH, host_width);
    self
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
  field_notes: Vec<FieldNote>,
  industry: Vec<prompts::IndustryEvidence>,
  kill_count: usize,
  links: Vec<crate::store::model::ObjectiveLink>,
  log: Option<crate::store::model::CaptainsLog>,
  loss_count: usize,
  loss_value: f64,
  money: DayMoney,
  narrative: Option<String>,
  net_worth: Option<NetWorthDelta>,
  pilot_count: usize,
  skill_count: usize,
  skills: Vec<prompts::SkillEvidence>,
}

#[derive(Clone, Debug)]
pub struct MorePage {
  days: Vec<Day>,
  event_notes: HashMap<i64, String>,
  event_owners: HashMap<i64, i64>,
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
    Message::Entries(entries::Message::MarkAllComplete) => mark_all_complete(state, db),
    Message::Entries(entries::Message::MarkComplete(day)) => mark_day_complete(state, db, day),
    Message::Entries(entries::Message::OpenOrders) => open_board(state),
    Message::Entries(entries::Message::Selected(day)) => select_day(state, db, day),
    Message::Events(msg) => route_events(state, db, msg),
    Message::Exit => Task::none(),
    Message::FieldNotes(msg) => route_field_notes(state, db, msg),
    Message::Loaded(snapshot) => install_snapshot(state, db, *snapshot),
    Message::Narrative(msg) => route_narrative(state, db, msg),
    Message::ObjectiveLink(msg) => route_objective_link(state, db, msg),
    Message::Past(msg) => route_past(state, db, msg),
    Message::StandingOrders(msg) => route_standing_orders(state, db, msg),
    Message::Wizard(msg) => route_wizard(state, db, msg),
    other => update_shell(state, other, db),
  }
}

fn update_shell(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::EntriesScrolled(relative) => entries_scrolled(state, db, relative),
    Message::Header(header::Message::JumpToDay) => toggle_jump(state),
    Message::Header(header::Message::NextMonth) => {
      state.jump_picker.next_month();
      Task::none()
    }
    Message::Header(header::Message::PrevMonth) => {
      state.jump_picker.prev_month();
      Task::none()
    }
    Message::MoreDays(page) => append_days(state, db, *page),
    Message::PaneDrag(x) => {
      state.entries_pane.drag_to(x);
      Task::none()
    }
    Message::PaneDragEnd => {
      state.entries_pane.end();
      Task::done(Message::PaneSettled(ENTRIES_PANE_KEY, state.entries_pane.ratio()))
    }
    Message::PaneDragStart => {
      state.entries_pane.start();
      Task::none()
    }
    _ => Task::none(),
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  if state.loading {
    return status_view(&t!("captains_log.loading"));
  }
  if state.days.is_empty() {
    return status_view(&t!("captains_log.empty"));
  }

  let main: Element<'_, Message> = if state.board_mode {
    container(main_body(state))
      .width(Length::Fill)
      .height(Length::Fill)
      .max_width(MAIN_MAX_WIDTH)
      .padding(Padding {
        top: 26.0,
        right: 34.0,
        bottom: 24.0,
        left: 34.0,
      })
      .into()
  } else {
    scrollable(
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
    .style(control::scrollbar)
    .into()
  };

  let panes = Row::with_children(vec![
    container(
      scrollable(
        container(entries::render(
          &log_of(state),
          state.selected.as_deref(),
          state.flagged_total,
          state.all_dates.len().max(state.days.len()),
          entries::Orders {
            active: state.board_mode,
            active_count: state.standing_orders.active_count(),
            total: state.standing_orders.total_count(),
          },
        ))
        .width(Length::Fill)
        .padding(spacing::SPACE_4_5),
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .on_scroll(|viewport| Message::EntriesScrolled(viewport.relative_offset().y))
      .style(control::scrollbar),
    )
    .width(Length::Fixed(state.entries_pane.width()))
    .height(Length::Fill)
    .into(),
    pane_handle(Message::PaneDragStart),
    main,
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

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  if state.entries_pane.is_active() {
    return iced::event::listen_with(|event, _status, _id| {
      resizable_pane::drag_event(event, Message::PaneDrag, Message::PaneDragEnd)
    });
  }
  iced::Subscription::none()
}

pub fn escape_dismiss(state: &State) -> Option<Message> {
  if state.jump_open {
    return Some(Message::Header(header::Message::JumpToDay));
  }
  if state.board_mode
    && let Some(message) = standing_orders::escape_dismiss(&state.standing_orders)
  {
    return Some(Message::StandingOrders(message));
  }
  Some(Message::Exit)
}

async fn build_day(db: &Database, config: &PromptConfig, character_ids: &[i64], iso: &str) -> Day {
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
  let field_notes = field_notes_repo::list_for_date(db, iso).await.unwrap_or_default();
  let links = objective::links_for_day(db, iso).await.unwrap_or_default();
  let industry = resolve_industry(db, &industry_rows).await;
  let skills = resolve_skills(db, &skill_rows).await;

  let activity = day_activity(&combat, &industry, &skills);
  let completeness = prompts::completeness(config, &activity, log.as_ref(), &reports);
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
    field_notes,
    industry,
    kill_count: combat.kill_count,
    links,
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
  let config = captains_log::load_prompt_config(db).await.unwrap_or_default();

  let logged = captains_log::dates(db).await.unwrap_or_default();
  let active = rollup::active_dates(db).await.unwrap_or_default();
  let mut day_isos = entries::merged_days(logged, active);
  // Today must always render, even with zero logged/rollup activity, so the entry is there to log against.
  if !day_isos.iter().any(|iso| iso == &today_iso) {
    day_isos.insert(0, today_iso.clone());
  }

  let mut days = Vec::with_capacity(DAYS_PAGE.min(day_isos.len()));
  for iso in day_isos.iter().take(DAYS_PAGE) {
    days.push(build_day(db, &config, character_ids, iso).await);
  }

  let (event_notes, event_owners) = load_event_notes(db, &days).await;
  let objectives = objective_link::options(&objective::list(db, None).await.unwrap_or_default());

  let incomplete = captains_log_rollup::incomplete_dates(db).await.unwrap_or_default();
  let mut flagged_total = incomplete.len();
  let today_flagged_locally = days
    .iter()
    .find(|day| day.date_iso == today_iso)
    .is_some_and(|day| !day.completeness.is_complete());
  if today_flagged_locally && !incomplete.iter().any(|iso| iso == &today_iso) {
    flagged_total += 1;
  }

  Snapshot {
    all_dates: day_isos,
    character_ids: character_ids.to_vec(),
    config,
    days,
    event_notes,
    event_owners,
    flagged_total,
    objectives,
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

fn day_activity(
  combat: &rollup::Combat,
  industry: &[prompts::IndustryEvidence],
  skills: &[prompts::SkillEvidence],
) -> prompts::DayActivity {
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
    industry: industry.to_vec(),
    industry_count: industry.len() as u32,
    losses,
    skill_count: skills.len() as u32,
    skills: skills.to_vec(),
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
  wizard::State::new(
    &PromptConfig::default(),
    &prompts::DayActivity::default(),
    Vec::new(),
    None,
    false,
  )
}

fn entry_section(state: &State) -> Element<'_, Message> {
  let ctx = wizard::LinkCtx {
    links: &state.objective_link,
    date: &state.today_iso,
  };
  Column::with_children(vec![
    section_kicker(&t!("captains_log.your_entry")),
    wizard::view_pane(&state.wizard, ctx),
  ])
  .spacing(spacing::SPACE_3)
  .width(Length::Fill)
  .into()
}

fn install_snapshot(state: &mut State, db: &Database, snapshot: Snapshot) -> Task<Message> {
  state.all_dates = snapshot.all_dates;
  state.character_ids = snapshot.character_ids;
  state.config = snapshot.config;
  state.flagged_total = snapshot.flagged_total;
  state.loading_more = false;
  state.days = snapshot.days;
  state.event_notes = snapshot.event_notes;
  state.event_owners = snapshot.event_owners;
  state.event_editing = None;
  state.jump_open = false;
  state.loading = false;
  state.past = None;
  state.selected = None;
  state.today_date = snapshot.today_date;
  state.today_iso = iso_of(snapshot.today_date);
  state.objective_link.set_objectives(snapshot.objectives);
  sync_day_links(state);
  rebuild_today(state);

  let orders = standing_orders::load(db).map(Message::StandingOrders);
  match state.today_day().is_some() {
    true => Task::batch([wizard::load(&state.wizard, db), orders]),
    false => orders,
  }
}

fn iso_of(date: NaiveDate) -> String {
  date.format("%Y-%m-%d").to_string()
}

fn sync_day_links(state: &mut State) {
  for day in &state.days {
    state
      .objective_link
      .set_day_links(day.date_iso.clone(), day.links.clone());
  }
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
  if state.board_mode {
    return standing_orders::view(&state.standing_orders).map(Message::StandingOrders);
  }
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
    let dropdown = positioned_dropdown_right(header::jump_calendar(state), JUMP_OVERLAY_TOP, JUMP_OVERLAY_RIGHT);
    return vec![
      crate::ui::components::backdrop::click_catcher(Message::Header(header::Message::JumpToDay)),
      dropdown,
    ];
  }

  if state.board_mode {
    return standing_orders::overlay_layers(&state.standing_orders)
      .into_iter()
      .map(|layer| layer.map(Message::StandingOrders))
      .collect();
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

  past::view_pane(past, &state.objective_link, &summary, events)
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
    state.field_notes = field_notes::State::new(today_iso, Vec::new());
    return;
  };

  let narrative = narrative::State::new(day.narrative.clone());
  let wizard = wizard::State::new(
    &state.config,
    &day.activity,
    wizard_engagements(day),
    day.log.as_ref(),
    day.completeness.is_complete(),
  );
  let field_notes = field_notes::State::new(day.date_iso.clone(), day.field_notes.clone());

  state.narrative = narrative;
  state.wizard = wizard;
  state.field_notes = field_notes;
}

async fn resolve_industry(db: &Database, rows: &[IndustryDelivery]) -> Vec<prompts::IndustryEvidence> {
  let mut out = Vec::with_capacity(rows.len());
  for row in rows {
    let product = match row.product_type_id {
      Some(type_id) => type_name(db, type_id).await,
      None => t!("roster.fallback.unknown").into_owned(),
    };
    out.push(prompts::IndustryEvidence {
      character_id: row.character_id,
      character_name: character_name(db, row.character_id).await,
      product,
      product_type_id: row.product_type_id,
      runs: row.runs,
    });
  }
  out
}

async fn resolve_skills(db: &Database, rows: &[SkillCompletion]) -> Vec<prompts::SkillEvidence> {
  let mut out = Vec::with_capacity(rows.len());
  for row in rows {
    out.push(prompts::SkillEvidence {
      character_id: row.character_id,
      character_name: character_name(db, row.character_id).await,
      level: row.level,
      skill: type_name(db, row.skill_id).await,
      skill_id: row.skill_id,
    });
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

fn route_field_notes(state: &mut State, db: &Database, message: field_notes::Message) -> Task<Message> {
  // Only one day's field-notes card is on screen at a time (today OR the selected past day), so the
  // selection decides which owned sub-pane state the message belongs to.
  match (state.selected.is_some(), state.past.as_mut()) {
    (true, Some(past)) => past::update_field_notes(past, db, message),
    (true, None) => Task::none(),
    (false, _) => field_notes::update_pane(&mut state.field_notes, db, message),
  }
}

fn route_narrative(state: &mut State, db: &Database, message: narrative::Message) -> Task<Message> {
  if matches!(message, narrative::Message::WriteRequested) {
    let iso = iso_of(state.today_date);
    let index = state.wizard.narrative_step_index();
    return wizard::update_pane(&mut state.wizard, &iso, db, wizard::Message::JumpTo(index));
  }

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

fn route_standing_orders(state: &mut State, db: &Database, message: standing_orders::Message) -> Task<Message> {
  standing_orders::update(&mut state.standing_orders, message, db).map(Message::StandingOrders)
}

fn route_objective_link(state: &mut State, db: &Database, message: objective_link::Message) -> Task<Message> {
  match message {
    objective_link::Message::OpenBoard(objective) => open_board_jump(state, db, objective),
    other => objective_link::update(&mut state.objective_link, db, other),
  }
}

fn open_board_jump(state: &mut State, db: &Database, objective: Option<i64>) -> Task<Message> {
  let task = open_board(state);
  match objective {
    Some(id) => Task::batch([
      task,
      route_standing_orders(state, db, standing_orders::Message::OpenObjective(id)),
    ]),
    None => task,
  }
}

fn open_board(state: &mut State) -> Task<Message> {
  state.board_mode = true;
  state.selected = None;
  state.past = None;
  state.jump_open = false;
  Task::none()
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

fn append_days(state: &mut State, db: &Database, page: MorePage) -> Task<Message> {
  state.loading_more = false;
  for day in page.days {
    if !state.days.iter().any(|loaded| loaded.date_iso == day.date_iso) {
      state.days.push(day);
    }
  }
  state.days.sort_by(|a, b| b.date_iso.cmp(&a.date_iso));
  state.event_notes.extend(page.event_notes);
  state.event_owners.extend(page.event_owners);
  sync_day_links(state);

  match state.selected.clone() {
    Some(iso) if state.past.is_none() => build_past(state, db, &iso),
    _ => Task::none(),
  }
}

fn entries_scrolled(state: &mut State, db: &Database, relative: f32) -> Task<Message> {
  if relative < SCROLL_LOAD_THRESHOLD || state.loading_more || state.days.len() >= state.all_dates.len() {
    return Task::none();
  }
  let loaded: Vec<String> = state.days.iter().map(|day| day.date_iso.clone()).collect();
  let next: Vec<String> = state
    .all_dates
    .iter()
    .filter(|iso| !loaded.contains(iso))
    .take(DAYS_PAGE)
    .cloned()
    .collect();
  if next.is_empty() {
    return Task::none();
  }
  state.loading_more = true;
  load_days(db, state.character_ids.clone(), next)
}

fn load_days(db: &Database, character_ids: Vec<i64>, isos: Vec<String>) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      let config = captains_log::load_prompt_config(&db).await.unwrap_or_default();
      let mut days = Vec::with_capacity(isos.len());
      for iso in &isos {
        days.push(build_day(&db, &config, &character_ids, iso).await);
      }
      let (event_notes, event_owners) = load_event_notes(&db, &days).await;
      Box::new(MorePage {
        days,
        event_notes,
        event_owners,
      })
    },
    Message::MoreDays,
  )
}

fn mark_all_complete(state: &mut State, db: &Database) -> Task<Message> {
  for day in &mut state.days {
    day.completeness = prompts::Completeness::default();
  }
  state.flagged_total = 0;
  let db = db.clone();
  let today_iso = iso_of(state.today_date);
  Task::future(async move {
    let mut isos = captains_log_rollup::incomplete_dates(&db).await.unwrap_or_default();
    if !isos.iter().any(|iso| iso == &today_iso) {
      isos.push(today_iso);
    }
    for iso in isos {
      let _ = captains_log::mark_complete(&db, &iso).await;
    }
  })
  .discard()
}

fn mark_day_complete(state: &mut State, db: &Database, day: Option<String>) -> Task<Message> {
  let iso = day.unwrap_or_else(|| iso_of(state.today_date));
  if let Some(day) = state.days.iter_mut().find(|day| day.date_iso == iso) {
    if !day.completeness.is_complete() {
      state.flagged_total = state.flagged_total.saturating_sub(1);
    }
    day.completeness = prompts::Completeness::default();
  }
  let db = db.clone();
  Task::future(async move {
    let _ = captains_log::mark_complete(&db, &iso).await;
  })
  .discard()
}

fn select_day(state: &mut State, db: &Database, day: Option<String>) -> Task<Message> {
  state.jump_open = false;
  state.board_mode = false;
  state.selected = day.clone();

  match day {
    Some(iso) => {
      if state.days.iter().any(|day| day.date_iso == iso) {
        build_past(state, db, &iso)
      } else {
        state.past = None;
        load_days(db, state.character_ids.clone(), vec![iso])
      }
    }
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
    day.field_notes.clone(),
    prompts::all_field_prompts(&state.config),
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
    industry: day.industry.iter().map(|item| item.product.clone()).collect(),
    kill_count: day.kill_count,
    loss_count: day.loss_count,
    loss_value: day.loss_value,
    money: day.money,
    net_worth: day.net_worth,
    pilot_count: day.pilot_count,
    skills: day
      .skills
      .iter()
      .map(|item| rollup_tiles::SkillLine {
        level: item.level,
        skill: item.skill.clone(),
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
    objective_link::day_panel(&state.objective_link, &state.today_iso),
    entry_section(state),
    field_notes_section(state),
  ])
  .spacing(spacing::SPACE_6)
  .width(Length::Fill)
  .into()
}

fn field_notes_section(state: &State) -> Element<'_, Message> {
  Column::with_children(vec![
    section_kicker(&t!("captains_log.field_notes.kicker")),
    field_notes::view_pane(&state.field_notes, &state.objective_link),
  ])
  .spacing(spacing::SPACE_3)
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
  if state.jump_open {
    let shown = state
      .selected
      .as_deref()
      .and_then(|iso| NaiveDate::parse_from_str(iso, "%Y-%m-%d").ok())
      .unwrap_or(state.today_date);
    state.jump_picker = DatePickerState::new(shown, None);
  }
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
    field_notes: Vec::new(),
    industry: Vec::new(),
    kill_count: 0,
    links: Vec::new(),
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
        all_dates: vec!["2026-07-05".to_owned()],
        character_ids: Vec::new(),
        config: PromptConfig::default(),
        days: vec![day("2026-07-05")],
        event_notes: HashMap::new(),
        event_owners: HashMap::new(),
        flagged_total: 0,
        objectives: Vec::new(),
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

  mod narrative_hero {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_jumps_the_wizard_to_the_narrative_step_from_the_hero_cta() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state();

      let _ = update(&mut state, Message::Narrative(narrative::Message::WriteRequested), &db);

      assert_eq!(state.wizard.step(), state.wizard.narrative_step_index());
      assert!(!state.wizard.is_finished());
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

  mod update {
    use super::*;

    #[tokio::test]
    async fn it_routes_every_message_arm() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state();

      let _ = update(&mut state, Message::Exit, &db);

      let _ = update(&mut state, Message::Header(header::Message::JumpToDay), &db);
      assert!(state.jump_open, "the jump header toggles the dropdown");

      let _ = update(&mut state, Message::Entries(entries::Message::Selected(None)), &db);
      assert!(!state.jump_open, "selecting an entry closes the jump dropdown");

      let _ = update(&mut state, Message::Narrative(narrative::Message::EditRequested), &db);
      let _ = update(&mut state, Message::Past(past::Message::Cancelled), &db);
      let _ = update(&mut state, Message::Wizard(wizard::Message::Saved), &db);
      let _ = update(&mut state, Message::Events(events::Message::EditCancelled), &db);

      let snapshot = Snapshot {
        all_dates: vec!["2026-07-05".to_owned()],
        character_ids: Vec::new(),
        config: PromptConfig::default(),
        days: vec![day("2026-07-05")],
        event_notes: HashMap::new(),
        event_owners: HashMap::new(),
        flagged_total: 0,
        objectives: Vec::new(),
        today_date: NaiveDate::from_ymd_opt(2026, 7, 5).unwrap(),
      };
      let _ = update(&mut state, Message::Loaded(Box::new(snapshot)), &db);
      assert!(!state.loading, "a loaded snapshot clears the loading flag");
    }
  }

  mod mark_complete_actions {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_clears_a_days_flags_and_decrements_the_banner_total() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state();
      let mut flagged = day("2026-07-04");
      flagged
        .completeness
        .missing_prompts
        .push(crate::store::repo::captains_log::AnswerKey::Goal);
      state.days = vec![flagged];
      state.flagged_total = 3;

      let _ = update(
        &mut state,
        Message::Entries(entries::Message::MarkComplete(Some("2026-07-04".to_owned()))),
        &db,
      );

      assert!(state.days[0].completeness.is_complete());
      assert_eq!(state.flagged_total, 2);
    }

    #[tokio::test]
    async fn it_marks_everything_and_zeroes_the_banner_total() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state();
      let mut flagged = day("2026-07-04");
      flagged
        .completeness
        .missing_prompts
        .push(crate::store::repo::captains_log::AnswerKey::Goal);
      state.days = vec![flagged, day("2026-07-03")];
      state.flagged_total = 9;

      let _ = update(&mut state, Message::Entries(entries::Message::MarkAllComplete), &db);

      assert!(state.days.iter().all(|day| day.completeness.is_complete()));
      assert_eq!(state.flagged_total, 0);
    }
  }

  mod build_snapshot {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_pages_the_day_list_and_counts_flagged_days() {
      let db = crate::store::open_test().await.unwrap();
      let today_iso = iso_of(Utc::now().date_naive());
      captains_log::upsert_narrative(&db, "2026-01-03", Some("quiet day"))
        .await
        .unwrap();
      captains_log::mark_complete(&db, "2026-01-02").await.unwrap();

      let snapshot = build_snapshot(&db, &[]).await;

      assert_eq!(snapshot.all_dates.first(), Some(&today_iso));
      assert!(snapshot.all_dates.contains(&"2026-01-03".to_owned()));
      assert_eq!(snapshot.days.len(), snapshot.all_dates.len().min(DAYS_PAGE));
      // 2026-01-03 has no goal and 2026-01-02 is marked complete; today has no
      // activity row, so it is flagged locally on top of the query's count.
      assert_eq!(snapshot.flagged_total, 2);
      assert!(snapshot.character_ids.is_empty());
    }
  }

  mod shell_messages {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_drags_and_settles_the_entries_pane() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state();
      state.set_pane_host_width(1_200.0);

      let start = state.entries_pane.width();
      let _ = update(&mut state, Message::PaneDragStart, &db);
      let _ = update(&mut state, Message::PaneDrag(500.0), &db);
      let _ = update(&mut state, Message::PaneDrag(540.0), &db);
      let settled = state.entries_pane.width();
      let _ = update(&mut state, Message::PaneDragEnd, &db);
      let ratio = state.entries_pane.ratio();
      let _ = update(&mut state, Message::PaneSettled(ENTRIES_PANE_KEY, ratio), &db);

      assert!((settled - (start + 40.0)).abs() < 0.01);
      assert!(!state.entries_pane.is_active());
    }

    #[tokio::test]
    async fn it_steps_the_jump_calendar_across_a_year_boundary() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state();
      state.jump_picker.show_month(2026, 0);

      let _ = update(&mut state, Message::Header(header::Message::PrevMonth), &db);
      assert_eq!(
        (state.jump_picker.view_year(), state.jump_picker.view_month0()),
        (2025, 11)
      );

      let _ = update(&mut state, Message::Header(header::Message::NextMonth), &db);
      assert_eq!(
        (state.jump_picker.view_year(), state.jump_picker.view_month0()),
        (2026, 0)
      );
    }
  }

  mod pagination {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_loads_the_next_page_when_scrolled_near_the_end() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state();
      state.days = vec![day("2026-07-05")];
      state.all_dates = vec!["2026-07-05".to_owned(), "2026-07-04".to_owned()];

      let _ = update(&mut state, Message::EntriesScrolled(0.95), &db);

      assert!(state.loading_more);
    }

    #[tokio::test]
    async fn it_ignores_scroll_before_the_threshold_or_when_exhausted() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state();
      state.days = vec![day("2026-07-05")];
      state.all_dates = vec!["2026-07-05".to_owned()];

      let _ = update(&mut state, Message::EntriesScrolled(0.2), &db);
      assert!(!state.loading_more);

      let _ = update(&mut state, Message::EntriesScrolled(1.0), &db);
      assert!(!state.loading_more, "fully loaded list never re-fetches");
    }

    #[tokio::test]
    async fn it_appends_a_page_once_and_keeps_newest_first_order() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state();
      state.days = vec![day("2026-07-05")];
      state.loading_more = true;
      let page = MorePage {
        days: vec![day("2026-07-03"), day("2026-07-05"), day("2026-07-04")],
        event_notes: HashMap::new(),
        event_owners: HashMap::new(),
      };

      let _ = update(&mut state, Message::MoreDays(Box::new(page)), &db);

      let order: Vec<&str> = state.days.iter().map(|day| day.date_iso.as_str()).collect();
      assert_eq!(order, vec!["2026-07-05", "2026-07-04", "2026-07-03"]);
      assert!(!state.loading_more);
    }
  }

  mod build_day {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_assembles_a_day_from_an_empty_store() {
      let db = crate::store::open_test().await.unwrap();

      let built = build_day(&db, &PromptConfig::default(), &[7, 9], "2026-07-05").await;

      assert_eq!(built.date_iso, "2026-07-05");
      assert_eq!(built.kill_count, 0);
      // No recorded activity falls back to the full roster count.
      assert_eq!(built.pilot_count, 2);
    }
  }

  mod load_event_notes {
    use super::*;

    #[tokio::test]
    async fn it_yields_empty_maps_when_no_owner_is_known() {
      let db = crate::store::open_test().await.unwrap();
      let mut day = stub_day("2026-07-05");
      day.events = vec![CalendarEntry {
        event_id: 42,
        response: "accepted".to_owned(),
        timestamp: "2026-07-05T10:00:00Z".to_owned(),
        title: "Fleet op".to_owned(),
      }];

      let (notes, owners) = load_event_notes(&db, &[day]).await;

      assert!(notes.is_empty(), "an unowned event resolves no note");
      assert!(owners.is_empty(), "no owned character claims the event");
    }
  }

  mod route_events {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_routes_every_event_message() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state();

      let _ = route_events(&mut state, &db, events::Message::EditRequested(42));
      assert!(state.event_editing.is_some(), "editing arms a draft");

      let _ = route_events(&mut state, &db, events::Message::DraftChanged("draft".to_owned()));
      assert_eq!(
        state.event_editing.as_ref().map(|edit| edit.draft.as_str()),
        Some("draft")
      );

      let _ = route_events(
        &mut state,
        &db,
        events::Message::NoteChanged(42, "saved note".to_owned()),
      );
      assert_eq!(state.event_notes.get(&42).map(String::as_str), Some("saved note"));
      assert!(state.event_editing.is_none(), "saving closes the editor");

      let _ = route_events(&mut state, &db, events::Message::EditCancelled);
      let _ = route_events(&mut state, &db, events::Message::NoteSaved);
    }
  }

  mod pilot_count {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_counts_distinct_pilots_and_falls_back_to_the_roster() {
      let combat = rollup::Combat {
        engagements: vec![CombatKill {
          character_id: 7,
          is_kill: true,
          kill_time: "2026-07-05T10:00:00Z".to_owned(),
          killmail_id: 1,
          ship_type_id: 2,
          system_id: 3,
          value_isk: 1.0,
        }],
        kill_count: 1,
        kill_value: 1.0,
        loss_count: 0,
        loss_value: 0.0,
      };

      assert_eq!(super::pilot_count(&combat, &[], &[], &[7, 8, 9]), 1);
      assert_eq!(super::pilot_count(&empty_combat(), &[], &[], &[7, 8, 9]), 3);
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
        Vec::new(),
        prompts::all_field_prompts(&PromptConfig::default()),
      ));

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_collects_no_stale_images() {
      let state = loaded_state();

      assert!(state.stale_images().is_empty());
    }
  }

  mod objective_links {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{LinkSource, NewObjective};

    fn new_objective(title: &str) -> NewObjective {
      NewObjective {
        accent: "#5BB97E".to_owned(),
        horizon: None,
        target: None,
        title: title.to_owned(),
        why: None,
      }
    }

    async fn loaded(db: &Database) -> State {
      let mut state = State::new();
      let snapshot = build_snapshot(db, &[]).await;
      let _ = install_snapshot(&mut state, db, snapshot);
      state
    }

    #[tokio::test]
    async fn it_opens_the_inline_picker_from_an_objective_link_message() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded(&db).await;
      let iso = state.today_iso.clone();
      let source = LinkSource::LogAnswer {
        question_id: "goal".to_owned(),
      };

      let _ = update(
        &mut state,
        Message::ObjectiveLink(objective_link::Message::Toggle {
          date: iso.clone(),
          source: source.clone(),
        }),
        &db,
      );

      assert!(state.objective_link.is_open_for(&iso, &source));
    }

    #[tokio::test]
    async fn it_sets_and_clears_a_day_link_reflected_in_the_panel_state() {
      let db = crate::store::open_test().await.unwrap();
      let created = objective::create(&db, &new_objective("Fund a Nyx")).await.unwrap();
      let mut state = loaded(&db).await;
      let iso = state.today_iso.clone();
      let source = LinkSource::LogAnswer {
        question_id: "goal".to_owned(),
      };

      // Setting a link (the picker's write path) surfaces it in the day panel state.
      objective::set_link(&db, created.id, &iso, &source).await.unwrap();
      state.objective_link.apply(objective_link::reload_data(&db, &iso).await);
      assert_eq!(state.objective_link.linked(&iso, &source), Some(created.id));

      // Clearing it removes it from the panel state again.
      objective::clear_link(&db, created.id, &iso, &source).await.unwrap();
      state.objective_link.apply(objective_link::reload_data(&db, &iso).await);
      assert_eq!(state.objective_link.linked(&iso, &source), None);
    }

    #[tokio::test]
    async fn it_threads_a_linked_answer_onto_the_objective() {
      let db = crate::store::open_test().await.unwrap();
      let created = objective::create(&db, &new_objective("Break the doctrine"))
        .await
        .unwrap();
      let source = LinkSource::LogAnswer {
        question_id: "goal".to_owned(),
      };
      captains_log::upsert_answer(&db, "2026-07-05", "goal", Some("Undock the barge."))
        .await
        .unwrap();
      objective::set_link(&db, created.id, "2026-07-05", &source)
        .await
        .unwrap();

      let thread = objective::thread(&db, created.id).await.unwrap();

      assert_eq!(thread.len(), 1);
      assert_eq!(thread[0].source_kind, "log_answer");
      assert_eq!(thread[0].text.as_deref(), Some("Undock the barge."));
    }
  }
}
