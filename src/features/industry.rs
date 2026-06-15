mod blueprints;
mod jobs;
mod loaders;
mod planner;
#[allow(dead_code)]
mod planner_loaders;
#[allow(dead_code)]
mod planner_model;
mod shell;
mod side_rail;
mod switcher;

use std::time::Duration;

use chrono::{DateTime, Utc};
use iced::{Element, Subscription, Task};

pub use self::loaders::{Activity, Blueprint, IndustryJob, Loaded, Owner, RosterOwner};
use self::planner::Planner;
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
pub enum GroupBy {
  Activity,
  Facility,
  #[default]
  None,
  Owner,
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
  Blueprints,
  #[default]
  Jobs,
  Planner,
}

impl Tab {
  pub const ALL: [Tab; 3] = [Tab::Jobs, Tab::Blueprints, Tab::Planner];

  pub fn label(self) -> &'static str {
    match self {
      Tab::Blueprints => "Blueprints",
      Tab::Jobs => "Jobs",
      Tab::Planner => "Planner",
    }
  }
}

/// The All / Originals / Copies segmented filter on the Blueprints tab.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BlueprintKind {
  #[default]
  All,
  Originals,
  Copies,
}

/// The Name / ME / Runs sort toggle on the Blueprints tab.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BlueprintSort {
  #[default]
  Name,
  MaterialEfficiency,
  Runs,
}

#[derive(Clone, Debug)]
pub enum Message {
  BlueprintKindSelected(BlueprintKind),
  BlueprintScrolled { absolute: f32 },
  BlueprintSearchChanged(String),
  BlueprintSortSelected(BlueprintSort),
  FilterSelected(Filter),
  GroupBySelected(GroupBy),
  Loaded(Box<Loaded>),
  PickerToggled,
  Planner(planner::Message),
  PlannerLoaded(Box<planner_loaders::PlannerData>),
  ReauthRequested(i64),
  ScopeSelected(Scope),
  TabSelected(Tab),
  Tick,
}

#[derive(Debug)]
pub struct State {
  active: Scope,
  blueprint_kind: BlueprintKind,
  blueprint_scroll_offset: f32,
  blueprint_search: String,
  blueprint_sort: BlueprintSort,
  blueprints: Vec<Blueprint>,
  filter: Filter,
  group_by: GroupBy,
  jobs: Vec<IndustryJob>,
  picker_open: bool,
  planner: Planner,
  required_scopes: Vec<&'static str>,
  roster: Vec<RosterOwner>,
  tab: Tab,
}

impl State {
  pub fn new(active: i64, required_scopes: Vec<&'static str>) -> Self {
    State {
      active: if active == EMPTY_INDUSTRY_SELECTION {
        Scope::All
      } else {
        Scope::Char(active)
      },
      blueprint_kind: BlueprintKind::default(),
      blueprint_scroll_offset: 0.0,
      blueprint_search: String::new(),
      blueprint_sort: BlueprintSort::default(),
      blueprints: Vec::new(),
      filter: Filter::default(),
      group_by: GroupBy::default(),
      jobs: Vec::new(),
      picker_open: false,
      planner: Planner::new(),
      required_scopes,
      roster: Vec::new(),
      tab: Tab::default(),
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

  pub(super) fn blueprint_kind(&self) -> BlueprintKind {
    self.blueprint_kind
  }

  pub(super) fn blueprint_scroll_offset(&self) -> f32 {
    self.blueprint_scroll_offset
  }

  pub(super) fn blueprint_search(&self) -> &str {
    &self.blueprint_search
  }

  pub(super) fn blueprint_sort(&self) -> BlueprintSort {
    self.blueprint_sort
  }

  /// Blueprints visible under the current scope. Corporation blueprints are always shown; a character's blueprints are
  /// hidden when that pilot is missing the required industry scope (mirroring [`visible_jobs`]).
  pub(super) fn visible_blueprints(&self) -> Vec<&Blueprint> {
    self
      .blueprints
      .iter()
      .filter(|blueprint| self.is_authorized(blueprint.owner))
      .collect()
  }

  pub(super) fn filter(&self) -> Filter {
    self.filter
  }

  pub(super) fn group_by(&self) -> GroupBy {
    self.group_by
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

  pub(super) fn planner(&self) -> &Planner {
    &self.planner
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

fn delete_plan(db: &Database, id: i64) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      let _ = crate::store::repo::industry::delete_plan(&db, id).await;
      list_saved_plans(&db).await
    },
    |plans| Message::Planner(planner::Message::PlansListed(plans)),
  )
}

fn list_plans(db: &Database) -> Task<Message> {
  let db = db.clone();
  Task::perform(async move { list_saved_plans(&db).await }, |plans| {
    Message::Planner(planner::Message::PlansListed(plans))
  })
}

/// Lists every saved plan with its full tree so the Plans pane can recompute economics at current
/// prices. A plan whose node tree fails to load is dropped rather than shown without its config.
async fn list_saved_plans(db: &Database) -> Vec<planner::SavedPlanData> {
  let plans = crate::store::repo::industry::list_plans(db).await.unwrap_or_default();
  let mut out = Vec::with_capacity(plans.len());
  for plan in plans {
    if let Ok(Some(tree)) = crate::store::repo::industry::load_plan(db, plan.id()).await {
      out.push(planner::SavedPlanData {
        id: plan.id(),
        name: plan.name().to_owned(),
        tree,
      });
    }
  }
  out
}

fn load_plan(db: &Database, id: i64) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move { crate::store::repo::industry::load_plan(&db, id).await.ok().flatten() },
    |tree| match tree {
      Some(tree) => Message::Planner(planner::Message::PlanRestored(Box::new(tree))),
      // Plan missing or unreadable — emit a no-op rather than an error.
      None => Message::Tick,
    },
  )
}

fn load_planner(db: &Database, scope: Scope) -> Task<Message> {
  planner::load(db.clone(), scope).map(|data| Message::PlannerLoaded(Box::new(data)))
}

fn save_plan(db: &Database, name: String, tree: crate::store::repo::industry::PlanTree) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      let _ = crate::store::repo::industry::create_plan(&db, &name, &tree).await;
      list_saved_plans(&db).await
    },
    |plans| Message::Planner(planner::Message::PlansListed(plans)),
  )
}

pub fn subscription(_state: &State) -> Subscription<Message> {
  iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick)
}

pub fn update(state: &mut State, message: Message, db: &Database, _now: DateTime<Utc>) -> Task<Message> {
  match message {
    Message::BlueprintKindSelected(kind) => {
      state.blueprint_kind = kind;
      state.blueprint_scroll_offset = 0.0;
      Task::none()
    }
    Message::BlueprintScrolled {
      absolute,
    } => {
      state.blueprint_scroll_offset = absolute;
      Task::none()
    }
    Message::BlueprintSearchChanged(query) => {
      state.blueprint_search = query;
      state.blueprint_scroll_offset = 0.0;
      Task::none()
    }
    Message::BlueprintSortSelected(sort) => {
      state.blueprint_sort = sort;
      state.blueprint_scroll_offset = 0.0;
      Task::none()
    }
    Message::FilterSelected(filter) => {
      state.filter = filter;
      Task::none()
    }
    Message::GroupBySelected(group_by) => {
      state.group_by = group_by;
      Task::none()
    }
    Message::Loaded(loaded) => {
      let Loaded {
        blueprints,
        jobs,
        roster,
        scope,
      } = *loaded;
      // Drop results that belong to a scope the user already navigated away from.
      if scope == state.active {
        state.blueprints = blueprints;
        state.jobs = jobs;
        state.roster = roster;
      }
      Task::none()
    }
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Task::none()
    }
    Message::Planner(planner_message) => match planner_message {
      planner::Message::ShoppingListCopied => {
        state.planner.update(planner_message);
        iced::clipboard::write(state.planner.shopping_list())
      }
      planner::Message::PlanSaveRequested => match (state.planner.snapshot(), state.planner.save_name()) {
        (Some(tree), Some(name)) => save_plan(db, name, tree),
        _ => Task::none(),
      },
      planner::Message::PlanLoadRequested(id) => load_plan(db, id),
      planner::Message::PlanDeleteRequested(id) => delete_plan(db, id),
      other => {
        // Breaking down or collapsing a node rebuilds the Material Plan scrollable, which makes
        // iced reset the scroll offset to zero; re-apply the tracked offset afterward.
        let restore_scroll = matches!(
          other,
          planner::Message::NodeBrokenDown { .. } | planner::Message::NodeCollapsed { .. }
        );
        state.planner.update(other);
        if restore_scroll {
          planner::restore_material_scroll(state.planner.material_scroll_offset())
        } else {
          Task::none()
        }
      }
    },
    Message::PlannerLoaded(data) => {
      state.planner.apply_data(*data);
      Task::none()
    }
    Message::ReauthRequested(_) => Task::none(),
    Message::ScopeSelected(scope) => {
      state.active = scope;
      state.picker_open = false;
      state.blueprint_scroll_offset = 0.0;
      let mut tasks = vec![reload(db, scope, &state.required_scopes)];
      if state.planner.is_loaded() {
        tasks.push(load_planner(db, scope));
      }
      Task::batch(tasks)
    }
    Message::TabSelected(tab) => {
      state.tab = tab;
      if tab == Tab::Planner {
        let mut tasks = vec![list_plans(db)];
        if !state.planner.is_loaded() {
          tasks.push(load_planner(db, state.active));
        }
        return Task::batch(tasks);
      }
      Task::none()
    }
    Message::Tick => Task::none(),
  }
}

pub fn view<'a>(state: &'a State, required_scopes: &[&'static str], now: DateTime<Utc>) -> Element<'a, Message> {
  let _ = required_scopes;
  shell::shell(state, now)
}

#[cfg(test)]
mod tests {
  use super::{loaders::SlotCaps, *};
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
    let mut state = State::new(EMPTY_INDUSTRY_SELECTION, required());
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
    let mut state = state_with(Scope::All, roster, jobs);
    state.blueprints = vec![
      blueprint(Owner::Character(1), 1, "Rifter Blueprint", -1, 10, 20, false),
      blueprint(Owner::Character(1), 2, "Hobgoblin I Blueprint", 12, 4, 8, false),
      blueprint(Owner::Character(2), 3, "Hidden Blueprint", 5, 0, 0, false),
      blueprint(Owner::Corporation(98), 4, "Sulfuric Acid Reaction", -1, 0, 0, true),
    ];
    state
  }

  fn blueprint(owner: Owner, item_id: i64, name: &str, runs: i64, me: i64, te: i64, reaction: bool) -> Blueprint {
    Blueprint {
      group_name: "Frigate Blueprint".to_owned(),
      item_id,
      location: "Jita IV - Moon 4".to_owned(),
      material_efficiency: me,
      name: name.to_owned(),
      owner,
      product_name: Some("Rifter".to_owned()),
      reaction,
      runs,
      system_name: Some("Jita".to_owned()),
      time_efficiency: te,
      type_id: 681,
    }
  }

  mod blueprints_state {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_drops_unauthorized_character_blueprints_but_keeps_corporations() {
      let state = populated();
      let visible = state.visible_blueprints();

      // The character-2 blueprint is hidden (missing scope); originals/copies for the authorized pilot and the
      // corporation remain.
      assert_eq!(visible.len(), 3);
      assert!(visible.iter().all(|bp| bp.owner != Owner::Character(2)));
    }

    #[tokio::test]
    async fn it_stores_the_blueprint_scroll_offset() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = populated();

      let _ = update(
        &mut state,
        Message::BlueprintScrolled {
          absolute: 512.0,
        },
        &db,
        now(),
      );

      assert_eq!(state.blueprint_scroll_offset(), 512.0);
    }

    #[tokio::test]
    async fn it_resets_the_scroll_offset_when_the_filter_changes() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = populated();
      let _ = update(
        &mut state,
        Message::BlueprintScrolled {
          absolute: 512.0,
        },
        &db,
        now(),
      );

      let _ = update(
        &mut state,
        Message::BlueprintSearchChanged("rifter".to_owned()),
        &db,
        now(),
      );

      assert_eq!(state.blueprint_search(), "rifter");
      assert_eq!(state.blueprint_scroll_offset(), 0.0);
    }
  }

  mod rendering {
    use super::*;

    #[test]
    fn it_renders_the_combined_view() {
      let state = populated();

      let _el: Element<'_, Message> = view(&state, &required(), now());
    }

    #[test]
    fn it_renders_each_group_by() {
      let mut state = populated();
      for group_by in [GroupBy::None, GroupBy::Owner, GroupBy::Activity, GroupBy::Facility] {
        state.group_by = group_by;
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
    fn it_renders_the_blueprints_tab_for_each_kind_and_sort() {
      let mut state = populated();
      state.tab = Tab::Blueprints;
      for kind in [BlueprintKind::All, BlueprintKind::Originals, BlueprintKind::Copies] {
        state.blueprint_kind = kind;
        for sort in [
          BlueprintSort::Name,
          BlueprintSort::MaterialEfficiency,
          BlueprintSort::Runs,
        ] {
          state.blueprint_sort = sort;
          let _el: Element<'_, Message> = view(&state, &required(), now());
        }
      }
    }

    #[test]
    fn it_renders_an_empty_blueprints_tab() {
      let mut state = state_with(Scope::All, Vec::new(), Vec::new());
      state.tab = Tab::Blueprints;

      let _el: Element<'_, Message> = view(&state, &required(), now());
    }

    #[test]
    fn it_renders_the_planner_tab_loading_and_loaded() {
      use planner_loaders::{CatalogEntry, Category, PlannerData, Recipe};
      use planner_model::Material;

      let mut state = populated();
      state.tab = Tab::Planner;

      {
        let _loading: Element<'_, Message> = view(&state, &required(), now());
      }

      let mut data = PlannerData::default();
      data.recipes.insert(
        22_544,
        Recipe {
          activity_id: 1,
          blueprint_type_id: 22_545,
          is_reaction: false,
          materials: vec![Material::new(17_478, 2), Material::new(34, 100)],
          output_per_run: 1,
          time_per_run: 3_600,
        },
      );
      data.recipes.insert(
        17_478,
        Recipe {
          activity_id: 1,
          blueprint_type_id: 17_479,
          is_reaction: false,
          materials: vec![Material::new(34, 50)],
          output_per_run: 1,
          time_per_run: 600,
        },
      );
      data.names.insert(22_544, "Hulk".to_owned());
      data.names.insert(17_478, "Retriever".to_owned());
      data.names.insert(34, "Tritanium".to_owned());
      data.prices.insert(22_544, 200_000_000.0);
      data.prices.insert(17_478, 30_000_000.0);
      data.prices.insert(34, 5.0);
      data.catalog.push(CatalogEntry {
        category: Category::Ship,
        group_name: "Mining Barge".to_owned(),
        is_reaction: false,
        name: "Hulk".to_owned(),
        type_id: 22_544,
        volume: 3_750.0,
      });
      state.planner.apply_data(data);

      // Exercise the loaded body, a breakdown, the Plans stub, and a context menu.
      {
        let _loaded: Element<'_, Message> = view(&state, &required(), now());
      }
      state.planner.update(planner::Message::NodeBrokenDown {
        mat: 17_478,
        parent: Vec::new(),
      });
      state
        .planner
        .update(planner::Message::RightTabSelected(planner::RightTab::Plans));
      {
        let _empty_plans: Element<'_, Message> = view(&state, &required(), now());
      }
      // A listed plan exercises the populated Plans pane: name, recomputed economics, load/delete actions.
      let tree = state.planner.snapshot().unwrap();
      state
        .planner
        .update(planner::Message::PlansListed(vec![planner::SavedPlanData {
          id: 1,
          name: "Hulk run".to_owned(),
          tree,
        }]));
      {
        let _saved_plans: Element<'_, Message> = view(&state, &required(), now());
      }
      state
        .planner
        .update(planner::Message::RightTabSelected(planner::RightTab::Detail));
      state
        .planner
        .update(planner::Message::CursorMoved(iced::Point::new(20.0, 40.0)));
      state.planner.update(planner::Message::MaterialRightPressed {
        mat: 34,
        parent: Vec::new(),
      });
      {
        let _menu: Element<'_, Message> = view(&state, &required(), now());
      }
    }

    #[test]
    fn it_renders_the_scope_picker_overlay() {
      let mut state = populated();
      state.picker_open = true;

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
      let _ = update(&mut state, Message::TabSelected(Tab::Blueprints), &db, n);
      let _ = update(
        &mut state,
        Message::BlueprintKindSelected(BlueprintKind::Originals),
        &db,
        n,
      );
      let _ = update(&mut state, Message::BlueprintSortSelected(BlueprintSort::Runs), &db, n);
      let _ = update(&mut state, Message::BlueprintSearchChanged("rifter".to_owned()), &db, n);
      let _ = update(
        &mut state,
        Message::BlueprintScrolled {
          absolute: 240.0,
        },
        &db,
        n,
      );
      let _ = update(&mut state, Message::TabSelected(Tab::Planner), &db, n);
      let _ = update(&mut state, Message::PlannerLoaded(Box::default()), &db, n);
      let _ = update(&mut state, Message::Planner(planner::Message::RunsChanged(5)), &db, n);
      let _ = update(
        &mut state,
        Message::Planner(planner::Message::ShoppingListCopied),
        &db,
        n,
      );
      let _ = update(
        &mut state,
        Message::Planner(planner::Message::PlanSaveRequested),
        &db,
        n,
      );
      let _ = update(
        &mut state,
        Message::Planner(planner::Message::PlanLoadRequested(1)),
        &db,
        n,
      );
      let _ = update(
        &mut state,
        Message::Planner(planner::Message::PlanDeleteRequested(1)),
        &db,
        n,
      );
      let _ = update(
        &mut state,
        Message::Planner(planner::Message::PlansListed(Vec::new())),
        &db,
        n,
      );
      let _ = update(
        &mut state,
        Message::Planner(planner::Message::MaterialScrolled {
          absolute: 160.0,
        }),
        &db,
        n,
      );
      let _ = update(
        &mut state,
        Message::Planner(planner::Message::NodeBrokenDown {
          mat: 34,
          parent: Vec::new(),
        }),
        &db,
        n,
      );
      let _ = update(&mut state, Message::Tick, &db, n);
      let _ = update(&mut state, Message::PickerToggled, &db, n);
      let _ = update(&mut state, Message::ReauthRequested(1), &db, n);
      let _ = update(&mut state, Message::ScopeSelected(Scope::Char(1)), &db, n);

      let fresh = Loaded {
        blueprints: Vec::new(),
        jobs: Vec::new(),
        roster: Vec::new(),
        scope: state.active,
      };
      let _ = update(&mut state, Message::Loaded(Box::new(fresh)), &db, n);
      let stale = Loaded {
        blueprints: Vec::new(),
        jobs: Vec::new(),
        roster: Vec::new(),
        scope: Scope::Char(424_242),
      };
      let _ = update(&mut state, Message::Loaded(Box::new(stale)), &db, n);
    }
  }

  mod list_saved_plans {
    use pretty_assertions::assert_eq;

    use crate::store::repo::industry::{self as industry_repo, PlanNode, PlanTree};

    fn sample_tree() -> PlanTree {
      PlanTree {
        nodes: vec![PlanNode {
          facility_system: None,
          me: 10,
          path: vec![],
          te: 20,
        }],
        product_type_id: 22_544,
        root_facility_system: None,
        runs: 1,
      }
    }

    #[tokio::test]
    async fn it_returns_each_saved_plan_with_its_tree_newest_first() {
      let db = crate::store::open_test().await.unwrap();
      industry_repo::create_plan(&db, "First", &sample_tree()).await.unwrap();
      industry_repo::create_plan(&db, "Second", &sample_tree()).await.unwrap();

      let listed = super::list_saved_plans(&db).await;

      assert_eq!(listed.len(), 2);
      assert_eq!(listed[0].tree, sample_tree());
    }

    #[tokio::test]
    async fn it_drops_a_plan_once_deleted() {
      let db = crate::store::open_test().await.unwrap();
      let plan = industry_repo::create_plan(&db, "Doomed", &sample_tree()).await.unwrap();

      industry_repo::delete_plan(&db, plan.id()).await.unwrap();

      assert!(super::list_saved_plans(&db).await.is_empty());
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
