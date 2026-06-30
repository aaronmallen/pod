use super::*;

/// Drives a take-over end to end on a blocking-safe async task: it first closes every working-copy
/// pool the parked instance holds, then runs the lease claim (which on success overwrites the
/// working-copy file via `publish_database`), then reopens fresh pools against the swapped file. The
/// close-before-swap ordering is mandatory on Windows, whose mandatory file locking rejects an
/// in-place overwrite of an open `.db` file with `PermissionDenied` (POSIX hosts tolerate it). The
/// pools are always reopened — on a claim they read the freshly pulled canonical copy, and on a
/// declined or failed claim they reopen the unchanged working copy — so the app is never left with
/// closed pools regardless of outcome.
pub(super) fn run_take_over(
  ready: StoreReady,
  session: store::sync_session::SyncSession,
  force: bool,
) -> Task<Message> {
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

/// Checkpoints, closes every pool handle, releases the lease, and reopens the databases
/// read-only so the app continues as a passenger while the requester holds the share.
///
/// `checkpoint_and_push` errors are logged but never short-circuit: an early return would
/// strand writes that have not yet been pushed to the share.  All six pool handles are
/// closed explicitly before `session.release()` so the file lock is fully surrendered
/// before the requester's pools open on the same path.
pub(super) fn demote_to_slave(
  ready: StoreReady,
  session: store::sync_session::SyncSession,
  requester: HolderInfo,
) -> Task<Message> {
  Task::future(async move {
    let settings = ready.settings.clone();
    if let Err(error) = session.checkpoint_and_push().await {
      tracing::warn!(target: "pod::lifecycle", %error, "final checkpoint and push before yielding the share failed");
    }
    ready.db.reader().close().await;
    ready.db.writer().close().await;
    ready.sync_db.reader().close().await;
    ready.sync_db.writer().close().await;
    ready.sync_housekeeping_db.reader().close().await;
    ready.sync_housekeeping_db.writer().close().await;
    if let Err(error) = session.release() {
      tracing::warn!(target: "pod::lifecycle", %error, "releasing the lease while yielding the share failed");
    }
    match reopen_after_take_over_inner(&session, Some(requester.clone()), settings).await {
      Ok(ready) => Message::DemotedToSlave(Box::new(ready), requester),
      Err(error) => {
        tracing::error!(target: "pod::lifecycle", %error, "reopening the database read-only after yielding the share failed");
        Message::InitFailed(error)
      }
    }
  })
}

pub(super) fn claim_lease(session: &store::sync_session::SyncSession, force: bool) -> TakeOverOutcome {
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

pub(super) async fn reopen_after_take_over_inner(
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

pub(super) fn persist_machine_id(settings: &mut config::Settings) -> String {
  let had_id = settings.storage().machine_id().is_some();
  let machine_id = settings.storage_mut().machine_id_or_generate();
  if !had_id {
    config::save(settings);
  }
  machine_id
}

pub(super) fn acquire_lease(session: Option<&store::sync_session::SyncSession>) -> Option<HolderInfo> {
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

pub(super) fn read_only_engine_state(held_by: Option<HolderInfo>) -> EngineState {
  EngineState::ReadOnly {
    held_by,
  }
}

pub(super) fn holding_lease(app: &App) -> bool {
  app.sync_session.is_some() && app.read_only.is_none()
}

pub(super) fn parked(app: &App) -> bool {
  app.sync_session.is_some() && app.read_only.is_some()
}

pub(super) fn handle_lease_heartbeat(app: &mut App) -> Task<Message> {
  if !holding_lease(app) {
    return Task::none();
  }
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  if let Some(request) = fresh_foreign_request(&session, Utc::now()) {
    return start_demote_to_slave(app, HolderInfo::from(request));
  }
  Task::future(async move {
    if let Err(error) = session.heartbeat(Utc::now()) {
      tracing::warn!(target: "pod::lifecycle", %error, "lease heartbeat failed");
    }
  })
  .discard()
}

pub(super) fn fresh_foreign_request(
  session: &store::sync_session::SyncSession,
  now: DateTime<Utc>,
) -> Option<store::share_meta::TakeoverRequest> {
  let request = session.read_take_over_request()?;
  (request.machine_id != session.machine_id() && !request.is_stale(store::lease::STALE_THRESHOLD, now))
    .then_some(request)
}

pub(super) fn start_demote_to_slave(app: &mut App, requester: HolderInfo) -> Task<Message> {
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  let Some(ready) = app.store_ready.take() else {
    return Task::none();
  };
  if let Some(runtime) = app.runtime.as_ref() {
    runtime.sync.shutdown();
  }
  app.runtime = None;
  tracing::info!(target: "pod::lifecycle", hostname = %requester.hostname, "yielding the share to a take-over request");
  demote_to_slave(ready, session, requester)
}

pub(super) fn handle_demoted_to_slave(app: &mut App, ready: StoreReady, requester: HolderInfo) -> Task<Message> {
  app.read_only = Some(requester.clone());
  app.engine_state = read_only_engine_state(Some(requester));
  app.store_ready = Some(ready.clone());
  build_runtime(ready)
}

pub(super) fn handle_periodic_pull(app: &mut App) -> Task<Message> {
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

pub(super) fn handle_periodic_push(app: &mut App) -> Task<Message> {
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

pub(super) fn handle_pulled(app: &mut App, pulled: bool) -> Task<Message> {
  if pulled {
    app.last_synced = Some(Utc::now());
    app.roster_dirty = true;
  }
  refresh_storage_status(app);
  Task::none()
}

pub(super) fn pull_task(session: store::sync_session::SyncSession) -> Task<Message> {
  Task::future(pull_bundle(session))
}

pub(super) async fn pull_bundle(session: store::sync_session::SyncSession) -> Message {
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

pub(super) fn handle_lock_released(app: &mut App) -> Task<Message> {
  refresh_storage_status(app);
  Task::none()
}

pub(super) fn handle_pushed(app: &mut App, mark: Option<SystemTime>) -> Task<Message> {
  if let Some(mark) = mark {
    app.last_push = Some(mark);
    app.last_synced = Some(Utc::now());
  }
  refresh_storage_status(app);
  Task::none()
}

/// Captures the working-copy write timestamp *before* the push so the debounce mark never races
/// ahead of a write that lands mid-checkpoint; such a write simply re-pushes on the next tick.
pub(super) fn push_task(session: store::sync_session::SyncSession) -> Task<Message> {
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

pub(super) fn handle_reacquire_lease(app: &mut App) -> Task<Message> {
  if !parked(app) {
    return Task::none();
  }
  // A pending request means the requester has written its intent but has not yet claimed
  // the lease.  Reacquiring here would put a live foreign lease back in its way and force
  // an escalation to a force-claim.
  if let Some(session) = app.sync_session.as_ref()
    && fresh_foreign_request(session, Utc::now()).is_some()
  {
    tracing::trace!(target: "pod::lifecycle", "lease re-acquire stands down; a foreign take-over request is outstanding");
    return Task::none();
  }
  start_take_over(app, false)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TakeoverPollAction {
  Claim,
  Force,
  Wait,
}

/// Force requires both a stale lease and an elapsed request window: the lease can go stale
/// while the host is mid-checkpoint inside [`demote_to_slave`], and forcing without the
/// full window risks opening the databases while the host is still closing them.
pub(super) fn take_over_poll_action(
  lease: Option<&store::share_meta::Lease>,
  requested_at: DateTime<Utc>,
  now: DateTime<Utc>,
) -> TakeoverPollAction {
  match lease {
    None => TakeoverPollAction::Claim,
    Some(lease) if lease.is_stale(store::lease::STALE_THRESHOLD, now) && request_window_elapsed(requested_at, now) => {
      TakeoverPollAction::Force
    }
    Some(_) => TakeoverPollAction::Wait,
  }
}

pub(super) fn request_window_elapsed(requested_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
  now
    .signed_duration_since(requested_at)
    .to_std()
    .is_ok_and(|elapsed| elapsed > store::lease::STALE_THRESHOLD)
}

pub(super) fn handle_take_over_poll(app: &mut App) -> Task<Message> {
  if !parked(app) {
    return Task::none();
  }
  let Some(requested_at) = app.take_over_requested_at else {
    return Task::none();
  };
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  match take_over_poll_action(session.read_lease().as_ref(), requested_at, Utc::now()) {
    TakeoverPollAction::Claim => start_take_over(app, false),
    TakeoverPollAction::Force => start_take_over(app, true),
    TakeoverPollAction::Wait => Task::none(),
  }
}

/// Common take-over launch: drops the parked runtime (releasing its working-copy pool clones), takes
/// the `StoreReady` whose three pools are the only remaining handles on the file, and hands them to
/// [`run_take_over`] so they are closed before the swap and reopened after. Short-circuits cleanly
/// when no session or store is present, leaving the app untouched.
pub(super) fn start_take_over(app: &mut App, force: bool) -> Task<Message> {
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  let Some(ready) = app.store_ready.take() else {
    return Task::none();
  };
  app.runtime = None;
  run_take_over(ready, session, force)
}

pub(super) fn handle_cancel_take_over(app: &mut App) -> Task<Message> {
  app.confirm_force_takeover = false;
  Task::none()
}

pub(super) fn handle_confirm_take_over(app: &mut App) -> Task<Message> {
  app.confirm_force_takeover = false;
  if app.read_only.is_none() {
    return Task::none();
  }
  start_take_over(app, true)
}

pub(super) fn handle_take_over(app: &mut App) -> Task<Message> {
  if app.read_only.is_none() {
    return Task::none();
  }
  let Some(session) = app.sync_session.clone() else {
    return Task::none();
  };
  let now = Utc::now();
  let live_host = session.read_lease().is_some_and(|lease| {
    lease.machine_id != session.machine_id() && !lease.is_stale(store::lease::STALE_THRESHOLD, now)
  });
  // A live host means we request cooperatively: write a takeover.json so the holder can
  // checkpoint and yield gracefully.  The TakeoverPoll subscription escalates to a
  // force-claim if the host goes stale without yielding.
  if live_host {
    if let Err(error) = session.request_take_over(now) {
      tracing::warn!(target: "pod::lifecycle", %error, "writing the take-over request failed");
    }
    app.take_over_requested_at = Some(now);
    return Task::none();
  }
  start_take_over(app, false)
}

pub(super) fn handle_take_over_resolved(
  app: &mut App,
  outcome: TakeOverOutcome,
  mut ready: StoreReady,
) -> Task<Message> {
  app.confirm_force_takeover = false;
  match outcome {
    TakeOverOutcome::Claimed => {
      app.read_only = None;
      app.take_over_requested_at = None;
      if let Some(session) = app.sync_session.as_ref()
        && let Err(error) = session.clear_take_over_request()
      {
        tracing::warn!(target: "pod::lifecycle", %error, "clearing the take-over request after claiming failed");
      }
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum TakeOverOutcome {
  Claimed,
  Failed,
}

pub(super) fn read_only_banner(
  holder: &HolderInfo,
  confirming: bool,
  requesting: bool,
  now: DateTime<Utc>,
) -> Element<'static, Message> {
  let (message, actions): (String, Element<'static, Message>) = if confirming {
    let last_active = status::format_since((now - holder.last_active).num_seconds().max(0) as u64);
    let confirm = Button::danger(t!("shell.takeover.take_over_anyway").into_owned())
      .size(ButtonSize::Sm)
      .on_press(Message::ConfirmTakeOver);
    let cancel = Button::ghost(t!("common.cancel").into_owned())
      .size(ButtonSize::Sm)
      .on_press(Message::CancelTakeOver);
    (
      read_only_confirm_label(&holder.hostname, &last_active),
      Row::new()
        .push(cancel)
        .push(confirm)
        .align_y(Vertical::Center)
        .spacing(spacing::SPACE_2)
        .into(),
    )
  } else if requesting {
    let force = Button::danger(t!("shell.takeover.take_over_anyway").into_owned())
      .size(ButtonSize::Sm)
      .on_press(Message::ConfirmTakeOver);
    (read_only_requesting_label(&holder.hostname), force.into())
  } else {
    let action = Button::primary(t!("shell.takeover.take_over").into_owned())
      .size(ButtonSize::Sm)
      .on_press(Message::TakeOver);
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

pub(super) fn read_only_banner_label(hostname: &str) -> String {
  t!("shell.takeover.read_only", hostname => hostname).into_owned()
}

pub(super) fn read_only_requesting_label(hostname: &str) -> String {
  t!("shell.takeover.requesting", hostname => hostname).into_owned()
}

pub(super) fn read_only_confirm_label(hostname: &str, last_active: &str) -> String {
  t!("shell.takeover.confirm", hostname => hostname, last_active => last_active).into_owned()
}
