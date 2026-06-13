// The module and its public surface (Message/State/Scope/update/view/load/subscription) are mounted
// by the app router in a follow-up wiring change; until then the wiring is dead from the binary's
// perspective — the re-exported public types likewise have no in-binary consumer yet. The wiring
// task removes this allow.
#![allow(dead_code, unused_imports)]

mod jobs;
mod loaders;
mod shell;
mod side_rail;
mod switcher;
mod tweaks;

use std::time::Duration;

use chrono::{DateTime, Utc};
use iced::{Element, Subscription, Task};

pub use self::{
  loaders::{Activity, IndustryJob, Loaded, Owner, RosterOwner, SlotBucket, SlotCaps},
  tweaks::{BarColor, Density, GroupBy, IndustryTweaks},
};
use crate::store::{Database, images};

/// Sentinel character id meaning "no pilot selected" — opens the combined `Scope::All` view.
pub const EMPTY_INDUSTRY_SELECTION: i64 = 0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Filter {
  #[default]
  All,
  Active,
  Ready,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Scope {
  #[default]
  All,
  Char(i64),
  Corp(i64),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Tab {
  #[default]
  Jobs,
}

impl Tab {
  pub const ALL: [Tab; 1] = [Tab::Jobs];

  pub fn label(self) -> &'static str {
    match self {
      Tab::Jobs => "Jobs",
    }
  }
}

#[derive(Clone, Debug)]
pub enum Message {
  FilterSelected(Filter),
  GroupBySelected(GroupBy),
  Loaded(Box<Loaded>),
  PickerToggled,
  ReauthRequested(i64),
  ScopeSelected(Scope),
  TabSelected(Tab),
  Tick,
  TweakChanged(tweaks::Tweak),
  TweaksToggled,
}

#[derive(Debug)]
pub struct State {
  active: Scope,
  filter: Filter,
  jobs: Vec<IndustryJob>,
  picker_open: bool,
  required_scopes: Vec<&'static str>,
  roster: Vec<RosterOwner>,
  tab: Tab,
  tweaks: IndustryTweaks,
  tweaks_open: bool,
}

impl State {
  pub fn new(active: i64, tweaks: IndustryTweaks, required_scopes: Vec<&'static str>) -> Self {
    State {
      active: if active == EMPTY_INDUSTRY_SELECTION {
        Scope::All
      } else {
        Scope::Char(active)
      },
      filter: Filter::default(),
      jobs: Vec::new(),
      picker_open: false,
      required_scopes,
      roster: Vec::new(),
      tab: Tab::default(),
      tweaks,
      tweaks_open: false,
    }
  }

  pub fn active(&self) -> Scope {
    self.active
  }

  pub fn set_required_scopes(&mut self, scopes: Vec<&'static str>) {
    self.required_scopes = scopes;
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    self
      .roster
      .iter()
      .filter_map(|owner| owner.portrait.as_ref().or(owner.logo.as_ref()))
      .filter_map(images::ImageState::stale_key)
      .filter(|(_, id)| *id > 0)
      .collect()
  }

  pub fn tweaks(&self) -> IndustryTweaks {
    self.tweaks
  }

  pub(super) fn filter(&self) -> Filter {
    self.filter
  }

  pub(super) fn jobs(&self) -> &[IndustryJob] {
    &self.jobs
  }

  pub(super) fn owner(&self, owner: Owner) -> Option<&RosterOwner> {
    let is_corporation = matches!(owner, Owner::Corporation(_));
    self
      .roster
      .iter()
      .find(|entry| entry.id == owner.id() && entry.is_corporation == is_corporation)
  }

  pub(super) fn picker_open(&self) -> bool {
    self.picker_open
  }

  pub(super) fn required_scopes(&self) -> &[&'static str] {
    &self.required_scopes
  }

  pub(super) fn roster(&self) -> &[RosterOwner] {
    &self.roster
  }

  /// The (id, name, missing-scopes) gate for a per-character "Mine" view whose pilot lacks the
  /// required scopes; `None` for the combined view or an authorized pilot.
  pub(super) fn scope_gate(&self) -> Option<(i64, &str, Vec<&'static str>)> {
    let Scope::Char(id) = self.active else {
      return None;
    };
    let owner = self
      .roster
      .iter()
      .find(|owner| owner.id == id && !owner.is_corporation)?;
    let missing =
      crate::ui::components::forbidden::missing_scopes(owner.granted_scopes.as_deref(), &self.required_scopes);
    if missing.is_empty() {
      return None;
    }
    Some((id, owner.name.as_str(), missing))
  }

  pub(super) fn tab(&self) -> Tab {
    self.tab
  }

  pub(super) fn tweaks_open(&self) -> bool {
    self.tweaks_open
  }

  pub(super) fn unauthorized_characters(&self) -> Vec<&RosterOwner> {
    self
      .roster
      .iter()
      .filter(|owner| !owner.is_corporation)
      .filter(|owner| {
        !crate::ui::components::forbidden::missing_scopes(owner.granted_scopes.as_deref(), &self.required_scopes)
          .is_empty()
      })
      .collect()
  }

  /// Jobs visible in the combined view: jobs belonging to unauthorized characters are dropped (the
  /// combined view names them in the amber banner instead). Corporation jobs are always shown.
  pub(super) fn visible_jobs(&self) -> Vec<&IndustryJob> {
    self.jobs.iter().filter(|job| self.is_authorized(job.owner)).collect()
  }

  fn is_authorized(&self, owner: Owner) -> bool {
    match owner {
      Owner::Corporation(_) => true,
      Owner::Character(id) => self
        .roster
        .iter()
        .find(|owner| owner.id == id && !owner.is_corporation)
        .map(|owner| {
          crate::ui::components::forbidden::missing_scopes(owner.granted_scopes.as_deref(), &self.required_scopes)
            .is_empty()
        })
        // Characters absent from the roster are treated as authorized so jobs belonging to a
        // character not yet loaded are not silently hidden.
        .unwrap_or(true),
    }
  }
}

pub fn load(db: &Database, character: i64, required_scopes: &[&'static str]) -> Task<Message> {
  let scope = if character == EMPTY_INDUSTRY_SELECTION {
    Scope::All
  } else {
    Scope::Char(character)
  };
  reload(db, scope, required_scopes)
}

pub fn reload(db: &Database, scope: Scope, required_scopes: &[&'static str]) -> Task<Message> {
  let _ = required_scopes;
  Task::perform(loaders::load(db.clone(), scope), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
}

pub fn subscription(_state: &State) -> Subscription<Message> {
  iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick)
}

pub fn update(state: &mut State, message: Message, db: &Database, _now: DateTime<Utc>) -> Task<Message> {
  match message {
    Message::FilterSelected(filter) => {
      state.filter = filter;
      Task::none()
    }
    Message::GroupBySelected(group_by) => {
      state.tweaks.set_group_by(group_by);
      Task::none()
    }
    Message::Loaded(loaded) => {
      let Loaded {
        jobs,
        roster,
        scope,
      } = *loaded;
      // Drop results that belong to a scope the user already navigated away from.
      if scope == state.active {
        state.jobs = jobs;
        state.roster = roster;
      }
      Task::none()
    }
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Task::none()
    }
    Message::ReauthRequested(_) => Task::none(),
    Message::ScopeSelected(scope) => {
      state.active = scope;
      state.picker_open = false;
      reload(db, scope, &state.required_scopes)
    }
    Message::TabSelected(tab) => {
      state.tab = tab;
      Task::none()
    }
    Message::Tick => Task::none(),
    Message::TweakChanged(tweak) => {
      tweak.apply(&mut state.tweaks);
      Task::none()
    }
    Message::TweaksToggled => {
      state.tweaks_open = !state.tweaks_open;
      Task::none()
    }
  }
}

pub fn view<'a>(state: &'a State, required_scopes: &[&'static str], now: DateTime<Utc>) -> Element<'a, Message> {
  let _ = required_scopes;
  shell::shell(state, now)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::clients::esi::scopes;

  fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-06-13T12:00:00Z")
      .unwrap()
      .with_timezone(&Utc)
  }

  fn required() -> Vec<&'static str> {
    vec![scopes::CHARACTER_INDUSTRY_JOBS, scopes::CORPORATION_INDUSTRY_JOBS]
  }

  fn character_owner(id: i64, scopes: Option<&str>) -> RosterOwner {
    RosterOwner {
      corp: "TST".to_owned(),
      granted_scopes: scopes.map(str::to_owned),
      id,
      is_corporation: false,
      logo: None,
      name: format!("Pilot {id}"),
      portrait: Some(images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      }),
      slots: SlotCaps {
        manufacturing: 5,
        reactions: 0,
        science: 3,
      },
    }
  }

  fn corporation_owner(id: i64) -> RosterOwner {
    RosterOwner {
      corp: "TSC".to_owned(),
      granted_scopes: None,
      id,
      is_corporation: true,
      logo: Some(images::ImageState::Stale {
        id,
        kind: images::ImageKind::CorporationLogo,
      }),
      name: format!("Corp {id}"),
      portrait: None,
      slots: SlotCaps::default(),
    }
  }

  fn job(owner: Owner, job_id: i64, activity: Activity, end: &str) -> IndustryJob {
    IndustryJob {
      activity,
      blueprint_type_id: 681,
      cost: 1_000.0,
      end_date: end.to_owned(),
      facility: "Jita IV - Moon 4".to_owned(),
      installer: "Pilot 1".to_owned(),
      job_id,
      owner,
      owner_name: "Pilot 1".to_owned(),
      probability: (activity == Activity::Invention).then_some(0.42),
      product_name: "Rifter".to_owned(),
      product_type_id: Some(587),
      runs: 10,
      security: Some(0.9),
      start_date: "2026-06-13T11:00:00Z".to_owned(),
      system_name: Some("Jita".to_owned()),
      value: Some(1_000_000.0),
    }
  }

  fn granted() -> String {
    format!(
      "{} {}",
      scopes::CHARACTER_INDUSTRY_JOBS,
      scopes::CORPORATION_INDUSTRY_JOBS
    )
  }

  fn state_with(active: Scope, roster: Vec<RosterOwner>, jobs: Vec<IndustryJob>) -> State {
    let mut state = State::new(EMPTY_INDUSTRY_SELECTION, IndustryTweaks::default(), required());
    state.active = active;
    state.roster = roster;
    state.jobs = jobs;
    state
  }

  /// A combined-scope state with an authorized pilot, an unauthorized pilot, a corporation, and a
  /// spread of jobs (running, ready, invention, copy) exercising every render branch.
  fn populated() -> State {
    let granted = granted();
    let roster = vec![
      character_owner(1, Some(&granted)),
      character_owner(2, None),
      corporation_owner(98),
    ];
    let jobs = vec![
      job(Owner::Character(1), 10, Activity::Manufacturing, "2026-06-13T14:00:00Z"),
      job(Owner::Character(1), 11, Activity::Invention, "2026-06-13T11:30:00Z"),
      job(Owner::Character(2), 12, Activity::Copy, "2026-06-13T13:00:00Z"),
      job(Owner::Corporation(98), 13, Activity::Reactions, "2026-06-13T16:00:00Z"),
    ];
    state_with(Scope::All, roster, jobs)
  }

  mod rendering {
    use super::*;

    #[test]
    fn it_renders_the_combined_view() {
      let state = populated();

      let _el: Element<'_, Message> = view(&state, &required(), now());
    }

    #[test]
    fn it_renders_with_grouping_and_compact_density() {
      let mut state = populated();
      state.tweaks.set_density(Density::Compact);
      state.tweaks.set_group_by(GroupBy::Owner);
      state.tweaks.set_bar_color(BarColor::Status);

      let _el: Element<'_, Message> = view(&state, &required(), now());
    }

    #[test]
    fn it_renders_each_group_by_with_the_rail_off() {
      let mut state = populated();
      state.tweaks.set_show_rail(false);
      for group_by in [GroupBy::None, GroupBy::Owner, GroupBy::Activity, GroupBy::Facility] {
        state.tweaks.set_group_by(group_by);
        let _el: Element<'_, Message> = view(&state, &required(), now());
      }
    }

    #[test]
    fn it_renders_each_filter() {
      let mut state = populated();
      for filter in [Filter::All, Filter::Active, Filter::Ready] {
        state.filter = filter;
        let _el: Element<'_, Message> = view(&state, &required(), now());
      }
    }

    #[test]
    fn it_renders_the_scope_picker_overlay() {
      let mut state = populated();
      state.picker_open = true;

      let _el: Element<'_, Message> = view(&state, &required(), now());
    }

    #[test]
    fn it_renders_the_tweaks_overlay() {
      let mut state = populated();
      state.tweaks_open = true;

      let _el: Element<'_, Message> = view(&state, &required(), now());
    }

    #[test]
    fn it_renders_the_forbidden_gate_for_an_unauthorized_pilot() {
      let mut state = populated();
      state.active = Scope::Char(2);

      let _el: Element<'_, Message> = view(&state, &required(), now());
    }

    #[test]
    fn it_renders_an_empty_state() {
      let state = state_with(Scope::All, Vec::new(), Vec::new());

      let _el: Element<'_, Message> = view(&state, &required(), now());
    }
  }

  mod dispatch {
    use super::*;

    #[tokio::test]
    async fn it_dispatches_every_message_variant() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = populated();
      let n = now();

      let _ = update(&mut state, Message::FilterSelected(Filter::Ready), &db, n);
      let _ = update(&mut state, Message::GroupBySelected(GroupBy::Activity), &db, n);
      let _ = update(&mut state, Message::TabSelected(Tab::Jobs), &db, n);
      let _ = update(&mut state, Message::Tick, &db, n);
      let _ = update(&mut state, Message::PickerToggled, &db, n);
      let _ = update(&mut state, Message::TweaksToggled, &db, n);
      let _ = update(
        &mut state,
        Message::TweakChanged(tweaks::Tweak::ShowRail(false)),
        &db,
        n,
      );
      let _ = update(&mut state, Message::ReauthRequested(1), &db, n);
      let _ = update(&mut state, Message::ScopeSelected(Scope::Char(1)), &db, n);

      let fresh = Loaded {
        jobs: Vec::new(),
        roster: Vec::new(),
        scope: state.active,
      };
      let _ = update(&mut state, Message::Loaded(Box::new(fresh)), &db, n);
      let stale = Loaded {
        jobs: Vec::new(),
        roster: Vec::new(),
        scope: Scope::Char(424_242),
      };
      let _ = update(&mut state, Message::Loaded(Box::new(stale)), &db, n);
    }
  }

  mod scope_gate {
    use super::*;

    #[test]
    fn it_gates_a_char_scope_missing_the_required_scope() {
      let state = state_with(Scope::Char(1), vec![character_owner(1, None)], Vec::new());

      assert!(state.scope_gate().is_some());
    }

    #[test]
    fn it_does_not_gate_an_authorized_char_scope() {
      let granted = granted();
      let state = state_with(Scope::Char(1), vec![character_owner(1, Some(&granted))], Vec::new());

      assert!(state.scope_gate().is_none());
    }

    #[test]
    fn it_never_gates_the_combined_scope() {
      let state = state_with(Scope::All, vec![character_owner(1, None)], Vec::new());

      assert!(state.scope_gate().is_none());
    }
  }

  mod visible_jobs {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_drops_unauthorized_characters_but_keeps_corporations() {
      let granted = granted();
      let state = state_with(
        Scope::All,
        vec![
          character_owner(1, Some(&granted)),
          character_owner(2, None),
          corporation_owner(98),
        ],
        vec![
          job(Owner::Character(1), 10, Activity::Manufacturing, "2026-06-13T14:00:00Z"),
          job(Owner::Character(2), 11, Activity::Manufacturing, "2026-06-13T14:00:00Z"),
          job(Owner::Corporation(98), 12, Activity::Reactions, "2026-06-13T14:00:00Z"),
        ],
      );

      let visible = state.visible_jobs();

      assert_eq!(visible.len(), 2);
      assert!(visible.iter().all(|job| job.owner != Owner::Character(2)));
    }
  }

  mod unauthorized_characters {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_names_characters_missing_the_industry_scope() {
      let granted = granted();
      let state = state_with(
        Scope::All,
        vec![character_owner(1, Some(&granted)), character_owner(2, None)],
        Vec::new(),
      );

      let unauthorized = state.unauthorized_characters();

      assert_eq!(unauthorized.len(), 1);
      assert_eq!(unauthorized[0].id, 2);
    }
  }
}
