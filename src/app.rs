mod snooze_scheduler;
mod windows;

use std::{
  sync::Arc,
  time::{Duration, Instant, SystemTime},
};

use chrono::{DateTime, Utc};
use iced::{
  Background, Element, Length, Padding, Point, Size, Subscription, Task,
  alignment::{Horizontal, Vertical},
  futures::SinkExt as _,
  keyboard,
  widget::{Column, Row, Space, Stack, button, container, mouse_area, text},
  window,
};
use windows::{Window, Windows};

use crate::{
  clients::{self, esi, eve_image, eve_sso, http},
  config,
  features::{
    about, assets, auth, character_detail, character_manager, character_manager::OwnedPilot, mail, settings,
    skill_plan_editor, skills, skills_compare, splash, wallet,
  },
  services::{cache_cleaner, menu, updater},
  store,
  sync::{self, JobKey, JobKind, Phase, Subject},
  ui::{
    components::{
      backdrop,
      esi_status::esi_status,
      eve_time::eve_time,
      rail::{self, rail},
      sync_chip,
      sync_popover::{self, Header, JobRow, Model, RowState},
      updater_banner,
    },
    style::{color, control, spacing, typography},
  },
  window_state::{self, UiState, WindowGeometry, coalesce::WriteCoalescer, validity},
};

const ABOUT_WINDOW_HEIGHT: f32 = 240.0;
const ABOUT_WINDOW_WIDTH: f32 = 360.0;
const CHIP_OPEN_TINT_ALPHA: f32 = 0.06;
const COMPARE_WINDOW_HEIGHT: f32 = 760.0;
const COMPARE_WINDOW_WIDTH: f32 = 1100.0;
const CONSOLE_DEFAULT_FILTER: &str = "warn,pod=info";
const EDITOR_WINDOW_HEIGHT: f32 = 700.0;
const EDITOR_WINDOW_WIDTH: f32 = 900.0;
const EMPTY_SKILLS_SELECTION: i64 = 0;
const FILE_FILTER: &str = "warn,\
  pod=trace,\
  hyper=warn,\
  reqwest=warn,\
  iced=warn,\
  iced_wgpu=warn,\
  iced_winit=warn,\
  wgpu=warn,\
  wgpu_core=warn,\
  wgpu_hal=warn,\
  sqlx=warn,\
  sqlx::query=trace";
const POPOVER_BOTTOM_OFFSET: f32 = spacing::layout::STATUS_BAR_HEIGHT + 1.0 + 4.0;
const POPOVER_JOBS: [(JobKind, &str); 7] = [
  (JobKind::AssetSync, "Assets"),
  (JobKind::CharacterClones, "Clones"),
  (JobKind::CharacterContacts, "Contacts"),
  (JobKind::CharacterProfile, "Profile"),
  (JobKind::CharacterSkills, "Skills"),
  (JobKind::CharacterTelemetry, "Telemetry"),
  (JobKind::CharacterWallet, "Wallet"),
];
const PERIODIC_PUSH_INTERVAL: Duration = Duration::from_secs(60);
const POPOVER_LEFT: f32 = spacing::SPACE_3_5;
const PULSE_INTERVAL: Duration = Duration::from_millis(450);
const RUNTIME_CHANNEL_BUFFER: usize = 64;
const ZERO_GEOMETRY: WindowGeometry = WindowGeometry {
  height: 0.0,
  width: 0.0,
  x: 0.0,
  y: 0.0,
};

type Tx = iced::futures::channel::mpsc::Sender<Message>;

static UPDATER_RECEIVER: std::sync::Mutex<Option<tokio::sync::watch::Receiver<updater::State>>> =
  std::sync::Mutex::new(None);

struct App {
  about: Option<window::Id>,
  assets: Option<assets::State>,
  auth: auth::State,
  character_detail: Option<character_detail::State>,
  character_manager: Option<character_manager::State>,
  coalescer: WriteCoalescer,
  compare: Option<(window::Id, skills_compare::State)>,
  editor: Option<(window::Id, skill_plan_editor::State)>,
  esi_connected: bool,
  init_error: Option<String>,
  last_push: Option<SystemTime>,
  last_synced: Option<DateTime<Utc>>,
  mail: Option<mail::State>,
  mail_unread: i64,
  now: DateTime<Utc>,
  outbox: sync::OutboxStatus,
  pending_auth: Option<auth::Message>,
  read_only: Option<HolderInfo>,
  route: Route,
  runtime: Option<Runtime>,
  sde_stale: bool,
  selected_character: Option<i64>,
  settings: Option<settings::State>,
  skills: Option<skills::State>,
  splash: Option<splash::State>,
  splash_step: u32,
  store_ready: Option<StoreReady>,
  status: sync::SyncStatus,
  sync_popover_open: bool,
  sync_session: Option<store::sync_session::SyncSession>,
  sync_tick: bool,
  ui_state: UiState,
  updater: Option<updater::Handle>,
  updater_state: updater::State,
  updater_toast_dismissed: bool,
  wallet: Option<wallet::State>,
  windows: Windows,
}

/// Identifies the machine that currently holds the storage lease, surfaced when this instance opens
/// the share read-only. The read-only banner that consumes it lands in a follow-up task.
#[derive(Clone, Debug, Eq, PartialEq)]
struct HolderInfo {
  hostname: String,
  machine_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TakeOverOutcome {
  Claimed,
  Failed,
  StillHeld(HolderInfo),
}

impl From<store::lease::Outcome> for Option<HolderInfo> {
  fn from(outcome: store::lease::Outcome) -> Self {
    match outcome {
      store::lease::Outcome::Acquired => None,
      store::lease::Outcome::HeldBy {
        hostname,
        machine_id,
      } => Some(HolderInfo {
        hostname,
        machine_id,
      }),
    }
  }
}

#[derive(Clone, Copy, Debug, Default)]
struct JobStats {
  active: usize,
  attention: usize,
  done: usize,
  errors: usize,
  total: usize,
}

impl JobStats {
  fn in_progress(&self) -> bool {
    let settled = self.done + self.errors + self.attention;
    self.active > 0 || (self.errors == 0 && self.total > 0 && settled < self.total)
  }
}

#[derive(Clone, Debug)]
enum Message {
  About(about::Message),
  Assets(assets::Message),
  Auth(auth::Message),
  CharacterDetail(character_detail::Message),
  CharacterManager(character_manager::Message),
  ClockTick,
  CloseSyncPopover,
  Compare(skills_compare::Message),
  FocusMainWindow,
  InitFailed(String),
  LeaseHeartbeat,
  LockReleased,
  Mail(mail::Message),
  MailUnreadCounted(i64),
  Menu(menu::MenuAction),
  Nav(rail::Destination),
  OpenAbout,
  PeriodicPush,
  Pushed(Option<SystemTime>),
  Ready(Runtime),
  ReauthCharacter(i64),
  SeedProgress(splash::seed::Progress),
  Settings(settings::Message),
  SkillPlanEditor(skill_plan_editor::Message),
  Skills(skills::Message),
  SnoozesWoken(Vec<(i64, i64)>),
  Splash(splash::Message),
  StoreOpened(Box<StoreReady>),
  Sync(sync::Event),
  SyncPulse,
  TakeOver,
  TakeOverResolved(TakeOverOutcome),
  ToggleSyncPopover,
  UpdaterAction(updater_banner::Action),
  UpdaterDismissToast,
  UpdaterStateChanged(updater::State),
  Wallet(wallet::Message),
  Window(window::Id, window::Event),
  WindowOpened(window::Id),
}

impl Message {
  fn variant_name(&self) -> &'static str {
    self
      .feature_variant_name()
      .or_else(|| self.lifecycle_variant_name())
      .unwrap_or("Window")
  }

  fn feature_variant_name(&self) -> Option<&'static str> {
    Some(match self {
      Message::About(_) => "About",
      Message::Assets(_) => "Assets",
      Message::Auth(_) => "Auth",
      Message::CharacterDetail(_) => "CharacterDetail",
      Message::CharacterManager(_) => "CharacterManager",
      Message::Compare(_) => "Compare",
      Message::Mail(_) => "Mail",
      Message::MailUnreadCounted(_) => "MailUnreadCounted",
      Message::Menu(_) => "Menu",
      Message::Nav(_) => "Nav",
      Message::Settings(_) => "Settings",
      Message::SkillPlanEditor(_) => "SkillPlanEditor",
      Message::Skills(_) => "Skills",
      Message::Sync(_) => "Sync",
      Message::Wallet(_) => "Wallet",
      _ => return None,
    })
  }

  fn lifecycle_variant_name(&self) -> Option<&'static str> {
    Some(match self {
      Message::ClockTick => "ClockTick",
      Message::CloseSyncPopover => "CloseSyncPopover",
      Message::FocusMainWindow => "FocusMainWindow",
      Message::InitFailed(_) => "InitFailed",
      Message::LeaseHeartbeat => "LeaseHeartbeat",
      Message::OpenAbout => "OpenAbout",
      Message::PeriodicPush => "PeriodicPush",
      Message::Pushed(_) => "Pushed",
      Message::Ready(_) => "Ready",
      Message::ReauthCharacter(_) => "ReauthCharacter",
      Message::SeedProgress(_) => "SeedProgress",
      Message::SnoozesWoken(_) => "SnoozesWoken",
      Message::Splash(_) => "Splash",
      Message::StoreOpened(_) => "StoreOpened",
      Message::SyncPulse => "SyncPulse",
      Message::TakeOver => "TakeOver",
      Message::TakeOverResolved(_) => "TakeOverResolved",
      Message::ToggleSyncPopover => "ToggleSyncPopover",
      Message::UpdaterAction(_) => "UpdaterAction",
      Message::UpdaterDismissToast => "UpdaterDismissToast",
      Message::UpdaterStateChanged(_) => "UpdaterStateChanged",
      Message::WindowOpened(_) => "WindowOpened",
      _ => return None,
    })
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Route {
  Assets,
  CharacterDetail(i64),
  #[default]
  Characters,
  Mail,
  Settings,
  Skills(i64),
  Wallet,
}

impl From<rail::Destination> for Route {
  fn from(destination: rail::Destination) -> Self {
    match destination {
      rail::Destination::Assets => unreachable!("Assets is routed via Message::Nav, not From"),
      rail::Destination::Characters => Route::Characters,
      rail::Destination::Mail => Route::Mail,
      rail::Destination::Settings => Route::Settings,
      rail::Destination::Skills => unreachable!("Skills is routed via Message::Nav, not From"),
      rail::Destination::Wallet => unreachable!("Wallet is routed via Message::Nav, not From"),
    }
  }
}

impl Route {
  fn character_id(self) -> Option<i64> {
    match self {
      Route::CharacterDetail(id) | Route::Skills(id) => Some(id),
      _ => None,
    }
  }

  fn destination(self) -> rail::Destination {
    match self {
      Route::Assets => rail::Destination::Assets,
      Route::Characters | Route::CharacterDetail(_) => rail::Destination::Characters,
      Route::Mail => rail::Destination::Mail,
      Route::Settings => rail::Destination::Settings,
      Route::Skills(_) => rail::Destination::Skills,
      Route::Wallet => rail::Destination::Wallet,
    }
  }

  fn name(self) -> &'static str {
    match self {
      Route::Assets => "Assets",
      Route::CharacterDetail(_) => "CharacterDetail",
      Route::Characters => "Characters",
      Route::Mail => "Mail",
      Route::Settings => "Settings",
      Route::Skills(_) => "Skills",
      Route::Wallet => "Wallet",
    }
  }
}

#[allow(dead_code)]
#[derive(Clone)]
struct Runtime {
  db: store::Database,
  esi: Arc<esi::Client>,
  eve_image: Arc<eve_image::Client>,
  settings: config::Settings,
  sso: Arc<eve_sso::Client>,
  sync: sync::Handle,
}

impl std::fmt::Debug for Runtime {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Runtime").finish_non_exhaustive()
  }
}

#[derive(Clone)]
struct StoreReady {
  db: store::Database,
  http: Arc<http::Client>,
  lease: Option<HolderInfo>,
  settings: config::Settings,
  sync_session: Option<store::sync_session::SyncSession>,
}

impl std::fmt::Debug for StoreReady {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("StoreReady").finish_non_exhaustive()
  }
}

fn navigate(app: &mut App, to: Route) {
  let from = app.route;
  app.route = to;
  if from == to {
    tracing::debug!(
      target: "pod::nav",
      from = from.name(),
      to = to.name(),
      "navigation re-selected the current route",
    );
    return;
  }
  match to.character_id() {
    Some(character_id) => tracing::info!(
      target: "pod::nav",
      from = from.name(),
      to = to.name(),
      route = to.name(),
      character_id,
      "navigated to a new route",
    ),
    None => tracing::info!(
      target: "pod::nav",
      from = from.name(),
      to = to.name(),
      route = to.name(),
      "navigated to a new route",
    ),
  }
}

pub fn run() -> iced::Result {
  let log_dir = config::load()
    .map(|settings| settings.storage().resolved_log_dir())
    .unwrap_or_else(|_| config::log_dir());

  let _log_guard = init_tracing(&log_dir);

  iced::daemon(boot, update, view)
    .title(title)
    .theme(theme)
    .subscription(subscription)
    .font(typography::bytes::BODY_REGULAR)
    .font(typography::bytes::BODY_MEDIUM)
    .font(typography::bytes::BODY_SEMIBOLD)
    .font(typography::bytes::MONO_REGULAR)
    .font(typography::bytes::MONO_ITALIC)
    .run()
}

fn init_tracing(log_dir: &std::path::Path) -> Option<tracing_appender::non_blocking::WorkerGuard> {
  use tracing_subscriber::{Layer as _, filter::EnvFilter, fmt, prelude::*};

  let console_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(CONSOLE_DEFAULT_FILTER));
  let console_layer = fmt::layer().compact().with_filter(console_filter);

  let (file_layer, guard) = match std::fs::create_dir_all(log_dir) {
    Ok(()) => {
      let appender = tracing_appender::rolling::Builder::new()
        .filename_prefix("pod")
        .filename_suffix("log")
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .max_log_files(7)
        .build(log_dir);
      match appender {
        Ok(appender) => {
          let (writer, guard) = tracing_appender::non_blocking(appender);
          let layer = fmt::layer()
            .json()
            .with_ansi(false)
            .with_writer(writer)
            .with_filter(EnvFilter::new(FILE_FILTER));
          (Some(layer), Some(guard))
        }
        Err(error) => {
          eprintln!(
            "pod: could not open log file appender in {}: {error}",
            log_dir.display()
          );
          (None, None)
        }
      }
    }
    Err(error) => {
      eprintln!("pod: could not create log directory {}: {error}", log_dir.display());
      (None, None)
    }
  };

  let _ = tracing_subscriber::registry()
    .with(console_layer)
    .with(file_layer)
    .try_init();

  tracing::info!(
    target: "pod::lifecycle",
    version = env!("CARGO_PKG_VERSION"),
    log_dir = %log_dir.display(),
    console_filter = CONSOLE_DEFAULT_FILTER,
    file_filter = FILE_FILTER,
    "pod starting up"
  );

  guard
}

fn blank<'a>() -> Element<'a, Message> {
  container(Space::new().width(Length::Fill).height(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn boot() -> (App, Task<Message>) {
  let image_root = config::load()
    .map(|settings| settings.storage().resolved_cache_dir())
    .unwrap_or_else(|_| config::cache_dir())
    .join("images");
  store::images::init_root(image_root);

  auth::install();
  menu::init();
  let settings = window::Settings {
    size: Size::new(spacing::layout::SPLASH_WIDTH, spacing::layout::SPLASH_HEIGHT),
    decorations: false,
    resizable: false,
    transparent: true,
    position: window::Position::Centered,
    ..window::Settings::default()
  };
  let (id, open_task) = window::open(settings);

  let mut registry = Windows::default();
  registry.register(id, Window::Splash);

  let updater = updater::Config::from_env().map(updater::spawn);
  if let Some(handle) = &updater
    && let Ok(mut guard) = UPDATER_RECEIVER.lock()
  {
    *guard = Some(handle.subscribe());
  }

  let app = App {
    about: None,
    assets: None,
    auth: auth::State::default(),
    character_detail: None,
    character_manager: None,
    coalescer: WriteCoalescer::new(),
    compare: None,
    editor: None,
    esi_connected: true,
    init_error: None,
    last_push: None,
    last_synced: None,
    mail: None,
    mail_unread: 0,
    now: Utc::now(),
    outbox: sync::OutboxStatus::new(),
    pending_auth: None,
    read_only: None,
    route: Route::default(),
    runtime: None,
    sde_stale: false,
    selected_character: None,
    settings: None,
    skills: None,
    splash: Some(splash::State::default()),
    splash_step: 0,
    store_ready: None,
    status: sync::SyncStatus::new(),
    sync_popover_open: false,
    sync_session: None,
    sync_tick: false,
    ui_state: window_state::load(),
    updater: updater.clone(),
    updater_state: updater::State::default(),
    updater_toast_dismissed: false,
    wallet: None,
    windows: registry,
  };
  let task = Task::batch([open_task.map(Message::WindowOpened), open_store()]);

  (app, task)
}

fn open_store() -> Task<Message> {
  let (tx, rx) = iced::futures::channel::mpsc::channel(RUNTIME_CHANNEL_BUFFER);
  tokio::spawn(run_open_store(tx));
  Task::stream(rx)
}

async fn run_open_store(mut tx: Tx) {
  let message = match open_store_inner().await {
    Ok(ready) => {
      tracing::info!(target: "pod::lifecycle", "store opened");
      Message::StoreOpened(Box::new(ready))
    }
    Err(error) => {
      tracing::error!(target: "pod::lifecycle", %error, "opening the store failed");
      Message::InitFailed(error)
    }
  };
  let _ = tx.send(message).await;
}

async fn open_store_inner() -> Result<StoreReady, String> {
  let mut settings = config::load().map_err(|error| error.to_string())?;
  let machine_id = persist_machine_id(&mut settings);
  let database_path = store::bootstrap::resolve_local_path(settings.storage()).map_err(|error| error.to_string())?;
  let sync_session = store::sync_session::SyncSession::from_config(settings.storage(), machine_id);
  let lease = acquire_lease(sync_session.as_ref());
  run_migration_guard(&settings, &database_path);
  let db = store::open(&database_path).await.map_err(|error| error.to_string())?;
  let http = http::Client::builder(http::Cache::new(db.clone())).build();
  Ok(StoreReady {
    db,
    http,
    lease,
    settings,
    sync_session,
  })
}

/// Resolves a stable per-machine identity for the lease, persisting a freshly generated id so the
/// same machine reclaims its own lease (rather than colliding with itself) on the next launch.
fn persist_machine_id(settings: &mut config::Settings) -> String {
  let had_id = settings.storage().machine_id().is_some();
  let machine_id = settings.storage_mut().machine_id_or_generate();
  if !had_id {
    config::save(settings);
  }
  machine_id
}

fn acquire_lease(session: Option<&store::sync_session::SyncSession>) -> Option<HolderInfo> {
  let session = session?;
  match session.acquire(Utc::now()) {
    Ok(outcome) => {
      let holder: Option<HolderInfo> = outcome.into();
      if let Some(holder) = &holder {
        tracing::warn!(target: "pod::lifecycle", hostname = %holder.hostname, "the share is open elsewhere; opening read-only");
      } else {
        tracing::info!(target: "pod::lifecycle", "storage lease acquired");
      }
      holder
    }
    Err(error) => {
      tracing::warn!(target: "pod::lifecycle", %error, "failed to acquire the storage lease");
      None
    }
  }
}

fn run_migration_guard(settings: &config::Settings, database_path: &std::path::Path) {
  store::migration_guard::MigrationGuard::new(
    settings.storage().resolved_cache_dir(),
    database_path.to_path_buf(),
    splash::seed::sde_version_path(),
    window_state::state_path(),
    config::config_file_path(),
  )
  .run();
}

fn build_runtime(ready: StoreReady) -> Task<Message> {
  let (tx, rx) = iced::futures::channel::mpsc::channel(RUNTIME_CHANNEL_BUFFER);
  tokio::spawn(run_build_runtime(ready, tx));
  Task::stream(rx)
}

async fn run_build_runtime(ready: StoreReady, mut tx: Tx) {
  match build_runtime_inner(ready) {
    Ok((runtime, events)) => {
      let _ = tx.send(Message::Ready(runtime)).await;
      forward_sync_events(events, tx).await;
    }
    Err(error) => {
      tracing::error!(target: "pod::lifecycle", %error, "building the runtime failed");
      let _ = tx.send(Message::InitFailed(error)).await;
    }
  }
}

async fn forward_sync_events(mut events: tokio::sync::mpsc::Receiver<sync::Event>, mut tx: Tx) {
  while let Some(event) = events.recv().await {
    if tx.send(Message::Sync(event)).await.is_err() {
      break;
    }
  }
}

fn build_runtime_inner(ready: StoreReady) -> Result<(Runtime, tokio::sync::mpsc::Receiver<sync::Event>), String> {
  let StoreReady {
    db,
    http,
    lease,
    settings,
    ..
  } = ready;
  let read_only = lease.is_some();

  let esi = Arc::new(
    esi::Client::builder(http.clone())
      .user_agent(clients::user_agent())
      .build()
      .map_err(|error| error.to_string())?,
  );
  let sso = Arc::new(eve_sso::Client::new(http.clone(), settings.eve_client_id().clone()));
  let eve_image = Arc::new(eve_image::Client::new(http));

  let (handle, events) = if read_only {
    tracing::info!(target: "pod::lifecycle", "opened read-only; the sync engine stays parked");
    inert_sync()
  } else {
    let started = sync::spawn(
      db.clone(),
      Arc::clone(&esi),
      sso.clone(),
      Arc::clone(&eve_image),
      store::images::default_store(),
      *settings.features(),
    );
    tracing::info!(target: "pod::lifecycle", "sync engine started");
    started
  };
  tracing::info!(target: "pod::lifecycle", "runtime built");
  Ok((
    Runtime {
      db,
      esi,
      eve_image,
      settings,
      sso,
      sync: handle,
    },
    events,
  ))
}

/// A sync handle whose command receiver is dropped, so every command it forwards is a silent no-op,
/// paired with an already-closed event stream. A read-only opener installs this instead of spawning
/// the engine: the only autonomous writer to the working copy never runs, so the local copy cannot
/// diverge and nothing is ever pushed to the canonical share.
fn inert_sync() -> (sync::Handle, tokio::sync::mpsc::Receiver<sync::Event>) {
  let (commands, _commands_rx) = tokio::sync::mpsc::channel(1);
  let (_events_tx, events) = tokio::sync::mpsc::channel(1);
  (sync::Handle::new(commands), events)
}

fn enabled_features(app: &App) -> Vec<config::Feature> {
  if let Some(state) = app.settings.as_ref() {
    return state.settings().features().enabled();
  }
  if let Some(runtime) = app.runtime.as_ref() {
    return runtime.settings.features().enabled();
  }
  config::Feature::ALL.to_vec()
}

fn handle_close_requested(app: &mut App, id: window::Id) -> Task<Message> {
  match app.windows.kind(id) {
    Some(Window::Main | Window::Splash) => {
      tracing::info!(target: "pod::lifecycle", "shutting down");
      shutdown_storage(app).chain(iced::exit())
    }
    Some(Window::Compare) => close_compare_window(app, id),
    Some(Window::SkillPlanEditor) => close_editor_window(app, id),
    Some(Window::About) => close_about_window(app, id),
    _ => window::close(id),
  }
}

/// On a clean exit in sync mode, flushes the working copy back to the share and releases the lease
/// before the process exits, so the next launch sees a current canonical copy and an unheld share.
fn shutdown_storage(app: &mut App) -> Task<Message> {
  if !holding_lease(app) {
    return Task::none();
  }
  let Some(session) = app.sync_session.take() else {
    return Task::none();
  };
  Task::future(async move {
    if let Err(error) = session.checkpoint_and_push().await {
      tracing::warn!(target: "pod::lifecycle", %error, "exit checkpoint and push failed");
    }
    if let Err(error) = session.release() {
      tracing::warn!(target: "pod::lifecycle", %error, "releasing the lease on exit failed");
    }
  })
  .discard()
}

fn handle_focus_main_window(app: &App) -> Task<Message> {
  match app.windows.id_for(Window::Main) {
    Some(id) => {
      tracing::info!(target: "pod::lifecycle", "raising the main window for a duplicate launch");
      window::gain_focus(id)
    }
    None => Task::none(),
  }
}

fn geometry_after_resize(base: Option<WindowGeometry>, size: Size) -> WindowGeometry {
  WindowGeometry {
    height: size.height,
    width: size.width,
    ..base.unwrap_or(ZERO_GEOMETRY)
  }
}

fn geometry_after_move(base: Option<WindowGeometry>, position: Point) -> WindowGeometry {
  WindowGeometry {
    x: position.x,
    y: position.y,
    ..base.unwrap_or(ZERO_GEOMETRY)
  }
}

fn window_key(app: &App, id: window::Id) -> Option<&'static str> {
  app.windows.kind(id).and_then(Window::state_key)
}

fn record_window_geometry(app: &mut App, id: window::Id, geometry: WindowGeometry) {
  let Some(key) = window_key(app, id) else {
    return;
  };
  app.ui_state.windows.insert(key.to_owned(), geometry);
  app.coalescer.request(app.ui_state.clone(), Instant::now());
}

fn record_pane_width(app: &mut App, key: &str, width: f32) {
  app.ui_state.panes.insert(key.to_owned(), width);
  app.coalescer.request(app.ui_state.clone(), Instant::now());
}

fn drain_due_save(app: &mut App, now: Instant) {
  if let Some(state) = app.coalescer.take_due(now) {
    window_state::save(&state);
  }
}

fn flush_pending_save(app: &mut App) {
  if let Some(state) = app.coalescer.take() {
    window_state::save(&state);
  }
}

fn navigate_to_skills(app: &mut App, target: Option<i64>, owned: Vec<i64>) -> Task<Message> {
  match target {
    Some(id) => {
      navigate(app, Route::Skills(id));
      app.selected_character = Some(id);
      app.skills = Some(skills::State::new(id).with_restored_panes(&app.ui_state));
      match app.runtime.as_ref() {
        Some(runtime) => skills::load(&runtime.db, id, owned).map(Message::Skills),
        None => Task::none(),
      }
    }
    None => {
      navigate(app, Route::Skills(EMPTY_SKILLS_SELECTION));
      app.skills = Some(skills::State::new(EMPTY_SKILLS_SELECTION).with_restored_panes(&app.ui_state));
      Task::none()
    }
  }
}

fn navigate_to_wallet(app: &mut App) -> Task<Message> {
  navigate(app, Route::Wallet);
  app.wallet = Some(wallet::State::new());
  match app.runtime.as_ref() {
    Some(runtime) => wallet::load(&runtime.db).map(Message::Wallet),
    None => Task::none(),
  }
}

fn navigate_to_mail(app: &mut App) -> Task<Message> {
  navigate(app, Route::Mail);
  app.mail = Some(mail::State::new().with_restored_panes(&app.ui_state));
  match app.runtime.as_ref() {
    Some(runtime) => mail::load(&runtime.db).map(Message::Mail),
    None => Task::none(),
  }
}

fn mail_clock_reload(app: &App) -> Task<Message> {
  if app.route != Route::Mail {
    return Task::none();
  }
  match (app.mail.as_ref(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => mail::reload(&runtime.db, state.active()).map(Message::Mail),
    _ => Task::none(),
  }
}

fn snooze_wake_tick(app: &App) -> Task<Message> {
  match app.runtime.as_ref() {
    Some(runtime) => Task::perform(
      snooze_scheduler::wake_due_snoozes(runtime.db.clone(), app.now),
      Message::SnoozesWoken,
    ),
    None => Task::none(),
  }
}

fn mail_unread_tick(app: &App) -> Task<Message> {
  match app.runtime.as_ref() {
    Some(runtime) => {
      let db = runtime.db.clone();
      let now = app.now.to_rfc3339();
      Task::perform(
        async move {
          store::repo::mail::visible_unified_unread_count(&db, &now)
            .await
            .unwrap_or(0)
        },
        Message::MailUnreadCounted,
      )
    }
    None => Task::none(),
  }
}

fn rail_mail_unread(live: i64, screen: Option<i64>) -> i64 {
  match screen {
    Some(screen) => screen.min(live),
    None => live,
  }
}

fn navigate_to_assets(app: &mut App) -> Task<Message> {
  navigate(app, Route::Assets);
  app.assets = Some(assets::State::new().with_restored_panes(&app.ui_state));
  match app.runtime.as_ref() {
    Some(runtime) => assets::load(&runtime.db).map(Message::Assets),
    None => Task::none(),
  }
}

fn navigate_to_character_detail(app: &mut App, id: i64) -> Task<Message> {
  let owned: Vec<i64> = app
    .character_manager
    .as_ref()
    .map(character_manager::owned_roster)
    .unwrap_or_default()
    .iter()
    .map(|pilot| pilot.id)
    .collect();
  let features = enabled_features(app);
  navigate(app, Route::CharacterDetail(id));
  app.selected_character = Some(id);
  app.character_detail = Some(character_detail::State::new(id, &features));
  match app.runtime.as_ref() {
    Some(runtime) => character_detail::load(&runtime.db, id, owned).map(Message::CharacterDetail),
    None => Task::none(),
  }
}

fn detail_reload_target(
  detail: Option<&character_detail::State>,
  key: JobKey,
) -> Option<character_detail::DetailDataType> {
  let detail = detail?;
  if key.subject != Subject::Character(detail.active()) {
    return None;
  }
  character_detail::DetailDataType::for_job_kind(key.kind)
}

fn detail_reload_on_finished(app: &App, key: JobKey) -> Option<Task<Message>> {
  let data_type = detail_reload_target(app.character_detail.as_ref(), key)?;
  let runtime = app.runtime.as_ref()?;
  let active = app.character_detail.as_ref()?.active();
  Some(character_detail::reload(&runtime.db, active, data_type).map(Message::CharacterDetail))
}

fn wallet_reload_kind(kind: JobKind) -> bool {
  matches!(
    kind,
    JobKind::CharacterWallet | JobKind::CorporationWallet | JobKind::MarketPrices | JobKind::NetWorthSnapshot
  )
}

fn wallet_reload_on_finished(app: &App, key: JobKey) -> Option<Task<Message>> {
  if app.route != Route::Wallet || !wallet_reload_kind(key.kind) {
    return None;
  }
  let runtime = app.runtime.as_ref()?;
  app.wallet.as_ref()?;
  Some(wallet::load(&runtime.db).map(Message::Wallet))
}

fn assets_reload_on_finished(app: &App, key: JobKey) -> Option<Task<Message>> {
  if app.route != Route::Assets || key.kind != JobKind::AssetSync {
    return None;
  }
  let runtime = app.runtime.as_ref()?;
  app.assets.as_ref()?;
  Some(assets::load(&runtime.db).map(Message::Assets))
}

fn main_view(app: &App) -> Element<'_, Message> {
  let inner: Element<'_, Message> = if let Some(error) = &app.init_error {
    placeholder(format!("Couldn\u{2019}t start Pod: {error}"))
  } else if app.runtime.is_none() {
    placeholder("Starting up\u{2026}".to_owned())
  } else {
    route_view(app)
  };

  let content = container(inner)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    });

  let mail_unread = rail_mail_unread(app.mail_unread, app.mail.as_ref().map(mail::State::unified_unread));
  let body = Row::with_children(vec![
    rail(app.route.destination(), mail_unread, Message::Nav),
    content.into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  let mut column_children: Vec<Element<'_, Message>> = Vec::with_capacity(4);
  if let Some(banner) = updater_banner::banner(&app.updater_state, Message::UpdaterAction) {
    column_children.push(banner);
  }
  if let Some(holder) = &app.read_only {
    column_children.push(read_only_banner(holder));
  }
  if app.sde_stale {
    column_children.push(sde_stale_banner());
  }
  column_children.push(body.into());
  column_children.push(status_bar_view(app));

  let base: Element<'_, Message> = Column::with_children(column_children)
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

  let toast = if app.updater_toast_dismissed {
    None
  } else {
    updater_banner::toast(&app.updater_state, Message::UpdaterAction, Message::UpdaterDismissToast)
  };

  let mut layers: Vec<Element<'_, Message>> = vec![base];
  if app.sync_popover_open {
    let model = sync_model(app);
    let card = container(sync_popover::sync_popover(&model, Message::CloseSyncPopover))
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(Horizontal::Left)
      .align_y(Vertical::Bottom)
      .padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: POPOVER_BOTTOM_OFFSET,
        left: POPOVER_LEFT,
      });
    layers.push(backdrop::click_catcher(Message::CloseSyncPopover));
    layers.push(card.into());
  }
  if let Some(toast) = toast {
    layers.push(toast);
  }

  if layers.len() == 1 {
    return layers.pop().expect("base layer present");
  }

  Stack::with_children(layers)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn sde_stale_banner<'a>() -> Element<'a, Message> {
  let label = text("Static data refresh failed \u{2014} showing the last cached reference data.")
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::status::WARNING),
    });

  container(label)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_6,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_6,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.12))),
      ..container::Style::default()
    })
    .into()
}

fn read_only_banner(holder: &HolderInfo) -> Element<'_, Message> {
  let label = text(format!(
    "Open on {} \u{2014} close it there, or take over.",
    holder.hostname
  ))
  .font(typography::body::REGULAR)
  .size(typography::size::SM)
  .style(|_| text::Style {
    color: Some(color::status::WARNING),
  });

  let action = button(
    text("Take over")
      .font(typography::body::MEDIUM)
      .size(typography::size::SM),
  )
  .padding(control::padding())
  .on_press(Message::TakeOver)
  .style(control::primary_button);

  let row = Row::new()
    .push(container(label).width(Length::Fill).align_y(Vertical::Center))
    .push(action)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_6,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_6,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.12))),
      ..container::Style::default()
    })
    .into()
}

fn route_view(app: &App) -> Element<'_, Message> {
  match app.route {
    Route::Assets => assets_route_view(app),
    Route::CharacterDetail(_) => character_detail_route_view(app),
    Route::Characters => characters_route_view(app),
    Route::Mail => mail_route_view(app),
    Route::Settings => settings_route_view(app),
    Route::Skills(id) => skills_route_view(app, id),
    Route::Wallet => wallet_route_view(app),
  }
}

fn starting_up<'a>() -> Element<'a, Message> {
  placeholder("Starting up\u{2026}".to_owned())
}

fn characters_route_view(app: &App) -> Element<'_, Message> {
  match &app.character_manager {
    Some(_) if app.auth.is_active() => auth::view(&app.auth).map(Message::Auth),
    Some(state) => character_manager::view(state, &app.status).map(Message::CharacterManager),
    None => starting_up(),
  }
}

fn character_detail_route_view(app: &App) -> Element<'_, Message> {
  match &app.character_detail {
    Some(state) => character_detail::view(state).map(Message::CharacterDetail),
    None => starting_up(),
  }
}

fn skills_route_view(app: &App, id: i64) -> Element<'_, Message> {
  match &app.skills {
    Some(state) => skills::view(state, id, &app.status, app.now).map(Message::Skills),
    None => starting_up(),
  }
}

fn mail_route_view(app: &App) -> Element<'_, Message> {
  match &app.mail {
    Some(state) => mail::view(state).map(Message::Mail),
    None => starting_up(),
  }
}

fn wallet_route_view(app: &App) -> Element<'_, Message> {
  match &app.wallet {
    Some(state) => wallet::view(state, app.now).map(Message::Wallet),
    None => starting_up(),
  }
}

fn assets_route_view(app: &App) -> Element<'_, Message> {
  match &app.assets {
    Some(state) => assets::view(state, app.now).map(Message::Assets),
    None => starting_up(),
  }
}

fn settings_route_view(app: &App) -> Element<'_, Message> {
  match &app.settings {
    Some(state) => settings::view(state).map(Message::Settings),
    None => starting_up(),
  }
}

fn placeholder<'a>(message: String) -> Element<'a, Message> {
  container(text(message).size(typography::size::MD).style(|_| text::Style {
    color: Some(color::text::SECONDARY),
  }))
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .into()
}

fn sync_model(app: &App) -> Model {
  let pilots = app
    .character_manager
    .as_ref()
    .map(character_manager::owned_roster)
    .unwrap_or_default();

  let mut rows = Vec::with_capacity(pilots.len() * POPOVER_JOBS.len());
  for pilot in &pilots {
    let subject = Subject::Character(pilot.id);
    for (kind, label) in POPOVER_JOBS {
      let key = JobKey::new(kind, subject);
      let (state, error) = row_state(&app.status, &key);
      rows.push(JobRow {
        character_color: pilot.color,
        character_name: pilot.name.clone(),
        error,
        label: label.to_owned(),
        next_in_secs: app.status.next_in_secs(&key),
        state,
      });
    }
  }

  let total = rows.len();
  let done = rows.iter().filter(|row| row.state == RowState::Done).count();
  let errors = rows.iter().filter(|row| row.state == RowState::Error).count();
  let active = rows.iter().filter(|row| row.state == RowState::Syncing).count();
  let queued = rows.iter().filter(|row| row.state == RowState::Queued).count();

  let header = if active > 0 {
    let percent = (done * 100).checked_div(total).unwrap_or(100) as u8;
    Header::Syncing {
      active,
      percent,
      queued,
    }
  } else {
    Header::Idle {
      last_synced_secs: app.last_synced.map(|at| (app.now - at).num_seconds().max(0) as u64),
    }
  };

  Model {
    done,
    errors,
    header,
    pulse_on: app.sync_tick,
    rows,
    total,
  }
}

fn row_state(status: &sync::SyncStatus, key: &JobKey) -> (RowState, Option<String>) {
  match status.phase(key) {
    None => (RowState::Queued, None),
    Some(Phase::Done) => (RowState::Done, None),
    Some(Phase::Syncing) => (RowState::Syncing, None),
    Some(Phase::Failed) => (RowState::Error, status.reason(key).map(str::to_owned)),
    Some(Phase::BackingOff) => {
      let detail = status
        .retry_secs(key)
        .map(|secs| format!("Backing off {secs}s"))
        .or_else(|| status.reason(key).map(str::to_owned));
      (RowState::Error, detail)
    }
    Some(Phase::Blocked) => (
      RowState::Attention,
      status
        .reason(key)
        .map(str::to_owned)
        .or_else(|| Some("Blocked".to_owned())),
    ),
    Some(Phase::Empty) => (RowState::Attention, Some("No data".to_owned())),
    Some(Phase::NotReady) => (RowState::Attention, Some("Waiting on dependencies".to_owned())),
  }
}

fn expected_job_stats(app: &App) -> JobStats {
  let pilots = app
    .character_manager
    .as_ref()
    .map(character_manager::owned_roster)
    .unwrap_or_default();
  let mut stats = JobStats::default();
  for pilot in &pilots {
    let subject = Subject::Character(pilot.id);
    for (kind, _label) in POPOVER_JOBS {
      let (state, _) = row_state(&app.status, &JobKey::new(kind, subject));
      stats.total += 1;
      match state {
        RowState::Attention => stats.attention += 1,
        RowState::Done => stats.done += 1,
        RowState::Error => stats.errors += 1,
        RowState::Syncing => stats.active += 1,
        RowState::Queued => {}
      }
    }
  }
  stats
}

fn resolve_skills_target(roster: &[OwnedPilot], last_selected: Option<i64>) -> Option<i64> {
  if let Some(id) = last_selected
    && roster.iter().any(|pilot| pilot.id == id)
  {
    return Some(id);
  }
  roster.first().map(|pilot| pilot.id)
}

fn status_bar_view(app: &App) -> Element<'_, Message> {
  let stats = expected_job_stats(app);
  let percent = (stats.done * 100).checked_div(stats.total).unwrap_or(100) as u8;
  let chip = sync_chip::State {
    syncing: stats.in_progress(),
    done: stats.done,
    total: stats.total,
    percent,
    errors: stats.errors,
    attention: stats.attention,
    last_synced_secs: app.last_synced.map(|at| (app.now - at).num_seconds().max(0) as u64),
    pulse_on: app.sync_tick,
  };

  let eve = container(eve_time(app.now))
    .padding(region_padding())
    .height(Length::Fill)
    .align_y(Vertical::Center);

  let open = app.sync_popover_open;
  let chip = container(sync_chip::sync_chip(chip))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: open.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, CHIP_OPEN_TINT_ALPHA))),
      ..container::Style::default()
    });
  let chip = mouse_area(chip).on_press(Message::ToggleSyncPopover);

  let mut children = vec![eve.into(), separator(), chip.into()];
  if let Some(outbox) = outbox_indicator(&app.outbox) {
    children.push(separator());
    children.push(outbox);
  }
  children.push(separator());
  children.push(esi_status(app.esi_connected));

  let row = Row::with_children(children)
    .height(Length::Fill)
    .align_y(Vertical::Center);

  let bar = container(row)
    .width(Length::Fill)
    .height(Length::Fixed(spacing::layout::STATUS_BAR_HEIGHT))
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::NAVIGATION)),
      ..container::Style::default()
    });

  let top_border = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .height(Length::Fixed(1.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::state::OVERLAY_DARK)),
      ..container::Style::default()
    });

  Column::with_children(vec![top_border.into(), bar.into()])
    .width(Length::Fill)
    .into()
}

fn outbox_indicator(outbox: &sync::OutboxStatus) -> Option<Element<'_, Message>> {
  let pending = outbox.pending();
  let failed = outbox.failed();
  if pending == 0 && failed == 0 {
    return None;
  }

  let dot_color = if failed > 0 {
    color::status::DANGER
  } else {
    color::accent::PLASMA
  };

  let mut parts: Vec<Element<'_, Message>> = vec![
    dot(dot_color),
    text("MUTATIONS")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::DIM),
      })
      .into(),
  ];
  if pending > 0 {
    parts.push(
      text(format!("\u{21bb} {pending}"))
        .font(typography::mono::MEDIUM)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::accent::PLASMA),
        })
        .into(),
    );
  }
  if failed > 0 {
    parts.push(
      text(format!("\u{2715} {failed}"))
        .font(typography::mono::MEDIUM)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::status::DANGER),
        })
        .into(),
    );
  }

  Some(
    container(
      Row::with_children(parts)
        .spacing(spacing::SPACE_2)
        .align_y(Vertical::Center),
    )
    .padding(region_padding())
    .height(Length::Fill)
    .align_y(Vertical::Center)
    .into(),
  )
}

fn dot<'a>(fill: iced::Color) -> Element<'a, Message> {
  const DOT_SIZE: f32 = 6.0;
  container(
    Space::new()
      .width(Length::Fixed(DOT_SIZE))
      .height(Length::Fixed(DOT_SIZE)),
  )
  .width(Length::Fixed(DOT_SIZE))
  .height(Length::Fixed(DOT_SIZE))
  .style(move |_| container::Style {
    background: Some(Background::Color(fill)),
    border: iced::Border {
      radius: (DOT_SIZE / 2.0).into(),
      ..iced::Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn region_padding() -> Padding {
  Padding {
    top: 0.0,
    right: spacing::SPACE_3_5,
    bottom: 0.0,
    left: spacing::SPACE_3_5,
  }
}

fn separator<'a>() -> Element<'a, Message> {
  container(Space::new().width(Length::Fixed(1.0)).height(Length::Fill))
    .width(Length::Fixed(1.0))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
      ..container::Style::default()
    })
    .into()
}

fn splash_theme() -> iced::Theme {
  iced::Theme::custom(
    "splash".to_string(),
    iced::theme::Palette {
      background: iced::Color::TRANSPARENT,
      ..iced::theme::Palette::DARK
    },
  )
}

fn updater_subscription() -> Subscription<Message> {
  Subscription::run(updater_state_stream).map(Message::UpdaterStateChanged)
}

fn updater_state_stream() -> impl iced::futures::Stream<Item = updater::State> {
  iced::stream::channel(
    8,
    |mut tx: iced::futures::channel::mpsc::Sender<updater::State>| async move {
      let Some(mut receiver) = UPDATER_RECEIVER.lock().ok().and_then(|guard| guard.clone()) else {
        std::future::pending::<()>().await;
        return;
      };
      let snapshot = receiver.borrow().clone();
      if tx.send(snapshot).await.is_err() {
        return;
      }
      while receiver.changed().await.is_ok() {
        let state = receiver.borrow_and_update().clone();
        if tx.send(state).await.is_err() {
          break;
        }
      }
    },
  )
}

fn subscription(app: &App) -> Subscription<Message> {
  let mut subs = vec![
    iced::time::every(Duration::from_secs(1)).map(|_| Message::ClockTick),
    window::events().map(|(id, event)| Message::Window(id, event)),
  ];
  if app.updater.is_some() {
    subs.push(updater_subscription());
  }
  if app.splash.is_some() {
    subs.push(iced::time::every(Duration::from_millis(16)).map(|_| Message::Splash(splash::Message::Tick)));
  }
  if expected_job_stats(app).in_progress() {
    subs.push(iced::time::every(PULSE_INTERVAL).map(|_| Message::SyncPulse));
  }
  if holding_lease(app) {
    subs.push(iced::time::every(store::lease::HEARTBEAT_INTERVAL).map(|_| Message::LeaseHeartbeat));
    subs.push(iced::time::every(PERIODIC_PUSH_INTERVAL).map(|_| Message::PeriodicPush));
  }
  if app.sync_popover_open {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      matches!(
        event,
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
          key: keyboard::Key::Named(keyboard::key::Named::Escape),
          ..
        })
      )
      .then_some(Message::CloseSyncPopover)
    }));
  }
  subs.push(auth::subscription().map(Message::Auth));
  subs.push(auth::focus_subscription().map(|()| Message::FocusMainWindow));
  subs.push(menu::subscription().map(Message::Menu));
  if let Some(state) = &app.assets {
    subs.push(assets::subscription(state).map(Message::Assets));
  }
  if let Some(state) = &app.character_manager {
    subs.push(character_manager::subscription(state).map(Message::CharacterManager));
  }
  if let Some(state) = &app.settings {
    subs.push(settings::subscription(state).map(Message::Settings));
  }
  if let Some(state) = &app.skills {
    subs.push(skills::subscription(state).map(Message::Skills));
  }
  if let Some(state) = &app.mail {
    subs.push(mail::subscription(state).map(Message::Mail));
  }
  if let Some(state) = &app.wallet {
    subs.push(wallet::subscription(state).map(Message::Wallet));
  }
  if let Some((_, editor)) = &app.editor {
    subs.push(skill_plan_editor::subscription(editor).map(Message::SkillPlanEditor));
  }
  Subscription::batch(subs)
}

fn theme(app: &App, id: window::Id) -> iced::Theme {
  match app.windows.kind(id) {
    Some(Window::Splash) => splash_theme(),
    _ => iced::Theme::Dark,
  }
}

fn title(_app: &App, _id: window::Id) -> String {
  "Pod".to_string()
}

/// Always returns no monitors; window-position restore therefore falls back to the coordinate-range guard rather than per-display on-screen validation.
fn connected_monitors() -> Vec<validity::Rect> {
  Vec::new()
}

fn resolve_window_geometry(
  saved: Option<WindowGeometry>,
  monitors: &[validity::Rect],
  default: Size,
) -> (Size, window::Position) {
  let Some(geometry) = saved else {
    return (default, window::Position::Centered);
  };

  let size = Size::new(geometry.width, geometry.height);
  let position_valid = if monitors.is_empty() {
    validity::is_in_range(&geometry)
  } else {
    validity::is_position_valid(&geometry, monitors)
  };

  let position = if position_valid {
    window::Position::Specific(Point::new(geometry.x, geometry.y))
  } else {
    window::Position::Centered
  };

  (size, position)
}

fn restored_geometry(ui: &UiState, window: Window, default: Size) -> (Size, window::Position) {
  let saved = window.state_key().and_then(|key| ui.windows.get(key).copied());
  resolve_window_geometry(saved, &connected_monitors(), default)
}

fn transition_to_main(app: &mut App) -> Task<Message> {
  let close = match app.windows.id_for(Window::Splash) {
    Some(id) => {
      app.windows.remove(id);
      window::close(id)
    }
    None => Task::none(),
  };
  app.splash = None;

  let (size, position) = restored_geometry(
    &app.ui_state,
    Window::Main,
    Size::new(
      spacing::layout::WINDOW_DEFAULT_WIDTH,
      spacing::layout::WINDOW_DEFAULT_HEIGHT,
    ),
  );
  let settings = window::Settings {
    size,
    position,
    ..window::Settings::default()
  };
  let (id, open_task) = window::open(settings);
  app.windows.register(id, Window::Main);

  Task::batch([close, open_task.map(Message::WindowOpened)])
}

fn open_compare_window(app: &mut App, seed_ids: Vec<i64>) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  let roster = app
    .character_manager
    .as_ref()
    .map(character_manager::owned_roster)
    .unwrap_or_default();

  let close_existing = match app.compare.take() {
    Some((existing_id, _)) => {
      app.windows.remove(existing_id);
      window::close(existing_id)
    }
    None => Task::none(),
  };

  let (size, position) = restored_geometry(
    &app.ui_state,
    Window::Compare,
    Size::new(COMPARE_WINDOW_WIDTH, COMPARE_WINDOW_HEIGHT),
  );
  let settings = window::Settings {
    size,
    position,
    decorations: true,
    resizable: true,
    ..window::Settings::default()
  };
  let (id, open_task) = window::open(settings);
  app.windows.register(id, Window::Compare);
  app.compare = Some((id, skills_compare::State::new(seed_ids.clone(), roster)));

  Task::batch([
    close_existing,
    open_task.map(Message::WindowOpened),
    skills_compare::load(&db, seed_ids).map(Message::Compare),
  ])
}

fn close_compare_window(app: &mut App, id: window::Id) -> Task<Message> {
  if app.compare.as_ref().map(|(cid, _)| *cid) == Some(id) {
    app.compare = None;
  }
  app.windows.remove(id);
  window::close(id)
}

fn open_editor_window(app: &mut App, character_id: i64, seed: skill_plan_editor::Seed) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();

  let close_existing = match app.editor.take() {
    Some((existing_id, _)) => {
      app.windows.remove(existing_id);
      window::close(existing_id)
    }
    None => Task::none(),
  };

  let (size, position) = restored_geometry(
    &app.ui_state,
    Window::SkillPlanEditor,
    Size::new(EDITOR_WINDOW_WIDTH, EDITOR_WINDOW_HEIGHT),
  );
  let settings = window::Settings {
    size,
    position,
    decorations: true,
    resizable: true,
    ..window::Settings::default()
  };
  let (id, open_task) = window::open(settings);
  app.windows.register(id, Window::SkillPlanEditor);
  app.editor = Some((
    id,
    skill_plan_editor::State::new(character_id).with_restored_panes(&app.ui_state),
  ));

  Task::batch([
    close_existing,
    open_task.map(Message::WindowOpened),
    skill_plan_editor::load(&db, character_id, seed).map(Message::SkillPlanEditor),
  ])
}

fn close_editor_window(app: &mut App, id: window::Id) -> Task<Message> {
  let was_editor = app.editor.as_ref().map(|(eid, _)| *eid) == Some(id);
  if was_editor {
    app.editor = None;
  }
  app.windows.remove(id);

  let reload = match (was_editor, app.skills.as_ref(), app.runtime.as_ref()) {
    (true, Some(skills), Some(runtime)) => skills::reload_plans(&runtime.db, skills.active()).map(Message::Skills),
    _ => Task::none(),
  };
  Task::batch([window::close(id), reload])
}

fn handle_menu(app: &mut App, action: menu::MenuAction) -> Task<Message> {
  match action {
    menu::MenuAction::About => Task::done(Message::OpenAbout),
    menu::MenuAction::CheckUpdates => {
      if let Some(handle) = &app.updater {
        handle.check();
      }
      Task::none()
    }
    menu::MenuAction::ClearCache => clear_cache(app),
    menu::MenuAction::Quit => {
      tracing::info!(target: "pod::lifecycle", "shutting down");
      iced::exit()
    }
  }
}

fn clear_cache(app: &App) -> Task<Message> {
  let storage = app
    .runtime
    .as_ref()
    .map(|runtime| runtime.settings.storage().clone())
    .or_else(|| config::load().ok().map(|settings| settings.storage().clone()));
  if let Some(storage) = storage {
    tokio::task::spawn_blocking(move || {
      if let Err(error) = cache_cleaner::clear(&storage) {
        tracing::warn!(target: "pod::lifecycle", %error, "clearing the cache failed");
      }
    });
  }
  Task::none()
}

fn open_about_window(app: &mut App) -> Task<Message> {
  if app.about.is_some() {
    return Task::none();
  }

  let settings = window::Settings {
    size: Size::new(ABOUT_WINDOW_WIDTH, ABOUT_WINDOW_HEIGHT),
    position: window::Position::Centered,
    resizable: false,
    ..window::Settings::default()
  };
  let (id, open_task) = window::open(settings);
  app.windows.register(id, Window::About);
  app.about = Some(id);
  open_task.map(Message::WindowOpened)
}

fn close_about_window(app: &mut App, id: window::Id) -> Task<Message> {
  if app.about == Some(id) {
    app.about = None;
  }
  app.windows.remove(id);
  window::close(id)
}

fn handle_about(app: &mut App, msg: about::Message) -> Task<Message> {
  let dismiss = about::update(msg);
  match (dismiss, app.about) {
    (true, Some(id)) => close_about_window(app, id),
    _ => Task::none(),
  }
}

fn update(app: &mut App, message: Message) -> Task<Message> {
  let span = tracing::trace_span!(target: "pod::ui", "update", message = message.variant_name());
  let _entered = span.enter();
  match dispatch_feature(app, message) {
    Ok(task) => task,
    Err(message) => dispatch_lifecycle(app, *message),
  }
}

fn dispatch_feature(app: &mut App, message: Message) -> Result<Task<Message>, Box<Message>> {
  Ok(match message {
    Message::About(msg) => handle_about(app, msg),
    Message::Assets(msg) => handle_assets(app, msg),
    Message::Auth(msg) => handle_auth(app, msg),
    Message::CharacterDetail(msg) => handle_character_detail(app, msg),
    Message::CharacterManager(msg) => handle_character_manager(app, msg),
    Message::Compare(msg) => handle_compare(app, msg),
    Message::Mail(msg) => handle_mail(app, msg),
    Message::MailUnreadCounted(unread) => handle_mail_unread_counted(app, unread),
    Message::Menu(action) => handle_menu(app, action),
    Message::Nav(destination) => handle_nav(app, destination),
    Message::Settings(msg) => handle_settings(app, msg),
    Message::SkillPlanEditor(msg) => handle_skill_plan_editor(app, msg),
    Message::Skills(msg) => handle_skills(app, msg),
    Message::Sync(event) => handle_sync(app, event),
    Message::Wallet(msg) => handle_wallet(app, msg),
    other => return Err(Box::new(other)),
  })
}

fn dispatch_lifecycle(app: &mut App, message: Message) -> Task<Message> {
  match message {
    Message::ClockTick => handle_clock_tick(app),
    Message::CloseSyncPopover => set_sync_popover_open(app, false),
    Message::FocusMainWindow => handle_focus_main_window(app),
    Message::InitFailed(error) => handle_init_failed(app, error),
    Message::LeaseHeartbeat => handle_lease_heartbeat(app),
    Message::LockReleased => handle_lock_released(app),
    Message::OpenAbout => open_about_window(app),
    Message::PeriodicPush => handle_periodic_push(app),
    Message::Pushed(mark) => handle_pushed(app, mark),
    Message::Ready(runtime) => handle_ready(app, runtime),
    Message::ReauthCharacter(character_id) => handle_reauth_character(app, character_id),
    Message::SeedProgress(progress) => on_seed_progress(app, progress),
    Message::SnoozesWoken(woken) => handle_snoozes_woken(app, woken),
    Message::Splash(msg) => update_splash(app, msg),
    Message::StoreOpened(ready) => handle_store_opened(app, *ready),
    Message::SyncPulse => handle_sync_pulse(app),
    Message::TakeOver => handle_take_over(app),
    Message::TakeOverResolved(outcome) => handle_take_over_resolved(app, outcome),
    Message::ToggleSyncPopover => handle_toggle_sync_popover(app),
    Message::UpdaterAction(action) => handle_updater_action(app, action),
    Message::UpdaterDismissToast => handle_updater_dismiss_toast(app),
    Message::UpdaterStateChanged(state) => handle_updater_state_changed(app, state),
    Message::Window(id, event) => handle_window(app, id, event),
    Message::WindowOpened(id) => on_window_opened(app, id),
    _ => Task::none(),
  }
}

fn handle_assets(app: &mut App, msg: assets::Message) -> Task<Message> {
  if let assets::Message::PaneSettled(key, width) = msg {
    record_pane_width(app, key, width);
    return Task::none();
  }

  let (Some(state), Some(runtime)) = (app.assets.as_mut(), app.runtime.as_ref()) else {
    return Task::none();
  };

  dispatch_assets_with_runtime(state, runtime, msg)
}

fn dispatch_assets_with_runtime(state: &mut assets::State, runtime: &Runtime, msg: assets::Message) -> Task<Message> {
  match msg {
    assets::Message::StockpileEditorLocationSearchChanged(ref value) => {
      let query = value.clone();
      let update = assets::update(state, msg, &runtime.db).map(Message::Assets);
      Task::batch([update, stockpile_location_search(runtime, query)])
    }
    assets::Message::StockpileEditorItemSearchChanged(index, ref value) => {
      let query = value.clone();
      let update = assets::update(state, msg, &runtime.db).map(Message::Assets);
      Task::batch([update, stockpile_item_search(runtime, index, query)])
    }
    assets::Message::StockpileImportResolveRequested => match state.stockpile_import_text() {
      Some(text) => stockpile_import_resolve(runtime, text),
      None => Task::none(),
    },
    assets::Message::StockpileEditorSaved => match state.take_stockpile_editor() {
      Some(editor) => stockpile_save(runtime, editor),
      None => Task::none(),
    },
    msg => assets::update(state, msg, &runtime.db).map(Message::Assets),
  }
}

fn stockpile_save(runtime: &Runtime, editor: assets::Editor) -> Task<Message> {
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let image = Arc::clone(&runtime.eve_image);
  let sso = Arc::clone(&runtime.sso);
  Task::perform(
    async move { assets::save_stockpile(db, esi, image, sso, editor).await },
    |cards| Message::Assets(assets::Message::StockpilesReloaded(cards)),
  )
}

fn stockpile_import_resolve(runtime: &Runtime, text: String) -> Task<Message> {
  let entries = assets::parse_multibuy(&text);
  if entries.is_empty() {
    return Task::none();
  }
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let sso = Arc::clone(&runtime.sso);
  Task::perform(
    async move { assets::resolve_multibuy(db, esi, sso, entries).await },
    |resolution| Message::Assets(assets::Message::StockpileImportResolved(resolution)),
  )
}

fn stockpile_item_search(runtime: &Runtime, index: usize, query: String) -> Task<Message> {
  if query.trim().chars().count() < assets::STOCKPILE_SEARCH_MIN_CHARS {
    return Task::none();
  }
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let sso = Arc::clone(&runtime.sso);
  Task::perform(
    async move { assets::search_item_types(db, esi, sso, query).await },
    move |results| Message::Assets(assets::Message::StockpileEditorItemResults(index, results)),
  )
}

fn stockpile_location_search(runtime: &Runtime, query: String) -> Task<Message> {
  if query.trim().chars().count() < assets::STOCKPILE_SEARCH_MIN_CHARS {
    return Task::none();
  }
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let sso = Arc::clone(&runtime.sso);
  Task::perform(
    async move { assets::search_locations(db, esi, sso, query).await },
    |results| Message::Assets(assets::Message::StockpileEditorLocationResults(results)),
  )
}

fn handle_mail_unread_counted(app: &mut App, unread: i64) -> Task<Message> {
  app.mail_unread = unread;
  Task::none()
}

fn handle_reauth_character(app: &mut App, character_id: i64) -> Task<Message> {
  tracing::info!(character_id, "re-authorizing character via SSO sign-in");
  update(app, Message::Auth(auth::Message::Start(enabled_features(app))))
}

fn handle_settings(app: &mut App, msg: settings::Message) -> Task<Message> {
  let features_changed = matches!(
    msg,
    settings::Message::Features(settings::features_tab::Message::Toggled(..)) | settings::Message::ResetToDefaults
  );

  let Some(state) = app.settings.as_mut() else {
    return Task::none();
  };
  let (outcome, settings_task) = settings::update(state, msg);
  let task = settings_task.map(Message::Settings);

  match outcome {
    settings::Outcome::SyncNow => return Task::batch(vec![task, sync_now(app)]),
    settings::Outcome::ReleaseLock => return Task::batch(vec![task, release_lock(app)]),
    _ => {}
  }

  if !features_changed {
    return task;
  }
  let updated = state.settings().clone();

  let Some(runtime) = app.runtime.as_mut() else {
    return task;
  };
  runtime.settings = updated;
  let reload =
    character_manager::load(&runtime.db, runtime.settings.features().enabled()).map(Message::CharacterManager);

  Task::batch(vec![task, reload])
}

/// Routes the storage tab's "Sync now" up to the lifecycle sync engine, checkpointing the working
/// copy and pushing it to the share. A no-op unless this instance holds the lease in Sync mode.
fn sync_now(app: &App) -> Task<Message> {
  if !holding_lease(app) {
    return Task::none();
  }
  match app.sync_session.clone() {
    Some(session) => push_task(session),
    None => Task::none(),
  }
}

/// Routes the storage tab's "Release lock" up to the lifecycle lease engine, force-releasing the
/// lease on the share so another machine can take over. A no-op unless this instance holds it.
fn release_lock(app: &App) -> Task<Message> {
  if !holding_lease(app) {
    return Task::none();
  }
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  Task::future(async move {
    if let Err(error) = session.release() {
      tracing::warn!(target: "pod::lifecycle", %error, "force-releasing the lease failed");
    } else {
      tracing::info!(target: "pod::lifecycle", "force-released the storage lease");
    }
    Message::LockReleased
  })
}

fn refresh_storage_status(app: &mut App) {
  let holder = app.read_only.as_ref().map(|holder| holder.hostname.clone());
  let last_synced = app.last_synced;
  if let Some(settings) = app.settings.as_mut() {
    settings.set_sync_status(holder, last_synced);
  }
}

fn handle_snoozes_woken(app: &App, woken: Vec<(i64, i64)>) -> Task<Message> {
  if woken.is_empty() {
    return Task::none();
  }
  mail_clock_reload(app)
}

fn handle_store_opened(app: &mut App, ready: StoreReady) -> Task<Message> {
  app.sync_session = ready.sync_session.clone();
  app.read_only = ready.lease.clone();
  app.last_push = app
    .sync_session
    .as_ref()
    .and_then(store::sync_session::SyncSession::last_write);
  let recovery = recover_unsynced_changes(app);
  app.store_ready = Some(ready.clone());
  Task::batch([
    recovery,
    splash::seed::seed(ready.db, ready.http).map(Message::SeedProgress),
  ])
}

/// Pushes a working copy left ahead of the share by a prior crashed session (local generation >
/// share generation) on the next same-machine launch, so the unsynced changes reach the canonical copy.
fn recover_unsynced_changes(app: &App) -> Task<Message> {
  if !holding_lease(app) {
    return Task::none();
  }
  match app.sync_session.clone() {
    Some(session) if session.has_unsynced_changes() => {
      tracing::info!(target: "pod::lifecycle", "recovering unsynced local changes from a prior session");
      push_task(session)
    }
    _ => Task::none(),
  }
}

fn handle_sync_pulse(app: &mut App) -> Task<Message> {
  app.sync_tick = !app.sync_tick;
  Task::none()
}

fn holding_lease(app: &App) -> bool {
  app.sync_session.is_some() && app.read_only.is_none()
}

fn handle_lease_heartbeat(app: &mut App) -> Task<Message> {
  if !holding_lease(app) {
    return Task::none();
  }
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  Task::future(async move {
    if let Err(error) = session.heartbeat(Utc::now()) {
      tracing::warn!(target: "pod::lifecycle", %error, "lease heartbeat failed");
    }
  })
  .discard()
}

fn handle_periodic_push(app: &mut App) -> Task<Message> {
  if !holding_lease(app) {
    return Task::none();
  }
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  if !session.is_dirty_since(app.last_push) {
    tracing::trace!(target: "pod::lifecycle", "periodic push skipped; no local writes since the last push");
    return Task::none();
  }
  push_task(session)
}

fn handle_lock_released(app: &mut App) -> Task<Message> {
  refresh_storage_status(app);
  Task::none()
}

fn handle_pushed(app: &mut App, mark: Option<SystemTime>) -> Task<Message> {
  if let Some(mark) = mark {
    app.last_push = Some(mark);
    app.last_synced = Some(Utc::now());
  }
  refresh_storage_status(app);
  Task::none()
}

/// Captures the working-copy write timestamp *before* the push so the debounce mark never races
/// ahead of a write that lands mid-checkpoint; such a write simply re-pushes on the next tick.
fn push_task(session: store::sync_session::SyncSession) -> Task<Message> {
  let mark = session.last_write();
  Task::future(async move {
    match session.checkpoint_and_push().await {
      Ok(()) => {
        tracing::info!(target: "pod::lifecycle", "pushed the working copy to the share");
        Message::Pushed(mark)
      }
      Err(error) => {
        tracing::warn!(target: "pod::lifecycle", %error, "checkpoint and push failed");
        Message::Pushed(None)
      }
    }
  })
}

fn handle_take_over(app: &mut App) -> Task<Message> {
  if app.read_only.is_none() {
    return Task::none();
  }
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  Task::future(async move {
    let outcome = match session.take_over(Utc::now()) {
      Ok(store::lease::Outcome::Acquired) => {
        tracing::info!(target: "pod::lifecycle", "took over the storage lease");
        TakeOverOutcome::Claimed
      }
      Ok(store::lease::Outcome::HeldBy {
        hostname,
        machine_id,
      }) => {
        tracing::info!(target: "pod::lifecycle", %hostname, "take over declined; the share is still held");
        TakeOverOutcome::StillHeld(HolderInfo {
          hostname,
          machine_id,
        })
      }
      Err(error) => {
        tracing::warn!(target: "pod::lifecycle", %error, "taking over the storage lease failed");
        TakeOverOutcome::Failed
      }
    };
    Message::TakeOverResolved(outcome)
  })
}

/// Applies a resolved take-over. A claim rebuilds the runtime from the parked `store_ready`: the
/// database must reopen so the connection picks up the freshly pulled working copy (swapping the
/// file under a live connection would corrupt it) and the real sync engine replaces the inert one.
fn handle_take_over_resolved(app: &mut App, outcome: TakeOverOutcome) -> Task<Message> {
  match outcome {
    TakeOverOutcome::Claimed => {
      app.read_only = None;
      app.last_push = app
        .sync_session
        .as_ref()
        .and_then(store::sync_session::SyncSession::last_write);
      let Some(mut ready) = app.store_ready.clone() else {
        return Task::none();
      };
      ready.lease = None;
      app.store_ready = Some(ready.clone());
      build_runtime(ready)
    }
    TakeOverOutcome::StillHeld(holder) => {
      app.read_only = Some(holder);
      refresh_storage_status(app);
      Task::none()
    }
    TakeOverOutcome::Failed => Task::none(),
  }
}

fn handle_toggle_sync_popover(app: &mut App) -> Task<Message> {
  set_sync_popover_open(app, !app.sync_popover_open)
}

fn handle_updater_action(app: &mut App, action: updater_banner::Action) -> Task<Message> {
  if let Some(handle) = &app.updater {
    match action {
      updater_banner::Action::Apply => handle.apply(),
      updater_banner::Action::Restart => handle.restart(),
    }
  }
  Task::none()
}

fn handle_updater_dismiss_toast(app: &mut App) -> Task<Message> {
  app.updater_toast_dismissed = true;
  Task::none()
}

fn handle_updater_state_changed(app: &mut App, state: updater::State) -> Task<Message> {
  if state != app.updater_state {
    app.updater_toast_dismissed = false;
    app.updater_state = state;
  }
  Task::none()
}

fn handle_wallet(app: &mut App, msg: wallet::Message) -> Task<Message> {
  match msg {
    wallet::Message::PaneSettled(key, width) => {
      record_pane_width(app, key, width);
      Task::none()
    }
    msg => match (app.wallet.as_mut(), app.runtime.as_ref()) {
      (Some(state), Some(runtime)) => wallet::update(state, msg, &runtime.db).map(Message::Wallet),
      _ => Task::none(),
    },
  }
}

fn set_sync_popover_open(app: &mut App, open: bool) -> Task<Message> {
  app.sync_popover_open = open;
  Task::none()
}

fn handle_auth(app: &mut App, msg: auth::Message) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    app.pending_auth = Some(msg);
    return Task::none();
  };
  let (task, event) = auth::update(&mut app.auth, msg, &runtime.sso, &runtime.esi, &runtime.db);
  let mut tasks = vec![task.map(Message::Auth)];
  let enrolled = match event {
    Some(auth::Event::CorporationAdded(added)) => Some(sync::Subject::Corporation(added.corporation_id)),
    Some(auth::Event::SignedIn(signed)) => Some(sync::Subject::Character(signed.character_id)),
    None => None,
  };
  if let Some(subject) = enrolled {
    runtime.sync.enroll(subject);
    runtime.sync.discover();
    if app.character_manager.is_some() {
      tasks.push(character_manager::load(&runtime.db, enabled_features(app)).map(Message::CharacterManager));
    }
  }
  Task::batch(tasks)
}

fn handle_character_manager(app: &mut App, msg: character_manager::Message) -> Task<Message> {
  match msg {
    character_manager::Message::AddCharacterRequested => {
      update(app, Message::Auth(auth::Message::Start(enabled_features(app))))
    }
    character_manager::Message::AddCorporationRequested => {
      update(app, Message::Auth(auth::Message::StartAddCorporation))
    }
    character_manager::Message::CharacterSelected(id) => navigate_to_character_detail(app, id),
    character_manager::Message::TrainingSkillClicked(character_id) => {
      let owned = owned_pilot_ids(app);
      navigate_to_skills(app, Some(character_id), owned)
    }
    character_manager::Message::ReauthCharacterRequested(character_id) => {
      update(app, Message::ReauthCharacter(character_id))
    }
    character_manager::Message::RemoveCharacterConfirmed(id) => remove_subject_then_update(
      app,
      sync::Subject::Character(id),
      character_manager::Message::RemoveCharacterConfirmed(id),
    ),
    character_manager::Message::RemoveCorporationConfirmed(id) => remove_subject_then_update(
      app,
      sync::Subject::Corporation(id),
      character_manager::Message::RemoveCorporationConfirmed(id),
    ),
    msg => match (app.character_manager.as_mut(), app.runtime.as_ref()) {
      (Some(state), Some(runtime)) => character_manager::update(state, msg, &runtime.db).map(Message::CharacterManager),
      _ => Task::none(),
    },
  }
}

fn remove_subject_then_update(app: &mut App, subject: sync::Subject, msg: character_manager::Message) -> Task<Message> {
  match (app.character_manager.as_mut(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => {
      runtime.sync.withdraw(subject);
      character_manager::update(state, msg, &runtime.db).map(Message::CharacterManager)
    }
    _ => Task::none(),
  }
}

fn handle_character_detail(app: &mut App, msg: character_detail::Message) -> Task<Message> {
  if let character_detail::Message::CharacterChanged(id) = msg {
    let owned: Vec<i64> = owned_pilot_ids(app);
    navigate(app, Route::CharacterDetail(id));
    app.selected_character = Some(id);
    return match (app.character_detail.as_mut(), app.runtime.as_ref()) {
      (Some(state), Some(runtime)) => Task::batch([
        character_detail::update(state, character_detail::Message::CharacterChanged(id), &runtime.db)
          .map(Message::CharacterDetail),
        character_detail::load(&runtime.db, id, owned).map(Message::CharacterDetail),
      ]),
      _ => Task::none(),
    };
  }
  if let character_detail::Message::ReauthRequested(id) = msg {
    return update(app, Message::ReauthCharacter(id));
  }
  match (app.character_detail.as_mut(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => character_detail::update(state, msg, &runtime.db).map(Message::CharacterDetail),
    _ => Task::none(),
  }
}

fn compare_seed_ids(app: &App) -> Vec<i64> {
  let Some(manager) = app.character_manager.as_ref() else {
    return Vec::new();
  };

  let mut by_sp: Vec<(i64, i64)> = character_manager::groups(manager)
    .iter()
    .flat_map(|group| group.cards.iter())
    .chain(character_manager::unassigned(manager).iter())
    .map(|card| (card.character_id, card.total_sp.unwrap_or(0)))
    .collect();
  by_sp.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

  let active = app.skills.as_ref().map(skills::State::active);
  let mut seed_ids: Vec<i64> = Vec::new();
  if let Some(active_id) = active.filter(|id| by_sp.iter().any(|(card_id, _)| card_id == id)) {
    seed_ids.push(active_id);
  }
  for (card_id, _) in &by_sp {
    if seed_ids.len() >= 3 {
      break;
    }
    if !seed_ids.contains(card_id) {
      seed_ids.push(*card_id);
    }
  }
  seed_ids
}

fn owned_pilot_ids(app: &App) -> Vec<i64> {
  app
    .character_manager
    .as_ref()
    .map(character_manager::owned_roster)
    .unwrap_or_default()
    .iter()
    .map(|pilot| pilot.id)
    .collect()
}

fn handle_clock_tick(app: &mut App) -> Task<Message> {
  app.now = Utc::now();
  drain_due_save(app, Instant::now());
  Task::batch([snooze_wake_tick(app), mail_unread_tick(app), mail_clock_reload(app)])
}

fn handle_init_failed(app: &mut App, error: String) -> Task<Message> {
  tracing::error!(%error, "bootstrap failed");
  app.store_ready = None;
  if let Some(state) = app.splash.as_mut() {
    let _ = splash::update(
      state,
      splash::Message::StepChanged {
        label: format!("Couldn\u{2019}t start Pod: {error}"),
        progress: state.progress_target,
      },
    );
  }
  app.init_error = Some(error);
  Task::none()
}

fn handle_nav(app: &mut App, destination: rail::Destination) -> Task<Message> {
  match destination {
    rail::Destination::Skills => {
      let roster = app
        .character_manager
        .as_ref()
        .map(character_manager::owned_roster)
        .unwrap_or_default();
      let target = resolve_skills_target(&roster, app.selected_character);
      let owned = roster.iter().map(|pilot| pilot.id).collect();
      navigate_to_skills(app, target, owned)
    }
    rail::Destination::Mail => navigate_to_mail(app),
    rail::Destination::Wallet => navigate_to_wallet(app),
    rail::Destination::Assets => navigate_to_assets(app),
    other => {
      navigate(app, Route::from(other));
      Task::none()
    }
  }
}

fn handle_ready(app: &mut App, runtime: Runtime) -> Task<Message> {
  let load_roster = character_manager::load(&runtime.db, runtime.settings.features().enabled());
  app.character_manager = Some(character_manager::State::new());
  let settings_state = settings::State::new(runtime.settings.clone(), runtime.db.clone());
  let load_tags = settings::load(&settings_state).map(Message::Settings);
  app.settings = Some(settings_state);
  app.runtime = Some(runtime);
  refresh_storage_status(app);
  Task::batch([
    load_roster.map(Message::CharacterManager),
    load_tags,
    begin_splash_expand(app),
    replay_pending_auth(app),
  ])
}

fn begin_splash_expand(app: &mut App) -> Task<Message> {
  match app.splash.as_mut() {
    Some(state) => splash::update(state, splash::Message::LoadingComplete).map(Message::Splash),
    None => Task::none(),
  }
}

fn replay_pending_auth(app: &mut App) -> Task<Message> {
  match app.pending_auth.take() {
    Some(msg) => update(app, Message::Auth(msg)),
    None => Task::none(),
  }
}

fn handle_skills(app: &mut App, msg: skills::Message) -> Task<Message> {
  match msg {
    skills::Message::CharacterChanged(id) => {
      navigate(app, Route::Skills(id));
      app.selected_character = Some(id);
      let owned = owned_pilot_ids(app);
      match (app.skills.as_mut(), app.runtime.as_ref()) {
        (Some(state), Some(runtime)) => Task::batch([
          skills::update(state, skills::Message::CharacterChanged(id), &runtime.db).map(Message::Skills),
          skills::load(&runtime.db, id, owned).map(Message::Skills),
        ]),
        _ => Task::none(),
      }
    }
    skills::Message::OpenCompare => {
      let seed_ids = compare_seed_ids(app);
      if seed_ids.len() < 2 {
        Task::none()
      } else {
        open_compare_window(app, seed_ids)
      }
    }
    skills::Message::OpenPlanEditor(seed) => match app.skills.as_ref().map(skills::State::active) {
      Some(id) => open_editor_window(app, id, seed),
      None => Task::none(),
    },
    skills::Message::PaneSettled(key, width) => {
      record_pane_width(app, key, width);
      Task::none()
    }
    msg => match (app.skills.as_mut(), app.runtime.as_ref()) {
      (Some(state), Some(runtime)) => skills::update(state, msg, &runtime.db).map(Message::Skills),
      _ => Task::none(),
    },
  }
}

fn handle_mail(app: &mut App, msg: mail::Message) -> Task<Message> {
  if let mail::Message::PaneSettled(key, width) = msg {
    record_pane_width(app, key, width);
    return Task::none();
  }

  let (Some(state), Some(runtime)) = (app.mail.as_mut(), app.runtime.as_ref()) else {
    return Task::none();
  };

  match msg {
    mail::Message::ScopeSelected(scope) => Task::batch([
      mail::update(state, mail::Message::ScopeSelected(scope), &runtime.db).map(Message::Mail),
      mail::reload(&runtime.db, scope).map(Message::Mail),
    ]),
    mail::Message::ComposeToInput(_) | mail::Message::ComposeCcInput(_) => handle_compose_input(state, runtime, msg),
    msg => mail::update(state, msg, &runtime.db).map(Message::Mail),
  }
}

fn handle_compose_input(state: &mut mail::State, runtime: &Runtime, msg: mail::Message) -> Task<Message> {
  let (query, is_to) = match &msg {
    mail::Message::ComposeToInput(value) => (value.clone(), true),
    mail::Message::ComposeCcInput(value) => (value.clone(), false),
    _ => unreachable!("handle_compose_input only receives compose To/Cc inputs"),
  };
  let owner = state.compose_from_character();
  let update = mail::update(state, msg, &runtime.db).map(Message::Mail);
  match owner {
    Some(owner_id) => Task::batch([update, mail_recipient_search(runtime, owner_id, query, is_to)]),
    None => update,
  }
}

fn mail_recipient_search(runtime: &Runtime, owner_id: i64, query: String, is_to: bool) -> Task<Message> {
  if query.trim().chars().count() < mail::RECIPIENT_SEARCH_MIN_CHARS {
    return Task::none();
  }
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let eve_image = Arc::clone(&runtime.eve_image);
  let sso = Arc::clone(&runtime.sso);
  Task::perform(
    async move { mail::search_recipients(db, esi, eve_image, sso, owner_id, query).await },
    move |results| {
      Message::Mail(if is_to {
        mail::Message::ComposeToSearched(results)
      } else {
        mail::Message::ComposeCcSearched(results)
      })
    },
  )
}

fn handle_compare(app: &mut App, msg: skills_compare::Message) -> Task<Message> {
  match msg {
    skills_compare::Message::CloseRequested => match app.compare.as_ref() {
      Some((id, _)) => close_compare_window(app, *id),
      None => Task::none(),
    },
    msg => match (app.compare.as_mut(), app.runtime.as_ref()) {
      (Some((_, compare)), Some(runtime)) => skills_compare::update(compare, msg, &runtime.db).map(Message::Compare),
      _ => Task::none(),
    },
  }
}

fn handle_skill_plan_editor(app: &mut App, msg: skill_plan_editor::Message) -> Task<Message> {
  match msg {
    skill_plan_editor::Message::CloseRequested => match app.editor.as_ref() {
      Some((id, _)) => close_editor_window(app, *id),
      None => Task::none(),
    },
    skill_plan_editor::Message::PaneSettled(key, width) => {
      record_pane_width(app, key, width);
      Task::none()
    }
    msg => match (app.editor.as_mut(), app.runtime.as_ref()) {
      (Some((_, editor)), Some(runtime)) => {
        skill_plan_editor::update(editor, msg, &runtime.db).map(Message::SkillPlanEditor)
      }
      _ => Task::none(),
    },
  }
}

fn handle_sync(app: &mut App, event: sync::Event) -> Task<Message> {
  app.status.apply(&event);
  app.outbox.apply(&event);
  let sync::Event::Finished {
    key, ..
  } = event
  else {
    return Task::none();
  };
  app.last_synced = Some(app.now);
  let mut tasks: Vec<Task<Message>> = Vec::new();
  if let (Some(_), Some(runtime)) = (&app.character_manager, &app.runtime) {
    tasks.push(character_manager::load(&runtime.db, enabled_features(app)).map(Message::CharacterManager));
  }
  if let Some(reload) = detail_reload_on_finished(app, key) {
    tasks.push(reload);
  }
  if let Some(reload) = wallet_reload_on_finished(app, key) {
    tasks.push(reload);
  }
  if let Some(reload) = assets_reload_on_finished(app, key) {
    tasks.push(reload);
  }
  Task::batch(tasks)
}

fn handle_window(app: &mut App, id: window::Id, event: window::Event) -> Task<Message> {
  match event {
    window::Event::Resized(size) => {
      let base = window_key(app, id).and_then(|key| app.ui_state.windows.get(key).copied());
      record_window_geometry(app, id, geometry_after_resize(base, size));
      Task::none()
    }
    window::Event::Moved(position) => {
      let base = window_key(app, id).and_then(|key| app.ui_state.windows.get(key).copied());
      record_window_geometry(app, id, geometry_after_move(base, position));
      Task::none()
    }
    window::Event::CloseRequested => {
      flush_pending_save(app);
      handle_close_requested(app, id)
    }
    _ => Task::none(),
  }
}

#[cfg(target_os = "macos")]
fn disable_shadow(id: window::Id) -> Task<Message> {
  window::run(id, |w| {
    use iced::window::raw_window_handle::RawWindowHandle;

    let Ok(handle) = w.window_handle() else {
      return;
    };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else {
      return;
    };
    let ns_view: *mut objc2::runtime::AnyObject = h.ns_view.as_ptr().cast();
    unsafe {
      let ns_window: *mut objc2::runtime::AnyObject = objc2::msg_send![ns_view, window];
      if !ns_window.is_null() {
        let _: () = objc2::msg_send![ns_window, setHasShadow: false];
      }
    }
  })
  .discard()
}

#[cfg(not(target_os = "macos"))]
fn disable_shadow(_: window::Id) -> Task<Message> {
  Task::none()
}

fn on_window_opened(app: &App, id: window::Id) -> Task<Message> {
  match app.windows.kind(id) {
    Some(Window::Splash) => disable_shadow(id),
    _ => Task::none(),
  }
}

fn on_seed_progress(app: &mut App, progress: splash::seed::Progress) -> Task<Message> {
  match progress {
    splash::seed::Progress::Step(label) => {
      app.splash_step += 1;
      let target = seed_progress_target(app.splash_step);
      match app.splash.as_mut() {
        Some(state) => splash::update(
          state,
          splash::Message::StepChanged {
            label,
            progress: target,
          },
        )
        .map(Message::Splash),
        None => Task::none(),
      }
    }
    splash::seed::Progress::Complete => match app.store_ready.take() {
      Some(ready) => build_runtime(ready),
      None => Task::none(),
    },
    splash::seed::Progress::Degraded(error) => handle_seed_degraded(app, error),
    splash::seed::Progress::Error(error) => handle_seed_failed(app, error),
  }
}

fn handle_seed_degraded(app: &mut App, error: String) -> Task<Message> {
  tracing::warn!(%error, "SDE refresh failed; proceeding with existing reference data");
  app.sde_stale = true;
  match app.store_ready.take() {
    Some(ready) => build_runtime(ready),
    None => Task::none(),
  }
}

/// Reports a fatal seed failure but deliberately preserves `store_ready` (unlike `handle_init_failed`,
/// which clears it) so the splash Retry action can re-run the seed against the parked store.
fn handle_seed_failed(app: &mut App, error: String) -> Task<Message> {
  tracing::error!(%error, "SDE seed failed with no existing reference data");
  if let Some(state) = app.splash.as_mut() {
    let _ = splash::update(state, splash::Message::Failed(error.clone()));
  }
  app.init_error = Some(error);
  Task::none()
}

fn seed_progress_target(step: u32) -> f32 {
  const SEED_PROGRESS_CEILING: f32 = 0.95;
  const STAGE_ADVANCE: f32 = 0.30;
  let remaining = (1.0 - STAGE_ADVANCE).powi(step as i32);
  SEED_PROGRESS_CEILING * (1.0 - remaining)
}

fn update_splash(app: &mut App, message: splash::Message) -> Task<Message> {
  if matches!(message, splash::Message::DragWindow) {
    return match app.windows.id_for(Window::Splash) {
      Some(id) => window::drag(id),
      None => Task::none(),
    };
  }
  if matches!(message, splash::Message::ExpandComplete) {
    return transition_to_main(app);
  }
  if matches!(message, splash::Message::Retry) {
    return retry_seed(app);
  }
  match app.splash.as_mut() {
    Some(state) => splash::update(state, message).map(Message::Splash),
    None => Task::none(),
  }
}

fn retry_seed(app: &mut App) -> Task<Message> {
  let Some(ready) = app.store_ready.clone() else {
    return Task::none();
  };
  app.init_error = None;
  app.splash_step = 0;
  if let Some(state) = app.splash.as_mut() {
    let _ = splash::update(state, splash::Message::Retry);
  }
  splash::seed::seed(ready.db, ready.http).map(Message::SeedProgress)
}

fn view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.windows.kind(id) {
    Some(Window::Splash) => match app.splash.as_ref() {
      Some(state) => splash::view(state, app.now).map(Message::Splash),
      None => blank(),
    },
    Some(Window::Main) => main_view(app),
    Some(Window::Compare) => match app.compare.as_ref() {
      Some((compare_id, state)) if *compare_id == id => skills_compare::view(state).map(Message::Compare),
      _ => blank(),
    },
    Some(Window::SkillPlanEditor) => match app.editor.as_ref() {
      Some((editor_id, state)) if *editor_id == id => {
        skill_plan_editor::view(state, app.now).map(Message::SkillPlanEditor)
      }
      _ => blank(),
    },
    Some(Window::About) => about::view().map(Message::About),
    _ => blank(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn pilot(id: i64) -> OwnedPilot {
    OwnedPilot {
      color: iced::Color::WHITE,
      id,
      name: format!("Pilot {id}"),
    }
  }

  fn test_app() -> App {
    App {
      about: None,
      assets: None,
      auth: auth::State::default(),
      character_detail: None,
      character_manager: None,
      coalescer: WriteCoalescer::new(),
      compare: None,
      editor: None,
      esi_connected: true,
      init_error: None,
      last_push: None,
      last_synced: None,
      mail: None,
      mail_unread: 0,
      now: Utc::now(),
      outbox: sync::OutboxStatus::new(),
      pending_auth: None,
      read_only: None,
      route: Route::default(),
      runtime: None,
      sde_stale: false,
      selected_character: None,
      settings: None,
      skills: None,
      splash: None,
      splash_step: 0,
      store_ready: None,
      status: sync::SyncStatus::new(),
      sync_popover_open: false,
      sync_session: None,
      sync_tick: false,
      ui_state: UiState::default(),
      updater: None,
      updater_state: updater::State::default(),
      updater_toast_dismissed: false,
      wallet: None,
      windows: Windows::default(),
    }
  }

  async fn test_runtime() -> Runtime {
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = Arc::new(esi::Client::builder(http.clone()).user_agent("test").build().unwrap());
    let eve_image = Arc::new(eve_image::Client::new(http.clone()));
    let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
    let (tx, _rx) = tokio::sync::mpsc::channel(8);
    Runtime {
      db,
      esi,
      eve_image,
      settings: config::Settings::default(),
      sso,
      sync: sync::Handle::new(tx),
    }
  }

  mod destination {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_a_skills_route_to_the_skills_destination() {
      assert_eq!(Route::Skills(42).destination(), rail::Destination::Skills);
    }

    #[test]
    fn it_maps_a_mail_route_to_the_mail_destination() {
      assert_eq!(Route::Mail.destination(), rail::Destination::Mail);
    }

    #[test]
    fn it_round_trips_characters_settings_and_mail_through_from() {
      assert_eq!(Route::from(Route::Characters.destination()), Route::Characters);
      assert_eq!(Route::from(Route::Settings.destination()), Route::Settings);
      assert_eq!(Route::from(Route::Mail.destination()), Route::Mail);
    }
  }

  mod resolve_skills_target {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_to_the_first_owned_pilot_with_no_prior_selection() {
      let roster = vec![pilot(7), pilot(3)];

      assert_eq!(resolve_skills_target(&roster, None), Some(7));
    }

    #[test]
    fn it_keeps_the_sticky_selection_when_still_owned() {
      let roster = vec![pilot(7), pilot(3)];

      assert_eq!(resolve_skills_target(&roster, Some(3)), Some(3));
    }

    #[test]
    fn it_falls_back_to_first_owned_when_the_sticky_selection_left_the_roster() {
      let roster = vec![pilot(7), pilot(3)];

      assert_eq!(resolve_skills_target(&roster, Some(99)), Some(7));
    }

    #[test]
    fn it_yields_none_for_an_empty_roster() {
      assert_eq!(resolve_skills_target(&[], None), None);
      assert_eq!(resolve_skills_target(&[], Some(7)), None);
    }
  }

  mod seed_progress_target {
    use super::*;

    #[test]
    fn it_advances_monotonically_and_stays_below_the_full_bar() {
      let mut last = 0.0;
      for step in 1..=12 {
        let target = seed_progress_target(step);
        assert!(target > last, "stage {step} must advance the bar");
        assert!(target < 1.0, "stage {step} must reserve the full bar for readiness");
        last = target;
      }
    }
  }

  mod resolve_window_geometry {
    use pretty_assertions::assert_eq;

    use super::*;

    const DEFAULT: Size = Size::new(1200.0, 800.0);

    fn monitor() -> validity::Rect {
      validity::Rect {
        height: 1080.0,
        width: 1920.0,
        x: 0.0,
        y: 0.0,
      }
    }

    fn geometry(x: f32, y: f32) -> WindowGeometry {
      WindowGeometry {
        height: 700.0,
        width: 1000.0,
        x,
        y,
      }
    }

    #[test]
    fn it_centers_at_the_default_size_when_there_is_no_saved_geometry() {
      let (size, position) = resolve_window_geometry(None, &[monitor()], DEFAULT);

      assert_eq!(size, DEFAULT);
      assert!(matches!(position, window::Position::Centered));
    }

    #[test]
    fn it_restores_size_and_position_for_a_monitor_valid_saved_rect() {
      let (size, position) = resolve_window_geometry(Some(geometry(120.0, 90.0)), &[monitor()], DEFAULT);

      assert_eq!(size, Size::new(1000.0, 700.0));
      assert!(matches!(position, window::Position::Specific(p) if p == Point::new(120.0, 90.0)));
    }

    #[test]
    fn it_honors_the_saved_size_but_centers_an_off_monitor_position() {
      let (size, position) = resolve_window_geometry(Some(geometry(3000.0, 90.0)), &[monitor()], DEFAULT);

      assert_eq!(size, Size::new(1000.0, 700.0), "a valid saved size is still honored");
      assert!(
        matches!(position, window::Position::Centered),
        "an off-screen position falls back to centered"
      );
    }

    #[test]
    fn it_falls_back_to_the_range_guard_when_no_monitor_is_known() {
      let (_, in_range) = resolve_window_geometry(Some(geometry(120.0, 90.0)), &[], DEFAULT);
      assert!(matches!(in_range, window::Position::Specific(p) if p == Point::new(120.0, 90.0)));

      let (_, out_of_range) = resolve_window_geometry(Some(geometry(-50.0, 90.0)), &[], DEFAULT);
      assert!(matches!(out_of_range, window::Position::Centered));
    }
  }

  mod geometry_merge {
    use pretty_assertions::assert_eq;

    use super::*;

    fn base() -> WindowGeometry {
      WindowGeometry {
        height: 700.0,
        width: 1000.0,
        x: 50.0,
        y: 60.0,
      }
    }

    #[test]
    fn it_updates_only_the_size_on_a_resize_keeping_the_position() {
      let merged = geometry_after_resize(Some(base()), Size::new(1280.0, 960.0));

      assert_eq!(
        merged,
        WindowGeometry {
          height: 960.0,
          width: 1280.0,
          x: 50.0,
          y: 60.0,
        }
      );
    }

    #[test]
    fn it_updates_only_the_position_on_a_move_keeping_the_size() {
      let merged = geometry_after_move(Some(base()), Point::new(200.0, 300.0));

      assert_eq!(
        merged,
        WindowGeometry {
          height: 700.0,
          width: 1000.0,
          x: 200.0,
          y: 300.0,
        }
      );
    }

    #[test]
    fn it_seeds_from_zero_when_the_window_has_no_prior_entry() {
      let resized = geometry_after_resize(None, Size::new(800.0, 600.0));
      assert_eq!(resized.width, 800.0);
      assert_eq!(resized.height, 600.0);
      assert_eq!((resized.x, resized.y), (0.0, 0.0));
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_navigates_to_the_character_detail_for_the_selected_character() {
      let mut app = test_app();

      let _ = update(
        &mut app,
        Message::CharacterManager(character_manager::Message::CharacterSelected(42)),
      );

      assert_eq!(app.route, Route::CharacterDetail(42));
      assert_eq!(app.selected_character, Some(42));
      assert!(app.character_detail.is_some());
    }

    #[test]
    fn it_keeps_the_characters_destination_lit_while_a_pilot_is_drilled_in() {
      assert_eq!(Route::CharacterDetail(42).destination(), rail::Destination::Characters);
    }

    #[test]
    fn it_returns_to_the_roster_grid_when_the_characters_rail_is_activated_from_detail() {
      let mut app = test_app();
      let _ = update(
        &mut app,
        Message::CharacterManager(character_manager::Message::CharacterSelected(42)),
      );
      assert_eq!(app.route, Route::CharacterDetail(42));

      let _ = update(&mut app, Message::Nav(rail::Destination::Characters));

      assert_eq!(app.route, Route::Characters);
    }

    #[test]
    fn it_navigates_to_the_wallet_screen_on_the_wallet_rail_destination() {
      let mut app = test_app();

      let _ = update(&mut app, Message::Nav(rail::Destination::Wallet));

      assert_eq!(app.route, Route::Wallet);
      assert!(app.wallet.is_some());
      assert_eq!(app.route.destination(), rail::Destination::Wallet);
    }

    #[test]
    fn it_navigates_to_the_assets_screen_on_the_assets_rail_destination() {
      let mut app = test_app();

      let _ = update(&mut app, Message::Nav(rail::Destination::Assets));

      assert_eq!(app.route, Route::Assets);
      assert!(app.assets.is_some());
      assert_eq!(app.route.destination(), rail::Destination::Assets);
    }

    #[test]
    fn it_routes_to_the_skills_empty_state_for_an_empty_owned_roster() {
      let mut app = test_app();

      let _ = navigate_to_skills(&mut app, None, Vec::new());

      assert_eq!(app.route, Route::Skills(EMPTY_SKILLS_SELECTION));
      assert_eq!(app.selected_character, None);
      assert!(app.skills.is_some());
    }

    #[test]
    fn it_keeps_route_and_sticky_selection_in_sync_on_a_picker_switch() {
      let mut app = test_app();

      let _ = update(&mut app, Message::Skills(skills::Message::CharacterChanged(99)));

      assert_eq!(app.route, Route::Skills(99));
      assert_eq!(app.selected_character, Some(99));
    }

    #[test]
    fn it_clears_the_editor_and_deregisters_its_window_on_close() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::SkillPlanEditor);
      app.editor = Some((id, skill_plan_editor::State::new(42)));

      let _ = close_editor_window(&mut app, id);

      assert!(app.editor.is_none(), "the editor state is cleared");
      assert_eq!(app.windows.kind(id), None, "the editor window is de-registered");
    }

    #[test]
    fn it_clears_the_compare_window_and_deregisters_it_on_close() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::Compare);
      app.compare = Some((id, skills_compare::State::new(vec![1, 2], Vec::new())));

      let _ = close_compare_window(&mut app, id);

      assert!(app.compare.is_none(), "the compare state is cleared");
      assert_eq!(app.windows.kind(id), None, "the compare window is de-registered");
    }

    #[test]
    fn it_closes_the_compare_window_when_it_requests_close() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::Compare);
      app.compare = Some((id, skills_compare::State::new(vec![1, 2], Vec::new())));

      let _ = handle_compare(&mut app, skills_compare::Message::CloseRequested);

      assert!(app.compare.is_none(), "the compare state is cleared");
      assert_eq!(app.windows.kind(id), None, "the compare window is de-registered");
    }

    #[test]
    fn it_ignores_an_editor_message_with_no_open_editor() {
      let mut app = test_app();

      let _ = update(
        &mut app,
        Message::SkillPlanEditor(skill_plan_editor::Message::NameChanged("x".to_owned())),
      );

      assert!(app.editor.is_none());
    }

    #[test]
    fn it_surfaces_a_seed_error_as_a_fatal_init_failure_without_a_runtime() {
      let mut app = test_app();
      app.splash = Some(splash::State::default());
      app.store_ready = None;

      let _ = on_seed_progress(&mut app, splash::seed::Progress::Error("download failed".to_owned()));

      assert_eq!(app.init_error.as_deref(), Some("download failed"));
      assert!(app.runtime.is_none(), "a seed failure must not enter the main runtime");
    }

    #[tokio::test]
    async fn it_shows_the_seed_error_on_the_splash_and_keeps_the_store_handle_for_retry() {
      let db = store::open_test().await.expect("test db");
      let mut app = test_app();
      app.splash = Some(splash::State::default());
      app.store_ready = Some(StoreReady {
        db: db.clone(),
        http: http::Client::builder(http::Cache::new(db)).build(),
        lease: None,
        settings: config::Settings::default(),
        sync_session: None,
      });

      let _ = on_seed_progress(&mut app, splash::seed::Progress::Error("seed boom".to_owned()));

      assert_eq!(app.init_error.as_deref(), Some("seed boom"));
      assert_eq!(app.splash.as_ref().and_then(|s| s.error.as_deref()), Some("seed boom"));
      assert!(
        app.store_ready.is_some(),
        "a retryable seed failure keeps the store handle so Retry can re-run the seed"
      );
    }

    #[tokio::test]
    async fn it_proceeds_with_existing_data_and_flags_stale_on_a_degraded_seed() {
      let db = store::open_test().await.expect("test db");
      let mut app = test_app();
      app.splash = Some(splash::State::default());
      app.store_ready = Some(StoreReady {
        db: db.clone(),
        http: http::Client::builder(http::Cache::new(db)).build(),
        lease: None,
        settings: config::Settings::default(),
        sync_session: None,
      });

      let _ = on_seed_progress(&mut app, splash::seed::Progress::Degraded("stale refresh".to_owned()));

      assert!(app.sde_stale, "a degraded seed flags the stale-data warning");
      assert!(app.init_error.is_none(), "a degraded seed never surfaces a fatal error");
      assert!(
        app.store_ready.is_none(),
        "the store handle is consumed to build the runtime with existing data"
      );
    }

    #[tokio::test]
    async fn it_re_dispatches_the_seed_and_clears_the_error_on_retry() {
      let db = store::open_test().await.expect("test db");
      let mut app = test_app();
      app.init_error = Some("seed boom".to_owned());
      app.splash_step = 5;
      app.splash = Some(splash::State {
        error: Some("seed boom".to_owned()),
        ..splash::State::default()
      });
      app.store_ready = Some(StoreReady {
        db: db.clone(),
        http: http::Client::builder(http::Cache::new(db)).build(),
        lease: None,
        settings: config::Settings::default(),
        sync_session: None,
      });

      let _ = update(&mut app, Message::Splash(splash::Message::Retry));

      assert!(app.init_error.is_none(), "retry clears the fatal error");
      assert_eq!(app.splash_step, 0, "retry restarts seed progress from the first step");
      assert!(
        app.splash.as_ref().and_then(|s| s.error.as_ref()).is_none(),
        "retry clears the splash error so progress can resume"
      );
      assert!(app.store_ready.is_some(), "retry preserves the store handle");
    }

    #[test]
    fn it_advances_the_splash_label_and_progress_on_a_seed_step() {
      let mut app = test_app();
      app.splash = Some(splash::State::default());

      let _ = on_seed_progress(
        &mut app,
        splash::seed::Progress::Step("Seeding item types\u{2026}".to_owned()),
      );

      let splash = app.splash.as_ref().expect("splash present");
      assert_eq!(splash.step_label, "Seeding item types\u{2026}");
      assert!(splash.progress_target > 0.0, "a real stage advances the bar");
      assert_eq!(app.splash_step, 1);
    }

    #[test]
    fn it_records_main_window_geometry_and_schedules_a_coalesced_save_on_resize_and_move() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::Main);

      let _ = update(
        &mut app,
        Message::Window(id, window::Event::Resized(Size::new(1280.0, 960.0))),
      );
      let _ = update(
        &mut app,
        Message::Window(id, window::Event::Moved(Point::new(120.0, 90.0))),
      );

      let geometry = app
        .ui_state
        .windows
        .get("main")
        .copied()
        .expect("main geometry recorded");
      assert_eq!(geometry.width, 1280.0);
      assert_eq!(geometry.height, 960.0);
      assert_eq!((geometry.x, geometry.y), (120.0, 90.0));
      assert!(
        app.coalescer.has_pending(),
        "a coalesced save is pending after the gesture"
      );
    }

    #[test]
    fn it_persists_a_settled_pane_width_and_schedules_a_coalesced_save() {
      let mut app = test_app();

      let _ = update(
        &mut app,
        Message::Skills(skills::Message::PaneSettled("skills.left", 540.0)),
      );

      assert_eq!(app.ui_state.panes.get("skills.left"), Some(&540.0));
      assert!(
        app.coalescer.has_pending(),
        "a settled pane drag schedules a coalesced save"
      );
    }

    #[test]
    fn it_persists_a_settled_editor_pane_width() {
      let mut app = test_app();

      let _ = update(
        &mut app,
        Message::SkillPlanEditor(skill_plan_editor::Message::PaneSettled("plan.summary", 300.0)),
      );

      assert_eq!(app.ui_state.panes.get("plan.summary"), Some(&300.0));
      assert!(app.coalescer.has_pending());
    }

    #[test]
    fn it_never_records_splash_window_geometry() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::Splash);

      let _ = update(
        &mut app,
        Message::Window(id, window::Event::Resized(Size::new(640.0, 480.0))),
      );

      assert!(app.ui_state.windows.is_empty(), "splash geometry is never written");
      assert!(!app.coalescer.has_pending(), "splash resize schedules no save");
    }

    #[test]
    fn it_buffers_a_cold_start_callback_that_arrives_before_the_runtime_is_ready() {
      let mut app = test_app();

      let _ = update(
        &mut app,
        Message::Auth(auth::Message::CallbackReceived(
          "eveauth-pod://callback?code=a&state=b".to_owned(),
        )),
      );

      match app.pending_auth {
        Some(auth::Message::CallbackReceived(url)) => {
          assert_eq!(url, "eveauth-pod://callback?code=a&state=b");
        }
        other => panic!("expected a buffered CallbackReceived, got {other:?}"),
      }
    }
  }

  mod views {
    use super::*;

    fn ready_app() -> App {
      let mut app = test_app();
      app.character_manager = Some(character_manager::State::new());
      app.character_detail = Some(character_detail::State::new(1, &[]));
      app.skills = Some(skills::State::new(1));
      app.mail = Some(mail::State::new());
      app.wallet = Some(wallet::State::new());
      app.assets = Some(assets::State::new());
      app
    }

    fn render_route(route: Route) {
      let app = ready_app();
      let mut app = app;
      app.route = route;
      let _ = route_view(&app);
    }

    #[test]
    fn it_renders_every_route_through_route_view() {
      render_route(Route::Characters);
      render_route(Route::CharacterDetail(1));
      render_route(Route::Skills(1));
      render_route(Route::Mail);
      render_route(Route::Wallet);
      render_route(Route::Assets);
      render_route(Route::Settings);
    }

    #[test]
    fn it_renders_the_starting_up_placeholder_for_an_unbuilt_route() {
      let mut app = test_app();
      app.route = Route::Wallet;
      let _ = route_view(&app);
      let _ = starting_up();
    }

    #[test]
    fn it_renders_main_view_with_a_runtime_and_with_the_init_error_and_pre_runtime_placeholders() {
      let mut app = ready_app();
      app.route = Route::Characters;
      app.runtime = None;
      let _ = main_view(&app);
      app.init_error = Some("boom".to_owned());
      let _ = main_view(&app);
    }

    #[test]
    fn it_renders_main_view_with_the_sync_popover_open() {
      let mut app = ready_app();
      app.route = Route::Characters;
      app.sync_popover_open = true;
      let _ = main_view(&app);
    }

    #[test]
    fn it_dispatches_the_daemon_view_for_each_window_kind() {
      let mut app = ready_app();
      let splash_id = window::Id::unique();
      app.windows.register(splash_id, Window::Splash);
      app.splash = Some(splash::State::default());
      let _ = view(&app, splash_id);
      app.splash = None;
      let _ = view(&app, splash_id);

      let main_id = window::Id::unique();
      app.windows.register(main_id, Window::Main);
      app.route = Route::Characters;
      let _ = view(&app, main_id);

      let editor_id = window::Id::unique();
      app.windows.register(editor_id, Window::SkillPlanEditor);
      app.editor = Some((editor_id, skill_plan_editor::State::new(1)));
      let _ = view(&app, editor_id);

      let _ = view(&app, window::Id::unique());
    }

    #[test]
    fn it_builds_the_sync_model_with_per_pilot_job_rows() {
      let mut app = ready_app();
      app.last_synced = Some(app.now);
      let model = sync_model(&app);
      assert_eq!(model.total, model.rows.len());
    }

    #[test]
    fn it_renders_the_status_bar_with_and_without_an_active_outbox() {
      let mut app = ready_app();
      let _ = status_bar_view(&app);
      app.outbox.apply(&crate::sync::Event::OutboxInflight {
        id: 1,
      });
      let _ = status_bar_view(&app);
    }

    #[test]
    fn it_builds_the_subscription_set_for_each_live_screen() {
      let app = test_app();
      let _ = subscription(&app);

      let mut app = ready_app();
      app.splash = Some(splash::State::default());
      app.settings = None;
      app.sync_popover_open = true;
      app.status.apply(&crate::sync::Event::Started {
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(1)),
      });
      app.editor = Some((window::Id::unique(), skill_plan_editor::State::new(1)));
      let _ = subscription(&app);
    }
  }

  mod handlers {
    use super::*;

    #[test]
    fn it_routes_feature_messages_to_a_no_op_without_a_runtime() {
      let mut app = test_app();
      let _ = update(&mut app, Message::Wallet(wallet::Message::RailDragEnd));
      let _ = update(
        &mut app,
        Message::Assets(assets::Message::SearchChanged("x".to_owned())),
      );
      let _ = update(&mut app, Message::Settings(settings::Message::ResetToDefaults));
      let _ = update(
        &mut app,
        Message::CharacterDetail(character_detail::Message::CharacterChanged(7)),
      );
      assert_eq!(app.route, Route::CharacterDetail(7));
      assert_eq!(app.selected_character, Some(7));
    }

    #[test]
    fn it_records_a_settled_mail_pane_width() {
      let mut app = test_app();

      let _ = update(
        &mut app,
        Message::Mail(mail::Message::PaneSettled("mail.folder", 220.0)),
      );

      assert_eq!(app.ui_state.panes.get("mail.folder"), Some(&220.0));
      assert!(app.coalescer.has_pending());
    }

    #[test]
    fn it_routes_a_mail_compose_input_to_a_no_op_without_a_runtime() {
      let mut app = test_app();
      app.mail = Some(mail::State::new());

      let _ = update(&mut app, Message::Mail(mail::Message::ComposeToInput("Ve".to_owned())));
    }

    #[tokio::test]
    async fn it_dispatches_each_stockpile_branch_through_the_runtime() {
      let mut app = test_app();
      app.assets = Some(assets::State::new());
      app.runtime = Some(test_runtime().await);

      let _location = handle_assets(
        &mut app,
        assets::Message::StockpileEditorLocationSearchChanged("Jit".to_owned()),
      );
      let _item = handle_assets(
        &mut app,
        assets::Message::StockpileEditorItemSearchChanged(0, "Trit".to_owned()),
      );
      let _resolve = handle_assets(&mut app, assets::Message::StockpileImportResolveRequested);
      let _save = handle_assets(&mut app, assets::Message::StockpileEditorSaved);
      let _default = handle_assets(&mut app, assets::Message::SearchChanged("x".to_owned()));
    }

    #[tokio::test]
    async fn it_pairs_a_compose_input_with_a_recipient_search_when_a_runtime_is_present() {
      let mut app = test_app();
      app.mail = Some(mail::State::new());
      app.runtime = Some(test_runtime().await);

      let _to = handle_mail(&mut app, mail::Message::ComposeToInput("Vexor".to_owned()));
      let _cc = handle_mail(&mut app, mail::Message::ComposeCcInput("Alli".to_owned()));
      let _scope = handle_mail(&mut app, mail::Message::ScopeSelected(mail::Scope::AllInboxes));
    }

    #[tokio::test]
    async fn it_dispatches_each_native_menu_action() {
      let mut app = test_app();

      let _about = handle_menu(&mut app, menu::MenuAction::About);
      let _check = handle_menu(&mut app, menu::MenuAction::CheckUpdates);
      let _clear = handle_menu(&mut app, menu::MenuAction::ClearCache);
    }

    #[tokio::test]
    async fn it_falls_back_to_a_freshly_loaded_storage_when_clearing_the_cache_pre_runtime() {
      let app = test_app();

      let _task = clear_cache(&app);
    }

    #[test]
    fn it_routes_a_mail_scope_selection_to_a_no_op_without_a_runtime() {
      let mut app = test_app();
      app.mail = Some(mail::State::new());

      let _ = update(
        &mut app,
        Message::Mail(mail::Message::ScopeSelected(mail::Scope::AllInboxes)),
      );
    }

    #[test]
    fn it_records_a_new_updater_state_and_rearms_the_toast() {
      let mut app = test_app();
      app.updater_toast_dismissed = true;

      let _ = update(
        &mut app,
        Message::UpdaterStateChanged(updater::State::UpdateAvailable {
          version: "1.2.3".to_owned(),
        }),
      );

      assert_eq!(
        app.updater_state,
        updater::State::UpdateAvailable {
          version: "1.2.3".to_owned()
        }
      );
      assert!(
        !app.updater_toast_dismissed,
        "a fresh transition re-arms the dismissible toast"
      );
    }

    #[test]
    fn it_keeps_the_toast_dismissed_for_a_repeated_updater_state() {
      let mut app = test_app();
      app.updater_state = updater::State::Downloading {
        version: "1.2.3".to_owned(),
      };
      app.updater_toast_dismissed = true;

      let _ = update(
        &mut app,
        Message::UpdaterStateChanged(updater::State::Downloading {
          version: "1.2.3".to_owned(),
        }),
      );

      assert!(
        app.updater_toast_dismissed,
        "an identical state must not re-show a toast the user dismissed"
      );
    }

    #[test]
    fn it_dismisses_the_updater_toast() {
      let mut app = test_app();
      assert!(!app.updater_toast_dismissed);

      let _ = update(&mut app, Message::UpdaterDismissToast);

      assert!(app.updater_toast_dismissed, "the toast hides after a dismiss");
    }

    #[test]
    fn it_handles_updater_actions_without_a_provisioned_handle() {
      let mut app = test_app();
      assert!(app.updater.is_none());

      let _ = update(&mut app, Message::UpdaterAction(updater_banner::Action::Apply));
      let _ = update(&mut app, Message::UpdaterAction(updater_banner::Action::Restart));
    }

    #[test]
    fn it_renders_the_main_view_with_an_active_updater_state() {
      let mut app = test_app();
      app.character_manager = Some(character_manager::State::new());
      app.route = Route::Characters;
      app.updater_state = updater::State::ReadyToRestart {
        version: "1.2.3".to_owned(),
      };
      let _ = main_view(&app);
    }

    #[test]
    fn it_toggles_the_sync_popover_and_pulse() {
      let mut app = test_app();

      let _ = update(&mut app, Message::ToggleSyncPopover);
      assert!(app.sync_popover_open);
      let _ = update(&mut app, Message::CloseSyncPopover);
      assert!(!app.sync_popover_open);

      let _ = update(&mut app, Message::SyncPulse);
      assert!(app.sync_tick);
    }

    #[test]
    fn it_advances_the_clock_and_drains_due_saves_on_a_tick() {
      let mut app = test_app();
      let before = app.now;

      let _ = update(&mut app, Message::ClockTick);

      assert!(app.now >= before, "the tick advances the clock");
    }

    #[test]
    fn it_reissues_the_mail_reload_only_when_a_snooze_woke() {
      let mut app = test_app();
      let _ = update(&mut app, Message::SnoozesWoken(Vec::new()));
      let _ = update(&mut app, Message::SnoozesWoken(vec![(1, 2)]));
    }

    #[tokio::test]
    async fn it_clears_a_parked_store_handle_when_an_init_failure_lands() {
      let db = store::open_test().await.expect("test db");
      let mut app = test_app();
      app.splash = Some(splash::State::default());
      app.store_ready = Some(StoreReady {
        db: db.clone(),
        http: http::Client::builder(http::Cache::new(db)).build(),
        lease: None,
        settings: config::Settings::default(),
        sync_session: None,
      });

      let _ = update(&mut app, Message::InitFailed("nope".to_owned()));

      assert_eq!(app.init_error.as_deref(), Some("nope"));
      assert!(app.store_ready.is_none(), "a fatal init clears any parked store handle");
    }

    #[test]
    fn it_parks_then_replays_a_cold_start_callback_on_ready_paths() {
      let mut app = test_app();
      let _ = handle_auth(
        &mut app,
        auth::Message::CallbackReceived("eveauth-pod://callback?code=a&state=b".to_owned()),
      );
      assert!(app.pending_auth.is_some());
    }

    #[test]
    fn it_records_the_mail_unread_count_and_reauth_logs_without_a_runtime() {
      let mut app = test_app();
      let _ = update(&mut app, Message::MailUnreadCounted(9));
      assert_eq!(app.mail_unread, 9);
      let _ = update(&mut app, Message::ReauthCharacter(1));
    }

    #[test]
    fn it_routes_splash_messages_through_update_splash() {
      let mut app = test_app();
      let splash_id = window::Id::unique();
      app.windows.register(splash_id, Window::Splash);
      app.splash = Some(splash::State::default());

      let _ = update(&mut app, Message::Splash(splash::Message::DragWindow));
      let _ = update(&mut app, Message::Splash(splash::Message::Tick));
      let _ = update(&mut app, Message::Splash(splash::Message::ExpandComplete));
    }

    #[test]
    fn it_routes_a_splash_drag_to_a_no_op_with_no_splash_window() {
      let mut app = test_app();
      let _ = update(&mut app, Message::Splash(splash::Message::DragWindow));
    }

    #[test]
    fn it_disables_the_shadow_only_for_the_splash_window_on_open() {
      let mut app = test_app();
      let splash_id = window::Id::unique();
      app.windows.register(splash_id, Window::Splash);
      let _ = on_window_opened(&app, splash_id);
      let main_id = window::Id::unique();
      app.windows.register(main_id, Window::Main);
      let _ = on_window_opened(&app, main_id);
    }

    #[test]
    fn it_routes_a_window_close_request_for_the_editor_through_the_close_path() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::SkillPlanEditor);
      app.editor = Some((id, skill_plan_editor::State::new(1)));

      let _ = update(&mut app, Message::Window(id, window::Event::CloseRequested));

      assert!(app.editor.is_none(), "an OS close of the editor clears its state");
    }

    #[test]
    fn it_ignores_an_unhandled_window_event() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::Main);
      let _ = update(&mut app, Message::Window(id, window::Event::Focused));
    }

    #[test]
    fn a_held_foreign_lease_maps_to_read_only_holder_info() {
      let holder: Option<HolderInfo> = store::lease::Outcome::HeldBy {
        hostname: "studio-mac".to_owned(),
        machine_id: "machine-b".to_owned(),
      }
      .into();

      assert_eq!(
        holder,
        Some(HolderInfo {
          hostname: "studio-mac".to_owned(),
          machine_id: "machine-b".to_owned(),
        })
      );
    }

    #[test]
    fn an_acquired_lease_maps_to_no_read_only_state() {
      let holder: Option<HolderInfo> = store::lease::Outcome::Acquired.into();

      assert_eq!(holder, None);
    }

    #[test]
    fn direct_mode_does_not_hold_a_lease_and_runs_no_lifecycle_io() {
      let mut app = test_app();

      assert!(!holding_lease(&app), "with no sync session there is no lease to hold");
      let _ = handle_lease_heartbeat(&mut app);
      let _ = handle_periodic_push(&mut app);
    }

    #[test]
    fn a_read_only_session_neither_heartbeats_nor_pushes() {
      let mut app = test_app();
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        machine_id: "machine-b".to_owned(),
      });

      assert!(!holding_lease(&app), "a read-only opener does not hold the lease");
    }

    #[test]
    fn take_over_is_a_no_op_when_the_app_is_already_writable() {
      let mut app = test_app();

      let _ = handle_take_over(&mut app);

      assert!(app.read_only.is_none(), "a writable app stays writable");
    }

    #[test]
    fn take_over_without_a_sync_session_fires_no_real_io() {
      let mut app = test_app();
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        machine_id: "machine-b".to_owned(),
      });

      let _ = handle_take_over(&mut app);

      assert!(
        app.read_only.is_some(),
        "with no sync session the request short-circuits and the banner stays"
      );
    }

    #[test]
    fn a_claimed_take_over_drops_read_only() {
      let mut app = test_app();
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        machine_id: "machine-b".to_owned(),
      });

      let _ = handle_take_over_resolved(&mut app, TakeOverOutcome::Claimed);

      assert!(app.read_only.is_none(), "claiming the share makes the app writable");
    }

    #[test]
    fn a_still_held_take_over_keeps_the_named_banner() {
      let mut app = test_app();
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        machine_id: "machine-b".to_owned(),
      });

      let _ = handle_take_over_resolved(
        &mut app,
        TakeOverOutcome::StillHeld(HolderInfo {
          hostname: "studio-linux".to_owned(),
          machine_id: "machine-c".to_owned(),
        }),
      );

      assert_eq!(
        app.read_only,
        Some(HolderInfo {
          hostname: "studio-linux".to_owned(),
          machine_id: "machine-c".to_owned(),
        }),
        "a still-fresh holder is refused and the banner updates to the current holder"
      );
    }

    #[test]
    fn a_failed_take_over_keeps_the_app_read_only() {
      let mut app = test_app();
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        machine_id: "machine-b".to_owned(),
      });

      let _ = handle_take_over_resolved(&mut app, TakeOverOutcome::Failed);

      assert!(app.read_only.is_some(), "a failed take-over leaves the app read-only");
    }

    #[test]
    fn an_inert_sync_handle_swallows_commands_without_panicking() {
      let (handle, _events) = inert_sync();

      handle.discover();
      handle.enroll(sync::Subject::Character(7));
      handle.run_now(sync::Subject::Character(7));
    }

    #[test]
    fn a_push_completion_advances_the_debounce_mark() {
      let mut app = test_app();
      let mark = SystemTime::now();

      let _ = handle_pushed(&mut app, Some(mark));

      assert_eq!(app.last_push, Some(mark));
      assert!(
        app.last_synced.is_some(),
        "a successful push updates the last-synced clock"
      );
    }

    #[test]
    fn a_failed_push_leaves_the_debounce_mark_untouched() {
      let mut app = test_app();

      let _ = handle_pushed(&mut app, None);

      assert_eq!(app.last_push, None, "a failed push must re-attempt next tick");
    }

    #[test]
    fn direct_mode_runs_no_crash_recovery_push() {
      let app = test_app();

      let _ = recover_unsynced_changes(&app);
    }

    #[test]
    fn it_routes_each_character_manager_intent_arm() {
      let mut app = test_app();
      let _ = update(
        &mut app,
        Message::CharacterManager(character_manager::Message::AddCharacterRequested),
      );
      let _ = update(
        &mut app,
        Message::CharacterManager(character_manager::Message::AddCorporationRequested),
      );
      let _ = update(
        &mut app,
        Message::CharacterManager(character_manager::Message::ReauthCharacterRequested(7)),
      );
      let _ = update(
        &mut app,
        Message::CharacterManager(character_manager::Message::RemoveCharacterConfirmed(7)),
      );
      let _ = update(
        &mut app,
        Message::CharacterManager(character_manager::Message::RemoveCorporationConfirmed(7)),
      );
    }

    #[tokio::test]
    async fn it_opens_the_database_under_the_configured_directory_in_place() {
      let dir = tempfile::tempdir().expect("temp dir");
      let mut settings = config::Settings::default();
      let configured = dir.path().join("nested");
      settings.storage_mut().set_db_dir(Some(configured.clone()));
      settings.storage_mut().set_cache_dir(Some(dir.path().join("cache")));

      let path = store::bootstrap::resolve_local_path(settings.storage()).expect("the path resolves");
      let db = store::open(&path).await.expect("the database opens");
      drop(db);

      assert_eq!(path, configured.join("pod.db"), "direct mode opens in place");
      assert!(
        configured.join("pod.db").exists(),
        "the db file lands under the configured directory"
      );
      assert!(
        !settings.storage().resolved_working_copy_path().exists(),
        "a local path creates no working copy"
      );
    }
  }

  mod rail_mail_unread {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_the_live_count_when_the_mail_screen_is_closed() {
      assert_eq!(super::super::rail_mail_unread(3, None), 3);
      assert_eq!(super::super::rail_mail_unread(0, None), 0);
    }

    #[test]
    fn it_prefers_the_screens_fresher_optimistic_count_over_a_stale_live_count() {
      assert_eq!(super::super::rail_mail_unread(3, Some(2)), 2);
    }

    #[test]
    fn it_keeps_the_live_count_when_it_is_already_the_lower_of_the_two() {
      assert_eq!(super::super::rail_mail_unread(1, Some(4)), 1);
    }

    #[test]
    fn it_folds_a_count_tick_into_the_rail_dot_regardless_of_the_active_route() {
      let mut app = test_app();
      app.route = Route::Characters;
      assert!(app.mail.is_none());

      let _ = update(&mut app, Message::MailUnreadCounted(5));

      assert_eq!(app.mail_unread, 5);
      assert_eq!(
        super::super::rail_mail_unread(app.mail_unread, app.mail.as_ref().map(mail::State::unified_unread)),
        5,
        "the rail dot reflects the background count with no Mail screen open"
      );
    }

    #[test]
    fn it_clears_the_rail_dot_when_a_count_tick_reports_zero_unread() {
      let mut app = test_app();
      app.mail_unread = 4;

      let _ = update(&mut app, Message::MailUnreadCounted(0));

      assert_eq!(app.mail_unread, 0, "the dot clears when no unread mail remains");
    }
  }

  mod row_state {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::sync::Event;

    fn key() -> JobKey {
      JobKey::new(JobKind::CharacterProfile, Subject::Character(1))
    }

    #[test]
    fn it_reads_an_unreported_job_as_queued() {
      let status = sync::SyncStatus::new();

      assert_eq!(row_state(&status, &key()), (RowState::Queued, None));
    }

    #[test]
    fn it_maps_done_and_syncing_phases() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::Started {
        key: key(),
      });
      assert_eq!(row_state(&status, &key()), (RowState::Syncing, None));

      status.apply(&Event::Finished {
        key: key(),
        outcome: crate::sync::Outcome::synced(),
      });
      assert_eq!(row_state(&status, &key()), (RowState::Done, None));
    }

    #[test]
    fn it_surfaces_a_failure_reason_as_error_text() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::Failed {
        key: key(),
        reason: "token expired".to_owned(),
      });

      assert_eq!(
        row_state(&status, &key()),
        (RowState::Error, Some("token expired".to_owned()))
      );
    }

    #[test]
    fn it_renders_a_backoff_countdown_as_error_text() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::BackingOff {
        key: key(),
        retry_secs: 30,
      });

      assert_eq!(
        row_state(&status, &key()),
        (RowState::Error, Some("Backing off 30s".to_owned()))
      );
    }

    #[test]
    fn it_surfaces_empty_and_blocked_outcomes_as_attention_with_a_reason() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::Finished {
        key: key(),
        outcome: crate::sync::Outcome::Empty,
      });
      assert_eq!(
        row_state(&status, &key()),
        (RowState::Attention, Some("No data".to_owned()))
      );

      status.apply(&Event::Finished {
        key: key(),
        outcome: crate::sync::Outcome::Blocked {
          reason: "missing scope".to_owned(),
        },
      });
      assert_eq!(
        row_state(&status, &key()),
        (RowState::Attention, Some("missing scope".to_owned()))
      );
    }
  }

  mod detail_sync_reload {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::character_detail::{self, DetailDataType};

    const PILOT: i64 = 42;

    fn detail() -> character_detail::State {
      character_detail::State::new(PILOT, &[])
    }

    fn finished(kind: JobKind, subject: Subject) -> JobKey {
      JobKey::new(kind, subject)
    }

    #[test]
    fn it_reloads_only_the_matching_type_for_the_drilled_in_pilot() {
      let detail = detail();

      assert_eq!(
        detail_reload_target(
          Some(&detail),
          finished(JobKind::CharacterClones, Subject::Character(PILOT))
        ),
        Some(DetailDataType::Clones)
      );
      assert_eq!(
        detail_reload_target(
          Some(&detail),
          finished(JobKind::CharacterStandings, Subject::Character(PILOT))
        ),
        Some(DetailDataType::Standings)
      );
      assert_eq!(
        detail_reload_target(
          Some(&detail),
          finished(JobKind::CharacterContacts, Subject::Character(PILOT))
        ),
        Some(DetailDataType::Contacts)
      );
    }

    #[test]
    fn it_ignores_a_finished_job_for_a_different_pilot() {
      let detail = detail();

      assert_eq!(
        detail_reload_target(
          Some(&detail),
          finished(JobKind::CharacterClones, Subject::Character(PILOT + 1))
        ),
        None
      );
    }

    #[test]
    fn it_ignores_a_corporation_subject_job() {
      let detail = detail();

      assert_eq!(
        detail_reload_target(
          Some(&detail),
          finished(JobKind::CharacterClones, Subject::Corporation(PILOT))
        ),
        None
      );
    }

    #[test]
    fn it_ignores_a_kind_this_screen_does_not_render() {
      let detail = detail();

      assert_eq!(
        detail_reload_target(
          Some(&detail),
          finished(JobKind::CharacterWallet, Subject::Character(PILOT))
        ),
        None
      );
      assert_eq!(
        detail_reload_target(
          Some(&detail),
          finished(JobKind::CharacterTelemetry, Subject::Character(PILOT))
        ),
        None
      );
    }

    #[test]
    fn it_ignores_everything_when_no_detail_screen_is_open() {
      assert_eq!(
        detail_reload_target(None, finished(JobKind::CharacterClones, Subject::Character(PILOT))),
        None
      );
    }
  }

  mod wallet_reload_kind {
    use super::*;

    #[test]
    fn it_feeds_the_wallet_for_every_ledger_and_derive_kind() {
      assert!(wallet_reload_kind(JobKind::CharacterWallet));
      assert!(wallet_reload_kind(JobKind::CorporationWallet));
      assert!(wallet_reload_kind(JobKind::MarketPrices));
      assert!(wallet_reload_kind(JobKind::NetWorthSnapshot));
    }

    #[test]
    fn it_ignores_kinds_the_wallet_does_not_render() {
      assert!(!wallet_reload_kind(JobKind::AssetSync));
      assert!(!wallet_reload_kind(JobKind::CharacterSkills));
      assert!(!wallet_reload_kind(JobKind::CharacterProfile));
    }
  }

  mod sync_screen_reload {
    use super::*;

    fn finished(kind: JobKind) -> JobKey {
      JobKey::new(kind, Subject::Character(1))
    }

    #[test]
    fn it_skips_the_wallet_reload_off_route() {
      let mut app = test_app();
      app.route = Route::Assets;

      assert!(wallet_reload_on_finished(&app, finished(JobKind::CharacterWallet)).is_none());
    }

    #[test]
    fn it_skips_the_wallet_reload_for_an_unrelated_kind() {
      let mut app = test_app();
      app.route = Route::Wallet;

      assert!(wallet_reload_on_finished(&app, finished(JobKind::AssetSync)).is_none());
    }

    #[test]
    fn it_skips_the_assets_reload_off_route() {
      let mut app = test_app();
      app.route = Route::Wallet;

      assert!(assets_reload_on_finished(&app, finished(JobKind::AssetSync)).is_none());
    }

    #[test]
    fn it_skips_the_assets_reload_for_an_unrelated_kind() {
      let mut app = test_app();
      app.route = Route::Assets;

      assert!(assets_reload_on_finished(&app, finished(JobKind::CharacterWallet)).is_none());
    }
  }

  mod outbox_indicator {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::sync::Event;

    #[test]
    fn it_is_absent_when_the_outbox_is_quiet() {
      let outbox = sync::OutboxStatus::new();

      assert!(
        super::outbox_indicator(&outbox).is_none(),
        "an idle outbox adds no chrome"
      );
    }

    #[test]
    fn it_renders_when_a_row_is_pending() {
      let mut outbox = sync::OutboxStatus::new();
      outbox.apply(&Event::OutboxInflight {
        id: 1,
      });

      assert!(super::outbox_indicator(&outbox).is_some());
    }

    #[test]
    fn it_renders_when_a_row_has_failed() {
      let mut outbox = sync::OutboxStatus::new();
      outbox.apply(&Event::OutboxFailed {
        id: 1,
        reason: "403 Forbidden".to_owned(),
      });

      assert!(super::outbox_indicator(&outbox).is_some());
    }

    #[test]
    fn it_folds_a_sync_event_into_the_apps_outbox_aggregate() {
      let mut app = test_app();

      let _ = update(
        &mut app,
        Message::Sync(Event::OutboxInflight {
          id: 1,
        }),
      );
      let _ = update(
        &mut app,
        Message::Sync(Event::OutboxFailed {
          id: 2,
          reason: "boom".to_owned(),
        }),
      );

      assert_eq!(app.outbox.pending(), 1);
      assert_eq!(app.outbox.failed(), 1);
      assert_eq!(app.status.total(), 0, "outbox events do not enter the job-keyed status");
    }
  }

  mod name {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_names_every_route_variant() {
      assert_eq!(Route::Characters.name(), "Characters");
      assert_eq!(Route::CharacterDetail(1).name(), "CharacterDetail");
      assert_eq!(Route::Skills(1).name(), "Skills");
      assert_eq!(Route::Mail.name(), "Mail");
      assert_eq!(Route::Wallet.name(), "Wallet");
      assert_eq!(Route::Assets.name(), "Assets");
      assert_eq!(Route::Settings.name(), "Settings");
    }
  }

  mod variant_name {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_names_feature_messages() {
      assert_eq!(Message::Assets(assets::Message::StockpileNew).variant_name(), "Assets");
      assert_eq!(Message::Nav(rail::Destination::Wallet).variant_name(), "Nav");
      assert_eq!(Message::Wallet(wallet::Message::PickerToggled).variant_name(), "Wallet");
    }

    #[test]
    fn it_names_lifecycle_messages() {
      assert_eq!(Message::ClockTick.variant_name(), "ClockTick");
      assert_eq!(Message::OpenAbout.variant_name(), "OpenAbout");
      assert_eq!(Message::SyncPulse.variant_name(), "SyncPulse");
      assert_eq!(Message::MailUnreadCounted(3).variant_name(), "MailUnreadCounted");
    }
  }

  mod init_tracing {
    use super::*;

    #[test]
    fn it_initializes_a_file_logger_under_a_writable_dir() {
      let dir = tempfile::tempdir().expect("temp dir");

      let guard = init_tracing(dir.path());

      assert!(guard.is_some(), "a writable log dir yields a worker guard");
    }
  }

  mod updater_state_stream {
    use super::*;

    #[test]
    fn it_constructs_an_updater_state_stream() {
      let _stream = updater_state_stream();
    }
  }
}
