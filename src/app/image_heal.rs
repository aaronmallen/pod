use super::*;

fn route_stale_images(app: &App) -> Vec<(store::images::ImageKind, i64)> {
  match app.route {
    Route::Assets => app.assets.as_ref().map(assets::State::stale_images).unwrap_or_default(),
    Route::Calendar => app
      .calendar
      .as_ref()
      .map(calendar::State::stale_images)
      .unwrap_or_default(),
    Route::CaptainsLog => app
      .captains_log
      .as_ref()
      .map(captains_log::State::stale_images)
      .unwrap_or_default(),
    Route::CharacterDetail(_) => app
      .character_detail
      .as_ref()
      .map(character_detail::State::stale_images)
      .unwrap_or_default(),
    Route::ContactSync => app
      .contact_sync
      .as_ref()
      .map(contact_sync::State::stale_images)
      .unwrap_or_default(),
    Route::Roster => app.roster.as_ref().map(roster::State::stale_images).unwrap_or_default(),
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
    Route::Market | Route::Settings => Vec::new(),
    Route::StructureAlerts => app
      .structure_alerts
      .as_ref()
      .map(structure_alerts::State::stale_images)
      .unwrap_or_default(),
    Route::Skills(_) => app.skills.as_ref().map(skills::State::stale_images).unwrap_or_default(),
    Route::Wallet => app.wallet.as_ref().map(wallet::State::stale_images).unwrap_or_default(),
  }
}

pub(super) fn collect_stale_images(app: &App) -> Vec<(store::images::ImageKind, i64)> {
  let mut keys = route_stale_images(app);
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

pub(super) fn dispatch_image_fetches(app: &mut App, keys: Vec<(store::images::ImageKind, i64)>) -> Task<Message> {
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

pub(super) fn handle_image_ready(app: &mut App, kind: store::images::ImageKind, id: i64, ready: bool) -> Task<Message> {
  app.pending_images.remove(&(kind, id));
  if ready { image_reload(app) } else { Task::none() }
}

fn route_reload_task(app: &App, runtime: &Runtime) -> Option<Task<Message>> {
  primary_route_reload(app, runtime).or_else(|| secondary_route_reload(app, runtime))
}

fn primary_route_reload(app: &App, runtime: &Runtime) -> Option<Task<Message>> {
  match app.route {
    Route::Assets => app
      .assets
      .as_ref()
      .map(|_| assets::load(&runtime.db).map(Message::Assets)),
    Route::Calendar => app
      .calendar
      .as_ref()
      .map(|state| calendar::reload(&runtime.db, state.active(), *runtime.settings.features()).map(Message::Calendar)),
    Route::CaptainsLog => app
      .captains_log
      .as_ref()
      .map(|_| captains_log::load(&runtime.db, owned_character_ids(app)).map(Message::CaptainsLog)),
    Route::CharacterDetail(_) => app.character_detail.as_ref().map(|detail| {
      let owned = owned_pilot_ids(app);
      character_detail::load(&runtime.db, detail.active(), owned).map(Message::CharacterDetail)
    }),
    Route::ContactSync => app
      .contact_sync
      .as_ref()
      .map(|_| contact_sync::load(&runtime.db, Arc::clone(&runtime.esi)).map(Message::ContactSync)),
    Route::Roster => app
      .roster
      .as_ref()
      .map(|_| roster::load(&runtime.db, feature_flags(app)).map(Message::Roster)),
    _ => None,
  }
}

fn secondary_route_reload(app: &App, runtime: &Runtime) -> Option<Task<Message>> {
  match app.route {
    Route::CorporationDetail(_) => app
      .corporation_detail
      .as_ref()
      .map(|detail| corporation_detail::load(&runtime.db, detail.active()).map(Message::CorporationDetail)),
    Route::Industry => app
      .industry
      .as_ref()
      .map(|state| industry::reload(&runtime.db, state.active(), &industry_required_scopes()).map(Message::Industry)),
    Route::Mail => app.mail.as_ref().map(|state| {
      let mail::Scope::Character(id) = state.active();
      mail::load(&runtime.db, id).map(Message::Mail)
    }),
    Route::Settings => None,
    Route::Skills(_) => app.skills.as_ref().map(|skills| {
      let owned = owned_pilot_ids(app);
      skills::load(&runtime.db, skills.active(), owned).map(Message::Skills)
    }),
    Route::Wallet => app
      .wallet
      .as_ref()
      .map(|_| wallet::load(&runtime.db).map(Message::Wallet)),
    _ => None,
  }
}

pub(super) fn image_reload(app: &App) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let mut tasks = Vec::new();
  if let Some(task) = route_reload_task(app, runtime) {
    tasks.push(task);
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_support::*;

  mod collect_stale_images {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_appends_the_compare_window_keys_when_one_is_open() {
      let mut app = featured_app();
      app.route = Route::Settings;
      app.compare = Some((window::Id::unique(), skills_compare::State::new(vec![1, 2], Vec::new())));

      assert_eq!(crate::app::collect_stale_images(&app), Vec::new());
    }

    #[test]
    fn it_gathers_keys_for_every_active_route() {
      let mut app = featured_app();

      for route in [
        Route::Assets,
        Route::Calendar,
        Route::CaptainsLog,
        Route::CharacterDetail(1),
        Route::ContactSync,
        Route::Roster,
        Route::CorporationDetail(1),
        Route::Industry,
        Route::Mail,
        Route::Market,
        Route::Settings,
        Route::StructureAlerts,
        Route::Skills(1),
        Route::Wallet,
      ] {
        app.route = route;
        let _ = crate::app::collect_stale_images(&app);
      }
    }

    #[test]
    fn it_gathers_no_keys_for_settings_with_no_compare() {
      let mut app = featured_app();
      app.route = Route::Settings;

      assert_eq!(crate::app::collect_stale_images(&app), Vec::new());
    }
  }

  mod image_reload {
    use super::*;

    #[tokio::test]
    async fn it_batches_a_reload_for_each_active_route() {
      let mut app = featured_app();
      app.captains_log = Some(captains_log::State::new());
      app.contact_sync = Some(contact_sync::State::new());
      app.corporation_detail = Some(corporation_detail::State::new(1));
      app.runtime = Some(test_runtime().await);

      for route in [
        Route::Assets,
        Route::Calendar,
        Route::CaptainsLog,
        Route::CharacterDetail(1),
        Route::ContactSync,
        Route::Roster,
        Route::CorporationDetail(1),
        Route::Industry,
        Route::Mail,
        Route::Settings,
        Route::Skills(1),
        Route::Wallet,
      ] {
        app.route = route;
        let _ = crate::app::image_reload(&app);
      }
    }

    #[tokio::test]
    async fn it_is_a_no_op_without_a_runtime() {
      let app = featured_app();

      let _ = crate::app::image_reload(&app);
    }

    #[tokio::test]
    async fn it_reloads_the_compare_window_when_one_is_open() {
      let mut app = featured_app();
      app.runtime = Some(test_runtime().await);
      app.compare = Some((window::Id::unique(), skills_compare::State::new(vec![1, 2], Vec::new())));

      let _ = crate::app::image_reload(&app);
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
}
