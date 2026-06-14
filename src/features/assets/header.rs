use iced::Element;

use super::{Message, RosterCorp, RosterPilot, Scope, State, fmt_count, fmt_isk, fmt_volume};
use crate::{
  config::Feature,
  features::{character_manager, registry},
  ui::{
    components::{
      header::{header as shared_header, header_divider, stat_block},
      icon::Icon,
      picker::{
        PickerGroup, TriggerPortrait, picker_character_row, picker_dropdown as picker_dropdown_panel, picker_row,
        picker_trigger, trigger_badge_identity, trigger_identity,
      },
    },
    style::color,
  },
};

pub(super) fn header(state: &State) -> Element<'_, Message> {
  let totals = state.totals;

  let left: Vec<Element<'_, Message>> = vec![
    scope_picker(state),
    header_divider(),
    stat_block("Asset value", fmt_isk(totals.value), color::text::PRIMARY, None),
    header_divider(),
    stat_block("Volume", fmt_volume(totals.volume), color::text::PRIMARY, None),
    header_divider(),
    stat_block("Items", fmt_count(totals.items), color::text::PRIMARY, None),
    header_divider(),
    stat_block("Locations", fmt_count(totals.locations), color::text::PRIMARY, None),
  ];

  shared_header(left, vec![])
}

fn scope_picker(state: &State) -> Element<'_, Message> {
  picker_trigger(trigger(state), state.picker_open, Message::PickerToggled)
}

fn trigger(state: &State) -> Element<'_, Message> {
  match state.active() {
    Scope::All => trigger_badge_identity(
      Icon::assets(),
      "All Assets",
      format!("{} characters", state.roster.len()),
    ),
    Scope::Character(id) => match state.roster.iter().find(|pilot| pilot.id == id) {
      Some(pilot) => trigger_identity(
        pilot.name.clone(),
        pilot.corp.clone(),
        Some(TriggerPortrait {
          id: pilot.id,
          name: pilot.name.clone(),
          path: pilot.portrait.path(),
        }),
      ),
      None => trigger_identity("Character", String::new(), None),
    },
    Scope::Corporation(id) => match state.corporations.iter().find(|corp| corp.id == id) {
      Some(corp) => trigger_identity(
        corp.name.clone(),
        corp.ticker.clone(),
        Some(TriggerPortrait {
          id: corp.id,
          name: corp.name.clone(),
          path: corp.logo.path(),
        }),
      ),
      None => trigger_identity("Corporation", String::new(), None),
    },
  }
}

pub(super) fn picker_dropdown(state: &State) -> Element<'_, Message> {
  let mut groups: Vec<PickerGroup<'_, Message>> = Vec::with_capacity(3);

  groups.push(PickerGroup {
    title: None,
    items: vec![picker_row(
      "All Assets",
      state.active() == Scope::All,
      Message::ScopeSelected(Scope::All),
    )],
  });

  if !state.roster.is_empty() {
    groups.push(PickerGroup {
      title: Some("Characters".to_owned()),
      items: state
        .roster
        .iter()
        .map(|pilot| character_row(pilot, state.active()))
        .collect(),
    });
  }

  if !state.corporations.is_empty() {
    groups.push(PickerGroup {
      title: Some("Corporations".to_owned()),
      items: state
        .corporations
        .iter()
        .map(|corp| corporation_row(corp, state.active()))
        .collect(),
    });
  }

  picker_dropdown_panel(groups)
}

fn character_row(pilot: &RosterPilot, active: Scope) -> Element<'_, Message> {
  let required_scopes = registry::descriptor(Feature::AssetTracking).scopes;
  let needs_reauth = character_manager::needs_reauthorization(pilot.granted_scopes.as_deref(), required_scopes);
  picker_character_row(
    pilot.id,
    pilot.name.clone(),
    pilot.corp.clone(),
    pilot.portrait.path(),
    None,
    active == Scope::Character(pilot.id),
    needs_reauth.then(|| Feature::AssetTracking.noun()),
    Message::ScopeSelected(Scope::Character(pilot.id)),
  )
}

fn corporation_row(corp: &RosterCorp, active: Scope) -> Element<'_, Message> {
  picker_character_row(
    corp.id,
    corp.name.clone(),
    corp.ticker.clone(),
    corp.logo.path(),
    None,
    active == Scope::Corporation(corp.id),
    None,
    Message::ScopeSelected(Scope::Corporation(corp.id)),
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{features::assets::State, store::images};

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

  fn corporation(id: i64) -> RosterCorp {
    RosterCorp {
      id,
      logo: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CorporationLogo,
      },
      name: format!("Corp {id}"),
      ticker: "CRP".to_owned(),
    }
  }

  mod dropdown {
    use super::*;

    #[test]
    fn it_renders_all_three_groups_when_characters_and_corporations_are_owned() {
      let mut state = State::new();
      state.set_picker_for_test(Scope::Character(7), vec![pilot(7), pilot(9)], vec![corporation(98)]);

      let _el: Element<'_, Message> = picker_dropdown(&state);
    }

    #[test]
    fn it_renders_only_the_all_assets_group_with_an_empty_roster() {
      let state = State::new();

      let _el: Element<'_, Message> = picker_dropdown(&state);
    }
  }

  mod band {
    use super::*;

    #[test]
    fn it_renders_the_header_band_for_each_scope_form() {
      for scope in [
        Scope::All,
        Scope::Character(7),
        Scope::Corporation(98),
        Scope::Character(404),
      ] {
        let mut state = State::new();
        state.set_picker_for_test(scope, vec![pilot(7)], vec![corporation(98)]);
        let _el: Element<'_, Message> = header(&state);
      }
    }
  }
}
