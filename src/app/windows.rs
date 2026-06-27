use std::collections::HashMap;

use iced::window;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Window {
  // Registered by the foundation task; first constructed by the Calendar-detail conversion task.
  #[allow(dead_code)]
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
  // Registered by the foundation task; first constructed by the Stockpile-Import conversion task.
  #[allow(dead_code)]
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

// The id-keyed per-window state map ships ahead of its first instantiation: the Killmail pilot and the
// Contract/Stockpile/Mail child windows, none of which exist yet, each hold one `WindowStates<S>` so
// duplicates of a kind coexist.
#[allow(dead_code)]
#[derive(Debug)]
pub struct WindowStates<S> {
  states: HashMap<window::Id, S>,
}

#[allow(dead_code)]
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

  pub fn is_empty(&self) -> bool {
    self.states.is_empty()
  }

  pub fn iter(&self) -> impl Iterator<Item = (window::Id, &S)> + '_ {
    self.states.iter().map(|(id, state)| (*id, state))
  }

  pub fn len(&self) -> usize {
    self.states.len()
  }

  pub fn remove(&mut self, id: window::Id) -> Option<S> {
    self.states.remove(&id)
  }
}

#[allow(dead_code)]
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

  // Consumed by the not-yet-built detached child windows to enumerate every open instance of a kind.
  #[allow(dead_code)]
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
