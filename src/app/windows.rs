use std::collections::HashMap;

use iced::window;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Window {
  About,
  Compare,
  Main,
  SkillPlanEditor,
  Splash,
}

impl Window {
  pub fn state_key(self) -> Option<&'static str> {
    match self {
      Self::Compare => Some("skills_compare"),
      Self::Main => Some("main"),
      Self::SkillPlanEditor => Some("skill_plan_editor"),
      Self::About | Self::Splash => None,
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

#[cfg(test)]
mod tests {
  use super::*;

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
    fn it_never_persists_about() {
      assert_eq!(Window::About.state_key(), None);
    }

    #[test]
    fn it_never_persists_splash() {
      assert_eq!(Window::Splash.state_key(), None);
    }
  }
}
