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
  let assign_pilots = industry_assign_pilots(app);
  let mut state = industry::State::new(
    selection,
    required.clone(),
    feature_flags(app),
    industry::FacilityDefaults::default(),
    app.industry_catalog.clone(),
    assign_pilots,
  )
  .with_restored_panes(&app.ui_state);
  if let Some(runtime) = app.runtime.as_ref() {
    state.set_clients(std::sync::Arc::clone(&runtime.esi), std::sync::Arc::clone(&runtime.sso));
  }
  app.industry = Some(state);
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

pub(super) fn navigate_to_character_detail_section(
  app: &mut App,
  id: i64,
  tab: character_detail::Tab,
) -> Task<Message> {
  let task = navigate_to_character_detail(app, id);
  if let Some(state) = app.character_detail.as_mut() {
    state.focus_tab(tab);
  }
  task
}

#[allow(dead_code)]
pub(super) fn navigate_to_captains_log(app: &mut App) -> Task<Message> {
  navigate(app, Route::CaptainsLog);
  app.captains_log = Some(captains_log::State::new().with_restored_panes(&app.ui_state));
  let character_ids = owned_character_ids(app);
  match app.runtime.as_ref() {
    Some(runtime) => captains_log::load(&runtime.db, character_ids).map(Message::CaptainsLog),
    None => Task::none(),
  }
}

pub(super) fn navigate_to_contact_sync(app: &mut App) -> Task<Message> {
  if !feature_flags(app).is_sub_enabled(config::SubFeature::Contacts) {
    navigate(app, Route::Roster);
    return Task::none();
  }
  navigate(app, Route::ContactSync);
  app.contact_sync = Some(contact_sync::State::new());
  match app.runtime.as_ref() {
    Some(runtime) => contact_sync::load(&runtime.db, Arc::clone(&runtime.esi)).map(Message::ContactSync),
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
  telemetry::collector::record_view_loaded(telemetry::collector::route_token(app.route.name()));
  match app.route {
    Route::Assets => assets_route_view(app),
    Route::Calendar => calendar_route_view(app),
    Route::CaptainsLog => captains_log_route_view(app),
    Route::CharacterDetail(_) => character_detail_route_view(app),
    Route::ContactSync => contact_sync_route_view(app),
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

pub(super) fn captains_log_route_view(app: &App) -> Element<'_, Message> {
  match &app.captains_log {
    Some(state) => captains_log::view(state).map(Message::CaptainsLog),
    None => starting_up(),
  }
}

pub(super) fn contact_sync_route_view(app: &App) -> Element<'_, Message> {
  match &app.contact_sync {
    Some(state) => contact_sync::view(state).map(Message::ContactSync),
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
    telemetry::collector::record_sub_section(token);
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
  if id == "captains-log" {
    return navigate_to_captains_log(app);
  }
  if id == "contact-sync" {
    return navigate_to_contact_sync(app);
  }
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
    rail::Destination::Roster if app.route == Route::CaptainsLog => Some("captains-log"),
    rail::Destination::Roster if app.route == Route::ContactSync => Some("contact-sync"),
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    app::test_support::*,
    sync::{JobKind, Subject},
  };

  mod destination {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_a_calendar_route_to_the_calendar_destination() {
      assert_eq!(Route::Calendar.destination(), rail::Destination::Calendar);
    }

    #[test]
    fn it_maps_a_mail_route_to_the_mail_destination() {
      assert_eq!(Route::Mail.destination(), rail::Destination::Mail);
    }

    #[test]
    fn it_maps_a_skills_route_to_the_skills_destination() {
      assert_eq!(Route::Skills(42).destination(), rail::Destination::Skills);
    }

    #[test]
    fn it_round_trips_characters_settings_and_mail_through_from() {
      assert_eq!(Route::from(Route::Roster.destination()), Route::Roster);
      assert_eq!(Route::from(Route::Settings.destination()), Route::Settings);
      assert_eq!(Route::from(Route::Mail.destination()), Route::Mail);
      assert_eq!(Route::from(Route::Calendar.destination()), Route::Calendar);
    }
  }

  mod rail_mail_unread {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clears_the_rail_dot_when_a_count_tick_reports_zero_unread() {
      let mut app = test_app();
      app.mail_unread = 4;

      let _ = update(&mut app, Message::MailUnreadCounted(0));

      assert_eq!(app.mail_unread, 0, "the dot clears when no unread mail remains");
    }

    #[test]
    fn it_folds_a_count_tick_into_the_rail_dot_regardless_of_the_active_route() {
      let mut app = test_app();
      app.route = Route::Roster;
      assert!(app.mail.is_none());

      let _ = update(&mut app, Message::MailUnreadCounted(5));

      assert_eq!(app.mail_unread, 5);
      assert_eq!(
        crate::app::rail_mail_unread(app.mail_unread, app.mail.as_ref().map(mail::State::unified_unread)),
        5,
        "the rail dot reflects the background count with no Mail screen open"
      );
    }

    #[test]
    fn it_keeps_the_live_count_when_it_is_already_the_lower_of_the_two() {
      assert_eq!(crate::app::rail_mail_unread(1, Some(4)), 1);
    }

    #[test]
    fn it_prefers_the_screens_fresher_optimistic_count_over_a_stale_live_count() {
      assert_eq!(crate::app::rail_mail_unread(3, Some(2)), 2);
    }

    #[test]
    fn it_uses_the_live_count_when_the_mail_screen_is_closed() {
      assert_eq!(crate::app::rail_mail_unread(3, None), 3);
      assert_eq!(crate::app::rail_mail_unread(0, None), 0);
    }
  }

  mod resolve_mail_target {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_to_the_first_owned_pilot_with_no_prior_selection() {
      let roster = vec![pilot(7), pilot(3)];

      assert_eq!(resolve_mail_target(&roster, None), Some(7));
    }

    #[test]
    fn it_falls_back_to_first_owned_when_the_sticky_selection_left_the_roster() {
      let roster = vec![pilot(7), pilot(3)];

      assert_eq!(resolve_mail_target(&roster, Some(99)), Some(7));
    }

    #[test]
    fn it_keeps_the_sticky_selection_when_still_owned() {
      let roster = vec![pilot(7), pilot(3)];

      assert_eq!(resolve_mail_target(&roster, Some(3)), Some(3));
    }

    #[test]
    fn it_yields_none_for_an_empty_roster() {
      assert_eq!(resolve_mail_target(&[], None), None);
      assert_eq!(resolve_mail_target(&[], Some(7)), None);
    }
  }

  mod views {
    use super::*;

    fn render_route(route: Route) {
      let app = ready_app();
      let mut app = app;
      app.route = route;
      let _ = route_view(&app);
    }

    #[tokio::test]
    async fn it_builds_the_subscription_set_for_each_live_screen() {
      let app = test_app();
      let _ = subscription(&app);

      let mut app = ready_app();
      let runtime = test_runtime().await;
      app.splash = Some(splash::State::default());
      app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
      app.calendar = Some(calendar::State::new(1, app.now, calendar_features(&app)));
      app.industry = Some(industry::State::new(
        1,
        industry_required_scopes(),
        config::FeatureFlags::default(),
        industry::FacilityDefaults::default(),
        None,
        false,
      ));
      app.runtime = Some(runtime);
      app.sync_popover_open = true;
      app.status.apply(&crate::sync::Event::Started {
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(1)),
      });
      app
        .editors
        .insert(window::Id::unique(), skill_plan_editor::State::new(Some(1)));

      let (_dir, session) = temp_sync_session();
      app.sync_session = Some(session);
      app.read_only = None;
      let _ = subscription(&app);

      app.read_only = Some(HolderInfo {
        hostname: "studio-mac".to_owned(),
        last_active: Utc::now(),
        machine_id: "machine-b".to_owned(),
      });
      let _ = subscription(&app);
    }

    #[test]
    fn it_builds_the_sync_model_with_per_pilot_job_rows() {
      let mut app = ready_app();
      app.last_synced = Some(app.now);
      let model = sync_model(&app);
      assert_eq!(model.total, model.rows.len());
    }

    #[test]
    fn it_dispatches_the_daemon_view_for_each_window_kind() {
      let mut app = ready_app();
      let splash_id = window::Id::unique();
      app.windows.register(splash_id, Window::Splash);
      app.splash = Some(splash::State::default());
      let _ = view(&app, splash_id);
      app.splash = None;
      let _ = view(&app, splash_id);

      let main_id = window::Id::unique();
      app.windows.register(main_id, Window::Main);
      app.route = Route::Roster;
      let _ = view(&app, main_id);

      let editor_id = window::Id::unique();
      app.windows.register(editor_id, Window::SkillPlanEditor);
      app.editors.insert(editor_id, skill_plan_editor::State::new(Some(1)));
      let _ = view(&app, editor_id);

      let _ = view(&app, window::Id::unique());
    }

    #[tokio::test]
    async fn it_drives_character_detail_through_the_runtime_backed_handler() {
      let mut app = ready_app();
      app.runtime = Some(test_runtime().await);

      let _ = handle_character_detail(&mut app, character_detail::Message::CharacterChanged(7));
      assert_eq!(app.route, Route::CharacterDetail(7));
      assert_eq!(app.selected_character, Some(7));

      let _ = handle_character_detail(&mut app, character_detail::Message::ReauthRequested(7));

      let _ = handle_character_detail(
        &mut app,
        character_detail::Message::ContactEntityInput("jita".to_owned()),
      );

      let _ = handle_character_detail(&mut app, character_detail::Message::PickerToggled);
    }

    #[tokio::test]
    async fn it_drives_contact_sync_through_the_runtime_backed_handler() {
      let mut app = ready_app();
      app.runtime = Some(test_runtime().await);

      let _ = update(
        &mut app,
        Message::NavTo(rail::Destination::Roster, Some("contact-sync")),
      );
      assert_eq!(app.route, Route::ContactSync);
      assert!(app.contact_sync.is_some());

      let _ = update(
        &mut app,
        Message::ContactSync(contact_sync::Message::Contacts(Box::new(
          character_detail::Message::ContactAddOpened,
        ))),
      );
      let _ = update(
        &mut app,
        Message::ContactSync(contact_sync::Message::Contacts(Box::new(
          character_detail::Message::ContactEntityInput("vex".to_owned()),
        ))),
      );
      let _ = update(&mut app, Message::ContactSync(contact_sync::Message::EditorClosed));

      let _ = update(&mut app, Message::ContactSync(contact_sync::Message::Exit));
      assert_eq!(app.route, Route::Roster, "Exit returns to the roster");
    }

    #[test]
    fn it_drives_captains_log_from_the_roster_cascade() {
      let mut app = ready_app();

      let _ = update(
        &mut app,
        Message::NavTo(rail::Destination::Roster, Some("captains-log")),
      );

      assert_eq!(app.route, Route::CaptainsLog);
      assert!(app.captains_log.is_some());
    }

    #[test]
    fn it_drives_captains_log_from_the_roster_utilities_menu() {
      let mut app = ready_app();

      let _ = update(
        &mut app,
        Message::Roster(roster::Message::UtilityActivated(roster::Utility::CaptainsLog)),
      );

      assert_eq!(app.route, Route::CaptainsLog);
      assert!(app.captains_log.is_some());
    }

    #[test]
    fn it_highlights_captains_log_while_the_log_is_open() {
      let mut app = ready_app();
      app.route = Route::CaptainsLog;

      assert_eq!(
        app.route.destination(),
        rail::Destination::Roster,
        "the rail stays on the Roster destination"
      );
      assert_eq!(active_sub_section(&app), Some("captains-log"));
    }

    #[test]
    fn it_renders_every_route_through_route_view() {
      render_route(Route::Roster);
      render_route(Route::CharacterDetail(1));
      render_route(Route::ContactSync);
      render_route(Route::CorporationDetail(1));
      render_route(Route::Skills(1));
      render_route(Route::Mail);
      render_route(Route::Wallet);
      render_route(Route::Assets);
      render_route(Route::Settings);
    }

    #[test]
    fn it_renders_main_view_with_a_runtime_and_with_the_init_error_and_pre_runtime_placeholders() {
      let mut app = ready_app();
      app.route = Route::Roster;
      app.runtime = None;
      let _ = main_view(&app);
      app.init_error = Some("boom".to_owned());
      let _ = main_view(&app);
    }

    #[test]
    fn it_renders_main_view_with_the_sync_popover_open() {
      let mut app = ready_app();
      app.route = Route::Roster;
      app.sync_popover_open = true;
      let _ = main_view(&app);
    }

    #[test]
    fn it_renders_the_starting_up_placeholder_for_an_unbuilt_route() {
      let mut app = test_app();
      app.route = Route::Wallet;
      let _ = route_view(&app);
      let _ = starting_up();
    }

    #[test]
    fn it_renders_the_status_bar_with_and_without_an_active_outbox() {
      let mut app = ready_app();
      let _ = status_bar_view(&app);
      app.outbox.apply(&crate::sync::Event::OutboxInflight {
        id: 1,
      });
      let _ = status_bar_view(&app);
    }

    #[test]
    fn it_dispatches_every_per_window_view_helper() {
      let mut app = ready_app();

      let compose_id = window::Id::unique();
      app.windows.register(compose_id, Window::MailCompose);
      let _ = view(&app, compose_id);
      app.composes.insert(
        compose_id,
        mail::compose::Draft::from_seed(mail::compose::Seed::Blank {
          from_character_id: 1,
        }),
      );
      let _ = view(&app, compose_id);

      let manage_id = window::Id::unique();
      app.windows.register(manage_id, Window::ManagePlans);
      let _ = view(&app, manage_id);
      app.manage_plans = Some((manage_id, skill_plan_manager::State::new()));
      let _ = view(&app, manage_id);

      let compare_id = window::Id::unique();
      app.windows.register(compare_id, Window::Compare);
      let _ = view(&app, compare_id);
      app.compare = Some((compare_id, skills_compare::State::new(vec![1], Vec::new())));
      let _ = view(&app, compare_id);

      let contract_id = window::Id::unique();
      app.windows.register(contract_id, Window::Contract);
      let _ = view(&app, contract_id);

      let killmail_id = window::Id::unique();
      app.windows.register(killmail_id, Window::Killmail);
      let _ = view(&app, killmail_id);

      let stockpile_id = window::Id::unique();
      app.windows.register(stockpile_id, Window::StockpileEditor);
      let _ = view(&app, stockpile_id);
    }

    #[test]
    fn it_renders_the_notifications_panel_on_both_rail_sides() {
      let mut app = ready_app();
      let _ = notifications_panel(&app, config::NavLocation::Left);
      let _ = notifications_panel(&app, config::NavLocation::Right);

      app.notifications_unread = 2;
      app
        .notifications
        .push(test_notification(1, store::model::NotificationDestination::Skills));
      app
        .notification_names
        .insert(store::model::NotificationOwner::Character(1), "Pilot 1".to_owned());
      let _ = notifications_panel(&app, config::NavLocation::Left);
      let _ = notifications_panel(&app, config::NavLocation::Right);
    }
  }
}
