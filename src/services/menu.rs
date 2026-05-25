//! Native menu bar service: builds the muda menu and provides an iced subscription.

use iced::Subscription;
use muda::{
  Menu, MenuItem, PredefinedMenuItem, Submenu,
  accelerator::{Accelerator, CMD_OR_CTRL, Code},
};

pub const ABOUT_ID: &str = "menu_about";
pub const CHECK_UPDATES_ID: &str = "menu_check_updates";
pub const CLEAR_CACHE_ID: &str = "menu_clear_cache";
pub const QUIT_ID: &str = "menu_quit";

/// Messages produced by the native menu.
#[derive(Clone, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum MenuMessage {
  AboutRequested,
  CheckForUpdatesRequested,
  ClearCacheRequested,
  QuitRequested,
}

/// Builds and registers the native menu bar. Call once on the main thread
/// before the iced event loop starts.
pub fn init() -> Menu {
  let about = MenuItem::with_id(ABOUT_ID, "About Pod", true, None);
  let check_updates = MenuItem::with_id(CHECK_UPDATES_ID, "Check for Updates\u{2026}", true, None);
  let clear_cache = MenuItem::with_id(CLEAR_CACHE_ID, "Clear Cache", true, None);
  let quit = MenuItem::with_id(
    QUIT_ID,
    "Quit Pod",
    true,
    Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyQ)),
  );

  let pod_menu = Submenu::with_items(
    "Pod",
    true,
    &[
      &about,
      &check_updates,
      &PredefinedMenuItem::separator(),
      &clear_cache,
      &PredefinedMenuItem::separator(),
      &quit,
    ],
  )
  .expect("failed to build Pod submenu");

  let menu = Menu::new();
  menu.append(&pod_menu).expect("failed to append Pod submenu");

  #[cfg(target_os = "macos")]
  menu.init_for_nsapp();

  menu
}

/// Iced subscription that polls the muda event channel and yields
/// [`MenuMessage`] values for known item IDs.
pub fn subscription() -> Subscription<MenuMessage> {
  Subscription::run(stream)
}

fn id_to_message(id: &str) -> Option<MenuMessage> {
  match id {
    ABOUT_ID => Some(MenuMessage::AboutRequested),
    CHECK_UPDATES_ID => Some(MenuMessage::CheckForUpdatesRequested),
    id => id_to_action_message(id),
  }
}

fn id_to_action_message(id: &str) -> Option<MenuMessage> {
  match id {
    CLEAR_CACHE_ID => Some(MenuMessage::ClearCacheRequested),
    QUIT_ID => Some(MenuMessage::QuitRequested),
    _ => None,
  }
}

fn event_to_message(event: &muda::MenuEvent) -> Option<MenuMessage> {
  id_to_message(event.id().0.as_str())
}

async fn drain_menu_events(tx: &mut iced::futures::channel::mpsc::Sender<MenuMessage>) {
  use iced::futures::SinkExt as _;
  while let Ok(event) = muda::MenuEvent::receiver().try_recv() {
    if let Some(m) = event_to_message(&event) {
      let _ = tx.send(m).await;
    }
  }
}

fn stream() -> impl iced::futures::Stream<Item = MenuMessage> {
  iced::stream::channel(16, async |mut tx| {
    loop {
      drain_menu_events(&mut tx).await;
      tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
  })
}
