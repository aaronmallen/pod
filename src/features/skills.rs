pub mod attributes;
pub mod browse;
pub mod format;
mod header;
mod layout_shell;
pub mod optimizer;
pub mod plan_math;
mod queue;
mod queue_section;
pub mod queue_timing;
mod right_panel;
mod training_hero;
mod warning_strip;

use std::collections::HashMap;

use chrono::{DateTime, Datelike as _, Duration, Timelike as _, Utc};
use iced::{
  Element, Length, Padding, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Stack, container, scrollable, text},
};

pub use self::right_panel::RightTab;
use self::{
  layout_shell::layout_shell,
  optimizer::{Attribute, Attributes},
  right_panel::Panel,
};
pub use crate::features::skill_plan_editor::Seed as EditorSeed;
use crate::{
  store::{
    Database, images,
    model::{CharacterSkillqueue, OwnerType},
    repo::{character, infra, org, skills},
  },
  ui::{
    components::{
      backdrop,
      modal_overlay::modal_overlay,
      resizable_pane::{self, PaneDrag, pane_handle},
    },
    style::{color, spacing, typography},
  },
  window_state::UiState,
};

const LEFT_PANE_KEY: &str = "skills.left";
const LEFT_PANE_DEFAULT: f32 = 845.0;

const HEADER_SIDE_PADDING: f32 = 28.0;
const PICKER_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;
const PICKER_OVERLAY_LEFT: f32 = HEADER_SIDE_PADDING;
const MONTHS: [&str; 12] = [
  "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const SECONDS_PER_DAY: i64 = 86_400;

#[derive(Clone, Debug)]
pub struct Loaded {
  attributes: Option<attributes::AttrTabModel>,
  computed: queue::ComputedQueue,
  queue: Vec<CharacterSkillqueue>,
  roster: Vec<PickerPilot>,
}

#[derive(Clone, Debug)]
pub enum Message {
  CharacterChanged(i64),
  Loaded(Box<Loaded>),
  OpenCompare,
  OpenPlanEditor(EditorSeed),
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart,
  PaneSettled(&'static str, f32),
  PickerToggled,
  RightPanel(right_panel::Message),
}

#[derive(Debug)]
pub struct State {
  active: i64,
  attributes: Option<attributes::AttrTabModel>,
  browse: right_panel::browser_tab::State,
  computed: queue::ComputedQueue,
  left_pane: PaneDrag,
  picker_open: bool,
  plans: right_panel::plans_tab::State,
  queue: Vec<CharacterSkillqueue>,
  roster: Vec<PickerPilot>,
  tab: RightTab,
}

impl State {
  pub fn new(active: i64) -> Self {
    State {
      active,
      attributes: None,
      browse: right_panel::browser_tab::State::new(),
      computed: queue::ComputedQueue::default(),
      left_pane: PaneDrag::new(LEFT_PANE_DEFAULT, spacing::layout::WINDOW_DEFAULT_WIDTH),
      plans: right_panel::plans_tab::State::new(),
      queue: Vec::new(),
      picker_open: false,
      roster: Vec::new(),
      tab: RightTab::default(),
    }
  }

  pub fn with_restored_panes(mut self, ui: &UiState) -> Self {
    let host_width = ui.host_width("main", spacing::layout::WINDOW_DEFAULT_WIDTH);
    self.left_pane = PaneDrag::from_store(ui, LEFT_PANE_KEY, LEFT_PANE_DEFAULT, host_width);
    self
  }

  pub fn set_pane_host_width(&mut self, host_width: f32) {
    self.left_pane.set_host_width(host_width);
  }

  pub fn active(&self) -> i64 {
    self.active
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    self
      .roster
      .iter()
      .filter_map(|pilot| pilot.portrait.stale_key())
      .collect()
  }
}

#[derive(Clone, Debug)]
struct PickerPilot {
  corp: String,
  granted_scopes: Option<String>,
  id: i64,
  name: String,
  portrait: images::ImageState,
  total_sp: i64,
}

pub fn load(db: &Database, character_id: i64, owned: Vec<i64>) -> Task<Message> {
  let summary = Task::perform(load_summary(db.clone(), character_id, owned), |loaded| {
    Message::Loaded(Box::new(loaded))
  });
  let browse = right_panel::browser_tab::load(db, character_id, Utc::now())
    .map(right_panel::Message::Browse)
    .map(Message::RightPanel);
  let plans = right_panel::plans_tab::load(db, character_id)
    .map(right_panel::Message::Plans)
    .map(Message::RightPanel);
  Task::batch([summary, browse, plans])
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::CharacterChanged(id) => {
      state.active = id;
      state.picker_open = false;
      Task::none()
    }
    Message::Loaded(loaded) => {
      let Loaded {
        attributes,
        computed,
        queue,
        roster,
      } = *loaded;
      state.attributes = attributes;
      state.computed = computed;
      state.queue = queue;
      state.roster = roster;
      Task::none()
    }
    Message::OpenCompare => Task::none(),
    Message::OpenPlanEditor(_) => Task::none(),
    Message::PaneDrag(x) => {
      state.left_pane.drag_to(x);
      Task::none()
    }
    Message::PaneDragEnd => {
      state.left_pane.end();
      Task::done(Message::PaneSettled(LEFT_PANE_KEY, state.left_pane.ratio()))
    }
    Message::PaneDragStart => {
      state.left_pane.start();
      Task::none()
    }
    Message::PaneSettled(..) => Task::none(),
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Task::none()
    }
    Message::RightPanel(msg) => update_right_panel(state, msg, db),
  }
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  if !state.left_pane.is_active() {
    return iced::Subscription::none();
  }
  iced::event::listen_with(|event, _status, _id| {
    resizable_pane::drag_event(event, Message::PaneDrag, Message::PaneDragEnd)
  })
}

pub fn reload_plans(db: &Database, character_id: i64) -> Task<Message> {
  right_panel::plans_tab::load(db, character_id)
    .map(right_panel::Message::Plans)
    .map(Message::RightPanel)
}

fn update_right_panel(state: &mut State, message: right_panel::Message, db: &Database) -> Task<Message> {
  match message {
    right_panel::Message::Browse(msg) => right_panel::browser_tab::update(&mut state.browse, msg)
      .map(right_panel::Message::Browse)
      .map(Message::RightPanel),
    right_panel::Message::Plans(right_panel::plans_tab::Message::NewPlan) => {
      Task::done(Message::OpenPlanEditor(EditorSeed::New))
    }
    right_panel::Message::Plans(right_panel::plans_tab::Message::FromQueue) => {
      Task::done(Message::OpenPlanEditor(EditorSeed::FromQueue))
    }
    right_panel::Message::Plans(right_panel::plans_tab::Message::OpenPlan(plan_id)) => {
      Task::done(Message::OpenPlanEditor(EditorSeed::Existing(plan_id)))
    }
    right_panel::Message::Plans(msg) => right_panel::plans_tab::update(&mut state.plans, msg, db, state.active)
      .map(right_panel::Message::Plans)
      .map(Message::RightPanel),
    right_panel::Message::TabSelected(tab) => {
      state.tab = tab;
      Task::none()
    }
  }
}

pub fn view<'a>(
  state: &'a State,
  _id: i64,
  _status: &'a crate::sync::SyncStatus,
  now: DateTime<Utc>,
) -> Element<'a, Message> {
  if state.roster.is_empty() {
    return empty_state();
  }

  let body = Column::with_children(vec![header::header(state, now), panes(state, now)])
    .width(Length::Fill)
    .height(Length::Fill);

  let base = layout_shell(body);

  if state.picker_open {
    let dropdown = container(header::picker_dropdown(state)).padding(Padding {
      top: PICKER_OVERLAY_TOP,
      left: PICKER_OVERLAY_LEFT,
      ..Padding::ZERO
    });

    let overlay = Stack::with_children(vec![backdrop::click_catcher(Message::PickerToggled), dropdown.into()])
      .width(Length::Fill)
      .height(Length::Fill)
      .into();
    return modal_overlay(base, None, overlay);
  }

  base
}

async fn load_summary(db: Database, character_id: i64, owned: Vec<i64>) -> Loaded {
  let mut roster = Vec::with_capacity(owned.len());
  for id in owned {
    roster.push(picker_pilot(&db, id).await);
  }

  let now = Utc::now();
  let queue = queue_timing::active_queue(character::skillqueue(&db, character_id).await.unwrap_or_default(), now);

  let computed = queue::load_computed_queue(&db, character_id, now).await;

  let attributes = load_attributes_tab(&db, character_id, &queue).await;

  Loaded {
    attributes,
    computed,
    queue,
    roster,
  }
}

async fn load_attributes_tab(
  db: &Database,
  character_id: i64,
  queue: &[CharacterSkillqueue],
) -> Option<attributes::AttrTabModel> {
  let row = character::attributes(db, character_id).await.ok().flatten()?;

  let mut implants = Attributes::default();
  for implant in character::implants(db, character_id).await.unwrap_or_default() {
    let bonus = implant.bonus().max(0) as u32;
    match attributes::attribute_from_neural_id(implant.attribute_id()) {
      Attribute::Charisma => implants.charisma += bonus,
      Attribute::Intelligence => implants.intelligence += bonus,
      Attribute::Memory => implants.memory += bonus,
      Attribute::Perception => implants.perception += bonus,
      Attribute::Willpower => implants.willpower += bonus,
    }
  }

  let weight_meta = load_weight_meta(db, character_id, queue).await;
  let weights = attributes::queue_pair_weights(queue, &weight_meta);
  let active = queue
    .first()
    .map(CharacterSkillqueue::skill_id)
    .and_then(|skill_id| weight_meta.get(&skill_id))
    .map(|skill| (skill.primary, skill.secondary));

  Some(attributes::AttrTabModel::new(&row, implants, active, &weights))
}

async fn load_weight_meta(
  db: &Database,
  character_id: i64,
  queue: &[CharacterSkillqueue],
) -> HashMap<i64, attributes::WeightSkill> {
  let sp_by_skill: HashMap<i64, u64> = character::skills(db, character_id)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|skill| (skill.skill_id(), skill.skillpoints_in_skill().max(0) as u64))
    .collect();

  let mut meta = HashMap::new();
  for skill_id in queue.iter().map(CharacterSkillqueue::skill_id) {
    if meta.contains_key(&skill_id) {
      continue;
    }
    let Some(row) = skills::get_skill_metadata(db, skill_id).await.ok().flatten() else {
      continue;
    };
    meta.insert(
      skill_id,
      attributes::WeightSkill {
        primary: attributes::attribute_from_neural_id(row.primary_attribute()),
        rank: row.rank().max(1) as f64,
        secondary: attributes::attribute_from_neural_id(row.secondary_attribute()),
        skillpoints_in_skill: sp_by_skill.get(&skill_id).copied().unwrap_or(0),
      },
    );
  }
  meta
}

async fn picker_pilot(db: &Database, id: i64) -> PickerPilot {
  let character = character::get(db, id).await.ok().flatten();
  let name = character.as_ref().map(|c| c.name().to_owned()).unwrap_or_default();
  let corp = match character.as_ref().map(|c| c.corporation_id()) {
    Some(corp_id) => org::get_corporation(db, corp_id)
      .await
      .ok()
      .flatten()
      .map(|c| c.ticker().to_owned())
      .unwrap_or_default(),
    None => String::new(),
  };
  let total_sp = character::state(db, id)
    .await
    .ok()
    .flatten()
    .and_then(|state| state.total_sp)
    .unwrap_or(0);
  let portrait = images::resolve(&images::default_store(), images::ImageKind::CharacterPortrait, id);
  let granted_scopes = infra::get(db, id, OwnerType::Character)
    .await
    .ok()
    .flatten()
    .and_then(|credential| credential.scopes().clone());

  PickerPilot {
    corp,
    granted_scopes,
    id,
    name,
    portrait,
    total_sp,
  }
}

fn empty_state<'a>() -> Element<'a, Message> {
  container(
    text("Add a character to view skills")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .into()
}

pub(super) fn fmt_duration(seconds: i64) -> String {
  if seconds <= 0 {
    return "—".to_owned();
  }
  let days = seconds / SECONDS_PER_DAY;
  let hours = (seconds % SECONDS_PER_DAY) / 3_600;
  let minutes = (seconds % 3_600) / 60;
  if days > 0 {
    format!("{days}d {hours:02}h {minutes:02}m")
  } else if hours > 0 {
    format!("{hours}h {minutes:02}m")
  } else {
    format!("{minutes}m")
  }
}

pub(super) fn fmt_eta(now: DateTime<Utc>, seconds_from_now: i64) -> String {
  if seconds_from_now <= 0 {
    return "—".to_owned();
  }
  let eta = now + Duration::seconds(seconds_from_now);
  let day = eta.day();
  let month = MONTHS[(eta.month() - 1) as usize];
  let year = eta.year();
  let hour = eta.hour();
  let minute = eta.minute();
  format!("{day} {month} {year} · {hour:02}:{minute:02}")
}

pub(super) fn fmt_sp(sp: i64) -> String {
  if sp >= 1_000_000 {
    format!("{:.2}M", sp as f64 / 1_000_000.0)
  } else if sp >= 1_000 {
    format!("{:.0}K", sp as f64 / 1_000.0)
  } else {
    sp.to_string()
  }
}

fn panes<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let head = state.queue.first();
  let mut left_children: Vec<Element<'a, Message>> = Vec::with_capacity(3);
  left_children.push(training_hero::training_hero(&state.computed, head, now));
  if let Some(strip) = warning_strip::warning_strip(&state.computed, head) {
    left_children.push(strip);
  }
  left_children.push(queue_section::queue_section(&state.computed, head, now));

  let left = scrollable(
    Column::with_children(left_children)
      .spacing(spacing::SPACE_3)
      .width(Length::Fill),
  )
  .style(crate::ui::style::control::scrollbar)
  .width(Length::Fixed(state.left_pane.width()))
  .height(Length::Fill);

  Row::with_children(vec![
    left.into(),
    pane_handle(Message::PaneDragStart),
    right_panel(state, now),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn right_panel<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  Panel {
    attributes: state.attributes.as_ref(),
    browse: &state.browse,
    now,
    plans: &state.plans,
    tab: state.tab,
  }
  .render()
  .map(Message::RightPanel)
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
  DateTime::parse_from_rfc3339(value)
    .ok()
    .map(|dt| dt.with_timezone(&Utc))
}

fn queue_remaining_seconds(queue: &[CharacterSkillqueue], now: DateTime<Utc>) -> Option<i64> {
  let latest = queue
    .iter()
    .filter_map(|entry| entry.finish_date().as_deref())
    .filter_map(parse_timestamp)
    .max()?;
  Some((latest - now).num_seconds().max(0))
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone as _;

  use super::*;

  fn entry(finish_date: Option<&str>) -> CharacterSkillqueue {
    CharacterSkillqueue {
      character_id: 42,
      finish_date: finish_date.map(ToOwned::to_owned),
      finished_level: 5,
      level_end_sp: None,
      level_start_sp: None,
      queue_position: 0,
      skill_id: 3300,
      start_date: None,
      training_start_sp: None,
    }
  }

  fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap()
  }

  fn pilot(id: i64, name: &str) -> PickerPilot {
    PickerPilot {
      corp: "TEST".to_owned(),
      granted_scopes: None,
      id,
      name: name.to_owned(),
      portrait: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      },
      total_sp: 47_320_400,
    }
  }

  mod fmt_duration {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_pads_hours_and_minutes_when_days_are_present() {
      assert_eq!(fmt_duration(3 * 86_400 + 4 * 3_600 + 9 * 60), "3d 04h 09m");
    }

    #[test]
    fn it_pads_only_minutes_when_hours_are_present() {
      assert_eq!(fmt_duration(2 * 3_600 + 5 * 60), "2h 05m");
    }

    #[test]
    fn it_renders_an_em_dash_for_zero_or_negative() {
      assert_eq!(fmt_duration(0), "—");
      assert_eq!(fmt_duration(-5), "—");
    }

    #[test]
    fn it_renders_bare_minutes_below_an_hour() {
      assert_eq!(fmt_duration(7 * 60), "7m");
    }

    #[test]
    fn it_truncates_a_sub_minute_remainder() {
      assert_eq!(fmt_duration(59), "0m");
    }
  }

  mod fmt_eta {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_the_eve_time_instant_with_the_year() {
      assert_eq!(fmt_eta(now(), 2 * 3_600 + 30 * 60), "1 Jun 2026 · 14:30");
    }

    #[test]
    fn it_renders_an_em_dash_for_zero_or_negative() {
      assert_eq!(fmt_eta(now(), 0), "—");
      assert_eq!(fmt_eta(now(), -10), "—");
    }

    #[test]
    fn it_rolls_into_a_later_day_and_month() {
      let secs = 30 * 86_400 + 3 * 3_600 + 5 * 60;

      assert_eq!(fmt_eta(now(), secs), "1 Jul 2026 · 15:05");
    }

    #[test]
    fn it_rolls_into_a_later_year() {
      let secs = 250 * 86_400;

      assert!(fmt_eta(now(), secs).ends_with("2027 · 12:00"));
    }
  }

  mod fmt_sp {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_integers_below_one_thousand() {
      assert_eq!(fmt_sp(0), "0");
      assert_eq!(fmt_sp(999), "999");
    }

    #[test]
    fn it_renders_millions_with_two_decimals() {
      assert_eq!(fmt_sp(1_000_000), "1.00M");
      assert_eq!(fmt_sp(47_320_400), "47.32M");
    }

    #[test]
    fn it_renders_thousands_rounded_to_whole_k() {
      assert_eq!(fmt_sp(1_000), "1K");
      assert_eq!(fmt_sp(45_255), "45K");
    }
  }

  mod queue_remaining_seconds {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_a_past_finish_to_zero() {
      let queue = vec![entry(Some("2026-05-30T12:00:00Z"))];

      assert_eq!(queue_remaining_seconds(&queue, now()), Some(0));
    }

    #[test]
    fn it_ignores_null_finish_dates_among_real_ones() {
      let queue = vec![entry(None), entry(Some("2026-06-01T13:00:00Z")), entry(None)];

      assert_eq!(queue_remaining_seconds(&queue, now()), Some(3_600));
    }

    #[test]
    fn it_spans_now_to_the_finish_for_a_future_single_entry() {
      let queue = vec![entry(Some("2026-06-01T13:30:00Z"))];

      assert_eq!(queue_remaining_seconds(&queue, now()), Some(5_400));
    }

    #[test]
    fn it_spans_to_the_latest_finish_across_a_multi_entry_queue() {
      let queue = vec![
        entry(Some("2026-06-01T13:00:00Z")),
        entry(Some("2026-06-03T12:00:00Z")),
        entry(Some("2026-06-02T18:00:00Z")),
      ];

      assert_eq!(queue_remaining_seconds(&queue, now()), Some(2 * 86_400));
    }

    #[test]
    fn it_yields_none_for_an_empty_queue() {
      assert_eq!(queue_remaining_seconds(&[], now()), None);
    }

    #[test]
    fn it_yields_none_when_no_finish_date_parses() {
      let queue = vec![entry(None), entry(Some("not-a-date"))];

      assert_eq!(queue_remaining_seconds(&queue, now()), None);
    }
  }

  mod view {
    use super::*;

    fn status() -> crate::sync::SyncStatus {
      crate::sync::SyncStatus::new()
    }

    #[test]
    fn it_renders_a_loaded_state_with_a_queue() {
      let mut state = State::new(42);
      state.roster = vec![pilot(42, "Test Pilot"), pilot(7, "Wingmate")];
      state.queue = vec![entry(Some("2026-06-03T12:00:00Z"))];
      let status = status();

      let _el: Element<'_, Message> = view(&state, 42, &status, now());
    }

    #[test]
    fn it_renders_a_loaded_state_with_an_empty_queue() {
      let mut state = State::new(42);
      state.roster = vec![pilot(42, "Test Pilot")];
      let status = status();

      let _el: Element<'_, Message> = view(&state, 42, &status, now());
    }

    #[test]
    fn it_renders_the_empty_state_with_zero_owned_pilots() {
      let state = State::new(42);
      let status = status();

      let _el: Element<'_, Message> = view(&state, 42, &status, now());
    }

    #[test]
    fn it_renders_the_attributes_tab_when_selected() {
      let mut state = State::new(42);
      state.roster = vec![pilot(42, "Test Pilot")];
      state.tab = RightTab::Attributes;
      let status = status();

      let _el: Element<'_, Message> = view(&state, 42, &status, now());
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_switches_the_right_panel_tab() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(
        &mut state,
        Message::RightPanel(right_panel::Message::TabSelected(RightTab::Attributes)),
        &db,
      );
      assert_eq!(state.tab, RightTab::Attributes);

      let _ = update(
        &mut state,
        Message::RightPanel(right_panel::Message::TabSelected(RightTab::Plans)),
        &db,
      );
      assert_eq!(state.tab, RightTab::Plans);

      let _ = update(
        &mut state,
        Message::RightPanel(right_panel::Message::TabSelected(RightTab::Browse)),
        &db,
      );
      assert_eq!(state.tab, RightTab::Browse);
    }

    #[tokio::test]
    async fn it_forwards_browse_messages_to_the_browse_tab() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(
        &mut state,
        Message::RightPanel(right_panel::Message::Browse(
          right_panel::browser_tab::Message::SearchChanged("gun".to_owned()),
        )),
        &db,
      );
    }

    #[tokio::test]
    async fn the_plan_seams_bubble_up_without_touching_screen_state() {
      let db = crate::store::open_test().await.unwrap();

      for seam in [
        right_panel::plans_tab::Message::NewPlan,
        right_panel::plans_tab::Message::FromQueue,
        right_panel::plans_tab::Message::OpenPlan(7),
      ] {
        let mut state = State::new(42);
        let _ = update(&mut state, Message::RightPanel(right_panel::Message::Plans(seam)), &db);
        assert_eq!(state.active, 42);
      }
    }

    #[tokio::test]
    async fn the_open_plan_editor_bubble_is_a_feature_no_op() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::OpenPlanEditor(EditorSeed::New), &db);

      assert_eq!(state.active, 42);
    }
  }

  mod left_pane {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_the_left_pane_width_when_the_store_is_empty() {
      let state = State::new(42).with_restored_panes(&UiState::default());

      assert_eq!(state.left_pane.width(), LEFT_PANE_DEFAULT);
    }

    #[test]
    fn it_restores_the_left_pane_width_from_the_keyed_store() {
      let mut ui = UiState::default();
      ui.panes.insert(LEFT_PANE_KEY.to_owned(), 540.0);

      let state = State::new(42).with_restored_panes(&ui);

      assert_eq!(state.left_pane.width(), 540.0);
    }

    #[tokio::test]
    async fn it_resizes_the_left_pane_during_a_drag_and_bubbles_the_settled_width() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42);

      let _ = update(&mut state, Message::PaneDragStart, &db);
      let _ = update(&mut state, Message::PaneDrag(500.0), &db);
      let _ = update(&mut state, Message::PaneDrag(560.0), &db);
      assert_eq!(state.left_pane.width(), LEFT_PANE_DEFAULT + 60.0);
      assert!(state.left_pane.is_active());

      let task = update(&mut state, Message::PaneDragEnd, &db);
      assert!(!state.left_pane.is_active());
      let _ = task;
    }

    #[tokio::test]
    async fn it_records_the_new_active_pilot_and_closes_the_picker_on_character_changed() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42);
      state.picker_open = true;

      let _ = update(&mut state, Message::CharacterChanged(7), &db);

      assert_eq!(state.active, 7);
      assert!(!state.picker_open);
    }

    #[tokio::test]
    async fn it_adopts_the_loaded_screen_model() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42);

      let _ = update(
        &mut state,
        Message::Loaded(Box::new(Loaded {
          attributes: None,
          computed: queue::ComputedQueue::default(),
          queue: vec![entry(Some("2026-06-03T12:00:00Z"))],
          roster: vec![pilot(42, "Test Pilot")],
        })),
        &db,
      );

      assert_eq!(state.roster.len(), 1);
      assert_eq!(state.queue.len(), 1);
    }
  }

  async fn seed_character(db: &Database, id: i64, name: &str) {
    use crate::store::model::{Alliance, Bloodline, Character, Corporation, Gender, Race};

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
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, name);
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  mod picker_pilot {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_degrades_to_empty_fields_for_an_unknown_character() {
      let db = crate::store::open_test().await.unwrap();

      let pilot = super::picker_pilot(&db, 999).await;

      assert_eq!(pilot.id, 999);
      assert!(pilot.name.is_empty());
      assert!(pilot.corp.is_empty());
      assert_eq!(pilot.total_sp, 0);
      assert_eq!(
        pilot.portrait.stale_key(),
        Some((images::ImageKind::CharacterPortrait, 999))
      );
    }

    #[tokio::test]
    async fn it_resolves_the_name_and_corp_ticker_for_a_seeded_character() {
      let db = crate::store::open_test().await.unwrap();
      seed_character(&db, 42, "Test Pilot").await;

      let pilot = super::picker_pilot(&db, 42).await;

      assert_eq!(pilot.id, 42);
      assert_eq!(pilot.name, "Test Pilot");
      assert_eq!(pilot.corp, "TSC");
    }
  }

  mod stale_images {
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collects_a_stale_key_per_pilot_with_a_missing_portrait() {
      let mut state = State::new(42);
      state.roster = vec![pilot(42, "Test Pilot"), pilot(7, "Wingmate")];

      assert_eq!(
        state.stale_images(),
        vec![
          (images::ImageKind::CharacterPortrait, 42),
          (images::ImageKind::CharacterPortrait, 7),
        ]
      );
    }

    #[test]
    fn it_skips_pilots_with_a_fresh_portrait() {
      let mut fresh = pilot(42, "Test Pilot");
      fresh.portrait = images::ImageState::Fresh(PathBuf::from("/cache/42.jpg"));
      let mut state = State::new(42);
      state.roster = vec![fresh];

      assert!(state.stale_images().is_empty());
    }
  }

  mod load_attributes_tab {
    use super::*;
    use crate::store::model::{CharacterAttributes, CharacterImplant};

    fn attributes(character_id: i64) -> CharacterAttributes {
      CharacterAttributes {
        accrued_remap_cooldown_date: None,
        bonus_remaps: 2,
        character_id,
        charisma: 19,
        intelligence: 20,
        last_remap_date: None,
        memory: 21,
        perception: 27,
        unallocated_sp: 0,
        willpower: 24,
      }
    }

    #[tokio::test]
    async fn it_is_none_without_a_synced_attributes_row() {
      let db = crate::store::open_test().await.unwrap();

      assert!(super::load_attributes_tab(&db, 42, &[]).await.is_none());
    }

    #[tokio::test]
    async fn it_assembles_the_model_summing_implant_bonuses() {
      let db = crate::store::open_test().await.unwrap();
      seed_character(&db, 42, "Test Pilot").await;
      character::upsert_attributes(&db, &attributes(42)).await.unwrap();
      character::replace_implants(
        &db,
        42,
        &[CharacterImplant {
          attribute_id: 167,
          bonus: 5,
          character_id: 42,
        }],
      )
      .await
      .unwrap();

      let queue = vec![entry(Some("2026-06-03T12:00:00Z"))];
      let model = super::load_attributes_tab(&db, 42, &queue).await;

      assert!(model.is_some());
    }
  }

  mod load_summary {
    use super::*;

    #[tokio::test]
    async fn it_assembles_the_roster_queue_and_computed_model() {
      let db = crate::store::open_test().await.unwrap();
      seed_character(&db, 42, "Test Pilot").await;
      character::replace_skillqueue(&db, 42, &[entry(Some("2999-06-03T12:00:00Z"))])
        .await
        .unwrap();

      let loaded = super::load_summary(db, 42, vec![42]).await;

      assert_eq!(loaded.roster.len(), 1);
      assert_eq!(loaded.roster[0].name, "Test Pilot");
      assert_eq!(loaded.queue.len(), 1);
    }
  }
}
