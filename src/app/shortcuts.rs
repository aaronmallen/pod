use iced::{
  Subscription, Task,
  advanced::widget::{Id, operate, operation::focusable},
  keyboard,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Chord {
  FocusSearch,
  OpenSettings,
  Quit,
}

impl Chord {
  pub fn for_event(event: &iced::Event) -> Option<Chord> {
    let iced::Event::Keyboard(keyboard::Event::KeyPressed {
      key,
      modifiers,
      ..
    }) = event
    else {
      return None;
    };

    Chord::for_key(key, modifiers.command())
  }

  pub fn for_key(key: &keyboard::Key, command: bool) -> Option<Chord> {
    if !command {
      return None;
    }

    match key {
      keyboard::Key::Character(c) if c.as_str() == "," => Some(Chord::OpenSettings),
      keyboard::Key::Character(c) if c.as_str().eq_ignore_ascii_case("k") => Some(Chord::FocusSearch),
      keyboard::Key::Character(c) if c.as_str().eq_ignore_ascii_case("q") => Some(Chord::Quit),
      _ => None,
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaletteKey {
  Activate,
  Close,
  MoveDown,
  MoveUp,
  Open,
}

impl PaletteKey {
  pub fn for_event(event: &iced::Event, open: bool, text_focused: bool) -> Option<PaletteKey> {
    let iced::Event::Keyboard(keyboard::Event::KeyPressed {
      key, ..
    }) = event
    else {
      return None;
    };
    PaletteKey::for_key(key, open, text_focused)
  }

  pub fn for_key(key: &keyboard::Key, open: bool, text_focused: bool) -> Option<PaletteKey> {
    if open {
      return match key {
        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(PaletteKey::MoveDown),
        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(PaletteKey::MoveUp),
        keyboard::Key::Named(keyboard::key::Named::Enter) => Some(PaletteKey::Activate),
        keyboard::Key::Named(keyboard::key::Named::Escape) => Some(PaletteKey::Close),
        _ => None,
      };
    }
    match key {
      keyboard::Key::Character(c) if c.as_str() == "/" && !text_focused => Some(PaletteKey::Open),
      _ => None,
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct FocusTracker {
  focused: Option<Id>,
}

impl FocusTracker {
  // Test-only accessor: the focus-probe and Ctrl/Cmd+K wiring tests assert the exact focused Id;
  // production code only needs the boolean `is_text_input_focused`.
  #[cfg_attr(not(test), expect(dead_code))]
  pub fn focused_id(&self) -> Option<&Id> {
    self.focused.as_ref()
  }

  pub fn is_text_input_focused(&self) -> bool {
    self.focused.is_some()
  }

  pub fn set_focused(&mut self, id: Option<Id>) {
    self.focused = id;
  }
}

pub fn probe_focus<M: Send + 'static>(map: impl Fn(Id) -> M + Send + 'static) -> Task<M> {
  operate(focusable::find_focused()).map(map)
}

pub fn subscription<M: Send + 'static>(map: impl Fn(Chord) -> M + Clone + Send + 'static) -> Subscription<M> {
  iced::event::listen_with(|event, _status, _id| Chord::for_event(&event)).map(map)
}

#[cfg(test)]
mod tests {
  mod chord {
    mod for_key {
      use pretty_assertions::assert_eq;

      use super::super::super::*;

      fn character(value: &str) -> keyboard::Key {
        keyboard::Key::Character(value.into())
      }

      #[test]
      fn it_maps_command_q_to_quit() {
        assert_eq!(Chord::for_key(&character("q"), true), Some(Chord::Quit));
      }

      #[test]
      fn it_maps_uppercase_q_to_quit() {
        assert_eq!(Chord::for_key(&character("Q"), true), Some(Chord::Quit));
      }

      #[test]
      fn it_maps_command_comma_to_open_settings() {
        assert_eq!(Chord::for_key(&character(","), true), Some(Chord::OpenSettings));
      }

      #[test]
      fn it_maps_command_k_to_focus_search() {
        assert_eq!(Chord::for_key(&character("k"), true), Some(Chord::FocusSearch));
      }

      #[test]
      fn it_maps_uppercase_k_to_focus_search() {
        assert_eq!(Chord::for_key(&character("K"), true), Some(Chord::FocusSearch));
      }

      #[test]
      fn it_ignores_k_without_the_command_modifier() {
        assert_eq!(Chord::for_key(&character("k"), false), None);
      }

      #[test]
      fn it_ignores_q_without_the_command_modifier() {
        assert_eq!(Chord::for_key(&character("q"), false), None);
      }

      #[test]
      fn it_ignores_comma_without_the_command_modifier() {
        assert_eq!(Chord::for_key(&character(","), false), None);
      }

      #[test]
      fn it_ignores_unbound_command_keys() {
        assert_eq!(Chord::for_key(&character("z"), true), None);
      }

      #[test]
      fn it_ignores_named_keys() {
        assert_eq!(
          Chord::for_key(&keyboard::Key::Named(keyboard::key::Named::Enter), true),
          None
        );
      }
    }

    mod for_event {
      use pretty_assertions::assert_eq;

      use super::super::super::*;

      fn key_pressed(value: &str, modifiers: keyboard::Modifiers) -> iced::Event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
          key: keyboard::Key::Character(value.into()),
          modified_key: keyboard::Key::Character(value.into()),
          physical_key: keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
          location: keyboard::Location::Standard,
          modifiers,
          text: None,
          repeat: false,
        })
      }

      #[test]
      fn it_dispatches_the_platform_quit_chord() {
        let event = key_pressed("q", keyboard::Modifiers::COMMAND);

        assert_eq!(Chord::for_event(&event), Some(Chord::Quit));
      }

      #[test]
      fn it_dispatches_the_platform_settings_chord() {
        let event = key_pressed(",", keyboard::Modifiers::COMMAND);

        assert_eq!(Chord::for_event(&event), Some(Chord::OpenSettings));
      }

      #[test]
      fn it_dispatches_the_platform_focus_search_chord() {
        let event = key_pressed("k", keyboard::Modifiers::COMMAND);

        assert_eq!(Chord::for_event(&event), Some(Chord::FocusSearch));
      }

      #[test]
      fn it_ignores_an_unmodified_keypress() {
        let event = key_pressed("q", keyboard::Modifiers::empty());

        assert_eq!(Chord::for_event(&event), None);
      }

      #[test]
      fn it_ignores_non_keyboard_events() {
        let event = iced::Event::Keyboard(keyboard::Event::ModifiersChanged(keyboard::Modifiers::COMMAND));

        assert_eq!(Chord::for_event(&event), None);
      }
    }
  }

  mod palette_key {
    mod for_key {
      use pretty_assertions::assert_eq;

      use super::super::super::*;

      fn slash() -> keyboard::Key {
        keyboard::Key::Character("/".into())
      }

      fn named(key: keyboard::key::Named) -> keyboard::Key {
        keyboard::Key::Named(key)
      }

      #[test]
      fn it_opens_on_slash_when_no_text_input_is_focused() {
        assert_eq!(PaletteKey::for_key(&slash(), false, false), Some(PaletteKey::Open));
      }

      #[test]
      fn it_ignores_slash_when_a_text_input_is_focused() {
        assert_eq!(PaletteKey::for_key(&slash(), false, true), None);
      }

      #[test]
      fn it_ignores_slash_while_already_open() {
        assert_eq!(PaletteKey::for_key(&slash(), true, false), None);
      }

      #[test]
      fn it_maps_arrows_enter_and_escape_while_open() {
        assert_eq!(
          PaletteKey::for_key(&named(keyboard::key::Named::ArrowDown), true, false),
          Some(PaletteKey::MoveDown)
        );
        assert_eq!(
          PaletteKey::for_key(&named(keyboard::key::Named::ArrowUp), true, false),
          Some(PaletteKey::MoveUp)
        );
        assert_eq!(
          PaletteKey::for_key(&named(keyboard::key::Named::Enter), true, false),
          Some(PaletteKey::Activate)
        );
        assert_eq!(
          PaletteKey::for_key(&named(keyboard::key::Named::Escape), true, false),
          Some(PaletteKey::Close)
        );
      }

      #[test]
      fn it_ignores_navigation_keys_while_closed() {
        assert_eq!(
          PaletteKey::for_key(&named(keyboard::key::Named::ArrowDown), false, false),
          None
        );
        assert_eq!(
          PaletteKey::for_key(&named(keyboard::key::Named::Escape), false, false),
          None
        );
      }
    }
  }

  mod focus_tracker {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_reports_no_focus_by_default() {
      let tracker = FocusTracker::default();

      assert_eq!(tracker.is_text_input_focused(), false);
      assert_eq!(tracker.focused_id(), None);
    }

    #[test]
    fn it_reports_focus_once_an_input_is_set() {
      let id = Id::from("search");
      let mut tracker = FocusTracker::default();

      tracker.set_focused(Some(id.clone()));

      assert_eq!(tracker.is_text_input_focused(), true);
      assert_eq!(tracker.focused_id(), Some(&id));
    }

    #[test]
    fn it_clears_focus_when_set_to_none() {
      let mut tracker = FocusTracker::default();
      tracker.set_focused(Some(Id::from("search")));

      tracker.set_focused(None);

      assert_eq!(tracker.is_text_input_focused(), false);
    }
  }
}
