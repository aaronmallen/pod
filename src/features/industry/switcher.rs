use iced::Element;

use super::{Message, RosterOwner, Scope, State};
use crate::{
  store::images,
  ui::components::{
    icon::Icon,
    picker::{
      PickerGroup, TriggerPortrait, picker_character_row, picker_dropdown, picker_row, picker_trigger,
      trigger_badge_identity, trigger_identity,
    },
  },
};

pub(super) fn dropdown(state: &State) -> Element<'_, Message> {
  let combined = picker_row(
    format!(
      "All Industry \u{00B7} {} pilots, {} corps",
      character_count(state),
      corporation_count(state)
    ),
    matches!(state.active(), Scope::All),
    Message::ScopeSelected(Scope::All),
  );

  let characters: Vec<Element<'_, Message>> = state
    .roster()
    .iter()
    .filter(|owner| !owner.is_corporation)
    .map(|owner| character_row(state, owner))
    .collect();

  let corporations: Vec<Element<'_, Message>> = state
    .roster()
    .iter()
    .filter(|owner| owner.is_corporation)
    .map(|owner| corporation_row(state, owner))
    .collect();

  let mut groups = vec![PickerGroup {
    title: Some("Scope".to_owned()),
    items: vec![combined],
  }];
  if !characters.is_empty() {
    groups.push(PickerGroup {
      title: Some("Characters".to_owned()),
      items: characters,
    });
  }
  if !corporations.is_empty() {
    groups.push(PickerGroup {
      title: Some("Corporations".to_owned()),
      items: corporations,
    });
  }

  picker_dropdown(groups)
}

pub(super) fn trigger(state: &State) -> Element<'_, Message> {
  picker_trigger(trigger_content(state), state.picker_open(), Message::PickerToggled)
}

fn character_count(state: &State) -> usize {
  state.roster().iter().filter(|owner| !owner.is_corporation).count()
}

fn character_row<'a>(state: &'a State, owner: &'a RosterOwner) -> Element<'a, Message> {
  let needs_reauth =
    !crate::ui::components::forbidden::missing_scopes(owner.granted_scopes.as_deref(), state.required_scopes())
      .is_empty();

  picker_character_row(
    owner.id,
    owner.name.clone(),
    owner.corp.clone(),
    owner.portrait.as_ref().and_then(images::ImageState::path),
    None,
    matches!(state.active(), Scope::Char(id) if id == owner.id),
    needs_reauth.then_some("Industry"),
    Message::ScopeSelected(Scope::Char(owner.id)),
  )
}

fn corporation_count(state: &State) -> usize {
  state.roster().iter().filter(|owner| owner.is_corporation).count()
}

fn corporation_row<'a>(state: &'a State, owner: &'a RosterOwner) -> Element<'a, Message> {
  // Matches the Assets picker: corporation rows carry no re-auth indicator. Corp re-authorization
  // is surfaced through the roster corp context menu.
  picker_character_row(
    owner.id,
    owner.name.clone(),
    owner.corp.clone(),
    owner.logo.as_ref().and_then(images::ImageState::path),
    None,
    matches!(state.active(), Scope::Corp(id) if id == owner.id),
    None,
    Message::ScopeSelected(Scope::Corp(owner.id)),
  )
}

fn trigger_content(state: &State) -> Element<'_, Message> {
  match state.active() {
    Scope::Char(id) => match state
      .roster()
      .iter()
      .find(|owner| owner.id == id && !owner.is_corporation)
    {
      Some(owner) => trigger_identity(
        owner.name.clone(),
        owner.corp.clone(),
        Some(TriggerPortrait {
          id: owner.id,
          name: owner.name.clone(),
          path: owner.portrait.as_ref().and_then(images::ImageState::path),
        }),
      ),
      None => trigger_identity("Industry".to_owned(), String::new(), None),
    },
    Scope::Corp(id) => match state
      .roster()
      .iter()
      .find(|owner| owner.id == id && owner.is_corporation)
    {
      Some(owner) => trigger_identity(owner.name.clone(), owner.corp.clone(), None),
      None => trigger_identity("Industry".to_owned(), String::new(), None),
    },
    Scope::All => trigger_badge_identity(
      Icon::industry(),
      "All Industry".to_owned(),
      format!(
        "{} pilots, {} corps combined",
        character_count(state),
        corporation_count(state)
      ),
    ),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::images;

  fn character(id: i64) -> RosterOwner {
    RosterOwner {
      corp: "TST".to_owned(),
      corporation_id: Some(98),
      granted_scopes: None,
      id,
      is_corporation: false,
      logo: None,
      name: format!("Pilot {id}"),
      portrait: Some(images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      }),
      slots: super::super::loaders::SlotCaps::default(),
    }
  }

  fn corporation(id: i64) -> RosterOwner {
    RosterOwner {
      corp: "TSC".to_owned(),
      corporation_id: None,
      granted_scopes: None,
      id,
      is_corporation: true,
      logo: Some(images::ImageState::Stale {
        id,
        kind: images::ImageKind::CorporationLogo,
      }),
      name: format!("Corp {id}"),
      portrait: None,
      slots: super::super::loaders::SlotCaps::default(),
    }
  }

  fn state_with(active: Scope, roster: Vec<RosterOwner>) -> State {
    let mut state = State::new(
      0,
      vec![crate::clients::esi::scopes::CHARACTER_INDUSTRY_JOBS],
      crate::config::FeatureFlags::default(),
      super::super::FacilityDefaults::default(),
      None,
      false,
    );
    state.active = active;
    state.roster = roster;
    state
  }

  mod dropdown {
    use super::*;

    #[test]
    fn it_builds_a_dropdown_with_all_scopes_characters_and_corps() {
      let state = state_with(Scope::All, vec![character(1), corporation(98)]);

      let _el: Element<'_, Message> = dropdown(&state);
    }
  }

  mod trigger {
    use super::*;

    #[test]
    fn it_renders_a_character_trigger() {
      let state = state_with(Scope::Char(1), vec![character(1)]);

      let _el: Element<'_, Message> = trigger(&state);
    }

    #[test]
    fn it_renders_a_corporation_trigger() {
      let state = state_with(Scope::Corp(98), vec![corporation(98)]);

      let _el: Element<'_, Message> = trigger(&state);
    }

    #[test]
    fn it_renders_the_combined_trigger() {
      let state = state_with(Scope::All, vec![character(1)]);

      let _el: Element<'_, Message> = trigger(&state);
    }
  }
}
