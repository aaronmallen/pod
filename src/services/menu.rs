use std::sync::Mutex;

use iced::{Subscription, futures::Stream};
use muda::{Menu, MenuId, MenuItem, PredefinedMenuItem, accelerator};

pub const ABOUT_ID: &str = "pod.menu.about";
pub const CHECK_UPDATES_ID: &str = "pod.menu.check_updates";
pub const QUIT_ID: &str = "pod.menu.quit";

static SENDER: Mutex<Option<iced::futures::channel::mpsc::Sender<MenuAction>>> = Mutex::new(None);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
  About,
  CheckUpdates,
  Quit,
}

pub fn action_for_id(id: &MenuId) -> Option<MenuAction> {
  match id.as_ref() {
    ABOUT_ID => Some(MenuAction::About),
    CHECK_UPDATES_ID => Some(MenuAction::CheckUpdates),
    QUIT_ID => Some(MenuAction::Quit),
    _ => None,
  }
}

pub fn init() {
  let menu = match build() {
    Ok(menu) => menu,
    Err(error) => {
      tracing::warn!(target: "pod::lifecycle", %error, "building the native menu failed");
      return;
    }
  };

  let menu = Box::leak(Box::new(menu));

  #[cfg(target_os = "macos")]
  menu.init_for_nsapp();
  #[cfg(not(target_os = "macos"))]
  let _ = menu;

  spawn_event_pump();
}

pub fn subscription() -> Subscription<MenuAction> {
  Subscription::run(stream)
}

fn build() -> Result<Menu, muda::Error> {
  let menu = Menu::new();

  let about = MenuItem::with_id(ABOUT_ID, "About Pod", true, None);
  let check_updates = MenuItem::with_id(CHECK_UPDATES_ID, "Check for Updates…", true, None);

  let quit_accelerator = accelerator::Accelerator::new(Some(accelerator::Modifiers::META), accelerator::Code::KeyQ);
  let quit = MenuItem::with_id(QUIT_ID, "Quit", true, Some(quit_accelerator));

  let app_menu = muda::Submenu::new("Pod", true);
  app_menu.append_items(&[
    &about,
    &PredefinedMenuItem::separator(),
    &check_updates,
    &PredefinedMenuItem::separator(),
    &quit,
  ])?;
  menu.append(&app_menu)?;

  Ok(menu)
}

fn deliver(action: MenuAction) {
  if let Ok(mut guard) = SENDER.lock()
    && let Some(sender) = guard.as_mut()
  {
    let _ = sender.try_send(action);
  }
}

fn spawn_event_pump() {
  std::thread::Builder::new()
    .name("pod-menu-events".into())
    .spawn(|| {
      let receiver = muda::MenuEvent::receiver();
      while let Ok(event) = receiver.recv() {
        if let Some(action) = action_for_id(&event.id) {
          deliver(action);
        }
      }
    })
    .ok();
}

fn stream() -> impl Stream<Item = MenuAction> {
  iced::stream::channel(16, |tx: iced::futures::channel::mpsc::Sender<MenuAction>| async move {
    if let Ok(mut guard) = SENDER.lock() {
      *guard = Some(tx);
    }
    std::future::pending::<()>().await;
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  mod action_for_id {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_the_about_id() {
      assert_eq!(action_for_id(&MenuId::new(ABOUT_ID)), Some(MenuAction::About));
    }

    #[test]
    fn it_maps_the_check_updates_id() {
      assert_eq!(
        action_for_id(&MenuId::new(CHECK_UPDATES_ID)),
        Some(MenuAction::CheckUpdates)
      );
    }

    #[test]
    fn it_maps_the_quit_id() {
      assert_eq!(action_for_id(&MenuId::new(QUIT_ID)), Some(MenuAction::Quit));
    }

    #[test]
    fn it_returns_none_for_an_unknown_id() {
      assert_eq!(action_for_id(&MenuId::new("pod.menu.unknown")), None);
    }
  }
}
