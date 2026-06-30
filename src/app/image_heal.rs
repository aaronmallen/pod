use super::*;

pub(super) fn collect_stale_images(app: &App) -> Vec<(store::images::ImageKind, i64)> {
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

pub(super) fn image_reload(app: &App) -> Task<Message> {
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
    Route::Roster => {
      if app.roster.is_some() {
        tasks.push(roster::load(&runtime.db, feature_flags(app)).map(Message::Roster));
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
