//! Pod — EVE Online character manager entry point.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod config;
mod controllers;
mod services;

use std::collections::HashMap;

use controllers::{about_window, main_window as main_ctrl, skill_plan_window, splash as splash_ctrl};
use iced::{Color, Element, Point, Size, Subscription, Task, window};
use pod_model::Character;
use pod_ui::{
  components::update_banner,
  plan_math::BaseAttrs,
  style::{spacing::layout, typography::bytes as font_bytes},
  views::{mail, main_window, skills, splash, wallet},
};
use services::{Services, menu};

pub const ESI_CLIENT_ID: &str = "8fa6e582375c4633a100e9c0ffd37224";

/// Identifies what kind of content a window is displaying.
pub enum WindowKind {
  Main,
  SkillPlan {
    character_id: i64,
    seed: skill_plan_window::PlanSeed,
  },
}

struct App {
  about_window: Option<(window::Id, about_window::State)>,
  characters: Vec<Character>,
  config: config::Settings,
  db: Option<pod_db::Repo>,
  esi_client: Option<pod_esi::Client>,
  oauth_callback_tx: tokio::sync::broadcast::Sender<(String, String)>,
  phase: AppPhase,
  plan_window_position: Option<Point>,
  plan_window_size: Size,
  plan_windows: HashMap<window::Id, skill_plan_window::State>,
  step_label: String,
  update_dismissed: bool,
  update_state: services::updater::UpdateState,
  window_id: Option<window::Id>,
  window_position: Option<Point>,
  window_size: Size,
}

#[allow(clippy::large_enum_variant)]
enum AppPhase {
  Main(main_ctrl::State),
  Splash(splash_ctrl::State),
}

#[derive(Clone, Debug)]
enum Message {
  AboutWindow(about_window::Message),
  BackgroundSync(services::bootstrap::Message),
  Bootstrap(services::bootstrap::Message),
  Main(main_ctrl::Message),
  Menu(menu::MenuMessage),
  OAuthCallback(String, String),
  SkillPlan(window::Id, skill_plan_window::Message),
  Splash(splash_ctrl::Message),
  Tick,
  UpdateBanner(update_banner::Message),
  Updater(services::updater::Message),
  WindowCloseRequested(window::Id),
  WindowMoved(window::Id, Point),
  WindowOpened(window::Id),
  WindowResized(window::Id, Size),
}

#[allow(clippy::large_enum_variant)]
enum SplashMessage {
  Bootstrap(services::bootstrap::Message),
  Splash(splash_ctrl::Message),
  Tick,
}

enum UpdaterMessage {
  Banner(update_banner::Message),
  Updater(services::updater::Message),
}

enum WindowEvent {
  CloseRequested,
  Moved(Point),
  Opened,
  Resized(Size),
}

impl Default for App {
  fn default() -> Self {
    let (oauth_callback_tx, _) = tokio::sync::broadcast::channel(4);
    Self {
      about_window: None,
      characters: Vec::new(),
      config: config::Settings::default(),
      db: None,
      esi_client: None,
      oauth_callback_tx,
      phase: AppPhase::Splash(splash_ctrl::State::default()),
      plan_windows: HashMap::new(),
      step_label: "Opening database\u{2026}".to_string(),
      update_dismissed: false,
      update_state: services::updater::UpdateState::default(),
      window_id: None,
      window_position: None,
      window_size: Size::new(layout::WINDOW_DEFAULT_WIDTH, layout::WINDOW_DEFAULT_HEIGHT),
      plan_window_size: Size::new(900.0, 700.0),
      plan_window_position: None,
    }
  }
}

thread_local! {
  static MENU: std::cell::OnceCell<muda::Menu> = const { std::cell::OnceCell::new() };
}

fn boot() -> (App, Task<Message>) {
  MENU.with(|m| {
    m.get_or_init(menu::init);
  });
  let splash_settings = window::Settings {
    size: Size::new(layout::SPLASH_WIDTH, layout::SPLASH_HEIGHT),
    decorations: false,
    resizable: false,
    transparent: true,
    position: window::Position::Centered,
    ..window::Settings::default()
  };
  let (main_window_id, open_task) = window::open(splash_settings);
  let app = App {
    config: config::load().unwrap_or_default(),
    window_id: Some(main_window_id),
    ..App::default()
  };
  let task = Task::batch([
    open_task.map(Message::WindowOpened),
    services::bootstrap::run().map(Message::Bootstrap),
  ]);
  (app, task)
}

fn main() -> iced::Result {
  let log_dir = dir_spec::state_home()
    .expect("cannot determine state home directory")
    .join("pod/logs");
  let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
    .rotation(tracing_appender::rolling::Rotation::DAILY)
    .filename_prefix("pod")
    .max_log_files(7)
    .build(&log_dir)
    .expect("failed to initialize log file appender");
  let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
  tracing_subscriber::fmt()
    .with_ansi(false)
    .with_max_level(tracing::Level::TRACE)
    .with_writer(non_blocking)
    .init();

  iced::daemon(boot, update, view)
    .title(|_app: &App, _window: window::Id| "Pod".to_string())
    .theme(|app: &App, window: window::Id| {
      if app.plan_windows.contains_key(&window) {
        return Some(iced::Theme::Dark);
      }
      match &app.phase {
        AppPhase::Splash(_) => Some(iced::Theme::custom(
          "splash".to_string(),
          iced::theme::Palette {
            background: Color::TRANSPARENT,
            ..iced::theme::Palette::DARK
          },
        )),
        AppPhase::Main(_) => Some(iced::Theme::Dark),
      }
    })
    .subscription(subscription)
    .font(font_bytes::BODY_REGULAR)
    .font(font_bytes::BODY_MEDIUM)
    .font(font_bytes::BODY_SEMIBOLD)
    .font(font_bytes::MONO_REGULAR)
    .font(font_bytes::MONO_ITALIC)
    .run()
}

fn subscription(app: &App) -> Subscription<Message> {
  let window_events = window_event_subscription();
  let tick = phase_tick_subscription(&app.phase);
  let plan_subs = plan_window_subscriptions(&app.plan_windows);
  let menu_sub = menu::subscription().map(Message::Menu);
  let oauth_sub = services::oauth_callback::subscription().map(|(code, state)| Message::OAuthCallback(code, state));

  Subscription::batch(
    std::iter::once(window_events)
      .chain(std::iter::once(tick))
      .chain(std::iter::once(menu_sub))
      .chain(std::iter::once(oauth_sub))
      .chain(plan_subs),
  )
}

fn window_event_subscription() -> Subscription<Message> {
  window::events().filter_map(|(id, event)| match event {
    window::Event::Opened {
      ..
    } => Some(Message::WindowOpened(id)),
    window::Event::Moved(point) => Some(Message::WindowMoved(id, point)),
    window::Event::Resized(size) => Some(Message::WindowResized(id, size)),
    window::Event::CloseRequested => Some(Message::WindowCloseRequested(id)),
    _ => None,
  })
}

fn phase_tick_subscription(phase: &AppPhase) -> Subscription<Message> {
  match phase {
    AppPhase::Splash(_) => iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::Tick),
    AppPhase::Main(state) => main_phase_subscriptions(state),
  }
}

fn main_phase_subscriptions(state: &main_ctrl::State) -> Subscription<Message> {
  let main_subs = main_ctrl::subscription(state).map(Message::Main);
  let eve_tick =
    iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::Main(main_ctrl::Message::EveTimeTick));
  let update_check = iced::time::every(services::updater::check_interval())
    .map(|_| Message::Updater(services::updater::Message::CheckRequested));
  iced::Subscription::batch([main_subs, eve_tick, update_check])
}

fn plan_window_subscriptions(
  plan_windows: &HashMap<window::Id, skill_plan_window::State>,
) -> Vec<Subscription<Message>> {
  plan_windows
    .iter()
    .map(|(id, state)| {
      skill_plan_window::subscription(state)
        .with(*id)
        .map(|(id, m)| Message::SkillPlan(id, m))
    })
    .collect()
}

fn update_about_window(app: &mut App, msg: about_window::Message) -> Task<Message> {
  let Some((_, state)) = &mut app.about_window else {
    return Task::none();
  };
  about_window::update(state, msg).map(Message::AboutWindow)
}

fn update(app: &mut App, message: Message) -> Task<Message> {
  match message {
    Message::AboutWindow(msg) => update_about_window(app, msg),
    Message::BackgroundSync(msg) => update_background_sync(app, msg),
    Message::Bootstrap(msg) => update_splash(app, SplashMessage::Bootstrap(msg)),
    Message::Main(msg) => update_main(app, msg),
    Message::Menu(msg) => update_menu(app, msg),
    Message::OAuthCallback(code, state) => {
      let _ = app.oauth_callback_tx.send((code, state));
      Task::none()
    }
    Message::SkillPlan(id, msg) => update_skill_plan(app, id, msg),
    Message::Splash(msg) => update_splash(app, SplashMessage::Splash(msg)),
    Message::Tick => update_splash(app, SplashMessage::Tick),
    Message::UpdateBanner(msg) => update_updater(app, UpdaterMessage::Banner(msg)),
    Message::Updater(msg) => update_updater(app, UpdaterMessage::Updater(msg)),
    msg @ (Message::WindowCloseRequested(_)
    | Message::WindowMoved(..)
    | Message::WindowOpened(_)
    | Message::WindowResized(..)) => dispatch_window_event(app, msg),
  }
}

fn dispatch_window_event(app: &mut App, msg: Message) -> Task<Message> {
  match msg {
    Message::WindowCloseRequested(id) => update_window_events(app, id, WindowEvent::CloseRequested),
    Message::WindowMoved(id, pt) => update_window_events(app, id, WindowEvent::Moved(pt)),
    Message::WindowOpened(id) => update_window_events(app, id, WindowEvent::Opened),
    Message::WindowResized(id, sz) => update_window_events(app, id, WindowEvent::Resized(sz)),
    _ => unreachable!(),
  }
}

fn update_background_sync(app: &mut App, msg: services::bootstrap::Message) -> Task<Message> {
  use services::bootstrap::Message as BootMsg;
  match msg {
    BootMsg::CharacterSynced(character) => {
      if let Some(idx) = app.characters.iter().position(|c| *c.id() == *character.id()) {
        app.characters[idx] = character.clone();
      }
      if let AppPhase::Main(state) = &mut app.phase {
        main_ctrl::apply_synced_character(state, character);
      }
      Task::none()
    }
    BootMsg::TokenRefreshFailed(_) => {
      let AppPhase::Main(_) = &app.phase else {
        return Task::none();
      };
      let services = Services {
        config: app.config.clone(),
        db: app.db.clone(),
        esi_client: app.esi_client.clone(),
        oauth_callback_tx: app.oauth_callback_tx.clone(),
      };
      main_ctrl::reauth(&services).map(Message::Main)
    }
    _ => Task::none(),
  }
}

fn update_main(app: &mut App, msg: main_ctrl::Message) -> Task<Message> {
  let is_mail_pane_drag_end = matches!(&msg, main_window::Message::Mail(mail::Message::PaneDragEnd));
  let is_skills_pane_drag_end = matches!(&msg, main_window::Message::Skills(skills::Message::PaneDragEnd));
  let is_wallet_pane_drag_end = matches!(&msg, main_window::Message::Wallet(wallet::Message::PaneDragEnd));

  if let main_window::Message::Skills(ref skills_msg) = msg {
    let open_result = match skills_msg {
      skills::Message::PlanFromQueueRequested => {
        let AppPhase::Main(state) = &mut app.phase else {
          return Task::none();
        };
        let char_id = state.active_view.skills_char_id();
        let queue_items: Vec<(String, u8)> = if let main_window::ActiveView::Skills(s) = &state.active_view {
          s.queue.iter().map(|q| (q.skill_name.clone(), q.to_level)).collect()
        } else {
          Vec::new()
        };
        Some((char_id, skill_plan_window::PlanSeed::FromQueue(queue_items)))
      }
      skills::Message::PlanNewRequested => {
        let AppPhase::Main(state) = &mut app.phase else {
          return Task::none();
        };
        let char_id = state.active_view.skills_char_id();
        Some((char_id, skill_plan_window::PlanSeed::New))
      }
      skills::Message::PlanOpenRequested(id) => {
        let AppPhase::Main(state) = &mut app.phase else {
          return Task::none();
        };
        let char_id = state.active_view.skills_char_id();
        Some((char_id, skill_plan_window::PlanSeed::Existing(id.clone())))
      }
      _ => None,
    };
    if let Some((char_id, seed)) = open_result {
      return open_skill_plan_window(app, char_id, seed);
    }
  }

  let AppPhase::Main(state) = &mut app.phase else {
    return Task::none();
  };
  let services = Services {
    config: app.config.clone(),
    db: app.db.clone(),
    esi_client: app.esi_client.clone(),
    oauth_callback_tx: app.oauth_callback_tx.clone(),
  };
  let (task, new_config) = main_ctrl::update(state, msg, &services);
  if let Some(cfg) = new_config {
    app.config = cfg;
  }
  let task = task.map(Message::Main);
  if is_mail_pane_drag_end || is_skills_pane_drag_end || is_wallet_pane_drag_end {
    save_geometry(app);
  }
  task
}

fn update_skill_plan(app: &mut App, window_id: window::Id, msg: skill_plan_window::Message) -> Task<Message> {
  let is_pane_drag_end = matches!(&msg, skill_plan_window::Message::PaneDragEnd);
  let is_save_completed = matches!(&msg, skill_plan_window::Message::SaveCompleted);
  let services = Services {
    config: app.config.clone(),
    db: app.db.clone(),
    esi_client: app.esi_client.clone(),
    oauth_callback_tx: app.oauth_callback_tx.clone(),
  };
  let Some(plan_state) = app.plan_windows.get_mut(&window_id) else {
    return Task::none();
  };
  let plan_task = skill_plan_window::update(plan_state, msg, &services).map(move |m| Message::SkillPlan(window_id, m));
  if is_pane_drag_end {
    save_geometry(app);
  }
  if is_save_completed {
    if let AppPhase::Main(main_state) = &mut app.phase
      && let main_window::ActiveView::Skills(skills_state) = &mut main_state.active_view
    {
      skills_state.plans_loaded = false;
    }
    Task::batch([
      plan_task,
      Task::done(Message::Main(main_window::Message::Skills(
        skills::Message::PlansTabOpened,
      ))),
    ])
  } else {
    plan_task
  }
}

fn handle_splash_bootstrap(app: &mut App, msg: services::bootstrap::Message) -> Task<Message> {
  let AppPhase::Splash(state) = &mut app.phase else {
    return Task::none();
  };
  match splash_ctrl::handle_bootstrap(
    state,
    &mut app.db,
    &mut app.step_label,
    &mut app.characters,
    &mut app.esi_client,
    msg,
  ) {
    splash_ctrl::HandleResult::Bootstrap(t) => t.map(Message::Bootstrap),
    splash_ctrl::HandleResult::Fatal(e) => {
      eprintln!("pod: fatal startup error: {e}");
      iced::exit()
    }
    splash_ctrl::HandleResult::None => Task::none(),
    splash_ctrl::HandleResult::Splash(t) => t.map(Message::Splash),
  }
}

fn apply_saved_geometry(app: &mut App, saved: &services::window_state::WindowGeometry) {
  app.window_position = Some(Point::new(saved.x, saved.y));
  if let (Some(w), Some(h)) = (saved.plan_window_width, saved.plan_window_height) {
    app.plan_window_size = Size::new(w, h);
  }
  if let (Some(x), Some(y)) = (saved.plan_window_x, saved.plan_window_y) {
    app.plan_window_position = Some(Point::new(x, y));
  }
}

fn handle_splash_transition(app: &mut App, splash_task: Task<Message>) -> Task<Message> {
  let services = Services {
    config: app.config.clone(),
    db: app.db.clone(),
    esi_client: app.esi_client.clone(),
    oauth_callback_tx: app.oauth_callback_tx.clone(),
  };
  let saved = services::window_state::load();
  let (target_width, target_height) = saved
    .as_ref()
    .map(|g| (g.width, g.height))
    .unwrap_or((layout::WINDOW_DEFAULT_WIDTH, layout::WINDOW_DEFAULT_HEIGHT));
  let (main_state, init_task) = main_ctrl::new(
    app.characters.clone(),
    &services,
    saved.as_ref().and_then(|g| g.skills_left_pane_width),
    saved.as_ref().and_then(|g| g.mail_folder_pane_width),
    saved.as_ref().and_then(|g| g.mail_message_list_width),
    saved.as_ref().and_then(|g| g.wallet_right_rail_width),
  );
  app.phase = AppPhase::Main(main_state);
  app.window_size = Size::new(target_width, target_height);
  if let Some(geo) = &saved {
    apply_saved_geometry(app, geo);
  }
  let position = saved
    .as_ref()
    .map(|g| window::Position::Specific(Point::new(g.x, g.y)))
    .unwrap_or(window::Position::Default);
  let main_settings = window::Settings {
    size: Size::new(target_width, target_height),
    position,
    decorations: true,
    resizable: true,
    min_size: Some(Size::new(layout::WINDOW_MIN_WIDTH, layout::WINDOW_MIN_HEIGHT)),
    ..window::Settings::default()
  };
  let (new_id, open_task) = window::open(main_settings);
  let splash_id = app.window_id.replace(new_id);
  let mut tasks = vec![
    splash_task,
    init_task.map(Message::Main),
    open_task.map(Message::WindowOpened),
    services::updater::check().map(Message::Updater),
  ];
  if let (Some(db), Some(esi)) = (app.db.clone(), app.esi_client.clone()) {
    tasks.push(services::bootstrap::sync_characters(db, esi, app.characters.clone()).map(Message::BackgroundSync));
  }
  if let Some(id) = splash_id {
    tasks.push(window::close(id));
  }
  Task::batch(tasks)
}

fn handle_splash_inner(app: &mut App, inner: splash_ctrl::Message) -> Task<Message> {
  if matches!(inner, splash_ctrl::Message::DragWindow) {
    return match app.window_id {
      Some(id) => window::drag(id),
      None => Task::none(),
    };
  }
  let transitioning = matches!(inner, splash_ctrl::Message::ExpandComplete);
  let AppPhase::Splash(state) = &mut app.phase else {
    return Task::none();
  };
  let task = splash_ctrl::update(state, inner.clone()).map(Message::Splash);
  if transitioning {
    handle_splash_transition(app, task)
  } else {
    task
  }
}

fn update_splash(app: &mut App, msg: SplashMessage) -> Task<Message> {
  match msg {
    SplashMessage::Bootstrap(msg) => handle_splash_bootstrap(app, msg),
    SplashMessage::Splash(inner) => handle_splash_inner(app, inner),
    SplashMessage::Tick => {
      let AppPhase::Splash(state) = &mut app.phase else {
        return Task::none();
      };
      splash_ctrl::update(state, splash_ctrl::Message::Tick).map(Message::Splash)
    }
  }
}

fn handle_banner_message(app: &mut App, msg: update_banner::Message) -> Task<Message> {
  match msg {
    update_banner::Message::ApplyPressed => Task::done(Message::Updater(services::updater::Message::ApplyRequested)),
    update_banner::Message::DismissPressed => {
      app.update_dismissed = true;
      Task::none()
    }
    update_banner::Message::RestartPressed => {
      Task::done(Message::Updater(services::updater::Message::RestartRequested))
    }
    update_banner::Message::RetryPressed => {
      app.update_dismissed = false;
      Task::done(Message::Updater(services::updater::Message::CheckRequested))
    }
  }
}

fn handle_updater_message(app: &mut App, msg: services::updater::Message) -> Task<Message> {
  use services::updater::Message as UpdMsg;
  match msg {
    UpdMsg::ApplyComplete => {
      app.update_state = services::updater::UpdateState::ReadyToRestart;
      Task::none()
    }
    UpdMsg::ApplyFailed(e) => {
      app.update_state = services::updater::UpdateState::Error(e);
      Task::none()
    }
    UpdMsg::ApplyRequested => {
      app.update_state = services::updater::UpdateState::Downloading;
      services::updater::apply().map(Message::Updater)
    }
    UpdMsg::CheckComplete(Some(version)) => {
      app.update_state = services::updater::UpdateState::UpdateAvailable(version);
      Task::none()
    }
    UpdMsg::CheckComplete(None) | UpdMsg::CheckFailed => Task::none(),
    UpdMsg::CheckRequested => services::updater::check().map(Message::Updater),
    UpdMsg::RestartRequested => {
      services::updater::restart();
      Task::none()
    }
  }
}

fn update_updater(app: &mut App, msg: UpdaterMessage) -> Task<Message> {
  match msg {
    UpdaterMessage::Banner(msg) => handle_banner_message(app, msg),
    UpdaterMessage::Updater(msg) => handle_updater_message(app, msg),
  }
}

fn update_menu(app: &mut App, msg: menu::MenuMessage) -> Task<Message> {
  match msg {
    menu::MenuMessage::AboutRequested => {
      if let Some((id, _)) = &app.about_window {
        return window::gain_focus(*id);
      }
      let (win_id, open_task) = window::open(about_window::settings());
      app.about_window = Some((win_id, about_window::State::default()));
      open_task.map(Message::WindowOpened)
    }
    menu::MenuMessage::CheckForUpdatesRequested => {
      Task::done(Message::Updater(services::updater::Message::CheckRequested))
    }
    menu::MenuMessage::ClearCacheRequested => Task::future(services::cache_cleaner::clear_esi_cache()).discard(),
  }
}

fn handle_window_close(app: &mut App, id: window::Id) -> Task<Message> {
  if app.plan_windows.remove(&id).is_some() {
    return window::close(id);
  }
  if app.about_window.as_ref().map(|(wid, _)| *wid) == Some(id) {
    app.about_window = None;
    return window::close(id);
  }
  Task::none()
}

fn handle_window_moved(app: &mut App, id: window::Id, point: Point) -> Task<Message> {
  if app.plan_windows.contains_key(&id) {
    app.plan_window_position = Some(point);
    save_geometry(app);
    return Task::none();
  }
  if app.window_id != Some(id) {
    return Task::none();
  }
  app.window_position = Some(point);
  save_geometry(app);
  Task::none()
}

fn handle_window_opened(app: &mut App, id: window::Id) -> Task<Message> {
  if app.window_id.is_none() {
    app.window_id = Some(id);
  }
  // boot() calls init_for_nsapp() before NSApp is fully initialized in daemon
  // mode; re-calling here guarantees the menu is attached once NSApp is live.
  #[cfg(target_os = "macos")]
  if app.window_id == Some(id) {
    MENU.with(|m| {
      if let Some(menu) = m.get() {
        menu.init_for_nsapp();
      }
    });
  }
  disable_shadow(id)
}

fn handle_window_resized(app: &mut App, id: window::Id, size: Size) -> Task<Message> {
  if app.plan_windows.contains_key(&id) {
    app.plan_window_size = size;
    save_geometry(app);
    return Task::none();
  }
  if app.window_id != Some(id) {
    return Task::none();
  }
  app.window_size = size;
  save_geometry(app);
  Task::none()
}

fn update_window_events(app: &mut App, id: window::Id, event: WindowEvent) -> Task<Message> {
  match event {
    WindowEvent::CloseRequested => handle_window_close(app, id),
    WindowEvent::Moved(point) => handle_window_moved(app, id, point),
    WindowEvent::Opened => handle_window_opened(app, id),
    WindowEvent::Resized(size) => handle_window_resized(app, id, size),
  }
}

fn save_geometry(app: &App) {
  let AppPhase::Main(state) = &app.phase else {
    return;
  };
  let pos = app.window_position.unwrap_or(Point::ORIGIN);
  let plan_pos = app.plan_window_position;
  let plan_pane_widths: Option<(f32, f32)> = app
    .plan_windows
    .values()
    .next()
    .map(|s| (s.picker_pane_width, s.summary_pane_width));
  services::window_state::save(&services::window_state::WindowGeometry {
    width: app.window_size.width,
    height: app.window_size.height,
    x: pos.x,
    y: pos.y,
    skills_left_pane_width: Some(state.skills_left_pane_width),
    mail_folder_pane_width: Some(state.mail_folder_pane_width),
    mail_message_list_width: Some(state.mail_message_list_width),
    wallet_right_rail_width: Some(state.wallet_right_rail_width),
    plan_window_width: Some(app.plan_window_size.width),
    plan_window_height: Some(app.plan_window_size.height),
    plan_window_x: plan_pos.map(|p| p.x),
    plan_window_y: plan_pos.map(|p| p.y),
    plan_picker_pane_width: plan_pane_widths.map(|(w, _)| w),
    plan_summary_pane_width: plan_pane_widths.map(|(_, w)| w),
  });
}

#[cfg(target_os = "macos")]
fn disable_shadow(id: window::Id) -> Task<Message> {
  window::run(id, |w| {
    use window::raw_window_handle::RawWindowHandle;
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
fn disable_shadow(_: window::Id) -> Task<Message> {
  Task::none()
}

fn view(app: &App, window_id: window::Id) -> Element<'_, Message> {
  if let Some(plan_state) = app.plan_windows.get(&window_id) {
    return skill_plan_window::view(plan_state).map(move |m| Message::SkillPlan(window_id, m));
  }

  if let Some((about_id, about_state)) = &app.about_window
    && window_id == *about_id
  {
    return about_window::view(about_state).map(Message::AboutWindow);
  }

  match &app.phase {
    AppPhase::Splash(state) => splash::Component::new(state)
      .step_label(&app.step_label)
      .version(env!("CARGO_PKG_VERSION"))
      .render()
      .map(Message::Splash),
    AppPhase::Main(state) => {
      let content = main_window::Component::new(state)
        .window_size(app.window_size.width, app.window_size.height)
        .render()
        .map(Message::Main);
      match banner_state(&app.update_state, app.update_dismissed) {
        Some(state) => iced::widget::column([
          update_banner::Component::new(state).render().map(Message::UpdateBanner),
          content,
        ])
        .into(),
        None => content,
      }
    }
  }
}

fn banner_state(state: &services::updater::UpdateState, dismissed: bool) -> Option<update_banner::BannerState> {
  use services::updater::UpdateState;
  match state {
    UpdateState::Idle => None,
    UpdateState::UpdateAvailable(_) | UpdateState::Error(_) if dismissed => None,
    UpdateState::UpdateAvailable(v) => Some(update_banner::BannerState::UpdateAvailable(v.clone())),
    UpdateState::Downloading => Some(update_banner::BannerState::Downloading),
    UpdateState::ReadyToRestart => Some(update_banner::BannerState::ReadyToRestart),
    UpdateState::Error(e) => Some(update_banner::BannerState::Error(e.clone())),
  }
}

/// Opens a new skill plan window and registers it in the app's plan window map.
///
/// The window is created immediately with a default all-20 attribute set.
/// An async task is then dispatched to load real ESI attributes from the DB
/// and send back an `AttrsLoaded` message once complete.
pub(crate) fn open_skill_plan_window(
  app: &mut App,
  character_id: i64,
  seed: skill_plan_window::PlanSeed,
) -> Task<Message> {
  let saved = services::window_state::load();
  let picker_pane_width = saved.as_ref().and_then(|g| g.plan_picker_pane_width).unwrap_or(320.0);
  let summary_pane_width = saved.as_ref().and_then(|g| g.plan_summary_pane_width).unwrap_or(360.0);
  let position = app
    .plan_window_position
    .map(window::Position::Specific)
    .unwrap_or(window::Position::Default);
  let settings = window::Settings {
    size: app.plan_window_size,
    position,
    resizable: true,
    decorations: true,
    ..window::Settings::default()
  };
  let (win_id, open_task) = window::open(settings);
  let default_attrs = BaseAttrs {
    intelligence: 20,
    memory: 20,
    perception: 20,
    willpower: 20,
    charisma: 20,
  };
  let (state, init_task) = skill_plan_window::new(
    win_id,
    character_id,
    seed,
    picker_pane_width,
    summary_pane_width,
    app.db.clone(),
    default_attrs.clone(),
    default_attrs,
    true,
  );
  app.plan_windows.insert(win_id, state);
  let plan_task = init_task.map(move |m| Message::SkillPlan(win_id, m));
  let attrs_task = if let Some(db) = app.db.clone() {
    Task::perform(
      async move {
        let effective_opt = db.characters().effective_attributes(character_id).await.unwrap_or(None);
        let clone_bonus = db
          .clones()
          .active_clone_implant_bonus(character_id)
          .await
          .unwrap_or(None);
        (effective_opt, clone_bonus)
      },
      move |(effective_opt, clone_bonus)| {
        use pod_ui::views::skill_plan::Message as PlanMsg;
        let fallback = BaseAttrs {
          intelligence: 20,
          memory: 20,
          perception: 20,
          willpower: 20,
          charisma: 20,
        };
        let (current_effective_attrs, clone_data_missing) = match effective_opt {
          Some(eff) => (
            BaseAttrs {
              charisma: eff.charisma,
              intelligence: eff.intelligence,
              memory: eff.memory,
              perception: eff.perception,
              willpower: eff.willpower,
            },
            false,
          ),
          None => {
            eprintln!(
              "[pod] character {character_id}: effective \
              attributes not yet synced; using all-20 \
              fallback for skill plan"
            );
            (fallback.clone(), true)
          }
        };
        let clone_data_missing = clone_data_missing || clone_bonus.is_none();
        let clone_bonus = clone_bonus.unwrap_or_default();
        let base_attrs = BaseAttrs {
          charisma: current_effective_attrs.charisma - clone_bonus.charisma,
          intelligence: current_effective_attrs.intelligence - clone_bonus.intelligence,
          memory: current_effective_attrs.memory - clone_bonus.memory,
          perception: current_effective_attrs.perception - clone_bonus.perception,
          willpower: current_effective_attrs.willpower - clone_bonus.willpower,
        };
        Message::SkillPlan(
          win_id,
          PlanMsg::AttrsLoaded {
            base_attrs,
            current_effective_attrs,
            clone_data_missing,
          },
        )
      },
    )
  } else {
    Task::none()
  };
  Task::batch([open_task.map(Message::WindowOpened), plan_task, attrs_task])
}
