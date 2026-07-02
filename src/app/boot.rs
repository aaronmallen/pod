use super::*;

pub(super) struct PreparedStore {
  database_path: std::path::PathBuf,
  db_present: bool,
  from_marker: Option<String>,
  lease: Option<HolderInfo>,
  settings: config::Settings,
  sync_session: Option<store::sync_session::SyncSession>,
}

pub(super) fn boot() -> (App, Task<Message>) {
  let settings = config::load().unwrap_or_default();
  let accessibility = *settings.accessibility();
  color::set_high_contrast(*accessibility.high_contrast());
  color::set_accent(settings.ui().accent());
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
    budget_rules: None,
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
    contact_sync: None,
    contracts: WindowStates::default(),
    corporation_detail: None,
    editor: None,
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
  let db_present = database_path.exists();
  let from_marker = splash::seed::sde_version_path()
    .and_then(|path| std::fs::read_to_string(path).ok())
    .map(|contents| contents.trim().to_owned());
  let sync_session = store::sync_session::SyncSession::from_config(settings.storage(), machine_id);
  let lease = acquire_lease(sync_session.as_ref());
  Ok(PreparedStore {
    database_path,
    db_present,
    from_marker,
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
  let PreparedStore {
    database_path,
    db_present,
    from_marker,
    lease,
    settings,
    sync_session,
  } = prepared;

  let pools = open_migrated_pools(&database_path, from_marker, db_present).await?;

  let http = http::Client::builder(http::Cache::new(pools.interactive.clone())).build();
  Ok(StoreReady {
    db: pools.interactive,
    http,
    lease,
    settings,
    sync_db: pools.sync,
    sync_housekeeping_db: pools.housekeeping,
    sync_session,
  })
}

// Each hook is self-contained: it resolves and opens the resources it needs, so no config re-save
// fires purely because a migrator ran. The before -> sqlx migrate -> after ordering stays enforced by
// this call sequence (a comment-preserving migrator's toml_edit edit therefore survives the boot flow).
async fn open_migrated_pools(
  database_path: &std::path::Path,
  from_marker: Option<String>,
  db_present: bool,
) -> Result<store::Pools, String> {
  let registry = migration::Registry::resolve(from_marker.as_deref(), db_present);
  registry.before_db_migration().await.map_err(store_err)?;
  let pools = store::open_pools(database_path).await.map_err(store_err)?;
  registry.after_db_migration().await.map_err(store_err)?;
  Ok(pools)
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_support::*;

  mod boot {
    use super::*;

    #[test]
    fn init_telemetry_stays_a_no_op_without_a_baked_endpoint() {
      let settings = config::Settings::default();

      assert!(
        init_telemetry(&settings).is_none(),
        "with no baked endpoint the telemetry sender is never built"
      );
    }

    #[test]
    fn subscribe_updater_is_a_no_op_without_a_handle() {
      subscribe_updater(None);
    }
  }

  mod open_migrated_pools {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_does_not_re_serialize_config_when_a_migrator_runs() {
      let config_home = tempfile::tempdir().unwrap();
      let db_root = tempfile::tempdir().unwrap();
      // SAFETY: only this test touches XDG_CONFIG_HOME within its body.
      unsafe {
        std::env::set_var("XDG_CONFIG_HOME", config_home.path());
      }

      let db_dir = db_root.path().join("data");
      std::fs::create_dir_all(&db_dir).unwrap();
      let mut settings = config::Settings::default();
      settings.storage_mut().set_db_dir(Some(db_dir.clone()));
      config::save(&settings);

      // Prepend a user comment the whole-config `toml::to_string_pretty` save path would drop.
      let config_path = config_home.path().join("pod").join("config.toml");
      let original = format!(
        "# a blanket config re-save would clobber this comment\n{}",
        std::fs::read_to_string(&config_path).unwrap()
      );
      std::fs::write(&config_path, &original).unwrap();

      let database_path = store::bootstrap::local_path(config::load().unwrap().storage());

      // A 0.6.6 from-marker selects the 0.6.8 CRLF-heal migrator, so the registry is non-empty and the
      // retired `persist_migrated_settings` would have re-serialized (and clobbered) the config here.
      open_migrated_pools(&database_path, Some("0+pod-0.6.6+seed-1+lang-en".to_owned()), true)
        .await
        .unwrap();

      assert_eq!(
        std::fs::read_to_string(&config_path).unwrap(),
        original,
        "a migrator running must not trigger a config re-serialize that drops user comments"
      );
    }
  }

  mod boot_ordering {
    use pretty_assertions::assert_eq;

    use super::*;

    fn splash_app() -> App {
      let mut app = test_app();
      app.splash = Some(splash::State::default());
      app
    }

    fn phase(app: &App) -> &splash::Phase {
      &app.splash.as_ref().expect("splash present").phase
    }

    #[test]
    fn start_boot_runs_the_preflight_first_when_an_updater_is_present() {
      let mut app = splash_app();
      app.updater = Some(updater::detached_handle());

      let _ = start_boot(&mut app);

      assert_eq!(
        phase(&app),
        &splash::Phase::CheckingUpdate,
        "an updater handle makes the splash check for an update before any boot work"
      );
      assert!(
        app.store_ready.is_none(),
        "no store-open work runs while the preflight is in flight"
      );
    }

    #[tokio::test]
    async fn start_boot_skips_straight_to_loading_without_an_updater() {
      let mut app = splash_app();
      assert!(app.updater.is_none());

      let _ = start_boot(&mut app);

      assert_eq!(
        phase(&app),
        &splash::Phase::Loading,
        "with no updater the splash skips the preflight and boots immediately"
      );
    }

    #[tokio::test]
    async fn begin_boot_moves_a_checking_splash_into_loading() {
      let mut app = splash_app();
      app.splash.as_mut().unwrap().phase = splash::Phase::CheckingUpdate;

      let _ = begin_boot(&mut app);

      assert_eq!(phase(&app), &splash::Phase::Loading);
    }

    #[test]
    fn begin_boot_is_a_no_op_once_the_splash_has_left_the_preflight() {
      let mut app = splash_app();

      let _ = begin_boot(&mut app);

      assert_eq!(
        phase(&app),
        &splash::Phase::Loading,
        "a duplicate fall-through after boot already started leaves the splash untouched"
      );
    }

    #[tokio::test]
    async fn later_proceeds_to_boot() {
      let mut app = splash_app();
      app.splash.as_mut().unwrap().phase = splash::Phase::Update;

      let _ = update_splash(&mut app, splash::Message::Later);

      assert_eq!(phase(&app), &splash::Phase::Loading, "Later falls through to boot");
    }

    #[tokio::test]
    async fn no_update_proceeds_to_boot() {
      let mut app = splash_app();
      app.splash.as_mut().unwrap().phase = splash::Phase::CheckingUpdate;

      let _ = update_splash(&mut app, splash::Message::UpdateNotAvailable);

      assert_eq!(phase(&app), &splash::Phase::Loading);
    }

    #[tokio::test]
    async fn a_check_failure_during_the_preflight_proceeds_to_boot() {
      let mut app = splash_app();
      app.splash.as_mut().unwrap().phase = splash::Phase::CheckingUpdate;

      let _ = update_splash(&mut app, splash::Message::UpdateFailed("check boom".to_owned()));

      assert_eq!(
        phase(&app),
        &splash::Phase::Loading,
        "a failed preflight check never strands the splash"
      );
    }

    #[tokio::test]
    async fn an_install_failure_during_updating_proceeds_to_boot() {
      let mut app = splash_app();
      app.splash.as_mut().unwrap().phase = splash::Phase::Updating;

      let _ = update_splash(&mut app, splash::Message::UpdateFailed("install boom".to_owned()));

      assert_eq!(phase(&app), &splash::Phase::Loading);
    }

    #[test]
    fn choosing_update_moves_into_updating() {
      let mut app = splash_app();
      app.splash.as_mut().unwrap().phase = splash::Phase::Update;

      let _ = update_splash(&mut app, splash::Message::Update);

      assert_eq!(
        phase(&app),
        &splash::Phase::Updating,
        "choosing the update applies it and shows download progress"
      );
    }

    #[test]
    fn an_available_update_during_the_preflight_shows_the_prompt() {
      let mut app = splash_app();
      app.splash.as_mut().unwrap().phase = splash::Phase::CheckingUpdate;

      let _ = drive_splash_preflight(
        &mut app,
        &updater::State::UpdateAvailable {
          version: "9.9.9".to_owned(),
        },
      );

      assert_eq!(phase(&app), &splash::Phase::Update);
      assert_eq!(app.splash.as_ref().unwrap().update_version.as_deref(), Some("9.9.9"));
    }

    #[test]
    fn downloading_advances_the_update_progress() {
      let mut app = splash_app();
      app.splash.as_mut().unwrap().phase = splash::Phase::Updating;

      let _ = drive_splash_preflight(
        &mut app,
        &updater::State::Downloading {
          version: "9.9.9".to_owned(),
        },
      );

      assert_eq!(app.splash.as_ref().unwrap().update_progress, 0.5);
    }

    #[test]
    fn ready_to_restart_fills_the_progress_bar() {
      let mut app = splash_app();
      app.splash.as_mut().unwrap().phase = splash::Phase::Updating;

      let _ = drive_splash_preflight(
        &mut app,
        &updater::State::ReadyToRestart {
          version: "9.9.9".to_owned(),
        },
      );

      assert_eq!(
        app.splash.as_ref().unwrap().update_progress,
        1.0,
        "a ready install fills the bar and triggers the restart"
      );
    }

    #[tokio::test]
    async fn an_updater_error_during_the_preflight_falls_through_to_boot() {
      let mut app = splash_app();
      app.splash.as_mut().unwrap().phase = splash::Phase::CheckingUpdate;

      let _ = handle_updater_state_changed(
        &mut app,
        updater::State::Error {
          message: "network down".to_owned(),
        },
      );

      assert_eq!(
        phase(&app),
        &splash::Phase::Loading,
        "an updater error while checking never strands the splash"
      );
    }

    #[test]
    fn updater_state_changes_leave_the_running_app_untouched() {
      let mut app = test_app();
      assert!(app.splash.is_none());

      let _ = handle_updater_state_changed(
        &mut app,
        updater::State::UpdateAvailable {
          version: "9.9.9".to_owned(),
        },
      );

      assert_eq!(
        app.updater_state,
        updater::State::UpdateAvailable {
          version: "9.9.9".to_owned()
        },
        "with no splash the handler keeps its existing running-app banner/toast behaviour"
      );
    }

    #[test]
    fn the_preflight_is_inert_without_a_splash() {
      let mut app = test_app();
      assert!(app.splash.is_none());

      let _ = drive_splash_preflight(
        &mut app,
        &updater::State::UpdateAvailable {
          version: "9.9.9".to_owned(),
        },
      );

      assert!(
        app.splash.is_none(),
        "the preflight does nothing when there is no splash to drive"
      );
    }

    #[test]
    fn the_preflight_ignores_a_state_that_does_not_match_the_phase() {
      let mut app = splash_app();
      app.splash.as_mut().unwrap().phase = splash::Phase::Loading;

      let _ = drive_splash_preflight(
        &mut app,
        &updater::State::UpdateAvailable {
          version: "9.9.9".to_owned(),
        },
      );

      assert_eq!(
        phase(&app),
        &splash::Phase::Loading,
        "an update available outside the checking phase leaves the splash untouched"
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

      let sync_esi = build_sync_esi(sync_db, crate::services::i18n::Language::default()).unwrap();
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

      let sync_esi = build_sync_esi(interactive_db, crate::services::i18n::Language::default()).unwrap();

      assert!(
        !Arc::ptr_eq(&sync_esi.http(), &ui_esi.http()),
        "the sync engine no longer shares the interactive-pool-backed HTTP client"
      );
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
}
