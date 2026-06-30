use std::collections::HashMap;

use iced::window;

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Window {
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
      if let Some((_, state)) = app.editor.as_mut() {
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
    Window::SkillPlanEditor if app.editor.as_ref().map(|(eid, _)| *eid) == Some(id) => app.editor = None,
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
pub(super) fn open_editor_window(app: &mut App, character_id: i64, seed: skill_plan_editor::Seed) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();

  let close_existing = match app.editor.take() {
    Some((existing_id, _)) => {
      app.windows.remove(existing_id);
      window::close(existing_id)
    }
    None => Task::none(),
  };

  let (id, open_task) = open_native_window(
    app,
    Window::SkillPlanEditor,
    Size::new(EDITOR_WINDOW_WIDTH, EDITOR_WINDOW_HEIGHT),
  );
  app.editor = Some((
    id,
    skill_plan_editor::State::new(character_id).with_restored_panes(&app.ui_state),
  ));

  Task::batch([
    close_existing,
    open_task,
    skill_plan_editor::load(&db, character_id, seed).map(Message::SkillPlanEditor),
  ])
}
pub(super) fn close_editor_window(app: &mut App, id: window::Id) -> Task<Message> {
  let was_editor = app.editor.as_ref().map(|(eid, _)| *eid) == Some(id);
  if was_editor {
    app.editor = None;
  }
  app.windows.remove(id);

  let reload = match (was_editor, app.skills.as_ref(), app.runtime.as_ref()) {
    (true, Some(skills), Some(runtime)) => skills::reload_plans(&runtime.db, skills.active()).map(Message::Skills),
    _ => Task::none(),
  };
  Task::batch([window::close(id), reload])
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
pub(super) fn handle_manage_plans(app: &mut App, msg: skill_plan_manager::Message) -> Task<Message> {
  match msg {
    skill_plan_manager::Message::CancelDelete => {
      if let Some((_, state)) = app.manage_plans.as_mut() {
        state.clear_delete();
      }
      Task::none()
    }
    skill_plan_manager::Message::CharacterSelected(character_id) => {
      if let Some((_, state)) = app.manage_plans.as_mut() {
        state.select(character_id);
      }
      Task::none()
    }
    skill_plan_manager::Message::ConfirmDelete(plan_id) => {
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
    skill_plan_manager::Message::CopyPlan {
      plan_id,
      target_character_id,
    } => {
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
    skill_plan_manager::Message::Loaded(roster) => {
      let Some((_, state)) = app.manage_plans.as_mut() else {
        return Task::none();
      };
      state.set_roster(*roster);
      let keys = state.stale_images();
      dispatch_image_fetches(app, keys)
    }
    skill_plan_manager::Message::NewPlan(character_id) => {
      open_plan_from_manager(app, character_id, skill_plan_editor::Seed::New)
    }
    skill_plan_manager::Message::OpenPlan {
      character_id,
      plan_id,
    } => open_plan_from_manager(app, character_id, skill_plan_editor::Seed::Existing(plan_id)),
    skill_plan_manager::Message::RequestDelete(plan_id) => {
      if let Some((_, state)) = app.manage_plans.as_mut() {
        state.arm_delete(plan_id);
      }
      Task::none()
    }
    skill_plan_manager::Message::ToggleCopyMenu(plan_id) => {
      if let Some((_, state)) = app.manage_plans.as_mut() {
        state.toggle_copy_menu(plan_id);
      }
      Task::none()
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
  let close = match app.manage_plans.take() {
    Some((id, _)) => {
      app.windows.remove(id);
      window::close(id)
    }
    None => Task::none(),
  };

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

  Task::batch([close, switch, open_editor_window(app, character_id, seed)])
}
pub(super) fn close_manage_plans_window(app: &mut App, id: window::Id) -> Task<Message> {
  if app.manage_plans.as_ref().map(|(mid, _)| *mid) == Some(id) {
    app.manage_plans = None;
  }
  app.windows.remove(id);
  window::close(id)
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
  match app.editor.as_ref() {
    Some((editor_id, state)) if *editor_id == id => {
      skill_plan_editor::view(state, app.now).map(Message::SkillPlanEditor)
    }
    _ => blank(),
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
}
