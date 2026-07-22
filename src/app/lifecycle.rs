use super::*;

pub(super) fn shutdown_if_last_window(app: &mut App) -> Task<Message> {
  if app.windows.is_empty() {
    shutdown(app)
  } else {
    Task::none()
  }
}

pub(super) fn shutdown(app: &mut App) -> Task<Message> {
  tracing::info!(target: "pod::lifecycle", "shutting down");
  let save_draft = save_open_compose(app);
  let checkpoint = shutdown_storage(app);
  let flush = flush_telemetry_on_exit(app);
  stop_engines(app);
  save_draft
    .chain(Task::batch([checkpoint, flush]))
    .chain(Task::batch([iced::exit(), exit_process()]))
}

pub(super) fn handle_telemetry_flush_tick(app: &App) -> Task<Message> {
  if let Some(sender) = app.telemetry.as_ref() {
    telemetry::collector::flush(sender);
  }
  Task::none()
}

// The exit flush rides the shutdown Task chain and is awaited (bounded inside
// flush_and_wait), so the final batch is delivered before the exit_process()
// backstop can truncate a detached send.
pub(super) fn flush_telemetry_on_exit(app: &App) -> Task<Message> {
  match app.telemetry.as_ref() {
    Some(sender) => Task::future(telemetry::collector::flush_and_wait(sender.clone())).discard(),
    None => Task::none(),
  }
}

/// Flushes every open, non-empty compose window to Drafts before the storage checkpoint, so any draft
/// in flight at quit survives to the next launch. Runs before the checkpoint so the persisted rows are
/// included in the pushed working copy.
pub(super) fn save_open_compose(app: &App) -> Task<Message> {
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

pub(super) fn stop_engines(app: &App) {
  if let Some(runtime) = app.runtime.as_ref() {
    runtime.sync.shutdown();
  }
  if let Some(updater) = app.updater.as_ref() {
    updater.shutdown();
  }
}

/// Hard backstop that guarantees the process exits even if a tokio task refuses to drain after
/// `iced::exit()`. Fires only after the storage checkpoint completes (it is chained after it).
pub(super) fn exit_process() -> Task<Message> {
  Task::future(async {
    std::process::exit(0);
  })
  .discard()
}

pub(super) fn shutdown_storage(app: &mut App) -> Task<Message> {
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

pub(super) fn release_lock(app: &App) -> Task<Message> {
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
