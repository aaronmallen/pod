use chrono::{DateTime, Utc};
use iced::Element;

use super::{Message, RosterCorp, RosterPilot, Scope, State, Tab, fmt_isk};
use crate::{
  config::Feature,
  features::{roster, shell::registry},
  ui::{
    components::{
      button::Button,
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

const HEADER_BUTTON_HEIGHT: f32 = 36.0;

pub(super) fn header(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  let liquid = super::scope_liquid(state);

  let sliced = super::sliced_series(state, now.date_naive());
  let net_worth = super::series_current(sliced).or(liquid);
  let change = super::series_change(sliced);
  let change_color = if change >= 0.0 {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let change_text = if sliced.len() < 2 {
    "\u{2014}".to_owned()
  } else {
    let sign = if change >= 0.0 { "+" } else { "-" };
    format!("{sign}{}", fmt_isk(Some(change.abs())))
  };

  let mut left: Vec<Element<'_, Message>> = Vec::new();
  // Budgets are all-wallet by definition (ADR-0044), so the Budget tab renders
  // no scope picker; every other tab keeps it.
  if state.tab != Tab::Budget {
    left.push(scope_picker(state));
    left.push(header_divider());
  }
  left.extend([
    stat_block(
      super::i18n::tr_static("wallet.header.liquid_isk"),
      fmt_isk(liquid),
      color::text::PRIMARY,
      None,
    ),
    header_divider(),
    stat_block(
      super::i18n::tr_static("wallet.header.net_worth"),
      fmt_isk(net_worth),
      color::text::PRIMARY,
      None,
    ),
    header_divider(),
    stat_block(
      &t!("wallet.header.change", label => state.timeframe().label()),
      change_text,
      change_color,
      None,
    ),
  ]);

  let mut actions: Vec<Element<'_, Message>> = Vec::new();
  if state.tab == Tab::Budget {
    actions.push(automation_btn());
  }

  shared_header(left, actions)
}

fn automation_btn<'a>() -> Element<'a, Message> {
  Button::secondary(t!("wallet.header.automation"))
    .icon(Icon::settings().size(14.0))
    .height(HEADER_BUTTON_HEIGHT)
    .on_press(Message::BudgetGlobalRulesOpened)
    .into()
}

fn scope_picker(state: &State) -> Element<'_, Message> {
  picker_trigger(trigger(state), state.picker_open, Message::PickerToggled)
}

fn trigger(state: &State) -> Element<'_, Message> {
  match state.active() {
    Scope::All => trigger_badge_identity(
      Icon::wallet(),
      t!("wallet.header.all_wallets").into_owned(),
      t!("wallet.header.characters_combined", count => state.roster.len()).into_owned(),
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
      None => trigger_identity(t!("wallet.header.character").into_owned(), String::new(), None),
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
      None => trigger_identity(t!("wallet.header.corporation").into_owned(), String::new(), None),
    },
  }
}

pub(super) fn picker_dropdown(state: &State) -> Element<'_, Message> {
  let mut groups: Vec<PickerGroup<'_, Message>> = Vec::with_capacity(3);

  groups.push(PickerGroup {
    title: None,
    items: vec![all_wallets_row(state)],
  });

  if !state.roster.is_empty() {
    groups.push(PickerGroup {
      title: Some(t!("wallet.header.characters").into_owned()),
      items: state.roster.iter().map(|pilot| character_row(state, pilot)).collect(),
    });
  }

  if !state.corporations.is_empty() {
    groups.push(PickerGroup {
      title: Some(t!("wallet.header.corporations").into_owned()),
      items: state.corporations.iter().map(|corp| corp_row(state, corp)).collect(),
    });
  }

  picker_dropdown_panel(groups)
}

fn all_wallets_row(state: &State) -> Element<'_, Message> {
  let label = t!(
    "wallet.header.all_wallets_row",
    isk => fmt_isk(super::combined_liquid(state)),
    count => state.roster.len(),
  )
  .into_owned();
  picker_row(
    label,
    matches!(state.active(), Scope::All),
    Message::ScopeSelected(Scope::All),
  )
}

fn character_row<'a>(state: &'a State, pilot: &'a RosterPilot) -> Element<'a, Message> {
  let liquid = state
    .financials
    .iter()
    .find(|row| row.character_id == pilot.id)
    .and_then(|row| row.liquid);
  let sub = t!("wallet.header.character_row", corp => pilot.corp, isk => fmt_isk(liquid)).into_owned();
  let required_scopes = registry::descriptor(Feature::Wallet).scopes;
  let needs_reauth = roster::needs_reauthorization(pilot.granted_scopes.as_deref(), required_scopes);
  picker_character_row(
    pilot.id,
    pilot.name.clone(),
    sub,
    pilot.portrait.path(),
    None,
    matches!(state.active(), Scope::Character(id) if id == pilot.id),
    needs_reauth.then(|| Feature::Wallet.noun()),
    Message::ScopeSelected(Scope::Character(pilot.id)),
  )
}

fn corp_row<'a>(state: &'a State, corp: &'a RosterCorp) -> Element<'a, Message> {
  picker_character_row(
    corp.id,
    corp.name.clone(),
    corp.ticker.clone(),
    corp.logo.path(),
    None,
    matches!(state.active(), Scope::Corporation(id) if id == corp.id),
    None,
    Message::ScopeSelected(Scope::Corporation(corp.id)),
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::images;

  fn pilot(id: i64) -> RosterPilot {
    RosterPilot {
      corp: "TST".to_owned(),
      granted_scopes: None,
      id,
      liquid: Some(1_000.0),
      name: format!("Pilot {id}"),
      portrait: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      },
    }
  }

  fn corp(id: i64) -> RosterCorp {
    RosterCorp {
      id,
      liquid: None,
      logo: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CorporationLogo,
      },
      name: format!("Corp {id}"),
      ticker: "CRP".to_owned(),
    }
  }

  fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-06-09T00:00:00Z")
      .unwrap()
      .with_timezone(&Utc)
  }

  mod header {
    use super::*;

    #[test]
    fn it_renders_with_a_closed_picker() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.roster = vec![pilot(1), pilot(2)];

      let _el: Element<'_, Message> = header(&state, now());
    }

    #[test]
    fn it_renders_without_the_scope_picker_on_the_budget_tab() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.roster = vec![pilot(1), pilot(2)];
      state.tab = Tab::Budget;

      let _el: Element<'_, Message> = header(&state, now());
    }
  }

  mod picker_dropdown {
    use super::*;

    #[test]
    fn it_builds_all_three_groups_when_corps_and_chars_exist() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.roster = vec![pilot(1)];
      state.corporations = vec![corp(98_000_001)];

      let _el: Element<'_, Message> = picker_dropdown(&state);
    }

    #[test]
    fn it_builds_with_only_the_all_wallets_row_when_empty() {
      let state = State::new(crate::config::FeatureFlags::default());

      let _el: Element<'_, Message> = picker_dropdown(&state);
    }
  }
}
