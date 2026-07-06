use std::collections::HashMap;

use iced::window;

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Window {
  BudgetRules,
  CalendarEvent,
  Compare,
  Contract,
  FirstRun,
  Killmail,
  MailCompose,
  Main,
  ManagePlans,
  SkillPlanEditor,
  Splash,
  StockpileEditor,
  StockpileImport,
}

impl Window {
  pub fn state_key(self) -> Option<&'static str> {
    match self {
      Self::BudgetRules => Some("budget_rules"),
      Self::Compare => Some("skills_compare"),
      Self::Main => Some("main"),
      Self::ManagePlans => Some("skill_plan_manager"),
      Self::SkillPlanEditor => Some("skill_plan_editor"),
      Self::StockpileImport => Some("stockpile_import"),
      Self::CalendarEvent
      | Self::Contract
      | Self::FirstRun
      | Self::Killmail
      | Self::MailCompose
      | Self::Splash
      | Self::StockpileEditor => None,
    }
  }

  // The usage view_open token a window emits when it opens. Main is covered by
  // route navigation, Splash is not a user screen, and SkillPlanEditor is
  // tokenized at its open site (plan vs template mode).
  pub fn usage_token(self) -> Option<&'static str> {
    match self {
      Self::BudgetRules => Some("wallet.budget_rules"),
      Self::CalendarEvent => Some("calendar.event"),
      Self::Compare => Some("skills.compare"),
      Self::Contract => Some("contract"),
      Self::FirstRun => Some("first_run"),
      Self::Killmail => Some("killmail"),
      Self::MailCompose => Some("mail.compose"),
      Self::ManagePlans => Some("skills.manage_plans"),
      Self::StockpileEditor => Some("industry.stockpile_editor"),
      Self::StockpileImport => Some("industry.stockpile_import"),
      Self::Main | Self::SkillPlanEditor | Self::Splash => None,
    }
  }
}

#[derive(Debug)]
pub struct WindowStates<S> {
  states: HashMap<window::Id, S>,
}

impl<S> WindowStates<S> {
  pub fn get(&self, id: window::Id) -> Option<&S> {
    self.states.get(&id)
  }

  pub fn get_mut(&mut self, id: window::Id) -> Option<&mut S> {
    self.states.get_mut(&id)
  }

  pub fn insert(&mut self, id: window::Id, state: S) {
    self.states.insert(id, state);
  }

  // Exercised by this module's WindowStates tests; the detached child windows that read it are unbuilt.
  #[cfg_attr(not(test), expect(dead_code))]
  pub fn is_empty(&self) -> bool {
    self.states.is_empty()
  }

  pub fn iter(&self) -> impl Iterator<Item = (window::Id, &S)> + '_ {
    self.states.iter().map(|(id, state)| (*id, state))
  }

  // Exercised by this module's WindowStates tests; the detached child windows that read it are unbuilt.
  #[cfg_attr(not(test), expect(dead_code))]
  pub fn len(&self) -> usize {
    self.states.len()
  }

  pub fn remove(&mut self, id: window::Id) -> Option<S> {
    self.states.remove(&id)
  }
}

impl<S> Default for WindowStates<S> {
  fn default() -> Self {
    Self {
      states: HashMap::new(),
    }
  }
}

#[derive(Debug, Default)]
pub struct Windows {
  ids: HashMap<window::Id, Window>,
}

impl Windows {
  pub fn id_for(&self, window: Window) -> Option<window::Id> {
    self.ids.iter().find(|(_, kind)| **kind == window).map(|(id, _)| *id)
  }

  pub fn ids(&self) -> impl Iterator<Item = window::Id> + '_ {
    self.ids.keys().copied()
  }

  // Consumed by the not-yet-built detached child windows to enumerate every open instance of a kind;
  // exercised by this module's tests today.
  #[cfg_attr(not(test), expect(dead_code))]
  pub fn ids_for(&self, window: Window) -> impl Iterator<Item = window::Id> + '_ {
    self
      .ids
      .iter()
      .filter(move |(_, kind)| **kind == window)
      .map(|(id, _)| *id)
  }

  pub fn is_empty(&self) -> bool {
    self.ids.is_empty()
  }

  pub fn kind(&self, id: window::Id) -> Option<Window> {
    self.ids.get(&id).copied()
  }

  pub fn register(&mut self, id: window::Id, window: Window) {
    if let Some(token) = window.usage_token() {
      telemetry::record_window_open(token);
    }
    self.ids.insert(id, window);
  }

  pub fn remove(&mut self, id: window::Id) -> Option<Window> {
    self.ids.remove(&id)
  }
}

pub(super) fn scale_factor(app: &App, _id: window::Id) -> f32 {
  scale_to_factor(*app.accessibility.scale())
}
pub(super) fn scale_to_factor(scale: u8) -> f32 {
  f32::from(scale.clamp(SCALE_MIN, SCALE_MAX)) / 100.0
}
pub(super) fn geometry_after_resize(base: Option<WindowGeometry>, size: Size) -> WindowGeometry {
  WindowGeometry {
    height: size.height,
    width: size.width,
    ..base.unwrap_or(ZERO_GEOMETRY)
  }
}
pub(super) fn geometry_after_move(base: Option<WindowGeometry>, position: Point) -> WindowGeometry {
  WindowGeometry {
    x: position.x,
    y: position.y,
    ..base.unwrap_or(ZERO_GEOMETRY)
  }
}
pub(super) fn window_key(app: &App, id: window::Id) -> Option<&'static str> {
  app.windows.kind(id).and_then(Window::state_key)
}
pub(super) fn record_window_geometry(app: &mut App, id: window::Id, geometry: WindowGeometry) {
  let Some(key) = window_key(app, id) else {
    return;
  };
  if matches!(app.windows.kind(id), Some(Window::Main)) {
    telemetry::set_window_size(geometry.width as u32, geometry.height as u32);
  }
  app.ui_state.windows.insert(key.to_owned(), geometry);
  app.coalescer.request(app.ui_state.clone(), Instant::now());
}
pub(super) fn handle_main_screen_size_probed(size: Option<Size>) -> Task<Message> {
  if let Some(size) = size {
    telemetry::set_screen_size(size.width as u32, size.height as u32);
  }
  Task::none()
}
pub(super) fn propagate_host_width(app: &mut App, id: window::Id, width: f32) {
  match app.windows.kind(id) {
    Some(Window::Main) => {
      if let Some(state) = app.skills.as_mut() {
        state.set_pane_host_width(width);
      }
      if let Some(state) = app.mail.as_mut() {
        state.set_pane_host_width(width);
      }
      if let Some(state) = app.wallet.as_mut() {
        state.set_pane_host_width(width);
      }
      if let Some(state) = app.assets.as_mut() {
        state.set_pane_host_width(width);
      }
      if let Some(state) = app.industry.as_mut() {
        state.set_pane_host_width(width);
      }
    }
    Some(Window::SkillPlanEditor) => {
      if let Some(state) = app.editors.get_mut(id) {
        state.set_pane_host_width(width);
      }
    }
    _ => {}
  }
}
/// Always returns no monitors; window-position restore therefore falls back to the coordinate-range guard rather than per-display on-screen validation.
pub(super) fn connected_monitors() -> Vec<validity::Rect> {
  Vec::new()
}
pub(super) fn resolve_window_geometry(
  saved: Option<WindowGeometry>,
  monitors: &[validity::Rect],
  default: Size,
) -> (Size, window::Position) {
  let Some(geometry) = saved else {
    return (default, window::Position::Centered);
  };

  let size = if validity::is_size_in_range(&geometry) {
    Size::new(
      geometry.width.max(spacing::layout::MIN_WINDOW_WIDTH),
      geometry.height.max(spacing::layout::MIN_WINDOW_HEIGHT),
    )
  } else {
    default
  };
  let position_valid = if monitors.is_empty() {
    validity::is_in_range(&geometry)
  } else {
    validity::is_position_valid(&geometry, monitors)
  };

  let position = if position_valid {
    window::Position::Specific(Point::new(geometry.x, geometry.y))
  } else {
    window::Position::Centered
  };

  (size, position)
}
pub(super) fn restored_geometry(ui: &UiState, window: Window, default: Size) -> (Size, window::Position) {
  let saved = window.state_key().and_then(|key| ui.windows.get(key).copied());
  resolve_window_geometry(saved, &connected_monitors(), default)
}
pub(super) fn handle_close_requested(app: &mut App, id: window::Id) -> Task<Message> {
  let close = match app.windows.kind(id) {
    Some(Window::BudgetRules) => close_budget_rules_window(app, id),
    Some(Window::CalendarEvent) => close_calendar_event_window(app, id),
    Some(Window::Compare) => close_compare_window(app, id),
    Some(Window::Contract) => close_contract_window(app, id),
    Some(Window::Killmail) => close_killmail_window(app, id),
    Some(Window::MailCompose) => close_compose_window(app, id),
    Some(Window::ManagePlans) => close_manage_plans_window(app, id),
    Some(Window::SkillPlanEditor) => close_editor_window(app, id),
    Some(Window::StockpileEditor) => close_stockpile_editor_window(app, id),
    Some(Window::StockpileImport) => close_stockpile_import_window(app, id),
    _ => {
      app.windows.remove(id);
      window::close(id)
    }
  };
  Task::batch([close, shutdown_if_last_window(app)])
}
pub(super) fn on_window_closed(app: &mut App, id: window::Id) -> Task<Message> {
  let Some(kind) = app.windows.remove(id) else {
    return Task::none();
  };
  match kind {
    Window::BudgetRules if app.budget_rules.as_ref().map(|(bid, _)| *bid) == Some(id) => app.budget_rules = None,
    Window::CalendarEvent => {
      app.calendar_events.remove(id);
    }
    Window::Compare if app.compare.as_ref().map(|(cid, _)| *cid) == Some(id) => app.compare = None,
    Window::Contract => {
      app.contracts.remove(id);
    }
    Window::Killmail => {
      app.killmails.remove(id);
    }
    Window::MailCompose => {
      let save = compose_save_on_drop(app, id);
      app.composes.remove(id);
      return Task::batch([save, shutdown_if_last_window(app)]);
    }
    Window::ManagePlans if app.manage_plans.as_ref().map(|(mid, _)| *mid) == Some(id) => app.manage_plans = None,
    Window::SkillPlanEditor => {
      app.editors.remove(id);
    }
    Window::StockpileEditor => {
      app.stockpile_editors.remove(id);
    }
    Window::StockpileImport => {
      app.stockpile_imports.remove(id);
    }
    _ => {}
  }
  shutdown_if_last_window(app)
}
pub(super) fn compose_save_on_drop(app: &App, id: window::Id) -> Task<Message> {
  match (
    app.composes.get(id).and_then(mail::compose::Draft::pending_save),
    app.runtime.as_ref(),
  ) {
    (Some((draft_id, input)), Some(runtime)) => {
      let db = runtime.db.clone();
      Task::perform(
        async move { mail::persist_pending_draft(db, draft_id, input).await },
        |()| Message::Mail(mail::Message::DraftSaved(None)),
      )
    }
    _ => Task::none(),
  }
}
pub(super) fn handle_focus_main_window(app: &App) -> Task<Message> {
  match app.windows.id_for(Window::Main) {
    Some(id) => {
      tracing::info!(target: "pod::lifecycle", "raising the main window for a duplicate launch");
      window::gain_focus(id)
    }
    None => Task::none(),
  }
}
pub(super) fn on_window_opened(app: &App, id: window::Id) -> Task<Message> {
  match app.windows.kind(id) {
    // Transparent custom-chrome windows need the OS drop-shadow suppressed. Each kind leaves this
    // arm when its conversion task promotes it to a native window; `Window::Splash` stays for good.
    Some(Window::Killmail | Window::Splash) => disable_shadow(id),
    Some(Window::Main) => window::monitor_size(id).map(Message::MainScreenSizeProbed),
    _ => Task::none(),
  }
}
pub(super) fn handle_window(app: &mut App, id: window::Id, event: window::Event) -> Task<Message> {
  match event {
    window::Event::Resized(size) => {
      let base = window_key(app, id).and_then(|key| app.ui_state.windows.get(key).copied());
      record_window_geometry(app, id, geometry_after_resize(base, size));
      propagate_host_width(app, id, size.width);
      Task::none()
    }
    window::Event::Moved(position) => {
      let base = window_key(app, id).and_then(|key| app.ui_state.windows.get(key).copied());
      record_window_geometry(app, id, geometry_after_move(base, position));
      Task::none()
    }
    window::Event::CloseRequested => {
      flush_pending_save(app);
      handle_close_requested(app, id)
    }
    window::Event::Closed => {
      flush_pending_save(app);
      on_window_closed(app, id)
    }
    _ => Task::none(),
  }
}
#[cfg(target_os = "macos")]
pub(super) fn disable_shadow(id: window::Id) -> Task<Message> {
  window::run(id, |w| {
    use iced::window::raw_window_handle::RawWindowHandle;

    let Ok(handle) = w.window_handle() else {
      return;
    };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else {
      return;
    };
    let ns_view: *mut objc2::runtime::AnyObject = h.ns_view.as_ptr().cast();
    unsafe {
      let ns_window: *mut objc2::runtime::AnyObject = objc2::msg_send![ns_view, window];
      if !ns_window.is_null() {
        let _: () = objc2::msg_send![ns_window, setHasShadow: false];
      }
    }
  })
  .discard()
}
#[cfg(not(target_os = "macos"))]
pub(super) fn disable_shadow(_: window::Id) -> Task<Message> {
  Task::none()
}
pub(super) fn open_native_window(app: &mut App, kind: Window, default_size: Size) -> (window::Id, Task<Message>) {
  let (size, position) = restored_geometry(&app.ui_state, kind, default_size);
  let settings = window::Settings {
    size,
    position,
    decorations: true,
    resizable: true,
    icon: app_icon(),
    ..window::Settings::default()
  };
  let (id, open_task) = window::open(settings);
  app.windows.register(id, kind);
  (id, open_task.map(Message::WindowOpened))
}
pub(super) fn open_compare_window(app: &mut App, seed_ids: Vec<i64>) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  let roster = app.roster.as_ref().map(roster::owned_roster).unwrap_or_default();

  let close_existing = match app.compare.take() {
    Some((existing_id, _)) => {
      app.windows.remove(existing_id);
      window::close(existing_id)
    }
    None => Task::none(),
  };

  let (id, open_task) = open_native_window(
    app,
    Window::Compare,
    Size::new(COMPARE_WINDOW_WIDTH, COMPARE_WINDOW_HEIGHT),
  );
  app.compare = Some((id, skills_compare::State::new(seed_ids.clone(), roster)));

  Task::batch([
    close_existing,
    open_task,
    skills_compare::load(&db, seed_ids).map(Message::Compare),
  ])
}
pub(super) fn close_compare_window(app: &mut App, id: window::Id) -> Task<Message> {
  if app.compare.as_ref().map(|(cid, _)| *cid) == Some(id) {
    app.compare = None;
  }
  app.windows.remove(id);
  window::close(id)
}
pub(super) fn open_editor_window(
  app: &mut App,
  character_id: Option<i64>,
  seed: skill_plan_editor::Seed,
) -> (Option<window::Id>, Task<Message>) {
  let Some(runtime) = app.runtime.as_ref() else {
    return (None, Task::none());
  };
  let db = runtime.db.clone();

  let source_plan_id = match &seed {
    skill_plan_editor::Seed::Existing(plan_id) => Some(*plan_id),
    _ => None,
  };
  if let Some(plan_id) = source_plan_id
    && let Some((existing, _)) = app
      .editors
      .iter()
      .find(|(_, state)| state.source_plan_id() == Some(plan_id))
  {
    return (Some(existing), window::gain_focus(existing));
  }

  let (id, open_task) = open_native_window(
    app,
    Window::SkillPlanEditor,
    Size::new(EDITOR_WINDOW_WIDTH, EDITOR_WINDOW_HEIGHT),
  );
  telemetry::record_window_open(if character_id.is_none() {
    "skills.template_editor"
  } else {
    "skills.plan_editor"
  });
  app.editors.insert(
    id,
    skill_plan_editor::State::new(character_id)
      .with_source_plan_id(source_plan_id)
      .with_restored_panes(&app.ui_state),
  );

  (
    Some(id),
    Task::batch([
      open_task,
      skill_plan_editor::load(&db, character_id, seed).map(move |msg| Message::SkillPlanEditor(id, msg)),
    ]),
  )
}
pub(super) fn close_editor_window(app: &mut App, id: window::Id) -> Task<Message> {
  let was_editor = app.editors.remove(id).is_some();
  app.windows.remove(id);

  let reload = match (was_editor, app.skills.as_ref(), app.runtime.as_ref()) {
    (true, Some(skills), Some(runtime)) => skills::reload_plans(&runtime.db, skills.active()).map(Message::Skills),
    _ => Task::none(),
  };
  let manager_reload = if was_editor {
    reload_manage_plans_roster(app)
  } else {
    Task::none()
  };
  Task::batch([window::close(id), reload, manager_reload])
}
pub(super) fn open_killmail_window(app: &mut App, source: killmail_detail::Source, killmail_id: i64) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  let size = Size::new(
    killmail_detail::KILLMAIL_WINDOW_WIDTH,
    killmail_detail::KILLMAIL_WINDOW_HEIGHT,
  );
  let (id, open_task) = open_native_window(app, Window::Killmail, size);
  app
    .killmails
    .insert(id, killmail_detail::State::new(source, killmail_id));

  Task::batch([
    open_task,
    killmail_detail::load(&db, source, killmail_id).map(move |msg| Message::Killmail(id, msg)),
  ])
}
pub(super) fn handle_killmail(app: &mut App, id: window::Id, msg: killmail_detail::Message) -> Task<Message> {
  let Some(state) = app.killmails.get_mut(id) else {
    return Task::none();
  };
  let killmail_detail::Message::Loaded(detail) = msg;
  state.set_detail(*detail);
  let keys = state.stale_images();
  dispatch_image_fetches(app, keys)
}
pub(super) fn close_killmail_window(app: &mut App, id: window::Id) -> Task<Message> {
  app.killmails.remove(id);
  app.windows.remove(id);
  window::close(id)
}
pub(super) fn open_contract_window(app: &mut App, source: contract_detail::Source, contract_id: i64) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  let size = Size::new(
    contract_detail::CONTRACT_WINDOW_WIDTH,
    contract_detail::CONTRACT_WINDOW_HEIGHT,
  );
  let (id, open_task) = open_native_window(app, Window::Contract, size);
  app
    .contracts
    .insert(id, contract_detail::State::new(source, contract_id));
  Task::batch([
    open_task,
    contract_detail::load(&db, source, contract_id).map(move |msg| Message::Contract(id, msg)),
  ])
}
pub(super) fn handle_contract(app: &mut App, id: window::Id, msg: contract_detail::Message) -> Task<Message> {
  let Some(state) = app.contracts.get_mut(id) else {
    return Task::none();
  };
  let contract_detail::Message::Loaded(detail) = msg;
  state.set_detail(*detail);
  let keys = state.stale_images();
  dispatch_image_fetches(app, keys)
}
pub(super) fn close_contract_window(app: &mut App, id: window::Id) -> Task<Message> {
  app.contracts.remove(id);
  app.windows.remove(id);
  window::close(id)
}
pub(super) fn open_manage_plans_window(app: &mut App) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  if let Some(id) = app.windows.id_for(Window::ManagePlans) {
    return window::gain_focus(id);
  }
  let size = Size::new(
    skill_plan_manager::MANAGE_PLANS_WINDOW_WIDTH,
    skill_plan_manager::MANAGE_PLANS_WINDOW_HEIGHT,
  );
  let (id, open_task) = open_native_window(app, Window::ManagePlans, size);
  app.manage_plans = Some((id, skill_plan_manager::State::new()));
  Task::batch([open_task, skill_plan_manager::load(&db).map(Message::ManagePlans)])
}
fn with_manage_plans(app: &mut App, edit: impl FnOnce(&mut skill_plan_manager::State)) -> Task<Message> {
  if let Some((_, state)) = app.manage_plans.as_mut() {
    edit(state);
  }
  Task::none()
}

fn confirm_delete_plan(app: &mut App, plan_id: i64) -> Task<Message> {
  if let Some((_, state)) = app.manage_plans.as_mut() {
    state.clear_delete();
  }
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  Task::perform(
    async move {
      if let Err(error) = store::repo::skills::delete(&db, plan_id).await {
        tracing::error!(plan_id, %error, "failed to delete skill plan");
      }
      Box::new(skill_plan_manager::load_roster(&db).await)
    },
    skill_plan_manager::Message::Loaded,
  )
  .map(Message::ManagePlans)
}

fn copy_plan(app: &mut App, plan_id: i64, target_character_id: i64) -> Task<Message> {
  if let Some((_, state)) = app.manage_plans.as_mut() {
    state.close_copy_menu();
  }
  let existing_names = manage_plans_target_names(app, target_character_id);
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  Task::perform(
    async move {
      if let Err(error) = copy_plan_to_character(&db, plan_id, target_character_id, &existing_names).await {
        tracing::error!(plan_id, target_character_id, %error, "failed to copy skill plan");
      }
      Box::new(skill_plan_manager::load_roster(&db).await)
    },
    skill_plan_manager::Message::Loaded,
  )
  .map(Message::ManagePlans)
}

fn manage_plans_open(app: &App) -> bool {
  app.manage_plans.is_some()
}

pub(super) fn reload_manage_plans_roster(app: &App) -> Task<Message> {
  if !manage_plans_open(app) {
    return Task::none();
  }
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  Task::perform(
    async move { Box::new(skill_plan_manager::load_roster(&db).await) },
    skill_plan_manager::Message::Loaded,
  )
  .map(Message::ManagePlans)
}

fn loaded_manage_plans(app: &mut App, roster: skill_plan_manager::Roster) -> Task<Message> {
  let Some((_, state)) = app.manage_plans.as_mut() else {
    return Task::none();
  };
  state.set_roster(roster);
  let keys = state.stale_images();
  dispatch_image_fetches(app, keys)
}

pub(super) fn handle_manage_plans(app: &mut App, msg: skill_plan_manager::Message) -> Task<Message> {
  match msg {
    skill_plan_manager::Message::CancelDelete => with_manage_plans(app, skill_plan_manager::State::clear_delete),
    skill_plan_manager::Message::CharacterSelected(character_id) => {
      with_manage_plans(app, |state| state.select(character_id))
    }
    skill_plan_manager::Message::CloseCopyMenu => with_manage_plans(app, skill_plan_manager::State::close_copy_menu),
    skill_plan_manager::Message::ConfirmDelete(plan_id) => confirm_delete_plan(app, plan_id),
    skill_plan_manager::Message::CopyPlan {
      plan_id,
      target_character_id,
    } => copy_plan(app, plan_id, target_character_id),
    skill_plan_manager::Message::Loaded(roster) => loaded_manage_plans(app, *roster),
    skill_plan_manager::Message::NewPlan(character_id) => {
      open_plan_from_manager(app, character_id, skill_plan_editor::Seed::New)
    }
    skill_plan_manager::Message::NewTemplate => open_template_from_manager(app, skill_plan_editor::Seed::NewTemplate),
    skill_plan_manager::Message::OpenPlan {
      character_id,
      plan_id,
    } => open_plan_from_manager(app, character_id, skill_plan_editor::Seed::Existing(plan_id)),
    skill_plan_manager::Message::OpenTemplate(plan_id) => {
      open_template_from_manager(app, skill_plan_editor::Seed::Existing(plan_id))
    }
    skill_plan_manager::Message::RequestDelete(plan_id) => with_manage_plans(app, |state| state.arm_delete(plan_id)),
    skill_plan_manager::Message::TabSelected(tab) => with_manage_plans(app, move |state| state.set_tab(tab)),
    skill_plan_manager::Message::ToggleCopyMenu(plan_id) => {
      with_manage_plans(app, |state| state.toggle_copy_menu(plan_id))
    }
  }
}
pub(super) fn manage_plans_target_names(app: &App, target_character_id: i64) -> Vec<String> {
  app
    .manage_plans
    .as_ref()
    .map(|(_, state)| {
      state
        .entries()
        .iter()
        .find(|entry| entry.character_id == target_character_id)
        .map(|entry| entry.plans.iter().map(|plan| plan.name.clone()).collect())
        .unwrap_or_default()
    })
    .unwrap_or_default()
}
pub(super) async fn copy_plan_to_character(
  db: &store::Database,
  plan_id: i64,
  target_character_id: i64,
  existing_names: &[String],
) -> Result<i64, store::Error> {
  let Some((_, mut plan)) = skill_plan_editor::read_stored_plan(db, plan_id).await? else {
    return Ok(0);
  };
  plan.name = skill_plan_editor::deduped_name(&plan.name, existing_names);
  skill_plan_editor::persist_onto_character(db, target_character_id, None, &plan).await
}
pub(super) fn open_plan_from_manager(app: &mut App, character_id: i64, seed: skill_plan_editor::Seed) -> Task<Message> {
  navigate(app, Route::Skills(character_id));
  app.selected_character = Some(character_id);
  let owned = owned_pilot_ids(app);
  let switch = match (app.skills.as_mut(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => Task::batch([
      skills::update(state, skills::Message::CharacterChanged(character_id), &runtime.db).map(Message::Skills),
      skills::load(&runtime.db, character_id, owned).map(Message::Skills),
    ]),
    _ => Task::none(),
  };

  Task::batch([switch, open_editor_window(app, Some(character_id), seed).1])
}
pub(super) fn open_template_from_manager(app: &mut App, seed: skill_plan_editor::Seed) -> Task<Message> {
  open_editor_window(app, None, seed).1
}
pub(super) fn close_manage_plans_window(app: &mut App, id: window::Id) -> Task<Message> {
  if app.manage_plans.as_ref().map(|(mid, _)| *mid) == Some(id) {
    app.manage_plans = None;
  }
  app.windows.remove(id);
  window::close(id)
}
pub(super) fn open_budget_rules_window(app: &mut App) -> Task<Message> {
  if app.wallet.is_none() {
    return Task::none();
  }
  if let Some(id) = app.windows.id_for(Window::BudgetRules) {
    return window::gain_focus(id);
  }
  let size = Size::new(
    wallet::budget_rules::BUDGET_RULES_WINDOW_WIDTH,
    wallet::budget_rules::BUDGET_RULES_WINDOW_HEIGHT,
  );
  let (id, open_task) = open_native_window(app, Window::BudgetRules, size);
  app.budget_rules = Some((id, wallet::budget_rules::State::default()));
  open_task
}
pub(super) fn open_budget_rules_editor(app: &mut App, seed: wallet::budget_rules::EditorSeed) -> Task<Message> {
  let open = open_budget_rules_window(app);
  if let (Some((_, state)), Some(wallet)) = (app.budget_rules.as_mut(), app.wallet.as_ref()) {
    state.open_editor(wallet, seed);
  }
  open
}
pub(super) fn handle_budget_rules(app: &mut App, msg: wallet::budget_rules::Message) -> Task<Message> {
  if matches!(msg, wallet::budget_rules::Message::Closed) {
    return match app.windows.id_for(Window::BudgetRules) {
      Some(id) => close_budget_rules_window(app, id),
      None => Task::none(),
    };
  }
  let Some(db) = app.runtime.as_ref().map(|runtime| runtime.db.clone()) else {
    return Task::none();
  };
  match (app.budget_rules.as_mut(), app.wallet.as_mut()) {
    (Some((_, state)), Some(wallet)) => wallet::budget_rules::update(state, wallet, &db, msg).map(Message::Wallet),
    _ => Task::none(),
  }
}
pub(super) fn close_budget_rules_window(app: &mut App, id: window::Id) -> Task<Message> {
  if app.budget_rules.as_ref().map(|(bid, _)| *bid) == Some(id) {
    app.budget_rules = None;
  }
  app.windows.remove(id);
  window::close(id)
}
pub(super) fn budget_rules_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match (app.budget_rules.as_ref(), app.wallet.as_ref()) {
    (Some((rules_id, state)), Some(wallet)) if *rules_id == id => {
      wallet::budget_rules::view(wallet, state).map(Message::BudgetRules)
    }
    _ => blank(),
  }
}
pub(super) fn open_stockpile_editor_window(app: &mut App, seed: assets::EditorSeed) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let editor = assets::Editor::from_seed(seed);
  let scope = editor.scope_query().to_owned();
  let resolve = stockpile_scope_resolve(runtime, scope);
  let size = Size::new(
    assets::STOCKPILE_EDITOR_WINDOW_WIDTH,
    assets::STOCKPILE_EDITOR_WINDOW_HEIGHT,
  );
  let (id, open_task) = open_native_window(app, Window::StockpileEditor, size);
  app.stockpile_editors.insert(id, editor);
  Task::batch([
    open_task,
    resolve.map(move |msg| match msg {
      Message::Assets(assets) => Message::StockpileEditor(id, assets),
      other => other,
    }),
  ])
}
pub(super) fn handle_stockpile_editor(app: &mut App, id: window::Id, msg: assets::Message) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let Some(editor) = app.stockpile_editors.get_mut(id) else {
    return Task::none();
  };
  match assets::apply_editor(editor, msg) {
    assets::EditorEffect::None => Task::none(),
    assets::EditorEffect::ItemSearch(query) => {
      stockpile_item_search(runtime, query).map(move |msg| reroute_to_stockpile_editor(id, msg))
    }
    assets::EditorEffect::LocationSearch {
      generation,
      query,
    } => stockpile_location_search(runtime, query, generation).map(move |msg| reroute_to_stockpile_editor(id, msg)),
    assets::EditorEffect::ScopeResolve(query) => {
      stockpile_scope_resolve(runtime, query).map(move |msg| reroute_to_stockpile_editor(id, msg))
    }
    assets::EditorEffect::Save => {
      let Some(editor) = app.stockpile_editors.get(id).cloned() else {
        return Task::none();
      };
      let save = stockpile_save_window(runtime, editor);
      Task::batch([save, close_stockpile_editor_window(app, id)])
    }
    assets::EditorEffect::Close => close_stockpile_editor_window(app, id),
  }
}
pub(super) fn reroute_to_stockpile_editor(id: window::Id, msg: Message) -> Message {
  match msg {
    Message::Assets(assets) => Message::StockpileEditor(id, assets),
    other => other,
  }
}
pub(super) fn stockpile_save_window(runtime: &Runtime, editor: assets::Editor) -> Task<Message> {
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let image = Arc::clone(&runtime.eve_image);
  let sso = Arc::clone(&runtime.sso);
  Task::perform(
    async move { assets::save_stockpile(db, esi, image, sso, editor).await },
    |cards| Message::Assets(assets::Message::StockpilesReloaded(cards)),
  )
}
pub(super) fn close_stockpile_editor_window(app: &mut App, id: window::Id) -> Task<Message> {
  app.stockpile_editors.remove(id);
  app.windows.remove(id);
  window::close(id)
}
pub(super) fn open_stockpile_import_window(app: &mut App) -> Task<Message> {
  if app.runtime.is_none() {
    return Task::none();
  }

  let close_existing = match app.windows.id_for(Window::StockpileImport) {
    Some(existing) => close_stockpile_import_window(app, existing),
    None => Task::none(),
  };

  let size = Size::new(
    assets::STOCKPILE_IMPORT_WINDOW_WIDTH,
    assets::STOCKPILE_IMPORT_WINDOW_HEIGHT,
  );
  let (id, open_task) = open_native_window(app, Window::StockpileImport, size);
  app.stockpile_imports.insert(id, assets::ImportPanel::blank());
  Task::batch([close_existing, open_task])
}
pub(super) fn handle_stockpile_import(app: &mut App, id: window::Id, msg: assets::Message) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let Some(panel) = app.stockpile_imports.get_mut(id) else {
    return Task::none();
  };
  match assets::apply_import(panel, msg) {
    assets::ImportEffect::None => Task::none(),
    assets::ImportEffect::Resolve(text) => {
      stockpile_import_resolve(runtime, text).map(move |msg| reroute_to_stockpile_import(id, msg))
    }
    assets::ImportEffect::Confirm(matched) => {
      let matched: Vec<assets::MultibuyMatch> = matched;
      let open = open_stockpile_editor_window(app, assets::EditorSeed::Prefill(matched));
      Task::batch([open, close_stockpile_import_window(app, id)])
    }
    assets::ImportEffect::Close => close_stockpile_import_window(app, id),
  }
}
pub(super) fn reroute_to_stockpile_import(id: window::Id, msg: Message) -> Message {
  match msg {
    Message::Assets(assets) => Message::StockpileImport(id, assets),
    other => other,
  }
}
pub(super) fn close_stockpile_import_window(app: &mut App, id: window::Id) -> Task<Message> {
  app.stockpile_imports.remove(id);
  app.windows.remove(id);
  window::close(id)
}
pub(super) fn open_compose_window(app: &mut App, seed: mail::compose::Seed) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let load = match seed.draft_id() {
    Some(draft_id) => {
      let db = runtime.db.clone();
      mail::compose::load_draft(&db, draft_id)
    }
    None => Task::none(),
  };
  let size = Size::new(
    mail::compose::COMPOSE_WINDOW_WIDTH,
    mail::compose::COMPOSE_WINDOW_HEIGHT,
  );
  let (id, open_task) = open_native_window(app, Window::MailCompose, size);
  app.composes.insert(id, mail::compose::Draft::from_seed(seed));
  Task::batch([open_task, load.map(move |msg| Message::Compose(id, msg))])
}
pub(super) fn open_draft_window(app: &mut App, draft_id: i64) -> Task<Message> {
  let Some(from) = app.mail.as_ref().and_then(mail::State::default_from) else {
    return Task::none();
  };
  open_compose_window(
    app,
    mail::compose::Seed::Draft {
      draft_id,
      from_character_id: from,
    },
  )
}
pub(super) fn handle_compose(app: &mut App, id: window::Id, msg: mail::Message) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  match msg {
    mail::Message::DraftLoaded(row) => {
      if let (Some(row), Some(draft)) = (*row, app.composes.get_mut(id)) {
        *draft = mail::compose::Draft::from_persisted(&row);
      }
      return Task::none();
    }
    mail::Message::DraftSaved(saved_id) => {
      if let Some(draft) = app.composes.get_mut(id) {
        draft.set_id(saved_id);
      }
      return Task::none();
    }
    mail::Message::ComposeSent(Ok(())) => return compose_send_completed(app, id),
    _ => {}
  }
  let Some(draft) = app.composes.get_mut(id) else {
    return Task::none();
  };
  match mail::compose::update(draft, msg) {
    mail::compose::Effect::None => Task::none(),
    mail::compose::Effect::RecipientSearch {
      is_to,
      query,
    } => compose_recipient_search(runtime, draft, is_to, query, id),
    mail::compose::Effect::LinkSearch(query) => compose_link_search(runtime, draft, query, id),
    mail::compose::Effect::Send => {
      let send = mail::compose::send(&runtime.db, draft);
      send.map(move |msg| Message::Compose(id, msg))
    }
    mail::compose::Effect::Discard => discard_compose_window(app, id),
  }
}
pub(super) fn discard_compose_window(app: &mut App, id: window::Id) -> Task<Message> {
  app.composes.remove(id);
  app.windows.remove(id);
  window::close(id)
}
pub(super) fn compose_send_completed(app: &mut App, id: window::Id) -> Task<Message> {
  let sent_draft_id = app.composes.get(id).and_then(mail::compose::Draft::sent_draft_id);
  let delete = match (sent_draft_id, app.runtime.as_ref()) {
    (Some(draft_id), Some(runtime)) => {
      let db = runtime.db.clone();
      Task::future(async move { mail::delete_draft(db, draft_id).await }).discard()
    }
    _ => Task::none(),
  };
  let reload = reload_main_mail(app);
  Task::batch([delete, close_compose_window(app, id), reload])
}
pub(super) fn close_compose_window(app: &mut App, id: window::Id) -> Task<Message> {
  let save = match (
    app.composes.get(id).and_then(mail::compose::Draft::pending_save),
    app.runtime.as_ref(),
  ) {
    (Some((draft_id, input)), Some(runtime)) => {
      let db = runtime.db.clone();
      Task::perform(
        async move { mail::persist_pending_draft(db, draft_id, input).await },
        |()| Message::Mail(mail::Message::DraftSaved(None)),
      )
    }
    _ => Task::none(),
  };
  app.composes.remove(id);
  app.windows.remove(id);
  Task::batch([save, window::close(id)])
}
pub(super) fn compose_recipient_search(
  runtime: &Runtime,
  draft: &mail::compose::Draft,
  is_to: bool,
  query: String,
  id: window::Id,
) -> Task<Message> {
  use crate::features::roster::entity_search;

  if query.trim().chars().count() < mail::RECIPIENT_SEARCH_MIN_CHARS {
    return Task::none();
  }
  let generation = draft.recipient_search_generation(is_to);
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let eve_image = Arc::clone(&runtime.eve_image);
  let sso = Arc::clone(&runtime.sso);
  let categories = vec![
    entity_search::EntityCategory::Character,
    entity_search::EntityCategory::Corporation,
  ];
  Task::perform(
    async move { entity_search::search_entities(db, esi, eve_image, sso, categories, query).await },
    move |results| {
      let results = results.into_iter().map(entity_ref_from_result).collect();
      let msg = if is_to {
        mail::Message::ComposeToSearched {
          generation,
          results,
        }
      } else {
        mail::Message::ComposeCcSearched {
          generation,
          results,
        }
      };
      Message::Compose(id, msg)
    },
  )
}
pub(super) fn compose_link_search(
  runtime: &Runtime,
  draft: &mail::compose::Draft,
  query: String,
  id: window::Id,
) -> Task<Message> {
  use crate::features::roster::entity_search;

  let Some((generation, category)) = draft.link_search() else {
    return Task::none();
  };
  if query.trim().chars().count() < mail::RECIPIENT_SEARCH_MIN_CHARS {
    return Task::none();
  }
  let db = runtime.db.clone();
  let esi = Arc::clone(&runtime.esi);
  let eve_image = Arc::clone(&runtime.eve_image);
  let sso = Arc::clone(&runtime.sso);
  Task::perform(
    async move { entity_search::search_entities(db, esi, eve_image, sso, vec![category], query).await },
    move |results| {
      let results = results.into_iter().map(entity_ref_from_result).collect();
      Message::Compose(
        id,
        mail::Message::ComposeLinkSearched {
          generation,
          results,
        },
      )
    },
  )
}
pub(super) fn reload_main_mail(app: &App) -> Task<Message> {
  match (app.mail.as_ref(), app.runtime.as_ref()) {
    (Some(state), Some(runtime)) => mail::reload(&runtime.db, state.active()).map(Message::Mail),
    _ => Task::none(),
  }
}
pub(super) fn open_calendar_event_window(app: &mut App, character_id: i64, event_id: i64) -> Task<Message> {
  let (Some(calendar), Some(runtime)) = (app.calendar.as_ref(), app.runtime.as_ref()) else {
    return Task::none();
  };
  let Some((event, pilot_name)) = calendar.event_for(character_id, event_id) else {
    return Task::none();
  };
  let local_time = calendar.tweaks().local_time();
  let previous_response = event.response.clone();
  let db = runtime.db.clone();

  let size = Size::new(calendar::EVENT_WINDOW_WIDTH, calendar::EVENT_WINDOW_HEIGHT);
  let (id, open_task) = open_native_window(app, Window::CalendarEvent, size);
  app.calendar_events.insert(
    id,
    calendar::EventWindow::new(event, pilot_name, local_time, previous_response),
  );

  Task::batch([
    open_task,
    calendar::load_event_attendees(&db, character_id, event_id).map(move |msg| Message::CalendarEvent(id, msg)),
  ])
}
pub(super) fn handle_calendar_event(app: &mut App, id: window::Id, msg: calendar::EventMessage) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let reload_main = matches!(msg, calendar::EventMessage::RsvpWritten);
  let db = runtime.db.clone();

  let Some(window) = app.calendar_events.get_mut(id) else {
    return Task::none();
  };
  let window_task = calendar::event_window_update(window, msg, &db).map(move |msg| Message::CalendarEvent(id, msg));

  if reload_main && let (Some(state), Some(runtime)) = (app.calendar.as_ref(), app.runtime.as_ref()) {
    let reload = calendar::reload(&runtime.db, state.active(), *runtime.settings.features()).map(Message::Calendar);
    return Task::batch([window_task, reload]);
  }
  window_task
}
pub(super) fn close_calendar_event_window(app: &mut App, id: window::Id) -> Task<Message> {
  app.calendar_events.remove(id);
  app.windows.remove(id);
  window::close(id)
}
pub(super) fn calendar_event_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.calendar_events.get(id) {
    Some(window) => calendar::event_window_view(window).map(move |msg| Message::CalendarEvent(id, msg)),
    None => blank(),
  }
}
pub(super) fn stockpile_import_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.stockpile_imports.get(id) {
    Some(panel) => assets::stockpile_import_view(panel).map(move |msg| Message::StockpileImport(id, msg)),
    None => blank(),
  }
}
pub(super) fn splash_window_view(app: &App) -> Element<'_, Message> {
  match app.splash.as_ref() {
    Some(state) => splash::view(state, app.now).map(Message::Splash),
    None => blank(),
  }
}
pub(super) fn first_run_window_view(app: &App) -> Element<'_, Message> {
  match app.wizard.as_ref() {
    Some(state) => wizard::view(state).map(Message::Wizard),
    None => blank(),
  }
}
pub(super) fn compare_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.compare.as_ref() {
    Some((compare_id, state)) if *compare_id == id => skills_compare::view(state).map(Message::Compare),
    _ => blank(),
  }
}
pub(super) fn contract_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.contracts.get(id) {
    Some(state) => contract_detail::view(state),
    None => blank(),
  }
}
pub(super) fn killmail_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.killmails.get(id) {
    Some(state) => killmail_detail::view(state),
    None => blank(),
  }
}
pub(super) fn compose_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.composes.get(id) {
    Some(draft) => {
      let roster = app.mail.as_ref().map(mail::State::roster).unwrap_or(&[]);
      mail::compose::view(draft, roster).map(move |msg| Message::Compose(id, msg))
    }
    None => blank(),
  }
}
pub(super) fn manage_plans_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.manage_plans.as_ref() {
    Some((manage_id, state)) if *manage_id == id => skill_plan_manager::view(state).map(Message::ManagePlans),
    _ => blank(),
  }
}
pub(super) fn skill_plan_editor_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.editors.get(id) {
    Some(state) => skill_plan_editor::view(state, app.now).map(move |msg| Message::SkillPlanEditor(id, msg)),
    None => blank(),
  }
}
pub(super) fn stockpile_editor_window_view(app: &App, id: window::Id) -> Element<'_, Message> {
  match app.stockpile_editors.get(id) {
    Some(editor) => assets::stockpile_editor_view(editor).map(move |msg| Message::StockpileEditor(id, msg)),
    None => blank(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_support::*;

  mod usage_token {
    use super::*;

    #[test]
    fn every_user_window_has_a_well_formed_view_open_token() {
      let windows = [
        Window::BudgetRules,
        Window::CalendarEvent,
        Window::Compare,
        Window::Contract,
        Window::FirstRun,
        Window::Killmail,
        Window::MailCompose,
        Window::Main,
        Window::ManagePlans,
        Window::SkillPlanEditor,
        Window::Splash,
        Window::StockpileEditor,
        Window::StockpileImport,
      ];
      for window in windows {
        match window {
          Window::Main | Window::SkillPlanEditor | Window::Splash => {
            assert!(window.usage_token().is_none(), "{window:?} is tokenized elsewhere");
          }
          _ => {
            let token = window.usage_token().expect("user window carries a token");
            assert!(telemetry::is_well_formed_token(token), "malformed token: {token}");
          }
        }
      }
    }

    #[test]
    fn the_editor_open_site_tokens_are_well_formed() {
      assert!(telemetry::is_well_formed_token("skills.plan_editor"));
      assert!(telemetry::is_well_formed_token("skills.template_editor"));
    }
  }

  mod ids_for {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_yields_every_window_of_a_kind_that_allows_duplicates() {
      let mut windows = Windows::default();
      let first = window::Id::unique();
      let second = window::Id::unique();
      windows.register(first, Window::Killmail);
      windows.register(second, Window::Killmail);
      windows.register(window::Id::unique(), Window::Main);

      let mut killmails: Vec<window::Id> = windows.ids_for(Window::Killmail).collect();
      killmails.sort();
      let mut expected = vec![first, second];
      expected.sort();

      assert_eq!(killmails, expected);
    }

    #[test]
    fn it_yields_nothing_when_no_window_of_the_kind_is_open() {
      let mut windows = Windows::default();
      windows.register(window::Id::unique(), Window::Main);

      assert_eq!(windows.ids_for(Window::Killmail).count(), 0);
    }
  }

  mod is_empty {
    use super::*;

    #[test]
    fn it_becomes_empty_again_once_the_last_window_is_removed() {
      let mut windows = Windows::default();
      let main = window::Id::unique();
      let editor = window::Id::unique();
      windows.register(main, Window::Main);
      windows.register(editor, Window::SkillPlanEditor);

      windows.remove(main);
      assert!(!windows.is_empty(), "one window still open keeps the app alive");

      windows.remove(editor);
      assert!(windows.is_empty(), "removing the final window empties the registry");
    }

    #[test]
    fn it_is_empty_before_any_window_registers() {
      let windows = Windows::default();

      assert!(windows.is_empty());
    }

    #[test]
    fn it_is_not_empty_while_a_window_is_registered() {
      let mut windows = Windows::default();
      windows.register(window::Id::unique(), Window::Main);

      assert!(!windows.is_empty());
    }
  }

  mod state_key {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_gives_compare_and_the_editor_distinct_keys() {
      assert_ne!(Window::Compare.state_key(), Window::SkillPlanEditor.state_key());
    }

    #[test]
    fn it_never_persists_killmail() {
      assert_eq!(Window::Killmail.state_key(), None);
    }

    #[test]
    fn it_never_persists_the_detached_child_windows() {
      assert_eq!(Window::Contract.state_key(), None);
      assert_eq!(Window::MailCompose.state_key(), None);
      assert_eq!(Window::StockpileEditor.state_key(), None);
    }

    #[test]
    fn it_never_persists_calendar_event() {
      assert_eq!(Window::CalendarEvent.state_key(), None);
    }

    #[test]
    fn it_maps_budget_rules_to_a_stable_key() {
      assert_eq!(Window::BudgetRules.state_key(), Some("budget_rules"));
    }

    #[test]
    fn it_maps_manage_plans_to_a_stable_key() {
      assert_eq!(Window::ManagePlans.state_key(), Some("skill_plan_manager"));
    }

    #[test]
    fn it_maps_stockpile_import_to_a_stable_key() {
      assert_eq!(Window::StockpileImport.state_key(), Some("stockpile_import"));
    }

    #[test]
    fn it_gives_main_and_the_editor_distinct_keys() {
      assert_ne!(Window::Main.state_key(), Window::SkillPlanEditor.state_key());
    }

    #[test]
    fn it_maps_compare_to_a_stable_key() {
      assert_eq!(Window::Compare.state_key(), Some("skills_compare"));
    }

    #[test]
    fn it_maps_main_to_a_stable_key() {
      assert_eq!(Window::Main.state_key(), Some("main"));
    }

    #[test]
    fn it_maps_the_skill_plan_editor_to_a_stable_key() {
      assert_eq!(Window::SkillPlanEditor.state_key(), Some("skill_plan_editor"));
    }

    #[test]
    fn it_never_persists_splash() {
      assert_eq!(Window::Splash.state_key(), None);
    }

    #[test]
    fn it_never_persists_first_run() {
      assert_eq!(Window::FirstRun.state_key(), None);
    }
  }

  mod window_states {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_holds_two_states_under_distinct_ids() {
      let mut states: WindowStates<&str> = WindowStates::default();
      let first = window::Id::unique();
      let second = window::Id::unique();
      states.insert(first, "alpha");
      states.insert(second, "beta");

      assert_eq!(states.len(), 2);
      assert_eq!(states.get(first), Some(&"alpha"));
      assert_eq!(states.get(second), Some(&"beta"));
    }

    #[test]
    fn it_is_empty_before_any_state_is_inserted() {
      let states: WindowStates<u8> = WindowStates::default();

      assert!(states.is_empty());
    }

    #[test]
    fn it_mutates_the_state_for_a_given_id() {
      let mut states: WindowStates<u32> = WindowStates::default();
      let id = window::Id::unique();
      states.insert(id, 1);

      if let Some(value) = states.get_mut(id) {
        *value = 7;
      }

      assert_eq!(states.get(id), Some(&7));
    }

    #[test]
    fn it_removes_one_state_without_disturbing_the_other() {
      let mut states: WindowStates<&str> = WindowStates::default();
      let first = window::Id::unique();
      let second = window::Id::unique();
      states.insert(first, "alpha");
      states.insert(second, "beta");

      let removed = states.remove(first);

      assert_eq!(removed, Some("alpha"));
      assert_eq!(states.len(), 1);
      assert_eq!(states.get(second), Some(&"beta"));
    }
  }

  mod geometry_merge {
    use pretty_assertions::assert_eq;

    use super::*;

    fn base() -> WindowGeometry {
      WindowGeometry {
        height: 700.0,
        width: 1000.0,
        x: 50.0,
        y: 60.0,
      }
    }

    #[test]
    fn it_seeds_from_zero_when_the_window_has_no_prior_entry() {
      let resized = geometry_after_resize(None, Size::new(800.0, 600.0));
      assert_eq!(resized.width, 800.0);
      assert_eq!(resized.height, 600.0);
      assert_eq!((resized.x, resized.y), (0.0, 0.0));
    }

    #[test]
    fn it_updates_only_the_position_on_a_move_keeping_the_size() {
      let merged = geometry_after_move(Some(base()), Point::new(200.0, 300.0));

      assert_eq!(
        merged,
        WindowGeometry {
          height: 700.0,
          width: 1000.0,
          x: 200.0,
          y: 300.0,
        }
      );
    }

    #[test]
    fn it_updates_only_the_size_on_a_resize_keeping_the_position() {
      let merged = geometry_after_resize(Some(base()), Size::new(1280.0, 960.0));

      assert_eq!(
        merged,
        WindowGeometry {
          height: 960.0,
          width: 1280.0,
          x: 50.0,
          y: 60.0,
        }
      );
    }
  }

  mod killmail_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn detail(killmail_id: i64) -> killmail_detail::KillmailDetail {
      killmail_detail::KillmailDetail {
        attackers: Vec::new(),
        damage_taken: 0,
        dropped_isk: 0.0,
        is_kill: true,
        kill_time: "2024-01-01T00:00:00Z".to_owned(),
        killmail_id,
        ship_icon: store::images::IconResolution::Missing,
        ship_name: "Rifter".to_owned(),
        slots: Vec::new(),
        system_name: None,
        system_security: 0.0,
        value_destroyed_isk: 0.0,
        value_isk: 0.0,
        victim_alliance: None,
        victim_corp: None,
        victim_name: "Target".to_owned(),
        victim_portrait: store::images::ImageState::Fresh("/tmp/p.jpg".into()),
      }
    }

    fn ready(app: &mut App, source: killmail_detail::Source, killmail_id: i64) -> window::Id {
      let id = window::Id::unique();
      app.windows.register(id, Window::Killmail);
      app
        .killmails
        .insert(id, killmail_detail::State::new(source, killmail_id));
      id
    }

    #[tokio::test]
    async fn it_registers_the_kind_and_seeds_the_per_window_state() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let id = ready(
        &mut app,
        killmail_detail::Source::Character {
          character_id: 42,
        },
        100,
      );

      assert_eq!(app.windows.kind(id), Some(Window::Killmail));
      assert_eq!(
        app.killmails.get(id).map(killmail_detail::State::killmail_id),
        Some(100)
      );
    }

    #[tokio::test]
    async fn it_holds_duplicate_killmails_under_distinct_ids() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let source = killmail_detail::Source::Corporation {
        corporation_id: 7,
      };

      let first = ready(&mut app, source, 100);
      let second = ready(&mut app, source, 100);

      assert_ne!(first, second);
      assert_eq!(app.killmails.len(), 2);
      assert_eq!(app.windows.ids_for(Window::Killmail).count(), 2);
    }

    #[tokio::test]
    async fn it_routes_a_loaded_detail_to_only_its_own_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let source = killmail_detail::Source::Character {
        character_id: 42,
      };
      let first = ready(&mut app, source, 100);
      let second = ready(&mut app, source, 200);

      let _ = handle_killmail(
        &mut app,
        first,
        killmail_detail::Message::Loaded(Box::new(Some(detail(100)))),
      );

      assert_eq!(
        app
          .killmails
          .get(first)
          .and_then(killmail_detail::State::loaded_killmail_id),
        Some(100)
      );
      assert_eq!(
        app
          .killmails
          .get(second)
          .and_then(killmail_detail::State::loaded_killmail_id),
        None
      );
    }

    #[tokio::test]
    async fn it_closes_only_the_targeted_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let source = killmail_detail::Source::Character {
        character_id: 42,
      };
      let first = ready(&mut app, source, 100);
      let second = ready(&mut app, source, 200);

      let _ = close_killmail_window(&mut app, first);

      assert_eq!(app.windows.kind(first), None);
      assert!(app.killmails.get(first).is_none());
      assert_eq!(app.windows.kind(second), Some(Window::Killmail));
      assert!(app.killmails.get(second).is_some());
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_a_killmail_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(
        &mut app,
        killmail_detail::Source::Character {
          character_id: 42,
        },
        100,
      );

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.killmails.get(id).is_none());
    }
  }

  mod budget_rules_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn wallet_state() -> wallet::State {
      wallet::State::new(crate::config::FeatureFlags::default())
    }

    #[test]
    fn it_does_not_open_without_a_wallet_screen() {
      let mut app = test_app();

      let _ = open_budget_rules_window(&mut app);

      assert!(app.budget_rules.is_none());
      assert!(app.windows.id_for(Window::BudgetRules).is_none());
    }

    #[test]
    fn it_opens_a_singleton_and_focuses_the_existing_window() {
      let mut app = test_app();
      app.wallet = Some(wallet_state());

      let _ = open_budget_rules_window(&mut app);
      let first = app.windows.id_for(Window::BudgetRules);
      assert!(first.is_some());
      assert!(app.budget_rules.is_some());

      let _ = open_budget_rules_window(&mut app);

      assert_eq!(app.windows.ids_for(Window::BudgetRules).count(), 1);
      assert_eq!(app.windows.id_for(Window::BudgetRules), first);
    }

    #[test]
    fn it_seeds_the_window_state_when_opened_from_the_inspector() {
      let mut app = test_app();
      app.wallet = Some(wallet_state());

      let _ = open_budget_rules_editor(&mut app, wallet::budget_rules::EditorSeed::New(7));

      assert!(app.budget_rules.is_some());
      assert!(app.windows.id_for(Window::BudgetRules).is_some());
    }

    #[test]
    fn it_drops_the_state_when_the_window_closes() {
      let mut app = test_app();
      app.windows.register(window::Id::unique(), Window::Main);
      app.wallet = Some(wallet_state());
      let _ = open_budget_rules_window(&mut app);
      let id = app.windows.id_for(Window::BudgetRules).unwrap();

      let _ = on_window_closed(&mut app, id);

      assert!(app.budget_rules.is_none());
      assert_eq!(app.windows.kind(id), None);
    }

    #[tokio::test]
    async fn it_routes_window_messages_through_the_app_dispatch() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.wallet = Some(wallet_state());
      let _ = open_budget_rules_window(&mut app);

      let _ = update(
        &mut app,
        Message::BudgetRules(wallet::budget_rules::Message::DragStarted(1)),
      );
      let _ = update(
        &mut app,
        Message::BudgetRules(wallet::budget_rules::Message::DropReleased),
      );

      assert!(app.budget_rules.is_some(), "routing a message keeps the window state");
    }

    #[tokio::test]
    async fn it_closes_through_the_header_close_message() {
      let mut app = test_app();
      app.windows.register(window::Id::unique(), Window::Main);
      app.runtime = Some(test_runtime().await);
      app.wallet = Some(wallet_state());
      let _ = open_budget_rules_window(&mut app);

      let _ = handle_budget_rules(&mut app, wallet::budget_rules::Message::Closed);

      assert!(app.budget_rules.is_none());
      assert!(app.windows.id_for(Window::BudgetRules).is_none());
    }

    #[test]
    fn it_ignores_a_header_close_when_no_window_is_open() {
      let mut app = test_app();

      let _ = handle_budget_rules(&mut app, wallet::budget_rules::Message::Closed);

      assert!(app.budget_rules.is_none());
    }

    #[test]
    fn it_ignores_messages_without_a_runtime() {
      let mut app = test_app();
      app.wallet = Some(wallet_state());
      let _ = open_budget_rules_window(&mut app);

      let _ = handle_budget_rules(&mut app, wallet::budget_rules::Message::DropTargetLeft);

      assert!(app.budget_rules.is_some());
    }

    #[test]
    fn it_closes_through_the_close_request_arm() {
      let mut app = test_app();
      app.windows.register(window::Id::unique(), Window::Main);
      app.wallet = Some(wallet_state());
      let _ = open_budget_rules_window(&mut app);
      let id = app.windows.id_for(Window::BudgetRules).unwrap();

      let _ = handle_close_requested(&mut app, id);

      assert!(app.budget_rules.is_none());
      assert_eq!(app.windows.kind(id), None);
    }
  }

  mod calendar_event_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn event(character_id: i64, event_id: i64, title: &str) -> calendar::CalendarEvent {
      calendar::CalendarEvent {
        body: Some("<p>Form up.</p>".to_owned()),
        character_id,
        duration_minutes: 90,
        event_id,
        importance: 0,
        owner_name: "Corp".to_owned(),
        owner_type: "corporation".to_owned(),
        response: "not_responded".to_owned(),
        source: None,
        timestamp: "2026-06-20T19:00:00Z".to_owned(),
        title: title.to_owned(),
      }
    }

    fn ready(app: &mut App, character_id: i64, event_id: i64, title: &str) -> window::Id {
      let id = window::Id::unique();
      app.windows.register(id, Window::CalendarEvent);
      app.calendar_events.insert(
        id,
        calendar::EventWindow::new(
          event(character_id, event_id, title),
          Some("Pilot".to_owned()),
          false,
          "not_responded".to_owned(),
        ),
      );
      id
    }

    #[tokio::test]
    async fn it_holds_several_event_windows_under_distinct_ids() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let first = ready(&mut app, 1, 10, "Op Alpha");
      let second = ready(&mut app, 1, 10, "Op Alpha");

      assert_ne!(first, second);
      assert_eq!(app.calendar_events.len(), 2);
      assert_eq!(app.windows.ids_for(Window::CalendarEvent).count(), 2);
    }

    #[tokio::test]
    async fn it_titles_the_window_with_the_event_subject() {
      let mut app = test_app();
      let id = ready(&mut app, 1, 10, "Doctrine refit night");

      assert_eq!(window_title(&app, id), "Pod \u{2014} Doctrine refit night");
    }

    #[tokio::test]
    async fn it_renders_the_event_window_body() {
      let mut app = test_app();
      let id = ready(&mut app, 1, 10, "Op Alpha");

      let _el: Element<'_, Message> = view(&app, id);
    }

    #[tokio::test]
    async fn it_routes_a_per_window_message_to_only_its_own_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let first = ready(&mut app, 1, 10, "Op Alpha");
      let second = ready(&mut app, 1, 20, "Op Beta");

      let _ = handle_calendar_event(
        &mut app,
        first,
        calendar::EventMessage::AttendeesLoaded(Box::new(Some(store::model::AttendeeTally {
          accepted: 2,
          declined: 0,
          invited: 4,
          tentative: 1,
        }))),
      );
      let _ = handle_calendar_event(
        &mut app,
        first,
        calendar::EventMessage::Responded(calendar::Response::Accepted),
      );
      let _ = handle_calendar_event(&mut app, first, calendar::EventMessage::RsvpWritten);

      assert_eq!(window_title(&app, second), "Pod \u{2014} Op Beta");
    }

    #[tokio::test]
    async fn it_closes_only_the_targeted_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let first = ready(&mut app, 1, 10, "Op Alpha");
      let second = ready(&mut app, 1, 20, "Op Beta");

      let _ = close_calendar_event_window(&mut app, first);

      assert_eq!(app.windows.kind(first), None);
      assert!(app.calendar_events.get(first).is_none());
      assert_eq!(app.windows.kind(second), Some(Window::CalendarEvent));
      assert!(app.calendar_events.get(second).is_some());
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_the_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(&mut app, 1, 10, "Op Alpha");

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.calendar_events.get(id).is_none());
    }
  }

  mod manage_plans_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn ready(app: &mut App) -> window::Id {
      let _ = open_manage_plans_window(app);
      app.manage_plans.as_ref().map(|(id, _)| *id).expect("window registered")
    }

    #[tokio::test]
    async fn it_registers_the_kind_and_seeds_the_per_window_state() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let id = ready(&mut app);

      assert_eq!(app.windows.kind(id), Some(Window::ManagePlans));
      assert_eq!(app.manage_plans.as_ref().map(|(mid, _)| *mid), Some(id));
    }

    #[tokio::test]
    async fn it_focuses_the_existing_window_instead_of_opening_a_second() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let first = ready(&mut app);

      let _ = open_manage_plans_window(&mut app);

      assert_eq!(app.windows.ids_for(Window::ManagePlans).count(), 1);
      assert_eq!(app.manage_plans.as_ref().map(|(mid, _)| *mid), Some(first));
    }

    #[tokio::test]
    async fn it_drops_the_state_on_close() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(&mut app);

      let _ = close_manage_plans_window(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.manage_plans.is_none());
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_the_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(&mut app);

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.manage_plans.is_none());
    }

    async fn seed_owned(db: &store::Database, id: i64) {
      use crate::store::{
        model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
        repo::{character, infra},
      };

      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
      infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();
    }

    async fn ready_with_roster(app: &mut App) -> window::Id {
      let id = ready(app);
      let db = app.runtime.as_ref().unwrap().db.clone();
      let roster = skill_plan_manager::load_roster(&db).await;
      let _ = handle_manage_plans(app, skill_plan_manager::Message::Loaded(Box::new(roster)));
      id
    }

    #[tokio::test]
    async fn open_switches_the_active_character_seeds_the_editor_and_keeps_the_manager_open() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.skills = Some(skills::State::new(7));
      app.windows.register(window::Id::unique(), Window::Main);
      let db = app.runtime.as_ref().unwrap().db.clone();
      seed_owned(&db, 42).await;
      let plan = store::repo::skills::create(&db, 42, "Combat").await.unwrap();
      let id = ready_with_roster(&mut app).await;

      let _ = handle_manage_plans(
        &mut app,
        skill_plan_manager::Message::OpenPlan {
          character_id: 42,
          plan_id: plan.id(),
        },
      );

      assert_eq!(
        app.windows.kind(id),
        Some(Window::ManagePlans),
        "the manager stays open"
      );
      assert_eq!(app.manage_plans.as_ref().map(|(mid, _)| *mid), Some(id));
      assert_eq!(app.skills.as_ref().map(skills::State::active), Some(42));
      let (eid, editor) = app.editors.iter().next().expect("editor window opened");
      assert_eq!(app.windows.kind(eid), Some(Window::SkillPlanEditor));
      assert_eq!(editor.character_id(), Some(42));
    }

    #[tokio::test]
    async fn new_seeds_an_editor_for_the_selected_character_and_keeps_the_manager_open() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.skills = Some(skills::State::new(7));
      app.windows.register(window::Id::unique(), Window::Main);
      let db = app.runtime.as_ref().unwrap().db.clone();
      seed_owned(&db, 42).await;
      let id = ready_with_roster(&mut app).await;

      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::NewPlan(42));

      assert_eq!(
        app.windows.kind(id),
        Some(Window::ManagePlans),
        "the manager stays open"
      );
      assert_eq!(app.skills.as_ref().map(skills::State::active), Some(42));
      let (_, editor) = app.editors.iter().next().expect("editor window opened");
      assert_eq!(editor.character_id(), Some(42));
    }

    #[tokio::test]
    async fn request_delete_arms_the_confirm_and_confirm_clears_it() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let db = app.runtime.as_ref().unwrap().db.clone();
      seed_owned(&db, 42).await;
      let plan = store::repo::skills::create(&db, 42, "Combat").await.unwrap();
      let _ = ready_with_roster(&mut app).await;

      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::RequestDelete(plan.id()));
      assert_eq!(app.manage_plans.as_ref().unwrap().1.confirm_delete(), Some(plan.id()));

      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::ConfirmDelete(plan.id()));
      assert_eq!(app.manage_plans.as_ref().unwrap().1.confirm_delete(), None);
    }

    #[tokio::test]
    async fn copy_clones_the_full_plan_onto_the_target_with_name_de_dup() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, 42).await;
      seed_owned(&db, 7).await;
      let source = store::repo::skills::create(&db, 42, "Combat").await.unwrap();
      store::repo::skills::replace_entries(&db, source.id(), &[(3300, 5, "high", "core", 0)])
        .await
        .unwrap();
      store::repo::skills::create(&db, 7, "Combat").await.unwrap();

      let clone_id = copy_plan_to_character(&db, source.id(), 7, &["Combat".to_owned()])
        .await
        .unwrap();

      let clone = store::repo::skills::get(&db, clone_id).await.unwrap().unwrap();
      assert_eq!(clone.name(), "Combat (2)", "name de-duped against the target");
      assert_eq!(clone.character_id(), Some(7));
      let entries = store::repo::skills::entries(&db, clone_id).await.unwrap();
      assert_eq!(entries.iter().map(|e| e.skill_id()).collect::<Vec<_>>(), [3300]);
      assert_eq!(entries[0].to_level(), 5);
    }
  }

  mod skill_plan_editor_window {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn ready(app: &mut App) {
      app.runtime = Some(test_runtime().await);
      app.skills = Some(skills::State::new(7));
      app.windows.register(window::Id::unique(), Window::Main);
    }

    #[tokio::test]
    async fn it_opens_independent_windows_for_two_different_plans() {
      let mut app = test_app();
      ready(&mut app).await;

      let (first, _) = open_editor_window(&mut app, Some(1), skill_plan_editor::Seed::Existing(10));
      let (second, _) = open_editor_window(&mut app, Some(1), skill_plan_editor::Seed::Existing(20));

      let first = first.expect("first editor opened");
      let second = second.expect("second editor opened");
      assert_ne!(first, second);
      assert_eq!(app.editors.len(), 2);
      assert_eq!(app.windows.ids_for(Window::SkillPlanEditor).count(), 2);
    }

    #[tokio::test]
    async fn it_focuses_an_already_open_plan_instead_of_duplicating_it() {
      let mut app = test_app();
      ready(&mut app).await;

      let (first, _) = open_editor_window(&mut app, Some(1), skill_plan_editor::Seed::Existing(10));
      let (again, _) = open_editor_window(&mut app, Some(1), skill_plan_editor::Seed::Existing(10));

      assert_eq!(again, first, "reopening the same plan focuses its window");
      assert_eq!(app.editors.len(), 1, "no duplicate editor is spawned");
    }

    #[tokio::test]
    async fn it_opens_a_fresh_window_for_each_new_draft() {
      let mut app = test_app();
      ready(&mut app).await;

      let (first, _) = open_editor_window(&mut app, Some(1), skill_plan_editor::Seed::New);
      let (second, _) = open_editor_window(&mut app, Some(1), skill_plan_editor::Seed::New);

      assert_ne!(first.expect("first draft"), second.expect("second draft"));
      assert_eq!(app.editors.len(), 2);
    }

    #[tokio::test]
    async fn it_is_a_no_op_without_a_runtime() {
      let mut app = test_app();

      let (id, _) = open_editor_window(&mut app, Some(1), skill_plan_editor::Seed::New);

      assert!(id.is_none());
      assert!(app.editors.is_empty());
    }

    #[tokio::test]
    async fn closing_one_editor_leaves_the_others_open() {
      let mut app = test_app();
      ready(&mut app).await;
      let first = open_editor_window(&mut app, Some(1), skill_plan_editor::Seed::Existing(10))
        .0
        .expect("first editor opened");
      let second = open_editor_window(&mut app, Some(1), skill_plan_editor::Seed::Existing(20))
        .0
        .expect("second editor opened");

      let _ = close_editor_window(&mut app, first);

      assert!(app.editors.get(first).is_none(), "the closed editor is dropped");
      assert_eq!(app.windows.kind(first), None);
      assert!(app.editors.get(second).is_some(), "the other editor stays open");
      assert_eq!(app.windows.kind(second), Some(Window::SkillPlanEditor));
    }

    #[tokio::test]
    async fn an_os_close_drops_only_that_editor() {
      let mut app = test_app();
      ready(&mut app).await;
      let first = open_editor_window(&mut app, Some(1), skill_plan_editor::Seed::Existing(10))
        .0
        .expect("first editor opened");
      let second = open_editor_window(&mut app, Some(1), skill_plan_editor::Seed::Existing(20))
        .0
        .expect("second editor opened");

      let _ = on_window_closed(&mut app, first);

      assert!(app.editors.get(first).is_none());
      assert!(app.editors.get(second).is_some());
    }
  }

  mod manager_reload_on_editor_close {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn ready(app: &mut App) {
      app.runtime = Some(test_runtime().await);
      app.skills = Some(skills::State::new(7));
      app.windows.register(window::Id::unique(), Window::Main);
    }

    #[tokio::test]
    async fn the_gate_opens_only_while_the_manager_window_exists() {
      let mut app = test_app();
      ready(&mut app).await;
      assert!(!manage_plans_open(&app), "a closed manager keeps the reload gate shut");

      app.manage_plans = Some((window::Id::unique(), skill_plan_manager::State::new()));
      assert!(manage_plans_open(&app), "an open manager opens the reload gate");
    }

    #[tokio::test]
    async fn closing_an_editor_keeps_the_open_manager_to_receive_the_reload() {
      let mut app = test_app();
      ready(&mut app).await;
      let manager_id = window::Id::unique();
      app.manage_plans = Some((manager_id, skill_plan_manager::State::new()));
      let editor = open_editor_window(&mut app, Some(1), skill_plan_editor::Seed::Existing(10))
        .0
        .expect("editor opened");

      let _ = close_editor_window(&mut app, editor);

      assert!(app.editors.get(editor).is_none(), "the closed editor is dropped");
      assert_eq!(
        app.manage_plans.as_ref().map(|(mid, _)| *mid),
        Some(manager_id),
        "the manager stays open so the roster reload lands",
      );
    }

    #[tokio::test]
    async fn closing_an_editor_with_no_manager_open_touches_no_manager_state() {
      let mut app = test_app();
      ready(&mut app).await;
      let editor = open_editor_window(&mut app, Some(1), skill_plan_editor::Seed::Existing(10))
        .0
        .expect("editor opened");

      let _ = close_editor_window(&mut app, editor);

      assert!(
        app.manage_plans.is_none(),
        "no manager reload path is taken when the manager is closed"
      );
    }
  }

  mod contract_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn detail(contract_id: i64) -> contract_detail::ContractDetail {
      contract_detail::ContractDetail {
        acceptor: None,
        availability: "Public".to_owned(),
        bids: Vec::new(),
        buyout: None,
        collateral: None,
        contract_id,
        days_to_complete: Some(0),
        expiry: contract_detail::ExpiryView {
          future: true,
          label: "Open".to_owned(),
          title: "Expires",
        },
        headline: 200.0,
        headline_label: "Price",
        issued_time: "2024-01-01T00:00:00Z".to_owned(),
        issuer: contract_detail::PartyView {
          name: "Issuer Pilot".to_owned(),
          portrait: store::images::ImageState::Fresh("/tmp/p.jpg".into()),
          role: "Issuer",
          sub: None,
        },
        items: Vec::new(),
        items_value: 0.0,
        kind: contract_detail::ContractKind::ItemExchange,
        location_name: "Jita IV - Moon 4".to_owned(),
        route: None,
        status: "outstanding".to_owned(),
        title: "Test Contract".to_owned(),
        volume: 0.0,
      }
    }

    fn ready(app: &mut App, source: contract_detail::Source, contract_id: i64) -> window::Id {
      let id = window::Id::unique();
      app.windows.register(id, Window::Contract);
      app
        .contracts
        .insert(id, contract_detail::State::new(source, contract_id));
      id
    }

    #[tokio::test]
    async fn it_registers_the_kind_and_seeds_the_per_window_state() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let id = ready(
        &mut app,
        contract_detail::Source::Character {
          character_id: 42,
        },
        100,
      );

      assert_eq!(app.windows.kind(id), Some(Window::Contract));
      assert_eq!(
        app.contracts.get(id).map(contract_detail::State::contract_id),
        Some(100)
      );
    }

    #[tokio::test]
    async fn it_holds_duplicate_contracts_under_distinct_ids() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let source = contract_detail::Source::Corporation {
        corporation_id: 7,
      };

      let first = ready(&mut app, source, 100);
      let second = ready(&mut app, source, 100);

      assert_ne!(first, second);
      assert_eq!(app.contracts.len(), 2);
      assert_eq!(app.windows.ids_for(Window::Contract).count(), 2);
    }

    #[tokio::test]
    async fn it_routes_a_loaded_detail_to_only_its_own_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let source = contract_detail::Source::Character {
        character_id: 42,
      };
      let first = ready(&mut app, source, 100);
      let second = ready(&mut app, source, 200);

      let _ = handle_contract(
        &mut app,
        first,
        contract_detail::Message::Loaded(Box::new(Some(detail(100)))),
      );

      assert_eq!(
        app
          .contracts
          .get(first)
          .and_then(contract_detail::State::loaded_contract_id),
        Some(100)
      );
      assert_eq!(
        app
          .contracts
          .get(second)
          .and_then(contract_detail::State::loaded_contract_id),
        None
      );
    }

    #[tokio::test]
    async fn it_closes_only_the_targeted_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let source = contract_detail::Source::Character {
        character_id: 42,
      };
      let first = ready(&mut app, source, 100);
      let second = ready(&mut app, source, 200);

      let _ = close_contract_window(&mut app, first);

      assert_eq!(app.windows.kind(first), None);
      assert!(app.contracts.get(first).is_none());
      assert_eq!(app.windows.kind(second), Some(Window::Contract));
      assert!(app.contracts.get(second).is_some());
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_a_contract_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(
        &mut app,
        contract_detail::Source::Character {
          character_id: 42,
        },
        100,
      );

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.contracts.get(id).is_none());
    }

    #[tokio::test]
    async fn it_registers_and_seeds_synchronously_via_the_native_opener() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let _ = open_contract_window(
        &mut app,
        contract_detail::Source::Character {
          character_id: 42,
        },
        100,
      );

      let id = app
        .windows
        .ids_for(Window::Contract)
        .next()
        .expect("contract window registered");
      assert_eq!(
        app.contracts.get(id).map(contract_detail::State::contract_id),
        Some(100)
      );
    }

    #[test]
    fn it_is_a_no_op_without_a_runtime() {
      let mut app = test_app();

      let _ = open_contract_window(
        &mut app,
        contract_detail::Source::Character {
          character_id: 42,
        },
        100,
      );

      assert_eq!(app.windows.ids_for(Window::Contract).count(), 0);
      assert_eq!(app.contracts.len(), 0);
    }
  }

  mod stockpile_editor_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn ready(app: &mut App, seed: assets::EditorSeed) -> window::Id {
      let id = window::Id::unique();
      app.windows.register(id, Window::StockpileEditor);
      app.stockpile_editors.insert(id, assets::Editor::from_seed(seed));
      id
    }

    #[tokio::test]
    async fn it_registers_the_kind_and_seeds_a_blank_new_editor() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let id = ready(&mut app, assets::EditorSeed::Blank);

      assert_eq!(app.windows.kind(id), Some(Window::StockpileEditor));
      assert!(app.stockpile_editors.get(id).is_some());
    }

    #[tokio::test]
    async fn it_holds_a_new_and_an_edit_window_at_once() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let new = ready(&mut app, assets::EditorSeed::Blank);
      let edit = ready(&mut app, assets::EditorSeed::Blank);

      assert_ne!(new, edit);
      assert_eq!(app.stockpile_editors.len(), 2);
      assert_eq!(app.windows.ids_for(Window::StockpileEditor).count(), 2);
    }

    #[tokio::test]
    async fn it_routes_an_edit_to_only_its_own_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let first = ready(&mut app, assets::EditorSeed::Blank);
      let second = ready(&mut app, assets::EditorSeed::Blank);

      let _ = handle_stockpile_editor(
        &mut app,
        first,
        assets::Message::StockpileEditorNameChanged("Cap boosters".to_owned()),
      );

      assert_eq!(
        app.stockpile_editors.get(first).map(assets::Editor::name),
        Some("Cap boosters")
      );
      assert_eq!(app.stockpile_editors.get(second).map(assets::Editor::name), Some(""));
    }

    #[tokio::test]
    async fn it_dispatches_the_item_scope_and_location_search_effects() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let id = ready(&mut app, assets::EditorSeed::Blank);

      let _ = handle_stockpile_editor(
        &mut app,
        id,
        assets::Message::StockpileEditorItemSearchChanged("Trit".to_owned()),
      );
      let _ = handle_stockpile_editor(
        &mut app,
        id,
        assets::Message::StockpileEditorScopeChanged("tag:pvp".to_owned()),
      );
      let _ = handle_stockpile_editor(
        &mut app,
        id,
        assets::Message::StockpileEditorLocationSearchChanged("Jita".to_owned()),
      );

      assert!(app.stockpile_editors.get(id).is_some(), "the editor stays open");
    }

    #[tokio::test]
    async fn it_ignores_a_message_for_an_unknown_window_or_missing_runtime() {
      let mut app = test_app();
      let _ = handle_stockpile_editor(
        &mut app,
        window::Id::unique(),
        assets::Message::StockpileEditorNameChanged("x".to_owned()),
      );

      app.runtime = Some(test_runtime().await);
      let _ = handle_stockpile_editor(
        &mut app,
        window::Id::unique(),
        assets::Message::StockpileEditorNameChanged("x".to_owned()),
      );

      assert!(app.stockpile_editors.is_empty());
    }

    #[tokio::test]
    async fn it_closes_the_window_on_cancel() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(&mut app, assets::EditorSeed::Blank);

      let _ = handle_stockpile_editor(&mut app, id, assets::Message::StockpileEditorClosed);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.stockpile_editors.get(id).is_none());
    }

    #[tokio::test]
    async fn it_saves_and_closes_only_the_targeted_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let first = ready(&mut app, assets::EditorSeed::Blank);
      let second = ready(&mut app, assets::EditorSeed::Blank);

      let _ = handle_stockpile_editor(&mut app, first, assets::Message::StockpileEditorSaved);

      assert_eq!(app.windows.kind(first), None);
      assert!(app.stockpile_editors.get(first).is_none());
      assert_eq!(app.windows.kind(second), Some(Window::StockpileEditor));
      assert!(app.stockpile_editors.get(second).is_some());
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_the_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(&mut app, assets::EditorSeed::Blank);

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.stockpile_editors.get(id).is_none());
    }
  }

  mod stockpile_import_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn open(app: &mut App) -> window::Id {
      let id = window::Id::unique();
      app.windows.register(id, Window::StockpileImport);
      app.stockpile_imports.insert(id, assets::ImportPanel::blank());
      id
    }

    fn import_resolution() -> assets::MultibuyResolution {
      assets::MultibuyResolution {
        matched: vec![assets::MultibuyMatch {
          name: "Tritanium".to_owned(),
          quantity: 100,
          type_id: 34,
        }],
        unmatched: Vec::new(),
      }
    }

    #[tokio::test]
    async fn it_opens_a_single_instance_window_with_a_runtime() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let _ = open_stockpile_import_window(&mut app);

      assert_eq!(app.windows.ids_for(Window::StockpileImport).count(), 1);
      assert_eq!(app.stockpile_imports.len(), 1);
    }

    #[tokio::test]
    async fn it_replaces_the_existing_import_window_on_reopen() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let _ = open_stockpile_import_window(&mut app);
      let _ = open_stockpile_import_window(&mut app);

      assert_eq!(app.windows.ids_for(Window::StockpileImport).count(), 1);
    }

    #[tokio::test]
    async fn it_is_a_no_op_without_a_runtime() {
      let mut app = test_app();

      let _ = open_stockpile_import_window(&mut app);

      assert_eq!(app.windows.ids_for(Window::StockpileImport).count(), 0);
      assert_eq!(app.stockpile_imports.len(), 0);
    }

    #[tokio::test]
    async fn it_routes_a_text_edit_to_only_its_own_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let id = open(&mut app);

      let _ = handle_stockpile_import(
        &mut app,
        id,
        assets::Message::StockpileImportTextChanged(iced::widget::text_editor::Action::Edit(
          iced::widget::text_editor::Edit::Paste(std::sync::Arc::new("Tritanium 100".to_owned())),
        )),
      );

      assert_eq!(
        app.stockpile_imports.get(id).map(assets::ImportPanel::text),
        Some("Tritanium 100".to_owned())
      );
    }

    #[tokio::test]
    async fn it_closes_the_window_on_cancel() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = open(&mut app);

      let _ = handle_stockpile_import(&mut app, id, assets::Message::StockpileImportClosed);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.stockpile_imports.get(id).is_none());
    }

    #[tokio::test]
    async fn it_confirms_into_a_prefilled_editor_and_closes_the_import_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = open(&mut app);
      let _ = handle_stockpile_import(
        &mut app,
        id,
        assets::Message::StockpileImportResolved(import_resolution()),
      );

      let _ = handle_stockpile_import(&mut app, id, assets::Message::StockpileImportConfirmed);

      assert!(app.stockpile_imports.get(id).is_none());
      assert_eq!(app.windows.ids_for(Window::StockpileEditor).count(), 1);
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_the_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = open(&mut app);

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.stockpile_imports.get(id).is_none());
    }
  }

  mod compose_window {
    use pretty_assertions::assert_eq;

    use super::*;

    fn ready(app: &mut App, seed: mail::compose::Seed) -> window::Id {
      let id = window::Id::unique();
      app.windows.register(id, Window::MailCompose);
      app.composes.insert(id, mail::compose::Draft::from_seed(seed));
      id
    }

    #[tokio::test]
    async fn it_registers_the_kind_and_seeds_a_blank_compose() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let id = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );

      assert_eq!(app.windows.kind(id), Some(Window::MailCompose));
      assert!(app.composes.get(id).is_some());
    }

    #[tokio::test]
    async fn it_holds_two_composes_at_once() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);

      let first = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );
      let second = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 7,
        },
      );

      assert_ne!(first, second);
      assert_eq!(app.composes.len(), 2);
      assert_eq!(app.windows.ids_for(Window::MailCompose).count(), 2);
    }

    #[tokio::test]
    async fn it_routes_a_subject_edit_to_only_its_own_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      let first = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );
      let second = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );

      let _ = handle_compose(&mut app, first, mail::Message::ComposeSubjectChanged("CTA".to_owned()));

      assert_eq!(app.composes.get(first).map(|d| d.subject.as_str()), Some("CTA"));
      assert_eq!(app.composes.get(second).map(|d| d.subject.as_str()), Some(""));
    }

    #[tokio::test]
    async fn it_discards_a_compose_without_saving() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );

      let _ = handle_compose(&mut app, id, mail::Message::ComposeDiscarded);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.composes.get(id).is_none());
    }

    #[tokio::test]
    async fn it_closes_only_the_targeted_window() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let first = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );
      let second = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );

      let _ = close_compose_window(&mut app, first);

      assert_eq!(app.windows.kind(first), None);
      assert!(app.composes.get(first).is_none());
      assert_eq!(app.windows.kind(second), Some(Window::MailCompose));
      assert!(app.composes.get(second).is_some());
    }

    #[tokio::test]
    async fn it_drops_the_state_when_the_os_reports_a_compose_window_closed() {
      let mut app = test_app();
      app.runtime = Some(test_runtime().await);
      app.windows.register(window::Id::unique(), Window::Main);
      let id = ready(
        &mut app,
        mail::compose::Seed::Blank {
          from_character_id: 42,
        },
      );

      let _ = on_window_closed(&mut app, id);

      assert_eq!(app.windows.kind(id), None);
      assert!(app.composes.get(id).is_none());
    }
  }

  mod resolve_window_geometry {
    use pretty_assertions::assert_eq;

    use super::*;

    const DEFAULT: Size = Size::new(1200.0, 800.0);

    fn monitor() -> validity::Rect {
      validity::Rect {
        height: 1080.0,
        width: 1920.0,
        x: 0.0,
        y: 0.0,
      }
    }

    fn geometry(x: f32, y: f32) -> WindowGeometry {
      WindowGeometry {
        height: 700.0,
        width: 1000.0,
        x,
        y,
      }
    }

    fn sized(width: f32, height: f32) -> WindowGeometry {
      WindowGeometry {
        height,
        width,
        x: 100.0,
        y: 100.0,
      }
    }

    #[test]
    fn it_centers_at_the_default_size_when_there_is_no_saved_geometry() {
      let (size, position) = resolve_window_geometry(None, &[monitor()], DEFAULT);

      assert_eq!(size, DEFAULT);
      assert!(matches!(position, window::Position::Centered));
    }

    #[test]
    fn it_clamps_a_valid_size_below_the_floor_up_to_the_minimum() {
      let (size, _) = resolve_window_geometry(Some(sized(700.0, 500.0)), &[monitor()], DEFAULT);

      assert_eq!(
        size,
        Size::new(800.0, 600.0),
        "a too-small but valid size is raised to the floor"
      );
    }

    #[test]
    fn it_defaults_the_size_for_a_zero_sized_window() {
      let (size, _) = resolve_window_geometry(Some(sized(0.0, 0.0)), &[monitor()], DEFAULT);

      assert_eq!(size, DEFAULT, "a 0x0 saved size never reopens broken");
    }

    #[test]
    fn it_defaults_the_size_for_an_absurdly_large_window() {
      let (size, _) = resolve_window_geometry(Some(sized(999_999.0, 999_999.0)), &[monitor()], DEFAULT);

      assert_eq!(size, DEFAULT);
    }

    #[test]
    fn it_defaults_the_size_for_negative_or_non_finite_dimensions() {
      assert_eq!(
        resolve_window_geometry(Some(sized(-1200.0, 800.0)), &[monitor()], DEFAULT).0,
        DEFAULT
      );
      assert_eq!(
        resolve_window_geometry(Some(sized(f32::NAN, 800.0)), &[monitor()], DEFAULT).0,
        DEFAULT
      );
      assert_eq!(
        resolve_window_geometry(Some(sized(1200.0, f32::INFINITY)), &[monitor()], DEFAULT).0,
        DEFAULT
      );
    }

    #[test]
    fn it_falls_back_to_the_range_guard_when_no_monitor_is_known() {
      let (_, in_range) = resolve_window_geometry(Some(geometry(120.0, 90.0)), &[], DEFAULT);
      assert!(matches!(in_range, window::Position::Specific(p) if p == Point::new(120.0, 90.0)));

      let (_, out_of_range) = resolve_window_geometry(Some(geometry(-50.0, 90.0)), &[], DEFAULT);
      assert!(matches!(out_of_range, window::Position::Centered));
    }

    #[test]
    fn it_honors_the_saved_size_but_centers_an_off_monitor_position() {
      let (size, position) = resolve_window_geometry(Some(geometry(3000.0, 90.0)), &[monitor()], DEFAULT);

      assert_eq!(size, Size::new(1000.0, 700.0), "a valid saved size is still honored");
      assert!(
        matches!(position, window::Position::Centered),
        "an off-screen position falls back to centered"
      );
    }

    #[test]
    fn it_restores_a_size_at_or_above_the_floor_unchanged() {
      let (size, _) = resolve_window_geometry(Some(sized(900.0, 650.0)), &[monitor()], DEFAULT);

      assert_eq!(size, Size::new(900.0, 650.0));
    }

    #[test]
    fn it_restores_size_and_position_for_a_monitor_valid_saved_rect() {
      let (size, position) = resolve_window_geometry(Some(geometry(120.0, 90.0)), &[monitor()], DEFAULT);

      assert_eq!(size, Size::new(1000.0, 700.0));
      assert!(matches!(position, window::Position::Specific(p) if p == Point::new(120.0, 90.0)));
    }
  }

  mod scale_to_factor {
    use super::*;

    #[test]
    fn it_clamps_values_outside_the_supported_range() {
      assert_eq!(scale_to_factor(0), 0.85);
      assert_eq!(scale_to_factor(255), 1.5);
    }

    #[test]
    fn it_maps_a_default_scale_to_a_unit_factor() {
      assert_eq!(scale_to_factor(100), 1.0);
    }

    #[test]
    fn it_maps_the_extremes_of_the_range() {
      assert_eq!(scale_to_factor(85), 0.85);
      assert_eq!(scale_to_factor(150), 1.5);
    }
  }

  mod handle_manage_plans {
    use super::*;

    fn app_with_manage_plans() -> (App, window::Id) {
      let mut app = ready_app();
      let id = window::Id::unique();
      app.manage_plans = Some((id, skill_plan_manager::State::new()));
      (app, id)
    }

    #[test]
    fn it_handles_the_state_only_messages_without_a_runtime() {
      let (mut app, _id) = app_with_manage_plans();

      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::CancelDelete);
      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::CharacterSelected(7));
      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::RequestDelete(3));
      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::ToggleCopyMenu(3));
      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::CancelDelete);
    }

    #[test]
    fn it_closes_the_copy_menu_on_dismiss() {
      let (mut app, _id) = app_with_manage_plans();
      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::ToggleCopyMenu(3));
      assert_eq!(app.manage_plans.as_ref().unwrap().1.copy_menu(), Some(3));

      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::CloseCopyMenu);

      assert_eq!(app.manage_plans.as_ref().unwrap().1.copy_menu(), None);
    }

    #[test]
    fn it_short_circuits_the_runtime_backed_messages_when_no_runtime_is_present() {
      let (mut app, _id) = app_with_manage_plans();

      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::ConfirmDelete(1));
      let _ = handle_manage_plans(
        &mut app,
        skill_plan_manager::Message::CopyPlan {
          plan_id: 1,
          target_character_id: 2,
        },
      );
      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::NewPlan(1));
      let _ = handle_manage_plans(
        &mut app,
        skill_plan_manager::Message::OpenPlan {
          character_id: 1,
          plan_id: 5,
        },
      );
    }

    #[tokio::test]
    async fn it_loads_the_roster_and_fetches_stale_images() {
      let (mut app, _id) = app_with_manage_plans();
      app.runtime = Some(test_runtime().await);

      let _ = handle_manage_plans(&mut app, skill_plan_manager::Message::Loaded(Box::default()));
    }
  }

  mod handle_compose {
    use super::*;

    fn app_with_compose() -> (App, window::Id) {
      let mut app = ready_app();
      let id = window::Id::unique();
      app.composes.insert(
        id,
        mail::compose::Draft::from_seed(mail::compose::Seed::Blank {
          from_character_id: 1,
        }),
      );
      (app, id)
    }

    #[test]
    fn it_is_a_no_op_without_a_runtime() {
      let mut app = ready_app();
      let id = window::Id::unique();

      let _ = handle_compose(&mut app, id, mail::Message::DraftSaved(Some(7)));
    }

    #[tokio::test]
    async fn it_threads_draft_load_and_save_ids_per_window() {
      let (mut app, id) = app_with_compose();
      app.runtime = Some(test_runtime().await);

      let _ = handle_compose(&mut app, id, mail::Message::DraftSaved(Some(42)));
      assert_eq!(
        app.composes.get(id).and_then(mail::compose::Draft::sent_draft_id),
        Some(42)
      );

      let _ = handle_compose(&mut app, id, mail::Message::DraftLoaded(Box::new(None)));

      let _ = handle_compose(&mut app, window::Id::unique(), mail::Message::PickerToggled);
    }

    #[tokio::test]
    async fn it_routes_a_successful_send_through_completion() {
      let (mut app, id) = app_with_compose();
      app.runtime = Some(test_runtime().await);

      let _ = handle_compose(&mut app, id, mail::Message::ComposeSent(Ok(())));
      assert!(app.composes.get(id).is_none(), "the window closes on send");
    }
  }

  mod open_native_window {
    use super::*;

    #[test]
    fn it_registers_the_kind_synchronously() {
      let mut app = test_app();
      let (id, _task) = crate::app::open_native_window(&mut app, Window::Compare, Size::new(800.0, 600.0));

      assert_eq!(app.windows.kind(id), Some(Window::Compare));
    }
  }
}
