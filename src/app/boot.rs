use super::*;

pub(super) struct PreparedStore {
  database_path: std::path::PathBuf,
  lease: Option<HolderInfo>,
  settings: config::Settings,
  sync_session: Option<store::sync_session::SyncSession>,
}

pub(super) fn boot() -> (App, Task<Message>) {
  let settings = config::load().unwrap_or_default();
  let accessibility = *settings.accessibility();
  color::set_high_contrast(*accessibility.high_contrast());
  i18n::set_locale(accessibility.language());
  let image_root = settings.storage().resolved_cache_dir().join("images");
  store::images::init_root(image_root);

  let telemetry = init_telemetry(&settings);
  let first_run = config::should_run_wizard(&settings);

  auth::install();
  let window_settings = if first_run {
    window::Settings {
      size: Size::new(
        spacing::layout::WINDOW_DEFAULT_WIDTH,
        spacing::layout::WINDOW_DEFAULT_HEIGHT,
      ),
      position: window::Position::Centered,
      icon: app_icon(),
      ..window::Settings::default()
    }
  } else {
    window::Settings {
      size: Size::new(spacing::layout::SPLASH_WIDTH, spacing::layout::SPLASH_HEIGHT),
      decorations: false,
      resizable: false,
      transparent: true,
      position: window::Position::Centered,
      icon: app_icon(),
      ..window::Settings::default()
    }
  };
  let (id, open_task) = window::open(window_settings);

  let mut registry = Windows::default();
  registry.register(id, if first_run { Window::FirstRun } else { Window::Splash });

  let updater = updater::Config::from_env().map(updater::spawn);
  subscribe_updater(updater.as_ref());

  let mut app = App {
    accessibility,
    assets: None,
    auth: auth::State::default(),
    calendar: None,
    calendar_attention: 0,
    calendar_events: WindowStates::default(),
    character_detail: None,
    roster: None,
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
    splash: (!first_run).then(splash::State::default),
    splash_step: 0,
    stockpile_editors: WindowStates::default(),
    stockpile_imports: WindowStates::default(),
    store_ready: None,
    status: sync::SyncStatus::new(),
    sync_popover_open: false,
    sync_session: None,
    sync_tick: false,
    take_over_requested_at: None,
    telemetry,
    toasts: Vec::new(),
    ui_state: window_state::load(),
    updater: updater.clone(),
    updater_state: updater::State::default(),
    updater_toast_dismissed: false,
    wallet: None,
    windows: registry,
    wizard: first_run.then(wizard::State::default),
  };
  let boot = if first_run { Task::none() } else { start_boot(&mut app) };
  let task = Task::batch([open_task.map(Message::WindowOpened), boot]);

  (app, task)
}

pub(super) fn start_boot(app: &mut App) -> Task<Message> {
  match app.updater.clone() {
    Some(handle) => {
      if let Some(state) = app.splash.as_mut() {
        let _ = splash::update(state, splash::Message::BeginChecking);
      }
      splash::preflight::check(&handle).map(Message::Splash)
    }
    None => open_store(),
  }
}

/// The single boot fall-through: once the preflight resolves (no update, check error, timeout, the
/// user chose Later, or an update download/install error), this moves the splash out of the preflight
/// into `Loading` and starts the store-open/seed chain. Guarding on the preflight phase makes it a
/// no-op when the same resolution arrives twice (the preflight watch and the global updater receiver
/// can both observe one `Error`), so the store opens exactly once.
pub(super) fn begin_boot(app: &mut App) -> Task<Message> {
  let proceed = match app.splash.as_mut() {
    Some(state)
      if matches!(
        state.phase,
        splash::Phase::CheckingUpdate | splash::Phase::Update | splash::Phase::Updating
      ) =>
    {
      state.phase = splash::Phase::Loading;
      state.step_label = t!("splash.status.starting_up").into_owned();
      true
    }
    _ => false,
  };

  if proceed { open_store() } else { Task::none() }
}

pub(super) fn open_store() -> Task<Message> {
  let (tx, rx) = iced::futures::channel::mpsc::channel(RUNTIME_CHANNEL_BUFFER);
  tokio::spawn(run_open_store(tx));
  Task::stream(rx)
}

pub(super) async fn run_open_store(mut tx: Tx) {
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

pub(super) fn store_err(error: impl std::fmt::Display) -> String {
  error.to_string()
}

pub(super) fn prepare_store() -> Result<PreparedStore, String> {
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

pub(super) async fn open_store_inner() -> Result<StoreReady, String> {
  // SEAM (networked-drive storage rework): store prep performs a blocking copy off the (possibly
  // network) share in Sync mode plus lease file IO. Run it on a blocking thread so a stalled or slow
  // mount can't wedge the async boot worker — the first window renders independent of this finishing.
  let prepared = tokio::task::spawn_blocking(prepare_store).await.map_err(store_err)??;
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

pub(super) fn build_runtime(ready: StoreReady) -> Task<Message> {
  let (tx, rx) = iced::futures::channel::mpsc::channel(RUNTIME_CHANNEL_BUFFER);
  tokio::spawn(run_build_runtime(ready, tx));
  Task::stream(rx)
}

pub(super) async fn run_build_runtime(ready: StoreReady, mut tx: Tx) {
  apply_language_refresh(&ready).await;
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

// Runs the boot-time language-switch re-sync (ADR-0041 sections 3 and 4) after the SDE re-seed has
// landed and before the engine's first discovery pass, so the expired language-dependent jobs present
// as never-attempted and re-fetch under the new `?language=`. A read-only opener holds no lease, so it
// must not write the ledger; it is skipped there and the writer that owns the lease applies it.
pub(super) async fn apply_language_refresh(ready: &StoreReady) {
  if ready.lease.is_some() {
    return;
  }
  let Some(marker) = splash::seed::synced_language_path() else {
    return;
  };
  let configured = ready.settings.accessibility().language();
  match sync::refresh_for_language_switch(&ready.sync_db, configured, &marker).await {
    Ok(sync::Refresh::Switched {
      expired,
    }) => {
      tracing::info!(target: "pod::sync", %configured, expired, "language switch detected; expired language-dependent jobs");
    }
    Ok(sync::Refresh::NoSwitch) => {}
    Err(error) => {
      tracing::warn!(target: "pod::sync", %error, "language-switch re-sync failed; leaving the marker for the next boot");
    }
  }
}

pub(super) async fn forward_sync_events(mut events: tokio::sync::mpsc::Receiver<sync::Event>, mut tx: Tx) {
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

pub(super) fn build_sync_esi(
  sync_db: store::Database,
  language: crate::services::i18n::Language,
) -> Result<Arc<esi::Client>, String> {
  let sync_http = http::Client::builder(http::Cache::new(sync_db)).build();
  Ok(Arc::new(
    esi::Client::builder(sync_http)
      .language(language)
      .user_agent(clients::user_agent())
      .build()
      .map_err(|error| error.to_string())?,
  ))
}

pub(super) fn build_runtime_inner(
  ready: StoreReady,
) -> Result<(Runtime, tokio::sync::mpsc::Receiver<sync::Event>), String> {
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
      .language(settings.accessibility().language())
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
    let sync_esi = build_sync_esi(sync_db.clone(), settings.accessibility().language())?;
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
pub(super) fn inert_sync() -> (sync::Handle, tokio::sync::mpsc::Receiver<sync::Event>) {
  let (commands, _commands_rx) = tokio::sync::mpsc::unbounded_channel();
  let (restart, _restart_rx) = tokio::sync::mpsc::unbounded_channel();
  let (_events_tx, events) = tokio::sync::mpsc::channel(1);
  (sync::Handle::new(commands, restart), events)
}

pub(super) fn init_telemetry(settings: &config::Settings) -> Option<clients::telemetry::Sender> {
  let sender = clients::telemetry::Endpoint::from_env()
    .and_then(clients::telemetry::Sender::new)
    .inspect(|sender| {
      telemetry::init(
        &settings.storage().machine_id().clone().unwrap_or_default(),
        *settings.telemetry(),
        settings.accessibility().language(),
      );
      if let Some(buffer) = crash::buffer_path() {
        crash::deliver(sender, &buffer, *settings.telemetry(), true);
      }
    });

  if sender.is_none()
    && let Some(buffer) = crash::buffer_path()
  {
    let _ = std::fs::remove_file(&buffer);
  }

  sender
}

pub(super) fn subscribe_updater(handle: Option<&updater::Handle>) {
  if let Some(handle) = handle
    && let Ok(mut guard) = UPDATER_RECEIVER.lock()
  {
    *guard = Some(handle.subscribe());
  }
}

pub(super) fn handle_store_opened(app: &mut App, ready: StoreReady) -> Task<Message> {
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

pub(super) fn recover_unsynced_changes(app: &App) -> Task<Message> {
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

pub(super) fn handle_ready(app: &mut App, runtime: Runtime) -> Task<Message> {
  let load_roster = roster::load(&runtime.db, *runtime.settings.features());
  app.roster = Some(roster::State::new());
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
    load_roster.map(Message::Roster),
    load_tags,
    begin_splash_expand(app),
    replay_pending_auth(app),
  ])
}

pub(super) fn begin_splash_expand(app: &mut App) -> Task<Message> {
  match app.splash.as_mut() {
    Some(state) => splash::update(state, splash::Message::LoadingComplete).map(Message::Splash),
    None => Task::none(),
  }
}

pub(super) fn replay_pending_auth(app: &mut App) -> Task<Message> {
  match app.pending_auth.take() {
    Some(msg) => update(app, Message::Auth(msg)),
    None => Task::none(),
  }
}

pub(super) fn handle_init_failed(app: &mut App, error: String) -> Task<Message> {
  tracing::error!(%error, "bootstrap failed");
  app.store_ready = None;
  if let Some(state) = app.splash.as_mut() {
    let _ = splash::update(
      state,
      splash::Message::StepChanged {
        label: t!("splash.view.error", error => error).into_owned(),
        progress: state.progress_target,
      },
    );
  }
  app.init_error = Some(error);
  Task::none()
}

pub(super) fn transition_to_main(app: &mut App) -> Task<Message> {
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
