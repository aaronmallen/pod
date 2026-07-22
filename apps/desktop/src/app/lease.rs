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
    let displaced_by: Option<HolderInfo> = match session.heartbeat(Utc::now()) {
      Ok(outcome) => outcome.into(),
      Err(error) => {
        tracing::warn!(target: "pod::lifecycle", %error, "lease heartbeat failed");
        None
      }
    };
    Message::LeaseHeartbeatChecked(displaced_by)
  })
}

pub(super) fn handle_lease_heartbeat_checked(app: &mut App, displaced_by: Option<HolderInfo>) -> Task<Message> {
  let Some(holder) = displaced_by else {
    return Task::none();
  };
  if !holding_lease(app) {
    return Task::none();
  }
  tracing::warn!(
    target: "pod::lifecycle",
    hostname = %holder.hostname,
    "another machine claimed the lease out from under this instance; yielding"
  );
  start_demote_to_slave(app, holder)
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
  let lease = app
    .sync_session
    .as_ref()
    .and_then(store::sync_session::SyncSession::read_lease);
  let unchanged_for = app.holder_watch.observe(lease.as_ref(), std::time::Instant::now());
  if holder_heartbeat_flatlined(unchanged_for) {
    tracing::info!(
      target: "pod::lifecycle",
      "the holder's heartbeat has not advanced despite a fresh-looking timestamp; force-reclaiming the share"
    );
    return start_take_over(app, true);
  }
  start_take_over(app, false)
}

// A lease timestamp alone cannot prove liveness across machines: a dead holder whose clock ran
// ahead leaves a heartbeat that looks perpetually fresh to `is_stale`. A live holder rewrites the
// lease every HEARTBEAT_INTERVAL, so a heartbeat *value* that never changes across locally-timed
// observations is the clock-skew-immune deadness signal.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct HolderWatch {
  seen: Option<(String, DateTime<Utc>, std::time::Instant)>,
}

impl HolderWatch {
  pub(super) fn clear(&mut self) {
    self.seen = None;
  }

  pub(super) fn observe(
    &mut self,
    lease: Option<&store::share_meta::Lease>,
    at: std::time::Instant,
  ) -> Option<std::time::Duration> {
    let Some(lease) = lease else {
      self.seen = None;
      return None;
    };
    match &self.seen {
      Some((machine_id, heartbeat, since)) if *machine_id == lease.machine_id && *heartbeat == lease.heartbeat => {
        Some(at.saturating_duration_since(*since))
      }
      _ => {
        self.seen = Some((lease.machine_id.clone(), lease.heartbeat, at));
        Some(std::time::Duration::ZERO)
      }
    }
  }
}

pub(super) fn holder_heartbeat_flatlined(unchanged_for: Option<std::time::Duration>) -> bool {
  unchanged_for.is_some_and(|elapsed| elapsed > store::lease::STALE_THRESHOLD * 3)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TakeoverPollAction {
  Claim,
  Force,
  Wait,
}

/// Force requires an elapsed request window plus evidence the host is gone: either a stale lease
/// timestamp, or a heartbeat value that stopped advancing for longer than the stale threshold (the
/// clock-skew-immune signal for a dead holder whose timestamp still looks fresh). The window matters
/// because the lease can go stale while the host is mid-checkpoint inside [`demote_to_slave`], and
/// forcing without it risks opening the databases while the host is still closing them.
pub(super) fn take_over_poll_action(
  lease: Option<&store::share_meta::Lease>,
  requested_at: DateTime<Utc>,
  now: DateTime<Utc>,
  heartbeat_unchanged_for: Option<std::time::Duration>,
) -> TakeoverPollAction {
  let holder_gone = |lease: &store::share_meta::Lease| {
    lease.is_stale(store::lease::STALE_THRESHOLD, now)
      || heartbeat_unchanged_for.is_some_and(|elapsed| elapsed > store::lease::STALE_THRESHOLD)
  };
  match lease {
    None => TakeoverPollAction::Claim,
    Some(lease) if holder_gone(lease) && request_window_elapsed(requested_at, now) => TakeoverPollAction::Force,
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
  let lease = session.read_lease();
  let unchanged_for = app.holder_watch.observe(lease.as_ref(), std::time::Instant::now());
  match take_over_poll_action(lease.as_ref(), requested_at, Utc::now(), unchanged_for) {
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
    tracing::warn!(target: "pod::lifecycle", force, "take-over skipped; the store is already mid-transition");
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
  let lease = session.read_lease();
  app.holder_watch.observe(lease.as_ref(), std::time::Instant::now());
  let live_host = lease.is_some_and(|lease| {
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
      app.holder_watch.clear();
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_support::*;

  mod holder_watch {
    use std::time::{Duration, Instant};

    use pretty_assertions::assert_eq;

    use super::*;

    fn lease(machine_id: &str, heartbeat: DateTime<Utc>) -> store::share_meta::Lease {
      store::share_meta::Lease {
        db_generation: 0,
        heartbeat,
        hostname: format!("host-{machine_id}"),
        machine_id: machine_id.to_owned(),
        pid: 99,
      }
    }

    #[test]
    fn it_accumulates_while_the_same_heartbeat_persists() {
      let mut watch = HolderWatch::default();
      let heartbeat = Utc::now();
      let start = Instant::now();

      assert_eq!(
        watch.observe(Some(&lease("machine-b", heartbeat)), start),
        Some(Duration::ZERO)
      );
      assert_eq!(
        watch.observe(Some(&lease("machine-b", heartbeat)), start + Duration::from_secs(45)),
        Some(Duration::from_secs(45))
      );
    }

    #[test]
    fn it_resets_when_the_heartbeat_advances() {
      let mut watch = HolderWatch::default();
      let heartbeat = Utc::now();
      let start = Instant::now();
      watch.observe(Some(&lease("machine-b", heartbeat)), start);

      let unchanged = watch.observe(
        Some(&lease("machine-b", heartbeat + chrono::Duration::seconds(10))),
        start + Duration::from_secs(45),
      );

      assert_eq!(unchanged, Some(Duration::ZERO));
    }

    #[test]
    fn it_resets_when_the_holder_changes() {
      let mut watch = HolderWatch::default();
      let heartbeat = Utc::now();
      let start = Instant::now();
      watch.observe(Some(&lease("machine-b", heartbeat)), start);

      let unchanged = watch.observe(Some(&lease("machine-c", heartbeat)), start + Duration::from_secs(45));

      assert_eq!(unchanged, Some(Duration::ZERO));
    }

    #[test]
    fn it_clears_when_the_lease_disappears() {
      let mut watch = HolderWatch::default();
      let heartbeat = Utc::now();
      let start = Instant::now();
      watch.observe(Some(&lease("machine-b", heartbeat)), start);

      assert_eq!(watch.observe(None, start + Duration::from_secs(45)), None);
      assert_eq!(
        watch.observe(Some(&lease("machine-b", heartbeat)), start + Duration::from_secs(50)),
        Some(Duration::ZERO),
        "a reappearing lease starts a fresh observation window"
      );
    }
  }

  mod holder_heartbeat_flatlined {
    use std::time::Duration;

    use super::*;

    #[test]
    fn it_holds_off_within_three_stale_thresholds() {
      assert!(!holder_heartbeat_flatlined(None));
      assert!(!holder_heartbeat_flatlined(Some(store::lease::STALE_THRESHOLD * 3)));
    }

    #[test]
    fn it_trips_past_three_stale_thresholds() {
      assert!(holder_heartbeat_flatlined(Some(
        store::lease::STALE_THRESHOLD * 3 + Duration::from_secs(1)
      )));
    }
  }

  mod handle_lease_heartbeat_checked {
    use super::*;

    async fn parked_store_ready() -> StoreReady {
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

    #[tokio::test]
    async fn it_demotes_the_holder_when_another_machine_claimed_the_lease() {
      let (_dir, session) = temp_sync_session();
      let mut app = test_app();
      app.sync_session = Some(session);
      app.read_only = None;
      app.store_ready = Some(parked_store_ready().await);
      let holder = HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-other".to_owned(),
      };

      let _ = handle_lease_heartbeat_checked(&mut app, Some(holder));

      assert!(
        app.store_ready.is_none(),
        "a reported displacement starts the demotion instead of clobbering the new holder"
      );
    }

    #[tokio::test]
    async fn it_is_a_no_op_when_the_heartbeat_confirmed_our_claim() {
      let (_dir, session) = temp_sync_session();
      let mut app = test_app();
      app.sync_session = Some(session);
      app.read_only = None;
      app.store_ready = Some(parked_store_ready().await);

      let _ = handle_lease_heartbeat_checked(&mut app, None);

      assert!(app.store_ready.is_some());
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
      sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pools.interactive.writer())
        .await
        .unwrap();
    }

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
}
