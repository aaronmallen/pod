use super::*;

pub(super) fn navigate_to_skills(app: &mut App, target: Option<i64>, owned: Vec<i64>) -> Task<Message> {
  match target {
    Some(id) => {
      navigate(app, Route::Skills(id));
      app.selected_character = Some(id);
      app.skills = Some(skills::State::new(id).with_restored_panes(&app.ui_state));
      match app.runtime.as_ref() {
        Some(runtime) => skills::load(&runtime.db, id, owned).map(Message::Skills),
        None => Task::none(),
      }
    }
    None => {
      navigate(app, Route::Skills(EMPTY_SKILLS_SELECTION));
      app.skills = Some(skills::State::new(EMPTY_SKILLS_SELECTION).with_restored_panes(&app.ui_state));
      Task::none()
    }
  }
}

pub(super) fn navigate_to_wallet(app: &mut App) -> Task<Message> {
  navigate(app, Route::Wallet);
  app.wallet = Some(wallet::State::new(feature_flags(app)).with_restored_panes(&app.ui_state));
  match app.runtime.as_ref() {
    Some(runtime) => wallet::load(&runtime.db).map(Message::Wallet),
    None => Task::none(),
  }
}

pub(super) fn navigate_to_mail(app: &mut App, target: Option<i64>) -> Task<Message> {
  navigate(app, Route::Mail);
  match target {
    Some(id) => {
      app.mail = Some(mail::State::new(id).with_restored_panes(&app.ui_state));
      match app.runtime.as_ref() {
        Some(runtime) => mail::load(&runtime.db, id).map(Message::Mail),
        None => Task::none(),
      }
    }
    None => {
      app.mail = Some(mail::State::new(mail::EMPTY_MAIL_SELECTION).with_restored_panes(&app.ui_state));
      Task::none()
    }
  }
}

pub(super) fn resolve_mail_target(roster: &[OwnedPilot], last_selected: Option<i64>) -> Option<i64> {
  if let Some(id) = last_selected
    && roster.iter().any(|pilot| pilot.id == id)
  {
    return Some(id);
  }
  roster.first().map(|pilot| pilot.id)
}

pub(super) fn mail_clock_reload(app: &App) -> Task<Message> {
  if app.route != Route::Mail {
    return Task::none();
  }
  match (app.mail.as_ref(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => mail::reload(&runtime.db, state.active()).map(Message::Mail),
    _ => Task::none(),
  }
}

pub(super) fn navigate_to_calendar(app: &mut App, target: Option<i64>) -> Task<Message> {
  navigate(app, Route::Calendar);
  let features = calendar_features(app);
  let selection = target.unwrap_or(calendar::EMPTY_CALENDAR_SELECTION);
  app.calendar = Some(calendar::State::new(selection, app.now, features));
  match app.runtime.as_ref() {
    Some(runtime) => calendar::load(&runtime.db, selection, features).map(Message::Calendar),
    None => Task::none(),
  }
}

pub(super) fn industry_required_scopes() -> Vec<&'static str> {
  registry::descriptor(config::Feature::Industry).scopes.to_vec()
}

pub(super) fn industry_assign_pilots(app: &App) -> bool {
  app
    .runtime
    .as_ref()
    .map(|runtime| {
      let features = runtime.settings.features();
      features.is_enabled(config::Feature::SkillMonitoring) && features.is_enabled(config::Feature::CloneMonitoring)
    })
    .unwrap_or(false)
}

pub(super) fn navigate_to_industry(app: &mut App, target: Option<i64>) -> Task<Message> {
  navigate(app, Route::Industry);
  let required = industry_required_scopes();
  let selection = target.unwrap_or(industry::EMPTY_INDUSTRY_SELECTION);
  let facility_defaults = app
    .runtime
    .as_ref()
    .map(|runtime| industry::FacilityDefaults::from(runtime.settings.industry()))
    .unwrap_or_default();
  let assign_pilots = industry_assign_pilots(app);
  app.industry = Some(
    industry::State::new(
      selection,
      required.clone(),
      feature_flags(app),
      facility_defaults,
      app.industry_catalog.clone(),
      assign_pilots,
    )
    .with_restored_panes(&app.ui_state),
  );
  match app.runtime.as_ref() {
    Some(runtime) => industry::load(&runtime.db, selection, &required).map(Message::Industry),
    None => Task::none(),
  }
}

pub(super) fn industry_clock_reload(app: &App) -> Task<Message> {
  if app.route != Route::Industry {
    return Task::none();
  }
  match (app.industry.as_ref(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => {
      industry::reload(&runtime.db, state.active(), &industry_required_scopes()).map(Message::Industry)
    }
    _ => Task::none(),
  }
}

pub(super) fn calendar_features(app: &App) -> config::FeatureFlags {
  if let Some(state) = app.settings.as_ref() {
    return *state.settings().features();
  }
  if let Some(runtime) = app.runtime.as_ref() {
    return *runtime.settings.features();
  }
  config::FeatureFlags::default()
}

pub(super) fn calendar_clock_reload(app: &App) -> Task<Message> {
  if app.route != Route::Calendar {
    return Task::none();
  }
  match (app.calendar.as_ref(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => {
      calendar::reload(&runtime.db, state.active(), *runtime.settings.features()).map(Message::Calendar)
    }
    _ => Task::none(),
  }
}

pub(super) fn calendar_attention_tick(app: &App) -> Task<Message> {
  match app.runtime.as_ref() {
    Some(runtime) => {
      let db = runtime.db.clone();
      let now = app.now.to_rfc3339();
      Task::perform(
        async move { store::repo::calendar::attention_count(&db, &now).await.unwrap_or(0) },
        Message::CalendarAttentionCounted,
      )
    }
    None => Task::none(),
  }
}

pub(super) fn rail_mail_unread(live: i64, screen: Option<i64>) -> i64 {
  match screen {
    Some(screen) => screen.min(live),
    None => live,
  }
}

pub(super) fn navigate_to_assets(app: &mut App) -> Task<Message> {
  navigate(app, Route::Assets);
  app.assets = Some(assets::State::new(feature_flags(app)).with_restored_panes(&app.ui_state));
  match app.runtime.as_ref() {
    Some(runtime) => assets::load(&runtime.db).map(Message::Assets),
    None => Task::none(),
  }
}

pub(super) fn navigate_to_character_detail(app: &mut App, id: i64) -> Task<Message> {
  let owned: Vec<i64> = app
    .roster
    .as_ref()
    .map(roster::owned_roster)
    .unwrap_or_default()
    .iter()
    .map(|pilot| pilot.id)
    .collect();
  let features = enabled_features(app);
  navigate(app, Route::CharacterDetail(id));
  app.selected_character = Some(id);
  app.character_detail = Some(character_detail::State::new(id, &features));
  match app.runtime.as_ref() {
    Some(runtime) => character_detail::load(&runtime.db, id, owned).map(Message::CharacterDetail),
    None => Task::none(),
  }
}

pub(super) fn navigate_to_corporation_detail(app: &mut App, id: i64) -> Task<Message> {
  navigate(app, Route::CorporationDetail(id));
  app.corporation_detail = Some(corporation_detail::State::new(id));
  match app.runtime.as_ref() {
    Some(runtime) => corporation_detail::load(&runtime.db, id).map(Message::CorporationDetail),
    None => Task::none(),
  }
}

pub(super) fn route_view(app: &App) -> Element<'_, Message> {
  telemetry::record_view_loaded(telemetry::route_token(app.route.name()));
  match app.route {
    Route::Assets => assets_route_view(app),
    Route::Calendar => calendar_route_view(app),
    Route::CharacterDetail(_) => character_detail_route_view(app),
    Route::Roster => characters_route_view(app),
    Route::CorporationDetail(_) => corporation_detail_route_view(app),
    Route::Industry => industry_route_view(app),
    Route::Mail => mail_route_view(app),
    Route::Settings => settings_route_view(app),
    Route::Skills(id) => skills_route_view(app, id),
    Route::Wallet => wallet_route_view(app),
  }
}

pub(super) fn starting_up<'a>() -> Element<'a, Message> {
  placeholder(t!("shell.status.starting_up").into_owned())
}

pub(super) fn calendar_route_view(app: &App) -> Element<'_, Message> {
  match &app.calendar {
    Some(state) => calendar::view(state, app.now).map(Message::Calendar),
    None => starting_up(),
  }
}

pub(super) fn characters_route_view(app: &App) -> Element<'_, Message> {
  match &app.roster {
    Some(_) if app.auth.is_active() => auth::view(&app.auth).map(Message::Auth),
    Some(state) => roster::view(state, &app.status).map(Message::Roster),
    None => starting_up(),
  }
}

pub(super) fn character_detail_route_view(app: &App) -> Element<'_, Message> {
  match &app.character_detail {
    Some(state) => character_detail::view(state).map(Message::CharacterDetail),
    None => starting_up(),
  }
}

pub(super) fn corporation_detail_route_view(app: &App) -> Element<'_, Message> {
  match &app.corporation_detail {
    Some(state) => corporation_detail::view(state).map(Message::CorporationDetail),
    None => starting_up(),
  }
}

pub(super) fn skills_route_view(app: &App, id: i64) -> Element<'_, Message> {
  match &app.skills {
    Some(state) => skills::view(state, id, &app.status, app.now).map(Message::Skills),
    None => starting_up(),
  }
}

pub(super) fn industry_route_view(app: &App) -> Element<'_, Message> {
  match &app.industry {
    Some(state) => industry::view(state, &industry_required_scopes(), app.now).map(Message::Industry),
    None => starting_up(),
  }
}

pub(super) fn mail_route_view(app: &App) -> Element<'_, Message> {
  match &app.mail {
    Some(state) => mail::view(state).map(Message::Mail),
    None => starting_up(),
  }
}

pub(super) fn wallet_route_view(app: &App) -> Element<'_, Message> {
  match &app.wallet {
    Some(state) => wallet::view(state, app.now).map(Message::Wallet),
    None => starting_up(),
  }
}

pub(super) fn assets_route_view(app: &App) -> Element<'_, Message> {
  match &app.assets {
    Some(state) => assets::view(state, app.now).map(Message::Assets),
    None => starting_up(),
  }
}

pub(super) fn settings_route_view(app: &App) -> Element<'_, Message> {
  match &app.settings {
    Some(state) => settings::view(state).map(Message::Settings),
    None => starting_up(),
  }
}

pub(super) fn placeholder<'a>(message: String) -> Element<'a, Message> {
  container(text(message).size(typography::size::MD).style(|_| text::Style {
    color: Some(color::text::secondary()),
  }))
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .into()
}

pub(super) fn handle_nav(app: &mut App, destination: rail::Destination) -> Task<Message> {
  let enabled = enabled_features(app);
  let is_feature_disabled = |dest: rail::Destination| {
    registry::feature_for_destination(dest).is_some_and(|feature| !enabled.contains(&feature))
  };

  if is_feature_disabled(destination) {
    navigate(app, Route::Roster);
    return Task::none();
  }

  match destination {
    rail::Destination::Skills => {
      let roster = app.roster.as_ref().map(roster::owned_roster).unwrap_or_default();
      let target = resolve_skills_target(&roster, app.selected_character);
      let owned = roster.iter().map(|pilot| pilot.id).collect();
      navigate_to_skills(app, target, owned)
    }
    rail::Destination::Mail => {
      let roster = app.roster.as_ref().map(roster::owned_roster).unwrap_or_default();
      let target = resolve_mail_target(&roster, app.selected_character);
      navigate_to_mail(app, target)
    }
    rail::Destination::Calendar => navigate_to_calendar(app, None),
    rail::Destination::Industry => navigate_to_industry(app, None),
    rail::Destination::Wallet => navigate_to_wallet(app),
    rail::Destination::Assets => navigate_to_assets(app),
    other => {
      navigate(app, Route::from(other));
      Task::none()
    }
  }
}

pub(super) fn handle_nav_to(
  app: &mut App,
  destination: rail::Destination,
  sub_section: Option<&'static str>,
) -> Task<Message> {
  let nav = handle_nav(app, destination);
  let Some(id) = sub_section else {
    return nav;
  };
  if app.route.destination() != destination {
    return nav;
  }
  Task::batch([nav, select_sub_section(app, destination, id)])
}

pub(super) fn select_sub_section(app: &mut App, destination: rail::Destination, id: &str) -> Task<Message> {
  if let Some(token) = sub_section_token(destination, id) {
    telemetry::record_sub_section(token);
  }
  match destination {
    rail::Destination::Assets => select_assets_sub_section(app, id),
    rail::Destination::Calendar => select_calendar_sub_section(app, id),
    rail::Destination::Roster => select_characters_sub_section(app, id),
    rail::Destination::Industry => select_industry_sub_section(app, id),
    rail::Destination::Settings => select_settings_sub_section(app, id),
    rail::Destination::Wallet => select_wallet_sub_section(app, id),
    rail::Destination::Skills => select_skills_sub_section(app, id),
    rail::Destination::Mail => Task::none(),
  }
}

pub(super) fn sub_section_token(destination: rail::Destination, id: &str) -> Option<String> {
  // Inspecting telemetry never emits telemetry (§7.6).
  if matches!(destination, rail::Destination::Settings) && id == "telemetry" {
    return None;
  }
  let destination = destination_token(destination);
  Some(format!("{destination}.{id}"))
}

pub(super) fn destination_token(destination: rail::Destination) -> &'static str {
  match destination {
    rail::Destination::Assets => "assets",
    rail::Destination::Calendar => "calendar",
    rail::Destination::Industry => "industry",
    rail::Destination::Mail => "mail",
    rail::Destination::Roster => "roster",
    rail::Destination::Settings => "settings",
    rail::Destination::Skills => "skills",
    rail::Destination::Wallet => "wallet",
  }
}

pub(super) fn select_assets_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match assets::Tab::from_id(id) {
    Some(tab) if app.assets.as_mut().is_some_and(|state| state.select_tab_by_id(id)) => {
      update(app, Message::Assets(assets::Message::TabSelected(tab)))
    }
    _ => Task::none(),
  }
}

pub(super) fn select_calendar_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match calendar::View::from_id(id) {
    Some(view) if app.calendar.as_mut().is_some_and(|state| state.select_view_by_id(id)) => {
      update(app, Message::Calendar(calendar::Message::ViewSelected(view)))
    }
    _ => Task::none(),
  }
}

pub(super) fn select_characters_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match roster::Pane::from_id(id) {
    Some(pane) if app.roster.as_mut().is_some_and(|state| state.select_pane_by_id(id)) => {
      update(app, Message::Roster(roster::Message::TabSelected(pane)))
    }
    _ => Task::none(),
  }
}

pub(super) fn select_industry_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match industry::Tab::from_id(id) {
    Some(tab) if app.industry.as_mut().is_some_and(|state| state.select_tab_by_id(id)) => {
      update(app, Message::Industry(industry::Message::TabSelected(tab)))
    }
    _ => Task::none(),
  }
}

pub(super) fn select_settings_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match settings::Category::from_id(id) {
    Some(category)
      if app
        .settings
        .as_mut()
        .is_some_and(|state| state.select_category_by_id(id)) =>
    {
      update(app, Message::Settings(settings::Message::CategorySelected(category)))
    }
    _ => Task::none(),
  }
}

pub(super) fn select_wallet_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match wallet::Tab::from_id(id) {
    Some(tab) if app.wallet.as_mut().is_some_and(|state| state.select_tab_by_id(id)) => {
      update(app, Message::Wallet(wallet::Message::TabSelected(tab)))
    }
    _ => Task::none(),
  }
}

pub(super) fn select_skills_sub_section(app: &mut App, id: &str) -> Task<Message> {
  match id {
    "compare" => handle_skills(app, skills::Message::OpenCompare),
    _ => Task::none(),
  }
}

pub(super) fn active_sub_section(app: &App) -> Option<&'static str> {
  match app.route.destination() {
    rail::Destination::Assets => app.assets.as_ref().map(|state| state.active_tab().id()),
    rail::Destination::Calendar => app.calendar.as_ref().map(|state| state.active_view().id()),
    rail::Destination::Roster => app.roster.as_ref().map(|state| state.active_pane().id()),
    rail::Destination::Industry => app.industry.as_ref().map(|state| state.active_tab().id()),
    rail::Destination::Settings => app.settings.as_ref().map(|state| state.active_category().id()),
    rail::Destination::Wallet => app.wallet.as_ref().map(|state| state.active_tab().id()),
    rail::Destination::Mail | rail::Destination::Skills => None,
  }
}

pub(super) fn handle_rail_hover(app: &mut App, destination: Option<rail::Destination>) -> Task<Message> {
  match destination {
    Some(destination) => {
      app.rail_hover = Some(destination);
      app.rail_hover_gen = app.rail_hover_gen.wrapping_add(1);
      Task::none()
    }
    None => {
      app.rail_hover_gen = app.rail_hover_gen.wrapping_add(1);
      let generation = app.rail_hover_gen;
      Task::perform(async move { tokio::time::sleep(RAIL_HOVER_GRACE).await }, move |()| {
        Message::RailHoverExpire(generation)
      })
    }
  }
}

pub(super) fn handle_rail_hover_expire(app: &mut App, generation: u64) -> Task<Message> {
  if app.rail_hover_gen == generation {
    app.rail_hover = None;
  }
  Task::none()
}
