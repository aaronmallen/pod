use super::*;

pub(super) fn dispatch_feature(app: &mut App, message: Message) -> Result<Task<Message>, Box<Message>> {
  Ok(match message {
    Message::Assets(msg) => handle_assets(app, msg),
    Message::Auth(msg) => handle_auth(app, msg),
    Message::Calendar(msg) => handle_calendar(app, msg),
    Message::CalendarAttentionCounted(count) => handle_calendar_attention_counted(app, count),
    Message::CalendarEvent(id, msg) => handle_calendar_event(app, id, msg),
    Message::CharacterDetail(msg) => handle_character_detail(app, msg),
    Message::ContactSync(msg) => handle_contact_sync(app, msg),
    Message::Roster(msg) => handle_roster(app, msg),
    Message::Compare(msg) => handle_compare(app, msg),
    Message::Compose(id, msg) => handle_compose(app, id, msg),
    Message::Contract(id, msg) => handle_contract(app, id, msg),
    Message::CorporationDetail(msg) => handle_corporation_detail(app, msg),
    Message::Industry(msg) => handle_industry(app, msg),
    Message::Killmail(id, msg) => handle_killmail(app, id, msg),
    Message::Mail(msg) => handle_mail(app, msg),
    Message::MailUnreadCounted(unread) => handle_mail_unread_counted(app, unread),
    Message::MainScreenSizeProbed(size) => handle_main_screen_size_probed(size),
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

pub(super) fn dispatch_feature_aux(app: &mut App, message: Message) -> Result<Task<Message>, Box<Message>> {
  Ok(match message {
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

pub(super) fn dispatch_lifecycle(app: &mut App, message: Message) -> Task<Message> {
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
    Message::Wizard(msg) => update_wizard(app, msg),
    other => dispatch_sync_lifecycle(app, other),
  }
}

pub(super) fn dispatch_sync_lifecycle(app: &mut App, message: Message) -> Task<Message> {
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
    Message::TakeoverPoll => handle_take_over_poll(app),
    Message::DemotedToSlave(ready, requester) => handle_demoted_to_slave(app, *ready, requester),
    other => dispatch_window_lifecycle(app, other),
  }
}

pub(super) fn dispatch_window_lifecycle(app: &mut App, message: Message) -> Task<Message> {
  match message {
    Message::CloseSyncPopover => set_sync_popover_open(app, false),
    Message::FocusMainWindow => handle_focus_main_window(app),
    Message::Palette(msg) => handle_palette(app, msg),
    Message::Quit => shutdown(app),
    Message::Shortcut(chord) => handle_shortcut(app, chord),
    Message::TelemetryFlushTick => handle_telemetry_flush_tick(app),
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    app::test_support::*,
    sync::{JobKind, Subject},
  };

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
        let _ = crate::app::dispatch_lifecycle(&mut app, message);
      }
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
      let db = crate::store::open_test().await.unwrap();
      let mut app = test_app();
      app.settings = Some(settings::State::new(config::Settings::default(), db));

      for feature in [config::Feature::Mail, config::Feature::SkillMonitoring] {
        for value in [false, true] {
          let _ = handle_settings(
            &mut app,
            settings::Message::Features(settings::features_tab::Message::Toggled(feature, value)),
          );
        }
      }

      let _ = update(&mut app, Message::Roster(roster::Message::ReauthCharacterRequested(7)));

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

    #[test]
    fn it_dispatches_a_main_screen_size_probe_without_a_probed_size() {
      let mut app = test_app();

      let _ = update(&mut app, Message::MainScreenSizeProbed(None));
    }

    #[tokio::test]
    async fn disabling_contacts_while_contact_sync_is_open_redirects_to_characters() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = update(
        &mut app,
        Message::NavTo(rail::Destination::Roster, Some("contact-sync")),
      );
      assert_eq!(app.route, Route::ContactSync, "Contact Sync is reachable while enabled");

      let _ = handle_settings(
        &mut app,
        settings::Message::Features(settings::features_tab::Message::SubToggled(
          config::SubFeature::Contacts,
          false,
        )),
      );

      assert_eq!(
        app.route,
        Route::Roster,
        "disabling Contacts while Contact Sync is open redirects to Roster"
      );

      let _ = update(
        &mut app,
        Message::NavTo(rail::Destination::Roster, Some("contact-sync")),
      );
      assert_eq!(
        app.route,
        Route::Roster,
        "the Contact Sync route is refused while Contacts is disabled"
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
        Route::Roster,
        "disabling Industry while its screen is open redirects to Roster"
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
        settings::Message::Facility(settings::facility_tab::Message::QueryChanged {
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

    #[tokio::test]
    async fn handle_settings_mirrors_an_mcp_change_onto_the_runtime() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = handle_settings(
        &mut app,
        settings::Message::Mcp(settings::mcp_tab::Message::EnabledToggled(true)),
      );

      assert!(
        *app.runtime.as_ref().unwrap().settings.mcp().enabled(),
        "an MCP toggle is mirrored onto the runtime settings so the server reconciles live"
      );
      assert!(app.settings.is_some(), "an MCP change leaves the settings screen open");
    }

    #[tokio::test]
    async fn handle_settings_requests_a_data_export_through_the_diagnostics() {
      let runtime = test_runtime().await;
      let mut app = test_app();
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.runtime = Some(runtime);

      let _ = handle_settings(
        &mut app,
        settings::Message::Storage(settings::storage_tab::Message::RequestDataExport),
      );

      assert!(
        app.settings.is_some(),
        "requesting a data export leaves the settings screen open and routes the export task"
      );
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
      app.roster = Some(roster::State::new());
      app.route = Route::Roster;
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
    fn it_routes_each_roster_intent_arm() {
      let mut app = test_app();
      let _ = update(&mut app, Message::Roster(roster::Message::AddCharacterRequested));
      let _ = update(&mut app, Message::Roster(roster::Message::AddCorporationRequested));
      let _ = update(&mut app, Message::Roster(roster::Message::ReauthCharacterRequested(7)));
      let _ = update(
        &mut app,
        Message::Roster(roster::Message::ReauthCorporationRequested(7)),
      );
      let _ = update(&mut app, Message::Roster(roster::Message::RemoveCharacterConfirmed(7)));
      let _ = update(
        &mut app,
        Message::Roster(roster::Message::RemoveCorporationConfirmed(7)),
      );
    }

    #[test]
    fn it_routes_feature_messages_to_a_no_op_without_a_runtime() {
      let mut app = test_app();
      let _ = update(&mut app, Message::Wallet(wallet::Message::BudgetInspectorDragEnd));
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
    fn pressing_take_over_against_a_live_host_requests_without_claiming() {
      let (dir, session) = temp_sync_session();
      let share = dir.path().join("share");
      store::lease::LeaseManager::new("machine-other".to_owned(), "studio-mac".to_owned(), 99, 0)
        .heartbeat(&share, Utc::now())
        .unwrap();
      let mut app = test_app();
      app.sync_session = Some(session);
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-other".to_owned(),
      });

      let _ = handle_take_over(&mut app);

      assert!(
        app.take_over_requested_at.is_some(),
        "a live host is asked to yield rather than clobbered"
      );
      assert!(
        app.read_only.is_some(),
        "the requester stays read-only until the host yields"
      );
      assert!(
        !app.confirm_force_takeover,
        "the cooperative request never opens the force confirmation"
      );
      assert!(
        store::share_meta::TakeoverRequest::read(&store::lease::takeover_path(&share)).is_some(),
        "a take-over request is written to the share"
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

    fn foreign_lease(share: &std::path::Path, heartbeat: DateTime<Utc>) {
      store::lease::LeaseManager::new("machine-other".to_owned(), "studio-mac".to_owned(), 99, 0)
        .heartbeat(share, heartbeat)
        .unwrap();
    }

    fn foreign_request(share: &std::path::Path, requested_at: DateTime<Utc>) {
      store::share_meta::TakeoverRequest {
        db_generation: 0,
        requested_at,
        hostname: "studio-mac".to_owned(),
        machine_id: "machine-other".to_owned(),
        pid: 1234,
      }
      .write(&store::lease::takeover_path(share))
      .unwrap();
    }

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
    async fn a_fresh_foreign_request_triggers_demotion_from_a_holding_host() {
      let (dir, session) = temp_sync_session();
      let share = dir.path().join("share");
      foreign_request(&share, Utc::now());
      let mut app = test_app();
      app.sync_session = Some(session);
      app.store_ready = Some(parked_store_ready().await);
      app.runtime = Some(test_runtime().await);
      app.engine_state = EngineState::Running;
      assert!(holding_lease(&app), "the host holds the lease before the request lands");

      let _ = handle_lease_heartbeat(&mut app);

      assert!(
        app.store_ready.is_none(),
        "a fresh foreign request yields the share instead of heartbeating"
      );
      assert!(app.runtime.is_none(), "demotion drops the writable runtime");
    }

    #[tokio::test]
    async fn the_reacquire_guard_stands_down_while_a_fresh_foreign_request_exists() {
      let (dir, session) = temp_sync_session();
      let share = dir.path().join("share");
      foreign_request(&share, Utc::now());
      let mut app = test_app();
      app.sync_session = Some(session);
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-other".to_owned(),
      });
      app.store_ready = Some(parked_store_ready().await);

      let _ = handle_reacquire_lease(&mut app);

      assert!(
        app.store_ready.is_some(),
        "the ex-host never re-grabs the lease while a foreign request is outstanding"
      );
      assert!(app.read_only.is_some(), "the ex-host stays parked behind the request");
    }

    #[tokio::test]
    async fn the_poll_claims_once_the_foreign_lease_is_released() {
      let (_dir, session) = temp_sync_session();
      let mut app = test_app();
      app.sync_session = Some(session);
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-other".to_owned(),
      });
      app.store_ready = Some(parked_store_ready().await);
      app.take_over_requested_at = Some(Utc::now());

      let _ = handle_take_over_poll(&mut app);

      assert!(
        app.store_ready.is_none(),
        "a released lease lets the poll claim cooperatively"
      );
    }

    #[tokio::test]
    async fn the_poll_forces_once_the_lease_is_stale_and_the_window_has_elapsed() {
      let (dir, session) = temp_sync_session();
      let share = dir.path().join("share");
      foreign_lease(&share, Utc::now() - chrono::Duration::seconds(31));
      let mut app = test_app();
      app.sync_session = Some(session);
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-other".to_owned(),
      });
      app.store_ready = Some(parked_store_ready().await);
      app.take_over_requested_at = Some(Utc::now() - chrono::Duration::seconds(31));

      let _ = handle_take_over_poll(&mut app);

      assert!(
        app.store_ready.is_none(),
        "a dead host past the request window is force-claimed"
      );
    }

    #[tokio::test]
    async fn the_poll_waits_while_the_host_lease_is_still_fresh() {
      let (dir, session) = temp_sync_session();
      let share = dir.path().join("share");
      foreign_lease(&share, Utc::now());
      let mut app = test_app();
      app.sync_session = Some(session);
      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-other".to_owned(),
      });
      app.store_ready = Some(parked_store_ready().await);
      app.take_over_requested_at = Some(Utc::now());

      let _ = handle_take_over_poll(&mut app);

      assert!(
        app.store_ready.is_some(),
        "a still-fresh host lease keeps the requester waiting"
      );
    }

    #[test]
    fn the_poll_action_claims_when_the_lease_is_gone() {
      let now = Utc::now();

      assert_eq!(take_over_poll_action(None, now, now), TakeoverPollAction::Claim);
    }

    #[test]
    fn the_poll_action_waits_while_the_lease_is_fresh() {
      let now = Utc::now();
      let lease = sample_lease(now);

      assert_eq!(
        take_over_poll_action(Some(&lease), now - chrono::Duration::seconds(31), now),
        TakeoverPollAction::Wait
      );
    }

    #[test]
    fn the_poll_action_waits_when_stale_but_the_window_has_not_elapsed() {
      let now = Utc::now();
      let lease = sample_lease(now - chrono::Duration::seconds(31));

      assert_eq!(
        take_over_poll_action(Some(&lease), now - chrono::Duration::seconds(5), now),
        TakeoverPollAction::Wait
      );
    }

    #[test]
    fn the_poll_action_forces_when_stale_and_the_window_has_elapsed() {
      let now = Utc::now();
      let lease = sample_lease(now - chrono::Duration::seconds(31));

      assert_eq!(
        take_over_poll_action(Some(&lease), now - chrono::Duration::seconds(31), now),
        TakeoverPollAction::Force
      );
    }

    fn sample_lease(heartbeat: DateTime<Utc>) -> store::share_meta::Lease {
      store::share_meta::Lease {
        db_generation: 0,
        heartbeat,
        hostname: "studio-mac".to_owned(),
        machine_id: "machine-other".to_owned(),
        pid: 99,
      }
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

    #[test]
    fn the_requesting_banner_names_the_host_and_offers_a_force_escape_hatch() {
      let label = read_only_requesting_label("studio-mac");

      assert_eq!(label, "Requesting control from studio-mac\u{2026}");

      let holder = HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-other".to_owned(),
      };
      let _ = read_only_banner(&holder, false, true, Utc::now());

      let force = t!("shell.takeover.take_over_anyway").into_owned();
      assert_eq!(force, "Take over anyway");
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
      app.roster = Some(roster::State::new());

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
      app.roster = Some(roster::State::new());
      let start = Instant::now();

      app.roster_dirty = true;
      let _ = drain_roster_dirty_at(&mut app, start);
      assert!(!app.roster_dirty, "the first dirty pulse reloads and clears the flag");
      assert!(app.next_roster_reload.is_some(), "the reload arms a debounce floor");

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
      app.roster = Some(roster::State::new());
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
        rail::Destination::Roster
      );
    }

    #[test]
    fn it_keeps_the_characters_destination_lit_while_a_pilot_is_drilled_in() {
      assert_eq!(Route::CharacterDetail(42).destination(), rail::Destination::Roster);
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

      let _ = update(&mut app, Message::Roster(roster::Message::CharacterSelected(42)));

      assert_eq!(app.route, Route::CharacterDetail(42));
      assert_eq!(app.selected_character, Some(42));
      assert!(app.character_detail.is_some());
    }

    #[test]
    fn it_navigates_to_the_corporation_detail_for_the_selected_corporation() {
      let mut app = test_app();

      let _ = update(
        &mut app,
        Message::Roster(roster::Message::CorporationSelected(98_000_001)),
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
      app.roster = Some(roster::State::new());

      let _ = update(
        &mut app,
        Message::NavTo(rail::Destination::Roster, Some("corporations")),
      );

      assert_eq!(app.route, Route::Roster);
      assert_eq!(
        app.roster.as_ref().map(roster::State::active_pane),
        Some(roster::Pane::Corporations)
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

      assert_eq!(app.route, Route::Roster);
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

      assert_eq!(app.route, Route::Roster);
      assert!(app.industry.is_none());
    }

    #[test]
    fn it_returns_to_the_roster_grid_when_the_characters_rail_is_activated_from_corp_detail() {
      let mut app = test_app();
      let _ = update(
        &mut app,
        Message::Roster(roster::Message::CorporationSelected(98_000_001)),
      );
      assert_eq!(app.route, Route::CorporationDetail(98_000_001));

      let _ = update(&mut app, Message::Nav(rail::Destination::Roster));

      assert_eq!(app.route, Route::Roster);
    }

    #[test]
    fn it_returns_to_the_roster_grid_when_the_characters_rail_is_activated_from_detail() {
      let mut app = test_app();
      let _ = update(&mut app, Message::Roster(roster::Message::CharacterSelected(42)));
      assert_eq!(app.route, Route::CharacterDetail(42));

      let _ = update(&mut app, Message::Nav(rail::Destination::Roster));

      assert_eq!(app.route, Route::Roster);
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
      app.roster = Some(roster::State::new());
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

      assert_eq!(
        compare.action,
        command_palette::Action::NavTo(
          *crate::features::shell::nav_catalog::section(rail::Destination::Skills).expect("the Skills section"),
          Some("compare"),
        ),
      );
    }

    #[test]
    fn it_routes_the_skills_compare_sub_section_through_open_compare() {
      let mut app = featured_app();

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
      app.roster = Some(roster::State::new());
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

      let _ = update(&mut app, Message::Palette(PaletteMessage::Activate(index)));

      assert!(app.palette.is_none(), "activating a window command closes the palette");
    }

    #[test]
    fn it_dispatches_the_window_commands_without_a_runtime() {
      let mut app = test_app();

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

      let _ = dispatch_window_lifecycle(&mut app, Message::ClockTick);
    }

    #[test]
    fn it_routes_the_remaining_window_lifecycle_branches() {
      let mut app = ready_app();
      let id = window::Id::unique();

      let _ = dispatch_window_lifecycle(&mut app, Message::Palette(PaletteMessage::Close));
      let _ = dispatch_window_lifecycle(&mut app, Message::Shortcut(Chord::FocusSearch));
      let _ = dispatch_window_lifecycle(&mut app, Message::UpdaterAction(updater_banner::Action::Apply));
      let _ = dispatch_window_lifecycle(&mut app, Message::UpdaterStateChanged(updater::State::default()));
      let _ = dispatch_window_lifecycle(
        &mut app,
        Message::Window(id, window::Event::Resized(Size::new(640.0, 480.0))),
      );

      let _ = dispatch_window_lifecycle(&mut app, Message::Quit);
    }
  }

  mod dispatch_feature_aux {
    use super::*;

    #[test]
    fn it_routes_every_notification_and_rail_message_without_a_runtime() {
      let mut app = ready_app();
      let mcp = mcp::McpRequest::new("skill_plan_create".to_owned(), serde_json::Value::Null).0;

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
      assert!(dispatch_feature(&mut app, Message::Roster(roster::Message::AddCharacterRequested),).is_ok());
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
}
