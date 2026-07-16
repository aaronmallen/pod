use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LanguageChangeAction {
  Ignore,
  Relaunch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SyncNowOutcome {
  Failed,
  Reconciled { mark: Option<SystemTime>, pulled: bool },
}

pub(super) fn handle_settings(app: &mut App, msg: settings::Message) -> Task<Message> {
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

  match outcome {
    settings::Outcome::AccessibilityChanged => {
      let accessibility = *state.settings().accessibility();
      return apply_accessibility_outcome(app, accessibility, task);
    }
    settings::Outcome::UiChanged => {
      let ui = state.settings().ui().clone();
      return apply_ui_outcome(app, ui, task);
    }
    settings::Outcome::McpChanged => {
      let mcp = state.settings().mcp().clone();
      return apply_mcp_outcome(app, mcp, task);
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
    settings::Outcome::ExportData => return export_data_outcome(state.settings(), task),
    settings::Outcome::ExportIntel {
      facilities,
    } => {
      return Task::batch(vec![task, export_intel(facilities)]);
    }
    settings::Outcome::ImportData {
      path,
    } => {
      let storage = state.settings().storage().clone();
      let local_settings = state.settings().clone();
      let machine_id = storage.machine_id().clone().unwrap_or_default();
      return Task::batch(vec![task, import_data(path, storage, machine_id, local_settings)]);
    }
    settings::Outcome::ImportIntel {
      facilities,
    } => {
      return Task::batch(vec![task, import_intel(app, facilities)]);
    }
    settings::Outcome::SetLogLevel(level) => {
      apply_log_level(level);
      return task;
    }
    settings::Outcome::IndustrySearch {
      activity,
      generation,
      query,
    } => {
      return Task::batch(vec![task, settings_facility_search(app, activity, generation, query)]);
    }
    settings::Outcome::LanguageChanged(language) => {
      return apply_language_change(app, language, task);
    }
    settings::Outcome::TagsChanged => return reload_roster_after_tag_change(app, task),
    _ => {}
  }

  if !features_changed {
    return task;
  }
  let updated = state.settings().clone();
  propagate_feature_change(app, updated, task)
}

fn apply_accessibility_outcome(
  app: &mut App,
  accessibility: config::AccessibilityConfig,
  task: Task<Message>,
) -> Task<Message> {
  app.accessibility = accessibility;
  if let Some(runtime) = app.runtime.as_mut() {
    *runtime.settings.accessibility_mut() = accessibility;
  }
  color::set_high_contrast(*accessibility.high_contrast());
  Task::batch(vec![task, refresh_all_windows(app)])
}

fn apply_ui_outcome(app: &mut App, ui: config::UiConfig, task: Task<Message>) -> Task<Message> {
  color::set_accent(ui.accent());
  if let Some(runtime) = app.runtime.as_mut() {
    *runtime.settings.ui_mut() = ui;
  }
  Task::batch(vec![task, refresh_all_windows(app)])
}

fn apply_mcp_outcome(app: &mut App, mcp: config::McpConfig, task: Task<Message>) -> Task<Message> {
  if let Some(runtime) = app.runtime.as_mut() {
    *runtime.settings.mcp_mut() = mcp;
  }
  sync_mcp_server(app);
  task
}

fn export_data_outcome(settings: &config::Settings, task: Task<Message>) -> Task<Message> {
  let storage = settings.storage();
  let diagnostics = settings::log_export::Diagnostics {
    cache_dir: storage.resolved_cache_dir(),
    database_path: storage.resolved_database_path(),
    db_dir: storage.resolved_db_dir(),
    log_dir: storage.resolved_log_dir(),
  };
  let database_path = storage.resolved_database_path();
  let config_bytes = match toml::to_string_pretty(settings) {
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
  Task::batch(vec![task, export_data(database_path, config_bytes, diagnostics)])
}

// A tag write landed in Settings; reload the roster so its cached tag list (the add-tag
// modal's choices and the card chips) reflects the change without a restart.
fn reload_roster_after_tag_change(app: &mut App, task: Task<Message>) -> Task<Message> {
  if app.roster.is_some()
    && let Some(runtime) = app.runtime.as_ref()
  {
    let flags = *runtime.settings.features();
    return Task::batch(vec![task, roster::load(&runtime.db, flags).map(Message::Roster)]);
  }
  task
}

// Applies a confirmed language change. Unlike scale and high-contrast, which apply live through
// AccessibilityChanged + refresh_all_windows, a language change is committed at the next boot: the new
// language is already persisted to config by `settings::update`, so relaunching lets the splash
// re-seed the SDE (the language is folded into composite_version) and the boot-time hook expire the
// language-dependent jobs, bringing the app up fully in the new language. See ADR-0041 section 6.
pub(super) fn apply_language_change(
  app: &mut App,
  language: crate::services::i18n::Language,
  task: Task<Message>,
) -> Task<Message> {
  match language_change_action(app, language) {
    LanguageChangeAction::Relaunch => {
      tracing::info!(target: "pod::lifecycle", %language, "language change confirmed; relaunching to re-seed and re-sync");
      restart();
      task
    }
    LanguageChangeAction::Ignore => task,
  }
}

pub(super) fn language_change_action(app: &App, language: crate::services::i18n::Language) -> LanguageChangeAction {
  if app.accessibility.language() == language {
    LanguageChangeAction::Ignore
  } else {
    LanguageChangeAction::Relaunch
  }
}

pub(super) fn propagate_feature_change(
  app: &mut App,
  updated: crate::config::Settings,
  base: Task<Message>,
) -> Task<Message> {
  let Some(runtime) = app.runtime.as_mut() else {
    return base;
  };
  runtime.settings = updated;
  let enabled = runtime.settings.features().enabled();
  let flags = *runtime.settings.features();
  let db = runtime.db.clone();
  runtime.sync.set_features(flags);
  let mut tasks = vec![base, roster::load(&db, flags).map(Message::Roster)];

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

  if registry::feature_for_destination(route.destination()).is_some_and(|feature| !enabled.contains(&feature)) {
    navigate(app, Route::Roster);
  }

  if route == Route::ContactSync && !flags.is_sub_enabled(config::SubFeature::Contacts) {
    navigate(app, Route::Roster);
  }

  if route == Route::StructureAlerts && !structure_alerts_reachable(app) {
    navigate(app, Route::Roster);
  }

  Task::batch(tasks)
}

pub(super) fn settings_facility_search(app: &App, activity: i64, generation: u64, query: String) -> Task<Message> {
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
      Message::Settings(settings::Message::Facility(
        settings::facility_tab::Message::SearchResults {
          activity,
          generation,
          results,
        },
      ))
    },
  )
}

// The resolved color functions are read inside each window's `view` closure, which only re-runs
// when that window redraws. Unlike `scale_factor`, iced does not re-read them every frame, so after
// the high-contrast flag flips we issue a benign per-window action (querying size and discarding it)
// to schedule a fresh draw of every open window, applying the new palette live without a restart.
pub(super) fn refresh_all_windows(app: &App) -> Task<Message> {
  Task::batch(app.windows.ids().map(|id| window::size(id).discard()))
}

pub(super) fn sync_now(app: &mut App) -> Task<Message> {
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

pub(super) fn handle_sync_now_resolved(app: &mut App, outcome: SyncNowOutcome) -> Task<Message> {
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

pub(super) fn export_logs(
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

pub(super) async fn export_log_bundle(
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

pub(super) async fn save_log_bundle(
  default_name: String,
  bytes: Vec<u8>,
) -> Result<Option<std::path::PathBuf>, String> {
  #[cfg(not(test))]
  {
    let Some(handle) = rfd::AsyncFileDialog::new()
      .set_title(t!("settings.storage.export_logs").into_owned())
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

pub(super) fn export_data(
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

pub(super) async fn export_data_archive(
  database_path: std::path::PathBuf,
  config_bytes: Vec<u8>,
  diagnostics: settings::log_export::Diagnostics,
) -> Result<Option<std::path::PathBuf>, String> {
  let default_name = settings::data_export::default_file_name(Utc::now());

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

  drop(staging);

  save_data_archive(default_name, bytes).await
}

pub(super) async fn save_data_archive(
  default_name: String,
  bytes: Vec<u8>,
) -> Result<Option<std::path::PathBuf>, String> {
  #[cfg(not(test))]
  {
    let Some(handle) = rfd::AsyncFileDialog::new()
      .set_title(t!("settings.storage.export_data").into_owned())
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

pub(super) fn export_intel(facilities: Vec<settings::facility_intel_share::PortableFacility>) -> Task<Message> {
  let pack = settings::facility_intel_share::build_pack(facilities);
  match settings::facility_intel_share::encode_pack(&pack) {
    Ok(contents) => Task::future(save_intel_pack(contents)).discard(),
    Err(_) => Task::none(),
  }
}

pub(super) async fn save_intel_pack(contents: String) -> Option<std::path::PathBuf> {
  #[cfg(not(test))]
  {
    let filter = t!("settings.facility.export_file_filter");
    let handle = rfd::AsyncFileDialog::new()
      .set_title(t!("settings.facility.export_dialog_title").into_owned())
      .set_file_name(settings::facility_intel_share::FILE_NAME)
      .add_filter(&*filter, &[settings::facility_intel_share::PACK_EXTENSION])
      .save_file()
      .await?;
    let path = handle.path().to_path_buf();
    std::fs::write(&path, contents).ok()?;
    Some(path)
  }
  #[cfg(test)]
  {
    let _ = contents;
    None
  }
}

pub(super) fn import_intel(
  app: &App,
  facilities: Vec<settings::facility_intel_share::PortableFacility>,
) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    let summary = settings::facility_intel_import::skipped_summary(&facilities);
    return Task::done(intel_import_finished(summary));
  };
  let db = runtime.db.clone();
  Task::perform(
    settings::facility_intel_import::import_facilities(db, facilities),
    intel_import_finished,
  )
}

fn intel_import_finished(summary: settings::facility_intel_import::ImportSummary) -> Message {
  Message::Settings(settings::Message::Facility(
    settings::facility_tab::Message::ImportFinished(summary),
  ))
}

pub(super) fn import_data(
  path: std::path::PathBuf,
  storage: config::StorageConfig,
  machine_id: String,
  local_settings: config::Settings,
) -> Task<Message> {
  Task::perform(
    import_data_archive(path, storage, machine_id, local_settings),
    |result| match result {
      Ok(()) => Message::Quit,
      Err(error) => Message::Settings(settings::Message::Storage(
        settings::storage_tab::Message::DataImportFinished(Err(error)),
      )),
    },
  )
}

pub(super) async fn import_data_archive(
  path: std::path::PathBuf,
  storage: config::StorageConfig,
  machine_id: String,
  local_settings: config::Settings,
) -> Result<(), String> {
  let bytes = tokio::fs::read(&path)
    .await
    .map_err(|err| format!("Couldn't read {}: {err}", path.display()))?;

  let parsed = tokio::task::spawn_blocking(move || settings::data_export::read_archive(&bytes))
    .await
    .map_err(|err| err.to_string())??;
  if parsed.verdict == settings::data_export::VersionVerdict::Incompatible {
    return Err(format!(
      "This archive was made by a newer Pod ({}); it can't be restored into this build.",
      parsed.manifest.pod_version
    ));
  }

  let staging = tempfile::Builder::new()
    .prefix("pod-import-")
    .suffix(".db")
    .tempfile()
    .map_err(|err| format!("Couldn't create import staging file: {err}"))?;
  let temp_db = staging.path().to_path_buf();
  tokio::fs::write(&temp_db, &parsed.database)
    .await
    .map_err(|err| format!("Couldn't stage the archived database: {err}"))?;

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

  drop(staging);

  let config_text =
    String::from_utf8(parsed.config).map_err(|err| format!("The archived settings aren't valid UTF-8: {err}"))?;
  let archived: config::Settings =
    toml::from_str(&config_text).map_err(|err| format!("Couldn't parse the archived settings: {err}"))?;
  let merged = config::merge_for_restore(&local_settings, &archived);
  config::save(&merged);

  Ok(())
}

pub(super) fn migrate_storage(previous: config::StorageConfig, next: config::StorageConfig) -> Task<Message> {
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

pub(super) fn refresh_storage_status(app: &mut App) {
  let holder = app.read_only.as_ref().map(|holder| holder.hostname.clone());
  let last_synced = app.last_synced;
  if let Some(settings) = app.settings.as_mut() {
    settings.set_sync_status(holder, last_synced);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_support::*;

  mod language_change_action {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::services::i18n::Language;

    #[test]
    fn it_relaunches_when_the_language_actually_changes() {
      let mut app = test_app();
      app.accessibility.set_language(Language::En);

      assert_eq!(
        language_change_action(&app, Language::Fr),
        LanguageChangeAction::Relaunch
      );
    }

    #[test]
    fn it_ignores_a_confirmed_change_to_the_running_language() {
      let mut app = test_app();
      app.accessibility.set_language(Language::De);

      assert_eq!(language_change_action(&app, Language::De), LanguageChangeAction::Ignore);
    }
  }
}
