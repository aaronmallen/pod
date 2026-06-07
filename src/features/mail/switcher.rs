use iced::Element;

use super::{Message, RosterPilot, Scope, State};
use crate::ui::components::picker::{
  PickerGroup, TriggerPortrait, picker_character_row, picker_dropdown as picker_dropdown_panel, picker_trigger,
  trigger_identity,
};

pub(super) fn trigger(state: &State) -> Element<'_, Message> {
  picker_trigger(trigger_content(state), state.picker_open(), Message::PickerToggled)
}

fn trigger_content(state: &State) -> Element<'_, Message> {
  let (title, subtitle, portrait) = match state.active() {
    Scope::AllInboxes => (
      "All Inboxes".to_owned(),
      format!("{} unread", state.unified_unread()),
      None,
    ),
    Scope::Character(id) => match state.roster().iter().find(|pilot| pilot.id == id) {
      Some(pilot) => (
        pilot.name.clone(),
        format!("{} unread", pilot.unread),
        Some(TriggerPortrait {
          id: pilot.id,
          name: pilot.name.clone(),
          path: pilot.portrait.clone(),
        }),
      ),
      None => ("Mail".to_owned(), String::new(), None),
    },
  };

  trigger_identity(title, subtitle, portrait)
}

pub(super) fn dropdown(state: &State) -> Element<'_, Message> {
  let mut groups: Vec<PickerGroup<'_, Message>> = Vec::with_capacity(1);

  if !state.roster().is_empty() {
    groups.push(PickerGroup {
      title: Some(format!("Switch character · {} unread", state.unified_unread())),
      items: state.roster().iter().map(|pilot| character_row(state, pilot)).collect(),
    });
  }

  picker_dropdown_panel(groups)
}

fn character_row<'a>(state: &'a State, pilot: &'a RosterPilot) -> Element<'a, Message> {
  let sub = if pilot.unread > 0 {
    format!("{}  ·  {} unread", pilot.corp, pilot.unread)
  } else {
    pilot.corp.clone()
  };
  picker_character_row(
    pilot.id,
    pilot.name.clone(),
    sub,
    pilot.portrait.clone(),
    None,
    matches!(state.active(), Scope::Character(id) if id == pilot.id),
    Message::ScopeSelected(Scope::Character(pilot.id)),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn pilot(id: i64, unread: i64) -> RosterPilot {
    RosterPilot {
      corp: "TST".to_owned(),
      id,
      name: format!("Pilot {id}"),
      portrait: None,
      unread,
    }
  }

  fn state_with(roster: Vec<RosterPilot>, active: Scope) -> State {
    let mut state = State::new();
    state.roster = roster;
    state.active = active;
    state
  }

  #[test]
  fn it_renders_the_trigger_in_all_inboxes_scope() {
    let state = state_with(vec![pilot(1, 3)], Scope::AllInboxes);

    let _el: Element<'_, Message> = trigger(&state);
  }

  #[test]
  fn it_renders_the_trigger_in_a_character_scope() {
    let state = state_with(vec![pilot(1, 3), pilot(2, 0)], Scope::Character(2));

    let _el: Element<'_, Message> = trigger(&state);
  }

  #[test]
  fn it_builds_a_character_group_when_the_roster_is_non_empty() {
    let state = state_with(vec![pilot(1, 3), pilot(2, 0)], Scope::AllInboxes);

    let _el: Element<'_, Message> = dropdown(&state);
  }

  #[test]
  fn it_builds_an_empty_dropdown_when_the_roster_is_empty() {
    let state = state_with(Vec::new(), Scope::AllInboxes);

    let _el: Element<'_, Message> = dropdown(&state);
  }
}
