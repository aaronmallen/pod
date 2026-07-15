mod boot;
mod dispatch;
mod graphics;
mod image_heal;
mod lease;
mod lifecycle;
mod logging;
mod navigation;
mod notification_center;
mod pack_open;
mod palette;
mod settings_ops;
mod shortcuts;
mod snooze_scheduler;
mod status_bar;
mod trash_purge_scheduler;
mod windows;

use std::{
  collections::HashSet,
  sync::{Arc, OnceLock},
  time::{Duration, Instant, SystemTime},
};

use boot::*;
use chrono::{DateTime, Utc};
use dispatch::*;
use iced::{
  Background, Element, Length, Padding, Point, Size, Subscription, Task,
  alignment::{Horizontal, Vertical},
  futures::SinkExt as _,
  keyboard,
  widget::{Column, Row, Space, Stack, container, mouse_area, scrollable, text},
  window,
};
use image_heal::*;
use lease::*;
use lifecycle::*;
use logging::*;
use navigation::*;
use notification_center::*;
use pack_open::*;
use palette::*;
use settings_ops::*;
use shortcuts::{Chord, FocusTracker};
use status_bar::*;
use windows::*;

use crate::{
  clients::{self, esi, eve_image, eve_sso, http},
  config,
  features::{
    assets, calendar, industry, mail, market, roster,
    roster::{OwnedPilot, auth, captains_log, character_detail, contact_sync, corporation_detail, killmail_detail},
    settings,
    shell::{
      command_palette::{
        self, Action as PaletteAction, Command as PaletteCommand, Entity as PaletteEntity,
        EntityKind as PaletteEntityKind,
      },
      focus_search, notifications, registry,
      window_state::{self, UiState, WindowGeometry, coalesce::WriteCoalescer, validity},
    },
    skills,
    skills::{skill_plan_editor, skill_plan_manager, skills_compare},
    splash, wallet,
    wallet::contract_detail,
    wizard,
  },
  services::{i18n, images, mcp, migration, telemetry, updater},
  store,
  sync::{self, FreshnessSummary, JobKey, JobKind},
  ui::{
    components::{
      backdrop,
      button::{Button, Size as ButtonSize},
      esi_status::esi_status,
      eve_time::eve_time,
      modal_overlay::modal_layers,
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

const RAIL_HOVER_GRACE: Duration = Duration::from_millis(160);

const REACQUIRE_INTERVAL: Duration = Duration::from_secs(30);

const REQUEST_POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Trailing-debounce window for the heavy `load_roster_at` reload. A sync burst re-marks
/// `roster_dirty` on every `Finished` event, and the 450ms pulse would otherwise drain it ~2x/s.
/// Collapsing those to one reload per window keeps the interactive reader pool from starving while
/// still refreshing the roster within a couple of pulses of the burst settling.
const ROSTER_RELOAD_DEBOUNCE: Duration = Duration::from_millis(1500);

const RUNTIME_CHANNEL_BUFFER: usize = 64;

const SCALE_MAX: u8 = 150;

const SCALE_MIN: u8 = 85;

const TELEMETRY_FLUSH_INTERVAL: Duration = Duration::from_secs(60);

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

const TICK_NOTIFICATIONS: u64 = 10;

const TOAST_CAP: usize = 3;

const TOAST_MS: Duration = Duration::from_secs(15);

const TOAST_TICK: Duration = Duration::from_millis(100);

const ZERO_GEOMETRY: WindowGeometry = WindowGeometry {
  height: 0.0,
  width: 0.0,
  x: 0.0,
  y: 0.0,
};

static UPDATER_RECEIVER: std::sync::Mutex<Option<tokio::sync::watch::Receiver<updater::State>>> =
  std::sync::Mutex::new(None);

struct App {
  accessibility: config::AccessibilityConfig,
  assets: Option<assets::State>,
  auth: auth::State,
  budget_rules: Option<(window::Id, wallet::budget_rules::State)>,
  calendar: Option<calendar::State>,
  calendar_attention: i64,
  calendar_events: WindowStates<calendar::EventWindow>,
  captains_log: Option<captains_log::State>,
  captains_log_reminder_date: Option<chrono::NaiveDate>,
  character_detail: Option<character_detail::State>,
  roster: Option<roster::State>,
  clock_tick: u64,
  coalescer: WriteCoalescer,
  compare: Option<(window::Id, skills_compare::State)>,
  composes: WindowStates<mail::compose::Draft>,
  confirm_force_takeover: bool,
  contact_sync: Option<contact_sync::State>,
  contracts: WindowStates<contract_detail::State>,
  corporation_detail: Option<corporation_detail::State>,
  editors: WindowStates<skill_plan_editor::State>,
  engine_state: EngineState,
  esi_connected: bool,
  holder_watch: HolderWatch,
  industry: Option<industry::State>,
  industry_catalog: Option<industry::StaticCatalog>,
  init_error: Option<String>,
  keyboard_focus: FocusTracker,
  killmails: WindowStates<killmail_detail::State>,
  last_push: Option<SystemTime>,
  last_synced: Option<DateTime<Utc>>,
  mail: Option<mail::State>,
  mail_unread: i64,
  manage_plans: Option<(window::Id, skill_plan_manager::State)>,
  market: Option<market::State>,
  market_outbid: i64,
  mcp_server: Option<mcp::Server>,
  next_roster_reload: Option<Instant>,
  next_trash_purge: Option<Instant>,
  notifications: Vec<store::model::Notification>,
  notification_names: std::collections::HashMap<store::model::NotificationOwner, String>,
  notifications_dirty: bool,
  notifications_history: Vec<store::model::Notification>,
  notifications_history_cursor: Option<store::model::HistoryCursor>,
  notifications_history_epoch: u64,
  notifications_history_has_more: bool,
  notifications_history_loading: bool,
  notifications_history_scroll: f32,
  notifications_panel_open: bool,
  notifications_tab: NotificationTab,
  notifications_unread: i64,
  now: DateTime<Utc>,
  outbox: sync::OutboxStatus,
  pack_open: pack_open::State,
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
  skills_dirty: bool,
  splash: Option<splash::State>,
  splash_step: u32,
  status: sync::SyncStatus,
  stockpile_editors: WindowStates<assets::Editor>,
  stockpile_imports: WindowStates<assets::ImportPanel>,
  store_ready: Option<StoreReady>,
  sync_popover_open: bool,
  sync_session: Option<store::sync_session::SyncSession>,
  sync_tick: bool,
  take_over_requested_at: Option<DateTime<Utc>>,
  telemetry: Option<clients::telemetry::Sender>,
  toasts: Vec<ToastEntry>,
  ui_state: UiState,
  updater: Option<updater::Handle>,
  updater_state: updater::State,
  updater_toast_dismissed: bool,
  wallet: Option<wallet::State>,
  windows: Windows,
  wizard: Option<wizard::State>,
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
  BudgetRules(wallet::budget_rules::Message),
  Calendar(calendar::Message),
  CalendarAttentionCounted(i64),
  CalendarEvent(window::Id, calendar::EventMessage),
  CancelTakeOver,
  CaptainsLog(captains_log::Message),
  CaptainsLogNudgeChecked {
    complete: bool,
    date: chrono::NaiveDate,
  },
  CaptainsLogReminded(Option<Box<store::model::Notification>>),
  CharacterDetail(character_detail::Message),
  Roster(roster::Message),
  ClockTick,
  CloseSyncPopover,
  Compare(skills_compare::Message),
  Compose(window::Id, mail::Message),
  ConfirmTakeOver,
  ContactSync(contact_sync::Message),
  Contract(window::Id, contract_detail::Message),
  CloseNotificationsPanel,
  CorporationDetail(corporation_detail::Message),
  DemotedToSlave(Box<StoreReady>, HolderInfo),
  EngineStopped {
    reason: Option<String>,
  },
  Escape,
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
  LeaseHeartbeatChecked(Option<HolderInfo>),
  LockReleased,
  Mail(mail::Message),
  MailUnreadCounted(i64),
  MainScreenSizeProbed(Option<Size>),
  ManagePlans(skill_plan_manager::Message),
  MarkAllNotificationsRead,
  Market(market::Message),
  Mcp(mcp::McpRequest),
  McpDataChanged,
  Nav(rail::Destination),
  NavTo(rail::Destination, Option<&'static str>),
  NotificationActivated(i64),
  NotificationsHistoryPageLoaded {
    epoch: u64,
    rows: Vec<store::model::Notification>,
    who: std::collections::HashMap<store::model::NotificationOwner, String>,
  },
  NotificationsHistoryScrolled {
    absolute: f32,
    relative: f32,
  },
  NotificationsRefreshed(Box<notifications::Snapshot>),
  PackConfirmed,
  PackDeclined,
  PackFileOpened(std::path::PathBuf),
  PackFileProcessed(Box<pack_open::Prompt>),
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
  SkillPlanEditor(window::Id, skill_plan_editor::Message),
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
  TakeoverPoll,
  TelemetryFlushTick,
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
  Wizard(wizard::Message),
}

impl Message {
  fn affects_images(&self) -> bool {
    match self {
      Message::Assets(msg) => msg.loads_data(),
      Message::Calendar(msg) => msg.loads_data(),
      Message::CaptainsLog(msg) => msg.loads_data(),
      Message::CharacterDetail(msg) => msg.loads_data(),
      Message::ContactSync(msg) => msg.loads_data(),
      Message::Roster(msg) => msg.loads_data(),
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
    self
      .screen_variant_name_primary()
      .or_else(|| self.screen_variant_name_secondary())
  }

  fn screen_variant_name_primary(&self) -> Option<&'static str> {
    Some(match self {
      Message::Assets(_) => "Assets",
      Message::Auth(_) => "Auth",
      Message::BudgetRules(_) => "BudgetRules",
      Message::Calendar(_) => "Calendar",
      Message::CalendarAttentionCounted(_) => "CalendarAttentionCounted",
      Message::CalendarEvent(..) => "CalendarEvent",
      Message::CaptainsLog(_) => "CaptainsLog",
      Message::CaptainsLogNudgeChecked {
        ..
      } => "CaptainsLogNudgeChecked",
      Message::CaptainsLogReminded(_) => "CaptainsLogReminded",
      Message::CharacterDetail(_) => "CharacterDetail",
      Message::Roster(_) => "Roster",
      Message::Compare(_) => "Compare",
      Message::Compose(..) => "Compose",
      Message::ContactSync(_) => "ContactSync",
      Message::Contract(..) => "Contract",
      Message::CorporationDetail(_) => "CorporationDetail",
      _ => return None,
    })
  }

  fn screen_variant_name_secondary(&self) -> Option<&'static str> {
    Some(match self {
      Message::Industry(_) => "Industry",
      Message::Killmail(..) => "Killmail",
      Message::Mail(_) => "Mail",
      Message::MailUnreadCounted(_) => "MailUnreadCounted",
      Message::ManagePlans(_) => "ManagePlans",
      Message::Market(_) => "Market",
      Message::Settings(_) => "Settings",
      Message::SkillPlanEditor(..) => "SkillPlanEditor",
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
      Message::Escape => "Escape",
      Message::FocusMainWindow => "FocusMainWindow",
      Message::ImageReady {
        ..
      } => "ImageReady",
      Message::InitFailed(_) => "InitFailed",
      Message::PackConfirmed => "PackConfirmed",
      Message::PackDeclined => "PackDeclined",
      Message::PackFileOpened(_) => "PackFileOpened",
      Message::PackFileProcessed(_) => "PackFileProcessed",
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
      Message::Wizard(_) => "Wizard",
      _ => return None,
    })
  }

  fn sync_variant_name(&self) -> Option<&'static str> {
    Some(match self {
      Message::CancelTakeOver => "CancelTakeOver",
      Message::CloseSyncPopover => "CloseSyncPopover",
      Message::ConfirmTakeOver => "ConfirmTakeOver",
      Message::DemotedToSlave(..) => "DemotedToSlave",
      Message::EngineStopped {
        ..
      } => "EngineStopped",
      Message::LeaseHeartbeat => "LeaseHeartbeat",
      Message::LeaseHeartbeatChecked(_) => "LeaseHeartbeatChecked",
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
      Message::TakeoverPoll => "TakeoverPoll",
      Message::TelemetryFlushTick => "TelemetryFlushTick",
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
  CaptainsLog,
  CharacterDetail(i64),
  ContactSync,
  CorporationDetail(i64),
  Industry,
  Mail,
  Market,
  #[default]
  Roster,
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
      rail::Destination::Industry => {
        unreachable!("Industry is routed via Message::Nav, not From")
      }
      rail::Destination::Mail => Route::Mail,
      rail::Destination::Market => {
        unreachable!("Market is routed via Message::Nav, not From")
      }
      rail::Destination::Roster => Route::Roster,
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
      Route::Roster
      | Route::CaptainsLog
      | Route::CharacterDetail(_)
      | Route::ContactSync
      | Route::CorporationDetail(_) => rail::Destination::Roster,
      Route::Industry => rail::Destination::Industry,
      Route::Mail => rail::Destination::Mail,
      Route::Market => rail::Destination::Market,
      Route::Settings => rail::Destination::Settings,
      Route::Skills(_) => rail::Destination::Skills,
      Route::Wallet => rail::Destination::Wallet,
    }
  }

  fn name(self) -> &'static str {
    match self {
      Route::Assets => "Assets",
      Route::Calendar => "Calendar",
      Route::CaptainsLog => "roster.captains_log",
      Route::CharacterDetail(_) => "roster.character_detail",
      Route::ContactSync => "roster.contact_sync",
      Route::CorporationDetail(_) => "roster.corporation_detail",
      Route::Industry => "Industry",
      Route::Mail => "Mail",
      Route::Market => "Market",
      Route::Roster => "Roster",
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

impl From<store::share_meta::TakeoverRequest> for HolderInfo {
  fn from(request: store::share_meta::TakeoverRequest) -> Self {
    HolderInfo {
      hostname: request.hostname,
      last_active: request.requested_at,
      machine_id: request.machine_id,
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
  // usage view_open (§8.1): only a real route change is counted (after the
  // re-selection guard), keyed by the parameter-free `Route::name()` token so
  // id-carrying variants never leak an id. A no-op unless telemetry is built.
  telemetry::collector::record_view_open(telemetry::collector::route_token(to.name()));
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

fn relocate_default_paths() -> bool {
  let marker = migration::legacy_sde_version_marker();
  let db_present = migration::legacy_default_db_present();
  let registry = migration::Registry::resolve(marker.as_deref(), db_present);
  if registry.is_empty() {
    return false;
  }
  let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else {
    return false;
  };
  let _ = runtime.block_on(registry.before_startup());
  true
}

pub fn run() -> iced::Result {
  let relocated = relocate_default_paths();

  let settings = config::load().ok();
  let (log_dir, log_level) = settings
    .as_ref()
    .map(|settings| (settings.storage().resolved_log_dir(), *settings.storage().log_level()))
    .unwrap_or_else(|| (config::log_dir(), config::LogLevel::default()));

  let _log_guard = init_tracing(&log_dir, log_level);

  if relocated {
    tracing::info!(
      target: "pod::migration",
      "relocated default data directories to dev.aaronmallen.pod",
    );
  }

  // Crash pipeline (spec mmmzstpq §8.4): set the process-global attribution
  // statics + the context_log ring BEFORE installing the panic hook, so a panic
  // in early boot still produces a correctly-attributed, fully-scrubbed record.
  // The ring layer added inside `init_tracing` above is inert until this runs.
  let (machine_id, telemetry_config) = settings
    .as_ref()
    .map(|settings| {
      (
        settings.storage().machine_id().clone().unwrap_or_default(),
        *settings.telemetry(),
      )
    })
    .unwrap_or_default();
  telemetry::crash::install(&log_dir, &machine_id, telemetry_config);

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

pub fn restart() {
  auth::release_lock();
  if let Ok(exe) = std::env::current_exe()
    && let Err(error) = std::process::Command::new(exe).spawn()
  {
    tracing::error!(target: "pod::lifecycle", %error, "failed to relaunch on restart");
  }
  std::process::exit(0);
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

fn drain_roster_dirty_at(app: &mut App, now: Instant) -> Option<Task<Message>> {
  if !app.roster_dirty || app.roster.is_none() {
    return None;
  }
  if app.next_roster_reload.is_some_and(|floor| now < floor) {
    return None;
  }
  app.roster_dirty = false;
  app.next_roster_reload = Some(now + ROSTER_RELOAD_DEBOUNCE);
  let runtime = app.runtime.as_ref()?;
  Some(roster::load(&runtime.db, feature_flags(app)).map(Message::Roster))
}

fn drain_wallet_dirty(app: &mut App) -> Option<Task<Message>> {
  let db = app.runtime.as_ref()?.db.clone();
  Some(app.wallet.as_mut()?.drain_dirty(&db)?.map(Message::Wallet))
}

fn drain_skills_dirty(app: &mut App) -> Option<Task<Message>> {
  if !app.skills_dirty || app.skills.is_none() {
    return None;
  }
  let db = app.runtime.as_ref()?.db.clone();
  app.skills_dirty = false;
  let active = app.skills.as_ref()?.active();
  let owned = owned_pilot_ids(app);
  Some(skills::load(&db, active, owned).map(Message::Skills))
}

fn main_view(app: &App) -> Element<'_, Message> {
  let inner: Element<'_, Message> = if let Some(error) = &app.init_error {
    placeholder(t!("shell.window.init_error", error => error).into_owned())
  } else if app.runtime.is_none() {
    placeholder(t!("shell.status.starting_up").into_owned())
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
    feature_flags: feature_flags(app),
    hovered: app.rail_hover,
    mail_unread,
    market_outbid: app.market_outbid,
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
  let sub_rail_element = (cascade_mode == config::CascadeMode::SubRail)
    .then(|| {
      rail::sub_rail(
        app.route.destination(),
        active_sub_section(app),
        feature_flags(app),
        nav_location,
        |dest, id| Message::NavTo(dest, Some(id)),
      )
    })
    .flatten();
  let body = main_body_row(content.into(), rail_element, sub_rail_element, nav_location);

  let mut column_children: Vec<Element<'_, Message>> = Vec::with_capacity(4);
  if let Some(banner) = updater_banner::banner(&app.updater_state, Message::UpdaterAction) {
    column_children.push(banner);
  }
  if let Some(holder) = &app.read_only {
    column_children.push(read_only_banner(
      holder,
      app.confirm_force_takeover,
      app.take_over_requested_at.is_some(),
      app.now,
    ));
  }
  if app.sde_stale {
    column_children.push(sde_stale_banner());
  }
  column_children.push(body);
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

  let layers = main_overlay_layers(app, base, nav_location, toast);

  Stack::with_children(layers)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn main_body_row<'a>(
  content: Element<'a, Message>,
  rail_element: Element<'a, Message>,
  sub_rail_element: Option<Element<'a, Message>>,
  nav_location: config::NavLocation,
) -> Element<'a, Message> {
  let mut body_children: Vec<Element<'a, Message>> = Vec::with_capacity(3);
  match nav_location {
    config::NavLocation::Left => {
      body_children.push(rail_element);
      if let Some(sub_rail) = sub_rail_element {
        body_children.push(sub_rail);
      }
      body_children.push(content);
    }
    config::NavLocation::Right => {
      body_children.push(content);
      if let Some(sub_rail) = sub_rail_element {
        body_children.push(sub_rail);
      }
      body_children.push(rail_element);
    }
  }
  Row::with_children(body_children)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn main_overlay_layers<'a>(
  app: &'a App,
  base: Element<'a, Message>,
  nav_location: config::NavLocation,
  toast: Option<Element<'a, Message>>,
) -> Vec<Element<'a, Message>> {
  let mut layers: Vec<Element<'a, Message>> = vec![base];
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
  if let Some(prompt) = app.pack_open.prompt() {
    layers.extend(modal_layers(Message::PackDeclined, pack_open::overlay(prompt)));
  }
  layers
}

const NOTIFICATIONS_PANEL_WIDTH: f32 = 384.0;

const NOTIFICATIONS_PANEL_MAX_HEIGHT: f32 = 560.0;

const NOTIFICATIONS_TAB_STRIP_HEIGHT: f32 = 40.0;

const NOTIFICATIONS_HISTORY_ROW_HEIGHT: f32 = 64.0;

const NOTIFICATIONS_HISTORY_SCROLL_THRESHOLD: f32 = 0.85;

fn sde_stale_banner<'a>() -> Element<'a, Message> {
  let label = text(t!("shell.sde.stale_banner").into_owned())
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

fn roster(app: &App) -> Vec<OwnedPilot> {
  app.roster.as_ref().map(roster::owned_roster).unwrap_or_default()
}

fn resolve_skills_target(roster: &[OwnedPilot], last_selected: Option<i64>) -> Option<i64> {
  if let Some(id) = last_selected
    && roster.iter().any(|pilot| pilot.id == id)
  {
    return Some(id);
  }
  roster.first().map(|pilot| pilot.id)
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
      primary: color::accent(),
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

fn is_escape_pressed(event: &iced::Event) -> bool {
  matches!(
    event,
    iced::Event::Keyboard(keyboard::Event::KeyPressed {
      key: keyboard::Key::Named(keyboard::key::Named::Escape),
      ..
    })
  )
}

/// Resolves an Escape press to the message that dismisses the topmost overlay.
///
/// Checked in z-order: the pack-open prompt, then the command palette (returns
/// `None` here — it owns Escape via its own always-on `palette_key_subscription`
/// rather than being dismissed by this path), then the notifications panel, the
/// sync popover, and finally the active feature's own dialogs.
fn topmost_dismiss(app: &App) -> Option<Message> {
  if app.pack_open.prompt().is_some() {
    return Some(Message::PackDeclined);
  }

  if app.palette.is_some() {
    return None;
  }

  if app.notifications_panel_open {
    return Some(Message::CloseNotificationsPanel);
  }

  if app.sync_popover_open {
    return Some(Message::CloseSyncPopover);
  }

  active_feature_dismiss(app)
}

fn handle_escape(app: &mut App) -> Task<Message> {
  match topmost_dismiss(app) {
    Some(message) => update(app, message),
    None => Task::none(),
  }
}

fn active_feature_dismiss(app: &App) -> Option<Message> {
  match app.route {
    Route::Assets => app
      .assets
      .as_ref()
      .and_then(assets::escape_dismiss)
      .map(Message::Assets),
    Route::CaptainsLog => app
      .captains_log
      .as_ref()
      .and_then(captains_log::escape_dismiss)
      .map(Message::CaptainsLog),
    Route::CharacterDetail(_) => app
      .character_detail
      .as_ref()
      .and_then(character_detail::escape_dismiss)
      .map(Message::CharacterDetail),
    Route::ContactSync => app
      .contact_sync
      .as_ref()
      .and_then(contact_sync::escape_dismiss)
      .map(Message::ContactSync),
    Route::Mail => app.mail.as_ref().and_then(mail::escape_dismiss).map(Message::Mail),
    Route::Roster => app
      .roster
      .as_ref()
      .and_then(roster::escape_dismiss)
      .map(Message::Roster),
    Route::Settings => app
      .settings
      .as_ref()
      .and_then(settings::escape_dismiss)
      .map(Message::Settings),
    Route::Wallet => app
      .wallet
      .as_ref()
      .and_then(wallet::escape_dismiss)
      .map(Message::Wallet),
    _ => None,
  }
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
    if app.take_over_requested_at.is_some() {
      subs.push(iced::time::every(REQUEST_POLL_INTERVAL).map(|_| Message::TakeoverPoll));
    }
  }
  if topmost_dismiss(app).is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      is_escape_pressed(&event).then_some(Message::Escape)
    }));
  }
  if !app.toasts.is_empty() {
    subs.push(iced::time::every(TOAST_TICK).map(|_| Message::ToastTick));
  }
  if app.telemetry.is_some() {
    subs.push(iced::time::every(TELEMETRY_FLUSH_INTERVAL).map(|_| Message::TelemetryFlushTick));
  }
  subs.push(auth::subscription().map(Message::Auth));
  subs.push(auth::file_subscription().map(Message::PackFileOpened));
  subs.push(auth::focus_subscription().map(|()| Message::FocusMainWindow));
  subs.push(mcp::bridge::subscription().map(Message::Mcp));
  subs.push(mcp::reload::subscription().map(|_| Message::McpDataChanged));
  subs.push(shortcuts::subscription(Message::Shortcut));
  subs.push(palette_key_subscription(app));
  subs.extend(data_subscriptions(app));
  Subscription::batch(subs)
}

fn data_subscriptions(app: &App) -> Vec<Subscription<Message>> {
  let mut subs = Vec::new();
  if let Some(state) = &app.assets {
    subs.push(assets::subscription(state).map(Message::Assets));
  }
  if let Some(state) = &app.captains_log {
    subs.push(captains_log::subscription(state).map(Message::CaptainsLog));
  }
  if let Some(state) = &app.roster {
    subs.push(roster::subscription(state).map(Message::Roster));
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
  if let Some(state) = &app.market {
    subs.push(market::subscription(state).map(Message::Market));
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
  if let Some((_, state)) = &app.budget_rules {
    subs.push(wallet::budget_rules::subscription(state).map(Message::BudgetRules));
  }
  for (id, editor) in app.editors.iter() {
    subs.push(
      skill_plan_editor::subscription(editor)
        .with(id)
        .map(|(id, msg)| Message::SkillPlanEditor(id, msg)),
    );
  }
  if let Some((_, state)) = &app.manage_plans {
    subs.push(skill_plan_manager::subscription(state).map(Message::ManagePlans));
  }
  subs
}

fn theme(app: &App, id: window::Id) -> iced::Theme {
  match app.windows.kind(id) {
    Some(Window::Killmail | Window::Splash) => splash_theme(),
    _ => pod_theme(),
  }
}

fn title(app: &App, id: window::Id) -> String {
  window_title(app, id)
}

fn window_title(app: &App, id: window::Id) -> String {
  match app.windows.kind(id) {
    Some(Window::BudgetRules) => t!("shell.window.budget_rules").into_owned(),
    Some(Window::CalendarEvent) => app
      .calendar_events
      .get(id)
      .map(|window| t!("shell.window.titled", title => window.title()).into_owned())
      .unwrap_or_else(|| t!("shell.window.event").into_owned()),
    Some(Window::Compare) => t!("shell.window.compare_skills").into_owned(),
    Some(Window::FirstRun) => t!("wizard.window.title").into_owned(),
    Some(Window::Contract) => match app.contracts.get(id) {
      Some(state) => t!("shell.window.titled", title => state.title()).into_owned(),
      None => t!("shell.window.contract").into_owned(),
    },
    Some(Window::Killmail) => app
      .killmails
      .get(id)
      .map(|state| t!("shell.window.titled", title => state.title()).into_owned())
      .unwrap_or_else(|| t!("shell.window.killmail").into_owned()),
    Some(Window::MailCompose) => app
      .composes
      .get(id)
      .map(|draft| t!("shell.window.titled", title => mail::compose::window_title(draft)).into_owned())
      .unwrap_or_else(|| t!("shell.window.compose_mail").into_owned()),
    Some(Window::Main) => t!("shell.window.app").into_owned(),
    Some(Window::ManagePlans) => t!("shell.window.manage_skill_plans").into_owned(),
    Some(Window::SkillPlanEditor) => t!("shell.window.skill_plan_editor").into_owned(),
    Some(Window::Splash) => t!("shell.window.app").into_owned(),
    Some(Window::StockpileEditor) => app
      .stockpile_editors
      .get(id)
      .map(|editor| t!("shell.window.titled", title => assets::stockpile_editor_window_title(editor)).into_owned())
      .unwrap_or_else(|| t!("shell.window.stockpile_editor").into_owned()),
    Some(Window::StockpileImport) => {
      t!("shell.window.titled", title => assets::stockpile_import_window_title()).into_owned()
    }
    None => t!("shell.window.app").into_owned(),
  }
}

fn update(app: &mut App, message: Message) -> Task<Message> {
  let span = tracing::trace_span!(target: "pod::ui", "update", message = message.variant_name());
  let _entered = span.enter();
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

fn handle_assets(app: &mut App, msg: assets::Message) -> Task<Message> {
  if let assets::Message::PaneSettled(key, ratio) = msg {
    record_pane_ratio(app, key, ratio);
    return Task::none();
  }
  if let assets::Message::UiFlagPersisted(key, value) = msg {
    record_ui_flag(app, key, value);
    return Task::none();
  }
  if let assets::Message::ReauthRequested(id) = msg {
    return update(app, Message::ReauthCharacter(id));
  }
  match msg {
    assets::Message::StockpileNew => {
      return open_stockpile_editor_window(app, assets::EditorSeed::Blank);
    }
    assets::Message::StockpileEditStarted(id) => {
      let Some(card) = app.assets.as_ref().and_then(|state| state.stockpile_card(id).cloned()) else {
        return Task::none();
      };
      if let Some(state) = app.assets.as_mut() {
        state.dismiss_stockpile_context_menu();
      }
      return open_stockpile_editor_window(app, assets::EditorSeed::FromCard(Box::new(card)));
    }
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

/// The well-known EVE Inbox system label id — kept in sync with `features::mail::labels` (which is a
/// private module and so cannot be imported here). Waking a snooze restores this membership.
const INBOX_LABEL_ID: i64 = 1;

/// The name of the user label that mirrors Pod's snooze state into EVE. Resolved from the catalog
/// by this name; created on demand at snooze time by the mail feature.
const SNOOZED_LABEL_NAME: &str = "Snoozed";

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
  if let Some(reload) = drain_skills_dirty(app) {
    tasks.push(reload);
  }
  if let Some(detect) = drain_notifications_dirty(app) {
    tasks.push(detect);
  }
  Task::batch(tasks)
}

fn owned_character_ids(app: &App) -> Vec<i64> {
  app
    .roster
    .as_ref()
    .map(roster::owned_roster)
    .unwrap_or_default()
    .iter()
    .map(|pilot| pilot.id)
    .collect()
}

fn owned_corporation_ids(app: &App) -> Vec<i64> {
  app
    .roster
    .as_ref()
    .map(roster::owned_corporations)
    .unwrap_or_default()
    .into_iter()
    .map(|(id, _)| id)
    .collect()
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
    app.updater_state = state.clone();
  }
  if app.splash.is_some() {
    return drive_splash_preflight(app, &state);
  }
  Task::none()
}

fn drive_splash_preflight(app: &mut App, state: &updater::State) -> Task<Message> {
  let phase = match app.splash.as_ref() {
    Some(splash) => &splash.phase,
    None => return Task::none(),
  };

  match state {
    updater::State::UpdateAvailable {
      version,
    } if *phase == splash::Phase::CheckingUpdate => match app.splash.as_mut() {
      Some(splash) => splash::update(splash, splash::Message::UpdateAvailable(version.clone())).map(Message::Splash),
      None => Task::none(),
    },
    updater::State::Downloading {
      ..
    } if *phase == splash::Phase::Updating => match app.splash.as_mut() {
      Some(splash) => splash::update(splash, splash::Message::DownloadProgress(0.5)).map(Message::Splash),
      None => Task::none(),
    },
    updater::State::ReadyToRestart {
      ..
    } if *phase == splash::Phase::Updating => {
      let progress = match app.splash.as_mut() {
        Some(splash) => splash::update(splash, splash::Message::DownloadProgress(1.0)).map(Message::Splash),
        None => Task::none(),
      };
      Task::batch([progress, handle_updater_action(app, updater_banner::Action::Restart)])
    }
    updater::State::Error {
      ..
    } if matches!(*phase, splash::Phase::CheckingUpdate | splash::Phase::Updating) => begin_boot(app),
    _ => Task::none(),
  }
}

fn handle_wallet(app: &mut App, msg: wallet::Message) -> Task<Message> {
  if let wallet::Message::ContractSelected(contract_id) = msg {
    let Some(source) = app.wallet.as_ref().and_then(|state| state.contract_source(contract_id)) else {
      return Task::none();
    };
    return open_contract_window(app, source, contract_id);
  }
  // Doesn't return: the deletion itself still falls through to wallet::update via the catch-all arm below.
  if let wallet::Message::BudgetRuleDeleted(rule_id) = msg
    && let Some((_, state)) = app.budget_rules.as_mut()
  {
    state.clear_editor_for_rule(rule_id);
  }
  match msg {
    wallet::Message::BudgetGlobalRulesOpened => open_budget_rules_window(app),
    wallet::Message::BudgetRuleEditOpened(rule_id) => {
      open_budget_rules_editor(app, wallet::budget_rules::EditorSeed::Existing(rule_id))
    }
    wallet::Message::BudgetRuleNewOpened(category_id) => {
      open_budget_rules_editor(app, wallet::budget_rules::EditorSeed::New(category_id))
    }
    wallet::Message::BudgetRulesWindow(msg) => handle_budget_rules(app, msg),
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
      if app.roster.is_some() {
        tasks.push(Task::done(Message::Roster(roster::Message::SignedIn {
          character_id: signed.character_id,
          name: signed.character_name,
        })));
      }
      Some(sync::Subject::Character(signed.character_id))
    }
    None => None,
  };
  if let Some(subject) = enrolled {
    runtime.sync.enroll(subject);
    runtime.sync.run_now(subject);
    runtime.sync.discover();
    if app.roster.is_some() {
      tasks.push(roster::load(&runtime.db, feature_flags(app)).map(Message::Roster));
    }
  }
  Task::batch(tasks)
}

fn handle_roster(app: &mut App, msg: roster::Message) -> Task<Message> {
  match msg {
    roster::Message::AddCharacterRequested => update(app, Message::Auth(auth::Message::Start(feature_flags(app)))),
    roster::Message::AddCorporationRequested => update(
      app,
      Message::Auth(auth::Message::StartAddCorporation(feature_flags(app))),
    ),
    roster::Message::CharacterSelected(id) => navigate_to_character_detail(app, id),
    roster::Message::CharacterSectionSelected {
      character_id,
      tab,
    } => open_character_detail_section(app, character_id, tab),
    roster::Message::CorporationSelected(id) => navigate_to_corporation_detail(app, id),
    roster::Message::UtilityActivated(utility) => {
      if let (Some(state), Some(runtime)) = (app.roster.as_mut(), app.runtime.as_ref()) {
        let _ = roster::update(state, roster::Message::UtilityActivated(utility), &runtime.db);
      }
      match utility {
        roster::Utility::CaptainsLog => navigate_to_captains_log(app),
        roster::Utility::ContactSync => navigate_to_contact_sync(app),
      }
    }
    roster::Message::TrainingSkillClicked(character_id) => {
      let owned = owned_pilot_ids(app);
      navigate_to_skills(app, Some(character_id), owned)
    }
    roster::Message::ViewModePersisted(key, values) => {
      record_ui_list(app, key, values);
      Task::none()
    }
    roster::Message::CaptainsLogNudgeDismissed => {
      persist_captains_log_nudge_dismissal(app);
      Task::none()
    }
    roster::Message::CaptainsLogNudgeOpened => {
      persist_captains_log_nudge_dismissal(app);
      navigate_to_captains_log(app)
    }
    roster::Message::ReauthCharacterRequested(character_id) => update(app, Message::ReauthCharacter(character_id)),
    roster::Message::ReauthCorporationRequested(corporation_id) => reauth_corporation(app, corporation_id),
    roster::Message::RemoveCharacterConfirmed(id) => remove_subject_then_update(
      app,
      sync::Subject::Character(id),
      roster::Message::RemoveCharacterConfirmed(id),
    ),
    roster::Message::RemoveCorporationConfirmed(id) => remove_subject_then_update(
      app,
      sync::Subject::Corporation(id),
      roster::Message::RemoveCorporationConfirmed(id),
    ),
    msg => match (app.roster.as_mut(), app.runtime.as_ref()) {
      (Some(state), Some(runtime)) => roster::update(state, msg, &runtime.db).map(Message::Roster),
      _ => Task::none(),
    },
  }
}

fn open_character_detail_section(app: &mut App, character_id: i64, tab: character_detail::Tab) -> Task<Message> {
  // Let the roster dismiss its context menu before this pane routes away, so the menu
  // is not still up when the user navigates back.
  if let (Some(state), Some(runtime)) = (app.roster.as_mut(), app.runtime.as_ref()) {
    let _ = roster::update(
      state,
      roster::Message::CharacterSectionSelected {
        character_id,
        tab,
      },
      &runtime.db,
    );
  }
  navigate_to_character_detail_section(app, character_id, tab)
}

fn remove_subject_then_update(app: &mut App, subject: sync::Subject, msg: roster::Message) -> Task<Message> {
  match (app.roster.as_mut(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => {
      runtime.sync.withdraw(subject);
      roster::update(state, msg, &runtime.db).map(Message::Roster)
    }
    _ => Task::none(),
  }
}

fn handle_calendar(app: &mut App, msg: calendar::Message) -> Task<Message> {
  if let calendar::Message::ReauthRequested(id) = msg {
    return update(app, Message::ReauthCharacter(id));
  }

  if let calendar::Message::EventOpened(character_id, event_id) = msg {
    return open_calendar_event_window(app, character_id, event_id);
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

fn handle_captains_log(app: &mut App, msg: captains_log::Message) -> Task<Message> {
  if matches!(msg, captains_log::Message::Exit) {
    return handle_nav(app, rail::Destination::Roster);
  }
  if let captains_log::Message::PaneSettled(key, ratio) = msg {
    record_pane_ratio(app, key, ratio);
    return Task::none();
  }
  match (app.captains_log.as_mut(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => captains_log::update(state, msg, &runtime.db).map(Message::CaptainsLog),
    _ => Task::none(),
  }
}

fn handle_contact_sync(app: &mut App, msg: contact_sync::Message) -> Task<Message> {
  if matches!(msg, contact_sync::Message::Exit) {
    return handle_nav(app, rail::Destination::Roster);
  }
  if let contact_sync::Message::Contacts(detail) = &msg
    && let character_detail::Message::ContactEntityInput(query) = detail.as_ref()
  {
    let query = query.clone();
    return match (app.contact_sync.as_mut(), app.runtime.as_ref()) {
      (Some(state), Some(runtime)) => {
        let update = contact_sync::update(state, msg, &runtime.db).map(Message::ContactSync);
        Task::batch([update, contact_sync_entity_search(state, runtime, query)])
      }
      _ => Task::none(),
    };
  }
  match (app.contact_sync.as_mut(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => contact_sync::update(state, msg, &runtime.db).map(Message::ContactSync),
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

fn contact_sync_entity_search(state: &contact_sync::State, runtime: &Runtime, query: String) -> Task<Message> {
  use crate::features::roster::entity_search;

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
      Message::ContactSync(contact_sync::Message::Contacts(Box::new(
        character_detail::Message::ContactEntityResults {
          generation,
          results,
        },
      )))
    },
  )
}

fn contact_entity_search(state: &character_detail::State, runtime: &Runtime, query: String) -> Task<Message> {
  use crate::features::roster::entity_search;

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
  let Some(manager) = app.roster.as_ref() else {
    return Vec::new();
  };

  let by_sp: Vec<(i64, i64)> = roster::groups(manager)
    .iter()
    .flat_map(|group| group.cards.iter())
    .chain(roster::unassigned(manager).iter())
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
    .roster
    .as_ref()
    .map(roster::owned_roster)
    .unwrap_or_default()
    .iter()
    .map(|pilot| pilot.id)
    .collect()
}

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
  if due.notifications {
    app.notifications_dirty = true;
    tasks.push(captains_log_reminder_tick(app));
  }
  Task::batch(tasks)
}

fn captains_log_reminder_tick(app: &mut App) -> Task<Message> {
  let today = app.now.date_naive();
  if app.captains_log_reminder_date == Some(today) {
    return Task::none();
  }
  let character_ids = owned_character_ids(app);
  if character_ids.is_empty() {
    return Task::none();
  }
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  app.captains_log_reminder_date = Some(today);
  let db = runtime.db.clone();
  let reminder = Task::perform(
    emit_captains_log_reminder(db.clone(), today.to_string(), character_ids.clone()),
    |emitted| Message::CaptainsLogReminded(emitted.map(Box::new)),
  );
  let nudge = Task::perform(
    roster::captains_log_nudge::evaluate(db, today.to_string(), character_ids),
    move |complete| Message::CaptainsLogNudgeChecked {
      complete,
      date: today,
    },
  );
  Task::batch([reminder, nudge])
}

fn handle_captains_log_nudge_checked(app: &mut App, date: chrono::NaiveDate, complete: bool) -> Task<Message> {
  if let Some(state) = app.roster.as_mut() {
    state.evaluate_captains_log_nudge(date, complete);
  }
  Task::none()
}

fn persist_captains_log_nudge_dismissal(app: &mut App) {
  if let Some(date) = app.roster.as_mut().and_then(roster::State::dismiss_captains_log_nudge) {
    record_ui_list(
      app,
      roster::CAPTAINS_LOG_NUDGE_DISMISSED_KEY.to_owned(),
      vec![date.to_string()],
    );
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
    skills::Message::OpenManagePlans => {
      if let Some(state) = app.skills.as_mut() {
        state.close_plan_menu();
      }
      open_manage_plans_window(app)
    }
    skills::Message::OpenPlanEditor(seed) => match &seed {
      skill_plan_editor::Seed::NewTemplate => {
        if let Some(state) = app.skills.as_mut() {
          state.close_plan_menu();
        }
        open_editor_window(app, None, seed).1
      }
      _ => match app.skills.as_ref().map(skills::State::active) {
        Some(id) => open_editor_window(app, Some(id), seed).1,
        None => Task::none(),
      },
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
    _ => industry::update(state, msg, &runtime.db, app.now).map(Message::Industry),
  };

  if app.industry_catalog.is_none()
    && let Some(catalog) = app.industry.as_ref().and_then(industry::State::planner_catalog)
  {
    app.industry_catalog = Some(catalog.clone());
  }

  task
}

fn handle_market(app: &mut App, msg: market::Message) -> Task<Message> {
  if let market::Message::PaneSettled(key, ratio) = msg {
    record_pane_ratio(app, key, ratio);
    return Task::none();
  }
  let (Some(state), Some(runtime)) = (app.market.as_mut(), app.runtime.as_ref()) else {
    return Task::none();
  };
  // Opening the in-game market window needs the character's authed grant, so it is threaded with the
  // ESI/SSO clients here rather than through the db-only reducer.
  if let market::Message::OpenInGame {
    character_id,
    type_id,
  } = msg
  {
    return market::open_market_window_task(
      &runtime.db,
      Arc::clone(&runtime.esi),
      Arc::clone(&runtime.sso),
      character_id,
      type_id,
    )
    .map(Message::Market);
  }
  // A structure order book needs an authed grant, so thread the ESI/SSO clients for a structure
  // pick (or an item change while a structure market is active) alongside the db-only reducer.
  if let Some((structure_id, type_id)) = market::structure_book_fetch(state, &msg) {
    let reduce = market::dispatch(state, msg, &runtime.db).map(Message::Market);
    let fetch = market::fetch_structure_book_task(
      &runtime.db,
      Arc::clone(&runtime.esi),
      Arc::clone(&runtime.sso),
      structure_id,
      type_id,
    )
    .map(Message::Market);
    return Task::batch([reduce, fetch]);
  }
  let was_book_loaded = matches!(&msg, market::Message::BookLoaded(_));
  let reduce = market::dispatch(state, msg, &runtime.db).map(Message::Market);
  match market_structure_resolution(runtime, state, was_book_loaded) {
    Some(resolve) => Task::batch([reduce, resolve]),
    None => reduce,
  }
}

// A freshly loaded region book may quote player structures that aren't in the static SDE; resolve and
// cache their names with an authed lookup, then re-label the book.
fn market_structure_resolution(
  runtime: &Runtime,
  state: &market::State,
  was_book_loaded: bool,
) -> Option<Task<Message>> {
  if !was_book_loaded || market::book_structure_ids(state).is_empty() {
    return None;
  }
  let book = state.book()?.clone();
  Some(
    market::resolve_book_structures_task(
      &runtime.db,
      Arc::clone(&runtime.esi),
      Arc::clone(&runtime.eve_image),
      Arc::clone(&runtime.sso),
      book,
    )
    .map(Message::Market),
  )
}

fn handle_mail(app: &mut App, msg: mail::Message) -> Task<Message> {
  if let mail::Message::PaneSettled(key, ratio) = msg {
    record_pane_ratio(app, key, ratio);
    return Task::none();
  }
  if let mail::Message::ReauthRequested(id) = msg {
    return update(app, Message::ReauthCharacter(id));
  }
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
    mail::Message::Reply(mail_id) => {
      return open_reply_window(app, mail_id, mail::compose::Kind::Reply);
    }
    mail::Message::ReplyAll(mail_id) => {
      return open_reply_window(app, mail_id, mail::compose::Kind::ReplyAll);
    }
    mail::Message::Forward(mail_id) => {
      return open_reply_window(app, mail_id, mail::compose::Kind::Forward);
    }
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
  result: crate::features::roster::entity_search::EntityResult,
) -> crate::ui::components::entity_search::EntityRef {
  use crate::{features::roster::entity_search::EntityCategory, ui::components::entity_search::EntityKind};
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

fn handle_skill_plan_editor(app: &mut App, id: window::Id, msg: skill_plan_editor::Message) -> Task<Message> {
  match msg {
    skill_plan_editor::Message::CloseRequested => close_editor_window(app, id),
    skill_plan_editor::Message::PaneSettled(key, ratio) => {
      record_pane_ratio(app, key, ratio);
      Task::none()
    }
    msg => {
      let manager_reload = if matches!(msg, skill_plan_editor::Message::Saved(Ok(_))) {
        reload_manage_plans_roster(app)
      } else {
        Task::none()
      };
      let editor_task = match (app.editors.get_mut(id), app.runtime.as_ref()) {
        (Some(editor), Some(runtime)) => {
          skill_plan_editor::update(editor, msg, &runtime.db).map(move |msg| Message::SkillPlanEditor(id, msg))
        }
        _ => Task::none(),
      };
      Task::batch([editor_task, manager_reload])
    }
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
  mark_skills_dirty(app, key);
  Task::none()
}

fn mark_skills_dirty(app: &mut App, key: JobKey) {
  if matches!(app.route, Route::Skills(_)) && key.kind == JobKind::CharacterSkills {
    app.skills_dirty = true;
  }
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
  match message {
    splash::Message::DragWindow => match app.windows.id_for(Window::Splash) {
      Some(id) => window::drag(id),
      None => Task::none(),
    },
    splash::Message::ExpandComplete => transition_to_main(app),
    splash::Message::Retry => retry_seed(app),
    splash::Message::UpdateNotAvailable | splash::Message::UpdateFailed(_) => begin_boot(app),
    splash::Message::Later => begin_boot(app),
    splash::Message::Update => {
      let advance = match app.splash.as_mut() {
        Some(state) => splash::update(state, splash::Message::Update).map(Message::Splash),
        None => Task::none(),
      };
      Task::batch([advance, handle_updater_action(app, updater_banner::Action::Apply)])
    }
    other => match app.splash.as_mut() {
      Some(state) => splash::update(state, other).map(Message::Splash),
      None => Task::none(),
    },
  }
}

fn update_wizard(app: &mut App, message: wizard::Message) -> Task<Message> {
  if matches!(message, wizard::Message::Complete) {
    if let Some(state) = app.wizard.as_ref() {
      complete_wizard(state.settings());
    }
    restart();
    return Task::none();
  }

  let relocalize = matches!(message, wizard::Message::SelectLanguage(_));
  if let Some(state) = app.wizard.as_mut() {
    wizard::update(state, message);
    if relocalize {
      i18n::set_locale(state.pending_language());
    }
  }
  Task::none()
}

fn complete_wizard(settings: &config::Settings) {
  let storage = settings.storage();
  for dir in [
    storage.resolved_db_dir(),
    storage.resolved_log_dir(),
    storage.resolved_cache_dir(),
  ] {
    if let Err(error) = std::fs::create_dir_all(&dir) {
      tracing::warn!(target: "pod::lifecycle", %error, dir = %dir.display(), "failed to create storage directory on wizard finish");
    }
  }
  config::save(settings);
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
    Some(Window::FirstRun) => first_run_window_view(app),
    Some(Window::Main) => main_view(app),
    Some(Window::BudgetRules) => budget_rules_window_view(app, id),
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

#[cfg(test)]
mod test_support {
  use super::*;

  pub(super) fn pilot(id: i64) -> OwnedPilot {
    OwnedPilot {
      color: iced::Color::WHITE,
      granted_scopes: None,
      id,
      name: format!("Pilot {id}"),
    }
  }

  pub(super) fn only(feature: config::Feature) -> config::FeatureFlags {
    let mut flags = config::FeatureFlags::default();
    for candidate in config::Feature::ALL {
      flags.set_enabled(candidate, candidate == feature);
    }
    flags
  }

  pub(super) fn test_app() -> App {
    App {
      accessibility: config::AccessibilityConfig::default(),
      assets: None,
      auth: auth::State::default(),
      budget_rules: None,
      calendar: None,
      calendar_attention: 0,
      calendar_events: WindowStates::default(),
      captains_log: None,
      captains_log_reminder_date: None,
      character_detail: None,
      roster: None,
      clock_tick: 0,
      coalescer: WriteCoalescer::new(),
      compare: None,
      composes: WindowStates::default(),
      confirm_force_takeover: false,
      contact_sync: None,
      contracts: WindowStates::default(),
      corporation_detail: None,
      editors: WindowStates::default(),
      engine_state: EngineState::default(),
      esi_connected: true,
      holder_watch: HolderWatch::default(),
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
      market: None,
      market_outbid: 0,
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
      pack_open: pack_open::State::default(),
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
      skills_dirty: false,
      splash: None,
      splash_step: 0,
      stockpile_editors: WindowStates::default(),
      stockpile_imports: WindowStates::default(),
      store_ready: None,
      status: sync::SyncStatus::new(),
      sync_popover_open: false,
      sync_session: None,
      sync_tick: false,
      take_over_requested_at: None,
      telemetry: None,
      toasts: Vec::new(),
      ui_state: UiState::default(),
      updater: None,
      updater_state: updater::State::default(),
      updater_toast_dismissed: false,
      wallet: None,
      windows: Windows::default(),
      wizard: None,
    }
  }

  pub(super) async fn test_runtime() -> Runtime {
    let (runtime, _rx) = test_runtime_with_commands().await;
    runtime
  }

  pub(super) async fn test_runtime_with_commands() -> (Runtime, tokio::sync::mpsc::UnboundedReceiver<sync::Command>) {
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

  pub(super) async fn test_runtime_with_restart() -> (Runtime, tokio::sync::mpsc::UnboundedReceiver<()>) {
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

  pub(super) fn temp_sync_session() -> (tempfile::TempDir, store::sync_session::SyncSession) {
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

  pub(super) fn featured_app() -> App {
    let mut app = test_app();
    app.assets = Some(assets::State::new(config::FeatureFlags::default()));
    app.calendar = Some(calendar::State::new(42, app.now, config::FeatureFlags::default()));
    app.character_detail = Some(character_detail::State::new(1, &[]));
    app.roster = Some(roster::State::new());
    app.mail = Some(mail::State::new(42));
    app.skills = Some(skills::State::new(1));
    app.wallet = Some(wallet::State::new(config::FeatureFlags::default()));
    app
  }

  pub(super) fn ready_app() -> App {
    let mut app = test_app();
    app.roster = Some(roster::State::new());
    app.character_detail = Some(character_detail::State::new(1, &[]));
    app.skills = Some(skills::State::new(1));
    app.mail = Some(mail::State::new(42));
    app.wallet = Some(wallet::State::new(config::FeatureFlags::default()));
    app.assets = Some(assets::State::new(config::FeatureFlags::default()));
    app
  }

  pub(super) fn test_notification(
    id: i64,
    destination: store::model::NotificationDestination,
  ) -> store::model::Notification {
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
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    app::test_support::*,
    sync::{JobKind, Subject},
  };

  mod telemetry_instrumentation {
    use super::*;
    use crate::services::telemetry;

    const ROUTES: [Route; 10] = [
      Route::Assets,
      Route::Calendar,
      Route::CharacterDetail(1),
      Route::Roster,
      Route::CorporationDetail(1),
      Route::Industry,
      Route::Mail,
      Route::Settings,
      Route::Skills(1),
      Route::Wallet,
    ];

    #[test]
    fn every_usage_token_is_free_of_spaces_slash_at_and_digits() {
      for route in ROUTES {
        let token = telemetry::collector::route_token(route.name());
        assert_eq!(token, token.to_ascii_lowercase(), "route tokens are lowercased");
        assert!(
          telemetry::collector::is_well_formed_token(&token),
          "route token `{token}` violates the shape invariant"
        );
      }

      for destination in rail::Destination::REORDERABLE {
        let token = destination_token(destination);
        assert!(
          telemetry::collector::is_well_formed_token(token),
          "destination token `{token}` violates the shape invariant"
        );
      }
      assert!(telemetry::collector::is_well_formed_token(destination_token(
        rail::Destination::Settings
      )));

      let destinations = [
        rail::Destination::Assets,
        rail::Destination::Calendar,
        rail::Destination::Roster,
        rail::Destination::Industry,
        rail::Destination::Mail,
        rail::Destination::Market,
        rail::Destination::Settings,
        rail::Destination::Skills,
        rail::Destination::Wallet,
      ];
      for destination in destinations {
        let Some(section) = crate::features::shell::nav_catalog::section(destination) else {
          continue;
        };
        for sub in section.sub_sections {
          if let Some(token) = sub_section_token(destination, sub.id) {
            assert!(
              telemetry::collector::is_well_formed_token(&token),
              "sub_section token `{token}` violates the shape invariant"
            );
          }
        }
      }
    }

    #[test]
    fn the_telemetry_sub_section_is_excluded_from_usage() {
      assert_eq!(sub_section_token(rail::Destination::Settings, "telemetry"), None);
      assert_eq!(
        sub_section_token(rail::Destination::Settings, "features"),
        Some("settings.features".to_owned())
      );
    }
  }

  mod captains_log_reminder_tick {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_stays_closed_for_the_rest_of_the_day_once_reminded() {
      let mut app = ready_app();
      let today = app.now.date_naive();
      app.captains_log_reminder_date = Some(today);

      let _ = captains_log_reminder_tick(&mut app);

      assert_eq!(
        app.captains_log_reminder_date,
        Some(today),
        "the gate fires at most once per calendar day"
      );
    }

    #[test]
    fn it_holds_the_gate_open_until_the_roster_loads() {
      let mut app = ready_app();

      let _ = captains_log_reminder_tick(&mut app);

      assert_eq!(
        app.captains_log_reminder_date, None,
        "an empty roster leaves the gate open to retry once pilots load"
      );
    }
  }

  mod handle_mcp_data_changed {
    use super::*;

    #[test]
    fn it_forces_every_open_data_view_dirty() {
      let mut app = featured_app();

      let _task = handle_mcp_data_changed(&mut app);

      assert!(app.assets.as_ref().unwrap().is_dirty(), "open assets view reloads");
      assert!(app.wallet.as_ref().unwrap().is_dirty(), "open wallet view reloads");
      assert!(
        app.character_detail.as_ref().unwrap().is_dirty(),
        "open character detail view reloads"
      );
    }
  }

  mod compare_seed_ids {
    use super::*;

    #[test]
    fn it_returns_no_seeds_without_a_roster() {
      let app = test_app();

      assert!(crate::app::compare_seed_ids(&app).is_empty());
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
      app.roster = Some(roster::State::new());
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
      app.roster = Some(roster::State::new());
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

  mod handle_shortcut {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_routes_the_open_settings_chord_to_the_settings_view() {
      let mut app = featured_app();

      let _ = crate::app::handle_shortcut(&mut app, Chord::OpenSettings);

      assert_eq!(app.route, Route::Settings);
    }

    #[test]
    fn it_opens_settings_from_any_starting_route() {
      let mut app = featured_app();
      app.route = Route::Wallet;

      let _ = crate::app::handle_shortcut(&mut app, Chord::OpenSettings);

      assert_eq!(app.route, Route::Settings);
    }
  }

  mod handle_text_input_focused {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_records_the_focused_input_on_the_tracker() {
      let mut app = test_app();

      let _ = crate::app::handle_text_input_focused(&mut app, iced::widget::Id::from("search"));

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

      let _ = crate::app::handle_skills(&mut app, skills::Message::CharacterChanged(1));
      let _ = crate::app::handle_skills(&mut app, skills::Message::OpenCompare);
      let _ = crate::app::handle_skills(&mut app, skills::Message::OpenPlanEditor(EditorSeed::New));
      let _ = crate::app::handle_skills(&mut app, skills::Message::PaneSettled("skills", 280.0));
      let _ = crate::app::handle_skills(&mut app, skills::Message::PickerToggled);
    }

    #[tokio::test]
    async fn it_opens_the_template_editor_without_an_active_character() {
      use crate::features::skills::EditorSeed;
      let mut app = featured_app();
      app.runtime = Some(test_runtime().await);
      app.skills = None;

      let _ = crate::app::handle_skills(&mut app, skills::Message::OpenPlanEditor(EditorSeed::NewTemplate));

      let (_, editor) = app.editors.iter().next().expect("the template editor opened");
      assert_eq!(editor.character_id(), None);
    }

    #[tokio::test]
    async fn it_is_a_no_op_without_a_runtime() {
      let mut app = test_app();

      let _ = crate::app::handle_skills(&mut app, skills::Message::PickerToggled);
    }
  }

  mod complete_wizard {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::services::i18n::Language;

    #[test]
    fn it_writes_a_config_with_the_chosen_language_and_storage_then_clears_should_run_wizard() {
      let config_home = tempfile::tempdir().unwrap();
      // SAFETY: only this test touches XDG_CONFIG_HOME within its body.
      unsafe {
        std::env::set_var("XDG_CONFIG_HOME", config_home.path());
      }

      let storage_root = tempfile::tempdir().unwrap();
      let db_dir = storage_root.path().join("data");
      let mut settings = config::Settings::default();
      settings.accessibility_mut().set_language(Language::De);
      settings.storage_mut().set_db_dir(Some(db_dir.clone()));
      settings.features_mut().set_sub_enabled(config::SubFeature::Mail, false);
      assert!(
        config::should_run_wizard(&settings),
        "no config exists before finishing"
      );

      complete_wizard(&settings);

      let config_path = config_home.path().join(config::APP_DIR).join("config.toml");
      assert!(config_path.is_file(), "finishing writes config.toml");
      assert!(
        db_dir.is_dir(),
        "the configured database directory is created on finish"
      );

      let written = std::fs::read_to_string(&config_path).unwrap();
      assert!(
        written.contains("language = \"de\""),
        "the chosen language is persisted to config.toml"
      );

      let reloaded = config::load().unwrap();
      assert_eq!(
        reloaded.accessibility().language(),
        Language::De,
        "the reloaded config carries the chosen language"
      );
      assert_eq!(
        reloaded.storage().db_dir(),
        &Some(db_dir),
        "the reloaded config carries the storage override"
      );
      assert!(
        !reloaded.features().is_sub_enabled(config::SubFeature::Mail),
        "the reloaded config carries the disabled feature flag"
      );
      assert!(
        !config::should_run_wizard(&reloaded),
        "with a config present the wizard is skipped on the next boot"
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
    use crate::features::roster::character_detail;

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
      assert_eq!(Route::Roster.name(), "Roster");
      assert_eq!(Route::CharacterDetail(1).name(), "roster.character_detail");
      assert_eq!(Route::ContactSync.name(), "roster.contact_sync");
      assert_eq!(Route::CorporationDetail(1).name(), "roster.corporation_detail");
      assert_eq!(Route::Skills(1).name(), "Skills");
      assert_eq!(Route::Mail.name(), "Mail");
      assert_eq!(Route::Wallet.name(), "Wallet");
      assert_eq!(Route::Assets.name(), "Assets");
      assert_eq!(Route::Settings.name(), "Settings");
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
      assert_eq!(
        Message::BudgetRules(wallet::budget_rules::Message::DropTargetLeft).variant_name(),
        "BudgetRules"
      );
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
      assert_eq!(
        Message::LeaseHeartbeatChecked(None).variant_name(),
        "LeaseHeartbeatChecked"
      );
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
        Message::BudgetRules(wallet::budget_rules::Message::DropReleased).variant_name(),
        "BudgetRules"
      );
      assert_eq!(
        Message::Calendar(calendar::Message::PickerToggled).variant_name(),
        "Calendar"
      );
      assert_eq!(
        Message::CalendarAttentionCounted(2).variant_name(),
        "CalendarAttentionCounted"
      );
      assert_eq!(
        Message::CalendarEvent(id, calendar::EventMessage::RsvpWritten).variant_name(),
        "CalendarEvent"
      );
      assert_eq!(
        Message::Contract(id, contract_detail::Message::Loaded(Box::new(None))).variant_name(),
        "Contract"
      );
      assert_eq!(
        Message::Killmail(id, killmail_detail::Message::Loaded(Box::new(None))).variant_name(),
        "Killmail"
      );
      assert_eq!(
        Message::StockpileEditor(id, assets::Message::StockpileNew).variant_name(),
        "StockpileEditor"
      );
      assert_eq!(
        Message::StockpileImport(id, assets::Message::StockpileNew).variant_name(),
        "StockpileImport"
      );
      assert_eq!(
        Message::CharacterDetail(character_detail::Message::PickerToggled).variant_name(),
        "CharacterDetail"
      );
      assert_eq!(
        Message::Roster(roster::Message::AddCharacterRequested).variant_name(),
        "Roster"
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
        Message::ContactSync(contact_sync::Message::CreateList).variant_name(),
        "ContactSync"
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
        Message::SkillPlanEditor(id, skill_plan_editor::Message::CloseRequested).variant_name(),
        "SkillPlanEditor"
      );
      assert_eq!(Message::Skills(skills::Message::PickerToggled).variant_name(), "Skills");
      assert_eq!(Message::Sync(finished_event(1)).variant_name(), "Sync");
      assert_eq!(Message::Wallet(wallet::Message::PickerToggled).variant_name(), "Wallet");
      assert_eq!(
        Message::CaptainsLog(captains_log::Message::Exit).variant_name(),
        "CaptainsLog"
      );
      assert_eq!(
        Message::CaptainsLogNudgeChecked {
          complete: true,
          date: chrono::NaiveDate::from_ymd_opt(2026, 7, 6).unwrap(),
        }
        .variant_name(),
        "CaptainsLogNudgeChecked"
      );
      assert_eq!(Message::CaptainsLogReminded(None).variant_name(), "CaptainsLogReminded");
    }
  }

  mod topmost_dismiss {
    use super::*;

    #[test]
    fn it_returns_none_when_no_overlay_is_open() {
      let app = ready_app();

      assert!(topmost_dismiss(&app).is_none());
    }

    #[test]
    fn it_dismisses_the_sync_popover() {
      let mut app = ready_app();
      app.sync_popover_open = true;

      assert_eq!(
        topmost_dismiss(&app).map(|m| m.variant_name()),
        Some("CloseSyncPopover")
      );
    }

    #[test]
    fn it_prefers_the_notifications_panel_over_the_sync_popover() {
      let mut app = ready_app();
      app.sync_popover_open = true;
      app.notifications_panel_open = true;

      assert_eq!(
        topmost_dismiss(&app).map(|m| m.variant_name()),
        Some("CloseNotificationsPanel")
      );
    }

    #[test]
    fn it_defers_to_the_palette_which_handles_its_own_escape() {
      let mut app = ready_app();
      app.sync_popover_open = true;
      app.palette = Some(command_palette::State::default());

      assert!(topmost_dismiss(&app).is_none());
    }
  }

  mod handle_escape {
    use super::*;

    #[test]
    fn it_closes_exactly_one_overlay_per_press_topmost_first() {
      let mut app = ready_app();
      app.sync_popover_open = true;
      app.notifications_panel_open = true;

      let _ = update(&mut app, Message::Escape);

      assert!(!app.notifications_panel_open, "the topmost panel closes first");
      assert!(app.sync_popover_open, "the sync popover stays open on the first press");

      let _ = update(&mut app, Message::Escape);

      assert!(!app.sync_popover_open, "the second press closes the next overlay");
    }

    #[test]
    fn it_is_a_no_op_when_nothing_is_open() {
      let mut app = ready_app();

      let _ = update(&mut app, Message::Escape);

      assert!(!app.sync_popover_open);
      assert!(!app.notifications_panel_open);
    }
  }

  mod active_feature_dismiss {
    use super::*;

    #[test]
    fn it_dismisses_nothing_for_a_route_without_open_dialogs() {
      let mut app = ready_app();
      for route in [
        Route::Assets,
        Route::CharacterDetail(1),
        Route::ContactSync,
        Route::Mail,
        Route::Roster,
        Route::Settings,
        Route::Wallet,
        Route::Calendar,
      ] {
        app.route = route;
        assert!(active_feature_dismiss(&app).is_none(), "{route:?} has no open overlay");
      }
    }

    #[test]
    fn it_dismisses_nothing_when_the_active_feature_state_is_absent() {
      let mut app = ready_app();
      app.route = Route::Wallet;
      app.wallet = None;

      assert!(active_feature_dismiss(&app).is_none());
    }

    #[tokio::test]
    async fn it_dispatches_an_open_assets_overlay_to_the_assets_feature() {
      let db = store::open_test().await.unwrap();
      let mut app = ready_app();
      app.route = Route::Assets;
      let assets = app.assets.as_mut().unwrap();
      let _ = assets::update(assets, assets::Message::TabSelected(assets::Tab::Abyssals), &db);
      let _ = assets::update(assets, assets::Message::AbyssalPickerToggled, &db);

      assert_eq!(active_feature_dismiss(&app).map(|m| m.variant_name()), Some("Assets"));
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

    #[tokio::test]
    async fn it_arms_the_updater_takeover_and_telemetry_subscriptions() {
      let mut app = ready_app();
      app.updater = Some(updater::detached_handle());
      app.telemetry = crate::clients::telemetry::Sender::new(crate::clients::telemetry::Endpoint {
        url: "http://localhost/ingest".to_owned(),
        key: "write-key".to_owned(),
      });
      assert!(app.telemetry.is_some(), "the telemetry sender builds");

      let (_dir, session) = temp_sync_session();
      app.sync_session = Some(session);
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });
      app.take_over_requested_at = Some(Utc::now());

      let _ = subscription(&app);
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

    #[test]
    fn it_falls_back_to_a_generic_event_title_before_the_state_loads() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::CalendarEvent);

      assert_eq!(window_title(&app, id), "Pod \u{2014} Event");
    }

    #[test]
    fn it_falls_back_to_a_generic_compose_title_before_the_draft_loads() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::MailCompose);

      assert_eq!(window_title(&app, id), "Pod — Compose Mail");
    }

    #[test]
    fn it_falls_back_to_a_generic_contract_title_without_a_registered_detail() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::Contract);

      assert_eq!(window_title(&app, id), "Pod \u{2014} Contract");
    }

    #[test]
    fn it_titles_a_killmail_window_from_its_loaded_state() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::Killmail);
      app.killmails.insert(
        id,
        killmail_detail::State::new(
          killmail_detail::Source::Character {
            character_id: 42,
          },
          100,
        ),
      );

      assert_eq!(window_title(&app, id), "Pod — Killmail #100");
    }

    #[test]
    fn it_falls_back_to_a_generic_killmail_title_without_a_registered_state() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::Killmail);

      assert_eq!(window_title(&app, id), "Pod — Killmail");
    }

    #[test]
    fn it_titles_the_manage_plans_window() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::ManagePlans);

      assert_eq!(window_title(&app, id), "Pod — Manage Skill Plans");
    }

    #[test]
    fn it_titles_the_skill_plan_editor_window() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::SkillPlanEditor);

      assert_eq!(window_title(&app, id), "Pod — Skill Plan Editor");
    }

    #[test]
    fn it_titles_the_splash_window_with_the_bare_app_name() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::Splash);

      assert_eq!(window_title(&app, id), "Pod");
    }

    #[test]
    fn it_falls_back_to_a_generic_stockpile_editor_title_without_a_registered_editor() {
      let mut app = test_app();
      let id = window::Id::unique();
      app.windows.register(id, Window::StockpileEditor);

      assert_eq!(window_title(&app, id), "Pod \u{2014} Stockpile Editor");
    }
  }

  mod handle_assets {
    use super::*;

    #[test]
    fn it_handles_the_window_opening_and_pane_messages() {
      let mut app = ready_app();

      let _ = handle_assets(&mut app, assets::Message::PaneSettled("assets.left", 0.4));
      assert_eq!(app.ui_state.panes.get("assets.left"), Some(&0.4));

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

  mod handle_market {
    use super::*;

    #[test]
    fn it_records_the_tree_pane_ratio_on_pane_settled() {
      let mut app = ready_app();

      let _ = handle_market(&mut app, market::Message::PaneSettled("market.tree", 0.42));

      assert_eq!(app.ui_state.panes.get("market.tree"), Some(&0.42));
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

      let _ = handle_wallet(&mut app, wallet::Message::ContractSelected(404));
      let _ = handle_wallet(&mut app, wallet::Message::ReauthRequested(1));
    }
  }

  mod entity_ref_from_result {
    use crate::{
      features::roster::entity_search::{EntityCategory, EntityResult},
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
      let mapped = crate::app::entity_ref_from_result(result(EntityCategory::Alliance, 11));

      assert_eq!(mapped.id, 11);
      assert_eq!(mapped.kind, EntityKind::Alliance);
      assert_eq!(mapped.name, "Entity 11");
      assert!(mapped.portrait.is_some());
    }

    #[test]
    fn it_maps_a_character_to_a_portrait() {
      let mapped = crate::app::entity_ref_from_result(result(EntityCategory::Character, 22));

      assert_eq!(mapped.kind, EntityKind::Character);
      assert!(mapped.portrait.is_some());
    }

    #[test]
    fn it_maps_a_corporation_to_a_logo_portrait() {
      let mapped = crate::app::entity_ref_from_result(result(EntityCategory::Corporation, 33));

      assert_eq!(mapped.kind, EntityKind::Corporation);
      assert!(mapped.portrait.is_some());
    }

    #[test]
    fn it_maps_a_solar_system_without_a_portrait() {
      let mapped = crate::app::entity_ref_from_result(result(EntityCategory::SolarSystem, 44));

      assert_eq!(mapped.kind, EntityKind::SolarSystem);
      assert!(mapped.portrait.is_none());
    }

    #[test]
    fn it_maps_a_station_without_a_portrait() {
      let mapped = crate::app::entity_ref_from_result(result(EntityCategory::Station, 55));

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

      let _ = crate::app::contact_entity_search(&state, &runtime, "qu".to_owned());
    }

    #[tokio::test]
    async fn it_builds_a_search_task_for_an_empty_query() {
      let runtime = test_runtime().await;
      let state = character_detail::State::new(42, &[]);

      let _ = crate::app::contact_entity_search(&state, &runtime, String::new());
    }
  }
}
