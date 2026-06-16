mod graphics;
mod snooze_scheduler;
mod windows;

use std::{
  collections::HashSet,
  sync::{Arc, OnceLock},
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

#[cfg(target_os = "macos")]
use crate::services::menu;
use crate::{
  clients::{self, esi, eve_image, eve_sso, http},
  config,
  features::{
    about, assets, auth, calendar, character_detail, character_manager, character_manager::OwnedPilot,
    corporation_detail, industry, mail, registry, settings, skill_plan_editor, skills, skills_compare, splash, wallet,
  },
  services::updater,
  store,
  sync::{self, JobKey, JobKind, Phase, Subject},
  ui::{
    components::{
      backdrop,
      esi_status::esi_status,
      eve_time::eve_time,
      rail::{self, rail},
      status, sync_chip,
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
// Intentionally omits `pod=<level>`; the active level is prepended at runtime by `file_filter()`.
const FILE_FILTER_CLAMP: &str = "warn,\
  hyper=warn,\
  reqwest=warn,\
  iced=warn,\
  iced_wgpu=info,\
  iced_winit=warn,\
  wgpu=warn,\
  wgpu_core=info,\
  wgpu_hal=warn,\
  sqlx=warn,\
  sqlx::query=warn";
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
const PERIODIC_PULL_INTERVAL: Duration = Duration::from_secs(60);
const PERIODIC_PUSH_INTERVAL: Duration = Duration::from_secs(60);
const POPOVER_LEFT: f32 = spacing::SPACE_3_5;
const PULSE_INTERVAL: Duration = Duration::from_millis(450);
const REACQUIRE_INTERVAL: Duration = Duration::from_secs(30);
const RUNTIME_CHANNEL_BUFFER: usize = 64;
const SCALE_MAX: u8 = 150;
const SCALE_MIN: u8 = 85;
const ZERO_GEOMETRY: WindowGeometry = WindowGeometry {
  height: 0.0,
  width: 0.0,
  x: 0.0,
  y: 0.0,
};

type FileFilterReloadHandle =
  OnceLock<tracing_subscriber::reload::Handle<tracing_subscriber::EnvFilter, tracing_subscriber::Registry>>;
type Tx = iced::futures::channel::mpsc::Sender<Message>;

static UPDATER_RECEIVER: std::sync::Mutex<Option<tokio::sync::watch::Receiver<updater::State>>> =
  std::sync::Mutex::new(None);

struct App {
  about: Option<window::Id>,
  accessibility: config::AccessibilityConfig,
  assets: Option<assets::State>,
  assets_dirty: bool,
  auth: auth::State,
  calendar: Option<calendar::State>,
  calendar_attention: i64,
  character_detail: Option<character_detail::State>,
  character_manager: Option<character_manager::State>,
  coalescer: WriteCoalescer,
  compare: Option<(window::Id, skills_compare::State)>,
  /// Whether the data-loss confirmation gate is open. `true` means the first "Take over" click has
  /// been received but the share has not yet been claimed — the forceful claim fires only on the
  /// second explicit confirmation.
  confirm_force_takeover: bool,
  corporation_detail: Option<corporation_detail::State>,
  detail_dirty: HashSet<character_detail::DetailDataType>,
  editor: Option<(window::Id, skill_plan_editor::State)>,
  engine_state: EngineState,
  esi_connected: bool,
  industry: Option<industry::State>,
  init_error: Option<String>,
  last_push: Option<SystemTime>,
  last_synced: Option<DateTime<Utc>>,
  mail: Option<mail::State>,
  mail_unread: i64,
  now: DateTime<Utc>,
  outbox: sync::OutboxStatus,
  pending_auth: Option<auth::Message>,
  pending_images: HashSet<(store::images::ImageKind, i64)>,
  read_only: Option<HolderInfo>,
  roster_dirty: bool,
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
  wallet_dirty: bool,
  windows: Windows,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
enum EngineState {
  Idle,
  ReadOnly {
    held_by: Option<HolderInfo>,
  },
  #[default]
  Running,
  Stopped {
    reason: Option<String>,
  },
}

impl EngineState {
  fn is_read_only(&self) -> bool {
    matches!(self, EngineState::ReadOnly { .. })
  }

  fn is_stopped(&self) -> bool {
    matches!(self, EngineState::Stopped { .. })
  }

  fn settled(&self) -> bool {
    self.is_stopped() || self.is_read_only()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HolderInfo {
  hostname: String,
  last_active: DateTime<Utc>,
  machine_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SyncNowOutcome {
  Failed,
  Reconciled { mark: Option<SystemTime>, pulled: bool },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TakeOverOutcome {
  Claimed,
  Failed,
}

impl From<store::lease::Outcome> for Option<HolderInfo> {
  fn from(outcome: store::lease::Outcome) -> Self {
    match outcome {
      store::lease::Outcome::Acquired => None,
      store::lease::Outcome::HeldBy {
        hostname,
        last_seen,
        machine_id,
      } => Some(HolderInfo {
        hostname,
        last_active: last_seen,
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
  Calendar(calendar::Message),
  CalendarAttentionCounted(i64),
  CancelTakeOver,
  CharacterDetail(character_detail::Message),
  CharacterManager(character_manager::Message),
  ClockTick,
  CloseSyncPopover,
  Compare(skills_compare::Message),
  ConfirmTakeOver,
  CorporationDetail(corporation_detail::Message),
  EngineStopped {
    reason: Option<String>,
  },
  FocusMainWindow,
  ImageReady {
    id: i64,
    kind: store::images::ImageKind,
    ready: bool,
  },
  Industry(industry::Message),
  InitFailed(String),
  LeaseHeartbeat,
  LockReleased,
  Mail(mail::Message),
  MailUnreadCounted(i64),
  #[cfg(target_os = "macos")]
  Menu(menu::MenuAction),
  Nav(rail::Destination),
  // Only constructed by the macOS native menu; the About window has no other entry point yet.
  #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
  OpenAbout,
  PeriodicPull,
  PeriodicPush,
  Pulled(bool),
  Pushed(Option<SystemTime>),
  ReacquireLease,
  Ready(Runtime),
  ReauthCharacter(i64),
  RestartSync,
  SeedProgress(splash::seed::Progress),
  Settings(settings::Message),
  SkillPlanEditor(skill_plan_editor::Message),
  Skills(skills::Message),
  SnoozesWoken(Vec<(i64, i64)>),
  Splash(splash::Message),
  StorageMigrated,
  StoreOpened(Box<StoreReady>),
  Sync(sync::Event),
  SyncNowResolved(SyncNowOutcome),
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
      Message::Calendar(_) => "Calendar",
      Message::CalendarAttentionCounted(_) => "CalendarAttentionCounted",
      Message::CharacterDetail(_) => "CharacterDetail",
      Message::CharacterManager(_) => "CharacterManager",
      Message::Compare(_) => "Compare",
      Message::CorporationDetail(_) => "CorporationDetail",
      Message::Industry(_) => "Industry",
      Message::Mail(_) => "Mail",
      Message::MailUnreadCounted(_) => "MailUnreadCounted",
      #[cfg(target_os = "macos")]
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
    self
      .sync_variant_name()
      .or_else(|| self.updater_variant_name())
      .or_else(|| self.boot_variant_name())
  }

  fn boot_variant_name(&self) -> Option<&'static str> {
    Some(match self {
      Message::ClockTick => "ClockTick",
      Message::FocusMainWindow => "FocusMainWindow",
      Message::ImageReady {
        ..
      } => "ImageReady",
      Message::InitFailed(_) => "InitFailed",
      Message::OpenAbout => "OpenAbout",
      Message::Ready(_) => "Ready",
      Message::ReauthCharacter(_) => "ReauthCharacter",
      Message::SeedProgress(_) => "SeedProgress",
      Message::SnoozesWoken(_) => "SnoozesWoken",
      Message::Splash(_) => "Splash",
      Message::StorageMigrated => "StorageMigrated",
      Message::StoreOpened(_) => "StoreOpened",
      Message::WindowOpened(_) => "WindowOpened",
      _ => return None,
    })
  }

  fn sync_variant_name(&self) -> Option<&'static str> {
    Some(match self {
      Message::CancelTakeOver => "CancelTakeOver",
      Message::CloseSyncPopover => "CloseSyncPopover",
      Message::ConfirmTakeOver => "ConfirmTakeOver",
      Message::EngineStopped {
        ..
      } => "EngineStopped",
      Message::LeaseHeartbeat => "LeaseHeartbeat",
      Message::LockReleased => "LockReleased",
      Message::PeriodicPull => "PeriodicPull",
      Message::PeriodicPush => "PeriodicPush",
      Message::Pulled(_) => "Pulled",
      Message::Pushed(_) => "Pushed",
      Message::ReacquireLease => "ReacquireLease",
      Message::RestartSync => "RestartSync",
      Message::SyncNowResolved(_) => "SyncNowResolved",
      Message::SyncPulse => "SyncPulse",
      Message::TakeOver => "TakeOver",
      Message::TakeOverResolved(_) => "TakeOverResolved",
      Message::ToggleSyncPopover => "ToggleSyncPopover",
      _ => return None,
    })
  }

  fn updater_variant_name(&self) -> Option<&'static str> {
    Some(match self {
      Message::UpdaterAction(_) => "UpdaterAction",
      Message::UpdaterDismissToast => "UpdaterDismissToast",
      Message::UpdaterStateChanged(_) => "UpdaterStateChanged",
      _ => return None,
    })
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Route {
  Assets,
  Calendar,
  CharacterDetail(i64),
  #[default]
  Characters,
  CorporationDetail(i64),
  Industry,
  Mail,
  Settings,
  Skills(i64),
  Wallet,
}

impl From<rail::Destination> for Route {
  fn from(destination: rail::Destination) -> Self {
    match destination {
      rail::Destination::Assets => {
        unreachable!("Assets is routed via Message::Nav, not From")
      }
      rail::Destination::Calendar => Route::Calendar,
      rail::Destination::Characters => Route::Characters,
      rail::Destination::Industry => {
        unreachable!("Industry is routed via Message::Nav, not From")
      }
      rail::Destination::Mail => Route::Mail,
      rail::Destination::Settings => Route::Settings,
      rail::Destination::Skills => {
        unreachable!("Skills is routed via Message::Nav, not From")
      }
      rail::Destination::Wallet => {
        unreachable!("Wallet is routed via Message::Nav, not From")
      }
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
      Route::Calendar => rail::Destination::Calendar,
      Route::Characters | Route::CharacterDetail(_) | Route::CorporationDetail(_) => rail::Destination::Characters,
      Route::Industry => rail::Destination::Industry,
      Route::Mail => rail::Destination::Mail,
      Route::Settings => rail::Destination::Settings,
      Route::Skills(_) => rail::Destination::Skills,
      Route::Wallet => rail::Destination::Wallet,
    }
  }

  fn name(self) -> &'static str {
    match self {
      Route::Assets => "Assets",
      Route::Calendar => "Calendar",
      Route::CharacterDetail(_) => "CharacterDetail",
      Route::Characters => "Characters",
      Route::CorporationDetail(_) => "CorporationDetail",
      Route::Industry => "Industry",
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
  sync_db: store::Database,
  sync_housekeeping_db: store::Database,
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
  let (log_dir, log_level) = config::load()
    .map(|settings| (settings.storage().resolved_log_dir(), *settings.storage().log_level()))
    .unwrap_or_else(|_| (config::log_dir(), config::LogLevel::default()));

  let _log_guard = init_tracing(&log_dir, log_level);
  install_panic_hook();
  graphics::probe();

  iced::daemon(boot, update, view)
    .title(title)
    .theme(theme)
    .scale_factor(scale_factor)
    .subscription(subscription)
    .font(typography::bytes::BODY_REGULAR)
    .font(typography::bytes::BODY_MEDIUM)
    .font(typography::bytes::BODY_SEMIBOLD)
    .font(typography::bytes::MONO_REGULAR)
    .font(typography::bytes::MONO_ITALIC)
    .run()
}

fn apply_log_level(level: config::LogLevel) {
  let filter = file_filter(level);
  // Handle is absent before init_tracing runs or in no-logfile sessions — silently skip.
  let Some(handle) = file_filter_reload_handle().get() else {
    return;
  };
  if let Err(error) = handle.reload(tracing_subscriber::EnvFilter::new(&filter)) {
    tracing::warn!(target: "pod::lifecycle", %error, "could not apply the new log level live");
  }
}

fn file_filter(level: config::LogLevel) -> String {
  let pod = match level {
    config::LogLevel::Normal => "debug",
    config::LogLevel::Quiet => "info",
    config::LogLevel::Verbose => "trace",
  };
  format!("pod={pod},{FILE_FILTER_CLAMP}")
}

fn file_filter_reload_handle() -> &'static FileFilterReloadHandle {
  static HANDLE: OnceLock<FileFilterReloadHandle> = OnceLock::new();
  HANDLE.get_or_init(OnceLock::new)
}

fn init_tracing(
  log_dir: &std::path::Path,
  log_level: config::LogLevel,
) -> Option<tracing_appender::non_blocking::WorkerGuard> {
  use tracing_subscriber::{Layer as _, filter::EnvFilter, fmt, prelude::*, reload};

  let console_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(CONSOLE_DEFAULT_FILTER));
  let console_layer = fmt::layer().compact().with_filter(console_filter);

  let active_file_filter = file_filter(log_level);

  let (file_layer, guard) = match std::fs::create_dir_all(log_dir) {
    Ok(()) => {
      let appender = tracing_appender::rolling::Builder::new()
        .filename_prefix("pod")
        .filename_suffix("log")
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .max_log_files(5)
        .build(log_dir);
      match appender {
        Ok(appender) => {
          let (writer, guard) = tracing_appender::non_blocking(appender);
          let (filter, handle) = reload::Layer::new(EnvFilter::new(&active_file_filter));
          let _ = file_filter_reload_handle().set(handle);
          let layer = fmt::layer()
            .json()
            .with_ansi(false)
            .with_writer(writer)
            .with_filter(filter);
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
    .with(file_layer)
    .with(console_layer)
    .try_init();

  tracing::info!(
    target: "pod::lifecycle",
    version = env!("CARGO_PKG_VERSION"),
    log_dir = %log_dir.display(),
    console_filter = CONSOLE_DEFAULT_FILTER,
    file_filter = %active_file_filter,
    "pod starting up"
  );

  guard
}

fn scale_factor(app: &App, _id: window::Id) -> f32 {
  scale_to_factor(*app.accessibility.scale())
}

fn scale_to_factor(scale: u8) -> f32 {
  f32::from(scale.clamp(SCALE_MIN, SCALE_MAX)) / 100.0
}

/// Installs a process-wide panic hook that records every panic into the tracing JSON file log before
/// the default hook (and the `windows_subsystem = "windows"` console detachment) swallows the stderr
/// message. Without this, a panic in any spawned task — notably the sync engine's top-level task —
/// dies silently and leaves no trace in an exported field log.
///
/// Must be called after [`init_tracing`] so the subscriber already exists, and only on the non-test
/// boot path: tests must not mutate the global panic hook (it would clobber the harness's own hook
/// and break `#[should_panic]` / unwinding diagnostics).
#[cfg(not(test))]
fn install_panic_hook() {
  let default_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |info| {
    log_panic(info);
    default_hook(info);
  }));
}

#[cfg(test)]
fn install_panic_hook() {}

/// Emits a single ERROR event capturing a panic's payload, location, and backtrace under the
/// `pod::lifecycle` target. Split out from the hook closure so a test can drive it through a
/// capturing subscriber without touching the global panic hook.
fn log_panic(info: &std::panic::PanicHookInfo<'_>) {
  let message = info
    .payload()
    .downcast_ref::<&str>()
    .copied()
    .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
    .unwrap_or("<non-string panic payload>");
  let location = info
    .location()
    .map(ToString::to_string)
    .unwrap_or_else(|| "<unknown>".to_owned());
  let backtrace = std::backtrace::Backtrace::force_capture();
  tracing::error!(
    target: "pod::lifecycle",
    panic_message = message,
    panic_location = location,
    panic_backtrace = %backtrace,
    "the process panicked",
  );
}

fn app_icon() -> Option<window::Icon> {
  static ICON: OnceLock<Option<window::Icon>> = OnceLock::new();
  ICON
    .get_or_init(|| {
      const DATA: &[u8] = include_bytes!("../assets/images/identity/256x256.png");
      match window::icon::from_file_data(DATA, None) {
        Ok(icon) => Some(icon),
        Err(error) => {
          tracing::warn!(%error, "failed to load application window icon");
          None
        }
      }
    })
    .clone()
}

fn blank<'a>() -> Element<'a, Message> {
  container(Space::new().width(Length::Fill).height(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn boot() -> (App, Task<Message>) {
  let settings = config::load().unwrap_or_default();
  let accessibility = *settings.accessibility();
  color::set_high_contrast(*accessibility.high_contrast());
  let image_root = settings.storage().resolved_cache_dir().join("images");
  store::images::init_root(image_root);

  auth::install();
  #[cfg(target_os = "macos")]
  menu::init();
  let settings = window::Settings {
    size: Size::new(spacing::layout::SPLASH_WIDTH, spacing::layout::SPLASH_HEIGHT),
    decorations: false,
    resizable: false,
    transparent: true,
    position: window::Position::Centered,
    icon: app_icon(),
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
    accessibility,
    assets: None,
    assets_dirty: false,
    auth: auth::State::default(),
    calendar: None,
    calendar_attention: 0,
    character_detail: None,
    character_manager: None,
    coalescer: WriteCoalescer::new(),
    compare: None,
    confirm_force_takeover: false,
    corporation_detail: None,
    detail_dirty: HashSet::new(),
    editor: None,
    engine_state: EngineState::default(),
    esi_connected: true,
    industry: None,
    init_error: None,
    last_push: None,
    last_synced: None,
    mail: None,
    mail_unread: 0,
    now: Utc::now(),
    outbox: sync::OutboxStatus::new(),
    pending_auth: None,
    pending_images: HashSet::new(),
    read_only: None,
    roster_dirty: false,
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
    wallet_dirty: false,
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

struct PreparedStore {
  database_path: std::path::PathBuf,
  lease: Option<HolderInfo>,
  settings: config::Settings,
  sync_session: Option<store::sync_session::SyncSession>,
}

fn store_err(error: impl std::fmt::Display) -> String {
  error.to_string()
}

fn prepare_store() -> Result<PreparedStore, String> {
  let mut settings = config::load().map_err(store_err)?;
  let machine_id = persist_machine_id(&mut settings);
  let database_path = store::bootstrap::resolve_local_path(settings.storage()).map_err(store_err)?;
  let sync_session = store::sync_session::SyncSession::from_config(settings.storage(), machine_id);
  let lease = acquire_lease(sync_session.as_ref());
  Ok(PreparedStore {
    database_path,
    lease,
    settings,
    sync_session,
  })
}

async fn open_store_inner() -> Result<StoreReady, String> {
  // SEAM (networked-drive storage rework): store prep performs a blocking copy off the (possibly
  // network) share in Sync mode plus lease file IO. Run it on a blocking thread so a stalled or slow
  // mount can't wedge the async boot worker — the first window renders independent of this finishing.
  let prepared = tokio::task::spawn_blocking(prepare_store).await.map_err(store_err)??;
  // The interactive, sync-worker, and housekeeping pools each open against the same database file;
  // see store::open_pools for why the engine gets its own pools rather than sharing the interactive
  // one.
  let pools = store::open_pools(&prepared.database_path).await.map_err(store_err)?;
  let http = http::Client::builder(http::Cache::new(pools.interactive.clone())).build();
  Ok(StoreReady {
    db: pools.interactive,
    http,
    lease: prepared.lease,
    settings: prepared.settings,
    sync_db: pools.sync,
    sync_housekeeping_db: pools.housekeeping,
    sync_session: prepared.sync_session,
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
      return;
    }
  }
  // The channel closes only once the whole supervisor ends — a deliberate Shutdown or app teardown,
  // since a give-up now parks (emitting Event::GaveUp) rather than dropping the stream. Emit a
  // reasonless Stopped so the chip reflects the terminal state even on this teardown path.
  let _ = tx
    .send(Message::EngineStopped {
      reason: None,
    })
    .await;
}

/// Builds a dedicated ESI client whose HTTP cache is backed by the sync pool, keeping sync
/// cache reads/writes off the interactive pool and avoiding contention with UI queries.
fn build_sync_esi(sync_db: store::Database) -> Result<Arc<esi::Client>, String> {
  let sync_http = http::Client::builder(http::Cache::new(sync_db)).build();
  Ok(Arc::new(
    esi::Client::builder(sync_http)
      .user_agent(clients::user_agent())
      .build()
      .map_err(|error| error.to_string())?,
  ))
}

fn build_runtime_inner(ready: StoreReady) -> Result<(Runtime, tokio::sync::mpsc::Receiver<sync::Event>), String> {
  let StoreReady {
    db,
    http,
    lease,
    settings,
    sync_db,
    sync_housekeeping_db,
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
    let sync_esi = build_sync_esi(sync_db.clone())?;
    let started = sync::spawn(
      sync_db,
      sync_housekeeping_db,
      sync_esi,
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
  let (commands, _commands_rx) = tokio::sync::mpsc::unbounded_channel();
  let (restart, _restart_rx) = tokio::sync::mpsc::unbounded_channel();
  let (_events_tx, events) = tokio::sync::mpsc::channel(1);
  (sync::Handle::new(commands, restart), events)
}

fn read_only_engine_state(held_by: Option<HolderInfo>) -> EngineState {
  EngineState::ReadOnly {
    held_by,
  }
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
  let close = match app.windows.kind(id) {
    Some(Window::Compare) => close_compare_window(app, id),
    Some(Window::SkillPlanEditor) => close_editor_window(app, id),
    Some(Window::About) => close_about_window(app, id),
    _ => {
      app.windows.remove(id);
      window::close(id)
    }
  };
  Task::batch([close, shutdown_if_last_window(app)])
}

fn on_window_closed(app: &mut App, id: window::Id) -> Task<Message> {
  let Some(kind) = app.windows.remove(id) else {
    return Task::none();
  };
  match kind {
    Window::About if app.about == Some(id) => app.about = None,
    Window::Compare if app.compare.as_ref().map(|(cid, _)| *cid) == Some(id) => app.compare = None,
    Window::SkillPlanEditor if app.editor.as_ref().map(|(eid, _)| *eid) == Some(id) => app.editor = None,
    _ => {}
  }
  shutdown_if_last_window(app)
}

fn shutdown_if_last_window(app: &mut App) -> Task<Message> {
  if app.windows.is_empty() {
    shutdown(app)
  } else {
    Task::none()
  }
}

fn shutdown(app: &mut App) -> Task<Message> {
  tracing::info!(target: "pod::lifecycle", "shutting down");
  let checkpoint = shutdown_storage(app);
  stop_engines(app);
  checkpoint.chain(Task::batch([iced::exit(), exit_process()]))
}

fn stop_engines(app: &App) {
  if let Some(runtime) = app.runtime.as_ref() {
    runtime.sync.shutdown();
  }
  if let Some(updater) = app.updater.as_ref() {
    updater.shutdown();
  }
}

/// Hard backstop that guarantees the process exits even if a tokio task refuses to drain after
/// `iced::exit()`. Fires only after the storage checkpoint completes (it is chained after it).
fn exit_process() -> Task<Message> {
  Task::future(async {
    std::process::exit(0);
  })
  .discard()
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

fn propagate_host_width(app: &mut App, id: window::Id, width: f32) {
  match app.windows.kind(id) {
    Some(Window::Main) => {
      if let Some(state) = app.skills.as_mut() {
        state.set_pane_host_width(width);
      }
      if let Some(state) = app.mail.as_mut() {
        state.set_pane_host_width(width);
      }
      if let Some(state) = app.wallet.as_mut() {
        state.set_pane_host_width(width);
      }
      if let Some(state) = app.assets.as_mut() {
        state.set_pane_host_width(width);
      }
      if let Some(state) = app.industry.as_mut() {
        state.set_pane_host_width(width);
      }
    }
    Some(Window::SkillPlanEditor) => {
      if let Some((_, state)) = app.editor.as_mut() {
        state.set_pane_host_width(width);
      }
    }
    _ => {}
  }
}

fn record_pane_ratio(app: &mut App, key: &str, ratio: f32) {
  app.ui_state.panes.insert(key.to_owned(), ratio);
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
  app.wallet = Some(wallet::State::new().with_restored_panes(&app.ui_state));
  match app.runtime.as_ref() {
    Some(runtime) => wallet::load(&runtime.db).map(Message::Wallet),
    None => Task::none(),
  }
}

fn navigate_to_mail(app: &mut App, target: Option<i64>) -> Task<Message> {
  navigate(app, Route::Mail);
  match target {
    Some(id) => {
      app.mail = Some(mail::State::new(id).with_restored_panes(&app.ui_state));
      match app.runtime.as_ref() {
        Some(runtime) => mail::load(&runtime.db, id).map(Message::Mail),
        None => Task::none(),
      }
    }
    None => {
      app.mail = Some(mail::State::new(mail::EMPTY_MAIL_SELECTION).with_restored_panes(&app.ui_state));
      Task::none()
    }
  }
}

fn resolve_mail_target(roster: &[OwnedPilot], last_selected: Option<i64>) -> Option<i64> {
  if let Some(id) = last_selected
    && roster.iter().any(|pilot| pilot.id == id)
  {
    return Some(id);
  }
  roster.first().map(|pilot| pilot.id)
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

fn navigate_to_calendar(app: &mut App, target: Option<i64>) -> Task<Message> {
  navigate(app, Route::Calendar);
  let features = calendar_features(app);
  let selection = target.unwrap_or(calendar::EMPTY_CALENDAR_SELECTION);
  app.calendar = Some(calendar::State::new(selection, app.now, features));
  match app.runtime.as_ref() {
    Some(runtime) => calendar::load(&runtime.db, selection, features).map(Message::Calendar),
    None => Task::none(),
  }
}

fn industry_required_scopes() -> Vec<&'static str> {
  registry::descriptor(config::Feature::Industry).scopes.to_vec()
}

fn navigate_to_industry(app: &mut App, target: Option<i64>) -> Task<Message> {
  navigate(app, Route::Industry);
  let required = industry_required_scopes();
  let selection = target.unwrap_or(industry::EMPTY_INDUSTRY_SELECTION);
  let facility_defaults = app
    .runtime
    .as_ref()
    .map(|runtime| industry::FacilityDefaults::from(runtime.settings.industry()))
    .unwrap_or_default();
  app.industry =
    Some(industry::State::new(selection, required.clone(), facility_defaults).with_restored_panes(&app.ui_state));
  match app.runtime.as_ref() {
    Some(runtime) => industry::load(&runtime.db, selection, &required).map(Message::Industry),
    None => Task::none(),
  }
}

fn industry_clock_reload(app: &App) -> Task<Message> {
  if app.route != Route::Industry {
    return Task::none();
  }
  match (app.industry.as_ref(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => {
      industry::reload(&runtime.db, state.active(), &industry_required_scopes()).map(Message::Industry)
    }
    _ => Task::none(),
  }
}

fn calendar_features(app: &App) -> config::FeatureFlags {
  if let Some(state) = app.settings.as_ref() {
    return *state.settings().features();
  }
  if let Some(runtime) = app.runtime.as_ref() {
    return *runtime.settings.features();
  }
  config::FeatureFlags::default()
}

fn calendar_clock_reload(app: &App) -> Task<Message> {
  if app.route != Route::Calendar {
    return Task::none();
  }
  match (app.calendar.as_ref(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => {
      calendar::reload(&runtime.db, state.active(), *runtime.settings.features()).map(Message::Calendar)
    }
    _ => Task::none(),
  }
}

fn calendar_attention_tick(app: &App) -> Task<Message> {
  match app.runtime.as_ref() {
    Some(runtime) => {
      let db = runtime.db.clone();
      let now = app.now.to_rfc3339();
      Task::perform(
        async move { store::repo::calendar::attention_count(&db, &now).await.unwrap_or(0) },
        Message::CalendarAttentionCounted,
      )
    }
    None => Task::none(),
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

fn navigate_to_corporation_detail(app: &mut App, id: i64) -> Task<Message> {
  navigate(app, Route::CorporationDetail(id));
  app.corporation_detail = Some(corporation_detail::State::new(id));
  match app.runtime.as_ref() {
    Some(runtime) => corporation_detail::load(&runtime.db, id).map(Message::CorporationDetail),
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

fn drain_assets_dirty(app: &mut App) -> Option<Task<Message>> {
  if !app.assets_dirty {
    return None;
  }
  app.assets_dirty = false;
  let runtime = app.runtime.as_ref()?;
  let assets = app.assets.as_ref()?;
  Some(assets::sync_reload(assets, &runtime.db).map(Message::Assets))
}

fn drain_detail_dirty(app: &mut App) -> Option<Task<Message>> {
  if app.detail_dirty.is_empty() {
    return None;
  }
  // Take before the guards: if runtime or detail screen is absent there is nothing to reload,
  // so discarding the set here is intentional rather than deferring to the next pulse.
  let data_types = std::mem::take(&mut app.detail_dirty);
  let runtime = app.runtime.as_ref()?;
  let active = app.character_detail.as_ref()?.active();
  let tasks = data_types
    .into_iter()
    .map(|data_type| character_detail::reload(&runtime.db, active, data_type).map(Message::CharacterDetail));
  Some(Task::batch(tasks))
}

fn drain_roster_dirty(app: &mut App) -> Option<Task<Message>> {
  if !app.roster_dirty || app.character_manager.is_none() {
    return None;
  }
  app.roster_dirty = false;
  let runtime = app.runtime.as_ref()?;
  Some(character_manager::load(&runtime.db, enabled_features(app)).map(Message::CharacterManager))
}

fn drain_wallet_dirty(app: &mut App) -> Option<Task<Message>> {
  if !app.wallet_dirty {
    return None;
  }
  app.wallet_dirty = false;
  let runtime = app.runtime.as_ref()?;
  app.wallet.as_ref()?;
  Some(wallet::load(&runtime.db).map(Message::Wallet))
}

fn wallet_reload_kind(kind: JobKind) -> bool {
  matches!(
    kind,
    JobKind::CharacterWallet | JobKind::CorporationWallet | JobKind::MarketPrices | JobKind::NetWorthSnapshot
  )
}

fn collect_stale_images(app: &App) -> Vec<(store::images::ImageKind, i64)> {
  let mut keys = match app.route {
    Route::Assets => app.assets.as_ref().map(assets::State::stale_images).unwrap_or_default(),
    Route::Calendar => app
      .calendar
      .as_ref()
      .map(calendar::State::stale_images)
      .unwrap_or_default(),
    Route::CharacterDetail(_) => app
      .character_detail
      .as_ref()
      .map(character_detail::State::stale_images)
      .unwrap_or_default(),
    Route::Characters => app
      .character_manager
      .as_ref()
      .map(character_manager::State::stale_images)
      .unwrap_or_default(),
    Route::CorporationDetail(_) => app
      .corporation_detail
      .as_ref()
      .map(corporation_detail::State::stale_images)
      .unwrap_or_default(),
    Route::Industry => app
      .industry
      .as_ref()
      .map(industry::State::stale_images)
      .unwrap_or_default(),
    Route::Mail => app.mail.as_ref().map(mail::State::stale_images).unwrap_or_default(),
    Route::Settings => Vec::new(),
    Route::Skills(_) => app.skills.as_ref().map(skills::State::stale_images).unwrap_or_default(),
    Route::Wallet => app.wallet.as_ref().map(wallet::State::stale_images).unwrap_or_default(),
  };
  if let Some((_, compare)) = app.compare.as_ref() {
    keys.extend(compare.stale_images());
  }
  keys
}

fn dispatch_image_fetches(app: &mut App, keys: Vec<(store::images::ImageKind, i64)>) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let mut tasks = Vec::new();
  for (kind, id) in keys {
    if !app.pending_images.insert((kind, id)) {
      continue;
    }
    let client = Arc::clone(&runtime.eve_image);
    tasks.push(Task::perform(
      async move { ensure_image(client, kind, id).await },
      move |ready| Message::ImageReady {
        id,
        kind,
        ready,
      },
    ));
  }
  Task::batch(tasks)
}

async fn ensure_image(client: Arc<eve_image::Client>, kind: store::images::ImageKind, id: i64) -> bool {
  use store::images;

  let store = images::default_store();
  let path = store.image_path(kind, id);
  if images::is_fresh(&path, images::STALE_AFTER) {
    return true;
  }

  match client.fetch(&image_url(&client, kind, id)).await {
    Ok(bytes) => write_refetched(&store, &path, &bytes, kind, id),
    Err(error) => {
      tracing::warn!(target: "pod::images", %error, ?kind, id, "refetching an evicted image failed");
      false
    }
  }
}

fn image_url(client: &eve_image::Client, kind: store::images::ImageKind, id: i64) -> String {
  use store::images::{self, ImageKind};

  match kind {
    ImageKind::AllianceLogo => client.alliance_logo_url(id, images::LOGO_SIZE),
    ImageKind::CharacterPortrait => client.character_portrait_url(id, images::PORTRAIT_SIZE),
    ImageKind::CorporationLogo => client.corporation_logo_url(id, images::LOGO_SIZE),
  }
}

fn write_refetched(
  store: &store::images::Store,
  path: &std::path::Path,
  bytes: &[u8],
  kind: store::images::ImageKind,
  id: i64,
) -> bool {
  match store.write(path, bytes) {
    Ok(()) => true,
    Err(error) => {
      tracing::warn!(target: "pod::images", %error, ?kind, id, "writing a refetched image failed");
      false
    }
  }
}

fn handle_image_ready(app: &mut App, kind: store::images::ImageKind, id: i64, ready: bool) -> Task<Message> {
  app.pending_images.remove(&(kind, id));
  if ready { image_reload(app) } else { Task::none() }
}

fn image_reload(app: &App) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let mut tasks = Vec::new();
  match app.route {
    Route::Assets => {
      if app.assets.is_some() {
        tasks.push(assets::load(&runtime.db).map(Message::Assets));
      }
    }
    Route::Calendar => {
      if let Some(state) = app.calendar.as_ref() {
        tasks.push(calendar::reload(&runtime.db, state.active(), *runtime.settings.features()).map(Message::Calendar));
      }
    }
    Route::CharacterDetail(_) => {
      if let Some(detail) = app.character_detail.as_ref() {
        let owned = owned_pilot_ids(app);
        tasks.push(character_detail::load(&runtime.db, detail.active(), owned).map(Message::CharacterDetail));
      }
    }
    Route::Characters => {
      if app.character_manager.is_some() {
        tasks.push(character_manager::load(&runtime.db, enabled_features(app)).map(Message::CharacterManager));
      }
    }
    Route::CorporationDetail(_) => {
      if let Some(detail) = app.corporation_detail.as_ref() {
        tasks.push(corporation_detail::load(&runtime.db, detail.active()).map(Message::CorporationDetail));
      }
    }
    Route::Industry => {
      if let Some(state) = app.industry.as_ref() {
        tasks.push(industry::reload(&runtime.db, state.active(), &industry_required_scopes()).map(Message::Industry));
      }
    }
    Route::Mail => {
      if let Some(state) = app.mail.as_ref() {
        let mail::Scope::Character(id) = state.active();
        tasks.push(mail::load(&runtime.db, id).map(Message::Mail));
      }
    }
    Route::Settings => {}
    Route::Skills(_) => {
      if let Some(skills) = app.skills.as_ref() {
        let owned = owned_pilot_ids(app);
        tasks.push(skills::load(&runtime.db, skills.active(), owned).map(Message::Skills));
      }
    }
    Route::Wallet => {
      if app.wallet.is_some() {
        tasks.push(wallet::load(&runtime.db).map(Message::Wallet));
      }
    }
  }
  if let Some((_, compare)) = app.compare.as_ref() {
    tasks.push(skills_compare::load(&runtime.db, compare.selected_ids().to_vec()).map(Message::Compare));
  }
  Task::batch(tasks)
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
  let enabled_features = enabled_features(app);
  let body = Row::with_children(vec![
    rail(
      app.route.destination(),
      mail_unread,
      app.calendar_attention,
      &enabled_features,
      Message::Nav,
    ),
    content.into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  let mut column_children: Vec<Element<'_, Message>> = Vec::with_capacity(4);
  if let Some(banner) = updater_banner::banner(&app.updater_state, Message::UpdaterAction) {
    column_children.push(banner);
  }
  if let Some(holder) = &app.read_only {
    column_children.push(read_only_banner(holder, app.confirm_force_takeover, app.now));
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

fn read_only_banner(holder: &HolderInfo, confirming: bool, now: DateTime<Utc>) -> Element<'static, Message> {
  let (message, actions): (String, Element<'static, Message>) = if confirming {
    let last_active = status::format_since((now - holder.last_active).num_seconds().max(0) as u64);
    let confirm = button(
      text("Take over anyway")
        .font(typography::body::MEDIUM)
        .size(typography::size::SM),
    )
    .padding(control::padding())
    .on_press(Message::ConfirmTakeOver)
    .style(control::danger_button);
    let cancel = button(text("Cancel").font(typography::body::MEDIUM).size(typography::size::SM))
      .padding(control::padding())
      .on_press(Message::CancelTakeOver)
      .style(control::ghost_button);
    (
      read_only_confirm_label(&holder.hostname, &last_active),
      Row::new()
        .push(cancel)
        .push(confirm)
        .align_y(Vertical::Center)
        .spacing(spacing::SPACE_2)
        .into(),
    )
  } else {
    let action = button(
      text("Take over")
        .font(typography::body::MEDIUM)
        .size(typography::size::SM),
    )
    .padding(control::padding())
    .on_press(Message::TakeOver)
    .style(control::primary_button);
    (read_only_banner_label(&holder.hostname), action.into())
  };

  let label = text(message)
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::status::WARNING),
    });

  let row = Row::new()
    .push(container(label).width(Length::Fill).align_y(Vertical::Center))
    .push(actions)
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

fn read_only_banner_label(hostname: &str) -> String {
  format!("Open on {hostname} \u{2014} close it there, or take over.")
}

/// Data-loss warning shown before a forceful take-over: surfaces how recently the holder was seen so
/// the user can judge whether it is plausibly dead before clobbering its in-flight work on the share.
fn read_only_confirm_label(hostname: &str, last_active: &str) -> String {
  format!(
    "{hostname} was last active {last_active}. Taking over overwrites any unsaved changes it still has open. Continue?"
  )
}

fn route_view(app: &App) -> Element<'_, Message> {
  match app.route {
    Route::Assets => assets_route_view(app),
    Route::Calendar => calendar_route_view(app),
    Route::CharacterDetail(_) => character_detail_route_view(app),
    Route::Characters => characters_route_view(app),
    Route::CorporationDetail(_) => corporation_detail_route_view(app),
    Route::Industry => industry_route_view(app),
    Route::Mail => mail_route_view(app),
    Route::Settings => settings_route_view(app),
    Route::Skills(id) => skills_route_view(app, id),
    Route::Wallet => wallet_route_view(app),
  }
}

fn starting_up<'a>() -> Element<'a, Message> {
  placeholder("Starting up\u{2026}".to_owned())
}

fn calendar_route_view(app: &App) -> Element<'_, Message> {
  match &app.calendar {
    Some(state) => calendar::view(state, app.now).map(Message::Calendar),
    None => starting_up(),
  }
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

fn corporation_detail_route_view(app: &App) -> Element<'_, Message> {
  match &app.corporation_detail {
    Some(state) => corporation_detail::view(state).map(Message::CorporationDetail),
    None => starting_up(),
  }
}

fn skills_route_view(app: &App, id: i64) -> Element<'_, Message> {
  match &app.skills {
    Some(state) => skills::view(state, id, &app.status, app.now).map(Message::Skills),
    None => starting_up(),
  }
}

fn industry_route_view(app: &App) -> Element<'_, Message> {
  match &app.industry {
    Some(state) => industry::view(state, &industry_required_scopes(), app.now).map(Message::Industry),
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
    color: Some(color::text::secondary()),
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

  let enabled = enabled_features(app);
  let mut rows = Vec::with_capacity(pilots.len() * POPOVER_JOBS.len());
  for pilot in &pilots {
    let subject = Subject::Character(pilot.id);
    for (kind, label) in POPOVER_JOBS {
      // A feature-gated job whose feature is disabled is not syncing, so it must not appear queued;
      // featureless jobs (e.g. Profile) always run and are always shown.
      if kind.feature().is_some_and(|feature| !enabled.contains(&feature)) {
        continue;
      }
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
    Some(Phase::Empty) => (RowState::Empty, Some("No data".to_owned())),
    Some(Phase::NotReady) => (RowState::Attention, Some("Waiting on dependencies".to_owned())),
  }
}

fn expected_job_stats(app: &App) -> JobStats {
  let pilots = app
    .character_manager
    .as_ref()
    .map(character_manager::owned_roster)
    .unwrap_or_default();
  let enabled = enabled_features(app);
  let mut stats = JobStats::default();
  for pilot in &pilots {
    let subject = Subject::Character(pilot.id);
    for (kind, _label) in POPOVER_JOBS {
      if kind.feature().is_some_and(|feature| !enabled.contains(&feature)) {
        continue;
      }
      let (state, _) = row_state(&app.status, &JobKey::new(kind, subject));
      stats.total += 1;
      match state {
        RowState::Attention => stats.attention += 1,
        RowState::Done | RowState::Empty => stats.done += 1,
        RowState::Error => stats.errors += 1,
        RowState::Syncing => stats.active += 1,
        RowState::Queued => {}
      }
    }
  }
  stats
}

fn engine_syncing(app: &App) -> bool {
  syncing_with(&app.engine_state, &expected_job_stats(app))
}

fn syncing_with(engine_state: &EngineState, stats: &JobStats) -> bool {
  !engine_state.settled() && stats.in_progress()
}

fn resolve_skills_target(roster: &[OwnedPilot], last_selected: Option<i64>) -> Option<i64> {
  if let Some(id) = last_selected
    && roster.iter().any(|pilot| pilot.id == id)
  {
    return Some(id);
  }
  roster.first().map(|pilot| pilot.id)
}

fn chip_lifecycle(app: &App) -> sync_chip::Lifecycle {
  match &app.engine_state {
    EngineState::Stopped {
      ..
    } => sync_chip::Lifecycle::Stopped,
    EngineState::ReadOnly {
      held_by,
    } => sync_chip::Lifecycle::ReadOnly {
      hostname: held_by.as_ref().map(|holder| holder.hostname.clone()),
    },
    EngineState::Idle | EngineState::Running => sync_chip::Lifecycle::Active,
  }
}

fn status_affordance(state: &EngineState) -> Option<Element<'static, Message>> {
  let (label, message) = match state {
    EngineState::Stopped {
      ..
    } => ("Restart sync", Message::RestartSync),
    EngineState::ReadOnly {
      ..
    } => ("Take over", Message::TakeOver),
    EngineState::Idle | EngineState::Running => return None,
  };
  let action = button(text(label).font(typography::body::MEDIUM).size(typography::size::XS))
    .padding(control::padding())
    .on_press(message)
    .style(control::primary_button);
  Some(
    container(action)
      .padding(region_padding())
      .height(Length::Fill)
      .align_y(Vertical::Center)
      .into(),
  )
}

fn status_bar_view(app: &App) -> Element<'_, Message> {
  let stats = expected_job_stats(app);
  let percent = (stats.done * 100).checked_div(stats.total).unwrap_or(100) as u8;
  let chip = sync_chip::State {
    syncing: engine_syncing(app),
    done: stats.done,
    total: stats.total,
    percent,
    errors: stats.errors,
    attention: stats.attention,
    last_synced_secs: app.last_synced.map(|at| (app.now - at).num_seconds().max(0) as u64),
    lifecycle: chip_lifecycle(app),
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
  if let Some(affordance) = status_affordance(&app.engine_state) {
    children.push(affordance);
    children.push(separator());
  }
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
        color: Some(color::text::dim()),
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
  if engine_syncing(app) {
    subs.push(iced::time::every(PULSE_INTERVAL).map(|_| Message::SyncPulse));
  }
  if holding_lease(app) {
    subs.push(iced::time::every(store::lease::HEARTBEAT_INTERVAL).map(|_| Message::LeaseHeartbeat));
    subs.push(iced::time::every(PERIODIC_PULL_INTERVAL).map(|_| Message::PeriodicPull));
    subs.push(iced::time::every(PERIODIC_PUSH_INTERVAL).map(|_| Message::PeriodicPush));
  }
  if parked(app) {
    subs.push(iced::time::every(REACQUIRE_INTERVAL).map(|_| Message::ReacquireLease));
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
  #[cfg(target_os = "macos")]
  subs.push(menu::subscription().map(Message::Menu));
  if let Some(state) = &app.assets {
    subs.push(assets::subscription(state).map(Message::Assets));
  }
  if let Some(state) = &app.character_detail {
    subs.push(character_detail::subscription(state).map(Message::CharacterDetail));
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
  if let Some(state) = &app.calendar {
    subs.push(calendar::subscription(state).map(Message::Calendar));
  }
  if let Some(state) = &app.industry {
    subs.push(industry::subscription(state).map(Message::Industry));
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

  let size = if validity::is_size_in_range(&geometry) {
    Size::new(
      geometry.width.max(spacing::layout::MIN_WINDOW_WIDTH),
      geometry.height.max(spacing::layout::MIN_WINDOW_HEIGHT),
    )
  } else {
    default
  };
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
    icon: app_icon(),
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
    icon: app_icon(),
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
    icon: app_icon(),
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

#[cfg(target_os = "macos")]
fn handle_menu(app: &mut App, action: menu::MenuAction) -> Task<Message> {
  match action {
    menu::MenuAction::About => Task::done(Message::OpenAbout),
    menu::MenuAction::CheckUpdates => {
      if let Some(handle) = &app.updater {
        handle.check();
      }
      Task::none()
    }
    menu::MenuAction::Quit => shutdown(app),
  }
}

fn open_about_window(app: &mut App) -> Task<Message> {
  if app.about.is_some() {
    return Task::none();
  }

  let settings = window::Settings {
    size: Size::new(ABOUT_WINDOW_WIDTH, ABOUT_WINDOW_HEIGHT),
    position: window::Position::Centered,
    resizable: false,
    icon: app_icon(),
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
  let is_feature = message.feature_variant_name().is_some();
  let task = match dispatch_feature(app, message) {
    Ok(task) => task,
    Err(message) => dispatch_lifecycle(app, *message),
  };
  if !is_feature {
    return task;
  }
  let stale = collect_stale_images(app);
  if stale.is_empty() {
    return task;
  }
  Task::batch([task, dispatch_image_fetches(app, stale)])
}

fn dispatch_feature(app: &mut App, message: Message) -> Result<Task<Message>, Box<Message>> {
  Ok(match message {
    Message::About(msg) => handle_about(app, msg),
    Message::Assets(msg) => handle_assets(app, msg),
    Message::Auth(msg) => handle_auth(app, msg),
    Message::Calendar(msg) => handle_calendar(app, msg),
    Message::CalendarAttentionCounted(count) => handle_calendar_attention_counted(app, count),
    Message::CharacterDetail(msg) => handle_character_detail(app, msg),
    Message::CharacterManager(msg) => handle_character_manager(app, msg),
    Message::Compare(msg) => handle_compare(app, msg),
    Message::CorporationDetail(msg) => handle_corporation_detail(app, msg),
    Message::Industry(msg) => handle_industry(app, msg),
    Message::Mail(msg) => handle_mail(app, msg),
    Message::MailUnreadCounted(unread) => handle_mail_unread_counted(app, unread),
    #[cfg(target_os = "macos")]
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
    Message::ImageReady {
      id,
      kind,
      ready,
    } => handle_image_ready(app, kind, id, ready),
    Message::InitFailed(error) => handle_init_failed(app, error),
    Message::Ready(runtime) => handle_ready(app, runtime),
    Message::ReauthCharacter(character_id) => handle_reauth_character(app, character_id),
    Message::SeedProgress(progress) => on_seed_progress(app, progress),
    Message::SnoozesWoken(woken) => handle_snoozes_woken(app, woken),
    Message::Splash(msg) => update_splash(app, msg),
    Message::StorageMigrated => Task::none(),
    Message::StoreOpened(ready) => handle_store_opened(app, *ready),
    other => dispatch_sync_lifecycle(app, other),
  }
}

fn dispatch_sync_lifecycle(app: &mut App, message: Message) -> Task<Message> {
  match message {
    Message::EngineStopped {
      reason,
    } => handle_engine_stopped(app, reason),
    Message::LeaseHeartbeat => handle_lease_heartbeat(app),
    Message::LockReleased => handle_lock_released(app),
    Message::PeriodicPull => handle_periodic_pull(app),
    Message::PeriodicPush => handle_periodic_push(app),
    Message::SyncNowResolved(outcome) => handle_sync_now_resolved(app, outcome),
    Message::Pulled(pulled) => handle_pulled(app, pulled),
    Message::Pushed(mark) => handle_pushed(app, mark),
    Message::ReacquireLease => handle_reacquire_lease(app),
    Message::RestartSync => handle_restart_sync(app),
    Message::SyncPulse => handle_sync_pulse(app),
    Message::CancelTakeOver => handle_cancel_take_over(app),
    Message::ConfirmTakeOver => handle_confirm_take_over(app),
    Message::TakeOver => handle_take_over(app),
    Message::TakeOverResolved(outcome) => handle_take_over_resolved(app, outcome),
    other => dispatch_window_lifecycle(app, other),
  }
}

fn dispatch_window_lifecycle(app: &mut App, message: Message) -> Task<Message> {
  match message {
    Message::CloseSyncPopover => set_sync_popover_open(app, false),
    Message::FocusMainWindow => handle_focus_main_window(app),
    Message::OpenAbout => open_about_window(app),
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
  if let assets::Message::PaneSettled(key, ratio) = msg {
    record_pane_ratio(app, key, ratio);
    return Task::none();
  }
  if let assets::Message::ReauthRequested(id) = msg {
    return update(app, Message::ReauthCharacter(id));
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
  let features = enabled_features(app);
  tracing::info!(
    character_id,
    scopes = ?auth::scopes_for(&features),
    "re-authorizing character via SSO sign-in"
  );
  update(app, Message::Auth(auth::Message::Start(features)))
}

fn reauth_corporation(app: &mut App, corporation_id: i64) -> Task<Message> {
  let features = enabled_features(app);
  tracing::info!(
    corporation_id,
    scopes = ?auth::corp_scopes_for(&features),
    "re-authorizing corporation via SSO sign-in"
  );
  update(app, Message::Auth(auth::Message::StartAddCorporation(features)))
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
  let mut task = settings_task.map(Message::Settings);

  if let Some(request) = state.take_storage_migration() {
    let next = state.settings().storage().clone();
    task = Task::batch(vec![task, migrate_storage(request.previous, next)]);
  }

  // Keep the long-lived runtime settings' industry defaults in lock-step with the settings screen, so
  // the planner (which reads `runtime.settings` when it opens) honors a freshly-changed default without
  // waiting for a restart. Feature/accessibility sections sync via their own paths below.
  if let Some(runtime) = app.runtime.as_mut() {
    let industry = *state.settings().industry();
    *runtime.settings.industry_mut() = industry;
  }

  match outcome {
    settings::Outcome::AccessibilityChanged => {
      let accessibility = *state.settings().accessibility();
      app.accessibility = accessibility;
      if let Some(runtime) = app.runtime.as_mut() {
        *runtime.settings.accessibility_mut() = accessibility;
      }
      color::set_high_contrast(*accessibility.high_contrast());
      return Task::batch(vec![task, refresh_all_windows(app)]);
    }
    settings::Outcome::SyncNow => return Task::batch(vec![task, sync_now(app)]),
    settings::Outcome::ReleaseLock => return Task::batch(vec![task, release_lock(app)]),
    settings::Outcome::ExportLogs {
      start,
      end,
    } => {
      let storage = state.settings().storage();
      let diagnostics = settings::log_export::Diagnostics {
        cache_dir: storage.resolved_cache_dir(),
        database_path: storage.resolved_database_path(),
        db_dir: storage.resolved_db_dir(),
        log_dir: storage.resolved_log_dir(),
      };
      let log_dir = storage.resolved_log_dir();
      return Task::batch(vec![task, export_logs(log_dir, start, end, diagnostics)]);
    }
    settings::Outcome::SetLogLevel(level) => {
      apply_log_level(level);
      return task;
    }
    settings::Outcome::IndustrySearch {
      activity,
      generation,
      query,
    } => return Task::batch(vec![task, settings_facility_search(app, activity, generation, query)]),
    settings::Outcome::IndustryPin(pin) => return Task::batch(vec![task, settings_facility_pin(app, pin)]),
    _ => {}
  }

  if !features_changed {
    return task;
  }
  let updated = state.settings().clone();
  propagate_feature_change(app, updated, task)
}

/// Pushes a just-changed feature set out to the runtime sync engine and every open feature screen
/// (calendar, industry, character detail), reloading the active one and falling back to Characters
/// when the current route's feature was disabled out from under it. A no-op without a runtime.
fn propagate_feature_change(app: &mut App, updated: crate::config::Settings, base: Task<Message>) -> Task<Message> {
  let Some(runtime) = app.runtime.as_mut() else {
    return base;
  };
  runtime.settings = updated;
  let enabled = runtime.settings.features().enabled();
  let flags = *runtime.settings.features();
  let db = runtime.db.clone();
  runtime.sync.set_features(flags);
  let mut tasks = vec![
    base,
    character_manager::load(&db, enabled.clone()).map(Message::CharacterManager),
  ];

  let route = app.route;
  if let Some(state) = app.calendar.as_mut() {
    state.set_features(flags);
    if route == Route::Calendar {
      tasks.push(calendar::reload(&db, state.active(), flags).map(Message::Calendar));
    }
  }

  if let Some(state) = app.industry.as_mut() {
    state.set_required_scopes(industry_required_scopes());
    if route == Route::Industry {
      tasks.push(industry::reload(&db, state.active(), &industry_required_scopes()).map(Message::Industry));
    }
  }

  if let Some(detail) = app.character_detail.as_mut() {
    detail.sync_features(&enabled);
  }

  // A feature disabled while its screen is open leaves the route stranded with its rail icon gone;
  // fall back to Characters so the now-unreachable route can't linger.
  if registry::feature_for_destination(route.destination()).is_some_and(|feature| !enabled.contains(&feature)) {
    navigate(app, Route::Characters);
  }

  Task::batch(tasks)
}

/// Runs the Settings Industry tab's facility search through the same live ESI search the planner uses.
/// Mirrors the planner's debounce + min-char gate, stamping `generation` so the tab can drop stale
/// responses, and degrades to a no-op when no runtime (and therefore no authenticated character) exists.
fn settings_facility_search(app: &App, activity: i64, generation: u64, query: String) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  if query.trim().chars().count() < industry::FACILITY_SEARCH_MIN_CHARS {
    return Task::none();
  }

  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let sso = Arc::clone(&runtime.sso);
  Task::perform(
    async move {
      tokio::time::sleep(Duration::from_millis(industry::FACILITY_SEARCH_DEBOUNCE_MS)).await;
      industry::search_facilities(db, esi, sso, query).await
    },
    move |results| {
      Message::Settings(settings::Message::Industry(
        settings::industry_tab::Message::SearchResults {
          activity,
          generation,
          results,
        },
      ))
    },
  )
}

/// Persists a player structure picked as a Settings default so it survives in the locally known
/// facility set, exactly as the planner pins picked structures.
fn settings_facility_pin(app: &App, pin: industry::PinnedStructure) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  // The tab already shows the picked facility from the user's selection, so the pin only persists; an
  // empty `SelectionsResolved` is a benign no-op completion message.
  Task::perform(async move { industry::pin_facility(db, pin).await }, |()| {
    Message::Settings(settings::Message::Industry(
      settings::industry_tab::Message::SelectionsResolved(Vec::new()),
    ))
  })
}

// The resolved color functions are read inside each window's `view` closure, which only re-runs
// when that window redraws. Unlike `scale_factor`, iced does not re-read them every frame, so after
// the high-contrast flag flips we issue a benign per-window action (querying size and discarding it)
// to schedule a fresh draw of every open window, applying the new palette live without a restart.
fn refresh_all_windows(app: &App) -> Task<Message> {
  Task::batch(app.windows.ids().map(|id| window::size(id).discard()))
}

/// Routes the storage tab's "Sync now" action, always reporting an outcome rather than silently
/// no-opping: read-only sessions delegate to take-over, otherwise it pushes when dirty and pulls
/// when the share has advanced.
fn sync_now(app: &mut App) -> Task<Message> {
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  if app.read_only.is_some() {
    return handle_take_over(app);
  }
  let dirty = session.is_dirty_since(app.last_push);
  let advanced = session.share_advanced();
  if !dirty && !advanced {
    app.last_synced = Some(Utc::now());
    refresh_storage_status(app);
    return Task::none();
  }
  let mark = session.last_write();
  Task::future(async move {
    if dirty && let Err(error) = session.checkpoint_and_push().await {
      tracing::warn!(target: "pod::lifecycle", %error, "sync now: push failed");
      return Message::SyncNowResolved(SyncNowOutcome::Failed);
    }
    let pulled = if advanced {
      matches!(tokio::task::spawn_blocking(move || session.pull()).await, Ok(Ok(true)))
    } else {
      false
    };
    Message::SyncNowResolved(SyncNowOutcome::Reconciled {
      mark: if dirty { mark } else { None },
      pulled,
    })
  })
}

fn handle_sync_now_resolved(app: &mut App, outcome: SyncNowOutcome) -> Task<Message> {
  match outcome {
    SyncNowOutcome::Reconciled {
      mark,
      pulled,
    } => {
      if let Some(mark) = mark {
        app.last_push = Some(mark);
      }
      app.last_synced = Some(Utc::now());
      if pulled {
        app.roster_dirty = true;
      }
      refresh_storage_status(app);
      Task::none()
    }
    SyncNowOutcome::Failed => Task::none(),
  }
}

/// Routes the storage tab's "Export logs" action, building the diagnostics zip on a blocking thread
/// so the UI stays responsive. Log files (including the live current-day file) are read read-only,
/// never truncated.
fn export_logs(
  log_dir: std::path::PathBuf,
  start: DateTime<Utc>,
  end: DateTime<Utc>,
  diagnostics: settings::log_export::Diagnostics,
) -> Task<Message> {
  Task::perform(export_log_bundle(log_dir, start, end, diagnostics), |result| {
    Message::Settings(settings::Message::Storage(
      settings::storage_tab::Message::ExportFinished(result),
    ))
  })
}

async fn export_log_bundle(
  log_dir: std::path::PathBuf,
  start: DateTime<Utc>,
  end: DateTime<Utc>,
  diagnostics: settings::log_export::Diagnostics,
) -> Result<Option<std::path::PathBuf>, String> {
  let default_name = settings::log_export::default_file_name(start, end);
  let bytes = tokio::task::spawn_blocking(move || settings::log_export::build_zip(&log_dir, start, end, &diagnostics))
    .await
    .map_err(|err| err.to_string())??;
  save_log_bundle(default_name, bytes).await
}

/// Prompts for a save location via the native dialog and writes the zip there. Stubbed to a no-op
/// under `cfg(test)` so tests never open a real file dialog.
async fn save_log_bundle(default_name: String, bytes: Vec<u8>) -> Result<Option<std::path::PathBuf>, String> {
  #[cfg(not(test))]
  {
    let Some(handle) = rfd::AsyncFileDialog::new()
      .set_title("Export logs")
      .set_file_name(default_name)
      .add_filter("Zip archive", &["zip"])
      .save_file()
      .await
    else {
      return Ok(None);
    };
    std::fs::write(handle.path(), bytes).map_err(|err| err.to_string())?;
    Ok(Some(handle.path().to_path_buf()))
  }
  #[cfg(test)]
  {
    let _ = (default_name, bytes);
    Ok(None)
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

/// Migrates the on-disk database layout after the storage configuration crossed (or could have
/// crossed) the Direct/Sync boundary. Driven async because a Sync→Direct consolidation must run a
/// WAL checkpoint; the change takes effect on next launch, when bootstrap resolves the new layout.
fn migrate_storage(previous: config::StorageConfig, next: config::StorageConfig) -> Task<Message> {
  let old_mode = previous.storage_mode();
  let new_mode = next.storage_mode();
  Task::future(async move {
    match store::storage_migration::migrate(&previous, &next, old_mode, new_mode).await {
      Ok(()) => tracing::info!(
        target: "pod::lifecycle",
        ?old_mode,
        ?new_mode,
        "migrated the database layout for the new storage location"
      ),
      Err(error) => tracing::warn!(
        target: "pod::lifecycle",
        %error,
        "storage layout migration failed; the previous layout is left intact"
      ),
    }
    Message::StorageMigrated
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
  app.engine_state = read_only_engine_state(app.read_only.clone());
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
  let mut tasks: Vec<Task<Message>> = Vec::new();
  if let Some(reload) = drain_roster_dirty(app) {
    tasks.push(reload);
  }
  if let Some(reload) = drain_assets_dirty(app) {
    tasks.push(reload);
  }
  if let Some(reload) = drain_wallet_dirty(app) {
    tasks.push(reload);
  }
  if let Some(reload) = drain_detail_dirty(app) {
    tasks.push(reload);
  }
  Task::batch(tasks)
}

fn holding_lease(app: &App) -> bool {
  app.sync_session.is_some() && app.read_only.is_none()
}

/// Symmetric inverse of `holding_lease`: exactly one of the two is true whenever a sync session is
/// active. The `sync_session` guard ensures this returns `false` outside of networked-share mode.
fn parked(app: &App) -> bool {
  app.sync_session.is_some() && app.read_only.is_some()
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

fn handle_periodic_pull(app: &mut App) -> Task<Message> {
  if !holding_lease(app) {
    return Task::none();
  }
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  if session.is_dirty_since(app.last_push) {
    tracing::trace!(target: "pod::lifecycle", "periodic pull skipped; a local write is in flight");
    return Task::none();
  }
  if !session.share_advanced() {
    return Task::none();
  }
  pull_task(session)
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

fn handle_pulled(app: &mut App, pulled: bool) -> Task<Message> {
  if pulled {
    app.last_synced = Some(Utc::now());
    app.roster_dirty = true;
  }
  refresh_storage_status(app);
  Task::none()
}

fn pull_task(session: store::sync_session::SyncSession) -> Task<Message> {
  Task::future(pull_bundle(session))
}

async fn pull_bundle(session: store::sync_session::SyncSession) -> Message {
  match tokio::task::spawn_blocking(move || session.pull()).await {
    Ok(Ok(pulled)) => {
      if pulled {
        tracing::info!(target: "pod::lifecycle", "pulled newer changes from the share");
      }
      Message::Pulled(pulled)
    }
    Ok(Err(error)) => {
      tracing::warn!(target: "pod::lifecycle", %error, "pull from the share failed");
      Message::Pulled(false)
    }
    Err(error) => {
      tracing::warn!(target: "pod::lifecycle", %error, "pull task panicked");
      Message::Pulled(false)
    }
  }
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

/// Polls for lease re-acquisition on each `REACQUIRE_INTERVAL` tick while this instance is parked.
/// This automatic path stays stale-aware: a `HeldBy` response (foreign holder still fresh) maps to
/// `TakeOverOutcome::Failed` — a silent no-op in the resolver — so only the explicit user-confirmed
/// take-over ever overrides lease freshness.
fn handle_reacquire_lease(app: &mut App) -> Task<Message> {
  if !parked(app) {
    return Task::none();
  }
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  let released_by = app.read_only.as_ref().map(|holder| holder.hostname.clone());
  Task::future(async move {
    let outcome = match session.take_over(Utc::now()) {
      Ok(store::lease::Outcome::Acquired) => {
        let hostname = released_by.as_deref().unwrap_or("an unknown host");
        tracing::info!(target: "pod::lifecycle", %hostname, "auto-promoted to read-write after the share was released");
        TakeOverOutcome::Claimed
      }
      Ok(store::lease::Outcome::HeldBy {
        hostname, ..
      }) => {
        tracing::trace!(target: "pod::lifecycle", %hostname, "auto re-acquire declined; the share is still held");
        TakeOverOutcome::Failed
      }
      Err(error) => {
        tracing::warn!(target: "pod::lifecycle", %error, "auto re-acquiring the storage lease failed");
        TakeOverOutcome::Failed
      }
    };
    Message::TakeOverResolved(outcome)
  })
}

fn handle_cancel_take_over(app: &mut App) -> Task<Message> {
  app.confirm_force_takeover = false;
  Task::none()
}

/// Performs the explicit, user-confirmed forceful take-over. Unlike the stale-aware automatic
/// re-acquire, this displaces even a still-live foreign holder — the confirmation gate is the only
/// path that overrides lease freshness — so the share is clobbered unconditionally on success.
fn handle_confirm_take_over(app: &mut App) -> Task<Message> {
  app.confirm_force_takeover = false;
  if app.read_only.is_none() {
    return Task::none();
  }
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  let displaced = app.read_only.as_ref().map(|holder| holder.hostname.clone());
  Task::future(async move {
    let hostname = displaced.as_deref().unwrap_or("an unknown host");
    let outcome = match session.force_take_over(Utc::now()) {
      Ok(_) => {
        tracing::info!(target: "pod::lifecycle", %hostname, "force-took over the storage lease");
        TakeOverOutcome::Claimed
      }
      Err(error) => {
        tracing::warn!(target: "pod::lifecycle", %hostname, %error, "force-taking over the storage lease failed");
        TakeOverOutcome::Failed
      }
    };
    Message::TakeOverResolved(outcome)
  })
}

/// Opens the data-loss confirmation gate rather than claiming immediately. The forceful claim is
/// deferred to [`handle_confirm_take_over`] so the user first sees the holder's last-active age and
/// the clobber warning; a still-live writer is never displaced on a single accidental click.
fn handle_take_over(app: &mut App) -> Task<Message> {
  if app.read_only.is_none() || app.sync_session.is_none() {
    return Task::none();
  }
  app.confirm_force_takeover = true;
  Task::none()
}

/// Applies a resolved take-over. A claim rebuilds the runtime from the parked `store_ready`: the
/// database must reopen so the connection picks up the freshly pulled working copy (swapping the
/// file under a live connection would corrupt it) and the real sync engine replaces the inert one.
fn handle_take_over_resolved(app: &mut App, outcome: TakeOverOutcome) -> Task<Message> {
  app.confirm_force_takeover = false;
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
      app.engine_state = EngineState::Running;
      build_runtime(ready)
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
    wallet::Message::PaneSettled(key, ratio) => {
      record_pane_ratio(app, key, ratio);
      Task::none()
    }
    wallet::Message::ReauthRequested(id) => update(app, Message::ReauthCharacter(id)),
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
    Some(auth::Event::SignedIn(signed)) => {
      if let Some(state) = app.character_manager.as_mut() {
        character_manager::insert_signed_in_card(state, signed.character_id, signed.character_name);
      }
      Some(sync::Subject::Character(signed.character_id))
    }
    None => None,
  };
  if let Some(subject) = enrolled {
    // The session handler cleared any persisted needs-reauth flag, so re-enrolling now picks up the
    // full granted job set instead of just the public ones; run_now makes those revived jobs due so
    // a re-authorized entity's parked sync resumes promptly without an app restart.
    runtime.sync.enroll(subject);
    runtime.sync.run_now(subject);
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
    character_manager::Message::AddCorporationRequested => update(
      app,
      Message::Auth(auth::Message::StartAddCorporation(enabled_features(app))),
    ),
    character_manager::Message::CharacterSelected(id) => navigate_to_character_detail(app, id),
    character_manager::Message::CorporationSelected(id) => navigate_to_corporation_detail(app, id),
    character_manager::Message::TrainingSkillClicked(character_id) => {
      let owned = owned_pilot_ids(app);
      navigate_to_skills(app, Some(character_id), owned)
    }
    character_manager::Message::ReauthCharacterRequested(character_id) => {
      update(app, Message::ReauthCharacter(character_id))
    }
    character_manager::Message::ReauthCorporationRequested(corporation_id) => reauth_corporation(app, corporation_id),
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

fn handle_calendar(app: &mut App, msg: calendar::Message) -> Task<Message> {
  if let calendar::Message::ReauthRequested(id) = msg {
    return update(app, Message::ReauthCharacter(id));
  }

  let (Some(state), Some(runtime)) = (app.calendar.as_mut(), app.runtime.as_ref()) else {
    return Task::none();
  };

  calendar::update(state, msg, &runtime.db, app.now).map(Message::Calendar)
}

fn handle_calendar_attention_counted(app: &mut App, count: i64) -> Task<Message> {
  app.calendar_attention = count;
  Task::none()
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
  if let character_detail::Message::ContactEntityInput(query) = &msg {
    let query = query.clone();
    return match (app.character_detail.as_mut(), app.runtime.as_ref()) {
      (Some(state), Some(runtime)) => {
        let update = character_detail::update(state, msg, &runtime.db).map(Message::CharacterDetail);
        Task::batch([update, contact_entity_search(state, runtime, query)])
      }
      _ => Task::none(),
    };
  }
  match (app.character_detail.as_mut(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => character_detail::update(state, msg, &runtime.db).map(Message::CharacterDetail),
    _ => Task::none(),
  }
}

fn handle_corporation_detail(app: &mut App, msg: corporation_detail::Message) -> Task<Message> {
  match (app.corporation_detail.as_mut(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => corporation_detail::update(state, msg, &runtime.db).map(Message::CorporationDetail),
    _ => Task::none(),
  }
}

/// Captures the modal's current search generation and stamps it onto the async result so a stale response
/// arriving after the user has typed again is discarded by the handler rather than clobbering newer results.
fn contact_entity_search(state: &character_detail::State, runtime: &Runtime, query: String) -> Task<Message> {
  use crate::{features::entity_search, ui::components::entity_search as picker};

  let generation = state.contact_search_generation();
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let eve_image = Arc::clone(&runtime.eve_image);
  let sso = Arc::clone(&runtime.sso);
  let categories = vec![
    entity_search::EntityCategory::Character,
    entity_search::EntityCategory::Corporation,
    entity_search::EntityCategory::Alliance,
  ];
  Task::perform(
    async move { entity_search::search_entities(db, esi, eve_image, sso, categories, query).await },
    move |results| {
      let results = results
        .into_iter()
        .map(|result| picker::EntityRef {
          id: result.id,
          kind: match result.category {
            entity_search::EntityCategory::Alliance => picker::EntityKind::Alliance,
            entity_search::EntityCategory::Character => picker::EntityKind::Character,
            entity_search::EntityCategory::Corporation => picker::EntityKind::Corporation,
          },
          name: result.name,
          portrait: Some(store::images::default_store().image_path(result.category.image_kind(), result.id)),
        })
        .collect();
      Message::CharacterDetail(character_detail::Message::ContactEntityResults {
        generation,
        results,
      })
    },
  )
}

fn compare_seed_ids(app: &App) -> Vec<i64> {
  let Some(manager) = app.character_manager.as_ref() else {
    return Vec::new();
  };

  let by_sp: Vec<(i64, i64)> = character_manager::groups(manager)
    .iter()
    .flat_map(|group| group.cards.iter())
    .chain(character_manager::unassigned(manager).iter())
    .map(|card| (card.character_id, card.total_sp.unwrap_or(0)))
    .collect();
  let active = app.skills.as_ref().map(skills::State::active);

  compare_seeds(by_sp, active)
}

fn compare_seeds(mut by_sp: Vec<(i64, i64)>, active: Option<i64>) -> Vec<i64> {
  by_sp.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

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
  Task::batch([
    snooze_wake_tick(app),
    mail_unread_tick(app),
    mail_clock_reload(app),
    calendar_attention_tick(app),
    calendar_clock_reload(app),
    industry_clock_reload(app),
  ])
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
  let enabled = enabled_features(app);
  let is_feature_disabled = |dest: rail::Destination| {
    registry::feature_for_destination(dest).is_some_and(|feature| !enabled.contains(&feature))
  };

  if is_feature_disabled(destination) {
    navigate(app, Route::Characters);
    return Task::none();
  }

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
    rail::Destination::Mail => {
      let roster = app
        .character_manager
        .as_ref()
        .map(character_manager::owned_roster)
        .unwrap_or_default();
      let target = resolve_mail_target(&roster, app.selected_character);
      navigate_to_mail(app, target)
    }
    rail::Destination::Calendar => navigate_to_calendar(app, None),
    rail::Destination::Industry => navigate_to_industry(app, None),
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
  app.engine_state = if app.read_only.is_some() {
    read_only_engine_state(app.read_only.clone())
  } else {
    EngineState::Running
  };
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
    skills::Message::PaneSettled(key, ratio) => {
      record_pane_ratio(app, key, ratio);
      Task::none()
    }
    msg => match (app.skills.as_mut(), app.runtime.as_ref()) {
      (Some(state), Some(runtime)) => skills::update(state, msg, &runtime.db).map(Message::Skills),
      _ => Task::none(),
    },
  }
}

fn handle_industry(app: &mut App, msg: industry::Message) -> Task<Message> {
  if let industry::Message::PaneSettled(key, ratio) = msg {
    record_pane_ratio(app, key, ratio);
    return Task::none();
  }
  if let industry::Message::ReauthRequested(id) = msg {
    return update(app, Message::ReauthCharacter(id));
  }

  let (Some(state), Some(runtime)) = (app.industry.as_mut(), app.runtime.as_ref()) else {
    return Task::none();
  };

  // The facility picker's live ESI search and structure-pin persistence need the runtime's esi/sso/db,
  // so they are seamed here rather than in the pure planner reducer (mirrors stockpile location search).
  match msg {
    industry::Message::Planner(industry::PlannerMessage::FacilitySearchChanged {
      ref path,
      ref query,
    }) => {
      let (path, query) = (path.clone(), query.clone());
      let update = industry::update(state, msg, &runtime.db, app.now).map(Message::Industry);
      let search = match state.facility_search_target() {
        Some((target, generation)) if target == path => industry::facility_search(
          &runtime.db,
          Arc::clone(&runtime.esi),
          Arc::clone(&runtime.sso),
          path,
          query,
          generation,
        )
        .map(Message::Industry),
        _ => Task::none(),
      };
      Task::batch([update, search])
    }
    industry::Message::Planner(industry::PlannerMessage::FacilitySelected {
      pin: Some(ref pin), ..
    }) => {
      let pin = pin.clone();
      let scope = state.active();
      let update = industry::update(state, msg, &runtime.db, app.now).map(Message::Industry);
      Task::batch([
        update,
        industry::facility_pin(&runtime.db, scope, pin).map(Message::Industry),
      ])
    }
    _ => industry::update(state, msg, &runtime.db, app.now).map(Message::Industry),
  }
}

fn handle_mail(app: &mut App, msg: mail::Message) -> Task<Message> {
  if let mail::Message::PaneSettled(key, ratio) = msg {
    record_pane_ratio(app, key, ratio);
    return Task::none();
  }
  if let mail::Message::ReauthRequested(id) = msg {
    return update(app, Message::ReauthCharacter(id));
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
  let update = mail::update(state, msg, &runtime.db).map(Message::Mail);
  Task::batch([update, mail_recipient_search(state, runtime, query, is_to)])
}

/// Captures the draft's current search generation and stamps it onto the async result so a stale
/// response arriving after the user has typed again is discarded by the handler.
///
/// Mail recipients are characters only, so the shared entity loader is restricted to `[Character]`.
fn mail_recipient_search(state: &mail::State, runtime: &Runtime, query: String, is_to: bool) -> Task<Message> {
  use crate::features::entity_search;

  if query.trim().chars().count() < mail::RECIPIENT_SEARCH_MIN_CHARS {
    return Task::none();
  }
  let generation = state.compose_search_generation(is_to);
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let eve_image = Arc::clone(&runtime.eve_image);
  let sso = Arc::clone(&runtime.sso);
  let categories = vec![entity_search::EntityCategory::Character];
  Task::perform(
    async move { entity_search::search_entities(db, esi, eve_image, sso, categories, query).await },
    move |results| {
      let results = results
        .into_iter()
        .map(|result| crate::ui::components::entity_search::EntityRef {
          id: result.id,
          kind: crate::ui::components::entity_search::EntityKind::Character,
          name: result.name,
          portrait: Some(store::images::default_store().image_path(result.category.image_kind(), result.id)),
        })
        .collect();
      Message::Mail(if is_to {
        mail::Message::ComposeToSearched {
          generation,
          results,
        }
      } else {
        mail::Message::ComposeCcSearched {
          generation,
          results,
        }
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
    skill_plan_editor::Message::PaneSettled(key, ratio) => {
      record_pane_ratio(app, key, ratio);
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

fn handle_engine_stopped(app: &mut App, reason: Option<String>) -> Task<Message> {
  // A parked (read-only) instance's inert event stream closes immediately; ignore that close so it
  // doesn't overwrite ReadOnly with Stopped.
  if app.engine_state.is_read_only() {
    return Task::none();
  }
  tracing::warn!(
    target: "pod::lifecycle",
    reason = reason.as_deref().unwrap_or("unknown"),
    "the sync engine task stopped"
  );
  app.engine_state = EngineState::Stopped {
    reason,
  };
  Task::none()
}

fn refresh_running_engine_state(app: &mut App) {
  if app.engine_state.settled() {
    return;
  }
  app.engine_state = if expected_job_stats(app).in_progress() {
    EngineState::Running
  } else {
    EngineState::Idle
  };
}

fn handle_restart_sync(app: &mut App) -> Task<Message> {
  if !app.engine_state.is_stopped() {
    return Task::none();
  }
  // The supervisor outlives any single engine and stays parked after it gives up, so the live Handle
  // still reaches it. Signalling restart_sync resets the circuit breaker and respawns a fresh engine
  // over the same command/event channels — no Runtime rebuild and no re-threaded Handle.
  let Some(runtime) = app.runtime.as_ref() else {
    tracing::warn!(
      target: "pod::lifecycle",
      "restart requested but the runtime handle is unavailable; leaving sync stopped"
    );
    return Task::none();
  };
  tracing::info!(target: "pod::lifecycle", "restarting the sync engine on request");
  runtime.sync.restart_sync();
  app.engine_state = EngineState::Running;
  Task::none()
}

fn apply_engine_lifecycle(app: &mut App, event: &sync::Event) {
  // A parked read-only instance runs an inert engine, so its lifecycle is owned by the lease, not by
  // these events; never let a stray sync event overwrite ReadOnly.
  if app.engine_state.is_read_only() {
    return;
  }
  match event {
    sync::Event::GaveUp {
      reason,
    } => {
      tracing::warn!(target: "pod::lifecycle", %reason, "the sync engine supervisor gave up auto-restart");
      app.engine_state = EngineState::Stopped {
        reason: Some(reason.clone()),
      };
    }
    // A respawn (auto or manual) means the engine is live again; leave the settled-state early return
    // behind and recompute Running/Idle from the in-flight job stats.
    sync::Event::Restarted {
      ..
    } => {
      app.engine_state = if expected_job_stats(app).in_progress() {
        EngineState::Running
      } else {
        EngineState::Idle
      };
    }
    _ => refresh_running_engine_state(app),
  }
}

fn handle_sync(app: &mut App, event: sync::Event) -> Task<Message> {
  app.status.apply(&event);
  app.outbox.apply(&event);
  apply_engine_lifecycle(app, &event);
  let sync::Event::Finished {
    key, ..
  } = event
  else {
    return Task::none();
  };
  app.last_synced = Some(app.now);
  // Defer every screen reload to the next SyncPulse so a burst of Finished events coalesces into one
  // reload apiece instead of starving the interactive DB pool with one reload each.
  app.roster_dirty = true;
  mark_detail_dirty(app, key);
  mark_wallet_dirty(app, key);
  mark_assets_dirty(app, key);
  Task::none()
}

fn mark_assets_dirty(app: &mut App, key: JobKey) {
  if app.route == Route::Assets && key.kind == JobKind::AssetSync && app.assets.is_some() {
    app.assets_dirty = true;
  }
}

fn mark_detail_dirty(app: &mut App, key: JobKey) {
  if let Some(data_type) = detail_reload_target(app.character_detail.as_ref(), key) {
    app.detail_dirty.insert(data_type);
  }
}

fn mark_wallet_dirty(app: &mut App, key: JobKey) {
  if app.route == Route::Wallet && wallet_reload_kind(key.kind) && app.wallet.is_some() {
    app.wallet_dirty = true;
  }
}

fn handle_window(app: &mut App, id: window::Id, event: window::Event) -> Task<Message> {
  match event {
    window::Event::Resized(size) => {
      let base = window_key(app, id).and_then(|key| app.ui_state.windows.get(key).copied());
      record_window_geometry(app, id, geometry_after_resize(base, size));
      propagate_host_width(app, id, size.width);
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
    window::Event::Closed => {
      flush_pending_save(app);
      on_window_closed(app, id)
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
      granted_scopes: None,
      id,
      name: format!("Pilot {id}"),
    }
  }

  fn test_app() -> App {
    App {
      about: None,
      accessibility: config::AccessibilityConfig::default(),
      assets: None,
      assets_dirty: false,
      auth: auth::State::default(),
      calendar: None,
      calendar_attention: 0,
      character_detail: None,
      character_manager: None,
      coalescer: WriteCoalescer::new(),
      compare: None,
      confirm_force_takeover: false,
      corporation_detail: None,
      detail_dirty: HashSet::new(),
      editor: None,
      engine_state: EngineState::default(),
      esi_connected: true,
      industry: None,
      init_error: None,
      last_push: None,
      last_synced: None,
      mail: None,
      mail_unread: 0,
      now: Utc::now(),
      outbox: sync::OutboxStatus::new(),
      pending_auth: None,
      pending_images: HashSet::new(),
      read_only: None,
      roster_dirty: false,
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
      wallet_dirty: false,
      windows: Windows::default(),
    }
  }

  async fn test_runtime() -> Runtime {
    let (runtime, _rx) = test_runtime_with_commands().await;
    runtime
  }

  async fn test_runtime_with_commands() -> (Runtime, tokio::sync::mpsc::UnboundedReceiver<sync::Command>) {
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = Arc::new(esi::Client::builder(http.clone()).user_agent("test").build().unwrap());
    let eve_image = Arc::new(eve_image::Client::new(http.clone()));
    let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let (restart_tx, _restart_rx) = tokio::sync::mpsc::unbounded_channel();
    let runtime = Runtime {
      db,
      esi,
      eve_image,
      settings: config::Settings::default(),
      sso,
      sync: sync::Handle::new(tx, restart_tx),
    };
    (runtime, rx)
  }

  async fn test_runtime_with_restart() -> (Runtime, tokio::sync::mpsc::UnboundedReceiver<()>) {
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = Arc::new(esi::Client::builder(http.clone()).user_agent("test").build().unwrap());
    let eve_image = Arc::new(eve_image::Client::new(http.clone()));
    let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let (restart_tx, restart_rx) = tokio::sync::mpsc::unbounded_channel();
    let runtime = Runtime {
      db,
      esi,
      eve_image,
      settings: config::Settings::default(),
      sso,
      sync: sync::Handle::new(tx, restart_tx),
    };
    (runtime, restart_rx)
  }

  fn temp_sync_session() -> (tempfile::TempDir, store::sync_session::SyncSession) {
    let dir = tempfile::tempdir().unwrap();
    let share = dir.path().join("share");
    let cache = dir.path().join("cache");
    std::fs::create_dir_all(&share).unwrap();
    std::fs::create_dir_all(&cache).unwrap();

    let mut storage = config::StorageConfig::default();
    storage.set_db_dir(Some(share));
    storage.set_cache_dir(Some(cache));
    storage.set_working_copy_dir(Some(dir.path().join("working-copy")));
    storage.set_network(true);

    let session = store::sync_session::SyncSession::from_config(&storage, "machine-test".to_owned())
      .expect("sync mode yields a session");
    (dir, session)
  }

  mod build_sync_esi {
    use super::*;
    use crate::store::{model::HttpCacheEntry, repo::infra};

    #[tokio::test]
    async fn it_backs_the_sync_clients_cache_with_the_supplied_sync_pool() {
      let sync_db = store::open_test().await.unwrap();
      let unrelated_db = store::open_test().await.unwrap();
      let url = "https://esi.example/character/1/assets";
      let entry = HttpCacheEntry::new(b"sync-pool".to_vec(), 0, url);
      infra::http_cache_upsert(&sync_db, &entry).await.unwrap();

      let sync_esi = build_sync_esi(sync_db).unwrap();
      let cache_db = sync_esi.http().cache_db().clone();

      assert!(
        infra::http_cache_get(&cache_db, url).await.unwrap().is_some(),
        "the sync client reads its cache from the supplied sync pool"
      );
      assert!(
        infra::http_cache_get(&unrelated_db, url).await.unwrap().is_none(),
        "an unrelated pool cannot see the sync pool's cache, proving the wiring is pool-specific"
      );
    }

    #[tokio::test]
    async fn it_builds_a_distinct_client_from_an_interactive_pool_client() {
      let interactive_db = store::open_test().await.unwrap();
      let ui_http = http::Client::builder(http::Cache::new(interactive_db.clone())).build();
      let ui_esi = Arc::new(esi::Client::builder(ui_http).user_agent("test").build().unwrap());

      let sync_esi = build_sync_esi(interactive_db).unwrap();

      assert!(
        !Arc::ptr_eq(&sync_esi.http(), &ui_esi.http()),
        "the sync engine no longer shares the interactive-pool-backed HTTP client"
      );
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
    fn it_maps_a_calendar_route_to_the_calendar_destination() {
      assert_eq!(Route::Calendar.destination(), rail::Destination::Calendar);
    }

    #[test]
    fn it_round_trips_characters_settings_and_mail_through_from() {
      assert_eq!(Route::from(Route::Characters.destination()), Route::Characters);
      assert_eq!(Route::from(Route::Settings.destination()), Route::Settings);
      assert_eq!(Route::from(Route::Mail.destination()), Route::Mail);
      assert_eq!(Route::from(Route::Calendar.destination()), Route::Calendar);
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

  mod resolve_mail_target {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_to_the_first_owned_pilot_with_no_prior_selection() {
      let roster = vec![pilot(7), pilot(3)];

      assert_eq!(resolve_mail_target(&roster, None), Some(7));
    }

    #[test]
    fn it_keeps_the_sticky_selection_when_still_owned() {
      let roster = vec![pilot(7), pilot(3)];

      assert_eq!(resolve_mail_target(&roster, Some(3)), Some(3));
    }

    #[test]
    fn it_falls_back_to_first_owned_when_the_sticky_selection_left_the_roster() {
      let roster = vec![pilot(7), pilot(3)];

      assert_eq!(resolve_mail_target(&roster, Some(99)), Some(7));
    }

    #[test]
    fn it_yields_none_for_an_empty_roster() {
      assert_eq!(resolve_mail_target(&[], None), None);
      assert_eq!(resolve_mail_target(&[], Some(7)), None);
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

  mod file_filter {
    use pretty_assertions::assert_eq;

    use super::*;

    fn clamp_of(filter: &str) -> &str {
      filter.split_once(',').expect("filter has a pod= prefix").1
    }

    #[test]
    fn it_varies_only_the_pod_level_per_log_level() {
      assert_eq!(
        file_filter(config::LogLevel::Quiet),
        format!("pod=info,{FILE_FILTER_CLAMP}")
      );
      assert_eq!(
        file_filter(config::LogLevel::Normal),
        format!("pod=debug,{FILE_FILTER_CLAMP}")
      );
      assert_eq!(
        file_filter(config::LogLevel::Verbose),
        format!("pod=trace,{FILE_FILTER_CLAMP}")
      );
    }

    #[test]
    fn it_keeps_every_dependency_clamp_identical_across_levels() {
      let quiet = file_filter(config::LogLevel::Quiet);
      let normal = file_filter(config::LogLevel::Normal);
      let verbose = file_filter(config::LogLevel::Verbose);

      assert_eq!(clamp_of(&quiet), FILE_FILTER_CLAMP);
      assert_eq!(clamp_of(&normal), FILE_FILTER_CLAMP);
      assert_eq!(clamp_of(&verbose), FILE_FILTER_CLAMP);
    }
  }

  mod scale_to_factor {
    use super::*;

    #[test]
    fn it_maps_a_default_scale_to_a_unit_factor() {
      assert_eq!(scale_to_factor(100), 1.0);
    }

    #[test]
    fn it_maps_the_extremes_of_the_range() {
      assert_eq!(scale_to_factor(85), 0.85);
      assert_eq!(scale_to_factor(150), 1.5);
    }

    #[test]
    fn it_clamps_values_outside_the_supported_range() {
      assert_eq!(scale_to_factor(0), 0.85);
      assert_eq!(scale_to_factor(255), 1.5);
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

    fn sized(width: f32, height: f32) -> WindowGeometry {
      WindowGeometry {
        height,
        width,
        x: 100.0,
        y: 100.0,
      }
    }

    #[test]
    fn it_defaults_the_size_for_a_zero_sized_window() {
      let (size, _) = resolve_window_geometry(Some(sized(0.0, 0.0)), &[monitor()], DEFAULT);

      assert_eq!(size, DEFAULT, "a 0x0 saved size never reopens broken");
    }

    #[test]
    fn it_defaults_the_size_for_negative_or_non_finite_dimensions() {
      assert_eq!(
        resolve_window_geometry(Some(sized(-1200.0, 800.0)), &[monitor()], DEFAULT).0,
        DEFAULT
      );
      assert_eq!(
        resolve_window_geometry(Some(sized(f32::NAN, 800.0)), &[monitor()], DEFAULT).0,
        DEFAULT
      );
      assert_eq!(
        resolve_window_geometry(Some(sized(1200.0, f32::INFINITY)), &[monitor()], DEFAULT).0,
        DEFAULT
      );
    }

    #[test]
    fn it_defaults_the_size_for_an_absurdly_large_window() {
      let (size, _) = resolve_window_geometry(Some(sized(999_999.0, 999_999.0)), &[monitor()], DEFAULT);

      assert_eq!(size, DEFAULT);
    }

    #[test]
    fn it_clamps_a_valid_size_below_the_floor_up_to_the_minimum() {
      let (size, _) = resolve_window_geometry(Some(sized(700.0, 500.0)), &[monitor()], DEFAULT);

      assert_eq!(
        size,
        Size::new(800.0, 600.0),
        "a too-small but valid size is raised to the floor"
      );
    }

    #[test]
    fn it_restores_a_size_at_or_above_the_floor_unchanged() {
      let (size, _) = resolve_window_geometry(Some(sized(900.0, 650.0)), &[monitor()], DEFAULT);

      assert_eq!(size, Size::new(900.0, 650.0));
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

    fn finished_event(character_id: i64) -> sync::Event {
      sync::Event::Finished {
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(character_id)),
        outcome: sync::Outcome::synced(),
      }
    }

    #[test]
    fn it_coalesces_a_burst_of_finished_events_into_one_pending_roster_refresh() {
      let mut app = test_app();
      app.character_manager = Some(character_manager::State::new());

      for character_id in 0..6 {
        let _ = update(&mut app, Message::Sync(finished_event(character_id)));
      }
      assert!(
        app.roster_dirty,
        "a burst of Finished events marks the roster dirty once instead of reloading per event"
      );

      let _ = update(&mut app, Message::SyncPulse);
      assert!(!app.roster_dirty, "the pulse consumes the coalesced refresh");

      let _ = update(&mut app, Message::SyncPulse);
      assert!(!app.roster_dirty, "a quiet pulse schedules no further reload");
    }

    fn asset_sync_event(character_id: i64) -> sync::Event {
      sync::Event::Finished {
        key: JobKey::new(JobKind::AssetSync, Subject::Character(character_id)),
        outcome: sync::Outcome::synced(),
      }
    }

    #[test]
    fn it_coalesces_a_burst_of_asset_syncs_into_one_pending_assets_refresh() {
      let mut app = test_app();
      app.route = Route::Assets;
      app.assets = Some(assets::State::new());

      for character_id in 0..6 {
        let _ = update(&mut app, Message::Sync(asset_sync_event(character_id)));
      }

      assert!(
        app.assets_dirty,
        "a burst of AssetSync events marks the assets dirty once instead of reloading per event"
      );

      let _ = update(&mut app, Message::SyncPulse);
      assert!(!app.assets_dirty, "the pulse consumes the coalesced assets refresh");

      let _ = update(&mut app, Message::SyncPulse);
      assert!(!app.assets_dirty, "a quiet pulse schedules no further assets reload");
    }

    #[test]
    fn it_does_not_mark_assets_dirty_while_off_the_assets_route() {
      let mut app = test_app();
      app.route = Route::Wallet;
      app.assets = Some(assets::State::new());

      let _ = update(&mut app, Message::Sync(asset_sync_event(1)));

      assert!(!app.assets_dirty, "an off-route asset sync schedules no assets reload");
    }

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
    fn it_navigates_to_the_corporation_detail_for_the_selected_corporation() {
      let mut app = test_app();

      let _ = update(
        &mut app,
        Message::CharacterManager(character_manager::Message::CorporationSelected(98_000_001)),
      );

      assert_eq!(app.route, Route::CorporationDetail(98_000_001));
      assert!(app.corporation_detail.is_some());
    }

    #[test]
    fn it_keeps_the_characters_destination_lit_while_a_corporation_is_drilled_in() {
      assert_eq!(
        Route::CorporationDetail(98_000_001).destination(),
        rail::Destination::Characters
      );
    }

    #[test]
    fn it_returns_to_the_roster_grid_when_the_characters_rail_is_activated_from_corp_detail() {
      let mut app = test_app();
      let _ = update(
        &mut app,
        Message::CharacterManager(character_manager::Message::CorporationSelected(98_000_001)),
      );
      assert_eq!(app.route, Route::CorporationDetail(98_000_001));

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
    fn it_navigates_to_the_calendar_screen_on_the_calendar_rail_destination() {
      let mut app = test_app();

      let _ = update(&mut app, Message::Nav(rail::Destination::Calendar));

      assert_eq!(app.route, Route::Calendar);
      assert!(app.calendar.is_some());
      assert_eq!(app.route.destination(), rail::Destination::Calendar);
    }

    #[tokio::test]
    async fn it_redirects_a_disabled_calendar_nav_to_characters() {
      let mut app = test_app();
      let mut runtime = test_runtime().await;
      runtime
        .settings
        .features_mut()
        .set_enabled(config::Feature::Calendar, false);
      app.runtime = Some(runtime);

      let _ = update(&mut app, Message::Nav(rail::Destination::Calendar));

      assert_eq!(app.route, Route::Characters);
      assert!(app.calendar.is_none());
    }

    #[test]
    fn it_navigates_to_the_industry_screen_on_the_industry_rail_destination() {
      let mut app = test_app();

      let _ = update(&mut app, Message::Nav(rail::Destination::Industry));

      assert_eq!(app.route, Route::Industry);
      assert!(app.industry.is_some());
      assert_eq!(app.route.destination(), rail::Destination::Industry);
    }

    #[tokio::test]
    async fn it_redirects_a_disabled_industry_nav_to_characters() {
      let mut app = test_app();
      let mut runtime = test_runtime().await;
      runtime
        .settings
        .features_mut()
        .set_enabled(config::Feature::Industry, false);
      app.runtime = Some(runtime);

      let _ = update(&mut app, Message::Nav(rail::Destination::Industry));

      assert_eq!(app.route, Route::Characters);
      assert!(app.industry.is_none());
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
        sync_db: db.clone(),
        sync_housekeeping_db: db.clone(),
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
        sync_db: db.clone(),
        sync_housekeeping_db: db.clone(),
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
        sync_db: db.clone(),
        sync_housekeeping_db: db.clone(),
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
      app.mail = Some(mail::State::new(42));
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
      render_route(Route::CorporationDetail(1));
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

    #[tokio::test]
    async fn it_builds_the_subscription_set_for_each_live_screen() {
      let app = test_app();
      let _ = subscription(&app);

      let mut app = ready_app();
      let runtime = test_runtime().await;
      app.splash = Some(splash::State::default());
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.calendar = Some(calendar::State::new(1, app.now, calendar_features(&app)));
      app.industry = Some(industry::State::new(
        1,
        industry_required_scopes(),
        industry::FacilityDefaults::default(),
      ));
      app.runtime = Some(runtime);
      app.sync_popover_open = true;
      app.status.apply(&crate::sync::Event::Started {
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(1)),
      });
      app.editor = Some((window::Id::unique(), skill_plan_editor::State::new(1)));

      // Holding the lease arms the heartbeat, periodic-pull, and periodic-push timers.
      let (_dir, session) = temp_sync_session();
      app.sync_session = Some(session);
      app.read_only = None;
      let _ = subscription(&app);

      // A parked (read-only) session arms the re-acquire timer instead.
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });
      let _ = subscription(&app);
    }

    #[tokio::test]
    async fn it_drives_character_detail_through_the_runtime_backed_handler() {
      let mut app = ready_app();
      app.runtime = Some(test_runtime().await);

      // CharacterChanged navigates, selects the pilot, and batches an update with a reload.
      let _ = handle_character_detail(&mut app, character_detail::Message::CharacterChanged(7));
      assert_eq!(app.route, Route::CharacterDetail(7));
      assert_eq!(app.selected_character, Some(7));

      // ReauthRequested reroutes to the app-level reauth flow.
      let _ = handle_character_detail(&mut app, character_detail::Message::ReauthRequested(7));

      // ContactEntityInput batches the modal update with a debounced entity search task.
      let _ = handle_character_detail(
        &mut app,
        character_detail::Message::ContactEntityInput("jita".to_owned()),
      );

      // Any other message falls through to the plain feature update.
      let _ = handle_character_detail(&mut app, character_detail::Message::PickerToggled);
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
      app.mail = Some(mail::State::new(42));

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
      app.mail = Some(mail::State::new(42));
      app.runtime = Some(test_runtime().await);

      let _to = handle_mail(&mut app, mail::Message::ComposeToInput("Vexor".to_owned()));
      let _cc = handle_mail(&mut app, mail::Message::ComposeCcInput("Alli".to_owned()));
      let _scope = handle_mail(&mut app, mail::Message::ScopeSelected(mail::Scope::Character(7)));
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn it_dispatches_each_native_menu_action() {
      let mut app = test_app();

      let _about = handle_menu(&mut app, menu::MenuAction::About);
      let _check = handle_menu(&mut app, menu::MenuAction::CheckUpdates);
    }

    #[test]
    fn it_routes_a_mail_scope_selection_to_a_no_op_without_a_runtime() {
      let mut app = test_app();
      app.mail = Some(mail::State::new(42));

      let _ = update(
        &mut app,
        Message::Mail(mail::Message::ScopeSelected(mail::Scope::Character(7))),
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
        sync_db: db.clone(),
        sync_housekeeping_db: db.clone(),
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
    fn it_records_the_calendar_attention_count() {
      let mut app = test_app();

      let _ = update(&mut app, Message::CalendarAttentionCounted(4));

      assert_eq!(app.calendar_attention, 4);
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
    fn it_keeps_the_app_alive_when_main_closes_while_a_secondary_window_is_open() {
      let mut app = test_app();
      let main_id = window::Id::unique();
      let editor_id = window::Id::unique();
      app.windows.register(main_id, Window::Main);
      app.windows.register(editor_id, Window::SkillPlanEditor);
      app.editor = Some((editor_id, skill_plan_editor::State::new(1)));

      let _ = update(&mut app, Message::Window(main_id, window::Event::CloseRequested));

      assert_eq!(app.windows.kind(main_id), None, "the main window is gone");
      assert_eq!(
        app.windows.kind(editor_id),
        Some(Window::SkillPlanEditor),
        "the still-open editor keeps the app alive"
      );
      assert!(!app.windows.is_empty(), "a surviving window means no shutdown yet");
    }

    #[test]
    fn it_empties_the_registry_when_the_final_window_closes_after_main() {
      let mut app = test_app();
      let main_id = window::Id::unique();
      let editor_id = window::Id::unique();
      app.windows.register(main_id, Window::Main);
      app.windows.register(editor_id, Window::SkillPlanEditor);
      app.editor = Some((editor_id, skill_plan_editor::State::new(1)));

      let _ = update(&mut app, Message::Window(main_id, window::Event::CloseRequested));
      let _ = update(&mut app, Message::Window(editor_id, window::Event::CloseRequested));

      assert!(
        app.windows.is_empty(),
        "closing the last window empties the registry and shuts down"
      );
    }

    #[test]
    fn it_tears_down_on_an_os_kill_that_skips_the_close_request() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::SkillPlanEditor);
      app.editor = Some((id, skill_plan_editor::State::new(1)));

      let _ = update(&mut app, Message::Window(id, window::Event::Closed));

      assert!(app.editor.is_none(), "a compositor-killed editor clears its state");
      assert!(
        app.windows.is_empty(),
        "the destroyed window leaves an empty registry, triggering shutdown"
      );
    }

    #[test]
    fn a_close_event_for_an_already_removed_window_is_a_no_op() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::Main);

      let _ = update(&mut app, Message::Window(id, window::Event::CloseRequested));
      let _ = update(&mut app, Message::Window(id, window::Event::Closed));

      assert!(
        app.windows.is_empty(),
        "the late Closed event finds nothing to remove and does not re-trigger"
      );
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
      let last_seen = Utc::now();
      let holder: Option<HolderInfo> = store::lease::Outcome::HeldBy {
        hostname: "studio-mac".to_owned(),
        last_seen,
        machine_id: "machine-b".to_owned(),
      }
      .into();

      assert_eq!(
        holder,
        Some(HolderInfo {
          hostname: "studio-mac".to_owned(),
          last_active: last_seen,
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
      let _ = handle_periodic_pull(&mut app);
    }

    #[test]
    fn a_read_only_session_neither_pulls_nor_pushes() {
      let mut app = test_app();
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });

      assert!(!holding_lease(&app), "a read-only opener does not pull from the share");
      let _ = handle_periodic_pull(&mut app);
      let _ = handle_periodic_push(&mut app);
    }

    #[test]
    fn a_pull_that_changed_nothing_leaves_the_synced_marker_untouched() {
      let mut app = test_app();
      app.last_synced = None;

      let _ = handle_pulled(&mut app, false);

      assert!(app.last_synced.is_none(), "no pull means no new synced timestamp");
    }

    #[test]
    fn sync_now_without_a_session_is_a_no_op() {
      let mut app = test_app();
      app.last_synced = None;

      let _ = sync_now(&mut app);

      assert!(
        app.last_synced.is_none(),
        "with no sync session there is nothing to sync"
      );
    }

    #[test]
    fn sync_now_with_a_clean_session_stamps_the_synced_time() {
      let (_dir, session) = temp_sync_session();
      let mut app = test_app();
      app.sync_session = Some(session);
      app.last_synced = None;

      let _ = sync_now(&mut app);

      assert!(
        app.last_synced.is_some(),
        "a clean session with nothing to push or pull still refreshes the synced timestamp"
      );
    }

    #[tokio::test]
    async fn pull_bundle_reports_no_change_for_a_fresh_share() {
      let (_dir, session) = temp_sync_session();

      let message = pull_bundle(session).await;

      assert!(
        matches!(message, Message::Pulled(false)),
        "a fresh share has nothing newer to pull"
      );
    }

    #[tokio::test]
    async fn export_log_bundle_writes_nowhere_when_the_save_dialog_is_stubbed() {
      let dir = tempfile::tempdir().unwrap();
      let log_dir = dir.path().join("logs");
      std::fs::create_dir_all(&log_dir).unwrap();
      std::fs::write(log_dir.join("pod.log"), b"{\"ts\":\"now\"}\n").unwrap();
      let diagnostics = settings::log_export::Diagnostics {
        cache_dir: dir.path().join("cache"),
        database_path: dir.path().join("pod.db"),
        db_dir: dir.path().to_path_buf(),
        log_dir: log_dir.clone(),
      };

      let result = export_log_bundle(
        log_dir,
        Utc::now() - chrono::Duration::hours(1),
        Utc::now(),
        diagnostics,
      )
      .await;

      assert_eq!(result, Ok(None), "the cfg(test) save dialog is a no-op");
    }

    #[tokio::test]
    async fn handle_settings_routes_a_tab_switch_through_the_settings_state() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = handle_settings(
        &mut app,
        settings::Message::CategorySelected(settings::Category::Storage),
      );

      assert!(app.settings.is_some(), "switching tabs leaves the settings screen open");
    }

    #[tokio::test]
    async fn handle_settings_without_a_settings_screen_is_a_no_op() {
      let mut app = test_app();

      let _ = handle_settings(
        &mut app,
        settings::Message::CategorySelected(settings::Category::Storage),
      );

      assert!(app.settings.is_none());
    }

    #[tokio::test]
    async fn handle_settings_hoists_an_interface_scale_change_onto_the_app_and_runtime() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = handle_settings(
        &mut app,
        settings::Message::Accessibility(settings::accessibility_tab::Message::ScaleChanged(125)),
      );

      assert_eq!(
        *app.accessibility.scale(),
        125,
        "the new scale is hoisted onto the app live"
      );
      assert_eq!(
        *app.runtime.as_ref().unwrap().settings.accessibility().scale(),
        125,
        "the runtime settings mirror the new scale so a later save persists it",
      );
    }

    #[tokio::test]
    async fn handle_settings_drives_the_color_engine_when_high_contrast_toggles() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = handle_settings(
        &mut app,
        settings::Message::Accessibility(settings::accessibility_tab::Message::HighContrastToggled(true)),
      );

      assert!(*app.accessibility.high_contrast(), "the toggle is hoisted onto the app");
      assert!(
        color::high_contrast(),
        "the runtime color engine reflects the high-contrast toggle"
      );

      // The engine flag is process-global; leave it as it was found for any sibling tests.
      color::set_high_contrast(false);
    }

    #[tokio::test]
    async fn handle_settings_sends_a_feature_toggle_to_the_running_sync_engine() {
      let (runtime, mut commands) = test_runtime_with_commands().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = handle_settings(
        &mut app,
        settings::Message::Features(settings::features_tab::Message::Toggled(config::Feature::Wallet, false)),
      );

      let command = commands.try_recv().expect("a feature toggle reaches the engine");
      let sync::Command::SetFeatures(flags) = command else {
        panic!("expected SetFeatures, got {command:?}");
      };
      assert!(
        !flags.is_enabled(config::Feature::Wallet),
        "the engine receives the post-toggle feature flags"
      );
    }

    #[tokio::test]
    async fn handle_settings_sends_set_features_to_the_engine_on_reset_to_defaults() {
      let (runtime, mut commands) = test_runtime_with_commands().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = handle_settings(&mut app, settings::Message::ResetToDefaults);

      let command = commands.try_recv().expect("resetting to defaults reaches the engine");
      assert!(
        matches!(command, sync::Command::SetFeatures(_)),
        "reset-to-defaults reconciles the running engine, got {command:?}"
      );
    }

    #[tokio::test]
    async fn handle_settings_rebuilds_the_char_detail_tab_strip_on_a_toggle() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.character_detail = Some(character_detail::State::new(7, &config::Feature::ALL));
      app.runtime = Some(runtime);

      let _ = handle_settings(
        &mut app,
        settings::Message::Features(settings::features_tab::Message::Toggled(
          config::Feature::Standings,
          false,
        )),
      );

      let detail = app.character_detail.as_ref().expect("the detail screen stays open");
      assert!(
        !detail.enabled_tabs().contains(&character_detail::Tab::Standings),
        "disabling Standings drops its detail tab live, not only on next navigation"
      );
    }

    #[tokio::test]
    async fn handle_settings_syncs_industry_defaults_onto_the_runtime() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      // Picking an NPC station as the Manufacturing default persists it; the planner reads the runtime
      // copy on open, so it must mirror the change immediately (not only after a restart).
      let facility = crate::ui::components::facility_combobox::FacilityRef {
        cost_index: Some(0.05),
        id: 60_003_760,
        name: "Jita IV - Moon 4 - CNAP".to_owned(),
        region: Some("The Forge".to_owned()),
        security_status: Some(0.9),
        solar_system: "Jita".to_owned(),
        solar_system_id: 30_000_142,
        type_id: None,
      };

      let _ = handle_settings(
        &mut app,
        settings::Message::Industry(settings::industry_tab::Message::FacilityPicked {
          activity: 1,
          facility,
        }),
      );

      assert_eq!(
        *app.runtime.as_ref().unwrap().settings.industry().manufacturing(),
        Some(60_003_760),
        "the runtime industry config mirrors the settings screen so the planner honors the new default"
      );
    }

    fn test_industry_state() -> industry::State {
      industry::State::new(
        industry::EMPTY_INDUSTRY_SELECTION,
        Vec::new(),
        industry::FacilityDefaults::default(),
      )
    }

    #[tokio::test]
    async fn handle_industry_records_a_pane_ratio_before_the_runtime_gate() {
      let mut app = test_app();

      let _ = handle_industry(
        &mut app,
        industry::Message::PaneSettled("industry.planner.detail", 0.42),
      );

      assert_eq!(
        app.ui_state.panes.get("industry.planner.detail"),
        Some(&0.42),
        "a pane drag is recorded even without a runtime or industry screen"
      );
    }

    #[tokio::test]
    async fn handle_industry_reauth_request_defers_an_auth_start() {
      let mut app = test_app();

      let _ = handle_industry(&mut app, industry::Message::ReauthRequested(7));

      assert!(
        app.pending_auth.is_some(),
        "a re-auth request from the industry screen defers an auth Start"
      );
    }

    #[tokio::test]
    async fn handle_industry_without_a_screen_is_a_no_op() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.runtime = Some(runtime);

      let _ = handle_industry(&mut app, industry::Message::TabSelected(industry::Tab::Planner));

      assert!(
        app.industry.is_none(),
        "with no industry screen open the message is dropped"
      );
    }

    #[tokio::test]
    async fn handle_industry_dispatches_a_message_through_the_reducer() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.industry = Some(test_industry_state());
      app.runtime = Some(runtime);

      let _ = handle_industry(&mut app, industry::Message::TabSelected(industry::Tab::Blueprints));

      assert!(
        app.industry.is_some(),
        "the industry screen stays open after a plain reducer message"
      );
    }

    #[tokio::test]
    async fn handle_industry_seams_the_facility_search_for_the_planner() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.industry = Some(test_industry_state());
      app.runtime = Some(runtime);

      let _ = handle_industry(
        &mut app,
        industry::Message::Planner(industry::PlannerMessage::FacilitySearchChanged {
          path: Vec::new(),
          query: "jita".to_owned(),
        }),
      );

      assert!(
        app.industry.as_ref().unwrap().facility_search_target().is_some(),
        "typing into the facility field opens the picker and arms a live search"
      );
    }

    #[test]
    fn a_reauth_from_a_403_state_requests_the_full_enabled_feature_scope_set() {
      // No runtime, so the auth Start is deferred and we can inspect its feature set without
      // opening a browser. With Mail and Skills both enabled, a single re-auth must request both.
      let mut app = test_app();

      let _ = handle_reauth_character(&mut app, 7);

      let Some(auth::Message::Start(features)) = app.pending_auth.clone() else {
        panic!("a re-auth defers an auth Start, got {:?}", app.pending_auth);
      };
      assert!(
        features.contains(&config::Feature::Mail) && features.contains(&config::Feature::SkillMonitoring),
        "the single re-auth carries the full enabled-feature set, not a per-feature subset"
      );

      let scopes = auth::scopes_for(&features);
      assert!(
        auth::scopes_for(&[config::Feature::Mail])
          .iter()
          .all(|scope| scopes.contains(scope)),
        "re-auth requests Mail scopes"
      );
      assert!(
        auth::scopes_for(&[config::Feature::SkillMonitoring])
          .iter()
          .all(|scope| scopes.contains(scope)),
        "the same single re-auth also requests Skills scopes"
      );
    }

    #[tokio::test]
    async fn re_enabling_a_feature_restores_its_scopes_to_the_live_reauth_set() {
      // Reproduces the disable -> re-enable -> re-auth flow: a toggle must reach the running
      // runtime so that a later re-auth (which reads the live enabled set) requests the restored
      // feature's scopes, even with the settings screen closed.
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let toggle = |app: &mut App, value| {
        let _ = handle_settings(
          app,
          settings::Message::Features(settings::features_tab::Message::Toggled(config::Feature::Mail, value)),
        );
      };
      toggle(&mut app, false);
      assert!(
        !enabled_features(&app).contains(&config::Feature::Mail),
        "disabling Mail removes it from the live enabled set"
      );
      toggle(&mut app, true);

      // Close the settings screen so the enabled set resolves from the running runtime, not the panel.
      app.settings = None;
      let features = enabled_features(&app);

      assert!(
        features.contains(&config::Feature::Mail),
        "re-enabling Mail restores it to the live runtime the re-auth reads from"
      );
      let scopes = auth::scopes_for(&features);
      assert!(
        auth::scopes_for(&[config::Feature::Mail])
          .iter()
          .all(|scope| scopes.contains(scope)),
        "the re-auth requests the re-enabled Mail scopes"
      );
    }

    #[tokio::test]
    async fn disabling_industry_while_open_redirects_to_characters_and_re_enabling_restores_the_route() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = update(&mut app, Message::Nav(rail::Destination::Industry));
      assert_eq!(app.route, Route::Industry, "Industry is reachable while enabled");

      let _ = handle_settings(
        &mut app,
        settings::Message::Features(settings::features_tab::Message::Toggled(
          config::Feature::Industry,
          false,
        )),
      );

      assert_eq!(
        app.route,
        Route::Characters,
        "disabling Industry while its screen is open redirects to Characters"
      );

      let _ = handle_settings(
        &mut app,
        settings::Message::Features(settings::features_tab::Message::Toggled(
          config::Feature::Industry,
          true,
        )),
      );
      let _ = update(&mut app, Message::Nav(rail::Destination::Industry));

      assert_eq!(
        app.route,
        Route::Industry,
        "re-enabling Industry restores the route instantly"
      );
    }

    #[tokio::test]
    async fn a_card_reauth_after_toggling_requests_every_enabled_scope_through_the_real_dispatch() {
      // Full repro of the user flow with no runtime, so the auth Start is deferred into
      // `pending_auth` and we can read the exact feature set it carries.
      let db = crate::store::open_test().await.unwrap();
      let mut app = test_app();
      app.settings = Some(settings::State::new(config::Settings::default(), db));

      // Disable then re-enable Mail and Skills through the real settings handler.
      for feature in [config::Feature::Mail, config::Feature::SkillMonitoring] {
        for value in [false, true] {
          let _ = handle_settings(
            &mut app,
            settings::Message::Features(settings::features_tab::Message::Toggled(feature, value)),
          );
        }
      }

      // Drive the card / context-menu re-auth message through the top-level dispatch.
      let _ = update(
        &mut app,
        Message::CharacterManager(character_manager::Message::ReauthCharacterRequested(7)),
      );

      let Some(auth::Message::Start(features)) = app.pending_auth.clone() else {
        panic!("the re-auth must defer an auth Start, got {:?}", app.pending_auth);
      };
      assert!(
        features.contains(&config::Feature::Mail) && features.contains(&config::Feature::SkillMonitoring),
        "a re-auth after re-enabling features must request their scopes, got {features:?}"
      );
    }

    #[tokio::test]
    async fn handle_auth_cancel_with_a_runtime_is_handled_not_deferred() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.runtime = Some(runtime);

      let _ = handle_auth(&mut app, auth::Message::Cancel);

      assert!(
        app.pending_auth.is_none(),
        "with a runtime present the auth message is handled inline, not queued"
      );
    }

    #[test]
    fn handle_auth_without_a_runtime_defers_the_message() {
      let mut app = test_app();

      let _ = handle_auth(&mut app, auth::Message::Cancel);

      assert!(
        app.pending_auth.is_some(),
        "auth before the runtime is ready is queued until boot completes"
      );
    }

    #[test]
    fn a_completed_reconcile_stamps_the_synced_time_and_refreshes_after_a_pull() {
      let mut app = test_app();
      app.last_synced = None;

      let _ = handle_sync_now_resolved(
        &mut app,
        SyncNowOutcome::Reconciled {
          mark: None,
          pulled: true,
        },
      );

      assert!(
        app.last_synced.is_some(),
        "a completed 'Sync now' updates the visible last-synced status"
      );
      assert!(app.roster_dirty, "a pull marks the roster for a refresh");
    }

    #[test]
    fn a_failed_sync_does_not_claim_success() {
      let mut app = test_app();
      app.last_synced = None;

      let _ = handle_sync_now_resolved(&mut app, SyncNowOutcome::Failed);

      assert!(
        app.last_synced.is_none(),
        "a failed sync leaves the last-synced status stale"
      );
    }

    #[test]
    fn a_read_only_session_neither_heartbeats_nor_pushes() {
      let mut app = test_app();
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
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
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });

      let _ = handle_take_over(&mut app);

      assert!(
        app.read_only.is_some(),
        "with no sync session the request short-circuits and the banner stays"
      );
      assert!(
        !app.confirm_force_takeover,
        "with no sync session there is nothing to confirm"
      );
    }

    #[test]
    fn pressing_take_over_opens_the_confirmation_without_claiming() {
      let (_dir, session) = temp_sync_session();
      let mut app = test_app();
      app.sync_session = Some(session);
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });

      let _ = handle_take_over(&mut app);

      assert!(app.confirm_force_takeover, "the data-loss confirmation is shown");
      assert!(app.read_only.is_some(), "the share is not claimed on the first press");
    }

    #[test]
    fn cancelling_the_confirmation_leaves_the_instance_read_only() {
      let mut app = test_app();
      app.confirm_force_takeover = true;
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });

      let _ = handle_cancel_take_over(&mut app);

      assert!(!app.confirm_force_takeover, "cancelling closes the confirmation");
      assert!(app.read_only.is_some(), "cancelling leaves the instance read-only");
    }

    #[test]
    fn confirming_closes_the_gate_even_when_it_short_circuits() {
      let mut app = test_app();
      app.confirm_force_takeover = true;
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });

      let _ = handle_confirm_take_over(&mut app);

      assert!(!app.confirm_force_takeover, "confirming always closes the gate");
      assert!(
        app.read_only.is_some(),
        "with no sync session the forceful claim short-circuits and the banner stays"
      );
    }

    #[test]
    fn the_force_takeover_confirmation_warns_of_data_loss_and_names_the_last_active_age() {
      let label = read_only_confirm_label("studio-mac", "12s ago");

      assert_eq!(
        label,
        "studio-mac was last active 12s ago. Taking over overwrites any unsaved changes it still has open. Continue?"
      );
    }

    #[test]
    fn a_claimed_take_over_drops_read_only() {
      let mut app = test_app();
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });

      let _ = handle_take_over_resolved(&mut app, TakeOverOutcome::Claimed);

      assert!(app.read_only.is_none(), "claiming the share makes the app writable");
    }

    #[test]
    fn the_initial_read_only_banner_invites_a_take_over() {
      let label = read_only_banner_label("studio-mac");

      assert_eq!(label, "Open on studio-mac \u{2014} close it there, or take over.");
    }

    #[test]
    fn a_failed_take_over_keeps_the_app_read_only() {
      let mut app = test_app();
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });

      let _ = handle_take_over_resolved(&mut app, TakeOverOutcome::Failed);

      assert!(app.read_only.is_some(), "a failed take-over leaves the app read-only");
    }

    #[test]
    fn parked_is_the_symmetric_inverse_of_holding_the_lease() {
      let (_dir, session) = temp_sync_session();
      let mut app = test_app();
      app.sync_session = Some(session);
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });

      assert!(parked(&app), "a read-only opener with a session is parked");
      assert!(!holding_lease(&app), "a parked opener does not hold the lease");

      app.read_only = None;

      assert!(!parked(&app), "clearing read-only ends the parked state");
      assert!(holding_lease(&app), "a writable opener holds the lease");
    }

    #[test]
    fn direct_mode_is_neither_parked_nor_holding_the_lease() {
      let app = test_app();

      assert!(!parked(&app), "with no sync session there is nothing parked");
      assert!(!holding_lease(&app), "with no sync session there is no lease to hold");
    }

    #[test]
    fn re_acquire_is_a_no_op_when_the_app_is_writable() {
      let (_dir, session) = temp_sync_session();
      let mut app = test_app();
      app.sync_session = Some(session);

      let _ = handle_reacquire_lease(&mut app);

      assert!(app.read_only.is_none(), "a writable app is never re-acquired");
    }

    #[test]
    fn re_acquire_without_a_sync_session_short_circuits() {
      let mut app = test_app();
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });

      let _ = handle_reacquire_lease(&mut app);

      assert!(
        app.read_only.is_some(),
        "with no sync session the re-acquire short-circuits and the parked banner stays"
      );
    }

    #[test]
    fn a_declined_re_acquire_poll_writes_nothing_to_the_lease() {
      let (dir, session) = temp_sync_session();
      let share = dir.path().join("share");
      let now = Utc::now();
      store::lease::LeaseManager::new("machine-holder".to_owned(), "studio-mac".to_owned(), 99, 0)
        .heartbeat(&share, now)
        .unwrap();
      let lease_path = store::lease::LeaseManager::lease_path(&share);
      let before = std::fs::read(&lease_path).unwrap();

      let outcome = session.take_over(now).unwrap();
      let after = std::fs::read(&lease_path).unwrap();

      assert_eq!(
        outcome,
        store::lease::Outcome::HeldBy {
          hostname: "studio-mac".to_owned(),
          last_seen: store::share_meta::Lease::read(&lease_path).unwrap().heartbeat,
          machine_id: "machine-holder".to_owned(),
        },
        "a still-fresh holder is reported, not displaced"
      );
      assert_eq!(
        before, after,
        "a declined poll heartbeats nothing and never overwrites the foreign lease"
      );
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
        Message::CharacterManager(character_manager::Message::ReauthCorporationRequested(7)),
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
      settings
        .storage_mut()
        .set_working_copy_dir(Some(dir.path().join("working-copy")));

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
    fn it_surfaces_an_empty_outcome_as_benign_and_a_blocked_outcome_as_attention() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::Finished {
        key: key(),
        outcome: crate::sync::Outcome::Empty,
      });
      assert_eq!(
        row_state(&status, &key()),
        (RowState::Empty, Some("No data".to_owned())),
        "a successful empty sync is benign, not an amber attention chip"
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

  mod mark_assets_dirty {
    use super::*;

    fn finished(kind: JobKind) -> JobKey {
      JobKey::new(kind, Subject::Character(1))
    }

    #[test]
    fn it_marks_assets_dirty_on_route_for_an_asset_sync() {
      let mut app = test_app();
      app.route = Route::Assets;
      app.assets = Some(assets::State::new());

      mark_assets_dirty(&mut app, finished(JobKind::AssetSync));

      assert!(app.assets_dirty);
    }

    #[test]
    fn it_skips_the_assets_reload_off_route() {
      let mut app = test_app();
      app.route = Route::Wallet;
      app.assets = Some(assets::State::new());

      mark_assets_dirty(&mut app, finished(JobKind::AssetSync));

      assert!(!app.assets_dirty);
    }

    #[test]
    fn it_skips_the_assets_reload_for_an_unrelated_kind() {
      let mut app = test_app();
      app.route = Route::Assets;
      app.assets = Some(assets::State::new());

      mark_assets_dirty(&mut app, finished(JobKind::CharacterWallet));

      assert!(!app.assets_dirty);
    }
  }

  mod mark_wallet_dirty {
    use super::*;

    fn finished(kind: JobKind) -> JobKey {
      JobKey::new(kind, Subject::Character(1))
    }

    #[test]
    fn it_marks_the_wallet_dirty_on_route_for_a_ledger_kind() {
      let mut app = test_app();
      app.route = Route::Wallet;
      app.wallet = Some(wallet::State::new());

      mark_wallet_dirty(&mut app, finished(JobKind::CharacterWallet));

      assert!(app.wallet_dirty);
    }

    #[test]
    fn it_skips_the_wallet_reload_off_route() {
      let mut app = test_app();
      app.route = Route::Assets;
      app.wallet = Some(wallet::State::new());

      mark_wallet_dirty(&mut app, finished(JobKind::CharacterWallet));

      assert!(!app.wallet_dirty);
    }

    #[test]
    fn it_skips_the_wallet_reload_for_an_unrelated_kind() {
      let mut app = test_app();
      app.route = Route::Wallet;
      app.wallet = Some(wallet::State::new());

      mark_wallet_dirty(&mut app, finished(JobKind::AssetSync));

      assert!(!app.wallet_dirty);
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

  mod image_self_heal {
    use super::*;
    use crate::store::images::ImageKind;

    #[tokio::test]
    async fn it_marks_each_stale_key_in_flight_exactly_once() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let _task = dispatch_image_fetches(
        &mut app,
        vec![(ImageKind::CharacterPortrait, 42), (ImageKind::CharacterPortrait, 42)],
      );

      assert!(app.pending_images.contains(&(ImageKind::CharacterPortrait, 42)));
      assert_eq!(app.pending_images.len(), 1);
    }

    #[tokio::test]
    async fn it_does_not_redispatch_a_key_already_in_flight() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.pending_images.insert((ImageKind::CorporationLogo, 7));

      let _task = dispatch_image_fetches(&mut app, vec![(ImageKind::CorporationLogo, 7)]);

      assert_eq!(app.pending_images.len(), 1);
    }

    #[test]
    fn it_clears_the_pending_key_when_an_image_resolves() {
      let mut app = test_app();
      app.pending_images.insert((ImageKind::CharacterPortrait, 42));

      let _task = handle_image_ready(&mut app, ImageKind::CharacterPortrait, 42, false);

      assert!(app.pending_images.is_empty());
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
      assert_eq!(Route::CorporationDetail(1).name(), "CorporationDetail");
      assert_eq!(Route::Skills(1).name(), "Skills");
      assert_eq!(Route::Mail.name(), "Mail");
      assert_eq!(Route::Wallet.name(), "Wallet");
      assert_eq!(Route::Assets.name(), "Assets");
      assert_eq!(Route::Settings.name(), "Settings");
    }
  }

  mod engine_lifecycle {
    use pretty_assertions::assert_eq;

    use super::*;

    fn holder() -> HolderInfo {
      HolderInfo {
        hostname: "nebula".to_owned(),
        last_active: DateTime::from_timestamp(1_700_000_000, 0).expect("valid timestamp"),
        machine_id: "machine-other".to_owned(),
      }
    }

    fn in_progress_stats() -> JobStats {
      JobStats {
        active: 1,
        attention: 0,
        done: 0,
        errors: 0,
        total: 3,
      }
    }

    #[tokio::test]
    async fn it_ignores_a_manual_restart_when_the_engine_is_not_stopped() {
      let mut app = test_app();
      let (runtime, mut restart_rx) = test_runtime_with_restart().await;
      app.runtime = Some(runtime);
      app.engine_state = EngineState::Running;

      let _ = update(&mut app, Message::RestartSync);

      assert_eq!(
        restart_rx.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty),
        "restart is a stopped-only escape hatch; a running engine is never re-signalled"
      );
    }

    #[test]
    fn it_keeps_a_parked_engine_read_only_when_a_stray_give_up_arrives() {
      let mut app = test_app();
      app.engine_state = read_only_engine_state(Some(holder()));

      let _ = update(
        &mut app,
        Message::Sync(sync::Event::GaveUp {
          reason: "irrelevant".to_owned(),
        }),
      );

      assert_eq!(
        app.engine_state,
        EngineState::ReadOnly {
          held_by: Some(holder()),
        }
      );
    }

    #[test]
    fn it_keeps_a_parked_engine_read_only_when_the_inert_stream_closes() {
      let mut app = test_app();
      app.engine_state = read_only_engine_state(Some(holder()));

      let _ = update(
        &mut app,
        Message::EngineStopped {
          reason: None,
        },
      );

      assert_eq!(
        app.engine_state,
        EngineState::ReadOnly {
          held_by: Some(holder()),
        }
      );
    }

    #[test]
    fn it_reports_syncing_while_the_engine_runs_with_active_jobs() {
      let stats = in_progress_stats();

      assert!(super::syncing_with(&EngineState::Running, &stats));
    }

    #[test]
    fn it_rests_at_idle_when_the_engine_is_alive_and_jobs_are_settled() {
      let settled = JobStats {
        active: 0,
        attention: 0,
        done: 3,
        errors: 0,
        total: 3,
      };

      assert!(!super::syncing_with(&EngineState::Running, &settled));
    }

    #[test]
    fn it_returns_to_running_when_a_respawn_arrives_after_a_stop() {
      let mut app = test_app();
      app.character_manager = Some(character_manager::State::new());
      app.engine_state = EngineState::Stopped {
        reason: Some("gave up".to_owned()),
      };

      let _ = update(
        &mut app,
        Message::Sync(sync::Event::Restarted {
          attempt: 0,
        }),
      );

      assert_eq!(
        app.engine_state,
        EngineState::Idle,
        "a respawn with no in-flight jobs leaves the engine alive but idle, no longer stopped"
      );
      assert_eq!(super::chip_lifecycle(&app), sync_chip::Lifecycle::Active);
    }

    #[test]
    fn it_settles_a_running_engine_to_idle_when_jobs_finish() {
      let mut app = test_app();
      app.character_manager = Some(character_manager::State::new());
      app.engine_state = EngineState::Running;

      let event = sync::Event::Finished {
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(1)),
        outcome: sync::Outcome::synced(),
      };
      let _ = update(&mut app, Message::Sync(event));

      assert_eq!(app.engine_state, EngineState::Idle);
      assert_eq!(super::chip_lifecycle(&app), sync_chip::Lifecycle::Active);
    }

    #[tokio::test]
    async fn it_signals_the_supervisor_to_restart_on_a_manual_restart() {
      let mut app = test_app();
      let (runtime, mut restart_rx) = test_runtime_with_restart().await;
      app.runtime = Some(runtime);
      app.engine_state = EngineState::Stopped {
        reason: Some("gave up".to_owned()),
      };

      let _ = update(&mut app, Message::RestartSync);

      assert_eq!(
        restart_rx.try_recv(),
        Ok(()),
        "a manual restart reaches the parked supervisor over its dedicated channel"
      );
      assert_eq!(
        app.engine_state,
        EngineState::Running,
        "the chip optimistically shows Running once the restart is dispatched"
      );
    }

    #[test]
    fn it_stops_reporting_syncing_once_the_engine_terminates() {
      let mut app = test_app();
      app.engine_state = EngineState::Running;

      let _ = update(
        &mut app,
        Message::EngineStopped {
          reason: Some("the channel closed".to_owned()),
        },
      );

      assert_eq!(
        app.engine_state,
        EngineState::Stopped {
          reason: Some("the channel closed".to_owned()),
        }
      );
      assert!(!super::syncing_with(&app.engine_state, &in_progress_stats()));
      assert_eq!(super::chip_lifecycle(&app), sync_chip::Lifecycle::Stopped);
    }

    #[test]
    fn it_stops_with_a_meaningful_reason_when_the_supervisor_gives_up() {
      let mut app = test_app();
      app.engine_state = EngineState::Running;

      let _ = update(
        &mut app,
        Message::Sync(sync::Event::GaveUp {
          reason: "the sync engine died 5 times in quick succession; auto-restart gave up".to_owned(),
        }),
      );

      assert_eq!(
        app.engine_state,
        EngineState::Stopped {
          reason: Some("the sync engine died 5 times in quick succession; auto-restart gave up".to_owned()),
        }
      );
      assert_eq!(super::chip_lifecycle(&app), sync_chip::Lifecycle::Stopped);
    }

    #[tokio::test]
    async fn it_yields_a_read_only_chip_at_a_parked_boot() {
      let db = store::open_test().await.expect("test db");
      let mut app = test_app();
      let ready = StoreReady {
        db: db.clone(),
        http: http::Client::builder(http::Cache::new(db.clone())).build(),
        lease: Some(holder()),
        settings: config::Settings::default(),
        sync_db: db.clone(),
        sync_housekeeping_db: db,
        sync_session: None,
      };

      let _ = handle_store_opened(&mut app, ready);

      assert_eq!(
        app.engine_state,
        EngineState::ReadOnly {
          held_by: Some(holder()),
        }
      );
      assert!(!super::syncing_with(&app.engine_state, &in_progress_stats()));
      assert_eq!(
        super::chip_lifecycle(&app),
        sync_chip::Lifecycle::ReadOnly {
          hostname: Some("nebula".to_owned()),
        }
      );
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
      assert_eq!(Message::CloseSyncPopover.variant_name(), "CloseSyncPopover");
      assert_eq!(
        Message::EngineStopped {
          reason: None
        }
        .variant_name(),
        "EngineStopped"
      );
      assert_eq!(Message::FocusMainWindow.variant_name(), "FocusMainWindow");
      assert_eq!(
        Message::ImageReady {
          id: 1,
          kind: store::images::ImageKind::CharacterPortrait,
          ready: true,
        }
        .variant_name(),
        "ImageReady"
      );
      assert_eq!(Message::InitFailed("boom".to_owned()).variant_name(), "InitFailed");
      assert_eq!(Message::LeaseHeartbeat.variant_name(), "LeaseHeartbeat");
      assert_eq!(Message::OpenAbout.variant_name(), "OpenAbout");
      assert_eq!(Message::PeriodicPull.variant_name(), "PeriodicPull");
      assert_eq!(Message::PeriodicPush.variant_name(), "PeriodicPush");
      assert_eq!(Message::Pulled(false).variant_name(), "Pulled");
      assert_eq!(Message::Pushed(None).variant_name(), "Pushed");
      assert_eq!(
        Message::SeedProgress(splash::seed::Progress::Complete).variant_name(),
        "SeedProgress"
      );
      assert_eq!(Message::Splash(splash::Message::Tick).variant_name(), "Splash");
      assert_eq!(
        Message::SyncNowResolved(SyncNowOutcome::Failed).variant_name(),
        "SyncNowResolved"
      );
      assert_eq!(Message::ReauthCharacter(1).variant_name(), "ReauthCharacter");
      assert_eq!(Message::RestartSync.variant_name(), "RestartSync");
      assert_eq!(Message::SnoozesWoken(Vec::new()).variant_name(), "SnoozesWoken");
      assert_eq!(Message::StorageMigrated.variant_name(), "StorageMigrated");
      assert_eq!(Message::SyncPulse.variant_name(), "SyncPulse");
      assert_eq!(Message::TakeOver.variant_name(), "TakeOver");
      assert_eq!(Message::CancelTakeOver.variant_name(), "CancelTakeOver");
      assert_eq!(Message::ConfirmTakeOver.variant_name(), "ConfirmTakeOver");
      assert_eq!(
        Message::TakeOverResolved(TakeOverOutcome::Failed).variant_name(),
        "TakeOverResolved"
      );
      assert_eq!(Message::ToggleSyncPopover.variant_name(), "ToggleSyncPopover");
      assert_eq!(
        Message::UpdaterAction(updater_banner::Action::Apply).variant_name(),
        "UpdaterAction"
      );
      assert_eq!(Message::UpdaterDismissToast.variant_name(), "UpdaterDismissToast");
      assert_eq!(
        Message::UpdaterStateChanged(updater::State::default()).variant_name(),
        "UpdaterStateChanged"
      );
      assert_eq!(
        Message::WindowOpened(window::Id::unique()).variant_name(),
        "WindowOpened"
      );
      assert_eq!(Message::MailUnreadCounted(3).variant_name(), "MailUnreadCounted");
      assert_eq!(
        Message::CalendarAttentionCounted(3).variant_name(),
        "CalendarAttentionCounted"
      );
    }
  }

  fn featured_app() -> App {
    let mut app = test_app();
    app.assets = Some(assets::State::new());
    app.calendar = Some(calendar::State::new(42, app.now, config::FeatureFlags::default()));
    app.character_detail = Some(character_detail::State::new(1, &[]));
    app.character_manager = Some(character_manager::State::new());
    app.mail = Some(mail::State::new(42));
    app.skills = Some(skills::State::new(1));
    app.wallet = Some(wallet::State::new());
    app
  }

  mod compare_seeds {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_leads_with_the_active_pilot_then_fills_by_descending_sp() {
      let seeds = compare_seeds(vec![(1, 100), (2, 500), (3, 300)], Some(1));

      assert_eq!(seeds, vec![1, 2, 3]);
    }

    #[test]
    fn it_caps_the_selection_at_three_pilots() {
      let seeds = compare_seeds(vec![(1, 10), (2, 20), (3, 30), (4, 40)], None);

      assert_eq!(seeds, vec![4, 3, 2]);
    }

    #[test]
    fn it_ignores_an_active_pilot_absent_from_the_cards() {
      let seeds = compare_seeds(vec![(1, 10), (2, 20)], Some(99));

      assert_eq!(seeds, vec![2, 1]);
    }

    #[test]
    fn it_breaks_skill_point_ties_by_id() {
      let seeds = compare_seeds(vec![(5, 100), (3, 100)], None);

      assert_eq!(seeds, vec![3, 5]);
    }
  }

  mod compare_seed_ids {
    use super::*;

    #[test]
    fn it_returns_no_seeds_without_a_character_manager() {
      let app = test_app();

      assert!(super::super::compare_seed_ids(&app).is_empty());
    }
  }

  mod image_url {
    use super::*;

    #[tokio::test]
    async fn it_builds_a_url_for_every_image_kind() {
      use store::images::ImageKind;
      let runtime = test_runtime().await;
      let client = runtime.eve_image.as_ref();

      assert!(!super::super::image_url(client, ImageKind::AllianceLogo, 1).is_empty());
      assert!(!super::super::image_url(client, ImageKind::CharacterPortrait, 1).is_empty());
      assert!(!super::super::image_url(client, ImageKind::CorporationLogo, 1).is_empty());
    }
  }

  mod image_reload {
    use super::*;

    #[tokio::test]
    async fn it_is_a_no_op_without_a_runtime() {
      let app = featured_app();

      let _ = super::super::image_reload(&app);
    }

    #[tokio::test]
    async fn it_batches_a_reload_for_each_active_route() {
      let mut app = featured_app();
      app.runtime = Some(test_runtime().await);

      for route in [
        Route::Assets,
        Route::Calendar,
        Route::CharacterDetail(1),
        Route::Characters,
        Route::Industry,
        Route::Mail,
        Route::Settings,
        Route::Skills(1),
        Route::Wallet,
      ] {
        app.route = route;
        let _ = super::super::image_reload(&app);
      }
    }

    #[tokio::test]
    async fn it_reloads_the_compare_window_when_one_is_open() {
      let mut app = featured_app();
      app.runtime = Some(test_runtime().await);
      app.compare = Some((window::Id::unique(), skills_compare::State::new(vec![1, 2], Vec::new())));

      let _ = super::super::image_reload(&app);
    }
  }

  mod handle_skills {
    use super::*;

    #[tokio::test]
    async fn it_dispatches_each_skills_branch() {
      use crate::features::skills::EditorSeed;
      let mut app = featured_app();
      app.runtime = Some(test_runtime().await);

      let _ = super::super::handle_skills(&mut app, skills::Message::CharacterChanged(1));
      let _ = super::super::handle_skills(&mut app, skills::Message::OpenCompare);
      let _ = super::super::handle_skills(&mut app, skills::Message::OpenPlanEditor(EditorSeed::New));
      let _ = super::super::handle_skills(&mut app, skills::Message::PaneSettled("skills", 280.0));
      let _ = super::super::handle_skills(&mut app, skills::Message::PickerToggled);
    }

    #[tokio::test]
    async fn it_is_a_no_op_without_a_runtime() {
      let mut app = test_app();

      let _ = super::super::handle_skills(&mut app, skills::Message::PickerToggled);
    }
  }

  mod dispatch_lifecycle {
    use super::*;

    #[tokio::test]
    async fn it_routes_each_lifecycle_message() {
      let mut app = featured_app();

      let messages = vec![
        Message::CancelTakeOver,
        Message::ClockTick,
        Message::CloseSyncPopover,
        Message::ConfirmTakeOver,
        Message::FocusMainWindow,
        Message::ImageReady {
          id: 1,
          kind: store::images::ImageKind::CharacterPortrait,
          ready: true,
        },
        Message::InitFailed("boom".to_owned()),
        Message::LeaseHeartbeat,
        Message::LockReleased,
        Message::OpenAbout,
        Message::PeriodicPush,
        Message::Pushed(None),
        Message::ReauthCharacter(1),
        Message::SeedProgress(splash::seed::Progress::Step("seeding".to_owned())),
        Message::SnoozesWoken(Vec::new()),
        Message::Splash(splash::Message::Tick),
        Message::StorageMigrated,
        Message::SyncPulse,
        Message::TakeOver,
        Message::TakeOverResolved(TakeOverOutcome::Failed),
        Message::ToggleSyncPopover,
        Message::UpdaterAction(updater_banner::Action::Apply),
        Message::UpdaterDismissToast,
        Message::UpdaterStateChanged(updater::State::default()),
        Message::WindowOpened(window::Id::unique()),
        Message::Wallet(wallet::Message::PickerToggled),
      ];

      for message in messages {
        let _ = super::super::dispatch_lifecycle(&mut app, message);
      }
    }
  }

  mod init_tracing {
    use super::*;

    #[test]
    fn it_initializes_a_file_logger_under_a_writable_dir() {
      let dir = tempfile::tempdir().expect("temp dir");

      let guard = init_tracing(dir.path(), config::LogLevel::default());

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

  mod crash_visibility {
    use std::sync::{Arc, Mutex};

    use tracing::{Event, Subscriber, field::Visit};
    use tracing_subscriber::{
      Layer,
      filter::EnvFilter,
      layer::{Context, SubscriberExt as _},
      registry,
    };

    use super::*;

    /// Collects the `message` field of every captured event into a shared buffer so a test can
    /// assert what was logged through tracing.
    #[derive(Clone, Default)]
    struct CaptureLayer {
      messages: Arc<Mutex<Vec<String>>>,
    }

    struct MessageVisitor<'a>(&'a mut Option<String>);

    impl Visit for MessageVisitor<'_> {
      fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
          *self.0 = Some(format!("{value:?}"));
        }
      }
    }

    impl<S: Subscriber> Layer<S> for CaptureLayer {
      fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut message = None;
        event.record(&mut MessageVisitor(&mut message));
        if let Some(message) = message {
          self.messages.lock().expect("capture buffer").push(message);
        }
      }
    }

    #[test]
    fn it_routes_a_panic_through_the_hook_into_tracing() {
      let layer = CaptureLayer::default();
      let messages = layer.messages.clone();

      // Install a hook that drives the same `log_panic` path the production hook uses, scoped to a
      // capturing subscriber, then restore the previous hook so the test harness is unaffected.
      let previous = std::panic::take_hook();
      std::panic::set_hook(Box::new(log_panic));

      tracing::subscriber::with_default(registry().with(layer), || {
        // A panic raised from a sync-style closure, caught so it does not abort the test.
        let _ = std::panic::catch_unwind(|| {
          fn run_sync_job() {
            panic!("simulated sync engine crash");
          }
          run_sync_job();
        });
      });

      std::panic::set_hook(previous);

      let captured = messages.lock().expect("capture buffer");
      assert!(
        captured.iter().any(|m| m.contains("the process panicked")),
        "the panic hook routed an ERROR event into tracing; captured: {captured:?}",
      );
    }

    #[test]
    fn it_pins_sqlx_query_logging_to_warn_or_higher() {
      // Build the real file filter and route events through it so a regression that loosens
      // `sqlx::query` to DEBUG/TRACE fails this test instead of silently flooding the field log.
      let captured = |level: tracing::Level| -> bool {
        let layer = CaptureLayer::default();
        let messages = layer.messages.clone();
        let filtered = layer.with_filter(EnvFilter::new(file_filter(config::LogLevel::default())));
        tracing::subscriber::with_default(registry().with(filtered), || match level {
          tracing::Level::TRACE => tracing::trace!(target: "sqlx::query", "stmt"),
          tracing::Level::DEBUG => tracing::debug!(target: "sqlx::query", "stmt"),
          tracing::Level::INFO => tracing::info!(target: "sqlx::query", "stmt"),
          tracing::Level::WARN => tracing::warn!(target: "sqlx::query", "stmt"),
          tracing::Level::ERROR => tracing::error!(target: "sqlx::query", "stmt"),
        });
        !messages.lock().expect("capture buffer").is_empty()
      };

      assert!(
        !captured(tracing::Level::TRACE),
        "sqlx::query TRACE must be filtered out"
      );
      assert!(
        !captured(tracing::Level::DEBUG),
        "sqlx::query DEBUG must be filtered out"
      );
      assert!(!captured(tracing::Level::INFO), "sqlx::query INFO must be filtered out");
      assert!(
        captured(tracing::Level::WARN),
        "sqlx::query WARN must pass (filter pins WARN-or-higher)"
      );
    }

    // Routes the events emitted by `emit` through the real file filter for `log_level` and reports
    // whether any survived. Mirrors the live wiring: the file layer that ships to disk wraps exactly
    // `file_filter(log_level)`. The `tracing` macros bake their target into a static callsite, so the
    // caller passes a closure that emits at a literal target rather than a runtime `&str`.
    fn passes_file_filter(log_level: config::LogLevel, emit: impl FnOnce()) -> bool {
      let layer = CaptureLayer::default();
      let messages = layer.messages.clone();
      let filtered = layer.with_filter(EnvFilter::new(file_filter(log_level)));
      tracing::subscriber::with_default(registry().with(filtered), emit);
      !messages.lock().expect("capture buffer").is_empty()
    }

    #[test]
    fn it_filters_pod_debug_out_at_quiet() {
      assert!(
        !passes_file_filter(config::LogLevel::Quiet, || {
          tracing::debug!(target: "pod::sync::engine", "event")
        }),
        "Quiet pins pod to INFO, so pod DEBUG must be filtered out"
      );

      assert!(
        passes_file_filter(config::LogLevel::Quiet, || {
          tracing::info!(target: "pod::sync::engine", "event")
        }),
        "Quiet must still admit pod INFO"
      );
    }

    #[test]
    fn it_hides_the_demoted_resolve_site_until_verbose() {
      // The chronically noisy resolve cache-hit site was demoted to TRACE so it only surfaces at
      // Verbose; its target is the module path it logs from.
      let emit = || tracing::trace!(target: "pod::sync::jobs::resolve", "resolved item type from db");

      assert!(
        !passes_file_filter(config::LogLevel::Quiet, emit),
        "the resolve cache-hit site must be silent at Quiet"
      );
      assert!(
        !passes_file_filter(config::LogLevel::Normal, emit),
        "the resolve cache-hit site must stay silent at Normal so the demotion keeps real signal afloat"
      );
      assert!(
        passes_file_filter(config::LogLevel::Verbose, emit),
        "the resolve cache-hit site must surface at Verbose for a deep-dive repro"
      );
    }

    #[test]
    fn it_hides_the_demoted_http_site_until_verbose() {
      // The chronically noisy http per-request site was demoted to TRACE so it only surfaces at
      // Verbose; it logs under the dedicated `pod::http` target.
      let emit = || tracing::trace!(target: "pod::http", "request completed");

      assert!(
        !passes_file_filter(config::LogLevel::Quiet, emit),
        "the http per-request site must be silent at Quiet"
      );
      assert!(
        !passes_file_filter(config::LogLevel::Normal, emit),
        "the http per-request site must stay silent at Normal so the demotion keeps real signal afloat"
      );
      assert!(
        passes_file_filter(config::LogLevel::Verbose, emit),
        "the http per-request site must surface at Verbose for a deep-dive repro"
      );
    }
  }
}
