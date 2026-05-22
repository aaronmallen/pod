//! Native menu bar service: builds the muda menu and provides an iced subscription.

use iced::Subscription;
use muda::{Menu, MenuItem, PredefinedMenuItem, Submenu};

pub const ABOUT_ID: &str = "menu_about";
pub const CHECK_UPDATES_ID: &str = "menu_check_updates";
pub const CLEAR_CACHE_ID: &str = "menu_clear_cache";

/// Messages produced by the native menu.
#[derive(Clone, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum MenuMessage {
  AboutRequested,
  CheckForUpdatesRequested,
  ClearCacheRequested,
}

/// Builds and registers the native menu bar. Call once on the main thread
/// before the iced event loop starts.
pub fn init() -> Menu {
  let about = MenuItem::with_id(ABOUT_ID, "About Pod", true, None);
  let check_updates = MenuItem::with_id(CHECK_UPDATES_ID, "Check for Updates\u{2026}", true, None);
  let clear_cache = MenuItem::with_id(CLEAR_CACHE_ID, "Clear Cache", true, None);

  let pod_menu = Submenu::with_items(
    "Pod",
    true,
    &[
      &about,
      &check_updates,
      &PredefinedMenuItem::separator(),
      &clear_cache,
      &PredefinedMenuItem::separator(),
      &PredefinedMenuItem::quit(Some("Quit Pod")),
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

fn stream() -> impl iced::futures::Stream<Item = MenuMessage> {
  iced::stream::channel(16, async |mut tx| {
    use iced::futures::SinkExt as _;

    loop {
      while let Ok(event) = muda::MenuEvent::receiver().try_recv() {
        let msg = match event.id().0.as_str() {
          ABOUT_ID => Some(MenuMessage::AboutRequested),
          CHECK_UPDATES_ID => Some(MenuMessage::CheckForUpdatesRequested),
          CLEAR_CACHE_ID => Some(MenuMessage::ClearCacheRequested),
          _ => None,
        };
        if let Some(m) = msg {
          let _ = tx.send(m).await;
        }
      }
      tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
  })
}
