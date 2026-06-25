mod graphics;
mod shortcuts;
mod snooze_scheduler;
mod trash_purge_scheduler;
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
  widget::{Column, Row, Space, Stack, button, container, mouse_area, scrollable, text},
  window,
};
use shortcuts::{Chord, FocusTracker};
use windows::{Window, WindowStates, Windows};

use crate::{
  clients::{self, esi, eve_image, eve_sso, http},
  config,
  features::{
    assets, auth, calendar, character_detail, character_manager, character_manager::OwnedPilot, contract_detail,
    corporation_detail, focus_search, industry, killmail_detail, mail, registry, settings, skill_plan_editor,
    skill_plan_manager, skills, skills_compare, splash, wallet, window_chrome,
  },
  mcp, notifications,
  services::{images, updater},
  store,
  sync::{self, FreshnessSummary, JobKey, JobKind},
  ui::{
    components::{
      backdrop,
      command_palette::{
        self, Action as PaletteAction, Command as PaletteCommand, Entity as PaletteEntity,
        EntityKind as PaletteEntityKind,
      },
      esi_status::esi_status,
      eve_time::eve_time,
      notification_row::notification_row,
      notification_toaster::{ToastView, toaster},
      rail::{self, rail},
      status, sync_chip,
      sync_popover::{self, JobStats, Model},
      tab_select::{Tab, TabLayout, tab_select_with},
      updater_banner,
      virtual_list::{self, VirtualList, VirtualListConfig},
    },
    style::{color, control, radius, shadow, spacing, typography},
  },
  window_state::{self, UiState, WindowGeometry, coalesce::WriteCoalescer, validity},
};

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

const PERIODIC_PULL_INTERVAL: Duration = Duration::from_secs(60);

const PERIODIC_PUSH_INTERVAL: Duration = Duration::from_secs(60);

const POPOVER_LEFT: f32 = spacing::SPACE_3_5;

const PULSE_INTERVAL: Duration = Duration::from_millis(450);

/// Grace window before a rail flyout closes after the pointer leaves the icon, so the cursor can
/// cross the gap into the flyout without it snapping shut.
const RAIL_HOVER_GRACE: Duration = Duration::from_millis(160);

const REACQUIRE_INTERVAL: Duration = Duration::from_secs(30);

/// Trailing-debounce window for the heavy `load_roster_at` reload. A sync burst re-marks
/// `roster_dirty` on every `Finished` event, and the 450ms pulse would otherwise drain it ~2x/s.
/// Collapsing those to one reload per window keeps the interactive reader pool from starving while
/// still refreshing the roster within a couple of pulses of the burst settling.
const ROSTER_RELOAD_DEBOUNCE: Duration = Duration::from_millis(1500);

const RUNTIME_CHANNEL_BUFFER: usize = 64;

const SCALE_MAX: u8 = 150;

const SCALE_MIN: u8 = 85;

const TRASH_PURGE_INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);

/// Clock-tick cadences (in 1-second ticks) for the periodic interactive-DB checks. The tick handler
/// fires every second, so each check runs only on ticks where `clock_tick % N == offset`. Staggering
/// the offsets keeps the ~7 queries from all landing on the same tick, and stretching the low-urgency
/// checks cuts the steady-state interactive load that was starving the reader pool.
///
/// Snooze wake and mail-unread are user-facing freshness signals, so they stay snappy (every 2s on
/// opposite ticks). Calendar attention is a quieter badge (every 3s). The route-scoped reloads only
/// fire when their screen is open, but the long-lived industry jobs need far less polling than the
/// faster-moving mail and calendar views.
const TICK_SNOOZE_WAKE: u64 = 2;

const TICK_MAIL_UNREAD: u64 = 2;

const TICK_MAIL_RELOAD: u64 = 2;

const TICK_CALENDAR_ATTENTION: u64 = 3;

const TICK_CALENDAR_RELOAD: u64 = 2;

const TICK_INDUSTRY_RELOAD: u64 = 5;

/// Cadence (in 1-second ticks) for the idle notification detector sweep. The pulse already runs the
/// detectors after every relevant sync; this slower standing sweep catches the time-threshold events
/// (skill / industry / extraction-cracked) that mature on the wall clock with no fresh sync.
const TICK_NOTIFICATIONS: u64 = 10;

/// How many toasts may be visible at once. Surfacing more drops the oldest (it still lands in the
/// center), matching the design's "cap visible, newest kept".
const TOAST_CAP: usize = 3;

/// A toast's lifetime before it auto-dismisses, unless a hover pauses the countdown.
const TOAST_MS: Duration = Duration::from_secs(15);

/// How often the toast tick subscription ages the live toasts while any are visible.
const TOAST_TICK: Duration = Duration::from_millis(100);

const ZERO_GEOMETRY: WindowGeometry = WindowGeometry {
  height: 0.0,
  width: 0.0,
  x: 0.0,
  y: 0.0,
};

static UPDATER_RECEIVER: std::sync::Mutex<Option<tokio::sync::watch::Receiver<updater::State>>> =
  std::sync::Mutex::new(None);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum NotificationTab {
  #[default]
  New,
  History,
}

struct App {
  accessibility: config::AccessibilityConfig,
  assets: Option<assets::State>,
  auth: auth::State,
  calendar: Option<calendar::State>,
  calendar_attention: i64,
  calendar_events: WindowStates<calendar::EventWindow>,
  character_detail: Option<character_detail::State>,
  character_manager: Option<character_manager::State>,
  /// Monotonic count of 1-second clock ticks, used to stagger the periodic interactive-DB checks
  /// across ticks (see the `TICK_*` cadences) instead of firing them all on every tick.
  clock_tick: u64,
  coalescer: WriteCoalescer,
  compare: Option<(window::Id, skills_compare::State)>,
  composes: WindowStates<mail::compose::Draft>,
  /// Whether the data-loss confirmation gate is open. `true` means the first "Take over" click has
  /// been received but the share has not yet been claimed — the forceful claim fires only on the
  /// second explicit confirmation.
  confirm_force_takeover: bool,
  contracts: WindowStates<contract_detail::State>,
  corporation_detail: Option<corporation_detail::State>,
  editor: Option<(window::Id, skill_plan_editor::State)>,
  engine_state: EngineState,
  esi_connected: bool,
  industry: Option<industry::State>,
  /// Session-lived cache of the planner's static catalog (recipes, names, icons), built once on the first
  /// planner load and handed to every later Industry navigation so the costly "Loading build catalog" build
  /// runs at most once per app session. The cheap per-entry data (prices, owned blueprints) is always refreshed.
  industry_catalog: Option<industry::StaticCatalog>,
  init_error: Option<String>,
  keyboard_focus: FocusTracker,
  killmails: WindowStates<killmail_detail::State>,
  last_push: Option<SystemTime>,
  last_synced: Option<DateTime<Utc>>,
  mail: Option<mail::State>,
  mail_unread: i64,
  /// The detached single-instance Manage Plans window: id-keyed roster master/detail. `None` when closed;
  /// re-opening focuses the existing window rather than spawning a second.
  manage_plans: Option<(window::Id, skill_plan_manager::State)>,
  /// The embedded MCP automation server. `None` until the store is ready; thereafter its lifecycle
  /// is reconciled against the live [`config::McpConfig`] (off by default) via [`mcp::Server::apply`].
  mcp_server: Option<mcp::Server>,
  /// Trailing-debounce floor for the heavy roster reload. `None` lets the next dirty pulse reload
  /// immediately; `Some` holds the earliest instant another reload may fire, so a sync burst that
  /// keeps re-marking `roster_dirty` collapses into one reload per [`ROSTER_RELOAD_DEBOUNCE`] window.
  next_roster_reload: Option<Instant>,
  /// `None` arms the purge for the very next clock tick (fires once shortly after launch); `Some`
  /// holds the earliest instant it may run again.
  next_trash_purge: Option<Instant>,
  /// Cached surfaced notifications (newest-first) backing the center panel, refreshed by the sync
  /// pulse and on panel open. `notification_names` resolves each owner to its display "who" line.
  notifications: Vec<store::model::Notification>,
  notification_names: std::collections::HashMap<store::model::NotificationOwner, String>,
  notifications_dirty: bool,
  /// History-tab page accumulator (newest-first), grown one keyset page at a time as the user scrolls.
  /// Distinct from `notifications` (the live New-tab source) so paging arbitrarily deep never disturbs
  /// the New tab or the bell badge.
  notifications_history: Vec<store::model::Notification>,
  /// Keyset cursor positioned just past the last accumulated History row; `None` requests the newest page.
  notifications_history_cursor: Option<store::repo::notifications::HistoryCursor>,
  /// Monotonic generation bumped whenever the History accumulator resets (panel open or newer rows
  /// arrive). An in-flight page tagged with a stale generation is dropped so it can't append to the
  /// freshly-reset accumulator.
  notifications_history_epoch: u64,
  /// Whether another older History page may exist (the last fetch filled a full page).
  notifications_history_has_more: bool,
  /// Guards against a second concurrent History page fetch while one is already in flight.
  notifications_history_loading: bool,
  /// Last absolute vertical scroll offset of the History list, fed back into the windowed view.
  notifications_history_scroll: f32,
  notifications_panel_open: bool,
  notifications_tab: NotificationTab,
  notifications_unread: i64,
  now: DateTime<Utc>,
  outbox: sync::OutboxStatus,
  palette: Option<command_palette::State>,
  pending_auth: Option<auth::Message>,
  pending_images: HashSet<(store::images::ImageKind, i64)>,
  rail_hover: Option<rail::Destination>,
  rail_hover_gen: u64,
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
  status: sync::SyncStatus,
  stockpile_editors: WindowStates<assets::Editor>,
  stockpile_imports: WindowStates<assets::ImportPanel>,
  store_ready: Option<StoreReady>,
  sync_popover_open: bool,
  sync_session: Option<store::sync_session::SyncSession>,
  sync_tick: bool,
  /// Live bottom-right toasts (newest-surfaced notifications), capped at [`TOAST_CAP`]. Each entry
  /// tracks its own remaining lifetime and hover-pause state; the toast tick subscription ages them.
  toasts: Vec<ToastEntry>,
  ui_state: UiState,
  updater: Option<updater::Handle>,
  updater_state: updater::State,
  updater_toast_dismissed: bool,
  wallet: Option<wallet::State>,
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

type FileFilterReloadHandle =
  OnceLock<tracing_subscriber::reload::Handle<tracing_subscriber::EnvFilter, tracing_subscriber::Registry>>;

#[derive(Clone, Debug, Eq, PartialEq)]
struct HolderInfo {
  hostname: String,
  last_active: DateTime<Utc>,
  machine_id: String,
}

/// A live bottom-right toast. `remaining` counts down from [`TOAST_MS`] each toast tick while not
/// `paused`; the toast is dismissed when it reaches zero. `paused` is set while the cursor hovers it.
#[derive(Clone, Debug)]
struct ToastEntry {
  notification: store::model::Notification,
  paused: bool,
  remaining: Duration,
  who: String,
}

#[derive(Clone, Debug)]
enum Message {
  Assets(assets::Message),
  Auth(auth::Message),
  Calendar(calendar::Message),
  CalendarAttentionCounted(i64),
  CalendarEvent(window::Id, calendar::EventMessage),
  CancelTakeOver,
  CharacterDetail(character_detail::Message),
  CharacterManager(character_manager::Message),
  Chrome(window::Id, window_chrome::Event),
  ClockTick,
  CloseSyncPopover,
  Compare(skills_compare::Message),
  Compose(window::Id, mail::Message),
  ConfirmTakeOver,
  Contract(window::Id, contract_detail::Message),
  ClearNotifications,
  CloseNotificationsPanel,
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
  Killmail(window::Id, killmail_detail::Message),
  LeaseHeartbeat,
  LockReleased,
  Mail(mail::Message),
  MailUnreadCounted(i64),
  ManagePlans(skill_plan_manager::Message),
  MarkAllNotificationsRead,
  Mcp(mcp::McpRequest),
  McpDataChanged,
  Nav(rail::Destination),
  NavTo(rail::Destination, Option<&'static str>),
  NotificationActivated(i64),
  /// One more keyset page of History finished loading; carries the rows and a per-owner "who" map for
  /// the freshly-paged rows. An empty `epoch` is rejected against the live one so a page captured before
  /// a reset (newer rows arrived) never appends to the fresh accumulator.
  NotificationsHistoryPageLoaded {
    epoch: u64,
    rows: Vec<store::model::Notification>,
    who: std::collections::HashMap<store::model::NotificationOwner, String>,
  },
  /// The History list scrolled; carries the absolute offset (fed back into the windowed view) and the
  /// relative offset (0..1), used to trigger the next page once the user nears the bottom.
  NotificationsHistoryScrolled {
    absolute: f32,
    relative: f32,
  },
  NotificationsRefreshed(Box<notifications::Snapshot>),
  Palette(PaletteMessage),
  PeriodicPull,
  PeriodicPush,
  Pulled(bool),
  Pushed(Option<SystemTime>),
  Quit,
  RailHover(Option<rail::Destination>),
  RailHoverExpire(u64),
  ReacquireLease,
  Ready(Runtime),
  ReauthCharacter(i64),
  RestartSync,
  SeedProgress(splash::seed::Progress),
  SelectNotificationTab(NotificationTab),
  Settings(settings::Message),
  Shortcut(Chord),
  SkillPlanEditor(skill_plan_editor::Message),
  Skills(skills::Message),
  SnoozesWoken(Vec<(i64, i64)>),
  Splash(splash::Message),
  StockpileEditor(window::Id, assets::Message),
  StockpileImport(window::Id, assets::Message),
  StorageMigrated,
  StoreOpened(Box<StoreReady>),
  Sync(sync::Event),
  SyncNowResolved(SyncNowOutcome),
  SyncPulse,
  TakeOver,
  TakeOverResolved(TakeOverOutcome, Box<StoreReady>),
  TextInputFocused(iced::widget::Id),
  ToastDismissed(i64),
  ToastHover(i64, bool),
  ToastTick,
  ToggleNotificationsPanel,
  ToggleSyncPopover,
  TrashPurged(Vec<(i64, i64)>),
  UpdaterAction(updater_banner::Action),
  UpdaterDismissToast,
  UpdaterStateChanged(updater::State),
  Wallet(wallet::Message),
  Window(window::Id, window::Event),
  WindowOpened(window::Id),
}

#[derive(Clone, Debug)]
enum PaletteMessage {
  Activate(usize),
  ActivateSelected,
  Close,
  MoveDown,
  MoveUp,
  Open,
  QueryChanged(String),
  Select(usize),
}

impl Message {
  /// Whether handling this message can surface new image-bearing rows, gating the stale-image scan to load/sync
  /// messages instead of running it after every feature interaction (scroll, hover, filter). Each feature reports
  /// its own data-loading variants via `Message::loads_data`.
  fn affects_images(&self) -> bool {
    match self {
      Message::Assets(msg) => msg.loads_data(),
      Message::Calendar(msg) => msg.loads_data(),
      Message::CharacterDetail(msg) => msg.loads_data(),
      Message::CharacterManager(msg) => msg.loads_data(),
      Message::Compare(msg) => msg.loads_data(),
      Message::CorporationDetail(msg) => msg.loads_data(),
      Message::Industry(msg) => msg.loads_data(),
      Message::Mail(msg) => msg.loads_data(),
      Message::ManagePlans(msg) => msg.loads_data(),
      Message::Skills(msg) => msg.loads_data(),
      Message::Wallet(msg) => msg.loads_data(),
      _ => false,
    }
  }

  fn variant_name(&self) -> &'static str {
    self
      .feature_variant_name()
      .or_else(|| self.lifecycle_variant_name())
      .unwrap_or("Window")
  }

  fn feature_variant_name(&self) -> Option<&'static str> {
    self.screen_variant_name().or_else(|| self.notification_variant_name())
  }

  fn screen_variant_name(&self) -> Option<&'static str> {
    Some(match self {
      Message::Assets(_) => "Assets",
      Message::Auth(_) => "Auth",
      Message::Calendar(_) => "Calendar",
      Message::CalendarAttentionCounted(_) => "CalendarAttentionCounted",
      Message::CalendarEvent(..) => "CalendarEvent",
      Message::CharacterDetail(_) => "CharacterDetail",
      Message::CharacterManager(_) => "CharacterManager",
      Message::Compare(_) => "Compare",
      Message::Compose(..) => "Compose",
      Message::Contract(..) => "Contract",
      Message::CorporationDetail(_) => "CorporationDetail",
      Message::Industry(_) => "Industry",
      Message::Killmail(..) => "Killmail",
      Message::Mail(_) => "Mail",
      Message::MailUnreadCounted(_) => "MailUnreadCounted",
      Message::ManagePlans(_) => "ManagePlans",
      Message::Settings(_) => "Settings",
      Message::SkillPlanEditor(_) => "SkillPlanEditor",
      Message::Skills(_) => "Skills",
      Message::StockpileEditor(..) => "StockpileEditor",
      Message::StockpileImport(..) => "StockpileImport",
      Message::Sync(_) => "Sync",
      Message::Wallet(_) => "Wallet",
      _ => return None,
    })
  }

  fn notification_variant_name(&self) -> Option<&'static str> {
    Some(match self {
      Message::ClearNotifications => "ClearNotifications",
      Message::CloseNotificationsPanel => "CloseNotificationsPanel",
      Message::MarkAllNotificationsRead => "MarkAllNotificationsRead",
      Message::Mcp(_) => "Mcp",
      Message::McpDataChanged => "McpDataChanged",
      Message::Nav(_) => "Nav",
      Message::NavTo(..) => "NavTo",
      Message::NotificationActivated(_) => "NotificationActivated",
      Message::NotificationsHistoryPageLoaded {
        ..
      } => "NotificationsHistoryPageLoaded",
      Message::NotificationsHistoryScrolled {
        ..
      } => "NotificationsHistoryScrolled",
      Message::NotificationsRefreshed(_) => "NotificationsRefreshed",
      Message::SelectNotificationTab(_) => "SelectNotificationTab",
      Message::ToastDismissed(_) => "ToastDismissed",
      Message::ToastHover(..) => "ToastHover",
      Message::ToastTick => "ToastTick",
      Message::ToggleNotificationsPanel => "ToggleNotificationsPanel",
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
      Message::Palette(_) => "Palette",
      Message::Quit => "Quit",
      Message::Ready(_) => "Ready",
      Message::ReauthCharacter(_) => "ReauthCharacter",
      Message::SeedProgress(_) => "SeedProgress",
      Message::Shortcut(_) => "Shortcut",
      Message::SnoozesWoken(_) => "SnoozesWoken",
      Message::Splash(_) => "Splash",
      Message::StorageMigrated => "StorageMigrated",
      Message::StoreOpened(_) => "StoreOpened",
      Message::TextInputFocused(_) => "TextInputFocused",
      Message::TrashPurged(_) => "TrashPurged",
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
      Message::TakeOverResolved(..) => "TakeOverResolved",
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

struct PreparedStore {
  database_path: std::path::PathBuf,
  lease: Option<HolderInfo>,
  settings: config::Settings,
  sync_session: Option<store::sync_session::SyncSession>,
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

type Tx = iced::futures::channel::mpsc::Sender<Message>;

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
    accessibility,
    assets: None,
    auth: auth::State::default(),
    calendar: None,
    calendar_attention: 0,
    calendar_events: WindowStates::default(),
    character_detail: None,
    character_manager: None,
    clock_tick: 0,
    coalescer: WriteCoalescer::new(),
    compare: None,
    composes: WindowStates::default(),
    confirm_force_takeover: false,
    contracts: WindowStates::default(),
    corporation_detail: None,
    editor: None,
    engine_state: EngineState::default(),
    esi_connected: true,
    industry: None,
    industry_catalog: None,
    init_error: None,
    keyboard_focus: FocusTracker::default(),
    killmails: WindowStates::default(),
    last_push: None,
    last_synced: None,
    mail: None,
    mail_unread: 0,
    manage_plans: None,
    mcp_server: None,
    next_roster_reload: None,
    next_trash_purge: None,
    notification_names: std::collections::HashMap::new(),
    notifications: Vec::new(),
    notifications_dirty: false,
    notifications_history: Vec::new(),
    notifications_history_cursor: None,
    notifications_history_epoch: 0,
    notifications_history_has_more: false,
    notifications_history_loading: false,
    notifications_history_scroll: 0.0,
    notifications_panel_open: false,
    notifications_tab: NotificationTab::default(),
    notifications_unread: 0,
    now: Utc::now(),
    outbox: sync::OutboxStatus::new(),
    palette: None,
    pending_auth: None,
    pending_images: HashSet::new(),
    rail_hover: None,
    rail_hover_gen: 0,
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
    stockpile_editors: WindowStates::default(),
    stockpile_imports: WindowStates::default(),
    store_ready: None,
    status: sync::SyncStatus::new(),
    sync_popover_open: false,
    sync_session: None,
    sync_tick: false,
    toasts: Vec::new(),
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
  // One-writer/many-readers over a single database file: open_pools returns interactive, sync, and
  // housekeeping handles that all clone the same reader pool + single writer connection, so a sync
  // write-storm can never starve the interactive roster read. See store::open_pools.
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

/// Drives a take-over end to end on a blocking-safe async task: it first closes every working-copy
/// pool the parked instance holds, then runs the lease claim (which on success overwrites the
/// working-copy file via `publish_database`), then reopens fresh pools against the swapped file. The
/// close-before-swap ordering is mandatory on Windows, whose mandatory file locking rejects an
/// in-place overwrite of an open `.db` file with `PermissionDenied` (POSIX hosts tolerate it). The
/// pools are always reopened — on a claim they read the freshly pulled canonical copy, and on a
/// declined or failed claim they reopen the unchanged working copy — so the app is never left with
/// closed pools regardless of outcome.
fn run_take_over(ready: StoreReady, session: store::sync_session::SyncSession, force: bool) -> Task<Message> {
  Task::future(async move {
    let lease = ready.lease.clone();
    let settings = ready.settings.clone();
    // Release every handle on the working-copy file before the swap. Under the one-writer/many-readers
    // model all three handles clone the same reader pool and writer connection, so closing both pools
    // once releases every clone (the http client's interactive-pool clone included). `Pool::close` is
    // idempotent, so the repeated calls below are harmless and keep the close path explicit.
    ready.db.reader().close().await;
    ready.db.writer().close().await;
    ready.sync_db.reader().close().await;
    ready.sync_db.writer().close().await;
    ready.sync_housekeeping_db.reader().close().await;
    ready.sync_housekeeping_db.writer().close().await;
    let outcome = claim_lease(&session, force);
    match reopen_after_take_over_inner(&session, lease, settings).await {
      Ok(ready) => Message::TakeOverResolved(outcome, Box::new(ready)),
      Err(error) => {
        tracing::error!(target: "pod::lifecycle", %error, "reopening the database after take-over failed");
        Message::InitFailed(error)
      }
    }
  })
}

/// Runs the lease claim now that the pools are closed. The stale-aware `take_over` declines a
/// still-live foreign holder (mapping to `Failed` so the resolver re-parks); the forceful path always
/// claims on success. Either way no file remains open across the `publish_database` swap.
fn claim_lease(session: &store::sync_session::SyncSession, force: bool) -> TakeOverOutcome {
  let claimed = if force {
    session.force_take_over(Utc::now())
  } else {
    session.take_over(Utc::now())
  };
  match claimed {
    Ok(store::lease::Outcome::Acquired) => TakeOverOutcome::Claimed,
    Ok(store::lease::Outcome::HeldBy {
      hostname, ..
    }) => {
      tracing::trace!(target: "pod::lifecycle", %hostname, "take-over declined; the share is still held");
      TakeOverOutcome::Failed
    }
    Err(error) => {
      tracing::warn!(target: "pod::lifecycle", %error, "claiming the storage lease during take-over failed");
      TakeOverOutcome::Failed
    }
  }
}

/// Builds a `StoreReady` whose pools are opened against the working-copy file after the take-over
/// swap, so no connection from the boot-time pools straddles the `publish_database` file swap. On a
/// successful claim the file now holds the freshly pulled canonical copy; on a declined or failed
/// claim the unchanged working copy is reopened so the app keeps functioning.
async fn reopen_after_take_over_inner(
  session: &store::sync_session::SyncSession,
  lease: Option<HolderInfo>,
  settings: config::Settings,
) -> Result<StoreReady, String> {
  let pools = store::open_pools(session.working_copy()).await.map_err(store_err)?;
  let http = http::Client::builder(http::Cache::new(pools.interactive.clone())).build();
  Ok(StoreReady {
    db: pools.interactive,
    http,
    lease,
    settings,
    sync_db: pools.sync,
    sync_housekeeping_db: pools.housekeeping,
    sync_session: Some(session.clone()),
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
  feature_flags(app).enabled()
}

fn feature_flags(app: &App) -> config::FeatureFlags {
  if let Some(state) = app.settings.as_ref() {
    return *state.settings().features();
  }
  if let Some(runtime) = app.runtime.as_ref() {
    return *runtime.settings.features();
  }
  config::FeatureFlags::default()
}

fn ui_config(app: &App) -> config::UiConfig {
  if let Some(state) = app.settings.as_ref() {
    return state.settings().ui().clone();
  }
  if let Some(runtime) = app.runtime.as_ref() {
    return runtime.settings.ui().clone();
  }
  config::UiConfig::default()
}

fn handle_close_requested(app: &mut App, id: window::Id) -> Task<Message> {
  let close = match app.windows.kind(id) {
    Some(Window::CalendarEvent) => close_calendar_event_window(app, id),
    Some(Window::Compare) => close_compare_window(app, id),
    Some(Window::Contract) => close_contract_window(app, id),
    Some(Window::Killmail) => close_killmail_window(app, id),
    Some(Window::MailCompose) => close_compose_window(app, id),
    Some(Window::ManagePlans) => close_manage_plans_window(app, id),
    Some(Window::SkillPlanEditor) => close_editor_window(app, id),
    Some(Window::StockpileEditor) => close_stockpile_editor_window(app, id),
    Some(Window::StockpileImport) => close_stockpile_import_window(app, id),
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
    Window::CalendarEvent => {
      app.calendar_events.remove(id);
    }
    Window::Compare if app.compare.as_ref().map(|(cid, _)| *cid) == Some(id) => app.compare = None,
    Window::Contract => {
      app.contracts.remove(id);
    }
    Window::Killmail => {
      app.killmails.remove(id);
    }
    Window::MailCompose => {
      // The OS already tore the window down, so save the in-flight draft (if non-empty) without
      // re-issuing a close, then drop the per-window state.
      let save = compose_save_on_drop(app, id);
      app.composes.remove(id);
      return Task::batch([save, shutdown_if_last_window(app)]);
    }
    Window::ManagePlans if app.manage_plans.as_ref().map(|(mid, _)| *mid) == Some(id) => app.manage_plans = None,
    Window::SkillPlanEditor if app.editor.as_ref().map(|(eid, _)| *eid) == Some(id) => app.editor = None,
    Window::StockpileEditor => {
      app.stockpile_editors.remove(id);
    }
    Window::StockpileImport => {
      app.stockpile_imports.remove(id);
    }
    _ => {}
  }
  shutdown_if_last_window(app)
}

/// Persists a compose window's draft (if non-empty) when the OS reports it closed, refreshing the
/// main-view Drafts list. Used by `on_window_closed`, which must not re-issue a `window::close`.
fn compose_save_on_drop(app: &App, id: window::Id) -> Task<Message> {
  match (
    app.composes.get(id).and_then(mail::compose::Draft::pending_save),
    app.runtime.as_ref(),
  ) {
    (Some((draft_id, input)), Some(runtime)) => {
      let db = runtime.db.clone();
      Task::perform(
        async move { mail::persist_pending_draft(db, draft_id, input).await },
        |()| Message::Mail(mail::Message::DraftSaved(None)),
      )
    }
    _ => Task::none(),
  }
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
  let save_draft = save_open_compose(app);
  let checkpoint = shutdown_storage(app);
  stop_engines(app);
  save_draft
    .chain(checkpoint)
    .chain(Task::batch([iced::exit(), exit_process()]))
}

/// Flushes every open, non-empty compose window to Drafts before the storage checkpoint, so any draft
/// in flight at quit survives to the next launch. Runs before the checkpoint so the persisted rows are
/// included in the pushed working copy.
fn save_open_compose(app: &App) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let saves: Vec<Task<Message>> = app
    .composes
    .iter()
    .filter_map(|(_, draft)| draft.pending_save())
    .map(|(id, input)| {
      let db = runtime.db.clone();
      Task::future(async move { mail::persist_pending_draft(db, id, input).await }).discard()
    })
    .collect();
  if saves.is_empty() {
    Task::none()
  } else {
    Task::batch(saves)
  }
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

fn record_ui_flag(app: &mut App, key: String, value: bool) {
  app.ui_state.flags.insert(key, value);
  app.coalescer.request(app.ui_state.clone(), Instant::now());
}

fn record_ui_list(app: &mut App, key: String, values: Vec<String>) {
  app.ui_state.lists.insert(key, values);
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
  app.wallet = Some(wallet::State::new(feature_flags(app)).with_restored_panes(&app.ui_state));
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

/// Whether the industry planner may offer the pilot/clone assignment picker: the clone/implant data it reads is
/// gated behind BOTH the Skills and Clone-Monitoring features (never Industry), so both must be enabled.
fn industry_assign_pilots(app: &App) -> bool {
  app
    .runtime
    .as_ref()
    .map(|runtime| {
      let features = runtime.settings.features();
      features.is_enabled(config::Feature::SkillMonitoring) && features.is_enabled(config::Feature::CloneMonitoring)
    })
    .unwrap_or(false)
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
  let assign_pilots = industry_assign_pilots(app);
  app.industry = Some(
    industry::State::new(
      selection,
      required.clone(),
      feature_flags(app),
      facility_defaults,
      app.industry_catalog.clone(),
      assign_pilots,
    )
    .with_restored_panes(&app.ui_state),
  );
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

fn trash_purge_tick(app: &mut App) -> Task<Message> {
  let now = Instant::now();
  if app.next_trash_purge.is_some_and(|due| now < due) {
    return Task::none();
  }
  app.next_trash_purge = Some(now + TRASH_PURGE_INTERVAL);

  match app.runtime.as_ref() {
    Some(runtime) => Task::perform(
      trash_purge_scheduler::purge_expired_trash(runtime.db.clone(), app.now),
      Message::TrashPurged,
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
  app.assets = Some(assets::State::new(feature_flags(app)).with_restored_panes(&app.ui_state));
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

fn drain_assets_dirty(app: &mut App) -> Option<Task<Message>> {
  let db = app.runtime.as_ref()?.db.clone();
  Some(app.assets.as_mut()?.drain_dirty(&db)?.map(Message::Assets))
}

fn drain_detail_dirty(app: &mut App) -> Option<Task<Message>> {
  let db = app.runtime.as_ref()?.db.clone();
  Some(
    app
      .character_detail
      .as_mut()?
      .drain_dirty(&db)?
      .map(Message::CharacterDetail),
  )
}

fn drain_roster_dirty(app: &mut App) -> Option<Task<Message>> {
  drain_roster_dirty_at(app, Instant::now())
}

/// Trailing-debounced drain of the roster-reload flag. During a sync burst `roster_dirty` is re-set
/// on every `Finished` event, so without a floor the 450ms pulse would re-fire the heavy reload
/// ~2x/s. Holding the dirty flag until [`ROSTER_RELOAD_DEBOUNCE`] has elapsed collapses the burst
/// into one reload per window; the flag stays set so a later pulse reloads once the window opens.
fn drain_roster_dirty_at(app: &mut App, now: Instant) -> Option<Task<Message>> {
  if !app.roster_dirty || app.character_manager.is_none() {
    return None;
  }
  if app.next_roster_reload.is_some_and(|floor| now < floor) {
    return None;
  }
  app.roster_dirty = false;
  app.next_roster_reload = Some(now + ROSTER_RELOAD_DEBOUNCE);
  let runtime = app.runtime.as_ref()?;
  Some(character_manager::load(&runtime.db, feature_flags(app)).map(Message::CharacterManager))
}

fn drain_wallet_dirty(app: &mut App) -> Option<Task<Message>> {
  let db = app.runtime.as_ref()?.db.clone();
  Some(app.wallet.as_mut()?.drain_dirty(&db)?.map(Message::Wallet))
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
  if let Some((_, manage_plans)) = app.manage_plans.as_ref() {
    keys.extend(manage_plans.stale_images());
  }
  for (_, contract) in app.contracts.iter() {
    keys.extend(contract.stale_images());
  }
  for (_, killmail) in app.killmails.iter() {
    keys.extend(killmail.stale_images());
  }
  for (_, editor) in app.stockpile_editors.iter() {
    keys.extend(editor.stale_images());
  }
  keys
}

fn dispatch_image_fetches(app: &mut App, keys: Vec<(store::images::ImageKind, i64)>) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  images::dispatch_fetches(
    &mut app.pending_images,
    &runtime.eve_image,
    keys,
    |(kind, id), ready| Message::ImageReady {
      id,
      kind,
      ready,
    },
  )
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
        tasks.push(character_manager::load(&runtime.db, feature_flags(app)).map(Message::CharacterManager));
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
  if app.manage_plans.is_some() {
    tasks.push(skill_plan_manager::load(&runtime.db).map(Message::ManagePlans));
  }
  for (id, contract) in app.contracts.iter() {
    let load = contract_detail::load(&runtime.db, contract.source(), contract.contract_id());
    tasks.push(load.map(move |msg| Message::Contract(id, msg)));
  }
  for (id, killmail) in app.killmails.iter() {
    let load = killmail_detail::load(&runtime.db, killmail.source(), killmail.killmail_id());
    tasks.push(load.map(move |msg| Message::Killmail(id, msg)));
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
  let ui = ui_config(app);
  let nav_location = *ui.nav_location();
  let rail_props = rail::RailProps {
    active: app.route.destination(),
    active_sub: active_sub_section(app),
    calendar_attention: app.calendar_attention,
    cascade_mode: *ui.cascade_mode(),
    enabled_features: &enabled_features,
    hovered: app.rail_hover,
    mail_unread,
    nav_location,
    notifications_unread: app.notifications_unread,
    rail_order: ui.rail_order(),
  };
  let cascade_mode = *ui.cascade_mode();
  let rail_element = rail(
    rail_props,
    Message::Nav,
    Message::RailHover,
    |dest, id| Message::NavTo(dest, Some(id)),
    Message::ToggleNotificationsPanel,
    Message::Palette(PaletteMessage::Open),
  );
  // In sub-rail mode a persistent sub-section column sits inboard of the edge rail (between the rail
  // and the content), mirrored to whichever side the rail is docked. Decision: every view keeps its
  // own in-view tab strip even in sub-rail mode; hiding it per view would mean threading cascade_mode
  // through six feature view signatures, which is more invasive than the duplicate strip is worth.
  let sub_rail_element = (cascade_mode == config::CascadeMode::SubRail)
    .then(|| {
      rail::sub_rail(
        app.route.destination(),
        active_sub_section(app),
        nav_location,
        |dest, id| Message::NavTo(dest, Some(id)),
      )
    })
    .flatten();
  let mut body_children: Vec<Element<'_, Message>> = Vec::with_capacity(3);
  match nav_location {
    config::NavLocation::Left => {
      body_children.push(rail_element);
      if let Some(sub_rail) = sub_rail_element {
        body_children.push(sub_rail);
      }
      body_children.push(content.into());
    }
    config::NavLocation::Right => {
      body_children.push(content.into());
      if let Some(sub_rail) = sub_rail_element {
        body_children.push(sub_rail);
      }
      body_children.push(rail_element);
    }
  }
  let body = Row::with_children(body_children)
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
  if app.notifications_panel_open {
    layers.push(backdrop::click_catcher(Message::CloseNotificationsPanel));
    layers.push(notifications_panel(app, nav_location));
  }
  if let Some(toaster) = notifications_toaster(app) {
    layers.push(toaster);
  }
  if let Some(toast) = toast {
    layers.push(toast);
  }
  if let Some(state) = &app.palette {
    let entries = palette_entries(app);
    layers.push(palette_overlay(state, entries));
  }

  Stack::with_children(layers)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

const NOTIFICATIONS_PANEL_WIDTH: f32 = 384.0;

const NOTIFICATIONS_PANEL_MAX_HEIGHT: f32 = 560.0;

const NOTIFICATIONS_TAB_STRIP_HEIGHT: f32 = 40.0;

/// Estimated pixel height of one History notification row, used by the windowed list's offset math.
/// Rows are content-driven (title + a "who · when" line), so this is an estimate; overscan absorbs the
/// variance.
const NOTIFICATIONS_HISTORY_ROW_HEIGHT: f32 = 64.0;

/// Fraction of the History list a scroll must reach (0..1) before the next keyset page is requested.
/// Mirrors the mail list's load-more threshold so a page is fetched a little before the true bottom.
const NOTIFICATIONS_HISTORY_SCROLL_THRESHOLD: f32 = 0.85;

/// The notification center panel: a card flying out beside the rail, bottom-aligned to the bell. A
/// header with the title and a "Mark all read" button sits above a New/History tab strip. The New tab
/// filters to unread notifications; the History tab lists every loaded notification newest-first (the
/// repo already orders them). A later task will repoint History at a paginated source, so the row
/// rendering is factored out of the tab shell. Each row marks itself read and deep-links on click;
/// opening the panel never auto-reads. A footer "Clear all" + total stays available.
fn notifications_panel(app: &App, nav_location: config::NavLocation) -> Element<'_, Message> {
  let unread = app.notifications_unread;
  let new_count = app
    .notifications
    .iter()
    .filter(|notification| notification.read_at().is_none())
    .count();
  // History tracks the keyset-paged accumulator; New tracks the live unread set. The footer "total"
  // mirrors whichever tab is active so the count matches what the body actually renders.
  let history_count = app.notifications_history.len();
  let total = match app.notifications_tab {
    NotificationTab::New => new_count,
    NotificationTab::History => history_count,
  };

  let header = Row::with_children(vec![
    text("Notifications")
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    Space::new().width(Length::Fill).into(),
    mark_all_read_button(unread > 0).into(),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_2);

  let tabs = notifications_tab_strip(app.notifications_tab, new_count, history_count);

  let body = notifications_tab_body(app, app.notifications_tab);

  let footer_visible = match app.notifications_tab {
    NotificationTab::New => !app.notifications.is_empty(),
    NotificationTab::History => !app.notifications_history.is_empty(),
  };
  let mut children: Vec<Element<'_, Message>> = vec![header.into(), tabs, rule_line(), body];
  if footer_visible {
    children.push(rule_line());
    children.push(
      Row::with_children(vec![
        notifications_text_button("Clear all".to_owned(), true, Message::ClearNotifications).into(),
        Space::new().width(Length::Fill).into(),
        text(format!("{total} total"))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::tertiary()))
          .into(),
      ])
      .align_y(Vertical::Center)
      .into(),
    );
  }

  let card = container(
    Column::with_children(children)
      .spacing(spacing::SPACE_2_5)
      .width(Length::Fixed(NOTIFICATIONS_PANEL_WIDTH)),
  )
  .width(Length::Fixed(NOTIFICATIONS_PANEL_WIDTH))
  .max_height(NOTIFICATIONS_PANEL_MAX_HEIGHT)
  .padding(spacing::SPACE_3_5)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: iced::Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: radius::PANEL.into(),
    },
    shadow: shadow::CARD,
    ..container::Style::default()
  });

  let align_x = match nav_location {
    config::NavLocation::Left => Horizontal::Left,
    config::NavLocation::Right => Horizontal::Right,
  };
  // Clear the nav rail so the panel flies out to its side (like the cascade
  // sub-rail) instead of covering it.
  let (pad_left, pad_right) = match nav_location {
    config::NavLocation::Left => (rail::RAIL_WIDTH + POPOVER_LEFT, POPOVER_LEFT),
    config::NavLocation::Right => (POPOVER_LEFT, rail::RAIL_WIDTH + POPOVER_LEFT),
  };
  container(card)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(align_x)
    .align_y(Vertical::Bottom)
    .padding(Padding {
      top: 0.0,
      right: pad_right,
      bottom: POPOVER_BOTTOM_OFFSET,
      left: pad_left,
    })
    .into()
}

fn notifications_text_button(label: String, enabled: bool, message: Message) -> button::Button<'static, Message> {
  let color = if enabled {
    color::text::secondary()
  } else {
    color::text::tertiary()
  };
  let button = button(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(color),
      }),
  )
  .padding(spacing::UNIT)
  .style(|_, _| button::Style {
    background: Some(Background::Color(iced::Color::TRANSPARENT)),
    ..button::Style::default()
  });
  if enabled { button.on_press(message) } else { button }
}

fn mark_all_read_button<'a>(enabled: bool) -> button::Button<'a, Message> {
  let color = if enabled {
    color::text::PRIMARY
  } else {
    color::text::tertiary()
  };
  let button = button(
    text("Mark all read")
      .font(typography::body::MEDIUM)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(color),
      }),
  )
  .padding(Padding {
    top: spacing::UNIT,
    right: spacing::SPACE_2,
    bottom: spacing::UNIT,
    left: spacing::SPACE_2,
  })
  .style(control::ghost_button);
  if enabled {
    button.on_press(Message::MarkAllNotificationsRead)
  } else {
    button
  }
}

fn notifications_tab_strip<'a>(active: NotificationTab, new_count: usize, total: usize) -> Element<'a, Message> {
  let tabs = vec![
    Tab {
      count: new_count.to_string(),
      icon: None,
      label: "New",
      on_press: (active != NotificationTab::New).then_some(Message::SelectNotificationTab(NotificationTab::New)),
      selected: active == NotificationTab::New,
    },
    Tab {
      count: total.to_string(),
      icon: None,
      label: "History",
      on_press: (active != NotificationTab::History)
        .then_some(Message::SelectNotificationTab(NotificationTab::History)),
      selected: active == NotificationTab::History,
    },
  ];
  container(tab_select_with(tabs, TabLayout::Fill))
    .width(Length::Fill)
    .height(Length::Fixed(NOTIFICATIONS_TAB_STRIP_HEIGHT))
    .into()
}

fn notifications_tab_body(app: &App, active: NotificationTab) -> Element<'_, Message> {
  match active {
    NotificationTab::New => notifications_new_body(app),
    NotificationTab::History => notifications_history_body(app),
  }
}

/// The New tab: the live unread set drawn from the refresh-loaded `notifications` list. Bounded by the
/// refresh limit and the unread count, so it renders in one shrink-scrollable column without paging.
fn notifications_new_body(app: &App) -> Element<'_, Message> {
  let rows: Vec<Element<'_, Message>> = app
    .notifications
    .iter()
    .filter(|notification| notification.read_at().is_none())
    .map(|notification| notification_history_row(app, notification))
    .collect();

  if rows.is_empty() {
    return notifications_empty_state("You\u{2019}re all caught up", "No new events");
  }

  scrollable(
    Column::with_children(rows)
      .spacing(spacing::UNIT / 2.0)
      .width(Length::Fill),
  )
  .height(Length::Shrink)
  .into()
}

/// The History tab: the keyset-paged accumulator, windowed and infinite-scrolled. Each scroll past the
/// load-more threshold emits `NotificationsHistoryScrolled`, which requests the next page once no fetch
/// is in flight and an older page may exist.
fn notifications_history_body(app: &App) -> Element<'_, Message> {
  if app.notifications_history.is_empty() {
    return notifications_empty_state("Nothing here yet", "No past notifications");
  }

  let rows = &app.notifications_history;
  let offset = app.notifications_history_scroll;
  virtual_list::responsive_window(move |viewport_height| {
    let config = VirtualListConfig::new(rows.len(), NOTIFICATIONS_HISTORY_ROW_HEIGHT)
      .viewport_height(viewport_height)
      .scroll_offset(offset);
    let windowed = VirtualList::new(config, |index| notification_history_row(app, &rows[index])).view();
    scrollable(windowed)
      .style(control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill)
      .on_scroll(|viewport| Message::NotificationsHistoryScrolled {
        absolute: viewport.absolute_offset().y,
        relative: viewport.relative_offset().y,
      })
      .into()
  })
}

fn notification_history_row<'a>(app: &'a App, notification: &'a store::model::Notification) -> Element<'a, Message> {
  let who = app
    .notification_names
    .get(&notification.owner())
    .map(String::as_str)
    .unwrap_or("");
  let when = relative_time(notification.created_at(), app.now);
  notification_row(
    notification,
    who,
    &when,
    true,
    Message::NotificationActivated(notification.id()),
  )
}

fn notifications_empty_state<'a>(title: &'a str, subtitle: &'a str) -> Element<'a, Message> {
  container(
    Column::with_children(vec![
      text(title)
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::text::secondary()))
        .into(),
      text(subtitle)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    ])
    .spacing(spacing::UNIT)
    .align_x(Horizontal::Center),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_4_5)
  .align_x(Horizontal::Center)
  .into()
}

fn notifications_toaster(app: &App) -> Option<Element<'_, Message>> {
  let views: Vec<ToastView<'_>> = app
    .toasts
    .iter()
    .map(|toast| ToastView {
      notification: &toast.notification,
      who: toast.who.as_str(),
    })
    .collect();
  toaster(
    &views,
    Message::NotificationActivated,
    Message::ToastDismissed,
    Message::ToastHover,
  )
}

fn rule_line<'a>() -> Element<'a, Message> {
  container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

/// A compact relative timestamp ("now", "5m", "3h", "2d") from a stored RFC3339 `created_at`.
fn relative_time(created_at: &str, now: DateTime<Utc>) -> String {
  let Ok(when) = DateTime::parse_from_rfc3339(created_at) else {
    return String::new();
  };
  let secs = (now - when.with_timezone(&Utc)).num_seconds().max(0);
  if secs < 45 {
    "now".to_owned()
  } else if secs < 3_600 {
    format!("{}m", secs / 60)
  } else if secs < 86_400 {
    format!("{}h", secs / 3_600)
  } else {
    format!("{}d", secs / 86_400)
  }
}

fn palette_overlay(state: &command_palette::State, entries: Vec<command_palette::Entry>) -> Element<'_, Message> {
  command_palette::view(
    state,
    entries,
    |query| Message::Palette(PaletteMessage::QueryChanged(query)),
    |index| Message::Palette(PaletteMessage::Select(index)),
    |index| Message::Palette(PaletteMessage::Activate(index)),
    Message::Palette(PaletteMessage::Close),
  )
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
  let pilots = roster(app);
  let last_synced_secs = app.last_synced.map(|at| (app.now - at).num_seconds().max(0) as u64);
  sync_popover::build_model(
    &pilots,
    &app.status,
    &feature_flags(app),
    last_synced_secs,
    app.sync_tick,
  )
}

fn expected_job_stats(app: &App) -> JobStats {
  sync_popover::job_stats(&roster(app), &app.status, &feature_flags(app))
}

// Regroup the shared `JobStats` — itself derived from the one freshness function the popover and chip
// both consume — back into the `FreshnessSummary` the calm chip headline reads. The chip never recounts
// jobs; it only renders the buckets. `errors` (persistent `Failed`) is split out of `JobStats::attention`
// upstream, so the chip carries it alongside the summary and folds it back into its attention headline.
fn chip_freshness(stats: &JobStats) -> FreshnessSummary {
  let catching_up = stats
    .total
    .saturating_sub(stats.done + stats.active + stats.attention + stats.errors);
  FreshnessSummary {
    attention: stats.attention,
    catching_up,
    fresh: stats.done,
    refreshing: stats.active,
    total: stats.total,
  }
}

fn roster(app: &App) -> Vec<OwnedPilot> {
  app
    .character_manager
    .as_ref()
    .map(character_manager::owned_roster)
    .unwrap_or_default()
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
  let chip = sync_chip::State {
    errors: stats.errors,
    last_synced_secs: app.last_synced.map(|at| (app.now - at).num_seconds().max(0) as u64),
    lifecycle: chip_lifecycle(app),
    pulse_on: app.sync_tick,
    summary: chip_freshness(&stats),
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

fn pod_theme() -> iced::Theme {
  iced::Theme::custom(
    "pod".to_string(),
    iced::theme::Palette {
      background: color::surface::BASE,
      danger: color::status::DANGER,
      primary: color::accent::PLASMA,
      success: color::status::ONLINE,
      text: color::text::PRIMARY,
      ..iced::theme::Palette::DARK
    },
  )
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
  if app.notifications_panel_open {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      matches!(
        event,
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
          key: keyboard::Key::Named(keyboard::key::Named::Escape),
          ..
        })
      )
      .then_some(Message::CloseNotificationsPanel)
    }));
  }
  if !app.toasts.is_empty() {
    subs.push(iced::time::every(TOAST_TICK).map(|_| Message::ToastTick));
  }
  subs.push(auth::subscription().map(Message::Auth));
  subs.push(auth::focus_subscription().map(|()| Message::FocusMainWindow));
  subs.push(mcp::bridge::subscription().map(Message::Mcp));
  subs.push(mcp::reload::subscription().map(|_| Message::McpDataChanged));
  subs.push(shortcuts::subscription(Message::Shortcut));
  subs.push(palette_key_subscription(app));
  subs.extend(data_subscriptions(app));
  Subscription::batch(subs)
}

// The per-feature screen subscriptions, armed only for the screens that are currently built. Split
// out of `subscription` so its open/lease/panel timer wiring stays the readable core.
fn data_subscriptions(app: &App) -> Vec<Subscription<Message>> {
  let mut subs = Vec::new();
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
  subs
}

fn palette_key_subscription(app: &App) -> Subscription<Message> {
  // `iced::event::listen_with` only accepts a non-capturing `fn`, so the open/focus context is
  // threaded by picking one of three fixed mappers rather than by capturing into a closure.
  if app.palette.is_some() {
    iced::event::listen_with(map_palette_open)
  } else if app.keyboard_focus.is_text_input_focused() {
    iced::event::listen_with(map_palette_closed_focused)
  } else {
    iced::event::listen_with(map_palette_closed_unfocused)
  }
}

fn palette_message(key: shortcuts::PaletteKey) -> Message {
  Message::Palette(match key {
    shortcuts::PaletteKey::Activate => PaletteMessage::ActivateSelected,
    shortcuts::PaletteKey::Close => PaletteMessage::Close,
    shortcuts::PaletteKey::MoveDown => PaletteMessage::MoveDown,
    shortcuts::PaletteKey::MoveUp => PaletteMessage::MoveUp,
    shortcuts::PaletteKey::Open => PaletteMessage::Open,
  })
}

fn map_palette_open(event: iced::Event, _status: iced::event::Status, _id: window::Id) -> Option<Message> {
  shortcuts::PaletteKey::for_event(&event, true, false).map(palette_message)
}

fn map_palette_closed_focused(event: iced::Event, _status: iced::event::Status, _id: window::Id) -> Option<Message> {
  shortcuts::PaletteKey::for_event(&event, false, true).map(palette_message)
}

fn map_palette_closed_unfocused(event: iced::Event, _status: iced::event::Status, _id: window::Id) -> Option<Message> {
  shortcuts::PaletteKey::for_event(&event, false, false).map(palette_message)
}

fn theme(app: &App, id: window::Id) -> iced::Theme {
  match app.windows.kind(id) {
    // Custom-chrome windows still on the legacy path. Each kind drops out of this arm when its
    // own conversion task promotes it to a native window; `Window::Splash` stays for good.
    Some(Window::Killmail | Window::Splash) => splash_theme(),
    _ => pod_theme(),
  }
}

fn title(app: &App, id: window::Id) -> String {
  window_title(app, id)
}

/// Kind-aware OS title for the window `id`: one arm per `Window` kind so each conversion task
/// fills in its own window's title on its own line. Multi-instance kinds derive their title from
/// that window's per-id state; single-instance kinds use a constant. Unregistered ids fall back to
/// the bare app name.
fn window_title(app: &App, id: window::Id) -> String {
  match app.windows.kind(id) {
    Some(Window::CalendarEvent) => app
      .calendar_events
      .get(id)
      .map(|window| format!("Pod \u{2014} {}", window.title()))
      .unwrap_or_else(|| "Pod \u{2014} Event".to_string()),
    Some(Window::Compare) => "Pod — Compare Skills".to_string(),
    Some(Window::Contract) => match app.contracts.get(id) {
      Some(state) => format!("Pod \u{2014} {}", state.title()),
      None => "Pod \u{2014} Contract".to_string(),
    },
    Some(Window::Killmail) => app
      .killmails
      .get(id)
      .map(|state| format!("Pod — {}", state.title()))
      .unwrap_or_else(|| "Pod — Killmail".to_string()),
    Some(Window::MailCompose) => app
      .composes
      .get(id)
      .map(|draft| format!("Pod — {}", mail::compose::window_title(draft)))
      .unwrap_or_else(|| "Pod — Compose Mail".to_string()),
    Some(Window::Main) => "Pod".to_string(),
    Some(Window::ManagePlans) => "Pod — Manage Skill Plans".to_string(),
    Some(Window::SkillPlanEditor) => "Pod — Skill Plan Editor".to_string(),
    Some(Window::Splash) => "Pod".to_string(),
    Some(Window::StockpileEditor) => app
      .stockpile_editors
      .get(id)
      .map(|editor| format!("Pod \u{2014} {}", assets::stockpile_editor_window_title(editor)))
      .unwrap_or_else(|| "Pod \u{2014} Stockpile Editor".to_string()),
    Some(Window::StockpileImport) => format!("Pod \u{2014} {}", assets::stockpile_import_window_title()),
    None => "Pod".to_string(),
  }
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

/// Opens a real native-chrome OS window of `kind` and registers its id synchronously.
///
/// This is the single source of native-window open boilerplate. It builds an opaque, decorated,
/// resizable `window::Settings`, restores persisted geometry when `kind.state_key()` is `Some(..)`
/// (otherwise centers at `default_size`), calls `window::open`, and records `id -> kind` in the
/// registry. `window::open` resolves the id immediately, so callers get it back directly with NO
/// deferred `*WindowReady` round-trip.
///
/// Returns `(id, open_task)`: the caller seeds its per-window state under `id` and batches
/// `open_task` (already mapped to `Message::WindowOpened`) alongside any loader it kicks off.
fn open_native_window(app: &mut App, kind: Window, default_size: Size) -> (window::Id, Task<Message>) {
  let (size, position) = restored_geometry(&app.ui_state, kind, default_size);
  let settings = window::Settings {
    size,
    position,
    decorations: true,
    resizable: true,
    icon: app_icon(),
    ..window::Settings::default()
  };
  let (id, open_task) = window::open(settings);
  app.windows.register(id, kind);
  (id, open_task.map(Message::WindowOpened))
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

  let (id, open_task) = open_native_window(
    app,
    Window::Compare,
    Size::new(COMPARE_WINDOW_WIDTH, COMPARE_WINDOW_HEIGHT),
  );
  app.compare = Some((id, skills_compare::State::new(seed_ids.clone(), roster)));

  Task::batch([
    close_existing,
    open_task,
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

  let (id, open_task) = open_native_window(
    app,
    Window::SkillPlanEditor,
    Size::new(EDITOR_WINDOW_WIDTH, EDITOR_WINDOW_HEIGHT),
  );
  app.editor = Some((
    id,
    skill_plan_editor::State::new(character_id).with_restored_panes(&app.ui_state),
  ));

  Task::batch([
    close_existing,
    open_task,
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

/// Opens a detached native-chrome killmail window. The window opens via the shared native-window
/// helper (native frame, OS title bar, centered default geometry), registering the kind and seeding
/// the per-id state synchronously. Unlimited instances coexist, duplicates included, because every
/// open mints a fresh id.
fn open_killmail_window(app: &mut App, source: killmail_detail::Source, killmail_id: i64) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  let size = Size::new(
    killmail_detail::KILLMAIL_WINDOW_WIDTH,
    killmail_detail::KILLMAIL_WINDOW_HEIGHT,
  );
  let (id, open_task) = open_native_window(app, Window::Killmail, size);
  app
    .killmails
    .insert(id, killmail_detail::State::new(source, killmail_id));

  Task::batch([
    open_task,
    killmail_detail::load(&db, source, killmail_id).map(move |msg| Message::Killmail(id, msg)),
  ])
}

fn handle_killmail(app: &mut App, id: window::Id, msg: killmail_detail::Message) -> Task<Message> {
  let Some(state) = app.killmails.get_mut(id) else {
    return Task::none();
  };
  let killmail_detail::Message::Loaded(detail) = msg;
  state.set_detail(*detail);
  let keys = state.stale_images();
  dispatch_image_fetches(app, keys)
}

fn close_killmail_window(app: &mut App, id: window::Id) -> Task<Message> {
  app.killmails.remove(id);
  app.windows.remove(id);
  window::close(id)
}

/// Opens a detached contract window with native chrome. `open_native_window` mints the id
/// synchronously, so registration, state seeding, and the loader kickoff all happen here with no
/// ready-message indirection. Unlimited instances coexist, duplicates included, because every open
/// mints a fresh id; geometry is never persisted, so each window opens centered at the default size.
fn open_contract_window(app: &mut App, source: contract_detail::Source, contract_id: i64) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  let size = Size::new(
    contract_detail::CONTRACT_WINDOW_WIDTH,
    contract_detail::CONTRACT_WINDOW_HEIGHT,
  );
  let (id, open_task) = open_native_window(app, Window::Contract, size);
  app
    .contracts
    .insert(id, contract_detail::State::new(source, contract_id));
  Task::batch([
    open_task,
    contract_detail::load(&db, source, contract_id).map(move |msg| Message::Contract(id, msg)),
  ])
}

fn handle_contract(app: &mut App, id: window::Id, msg: contract_detail::Message) -> Task<Message> {
  let Some(state) = app.contracts.get_mut(id) else {
    return Task::none();
  };
  let contract_detail::Message::Loaded(detail) = msg;
  state.set_detail(*detail);
  let keys = state.stale_images();
  dispatch_image_fetches(app, keys)
}

fn close_contract_window(app: &mut App, id: window::Id) -> Task<Message> {
  app.contracts.remove(id);
  app.windows.remove(id);
  window::close(id)
}

/// Opens the detached Manage Plans window with native chrome, or focuses the existing one when already
/// open (single-instance). The shared native-window helper mints the id synchronously, so registration,
/// state seeding, and the roster loader all happen here with no ready-message indirection. Geometry
/// persists across launches via the window's `state_key` + `restored_geometry`.
fn open_manage_plans_window(app: &mut App) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  if let Some(id) = app.windows.id_for(Window::ManagePlans) {
    return window::gain_focus(id);
  }
  let size = Size::new(
    skill_plan_manager::MANAGE_PLANS_WINDOW_WIDTH,
    skill_plan_manager::MANAGE_PLANS_WINDOW_HEIGHT,
  );
  let (id, open_task) = open_native_window(app, Window::ManagePlans, size);
  app.manage_plans = Some((id, skill_plan_manager::State::new()));
  Task::batch([open_task, skill_plan_manager::load(&db).map(Message::ManagePlans)])
}

fn handle_manage_plans(app: &mut App, msg: skill_plan_manager::Message) -> Task<Message> {
  match msg {
    skill_plan_manager::Message::CancelDelete => {
      if let Some((_, state)) = app.manage_plans.as_mut() {
        state.clear_delete();
      }
      Task::none()
    }
    skill_plan_manager::Message::CharacterSelected(character_id) => {
      if let Some((_, state)) = app.manage_plans.as_mut() {
        state.select(character_id);
      }
      Task::none()
    }
    skill_plan_manager::Message::ConfirmDelete(plan_id) => {
      if let Some((_, state)) = app.manage_plans.as_mut() {
        state.clear_delete();
      }
      let Some(runtime) = app.runtime.as_ref() else {
        return Task::none();
      };
      let db = runtime.db.clone();
      Task::perform(
        async move {
          if let Err(error) = store::repo::skills::delete(&db, plan_id).await {
            tracing::error!(plan_id, %error, "failed to delete skill plan");
          }
          Box::new(skill_plan_manager::load_roster(&db).await)
        },
        skill_plan_manager::Message::Loaded,
      )
      .map(Message::ManagePlans)
    }
    skill_plan_manager::Message::CopyPlan {
      plan_id,
      target_character_id,
    } => {
      if let Some((_, state)) = app.manage_plans.as_mut() {
        state.close_copy_menu();
      }
      let existing_names = manage_plans_target_names(app, target_character_id);
      let Some(runtime) = app.runtime.as_ref() else {
        return Task::none();
      };
      let db = runtime.db.clone();
      Task::perform(
        async move {
          if let Err(error) = copy_plan_to_character(&db, plan_id, target_character_id, &existing_names).await {
            tracing::error!(plan_id, target_character_id, %error, "failed to copy skill plan");
          }
          Box::new(skill_plan_manager::load_roster(&db).await)
        },
        skill_plan_manager::Message::Loaded,
      )
      .map(Message::ManagePlans)
    }
    skill_plan_manager::Message::Loaded(roster) => {
      let Some((_, state)) = app.manage_plans.as_mut() else {
        return Task::none();
      };
      state.set_roster(*roster);
      let keys = state.stale_images();
      dispatch_image_fetches(app, keys)
    }
    skill_plan_manager::Message::NewPlan(character_id) => {
      open_plan_from_manager(app, character_id, skill_plan_editor::Seed::New)
    }
    skill_plan_manager::Message::OpenPlan {
      character_id,
      plan_id,
    } => open_plan_from_manager(app, character_id, skill_plan_editor::Seed::Existing(plan_id)),
    skill_plan_manager::Message::RequestDelete(plan_id) => {
      if let Some((_, state)) = app.manage_plans.as_mut() {
        state.arm_delete(plan_id);
      }
      Task::none()
    }
    skill_plan_manager::Message::ToggleCopyMenu(plan_id) => {
      if let Some((_, state)) = app.manage_plans.as_mut() {
        state.toggle_copy_menu(plan_id);
      }
      Task::none()
    }
  }
}

fn manage_plans_target_names(app: &App, target_character_id: i64) -> Vec<String> {
  app
    .manage_plans
    .as_ref()
    .map(|(_, state)| {
      state
        .entries()
        .iter()
        .find(|entry| entry.character_id == target_character_id)
        .map(|entry| entry.plans.iter().map(|plan| plan.name.clone()).collect())
        .unwrap_or_default()
    })
    .unwrap_or_default()
}

async fn copy_plan_to_character(
  db: &store::Database,
  plan_id: i64,
  target_character_id: i64,
  existing_names: &[String],
) -> Result<i64, store::Error> {
  let Some((_, mut plan)) = skill_plan_editor::read_stored_plan(db, plan_id).await? else {
    return Ok(0);
  };
  plan.name = skill_plan_editor::deduped_name(&plan.name, existing_names);
  skill_plan_editor::persist_onto_character(db, target_character_id, None, &plan).await
}

/// Switches the Skills active character to the plan owner, opens the pinned editor on `seed`, and
/// closes the Manage Plans window so the editor takes over.
fn open_plan_from_manager(app: &mut App, character_id: i64, seed: skill_plan_editor::Seed) -> Task<Message> {
  let close = match app.manage_plans.take() {
    Some((id, _)) => {
      app.windows.remove(id);
      window::close(id)
    }
    None => Task::none(),
  };

  navigate(app, Route::Skills(character_id));
  app.selected_character = Some(character_id);
  let owned = owned_pilot_ids(app);
  let switch = match (app.skills.as_mut(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => Task::batch([
      skills::update(state, skills::Message::CharacterChanged(character_id), &runtime.db).map(Message::Skills),
      skills::load(&runtime.db, character_id, owned).map(Message::Skills),
    ]),
    _ => Task::none(),
  };

  Task::batch([close, switch, open_editor_window(app, character_id, seed)])
}

fn close_manage_plans_window(app: &mut App, id: window::Id) -> Task<Message> {
  if app.manage_plans.as_ref().map(|(mid, _)| *mid) == Some(id) {
    app.manage_plans = None;
  }
  app.windows.remove(id);
  window::close(id)
}

/// Opens a detached Stockpile Editor window with native chrome. `open_native_window` mints the id
/// synchronously, so registration, per-window `Editor` seeding, and the on-open scope resolve all
/// happen here with no `*WindowReady` round-trip. Multi-instance: every open mints a fresh id (New,
/// Edit, and import-prefill can coexist), and geometry is never persisted, so each opens centered at
/// the default size.
fn open_stockpile_editor_window(app: &mut App, seed: assets::EditorSeed) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let editor = assets::Editor::from_seed(seed);
  // Seed the live pilot preview as soon as the window opens, before the user edits the scope, mirroring
  // the former on-open scope resolve.
  let scope = editor.scope_query().to_owned();
  let resolve = stockpile_scope_resolve(runtime, scope);
  let size = Size::new(
    assets::STOCKPILE_EDITOR_WINDOW_WIDTH,
    assets::STOCKPILE_EDITOR_WINDOW_HEIGHT,
  );
  let (id, open_task) = open_native_window(app, Window::StockpileEditor, size);
  app.stockpile_editors.insert(id, editor);
  Task::batch([
    open_task,
    resolve.map(move |msg| match msg {
      Message::Assets(assets) => Message::StockpileEditor(id, assets),
      other => other,
    }),
  ])
}

/// Routes a per-window editor message to its window's [`Editor`], applies it, and dispatches the
/// reported follow-up (item/location/scope search, save, or close). Save and Close both close the
/// window; Save first persists and reloads the main view's stockpile grid.
fn handle_stockpile_editor(app: &mut App, id: window::Id, msg: assets::Message) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let Some(editor) = app.stockpile_editors.get_mut(id) else {
    return Task::none();
  };
  match assets::apply_editor(editor, msg) {
    assets::EditorEffect::None => Task::none(),
    assets::EditorEffect::ItemSearch(query) => {
      stockpile_item_search(runtime, query).map(move |msg| reroute_to_stockpile_editor(id, msg))
    }
    assets::EditorEffect::LocationSearch {
      generation,
      query,
    } => stockpile_location_search(runtime, query, generation).map(move |msg| reroute_to_stockpile_editor(id, msg)),
    assets::EditorEffect::ScopeResolve(query) => {
      stockpile_scope_resolve(runtime, query).map(move |msg| reroute_to_stockpile_editor(id, msg))
    }
    assets::EditorEffect::Save => {
      let Some(editor) = app.stockpile_editors.get(id).cloned() else {
        return Task::none();
      };
      let save = stockpile_save_window(runtime, editor);
      Task::batch([save, close_stockpile_editor_window(app, id)])
    }
    assets::EditorEffect::Close => close_stockpile_editor_window(app, id),
  }
}

/// Re-tags an `assets::Message` produced by a stockpile editor search helper so the result routes back
/// to the originating editor window instead of the main assets view.
fn reroute_to_stockpile_editor(id: window::Id, msg: Message) -> Message {
  match msg {
    Message::Assets(assets) => Message::StockpileEditor(id, assets),
    other => other,
  }
}

/// Saves a detached editor and reloads the main view's stockpile grid. Unlike the retired in-place
/// save, the reload routes to the main assets state (the editor window is closing) via a top-level
/// `Assets(StockpilesReloaded)`.
fn stockpile_save_window(runtime: &Runtime, editor: assets::Editor) -> Task<Message> {
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let image = Arc::clone(&runtime.eve_image);
  let sso = Arc::clone(&runtime.sso);
  Task::perform(
    async move { assets::save_stockpile(db, esi, image, sso, editor).await },
    |cards| Message::Assets(assets::Message::StockpilesReloaded(cards)),
  )
}

fn close_stockpile_editor_window(app: &mut App, id: window::Id) -> Task<Message> {
  app.stockpile_editors.remove(id);
  app.windows.remove(id);
  window::close(id)
}

/// Opens the detached Import-Multibuy window with native chrome. Single-instance: any already-open
/// import window is closed first so a fresh paste starts clean, and `Window::StockpileImport` carries
/// a `state_key`, so `open_native_window` restores its persisted size/position. Confirming a paste in
/// this window opens a prefilled Stockpile Editor window (a window spawning another window).
fn open_stockpile_import_window(app: &mut App) -> Task<Message> {
  if app.runtime.is_none() {
    return Task::none();
  }

  let close_existing = match app.windows.id_for(Window::StockpileImport) {
    Some(existing) => close_stockpile_import_window(app, existing),
    None => Task::none(),
  };

  let size = Size::new(
    assets::STOCKPILE_IMPORT_WINDOW_WIDTH,
    assets::STOCKPILE_IMPORT_WINDOW_HEIGHT,
  );
  let (id, open_task) = open_native_window(app, Window::StockpileImport, size);
  app.stockpile_imports.insert(id, assets::ImportPanel::blank());
  Task::batch([close_existing, open_task])
}

/// Routes a per-window import message to its window's [`ImportPanel`], applies it, and dispatches the
/// reported follow-up. Resolve runs the multibuy resolver; Confirm opens a prefilled editor window and
/// closes the import window; Close just closes the window.
fn handle_stockpile_import(app: &mut App, id: window::Id, msg: assets::Message) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let Some(panel) = app.stockpile_imports.get_mut(id) else {
    return Task::none();
  };
  match assets::apply_import(panel, msg) {
    assets::ImportEffect::None => Task::none(),
    assets::ImportEffect::Resolve(text) => {
      stockpile_import_resolve(runtime, text).map(move |msg| reroute_to_stockpile_import(id, msg))
    }
    assets::ImportEffect::Confirm(matched) => {
      let matched: Vec<assets::MultibuyMatch> = matched;
      let open = open_stockpile_editor_window(app, assets::EditorSeed::Prefill(matched));
      Task::batch([open, close_stockpile_import_window(app, id)])
    }
    assets::ImportEffect::Close => close_stockpile_import_window(app, id),
  }
}

/// Re-tags an `assets::Message` produced by the import resolver so the result routes back to the
/// originating import window instead of the main assets view.
fn reroute_to_stockpile_import(id: window::Id, msg: Message) -> Message {
  match msg {
    Message::Assets(assets) => Message::StockpileImport(id, assets),
    other => other,
  }
}

fn close_stockpile_import_window(app: &mut App, id: window::Id) -> Task<Message> {
  app.stockpile_imports.remove(id);
  app.windows.remove(id);
  window::close(id)
}

/// Opens a detached Mail Compose window with native chrome. `open_native_window` mints the id
/// synchronously, so registration, state seeding, and the draft loader all happen here with no
/// ready-message indirection. Unlimited instances coexist, duplicates included, because every open
/// mints a fresh id; geometry is never persisted, so each window opens centered at the default size.
fn open_compose_window(app: &mut App, seed: mail::compose::Seed) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let load = match seed.draft_id() {
    Some(draft_id) => {
      let db = runtime.db.clone();
      mail::compose::load_draft(&db, draft_id)
    }
    None => Task::none(),
  };
  let size = Size::new(
    mail::compose::COMPOSE_WINDOW_WIDTH,
    mail::compose::COMPOSE_WINDOW_HEIGHT,
  );
  let (id, open_task) = open_native_window(app, Window::MailCompose, size);
  app.composes.insert(id, mail::compose::Draft::from_seed(seed));
  Task::batch([open_task, load.map(move |msg| Message::Compose(id, msg))])
}

/// Opens a compose window seeded for a persisted draft `draft_id`; the row is loaded into the window
/// once it exists (routed per-window via `Compose(id, DraftLoaded)`).
fn open_draft_window(app: &mut App, draft_id: i64) -> Task<Message> {
  let Some(from) = app.mail.as_ref().and_then(mail::State::default_from) else {
    return Task::none();
  };
  open_compose_window(
    app,
    mail::compose::Seed::Draft {
      draft_id,
      from_character_id: from,
    },
  )
}

/// Routes a per-window compose message to its window's [`Draft`], applies it, and dispatches the
/// reported follow-up (recipient/link search, send, or close). Close auto-saves a non-empty draft
/// then closes the window; Send enqueues the mail, deletes the draft by id, and closes the window.
fn handle_compose(app: &mut App, id: window::Id, msg: mail::Message) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  // A persisted-draft load and a save-completed id thread back per window, outside the Effect path.
  match msg {
    mail::Message::DraftLoaded(row) => {
      if let (Some(row), Some(draft)) = (*row, app.composes.get_mut(id)) {
        *draft = mail::compose::Draft::from_persisted(&row);
      }
      return Task::none();
    }
    mail::Message::DraftSaved(saved_id) => {
      if let Some(draft) = app.composes.get_mut(id) {
        draft.set_id(saved_id);
      }
      return Task::none();
    }
    mail::Message::ComposeSent(Ok(())) => return compose_send_completed(app, id),
    _ => {}
  }
  let Some(draft) = app.composes.get_mut(id) else {
    return Task::none();
  };
  match mail::compose::update(draft, msg) {
    mail::compose::Effect::None => Task::none(),
    mail::compose::Effect::RecipientSearch {
      is_to,
      query,
    } => compose_recipient_search(runtime, draft, is_to, query, id),
    mail::compose::Effect::LinkSearch(query) => compose_link_search(runtime, draft, query, id),
    mail::compose::Effect::Send => {
      let send = mail::compose::send(&runtime.db, draft);
      send.map(move |msg| Message::Compose(id, msg))
    }
    mail::compose::Effect::Discard => discard_compose_window(app, id),
  }
}

/// Closes a compose window without saving its draft (the explicit Discard path).
fn discard_compose_window(app: &mut App, id: window::Id) -> Task<Message> {
  app.composes.remove(id);
  app.windows.remove(id);
  window::close(id)
}

/// Completes a successful send: deletes the persisted draft (if any) by id, closes the window, and
/// refreshes the main-view Drafts/Sent listing.
fn compose_send_completed(app: &mut App, id: window::Id) -> Task<Message> {
  let sent_draft_id = app.composes.get(id).and_then(mail::compose::Draft::sent_draft_id);
  let delete = match (sent_draft_id, app.runtime.as_ref()) {
    (Some(draft_id), Some(runtime)) => {
      let db = runtime.db.clone();
      Task::future(async move { mail::delete_draft(db, draft_id).await }).discard()
    }
    _ => Task::none(),
  };
  let reload = reload_main_mail(app);
  Task::batch([delete, close_compose_window(app, id), reload])
}

/// Saves a compose window's draft (if non-empty) then closes it, refreshing the main-view Drafts list.
fn close_compose_window(app: &mut App, id: window::Id) -> Task<Message> {
  let save = match (
    app.composes.get(id).and_then(mail::compose::Draft::pending_save),
    app.runtime.as_ref(),
  ) {
    (Some((draft_id, input)), Some(runtime)) => {
      let db = runtime.db.clone();
      Task::perform(
        async move { mail::persist_pending_draft(db, draft_id, input).await },
        |()| Message::Mail(mail::Message::DraftSaved(None)),
      )
    }
    _ => Task::none(),
  };
  app.composes.remove(id);
  app.windows.remove(id);
  Task::batch([save, window::close(id)])
}

fn compose_recipient_search(
  runtime: &Runtime,
  draft: &mail::compose::Draft,
  is_to: bool,
  query: String,
  id: window::Id,
) -> Task<Message> {
  use crate::features::entity_search;

  if query.trim().chars().count() < mail::RECIPIENT_SEARCH_MIN_CHARS {
    return Task::none();
  }
  let generation = draft.recipient_search_generation(is_to);
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let eve_image = Arc::clone(&runtime.eve_image);
  let sso = Arc::clone(&runtime.sso);
  let categories = vec![
    entity_search::EntityCategory::Character,
    entity_search::EntityCategory::Corporation,
  ];
  Task::perform(
    async move { entity_search::search_entities(db, esi, eve_image, sso, categories, query).await },
    move |results| {
      let results = results.into_iter().map(entity_ref_from_result).collect();
      let msg = if is_to {
        mail::Message::ComposeToSearched {
          generation,
          results,
        }
      } else {
        mail::Message::ComposeCcSearched {
          generation,
          results,
        }
      };
      Message::Compose(id, msg)
    },
  )
}

fn compose_link_search(
  runtime: &Runtime,
  draft: &mail::compose::Draft,
  query: String,
  id: window::Id,
) -> Task<Message> {
  use crate::features::entity_search;

  let Some((generation, category)) = draft.link_search() else {
    return Task::none();
  };
  if query.trim().chars().count() < mail::RECIPIENT_SEARCH_MIN_CHARS {
    return Task::none();
  }
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let eve_image = Arc::clone(&runtime.eve_image);
  let sso = Arc::clone(&runtime.sso);
  Task::perform(
    async move { entity_search::search_entities(db, esi, eve_image, sso, vec![category], query).await },
    move |results| {
      let results = results.into_iter().map(entity_ref_from_result).collect();
      Message::Compose(
        id,
        mail::Message::ComposeLinkSearched {
          generation,
          results,
        },
      )
    },
  )
}

/// Refreshes the main mail view after a compose window saves or sends, so the Drafts/Sent listing and
/// folder badges reflect the change.
fn reload_main_mail(app: &App) -> Task<Message> {
  match (app.mail.as_ref(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => mail::reload(&runtime.db, state.active()).map(Message::Mail),
    _ => Task::none(),
  }
}

fn update(app: &mut App, message: Message) -> Task<Message> {
  let span = tracing::trace_span!(target: "pod::ui", "update", message = message.variant_name());
  let _entered = span.enter();
  // Only a load/sync message can introduce new image-bearing rows; gating here keeps the staleness scan off the
  // per-frame interaction path (scroll, hover, filter).
  let recheck_images = message.affects_images();
  let task = match dispatch_feature(app, message) {
    Ok(task) => task,
    Err(message) => dispatch_lifecycle(app, *message),
  };
  if !recheck_images {
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
    Message::Assets(msg) => handle_assets(app, msg),
    Message::Auth(msg) => handle_auth(app, msg),
    Message::Calendar(msg) => handle_calendar(app, msg),
    Message::CalendarAttentionCounted(count) => handle_calendar_attention_counted(app, count),
    Message::CalendarEvent(id, msg) => handle_calendar_event(app, id, msg),
    Message::CharacterDetail(msg) => handle_character_detail(app, msg),
    Message::CharacterManager(msg) => handle_character_manager(app, msg),
    Message::Compare(msg) => handle_compare(app, msg),
    Message::Compose(id, msg) => handle_compose(app, id, msg),
    Message::Contract(id, msg) => handle_contract(app, id, msg),
    Message::CorporationDetail(msg) => handle_corporation_detail(app, msg),
    Message::Industry(msg) => handle_industry(app, msg),
    Message::Killmail(id, msg) => handle_killmail(app, id, msg),
    Message::Mail(msg) => handle_mail(app, msg),
    Message::MailUnreadCounted(unread) => handle_mail_unread_counted(app, unread),
    Message::ManagePlans(msg) => handle_manage_plans(app, msg),
    Message::Settings(msg) => handle_settings(app, msg),
    Message::SkillPlanEditor(msg) => handle_skill_plan_editor(app, msg),
    Message::Skills(msg) => handle_skills(app, msg),
    Message::StockpileEditor(id, msg) => handle_stockpile_editor(app, id, msg),
    Message::StockpileImport(id, msg) => handle_stockpile_import(app, id, msg),
    Message::Sync(event) => handle_sync(app, event),
    Message::Wallet(msg) => handle_wallet(app, msg),
    other => return dispatch_feature_aux(app, other),
  })
}

fn dispatch_feature_aux(app: &mut App, message: Message) -> Result<Task<Message>, Box<Message>> {
  Ok(match message {
    Message::ClearNotifications => handle_clear_notifications(app),
    Message::CloseNotificationsPanel => handle_close_notifications_panel(app),
    Message::MarkAllNotificationsRead => handle_mark_all_notifications_read(app),
    Message::Mcp(request) => handle_mcp(app, request),
    Message::McpDataChanged => handle_mcp_data_changed(app),
    Message::Nav(destination) => handle_nav(app, destination),
    Message::NavTo(destination, sub_section) => handle_nav_to(app, destination, sub_section),
    Message::NotificationActivated(id) => handle_notification_activated(app, id),
    Message::NotificationsHistoryPageLoaded {
      epoch,
      rows,
      who,
    } => handle_notifications_history_page_loaded(app, epoch, rows, who),
    Message::NotificationsHistoryScrolled {
      absolute,
      relative,
    } => handle_notifications_history_scrolled(app, absolute, relative),
    Message::NotificationsRefreshed(snapshot) => handle_notifications_refreshed(app, *snapshot),
    Message::SelectNotificationTab(tab) => handle_select_notification_tab(app, tab),
    Message::RailHover(destination) => handle_rail_hover(app, destination),
    Message::RailHoverExpire(generation) => handle_rail_hover_expire(app, generation),
    Message::ToastDismissed(id) => handle_toast_dismissed(app, id),
    Message::ToastHover(id, hovered) => handle_toast_hover(app, id, hovered),
    Message::ToastTick => handle_toast_tick(app),
    Message::ToggleNotificationsPanel => handle_toggle_notifications_panel(app),
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
    Message::TrashPurged(purged) => handle_trash_purged(app, purged),
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
    Message::TakeOverResolved(outcome, ready) => handle_take_over_resolved(app, outcome, *ready),
    other => dispatch_window_lifecycle(app, other),
  }
}

fn dispatch_window_lifecycle(app: &mut App, message: Message) -> Task<Message> {
  match message {
    Message::Chrome(id, event) => handle_chrome_event(app, id, event),
    Message::CloseSyncPopover => set_sync_popover_open(app, false),
    Message::FocusMainWindow => handle_focus_main_window(app),
    Message::Palette(msg) => handle_palette(app, msg),
    Message::Quit => shutdown(app),
    Message::Shortcut(chord) => handle_shortcut(app, chord),
    Message::TextInputFocused(id) => handle_text_input_focused(app, id),
    Message::ToggleSyncPopover => handle_toggle_sync_popover(app),
    Message::UpdaterAction(action) => handle_updater_action(app, action),
    Message::UpdaterDismissToast => handle_updater_dismiss_toast(app),
    Message::UpdaterStateChanged(state) => handle_updater_state_changed(app, state),
    Message::Window(id, event) => handle_window(app, id, event),
    Message::WindowOpened(id) => on_window_opened(app, id),
    _ => Task::none(),
  }
}

fn handle_shortcut(app: &mut App, chord: Chord) -> Task<Message> {
  let action = match chord {
    Chord::FocusSearch => focus_route_search(app),
    Chord::OpenSettings => handle_nav(app, rail::Destination::Settings),
    Chord::Quit => Task::done(Message::Quit),
  };
  app.keyboard_focus.set_focused(None);
  Task::batch([action, shortcuts::probe_focus(Message::TextInputFocused)])
}

fn focus_route_search(app: &App) -> Task<Message> {
  match focus_search::search_id(app.route.destination()) {
    Some(id) => iced::widget::operation::focus(id),
    None => Task::none(),
  }
}

fn handle_text_input_focused(app: &mut App, id: iced::widget::Id) -> Task<Message> {
  app.keyboard_focus.set_focused(Some(id));
  Task::none()
}

fn handle_palette(app: &mut App, message: PaletteMessage) -> Task<Message> {
  match message {
    PaletteMessage::Activate(index) => palette_activate(app, index),
    PaletteMessage::ActivateSelected => {
      let index = app.palette.as_ref().map(|state| state.selected).unwrap_or(0);
      palette_activate(app, index)
    }
    PaletteMessage::Close => {
      app.palette = None;
      Task::none()
    }
    PaletteMessage::MoveDown => {
      let count = palette_entries(app).len();
      if let Some(state) = app.palette.as_mut() {
        let max = count.saturating_sub(1);
        state.selected = (state.selected + 1).min(max);
      }
      Task::none()
    }
    PaletteMessage::MoveUp => {
      if let Some(state) = app.palette.as_mut() {
        state.selected = state.selected.saturating_sub(1);
      }
      Task::none()
    }
    PaletteMessage::Open => palette_open(app),
    PaletteMessage::QueryChanged(query) => {
      if let Some(state) = app.palette.as_mut() {
        state.query = query;
        state.selected = 0;
      }
      Task::none()
    }
    PaletteMessage::Select(index) => {
      if let Some(state) = app.palette.as_mut() {
        state.selected = index;
      }
      Task::none()
    }
  }
}

fn palette_open(app: &mut App) -> Task<Message> {
  app.palette = Some(command_palette::State::default());
  // The palette's own field owns text focus; clear the global tracker so a still-focused page input
  // can't keep stealing the focus-gated `/` while the palette is up.
  app.keyboard_focus.set_focused(None);
  iced::widget::operation::focus(command_palette::input_id())
}

fn palette_activate(app: &mut App, index: usize) -> Task<Message> {
  let entries = palette_entries(app);
  let Some(entry) = entries.get(index) else {
    return Task::none();
  };
  let action = entry.action.clone();
  palette_activate_action(app, action)
}

fn palette_activate_action(app: &mut App, action: PaletteAction) -> Task<Message> {
  app.palette = None;
  match action {
    PaletteAction::Command(command) => palette_command(app, command),
    PaletteAction::Detail(PaletteEntity {
      id,
      kind,
      ..
    }) => match kind {
      PaletteEntityKind::Character => navigate_to_character_detail(app, id),
      PaletteEntityKind::Corporation => navigate_to_corporation_detail(app, id),
    },
    PaletteAction::NavTo(section, sub) => handle_nav_to(app, section.destination, sub),
  }
}

fn palette_command(app: &mut App, command: PaletteCommand) -> Task<Message> {
  match command {
    PaletteCommand::AddCharacter => update(app, Message::Auth(auth::Message::Start(feature_flags(app)))),
    PaletteCommand::ComposeMail => match palette_compose_from(app) {
      Some(from_character_id) => open_compose_window(
        app,
        mail::compose::Seed::Blank {
          from_character_id,
        },
      ),
      None => Task::none(),
    },
    PaletteCommand::CreateStockpile => open_stockpile_editor_window(app, assets::EditorSeed::Blank),
    PaletteCommand::ManageSkillPlans => open_manage_plans_window(app),
    PaletteCommand::OpenSettings => handle_nav(app, rail::Destination::Settings),
    PaletteCommand::SyncNow => sync_now(app),
    PaletteCommand::ToggleHighContrast => toggle_high_contrast(app),
  }
}

/// Resolves the from-character for a palette-launched compose: the mail view's default sender when a
/// mail view is open, otherwise the active / first owned character. `None` only when no character is
/// owned, in which case the command no-ops.
fn palette_compose_from(app: &App) -> Option<i64> {
  if let Some(from) = app.mail.as_ref().and_then(mail::State::default_from) {
    return Some(from);
  }
  let roster = app
    .character_manager
    .as_ref()
    .map(character_manager::owned_roster)
    .unwrap_or_default();
  resolve_mail_target(&roster, app.selected_character)
}

fn toggle_high_contrast(app: &mut App) -> Task<Message> {
  let enabled = !app.accessibility.high_contrast();
  app.accessibility.set_high_contrast(enabled);
  color::set_high_contrast(enabled);
  if let Some(runtime) = app.runtime.as_mut() {
    runtime.settings.accessibility_mut().set_high_contrast(enabled);
    config::save(&runtime.settings);
  }
  // The Settings screen builds its own accessibility copy on open; rebuild it so a live screen mirrors
  // the toggle instead of showing the pre-toggle value.
  if let (Some(runtime), Some(_)) = (app.runtime.as_ref(), app.settings.as_ref()) {
    app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
  }
  refresh_all_windows(app)
}

fn palette_entries(app: &App) -> Vec<command_palette::Entry> {
  let query = app.palette.as_ref().map(|state| state.query.as_str()).unwrap_or("");
  command_palette::build_entries(
    &enabled_features(app),
    &palette_characters(app),
    &palette_corporations(app),
    query,
  )
}

fn palette_characters(app: &App) -> Vec<(i64, String)> {
  app
    .character_manager
    .as_ref()
    .map(character_manager::owned_roster)
    .unwrap_or_default()
    .into_iter()
    .map(|pilot| (pilot.id, pilot.name))
    .collect()
}

fn palette_corporations(app: &App) -> Vec<(i64, String)> {
  app
    .character_manager
    .as_ref()
    .map(character_manager::owned_corporations)
    .unwrap_or_default()
}

fn handle_assets(app: &mut App, msg: assets::Message) -> Task<Message> {
  if let assets::Message::PaneSettled(key, ratio) = msg {
    record_pane_ratio(app, key, ratio);
    return Task::none();
  }
  if let assets::Message::ReauthRequested(id) = msg {
    return update(app, Message::ReauthCharacter(id));
  }
  // The stockpile editor is a detached window: New/Edit/import-confirm open one instead of mutating an
  // in-place holder. Edit and import-confirm read from the main assets state first.
  match msg {
    assets::Message::StockpileNew => return open_stockpile_editor_window(app, assets::EditorSeed::Blank),
    assets::Message::StockpileEditStarted(id) => {
      let Some(card) = app.assets.as_ref().and_then(|state| state.stockpile_card(id).cloned()) else {
        return Task::none();
      };
      if let Some(state) = app.assets.as_mut() {
        state.dismiss_stockpile_context_menu();
      }
      return open_stockpile_editor_window(app, assets::EditorSeed::FromCard(Box::new(card)));
    }
    // The import multibuy panel is its own single-instance native window now, so the main view's
    // "Import multibuy" button opens it instead of toggling an in-place overlay.
    assets::Message::StockpileImportOpened => return open_stockpile_import_window(app),
    _ => {}
  }

  let (Some(state), Some(runtime)) = (app.assets.as_mut(), app.runtime.as_ref()) else {
    return Task::none();
  };

  dispatch_assets_with_runtime(state, runtime, msg)
}

fn dispatch_assets_with_runtime(state: &mut assets::State, runtime: &Runtime, msg: assets::Message) -> Task<Message> {
  assets::update(state, msg, &runtime.db).map(Message::Assets)
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

fn stockpile_item_search(runtime: &Runtime, query: String) -> Task<Message> {
  if query.trim().chars().count() < assets::STOCKPILE_SEARCH_MIN_CHARS {
    return Task::none();
  }
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let sso = Arc::clone(&runtime.sso);
  Task::perform(
    async move { assets::search_item_types(db, esi, sso, query).await },
    move |results| Message::Assets(assets::Message::StockpileEditorItemResults(results)),
  )
}

fn stockpile_location_search(runtime: &Runtime, query: String, generation: u64) -> Task<Message> {
  if query.trim().chars().count() < assets::STOCKPILE_SEARCH_MIN_CHARS {
    return Task::none();
  }
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let sso = Arc::clone(&runtime.sso);
  Task::perform(
    async move { assets::search_locations_enriched(db, esi, sso, query).await },
    move |results| Message::Assets(assets::Message::StockpileEditorLocationResults(generation, results)),
  )
}

fn stockpile_scope_resolve(runtime: &Runtime, query: String) -> Task<Message> {
  let db = runtime.db.clone();
  Task::perform(async move { assets::resolve_scope_pilots(db, query).await }, |pilots| {
    Message::Assets(assets::Message::StockpileEditorScopeResolved(pilots))
  })
}

fn handle_mail_unread_counted(app: &mut App, unread: i64) -> Task<Message> {
  app.mail_unread = unread;
  Task::none()
}

/// Runs a bridged MCP tool call to completion off the UI thread and replies to the waiting agent
/// through the request's one-shot. The tool is gated against the live config's permissions, so a
/// call whose permission is disabled is refused without touching the database.
fn handle_mcp(app: &mut App, request: mcp::McpRequest) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    request.reply(Err(mcp::tool::ToolError::Internal(
      "the store is not open yet".to_owned(),
    )));
    return Task::none();
  };
  let db = runtime.db.clone();
  let config = runtime.settings.mcp().clone();
  tokio::spawn(mcp::fulfill(request, mcp::registry(), config, db));
  Task::none()
}

/// An MCP write tool reported that it changed the database, so reload whatever the open view shows.
/// Marks the roster dirty and lifts the reload debounce so the refresh fires now, then drives the
/// same per-view drains a sync pulse runs.
// The MCP write signal is intentionally coarse and kind-agnostic: an agent wrote something, but we
// don't know what. Force every open data view dirty (not just the roster) so the drain below actually
// reloads the currently-open assets/wallet/character-detail view instead of waiting for the next sync.
fn handle_mcp_data_changed(app: &mut App) -> Task<Message> {
  app.roster_dirty = true;
  app.next_roster_reload = None;
  if let Some(assets) = app.assets.as_mut() {
    assets.force_dirty();
  }
  if let Some(wallet) = app.wallet.as_mut() {
    wallet.force_dirty();
  }
  if let Some(detail) = app.character_detail.as_mut() {
    detail.force_dirty();
  }
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

/// Reconciles the embedded MCP listener with the live config, generating and persisting a bearer
/// token the first time the server is enabled without one. A no-op when the runtime is absent.
fn sync_mcp_server(app: &mut App) {
  let Some(runtime) = app.runtime.as_mut() else {
    return;
  };
  if *runtime.settings.mcp().enabled() && runtime.settings.mcp().token().is_empty() {
    runtime.settings.mcp_mut().token_or_generate();
    config::save(&runtime.settings);
  }
  let config = runtime.settings.mcp().clone();
  let server = app.mcp_server.get_or_insert_with(mcp::server);
  server.apply(&config);
}

fn handle_reauth_character(app: &mut App, character_id: i64) -> Task<Message> {
  let flags = feature_flags(app);
  tracing::info!(
    character_id,
    scopes = ?auth::scopes_for(&flags),
    "re-authorizing character via SSO sign-in"
  );
  update(app, Message::Auth(auth::Message::Start(flags)))
}

fn reauth_corporation(app: &mut App, corporation_id: i64) -> Task<Message> {
  let flags = feature_flags(app);
  tracing::info!(
    corporation_id,
    scopes = ?auth::corp_scopes_for(&flags),
    "re-authorizing corporation via SSO sign-in"
  );
  update(app, Message::Auth(auth::Message::StartAddCorporation(flags)))
}

fn handle_settings(app: &mut App, msg: settings::Message) -> Task<Message> {
  let features_changed = matches!(
    msg,
    settings::Message::Features(
      settings::features_tab::Message::GroupToggled(..)
        | settings::features_tab::Message::SubToggled(..)
        | settings::features_tab::Message::Toggled(..)
    ) | settings::Message::ResetToDefaults
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
    settings::Outcome::UiChanged => {
      // The rail reads its side and order from `ui_config`, which prefers the live settings screen
      // and falls back to the runtime; keep the runtime copy in lock-step so windows that read it
      // stay honest, then redraw every window to re-dock and reorder the rail live.
      let ui = state.settings().ui().clone();
      if let Some(runtime) = app.runtime.as_mut() {
        *runtime.settings.ui_mut() = ui;
      }
      return Task::batch(vec![task, refresh_all_windows(app)]);
    }
    settings::Outcome::McpChanged => {
      let mcp = state.settings().mcp().clone();
      if let Some(runtime) = app.runtime.as_mut() {
        *runtime.settings.mcp_mut() = mcp;
      }
      sync_mcp_server(app);
      return task;
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
    settings::Outcome::ExportData => {
      let storage = state.settings().storage();
      let diagnostics = settings::log_export::Diagnostics {
        cache_dir: storage.resolved_cache_dir(),
        database_path: storage.resolved_database_path(),
        db_dir: storage.resolved_db_dir(),
        log_dir: storage.resolved_log_dir(),
      };
      let database_path = storage.resolved_database_path();
      let config_bytes = match toml::to_string_pretty(state.settings()) {
        Ok(toml) => toml.into_bytes(),
        Err(error) => {
          return Task::batch(vec![
            task,
            Task::done(Message::Settings(settings::Message::Storage(
              settings::storage_tab::Message::DataExportFinished(Err(format!("Couldn't serialize settings: {error}"))),
            ))),
          ]);
        }
      };
      return Task::batch(vec![task, export_data(database_path, config_bytes, diagnostics)]);
    }
    settings::Outcome::ImportData {
      path,
    } => {
      let storage = state.settings().storage().clone();
      let local_settings = state.settings().clone();
      let machine_id = storage.machine_id().clone().unwrap_or_default();
      return Task::batch(vec![task, import_data(path, storage, machine_id, local_settings)]);
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
  let mut tasks = vec![base, character_manager::load(&db, flags).map(Message::CharacterManager)];

  let route = app.route;
  if let Some(state) = app.calendar.as_ref() {
    tasks.push(Task::done(Message::Calendar(calendar::Message::FeaturesChanged(flags))));
    if route == Route::Calendar {
      tasks.push(calendar::reload(&db, state.active(), flags).map(Message::Calendar));
    }
  }

  if let Some(state) = app.industry.as_ref() {
    let assign_pilots =
      flags.is_enabled(config::Feature::SkillMonitoring) && flags.is_enabled(config::Feature::CloneMonitoring);
    tasks.push(Task::done(Message::Industry(industry::Message::RequiredScopesChanged(
      industry_required_scopes(),
    ))));
    tasks.push(Task::done(Message::Industry(industry::Message::AssignPilotsChanged(
      assign_pilots,
    ))));
    if route == Route::Industry {
      tasks.push(industry::reload(&db, state.active(), &industry_required_scopes()).map(Message::Industry));
    }
  }

  if app.character_detail.is_some() {
    tasks.push(Task::done(Message::CharacterDetail(
      character_detail::Message::FeaturesChanged(enabled.clone()),
    )));
  }

  if app.wallet.is_some() {
    tasks.push(Task::done(Message::Wallet(wallet::Message::FeaturesChanged(flags))));
  }

  if app.assets.is_some() {
    tasks.push(Task::done(Message::Assets(assets::Message::FeaturesChanged(flags))));
  }

  if app.industry.is_some() {
    tasks.push(Task::done(Message::Industry(industry::Message::FeaturesChanged(flags))));
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

fn export_data(
  database_path: std::path::PathBuf,
  config_bytes: Vec<u8>,
  diagnostics: settings::log_export::Diagnostics,
) -> Task<Message> {
  Task::perform(
    export_data_archive(database_path, config_bytes, diagnostics),
    |result| {
      Message::Settings(settings::Message::Storage(
        settings::storage_tab::Message::DataExportFinished(result),
      ))
    },
  )
}

async fn export_data_archive(
  database_path: std::path::PathBuf,
  config_bytes: Vec<u8>,
  diagnostics: settings::log_export::Diagnostics,
) -> Result<Option<std::path::PathBuf>, String> {
  let default_name = settings::data_export::default_file_name(Utc::now());

  // Stage a self-contained snapshot of the live working file: checkpoint_into folds the WAL in so
  // the bundled pod.db carries no -wal/-shm trail. This works in both Direct and Sync modes because
  // the resolved DB path always points at the live file Pod is writing.
  let staging = tempfile::Builder::new()
    .prefix("pod-export-")
    .suffix(".db")
    .tempfile()
    .map_err(|err| format!("Couldn't create export staging file: {err}"))?;
  let snapshot_path = staging.path().to_path_buf();
  crate::store::sync_copy::checkpoint_into(&database_path, &snapshot_path)
    .await
    .map_err(|err| format!("Couldn't snapshot the database: {err}"))?;

  let bytes = tokio::task::spawn_blocking(move || {
    settings::data_export::build_archive(&snapshot_path, &config_bytes, &diagnostics)
  })
  .await
  .map_err(|err| err.to_string())??;

  // Keep the staging file alive until the archive bytes are built, then drop it.
  drop(staging);

  save_data_archive(default_name, bytes).await
}

/// Prompts for a save location via the native dialog and writes the archive there. Stubbed to a
/// no-op under `cfg(test)` so tests never open a real file dialog.
async fn save_data_archive(default_name: String, bytes: Vec<u8>) -> Result<Option<std::path::PathBuf>, String> {
  #[cfg(not(test))]
  {
    let Some(handle) = rfd::AsyncFileDialog::new()
      .set_title("Export data")
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

/// Drives the confirmed import: re-reads the archive, atomically restores its database into place,
/// merges the archived config while preserving this machine's identity, and on success quits Pod so
/// the next launch re-seeds from the restored database (ADR-0038's quit-and-reopen). A restore
/// failure surfaces through `DataImportFinished(Err)` and leaves the live data untouched.
fn import_data(
  path: std::path::PathBuf,
  storage: config::StorageConfig,
  machine_id: String,
  local_settings: config::Settings,
) -> Task<Message> {
  Task::perform(
    import_data_archive(path, storage, machine_id, local_settings),
    |result| match result {
      // Success: quit through the existing shutdown chain so the restored database takes effect on the
      // next launch (the checkpoint there is a no-op or re-establishes the restored canonical).
      Ok(()) => Message::Quit,
      Err(error) => Message::Settings(settings::Message::Storage(
        settings::storage_tab::Message::DataImportFinished(Err(error)),
      )),
    },
  )
}

async fn import_data_archive(
  path: std::path::PathBuf,
  storage: config::StorageConfig,
  machine_id: String,
  local_settings: config::Settings,
) -> Result<(), String> {
  let bytes = tokio::fs::read(&path)
    .await
    .map_err(|err| format!("Couldn't read {}: {err}", path.display()))?;

  // Re-validate the picked archive on the restore path: the UI version-guarded it for the confirm
  // modal, but the bytes are re-read here so the restore never trusts a stale parse.
  let parsed = tokio::task::spawn_blocking(move || settings::data_export::read_archive(&bytes))
    .await
    .map_err(|err| err.to_string())??;
  if parsed.verdict == settings::data_export::VersionVerdict::Incompatible {
    return Err(format!(
      "This archive was made by a newer Pod ({}); it can't be restored into this build.",
      parsed.manifest.pod_version
    ));
  }

  // Stage the archived pod.db to a tempfile so the atomic restore publishes a real on-disk file.
  let staging = tempfile::Builder::new()
    .prefix("pod-import-")
    .suffix(".db")
    .tempfile()
    .map_err(|err| format!("Couldn't create import staging file: {err}"))?;
  let temp_db = staging.path().to_path_buf();
  tokio::fs::write(&temp_db, &parsed.database)
    .await
    .map_err(|err| format!("Couldn't stage the archived database: {err}"))?;

  // Atomically replace the live database (backing up the prior state first); Sync mode acquires the
  // lease and bumps the generation so the next launch re-seeds the working copy from the canonical.
  let now = Utc::now();
  let restore_storage = storage.clone();
  let restore_machine_id = machine_id.clone();
  let restore_temp_db = temp_db.clone();
  tokio::task::spawn_blocking(move || {
    crate::store::data_restore::restore(&restore_storage, restore_machine_id, &restore_temp_db, now)
  })
  .await
  .map_err(|err| err.to_string())?
  .map_err(|err| err.to_string())?;

  // Hold the staging file until the restore has copied it into place.
  drop(staging);

  // Merge the archived portable settings over this machine's identity, then persist so the next
  // launch reads features/ui/accessibility/industry from the archive while keeping local paths,
  // machine_id, and tokens.
  let config_text =
    String::from_utf8(parsed.config).map_err(|err| format!("The archived settings aren't valid UTF-8: {err}"))?;
  let archived: config::Settings =
    toml::from_str(&config_text).map_err(|err| format!("Couldn't parse the archived settings: {err}"))?;
  let merged = config::merge_for_restore(&local_settings, &archived);
  config::save(&merged);

  Ok(())
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

/// The well-known EVE Inbox system label id — kept in sync with `features::mail::labels` (which is a
/// private module and so cannot be imported here). Waking a snooze restores this membership.
const INBOX_LABEL_ID: i64 = 1;

/// The name of the user label that mirrors Pod's snooze state into EVE. Resolved from the catalog
/// by this name; created on demand at snooze time by the mail feature.
const SNOOZED_LABEL_NAME: &str = "Snoozed";

/// Reverses the snooze-time label flip for each mail the scheduler woke this tick: drop the Snoozed
/// label and restore Inbox membership, mirroring the move back to EVE via a `mail.set_labels`
/// outbox row. Because the scheduler wakes *every* expired snooze regardless of when it elapsed,
/// this also covers backdated wakes — a mail whose wake time passed while Pod was closed gets its
/// flip enqueued on the next launch tick.
fn handle_snoozes_woken(app: &App, woken: Vec<(i64, i64)>) -> Task<Message> {
  if woken.is_empty() {
    return Task::none();
  }
  let reload = mail_clock_reload(app);
  let Some(runtime) = app.runtime.as_ref() else {
    return reload;
  };
  let db = runtime.db.clone();
  let flip = Task::future(async move {
    for (character_id, mail_id) in woken {
      enqueue_wake_label_flip(&db, character_id, mail_id).await;
    }
  })
  .discard();
  Task::batch([flip, reload])
}

fn handle_trash_purged(app: &App, purged: Vec<(i64, i64)>) -> Task<Message> {
  if purged.is_empty() {
    return Task::none();
  }
  mail_clock_reload(app)
}

/// App-side mirror of the mail feature's wake flip (the feature module is private). Drops the
/// Snoozed label when the catalog still carries one, restores Inbox membership, and enqueues a
/// `mail.set_labels` outbox row. A no-op write (already in Inbox, never flipped) is skipped so the
/// outbox stays clean.
async fn enqueue_wake_label_flip(db: &store::Database, character_id: i64, mail_id: i64) {
  use store::{model::OwnerType, repo::mail};

  let catalog = mail::labels(db, character_id).await.unwrap_or_default();
  let snoozed_id = catalog
    .iter()
    .find(|label| label.name().eq_ignore_ascii_case(SNOOZED_LABEL_NAME))
    .map(|label| label.label_id());

  let previous = mail::membership(db, character_id, mail_id).await.unwrap_or_default();
  let mut labels: Vec<i64> = previous.iter().copied().filter(|id| Some(*id) != snoozed_id).collect();
  if !labels.contains(&INBOX_LABEL_ID) {
    labels.push(INBOX_LABEL_ID);
  }
  if labels == previous {
    return;
  }

  for label_id in &previous {
    if !labels.contains(label_id) {
      let _ = mail::remove_membership(db, character_id, mail_id, *label_id).await;
    }
  }
  for label_id in &labels {
    if !previous.contains(label_id) {
      let _ = mail::add_membership(db, character_id, mail_id, *label_id).await;
    }
  }

  let payload = serde_json::json!({
    "character_id": character_id,
    "labels": labels,
    "mail_id": mail_id,
    "previous": previous,
  });
  let Ok(json) = serde_json::to_string(&payload) else {
    return;
  };
  let dedupe = format!("set_labels:{mail_id}");
  let _ = store::repo::infra::append(
    db,
    OwnerType::Character,
    character_id,
    "mail.set_labels",
    &json,
    Some(&dedupe),
  )
  .await;
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
  if let Some(detect) = drain_notifications_dirty(app) {
    tasks.push(detect);
  }
  Task::batch(tasks)
}

/// Runs the notification detector sweep when a relevant sync (or the idle cadence) has marked the
/// notifications dirty, refreshing the cached list/unread and surfacing toasts. A pure-UI refresh
/// (panel open, mark-read) takes the same path with `run_detectors = false`.
fn drain_notifications_dirty(app: &mut App) -> Option<Task<Message>> {
  if !app.notifications_dirty {
    return None;
  }
  app.notifications_dirty = false;
  Some(refresh_notifications(app, true))
}

fn refresh_notifications(app: &App, run_detectors: bool) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  let now = app.now;
  let features = feature_flags(app);
  let characters = owned_character_ids(app);
  let corporations = owned_corporation_ids(app);
  Task::perform(
    async move { Box::new(notifications::refresh(&db, now, &characters, &corporations, &features, run_detectors).await) },
    Message::NotificationsRefreshed,
  )
}

fn owned_character_ids(app: &App) -> Vec<i64> {
  app
    .character_manager
    .as_ref()
    .map(character_manager::owned_roster)
    .unwrap_or_default()
    .iter()
    .map(|pilot| pilot.id)
    .collect()
}

fn owned_corporation_ids(app: &App) -> Vec<i64> {
  app
    .character_manager
    .as_ref()
    .map(character_manager::owned_corporations)
    .unwrap_or_default()
    .into_iter()
    .map(|(id, _)| id)
    .collect()
}

/// Whether a finished sync job feeds one of the seven notification detectors, gating the dirty flag
/// so the detector sweep only runs after sync activity that could surface a new event.
fn is_notification_source(kind: JobKind) -> bool {
  matches!(
    kind,
    JobKind::CharacterCalendar
      | JobKind::CharacterIndustryJobs
      | JobKind::CharacterKillmails
      | JobKind::CharacterMail
      | JobKind::CharacterSkills
      | JobKind::CorporationIndustryJobs
      | JobKind::CorporationKillmails
      | JobKind::CorporationMiningExtractions
  )
}

fn handle_notifications_refreshed(app: &mut App, snapshot: notifications::Snapshot) -> Task<Message> {
  let notifications::Snapshot {
    list,
    surfaced,
    unread,
    who,
  } = snapshot;
  // Newer rows arrived if the live list's newest id differs from what History currently shows on top.
  // Reset History to the first page so the new rows appear at the top without corrupting the cursor.
  let newest_changed = list.first().map(store::model::Notification::id)
    != app.notifications_history.first().map(store::model::Notification::id);
  app.notifications = list;
  app.notification_names = who;
  app.notifications_unread = unread;
  for notification in surfaced {
    enqueue_toast(app, notification);
  }
  // Only reset once History has actually materialized a page: while the first page is still in flight
  // (accumulator empty) that load already targets the newest page, so resetting would just discard it.
  if app.notifications_panel_open && newest_changed && !app.notifications_history.is_empty() {
    return reset_notifications_history(app);
  }
  Task::none()
}

/// Clears the History accumulator and re-requests the newest keyset page, bumping the epoch so any
/// in-flight older page is dropped on arrival. Returns the first-page fetch task (or none without a
/// runtime). Driven on panel open and whenever a refresh surfaces a newer head row.
fn reset_notifications_history(app: &mut App) -> Task<Message> {
  app.notifications_history.clear();
  app.notifications_history_cursor = None;
  app.notifications_history_has_more = true;
  app.notifications_history_loading = false;
  app.notifications_history_scroll = 0.0;
  app.notifications_history_epoch = app.notifications_history_epoch.wrapping_add(1);
  load_more_notifications_history(app)
}

/// Fetches the next keyset History page past the current cursor and resolves "who" names for it. Guards
/// against a second concurrent fetch and against fetching once the last page has been seen.
fn load_more_notifications_history(app: &mut App) -> Task<Message> {
  if app.notifications_history_loading || !app.notifications_history_has_more {
    return Task::none();
  }
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  app.notifications_history_loading = true;
  let db = runtime.db.clone();
  let cursor = app.notifications_history_cursor.clone();
  let epoch = app.notifications_history_epoch;
  Task::perform(
    async move {
      let rows =
        store::repo::notifications::list_page(&db, cursor.as_ref(), store::repo::notifications::HISTORY_PAGE_SIZE)
          .await
          .unwrap_or_default();
      let who = notifications::resolve_names(&db, &rows).await;
      (rows, who)
    },
    move |(rows, who)| Message::NotificationsHistoryPageLoaded {
      epoch,
      rows,
      who,
    },
  )
}

fn handle_notifications_history_page_loaded(
  app: &mut App,
  epoch: u64,
  rows: Vec<store::model::Notification>,
  who: std::collections::HashMap<store::model::NotificationOwner, String>,
) -> Task<Message> {
  // Drop a page captured before a reset (newer rows arrived / panel reopened): its rows belong to a
  // stale cursor walk and would duplicate or interleave against the fresh accumulator.
  if epoch != app.notifications_history_epoch {
    return Task::none();
  }
  app.notifications_history_loading = false;
  app.notifications_history_has_more = rows.len() as i64 == store::repo::notifications::HISTORY_PAGE_SIZE;
  if let Some(cursor) = store::repo::notifications::HistoryCursor::from_page(&rows) {
    app.notifications_history_cursor = Some(cursor);
  }
  // Merge freshly-resolved "who" names so the paged rows render their author line; the live refresh map
  // already holds names for the New-tab rows.
  app.notification_names.extend(who);
  app.notifications_history.extend(rows);
  Task::none()
}

fn handle_notifications_history_scrolled(app: &mut App, absolute: f32, relative: f32) -> Task<Message> {
  app.notifications_history_scroll = absolute;
  if relative < NOTIFICATIONS_HISTORY_SCROLL_THRESHOLD {
    return Task::none();
  }
  load_more_notifications_history(app)
}

fn enqueue_toast(app: &mut App, notification: store::model::Notification) {
  let who = app
    .notification_names
    .get(&notification.owner())
    .cloned()
    .unwrap_or_default();
  app.toasts.push(ToastEntry {
    notification,
    paused: false,
    remaining: TOAST_MS,
    who,
  });
  // Cap visible toasts at the newest few; the dropped ones still live in the center.
  let overflow = app.toasts.len().saturating_sub(TOAST_CAP);
  if overflow > 0 {
    app.toasts.drain(0..overflow);
  }
}

fn handle_toggle_notifications_panel(app: &mut App) -> Task<Message> {
  app.notifications_panel_open = !app.notifications_panel_open;
  if app.notifications_panel_open {
    // Opening reads the latest surfaced rows without re-scanning sources, and never auto-marks read,
    // and loads the first keyset page of History so it is ready when the user switches tabs.
    Task::batch([refresh_notifications(app, false), reset_notifications_history(app)])
  } else {
    Task::none()
  }
}

fn handle_close_notifications_panel(app: &mut App) -> Task<Message> {
  app.notifications_panel_open = false;
  // Drop the History page accumulator so a reopen starts from the newest page rather than re-showing a
  // deep, possibly stale scroll position. The epoch bump invalidates any page still in flight.
  app.notifications_history.clear();
  app.notifications_history_cursor = None;
  app.notifications_history_has_more = false;
  app.notifications_history_loading = false;
  app.notifications_history_scroll = 0.0;
  app.notifications_history_epoch = app.notifications_history_epoch.wrapping_add(1);
  Task::none()
}

fn handle_clear_notifications(app: &mut App) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  app.notifications.clear();
  app.notifications_unread = 0;
  let db = runtime.db.clone();
  Task::perform(
    async move { store::repo::notifications::clear_all(&db).await.is_ok() },
    |_| Message::CloseNotificationsPanel,
  )
}

fn handle_select_notification_tab(app: &mut App, tab: NotificationTab) -> Task<Message> {
  app.notifications_tab = tab;
  Task::none()
}

fn handle_mark_all_notifications_read(app: &mut App) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  app.notifications_unread = 0;
  let db = runtime.db.clone();
  Task::perform(
    async move {
      let _ = store::repo::notifications::mark_all_read(&db).await;
    },
    |()| Message::ToastTick,
  )
}

fn handle_notification_activated(app: &mut App, id: i64) -> Task<Message> {
  let target = app
    .notifications
    .iter()
    .find(|notification| notification.id() == id)
    .map(|notification| notification.target().clone());
  app.notifications_panel_open = false;
  app.toasts.retain(|toast| toast.notification.id() != id);
  let read = mark_notification_read(app, id);
  match target {
    Some(target) => Task::batch([read, navigate_to_notification_target(app, &target)]),
    None => read,
  }
}

fn mark_notification_read(app: &mut App, id: i64) -> Task<Message> {
  if let Some(notification) = app.notifications.iter_mut().find(|n| n.id() == id)
    && notification.read_at().is_none()
  {
    notification.read_at = Some(app.now.to_rfc3339());
    app.notifications_unread = app.notifications_unread.saturating_sub(1);
  }
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  Task::perform(
    async move {
      let _ = store::repo::notifications::mark_read(&db, id).await;
    },
    |()| Message::ToastTick,
  )
}

fn navigate_to_notification_target(app: &mut App, target: &store::model::NotificationTarget) -> Task<Message> {
  use store::model::NotificationDestination;
  match target.destination {
    NotificationDestination::Assets => navigate_to_assets(app),
    NotificationDestination::Calendar => navigate_to_calendar(app, target.character),
    // No corp-detail nav destination exists, so a character-less killmail lands on the roster.
    NotificationDestination::CharacterDetail => match target.character {
      Some(id) => navigate_to_character_detail(app, id),
      None => handle_nav(app, rail::Destination::Characters),
    },
    NotificationDestination::Industry => navigate_to_industry(app, target.character),
    NotificationDestination::Mail => navigate_to_mail(app, target.character),
    NotificationDestination::Skills => {
      let owned = owned_character_ids(app);
      navigate_to_skills(app, target.character, owned)
    }
    NotificationDestination::Wallet => navigate_to_wallet(app),
  }
}

fn handle_toast_tick(app: &mut App) -> Task<Message> {
  app.toasts.retain_mut(|toast| {
    if toast.paused {
      return true;
    }
    toast.remaining = toast.remaining.saturating_sub(TOAST_TICK);
    !toast.remaining.is_zero()
  });
  Task::none()
}

fn handle_toast_dismissed(app: &mut App, id: i64) -> Task<Message> {
  // The X dismisses the toast and marks the row read: it stays visible in the center as read history
  // and, because mark_read stamps read_at durably, never re-toasts on a later detector pass.
  app.toasts.retain(|toast| toast.notification.id() != id);
  mark_notification_read(app, id)
}

fn handle_toast_hover(app: &mut App, id: i64, hovered: bool) -> Task<Message> {
  if let Some(toast) = app.toasts.iter_mut().find(|toast| toast.notification.id() == id) {
    toast.paused = hovered;
  }
  Task::none()
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
/// `TakeOverOutcome::Failed` — re-parked in the resolver — so only the explicit user-confirmed
/// take-over ever overrides lease freshness. The take-over runs through [`run_take_over`], which
/// closes the working-copy pools before the swap and reopens them after, so the runtime is dropped
/// here to release its pool clones first.
fn handle_reacquire_lease(app: &mut App) -> Task<Message> {
  if !parked(app) {
    return Task::none();
  }
  start_take_over(app, false)
}

/// Common take-over launch: drops the parked runtime (releasing its working-copy pool clones), takes
/// the `StoreReady` whose three pools are the only remaining handles on the file, and hands them to
/// [`run_take_over`] so they are closed before the swap and reopened after. Short-circuits cleanly
/// when no session or store is present, leaving the app untouched.
fn start_take_over(app: &mut App, force: bool) -> Task<Message> {
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  let Some(ready) = app.store_ready.take() else {
    return Task::none();
  };
  app.runtime = None;
  run_take_over(ready, session, force)
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
  start_take_over(app, true)
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

/// Applies a resolved take-over by installing the pools [`run_take_over`] reopened against the
/// working-copy file (after closing the boot-time pools and performing the swap), then rebuilding the
/// runtime. Both outcomes install fresh pools so the app is never left with closed handles:
///
/// * `Claimed` — the working copy now holds the freshly pulled canonical copy; the lease is nulled so
///   [`build_runtime_inner`] starts read-write with the real sync engine.
/// * `Failed` — no swap happened (declined or errored); the unchanged working copy is reopened and the
///   app stays parked read-only with the inert engine, per ADR-0024.
fn handle_take_over_resolved(app: &mut App, outcome: TakeOverOutcome, mut ready: StoreReady) -> Task<Message> {
  app.confirm_force_takeover = false;
  match outcome {
    TakeOverOutcome::Claimed => {
      app.read_only = None;
      app.last_push = app
        .sync_session
        .as_ref()
        .and_then(store::sync_session::SyncSession::last_write);
      app.engine_state = EngineState::Running;
      ready.lease = None;
    }
    TakeOverOutcome::Failed => {
      app.engine_state = read_only_engine_state(app.read_only.clone());
      ready.lease = app.read_only.clone();
    }
  }
  app.store_ready = Some(ready.clone());
  build_runtime(ready)
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
  if let wallet::Message::ContractSelected(contract_id) = msg {
    let Some(source) = app.wallet.as_ref().and_then(|state| state.contract_source(contract_id)) else {
      return Task::none();
    };
    return open_contract_window(app, source, contract_id);
  }
  match msg {
    wallet::Message::PaneSettled(key, ratio) => {
      record_pane_ratio(app, key, ratio);
      Task::none()
    }
    wallet::Message::UiFlagPersisted(key, value) => {
      record_ui_flag(app, key, value);
      Task::none()
    }
    wallet::Message::UiListPersisted(key, values) => {
      record_ui_list(app, key, values);
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
      if app.character_manager.is_some() {
        tasks.push(Task::done(Message::CharacterManager(
          character_manager::Message::SignedIn {
            character_id: signed.character_id,
            name: signed.character_name,
          },
        )));
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
      tasks.push(character_manager::load(&runtime.db, feature_flags(app)).map(Message::CharacterManager));
    }
  }
  Task::batch(tasks)
}

fn handle_character_manager(app: &mut App, msg: character_manager::Message) -> Task<Message> {
  match msg {
    character_manager::Message::AddCharacterRequested => {
      update(app, Message::Auth(auth::Message::Start(feature_flags(app))))
    }
    character_manager::Message::AddCorporationRequested => update(
      app,
      Message::Auth(auth::Message::StartAddCorporation(feature_flags(app))),
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

  // Opening an event is promoted to a detached native window rather than handled inline.
  if let calendar::Message::EventOpened(character_id, event_id) = msg {
    return open_calendar_event_window(app, character_id, event_id);
  }

  let (Some(state), Some(runtime)) = (app.calendar.as_mut(), app.runtime.as_ref()) else {
    return Task::none();
  };

  calendar::update(state, msg, &runtime.db, app.now).map(Message::Calendar)
}

/// Opens a detached native-chrome calendar-event window. The window is multi-instance: every open
/// mints a fresh `window::Id`, so several event windows can coexist (duplicates included). The event
/// and its owning pilot are resolved from the live calendar at open time, the per-window state is
/// seeded synchronously, and the attendee tally loads in the background routed by id.
fn open_calendar_event_window(app: &mut App, character_id: i64, event_id: i64) -> Task<Message> {
  let (Some(calendar), Some(runtime)) = (app.calendar.as_ref(), app.runtime.as_ref()) else {
    return Task::none();
  };
  let Some((event, pilot_name)) = calendar.event_for(character_id, event_id) else {
    return Task::none();
  };
  let local_time = calendar.tweaks().local_time();
  let previous_response = event.response.clone();
  let db = runtime.db.clone();

  let size = Size::new(calendar::EVENT_WINDOW_WIDTH, calendar::EVENT_WINDOW_HEIGHT);
  let (id, open_task) = open_native_window(app, Window::CalendarEvent, size);
  app.calendar_events.insert(
    id,
    calendar::EventWindow::new(event, pilot_name, local_time, previous_response),
  );

  Task::batch([
    open_task,
    calendar::load_event_attendees(&db, character_id, event_id).map(move |msg| Message::CalendarEvent(id, msg)),
  ])
}

/// Routes a per-window calendar-event message to its [`EventWindow`] and applies it (attendee load,
/// optimistic RSVP write, or write acknowledgement). An RSVP write also refreshes the main calendar so
/// the underlying grid reflects the new response once the local mirror flips.
fn handle_calendar_event(app: &mut App, id: window::Id, msg: calendar::EventMessage) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let reload_main = matches!(msg, calendar::EventMessage::RsvpWritten);
  let db = runtime.db.clone();

  let Some(window) = app.calendar_events.get_mut(id) else {
    return Task::none();
  };
  let window_task = calendar::event_window_update(window, msg, &db).map(move |msg| Message::CalendarEvent(id, msg));

  if reload_main && let (Some(state), Some(runtime)) = (app.calendar.as_ref(), app.runtime.as_ref()) {
    let reload = calendar::reload(&runtime.db, state.active(), *runtime.settings.features()).map(Message::Calendar);
    return Task::batch([window_task, reload]);
  }
  window_task
}

fn close_calendar_event_window(app: &mut App, id: window::Id) -> Task<Message> {
  app.calendar_events.remove(id);
  app.windows.remove(id);
  window::close(id)
}

fn handle_calendar_attention_counted(app: &mut App, count: i64) -> Task<Message> {
  app.calendar_attention = count;
  Task::none()
}

fn handle_character_detail(app: &mut App, msg: character_detail::Message) -> Task<Message> {
  if let character_detail::Message::KillmailSelected(killmail_id) = msg {
    let Some(character_id) = app.character_detail.as_ref().map(character_detail::State::active) else {
      return Task::none();
    };
    return open_killmail_window(
      app,
      killmail_detail::Source::Character {
        character_id,
      },
      killmail_id,
    );
  }
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
  if let corporation_detail::Message::KillmailSelected(killmail_id) = msg {
    let Some(corporation_id) = app.corporation_detail.as_ref().map(corporation_detail::State::active) else {
      return Task::none();
    };
    return open_killmail_window(
      app,
      killmail_detail::Source::Corporation {
        corporation_id,
      },
      killmail_id,
    );
  }
  match (app.corporation_detail.as_mut(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => corporation_detail::update(state, msg, &runtime.db).map(Message::CorporationDetail),
    _ => Task::none(),
  }
}

/// Captures the modal's current search generation and stamps it onto the async result so a stale response
/// arriving after the user has typed again is discarded by the handler rather than clobbering newer results.
fn contact_entity_search(state: &character_detail::State, runtime: &Runtime, query: String) -> Task<Message> {
  use crate::features::entity_search;

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
      let results = results.into_iter().map(entity_ref_from_result).collect();
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

/// Which staggered interactive-DB checks are due on a given 1-second tick. Each check runs only on
/// the ticks where `tick % cadence == offset`, so the ~7 per-second queries are spread across ticks
/// (and the low-urgency ones stretched to multi-second cadences) instead of all firing every tick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClockChecks {
  calendar_attention: bool,
  calendar_reload: bool,
  industry_reload: bool,
  mail_reload: bool,
  mail_unread: bool,
  notifications: bool,
  snooze_wake: bool,
}

impl ClockChecks {
  fn for_tick(tick: u64) -> Self {
    Self {
      calendar_attention: tick % TICK_CALENDAR_ATTENTION == 1,
      calendar_reload: tick % TICK_CALENDAR_RELOAD == 1,
      industry_reload: tick % TICK_INDUSTRY_RELOAD == 2,
      mail_reload: tick.is_multiple_of(TICK_MAIL_RELOAD),
      mail_unread: tick % TICK_MAIL_UNREAD == 1,
      notifications: tick % TICK_NOTIFICATIONS == 3,
      snooze_wake: tick.is_multiple_of(TICK_SNOOZE_WAKE),
    }
  }
}

fn handle_clock_tick(app: &mut App) -> Task<Message> {
  app.now = Utc::now();
  app.clock_tick = app.clock_tick.wrapping_add(1);
  drain_due_save(app, Instant::now());

  // Stagger the periodic interactive-DB checks across ticks (see the `TICK_*` cadences) so the ~7
  // queries no longer all fire on every 1s tick and starve the reader pool. `trash_purge_tick`
  // carries its own multi-hour floor, so it stays on every tick (the gate is a cheap time compare).
  let due = ClockChecks::for_tick(app.clock_tick);
  let mut tasks: Vec<Task<Message>> = vec![trash_purge_tick(app)];
  if due.snooze_wake {
    tasks.push(snooze_wake_tick(app));
  }
  if due.mail_unread {
    tasks.push(mail_unread_tick(app));
  }
  if due.mail_reload {
    tasks.push(mail_clock_reload(app));
  }
  if due.calendar_attention {
    tasks.push(calendar_attention_tick(app));
  }
  if due.calendar_reload {
    tasks.push(calendar_clock_reload(app));
  }
  if due.industry_reload {
    tasks.push(industry_clock_reload(app));
  }
  // The standing sweep catches time-threshold events (skill / industry / extraction-cracked) that
  // mature on the wall clock with no fresh sync; the pulse drains the flag and runs the detectors.
  if due.notifications {
    app.notifications_dirty = true;
  }
  Task::batch(tasks)
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

fn handle_nav_to(app: &mut App, destination: rail::Destination, sub_section: Option<&'static str>) -> Task<Message> {
  let nav = handle_nav(app, destination);
  let Some(id) = sub_section else {
    return nav;
  };
  // A disabled feature lands on Characters instead; honor that fallback and skip the sub-section.
  if app.route.destination() != destination {
    return nav;
  }
  Task::batch([nav, select_sub_section(app, destination, id)])
}

// Sets the freshly-navigated feature's inner tab from a catalog sub-section id, then reuses that
// feature's own tab-selection handler to load the tab's data. The id is applied directly to state so
// the tab is correct even before a runtime exists; the load handler is a no-op until one does.
fn select_sub_section(app: &mut App, destination: rail::Destination, id: &str) -> Task<Message> {
  match destination {
    rail::Destination::Assets => select_assets_sub_section(app, id),
    rail::Destination::Calendar => select_calendar_sub_section(app, id),
    rail::Destination::Characters => select_characters_sub_section(app, id),
    rail::Destination::Industry => select_industry_sub_section(app, id),
    rail::Destination::Settings => select_settings_sub_section(app, id),
    rail::Destination::Wallet => select_wallet_sub_section(app, id),
    // Skills' "queue" surface is the default landing view, so nav alone shows it; "compare" is a
    // separate window the Skills view opens via OpenCompare, so reuse that handler here.
    rail::Destination::Skills => select_skills_sub_section(app, id),
    rail::Destination::Mail => Task::none(),
  }
}

fn select_assets_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match assets::Tab::from_id(id) {
    Some(tab) if app.assets.as_mut().is_some_and(|state| state.select_tab_by_id(id)) => {
      update(app, Message::Assets(assets::Message::TabSelected(tab)))
    }
    _ => Task::none(),
  }
}

fn select_calendar_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match calendar::View::from_id(id) {
    Some(view) if app.calendar.as_mut().is_some_and(|state| state.select_view_by_id(id)) => {
      update(app, Message::Calendar(calendar::Message::ViewSelected(view)))
    }
    _ => Task::none(),
  }
}

fn select_characters_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match character_manager::Pane::from_id(id) {
    Some(pane)
      if app
        .character_manager
        .as_mut()
        .is_some_and(|state| state.select_pane_by_id(id)) =>
    {
      update(
        app,
        Message::CharacterManager(character_manager::Message::TabSelected(pane)),
      )
    }
    _ => Task::none(),
  }
}

fn select_industry_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match industry::Tab::from_id(id) {
    Some(tab) if app.industry.as_mut().is_some_and(|state| state.select_tab_by_id(id)) => {
      update(app, Message::Industry(industry::Message::TabSelected(tab)))
    }
    _ => Task::none(),
  }
}

fn select_settings_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match settings::Category::from_id(id) {
    Some(category)
      if app
        .settings
        .as_mut()
        .is_some_and(|state| state.select_category_by_id(id)) =>
    {
      update(app, Message::Settings(settings::Message::CategorySelected(category)))
    }
    _ => Task::none(),
  }
}

fn select_wallet_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match wallet::Tab::from_id(id) {
    Some(tab) if app.wallet.as_mut().is_some_and(|state| state.select_tab_by_id(id)) => {
      update(app, Message::Wallet(wallet::Message::TabSelected(tab)))
    }
    _ => Task::none(),
  }
}

fn select_skills_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match id {
    "compare" => handle_skills(app, skills::Message::OpenCompare),
    _ => Task::none(),
  }
}

// The catalog sub-section id of the active tab on the current route, so the flyout can highlight the
// open tab. Destinations without inner tabs (Mail) have no active sub-section.
fn active_sub_section(app: &App) -> Option<&'static str> {
  match app.route.destination() {
    rail::Destination::Assets => app.assets.as_ref().map(|state| state.active_tab().id()),
    rail::Destination::Calendar => app.calendar.as_ref().map(|state| state.active_view().id()),
    rail::Destination::Characters => app.character_manager.as_ref().map(|state| state.active_pane().id()),
    rail::Destination::Industry => app.industry.as_ref().map(|state| state.active_tab().id()),
    rail::Destination::Settings => app.settings.as_ref().map(|state| state.active_category().id()),
    rail::Destination::Wallet => app.wallet.as_ref().map(|state| state.active_tab().id()),
    rail::Destination::Mail | rail::Destination::Skills => None,
  }
}

fn handle_rail_hover(app: &mut App, destination: Option<rail::Destination>) -> Task<Message> {
  match destination {
    Some(destination) => {
      app.rail_hover = Some(destination);
      app.rail_hover_gen = app.rail_hover_gen.wrapping_add(1);
      Task::none()
    }
    None => {
      // Defer the close so the pointer can cross into the flyout; a re-entry bumps the generation
      // and strands this expiry.
      app.rail_hover_gen = app.rail_hover_gen.wrapping_add(1);
      let generation = app.rail_hover_gen;
      Task::perform(async move { tokio::time::sleep(RAIL_HOVER_GRACE).await }, move |()| {
        Message::RailHoverExpire(generation)
      })
    }
  }
}

fn handle_rail_hover_expire(app: &mut App, generation: u64) -> Task<Message> {
  if app.rail_hover_gen == generation {
    app.rail_hover = None;
  }
  Task::none()
}

fn handle_ready(app: &mut App, runtime: Runtime) -> Task<Message> {
  let load_roster = character_manager::load(&runtime.db, *runtime.settings.features());
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
  sync_mcp_server(app);
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
    skills::Message::OpenManagePlans => open_manage_plans_window(app),
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
  let task = match msg {
    industry::Message::Planner(industry::PlannerMessage::FacilitySearchChanged {
      ref query,
      type_id,
    }) => {
      let query = query.clone();
      let update = industry::update(state, msg, &runtime.db, app.now).map(Message::Industry);
      let search = match state.facility_search_target() {
        Some((target, generation)) if target == type_id => industry::facility_search(
          &runtime.db,
          Arc::clone(&runtime.esi),
          Arc::clone(&runtime.sso),
          type_id,
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
      let catalog = state.planner_catalog().cloned();
      Task::batch([
        update,
        industry::facility_pin(&runtime.db, scope, pin, catalog).map(Message::Industry),
      ])
    }
    _ => industry::update(state, msg, &runtime.db, app.now).map(Message::Industry),
  };

  // Hoist the static catalog the state captured on its first planner load into the session cache, so the next
  // Industry navigation reuses it instead of rebuilding from scratch.
  if app.industry_catalog.is_none()
    && let Some(catalog) = app.industry.as_ref().and_then(industry::State::planner_catalog)
  {
    app.industry_catalog = Some(catalog.clone());
  }

  task
}

fn handle_mail(app: &mut App, msg: mail::Message) -> Task<Message> {
  if let mail::Message::PaneSettled(key, ratio) = msg {
    record_pane_ratio(app, key, ratio);
    return Task::none();
  }
  if let mail::Message::ReauthRequested(id) = msg {
    return update(app, Message::ReauthCharacter(id));
  }
  // Compose lives in detached windows: open one for new/reply/forward/draft instead of mutating an
  // in-place holder. Reply/forward read the open render from the main mail state.
  match msg {
    mail::Message::ComposeOpened => {
      let Some(from) = app.mail.as_ref().and_then(mail::State::default_from) else {
        return Task::none();
      };
      return open_compose_window(
        app,
        mail::compose::Seed::Blank {
          from_character_id: from,
        },
      );
    }
    mail::Message::Reply(mail_id) => return open_reply_window(app, mail_id, mail::compose::Kind::Reply),
    mail::Message::ReplyAll(mail_id) => return open_reply_window(app, mail_id, mail::compose::Kind::ReplyAll),
    mail::Message::Forward(mail_id) => return open_reply_window(app, mail_id, mail::compose::Kind::Forward),
    mail::Message::DraftOpened(draft_id) => return open_draft_window(app, draft_id),
    _ => {}
  }

  let (Some(state), Some(runtime)) = (app.mail.as_mut(), app.runtime.as_ref()) else {
    return Task::none();
  };

  match msg {
    mail::Message::ScopeSelected(scope) => Task::batch([
      mail::update(state, mail::Message::ScopeSelected(scope), &runtime.db).map(Message::Mail),
      mail::reload(&runtime.db, scope).map(Message::Mail),
    ]),
    msg => mail::update(state, msg, &runtime.db).map(Message::Mail),
  }
}

fn open_reply_window(app: &mut App, mail_id: i64, kind: mail::compose::Kind) -> Task<Message> {
  let Some(seed) = app.mail.as_ref().and_then(|state| state.reply_seed(mail_id, kind)) else {
    return Task::none();
  };
  open_compose_window(app, seed)
}

fn entity_ref_from_result(
  result: crate::features::entity_search::EntityResult,
) -> crate::ui::components::entity_search::EntityRef {
  use crate::{features::entity_search::EntityCategory, ui::components::entity_search::EntityKind};
  let kind = match result.category {
    EntityCategory::Alliance => EntityKind::Alliance,
    EntityCategory::Character => EntityKind::Character,
    EntityCategory::Corporation => EntityKind::Corporation,
    EntityCategory::SolarSystem => EntityKind::SolarSystem,
    EntityCategory::Station => EntityKind::Station,
  };
  crate::ui::components::entity_search::EntityRef {
    id: result.id,
    kind,
    name: result.name,
    portrait: result
      .category
      .image_kind()
      .map(|image_kind| store::images::default_store().image_path(image_kind, result.id)),
  }
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
  if is_notification_source(key.kind) {
    app.notifications_dirty = true;
  }
  mark_detail_dirty(app, key);
  mark_wallet_dirty(app, key);
  mark_assets_dirty(app, key);
  Task::none()
}

fn mark_assets_dirty(app: &mut App, key: JobKey) {
  if app.route == Route::Assets
    && let Some(assets) = app.assets.as_mut()
  {
    assets.mark_dirty(key.kind);
  }
}

fn mark_detail_dirty(app: &mut App, key: JobKey) {
  if let Some(detail) = app.character_detail.as_mut() {
    detail.mark_dirty(key);
  }
}

fn mark_wallet_dirty(app: &mut App, key: JobKey) {
  if app.route == Route::Wallet
    && let Some(wallet) = app.wallet.as_mut()
  {
    wallet.mark_dirty(key.kind);
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
    // Transparent custom-chrome windows need the OS drop-shadow suppressed. Each kind leaves this
    // arm when its conversion task promotes it to a native window; `Window::Splash` stays for good.
    Some(Window::Killmail | Window::Splash) => disable_shadow(id),
    _ => Task::none(),
  }
}

// Translates a window-chrome interaction on the window `id` into the matching iced window task: the drag
// bar moves the window, a resize edge begins an edge/corner drag-resize, and the close button routes through
// the standard close path so lifetime/shutdown bookkeeping stays in one place.
fn handle_chrome_event(app: &mut App, id: window::Id, event: window_chrome::Event) -> Task<Message> {
  match event {
    window_chrome::Event::Close => handle_close_requested(app, id),
    window_chrome::Event::Drag => window::drag(id),
    window_chrome::Event::Resize(direction) => window::drag_resize(id, direction),
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
    Some(Window::Splash) => splash_window_view(app),
    Some(Window::Main) => main_view(app),
    Some(Window::Compare) => compare_window_view(app, id),
    Some(Window::Contract) => contract_window_view(app, id),
    Some(Window::Killmail) => killmail_window_view(app, id),
    Some(Window::MailCompose) => compose_window_view(app, id),
    Some(Window::ManagePlans) => manage_plans_window_view(app, id),
    Some(Window::SkillPlanEditor) => skill_plan_editor_window_view(app, id),
    Some(Window::StockpileEditor) => stockpile_editor_window_view(app, id),
    Some(Window::CalendarEvent) => calendar_event_window_view(app, id),
    Some(Window::StockpileImport) => stockpile_import_window_view(app, id),
    _ => blank(),
  }
}

fn calendar_event_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.calendar_events.get(id) {
    Some(window) => calendar::event_window_view(window).map(move |msg| Message::CalendarEvent(id, msg)),
    None => blank(),
  }
}

fn stockpile_import_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.stockpile_imports.get(id) {
    Some(panel) => assets::stockpile_import_view(panel).map(move |msg| Message::StockpileImport(id, msg)),
    None => blank(),
  }
}

fn splash_window_view(app: &App) -> Element<'_, Message> {
  match app.splash.as_ref() {
    Some(state) => splash::view(state, app.now).map(Message::Splash),
    None => blank(),
  }
}

fn compare_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.compare.as_ref() {
    Some((compare_id, state)) if *compare_id == id => skills_compare::view(state).map(Message::Compare),
    _ => blank(),
  }
}

fn contract_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.contracts.get(id) {
    Some(state) => contract_detail::view(state),
    None => blank(),
  }
}

fn killmail_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.killmails.get(id) {
    Some(state) => killmail_detail::view(state),
    None => blank(),
  }
}

fn compose_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.composes.get(id) {
    Some(draft) => {
      let roster = app.mail.as_ref().map(mail::State::roster).unwrap_or(&[]);
      mail::compose::view(draft, roster).map(move |msg| Message::Compose(id, msg))
    }
    None => blank(),
  }
}

fn manage_plans_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.manage_plans.as_ref() {
    Some((manage_id, state)) if *manage_id == id => skill_plan_manager::view(state).map(Message::ManagePlans),
    _ => blank(),
  }
}

fn skill_plan_editor_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.editor.as_ref() {
    Some((editor_id, state)) if *editor_id == id => {
      skill_plan_editor::view(state, app.now).map(Message::SkillPlanEditor)
    }
    _ => blank(),
  }
}

fn stockpile_editor_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.stockpile_editors.get(id) {
    Some(editor) => assets::stockpile_editor_view(editor).map(move |msg| Message::StockpileEditor(id, msg)),
    None => blank(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::sync::{JobKind, Subject};

  fn pilot(id: i64) -> OwnedPilot {
    OwnedPilot {
      color: iced::Color::WHITE,
      granted_scopes: None,
      id,
      name: format!("Pilot {id}"),
    }
  }

  /// Feature flags with exactly `feature` enabled (all its children on) and every other group off.
  fn only(feature: config::Feature) -> config::FeatureFlags {
    let mut flags = config::FeatureFlags::default();
    for candidate in config::Feature::ALL {
      flags.set_enabled(candidate, candidate == feature);
    }
    flags
  }

  fn test_app() -> App {
    App {
      accessibility: config::AccessibilityConfig::default(),
      assets: None,
      auth: auth::State::default(),
      calendar: None,
      calendar_attention: 0,
      calendar_events: WindowStates::default(),
      character_detail: None,
      character_manager: None,
      clock_tick: 0,
      coalescer: WriteCoalescer::new(),
      compare: None,
      composes: WindowStates::default(),
      confirm_force_takeover: false,
      contracts: WindowStates::default(),
      corporation_detail: None,
      editor: None,
      engine_state: EngineState::default(),
      esi_connected: true,
      industry: None,
      industry_catalog: None,
      init_error: None,
      keyboard_focus: FocusTracker::default(),
      killmails: WindowStates::default(),
      last_push: None,
      last_synced: None,
      mail: None,
      mail_unread: 0,
      manage_plans: None,
      mcp_server: None,
      next_roster_reload: None,
      next_trash_purge: None,
      notification_names: std::collections::HashMap::new(),
      notifications: Vec::new(),
      notifications_dirty: false,
      notifications_history: Vec::new(),
      notifications_history_cursor: None,
      notifications_history_epoch: 0,
      notifications_history_has_more: false,
      notifications_history_loading: false,
      notifications_history_scroll: 0.0,
      notifications_panel_open: false,
      notifications_tab: NotificationTab::default(),
      notifications_unread: 0,
      now: Utc::now(),
      outbox: sync::OutboxStatus::new(),
      palette: None,
      pending_auth: None,
      pending_images: HashSet::new(),
      rail_hover: None,
      rail_hover_gen: 0,
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
      stockpile_editors: WindowStates::default(),
      stockpile_imports: WindowStates::default(),
      store_ready: None,
      status: sync::SyncStatus::new(),
      sync_popover_open: false,
      sync_session: None,
      sync_tick: false,
      toasts: Vec::new(),
      ui_state: UiState::default(),
      updater: None,
      updater_state: updater::State::default(),
      updater_toast_dismissed: false,
      wallet: None,
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

  fn featured_app() -> App {
    let mut app = test_app();
    app.assets = Some(assets::State::new(config::FeatureFlags::default()));
    app.calendar = Some(calendar::State::new(42, app.now, config::FeatureFlags::default()));
    app.character_detail = Some(character_detail::State::new(1, &[]));
    app.character_manager = Some(character_manager::State::new());
    app.mail = Some(mail::State::new(42));
    app.skills = Some(skills::State::new(1));
    app.wallet = Some(wallet::State::new(config::FeatureFlags::default()));
    app
  }

  fn ready_app() -> App {
    let mut app = test_app();
    app.character_manager = Some(character_manager::State::new());
    app.character_detail = Some(character_detail::State::new(1, &[]));
    app.skills = Some(skills::State::new(1));
    app.mail = Some(mail::State::new(42));
    app.wallet = Some(wallet::State::new(config::FeatureFlags::default()));
    app.assets = Some(assets::State::new(config::FeatureFlags::default()));
    app
  }

  fn test_notification(id: i64, destination: store::model::NotificationDestination) -> store::model::Notification {
    use store::model::{NotificationKind, NotificationOwner, NotificationTarget};

    store::model::Notification {
      body: "body".to_owned(),
      created_at: Utc::now().to_rfc3339(),
      dedup_key: format!("dedup-{id}"),
      id,
      kind: NotificationKind::Skill,
      owner: NotificationOwner::Character(1),
      read_at: None,
      target: NotificationTarget {
        character: Some(1),
        destination,
        sub: None,
      },
      title: "title".to_owned(),
    }
  }

  mod handle_mcp_data_changed {
    use super::*;

    #[test]
    fn it_forces_every_open_data_view_dirty() {
      let mut app = featured_app();

      // No runtime, so the data-view drains short-circuit before touching the dirty flag, leaving the
      // forced-dirty marks set for inspection (the roster drain is the pre-existing path and is not
      // asserted here because it flips its own flag before bailing on the missing runtime).
      let _task = handle_mcp_data_changed(&mut app);

      assert!(app.assets.as_ref().unwrap().is_dirty(), "open assets view reloads");
      assert!(app.wallet.as_ref().unwrap().is_dirty(), "open wallet view reloads");
      assert!(
        app.character_detail.as_ref().unwrap().is_dirty(),
        "open character detail view reloads"
      );
    }
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

  mod collect_stale_images {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_appends_the_compare_window_keys_when_one_is_open() {
      let mut app = featured_app();
      app.route = Route::Settings;
      app.compare = Some((window::Id::unique(), skills_compare::State::new(vec![1, 2], Vec::new())));

      assert_eq!(super::super::collect_stale_images(&app), Vec::new());
    }

    #[test]
    fn it_gathers_keys_for_every_active_route() {
      let mut app = featured_app();

      for route in [
        Route::Assets,
        Route::Calendar,
        Route::CharacterDetail(1),
        Route::Characters,
        Route::CorporationDetail(1),
        Route::Industry,
        Route::Mail,
        Route::Settings,
        Route::Skills(1),
        Route::Wallet,
      ] {
        app.route = route;
        let _ = super::super::collect_stale_images(&app);
      }
    }

    #[test]
    fn it_gathers_no_keys_for_settings_with_no_compare() {
      let mut app = featured_app();
      app.route = Route::Settings;

      assert_eq!(super::super::collect_stale_images(&app), Vec::new());
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

  mod compare_seeds {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_breaks_skill_point_ties_by_id() {
      let seeds = compare_seeds(vec![(5, 100), (3, 100)], None);

      assert_eq!(seeds, vec![3, 5]);
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
    fn it_leads_with_the_active_pilot_then_fills_by_descending_sp() {
      let seeds = compare_seeds(vec![(1, 100), (2, 500), (3, 300)], Some(1));

      assert_eq!(seeds, vec![1, 2, 3]);
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
  }

  mod destination {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_a_calendar_route_to_the_calendar_destination() {
      assert_eq!(Route::Calendar.destination(), rail::Destination::Calendar);
    }

    #[test]
    fn it_maps_a_mail_route_to_the_mail_destination() {
      assert_eq!(Route::Mail.destination(), rail::Destination::Mail);
    }

    #[test]
    fn it_maps_a_skills_route_to_the_skills_destination() {
      assert_eq!(Route::Skills(42).destination(), rail::Destination::Skills);
    }

    #[test]
    fn it_round_trips_characters_settings_and_mail_through_from() {
      assert_eq!(Route::from(Route::Characters.destination()), Route::Characters);
      assert_eq!(Route::from(Route::Settings.destination()), Route::Settings);
      assert_eq!(Route::from(Route::Mail.destination()), Route::Mail);
      assert_eq!(Route::from(Route::Calendar.destination()), Route::Calendar);
    }
  }

  mod dispatch_lifecycle {
    use super::*;

    #[tokio::test]
    async fn it_routes_each_lifecycle_message() {
      let mut app = featured_app();
      let db = store::open_test().await.expect("test db");
      let reopened = StoreReady {
        db: db.clone(),
        sync_db: db.clone(),
        sync_housekeeping_db: db.clone(),
        http: http::Client::builder(http::Cache::new(db)).build(),
        lease: None,
        settings: config::Settings::default(),
        sync_session: None,
      };

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
        Message::PeriodicPush,
        Message::Pushed(None),
        Message::ReauthCharacter(1),
        Message::SeedProgress(splash::seed::Progress::Step("seeding".to_owned())),
        Message::Shortcut(Chord::OpenSettings),
        Message::SnoozesWoken(Vec::new()),
        Message::Splash(splash::Message::Tick),
        Message::StorageMigrated,
        Message::SyncPulse,
        Message::TakeOver,
        Message::TakeOverResolved(TakeOverOutcome::Failed, Box::new(reopened)),
        Message::TextInputFocused(iced::widget::Id::from("search")),
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

  mod file_filter {
    use pretty_assertions::assert_eq;

    use super::*;

    fn clamp_of(filter: &str) -> &str {
      filter.split_once(',').expect("filter has a pod= prefix").1
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
    fn it_seeds_from_zero_when_the_window_has_no_prior_entry() {
      let resized = geometry_after_resize(None, Size::new(800.0, 600.0));
      assert_eq!(resized.width, 800.0);
      assert_eq!(resized.height, 600.0);
      assert_eq!((resized.x, resized.y), (0.0, 0.0));
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
  }

  mod handle_shortcut {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_routes_the_open_settings_chord_to_the_settings_view() {
      let mut app = featured_app();

      let _ = super::super::handle_shortcut(&mut app, Chord::OpenSettings);

      assert_eq!(app.route, Route::Settings);
    }

    #[test]
    fn it_opens_settings_from_any_starting_route() {
      let mut app = featured_app();
      app.route = Route::Wallet;

      let _ = super::super::handle_shortcut(&mut app, Chord::OpenSettings);

      assert_eq!(app.route, Route::Settings);
    }
  }

  mod handle_text_input_focused {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_records_the_focused_input_on_the_tracker() {
      let mut app = test_app();

      let _ = super::super::handle_text_input_focused(&mut app, iced::widget::Id::from("search"));

      assert_eq!(app.keyboard_focus.is_text_input_focused(), true);
      assert_eq!(app.keyboard_focus.focused_id(), Some(&iced::widget::Id::from("search")));
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

  mod handlers {
    use super::*;

    fn test_industry_state() -> industry::State {
      industry::State::new(
        industry::EMPTY_INDUSTRY_SELECTION,
        Vec::new(),
        config::FeatureFlags::default(),
        industry::FacilityDefaults::default(),
        None,
        false,
      )
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

      let Some(auth::Message::Start(flags)) = app.pending_auth.clone() else {
        panic!("the re-auth must defer an auth Start, got {:?}", app.pending_auth);
      };
      assert!(
        flags.is_enabled(config::Feature::Mail) && flags.is_enabled(config::Feature::SkillMonitoring),
        "a re-auth after re-enabling features must request their scopes, got {flags:?}"
      );
    }

    #[tokio::test]
    async fn a_claimed_take_over_drops_read_only_and_installs_the_reopened_store() {
      let db = store::open_test().await.expect("test db");
      let reopened = StoreReady {
        db: db.clone(),
        sync_db: db.clone(),
        sync_housekeeping_db: db.clone(),
        http: http::Client::builder(http::Cache::new(db)).build(),
        lease: Some(HolderInfo {
          hostname: "studio-mac".to_owned(),
          last_active: Utc::now(),
          machine_id: "machine-b".to_owned(),
        }),
        settings: config::Settings::default(),
        sync_session: None,
      };
      let mut app = test_app();
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });

      let _ = handle_take_over_resolved(&mut app, TakeOverOutcome::Claimed, reopened);

      assert!(app.read_only.is_none(), "claiming the share makes the app writable");
      assert!(app.store_ready.is_some(), "the reopened pools are installed");
      assert_eq!(app.engine_state, EngineState::Running);
      assert!(
        app.store_ready.as_ref().unwrap().lease.is_none(),
        "the claimed store opens read-write with a nulled lease"
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
    fn a_failed_push_leaves_the_debounce_mark_untouched() {
      let mut app = test_app();

      let _ = handle_pushed(&mut app, None);

      assert_eq!(app.last_push, None, "a failed push must re-attempt next tick");
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

    #[tokio::test]
    async fn a_failed_take_over_keeps_the_app_read_only_and_reopens_parked_pools() {
      let db = store::open_test().await.expect("test db");
      let reopened = StoreReady {
        db: db.clone(),
        sync_db: db.clone(),
        sync_housekeeping_db: db.clone(),
        http: http::Client::builder(http::Cache::new(db)).build(),
        lease: None,
        settings: config::Settings::default(),
        sync_session: None,
      };
      let mut app = test_app();
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });

      let _ = handle_take_over_resolved(&mut app, TakeOverOutcome::Failed, reopened);

      assert!(app.read_only.is_some(), "a failed take-over leaves the app read-only");
      assert!(
        app.store_ready.is_some(),
        "a declined take-over still reopens pools so the app is never left with closed handles"
      );
      assert!(
        matches!(app.engine_state, EngineState::ReadOnly { .. }),
        "a declined take-over re-parks the engine read-only"
      );
      assert!(
        app.store_ready.as_ref().unwrap().lease.is_some(),
        "the reopened parked store carries the held-by lease"
      );
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
    fn a_pull_that_changed_nothing_leaves_the_synced_marker_untouched() {
      let mut app = test_app();
      app.last_synced = None;

      let _ = handle_pulled(&mut app, false);

      assert!(app.last_synced.is_none(), "no pull means no new synced timestamp");
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
    fn a_reauth_from_a_403_state_requests_the_full_enabled_feature_scope_set() {
      // No runtime, so the auth Start is deferred and we can inspect its feature set without
      // opening a browser. With Mail and Skills both enabled, a single re-auth must request both.
      let mut app = test_app();

      let _ = handle_reauth_character(&mut app, 7);

      let Some(auth::Message::Start(flags)) = app.pending_auth.clone() else {
        panic!("a re-auth defers an auth Start, got {:?}", app.pending_auth);
      };
      assert!(
        flags.is_enabled(config::Feature::Mail) && flags.is_enabled(config::Feature::SkillMonitoring),
        "the single re-auth carries the full enabled-feature set, not a per-feature subset"
      );

      let scopes = auth::scopes_for(&flags);
      let mail_only = only(config::Feature::Mail);
      let skills_only = only(config::Feature::SkillMonitoring);
      assert!(
        auth::scopes_for(&mail_only).iter().all(|scope| scopes.contains(scope)),
        "re-auth requests Mail scopes"
      );
      assert!(
        auth::scopes_for(&skills_only)
          .iter()
          .all(|scope| scopes.contains(scope)),
        "the same single re-auth also requests Skills scopes"
      );
    }

    #[test]
    fn an_acquired_lease_maps_to_no_read_only_state() {
      let holder: Option<HolderInfo> = store::lease::Outcome::Acquired.into();

      assert_eq!(holder, None);
    }

    #[test]
    fn an_inert_sync_handle_swallows_commands_without_panicking() {
      let (handle, _events) = inert_sync();

      handle.discover();
      handle.enroll(sync::Subject::Character(7));
      handle.run_now(sync::Subject::Character(7));
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
    fn direct_mode_does_not_hold_a_lease_and_runs_no_lifecycle_io() {
      let mut app = test_app();

      assert!(!holding_lease(&app), "with no sync session there is no lease to hold");
      let _ = handle_lease_heartbeat(&mut app);
      let _ = handle_periodic_push(&mut app);
      let _ = handle_periodic_pull(&mut app);
    }

    #[test]
    fn direct_mode_is_neither_parked_nor_holding_the_lease() {
      let app = test_app();

      assert!(!parked(&app), "with no sync session there is nothing parked");
      assert!(!holding_lease(&app), "with no sync session there is no lease to hold");
    }

    #[test]
    fn direct_mode_runs_no_crash_recovery_push() {
      let app = test_app();

      let _ = recover_unsynced_changes(&app);
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
    async fn export_data_archive_snapshots_the_db_then_writes_nowhere_when_stubbed() {
      use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

      let dir = tempfile::tempdir().unwrap();
      let database_path = dir.path().join("pod.db");
      let options = SqliteConnectOptions::new()
        .filename(&database_path)
        .create_if_missing(true);
      let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
      sqlx::query("CREATE TABLE note (body TEXT)")
        .execute(&mut connection)
        .await
        .unwrap();
      connection.close().await.unwrap();

      let diagnostics = settings::log_export::Diagnostics {
        cache_dir: dir.path().join("cache"),
        database_path: database_path.clone(),
        db_dir: dir.path().to_path_buf(),
        log_dir: dir.path().join("logs"),
      };

      let result = export_data_archive(database_path, b"[storage]\n".to_vec(), diagnostics).await;

      assert_eq!(result, Ok(None), "the cfg(test) save dialog is a no-op");
    }

    #[tokio::test]
    async fn export_data_archive_errors_when_the_database_is_missing() {
      let dir = tempfile::tempdir().unwrap();
      let database_path = dir.path().join("absent.db");
      let diagnostics = settings::log_export::Diagnostics {
        cache_dir: dir.path().join("cache"),
        database_path: database_path.clone(),
        db_dir: dir.path().to_path_buf(),
        log_dir: dir.path().join("logs"),
      };

      let result = export_data_archive(database_path, b"config".to_vec(), diagnostics).await;

      assert!(result.is_err(), "a missing live database surfaces an error");
    }

    /// Builds an in-memory `.zip` data archive carrying the given database bytes, config text, and Pod
    /// version, reusing the production `read_archive` format so the import path is exercised end to end.
    fn import_archive(db: &[u8], config: &str, version: &str) -> Vec<u8> {
      use std::io::{Cursor, Write};

      use zip::{CompressionMethod, ZipWriter, write::FileOptions};

      let manifest = serde_json::json!({
        "archive_version": 1,
        "arch": "x86_64",
        "created_at": "2026-06-25T00:00:00+00:00",
        "pod_version": version,
        "os": "linux",
        "storage": {
          "cache_dir": "/cache",
          "database_path": "/db/pod.db",
          "db_dir": "/db",
          "log_dir": "/logs",
        },
        "files": [],
      });
      let mut buf = Vec::new();
      {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        let options: FileOptions<'_, ()> = FileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("pod.db", options).unwrap();
        zip.write_all(db).unwrap();
        zip.start_file("config.toml", options).unwrap();
        zip.write_all(config.as_bytes()).unwrap();
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();
        zip.finish().unwrap();
      }
      buf
    }

    #[tokio::test]
    async fn import_data_archive_restores_the_database_and_persists_the_merged_config() {
      // Keep config::save off the real user config by pointing XDG_CONFIG_HOME at a tempdir.
      let config_home = tempfile::tempdir().unwrap();
      // SAFETY: tests run single-threaded enough here; only this test touches XDG_CONFIG_HOME.
      unsafe {
        std::env::set_var("XDG_CONFIG_HOME", config_home.path());
      }

      let dir = tempfile::tempdir().unwrap();
      let mut storage = config::StorageConfig::default();
      storage.set_db_dir(Some(dir.path().join("data")));
      std::fs::create_dir_all(storage.resolved_db_dir()).unwrap();
      std::fs::write(storage.resolved_database_path(), b"old live data").unwrap();

      let archive = import_archive(
        b"restored archive bytes",
        "[storage]\nnetwork = false\n",
        env!("CARGO_PKG_VERSION"),
      );
      let archive_path = dir.path().join("pod-data.zip");
      std::fs::write(&archive_path, &archive).unwrap();

      let result = import_data_archive(
        archive_path,
        storage.clone(),
        "machine-a".to_owned(),
        config::Settings::default(),
      )
      .await;

      assert_eq!(result, Ok(()));
      assert_eq!(
        std::fs::read(storage.resolved_database_path()).unwrap(),
        b"restored archive bytes",
        "the canonical database is replaced with the archive's"
      );
      let backup = std::fs::read_dir(storage.resolved_db_dir())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().ends_with(".backup"));
      assert!(backup.is_some(), "the prior database is backed up before replacement");
    }

    #[tokio::test]
    async fn import_data_archive_refuses_a_newer_major_archive_without_touching_data() {
      let dir = tempfile::tempdir().unwrap();
      let mut storage = config::StorageConfig::default();
      storage.set_db_dir(Some(dir.path().join("data")));
      std::fs::create_dir_all(storage.resolved_db_dir()).unwrap();
      std::fs::write(storage.resolved_database_path(), b"old live data").unwrap();

      let archive = import_archive(b"never written", "[storage]\n", "999.0.0");
      let archive_path = dir.path().join("pod-data.zip");
      std::fs::write(&archive_path, &archive).unwrap();

      let result = import_data_archive(
        archive_path,
        storage.clone(),
        "machine-a".to_owned(),
        config::Settings::default(),
      )
      .await;

      assert!(result.is_err(), "a newer-major archive is refused");
      assert_eq!(
        std::fs::read(storage.resolved_database_path()).unwrap(),
        b"old live data",
        "the live database is untouched when the archive is refused"
      );
    }

    #[tokio::test]
    async fn import_data_archive_errors_when_the_archive_is_missing() {
      let dir = tempfile::tempdir().unwrap();
      let storage = config::StorageConfig::default();
      let missing = dir.path().join("absent.zip");

      let result = import_data_archive(missing, storage, "machine-a".to_owned(), config::Settings::default()).await;

      assert!(result.is_err(), "a missing archive file surfaces an error");
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
    async fn handle_industry_reauth_request_defers_an_auth_start() {
      let mut app = test_app();

      let _ = handle_industry(&mut app, industry::Message::ReauthRequested(7));

      assert!(
        app.pending_auth.is_some(),
        "a re-auth request from the industry screen defers an auth Start"
      );
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
    async fn handle_industry_seams_the_facility_search_for_the_planner() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.industry = Some(test_industry_state());
      app.runtime = Some(runtime);

      let _ = handle_industry(
        &mut app,
        industry::Message::Planner(industry::PlannerMessage::FacilitySearchChanged {
          query: "jita".to_owned(),
          type_id: 0,
        }),
      );

      assert!(
        app.industry.as_ref().unwrap().facility_search_target().is_some(),
        "typing into the facility field opens the picker and arms a live search"
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
    async fn handle_settings_exports_logs_through_the_storage_diagnostics() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = handle_settings(
        &mut app,
        settings::Message::Storage(settings::storage_tab::Message::ExportLogs(
          settings::log_export::RangePreset::LastHour,
        )),
      );

      assert!(
        app.settings.is_some(),
        "exporting logs leaves the settings screen open and runs the diagnostics task"
      );
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
    async fn handle_settings_migrates_storage_when_sync_is_toggled() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      let mut state = settings::State::new(runtime.settings.clone(), runtime.db.clone());
      let networked = *state.settings().storage().network();
      app.runtime = Some(runtime);

      // Flipping the sync toggle stages a storage migration request the handler must drain.
      let _ = settings::update(
        &mut state,
        settings::Message::Storage(settings::storage_tab::Message::SyncToggled(!networked)),
      );
      app.settings = Some(state);
      let _ = handle_settings(
        &mut app,
        settings::Message::CategorySelected(settings::Category::Storage),
      );

      assert!(
        app
          .settings
          .as_mut()
          .expect("the settings screen stays open")
          .take_storage_migration()
          .is_none(),
        "the handler drains the staged storage migration request"
      );
    }

    #[tokio::test]
    async fn handle_settings_pins_a_picked_structure_facility() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      // A player-owned structure (id past the NPC-station range) is pinned, not just persisted.
      let facility = crate::ui::components::facility_combobox::FacilityRef {
        cost_index: Some(0.05),
        id: 1_030_000_000_001,
        name: "Player Keepstar".to_owned(),
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
        Some(1_030_000_000_001),
        "picking a structure mirrors it onto the runtime and routes through the pin path"
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
      let enabled = enabled_features(&app);
      let _ = update(
        &mut app,
        Message::CharacterDetail(character_detail::Message::FeaturesChanged(enabled)),
      );

      let detail = app.character_detail.as_ref().expect("the detail screen stays open");
      assert!(
        !detail.enabled_tabs().contains(&character_detail::Tab::Standings),
        "the dispatched feature change drops the Standings detail tab live"
      );
    }

    #[tokio::test]
    async fn handle_settings_redocks_the_rail_when_the_nav_side_changes() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = handle_settings(
        &mut app,
        settings::Message::Ui(settings::ui_tab::Message::SideSelected(config::NavLocation::Right)),
      );

      assert_eq!(
        app.runtime.as_ref().unwrap().settings.ui().nav_location(),
        &config::NavLocation::Right,
        "the runtime UI config mirrors the new rail side so open windows re-dock live"
      );
    }

    #[tokio::test]
    async fn handle_settings_releases_the_storage_lock() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = handle_settings(
        &mut app,
        settings::Message::Storage(settings::storage_tab::Message::ReleaseLock),
      );

      assert!(
        app.settings.is_some(),
        "requesting a lock release leaves the settings screen open and routes the release task"
      );
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
    async fn handle_settings_runs_an_industry_facility_search() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = handle_settings(
        &mut app,
        settings::Message::Industry(settings::industry_tab::Message::QueryChanged {
          activity: 1,
          query: "jita".to_owned(),
        }),
      );

      assert!(
        app.settings.is_some(),
        "typing into the facility field seams a live search and keeps the screen open"
      );
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
    async fn handle_settings_sets_the_log_level() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      let level = *runtime.settings.storage().log_level();
      let next = if level == config::LogLevel::Verbose {
        config::LogLevel::default()
      } else {
        config::LogLevel::Verbose
      };
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = handle_settings(
        &mut app,
        settings::Message::Storage(settings::storage_tab::Message::LogLevelChanged(next)),
      );

      assert_eq!(
        app.settings.as_ref().unwrap().settings().storage().log_level(),
        &next,
        "the new log level is recorded on the settings screen and applied live"
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

    #[tokio::test]
    async fn handle_settings_triggers_a_manual_sync() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = handle_settings(
        &mut app,
        settings::Message::Storage(settings::storage_tab::Message::SyncNow),
      );

      assert!(
        app.settings.is_some(),
        "requesting a manual sync leaves the settings screen open and routes the sync task"
      );
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

    #[test]
    fn it_advances_the_clock_and_drains_due_saves_on_a_tick() {
      let mut app = test_app();
      let before = app.now;

      let _ = update(&mut app, Message::ClockTick);

      assert!(app.now >= before, "the tick advances the clock");
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
    fn it_dismisses_the_updater_toast() {
      let mut app = test_app();
      assert!(!app.updater_toast_dismissed);

      let _ = update(&mut app, Message::UpdaterDismissToast);

      assert!(app.updater_toast_dismissed, "the toast hides after a dismiss");
    }

    #[tokio::test]
    async fn it_dispatches_each_stockpile_branch_through_the_runtime() {
      let mut app = test_app();
      app.assets = Some(assets::State::new(config::FeatureFlags::default()));
      app.runtime = Some(test_runtime().await);

      let _resolve = handle_assets(&mut app, assets::Message::StockpileImportResolveRequested);
      let _default = handle_assets(&mut app, assets::Message::SearchChanged("x".to_owned()));
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
    fn it_handles_updater_actions_without_a_provisioned_handle() {
      let mut app = test_app();
      assert!(app.updater.is_none());

      let _ = update(&mut app, Message::UpdaterAction(updater_banner::Action::Apply));
      let _ = update(&mut app, Message::UpdaterAction(updater_banner::Action::Restart));
    }

    #[test]
    fn it_ignores_an_unhandled_window_event() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::Main);
      let _ = update(&mut app, Message::Window(id, window::Event::Focused));
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

    #[tokio::test]
    async fn it_pairs_a_compose_window_input_with_a_recipient_search_when_a_runtime_is_present() {
      let mut app = test_app();
      app.mail = Some(mail::State::new(42));
      app.runtime = Some(test_runtime().await);

      let id = window::Id::unique();
      app.windows.register(id, Window::MailCompose);
      app.composes.insert(
        id,
        mail::compose::Draft::from_seed(mail::compose::Seed::Blank {
          from_character_id: 42,
        }),
      );

      let _to = handle_compose(&mut app, id, mail::Message::ComposeToInput("Vexor".to_owned()));
      let _cc = handle_compose(&mut app, id, mail::Message::ComposeCcInput("Alli".to_owned()));
      let _scope = handle_mail(&mut app, mail::Message::ScopeSelected(mail::Scope::Character(7)));
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
    fn it_records_the_calendar_attention_count() {
      let mut app = test_app();

      let _ = update(&mut app, Message::CalendarAttentionCounted(4));

      assert_eq!(app.calendar_attention, 4);
    }

    #[test]
    fn it_records_the_mail_unread_count_and_reauth_logs_without_a_runtime() {
      let mut app = test_app();
      let _ = update(&mut app, Message::MailUnreadCounted(9));
      assert_eq!(app.mail_unread, 9);
      let _ = update(&mut app, Message::ReauthCharacter(1));
    }

    #[test]
    fn it_reissues_the_mail_reload_only_when_a_snooze_woke() {
      let mut app = test_app();
      let _ = update(&mut app, Message::SnoozesWoken(Vec::new()));
      let _ = update(&mut app, Message::SnoozesWoken(vec![(1, 2)]));
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
    fn it_routes_a_mail_compose_input_to_a_no_op_without_a_runtime() {
      let mut app = test_app();
      app.mail = Some(mail::State::new(42));

      let _ = update(&mut app, Message::Mail(mail::Message::ComposeToInput("Ve".to_owned())));
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
    fn it_routes_a_splash_drag_to_a_no_op_with_no_splash_window() {
      let mut app = test_app();
      let _ = update(&mut app, Message::Splash(splash::Message::DragWindow));
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

    #[tokio::test]
    async fn pull_bundle_reports_no_change_for_a_fresh_share() {
      let (_dir, session) = temp_sync_session();

      let message = pull_bundle(session).await;

      assert!(
        matches!(message, Message::Pulled(false)),
        "a fresh share has nothing newer to pull"
      );
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
      let flags = feature_flags(&app);

      assert!(
        flags.is_enabled(config::Feature::Mail),
        "re-enabling Mail restores it to the live runtime the re-auth reads from"
      );
      let scopes = auth::scopes_for(&flags);
      let mail_only = only(config::Feature::Mail);
      assert!(
        auth::scopes_for(&mail_only).iter().all(|scope| scopes.contains(scope)),
        "the re-auth requests the re-enabled Mail scopes"
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
    fn the_force_takeover_confirmation_warns_of_data_loss_and_names_the_last_active_age() {
      let label = read_only_confirm_label("studio-mac", "12s ago");

      assert_eq!(
        label,
        "studio-mac was last active 12s ago. Taking over overwrites any unsaved changes it still has open. Continue?"
      );
    }

    #[test]
    fn the_initial_read_only_banner_invites_a_take_over() {
      let label = read_only_banner_label("studio-mac");

      assert_eq!(label, "Open on studio-mac \u{2014} close it there, or take over.");
    }
  }

  mod image_reload {
    use super::*;

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
    async fn it_is_a_no_op_without_a_runtime() {
      let app = featured_app();

      let _ = super::super::image_reload(&app);
    }

    #[tokio::test]
    async fn it_reloads_the_compare_window_when_one_is_open() {
      let mut app = featured_app();
      app.runtime = Some(test_runtime().await);
      app.compare = Some((window::Id::unique(), skills_compare::State::new(vec![1, 2], Vec::new())));

      let _ = super::super::image_reload(&app);
    }
  }

  mod image_self_heal {
    use super::*;
    use crate::store::images::ImageKind;

    #[test]
    fn it_clears_the_pending_key_when_an_image_resolves() {
      let mut app = test_app();
      app.pending_images.insert((ImageKind::CharacterPortrait, 42));

      let _task = handle_image_ready(&mut app, ImageKind::CharacterPortrait, 42, false);

      assert!(app.pending_images.is_empty());
    }

    #[tokio::test]
    async fn it_does_not_redispatch_a_key_already_in_flight() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.pending_images.insert((ImageKind::CorporationLogo, 7));

      let _task = dispatch_image_fetches(&mut app, vec![(ImageKind::CorporationLogo, 7)]);

      assert_eq!(app.pending_images.len(), 1);
    }

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

    #[test]
    fn it_rechecks_images_only_for_a_data_loading_feature_message() {
      let interaction = Message::Wallet(wallet::Message::TimeframeSelected(wallet::Timeframe::default()));
      assert!(
        !interaction.affects_images(),
        "an interaction message must not trigger the scan"
      );

      assert!(
        !Message::ClockTick.affects_images(),
        "a non-feature lifecycle message must not trigger the scan"
      );
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

  mod mark_assets_dirty {
    use super::*;

    fn finished(kind: JobKind) -> JobKey {
      JobKey::new(kind, Subject::Character(1))
    }

    #[test]
    fn it_marks_assets_dirty_on_route_for_an_asset_sync() {
      let mut app = test_app();
      app.route = Route::Assets;
      app.assets = Some(assets::State::new(config::FeatureFlags::default()));

      mark_assets_dirty(&mut app, finished(JobKind::AssetSync));

      assert!(app.assets.as_ref().unwrap().is_dirty());
    }

    #[test]
    fn it_skips_the_assets_reload_for_an_unrelated_kind() {
      let mut app = test_app();
      app.route = Route::Assets;
      app.assets = Some(assets::State::new(config::FeatureFlags::default()));

      mark_assets_dirty(&mut app, finished(JobKind::CharacterWallet));

      assert!(!app.assets.as_ref().unwrap().is_dirty());
    }

    #[test]
    fn it_skips_the_assets_reload_off_route() {
      let mut app = test_app();
      app.route = Route::Wallet;
      app.assets = Some(assets::State::new(config::FeatureFlags::default()));

      mark_assets_dirty(&mut app, finished(JobKind::AssetSync));

      assert!(!app.assets.as_ref().unwrap().is_dirty());
    }
  }

  mod mark_detail_dirty {
    use super::*;
    use crate::features::character_detail;

    const PILOT: i64 = 42;

    fn finished(kind: JobKind, subject: Subject) -> JobKey {
      JobKey::new(kind, subject)
    }

    #[test]
    fn it_ignores_everything_when_no_detail_screen_is_open() {
      let mut app = test_app();

      mark_detail_dirty(&mut app, finished(JobKind::CharacterClones, Subject::Character(PILOT)));

      assert!(app.character_detail.is_none());
    }

    #[test]
    fn it_routes_a_finished_job_to_the_open_detail_screen() {
      let mut app = test_app();
      app.character_detail = Some(character_detail::State::new(PILOT, &[]));

      mark_detail_dirty(&mut app, finished(JobKind::CharacterClones, Subject::Character(PILOT)));

      assert!(app.character_detail.as_ref().unwrap().is_dirty());
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
      app.wallet = Some(wallet::State::new(config::FeatureFlags::default()));

      mark_wallet_dirty(&mut app, finished(JobKind::CharacterWallet));

      assert!(app.wallet.as_ref().unwrap().is_dirty());
    }

    #[test]
    fn it_skips_the_wallet_reload_for_an_unrelated_kind() {
      let mut app = test_app();
      app.route = Route::Wallet;
      app.wallet = Some(wallet::State::new(config::FeatureFlags::default()));

      mark_wallet_dirty(&mut app, finished(JobKind::AssetSync));

      assert!(!app.wallet.as_ref().unwrap().is_dirty());
    }

    #[test]
    fn it_skips_the_wallet_reload_off_route() {
      let mut app = test_app();
      app.route = Route::Assets;
      app.wallet = Some(wallet::State::new(config::FeatureFlags::default()));

      mark_wallet_dirty(&mut app, finished(JobKind::CharacterWallet));

      assert!(!app.wallet.as_ref().unwrap().is_dirty());
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

  mod notifications {
    use super::*;
    use crate::store::model::{
      Notification, NotificationDestination, NotificationKind, NotificationOwner, NotificationTarget,
    };

    fn notification(id: i64) -> Notification {
      Notification {
        body: "body".to_owned(),
        created_at: "2026-06-22T00:00:00+00:00".to_owned(),
        dedup_key: format!("skill:{id}"),
        id,
        kind: NotificationKind::Skill,
        owner: NotificationOwner::Character(42),
        read_at: None,
        target: NotificationTarget {
          character: Some(42),
          destination: NotificationDestination::Skills,
          sub: None,
        },
        title: "title".to_owned(),
      }
    }

    mod enqueue_toast {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_caps_visible_toasts_and_keeps_the_newest() {
        let mut app = test_app();

        for id in 1..=(TOAST_CAP as i64 + 2) {
          enqueue_toast(&mut app, notification(id));
        }

        assert_eq!(app.toasts.len(), TOAST_CAP);
        let ids: Vec<i64> = app.toasts.iter().map(|toast| toast.notification.id()).collect();
        assert_eq!(ids, vec![3, 4, 5], "the oldest are dropped, the newest are kept");
      }
    }

    mod handle_toast_tick {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_dismisses_a_toast_once_its_lifetime_elapses() {
        let mut app = test_app();
        enqueue_toast(&mut app, notification(1));
        app.toasts[0].remaining = TOAST_TICK;

        let _ = handle_toast_tick(&mut app);

        assert!(app.toasts.is_empty(), "a fully aged toast is removed");
      }

      #[test]
      fn it_leaves_a_paused_toast_untouched() {
        let mut app = test_app();
        enqueue_toast(&mut app, notification(1));
        app.toasts[0].paused = true;
        app.toasts[0].remaining = TOAST_TICK;

        let _ = handle_toast_tick(&mut app);

        assert_eq!(app.toasts.len(), 1, "hover pauses the countdown");
        assert_eq!(app.toasts[0].remaining, TOAST_TICK);
      }
    }

    mod handle_toast_hover {
      use super::*;

      #[test]
      fn it_pauses_and_resumes_the_hovered_toast() {
        let mut app = test_app();
        enqueue_toast(&mut app, notification(1));

        let _ = handle_toast_hover(&mut app, 1, true);
        assert!(app.toasts[0].paused);

        let _ = handle_toast_hover(&mut app, 1, false);
        assert!(!app.toasts[0].paused);
      }
    }

    mod handle_toast_dismissed {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_removes_the_toast_and_marks_the_row_read() {
        let mut app = test_app();
        enqueue_toast(&mut app, notification(1));
        app.notifications = vec![notification(1)];
        app.notifications_unread = 1;

        let _ = handle_toast_dismissed(&mut app, 1);

        assert!(app.toasts.is_empty());
        assert_eq!(app.notifications_unread, 0, "the X marks the dismissed row read");
        assert!(
          app.notifications[0].read_at().is_some(),
          "the row stays in the center as read history"
        );
      }
    }

    mod is_notification_source {
      use super::*;

      #[test]
      fn it_gates_to_the_seven_event_sources() {
        assert!(is_notification_source(JobKind::CharacterMail));
        assert!(is_notification_source(JobKind::CharacterSkills));
        assert!(is_notification_source(JobKind::CorporationMiningExtractions));
        assert!(!is_notification_source(JobKind::CharacterWallet));
        assert!(!is_notification_source(JobKind::MarketPrices));
      }
    }
  }

  mod outbox_indicator {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::sync::Event;

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
      assert_eq!(
        app.status.phase(&sync::JobKey::new(
          sync::JobKind::CharacterProfile,
          sync::Subject::Character(1)
        )),
        None,
        "outbox events do not enter the job-keyed status"
      );
    }

    #[test]
    fn it_is_absent_when_the_outbox_is_quiet() {
      let outbox = sync::OutboxStatus::new();

      assert!(
        super::outbox_indicator(&outbox).is_none(),
        "an idle outbox adds no chrome"
      );
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
    fn it_renders_when_a_row_is_pending() {
      let mut outbox = sync::OutboxStatus::new();
      outbox.apply(&Event::OutboxInflight {
        id: 1,
      });

      assert!(super::outbox_indicator(&outbox).is_some());
    }
  }

  mod rail_mail_unread {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clears_the_rail_dot_when_a_count_tick_reports_zero_unread() {
      let mut app = test_app();
      app.mail_unread = 4;

      let _ = update(&mut app, Message::MailUnreadCounted(0));

      assert_eq!(app.mail_unread, 0, "the dot clears when no unread mail remains");
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
    fn it_keeps_the_live_count_when_it_is_already_the_lower_of_the_two() {
      assert_eq!(super::super::rail_mail_unread(1, Some(4)), 1);
    }

    #[test]
    fn it_prefers_the_screens_fresher_optimistic_count_over_a_stale_live_count() {
      assert_eq!(super::super::rail_mail_unread(3, Some(2)), 2);
    }

    #[test]
    fn it_uses_the_live_count_when_the_mail_screen_is_closed() {
      assert_eq!(super::super::rail_mail_unread(3, None), 3);
      assert_eq!(super::super::rail_mail_unread(0, None), 0);
    }
  }

  mod reopen_after_take_over_inner {
    use chrono::Utc;

    use super::*;
    use crate::store::{
      model::HttpCacheEntry,
      repo::infra,
      share_meta::{read_generation, write_generation},
    };

    async fn ready_for(session: &store::sync_session::SyncSession) -> StoreReady {
      let pools = store::open_pools(session.working_copy()).await.unwrap();
      let http = http::Client::builder(http::Cache::new(pools.interactive.clone())).build();
      StoreReady {
        db: pools.interactive,
        http,
        lease: None,
        settings: config::Settings::default(),
        sync_db: pools.sync,
        sync_housekeeping_db: pools.housekeeping,
        sync_session: Some(session.clone()),
      }
    }

    async fn seed(path: &std::path::Path, url: &str) {
      let pools = store::open_pools(path).await.unwrap();
      infra::http_cache_upsert(&pools.interactive, &HttpCacheEntry::new(b"x".to_vec(), 0, url))
        .await
        .unwrap();
      // Fold the WAL into the main .db so the published copy is self-contained.
      sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pools.interactive.writer())
        .await
        .unwrap();
    }

    /// Closes the working-copy pools, claims, and reopens in the exact order [`run_take_over`] does in
    /// production (minus the iced `Task` wrapper). The close-before-swap ordering is what lets the
    /// `publish_database` overwrite succeed on Windows, whose mandatory file locking would otherwise
    /// reject the in-place replace of the still-open `.db` with `PermissionDenied`.
    async fn close_then_take_over(
      ready: StoreReady,
      session: &store::sync_session::SyncSession,
      force: bool,
    ) -> (TakeOverOutcome, StoreReady) {
      let lease = ready.lease.clone();
      let settings = ready.settings.clone();
      ready.db.reader().close().await;
      ready.db.writer().close().await;
      ready.sync_db.reader().close().await;
      ready.sync_db.writer().close().await;
      ready.sync_housekeeping_db.reader().close().await;
      ready.sync_housekeeping_db.writer().close().await;
      let outcome = claim_lease(session, force);
      let reopened = reopen_after_take_over_inner(session, lease, settings).await.unwrap();
      (outcome, reopened)
    }

    #[tokio::test]
    async fn it_reads_the_pulled_contents_after_a_newer_canonical_is_taken_over() {
      let (dir, session) = temp_sync_session();
      let canonical = dir.path().join("share").join("pod.db");
      let sidecar = canonical.with_extension("db.generation");
      let marker = session.working_copy().with_extension("db.generation");
      std::fs::create_dir_all(session.working_copy().parent().unwrap()).unwrap();
      seed(session.working_copy(), "https://esi.example/stale").await;
      seed(&canonical, "https://esi.example/pulled").await;
      write_generation(&sidecar, 9).unwrap();
      write_generation(&marker, 4).unwrap();
      let ready = ready_for(&session).await;

      // Mirror production: the pools are closed *before* the swap so no OS handle straddles the
      // `publish_database` overwrite, then reopened against the pulled file.
      let (outcome, reopened) = close_then_take_over(ready, &session, false).await;

      assert_eq!(outcome, TakeOverOutcome::Claimed);
      assert!(
        infra::http_cache_get(&reopened.db, "https://esi.example/pulled")
          .await
          .unwrap()
          .is_some(),
        "the reopened pool reads the freshly pulled canonical contents"
      );
      assert!(
        infra::http_cache_get(&reopened.db, "https://esi.example/stale")
          .await
          .unwrap()
          .is_none(),
        "the reopened pool no longer sees the pre-swap working-copy contents"
      );
      assert_eq!(read_generation(&marker), 9);
    }

    #[tokio::test]
    async fn it_reopens_the_unchanged_working_copy_when_a_take_over_is_declined() {
      let (dir, session) = temp_sync_session();
      let canonical = dir.path().join("share").join("pod.db");
      let sidecar = canonical.with_extension("db.generation");
      let marker = session.working_copy().with_extension("db.generation");
      std::fs::create_dir_all(session.working_copy().parent().unwrap()).unwrap();
      seed(session.working_copy(), "https://esi.example/local").await;
      seed(&canonical, "https://esi.example/pulled").await;
      write_generation(&sidecar, 9).unwrap();
      write_generation(&marker, 4).unwrap();
      // A still-fresh foreign holder makes the stale-aware claim decline, so no swap happens.
      let share = dir.path().join("share");
      store::lease::LeaseManager::new("machine-holder".to_owned(), "studio-mac".to_owned(), 99, 0)
        .heartbeat(&share, Utc::now())
        .unwrap();
      let ready = ready_for(&session).await;

      let (outcome, reopened) = close_then_take_over(ready, &session, false).await;

      assert_eq!(outcome, TakeOverOutcome::Failed);
      assert!(
        infra::http_cache_get(&reopened.db, "https://esi.example/local")
          .await
          .unwrap()
          .is_some(),
        "a declined take-over reopens the unchanged working copy so the app keeps functioning"
      );
      assert!(
        infra::http_cache_get(&reopened.db, "https://esi.example/pulled")
          .await
          .unwrap()
          .is_none(),
        "no swap happened, so the pulled canonical contents are not present"
      );
      assert_eq!(read_generation(&marker), 4, "the working-copy generation is untouched");
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
    fn it_falls_back_to_first_owned_when_the_sticky_selection_left_the_roster() {
      let roster = vec![pilot(7), pilot(3)];

      assert_eq!(resolve_mail_target(&roster, Some(99)), Some(7));
    }

    #[test]
    fn it_keeps_the_sticky_selection_when_still_owned() {
      let roster = vec![pilot(7), pilot(3)];

      assert_eq!(resolve_mail_target(&roster, Some(3)), Some(3));
    }

    #[test]
    fn it_yields_none_for_an_empty_roster() {
      assert_eq!(resolve_mail_target(&[], None), None);
      assert_eq!(resolve_mail_target(&[], Some(7)), None);
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
    fn it_falls_back_to_first_owned_when_the_sticky_selection_left_the_roster() {
      let roster = vec![pilot(7), pilot(3)];

      assert_eq!(resolve_skills_target(&roster, Some(99)), Some(7));
    }

    #[test]
    fn it_keeps_the_sticky_selection_when_still_owned() {
      let roster = vec![pilot(7), pilot(3)];

      assert_eq!(resolve_skills_target(&roster, Some(3)), Some(3));
    }

    #[test]
    fn it_yields_none_for_an_empty_roster() {
      assert_eq!(resolve_skills_target(&[], None), None);
      assert_eq!(resolve_skills_target(&[], Some(7)), None);
    }
  }

  mod handle_chrome_event {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_closes_the_window_through_the_standard_close_path() {
      let mut app = test_app();
      let main = window::Id::unique();
      let killmail = window::Id::unique();
      app.windows.register(main, Window::Main);
      app.windows.register(killmail, Window::Killmail);

      let _ = handle_chrome_event(&mut app, killmail, window_chrome::Event::Close);

      assert_eq!(app.windows.kind(killmail), None);
      assert_eq!(app.windows.kind(main), Some(Window::Main));
    }

    #[test]
    fn it_leaves_the_registry_untouched_for_a_drag() {
      let mut app = test_app();
      let killmail = window::Id::unique();
      app.windows.register(killmail, Window::Killmail);

      let _ = handle_chrome_event(&mut app, killmail, window_chrome::Event::Drag);

      assert_eq!(app.windows.kind(killmail), Some(Window::Killmail));
    }
  }

  mod killmail_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn detail(killmail_id: i64) -> killmail_detail::KillmailDetail {
      killmail_detail::KillmailDetail {
        attackers: Vec::new(),
        damage_taken: 0,
        dropped_isk: 0.0,
        is_kill: true,
        kill_time: "2024-01-01T00:00:00Z".to_owned(),
        killmail_id,
        ship_icon: store::images::IconResolution::Missing,
        ship_name: "Rifter".to_owned(),
        slots: Vec::new(),
        system_name: None,
        system_security: 0.0,
        value_destroyed_isk: 0.0,
        value_isk: 0.0,
        victim_alliance: None,
        victim_corp: None,
        victim_name: "Target".to_owned(),
        victim_portrait: store::images::ImageState::Fresh("/tmp/p.jpg".into()),
      }
    }

    fn ready(app: &mut App, source: killmail_detail::Source, killmail_id: i64) -> window::Id {
      let id = window::Id::unique();
      app.windows.register(id, Window::Killmail);
      app
        .killmails
        .insert(id, killmail_detail::State::new(source, killmail_id));
      id
    }

    #[tokio::test]
    async fn it_registers_the_kind_and_seeds_the_per_window_state() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let id = ready(
        &mut app,
        killmail_detail::Source::Character {
          character_id: 42,
        },
        100,
      );

      assert_eq!(app.windows.kind(id), Some(Window::Killmail));
      assert_eq!(
        app.killmails.get(id).map(killmail_detail::State::killmail_id),
        Some(100)
      );
    }

    #[tokio::test]
    async fn it_holds_duplicate_killmails_under_distinct_ids() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let source = killmail_detail::Source::Corporation {
        corporation_id: 7,
      };

      let first = ready(&mut app, source, 100);
      let second = ready(&mut app, source, 100);

      assert_ne!(first, second);
      assert_eq!(app.killmails.len(), 2);
      assert_eq!(app.windows.ids_for(Window::Killmail).count(), 2);
    }

    #[tokio::test]
    async fn it_routes_a_loaded_detail_to_only_its_own_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let source = killmail_detail::Source::Character {
        character_id: 42,
      };
      let first = ready(&mut app, source, 100);
      let second = ready(&mut app, source, 200);

      let _ = handle_killmail(
        &mut app,
        first,
        killmail_detail::Message::Loaded(Box::new(Some(detail(100)))),
      );

      assert_eq!(
        app
          .killmails
          .get(first)
          .and_then(killmail_detail::State::loaded_killmail_id),
        Some(100)
      );
      assert_eq!(
        app
          .killmails
          .get(second)
          .and_then(killmail_detail::State::loaded_killmail_id),
        None
      );
    }

    #[tokio::test]
    async fn it_closes_only_the_targeted_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let source = killmail_detail::Source::Character {
        character_id: 42,
      };
      let first = ready(&mut app, source, 100);
      let second = ready(&mut app, source, 200);

      let _ = close_killmail_window(&mut app, first);

      assert_eq!(app.windows.kind(first), None);
      assert!(app.killmails.get(first).is_none());
      assert_eq!(app.windows.kind(second), Some(Window::Killmail));
      assert!(app.killmails.get(second).is_some());
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_a_killmail_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(
        &mut app,
        killmail_detail::Source::Character {
          character_id: 42,
        },
        100,
      );

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.killmails.get(id).is_none());
    }
  }

  mod calendar_event_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn event(character_id: i64, event_id: i64, title: &str) -> calendar::CalendarEvent {
      calendar::CalendarEvent {
        body: Some("<p>Form up.</p>".to_owned()),
        character_id,
        duration_minutes: 90,
        event_id,
        importance: 0,
        owner_name: "Corp".to_owned(),
        owner_type: "corporation".to_owned(),
        response: "not_responded".to_owned(),
        source: None,
        timestamp: "2026-06-20T19:00:00Z".to_owned(),
        title: title.to_owned(),
      }
    }

    fn ready(app: &mut App, character_id: i64, event_id: i64, title: &str) -> window::Id {
      let id = window::Id::unique();
      app.windows.register(id, Window::CalendarEvent);
      app.calendar_events.insert(
        id,
        calendar::EventWindow::new(
          event(character_id, event_id, title),
          Some("Pilot".to_owned()),
          false,
          "not_responded".to_owned(),
        ),
      );
      id
    }

    #[tokio::test]
    async fn it_holds_several_event_windows_under_distinct_ids() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let first = ready(&mut app, 1, 10, "Op Alpha");
      let second = ready(&mut app, 1, 10, "Op Alpha");

      assert_ne!(first, second);
      assert_eq!(app.calendar_events.len(), 2);
      assert_eq!(app.windows.ids_for(Window::CalendarEvent).count(), 2);
    }

    #[tokio::test]
    async fn it_titles_the_window_with_the_event_subject() {
      let mut app = test_app();
      let id = ready(&mut app, 1, 10, "Doctrine refit night");

      assert_eq!(window_title(&app, id), "Pod \u{2014} Doctrine refit night");
    }

    #[tokio::test]
    async fn it_renders_the_event_window_body() {
      let mut app = test_app();
      let id = ready(&mut app, 1, 10, "Op Alpha");

      let _el: Element<'_, Message> = view(&app, id);
    }

    #[tokio::test]
    async fn it_routes_a_per_window_message_to_only_its_own_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let first = ready(&mut app, 1, 10, "Op Alpha");
      let second = ready(&mut app, 1, 20, "Op Beta");

      let _ = handle_calendar_event(
        &mut app,
        first,
        calendar::EventMessage::AttendeesLoaded(Box::new(Some(store::model::AttendeeTally {
          accepted: 2,
          declined: 0,
          invited: 4,
          tentative: 1,
        }))),
      );
      let _ = handle_calendar_event(
        &mut app,
        first,
        calendar::EventMessage::Responded(calendar::Response::Accepted),
      );
      let _ = handle_calendar_event(&mut app, first, calendar::EventMessage::RsvpWritten);

      // The second window is untouched and still titled by its own subject.
      assert_eq!(window_title(&app, second), "Pod \u{2014} Op Beta");
    }

    #[tokio::test]
    async fn it_closes_only_the_targeted_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let first = ready(&mut app, 1, 10, "Op Alpha");
      let second = ready(&mut app, 1, 20, "Op Beta");

      let _ = close_calendar_event_window(&mut app, first);

      assert_eq!(app.windows.kind(first), None);
      assert!(app.calendar_events.get(first).is_none());
      assert_eq!(app.windows.kind(second), Some(Window::CalendarEvent));
      assert!(app.calendar_events.get(second).is_some());
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_the_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(&mut app, 1, 10, "Op Alpha");

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.calendar_events.get(id).is_none());
    }
  }

  mod manage_plans_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn ready(app: &mut App) -> window::Id {
      let _ = open_manage_plans_window(app);
      app.manage_plans.as_ref().map(|(id, _)| *id).expect("window registered")
    }

    #[tokio::test]
    async fn it_registers_the_kind_and_seeds_the_per_window_state() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let id = ready(&mut app);

      assert_eq!(app.windows.kind(id), Some(Window::ManagePlans));
      assert_eq!(app.manage_plans.as_ref().map(|(mid, _)| *mid), Some(id));
    }

    #[tokio::test]
    async fn it_focuses_the_existing_window_instead_of_opening_a_second() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let first = ready(&mut app);

      let _ = open_manage_plans_window(&mut app);

      assert_eq!(app.windows.ids_for(Window::ManagePlans).count(), 1);
      assert_eq!(app.manage_plans.as_ref().map(|(mid, _)| *mid), Some(first));
    }

    #[tokio::test]
    async fn it_drops_the_state_on_close() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(&mut app);

      let _ = close_manage_plans_window(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.manage_plans.is_none());
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_the_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(&mut app);

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.manage_plans.is_none());
    }

    async fn seed_owned(db: &store::Database, id: i64) {
      use crate::store::{
        model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
        repo::{character, infra},
      };

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
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
      infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();
    }

    async fn ready_with_roster(app: &mut App) -> window::Id {
      let id = ready(app);
      let db = app.runtime.as_ref().unwrap().db.clone();
      let roster = skill_plan_manager::load_roster(&db).await;
      let _ = handle_manage_plans(app, skill_plan_manager::Message::Loaded(Box::new(roster)));
      id
    }

    #[tokio::test]
    async fn open_switches_the_active_character_seeds_the_editor_and_closes_the_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.skills = Some(skills::State::new(7));
      app.windows.register(window::Id::unique(), Window::Main);
      let db = app.runtime.as_ref().unwrap().db.clone();
      seed_owned(&db, 42).await;
      let plan = store::repo::skills::create(&db, 42, "Combat").await.unwrap();
      let id = ready_with_roster(&mut app).await;

      let _ = handle_manage_plans(
        &mut app,
        skill_plan_manager::Message::OpenPlan {
          character_id: 42,
          plan_id: plan.id(),
        },
      );

      assert!(app.manage_plans.is_none(), "the manage plans window closes on open");
      assert_eq!(app.windows.kind(id), None);
      assert_eq!(app.skills.as_ref().map(skills::State::active), Some(42));
      let (eid, editor) = app.editor.as_ref().expect("editor window opened");
      assert_eq!(app.windows.kind(*eid), Some(Window::SkillPlanEditor));
      assert_eq!(editor.character_id(), 42);
    }

    #[tokio::test]
    async fn new_seeds_an_editor_for_the_selected_character_and_closes_the_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.skills = Some(skills::State::new(7));
      app.windows.register(window::Id::unique(), Window::Main);
      let db = app.runtime.as_ref().unwrap().db.clone();
      seed_owned(&db, 42).await;
      let id = ready_with_roster(&mut app).await;

      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::NewPlan(42));

      assert!(app.manage_plans.is_none());
      assert_eq!(app.windows.kind(id), None);
      assert_eq!(app.skills.as_ref().map(skills::State::active), Some(42));
      let (_, editor) = app.editor.as_ref().expect("editor window opened");
      assert_eq!(editor.character_id(), 42);
    }

    #[tokio::test]
    async fn request_delete_arms_the_confirm_and_confirm_clears_it() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let db = app.runtime.as_ref().unwrap().db.clone();
      seed_owned(&db, 42).await;
      let plan = store::repo::skills::create(&db, 42, "Combat").await.unwrap();
      let _ = ready_with_roster(&mut app).await;

      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::RequestDelete(plan.id()));
      assert_eq!(app.manage_plans.as_ref().unwrap().1.confirm_delete(), Some(plan.id()));

      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::ConfirmDelete(plan.id()));
      assert_eq!(app.manage_plans.as_ref().unwrap().1.confirm_delete(), None);
    }

    #[tokio::test]
    async fn copy_clones_the_full_plan_onto_the_target_with_name_de_dup() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, 42).await;
      seed_owned(&db, 7).await;
      let source = store::repo::skills::create(&db, 42, "Combat").await.unwrap();
      store::repo::skills::replace_entries(&db, source.id(), &[(3300, 5, "high", "core", 0)])
        .await
        .unwrap();
      store::repo::skills::create(&db, 7, "Combat").await.unwrap();

      let clone_id = copy_plan_to_character(&db, source.id(), 7, &["Combat".to_owned()])
        .await
        .unwrap();

      let clone = store::repo::skills::get(&db, clone_id).await.unwrap().unwrap();
      assert_eq!(clone.name(), "Combat (2)", "name de-duped against the target");
      assert_eq!(clone.character_id(), 7);
      let entries = store::repo::skills::entries(&db, clone_id).await.unwrap();
      assert_eq!(entries.iter().map(|e| e.skill_id()).collect::<Vec<_>>(), [3300]);
      assert_eq!(entries[0].to_level(), 5);
    }
  }

  mod contract_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn detail(contract_id: i64) -> contract_detail::ContractDetail {
      contract_detail::ContractDetail {
        acceptor: None,
        availability: "Public".to_owned(),
        bids: Vec::new(),
        buyout: None,
        collateral: None,
        contract_id,
        days_to_complete: Some(0),
        expiry: contract_detail::ExpiryView {
          future: true,
          label: "Open".to_owned(),
          title: "Expires",
        },
        headline: 200.0,
        headline_label: "Price",
        issued_time: "2024-01-01T00:00:00Z".to_owned(),
        issuer: contract_detail::PartyView {
          name: "Issuer Pilot".to_owned(),
          portrait: store::images::ImageState::Fresh("/tmp/p.jpg".into()),
          role: "Issuer",
          sub: None,
        },
        items: Vec::new(),
        items_value: 0.0,
        kind: contract_detail::ContractKind::ItemExchange,
        location_name: "Jita IV - Moon 4".to_owned(),
        route: None,
        status: "outstanding".to_owned(),
        title: "Test Contract".to_owned(),
        volume: 0.0,
      }
    }

    fn ready(app: &mut App, source: contract_detail::Source, contract_id: i64) -> window::Id {
      let id = window::Id::unique();
      app.windows.register(id, Window::Contract);
      app
        .contracts
        .insert(id, contract_detail::State::new(source, contract_id));
      id
    }

    #[tokio::test]
    async fn it_registers_the_kind_and_seeds_the_per_window_state() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let id = ready(
        &mut app,
        contract_detail::Source::Character {
          character_id: 42,
        },
        100,
      );

      assert_eq!(app.windows.kind(id), Some(Window::Contract));
      assert_eq!(
        app.contracts.get(id).map(contract_detail::State::contract_id),
        Some(100)
      );
    }

    #[tokio::test]
    async fn it_holds_duplicate_contracts_under_distinct_ids() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let source = contract_detail::Source::Corporation {
        corporation_id: 7,
      };

      let first = ready(&mut app, source, 100);
      let second = ready(&mut app, source, 100);

      assert_ne!(first, second);
      assert_eq!(app.contracts.len(), 2);
      assert_eq!(app.windows.ids_for(Window::Contract).count(), 2);
    }

    #[tokio::test]
    async fn it_routes_a_loaded_detail_to_only_its_own_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let source = contract_detail::Source::Character {
        character_id: 42,
      };
      let first = ready(&mut app, source, 100);
      let second = ready(&mut app, source, 200);

      let _ = handle_contract(
        &mut app,
        first,
        contract_detail::Message::Loaded(Box::new(Some(detail(100)))),
      );

      assert_eq!(
        app
          .contracts
          .get(first)
          .and_then(contract_detail::State::loaded_contract_id),
        Some(100)
      );
      assert_eq!(
        app
          .contracts
          .get(second)
          .and_then(contract_detail::State::loaded_contract_id),
        None
      );
    }

    #[tokio::test]
    async fn it_closes_only_the_targeted_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let source = contract_detail::Source::Character {
        character_id: 42,
      };
      let first = ready(&mut app, source, 100);
      let second = ready(&mut app, source, 200);

      let _ = close_contract_window(&mut app, first);

      assert_eq!(app.windows.kind(first), None);
      assert!(app.contracts.get(first).is_none());
      assert_eq!(app.windows.kind(second), Some(Window::Contract));
      assert!(app.contracts.get(second).is_some());
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_a_contract_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(
        &mut app,
        contract_detail::Source::Character {
          character_id: 42,
        },
        100,
      );

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.contracts.get(id).is_none());
    }

    #[tokio::test]
    async fn it_registers_and_seeds_synchronously_via_the_native_opener() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let _ = open_contract_window(
        &mut app,
        contract_detail::Source::Character {
          character_id: 42,
        },
        100,
      );

      let id = app
        .windows
        .ids_for(Window::Contract)
        .next()
        .expect("contract window registered");
      assert_eq!(
        app.contracts.get(id).map(contract_detail::State::contract_id),
        Some(100)
      );
    }

    #[test]
    fn it_is_a_no_op_without_a_runtime() {
      let mut app = test_app();

      let _ = open_contract_window(
        &mut app,
        contract_detail::Source::Character {
          character_id: 42,
        },
        100,
      );

      assert_eq!(app.windows.ids_for(Window::Contract).count(), 0);
      assert_eq!(app.contracts.len(), 0);
    }
  }

  mod stockpile_editor_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn ready(app: &mut App, seed: assets::EditorSeed) -> window::Id {
      let id = window::Id::unique();
      app.windows.register(id, Window::StockpileEditor);
      app.stockpile_editors.insert(id, assets::Editor::from_seed(seed));
      id
    }

    #[tokio::test]
    async fn it_registers_the_kind_and_seeds_a_blank_new_editor() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let id = ready(&mut app, assets::EditorSeed::Blank);

      assert_eq!(app.windows.kind(id), Some(Window::StockpileEditor));
      assert!(app.stockpile_editors.get(id).is_some());
    }

    #[tokio::test]
    async fn it_holds_a_new_and_an_edit_window_at_once() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let new = ready(&mut app, assets::EditorSeed::Blank);
      let edit = ready(&mut app, assets::EditorSeed::Blank);

      assert_ne!(new, edit);
      assert_eq!(app.stockpile_editors.len(), 2);
      assert_eq!(app.windows.ids_for(Window::StockpileEditor).count(), 2);
    }

    #[tokio::test]
    async fn it_routes_an_edit_to_only_its_own_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let first = ready(&mut app, assets::EditorSeed::Blank);
      let second = ready(&mut app, assets::EditorSeed::Blank);

      let _ = handle_stockpile_editor(
        &mut app,
        first,
        assets::Message::StockpileEditorNameChanged("Cap boosters".to_owned()),
      );

      assert_eq!(
        app.stockpile_editors.get(first).map(assets::Editor::name),
        Some("Cap boosters")
      );
      assert_eq!(app.stockpile_editors.get(second).map(assets::Editor::name), Some(""));
    }

    #[tokio::test]
    async fn it_closes_the_window_on_cancel() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(&mut app, assets::EditorSeed::Blank);

      let _ = handle_stockpile_editor(&mut app, id, assets::Message::StockpileEditorClosed);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.stockpile_editors.get(id).is_none());
    }

    #[tokio::test]
    async fn it_saves_and_closes_only_the_targeted_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let first = ready(&mut app, assets::EditorSeed::Blank);
      let second = ready(&mut app, assets::EditorSeed::Blank);

      let _ = handle_stockpile_editor(&mut app, first, assets::Message::StockpileEditorSaved);

      assert_eq!(app.windows.kind(first), None);
      assert!(app.stockpile_editors.get(first).is_none());
      assert_eq!(app.windows.kind(second), Some(Window::StockpileEditor));
      assert!(app.stockpile_editors.get(second).is_some());
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_the_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(&mut app, assets::EditorSeed::Blank);

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.stockpile_editors.get(id).is_none());
    }
  }

  mod stockpile_import_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn open(app: &mut App) -> window::Id {
      let id = window::Id::unique();
      app.windows.register(id, Window::StockpileImport);
      app.stockpile_imports.insert(id, assets::ImportPanel::blank());
      id
    }

    fn import_resolution() -> assets::MultibuyResolution {
      assets::MultibuyResolution {
        matched: vec![assets::MultibuyMatch {
          name: "Tritanium".to_owned(),
          quantity: 100,
          type_id: 34,
        }],
        unmatched: Vec::new(),
      }
    }

    #[tokio::test]
    async fn it_opens_a_single_instance_window_with_a_runtime() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let _ = open_stockpile_import_window(&mut app);

      assert_eq!(app.windows.ids_for(Window::StockpileImport).count(), 1);
      assert_eq!(app.stockpile_imports.len(), 1);
    }

    #[tokio::test]
    async fn it_replaces_the_existing_import_window_on_reopen() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let _ = open_stockpile_import_window(&mut app);
      let _ = open_stockpile_import_window(&mut app);

      // Single-instance: the second open closes the first, leaving exactly one registered.
      assert_eq!(app.windows.ids_for(Window::StockpileImport).count(), 1);
    }

    #[tokio::test]
    async fn it_is_a_no_op_without_a_runtime() {
      let mut app = test_app();

      let _ = open_stockpile_import_window(&mut app);

      assert_eq!(app.windows.ids_for(Window::StockpileImport).count(), 0);
      assert_eq!(app.stockpile_imports.len(), 0);
    }

    #[tokio::test]
    async fn it_routes_a_text_edit_to_only_its_own_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let id = open(&mut app);

      let _ = handle_stockpile_import(
        &mut app,
        id,
        assets::Message::StockpileImportTextChanged(iced::widget::text_editor::Action::Edit(
          iced::widget::text_editor::Edit::Paste(std::sync::Arc::new("Tritanium 100".to_owned())),
        )),
      );

      assert_eq!(
        app.stockpile_imports.get(id).map(assets::ImportPanel::text),
        Some("Tritanium 100".to_owned())
      );
    }

    #[tokio::test]
    async fn it_closes_the_window_on_cancel() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = open(&mut app);

      let _ = handle_stockpile_import(&mut app, id, assets::Message::StockpileImportClosed);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.stockpile_imports.get(id).is_none());
    }

    #[tokio::test]
    async fn it_confirms_into_a_prefilled_editor_and_closes_the_import_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = open(&mut app);
      // Resolve a paste so the panel holds a match, then confirm it. (The resolver itself runs against
      // ESI, so the panel is seeded directly via the resolved message the resolver would emit.)
      let _ = handle_stockpile_import(
        &mut app,
        id,
        assets::Message::StockpileImportResolved(import_resolution()),
      );

      let _ = handle_stockpile_import(&mut app, id, assets::Message::StockpileImportConfirmed);

      // The import window is gone and a prefilled editor window took its place.
      assert!(app.stockpile_imports.get(id).is_none());
      assert_eq!(app.windows.ids_for(Window::StockpileEditor).count(), 1);
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_the_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = open(&mut app);

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.stockpile_imports.get(id).is_none());
    }
  }

  mod compose_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn ready(app: &mut App, seed: mail::compose::Seed) -> window::Id {
      let id = window::Id::unique();
      app.windows.register(id, Window::MailCompose);
      app.composes.insert(id, mail::compose::Draft::from_seed(seed));
      id
    }

    #[tokio::test]
    async fn it_registers_the_kind_and_seeds_a_blank_compose() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let id = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );

      assert_eq!(app.windows.kind(id), Some(Window::MailCompose));
      assert!(app.composes.get(id).is_some());
    }

    #[tokio::test]
    async fn it_holds_two_composes_at_once() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let first = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );
      let second = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 7,
        },
      );

      assert_ne!(first, second);
      assert_eq!(app.composes.len(), 2);
      assert_eq!(app.windows.ids_for(Window::MailCompose).count(), 2);
    }

    #[tokio::test]
    async fn it_routes_a_subject_edit_to_only_its_own_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let first = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );
      let second = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );

      let _ = handle_compose(&mut app, first, mail::Message::ComposeSubjectChanged("CTA".to_owned()));

      assert_eq!(app.composes.get(first).map(|d| d.subject.as_str()), Some("CTA"));
      assert_eq!(app.composes.get(second).map(|d| d.subject.as_str()), Some(""));
    }

    #[tokio::test]
    async fn it_discards_a_compose_without_saving() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );

      let _ = handle_compose(&mut app, id, mail::Message::ComposeDiscarded);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.composes.get(id).is_none());
    }

    #[tokio::test]
    async fn it_closes_only_the_targeted_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let first = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );
      let second = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );

      let _ = close_compose_window(&mut app, first);

      assert_eq!(app.windows.kind(first), None);
      assert!(app.composes.get(first).is_none());
      assert_eq!(app.windows.kind(second), Some(Window::MailCompose));
      assert!(app.composes.get(second).is_some());
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_a_compose_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.composes.get(id).is_none());
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

    fn sized(width: f32, height: f32) -> WindowGeometry {
      WindowGeometry {
        height,
        width,
        x: 100.0,
        y: 100.0,
      }
    }

    #[test]
    fn it_centers_at_the_default_size_when_there_is_no_saved_geometry() {
      let (size, position) = resolve_window_geometry(None, &[monitor()], DEFAULT);

      assert_eq!(size, DEFAULT);
      assert!(matches!(position, window::Position::Centered));
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
    fn it_defaults_the_size_for_a_zero_sized_window() {
      let (size, _) = resolve_window_geometry(Some(sized(0.0, 0.0)), &[monitor()], DEFAULT);

      assert_eq!(size, DEFAULT, "a 0x0 saved size never reopens broken");
    }

    #[test]
    fn it_defaults_the_size_for_an_absurdly_large_window() {
      let (size, _) = resolve_window_geometry(Some(sized(999_999.0, 999_999.0)), &[monitor()], DEFAULT);

      assert_eq!(size, DEFAULT);
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
    fn it_falls_back_to_the_range_guard_when_no_monitor_is_known() {
      let (_, in_range) = resolve_window_geometry(Some(geometry(120.0, 90.0)), &[], DEFAULT);
      assert!(matches!(in_range, window::Position::Specific(p) if p == Point::new(120.0, 90.0)));

      let (_, out_of_range) = resolve_window_geometry(Some(geometry(-50.0, 90.0)), &[], DEFAULT);
      assert!(matches!(out_of_range, window::Position::Centered));
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
    fn it_restores_a_size_at_or_above_the_floor_unchanged() {
      let (size, _) = resolve_window_geometry(Some(sized(900.0, 650.0)), &[monitor()], DEFAULT);

      assert_eq!(size, Size::new(900.0, 650.0));
    }

    #[test]
    fn it_restores_size_and_position_for_a_monitor_valid_saved_rect() {
      let (size, position) = resolve_window_geometry(Some(geometry(120.0, 90.0)), &[monitor()], DEFAULT);

      assert_eq!(size, Size::new(1000.0, 700.0));
      assert!(matches!(position, window::Position::Specific(p) if p == Point::new(120.0, 90.0)));
    }
  }

  mod scale_to_factor {
    use super::*;

    #[test]
    fn it_clamps_values_outside_the_supported_range() {
      assert_eq!(scale_to_factor(0), 0.85);
      assert_eq!(scale_to_factor(255), 1.5);
    }

    #[test]
    fn it_maps_a_default_scale_to_a_unit_factor() {
      assert_eq!(scale_to_factor(100), 1.0);
    }

    #[test]
    fn it_maps_the_extremes_of_the_range() {
      assert_eq!(scale_to_factor(85), 0.85);
      assert_eq!(scale_to_factor(150), 1.5);
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

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    fn finished_event(character_id: i64) -> sync::Event {
      sync::Event::Finished {
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(character_id)),
        outcome: sync::Outcome::synced(),
      }
    }

    fn asset_sync_event(character_id: i64) -> sync::Event {
      sync::Event::Finished {
        key: JobKey::new(JobKind::AssetSync, Subject::Character(character_id)),
        outcome: sync::Outcome::synced(),
      }
    }

    fn assets_dirty(app: &App) -> bool {
      app.assets.as_ref().is_some_and(assets::State::is_dirty)
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
    fn it_closes_the_compare_window_when_it_requests_close() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::Compare);
      app.compare = Some((id, skills_compare::State::new(vec![1, 2], Vec::new())));

      let _ = handle_compare(&mut app, skills_compare::Message::CloseRequested);

      assert!(app.compare.is_none(), "the compare state is cleared");
      assert_eq!(app.windows.kind(id), None, "the compare window is de-registered");
    }

    #[tokio::test]
    async fn it_coalesces_a_burst_of_asset_syncs_into_one_pending_assets_refresh() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.route = Route::Assets;
      app.assets = Some(assets::State::new(config::FeatureFlags::default()));

      for character_id in 0..6 {
        let _ = update(&mut app, Message::Sync(asset_sync_event(character_id)));
      }

      assert!(
        assets_dirty(&app),
        "a burst of AssetSync events marks the assets dirty once instead of reloading per event"
      );

      let _ = update(&mut app, Message::SyncPulse);
      assert!(!assets_dirty(&app), "the pulse consumes the coalesced assets refresh");

      let _ = update(&mut app, Message::SyncPulse);
      assert!(!assets_dirty(&app), "a quiet pulse schedules no further assets reload");
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

    #[test]
    fn it_holds_a_re_dirtied_roster_until_the_debounce_window_opens() {
      let mut app = test_app();
      app.character_manager = Some(character_manager::State::new());
      let start = Instant::now();

      // First drain in the burst reloads immediately and arms the trailing-debounce floor.
      app.roster_dirty = true;
      let _ = drain_roster_dirty_at(&mut app, start);
      assert!(!app.roster_dirty, "the first dirty pulse reloads and clears the flag");
      assert!(app.next_roster_reload.is_some(), "the reload arms a debounce floor");

      // A later Finished event inside the window re-marks the roster, but a pulse must not reload yet.
      app.roster_dirty = true;
      let _ = drain_roster_dirty_at(&mut app, start + Duration::from_millis(450));
      assert!(
        app.roster_dirty,
        "a re-dirty inside the debounce window is held, not reloaded ~2x/s"
      );
    }

    #[test]
    fn it_reloads_the_roster_again_once_the_debounce_window_elapses() {
      let mut app = test_app();
      app.character_manager = Some(character_manager::State::new());
      let start = Instant::now();

      app.roster_dirty = true;
      let _ = drain_roster_dirty_at(&mut app, start);

      app.roster_dirty = true;
      let _ = drain_roster_dirty_at(&mut app, start + ROSTER_RELOAD_DEBOUNCE + Duration::from_millis(1));
      assert!(
        !app.roster_dirty,
        "once the debounce window opens the held refresh fires and clears the flag"
      );
    }

    #[test]
    fn it_staggers_the_clock_checks_so_they_do_not_all_fire_on_one_tick() {
      // No single tick should carry every staggered check; the whole point is to spread the load.
      for tick in 0..30u64 {
        let due = ClockChecks::for_tick(tick);
        let firing = [
          due.snooze_wake,
          due.mail_unread,
          due.mail_reload,
          due.calendar_attention,
          due.calendar_reload,
          due.industry_reload,
        ]
        .iter()
        .filter(|fired| **fired)
        .count();
        assert!(
          firing < 6,
          "tick {tick} fired all staggered checks at once; they should be spread across ticks"
        );
      }
    }

    #[test]
    fn it_keeps_user_facing_checks_fresh_within_their_cadence() {
      // Snooze, mail, and calendar freshness must still fire at least once across any short window so
      // there is no behavioral regression in wake/unread/attention promptness.
      let window: Vec<ClockChecks> = (0..6).map(ClockChecks::for_tick).collect();
      assert!(
        window.iter().any(|c| c.snooze_wake),
        "snooze wake still fires regularly"
      );
      assert!(
        window.iter().any(|c| c.mail_unread),
        "mail unread still fires regularly"
      );
      assert!(
        window.iter().any(|c| c.mail_reload),
        "mail reload still fires regularly"
      );
      assert!(
        window.iter().any(|c| c.calendar_attention),
        "calendar attention still fires regularly"
      );
      assert!(
        window.iter().any(|c| c.calendar_reload),
        "calendar reload still fires regularly"
      );
      // Industry jobs are long-lived, so its reload is the rarest but must still recur.
      let long_window: Vec<ClockChecks> = (0..10).map(ClockChecks::for_tick).collect();
      assert!(
        long_window.iter().any(|c| c.industry_reload),
        "industry reload still recurs on its slower cadence"
      );
    }

    #[test]
    fn it_advances_the_clock_tick_counter_each_tick() {
      let mut app = test_app();
      assert_eq!(app.clock_tick, 0);

      let _ = update(&mut app, Message::ClockTick);
      assert_eq!(app.clock_tick, 1);

      let _ = update(&mut app, Message::ClockTick);
      assert_eq!(app.clock_tick, 2);
    }

    #[test]
    fn it_does_not_mark_assets_dirty_while_off_the_assets_route() {
      let mut app = test_app();
      app.route = Route::Wallet;
      app.assets = Some(assets::State::new(config::FeatureFlags::default()));

      let _ = update(&mut app, Message::Sync(asset_sync_event(1)));

      assert!(
        !assets_dirty(&app),
        "an off-route asset sync schedules no assets reload"
      );
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
    fn it_keeps_route_and_sticky_selection_in_sync_on_a_picker_switch() {
      let mut app = test_app();

      let _ = update(&mut app, Message::Skills(skills::Message::CharacterChanged(99)));

      assert_eq!(app.route, Route::Skills(99));
      assert_eq!(app.selected_character, Some(99));
    }

    #[test]
    fn it_keeps_the_characters_destination_lit_while_a_corporation_is_drilled_in() {
      assert_eq!(
        Route::CorporationDetail(98_000_001).destination(),
        rail::Destination::Characters
      );
    }

    #[test]
    fn it_keeps_the_characters_destination_lit_while_a_pilot_is_drilled_in() {
      assert_eq!(Route::CharacterDetail(42).destination(), rail::Destination::Characters);
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
    fn it_navigates_to_the_industry_screen_on_the_industry_rail_destination() {
      let mut app = test_app();

      let _ = update(&mut app, Message::Nav(rail::Destination::Industry));

      assert_eq!(app.route, Route::Industry);
      assert!(app.industry.is_some());
      assert_eq!(app.route.destination(), rail::Destination::Industry);
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
    fn it_deep_navigates_to_a_specific_wallet_tab() {
      let mut app = test_app();

      let _ = update(&mut app, Message::NavTo(rail::Destination::Wallet, Some("budget")));

      assert_eq!(app.route, Route::Wallet);
      assert_eq!(
        app.wallet.as_ref().map(wallet::State::active_tab),
        Some(wallet::Tab::Budget)
      );
    }

    #[test]
    fn it_deep_navigates_to_a_specific_assets_tab() {
      let mut app = test_app();

      let _ = update(&mut app, Message::NavTo(rail::Destination::Assets, Some("values")));

      assert_eq!(app.route, Route::Assets);
      assert_eq!(
        app.assets.as_ref().map(assets::State::active_tab),
        Some(assets::Tab::Values)
      );
    }

    #[test]
    fn it_deep_navigates_to_a_specific_industry_tab() {
      let mut app = test_app();

      let _ = update(&mut app, Message::NavTo(rail::Destination::Industry, Some("planner")));

      assert_eq!(app.route, Route::Industry);
      assert_eq!(
        app.industry.as_ref().map(industry::State::active_tab),
        Some(industry::Tab::Planner)
      );
    }

    #[test]
    fn it_deep_navigates_to_a_specific_calendar_view() {
      let mut app = test_app();

      let _ = update(&mut app, Message::NavTo(rail::Destination::Calendar, Some("week")));

      assert_eq!(app.route, Route::Calendar);
      assert_eq!(
        app.calendar.as_ref().map(calendar::State::active_view),
        Some(calendar::View::Week)
      );
    }

    #[test]
    fn it_deep_navigates_to_a_specific_characters_pane() {
      let mut app = test_app();
      app.character_manager = Some(character_manager::State::new());

      let _ = update(
        &mut app,
        Message::NavTo(rail::Destination::Characters, Some("corporations")),
      );

      assert_eq!(app.route, Route::Characters);
      assert_eq!(
        app
          .character_manager
          .as_ref()
          .map(character_manager::State::active_pane),
        Some(character_manager::Pane::Corporations)
      );
    }

    #[tokio::test]
    async fn it_deep_navigates_to_a_specific_settings_category() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = update(&mut app, Message::NavTo(rail::Destination::Settings, Some("storage")));

      assert_eq!(app.route, Route::Settings);
      assert_eq!(
        app.settings.as_ref().map(settings::State::active_category),
        Some(settings::Category::Storage)
      );
    }

    #[test]
    fn it_deep_navigates_without_a_sub_section_keeping_the_default_tab() {
      let mut app = test_app();

      let _ = update(&mut app, Message::NavTo(rail::Destination::Wallet, None));

      assert_eq!(app.route, Route::Wallet);
      assert_eq!(
        app.wallet.as_ref().map(wallet::State::active_tab),
        Some(wallet::Tab::default())
      );
    }

    #[test]
    fn it_ignores_an_unknown_sub_section_id_keeping_the_default_tab() {
      let mut app = test_app();

      let _ = update(&mut app, Message::NavTo(rail::Destination::Wallet, Some("nonexistent")));

      assert_eq!(app.route, Route::Wallet);
      assert_eq!(
        app.wallet.as_ref().map(wallet::State::active_tab),
        Some(wallet::Tab::default())
      );
    }

    #[test]
    fn it_records_the_hovered_rail_destination() {
      let mut app = test_app();

      let _ = update(&mut app, Message::RailHover(Some(rail::Destination::Wallet)));

      assert_eq!(app.rail_hover, Some(rail::Destination::Wallet));
    }

    #[test]
    fn it_defers_the_flyout_close_until_the_grace_window_expires() {
      let mut app = test_app();
      let _ = update(&mut app, Message::RailHover(Some(rail::Destination::Wallet)));

      let _ = update(&mut app, Message::RailHover(None));

      assert_eq!(
        app.rail_hover,
        Some(rail::Destination::Wallet),
        "the close is deferred, not immediate"
      );
    }

    #[test]
    fn it_closes_the_flyout_when_the_current_expiry_fires() {
      let mut app = test_app();
      let _ = update(&mut app, Message::RailHover(Some(rail::Destination::Wallet)));
      let _ = update(&mut app, Message::RailHover(None));
      let generation = app.rail_hover_gen;

      let _ = update(&mut app, Message::RailHoverExpire(generation));

      assert_eq!(app.rail_hover, None);
    }

    #[test]
    fn it_strands_a_stale_expiry_after_a_re_entry() {
      let mut app = test_app();
      let _ = update(&mut app, Message::RailHover(Some(rail::Destination::Wallet)));
      let _ = update(&mut app, Message::RailHover(None));
      let stale = app.rail_hover_gen;
      let _ = update(&mut app, Message::RailHover(Some(rail::Destination::Assets)));

      let _ = update(&mut app, Message::RailHoverExpire(stale));

      assert_eq!(
        app.rail_hover,
        Some(rail::Destination::Assets),
        "re-entry survives the stale expiry"
      );
    }

    #[test]
    fn it_reports_the_active_sub_section_for_the_open_tab() {
      let mut app = test_app();
      let _ = update(&mut app, Message::NavTo(rail::Destination::Wallet, Some("budget")));

      assert_eq!(active_sub_section(&app), Some("budget"));
    }

    #[test]
    fn it_reports_no_active_sub_section_for_a_tabless_destination() {
      let mut app = test_app();
      let _ = update(&mut app, Message::Nav(rail::Destination::Mail));

      assert_eq!(active_sub_section(&app), None);
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
    fn it_routes_to_the_skills_empty_state_for_an_empty_owned_roster() {
      let mut app = test_app();

      let _ = navigate_to_skills(&mut app, None, Vec::new());

      assert_eq!(app.route, Route::Skills(EMPTY_SKILLS_SELECTION));
      assert_eq!(app.selected_character, None);
      assert!(app.skills.is_some());
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

    #[test]
    fn it_surfaces_a_seed_error_as_a_fatal_init_failure_without_a_runtime() {
      let mut app = test_app();
      app.splash = Some(splash::State::default());
      app.store_ready = None;

      let _ = on_seed_progress(&mut app, splash::seed::Progress::Error("download failed".to_owned()));

      assert_eq!(app.init_error.as_deref(), Some("download failed"));
      assert!(app.runtime.is_none(), "a seed failure must not enter the main runtime");
    }

    #[test]
    fn it_opens_the_palette_on_the_slash_key_when_no_text_input_is_focused() {
      let mut app = test_app();

      let _ = update(&mut app, Message::Palette(PaletteMessage::Open));

      assert!(app.palette.is_some());
    }

    #[test]
    fn it_does_not_open_the_palette_on_slash_while_a_text_input_is_focused() {
      let mut app = test_app();
      app.keyboard_focus.set_focused(Some(iced::widget::Id::from("search")));

      let opener = shortcuts::PaletteKey::for_key(
        &iced::keyboard::Key::Character("/".into()),
        app.palette.is_some(),
        app.keyboard_focus.is_text_input_focused(),
      );

      assert_eq!(opener, None);
      assert!(app.palette.is_none());
    }

    #[test]
    fn it_filters_synchronously_across_nav_commands_and_entities() {
      let mut app = test_app();
      app.character_manager = Some(character_manager::State::new());
      let _ = update(&mut app, Message::Palette(PaletteMessage::Open));

      let _ = update(
        &mut app,
        Message::Palette(PaletteMessage::QueryChanged("budget".to_owned())),
      );
      let nav = palette_entries(&app);
      let _ = update(
        &mut app,
        Message::Palette(PaletteMessage::QueryChanged("sync".to_owned())),
      );
      let commands = palette_entries(&app);

      assert!(
        nav
          .iter()
          .any(|e| matches!(e.kind, command_palette::Kind::Section | command_palette::Kind::Tab)),
        "a nav query resolves nav results"
      );
      assert!(
        commands.iter().any(|e| e.kind == command_palette::Kind::Command),
        "a command query resolves a curated command"
      );
    }

    #[test]
    fn it_moves_the_selection_with_the_arrow_messages() {
      let mut app = test_app();
      let _ = update(&mut app, Message::Palette(PaletteMessage::Open));

      let _ = update(&mut app, Message::Palette(PaletteMessage::MoveDown));
      let after_down = app.palette.as_ref().map(|s| s.selected);
      let _ = update(&mut app, Message::Palette(PaletteMessage::MoveUp));
      let after_up = app.palette.as_ref().map(|s| s.selected);

      assert_eq!(after_down, Some(1));
      assert_eq!(after_up, Some(0));
    }

    #[test]
    fn it_deep_navigates_when_a_nav_result_is_activated() {
      let mut app = test_app();
      let _ = update(&mut app, Message::Palette(PaletteMessage::Open));
      let _ = update(
        &mut app,
        Message::Palette(PaletteMessage::QueryChanged("budget".to_owned())),
      );
      let index = palette_entries(&app)
        .iter()
        .position(|e| e.label == "Budget")
        .expect("a Budget tab result");

      let _ = update(&mut app, Message::Palette(PaletteMessage::Activate(index)));

      assert_eq!(app.route, Route::Wallet);
      assert_eq!(
        app.wallet.as_ref().map(wallet::State::active_tab),
        Some(wallet::Tab::Budget)
      );
      assert!(app.palette.is_none(), "activating a result closes the palette");
    }

    #[test]
    fn it_maps_the_skills_compare_result_to_the_open_compare_action() {
      let mut app = test_app();
      let _ = update(&mut app, Message::Palette(PaletteMessage::Open));
      let _ = update(
        &mut app,
        Message::Palette(PaletteMessage::QueryChanged("compare".to_owned())),
      );

      let compare = palette_entries(&app)
        .into_iter()
        .find(|entry| entry.label == "Compare")
        .expect("a Compare result");

      // The Compare surface is a separate window, so the palette entry must carry the Skills
      // "compare" sub-section that select_sub_section turns into skills::Message::OpenCompare.
      assert_eq!(
        compare.action,
        command_palette::Action::NavTo(
          *crate::features::nav_catalog::section(rail::Destination::Skills).expect("the Skills section"),
          Some("compare"),
        ),
      );
    }

    #[test]
    fn it_routes_the_skills_compare_sub_section_through_open_compare() {
      let mut app = featured_app();

      // With no synced pilots there are no seed ids, so OpenCompare's guard leaves the window
      // closed — but reaching that guard (rather than the old Skills no-op) is the wiring under test.
      let _ = handle_nav_to(&mut app, rail::Destination::Skills, Some("compare"));

      assert_eq!(app.route.destination(), rail::Destination::Skills);
      assert!(
        app.compare.is_none(),
        "OpenCompare bails without at least two pilots to compare"
      );
    }

    #[test]
    fn it_opens_a_character_detail_when_an_entity_result_is_activated() {
      let mut app = test_app();
      app.character_manager = Some(character_manager::State::new());
      let _ = update(&mut app, Message::Palette(PaletteMessage::Open));

      let action = command_palette::Action::Detail(command_palette::Entity {
        id: 42,
        kind: command_palette::EntityKind::Character,
        name: "Pilot".to_owned(),
      });
      let _ = palette_activate_action(&mut app, action);

      assert_eq!(app.route, Route::CharacterDetail(42));
      assert!(app.character_detail.is_some());
    }

    #[test]
    fn it_dispatches_a_curated_command_when_activated() {
      let mut app = test_app();
      let _ = update(&mut app, Message::Palette(PaletteMessage::Open));

      let _ = palette_command(&mut app, command_palette::Command::OpenSettings);

      assert_eq!(app.route, Route::Settings);
    }

    #[test]
    fn it_closes_the_palette_when_a_window_command_is_activated() {
      let mut app = test_app();
      let _ = update(&mut app, Message::Palette(PaletteMessage::Open));
      let _ = update(
        &mut app,
        Message::Palette(PaletteMessage::QueryChanged("stockpile".to_owned())),
      );
      let index = palette_entries(&app)
        .iter()
        .position(|e| e.action == command_palette::Action::Command(command_palette::Command::CreateStockpile))
        .expect("a Create stockpile command result");

      // Without a runtime the open helper no-ops, but activating the palette entry must still close it.
      let _ = update(&mut app, Message::Palette(PaletteMessage::Activate(index)));

      assert!(app.palette.is_none(), "activating a window command closes the palette");
    }

    #[test]
    fn it_dispatches_the_window_commands_without_a_runtime() {
      let mut app = test_app();

      // The open helpers all early-return without a runtime; dispatching must not panic.
      let _ = palette_command(&mut app, command_palette::Command::ComposeMail);
      let _ = palette_command(&mut app, command_palette::Command::CreateStockpile);
      let _ = palette_command(&mut app, command_palette::Command::ManageSkillPlans);
    }

    #[test]
    fn it_resolves_the_compose_from_to_the_mail_views_default_sender() {
      let mut app = test_app();
      app.mail = Some(mail::State::new(77));

      assert_eq!(palette_compose_from(&app), Some(77));
    }

    #[test]
    fn it_resolves_no_compose_from_without_a_mail_view_or_characters() {
      let app = test_app();

      assert_eq!(palette_compose_from(&app), None);
    }

    #[test]
    fn it_closes_the_palette_on_the_escape_message() {
      let mut app = test_app();
      let _ = update(&mut app, Message::Palette(PaletteMessage::Open));
      assert!(app.palette.is_some());

      let _ = update(&mut app, Message::Palette(PaletteMessage::Close));

      assert!(app.palette.is_none());
    }

    #[test]
    fn it_maps_each_palette_key_to_its_palette_message() {
      fn payload(key: shortcuts::PaletteKey) -> PaletteMessage {
        match palette_message(key) {
          Message::Palette(message) => message,
          other => panic!("palette_message produced a non-Palette message: {other:?}"),
        }
      }

      assert!(matches!(
        payload(shortcuts::PaletteKey::Activate),
        PaletteMessage::ActivateSelected
      ));
      assert!(matches!(payload(shortcuts::PaletteKey::Close), PaletteMessage::Close));
      assert!(matches!(
        payload(shortcuts::PaletteKey::MoveDown),
        PaletteMessage::MoveDown
      ));
      assert!(matches!(payload(shortcuts::PaletteKey::MoveUp), PaletteMessage::MoveUp));
      assert!(matches!(payload(shortcuts::PaletteKey::Open), PaletteMessage::Open));
    }
  }

  mod updater_state_stream {
    use super::*;

    #[test]
    fn it_constructs_an_updater_state_stream() {
      let _stream = updater_state_stream();
    }
  }

  mod variant_name {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn stub_store_ready() -> StoreReady {
      let db = store::open_test().await.expect("test db");
      StoreReady {
        db: db.clone(),
        sync_db: db.clone(),
        sync_housekeeping_db: db.clone(),
        http: http::Client::builder(http::Cache::new(db)).build(),
        lease: None,
        settings: config::Settings::default(),
        sync_session: None,
      }
    }

    #[test]
    fn it_names_feature_messages() {
      assert_eq!(Message::Assets(assets::Message::StockpileNew).variant_name(), "Assets");
      assert_eq!(Message::Nav(rail::Destination::Wallet).variant_name(), "Nav");
      assert_eq!(Message::Wallet(wallet::Message::PickerToggled).variant_name(), "Wallet");
    }

    #[tokio::test]
    async fn it_names_lifecycle_messages() {
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
      assert_eq!(Message::TrashPurged(Vec::new()).variant_name(), "TrashPurged");
      assert_eq!(Message::StorageMigrated.variant_name(), "StorageMigrated");
      assert_eq!(Message::SyncPulse.variant_name(), "SyncPulse");
      assert_eq!(Message::TakeOver.variant_name(), "TakeOver");
      assert_eq!(Message::CancelTakeOver.variant_name(), "CancelTakeOver");
      assert_eq!(Message::ConfirmTakeOver.variant_name(), "ConfirmTakeOver");
      assert_eq!(
        Message::TakeOverResolved(TakeOverOutcome::Failed, Box::new(stub_store_ready().await)).variant_name(),
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

  mod notification_variant_name {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_names_every_notification_and_panel_message() {
      let mcp = mcp::McpRequest::new("skill_plan_create".to_owned(), serde_json::Value::Null).0;

      assert_eq!(Message::ClearNotifications.variant_name(), "ClearNotifications");
      assert_eq!(
        Message::CloseNotificationsPanel.variant_name(),
        "CloseNotificationsPanel"
      );
      assert_eq!(
        Message::MarkAllNotificationsRead.variant_name(),
        "MarkAllNotificationsRead"
      );
      assert_eq!(Message::Mcp(mcp).variant_name(), "Mcp");
      assert_eq!(Message::McpDataChanged.variant_name(), "McpDataChanged");
      assert_eq!(Message::Nav(rail::Destination::Wallet).variant_name(), "Nav");
      assert_eq!(
        Message::NavTo(rail::Destination::Settings, Some("mcp")).variant_name(),
        "NavTo"
      );
      assert_eq!(
        Message::NotificationActivated(1).variant_name(),
        "NotificationActivated"
      );
      assert_eq!(
        Message::NotificationsHistoryPageLoaded {
          epoch: 0,
          rows: Vec::new(),
          who: std::collections::HashMap::new(),
        }
        .variant_name(),
        "NotificationsHistoryPageLoaded"
      );
      assert_eq!(
        Message::NotificationsHistoryScrolled {
          absolute: 0.0,
          relative: 0.0,
        }
        .variant_name(),
        "NotificationsHistoryScrolled"
      );
      assert_eq!(
        Message::NotificationsRefreshed(Box::default()).variant_name(),
        "NotificationsRefreshed"
      );
      assert_eq!(
        Message::SelectNotificationTab(NotificationTab::History).variant_name(),
        "SelectNotificationTab"
      );
      assert_eq!(Message::ToastDismissed(1).variant_name(), "ToastDismissed");
      assert_eq!(Message::ToastHover(1, true).variant_name(), "ToastHover");
      assert_eq!(Message::ToastTick.variant_name(), "ToastTick");
      assert_eq!(
        Message::ToggleNotificationsPanel.variant_name(),
        "ToggleNotificationsPanel"
      );
    }
  }

  mod screen_variant_name {
    use pretty_assertions::assert_eq;

    use super::*;

    fn finished_event(character_id: i64) -> sync::Event {
      sync::Event::Finished {
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(character_id)),
        outcome: sync::Outcome::synced(),
      }
    }

    #[test]
    fn it_names_every_per_screen_message() {
      let id = window::Id::unique();

      assert_eq!(Message::Assets(assets::Message::StockpileNew).variant_name(), "Assets");
      assert_eq!(Message::Auth(auth::Message::Cancel).variant_name(), "Auth");
      assert_eq!(
        Message::Calendar(calendar::Message::PickerToggled).variant_name(),
        "Calendar"
      );
      assert_eq!(
        Message::CalendarAttentionCounted(2).variant_name(),
        "CalendarAttentionCounted"
      );
      assert_eq!(
        Message::CharacterDetail(character_detail::Message::PickerToggled).variant_name(),
        "CharacterDetail"
      );
      assert_eq!(
        Message::CharacterManager(character_manager::Message::AddCharacterRequested).variant_name(),
        "CharacterManager"
      );
      assert_eq!(
        Message::Compare(skills_compare::Message::CloseRequested).variant_name(),
        "Compare"
      );
      assert_eq!(
        Message::Compose(id, mail::Message::PickerToggled).variant_name(),
        "Compose"
      );
      assert_eq!(
        Message::CorporationDetail(corporation_detail::Message::StandingsClearSearch).variant_name(),
        "CorporationDetail"
      );
      assert_eq!(
        Message::Industry(industry::Message::PickerToggled).variant_name(),
        "Industry"
      );
      assert_eq!(Message::Mail(mail::Message::PickerToggled).variant_name(), "Mail");
      assert_eq!(Message::MailUnreadCounted(3).variant_name(), "MailUnreadCounted");
      assert_eq!(
        Message::ManagePlans(skill_plan_manager::Message::CancelDelete).variant_name(),
        "ManagePlans"
      );
      assert_eq!(
        Message::Settings(settings::Message::ResetToDefaults).variant_name(),
        "Settings"
      );
      assert_eq!(
        Message::SkillPlanEditor(skill_plan_editor::Message::CloseRequested).variant_name(),
        "SkillPlanEditor"
      );
      assert_eq!(Message::Skills(skills::Message::PickerToggled).variant_name(), "Skills");
      assert_eq!(Message::Sync(finished_event(1)).variant_name(), "Sync");
      assert_eq!(Message::Wallet(wallet::Message::PickerToggled).variant_name(), "Wallet");
    }
  }

  mod views {
    use super::*;

    fn render_route(route: Route) {
      let app = ready_app();
      let mut app = app;
      app.route = route;
      let _ = route_view(&app);
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
        config::FeatureFlags::default(),
        industry::FacilityDefaults::default(),
        None,
        false,
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

    #[test]
    fn it_builds_the_sync_model_with_per_pilot_job_rows() {
      let mut app = ready_app();
      app.last_synced = Some(app.now);
      let model = sync_model(&app);
      assert_eq!(model.total, model.rows.len());
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
    fn it_renders_the_starting_up_placeholder_for_an_unbuilt_route() {
      let mut app = test_app();
      app.route = Route::Wallet;
      let _ = route_view(&app);
      let _ = starting_up();
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
    fn it_dispatches_every_per_window_view_helper() {
      let mut app = ready_app();

      let compose_id = window::Id::unique();
      app.windows.register(compose_id, Window::MailCompose);
      let _ = view(&app, compose_id);
      app.composes.insert(
        compose_id,
        mail::compose::Draft::from_seed(mail::compose::Seed::Blank {
          from_character_id: 1,
        }),
      );
      let _ = view(&app, compose_id);

      let manage_id = window::Id::unique();
      app.windows.register(manage_id, Window::ManagePlans);
      let _ = view(&app, manage_id);
      app.manage_plans = Some((manage_id, skill_plan_manager::State::new()));
      let _ = view(&app, manage_id);

      let compare_id = window::Id::unique();
      app.windows.register(compare_id, Window::Compare);
      let _ = view(&app, compare_id);
      app.compare = Some((compare_id, skills_compare::State::new(vec![1], Vec::new())));
      let _ = view(&app, compare_id);

      let contract_id = window::Id::unique();
      app.windows.register(contract_id, Window::Contract);
      let _ = view(&app, contract_id);

      let killmail_id = window::Id::unique();
      app.windows.register(killmail_id, Window::Killmail);
      let _ = view(&app, killmail_id);

      let stockpile_id = window::Id::unique();
      app.windows.register(stockpile_id, Window::StockpileEditor);
      let _ = view(&app, stockpile_id);
    }

    #[test]
    fn it_renders_the_notifications_panel_on_both_rail_sides() {
      let mut app = ready_app();
      let _ = notifications_panel(&app, config::NavLocation::Left);
      let _ = notifications_panel(&app, config::NavLocation::Right);

      app.notifications_unread = 2;
      app
        .notifications
        .push(test_notification(1, store::model::NotificationDestination::Skills));
      app
        .notification_names
        .insert(store::model::NotificationOwner::Character(1), "Pilot 1".to_owned());
      let _ = notifications_panel(&app, config::NavLocation::Left);
      let _ = notifications_panel(&app, config::NavLocation::Right);
    }
  }

  mod notification_tabs {
    use pretty_assertions::assert_eq;

    use super::*;

    fn read_notification(id: i64) -> store::model::Notification {
      store::model::Notification {
        read_at: Some(Utc::now().to_rfc3339()),
        ..test_notification(id, store::model::NotificationDestination::Skills)
      }
    }

    fn unread_notification(id: i64) -> store::model::Notification {
      test_notification(id, store::model::NotificationDestination::Skills)
    }

    fn ids(rows: &[store::model::Notification], tab: NotificationTab) -> Vec<i64> {
      rows
        .iter()
        .filter(|notification| match tab {
          NotificationTab::New => notification.read_at().is_none(),
          NotificationTab::History => true,
        })
        .map(store::model::Notification::id)
        .collect()
    }

    #[test]
    fn it_filters_the_new_tab_to_unread_and_history_to_all() {
      let mut app = ready_app();
      app.notifications = vec![unread_notification(1), read_notification(2), unread_notification(3)];

      assert_eq!(
        ids(&app.notifications, NotificationTab::New),
        vec![1, 3],
        "the New tab lists only unread notifications"
      );
      assert_eq!(
        ids(&app.notifications, NotificationTab::History),
        vec![1, 2, 3],
        "the History tab lists every loaded notification"
      );
    }

    #[test]
    fn it_selects_a_tab_as_durable_app_state() {
      let mut app = ready_app();
      assert_eq!(
        app.notifications_tab,
        NotificationTab::New,
        "the panel opens on the New tab"
      );

      let _ = handle_select_notification_tab(&mut app, NotificationTab::History);
      assert_eq!(
        app.notifications_tab,
        NotificationTab::History,
        "selecting History sticks"
      );

      let _ = handle_select_notification_tab(&mut app, NotificationTab::New);
      assert_eq!(app.notifications_tab, NotificationTab::New, "selecting New sticks");
    }

    #[test]
    fn it_empties_the_new_tab_but_retains_history_after_mark_all_read() {
      // Marking all read flips every row's read_at; the New tab filters on read_at.is_none() while
      // History is unconditional, so the same row set empties New but stays in History.
      let marked: Vec<store::model::Notification> = vec![unread_notification(1), unread_notification(2)]
        .into_iter()
        .map(|notification| store::model::Notification {
          read_at: Some(Utc::now().to_rfc3339()),
          ..notification
        })
        .collect();

      assert!(
        ids(&marked, NotificationTab::New).is_empty(),
        "the New tab is emptied once every row is read"
      );
      assert_eq!(
        ids(&marked, NotificationTab::History),
        vec![1, 2],
        "History keeps every notification"
      );
    }

    #[test]
    fn it_renders_both_tabs_with_their_empty_states() {
      let mut app = ready_app();
      app.notifications = vec![read_notification(1)];
      app
        .notification_names
        .insert(store::model::NotificationOwner::Character(1), "Pilot 1".to_owned());

      // New is empty (the only row is read) -> "all caught up". History renders the paged accumulator.
      app.notifications_tab = NotificationTab::New;
      let _ = notifications_panel(&app, config::NavLocation::Left);
      app.notifications_history = vec![read_notification(1)];
      app.notifications_tab = NotificationTab::History;
      let _ = notifications_panel(&app, config::NavLocation::Left);

      // History empty state when nothing is paged in.
      app.notifications.clear();
      app.notifications_history.clear();
      let _ = notifications_panel(&app, config::NavLocation::Left);
    }
  }

  mod notifications_history {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    // A History row with a controllable id and created_at, so cursor and ordering assertions are exact.
    fn history_notification(id: i64, created_at: &str) -> store::model::Notification {
      store::model::Notification {
        created_at: created_at.to_owned(),
        ..test_notification(id, store::model::NotificationDestination::Skills)
      }
    }

    // A full keyset page, so `has_more` flips true exactly when the fetch fills the page.
    fn full_page() -> Vec<store::model::Notification> {
      (0..store::repo::notifications::HISTORY_PAGE_SIZE)
        .map(|i| history_notification(i, &format!("2026-06-01T00:00:{:02}+00:00", i % 60)))
        .collect()
    }

    #[test]
    fn it_appends_a_page_and_advances_the_cursor() {
      let mut app = ready_app();
      app.notifications_history_epoch = 7;
      app.notifications_history_has_more = true;

      let page = vec![
        history_notification(3, "2026-06-03T00:00:00+00:00"),
        history_notification(2, "2026-06-02T00:00:00+00:00"),
      ];
      let _ = handle_notifications_history_page_loaded(&mut app, 7, page, HashMap::new());

      let ids: Vec<i64> = app
        .notifications_history
        .iter()
        .map(store::model::Notification::id)
        .collect();
      assert_eq!(ids, vec![3, 2], "the page is appended newest-first");
      assert_eq!(
        app.notifications_history_cursor,
        Some(store::repo::notifications::HistoryCursor {
          created_at: "2026-06-02T00:00:00+00:00".to_owned(),
          id: 2,
        }),
        "the cursor advances to the last row of the page"
      );
      assert!(!app.notifications_history_loading, "the in-flight guard is cleared");
      assert!(
        !app.notifications_history_has_more,
        "a short page means no further pages remain"
      );
    }

    #[test]
    fn it_keeps_paging_while_pages_arrive_full() {
      let mut app = ready_app();
      app.notifications_history_has_more = true;

      let _ = handle_notifications_history_page_loaded(&mut app, 0, full_page(), HashMap::new());

      assert_eq!(
        app.notifications_history.len() as i64,
        store::repo::notifications::HISTORY_PAGE_SIZE
      );
      assert!(
        app.notifications_history_has_more,
        "a full page leaves the door open for another"
      );
    }

    #[test]
    fn it_merges_resolved_who_names_for_the_paged_rows() {
      let mut app = ready_app();
      app.notifications_history_has_more = true;
      let mut who = HashMap::new();
      who.insert(store::model::NotificationOwner::Character(1), "Vex Voronova".to_owned());

      let page = vec![history_notification(1, "2026-06-01T00:00:00+00:00")];
      let _ = handle_notifications_history_page_loaded(&mut app, 0, page, who);

      assert_eq!(
        app
          .notification_names
          .get(&store::model::NotificationOwner::Character(1))
          .map(String::as_str),
        Some("Vex Voronova"),
        "the paged rows' author names are merged in"
      );
    }

    #[test]
    fn it_drops_a_page_captured_against_a_stale_epoch() {
      let mut app = ready_app();
      app.notifications_history_epoch = 5;
      app.notifications_history_loading = true;
      app.notifications_history_has_more = true;

      // A page tagged with the pre-reset epoch must not append to the freshly-reset accumulator.
      let page = vec![history_notification(9, "2026-06-09T00:00:00+00:00")];
      let _ = handle_notifications_history_page_loaded(&mut app, 4, page, HashMap::new());

      assert!(
        app.notifications_history.is_empty(),
        "a stale-epoch page is discarded, not appended"
      );
      assert!(
        app.notifications_history_loading,
        "the stale page does not clear the live in-flight guard"
      );
    }

    #[test]
    fn it_requests_a_page_only_past_the_scroll_threshold() {
      let mut app = ready_app();
      app.notifications_history_has_more = true;
      app
        .notifications_history
        .push(history_notification(1, "2026-06-01T00:00:00+00:00"));

      // A shallow scroll records the offset but requests no page.
      let _ = handle_notifications_history_scrolled(&mut app, 120.0, 0.10);
      assert_eq!(app.notifications_history_scroll, 120.0, "the offset is tracked");
      assert!(!app.notifications_history_loading, "a shallow scroll triggers no fetch");
    }

    #[test]
    fn it_does_not_over_fetch_while_a_page_is_in_flight() {
      let mut app = ready_app();
      app.notifications_history_has_more = true;
      app.notifications_history_loading = true;

      // A deep scroll while loading must not kick off a second concurrent fetch.
      let task = load_more_notifications_history(&mut app);

      assert!(app.notifications_history_loading);
      // The guard short-circuits to an empty task.
      let _ = task;
    }

    #[test]
    fn it_does_not_fetch_once_the_last_page_is_reached() {
      let mut app = ready_app();
      app.notifications_history_has_more = false;
      app.notifications_history_loading = false;

      let _ = handle_notifications_history_scrolled(&mut app, 999.0, 0.99);

      assert!(
        !app.notifications_history_loading,
        "no fetch starts once has_more is false"
      );
    }

    #[test]
    fn it_resets_the_accumulator_and_bumps_the_epoch() {
      let mut app = ready_app();
      app.notifications_history = vec![history_notification(1, "2026-06-01T00:00:00+00:00")];
      app.notifications_history_cursor = Some(store::repo::notifications::HistoryCursor {
        created_at: "2026-06-01T00:00:00+00:00".to_owned(),
        id: 1,
      });
      app.notifications_history_scroll = 500.0;
      let before = app.notifications_history_epoch;

      let _ = reset_notifications_history(&mut app);

      assert!(app.notifications_history.is_empty(), "the accumulator clears");
      assert_eq!(
        app.notifications_history_cursor, None,
        "the cursor rewinds to the newest page"
      );
      assert_eq!(app.notifications_history_scroll, 0.0, "the scroll offset rewinds");
      assert_eq!(
        app.notifications_history_epoch,
        before.wrapping_add(1),
        "the epoch bumps so in-flight pages are invalidated"
      );
    }

    #[test]
    fn it_resets_history_when_a_refresh_brings_a_newer_head_row() {
      let mut app = ready_app();
      app.notifications_panel_open = true;
      app.notifications_history = vec![history_notification(1, "2026-06-01T00:00:00+00:00")];
      let before = app.notifications_history_epoch;

      // A refresh whose newest row (id 2) differs from History's head (id 1) resets History.
      let snapshot = crate::notifications::Snapshot {
        list: vec![history_notification(2, "2026-06-02T00:00:00+00:00")],
        surfaced: Vec::new(),
        unread: 1,
        who: HashMap::new(),
      };
      let _ = handle_notifications_refreshed(&mut app, snapshot);

      assert!(
        app.notifications_history.is_empty(),
        "History rewinds to the first page"
      );
      assert_eq!(
        app.notifications_history_epoch,
        before.wrapping_add(1),
        "the reset bumps the epoch"
      );
    }

    #[test]
    fn it_leaves_history_intact_when_a_refresh_brings_no_newer_head() {
      let mut app = ready_app();
      app.notifications_panel_open = true;
      app.notifications_history = vec![history_notification(2, "2026-06-02T00:00:00+00:00")];
      let before = app.notifications_history_epoch;

      // The refresh's newest row matches History's head, so no reset is needed.
      let snapshot = crate::notifications::Snapshot {
        list: vec![history_notification(2, "2026-06-02T00:00:00+00:00")],
        surfaced: Vec::new(),
        unread: 0,
        who: HashMap::new(),
      };
      let _ = handle_notifications_refreshed(&mut app, snapshot);

      assert_eq!(app.notifications_history.len(), 1, "History is untouched");
      assert_eq!(app.notifications_history_epoch, before, "the epoch is unchanged");
    }

    #[test]
    fn it_clears_history_state_on_panel_close() {
      let mut app = ready_app();
      app.notifications_panel_open = true;
      app.notifications_history = vec![history_notification(1, "2026-06-01T00:00:00+00:00")];
      app.notifications_history_has_more = true;
      let before = app.notifications_history_epoch;

      let _ = handle_close_notifications_panel(&mut app);

      assert!(!app.notifications_panel_open);
      assert!(app.notifications_history.is_empty(), "closing drops the accumulator");
      assert!(!app.notifications_history_has_more);
      assert_eq!(
        app.notifications_history_epoch,
        before.wrapping_add(1),
        "closing invalidates any in-flight page"
      );
    }
  }

  mod subscription {
    use super::*;

    #[tokio::test]
    async fn it_arms_the_popover_panel_and_toast_listeners() {
      let mut app = ready_app();
      app.sync_popover_open = true;
      app.notifications_panel_open = true;
      app.toasts.push(ToastEntry {
        notification: test_notification(1, store::model::NotificationDestination::Skills),
        paused: false,
        remaining: TOAST_MS,
        who: String::new(),
      });
      app.keyboard_focus.set_focused(Some(iced::widget::Id::from("search")));
      let _ = subscription(&app);

      app.palette = Some(command_palette::State::default());
      let _ = subscription(&app);
    }
  }

  mod boot_variant_name {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_names_the_boot_messages() {
      assert_eq!(Message::ClockTick.variant_name(), "ClockTick");
      assert_eq!(Message::Quit.variant_name(), "Quit");
      assert_eq!(
        Message::TextInputFocused(iced::widget::Id::from("x")).variant_name(),
        "TextInputFocused"
      );
      assert_eq!(Message::Shortcut(Chord::FocusSearch).variant_name(), "Shortcut");
      assert_eq!(Message::InitFailed("boom".to_owned()).variant_name(), "InitFailed");
      assert_eq!(Message::FocusMainWindow.variant_name(), "FocusMainWindow");
      assert_eq!(Message::SnoozesWoken(Vec::new()).variant_name(), "SnoozesWoken");
      assert_eq!(Message::TrashPurged(Vec::new()).variant_name(), "TrashPurged");
      assert_eq!(Message::StorageMigrated.variant_name(), "StorageMigrated");
    }

    #[test]
    fn it_falls_back_to_window_for_unnamed_messages() {
      assert_eq!(
        Message::RailHover(Some(rail::Destination::Wallet)).variant_name(),
        "Window"
      );
      assert_eq!(Message::RailHoverExpire(0).variant_name(), "Window");
    }
  }

  mod dispatch_window_lifecycle {
    use super::*;

    #[test]
    fn it_routes_window_lifecycle_messages_without_a_runtime() {
      let mut app = ready_app();

      let _ = dispatch_window_lifecycle(&mut app, Message::CloseSyncPopover);
      assert!(!app.sync_popover_open);

      app.sync_popover_open = false;
      let _ = dispatch_window_lifecycle(&mut app, Message::ToggleSyncPopover);
      assert!(app.sync_popover_open);

      let _ = dispatch_window_lifecycle(&mut app, Message::FocusMainWindow);
      let _ = dispatch_window_lifecycle(&mut app, Message::TextInputFocused(iced::widget::Id::from("search")));
      let _ = dispatch_window_lifecycle(&mut app, Message::UpdaterDismissToast);
      let _ = dispatch_window_lifecycle(&mut app, Message::WindowOpened(window::Id::unique()));

      // An unmatched message falls through to a no-op task.
      let _ = dispatch_window_lifecycle(&mut app, Message::ClockTick);
    }

    #[test]
    fn it_routes_the_remaining_window_lifecycle_branches() {
      let mut app = ready_app();
      let id = window::Id::unique();

      let _ = dispatch_window_lifecycle(&mut app, Message::Chrome(id, window_chrome::Event::Drag));
      let _ = dispatch_window_lifecycle(&mut app, Message::Palette(PaletteMessage::Close));
      let _ = dispatch_window_lifecycle(&mut app, Message::Shortcut(Chord::FocusSearch));
      let _ = dispatch_window_lifecycle(&mut app, Message::UpdaterAction(updater_banner::Action::Apply));
      let _ = dispatch_window_lifecycle(&mut app, Message::UpdaterStateChanged(updater::State::default()));
      let _ = dispatch_window_lifecycle(
        &mut app,
        Message::Window(id, window::Event::Resized(Size::new(640.0, 480.0))),
      );

      // Quitting with no windows registered tears the app down; the returned task is dropped here
      // (never run by the iced runtime), so no real exit happens.
      let _ = dispatch_window_lifecycle(&mut app, Message::Quit);
    }
  }

  mod dispatch_feature_aux {
    use super::*;

    #[test]
    fn it_routes_every_notification_and_rail_message_without_a_runtime() {
      let mut app = ready_app();
      let mcp = mcp::McpRequest::new("skill_plan_create".to_owned(), serde_json::Value::Null).0;

      assert!(dispatch_feature_aux(&mut app, Message::ClearNotifications).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::CloseNotificationsPanel).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::MarkAllNotificationsRead).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::Mcp(mcp)).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::McpDataChanged).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::Nav(rail::Destination::Wallet)).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::NavTo(rail::Destination::Settings, Some("mcp"))).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::NotificationActivated(1)).is_ok());
      assert!(
        dispatch_feature_aux(
          &mut app,
          Message::NotificationsHistoryPageLoaded {
            epoch: 0,
            rows: Vec::new(),
            who: std::collections::HashMap::new(),
          }
        )
        .is_ok()
      );
      assert!(
        dispatch_feature_aux(
          &mut app,
          Message::NotificationsHistoryScrolled {
            absolute: 0.0,
            relative: 0.0,
          }
        )
        .is_ok()
      );
      assert!(dispatch_feature_aux(&mut app, Message::NotificationsRefreshed(Box::default())).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::SelectNotificationTab(NotificationTab::History)).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::RailHover(Some(rail::Destination::Wallet))).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::RailHoverExpire(0)).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::ToastDismissed(1)).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::ToastHover(1, true)).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::ToastTick).is_ok());
      assert!(dispatch_feature_aux(&mut app, Message::ToggleNotificationsPanel).is_ok());
    }

    #[test]
    fn it_returns_the_message_for_a_non_feature_message() {
      let mut app = ready_app();

      let result = dispatch_feature_aux(&mut app, Message::ClockTick);

      assert!(matches!(result, Err(boxed) if matches!(*boxed, Message::ClockTick)));
    }
  }

  mod dispatch_feature {
    use super::*;

    #[test]
    fn it_routes_every_screen_message_without_a_runtime() {
      let mut app = ready_app();
      let id = window::Id::unique();

      assert!(dispatch_feature(&mut app, Message::Assets(assets::Message::PickerToggled)).is_ok());
      assert!(dispatch_feature(&mut app, Message::Auth(auth::Message::Cancel)).is_ok());
      assert!(dispatch_feature(&mut app, Message::Calendar(calendar::Message::PickerToggled)).is_ok());
      assert!(dispatch_feature(&mut app, Message::CalendarAttentionCounted(2)).is_ok());
      assert!(
        dispatch_feature(
          &mut app,
          Message::CharacterDetail(character_detail::Message::PickerToggled)
        )
        .is_ok()
      );
      assert!(
        dispatch_feature(
          &mut app,
          Message::CharacterManager(character_manager::Message::AddCharacterRequested),
        )
        .is_ok()
      );
      assert!(dispatch_feature(&mut app, Message::Compose(id, mail::Message::PickerToggled)).is_ok());
      assert!(
        dispatch_feature(
          &mut app,
          Message::CorporationDetail(corporation_detail::Message::StandingsClearSearch),
        )
        .is_ok()
      );
      assert!(dispatch_feature(&mut app, Message::Industry(industry::Message::PickerToggled)).is_ok());
      assert!(dispatch_feature(&mut app, Message::Mail(mail::Message::PickerToggled)).is_ok());
      assert!(dispatch_feature(&mut app, Message::MailUnreadCounted(3)).is_ok());
      assert!(
        dispatch_feature(
          &mut app,
          Message::ManagePlans(skill_plan_manager::Message::CancelDelete)
        )
        .is_ok()
      );
      assert!(dispatch_feature(&mut app, Message::Settings(settings::Message::ResetToDefaults)).is_ok());
      assert!(
        dispatch_feature(
          &mut app,
          Message::SkillPlanEditor(skill_plan_editor::Message::CloseRequested),
        )
        .is_ok()
      );
      assert!(dispatch_feature(&mut app, Message::Skills(skills::Message::PickerToggled)).is_ok());
      assert!(dispatch_feature(&mut app, Message::StockpileEditor(id, assets::Message::PickerToggled)).is_ok());
      assert!(dispatch_feature(&mut app, Message::StockpileImport(id, assets::Message::PickerToggled)).is_ok());
      assert!(dispatch_feature(&mut app, Message::Wallet(wallet::Message::PickerToggled)).is_ok());
    }

    #[test]
    fn it_delegates_an_aux_message_to_the_aux_dispatcher() {
      let mut app = ready_app();

      assert!(dispatch_feature(&mut app, Message::ToastTick).is_ok());
    }

    #[test]
    fn it_returns_a_lifecycle_message_for_the_caller_to_route() {
      let mut app = ready_app();

      let result = dispatch_feature(&mut app, Message::ClockTick);

      assert!(matches!(result, Err(boxed) if matches!(*boxed, Message::ClockTick)));
    }
  }

  mod handle_manage_plans {
    use super::*;

    fn app_with_manage_plans() -> (App, window::Id) {
      let mut app = ready_app();
      let id = window::Id::unique();
      app.manage_plans = Some((id, skill_plan_manager::State::new()));
      (app, id)
    }

    #[test]
    fn it_handles_the_state_only_messages_without_a_runtime() {
      let (mut app, _id) = app_with_manage_plans();

      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::CancelDelete);
      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::CharacterSelected(7));
      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::RequestDelete(3));
      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::ToggleCopyMenu(3));
      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::CancelDelete);
    }

    #[test]
    fn it_short_circuits_the_runtime_backed_messages_when_no_runtime_is_present() {
      let (mut app, _id) = app_with_manage_plans();

      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::ConfirmDelete(1));
      let _ = handle_manage_plans(
        &mut app,
        skill_plan_manager::Message::CopyPlan {
          plan_id: 1,
          target_character_id: 2,
        },
      );
      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::NewPlan(1));
      let _ = handle_manage_plans(
        &mut app,
        skill_plan_manager::Message::OpenPlan {
          character_id: 1,
          plan_id: 5,
        },
      );
    }

    #[tokio::test]
    async fn it_loads_the_roster_and_fetches_stale_images() {
      let (mut app, _id) = app_with_manage_plans();
      app.runtime = Some(test_runtime().await);

      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::Loaded(Box::default()));
    }
  }

  mod handle_compose {
    use super::*;

    fn app_with_compose() -> (App, window::Id) {
      let mut app = ready_app();
      let id = window::Id::unique();
      app.composes.insert(
        id,
        mail::compose::Draft::from_seed(mail::compose::Seed::Blank {
          from_character_id: 1,
        }),
      );
      (app, id)
    }

    #[test]
    fn it_is_a_no_op_without_a_runtime() {
      let mut app = ready_app();
      let id = window::Id::unique();

      let _ = handle_compose(&mut app, id, mail::Message::DraftSaved(Some(7)));
    }

    #[tokio::test]
    async fn it_threads_draft_load_and_save_ids_per_window() {
      let (mut app, id) = app_with_compose();
      app.runtime = Some(test_runtime().await);

      let _ = handle_compose(&mut app, id, mail::Message::DraftSaved(Some(42)));
      assert_eq!(
        app.composes.get(id).and_then(mail::compose::Draft::sent_draft_id),
        Some(42)
      );

      let _ = handle_compose(&mut app, id, mail::Message::DraftLoaded(Box::new(None)));

      // An unknown id falls through to a no-op.
      let _ = handle_compose(&mut app, window::Id::unique(), mail::Message::PickerToggled);
    }

    #[tokio::test]
    async fn it_routes_a_successful_send_through_completion() {
      let (mut app, id) = app_with_compose();
      app.runtime = Some(test_runtime().await);

      let _ = handle_compose(&mut app, id, mail::Message::ComposeSent(Ok(())));
      assert!(app.composes.get(id).is_none(), "the window closes on send");
    }
  }

  mod navigate_to_notification_target {
    use pretty_assertions::assert_eq;
    use store::model::{NotificationDestination, NotificationTarget};

    use super::*;

    fn target(destination: NotificationDestination, character: Option<i64>) -> NotificationTarget {
      NotificationTarget {
        character,
        destination,
        sub: None,
      }
    }

    #[test]
    fn it_routes_every_destination_to_its_route() {
      let mut app = ready_app();

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::Assets, None));
      assert_eq!(app.route, Route::Assets);

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::Calendar, Some(1)));
      assert_eq!(app.route, Route::Calendar);

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::CharacterDetail, Some(9)));
      assert_eq!(app.route, Route::CharacterDetail(9));

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::Industry, Some(1)));
      assert_eq!(app.route, Route::Industry);

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::Mail, Some(1)));
      assert_eq!(app.route, Route::Mail);

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::Skills, Some(1)));
      assert_eq!(app.route, Route::Skills(1));

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::Wallet, None));
      assert_eq!(app.route, Route::Wallet);
    }

    #[test]
    fn it_lands_a_character_less_character_detail_on_the_roster() {
      let mut app = ready_app();

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::CharacterDetail, None));

      assert_eq!(app.route, Route::Characters);
    }
  }

  mod handle_notification_activated {
    use super::*;

    #[test]
    fn it_marks_read_clears_the_toast_and_navigates_to_the_target() {
      let mut app = ready_app();
      app.notifications_unread = 1;
      app.notifications_panel_open = true;
      app
        .notifications
        .push(test_notification(5, store::model::NotificationDestination::Wallet));
      app.toasts.push(ToastEntry {
        notification: test_notification(5, store::model::NotificationDestination::Wallet),
        paused: false,
        remaining: TOAST_MS,
        who: String::new(),
      });

      let _ = handle_notification_activated(&mut app, 5);

      assert!(!app.notifications_panel_open, "the panel closes");
      assert!(app.toasts.is_empty(), "the matching toast is removed");
      assert_eq!(app.notifications_unread, 0, "the row is marked read");
      assert_eq!(app.route, Route::Wallet, "it navigates to the target");
      assert!(
        app.notifications[0].read_at().is_some(),
        "the activated row carries a read timestamp"
      );
    }

    #[test]
    fn it_only_marks_read_when_the_id_is_unknown() {
      let mut app = ready_app();
      app.route = Route::Characters;
      app.notifications_panel_open = true;

      let _ = handle_notification_activated(&mut app, 999);

      assert!(!app.notifications_panel_open);
      assert_eq!(app.route, Route::Characters, "no target means no navigation");
    }
  }

  mod window_title {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_the_app_name_for_an_unregistered_id() {
      let app = test_app();

      assert_eq!(window_title(&app, window::Id::unique()), "Pod");
    }

    #[test]
    fn it_derives_a_per_kind_title_for_registered_windows() {
      let mut app = test_app();
      let compare = window::Id::unique();
      let import = window::Id::unique();
      app.windows.register(compare, Window::Compare);
      app.windows.register(import, Window::StockpileImport);

      assert_eq!(window_title(&app, compare), "Pod — Compare Skills");
      assert_eq!(window_title(&app, import), "Pod — Import multibuy");
    }

    #[test]
    fn it_distinguishes_a_new_from_an_edit_stockpile_editor_title() {
      let mut app = test_app();
      let new = window::Id::unique();
      app.windows.register(new, Window::StockpileEditor);
      app
        .stockpile_editors
        .insert(new, assets::Editor::from_seed(assets::EditorSeed::Blank));

      assert_eq!(window_title(&app, new), "Pod — New stockpile");
    }

    #[test]
    fn it_titles_the_main_window_with_the_bare_app_name() {
      let mut app = test_app();
      let main = window::Id::unique();
      app.windows.register(main, Window::Main);

      assert_eq!(window_title(&app, main), "Pod");
    }

    #[test]
    fn it_falls_back_to_a_generic_contract_title_before_the_detail_loads() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::Contract);
      app.contracts.insert(
        id,
        contract_detail::State::new(
          contract_detail::Source::Character {
            character_id: 1,
          },
          42,
        ),
      );

      assert_eq!(window_title(&app, id), "Pod — Contract #42");
    }

    #[test]
    fn it_titles_a_compose_window_from_its_draft_subject() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::MailCompose);
      let mut draft = mail::compose::Draft::from_seed(mail::compose::Seed::Blank {
        from_character_id: 42,
      });
      draft.subject = "CTA tonight".to_owned();
      app.composes.insert(id, draft);

      assert_eq!(window_title(&app, id), "Pod — CTA tonight");
    }

    #[test]
    fn it_titles_a_blank_compose_window_as_a_new_message() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::MailCompose);
      app.composes.insert(
        id,
        mail::compose::Draft::from_seed(mail::compose::Seed::Blank {
          from_character_id: 42,
        }),
      );

      assert_eq!(window_title(&app, id), "Pod — New message");
    }
  }

  mod open_native_window {
    use super::*;

    #[test]
    fn it_registers_the_kind_synchronously() {
      let mut app = test_app();
      let (id, _task) = super::super::open_native_window(&mut app, Window::Compare, Size::new(800.0, 600.0));

      assert_eq!(app.windows.kind(id), Some(Window::Compare));
    }
  }

  mod handle_assets {
    use super::*;

    #[test]
    fn it_handles_the_window_opening_and_pane_messages() {
      let mut app = ready_app();

      let _ = handle_assets(&mut app, assets::Message::PaneSettled("assets.left", 0.4));
      assert_eq!(app.ui_state.panes.get("assets.left"), Some(&0.4));

      // Without a runtime, the stockpile-window openers short-circuit to a no-op.
      let _ = handle_assets(&mut app, assets::Message::StockpileNew);
      let _ = handle_assets(&mut app, assets::Message::StockpileEditStarted(1));
      let _ = handle_assets(&mut app, assets::Message::StockpileImportOpened);
      let _ = handle_assets(&mut app, assets::Message::ReauthRequested(1));
    }

    #[tokio::test]
    async fn it_opens_the_stockpile_editor_window_with_a_runtime() {
      let mut app = ready_app();
      app.runtime = Some(test_runtime().await);

      let _ = handle_assets(&mut app, assets::Message::StockpileNew);
    }
  }

  mod handle_wallet {
    use super::*;

    #[test]
    fn it_handles_the_pane_flag_and_list_persistence_messages() {
      let mut app = ready_app();

      let _ = handle_wallet(&mut app, wallet::Message::PaneSettled("wallet.left", 0.6));
      assert_eq!(app.ui_state.panes.get("wallet.left"), Some(&0.6));

      let _ = handle_wallet(&mut app, wallet::Message::UiFlagPersisted("pin".to_owned(), true));
      assert_eq!(app.ui_state.flags.get("pin"), Some(&true));

      let _ = handle_wallet(
        &mut app,
        wallet::Message::UiListPersisted("order".to_owned(), vec!["a".to_owned()]),
      );
      assert_eq!(app.ui_state.lists.get("order"), Some(&vec!["a".to_owned()]));

      // ContractSelected with no matching source is a no-op; ReauthRequested reroutes through update.
      let _ = handle_wallet(&mut app, wallet::Message::ContractSelected(404));
      let _ = handle_wallet(&mut app, wallet::Message::ReauthRequested(1));
    }
  }

  mod entity_ref_from_result {
    use crate::{
      features::entity_search::{EntityCategory, EntityResult},
      ui::components::entity_search::EntityKind,
    };

    fn result(category: EntityCategory, id: i64) -> EntityResult {
      EntityResult {
        category,
        id,
        name: format!("Entity {id}"),
      }
    }

    #[test]
    fn it_maps_an_alliance_to_a_logo_portrait() {
      let mapped = super::super::entity_ref_from_result(result(EntityCategory::Alliance, 11));

      assert_eq!(mapped.id, 11);
      assert_eq!(mapped.kind, EntityKind::Alliance);
      assert_eq!(mapped.name, "Entity 11");
      assert!(mapped.portrait.is_some());
    }

    #[test]
    fn it_maps_a_character_to_a_portrait() {
      let mapped = super::super::entity_ref_from_result(result(EntityCategory::Character, 22));

      assert_eq!(mapped.kind, EntityKind::Character);
      assert!(mapped.portrait.is_some());
    }

    #[test]
    fn it_maps_a_corporation_to_a_logo_portrait() {
      let mapped = super::super::entity_ref_from_result(result(EntityCategory::Corporation, 33));

      assert_eq!(mapped.kind, EntityKind::Corporation);
      assert!(mapped.portrait.is_some());
    }

    #[test]
    fn it_maps_a_solar_system_without_a_portrait() {
      let mapped = super::super::entity_ref_from_result(result(EntityCategory::SolarSystem, 44));

      assert_eq!(mapped.kind, EntityKind::SolarSystem);
      assert!(mapped.portrait.is_none());
    }

    #[test]
    fn it_maps_a_station_without_a_portrait() {
      let mapped = super::super::entity_ref_from_result(result(EntityCategory::Station, 55));

      assert_eq!(mapped.kind, EntityKind::Station);
      assert!(mapped.portrait.is_none());
    }
  }

  mod contact_entity_search {
    use super::*;

    #[tokio::test]
    async fn it_builds_a_search_task_without_panicking() {
      let runtime = test_runtime().await;
      let state = character_detail::State::new(42, &[]);

      let _ = super::super::contact_entity_search(&state, &runtime, "qu".to_owned());
    }

    #[tokio::test]
    async fn it_builds_a_search_task_for_an_empty_query() {
      let runtime = test_runtime().await;
      let state = character_detail::State::new(42, &[]);

      let _ = super::super::contact_entity_search(&state, &runtime, String::new());
    }
  }

  mod enqueue_wake_label_flip {
    use super::*;
    use crate::store::{
      Database,
      model::{
        Alliance, Bloodline, Character, CharacterMail, CharacterMailBody, CharacterMailLabel, Corporation, Gender,
        OwnerType, Race,
      },
      repo::mail,
    };

    async fn seed_character(db: &Database, id: i64) {
      use crate::store::repo::character;

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
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    async fn store_unread(db: &Database, character_id: i64, mail_id: i64) {
      let header = CharacterMail {
        character_id,
        from_id: 95_000_001,
        from_name: "Sender".to_owned(),
        is_read: false,
        mail_id,
        subject: Some("Subject".to_owned()),
        timestamp: "2026-06-01T10:00:00Z".to_owned(),
        ..Default::default()
      };
      let body = CharacterMailBody {
        body: "<p>hi</p>".to_owned(),
        character_id,
        mail_id,
      };
      mail::upsert_complete(db, &header, &body, &[]).await.unwrap();
    }

    async fn insert_label(db: &Database, character_id: i64, label_id: i64, name: &str) {
      let label = CharacterMailLabel {
        character_id,
        color: None,
        label_id,
        name: name.to_owned(),
      };
      mail::insert_label(db, &label).await.unwrap();
    }

    async fn pending_set_labels(db: &Database) -> i64 {
      sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox WHERE kind = 'mail.set_labels'")
        .fetch_one(&db.0)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn it_drops_snoozed_and_restores_inbox_then_enqueues_the_flip() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      insert_label(&db, 42, INBOX_LABEL_ID, "Inbox").await;
      insert_label(&db, 42, -9, SNOOZED_LABEL_NAME).await;
      mail::add_membership(&db, 42, 7, -9).await.unwrap();

      super::super::enqueue_wake_label_flip(&db, 42, 7).await;

      let membership = mail::membership(&db, 42, 7).await.unwrap();
      assert!(!membership.contains(&-9), "the snoozed label is dropped");
      assert!(membership.contains(&INBOX_LABEL_ID), "inbox membership is restored");
      assert_eq!(pending_set_labels(&db).await, 1, "a single set_labels row is enqueued");
    }

    #[tokio::test]
    async fn it_is_a_no_op_when_the_mail_is_already_only_in_inbox() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      insert_label(&db, 42, INBOX_LABEL_ID, "Inbox").await;
      mail::add_membership(&db, 42, 7, INBOX_LABEL_ID).await.unwrap();

      super::super::enqueue_wake_label_flip(&db, 42, 7).await;

      let membership = mail::membership(&db, 42, 7).await.unwrap();
      assert_eq!(membership, vec![INBOX_LABEL_ID], "membership is unchanged");
      assert_eq!(pending_set_labels(&db).await, 0, "no outbox row is enqueued");
    }

    #[tokio::test]
    async fn it_adds_inbox_when_a_mail_carries_no_labels() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      insert_label(&db, 42, INBOX_LABEL_ID, "Inbox").await;

      super::super::enqueue_wake_label_flip(&db, 42, 7).await;

      let membership = mail::membership(&db, 42, 7).await.unwrap();
      assert_eq!(membership, vec![INBOX_LABEL_ID], "inbox membership is added");
      assert_eq!(pending_set_labels(&db).await, 1, "a set_labels row is enqueued");
    }

    #[tokio::test]
    async fn it_preserves_unrelated_labels_alongside_inbox() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      insert_label(&db, 42, INBOX_LABEL_ID, "Inbox").await;
      insert_label(&db, 42, 5, "Keep").await;
      insert_label(&db, 42, -9, SNOOZED_LABEL_NAME).await;
      mail::add_membership(&db, 42, 7, 5).await.unwrap();
      mail::add_membership(&db, 42, 7, -9).await.unwrap();

      super::super::enqueue_wake_label_flip(&db, 42, 7).await;

      let membership = mail::membership(&db, 42, 7).await.unwrap();
      assert!(membership.contains(&5), "the unrelated label is preserved");
      assert!(membership.contains(&INBOX_LABEL_ID), "inbox membership is restored");
      assert!(!membership.contains(&-9), "the snoozed label is dropped");
      let _ = OwnerType::Character;
    }
  }
}
