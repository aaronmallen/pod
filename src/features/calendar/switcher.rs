use iced::{
  Border, Element, Length,
  widget::{Space, container},
};

use super::{Message, RosterPilot, Scope, State, palette, registry_scopes};
use crate::{
  config::Feature,
  features::character_manager,
  ui::{
    components::{
      icon::Icon,
      picker::{
        PickerGroup, TriggerPortrait, picker_character_row, picker_dropdown as picker_dropdown_panel, picker_row,
        picker_trigger, trigger_badge_identity, trigger_identity,
      },
    },
    style::radius,
  },
};

const SWATCH: f32 = 10.0;

pub(super) fn trigger(state: &State) -> Element<'_, Message> {
  picker_trigger(trigger_content(state), state.picker_open(), Message::PickerToggled)
}

pub(super) fn dropdown(state: &State) -> Element<'_, Message> {
  let mut items: Vec<Element<'_, Message>> = vec![picker_row(
    format!("All Pilots \u{00B7} {} calendars", state.roster().len()),
    matches!(state.active(), Scope::All),
    Message::ScopeSelected(Scope::All),
  )];
  items.extend(
    state
      .roster()
      .iter()
      .enumerate()
      .map(|(index, pilot)| character_row(state, index, pilot)),
  );

  picker_dropdown_panel(vec![PickerGroup {
    title: Some("Calendar source".to_owned()),
    items,
  }])
}

fn character_row<'a>(state: &'a State, index: usize, pilot: &'a RosterPilot) -> Element<'a, Message> {
  let needs_reauth = character_manager::needs_reauthorization(pilot.granted_scopes.as_deref(), registry_scopes());
  let trailing = (!needs_reauth).then(|| swatch(palette::pilot_color(index)));

  picker_character_row(
    pilot.id,
    pilot.name.clone(),
    pilot.corp.clone(),
    pilot.portrait.path(),
    trailing,
    matches!(state.active(), Scope::Mine(id) if id == pilot.id),
    needs_reauth.then(|| Feature::Calendar.noun()),
    Message::ScopeSelected(Scope::Mine(pilot.id)),
  )
}

fn swatch<'a>(fill: iced::Color) -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(SWATCH))
    .height(Length::Fixed(SWATCH))
    .style(move |_| container::Style {
      background: Some(iced::Background::Color(fill)),
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn trigger_content(state: &State) -> Element<'_, Message> {
  match state.active() {
    Scope::Mine(id) => match state.pilot(id) {
      Some(pilot) => trigger_identity(
        pilot.name.clone(),
        pilot.corp.clone(),
        Some(TriggerPortrait {
          id: pilot.id,
          name: pilot.name.clone(),
          path: pilot.portrait.path(),
        }),
      ),
      None => trigger_identity("Calendar".to_owned(), String::new(), None),
    },
    _ => trigger_badge_identity(
      Icon::calendar(),
      "All Pilots".to_owned(),
      format!("{} calendars combined", state.roster().len()),
    ),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    config::{CalendarTweaks, FeatureFlags},
    store::images,
  };

  fn pilot(id: i64) -> RosterPilot {
    RosterPilot {
      corp: "TST".to_owned(),
      granted_scopes: None,
      id,
      name: format!("Pilot {id}"),
      portrait: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      },
    }
  }

  fn state_with(active: Scope, roster: Vec<RosterPilot>) -> State {
    let now = chrono::Utc::now();
    let mut state = State::new(0, now, CalendarTweaks::default(), FeatureFlags::default());
    state.active = active;
    state.roster = roster;
    state
  }

  mod dropdown {
    use super::*;

    #[test]
    fn it_builds_a_dropdown_with_all_pilots_plus_each_character() {
      let state = state_with(Scope::All, vec![pilot(1), pilot(2)]);

      let _el: Element<'_, Message> = dropdown(&state);
    }
  }

  mod trigger {
    use super::*;

    #[test]
    fn it_renders_a_character_trigger() {
      let state = state_with(Scope::Mine(1), vec![pilot(1)]);

      let _el: Element<'_, Message> = trigger(&state);
    }

    #[test]
    fn it_renders_the_combined_trigger() {
      let state = state_with(Scope::All, vec![pilot(1), pilot(2)]);

      let _el: Element<'_, Message> = trigger(&state);
    }
  }
}
