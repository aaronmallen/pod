use std::{
  collections::HashMap,
  path::PathBuf,
  sync::{OnceLock, RwLock},
};

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, text, text_input},
};

use crate::{
  config::{self, LogLevel, Settings},
  features::settings::{features_tab, features_tab::Group},
  i18n::Language,
  ui::{
    components::{
      eve_time::eve_time, icon::Icon, progress_bar::progress_bar, rule, status::dot, status_bar::status_bar, toggle,
    },
    style::{color, control, radius, spacing, typography},
  },
};

const BENEFIT_CARD_MAX_WIDTH: f32 = 720.0;
const LANGUAGE_GRID_COLUMNS: usize = 3;
const LANGUAGE_GRID_MAX_WIDTH: f32 = 860.0;
const STEP_TITLE_SIZE: f32 = 30.0;
const STORAGE_MAX_WIDTH: f32 = 920.0;

const CONTENT_PADDING: f32 = 48.0;
const FOOTER_SIDE_PADDING: f32 = 48.0;
const RAIL_WIDTH: f32 = 300.0;

// Interns a resolved translation as a `&'static str`, for the few widgets (the storage path field's
// placeholder) that need a borrowed string outliving the view. Mirrors the settings module's own
// interner; the leak is bounded by the small fixed key set.
fn tr_static(key: &str) -> &'static str {
  static CACHE: OnceLock<RwLock<HashMap<String, &'static str>>> = OnceLock::new();
  let cache = CACHE.get_or_init(|| RwLock::new(HashMap::new()));
  if let Some(&interned) = cache.read().expect("wizard i18n cache poisoned").get(key) {
    return interned;
  }

  let resolved: &'static str = Box::leak(t!(key).into_owned().into_boxed_str());
  cache
    .write()
    .expect("wizard i18n cache poisoned")
    .entry(key.to_owned())
    .or_insert(resolved)
}

#[derive(Clone, Debug)]
pub enum Message {
  Back,
  // The Finish step's completion action. The app's wizard update consumes the draft `Settings`,
  // persists it to config.toml (creating the configured storage dirs), then restarts into the normal
  // boot path — config now exists, so the next launch skips the wizard and runs the splash.
  Complete,
  // A per-group Features step toggle. The wizard routes this through the settings Features tab's own
  // `update` over the draft `Settings.features`, dropping its persist outcome (config is written once
  // at Finish), so the wizard and the settings tab mutate the same flag model.
  Features(features_tab::Message),
  // The Features sub-step model derives its rail counts from the live feature flags the
  // per-group step tasks will own; this is the seam they dispatch when a flag flips.
  #[allow(dead_code)]
  JumpTo(usize),
  Next,
  // Selecting a language re-renders the whole wizard in that language on the next frame, before any
  // config is written (the view resolves keys against `pending_language`). The language grid
  // dispatches this; the app's wizard update applies the locale so the next frame renders in it.
  SelectLanguage(Language),
  Skip,
  // The slimmed Storage step's actions. Each writes only into the draft `Settings.storage` (a path
  // override, the log level, or the sync flag) — first run has no data, so there is no migration,
  // export, or relocation machinery behind any of these.
  StorageBrowse(PathKind),
  StorageLogLevel(LogLevel),
  StoragePathEdited(PathKind, String),
  StoragePathSubmitted(PathKind),
  StorageReset(PathKind),
  StorageSyncToggled(bool),
}

/// One concrete position in the wizard. The flat step list is Welcome, Language, one
/// [`Features`](Step::Features) step per [`Group`], then Storage and Finish; [`steps`] builds it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
  Features(Group),
  Finish,
  Language,
  Storage,
  Welcome,
}

impl Step {
  pub fn phase(self) -> Phase {
    match self {
      Step::Features(_) => Phase::Features,
      Step::Finish => Phase::Finish,
      Step::Language => Phase::Language,
      Step::Storage => Phase::Storage,
      Step::Welcome => Phase::Welcome,
    }
  }
}

/// A rail entry. Several [`Step`]s collapse into the single [`Features`](Phase::Features) phase, so
/// the rail shows five rows while the flat step list is longer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
  Features,
  Finish,
  Language,
  Storage,
  Welcome,
}

impl Phase {
  pub const ALL: [Phase; 5] = [
    Phase::Welcome,
    Phase::Language,
    Phase::Features,
    Phase::Storage,
    Phase::Finish,
  ];

  fn glyph(self) -> Icon {
    match self {
      Phase::Features => Icon::settings(),
      Phase::Finish => Icon::pulse(),
      Phase::Language => Icon::market(),
      Phase::Storage => Icon::archive(),
      Phase::Welcome => Icon::star(),
    }
  }

  fn label_key(self) -> &'static str {
    match self {
      Phase::Features => "wizard.phase.features.label",
      Phase::Finish => "wizard.phase.finish.label",
      Phase::Language => "wizard.phase.language.label",
      Phase::Storage => "wizard.phase.storage.label",
      Phase::Welcome => "wizard.phase.welcome.label",
    }
  }
}

/// The three directories the slimmed Storage step lets the user repoint, mapped onto the matching
/// `StorageConfig` override. First run has no data to migrate, so each kind only reads the resolved
/// path and writes the override — there is no relocation or migration behind it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PathKind {
  Cache,
  Database,
  Log,
}

impl PathKind {
  const ALL: [PathKind; 3] = [PathKind::Database, PathKind::Log, PathKind::Cache];

  fn default_dir(self) -> PathBuf {
    match self {
      PathKind::Cache => config::cache_dir(),
      PathKind::Database => config::data_dir(),
      PathKind::Log => config::log_dir(),
    }
  }

  fn description_key(self) -> &'static str {
    match self {
      PathKind::Cache => "settings.storage.cache_description",
      PathKind::Database => "settings.storage.database_description",
      PathKind::Log => "settings.storage.log_description",
    }
  }

  fn label_key(self) -> &'static str {
    match self {
      PathKind::Cache => "settings.storage.cache_label",
      PathKind::Database => "settings.storage.database_label",
      PathKind::Log => "settings.storage.log_label",
    }
  }

  fn override_dir(self, settings: &Settings) -> Option<PathBuf> {
    let storage = settings.storage();
    match self {
      PathKind::Cache => storage.cache_dir().clone(),
      PathKind::Database => storage.db_dir().clone(),
      PathKind::Log => storage.log_dir().clone(),
    }
  }

  fn resolved_dir(self, settings: &Settings) -> PathBuf {
    let storage = settings.storage();
    match self {
      PathKind::Cache => storage.resolved_cache_dir(),
      PathKind::Database => storage.resolved_db_dir(),
      PathKind::Log => storage.resolved_log_dir(),
    }
  }

  // Writes only the path override (cleared to the default when it matches), never a relocation.
  fn set_dir(self, settings: &mut Settings, dir: Option<PathBuf>) {
    let storage = settings.storage_mut();
    match self {
      PathKind::Cache => storage.set_cache_dir(dir),
      PathKind::Database => storage.set_db_dir(dir),
      PathKind::Log => storage.set_log_dir(dir),
    };
  }
}

#[derive(Debug)]
pub struct State {
  current: usize,
  // The settings Features tab's own transient state (its search query), reused so the wizard's
  // per-group Features steps render the identical toggle rows over the same flag model.
  features: features_tab::State,
  // The in-progress language drives the rendered locale before any config write (ADR-0041 keeps the
  // committed locale fixed mid-session). The language grid reads/writes this through
  // `Message::SelectLanguage`; the app applies the locale so the next frame renders in it.
  pending_language: Language,
  // The in-progress configuration the Features and Storage steps mutate. It is written to disk once
  // at Finish (no per-step persistence), so the wizard owns its own draft instead of touching the
  // live settings until the run completes.
  settings: Settings,
  steps: Vec<Step>,
  // The editable path text per storage kind, kept in sync with the resolved override so a typed or
  // browsed path round-trips through the field before it commits to `Settings.storage`.
  storage_drafts: HashMap<PathKind, String>,
}

impl State {
  pub fn current_step(&self) -> Step {
    self.steps[self.current]
  }

  pub fn pending_language(&self) -> Language {
    self.pending_language
  }

  // The assembled draft, read by the Finish step's summary and consumed by the app's completion path
  // (`Message::Complete`), which persists it to config.toml verbatim — language from the Language step,
  // feature flags from the Features steps, and the storage overrides from the Storage step.
  pub fn settings(&self) -> &Settings {
    &self.settings
  }

  pub fn is_first(&self) -> bool {
    self.current == 0
  }

  pub fn is_last(&self) -> bool {
    self.current + 1 == self.steps.len()
  }

  // A step can advance only when its body is satisfied. Welcome, Language, and Finish are always
  // valid; the Features and Storage step tasks tighten this seam against their own state.
  fn can_advance(&self) -> bool {
    match self.current_step() {
      Step::Features(_) | Step::Storage => true,
      Step::Finish | Step::Language | Step::Welcome => true,
    }
  }

  // The rail jumps only to phases at or before the furthest-reached one. A phase is reachable once
  // its first step index has been visited (or passed).
  fn reachable(&self, phase: Phase) -> bool {
    self
      .first_index_of(phase)
      .is_some_and(|first| self.current >= first || self.current > self.last_index_of(phase))
  }

  fn first_index_of(&self, phase: Phase) -> Option<usize> {
    self.steps.iter().position(|step| step.phase() == phase)
  }

  fn last_index_of(&self, phase: Phase) -> usize {
    self.steps.iter().rposition(|step| step.phase() == phase).unwrap_or(0)
  }
}

impl Default for State {
  fn default() -> Self {
    let settings = Settings::default();
    let storage_drafts = PathKind::ALL
      .into_iter()
      .map(|kind| (kind, kind.resolved_dir(&settings).display().to_string()))
      .collect();
    State {
      current: 0,
      features: features_tab::State::from_settings(&settings),
      pending_language: settings.accessibility().language(),
      settings,
      steps: steps(),
      storage_drafts,
    }
  }
}

/// The flat step list: Welcome, Language, one step per feature [`Group`], then Storage and Finish.
fn steps() -> Vec<Step> {
  let mut steps = vec![Step::Welcome, Step::Language];
  steps.extend(Group::ALL.into_iter().map(Step::Features));
  steps.push(Step::Storage);
  steps.push(Step::Finish);
  steps
}

pub fn update(state: &mut State, message: Message) {
  match message {
    Message::Back => {
      state.current = state.current.saturating_sub(1);
    }
    Message::Complete => {
      // The completion action (persist + restart) lives in the app, which consumes `settings()`
      // before delegating here, so the wizard state itself has nothing left to mutate.
    }
    Message::Features(message) => {
      // Route through the settings Features tab's own update so the catalog, cascade rules, and
      // dependency locking stay in one place. The persist outcome is dropped: the wizard writes the
      // whole draft once at Finish rather than after each toggle.
      let _ = features_tab::update(&mut state.features, message, &mut state.settings);
    }
    Message::JumpTo(index) => {
      if index < state.steps.len() && state.reachable(state.steps[index].phase()) {
        state.current = index;
      }
    }
    Message::Next => {
      if state.can_advance() && !state.is_last() {
        state.current += 1;
      }
    }
    Message::SelectLanguage(language) => {
      state.pending_language = language;
      state.settings.accessibility_mut().set_language(language);
    }
    Message::Skip => {
      state.current = state.steps.len().saturating_sub(1);
    }
    Message::StorageBrowse(kind) => {
      if let Some(dir) = pick_folder(kind, &state.settings) {
        commit_path(state, kind, dir);
      }
    }
    Message::StorageLogLevel(level) => {
      state.settings.storage_mut().set_log_level(level);
    }
    Message::StoragePathEdited(kind, value) => {
      state.storage_drafts.insert(kind, value);
    }
    Message::StoragePathSubmitted(kind) => {
      let draft = state.storage_drafts.get(&kind).cloned().unwrap_or_default();
      let trimmed = draft.trim();
      if trimmed.is_empty() {
        sync_storage_draft(state, kind);
      } else {
        commit_path(state, kind, PathBuf::from(trimmed));
      }
    }
    Message::StorageReset(kind) => {
      commit_path(state, kind, kind.default_dir());
    }
    Message::StorageSyncToggled(value) => {
      // First run has no database to migrate, so flipping sync only records the network flag — there
      // is no MigrationRequest, working-copy seed, or consolidation behind it.
      state.settings.storage_mut().set_network(value);
    }
  }
}

// Commits a chosen directory to the draft override (cleared to the default when it matches) and
// resyncs the field to the resolved path. No relocation runs — first run has nothing to move.
fn commit_path(state: &mut State, kind: PathKind, dir: PathBuf) {
  let dir = if paths_equal(&dir, &kind.default_dir()) {
    None
  } else {
    Some(dir)
  };
  kind.set_dir(&mut state.settings, dir);
  sync_storage_draft(state, kind);
}

fn sync_storage_draft(state: &mut State, kind: PathKind) {
  let resolved = kind.resolved_dir(&state.settings).display().to_string();
  state.storage_drafts.insert(kind, resolved);
}

fn paths_equal(a: &std::path::Path, b: &std::path::Path) -> bool {
  match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
    (Ok(a), Ok(b)) => a == b,
    _ => a == b,
  }
}

/// Opens a folder picker rooted at the kind's current directory. Stubbed to a no-op (`None`) under
/// `cfg(test)` so the storage update path can be exercised without opening a real dialog, mirroring
/// the settings storage tab's own test stub.
fn pick_folder(kind: PathKind, settings: &Settings) -> Option<PathBuf> {
  #[cfg(not(test))]
  {
    let mut dialog = rfd::FileDialog::new()
      .set_title(t!("settings.storage.pick_folder_title", name => tr_static(kind.label_key())).into_owned());
    let start = kind.resolved_dir(settings);
    if start.is_dir() {
      dialog = dialog.set_directory(&start);
    }
    dialog.pick_folder()
  }
  #[cfg(test)]
  {
    let _ = (kind, settings);
    None
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  // The rail sits beside the content column; a Row lays them out left-to-right.
  let shell = Row::with_children(vec![rail(state), content(state)])
    .width(Length::Fill)
    .height(Length::Fill);

  container(shell)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
}

fn rail(state: &State) -> Element<'_, Message> {
  let brand = rail_brand();
  let phases = Column::with_children(Phase::ALL.into_iter().map(|phase| rail_phase(state, phase)))
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let body = Column::with_children(vec![
    brand,
    container(phases)
      .padding(Padding {
        top: spacing::SPACE_4_5,
        right: spacing::SPACE_4_5,
        bottom: spacing::SPACE_4_5,
        left: spacing::SPACE_4_5,
      })
      .into(),
    Space::new().width(Length::Fill).height(Length::Fill).into(),
    rail_progress(state),
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  container(body)
    .width(Length::Fixed(RAIL_WIDTH))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::NAVIGATION)),
      ..container::Style::default()
    })
    .into()
}

fn rail_brand<'a>() -> Element<'a, Message> {
  let eyebrow = text(t!("wizard.rail.eyebrow").into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let title = text(t!("wizard.rail.title").into_owned())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));

  let label = Column::with_children(vec![eyebrow.into(), title.into()]).spacing(spacing::UNIT);

  let row = Row::with_children(vec![
    Icon::star().size(28.0).color(color::accent::PLASMA).render(),
    label.into(),
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Center);

  Column::with_children(vec![
    container(row)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_6,
        right: spacing::SPACE_6,
        bottom: spacing::SPACE_4_5,
        left: spacing::SPACE_6,
      })
      .into(),
    rule::horizontal(),
  ])
  .width(Length::Fill)
  .into()
}

fn rail_phase(state: &State, phase: Phase) -> Element<'_, Message> {
  let current_phase = state.current_step().phase();
  let active = current_phase == phase;
  let done = state.current > state.last_index_of(phase);
  let reachable = state.reachable(phase);

  let chip = rail_chip(phase, active, done);

  let label = text(t!(phase.label_key()).into_owned())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(if active {
      color::text::PRIMARY
    } else {
      color::text::secondary()
    }));
  let sub = text(rail_phase_sub(phase).into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let labels = Column::with_children(vec![label.into(), sub.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let chevron = if active {
    Icon::chevron_right().size(14.0).color(color::accent::PLASMA).render()
  } else {
    Space::new()
      .width(Length::Fixed(14.0))
      .height(Length::Fixed(14.0))
      .into()
  };

  let row = Row::with_children(vec![chip, labels.into(), chevron])
    .spacing(spacing::SPACE_3_5)
    .align_y(Vertical::Center);

  let cell = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3_5,
  });

  let mut entry = button(cell)
    .padding(0)
    .width(Length::Fill)
    .style(move |_, _| rail_phase_style(active));
  if reachable && let Some(first) = state.first_index_of(phase) {
    entry = entry.on_press(Message::JumpTo(first));
  }

  entry.into()
}

fn rail_chip<'a>(phase: Phase, active: bool, done: bool) -> Element<'a, Message> {
  let glyph = if done {
    Icon::check()
      .size(15.0)
      .color(color::on_fill(color::accent::PLASMA))
      .render()
  } else {
    let tint = if active {
      color::accent::PLASMA
    } else {
      color::text::tertiary()
    };
    phase.glyph().size(15.0).color(tint).render()
  };

  let (fill, border) = if done {
    (Some(color::accent::PLASMA), color::accent::PLASMA)
  } else if active {
    (
      Some(color::with_alpha(color::accent::PLASMA, 0.16)),
      color::accent::PLASMA,
    )
  } else {
    (None, color::rule_strong())
  };

  container(glyph)
    .width(Length::Fixed(30.0))
    .height(Length::Fixed(30.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: fill.map(Background::Color),
      border: Border {
        color: border,
        width: 1.0,
        radius: 15.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn rail_progress(state: &State) -> Element<'_, Message> {
  let total = state.steps.len();
  let current = state.current + 1;

  let label = text(t!("wizard.rail.progress").into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let counter = text(
    t!(
      "wizard.rail.step_counter",
      current => format!("{current:02}"),
      total => format!("{total:02}"),
    )
    .into_owned(),
  )
  .font(typography::mono::REGULAR)
  .size(typography::size::SM)
  .style(typography::colored(color::accent::PLASMA));

  let header = Row::with_children(vec![
    label.into(),
    Space::new().width(Length::Fill).height(Length::Shrink).into(),
    counter.into(),
  ])
  .align_y(Vertical::Center);

  let fraction = current as f32 / total as f32;
  let bar = progress_bar(fraction, color::accent::PLASMA, 4.0);

  let body = Column::with_children(vec![header.into(), bar])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill);

  Column::with_children(vec![
    rule::horizontal(),
    container(body)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_4_5,
        right: spacing::SPACE_6,
        bottom: spacing::SPACE_6,
        left: spacing::SPACE_6,
      })
      .into(),
  ])
  .width(Length::Fill)
  .into()
}

fn content(state: &State) -> Element<'_, Message> {
  let stage = scrollable(container(step_body(state)).width(Length::Fill).padding(CONTENT_PADDING))
    .style(control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill);

  let column = Column::with_children(vec![stage.into(), footer(state), status(state)])
    .width(Length::Fill)
    .height(Length::Fill);

  container(column).width(Length::Fill).height(Length::Fill).into()
}

// The per-step body seam. Each arm renders one step's content; the Features and Storage steps still
// render the shared placeholder until their sibling tasks fill them in.
fn step_body(state: &State) -> Element<'_, Message> {
  match state.current_step() {
    Step::Features(group) => features_body(state, group),
    Step::Language => language_body(state),
    Step::Storage => storage_body(state),
    Step::Welcome => welcome_body(),
    Step::Finish => finish_body(state),
  }
}

// The shared step header: a Plasma eyebrow over a large title and an optional lede, with an optional
// right-aligned readout. Mirrors firstrun.jsx's StepHeader so every step shares the same masthead.
fn step_header<'a>(
  eyebrow: String,
  title: String,
  lede: Option<String>,
  right: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
  let eyebrow = text(eyebrow)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::accent::PLASMA));
  let title = text(title)
    .font(typography::body::MEDIUM)
    .size(STEP_TITLE_SIZE)
    .style(typography::colored(color::text::PRIMARY));

  let mut column: Vec<Element<'a, Message>> = vec![eyebrow.into(), title.into()];
  if let Some(lede) = lede {
    column.push(
      container(
        text(lede)
          .font(typography::body::REGULAR)
          .size(typography::size::MD)
          .style(typography::colored(color::text::secondary())),
      )
      .max_width(620.0)
      .into(),
    );
  }
  let identity = Column::with_children(column)
    .spacing(spacing::SPACE_3)
    .width(Length::Fill);

  let mut children: Vec<Element<'a, Message>> = vec![identity.into()];
  if let Some(right) = right {
    children.push(right);
  }

  Row::with_children(children)
    .align_y(Vertical::Bottom)
    .spacing(spacing::SPACE_6)
    .width(Length::Fill)
    .into()
}

fn welcome_body<'a>() -> Element<'a, Message> {
  let mark = container(Icon::star().size(52.0).color(color::accent::PLASMA).render()).padding(Padding {
    top: 0.0,
    right: 0.0,
    bottom: spacing::SPACE_6 + 6.0,
    left: 0.0,
  });

  let header = step_header(
    t!("wizard.welcome.eyebrow").into_owned(),
    t!("wizard.welcome.title").into_owned(),
    Some(t!("wizard.welcome.lede").into_owned()),
    None,
  );

  let points = Column::with_children(vec![
    benefit_card(
      Icon::settings(),
      t!("wizard.welcome.point_features_title").into_owned(),
      t!("wizard.welcome.point_features_desc").into_owned(),
    ),
    benefit_card(
      Icon::archive(),
      t!("wizard.welcome.point_storage_title").into_owned(),
      t!("wizard.welcome.point_storage_desc").into_owned(),
    ),
    benefit_card(
      Icon::shield(),
      t!("wizard.welcome.point_privacy_title").into_owned(),
      t!("wizard.welcome.point_privacy_desc").into_owned(),
    ),
  ])
  .spacing(spacing::SPACE_3_5)
  .width(Length::Fill);

  let column = Column::with_children(vec![
    mark.into(),
    container(header)
      .padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: spacing::SPACE_6 + 6.0,
        left: 0.0,
      })
      .into(),
    points.into(),
  ])
  .width(Length::Fill);

  container(column).max_width(BENEFIT_CARD_MAX_WIDTH).into()
}

fn benefit_card<'a>(icon: Icon, title: String, description: String) -> Element<'a, Message> {
  let glyph = container(icon.size(20.0).color(color::accent::PLASMA).render())
    .width(Length::Fixed(40.0))
    .height(Length::Fixed(40.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.1))),
      border: Border {
        color: color::with_alpha(color::accent::PLASMA, 0.28),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    });

  let heading = text(title)
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(description)
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let copy = Column::with_children(vec![heading.into(), blurb.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let row = Row::with_children(vec![glyph.into(), copy.into()])
    .spacing(spacing::SPACE_4_5 - 2.0)
    .align_y(Vertical::Top);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_4_5,
      right: spacing::SPACE_4_5 + 2.0,
      bottom: spacing::SPACE_4_5,
      left: spacing::SPACE_4_5 + 2.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

// The Finish step: a plasma check mark, the "you're all set" masthead, a two-up summary of the
// choices (enabled features over the catalog total, customized-vs-default storage paths with the
// resolved database location), a language readout, and a privacy footnote. Mirrors firstrun.jsx's
// FinishStep; the footer's primary button dispatches `Message::Complete` to persist and restart.
fn finish_body(state: &State) -> Element<'_, Message> {
  let settings = state.settings();

  let mark = container(
    Icon::check()
      .size(34.0)
      .color(color::on_fill(color::accent::PLASMA))
      .render(),
  )
  .width(Length::Fixed(64.0))
  .height(Length::Fixed(64.0))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
    border: Border {
      color: color::with_alpha(color::accent::PLASMA, 0.32),
      width: 1.0,
      radius: radius::CARD.into(),
    },
    ..container::Style::default()
  });

  let header = step_header(
    t!("wizard.finish.eyebrow").into_owned(),
    t!("wizard.finish.title").into_owned(),
    Some(t!("wizard.finish.lede").into_owned()),
    None,
  );

  let summary = Row::with_children(vec![finish_features_card(settings), finish_storage_card(settings)])
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill);

  let column = Column::with_children(vec![
    container(mark)
      .padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: spacing::SPACE_6,
        left: 0.0,
      })
      .into(),
    container(header)
      .padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: spacing::SPACE_6 + 6.0,
        left: 0.0,
      })
      .into(),
    summary.into(),
    container(finish_language_row(state.pending_language()))
      .padding(Padding {
        top: spacing::SPACE_3_5,
        right: 0.0,
        bottom: spacing::SPACE_3_5,
        left: 0.0,
      })
      .into(),
    finish_privacy_note(),
  ])
  .width(Length::Fill);

  container(column).max_width(BENEFIT_CARD_MAX_WIDTH).into()
}

// The total enabled sub-features over the catalog size, summed across the display groups. The Finish
// summary reads this as the "N of M features" headline.
fn finish_enabled_over_total(settings: &Settings) -> (usize, usize) {
  Group::ALL
    .into_iter()
    .map(|group| group.enabled_over_total(settings))
    .fold((0, 0), |(on, total), (group_on, group_total)| {
      (on + group_on, total + group_total)
    })
}

fn finish_features_card(settings: &Settings) -> Element<'_, Message> {
  let (on, total) = finish_enabled_over_total(settings);

  let label = text(t!("wizard.finish.features_label").into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let figure = Row::with_children(vec![
    text(format!("{on}"))
      .font(typography::body::MEDIUM)
      .size(STEP_TITLE_SIZE)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(format!("/ {total}"))
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Bottom);
  let groups = text(t!("wizard.finish.features_groups", count => Group::ALL.len()).into_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));

  finish_card(Column::with_children(vec![label.into(), figure.into(), groups.into()]).spacing(spacing::SPACE_2))
}

fn finish_storage_card(settings: &Settings) -> Element<'_, Message> {
  let custom = PathKind::ALL
    .into_iter()
    .filter(|kind| kind.override_dir(settings).is_some())
    .count();
  let db = PathKind::Database.resolved_dir(settings).display().to_string();

  let label = text(t!("wizard.finish.storage_label").into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let figure: Element<'_, Message> = if custom > 0 {
    Row::with_children(vec![
      text(format!("{custom}"))
        .font(typography::body::MEDIUM)
        .size(STEP_TITLE_SIZE)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
      text(t!("wizard.finish.storage_customized").into_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Bottom)
    .into()
  } else {
    text(t!("wizard.finish.storage_default").into_owned())
      .font(typography::body::MEDIUM)
      .size(STEP_TITLE_SIZE)
      .style(typography::colored(color::text::PRIMARY))
      .into()
  };
  let path = text(db)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::secondary()));

  finish_card(Column::with_children(vec![label.into(), figure, path.into()]).spacing(spacing::SPACE_2))
}

fn finish_card<'a>(body: Column<'a, Message>) -> Element<'a, Message> {
  container(body.width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_4_5,
      right: spacing::SPACE_4_5 + 2.0,
      bottom: spacing::SPACE_4_5,
      left: spacing::SPACE_4_5 + 2.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn finish_language_row<'a>(language: Language) -> Element<'a, Message> {
  let label = text(t!("wizard.finish.language_label").into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let native = text(language.native_label())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let name = text(language.label())
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let code = language_code_tag(language.esi_code(), true);

  let row = Row::with_children(vec![
    Icon::market().size(18.0).color(color::accent::PLASMA).render(),
    label.into(),
    Space::new().width(Length::Fill).height(Length::Shrink).into(),
    native.into(),
    name.into(),
    code,
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_4_5 - 2.0,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_4_5 - 2.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn finish_privacy_note<'a>() -> Element<'a, Message> {
  let row = Row::with_children(vec![
    Icon::shield().size(15.0).color(color::text::secondary()).render(),
    text(t!("wizard.finish.privacy_note").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Top);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_4_5 - 2.0,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_4_5 - 2.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn language_body(state: &State) -> Element<'_, Message> {
  let selected = state.pending_language;

  let readout = Row::with_children(vec![
    Icon::market().size(18.0).color(color::accent::PLASMA).render(),
    text(format!("{} · {}", selected.native_label(), selected.esi_code()))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_2_5);

  let header = step_header(
    t!("wizard.language.eyebrow").into_owned(),
    t!("wizard.language.title").into_owned(),
    Some(t!("wizard.language.lede").into_owned()),
    Some(readout.into()),
  );

  let grid = language_grid(selected);
  let note = language_note(selected);

  let column = Column::with_children(vec![
    container(header)
      .padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: spacing::SPACE_6 + 6.0,
        left: 0.0,
      })
      .into(),
    grid,
    container(note)
      .padding(Padding {
        top: spacing::SPACE_6,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
      })
      .into(),
  ])
  .width(Length::Fill);

  container(column).max_width(LANGUAGE_GRID_MAX_WIDTH).into()
}

// A three-column grid of language cards. Iced has no grid widget, so the rows are built by chunking
// the nine languages into rows of three, padding the trailing row with spacers to keep the columns
// aligned. `native_label` is rendered straight (the native names are not themselves translated).
fn language_grid<'a>(selected: Language) -> Element<'a, Message> {
  let rows: Vec<Element<'a, Message>> = Language::ALL
    .chunks(LANGUAGE_GRID_COLUMNS)
    .map(|chunk| {
      let mut cells: Vec<Element<'a, Message>> = chunk
        .iter()
        .map(|&language| language_card(language, language == selected))
        .collect();
      while cells.len() < LANGUAGE_GRID_COLUMNS {
        cells.push(Space::new().width(Length::Fill).into());
      }
      Row::with_children(cells)
        .spacing(spacing::SPACE_3_5)
        .width(Length::Fill)
        .into()
    })
    .collect();

  Column::with_children(rows)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn language_card<'a>(language: Language, selected: bool) -> Element<'a, Message> {
  let native_color = if selected {
    color::text::PRIMARY
  } else {
    color::with_alpha(color::text::PRIMARY, 0.86)
  };
  let native = text(language.native_label())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(native_color));

  let code = language_code_tag(language.esi_code(), selected);
  let top = Row::with_children(vec![native.into(), Space::new().width(Length::Fill).into(), code])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2_5);

  let label = text(t!("wizard.language.card_label", label => language.label()).into_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));

  let mut stack: Vec<Element<'a, Message>> = vec![top.into(), label.into()];
  if selected {
    stack.push(
      container(
        Icon::check()
          .size(11.0)
          .color(color::on_fill(color::accent::PLASMA))
          .render(),
      )
      .width(Length::Fixed(18.0))
      .height(Length::Fixed(18.0))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center)
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA)),
        border: Border {
          radius: 9.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    );
  }

  let content = Column::with_children(stack)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill);

  let (background, border) = if selected {
    (color::with_alpha(color::accent::PLASMA, 0.1), color::accent::PLASMA)
  } else {
    (color::surface::RAISED, color::rule())
  };

  let cell = container(content).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_4_5,
    right: spacing::SPACE_4_5,
    bottom: spacing::SPACE_4_5 - 2.0,
    left: spacing::SPACE_4_5,
  });

  button(cell)
    .padding(0)
    .width(Length::Fill)
    .on_press(Message::SelectLanguage(language))
    .style(move |_, _| button::Style {
      background: Some(Background::Color(background)),
      border: Border {
        color: border,
        width: 1.0,
        radius: radius::CARD.into(),
      },
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    })
    .into()
}

fn language_code_tag<'a>(code: &'static str, selected: bool) -> Element<'a, Message> {
  let (text_color, border_color, background) = if selected {
    (
      color::accent::PLASMA,
      color::with_alpha(color::accent::PLASMA, 0.28),
      color::with_alpha(color::accent::PLASMA, 0.1),
    )
  } else {
    (
      color::text::tertiary(),
      color::rule(),
      color::with_alpha(color::text::PRIMARY, 0.04),
    )
  };
  let label = text(code)
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS)
    .style(typography::colored(text_color));

  container(label)
    .padding(Padding {
      top: 2.0,
      right: spacing::SPACE_2 - 2.0,
      bottom: 2.0,
      left: spacing::SPACE_2 - 2.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(background)),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn language_note<'a>(selected: Language) -> Element<'a, Message> {
  let row = Row::with_children(vec![
    Icon::market().size(15.0).color(color::text::secondary()).render(),
    text(t!("wizard.language.note", code => selected.esi_code()).into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Top);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_4_5 - 2.0,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_4_5 - 2.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

// One Features sub-step: the group's title under a `Features · N of M` eyebrow, an on-count and an
// Enable-all/Disable-all bulk control on the right, then the settings Features tab's own toggle rows
// for that group (mapped into the wizard's message space). The rows mutate the shared draft flags.
fn features_body(state: &State, group: Group) -> Element<'_, Message> {
  let settings = &state.settings;
  let (on, total) = group.enabled_over_total(settings);
  let all_on = on == total;

  let position = features_position(group);
  let group_total = Group::ALL.len();

  let count = text(t!("wizard.features.on_count", on => on, total => total).into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::secondary()));

  let bulk_label = if all_on {
    t!("wizard.features.disable_all").into_owned()
  } else {
    t!("wizard.features.enable_all").into_owned()
  };
  let bulk = button(
    text(bulk_label)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD),
  )
  .padding(control::padding())
  .on_press(Message::Features(features_tab::Message::GroupToggled(group, !all_on)))
  .style(control::ghost_button);

  let right = Row::with_children(vec![count.into(), bulk.into()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3_5);

  let header = step_header(
    t!(
      "wizard.features.eyebrow",
      current => position + 1,
      total => group_total
    )
    .into_owned(),
    group.title().to_owned(),
    None,
    Some(right.into()),
  );

  let rows = container(features_tab::group_rows(group, settings).map(Message::Features))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: 0.0,
      bottom: 0.0,
      left: 0.0,
    });

  let list = Column::with_children(vec![rule::horizontal(), rows.into()]).width(Length::Fill);

  let column = Column::with_children(vec![
    container(header)
      .padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: spacing::SPACE_6 + 6.0,
        left: 0.0,
      })
      .into(),
    list.into(),
  ])
  .width(Length::Fill);

  container(column).max_width(LANGUAGE_GRID_MAX_WIDTH).into()
}

// The zero-based position of a display group among the Features sub-steps, used for the
// `Features · N of M` sub-progress in the step header.
fn features_position(group: Group) -> usize {
  Group::ALL.iter().position(|&candidate| candidate == group).unwrap_or(0)
}

// The slimmed Storage step: a path row per directory (db / log / cache) with a Browse picker and a
// reset-to-default, a verbosity selector on the log row, and a sync toggle on the database row. Every
// control writes only into the draft `Settings.storage` — first run has no data, so no migration,
// export, or relocation flow is reachable from here.
fn storage_body(state: &State) -> Element<'_, Message> {
  let settings = &state.settings;

  let header = step_header(
    t!("wizard.storage.eyebrow").into_owned(),
    t!("wizard.storage.title").into_owned(),
    Some(t!("wizard.storage.lede").into_owned()),
    Some(storage_customized_indicator(settings)),
  );

  let rows: Vec<Element<'_, Message>> = PathKind::ALL
    .into_iter()
    .map(|kind| storage_path_row(state, kind))
    .collect();
  let list = Column::with_children(rows).width(Length::Fill);

  let column = Column::with_children(vec![
    container(header)
      .padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: spacing::SPACE_6 + 6.0,
        left: 0.0,
      })
      .into(),
    Column::with_children(vec![rule::horizontal(), list.into()])
      .width(Length::Fill)
      .into(),
  ])
  .width(Length::Fill);

  container(column).max_width(STORAGE_MAX_WIDTH).into()
}

fn storage_customized_indicator(settings: &Settings) -> Element<'_, Message> {
  let custom = PathKind::ALL
    .into_iter()
    .filter(|kind| kind.override_dir(settings).is_some())
    .count();
  let (dot_color, label) = if custom > 0 {
    (
      color::accent::PLASMA,
      t!("wizard.storage.customized_count", count => custom, total => PathKind::ALL.len()).into_owned(),
    )
  } else {
    (color::status::ONLINE, t!("wizard.storage.all_defaults").into_owned())
  };

  Row::with_children(vec![
    dot(dot_color),
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_2)
  .into()
}

fn storage_path_row(state: &State, kind: PathKind) -> Element<'_, Message> {
  let settings = &state.settings;
  let overridden = kind.override_dir(settings).is_some();
  let default = kind.default_dir();

  let mut title_row: Vec<Element<'_, Message>> = vec![
    text(t!(kind.label_key()).into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if overridden {
    title_row.push(storage_custom_badge());
  }
  let title = Row::with_children(title_row)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3);

  let description = container(
    text(t!(kind.description_key()).into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary())),
  )
  .max_width(640.0);

  let value = state.storage_drafts.get(&kind).map(String::as_str).unwrap_or_default();
  let field = text_input(tr_static("settings.storage.path_placeholder"), value)
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3,
    })
    .width(Length::Fill)
    .on_input(move |next| Message::StoragePathEdited(kind, next))
    .on_submit(Message::StoragePathSubmitted(kind));

  let browse = button(
    text(t!("settings.storage.browse").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD),
  )
  .padding(control::padding())
  .on_press(Message::StorageBrowse(kind))
  .style(control::ghost_button);

  let mut reset = button(
    text(t!("settings.storage.default_button").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD),
  )
  .padding(control::padding())
  .style(control::ghost_button);
  if overridden {
    reset = reset.on_press(Message::StorageReset(kind));
  }

  let controls = Row::with_children(vec![field.into(), browse.into(), reset.into()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2);

  let mut cell_children: Vec<Element<'_, Message>> = vec![title.into(), description.into(), controls.into()];
  if kind == PathKind::Log {
    cell_children.push(storage_verbosity_row(*settings.storage().log_level()));
  }
  if kind == PathKind::Database {
    cell_children.push(storage_sync_row(*settings.storage().network()));
  }
  cell_children.push(storage_default_footnote(&default));

  let cell = container(
    Column::with_children(cell_children)
      .spacing(spacing::SPACE_3)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6 - 4.0,
    right: 0.0,
    bottom: spacing::SPACE_6 - 4.0,
    left: 0.0,
  });

  Column::with_children(vec![cell.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn storage_default_footnote(default: &std::path::Path) -> Element<'static, Message> {
  Row::with_children(vec![
    text(t!("settings.storage.default_label").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    text(default.display().to_string())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn storage_custom_badge<'a>() -> Element<'a, Message> {
  container(
    text(t!("settings.storage.custom").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::accent::PLASMA)),
  )
  .padding(Padding {
    top: 1.0,
    right: spacing::UNIT + 2.0,
    bottom: 1.0,
    left: spacing::UNIT + 2.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.06))),
    border: Border {
      color: color::with_alpha(color::accent::PLASMA, 0.3),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn storage_verbosity_row<'a>(active: LogLevel) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![
    text(t!("settings.storage.verbosity").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ];

  for level in LogLevel::ALL {
    children.push(storage_verbosity_cell(level, level == active));
  }

  Row::with_children(children)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2)
    .into()
}

fn storage_verbosity_cell<'a>(level: LogLevel, active: bool) -> Element<'a, Message> {
  let label_color = if active {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  let cell = container(
    text(level.label())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(label_color)),
  )
  .padding(control::padding())
  .style(move |_| container::Style {
    background: Some(Background::Color(if active {
      color::with_alpha(color::accent::PLASMA, 0.07)
    } else {
      iced::Color::TRANSPARENT
    })),
    border: Border {
      color: if active {
        color::with_alpha(color::accent::PLASMA, 0.45)
      } else {
        color::rule_strong()
      },
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  });

  button(cell)
    .padding(0)
    .on_press(Message::StorageLogLevel(level))
    .style(|_, _| button::Style {
      background: Some(Background::Color(iced::Color::TRANSPARENT)),
      ..button::Style::default()
    })
    .into()
}

fn storage_sync_row<'a>(checked: bool) -> Element<'a, Message> {
  let label = text(t!("settings.storage.sync_label").into_owned())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let explanation = container(
    text(t!("settings.storage.sync_description").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary())),
  )
  .max_width(560.0);
  let copy = Column::with_children(vec![label.into(), explanation.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let row = Row::with_children(vec![
    copy.into(),
    toggle::toggle(checked, Message::StorageSyncToggled(!checked)),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_6);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: 0.0,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_6,
    })
    .into()
}

fn footer(state: &State) -> Element<'_, Message> {
  let mut back = button(footer_label(t!("wizard.footer.back").into_owned()))
    .padding(control::padding())
    .style(control::ghost_button);
  if !state.is_first() {
    back = back.on_press(Message::Back);
  }

  let mut children: Vec<Element<'_, Message>> = vec![
    back.into(),
    Space::new().width(Length::Fill).height(Length::Shrink).into(),
  ];

  if !state.is_last() {
    children.push(
      button(footer_label(t!("wizard.footer.skip").into_owned()))
        .padding(control::padding())
        .on_press(Message::Skip)
        .style(control::ghost_button)
        .into(),
    );
  }

  if state.is_last() {
    children.push(
      button(footer_label(t!("wizard.footer.open").into_owned()))
        .padding(control::padding())
        .on_press(Message::Complete)
        .style(control::primary_button)
        .into(),
    );
  } else {
    let mut next = button(footer_label(next_label(state)))
      .padding(control::padding())
      .style(control::primary_button);
    if state.can_advance() {
      next = next.on_press(Message::Next);
    }
    children.push(next.into());
  }

  let row = Row::with_children(children)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center);

  Column::with_children(vec![
    rule::horizontal(),
    container(row)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_4_5,
        right: FOOTER_SIDE_PADDING,
        bottom: spacing::SPACE_4_5,
        left: FOOTER_SIDE_PADDING,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::BASE)),
        ..container::Style::default()
      })
      .into(),
  ])
  .width(Length::Fill)
  .into()
}

fn footer_label<'a>(label: String) -> iced::widget::Text<'a> {
  text(label).font(typography::body::MEDIUM).size(typography::size::MD)
}

fn next_label(state: &State) -> String {
  let key = match state.current_step() {
    Step::Welcome => "wizard.footer.get_started",
    Step::Language => "wizard.footer.continue_to_features",
    Step::Features(_) if state.current == state.last_index_of(Phase::Features) => "wizard.footer.continue_to_storage",
    Step::Storage => "wizard.footer.review",
    Step::Features(_) | Step::Finish => "wizard.footer.next",
  };
  t!(key).into_owned()
}

fn rail_phase_sub(phase: Phase) -> std::borrow::Cow<'static, str> {
  match phase {
    Phase::Features => t!("wizard.phase.features.sub", count => Group::ALL.len()),
    Phase::Finish => t!("wizard.phase.finish.sub"),
    Phase::Language => t!("wizard.phase.language.sub"),
    Phase::Storage => t!("wizard.phase.storage.sub"),
    Phase::Welcome => t!("wizard.phase.welcome.sub"),
  }
}

fn rail_phase_style(active: bool) -> button::Style {
  let (background, border) = if active {
    (
      Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.1))),
      color::with_alpha(color::accent::PLASMA, 0.3),
    )
  } else {
    (None, color::with_alpha(color::accent::PLASMA, 0.0))
  };

  button::Style {
    background,
    border: Border {
      color: border,
      width: 1.0,
      radius: radius::NAV_CARD.into(),
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn status(state: &State) -> Element<'_, Message> {
  let session = Row::with_children(vec![
    dot(color::accent::PLASMA),
    text(t!("wizard.status.session").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let step = text(
    t!(
      "wizard.status.step_counter",
      current => state.current + 1,
      total => state.steps.len(),
    )
    .into_owned(),
  )
  .font(typography::mono::REGULAR)
  .size(typography::size::XS_PLUS)
  .style(typography::colored(color::accent::PLASMA));

  status_bar(vec![eve_time(chrono::Utc::now())], vec![session.into(), step.into()])
}

#[cfg(test)]
mod tests {
  use super::*;

  fn state() -> State {
    State::default()
  }

  mod steps {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_lays_welcome_language_features_storage_finish_in_order() {
      let steps = steps();

      assert_eq!(steps.first(), Some(&Step::Welcome));
      assert_eq!(steps.get(1), Some(&Step::Language));
      assert_eq!(steps.last(), Some(&Step::Finish));
    }

    #[test]
    fn it_expands_features_to_one_step_per_group() {
      let steps = steps();
      let feature_steps = steps.iter().filter(|step| step.phase() == Phase::Features).count();

      assert_eq!(feature_steps, Group::ALL.len());
    }

    #[test]
    fn its_length_is_two_plus_storage_finish_plus_one_per_group() {
      let steps = steps();

      assert_eq!(steps.len(), Group::ALL.len() + 4);
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn next_advances_one_step() {
      let mut state = state();

      update(&mut state, Message::Next);

      assert_eq!(state.current_step(), Step::Language);
    }

    #[test]
    fn back_returns_to_the_previous_step() {
      let mut state = state();
      update(&mut state, Message::Next);

      update(&mut state, Message::Back);

      assert_eq!(state.current_step(), Step::Welcome);
    }

    #[test]
    fn back_is_a_no_op_on_the_first_step() {
      let mut state = state();

      update(&mut state, Message::Back);

      assert!(state.is_first());
      assert_eq!(state.current_step(), Step::Welcome);
    }

    #[test]
    fn next_is_a_no_op_on_the_last_step() {
      let mut state = state();
      update(&mut state, Message::Skip);
      assert!(state.is_last());

      update(&mut state, Message::Next);

      assert!(state.is_last());
      assert_eq!(state.current_step(), Step::Finish);
    }

    #[test]
    fn skip_jumps_straight_to_finish() {
      let mut state = state();

      update(&mut state, Message::Skip);

      assert_eq!(state.current_step(), Step::Finish);
      assert!(state.is_last());
    }

    #[test]
    fn select_language_records_the_pending_choice() {
      let mut state = state();

      update(&mut state, Message::SelectLanguage(Language::De));

      assert_eq!(state.pending_language, Language::De);
    }

    #[test]
    fn select_language_also_writes_the_choice_into_the_draft_settings() {
      let mut state = state();

      update(&mut state, Message::SelectLanguage(Language::Ja));

      assert_eq!(state.settings().accessibility().language(), Language::Ja);
    }

    #[test]
    fn jump_to_moves_to_a_reachable_phase_step() {
      let mut state = state();
      update(&mut state, Message::Next);
      update(&mut state, Message::Next);
      let welcome = state.first_index_of(Phase::Welcome).unwrap();

      update(&mut state, Message::JumpTo(welcome));

      assert_eq!(state.current_step(), Step::Welcome);
    }

    #[test]
    fn jump_to_ignores_an_unreached_phase() {
      let mut state = state();
      let finish = state.first_index_of(Phase::Finish).unwrap();

      update(&mut state, Message::JumpTo(finish));

      assert_eq!(state.current_step(), Step::Welcome);
    }
  }

  mod reachable {
    use super::*;

    #[test]
    fn the_current_phase_is_reachable() {
      let state = state();

      assert!(state.reachable(Phase::Welcome));
    }

    #[test]
    fn a_passed_phase_stays_reachable() {
      let mut state = state();
      update(&mut state, Message::Skip);

      assert!(state.reachable(Phase::Welcome));
      assert!(state.reachable(Phase::Features));
    }

    #[test]
    fn an_unreached_phase_is_not_reachable() {
      let state = state();

      assert!(!state.reachable(Phase::Finish));
      assert!(!state.reachable(Phase::Storage));
    }
  }

  mod current_step {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn the_rail_phase_tracks_the_current_step() {
      let mut state = state();
      assert_eq!(state.current_step().phase(), Phase::Welcome);

      update(&mut state, Message::Next);
      assert_eq!(state.current_step().phase(), Phase::Language);

      update(&mut state, Message::Next);
      assert_eq!(state.current_step().phase(), Phase::Features);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_first_step() {
      let state = state();

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_last_step() {
      let mut state = state();
      update(&mut state, Message::Skip);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_a_features_sub_step() {
      let mut state = state();
      update(&mut state, Message::Next);
      update(&mut state, Message::Next);

      let _el: Element<'_, Message> = view(&state);
    }
  }

  mod step_body {
    use super::*;

    #[test]
    fn it_renders_the_welcome_body() {
      let state = state();
      assert_eq!(state.current_step(), Step::Welcome);

      let _el: Element<'_, Message> = step_body(&state);
    }

    #[test]
    fn it_renders_the_language_body() {
      let mut state = state();
      update(&mut state, Message::Next);
      assert_eq!(state.current_step(), Step::Language);

      let _el: Element<'_, Message> = step_body(&state);
    }

    #[test]
    fn the_language_grid_offers_every_one_of_the_nine_languages() {
      let _el: Element<'_, Message> = language_grid(Language::Ja);

      assert_eq!(Language::ALL.len(), 9);
    }

    #[test]
    fn the_language_body_renders_for_a_non_latin_selection() {
      let mut state = state();
      update(&mut state, Message::SelectLanguage(Language::Ko));

      let _el: Element<'_, Message> = language_body(&state);
    }

    #[test]
    fn it_renders_each_features_group_body() {
      let state = state();

      for group in Group::ALL {
        let _el: Element<'_, Message> = features_body(&state, group);
      }
    }

    #[test]
    fn it_renders_the_finish_body() {
      let mut state = state();
      update(&mut state, Message::Skip);
      assert_eq!(state.current_step(), Step::Finish);

      let _el: Element<'_, Message> = finish_body(&state);
    }
  }

  mod finish {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::config::SubFeature;

    #[test]
    fn complete_is_a_no_op_on_the_finish_step() {
      let mut state = state();
      update(&mut state, Message::Skip);

      update(&mut state, Message::Complete);

      assert_eq!(state.current_step(), Step::Finish);
      assert!(state.is_last());
    }

    #[test]
    fn the_summary_counts_every_enabled_sub_feature_over_the_catalog() {
      let state = state();

      let (on, total) = finish_enabled_over_total(state.settings());

      assert_eq!(on, total, "every feature starts enabled on a fresh draft");
      assert_eq!(
        total,
        SubFeature::ALL.len(),
        "the total spans the whole feature catalog"
      );
    }

    #[test]
    fn a_disabled_feature_drops_the_summary_enabled_count() {
      let mut state = state();
      let (baseline, total) = finish_enabled_over_total(state.settings());

      update(
        &mut state,
        Message::Features(features_tab::Message::SubToggled(SubFeature::Mail, false)),
      );

      let (on, after_total) = finish_enabled_over_total(state.settings());
      assert_eq!(
        on,
        baseline - 1,
        "disabling one feature lowers the enabled tally by one"
      );
      assert_eq!(after_total, total, "the catalog total is unchanged");
    }

    #[test]
    fn the_draft_carries_the_language_features_and_storage_into_the_assembled_settings() {
      let mut state = state();
      let db_dir = std::env::temp_dir().join("pod-wizard-finish-test-db");

      update(&mut state, Message::SelectLanguage(Language::De));
      update(
        &mut state,
        Message::Features(features_tab::Message::SubToggled(SubFeature::Mail, false)),
      );
      update(
        &mut state,
        Message::StoragePathEdited(PathKind::Database, db_dir.display().to_string()),
      );
      update(&mut state, Message::StoragePathSubmitted(PathKind::Database));

      let assembled = state.settings();
      assert_eq!(
        assembled.accessibility().language(),
        Language::De,
        "the chosen language is folded into the assembled config"
      );
      assert!(
        !assembled.features().is_sub_enabled(SubFeature::Mail),
        "the disabled feature flag carries into the assembled config"
      );
      assert_eq!(
        assembled.storage().db_dir(),
        &Some(db_dir),
        "the storage override carries into the assembled config"
      );
    }
  }

  mod features {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::config::SubFeature;

    #[test]
    fn a_group_toggle_off_clears_that_groups_flags_in_the_shared_draft() {
      let mut state = state();
      let (on, total) = Group::Wallet.enabled_over_total(state.settings());
      assert_eq!(on, total);

      update(
        &mut state,
        Message::Features(features_tab::Message::GroupToggled(Group::Wallet, false)),
      );

      let (on, _) = Group::Wallet.enabled_over_total(state.settings());
      assert_eq!(on, 0);
    }

    #[test]
    fn a_child_toggle_flips_only_that_flag_and_leaves_the_step_unchanged() {
      let mut state = state();
      let before = state.current_step();

      update(
        &mut state,
        Message::Features(features_tab::Message::SubToggled(SubFeature::Mail, false)),
      );

      assert!(!state.settings().features().is_sub_enabled(SubFeature::Mail));
      assert_eq!(
        state.current_step(),
        before,
        "a feature toggle does not advance the step"
      );
    }

    #[test]
    fn the_features_position_orders_the_groups_as_listed() {
      for (index, group) in Group::ALL.into_iter().enumerate() {
        assert_eq!(features_position(group), index);
      }
    }
  }

  mod storage {
    use std::path::{Path, PathBuf};

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_the_storage_body() {
      let mut state = state();
      while state.current_step() != Step::Storage {
        update(&mut state, Message::Next);
      }

      let _el: Element<'_, Message> = storage_body(&state);
    }

    #[test]
    fn a_custom_db_dir_updates_the_draft_storage_without_moving_files() {
      let target = std::env::temp_dir().join("pod-wizard-storage-test-db");
      assert!(!target.exists(), "the test target must not pre-exist");
      let mut state = state();

      update(
        &mut state,
        Message::StoragePathEdited(PathKind::Database, target.display().to_string()),
      );
      update(&mut state, Message::StoragePathSubmitted(PathKind::Database));

      assert_eq!(
        state.settings().storage().db_dir(),
        &Some(target.clone()),
        "the custom db dir lands on the draft override"
      );
      assert!(
        !target.exists(),
        "writing the override must not create or move any directory on disk"
      );
    }

    #[test]
    fn resetting_a_path_clears_the_override_back_to_the_default() {
      let mut state = state();
      update(
        &mut state,
        Message::StoragePathEdited(
          PathKind::Cache,
          PathBuf::from("/tmp/pod-custom-cache").display().to_string(),
        ),
      );
      update(&mut state, Message::StoragePathSubmitted(PathKind::Cache));
      assert!(state.settings().storage().cache_dir().is_some());

      update(&mut state, Message::StorageReset(PathKind::Cache));

      assert_eq!(
        state.settings().storage().cache_dir(),
        &None,
        "a reset returns the path to the platform default (no override)"
      );
    }

    #[test]
    fn the_log_verbosity_selector_sets_the_draft_log_level() {
      let mut state = state();

      update(&mut state, Message::StorageLogLevel(LogLevel::Verbose));

      assert_eq!(state.settings().storage().log_level(), &LogLevel::Verbose);
    }

    #[test]
    fn the_sync_toggle_records_the_network_flag_with_no_relocation() {
      let mut state = state();
      assert!(!*state.settings().storage().network());

      update(&mut state, Message::StorageSyncToggled(true));

      assert!(
        *state.settings().storage().network(),
        "the sync toggle flips the network flag on the draft"
      );
      assert_eq!(
        state.settings().storage().db_dir(),
        &None,
        "flipping sync never repoints or relocates the database on first run"
      );
    }

    #[test]
    fn an_empty_submitted_path_resyncs_the_field_to_the_resolved_default() {
      let mut state = state();

      update(&mut state, Message::StoragePathEdited(PathKind::Log, "   ".to_owned()));
      update(&mut state, Message::StoragePathSubmitted(PathKind::Log));

      assert_eq!(
        state.settings().storage().log_dir(),
        &None,
        "a blank submit leaves the override untouched"
      );
      assert_eq!(
        state.storage_drafts.get(&PathKind::Log).map(String::as_str),
        Some(PathKind::Log.default_dir().display().to_string().as_str()),
        "the field resyncs to the resolved default"
      );
    }

    #[test]
    fn paths_equal_matches_the_same_directory() {
      let dir = Path::new("/var/pod/data");

      assert!(super::super::paths_equal(dir, dir));
      assert!(!super::super::paths_equal(dir, Path::new("/var/pod/other")));
    }
  }
}
